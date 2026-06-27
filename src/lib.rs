// SPDX-License-Identifier: AGPL-3.0-only

pub mod api;
pub mod auth;
pub mod backup;
pub mod cli;
pub mod config;
pub mod crypto;
pub mod db;
pub mod filter;
pub mod generation;
pub mod models;
pub mod release;
pub mod search;
pub mod settings;
pub mod staging;
pub mod system_metrics;
pub mod telemetry;
pub mod webhooks;

pub use api::{build_router, router::AppState};
pub use config::Config;
pub use db::DbPool;
