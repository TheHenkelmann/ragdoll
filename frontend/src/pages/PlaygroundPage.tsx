// SPDX-License-Identifier: AGPL-3.0-only

import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import type { TextareaHTMLAttributes } from "react";
import { useParams, useSearchParams } from "react-router-dom";
import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { FilterBuilder } from "../components/FilterBuilder";
import { InfoTip } from "../components/InfoTip";
import { PermissionDenied } from "../components/PermissionDenied";
import { usePermissions } from "../hooks/usePermissions";
import { PERM } from "../permissions";
import {
  DEFAULT_GENERATION_MAX_TOKENS,
  DEFAULT_GENERATION_SYSTEM_PROMPT,
  DEFAULT_GENERATION_TEMPERATURE,
  QueryDetail,
  QueryMatch,
  QueryResult,
  LlmModelRecord,
  StreamLatencySnapshot,
  api,
  runPlaygroundQueryStream,
} from "../api/client";
import { parseScore } from "../utils/score";

import { buildSnippets } from "../utils/querySnippets";

type TabMode = "ui" | "curl" | "python" | "javascript" | "go" | "rust" | "java";

const TABS: { id: TabMode; label: string }[] = [
  { id: "ui", label: "UI" },
  { id: "curl", label: "cURL" },
  { id: "python", label: "Python" },
  { id: "javascript", label: "Javascript" },
  { id: "go", label: "Go" },
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

const ERROR_BORDER = { borderColor: "#ef4444" } as const;

function AutoGrowTextarea({
  value,
  minRows = 5,
  maxRows = 20,
  maxViewportRatio = 0.6,
  style,
  ...props
}: {
  value: string;
  minRows?: number;
  maxRows?: number;
  maxViewportRatio?: number;
} & TextareaHTMLAttributes<HTMLTextAreaElement>) {
  const ref = useRef<HTMLTextAreaElement>(null);

  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    el.style.height = "auto";
    const cs = window.getComputedStyle(el);
    const lineHeight = parseFloat(cs.lineHeight) || 20;
    const paddingV = parseFloat(cs.paddingTop) + parseFloat(cs.paddingBottom);
    const borderV = parseFloat(cs.borderTopWidth) + parseFloat(cs.borderBottomWidth);
    const chrome = paddingV + borderV;
    const minPx = lineHeight * minRows + chrome;
    const maxByRows = lineHeight * maxRows + chrome;
    const maxPx = Math.min(maxByRows, window.innerHeight * maxViewportRatio);
    const next = Math.max(minPx, Math.min(el.scrollHeight, maxPx));
    el.style.height = `${next}px`;
    el.style.overflowY = el.scrollHeight > maxPx ? "auto" : "hidden";
  }, [value, minRows, maxRows, maxViewportRatio]);

  return (
    <textarea
      ref={ref}
      rows={minRows}
      value={value}
      style={{ resize: "none", ...style }}
      {...props}
    />
  );
}

function decodeUrlParam(raw: string | null): string {
  if (!raw) return "";
  try {
    return decodeURIComponent(raw);
  } catch {
    return raw;
  }
}

function msWithPct(ms: number, totalMs: number, growing = false): string {
  if (growing) return "…";
  if (totalMs <= 0) return `${ms}ms`;
  const pct = Math.round((ms / totalMs) * 100);
  return pct >= 5 ? `${ms}ms (${pct}%)` : `${ms}ms`;
}

function TimelineBlock({
  label,
  ms,
  index,
  growing,
  totalMs,
  visualWeight,
  totalVisual,
}: {
  label: string;
  ms: number;
  index: number;
  growing: boolean;
  totalMs: number;
  visualWeight: number;
  totalVisual: number;
}) {
  const isZero = ms <= 0;
  const labelOnTop = index % 2 === 0;
  const labelEl = (
    <span className="truncate text-[10px] font-medium leading-tight">{label}</span>
  );
  const msEl = (
    <span className="text-[10px] tabular-nums leading-tight text-[var(--muted)]">
      {msWithPct(ms, totalMs, growing)}
    </span>
  );

  return (
    <div
      className="flex min-w-10 flex-col items-center justify-center rounded border px-1"
      style={{
        flex: `${visualWeight} 1 0%`,
        borderColor: "var(--border)",
        background: isZero
          ? "color-mix(in srgb, var(--surface) 92%, var(--muted) 8%)"
          : growing
            ? "color-mix(in srgb, var(--surface) 75%, var(--accent) 25%)"
            : "color-mix(in srgb, var(--surface) 85%, var(--accent) 15%)",
      }}
      title={`${label}: ${ms}ms (${Math.round((visualWeight / totalVisual) * 100)}% visual)`}
    >
      <div className="flex flex-col items-center gap-0.5">
        {labelOnTop ? [labelEl, msEl] : [msEl, labelEl]}
      </div>
    </div>
  );
}

