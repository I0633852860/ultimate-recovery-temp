use crate::disk::DiskImage;
use crate::error::Result;
use crate::numa::{NumaTopology, pin_thread_to_cpu};
use crate::types_aligned::{HotFragmentAligned, ScanStatsAligned};
use crate::simd_block_scanner_asm::{scan_block_avx2_asm, AlignedBlock};
use crate::types::{
    EnrichedLink, HotFragment, ScanConfig, ScanProgress, ScanResult, Offset,
};
use crate::matcher::{EnhancedMatcher, calculate_fragment_score};
use rayon::prelude::*;
use std::collections::HashMap;
use std::time::Instant;
use std::arch::x86_64::{_mm_prefetch, _MM_HINT_T0};
use tokio::sync::mpsc::Sender;

/// Information about a chunk to be scanned
#[derive(Debug, Clone)]
pub struct ChunkInfo {
    pub offset: u64,
    pub size: usize,
}

/// Parallel file scanner with SIMD-accelerated pattern matching
#[derive(Clone)]
pub struct ParallelScanner {
    config: ScanConfig,
    enhanced_matcher: EnhancedMatcher,
}

/// Адаптивный prefetch на основе паттернов доступа
#[derive(Debug, Clone)]
pub struct AdaptivePrefetcher {
    last_access: usize,
    stride: usize,
    confidence: f32,
}

impl AdaptivePrefetcher {
    pub fn new() -> Self {
        Self {
            last_access: 0,
            stride: 64,
            confidence: 0.0,
        }
    }
    
    pub fn record_access(&mut self, offset: usize) {
        let current_stride = offset.saturating_sub(self.last_access);
        
        if current_stride == self.stride {
            // Паттерн подтвердился
            self.confidence = (self.confidence + 0.1).min(1.0);
        } else {
            // Паттерн изменился
            self.stride = current_stride;
            self.confidence = 0.0;
        }
        
        self.last_access = offset;
    }
    
    pub unsafe fn prefetch_next(&self, current_ptr: *const u8) {
        if self.confidence > 0.5 {
            let next_ptr = current_ptr.add(self.stride);
            _mm_prefetch(next_ptr as *const i8, _MM_HINT_T0);
        }
    }
}

impl ParallelScanner {
    pub fn new(config: ScanConfig) -> Self {
        // Detect NUMA topology
        let numa_topology = NumaTopology::detect();

        if let Some(ref topo) = numa_topology {
            // Configure NUMA-aware thread pool
            let thread_count = if config.num_threads > 0 {
                config.num_threads
            } else {
                topo.total_cores
            };

            let _ = rayon::ThreadPoolBuilder::new()
                .num_threads(thread_count)
                .start_handler(move |thread_id| {
                    if let Some(ref topo) = NumaTopology::detect() {
                        // Pin thread to CPU core
                        let cpu = topo.nodes
                            .iter()
                            .flat_map(|n| &n.cpu_cores)
                            .nth(thread_id)
                            .copied()
                            .unwrap_or(thread_id);
                        
                        let _ = pin_thread_to_cpu(cpu);
                    }
                })
                .build_global();
        } else if config.num_threads > 0 {
            let _ = rayon::ThreadPoolBuilder::new()
                .num_threads(config.num_threads)
                .build_global();
        }

        let enhanced_matcher = EnhancedMatcher::new();

        Self { config, enhanced_matcher }
    }

