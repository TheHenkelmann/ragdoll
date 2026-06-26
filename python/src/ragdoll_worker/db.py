# SPDX-License-Identifier: AGPL-3.0-only

from __future__ import annotations

import json
from typing import Any

import libsql_experimental as libsql

from ragdoll_worker.config import WorkerConfig


def _json_object(raw: Any) -> dict[str, Any]:
    if raw is None or raw == "null":
        return {}
    value = json.loads(raw) if isinstance(raw, str) else raw
    return value if isinstance(value, dict) else {}


class WorkerDb:
    def __init__(self, config: WorkerConfig) -> None:
        self.config = config
        self.conn: libsql.Connection | None = None
        self.ensure_connected()

    def ensure_connected(self) -> None:
        if self.conn is None:
            self.conn = libsql.connect(str(self.config.db_path))
            self._apply_pragmas()

    def release(self) -> None:
        if self.conn is None:
            return
        close = getattr(self.conn, "close", None)
        if callable(close):
            close()
        self.conn = None

    def _commit(self) -> None:
        commit = getattr(self.conn, "commit", None)
        if callable(commit):
            commit()

    def _apply_pragmas(self) -> None:
        assert self.conn is not None
        self.conn.execute("PRAGMA journal_mode = WAL")
        self.conn.execute("PRAGMA busy_timeout = 30000")
        self.conn.execute("PRAGMA synchronous = NORMAL")
        self.conn.execute("PRAGMA foreign_keys = ON")

    def fetch_settings(self, release_id: str) -> dict[str, Any]:
        rows = self.conn.execute(
            "SELECT key, value FROM settings WHERE release_id = ?",
            (release_id,),
        ).fetchall()
        settings: dict[str, Any] = {}
        for key, raw in rows:
            try:
                settings[key] = json.loads(raw)
            except json.JSONDecodeError:
                settings[key] = raw
        return settings

    def claim_job(self, worker_id: str) -> dict[str, Any] | None:
        row = self.conn.execute(
            """
            UPDATE ingest_jobs
            SET status = 'processing',
                worker_id = ?,
                started_at = datetime('now'),
                heartbeat_at = datetime('now'),
                attempts = attempts + 1
            WHERE id = (
                SELECT id FROM ingest_jobs
                WHERE status = 'pending'
                ORDER BY created_at ASC
                LIMIT 1
            )
            RETURNING id, source_id, attempts, max_attempts, created_at, started_at
            """,
            (worker_id,),
        ).fetchone()
        if not row:
            return None
        self._commit()
        return {
            "id": row[0],
            "source_id": row[1],
            "attempts": row[2],
            "max_attempts": row[3],
            "created_at": row[4],
            "started_at": row[5],
        }

    def heartbeat(self, job_id: str, worker_id: str) -> None:
        self.conn.execute(
            """
            UPDATE ingest_jobs
            SET heartbeat_at = datetime('now')
            WHERE id = ? AND worker_id = ?
            """,
            (job_id, worker_id),
        )
        self._commit()

    def update_job_metrics(self, job_id: str, metrics: dict[str, int]) -> None:
        self.conn.execute(
            """
            UPDATE ingest_jobs
            SET queue_ms = ?, extract_ms = ?, chunk_ms = ?, embed_ms = ?, db_write_ms = ?, total_ms = ?,
                chunk_count = ?, char_count = ?
            WHERE id = ?
            """,
            (
                metrics.get("queue_ms"),
                metrics.get("extract_ms"),
                metrics.get("chunk_ms"),
                metrics.get("embed_ms"),
                metrics.get("db_write_ms"),
                metrics.get("total_ms"),
                metrics.get("chunk_count"),
                metrics.get("char_count"),
                job_id,
            ),
        )
        self._commit()

    def complete_job(self, job_id: str, source_id: str) -> None:
        self.conn.execute(
            """
            UPDATE ingest_jobs
            SET status = 'completed', finished_at = datetime('now'), error = NULL
            WHERE id = ?
            """,
            (job_id,),
        )
        self.conn.execute(
            """
            UPDATE sources
            SET status = 'completed', updated_at = datetime('now'), error = NULL
            WHERE id = ?
            """,
            (source_id,),
        )
        self._commit()

    def fail_job(self, job_id: str, source_id: str, error: str, retry: bool) -> None:
        status = "pending" if retry else "failed"
        self.conn.execute(
            """
            UPDATE ingest_jobs
            SET status = ?, finished_at = datetime('now'), error = ?
            WHERE id = ?
            """,
            (status, error, job_id),
        )
        self.conn.execute(
            """
            UPDATE sources
            SET status = ?, updated_at = datetime('now'), error = ?
            WHERE id = ?
            """,
            (status if status == "failed" else "processing", error, source_id),
        )
        self._commit()

    def fetch_source(self, source_id: str) -> dict[str, Any]:
        row = self.conn.execute(
            """
            SELECT id, release_id, name, type, uri, config, metadata
            FROM sources WHERE id = ?
            """,
            (source_id,),
        ).fetchone()
        if not row:
            raise RuntimeError(f"source not found: {source_id}")
        return {
            "id": row[0],
            "release_id": row[1],
            "name": row[2],
            "type": row[3],
            "uri": row[4],
            "config": _json_object(row[5]),
            "metadata": _json_object(row[6]),
        }

    def replace_chunks(
        self,
        source_id: str,
        release_id: str,
        chunks: list[dict[str, Any]],
        embedding_model: str,
        embedding_dim: int,
        embedding_version: str,
    ) -> None:
        self.conn.execute("DELETE FROM chunks WHERE source_id = ?", (source_id,))
        self._commit()
        batch_size = 50
        for start in range(0, len(chunks), batch_size):
            batch = chunks[start : start + batch_size]
            for ordinal, chunk in enumerate(batch, start=start):
                self.conn.execute(
                    """
                    INSERT INTO chunks (
                        id, release_id, source_id, ordinal, content, provenance, metadata,
                        token_count, embedding, embedding_model, embedding_dim, embedding_version
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, vector32(?), ?, ?, ?)
                    """,
                    (
                        chunk["id"],
                        release_id,
                        source_id,
                        ordinal,
                        chunk["content"],
                        json.dumps(chunk["provenance"]),
                        json.dumps(chunk["metadata"]),
                        chunk.get("token_count"),
                        json.dumps(chunk["embedding"]),
                        embedding_model,
                        embedding_dim,
                        embedding_version,
                    ),
                )
            self._commit()
