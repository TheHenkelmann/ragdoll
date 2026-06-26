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
    created_at          TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS api_keys (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS settings (
    release_id  TEXT NOT NULL REFERENCES releases(id) ON DELETE CASCADE,
    key         TEXT NOT NULL,
    value       TEXT NOT NULL,
    PRIMARY KEY (release_id, key)
);

CREATE TABLE IF NOT EXISTS models (
    name        TEXT PRIMARY KEY,
    kind        TEXT NOT NULL CHECK (kind IN ('embed', 'rerank', 'llm')),
    runtime     TEXT NOT NULL CHECK (runtime IN ('onnx', 'litellm')),
    dim         INTEGER,
    uri         TEXT,
    status      TEXT NOT NULL DEFAULT 'available',
    is_default  INTEGER NOT NULL DEFAULT 0
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
    created_at    TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_sources_release ON sources(release_id);
CREATE INDEX IF NOT EXISTS idx_sources_created ON sources(created_at);
CREATE INDEX IF NOT EXISTS idx_sources_status ON sources(status);

CREATE TABLE IF NOT EXISTS ingest_jobs (
    id            TEXT PRIMARY KEY,
    release_id    TEXT NOT NULL REFERENCES releases(id) ON DELETE CASCADE,
    stage_id      TEXT REFERENCES stages(id),
    source_id     TEXT NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
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
    total_ms         INTEGER,
    candidate_count  INTEGER,
    result_count     INTEGER,
    response_status  INTEGER NOT NULL DEFAULT 200,
    created_at       TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_queries_release ON queries(release_id);
CREATE INDEX IF NOT EXISTS idx_queries_stage ON queries(stage_id);
CREATE INDEX IF NOT EXISTS idx_queries_created ON queries(created_at);
CREATE INDEX IF NOT EXISTS idx_queries_playground ON queries(playground);
CREATE INDEX IF NOT EXISTS idx_queries_response_status ON queries(response_status);

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
    ('00000000-0000-0000-0000-000000000001', 'max_batch_size', '100');

-- Seed default model registry entries
INSERT OR IGNORE INTO models (name, kind, runtime, dim, uri, status, is_default) VALUES
    ('BAAI/bge-m3', 'embed', 'onnx', 1024, 'BAAI/bge-m3', 'available', 1),
    ('BAAI/bge-reranker-v2-m3', 'rerank', 'onnx', NULL, 'BAAI/bge-reranker-v2-m3', 'available', 1);
