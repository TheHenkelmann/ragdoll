// SPDX-License-Identifier: AGPL-3.0-only

use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Instant;

use async_trait::async_trait;
use futures_util::stream::{BoxStream, Stream};
use genai::adapter::AdapterKind;
use genai::chat::{ChatMessage, ChatOptions, ChatRequest, ChatStream, ChatStreamEvent, StreamEnd};
use genai::resolver::{AuthData, Endpoint, ServiceTargetResolver};
use genai::{Client, Headers, ModelIden, ServiceTarget};

use crate::generation::endpoints::{
    normalize_azure_endpoint, normalize_openai_base_url, provider_default_endpoint,
    resolve_model_name,
};
use crate::generation::prompt::build_prompt;
use crate::generation::types::{GenerationOutput, ResolvedGenerationSpec, StreamEvent};
use crate::generation::vertex;
use crate::generation::Generator;

pub struct GenaiGenerator;

impl GenaiGenerator {
    pub fn new() -> Self {
        Self
    }

    fn azure_adapter_kind(endpoint: &str) -> AdapterKind {
        // Azure exposes two APIs:
        // - …/deployments/{name}/chat/completions → Chat Completions (`messages`)
        // - …/openai/responses → Responses API (`input`) — required for GPT-5+ on Azure
        if endpoint.contains("/responses") {
            AdapterKind::OpenAIResp
        } else {
            AdapterKind::OpenAI
        }
    }

    fn build_client(spec: &ResolvedGenerationSpec) -> anyhow::Result<Client> {
        let provider_adapter = parse_adapter(&spec.provider)?;
        let model_name =
            resolve_model_name(&spec.provider, &spec.model_name, spec.endpoint.as_deref())?;
        let endpoint = if spec.provider.eq_ignore_ascii_case("azure") {
            spec.endpoint
                .as_ref()
                .map(|url| normalize_azure_endpoint(url))
                .filter(|url| !url.is_empty())
        } else {
            spec.endpoint
                .as_ref()
                .map(|url| normalize_openai_base_url(url))
                .filter(|url| !url.is_empty())
        };
        let api_key = spec.api_key.clone();

        if spec.provider.eq_ignore_ascii_case("vertex") {
            return vertex::build_vertex_client(spec);
        }

        if spec.provider.eq_ignore_ascii_case("azure") {
            let url = endpoint.clone().ok_or_else(|| {
                anyhow::anyhow!(
                    "azure provider requires `endpoint` to be the full Azure URL. \
                     Chat Completions: …/openai/deployments/<deployment>/chat/completions?api-version=… \
                     Responses API (GPT-5+): …/openai/responses?api-version=…"
                )
            })?;
            if api_key.is_empty() {
                return Err(anyhow::anyhow!(
                    "azure provider requires an api key credential"
                ));
            }
            let azure_adapter = Self::azure_adapter_kind(&url);
            let target_resolver = ServiceTargetResolver::from_resolver_fn({
                let url = url.clone();
                let api_key = api_key.clone();
                let model_name = model_name.clone();
                move |_service_target: ServiceTarget| -> Result<ServiceTarget, genai::resolver::Error> {
                    let auth = AuthData::RequestOverride {
                        url: url.clone(),
                        headers: Headers::from(("api-key", api_key.clone())),
                    };
                    let model = ModelIden::new(azure_adapter, model_name.clone());
                    Ok(ServiceTarget {
                        endpoint: Endpoint::from_owned(url.clone()),
                        auth,
                        model,
                    })
                }
            });
            return Ok(Client::builder()
                .with_adapter_kind(azure_adapter)
                .with_service_target_resolver(target_resolver)
                .build());
        }

        let target_resolver = ServiceTargetResolver::from_resolver_fn({
            let provider_adapter = provider_adapter;
            let endpoint = endpoint.clone();
            let api_key = api_key.clone();
            let model_name = model_name.clone();
            move |service_target: ServiceTarget| -> Result<ServiceTarget, genai::resolver::Error> {
                let resolved_endpoint = endpoint
                    .as_ref()
                    .map(|url| Endpoint::from_owned(url.clone()))
                    .unwrap_or_else(|| provider_default_endpoint(provider_adapter));
                let auth = if api_key.is_empty() {
                    service_target.auth.clone()
                } else {
                    AuthData::from_single(api_key.clone())
                };
                let resolved_adapter = resolved_endpoint
                    .base_url()
                    .contains("/responses")
                    .then_some(AdapterKind::OpenAIResp)
                    .unwrap_or(provider_adapter);
                let model = ModelIden::new(resolved_adapter, model_name.clone());
                Ok(ServiceTarget {
                    endpoint: resolved_endpoint,
                    auth,
                    model,
                })
            }
        });

        Ok(Client::builder()
            .with_adapter_kind(provider_adapter)
            .with_service_target_resolver(target_resolver)
            .build())
    }

    fn chat_options(spec: &ResolvedGenerationSpec, capture_usage: bool) -> ChatOptions {
        ChatOptions::default()
            .with_capture_usage(capture_usage)
            .with_temperature(spec.temperature)
            .with_max_tokens(spec.max_tokens)
    }
}

