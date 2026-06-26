# SPDX-License-Identifier: AGPL-3.0-only

from __future__ import annotations

from typing import Any

from tokenizers import Tokenizer

from ragdoll_worker.chunk.units import AtomicUnit


def count_tokens(text: str, tokenizer: Tokenizer) -> int:
    return len(tokenizer.encode(text).ids)


def assemble_units_text(units: list[AtomicUnit]) -> str:
    return "\n\n".join(unit.text for unit in units)


def section_prefix(section_path: tuple[str, ...]) -> str:
    if not section_path:
        return ""
    return "[" + " > ".join(section_path) + "]"


def pack_unit_groups(
    groups: list[list[AtomicUnit]],
    min_tokens: int,
    max_tokens: int,
    tokenizer: Tokenizer,
) -> list[list[AtomicUnit]]:
    if not groups:
        return []

    merged = list(groups)
    output: list[list[AtomicUnit]] = []
    idx = 0
    while idx < len(merged):
        group = merged[idx]
        text = assemble_units_text(group)
        if (
            count_tokens(text, tokenizer) < min_tokens
            and idx + 1 < len(merged)
            and group[0].section_path == merged[idx + 1][0].section_path
        ):
            combined = group + merged[idx + 1]
            combined_text = assemble_units_text(combined)
            if count_tokens(combined_text, tokenizer) <= max_tokens:
                merged[idx + 1] = combined
                idx += 1
                continue
        output.append(group)
        idx += 1
    return output


def build_unit_chunk_records(
    groups: list[list[AtomicUnit]],
    source_metadata: dict[str, Any],
) -> list[dict[str, Any]]:
    chunks: list[dict[str, Any]] = []
    for group in groups:
        if not group:
            continue
        body = assemble_units_text(group)
        prefix = section_prefix(group[0].section_path)
        content = f"{prefix}\n\n{body}" if prefix else body
        content = content.strip()
        if not content:
            continue
        metadata = dict(source_metadata)
        if group[0].section_path:
            metadata["section_path"] = list(group[0].section_path)
        metadata["unit_kinds"] = [unit.kind for unit in group]
        chunks.append(
            {
                "content": content,
                "provenance": [{"start": group[0].start, "end": group[-1].end}],
                "metadata": metadata,
            }
        )
    return chunks
