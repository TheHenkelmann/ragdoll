// SPDX-License-Identifier: AGPL-3.0-only

/** Short UI tips keyed by catalog doc_slug (see docs/models.md for full pros/cons). */
export const MODEL_TIPS: Record<string, string> = {
  "bge-m3":
    "Multilingual general-purpose default. Strong balance of quality and speed across many languages.",
  "bge-large-en":
    "English-focused embedding with high retrieval quality. Best when your corpus is mostly English.",
  "mxbai-embed-large":
    "Compact English embedding model. Lower latency than larger models with good English quality.",
  "multilingual-e5-large":
    "Multilingual E5 model. Ragdoll applies query:/passage: prefixes automatically.",
  "snowflake-arctic-embed-l-v2":
    "Snowflake Arctic Embed v2 — multilingual retrieval model tuned for search workloads.",
  "deepset-mxbai-embed-de":
    "German/English embedding from deepset and Mixedbread. Good for DACH + English mixed corpora.",
  "jina-embeddings-v3":
    "Jina v3 multilingual embeddings with strong cross-lingual retrieval performance.",
  "multilingual-e5-large-instruct":
    "Instruction-tuned E5 large model. Uses query:/passage: prefixes like standard E5.",
  "gte-large-en":
    "Alibaba GTE large English model. High-quality English embeddings at 1024 dimensions.",
  "bge-reranker-v2-m3":
    "Multilingual reranker default. Cross-encoder refinement after vector search.",
  "jina-reranker-v2":
    "Multilingual Jina reranker. Good alternative to BGE reranker v2-m3.",
  "mxbai-rerank-base-v1":
    "English reranker. Pair with a multilingual embedder for mixed-language corpora.",
};

export const COLUMN_TIPS = {
  model: "Hugging Face model id. Click ↗ to open the model card on huggingface.co.",
  kind: "embed: vectors at ingest + query. rerank: refines top semantic hits at query time only.",
  languages: "Primary language coverage supported by this model.",
  releases:
    "Releases that reference this model in Settings (embedding_model or rerank_model).",
  download:
    "Present = ONNX artifacts on disk and verified. Download Now = fetch from Hugging Face.",
  ram: "Estimated gateway RAM when the model ONNX session is loaded (~artifact size). Unload drops it from memory.",
  actions: "Test runs a quick inference. Delete removes local ONNX files from disk.",
} as const;

export function hfModelUrl(name: string): string {
  return `https://huggingface.co/${name}`;
}

export function formatRam(bytes: number | null | undefined): string {
  if (bytes == null || bytes === 0) return "—";
  const mb = bytes / (1024 * 1024);
  if (mb >= 1024) return `~${(mb / 1024).toFixed(1)} GB`;
  return `~${Math.round(mb)} MB`;
}

export type DownloadSortGroup = "downloading" | "present" | "missing";

export function downloadSortGroup(
  name: string,
  present: boolean,
  activeDownloads: string[],
  rowStatus?: string,
): DownloadSortGroup {
  if (rowStatus === "downloading" || activeDownloads.includes(name)) return "downloading";
  if (present) return "present";
  return "missing";
}

export function compareCatalogRows(
  a: { name: string; present: boolean },
  b: { name: string; present: boolean },
  activeDownloads: string[],
  rowState: Record<string, { status?: string }>,
): number {
  const groupOrder: Record<DownloadSortGroup, number> = {
    downloading: 0,
    present: 1,
    missing: 2,
  };
  const ga = downloadSortGroup(
    a.name,
    a.present,
    activeDownloads,
    rowState[a.name]?.status,
  );
  const gb = downloadSortGroup(
    b.name,
    b.present,
    activeDownloads,
    rowState[b.name]?.status,
  );
  const cmp = groupOrder[ga] - groupOrder[gb];
  if (cmp !== 0) return cmp;
  return a.name.localeCompare(b.name);
}

export function filterCatalogRows<T extends { name: string; releases: string[] }>(
  rows: T[],
  search: string,
): T[] {
  const q = search.trim().toLowerCase();
  if (!q) return rows;
  return rows.filter(
    (row) =>
      row.name.toLowerCase().includes(q) ||
      row.releases.some((tag) => tag.toLowerCase().includes(q)),
  );
}
