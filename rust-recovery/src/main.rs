use rust_recovery::cli::Args;
use clap::Parser;
use rust_recovery::disk::DiskImage;
use rust_recovery::error::{Result, RecoveryError};
use rust_recovery::types::{Offset, ScanConfig, ScanProgress, StreamFragment, FragmentScore};
use rust_recovery::scanner::ParallelScanner;
use rust_recovery::report;
use rust_recovery::stream_solver;
use tokio::runtime::Runtime;
use std::sync::Arc;

use tokio::sync::mpsc;
use rust_recovery::tui::{TuiApplication, TuiApp, TuiEvent};
use rust_recovery::report::{ProfessionalReportGenerator, create_report_metadata, create_scan_results};
use rust_recovery::recovery::{clean_file_content, extract_title};

use std::path::Path;
use std::fs;

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

    // Initialize output directory
    let output_dir = args.output.clone();
    if !output_dir.exists() {
        fs::create_dir_all(&output_dir)
            .map_err(|e| RecoveryError::Config(format!("Failed to create output directory: {}", e)))?;
    }

    // Create session info
    let session_info = format!(
        "version: 12.0\nimage_file: {}\nstart_time: {}\nparameters: {:?}\n",
        args.image.display(),
        chrono::Local::now().format("%Y-%m-%dT%H:%M:%S"),
        args
    );
    
    let session_path = output_dir.join("session.info");
    fs::write(&session_path, session_info)
        .map_err(|e| RecoveryError::Config(format!("Failed to save session info: {}", e)))?;

    // Open disk image
    println!("Opening disk image...");
    let disk = DiskImage::open(&args.image)?;
    let image_size = disk.size().as_u64();
    println!("  Image size: {} bytes ({:.2} GB)", 
        image_size,
        image_size as f64 / (1024.0 * 1024.0 * 1024.0)
    );
    println!();

    // Create scan configuration
    let mut scan_config = ScanConfig::new(
        args.chunk_max_bytes() as usize,
        64 * 1024, // 64KB overlap
        0,         // auto threads
    );
    scan_config.reverse = args.reverse;
    scan_config.nvme_optimization = args.nvme;

    // Create report generator
    let report_generator = ProfessionalReportGenerator::new(&output_dir);
    
    // Create TUI if enabled
    let mut tui_app = None;
    let mut tui_sender = None;
    
    if !args.no_live {
        // Create TUI event channel
        let (sender, receiver) = mpsc::unbounded_channel::<TuiEvent>();
        tui_sender = Some(sender);
        
        // Create TUI application
        let mut app = TuiApp::new(
            image_size,
            args.image.to_string_lossy().to_string(),
            output_dir.to_string_lossy().to_string(),
            scan_config.clone(),
        );
        app.target_files = args.early_exit as u32;
        
        tui_app = Some(TuiApplication::new(app, receiver)?);
    }

    // Send initial log message
    if let Some(ref sender) = tui_sender {
        let _ = sender.send(TuiEvent::LogMessage {
            message: "Starting disk recovery scan".to_string(),
        });
    }

    // Print configuration
    print_configuration(&args);

    // Run the main scanning pipeline in a separate thread if TUI is enabled
    // This allows TUI to run on the main thread (required for some terminals)
    
    // items to move into thread
    let disk_clone = disk.clone();
    let args_clone = args.clone();
    let scan_config_clone = scan_config.clone();
    let output_dir_clone = output_dir.clone();
    let tui_sender_clone = tui_sender.clone();

    let scan_thread = std::thread::spawn(move || {
        let result = run_scan_pipeline(
            disk_clone,
            &args_clone,
            &scan_config_clone,
            tui_sender_clone.as_ref(),
            &output_dir_clone,
        );

        // Send completion event
        if let Some(ref sender) = tui_sender_clone {
            let _ = sender.send(TuiEvent::ScanCompleted);
        }
        
        result
    });

    // Run TUI if enabled
    if let Some(mut app) = tui_app {
        // This will block until Q is pressed or scan completes
        if let Err(e) = app.run() {
            eprintln!("TUI Error: {}", e);
        }
    }

    // Wait for scan to finish and get results
    // If TUI was quit early, we still wait for scan to complete
    let scan_results = scan_thread.join()
        .map_err(|_| RecoveryError::Config("Scan thread panicked".to_string()))??;

    // Generate reports
    println!("\nScanning complete. Generating reports...");
    let metadata = create_report_metadata(
        &args.image.to_string_lossy(),
        &output_dir.to_string_lossy(),
        "12.0",
    );
    
    let mut scan_stats = create_scan_results(
        image_size,
        scan_results.bytes_scanned,
        scan_results.candidates_found,
        scan_results.scan_duration,
        args.reverse,
        args.enable_exfat,
        args.nvme,
    );
    scan_stats.files_recovered = scan_results.recovered_files.len() as u32;

    let report_paths = report_generator.generate_full_report(
        scan_stats,
        scan_results.clusters,
        scan_results.recovered_files,
        scan_results.failure_reasons,
        metadata,
    ).map_err(|e| RecoveryError::Config(format!("Report generation failed: {}", e)))?;

    println!("Reports generated:");
    println!("  HTML: {}", report_paths.html_path.display());
    println!("  JSON: {}", report_paths.json_path.display());

    // TUI cleanup is automatic via Drop, but we can ensure terminal is restored here if needed
    // if let Some(mut app) = tui_app {
    //     let _ = app.run(); // already ran
    // }

    println!("Recovery complete!");
    Ok(())
}

