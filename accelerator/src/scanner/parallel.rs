use crate::matcher::EnhancedMatcher;
use crate::types::{EnrichedLink, ScanConfig, ScanResult};
use anyhow::{Context, Result};
use memmap2::MmapOptions;
use rayon::prelude::*;
use std::fs::File;
use std::path::Path;
use std::time::Instant;
use std::collections::HashMap;

/// Parallel file scanner with pre-compiled regex patterns
pub struct ParallelScanner {
    config: ScanConfig,
    /// Pre-compiled matcher template (cloned for each thread)
    matcher_template: EnhancedMatcher,
}

impl ParallelScanner {
    pub fn new(config: ScanConfig) -> Self {
        // Configure global thread pool if requested
        if config.num_threads > 0 {
            let _ = rayon::ThreadPoolBuilder::new()
                .num_threads(config.num_threads)
                .build_global();
        }
        
        // Pre-compile matcher once (expensive)
        let matcher_template = EnhancedMatcher::new();
        
        Self { config, matcher_template }
    }
    
    /// Scan a file path with progress callback
    pub fn scan_file<F>(&self, path: &Path, progress_cb: Option<&F>) -> Result<ScanResult> 
    where 
        F: Fn(usize) + Sync + Send
    {

        let start_time = Instant::now();
        
        // Open file
        let file = File::open(path)
            .with_context(|| format!("Failed to open file: {:?}", path))?;
            
        let metadata = file.metadata()?;
        let file_size = metadata.len() as usize;
        
        if file_size == 0 {
            return Ok(ScanResult::default());
        }
        
        // Memory-map file
        // unsafe because file could be modified by other processes while mapped
        let mmap = unsafe {
            MmapOptions::new()
                .map(&file)
                .context("Failed to memory-map file")?
        };
        
        // Create chunks
        let chunks = self.create_chunks(&mmap, 0);
        
        // Parallel scan with pre-compiled matcher (cloned per thread)
        // Uses catch_unwind for crash isolation on corrupted data (v6.1 Forensic)
        let matcher_template = &self.matcher_template;
        let all_links: Vec<Vec<EnrichedLink>> = chunks
            .par_iter()
            .filter_map(|(chunk_data, offset)| {
                // Report progress
                if let Some(cb) = progress_cb {
                    cb(chunk_data.len());
                }
                
                // Isolate panics from corrupted data using catch_unwind
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    // Clone pre-compiled matcher (cheap - only clones Arc pointer)
                    let mut matcher = matcher_template.clone_fresh();
                    matcher.scan_chunk(
                        chunk_data,
                        *offset,
                        self.config.deduplicate,
                    )
                }));
                
