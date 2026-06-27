# Pitfalls (breaking points)

Ragdoll is powerful, but a handful of behaviors trip up almost everyone the first
time. This page collects them in one place as a quick-reference checklist. Each
entry links to the guide where it is explained in context.

## Setup

- **First start downloads ~2 GB of models.** Until the download finishes,
  `/api/v1/health` reports `ready: false` and queries fail. This is expected —
  watch the logs and wait. → [getting-started.md](getting-started.md),
  [operations.md → Offline mode](operations.md#offline-mode)
- **Default `admin` password is insecure.** The UI shows a red banner until you
  set `RAGDOLL_SUPERADMIN_PW` and restart. →
  [getting-started.md](getting-started.md)
- **Run the tutorial from `docs/tutorial/`.** The notebook uses the relative path
  `test_documents/`; from any other working directory it will not find the files.
  → [tutorial/data_ingestion_tutorial.ipynb](tutorial/data_ingestion_tutorial.ipynb)

## Auth and planes

- **Query endpoints require an API key.** `/api/v1/releases/{tag}/queries` and
  `/api/v1/stages/{tag}/queries` reject session tokens (UI login). Use an API key
  with `queries:run` for production queries; use `/api/v1/playground/{tag}/queries`
  with a session token for interactive debugging in the UI. → [querying.md](querying.md)
- **API keys need the right permissions.** A key without `queries:run` returns
  `403`. Grant only what each integration needs. → [concepts.md](concepts.md)
- **Rate limits on API keys.** Optional `rpm` / `rph` per key; exceeding them
  returns `429` with `Retry-After`. Common when batch-testing in a notebook.
- **The stage plane (`/api/v1/stages/...`) only accepts API-key writes.** A
  session token (your UI login) will be rejected for writes. Use a session token
  on the release plane during development, and an API key for stage/production. →
  [concepts.md](concepts.md)
- **API key JWTs are shown only once.** Copy the token at creation time; it
  cannot be retrieved afterward. → [concepts.md](concepts.md)

## Ingestion

- **Ingest is asynchronous.** Nothing is searchable until the source `status`
  reaches `completed`. If a fresh query returns no matches, poll the source first
  — it is usually just the worker still running, not an error. →
  [ingestion.md](ingestion.md)
- **Filters only see metadata present at ingest time.** Decide your metadata
  schema before bulk-ingesting; adding a field later means re-ingesting. →
  [ingestion.md](ingestion.md), [querying.md](querying.md)

## Re-ingest triggers (silent footguns)

These changes do **not** retroactively update existing chunks — you must
re-ingest the affected sources:

- **Changing the embedding model.** Old vectors live in the old model's space and
  will not match new queries. Ragdoll never auto-reembeds — trigger **reindex** or
  re-ingest. → [models.md](models.md), [operations.md → Reindex](operations.md#reindex)
- **Changing chunking settings** (`sentence_buffer`, `breakpoint_percentile`,
  `min/max_chunk_tokens`, etc.). Existing chunks are not re-split. →
  [chunking.md → Tuning](chunking.md#tuning)

## Operations

- **Cloud deploy uses an auto-generated secret by default.** One-click deploy
  templates set a random `RAGDOLL_SECRET` that is **not saved or shown to you**.
  Redeploying without overriding it invalidates existing tokens **and** makes
  stored LLM credentials undecryptable. Set `RAGDOLL_SECRET` before deploy for
  production. → [deploy/README.md](../deploy/README.md)
- **Rotating `RAGDOLL_SECRET` invalidates encrypted LLM credentials.** You must
  re-enter provider API keys after a secret change. →
  [configuration.md](configuration.md)
- **Reset the DB after schema/seed changes.** After editing
  `migrations/0001_init.sql`, delete `${RAGDOLL_DATA_DIR}/db` and let migrations
  re-run, otherwise you get migration mismatches. →
  [operations.md → Local development](operations.md#local-development)
- **Back up before risky changes.** Create a manual backup before changing the
  embedding model, editing migrations, or bulk re-ingesting. Daily backups cover
  routine protection; manual snapshots mark intentional save points. →
  [operations.md → Backup & Restore](operations.md#backup--restore)
- **Only dimension-1024 models are supported** (Option A whitelist). Embedding:
  `BAAI/bge-m3`, `BAAI/bge-large-en-v1.5`, `mixedbread-ai/mxbai-embed-large-v1`,
  `intfloat/multilingual-e5-large`. Rerank: `BAAI/bge-reranker-v2-m3`,
  `jinaai/jina-reranker-v2-base-multilingual`, `mixedbread-ai/mxbai-rerank-base-v1`.
  E5 models require query/passage prefixes (applied automatically). →
  [models.md](models.md)
