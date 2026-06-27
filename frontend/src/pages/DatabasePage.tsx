// SPDX-License-Identifier: AGPL-3.0-only

import { useEffect, useMemo, useState } from "react";
import { useParams, useSearchParams } from "react-router-dom";
import { api } from "../api/client";
import { PermissionDenied } from "../components/PermissionDenied";
import { usePermissions } from "../hooks/usePermissions";
import { PERM } from "../permissions";

const TABLES = [
  "sources",
  "chunks",
  "queries",
  "query_chunks",
  "ingest_jobs",
  "webhooks",
  "webhook_deliveries",
  "settings",
  "models",
  "releases",
  "stages",
];

type ColumnFacet = {
  truncated: boolean;
  values?: unknown[];
};

type DbTableResponse = {
  columns: string[];
  rows: Record<string, unknown>[];
  facets: Record<string, ColumnFacet>;
};

type ColFilter = {
  column: string;
  op: string;
  value: unknown;
};

function cellText(value: unknown): string {
  if (value == null) return "";
  if (typeof value === "object") return JSON.stringify(value);
  return String(value);
}

export function DatabasePage() {
  const { releaseTag = "" } = useParams();
  const { can, ready } = usePermissions();
  const canRead = can(PERM.db.read);
  const [params, setParams] = useSearchParams();
  const table = params.get("table") ?? "sources";
  const [data, setData] = useState<DbTableResponse | null>(null);
  const [sort, setSort] = useState<string>("");
  const [dir, setDir] = useState<"asc" | "desc">("asc");
  const [filters, setFilters] = useState<ColFilter[]>([]);
  const [draftFilters, setDraftFilters] = useState<Record<string, string>>({});

  const queryString = useMemo(() => {
    const q = new URLSearchParams({ limit: "100" });
    if (sort) {
      q.set("sort", sort);
      q.set("dir", dir);
    }
    if (filters.length > 0) q.set("filter", JSON.stringify(filters));
    return q.toString();
  }, [sort, dir, filters]);

  useEffect(() => {
    if (!ready || !canRead) return;
    void api<DbTableResponse>(`/releases/${releaseTag}/db/${table}?${queryString}`)
      .then(setData)
      .catch(console.error);
  }, [ready, canRead, releaseTag, table, queryString]);

  function toggleSort(col: string) {
    if (sort === col) {
      setDir((d) => (d === "asc" ? "desc" : "asc"));
    } else {
      setSort(col);
      setDir("asc");
    }
  }

  function applyFilter(col: string, value: string, op = "eq") {
    if (!value.trim()) {
      setFilters((prev) => prev.filter((f) => f.column !== col));
      return;
    }
    setFilters((prev) => {
      const rest = prev.filter((f) => f.column !== col);
      return [...rest, { column: col, op, value }];
    });
  }

  if (ready && !canRead) {
    return <PermissionDenied permission={PERM.db.read} />;
  }

  return (
    <div className="space-y-6">
      <h2 className="text-2xl font-semibold">Database</h2>
      <div className="flex flex-wrap gap-2">
        {TABLES.map((t) => (
          <button
            key={t}
            className={`btn-secondary ${table === t ? "btn-toggle-active" : ""}`}
            onClick={() => {
              params.set("table", t);
              setParams(params, { replace: true });
              setFilters([]);
              setSort("");
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
              {(data?.columns ?? []).map((col) => {
                const facet = data?.facets[col];
                const active = filters.find((f) => f.column === col);
                return (
                  <th key={col} className="px-3 py-2 text-left align-top">
                    <button type="button" className="font-medium hover:underline" onClick={() => toggleSort(col)}>
                      {col}
                      {sort === col ? (dir === "asc" ? " ↑" : " ↓") : ""}
                    </button>
                    <div className="mt-1">
                      {facet?.values ? (
                        <select
                          className="input max-w-[10rem] py-1 text-xs"
                          value={active ? String(active.value ?? "") : ""}
                          onChange={(e) => applyFilter(col, e.target.value)}
                        >
                          <option value="">All</option>
                          {facet.values.map((v, i) => (
                            <option key={i} value={cellText(v)}>
                              {cellText(v)}
                            </option>
                          ))}
                        </select>
                      ) : (
                        <input
                          className="input max-w-[10rem] py-1 text-xs"
                          placeholder={facet?.truncated ? "Type to filter…" : "Filter…"}
                          value={draftFilters[col] ?? (active ? String(active.value ?? "") : "")}
                          onChange={(e) =>
                            setDraftFilters((prev) => ({ ...prev, [col]: e.target.value }))
                          }
                          onKeyDown={(e) => {
                            if (e.key === "Enter") {
                              applyFilter(col, draftFilters[col] ?? "", "contains");
                            }
                          }}
                          onBlur={() => applyFilter(col, draftFilters[col] ?? "", "contains")}
                        />
                      )}
                      {facet?.truncated && (
                        <div className="text-[10px] text-subtle">Many unique values</div>
                      )}
                    </div>
                  </th>
                );
              })}
            </tr>
          </thead>
          <tbody>
            {(data?.rows ?? []).map((row, idx) => (
              <tr key={idx} className="border-b" style={{ borderColor: "var(--border)" }}>
                {(data?.columns ?? []).map((col) => (
                  <td key={col} className="px-3 py-2 align-top font-mono text-xs">
                    {cellText(row[col])}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
