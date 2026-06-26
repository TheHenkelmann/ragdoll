# SPDX-License-Identifier: AGPL-3.0-only

from __future__ import annotations

from pathlib import Path

import pytest

from ragdoll_worker.config import WorkerConfig
from ragdoll_worker.models import _model_is_complete


def _write_model_files(model_dir: Path) -> None:
    model_dir.mkdir(parents=True)
    for name in (
        "model.onnx",
        "tokenizer.json",
        "config.json",
        "special_tokens_map.json",
        "tokenizer_config.json",
    ):
        (model_dir / name).write_text("{}", encoding="utf-8")


def test_model_is_complete_false_when_directory_missing(tmp_path: Path) -> None:
    assert not _model_is_complete(tmp_path / "missing")


def test_model_is_complete_false_when_files_missing(tmp_path: Path) -> None:
    model_dir = tmp_path / "partial"
    model_dir.mkdir()
    (model_dir / "model.onnx").write_text("x", encoding="utf-8")
    assert not _model_is_complete(model_dir)


def test_model_is_complete_true_when_all_files_present(tmp_path: Path) -> None:
    model_dir = tmp_path / "complete"
    _write_model_files(model_dir)
    assert _model_is_complete(model_dir)


def test_embedder_raises_when_model_missing(worker_config: WorkerConfig) -> None:
    from ragdoll_worker.models import Embedder

    with pytest.raises(RuntimeError, match="embedding model missing"):
        Embedder(worker_config)
