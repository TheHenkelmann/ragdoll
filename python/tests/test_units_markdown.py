# SPDX-License-Identifier: AGPL-3.0-only

from ragdoll_worker.chunk.units import parse_atomic_units


def test_markdown_list_items_are_atomic() -> None:
    text = """# Title

Intro paragraph.

- first item
- second item
"""
    source = {"name": "doc.md", "uri": "", "metadata": {}}
    units = parse_atomic_units(text, source)
    list_items = [u for u in units if u.kind == "list_item"]
    assert len(list_items) == 2
    assert all(not u.splittable for u in list_items)
    assert list_items[0].text.strip().startswith("- first")


def test_markdown_section_path_from_headings() -> None:
    text = "# Install\n\n## macOS\n\nSteps here."
    source = {"name": "doc.md", "uri": "", "metadata": {}}
    units = parse_atomic_units(text, source)
    paragraph = next(u for u in units if u.kind == "paragraph")
    assert paragraph.section_path == ("Install", "macOS")


def test_markdown_code_block_not_splittable() -> None:
    text = """# Code

```python
print("hello")
```
"""
    source = {"name": "doc.md", "uri": "", "metadata": {}}
    units = parse_atomic_units(text, source)
    code = next(u for u in units if u.kind == "code_block")
    assert not code.splittable
    assert "print" in code.text
