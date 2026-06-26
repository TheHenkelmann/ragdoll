# Configuration

All environment variables use the `RAGDOLL_` prefix.

## Required

| Variable | Description |
|---|---|
| `RAGDOLL_DATA_DIR` | Root data directory |
| `RAGDOLL_JWT_SECRET` | HMAC secret for session and API-key JWTs |

Cloud deploy templates ([deploy/README.md](../deploy/README.md)) generate a random JWT secret automatically if you do not override it. That value is **not stored or displayed** for you. Set `RAGDOLL_JWT_SECRET` yourself before deploy for a stable production secret.

Derived paths unless overridden:

- `${RAGDOLL_DATA_DIR}/db/ragdoll.db`
- `${RAGDOLL_DATA_DIR}/models/`
- `${RAGDOLL_DATA_DIR}/staging/`

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
| `RAGDOLL_ONNX_NUM_THREADS` | `4` | ONNX intra-op threads |
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
| `RAGDOLL_MIGRATIONS_DIR` | Migration directory for Rust CLI |
| `RAGDOLL_STATIC_DIR` | Built frontend assets directory |

## Per-release runtime settings

Managed via `GET/PATCH /api/v1/releases/{tag}/settings` (superadmin, session token):

- `embedding_model`, `rerank_model`
- `payload_storage` (`per_request` | `forced` | `forbidden`)
- `chunking_strategy`, `sentence_buffer`, `breakpoint_percentile`
- `min_chunk_tokens`, `max_chunk_tokens`
- `max_upload_size`, `max_batch_size`

Query-time knobs (`top_k`, `rerank`, `rerank_candidates`, `filter`) are per request with hardcoded fallbacks in the gateway.

## Docker Compose

Set at minimum:

```yaml
environment:
  RAGDOLL_DATA_DIR: /data
  RAGDOLL_JWT_SECRET: ${RAGDOLL_JWT_SECRET:-change-me-in-production}
```

## Related

- [operations.md](operations.md) — running the server, models, offline mode
- [chunking.md](chunking.md) — what the chunking settings actually do
- [pitfalls.md](pitfalls.md) — settings that require a re-ingest
