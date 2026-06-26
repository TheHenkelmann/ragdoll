// SPDX-License-Identifier: AGPL-3.0-only

import { FormEvent, useEffect, useState } from "react";
import { ReleaseRecord, api } from "../api/client";

type Props = {
  open: boolean;
  onClose: () => void;
  releases: ReleaseRecord[];
  currentTag: string;
  tab: string;
  onChanged: () => void;
  onSelect: (tag: string) => void;
};

export function ReleaseModal({ open, onClose, releases, currentTag, tab, onChanged, onSelect }: Props) {
  const [search, setSearch] = useState("");
  const [tag, setTag] = useState("");
  const [message, setMessage] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [items, setItems] = useState<ReleaseRecord[]>(releases);

  useEffect(() => {
    if (!open) return;
    setLoading(true);
    void api<ReleaseRecord[]>("/releases")
      .then(setItems)
      .catch((err) => setError(String(err)))
      .finally(() => setLoading(false));
  }, [open]);

  useEffect(() => {
    if (open) setItems(releases);
  }, [releases, open]);

  if (!open) return null;

  const filtered = items.filter(
    (r) =>
      r.tag.includes(search) ||
      r.id.includes(search) ||
      r.message.toLowerCase().includes(search.toLowerCase()),
  );

  async function createRelease(e: FormEvent) {
    e.preventDefault();
    setError(null);
    try {
      await api("/releases", {
        method: "POST",
        body: JSON.stringify({ tag, message, init: { type: "new" } }),
      });
      setTag("");
      setMessage("");
      onChanged();
      const next = await api<ReleaseRecord[]>("/releases");
      setItems(next);
      onSelect(tag);
      onClose();
    } catch (err) {
      setError(String(err));
    }
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4" onClick={onClose}>
      <div className="card w-full max-w-lg space-y-4" onClick={(e) => e.stopPropagation()}>
        <div className="flex items-center justify-between">
          <h3 className="text-lg font-medium">Releases</h3>
          <button type="button" className="btn-secondary" onClick={onClose}>Close</button>
        </div>
        {error && <div className="text-sm text-red-400">{error}</div>}
        <input className="input" placeholder="Search releases" value={search} onChange={(e) => setSearch(e.target.value)} />
        <div className="max-h-64 space-y-1 overflow-auto">
          {loading && <p className="text-sm text-[var(--muted)]">Loading…</p>}
          {!loading && filtered.length === 0 && <p className="text-sm text-[var(--muted)]">No releases found.</p>}
          {filtered.map((r) => (
            <button
              key={r.id}
              type="button"
              className={`block w-full rounded-lg px-3 py-2 text-left hover:bg-black/10 ${r.tag === currentTag ? "ring-2 ring-[var(--accent)]" : ""}`}
              onClick={() => {
                onSelect(r.tag);
                onClose();
              }}
            >
              <div className="font-medium">{r.tag}</div>
              <div className="text-xs text-[var(--muted)]">{r.message || r.id}</div>
            </button>
          ))}
        </div>
        <form className="space-y-3 border-t pt-4" style={{ borderColor: "var(--border)" }} onSubmit={(e) => void createRelease(e)}>
          <div className="text-sm font-medium">Create release</div>
          <div className="grid gap-3 md:grid-cols-2">
            <label className="space-y-1 text-sm">
              <span>Tag</span>
              <input className="input" value={tag} onChange={(e) => setTag(e.target.value)} maxLength={50} required />
            </label>
            <label className="space-y-1 text-sm">
              <span>Message</span>
              <input className="input" value={message} onChange={(e) => setMessage(e.target.value)} placeholder="optional" />
            </label>
          </div>
          <button className="btn-primary" type="submit">Create release</button>
        </form>
      </div>
    </div>
  );
}