const PROGRESS_ASSUMED_TOTAL_MS = 10_000;

type TimelineSegmentDef = {
  key: string;
  label: string;
  latencyKey?: keyof StreamLatencySnapshot;
  optional: boolean;
};

const TIMELINE_SEGMENT_DEFS: TimelineSegmentDef[] = [
  { key: "upstream", label: "upstream", latencyKey: "upstream_ms", optional: false },
  { key: "embed", label: "embed", latencyKey: "embed_ms", optional: false },
  { key: "search", label: "search", latencyKey: "search_ms", optional: false },
  { key: "rerank", label: "rerank", latencyKey: "rerank_ms", optional: true },
  { key: "store", label: "store", latencyKey: "store_ms", optional: false },
  { key: "generation", label: "generation", optional: true },
  { key: "downstream", label: "downstream", optional: false },
];

const LATENCY_SEGMENT_KEYS = [
  "upstream_ms",
  "embed_ms",
  "search_ms",
  "rerank_ms",
  "store_ms",
  "generation_ms",
] as const;

function hasRerank(latency: StreamLatencySnapshot): boolean {
  return latency.rerank_ms != null;
}

function hasGeneration(latency: StreamLatencySnapshot): boolean {
  return latency.generation_total_ms != null || latency.generation_ms != null;
}

function segmentMsFromLatency(def: TimelineSegmentDef, latency: StreamLatencySnapshot): number | undefined {
  if (def.key === "generation") {
    if (!hasGeneration(latency)) return undefined;
    return latency.generation_total_ms ?? latency.generation_ms;
  }
  if (!def.latencyKey) return undefined;
  const value = latency[def.latencyKey];
  return typeof value === "number" ? value : undefined;
}

function isSegmentVisible(def: TimelineSegmentDef, latency: StreamLatencySnapshot): boolean {
  if (def.key === "rerank") return hasRerank(latency);
  if (def.key === "generation") return hasGeneration(latency);
  return true;
}

function visibleSegmentDefs(latency: StreamLatencySnapshot): TimelineSegmentDef[] {
  return TIMELINE_SEGMENT_DEFS.filter((def) => isSegmentVisible(def, latency));
}

function growingSegmentIndex(latency: StreamLatencySnapshot, complete: boolean): number {
  if (complete) return -1;
  const defs = visibleSegmentDefs(latency);
  for (let i = 0; i < defs.length; i++) {
    const def = defs[i];
    if (def.key === "downstream") return i;
    const ms = segmentMsFromLatency(def, latency);
    if (ms === undefined) return i;
  }
  return defs.length - 1;
}

function applyLatencyEvent(
  prev: StreamLatencySnapshot,
  data: Record<string, unknown>,
): StreamLatencySnapshot {
  const next = { ...prev };
  if (typeof data.segment === "string" && typeof data.ms === "number") {
    next[data.segment as keyof StreamLatencySnapshot] = data.ms;
  }
  for (const key of LATENCY_SEGMENT_KEYS) {
    if (typeof data[key] === "number") {
      next[key] = data[key] as number;
    }
  }
  if (typeof data.total_ragdoll_ms === "number") {
    next.total_ragdoll_ms = data.total_ragdoll_ms;
  }
  if (typeof data.generation_ms === "number") {
    next.generation_ms = data.generation_ms;
  }
  if (typeof data.generation_total_ms === "number") {
    next.generation_total_ms = data.generation_total_ms;
  }
  return next;
}

