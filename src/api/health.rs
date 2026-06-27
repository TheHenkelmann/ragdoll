// SPDX-License-Identifier: AGPL-3.0-only

use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::api::router::AppState;

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub ready: bool,
    pub embedding_mismatch_count: usize,
}

pub async fn health(State(state): State<Arc<AppState>>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: if state.ready { "ok" } else { "starting" }.to_string(),
        ready: state.ready,
        embedding_mismatch_count: state.embedding_mismatches.len(),
    })
}
