// exFAT Full Recovery v5.1 — Military Grade Performance Update
// Optimized for fast Phase 0 scanning (< 300s for large disks)
// Implements early exit for zero blocks and improved parallel processing

use pyo3::prelude::*;
use pyo3::types::PyBytes;
use rayon::prelude::*;
use memmap2::Mmap;
use std::fs::File;
use std::sync::{Arc, RwLock};
use crate::simd_search::scan_block_simd;
use crate::matcher::EnhancedMatcher;
use crate::types::EnrichedLink;

// ═══════════════════════════════════════════════════════════════════════════════
// exFAT CONSTANTS — по спецификации Microsoft exFAT Revision 1.00
// ═══════════════════════════════════════════════════════════════════════════════

/// Entry type markers
const ENTRY_FILE:            u8 = 0x85;  // File Directory Entry (active)
const ENTRY_STREAM:          u8 = 0xC0;  // Stream Extension Entry (active)
const ENTRY_FILENAME:        u8 = 0xC1;  // File Name Entry (active)
const ENTRY_DELETED_FILE:    u8 = 0x05;  // File Directory Entry (deleted)
const ENTRY_DELETED_STREAM:  u8 = 0x40;  // Stream Extension Entry (deleted)
const ENTRY_DELETED_FILENAME:u8 = 0x41;  // File Name Entry (deleted)
const ENTRY_ALLOC_BITMAP:    u8 = 0x81;  // Allocation Bitmap
const ENTRY_UPCASE_TABLE:    u8 = 0x82;  // Up-case Table
const ENTRY_VOLUME_LABEL:    u8 = 0x83;  // Volume Label

/// Boot sector field offsets (по спецификации exFAT §3.1)
const BS_JUMP_BOOT:               usize = 0;    // 3 bytes
const BS_FILE_SYSTEM_NAME:        usize = 3;    // 8 bytes "EXFAT   "
const BS_PARTITION_OFFSET:        usize = 64;   // 8 bytes (u64)
const BS_VOLUME_LENGTH:           usize = 72;   // 8 bytes (u64)
const BS_FAT_OFFSET:              usize = 80;   // 4 bytes (u32) — in sectors
const BS_FAT_LENGTH:              usize = 84;   // 4 bytes (u32) — in sectors
const BS_CLUSTER_HEAP_OFFSET:     usize = 88;   // 4 bytes (u32) — in sectors
const BS_CLUSTER_COUNT:           usize = 92;   // 4 bytes (u32)
const BS_FIRST_CLUSTER_OF_ROOT:   usize = 96;   // 4 bytes (u32)
const BS_VOLUME_SERIAL_NUMBER:    usize = 100;  // 4 bytes (u32)
const BS_FILE_SYSTEM_REVISION:    usize = 104;  // 2 bytes (u16)
const BS_VOLUME_FLAGS:            usize = 106;  // 2 bytes (u16)
const BS_BYTES_PER_SECTOR_SHIFT:  usize = 108;  // 1 byte (u8) — 2^N
const BS_SECTORS_PER_CLUSTER_SHIFT:usize = 109; // 1 byte (u8) — 2^N
const BS_NUMBER_OF_FATS:          usize = 110;  // 1 byte (u8)
const BS_DRIVE_SELECT:            usize = 111;  // 1 byte (u8)
const BS_PERCENT_IN_USE:          usize = 112;  // 1 byte (u8)

/// Stream Extension Entry field offsets (внутри 32-byte entry, §7.4)
const SE_GENERAL_FLAGS:      usize = 1;   // 1 byte — bit1 = NoFatChain
const SE_NAME_LENGTH:        usize = 3;   // 1 byte
const SE_NAME_HASH:          usize = 4;   // 2 bytes (u16)
const SE_VALID_DATA_LENGTH:  usize = 8;   // 8 bytes (u64)
const SE_FIRST_CLUSTER:      usize = 20;  // 4 bytes (u32)
const SE_DATA_LENGTH:        usize = 24;  // 8 bytes (u64) — ПОЛНЫЙ размер файла

/// File Name Entry field offsets (внутри 32-byte entry, §7.7)
const FN_GENERAL_FLAGS:      usize = 1;   // 1 byte
const FN_FILE_NAME:          usize = 2;   // 30 bytes (15 UTF-16LE chars)

const DIRECTORY_ENTRY_SIZE: usize = 32;
const SCAN_CHUNK_SIZE: usize = 64 * 1024 * 1024; // Optimized for performance (64MB)

