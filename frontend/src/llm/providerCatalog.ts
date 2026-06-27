// SPDX-License-Identifier: AGPL-3.0-only

export type ProviderOption = { value: string; label: string; hint?: string };

export type ModelPreset = { label: string; id: string };

export const CUSTOM_MODEL_VALUE = "__custom__";

export const LLM_PROVIDERS: ProviderOption[] = [
  { value: "openai", label: "OpenAI" },
  {
    value: "openai_compat",
    label: "OpenAI-compatible",
    hint: "OpenRouter, vLLM, LM Studio, LiteLLM, … — set the API base URL.",
  },
  {
    value: "azure",
    label: "Azure OpenAI",
    hint: "Full deployment or Responses API URL. Deployment name is read from the URL when possible.",
  },
  { value: "anthropic", label: "Anthropic" },
  {
    value: "gemini",
    label: "Google Gemini (API key)",
    hint: "Google AI Studio API key — not Vertex AI.",
  },
  {
    value: "vertex",
    label: "Google Vertex AI",
    hint: "GCP service account JSON credential.",
  },
  { value: "groq", label: "Groq" },
  { value: "deepseek", label: "DeepSeek" },
  { value: "xai", label: "xAI (Grok)" },
];

/** Newest models first. `label` is shown in the UI; `id` is stored as model_name. */
export const MODEL_PRESETS: Record<string, ModelPreset[]> = {
  openai: [
    { label: "GPT-5.5", id: "gpt-5.5" },
    { label: "GPT-5.4 mini", id: "gpt-5.4-mini" },
    { label: "GPT-5.4 nano", id: "gpt-5.4-nano" },
    { label: "GPT-4.1", id: "gpt-4.1" },
    { label: "GPT-4.1 mini", id: "gpt-4.1-mini" },
    { label: "o4-mini", id: "o4-mini" },
    { label: "o3", id: "o3" },
  ],
  openai_compat: [
    { label: "GPT-4.1 mini (example)", id: "gpt-4.1-mini" },
    { label: "Llama 3.3 70B (example)", id: "meta-llama/Llama-3.3-70B-Instruct" },
  ],
  azure: [
    { label: "GPT-5.4 mini", id: "gpt-5.4-mini" },
    { label: "GPT-5.5", id: "gpt-5.5" },
    { label: "GPT-4.1", id: "gpt-4.1" },
    { label: "GPT-4.1 mini", id: "gpt-4.1-mini" },
    { label: "o4-mini", id: "o4-mini" },
  ],
  anthropic: [
    { label: "Claude Opus 4.8", id: "claude-opus-4-8" },
    { label: "Claude Opus 4.7", id: "claude-opus-4-7" },
    { label: "Claude Sonnet 4.6", id: "claude-sonnet-4-6" },
    { label: "Claude Opus 4.6", id: "claude-opus-4-6" },
    { label: "Claude Haiku 4.5", id: "claude-haiku-4-5-20251001" },
    { label: "Claude Sonnet 4.5", id: "claude-sonnet-4-5-20250929" },
    { label: "Claude Opus 4.5", id: "claude-opus-4-5-20251101" },
  ],
  gemini: [
    { label: "Gemini 3.5 Flash", id: "gemini-3.5-flash" },
    { label: "Gemini 3.1 Pro", id: "gemini-3.1-pro-preview" },
    { label: "Gemini 3 Flash", id: "gemini-3-flash-preview" },
    { label: "Gemini 3.1 Flash-Lite", id: "gemini-3.1-flash-lite" },
    { label: "Gemini 2.5 Pro", id: "gemini-2.5-pro" },
    { label: "Gemini 2.5 Flash", id: "gemini-2.5-flash" },
    { label: "Gemini 2.5 Flash-Lite", id: "gemini-2.5-flash-lite" },
  ],
  vertex: [
    { label: "Gemini 3.5 Flash", id: "gemini-3.5-flash" },
    { label: "Gemini 3.1 Pro", id: "gemini-3.1-pro-preview" },
    { label: "Gemini 3 Flash", id: "gemini-3-flash-preview" },
    { label: "Gemini 3.1 Flash-Lite", id: "gemini-3.1-flash-lite" },
    { label: "Gemini 2.5 Pro", id: "gemini-2.5-pro" },
    { label: "Gemini 2.5 Flash", id: "gemini-2.5-flash" },
    { label: "Gemini 2.5 Flash-Lite", id: "gemini-2.5-flash-lite" },
    { label: "Claude Opus 4.8", id: "claude-opus-4-8" },
    { label: "Claude Opus 4.7", id: "claude-opus-4-7" },
    { label: "Claude Sonnet 4.6", id: "claude-sonnet-4-6" },
    { label: "Claude Opus 4.6", id: "claude-opus-4-6" },
    { label: "Claude Haiku 4.5", id: "claude-haiku-4-5@20251001" },
    { label: "Claude Sonnet 4.5", id: "claude-sonnet-4-5@20250929" },
    { label: "Claude Opus 4.5", id: "claude-opus-4-5@20251101" },
  ],
  groq: [
    { label: "Qwen3.6 27B (preview)", id: "qwen/qwen3.6-27b" },
    { label: "Safety GPT OSS 20B (preview)", id: "openai/gpt-oss-safeguard-20b" },
    { label: "Llama 4 Scout 17B (preview)", id: "meta-llama/llama-4-scout-17b-16e-instruct" },
    { label: "Qwen3 32B (preview)", id: "qwen/qwen3-32b" },
    { label: "GPT OSS 120B", id: "openai/gpt-oss-120b" },
    { label: "GPT OSS 20B", id: "openai/gpt-oss-20b" },
    { label: "Compound", id: "groq/compound" },
    { label: "Compound Mini", id: "groq/compound-mini" },
    { label: "Llama 3.3 70B", id: "llama-3.3-70b-versatile" },
    { label: "Llama 3.1 8B", id: "llama-3.1-8b-instant" },
    { label: "Prompt Guard 2 86M (preview)", id: "meta-llama/llama-prompt-guard-2-86m" },
    { label: "Prompt Guard 2 22M (preview)", id: "meta-llama/llama-prompt-guard-2-22m" },
  ],
  deepseek: [
    { label: "DeepSeek V4 Pro", id: "deepseek-v4-pro" },
    { label: "DeepSeek V4 Flash", id: "deepseek-v4-flash" },
  ],
  xai: [
    { label: "Grok 4.3", id: "grok-4.3" },
    { label: "Grok 4.3 (latest alias)", id: "grok-4.3-latest" },
    { label: "Grok 4.20 reasoning", id: "grok-4.20-0309-reasoning" },
    { label: "Grok 4.20 non-reasoning", id: "grok-4.20-0309-non-reasoning" },
    { label: "Grok 4.20 multi-agent", id: "grok-4.20-multi-agent-0309" },
  ],
};

