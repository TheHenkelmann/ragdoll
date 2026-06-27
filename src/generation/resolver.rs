// SPDX-License-Identifier: AGPL-3.0-only

use anyhow::{anyhow, Context, Result};

use crate::crypto::Crypto;
use crate::db::DbPool;
use crate::generation::prompt::resolve_generation_params;
use crate::generation::types::{GenerationRequest, ResolvedGenerationSpec};
use crate::release::ReleaseCtx;
use crate::search::QueryMatch;
use crate::settings::RuntimeSettings;

#[derive(Debug, Clone)]
struct LlmModelRow {
    id: String,
    tag: String,
    model_name: String,
    provider: String,
    endpoint: Option<String>,
    credential_id: Option<String>,
}

pub async fn resolve_generation_spec(
    pool: &DbPool,
    crypto: &Crypto,
    ctx: &ReleaseCtx,
    settings: &RuntimeSettings,
    request: &GenerationRequest,
    query_text: &str,
    matches: Vec<QueryMatch>,
) -> Result<ResolvedGenerationSpec> {
    if !settings.generation_allowed {
        return Err(anyhow!("generation is not allowed for this release"));
    }

    let tag = request
        .tag
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow!("generation.tag is required"))?;

    let (system_prompt, temperature, max_tokens) =
        resolve_generation_params(request).map_err(|e| anyhow!(e))?;

    let conn = pool.connect_one().await?;
    let model_row = load_model_by_tag(&conn, &ctx.release_id, tag).await?;

    let api_key = if let Some(cred_id) = &model_row.credential_id {
        load_credential_key(&conn, crypto, cred_id, &ctx.release_id).await?
    } else {
        String::new()
    };

    Ok(ResolvedGenerationSpec {
        llm_model_id: model_row.id,
        llm_model_tag: model_row.tag,
        model_name: model_row.model_name,
        provider: model_row.provider,
        endpoint: model_row.endpoint,
        api_key,
        system_prompt,
        temperature,
        max_tokens,
        query_text: query_text.to_string(),
        matches,
    })
}

async fn load_model_by_tag(
    conn: &libsql::Connection,
    release_id: &str,
    tag: &str,
) -> Result<LlmModelRow> {
    let mut rows = conn
        .query(
            "SELECT id, tag, model_name, provider, endpoint, credential_id
             FROM llm_models WHERE tag = ?1 AND release_id = ?2",
            (tag, release_id),
        )
        .await?;
    let row = rows
        .next()
        .await?
        .ok_or_else(|| anyhow!("llm model not found: {tag}"))?;
    read_model_row(row)
}

async fn load_credential_key(
    conn: &libsql::Connection,
    crypto: &Crypto,
    credential_id: &str,
    release_id: &str,
) -> Result<String> {
    let mut rows = conn
        .query(
            "SELECT nonce, ciphertext FROM llm_credentials WHERE id = ?1 AND release_id = ?2",
            (credential_id, release_id),
        )
        .await?;
    let row = rows
        .next()
        .await?
        .ok_or_else(|| anyhow!("llm credential not found"))?;
    let nonce: String = row.get(0)?;
    let ciphertext: String = row.get(1)?;
    crypto
        .decrypt(&nonce, &ciphertext)
        .context("decrypt llm credential")
}

fn read_model_row(row: libsql::Row) -> Result<LlmModelRow> {
    Ok(LlmModelRow {
        id: row.get(0)?,
        tag: row.get(1)?,
        model_name: row.get(2)?,
        provider: row.get(3)?,
        endpoint: row.get(4).ok(),
        credential_id: row.get(5).ok(),
    })
}
