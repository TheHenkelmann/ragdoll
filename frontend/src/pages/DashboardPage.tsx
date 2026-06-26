// SPDX-License-Identifier: AGPL-3.0-only

import { useEffect, useState } from "react";
import { useParams } from "react-router-dom";
import { Bar, BarChart, Legend, ResponsiveContainer, Tooltip, XAxis, YAxis } from "recharts";
import { AnalyticsResponse, api } from "../api/client";
import { InfoTip } from "../components/InfoTip";
import { QueryChunkCirclePack } from "../components/QueryChunkCirclePack";
import { SourceCirclePack } from "../components/SourceCirclePack";
import { defaultStartDate, formatLatencyStats, todayDate } from "../utils/format";

const STATUS_GROUPS = ["2xx", "4xx", "5xx"] as const;
const STATUS_COLORS = { s2xx: "#22c55e", s4xx: "#f59e0b", s5xx: "#ef4444" };

function SectionHeader({ title, info }: { title: string; info: string }) {
  return (
    <h3 className="mb-4 flex items-center font-medium">
      {title}
      <InfoTip text={info} wide />
    </h3>
  );
}

function KpiCard({ label, value, info }: { label: string; value: number; info: string }) {
  return (
    <div className="card">
      <div className="flex items-center text-sm text-[var(--muted)]">
        {label}
        <InfoTip text={info} wide />
      </div>
      <div className="text-3xl font-semibold">{value}</div>
    </div>
  );
}

function defaultStartDateState() {
  return defaultStartDate();
}

function todayDateState() {
  return todayDate();
}

