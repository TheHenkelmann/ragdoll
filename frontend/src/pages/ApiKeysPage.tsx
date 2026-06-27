// SPDX-License-Identifier: AGPL-3.0-only

import { FormEvent, useEffect, useState } from "react";
import { ApiKeyRecord, CreateApiKeyResponse, api } from "../api/client";
import { PermissionDenied } from "../components/PermissionDenied";
import { PermissionEditor } from "../components/PermissionEditor";
import { usePermissions } from "../hooks/usePermissions";
import { PERM } from "../permissions";
import { useSnackbar } from "../context/SnackbarContext";
import { maskApiKeyToken } from "../utils/password";
import { pushApiError } from "../utils/snackbarFormat";

function parseRateLimitInput(raw: string): number | null {
  const trimmed = raw.trim();
  if (!trimmed) return null;
  const n = Number(trimmed);
  return Number.isFinite(n) && n > 0 ? Math.floor(n) : null;
}

function formatRateLimit(value: number | null | undefined): string {
  if (value == null) return "Unlimited";
  return String(value);
}

function rateLimitInputValue(value: number | null | undefined): string {
  return value == null ? "" : String(value);
}

export function ApiKeysPage() {
  const snackbar = useSnackbar();
  const { can, ready } = usePermissions();
  const canRead = can(PERM.apiKeys.read);
  const canWrite = can(PERM.apiKeys.write);
  const canDelete = can(PERM.apiKeys.delete);
  const [keys, setKeys] = useState<ApiKeyRecord[]>([]);
  const [loading, setLoading] = useState(true);
  const [createdToken, setCreatedToken] = useState<CreateApiKeyResponse | null>(null);
  const [copied, setCopied] = useState(false);
  const [revealed, setRevealed] = useState(false);
  const [deleteTarget, setDeleteTarget] = useState<ApiKeyRecord | null>(null);
  const [deleteConfirm, setDeleteConfirm] = useState("");
  const [creating, setCreating] = useState(false);
  const [createName, setCreateName] = useState("");
  const [createPermissions, setCreatePermissions] = useState<string[]>([]);
  const [createRpm, setCreateRpm] = useState("");
  const [createRph, setCreateRph] = useState("");
  const [editTarget, setEditTarget] = useState<ApiKeyRecord | null>(null);
  const [editPermissions, setEditPermissions] = useState<string[]>([]);
  const [editRpm, setEditRpm] = useState("");
  const [editRph, setEditRph] = useState("");

  const reload = () => {
    if (!canRead) {
      setLoading(false);
      return;
    }
    setLoading(true);
    void api<ApiKeyRecord[]>("/api_keys")
      .then(setKeys)
      .catch((err) => pushApiError(snackbar.error, err))
      .finally(() => setLoading(false));
  };

  useEffect(() => {
    if (!ready) return;
    reload();
  }, [ready, canRead]);

  async function copyToken(token: string) {
    await navigator.clipboard.writeText(token);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  }

  function openCreate() {
    setCreating(true);
    setCreateName("");
    setCreatePermissions([]);
    setCreateRpm("");
    setCreateRph("");
  }

  function openEdit(key: ApiKeyRecord) {
    setEditTarget(key);
    setEditPermissions(key.permissions ?? []);
    setEditRpm(rateLimitInputValue(key.rpm));
    setEditRph(rateLimitInputValue(key.rph));
  }

  async function createKey(e: FormEvent) {
    e.preventDefault();
    const name = createName.trim();
    if (!name) return;
    if (keys.some((k) => k.name === name)) {
      snackbar.error(`An API key named "${name}" already exists`);
      return;
    }
    if (createPermissions.length === 0) {
      snackbar.error("Select at least one permission");
      return;
    }
    try {
      const res = await api<CreateApiKeyResponse>("/api_keys", {
        method: "POST",
        body: JSON.stringify({
          name,
          permissions: createPermissions,
          rpm: parseRateLimitInput(createRpm),
          rph: parseRateLimitInput(createRph),
        }),
      });
      setCreatedToken(res);
      setRevealed(false);
      setCreating(false);
      reload();
    } catch (err) {
      pushApiError(snackbar.error, err);
    }
  }

  async function saveEdit(e: FormEvent) {
    e.preventDefault();
    if (!editTarget) return;
    if (editPermissions.length === 0) {
      snackbar.error("Select at least one permission");
      return;
    }
    try {
      await api<ApiKeyRecord>(`/api_keys/${editTarget.id}`, {
        method: "PATCH",
        body: JSON.stringify({
          permissions: editPermissions,
          rpm: parseRateLimitInput(editRpm),
          rph: parseRateLimitInput(editRph),
        }),
      });
      setEditTarget(null);
      reload();
    } catch (err) {
      pushApiError(snackbar.error, err);
    }
  }

  const deletePhrase = deleteTarget ? `apikey/${deleteTarget.name}` : "";

  if (ready && !canRead) {
    return <PermissionDenied permission={PERM.apiKeys.read} />;
  }

  return (
    <div className="space-y-6">
      <h2 className="text-2xl font-semibold">API Keys</h2>
      <p className="text-sm text-[var(--muted)]">
        API keys authenticate stage-plane writes. The secret token is shown only once when created.
      </p>

      {!creating ? (
        <button type="button" className="btn-primary" disabled={!canWrite} onClick={openCreate}>
          Create API key
        </button>
      ) : (
        <form className="card max-w-lg space-y-4" onSubmit={(e) => void createKey(e)}>
          <h3 className="text-lg font-semibold">Create API key</h3>
          <label className="block space-y-1 text-sm">
            <span>Name</span>
            <input
              className="input"
              value={createName}
              onChange={(e) => setCreateName(e.target.value)}
              maxLength={100}
              required
              autoComplete="off"
            />
          </label>
          <div className="grid gap-3 sm:grid-cols-2">
            <label className="block space-y-1 text-sm">
              <span>RPM limit</span>
              <input
                className="input"
                type="number"
                min={1}
                value={createRpm}
                onChange={(e) => setCreateRpm(e.target.value)}
                placeholder="Unlimited"
              />
            </label>
            <label className="block space-y-1 text-sm">
              <span>RPH limit</span>
              <input
                className="input"
                type="number"
                min={1}
                value={createRph}
                onChange={(e) => setCreateRph(e.target.value)}
                placeholder="Unlimited"
              />
            </label>
          </div>
          <PermissionEditor
            idPrefix="create-key"
            value={createPermissions}
            onChange={setCreatePermissions}
            forceReleasesRead={false}
          />
          <div className="flex items-center gap-2">
            <button
              type="submit"
              className="btn-primary"
              disabled={createPermissions.length === 0}
            >
              Create
            </button>
            <button type="button" className="btn-secondary" onClick={() => setCreating(false)}>
              Cancel
            </button>
            {createPermissions.length === 0 && (
              <span className="text-xs text-[var(--muted)]">
                Select at least one permission
              </span>
            )}
          </div>
        </form>
      )}

      {createdToken && (
        <div className="card space-y-3 border" style={{ borderColor: "var(--border)" }}>
          <p className="text-sm font-medium">New key: {createdToken.name}</p>
          <p className="text-sm text-[var(--muted)]">
            Copy this token now. It will not be shown again.
          </p>
          <div className="token-once flex items-center justify-between gap-3">
            <span className="min-w-0 font-mono text-sm tracking-wider">
              {revealed ? createdToken.token : maskApiKeyToken()}
            </span>
            <button
              type="button"
              className="btn-secondary shrink-0 text-xs"
              onClick={() => setRevealed((v) => !v)}
            >
              {revealed ? "Hide" : "Reveal"}
            </button>
          </div>
          <div className="flex gap-2">
            <button
              type="button"
              className="btn-secondary"
              onClick={() => void copyToken(createdToken.token)}
            >
              {copied ? "Copied" : "Copy token"}
            </button>
            <button type="button" className="btn-secondary" onClick={() => setCreatedToken(null)}>
              Dismiss
            </button>
          </div>
        </div>
      )}

      <div className="space-y-2">
        {loading && <p className="text-sm text-[var(--muted)]">Loading…</p>}
        {!loading && keys.length === 0 && (
          <p className="text-sm text-[var(--muted)]">No API keys yet.</p>
        )}
        {keys.map((k) => (
          <div
            key={k.id}
            className="flex items-center gap-3 rounded-lg border px-4 py-3"
            style={{ borderColor: "var(--border)", background: "var(--surface)" }}
          >
            <div className="min-w-0 flex-1">
              <div className="font-medium">{k.name}</div>
              <div className="text-xs text-subtle">
                {k.created_at}
                {(k.permissions?.length ?? 0) > 0 && ` · ${k.permissions.length} permissions`}
                {` · RPM ${formatRateLimit(k.rpm)} · RPH ${formatRateLimit(k.rph)}`}
              </div>
            </div>
            <button type="button" className="btn-secondary shrink-0" disabled={!canWrite} onClick={() => openEdit(k)}>
              Edit
            </button>
            <button
              type="button"
              className="btn-danger shrink-0"
              disabled={!canDelete}
              onClick={() => {
                setDeleteTarget(k);
                setDeleteConfirm("");
              }}
            >
              Delete
            </button>
          </div>
        ))}
      </div>

      {editTarget && (
        <div className="modal-overlay" onClick={() => setEditTarget(null)}>
          <form
            className="card w-full max-w-lg space-y-4"
            onClick={(e) => e.stopPropagation()}
            onSubmit={(e) => void saveEdit(e)}
          >
            <h3 className="text-lg font-semibold">Edit API key: {editTarget.name}</h3>
            <div className="grid gap-3 sm:grid-cols-2">
              <label className="block space-y-1 text-sm">
                <span>RPM limit</span>
                <input
                  className="input"
                  type="number"
                  min={1}
                  value={editRpm}
                  onChange={(e) => setEditRpm(e.target.value)}
                  placeholder="Unlimited"
                />
              </label>
              <label className="block space-y-1 text-sm">
                <span>RPH limit</span>
                <input
                  className="input"
                  type="number"
                  min={1}
                  value={editRph}
                  onChange={(e) => setEditRph(e.target.value)}
                  placeholder="Unlimited"
                />
              </label>
            </div>
            <PermissionEditor
              idPrefix={`edit-key-${editTarget.id}`}
              value={editPermissions}
              onChange={setEditPermissions}
              forceReleasesRead={false}
            />
            <div className="flex items-center justify-end gap-2">
              {editPermissions.length === 0 && (
                <span className="mr-auto text-xs text-[var(--muted)]">
                  Select at least one permission
                </span>
              )}
              <button type="button" className="btn-secondary" onClick={() => setEditTarget(null)}>
                Cancel
              </button>
              <button
                type="submit"
                className="btn-primary"
                disabled={editPermissions.length === 0}
              >
                Save
              </button>
            </div>
          </form>
        </div>
      )}

      {deleteTarget && (
        <div
          className="modal-overlay"
          onClick={() => {
            setDeleteTarget(null);
            setDeleteConfirm("");
          }}
        >
          <div className="card w-full max-w-md space-y-4" onClick={(e) => e.stopPropagation()}>
            <h3 className="text-lg font-semibold">Delete API key?</h3>
            <p className="text-sm text-[var(--muted)]">
              Type <code className="rounded px-1" style={{ background: "var(--selected)" }}>{deletePhrase}</code> to
              confirm. This cannot be undone.
            </p>
            <input
              className="input font-mono text-sm"
              value={deleteConfirm}
              onChange={(e) => setDeleteConfirm(e.target.value)}
              placeholder={deletePhrase}
              autoComplete="off"
            />
            <div className="flex justify-end gap-2">
              <button
                type="button"
                className="btn-secondary"
                onClick={() => {
                  setDeleteTarget(null);
                  setDeleteConfirm("");
                }}
              >
                Cancel
              </button>
              <button
                type="button"
                className="btn-danger"
                disabled={deleteConfirm !== deletePhrase}
                onClick={() => {
                  void api(`/api_keys/${deleteTarget.id}`, { method: "DELETE" })
                    .then(() => {
                      setDeleteTarget(null);
                      setDeleteConfirm("");
                      reload();
                    })
                    .catch((err) => pushApiError(snackbar.error, err));
                }}
              >
                Delete
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
