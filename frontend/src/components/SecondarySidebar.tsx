// SPDX-License-Identifier: AGPL-3.0-only

import { NavLink } from "react-router-dom";
import { usePermissions } from "../hooks/usePermissions";
import { PERM } from "../permissions";

type Props = {
  releaseTag: string;
};

const SUB_PAGES_BEFORE_MODELS = [
  { path: "dashboard", label: "Dashboard", end: true, permission: PERM.analytics.read },
  { path: "playground", label: "Playground", end: false, permission: PERM.playground.run },
  { path: "sources", label: "Sources", end: false, permission: PERM.sources.read },
] as const;

const SUB_PAGES_AFTER_MODELS = [
  { path: "database", label: "Database", end: false, permission: PERM.db.read },
  { path: "webhooks", label: "Webhooks", end: false, permission: PERM.webhooks.read },
  { path: "settings", label: "Settings", end: false, permission: PERM.settings.read },
] as const;

function SubNavLink({
  releaseTag,
  path,
  label,
  end,
  disabled,
}: {
  releaseTag: string;
  path: string;
  label: string;
  end?: boolean;
  disabled?: boolean;
}) {
  const base = `/releases/${releaseTag}`;
  const to = path === "dashboard" ? base : `${base}/${path}`;

  if (disabled) {
    return (
      <span className="secondary-nav-item disabled" aria-disabled title={`Missing permission for ${label}`}>
        {label}
      </span>
    );
  }

  return (
    <NavLink
      to={to}
      end={end}
      className={({ isActive }) => `secondary-nav-item ${isActive ? "active" : ""}`}
    >
      {label}
    </NavLink>
  );
}

export function SecondarySidebar({ releaseTag }: Props) {
  const { can, ready } = usePermissions();
  const llmModelsDisabled = ready && !can(PERM.llmModels.read);

  return (
    <aside className="secondary-sidebar">
      <div className="mb-3 truncate text-sm font-semibold">{releaseTag}</div>
      <nav className="space-y-0.5">
        {SUB_PAGES_BEFORE_MODELS.map(({ path, label, end, permission }) => (
          <SubNavLink
            key={path}
            releaseTag={releaseTag}
            path={path}
            label={label}
            end={end}
            disabled={ready && !can(permission)}
          />
        ))}
        {llmModelsDisabled ? (
          <span
            className="secondary-nav-item disabled"
            aria-disabled
            title="Missing permission for LLM Models"
          >
            LLM Models
          </span>
        ) : (
          <NavLink
            to={`/releases/${releaseTag}/models`}
            className={({ isActive }) => `secondary-nav-item ${isActive ? "active" : ""}`}
          >
            LLM Models
          </NavLink>
        )}
        {SUB_PAGES_AFTER_MODELS.map(({ path, label, end, permission }) => (
          <SubNavLink
            key={path}
            releaseTag={releaseTag}
            path={path}
            label={label}
            end={end}
            disabled={ready && !can(permission)}
          />
        ))}
      </nav>
    </aside>
  );
}
