# SPDX-License-Identifier: AGPL-3.0-only

from __future__ import annotations

from pathlib import Path

import pytest

from ragdoll_worker.config import WorkerConfig, sanitize_model_name
from ragdoll_worker.models import Embedder, _model_is_complete


def _model_dir(config: WorkerConfig) -> Path:
    return config.model_cache_dir / sanitize_model_name(config.embedding_model)


@pytest.mark.skipif(
    not _model_is_complete(
        Path(__file__).resolve().parents[1]
        / "data"
        / "models"
        / sanitize_model_name("BAAI/bge-m3")
    ),
    reason="ONNX embedding model files not present under ragdoll/data/models",
)
def test_embedder_produces_normalized_vectors(worker_config: WorkerConfig) -> None:
    model_dir = _model_dir(worker_config)
    if not _model_is_complete(model_dir):
        pytest.skip(f"embedding model missing at {model_dir}")

    embedder = Embedder(worker_config)
    vectors = list(embedder.embed(["hello ragdoll", "second sentence"]))

    assert len(vectors) == 2
    assert len(vectors[0]) == worker_config.embedding_dim
    assert len(vectors[1]) == worker_config.embedding_dim
    assert vectors[0] != vectors[1]
