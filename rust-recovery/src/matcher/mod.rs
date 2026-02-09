pub mod patterns;
pub mod validator;

use crate::matcher::patterns::{YOUTUBE_PATTERNS, TITLE_PATTERNS};
use crate::matcher::validator::{is_valid_video_id, is_valid_json, is_probably_json, is_valid_youtube_url, is_probably_youtube_url};
use crate::types::{EnrichedLink, FragmentScore, ValidationResult};
use crate::entropy::{calculate_shannon_entropy, is_compressed_like, is_structured_text, get_entropy_category};
use ahash::AHashSet;
use regex::bytes::Regex;
use regex::bytes::RegexSet;
use regex::bytes::RegexSetBuilder;
use std::sync::Arc;
use html_escape::decode_html_entities;

// ═══════════════════════════════════════════════════════════════════════════════
// FORENSIC DETECTION v6.1 - SIMD Optimized + SHA-256
// ═══════════════════════════════════════════════════════════════════════════════

use sha2::{Sha256, Digest};

/// Compute SHA-256 hash of data (for forensic evidence integrity)
#[inline]
pub fn sha256_hash(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

/// Fast Cyrillic detection - checks for UTF-8 Cyrillic byte sequences (0xD0, 0xD1)
/// SIMD-optimized: processes 32 bytes at a time when possible
#[inline]
pub fn detect_cyrillic(data: &[u8]) -> bool {
    // Fast path: use SIMD-like unrolled loop for larger buffers
    if data.len() >= 32 {
        // Process 32 bytes at a time (unrolled for auto-vectorization)
        let chunks = data.chunks_exact(32);
        let remainder = chunks.remainder();
        
        for chunk in chunks {
            // Check all 32 bytes - compiler will auto-vectorize this
            if chunk.iter().any(|&b| b == 0xD0 || b == 0xD1) {
                return true;
            }
        }
        
        // Check remainder
        return remainder.iter().any(|&b| b == 0xD0 || b == 0xD1);
    }
    
    // Short path for small buffers
    data.iter().any(|&b| b == 0xD0 || b == 0xD1)
}

/// Count JSON structure markers using SIMD-friendly unrolled loop
/// Returns count of { } [ ] characters
#[inline]
pub fn count_json_markers_fast(data: &[u8]) -> usize {
    let mut count = 0usize;
    
    // Process 32 bytes at a time for auto-vectorization
    if data.len() >= 32 {
        let chunks = data.chunks_exact(32);
        let remainder = chunks.remainder();
        
        for chunk in chunks {
            for &b in chunk {
                if b == b'{' || b == b'}' || b == b'[' || b == b']' {
                    count += 1;
                }
            }
        }
        
        for &b in remainder {
            if b == b'{' || b == b'}' || b == b'[' || b == b']' {
                count += 1;
            }
        }
    } else {
        for &b in data {
            if b == b'{' || b == b'}' || b == b'[' || b == b']' {
                count += 1;
            }
        }
    }
    
    count
}

/// Calculate Cyrillic character density in data
/// Returns a value between 0.0 and 1.0
#[inline]
pub fn cyrillic_density(data: &[u8]) -> f32 {
    if data.is_empty() {
        return 0.0;
    }
    
    // Count UTF-8 Cyrillic character sequences
    // Cyrillic letters in UTF-8 are 2-byte sequences: 0xD0 0x80-0xBF or 0xD1 0x80-0xBF
    let mut cyrillic_chars = 0usize;
    let mut i = 0;
    
    while i < data.len().saturating_sub(1) {
        let b = data[i];
        let next = data[i + 1];
        
        if (b == 0xD0 || b == 0xD1) && (0x80..=0xBF).contains(&next) {
            cyrillic_chars += 1;
            i += 2; // Skip both bytes of the character
        } else {
            i += 1;
        }
    }
    
    // Return density as ratio of Cyrillic chars to total possible chars
    cyrillic_chars as f32 / (data.len() / 2).max(1) as f32
}

/// Calculate YouTube link density (links per megabyte)
#[inline]
pub fn calculate_link_density(youtube_count: usize, data_size: usize) -> f32 {
    if data_size == 0 {
        return 0.0;
    }
    let size_mb = data_size as f32 / (1024.0 * 1024.0);
    youtube_count as f32 / size_mb.max(0.001)
}

/// Calculate target score for a fragment (legacy function, kept for compatibility)
pub fn calculate_target_score(
    youtube_count: usize,
    cyrillic_density: f32,
    has_json_markers: bool,
    size: usize,
) -> f32 {
    let mut score = 0.0;
    
    // YouTube link density (main factor for JSON)
    let link_density = calculate_link_density(youtube_count, size);
    score += link_density.min(100.0) * 0.4; // Max 40 points
    
    // Cyrillic density (main factor for TXT)
    score += cyrillic_density * 30.0; // Max 30 points
    
    // JSON structure markers
    if has_json_markers {
        score += 15.0;
    }
    
    // Size penalty for files outside target range (15-350 KB)
    let size_kb = size as f32 / 1024.0;
    if size_kb >= 15.0 && size_kb <= 350.0 {
        score += 15.0; // Bonus for ideal size range
    } else if size_kb > 400.0 || size_kb < 5.0 {
        score *= 0.5; // Penalty for out-of-range
    }
    
    score
}

/// Enhanced fragment scoring with validation and entropy analysis
pub fn calculate_fragment_score(
    data: &[u8],
    youtube_count: usize,
    cyrillic_density: f32,
    json_markers: usize,
) -> FragmentScore {
    let mut score = 0.0;
    let mut reasons = Vec::new();
    
    // Base target score
    let base_score = calculate_target_score(youtube_count, cyrillic_density, json_markers > 0, data.len());
    score += base_score * 0.6; // 60% weight for base factors
    
    // Entropy analysis
    let entropy = calculate_shannon_entropy(data);
    let _entropy_category = get_entropy_category(data);
    let is_compressed = is_compressed_like(data);
    let is_text_structured = is_structured_text(data);
    
    // Entropy scoring
    if !is_compressed {
        if is_text_structured {
            score += 20.0;
            reasons.push("structured_text".to_string());
        }
        if entropy >= 3.5 && entropy <= 6.5 {
            score += 10.0;
            reasons.push("optimal_entropy".to_string());
        }
    } else {
        score -= 25.0;
        reasons.push("high_entropy_compressed".to_string());
    }
    
    // Validation scoring
    let validation = validate_data_chunk(data);
    
    if validation.is_valid_json {
        score += 30.0;
        reasons.push("valid_json".to_string());
    } else if validation.is_probably_json {
        score += 15.0;
        reasons.push("probably_json".to_string());
    }
    
    if validation.is_valid_youtube_url {
        score += 25.0;
        reasons.push("valid_youtube_url".to_string());
    } else if validation.is_probably_youtube {
        score += 10.0;
        reasons.push("probably_youtube".to_string());
    }
    
    // HTML detection
    if is_valid_html(data) {
        score += 20.0;
        reasons.push("valid_html".to_string());
    }
    
    // CSV detection
    if is_valid_csv(data) {
        score += 15.0;
        reasons.push("valid_csv".to_string());
    }
    
    // Size bonus for target range
    let size_kb = data.len() as f32 / 1024.0;
    if size_kb >= 15.0 && size_kb <= 350.0 {
        score += 10.0;
        reasons.push("target_size".to_string());
    }
    
    // Ensure score doesn't go negative
    score = score.max(0.0);
    
    FragmentScore {
        overall_score: score,
        is_valid_json: validation.is_valid_json,
        is_valid_html: is_valid_html(data),
        is_valid_csv: is_valid_csv(data),
        is_valid_youtube_url: validation.is_valid_youtube_url,
        has_structured_text: is_text_structured,
        is_compressed,
        reasons,
    }
}

/// Quick HTML validation
fn is_valid_html(data: &[u8]) -> bool {
    if let Ok(text) = std::str::from_utf8(data) {
        let trimmed = text.trim();
        trimmed.contains('<') && trimmed.contains('>') && (
            trimmed.to_lowercase().contains("<html") ||
            trimmed.to_lowercase().contains("<body") ||
            trimmed.to_lowercase().contains("<div") ||
            trimmed.to_lowercase().contains("<p")
        )
    } else {
        false
    }
}

/// Quick CSV validation
fn is_valid_csv(data: &[u8]) -> bool {
    if let Ok(text) = std::str::from_utf8(data) {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return false;
        }
        
        let lines: Vec<&str> = trimmed.lines().collect();
        if lines.len() < 2 {
            return false;
        }
        
        // Check for comma-separated structure
        let first_line_commas = lines[0].chars().filter(|&c| c == ',').count();
        if first_line_commas < 1 {
            return false;
        }
        
        // Check consistency - at least half of lines should have similar comma count
        let consistent_lines = lines.iter()
            .filter(|line| line.chars().filter(|&c| c == ',').count() >= first_line_commas / 2)
            .count();
        
        consistent_lines as f32 >= lines.len() as f32 * 0.6
    } else {
        false
    }
}

