// SPDX-License-Identifier: AGPL-3.0-only

pub mod analytics;
pub mod api_keys;
pub mod auth;
pub mod batch;
pub mod chunks;
pub mod db_viewer;
pub mod error;
pub mod health;
pub mod models;
pub mod openapi;
pub mod queries;
pub mod releases;
pub mod router;
pub mod settings;
pub mod sources;
pub mod sources_list;
pub mod stages;
pub mod users;

pub use router::build_router;
