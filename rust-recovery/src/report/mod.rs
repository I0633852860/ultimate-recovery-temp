//! Report generation module for Ultimate File Recovery
//! 
//! This module provides functionality to generate professional HTML and JSON reports
//! using askama templates. The reports include scan statistics, recovered files,
//! data clusters, and comprehensive analysis results.

pub mod templates;

// use askama::Template; // Temporarily disabled
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::fs;

/// Report context containing all data for template rendering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportContext {
    /// Report metadata
    pub metadata: ReportMetadata,
    /// Scan configuration and results
    pub scan_results: ScanResults,
    /// Data clusters found during scan
    pub clusters: Vec<DataCluster>,
    /// Recovered files information
    pub recovered_files: Vec<RecoveredFile>,
    /// Failure reasons (if any)
    pub failure_reasons: Vec<String>,
    /// Success status
    pub success: bool,
}

/// Report metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportMetadata {
    /// Report generation timestamp
    pub timestamp: String,
    /// Report version
    pub version: String,
    /// Tool name and version
    pub tool_name: String,
    /// Image file path
    pub image_path: String,
    /// Output directory
    pub output_dir: String,
}

/// Scan results and statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResults {
    /// Image size in MB
    pub image_size_mb: f64,
    /// Bytes scanned in MB
    pub bytes_scanned_mb: f64,
    /// Candidates found count
    pub candidates_found: u32,
    /// Files recovered count
    pub files_recovered: u32,
    /// Scan time in seconds
    pub scan_time_sec: f64,
    /// Average speed in MB/s
    pub avg_speed_mbps: f64,
    /// Maximum speed in MB/s
    pub max_speed_mbps: f64,
    /// Minimum speed in MB/s
    pub min_speed_mbps: f64,
    /// Reverse scan flag
    pub reverse_scan: bool,
    /// exFAT scan enabled
    pub exfat_enabled: bool,
    /// NVMe optimization enabled
    pub nvme_optimization: bool,
}

/// Data cluster information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataCluster {
    /// Cluster ID
    pub id: usize,
    /// Start offset in bytes (hex)
    pub start_offset_hex: String,
    /// End offset in bytes (hex)
    pub end_offset_hex: String,
    /// Cluster size in bytes
    pub size_bytes: u64,
    /// Cluster size in KB
    pub size_kb: u64,
    /// Number of links found
    pub link_count: u32,
    /// Density (links per KB)
    pub density: f64,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f64,
    /// Links found in this cluster
    pub links: Vec<String>,
}

/// Recovered file information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveredFile {
    /// File ID
    pub id: usize,
    /// Filename
    pub filename: String,
    /// File type
    pub file_type: String,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f64,
    /// Links extracted from file
    pub links: Vec<String>,
    /// File size in KB
    pub size_kb: u64,
    /// SHA256 hash
    pub sha256: String,
    /// Start offset in disk image
    pub start_offset: u64,
    /// End offset in disk image
    pub end_offset: u64,
    /// File validation status
    pub validation_status: ValidationStatus,
    /// Recovery timestamp
    pub recovery_time: String,
}

/// File validation status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationStatus {
    /// File is valid
    Valid,
    /// File has minor issues but is usable
    MinorIssues,
    /// File has significant issues
    MajorIssues,
    /// File validation failed
    Invalid,
    /// File type could not be determined
    Unknown,
}

/// Recovery statistics summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryStats {
    /// Total files processed
    pub total_processed: u32,
    /// Successfully recovered files
    pub successful_recoveries: u32,
    /// Failed recovery attempts
    pub failed_recoveries: u32,
    /// Success rate percentage
    pub success_rate: f64,
    /// Total data recovered in bytes
    pub total_bytes_recovered: u64,
    /// Recovery efficiency score
    pub efficiency_score: f64,
}

/// HTML report template using askama (temporarily disabled)
 /*
#[derive(Template)]
#[template(path = "report.html", escape = "html")]
pub struct HtmlReportTemplate {
    pub context: ReportContext,
    pub stats: RecoveryStats,
}
*/

