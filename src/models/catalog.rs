// SPDX-License-Identifier: AGPL-3.0-only

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelKind {
    Embed,
    Rerank,
}

impl ModelKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Embed => "embed",
            Self::Rerank => "rerank",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadStrategy {
    FastEmbedPreset,
    UserDefined,
}

#[derive(Debug, Clone, Copy)]
pub struct CatalogEntry {
    pub name: &'static str,
    pub kind: ModelKind,
    pub languages: &'static [&'static str],
    pub load_strategy: LoadStrategy,
    pub doc_slug: &'static str,
}

const CATALOG: &[CatalogEntry] = &[
    // --- Embedding (existing) ---
    CatalogEntry {
        name: "BAAI/bge-m3",
        kind: ModelKind::Embed,
        languages: &["multilingual"],
        load_strategy: LoadStrategy::FastEmbedPreset,
        doc_slug: "bge-m3",
    },
    CatalogEntry {
        name: "BAAI/bge-large-en-v1.5",
        kind: ModelKind::Embed,
        languages: &["en"],
        load_strategy: LoadStrategy::FastEmbedPreset,
        doc_slug: "bge-large-en",
    },
    CatalogEntry {
        name: "mixedbread-ai/mxbai-embed-large-v1",
        kind: ModelKind::Embed,
        languages: &["en"],
        load_strategy: LoadStrategy::FastEmbedPreset,
        doc_slug: "mxbai-embed-large",
    },
    CatalogEntry {
        name: "intfloat/multilingual-e5-large",
        kind: ModelKind::Embed,
        languages: &["multilingual"],
        load_strategy: LoadStrategy::FastEmbedPreset,
        doc_slug: "multilingual-e5-large",
    },
    // --- Embedding (new) ---
    CatalogEntry {
        name: "Snowflake/snowflake-arctic-embed-l-v2.0",
        kind: ModelKind::Embed,
        languages: &["multilingual"],
        load_strategy: LoadStrategy::UserDefined,
        doc_slug: "snowflake-arctic-embed-l-v2",
    },
    CatalogEntry {
        name: "mixedbread-ai/deepset-mxbai-embed-de-large-v1",
        kind: ModelKind::Embed,
        languages: &["de", "en"],
        load_strategy: LoadStrategy::UserDefined,
        doc_slug: "deepset-mxbai-embed-de",
    },
    CatalogEntry {
        name: "jinaai/jina-embeddings-v3",
        kind: ModelKind::Embed,
        languages: &["multilingual"],
        load_strategy: LoadStrategy::UserDefined,
        doc_slug: "jina-embeddings-v3",
    },
    CatalogEntry {
        name: "intfloat/multilingual-e5-large-instruct",
        kind: ModelKind::Embed,
        languages: &["multilingual"],
        load_strategy: LoadStrategy::UserDefined,
        doc_slug: "multilingual-e5-large-instruct",
    },
    CatalogEntry {
        name: "Alibaba-NLP/gte-large-en-v1.5",
        kind: ModelKind::Embed,
        languages: &["en"],
        load_strategy: LoadStrategy::FastEmbedPreset,
        doc_slug: "gte-large-en",
    },
    // --- Rerank (existing) ---
    CatalogEntry {
        name: "BAAI/bge-reranker-v2-m3",
        kind: ModelKind::Rerank,
        languages: &["multilingual"],
        load_strategy: LoadStrategy::FastEmbedPreset,
        doc_slug: "bge-reranker-v2-m3",
    },
    CatalogEntry {
        name: "jinaai/jina-reranker-v2-base-multilingual",
        kind: ModelKind::Rerank,
        languages: &["multilingual"],
        load_strategy: LoadStrategy::FastEmbedPreset,
        doc_slug: "jina-reranker-v2",
    },
    CatalogEntry {
        name: "mixedbread-ai/mxbai-rerank-base-v1",
        kind: ModelKind::Rerank,
        languages: &["en"],
        load_strategy: LoadStrategy::UserDefined,
        doc_slug: "mxbai-rerank-base-v1",
    },
];

pub fn predefined_catalog() -> &'static [CatalogEntry] {
    CATALOG
}

pub fn find_catalog_entry(name: &str) -> Option<&'static CatalogEntry> {
    CATALOG.iter().find(|e| e.name == name)
}

pub fn is_predefined_model(name: &str) -> bool {
    find_catalog_entry(name).is_some()
}

pub fn load_strategy_for(name: &str) -> Option<LoadStrategy> {
    find_catalog_entry(name).map(|e| e.load_strategy)
}

pub fn all_supported_model_names() -> Vec<(&'static str, &'static str)> {
    CATALOG.iter().map(|e| (e.name, e.kind.as_str())).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_has_all_requested_models() {
        assert!(find_catalog_entry("Snowflake/snowflake-arctic-embed-l-v2.0").is_some());
        assert!(find_catalog_entry("mixedbread-ai/mxbai-rerank-base-v1").is_some());
        assert!(find_catalog_entry("BAAI/bge-m3").is_some());
    }

    #[test]
    fn embed_and_rerank_counts() {
        let embeds = CATALOG
            .iter()
            .filter(|e| e.kind == ModelKind::Embed)
            .count();
        let reranks = CATALOG
            .iter()
            .filter(|e| e.kind == ModelKind::Rerank)
            .count();
        assert_eq!(embeds, 9);
        assert_eq!(reranks, 3);
    }
}