    /// Public async scan method
    pub async fn scan(&self, disk: &DiskImage, sender: Sender<ScanProgress>) -> Result<ScanResult> {
        let scanner = self.clone();
        let disk = disk.clone();
        
        tokio::task::spawn_blocking(move || {
            let start_offset = Offset::new(0);
            scanner.scan_streaming(&disk, start_offset, scanner.config.reverse, Some(sender))
        })
        .await
        .map_err(|e| crate::error::RecoveryError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?
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

        let numa_topology = NumaTopology::detect();
        let mut chunks = Vec::new();
        
        if let Some(ref topo) = numa_topology {
            // NUMA-aware distribution
            let base_chunks = self.create_chunks(data, start_offset);
            let distribution = topo.distribute_chunks(base_chunks.len());
            
            for (_node_id, chunk_ids) in distribution {
                for id in chunk_ids {
                    if let Some(chunk) = base_chunks.get(id) {
                        chunks.push(chunk.clone());
                    }
                }
            }
        } else {
            chunks = self.create_chunks(data, start_offset);
        }

        if reverse {
            chunks.reverse();
        }

        let stats = ScanStatsAligned::new();
        let _total_chunks = chunks.len();
        let config = &self.config;
        let sender_clone = sender;
        let matcher = &self.enhanced_matcher;

        // Parallel scan with panic isolation and stats tracking
        let all_links: Vec<Vec<EnrichedLink>> = chunks
            .par_iter()
            .enumerate()
            .filter_map(|(_i, chunk_info)| {
                let chunk_start = (chunk_info.offset - start_offset) as usize;
                let chunk_end = chunk_start + chunk_info.size;
                let chunk_data = &data[chunk_start..chunk_end];

                stats.add_chunk();

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
        let mut json_markers = 0;
        let mut cyrillic_count = 0;
        let mut prefetcher = AdaptivePrefetcher::new();

        // Use enhanced matcher for YouTube links
        let links: Vec<EnrichedLink> = matcher.scan_chunk(chunk_data, offset as usize, self.config.deduplicate);
        let youtube_count = links.len();

        // Optimized block scan with prefetching
        let block_size = 64; // Use 64 bytes for cache line alignment
        let mut is_empty = true;
        let mut has_metadata = false;

        let mut i = 0;
        while i + block_size <= chunk_data.len() {
            // Adaptive software prefetching
            unsafe {
                prefetcher.record_access(i);
                prefetcher.prefetch_next(chunk_data.as_ptr().add(i));
            }

            // SIMD block scan (AVX2 ASM optimized)
            unsafe {
                let block_ptr = chunk_data.as_ptr().add(i) as *const AlignedBlock;
                if is_x86_feature_detected!("avx2") {
                    let res = scan_block_avx2_asm(&*block_ptr);
                    if !res.is_empty {
                        is_empty = false;
                    }
                    if res.has_metadata {
                        has_metadata = true;
                    }
                    
                    if res.hot_mask_low != 0 || res.hot_mask_high != 0 {
                        json_markers += (res.hot_mask_low.count_ones() + res.hot_mask_high.count_ones()) as usize;
                    }
                }
            }

            i += block_size;
        }

        // Processing remainder
        for &b in &chunk_data[i..] {
            if b != 0 { is_empty = false; }
            if b == b'{' || b == b'}' || b == b'[' || b == b']' { json_markers += 1; }
            if b >= 0xD0 && b <= 0xDF { cyrillic_count += 1; }
        }

        let cyrillic_density = if chunk_data.is_empty() { 0.0 } else { cyrillic_count as f32 / chunk_data.len() as f32 };
        let fragment_score = calculate_fragment_score(chunk_data, youtube_count, cyrillic_density, json_markers);
        let target_score = fragment_score.overall_score;

        // Create hot fragment if promising using Aligned version internally
        let hot_fragment = if target_score > 20.0 && !is_empty {
            let file_type = self.guess_file_type_fast(chunk_data);
            let mut aligned = HotFragmentAligned::new(offset, chunk_data.len() as u64);
            
            aligned.youtube_count = youtube_count as u32;
            aligned.cyrillic_density = cyrillic_density;
            aligned.json_markers = json_markers as u32;
            aligned.has_valid_json = fragment_score.is_valid_json;
            aligned.target_score = target_score;
            aligned.entropy = crate::entropy::calculate_shannon_entropy(chunk_data);
            aligned.has_metadata = has_metadata;
            
            // Convert to standard HotFragment for compatibility with existing Result types
            let mut fragment = HotFragment::new(aligned.offset, aligned.size as usize);
            fragment.youtube_count = aligned.youtube_count as usize;
            fragment.cyrillic_density = aligned.cyrillic_density;
            fragment.json_markers = aligned.json_markers as usize;
            fragment.has_valid_json = aligned.has_valid_json;
            fragment.target_score = aligned.target_score;
            fragment.file_type_guess = file_type;
            fragment.entropy = aligned.entropy;
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