function snapshotToSegments(
  latency: StreamLatencySnapshot,
  clientStartMs: number | null,
  clientEndMs: number | null,
): Array<{ key: string; label: string; ms: number }> {
  const upstream = latency.upstream_ms ?? 0;
  const embed = latency.embed_ms ?? 0;
  const search = latency.search_ms ?? 0;
  const rerank = latency.rerank_ms ?? 0;
  const store = latency.store_ms ?? 0;
  const generation = latency.generation_total_ms ?? latency.generation_ms ?? 0;
  const totalRagdoll =
    latency.total_ragdoll_ms ?? upstream + embed + search + (hasRerank(latency) ? rerank : 0) + store + (hasGeneration(latency) ? generation : 0);
  const downstream =
    clientStartMs != null && clientEndMs != null
      ? Math.max(0, clientEndMs - clientStartMs - totalRagdoll)
      : 0;

  const byKey: Record<string, number> = {
    upstream,
    embed,
    search,
    rerank,
    store,
    generation,
    downstream,
  };

  return visibleSegmentDefs(latency).map((def) => ({
    key: def.key,
    label: def.label,
    ms: byKey[def.key] ?? 0,
  }));
}

function ProgressiveLatencyTimeline({
  latency,
  complete,
  clientStartMs,
  clientEndMs,
}: {
  latency: StreamLatencySnapshot;
  complete: boolean;
  clientStartMs: number | null;
  clientEndMs: number | null;
}) {
  const [tick, setTick] = useState(0);
  const growAnchorRef = useRef(Date.now());
  const latencyKey = JSON.stringify(latency);

  useEffect(() => {
    growAnchorRef.current = Date.now();
  }, [latencyKey]);

  useEffect(() => {
    if (complete) return;
    const id = window.setInterval(() => setTick((t) => t + 1), 50);
    return () => clearInterval(id);
  }, [complete]);

  const visualWeight = (ms: number) => (ms <= 0 ? 1 : ms);

  if (complete && clientStartMs != null && clientEndMs != null) {
    const segments = snapshotToSegments(latency, clientStartMs, clientEndMs);
    const totalWallMs = segments.reduce((sum, s) => sum + s.ms, 0);
    const totalVisual = segments.reduce((sum, s) => sum + visualWeight(s.ms), 0) || 1;
    const totalRagdollMs = segments
      .filter((s) => s.key !== "downstream")
      .reduce((sum, s) => sum + s.ms, 0);

    return (
      <div className="w-full max-w-[1000px]">
        <div className="flex h-12 gap-1">
          {segments.map((seg, index) => (
            <TimelineBlock
              key={seg.key}
              label={seg.label}
              ms={seg.ms}
              index={index}
              growing={false}
              totalMs={totalWallMs}
              visualWeight={visualWeight(seg.ms)}
              totalVisual={totalVisual}
            />
          ))}
        </div>
        <div className="mt-2 flex justify-between text-xs text-[var(--muted)]">
          <span>t0 request</span>
          <span>
            ragdoll {totalRagdollMs}ms · total {totalWallMs}ms
          </span>
        </div>
      </div>
    );
  }

  const growingIdx = growingSegmentIndex(latency, false);
  const elapsed = tick * 50;
  const defs = visibleSegmentDefs(latency);

  const knownSum = defs.reduce((sum, def, i) => {
    if (growingIdx >= 0 && i >= growingIdx) return sum;
    if (def.key === "downstream") return sum;
    return sum + (segmentMsFromLatency(def, latency) ?? 0);
  }, 0);

  const growingMs =
    growingIdx < 0 ? 0 : Math.max(1, Math.min(PROGRESS_ASSUMED_TOTAL_MS - knownSum, elapsed));

  const segments = defs.map((def, i) => {
    if (def.key === "downstream") {
      if (i === growingIdx) return { key: def.key, label: def.label, ms: growingMs };
      return { key: def.key, label: def.label, ms: 1 };
    }
    const fixed = segmentMsFromLatency(def, latency);
    if (fixed !== undefined) return { key: def.key, label: def.label, ms: fixed };
    if (i === growingIdx) return { key: def.key, label: def.label, ms: growingMs };
    return { key: def.key, label: def.label, ms: 1 };
  });

  const totalVisual = segments.reduce((sum, s) => sum + visualWeight(s.ms), 0) || 1;

  return (
    <div className="w-full max-w-[1000px]">
      <div className="flex h-12 gap-1">
        {segments.map((seg, index) => (
          <TimelineBlock
            key={seg.key}
            label={seg.label}
            ms={seg.ms}
            index={index}
            growing={index === growingIdx}
            totalMs={PROGRESS_ASSUMED_TOTAL_MS}
            visualWeight={visualWeight(seg.ms)}
            totalVisual={totalVisual}
          />
        ))}
      </div>
      <div className="mt-2 flex justify-between text-xs text-[var(--muted)]">
        <span>t0 request</span>
        <span>est. {PROGRESS_ASSUMED_TOTAL_MS}ms · ragdoll {knownSum + growingMs}ms+</span>
      </div>
    </div>
  );
}

