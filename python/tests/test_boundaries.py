# SPDX-License-Identifier: AGPL-3.0-only

from __future__ import annotations

import numpy as np

from ragdoll_worker.chunk.boundaries import find_breakpoints, groups_from_breakpoints


def test_find_breakpoints_applies_section_bias() -> None:
    embeddings = np.array(
        [
            [1.0, 0.0, 0.0],
            [0.99, 0.01, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.99, 0.01],
        ]
    )
    paths = [("A",), ("A",), ("B",), ("B",)]
    breakpoints = find_breakpoints(embeddings, percentile=50, section_bias=1.5, section_paths=paths)
    assert 1 in breakpoints


def test_groups_from_breakpoints() -> None:
    groups = groups_from_breakpoints([1, 3], 5)
    assert groups == [[0, 1], [2, 3], [4]]
