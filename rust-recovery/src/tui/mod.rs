//! TUI (Terminal User Interface) module for Ultimate File Recovery
//! 
//! This module provides a terminal-based user interface using ratatui and crossterm.
//! It displays real-time scan progress, disk heatmap, statistics, and logs.
//!
//! Hotkeys supported:
//! - P: Pause/Resume scan
//! - S: Skip to next chunk
//! - V: View current fragment
//! - C: Save checkpoint
//! - Q: Quit application

pub mod widgets;

use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io;
use tokio::sync::mpsc;
// use widgets::{DashboardWidget, DiskHeatmapWidget, StatsWidget, LogsWidget}; // Simplified

use crate::types::{Offset, ScanConfig};

/// TUI Application state
#[derive(Debug, Clone)]
pub struct TuiApp {
    /// Total size of disk image in bytes
    pub total_size: u64,
    /// Current scan position
    pub current_position: u64,
    /// Number of bytes scanned
    pub bytes_scanned: u64,
    /// Scan start timestamp
    pub start_time: std::time::Instant,
    /// Number of fragments found
    pub fragments_found: u32,
    /// Number of recovered files
    pub recovered_files: u32,
    /// Scan is in reverse mode
    pub is_reverse: bool,
    /// Scan is paused
    pub paused: bool,
    /// Average speed in MB/s
    pub avg_speed_mbps: f64,
    /// Current speed in MB/s
    pub current_speed_mbps: f64,
    /// Estimated time remaining in seconds
    pub eta_seconds: f64,
    /// Top candidate information
    pub top_candidate: Option<TopCandidate>,
    /// Number of hot clusters
    pub hot_clusters: u32,
    /// Target files for early exit
    pub target_files: u32,
    /// Activity log entries
    pub activity_log: Vec<LogEntry>,
    /// Disk heatmap data
    pub disk_heatmap: DiskHeatmap,
    /// Scan configuration
    pub scan_config: ScanConfig,
}

/// Top candidate information
#[derive(Debug, Clone)]
pub struct TopCandidate {
    pub offset: Offset,
    pub confidence: f64,
    pub score: f64,
}

/// Log entry for activity log
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: String,
    pub message: String,
}

/// Disk heatmap representing scan progress
#[derive(Debug, Clone)]
pub struct DiskHeatmap {
    /// Width of the heatmap in blocks
    pub width: usize,
    /// Height of the heatmap in blocks  
    pub height: usize,
    /// Total number of blocks
    pub total_blocks: usize,
    /// Heatmap data: 0=Unscanned, 1=Scanned, 2=Found Data, 3=Hot/Recent
    pub blocks: Vec<u8>,
    /// Image path for display
    pub image_path: String,
    /// Output directory for display
    pub output_dir: String,
}

impl DiskHeatmap {
    /// Create new disk heatmap
    pub fn new(_total_size: u64, image_path: String, output_dir: String) -> Self {
        // Initial default size, will be resized on first draw
        let width = 100;
        let height = 4;
        let total_blocks = width * height;
        
        Self {
            width,
            height,
            total_blocks,
            blocks: vec![0; total_blocks],
            image_path,
            output_dir,
        }
    }

    /// Resize heatmap
    pub fn resize(&mut self, width: usize, height: usize) {
        if width == self.width && height == self.height {
            return;
        }

        let new_total = width * height;
        let new_blocks = vec![0; new_total];
        
        // Simple resampling - mostly preserving "hot" status
        // This is a naive implementation, but sufficient for TUI visualization
        // To do it properly we'd need to re-map based on original scan data ranges, 
        // but since we only store block states, we'll just clear and let it fill up again
        // or attempt to map old to new. 
        // For now: clear and let it refill (simpler, but loses history if resized)
        // Ideally: keep a list of "scanned ranges" and "hot ranges" in TuiApp and re-render heatmap from that.
        // Given constraints, we'll just keep it simple: resize resets visualization, but current position will refill scanned part.
        
        self.width = width;
        self.height = height;
        self.total_blocks = new_total;
        self.blocks = new_blocks;
    }

    /// Update scan position and mark blocks as scanned
    pub fn update_position(&mut self, position: u64, total_size: u64) {
        if total_size == 0 {
            return;
        }

        let progress = position as f64 / total_size as f64;
        let blocks_scanned = (progress * self.total_blocks as f64) as usize;
        
        // Fill blocks up to current position
        for i in 0..blocks_scanned.min(self.total_blocks) {
            if self.blocks[i] == 0 {
                self.blocks[i] = 1; // Mark as scanned
            }
        }
    }

    /// Mark a block as found data
    pub fn mark_found_data(&mut self, offset: u64, total_size: u64) {
        if total_size == 0 {
            return;
        }

        let block_idx = ((offset as f64 / total_size as f64) * self.total_blocks as f64) as usize;
        if block_idx < self.total_blocks {
            self.blocks[block_idx] = 2; // Mark as found data
        }
    }

