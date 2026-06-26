// SPDX-License-Identifier: AGPL-3.0-only

import { NavLink, Outlet, useLocation, useNavigate, useParams } from "react-router-dom";
import { useEffect, useState } from "react";
import { api, ReleaseRecord, StageRecord } from "../api/client";
import { useAuth } from "../context/AuthContext";
import { ThemeToggle } from "./ThemeToggle";
import { UserMenu } from "./UserMenu";

const RELEASE_KEY = "ragdoll_release";

export function Layout() {
  const { releaseTag = "", stageTag = "" } = useParams();
  const location = useLocation();
  const navigate = useNavigate();
  const { status } = useAuth();
  const [releases, setReleases] = useState<ReleaseRecord[]>([]);
  const [stages, setStages] = useState<StageRecord[]>([]);
  const [metaLoaded, setMetaLoaded] = useState(false);

  const isOverview =
    location.pathname === "/releases" || location.pathname === "/stages";
  const isStageView = Boolean(stageTag);

  const reloadMeta = () => {
    void Promise.all([
      api<ReleaseRecord[]>("/releases").then(setReleases),
      api<StageRecord[]>("/stages").then(setStages),
    ])
      .catch(console.error)
      .finally(() => setMetaLoaded(true));
  };

  useEffect(() => {
    reloadMeta();
  }, [location.pathname]);

  useEffect(() => {
    if (isOverview || releaseTag) return;
    const saved = localStorage.getItem(RELEASE_KEY);
    const target =
      saved && releases.some((r) => r.tag === saved) ? saved : releases[0]?.tag;
    if (target && location.pathname === "/") {
      navigate(`/releases/${target}/dashboard`, { replace: true });
    }
  }, [releaseTag, releases, navigate, isOverview, location.pathname]);

  useEffect(() => {
    if (releaseTag) localStorage.setItem(RELEASE_KEY, releaseTag);
  }, [releaseTag]);

  const currentRelease = releases.find((r) => r.tag === releaseTag);
  const currentStage = stages.find((s) => s.tag === stageTag);
  const linkedStages = stages.filter((s) => s.release_tag === releaseTag);
  const unknownRelease = Boolean(releaseTag) && metaLoaded && !currentRelease;
  const unknownStage = Boolean(stageTag) && metaLoaded && !currentStage;

  const nav = (path: string) =>
    path === "dashboard" ? `/releases/${releaseTag}` : `/releases/${releaseTag}/${path}`;

  return (
    <div className="min-h-screen">
      {status?.password_is_default && (
        <div className="banner-warning">
          Default admin password active. Set environment variable RAGDOLL_SUPERADMIN_PW and restart the container.
        </div>
      )}
      <header
        className="flex items-center justify-between border-b px-6 py-4"
        style={{ borderColor: "var(--border)" }}
      >
        <div className="flex items-center gap-3 text-sm">
          <div className="flex items-center gap-2">
            <img src="/assets/logo.png" alt="" className="h-8 w-auto" aria-hidden />
            <span className="text-lg font-semibold">Ragdoll</span>
          </div>
          <span className="text-[var(--muted)]">›</span>
          <button
            type="button"
            className="breadcrumb-link"
            onClick={() => navigate("/stages")}
          >
            {stageTag ? stageTag : "Stages"}
          </button>
          <span className="text-[var(--muted)]">·</span>
          <button
            type="button"
            className="breadcrumb-link"
            onClick={() => {
              if (stageTag && currentStage?.release_tag) {
                navigate(`/releases/${currentStage.release_tag}`);
                return;
              }
              navigate("/releases");
            }}
          >
            {releaseTag && !isOverview ? (
              <>
                {currentRelease?.tag ?? releaseTag}
                {linkedStages.length > 0 && (
                  <span className="text-[var(--muted)]">
                    {" "}
                    ({linkedStages.map((s) => s.tag).join(", ")})
                  </span>
                )}
              </>
            ) : stageTag && currentStage ? (
              currentStage.release_tag || "No release"
            ) : (
              "Releases"
            )}
          </button>
        </div>
        <div className="flex items-center gap-2">
          <ThemeToggle />
          <UserMenu />
        </div>
      </header>

      <div className="flex">
        {!isOverview && !isStageView && (
          <aside className="w-56 border-r p-4" style={{ borderColor: "var(--border)" }}>
            <nav className="space-y-1">
              {[
                ["dashboard", "Dashboard"],
                ["playground", "Playground"],
                ["sources", "Sources"],
                ["database", "Database"],
                ["settings", "Settings"],
              ].map(([path, label]) => (
                <NavLink
                  key={path}
                  to={nav(path)}
                  end={path === "dashboard"}
                  className={({ isActive }) => `nav-item ${isActive ? "active" : ""}`}
                >
                  {label}
                </NavLink>
              ))}
            </nav>
          </aside>
        )}
        <main className="min-w-0 flex-1 p-8">
          {unknownRelease ? (
            <div className="card">
              <h2 className="text-xl font-semibold">Release not found</h2>
              <p className="mt-2 text-sm text-[var(--muted)]">
                No release with tag <code>{releaseTag}</code>.
              </p>
            </div>
          ) : unknownStage ? (
            <div className="card">
              <h2 className="text-xl font-semibold">Stage not found</h2>
              <p className="mt-2 text-sm text-[var(--muted)]">
                No stage with tag <code>{stageTag}</code>.
              </p>
            </div>
          ) : (
            <Outlet context={{ releaseTag, stageTag, stages, releases }} />
          )}
        </main>
      </div>
    </div>
  );
}
