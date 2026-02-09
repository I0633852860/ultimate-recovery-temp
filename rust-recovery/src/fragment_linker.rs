use std::collections::HashSet;

use crate::smart_separation::{ByteFrequency, SmartSeparation};

#[derive(Debug, Clone, Default)]
pub struct ExFatMetadata {
    pub filename: Option<String>,
    pub first_cluster: Option<u32>,
    pub size: Option<u64>,
}

impl ExFatMetadata {
    pub fn match_score(&self, other: &Self) -> f32 {
        let name_match = match (&self.filename, &other.filename) {
            (Some(left), Some(right)) => left.eq_ignore_ascii_case(right),
            _ => false,
        };
        let cluster_match = match (self.first_cluster, other.first_cluster) {
            (Some(left), Some(right)) => left == right,
            _ => false,
        };
        let size_match = match (self.size, other.size) {
            (Some(left), Some(right)) => left == right,
            _ => false,
        };

        if name_match || cluster_match || size_match {
            1.0
        } else {
            0.0
        }
    }
}

#[derive(Debug, Clone)]
pub struct FragmentDescriptor {
    pub byte_frequency: ByteFrequency,
    pub links: HashSet<String>,
    pub exfat_metadata: Option<ExFatMetadata>,
}

impl FragmentDescriptor {
    pub fn new(data: &[u8]) -> Self {
        Self {
            byte_frequency: ByteFrequency::from_bytes(data),
            links: HashSet::new(),
            exfat_metadata: None,
        }
    }

    pub fn with_links<I>(mut self, links: I) -> Self
    where
        I: IntoIterator<Item = String>,
    {
        self.links = links.into_iter().collect();
        self
    }

    pub fn with_exfat_metadata(mut self, metadata: ExFatMetadata) -> Self {
        self.exfat_metadata = Some(metadata);
        self
    }
}

#[derive(Debug, Clone)]
pub struct LinkScore {
    pub cosine_similarity: f32,
    pub jaccard_similarity: f32,
    pub exfat_similarity: f32,
    pub total_score: f32,
}

pub struct FragmentLinker {
    pub cosine_weight: f32,
    pub cosine_threshold: f32,
    pub jaccard_weight: f32,
    pub jaccard_threshold: f32,
    pub exfat_weight: f32,
    pub exfat_threshold: f32,
}

impl Default for FragmentLinker {
    fn default() -> Self {
        Self {
            cosine_weight: 0.55,
            cosine_threshold: 0.92,
            jaccard_weight: 0.25,
            jaccard_threshold: 0.3,
            exfat_weight: 0.20,
            exfat_threshold: 1.0,
        }
    }
}

impl FragmentLinker {
    pub fn score(&self, left: &FragmentDescriptor, right: &FragmentDescriptor) -> LinkScore {
        let cosine = SmartSeparation::cosine_similarity(&left.byte_frequency, &right.byte_frequency);
        let jaccard = jaccard_similarity(&left.links, &right.links);
        let exfat = match (&left.exfat_metadata, &right.exfat_metadata) {
            (Some(left_meta), Some(right_meta)) => left_meta.match_score(right_meta),
            _ => 0.0,
        };

        let mut total = 0.0;
        if cosine >= self.cosine_threshold {
            total += cosine * self.cosine_weight;
        }
        if jaccard >= self.jaccard_threshold {
            total += jaccard * self.jaccard_weight;
        }
        if exfat >= self.exfat_threshold {
            total += exfat * self.exfat_weight;
        }

        LinkScore {
            cosine_similarity: cosine,
            jaccard_similarity: jaccard,
            exfat_similarity: exfat,
            total_score: total,
        }
    }
}

fn jaccard_similarity(left: &HashSet<String>, right: &HashSet<String>) -> f32 {
    if left.is_empty() && right.is_empty() {
        return 0.0;
    }

    let intersection = left.intersection(right).count() as f32;
    let union = left.union(right).count() as f32;
    if union == 0.0 {
        0.0
    } else {
        intersection / union
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(left: f32, right: f32) -> bool {
        (left - right).abs() < 1e-4
    }

    #[test]
    fn test_fragment_linker_scores_components() {
        let left = FragmentDescriptor::new(b"aaaaaa").with_links(vec!["a".to_string(), "b".to_string()]);
        let right = FragmentDescriptor::new(b"aaaaaa").with_links(vec!["b".to_string(), "c".to_string()]);

        let left = left.with_exfat_metadata(ExFatMetadata {
            filename: Some("clip.txt".to_string()),
            first_cluster: Some(2),
            size: Some(100),
        });
        let right = right.with_exfat_metadata(ExFatMetadata {
            filename: Some("clip.txt".to_string()),
            first_cluster: Some(3),
            size: Some(200),
        });

        let linker = FragmentLinker::default();
        let score = linker.score(&left, &right);

        assert!(approx_eq(score.cosine_similarity, 1.0));
        assert!(score.jaccard_similarity > 0.3);
        assert!(approx_eq(score.exfat_similarity, 1.0));
        let expected_total = 0.55 + (1.0 / 3.0) * 0.25 + 0.20;
        assert!(approx_eq(score.total_score, expected_total));
    }

    #[test]
    fn test_fragment_linker_respects_thresholds() {
        let left = FragmentDescriptor::new(b"aaaaaa");
        let right = FragmentDescriptor::new(b"bbbbbb");
        let linker = FragmentLinker::default();
        let score = linker.score(&left, &right);

        assert!(approx_eq(score.cosine_similarity, 0.0));
        assert!(approx_eq(score.total_score, 0.0));
    }

    #[test]
    fn test_jaccard_similarity() {
        let left: HashSet<String> = ["x".to_string(), "y".to_string()].into_iter().collect();
        let right: HashSet<String> = ["y".to_string(), "z".to_string()].into_iter().collect();
        let similarity = jaccard_similarity(&left, &right);
        assert!(approx_eq(similarity, 1.0 / 3.0));
    }
}
