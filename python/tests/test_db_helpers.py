# SPDX-License-Identifier: AGPL-3.0-only

from __future__ import annotations

import pytest

from ragdoll_worker.config import WorkerConfig
from ragdoll_worker.db import WorkerDb, _json_object
from tests.job_helpers import insert_ingest_job


def test_json_object_parses_string_payload() -> None:
    assert _json_object('{"a": 1}') == {"a": 1}


def test_json_object_returns_empty_for_null() -> None:
    assert _json_object(None) == {}
    assert _json_object("null") == {}


def test_json_object_returns_empty_for_non_object() -> None:
    assert _json_object("[1, 2]") == {}
    assert _json_object(42) == {}


def test_fetch_settings_parses_json_values(worker_db: WorkerDb) -> None:
    release_id = "00000000-0000-0000-0000-000000000001"
    settings = worker_db.fetch_settings(release_id)
    assert settings["embedding_model"] == "BAAI/bge-m3"
    assert settings["sentence_buffer"] == 2
    assert settings["dedup_policy"] == "replace"


def test_try_fetch_source_returns_metadata(worker_db: WorkerDb) -> None:
    release_id = "00000000-0000-0000-0000-000000000001"
    worker_db.conn.execute(
        """
        INSERT OR REPLACE INTO sources (id, release_id, name, type, uri, config, metadata, status)
        VALUES ('src-1', ?, 'demo', 'text', NULL, '{"k":"v"}', '{"tag":"x"}', 'completed')
        """,
        (release_id,),
    )
    commit = getattr(worker_db.conn, "commit", None)
    if callable(commit):
        commit()

    source = worker_db.try_fetch_source("src-1")
    assert source is not None
    assert source["name"] == "demo"
    assert source["config"] == {"k": "v"}
    assert source["metadata"] == {"tag": "x"}


def test_claim_and_complete_job(worker_db: WorkerDb, worker_config: WorkerConfig) -> None:
    insert_ingest_job(worker_db, job_id="job-claim", source_id="src-job")

    job = worker_db.claim_job(worker_config.worker_id)
    assert job is not None
    assert job["release_id"] == "00000000-0000-0000-0000-000000000001"
    assert job["stage_id"] is None
    assert job["source_id"] == "src-job"
    assert job["source_name"] == "demo"
    assert job["source_type"] == "text"
    assert job["attempts"] == 1

    worker_db.complete_job(str(job["id"]))
    status = worker_db.conn.execute(
        "SELECT status FROM ingest_jobs WHERE id = 'job-claim'",
    ).fetchone()[0]
    assert status == "completed"


def test_fail_job_retries_until_max_attempts(worker_db: WorkerDb) -> None:
    insert_ingest_job(
        worker_db,
        job_id="job-fail",
        source_id="src-fail",
        status="processing",
        attempts=2,
    )

    worker_db.fail_job("job-fail", "boom", retry=True)
    row = worker_db.conn.execute(
        "SELECT status, error FROM ingest_jobs WHERE id = 'job-fail'",
    ).fetchone()
    assert row[0] == "pending"
    assert row[1] == "boom"

    worker_db.fail_job("job-fail", "final", retry=False)
    row = worker_db.conn.execute(
        "SELECT status FROM ingest_jobs WHERE id = 'job-fail'",
    ).fetchone()
    assert row[0] == "failed"


