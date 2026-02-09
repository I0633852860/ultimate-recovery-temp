mod cli;
mod disk;
mod error;
mod simd_search;
mod types;
mod scanner;

use clap::Parser;
use cli::Args;
use disk::DiskImage;
use error::Result;
use types::{Offset, ScanConfig};
use scanner::ParallelScanner;
use simd_search;

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    // Parse command line arguments
    let args = Args::parse();

    // Validate arguments
    if let Err(e) = args.validate() {
        eprintln!("Invalid arguments: {}", e);
        std::process::exit(1);
    }

    // Print configuration
    println!("Ultimate File Recovery - Rust Implementation v0.1.0");
    println!("{}", "=".repeat(60));
    println!();
    println!("Configuration:");
    println!("  Image:              {}", args.image.display());
    println!("  Output directory:   {}", args.output.display());
    println!(
        "  Target size range:  {} - {} KB",
        args.target_size_min, args.target_size_max
    );
    println!(
        "  Chunk size range:   {} - {} KB",
        args.chunk_min, args.chunk_max
    );
    println!("  Reverse scan:       {}", args.reverse);
    println!("  NVMe optimization:  {}", args.nvme);
    println!("  Enable exFAT:       {}", args.enable_exfat);
    println!("  Full exFAT recovery: {}", args.full_exfat_recovery);
    println!("  Links only:         {}", args.links_only);
    println!("  Semantic scan:      {}", args.semantic_scan);
    println!("  Live dashboard:     {}", !args.no_live);
    if args.early_exit > 0 {
        println!("  Early exit after:   {} files", args.early_exit);
    }
    println!();

    // Open disk image
    println!("Opening disk image...");
    let disk = DiskImage::open(&args.image)?;
    println!("  Image size:         {} bytes ({:.2} GB)", 
        disk.size().as_u64(),
        disk.size().as_u64() as f64 / (1024.0 * 1024.0 * 1024.0)
    );
    println!();

    // Test basic read operations
    println!("Testing disk access...");
    let test_offset = Offset::new(0);
    let test_size = 512.min(disk.size().as_u64() as usize);
    let slice = disk.get_slice(test_offset, test_size)?;
    println!("  Successfully read {} bytes at offset {}",
        slice.data.len(),
        slice.offset
    );
    println!();

    // Test SIMD search functionality
    println!("Testing SIMD search...");
    let test_pattern = b"test";
    let test_data = b"this is a test of the simd search functionality";
    if let Some(pos) = simd_search::find_pattern_simd(test_data, test_pattern) {
        println!("  Found pattern at position {} using SIMD", pos);
    }
    println!();

    // Test parallel scanner with small chunks
    println!("Testing parallel scanner...");
    let scan_config = ScanConfig::new(1024 * 1024, 64 * 1024, 0);
    let scanner = ParallelScanner::new(scan_config);
    println!("  Scanner configured with chunk_size: {}", scanner.config.chunk_size);
    println!("  Chunk alignment: 64 bytes (cache line)");
    println!();

    println!("Stage 2 initialization complete!");
    println!("Disk image opened and memory-mapped successfully.");
    println!();
    println!("Implemented in Stage 2:");
    println!("  - SIMD-accelerated pattern search (AVX2/SSE4.2 with scalar fallback)");
    println!("  - Parallel chunk scanner using rayon");
    println!("  - Runtime SIMD dispatching");
    println!("  - Panic isolation with catch_unwind");
    println!("  - Progress streaming via tokio::sync::mpsc");
    println!("  - 64-byte chunk alignment for cache efficiency");
    println!();
    println!("Next stages will implement:");
    println!("  - Candidate discovery and clustering");
    println!("  - Fragment assembly");
    println!("  - File reconstruction");
    println!("  - Report generation");

    Ok(())
}
