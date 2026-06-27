// SPDX-License-Identifier: AGPL-3.0-only

import { FormEvent, useCallback, useEffect, useMemo, useState } from "react";
import { useParams, useSearchParams } from "react-router-dom";
import { ChunkRecord, SourceRecord, api } from "../api/client";
import { PermissionDenied } from "../components/PermissionDenied";
import { usePermissions } from "../hooks/usePermissions";
import { PERM } from "../permissions";
import { useSnackbar } from "../context/SnackbarContext";
import { resolveFileSourceName } from "../utils/sourceUpload";
import { pushApiError } from "../utils/snackbarFormat";

type CreateMode = "text" | "url" | "file";

const POLL_MS = 2000;
const ACTIVE_STATUSES = new Set(["pending", "processing"]);

function readFileAsBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const result = reader.result;
      if (typeof result !== "string") {
        reject(new Error("failed to read file"));
        return;
      }
      const base64 = result.split(",")[1];
      if (!base64) {
        reject(new Error("failed to encode file"));
        return;
      }
      resolve(base64);
    };
    reader.onerror = () => reject(reader.error ?? new Error("failed to read file"));
    reader.readAsDataURL(file);
  });
}

function StatusSpinner() {
  return (
    <span
      className="inline-block h-3 w-3 animate-spin rounded-full border-2 border-[var(--muted)] border-t-transparent"
      aria-hidden
    />
  );
}

