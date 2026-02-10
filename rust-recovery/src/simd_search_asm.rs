//! Ручная ASM оптимизация для критических SIMD путей
#![allow(unsafe_code)]

use std::arch::asm;
use std::arch::x86_64::*;

/// AVX2-оптимизированный поиск с ручным ASM
/// Использует inline asm для точного контроля над планированием инструкций
#[target_feature(enable = "avx2")]
pub unsafe fn find_pattern_avx2_asm(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }

    let first_byte = needle[0];
    let needle_len = needle.len();
    let haystack_ptr = haystack.as_ptr();
    let haystack_len = haystack.len();
    
    // Создаем вектор для поиска первого байта
    let search_vec: __m256i;
    asm!(
        "vpbroadcastb {search}, {first_byte}",
        search = out(ymm_reg) search_vec,
        first_byte = in(reg_byte) first_byte,
        options(pure, nomem, nostack)
    );
    
    let mut i: usize = 0;
    let end = haystack_len.saturating_sub(needle_len);
    
    while i + 64 <= end {
        let mut mask1: u32;
        let mut mask2: u32;
        
        // Unrolled loop: обрабатываем 64 байта за итерацию
        // Prefetch следующих 128 байт
        asm!(
            // Prefetch next cache line (128 bytes ahead)
            "prefetcht0 [{ptr} + 128]",
            
            // Load first 32 bytes and compare
            "vmovdqu {chunk1}, [{ptr}]",
            "vpcmpeqb {cmp1}, {chunk1}, {search}",
            "vpmovmskb {mask1:e}, {cmp1}",
            
            // Load second 32 bytes and compare (in parallel)
            "vmovdqu {chunk2}, [{ptr} + 32]",
            "vpcmpeqb {cmp2}, {chunk2}, {search}",
            "vpmovmskb {mask2:e}, {chunk2}",
            
            ptr = in(reg) haystack_ptr.add(i),
            search = in(ymm_reg) search_vec,
            chunk1 = out(ymm_reg) _,
            chunk2 = out(ymm_reg) _,
            cmp1 = out(ymm_reg) _,
            cmp2 = out(ymm_reg) _,
            mask1 = out(reg) mask1,
            mask2 = out(reg) mask2,
            options(readonly, nostack)
        );
        
        // Обработка первой маски (32 байта)
        if mask1 != 0 {
            for bit in 0..32 {
                if (mask1 & (1 << bit)) != 0 {
                    let pos = i + bit;
                    if pos + needle_len <= haystack_len {
                        if verify_match_asm(&haystack[pos..pos + needle_len], needle) {
                            return Some(pos);
                        }
                    }
                }
            }
        }
        
        // Обработка второй маски (32 байта)
        if mask2 != 0 {
            for bit in 0..32 {
                if (mask2 & (1 << bit)) != 0 {
                    let pos = i + 32 + bit;
                    if pos + needle_len <= haystack_len {
                        if verify_match_asm(&haystack[pos..pos + needle_len], needle) {
                            return Some(pos);
                        }
                    }
                }
            }
        }
        
        i += 64;
    }
    
    // Fallback для оставшихся байт
    haystack[i..].windows(needle_len)
        .position(|window| window == needle)
        .map(|pos| i + pos)
}

/// Быстрая верификация совпадения с использованием SIMD
#[inline(always)]
pub unsafe fn verify_match_asm(window: &[u8], needle: &[u8]) -> bool {
    if needle.len() <= 32 {
        verify_match_simd_32(window, needle)
    } else {
        verify_match_scalar(window, needle)
    }
}

/// SIMD верификация для паттернов до 32 байт
#[target_feature(enable = "avx2")]
pub unsafe fn verify_match_simd_32(window: &[u8], needle: &[u8]) -> bool {
    if window.len() < needle.len() {
        return false;
    }
    
    // Для небольших паттернов используем AVX2
    let mut w_vec = [0u8; 32];
    let mut n_vec = [0u8; 32];
    
    w_vec[..needle.len()].copy_from_slice(&window[..needle.len()]);
    n_vec[..needle.len()].copy_from_slice(needle);
    
    let w = _mm256_loadu_si256(w_vec.as_ptr() as *const __m256i);
    let n = _mm256_loadu_si256(n_vec.as_ptr() as *const __m256i);
    
    let cmp = _mm256_cmpeq_epi8(w, n);
    let mask = _mm256_movemask_epi8(cmp) as u32;
    
    // Создаем маску для релевантных байт
    let relevant_mask = if needle.len() >= 32 {
        0xFFFFFFFF
    } else {
        (1u32 << needle.len()) - 1
    };
    
    (mask & relevant_mask) == relevant_mask
}

#[inline(always)]
fn verify_match_scalar(window: &[u8], needle: &[u8]) -> bool {
    window == needle
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_pattern_asm() {
        if is_x86_feature_detected!("avx2") {
            let haystack = b"test youtube.com search";
            let needle = b"youtube.com";
            unsafe {
                let pos = find_pattern_avx2_asm(haystack, needle);
                assert_eq!(pos, Some(5));
            }
        }
    }
}