// ═══════════════════════════════════════════════════════════════════════════════
// DATA STRUCTURES
// ═══════════════════════════════════════════════════════════════════════════════

/// Parsed exFAT Boot Sector parameters
#[derive(Clone, Debug)]
pub struct ExFatBootParams {
    pub sector_size: u64,
    pub cluster_size: u64,
    pub fat_offset: u64,
    pub fat_length_sectors: u32,
    pub cluster_heap_offset: u64,
    pub cluster_count: u32,
    pub root_dir_cluster: u32,
    pub boot_sector_offset: u64,
}

/// exFAT directory entry — результат парсинга
#[derive(Clone, Debug)]
#[pyclass]
pub struct ExFATEntry {
    #[pyo3(get)]
    pub offset: u64,
    #[pyo3(get)]
    pub data_offset: u64,
    #[pyo3(get)]
    pub is_deleted: bool,
    #[pyo3(get)]
    pub filename: String,
    #[pyo3(get)]
    pub size: u64,
    #[pyo3(get)]
    pub first_cluster: u32,
    #[pyo3(get)]
    pub no_fat_chain: bool,
}

// ═══════════════════════════════════════════════════════════════════════════════
// BOOT SECTOR PARSING
// ═══════════════════════════════════════════════════════════════════════════════

fn parse_boot_sector_at(data: &[u8], bs_offset: u64) -> Option<ExFatBootParams> {
    let off = bs_offset as usize;
    if data.len() < off + 120 {
        return None;
    }

    if &data[off + BS_FILE_SYSTEM_NAME..off + BS_FILE_SYSTEM_NAME + 8] != b"EXFAT   " {
        return None;
    }

    let bytes_per_sector_shift = data[off + BS_BYTES_PER_SECTOR_SHIFT];
    let sectors_per_cluster_shift = data[off + BS_SECTORS_PER_CLUSTER_SHIFT];

    if bytes_per_sector_shift < 9 || bytes_per_sector_shift > 12 {
        return None;
    }
    if sectors_per_cluster_shift > 25 {
        return None;
    }

    let sector_size = 1u64 << bytes_per_sector_shift;
    let cluster_size = sector_size << sectors_per_cluster_shift;

    let fat_offset_sectors = u32::from_le_bytes(
        data[off + BS_FAT_OFFSET..off + BS_FAT_OFFSET + 4].try_into().ok()?
    ) as u64;
    let fat_length_sectors = u32::from_le_bytes(
        data[off + BS_FAT_LENGTH..off + BS_FAT_LENGTH + 4].try_into().ok()?
    );
    let cluster_heap_offset_sectors = u32::from_le_bytes(
        data[off + BS_CLUSTER_HEAP_OFFSET..off + BS_CLUSTER_HEAP_OFFSET + 4].try_into().ok()?
    ) as u64;
    let cluster_count = u32::from_le_bytes(
        data[off + BS_CLUSTER_COUNT..off + BS_CLUSTER_COUNT + 4].try_into().ok()?
    );
    let root_dir_cluster = u32::from_le_bytes(
        data[off + BS_FIRST_CLUSTER_OF_ROOT..off + BS_FIRST_CLUSTER_OF_ROOT + 4].try_into().ok()?
    );

    let fat_offset = bs_offset + fat_offset_sectors * sector_size;
    let cluster_heap_offset = bs_offset + cluster_heap_offset_sectors * sector_size;

    if cluster_size == 0 || cluster_size > 32 * 1024 * 1024 {
        return None;
    }
    if fat_offset == 0 || cluster_heap_offset == 0 {
        return None;
    }

    Some(ExFatBootParams {
        sector_size,
        cluster_size,
        fat_offset,
        fat_length_sectors,
        cluster_heap_offset,
        cluster_count,
        root_dir_cluster,
        boot_sector_offset: bs_offset,
    })
}

fn find_boot_sector(data: &[u8]) -> Option<ExFatBootParams> {
    if let Some(params) = parse_boot_sector_at(data, 0) {
        return Some(params);
    }

    let search_limit = std::cmp::min(data.len(), 4 * 1024 * 1024); // Extended search range
    for offset in (512..search_limit).step_by(512) {
        if offset + 120 > data.len() {
            break;
        }
        if &data[offset + 3..offset + 11] == b"EXFAT   " {
            if let Some(params) = parse_boot_sector_at(data, offset as u64) {
                return Some(params);
            }
        }
    }

    None
}

