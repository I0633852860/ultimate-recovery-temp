mod cli;
mod disk;
mod error;
mod types;

use clap::Parser;
use cli::Args;
use disk::DiskImage;
use error::Result;

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
    let test_offset = types::Offset::new(0);
    let test_size = 512.min(disk.size().as_u64() as usize);
    let slice = disk.get_slice(test_offset, test_size)?;
    println!("  Successfully read {} bytes at offset {}", 
        slice.data.len(), 
        slice.offset
    );
    println!();

    println!("Stage 1 initialization complete!");
    println!("Disk image opened and memory-mapped successfully.");
    println!();
    println!("Next stages will implement:");
    println!("  - Pattern scanning and matching");
    println!("  - Candidate discovery and clustering");
    println!("  - Fragment assembly");
    println!("  - File reconstruction");
    println!("  - Report generation");

    Ok(())
}
