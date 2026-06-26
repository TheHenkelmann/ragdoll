# SPDX-License-Identifier: AGPL-3.0-only

from __future__ import annotations

from pathlib import Path
from unittest.mock import MagicMock, patch

import pytest

from ragdoll_worker.extract.files import (
    extract_docx,
    extract_file,
    extract_from_source,
    extract_pdf,
    extract_pptx,
    extract_url,
    extract_xlsx,
    ocr_pdf,
)
from conftest import make_docx, make_pptx, make_xlsx


def test_extract_from_source_unsupported_type(tmp_path: Path) -> None:
    with pytest.raises(RuntimeError, match="unsupported source type"):
        extract_from_source({"id": "x", "type": "video", "uri": None}, tmp_path)


def test_extract_from_source_text_missing_staging_file(tmp_path: Path) -> None:
    with pytest.raises(RuntimeError, match="text staging file missing"):
        extract_from_source({"id": "missing", "type": "text", "uri": None}, tmp_path)


def test_extract_from_source_url_missing_uri(tmp_path: Path) -> None:
    with pytest.raises(RuntimeError, match="url source missing uri"):
        extract_from_source({"id": "u", "type": "url", "uri": None}, tmp_path)


def test_extract_from_source_file_missing_uri(tmp_path: Path) -> None:
    with pytest.raises(RuntimeError, match="file source missing uri"):
        extract_from_source({"id": "f", "type": "file", "uri": None}, tmp_path)


def test_extract_from_source_file_routes_to_extract_file(tmp_path: Path) -> None:
    path = tmp_path / "notes.md"
    path.write_text("# Heading\n\nBody", encoding="utf-8")
    text = extract_from_source(
        {"id": "f", "type": "file", "uri": str(path), "name": "notes.md"},
        tmp_path,
    )
    assert "Heading" in text


@pytest.mark.parametrize(
    ("suffix", "content"),
    [
        (".txt", "plain text"),
        (".md", "# Title\n\nmarkdown"),
        (".csv", "a,b,c\n1,2,3"),
        (".json", '{"key": "value"}'),
    ],
)
def test_extract_file_text_formats(tmp_path: Path, suffix: str, content: str) -> None:
    path = tmp_path / f"sample{suffix}"
    path.write_text(content, encoding="utf-8")
    assert extract_file(path) == content


def test_extract_file_uses_name_when_suffix_missing(tmp_path: Path) -> None:
    path = tmp_path / "blob"
    path.write_text("named markdown", encoding="utf-8")
    assert extract_file(path, name="blob.md") == "named markdown"


def test_extract_file_unsupported_type(tmp_path: Path) -> None:
    path = tmp_path / "archive.zip"
    path.write_bytes(b"PK")
    with pytest.raises(RuntimeError, match="unsupported file type"):
        extract_file(path)


def test_extract_docx_reads_paragraphs(tmp_path: Path) -> None:
    path = make_docx(tmp_path / "doc.docx", ["First paragraph", "Second paragraph"])
    text = extract_docx(path)
    assert "First paragraph" in text
    assert "Second paragraph" in text


def test_extract_xlsx_reads_cell_values(tmp_path: Path) -> None:
    path = make_xlsx(
        tmp_path / "sheet.xlsx",
        [["Name", "Score"], ["Alice", 10], ["Bob", 20]],
    )
    text = extract_xlsx(path)
    assert "Alice" in text
    assert "Bob" in text
    assert "Score" in text


def test_extract_pptx_reads_slide_text(tmp_path: Path) -> None:
    path = make_pptx(tmp_path / "deck.pptx", ["Slide one", "Slide two"])
    text = extract_pptx(path)
    assert "Slide one" in text
    assert "Slide two" in text


def test_extract_file_docx_xlsx_pptx(tmp_path: Path) -> None:
    docx_path = make_docx(tmp_path / "a.docx", ["docx body"])
    xlsx_path = make_xlsx(tmp_path / "a.xlsx", [["cell"]])
    pptx_path = make_pptx(tmp_path / "a.pptx", ["slide body"])

    assert "docx body" in extract_file(docx_path)
    assert "cell" in extract_file(xlsx_path)
    assert "slide body" in extract_file(pptx_path)


@patch("ragdoll_worker.extract.files.ocr_pdf")
@patch("ragdoll_worker.extract.files.PdfReader")
def test_extract_pdf_uses_text_layer(mock_reader_cls: MagicMock, mock_ocr: MagicMock, tmp_path: Path) -> None:
    page = MagicMock()
    page.extract_text.return_value = "Page one text"
    reader = MagicMock()
    reader.pages = [page]
    mock_reader_cls.return_value = reader

    path = tmp_path / "scan.pdf"
    path.write_bytes(b"%PDF-1.4")

    assert extract_pdf(path) == "Page one text"
    mock_ocr.assert_not_called()


@patch("ragdoll_worker.extract.files.ocr_pdf")
@patch("ragdoll_worker.extract.files.PdfReader")
def test_extract_pdf_falls_back_to_ocr(mock_reader_cls: MagicMock, mock_ocr: MagicMock, tmp_path: Path) -> None:
    page = MagicMock()
    page.extract_text.return_value = "   "
    reader = MagicMock()
    reader.pages = [page]
    mock_reader_cls.return_value = reader
    mock_ocr.return_value = "OCR text"

    path = tmp_path / "scan.pdf"
    path.write_bytes(b"%PDF-1.4")

    assert extract_pdf(path) == "OCR text"
    mock_ocr.assert_called_once_with(path)


@patch("ragdoll_worker.extract.files.pytesseract.image_to_string")
@patch("ragdoll_worker.extract.files.convert_from_path")
def test_ocr_pdf_extracts_from_images(
    mock_convert: MagicMock,
    mock_ocr_text: MagicMock,
    tmp_path: Path,
) -> None:
    image = MagicMock()
    mock_convert.return_value = [image]
    mock_ocr_text.return_value = "Recognized text"

    path = tmp_path / "image.pdf"
    path.write_bytes(b"%PDF-1.4")

    assert ocr_pdf(path) == "Recognized text"
    mock_convert.assert_called_once_with(str(path))
    mock_ocr_text.assert_called_once_with(image, lang="deu+eng")


@patch("ragdoll_worker.extract.files.httpx.get")
@patch("ragdoll_worker.extract.files.trafilatura.extract")
def test_extract_url_falls_back_to_raw_html(mock_extract: MagicMock, mock_get: MagicMock) -> None:
    response = MagicMock()
    response.text = "<html><body>raw fallback</body></html>"
    response.raise_for_status = MagicMock()
    mock_get.return_value = response
    mock_extract.return_value = None

    assert extract_url("https://example.com/raw") == response.text


@patch("ragdoll_worker.extract.files.httpx.get")
@patch("ragdoll_worker.extract.files.trafilatura.extract")
def test_extract_url_uses_trafilatura_when_available(mock_extract: MagicMock, mock_get: MagicMock) -> None:
    response = MagicMock()
    response.text = "<html><body>ignored</body></html>"
    response.raise_for_status = MagicMock()
    mock_get.return_value = response
    mock_extract.return_value = "Clean markdown body"

    assert extract_url("https://example.com/clean") == "Clean markdown body"