/// Scan results from the main pipeline
#[derive(Debug, Clone)]
struct ScanResults {
    bytes_scanned: u64,
    candidates_found: u32,
    scan_duration: std::time::Duration,
    clusters: Vec<report::DataCluster>,
    recovered_files: Vec<report::RecoveredFile>,
    failure_reasons: Vec<String>,
}

/// Main scanning pipeline
fn run_scan_pipeline(
    disk: DiskImage,
    args: &Args,
    scan_config: &ScanConfig,
    tui_sender: Option<&mpsc::UnboundedSender<TuiEvent>>,
    output_dir: &Path,
) -> Result<ScanResults> {
    let start_time = std::time::Instant::now();
    
    // Test basic read operations first
    test_disk_access(&disk)?;

    // Send initial status
    if let Some(sender) = tui_sender {
        let _ = sender.send(TuiEvent::LogMessage {
            message: "Disk access verified, starting real-time scan".to_string(),
        });
    }

    // Run the actual scanner
    let (bytes_scanned, candidates_found, recovered_files, clusters) = 
        run_real_scan(disk, args, scan_config, tui_sender, output_dir)?;

    let scan_duration = start_time.elapsed();
    let mut failure_reasons = Vec::new();

    // If no files recovered, add failure reasons
    if recovered_files.is_empty() {
        failure_reasons.push("No valid data structures found in scanned data".to_string());
        failure_reasons.push("Try adjusting --target-size-min or --chunk-max".to_string());
    }

    Ok(ScanResults {
        bytes_scanned,
        candidates_found: candidates_found as u32,
        scan_duration,
        clusters,
        recovered_files,
        failure_reasons,
    })
}