function LatencyTimeline({
  upstreamMs,
  embedMs,
  searchMs,
  rerankMs,
  storeMs,
  generationMs,
  downstreamMs,
}: {
  upstreamMs: number;
  embedMs: number;
  searchMs: number;
  rerankMs?: number | null;
  storeMs: number;
  generationMs?: number | null;
  downstreamMs: number;
}) {
  const segments = [
    { key: "upstream", label: "upstream", ms: upstreamMs },
    { key: "embed", label: "embed", ms: embedMs },
    { key: "search", label: "search", ms: searchMs },
    ...(rerankMs != null ? [{ key: "rerank", label: "rerank", ms: rerankMs }] : []),
    { key: "store", label: "store", ms: storeMs },
    ...(generationMs != null ? [{ key: "generation", label: "generation", ms: generationMs }] : []),
    { key: "downstream", label: "downstream", ms: downstreamMs },
  ];

  const visualWeight = (ms: number) => (ms <= 0 ? 1 : ms);
  const totalWallMs = segments.reduce((sum, s) => sum + s.ms, 0);
  const totalVisual = segments.reduce((sum, s) => sum + visualWeight(s.ms), 0) || 1;
  const ragdollDisplayMs = segments
    .filter((s) => s.key !== "downstream")
    .reduce((sum, s) => sum + s.ms, 0);

  return (
    <div className="w-full max-w-[1000px]">
      <div className="flex h-12 gap-1">
        {segments.map((seg, index) => (
          <TimelineBlock
            key={seg.key}
            label={seg.label}
            ms={seg.ms}
            index={index}
            growing={false}
            totalMs={totalWallMs}
            visualWeight={visualWeight(seg.ms)}
            totalVisual={totalVisual}
          />
        ))}
      </div>
      <div className="mt-2 flex justify-between text-xs text-[var(--muted)]">
        <span>t0 request</span>
        <span>
          ragdoll {ragdollDisplayMs}ms · total {totalWallMs}ms
        </span>
      </div>
    </div>
  );
}

