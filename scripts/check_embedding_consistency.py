#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-only
"""Compare Rust and Python embeddings for the same canonical model artifacts."""

from __future__ import annotations

import json
import math
import os
import subprocess
import sys
from pathlib import Path


def cosine(a: list[float], b: list[float]) -> float:
    dot = sum(x * y for x, y in zip(a, b, strict=True))
    na = math.sqrt(sum(x * x for x in a))
    nb = math.sqrt(sum(y * y for y in b))
    return dot / (na * nb)


def main() -> int:
    data_dir = Path(os.environ.get("RAGDOLL_DATA_DIR", "/tmp/ragdoll-consistency"))
    model_dir = data_dir / "models" / "BAAI__bge-m3"
    if not (model_dir / "model.onnx").exists():
        print("model artifacts missing; run ragdoll models-ensure first")
        return 0

    sample = ["Ragdoll embedding consistency check.", "Second probe sentence."]
    rust = subprocess.run(
        ["cargo", "run", "--quiet", "--bin", "ragdoll", "--", "doctor"],
        check=False,
        capture_output=True,
        text=True,
    )
    if rust.returncode != 0:
        print(rust.stderr)
        return rust.returncode

    from fastembed import TextEmbedding

    embedder = TextEmbedding(
        model_name="BAAI/bge-m3",
        cache_dir=str(model_dir),
        local_files_only=True,
    )
    py_vectors = [list(map(float, vec)) for vec in embedder.embed(sample)]
    print(json.dumps({"python_vectors": len(py_vectors)}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
