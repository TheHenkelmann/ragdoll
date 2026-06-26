// SPDX-License-Identifier: AGPL-3.0-only

import { useEffect, useRef, useState } from "react";
import { useAuth } from "../context/AuthContext";

export function UserMenu() {
  const { status, logout } = useAuth();
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    function onClick(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    }
    document.addEventListener("mousedown", onClick);
    return () => document.removeEventListener("mousedown", onClick);
  }, []);

  const initial = (status?.email?.[0] ?? "?").toUpperCase();

  return (
    <div className="relative" ref={ref}>
      <button type="button" className="icon-btn" onClick={() => setOpen((v) => !v)} aria-label="Account menu">
        <span className="flex h-7 w-7 items-center justify-center rounded-full text-xs font-medium" style={{ background: "color-mix(in srgb, var(--accent) 25%, var(--surface))" }}>
          {initial}
        </span>
      </button>
      {open && (
        <div className="absolute right-0 top-10 z-30 min-w-48 rounded-xl border p-2 shadow-lg" style={{ background: "var(--surface)", borderColor: "var(--border)" }}>
          <div className="px-3 py-2 text-sm text-[var(--muted)] truncate">{status?.email}</div>
          <button type="button" className="block w-full rounded-lg px-3 py-2 text-left text-sm hover:bg-black/10" onClick={() => { logout(); setOpen(false); }}>
            Logout
          </button>
        </div>
      )}
    </div>
  );
}