// ═══════════════════════════════════════════════════════════════════════════════
// FAT CHAIN FOLLOWING
// ═══════════════════════════════════════════════════════════════════════════════

#[inline]
fn fat_next_cluster(data: &[u8], params: &ExFatBootParams, cluster: u32) -> u32 {
    let fat_entry_offset = params.fat_offset + (cluster as u64 * 4);
    if fat_entry_offset + 4 > data.len() as u64 {
        return 0xFFFFFFFF;
    }
    let off = fat_entry_offset as usize;
    u32::from_le_bytes(
        data[off..off + 4].try_into().unwrap_or([0xFF, 0xFF, 0xFF, 0xFF])
    )
}

#[inline]
fn cluster_to_offset(params: &ExFatBootParams, cluster: u32) -> u64 {
    params.cluster_heap_offset + ((cluster as u64 - 2) * params.cluster_size)
}

fn extract_file_content(
    data: &[u8],
    params: &ExFatBootParams,
    first_cluster: u32,
    file_size: u64,
    no_fat_chain: bool,
) -> Vec<u8> {
    if first_cluster < 2 || file_size == 0 {
        return Vec::new();
    }

    let max_extract_size: u64 = 250 * 1024 * 1024; // Increased limit for military grade
    let actual_size = file_size.min(max_extract_size);

    let mut content = Vec::with_capacity(actual_size as usize);
    let mut remaining = actual_size;
    let mut cluster = first_cluster;
    let mut chain_len = 0u32;
    let max_chain = params.cluster_count.saturating_add(100);

    while remaining > 0 && cluster >= 2 && cluster < 0xFFFFFFF7 && chain_len < max_chain {
        let start = cluster_to_offset(params, cluster);
        let to_read = remaining.min(params.cluster_size);
        let end = (start + to_read).min(data.len() as u64);

        if start >= data.len() as u64 || start >= end {
            break;
        }

        content.extend_from_slice(&data[start as usize..end as usize]);
        remaining -= to_read;
        chain_len += 1;

        cluster = if no_fat_chain {
            cluster + 1
        } else {
            fat_next_cluster(data, params, cluster)
        };
    }

    content.truncate(actual_size as usize);
    content
}

// ═══════════════════════════════════════════════════════════════════════════════
// DIRECTORY ENTRY PARSING
// ═══════════════════════════════════════════════════════════════════════════════

fn parse_entry_set(data: &[u8], base_offset: u64) -> Option<(ExFATEntry, usize)> {
    if data.len() < DIRECTORY_ENTRY_SIZE {
        return None;
    }

    let file_type = data[0];
    let is_deleted = file_type == ENTRY_DELETED_FILE;

    if file_type != ENTRY_FILE && file_type != ENTRY_DELETED_FILE {
        return None;
    }

    let secondary_count = data[1] as usize;
    let total_entries = 1 + secondary_count;
    let total_bytes = total_entries * DIRECTORY_ENTRY_SIZE;

    if data.len() < total_bytes || secondary_count < 2 {
        return None;
    }

    let se_offset = DIRECTORY_ENTRY_SIZE;
    let se_type = data[se_offset];
    if se_type != ENTRY_STREAM && se_type != ENTRY_DELETED_STREAM {
        return None;
    }

    let general_flags = data[se_offset + SE_GENERAL_FLAGS];
    let no_fat_chain = (general_flags & 0x02) != 0;
    let name_length = data[se_offset + SE_NAME_LENGTH] as usize;

    let first_cluster = u32::from_le_bytes(
        data[se_offset + SE_FIRST_CLUSTER..se_offset + SE_FIRST_CLUSTER + 4]
            .try_into().ok()?
    );
    let file_size = u64::from_le_bytes(
        data[se_offset + SE_DATA_LENGTH..se_offset + SE_DATA_LENGTH + 8]
            .try_into().ok()?
    );

    let mut filename = String::with_capacity(name_length);
    let mut chars_collected = 0;

    for i in 2..total_entries {
        let fn_offset = i * DIRECTORY_ENTRY_SIZE;
        if fn_offset + DIRECTORY_ENTRY_SIZE > data.len() {
            break;
        }

        let fn_type = data[fn_offset];
        if fn_type != ENTRY_FILENAME && fn_type != ENTRY_DELETED_FILENAME {
            break;
        }

        for j in 0..15 {
            if chars_collected >= name_length {
                break;
            }
            let char_offset = fn_offset + FN_FILE_NAME + j * 2;
            if char_offset + 2 > data.len() {
                break;
            }
            let ch = u16::from_le_bytes([data[char_offset], data[char_offset + 1]]);
            if ch == 0 {
                break;
            }
            if let Some(c) = char::from_u32(ch as u32) {
                filename.push(c);
                chars_collected += 1;
            }
        }
    }

    if first_cluster < 2 && file_size > 0 {
        return None;
    }

    // Optimization: avoid trim().to_string() allocation if possible, but filename is built from chars.
    // The previous code had filename.push(c).
    // Let's just return it. The trimming might be important if there are padding nulls/spaces, 
    // but usually exFAT filenames are exact.
    // Spec says they are padded with 0x00 if entry is not full, but we check for ch == 0 break.
    // So explicit trim shouldn't be needed for standard compliance, but maybe for safety.
    // We'll keep it simple for now as requested.

    Some((ExFATEntry {
        offset: base_offset,
        data_offset: 0,
        is_deleted,
        filename, // Removed trim().to_string()
        size: file_size,
        first_cluster,
        no_fat_chain,
    }, total_entries))
}

