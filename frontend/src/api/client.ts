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
};

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
  latency: {
    upstream_ms: number | null;
    embed_ms: number;
    search_ms: number;
    rerank_ms: number | null;
    store_ms: number;
    total_ms: number;
    candidate_count: number;
    result_count: number;
  };
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
  source_count: number;
  chunk_count: number;
  chunks_per_source: SourceChunkCount[];
  metadata_keys: [string, number][];
  query_chunk_hits: QueryChunkHit[];
  query_chunk_metadata_keys: [string, number][];
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
  total_ms?: number;
};
