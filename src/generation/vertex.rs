// SPDX-License-Identifier: AGPL-3.0-only

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use gcp_auth::{CustomServiceAccount, TokenProvider};
use genai::adapter::AdapterKind;
use genai::resolver::{AuthData, AuthResolver, Endpoint, ServiceTargetResolver};
use genai::{Client, ModelIden, ServiceTarget};

use crate::generation::types::ResolvedGenerationSpec;

const GCP_SCOPE: &str = "https://www.googleapis.com/auth/cloud-platform";

#[derive(Debug, Clone)]
pub struct VertexConfig {
    pub project_id: String,
    pub location: String,
}

pub fn parse_vertex_config(endpoint: Option<&str>) -> Result<VertexConfig> {
    let raw = endpoint
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| anyhow!("vertex provider requires endpoint JSON with project_id"))?;
    let value: serde_json::Value = serde_json::from_str(raw)
        .context("vertex endpoint must be JSON: {\"project_id\":\"…\",\"location\":\"…\"}")?;
    let project_id = value
        .get("project_id")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow!("vertex endpoint JSON requires project_id"))?
        .to_string();
    let location = value
        .get("location")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("global")
        .to_string();
    Ok(VertexConfig {
        project_id,
        location,
    })
}

pub fn build_vertex_base_url(project_id: &str, location: &str) -> String {
    if location == "global" {
        format!("https://aiplatform.googleapis.com/v1/projects/{project_id}/locations/global/")
    } else {
        format!("https://{location}-aiplatform.googleapis.com/v1/projects/{project_id}/locations/{location}/")
    }
}

pub fn validate_service_account_json(raw: &str) -> Result<()> {
    let value: serde_json::Value =
        serde_json::from_str(raw.trim()).context("service account JSON is not valid JSON")?;
    for key in ["type", "project_id", "private_key", "client_email"] {
        if value
            .get(key)
            .and_then(|v| v.as_str())
            .is_none_or(|s| s.is_empty())
        {
            return Err(anyhow!(
                "service account JSON missing required field '{key}'"
            ));
        }
    }
    Ok(())
}

pub fn build_vertex_client(spec: &ResolvedGenerationSpec) -> Result<Client> {
    if spec.api_key.trim().is_empty() {
        return Err(anyhow!(
            "vertex provider requires a service account credential"
        ));
    }
    validate_service_account_json(&spec.api_key)?;

    let vertex_cfg = parse_vertex_config(spec.endpoint.as_deref())?;
    let sa_json = Arc::new(spec.api_key.clone());
    let model_name = spec.model_name.clone();
    let base_url = build_vertex_base_url(&vertex_cfg.project_id, &vertex_cfg.location);

    let auth_resolver = AuthResolver::from_resolver_async_fn({
        let sa_json = sa_json.clone();
        move |_model: ModelIden| -> Pin<
            Box<
                dyn Future<Output = Result<Option<AuthData>, genai::resolver::Error>>
                    + Send
                    + 'static,
            >,
        > {
            let sa_json = sa_json.clone();
            Box::pin(async move {
                let account = CustomServiceAccount::from_json(sa_json.as_str()).map_err(|err| {
                    genai::resolver::Error::Custom(format!("invalid service account JSON: {err}"))
                })?;
                let token = account.token(&[GCP_SCOPE]).await.map_err(|err| {
                    genai::resolver::Error::Custom(format!(
                        "failed to obtain GCP access token: {err}"
                    ))
                })?;
                Ok(Some(AuthData::from_single(token.as_str())))
            })
        }
    });

    let target_resolver = ServiceTargetResolver::from_resolver_fn({
        let base_url = base_url.clone();
        let model_name = model_name.clone();
        move |service_target: ServiceTarget| -> Result<ServiceTarget, genai::resolver::Error> {
            Ok(ServiceTarget {
                endpoint: Endpoint::from_owned(base_url.clone()),
                auth: service_target.auth,
                model: ModelIden::new(AdapterKind::Vertex, model_name.clone()),
            })
        }
    });

    Ok(Client::builder()
        .with_adapter_kind(AdapterKind::Vertex)
        .with_auth_resolver(auth_resolver)
        .with_service_target_resolver(target_resolver)
        .build())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_vertex_config_requires_project_id() {
        assert!(parse_vertex_config(None).is_err());
        assert!(parse_vertex_config(Some(r#"{"location":"europe-west1"}"#)).is_err());
    }

    #[test]
    fn parse_vertex_config_defaults_location_to_global() {
        let cfg = parse_vertex_config(Some(r#"{"project_id":"my-proj"}"#)).unwrap();
        assert_eq!(cfg.project_id, "my-proj");
        assert_eq!(cfg.location, "global");
    }

    #[test]
    fn build_vertex_base_url_global() {
        let url = build_vertex_base_url("p1", "global");
        assert!(url.contains("locations/global/"));
    }
}
