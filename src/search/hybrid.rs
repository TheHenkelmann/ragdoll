// SPDX-License-Identifier: AGPL-3.0-only

use std::collections::HashMap;

const RRF_K: f64 = 60.0;

pub fn reciprocal_rank_fusion(
    vector_ranked: &[String],
    bm25_ranked: &[String],
    bm25_weight: f32,
) -> Vec<(String, f64)> {
    let weight = bm25_weight.max(0.0) as f64;
    let mut scores: HashMap<&str, f64> = HashMap::new();

    for (rank, id) in vector_ranked.iter().enumerate() {
        *scores.entry(id.as_str()).or_default() += 1.0 / (RRF_K + rank as f64 + 1.0);
    }
    for (rank, id) in bm25_ranked.iter().enumerate() {
        *scores.entry(id.as_str()).or_default() += weight * (1.0 / (RRF_K + rank as f64 + 1.0));
    }

    let mut fused: Vec<(String, f64)> = scores
        .into_iter()
        .map(|(id, score)| (id.to_string(), score))
        .collect();
    fused.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    fused
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rrf_fuses_both_lists() {
        let vector = vec!["a".into(), "b".into(), "c".into()];
        let bm25 = vec!["b".into(), "c".into(), "a".into()];
        let fused = reciprocal_rank_fusion(&vector, &bm25, 1.0);
        assert_eq!(fused.len(), 3);
        assert!(fused[0].1 >= fused[1].1);
    }
}
