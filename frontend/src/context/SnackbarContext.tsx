// SPDX-License-Identifier: AGPL-3.0-only

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { SnackbarContainer } from "../components/Snackbar";

export type SnackbarType = "error" | "success" | "warning" | "info";

export type SnackbarItem = {
  id: string;
  mergeKey: string;
  title: string;
  body: string;
  type: SnackbarType;
  ttl: number;
  remaining: number;
  count: number;
  expanded: boolean;
};

export type SnackbarOptions = {
  title: string;
  body?: string;
  type?: SnackbarType;
  ttl?: number;
};

type SnackbarContextValue = {
  items: SnackbarItem[];
  paused: boolean;
  push: (opts: SnackbarOptions) => void;
  error: (title: string, body?: string, ttl?: number) => void;
  success: (title: string, body?: string, ttl?: number) => void;
  warning: (title: string, body?: string, ttl?: number) => void;
  info: (title: string, body?: string, ttl?: number) => void;
  dismiss: (id: string) => void;
  toggleExpanded: (id: string) => void;
  setPaused: (paused: boolean) => void;
};

const DEFAULT_TTL = 5000;

const SnackbarContext = createContext<SnackbarContextValue | null>(null);

let nextId = 0;

function makeMergeKey(type: SnackbarType, title: string, body: string): string {
  return `${type}::${title}::${body}`;
}

export function SnackbarProvider({ children }: { children: React.ReactNode }) {
  const [items, setItems] = useState<SnackbarItem[]>([]);
  const [paused, setPaused] = useState(false);
  const pausedRef = useRef(false);
  const itemsRef = useRef(items);

  useEffect(() => {
    pausedRef.current = paused;
  }, [paused]);

  useEffect(() => {
    itemsRef.current = items;
  }, [items]);

  const push = useCallback((opts: SnackbarOptions) => {
    const type = opts.type ?? "error";
    const ttl = opts.ttl ?? DEFAULT_TTL;
    const body = opts.body ?? "";
    const mergeKey = makeMergeKey(type, opts.title, body);

    setItems((prev) => {
      const existing = prev.find((item) => item.mergeKey === mergeKey);
      if (existing) {
        return prev.map((item) =>
          item.id === existing.id
            ? {
                ...item,
                count: item.count + 1,
                ttl,
                remaining: ttl,
              }
            : item,
        );
      }
      return [
        ...prev,
        {
          id: `snackbar-${++nextId}`,
          mergeKey,
          title: opts.title,
          body,
          type,
          ttl,
          remaining: ttl,
          count: 1,
          expanded: false,
        },
      ];
    });
  }, []);

  const dismiss = useCallback((id: string) => {
    setItems((prev) => prev.filter((item) => item.id !== id));
  }, []);

  const toggleExpanded = useCallback((id: string) => {
    setItems((prev) =>
      prev.map((item) => (item.id === id ? { ...item, expanded: !item.expanded } : item)),
    );
  }, []);

  const error = useCallback(
    (title: string, body?: string, ttl?: number) => push({ title, body, type: "error", ttl }),
    [push],
  );
  const success = useCallback(
    (title: string, body?: string, ttl?: number) => push({ title, body, type: "success", ttl }),
    [push],
  );
  const warning = useCallback(
    (title: string, body?: string, ttl?: number) => push({ title, body, type: "warning", ttl }),
    [push],
  );
  const info = useCallback(
    (title: string, body?: string, ttl?: number) => push({ title, body, type: "info", ttl }),
    [push],
  );

  useEffect(() => {
    let lastTick = performance.now();
    let frameId: number;

    function tick(now: number) {
      const delta = now - lastTick;
      lastTick = now;

      if (!pausedRef.current && itemsRef.current.length > 0) {
        setItems((prev) => {
          const next = prev
            .map((item) => ({ ...item, remaining: item.remaining - delta }))
            .filter((item) => item.remaining > 0);
          return next.length === prev.length && next.every((item, i) => item.remaining === prev[i].remaining)
            ? prev
            : next;
        });
      }

      frameId = requestAnimationFrame(tick);
    }

    frameId = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frameId);
  }, []);

  const value = useMemo(
    () => ({
      items,
      paused,
      push,
      error,
      success,
      warning,
      info,
      dismiss,
      toggleExpanded,
      setPaused,
    }),
    [items, paused, push, error, success, warning, info, dismiss, toggleExpanded],
  );

  return (
    <SnackbarContext.Provider value={value}>
      {children}
      <SnackbarContainer />
    </SnackbarContext.Provider>
  );
}

export function useSnackbar() {
  const ctx = useContext(SnackbarContext);
  if (!ctx) throw new Error("SnackbarProvider missing");
  const { push, error, success, warning, info, dismiss } = ctx;
  return useMemo(
    () => ({ push, error, success, warning, info, dismiss }),
    [push, error, success, warning, info, dismiss],
  );
}

export function useSnackbarInternal() {
  const ctx = useContext(SnackbarContext);
  if (!ctx) throw new Error("SnackbarProvider missing");
  return ctx;
}
