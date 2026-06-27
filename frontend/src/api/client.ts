// SPDX-License-Identifier: AGPL-3.0-only

const TOKEN_KEY = "ragdoll_token";

export const API_PREFIX = "/api/v1";

function toApiPath(path: string): string {
  if (path.startsWith(API_PREFIX)) return path;
  return `${API_PREFIX}${path.startsWith("/") ? path : `/${path}`}`;
}

export function getToken(): string | null {
  return localStorage.getItem(TOKEN_KEY);
}

export function setToken(token: string | null) {
  if (token) localStorage.setItem(TOKEN_KEY, token);
  else localStorage.removeItem(TOKEN_KEY);
}

function loginRedirectPath() {
  const returnTo = window.location.pathname + window.location.search;
  if (returnTo === "/login" || returnTo.startsWith("/login?")) {
    return "/login";
  }
  return `/login?redirect=${encodeURIComponent(returnTo)}`;
}

/** Public endpoints — never redirects to login on 401. */
export async function publicApi<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(toApiPath(path), {
    ...init,
    headers: {
      "Content-Type": "application/json",
      ...(init?.headers as Record<string, string> | undefined),
    },
  });
  if (!response.ok) {
    throw new Error(`${response.status} ${await response.text()}`);
  }
  if (response.status === 204) return undefined as T;
  return response.json() as Promise<T>;
}

export async function api<T>(path: string, init?: RequestInit): Promise<T> {
  const token = getToken();
  const headers: Record<string, string> = {
    "Content-Type": "application/json",
    ...(init?.headers as Record<string, string> | undefined),
  };
  if (token) headers.Authorization = `Bearer ${token}`;

  const response = await fetch(toApiPath(path), { ...init, headers });
  if (response.status === 401) {
    setToken(null);
    window.location.assign(loginRedirectPath());
    throw new Error("unauthorized");
  }
  if (!response.ok) {
    throw new Error(`${response.status} ${await response.text()}`);
  }
  if (response.status === 204) return undefined as T;
  return response.json() as Promise<T>;
}

export type AuthInfo = {
  default_admin_email: string;
  password_is_default: boolean;
};

export type AuthStatus = {
  email: string;
  is_superadmin: boolean;
  password_is_default: boolean;
  permissions: string[];
};

export type ApiKeyRecord = {
  id: string;
  name: string;
  permissions: string[];
  rpm?: number | null;
  rph?: number | null;
  created_at: string;
};

// Mirrors the backend whitelist in src/models/mapping.rs. All models are 1024-dim.
export const FORCED_PERMISSION = "releases:read";

export function ensureForcedPermissions(permissions: string[]): string[] {
  const set = new Set(permissions);
  set.add(FORCED_PERMISSION);
  return [...set].sort();
}

export function optionalPermissionCount(permissions: string[]): number {
  return permissions.filter((p) => p !== FORCED_PERMISSION).length;
}

export const SUPPORTED_EMBED_MODELS: string[] = [
  "BAAI/bge-m3",
  "BAAI/bge-large-en-v1.5",
  "mixedbread-ai/mxbai-embed-large-v1",
  "intfloat/multilingual-e5-large",
  "Snowflake/snowflake-arctic-embed-l-v2.0",
  "mixedbread-ai/deepset-mxbai-embed-de-large-v1",
  "jinaai/jina-embeddings-v3",
  "intfloat/multilingual-e5-large-instruct",
  "Alibaba-NLP/gte-large-en-v1.5",
];

export const SUPPORTED_RERANK_MODELS: string[] = [
  "BAAI/bge-reranker-v2-m3",
  "jinaai/jina-reranker-v2-base-multilingual",
  "mixedbread-ai/mxbai-rerank-base-v1",
];

