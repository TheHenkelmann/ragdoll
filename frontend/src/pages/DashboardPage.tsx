// SPDX-License-Identifier: AGPL-3.0-only

import { useEffect, useState } from "react";
import { useParams } from "react-router-dom";
import { Bar, BarChart, Legend, Line, LineChart, ResponsiveContainer, Tooltip, XAxis, YAxis } from "recharts";
import { AnalyticsResponse, api, SystemMetricsResponse } from "../api/client";
import { InfoTip } from "../components/InfoTip";
import { PermissionDenied } from "../components/PermissionDenied";
import { QueryChunkCirclePack } from "../components/QueryChunkCirclePack";
import { SourceCirclePack } from "../components/SourceCirclePack";
import { usePermissions } from "../hooks/usePermissions";
import { PERM } from "../permissions";
import { defaultStartDate, formatBytesGiB, formatLatencyStats, formatPercent, todayDate } from "../utils/format";

const STATUS_GROUPS = ["2xx", "4xx", "5xx"] as const;
const STATUS_COLORS = { s2xx: "#22c55e", s4xx: "#f59e0b", s5xx: "#ef4444" };
const SYSTEM_METRICS_REFRESH_MS = 5_000;

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
  const { can, ready } = usePermissions();
  const canRead = can(PERM.analytics.read);
  const lens = stageTag ? "stage" : "release";
  const tag = stageTag ?? releaseTag ?? "";
  const [data, setData] = useState<AnalyticsResponse | null>(null);
  const [systemMetrics, setSystemMetrics] = useState<SystemMetricsResponse | null>(null);
  const [startDate, setStartDate] = useState(defaultStartDateState);
  const [endDate, setEndDate] = useState(todayDateState);
  const [statusFilter, setStatusFilter] = useState<string[]>([...STATUS_GROUPS]);

  useEffect(() => {
    if (!ready || !canRead || !tag) return;
    const qs = new URLSearchParams({ lens, tag, start: startDate, end: endDate });
    if (statusFilter.length > 0 && statusFilter.length < STATUS_GROUPS.length) {
      qs.set("status", statusFilter.join(","));
    }
    void api<AnalyticsResponse>(`/analytics?${qs}`)
      .then(setData)
      .catch(console.error);
  }, [ready, canRead, lens, tag, startDate, endDate, statusFilter]);

  useEffect(() => {
    if (!ready || !canRead || !tag) return;
    const loadSystemMetrics = () => {
      void api<SystemMetricsResponse>(`/system-metrics?start=${startDate}&end=${endDate}`)
        .then(setSystemMetrics)
        .catch(console.error);
    };

    loadSystemMetrics();
    const interval = window.setInterval(loadSystemMetrics, SYSTEM_METRICS_REFRESH_MS);
    return () => window.clearInterval(interval);
  }, [ready, canRead, tag, startDate, endDate]);

  if (ready && !canRead) {
    return <PermissionDenied permission={PERM.analytics.read} />;
  }

  const chartData = (data?.daily_requests ?? []).map((row) => ({
    day: row.day,
    s2xx: statusFilter.includes("2xx") ? row.s2xx : 0,
    s4xx: statusFilter.includes("4xx") ? row.s4xx : 0,
    s5xx: statusFilter.includes("5xx") ? row.s5xx : 0,
  }));
  const hasRequests = (data?.request_count ?? 0) > 0;
  const systemChartData = (systemMetrics?.samples ?? []).map((row) => ({
    recorded_at: row.recorded_at.replace("T", " ").slice(0, 16),
    cpu: row.cpu_percent,
    memory_pct:
      row.memory_total_bytes > 0 ? (row.memory_used_bytes / row.memory_total_bytes) * 100 : 0,
  }));
  const currentSystem = systemMetrics?.current;
  const memoryUsedPct =
    currentSystem && currentSystem.memory_total_bytes > 0
      ? (currentSystem.memory_used_bytes / currentSystem.memory_total_bytes) * 100
      : 0;

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
          info="Count of POST /queries requests in the selected time range (excluding playground). Filtered by HTTP status groups when active."
        />
        <div className="card">
          <div className="flex items-center text-sm text-[var(--muted)]">
            Sources
            <InfoTip
              text="Count of sources in the current release snapshot. In stage lens: data from the linked release at display time — ingestion data, not query results."
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
          info="Total chunks in the release snapshot after ingestion. Not the same as query_chunks (results of individual queries)."
        />
      </section>

      <section className="card">
        <SectionHeader
          title="Query requests per day"
          info="Daily POST /queries requests, split by HTTP status of the batch response per item: 2xx (success), 4xx (client error), 5xx (server error). Playground requests are excluded."
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
                  className={`btn-secondary text-xs ${statusFilter.includes(group) ? "btn-toggle-active" : "opacity-50"}`}
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
          info="Search pipeline latency for successful queries (HTTP 2xx) in the selected time range. Covers embed, vector search, reranking, generation, and DB storage — not source ingestion."
        />
        <div className="grid gap-3 md:grid-cols-2 lg:grid-cols-3">
          {(
            [
              ["Total", data?.total_latency],
              ["Embed", data?.embed_latency],
              ["Search", data?.search_latency],
              ["Rerank", data?.rerank_latency],
              ["Generation", data?.generation_latency],
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

      <section className="card">
        <SectionHeader
          title="Host system utilization"
          info="CPU and RAM usage of the entire machine Ragdoll runs on, sampled every second and stored locally. This is host-wide utilization — not scoped to this stage, release, or Ragdoll process alone."
        />
        <div className="mb-4 grid gap-4 md:grid-cols-3">
          <div className="rounded-lg border p-4" style={{ borderColor: "var(--border)" }}>
            <div className="text-sm text-[var(--muted)]">CPU now</div>
            <div className="mt-2 text-2xl font-semibold tabular-nums">
              {currentSystem ? formatPercent(currentSystem.cpu_percent, 1) : "–"}
            </div>
            <div className="mt-1 text-xs text-[var(--muted)]">
              {currentSystem ? `${currentSystem.cpu_cores} cores available` : "Collecting…"}
            </div>
          </div>
          <div className="rounded-lg border p-4" style={{ borderColor: "var(--border)" }}>
            <div className="text-sm text-[var(--muted)]">RAM now</div>
            <div className="mt-2 text-2xl font-semibold tabular-nums">
              {currentSystem
                ? `${formatBytesGiB(currentSystem.memory_used_bytes)} / ${formatBytesGiB(currentSystem.memory_total_bytes)}`
                : "–"}
            </div>
            <div className="mt-1 text-xs text-[var(--muted)]">
              {currentSystem ? `${formatPercent(memoryUsedPct, 1)} used` : "Collecting…"}
            </div>
          </div>
          <div className="rounded-lg border p-4" style={{ borderColor: "var(--border)" }}>
            <div className="text-sm text-[var(--muted)]">RAM available</div>
            <div className="mt-2 text-2xl font-semibold tabular-nums">
              {currentSystem ? formatBytesGiB(currentSystem.memory_available_bytes) : "–"}
            </div>
            <div className="mt-1 text-xs text-[var(--muted)]">Host-wide free memory</div>
          </div>
        </div>
        <p className="mb-4 text-xs text-[var(--muted)]">
          Overall system load on this host — includes the OS, other apps, and all Ragdoll stages/releases.
        </p>
        <div className="h-64">
          {systemChartData.length > 0 ? (
            <ResponsiveContainer width="100%" height="100%">
              <LineChart data={systemChartData}>
                <XAxis dataKey="recorded_at" stroke="var(--muted)" minTickGap={24} />
                <YAxis
                  yAxisId="cpu"
                  stroke="var(--muted)"
                  domain={[0, 100]}
                  tickFormatter={(v) => `${v}%`}
                />
                <YAxis
                  yAxisId="memory"
                  orientation="right"
                  stroke="var(--muted)"
                  domain={[0, 100]}
                  tickFormatter={(v) => `${v}%`}
                />
                <Tooltip
                  contentStyle={{
                    background: "var(--surface)",
                    border: "1px solid var(--border)",
                    color: "var(--text)",
                    borderRadius: "8px",
                  }}
                  itemStyle={{ color: "var(--text)" }}
                  labelStyle={{ color: "var(--muted)" }}
                  formatter={(value, name) => [
                    typeof value === "number" ? formatPercent(value, 1) : String(value),
                    name === "cpu" ? "CPU" : "RAM",
                  ]}
                />
                <Legend formatter={(value) => (value === "cpu" ? "CPU" : "RAM")} />
                <Line
                  yAxisId="cpu"
                  type="monotone"
                  dataKey="cpu"
                  name="cpu"
                  stroke="#3b82f6"
                  dot={false}
                  strokeWidth={2}
                />
                <Line
                  yAxisId="memory"
                  type="monotone"
                  dataKey="memory_pct"
                  name="memory_pct"
                  stroke="#a855f7"
                  dot={false}
                  strokeWidth={2}
                />
              </LineChart>
            </ResponsiveContainer>
          ) : (
            <div className="flex h-full items-center justify-center text-sm text-[var(--muted)]">
              System metrics appear after the server has been running for a few seconds.
            </div>
          )}
        </div>
      </section>

      {(data?.query_chunk_hits?.length ?? 0) > 0 && (
        <section className="card">
          <SectionHeader
            title="Query result chunks"
            info="Frequency of chunks returned in query results (query_chunks), filtered by the same time range and status as the request stats. Each circle is a chunk; size reflects how many queries included it (at most once per query)."
          />
          <QueryChunkCirclePack data={data?.query_chunk_hits ?? []} />
        </section>
      )}

      {(data?.query_chunk_metadata_keys?.length ?? 0) > 0 && (
        <section className="card">
          <SectionHeader
            title="Query chunk metadata key distribution"
            info="Frequency of metadata keys on chunk metadata stored in query_chunks — fields present on actually returned search hits. Independent of sources.metadata."
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
          info="Distribution of ingested chunks per source in the release snapshot (sources + chunks tables). Circle size reflects chunk count. Not query results."
        />
        <SourceCirclePack data={data?.chunks_per_source ?? []} />
      </section>

      {(data?.metadata_keys?.length ?? 0) > 0 && (
        <section className="card">
          <SectionHeader
            title="Source metadata key distribution (ingestion)"
            info="Frequency of metadata keys at source level (sources.metadata JSON). Shows which custom fields were set at upload — not chunk or query metadata."
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
