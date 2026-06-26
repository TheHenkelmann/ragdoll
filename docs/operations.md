# Operations

Everything about running Ragdoll day-to-day: the management UI, models,
analytics, offline operation, the data layout, the API surface, and local
development.

## Management UI

After login, the UI is organized around **releases** and **stages** (see
[concepts.md](concepts.md)). Breadcrumb navigation switches between them, and
both can be created from modal dialogs.

| Tab | Purpose |
|---|---|
| **Dashboard** | Request counts, latency p50/p95 per pipeline step, sources/chunks overview, circle-packing chart |
| **Playground** | Interactive query builder with filter UI, rerank controls, timeline, code-snippet export |
| **Sources** | List the release's sources, browse chunks per source |
| **Database** | Read-only table viewer (release-scoped, with the filter DSL) |
| **Settings** | Per-release Semantic Split tuning, models, payload-storage policy |

## Data layout

```text
RAGDOLL_DATA_DIR/
  db/ragdoll.db       # libSQL database (vectors, jobs, settings, …)
  models/             # cached ONNX model artifacts
  staging/            # temporary upload staging for the worker
```

## Models

Embedding and rerank models are configured **per release** via settings.
Supported whitelist (dimension 1024):

- `BAAI/bge-m3` (embedding)
- `BAAI/bge-reranker-v2-m3` (rerank)

```bash
# Download model artifacts ahead of time
curl -sS -X POST http://localhost:8080/api/v1/models/BAAI/bge-m3/download \
  -H "Authorization: Bearer $TOKEN"
```

> **Changing the embedding model requires manual re-ingest.** Ragdoll does not
> auto-reembed existing chunks — old vectors stay in the old model's space and
> will not match new queries correctly. Re-ingest affected sources after a model
> change. See [pitfalls.md](pitfalls.md).

## Offline mode

On first start Ragdoll downloads the default models into
`${RAGDOLL_DATA_DIR}/models`. That requires network access once.

For fully offline / air-gapped operation:

1. Pre-populate `${RAGDOLL_DATA_DIR}/models` with the ONNX artifacts for
   `BAAI/bge-m3` and `BAAI/bge-reranker-v2-m3`.
2. Set `RAGDOLL_HF_HUB_OFFLINE=1`.

## Analytics and observability

```bash
# Aggregated metrics, release lens (14 days default)
curl -sS "http://localhost:8080/api/v1/analytics?lens=release&tag=first-release&days=14" \
  -H "Authorization: Bearer $TOKEN"

# Stage lens
curl -sS "http://localhost:8080/api/v1/analytics?lens=stage&tag=prod&days=14" \
  -H "Authorization: Bearer $TOKEN"
```

Returns request counts, a daily histogram, p50/p95 per latency step, source and
chunk counts, a chunks-per-source breakdown, and the metadata-key distribution.

The UI **Database** tab exposes a read-only viewer for whitelisted tables, always
release-filtered. Playground queries are excluded from the `queries` table view.

## API overview

All HTTP API routes are under `/api/v1`; the web UI is served from `/`. Public
routes (no auth): `/api/v1/health`, `/api/v1/auth/login`, `/api/v1/auth/info`.

Top-level routes:

| Route | Description |
|---|---|
| `/api/v1/auth/*` | Login, status, bootstrap info |
| `/api/v1/users`, `/api/v1/api_keys` | Superadmin user/key management |
| `/api/v1/releases`, `/api/v1/stages` | Release/stage CRUD |
| `/api/v1/models` | Model registry and download |
| `/api/v1/analytics` | Aggregated metrics |

Nested under `/api/v1/releases/{tag}/...` and `/api/v1/stages/{tag}/...`:

| Route | Methods | Description |
|---|---|---|
| `/sources` | GET, POST, PUT, DELETE | Source ingest and listing |
| `/chunks` | GET, POST, DELETE, PATCH | Chunk access and manual writes |
| `/queries` | POST, GET, DELETE | Batch retrieval |
| `/queries/{id}` | GET | Query detail with semantic/rerank steps |
| `/settings` | GET, PATCH | Per-release runtime settings |
| `/db/{table}` | GET | Read-only table viewer |

All batch endpoints return multi-status responses for partial failures. The full
contract is browsable at `/api/v1/swagger-ui`.

## Local development

```bash
export RAGDOLL_DATA_DIR=$PWD/.data
export RAGDOLL_JWT_SECRET=dev-local-jwt-secret
./scripts/dev-local.sh
```

Lifecycle commands: `./scripts/dev-local.sh stop|status|logs`.

> **Reset the DB after schema or seed changes.** After editing
> `migrations/0001_init.sql`, the old database is incompatible. Delete it and let
> migrations run again:
>
> ```bash
> rm -rf $RAGDOLL_DATA_DIR/db
> ./scripts/dev-local.sh
> ```

## Related

- [configuration.md](configuration.md) — every environment variable
- [architecture.md](architecture.md) — components and data flow
- [pitfalls.md](pitfalls.md) — the operational traps in one place