/// Validate data chunk with quick heuristics and full validation
pub fn validate_data_chunk(data: &[u8]) -> ValidationResult {
    let mut result = ValidationResult::default();
    
    if data.is_empty() {
        return result;
    }
    
    // JSON validation
    result.is_probably_json = is_probably_json(data);
    result.is_valid_json = is_valid_json(data);
    
    if result.is_probably_json {
        result.json_confidence = if result.is_valid_json { 1.0 } else { 0.6 };
    }
    
    // YouTube URL validation
    result.is_probably_youtube = is_probably_youtube_url(data);
    result.is_valid_youtube_url = is_valid_youtube_url(data);
    
    if result.is_probably_youtube {
        result.url_confidence = if result.is_valid_youtube_url { 1.0 } else { 0.5 };
    }
    
    result
}

/// Optimized pattern matcher with pre-compiled regex
/// Clone is cheap because RegexSet is wrapped in Arc
#[derive(Clone)]
pub struct EnhancedMatcher {
    /// Regex for fast needle search (to avoid scanning full chunk with RegexSet)
    finder_regex: Regex,
    
    /// RegexSet for fast pre-filtering (Arc for cheap cloning)
    pattern_set: Arc<RegexSet>,
    
    /// For thread-local deduplication
    seen_ids: AHashSet<[u8; 11]>,
}

