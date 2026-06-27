// SPDX-License-Identifier: AGPL-3.0-only

import { useState } from "react";
import {
  FORCED_PERMISSION,
  PERMISSION_CATALOG,
  ensureForcedPermissions,
  optionalPermissionCount,
} from "../api/client";

type Props = {
  value: string[];
  onChange: (permissions: string[]) => void;
  idPrefix?: string;
  /** When false, no permission is force-granted (used for API keys). Defaults to true (users). */
  forceReleasesRead?: boolean;
};

const ALL_PERMISSIONS: string[] = PERMISSION_CATALOG.flatMap((g) => g.permissions);

function permissionsEndingWith(perms: string[], suffix: string): string[] {
  return perms.filter((p) => p.endsWith(`:${suffix}`));
}

export function PermissionEditor({
  value,
  onChange,
  idPrefix = "perm",
  forceReleasesRead = true,
}: Props) {
  const [expanded, setExpanded] = useState(false);

  const isForced = (perm: string) => forceReleasesRead && perm === FORCED_PERMISSION;
  const optionalPermissions = ALL_PERMISSIONS.filter((p) => !isForced(p));
  const totalOptional = optionalPermissions.length;

  const normalized = forceReleasesRead ? ensureForcedPermissions(value) : value;
  const selected = new Set(normalized);
  const optionalSelected = forceReleasesRead
    ? optionalPermissionCount(normalized)
    : selected.size;

  function emit(next: Set<string>) {
    if (forceReleasesRead) next.add(FORCED_PERMISSION);
    onChange([...next].sort());
  }

  function toggle(perm: string) {
    if (isForced(perm)) return;
    const next = new Set(selected);
    if (next.has(perm)) next.delete(perm);
    else next.add(perm);
    emit(next);
  }

  function toggleSection(perms: string[]) {
    const optional = perms.filter((p) => !isForced(p));
    if (optional.length === 0) return;
    const allSelected = optional.every((p) => selected.has(p));
    const next = new Set(selected);
    if (allSelected) {
      for (const p of optional) next.delete(p);
    } else {
      for (const p of optional) next.add(p);
    }
    emit(next);
  }

  function setAll() {
    onChange([...ALL_PERMISSIONS].sort());
  }

  function clearOptional() {
    onChange(forceReleasesRead ? [FORCED_PERMISSION] : []);
  }

  function addGroup(suffix: string) {
    const next = new Set(selected);
    for (const p of permissionsEndingWith(optionalPermissions, suffix)) next.add(p);
    emit(next);
  }

  return (
    <div className="space-y-2">
      <button
        type="button"
        className="text-sm text-[var(--muted)] hover:text-[var(--text)]"
        onClick={() => setExpanded((v) => !v)}
        aria-expanded={expanded}
      >
        {expanded ? "Hide permissions" : "Edit permissions"}
        <span className="ml-2 text-xs">
          ({optionalSelected}/{totalOptional} selected
          {forceReleasesRead ? ` · ${FORCED_PERMISSION} always granted` : ""})
        </span>
      </button>
      {expanded && (
        <div className="flex flex-wrap gap-2">
          <button type="button" className="btn-secondary px-2 py-1 text-xs" onClick={setAll}>
            Select all
          </button>
          <button
            type="button"
            className="btn-secondary px-2 py-1 text-xs"
            onClick={() => addGroup("read")}
          >
            All read
          </button>
          <button
            type="button"
            className="btn-secondary px-2 py-1 text-xs"
            onClick={() => addGroup("write")}
          >
            All write
          </button>
          <button
            type="button"
            className="btn-secondary px-2 py-1 text-xs"
            onClick={() => addGroup("delete")}
          >
            All delete
          </button>
          <button type="button" className="btn-secondary px-2 py-1 text-xs" onClick={clearOptional}>
            {forceReleasesRead ? "Clear optional" : "Clear all"}
          </button>
        </div>
      )}
      {expanded && (
        <div
          className="max-h-72 space-y-3 overflow-y-auto rounded-lg border p-3"
          style={{ borderColor: "var(--border)", background: "var(--surface)" }}
        >
          {PERMISSION_CATALOG.map(({ section, permissions }) => {
            const sectionId = `${idPrefix}-${section.replace(/\s+/g, "-").toLowerCase()}`;
            const optional = permissions.filter((p) => !isForced(p));
            const allSelected =
              optional.length === 0
                ? permissions.every((p) => selected.has(p))
                : optional.every((p) => selected.has(p));
            const someSelected = permissions.some((p) => selected.has(p));
            return (
              <div key={section} className="space-y-1.5">
                <label className="flex cursor-pointer items-center gap-2 text-sm font-medium">
                  <input
                    type="checkbox"
                    className="rounded"
                    checked={allSelected}
                    ref={(el) => {
                      if (el) el.indeterminate = someSelected && !allSelected;
                    }}
                    onChange={() => toggleSection(permissions)}
                  />
                  {section}
                </label>
                <div className="ml-5 space-y-1">
                  {permissions.map((perm) => {
                    const forced = isForced(perm);
                    return (
                      <label
                        key={perm}
                        className={`flex items-center gap-2 text-sm ${forced ? "cursor-default opacity-80" : "cursor-pointer"}`}
                      >
                        <input
                          id={`${sectionId}-${perm}`}
                          type="checkbox"
                          className="rounded"
                          checked={selected.has(perm)}
                          disabled={forced}
                          onChange={() => toggle(perm)}
                        />
                        <span className="font-mono text-xs">
                          {perm}
                          {forced && (
                            <span className="ml-1.5 font-sans text-[var(--muted)]">(always granted)</span>
                          )}
                        </span>
                      </label>
                    );
                  })}
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
