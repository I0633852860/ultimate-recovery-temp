// SIMD-Optimized Pattern Search
// Uses AVX2/SSE4.2 for ultra-fast pattern matching
// Optimized for Intel CPUs (OptiPlex 3070 Micro)

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

use crate::simd_block_scanner_asm::AlignedBlock;

/// Results of a 32-byte block scan
#[derive(Debug, Clone, Copy)]
pub struct BlockScanResult {
    pub is_empty: bool,
    pub has_metadata: bool,
    pub hot_mask: u32,
}

/// SIMD-accelerated pattern search with runtime dispatch
/// Returns offset of first match, or None
#[inline]
pub fn find_pattern_simd(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }

    // For small patterns, use optimized scalar search
    if needle.len() < 16 {
        return find_pattern_scalar(haystack, needle);
    }

    // Try SIMD search if available
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            // Safety: We checked for AVX2 support. Using manual ASM for extra speed.
            return unsafe { crate::simd_search_asm::find_pattern_avx2_asm(haystack, needle) };
        } else if is_x86_feature_detected!("sse4.2") {
            // Safety: We checked for SSE4.2 support via runtime detection.
            return unsafe { find_pattern_sse42(haystack, needle) };
        }
    }

    // Fallback to scalar
    find_pattern_scalar(haystack, needle)
}

/// Scalar pattern search (fallback)
#[inline]
fn find_pattern_scalar(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// AVX2-accelerated search (32 bytes at a time)
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn find_pattern_avx2(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    let first_byte = needle[0];
    let needle_len = needle.len();

    let first_byte_vec = _mm256_set1_epi8(first_byte as i8);

    let mut i = 0;
    let end = haystack.len().saturating_sub(needle_len);

    while i + 32 <= end {
        let chunk = _mm256_loadu_si256(haystack.as_ptr().add(i) as *const __m256i);

        let cmp = _mm256_cmpeq_epi8(chunk, first_byte_vec);

        let mask = _mm256_movemask_epi8(cmp);

        if mask != 0 {
            for bit in 0..32 {
                if (mask & (1 << bit)) != 0 {
                    let pos = i + bit;
                    if pos + needle_len <= haystack.len() {
                        if &haystack[pos..pos + needle_len] == needle {
                            return Some(pos);
                        }
                    }
                }
            }
        }

        i += 32;
    }

    haystack[i..]
        .windows(needle_len)
        .position(|window| window == needle)
        .map(|pos| i + pos)
}

/// SSE4.2-accelerated search (16 bytes at a time)
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse4.2")]
unsafe fn find_pattern_sse42(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    let first_byte = needle[0];
    let needle_len = needle.len();

    let first_byte_vec = _mm_set1_epi8(first_byte as i8);

    let mut i = 0;
    let end = haystack.len().saturating_sub(needle_len);

    while i + 16 <= end {
        let chunk = _mm_loadu_si128(haystack.as_ptr().add(i) as *const __m128i);

        let cmp = _mm_cmpeq_epi8(chunk, first_byte_vec);

        let mask = _mm_movemask_epi8(cmp);

        if mask != 0 {
            for bit in 0..16 {
                if (mask & (1 << bit)) != 0 {
                    let pos = i + bit;
                    if pos + needle_len <= haystack.len() {
                        if &haystack[pos..pos + needle_len] == needle {
                            return Some(pos);
                        }
                    }
                }
            }
        }

        i += 16;
    }

    haystack[i..]
        .windows(needle_len)
        .position(|window| window == needle)
        .map(|pos| i + pos)
}

/// Count pattern occurrences using SIMD
#[inline]
pub fn count_pattern_simd(haystack: &[u8], needle: &[u8]) -> usize {
    let mut count = 0;
    let mut offset = 0;

    while let Some(pos) = find_pattern_simd(&haystack[offset..], needle) {
        count += 1;
        offset += pos + 1;
        if offset >= haystack.len() {
            break;
        }
    }

    count
}

/// Main entry point for block scanning with runtime dispatch
#[inline]
pub fn scan_block_simd(block: &[u8]) -> BlockScanResult {
    if block.len() < 32 {
        return scan_block_scalar(block);
    }

    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            unsafe {
                let mut aligned_block = AlignedBlock { data: [0u8; 64] };
                let len = block.len().min(64);
                aligned_block.data[..len].copy_from_slice(&block[..len]);
                let res = crate::simd_block_scanner_asm::scan_block_avx2_asm(&aligned_block);
                return BlockScanResult {
                    is_empty: res.is_empty,
                    has_metadata: res.has_metadata,
                    hot_mask: res.hot_mask_low, // Using low mask for compat
                };
            }
        }
    }

    scan_block_scalar(block)
}

