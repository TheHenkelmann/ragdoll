// SPDX-License-Identifier: AGPL-3.0-only

import { useEffect, useRef, useState } from "react";
import {
  BackupRecord,
  BackupRetention,
  createBackup,
  deleteBackup,
  downloadBackup,
  listBackups,
  restoreBackup,
  uploadBackup,
} from "../api/client";
import { PermissionDenied } from "../components/PermissionDenied";
import { usePermissions } from "../hooks/usePermissions";
import { PERM } from "../permissions";
import { useSnackbar } from "../context/SnackbarContext";
import { pushApiError } from "../utils/snackbarFormat";

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

function triggerLabel(trigger: BackupRecord["trigger"]) {
  return trigger === "daily" ? "Daily" : "Manual";
}

export function BackupsPage() {
  const snackbar = useSnackbar();
  const { can, ready } = usePermissions();
  const canRead = can(PERM.backups.read);
  const canCreate = can(PERM.backups.create);
  const canUpload = can(PERM.backups.upload);
  const canDownload = can(PERM.backups.download);
  const canRestore = can(PERM.backups.restore);
  const canDelete = can(PERM.backups.delete);
  const [backups, setBackups] = useState<BackupRecord[]>([]);
  const [retention, setRetention] = useState<BackupRetention | null>(null);
  const [loading, setLoading] = useState(true);
  const [creating, setCreating] = useState(false);
  const [uploading, setUploading] = useState(false);
  const [restoreTarget, setRestoreTarget] = useState<BackupRecord | null>(null);
  const [restoreAcknowledged, setRestoreAcknowledged] = useState(false);
  const [createSafetyBackup, setCreateSafetyBackup] = useState(false);
  const [restoring, setRestoring] = useState(false);
  const [deleteTarget, setDeleteTarget] = useState<BackupRecord | null>(null);
  const [deleteConfirmText, setDeleteConfirmText] = useState("");
  const [deleting, setDeleting] = useState(false);
  const uploadInputRef = useRef<HTMLInputElement>(null);

  const reload = () => {
    if (!canRead) {
      setLoading(false);
      return;
    }
    setLoading(true);
    void listBackups()
      .then((response) => {
        setBackups(response.backups);
        setRetention(response.retention);
      })
      .catch((err) => pushApiError(snackbar.error, err))
      .finally(() => setLoading(false));
  };

  useEffect(() => {
    if (!ready) return;
    reload();
  }, [ready, canRead]);

  function closeRestoreModal() {
    setRestoreTarget(null);
    setRestoreAcknowledged(false);
    setCreateSafetyBackup(false);
  }

  function closeDeleteModal() {
    setDeleteTarget(null);
    setDeleteConfirmText("");
  }

  if (ready && !canRead) {
    return <PermissionDenied permission={PERM.backups.read} />;
  }

  return (
    <div className="space-y-6">
      <h2 className="text-2xl font-semibold">Backups</h2>
      <p className="text-sm text-[var(--muted)]">
        Local database snapshots. Daily backups run automatically once per UTC day when the server
        starts or on an hourly check. Manual backups are kept separately. Uploads must use the
        backup file name format{" "}
        <code className="text-xs">ragdoll-&lt;timestamp&gt;-&lt;daily|manual&gt;.db</code>.
      </p>

      {retention && (
        <div
          className="rounded-lg border px-4 py-3 text-sm"
          style={{ borderColor: "var(--border)", background: "var(--surface)" }}
        >
          <div className="font-medium">Retention</div>
          <p className="mt-1 text-[var(--muted)]">
            Ragdoll keeps the newest <strong>{retention.keep_daily}</strong> daily and{" "}
            <strong>{retention.keep_manual}</strong> manual backups. Older snapshots are removed
            automatically after each new backup of the same type. Configure via{" "}
            <code className="text-xs">RAGDOLL_BACKUP_KEEP_DAILY</code> and{" "}
            <code className="text-xs">RAGDOLL_BACKUP_KEEP_MANUAL</code>.
          </p>
        </div>
      )}

      <div className="flex flex-wrap gap-2">
        <button
          type="button"
          className="btn-primary"
          disabled={creating || !canCreate}
          onClick={() => {
            setCreating(true);
            void createBackup()
              .then(() => reload())
              .catch((err) => pushApiError(snackbar.error, err))
              .finally(() => setCreating(false));
          }}
        >
          {creating ? "Creating…" : "Create backup now"}
        </button>
        <button
          type="button"
          className="btn-secondary"
          disabled={uploading || !canUpload}
          onClick={() => uploadInputRef.current?.click()}
        >
          {uploading ? "Uploading…" : "Upload backup"}
        </button>
        <input
          ref={uploadInputRef}
          type="file"
          accept=".db,application/octet-stream"
          className="hidden"
          onChange={(event) => {
            const file = event.target.files?.[0];
            event.target.value = "";
            if (!file) return;
            setUploading(true);
            void uploadBackup(file)
              .then((uploaded) => {
                snackbar.success(`Uploaded backup as ${uploaded.file_name}.`);
                reload();
              })
              .catch((err) => pushApiError(snackbar.error, err))
              .finally(() => setUploading(false));
          }}
        />
      </div>

      <div className="space-y-2">
        {loading && <p className="text-sm text-[var(--muted)]">Loading…</p>}
        {!loading && backups.length === 0 && (
          <p className="text-sm text-[var(--muted)]">No backups yet.</p>
        )}
        {backups.map((backup) => (
          <div
            key={backup.file_name}
            className="flex flex-wrap items-center gap-3 rounded-lg border px-4 py-3"
            style={{ borderColor: "var(--border)", background: "var(--surface)" }}
          >
            <div className="min-w-0 flex-1">
              <div className="font-medium">{backup.file_name}</div>
              <div className="text-xs text-subtle">
                {triggerLabel(backup.trigger)} · {backup.created_at} ·{" "}
                {formatBytes(backup.size_bytes)}
              </div>
            </div>
            <span
              className="shrink-0 rounded px-2 py-0.5 text-xs font-medium"
              style={{
                background: backup.trigger === "daily" ? "var(--accent-soft)" : "var(--surface-2)",
                color: "var(--text)",
              }}
            >
              {triggerLabel(backup.trigger)}
            </span>
            <div className="flex shrink-0 flex-wrap gap-2">
              <button
                type="button"
                className="btn-secondary"
                disabled={!canDownload}
                onClick={() => {
                  void downloadBackup(backup.file_name).catch((err) =>
                    pushApiError(snackbar.error, err),
                  );
                }}
              >
                Download
              </button>
              <button
                type="button"
                className="btn-secondary"
                disabled={!canRestore}
                onClick={() => {
                  setRestoreAcknowledged(false);
                  setCreateSafetyBackup(false);
                  setRestoreTarget(backup);
                }}
              >
                Restore
              </button>
              <button
                type="button"
                className="btn-danger"
                disabled={!canDelete}
                onClick={() => {
                  setDeleteConfirmText("");
                  setDeleteTarget(backup);
                }}
              >
                Delete
              </button>
            </div>
          </div>
        ))}
      </div>

      {restoreTarget && (
        <div className="modal-overlay" onClick={closeRestoreModal}>
          <div
            className="card w-full max-w-lg space-y-4"
            onClick={(e) => e.stopPropagation()}
          >
            <h3 className="text-lg font-semibold">Restore backup?</h3>
            <div className="space-y-2 text-sm text-[var(--muted)]">
              <p>
                This replaces the <strong>live database</strong> with the selected snapshot. All
                current data that is not in this backup will be lost unless you create a safety
                backup first.
              </p>
              <p className="font-medium text-[var(--text)]">{restoreTarget.file_name}</p>
            </div>
            <label className="flex items-start gap-2 text-sm">
              <input
                type="checkbox"
                className="mt-1"
                checked={createSafetyBackup}
                onChange={(e) => setCreateSafetyBackup(e.target.checked)}
              />
              <span>Create a manual safety backup of the current database before restore.</span>
            </label>
            <label className="flex items-start gap-2 text-sm">
              <input
                type="checkbox"
                className="mt-1"
                checked={restoreAcknowledged}
                onChange={(e) => setRestoreAcknowledged(e.target.checked)}
              />
              <span>
                I understand this will overwrite the live database and cannot be undone except by
                restoring another backup.
              </span>
            </label>
            <div className="flex justify-end gap-2">
              <button type="button" className="btn-secondary" onClick={closeRestoreModal}>
                Cancel
              </button>
              <button
                type="button"
                className="btn-danger"
                disabled={!restoreAcknowledged || restoring}
                onClick={() => {
                  setRestoring(true);
                  void restoreBackup(restoreTarget.file_name, {
                    safetyBackup: createSafetyBackup,
                  })
                    .then((result) => {
                      closeRestoreModal();
                      snackbar.success(
                        result.safety_backup
                          ? `Restored ${result.restored_from}. Safety backup: ${result.safety_backup}.`
                          : `Restored ${result.restored_from}.`,
                      );
                      reload();
                    })
                    .catch((err) => pushApiError(snackbar.error, err))
                    .finally(() => setRestoring(false));
                }}
              >
                {restoring ? "Restoring…" : "Restore database"}
              </button>
            </div>
          </div>
        </div>
      )}

      {deleteTarget && (
        <div className="modal-overlay" onClick={closeDeleteModal}>
          <div
            className="card w-full max-w-lg space-y-4"
            onClick={(e) => e.stopPropagation()}
          >
            <h3 className="text-lg font-semibold">Delete backup?</h3>
            <div className="space-y-2 text-sm text-[var(--muted)]">
              <p>
                This permanently removes the snapshot file from the server. It cannot be recovered
                unless you still have a local copy.
              </p>
              <p className="font-medium text-[var(--text)]">{deleteTarget.file_name}</p>
            </div>
            <label className="block space-y-1 text-sm">
              <span>Type the file name to confirm:</span>
              <input
                type="text"
                className="input w-full"
                value={deleteConfirmText}
                placeholder={deleteTarget.file_name}
                onChange={(e) => setDeleteConfirmText(e.target.value)}
              />
            </label>
            <div className="flex justify-end gap-2">
              <button type="button" className="btn-secondary" onClick={closeDeleteModal}>
                Cancel
              </button>
              <button
                type="button"
                className="btn-danger"
                disabled={deleteConfirmText !== deleteTarget.file_name || deleting}
                onClick={() => {
                  setDeleting(true);
                  void deleteBackup(deleteTarget.file_name)
                    .then(() => {
                      closeDeleteModal();
                      snackbar.success(`Deleted ${deleteTarget.file_name}.`);
                      reload();
                    })
                    .catch((err) => pushApiError(snackbar.error, err))
                    .finally(() => setDeleting(false));
                }}
              >
                {deleting ? "Deleting…" : "Delete backup"}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
