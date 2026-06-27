// SPDX-License-Identifier: AGPL-3.0-only

import { ReactNode, useEffect, useMemo, useState } from "react";
import { Link, useParams } from "react-router-dom";
import { InfoTip } from "../components/InfoTip";
import { PermissionDenied } from "../components/PermissionDenied";
import { ReindexProgressPanel } from "../components/ReindexProgressPanel";
import {
  CatalogStatusEntry,
  ChunkRecord,
  RuntimeSettings,
  api,
  createBackup,
  getModelsStatus,
  triggerReindex,
} from "../api/client";
import { usePermissions } from "../hooks/usePermissions";
import { PERM } from "../permissions";

function modelOptions(present: string[], current: string): { name: string; disabled: boolean }[] {
  const options = present.map((name) => ({ name, disabled: false }));
  if (current && !present.includes(current)) {
    return [{ name: current, disabled: true }, ...options];
  }
  return options;
}

const DOCS_BASE = "https://github.com/TheHenkelmann/ragdoll/blob/main/docs";

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

function SettingRow({
  label,
  info,
  docHref,
  children,
}: {
  label: string;
  info: string;
  docHref: string;
  children: ReactNode;
}) {
  return (
    <label className="block space-y-1 text-sm">
      <span className="inline-flex items-center gap-2">
        {label}
        <InfoTip text={info} />
        <DocLink href={docHref} label={`Open docs: ${label}`} />
      </span>
      {children}
    </label>
  );
}

type ConfirmState =
  | { kind: "rerank-only-save" }
  | {
      kind: "reindex";
      trigger: "save" | "manual";
      embeddingChanged: boolean;
      rerankChanged: boolean;
    };

function reindexMessage(result: Awaited<ReturnType<typeof triggerReindex>>, prefix: string): string {
  const queued = result.items.filter((item) => item.result != null).length;
  const failed = result.items.filter((item) => item.error != null).length;
  if (queued === 0 && failed === 0) {
    return `${prefix} No sources with stored text to reindex.`;
  }
  if (failed > 0) {
    return `${prefix} ${queued} reindex job(s) queued, ${failed} skipped.`;
  }
  return `${prefix} ${queued} reindex job(s) queued.`;
}

