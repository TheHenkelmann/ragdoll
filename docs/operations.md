# Operations

Everything about running Ragdoll day-to-day: the management UI, models,
analytics, offline operation, the data layout, the API surface, and local
development.

## Management UI

After login, navigation has two levels:

**Primary sidebar (global)**

| Item | Purpose |
|---|---|
| **Stages** | List stages, create/edit, point at releases |
| **Releases** | List releases, create/fork/delete |
| **API Keys** | Create keys with permissions and rate limits |
| **Models** | ONNX embedding/rerank catalog, required/missing status, download, verify, delete |
| **Backups** | List, create, upload, download, restore DB snapshots |
| **Users** | Non-superadmin users and permission sets |
| **Profile** | Change password (non-superadmin) |

**Secondary sidebar (per release)**

| Item | Purpose |
|---|---|
| **Dashboard** | Request counts, latency p50/p95, sources/chunks overview |
| **Playground** | Query builder, filters, rerank, generation, SSE, code snippets |
| **Sources** | List sources, browse chunks, upload |
| **LLM Models** | BYO-LLM credentials and model configs for generation |
| **Database** | Read-only table viewer (release-scoped, filter DSL) |
| **Webhooks** | Ingest-status webhook configuration |
| **Settings** | Chunking, ONNX model selection, payload policy, reindex |

Use **Settings** to choose which embedding/rerank models a release uses. Use the
global **Models** page to manage the shared ONNX artifacts on disk, see missing
required models, verify downloads, and spot embedding mismatches. See
[models.md](models.md).

## Data layout

```text
RAGDOLL_DATA_DIR/
  db/ragdoll.db       # libSQL database (vectors, jobs, settings, …)
  models/             # cached ONNX model artifacts
  staging/            # temporary upload staging for the worker
  backups/            # local database snapshots (see below)
```

## Backup & Restore

Ragdoll stores local database snapshots under `${RAGDOLL_DATA_DIR}/backups/`
(override with `RAGDOLL_BACKUP_DIR`). Only `db/ragdoll.db` is backed up — model
artifacts in `models/` are reproducible and not included.

### Triggers

| Trigger | When |
|---|---|
| **Daily** | Once per UTC calendar day. Checked on server start (catch-up for serverless cold starts) and on an hourly tick while running. |
| **Manual** | User with `backups:create` via `POST /api/v1/backups` or the **Backups** page. |
| **Restore** | User with `backups:restore` via API or **Backups** page (with confirmation). Creates a safety backup first. |

### File names

```text
ragdoll-<YYYYMMDDThhmmss[mmm]Z>-<trigger>.db
# e.g. ragdoll-20260627T093500123Z-daily.db
ragdoll-20260627T101212456Z-manual.db
```

Old backups are pruned automatically per trigger (`RAGDOLL_BACKUP_KEEP_DAILY`,
`RAGDOLL_BACKUP_KEEP_MANUAL`). The **Backups** page shows the active retention limits.

Upload accepts libSQL/SQLite snapshots whose **file name** matches
`ragdoll-<YYYYMMDDThhmmss[mmm]Z>-<daily|manual>.db`. Invalid names are rejected before
import. The uploaded file keeps its original name. Download returns the raw `.db` file.

### Restore

From the UI (**Backups** → **Restore** on a snapshot): confirm the warning checkbox, then
restore. Optionally enable **Create a manual safety backup** before restore (off by default).

Via API:

```bash
curl -sS -X POST http://localhost:8080/api/v1/backups/restore \
  -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"file_name":"ragdoll-20260627T093500123Z-daily.db","safety_backup":false}'
```

Set `"safety_backup": true` to create a manual snapshot of the current database before restore.

Manual file restore (e.g. when the server is stopped):

1. Stop the Ragdoll container/process.
2. Replace `${RAGDOLL_DATA_DIR}/db/ragdoll.db` with the chosen snapshot.
3. Remove any leftover `ragdoll.db-wal` and `ragdoll.db-shm` files in the same directory.
4. Start Ragdoll again.

> Create a **manual backup** before risky operations (embedding model change,
> schema migration, bulk re-ingest). See [pitfalls.md](pitfalls.md).

## Retrieval models (ONNX)

Embedding and rerank models are local ONNX artifacts configured **per release**
and cached **instance-wide**. See [models.md](models.md) for what embedding and
rerank models do, the 1024-dim whitelist, download/verify flow, and re-ingest
rules when switching embedders.

## BYO-LLM generation (optional)

Ragdoll retrieval stays fully local. Answer generation is opt-in per query and
uses **your** external LLM credentials — Ragdoll never ships with a hosted LLM.

Setup (requires `llm_credentials:write` and `llm_models:write`, or superadmin):

