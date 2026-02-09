//! Rust-based file recovery tool - Stage 2 Implementation
//!
//! This library provides high-performance disk image scanning capabilities:
//! - SIMD-accelerated pattern search (AVX2/SSE4.2 with scalar fallback)
//! - Parallel chunk scanner using rayon
//! - Runtime SIMD dispatching
//! - Panic isolation with catch_unwind
//! - Progress streaming via tokio::sync::mpsc
//! - 64-byte chunk alignment for cache efficiency

pub mod cli;
pub mod disk;
pub mod error;
pub mod simd_search;
pub mod types;
pub mod scanner;

// Re-export commonly used types
pub use types::{Offset, Size, ClusterId};
pub use types::{ScanConfig, ScanResult, ScanProgress, ScanStats, HotFragment, EnrichedLink};
pub use disk::{DiskImage, FragmentSlice};
pub use scanner::{ParallelScanner, ChunkInfo};
pub use simd_search::{find_pattern_simd, count_pattern_simd, scan_block_simd, BlockScanResult};
pub use error::{RecoveryError, Result};