export function SettingsPage() {
  const { releaseTag = "" } = useParams();
  const { can, ready } = usePermissions();
  const canRead = can(PERM.settings.read);
  const canWrite = can(PERM.settings.write);
  const canReindex = can(PERM.sources.write);
  const canReadChunks = can(PERM.chunks.read);
  const canCreateBackup = can(PERM.backups.create);

  const [savedSettings, setSavedSettings] = useState<RuntimeSettings | null>(null);
  const [settings, setSettings] = useState<RuntimeSettings | null>(null);
  const [catalog, setCatalog] = useState<CatalogStatusEntry[]>([]);
  // null = unknown (e.g. no chunks:read permission) → keep reindex enabled.
  const [hasChunks, setHasChunks] = useState<boolean | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [reindexing, setReindexing] = useState(false);
  const [creatingBackup, setCreatingBackup] = useState(false);
  const [backupMessage, setBackupMessage] = useState<string | null>(null);
  const [confirm, setConfirm] = useState<ConfirmState | null>(null);
  const [reindexBatchId, setReindexBatchId] = useState<string | null>(null);

  useEffect(() => {
    if (!ready || !canRead) return;
    void getModelsStatus()
      .then((status) => setCatalog(status.catalog))
      .catch(() => setCatalog([]));
  }, [ready, canRead]);

  useEffect(() => {
    if (!ready || !canReindex) return;
    if (!canReadChunks) {
      // Can't verify chunk presence → leave reindex enabled, info on empty result.
      setHasChunks(null);
      return;
    }
    void api<ChunkRecord[]>(`/releases/${releaseTag}/chunks?limit=1`)
      .then((chunks) => setHasChunks(chunks.length > 0))
      .catch(() => setHasChunks(null));
  }, [ready, canReindex, canReadChunks, releaseTag]);

  useEffect(() => {
    if (!ready || !canRead) return;
    void api<RuntimeSettings>(`/releases/${releaseTag}/settings`)
      .then((loaded) => {
        setSavedSettings(loaded);
        setSettings(loaded);
      })
      .catch((err) => setError(String(err)));
  }, [releaseTag, ready, canRead]);

  const embeddingChanged =
    savedSettings != null && settings != null && settings.embedding_model !== savedSettings.embedding_model;
  const rerankChanged =
    savedSettings != null && settings != null && settings.rerank_model !== savedSettings.rerank_model;
  const modelChanged = embeddingChanged || rerankChanged;
  const confirmBusy = saving || reindexing;

  const presentEmbedModels = useMemo(
    () => catalog.filter((c) => c.kind === "embed" && c.present).map((c) => c.name),
    [catalog],
  );
  const presentRerankModels = useMemo(
    () => catalog.filter((c) => c.kind === "rerank" && c.present).map((c) => c.name),
    [catalog],
  );

  function selectedModelsPresent(): boolean {
    if (!settings) return true;
    const embedOk = catalog.find((c) => c.name === settings.embedding_model)?.present ?? false;
    const rerankOk = catalog.find((c) => c.name === settings.rerank_model)?.present ?? false;
    return embedOk && rerankOk;
  }

  async function runReindex(prefix: string) {
    const result = await triggerReindex(releaseTag);
    const queued = result.items.filter((item) => item.result != null).length;
    // Only subscribe to batch events if jobs were actually queued; an empty
    // batch is never persisted, so its events endpoint returns 404.
    if (queued > 0) {
      setReindexBatchId(result.batch_id);
    } else {
      setReindexBatchId(null);
      setHasChunks(false);
    }
    setMessage(reindexMessage(result, prefix));
  }

  async function performSave(confirmed?: ConfirmState) {
    if (!settings || !savedSettings) return;
    if (!selectedModelsPresent()) {
      setError(
        "Selected embedding or rerank model is not downloaded. Download models on the Models page first.",
      );
      return;
    }
    const embedChanged = confirmed
      ? confirmed.kind === "reindex" && confirmed.embeddingChanged
      : settings.embedding_model !== savedSettings.embedding_model;

    setSaving(true);
    setError(null);
    setMessage(null);
    try {
      const updated = await api<RuntimeSettings>(`/releases/${releaseTag}/settings`, {
        method: "PATCH",
        body: JSON.stringify({
          chunking_strategy: settings.chunking_strategy,
          payload_storage: settings.payload_storage,
          embedding_model: settings.embedding_model,
          rerank_model: settings.rerank_model,
          generation_allowed: settings.generation_allowed,
          rerank_max_length: settings.rerank_max_length,
        }),
      });
      setSavedSettings(updated);
      setSettings(updated);

      if (embedChanged) {
        if (!canReindex) {
          setMessage(
            "Settings saved. Reindex requires sources:write — trigger reindex manually when ready.",
          );
        } else {
          await runReindex("Settings saved.");
        }
      } else {
        setMessage("Saved");
      }
    } catch (err) {
      setError(String(err));
    } finally {
      setSaving(false);
      setConfirm(null);
    }
  }

  async function performManualReindex() {
    setReindexing(true);
    setError(null);
    setMessage(null);
    try {
      await runReindex("Reindex started.");
    } catch (err) {
      setError(String(err));
    } finally {
      setReindexing(false);
      setConfirm(null);
    }
  }

  function requestSave() {
    if (!settings || !savedSettings) return;
    if (!selectedModelsPresent()) {
      setError(
        "Selected embedding or rerank model is not downloaded. Download models on the Models page first.",
      );
      return;
    }
    if (embeddingChanged) {
      setBackupMessage(null);
      setConfirm({
        kind: "reindex",
        trigger: "save",
        embeddingChanged,
        rerankChanged,
      });
      return;
    }
    if (rerankChanged) {
      setConfirm({ kind: "rerank-only-save" });
      return;
    }
    void performSave();
  }

  function requestManualReindex() {
    setBackupMessage(null);
    setConfirm({
      kind: "reindex",
      trigger: "manual",
      embeddingChanged: false,
      rerankChanged: false,
    });
  }

  function closeConfirm() {
    if (confirmBusy) return;
    setConfirm(null);
    setBackupMessage(null);
  }

  function handleConfirm() {
    if (!confirm) return;
    if (confirm.kind === "rerank-only-save") {
      void performSave();
      return;
    }
    if (confirm.trigger === "manual") {
      void performManualReindex();
      return;
    }
    void performSave(confirm);
  }

  function handleCreateBackup() {
    setCreatingBackup(true);
    setBackupMessage(null);
    void createBackup()
      .then((record) => {
        setBackupMessage(`Backup created: ${record.file_name}`);
      })
      .catch((err) => setBackupMessage(String(err)))
      .finally(() => setCreatingBackup(false));
  }

  if (ready && !canRead) {
    return <PermissionDenied permission={PERM.settings.read} />;
  }

  if (!settings || !savedSettings) {
    return <p className="text-sm text-[var(--muted)]">{error ?? "Loading…"}</p>;
  }

  const readOnly = !canWrite;
  const showReindexConfirm = confirm?.kind === "reindex";

  return (
    <div className="space-y-6">
      <h2 className="text-2xl font-semibold">Settings</h2>

      <div className="card max-w-xl space-y-4">
        <SettingRow
          label="Chunking strategy"
          info="How incoming source content is split into chunks before embedding."
          docHref={`${DOCS_BASE}/chunking.md`}
        >
          <select
            className="input"
            disabled={readOnly}
            value={settings.chunking_strategy}
            onChange={(e) => setSettings({ ...settings, chunking_strategy: e.target.value })}
          >
            <option value="semantic_split">Semantic Split</option>
          </select>
        </SettingRow>
        <SettingRow
          label="Payload storage"
          info="Whether query text and matched chunk content are persisted in the database."
          docHref={`${DOCS_BASE}/querying.md#payload-storage-policy`}
        >
          <select
            className="input"
            disabled={readOnly}
            value={settings.payload_storage}
            onChange={(e) =>
              setSettings({
                ...settings,
                payload_storage: e.target.value as RuntimeSettings["payload_storage"],
              })
            }
          >
            <option value="per_request">per_request</option>
            <option value="forced">forced</option>
            <option value="forbidden">forbidden</option>
          </select>
        </SettingRow>
        <SettingRow
          label="Generation allowed"
          info="When disabled, requests with a generation object are rejected for this release."
          docHref={`${DOCS_BASE}/querying.md`}
        >
          <select
            className="input"
            disabled={readOnly}
            value={settings.generation_allowed ? "true" : "false"}
            onChange={(e) =>
              setSettings({ ...settings, generation_allowed: e.target.value === "true" })
            }
          >
            <option value="true">Allowed</option>
            <option value="false">Disabled</option>
          </select>
        </SettingRow>

        <div className="space-y-2 border-t pt-4" style={{ borderColor: "var(--border)" }}>
          <h3 className="text-base font-medium">Advanced Settings</h3>
          <p className="text-sm text-[var(--muted)]">
            Changing embedding or rerank models affects latency, compatibility, and re-embedding
            requirements. Models must be downloaded on the global{" "}
            <Link to="/models" className="underline hover:text-[var(--text)]">
              Models page
            </Link>{" "}
            before they appear here.
          </p>
        </div>

        <SettingRow
          label="Embedding model (dim 1024 only)"
          info="Model used to embed chunks and queries. Must produce 1024-dimensional vectors and be downloaded locally."
          docHref={`${DOCS_BASE}/models.md`}
        >
          <select
            className="input"
            disabled={readOnly}
            value={settings.embedding_model}
            onChange={(e) => setSettings({ ...settings, embedding_model: e.target.value })}
          >
            {modelOptions(presentEmbedModels, settings.embedding_model).map((m) => (
              <option key={m.name} value={m.name} disabled={m.disabled}>
                {m.name}
                {m.disabled ? " (not downloaded)" : ""}
              </option>
            ))}
          </select>
        </SettingRow>
        <SettingRow
          label="Rerank model"
          info="Cross-encoder model used to rerank semantic candidates. Must be downloaded locally."
          docHref={`${DOCS_BASE}/models.md`}
        >
          <select
            className="input"
            disabled={readOnly}
            value={settings.rerank_model}
            onChange={(e) => setSettings({ ...settings, rerank_model: e.target.value })}
          >
            {modelOptions(presentRerankModels, settings.rerank_model).map((m) => (
              <option key={m.name} value={m.name} disabled={m.disabled}>
                {m.name}
                {m.disabled ? " (not downloaded)" : ""}
              </option>
            ))}
          </select>
        </SettingRow>
        <SettingRow
          label="Rerank max length"
          info="Maximum token length per document sent to the reranker. Lower values reduce latency; 256 is recommended."
          docHref={`${DOCS_BASE}/querying.md`}
        >
          <select
            className="input"
            disabled={readOnly}
            value={settings.rerank_max_length}
            onChange={(e) =>
              setSettings({ ...settings, rerank_max_length: Number(e.target.value) })
            }
          >
            <option value={0}>Uncapped</option>
            <option value={1024}>1024</option>
            <option value={512}>512</option>
            <option value={256}>256 (recommended)</option>
            <option value={128}>128</option>
          </select>
        </SettingRow>

        {canReindex && (
          <div className="space-y-2 border-t pt-4" style={{ borderColor: "var(--border)" }}>
            <h4 className="text-sm font-medium">Reindex</h4>
            <p className="text-sm text-[var(--muted)]">
              Re-embed all sources that have stored extracted text using the current embedding model.
              Use this after data recovery or if embeddings are out of sync.
            </p>
            <button
              type="button"
              className="btn-secondary"
              disabled={reindexing || saving || hasChunks === false}
              onClick={requestManualReindex}
            >
              {reindexing ? "Starting reindex…" : "Trigger reindex"}
            </button>
            {hasChunks === false && (
              <p className="text-xs text-[var(--muted)]">
                No chunks available to reindex yet. Ingest sources first.
              </p>
            )}
          </div>
        )}

        {error && <p className="text-sm text-error">{error}</p>}
        <button
          className="btn-primary"
          disabled={readOnly || saving || reindexing || !selectedModelsPresent()}
          onClick={() => requestSave()}
        >
          {saving ? "Saving…" : "Save"}
        </button>
        {!selectedModelsPresent() && (
          <p className="text-xs text-[var(--muted)]">
            Download the selected models on the{" "}
            <Link to="/models" className="underline hover:text-[var(--text)]">
              Models page
            </Link>{" "}
            before saving.
          </p>
        )}
        {message && <p className="text-sm text-[var(--muted)]">{message}</p>}
      </div>

      {reindexBatchId && (
        <ReindexProgressPanel releaseTag={releaseTag} batchId={reindexBatchId} />
      )}

      {confirm && (
        <div className="modal-overlay" onClick={closeConfirm}>
          <div
            className="card w-full max-w-lg space-y-4"
            onClick={(e) => e.stopPropagation()}
          >
            {showReindexConfirm ? (
              <>
                <h3 className="text-lg font-semibold">
                  {confirm.trigger === "manual" ? "Confirm reindex" : "Confirm model change & reindex"}
                </h3>
                <div className="space-y-3 text-sm text-[var(--muted)]">
                  {confirm.trigger === "save" && confirm.embeddingChanged && (
                    <p>
                      Changing the <strong className="text-[var(--text)]">embedding model</strong>{" "}
                      requires re-indexing all sources. Every chunk is re-embedded.
                    </p>
                  )}
                  {confirm.trigger === "save" && confirm.rerankChanged && (
                    <p>
                      Changing the <strong className="text-[var(--text)]">rerank model</strong>{" "}
                      takes effect immediately for queries. No re-indexing is needed.
                    </p>
                  )}
                  <p>
                    Reindexing can take a long time depending on the number of sources. While jobs
                    are running, <strong className="text-[var(--text)]">queries may be unreliable</strong>{" "}
                    because chunks are being replaced incrementally.
                  </p>
                  <p>
                    Consider creating a backup before proceeding so you can restore if something
                    goes wrong.
                  </p>
                  <div className="flex flex-wrap items-center gap-2 pt-1">
                    <button
                      type="button"
                      className="btn-secondary"
                      disabled={creatingBackup || !canCreateBackup}
                      onClick={handleCreateBackup}
                    >
                      {creatingBackup ? "Creating backup…" : "Create backup now"}
                    </button>
                    {!canCreateBackup && (
                      <span className="text-xs">Requires backups:create permission.</span>
                    )}
                  </div>
                  {backupMessage && <p className="text-xs">{backupMessage}</p>}
                </div>
                <div className="flex justify-end gap-2">
                  <button
                    type="button"
                    className="btn-secondary"
                    disabled={confirmBusy}
                    onClick={closeConfirm}
                  >
                    Cancel
                  </button>
                  <button
                    type="button"
                    className="btn-primary"
                    disabled={confirmBusy}
                    onClick={handleConfirm}
                  >
                    {confirmBusy
                      ? "Working…"
                      : confirm.trigger === "manual"
                        ? "Start reindex"
                        : "Save and reindex"}
                  </button>
                </div>
              </>
            ) : (
              <>
                <h3 className="text-lg font-semibold">Confirm model change</h3>
                <div className="space-y-3 text-sm text-[var(--muted)]">
                  <p>
                    Changing the <strong className="text-[var(--text)]">rerank model</strong>{" "}
                    takes effect immediately for queries. No re-indexing is needed.
                  </p>
                  <p>Do you want to save and proceed?</p>
                </div>
                <div className="flex justify-end gap-2">
                  <button
                    type="button"
                    className="btn-secondary"
                    disabled={confirmBusy}
                    onClick={closeConfirm}
                  >
                    Cancel
                  </button>
                  <button
                    type="button"
                    className="btn-primary"
                    disabled={confirmBusy}
                    onClick={handleConfirm}
                  >
                    {saving ? "Working…" : "Save and proceed"}
                  </button>
                </div>
              </>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
