use serde::{Deserialize, Serialize};
use pyo3::prelude::*;

/// YouTube link with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
#[pyclass]
pub struct EnrichedLink {
    /// Full URL
    #[pyo3(get)]
    pub url: String,
    
    /// 11-character video ID
    #[pyo3(get)]
    pub video_id: String,
    
    /// Video title (if found)
    #[pyo3(get)]
    pub title: Option<String>,
    
    /// Offset in the file
    #[pyo3(get)]
    pub offset: usize,
    
    /// Name of the pattern that matched
    #[pyo3(get)]
    pub pattern_name: String,
    
    /// Confidence level (0.0 - 1.0)
    #[pyo3(get)]
    pub confidence: f32,
}

impl EnrichedLink {
    pub fn new(url: String, video_id: String, offset: usize, pattern_name: String, confidence: f32) -> Self {
        Self {
            url,
            video_id,
            title: None,
            offset,
            pattern_name,
            confidence,
        }
    }
}

/// Scanner configuration
#[derive(Debug, Clone)]
pub struct ScanConfig {
    /// Chunk size in bytes
    pub chunk_size: usize,
    
    /// Overlap size in bytes
    pub overlap_size: usize,
    
    /// Number of threads (0 = auto)
    pub num_threads: usize,
    
    /// Enable deduplication
    pub deduplicate: bool,
    
    /// Minimum confidence level
    pub min_confidence: f32,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            chunk_size: 256 * 1024 * 1024, // 256 MB
            overlap_size: 64 * 1024,           // 64 KB (increased from 32KB for better title extraction)
            num_threads: 0,                 // Auto
            deduplicate: true,
            min_confidence: 0.0,
        }
    }
}

/// Result of a scan operation
#[derive(Debug, Default)]
pub struct ScanResult {
    /// Found links
    pub links: Vec<EnrichedLink>,
    
    /// Total bytes scanned
    pub bytes_scanned: usize,
    
    /// Duration in seconds
    pub duration_secs: f64,
}

// ═══════════════════════════════════════════════════════════════════════════════
#[derive(Debug, Clone, Serialize, Deserialize)]
#[pyclass]
pub struct ExFATHint {
    #[pyo3(get)]
    pub filename: String,
    #[pyo3(get)]
    pub is_deleted: bool,
    #[pyo3(get)]
    pub entry_offset: u64,
}

/// A "hot" fragment detected during scanning - likely to be a target file
#[derive(Debug, Clone)]
pub struct HotFragment {
    /// Offset in the disk image
    pub offset: u64,
    
    /// Size of the fragment in bytes
    pub size: usize,
    
    /// Number of YouTube links found
    pub youtube_count: usize,
    
    /// Cyrillic character density (0.0 - 1.0)
    pub cyrillic_density: f32,
    
    /// Count of JSON structure markers ({, [, "url":, etc.)
    pub json_markers: usize,
    
    /// Whether this looks like valid JSON structure
    pub has_valid_json: bool,
    
    /// Calculated target score (higher = more likely target file)
    pub target_score: f32,
    
    /// File type guess (json, txt, html, unknown)
    pub file_type_guess: String,
    
    /// SHA-256 hash for forensic evidence integrity (v6.1)
    pub sha256_hash: Option<String>,
    
    /// Timestamp when discovered (v6.1)
    pub discovered_at: u64,

    /// SmartSeparation: Normalized byte-frequency vector (lazy)
    pub feature_vector: Option<[f32; 256]>,

    /// SmartSeparation: Representative words for text-heavy fragments (lazy)
    pub semantic_words: Option<Vec<String>>,

    /// SmartSeparation: exFAT metadata hint
    pub exfat_hint: Option<ExFATHint>,
}

impl HotFragment {
    pub fn new(offset: u64, size: usize) -> Self {
        Self {
            offset,
            size,
            youtube_count: 0,
            cyrillic_density: 0.0,
            json_markers: 0,
            has_valid_json: false,
            target_score: 0.0,
            file_type_guess: "unknown".to_string(),
            sha256_hash: None,
            discovered_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            feature_vector: None,
            semantic_words: None,
            exfat_hint: None,
        }
    }
    
    /// Check if fragment is in target size range (15-350 KB)
    pub fn is_target_size(&self) -> bool {
        let size_kb = self.size as f32 / 1024.0;
        size_kb >= 15.0 && size_kb <= 350.0
    }
}

/// Epicenter found during heatmap scanning
#[derive(Debug, Clone)]
pub struct Epicenter {
    /// Offset where high density was detected
    pub offset: u64,
    
    /// Link density (links per MB)
    pub density: f32,
    
    /// Whether deep scan is needed
    pub needs_deep_scan: bool,
}

impl Epicenter {
    /// Threshold for triggering deep scan (50 links per MB)
    pub const DEEP_SCAN_THRESHOLD: f32 = 50.0;
    
    pub fn new(offset: u64, density: f32) -> Self {
        Self {
            offset,
            density,
            needs_deep_scan: density >= Self::DEEP_SCAN_THRESHOLD,
        }
    }
}
