# SPDX-License-Identifier: AGPL-3.0-only

from __future__ import annotations

import numpy as np


def adjacent_distances(embeddings: np.ndarray) -> list[float]:
    distances: list[float] = []
    for i in range(len(embeddings) - 1):
        distances.append(1.0 - float(np.dot(embeddings[i], embeddings[i + 1])))
    return distances


def find_breakpoints(
    embeddings: np.ndarray,
    percentile: float,
    section_bias: float = 1.15,
    section_paths: list[tuple[str, ...]] | None = None,
) -> list[int]:
    if len(embeddings) <= 1:
        return []

    distances = adjacent_distances(embeddings)
    if section_paths and len(section_paths) == len(embeddings):
        for i in range(len(distances)):
            if section_paths[i] != section_paths[i + 1]:
                distances[i] *= section_bias

    threshold = float(np.percentile(distances, percentile))
    return [idx for idx, distance in enumerate(distances) if distance >= threshold]


def groups_from_breakpoints(breakpoints: list[int], count: int) -> list[list[int]]:
    groups: list[list[int]] = []
    start_idx = 0
    for breakpoint in breakpoints:
        groups.append(list(range(start_idx, breakpoint + 1)))
        start_idx = breakpoint + 1
    groups.append(list(range(start_idx, count)))
    return groups
