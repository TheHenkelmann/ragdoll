// SPDX-License-Identifier: AGPL-3.0-only

use std::collections::HashMap;
use std::time::{Duration, Instant};

use serde_json::json;

use crate::db::DbPool;
use crate::system_metrics::SystemSnapshot;
use crate::webhooks::dispatch::deliver_webhook;

pub const INGEST_STATUS_TYPE: &str = "ingest_status";
pub const HOST_UTILIZATION_TYPE: &str = "host_utilization";

pub const INGEST_STATUS_EVENTS: &[&str] = &["completed", "failed"];

pub const HOST_UTILIZATION_EVENTS: &[&str] = &[
    "cpu_high",
    "cpu_critical",
    "memory_high",
    "memory_critical",
    "cpu_recovered",
    "memory_recovered",
];

const COOLDOWN: Duration = Duration::from_secs(15 * 60);

const CPU_HIGH_THRESHOLD: f64 = 85.0;
const CPU_CRITICAL_THRESHOLD: f64 = 95.0;
const CPU_RECOVERY_THRESHOLD: f64 = 75.0;

const MEMORY_HIGH_THRESHOLD: f64 = 85.0;
const MEMORY_CRITICAL_THRESHOLD: f64 = 95.0;
const MEMORY_RECOVERY_THRESHOLD: f64 = 75.0;

const CPU_HIGH_DURATION_SECS: u32 = 15;
const CPU_CRITICAL_DURATION_SECS: u32 = 15;
const MEMORY_HIGH_DURATION_SECS: u32 = 15;
const MEMORY_CRITICAL_DURATION_SECS: u32 = 15;
const RECOVERY_DURATION_SECS: u32 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostUtilizationEvent {
    CpuHigh,
    CpuCritical,
    MemoryHigh,
    MemoryCritical,
    CpuRecovered,
    MemoryRecovered,
}

impl HostUtilizationEvent {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CpuHigh => "cpu_high",
            Self::CpuCritical => "cpu_critical",
            Self::MemoryHigh => "memory_high",
            Self::MemoryCritical => "memory_critical",
            Self::CpuRecovered => "cpu_recovered",
            Self::MemoryRecovered => "memory_recovered",
        }
    }
}

pub fn is_valid_webhook_type(webhook_type: &str) -> bool {
    webhook_type == INGEST_STATUS_TYPE || webhook_type == HOST_UTILIZATION_TYPE
}

pub fn is_known_event(event: &str) -> bool {
    INGEST_STATUS_EVENTS.contains(&event) || HOST_UTILIZATION_EVENTS.contains(&event)
}

pub fn is_host_event(event: &str) -> bool {
    HOST_UTILIZATION_EVENTS.contains(&event)
}

/// Validate that every event is a known event from any category. Empty is allowed
/// (a webhook with no events simply never fires).
pub fn validate_known_events(events: &[String]) -> Result<(), String> {
    for event in events {
        if !is_known_event(event) {
            return Err(format!("unknown webhook event '{event}'"));
        }
    }
    Ok(())
}

pub fn default_events_for_type(webhook_type: &str) -> Vec<String> {
    match webhook_type {
        HOST_UTILIZATION_TYPE => HOST_UTILIZATION_EVENTS
            .iter()
            .filter(|event| !event.ends_with("_recovered"))
            .map(|event| event.to_string())
            .collect(),
        _ => INGEST_STATUS_EVENTS.iter().map(|event| event.to_string()).collect(),
    }
}

pub fn validate_events(webhook_type: &str, events: &[String]) -> Result<(), String> {
    let allowed: &[&str] = match webhook_type {
        INGEST_STATUS_TYPE => INGEST_STATUS_EVENTS,
        HOST_UTILIZATION_TYPE => HOST_UTILIZATION_EVENTS,
        other => return Err(format!("unsupported webhook type: {other}")),
    };
    for event in events {
        if !allowed.contains(&event.as_str()) {
            return Err(format!("unsupported event '{event}' for type '{webhook_type}'"));
        }
    }
    Ok(())
}

#[derive(Default)]
pub struct HostUtilizationAlertState {
    cpu_high_secs: u32,
    cpu_critical_secs: u32,
    memory_high_secs: u32,
    memory_critical_secs: u32,
    cpu_recovered_secs: u32,
    memory_recovered_secs: u32,
    cpu_alert_active: bool,
    memory_alert_active: bool,
    last_fired: HashMap<String, Instant>,
}

