use crate::error::{RecoveryError, Result};
use crate::types::{Offset, Size};
use memmap2::Mmap;
use std::fs::File;
use std::path::Path;
use std::sync::Arc;

/// A zero-copy slice of disk image data with lifetime tied to the parent DiskImage
#[derive(Debug)]
pub struct FragmentSlice<'a> {
    pub offset: Offset,
    pub data: &'a [u8],
}

impl<'a> FragmentSlice<'a> {
    /// Create a new fragment slice
    pub fn new(offset: Offset, data: &'a [u8]) -> Self {
        Self { offset, data }
    }

    /// Get the size of this fragment
    pub fn size(&self) -> Size {
        Size::new(self.data.len() as u64)
    }
}

/// Zero-copy memory-mapped disk image with shared ownership
#[derive(Clone)]
pub struct DiskImage {
    mmap: Arc<Mmap>,
    size: Size,
    path: String,
}

impl DiskImage {
    /// Open a disk image file with memory mapping
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path_ref = path.as_ref();
        let path_str = path_ref
            .to_str()
            .ok_or_else(|| RecoveryError::InvalidArgument("Invalid path encoding".to_string()))?
            .to_string();

        // Open the file
        let file = File::open(path_ref).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                RecoveryError::FileNotFound(path_str.clone())
            } else {
                RecoveryError::Io(e)
            }
        })?;

        // Get file size
        let metadata = file.metadata()?;
        let size = Size::new(metadata.len());

        // Memory map the file
        let mmap = unsafe {
            Mmap::map(&file)
                .map_err(|e| RecoveryError::Mmap(format!("Failed to mmap file: {}", e)))?
        };

        Ok(Self {
            mmap: Arc::new(mmap),
            size,
            path: path_str,
        })
    }

    /// Get the total size of the disk image
    pub fn size(&self) -> Size {
        self.size
    }

    /// Get the path to the disk image
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Get a zero-copy slice of the disk image with bounds checking
    pub fn get_slice(&self, offset: Offset, len: usize) -> Result<FragmentSlice<'_>> {
        let offset_u64 = offset.as_u64();
        let size_u64 = self.size.as_u64();

        // Check if offset is valid
        if offset_u64 >= size_u64 {
            return Err(RecoveryError::InvalidOffset {
                offset: offset_u64,
                image_size: size_u64,
            });
        }

        // Check if the requested slice would exceed bounds
        let end_offset = offset_u64
            .checked_add(len as u64)
            .ok_or_else(|| RecoveryError::InvalidSize {
                offset: offset_u64,
                size: len as u64,
                image_size: size_u64,
            })?;

        if end_offset > size_u64 {
            return Err(RecoveryError::InvalidSize {
                offset: offset_u64,
                size: len as u64,
                image_size: size_u64,
            });
        }

        // Safe: bounds have been checked
        let start = offset_u64 as usize;
        let end = end_offset as usize;
        let data = &self.mmap[start..end];

        Ok(FragmentSlice::new(offset, data))
    }

    /// Get the Arc-wrapped memory map for shared access
    pub fn get_mmap(&self) -> Arc<Mmap> {
        Arc::clone(&self.mmap)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fragment_slice_creation() {
        let data = b"test data";
        let offset = Offset::new(100);
        let slice = FragmentSlice::new(offset, data);

        assert_eq!(slice.offset.as_u64(), 100);
        assert_eq!(slice.data, b"test data");
        assert_eq!(slice.size().as_u64(), 9);
    }

    #[test]
    fn test_offset_checked_add() {
        let offset = Offset::new(100);
        let size = Size::new(50);
        let result = offset.checked_add(size);
        assert_eq!(result.unwrap().as_u64(), 150);

        // Test overflow
        let offset = Offset::new(u64::MAX);
        let size = Size::new(1);
        assert!(offset.checked_add(size).is_none());
    }
}
