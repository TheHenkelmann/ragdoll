# SPDX-License-Identifier: AGPL-3.0-only

from __future__ import annotations

import numpy as np

from ragdoll_worker.chunk.semantic_split import semantic_split_chunk


class MockTokenizer:
    def encode(self, text: str):
        return type("Enc", (), {"ids": list(range(len(text.split())))})()


class TopicEmbedder:
    def embed(self, texts: list[str]):
        for text in texts:
            lower = text.lower()
            if "alpha" in lower or "first" in lower:
                vec = np.array([1.0, 0.0, 0.0])
            elif "beta" in lower or "second" in lower:
                vec = np.array([0.0, 1.0, 0.0])
            else:
                vec = np.array([0.0, 0.0, 1.0])
            yield vec


def test_semantic_split_keeps_list_items_whole() -> None:
    text = """# Topic

- alpha first item stays whole
- beta second item stays whole
"""
    source = {"name": "doc.md", "uri": "", "metadata": {"tag": "t"}}
    settings = {
        "sentence_buffer": 1,
        "breakpoint_percentile": 50,
        "min_chunk_tokens": 1,
        "max_chunk_tokens": 100,
    }
    chunks = semantic_split_chunk(text, TopicEmbedder(), settings, MockTokenizer(), source)
    contents = [c["content"] for c in chunks]
    for content in contents:
        if "alpha first" in content:
            assert "- alpha first item stays whole" in content
        if "beta second" in content:
            assert "- beta second item stays whole" in content


def test_semantic_split_adds_section_prefix() -> None:
    text = "# Install\n\nalpha content here."
    source = {"name": "doc.md", "uri": "", "metadata": {}}
    settings = {
        "sentence_buffer": 1,
        "breakpoint_percentile": 95,
        "min_chunk_tokens": 1,
        "max_chunk_tokens": 512,
    }
    chunks = semantic_split_chunk(text, TopicEmbedder(), settings, MockTokenizer(), source)
    assert chunks[0]["content"].startswith("[Install]")
    assert chunks[0]["metadata"]["section_path"] == ["Install"]
