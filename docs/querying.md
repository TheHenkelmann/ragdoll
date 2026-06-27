# Querying

Retrieval is the read side of Ragdoll: embed the query, apply hard metadata
filters, run exact cosine search over the matching chunk rows, and optionally
rerank with a cross-encoder. By default Ragdoll is **retrieval-only** — it
returns ranked chunks. Optionally, include a `generation` object to synthesize an
answer with your own external LLM (BYO-LLM).

## Where queries go

For production, queries go through a **stage** (or release) with an **API key**:

```bash
curl -sS -X POST 'http://localhost:8080/api/v1/stages/prod/queries?store_payload=false' \
  -H "Authorization: Bearer $API_KEY" \
  -H 'Content-Type: application/json' \
  -d '[{"text":"local RAG pipeline","top_k":10,"rerank":true,"rerank_candidates":20}]'
```

During development the UI **Playground** uses a dedicated session-only endpoint:

```bash
curl -sS -X POST 'http://localhost:8080/api/v1/playground/first-release/queries' \
  -H "Authorization: Bearer $SESSION_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '[{"text":"local RAG pipeline","top_k":10,"rerank":true}]'
```

Release and stage query endpoints (`/api/v1/releases/{tag}/queries`,
`/api/v1/stages/{tag}/queries`) require an **API key** — session tokens are
rejected. See [concepts.md](concepts.md).

Queries are a **batch** endpoint: send an array, get a multi-status response.

## Per-request body fields

| Field | Default | Description |
|---|---|---|
| `text` | required | Query string |
| `top_k` | `10` | Final results returned |
| `rerank` | `true` | Enable the cross-encoder reranker |
| `rerank_candidates` | `20` | Semantic pool size before rerank |
| `min_semantic_score` | `0.5` | Minimum cosine similarity before a candidate enters reranking |
| `min_rerank_score` | `0.5` | Minimum normalized rerank score in the final response |
| `filter` | none | Hard filter DSL (see below) |
| `hybrid` | `false` | Fuse vector search with BM25 full-text ranks (RRF) |
| `bm25_weight` | `1.0` | Weight of the BM25 rank list in hybrid fusion |
| `generation` | none | Optional BYO-LLM answer generation (see below) |

Query knobs are **per request only**; there is no per-release default for them
(the gateway has hardcoded fallbacks).

Per-release `rerank_max_length` (Settings UI) caps how many tokens of each
document are passed to the cross-encoder. Lower values reduce rerank latency;
`256` is the recommended default.

### Hybrid search (BM25 + vector)

Set `"hybrid": true` to combine cosine vector ranks with BM25 full-text search
over `chunks_fts`, fused via reciprocal rank fusion (RRF). Useful when queries
contain rare keywords, SKUs, or exact phrases that pure embedding search may miss.

| Field | Default | Description |
|---|---|---|
| `hybrid` | `false` | Enable hybrid retrieval |
| `bm25_weight` | `1.0` | Relative weight of the BM25 ranking (0 = vector-only) |

Hybrid runs before reranking. The Playground UI does not expose this knob yet;
use the API directly.

### Citations

Each match in the response includes a `citation` object with provenance:

| Field | Description |
|---|---|
| `citation_id` | Stable id (`{chunk_id}:{embedding_version}`) |
| `source_id`, `source_name`, `source_type` | Originating source |
| `uri` | Original URL for `url` sources (if set) |
| `char_start`, `char_end` | Offsets in extracted source text |
| `page` | PDF page when page map is available |
| `section_path` | Heading hierarchy from chunk metadata |
| `snippet` | Surrounding text excerpt (when enabled) |