export function PlaygroundPage() {
  const { releaseTag = "" } = useParams();
  const { can, ready } = usePermissions();
  const canRun = can(PERM.playground.run);
  const canReadModels = can(PERM.llmModels.read);
  const [params, setParams] = useSearchParams();
  const [tab, setTab] = useState<TabMode>("ui");
  const [text, setText] = useState(params.get("q") ?? "");
  const [topK, setTopK] = useState(Number(params.get("top_k") ?? 10));
  const [rerank, setRerank] = useState(params.get("rerank") !== "false");
  const [rerankCandidates, setRerankCandidates] = useState(Number(params.get("rerank_candidates") ?? 20));
  const [minSemanticScore, setMinSemanticScore] = useState(params.get("min_semantic_score") ?? "0.5");
  const [minRerankScore, setMinRerankScore] = useState(params.get("min_rerank_score") ?? "0.5");
  const [filter, setFilter] = useState(params.get("filter") ?? "");
  const [result, setResult] = useState<QueryResult | null>(null);
  const [detail, setDetail] = useState<QueryDetail | null>(null);
  const [clientEndMs, setClientEndMs] = useState<number | null>(null);
  const [clientStartMs, setClientStartMs] = useState<number | null>(null);
  const [loading, setLoading] = useState(false);
  const [generateEnabled, setGenerateEnabled] = useState(params.get("generate") === "true");
  const [streamEnabled, setStreamEnabled] = useState(params.get("stream") === "true");
  const [modelTag, setModelTag] = useState(params.get("model") ?? "");
  const [genTemperature, setGenTemperature] = useState(
    params.get("temperature") ?? String(DEFAULT_GENERATION_TEMPERATURE),
  );
  const [genMaxTokens, setGenMaxTokens] = useState(
    params.get("max_tokens") ?? String(DEFAULT_GENERATION_MAX_TOKENS),
  );
  const [genSystemPrompt, setGenSystemPrompt] = useState(
    () => decodeUrlParam(params.get("system_prompt")) || DEFAULT_GENERATION_SYSTEM_PROMPT,
  );
  const [releaseModels, setReleaseModels] = useState<LlmModelRecord[]>([]);
  const [modelsLoading, setModelsLoading] = useState(false);
  const [answerText, setAnswerText] = useState("");
  const [streamMatches, setStreamMatches] = useState<QueryMatch[]>([]);
  const [streamLatency, setStreamLatency] = useState<StreamLatencySnapshot>({});
  const [streamLatencyFinal, setStreamLatencyFinal] = useState(false);

  useEffect(() => {
    if (!canReadModels || params.get("generate") !== "true") return;
    void loadReleaseModels();
  }, [releaseTag, canReadModels]);

  async function loadReleaseModels() {
    if (!canReadModels) return [];
    setModelsLoading(true);
    try {
      const models = await api<LlmModelRecord[]>(`/releases/${releaseTag}/llm_models`);
      setReleaseModels(models);
      return models;
    } catch (err) {
      console.error(err);
      return [];
    } finally {
      setModelsLoading(false);
    }
  }

  async function handleGenerateChange(enabled: boolean) {
    setGenerateEnabled(enabled);
    if (!enabled) {
      setModelTag("");
      setGenTemperature(String(DEFAULT_GENERATION_TEMPERATURE));
      setGenMaxTokens(String(DEFAULT_GENERATION_MAX_TOKENS));
      setGenSystemPrompt(DEFAULT_GENERATION_SYSTEM_PROMPT);
      return;
    }
    setGenSystemPrompt((current) => current.trim() || DEFAULT_GENERATION_SYSTEM_PROMPT);
    if (releaseModels.length === 0 && canReadModels) {
      await loadReleaseModels();
    }
  }

  function parseOptionalNumber(raw: string): number | undefined {
    const trimmed = raw.trim();
    if (!trimmed) return undefined;
    const value = Number(trimmed);
    return Number.isFinite(value) ? value : undefined;
  }

  const generation = useMemo(() => {
    if (!generateEnabled) return undefined;
    const temperature = parseOptionalNumber(genTemperature) ?? DEFAULT_GENERATION_TEMPERATURE;
    const maxTokens = parseOptionalNumber(genMaxTokens) ?? DEFAULT_GENERATION_MAX_TOKENS;
    const systemPrompt = genSystemPrompt.trim() || DEFAULT_GENERATION_SYSTEM_PROMPT;
    return {
      stream: streamEnabled,
      tag: modelTag,
      system_prompt: systemPrompt,
      temperature,
      max_tokens: maxTokens,
    };
  }, [
    generateEnabled,
    streamEnabled,
    modelTag,
    genTemperature,
    genMaxTokens,
    genSystemPrompt,
  ]);

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
        ...(generation ? { generation } : {}),
      },
    ],
    [text, topK, rerank, rerankCandidates, minSemantic, minRerank, parsedFilter, generation],
  );

  const baseUrl = window.location.origin;

  const snippets = useMemo(
    () => buildSnippets(baseUrl, releaseTag, queryBody[0] ? [queryBody[0]] : []),
    [baseUrl, releaseTag, queryBody],
  );

  useEffect(() => {
    const next = new URLSearchParams();
    if (text) next.set("q", text);
    next.set("top_k", String(topK));
    next.set("rerank", String(rerank));
    next.set("rerank_candidates", String(rerankCandidates));
    next.set("min_semantic_score", minSemanticScore);
    next.set("min_rerank_score", minRerankScore);
    if (filter) next.set("filter", filter);
    if (generateEnabled) {
      next.set("generate", "true");
      next.set("stream", String(streamEnabled));
      if (modelTag) next.set("model", modelTag);
      next.set("temperature", genTemperature);
      next.set("max_tokens", genMaxTokens);
      next.set("system_prompt", encodeURIComponent(genSystemPrompt));
    }
    setParams(next, { replace: true });
  }, [
    text,
    topK,
    rerank,
    rerankCandidates,
    minSemanticScore,
    minRerankScore,
    filter,
    generateEnabled,
    streamEnabled,
    modelTag,
    genTemperature,
    genMaxTokens,
    genSystemPrompt,
    setParams,
  ]);

  async function runQuery() {
    setLoading(true);
    setAnswerText("");
    setStreamMatches([]);
    setStreamLatency({});
    setStreamLatencyFinal(false);
    setDetail(null);
    setResult(null);
    try {
      const tsStart = Date.now();
      setClientStartMs(tsStart);
      setClientEndMs(null);
      if (generateEnabled && streamEnabled) {
        await runPlaygroundQueryStream(releaseTag, queryBody, (event, data) => {
          if (event === "sources") {
            setStreamMatches(JSON.parse(data) as QueryMatch[]);
          } else if (event === "latency") {
            const parsed = JSON.parse(data) as Record<string, unknown>;
            setStreamLatency((prev) => applyLatencyEvent(prev, parsed));
          } else if (event === "token") {
            const parsed = JSON.parse(data) as { delta: string };
            setAnswerText((prev) => prev + parsed.delta);
          } else if (event === "done") {
            const parsed = JSON.parse(data) as {
              query_id: string;
              text?: string;
              latency?: StreamLatencySnapshot;
            };
            setClientEndMs(Date.now());
            if (parsed.text) {
              setAnswerText(parsed.text);
            }
            if (parsed.latency) {
              setStreamLatency((prev) => ({ ...prev, ...parsed.latency }));
            }
            setStreamLatencyFinal(true);
            void api<QueryDetail>(`/playground/${releaseTag}/queries/${parsed.query_id}`).then(
              setDetail,
            );
          }
        });
        setResult(null);
        return;
      }

      const res = await api<{ items: Array<{ result?: QueryResult }> }>(
        `/playground/${releaseTag}/queries?ts_start=${tsStart}`,
        { method: "POST", body: JSON.stringify(queryBody) },
      );
      setClientEndMs(Date.now());
      const query = res.items[0]?.result ?? null;
      setResult(query);
      setAnswerText(query?.answer?.text ?? "");
      if (query) {
        setDetail(await api<QueryDetail>(`/playground/${releaseTag}/queries/${query.query_id}`));
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
  const rerankMs = detail?.rerank_ms ?? result?.latency.rerank_ms ?? null;
  const storeMs = detail?.store_ms ?? result?.latency.store_ms ?? 0;
  const generationMs =
    detail?.generation_total_ms ??
    detail?.generation_ms ??
    result?.latency.generation_total_ms ??
    result?.latency.generation_ms ??
    null;
  const totalRagdollMs =
    detail?.total_ragdoll_ms ?? result?.latency.total_ragdoll_ms ?? 0;

  const downstreamMs =
    clientStartMs != null && clientEndMs != null
      ? Math.max(0, clientEndMs - clientStartMs - totalRagdollMs)
      : 0;

  const semanticChunks = (detail?.chunks ?? []).filter((c) => c.step === "semantic");
  const rerankChunks = (detail?.chunks ?? []).filter((c) => c.step === "rerank");
  const semanticPassing = semanticChunks.filter((c) => c.score >= minSemantic).length;
  const rerankPassing = rerankChunks.filter((c) => c.score >= minRerank).length;

  const queryMissing = !text.trim();
  const modelMissing = generateEnabled && !modelTag;
  const runDisabled = loading || queryMissing || modelMissing || !canRun;

  const showProgressiveTimeline =
    generateEnabled &&
    streamEnabled &&
    (loading || streamMatches.length > 0 || (streamLatencyFinal && detail == null));
  const showStaticTimeline = detail != null;
  const showResults =
    detail || answerText || streamMatches.length > 0 || showProgressiveTimeline;

  if (ready && !canRun) {
    return <PermissionDenied permission={PERM.playground.run} />;
  }

  return (
    <div className="space-y-6">
      <h2 className="text-2xl font-semibold">Playground</h2>

      <div className="card space-y-4">
        <div className="flex flex-wrap gap-2">
          {TABS.map(({ id, label }) => (
            <button
              key={id}
              type="button"
              className={`btn-secondary ${tab === id ? "btn-toggle-active" : ""}`}
              onClick={() => setTab(id)}
            >
              {label}
            </button>
          ))}
        </div>

        {tab === "ui" ? (
          <div className="space-y-8">
            <label className="block space-y-1 text-sm">
              <span className="font-medium">Query</span>
              <textarea
                className="input min-h-28"
                style={queryMissing ? ERROR_BORDER : undefined}
                value={text}
                onChange={(e) => setText(e.target.value)}
                placeholder="Query text"
              />
            </label>

            <section className="space-y-3">
              <div>
                <h3 className="text-base font-semibold">1 · Filter</h3>
                <p className="text-sm text-[var(--muted)]">
                  Hard metadata filter applied before vector search. Narrows both the returned
                  results and the search space.
                </p>
              </div>
              <FilterBuilder value={filter} onChange={setFilter} />
            </section>

            <section className="space-y-3">
              <div>
                <h3 className="text-base font-semibold">2 · Retrieve</h3>
                <p className="text-sm text-[var(--muted)]">
                  Vector search and optional cross-encoder reranking. Controls how many chunks are
                  fetched and the score thresholds that gate the final results.
                </p>
              </div>
              <div className="grid gap-4 md:grid-cols-3 lg:grid-cols-5">
                <label className="space-y-1 text-sm">
                  <span className="inline-flex items-center">
                    top_k
                    <InfoTip text="Maximum number of results in the final response (after score filters)." />
                  </span>
                  <input className="input" type="number" value={topK} onChange={(e) => setTopK(Number(e.target.value))} />
                </label>
                <label className="space-y-1 text-sm">
                  <span className="inline-flex items-center">
                    rerank
                    <InfoTip text="Enables the cross-encoder reranker." />
                  </span>
                  <select className="input" value={rerank ? "true" : "false"} onChange={(e) => setRerank(e.target.value === "true")}>
                    <option value="true">Enabled</option>
                    <option value="false">Disabled</option>
                  </select>
                </label>
                <label className="space-y-1 text-sm">
                  <span className="inline-flex items-center">
                    rerank_candidates
                    <InfoTip text="Semantic candidates for the reranker. Higher values increase request latency." />
                  </span>
                  <input className="input" type="number" value={rerankCandidates} onChange={(e) => setRerankCandidates(Number(e.target.value))} />
                </label>
                <label className="space-y-1 text-sm">
                  <span className="inline-flex items-center">
                    min_semantic_score
                    <InfoTip text="Hard filter before the rerank step. Default 0.5." />
                  </span>
                  <input
                    className="input"
                    type="text"
                    inputMode="decimal"
                    value={minSemanticScore}
                    onChange={(e) => setMinSemanticScore(e.target.value)}
                    placeholder="0.5"
                  />
                </label>
                <label className="space-y-1 text-sm">
                  <span className="inline-flex items-center">
                    min_rerank_score
                    <InfoTip text="Hard filter after the rerank step. Default 0.5. In the playground, all results are still shown." />
                  </span>
                  <input
                    className="input"
                    type="text"
                    inputMode="decimal"
                    value={minRerankScore}
                    onChange={(e) => setMinRerankScore(e.target.value)}
                    placeholder="0.5"
                  />
                </label>
              </div>
            </section>

            <section className="space-y-3">
              <div>
                <h3 className="text-base font-semibold">3 · Generate</h3>
                <p className="text-sm text-[var(--muted)]">
                  Optional BYO-LLM answer generation from the retrieved sources. Requires a model
                  configured on this release.
                </p>
              </div>
              <div className="grid gap-4 md:grid-cols-3 lg:grid-cols-5">
                <label className="space-y-1 text-sm">
                  <span className="inline-flex items-center">generate</span>
                  <select className="input" value={generateEnabled ? "true" : "false"} onChange={(e) => void handleGenerateChange(e.target.value === "true")}>
                    <option value="false">Off</option>
                    <option value="true">On</option>
                  </select>
                </label>
                {generateEnabled && (
                  <>
                    <label className="space-y-1 text-sm">
                      <span className="inline-flex items-center">stream</span>
                      <select className="input" value={streamEnabled ? "true" : "false"} onChange={(e) => setStreamEnabled(e.target.value === "true")}>
                        <option value="false">Sync JSON</option>
                        <option value="true">SSE</option>
                      </select>
                    </label>
                    <label className="space-y-1 text-sm md:col-span-2">
                      <span className="inline-flex items-center">model</span>
                      <select
                        className="input"
                        style={modelMissing ? ERROR_BORDER : undefined}
                        value={modelTag}
                        disabled={modelsLoading}
                        onChange={(e) => setModelTag(e.target.value)}
                      >
                        <option value="">
                          {modelsLoading ? "Loading models…" : "Select model"}
                        </option>
                        {releaseModels.map((m) => (
                          <option key={m.id} value={m.tag}>
                            {m.tag}
                          </option>
                        ))}
                      </select>
                    </label>
                  </>
                )}
              </div>
              {generateEnabled && (
                <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
                  <label className="space-y-1 text-sm">
                    <span className="inline-flex items-center">
                      temperature
                      <InfoTip text="Sampling temperature for the LLM. Default: 1." />
                    </span>
                    <input
                      className="input"
                      type="number"
                      step="0.1"
                      min="0"
                      max="2"
                      value={genTemperature}
                      onChange={(e) => setGenTemperature(e.target.value)}
                    />
                  </label>
                  <label className="space-y-1 text-sm">
                    <span className="inline-flex items-center">
                      max_tokens
                      <InfoTip text="Maximum output tokens for the LLM. Default: 5096." />
                    </span>
                    <input
                      className="input"
                      type="number"
                      min="1"
                      value={genMaxTokens}
                      onChange={(e) => setGenMaxTokens(e.target.value)}
                    />
                  </label>
                  <label className="space-y-1 text-sm md:col-span-2 lg:col-span-3">
                    <span className="inline-flex items-center">
                      system_prompt
                      <InfoTip text="Required system prompt sent to the LLM. Use it to control tone, format, and whether sources should be cited." />
                    </span>
                    <AutoGrowTextarea
                      className="input"
                      value={genSystemPrompt}
                      onChange={(e) => setGenSystemPrompt(e.target.value)}
                    />
                  </label>
                </div>
              )}
            </section>

            <button
              type="button"
              className="btn-run inline-flex items-center gap-2 disabled:cursor-not-allowed disabled:opacity-60"
              disabled={runDisabled}
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
          </div>
        ) : (
          <div className="relative">
            <CopySnippetButton text={snippets[tab as Exclude<TabMode, "ui">]} />
            <pre className="overflow-auto rounded-lg border p-4 pt-10 text-xs" style={{ borderColor: "var(--border)" }}>
              {snippets[tab as Exclude<TabMode, "ui">]}
            </pre>
          </div>
        )}
      </div>

      {showResults && (
        <>
          {showProgressiveTimeline && (
            <div className="card space-y-4">
              <h3 className="font-medium">Timeline</h3>
              <ProgressiveLatencyTimeline
                latency={streamLatency}
                complete={streamLatencyFinal && clientEndMs != null}
                clientStartMs={clientStartMs}
                clientEndMs={clientEndMs}
              />
            </div>
          )}
          {showStaticTimeline && (
          <div className="card space-y-4">
            <h3 className="font-medium">Timeline</h3>
            <LatencyTimeline
              upstreamMs={upstreamMs ?? 0}
              embedMs={embedMs}
              searchMs={searchMs}
              rerankMs={rerankMs}
              storeMs={storeMs}
              generationMs={generationMs}
              downstreamMs={downstreamMs}
            />
          </div>
          )}
          {answerText && (
            <div className="card space-y-2">
              <h3 className="font-medium">Generated answer</h3>
              <div className="markdown-answer space-y-2 text-sm [&_h1]:text-lg [&_h1]:font-semibold [&_h2]:text-base [&_h2]:font-semibold [&_ol]:list-decimal [&_ol]:pl-5 [&_p+p]:mt-2 [&_ul]:list-disc [&_ul]:pl-5 [&_table]:w-full [&_table]:border-collapse [&_th]:border [&_td]:border [&_th]:border-[var(--border)] [&_td]:border-[var(--border)] [&_th]:px-2 [&_th]:py-1 [&_td]:px-2 [&_td]:py-1 [&_th]:text-left [&_code]:rounded [&_code]:bg-[var(--selected)] [&_code]:px-1 [&_pre]:overflow-auto [&_pre]:rounded [&_pre]:border [&_pre]:border-[var(--border)] [&_pre]:p-3">
                <Markdown remarkPlugins={[remarkGfm]}>{answerText}</Markdown>
              </div>
            </div>
          )}

          <div className="grid gap-4 lg:grid-cols-2">
            <div className="card space-y-4">
              <div>
                <h3 className="font-medium">Semantic Results</h3>
                <p className="text-sm text-[var(--muted)]">
                  {semanticPassing} pass filter · {semanticChunks.length} total
                </p>
              </div>
              {(semanticChunks.length === 0 && streamMatches.length === 0) ? (
                <p className="text-sm text-[var(--muted)]">No semantic results.</p>
              ) : (
                (semanticChunks.length > 0 ? semanticChunks : streamMatches).map((c, idx) => (
                  <ResultChunkCard
                    key={`${"chunk_id" in c ? c.chunk_id : idx}-semantic`}
                    sourceName={"source_name" in c ? (c.source_name ?? c.source_id) : c.source_id}
                    score={"score" in c ? c.score : c.semantic_score}
                    step="semantic"
                    content={c.content}
                    filtered={("score" in c ? c.score : c.semantic_score) < minSemantic}
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