export function DashboardPage() {
  const { releaseTag, stageTag } = useParams();
  const lens = stageTag ? "stage" : "release";
  const tag = stageTag ?? releaseTag ?? "";
  const [data, setData] = useState<AnalyticsResponse | null>(null);
  const [startDate, setStartDate] = useState(defaultStartDateState);
  const [endDate, setEndDate] = useState(todayDateState);
  const [statusFilter, setStatusFilter] = useState<string[]>([...STATUS_GROUPS]);

  useEffect(() => {
    if (!tag) return;
    const qs = new URLSearchParams({ lens, tag, start: startDate, end: endDate });
    if (statusFilter.length > 0 && statusFilter.length < STATUS_GROUPS.length) {
      qs.set("status", statusFilter.join(","));
    }
    void api<AnalyticsResponse>(`/analytics?${qs}`)
      .then(setData)
      .catch(console.error);
  }, [lens, tag, startDate, endDate, statusFilter]);

  const chartData = (data?.daily_requests ?? []).map((row) => ({
    day: row.day,
    s2xx: statusFilter.includes("2xx") ? row.s2xx : 0,
    s4xx: statusFilter.includes("4xx") ? row.s4xx : 0,
    s5xx: statusFilter.includes("5xx") ? row.s5xx : 0,
  }));
  const hasRequests = (data?.request_count ?? 0) > 0;

  function fmtLatency(stats?: { p50: number; p95: number }) {
    return formatLatencyStats(stats, hasRequests);
  }

  function toggleStatus(group: string) {
    setStatusFilter((prev) => {
      const next = prev.includes(group) ? prev.filter((g) => g !== group) : [...prev, group];
      return next.length === 0 ? [...STATUS_GROUPS] : next;
    });
  }

  return (
    <div className="space-y-8">
      <h2 className="text-2xl font-semibold">Dashboard</h2>

      <section className="grid gap-4 md:grid-cols-3">
        <KpiCard
          label="Requests"
          value={data?.request_count ?? 0}
          info="Anzahl der POST /queries-Abfragen im gewählten Zeitraum (ohne Playground). Gefiltert nach HTTP-Status-Gruppen, sofern aktiv."
        />
        <div className="card">
          <div className="flex items-center text-sm text-[var(--muted)]">
            Sources
            <InfoTip
              text="Anzahl der Sources im aktuellen Release-Snapshot. Bei Stage-Lens: Daten des verknüpften Releases zum Zeitpunkt der Anzeige — Ingestion-Daten, keine Query-Ergebnisse."
              wide
            />
          </div>
          <div className="text-3xl font-semibold">{data?.source_count ?? 0}</div>
          {lens === "stage" && (
            <div className="mt-2 text-xs text-[var(--muted)]">Snapshot of current linked release</div>
          )}
        </div>
        <KpiCard
          label="Chunks"
          value={data?.chunk_count ?? 0}
          info="Gesamtzahl der Chunks im Release-Snapshot nach Ingestion. Nicht identisch mit query_chunks (Ergebnisse einzelner Abfragen)."
        />
      </section>

      <section className="card">
        <SectionHeader
          title="Query requests per day"
          info="Tägliche Abfragen über POST /queries, aufgeteilt nach HTTP-Status der Batch-Antwort pro Item: 2xx (erfolgreich), 4xx (Client-Fehler), 5xx (Server-Fehler). Playground-Abfragen sind ausgeschlossen."
        />
        <div className="mb-4 flex flex-wrap items-end gap-3">
          <label className="text-sm">
            <span className="mb-1 block text-[var(--muted)]">Start</span>
            <input
              type="date"
              className="input"
              value={startDate}
              max={endDate}
              onChange={(e) => setStartDate(e.target.value)}
            />
          </label>
          <label className="text-sm">
            <span className="mb-1 block text-[var(--muted)]">End</span>
            <input
              type="date"
              className="input"
              value={endDate}
              min={startDate}
              onChange={(e) => setEndDate(e.target.value)}
            />
          </label>
          <div className="text-sm">
            <span className="mb-1 block text-[var(--muted)]">Status</span>
            <div className="flex gap-2">
              {STATUS_GROUPS.map((group) => (
                <button
                  key={group}
                  type="button"
                  className={`btn-secondary text-xs ${statusFilter.includes(group) ? "ring-2 ring-[var(--accent)]" : "opacity-50"}`}
                  onClick={() => toggleStatus(group)}
                >
                  {group}
                </button>
              ))}
            </div>
          </div>
        </div>
        <div className="h-64">
          <ResponsiveContainer width="100%" height="100%">
            <BarChart data={chartData}>
              <XAxis dataKey="day" stroke="var(--muted)" />
              <YAxis stroke="var(--muted)" allowDecimals={false} />
              <Tooltip
                contentStyle={{
                  background: "var(--surface)",
                  border: "1px solid var(--border)",
                  color: "var(--text)",
                  borderRadius: "8px",
                }}
                itemStyle={{ color: "var(--text)" }}
                labelStyle={{ color: "var(--muted)" }}
                cursor={{ fill: "color-mix(in srgb, var(--accent) 15%, transparent)" }}
              />
              <Legend />
              {statusFilter.includes("2xx") && (
                <Bar dataKey="s2xx" name="2xx" stackId="status" fill={STATUS_COLORS.s2xx} radius={[0, 0, 0, 0]} />
              )}
              {statusFilter.includes("4xx") && (
                <Bar dataKey="s4xx" name="4xx" stackId="status" fill={STATUS_COLORS.s4xx} />
              )}
              {statusFilter.includes("5xx") && (
                <Bar dataKey="s5xx" name="5xx" stackId="status" fill={STATUS_COLORS.s5xx} radius={[4, 4, 0, 0]} />
              )}
            </BarChart>
          </ResponsiveContainer>
        </div>
      </section>

      <section className="card">
        <SectionHeader
          title="Query latency p50 / p95 (ms)"
          info="Latenz der Such-Pipeline bei erfolgreichen Abfragen (HTTP 2xx) im gewählten Zeitraum. Betrifft Embed, Vektorsuche, Reranking und DB-Speicherung — nicht die Ingestion von Sources."
        />
        <div className="grid gap-3 md:grid-cols-2 lg:grid-cols-3">
          {(
            [
              ["Total", data?.total_latency],
              ["Embed", data?.embed_latency],
              ["Search", data?.search_latency],
              ["Rerank", data?.rerank_latency],
              ["Store", data?.store_latency],
            ] as const
          ).map(([label, stats]) => {
            const lat = fmtLatency(stats);
            return (
              <div key={label} className="rounded-lg border p-4" style={{ borderColor: "var(--border)" }}>
                <div className="text-sm text-[var(--muted)]">{label}</div>
                <div className="mt-2 flex gap-8">
                  <div>
                    <span className="text-xs text-[var(--muted)]">p50</span>
                    <span className="ml-2 text-2xl font-semibold tabular-nums">{lat.p50}</span>
                  </div>
                  <div>
                    <span className="text-xs text-[var(--muted)]">p95</span>
                    <span className="ml-2 text-2xl font-semibold tabular-nums">{lat.p95}</span>
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      </section>

      {(data?.query_chunk_hits?.length ?? 0) > 0 && (
        <section className="card">
          <SectionHeader
            title="Query result chunks"
            info="Häufigkeit der in Abfrageergebnissen zurückgegebenen Chunks (query_chunks), gefiltert nach dem gleichen Zeitraum und Status wie die Request-Statistik. Jeder Kreis ist ein Chunk; die Größe entspricht der Anzahl Abfragen, in denen er vorkam (max. einmal pro Abfrage)."
          />
          <QueryChunkCirclePack data={data?.query_chunk_hits ?? []} />
        </section>
      )}

      {(data?.query_chunk_metadata_keys?.length ?? 0) > 0 && (
        <section className="card">
          <SectionHeader
            title="Query chunk metadata key distribution"
            info="Häufigkeit von Metadaten-Schlüsseln auf den in query_chunks gespeicherten Chunk-Metadaten — also Felder, die bei den tatsächlich zurückgegebenen Suchtreffern vorkommen. Unabhängig von sources.metadata."
          />
          <div className="grid gap-2 md:grid-cols-2 lg:grid-cols-3">
            {data?.query_chunk_metadata_keys.map(([key, count]) => (
              <div
                key={key}
                className="flex justify-between rounded border px-3 py-2 text-sm"
                style={{ borderColor: "var(--border)" }}
              >
                <span>{key}</span>
                <span className="text-[var(--muted)]">{count}</span>
              </div>
            ))}
          </div>
        </section>
      )}

      <section className="card">
        <SectionHeader
          title="Chunks per source (ingestion)"
          info="Verteilung der ingestierten Chunks pro Source im Release-Snapshot (Tabellen sources + chunks). Größe der Kreise entspricht der Chunk-Anzahl. Keine Query-Ergebnisse."
        />
        <SourceCirclePack data={data?.chunks_per_source ?? []} />
      </section>

      {(data?.metadata_keys?.length ?? 0) > 0 && (
        <section className="card">
          <SectionHeader
            title="Source metadata key distribution (ingestion)"
            info="Häufigkeit von Metadaten-Schlüsseln auf Source-Ebene (sources.metadata JSON). Zeigt, welche Custom-Felder beim Upload gesetzt wurden — nicht Chunk- oder Query-Metadaten."
          />
          <div className="grid gap-2 md:grid-cols-2 lg:grid-cols-3">
            {data?.metadata_keys.map(([key, count]) => (
              <div
                key={key}
                className="flex justify-between rounded border px-3 py-2 text-sm"
                style={{ borderColor: "var(--border)" }}
              >
                <span>{key}</span>
                <span className="text-[var(--muted)]">{count}</span>
              </div>
            ))}
          </div>
        </section>
      )}
    </div>
  );
}
