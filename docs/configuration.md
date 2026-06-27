# Configuration

All environment variables use the `RAGDOLL_` prefix.

## Required

| Variable | Description |
|---|---|
| `RAGDOLL_DATA_DIR` | Root data directory |
| `RAGDOLL_SECRET` | Master secret for JWT signing and encrypted LLM credential storage |

Cloud deploy templates ([deploy/README.md](../deploy/README.md)) generate a random secret automatically if you do not override it. That value is **not stored or displayed** for you. Set `RAGDOLL_SECRET` yourself before deploy for a stable production secret. Rotating the secret invalidates existing tokens **and** makes stored LLM API keys undecryptable.

Derived paths unless overridden:

- `${RAGDOLL_DATA_DIR}/db/ragdoll.db`
- `${RAGDOLL_DATA_DIR}/models/`
- `${RAGDOLL_DATA_DIR}/staging/`
- `${RAGDOLL_DATA_DIR}/backups/`

## Backup

| Variable | Default | Description |
|---|---|---|
| `RAGDOLL_BACKUP_DIR` | `${RAGDOLL_DATA_DIR}/backups` | Directory for local database snapshots |
| `RAGDOLL_BACKUP_KEEP_DAILY` | `7` | Number of daily backups to retain |
| `RAGDOLL_BACKUP_KEEP_MANUAL` | `10` | Number of manual backups to retain |

See [operations.md → Backup & Restore](operations.md#backup--restore) for triggers and restore steps.

## Auth bootstrap

| Variable | Default | Description |
|---|---|---|
| `RAGDOLL_SUPERADMIN_EMAIL` | `admin@ragdoll.ai` | Initial superadmin email |
| `RAGDOLL_SUPERADMIN_PW` | unset → password `admin` | Superadmin password; when unset, UI shows a warning banner |

When `RAGDOLL_SUPERADMIN_PW` is set, the password is enforced on every boot.

## Deploy-time defaults

| Variable | Default | Description |
|---|---|---|
| `RAGDOLL_EMBEDDING_MODEL` | `BAAI/bge-m3` | Fallback embedding model name |
| `RAGDOLL_EMBEDDING_DIM` | `1024` | Vector dimension (whitelist: 1024 only) |
| `RAGDOLL_DISTANCE_METRIC` | `cosine` | Stored vector comparison metric |
| `RAGDOLL_PORT` | `8080` | Gateway port |
| `RAGDOLL_ONNX_NUM_THREADS` | `4` | ONNX intra-op threads per model instance (applied to embedder and reranker) |
| `RAGDOLL_RERANK_POOL_SIZE` | `1` | Number of reranker model instances for concurrent queries (each uses ~model RAM) |
| `RAGDOLL_MODEL_DOWNLOAD_MAX_CONCURRENT` | `1` | Maximum simultaneous model downloads (keeps retrieval responsive) |
| `RAGDOLL_MODEL_DOWNLOAD_BANDWIDTH_BPS` | unset | Optional download bandwidth cap in bytes/sec (e.g. `10485760` = 10 MB/s) |
| `RAGDOLL_MODEL_DOWNLOAD_WRITE_CHUNK_BYTES` | `262144` | Read/write chunk size for throttled model downloads and materialization |
| `RAGDOLL_HF_HUB_OFFLINE` | `0` | Skip model downloads when artifacts are pre-mounted |
| `RAGDOLL_HF_TOKEN` | unset | Optional Hugging Face token |

## Worker

| Variable | Default | Description |
|---|---|---|
| `RAGDOLL_WORKER_POLL_INTERVAL_MS` | `1000` | Poll interval |
| `RAGDOLL_JOB_LEASE_SECONDS` | `300` | Stale processing recovery threshold |
| `RAGDOLL_MAX_ATTEMPTS` | `3` | Job retry limit |
| `RAGDOLL_WORKER_ID` | hostname | Worker identity |

## Path overrides

| Variable | Description |
|---|---|
| `RAGDOLL_DB_PATH` | Override database file path |
| `RAGDOLL_MODEL_CACHE_DIR` | Override model cache directory |
| `RAGDOLL_STAGING_DIR` | Override upload staging directory |
| `RAGDOLL_BACKUP_DIR` | Override backup snapshot directory |
| `RAGDOLL_MIGRATIONS_DIR` | Migration directory for Rust CLI |
| `RAGDOLL_STATIC_DIR` | Built frontend assets directory |

## Per-release runtime settings

Managed via `GET/PATCH /api/v1/releases/{tag}/settings` (`settings:read` /
`settings:write`):

- `embedding_model`, `rerank_model` — see [models.md](models.md)
- `rerank_max_length` (`0` = uncapped, or `128` / `256` / `512` / `1024` tokens; default `256`) — caps document length sent to the reranker
- `payload_storage` (`per_request` | `forced` | `forbidden`)
- `generation_allowed` (default `true`) — when `false`, requests with a `generation` object are rejected
- `dedup_policy` (`skip` | `reject` | `replace`) — duplicate content handling on ingest
- `chunking_strategy`, `sentence_buffer`, `breakpoint_percentile`
- `min_chunk_tokens`, `max_chunk_tokens`
- `max_upload_size`, `max_batch_size`

Query-time knobs (`top_k`, `rerank`, `rerank_candidates`, `min_semantic_score`,
`min_rerank_score`, `hybrid`, `bm25_weight`, `filter`) are per request with
hardcoded fallbacks in the gateway. Defaults: `min_semantic_score` and
`min_rerank_score` are `0.5`.

## Docker Compose

Set at minimum:

```yaml
environment:
  RAGDOLL_DATA_DIR: /data
  RAGDOLL_SECRET: ${RAGDOLL_SECRET:-change-me-in-production}
```

## Related

- [models.md](models.md) — embedding and rerank model whitelist
- [operations.md](operations.md) — running the server, offline mode
- [chunking.md](chunking.md) — what the chunking settings actually do
- [pitfalls.md](pitfalls.md) — settings that require a re-ingest
