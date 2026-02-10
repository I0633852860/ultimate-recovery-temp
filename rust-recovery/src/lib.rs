//! Rust-based file recovery tool - Stage 3 Implementation
//!
//! This library provides high-performance disk image scanning capabilities:
//! - SIMD-accelerated pattern search (AVX2/SSE4.2 with scalar fallback)
//! - Parallel chunk scanner using rayon
//! - Runtime SIMD dispatching
//! - Panic isolation with catch_unwind
//! - Progress streaming via tokio::sync::mpsc
//! - 64-byte chunk alignment for cache efficiency
//! - Enhanced validation and scoring system with entropy analysis

pub mod cli;
pub mod disk;
pub mod error;
pub mod simd_search;
pub mod types;
pub mod scanner;
pub mod matcher;
pub mod entropy;
pub mod exfat;
pub mod fragment_linker;
pub mod smart_separation;
pub mod stream_solver;
pub mod checkpoint;
pub mod tui;
pub mod report;
pub mod recovery;

// Re-export commonly used types
pub use types::{Offset, Size, ClusterId};
pub use types::{ScanConfig, ScanResult, ScanProgress, ScanStats, HotFragment, EnrichedLink};
pub use types::{FragmentScore, ValidationResult};
pub use types::{StreamFragment, StreamScoringWeights, AssembledStream};
pub use disk::{DiskImage, FragmentSlice};
pub use scanner::{ParallelScanner, ChunkInfo};
pub use simd_search::{find_pattern_simd, count_pattern_simd, scan_block_simd, BlockScanResult};
pub use matcher::{EnhancedMatcher, calculate_fragment_score, validate_data_chunk};
pub use matcher::{detect_cyrillic, cyrillic_density, count_json_markers_fast, calculate_link_density};
pub use entropy::{calculate_shannon_entropy, is_compressed_like, is_structured_text, get_entropy_category};
pub use stream_solver::{assemble_streams, assemble_streams_with_weights};
pub use checkpoint::{
    Checkpoint, CheckpointManager, ResumeValidation, compute_image_hash, create_checkpoint,
    validate_resume, load_checkpoint, save_checkpoint_atomic, save_checkpoint_blocking,
};
pub use tui::{TuiApp, TuiEvent, TuiApplication};
pub use report::{ProfessionalReportGenerator, ReportContext, create_report_metadata, create_scan_results};
pub use error::{RecoveryError, Result};
