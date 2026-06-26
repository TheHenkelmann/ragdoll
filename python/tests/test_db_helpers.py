# SPDX-License-Identifier: AGPL-3.0-only

from __future__ import annotations

from ragdoll_worker.config import WorkerConfig
from ragdoll_worker.db import WorkerDb, _json_object


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


def test_fetch_source_returns_metadata(worker_db: WorkerDb) -> None:
    release_id = "00000000-0000-0000-0000-000000000001"
    worker_db.conn.execute(
        """
        INSERT OR REPLACE INTO sources (id, release_id, name, type, uri, config, metadata, status)
        VALUES ('src-1', ?, 'demo', 'text', NULL, '{"k":"v"}', '{"tag":"x"}', 'pending')
        """,
        (release_id,),
    )
    commit = getattr(worker_db.conn, "commit", None)
    if callable(commit):
        commit()

    source = worker_db.fetch_source("src-1")
    assert source["name"] == "demo"
    assert source["config"] == {"k": "v"}
    assert source["metadata"] == {"tag": "x"}


def test_claim_and_complete_job(worker_db: WorkerDb, worker_config: WorkerConfig) -> None:
    release_id = "00000000-0000-0000-0000-000000000001"
    worker_db.conn.execute(
        """
        INSERT OR REPLACE INTO sources (id, release_id, name, type, status)
        VALUES ('src-job', ?, 'demo', 'text', 'pending')
        """,
        (release_id,),
    )
    worker_db.conn.execute(
        """
        INSERT OR REPLACE INTO ingest_jobs (id, release_id, source_id, status, attempts, max_attempts)
        VALUES ('job-claim', ?, 'src-job', 'pending', 0, 3)
        """,
        (release_id,),
    )
    commit = getattr(worker_db.conn, "commit", None)
    if callable(commit):
        commit()

    job = worker_db.claim_job(worker_config.worker_id)
    assert job is not None
    assert job["source_id"] == "src-job"
    assert job["attempts"] == 1

    worker_db.complete_job(str(job["id"]), "src-job")
    status = worker_db.conn.execute(
        "SELECT status FROM ingest_jobs WHERE id = 'job-claim'",
    ).fetchone()[0]
    assert status == "completed"


def test_fail_job_retries_until_max_attempts(worker_db: WorkerDb) -> None:
    release_id = "00000000-0000-0000-0000-000000000001"
    worker_db.conn.execute(
        """
        INSERT OR REPLACE INTO sources (id, release_id, name, type, status)
        VALUES ('src-fail', ?, 'demo', 'text', 'processing')
        """,
        (release_id,),
    )
    worker_db.conn.execute(
        """
        INSERT OR REPLACE INTO ingest_jobs (id, release_id, source_id, status, attempts, max_attempts)
        VALUES ('job-fail', ?, 'src-fail', 'processing', 2, 3)
        """,
        (release_id,),
    )
    commit = getattr(worker_db.conn, "commit", None)
    if callable(commit):
        commit()

    worker_db.fail_job("job-fail", "src-fail", "boom", retry=True)
    row = worker_db.conn.execute(
        "SELECT status, error FROM ingest_jobs WHERE id = 'job-fail'",
    ).fetchone()
    assert row[0] == "pending"
    assert row[1] == "boom"

    worker_db.fail_job("job-fail", "src-fail", "final", retry=False)
    row = worker_db.conn.execute(
        "SELECT status FROM ingest_jobs WHERE id = 'job-fail'",
    ).fetchone()
    assert row[0] == "failed"


def test_replace_chunks_replaces_existing_rows(worker_db: WorkerDb) -> None:
    release_id = "00000000-0000-0000-0000-000000000001"
    source_id = "src-chunks"
    worker_db.conn.execute(
        """
        INSERT OR REPLACE INTO sources (id, release_id, name, type, status)
        VALUES (?, ?, 'demo', 'text', 'completed')
        """,
        (source_id, release_id),
    )
    commit = getattr(worker_db.conn, "commit", None)
    if callable(commit):
        commit()

    worker_db.replace_chunks(
        source_id=source_id,
        release_id=release_id,
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
    )

    count = worker_db.conn.execute(
        "SELECT COUNT(*) FROM chunks WHERE source_id = ?",
        (source_id,),
    ).fetchone()[0]
    assert count == 2


def test_heartbeat_updates_timestamp(worker_db: WorkerDb, worker_config: WorkerConfig) -> None:
    release_id = "00000000-0000-0000-0000-000000000001"
    worker_db.conn.execute(
        """
        INSERT OR REPLACE INTO sources (id, release_id, name, type, status)
        VALUES ('src-heartbeat', ?, 'demo', 'text', 'processing')
        """,
        (release_id,),
    )
    worker_db.conn.execute(
        """
        INSERT OR REPLACE INTO ingest_jobs (
            id, release_id, source_id, status, worker_id, attempts, max_attempts,
            heartbeat_at
        ) VALUES (
            'job-heartbeat', ?, 'src-heartbeat', 'processing', ?, 1, 3, '2000-01-01 00:00:00'
        )
        """,
        (release_id, worker_config.worker_id),
    )
    commit = getattr(worker_db.conn, "commit", None)
    if callable(commit):
        commit()

    worker_db.heartbeat("job-heartbeat", worker_config.worker_id)
    heartbeat_at = worker_db.conn.execute(
        "SELECT heartbeat_at FROM ingest_jobs WHERE id = 'job-heartbeat'",
    ).fetchone()[0]
    assert heartbeat_at != "2000-01-01 00:00:00"


def test_update_job_metrics_persists_values(worker_db: WorkerDb) -> None:
    release_id = "00000000-0000-0000-0000-000000000001"
    worker_db.conn.execute(
        """
        INSERT OR REPLACE INTO sources (id, release_id, name, type, status)
        VALUES ('src-metrics', ?, 'demo', 'text', 'processing')
        """,
        (release_id,),
    )
    worker_db.conn.execute(
        """
        INSERT OR REPLACE INTO ingest_jobs (id, release_id, source_id, status, attempts, max_attempts)
        VALUES ('job-metrics', ?, 'src-metrics', 'processing', 1, 3)
        """,
        (release_id,),
    )
    commit = getattr(worker_db.conn, "commit", None)
    if callable(commit):
        commit()

    worker_db.update_job_metrics(
        "job-metrics",
        {
            "queue_ms": 10,
            "extract_ms": 20,
            "chunk_ms": 30,
            "embed_ms": 0,
            "db_write_ms": 40,
            "total_ms": 100,
            "chunk_count": 2,
            "char_count": 500,
        },
    )

    row = worker_db.conn.execute(
        """
        SELECT queue_ms, extract_ms, chunk_ms, embed_ms, db_write_ms, total_ms,
               chunk_count, char_count
        FROM ingest_jobs WHERE id = 'job-metrics'
        """,
    ).fetchone()
    assert row == (10, 20, 30, 0, 40, 100, 2, 500)
