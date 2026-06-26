// SPDX-License-Identifier: AGPL-3.0-only

import * as d3 from "d3";
import { useEffect, useMemo, useRef, useState } from "react";
import { QueryChunkHit } from "../api/client";

type PackNode = d3.HierarchyCircularNode<PackDatum>;

type PackDatum = { name: string; value: number; id: string; children?: PackDatum[] };

export function QueryChunkCirclePack({ data }: { data: QueryChunkHit[] }) {
  const ref = useRef<SVGSVGElement>(null);
  const [view, setView] = useState<"chart" | "list">("chart");
  const [search, setSearch] = useState("");
  const [hover, setHover] = useState<QueryChunkHit | null>(null);

  const filtered = useMemo(() => {
    const q = search.toLowerCase();
    return data.filter(
      (d) =>
        !q ||
        d.chunk_id.toLowerCase().includes(q) ||
        d.source_id.toLowerCase().includes(q) ||
        d.source_name.toLowerCase().includes(q),
    );
  }, [data, search]);

  useEffect(() => {
    if (view !== "chart" || !ref.current || filtered.length === 0) return;
    const width = 520;
    const height = 420;
    const rootData: PackDatum = {
      name: "root",
      value: 0,
      id: "root",
      children: filtered.map((d) => ({
        name: d.source_name,
        value: Math.max(d.hit_count, 1),
        id: d.chunk_id,
      })),
    };
    const root = d3
      .hierarchy(rootData)
      .sum((d) => d.value)
      .sort((a, b) => (b.value ?? 0) - (a.value ?? 0));

    const pack = d3.pack<PackDatum>().size([width, height]).padding(3);
    const nodes = pack(root).descendants().slice(1) as PackNode[];

    const svg = d3.select(ref.current);
    svg.selectAll("*").remove();
    svg.attr("viewBox", `0 0 ${width} ${height}`);

    const g = svg.append("g");
    g.selectAll("circle")
      .data(nodes)
      .join("circle")
      .attr("cx", (d) => d.x)
      .attr("cy", (d) => d.y)
      .attr("r", (d) => d.r)
      .attr("fill", "var(--accent)")
      .attr("fill-opacity", 0.35)
      .attr("stroke", "var(--accent)")
      .on("mouseenter", (_, d) => {
        const item = filtered.find((f) => f.chunk_id === d.data.id) ?? null;
        setHover(item);
      })
      .on("mouseleave", () => setHover(null));
  }, [filtered, view]);

  return (
    <div className="space-y-3">
      <div className="flex flex-wrap items-center gap-2">
        <input
          className="input max-w-xs"
          placeholder="Search chunks or sources"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
        <button
          className={`btn-secondary ${view === "chart" ? "ring-2 ring-[var(--accent)]" : ""}`}
          onClick={() => setView("chart")}
        >
          Chart
        </button>
        <button
          className={`btn-secondary ${view === "list" ? "ring-2 ring-[var(--accent)]" : ""}`}
          onClick={() => setView("list")}
        >
          List
        </button>
      </div>
      <div className="min-h-[1.25rem] text-sm text-[var(--muted)]" aria-live="polite">
        {hover
          ? `${hover.source_name} · ${hover.hit_count} hits · ${hover.chunk_id}`
          : "\u00a0"}
      </div>
      {filtered.length === 0 ? (
        <p className="text-sm text-[var(--muted)]">No query results with stored chunks yet.</p>
      ) : view === "chart" ? (
        <svg ref={ref} className="mx-auto block h-[420px] w-full max-w-xl" />
      ) : (
        <div className="max-h-80 space-y-1 overflow-auto text-sm">
          {filtered.map((d) => (
            <div
              key={d.chunk_id}
              className="flex justify-between gap-3 rounded border px-3 py-2"
              style={{ borderColor: "var(--border)" }}
            >
              <span className="truncate">
                {d.source_name}
                <span className="ml-2 text-[var(--muted)]">{d.chunk_id.slice(0, 8)}…</span>
              </span>
              <span className="shrink-0 text-[var(--muted)]">{d.hit_count}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
