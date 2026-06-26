# SPDX-License-Identifier: AGPL-3.0-only

from __future__ import annotations

import pytest

from ragdoll_worker.chunk.semantic_split import semantic_split_chunk


def test_semantic_split_single_chunk_when_text_fits(mock_embedder, mock_tokenizer) -> None:
    text = "Short document that fits in one chunk."
    source = {"name": "doc.txt", "uri": "", "metadata": {"dept": "qa"}}
    settings = {
        "min_chunk_tokens": 1,
        "max_chunk_tokens": 512,
        "sentence_buffer": 1,
        "breakpoint_percentile": 95,
    }

    chunks = semantic_split_chunk(text, mock_embedder, settings, mock_tokenizer, source)

    assert len(chunks) == 1
    assert chunks[0]["content"] == text
    assert chunks[0]["metadata"]["dept"] == "qa"
    assert len(chunks[0]["embedding"]) == mock_embedder.dim
    assert chunks[0]["token_count"] > 0
    assert chunks[0]["id"]


def test_semantic_split_splits_long_plain_text(mock_embedder, mock_tokenizer) -> None:
    paragraphs = [f"Paragraph {idx} with enough words to grow token count." for idx in range(20)]
    text = "\n\n".join(paragraphs)
    source = {"name": "long.txt", "uri": "", "metadata": {}}
    settings = {
        "min_chunk_tokens": 1,
        "max_chunk_tokens": 8,
        "sentence_buffer": 1,
        "breakpoint_percentile": 50,
    }

    chunks = semantic_split_chunk(text, mock_embedder, settings, mock_tokenizer, source)

    assert len(chunks) > 1
    for chunk in chunks:
        assert chunk["token_count"] <= 8 or chunk["content"].strip()
        assert len(chunk["embedding"]) == mock_embedder.dim


def test_semantic_split_raises_when_no_units(mock_embedder, mock_tokenizer) -> None:
    with pytest.raises(RuntimeError, match="no atomic units extracted"):
        semantic_split_chunk(
            "   \n\n  ",
            mock_embedder,
            {"max_chunk_tokens": 512},
            mock_tokenizer,
            {"name": "empty.txt", "uri": ""},
        )


def test_semantic_split_adds_section_prefix_for_markdown(mock_embedder, mock_tokenizer) -> None:
    text = "# Setup\n\nInstall dependencies and verify the build."
    source = {"name": "guide.md", "uri": "", "metadata": {}}
    settings = {
        "min_chunk_tokens": 1,
        "max_chunk_tokens": 512,
        "sentence_buffer": 1,
        "breakpoint_percentile": 95,
    }

    chunks = semantic_split_chunk(text, mock_embedder, settings, mock_tokenizer, source)

    assert chunks[0]["content"].startswith("[Setup]")
    assert chunks[0]["metadata"]["section_path"] == ["Setup"]


def test_semantic_split_embedder_is_deterministic(mock_embedder, mock_tokenizer) -> None:
    text = "# Topic\n\nAlpha content here."
    source = {"name": "doc.md", "uri": "", "metadata": {}}
    settings = {
        "min_chunk_tokens": 1,
        "max_chunk_tokens": 512,
        "sentence_buffer": 1,
        "breakpoint_percentile": 95,
    }

    first = semantic_split_chunk(text, mock_embedder, settings, mock_tokenizer, source)
    second = semantic_split_chunk(text, mock_embedder, settings, mock_tokenizer, source)

    assert first[0]["embedding"] == second[0]["embedding"]
