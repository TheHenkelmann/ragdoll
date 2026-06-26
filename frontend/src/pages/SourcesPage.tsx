// SPDX-License-Identifier: AGPL-3.0-only

import { useEffect, useMemo, useState } from "react";
import { useParams, useSearchParams } from "react-router-dom";
import { ChunkRecord, SourceRecord, api } from "../api/client";

export function SourcesPage() {
  const { releaseTag = "" } = useParams();
  const [params, setParams] = useSearchParams();
  const [sources, setSources] = useState<SourceRecord[]>([]);
  const [chunks, setChunks] = useState<ChunkRecord[]>([]);
  const [selected, setSelected] = useState<string | null>(params.get("source"));
  const filter = params.get("q") ?? "";

  useEffect(() => {
    void api<SourceRecord[]>(`/releases/${releaseTag}/sources?limit=200`).then(setSources).catch(console.error);
  }, [releaseTag]);

  useEffect(() => {
    if (!selected) {
      setChunks([]);
      return;
    }
    void api<ChunkRecord[]>(`/releases/${releaseTag}/chunks?limit=200&filter=${encodeURIComponent(JSON.stringify({ field: "source_id", op: "eq", value: selected }))}`)
      .then(setChunks)
      .catch(console.error);
  }, [releaseTag, selected]);

  const filteredSources = useMemo(() => {
    const q = filter.toLowerCase();
    return sources.filter((s) => !q || s.name.toLowerCase().includes(q) || s.id.includes(q));
  }, [sources, filter]);

  return (
    <div className="space-y-6">
      <h2 className="text-2xl font-semibold">Sources</h2>
      <input
        className="input max-w-md"
        placeholder="Filter sources by name or id"
        value={filter}
        onChange={(e) => {
          params.set("q", e.target.value);
          setParams(params, { replace: true });
        }}
      />
      <div className="grid gap-6 lg:grid-cols-2">
        <div className="card space-y-2">
          {filteredSources.map((s) => (
            <button
              key={s.id}
              className={`block w-full rounded-lg border px-3 py-2 text-left ${selected === s.id ? "ring-2 ring-[var(--accent)]" : ""}`}
              style={{ borderColor: "var(--border)" }}
              onClick={() => {
                setSelected(s.id);
                params.set("source", s.id);
                setParams(params, { replace: true });
              }}
            >
              <div className="font-medium">{s.name}</div>
              <div className="text-xs text-[var(--muted)]">
                {s.type} · {s.status} · {s.chunk_count} {s.chunk_count === 1 ? "chunk" : "chunks"}
              </div>
            </button>
          ))}
        </div>
        <div className="card space-y-3">
          {!selected && <p className="text-sm text-[var(--muted)]">Select a source to view chunks</p>}
          {chunks.map((c) => (
            <div key={c.id} className="rounded-lg border p-3 text-sm" style={{ borderColor: "var(--border)" }}>
              <div className="text-xs text-[var(--muted)]">{c.id}</div>
              <p className="mt-2 whitespace-pre-wrap">{c.content}</p>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
