# SPDX-License-Identifier: AGPL-3.0-only

from ragdoll_worker.chunk.units import parse_atomic_units


def test_plain_paragraphs_from_blank_lines() -> None:
    text = "First paragraph.\n\nSecond paragraph."
    source = {"name": "notes.txt", "uri": "", "metadata": {}}
    units = parse_atomic_units(text, source)
    paragraphs = [u for u in units if u.kind == "paragraph"]
    assert len(paragraphs) == 2
    assert all(u.splittable for u in paragraphs)


def test_plain_list_lines_become_list_items() -> None:
    text = "- alpha\n- beta"
    source = {"name": "notes.txt", "uri": "", "metadata": {}}
    units = parse_atomic_units(text, source)
    assert len(units) == 2
    assert units[0].kind == "list_item"
    assert units[1].kind == "list_item"
    assert not units[0].splittable