fn scan_block_scalar(block: &[u8]) -> BlockScanResult {
    let mut is_empty = true;
    let mut hot_mask = 0u32;
    let has_metadata = block[0] == 0x85;

    for (i, &b) in block.iter().enumerate().take(32) {
        if b != 0 {
            is_empty = false;
        }
        if b == b'y' || b == b'h' || b == b'{' || b == b'v' || b == b'/' {
            hot_mask |= 1 << i;
        }
    }

    BlockScanResult {
        is_empty,
        has_metadata,
        hot_mask,
    }
}

/// AVX2 Optimized Block Scanner
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn fast_scan_block_avx2(block: &[u8]) -> BlockScanResult {
    let ptr = block.as_ptr() as *const __m256i;
    let chunk = _mm256_loadu_si256(ptr);

    // 1. Check for zeros (Empty Block)
    let zero = _mm256_setzero_si256();
    let cmp_zero = _mm256_cmpeq_epi8(chunk, zero);
    let mask_zero = _mm256_movemask_epi8(cmp_zero) as u32;
    let is_empty = mask_zero == 0xFFFFFFFF;

    // 2. Check for Metadata (0x85 at position 0)
    let has_metadata = *block.get_unchecked(0) == 0x85;

    // 3. Check for Hot Content characters
    let v_y = _mm256_set1_epi8(b'y' as i8);
    let v_h = _mm256_set1_epi8(b'h' as i8);
    let v_curly = _mm256_set1_epi8(b'{' as i8);
    let v_v = _mm256_set1_epi8(b'v' as i8);
    let v_slash = _mm256_set1_epi8(b'/' as i8);

    let eq_y = _mm256_cmpeq_epi8(chunk, v_y);
    let eq_h = _mm256_cmpeq_epi8(chunk, v_h);
    let eq_curly = _mm256_cmpeq_epi8(chunk, v_curly);
    let eq_v = _mm256_cmpeq_epi8(chunk, v_v);
    let eq_slash = _mm256_cmpeq_epi8(chunk, v_slash);

    let hot = _mm256_or_si256(
        _mm256_or_si256(eq_y, eq_h),
        _mm256_or_si256(
            eq_curly,
            _mm256_or_si256(eq_v, eq_slash)
        )
    );

    let hot_mask = _mm256_movemask_epi8(hot) as u32;

    BlockScanResult {
        is_empty,
        has_metadata,
        hot_mask,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_pattern_simd() {
        let haystack = b"youtube.com/watch?v=dQw4w9WgXcQ youtube.com/watch?v=abc123";
        let needle = b"youtube.com";

        let pos = find_pattern_simd(haystack, needle);
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn test_count_pattern_simd() {
        let haystack = b"youtube.com/watch?v=dQw4w9WgXcQ youtube.com/watch?v=abc123";
        let needle = b"youtube.com";

        let count = count_pattern_simd(haystack, needle);
        assert_eq!(count, 2);
    }

    #[test]
    fn test_small_pattern() {
        let haystack = b"abcdefghijklmnop";
        let needle = b"def";

        let pos = find_pattern_simd(haystack, needle);
        assert_eq!(pos, Some(3));
    }

    #[test]
    fn test_fast_scan_block() {
        #[repr(align(32))]
        struct Aligned([u8; 32]);

        let empty = Aligned([0u8; 32]);
        let res = scan_block_simd(&empty.0);
        assert!(res.is_empty);
        assert!(!res.has_metadata);
        assert_eq!(res.hot_mask, 0);

        let mut meta = Aligned([0u8; 32]);
        meta.0[0] = 0x85;
        let res = scan_block_simd(&meta.0);
        assert!(!res.is_empty);
        assert!(res.has_metadata);

        let mut hot = Aligned([0u8; 32]);
        hot.0[0] = b'y';
        hot.0[1] = b'o';
        hot.0[2] = b'u';
        let res = scan_block_simd(&hot.0);
        assert!(!res.is_empty);
        assert!(res.hot_mask & 1 != 0);
    }
}