#[async_trait]
impl Generator for GenaiGenerator {
    async fn generate(&self, spec: &ResolvedGenerationSpec) -> anyhow::Result<GenerationOutput> {
        let client = Self::build_client(spec)?;
        let model_name =
            resolve_model_name(&spec.provider, &spec.model_name, spec.endpoint.as_deref())?;
        let (system, user) = build_prompt(spec);
        let chat_req = ChatRequest::default()
            .with_system(system)
            .append_message(ChatMessage::user(user));
        let started = Instant::now();
        let chat_res = client
            .exec_chat(&model_name, chat_req, Some(&Self::chat_options(spec, true)))
            .await?;
        let elapsed = started.elapsed().as_millis() as i64;
        let text = chat_res.first_text().unwrap_or("").to_string();
        let usage = chat_res.usage;
        Ok(GenerationOutput {
            text,
            generation_ms: elapsed,
            generation_total_ms: elapsed,
            prompt_tokens: usage.prompt_tokens.map(|v| v as u32),
            completion_tokens: usage.completion_tokens.map(|v| v as u32),
        })
    }

    async fn generate_stream(
        &self,
        spec: &ResolvedGenerationSpec,
    ) -> anyhow::Result<BoxStream<'static, anyhow::Result<StreamEvent>>> {
        let client = Self::build_client(spec)?;
        let model_name =
            resolve_model_name(&spec.provider, &spec.model_name, spec.endpoint.as_deref())?;
        let (system, user) = build_prompt(spec);
        let chat_req = ChatRequest::default()
            .with_system(system)
            .append_message(ChatMessage::user(user));
        let chat_stream = client
            .exec_chat_stream(&model_name, chat_req, Some(&Self::chat_options(spec, true)))
            .await?;
        Ok(Box::pin(GenaiEventStream {
            inner: chat_stream.stream,
            started: Instant::now(),
            first_token_ms: None,
            finished: false,
        }))
    }
}

struct GenaiEventStream {
    inner: ChatStream,
    started: Instant,
    first_token_ms: Option<i64>,
    finished: bool,
}

impl Stream for GenaiEventStream {
    type Item = anyhow::Result<StreamEvent>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.finished {
            return Poll::Ready(None);
        }

        match Pin::new(&mut self.inner).poll_next(cx) {
            Poll::Ready(Some(Ok(event))) => match event {
                ChatStreamEvent::Chunk(chunk) => {
                    if self.first_token_ms.is_none() {
                        self.first_token_ms = Some(self.started.elapsed().as_millis() as i64);
                    }
                    Poll::Ready(Some(Ok(StreamEvent::Token {
                        delta: chunk.content,
                    })))
                }
                ChatStreamEvent::End(StreamEnd { captured_usage, .. }) => {
                    self.finished = true;
                    let total_ms = self.started.elapsed().as_millis() as i64;
                    let (prompt_tokens, completion_tokens) = captured_usage
                        .map(|u| {
                            (
                                u.prompt_tokens.map(|v| v as u32),
                                u.completion_tokens.map(|v| v as u32),
                            )
                        })
                        .unwrap_or((None, None));
                    Poll::Ready(Some(Ok(StreamEvent::Done {
                        generation_ms: self.first_token_ms.unwrap_or(total_ms),
                        generation_total_ms: total_ms,
                        prompt_tokens,
                        completion_tokens,
                    })))
                }
                ChatStreamEvent::Start
                | ChatStreamEvent::ReasoningChunk(_)
                | ChatStreamEvent::ThoughtSignatureChunk(_)
                | ChatStreamEvent::ToolCallChunk(_) => {
                    cx.waker().wake_by_ref();
                    self.poll_next(cx)
                }
            },
            Poll::Ready(Some(Err(err))) => Poll::Ready(Some(Err(err.into()))),
            Poll::Ready(None) => {
                self.finished = true;
                let total_ms = self.started.elapsed().as_millis() as i64;
                Poll::Ready(Some(Ok(StreamEvent::Done {
                    generation_ms: self.first_token_ms.unwrap_or(total_ms),
                    generation_total_ms: total_ms,
                    prompt_tokens: None,
                    completion_tokens: None,
                })))
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

fn parse_adapter(provider: &str) -> anyhow::Result<AdapterKind> {
    match provider.to_lowercase().as_str() {
        "openai" => Ok(AdapterKind::OpenAI),
        "openai_resp" | "openai-responses" => Ok(AdapterKind::OpenAIResp),
        "openai_compat" | "custom" => Ok(AdapterKind::OpenAI),
        "anthropic" => Ok(AdapterKind::Anthropic),
        "gemini" => Ok(AdapterKind::Gemini),
        "vertex" => Ok(AdapterKind::Vertex),
        "ollama" => Ok(AdapterKind::Ollama),
        "groq" => Ok(AdapterKind::Groq),
        "deepseek" => Ok(AdapterKind::DeepSeek),
        "xai" => Ok(AdapterKind::Xai),
        // Legacy provider strings kept for existing DB rows.
        "together" => Ok(AdapterKind::Together),
        "fireworks" => Ok(AdapterKind::Fireworks),
        "cohere" => Ok(AdapterKind::Cohere),
        "azure" => Ok(AdapterKind::OpenAI),
        other => Err(anyhow::anyhow!("unsupported llm provider: {other}")),
    }
}
