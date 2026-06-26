# SPDX-License-Identifier: AGPL-3.0-only

from __future__ import annotations

import re
from dataclasses import dataclass
from typing import Any, Literal

UnitKind = Literal["heading", "paragraph", "list_item", "code_block", "table_row", "blockquote"]

HEADING_RE = re.compile(r"^(#{1,6})\s+(.*)$")
LIST_RE = re.compile(r"^(\s*)([-*+]|\d+\.)\s+(.*)$")
TABLE_ROW_RE = re.compile(r"^\|.*\|.*$")
BLOCKQUOTE_RE = re.compile(r"^>\s?(.*)$")


@dataclass(frozen=True)
class AtomicUnit:
    text: str
    start: int
    end: int
    kind: UnitKind
    splittable: bool
    section_path: tuple[str, ...]


def detect_format(text: str, source: dict[str, Any]) -> str:
    name = str(source.get("name", ""))
    uri = str(source.get("uri", ""))
    if name.lower().endswith(".md") or uri.lower().endswith(".md"):
        return "markdown"
    if _looks_like_markdown(text):
        return "markdown"
    return "plain"


def _looks_like_markdown(text: str) -> bool:
    for line in text.split("\n")[:50]:
        stripped = line.strip()
        if HEADING_RE.match(stripped):
            return True
        if stripped.startswith("```"):
            return True
        if LIST_RE.match(stripped):
            return True
    return False


def parse_atomic_units(text: str, source: dict[str, Any]) -> list[AtomicUnit]:
    fmt = detect_format(text, source)
    if fmt == "markdown":
        return _parse_markdown(text)
    return _parse_plain(text)


def _parse_markdown(text: str) -> list[AtomicUnit]:
    units: list[AtomicUnit] = []
    section_path: list[str] = []
    lines = text.split("\n")
    pos = 0
    in_code_fence = False
    code_fence_lines: list[str] = []
    code_fence_start = 0
    para_lines: list[str] = []
    para_start = 0

    def flush_paragraph(end_pos: int) -> None:
        nonlocal para_lines, para_start
        if not para_lines:
            return
        block = "\n".join(para_lines)
        units.append(
            AtomicUnit(
                text=block,
                start=para_start,
                end=end_pos,
                kind="paragraph",
                splittable=True,
                section_path=tuple(section_path),
            )
        )
        para_lines = []

    for i, line in enumerate(lines):
        line_start = pos
        line_end = pos + len(line)
        next_pos = line_end + (1 if i < len(lines) - 1 else 0)
        stripped = line.strip()

        if stripped.startswith("```"):
            if in_code_fence:
                code_fence_lines.append(line)
                units.append(
                    AtomicUnit(
                        text="\n".join(code_fence_lines),
                        start=code_fence_start,
                        end=next_pos,
                        kind="code_block",
                        splittable=False,
                        section_path=tuple(section_path),
                    )
                )
                in_code_fence = False
                code_fence_lines = []
            else:
                flush_paragraph(line_start)
                in_code_fence = True
                code_fence_start = line_start
                code_fence_lines = [line]
            pos = next_pos
            continue

        if in_code_fence:
            code_fence_lines.append(line)
            pos = next_pos
            continue

        if not stripped:
            flush_paragraph(line_start)
            pos = next_pos
            continue

        heading_match = HEADING_RE.match(stripped)
        if heading_match:
            flush_paragraph(line_start)
            level = len(heading_match.group(1))
            title = heading_match.group(2).strip()
            while len(section_path) >= level:
                section_path.pop()
            section_path.append(title)
            units.append(
                AtomicUnit(
                    text=line,
                    start=line_start,
                    end=next_pos,
                    kind="heading",
                    splittable=False,
                    section_path=tuple(section_path),
                )
            )
            pos = next_pos
            continue

        if LIST_RE.match(stripped):
            flush_paragraph(line_start)
            units.append(
                AtomicUnit(
                    text=line.rstrip(),
                    start=line_start,
                    end=next_pos,
                    kind="list_item",
                    splittable=False,
                    section_path=tuple(section_path),
                )
            )
            pos = next_pos
            continue

        if TABLE_ROW_RE.match(stripped):
            flush_paragraph(line_start)
            units.append(
                AtomicUnit(
                    text=line.rstrip(),
                    start=line_start,
                    end=next_pos,
                    kind="table_row",
                    splittable=False,
                    section_path=tuple(section_path),
                )
            )
            pos = next_pos
            continue

        blockquote_match = BLOCKQUOTE_RE.match(stripped)
        if blockquote_match:
            flush_paragraph(line_start)
            units.append(
                AtomicUnit(
                    text=line.rstrip(),
                    start=line_start,
                    end=next_pos,
                    kind="blockquote",
                    splittable=True,
                    section_path=tuple(section_path),
                )
            )
            pos = next_pos
            continue

        if not para_lines:
            para_start = line_start
        para_lines.append(line)
        pos = next_pos

    if in_code_fence and code_fence_lines:
        units.append(
            AtomicUnit(
                text="\n".join(code_fence_lines),
                start=code_fence_start,
                end=pos,
                kind="code_block",
                splittable=False,
                section_path=tuple(section_path),
            )
        )
    else:
        flush_paragraph(pos)

    return units


def _parse_plain(text: str) -> list[AtomicUnit]:
    units: list[AtomicUnit] = []
    section_path: tuple[str, ...] = ()
    if not text.strip():
        return units

    blocks = re.split(r"\n\n+", text)
    cursor = 0
    for block in blocks:
        if not block.strip():
            cursor += len(block)
            if cursor < len(text) and text[cursor] == "\n":
                cursor += 1
            continue

        start = text.find(block, cursor)
        if start < 0:
            start = cursor
        end = start + len(block)
        cursor = end
        while cursor < len(text) and text[cursor] == "\n":
            cursor += 1

        lines = block.split("\n")
        non_empty = [line for line in lines if line.strip()]

        if non_empty and all(LIST_RE.match(line.strip()) for line in non_empty):
            line_cursor = start
            for line in lines:
                if not line.strip():
                    line_cursor += len(line) + 1
                    continue
                line_start = text.find(line, line_cursor)
                if line_start < 0:
                    line_start = line_cursor
                line_end = line_start + len(line)
                units.append(
                    AtomicUnit(
                        text=line.rstrip(),
                        start=line_start,
                        end=line_end,
                        kind="list_item",
                        splittable=False,
                        section_path=section_path,
                    )
                )
                line_cursor = line_end + 1
        else:
            units.append(
                AtomicUnit(
                    text=block,
                    start=start,
                    end=end,
                    kind="paragraph",
                    splittable=True,
                    section_path=section_path,
                )
            )

    return units
