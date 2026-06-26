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
  will not match new queries. Ragdoll never auto-reembeds. →
  [operations.md → Models](operations.md#models)
- **Changing chunking settings** (`sentence_buffer`, `breakpoint_percentile`,
  `min/max_chunk_tokens`, etc.). Existing chunks are not re-split. →
  [chunking.md → Tuning](chunking.md#tuning)

## Operations

- **Cloud deploy uses an auto-generated JWT secret by default.** One-click deploy
  templates set a random `RAGDOLL_JWT_SECRET` that is **not saved or shown to you**.
  Redeploying without overriding it invalidates existing tokens. Set
  `RAGDOLL_JWT_SECRET` before deploy for production. →
  [deploy/README.md](../deploy/README.md)
- **Reset the DB after schema/seed changes.** After editing
  `migrations/0001_init.sql`, delete `${RAGDOLL_DATA_DIR}/db` and let migrations
  re-run, otherwise you get migration mismatches. →
  [operations.md → Local development](operations.md#local-development)
- **Only dimension-1024 models are supported** (`BAAI/bge-m3`,
  `BAAI/bge-reranker-v2-m3`). Other models are off the whitelist. →
  [operations.md → Models](operations.md#models)
