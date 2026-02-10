use clap::Parser;
use std::path::PathBuf;

/// Ultimate File Recovery - Rust Implementation
/// Professional data recovery system for disk images
#[derive(Parser, Debug, Clone)]
#[command(name = "rust-recovery")]
#[command(version = "0.1.0")]
#[command(about = "Ultimate File Recovery - Rust Implementation", long_about = None)]
pub struct Args {
    /// Disk image file to scan
    #[arg(value_name = "IMAGE")]
    pub image: PathBuf,

    /// Minimum file size in KB
    #[arg(long = "target-size-min", default_value = "15")]
    pub target_size_min: u64,

    /// Maximum file size in KB
    #[arg(long = "target-size-max", default_value = "300")]
    pub target_size_max: u64,

    /// Scan from end to start (reverse scan)
    #[arg(long = "reverse")]
    pub reverse: bool,

    /// Optimize for NVMe drives
    #[arg(long = "nvme")]
    pub nvme: bool,

    /// Stop after N files recovered (0 = no limit)
    #[arg(long = "early-exit", default_value = "0")]
    pub early_exit: usize,

    /// Output directory for recovered files
    #[arg(short = 'o', long = "output", default_value = "recovery_output")]
    pub output: PathBuf,

    /// Enable exFAT metadata scanning (Opt-in)
    #[arg(long = "enable-exfat")]
    pub enable_exfat: bool,

    /// Disable live dashboard (use simple text output)
    #[arg(long = "no-live")]
    pub no_live: bool,

    /// Mode: extract only links, don't save binary chunks
    #[arg(long = "links-only")]
    pub links_only: bool,

    /// Minimum dynamic chunk size in KB
    #[arg(long = "chunk-min", default_value = "32")]
    pub chunk_min: u64,

    /// Maximum dynamic chunk size in KB
    #[arg(long = "chunk-max", default_value = "2048")]
    pub chunk_max: u64,

    /// Enable FAT chain following for full file recovery (default: true)
    #[arg(long = "full-exfat-recovery", default_value = "true")]
    pub full_exfat_recovery: bool,

    /// Analyze candidates and group by semantic category
    #[arg(long = "semantic-scan")]
    pub semantic_scan: bool,
}

impl Args {
    /// Validate the arguments
    pub fn validate(&self) -> Result<(), String> {
        // Check that image file path is not empty
        if self.image.as_os_str().is_empty() {
            return Err("Image path cannot be empty".to_string());
        }

        // Validate size ranges
        if self.target_size_min > self.target_size_max {
            return Err(format!(
                "target-size-min ({}) cannot be greater than target-size-max ({})",
                self.target_size_min, self.target_size_max
            ));
        }

        if self.target_size_min == 0 {
            return Err("target-size-min must be greater than 0".to_string());
        }

        // Validate chunk sizes
        if self.chunk_min > self.chunk_max {
            return Err(format!(
                "chunk-min ({}) cannot be greater than chunk-max ({})",
                self.chunk_min, self.chunk_max
            ));
        }

        if self.chunk_min == 0 {
            return Err("chunk-min must be greater than 0".to_string());
        }

        Ok(())
    }

    /// Get target size min in bytes
    pub fn target_size_min_bytes(&self) -> u64 {
        self.target_size_min * 1024
    }

    /// Get target size max in bytes
    pub fn target_size_max_bytes(&self) -> u64 {
        self.target_size_max * 1024
    }

    /// Get chunk min in bytes
    pub fn chunk_min_bytes(&self) -> u64 {
        self.chunk_min * 1024
    }

    /// Get chunk max in bytes
    pub fn chunk_max_bytes(&self) -> u64 {
        self.chunk_max * 1024
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_args_validation() {
        let args = Args {
            image: PathBuf::from("test.img"),
            target_size_min: 15,
            target_size_max: 300,
            reverse: false,
            nvme: false,
            early_exit: 0,
            output: PathBuf::from("output"),
            enable_exfat: false,
            no_live: false,
            links_only: false,
            chunk_min: 32,
            chunk_max: 2048,
            full_exfat_recovery: true,
            semantic_scan: false,
        };

        assert!(args.validate().is_ok());
    }

    #[test]
    fn test_invalid_size_range() {
        let args = Args {
            image: PathBuf::from("test.img"),
            target_size_min: 500,
            target_size_max: 300,
            reverse: false,
            nvme: false,
            early_exit: 0,
            output: PathBuf::from("output"),
            enable_exfat: false,
            no_live: false,
            links_only: false,
            chunk_min: 32,
            chunk_max: 2048,
            full_exfat_recovery: true,
            semantic_scan: false,
        };

        assert!(args.validate().is_err());
    }

    #[test]
    fn test_byte_conversions() {
        let args = Args {
            image: PathBuf::from("test.img"),
            target_size_min: 15,
            target_size_max: 300,
            reverse: false,
            nvme: false,
            early_exit: 0,
            output: PathBuf::from("output"),
            enable_exfat: false,
            no_live: false,
            links_only: false,
            chunk_min: 32,
            chunk_max: 2048,
            full_exfat_recovery: true,
            semantic_scan: false,
        };

        assert_eq!(args.target_size_min_bytes(), 15 * 1024);
        assert_eq!(args.target_size_max_bytes(), 300 * 1024);
        assert_eq!(args.chunk_min_bytes(), 32 * 1024);
        assert_eq!(args.chunk_max_bytes(), 2048 * 1024);
    }
}
