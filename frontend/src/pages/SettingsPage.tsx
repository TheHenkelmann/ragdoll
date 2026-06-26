// SPDX-License-Identifier: AGPL-3.0-only

import { useEffect, useState } from "react";
import { useParams } from "react-router-dom";
import { RuntimeSettings, api } from "../api/client";

export function SettingsPage() {
  const { releaseTag = "" } = useParams();
  const [settings, setSettings] = useState<RuntimeSettings | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  useEffect(() => {
    void api<RuntimeSettings>(`/releases/${releaseTag}/settings`).then(setSettings).catch(console.error);
  }, [releaseTag]);

  async function save() {
    if (!settings) return;
    const updated = await api<RuntimeSettings>(`/releases/${releaseTag}/settings`, {
      method: "PATCH",
      body: JSON.stringify({
        chunking_strategy: settings.chunking_strategy,
        payload_storage: settings.payload_storage,
        embedding_model: settings.embedding_model,
        rerank_model: settings.rerank_model,
      }),
    });
    setSettings(updated);
    setMessage("Saved");
  }

  if (!settings) return <p>Loading...</p>;

  return (
    <div className="space-y-6">
      <h2 className="text-2xl font-semibold">Settings</h2>
      <div className="card max-w-xl space-y-4">
        <label className="block space-y-1 text-sm"><span>Chunking strategy</span>
          <select className="input" value={settings.chunking_strategy} onChange={(e) => setSettings({ ...settings, chunking_strategy: e.target.value })}>
            <option value="semantic_split">Semantic Split</option>
          </select>
        </label>
        <label className="block space-y-1 text-sm"><span>Payload storage</span>
          <select className="input" value={settings.payload_storage} onChange={(e) => setSettings({ ...settings, payload_storage: e.target.value as RuntimeSettings["payload_storage"] })}>
            <option value="per_request">per_request</option>
            <option value="forced">forced</option>
            <option value="forbidden">forbidden</option>
          </select>
        </label>
        <label className="block space-y-1 text-sm"><span>Embedding model (dim 1024 only)</span>
          <input className="input" value={settings.embedding_model} onChange={(e) => setSettings({ ...settings, embedding_model: e.target.value })} />
        </label>
        <label className="block space-y-1 text-sm"><span>Rerank model</span>
          <input className="input" value={settings.rerank_model} onChange={(e) => setSettings({ ...settings, rerank_model: e.target.value })} />
        </label>
        <button className="btn-primary" onClick={() => void save()}>Save</button>
        {message && <p className="text-sm text-[var(--muted)]">{message}</p>}
      </div>
    </div>
  );
}
