#!/usr/bin/env python3
"""
Ultimate File Recovery v11.5 FINAL
Complete integrated system with:
  - Cluster analysis & file reconstruction
  - Professional HTML reports
  - Live dashboard (ddrescue/smartctl style)
  - Checkpoint/resume
  - Hotkey controls (P/S/V/C/Q)
"""

import sys
import os
import argparse
from pathlib import Path
import time
import json
import logging

# Настройка логирования (Ошибка #26)
logging.basicConfig(
    filename='recovery.log',
    level=logging.ERROR,
    format='%(asctime)s - %(levelname)s - %(message)s'
)

# Add project paths so imports work from any working directory
_PROJECT_ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(_PROJECT_ROOT / "src"))
sys.path.insert(0, str(_PROJECT_ROOT / "lib"))

# Import all modules from src/
from cluster_analyzer import ClusterAnalyzer
from file_reconstructor import FileReconstructor
from candidate_manager import CandidateManager
from professional_report import ProfessionalReportGenerator
from modern_ui import ModernUI
from directory_structure import DirectoryManager
from index_generator import IndexGenerator
from live_dashboard import LiveDashboard
from checkpoint_manager import CheckpointManager
from hotkey_controller import HotkeyController


from rich.console import Console
from rich.live import Live

console = Console()


