// SPDX-License-Identifier: AGPL-3.0-only

import { useEffect, useState } from "react";
import { useParams, useSearchParams } from "react-router-dom";
import { api } from "../api/client";

const TABLES = ["sources", "chunks", "queries", "query_chunks", "ingest_jobs", "settings", "models", "releases", "stages"];

export function DatabasePage() {
  const { releaseTag = "" } = useParams();
  const [params, setParams] = useSearchParams();
  const table = params.get("table") ?? "sources";
  const [rows, setRows] = useState<Record<string, string>[]>([]);

  useEffect(() => {
    void api<Record<string, string>[]>(`/releases/${releaseTag}/db/${table}?limit=100`)
      .then(setRows)
      .catch(console.error);
  }, [releaseTag, table]);

  return (
    <div className="space-y-6">
      <h2 className="text-2xl font-semibold">Database</h2>
      <div className="flex flex-wrap gap-2">
        {TABLES.map((t) => (
          <button
            key={t}
            className={`btn-secondary ${table === t ? "ring-2 ring-[var(--accent)]" : ""}`}
            onClick={() => {
              params.set("table", t);
              setParams(params, { replace: true });
            }}
          >
            {t}
          </button>
        ))}
      </div>
      <div className="card overflow-auto">
        <table className="min-w-full text-sm">
          <thead>
            <tr className="border-b" style={{ borderColor: "var(--border)" }}>
              {Object.keys(rows[0] ?? {}).map((col) => <th key={col} className="px-3 py-2 text-left">{col}</th>)}
            </tr>
          </thead>
          <tbody>
            {rows.map((row, idx) => (
              <tr key={idx} className="border-b" style={{ borderColor: "var(--border)" }}>
                {Object.values(row).map((val, i) => <td key={i} className="px-3 py-2 align-top font-mono text-xs">{val}</td>)}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
