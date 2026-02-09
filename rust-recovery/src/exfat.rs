use std::collections::HashSet;

/// Entry type markers
const ENTRY_FILE: u8 = 0x85;
const ENTRY_STREAM: u8 = 0xC0;
const ENTRY_FILENAME: u8 = 0xC1;
const ENTRY_DELETED_FILE: u8 = 0x05;
const ENTRY_DELETED_STREAM: u8 = 0x40;
const ENTRY_DELETED_FILENAME: u8 = 0x41;

/// Boot sector field offsets
const BS_FILE_SYSTEM_NAME: usize = 3;
const BS_FAT_OFFSET: usize = 80;
const BS_FAT_LENGTH: usize = 84;
const BS_CLUSTER_HEAP_OFFSET: usize = 88;
const BS_CLUSTER_COUNT: usize = 92;
const BS_FIRST_CLUSTER_OF_ROOT: usize = 96;
const BS_BYTES_PER_SECTOR_SHIFT: usize = 108;
const BS_SECTORS_PER_CLUSTER_SHIFT: usize = 109;

/// Stream Extension Entry field offsets
const SE_GENERAL_FLAGS: usize = 1;
const SE_NAME_LENGTH: usize = 3;
const SE_FIRST_CLUSTER: usize = 20;
const SE_DATA_LENGTH: usize = 24;

/// File Name Entry field offsets
const FN_FILE_NAME: usize = 2;

const DIRECTORY_ENTRY_SIZE: usize = 32;
const MAX_CLUSTER_SIZE: u64 = 32 * 1024 * 1024;
const MAX_EXTRACT_SIZE: u64 = 250 * 1024 * 1024;

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

#[derive(Clone, Debug)]
pub struct ExFatEntry {
    pub offset: u64,
    pub data_offset: Option<u64>,
    pub is_deleted: bool,
    pub filename: String,
    pub size: u64,
    pub first_cluster: u32,
    pub no_fat_chain: bool,
}

fn read_u16_le(data: &[u8], offset: usize) -> Option<u16> {
    data.get(offset..offset + 2)
        .and_then(|bytes| bytes.try_into().ok())
        .map(u16::from_le_bytes)
}

fn read_u32_le(data: &[u8], offset: usize) -> Option<u32> {
    data.get(offset..offset + 4)
        .and_then(|bytes| bytes.try_into().ok())
        .map(u32::from_le_bytes)
}

fn read_u64_le(data: &[u8], offset: usize) -> Option<u64> {
    data.get(offset..offset + 8)
        .and_then(|bytes| bytes.try_into().ok())
        .map(u64::from_le_bytes)
}

