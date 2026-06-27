// SPDX-License-Identifier: AGPL-3.0-only

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Citation {
    pub citation_id: String,
    pub source_id: String,
    pub source_name: String,
    pub source_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub section_path: Option<Vec<String>>,
    pub char_start: i64,
    pub char_end: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ProvenanceSpan {
    start: i64,
    end: i64,
}

#[derive(Debug, Clone, Deserialize)]
struct PageMapEntry {
    page: i64,
    start: i64,
    end: i64,
}

#[allow(clippy::too_many_arguments)]
pub fn build_citation(
    chunk_id: &str,
    embedding_version: &str,
    source_id: &str,
    source_name: &str,
    source_type: &str,
    uri: Option<&str>,
    metadata: &serde_json::Value,
    provenance: &serde_json::Value,
    page_map_json: &str,
    source_text: Option<&str>,
    include_snippet: bool,
    snippet_context: usize,
) -> Citation {
    let (char_start, char_end) = parse_provenance(provenance);
    let section_path = metadata
        .get("section_path")
        .and_then(|v| serde_json::from_value(v.clone()).ok());
    let page = resolve_page(char_start, page_map_json);

    let snippet = if include_snippet {
        source_text.map(|text| extract_snippet(text, char_start, char_end, snippet_context))
    } else {
        None
    };

    Citation {
        citation_id: format!("{chunk_id}:{embedding_version}"),
        source_id: source_id.to_string(),
        source_name: source_name.to_string(),
        source_type: source_type.to_string(),
        uri: uri.map(str::to_string),
        section_path,
        char_start,
        char_end,
        page,
        snippet,
    }
}

fn parse_provenance(provenance: &serde_json::Value) -> (i64, i64) {
    if let Ok(spans) = serde_json::from_value::<Vec<ProvenanceSpan>>(provenance.clone()) {
        if let Some(first) = spans.first() {
            return (first.start, first.end);
        }
    }
    (0, 0)
}

fn resolve_page(char_start: i64, page_map_json: &str) -> Option<i64> {
    let entries: Vec<PageMapEntry> = serde_json::from_str(page_map_json).unwrap_or_default();
    for entry in entries {
        if char_start >= entry.start && char_start <= entry.end {
            return Some(entry.page);
        }
    }
    None
}

fn extract_snippet(text: &str, start: i64, end: i64, context: usize) -> String {
    let len = text.len();
    if len == 0 {
        return String::new();
    }
    let start = start.clamp(0, len as i64) as usize;
    let end = end.clamp(start as i64, len as i64) as usize;
    let from = start.saturating_sub(context);
    let to = (end + context).min(len);
    text[from..to].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_citation_includes_page_from_map() {
        let page_map = r#"[{"page":1,"start":0,"end":100},{"page":2,"start":102,"end":200}]"#;
        let citation = build_citation(
            "chunk-1",
            "1",
            "src-1",
            "doc.pdf",
            "file",
            None,
            &serde_json::json!({"section_path": ["Intro"]}),
            &serde_json::json!([{"start": 110, "end": 150}]),
            page_map,
            Some("hello world page two content"),
            false,
            20,
        );
        assert_eq!(citation.page, Some(2));
        assert_eq!(citation.section_path, Some(vec!["Intro".to_string()]));
    }

    #[test]
    fn extract_snippet_adds_context() {
        let text = "0123456789abcdef";
        let snippet = extract_snippet(text, 4, 8, 2);
        assert_eq!(snippet, "23456789");
    }
}
