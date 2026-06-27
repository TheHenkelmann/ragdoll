// SPDX-License-Identifier: AGPL-3.0-only

import { useCallback, useEffect, useState } from "react";
import { getModelsStatus, type ModelsStatusResponse } from "../api/client";

const POLL_INTERVAL_MS = 30_000;
const REFRESH_EVENT = "ragdoll:models-status-refresh";

/** Trigger an immediate refresh of any mounted model-status consumers. */
export function refreshModelStatus() {
  window.dispatchEvent(new Event(REFRESH_EVENT));
}

/**
 * Polls model status so global surfaces (e.g. the sidebar) can flag releases
 * with missing models or embedding mismatches. Only fetches when `enabled`.
 */
export function useModelStatus(enabled: boolean) {
  const [data, setData] = useState<ModelsStatusResponse | null>(null);

  const load = useCallback(() => {
    if (!enabled) return;
    void getModelsStatus()
      .then(setData)
      .catch(() => {
        // Sidebar badge is best-effort; ignore transient errors.
      });
  }, [enabled]);

  useEffect(() => {
    if (!enabled) {
      setData(null);
      return;
    }
    load();
    const interval = window.setInterval(load, POLL_INTERVAL_MS);
    const onRefresh = () => load();
    const onFocus = () => load();
    window.addEventListener(REFRESH_EVENT, onRefresh);
    window.addEventListener("focus", onFocus);
    return () => {
      window.clearInterval(interval);
      window.removeEventListener(REFRESH_EVENT, onRefresh);
      window.removeEventListener("focus", onFocus);
    };
  }, [enabled, load]);

  const mismatchCount = data?.mismatches.length ?? 0;
  const missingCount = data?.missing.length ?? 0;

  return {
    data,
    mismatchCount,
    missingCount,
    hasIssues: mismatchCount > 0 || missingCount > 0,
    refresh: load,
  };
}
