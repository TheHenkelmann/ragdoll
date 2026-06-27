// SPDX-License-Identifier: AGPL-3.0-only

import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { ReleaseRecord, StageRecord, api } from "../api/client";
import { PermissionDenied } from "../components/PermissionDenied";
import {
  CreateTagControl,
  DeleteConfirmDialog,
  EditableTag,
  ForkControl,
  ViewButton,
} from "../components/ObjectOverview";
import { usePermissions } from "../hooks/usePermissions";
import { PERM } from "../permissions";
import { useSnackbar } from "../context/SnackbarContext";
import { pushApiError } from "../utils/snackbarFormat";

type OverviewKind = "release" | "stage";

type Props = {
  kind: OverviewKind;
};

export function ObjectOverviewPage({ kind }: Props) {
  const snackbar = useSnackbar();
  const navigate = useNavigate();
  const { can, ready } = usePermissions();
  const isRelease = kind === "release";
  const readPerm = isRelease ? PERM.releases.read : PERM.stages.read;
  const writePerm = isRelease ? PERM.releases.write : PERM.stages.write;
  const deletePerm = isRelease ? PERM.releases.delete : PERM.stages.delete;
  const canRead = can(readPerm);
  const canWrite = can(writePerm);
  const canDelete = can(deletePerm);
  const canReadReleases = can(PERM.releases.read);
  const title = isRelease ? "Releases" : "Stages";
  const typeLabel = kind;
  const searchPlaceholder = isRelease ? "Search releases" : "Search stages";

  const [search, setSearch] = useState("");
  const [loading, setLoading] = useState(true);
  const [releases, setReleases] = useState<ReleaseRecord[]>([]);
  const [stages, setStages] = useState<StageRecord[]>([]);
  const [deleteTarget, setDeleteTarget] = useState<{ tag: string } | null>(null);

  const reload = () => {
    if (!canRead) {
      setLoading(false);
      return;
    }
    setLoading(true);
    if (isRelease) {
      void api<ReleaseRecord[]>("/releases")
        .then(setReleases)
        .catch((err) => pushApiError(snackbar.error, err))
        .finally(() => setLoading(false));
      return;
    }
    const fetches: Promise<void>[] = [
      api<StageRecord[]>("/stages").then(setStages).then(() => undefined),
    ];
    if (canReadReleases) {
      fetches.push(api<ReleaseRecord[]>("/releases").then(setReleases).then(() => undefined));
    } else {
      setReleases([]);
    }
    void Promise.all(fetches)
      .catch((err) => pushApiError(snackbar.error, err))
      .finally(() => setLoading(false));
  };

  useEffect(() => {
    if (!ready) return;
    reload();
  }, [kind, ready, canRead, canReadReleases]);

  const filteredReleases = releases.filter(
    (r) =>
      r.tag.includes(search) ||
      r.id.includes(search) ||
      r.stage_tags.some((s) => s.includes(search)),
  );
  const filteredStages = stages.filter(
    (s) =>
      s.tag.includes(search) ||
      s.id.includes(search) ||
      s.release_tag.includes(search),
  );

  async function retargetStage(stage: StageRecord, releaseTag: string) {
    try {
      await api(`/stages/${stage.tag}`, {
        method: "PATCH",
        body: JSON.stringify({ release_tag: releaseTag }),
      });
      reload();
    } catch (err) {
      pushApiError(snackbar.error, err);
    }
  }

  async function forkRelease(sourceTag: string, tag: string) {
    try {
      await api("/releases", {
        method: "POST",
        body: JSON.stringify({ tag, init: { type: "fork", source: sourceTag } }),
      });
      reload();
    } catch (err) {
      pushApiError(snackbar.error, err);
      throw err;
    }
  }

  async function renameTag(currentTag: string, tag: string) {
    try {
      await api(`/${isRelease ? "releases" : "stages"}/${currentTag}`, {
        method: "PATCH",
        body: JSON.stringify({ tag }),
      });
      reload();
    } catch (err) {
      pushApiError(snackbar.error, err);
      throw err;
    }
  }

  function viewPath(tag: string) {
    return isRelease ? `/releases/${tag}` : `/stages/${tag}`;
  }

  if (ready && !canRead) {
    return <PermissionDenied permission={readPerm} />;
  }

  return (
    <div className="space-y-6">
      <h2 className="text-2xl font-semibold">{title}</h2>

      <div className="flex flex-wrap items-center gap-3">
        <CreateTagControl
          label={isRelease ? "Create empty release" : "Create stage"}
          maxLength={isRelease ? 50 : 12}
          disabled={!canWrite}
          onCreate={async (tag) => {
            try {
              if (isRelease) {
                await api("/releases", {
                  method: "POST",
                  body: JSON.stringify({ tag, init: { type: "new" } }),
                });
              } else {
                await api("/stages", {
                  method: "POST",
                  body: JSON.stringify({ tag }),
                });
              }
              reload();
            } catch (err) {
              pushApiError(snackbar.error, err);
              throw err;
            }
          }}
        />
        <input
          className="input min-w-[240px] flex-1"
          placeholder={searchPlaceholder}
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
      </div>

      <div className="space-y-2">
        {loading && <p className="text-sm text-[var(--muted)]">Loading…</p>}
        {!loading && isRelease && filteredReleases.length === 0 && (
          <p className="text-sm text-[var(--muted)]">No releases found.</p>
        )}
        {!loading && !isRelease && filteredStages.length === 0 && (
          <p className="text-sm text-[var(--muted)]">No stages found.</p>
        )}

        {isRelease &&
          filteredReleases.map((r) => (
            <div
              key={r.id}
              className="flex flex-wrap items-center gap-3 rounded-lg border px-4 py-3"
              style={{ borderColor: "var(--border)" }}
            >
              <ViewButton onClick={() => navigate(viewPath(r.tag))} />
              <ForkControl
                sourceTag={r.tag}
                maxLength={50}
                disabled={!canWrite}
                onFork={(tag) => forkRelease(r.tag, tag)}
              />
              <EditableTag
                tag={r.tag}
                maxLength={50}
                disabled={!canWrite}
                onRename={(tag) => renameTag(r.tag, tag)}
                subtitle={
                  r.stage_tags.length > 0 ? (
                    <div className="text-xs text-[var(--muted)]">
                      Stages: {r.stage_tags.join(", ")}
                    </div>
                  ) : undefined
                }
              />
              <button
                type="button"
                className="btn-danger ml-auto shrink-0"
                disabled={!canDelete}
                onClick={() => setDeleteTarget({ tag: r.tag })}
              >
                Delete
              </button>
            </div>
          ))}

        {!isRelease &&
          filteredStages.map((s) => (
            <div
              key={s.id}
              className="flex flex-wrap items-center gap-3 rounded-lg border px-4 py-3"
              style={{ borderColor: "var(--border)" }}
            >
              <ViewButton onClick={() => navigate(viewPath(s.tag))} />
              <EditableTag
                tag={s.tag}
                maxLength={12}
                disabled={!canWrite}
                onRename={(tag) => renameTag(s.tag, tag)}
              />
              <select
                className="input max-w-[200px]"
                value={s.release_tag}
                disabled={!canWrite || !canReadReleases}
                onChange={(e) => void retargetStage(s, e.target.value)}
              >
                <option value="">No release</option>
                {releases.map((r) => (
                  <option key={r.id} value={r.tag}>
                    {r.tag}
                  </option>
                ))}
              </select>
              <button
                type="button"
                className="btn-danger ml-auto shrink-0"
                disabled={!canDelete}
                onClick={() => setDeleteTarget({ tag: s.tag })}
              >
                Delete
              </button>
            </div>
          ))}
      </div>

      <DeleteConfirmDialog
        open={deleteTarget !== null}
        typeLabel={typeLabel}
        tag={deleteTarget?.tag ?? ""}
        onClose={() => setDeleteTarget(null)}
        onConfirm={async () => {
          if (!deleteTarget) return;
          try {
            if (isRelease) {
              await api(`/releases/${deleteTarget.tag}`, { method: "DELETE" });
            } else {
              await api(`/stages/${deleteTarget.tag}`, { method: "DELETE" });
            }
            reload();
          } catch (err) {
            pushApiError(snackbar.error, err);
            throw err;
          }
        }}
      />
    </div>
  );
}