fn parse_boot_sector_at(data: &[u8], bs_offset: u64) -> Option<ExFatBootParams> {
    let off = usize::try_from(bs_offset).ok()?;
    if data.len() < off + 120 {
        return None;
    }

    if data.get(off + BS_FILE_SYSTEM_NAME..off + BS_FILE_SYSTEM_NAME + 8)? != b"EXFAT   " {
        return None;
    }

    let bytes_per_sector_shift = *data.get(off + BS_BYTES_PER_SECTOR_SHIFT)?;
    let sectors_per_cluster_shift = *data.get(off + BS_SECTORS_PER_CLUSTER_SHIFT)?;

    if !(9..=12).contains(&bytes_per_sector_shift) {
        return None;
    }
    if sectors_per_cluster_shift > 25 {
        return None;
    }

    let sector_size = 1u64 << bytes_per_sector_shift;
    let cluster_size = sector_size << sectors_per_cluster_shift;

    if cluster_size == 0 || cluster_size > MAX_CLUSTER_SIZE {
        return None;
    }

    let fat_offset_sectors = read_u32_le(data, off + BS_FAT_OFFSET)? as u64;
    let fat_length_sectors = read_u32_le(data, off + BS_FAT_LENGTH)?;
    let cluster_heap_offset_sectors = read_u32_le(data, off + BS_CLUSTER_HEAP_OFFSET)? as u64;
    let cluster_count = read_u32_le(data, off + BS_CLUSTER_COUNT)?;
    let root_dir_cluster = read_u32_le(data, off + BS_FIRST_CLUSTER_OF_ROOT)?;

    let fat_offset = fat_offset_sectors
        .checked_mul(sector_size)?
        .checked_add(bs_offset)?;
    let cluster_heap_offset = cluster_heap_offset_sectors
        .checked_mul(sector_size)?
        .checked_add(bs_offset)?;

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

pub fn find_boot_sector(data: &[u8]) -> Option<ExFatBootParams> {
    if let Some(params) = parse_boot_sector_at(data, 0) {
        return Some(params);
    }

    let search_limit = data.len().min(4 * 1024 * 1024);
    for offset in (512..search_limit).step_by(512) {
        if offset + 120 > data.len() {
            break;
        }
        if data.get(offset + 3..offset + 11) == Some(&b"EXFAT   "[..]) {
            if let Some(params) = parse_boot_sector_at(data, offset as u64) {
                return Some(params);
            }
        }
    }

    None
}

fn fat_next_cluster(data: &[u8], params: &ExFatBootParams, cluster: u32) -> Option<u32> {
    let offset_bytes = (cluster as u64).checked_mul(4)?;
    let fat_entry_offset = params.fat_offset.checked_add(offset_bytes)?;
    let offset = usize::try_from(fat_entry_offset).ok()?;
    read_u32_le(data, offset)
}

pub fn cluster_to_offset(params: &ExFatBootParams, cluster: u32) -> Option<u64> {
    if cluster < 2 {
        return None;
    }
    params
        .cluster_heap_offset
        .checked_add((cluster as u64).saturating_sub(2).checked_mul(params.cluster_size)?)
}

pub fn extract_file_content(
    data: &[u8],
    params: &ExFatBootParams,
    first_cluster: u32,
    file_size: u64,
    no_fat_chain: bool,
) -> Vec<u8> {
    if first_cluster < 2 || file_size == 0 {
        return Vec::new();
    }

    let actual_size = file_size.min(MAX_EXTRACT_SIZE);
    let mut content = Vec::with_capacity(actual_size as usize);
    let mut remaining = actual_size;
    let mut cluster = first_cluster;
    let mut visited = HashSet::new();
    let max_chain = params.cluster_count.saturating_add(1);

    while remaining > 0 {
        if cluster < 2 || cluster >= 0xFFFFFFF7 || cluster > max_chain {
            break;
        }
        if !visited.insert(cluster) {
            break;
        }

        let start = match cluster_to_offset(params, cluster) {
            Some(offset) => offset,
            None => break,
        };

        if start >= data.len() as u64 {
            break;
        }

        let to_read = remaining.min(params.cluster_size);
        let end = start.saturating_add(to_read).min(data.len() as u64);
        if end <= start {
            break;
        }

        content.extend_from_slice(&data[start as usize..end as usize]);
        let read_len = end - start;
        remaining = remaining.saturating_sub(read_len);

        if no_fat_chain {
            cluster = match cluster.checked_add(1) {
                Some(next) => next,
                None => break,
            };
        } else {
            let next_cluster = match fat_next_cluster(data, params, cluster) {
                Some(next) => next,
                None => break,
            };
            cluster = next_cluster;
        }
    }

    content.truncate(actual_size as usize);
    content
}

pub fn parse_entry_set(data: &[u8], base_offset: u64) -> Option<(ExFatEntry, usize)> {
    if data.len() < DIRECTORY_ENTRY_SIZE {
        return None;
    }

    let file_type = *data.get(0)?;
    let is_deleted = file_type == ENTRY_DELETED_FILE;

    if file_type != ENTRY_FILE && file_type != ENTRY_DELETED_FILE {
        return None;
    }

    let secondary_count = *data.get(1)? as usize;
    if secondary_count < 2 {
        return None;
    }

    let total_entries = 1 + secondary_count;
    let total_bytes = total_entries * DIRECTORY_ENTRY_SIZE;
    if data.len() < total_bytes {
        return None;
    }

    let se_offset = DIRECTORY_ENTRY_SIZE;
    let se_type = *data.get(se_offset)?;
    if se_type != ENTRY_STREAM && se_type != ENTRY_DELETED_STREAM {
        return None;
    }

    let general_flags = *data.get(se_offset + SE_GENERAL_FLAGS)?;
    let no_fat_chain = (general_flags & 0x02) != 0;
    let name_length = *data.get(se_offset + SE_NAME_LENGTH)? as usize;

    let first_cluster = read_u32_le(data, se_offset + SE_FIRST_CLUSTER)?;
    let file_size = read_u64_le(data, se_offset + SE_DATA_LENGTH)?;

    let mut filename = String::with_capacity(name_length);
    let mut chars_collected = 0;

    for i in 2..total_entries {
        let fn_offset = i * DIRECTORY_ENTRY_SIZE;
        if fn_offset + DIRECTORY_ENTRY_SIZE > data.len() {
            break;
        }

        let fn_type = *data.get(fn_offset)?;
        if fn_type != ENTRY_FILENAME && fn_type != ENTRY_DELETED_FILENAME {
            break;
        }

        for j in 0..15 {
            if chars_collected >= name_length {
                break;
            }
            let char_offset = fn_offset + FN_FILE_NAME + j * 2;
            let ch = match read_u16_le(data, char_offset) {
                Some(value) => value,
                None => break,
            };
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

    Some((
        ExFatEntry {
            offset: base_offset,
            data_offset: None,
            is_deleted,
            filename,
            size: file_size,
            first_cluster,
            no_fat_chain,
        },
        total_entries,
    ))
}

pub fn scan_for_entries(data: &[u8], base_offset: u64) -> Vec<ExFatEntry> {
    let mut entries = Vec::new();
    let mut pos = 0usize;

    while pos + DIRECTORY_ENTRY_SIZE <= data.len() {
        if let Some((entry, consumed)) = parse_entry_set(&data[pos..], base_offset + pos as u64) {
            entries.push(entry);
            pos = pos.saturating_add(consumed * DIRECTORY_ENTRY_SIZE);
        } else {
            pos = pos.saturating_add(DIRECTORY_ENTRY_SIZE);
        }
    }

    entries
}

pub fn populate_data_offsets(entries: &mut [ExFatEntry], params: &ExFatBootParams) {
    for entry in entries {
        entry.data_offset = cluster_to_offset(params, entry.first_cluster);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_boot_sector() -> Vec<u8> {
        let mut data = vec![0u8; 512];
        data[BS_FILE_SYSTEM_NAME..BS_FILE_SYSTEM_NAME + 8].copy_from_slice(b"EXFAT   ");
        data[BS_BYTES_PER_SECTOR_SHIFT] = 9;
        data[BS_SECTORS_PER_CLUSTER_SHIFT] = 0;
        data[BS_FAT_OFFSET..BS_FAT_OFFSET + 4].copy_from_slice(&(1u32.to_le_bytes()));
        data[BS_FAT_LENGTH..BS_FAT_LENGTH + 4].copy_from_slice(&(1u32.to_le_bytes()));
        data[BS_CLUSTER_HEAP_OFFSET..BS_CLUSTER_HEAP_OFFSET + 4]
            .copy_from_slice(&(2u32.to_le_bytes()));
        data[BS_CLUSTER_COUNT..BS_CLUSTER_COUNT + 4].copy_from_slice(&(8u32.to_le_bytes()));
        data[BS_FIRST_CLUSTER_OF_ROOT..BS_FIRST_CLUSTER_OF_ROOT + 4]
            .copy_from_slice(&(2u32.to_le_bytes()));
        data
    }

    fn build_entry_set() -> Vec<u8> {
        let mut data = vec![0u8; DIRECTORY_ENTRY_SIZE * 3];
        data[0] = ENTRY_FILE;
        data[1] = 2;

        let stream_offset = DIRECTORY_ENTRY_SIZE;
        data[stream_offset] = ENTRY_STREAM;
        data[stream_offset + SE_GENERAL_FLAGS] = 0x00;
        data[stream_offset + SE_NAME_LENGTH] = 5;
        data[stream_offset + SE_FIRST_CLUSTER..stream_offset + SE_FIRST_CLUSTER + 4]
            .copy_from_slice(&2u32.to_le_bytes());
        data[stream_offset + SE_DATA_LENGTH..stream_offset + SE_DATA_LENGTH + 8]
            .copy_from_slice(&10u64.to_le_bytes());

        let name_offset = DIRECTORY_ENTRY_SIZE * 2;
        data[name_offset] = ENTRY_FILENAME;
        let name_chars: [u16; 5] = [b'h' as u16, b'e' as u16, b'l' as u16, b'l' as u16, b'o' as u16];
        for (i, ch) in name_chars.iter().enumerate() {
            let start = name_offset + FN_FILE_NAME + i * 2;
            data[start..start + 2].copy_from_slice(&ch.to_le_bytes());
        }

        data
    }

    #[test]
    fn test_find_boot_sector() {
        let data = build_boot_sector();
        let params = find_boot_sector(&data).expect("boot sector should be found");
        assert_eq!(params.sector_size, 512);
        assert_eq!(params.cluster_size, 512);
        assert_eq!(params.fat_offset, 512);
        assert_eq!(params.cluster_heap_offset, 1024);
        assert_eq!(params.cluster_count, 8);
        assert_eq!(params.root_dir_cluster, 2);
    }

    #[test]
    fn test_parse_entry_set() {
        let data = build_entry_set();
        let (entry, consumed) = parse_entry_set(&data, 4096).expect("entry should parse");
        assert_eq!(consumed, 3);
        assert_eq!(entry.offset, 4096);
        assert_eq!(entry.filename, "hello");
        assert_eq!(entry.size, 10);
        assert_eq!(entry.first_cluster, 2);
        assert!(!entry.is_deleted);
    }

    #[test]
    fn test_parse_entry_set_bounds() {
        let data = vec![0u8; DIRECTORY_ENTRY_SIZE - 1];
        assert!(parse_entry_set(&data, 0).is_none());
    }

    #[test]
    fn test_extract_file_content_chain() {
        let mut data = vec![0u8; 3072];
        let params = ExFatBootParams {
            sector_size: 512,
            cluster_size: 512,
            fat_offset: 512,
            fat_length_sectors: 1,
            cluster_heap_offset: 1024,
            cluster_count: 4,
            root_dir_cluster: 2,
            boot_sector_offset: 0,
        };

        let fat_cluster2_offset = 512 + 2 * 4;
        data[fat_cluster2_offset..fat_cluster2_offset + 4].copy_from_slice(&3u32.to_le_bytes());
        let fat_cluster3_offset = 512 + 3 * 4;
        data[fat_cluster3_offset..fat_cluster3_offset + 4]
            .copy_from_slice(&0xFFFFFFFFu32.to_le_bytes());

        let cluster2_offset = 1024;
        data[cluster2_offset..cluster2_offset + 5].copy_from_slice(b"hello");
        let cluster3_offset = 1536;
        data[cluster3_offset..cluster3_offset + 5].copy_from_slice(b"world");

        let content = extract_file_content(&data, &params, 2, 700, false);
        assert_eq!(&content[..5], b"hello");
        assert_eq!(&content[512..517], b"world");
    }
}