/// Military Grade optimized scanner with early exit for zero blocks and Hot-Stream analysis
fn scan_for_entries_impl(
    data: &[u8], 
    base_offset: u64,
    matcher: &mut EnhancedMatcher,
) -> (Vec<ExFATEntry>, Vec<EnrichedLink>) {
    let mut entries = Vec::new();
    let mut links = Vec::new();
    let mut pos = 0;
    let len = data.len();

    // Align pos to 32 bytes if needed (though we usually start at 0 or sector aligned)
    // For SIMD, alignment is good but scan_block_simd handles unaligned loads (loadu).

    // We keep track of the last position scanned for hot content to avoid overlapping scans
    let mut last_hot_scan_end = 0;

    while pos + 32 <= len {
        let block_res = scan_block_simd(&data[pos..pos+32]);

        // 1. FAST PATH: Empty Region
        if block_res.is_empty {
            // Optimization: if strictly empty, we can skip.
            pos += 32;
            continue;
        }

        // 2. METADATA PATH: Potential ExFAT Entry
        // Valid Entry starts with 0x85 (ENTRY_FILE) or 0x05 (ENTRY_DELETED_FILE)
        let byte0 = data[pos];
        if block_res.has_metadata || byte0 == ENTRY_DELETED_FILE {
             if let Some((entry, consumed)) = parse_entry_set(&data[pos..], base_offset + pos as u64) {
                entries.push(entry);
                pos += consumed * DIRECTORY_ENTRY_SIZE;
                continue;
            }
        }

        // 3. HOT CONTENT PATH: Forensic Artifacts
        if block_res.hot_mask != 0 {
            // Found potential hot content.
            // Check if we already scanned this region to avoid overlaps
            if pos >= last_hot_scan_end {
                // Scan a window around this position
                // Window size logic: scan a small chunk forward
                // We use 4KB window or until end of buffer
                let scan_end = (pos + 4096).min(len);
                let window = &data[pos..scan_end];
                
                // matcher.scan_chunk expects slice, base_offset, deduct_context?
                // scan_chunk(data, base_offset, deduplicate)
                let found_links = matcher.scan_chunk(
                    window, 
                    (base_offset + pos as u64) as usize, 
                    true
                );
                
                if !found_links.is_empty() {
                    links.extend(found_links);
                }
                
                // Update last scanned position to avoid re-scanning the same bytes immediately
                // We advance last_hot_scan_end. We do NOT advance `pos` drastically because 
                // we still need to check for metadata in 32-byte steps.
                // But if we found links, we might have covered 4KB.
                // However, metadata scanning MUST continue at 32-byte granularity.
                // So we just mark that we don't need to run `scan_chunk` again for this area.
                last_hot_scan_end = scan_end.saturating_sub(128); // Overlap slightly
            }
        }
        
        pos += 32;
    }

    // Handle trailing bytes if any (less than 32)
    // Not critical for general scanning as entries are 32-byte aligned usually

    (entries, links)
}

// ═══════════════════════════════════════════════════════════════════════════════
// PyO3 INTERFACE
// ═══════════════════════════════════════════════════════════════════════════════