impl HostUtilizationAlertState {
    pub fn evaluate(&mut self, snapshot: &SystemSnapshot) -> Vec<HostUtilizationEvent> {
        let memory_pct = memory_used_percent(snapshot);
        let mut fired = Vec::new();

        self.cpu_high_secs = if snapshot.cpu_percent > CPU_HIGH_THRESHOLD {
            self.cpu_high_secs.saturating_add(1)
        } else {
            0
        };
        self.cpu_critical_secs = if snapshot.cpu_percent > CPU_CRITICAL_THRESHOLD {
            self.cpu_critical_secs.saturating_add(1)
        } else {
            0
        };
        self.memory_high_secs = if memory_pct > MEMORY_HIGH_THRESHOLD {
            self.memory_high_secs.saturating_add(1)
        } else {
            0
        };
        self.memory_critical_secs = if memory_pct > MEMORY_CRITICAL_THRESHOLD {
            self.memory_critical_secs.saturating_add(1)
        } else {
            0
        };
        self.cpu_recovered_secs = if snapshot.cpu_percent < CPU_RECOVERY_THRESHOLD {
            self.cpu_recovered_secs.saturating_add(1)
        } else {
            0
        };
        self.memory_recovered_secs = if memory_pct < MEMORY_RECOVERY_THRESHOLD {
            self.memory_recovered_secs.saturating_add(1)
        } else {
            0
        };

        if self.cpu_critical_secs >= CPU_CRITICAL_DURATION_SECS
            && self.cooldown_elapsed(HostUtilizationEvent::CpuCritical)
        {
            fired.push(HostUtilizationEvent::CpuCritical);
            self.cpu_alert_active = true;
            self.mark_fired(HostUtilizationEvent::CpuCritical);
        } else if self.cpu_high_secs >= CPU_HIGH_DURATION_SECS
            && self.cooldown_elapsed(HostUtilizationEvent::CpuHigh)
        {
            fired.push(HostUtilizationEvent::CpuHigh);
            self.cpu_alert_active = true;
            self.mark_fired(HostUtilizationEvent::CpuHigh);
        }

        if self.memory_critical_secs >= MEMORY_CRITICAL_DURATION_SECS
            && self.cooldown_elapsed(HostUtilizationEvent::MemoryCritical)
        {
            fired.push(HostUtilizationEvent::MemoryCritical);
            self.memory_alert_active = true;
            self.mark_fired(HostUtilizationEvent::MemoryCritical);
        } else if self.memory_high_secs >= MEMORY_HIGH_DURATION_SECS
            && self.cooldown_elapsed(HostUtilizationEvent::MemoryHigh)
        {
            fired.push(HostUtilizationEvent::MemoryHigh);
            self.memory_alert_active = true;
            self.mark_fired(HostUtilizationEvent::MemoryHigh);
        }

        if self.cpu_alert_active
            && self.cpu_recovered_secs >= RECOVERY_DURATION_SECS
            && self.cooldown_elapsed(HostUtilizationEvent::CpuRecovered)
        {
            fired.push(HostUtilizationEvent::CpuRecovered);
            self.cpu_alert_active = false;
            self.cpu_high_secs = 0;
            self.cpu_critical_secs = 0;
            self.mark_fired(HostUtilizationEvent::CpuRecovered);
        }

        if self.memory_alert_active
            && self.memory_recovered_secs >= RECOVERY_DURATION_SECS
            && self.cooldown_elapsed(HostUtilizationEvent::MemoryRecovered)
        {
            fired.push(HostUtilizationEvent::MemoryRecovered);
            self.memory_alert_active = false;
            self.memory_high_secs = 0;
            self.memory_critical_secs = 0;
            self.mark_fired(HostUtilizationEvent::MemoryRecovered);
        }

        fired
    }

    fn cooldown_elapsed(&self, event: HostUtilizationEvent) -> bool {
        self.last_fired
            .get(event.as_str())
            .is_none_or(|instant| instant.elapsed() >= COOLDOWN)
    }

    fn mark_fired(&mut self, event: HostUtilizationEvent) {
        self.last_fired
            .insert(event.as_str().to_string(), Instant::now());
    }
}

