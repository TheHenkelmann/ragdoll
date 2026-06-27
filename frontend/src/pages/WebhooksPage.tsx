// SPDX-License-Identifier: AGPL-3.0-only

import { FormEvent, useEffect, useState } from "react";
import { useParams } from "react-router-dom";
import {
  WebhookRecord,
  WebhookTestResult,
  api,
  testWebhook,
} from "../api/client";
import { IconKey } from "../components/icons";
import { PermissionDenied } from "../components/PermissionDenied";
import { Toggle } from "../components/Toggle";
import { WebhookEventEditor } from "../components/WebhookEventEditor";
import { WebhookSecretDialog } from "../components/WebhookSecretDialog";
import { usePermissions } from "../hooks/usePermissions";
import { PERM } from "../permissions";
import { useSnackbar } from "../context/SnackbarContext";
import { pushApiError } from "../utils/snackbarFormat";

function eventCountLabel(webhook: WebhookRecord) {
  const n = webhook.events.length;
  if (n === 0) return "no events";
  return `${n} event${n === 1 ? "" : "s"}`;
}

export function WebhooksPage() {
  const snackbar = useSnackbar();
  const { releaseTag = "" } = useParams();
  const { can, ready } = usePermissions();
  const canRead = can(PERM.webhooks.read);
  const canWrite = can(PERM.webhooks.write);
  const canDelete = can(PERM.webhooks.delete);
  const base = `/releases/${encodeURIComponent(releaseTag)}/webhooks`;

  const [webhooks, setWebhooks] = useState<WebhookRecord[]>([]);
  const [loading, setLoading] = useState(true);
  const [creating, setCreating] = useState(false);
  const [createUrl, setCreateUrl] = useState("");
  const [createEvents, setCreateEvents] = useState<string[]>([]);
  const [createActive, setCreateActive] = useState(true);
  const [editTarget, setEditTarget] = useState<WebhookRecord | null>(null);
  const [editUrl, setEditUrl] = useState("");
  const [editEvents, setEditEvents] = useState<string[]>([]);
  const [editActive, setEditActive] = useState(true);
  const [deleteTarget, setDeleteTarget] = useState<WebhookRecord | null>(null);
  const [testingId, setTestingId] = useState<string | null>(null);
  const [testResult, setTestResult] = useState<{ id: string; result: WebhookTestResult } | null>(
    null,
  );
  const [secretTarget, setSecretTarget] = useState<WebhookRecord | null>(null);

  const reload = () => {
    if (!canRead) {
      setLoading(false);
      return;
    }
    setLoading(true);
    void api<WebhookRecord[]>(base)
      .then(setWebhooks)
      .catch((err) => pushApiError(snackbar.error, err))
      .finally(() => setLoading(false));
  };

  useEffect(() => {
    if (!ready) return;
    reload();
  }, [releaseTag, ready, canRead]);

  async function createWebhook(e: FormEvent) {
    e.preventDefault();
    try {
      await api<WebhookRecord>(base, {
        method: "POST",
        body: JSON.stringify({
          url: createUrl.trim(),
          events: createEvents,
          active: createActive,
        }),
      });
      setCreating(false);
      setCreateUrl("");
      setCreateEvents([]);
      setCreateActive(true);
      reload();
    } catch (err) {
      pushApiError(snackbar.error, err);
    }
  }

  function openEdit(webhook: WebhookRecord) {
    setEditTarget(webhook);
    setEditUrl(webhook.url);
    setEditEvents(webhook.events);
    setEditActive(webhook.active);
  }

  async function saveEdit(e: FormEvent) {
    e.preventDefault();
    if (!editTarget) return;
    try {
      await api<WebhookRecord>(`${base}/${editTarget.id}`, {
        method: "PATCH",
        body: JSON.stringify({
          url: editUrl.trim(),
          events: editEvents,
          active: editActive,
        }),
      });
      setEditTarget(null);
      reload();
    } catch (err) {
      pushApiError(snackbar.error, err);
    }
  }

  async function sendTest(webhook: WebhookRecord) {
    setTestingId(webhook.id);
    try {
      const result = await testWebhook(releaseTag, webhook.id);
      setTestResult({ id: webhook.id, result });
    } catch (err) {
      pushApiError(snackbar.error, err);
    } finally {
      setTestingId(null);
    }
  }

  if (ready && !canRead) {
    return <PermissionDenied permission={PERM.webhooks.read} />;
  }

  return (
    <div className="space-y-6">
      <h2 className="text-2xl font-semibold">Webhooks</h2>
      <p className="text-sm text-[var(--muted)]">
        HTTP callbacks for release <strong>{releaseTag}</strong>. Subscribe to ingest status events
        and/or host resource utilization events. Resource utilization is host-wide — it reflects the
        entire machine Ragdoll runs on, not this release or stage. Each webhook is signed with{" "}
        <strong>HMAC-SHA256</strong> over <code className="text-xs">{"{timestamp}.{body}"}</code>.
      </p>

      {!creating ? (
        <button
          type="button"
          className="btn-primary"
          disabled={!canWrite}
          onClick={() => setCreating(true)}
        >
          Create webhook
        </button>
      ) : (
        <form className="card max-w-lg space-y-4" onSubmit={(e) => void createWebhook(e)}>
          <h3 className="text-lg font-semibold">Create webhook</h3>
          <div className="flex items-center justify-between">
            <span className="text-sm font-medium">Active</span>
            <Toggle checked={createActive} onChange={setCreateActive} label="" />
          </div>
          <label className="block space-y-1 text-sm">
            <span>URL</span>
            <input
              className="input"
              type="url"
              value={createUrl}
              onChange={(e) => setCreateUrl(e.target.value)}
              placeholder="https://example.com/webhook"
              required
            />
          </label>
          <div className="space-y-2 text-sm">
            <span className="font-medium">Events</span>
            <WebhookEventEditor value={createEvents} onChange={setCreateEvents} idPrefix="create" />
            <p className="text-xs text-[var(--muted)]">
              Recovered events are only sent after a matching high or critical alert fired. Resource
              alerts use a 15 minute cooldown per event.
            </p>
          </div>
          <div className="flex gap-2">
            <button type="submit" className="btn-primary" disabled={createEvents.length === 0}>
              Create
            </button>
            <button type="button" className="btn-secondary" onClick={() => setCreating(false)}>
              Cancel
            </button>
          </div>
        </form>
      )}

      <div className="space-y-2">
        {loading && <p className="text-sm text-[var(--muted)]">Loading…</p>}
        {!loading && webhooks.length === 0 && (
          <p className="text-sm text-[var(--muted)]">No webhooks configured.</p>
        )}
        {webhooks.map((w) => (
          <div
            key={w.id}
            className="rounded-lg border px-4 py-3"
            style={{ borderColor: "var(--border)", background: "var(--surface)" }}
          >
            <div className="flex flex-wrap items-start gap-3">
              <div className="min-w-[16rem] flex-1">
                <div className="truncate font-medium">{w.url}</div>
                <div className="mt-1 text-xs text-subtle">
                  {w.created_at} · {w.active ? "active" : "inactive"} · {eventCountLabel(w)}
                </div>
              </div>
              <div className="flex flex-wrap gap-2">
                <button
                  type="button"
                  className="btn-secondary shrink-0 p-2"
                  disabled={!canRead}
                  onClick={() => setSecretTarget(w)}
                  aria-label="View webhook secret and verification"
                  title="View secret and verification"
                >
                  <IconKey size={18} />
                </button>
                <button
                  type="button"
                  className="btn-secondary shrink-0"
                  disabled={!canWrite || testingId === w.id}
                  onClick={() => void sendTest(w)}
                >
                  {testingId === w.id ? "Sending…" : "Send test request"}
                </button>
                <button
                  type="button"
                  className="btn-secondary shrink-0"
                  disabled={!canWrite}
                  onClick={() => openEdit(w)}
                >
                  Edit
                </button>
                <button
                  type="button"
                  className="btn-danger shrink-0"
                  disabled={!canDelete}
                  onClick={() => setDeleteTarget(w)}
                >
                  Delete
                </button>
              </div>
            </div>
            {testResult?.id === w.id && (
              <div
                className="mt-3 rounded border p-3 text-sm"
                style={{ borderColor: "var(--border)", background: "var(--bg)" }}
              >
                <div className="font-medium">
                  Test response
                  {testResult.result.status_code != null && (
                    <span className="ml-2 font-mono text-xs">
                      HTTP {testResult.result.status_code}
                    </span>
                  )}
                </div>
                <pre className="mt-2 max-h-32 overflow-auto whitespace-pre-wrap break-all font-mono text-xs text-[var(--muted)]">
                  {testResult.result.body || "(empty body)"}
                </pre>
              </div>
            )}
          </div>
        ))}
      </div>

      {secretTarget && (
        <WebhookSecretDialog
          releaseTag={releaseTag}
          webhookId={secretTarget.id}
          webhookUrl={secretTarget.url}
          onClose={() => setSecretTarget(null)}
        />
      )}

      {editTarget && (
        <div className="modal-overlay" onClick={() => setEditTarget(null)}>
          <form
            className="card w-full max-w-lg space-y-4"
            onClick={(e) => e.stopPropagation()}
            onSubmit={(e) => void saveEdit(e)}
          >
            <h3 className="text-lg font-semibold">Edit webhook</h3>
            <div className="flex items-center justify-between">
              <span className="text-sm font-medium">Active</span>
              <Toggle checked={editActive} onChange={setEditActive} label="" />
            </div>
            <label className="block space-y-1 text-sm">
              <span>URL</span>
              <input
                className="input"
                type="url"
                value={editUrl}
                onChange={(e) => setEditUrl(e.target.value)}
                required
              />
            </label>
            <div className="space-y-2 text-sm">
              <span className="font-medium">Events</span>
              <WebhookEventEditor value={editEvents} onChange={setEditEvents} idPrefix="edit" />
            </div>
            <div className="flex justify-end gap-2">
              <button type="button" className="btn-secondary" onClick={() => setEditTarget(null)}>
                Cancel
              </button>
              <button type="submit" className="btn-primary">
                Save
              </button>
            </div>
          </form>
        </div>
      )}

      {deleteTarget && (
        <div className="modal-overlay" onClick={() => setDeleteTarget(null)}>
          <div className="card w-full max-w-md space-y-4" onClick={(e) => e.stopPropagation()}>
            <h3 className="text-lg font-semibold">Delete webhook?</h3>
            <p className="text-sm text-[var(--muted)] truncate">{deleteTarget.url}</p>
            <div className="flex justify-end gap-2">
              <button type="button" className="btn-secondary" onClick={() => setDeleteTarget(null)}>
                Cancel
              </button>
              <button
                type="button"
                className="btn-danger"
                onClick={() => {
                  void api(`${base}/${deleteTarget.id}`, { method: "DELETE" })
                    .then(() => {
                      setDeleteTarget(null);
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
