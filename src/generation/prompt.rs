// SPDX-License-Identifier: AGPL-3.0-only

use crate::generation::types::{
    GenerationRequest, ResolvedGenerationSpec, DEFAULT_MAX_TOKENS, DEFAULT_TEMPERATURE,
};

pub fn build_prompt(spec: &ResolvedGenerationSpec) -> (String, String) {
    let mut context_parts = Vec::new();
    for (idx, m) in spec.matches.iter().enumerate() {
        context_parts.push(format!(
            "[source {}] (source_id={}, source_name={}, citation_id={})\n{}",
            idx + 1,
            m.source_id,
            m.source_name,
            m.citation.citation_id,
            m.content
        ));
    }

    let context = context_parts.join("\n\n---\n\n");
    let user = if context.is_empty() {
        spec.query_text.clone()
    } else {
        format!(
            "Use the following context to answer the question.\n\nContext:\n{context}\n\nQuestion: {}",
            spec.query_text
        )
    };

    (spec.system_prompt.clone(), user)
}

pub fn resolve_generation_params(
    request: &GenerationRequest,
) -> Result<(String, f64, u32), String> {
    let system_prompt = request
        .system_prompt
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "generation.system_prompt is required".to_string())?
        .to_string();

    let temperature = request.temperature.unwrap_or(DEFAULT_TEMPERATURE);
    let max_tokens = request.max_tokens.unwrap_or(DEFAULT_MAX_TOKENS);

    Ok((system_prompt, temperature, max_tokens))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_params_requires_system_prompt() {
        let req = GenerationRequest::default();
        assert!(resolve_generation_params(&req).is_err());
    }

    #[test]
    fn resolve_params_applies_defaults() {
        let req = GenerationRequest {
            system_prompt: Some("Answer briefly.".into()),
            ..Default::default()
        };
        let (_, temperature, max_tokens) = resolve_generation_params(&req).unwrap();
        assert_eq!(temperature, DEFAULT_TEMPERATURE);
        assert_eq!(max_tokens, DEFAULT_MAX_TOKENS);
    }
}