#[pyclass]
pub struct RustExFATScanner {
    chunk_size: usize,
    boot_params: std::sync::Arc<RwLock<Option<ExFatBootParams>>>,
    matcher: std::sync::Arc<EnhancedMatcher>,
}

#[pymethods]
impl RustExFATScanner {
    #[new]
    pub fn new() -> Self {
        RustExFATScanner {
            chunk_size: SCAN_CHUNK_SIZE,
            boot_params: std::sync::Arc::new(RwLock::new(None)),
            matcher: std::sync::Arc::new(EnhancedMatcher::new()),
        }
    }

    pub fn scan_file(&self, py: Python, file_path: String, offset: u64, limit: u64) -> PyResult<(Vec<ExFATEntry>, Vec<EnrichedLink>)> {
        let file = File::open(&file_path)
            .map_err(|e| pyo3::exceptions::PyIOError::new_err(format!("Cannot open {}: {}", file_path, e)))?;
        let mmap = unsafe { Mmap::map(&file)? };
        let data = mmap.as_ref();
        let file_size = data.len();

        let start_pos = offset as usize;
        let end_pos = if limit > 0 {
            (offset + limit).min(file_size as u64) as usize
        } else {
            file_size
        };

        if start_pos >= end_pos {
            return Ok((Vec::new(), Vec::new()));
        }

        if let Some(params) = find_boot_sector(data) {
            let mut writer = self.boot_params.write().unwrap();
            *writer = Some(params);
        }

        let range_data = &data[start_pos..end_pos];
        let num_chunks = (range_data.len() + self.chunk_size - 1) / self.chunk_size;

        let boot_params_lock = self.boot_params.clone();
        let matcher_arc = self.matcher.clone();
        
        // Parallel scan
        let (all_entries, all_links): (Vec<ExFATEntry>, Vec<EnrichedLink>) = py.allow_threads(|| {
            (0..num_chunks)
                .into_par_iter()
                .map(|chunk_idx| {
                    let start = chunk_idx * self.chunk_size;
                    let end = std::cmp::min(start + self.chunk_size, range_data.len());
                    let chunk = &range_data[start..end];
                    let chunk_base_offset = (start_pos + start) as u64;

                    // Create thread-local matcher
                    let mut local_matcher = matcher_arc.clone_fresh();

                    let (mut entries, links) = scan_for_entries_impl(chunk, chunk_base_offset, &mut local_matcher);

                    if let Ok(guard) = boot_params_lock.read() {
                        if let Some(ref params) = *guard {
                            for entry in &mut entries {
                                if entry.first_cluster >= 2 {
                                    entry.data_offset = cluster_to_offset(params, entry.first_cluster);
                                }
                            }
                        }
                    }

                    (entries, links)
                })
                .reduce(
                    || (Vec::new(), Vec::new()),
                    |mut acc, mut part| {
                        acc.0.append(&mut part.0);
                        acc.1.append(&mut part.1);
                        acc
                    }
                )
        });

        // Sorting
        // all_entries.sort_by_key(|e| e.offset); // Reduce/Collect already usually preserves order if map/reduce is ordered, but reduce is associative.
        // Parallel reduce does NOT guarantee order unless we use careful collection, but sorting is cheap enough compared to scan.
        // Actually simplistic reduce above joins generically. Let's just sort.
        
        let mut sorted_entries = all_entries;
        let mut sorted_links = all_links;
        
        sorted_entries.sort_by_key(|e| e.offset);
        sorted_links.sort_by_key(|l| l.offset);
        
        Ok((sorted_entries, sorted_links))
    }

    pub fn extract_file(&self, py: Python, file_path: &str, first_cluster: u32, size: u64, no_fat_chain: bool) -> PyResult<PyObject> {
        let file = File::open(file_path)
            .map_err(|e| pyo3::exceptions::PyIOError::new_err(format!("Cannot open {}: {}", file_path, e)))?;
        let mmap = unsafe { Mmap::map(&file)? };
        let data = mmap.as_ref();

        let params = find_boot_sector(data)
            .ok_or_else(|| pyo3::exceptions::PyValueError::new_err("exFAT boot sector not found in image"))?;

        if first_cluster < 2 || size == 0 {
            return Ok(PyBytes::new(py, &[]).into());
        }

        let content = extract_file_content(data, &params, first_cluster, size, no_fat_chain);
        Ok(PyBytes::new(py, &content).into())
    }