export const PERMISSION_CATALOG: { section: string; permissions: string[] }[] = [
  {
    section: "Sources",
    permissions: ["sources:read", "sources:write", "sources:delete"],
  },
  {
    section: "Chunks",
    permissions: ["chunks:read", "chunks:write", "chunks:delete"],
  },
  {
    section: "Queries",
    permissions: ["queries:run", "queries:read", "queries:delete"],
  },
  {
    section: "Playground",
    permissions: ["playground:run", "playground:read"],
  },
  { section: "Database", permissions: ["db:read"] },
  {
    section: "Settings",
    permissions: ["settings:read", "settings:write"],
  },
  {
    section: "LLM Models",
    permissions: ["llm_models:read", "llm_models:write", "llm_models:delete"],
  },
  {
    section: "LLM Credentials",
    permissions: [
      "llm_credentials:read",
      "llm_credentials:write",
      "llm_credentials:delete",
    ],
  },
  { section: "Analytics", permissions: ["analytics:read"] },
  {
    section: "Releases",
    permissions: ["releases:read", "releases:write", "releases:delete"],
  },
  {
    section: "Stages",
    permissions: ["stages:read", "stages:write", "stages:delete"],
  },
  {
    section: "Models",
    permissions: ["models:read", "models:download", "models:delete"],
  },
  {
    section: "Backups",
    permissions: [
      "backups:read",
      "backups:create",
      "backups:upload",
      "backups:download",
      "backups:restore",
      "backups:delete",
    ],
  },
  {
    section: "Users",
    permissions: ["users:read", "users:write", "users:delete"],
  },
  {
    section: "API Keys",
    permissions: ["api_keys:read", "api_keys:write", "api_keys:delete"],
  },
  {
    section: "Webhooks",
    permissions: ["webhooks:read", "webhooks:write", "webhooks:delete"],
  },
];

export const WEBHOOK_EVENT_CATALOG: {
  section: string;
  events: { id: string; label: string }[];
}[] = [
  {
    section: "Ingest status",
    events: [
      { id: "completed", label: "Completed" },
      { id: "failed", label: "Failed" },
    ],
  },
  {
    section: "Resource utilization",
    events: [
      { id: "cpu_high", label: "CPU high (>85% for 15s)" },
      { id: "cpu_critical", label: "CPU critical (>95% for 15s)" },
      { id: "memory_high", label: "Memory high (>85% for 15s)" },
      { id: "memory_critical", label: "Memory critical (>95% for 15s)" },
      { id: "cpu_recovered", label: "CPU recovered (<75% for 60s, after alert)" },
      { id: "memory_recovered", label: "Memory recovered (<75% for 60s, after alert)" },
    ],
  },
];

export const WEBHOOK_EVENT_LABELS: Record<string, string> = Object.fromEntries(
  WEBHOOK_EVENT_CATALOG.flatMap((group) => group.events.map((e) => [e.id, e.label])),
);

export function webhookEventSections(events: string[]): string[] {
  return WEBHOOK_EVENT_CATALOG.filter((group) =>
    group.events.some((e) => events.includes(e.id)),
  ).map((group) => group.section);
}

export type CreateApiKeyResponse = ApiKeyRecord & {
  token: string;
};

export type UserRecord = {
  id: string;
  email: string;
  is_superadmin: boolean;
  permissions: string[];
  created_at: string;
};

export type WebhookRecord = {
  id: string;
  release_id: string;
  type: string;
  url: string;
  events: string[];
  active: boolean;
  created_at: string;
};

export type WebhookTestResult = {
  status_code: number | null;
  body: string;
};

export type WebhookSecretResponse = {
  secret: string;
};

export function testWebhook(releaseTag: string, id: string) {
  return api<WebhookTestResult>(
    `/releases/${encodeURIComponent(releaseTag)}/webhooks/${encodeURIComponent(id)}/test`,
    { method: "POST" },
  );
}

export function getWebhookSecret(releaseTag: string, id: string) {
  return api<WebhookSecretResponse>(
    `/releases/${encodeURIComponent(releaseTag)}/webhooks/${encodeURIComponent(id)}/secret`,
  );
}

export type ReleaseRecord = {
  id: string;
  tag: string;
  message: string;
  created_at: string;
  stage_tags: string[];
};

export type StageRecord = {
  id: string;
  tag: string;
  release_id: string;
  release_tag: string;
  created_at: string;
};

