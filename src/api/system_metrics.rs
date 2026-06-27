// SPDX-License-Identifier: AGPL-3.0-only

use std::sync::Arc;

use axum::extract::{Query, State};
use axum::Extension;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::api::error::ApiError;
use crate::api::router::AppState;
use crate::auth::{authorize, AuthContext, Permission};
use crate::system_metrics::{collect_snapshot, downsample_for_chart, fetch_samples, SystemMetricSample, SystemSnapshot};

#[derive(Debug, Deserialize)]
pub struct SystemMetricsParams {
    #[serde(default = "default_days")]
    pub days: u32,
    pub start: Option<String>,
    pub end: Option<String>,
}

fn default_days() -> u32 {
    14
}

#[derive(Debug, Serialize)]
pub struct SystemMetricsResponse {
    pub samples: Vec<SystemMetricSample>,
    pub current: SystemSnapshot,
}

pub async fn get_system_metrics(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<SystemMetricsParams>,
) -> Result<Json<SystemMetricsResponse>, ApiError> {
    authorize(&auth, Permission::AnalyticsRead)?;

    let days = params.days.clamp(1, 365);
    let samples = if params.start.is_some() || params.end.is_some() {
        let start = params
            .start
            .clone()
            .unwrap_or_else(|| "1970-01-01".to_string());
        let end = params
            .end
            .clone()
            .unwrap_or_else(|| "9999-12-31".to_string());
        fetch_samples(&state.pool, &start, &end)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?
    } else {
        fetch_samples_days(&state.pool, days).await?
    };

    let current = tokio::task::spawn_blocking(collect_snapshot)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(SystemMetricsResponse { samples, current }))
}

async fn fetch_samples_days(
    pool: &crate::db::DbPool,
    days: u32,
) -> Result<Vec<SystemMetricSample>, ApiError> {
    let conn = pool
        .connect_one()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut rows = conn
        .query(
            "SELECT recorded_at, cpu_percent, memory_used_bytes, memory_total_bytes
             FROM system_metrics
             WHERE recorded_at >= datetime('now', ?1)
             ORDER BY recorded_at",
            [format!("-{days} days")],
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let mut samples = Vec::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
    {
        samples.push(SystemMetricSample {
            recorded_at: row.get(0).map_err(|e| ApiError::internal(e.to_string()))?,
            cpu_percent: row.get(1).map_err(|e| ApiError::internal(e.to_string()))?,
            memory_used_bytes: row.get(2).map_err(|e| ApiError::internal(e.to_string()))?,
            memory_total_bytes: row.get(3).map_err(|e| ApiError::internal(e.to_string()))?,
        });
    }
    Ok(downsample_for_chart(samples))
}