/// Perform real disk scanning using ParallelScanner
fn run_real_scan(
    disk: DiskImage,
    _args: &Args,
    scan_config: &ScanConfig,
    tui_sender: Option<&mpsc::UnboundedSender<TuiEvent>>,
    _output_dir: &Path,
) -> Result<(u64, usize, Vec<report::RecoveredFile>, Vec<report::DataCluster>)> {
    let rt = Arc::new(Runtime::new().map_err(|e| RecoveryError::Config(e.to_string()))?);
    let scanner = ParallelScanner::new(scan_config.clone());
    
    let (progress_tx, mut progress_rx) = mpsc::channel(100);
    
    let disk_clone = disk.clone();
    let scanner_clone = scanner.clone();
    let rt_clone = Arc::clone(&rt);
    
    // Start scanner in a background thread
    let scan_handle = std::thread::spawn(move || {
        rt_clone.block_on(async {
            scanner_clone.scan(&disk_clone, progress_tx).await
        })
    });

    let mut total_bytes_scanned = 0u64;
    let mut candidates_count = 0usize;
    let mut recovered_files = Vec::new();
    let mut clusters = Vec::new();
    let mut stream_fragments = Vec::new();

    // Process progress updates
    while let Some(progress) = rt.block_on(async { progress_rx.recv().await }) {
        match progress {
            ScanProgress::BytesScanned(bytes) => {
                total_bytes_scanned += bytes;
                if let Some(sender) = tui_sender {
                    let _ = sender.send(TuiEvent::UpdatePosition {
                        position: total_bytes_scanned, 
                        bytes_scanned: total_bytes_scanned,
                    });
                }
            }
            ScanProgress::HotFragment(fragment) => {
                candidates_count += 1;
                
                // Add to clusters for report
                clusters.push(report::DataCluster {
                    id: candidates_count,
                    start_offset_hex: format!("0x{:X}", fragment.offset),
                    end_offset_hex: format!("0x{:X}", fragment.offset + fragment.size as u64),
                    size_bytes: fragment.size as u64,
                    size_kb: (fragment.size / 1024) as u64,
                    link_count: fragment.youtube_count as u32,
                    density: fragment.cyrillic_density as f64,
                    confidence: fragment.target_score as f64,
                    links: Vec::new(), 
                });

                // Convert to StreamFragment for solver
                let stream_frag = StreamFragment {
                    offset: fragment.offset,
                    size: fragment.size,
                    base_score: fragment.target_score,
                    file_type: fragment.file_type_guess.clone(),
                    links: Vec::new(), // Optional: could extract links here
                    feature_vector: rust_recovery::smart_separation::ByteFrequency::default(), 
                    fragment_score: fragment.fragment_score.clone(),
                };
                stream_fragments.push(stream_frag);

                if let Some(sender) = tui_sender {
                    let _ = sender.send(TuiEvent::FragmentFound {
                        offset: fragment.offset,
                    });
                }
            }
            ScanProgress::ChunkCompleted(offset) => {
                if let Some(sender) = tui_sender {
                    let _ = sender.send(TuiEvent::LogMessage {
                        message: format!("Chunk at 0x{:X} completed", offset),
                    });
                }
            }
            ScanProgress::ChunkError(offset, err) => {
                if let Some(sender) = tui_sender {
                    let _ = sender.send(TuiEvent::LogMessage {
                        message: format!("Error at 0x{:X}: {}", offset, err),
                    });
                }
            }
        }
    }

    // Wait for scan to finish
    let _ = scan_handle.join().map_err(|_| RecoveryError::Config("Scanner thread panicked".to_string()))?;

    // --- ASSEMBLE STREAMS ---
    if !stream_fragments.is_empty() {
        if let Some(sender) = tui_sender {
            let _ = sender.send(TuiEvent::LogMessage {
                message: format!("Assembling {} fragments into streams...", stream_fragments.len()),
            });
        }

        let streams = stream_solver::assemble_streams(&stream_fragments);
        
        // Create output subdirectory for binary files
        let bin_output_dir = _output_dir.join("01_RECOVERED_FILES");
        if !bin_output_dir.exists() {
            let _ = fs::create_dir_all(&bin_output_dir);
        }

        for (i, stream) in streams.into_iter().enumerate() {
            let file_id = i + 1;
            let file_type = stream.fragments[0].file_type.clone();
            // Reconstruct file data by concatenating fragments
            let mut raw_data = Vec::new();
            for fragment in &stream.fragments {
                if let Ok(slice) = disk.get_slice(Offset::new(fragment.offset), fragment.size) {
                    raw_data.extend_from_slice(slice.data);
                }
            }

            // Clean content (remove junk/nulls)
            let file_data = clean_file_content(&raw_data, &file_type).into_owned();

            // Generate filename with title if possible
            let mut filename = format!("recovered_{:04}.{}", file_id, file_type);
            if let Some(title) = extract_title(&file_data, &file_type) {
                filename = format!("recovered_{:04}_{}.{}", file_id, title, file_type);
            }
            
            let file_path = bin_output_dir.join(&filename);

            let total_size_bytes = file_data.len() as u64;
            let sha256 = rust_recovery::matcher::sha256_hash(&file_data);

            // Physically save to disk
            let validation_status = if fs::write(&file_path, &file_data).is_ok() {
                report::ValidationStatus::Valid
            } else {
                report::ValidationStatus::Invalid
            };
            
            recovered_files.push(report::RecoveredFile {
                id: file_id,
                filename: filename.clone(),
                file_type,
                confidence: stream.confidence as f64,
                links: Vec::new(),
                size_kb: (total_size_bytes / 1024) as u64,
                sha256,
                start_offset: stream.fragments.first().unwrap().offset,
                end_offset: stream.fragments.last().unwrap().offset + stream.fragments.last().unwrap().size as u64,
                validation_status,
                recovery_time: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            });

            if let Some(sender) = tui_sender {
                let _ = sender.send(TuiEvent::FileRecovered { filename: filename.clone() });
                let _ = sender.send(TuiEvent::LogMessage {
                    message: format!("Saved recovered file: {} ({} KB)", filename, total_size_bytes / 1024),
                });
            }
        }
    }

    Ok((total_bytes_scanned, candidates_count, recovered_files, clusters))
}

/// Test basic disk access
fn test_disk_access(disk: &DiskImage) -> Result<()> {
    println!("Testing disk access...");
    let test_offset = Offset::new(0);
    let test_size = 512.min(disk.size().as_u64() as usize);
    let slice = disk.get_slice(test_offset, test_size)?;
    println!("  Successfully read {} bytes at offset {}",
        slice.data.len(),
        slice.offset
    );
    Ok(())
}


/// Print configuration information
fn print_configuration(args: &Args) {
    println!("Ultimate File Recovery - Rust Implementation v12.0");
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
}