export type RuntimeSettings = {
  embedding_model: string;
  rerank_model: string;
  payload_storage: "per_request" | "forced" | "forbidden";
  chunking_strategy: string;
  sentence_buffer: number;
  breakpoint_percentile: number;
  min_chunk_tokens: number;
  max_chunk_tokens: number;
  max_upload_size: number;
  max_batch_size: number;
  generation_allowed: boolean;
  rerank_max_length: number;
};

export type ModelInfo = {
  name: string;
  kind: string;
  present: boolean;
};

export type ModelsResponse = {
  embedding_dim: number;
  models: ModelInfo[];
};

export type EmbeddingMismatch = {
  release_id: string;
  release_tag: string;
  settings_model: string;
  chunks_model: string | null;
  message: string;
};

export type RequiredModelInfo = {
  name: string;
  kind: string;
  present: boolean;
  releases: string[];
};

export type CatalogStatusEntry = {
  name: string;
  kind: string;
  languages: string[];
  present: boolean;
  releases: string[];
  loaded: boolean;
  ram_bytes: number | null;
  custom: boolean;
};

export type ModelsStatusResponse = {
  embedding_dim: number;
  local: ModelInfo[];
  catalog: CatalogStatusEntry[];
  required: RequiredModelInfo[];
  missing: string[];
  mismatches: EmbeddingMismatch[];
  active_downloads: string[];
};

export function getModelsStatus() {
  return api<ModelsStatusResponse>("/models/status");
}

export function addCustomModel(name: string) {
  return api<{ added: boolean; name: string }>("/models/custom", {
    method: "POST",
    body: JSON.stringify({ name }),
  });
}

export function removeCustomModel(name: string) {
  return api<{ removed: boolean; name: string }>(
    `/models/custom/${encodeURIComponent(name)}`,
    { method: "DELETE" },
  );
}

export function deleteModel(name: string) {
  return api<{ deleted: boolean; name: string }>(`/models/${encodeURIComponent(name)}`, {
    method: "DELETE",
  });
}

export type IngestJobsSummary = {
  total: number;
  pending: number;
  processing: number;
  completed: number;
  failed: number;
  active: number;
};

export type IngestJobRecord = {
  id: string;
  source_id: string;
  source_name: string | null;
  status: string;
  error: string | null;
  created_at: string;
  finished_at: string | null;
};

export type IngestJobsStatusResponse = {
  summary: IngestJobsSummary;
  jobs?: IngestJobRecord[];
};

export type ReindexResult = {
  source_id: string;
  job_id: string;
};

export type BatchItemResult<T> = {
  index: number;
  status: number;
  result?: T;
  error?: { detail: string };
};

export type BatchResponse<T> = {
  items: BatchItemResult<T>[];
};

export type ReindexBatchResponse = BatchResponse<ReindexResult> & {
  batch_id: string;
};

export type ReindexBatchEvent = {
  batch_id: string;
  summary: IngestJobsSummary & { batch_id: string };
  jobs: IngestJobRecord[];
};

export function getModels() {
  return api<ModelsResponse>("/models");
}

export function downloadModel(name: string) {
  return api<{ downloaded: boolean; name: string }>(
    `/models/${encodeURIComponent(name)}/download`,
    { method: "POST" },
  );
}

export type ModelDownloadEvent =
  | { event: "started"; name: string }
  | { event: "progress"; name: string; bytes: number; total?: number | null; message: string }
  | { event: "materializing"; name: string }
  | { event: "testing"; name: string }
  | { event: "complete"; name: string; latency_ms: number }
  | { event: "error"; name: string; message: string }
  | { event: "cancelled"; name: string }
  | { event: "cancellable"; name: string; cancellable: boolean };

export function cancelModelDownload(name: string) {
  return api<{ cancelled: boolean; name: string }>(
    `/models/${encodeURIComponent(name)}/download/cancel`,
    { method: "POST" },
  );
}

export type TestModelResponse = {
  ok: boolean;
  name: string;
  kind: string;
  latency_ms: number;
};

