// SPDX-License-Identifier: AGPL-3.0-only

use anyhow::Result;

use crate::crypto::Crypto;
use crate::db::DbPool;
use crate::generation::{
    resolve_generation_spec, GeneratedAnswer, GenerationOutput, GenerationRequest, Generator,
    QueryUsage, ResolvedGenerationSpec,
};
use crate::release::ReleaseCtx;
use crate::search::{QueryMatch, QueryResult};
use crate::settings::RuntimeSettings;

#[allow(clippy::too_many_arguments)]
pub async fn generate_answer(
    pool: &DbPool,
    crypto: &Crypto,
    generator: &dyn Generator,
    ctx: &ReleaseCtx,
    settings: &RuntimeSettings,
    generation: &GenerationRequest,
    query_text: &str,
    matches: Vec<QueryMatch>,
) -> Result<(GeneratedAnswer, GenerationOutput, ResolvedGenerationSpec)> {
    let spec =
        resolve_generation_spec(pool, crypto, ctx, settings, generation, query_text, matches)
            .await?;
    let output = generator.generate(&spec).await?;
    let answer = build_answer(&spec, &output);
    Ok((answer, output, spec))
}

pub fn build_answer(spec: &ResolvedGenerationSpec, output: &GenerationOutput) -> GeneratedAnswer {
    GeneratedAnswer {
        text: output.text.clone(),
        llm_model_id: spec.llm_model_id.clone(),
        llm_model_tag: spec.llm_model_tag.clone(),
    }
}

pub fn attach_generation(
    mut result: QueryResult,
    answer: GeneratedAnswer,
    output: &GenerationOutput,
) -> QueryResult {
    result.latency.generation_ms = Some(output.generation_ms);
    result.latency.generation_total_ms = Some(output.generation_total_ms);
    result.latency.total_ragdoll_ms += output.generation_total_ms;
    result.usage = Some(QueryUsage {
        prompt_tokens: output.prompt_tokens,
        completion_tokens: output.completion_tokens,
    });
    result.answer = Some(answer);
    result
}

pub async fn persist_generation_metrics(
    pool: &DbPool,
    query_id: &str,
    output: &GenerationOutput,
    llm_model_id: &str,
    total_ragdoll_ms: i64,
) -> Result<()> {
    let conn = pool.connect_one().await?;
    conn.execute(
        "UPDATE queries SET generation_ms = ?1, generation_total_ms = ?2,
                prompt_tokens = ?3, completion_tokens = ?4, llm_model_id = ?5, total_ragdoll_ms = ?6
         WHERE id = ?7",
        (
            output.generation_ms,
            output.generation_total_ms,
            output.prompt_tokens.map(i64::from),
            output.completion_tokens.map(i64::from),
            llm_model_id,
            total_ragdoll_ms,
            query_id,
        ),
    )
    .await?;
    Ok(())
}