def main():
    parser = argparse.ArgumentParser(
        description="Ultimate File Recovery v11.5 - Professional Data Recovery System"
    )
    parser.add_argument("image", help="Disk image file to scan")
    parser.add_argument("--target-size-min", type=int, default=15,
                        help="Minimum file size in KB")
    parser.add_argument("--target-size-max", type=int, default=300,
                        help="Maximum file size in KB")
    parser.add_argument("--reverse", action="store_true",
                        help="Scan from end to start")
    parser.add_argument("--nvme", action="store_true",
                        help="Optimize for NVMe drives")
    parser.add_argument("--early-exit", type=int, default=0,
                        help="Stop after N files recovered")
    parser.add_argument("-o", "--output", default="recovery_output",
                        help="Output directory")
    parser.add_argument("--enable-exfat", action="store_true",
                        help="Enable exFAT metadata scanning (Opt-in)")
    parser.add_argument("--no-live", action="store_true",
                        help="Disable live dashboard (use simple text output)")
    parser.add_argument("--links-only", action="store_true",
                        help="Mode: extract only links, don't save binary chunks")
    parser.add_argument("--chunk-min", type=int, default=32,
                        help="Minimum dynamic chunk size in KB")
    parser.add_argument("--chunk-max", type=int, default=2048,
                        help="Maximum dynamic chunk size in KB")
    parser.add_argument("--full-exfat-recovery", action="store_true", default=True,
                        help="Enable FAT chain following for full file recovery")
    parser.add_argument("--semantic-scan", action="store_true",
                        help="Analyze candidates and group by semantic category (Trading/Psychology)")

    args = parser.parse_args()

    # ----------------------------------------------------------------
    # Initialize output directory
    # ----------------------------------------------------------------
    output_dir = Path(args.output)
    output_dir.mkdir(parents=True, exist_ok=True)

    dir_manager = DirectoryManager(str(output_dir))
    dir_manager.create_structure()

    # Save session info
    session_info = {
        "version": "11.5",
        "image_file": args.image,
        "start_time": time.strftime("%Y-%m-%dT%H:%M:%S"),
        "parameters": {
            "target_size_min_kb": args.target_size_min,
            "target_size_max_kb": args.target_size_max,
            "reverse": args.reverse,
            "nvme": args.nvme,
            "early_exit": args.early_exit,
        },
    }
    dir_manager.save_session_info(session_info)

    # ----------------------------------------------------------------
    # Initialize checkpoint manager
    # ----------------------------------------------------------------
    checkpoint_mgr = CheckpointManager(str(output_dir), args.image)

    # Try resume from checkpoint
    checkpoint = checkpoint_mgr.load()
    start_position = checkpoint.get("position", 0) if checkpoint else 0

    if checkpoint:
        console.print(
            f"Resuming from checkpoint: 0x{start_position:X}", style="bold cyan"
        )
        if "state" in checkpoint:
            console.print(
                f"Resumed progress: {checkpoint['state'].get('bytes_scanned', 0) / 1024 / 1024 / 1024:.1f} GB scanned"
            )

    # ----------------------------------------------------------------
    # Initialize live dashboard
    # ----------------------------------------------------------------
    image_size = Path(args.image).stat().st_size
    dashboard = LiveDashboard(image_size, args.image, str(output_dir))
    dashboard.is_reverse = args.reverse
    if args.early_exit > 0:
        dashboard.target_files = args.early_exit

    # Restore dashboard state from checkpoint
    if checkpoint and "state" in checkpoint:
        dashboard.bytes_scanned = checkpoint["state"].get("bytes_scanned", 0)
        dashboard.current_position = start_position
        dashboard.fragments_found = checkpoint["state"].get("fragments_found", 0)
        dashboard.recovered_files = checkpoint["state"].get("recovered_files", 0)

    # ----------------------------------------------------------------
    # Initialize hotkey controller
    # ----------------------------------------------------------------
    hotkeys = HotkeyController()
    hotkeys.start()

    # ----------------------------------------------------------------
    # Phase 0: exFAT Metadata Scan + Full File Recovery (v5.0)
    # ----------------------------------------------------------------
    priority_candidates = []
    if args.enable_exfat:
        console.print("\n[bold]Phase 0: Scanning exFAT Metadata (Enabled)[/bold]")
        try:
            import rust_accelerator
            exfat_scanner = rust_accelerator.RustExFATScanner()

            # 0.1: Диагностика boot sector
            boot_info = exfat_scanner.get_boot_info(args.image)
            if boot_info.get('found', False):
                console.print(f"  exFAT boot sector found at 0x{boot_info['boot_sector_offset']:X}")
                console.print(f"  Cluster size: {boot_info['cluster_size']} bytes, "
                              f"FAT at 0x{boot_info['fat_offset']:X}, "
                              f"Heap at 0x{boot_info['cluster_heap_offset']:X}")
            else:
                console.print("  [yellow]No exFAT boot sector found — chunk-only mode[/yellow]")

            # 0.2: Сканируем directory entries
            # 0.2: Сканируем directory entries + Hot Content (Single-Pass)
            with console.status("[cyan]Reading disk metadata & Searching for traces...[/cyan]"):
                exfat_files, exfat_links = exfat_scanner.scan_file(args.image, 0, 0)

            deleted = [f for f in exfat_files if f.is_deleted]
            active  = [f for f in exfat_files if not f.is_deleted]
            console.print(f"  Found {len(exfat_files)} entries ({len(deleted)} deleted, {len(active)} active)")
            console.print(f"  Found {len(exfat_links)} forensic artifacts in Phase 0")

            # Process links immediately
            for link in exfat_links:
                priority_candidates.append({
                    "offset": link.offset,
                    "size": 0, # It's a link/artifact
                    "score": link.confidence * 100,
                    "video_id": link.video_id,
                    "url": link.url,
                    "title": link.title,
                    "is_assembled": True, # Virtual file from link
                    "metadata_only": False, # It is content!
                })

            # 0.3: Full exFAT recovery — batch extract
            originals_exfat_dir = dir_manager.get_path('full_original_exfat')
            originals_exfat_dir.mkdir(parents=True, exist_ok=True)
            originals_count = 0

            if deleted and args.full_exfat_recovery and boot_info.get('found', False):
                console.print(f"  Extracting {len(deleted)} deleted files via FAT chain...")

                try:
                    # Batch extract — Rust делает всё за один mmap
                    extracted = exfat_scanner.extract_all_files(args.image, deleted)

                    for orig_name, content_bytes, entry_offset, is_del in extracted:
                        content = bytes(content_bytes)
                        if not content or len(content) < 16:
                            continue

                        # Сохраняем с оригинальным именем
                        ext = Path(orig_name).suffix if '.' in orig_name else ''
                        safe_name = "".join(c for c in orig_name if c.isalnum() or c in "._- ()")
                        if not safe_name:
                            safe_name = f"recovered_0x{entry_offset:X}"

                        # Уникальное имя (если дубликаты)
                        save_path = originals_exfat_dir / safe_name
                        counter = 1
                        while save_path.exists():
                            stem = Path(safe_name).stem
                            save_path = originals_exfat_dir / f"{stem}_{counter}{ext}"
                            counter += 1

                        save_path.write_bytes(content)

                        # Метаданные
                        import hashlib
                        meta = {
                            "original_filename": orig_name,
                            "entry_offset": f"0x{entry_offset:X}",
                            "size_bytes": len(content),
                            "is_deleted": is_del,
                            "sha256": hashlib.sha256(content).hexdigest(),
                            "recovery_method": "exfat_fat_chain",
                        }
                        meta_path = save_path.parent / f"{save_path.stem}.meta.json"
                        meta_path.write_text(json.dumps(meta, indent=2, ensure_ascii=False))

                        console.print(f"  [green]+ {save_path.name}[/green] ({len(content)/1024:.1f} KB)")
                        originals_count += 1

                        # Добавляем как кандидат для дальнейшей обработки (link extraction)
                        priority_candidates.append({
                            "offset": entry_offset,
                            "size": len(content),
                            "score": 99.0,
                            "video_id": orig_name,
                            "url": "exfat_original_recovery",
                            "data": content,
                            "is_assembled": True,
                            "original_filename": orig_name,
                            "metadata_only": False,
                        })

                except Exception as e:
                    console.print(f"  [yellow]Batch extract failed: {e} — trying individual...[/yellow]")
                    logging.warning(f"Batch exFAT extract failed: {e}")

            # 0.4: Fallback — для файлов без успешного batch extract
            for f in deleted:
                # Проверяем, не был ли уже извлечён
                already_extracted = any(
                    c.get('offset') == f.offset for c in priority_candidates
                )
                if already_extracted:
                    continue

                cand = {
                    "offset": f.data_offset if f.data_offset > 0 else 0,
                    "size": f.size,
                    "score": 90.0,
                    "video_id": f.filename or 'deleted_file',
                    "url": f"exfat_metadata:{f.filename or 'unnamed'}",
                    "metadata_only": True,
                }

                # Пробуем extract_file (простой, без entry parsing)
                if f.first_cluster >= 2 and f.size > 0 and f.size < 100 * 1024 * 1024:
                    try:
                        content = bytes(exfat_scanner.extract_file(
                            args.image, f.first_cluster, f.size, f.no_fat_chain
                        ))
                        if content and len(content) > 16:
                            cand["data"] = content
                            cand["is_assembled"] = True
                            cand["url"] = "exfat_fallback_recovery"
                    except Exception as e:
                        logging.warning(f"exFAT fallback for 0x{f.offset:X}: {e}")

                priority_candidates.append(cand)

            dashboard.add_log(f"Phase 0: {len(deleted)} deleted, {originals_count} fully extracted")
            console.print(f"  Full files extracted: {originals_count} of {len(deleted)} deleted entries")

        except ImportError:
            console.print("  [yellow]Rust accelerator not available — skipping exFAT scan[/yellow]")
        except Exception as e:
            console.print(f"  [yellow]Phase 0 error: {e}[/yellow]")
            logging.error(f"Phase 0 failed: {e}", exc_info=True)

    # ----------------------------------------------------------------
    # Phase 1: Scan
    # ----------------------------------------------------------------
    start_time = time.time()

    # Initialize scanner (Rust with Python fallback)
    scanner = None
    use_python = False

    try:
        # Check for forced restart/fallback
        if os.environ.get("FORCE_PYTHON_SCANNER", "0") == "1":
             raise ImportError("Forced Python scanner")

        import rust_accelerator 
        console.print("[bold green]>>> USING RUST ACCELERATOR (High-Performance Core)[/bold green]")

        scanner = rust_accelerator.RustParallelScanner(
            num_threads=8,
            chunk_size_mb=512 if args.nvme else 256,
            overlap_kb=128,
            deduplicate=True,
            min_confidence=0.1,
        )
    except ImportError as e:
        console.print(f"[bold red]CRITICAL ERROR: Rust accelerator failed to load: {e}[/bold red]")
        console.print("[red]Performance requires Rust. Please rebuild the accelerator or check library paths.[/red]")
        sys.exit(1)
    
    # Run scan
    try:
        if args.no_live:
            # --- Simple text mode (no live dashboard) ---
            console.print("\n[bold]Phase 1: Scanning Disk Image[/bold]")
            console.print(f"  Image: {args.image}")
            console.print(
                f"  Target size: {args.target_size_min}-{args.target_size_max} KB\n"
            )

            result = scanner.scan_streaming(
                args.image,
                start_position,
                args.reverse,
                lambda x: None,
                lambda x: None,
            )
        else:
            # --- Live dashboard mode ---
            dashboard.add_log("Scan initiated")
            if args.reverse:
                dashboard.add_log("Reverse scan mode active")

            with Live(
                dashboard.render(), refresh_per_second=10, screen=True
            ) as live:

                def progress_callback(bytes_scanned):
                    dashboard.update_position(bytes_scanned)

                    # Auto-checkpoint every 10GB (Optimized for speed)
                    state = {
                        "bytes_scanned": bytes_scanned,
                        "fragments_found": dashboard.fragments_found,
                        "recovered_files": dashboard.recovered_files,
                    }
                    # 15GB = 15 * 1024 * 1024 * 1024
                    if checkpoint_mgr.auto_save_interval(15 * 1024 * 1024 * 1024, state):
                        dashboard.add_log(
                            f"Checkpoint saved at {bytes_scanned / 1024 / 1024 / 1024:.1f} GB"
                        )

                    # Handle pause
                    if hotkeys.paused:
                        dashboard.paused = True
                        live.update(dashboard.render())
                        while hotkeys.paused and not hotkeys.quit_requested:
                            time.sleep(0.1)
                        dashboard.paused = False

                    # Handle manual checkpoint
                    if hotkeys.checkpoint_requested:
                        checkpoint_mgr.save(bytes_scanned, state)
                        dashboard.add_log("Manual checkpoint saved")
                        hotkeys.reset_flags()

                    # Handle quit
                    if hotkeys.quit_requested:
                        checkpoint_mgr.save(bytes_scanned, state)
                        dashboard.add_log("Quit requested, saving checkpoint...")
                        live.update(dashboard.render())
                        raise KeyboardInterrupt

                    live.update(dashboard.render())

                def hot_fragment_callback(frag):
                    dashboard.fragments_found += 1
                    dashboard.update_disk_map(frag["offset"], 20)

                    if frag.get("score", 0) > 90:
                        dashboard.top_candidate = frag
                        dashboard.add_log(
                            f"High-confidence fragment at 0x{frag['offset']:X} ({frag['score']:.1f}%)"
                        )

                    live.update(dashboard.render())

                result = scanner.scan_streaming(
                    args.image,
                    start_position,
                    args.reverse,
                    progress_callback,
                    hot_fragment_callback,
                )

                dashboard.add_log("Scan completed")
                live.update(dashboard.render())

        # Cleanup hotkeys
        hotkeys.stop()

        scan_time = time.time() - start_time

        # Final checkpoint
        final_state = {
            "bytes_scanned": image_size,
            "fragments_found": dashboard.fragments_found,
            "recovered_files": 0,
        }
        checkpoint_mgr.save(image_size, final_state)

        # Get links from result
        candidates = priority_candidates # Start with priority candidates
        for link in result.get("links", []):
            candidates.append(
                {
                    "offset": link["offset"],
                    "size": args.target_size_max * 1024,
                    "score": link["confidence"] * 100,
                    "video_id": link.get("video_id", ""),
                    "url": link.get("url", ""),
                }
            )

        console.print(
            f"\nFound {len(candidates)} candidates ({len(priority_candidates)} from metadata) in {scan_time:.1f}s"
        )

    except KeyboardInterrupt:
        console.print("\nScan interrupted. Checkpoint saved.", style="bold yellow")
        # Don't call hotkeys.stop() here, it's in finally
        sys.exit(0)

    except Exception as e:
        console.print(f"\nScan failed: {e}", style="bold red")
        # Don't call hotkeys.stop() here, it's in finally
        sys.exit(1)
        
    finally:
        # ALWAYS restore terminal
        hotkeys.stop()

    if not candidates:
        console.print("No candidates found", style="bold red")
        report_gen = ProfessionalReportGenerator(output_dir)
        scan_results = {
            "image_size_mb": image_size // 1024 // 1024,
            "bytes_scanned_mb": 0,
            "candidates_found": 0,
            "scan_time_sec": scan_time,
            "avg_speed_mbps": 0,
        }
        failure_reasons = [
            "No YouTube URL patterns found in scanned data",
            f"Searched for files between {args.target_size_min}-{args.target_size_max} KB",
            "Try different size ranges or check if disk image is correct",
        ]
        report_path = report_gen.generate_full_report(
            scan_results, [], [], failure_reasons
        )
        console.print(f"Report generated: {report_path}")
        sys.exit(0)

    # ----------------------------------------------------------------
    # Phase 1.5: exFAT Integration Analysis
    # ================================================================
    console.print("\nPhase 1.5: Analyzing exFAT-fragment relationships...", style="bold")
    
    from fragment_assembler import FragmentAssembler
    exfat_assembler = FragmentAssembler(max_gap=1024 * 1024)
    
    # Load exFAT candidates from recovery directory
    exfat_dir = dir_manager.get_path('full_original_exfat')
    exfat_candidates = exfat_assembler.load_exfat_candidates_from_dir(exfat_dir)
    
    if exfat_candidates:
        console.print(f"  Loaded {len(exfat_candidates)} exFAT-recovered files")
        
        # We'll analyze exFAT-fragment relationships in Phase 2.5
        # Store them for later use
        exfat_analysis_data = {
            'candidates': exfat_candidates,
            'assembler': exfat_assembler
        }
    else:
        console.print("  [yellow]No exFAT-recovered files found[/yellow]")
        exfat_analysis_data = None

    # ----------------------------------------------------------------
    # Phase 2: Cluster Analysis
    # ----------------------------------------------------------------
    console.print("\nPhase 2: Cluster analysis...", style="bold")

    cluster_analyzer = ClusterAnalyzer()
    clusters = cluster_analyzer.find_clusters(candidates)
    clusters = cluster_analyzer.merge_overlapping_clusters(clusters)
    clusters = cluster_analyzer.rank_clusters(clusters)

    console.print(f"Found {len(clusters)} data clusters")
    for i, cluster in enumerate(clusters[:5], 1):
        console.print(
            f"  {i}. Offset: 0x{cluster.start_offset:X}, "
            f"Links: {cluster.link_count}, Density: {cluster.density:.2f}"
        )

    # ----------------------------------------------------------------
    # Phase 2.5: Fragment Assembly from Clusters
    # ----------------------------------------------------------------
    console.print("\nPhase 2.5: Checking for fragmented files...", style="bold")
    
    from fragment_assembler import FragmentAssembler
    assembler = FragmentAssembler(max_gap=1024 * 1024) # 1MB gap tolerance for assembling clusters

    # Use training data for Semantic SmartSeparation
    classifier = None
    try:
        from semantic_classifier import SemanticClassifier
        training_dir = Path("semantic_training")
        if training_dir.exists():
            classifier = SemanticClassifier(training_dir)
            console.print("  [green]Semantic Classifier loaded for Smart Separation[/green]")
    except ImportError:
        pass
    except Exception as e:
        logger.warning(f"Failed to load Semantic Classifier: {e}")

    # 1. Convert Clusters to "Fragment Candidates"
    # We read the actual cluster content to see if we can link them
    cluster_fragments = []
    
    if clusters:
        try:
            with open(args.image, "rb") as f:
                for cluster in clusters:
                    # Add padding to capture headers/footers context (e.g. JSON braces)
                    PADDING = 4096
                    start_pos = max(0, cluster.start_offset - PADDING)
                    end_pos = cluster.end_offset + PADDING
                    read_size = end_pos - start_pos
                    
                    if read_size < 1: read_size = 4096
                    
                    # Read data
                    try:
                        f.seek(start_pos)
                        data = f.read(read_size)
                        
                        cluster_fragments.append({
                            "offset": start_pos,
                            "size": read_size,
                            "data": data,
                            "links": cluster.links,  # Use links from analysis
                            "score": 80.0,
                            "is_fragment": True
                        })
                    except Exception as e:
                        logging.warning(f"Failed to read cluster at {cluster.start_offset}: {e}")
        except Exception as e:
            console.print(f"[yellow]  Warning: Could not open image for phase 2.5: {e}[/yellow]")

    # 2. Analyze exFAT-fragment relationships (V11.0 Integration)
    exfat_analysis_results = None
    if exfat_analysis_data and cluster_fragments:
        console.print("  Analyzing exFAT-fragment relationships...")
        try:
            exfat_analysis_results = exfat_analysis_data['assembler'].analyze_exfat_candidates(
                exfat_analysis_data['candidates'],
                cluster_fragments
            )
            
            if exfat_analysis_results['statistics']['potential_matches'] > 0:
                console.print(
                    f"  [green]Found {exfat_analysis_results['statistics']['potential_matches']} "
                    f"exFAT-fragment matches (confidence: "
                    f"{exfat_analysis_results['statistics']['confidence_score']:.0f}%)[/green]"
                )
                
                if exfat_analysis_results['fragmented_files']:
                    console.print(
                        f"  {len(exfat_analysis_results['fragmented_files'])} exFAT files have linked fragments"
                    )
        except Exception as e:
            console.print(f"  [yellow]Warning: exFAT analysis failed: {e}[/yellow]")
            logging.warning(f"exFAT analysis error: {e}")
    
    # 3. Try to assemble these cluster fragments
    if cluster_fragments:
        # V11.2: Use multi-file detection to handle 2-3+ files
        assembled_files = assembler.assemble_multiple_files(cluster_fragments, classifier=classifier)
        
        if assembled_files:
            console.print(f"  Found {len(assembled_files)} assembled file(s)")
            for file_idx, assembled in enumerate(assembled_files, 1):
                size_kb = assembled.total_size // 1024
                console.print(
                    f"  File {file_idx}: {len(assembled.fragments)} fragments "
                    f"-> {size_kb} KB "
                    f"(confidence: {assembled.confidence:.0f}%)"
                )
                candidates.append(
                    {
                        "offset": assembled.fragments[0].offset if assembled.fragments else 0,
                        "size": assembled.total_size,
                        "data": assembled.content,
                        "final_score": assembled.confidence,
                        "is_assembled": True,
                        "assembly_source": "fragment_assembler_v11.2",
                        "video_id": f"assembled_file_{file_idx}",
                        "url": "fragment_assembly",
                        "file_number": file_idx,
                        "total_files": len(assembled_files)
                    }
                )
                    
    # 4. Add exFAT analysis results to candidates (V11.0)
    if exfat_analysis_results:
        # Add fragmented exFAT files with linked fragments
        for frag_file in exfat_analysis_results['fragmented_files']:
            if frag_file['linked_fragments']:
                candidates.append({
                    "offset": frag_file['offset'],
                    "size": frag_file['size'],
                    "data": b'',  # Data will be loaded during reconstruction
                    "final_score": 85.0,  # High confidence for exFAT-linked files
                    "is_assembled": False,
                    "assembly_source": "exfat_integration",
                    "video_id": frag_file['filename'],
                    "url": "exfat_recovery",
                    "exfat_metadata": {
                        'filename': frag_file['filename'],
                        'sha256': frag_file['sha256'],
                        'linked_fragments_count': len(frag_file['linked_fragments']),
                        'is_deleted': frag_file['is_deleted']
                    }
                })
        
        if exfat_analysis_results['fragmented_files']:
            console.print(
                f"  [green]Added {len(exfat_analysis_results['fragmented_files'])} "
                f"exFAT-linked files to candidates[/green]"
            )
    
    # 5. Also add the raw cluster fragments themselves as candidates!
    # If a cluster is dense, it might be a valid file even if not assembled from multiple pieces
    # (or it was merged by ClusterAnalyzer into one piece)
    count_clusters = 0
    for frag in cluster_fragments:
        # Check if this fragment was already used in an assembly?
        # Actually, adding it as a separate candidate is safer (retry strategy).
        # Give it a unique ID
        count_clusters += 1
        candidates.append({
            "offset": frag["offset"],
            "size": frag["size"],
            "data": frag["data"],
            "final_score": frag.get("score", 70.0),
            "is_assembled": True, # Treated as 'read' data
            "video_id": f"cluster_{count_clusters}",
            "url": "cluster_recovery"
        })
    if count_clusters > 0:
        console.print(f"  Added {count_clusters} dense clusters for reconstruction")



    # ----------------------------------------------------------------
    # Phase 3: File Reconstruction
    # ----------------------------------------------------------------
    console.print("\nPhase 3: Reconstructing files...", style="bold")

    reconstructor = FileReconstructor()
    candidate_manager = CandidateManager(output_dir)

    recovered_files = []

    try:
        with open(args.image, "rb") as img_file:
            for i, cand in enumerate(candidates, 1):
                offset = cand.get("offset", 0)
                size = cand.get("size", 0)

                # Skip size check for assembled/verified fragments
                if not cand.get("is_assembled") and not cand.get("is_fragment"):
                    if size < args.target_size_min * 1024 or size > args.target_size_max * 1024:
                        continue

                if cand.get("is_assembled", False) or cand.get("is_fragment", False):
                    data = cand.get("data", b"")
                else:
                    try:
                        # V11.0: Dynamic chunk size by entropy + structure + link density
                        read_size = args.chunk_max * 1024
                        img_file.seek(offset)
                        data_raw = img_file.read(read_size)

                        actual_size = reconstructor.compute_dynamic_chunk_size(
                            data_raw,
                            chunk_min=args.chunk_min * 1024,
                            chunk_max=args.chunk_max * 1024,
                        )
                        data = data_raw[:actual_size]
                        size = len(data)

                    except (OSError, IOError) as e:
                        msg = f"Error reading image at {offset}: {e}"
                        logging.error(msg)
                        console.print(f"[red]{msg}[/red]")
                        continue

                metadata = {
                    "offset": offset,
                    "size": size,
                    "score": cand.get("final_score", 0),
                }
                
                if cand.get("original_filename"):
                    metadata["original_filename"] = cand["original_filename"]
                
                cand_path = candidate_manager.add_candidate(data, metadata)
                
                if not cand_path:
                    # Ошибка #20: Пропуск при нехватке места
                    continue

                try:
                    result = reconstructor.reconstruct(data, offset, offset + size)

                    if result.is_valid and result.confidence >= 50:
                        candidate_manager.validate_candidate(
                            cand_path, True, f"Confidence: {result.confidence:.0f}%"
                        )

                        # Determine recovery directory and filename
                        custom_dir = None
                        custom_name = None
                        
                        if cand.get("assembly_source") == "fragment_assembler":
                            custom_dir = dir_manager.get_path('assembled')
                        elif cand.get("url") in ["exfat_full_recovery", "exfat_original_recovery"]:
                             # Preserve original filename for exFAT
                             custom_name = cand.get("video_id") # contains filename
                        
                        recovered_path = candidate_manager.recover_candidate(
                            cand_path,
                            result.content,
                            result.file_type.value,
                            result.confidence,
                            result.links_extracted,
                            cleaned_content=result.cleaned_content,
                            links_only=args.links_only
                        )
                        
                        # V10.2: Move to special folders if needed
                        if recovered_path and recovered_path.exists():
                            if custom_dir:
                                new_path = custom_dir / recovered_path.name
                                try:
                                    recovered_path.replace(new_path)
                                    recovered_path = new_path
                                except Exception: pass
                            elif custom_name:
                                # Safe rename with original exFAT name
                                clean_name = "".join(c for c in custom_name if c.isalnum() or c in "._- ")
                                if not clean_name: clean_name = recovered_path.name
                                new_path = recovered_path.parent / f"exfat_{clean_name}"
                                try:
                                    recovered_path.replace(new_path)
                                    recovered_path = new_path
                                except Exception: pass

                        filename = recovered_path.name if recovered_path else "EXTRACTED_ONLY"
                        recovered_files.append(
                            {
                                "filename": filename,
                                "file_type": result.file_type.value,
                                "confidence": result.confidence,
                                "links": result.links_extracted,
                                "size_kb": len(result.content) // 1024,
                                "sha256": metadata.get("sha256", "N/A"),
                            }
                        )
                        
                        msg = f"Phase 3: Recovered {filename} from {cand.get('url', 'search')} (Confidence: {result.confidence:.0f}%, {len(result.links_extracted)} links)"
                        logging.info(msg)
                        console.print(f"  {len(recovered_files)}: [green]{filename}[/green] ({len(result.links_extracted)} links)")

                        dashboard.recovered_files = len(recovered_files)

                        if (
                            args.early_exit > 0
                            and len(recovered_files) >= args.early_exit
                        ):
                            console.print(
                                f"  Early exit: {args.early_exit} files recovered"
                            )
                            break
                    else:
                        candidate_manager.validate_candidate(
                            cand_path,
                            False,
                            f"Low confidence: {result.confidence:.0f}%",
                        )


                except Exception as e:
                    logging.error(f"Candidate failure at {offset}: {e}")
                    candidate_manager.fail_candidate(cand_path, str(e))
                
    except FileNotFoundError:
        msg = f"Error: Image file not found: {args.image}"
        logging.critical(msg)
        console.print(f"[bold red]{msg}[/bold red]")
        sys.exit(1)
    except IOError as e:
        msg = f"Error opening image file: {e}"
        logging.critical(msg)
        console.print(f"[bold red]{msg}[/bold red]")
        sys.exit(1)

    # ----------------------------------------------------------------
    # Phase 4: Semantic Assembly (V11.5 addition)
    # ----------------------------------------------------------------
    if args.semantic_scan:
        console.print("\nPhase 4: Semantic Assembly...", style="bold")
        
        try:
            from semantic_classifier import SemanticClassifier
            from semantic_assembler import SemanticAssembler
            
            # Use training data from semantic_training folder
            training_dir = Path("semantic_training")
            if not training_dir.exists():
                console.print("  [yellow]No 'semantic_training' directory found. Using default keywords.[/yellow]")
            
            classifier = SemanticClassifier(training_dir)
            
            sem_assembler = SemanticAssembler(output_dir, classifier)
            
            # We scan the candidates folder (both validated and rejected/temp?)
            # Usually we want to scan things that FAILED to be recovered properly, 
            # OR raw candidates that were just carved. 
            # CandidateManager puts raw data in 00_CANDIDATES/cand_.../raw.bin
            
            candidates_root = output_dir / "00_CANDIDATES"
            if candidates_root.exists():
                 sem_assembler.process_candidates(candidates_root)
                 console.print("  [green]Semantic grouping complete. Check 07_SEMANTIC_GROUPS[/green]")
            else:
                 console.print("  [yellow]No 00_CANDIDATES directory found to scan.[/yellow]")
                 
        except Exception as e:
            console.print(f"  [red]Semantic assembly failed: {e}[/red]")
            logging.error(f"Semantic assembly failed: {e}", exc_info=True)

    cleaned = candidate_manager.cleanup_candidates(keep_rejected=False)
    console.print(f"  Cleaned {cleaned} temporary files")

    # ----------------------------------------------------------------
    # Phase 5: Generate Report
    # ----------------------------------------------------------------
    console.print("\nPhase 5: Generating professional report...", style="bold")

    report_gen = ProfessionalReportGenerator(output_dir)

    scan_results = {
        "image_size_mb": image_size // 1024 // 1024,
        "bytes_scanned_mb": image_size // 1024 // 1024,
        "candidates_found": len(candidates),
        "scan_time_sec": scan_time,
        "avg_speed_mbps": (image_size / 1024 / 1024) / max(scan_time, 1),
    }

    report_path = report_gen.generate_full_report(
        scan_results, clusters, recovered_files
    )
    console.print(f"Report: {report_path}")

    # ----------------------------------------------------------------
    # Phase 6: Generate HTML Index
    # ----------------------------------------------------------------
    console.print("\nPhase 6: Generating navigation index...", style="bold")

    index_gen = IndexGenerator(output_dir)

    index_files = []
    for rf in recovered_files:
        index_files.append(
            {
                "filename": rf.get("filename", "unknown"),
                "type": rf.get("file_type", "other"),
                "size_kb": rf.get("size_kb", 0),
                "quality": int(rf.get("confidence", 0)),
                "links_count": len(rf.get("links", [])),
                "offset": 0,
                "sha256": rf.get("sha256", "N/A"),
            }
        )

    index_path = index_gen.save_index(index_files)
    console.print(f"Index: {index_path}")

    # ----------------------------------------------------------------
    # Phase 7: Export Global Links (v10 specialized)
    # ----------------------------------------------------------------
    if args.links_only or recovered_files:
        console.print("\nPhase 7: Exporting all unique links...", style="bold")
        all_unique_links = set()
        for rf in recovered_files:
            all_unique_links.update(rf.get("links", []))
            
        links_file_txt = output_dir / "03_EXTRACTED_LINKS" / "all_links.txt"
        links_file_json = output_dir / "03_EXTRACTED_LINKS" / "all_links.json"
        
        if links_file_txt.exists():
            logging.warning(f"Overwriting existing links file: {links_file_txt}")
        with open(links_file_txt, "w") as f:
            for link in sorted(list(all_unique_links)):
                f.write(f"{link}\n")
                
        if links_file_json.exists():
            logging.warning(f"Overwriting existing links file: {links_file_json}")
        with open(links_file_json, "w") as f:
            json.dump({"total_links": len(all_unique_links), "links": sorted(list(all_unique_links))}, f, indent=2)
            
        console.print(f"  Exported {len(all_unique_links)} unique links to {links_file_txt}")

    # Update session info with results
    session_info["end_time"] = time.strftime("%Y-%m-%dT%H:%M:%S")
    session_info["results"] = {
        "bytes_scanned": image_size,
        "fragments_found": dashboard.fragments_found,
        "files_recovered": len(recovered_files),
        "scan_duration_sec": round(scan_time, 1),
    }
    dir_manager.save_session_info(session_info)

    # Final checkpoint update
    final_state = {
        "bytes_scanned": image_size,
        "fragments_found": dashboard.fragments_found,
        "recovered_files": len(recovered_files),
    }
    checkpoint_mgr.save(image_size, final_state)

    # ----------------------------------------------------------------
    # Summary
    # ----------------------------------------------------------------
    console.print()
    console.print("-" * 52)
    console.print(" RECOVERY COMPLETE", style="bold")
    console.print("-" * 52)
    console.print(f"  Candidates: {len(candidates)}")
    console.print(f"  Recovered:  {len(recovered_files)}")
    console.print(f"  Time:       {time.time() - start_time:.1f}s")
    console.print(f"  Output:     {output_dir}/")
    console.print()
    console.print("  Directory Structure:")
    console.print(f"    00_FULL_ORIGINAL_EXFAT/       - Full files from exFAT (original names)")
    console.print(f"    01_RECOVERED_FILES/           - Recovered files (by type)")
    console.print(f"    02_ASSEMBLED_FROM_FRAGMENTS/  - Assembled from fragments")
    console.print(f"    03_EXTRACTED_LINKS/           - Extracted links")
    console.print(f"    04_METADATA/                  - Metadata")
    console.print(f"    05_REPORTS/                   - Reports")
    console.print(f"    07_SEMANTIC_GROUPS/           - Semantic groups (Trading, Psychology...)")
    console.print()
    console.print("  Quick Start:")
    console.print(f"    1. Open INDEX.md for navigation")
    console.print(f"    2. View INDEX.html in browser")
    console.print(f"    3. Files in 01_RECOVERED_FILES/ sorted by type")
    console.print()


if __name__ == "__main__":
    main()