export async function streamModelDownload(
  name: string,
  onEvent: (event: ModelDownloadEvent) => void,
  signal?: AbortSignal,
): Promise<void> {
  const token = getToken();
  const response = await fetch(toApiPath(`/models/${encodeURIComponent(name)}/download/stream`), {
    headers: token ? { Authorization: `Bearer ${token}` } : {},
    signal,
  });
  if (response.status === 401) {
    setToken(null);
    window.location.assign(loginRedirectPath());
    throw new Error("unauthorized");
  }
  if (!response.ok) {
    throw new Error(`${response.status} ${await response.text()}`);
  }
  const reader = response.body?.getReader();
  if (!reader) throw new Error("no response body");

  const decoder = new TextDecoder();
  let buffer = "";
  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    buffer += decoder.decode(value, { stream: true });
    const parts = buffer.split("\n\n");
    buffer = parts.pop() ?? "";
    for (const part of parts) {
      for (const line of part.split("\n")) {
        if (!line.startsWith("data:")) continue;
        const payload = line.slice(5).trim();
        if (!payload || payload === "keep-alive") continue;
        onEvent(JSON.parse(payload) as ModelDownloadEvent);
      }
    }
  }
}

export function testModel(name: string) {
  return api<TestModelResponse>(`/models/${encodeURIComponent(name)}/test`, {
    method: "POST",
    body: JSON.stringify({}),
  });
}

export type PurgeModelsResponse = {
  purged_embedders: number;
  purged_rerankers: number;
};

export function purgeModelMemory(name: string) {
  return api<PurgeModelsResponse>(`/models/${encodeURIComponent(name)}/purge`, {
    method: "POST",
    body: JSON.stringify({}),
  });
}

export function purgeUnreferencedModels() {
  return api<PurgeModelsResponse>("/models/purge", {
    method: "POST",
    body: JSON.stringify({}),
  });
}

export function getIngestJobsStatus(releaseTag: string, details = false) {
  const qs = details ? "?details=true&limit=200" : "";
  return api<IngestJobsStatusResponse>(`/releases/${releaseTag}/ingest_jobs${qs}`);
}

export function triggerReindex(releaseTag: string) {
  return api<ReindexBatchResponse>(`/releases/${releaseTag}/reindex`, {
    method: "POST",
    body: JSON.stringify({}),
  });
}

export async function streamReindexBatch(
  releaseTag: string,
  batchId: string,
  onEvent: (event: ReindexBatchEvent) => void,
  signal?: AbortSignal,
): Promise<void> {
  const token = getToken();
  const response = await fetch(
    toApiPath(`/releases/${releaseTag}/reindex/${batchId}/events`),
    {
      headers: token ? { Authorization: `Bearer ${token}` } : {},
      signal,
    },
  );
  if (response.status === 401) {
    setToken(null);
    window.location.assign(loginRedirectPath());
    throw new Error("unauthorized");
  }
  if (!response.ok) {
    throw new Error(`${response.status} ${await response.text()}`);
  }
  const reader = response.body?.getReader();
  if (!reader) throw new Error("no response body");

  const decoder = new TextDecoder();
  let buffer = "";
  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    buffer += decoder.decode(value, { stream: true });
    const parts = buffer.split("\n\n");
    buffer = parts.pop() ?? "";
    for (const part of parts) {
      for (const line of part.split("\n")) {
        if (!line.startsWith("data:")) continue;
        const payload = line.slice(5).trim();
        if (!payload || payload === "keep-alive") continue;
        onEvent(JSON.parse(payload) as ReindexBatchEvent);
      }
    }
  }
}

export type LlmCredentialRecord = {
  id: string;
  name: string;
  provider: string;
  created_at: string;
  updated_at: string;
};

export type LlmModelRecord = {
  id: string;
  tag: string;
  model_name: string;
  provider: string;
  endpoint?: string | null;
  credential_id?: string | null;
  credential_name?: string | null;
  created_at: string;
  updated_at: string;
};

export type LlmModelTestResult = {
  ok: boolean;
  message: string;
  latency_ms?: number | null;
  completion_tokens?: number | null;
};

