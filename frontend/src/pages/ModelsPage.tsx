// SPDX-License-Identifier: AGPL-3.0-only

import { useCallback, useEffect, useMemo, useState } from "react";
import { Link } from "react-router-dom";
import {
  CatalogStatusEntry,
  ModelsStatusResponse,
  addCustomModel,
  deleteModel,
  getModelsStatus,
  purgeModelMemory,
  testModel,
} from "../api/client";
import { CreateTagControl } from "../components/ObjectOverview";
import { InfoTip } from "../components/InfoTip";
import { PermissionDenied } from "../components/PermissionDenied";
import { useSnackbar } from "../context/SnackbarContext";
import { useModelDownloads } from "../hooks/useModelDownloads";
import { usePermissions } from "../hooks/usePermissions";
import {
  COLUMN_TIPS,
  compareCatalogRows,
  filterCatalogRows,
  formatRam,
  hfModelUrl,
} from "../modelCatalog";
import { PERM } from "../permissions";

const MODELS_DOCS_URL = "https://github.com/TheHenkelmann/ragdoll/blob/main/docs/models.md";

const PAGE_TIPS = {
  page:
    "Retrieval models (this page): local ONNX embedding and rerank models for vector search. " +
    "Download and verify models here, then select them per release in Settings.",
  missing:
    "A release settings entry points to this model, but the ONNX files are not on disk yet. Download from this page.",
  mismatch:
    "Stored chunk vectors were produced by a different embedding model than currently configured in Settings. Re-index is required.",
  custom:
    "Add a Hugging Face org/model id not in the predefined catalog. Only 1024-dim models wired for retrieval will work.",
} as const;

function DocLink({ href, label }: { href: string; label: string }) {
  return (
    <a
      href={href}
      target="_blank"
      rel="noopener noreferrer"
      className="inline-flex items-center rounded-md border px-1.5 py-0.5 text-xs text-[var(--muted)] hover:text-[var(--text)]"
      style={{ borderColor: "var(--border)" }}
      aria-label={label}
      title={label}
    >
      ↗
    </a>
  );
}

function LabelWithTip({ label, tip, wide }: { label: string; tip: string; wide?: boolean }) {
  return (
    <span className="inline-flex items-center gap-1">
      {label}
      <InfoTip text={tip} wide={wide} />
    </span>
  );
}

function ThWithTip({ label, tip }: { label: string; tip: string }) {
  return (
    <th className="px-2 py-2 text-left font-medium">
      <LabelWithTip label={label} tip={tip} />
    </th>
  );
}

function ReleasesCell({ releases }: { releases: string[] }) {
  if (releases.length === 0) return <span className="text-[var(--muted)]">—</span>;
  if (releases.length === 1) {
    return <span className="text-xs">{releases[0]}</span>;
  }
  return (
    <span className="group relative cursor-default text-xs">
      {releases.length} releases
      <span
        className="pointer-events-none absolute left-0 top-full z-10 mt-1 hidden min-w-[10rem] rounded-md border px-2 py-1.5 text-xs shadow-md group-hover:block"
        style={{ borderColor: "var(--border)", background: "var(--surface)" }}
      >
        {releases.map((tag) => (
          <div key={tag}>{tag}</div>
        ))}
      </span>
    </span>
  );
}

