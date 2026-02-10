//! TUI Widgets for Ultimate File Recovery
//! 
//! This module contains individual widget components for the TUI including
//! disk heatmap, statistics, logs, and dashboard elements.

use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Gauge, List, ListItem, Paragraph, Widget,
    },
};

/// Create dashboard header widget
pub fn create_dashboard_header(app: &super::TuiApp) -> impl Widget {
    let img_name = if let Some(pos) = app.disk_heatmap.image_path.rfind('/') {
        &app.disk_heatmap.image_path[pos + 1..]
    } else {
        &app.disk_heatmap.image_path
    };

    let title = format!(
        "Ultimate Recovery v12.0 - {} -> {}/",
        img_name, app.disk_heatmap.output_dir
    );

    let subtitle = if app.paused {
        "** PAUSED **".to_string()
    } else {
        format!(
            "[{}] {:.1}% ({:.1}/{:.1} GB) | {:.1} MB/s avg | ETA: {}",
            if app.is_reverse { "REVERSE" } else { "FORWARD" },
            if app.total_size > 0 {
                (app.bytes_scanned as f64 / app.total_size as f64) * 100.0
            } else {
                0.0
            },
            app.bytes_scanned as f64 / 1024.0 / 1024.0 / 1024.0,
            app.total_size as f64 / 1024.0 / 1024.0 / 1024.0,
            app.avg_speed_mbps,
            format_duration(app.eta_seconds)
        )
    };

    Paragraph::new(vec![
        Line::from(Span::styled(title, Style::default().fg(Color::White).add_modifier(Modifier::BOLD))),
        Line::from(Span::styled(subtitle, if app.paused { Color::Cyan } else { Color::Green })),
    ])
    .style(Style::default().bg(Color::Black))
    .block(Block::default().borders(Borders::ALL).border_type(BorderType::Plain))
}

/// Dashboard footer widget
pub struct DashboardFooter;

impl DashboardFooter {
    pub fn new() -> Self {
        DashboardFooter
    }
    
    pub fn render() -> impl Widget {
        Paragraph::new("Controls: [P]ause  [S]kip  [V]iew  [C]heckpoint  [Q]uit")
            .style(Style::default().fg(Color::Gray))
            .alignment(ratatui::layout::Alignment::Center)
            .block(Block::default().borders(Borders::ALL).border_type(BorderType::Plain))
    }
}

/// Disk heatmap widget
pub struct DiskHeatmapWidget;

impl DiskHeatmapWidget {
    pub fn render(heatmap: &super::DiskHeatmap) -> impl Widget + use<'_> {
        let block = Block::default()
            .title("Disk Map - Linear Surface Scan")
            .borders(Borders::ALL)
            .border_type(BorderType::Plain);

        let mut chunks = Vec::new();
        for row in 0..heatmap.height {
            let start_idx = row * heatmap.width;
            let end_idx = start_idx + heatmap.width;
            
            let row_spans: Vec<Span> = heatmap.blocks[start_idx..end_idx]
                .iter()
                .enumerate()
                .map(|(i, &state)| {
                    let style = match state {
                        0 => Style::default().fg(Color::DarkGray),    // Unscanned
                        1 => Style::default().fg(Color::Blue),        // Scanned
                        2 => Style::default().fg(Color::Green).add_modifier(Modifier::BOLD), // Found Data
                        3 => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),   // Hot/Recent
                        _ => Style::default().fg(Color::DarkGray),
                    };
                    Span::styled(heatmap.get_block_char(start_idx + i).to_string(), style)
                })
                .collect();
            
            chunks.push(Line::from(row_spans));
        }

        let _legend = vec![
            Line::from(vec![
                Span::styled("░ Unscanned  ", Style::default().fg(Color::DarkGray)),
                Span::styled("▒ Scanned  ", Style::default().fg(Color::Blue)),
                Span::styled("█ Found Data", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            ]),
        ];

        Paragraph::new("Legend:")
            .style(Style::default().fg(Color::Yellow))
            .scroll((0, 0))
            .block(block.clone())
    }
}

/// Statistics widget
pub struct StatsWidget;

impl StatsWidget {
    pub fn render(app: &super::TuiApp) -> impl Widget + use<'_> {
        let stats_text = format!(
            "Fragments:      {:<10} Clusters:        {}\n\
             Top candidate:  {}\n\
             Recovered:      {} files{}\n\
             Checkpoint:     auto-saved at {:.1} GB",
            app.fragments_found,
            app.hot_clusters,
            match &app.top_candidate {
                Some(cand) => format!(
                    "0x{:X} (confidence {:.1}%)",
                    cand.offset.as_u64(),
                    cand.confidence * 100.0
                ),
                None => "None".to_string(),
            },
            app.recovered_files,
            if app.target_files > 0 {
                format!(" (target: {}, early-exit enabled)", app.target_files)
            } else {
                String::new()
            },
            app.bytes_scanned as f64 / 1024.0 / 1024.0 / 1024.0
        );