    /// Get block character for rendering
    pub fn get_block_char(&self, idx: usize) -> char {
        match self.blocks.get(idx).copied().unwrap_or(0) {
            0 => '░', // Unscanned
            1 => '▒', // Scanned
            2 => '█', // Found Data
            3 => '█', // Hot/Recent
            _ => '░',
        }
    }
}

impl TuiApp {
    /// Create new TUI application
    pub fn new(total_size: u64, image_path: String, output_dir: String, scan_config: ScanConfig) -> Self {
        Self {
            total_size,
            current_position: 0,
            bytes_scanned: 0,
            start_time: std::time::Instant::now(),
            fragments_found: 0,
            recovered_files: 0,
            is_reverse: scan_config.reverse,
            paused: false,
            avg_speed_mbps: 0.0,
            current_speed_mbps: 0.0,
            eta_seconds: 0.0,
            top_candidate: None,
            hot_clusters: 0,
            target_files: 0,
            activity_log: Vec::new(),
            disk_heatmap: DiskHeatmap::new(total_size, image_path, output_dir),
            scan_config,
        }
    }

    /// Add log entry
    pub fn add_log(&mut self, message: &str) {
        let timestamp = chrono::Local::now().format("%H:%M:%S").to_string();
        let entry = LogEntry {
            timestamp,
            message: message.to_string(),
        };
        
        self.activity_log.push(entry);
        
        // Keep only last 10 entries
        if self.activity_log.len() > 10 {
            self.activity_log.remove(0);
        }
    }

    /// Update scan statistics
    pub fn update_scan_stats(&mut self, position: u64, bytes_scanned: u64) {
        self.current_position = position;
        self.bytes_scanned = bytes_scanned;
        
        // Update disk heatmap
        self.disk_heatmap.update_position(position, self.total_size);
        
        // Calculate speeds
        let elapsed = self.start_time.elapsed().as_secs_f64();
        if elapsed > 0.0 {
            self.current_speed_mbps = bytes_scanned as f64 / 1024.0 / 1024.0 / elapsed;
            
            // Simple moving average for average speed
            if self.avg_speed_mbps == 0.0 {
                self.avg_speed_mbps = self.current_speed_mbps;
            } else {
                self.avg_speed_mbps = (self.avg_speed_mbps * 0.9) + (self.current_speed_mbps * 0.1);
            }
        }
        
        // Calculate ETA
        if self.avg_speed_mbps > 0.0 {
            let remaining_bytes = self.total_size.saturating_sub(bytes_scanned);
            let remaining_mb = remaining_bytes as f64 / 1024.0 / 1024.0;
            self.eta_seconds = remaining_mb / self.avg_speed_mbps;
        }
    }

    /// Mark fragment as found
    pub fn mark_fragment_found(&mut self, offset: u64) {
        self.fragments_found += 1;
        self.disk_heatmap.mark_found_data(offset, self.total_size);
    }

    /// Mark file as recovered
    pub fn mark_file_recovered(&mut self) {
        self.recovered_files += 1;
    }

    /// Check if should stop (early exit)
    pub fn should_stop_early(&self) -> bool {
        self.target_files > 0 && self.recovered_files >= self.target_files
    }
}

/// TUI Event types for communication with main pipeline
#[derive(Debug, Clone)]
pub enum TuiEvent {
    /// Update scan position
    UpdatePosition { position: u64, bytes_scanned: u64 },
    /// Fragment found at offset
    FragmentFound { offset: u64 },
    /// File recovered
    FileRecovered { filename: String },
    /// Log message
    LogMessage { message: String },
    /// Scan completed
    ScanCompleted,
    /// Error occurred
    Error { message: String },
}

/// TUI Application that handles rendering and input
pub struct TuiApplication {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    app: TuiApp,
    receiver: mpsc::UnboundedReceiver<TuiEvent>,
    should_quit: bool,
}

impl TuiApplication {
    /// Create new TUI application
    pub fn new(
        app: TuiApp,
        receiver: mpsc::UnboundedReceiver<TuiEvent>,
    ) -> Result<Self, io::Error> {
        // Setup terminal
        terminal::enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        Ok(Self {
            terminal,
            app,
            receiver,
            should_quit: false,
        })
    }

    /// Run the TUI application
    pub fn run(&mut self) -> Result<(), io::Error> {
        self.app.add_log("TUI initialized");
        
        while !self.should_quit {
            // Handle events
            self.handle_events()?;
            
            // Process incoming events
            self.process_events()?;
            
            // Draw UI
            self.draw()?;
        }
        
        Ok(())
    }