// Safety: EnhancedMatcher is Sync because:
// 1. Regex and Arc<RegexSet> are Sync.
// 2. seen_ids (AHashSet) is used ONLY in scan_chunk which takes &mut self.
// 3. clone_fresh takes &self but does not access seen_ids (creates new empty one).
// Therefore sharing &EnhancedMatcher across threads is safe.
unsafe impl Sync for EnhancedMatcher {}

impl EnhancedMatcher {
    /// Create a new matcher (compiles regex - call once, then clone)
    pub fn new() -> Self {
        // Create RegexSet from all patterns
        let pattern_strings: Vec<_> = YOUTUBE_PATTERNS
            .iter()
            .map(|p| p.regex.as_str())
            .collect();
        
        let pattern_set = Arc::new(
            RegexSetBuilder::new(&pattern_strings)
                .size_limit(50 * 1024 * 1024)  // 50 MB
                .build()
                .expect("Failed to compile pattern set")
        );

        // Create a fast pre-filter regex for "needles" (common substrings)
        // This is much faster than running the full RegexSet on every byte
        // Added video-id (hyphen) to catch data-video-id attributes
        let finder_regex = Regex::new(r"(?i)(?:youtube\.com|youtu\.be|video_id|video-id|v=|/v/|embed/|shorts/)").expect("Failed to compile finder regex");
        
        Self {
            finder_regex,
            pattern_set,
            seen_ids: AHashSet::new(),
        }
    }
    
    /// Clone matcher with fresh deduplication cache (cheap - only clones Arc pointer)
    pub fn clone_fresh(&self) -> Self {
        Self {
            finder_regex: self.finder_regex.clone(),
            pattern_set: Arc::clone(&self.pattern_set),
            seen_ids: AHashSet::new(),
        }
    }
    
