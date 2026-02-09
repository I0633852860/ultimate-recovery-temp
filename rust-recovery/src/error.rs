use thiserror::Error;

/// Main error type for the recovery tool
#[derive(Error, Debug)]
pub enum RecoveryError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Memory mapping error: {0}")]
    Mmap(String),

    #[error("Invalid offset: requested offset {offset} exceeds image size {image_size}")]
    InvalidOffset { offset: u64, image_size: u64 },

    #[error("Invalid size: requested size {size} at offset {offset} exceeds image bounds (image size: {image_size})")]
    InvalidSize {
        offset: u64,
        size: u64,
        image_size: u64,
    },

    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Configuration error: {0}")]
    Config(String),
}

/// Result type alias for recovery operations
pub type Result<T> = std::result::Result<T, RecoveryError>;
