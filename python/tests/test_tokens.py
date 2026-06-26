# SPDX-License-Identifier: AGPL-3.0-only

from __future__ import annotations

from ragdoll_worker.chunk.tokens import (
    assemble_units_text,
    build_unit_chunk_records,
    pack_unit_groups,
    section_prefix,
)
from ragdoll_worker.chunk.units import AtomicUnit


def test_section_prefix_formats_heading_path() -> None:
    assert section_prefix(("Install", "macOS")) == "[Install > macOS]"


def test_assemble_units_text_joins_with_blank_lines() -> None:
    units = [
        AtomicUnit(
            text="first", start=0, end=5, kind="paragraph", splittable=True, section_path=()
        ),
        AtomicUnit(
            text="second", start=6, end=12, kind="paragraph", splittable=True, section_path=()
        ),
    ]
    assert assemble_units_text(units) == "first\n\nsecond"


def test_pack_unit_groups_merges_small_same_section_groups(mock_tokenizer) -> None:
    units_a = [
        AtomicUnit(
            text="short", start=0, end=5, kind="paragraph", splittable=True, section_path=("a",)
        ),
    ]
    units_b = [
        AtomicUnit(
            text="also short",
            start=6,
            end=16,
            kind="paragraph",
            splittable=True,
            section_path=("a",),
        ),
    ]
    packed = pack_unit_groups(
        [units_a, units_b], min_tokens=10, max_tokens=50, tokenizer=mock_tokenizer
    )
    assert len(packed) == 1
    assert len(packed[0]) == 2


def test_build_unit_chunk_records_includes_metadata() -> None:
    group = [
        AtomicUnit(
            text="# Title",
            start=0,
            end=7,
            kind="heading",
            splittable=False,
            section_path=("Title",),
        )
    ]
    records = build_unit_chunk_records([group], {"department": "hr"})
    assert records[0]["metadata"]["department"] == "hr"
    assert records[0]["metadata"]["section_path"] == ["Title"]
    assert records[0]["content"].startswith("[Title]")
