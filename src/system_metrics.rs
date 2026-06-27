// SPDX-License-Identifier: AGPL-3.0-only

use std::time::Duration;

use serde::Serialize;
use sysinfo::{MemoryRefreshKind, RefreshKind, System};

use crate::db::DbPool;

const RETAIN_DAYS: u32 = 30;
const MAX_CHART_POINTS: usize = 720;

#[derive(Debug, Clone, Serialize)]
pub struct SystemSnapshot {
    pub cpu_percent: f64,
    pub memory_used_bytes: u64,
    pub memory_total_bytes: u64,
    pub memory_available_bytes: u64,
    pub cpu_cores: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct SystemMetricSample {
    pub recorded_at: String,
    pub cpu_percent: f64,
    pub memory_used_bytes: i64,
    pub memory_total_bytes: i64,
}

pub fn collect_snapshot() -> SystemSnapshot {
    let mut system = System::new_with_specifics(
        RefreshKind::nothing()
            .with_cpu(sysinfo::CpuRefreshKind::everything())
            .with_memory(MemoryRefreshKind::everything()),
    );
    system.refresh_cpu_usage();
    std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
    system.refresh_cpu_usage();

    let used = system.used_memory();
    let total = system.total_memory();
    // Derive "available" from the same source as used/total so the dashboard's
    // "RAM now" and "RAM available" cards always reconcile (used + available = total).
    // sysinfo's available_memory() is computed differently per platform and can
    // report 0 on macOS, which would otherwise contradict the used/total figures.
    let available = total.saturating_sub(used);

    SystemSnapshot {
        cpu_percent: f64::from(system.global_cpu_usage()),
        memory_used_bytes: used,
        memory_total_bytes: total,
        memory_available_bytes: available,
        cpu_cores: system.cpus().len(),
    }
}

pub async fn persist_sample(pool: &DbPool, snapshot: &SystemSnapshot) -> Result<(), crate::db::DbError> {
    let conn = pool.connect_one().await?;
    conn.execute(
        "INSERT INTO system_metrics (cpu_percent, memory_used_bytes, memory_total_bytes)
         VALUES (?1, ?2, ?3)",
        (
            snapshot.cpu_percent,
            snapshot.memory_used_bytes as i64,
            snapshot.memory_total_bytes as i64,
        ),
    )
    .await?;
    Ok(())
}

pub async fn purge_old_samples(pool: &DbPool) -> Result<(), crate::db::DbError> {
    let conn = pool.connect_one().await?;
    conn.execute(
        "DELETE FROM system_metrics WHERE recorded_at < datetime('now', ?1)",
        [format!("-{RETAIN_DAYS} days")],
    )
    .await?;
    Ok(())
}

pub async fn fetch_samples(
    pool: &DbPool,
    start: &str,
    end: &str,
) -> Result<Vec<SystemMetricSample>, crate::db::DbError> {
    let conn = pool.connect_one().await?;
    let mut rows = conn
        .query(
            "SELECT recorded_at, cpu_percent, memory_used_bytes, memory_total_bytes
             FROM system_metrics
             WHERE recorded_at >= ?1 AND recorded_at < date(?2, '+1 day')
             ORDER BY recorded_at",
            [start.to_string(), end.to_string()],
        )
        .await?;

    let mut samples = Vec::new();
    while let Some(row) = rows.next().await? {
        samples.push(SystemMetricSample {
            recorded_at: row.get(0)?,
            cpu_percent: row.get(1)?,
            memory_used_bytes: row.get(2)?,
            memory_total_bytes: row.get(3)?,
        });
    }
    Ok(downsample(samples, MAX_CHART_POINTS))
}

pub fn downsample_for_chart(samples: Vec<SystemMetricSample>) -> Vec<SystemMetricSample> {
    downsample(samples, MAX_CHART_POINTS)
}

fn downsample(samples: Vec<SystemMetricSample>, max_points: usize) -> Vec<SystemMetricSample> {
    if samples.len() <= max_points || max_points == 0 {
        return samples;
    }

    let bucket_size = samples.len().div_ceil(max_points);
    samples
        .chunks(bucket_size)
        .filter_map(average_bucket)
        .collect()
}

fn average_bucket(chunk: &[SystemMetricSample]) -> Option<SystemMetricSample> {
    let first = chunk.first()?;
    let last = chunk.last()?;
    let count = chunk.len() as f64;
    let cpu_percent = chunk.iter().map(|s| s.cpu_percent).sum::<f64>() / count;
    let memory_used_bytes =
        (chunk.iter().map(|s| s.memory_used_bytes as f64).sum::<f64>() / count).round() as i64;
    let memory_total_bytes = first.memory_total_bytes;

    Some(SystemMetricSample {
        recorded_at: last.recorded_at.clone(),
        cpu_percent,
        memory_used_bytes,
        memory_total_bytes,
    })
}

pub fn spawn_sampler(pool: DbPool) {
    let sample_pool = pool.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        interval.tick().await;
        let mut alert_state = crate::webhooks::host_utilization::HostUtilizationAlertState::default();

        loop {
            interval.tick().await;
            let pool = sample_pool.clone();
            let snapshot = tokio::task::spawn_blocking(collect_snapshot)
                .await
                .unwrap_or_else(|err| {
                    tracing::warn!(error = %err, "system metrics sample collection failed");
                    collect_snapshot()
                });
            if let Err(err) = persist_sample(&pool, &snapshot).await {
                tracing::warn!(error = %err, "system metrics sample persist failed");
            }
            let events = alert_state.evaluate(&snapshot);
            for event in events {
                if let Err(err) =
                    crate::webhooks::host_utilization::dispatch_event(&pool, &snapshot, event).await
                {
                    tracing::warn!(event = event.as_str(), error = %err, "host utilization webhook dispatch failed");
                }
            }
        }
    });

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(3600));
        interval.tick().await;
        loop {
            interval.tick().await;
            if let Err(err) = purge_old_samples(&pool).await {
                tracing::warn!(error = %err, "system metrics purge failed");
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downsample_averages_into_fewer_points() {
        let samples: Vec<_> = (0..100)
            .map(|i| SystemMetricSample {
                recorded_at: format!("2024-01-01T00:{i:02}:00Z"),
                cpu_percent: 10.0,
                memory_used_bytes: 100,
                memory_total_bytes: 1000,
            })
            .collect();
        let out = downsample(samples, 10);
        assert_eq!(out.len(), 10);
        assert!((out[0].cpu_percent - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn collect_snapshot_returns_positive_totals() {
        let snapshot = collect_snapshot();
        assert!(snapshot.cpu_cores > 0);
        assert!(snapshot.memory_total_bytes > 0);
    }
}
