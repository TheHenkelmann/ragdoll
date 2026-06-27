# SPDX-License-Identifier: AGPL-3.0-only

from __future__ import annotations

from unittest.mock import MagicMock, patch

import pytest

from ragdoll_worker.config import WorkerConfig
from ragdoll_worker.db import WorkerDb
from ragdoll_worker.worker import _sqlite_ms_delta, process_job
from tests.job_helpers import insert_ingest_job


def test_sqlite_ms_delta_returns_zero_for_missing_values() -> None:
    assert _sqlite_ms_delta(None, "2024-01-01 00:00:01") == 0
    assert _sqlite_ms_delta("2024-01-01 00:00:00", None) == 0


def test_sqlite_ms_delta_computes_positive_delta() -> None:
    delta = _sqlite_ms_delta("2024-01-01 00:00:00", "2024-01-01 00:00:02")
    assert delta == 2000


def test_sqlite_ms_delta_returns_zero_for_invalid_timestamps() -> None:
    assert _sqlite_ms_delta("not-a-date", "2024-01-01 00:00:00") == 0


def _job_dict(job_id: str, source_id: str) -> dict[str, object]:
    return {
        "id": job_id,
        "release_id": "00000000-0000-0000-0000-000000000001",
        "stage_id": None,
        "source_id": source_id,
        "source_name": "demo",
        "source_type": "text",
        "source_uri": None,
        "content_hash": None,
        "config": "{}",
        "metadata": "{}",
        "attempts": 1,
        "max_attempts": 3,
        "created_at": "2024-01-01 00:00:00",
        "started_at": "2024-01-01 00:00:01",
    }


def _insert_pending_job(
    db: WorkerDb,
    *,
    job_id: str,
    source_id: str,
    staging_text: str,
    worker_config: WorkerConfig,
) -> dict[str, object]:
    staging_path = worker_config.staging_dir / f"{source_id}.txt"
    staging_path.write_text(staging_text, encoding="utf-8")
    insert_ingest_job(
        db,
        job_id=job_id,
        source_id=source_id,
        source_name="demo",
        source_type="text",
    )
    return _job_dict(job_id, source_id)


@patch("ragdoll_worker.worker._embedder_for")
@patch("ragdoll_worker.worker.semantic_split_chunk")
def test_process_job_creates_source_and_chunks_atomically(
    mock_split: MagicMock,
    mock_embedder_for: MagicMock,
    worker_db: WorkerDb,
    worker_config: WorkerConfig,
) -> None:
    mock_split.return_value = [
        {
            "id": "chunk-1",
            "content": "hello ragdoll",
            "provenance": [],
            "metadata": {},
            "embedding": [0.1] * 1024,
            "token_count": 2,
        }
    ]
    embedder = MagicMock()
    mock_embedder_for.return_value = embedder
    _insert_pending_job(
        worker_db,
        job_id="job-process-1",
        source_id="source-process-1",
        staging_text="hello ragdoll",
        worker_config=worker_config,
    )

    before = worker_db.conn.execute(
        "SELECT COUNT(*) FROM sources WHERE id = 'source-process-1'",
    ).fetchone()[0]
    assert before == 0

    process_job(worker_db, worker_config, _job_dict("job-process-1", "source-process-1"))

    source_count = worker_db.conn.execute(
        "SELECT COUNT(*) FROM sources WHERE id = 'source-process-1'",
    ).fetchone()[0]
    chunk_count = worker_db.conn.execute(
        "SELECT COUNT(*) FROM chunks WHERE source_id = 'source-process-1'",
    ).fetchone()[0]
    text_count = worker_db.conn.execute(
        "SELECT COUNT(*) FROM source_texts WHERE source_id = 'source-process-1'",
    ).fetchone()[0]
    assert source_count == 1
    assert chunk_count == 1
    assert text_count == 1
    mock_split.assert_called_once()


@patch("ragdoll_worker.worker._embedder_for")
def test_process_job_rejects_empty_extract(
    mock_embedder_for: MagicMock,
    worker_db: WorkerDb,
    worker_config: WorkerConfig,
) -> None:
    mock_embedder_for.return_value = MagicMock()
    _insert_pending_job(
        worker_db,
        job_id="job-empty",
        source_id="source-empty",
        staging_text="   ",
        worker_config=worker_config,
    )

    with pytest.raises(RuntimeError, match="extracted text is empty"):
        process_job(worker_db, worker_config, _job_dict("job-empty", "source-empty"))

    source_count = worker_db.conn.execute(
        "SELECT COUNT(*) FROM sources WHERE id = 'source-empty'",
    ).fetchone()[0]
    assert source_count == 0


