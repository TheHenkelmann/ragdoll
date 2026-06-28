# SPDX-License-Identifier: AGPL-3.0-only

from __future__ import annotations

from pathlib import Path
from unittest.mock import MagicMock, patch

import pytest

from ragdoll_worker.config import WorkerConfig
from ragdoll_worker.models import _model_is_complete, _native_model_names


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


def test_native_model_names_is_nonempty_and_cached() -> None:
    names = _native_model_names()
    assert isinstance(names, frozenset)
    assert len(names) > 0
    # Cached: identical object on subsequent calls.
    assert _native_model_names() is names


def test_embedder_raises_when_model_missing(worker_config: WorkerConfig) -> None:
    from ragdoll_worker.models import Embedder

    with pytest.raises(RuntimeError, match="embedding model missing"):
        Embedder(worker_config)


def test_ensure_onnx_subdir_symlinks_root_model(tmp_path: Path) -> None:
    from ragdoll_worker.models import _ensure_onnx_subdir

    model_dir = tmp_path / "custom-model"
    model_dir.mkdir()
    (model_dir / "model.onnx").write_bytes(b"onnx")

    _ensure_onnx_subdir(model_dir)

    onnx_path = model_dir / "onnx" / "model.onnx"
    assert onnx_path.exists()
    assert onnx_path.is_symlink() or onnx_path.is_file()


def test_ensure_onnx_subdir_noop_when_already_present(tmp_path: Path) -> None:
    from ragdoll_worker.models import _ensure_onnx_subdir

    model_dir = tmp_path / "ready-model"
    onnx_dir = model_dir / "onnx"
    onnx_dir.mkdir(parents=True)
    existing = onnx_dir / "model.onnx"
    existing.write_bytes(b"existing")

    _ensure_onnx_subdir(model_dir)
    assert existing.read_bytes() == b"existing"


def test_ensure_custom_embedding_model_is_idempotent() -> None:
    from ragdoll_worker.models import _CUSTOM_MODEL_REGISTERED, _ensure_custom_embedding_model

    name = "org/test-custom-embed"
    _CUSTOM_MODEL_REGISTERED.discard(name)
    _ensure_custom_embedding_model(name, 1024)
    _ensure_custom_embedding_model(name, 1024)
    assert name in _CUSTOM_MODEL_REGISTERED


@patch("ragdoll_worker.models.TextEmbedding")
@patch("ragdoll_worker.models.Tokenizer")
@patch("ragdoll_worker.models._native_model_names")
def test_embedder_applies_e5_document_prefix(
    mock_native: MagicMock,
    mock_tokenizer: MagicMock,
    mock_text_embedding: MagicMock,
    worker_config: WorkerConfig,
    tmp_path: Path,
) -> None:
    from ragdoll_worker.models import Embedder

    model_dir = worker_config.model_dir_for("intfloat/multilingual-e5-large")
    _write_model_files(model_dir)
    mock_native.return_value = frozenset({"intfloat/multilingual-e5-large"})
    embed_instance = MagicMock()
    mock_text_embedding.return_value = embed_instance

    embedder = Embedder(worker_config, model_name="intfloat/multilingual-e5-large")
    list(embedder.embed(["hello world"]))

    embed_instance.embed.assert_called_once_with(["passage: hello world"])
