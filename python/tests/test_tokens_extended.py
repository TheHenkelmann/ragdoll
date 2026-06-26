# SPDX-License-Identifier: AGPL-3.0-only

from __future__ import annotations

from ragdoll_worker.chunk.tokens import (
    assemble_units_text,
    build_unit_chunk_records,
    pack_unit_groups,
    section_prefix,
)
from ragdoll_worker.chunk.units import AtomicUnit


def _unit(
    text: str,
    *,
    start: int = 0,
    end: int | None = None,
    section_path: tuple[str, ...] = (),
    kind: str = "paragraph",
) -> AtomicUnit:
    return AtomicUnit(
        text=text,
        start=start,
        end=end if end is not None else len(text),
        kind=kind,
        splittable=True,
        section_path=section_path,
    )


def test_section_prefix_empty_path() -> None:
    assert section_prefix(()) == ""


def test_assemble_units_text_empty_list() -> None:
    assert assemble_units_text([]) == ""


def test_pack_unit_groups_returns_empty_for_empty_input(mock_tokenizer) -> None:
    assert pack_unit_groups([], min_tokens=1, max_tokens=10, tokenizer=mock_tokenizer) == []


def test_pack_unit_groups_does_not_merge_different_sections(mock_tokenizer) -> None:
    groups = [
        [_unit("alpha", section_path=("a",))],
        [_unit("beta", section_path=("b",))],
    ]
    packed = pack_unit_groups(groups, min_tokens=100, max_tokens=200, tokenizer=mock_tokenizer)
    assert len(packed) == 2


def test_pack_unit_groups_skips_merge_when_combined_exceeds_max(mock_tokenizer) -> None:
    groups = [
        [_unit("one two three four five six", section_path=("same",))],
        [_unit("seven eight nine ten eleven twelve", section_path=("same",))],
    ]
    packed = pack_unit_groups(groups, min_tokens=100, max_tokens=5, tokenizer=mock_tokenizer)
    assert len(packed) == 2


def test_pack_unit_groups_keeps_last_group_when_no_neighbor(mock_tokenizer) -> None:
    groups = [[_unit("tiny", section_path=("solo",))]]
    packed = pack_unit_groups(groups, min_tokens=100, max_tokens=200, tokenizer=mock_tokenizer)
    assert len(packed) == 1
    assert packed[0][0].text == "tiny"


def test_build_unit_chunk_records_skips_empty_groups() -> None:
    group = [_unit("visible")]
    records = build_unit_chunk_records([[], group], {"source": "test"})
    assert len(records) == 1
    assert records[0]["content"] == "visible"


def test_build_unit_chunk_records_without_section_prefix() -> None:
    group = [_unit("plain body", section_path=())]
    records = build_unit_chunk_records([group], {"tag": "x"})
    assert records[0]["content"] == "plain body"
    assert "section_path" not in records[0]["metadata"]


def test_build_unit_chunk_records_skips_whitespace_only_content() -> None:
    group = [
        AtomicUnit(
            text="   ",
            start=0,
            end=3,
            kind="paragraph",
            splittable=False,
            section_path=(),
        )
    ]
    records = build_unit_chunk_records([group], {})
    assert records == []


def test_build_unit_chunk_records_includes_provenance_span() -> None:
    group = [
        _unit("first", start=0, end=5),
        _unit("second", start=7, end=13),
    ]
    records = build_unit_chunk_records([group], {})
    assert records[0]["provenance"] == [{"start": 0, "end": 13}]
    assert records[0]["metadata"]["unit_kinds"] == ["paragraph", "paragraph"]
