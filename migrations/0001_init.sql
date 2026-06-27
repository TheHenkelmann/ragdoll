-- SPDX-License-Identifier: AGPL-3.0-only
-- Ragdoll v2 initial schema

CREATE TABLE IF NOT EXISTS meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS releases (
    id          TEXT PRIMARY KEY,
    tag         TEXT NOT NULL UNIQUE CHECK (length(tag) <= 50),
    message     TEXT NOT NULL DEFAULT '',
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS stages (
    id          TEXT PRIMARY KEY,
    tag         TEXT NOT NULL UNIQUE CHECK (length(tag) <= 12),
    release_id  TEXT REFERENCES releases(id),
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_stages_release ON stages(release_id);

CREATE TABLE IF NOT EXISTS users (
    id                  TEXT PRIMARY KEY,
    email               TEXT NOT NULL UNIQUE,
    password_hash       TEXT NOT NULL,
    is_superadmin       INTEGER NOT NULL DEFAULT 0,
    password_is_default INTEGER NOT NULL DEFAULT 0,
    permissions         TEXT NOT NULL DEFAULT '[]',
    created_at          TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS api_keys (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL UNIQUE,
    permissions TEXT NOT NULL DEFAULT '[]',
    rpm         INTEGER,
    rph         INTEGER,
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS llm_credentials (
    id          TEXT PRIMARY KEY,
    release_id  TEXT NOT NULL REFERENCES releases(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    provider    TEXT NOT NULL,
    nonce       TEXT NOT NULL,
    ciphertext  TEXT NOT NULL,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE (release_id, name)
);

CREATE INDEX IF NOT EXISTS idx_llm_credentials_release ON llm_credentials(release_id);

CREATE TABLE IF NOT EXISTS llm_models (
    id              TEXT PRIMARY KEY,
    release_id      TEXT NOT NULL REFERENCES releases(id) ON DELETE CASCADE,
    tag             TEXT NOT NULL CHECK (length(tag) <= 50),
    model_name      TEXT NOT NULL,
    provider        TEXT NOT NULL,
    endpoint        TEXT,
    credential_id   TEXT REFERENCES llm_credentials(id),
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE (release_id, tag)
);

CREATE INDEX IF NOT EXISTS idx_llm_models_release ON llm_models(release_id);

CREATE TABLE IF NOT EXISTS settings (
    release_id  TEXT NOT NULL REFERENCES releases(id) ON DELETE CASCADE,
    key         TEXT NOT NULL,
    value       TEXT NOT NULL,
    PRIMARY KEY (release_id, key)
);

CREATE TABLE IF NOT EXISTS models (
    name         TEXT PRIMARY KEY,
    kind         TEXT NOT NULL CHECK (kind IN ('embed', 'rerank', 'llm')),
    runtime      TEXT NOT NULL CHECK (runtime IN ('onnx', 'litellm')),
    dim          INTEGER,
    uri          TEXT,
    query_prefix TEXT NOT NULL DEFAULT '',
    doc_prefix   TEXT NOT NULL DEFAULT '',
    status       TEXT NOT NULL DEFAULT 'available',
    is_default   INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS sources (
    id            TEXT PRIMARY KEY,
    release_id    TEXT NOT NULL REFERENCES releases(id) ON DELETE CASCADE,
    name          TEXT NOT NULL,
    type          TEXT NOT NULL CHECK (type IN ('text', 'file', 'url')),
    uri           TEXT,
    content_hash  TEXT,
    config        TEXT NOT NULL DEFAULT '{}',
    metadata      TEXT NOT NULL DEFAULT '{}',
    status        TEXT NOT NULL DEFAULT 'pending'
                  CHECK (status IN ('pending', 'processing', 'completed', 'failed')),
    error         TEXT,
    page_map      TEXT NOT NULL DEFAULT '[]',
    created_at    TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_sources_release ON sources(release_id);
CREATE INDEX IF NOT EXISTS idx_sources_created ON sources(created_at);
CREATE INDEX IF NOT EXISTS idx_sources_status ON sources(status);
CREATE INDEX IF NOT EXISTS idx_sources_release_hash ON sources(release_id, content_hash);

CREATE TABLE IF NOT EXISTS source_texts (
    source_id   TEXT PRIMARY KEY REFERENCES sources(id) ON DELETE CASCADE,
    text        TEXT NOT NULL,
    char_len    INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS ingest_jobs (
    id            TEXT PRIMARY KEY,
    batch_id      TEXT,
    release_id    TEXT NOT NULL REFERENCES releases(id) ON DELETE CASCADE,
    stage_id      TEXT REFERENCES stages(id),
    source_id     TEXT NOT NULL,
    source_name   TEXT NOT NULL,
    source_type   TEXT NOT NULL CHECK (source_type IN ('text', 'file', 'url')),
    source_uri    TEXT,
    content_hash  TEXT,
    config        TEXT NOT NULL DEFAULT '{}',
    metadata      TEXT NOT NULL DEFAULT '{}',
    status        TEXT NOT NULL DEFAULT 'pending'
                  CHECK (status IN ('pending', 'processing', 'completed', 'failed')),
    attempts      INTEGER NOT NULL DEFAULT 0,
    max_attempts  INTEGER NOT NULL DEFAULT 3,
    worker_id     TEXT,
    heartbeat_at  TEXT,
    error         TEXT,
    created_at    TEXT NOT NULL DEFAULT (datetime('now')),
    started_at    TEXT,
    finished_at   TEXT,
    queue_ms      INTEGER,
    extract_ms    INTEGER,
    chunk_ms      INTEGER,
    embed_ms      INTEGER,
    db_write_ms   INTEGER,
    total_ms      INTEGER,
    chunk_count   INTEGER,
    char_count    INTEGER
);

CREATE INDEX IF NOT EXISTS idx_jobs_claim ON ingest_jobs(status, created_at);
CREATE INDEX IF NOT EXISTS idx_jobs_release ON ingest_jobs(release_id);
CREATE INDEX IF NOT EXISTS idx_jobs_batch ON ingest_jobs(release_id, batch_id);

CREATE TABLE IF NOT EXISTS chunks (
    id                TEXT PRIMARY KEY,
    release_id        TEXT NOT NULL REFERENCES releases(id) ON DELETE CASCADE,
    source_id         TEXT NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
    ordinal           INTEGER NOT NULL,
    content           TEXT NOT NULL,
    provenance        TEXT NOT NULL DEFAULT '[]',
    metadata          TEXT NOT NULL DEFAULT '{}',
    token_count       INTEGER,
    embedding         F32_BLOB(1024) NOT NULL,
    embedding_model   TEXT NOT NULL,
    embedding_dim     INTEGER NOT NULL,
    embedding_version TEXT NOT NULL,
    created_at        TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_chunks_release ON chunks(release_id);
CREATE INDEX IF NOT EXISTS idx_chunks_source ON chunks(source_id);
CREATE INDEX IF NOT EXISTS idx_chunks_created ON chunks(created_at);

CREATE TABLE IF NOT EXISTS queries (
    id               TEXT PRIMARY KEY,
    release_id       TEXT NOT NULL REFERENCES releases(id) ON DELETE CASCADE,
    stage_id         TEXT REFERENCES stages(id),
    text             TEXT,
    filters          TEXT NOT NULL DEFAULT '{}',
    params           TEXT NOT NULL DEFAULT '{}', -- JSON: top_k, rerank, rerank_candidates, min_semantic_score, min_rerank_score
    playground       INTEGER NOT NULL DEFAULT 0,
    upstream_ms      INTEGER,
    embed_ms         INTEGER,
    search_ms        INTEGER,
    rerank_ms        INTEGER,
    store_ms         INTEGER,
    total_ragdoll_ms INTEGER,
    candidate_count  INTEGER,
    result_count     INTEGER,
    generation_ms         INTEGER,
    generation_total_ms   INTEGER,
    prompt_tokens         INTEGER,
    completion_tokens     INTEGER,
    llm_model_id          TEXT REFERENCES llm_models(id),
    response_status  INTEGER NOT NULL DEFAULT 200,
    created_at       TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_queries_release ON queries(release_id);
CREATE INDEX IF NOT EXISTS idx_queries_stage ON queries(stage_id);
CREATE INDEX IF NOT EXISTS idx_queries_created ON queries(created_at);
CREATE INDEX IF NOT EXISTS idx_queries_playground ON queries(playground);
CREATE INDEX IF NOT EXISTS idx_queries_response_status ON queries(response_status);

CREATE TABLE IF NOT EXISTS system_metrics (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    recorded_at         TEXT NOT NULL DEFAULT (datetime('now')),
    cpu_percent         REAL NOT NULL,
    memory_used_bytes   INTEGER NOT NULL,
    memory_total_bytes  INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_system_metrics_recorded ON system_metrics(recorded_at);

CREATE TABLE IF NOT EXISTS query_chunks (
    query_id    TEXT NOT NULL REFERENCES queries(id) ON DELETE CASCADE,
    release_id  TEXT NOT NULL REFERENCES releases(id) ON DELETE CASCADE,
    stage_id    TEXT REFERENCES stages(id),
    step        TEXT NOT NULL CHECK (step IN ('semantic', 'rerank')),
    rank        INTEGER NOT NULL,
    chunk_id    TEXT NOT NULL,
    source_id   TEXT NOT NULL,
    score       REAL NOT NULL,
    metadata    TEXT NOT NULL DEFAULT '{}',
    content     TEXT,
    PRIMARY KEY (query_id, step, rank)
);

CREATE INDEX IF NOT EXISTS idx_query_chunks_release ON query_chunks(release_id);
CREATE INDEX IF NOT EXISTS idx_query_chunks_query ON query_chunks(query_id);

-- Full-text search index for hybrid retrieval (BM25)
CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(
    chunk_id UNINDEXED,
    content,
    tokenize = 'porter'
);

CREATE TRIGGER IF NOT EXISTS chunks_fts_ai AFTER INSERT ON chunks BEGIN
    INSERT INTO chunks_fts(chunk_id, content) VALUES (new.id, new.content);
END;

CREATE TRIGGER IF NOT EXISTS chunks_fts_ad AFTER DELETE ON chunks BEGIN
    DELETE FROM chunks_fts WHERE chunk_id = old.id;
END;

CREATE TRIGGER IF NOT EXISTS chunks_fts_au AFTER UPDATE OF content ON chunks BEGIN
    DELETE FROM chunks_fts WHERE chunk_id = old.id;
    INSERT INTO chunks_fts(chunk_id, content) VALUES (new.id, new.content);
END;

CREATE TABLE IF NOT EXISTS webhooks (
    id          TEXT PRIMARY KEY,
    release_id  TEXT NOT NULL REFERENCES releases(id) ON DELETE CASCADE,
    type        TEXT NOT NULL DEFAULT 'ingest_status',
    url         TEXT NOT NULL,
    secret      TEXT NOT NULL,
    events      TEXT NOT NULL DEFAULT '["completed","failed"]',
    active      INTEGER NOT NULL DEFAULT 1,
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_webhooks_release ON webhooks(release_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_webhooks_release_url ON webhooks(release_id, url);

CREATE TABLE IF NOT EXISTS webhook_deliveries (
    id          TEXT PRIMARY KEY,
    webhook_id  TEXT NOT NULL REFERENCES webhooks(id) ON DELETE CASCADE,
    event       TEXT NOT NULL,
    payload     TEXT NOT NULL,
    status_code INTEGER,
    response    TEXT,
    error       TEXT,
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_webhook_deliveries_webhook ON webhook_deliveries(webhook_id);

-- Seed default release and stage
INSERT OR IGNORE INTO releases (id, tag, message) VALUES
    ('00000000-0000-0000-0000-000000000001', 'first-release', 'Initial release');

INSERT OR IGNORE INTO stages (id, tag, release_id) VALUES
    ('00000000-0000-0000-0000-000000000002', 'prod', '00000000-0000-0000-0000-000000000001');

-- Seed default settings for first-release
INSERT OR IGNORE INTO settings (release_id, key, value) VALUES
    ('00000000-0000-0000-0000-000000000001', 'embedding_model', '"BAAI/bge-m3"'),
    ('00000000-0000-0000-0000-000000000001', 'rerank_model', '"BAAI/bge-reranker-v2-m3"'),
    ('00000000-0000-0000-0000-000000000001', 'payload_storage', '"per_request"'),
    ('00000000-0000-0000-0000-000000000001', 'chunking_strategy', '"semantic_split"'),
    ('00000000-0000-0000-0000-000000000001', 'sentence_buffer', '2'),
    ('00000000-0000-0000-0000-000000000001', 'breakpoint_percentile', '95'),
    ('00000000-0000-0000-0000-000000000001', 'min_chunk_tokens', '64'),
    ('00000000-0000-0000-0000-000000000001', 'max_chunk_tokens', '512'),
    ('00000000-0000-0000-0000-000000000001', 'max_upload_size', '52428800'),
    ('00000000-0000-0000-0000-000000000001', 'max_batch_size', '100'),
    ('00000000-0000-0000-0000-000000000001', 'generation_allowed', 'true'),
    ('00000000-0000-0000-0000-000000000001', 'dedup_policy', '"replace"');

-- Seed default model registry entries (all 1024-dim embed models per Option A)
INSERT OR IGNORE INTO models (name, kind, runtime, dim, uri, query_prefix, doc_prefix, status, is_default) VALUES
    ('BAAI/bge-m3', 'embed', 'onnx', 1024, 'BAAI/bge-m3', '', '', 'available', 1),
    ('BAAI/bge-large-en-v1.5', 'embed', 'onnx', 1024, 'BAAI/bge-large-en-v1.5', '', '', 'available', 0),
    ('mixedbread-ai/mxbai-embed-large-v1', 'embed', 'onnx', 1024, 'mixedbread-ai/mxbai-embed-large-v1', '', '', 'available', 0),
    ('intfloat/multilingual-e5-large', 'embed', 'onnx', 1024, 'intfloat/multilingual-e5-large', 'query: ', 'passage: ', 'available', 0),
    ('BAAI/bge-reranker-v2-m3', 'rerank', 'onnx', NULL, 'BAAI/bge-reranker-v2-m3', '', '', 'available', 1),
    ('jinaai/jina-reranker-v2-base-multilingual', 'rerank', 'onnx', NULL, 'jinaai/jina-reranker-v2-base-multilingual', '', '', 'available', 0),
    ('mixedbread-ai/mxbai-rerank-base-v1', 'rerank', 'onnx', NULL, 'mixedbread-ai/mxbai-rerank-base-v1', '', '', 'available', 0);
