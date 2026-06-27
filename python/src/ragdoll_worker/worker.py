# SPDX-License-Identifier: AGPL-3.0-only

from __future__ import annotations

import hashlib
import json
import time
from datetime import datetime
from pathlib import Path
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
_embedder_cache: dict[str, Embedder] = {}
_last_embedder_evict = 0.0
EMBEDDER_EVICT_INTERVAL_SEC = 60.0


def _embedder_for(config: WorkerConfig, model_name: str) -> Embedder:
    if model_name not in _embedder_cache:
        _embedder_cache[model_name] = Embedder(config, model_name)
    return _embedder_cache[model_name]


def _evict_unused_embedders(required: set[str]) -> int:
    evicted = [name for name in list(_embedder_cache) if name not in required]
    for name in evicted:
        del _embedder_cache[name]
    if evicted:
        log.info("evicted_embedders_from_memory", models=evicted)
    return len(evicted)


def purge_embedder(model_name: str) -> bool:
    """Drop one embedder from the worker in-memory cache."""
    if model_name in _embedder_cache:
        del _embedder_cache[model_name]
        log.info("purged_embedder_from_memory", model=model_name)
        return True
    return False


def purge_unreferenced_embedders(db: WorkerDb) -> int:
    """Drop embedders not referenced by any release settings."""
    try:
        required = db.fetch_required_embedding_models()
        return _evict_unused_embedders(required)
    except Exception as exc:  # noqa: BLE001
        log.warning("embedder_purge_skipped", error=str(exc))
        return 0


def _maybe_evict_embedders(db: WorkerDb) -> None:
    global _last_embedder_evict
    now = time.time()
    if now - _last_embedder_evict < EMBEDDER_EVICT_INTERVAL_SEC:
        return
    _last_embedder_evict = now
    purge_unreferenced_embedders(db)


class DedupSkipped(Exception):
    """Ingest skipped because duplicate content exists (dedup_policy=skip)."""


def _handle_signal(signum: int, _frame: Any) -> None:
    global _shutdown
    log.info("shutdown_signal_received", signum=signum)
    _shutdown = True


def _cleanup_staging_artifacts(source: dict[str, Any], staging_dir: Path) -> None:
    source_type = str(source["type"])
    source_id = str(source["id"])
    if source_type == "text":
        path = staging_dir / f"{source_id}.txt"
        if path.exists():
            path.unlink()
        return
    if source_type == "file":
        uri = source.get("uri")
        if uri:
            path = Path(str(uri))
            if path.exists():
                path.unlink()


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


def _parse_page_map(raw: Any) -> list[dict[str, int]]:
    if raw is None or raw == "null":
        return []
    if isinstance(raw, str):
        try:
            parsed = json.loads(raw)
        except json.JSONDecodeError:
            return []
    else:
        parsed = raw
    return parsed if isinstance(parsed, list) else []