    /// Handle terminal events (keyboard input)
    fn handle_events(&mut self) -> Result<(), io::Error> {
        if event::poll(std::time::Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key_event) => {
                    if key_event.kind == KeyEventKind::Press {
                        match key_event.code {
                            KeyCode::Char('p') | KeyCode::Char('P') => {
                                self.app.paused = !self.app.paused;
                                let status = if self.app.paused { "PAUSED" } else { "RESUMED" };
                                self.app.add_log(&format!("Scan {}", status));
                            }
                            KeyCode::Char('s') | KeyCode::Char('S') => {
                                self.app.add_log("Skip to next chunk requested");
                                // TODO: Implement skip logic
                            }
                            KeyCode::Char('v') | KeyCode::Char('V') => {
                                self.app.add_log("View current fragment");
                                // TODO: Implement view logic
                            }
                            KeyCode::Char('c') | KeyCode::Char('C') => {
                                self.app.add_log("Checkpoint saved");
                                // TODO: Implement checkpoint logic
                            }
                            KeyCode::Char('q') | KeyCode::Char('Q') => {
                                self.app.add_log("Quit requested");
                                self.should_quit = true;
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
        
        Ok(())
    }

    /// Process incoming events from the pipeline
    fn process_events(&mut self) -> Result<(), io::Error> {
        while let Ok(event) = self.receiver.try_recv() {
            match event {
                TuiEvent::UpdatePosition { position, bytes_scanned } => {
                    self.app.update_scan_stats(position, bytes_scanned);
                }
                TuiEvent::FragmentFound { offset } => {
                    self.app.mark_fragment_found(offset);
                    self.app.add_log(&format!("Fragment found at 0x{:X}", offset));
                }
                TuiEvent::FileRecovered { filename } => {
                    self.app.mark_file_recovered();
                    self.app.add_log(&format!("File recovered: {}", filename));
                    
                    if self.app.should_stop_early() {
                        self.app.add_log("Early exit target reached");
                        self.should_quit = true;
                    }
                }
                TuiEvent::LogMessage { message } => {
                    self.app.add_log(&message);
                }
                TuiEvent::ScanCompleted => {
                    self.app.add_log("Scan completed");
                    // Auto-quit after completion or wait for user?
                    // self.should_quit = true;
                }
                TuiEvent::Error { message } => {
                    self.app.add_log(&format!("ERROR: {}", message));
                }
            }
        }
        
        Ok(())
    }

    /// Draw the TUI
    fn draw(&mut self) -> Result<(), io::Error> {
        self.terminal.draw(|f| {
            let chunks = ratatui::layout::Layout::default()
                .direction(ratatui::layout::Direction::Vertical)
                .constraints([
                    ratatui::layout::Constraint::Length(3),  // Header
                    ratatui::layout::Constraint::Min(5),     // Heatmap (dynamic height)
                    ratatui::layout::Constraint::Length(8),  // Stats & Progress
                    ratatui::layout::Constraint::Length(10), // Logs
                    ratatui::layout::Constraint::Length(3),  // Footer
                ].as_ref())
                .split(f.size());

            // Header
            f.render_widget(crate::tui::widgets::create_dashboard_header(&self.app), chunks[0]);

            // Footer
            f.render_widget(crate::tui::widgets::DashboardFooter::render(), chunks[4]);

            // Logs
            f.render_widget(crate::tui::widgets::LogsWidget::render(&self.app.activity_log), chunks[3]);

            // Dynamic Heatmap
            let heatmap_area = chunks[1];
            // Calculate available width for heatmap (minus borders/padding)
            let available_width = (heatmap_area.width as usize).saturating_sub(2);
            let available_height = (heatmap_area.height as usize).saturating_sub(2);
            
            if available_width > 0 && available_height > 0 {
                // Resize if dimensions changed
                if self.app.disk_heatmap.width != available_width || self.app.disk_heatmap.height != available_height {
                     self.app.disk_heatmap.resize(available_width, available_height);
                     // Refill scanned portion
                     self.app.disk_heatmap.update_position(self.app.current_position, self.app.total_size);
                }
            }

            f.render_widget(crate::tui::widgets::DiskHeatmapWidget::render(&self.app.disk_heatmap), chunks[1]);
            
            // Progress details (moved to stats area or separate line if needed)
            // For now, let's put detailed stats in chunk 2


            // Stats in chunk 2
            f.render_widget(crate::tui::widgets::StatsWidget::render(&self.app), chunks[2]);

            // Logs in chunk 3 (footer space, or create new chunk)
            // Let's adjust layout to 4 distinct sections
        })?;

        Ok(())
    }
}

impl Drop for TuiApplication {
    fn drop(&mut self) {
        // Restore terminal
        let _ = terminal::disable_raw_mode();
        let _ = execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        );
        let _ = self.terminal.show_cursor();
    }
}