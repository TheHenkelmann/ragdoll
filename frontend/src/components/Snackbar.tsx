// SPDX-License-Identifier: AGPL-3.0-only

import type { SnackbarItem, SnackbarType } from "../context/SnackbarContext";
import { useSnackbarInternal } from "../context/SnackbarContext";

const TYPE_ACCENT: Record<SnackbarType, string> = {
  error: "var(--danger)",
  success: "var(--positive)",
  warning: "var(--warning)",
  info: "var(--snackbar-info)",
};

const COUNTDOWN_RADIUS = 7;
const COUNTDOWN_CIRCUMFERENCE = 2 * Math.PI * COUNTDOWN_RADIUS;

function CountdownRing({ remaining, ttl, accent, paused }: { remaining: number; ttl: number; accent: string; paused: boolean }) {
  const progress = ttl > 0 ? Math.max(0, Math.min(1, remaining / ttl)) : 0;
  const offset = COUNTDOWN_CIRCUMFERENCE * (1 - progress);

  return (
    <svg
      className="snackbar-countdown shrink-0"
      width="18"
      height="18"
      viewBox="0 0 18 18"
      aria-hidden
    >
      <circle
        className="snackbar-countdown-track"
        cx="9"
        cy="9"
        r={COUNTDOWN_RADIUS}
        fill="none"
        strokeWidth="2"
      />
      <circle
        className="snackbar-countdown-progress"
        cx="9"
        cy="9"
        r={COUNTDOWN_RADIUS}
        fill="none"
        stroke={accent}
        strokeWidth="2"
        strokeDasharray={COUNTDOWN_CIRCUMFERENCE}
        strokeDashoffset={offset}
        transform="rotate(-90 9 9)"
        style={{ transition: paused ? "none" : "stroke-dashoffset 0.05s linear" }}
      />
    </svg>
  );
}

function SnackbarItemView({ item, paused }: { item: SnackbarItem; paused: boolean }) {
  const { dismiss, toggleExpanded } = useSnackbarInternal();
  const accent = TYPE_ACCENT[item.type];
  const titleSuffix = item.count > 1 ? ` ×${item.count}` : "";
  const canExpand = Boolean(item.body);

  function handleToggle() {
    if (canExpand) toggleExpanded(item.id);
  }

  return (
    <div
      className={`snackbar-item${canExpand ? " snackbar-item-expandable" : ""}`}
      style={{ borderLeftColor: accent }}
      data-type={item.type}
      role={canExpand ? "button" : undefined}
      tabIndex={canExpand ? 0 : undefined}
      onClick={canExpand ? handleToggle : undefined}
      onKeyDown={
        canExpand
          ? (e) => {
              if (e.key === "Enter" || e.key === " ") {
                e.preventDefault();
                handleToggle();
              }
            }
          : undefined
      }
    >
      <div className="snackbar-header">
        <CountdownRing remaining={item.remaining} ttl={item.ttl} accent={accent} paused={paused} />
        <span className="snackbar-title truncate">
          {item.title}
          {titleSuffix && <span className="snackbar-count">{titleSuffix}</span>}
        </span>
        <button
          type="button"
          className="snackbar-dismiss"
          aria-label="Dismiss"
          onClick={(e) => {
            e.stopPropagation();
            dismiss(item.id);
          }}
        >
          ×
        </button>
      </div>
      {item.expanded && item.body && (
        <div className="snackbar-body">{item.body}</div>
      )}
      {!item.expanded && item.body && (
        <div className="snackbar-expand-hint" aria-hidden>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <path d="m6 9 6 6 6-6" />
          </svg>
        </div>
      )}
    </div>
  );
}

export function SnackbarContainer() {
  const { items, paused, setPaused } = useSnackbarInternal();

  if (items.length === 0) return null;

  return (
    <div
      className="snackbar-container"
      onMouseEnter={() => setPaused(true)}
      onMouseLeave={() => setPaused(false)}
      aria-live="polite"
    >
      {items.map((item) => (
        <SnackbarItemView key={item.id} item={item} paused={paused} />
      ))}
    </div>
  );
}
