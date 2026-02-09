/// Newtype wrapper for byte offsets in disk images
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Offset(pub u64);

impl Offset {
    pub fn new(value: u64) -> Self {
        Self(value)
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }

    pub fn checked_add(&self, size: Size) -> Option<Offset> {
        self.0.checked_add(size.0).map(Offset)
    }
}

impl std::fmt::Display for Offset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{:X}", self.0)
    }
}

/// Newtype wrapper for sizes in bytes
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Size(pub u64);

impl Size {
    pub fn new(value: u64) -> Self {
        Self(value)
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }

    pub fn as_usize(&self) -> Option<usize> {
        usize::try_from(self.0).ok()
    }
}

impl std::fmt::Display for Size {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} bytes", self.0)
    }
}

/// Newtype wrapper for cluster IDs
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClusterId(pub u64);

impl ClusterId {
    pub fn new(value: u64) -> Self {
        Self(value)
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for ClusterId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "cluster_{}", self.0)
    }
}

/// Scanner configuration
#[derive(Debug, Clone)]
pub struct ScanConfig {
    /// Chunk size in bytes (aligned to 64 bytes)
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
            chunk_size: 256 * 1024 * 1024,
            overlap_size: 64 * 1024,
            num_threads: 0,
            deduplicate: true,
            min_confidence: 0.0,
        }
    }
}

impl ScanConfig {
    /// Create a new ScanConfig with 64-byte aligned chunk_size
    pub fn new(chunk_size: usize, overlap_size: usize, num_threads: usize) -> Self {
        // Align chunk_size to 64 bytes (cache line)
        let aligned_chunk_size = (chunk_size / 64) * 64;

        Self {
            chunk_size: aligned_chunk_size.max(64),
            overlap_size,
            num_threads,
            ..Default::default()
        }
    }
}

/// YouTube link with metadata
#[derive(Debug, Clone)]
pub struct EnrichedLink {
    pub url: String,
    pub video_id: String,
    pub title: Option<String>,
    pub offset: u64,
    pub pattern_name: String,
    pub confidence: f32,
}

impl EnrichedLink {
    pub fn new(url: String, video_id: String, offset: u64, pattern_name: String, confidence: f32) -> Self {
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

/// Result of a scan operation
#[derive(Debug, Default)]
pub struct ScanResult {
    pub links: Vec<EnrichedLink>,
    pub bytes_scanned: u64,
    pub duration_secs: f64,
}

/// Progress update sent via tokio channel
#[derive(Debug, Clone)]
pub enum ScanProgress {
    /// Bytes processed
    BytesScanned(u64),
    /// Chunk completed
    ChunkCompleted(u64),
    /// Hot fragment found
    HotFragment(HotFragment),
    /// Error in a chunk (non-fatal)
    ChunkError(u64, String),
}

/// Scan statistics
#[derive(Debug, Clone, Default)]
pub struct ScanStats {
    pub total_chunks: usize,
    pub completed_chunks: usize,
    pub error_chunks: usize,
    pub bytes_processed: u64,
    pub links_found: usize,
    pub hot_fragments_found: usize,
}

impl ScanStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn completion_percentage(&self) -> f32 {
        if self.total_chunks == 0 {
            0.0
        } else {
            (self.completed_chunks as f32 / self.total_chunks as f32) * 100.0
        }
    }
}

/// A "hot" fragment detected during scanning
#[derive(Debug, Clone)]
pub struct HotFragment {
    pub offset: u64,
    pub size: usize,
    pub youtube_count: usize,
    pub cyrillic_density: f32,
    pub json_markers: usize,
    pub has_valid_json: bool,
    pub target_score: f32,
    pub file_type_guess: String,
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
        }
    }

    pub fn is_target_size(&self) -> bool {
        let size_kb = self.size as f32 / 1024.0;
        size_kb >= 15.0 && size_kb <= 350.0
    }
}

