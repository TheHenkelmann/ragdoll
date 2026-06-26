# SPDX-License-Identifier: AGPL-3.0-only

from __future__ import annotations

import numpy as np

from ragdoll_worker.chunk.boundaries import adjacent_distances, find_breakpoints, groups_from_breakpoints


def test_adjacent_distances_for_orthogonal_vectors() -> None:
    embeddings = np.array([[1.0, 0.0], [0.0, 1.0], [1.0, 0.0]])
    assert adjacent_distances(embeddings) == [1.0, 1.0]


def test_find_breakpoints_respects_section_bias() -> None:
    embeddings = np.array(
        [
            [1.0, 0.0],
            [0.99, 0.01],
            [0.0, 1.0],
            [0.0, 0.99],
        ]
    )
    section_paths = [("a",), ("a",), ("b",), ("b",)]
    breakpoints = find_breakpoints(embeddings, percentile=50, section_paths=section_paths)
    assert 1 in breakpoints


def test_groups_from_breakpoints_covers_all_indices() -> None:
    groups = groups_from_breakpoints([1, 3], 5)
    assert groups == [[0, 1], [2, 3], [4]]


def test_adjacent_distances_empty_for_single_embedding() -> None:
    embeddings = np.array([[1.0, 0.0]])
    assert adjacent_distances(embeddings) == []


def test_find_breakpoints_empty_for_single_embedding() -> None:
    embeddings = np.array([[1.0, 0.0, 0.0]])
    assert find_breakpoints(embeddings, percentile=50) == []


def test_find_breakpoints_without_section_paths() -> None:
    embeddings = np.array(
        [
            [1.0, 0.0],
            [0.0, 1.0],
            [1.0, 0.0],
        ]
    )
    breakpoints = find_breakpoints(embeddings, percentile=50)
    assert breakpoints == [0, 1]


def test_find_breakpoints_ignores_mismatched_section_paths_length() -> None:
    embeddings = np.array(
        [
            [1.0, 0.0],
            [0.0, 1.0],
            [1.0, 0.0],
        ]
    )
    section_paths = [("a",), ("b",)]
    breakpoints = find_breakpoints(embeddings, percentile=50, section_paths=section_paths)
    assert breakpoints == [0, 1]


def test_groups_from_breakpoints_with_no_breakpoints() -> None:
    assert groups_from_breakpoints([], 4) == [[0, 1, 2, 3]]


def test_groups_from_breakpoints_with_zero_count() -> None:
    assert groups_from_breakpoints([], 0) == [[]]


def test_groups_from_breakpoints_with_single_breakpoint() -> None:
    assert groups_from_breakpoints([0], 2) == [[0], [1]]
