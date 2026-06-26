# SPDX-License-Identifier: AGPL-3.0-only

from __future__ import annotations

from typing import Any
from uuid import uuid4

import numpy as np
import structlog
from tokenizers import Tokenizer

from ragdoll_worker.chunk.boundaries import find_breakpoints, groups_from_breakpoints
from ragdoll_worker.chunk.sentences import split_sentences
from ragdoll_worker.chunk.tokens import (
    assemble_units_text,
    build_unit_chunk_records,
    count_tokens,
    pack_unit_groups,
)
from ragdoll_worker.chunk.units import AtomicUnit, parse_atomic_units

log = structlog.get_logger()


def _sentence_units_from_unit(unit: AtomicUnit) -> list[AtomicUnit]:
    local_spans = split_sentences(unit.text)
    return [
        AtomicUnit(
            text=span.text,
            start=unit.start + span.start,
            end=unit.start + span.end,
            kind=unit.kind,
            splittable=False,
            section_path=unit.section_path,
        )
        for span in local_spans
    ]


def _semantic_split_indices(
    units: list[AtomicUnit],
    embedder: Any,
    buffer: int,
    percentile: float,
) -> list[list[int]]:
    count = len(units)
    if count <= 1:
        return [list(range(count))]

    texts = [unit.text for unit in units]
    windows: list[str] = []
    for idx in range(count):
        start = max(0, idx - buffer)
        end = min(count, idx + buffer + 1)
        windows.append(" ".join(texts[start:end]))

    embeddings = np.array(list(embedder.embed(windows)))
    section_paths = [unit.section_path for unit in units]
    breakpoints = find_breakpoints(embeddings, percentile, section_paths=section_paths)
    return groups_from_breakpoints(breakpoints, count)


def _split_atomic_units(
    units: list[AtomicUnit],
    embedder: Any,
    buffer: int,
    percentile: float,
    max_tokens: int,
    tokenizer: Tokenizer,
) -> list[list[AtomicUnit]]:
    if not units:
        return []

    assembled = assemble_units_text(units)
    if count_tokens(assembled, tokenizer) <= max_tokens:
        return [units]

    index_groups = _semantic_split_indices(units, embedder, buffer, percentile)
    result: list[list[AtomicUnit]] = []

    for group_indices in index_groups:
        group_units = [units[i] for i in group_indices]
        group_text = assemble_units_text(group_units)
        if count_tokens(group_text, tokenizer) <= max_tokens:
            result.append(group_units)
            continue

        if len(group_units) > 1:
            result.extend(
                _split_atomic_units(
                    group_units,
                    embedder,
                    buffer,
                    percentile,
                    max_tokens,
                    tokenizer,
                )
            )
            continue

        unit = group_units[0]
        if unit.splittable:
            sentence_units = _sentence_units_from_unit(unit)
            if len(sentence_units) <= 1:
                result.append(group_units)
            else:
                result.extend(
                    _split_atomic_units(
                        sentence_units,
                        embedder,
                        buffer,
                        percentile,
                        max_tokens,
                        tokenizer,
                    )
                )
        else:
            log.warning(
                "oversized_non_splittable_unit",
                kind=unit.kind,
                token_count=count_tokens(group_text, tokenizer),
                max_tokens=max_tokens,
            )
            result.append(group_units)

    return result


def semantic_split_chunk(
    text: str,
    embedder: Any,
    settings: dict[str, Any],
    tokenizer: Tokenizer,
    source: dict[str, Any],
) -> list[dict[str, Any]]:
    units = parse_atomic_units(text, source)
    if not units:
        raise RuntimeError("no atomic units extracted")

    source_metadata = dict(source.get("metadata", {}) or {})
    min_tokens = int(settings.get("min_chunk_tokens", 64))
    max_tokens = int(settings.get("max_chunk_tokens", 512))
    buffer = int(settings.get("sentence_buffer", 2))
    percentile = float(settings.get("breakpoint_percentile", 95))

    if count_tokens(text, tokenizer) <= max_tokens:
        groups = [units]
    else:
        groups = _split_atomic_units(
            units,
            embedder,
            buffer,
            percentile,
            max_tokens,
            tokenizer,
        )

    groups = pack_unit_groups(groups, min_tokens, max_tokens, tokenizer)
    chunk_records = build_unit_chunk_records(groups, source_metadata)

    if not chunk_records:
        raise RuntimeError("no chunks produced")

    final_texts = [record["content"] for record in chunk_records]
    final_embeddings = list(embedder.embed(final_texts))
    for record, embedding in zip(chunk_records, final_embeddings, strict=True):
        record["embedding"] = list(map(float, embedding))
        record["token_count"] = count_tokens(record["content"], tokenizer)
        record["id"] = str(uuid4())
    return chunk_records
