// SPDX-License-Identifier: AGPL-3.0-only

import { NavLink } from "react-router-dom";
import { useState, type ReactNode } from "react";
import { usePermissions } from "../hooks/usePermissions";
import { useModelStatus } from "../hooks/useModelStatus";
import { PERM } from "../permissions";
import { useTheme } from "../context/ThemeContext";
import {
  IconKey,
  IconModels,
  IconMoon,
  IconProfile,
  IconReleases,
  IconStages,
  IconSun,
  IconUsers,
  IconBackup,
} from "./icons";

type RailItemProps = {
  to: string;
  label: string;
  icon: ReactNode;
  end?: boolean;
  disabled?: boolean;
  onNavigate?: () => void;
  badgeCount?: number;
  badgeTitle?: string;
};

function RailIcon({
  icon,
  badgeCount,
  badgeTitle,
}: {
  icon: ReactNode;
  badgeCount?: number;
  badgeTitle?: string;
}) {
  const showBadge = (badgeCount ?? 0) > 0;
  return (
    <span className="rail-icon-wrap">
      {icon}
      {showBadge && (
        <span className="rail-badge" title={badgeTitle} aria-label={badgeTitle}>
          {badgeCount! > 9 ? "9+" : badgeCount}
        </span>
      )}
    </span>
  );
}

function RailItem({ to, label, icon, end, disabled, onNavigate, badgeCount, badgeTitle }: RailItemProps) {
  if (disabled) {
    return (
      <span className="rail-item disabled" aria-disabled title={`Missing permission for ${label}`}>
        <RailIcon icon={icon} badgeCount={badgeCount} badgeTitle={badgeTitle} />
        <span className="rail-label">{label}</span>
      </span>
    );
  }

  return (
    <NavLink
      to={to}
      end={end}
      className={({ isActive }) => `rail-item ${isActive ? "active" : ""}`}
      onClick={onNavigate}
    >
      <RailIcon icon={icon} badgeCount={badgeCount} badgeTitle={badgeTitle} />
      <span className="rail-label">{label}</span>
    </NavLink>
  );
}

export function PrimaryRail() {
  const { can, ready } = usePermissions();
  const { theme, toggle } = useTheme();
  const isDark = theme === "dark";
  const [collapsed, setCollapsed] = useState(false);

  const collapseRail = () => setCollapsed(true);

  const stagesDisabled = ready && !can(PERM.stages.read);
  const releasesDisabled = ready && !can(PERM.releases.read);
  const apiKeysDisabled = ready && !can(PERM.apiKeys.read);
  const backupsDisabled = ready && !can(PERM.backups.read);
  const modelsDisabled = ready && !can(PERM.models.read);
  const usersDisabled = ready && !can(PERM.users.read);

  const canReadModels = ready && can(PERM.models.read);
  const { mismatchCount, missingCount, hasIssues } = useModelStatus(canReadModels);
  const modelIssueCount = mismatchCount + missingCount;
  const modelBadgeTitle = hasIssues
    ? [
        mismatchCount > 0 ? `${mismatchCount} embedding mismatch${mismatchCount > 1 ? "es" : ""}` : null,
        missingCount > 0 ? `${missingCount} missing model${missingCount > 1 ? "s" : ""}` : null,
      ]
        .filter(Boolean)
        .join(", ")
    : undefined;

  return (
    <div
      className={`rail-shell${collapsed ? " rail-collapsed" : ""}`}
      onMouseLeave={() => setCollapsed(false)}
    >
      <aside className="rail-panel">
        <div className="flex items-center gap-3 border-b px-3 py-4" style={{ borderColor: "var(--border)" }}>
          <img src="/assets/logo.png" alt="" className="h-8 w-8 shrink-0" aria-hidden />
          <span className="rail-label text-base font-semibold">Ragdoll</span>
        </div>

        <nav className="flex flex-1 flex-col gap-1 overflow-hidden p-2">
          <RailItem
            to="/stages"
            label="Stages"
            icon={<IconStages />}
            end
            disabled={stagesDisabled}
            onNavigate={collapseRail}
          />
          <RailItem
            to="/releases"
            label="Releases"
            icon={<IconReleases />}
            end
            disabled={releasesDisabled}
            onNavigate={collapseRail}
          />
          <RailItem
            to="/api-keys"
            label="API Keys"
            icon={<IconKey />}
            end
            disabled={apiKeysDisabled}
            onNavigate={collapseRail}
          />
          <RailItem
            to="/models"
            label="Models"
            icon={<IconModels />}
            end
            disabled={modelsDisabled}
            onNavigate={collapseRail}
            badgeCount={modelIssueCount}
            badgeTitle={modelBadgeTitle}
          />
          <RailItem
            to="/backups"
            label="Backups"
            icon={<IconBackup />}
            end
            disabled={backupsDisabled}
            onNavigate={collapseRail}
          />
          <RailItem
            to="/users"
            label="Users"
            icon={<IconUsers />}
            end
            disabled={usersDisabled}
            onNavigate={collapseRail}
          />
        </nav>

        <div className="mt-auto flex flex-col gap-1 border-t p-2" style={{ borderColor: "var(--border)" }}>
          <button
            type="button"
            className="rail-item w-full text-left"
            onClick={() => {
              collapseRail();
              toggle();
            }}
            aria-label={isDark ? "Switch to light mode" : "Switch to dark mode"}
          >
            <span className="flex shrink-0 items-center justify-center" style={{ width: 20 }}>
              {isDark ? <IconSun /> : <IconMoon />}
            </span>
            <span className="rail-label">{isDark ? "Light mode" : "Dark mode"}</span>
          </button>
          <RailItem to="/profile" label="Profile" icon={<IconProfile />} end onNavigate={collapseRail} />
        </div>
      </aside>
    </div>
  );
}
