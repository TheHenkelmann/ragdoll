# Architecture

<img src="../branding/logo.png" alt="Ragdoll" width="120">

Ragdoll is a single-image, single-port RAG system with **releases** (content
snapshots) and **stages** (production pointers). Retrieval runs locally on ONNX;
answer generation is optional via BYO-LLM.

## Components

- **Rust gateway**: HTTP API under `/api/v1`, JWT auth with fine-grained
  permissions, retrieval pipeline (vector + optional BM25 hybrid), rerank,
  optional generation, OpenAPI, management SPA at `/`
- **Python worker**: ingest jobs, extraction, chunking, embedding writes, ingest
  webhooks, step latency metrics
- **libSQL**: embedded database with 1024-dim vectors, FTS (`chunks_fts` for
  hybrid search), job queue, per-release settings, encrypted LLM credentials
- **ONNX models**: local embedding and reranking via fastembed; artifacts cached
  under `${RAGDOLL_DATA_DIR}/models/` (see [models.md](models.md))
- **Backup service**: scheduled and manual DB snapshots under `backups/`

## Planes

| Plane | API path prefix | Reads | Writes |
|---|---|---|---|
| Release | `/api/v1/releases/{tag}/...` | Session or API key | Session or API key (permission-gated; queries need API key) |
| Stage | `/api/v1/stages/{tag}/...` | Session or API key | **API key only** |
| Playground | `/api/v1/playground/{tag}/...` | Session only | Session only (queries) |

Stages resolve to a release for content; events (`queries`, `ingest_jobs`) record
both `release_id` and optional `stage_id`.

## Data flow

### Ingest

1. Client authenticates (session or API key with `sources:write`).
2. Client sends batch ingest to a release or stage prefix.
3. Rust writes `sources` and `ingest_jobs` (with `release_id`, optional `stage_id`).
4. Python worker claims jobs, extracts, semantic-splits, embeds, writes `chunks`
   and FTS rows, records ingest latencies.
5. On `completed` / `failed`, configured ingest webhooks receive signed POSTs.

### Query (retrieval)

1. Client sends batch queries (API key on release/stage; session on playground).
2. Rust embeds the query, applies hard filters scoped to `release_id`.
3. Optional hybrid mode fuses vector ranks with BM25 (`chunks_fts`) via RRF.
4. Optional cross-encoder rerank on top candidates.
5. Results include **citations** (source id, char offsets, page, snippet).
6. Rust persists `queries` + `query_chunks` with per-step latency.

### Query (generation, optional)

7. If `generation` is present and allowed, Rust calls the configured external
   LLM with cited chunks as context (sync batch or SSE stream).

## Auth model

- **Superadmin** bypasses all permission checks (bootstrap user).
- **Users** (session tokens) carry a JSON permission list; `releases:read` is
  always granted.
- **API keys** carry permissions and optional `rpm` / `rph` rate limits.

See [concepts.md → Permissions](concepts.md#permissions).

## Releases and stages

- **Release**: tagged bundle (`sources`, `chunks`, settings, `llm_credentials`,
  `llm_models`). Created with `init: new|fork|template`. Fork copies content and
  LLM config but not `queries` or `ingest_jobs`.
- **Stage**: short tag pointing at a release (e.g. `prod` → `first-release`).

Bootstrap seeds release `first-release`, stage `prod`, and default settings.

## Design choices

- **Exact cosine over filtered rows** instead of ANN + filter, for correctness.
- **Optional BM25 hybrid** via FTS5 + reciprocal rank fusion (per-request).
- **Embedding model per release**; changing it requires reindex/re-ingest.
- **Dim-1024 ONNX whitelist** only ([models.md](models.md)).
- **Batch endpoints** with multi-status responses.
- **Filter DSL** in JSON body or query param (base64url on GET/DELETE).
- **Master secret** (`RAGDOLL_SECRET`) for JWT signing and LLM credential encryption.

## Related

- [models.md](models.md) — embedding and rerank models
- [configuration.md](configuration.md) — environment variables
- [concepts.md](concepts.md) — planes and permissions
