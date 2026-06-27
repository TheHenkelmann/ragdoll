// SPDX-License-Identifier: AGPL-3.0-only

/** Mirrors backend Permission::as_str() values. */
export const PERM = {
  sources: { read: "sources:read", write: "sources:write", delete: "sources:delete" },
  chunks: { read: "chunks:read", write: "chunks:write", delete: "chunks:delete" },
  queries: { run: "queries:run", read: "queries:read", delete: "queries:delete" },
  playground: { run: "playground:run", read: "playground:read" },
  db: { read: "db:read" },
  settings: { read: "settings:read", write: "settings:write" },
  llmModels: { read: "llm_models:read", write: "llm_models:write", delete: "llm_models:delete" },
  llmCredentials: {
    read: "llm_credentials:read",
    write: "llm_credentials:write",
    delete: "llm_credentials:delete",
  },
  analytics: { read: "analytics:read" },
  releases: { read: "releases:read", write: "releases:write", delete: "releases:delete" },
  stages: { read: "stages:read", write: "stages:write", delete: "stages:delete" },
  models: { read: "models:read", download: "models:download", delete: "models:delete" },
  backups: {
    read: "backups:read",
    create: "backups:create",
    upload: "backups:upload",
    download: "backups:download",
    restore: "backups:restore",
    delete: "backups:delete",
  },
  users: { read: "users:read", write: "users:write", delete: "users:delete" },
  apiKeys: { read: "api_keys:read", write: "api_keys:write", delete: "api_keys:delete" },
  webhooks: { read: "webhooks:read", write: "webhooks:write", delete: "webhooks:delete" },
} as const;
