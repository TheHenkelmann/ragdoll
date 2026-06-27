// SPDX-License-Identifier: AGPL-3.0-only

pub mod bootstrap;
pub mod catalog;
pub mod custom_models;
pub mod download;
pub mod download_io;
pub mod embed;
pub mod mapping;
pub mod mock;
pub mod registry;
pub mod rerank;
pub mod traits;

pub use bootstrap::{
    collect_required_models, ensure_models, ensure_models_for_releases, list_local_models,
    list_supported_models,
};
pub use embed::EmbedModel;
pub use mock::{MockEmbedder, MockModelProvider};
pub use registry::ModelRegistry;
pub use rerank::RerankModel;
pub use traits::{Embedder, ModelProvider, Reranker};
