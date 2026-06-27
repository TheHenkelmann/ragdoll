// SPDX-License-Identifier: AGPL-3.0-only

use anyhow::{anyhow, Result};
use genai::adapter::AdapterKind;
use genai::resolver::Endpoint;

/// Default API base URLs per provider adapter (mirrors genai adapter defaults).
pub fn provider_default_endpoint(adapter: AdapterKind) -> Endpoint {
    match adapter {
        AdapterKind::OpenAI | AdapterKind::OpenAIResp => {
            Endpoint::from_static("https://api.openai.com/v1/")
        }
        AdapterKind::Anthropic => Endpoint::from_static("https://api.anthropic.com/v1/"),
        AdapterKind::Gemini => {
            Endpoint::from_static("https://generativelanguage.googleapis.com/v1beta/")
        }
        AdapterKind::Groq => Endpoint::from_static("https://api.groq.com/openai/v1/"),
        AdapterKind::DeepSeek => Endpoint::from_static("https://api.deepseek.com/"),
        AdapterKind::Xai => Endpoint::from_static("https://api.x.ai/v1/"),
        _ => Endpoint::from_static("https://api.openai.com/v1/"),
    }
}

/// Normalize an OpenAI-compatible base URL: strip accidental `/chat/completions`
/// or `/responses` path suffixes users paste from provider docs. Preserves query strings.
pub fn normalize_openai_base_url(raw: &str) -> String {
    let trimmed = raw.trim();
    let (path, query) = match trimmed.split_once('?') {
        Some((p, q)) => (p, Some(q)),
        None => (trimmed, None),
    };
    let mut url = path.trim_end_matches('/').to_string();
    for suffix in ["/chat/completions", "/responses", "/embeddings"] {
        if let Some(stripped) = url.strip_suffix(suffix) {
            url = stripped.trim_end_matches('/').to_string();
        }
    }
    if !url.ends_with('/') {
        url.push('/');
    }
    match query.filter(|q| !q.is_empty()) {
        Some(q) => format!("{url}?{q}"),
        None => url,
    }
}

/// Azure endpoints are full request URLs — never strip path segments or append slashes.
pub fn normalize_azure_endpoint(raw: &str) -> String {
    raw.trim().to_string()
}

/// Extract Azure deployment name from a Chat Completions deployment URL.
pub fn extract_azure_deployment(endpoint: &str) -> Option<String> {
    let path = endpoint.split('?').next().unwrap_or(endpoint);
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    for (i, part) in parts.iter().enumerate() {
        if part.eq_ignore_ascii_case("deployments") {
            if let Some(name) = parts.get(i + 1) {
                if !name.is_empty() {
                    return Some((*name).to_string());
                }
            }
        }
    }
    None
}

pub fn azure_endpoint_kind(endpoint: &str) -> AzureEndpointKind {
    let lower = endpoint.to_lowercase();
    if lower.contains("/responses") {
        AzureEndpointKind::Responses
    } else if lower.contains("/chat/completions") || lower.contains("/deployments/") {
        AzureEndpointKind::ChatCompletions
    } else {
        AzureEndpointKind::Unknown
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AzureEndpointKind {
    Responses,
    ChatCompletions,
    Unknown,
}

/// Resolve the model id sent to the provider. For Azure deployment URLs the
/// deployment name in the path is authoritative when `model_name` is empty.
pub fn resolve_model_name(
    provider: &str,
    model_name: &str,
    endpoint: Option<&str>,
) -> Result<String> {
    let provider = provider.trim().to_lowercase();
    let trimmed = model_name.trim();
    if provider == "azure" {
        if let Some(url) = endpoint.filter(|s| !s.trim().is_empty()) {
            if let Some(deployment) = extract_azure_deployment(url) {
                if trimmed.is_empty() {
                    return Ok(deployment);
                }
            }
            if trimmed.is_empty() && azure_endpoint_kind(url) == AzureEndpointKind::Responses {
                return Err(anyhow!(
                    "azure Responses API URLs require a model / deployment name"
                ));
            }
        }
    }
    if trimmed.is_empty() {
        return Err(anyhow!("model_name is required"));
    }
    Ok(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_chat_completions_suffix() {
        let url = normalize_openai_base_url("https://api.groq.com/openai/v1/chat/completions");
        assert_eq!(url, "https://api.groq.com/openai/v1/");
    }

    #[test]
    fn extracts_azure_deployment() {
        let url = "https://x.openai.azure.com/openai/deployments/gpt-5.4-mini/chat/completions?api-version=2025-04-01";
        assert_eq!(
            extract_azure_deployment(url).as_deref(),
            Some("gpt-5.4-mini")
        );
    }

    #[test]
    fn resolve_azure_model_from_deployment_url() {
        let name = resolve_model_name(
            "azure",
            "",
            Some("https://x.openai.azure.com/openai/deployments/my-deploy/chat/completions?api-version=2025-04-01"),
        )
        .unwrap();
        assert_eq!(name, "my-deploy");
    }

    #[test]
    fn preserves_query_string_when_normalizing() {
        let url = normalize_openai_base_url(
            "https://api.example.com/v1/chat/completions?api-version=2025-04-01",
        );
        assert_eq!(url, "https://api.example.com/v1/?api-version=2025-04-01");
    }

    #[test]
    fn azure_endpoint_not_mangled() {
        let raw =
            "https://x.cognitiveservices.azure.com/openai/responses?api-version=2025-04-01-preview";
        assert_eq!(normalize_azure_endpoint(raw), raw);
    }
}