        Paragraph::new(stats_text)
            .style(Style::default().fg(Color::White))
            .block(
                Block::default()
                    .title("Statistics")
                    .borders(Borders::ALL)
                    .border_type(BorderType::Plain),
            )
            .scroll((0, 0))
    }
}

/// Logs widget
pub struct LogsWidget;

impl LogsWidget {
    pub fn render(logs: &[super::LogEntry]) -> impl Widget + use<'_> {
        let log_items: Vec<ListItem> = logs
            .iter()
            .map(|entry| {
                ListItem::new(Line::from(vec![
                    Span::styled(
                        format!("  {}  ", entry.timestamp),
                        Style::default().fg(Color::Gray),
                    ),
                    Span::styled(entry.message.clone(), Style::default().fg(Color::White)),
                ]))
            })
            .collect();

        if log_items.is_empty() {
            let empty_msg = ListItem::new(Line::from(Span::styled(
                "  (no events yet)",
                Style::default().fg(Color::Gray),
            )));
            List::new(vec![empty_msg])
        } else {
            List::new(log_items)
        }
        .block(
            Block::default()
                .title("Log")
                .borders(Borders::ALL)
                .border_type(BorderType::Plain),
        )
    }
}

/// Dashboard widget combining header and footer
pub struct DashboardWidget;

impl DashboardWidget {
    pub fn new() -> Self {
        DashboardWidget
    }
    
    pub fn render_header(&self, app: &super::TuiApp) -> impl Widget {
        create_dashboard_header(app)
    }

    pub fn render_footer(&self) -> impl Widget {
        DashboardFooter::render()
    }
}

/// Progress gauge widget
pub struct ProgressGauge {
    pub title: String,
    pub percent: u16,
    pub label: String,
    pub color: Color,
}

impl ProgressGauge {
    pub fn new(title: String, percent: f64, label: String, color: Color) -> Self {
        Self {
            title,
            percent: percent.clamp(0.0, 100.0) as u16,
            label,
            color,
        }
    }

    pub fn render(&self) -> impl Widget + use<'_> {
        Gauge::default()
            .block(
                Block::default()
                    .title(self.title.as_str())
                    .borders(Borders::ALL)
                    .border_type(BorderType::Plain),
            )
            .gauge_style(Style::default().bg(Color::Black).fg(self.color))
            .percent(self.percent)
            .label(Span::from(self.label.as_str()))
    }
}

/// Multi-stats widget showing key metrics
pub struct MultiStatsWidget {
    pub stats: Vec<StatItem>,
}

#[derive(Debug, Clone)]
pub struct StatItem {
    pub label: String,
    pub value: String,
    pub color: Color,
}

impl MultiStatsWidget {
    pub fn new(stats: Vec<StatItem>) -> Self {
        Self { stats }
    }

    pub fn render(&self) -> impl Widget + use<'_> {
        let stat_text = self
            .stats
            .iter()
            .map(|stat| {
                Line::from(vec![
                    Span::styled(
                        format!("{}: ", stat.label),
                        Style::default().fg(Color::Gray),
                    ),
                    Span::styled(stat.value.clone(), Style::default().fg(stat.color)),
                ])
            })
            .collect::<Vec<_>>();

        Paragraph::new(stat_text)
            .style(Style::default().fg(Color::White))
            .block(
                Block::default()
                    .title("Scan Statistics")
                    .borders(Borders::ALL)
                    .border_type(BorderType::Plain),
            )
            .alignment(ratatui::layout::Alignment::Left)
    }
}

/// Helper function to format duration
fn format_duration(seconds: f64) -> String {
    if seconds <= 0.0 {
        "00:00:00".to_string()
    } else {
        let hours = (seconds / 3600.0).floor() as u64;
        let minutes = ((seconds % 3600.0) / 60.0).floor() as u64;
        let secs = (seconds % 60.0).floor() as u64;
        format!("{:02}:{:02}:{:02}", hours, minutes, secs)
    }
}

/// Helper function to create a block with title and borders
pub fn create_block(title: &str, borders: Borders) -> Block<'_> {
    Block::default()
        .title(Span::styled(
            title,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(borders)
        .border_type(BorderType::Plain)
}

/// Helper function to create styled text
pub fn create_styled_text(text: &str, color: Color, modifier: Modifier) -> Vec<Line<'_>> {
    vec![Line::from(Span::styled(
        text.to_string(),
        Style::default().fg(color).add_modifier(modifier),
    ))]
}

/// Helper function to create centered text
pub fn create_centered_text(text: &str, color: Color) -> Paragraph<'_> {
    Paragraph::new(text)
        .style(Style::default().fg(color))
        .alignment(ratatui::layout::Alignment::Center)
        .block(Block::default().borders(Borders::NONE))
}