/// JSON report structure for machine-readable output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonReport {
    pub metadata: ReportMetadata,
    pub scan_results: ScanResults,
    pub clusters: Vec<DataCluster>,
    pub recovered_files: Vec<RecoveredFile>,
    pub failure_reasons: Vec<String>,
    pub stats: RecoveryStats,
    pub success: bool,
    pub report_checksum: String,
}

/// Professional report generator
pub struct ProfessionalReportGenerator {
    output_dir: std::path::PathBuf,
    reports_dir: std::path::PathBuf,
}

impl ProfessionalReportGenerator {
    /// Create new report generator
    pub fn new(output_dir: &Path) -> Self {
        let reports_dir = output_dir.join("reports");
        
        // Create reports directory if it doesn't exist
        if !reports_dir.exists() {
            fs::create_dir_all(&reports_dir).expect("Failed to create reports directory");
        }

        Self {
            output_dir: output_dir.to_path_buf(),
            reports_dir,
        }
    }

    /// Generate full report (HTML + JSON)
    pub fn generate_full_report(
        &self,
        scan_results: ScanResults,
        clusters: Vec<DataCluster>,
        recovered_files: Vec<RecoveredFile>,
        failure_reasons: Vec<String>,
        metadata: ReportMetadata,
    ) -> Result<ReportPaths, ReportError> {
        let success = !recovered_files.is_empty();
        
        // Calculate recovery statistics
        let stats = self.calculate_recovery_stats(&recovered_files);
        
        // Create report context
        let context = ReportContext {
            metadata,
            scan_results,
            clusters,
            recovered_files: recovered_files.clone(),
            failure_reasons,
            success,
        };

        // Generate timestamp for filenames
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let report_name = format!("recovery_report_{}", timestamp);

        // Generate HTML report
        let html_path = self.reports_dir.join(format!("{}.html", report_name));
        self.generate_html_report(&context, &stats, &html_path)?;

        // Generate JSON report
        let json_path = self.reports_dir.join(format!("{}.json", report_name));
        self.generate_json_report(&context, &stats, &json_path)?;

        Ok(ReportPaths {
            html_path,
            json_path,
        })
    }

    /// Generate HTML report using askama template (temporarily disabled)
    fn generate_html_report(
        &self,
        context: &ReportContext,
        _stats: &RecoveryStats,
        path: &Path,
    ) -> Result<(), ReportError> {
        // Temporary simple HTML generation without askama
        let html_content = format!(
            r#"<!DOCTYPE html>
<html>
<head><title>Recovery Report</title></head>
<body>
<h1>Recovery Report</h1>
<p>Files recovered: {}</p>
<p>Scan time: {:.1}s</p>
</body>
</html>"#,
            context.recovered_files.len(),
            context.scan_results.scan_time_sec
        );

        fs::write(path, html_content)
            .map_err(|e| ReportError::IoError(e))?;

        Ok(())
    }

    /// Generate JSON report
    fn generate_json_report(
        &self,
        context: &ReportContext,
        stats: &RecoveryStats,
        path: &Path,
    ) -> Result<(), ReportError> {
        let json_report = JsonReport {
            metadata: context.metadata.clone(),
            scan_results: context.scan_results.clone(),
            clusters: context.clusters.clone(),
            recovered_files: context.recovered_files.clone(),
            failure_reasons: context.failure_reasons.clone(),
            stats: stats.clone(),
            success: context.success,
            report_checksum: self.calculate_checksum(context, stats)?,
        };

        let json_content = serde_json::to_string_pretty(&json_report)
            .map_err(|e| ReportError::SerializationError(e))?;

        fs::write(path, json_content)
            .map_err(|e| ReportError::IoError(e))?;

        Ok(())
    }

