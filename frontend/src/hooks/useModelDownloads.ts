// SPDX-License-Identifier: AGPL-3.0-only

import { useCallback, useEffect, useRef, useState } from "react";
import { ModelDownloadEvent, cancelModelDownload, streamModelDownload } from "../api/client";

export type ModelDownloadStatus =
  | "idle"
  | "downloading"
  | "materializing"
  | "testing"
  | "ready"
  | "error"
  | "cancelled"
  | "purging";

export type ModelRowState = {
  status: ModelDownloadStatus;
  message?: string;
  progress?: number;
  progressBytes?: number;
  progressTotal?: number;
  /** When false, cancel is hidden because the backend cannot stop safely. */
  cancellable?: boolean;
};

export function applyDownloadEvent(
  prev: ModelRowState,
  event: ModelDownloadEvent,
): ModelRowState {
  switch (event.event) {
    case "started":
      return { status: "downloading", message: "Starting…", cancellable: false };
    case "progress": {
      const pct =
        event.total && event.total > 0
          ? Math.min(99, Math.round((event.bytes / event.total) * 100))
          : undefined;
      return {
        status: "downloading",
        message: event.message,
        progress: pct,
        progressBytes: event.bytes,
        progressTotal: event.total ?? prev.progressTotal,
      };
    }
    case "materializing":
      return { status: "materializing", message: "Materializing…" };
    case "testing":
      return { status: "testing", message: "Running test inference…" };
    case "complete":
      return {
        status: "ready",
        message: `Verified (${event.latency_ms} ms)`,
      };
    case "error":
      return { status: "error", message: event.message };
    case "cancelled":
      return { status: "cancelled", message: "Download cancelled", cancellable: false };
    case "cancellable":
      return { ...prev, cancellable: event.cancellable };
    default:
      return prev;
  }
}

export type ModelDownloadCallbacks = {
  onComplete?: (name: string) => void;
  onError?: (name: string, message: string) => void;
  onCancel?: (name: string) => void;
};

export function useModelDownloads(callbacks: ModelDownloadCallbacks = {}) {
  const [rowState, setRowState] = useState<Record<string, ModelRowState>>({});
  const abortRef = useRef<Record<string, AbortController>>({});
  // Models the user cancelled this session; skip auto-reconnect for them even
  // though the backend job may still be running.
  const cancelledRef = useRef<Set<string>>(new Set());
  const callbacksRef = useRef(callbacks);
  callbacksRef.current = callbacks;

  useEffect(() => {
    return () => {
      for (const controller of Object.values(abortRef.current)) {
        controller.abort();
      }
    };
  }, []);

  const startDownload = useCallback(async (name: string) => {
    abortRef.current[name]?.abort();
    const controller = new AbortController();
    abortRef.current[name] = controller;
    cancelledRef.current.delete(name);

    setRowState((prev) => ({
      ...prev,
      [name]: { status: "downloading", message: "Starting…" },
    }));

    // The backend SSE channel stays open after a terminal event (the job lives
    // in a registry), so the stream never ends on its own. Treat complete/error
    // as terminal here: fire the callback and abort to stop reading.
    let terminal = false;
    try {
      await streamModelDownload(
        name,
        (event) => {
          setRowState((prev) => ({
            ...prev,
            [name]: applyDownloadEvent(prev[name] ?? { status: "idle" }, event),
          }));
          if (event.event === "complete") {
            terminal = true;
            delete abortRef.current[name];
            callbacksRef.current.onComplete?.(name);
            controller.abort();
          } else if (event.event === "error") {
            terminal = true;
            delete abortRef.current[name];
            callbacksRef.current.onError?.(name, event.message);
            controller.abort();
          } else if (event.event === "cancelled") {
            // Backend confirmed cancellation; let the catch block report it once.
            terminal = true;
            cancelledRef.current.add(name);
            controller.abort();
          }
        },
        controller.signal,
      );
      delete abortRef.current[name];
      if (!terminal) {
        // Stream closed without a terminal event (e.g. server restart).
        callbacksRef.current.onComplete?.(name);
      }
    } catch (err) {
      if (controller.signal.aborted) {
        if (cancelledRef.current.has(name)) {
          delete abortRef.current[name];
          setRowState((prev) => ({
            ...prev,
            [name]: { status: "cancelled", message: "Download cancelled" },
          }));
          callbacksRef.current.onCancel?.(name);
        }
        return;
      }
      delete abortRef.current[name];
      const message = String(err);
      setRowState((prev) => ({
        ...prev,
        [name]: { status: "error", message },
      }));
      callbacksRef.current.onError?.(name, message);
    }
  }, []);

  const cancelDownload = useCallback(async (name: string) => {
    try {
      const result = await cancelModelDownload(name);
      if (!result.cancelled) {
        callbacksRef.current.onError?.(
          name,
          "Download cannot be cancelled in this phase (fastembed is still fetching).",
        );
        return;
      }
    } catch (err) {
      callbacksRef.current.onError?.(name, `Cancel request failed: ${String(err)}`);
      return;
    }
    cancelledRef.current.add(name);
    const controller = abortRef.current[name];
    if (controller) {
      controller.abort();
    } else {
      setRowState((prev) => ({
        ...prev,
        [name]: { status: "cancelled", message: "Download cancelled", cancellable: false },
      }));
      callbacksRef.current.onCancel?.(name);
    }
  }, []);

  const reconnectActive = useCallback(
    (activeDownloads: string[]) => {
      for (const name of activeDownloads) {
        if (abortRef.current[name]) continue;
        if (cancelledRef.current.has(name)) continue;
        void startDownload(name);
      }
    },
    [startDownload],
  );

  const setRow = useCallback((name: string, state: ModelRowState) => {
    setRowState((prev) => ({ ...prev, [name]: state }));
  }, []);

  return { rowState, startDownload, cancelDownload, reconnectActive, setRow };
}
