// SPDX-License-Identifier: AGPL-3.0-only

import { FormEvent, useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { ReleaseRecord, StageRecord, api } from "../api/client";
import { useSnackbar } from "../context/SnackbarContext";
import { formatApiError } from "../utils/snackbarFormat";

type Props = {
  open: boolean;
  onClose: () => void;
  releases: ReleaseRecord[];
  stages: StageRecord[];
  onChanged: () => void;
};

export function StageModal({ open, onClose, releases, stages, onChanged }: Props) {
  const navigate = useNavigate();
  const snackbar = useSnackbar();
  const [tag, setTag] = useState("");
  const [releaseTag, setReleaseTag] = useState(releases[0]?.tag ?? "first-release");
  const [loading, setLoading] = useState(false);
  const [items, setItems] = useState<StageRecord[]>(stages);

  useEffect(() => {
    if (!open) return;
    setLoading(true);
    void api<StageRecord[]>("/stages")
      .then(setItems)
      .catch((err) => {
        const { title, body } = formatApiError(err);
        snackbar.error(title, body || undefined);
      })
      .finally(() => setLoading(false));
    void api<ReleaseRecord[]>("/releases").then((r) => {
      if (r[0]?.tag) setReleaseTag(r[0].tag);
    }).catch(console.error);
  }, [open]);

  useEffect(() => {
    if (open) setItems(stages);
  }, [stages, open]);

  if (!open) return null;

  async function createStage(e: FormEvent) {
    e.preventDefault();
    try {
      await api("/stages", {
        method: "POST",
        body: JSON.stringify({ tag, release_tag: releaseTag }),
      });
      setTag("");
      onChanged();
      setItems(await api<StageRecord[]>("/stages"));
    } catch (err) {
      const { title, body } = formatApiError(err);
      snackbar.error(title, body || undefined);
    }
  }

  async function retarget(stage: StageRecord, nextReleaseTag: string) {
    try {
      await api(`/stages/${stage.tag}`, {
        method: "PATCH",
        body: JSON.stringify({ release_tag: nextReleaseTag }),
      });
      onChanged();
      setItems(await api<StageRecord[]>("/stages"));
    } catch (err) {
      const { title, body } = formatApiError(err);
      snackbar.error(title, body || undefined);
    }
  }

  async function remove(stage: StageRecord) {
    try {
      await api(`/stages/${stage.tag}`, { method: "DELETE" });
      onChanged();
      setItems(await api<StageRecord[]>("/stages"));
    } catch (err) {
      const { title, body } = formatApiError(err);
      snackbar.error(title, body || undefined);
    }
  }

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="card w-full max-w-lg space-y-4" onClick={(e) => e.stopPropagation()}>
        <div className="flex items-center justify-between">
          <h3 className="text-lg font-medium">Stages</h3>
          <button type="button" className="btn-secondary" onClick={onClose}>Close</button>
        </div>
        <div className="space-y-2">
          <div className="text-sm font-medium">Existing stages</div>
          {loading && <p className="text-sm text-[var(--muted)]">Loading…</p>}
          {!loading && items.length === 0 && <p className="text-sm text-[var(--muted)]">No stages yet.</p>}
          {items.map((s) => (
            <div key={s.id} className="flex flex-wrap items-center gap-2 rounded border p-3 text-sm" style={{ borderColor: "var(--border)" }}>
              <button type="button" className="font-medium hover:underline" onClick={() => { navigate(`/stages/${s.tag}`); onClose(); }}>
                {s.tag}
              </button>
              <span className="text-[var(--muted)]">→</span>
              <select
                className="input max-w-[180px]"
                value={s.release_tag}
                onChange={(e) => void retarget(s, e.target.value)}
              >
                {releases.map((r) => <option key={r.id} value={r.tag}>{r.tag}</option>)}
              </select>
              <button type="button" className="btn-secondary ml-auto" onClick={() => void remove(s)}>Delete</button>
            </div>
          ))}
        </div>
        <form className="space-y-3 border-t pt-4" style={{ borderColor: "var(--border)" }} onSubmit={(e) => void createStage(e)}>
          <div className="text-sm font-medium">Create stage</div>
          <div className="grid gap-3 md:grid-cols-2">
            <label className="space-y-1 text-sm">
              <span>Stage tag</span>
              <input className="input" value={tag} onChange={(e) => setTag(e.target.value)} maxLength={12} required />
            </label>
            <label className="space-y-1 text-sm">
              <span>Release</span>
              <select className="input" value={releaseTag} onChange={(e) => setReleaseTag(e.target.value)}>
                {releases.map((r) => <option key={r.id} value={r.tag}>{r.tag}</option>)}
              </select>
            </label>
          </div>
          <button className="btn-primary" type="submit">Create stage</button>
        </form>
      </div>
    </div>
  );
}