@patch("ragdoll_worker.worker._embedder_for")
@patch("ragdoll_worker.worker.semantic_split_chunk")
def test_process_job_rejects_zero_chunks(
    mock_split: MagicMock,
    mock_embedder_for: MagicMock,
    worker_db: WorkerDb,
    worker_config: WorkerConfig,
) -> None:
    mock_split.return_value = []
    mock_embedder_for.return_value = MagicMock()
    _insert_pending_job(
        worker_db,
        job_id="job-zero-chunks",
        source_id="source-zero-chunks",
        staging_text="some content here",
        worker_config=worker_config,
    )

    with pytest.raises(RuntimeError, match="chunking produced no chunks"):
        process_job(
            worker_db,
            worker_config,
            _job_dict("job-zero-chunks", "source-zero-chunks"),
        )

    source_count = worker_db.conn.execute(
        "SELECT COUNT(*) FROM sources WHERE id = 'source-zero-chunks'",
    ).fetchone()[0]
    assert source_count == 0


@patch("ragdoll_worker.worker._embedder_for")
@patch("ragdoll_worker.worker.semantic_split_chunk")
def test_process_job_preserves_existing_chunks_on_failure(
    mock_split: MagicMock,
    mock_embedder_for: MagicMock,
    worker_db: WorkerDb,
    worker_config: WorkerConfig,
) -> None:
    release_id = "00000000-0000-0000-0000-000000000001"
    source_id = "source-rechunk-fail"
    worker_db.conn.execute(
        """
        INSERT OR REPLACE INTO sources (
            id, release_id, name, type, status, content_hash, config, metadata
        ) VALUES (?, ?, 'demo', 'text', 'completed', 'abc', '{}', '{}')
        """,
        (source_id, release_id),
    )
    worker_db.conn.execute(
        """
        INSERT OR REPLACE INTO source_texts (source_id, text, char_len)
        VALUES (?, 'old text stays', 14)
        """,
        (source_id,),
    )
    worker_db.conn.execute(
        """
        INSERT OR REPLACE INTO chunks (
            id, release_id, source_id, ordinal, content, provenance, metadata,
            embedding, embedding_model, embedding_dim, embedding_version
        ) VALUES ('old-chunk', ?, ?, 0, 'old chunk', '[]', '{}', vector32(?), 'BAAI/bge-m3', 1024, '1')
        """,
        (release_id, source_id, str([0.0] * 1024)),
    )
    commit = getattr(worker_db.conn, "commit", None)
    if callable(commit):
        commit()

    mock_split.return_value = []
    mock_embedder_for.return_value = MagicMock()
    insert_ingest_job(
        worker_db,
        job_id="job-rechunk-fail",
        source_id=source_id,
        source_name="demo",
        source_type="text",
    )

    with pytest.raises(RuntimeError, match="chunking produced no chunks"):
        process_job(
            worker_db,
            worker_config,
            _job_dict("job-rechunk-fail", source_id),
        )

    chunk_count = worker_db.conn.execute(
        "SELECT COUNT(*) FROM chunks WHERE source_id = ?",
        (source_id,),
    ).fetchone()[0]
    content = worker_db.conn.execute(
        "SELECT content FROM chunks WHERE source_id = ?",
        (source_id,),
    ).fetchone()[0]
    assert chunk_count == 1
    assert content == "old chunk"


@patch("ragdoll_worker.worker._embedder_for")
def test_process_job_rejects_unsupported_strategy(
    mock_embedder_for: MagicMock,
    worker_db: WorkerDb,
    worker_config: WorkerConfig,
) -> None:
    mock_embedder_for.return_value = MagicMock()
    release_id = "00000000-0000-0000-0000-000000000001"
    worker_db.conn.execute(
        """
        INSERT OR REPLACE INTO settings (release_id, key, value)
        VALUES (?, 'chunking_strategy', ?)
        """,
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
            _job_dict("job-strategy", "source-strategy"),
        )
