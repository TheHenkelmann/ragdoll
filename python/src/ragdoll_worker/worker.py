# SPDX-License-Identifier: AGPL-3.0-only

from __future__ import annotations

import json
import time
from datetime import datetime
from typing import Any

import structlog

from ragdoll_worker.chunk.semantic_split import semantic_split_chunk
from ragdoll_worker.config import WorkerConfig
from ragdoll_worker.db import WorkerDb
from ragdoll_worker.extract import extract_from_source
from ragdoll_worker.models import Embedder
from ragdoll_worker.reconcile import reconcile_jobs

log = structlog.get_logger()
_shutdown = False


def _handle_signal(signum: int, _frame: Any) -> None:
    global _shutdown
    log.info("shutdown_signal_received", signum=signum)
    _shutdown = True


def _sqlite_ms_delta(created_at: str | None, started_at: str | None) -> int:
    if not created_at or not started_at:
        return 0
    fmt = "%Y-%m-%d %H:%M:%S"
    try:
        created = datetime.strptime(created_at, fmt)
        started = datetime.strptime(started_at, fmt)
    except ValueError:
        return 0
    return max(0, int((started - created).total_seconds() * 1000))


def process_job(db: WorkerDb, config: WorkerConfig, embedder: Embedder, job: dict[str, Any]) -> None:
    started = time.perf_counter()
    queue_ms = _sqlite_ms_delta(job.get("created_at"), job.get("started_at"))
    source = db.fetch_source(job["source_id"])
    release_id = source["release_id"]
    settings = db.fetch_settings(release_id)

    extract_start = time.perf_counter()
    text = extract_from_source(source, config.staging_dir).strip()
    extract_ms = int((time.perf_counter() - extract_start) * 1000)
    if not text:
        raise RuntimeError("extracted text is empty")

    chunk_start = time.perf_counter()
    strategy = str(settings.get("chunking_strategy", "semantic_split"))
    if strategy != "semantic_split":
        raise RuntimeError(f"unsupported chunking_strategy: {strategy}")
    chunks = semantic_split_chunk(text, embedder.model, settings, embedder.tokenizer, source)
    chunk_ms = int((time.perf_counter() - chunk_start) * 1000)

    db_write_start = time.perf_counter()
    db.replace_chunks(
        source_id=source["id"],
        release_id=release_id,
        chunks=chunks,
        embedding_model=config.embedding_model,
        embedding_dim=config.embedding_dim,
        embedding_version="1",
    )
    db_write_ms = int((time.perf_counter() - db_write_start) * 1000)
    total_ms = int((time.perf_counter() - started) * 1000)

    db.update_job_metrics(
        job_id=str(job["id"]),
        metrics={
            "queue_ms": queue_ms,
            "extract_ms": extract_ms,
            "chunk_ms": chunk_ms,
            "embed_ms": 0,
            "db_write_ms": db_write_ms,
            "total_ms": total_ms,
            "chunk_count": len(chunks),
            "char_count": len(text),
        },
    )


def run_worker(config: WorkerConfig) -> None:
    import signal

    signal.signal(signal.SIGTERM, _handle_signal)
    signal.signal(signal.SIGINT, _handle_signal)

    config.ensure_directories()
    db = WorkerDb(config)
    embedder = Embedder(config)
    reconcile_jobs(db, config)
    db.release()

    log.info("worker_started", worker_id=config.worker_id)
    while not _shutdown:
        db.ensure_connected()
        try:
            job = db.claim_job(config.worker_id)
        except ValueError as exc:
            if "locked" not in str(exc).lower():
                raise
            db.release()
            time.sleep(config.worker_poll_interval_ms / 1000.0)
            continue
        if not job:
            db.release()
            time.sleep(config.worker_poll_interval_ms / 1000.0)
            continue

        job_id = str(job["id"])
        source_id = str(job["source_id"])
        log.info("job_claimed", job_id=job_id, source_id=source_id)
        try:
            db.heartbeat(job_id, config.worker_id)
            process_job(db, config, embedder, job)
            db.complete_job(job_id, source_id)
            log.info("job_completed", job_id=job_id, source_id=source_id)
        except Exception as exc:  # noqa: BLE001
            retry = int(job["attempts"]) < int(job["max_attempts"])
            db.fail_job(job_id, source_id, str(exc), retry=retry)
            log.exception("job_failed", job_id=job_id, source_id=source_id, retry=retry)
        finally:
            db.release()

    log.info("worker_stopped")
