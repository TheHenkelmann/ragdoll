# SPDX-License-Identifier: AGPL-3.0-only

from __future__ import annotations

import json
from collections.abc import Iterator
from contextlib import contextmanager
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

    @contextmanager
    def transaction(self) -> Iterator[None]:
        self.connection.execute("BEGIN IMMEDIATE")
        try:
            yield
            self._commit()
        except Exception:
            self.connection.execute("ROLLBACK")
            raise

    def _apply_pragmas(self) -> None:
        conn = self.connection
        conn.execute("PRAGMA journal_mode = WAL")
        conn.execute("PRAGMA busy_timeout = 30000")
        conn.execute("PRAGMA synchronous = NORMAL")
        conn.execute("PRAGMA foreign_keys = ON")

    @property
    def connection(self) -> libsql.Connection:
        if self.conn is None:
            self.ensure_connected()
        assert self.conn is not None
        return self.conn

    def fetch_settings(self, release_id: str) -> dict[str, Any]:
        rows = self.connection.execute(
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

    def fetch_required_embedding_models(self) -> set[str]:
        rows = self.connection.execute(
            "SELECT value FROM settings WHERE key = 'embedding_model'",
        ).fetchall()
        names: set[str] = set()
        for (raw,) in rows:
            try:
                names.add(str(json.loads(raw)))
            except json.JSONDecodeError:
                names.add(str(raw))
        if not names:
            names.add(self.config.embedding_model)
        return names

    def claim_job(self, worker_id: str) -> dict[str, Any] | None:
        row = self.connection.execute(
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
            RETURNING id, release_id, stage_id, source_id, source_name, source_type,
                      source_uri, content_hash, config, metadata,
                      attempts, max_attempts, created_at, started_at
            """,
            (worker_id,),
        ).fetchone()
        if not row:
            return None
        self._commit()
        return {
            "id": row[0],
            "release_id": row[1],
            "stage_id": row[2],
            "source_id": row[3],
            "source_name": row[4],
            "source_type": row[5],
            "source_uri": row[6],
            "content_hash": row[7],
            "config": row[8],
            "metadata": row[9],
            "attempts": row[10],
            "max_attempts": row[11],
            "created_at": row[12],
            "started_at": row[13],
        }

    def heartbeat(self, job_id: str, worker_id: str) -> None:
        self.connection.execute(
            """
            UPDATE ingest_jobs
            SET heartbeat_at = datetime('now')
            WHERE id = ? AND worker_id = ?
            """,
            (job_id, worker_id),
        )
        self._commit()

    def update_job_metrics(self, job_id: str, metrics: dict[str, int]) -> None:
        self.connection.execute(
            """
            UPDATE ingest_jobs
            SET queue_ms = ?, extract_ms = ?, chunk_ms = ?, embed_ms = ?,
                db_write_ms = ?, total_ms = ?, chunk_count = ?, char_count = ?
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

    def complete_job(self, job_id: str) -> None:
        self.connection.execute(
            """
            UPDATE ingest_jobs
            SET status = 'completed', finished_at = datetime('now'), error = NULL
            WHERE id = ?
            """,
            (job_id,),
        )
        self._commit()

    def fail_job(self, job_id: str, error: str, retry: bool) -> None:
        status = "pending" if retry else "failed"
        self.connection.execute(
            """
            UPDATE ingest_jobs
            SET status = ?, finished_at = datetime('now'), error = ?
            WHERE id = ?
            """,
            (status, error, job_id),
        )
        self._commit()

    def find_duplicate_source(
        self, release_id: str, content_hash: str, exclude_source_id: str
    ) -> str | None:
        row = self.connection.execute(
            """
            SELECT id FROM sources
            WHERE release_id = ? AND content_hash = ? AND id != ?
            LIMIT 1
            """,
            (release_id, content_hash, exclude_source_id),
        ).fetchone()
        if not row:
            return None
        return str(row[0])

    def delete_sources_by_hash_except(
        self, release_id: str, content_hash: str, keep_source_id: str
    ) -> None:
        self.connection.execute(
            """
            DELETE FROM sources
            WHERE release_id = ? AND content_hash = ? AND id != ?
            """,
            (release_id, content_hash, keep_source_id),
        )

    def fetch_source_text(self, source_id: str) -> str | None:
        row = self.connection.execute(
            "SELECT text FROM source_texts WHERE source_id = ?",
            (source_id,),
        ).fetchone()
        if not row:
            return None
        return str(row[0])

    def try_fetch_source(self, source_id: str) -> dict[str, Any] | None:
        row = self.connection.execute(
            """
            SELECT id, release_id, name, type, uri, config, metadata, page_map
            FROM sources WHERE id = ?
            """,
            (source_id,),
        ).fetchone()
        if not row:
            return None
        return {
            "id": row[0],
            "release_id": row[1],
            "name": row[2],
            "type": row[3],
            "uri": row[4],
            "config": _json_object(row[5]),
            "metadata": _json_object(row[6]),
            "page_map": row[7],
        }

    def source_from_job(self, job: dict[str, Any]) -> dict[str, Any]:
        return {
            "id": job["source_id"],
            "release_id": job["release_id"],
            "name": job["source_name"],
            "type": job["source_type"],
            "uri": job.get("source_uri"),
            "config": _json_object(job.get("config")),
            "metadata": _json_object(job.get("metadata")),
        }

    def commit_ingested_source(
        self,
        *,
        source_id: str,
        release_id: str,
        name: str,
        source_type: str,
        uri: str | None,
        content_hash: str,
        config: dict[str, Any],
        metadata: dict[str, Any],
        page_map: list[dict[str, int]],
        text: str,
        chunks: list[dict[str, Any]],
        embedding_model: str,
        embedding_dim: int,
        embedding_version: str,
        dedup_policy: str,
    ) -> None:
        config_json = json.dumps(config)
        metadata_json = json.dumps(metadata)
        page_map_json = json.dumps(page_map)

        with self.transaction():
            if dedup_policy == "replace":
                self.delete_sources_by_hash_except(release_id, content_hash, source_id)

            self.connection.execute(
                """
                INSERT INTO sources (
                    id, release_id, name, type, uri, content_hash, config, metadata,
                    page_map, status, error, created_at, updated_at
                ) VALUES (
                    ?, ?, ?, ?, ?, ?, ?, ?, ?, 'completed', NULL,
                    datetime('now'), datetime('now')
                )
                ON CONFLICT(id) DO UPDATE SET
                    name = excluded.name,
                    type = excluded.type,
                    uri = excluded.uri,
                    content_hash = excluded.content_hash,
                    config = excluded.config,
                    metadata = excluded.metadata,
                    page_map = excluded.page_map,
                    status = 'completed',
                    error = NULL,
                    updated_at = datetime('now')
                """,
                (
                    source_id,
                    release_id,
                    name,
                    source_type,
                    uri,
                    content_hash,
                    config_json,
                    metadata_json,
                    page_map_json,
                ),
            )
            self.connection.execute(
                """
                INSERT INTO source_texts (source_id, text, char_len)
                VALUES (?, ?, ?)
                ON CONFLICT(source_id) DO UPDATE SET
                    text = excluded.text,
                    char_len = excluded.char_len
                """,
                (source_id, text, len(text)),
            )
            self.connection.execute("DELETE FROM chunks WHERE source_id = ?", (source_id,))
            for ordinal, chunk in enumerate(chunks):
                self.connection.execute(
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
