// SPDX-License-Identifier: AGPL-3.0-only

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  ModelDownloadEvent,
  ModelInfo,
  getModels,
  streamModelDownload,
  testModel,
} from "../api/client";

type ModelStatus = "idle" | "downloading" | "materializing" | "testing" | "ready" | "error";

type ModelState = {
  kind: string;
  present: boolean;
  status: ModelStatus;
  progressMessage?: string;
  progressBytes?: number;
  progressTotal?: number;
  latencyMs?: number;
  error?: string;
};

type Props = {
  models: string[];
  canDownload: boolean;
  canTest: boolean;
  onReadyChange: (ready: boolean) => void;
};

function statusLabel(state: ModelState): string {
  switch (state.status) {
    case "downloading":
      return state.progressMessage ?? "Downloading…";
    case "materializing":
      return "Materializing model files…";
    case "testing":
      return "Running test inference…";
    case "ready":
      return `Verified (${state.latencyMs ?? "?"} ms)`;
    case "error":
      return state.error ?? "Error";
    default:
      return state.present ? "Downloaded, not verified" : "Not downloaded";
  }
}

function applyDownloadEvent(prev: ModelState, event: ModelDownloadEvent): ModelState {
  switch (event.event) {
    case "started":
      return { ...prev, status: "downloading", error: undefined, progressMessage: "Starting…" };
    case "progress":
      return {
        ...prev,
        status: "downloading",
        progressBytes: event.bytes,
        progressTotal: event.total ?? prev.progressTotal,
        progressMessage: event.message,
      };
    case "materializing":
      return { ...prev, status: "materializing", progressMessage: undefined };
    case "testing":
      return { ...prev, status: "testing", progressMessage: undefined };
    case "complete":
      return {
        ...prev,
        present: true,
        status: "ready",
        latencyMs: event.latency_ms,
        error: undefined,
        progressMessage: undefined,
      };
    case "error":
      return { ...prev, status: "error", error: event.message };
    default:
      return prev;
  }
}

