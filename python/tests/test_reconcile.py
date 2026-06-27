# SPDX-License-Identifier: AGPL-3.0-only

from __future__ import annotations

from datetime import UTC, datetime, timedelta

from ragdoll_worker.config import WorkerConfig
from ragdoll_worker.db import WorkerDb
from ragdoll_worker.reconcile import reconcile_jobs
from tests.job_helpers import insert_ingest_job


def test_reconcile_resets_stale_processing_job(
    worker_db: WorkerDb, worker_config: WorkerConfig
) -> None:
    stale = (
        datetime.now(UTC) - timedelta(seconds=worker_config.job_lease_seconds + 10)
    ).isoformat()
    insert_ingest_job(
        worker_db,
        job_id="job-1",
        source_id="source-1",
        source_type="text",
        status="processing",
        attempts=1,
        heartbeat_at=stale,
    )
    (worker_config.staging_dir / "source-1.txt").write_text("content", encoding="utf-8")

    reconcile_jobs(worker_db, worker_config)

    row = worker_db.conn.execute(
        "SELECT status, error FROM ingest_jobs WHERE id = 'job-1'",
    ).fetchone()
    assert row is not None
    assert row[0] == "pending"
    assert "stale" in row[1]


def test_reconcile_fails_job_with_missing_text_staging(
    worker_db: WorkerDb, worker_config: WorkerConfig
) -> None:
    recent = datetime.now(UTC).isoformat()
    insert_ingest_job(
        worker_db,
        job_id="job-2",
        source_id="source-2",
        source_type="text",
        status="processing",
        attempts=1,
        heartbeat_at=recent,
    )

    reconcile_jobs(worker_db, worker_config)

    row = worker_db.conn.execute(
        "SELECT status, error FROM ingest_jobs WHERE id = 'job-2'",
    ).fetchone()
    assert row is not None
    assert row[0] == "pending"
    assert "missing staging" in row[1]
