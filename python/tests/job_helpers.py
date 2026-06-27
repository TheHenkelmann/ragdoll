# SPDX-License-Identifier: AGPL-3.0-only

from __future__ import annotations

from ragdoll_worker.db import WorkerDb


def insert_ingest_job(
    db: WorkerDb,
    *,
    job_id: str,
    source_id: str,
    source_name: str = "demo",
    source_type: str = "text",
    source_uri: str | None = None,
    status: str = "pending",
    attempts: int = 0,
    max_attempts: int = 3,
    created_at: str = "2024-01-01 00:00:00",
    started_at: str | None = None,
    heartbeat_at: str | None = None,
) -> None:
    release_id = "00000000-0000-0000-0000-000000000001"
    db.conn.execute(
        """
        INSERT OR REPLACE INTO ingest_jobs (
            id, release_id, source_id, source_name, source_type, source_uri,
            config, metadata, status, attempts, max_attempts,
            created_at, started_at, heartbeat_at
        ) VALUES (?, ?, ?, ?, ?, ?, '{}', '{}', ?, ?, ?, ?, ?, ?)
        """,
        (
            job_id,
            release_id,
            source_id,
            source_name,
            source_type,
            source_uri,
            status,
            attempts,
            max_attempts,
            created_at,
            started_at,
            heartbeat_at,
        ),
    )
    commit = getattr(db.conn, "commit", None)
    if callable(commit):
        commit()
