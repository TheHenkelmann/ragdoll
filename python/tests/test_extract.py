# SPDX-License-Identifier: AGPL-3.0-only

from __future__ import annotations

from pathlib import Path
from unittest.mock import MagicMock, patch

import pytest

from ragdoll_worker.config import sanitize_model_name
from ragdoll_worker.extract.files import extract_file, extract_from_source, extract_url


def test_sanitize_model_name() -> None:
    assert sanitize_model_name("BAAI/bge-m3") == "BAAI__bge-m3"


def test_extract_from_source_text(tmp_path: Path) -> None:
    staging = tmp_path / "staging"
    staging.mkdir()
    source_id = "source-1"
    (staging / f"{source_id}.txt").write_text("hello ragdoll", encoding="utf-8")
    text = extract_from_source(
        {"id": source_id, "type": "text", "uri": None},
        staging,
    )
    assert text == "hello ragdoll"


def test_extract_file_txt(tmp_path: Path) -> None:
    path = tmp_path / "demo.txt"
    path.write_text("plain text content", encoding="utf-8")
    assert extract_file(path) == "plain text content"


@patch("ragdoll_worker.extract.files.httpx.get")
@patch("ragdoll_worker.extract.files.trafilatura.extract")
def test_extract_url_uses_trafilatura(mock_extract: MagicMock, mock_get: MagicMock) -> None:
    response = MagicMock()
    response.text = "<html><body>ignored</body></html>"
    response.raise_for_status = MagicMock()
    mock_get.return_value = response
    mock_extract.return_value = "# Title\n\nBody text"

    result = extract_url("https://example.com/page")
    assert "Body text" in result
    mock_get.assert_called_once()


@pytest.mark.skip(reason="OCR path requires poppler and tesseract system binaries")
def test_ocr_pdf_requires_system_tools() -> None:
    pytest.skip("optional OCR integration test")