export function testLlmModel(releaseTag: string, tag: string) {
  return api<LlmModelTestResult>(
    `/releases/${encodeURIComponent(releaseTag)}/llm_models/${encodeURIComponent(tag)}/test`,
    {
      method: "POST",
    },
  );
}

export const DEFAULT_GENERATION_TEMPERATURE = 1;
export const DEFAULT_GENERATION_MAX_TOKENS = 5096;

export const DEFAULT_GENERATION_SYSTEM_PROMPT = `You are a retrieval-augmented assistant. Answer the user's question using **only** the sources provided in the user message.

- Do not use outside knowledge or training data.
- If the provided knowledge does not contain enough information, say clearly that you cannot answer.
- Cite the source of each piece of information using annotations like [1], [2].
- Add an index of all used sources with only the name of the source at the end of the answer.
- Respond in the **same language** as the question.
- Format your answer in **Markdown** (headings, lists, and emphasis where helpful) for readability.`;

export type GenerationRequest = {
  stream?: boolean;
  tag?: string;
  system_prompt: string;
  temperature?: number;
  max_tokens?: number;
};

export type GeneratedAnswer = {
  text: string;
  llm_model_id: string;
  llm_model_tag: string;
};

export type QueryUsage = {
  prompt_tokens?: number;
  completion_tokens?: number;
};

export type QueryMatch = {
  chunk_id: string;
  source_id: string;
  source_name: string;
  content: string;
  metadata: Record<string, unknown>;
  semantic_score: number;
  rerank_score?: number;
};

export type QueryResult = {
  query_id: string;
  matches: QueryMatch[];
  answer?: GeneratedAnswer;
  latency: {
    upstream_ms: number | null;
    embed_ms: number;
    search_ms: number;
    rerank_ms: number | null;
    store_ms: number;
    generation_ms?: number;
    generation_total_ms?: number;
    total_ragdoll_ms: number;
    candidate_count: number;
    result_count: number;
  };
  usage?: QueryUsage;
};

export type SourceRecord = {
  id: string;
  name: string;
  type: string;
  status: string;
  metadata: Record<string, unknown>;
  created_at: string;
  chunk_count: number;
};

export type ChunkRecord = {
  id: string;
  source_id: string;
  content: string;
  metadata: Record<string, unknown>;
};

export type SourceChunkCount = {
  source_id: string;
  name: string;
  chunk_count: number;
};

export type DailyRequestCount = {
  day: string;
  s2xx: number;
  s4xx: number;
  s5xx: number;
};

export type QueryChunkHit = {
  chunk_id: string;
  source_id: string;
  source_name: string;
  hit_count: number;
};

export type AnalyticsResponse = {
  request_count: number;
  daily_requests: DailyRequestCount[];
  total_latency: { p50: number; p95: number };
  embed_latency: { p50: number; p95: number };
  search_latency: { p50: number; p95: number };
  rerank_latency: { p50: number; p95: number };
  store_latency: { p50: number; p95: number };
  generation_latency: { p50: number; p95: number };
  source_count: number;
  chunk_count: number;
  chunks_per_source: SourceChunkCount[];
  metadata_keys: [string, number][];
  query_chunk_hits: QueryChunkHit[];
  query_chunk_metadata_keys: [string, number][];
};

export type SystemMetricSample = {
  recorded_at: string;
  cpu_percent: number;
  memory_used_bytes: number;
  memory_total_bytes: number;
};

export type SystemMetricsResponse = {
  samples: SystemMetricSample[];
  current: {
    cpu_percent: number;
    memory_used_bytes: number;
    memory_total_bytes: number;
    memory_available_bytes: number;
    cpu_cores: number;
  };
};

export type QueryDetail = {
  id: string;
  text?: string;
  params?: Record<string, unknown>;
  chunks: Array<{
    step: string;
    rank: number;
    chunk_id: string;
    source_id: string;
    source_name?: string;
    score: number;
    content?: string;
  }>;
  upstream_ms?: number;
  embed_ms?: number;
  search_ms?: number;
  rerank_ms?: number;
  store_ms?: number;
  generation_ms?: number;
  generation_total_ms?: number;
  total_ragdoll_ms?: number;
};

