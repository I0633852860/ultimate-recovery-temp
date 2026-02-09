use crate::disk::DiskImage;
use crate::error::Result;
use crate::simd_search::{find_pattern_simd, scan_block_simd};
use crate::types::{
    EnrichedLink, HotFragment, ScanConfig, ScanProgress, ScanResult, Offset,
};
use crate::matcher::{EnhancedMatcher, calculate_fragment_score, validate_data_chunk};
use crate::entropy::{calculate_shannon_entropy, get_entropy_category};
use rayon::prelude::*;
use std::collections::HashMap;
use std::time::Instant;
use tokio::sync::mpsc::Sender;

/// Information about a chunk to be scanned
#[derive(Debug, Clone)]
pub struct ChunkInfo {
    pub offset: u64,
    pub size: usize,
}

/// Parallel file scanner with SIMD-accelerated pattern matching
pub struct ParallelScanner {
    config: ScanConfig,
    enhanced_matcher: EnhancedMatcher,
}

impl ParallelScanner {
    pub fn new(config: ScanConfig) -> Self {
        // Configure global thread pool if requested
        if config.num_threads > 0 {
            let _ = rayon::ThreadPoolBuilder::new()
                .num_threads(config.num_threads)
                .build_global();
        }

        let enhanced_matcher = EnhancedMatcher::new();

        Self { config, enhanced_matcher }
    }

    pub fn with_matcher(config: ScanConfig, matcher: EnhancedMatcher) -> Self {
        if config.num_threads > 0 {
            let _ = rayon::ThreadPoolBuilder::new()
                .num_threads(config.num_threads)
                .build_global();
        }

        Self { config, enhanced_matcher: matcher }
    }

    /// Scan a disk image with progress updates via tokio channel
    pub fn scan_streaming(
        &self,
        disk: &DiskImage,
        start: Offset,
        reverse: bool,
        sender: Option<Sender<ScanProgress>>,
    ) -> Result<ScanResult> {
        let start_time = Instant::now();

        let disk_size = disk.size().as_u64();
        let start_offset = start.as_u64();

        if disk_size == 0 || start_offset >= disk_size {
            return Ok(ScanResult::default());
        }

        let mmap = disk.get_mmap();
        let data = &mmap[start_offset as usize..];

        // Create chunks
        let mut chunks = self.create_chunks(data, start_offset);
        if reverse {
            chunks.reverse();
        }

        let total_chunks = chunks.len();
        let config = &self.config;
        let sender_clone = sender;
        let matcher = &self.enhanced_matcher;

        // Parallel scan with panic isolation
        let all_links: Vec<Vec<EnrichedLink>> = chunks
            .par_iter()
            .enumerate()
            .filter_map(|(i, chunk_info)| {
                let chunk_start = chunk_info.offset as usize;
                let chunk_end = chunk_start + chunk_info.size;
                let chunk_data = &data[chunk_start..chunk_end];

                // Report progress
                if let Some(ref s) = sender_clone {
                    if !s.is_closed() {
                        let _ = s.blocking_send(ScanProgress::ChunkCompleted(chunk_info.offset));
                        let _ = s.blocking_send(ScanProgress::BytesScanned(chunk_info.size as u64));
                    }
                }

                // Isolate panics with catch_unwind
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    self.scan_chunk_with_matcher(chunk_data, chunk_info.offset, matcher.clone_fresh())
                }));

