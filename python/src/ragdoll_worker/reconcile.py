# SPDX-License-Identifier: AGPL-3.0-only

from __future__ import annotations

from datetime import datetime, timedelta, timezone
from pathlib import Path

from ragdoll_worker.config import WorkerConfig
from ragdoll_worker.db import WorkerDb


def reconcile_jobs(db: WorkerDb, config: WorkerConfig) -> None:
    lease = timedelta(seconds=config.job_lease_seconds)
    now = datetime.now(timezone.utc)
    rows = db.conn.execute(
        """
        SELECT j.id, j.source_id, j.attempts, j.max_attempts, j.heartbeat_at, s.type, s.uri
        FROM ingest_jobs j
        JOIN sources s ON s.id = j.source_id
        WHERE j.status = 'processing'
        """
    ).fetchall()

    for job_id, source_id, attempts, max_attempts, heartbeat_at, source_type, uri in rows:
        stale = True
        if heartbeat_at:
            heartbeat = datetime.fromisoformat(heartbeat_at.replace("Z", "+00:00"))
            stale = now - heartbeat > lease

        missing_staging = False
        if source_type == "file" and uri:
            missing_staging = not Path(str(uri)).exists()
        if source_type == "text":
            missing_staging = not (config.staging_dir / f"{source_id}.txt").exists()

        if not stale and not missing_staging:
            continue

        retry = attempts < max_attempts
        status = "pending" if retry else "failed"
        reason = "reconciled stale processing job" if stale else "missing staging artifact"
        db.conn.execute(
            "UPDATE ingest_jobs SET status = ?, error = ?, finished_at = datetime('now') WHERE id = ?",
            (status, reason, job_id),
        )
        db.conn.execute(
            "UPDATE sources SET status = ?, error = ?, updated_at = datetime('now') WHERE id = ?",
            (status if status == "failed" else "processing", reason, source_id),
        )