export function SourcesPage() {
  const snackbar = useSnackbar();
  const { releaseTag = "" } = useParams();
  const { can, ready } = usePermissions();
  const canReadSources = can(PERM.sources.read);
  const canWriteSources = can(PERM.sources.write);
  const canDeleteSources = can(PERM.sources.delete);
  const canReadChunks = can(PERM.chunks.read);
  const [params, setParams] = useSearchParams();
  const [sources, setSources] = useState<SourceRecord[]>([]);
  const [chunks, setChunks] = useState<ChunkRecord[]>([]);
  const [selected, setSelected] = useState<string | null>(params.get("source"));
  const filter = params.get("q") ?? "";
  const [createOpen, setCreateOpen] = useState(false);
  const [createMode, setCreateMode] = useState<CreateMode>("text");
  const [createName, setCreateName] = useState("");
  const [createText, setCreateText] = useState("");
  const [createUrl, setCreateUrl] = useState("");
  const [createFile, setCreateFile] = useState<File | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<SourceRecord | null>(null);
  const [busy, setBusy] = useState(false);

  const reloadSources = useCallback(() => {
    if (!canReadSources) return;
    void api<SourceRecord[]>(`/releases/${releaseTag}/sources?limit=200`)
      .then(setSources)
      .catch((err) => pushApiError(snackbar.error, err));
  }, [releaseTag, canReadSources]);

  const reloadChunks = useCallback(
    (sourceId: string) => {
      if (!canReadChunks) return;
      void api<ChunkRecord[]>(
        `/releases/${releaseTag}/chunks?limit=200&filter=${encodeURIComponent(JSON.stringify({ field: "source_id", op: "eq", value: sourceId }))}`,
      )
        .then(setChunks)
        .catch(console.error);
    },
    [releaseTag, canReadChunks],
  );

  useEffect(() => {
    if (!ready) return;
    reloadSources();
  }, [ready, reloadSources]);

  const hasActiveIngest = useMemo(
    () => sources.some((s) => ACTIVE_STATUSES.has(s.status)),
    [sources],
  );

  useEffect(() => {
    if (!hasActiveIngest) return;
    const timer = window.setInterval(() => reloadSources(), POLL_MS);
    return () => window.clearInterval(timer);
  }, [hasActiveIngest, reloadSources]);

  useEffect(() => {
    if (!selected) {
      setChunks([]);
      return;
    }
    reloadChunks(selected);
  }, [releaseTag, selected, reloadChunks]);

  const selectedSource = sources.find((s) => s.id === selected);

  useEffect(() => {
    if (selected && selectedSource?.status === "completed") {
      reloadChunks(selected);
    }
  }, [selected, selectedSource?.status, reloadChunks]);

  const filteredSources = useMemo(() => {
    const q = filter.toLowerCase();
    return sources.filter((s) => !q || s.name.toLowerCase().includes(q) || s.id.includes(q));
  }, [sources, filter]);

  if (ready && !canReadSources) {
    return <PermissionDenied permission={PERM.sources.read} />;
  }

  async function submitCreate(e: FormEvent) {
    e.preventDefault();
    setBusy(true);
    try {
      let payload: Record<string, string>[];
      if (createMode === "text") {
        payload = [{ type: "text", name: createName, content: createText }];
      } else if (createMode === "url") {
        payload = [{ type: "url", name: createName, url: createUrl }];
      } else {
        if (!createFile) throw new Error("Select a file");
        const content = await readFileAsBase64(createFile);
        const name = resolveFileSourceName(createName, createFile);
        payload = [{ type: "file", name, content }];
      }
      await api(`/releases/${releaseTag}/sources`, {
        method: "POST",
        body: JSON.stringify(payload),
      });
      setCreateOpen(false);
      setCreateName("");
      setCreateText("");
      setCreateUrl("");
      setCreateFile(null);
      reloadSources();
    } catch (err) {
      pushApiError(snackbar.error, err);
    } finally {
      setBusy(false);
    }
  }

  async function confirmDelete() {
    if (!deleteTarget) return;
    try {
      await api(
        `/releases/${releaseTag}/sources?filter=${encodeURIComponent(JSON.stringify({ field: "id", op: "eq", value: deleteTarget.id }))}`,
        { method: "DELETE" },
      );
      if (selected === deleteTarget.id) {
        setSelected(null);
        params.delete("source");
        setParams(params, { replace: true });
      }
      setDeleteTarget(null);
      reloadSources();
    } catch (err) {
      pushApiError(snackbar.error, err);
    }
  }

  return (
    <div className="space-y-6">
      <h2 className="text-2xl font-semibold">Sources</h2>
      <div className="flex flex-wrap items-center gap-3">
        <button
          type="button"
          className="btn-primary"
          disabled={!canWriteSources}
          onClick={() => setCreateOpen(true)}
        >
          Create source
        </button>
        <input
          className="input max-w-md flex-1"
          placeholder="Filter sources by name or id"
          value={filter}
          onChange={(e) => {
            params.set("q", e.target.value);
            setParams(params, { replace: true });
          }}
        />
      </div>
      <div className="grid gap-6 lg:grid-cols-2">
        <div className="card space-y-2">
          {filteredSources.map((s) => (
            <div
              key={s.id}
              className={`flex items-center gap-2 rounded-lg border px-3 py-2 ${selected === s.id ? "btn-toggle-active border-[var(--border)]" : ""}`}
              style={{ borderColor: "var(--border)" }}
            >
              <button
                type="button"
                className="min-w-0 flex-1 text-left"
                onClick={() => {
                  setSelected(s.id);
                  params.set("source", s.id);
                  setParams(params, { replace: true });
                }}
              >
                <div className="font-medium">{s.name}</div>
                <div className="flex items-center gap-1.5 text-xs text-[var(--muted)]">
                  {ACTIVE_STATUSES.has(s.status) && <StatusSpinner />}
                  {s.type} · {s.status} · {s.chunk_count} {s.chunk_count === 1 ? "chunk" : "chunks"}
                </div>
              </button>
              <button
                type="button"
                className="btn-danger shrink-0 text-xs"
                disabled={!canDeleteSources}
                onClick={() => setDeleteTarget(s)}
              >
                Delete
              </button>
            </div>
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

      {createOpen && (
        <div className="modal-overlay" onClick={() => setCreateOpen(false)}>
          <form
            className="card w-full max-w-lg space-y-4"
            onClick={(e) => e.stopPropagation()}
            onSubmit={(e) => void submitCreate(e)}
          >
            <h3 className="text-lg font-semibold">Create source</h3>
            <div className="flex flex-wrap gap-2">
              {(["text", "url", "file"] as CreateMode[]).map((mode) => (
                <button
                  key={mode}
                  type="button"
                  className={`btn-secondary ${createMode === mode ? "btn-toggle-active" : ""}`}
                  onClick={() => setCreateMode(mode)}
                >
                  {mode}
                </button>
              ))}
            </div>
            <label className="block space-y-1 text-sm">
              <span>{createMode === "file" ? "Name (optional, extension added from file)" : "Name"}</span>
              <input
                className="input"
                value={createName}
                onChange={(e) => setCreateName(e.target.value)}
                required={createMode !== "file"}
                placeholder={createMode === "file" ? "Uses filename if empty" : undefined}
              />
            </label>
            {createMode === "text" && (
              <label className="block space-y-1 text-sm">
                <span>Content</span>
                <textarea className="input min-h-32" value={createText} onChange={(e) => setCreateText(e.target.value)} required />
              </label>
            )}
            {createMode === "url" && (
              <label className="block space-y-1 text-sm">
                <span>URL</span>
                <input className="input" type="url" value={createUrl} onChange={(e) => setCreateUrl(e.target.value)} required />
              </label>
            )}
            {createMode === "file" && (
              <label className="block space-y-1 text-sm">
                <span>File</span>
                <input
                  className="input"
                  type="file"
                  accept=".txt,.md,.csv,.json,.pdf,.docx,.xlsx,.xlsm,.pptx"
                  onChange={(e) => {
                    const file = e.target.files?.[0] ?? null;
                    setCreateFile(file);
                    if (file) setCreateName(file.name);
                  }}
                  required
                />
              </label>
            )}
            <div className="flex justify-end gap-2">
              <button type="button" className="btn-secondary" onClick={() => setCreateOpen(false)}>
                Cancel
              </button>
              <button type="submit" className="btn-primary" disabled={busy}>
                Create
              </button>
            </div>
          </form>
        </div>
      )}

      {deleteTarget && (
        <div className="modal-overlay" onClick={() => setDeleteTarget(null)}>
          <div className="card w-full max-w-md space-y-4" onClick={(e) => e.stopPropagation()}>
            <h3 className="text-lg font-semibold">Delete source?</h3>
            <p className="text-sm text-[var(--muted)]">
              Delete <strong>{deleteTarget.name}</strong>? This cannot be undone.
            </p>
            <div className="flex justify-end gap-2">
              <button type="button" className="btn-secondary" onClick={() => setDeleteTarget(null)}>
                Cancel
              </button>
              <button type="button" className="btn-danger" onClick={() => void confirmDelete()}>
                Delete
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