                match result {
                    Ok((links, hot_fragment)) => {
                        // Send hot fragment if found
                        if let Some(ref fragment) = hot_fragment {
                            if let Some(ref s) = sender_clone {
                                if !s.is_closed() {
                                    let _ = s.blocking_send(ScanProgress::HotFragment(fragment.clone()));
                                }
                            }
                        }
                        Some(links)
                    }
                    Err(_) => {
                        eprintln!(
                            "[WARN] Corrupted sector at offset 0x{:X}, skipping",
                            chunk_info.offset
                        );
                        if let Some(ref s) = sender_clone {
                            if !s.is_closed() {
                                let _ = s.blocking_send(ScanProgress::ChunkError(
                                    chunk_info.offset,
                                    "Panic in chunk processing".to_string(),
                                ));
                            }
                        }
                        Some(Vec::new())
                    }
                }
            })
            .collect();

        // Flatten results
        let mut links: Vec<EnrichedLink> = all_links.into_iter().flatten().collect();

        // Global deduplication and filtering
        if config.deduplicate {
            self.deduplicate_links(&mut links);
        }

        if config.min_confidence > 0.0 {
            links.retain(|l| l.confidence >= config.min_confidence);
        }

        links.sort_by_key(|l| l.offset);

        let bytes_scanned = (data.len() as u64).min(disk_size - start_offset);
        let duration = start_time.elapsed();

        Ok(ScanResult {
            links,
            bytes_scanned,
            duration_secs: duration.as_secs_f64(),
        })
    }

    /// Scan a single chunk with enhanced matcher and return (links, optional hot_fragment)
    fn scan_chunk_with_matcher(
        &self,
        chunk_data: &[u8],
        offset: u64,
        mut matcher: EnhancedMatcher,
    ) -> (Vec<EnrichedLink>, Option<HotFragment>) {
        let mut links = Vec::new();
        let mut json_markers = 0;
        let mut cyrillic_count = 0;

        // Use enhanced matcher for YouTube links
        links = matcher.scan_chunk(chunk_data, offset as usize, self.config.deduplicate);
        let youtube_count = links.len();

        // Fast block scan for hot fragment detection
        let block_size = 32;
        let mut hot_mask_accum = 0u32;
        let mut has_metadata = false;
        let mut is_empty = true;

        for i in (0..chunk_data.len()).step_by(block_size) {
            let block_end = (i + block_size).min(chunk_data.len());
            let block = &chunk_data[i..block_end];

            if block.len() < 32 {
                // Process partial block
                for &b in block {
                    if b != 0 {
                        is_empty = false;
                    }
                    if b == b'{' || b == b'}' || b == b'[' || b == b']' {
                        json_markers += 1;
                    }
                    if b >= 0xD0 && b <= 0xDF {
                        cyrillic_count += 1;
                    }
                }
            } else {
                let res = scan_block_simd(block);
                if !res.is_empty {
                    is_empty = false;
                }
                if res.has_metadata {
                    has_metadata = true;
                }
                hot_mask_accum |= res.hot_mask;

                // Count JSON markers in this block
                json_markers += block
                    .iter()
                    .filter(|&&b| b == b'{' || b == b'}' || b == b'[' || b == b']')
                    .count();
            }
        }

        // Calculate cyrillic density
        let cyrillic_density = if chunk_data.is_empty() {
            0.0
        } else {
            cyrillic_count as f32 / chunk_data.len() as f32
        };

        // Calculate target score using new enhanced scoring
        let fragment_score = calculate_fragment_score(chunk_data, youtube_count, cyrillic_density, json_markers);
        let target_score = fragment_score.overall_score;

        // Calculate entropy for the chunk
        let entropy = calculate_shannon_entropy(chunk_data);
        let entropy_category = get_entropy_category(chunk_data);

        // Create hot fragment if promising
        let hot_fragment = if target_score > 20.0 && !is_empty {
            let file_type = self.guess_file_type_fast(chunk_data);

            let mut fragment = HotFragment::new(offset, chunk_data.len());
            fragment.youtube_count = youtube_count;
            fragment.cyrillic_density = cyrillic_density;
            fragment.json_markers = json_markers;
            fragment.has_valid_json = fragment_score.is_valid_json;
            fragment.target_score = target_score;
            fragment.file_type_guess = file_type;
            fragment.entropy = entropy;
            fragment.entropy_category = entropy_category.to_string();
            fragment.fragment_score = fragment_score;

            Some(fragment)
        } else {
            None
        };

        (links, hot_fragment)
    }

    /// Legacy scan_chunk method (kept for compatibility)
    fn scan_chunk(
        &self,
        chunk_data: &[u8],
        offset: u64,
        _patterns: &[Vec<u8>],
    ) -> (Vec<EnrichedLink>, Option<HotFragment>) {
        // Delegate to new method with a fresh matcher
        self.scan_chunk_with_matcher(chunk_data, offset, self.enhanced_matcher.clone_fresh())
    }

    /// Create aligned chunks from data
    fn create_chunks(&self, data: &[u8], start_offset: u64) -> Vec<ChunkInfo> {
        let chunk_size = self.config.chunk_size;
        let overlap = self.config.overlap_size;

        let mut chunks = Vec::new();
        let mut offset = 0usize;

        while offset < data.len() {
            let chunk_end = offset
                .saturating_add(chunk_size)
                .saturating_add(overlap)
                .min(data.len());

            if offset < chunk_end {
                chunks.push(ChunkInfo {
                    offset: start_offset + offset as u64,
                    size: chunk_end - offset,
                });
            }

            offset = offset.saturating_add(chunk_size);
            if chunk_size == 0 {
                break;
            }
        }

        chunks
    }

    /// Deduplicate links, keeping the best version of each
    fn deduplicate_links(&self, links: &mut Vec<EnrichedLink>) {
        let mut best_links: HashMap<String, EnrichedLink> = HashMap::new();

        for link in links.drain(..) {
            let video_id = link.video_id.clone();

            best_links
                .entry(video_id)
                .and_modify(|existing| {
                    if Self::is_better_link(&link, existing) {
                        *existing = link.clone();
                    }
                })
                .or_insert(link);
        }

        links.extend(best_links.into_values());
    }

    /// Check if new link is "better" than existing one
    fn is_better_link(new: &EnrichedLink, existing: &EnrichedLink) -> bool {
        if new.title.is_some() && existing.title.is_none() {
            return true;
        }
        if new.title.is_none() && existing.title.is_some() {
            return false;
        }

        if new.confidence > existing.confidence {
            return true;
        }

        false
    }

    /// Calculate target score for hot fragment detection
    fn calculate_target_score(&self, youtube_count: usize, cyrillic_density: f32, json_markers: usize) -> f32 {
        let youtube_score = youtube_count as f32 * 10.0;
        let cyrillic_score = cyrillic_density * 100.0;
        let json_score = (json_markers / 10) as f32;

        youtube_score + cyrillic_score + json_score
    }

    /// Fast file type guessing based on content
    fn guess_file_type_fast(&self, data: &[u8]) -> String {
        if let Some(&first) = data.first() {
            if first == b'{' || first == b'[' {
                return "json".to_string();
            }
            if first == b'<' {
                return "html".to_string();
            }
        }

        if data.windows(4).any(|w| w == b"http") {
            return "txt".to_string();
        }

        "unknown".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_creation() {
        let config = ScanConfig::new(1024, 64, 0);
        let scanner = ParallelScanner::new(config);

        let data = vec![0u8; 5000];
        let chunks = scanner.create_chunks(&data, 0);

        assert!(!chunks.is_empty());
        assert!(chunks[0].offset == 0);
        assert!(chunks[0].size > 0);
    }

    #[test]
    fn test_chunk_alignment() {
        let config = ScanConfig::new(100, 64, 0);
        let scanner = ParallelScanner::new(config);

        // Chunk size should be aligned to 64 bytes
        assert_eq!(scanner.config.chunk_size % 64, 0);
    }
}
