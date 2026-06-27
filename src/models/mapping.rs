// SPDX-License-Identifier: AGPL-3.0-only

use anyhow::{bail, Result};
use fastembed::{EmbeddingModel, RerankerModel};

use crate::models::catalog::{find_catalog_entry, predefined_catalog, ModelKind};

pub fn supported_embed_models() -> Vec<&'static str> {
    predefined_catalog()
        .iter()
        .filter(|e| e.kind == ModelKind::Embed)
        .map(|e| e.name)
        .collect()
}

pub fn supported_rerank_models() -> Vec<&'static str> {
    predefined_catalog()
        .iter()
        .filter(|e| e.kind == ModelKind::Rerank)
        .map(|e| e.name)
        .collect()
}

pub fn is_supported_embed_model(name: &str) -> bool {
    find_catalog_entry(name).is_some_and(|e| e.kind == ModelKind::Embed)
}

pub fn is_supported_rerank_model(name: &str) -> bool {
    find_catalog_entry(name).is_some_and(|e| e.kind == ModelKind::Rerank)
}

pub fn embedding_model_enum(name: &str) -> Result<EmbeddingModel> {
    match name {
        "BAAI/bge-m3" => Ok(EmbeddingModel::BGEM3),
        "BAAI/bge-large-en-v1.5" => Ok(EmbeddingModel::BGELargeENV15),
        "mixedbread-ai/mxbai-embed-large-v1" => Ok(EmbeddingModel::MxbaiEmbedLargeV1),
        "intfloat/multilingual-e5-large" => Ok(EmbeddingModel::MultilingualE5Large),
        "Alibaba-NLP/gte-large-en-v1.5" => Ok(EmbeddingModel::GTELargeENV15),
        other => bail!("unsupported embedding model preset: {other}"),
    }
}

pub fn reranker_model_enum(name: &str) -> Result<RerankerModel> {
    match name {
        "BAAI/bge-reranker-v2-m3" => Ok(RerankerModel::BGERerankerV2M3),
        "jinaai/jina-reranker-v2-base-multilingual" => {
            Ok(RerankerModel::JINARerankerV2BaseMultiligual)
        }
        other => bail!("unsupported rerank model preset: {other}"),
    }
}

pub fn doc_prefix_for(model_name: &str) -> &'static str {
    match model_name {
        "intfloat/multilingual-e5-large" | "intfloat/multilingual-e5-large-instruct" => {
            "passage: "
        }
        _ => "",
    }
}

pub fn query_prefix_for(model_name: &str) -> &'static str {
    match model_name {
        "intfloat/multilingual-e5-large" | "intfloat/multilingual-e5-large-instruct" => "query: ",
        _ => "",
    }
}

pub fn all_supported_model_names() -> Vec<(&'static str, &'static str)> {
    crate::models::catalog::all_supported_model_names()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_new_embed_models_supported() {
        assert!(is_supported_embed_model("jinaai/jina-embeddings-v3"));
        assert!(is_supported_embed_model("Alibaba-NLP/gte-large-en-v1.5"));
    }

    #[test]
    fn all_new_rerank_models_supported() {
        assert!(is_supported_rerank_model("mixedbread-ai/mxbai-rerank-base-v1"));
        assert!(is_supported_rerank_model("jinaai/jina-reranker-v2-base-multilingual"));
    }

    #[test]
    fn e5_instruct_uses_prefixes() {
        assert_eq!(
            query_prefix_for("intfloat/multilingual-e5-large-instruct"),
            "query: "
        );
    }
}
