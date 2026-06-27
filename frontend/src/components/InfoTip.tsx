// SPDX-License-Identifier: AGPL-3.0-only

import { useState } from "react";

export function InfoTip({
  text,
  wide,
  tone = "default",
}: {
  text: string;
  wide?: boolean;
  tone?: "default" | "danger";
}) {
  const [open, setOpen] = useState(false);
  const danger = tone === "danger";

  return (
    <span className="relative inline-flex">
      <button
        type="button"
        className="ml-1.5 inline-flex h-4 w-4 items-center justify-center rounded-full text-[10px] hover:text-[var(--text)]"
        style={{
          border: `1px solid ${danger ? "var(--danger)" : "var(--border)"}`,
          color: danger ? "var(--danger-text)" : "var(--muted)",
        }}
        onMouseEnter={() => setOpen(true)}
        onMouseLeave={() => setOpen(false)}
        onClick={() => setOpen((v) => !v)}
        aria-label="More info"
      >
        i
      </button>
      {open && (
        <span
          className={`absolute left-5 top-0 z-20 rounded-lg border p-3 text-xs leading-relaxed shadow-lg ${wide ? "w-80" : "w-56"}`}
          style={{ background: "var(--surface)", borderColor: "var(--border)", color: "var(--muted)" }}
        >
          {text}
        </span>
      )}
    </span>
  );
}