Citations are included in LLM generation prompts so answers can reference sources.
See [operations.md → BYO-LLM](operations.md#byo-llm-generation-optional).

### Optional answer generation

When the `generation` object is present, Ragdoll runs retrieval first, then calls
your configured external LLM with the top chunks as context. Generation is
opt-in per request and gated by:

1. `generation_allowed` on the release (default `true`)
2. At least one LLM model configured on the release
3. `generation.tag` is required and must reference an existing release model

| Field | Default | Description |
|---|---|---|
| `stream` | `false` | When `true`, response is SSE (single item only) |
| `tag` | — | Required LLM model tag from this release |
| `system_prompt` | — | **Required.** System prompt sent to the LLM |
| `temperature` | `1` | Sampling temperature |
| `max_tokens` | `5096` | Maximum output tokens |

Retrieved chunks are always labeled with source metadata in the user message.

**Three response modes** on the same `POST .../queries` endpoint:

| `generation` | `stream` | Response |
|---|---|---|
| absent | — | Batch retrieval, multi-status JSON (default) |
| present | `false` | Batch sync generation (parallel, cap 100), multi-status JSON with `answer` |
| present | `true` | Single item only (`400` if batch length ≠ 1), SSE stream |

Sync generation example:

```bash
curl -sS -X POST 'http://localhost:8080/api/v1/stages/prod/queries' \
  -H "Authorization: Bearer $API_KEY" \
  -H 'Content-Type: application/json' \
  -d '[{
    "text": "What is the remote work policy?",
    "top_k": 8,
    "rerank": true,
    "generation": { "stream": false, "tag": "gpt4o-prod", "system_prompt": "Answer using only the provided sources." }
  }]'
```

Each successful item includes:

- `answer` — `text`, `llm_model_id`, `llm_model_tag`
- `latency` — pipeline timings plus `generation_ms`, `generation_total_ms`, and `total_ragdoll_ms` (request-in to request-out inside Ragdoll, including generation)
- `usage` — `prompt_tokens`, `completion_tokens` (when reported by the provider)

#### Streaming (SSE)

Set `"stream": true` and send **exactly one** query item. The response is
`text/event-stream`:

| Event | Payload |
|---|---|
| `sources` | JSON array of retrieval matches (before tokens) |
| `latency` | Per-segment update `{"segment":"embed_ms","ms":18,"final":false}` or full search-phase snapshot before tokens |
| `token` | `{"delta": "..."}` |
| `done` | `query_id`, `text` (full answer), final `latency`, `usage` |
| `error` | Error message on failure |

After `sources`, Ragdoll emits one `latency` event per search-phase segment
(`upstream_ms` through `store_ms`), then a combined snapshot. Generation timings
arrive only in the final `done` event. The `text` field in `done` is the
authoritative full answer (use it to reconcile streamed tokens).

Playground example (session token):

```bash
curl -N -X POST 'http://localhost:8080/api/v1/playground/first-release/queries' \
  -H "Authorization: Bearer $SESSION_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '[{"text":"summarize the policy","top_k":6,"generation":{"stream":true}}]'
```

## Query parameters

| Param | Default | Description |
|---|---|---|
| `store_payload` | `false` | Persist query text and chunk content in the DB |
| `ts_start` | none | Epoch ms anchor for `upstream_ms` latency |
| `playground` | n/a (use `/playground/{tag}/queries`) | Playground UI uses a dedicated endpoint; always stores payloads |

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
  -H "Authorization: Bearer $API_KEY" \
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
`store_ms`, `total_ragdoll_ms`, plus `candidate_count` and `result_count`. When
generation ran, the query row also stores `generation_ms` (time-to-first-token),
`generation_total_ms`, `prompt_tokens`, `completion_tokens`, and `llm_model_id`.

API responses expose timings under `latency` and token counts under `usage`.
`total_ragdoll_ms` is the wall time from request arrival until the response
leaves Ragdoll, including generation.

Inspect a single query, including its semantic and rerank steps:

```bash
curl -sS http://localhost:8080/api/v1/releases/first-release/queries/{id} \
  -H "Authorization: Bearer $API_KEY"
```

## Related

- [models.md](models.md) — embedding and rerank models
- [concepts.md](concepts.md) — release vs. stage, why production uses API keys
- [ingestion.md](ingestion.md) — attaching the metadata you filter on
- [operations.md](operations.md) — the Playground UI and analytics