export type BackupRecord = {
  file_name: string;
  trigger: "manual" | "daily";
  created_at: string;
  size_bytes: number;
};

export type BackupRetention = {
  keep_daily: number;
  keep_manual: number;
};

export type BackupsListResponse = {
  backups: BackupRecord[];
  retention: BackupRetention;
};

export type RestoreBackupResponse = {
  restored_from: string;
  safety_backup?: string;
  restored_at: string;
};

export function listBackups() {
  return api<BackupsListResponse>("/backups");
}

export function createBackup() {
  return api<BackupRecord>("/backups", { method: "POST" });
}

export function restoreBackup(fileName: string, options?: { safetyBackup?: boolean }) {
  return api<RestoreBackupResponse>("/backups/restore", {
    method: "POST",
    body: JSON.stringify({
      file_name: fileName,
      safety_backup: options?.safetyBackup ?? false,
    }),
  });
}

export async function downloadBackup(fileName: string) {
  const token = getToken();
  const response = await fetch(
    `${API_PREFIX}/backups/download?file_name=${encodeURIComponent(fileName)}`,
    {
      headers: token ? { Authorization: `Bearer ${token}` } : {},
    },
  );
  if (response.status === 401) {
    setToken(null);
    window.location.assign("/login");
    throw new Error("unauthorized");
  }
  if (!response.ok) {
    throw new Error(`${response.status} ${await response.text()}`);
  }
  const blob = await response.blob();
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = fileName;
  anchor.click();
  URL.revokeObjectURL(url);
}

export async function uploadBackup(file: File) {
  const token = getToken();
  const form = new FormData();
  form.append("file", file);
  const response = await fetch(`${API_PREFIX}/backups/upload`, {
    method: "POST",
    headers: token ? { Authorization: `Bearer ${token}` } : {},
    body: form,
  });
  if (response.status === 401) {
    setToken(null);
    window.location.assign("/login");
    throw new Error("unauthorized");
  }
  if (!response.ok) {
    throw new Error(`${response.status} ${await response.text()}`);
  }
  return response.json() as Promise<BackupRecord>;
}

export function deleteBackup(fileName: string) {
  return api<{ deleted: boolean; file_name: string }>("/backups/delete", {
    method: "DELETE",
    body: JSON.stringify({ file_name: fileName }),
  });
}

export type StreamLatencySnapshot = {
  upstream_ms?: number;
  embed_ms?: number;
  search_ms?: number;
  rerank_ms?: number;
  store_ms?: number;
  generation_ms?: number;
  generation_total_ms?: number;
  total_ragdoll_ms?: number;
};

export async function runPlaygroundQueryStream(
  releaseTag: string,
  body: unknown[],
  onEvent: (event: string, data: string) => void,
): Promise<void> {
  const token = getToken();
  const response = await fetch(`${API_PREFIX}/playground/${releaseTag}/queries`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
    },
    body: JSON.stringify(body),
  });
  if (!response.ok) {
    throw new Error(`${response.status} ${await response.text()}`);
  }
  const reader = response.body?.getReader();
  if (!reader) throw new Error("no response body");
  const decoder = new TextDecoder();
  let buffer = "";
  let currentEvent = "message";

  const dispatchPart = (part: string) => {
    const lines = part.split("\n");
    let data = "";
    for (const line of lines) {
      if (line.startsWith("event:")) currentEvent = line.slice(6).trim();
      if (line.startsWith("data:")) data += line.slice(5).trim();
    }
    if (data) onEvent(currentEvent, data);
  };

  while (true) {
    const { done, value } = await reader.read();
    if (value) {
      buffer += decoder.decode(value, { stream: true });
      const parts = buffer.split("\n\n");
      buffer = parts.pop() ?? "";
      for (const part of parts) {
        if (part.trim()) dispatchPart(part);
      }
    }
    if (done) {
      buffer += decoder.decode();
      if (buffer.trim()) dispatchPart(buffer);
      break;
    }
  }
}