export function providerLabel(value: string): string {
  return LLM_PROVIDERS.find((p) => p.value === value)?.label ?? value;
}

export function presetsForProvider(provider: string): ModelPreset[] {
  return MODEL_PRESETS[provider] ?? [];
}

export function presetSelectValue(modelName: string, provider: string): string {
  const presets = presetsForProvider(provider);
  if (presets.some((p) => p.id === modelName)) {
    return modelName;
  }
  return modelName.trim() ? CUSTOM_MODEL_VALUE : "";
}

export function azureEndpointKind(url: string): "responses" | "chat" | "unknown" {
  const lower = url.toLowerCase();
  if (lower.includes("/responses")) return "responses";
  if (lower.includes("/chat/completions") || lower.includes("/deployments/")) return "chat";
  return "unknown";
}

export function extractAzureDeployment(url: string): string | null {
  const path = url.split("?")[0] ?? url;
  const match = path.match(/\/deployments\/([^/]+)/i);
  return match?.[1] ?? null;
}

export function modelDisplayName(provider: string, modelId: string): string {
  const preset = presetsForProvider(provider).find((p) => p.id === modelId);
  return preset ? preset.label : modelId;
}

export function normalizeOpenAiBaseUrl(raw: string): string {
  let url = raw.trim().replace(/\/+$/, "");
  for (const suffix of ["/chat/completions", "/responses", "/embeddings"]) {
    if (url.endsWith(suffix)) {
      url = url.slice(0, -suffix.length).replace(/\/+$/, "");
    }
  }
  return url.endsWith("/") ? url : `${url}/`;
}
