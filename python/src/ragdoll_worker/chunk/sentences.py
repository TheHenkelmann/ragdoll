# SPDX-License-Identifier: AGPL-3.0-only

from __future__ import annotations

from dataclasses import dataclass

import pysbd


@dataclass(frozen=True)
class SentenceSpan:
    text: str
    start: int
    end: int


def split_sentences(text: str, language: str = "de") -> list[SentenceSpan]:
    segmenter = pysbd.Segmenter(language=language, clean=False)
    sentences = segmenter.segment(text)
    spans: list[SentenceSpan] = []
    cursor = 0
    for sentence in sentences:
        if not sentence:
            continue
        start = text.find(sentence, cursor)
        if start < 0:
            stripped = sentence.strip()
            if stripped:
                start = text.find(stripped, cursor)
                if start < 0:
                    start = cursor
                end = start + len(stripped)
                spans.append(SentenceSpan(text=stripped, start=start, end=end))
                cursor = max(cursor, end)
            continue
        end = start + len(sentence)
        spans.append(SentenceSpan(text=sentence, start=start, end=end))
        cursor = max(cursor, end)
    return spans
