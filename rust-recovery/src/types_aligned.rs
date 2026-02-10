//! Cache-aligned структуры для форензик данных

use cache_padded::CachePadded;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

/// Hot Fragment с выравниванием по кэш-линии
/// Размер: ровно 64 байта (1 cache line)
#[repr(C, align(64))]
#[derive(Debug, Clone)]
pub struct HotFragmentAligned {
    // Основные поля (56 байт)
    pub offset: u64,              // 8 байт
    pub size: u64,                // 8 байт
    pub youtube_count: u32,       // 4 байта
    pub json_markers: u32,        // 4 байта
    pub cyrillic_density: f32,    // 4 байта
    pub target_score: f32,        // 4 байта
    pub entropy: f32,             // 4 байта
    pub has_metadata: bool,       // 1 байт
    pub has_valid_json: bool,     // 1 байт
    pub high_entropy: bool,       // 1 байт
    
    // Padding до 64 байт
    _padding: [u8; 17],           // 17 байт padding
}

impl HotFragmentAligned {
    pub fn new(offset: u64, size: u64) -> Self {
        Self {
            offset,
            size,
            youtube_count: 0,
            json_markers: 0,
            cyrillic_density: 0.0,
            target_score: 0.0,
            entropy: 0.0,
            has_metadata: false,
            has_valid_json: false,
            high_entropy: false,
            _padding: [0; 17],
        }
    }
}

/// Scan Statistics с атомарными операциями и cache padding
#[derive(Debug)]
pub struct ScanStatsAligned {
    pub bytes_scanned: CachePadded<AtomicU64>,
    pub links_found: CachePadded<AtomicUsize>,
    pub hot_fragments: CachePadded<AtomicUsize>,
    pub chunks_processed: CachePadded<AtomicUsize>,
    pub errors: CachePadded<AtomicUsize>,
}

impl ScanStatsAligned {
    pub fn new() -> Self {
        Self {
            bytes_scanned: CachePadded::new(AtomicU64::new(0)),
            links_found: CachePadded::new(AtomicUsize::new(0)),
            hot_fragments: CachePadded::new(AtomicUsize::new(0)),
            chunks_processed: CachePadded::new(AtomicUsize::new(0)),
            errors: CachePadded::new(AtomicUsize::new(0)),
        }
    }
    
    #[inline(always)]
    pub fn add_bytes_scanned(&self, count: u64) {
        self.bytes_scanned.fetch_add(count, Ordering::Relaxed);
    }
    
    #[inline(always)]
    pub fn add_link(&self) {
        self.links_found.fetch_add(1, Ordering::Relaxed);
    }
    
    #[inline(always)]
    pub fn add_hot_fragment(&self) {
        self.hot_fragments.fetch_add(1, Ordering::Relaxed);
    }
    
    #[inline(always)]
    pub fn add_chunk(&self) {
        self.chunks_processed.fetch_add(1, Ordering::Relaxed);
    }
    
    #[inline(always)]
    pub fn add_error(&self) {
        self.errors.fetch_add(1, Ordering::Relaxed);
    }
    
    pub fn snapshot(&self) -> ScanStatsSnapshot {
        ScanStatsSnapshot {
            bytes_scanned: self.bytes_scanned.load(Ordering::Relaxed),
            links_found: self.links_found.load(Ordering::Relaxed),
            hot_fragments: self.hot_fragments.load(Ordering::Relaxed),
            chunks_processed: self.chunks_processed.load(Ordering::Relaxed),
            errors: self.errors.load(Ordering::Relaxed),
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ScanStatsSnapshot {
    pub bytes_scanned: u64,
    pub links_found: usize,
    pub hot_fragments: usize,
    pub chunks_processed: usize,
    pub errors: usize,
}

/// Aligned буфер для SIMD операций
#[repr(C, align(64))]
pub struct AlignedBuffer {
    pub data: Vec<u8>,
}

impl AlignedBuffer {
    pub fn new(size: usize) -> Self {
        let layout = std::alloc::Layout::from_size_align(size, 64).unwrap();
        let ptr = unsafe { std::alloc::alloc(layout) };
        if ptr.is_null() { std::alloc::handle_alloc_error(layout); }
        
        let data = unsafe { Vec::from_raw_parts(ptr, size, size) };
        
        Self { data }
    }
    
    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }
    
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.data
    }
}