export function ModelReadinessPanel({ models, canDownload, canTest, onReadyChange }: Props) {
  const [catalog, setCatalog] = useState<ModelInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [states, setStates] = useState<Record<string, ModelState>>({});
  const abortRef = useRef<Record<string, AbortController>>({});

  const uniqueModels = useMemo(() => [...new Set(models)], [models]);

  const reloadCatalog = useCallback(() => {
    setLoading(true);
    setError(null);
    void getModels()
      .then((res) => setCatalog(res.models))
      .catch((err) => setError(String(err)))
      .finally(() => setLoading(false));
  }, []);

  useEffect(() => {
    if (uniqueModels.length === 0) return;
    reloadCatalog();
  }, [uniqueModels, reloadCatalog]);

  useEffect(() => {
    setStates((prev) => {
      const next: Record<string, ModelState> = {};
      for (const name of uniqueModels) {
        const info = catalog.find((m) => m.name === name);
        const existing = prev[name];
        next[name] = existing ?? {
          kind: info?.kind ?? "model",
          present: info?.present ?? false,
          status: "idle",
        };
        if (info) {
          next[name] = {
            ...next[name],
            kind: info.kind,
            present: info.present,
          };
        }
      }
      return next;
    });
  }, [uniqueModels, catalog]);

  const allReady = useMemo(
    () =>
      uniqueModels.length > 0 &&
      uniqueModels.every((name) => states[name]?.status === "ready"),
    [uniqueModels, states],
  );

  useEffect(() => {
    onReadyChange(allReady);
  }, [allReady, onReadyChange]);

  useEffect(() => {
    return () => {
      for (const controller of Object.values(abortRef.current)) {
        controller.abort();
      }
    };
  }, []);

  async function runStream(name: string) {
    abortRef.current[name]?.abort();
    const controller = new AbortController();
    abortRef.current[name] = controller;
    setError(null);
    try {
      await streamModelDownload(
        name,
        (event) => {
          setStates((prev) => ({
            ...prev,
            [name]: applyDownloadEvent(prev[name] ?? { kind: "model", present: false, status: "idle" }, event),
          }));
        },
        controller.signal,
      );
      reloadCatalog();
    } catch (err) {
      if (controller.signal.aborted) return;
      setStates((prev) => ({
        ...prev,
        [name]: {
          ...(prev[name] ?? { kind: "model", present: false, status: "idle" }),
          status: "error",
          error: String(err),
        },
      }));
    }
  }

  async function verifyDownloaded(name: string) {
    setStates((prev) => ({
      ...prev,
      [name]: { ...(prev[name] ?? { kind: "model", present: true, status: "idle" }), status: "testing" },
    }));
    try {
      const result = await testModel(name);
      setStates((prev) => ({
        ...prev,
        [name]: {
          ...(prev[name] ?? { kind: result.kind, present: true, status: "idle" }),
          kind: result.kind,
          present: true,
          status: "ready",
          latencyMs: result.latency_ms,
          error: undefined,
        },
      }));
    } catch (err) {
      setStates((prev) => ({
        ...prev,
        [name]: {
          ...(prev[name] ?? { kind: "model", present: true, status: "idle" }),
          status: "error",
          error: String(err),
        },
      }));
    }
  }

  useEffect(() => {
    if (!canTest || loading) return;
    for (const name of uniqueModels) {
      const state = states[name];
      const info = catalog.find((m) => m.name === name);
      if (!state || state.status !== "idle") continue;
      if (info?.present) {
        void verifyDownloaded(name);
      }
    }
  }, [uniqueModels, states, catalog, canTest, loading]);

  if (uniqueModels.length === 0) return null;

  return (
    <div
      className="space-y-3 rounded-lg border p-4"
      style={{ borderColor: "var(--border)", background: "var(--surface)" }}
    >
      <div>
        <h4 className="text-sm font-medium">Model readiness</h4>
        <p className="text-xs text-[var(--muted)]">
          Required models must be downloaded and pass a test inference before you can save model
          changes.
        </p>
      </div>
      {error && <p className="text-xs text-error">{error}</p>}
      {loading && <p className="text-xs text-[var(--muted)]">Checking model status…</p>}
      <ul className="space-y-3">
        {uniqueModels.map((name) => {
          const state = states[name] ?? { kind: "model", present: false, status: "idle" as const };
          const busy = ["downloading", "materializing", "testing"].includes(state.status);
          const showProgress = state.status === "downloading" && state.progressBytes != null;
          const hasTotal =
            showProgress && state.progressTotal != null && state.progressTotal > 0;
          const percent = hasTotal
            ? Math.min(99, Math.round(((state.progressBytes ?? 0) / state.progressTotal!) * 100))
            : null;
          return (
            <li
              key={name}
              className="space-y-2 rounded-md border px-3 py-2 text-sm"
              style={{ borderColor: "var(--border)" }}
            >
              <div className="flex flex-wrap items-center justify-between gap-2">
                <div className="min-w-0">
                  <div className="truncate font-mono text-xs">{name}</div>
                  <div
                    className={`text-xs ${state.status === "error" ? "text-error" : "text-[var(--muted)]"}`}
                  >
                    {state.kind} · {statusLabel(state)}
                  </div>
                </div>
                {state.status !== "ready" && canDownload && (
                  <button
                    type="button"
                    className="btn-secondary shrink-0 text-xs"
                    disabled={busy}
                    onClick={() => void runStream(name)}
                  >
                    {busy ? "Working…" : state.present ? "Verify" : "Download & verify"}
                  </button>
                )}
                {state.status !== "ready" && !canDownload && (
                  <span className="text-xs text-[var(--muted)]">models:download required</span>
                )}
              </div>
              {showProgress && (
                <div className="space-y-1">
                  <div className="h-1.5 overflow-hidden rounded-full" style={{ background: "var(--border)" }}>
                    {percent != null ? (
                      <div
                        className="h-full rounded-full bg-blue-500 transition-all"
                        style={{ width: `${percent}%` }}
                      />
                    ) : (
                      <div
                        className="h-full animate-pulse rounded-full bg-blue-500"
                        style={{ width: "100%" }}
                      />
                    )}
                  </div>
                  <div className="flex justify-between text-[10px] text-[var(--muted)]">
                    <span>{state.progressMessage}</span>
                    {percent != null && <span>{percent}%</span>}
                  </div>
                </div>
              )}
            </li>
          );
        })}
      </ul>
      {!allReady && (
        <p className="text-xs text-[var(--muted)]">
          Save is disabled until all required models are verified.
        </p>
      )}
    </div>
  );
}
