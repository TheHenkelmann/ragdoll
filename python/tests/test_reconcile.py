# SPDX-License-Identifier: AGPL-3.0-only

from __future__ import annotations

from datetime import datetime, timedelta, timezone

from ragdoll_worker.config import WorkerConfig
from ragdoll_worker.db import WorkerDb
from ragdoll_worker.reconcile import reconcile_jobs


def _insert_processing_job(
    db: WorkerDb,
    *,
    job_id: str,
    source_id: str,
    heartbeat_at: str,
    source_type: str = "text",
    uri: str | None = None,
) -> None:
    release_id = "00000000-0000-0000-0000-000000000001"
    db.conn.execute(
        "INSERT OR REPLACE INTO sources (id, release_id, name, type, uri, status) VALUES (?, ?, 'demo', ?, ?, 'processing')",
        (source_id, release_id, source_type, uri),
    )
    db.conn.execute(
        """
        INSERT OR REPLACE INTO ingest_jobs (
            id, release_id, source_id, status, attempts, max_attempts, heartbeat_at
        ) VALUES (?, ?, ?, 'processing', 1, 3, ?)
        """,
        (job_id, release_id, source_id, heartbeat_at),
    )
    commit = getattr(db.conn, "commit", None)
    if callable(commit):
        commit()


def test_reconcile_resets_stale_processing_job(worker_db: WorkerDb, worker_config: WorkerConfig) -> None:
    stale = (datetime.now(timezone.utc) - timedelta(seconds=worker_config.job_lease_seconds + 10)).isoformat()
    _insert_processing_job(
        worker_db,
        job_id="job-1",
        source_id="source-1",
        heartbeat_at=stale,
    )
    (worker_config.staging_dir / "source-1.txt").write_text("content", encoding="utf-8")

    reconcile_jobs(worker_db, worker_config)
    commit = getattr(worker_db.conn, "commit", None)
    if callable(commit):
        commit()

    row = worker_db.conn.execute(
        "SELECT status, error FROM ingest_jobs WHERE id = 'job-1'",
    ).fetchone()
    assert row is not None
    assert row[0] == "pending"
    assert "stale" in row[1]


def test_reconcile_fails_job_with_missing_text_staging(worker_db: WorkerDb, worker_config: WorkerConfig) -> None:
    recent = datetime.now(timezone.utc).isoformat()
    _insert_processing_job(
        worker_db,
        job_id="job-2",
        source_id="source-2",
        heartbeat_at=recent,
    )

    reconcile_jobs(worker_db, worker_config)
    commit = getattr(worker_db.conn, "commit", None)
    if callable(commit):
        commit()

    row = worker_db.conn.execute(
        "SELECT status, error FROM ingest_jobs WHERE id = 'job-2'",
    ).fetchone()
    assert row is not None
    assert row[0] == "pending"
    assert "missing staging" in row[1]