def process_job(db: WorkerDb, config: WorkerConfig, job: dict[str, Any]) -> None:
    started = time.perf_counter()
    queue_ms = _sqlite_ms_delta(job.get("created_at"), job.get("started_at"))
    source = db.source_from_job(job)
    release_id = str(job["release_id"])
    source_id = str(source["id"])
    settings = db.fetch_settings(release_id)
    embedding_model = str(settings.get("embedding_model", config.embedding_model))
    embedder = _embedder_for(config, embedding_model)

    extract_start = time.perf_counter()
    stored_text = db.fetch_source_text(source_id)
    if stored_text is not None:
        text = stored_text.strip()
        extract_ms = 0
    else:
        text = extract_from_source(source, config.staging_dir).strip()
        extract_ms = int((time.perf_counter() - extract_start) * 1000)
    if not text:
        raise RuntimeError("extracted text is empty")

    page_map: list[dict[str, int]] = []
    if str(source["type"]) == "file" and source.get("uri"):
        from ragdoll_worker.extract.files import build_pdf_page_map

        file_path = Path(str(source["uri"]))
        if file_path.suffix.lower() == ".pdf" and file_path.exists():
            _extracted, page_map = build_pdf_page_map(file_path)
    else:
        existing = db.try_fetch_source(source_id)
        if existing is not None:
            page_map = _parse_page_map(existing.get("page_map"))

    content_hash = hashlib.sha256(text.encode("utf-8")).hexdigest()
    dedup_policy = str(settings.get("dedup_policy", "replace"))
    duplicate = db.find_duplicate_source(release_id, content_hash, source_id)
    if duplicate:
        if dedup_policy == "reject":
            raise RuntimeError("duplicate content rejected by dedup_policy")
        if dedup_policy == "skip":
            raise DedupSkipped(duplicate)

    chunk_start = time.perf_counter()
    strategy = str(settings.get("chunking_strategy", "semantic_split"))
    if strategy != "semantic_split":
        raise RuntimeError(f"unsupported chunking_strategy: {strategy}")
    chunks = semantic_split_chunk(text, embedder.model, settings, embedder.tokenizer, source)
    chunk_ms = int((time.perf_counter() - chunk_start) * 1000)
    if not chunks:
        raise RuntimeError("chunking produced no chunks")

    db_write_start = time.perf_counter()
    db.commit_ingested_source(
        source_id=source_id,
        release_id=release_id,
        name=str(source["name"]),
        source_type=str(source["type"]),
        uri=str(source["uri"]) if source.get("uri") else None,
        content_hash=content_hash,
        config=source["config"],
        metadata=source["metadata"],
        page_map=page_map,
        text=text,
        chunks=chunks,
        embedding_model=embedding_model,
        embedding_dim=config.embedding_dim,
        embedding_version="1",
        dedup_policy=dedup_policy,
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


def _dispatch_webhooks(
    db: WorkerDb,
    job: dict[str, Any],
    source_id: str,
    status: str,
    error: str | None,
) -> None:
    from ragdoll_worker.webhooks import dispatch_ingest_webhooks

    chunk_count = db.connection.execute(
        "SELECT COUNT(*) FROM chunks WHERE source_id = ?",
        (source_id,),
    ).fetchone()[0]
    dispatch_ingest_webhooks(
        db.connection,
        release_id=str(job["release_id"]),
        stage_id=str(job["stage_id"]) if job.get("stage_id") else None,
        source_id=source_id,
        status=status,
        chunk_count=int(chunk_count),
        error=error,
    )


def run_worker(config: WorkerConfig) -> None:
    import signal

    signal.signal(signal.SIGTERM, _handle_signal)
    signal.signal(signal.SIGINT, _handle_signal)

    config.ensure_directories()
    db = WorkerDb(config)
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
            _maybe_evict_embedders(db)
            time.sleep(config.worker_poll_interval_ms / 1000.0)
            continue

        job_id = str(job["id"])
        source_id = str(job["source_id"])
        log.info("job_claimed", job_id=job_id, source_id=source_id)
        try:
            db.heartbeat(job_id, config.worker_id)
            process_job(db, config, job)
            db.complete_job(job_id)
            source = db.source_from_job(job)
            _cleanup_staging_artifacts(source, config.staging_dir)
            _dispatch_webhooks(db, job, source_id, "completed", None)
            log.info("job_completed", job_id=job_id, source_id=source_id)
        except DedupSkipped as dedup:
            db.complete_job(job_id)
            source = db.source_from_job(job)
            _cleanup_staging_artifacts(source, config.staging_dir)
            log.info(
                "job_dedup_skipped",
                job_id=job_id,
                source_id=source_id,
                dedupe_of=str(dedup),
            )
        except Exception as exc:  # noqa: BLE001
            retry = int(job["attempts"]) < int(job["max_attempts"])
            db.fail_job(job_id, str(exc), retry=retry)
            if not retry:
                source = db.source_from_job(job)
                _cleanup_staging_artifacts(source, config.staging_dir)
                _dispatch_webhooks(db, job, source_id, "failed", str(exc))
            log.exception("job_failed", job_id=job_id, source_id=source_id, retry=retry)
        finally:
            db.release()

    log.info("worker_stopped")
