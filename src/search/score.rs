// SPDX-License-Identifier: AGPL-3.0-only

pub fn cosine_similarity_from_distance(distance: f64) -> f64 {
    1.0 - distance
}

pub fn normalize_rerank_scores(scores: &[f32]) -> Vec<f32> {
    if scores.is_empty() {
        return Vec::new();
    }
    let min = scores.iter().copied().fold(f32::INFINITY, f32::min);
    let max = scores.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    if (max - min).abs() < f32::EPSILON {
        return vec![1.0; scores.len()];
    }
    scores
        .iter()
        .map(|score| (score - min) / (max - min))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{cosine_similarity_from_distance, normalize_rerank_scores};

    #[test]
    fn cosine_similarity_from_distance_works() {
        assert!((cosine_similarity_from_distance(0.2) - 0.8).abs() < f64::EPSILON);
    }

    #[test]
    fn normalize_rerank_scores_spreads_values() {
        let scores = normalize_rerank_scores(&[1.0, 2.0, 3.0]);
        assert_eq!(scores.first(), Some(&0.0));
        assert_eq!(scores.last(), Some(&1.0));
    }
}
