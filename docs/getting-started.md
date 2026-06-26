# Getting started

This is the golden path: from nothing to your first retrieved chunk. Follow it
top to bottom — every step builds on the previous one.

If something behaves unexpectedly, check [pitfalls.md](pitfalls.md) first; it
collects the handful of traps that catch most newcomers.

## 1. Start the server

```bash
export RAGDOLL_JWT_SECRET=change-me-in-production
docker compose up --build
```

Or run the image directly with a bind mount for the data directory:

```bash
docker run --rm -p 8080:8080 \
  -e RAGDOLL_JWT_SECRET=change-me-in-production \
  -v /path/to/data:/data \
  ragdoll:latest
```

> **First start downloads models.** On the very first boot Ragdoll fetches the
> ONNX embedding and rerank models (~2 GB) into `${RAGDOLL_DATA_DIR}/models`.
> Until that finishes, `/api/v1/health` reports `ready: false` and queries will
> fail. This is normal — watch the logs and wait. For air-gapped setups see
> [operations.md → Offline mode](operations.md#offline-mode).

## 2. Confirm it is ready

```bash
curl -sS http://localhost:8080/api/v1/health
```

Wait until the response contains `"ready": true`. Then open:

| URL | Purpose |
|---|---|
| `http://localhost:8080/` | Management UI (login required) |
| `http://localhost:8080/api/v1/swagger-ui` | OpenAPI explorer |
| `http://localhost:8080/api/v1/health` | Readiness probe |

## 3. Log in

Ragdoll uses **Bearer JWT** — no cookies. On first boot it seeds a superadmin:

| Setting | Default | Override |
|---|---|---|
| Email | `admin@ragdoll.ai` | `RAGDOLL_SUPERADMIN_EMAIL` |
| Password | `admin` | `RAGDOLL_SUPERADMIN_PW` |

```bash
curl -sS -X POST http://localhost:8080/api/v1/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"email":"admin@ragdoll.ai","password":"admin"}'

export TOKEN="<token from the response>"
```

> **Change the default password.** While the default `admin` password is active
> the UI shows a red banner. Set `RAGDOLL_SUPERADMIN_PW` and restart to clear it.

## 4. Ingest your first document

Ingest is **asynchronous**: the gateway records the source and a job, and the
Python worker processes it in the background. The bootstrap already created a
release called `first-release`, so you can write to it immediately.

```bash
curl -sS -X POST http://localhost:8080/api/v1/releases/first-release/sources \
  -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  -d '[{"type":"text","name":"demo","content":"Ragdoll is a fully local RAG pipeline."}]'
```

Poll until the source `status` is `completed`:

```bash
curl -sS "http://localhost:8080/api/v1/releases/first-release/sources?limit=10" \
  -H "Authorization: Bearer $TOKEN"
```

> **Nothing is searchable until the job completes.** If a query returns no
> matches right after ingesting, the worker is probably still running. Poll the
> source status before querying.

## 5. Run your first query

```bash
curl -sS -X POST http://localhost:8080/api/v1/releases/first-release/queries \
  -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  -d '[{"text":"what is ragdoll?","top_k":5}]'
```

You get back ranked chunks with similarity scores. That is the full loop:
**ingest → embed → search → rank**, all local.

## 6. Do it for real

The hands-on tutorial ingests six realistic documents (`.md`, `.csv`, `.json`,
`.txt`, a URL, and plain text), attaches metadata, and shows retrieval with and
without filters:

- [tutorial/data_ingestion_tutorial.ipynb](tutorial/data_ingestion_tutorial.ipynb)

> **Run the notebook from `docs/tutorial/`** so the relative path
> `test_documents/` resolves, and make sure the server is up with models
> downloaded first.

## Where to go next

| You want to… | Read |
|---|---|
| Understand releases, stages, and auth | [concepts.md](concepts.md) |
| Ingest files, URLs, and metadata | [ingestion.md](ingestion.md) |
| Tune chunk quality | [chunking.md](chunking.md) |
| Filter and rerank queries | [querying.md](querying.md) |
| Operate the UI, models, analytics | [operations.md](operations.md) |
| Avoid the common traps | [pitfalls.md](pitfalls.md) |