1. **Credentials** — `POST /api/v1/releases/{tag}/llm_credentials` stores provider API keys
   encrypted at rest. Keys are write-only and never returned via the API.
2. **Models** — `POST /api/v1/releases/{tag}/llm_models` defines taggable LLM configs
   (provider, model name, optional custom endpoint, system prompt, defaults). Each
   model can be connectivity-tested via `POST /api/v1/releases/{tag}/llm_models/{model_tag}/test`, which
   issues a minimal connectivity request (16 output tokens); the UI also runs this test
   automatically right after a model is created.
3. **Settings** — set `generation_allowed: false` on a release to block all
   generation requests.

Manage credentials and models per release in the UI under **LLM Models** (release
sidebar). Configure `generation_allowed` under **Settings**.

Supported providers: `openai`, `openai_compat`, `azure`, `anthropic`, `gemini`,
`vertex`, `groq`, `deepseek`, `xai` (via the [`genai`](https://crates.io/crates/genai) crate).

### Provider vs. endpoint

| Case | Provider | Endpoint | Auth |
|---|---|---|---|
| OpenAI | `openai` | default (empty) | `Authorization: Bearer` |
| OpenAI-compatible (OpenRouter, vLLM, LM Studio, …) | `openai_compat` | `…/v1/` base URL (not `/chat/completions`) | `Authorization: Bearer` |
| Azure OpenAI | `azure` | full deployment or Responses URL incl. `api-version` | `api-key` header |
| Google Gemini (AI Studio) | `gemini` | default (empty) | API key |
| Google Vertex AI (GCP) | `vertex` | JSON: `{"project_id":"…","location":"…"}` | Service account JSON |
| Anthropic / Groq / DeepSeek / xAI | their provider | default (empty) | provider-native |

For **Azure deployment URLs** (`…/deployments/<name>/chat/completions`), Ragdoll
reads the deployment name from the URL — a separate model field is not required.
For **Azure Responses API** URLs (`…/openai/responses`), set the model/deployment
name explicitly (e.g. `gpt-5.4-mini`).

Paste **base URLs only** for OpenAI-compatible providers. If you copy a full
`/chat/completions` URL from provider docs, Ragdoll strips the suffix automatically.

**Responses API** (GPT-5+ on Azure) — endpoint must contain `/responses`:

```text
https://<resource>.cognitiveservices.azure.com/openai/responses?api-version=2025-04-01-preview
```

Use the **model/deployment name** (e.g. `gpt-5.4-mini`) as the model name field.

**Chat Completions API** (older Azure deployments) — endpoint contains
`/chat/completions`:

```text
https://<resource>.openai.azure.com/openai/deployments/<deployment>/chat/completions?api-version=2024-10-21
```

Use the **deployment name** as the model name field.

For OpenAI's own Responses API (non-Azure), use provider `openai_resp` or an
`openai` endpoint whose URL contains `/responses`.

**Google Vertex AI on GCP** uses provider `vertex`. Many GCP organizations disable
API key creation; Ragdoll accepts a **service account key JSON** as the credential
(stored encrypted, write-only). The model endpoint field holds JSON with
`project_id` and `location` (region, or `global`):

```json
{"project_id": "my-gcp-project", "location": "europe-west1"}
```

Model name examples: `gemini-2.5-flash`, `claude-sonnet-4-6`. Ragdoll obtains
short-lived OAuth2 tokens from the service account via the `gcp_auth` crate.

**Google Gemini (AI Studio)** remains provider `gemini` with a normal API key —
that is a different product from Vertex AI.

See [querying.md → Optional answer generation](querying.md#optional-answer-generation)
for request shape and SSE protocol.

## Reindex

Re-embed sources that have stored extracted text — required after an **embedding
model** change, or to recover from embedding drift. Does not re-extract files
from disk; uses text already stored on the source.

```bash
# Reindex all sources with stored text in a release
curl -sS -X POST http://localhost:8080/api/v1/releases/first-release/reindex \
  -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{}'

# Reindex one source or a filtered subset
curl -sS -X POST http://localhost:8080/api/v1/releases/first-release/reindex \
  -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"source_id":"<uuid>"}'

# SSE progress for a batch
curl -N http://localhost:8080/api/v1/releases/first-release/reindex/{batch_id}/events \
  -H "Authorization: Bearer $TOKEN"
```

Requires `sources:write` to start; `sources:read` for progress events. The
**Settings** page offers the same flow with a live progress panel.

## Offline mode

On first start Ragdoll downloads the default models into
`${RAGDOLL_DATA_DIR}/models`. That requires network access once.

For fully offline / air-gapped operation:

1. Pre-populate `${RAGDOLL_DATA_DIR}/models` with ONNX artifacts for your chosen
   embed/rerank models ([models.md](models.md)).
2. Set `RAGDOLL_HF_HUB_OFFLINE=1`.

## Webhooks

Configure ingest-status webhooks per release (`type: ingest_status`). The worker
POSTs signed JSON on source `completed` / `failed` events.

```bash
# Create webhook (superadmin or `webhooks:write`)
# Each webhook gets a unique signing secret (`rd_whsec_` + 32 random bytes, base64url).
# View it via the key icon in the Webhooks UI or GET .../webhooks/{id}/secret.
curl -sS -X POST "http://localhost:8080/api/v1/releases/first-release/webhooks" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"type":"ingest_status","url":"https://example.com/hook","events":["completed","failed"]}'

# Send test payload
curl -sS -X POST "http://localhost:8080/api/v1/releases/first-release/webhooks/{id}/test" \
  -H "Authorization: Bearer $TOKEN"
```

Verify signatures with HMAC-SHA256 over `{timestamp}.{raw_body}` using the webhook secret
(`GET /releases/{tag}/webhooks/{id}/secret` or the key icon in the Webhooks UI):

```
X-Ragdoll-Signature: sha256=<hex>
X-Ragdoll-Timestamp: <unix_seconds>
signing_input = "{timestamp}.{raw_body}"
```

The timestamp is part of the signed payload, so replays with an old timestamp fail verification
when you enforce a freshness window on `X-Ragdoll-Timestamp`.

Delivery failures are logged in `webhook_deliveries` and never block ingest.

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
| `/api/v1/auth/*` | Login, status, bootstrap info, password change |
| `/api/v1/users`, `/api/v1/api_keys` | User/key management (permission-gated) |
| `/api/v1/releases`, `/api/v1/stages` | Release/stage CRUD |
| `/api/v1/models` | ONNX catalog (`models:read`) |
| `/api/v1/models/status` | Local, required, missing, mismatch status (`models:read`) |
| `/api/v1/models/{name}/download` | Download ONNX artifacts (`models:download`) |
| `/api/v1/models/{name}/download/stream` | SSE download + verify progress (`models:download`) |
| `/api/v1/models/{name}/test` | Test inference (`models:read`) |
| `/api/v1/models/{name}` | Delete local artifacts (`models:delete`) |
| `/api/v1/analytics` | Aggregated metrics |
| `/api/v1/backups` | List/create database backups (`backups:*`) |
| `/api/v1/backups/restore` | Restore from snapshot (`backups:restore`) |
| `/api/v1/backups/download` | Download backup file (`backups:download`) |
| `/api/v1/backups/upload` | Upload backup file (`backups:upload`, multipart `file`) |
| `/api/v1/backups/delete` | Delete backup file (`backups:delete`) |
| `/api/v1/playground/{tag}/queries` | Session-only playground queries |

Nested under `/api/v1/releases/{tag}/...` and `/api/v1/stages/{tag}/...`:

| Route | Methods | Description |
|---|---|---|
| `/sources` | GET, POST, PUT, DELETE | Source ingest, replace, listing |
| `/sources/{id}` | PATCH | Update source metadata |
| `/chunks` | GET, POST, DELETE, PATCH | Chunk access and manual writes |
| `/queries` | POST, GET, DELETE | Batch retrieval (API key) |
| `/queries/{id}` | GET | Query detail with semantic/rerank steps |
| `/settings` | GET, PATCH | Per-release runtime settings |
| `/reindex` | POST | Queue re-embed jobs for sources |
| `/reindex/{batch_id}/events` | GET | SSE reindex progress |
| `/ingest_jobs` | GET | Ingest queue summary (optional job list) |
| `/llm_credentials` | GET, POST, PUT, DELETE | BYO-LLM credentials (`llm_credentials:*`) |
| `/llm_models` | GET, POST, PUT, DELETE | LLM model configs (`llm_models:*`) |
| `/llm_models/{model_tag}` | PUT, DELETE | Edit or delete a release LLM model |
| `/llm_models/{model_tag}/test` | POST | Connectivity test |
| `/webhooks` | GET, POST | Ingest webhooks (`webhooks:*`) |
| `/webhooks/{id}` | PATCH, DELETE | Update or delete webhook |
| `/webhooks/{id}/secret` | GET | Reveal signing secret |
| `/webhooks/{id}/test` | POST | Send test delivery |
| `/db/{table}` | GET | Read-only table viewer |

All batch endpoints return multi-status responses for partial failures. The full
contract is browsable at `/api/v1/swagger-ui`.

## Local development

```bash
export RAGDOLL_DATA_DIR=$PWD/.data
export RAGDOLL_SECRET=dev-local-secret
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
- [models.md](models.md) — embedding and rerank models
- [pitfalls.md](pitfalls.md) — the operational traps in one place