    /// Scan data chunk with context using needle optimization
    pub fn scan_chunk(
        &mut self,
        data: &[u8],
        base_offset: usize,
        deduplicate: bool,
    ) -> Vec<EnrichedLink> {
        let mut results = Vec::new();
        
        // LIMITATION: Simple needle search might miss some obscure patterns.
        // But for "youtube" and "video_id", it catches 99%.
        // "v=" is added to catch parameter-only patterns.
        
        // Iterate over needle matches
        for m in self.finder_regex.find_iter(data) {
            let start = m.start();
            let end = m.end();
            
            // Define context window around the match
            // We need enough context before (for URL start) and after (for Video ID)
            // URL can be long, so let's take e.g. 100 bytes before and 50 after
            let window_start = start.saturating_sub(100);
            let window_end = (end + 50).min(data.len());
            
            let window_data = &data[window_start..window_end];
            
            // Run RegexSet on this small window
            let matches = self.pattern_set.matches(window_data);
            if !matches.matched_any() {
                continue;
            }
            
            // Extract from window
            for idx in matches.iter() {
                let pattern = &YOUTUBE_PATTERNS[idx];
                
                for cap in pattern.regex.captures_iter(window_data) {
                     // Extract video ID
                    let video_id_bytes = match cap.get(1) {
                        Some(m) => m.as_bytes(),
                        None => continue,
                    };
                    
                    // Validate
                    if !is_valid_video_id(video_id_bytes) {
                        continue;
                    }
                    
                    // Deduplicate
                    if deduplicate {
                        let mut id_array = [0u8; 11];
                        id_array.copy_from_slice(video_id_bytes);
                        
                        if !self.seen_ids.insert(id_array) {
                            continue; // Already seen
                        }
                    }
                    
                    // Extract full URL
                    let full_match = cap.get(0).unwrap();
                    let url_bytes = full_match.as_bytes();
                    
                    // Safe UTF-8 conversion
                    let url = String::from_utf8_lossy(url_bytes).into_owned();
                    let video_id = String::from_utf8_lossy(video_id_bytes).into_owned();
                    
                    // Calculate absolute offset
                    // window_start is offset into 'data'
                    // full_match.start() is offset into 'window_data'
                    let abs_offset = base_offset + window_start + full_match.start();
                    
                    // Confidence
                    let confidence = (pattern.priority as f32) / 10.0;
                    
                    let mut link = EnrichedLink::new(
                        url,
                        video_id,
                        abs_offset as u64,
                        pattern.name.to_string(),
                        confidence,
                    );
                    
                    // Extract title from context (using larger context from original data if needed)
                    // We can use 'data' directly since we have the index
                    let match_pos = window_start + full_match.start();
                    link.title = self.extract_title_from_context(
                        data,
                        match_pos,
                        1000, 
                    );
                    
                    results.push(link);
                }
            }
        }
        
        // Sort results by offset to maintain file order
        results.sort_by(|a, b| a.offset.cmp(&b.offset));
        
        results
    }
    
    /// Extract title from context
    fn extract_title_from_context(
        &self,
        data: &[u8],
        match_pos: usize,
        window_size: usize,
    ) -> Option<String> {
        // Context window
        let ctx_start = match_pos.saturating_sub(window_size);
        let ctx_end = (match_pos + window_size).min(data.len());
        let context = &data[ctx_start..ctx_end];
        
        // Try each title pattern
        for pattern in TITLE_PATTERNS.iter() {
            if let Some(cap) = pattern.captures(context) {
                if let Some(title_match) = cap.get(1) {
                    let raw_title = String::from_utf8_lossy(title_match.as_bytes());
                    
                    // Decode all HTML entities using html_escape library
                    let clean = decode_html_entities(&raw_title)
                        .trim()
                        .to_string();
                    
                    // Filters
                    if clean.len() > 3
                        && clean.len() < 200
                        && !clean.to_lowercase().contains("youtube")
                    {
                        return Some(clean);
                    }
                }
            }
        }
        
        None
    }
    
    /// Clear deduplication cache
    pub fn clear_cache(&mut self) {
        self.seen_ids.clear();
    }
}

impl Default for EnhancedMatcher {
    fn default() -> Self {
        Self::new()
    }
}