function DownloadStatusCell({
  row,
  state,
  canDownload,
  onDownload,
  onCancel,
}: {
  row: CatalogStatusEntry;
  state?: { status: string; message?: string; progress?: number; cancellable?: boolean };
  canDownload: boolean;
  onDownload: (name: string) => void;
  onCancel: (name: string) => void;
}) {
  const downloading = ["downloading", "materializing"].includes(state?.status ?? "");
  const busy = downloading || state?.status === "testing";

  if (row.present && !busy) {
    return <span className="text-[var(--muted)]">Present</span>;
  }

  if (busy || state?.status === "downloading") {
    return (
      <div className="space-y-1">
        <div className="flex items-center gap-2">
          <div className="text-xs text-[var(--muted)]">{state?.message ?? "Downloading…"}</div>
          {downloading && canDownload && state?.cancellable === true && (
            <button
              type="button"
              className="btn-secondary px-1.5 py-0.5 text-[10px] text-error"
              onClick={() => onCancel(row.name)}
            >
              Cancel
            </button>
          )}
        </div>
        <div className="h-1.5 w-32 overflow-hidden rounded-full" style={{ background: "var(--border)" }}>
          {state?.progress != null ? (
            <div className="h-full bg-blue-500 transition-all" style={{ width: `${state.progress}%` }} />
          ) : (
            <div className="h-full w-full animate-pulse bg-blue-500" />
          )}
        </div>
      </div>
    );
  }

  if (canDownload) {
    return (
      <button
        type="button"
        className="btn-primary text-xs"
        onClick={() => onDownload(row.name)}
      >
        Download
      </button>
    );
  }

  return <span className="text-[var(--muted)]">Not downloaded</span>;
}

function validateCustomModel(name: string): string | null {
  const parts = name.split("/");
  if (parts.length !== 2 || !parts[0] || !parts[1]) {
    return "Enter a Hugging Face id as org/model";
  }
  return null;
}

