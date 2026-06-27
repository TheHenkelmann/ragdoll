# Ingestion

Ingest is how content enters Ragdoll. It is **asynchronous**: the Rust gateway
writes a `sources` row plus an `ingest_jobs` row, and the Python worker claims
the job, extracts text, chunks it ([chunking.md](chunking.md)), embeds the
chunks, and stores vectors.

For the end-to-end, runnable version of everything below, use
[tutorial/data_ingestion_tutorial.ipynb](tutorial/data_ingestion_tutorial.ipynb).

## The basic flow

```bash
# 1. Enqueue a source (returns source_id + job_id)
curl -sS -X POST http://localhost:8080/api/v1/releases/first-release/sources \
  -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  -d '[{"type":"text","name":"demo","content":"Ragdoll is a local RAG pipeline."}]'

# 2. Poll until status == completed
curl -sS "http://localhost:8080/api/v1/releases/first-release/sources?limit=10" \
  -H "Authorization: Bearer $TOKEN"
```

> **Poll before you query.** A source is only searchable once its job reaches
> `completed`. Querying earlier simply returns no matches for that source — it is
> not an error, the worker just has not finished yet.

The sources endpoint is a **batch** endpoint: it always takes an array and
returns a multi-status response, so a partial failure in a batch does not fail
the rest.

## Source types

| `type` | Required fields | Description |
|---|---|---|
| `text` | `content` | Plain text ingested directly |
| `file` | `content` (base64), `name` with extension | Binary upload; the worker picks an extractor from the extension |
| `url` | `url` | Web page fetched and extracted as Markdown at ingest time |

Optional on all types: `id`, `name`, `metadata` (JSON object), `config` (JSON
object).

## Supported file formats

| Extension | Processing |
|---|---|
| `.txt`, `.md`, `.csv`, `.json` | Read as UTF-8 text |
| `.pdf` | pypdf extraction; OCR fallback (Tesseract `deu+eng`) for image pages |
| `.docx` | python-docx paragraphs |
| `.xlsx`, `.xlsm` | openpyxl row flattening |
| `.pptx` | python-pptx slide text |

Max upload size defaults to 50 MiB (`max_upload_size` setting, per release).

URLs are extracted via trafilatura with `output_format=markdown`, so headings
and lists from web pages are preserved where possible — which directly improves
chunk quality (see [chunking.md](chunking.md)).

## Metadata: the key to good retrieval

Attach arbitrary JSON metadata when ingesting. Metadata is stored on the source
and **copied onto every chunk derived from it**, which is what makes hard
metadata filters at query time possible.

```bash
curl -sS -X POST http://localhost:8080/api/v1/releases/first-release/sources \
  -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  -d '[{
    "type": "text",
    "name": "policy-doc",
    "content": "Remote work is allowed up to three days per week.",
    "metadata": {
      "department": "hr",
      "language": "en",
      "tags": { "category": "policy" }
    }
  }]'
```

Nested keys are addressed with dot paths in filters: `meta.tags.category`,
`meta.section_path`, `meta.unit_kinds`. See
[querying.md → Filtering](querying.md#filtering-by-metadata).

> **Plan your metadata before bulk ingest.** Filters can only use metadata that
> was present at ingest time. If you forget a field, you must re-ingest the
> affected sources to add it.

## Ingest latency metrics

Every completed `ingest_job` records step timings, useful for spotting slow
stages:

`queue_ms`, `extract_ms`, `chunk_ms`, `embed_ms`, `db_write_ms`, `total_ms`,
plus `chunk_count` and `char_count`.

These feed the dashboard and the analytics endpoint
([operations.md](operations.md)).

## Deduplication

Per-release `dedup_policy` controls duplicate content (same SHA-256 hash within
a release):

| Value | Behavior |
|---|---|
| `skip` | Default — return existing source, skip re-ingest |
| `reject` | Fail the ingest request |
| `replace` | Delete old source/chunks and re-ingest |

Configure via **Settings** or `PATCH .../settings` with `"dedup_policy": "skip"`.

## Replace and metadata updates

- **`PUT /sources`** — replace sources by id (delete + re-ingest same id).
- **`PATCH /sources/{id}`** — update `metadata` without re-chunking; useful for
  filter fields you forgot at ingest time (does not add new chunk text).

## Ingest job status

Poll a release-wide summary instead of individual sources:

```bash
curl -sS "http://localhost:8080/api/v1/releases/first-release/ingest_jobs?details=true&limit=20" \
  -H "Authorization: Bearer $TOKEN"
```

Returns counts by status (`pending`, `processing`, `completed`, `failed`) and,
with `details=true`, recent jobs. Requires `sources:read`.

## Webhooks

Instead of polling, configure ingest-status webhooks per release. The worker POSTs
signed JSON on `completed` / `failed`. See
[operations.md → Webhooks](operations.md#webhooks).

## Reindex

Re-embed existing stored text after an embedding model change (no re-extraction):

```bash
curl -sS -X POST http://localhost:8080/api/v1/releases/first-release/reindex \
  -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{}'
```

Requires `sources:write`. See [operations.md → Reindex](operations.md#reindex).

## Related

- [models.md](models.md) — embedding model changes and reindex
- [chunking.md](chunking.md) — what happens to the extracted text
- [querying.md](querying.md) — retrieving and filtering what you ingested
- [pitfalls.md](pitfalls.md) — async timing, re-ingest triggers, metadata planning
