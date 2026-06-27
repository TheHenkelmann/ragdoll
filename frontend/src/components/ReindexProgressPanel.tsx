// SPDX-License-Identifier: AGPL-3.0-only

import { useEffect, useRef, useState } from "react";
import { IngestJobsStatusResponse, ReindexBatchEvent, streamReindexBatch } from "../api/client";
import { refreshModelStatus } from "../hooks/useModelStatus";

type Props = {
  releaseTag: string;
  batchId: string;
};

function pct(count: number, total: number): number {
  if (total === 0) return 0;
  return Math.round((count / total) * 100);
}

function StatusBar({
  label,
  count,
  total,
  color,
}: {
  label: string;
  count: number;
  total: number;
  color: string;
}) {
  const width = pct(count, total);
  return (
    <div className="space-y-1">
      <div className="flex justify-between text-xs">
        <span>{label}</span>
        <span className="text-[var(--muted)]">
          {count} ({width}%)
        </span>
      </div>
      <div className="h-2 overflow-hidden rounded-full" style={{ background: "var(--border)" }}>
        <div
          className="h-full rounded-full transition-all"
          style={{ width: `${width}%`, background: color }}
        />
      </div>
    </div>
  );
}

export function ReindexProgressPanel({ releaseTag, batchId }: Props) {
  const [data, setData] = useState<IngestJobsStatusResponse | null>(null);
  const [showDetails, setShowDetails] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const notifiedDone = useRef(false);

  useEffect(() => {
    const controller = new AbortController();
    setError(null);
    setData(null);
    notifiedDone.current = false;
    void streamReindexBatch(
      releaseTag,
      batchId,
      (event: ReindexBatchEvent) => {
        setData({
          summary: event.summary,
          jobs: event.jobs,
        });
        const finished =
          event.summary.total > 0 &&
          event.summary.active === 0 &&
          event.summary.completed + event.summary.failed >= event.summary.total;
        if (finished && !notifiedDone.current) {
          notifiedDone.current = true;
          // Clear the stale embedding-mismatch warning/badge once re-index settles.
          refreshModelStatus();
        }
      },
      controller.signal,
    ).catch((err) => {
      if (!controller.signal.aborted) setError(String(err));
    });
    return () => controller.abort();
  }, [releaseTag, batchId]);

  if (error) {
    return <p className="text-sm text-error">{error}</p>;
  }

  if (!data || data.summary.total === 0) return null;

  const { summary } = data;
  const done = summary.completed + summary.failed;
  const overallPct = pct(done, summary.total);

  return (
    <div
      className="space-y-4 rounded-lg border p-4"
      style={{ borderColor: "var(--border)", background: "var(--surface)" }}
    >
      <div className="flex flex-wrap items-start justify-between gap-2">
        <div>
          <h4 className="text-sm font-medium">Reindex batch</h4>
          <div className="font-mono text-[10px] text-[var(--muted)]">{batchId}</div>
          <p className="text-xs text-[var(--muted)]">
            {summary.active > 0
              ? "Reindexing in progress — search results may be unreliable until all jobs finish."
              : summary.failed > 0
                ? "Some jobs failed. Expand details to inspect errors."
                : "All jobs completed."}
          </p>
        </div>
        <div className="text-right">
          <div className="text-2xl font-semibold">{overallPct}%</div>
          <div className="text-xs text-[var(--muted)]">
            {done} / {summary.total} finished
          </div>
        </div>
      </div>

      <div className="space-y-2">
        <StatusBar label="Completed" count={summary.completed} total={summary.total} color="#22c55e" />
        <StatusBar label="Processing" count={summary.processing} total={summary.total} color="#3b82f6" />
        <StatusBar label="Pending" count={summary.pending} total={summary.total} color="#f59e0b" />
        {summary.failed > 0 && (
          <StatusBar label="Failed" count={summary.failed} total={summary.total} color="#ef4444" />
        )}
      </div>

      <button
        type="button"
        className="text-sm text-[var(--muted)] hover:text-[var(--text)]"
        onClick={() => setShowDetails((v) => !v)}
      >
        {showDetails ? "Hide job list" : "Show job list"}
      </button>

      {showDetails && data.jobs && (
        <div className="max-h-64 overflow-y-auto rounded-md border" style={{ borderColor: "var(--border)" }}>
          <table className="w-full text-left text-xs">
            <thead>
              <tr className="border-b" style={{ borderColor: "var(--border)" }}>
                <th className="px-2 py-1.5 font-medium">Source</th>
                <th className="px-2 py-1.5 font-medium">Status</th>
                <th className="px-2 py-1.5 font-medium">Error</th>
              </tr>
            </thead>
            <tbody>
              {data.jobs.map((job) => (
                <tr key={job.id} className="border-b" style={{ borderColor: "var(--border)" }}>
                  <td className="px-2 py-1.5">
                    <div className="font-medium">{job.source_name ?? job.source_id}</div>
                    <div className="font-mono text-[10px] text-[var(--muted)]">{job.id}</div>
                  </td>
                  <td className="px-2 py-1.5 capitalize">{job.status}</td>
                  <td className="px-2 py-1.5 text-error">{job.error ?? "—"}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
