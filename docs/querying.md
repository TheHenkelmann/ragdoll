# Querying

Retrieval is the read side of Ragdoll: embed the query, apply hard metadata
filters, run exact cosine search over the matching chunk rows, and optionally
rerank with a cross-encoder. Ragdoll is **retrieval-only** — it returns ranked
chunks, it does not call an LLM to generate an answer.

## Where queries go

For production, queries typically go through a **stage** with an **API key**:

```bash
curl -sS -X POST 'http://localhost:8080/api/v1/stages/prod/queries?store_payload=false' \
  -H "Authorization: Bearer $API_KEY_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '[{"text":"local RAG pipeline","top_k":10,"rerank":true,"rerank_candidates":50}]'
```

During development you can query the release plane directly with a session token
(`/api/v1/releases/{tag}/queries`). The stage plane requires an API key — see
[concepts.md](concepts.md).

Queries are a **batch** endpoint: send an array, get a multi-status response.

## Per-request body fields

| Field | Default | Description |
|---|---|---|
| `text` | required | Query string |
| `top_k` | `10` | Final results returned |
| `rerank` | `true` | Enable the cross-encoder reranker |
| `rerank_candidates` | `50` | Semantic pool size before rerank |
| `filter` | none | Hard filter DSL (see below) |

Query knobs are **per request only**; there is no per-release default for them
(the gateway has hardcoded fallbacks).

## Query parameters

| Param | Default | Description |
|---|---|---|
| `store_payload` | `false` | Persist query text and chunk content in the DB |
| `ts_start` | none | Epoch ms anchor for `upstream_ms` latency |
| `playground` | `false` | Internal UI flag; always stores payloads |

## Filtering by metadata

Hard filters run **before** vector search, so they both scope results and reduce
the search space. Use the `meta.` prefix for chunk metadata; filter on chunk
columns such as `source_id` directly.

| Operator | Meaning |
|---|---|
| `eq`, `ne` | Equal / not equal |
| `gt`, `gte`, `lt`, `lte` | Comparison |
| `in`, `nin` | Value in / not in array |
| `contains` | SQL `LIKE` substring |
| `exists` | Field present |

Combine clauses with `and`, `or`, `not`:

```bash
curl -sS -X POST http://localhost:8080/api/v1/releases/first-release/queries \
  -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  -d '[{
    "text": "remote work policy",
    "filter": {
      "and": [
        { "field": "meta.department", "op": "eq", "value": "hr" },
        { "field": "meta.language", "op": "eq", "value": "en" }
      ]
    }
  }]'
```

> **Filters only see metadata that existed at ingest time.** A filter on a field
> you never attached will match nothing. Plan metadata during
> [ingestion](ingestion.md).

The same filter DSL works on `GET /queries`, `GET /chunks`, `GET /sources`, and
`GET /db/{table}` via a `filter` query parameter (raw JSON, or base64url-encoded
on GET/DELETE).

## Payload storage policy

Whether query text and matched chunk content are persisted is governed by the
per-release `payload_storage` setting:

| Value | Behavior |
|---|---|
| `per_request` | Follow the `store_payload` query param |
| `forced` | Always store text/content |
| `forbidden` | Never store (unless `playground=true` in the UI) |

Regardless of policy, `query_chunks` always records ids, scores, metadata, and
the step (`semantic` / `rerank`) for both search stages — only the human-readable
text is gated.

## Latency metrics

Each query records: `upstream_ms`, `embed_ms`, `search_ms`, `rerank_ms`,
`store_ms`, `total_ms`, plus `candidate_count` and `result_count`.

Inspect a single query, including its semantic and rerank steps:

```bash
curl -sS http://localhost:8080/api/v1/releases/first-release/queries/{id} \
  -H "Authorization: Bearer $TOKEN"
```

## Related

- [concepts.md](concepts.md) — release vs. stage, why production uses API keys
- [ingestion.md](ingestion.md) — attaching the metadata you filter on
- [operations.md](operations.md) — the Playground UI and analytics
