# SPDX-License-Identifier: AGPL-3.0-only

from __future__ import annotations

from pathlib import Path

import pytest

from ragdoll_worker.config import WorkerConfig, sanitize_model_name


def test_sanitize_model_name_replaces_slashes() -> None:
    assert sanitize_model_name("org/model-name") == "org__model-name"


def test_model_dir_for_uses_sanitized_name(worker_config: WorkerConfig) -> None:
    path = worker_config.model_dir_for("BAAI/bge-m3")
    assert path.name == "BAAI__bge-m3"
    assert path.parent == worker_config.model_cache_dir


def test_ensure_directories_creates_expected_paths(worker_config: WorkerConfig) -> None:
    worker_config.ensure_directories()
    assert worker_config.data_dir.is_dir()
    assert worker_config.db_path.parent.is_dir()
    assert worker_config.model_cache_dir.is_dir()
    assert worker_config.staging_dir.is_dir()


def test_from_env_uses_defaults(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    monkeypatch.setenv("RAGDOLL_DATA_DIR", str(tmp_path))
    monkeypatch.delenv("RAGDOLL_DB_PATH", raising=False)
    monkeypatch.delenv("RAGDOLL_MODEL_CACHE_DIR", raising=False)
    monkeypatch.delenv("RAGDOLL_STAGING_DIR", raising=False)
    monkeypatch.setenv("RAGDOLL_WORKER_ID", "worker-test")

    config = WorkerConfig.from_env()
    assert config.data_dir == tmp_path
    assert config.db_path == tmp_path / "db" / "ragdoll.db"
    assert config.model_cache_dir == tmp_path / "models"
    assert config.staging_dir == tmp_path / "staging"
    assert config.worker_id == "worker-test"
    assert config.embedding_model == "BAAI/bge-m3"
    assert config.embedding_dim == 1024


def test_from_env_honors_overrides(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    custom_db = tmp_path / "custom.db"
    monkeypatch.setenv("RAGDOLL_DATA_DIR", str(tmp_path))
    monkeypatch.setenv("RAGDOLL_DB_PATH", str(custom_db))
    monkeypatch.setenv("RAGDOLL_EMBEDDING_DIM", "768")
    monkeypatch.setenv("RAGDOLL_MAX_ATTEMPTS", "5")

    config = WorkerConfig.from_env()
    assert config.db_path == custom_db
    assert config.embedding_dim == 768
    assert config.max_attempts == 5


def test_from_env_requires_data_dir(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.delenv("RAGDOLL_DATA_DIR", raising=False)
    with pytest.raises(KeyError):
        WorkerConfig.from_env()