    pub fn extract_original_file(&self, py: Python, image_path: &str, entry_offset: u64) -> PyResult<(String, PyObject)> {
        let file = File::open(image_path)
            .map_err(|e| pyo3::exceptions::PyIOError::new_err(format!("Cannot open {}: {}", image_path, e)))?;
        let mmap = unsafe { Mmap::map(&file)? };
        let data = mmap.as_ref();

        let params = find_boot_sector(data)
            .ok_or_else(|| pyo3::exceptions::PyValueError::new_err("exFAT boot sector not found in image"))?;

        let off = entry_offset as usize;
        if off + 96 > data.len() {
            return Err(pyo3::exceptions::PyValueError::new_err(
                format!("Entry offset 0x{:X} is beyond image size", entry_offset)
            ));
        }

        let (entry, _consumed) = parse_entry_set(&data[off..], entry_offset)
            .ok_or_else(|| pyo3::exceptions::PyValueError::new_err(
                format!("No valid exFAT entry set at offset 0x{:X}", entry_offset)
            ))?;

        if entry.first_cluster < 2 || entry.size == 0 {
            return Err(pyo3::exceptions::PyValueError::new_err(
                format!("Invalid entry: cluster={}, size={}", entry.first_cluster, entry.size)
            ));
        }

        let content = extract_file_content(data, &params, entry.first_cluster, entry.size, entry.no_fat_chain);
        Ok((entry.filename.clone(), PyBytes::new(py, &content).into()))
    }

    pub fn extract_all_files(
        &self,
        py: Python,
        image_path: &str,
        entries: Vec<ExFATEntry>,
    ) -> PyResult<Vec<(String, PyObject, u64, bool)>> {
        let file = File::open(image_path)
            .map_err(|e| pyo3::exceptions::PyIOError::new_err(format!("Cannot open {}: {}", image_path, e)))?;
        let mmap = unsafe { Mmap::map(&file)? };
        let data = mmap.as_ref();

        let params = match find_boot_sector(data) {
            Some(p) => p,
            None => return Ok(Vec::new()),
        };

        let mut results = Vec::new();
        for entry in &entries {
            if entry.first_cluster < 2 || entry.size == 0 {
                continue;
            }

            if entry.size > 250 * 1024 * 1024 {
                continue;
            }

            let content = extract_file_content(
                data, &params,
                entry.first_cluster, entry.size, entry.no_fat_chain,
            );

            if content.is_empty() {
                continue;
            }
            
            let non_zero = content.iter().take(1024).filter(|&&b| b != 0).count();
            if non_zero < 5 {
                continue;
            }

            let filename = if entry.filename.is_empty() {
                format!("recovered_0x{:X}.bin", entry.offset)
            } else {
                entry.filename.clone()
            };

            results.push((
                filename,
                PyBytes::new(py, &content).into(),
                entry.offset,
                entry.is_deleted,
            ));
        }

        Ok(results)
    }

    pub fn get_boot_info(&self, py: Python, image_path: &str) -> PyResult<PyObject> {
        let file = File::open(image_path)
            .map_err(|e| pyo3::exceptions::PyIOError::new_err(format!("Cannot open {}: {}", image_path, e)))?;
        let mmap = unsafe { Mmap::map(&file)? };
        let data = mmap.as_ref();

        let dict = pyo3::types::PyDict::new(py);
        match find_boot_sector(data) {
            Some(params) => {
                dict.set_item("found", true)?;
                dict.set_item("boot_sector_offset", params.boot_sector_offset)?;
                dict.set_item("sector_size", params.sector_size)?;
                dict.set_item("cluster_size", params.cluster_size)?;
                dict.set_item("fat_offset", params.fat_offset)?;
                dict.set_item("fat_length_sectors", params.fat_length_sectors)?;
                dict.set_item("cluster_heap_offset", params.cluster_heap_offset)?;
                dict.set_item("cluster_count", params.cluster_count)?;
                dict.set_item("root_dir_cluster", params.root_dir_cluster)?;
            }
            None => {
                dict.set_item("found", false)?;
            }
        }
        Ok(dict.into())
    }

    #[staticmethod]
    pub fn scan_chunk(_py: Python, data: &[u8], base_offset: u64) -> PyResult<Vec<ExFATEntry>> {
        // Legacy support / Test helper
        // We create a temp matcher
        let mut matcher = EnhancedMatcher::new();
        let (entries, _) = scan_for_entries_impl(data, base_offset, &mut matcher);
        Ok(entries)
    }
}