    /// Calculate recovery statistics
    fn calculate_recovery_stats(&self, recovered_files: &[RecoveredFile]) -> RecoveryStats {
        let total_processed = recovered_files.len() as u32;
        let successful_recoveries = recovered_files
            .iter()
            .filter(|f| matches!(f.validation_status, ValidationStatus::Valid | ValidationStatus::MinorIssues))
            .count() as u32;
        let failed_recoveries = total_processed.saturating_sub(successful_recoveries);
        let success_rate = if total_processed > 0 {
            (successful_recoveries as f64 / total_processed as f64) * 100.0
        } else {
            0.0
        };
        
        let total_bytes_recovered: u64 = recovered_files
            .iter()
            .map(|f| f.size_kb as u64 * 1024)
            .sum();

        // Simple efficiency score based on success rate and data recovery
        let efficiency_score = if total_processed > 0 {
            (success_rate * 0.7) + ((total_bytes_recovered as f64 / 1024.0 / 1024.0 / 1024.0).min(100.0) * 0.3)
        } else {
            0.0
        };

        RecoveryStats {
            total_processed,
            successful_recoveries,
            failed_recoveries,
            success_rate,
            total_bytes_recovered,
            efficiency_score,
        }
    }

    /// Calculate simple checksum for report integrity
    fn calculate_checksum(&self, context: &ReportContext, stats: &RecoveryStats) -> Result<String, ReportError> {
        use sha2::{Digest, Sha256};
        
        let data = format!(
            "{:?}{:?}{:?}",
            context.metadata.timestamp,
            stats.success_rate,
            context.recovered_files.len()
        );
        
        let mut hasher = Sha256::new();
        hasher.update(data);
        let result = hasher.finalize();
        
        Ok(format!("{:x}", result))
    }
}

/// Paths to generated report files
#[derive(Debug, Clone)]
pub struct ReportPaths {
    pub html_path: std::path::PathBuf,
    pub json_path: std::path::PathBuf,
}

/// Report generation errors
#[derive(Debug, thiserror::Error)]
pub enum ReportError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Template rendering error: {0}")]
    TemplateError(String),
    
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
    
    #[error("Checksum calculation error: {0}")]
    ChecksumError(String),
}

/// Helper function to create metadata from scan parameters
pub fn create_report_metadata(
    image_path: &str,
    output_dir: &str,
    version: &str,
) -> ReportMetadata {
    ReportMetadata {
        timestamp: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        version: version.to_string(),
        tool_name: "Ultimate File Recovery".to_string(),
        image_path: image_path.to_string(),
        output_dir: output_dir.to_string(),
    }
}

/// Helper function to create scan results from various sources
pub fn create_scan_results(
    image_size_bytes: u64,
    bytes_scanned: u64,
    candidates_found: u32,
    scan_duration: std::time::Duration,
    reverse_scan: bool,
    exfat_enabled: bool,
    nvme_optimization: bool,
) -> ScanResults {
    let scan_time_sec = scan_duration.as_secs_f64();
    let image_size_mb = image_size_bytes as f64 / 1024.0 / 1024.0;
    let bytes_scanned_mb = bytes_scanned as f64 / 1024.0 / 1024.0;
    let avg_speed_mbps = if scan_time_sec > 0.0 {
        bytes_scanned_mb / scan_time_sec
    } else {
        0.0
    };

    ScanResults {
        image_size_mb,
        bytes_scanned_mb,
        candidates_found,
        files_recovered: 0, // Will be updated separately
        scan_time_sec,
        avg_speed_mbps,
        max_speed_mbps: avg_speed_mbps, // Simplified for now
        min_speed_mbps: avg_speed_mbps, // Simplified for now
        reverse_scan,
        exfat_enabled,
        nvme_optimization,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_metadata() {
        let metadata = create_report_metadata("test.img", "/output", "1.0.0");
        assert_eq!(metadata.image_path, "test.img");
        assert_eq!(metadata.output_dir, "/output");
        assert_eq!(metadata.version, "1.0.0");
    }

    #[test]
    fn test_create_scan_results() {
        let results = create_scan_results(
            1024 * 1024, // 1MB
            512 * 1024,  // 512KB scanned
            5,           // 5 candidates
            std::time::Duration::from_secs(10),
            false,
            true,
            false,
        );
        
        assert_eq!(results.image_size_mb, 1.0);
        assert_eq!(results.bytes_scanned_mb, 0.5);
        assert_eq!(results.candidates_found, 5);
        assert_eq!(results.scan_time_sec, 10.0);
        assert_eq!(results.avg_speed_mbps, 0.05);
    }
}