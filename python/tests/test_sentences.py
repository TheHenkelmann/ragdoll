# SPDX-License-Identifier: AGPL-3.0-only

from ragdoll_worker.chunk.sentences import split_sentences


def test_split_sentences_tracks_offsets() -> None:
    text = "First sentence. Second sentence."
    spans = split_sentences(text, language="en")
    assert len(spans) >= 2
    assert spans[0].text in text
    assert text[spans[0].start : spans[0].end] == spans[0].text


def test_split_sentences_offsets_are_monotonic() -> None:
    text = "A. B. C."
    spans = split_sentences(text, language="en")
    assert spans
    for span in spans:
        assert text[span.start : span.end] == span.text
    for i in range(1, len(spans)):
        assert spans[i].start >= spans[i - 1].start
