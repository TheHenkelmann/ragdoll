# Retrieval models (embedding & rerank)

Ragdoll runs **local ONNX models** for retrieval: an **embedding model** turns
text into vectors, and an optional **rerank model** refines query results. Both
are downloaded from Hugging Face and cached on disk — no external API calls at
query time.

This page covers retrieval models only. For optional **answer generation** with
your own external LLM, see [operations.md → BYO-LLM generation](operations.md#byo-llm-generation-optional).

> **Naming in the UI:** the primary-sidebar **Models** page manages ONNX
> download, verification, and local cleanup. Per-release **Settings** only
> let you **select** models that are already downloaded. Per-release **LLM
> Models** configures external generation APIs — a different surface.

## Two layers in the RAG stack

```text
Ingest:  extract → chunk → [Embedding model] → vectors in libSQL
Query:   question → [Embedding model] → vector search → candidates
                              → optional [Rerank model] → top-K chunks
Optional: top-K → [LLM model] → synthesized answer (BYO-LLM)
```

| Model | When it runs | Configured where | Artifacts stored |
|---|---|---|---|
| **Embedding** | Ingest (every chunk) + every query | Per release (`embedding_model` in Settings) | Instance-wide under `${RAGDOLL_DATA_DIR}/models/` |
| **Rerank** | Query time only (on semantic candidates) | Per release (`rerank_model` in Settings) | Same cache directory |

Artifact files are **shared across all releases**. The **Models** page
downloads and verifies ONNX files; **Settings** picks which downloaded model
each release uses.

## What is an embedding model?

An embedding model maps text to a fixed-size vector (Ragdoll: **1024 dimensions**).
At ingest, every chunk is embedded and stored in libSQL. At query time, the
question is embedded the same way and compared with cosine similarity against
filtered chunk rows.

Changing the embedding model changes the **vector space**. Existing chunk vectors
were produced by the old model and will not align with new queries until you
**re-embed** (reindex or re-ingest). See [Changing models](#changing-models).

## What is a rerank model?

A rerank model is a **cross-encoder**: it scores each *(query, chunk)* pair
directly, which is more accurate than vector similarity alone but slower. Ragdoll
runs it on the top `rerank_candidates` semantic hits before returning `top_k`
results.

Changing the rerank model only requires downloading the new ONNX artifacts —
**no re-ingest**.

## Where models come from

1. **First boot** — Ragdoll downloads the default embedding and rerank models
   (~2 GB total) into `${RAGDOLL_DATA_DIR}/models/`. Until complete,
   `/api/v1/health` reports `"ready": false`.
2. **Models page** — unified catalog table: download with progress, verify,
   unload from RAM, delete local artifacts. Add custom Hugging Face `org/model`
   ids via **Add custom model**.
3. **Settings** — dropdown lists **only downloaded** models. Download missing
   models on the [Models page](/models) first.
4. **API** — programmatic status, download, test, and delete:

```bash
# Full status: catalog, local artifacts, required/missing, active downloads
curl -sS http://localhost:8080/api/v1/models/status \
  -H "Authorization: Bearer $TOKEN"

# SSE download with progress + auto-verify
curl -sN http://localhost:8080/api/v1/models/BAAI/bge-m3/download/stream \
  -H "Authorization: Bearer $TOKEN"

# Verify without re-downloading
curl -sS -X POST http://localhost:8080/api/v1/models/BAAI/bge-m3/test \
  -H "Authorization: Bearer $TOKEN"
```

Permissions: `models:read` (status/list/test), `models:download` (download),
`models:delete` (delete local artifacts).

### Models page

The primary-sidebar **Models** page is the single operational view:

| Column | Meaning |
|---|---|
| **Model** | Hugging Face id with link to model card |
| **Kind** | `embed` or `rerank` |
| **Languages** | Primary language coverage |
| **Releases** | Releases referencing this model in Settings |
| **Download Status** | Present / Download Now / progress bar |
| **RAM** | Estimated gateway memory when loaded (~ONNX size) + Unload |
| **Actions** | Test inference / Delete local files |

Rows sort by download status (downloading → present → missing), then
alphabetically. Not-yet-downloaded models appear dimmed.

On page reload, active downloads reconnect automatically via SSE.

### Offline / air-gapped

1. Pre-populate `${RAGDOLL_DATA_DIR}/models/` with ONNX artifacts for your
   chosen whitelist models.
2. Set `RAGDOLL_HF_HUB_OFFLINE=1` (optional `RAGDOLL_HF_TOKEN` if needed).

See [operations.md → Offline mode](operations.md#offline-mode).

## The 1024-dimension limit

Ragdoll supports a curated whitelist — all embedding models produce
**1024-dimensional** vectors. libSQL stores vectors at a fixed
`RAGDOLL_EMBEDDING_DIM` (default `1024`). Mixing dimensions in one database is
not supported.

## Predefined models

All models below are in the Ragdoll catalog. Defaults are marked with ✓.

### Embedding models

| Model | Languages | Pros | Cons |
|---|---|---|---|
| `BAAI/bge-m3` ✓ | multilingual | Strong general-purpose default; good multilingual balance | Larger/slower than English-only models |
| `BAAI/bge-large-en-v1.5` | en | High English retrieval quality | English-focused; not ideal for multilingual corpora |
| `mixedbread-ai/mxbai-embed-large-v1` | en | Compact and fast for English | English only |
| `intfloat/multilingual-e5-large` | multilingual | Solid multilingual baseline; Ragdoll applies `query:`/`passage:` prefixes | Requires prefix handling (automatic in Ragdoll) |
| `Snowflake/snowflake-arctic-embed-l-v2.0` | multilingual | Modern multilingual embedder tuned for search | User-defined ONNX loader; verify after first download |
| `mixedbread-ai/deepset-mxbai-embed-de-large-v1` | de, en | Strong German + English mixed corpora | Smaller community than BGE defaults |
| `jinaai/jina-embeddings-v3` | multilingual | Strong cross-lingual performance | User-defined loader; larger download |
| `intfloat/multilingual-e5-large-instruct` | multilingual | Instruction-tuned E5; good with formatted queries | Uses E5-style prefixes; slightly more setup sensitivity |
| `Alibaba-NLP/gte-large-en-v1.5` | en | High-quality English embeddings | English only |

### Rerank models

| Model | Languages | Pros | Cons |
|---|---|---|---|
| `BAAI/bge-reranker-v2-m3` ✓ | multilingual | Default multilingual reranker; well tested in Ragdoll | Cross-encoder latency on large candidate sets |
| `jinaai/jina-reranker-v2-base-multilingual` | multilingual | Good multilingual alternative to BGE reranker | Different latency profile |
| `mixedbread-ai/mxbai-rerank-base-v1` | en | Lightweight English reranker | English only |

**Choosing models:** start with the defaults (`bge-m3` + `bge-reranker-v2-m3`).
For mostly English content, try `bge-large-en-v1.5` + `mxbai-rerank-base-v1`.
For German-focused RAG, consider `deepset-mxbai-embed-de-large-v1` (download and
verify first).

Per-release selection: download on the **Models** page, then **Settings →
Models** (or `PATCH .../settings` with `embedding_model` / `rerank_model`).

## Changing models

| Change | Download on Models page? | Re-embed / reindex? |
|---|---|---|
| Embedding model | Yes (before selecting in Settings) | **Yes** |
| Rerank model | Yes (before selecting in Settings) | No |
| `rerank_max_length` | No | No |

After an embedding model change:

1. Download the new model on the Models page.
2. Save Settings with the new model.
3. Trigger **reindex** from Settings, or re-ingest affected sources.

## Performance tuning

| Variable | Default | Effect |
|---|---|---|
| `RAGDOLL_ONNX_NUM_THREADS` | `4` | ONNX intra-op threads per model instance |
| `RAGDOLL_RERANK_POOL_SIZE` | `1` | Parallel reranker instances (more RAM, less queueing) |

## Related

- [configuration.md](configuration.md) — env vars and per-release settings
- [operations.md](operations.md) — UI, offline mode, BYO-LLM generation
- [querying.md](querying.md) — rerank knobs and score thresholds
- [pitfalls.md](pitfalls.md) — re-ingest traps and dimension whitelist