export function ModelsPage() {
  const { can, ready } = usePermissions();
  const snackbar = useSnackbar();
  const canRead = can(PERM.models.read);
  const canDownload = can(PERM.models.download);
  const canDelete = can(PERM.models.delete);
  const canTest = can(PERM.models.read);

  const [data, setData] = useState<ModelsStatusResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [search, setSearch] = useState("");

  const reload = useCallback(() => {
    if (!canRead) {
      setLoading(false);
      return;
    }
    setLoading(true);
    setError(null);
    void getModelsStatus()
      .then(setData)
      .catch((err) => setError(String(err)))
      .finally(() => setLoading(false));
  }, [canRead]);

  const { rowState, startDownload, cancelDownload, reconnectActive, setRow } = useModelDownloads({
    onComplete: (name) => {
      snackbar.success("Model ready", `${name} downloaded and verified.`);
      reload();
    },
    onError: (name, message) => {
      snackbar.error("Model download failed", `${name}: ${message}`);
      reload();
    },
    onCancel: (name) => {
      snackbar.info("Download cancelled", name);
    },
  });

  function handleCancel(name: string) {
    if (!window.confirm(`Cancel the download of ${name}?`)) return;
    void cancelDownload(name);
  }

  useEffect(() => {
    if (!ready) return;
    reload();
  }, [ready, reload]);

  useEffect(() => {
    if (!data?.active_downloads.length) return;
    reconnectActive(data.active_downloads);
  }, [data?.active_downloads, reconnectActive]);

  async function handleTest(name: string) {
    setRow(name, { status: "testing", message: "Testing…" });
    try {
      const result = await testModel(name);
      setRow(name, { status: "ready", message: `OK (${result.latency_ms} ms)` });
      snackbar.success("Model verified", `${name}: ${result.latency_ms} ms`);
      // The model is now loaded in gateway RAM; refresh to show RAM usage.
      reload();
    } catch (err) {
      setRow(name, { status: "error", message: String(err) });
      snackbar.error("Model test failed", `${name}: ${String(err)}`);
    }
  }

  async function handlePurge(name: string) {
    setRow(name, { status: "purging", message: "Unloading…" });
    try {
      const result = await purgeModelMemory(name);
      const total = result.purged_embedders + result.purged_rerankers;
      setRow(name, {
        status: "ready",
        message: total > 0 ? "Unloaded from gateway RAM" : "Not loaded in gateway RAM",
      });
      reload();
    } catch (err) {
      setRow(name, { status: "error", message: String(err) });
      snackbar.error("Unload failed", `${name}: ${String(err)}`);
    }
  }

  function deleteConfirmPhrase(row: CatalogStatusEntry): string {
    return `${row.kind}/${row.name}`;
  }

  function handleDelete(row: CatalogStatusEntry) {
    const phrase = deleteConfirmPhrase(row);
    const typed = window.prompt(
      `Permanently delete all local files for this model?\n\nType "${phrase}" to confirm:`,
    );
    if (typed !== phrase) return;
    void deleteModel(row.name)
      .then(() => {
        snackbar.success("Model deleted", `${row.name} removed from disk.`);
        reload();
      })
      .catch((err) => {
        setError(String(err));
        snackbar.error("Delete failed", `${row.name}: ${String(err)}`);
      });
  }

  const sortedRows = useMemo(() => {
    if (!data) return [];
    const filtered = filterCatalogRows(data.catalog, search);
    return [...filtered].sort((a, b) =>
      compareCatalogRows(a, b, data.active_downloads, rowState),
    );
  }, [data, search, rowState]);

  if (ready && !canRead) {
    return <PermissionDenied permission={PERM.models.read} />;
  }

  const hasMissing = (data?.missing.length ?? 0) > 0;
  const hasMismatches = (data?.mismatches.length ?? 0) > 0;

  return (
    <div className="space-y-6">
      <h2 className="inline-flex items-center gap-2 text-2xl font-semibold">
        Models
        <DocLink href={MODELS_DOCS_URL} label="Open docs: Retrieval models" />
      </h2>
      <p className="text-sm text-[var(--muted)]">
        <span className="inline-flex items-center gap-1">
          Local ONNX embedding and rerank models (1024-dim). Download and verify here; release
          Settings only let you pick models that are already on disk.
          <InfoTip text={PAGE_TIPS.page} wide />
        </span>
      </p>

      {hasMissing && (
        <div
          className="rounded-lg border px-4 py-3 text-sm text-error"
          style={{
            borderColor: "var(--error)",
            background: "color-mix(in srgb, var(--error) 8%, transparent)",
          }}
        >
          <div className="inline-flex items-center font-medium">
            Missing models required by releases
            <InfoTip text={PAGE_TIPS.missing} wide tone="danger" />
          </div>
          <ul className="mt-2 list-inside list-disc">
            {data!.missing.map((name) => (
              <li key={name} className="font-mono text-xs">
                {name}
              </li>
            ))}
          </ul>
        </div>
      )}

      {hasMismatches && (
        <div
          className="rounded-lg border px-4 py-3 text-sm text-error"
          style={{
            borderColor: "var(--error)",
            background: "color-mix(in srgb, var(--error) 8%, transparent)",
          }}
        >
          <div className="inline-flex items-center font-medium">
            Embedding model mismatch — re-index required
            <InfoTip text={PAGE_TIPS.mismatch} wide tone="danger" />
          </div>
          <ul className="mt-2 space-y-2">
            {data!.mismatches.map((m) => (
              <li key={m.release_id}>
                <span className="font-medium">{m.release_tag}</span>: {m.message}
              </li>
            ))}
          </ul>
        </div>
      )}

      <div className="flex flex-wrap items-center gap-3">
        {canDownload && (
          <CreateTagControl
            label="Add custom model"
            maxLength={120}
            disabled={!canDownload}
            validate={validateCustomModel}
            onCreate={async (name) => {
              try {
                await addCustomModel(name);
                snackbar.success("Custom model added", name);
                reload();
              } catch (err) {
                snackbar.error("Could not add custom model", `${name}: ${String(err)}`);
                throw err;
              }
            }}
          />
        )}
        <input
          className="input min-w-[240px] flex-1"
          placeholder="Search models & releases…"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
      </div>

      {error && <p className="text-sm text-error">{error}</p>}
      {loading && <p className="text-sm text-[var(--muted)]">Loading…</p>}

      {data && (
        <div className="card overflow-auto">
          <table className="min-w-full text-sm">
            <thead>
              <tr className="border-b" style={{ borderColor: "var(--border)" }}>
                <ThWithTip label="Model" tip={COLUMN_TIPS.model} />
                <ThWithTip label="Kind" tip={COLUMN_TIPS.kind} />
                <ThWithTip label="Languages" tip={COLUMN_TIPS.languages} />
                <ThWithTip label="Releases" tip={COLUMN_TIPS.releases} />
                <ThWithTip label="Download Status" tip={COLUMN_TIPS.download} />
                <ThWithTip label="RAM" tip={COLUMN_TIPS.ram} />
                <th className="px-2 py-2 text-right font-medium">
                  <span className="inline-flex items-center justify-end gap-1">
                    Actions
                    <InfoTip text={COLUMN_TIPS.actions} />
                  </span>
                </th>
              </tr>
            </thead>
            <tbody>
              {sortedRows.map((row) => {
                const state = rowState[row.name];
                const disabled = !row.present;
                const busy =
                  state?.status === "downloading" ||
                  state?.status === "testing" ||
                  state?.status === "purging" ||
                  state?.status === "materializing";
                return (
                  <tr
                    key={row.name}
                    className={`border-b ${disabled ? "opacity-50" : ""}`}
                    style={{ borderColor: "var(--border)" }}
                  >
                    <td className="px-2 py-2">
                      <span className="inline-flex items-center gap-1 font-mono text-xs">
                        {row.name}
                        <DocLink href={hfModelUrl(row.name)} label={`Open ${row.name} on Hugging Face`} />
                        {row.custom && (
                          <span className="rounded border px-1 text-[10px] text-[var(--muted)]" style={{ borderColor: "var(--border)" }}>
                            custom
                          </span>
                        )}
                      </span>
                    </td>
                    <td className="px-2 py-2">{row.kind}</td>
                    <td className="px-2 py-2 text-xs text-[var(--muted)]">
                      {row.languages.length > 0 ? row.languages.join(", ") : "—"}
                    </td>
                    <td className="px-2 py-2">
                      <ReleasesCell releases={row.releases} />
                    </td>
                    <td className="px-2 py-2">
                      <DownloadStatusCell
                        row={row}
                        state={state}
                        canDownload={canDownload}
                        onDownload={(name) => void startDownload(name)}
                        onCancel={handleCancel}
                      />
                    </td>
                    <td className="px-2 py-2 text-xs">
                      {row.loaded ? (
                        <span className="inline-flex items-center gap-2">
                          <span>{formatRam(row.ram_bytes)}</span>
                          {canDownload && (
                            <button
                              type="button"
                              className="btn-secondary text-xs"
                              disabled={busy}
                              onClick={() => void handlePurge(row.name)}
                            >
                              Unload
                            </button>
                          )}
                        </span>
                      ) : (
                        "—"
                      )}
                    </td>
                    <td className="px-2 py-2 text-right">
                      <div className="flex justify-end gap-2">
                        {canTest && row.present && (
                          <button
                            type="button"
                            className="btn-secondary text-xs"
                            disabled={busy}
                            onClick={() => void handleTest(row.name)}
                          >
                            {state?.status === "testing" ? "Testing…" : "Test"}
                          </button>
                        )}
                        {canDelete && row.present && (
                          <button
                            type="button"
                            className="btn-secondary text-xs text-error"
                            disabled={busy}
                            onClick={() => handleDelete(row)}
                          >
                            Delete
                          </button>
                        )}
                      </div>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
          {sortedRows.length === 0 && (
            <p className="p-4 text-sm text-[var(--muted)]">No models match your search.</p>
          )}
        </div>
      )}

      <p className="text-xs text-[var(--muted)]">
        Per-release model selection:{" "}
        <Link to="/releases" className="underline hover:text-[var(--text)]">
          open a release → Settings
        </Link>
        . See{" "}
        <a href={MODELS_DOCS_URL} target="_blank" rel="noopener noreferrer" className="underline">
          docs/models.md
        </a>{" "}
        for pros/cons of each predefined model.
      </p>
    </div>
  );
}
