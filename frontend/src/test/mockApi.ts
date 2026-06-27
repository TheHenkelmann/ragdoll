// SPDX-License-Identifier: AGPL-3.0-only

import { vi } from "vitest";
import type {
  AnalyticsResponse,
  AuthInfo,
  AuthStatus,
  ChunkRecord,
  QueryDetail,
  QueryResult,
  ReleaseRecord,
  RuntimeSettings,
  SourceRecord,
  StageRecord,
  SystemMetricsResponse,
} from "../api/client";

export function jsonResponse(data: unknown, status = 200): Response {
  if (status === 204) return new Response(null, { status: 204 });
  return new Response(JSON.stringify(data), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

export type MockRoute = {
  path: string | RegExp;
  method?: string;
  response?: unknown | ((url: string, init?: RequestInit) => unknown | Response);
  status?: number;
};

export const mockAuthStatus: AuthStatus = {
  email: "admin@ragdoll.ai",
  is_superadmin: true,
  password_is_default: false,
  permissions: [],
};

export const mockAuthInfo: AuthInfo = {
  default_admin_email: "admin@ragdoll.ai",
  password_is_default: false,
};

export const mockRelease: ReleaseRecord = {
  id: "rel-1",
  tag: "v1",
  message: "Initial release",
  created_at: "2024-01-01T00:00:00Z",
  stage_tags: ["dev"],
};

export const mockStage: StageRecord = {
  id: "stg-1",
  tag: "dev",
  release_id: "rel-1",
  release_tag: "v1",
  created_at: "2024-01-01T00:00:00Z",
};

export const mockSettings: RuntimeSettings = {
  embedding_model: "embed-model",
  rerank_model: "rerank-model",
  payload_storage: "per_request",
  chunking_strategy: "semantic_split",
  sentence_buffer: 1,
  breakpoint_percentile: 95,
  min_chunk_tokens: 64,
  max_chunk_tokens: 512,
  max_upload_size: 10485760,
  max_batch_size: 100,
  generation_allowed: true,
  rerank_max_length: 256,
};

export const mockAnalytics: AnalyticsResponse = {
  request_count: 42,
  daily_requests: [{ day: "2024-06-01", s2xx: 10, s4xx: 1, s5xx: 0 }],
  total_latency: { p50: 100, p95: 200 },
  embed_latency: { p50: 20, p95: 40 },
  search_latency: { p50: 30, p95: 60 },
  rerank_latency: { p50: 40, p95: 80 },
  store_latency: { p50: 10, p95: 20 },
  generation_latency: { p50: 50, p95: 120 },
  source_count: 2,
  chunk_count: 5,
  chunks_per_source: [{ source_id: "src-1", name: "Doc A", chunk_count: 3 }],
  metadata_keys: [["department", 2]],
  query_chunk_hits: [{ chunk_id: "chk-1", source_id: "src-1", source_name: "Doc A", hit_count: 5 }],
  query_chunk_metadata_keys: [["topic", 3]],
};

export const mockSystemMetrics: SystemMetricsResponse = {
  samples: [
    {
      recorded_at: "2024-06-01T12:00:00Z",
      cpu_percent: 25,
      memory_used_bytes: 8_000_000_000,
      memory_total_bytes: 16_000_000_000,
    },
  ],
  current: {
    cpu_percent: 30,
    memory_used_bytes: 8_000_000_000,
    memory_total_bytes: 16_000_000_000,
    memory_available_bytes: 8_000_000_000,
    cpu_cores: 8,
  },
};

export const mockSource: SourceRecord = {
  id: "src-1",
  name: "Doc A",
  type: "file",
  status: "ready",
  metadata: {},
  created_at: "2024-01-01T00:00:00Z",
  chunk_count: 2,
};

export const mockChunk: ChunkRecord = {
  id: "chk-1",
  source_id: "src-1",
  content: "Sample chunk content",
  metadata: {},
};

export const mockQueryResult: QueryResult = {
  query_id: "q-1",
  matches: [],
  latency: {
    upstream_ms: 1,
    embed_ms: 10,
    search_ms: 20,
    rerank_ms: 5,
    store_ms: 3,
    total_ragdoll_ms: 39,
    candidate_count: 10,
    result_count: 3,
  },
};

export const mockQueryDetail: QueryDetail = {
  id: "q-1",
  text: "test query",
  chunks: [
    {
      step: "semantic",
      rank: 1,
      chunk_id: "chk-1",
      source_id: "src-1",
      source_name: "Doc A",
      score: 0.85,
      content: "Sample chunk content",
    },
    {
      step: "rerank",
      rank: 1,
      chunk_id: "chk-1",
      source_id: "src-1",
      source_name: "Doc A",
      score: 0.92,
      content: "Sample chunk content",
    },
  ],
  embed_ms: 10,
  search_ms: 20,
  rerank_ms: 5,
  store_ms: 3,
  total_ragdoll_ms: 39,
};

export function authRoutes(overrides: Partial<AuthStatus> = {}): MockRoute[] {
  const status = { ...mockAuthStatus, ...overrides };
  return [
    { path: "/auth/status", response: status },
    { path: "/auth/info", response: mockAuthInfo },
    {
      path: "/auth/login",
      method: "POST",
      response: { token: "test-token" },
    },
  ];
}

export function metaRoutes(): MockRoute[] {
  return [
    { path: "/releases", response: [mockRelease] },
    { path: "/stages", response: [mockStage] },
  ];
}

export function setupMockFetch(routes: MockRoute[]) {
  const sorted = [...routes].sort((a, b) => {
    const lenA = typeof a.path === "string" ? a.path.length : 0;
    const lenB = typeof b.path === "string" ? b.path.length : 0;
    return lenB - lenA;
  });

  return vi.spyOn(globalThis, "fetch").mockImplementation(async (input, init) => {
    const url = String(input);
    const method = init?.method ?? "GET";
    for (const route of sorted) {
      const pathMatch =
        typeof route.path === "string" ? url.includes(route.path) : route.path.test(url);
      const methodMatch = route.method ? route.method === method : method === "GET";
      if (pathMatch && methodMatch) {
        const payload =
          typeof route.response === "function" ? route.response(url, init) : route.response;
        if (payload instanceof Response) return payload;
        return jsonResponse(payload, route.status);
      }
    }
    throw new Error(`Unmocked fetch: ${method} ${url}`);
  });
}

export function setupDefaultMocks(extra: MockRoute[] = []) {
  return setupMockFetch([...authRoutes(), ...metaRoutes(), ...extra]);
}
