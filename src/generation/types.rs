// SPDX-License-Identifier: AGPL-3.0-only

use serde::{Deserialize, Serialize};

use crate::search::QueryMatch;

pub const DEFAULT_TEMPERATURE: f64 = 1.0;
pub const DEFAULT_MAX_TOKENS: u32 = 5096;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GenerationRequest {
    #[serde(default)]
    pub stream: bool,
    pub tag: Option<String>,
    pub system_prompt: Option<String>,
    pub temperature: Option<f64>,
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedAnswer {
    pub text: String,
    pub llm_model_id: String,
    pub llm_model_tag: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryUsage {
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct ResolvedGenerationSpec {
    pub llm_model_id: String,
    pub llm_model_tag: String,
    pub model_name: String,
    pub provider: String,
    pub endpoint: Option<String>,
    pub api_key: String,
    pub system_prompt: String,
    pub temperature: f64,
    pub max_tokens: u32,
    pub query_text: String,
    pub matches: Vec<QueryMatch>,
}

#[derive(Debug, Clone)]
pub struct GenerationOutput {
    pub text: String,
    pub generation_ms: i64,
    pub generation_total_ms: i64,
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEvent {
    Token {
        delta: String,
    },
    Done {
        generation_ms: i64,
        generation_total_ms: i64,
        prompt_tokens: Option<u32>,
        completion_tokens: Option<u32>,
    },
}
