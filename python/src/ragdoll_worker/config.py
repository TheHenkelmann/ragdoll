# SPDX-License-Identifier: AGPL-3.0-only

from __future__ import annotations

import os
from dataclasses import dataclass
from pathlib import Path


def _env_path(key: str) -> Path | None:
    value = os.getenv(key, "").strip()
    return Path(value) if value else None


def sanitize_model_name(model_name: str) -> str:
    return model_name.replace("/", "__")


@dataclass(frozen=True)
class WorkerConfig:
    data_dir: Path
    db_path: Path
    model_cache_dir: Path
    staging_dir: Path
    embedding_model: str
    embedding_dim: int
    worker_poll_interval_ms: int
    job_lease_seconds: int
    max_attempts: int
    worker_id: str

    @classmethod
    def from_env(cls) -> WorkerConfig:
        data_dir = Path(os.environ["RAGDOLL_DATA_DIR"])
        db_path = _env_path("RAGDOLL_DB_PATH") or data_dir / "db" / "ragdoll.db"
        model_cache_dir = _env_path("RAGDOLL_MODEL_CACHE_DIR") or data_dir / "models"
        staging_dir = _env_path("RAGDOLL_STAGING_DIR") or data_dir / "staging"
        worker_id = os.getenv("RAGDOLL_WORKER_ID", os.getenv("HOSTNAME", "worker-1"))
        return cls(
            data_dir=data_dir,
            db_path=db_path,
            model_cache_dir=model_cache_dir,
            staging_dir=staging_dir,
            embedding_model=os.getenv("RAGDOLL_EMBEDDING_MODEL", "BAAI/bge-m3"),
            embedding_dim=int(os.getenv("RAGDOLL_EMBEDDING_DIM", "1024")),
            worker_poll_interval_ms=int(os.getenv("RAGDOLL_WORKER_POLL_INTERVAL_MS", "1000")),
            job_lease_seconds=int(os.getenv("RAGDOLL_JOB_LEASE_SECONDS", "300")),
            max_attempts=int(os.getenv("RAGDOLL_MAX_ATTEMPTS", "3")),
            worker_id=worker_id,
        )

    def model_dir_for(self, model_name: str) -> Path:
        return self.model_cache_dir / sanitize_model_name(model_name)

    def ensure_directories(self) -> None:
        for path in (self.data_dir, self.db_path.parent, self.model_cache_dir, self.staging_dir):
            path.mkdir(parents=True, exist_ok=True)