pub async fn dispatch_event(
    pool: &DbPool,
    snapshot: &SystemSnapshot,
    event: HostUtilizationEvent,
) -> Result<(), crate::db::DbError> {
    let conn = pool.connect_one().await?;
    let mut rows = conn
        .query(
            "SELECT id, release_id, url, secret, events
             FROM webhooks
             WHERE active = 1",
            (),
        )
        .await?;

    let memory_pct = memory_used_percent(snapshot);

    while let Some(row) = rows.next().await? {
        let webhook_id: String = row.get(0)?;
        let release_id: String = row.get(1)?;
        let url: String = row.get(2)?;
        let secret: String = row.get(3)?;
        let events_raw: String = row.get(4)?;
        let subscribed: Vec<String> = serde_json::from_str(&events_raw).unwrap_or_default();
        if !subscribed.iter().any(|name| name == event.as_str()) {
            continue;
        }

        let payload = json!({
            "type": HOST_UTILIZATION_TYPE,
            "event": event.as_str(),
            "scope": "host",
            "note": "Host-wide utilization, not scoped to a release, stage, or Ragdoll process.",
            "release_id": release_id,
            "cpu_percent": snapshot.cpu_percent,
            "memory_used_bytes": snapshot.memory_used_bytes,
            "memory_total_bytes": snapshot.memory_total_bytes,
            "memory_available_bytes": snapshot.memory_available_bytes,
            "memory_used_percent": memory_pct,
            "cpu_cores": snapshot.cpu_cores,
            "ts": time::OffsetDateTime::now_utc().unix_timestamp(),
        });
        let body = payload.to_string();
        deliver_webhook(
            pool,
            &webhook_id,
            event.as_str(),
            &url,
            &secret,
            &body,
        )
        .await;
    }

    Ok(())
}

fn memory_used_percent(snapshot: &SystemSnapshot) -> f64 {
    if snapshot.memory_total_bytes == 0 {
        return 0.0;
    }
    (snapshot.memory_used_bytes as f64 / snapshot.memory_total_bytes as f64) * 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(cpu: f64, mem_used: u64, mem_total: u64) -> SystemSnapshot {
        SystemSnapshot {
            cpu_percent: cpu,
            memory_used_bytes: mem_used,
            memory_total_bytes: mem_total,
            memory_available_bytes: mem_total.saturating_sub(mem_used),
            cpu_cores: 8,
        }
    }

    fn low_memory_snapshot(cpu: f64) -> SystemSnapshot {
        snapshot(cpu, 500, 1000)
    }

    #[test]
    fn cpu_high_fires_after_duration() {
        let mut state = HostUtilizationAlertState::default();
        let snap = low_memory_snapshot(90.0);
        let mut events = Vec::new();
        for _ in 0..(CPU_HIGH_DURATION_SECS - 1) {
            events = state.evaluate(&snap);
        }
        assert!(events.is_empty());
        events = state.evaluate(&snap);
        assert_eq!(events, vec![HostUtilizationEvent::CpuHigh]);
        assert!(state.cpu_alert_active);
    }

    #[test]
    fn cpu_recovered_only_after_alert() {
        let mut state = HostUtilizationAlertState::default();
        let high = low_memory_snapshot(90.0);
        for _ in 0..60 {
            state.evaluate(&high);
        }
        let low = low_memory_snapshot(50.0);
        let mut events = Vec::new();
        for _ in 0..59 {
            events = state.evaluate(&low);
        }
        assert!(events.is_empty());
        events = state.evaluate(&low);
        assert_eq!(events, vec![HostUtilizationEvent::CpuRecovered]);
    }

    #[test]
    fn cpu_recovered_not_sent_without_prior_alert() {
        let mut state = HostUtilizationAlertState::default();
        let low = low_memory_snapshot(50.0);
        for _ in 0..120 {
            let events = state.evaluate(&low);
            assert!(events.is_empty());
        }
    }

    #[test]
    fn validate_events_rejects_unknown_host_event() {
        let err = validate_events(
            HOST_UTILIZATION_TYPE,
            &["cpu_high".to_string(), "unknown".to_string()],
        )
        .expect_err("expected validation error");
        assert!(err.contains("unknown"));
    }
}
