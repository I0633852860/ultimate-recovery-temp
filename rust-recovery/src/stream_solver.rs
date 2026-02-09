use std::collections::HashSet;

use crate::types::{AssembledStream, StreamFragment, StreamScoringWeights};

#[derive(Debug)]
struct PathResult {
    indices: Vec<usize>,
    edge_scores: Vec<f32>,
    total_score: f32,
}

pub fn assemble_streams(fragments: &[StreamFragment]) -> Vec<AssembledStream> {
    assemble_streams_with_weights(fragments, &StreamScoringWeights::default(), None)
}

pub fn assemble_streams_with_weights(
    fragments: &[StreamFragment],
    weights: &StreamScoringWeights,
    max_streams: Option<usize>,
) -> Vec<AssembledStream> {
    if fragments.is_empty() {
        return Vec::new();
    }

    let mut remaining: Vec<StreamFragment> = fragments.to_vec();
    let mut streams = Vec::new();
    let limit = max_streams.unwrap_or(3).max(1);

    while !remaining.is_empty() && streams.len() < limit {
        remaining.sort_by_key(|fragment| fragment.offset);
        let link_sets: Vec<HashSet<String>> = remaining
            .iter()
            .map(|fragment| fragment.links.iter().cloned().collect())
            .collect();

        let path = match find_best_path(&remaining, weights, &link_sets) {
            Some(path) => path,
            None => break,
        };

        if path.indices.is_empty() {
            break;
        }

        let stream = build_stream(&path, &remaining);
        streams.push(stream);

        let used: HashSet<usize> = path.indices.iter().cloned().collect();
        remaining = remaining
            .into_iter()
            .enumerate()
            .filter_map(|(idx, fragment)| if used.contains(&idx) { None } else { Some(fragment) })
            .collect();
    }

    streams
}

fn find_best_path(
    fragments: &[StreamFragment],
    weights: &StreamScoringWeights,
    link_sets: &[HashSet<String>],
) -> Option<PathResult> {
    let count = fragments.len();
    if count == 0 {
        return None;
    }

    let mut best_score = vec![0.0; count];
    let mut previous = vec![None; count];
    let mut edge_score_to = vec![0.0; count];

    for i in 0..count {
        let node_score = fragments[i].total_score();
        best_score[i] = node_score;
        let mut looked_back = 0usize;

        for j in (0..i).rev() {
            if looked_back >= weights.max_lookback {
                break;
            }
            looked_back += 1;

            if fragments[i].offset >= fragments[j].end_offset() {
                let gap = fragments[i].offset - fragments[j].end_offset();
                if gap > weights.max_gap {
                    break;
                }
            }

            if let Some(edge_score) = edge_score(
                &fragments[j],
                &fragments[i],
                weights,
                &link_sets[j],
                &link_sets[i],
            ) {
                let candidate = best_score[j] + edge_score + node_score;
                if candidate > best_score[i] {
                    best_score[i] = candidate;
                    previous[i] = Some(j);
                    edge_score_to[i] = edge_score;
                }
            }
        }
    }

    let (best_index, &total_score) = best_score
        .iter()
        .enumerate()
        .max_by(|(_, left), (_, right)| left.partial_cmp(right).unwrap())?;

    let mut indices_rev = Vec::new();
    let mut edge_scores_rev = Vec::new();
    let mut current = Some(best_index);
    while let Some(idx) = current {
        indices_rev.push(idx);
        if let Some(prev_idx) = previous[idx] {
            edge_scores_rev.push(edge_score_to[idx]);
            current = Some(prev_idx);
        } else {
            current = None;
        }
    }

    indices_rev.reverse();
    edge_scores_rev.reverse();

    Some(PathResult {
        indices: indices_rev,
        edge_scores: edge_scores_rev,
        total_score,
    })
}

fn edge_score(
    left: &StreamFragment,
    right: &StreamFragment,
    weights: &StreamScoringWeights,
    left_links: &HashSet<String>,
    right_links: &HashSet<String>,
) -> Option<f32> {
    let left_end = left.end_offset();
    let right_start = right.offset;
    let (gap, overlap) = if right_start >= left_end {
        (right_start - left_end, 0)
    } else {
        (0, left_end - right_start)
    };

    if gap > weights.max_gap || overlap > weights.max_overlap {
        return None;
    }

    let mut score = 0.0;
    if weights.max_gap > 0 {
        score -= weights.gap_penalty * (gap as f32 / weights.max_gap as f32);
    }
    if weights.max_overlap > 0 {
        score -= weights.overlap_penalty * (overlap as f32 / weights.max_overlap as f32);
    }

    if left.file_type == right.file_type {
        score += weights.type_match_bonus;
    } else {
        score -= weights.type_mismatch_penalty;
    }

    let cosine = left.feature_vector.cosine_similarity(&right.feature_vector);
    let jaccard = jaccard_similarity(left_links, right_links);
    score += cosine * weights.cosine_weight;
    score += jaccard * weights.jaccard_weight;

    if left.has_valid_structure() && right.has_valid_structure() {
        score += weights.structure_bonus;
    }

    if score < weights.min_edge_score {
        None
    } else {
        Some(score)
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

fn build_stream(path: &PathResult, fragments: &[StreamFragment]) -> AssembledStream {
    let selected: Vec<StreamFragment> = path
        .indices
        .iter()
        .map(|&idx| fragments[idx].clone())
        .collect();
    let average_edge = if path.edge_scores.is_empty() {
        0.0
    } else {
        path.edge_scores.iter().sum::<f32>() / path.edge_scores.len() as f32
    };
    let confidence = if selected.is_empty() {
        0.0
    } else {
        (path.total_score / selected.len() as f32).max(0.0)
    };

    AssembledStream {
        fragments: selected,
        confidence,
        total_score: path.total_score,
        reasons: vec![
            format!("fragments={}", path.indices.len()),
            format!("avg_edge_score={:.2}", average_edge),
            format!("path_score={:.2}", path.total_score),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{FragmentScore, StreamFragment, StreamScoringWeights};

    fn make_fragment(offset: u64, data: &[u8], file_type: &str) -> StreamFragment {
        StreamFragment::from_bytes(
            offset,
            data,
            file_type,
            10.0,
            FragmentScore {
                overall_score: 40.0,
                is_valid_json: file_type == "json",
                is_valid_html: file_type == "html",
                is_valid_csv: false,
                is_valid_youtube_url: false,
                has_structured_text: true,
                is_compressed: false,
                reasons: Vec::new(),
            },
        )
    }

    #[test]
    fn test_stream_solver_separates_interleaved_streams() {
        let fragments = vec![
            make_fragment(0, b"aaaaaaaa", "json"),
            make_fragment(50, b"zzzzzzzz", "html"),
            make_fragment(140, b"aaaaaaab", "json"),
            make_fragment(190, b"zzzzzzzy", "html"),
        ];

        let weights = StreamScoringWeights {
            max_gap: 200,
            max_overlap: 20,
            ..StreamScoringWeights::default()
        };

        let streams = assemble_streams_with_weights(&fragments, &weights, Some(2));
        assert_eq!(streams.len(), 2);

        let total_fragments: usize = streams.iter().map(|stream| stream.fragments.len()).sum();
        assert_eq!(total_fragments, 4);

        for stream in streams {
            let file_type = &stream.fragments[0].file_type;
            assert!(stream.fragments.iter().all(|fragment| &fragment.file_type == file_type));
        }
    }
}
