// SPDX-License-Identifier: AGPL-3.0-only

import { useEffect, useMemo, useState } from "react";
import { useParams, useSearchParams } from "react-router-dom";
import { FilterBuilder } from "../components/FilterBuilder";
import { InfoTip } from "../components/InfoTip";
import { QueryDetail, QueryResult, API_PREFIX, api } from "../api/client";
import { parseScore } from "../utils/score";

type TabMode = "ui" | "python" | "javascript" | "rust" | "java";

const TABS: { id: TabMode; label: string }[] = [
  { id: "ui", label: "UI" },
  { id: "python", label: "Python" },
  { id: "javascript", label: "Javascript" },
  { id: "rust", label: "Rust" },
  { id: "java", label: "Java" },
];

function CopySnippetButton({ text }: { text: string }) {
  const [copied, setCopied] = useState(false);

  async function copy() {
    await navigator.clipboard.writeText(text);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1500);
  }

  return (
    <button
      type="button"
      className="absolute right-2 top-2 rounded-md border px-2 py-1 text-xs transition-all"
      style={{
        borderColor: "var(--border)",
        background: copied ? "color-mix(in srgb, var(--accent) 25%, var(--surface))" : "var(--surface)",
        color: copied ? "var(--accent)" : "var(--muted)",
      }}
      onClick={() => void copy()}
      aria-label="Copy snippet"
    >
      {copied ? "Copied" : "Copy"}
    </button>
  );
}

function ResultChunkCard({
  sourceName,
  score,
  step,
  content,
  filtered,
  filterReason,
}: {
  sourceName: string;
  score: number;
  step: "semantic" | "rerank";
  content?: string;
  filtered: boolean;
  filterReason: string;
}) {
  return (
    <div
      className="rounded-lg border p-4"
      style={{
        borderColor: filtered ? "#ef4444" : "var(--border)",
        borderWidth: filtered ? 2 : 1,
      }}
    >
      <div className="flex items-start justify-between gap-2 text-sm text-[var(--muted)]">
        <span>
          {sourceName} · {step} {score.toFixed(4)}
        </span>
        {filtered && <InfoTip text={filterReason} wide tone="danger" />}
      </div>
      {content && <p className="mt-2 text-sm">{content}</p>}
    </div>
  );
}

function LatencyTimeline({
  upstreamMs,
  embedMs,
  searchMs,
  rerankMs,
  storeMs,
  downstreamMs,
}: {
  upstreamMs: number;
  embedMs: number;
  searchMs: number;
  rerankMs: number;
  storeMs: number;
  downstreamMs: number;
}) {
  const segments = [
    { key: "upstream", label: "upstream", ms: upstreamMs },
    { key: "embed", label: "embed", ms: embedMs },
    { key: "search", label: "search", ms: searchMs },
    { key: "rerank", label: "rerank", ms: rerankMs },
    { key: "store", label: "store", ms: storeMs },
    { key: "downstream", label: "downstream", ms: downstreamMs },
  ];

  const visualWeight = (ms: number) => (ms <= 0 ? 1 : ms);
  const totalVisual = segments.reduce((sum, s) => sum + visualWeight(s.ms), 0) || 1;
  const totalActual = segments.reduce((sum, s) => sum + s.ms, 0);

  return (
    <div className="w-full max-w-[1000px]">
      <div className="flex h-10 gap-1">
        {segments.map((seg) => {
          const isZero = seg.ms <= 0;
          const weight = visualWeight(seg.ms);
          return (
            <div
              key={seg.key}
              className="flex min-w-10 flex-col items-center justify-center rounded border px-1"
              style={{
                flex: `${weight} 1 0%`,
                borderColor: "var(--border)",
                background: isZero
                  ? "color-mix(in srgb, var(--surface) 92%, var(--muted) 8%)"
                  : "color-mix(in srgb, var(--surface) 85%, var(--accent) 15%)",
              }}
              title={`${seg.label}: ${seg.ms}ms (${Math.round((weight / totalVisual) * 100)}% visual)`}
            >
              <span className="truncate text-[10px] leading-tight">{seg.label}</span>
              <span className="text-[10px] tabular-nums leading-tight">{seg.ms}ms</span>
            </div>
          );
        })}
      </div>
      <div className="mt-2 flex justify-between text-xs text-[var(--muted)]">
        <span>t0 request</span>
        <span>total {totalActual}ms</span>
      </div>
    </div>
  );
}

