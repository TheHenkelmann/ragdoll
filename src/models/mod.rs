// SPDX-License-Identifier: AGPL-3.0-only

pub mod bootstrap;
pub mod embed;
pub mod mock;
pub mod registry;
pub mod rerank;
pub mod traits;

pub use bootstrap::{ensure_models, list_supported_models};
pub use embed::EmbedModel;
pub use mock::{MockEmbedder, MockModelProvider};
pub use registry::ModelRegistry;
pub use rerank::RerankModel;
pub use traits::{Embedder, ModelProvider, Reranker};
