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


def test_split_sentences_falls_back_when_segment_not_found_at_cursor() -> None:
    from unittest.mock import MagicMock, patch

    text = "Hello world. Next bit."
    with patch("ragdoll_worker.chunk.sentences.pysbd.Segmenter") as mock_segmenter_cls:
        segmenter = MagicMock()
        segmenter.segment.return_value = ["  padded sentence  ", "Next bit."]
        mock_segmenter_cls.return_value = segmenter

        spans = split_sentences(text, language="en")

    assert spans
    assert spans[0].text == "padded sentence"
    assert spans[0].text in text or spans[0].start >= 0
