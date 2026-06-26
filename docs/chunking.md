# Chunking (Semantic Split)

Chunk quality is the single biggest lever on retrieval quality. Ragdoll uses one
ingest chunking strategy: **Semantic Split** (`chunking_strategy:
semantic_split`). It splits documents into retrieval-sized chunks by embedding
similarity, but only at **structural boundaries** — never in the middle of a
list item, code block, or table row.

You can use Ragdoll well without reading this page. Read it when you want to tune
chunk size or understand why a chunk looks the way it does.

## Pipeline

```text
extract text
    → parse atomic units (headings, paragraphs, lists, code, tables)
    → embed unit windows → find semantic breakpoints between units
    → subdivide oversized groups (sentence fallback inside paragraphs only)
    → pack to min/max token bounds (merge only within the same section)
    → assemble chunk text + section prefix → embed final chunks
```

Quality is prioritized over ingest speed: boundary detection and final embedding
each run a batched ONNX pass.

## Atomic units

Before semantic splitting, the document is parsed into **atomic units** — the
smallest pieces that must stay intact:

| Unit kind | Splittable | Examples |
|---|---|---|
| `heading` | no | `# Installation`, `## macOS` |
| `paragraph` | yes | Blank-line-separated prose blocks |
| `list_item` | no | `- item`, `1. item` |
| `code_block` | no | Fenced ` ``` ` blocks |
| `table_row` | no | Markdown `\| col \| col \|` rows |
| `blockquote` | yes | `> quoted text` |

How units are detected depends on the input:

- **Markdown** (`.md` files, or content with heading/list/code signals) is parsed
  line-by-line with heading hierarchy tracked as `section_path`.
- **Plain text** (PDF, DOCX, TXT, …) uses paragraph breaks (`\n\n`) and list-line
  heuristics.
- **URLs** are extracted via trafilatura as Markdown, so web-page headings and
  lists survive into the unit layer.

## Semantic boundaries

Adjacent units are embedded with a sliding context window (`sentence_buffer`,
default `2`). The cosine distance between consecutive window embeddings measures
topic shift. A breakpoint is placed wherever that distance exceeds the
`breakpoint_percentile` threshold (default `95`).

When two units belong to different sections (`section_path` changes), the
distance is boosted so splits prefer section boundaries.

If a group still exceeds `max_chunk_tokens`:

- **Multiple units** → recursive semantic split on that subset.
- **One splittable paragraph** → sentence split (pysbd, German default) using the
  same breakpoint logic.
- **One non-splittable unit** (e.g. a long list item) → kept whole; a warning is
  logged if it exceeds the token limit.

Chunks are never cut by raw token count mid-unit.

## Chunk output

Each stored chunk includes:

- **Content** — units joined with blank lines; list markers and code fences
  preserved.
- **Section prefix** — `[Installation > macOS]` prepended when the chunk has a
  heading path, which improves retrieval context.
- **Metadata** — source metadata plus `section_path` and `unit_kinds` when
  applicable.
- **Provenance** — character offsets into the extracted source text.

## Tuning

These are per-release settings (see [configuration.md](configuration.md)):

| Setting | Default | Role |
|---|---|---|
| `sentence_buffer` | `2` | Neighbor units included in each boundary embedding window |
| `breakpoint_percentile` | `95` | Higher → fewer, larger chunks; lower → more splits |
| `min_chunk_tokens` | `64` | Undersized chunks merge with the next chunk in the same section |
| `max_chunk_tokens` | `512` | Upper bound before subdivision |

> **Changing chunking behavior requires re-ingest.** Existing chunks are not
> re-split when you change these settings. Re-ingest the affected sources to
> apply new chunk bounds. See [pitfalls.md](pitfalls.md).

A future **Semantic Group** strategy is planned on the same atomic-unit layer; it
is not available yet.

## Related

- [ingestion.md](ingestion.md) — how text reaches this pipeline
- [configuration.md](configuration.md) — where to set these knobs
- [querying.md](querying.md) — how chunks are searched and reranked
