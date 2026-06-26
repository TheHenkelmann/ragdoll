# SPDX-License-Identifier: AGPL-3.0-only

from __future__ import annotations

from unittest.mock import MagicMock, patch

import pytest

from ragdoll_worker.config import WorkerConfig
from ragdoll_worker.db import WorkerDb
from ragdoll_worker.worker import _sqlite_ms_delta, process_job


def test_sqlite_ms_delta_returns_zero_for_missing_values() -> None:
    assert _sqlite_ms_delta(None, "2024-01-01 00:00:01") == 0
    assert _sqlite_ms_delta("2024-01-01 00:00:00", None) == 0


def test_sqlite_ms_delta_computes_positive_delta() -> None:
    delta = _sqlite_ms_delta("2024-01-01 00:00:00", "2024-01-01 00:00:02")
    assert delta == 2000


def test_sqlite_ms_delta_returns_zero_for_invalid_timestamps() -> None:
    assert _sqlite_ms_delta("not-a-date", "2024-01-01 00:00:00") == 0


def _insert_pending_job(
    db: WorkerDb,
    *,
    job_id: str,
    source_id: str,
    staging_text: str,
    worker_config: WorkerConfig,
) -> None:
    release_id = "00000000-0000-0000-0000-000000000001"
    staging_path = worker_config.staging_dir / f"{source_id}.txt"
    staging_path.write_text(staging_text, encoding="utf-8")
    db.conn.execute(
        """
        INSERT OR REPLACE INTO sources (id, release_id, name, type, uri, status)
        VALUES (?, ?, 'demo', 'text', NULL, 'processing')
        """,
        (source_id, release_id),
    )
    db.conn.execute(
        """
        INSERT OR REPLACE INTO ingest_jobs (
            id, release_id, source_id, status, attempts, max_attempts,
            created_at, started_at
        ) VALUES (?, ?, ?, 'processing', 1, 3, '2024-01-01 00:00:00', '2024-01-01 00:00:01')
        """,
        (job_id, release_id, source_id),
    )
    commit = getattr(db.conn, "commit", None)
    if callable(commit):
        commit()


@patch("ragdoll_worker.worker.semantic_split_chunk")
def test_process_job_writes_chunks_and_metrics(
    mock_split: MagicMock,
    worker_db: WorkerDb,
    worker_config: WorkerConfig,
) -> None:
    mock_split.return_value = [
        {
            "id": "chunk-1",
            "content": "hello ragdoll",
            "provenance": [],
            "metadata": {},
            "embedding": [0.1, 0.2],
            "token_count": 2,
        }
    ]
    embedder = MagicMock()
    _insert_pending_job(
        worker_db,
        job_id="job-process-1",
        source_id="source-process-1",
        staging_text="hello ragdoll",
        worker_config=worker_config,
    )

    process_job(
        worker_db,
        worker_config,
        embedder,
        {
            "id": "job-process-1",
            "source_id": "source-process-1",
            "created_at": "2024-01-01 00:00:00",
            "started_at": "2024-01-01 00:00:01",
        },
    )

    row = worker_db.conn.execute(
        "SELECT chunk_count, char_count, total_ms FROM ingest_jobs WHERE id = 'job-process-1'",
    ).fetchone()
    assert row is not None
    assert row[0] == 1
    assert row[1] == len("hello ragdoll")
    assert row[2] >= 0

    chunk_count = worker_db.conn.execute(
        "SELECT COUNT(*) FROM chunks WHERE source_id = 'source-process-1'",
    ).fetchone()[0]
    assert chunk_count == 1
    mock_split.assert_called_once()


def test_process_job_rejects_empty_extract(worker_db: WorkerDb, worker_config: WorkerConfig) -> None:
    embedder = MagicMock()
    _insert_pending_job(
        worker_db,
        job_id="job-empty",
        source_id="source-empty",
        staging_text="   ",
        worker_config=worker_config,
    )

    with pytest.raises(RuntimeError, match="extracted text is empty"):
        process_job(
            worker_db,
            worker_config,
            embedder,
            {"id": "job-empty", "source_id": "source-empty"},
        )


def test_process_job_rejects_unsupported_strategy(
    worker_db: WorkerDb,
    worker_config: WorkerConfig,
) -> None:
    release_id = "00000000-0000-0000-0000-000000000001"
    worker_db.conn.execute(
        "INSERT OR REPLACE INTO settings (release_id, key, value) VALUES (?, 'chunking_strategy', ?)",
        (release_id, '"fixed"'),
    )
    commit = getattr(worker_db.conn, "commit", None)
    if callable(commit):
        commit()

    _insert_pending_job(
        worker_db,
        job_id="job-strategy",
        source_id="source-strategy",
        staging_text="some content here",
        worker_config=worker_config,
    )

    with pytest.raises(RuntimeError, match="unsupported chunking_strategy"):
        process_job(
            worker_db,
            worker_config,
            MagicMock(),
            {"id": "job-strategy", "source_id": "source-strategy"},
        )
