# SPDX-License-Identifier: AGPL-3.0-only

from __future__ import annotations

import os
from collections.abc import Iterable
from functools import lru_cache
from pathlib import Path
from typing import Any

from fastembed import TextEmbedding
from fastembed.common.model_description import ModelSource, PoolingType
from tokenizers import Tokenizer

from ragdoll_worker.config import WorkerConfig

# Mirrors ragdoll/src/models/catalog.rs (embedding presets with fastembed loaders).
SUPPORTED_EMBED_PRESETS = frozenset(
    {
        "BAAI/bge-m3",
        "BAAI/bge-large-en-v1.5",
        "mixedbread-ai/mxbai-embed-large-v1",
        "intfloat/multilingual-e5-large",
        "Alibaba-NLP/gte-large-en-v1.5",
    }
)

# All whitelisted embedding models (presets + user-defined ONNX on disk).
SUPPORTED_EMBED_MODELS = frozenset(
    {
        "BAAI/bge-m3",
        "BAAI/bge-large-en-v1.5",
        "mixedbread-ai/mxbai-embed-large-v1",
        "intfloat/multilingual-e5-large",
        "Snowflake/snowflake-arctic-embed-l-v2.0",
        "mixedbread-ai/deepset-mxbai-embed-de-large-v1",
        "jinaai/jina-embeddings-v3",
        "intfloat/multilingual-e5-large-instruct",
        "Alibaba-NLP/gte-large-en-v1.5",
    }
)

_DOC_PREFIXES = {
    "intfloat/multilingual-e5-large": "passage: ",
    "intfloat/multilingual-e5-large-instruct": "passage: ",
}

_QUERY_PREFIXES = {
    "intfloat/multilingual-e5-large": "query: ",
    "intfloat/multilingual-e5-large-instruct": "query: ",
}

_CUSTOM_MODEL_REGISTERED: set[str] = set()


@lru_cache(maxsize=1)
def _native_model_names() -> frozenset[str]:
    """Embedding model ids the installed fastembed build can load by name.

    fastembed's catalog varies by version (e.g. 0.8.0 ships mxbai but not
    bge-m3), so probe at runtime instead of trusting the static preset list.
    Presets missing here fall back to the custom on-disk ONNX loader.
    """
    names: set[str] = set()
    for desc in TextEmbedding.list_supported_models():
        name = desc["model"] if isinstance(desc, dict) else getattr(desc, "model", None)
        if name:
            names.add(name)
    return frozenset(names)


def _ensure_custom_embedding_model(model_name: str, dim: int) -> None:
    if model_name in _CUSTOM_MODEL_REGISTERED:
        return
    try:
        TextEmbedding.add_custom_model(
            model=model_name,
            pooling=PoolingType.CLS,
            normalization=True,
            sources=ModelSource(hf=model_name),
            dim=dim,
            model_file="model.onnx",
            additional_files=["model.onnx_data", "model.onnx.data"],
        )
    except ValueError:
        pass
    _CUSTOM_MODEL_REGISTERED.add(model_name)


def _ensure_onnx_subdir(model_dir: Path) -> None:
    """fastembed custom models resolve onnx under specific_model_path/onnx/."""
    onnx_file = model_dir / "onnx" / "model.onnx"
    if onnx_file.exists():
        return
    root_onnx = model_dir / "model.onnx"
    if not root_onnx.exists():
        return
    onnx_file.parent.mkdir(parents=True, exist_ok=True)
    if not onnx_file.exists():
        try:
            onnx_file.symlink_to(root_onnx.resolve())
        except OSError:
            if not onnx_file.exists():
                import shutil

                shutil.copy2(root_onnx, onnx_file)


class Embedder:
    def __init__(self, config: WorkerConfig, model_name: str | None = None) -> None:
        self.embedding_model = model_name or config.embedding_model
        model_dir = config.model_dir_for(self.embedding_model)
        if not _model_is_complete(model_dir):
            raise RuntimeError(
                f"embedding model missing at {model_dir}; run `ragdoll models-ensure` first"
            )

        if self.embedding_model in _native_model_names():
            # Natively in this fastembed build's catalog: load from the HF cache
            # layout under model_cache_dir, matching the Rust gateway.
            self.model = TextEmbedding(
                model_name=self.embedding_model,
                cache_dir=str(config.model_cache_dir),
                local_files_only=True,
            )
        else:
            _ensure_custom_embedding_model(self.embedding_model, config.embedding_dim)
            _ensure_onnx_subdir(model_dir)
            self.model = TextEmbedding(
                model_name=self.embedding_model,
                cache_dir=str(model_dir),
                specific_model_path=str(model_dir),
                local_files_only=True,
            )
        self.tokenizer = Tokenizer.from_file(str(model_dir / "tokenizer.json"))

    def embed(self, texts: list[str]) -> Iterable[Any]:
        prefix = _DOC_PREFIXES.get(self.embedding_model, "")
        if prefix:
            texts = [f"{prefix}{t}" for t in texts]
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
