// SPDX-License-Identifier: AGPL-3.0-only

pub mod endpoints;
pub mod genai_impl;
pub mod mock;
pub mod prompt;
pub mod resolver;
pub mod service;
pub mod types;
pub mod vertex;

pub use genai_impl::GenaiGenerator;
pub use mock::MockGenerator;
pub use resolver::resolve_generation_spec;
pub use service::{attach_generation, generate_answer, persist_generation_metrics};
pub use types::*;

use async_trait::async_trait;
use futures_util::stream::BoxStream;

#[async_trait]
pub trait Generator: Send + Sync {
    async fn generate(&self, spec: &ResolvedGenerationSpec) -> anyhow::Result<GenerationOutput>;

    async fn generate_stream(
        &self,
        spec: &ResolvedGenerationSpec,
    ) -> anyhow::Result<BoxStream<'static, anyhow::Result<StreamEvent>>>;
}

pub const MAX_GENERATION_CONCURRENCY: usize = 100;
