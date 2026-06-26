# Architecture

<img src="../branding/logo.png" alt="Ragdoll" width="120">

Ragdoll v2 is a single-image, single-port local RAG system with **releases** (immutable content snapshots) and **stages** (production pointers).

## Components

- **Rust gateway**: HTTP API under `/api/v1`, JWT auth, retrieval pipeline, OpenAPI docs, management SPA at `/`
- **Python worker**: ingest jobs, extraction, chunking, embedding writes, step latency metrics
- **libSQL**: embedded database with vectors, job queue, per-release settings
- **ONNX models**: local embedding and reranking (lazy-loaded per release model name)

## Planes

| Plane | API path prefix | Reads | Writes |
|---|---|---|---|
| Release | `/api/v1/releases/{tag}/...` | Session or API key | Session + superadmin |
| Stage | `/api/v1/stages/{tag}/...` | Session or API key | **API key only** |

Stages resolve to a release for content; events (`queries`, `ingest_jobs`) record both `release_id` and optional `stage_id`.

## Data flow

1. Client authenticates (`POST /api/v1/auth/login` or API key JWT).
2. Client sends batch ingest to `/api/v1/releases/{tag}/sources` or `/api/v1/stages/{tag}/sources`.
3. Rust writes `sources` (with `release_id`) and `ingest_jobs` (with `release_id`, optional `stage_id`).
4. Python worker claims jobs, extracts text, chunks, embeds, writes `chunks`, records latency columns on `ingest_jobs`.
5. Client sends batch queries to the same plane prefix.
6. Rust embeds the query, applies hard filters scoped to `release_id`, cosine search, optional rerank, persists `queries` + `query_chunks` with latency metrics.

## Releases and stages

- **Release**: tagged content bundle (`sources`, `chunks`, settings). Created with `init: new|fork|template`.
- **Stage**: short tag pointing at a release (e.g. `prod` → `first-release`). Production traffic uses stage paths with API keys.

Bootstrap seeds release `first-release`, stage `prod`, and default settings.

## Design choices

- **Exact cosine over filtered rows** instead of ANN + filter, for correctness and simplicity.
- **Embedding model is per-release**; changing it requires manual re-ingest (no auto-reembed).
- **Dim 1024 whitelist** for supported ONNX models.
- **Batch endpoints** with multi-status responses for partial failures.
- **Filter DSL** via `filter` in JSON body or query param (base64url on GET/DELETE).

See [configuration.md](configuration.md) for environment variables.