export function PlaygroundPage() {
  const { releaseTag = "" } = useParams();
  const [params, setParams] = useSearchParams();
  const [tab, setTab] = useState<TabMode>("ui");
  const [text, setText] = useState(params.get("q") ?? "");
  const [topK, setTopK] = useState(Number(params.get("top_k") ?? 10));
  const [rerank, setRerank] = useState(params.get("rerank") !== "false");
  const [rerankCandidates, setRerankCandidates] = useState(Number(params.get("rerank_candidates") ?? 50));
  const [minSemanticScore, setMinSemanticScore] = useState(params.get("min_semantic_score") ?? "0");
  const [minRerankScore, setMinRerankScore] = useState(params.get("min_rerank_score") ?? "0");
  const [filter, setFilter] = useState(params.get("filter") ?? "");
  const [result, setResult] = useState<QueryResult | null>(null);
  const [detail, setDetail] = useState<QueryDetail | null>(null);
  const [clientEndMs, setClientEndMs] = useState<number | null>(null);
  const [clientStartMs, setClientStartMs] = useState<number | null>(null);
  const [loading, setLoading] = useState(false);

  const minSemantic = parseScore(minSemanticScore);
  const minRerank = parseScore(minRerankScore);

  const parsedFilter = (() => {
    if (!filter.trim()) return undefined;
    try {
      return JSON.parse(filter) as unknown;
    } catch {
      return undefined;
    }
  })();

  const queryBody = useMemo(
    () => [
      {
        text,
        top_k: topK,
        rerank,
        rerank_candidates: rerankCandidates,
        min_semantic_score: minSemantic,
        min_rerank_score: minRerank,
        ...(parsedFilter !== undefined ? { filter: parsedFilter } : {}),
      },
    ],
    [text, topK, rerank, rerankCandidates, minSemantic, minRerank, parsedFilter],
  );

  const body = JSON.stringify(queryBody, null, 2);
  const baseUrl = window.location.origin;
  const queryPath = `${API_PREFIX}/releases/${releaseTag}/queries?playground=true&store_payload=true&ts_start=`;

  const snippets: Record<Exclude<TabMode, "ui">, string> = {
    python: `import time\nimport requests\n\nts_start = int(time.time() * 1000)\nresp = requests.post(\n    "${baseUrl}${API_PREFIX}/releases/${releaseTag}/queries",\n    params={"playground": "true", "store_payload": "true", "ts_start": ts_start},\n    headers={"Authorization": "Bearer $RAGDOLL_TOKEN"},\n    json=${body},\n)\nprint(resp.json())`,
    javascript: `const tsStart = Date.now();\nconst resp = await fetch("${baseUrl}${queryPath}" + tsStart, {\n  method: "POST",\n  headers: {\n    Authorization: "Bearer $RAGDOLL_TOKEN",\n    "Content-Type": "application/json",\n  },\n  body: JSON.stringify(${body}),\n});\nconsole.log(await resp.json());`,
    rust: `let ts_start = chrono::Utc::now().timestamp_millis();\nlet client = reqwest::Client::new();\nlet resp = client\n    .post(format!("${baseUrl}${queryPath}{ts_start}"))\n    .bearer_auth(std::env::var("RAGDOLL_TOKEN")?)\n    .json(&${body})\n    .send()\n    .await?;\nprintln!("{:?}", resp.json::<serde_json::Value>().await?);`,
    java: `long tsStart = System.currentTimeMillis();\nString body = ${JSON.stringify(body)};\nHttpClient client = HttpClient.newHttpClient();\nHttpRequest request = HttpRequest.newBuilder()\n    .uri(URI.create("${baseUrl}${queryPath}" + tsStart))\n    .header("Authorization", "Bearer " + System.getenv("RAGDOLL_TOKEN"))\n    .header("Content-Type", "application/json")\n    .POST(HttpRequest.BodyPublishers.ofString(body))\n    .build();\nHttpResponse<String> response = client.send(request, HttpResponse.BodyHandlers.ofString());\nSystem.out.println(response.body());`,
  };

  useEffect(() => {
    const next = new URLSearchParams();
    if (text) next.set("q", text);
    next.set("top_k", String(topK));
    next.set("rerank", String(rerank));
    next.set("rerank_candidates", String(rerankCandidates));
    next.set("min_semantic_score", minSemanticScore);
    next.set("min_rerank_score", minRerankScore);
    if (filter) next.set("filter", filter);
    setParams(next, { replace: true });
  }, [text, topK, rerank, rerankCandidates, minSemanticScore, minRerankScore, filter, setParams]);

  async function runQuery() {
    setLoading(true);
    try {
      const tsStart = Date.now();
      setClientStartMs(tsStart);
      const res = await api<{ items: Array<{ result?: QueryResult }> }>(
        `/releases/${releaseTag}/queries?playground=true&store_payload=true&ts_start=${tsStart}`,
        { method: "POST", body: JSON.stringify(queryBody) },
      );
      setClientEndMs(Date.now());
      const query = res.items[0]?.result ?? null;
      setResult(query);
      if (query) {
        setDetail(await api<QueryDetail>(`/releases/${releaseTag}/queries/${query.query_id}`));
      } else {
        setDetail(null);
      }
    } finally {
      setLoading(false);
    }
  }

  const upstreamMs = detail?.upstream_ms ?? result?.latency.upstream_ms ?? 0;
  const embedMs = detail?.embed_ms ?? result?.latency.embed_ms ?? 0;
  const searchMs = detail?.search_ms ?? result?.latency.search_ms ?? 0;
  const rerankMs = detail?.rerank_ms ?? result?.latency.rerank_ms ?? 0;
  const storeMs = detail?.store_ms ?? result?.latency.store_ms ?? 0;

  const downstreamMs =
    clientStartMs != null && clientEndMs != null
      ? Math.max(
          0,
          clientEndMs -
            clientStartMs -
            (upstreamMs ?? 0) -
            embedMs -
            searchMs -
            (rerankMs ?? 0) -
            storeMs,
        )
      : 0;

  const semanticChunks = (detail?.chunks ?? []).filter((c) => c.step === "semantic");
  const rerankChunks = (detail?.chunks ?? []).filter((c) => c.step === "rerank");
  const semanticPassing = semanticChunks.filter((c) => c.score >= minSemantic).length;
  const rerankPassing = rerankChunks.filter((c) => c.score >= minRerank).length;

  return (
    <div className="space-y-6">
      <h2 className="text-2xl font-semibold">Playground</h2>

      <div className="card space-y-4">
        <div className="flex flex-wrap gap-2">
          {TABS.map(({ id, label }) => (
            <button
              key={id}
              type="button"
              className={`btn-secondary ${tab === id ? "ring-2 ring-[var(--accent)]" : ""}`}
              onClick={() => setTab(id)}
            >
              {label}
            </button>
          ))}
        </div>

        {tab === "ui" ? (
          <>
            <textarea
              className="input min-h-28"
              value={text}
              onChange={(e) => setText(e.target.value)}
              placeholder="Query text"
            />
            <div className="grid gap-4 md:grid-cols-3 lg:grid-cols-5">
              <label className="space-y-1 text-sm">
                <span className="inline-flex items-center">
                  top_k
                  <InfoTip text="Maximale Anzahl an Results in der finalen Response (nach Score-Filtern)." />
                </span>
                <input className="input" type="number" value={topK} onChange={(e) => setTopK(Number(e.target.value))} />
              </label>
              <label className="space-y-1 text-sm">
                <span className="inline-flex items-center">
                  rerank
                  <InfoTip text="Aktiviert den Cross-Encoder-Reranker." />
                </span>
                <select className="input" value={rerank ? "true" : "false"} onChange={(e) => setRerank(e.target.value === "true")}>
                  <option value="true">Aktiviert</option>
                  <option value="false">Deaktiviert</option>
                </select>
              </label>
              <label className="space-y-1 text-sm">
                <span className="inline-flex items-center">
                  rerank_candidates
                  <InfoTip text="Semantische Kandidaten für den Reranker." />
                </span>
                <input className="input" type="number" value={rerankCandidates} onChange={(e) => setRerankCandidates(Number(e.target.value))} />
              </label>
              <label className="space-y-1 text-sm">
                <span className="inline-flex items-center">
                  min_semantic_score
                  <InfoTip text="Harter Filter vor dem Rerank-Schritt. Default 0." />
                </span>
                <input
                  className="input"
                  type="text"
                  inputMode="decimal"
                  value={minSemanticScore}
                  onChange={(e) => setMinSemanticScore(e.target.value)}
                  placeholder="0.0"
                />
              </label>
              <label className="space-y-1 text-sm">
                <span className="inline-flex items-center">
                  min_rerank_score
                  <InfoTip text="Harter Filter nach dem Rerank-Schritt. Default 0. Im Playground werden alle Ergebnisse trotzdem angezeigt." />
                </span>
                <input
                  className="input"
                  type="text"
                  inputMode="decimal"
                  value={minRerankScore}
                  onChange={(e) => setMinRerankScore(e.target.value)}
                  placeholder="0.0"
                />
              </label>
            </div>
            <div className="space-y-2">
              <div className="inline-flex items-center text-sm font-medium">
                Filter
                <InfoTip text="Hard metadata filter applied before vector search." />
              </div>
              <FilterBuilder value={filter} onChange={setFilter} />
            </div>
            <button
              type="button"
              className="btn-run inline-flex items-center gap-2 disabled:cursor-not-allowed disabled:opacity-60"
              disabled={loading}
              onClick={() => void runQuery()}
            >
              {loading && (
                <span
                  className="inline-block h-4 w-4 animate-spin rounded-full border-2 border-white border-t-transparent"
                  aria-hidden
                />
              )}
              Run query
            </button>
          </>
        ) : (
          <div className="relative">
            <CopySnippetButton text={snippets[tab]} />
            <pre className="overflow-auto rounded-lg border p-4 pt-10 text-xs" style={{ borderColor: "var(--border)" }}>
              {snippets[tab]}
            </pre>
          </div>
        )}
      </div>

      {detail && (
        <>
          <div className="card space-y-4">
            <h3 className="font-medium">Timeline</h3>
            <LatencyTimeline
              upstreamMs={upstreamMs ?? 0}
              embedMs={embedMs}
              searchMs={searchMs}
              rerankMs={rerankMs ?? 0}
              storeMs={storeMs}
              downstreamMs={downstreamMs}
            />
          </div>

          <div className="grid gap-4 lg:grid-cols-2">
            <div className="card space-y-4">
              <div>
                <h3 className="font-medium">Semantic Results</h3>
                <p className="text-sm text-[var(--muted)]">
                  {semanticPassing} pass filter · {semanticChunks.length} total
                </p>
              </div>
              {semanticChunks.length === 0 ? (
                <p className="text-sm text-[var(--muted)]">No semantic results.</p>
              ) : (
                semanticChunks.map((c) => (
                  <ResultChunkCard
                    key={`${c.chunk_id}-semantic`}
                    sourceName={c.source_name ?? c.source_id}
                    score={c.score}
                    step="semantic"
                    content={c.content}
                    filtered={c.score < minSemantic}
                    filterReason="Score below min_semantic_score — excluded before reranking in production queries."
                  />
                ))
              )}
            </div>

            <div className="card space-y-4">
              <div>
                <h3 className="font-medium">Rerank Results</h3>
                <p className="text-sm text-[var(--muted)]">
                  {rerank ? `${rerankPassing} pass filter · ${rerankChunks.length} total` : "Reranking disabled"}
                </p>
              </div>
              {!rerank ? (
                <p className="text-sm text-[var(--muted)]">Reranking disabled.</p>
              ) : rerankChunks.length === 0 ? (
                <p className="text-sm text-[var(--muted)]">No rerank results.</p>
              ) : (
                rerankChunks.map((c) => (
                  <ResultChunkCard
                    key={`${c.chunk_id}-rerank`}
                    sourceName={c.source_name ?? c.source_id}
                    score={c.score}
                    step="rerank"
                    content={c.content}
                    filtered={c.score < minRerank}
                    filterReason="Filtered after reranking because the score was below min_rerank_score."
                  />
                ))
              )}
            </div>
          </div>
        </>
      )}
    </div>
  );
}
