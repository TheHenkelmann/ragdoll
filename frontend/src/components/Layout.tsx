// SPDX-License-Identifier: AGPL-3.0-only

import { Outlet, useLocation, useNavigate, useParams } from "react-router-dom";
import { useEffect, useState } from "react";
import { api, ReleaseRecord, StageRecord } from "../api/client";
import { useAuth } from "../context/AuthContext";
import { usePermissions } from "../hooks/usePermissions";
import { PERM } from "../permissions";
import { PrimaryRail } from "./PrimaryRail";
import { SecondarySidebar } from "./SecondarySidebar";

const RELEASE_KEY = "ragdoll_release";

function isReleaseDetailPath(pathname: string): boolean {
  const match = pathname.match(/^\/releases\/([^/]+)(?:\/(.*))?$/);
  if (!match) return false;
  const rest = match[2];
  return rest === undefined || rest.length > 0;
}

export function Layout() {
  const { releaseTag = "", stageTag = "" } = useParams();
  const location = useLocation();
  const navigate = useNavigate();
  const { status } = useAuth();
  const { can, ready } = usePermissions();
  const [releases, setReleases] = useState<ReleaseRecord[]>([]);
  const [stages, setStages] = useState<StageRecord[]>([]);
  const [metaLoaded, setMetaLoaded] = useState(false);

  const showSecondarySidebar = isReleaseDetailPath(location.pathname);
  const canReadReleases = can(PERM.releases.read);
  const canReadStages = can(PERM.stages.read);

  useEffect(() => {
    if (!ready) return;

    const fetches: Promise<void>[] = [];
    if (canReadReleases) {
      fetches.push(api<ReleaseRecord[]>("/releases").then(setReleases));
    } else {
      setReleases([]);
    }
    if (canReadStages) {
      fetches.push(api<StageRecord[]>("/stages").then(setStages));
    } else {
      setStages([]);
    }

    if (fetches.length === 0) {
      setMetaLoaded(true);
      return;
    }

    void Promise.all(fetches)
      .catch(console.error)
      .finally(() => setMetaLoaded(true));
  }, [location.pathname, ready, canReadReleases, canReadStages]);

  useEffect(() => {
    if (!ready || !canReadReleases) return;
    if (releaseTag || stageTag || location.pathname !== "/") return;
    const saved = localStorage.getItem(RELEASE_KEY);
    const target =
      saved && releases.some((r) => r.tag === saved) ? saved : releases[0]?.tag;
    if (target) {
      navigate(`/releases/${target}`, { replace: true });
    }
  }, [ready, canReadReleases, releaseTag, stageTag, releases, navigate, location.pathname]);

  useEffect(() => {
    if (releaseTag) localStorage.setItem(RELEASE_KEY, releaseTag);
  }, [releaseTag]);

  const currentRelease = releases.find((r) => r.tag === releaseTag);
  const currentStage = stages.find((s) => s.tag === stageTag);
  const unknownRelease = Boolean(releaseTag) && metaLoaded && !currentRelease;
  const unknownStage = Boolean(stageTag) && metaLoaded && !currentStage;

  return (
    <div className="flex min-h-screen flex-col">
      {status?.password_is_default && (
        <div className="banner-warning">
          Default admin password active. Set environment variable RAGDOLL_SUPERADMIN_PW and restart
          the container.
        </div>
      )}
      <div className="flex min-h-0 flex-1">
        <PrimaryRail />
        {showSecondarySidebar && releaseTag && <SecondarySidebar releaseTag={releaseTag} />}
        <main className="min-w-0 flex-1 overflow-auto p-8">
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