                match result {
                    Ok(links) => Some(links),
                    Err(_) => {
                        // Corrupted sector - skip silently (forensic: log offset)
                        eprintln!("[WARN] Corrupted sector at offset 0x{:X}, skipping", offset);
                        Some(Vec::new())
                    }
                }
            })
            .collect();
            
        // Flatten results
        let mut links: Vec<EnrichedLink> = all_links
            .into_iter()
            .flatten()
            .collect();
            
        // Global deduplication and merging
        if self.config.deduplicate {
            self.deduplicate_links(&mut links);
        }
        
        // Filter by confidence
        if self.config.min_confidence > 0.0 {
            links.retain(|l| l.confidence >= self.config.min_confidence);
        }
        
        // Sort by offset
        links.sort_by_key(|l| l.offset);
        
        let duration = start_time.elapsed();
        
        Ok(ScanResult {
            links,
            bytes_scanned: file_size,
            duration_secs: duration.as_secs_f64(),
        })
    }
    
    /// Create overlapping chunks from data
    fn create_chunks<'a>(
        &self,
        data: &'a [u8],
        start_offset: usize,
    ) -> Vec<(&'a [u8], usize)> {
        let chunk_size = self.config.chunk_size;
        let overlap = self.config.overlap_size;
        
        let mut chunks = Vec::new();
        let mut offset = start_offset;
        
        while offset < data.len() {
            let chunk_end = offset
                .saturating_add(chunk_size)
                .saturating_add(overlap)
                .min(data.len());
            
            // Ensure we don't create empty chunks at the very end
            if offset < chunk_end {
                let chunk_data = &data[offset..chunk_end];
                chunks.push((chunk_data, offset));
            }
            
            // Advance by chunk_size
            offset = offset.saturating_add(chunk_size);
            if chunk_size == 0 { break; } // Prevent infinite loop
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
        // 1. Prefer title over no title
        if new.title.is_some() && existing.title.is_none() {
            return true;
        }
        if new.title.is_none() && existing.title.is_some() {
            return false;
        }
        
        // 2. Prefer higher confidence
        if new.confidence > existing.confidence {
            return true;
        }
        
        false
    }
    
    // ═══════════════════════════════════════════════════════════════════════════════
    // STREAMING SCAN v6.0 - Immediate hot fragment callback
    // ═══════════════════════════════════════════════════════════════════════════════
    
    /// Scan a file with streaming results and reverse support
    /// Calls hot_fragment_cb immediately when a promising chunk is found
    pub fn scan_file_streaming<F, H>(
        &self, 
        path: &Path, 
        start_offset: usize,
        reverse: bool,
        progress_cb: Option<&F>,
        hot_fragment_cb: Option<&H>,
    ) -> Result<ScanResult>
    where
        F: Fn(usize) + Sync + Send,
        H: Fn(crate::types::HotFragment) + Sync + Send,
    {
        use crate::matcher::{detect_cyrillic, cyrillic_density, calculate_target_score};
        use crate::types::HotFragment;
        
        let start_time = Instant::now();
        
        let file = File::open(path)
            .with_context(|| format!("Failed to open file: {:?}", path))?;
        
        let metadata = file.metadata()?;
        let file_size = metadata.len() as usize;
        
        if file_size == 0 {
            return Ok(ScanResult::default());
        }
        
        let mmap = unsafe {
            MmapOptions::new()
                .map(&file)
                .context("Failed to memory-map file")?
        };
        
        // Create chunks with optional reverse order
        let mut chunks = self.create_chunks(&mmap, start_offset);
        if reverse {
            chunks.reverse();
        }
        
        eprintln!("[RUST DEBUG] Created {} chunks. File size: {} bytes. Start offset: {}", chunks.len(), file_size, start_offset);

        let matcher_template = &self.matcher_template;
        
        // Parallel scan with streaming callback + catch_unwind (v5.0 forensic safety)
        let all_links: Vec<Vec<EnrichedLink>> = chunks
            .par_iter()
            .enumerate()
            .filter_map(|(i, (chunk_data, offset))| {
                // Debug log for every 100th chunk
                if i % 100 == 0 {
                     eprintln!("[RUST DEBUG] Processing chunk {} at offset 0x{:X}", i, offset);
                }

                // Report progress
                if let Some(cb) = progress_cb {
                    cb(chunk_data.len());
                }

                // Isolate panics from corrupted data using catch_unwind
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    // Scan chunk
                    let mut matcher = matcher_template.clone_fresh();
                    let links = matcher.scan_chunk(
                        chunk_data,
                        *offset,
                        self.config.deduplicate,
                    );
                    
                    // If we found links and have a hot fragment callback, create and report it
                    if !links.is_empty() {
                        if let Some(hot_cb) = hot_fragment_cb {
                            let has_cyrillic = detect_cyrillic(chunk_data);
                            let cyr_density = if has_cyrillic { 
                                cyrillic_density(chunk_data) 
                            } else { 
                                0.0 
                            };
                            
                            // Count JSON markers
                            let json_markers = chunk_data.iter()
                                .filter(|&&b| b == b'{' || b == b'}' || b == b'[' || b == b']')
                                .count();
                            
                            let target_score = calculate_target_score(
                                links.len(),
                                cyr_density,
                                json_markers > 10,
                                chunk_data.len(),
                            );
                            
                            // Only report if score is promising
                            if target_score > 10.0 {
                                let mut fragment = HotFragment::new(*offset as u64, chunk_data.len());
                                fragment.youtube_count = links.len();
                                fragment.cyrillic_density = cyr_density;
                                fragment.json_markers = json_markers;
                                fragment.target_score = target_score;
                                fragment.file_type_guess = guess_file_type_fast(chunk_data);
                                
                                hot_cb(fragment);
                            }
                        }
                    }
                    
                    links
                }));

                match result {
                    Ok(links) => Some(links),
                    Err(_) => {
                        eprintln!("[WARN] Corrupted sector at offset 0x{:X}, skipping", offset);
                        Some(Vec::new())
                    }
                }
            })
            .collect();
        
        // Flatten and deduplicate
        let mut links: Vec<EnrichedLink> = all_links.into_iter().flatten().collect();
        
        if self.config.deduplicate {
            self.deduplicate_links(&mut links);
        }
        
        if self.config.min_confidence > 0.0 {
            links.retain(|l| l.confidence >= self.config.min_confidence);
        }
        
        links.sort_by_key(|l| l.offset);
        
        let duration = start_time.elapsed();
        
        Ok(ScanResult {
            links,
            bytes_scanned: file_size,
            duration_secs: duration.as_secs_f64(),
        })
    }
}

/// Fast file type guessing based on content
fn guess_file_type_fast(data: &[u8]) -> String {
    if let Some(&first) = data.first() {
        if first == b'{' || first == b'[' {
            return "json".to_string();
        }
        if first == b'<' {
            return "html".to_string();
        }
    }
    
    // Check for URL patterns (text file)
    if data.windows(4).any(|w| w == b"http") {
        return "txt".to_string();
    }
    
    "unknown".to_string()
}
