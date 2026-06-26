# SPDX-License-Identifier: AGPL-3.0-only

from __future__ import annotations

from collections.abc import Iterable
from pathlib import Path
from typing import Any

from fastembed import TextEmbedding
from fastembed.common.model_description import ModelSource, PoolingType
from tokenizers import Tokenizer

from ragdoll_worker.config import WorkerConfig

_CUSTOM_MODEL_REGISTERED = False


def _ensure_custom_embedding_model(model_name: str, dim: int) -> None:
    global _CUSTOM_MODEL_REGISTERED
    if _CUSTOM_MODEL_REGISTERED:
        return
    try:
        TextEmbedding.add_custom_model(
            model=model_name,
            pooling=PoolingType.CLS,
            normalization=True,
            sources=ModelSource(hf=model_name),
            dim=dim,
            model_file="model.onnx",
            additional_files=["model.onnx_data"],
        )
    except ValueError:
        pass
    _CUSTOM_MODEL_REGISTERED = True


class Embedder:
    def __init__(self, config: WorkerConfig) -> None:
        model_dir = config.model_dir_for(config.embedding_model)
        if not _model_is_complete(model_dir):
            raise RuntimeError(
                f"embedding model missing at {model_dir}; run `ragdoll models-ensure` first"
            )

        _ensure_custom_embedding_model(config.embedding_model, config.embedding_dim)
        self.model = TextEmbedding(
            model_name=config.embedding_model,
            cache_dir=str(model_dir),
            specific_model_path=str(model_dir),
            local_files_only=True,
        )
        self.tokenizer = Tokenizer.from_file(str(model_dir / "tokenizer.json"))

    def embed(self, texts: list[str]) -> Iterable[Any]:
        return self.model.embed(texts)


def _model_is_complete(model_dir: Path) -> bool:
    required = [
        "model.onnx",
        "tokenizer.json",
        "config.json",
        "special_tokens_map.json",
        "tokenizer_config.json",
    ]
    return model_dir.exists() and all((model_dir / name).exists() for name in required)