def test_commit_ingested_source_replaces_existing_rows(worker_db: WorkerDb) -> None:
    release_id = "00000000-0000-0000-0000-000000000001"
    source_id = "src-chunks"
    worker_db.conn.execute(
        """
        INSERT OR REPLACE INTO sources (id, release_id, name, type, status, config, metadata)
        VALUES (?, ?, 'demo', 'text', 'completed', '{}', '{}')
        """,
        (source_id, release_id),
    )
    worker_db.conn.execute(
        """
        INSERT OR REPLACE INTO chunks (
            id, release_id, source_id, ordinal, content, provenance, metadata,
            embedding, embedding_model, embedding_dim, embedding_version
        ) VALUES ('old-chunk', ?, ?, 0, 'old', '[]', '{}', vector32(?), 'BAAI/bge-m3', 1024, '1')
        """,
        (release_id, source_id, str([0.0] * 1024)),
    )
    commit = getattr(worker_db.conn, "commit", None)
    if callable(commit):
        commit()

    worker_db.commit_ingested_source(
        source_id=source_id,
        release_id=release_id,
        name="demo",
        source_type="text",
        uri=None,
        content_hash="hash-1",
        config={},
        metadata={},
        page_map=[],
        text="updated text",
        chunks=[
            {
                "id": "chunk-a",
                "content": "first",
                "provenance": [],
                "metadata": {},
                "embedding": [0.0] * 1024,
            },
            {
                "id": "chunk-b",
                "content": "second",
                "provenance": [],
                "metadata": {"k": 1},
                "embedding": [0.1] * 1024,
            },
        ],
        embedding_model="BAAI/bge-m3",
        embedding_dim=1024,
        embedding_version="1",
        dedup_policy="replace",
    )

    count = worker_db.conn.execute(
        "SELECT COUNT(*) FROM chunks WHERE source_id = ?",
        (source_id,),
    ).fetchone()[0]
    assert count == 2
    old = worker_db.conn.execute(
        "SELECT COUNT(*) FROM chunks WHERE id = 'old-chunk'",
    ).fetchone()[0]
    assert old == 0


def test_transaction_rolls_back_on_error(worker_db: WorkerDb) -> None:
    release_id = "00000000-0000-0000-0000-000000000001"
    source_id = "src-rollback"
    worker_db.conn.execute(
        """
        INSERT OR REPLACE INTO sources (id, release_id, name, type, status, config, metadata)
        VALUES (?, ?, 'demo', 'text', 'completed', '{}', '{}')
        """,
        (source_id, release_id),
    )
    worker_db.conn.execute(
        """
        INSERT OR REPLACE INTO chunks (
            id, release_id, source_id, ordinal, content, provenance, metadata,
            embedding, embedding_model, embedding_dim, embedding_version
        ) VALUES ('keep-chunk', ?, ?, 0, 'keep', '[]', '{}', vector32(?), 'BAAI/bge-m3', 1024, '1')
        """,
        (release_id, source_id, str([0.0] * 1024)),
    )
    commit = getattr(worker_db.conn, "commit", None)
    if callable(commit):
        commit()

    with pytest.raises(RuntimeError, match="boom"):
        with worker_db.transaction():
            worker_db.connection.execute(
                "DELETE FROM chunks WHERE source_id = ?",
                (source_id,),
            )
            raise RuntimeError("boom")

    content = worker_db.conn.execute(
        "SELECT content FROM chunks WHERE source_id = ?",
        (source_id,),
    ).fetchone()[0]
    assert content == "keep"


def test_heartbeat_updates_timestamp(worker_db: WorkerDb, worker_config: WorkerConfig) -> None:
    insert_ingest_job(
        worker_db,
        job_id="job-heartbeat",
        source_id="src-heartbeat",
        status="processing",
        attempts=1,
        heartbeat_at="2024-01-01 00:00:00",
    )
    worker_db.conn.execute(
        "UPDATE ingest_jobs SET worker_id = ? WHERE id = 'job-heartbeat'",
        (worker_config.worker_id,),
    )
    commit = getattr(worker_db.conn, "commit", None)
    if callable(commit):
        commit()

    worker_db.heartbeat("job-heartbeat", worker_config.worker_id)
    row = worker_db.conn.execute(
        "SELECT heartbeat_at FROM ingest_jobs WHERE id = 'job-heartbeat'",
    ).fetchone()
    assert row[0] is not None
    assert row[0] != "2024-01-01 00:00:00"


def test_update_job_metrics_persists_values(worker_db: WorkerDb) -> None:
    insert_ingest_job(
        worker_db,
        job_id="job-metrics",
        source_id="src-metrics",
        status="processing",
        attempts=1,
    )

    worker_db.update_job_metrics(
        "job-metrics",
        {
            "queue_ms": 1,
            "extract_ms": 2,
            "chunk_ms": 3,
            "embed_ms": 4,
            "db_write_ms": 5,
            "total_ms": 15,
            "chunk_count": 7,
            "char_count": 100,
        },
    )
    row = worker_db.conn.execute(
        """
        SELECT queue_ms, extract_ms, chunk_ms, embed_ms, db_write_ms, total_ms, chunk_count, char_count
        FROM ingest_jobs WHERE id = 'job-metrics'
        """,
    ).fetchone()
    assert row == (1, 2, 3, 4, 5, 15, 7, 100)
