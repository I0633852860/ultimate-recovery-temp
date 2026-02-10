//! Ручная ASM оптимизация для block scanning

use std::arch::asm;
use std::arch::x86_64::*;

#[repr(C, align(64))]
#[derive(Debug, Clone, Copy)]
pub struct AlignedBlock {
    pub data: [u8; 64],
}

/// Результат сканирования блока с дополнительными метриками
#[derive(Debug, Clone, Copy)]
pub struct BlockScanResultExt {
    pub is_empty: bool,
    pub has_metadata: bool,
    pub hot_mask_low: u32,   // Первые 32 байта
    pub hot_mask_high: u32,  // Вторые 32 байта
    pub zero_count: u8,      // Количество нулевых байт
    pub high_entropy: bool,  // Высокая энтропия (>6.5)
}

/// Супер-оптимизированный сканер блоков с AVX2
#[target_feature(enable = "avx2", enable = "bmi2")]
pub unsafe fn scan_block_avx2_asm(block: &AlignedBlock) -> BlockScanResultExt {
    let ptr = block.data.as_ptr();
    
    let mut mask_zero_low: u32;
    let mut mask_zero_high: u32;
    let mut has_metadata: u8;
    
    // Векторы для hot symbols
    let v_y = _mm256_set1_epi8(b'y' as i8);
    let v_h = _mm256_set1_epi8(b'h' as i8);
    let v_curly = _mm256_set1_epi8(b'{' as i8);
    let v_v = _mm256_set1_epi8(b'v' as i8);
    let v_slash = _mm256_set1_epi8(b'/' as i8);
    
    asm!(
        // Prefetch данных в L1 кэш
        "prefetcht0 [{ptr}]",
        
        // Load первых 32 байт
        "vmovdqa {chunk_low}, [{ptr}]",
        
        // Load вторых 32 байт (параллельно)
        "vmovdqa {chunk_high}, [{ptr} + 32]",
        
        // Проверка на zeros (первые 32 байта)
        "vpxor {zero}, {zero}, {zero}",
        "vpcmpeqb {cmp_zero_low}, {chunk_low}, {zero}",
        "vpmovmskb {mask_zero_low:e}, {cmp_zero_low}",
        
        // Проверка на zeros (вторые 32 байта)
        "vpcmpeqb {cmp_zero_high}, {chunk_high}, {zero}",
        "vpmovmskb {mask_zero_high:e}, {cmp_zero_high}",
        
        // Проверка metadata (0x85) в первом байте
        "movzx {metadata_tmp:e}, byte ptr [{ptr}]",
        "cmp {metadata_tmp:e}, 0x85",
        "sete {metadata}",
        
        ptr = in(reg) ptr,
        chunk_low = out(ymm_reg) _,
        chunk_high = out(ymm_reg) _,
        zero = out(ymm_reg) _,
        cmp_zero_low = out(ymm_reg) _,
        cmp_zero_high = out(ymm_reg) _,
        mask_zero_low = out(reg) mask_zero_low,
        mask_zero_high = out(reg) mask_zero_high,
        metadata_tmp = out(reg) _,
        metadata = out(reg_byte) has_metadata,
        options(readonly, nostack)
    );
    
    // Поиск hot symbols (используем Rust intrinsics для clarity)
    let chunk_low = _mm256_load_si256(ptr as *const __m256i);
    let chunk_high = _mm256_load_si256(ptr.add(32) as *const __m256i);
    
    // Hot symbols в первых 32 байтах
    let hot_low = _mm256_or_si256(
        _mm256_or_si256(
            _mm256_cmpeq_epi8(chunk_low, v_y),
            _mm256_cmpeq_epi8(chunk_low, v_h)
        ),
        _mm256_or_si256(
            _mm256_cmpeq_epi8(chunk_low, v_curly),
            _mm256_or_si256(
                _mm256_cmpeq_epi8(chunk_low, v_v),
                _mm256_cmpeq_epi8(chunk_low, v_slash)
            )
        )
    );
    
    // Hot symbols во вторых 32 байтах
    let hot_high = _mm256_or_si256(
        _mm256_or_si256(
            _mm256_cmpeq_epi8(chunk_high, v_y),
            _mm256_cmpeq_epi8(chunk_high, v_h)
        ),
        _mm256_or_si256(
            _mm256_cmpeq_epi8(chunk_high, v_curly),
            _mm256_or_si256(
                _mm256_cmpeq_epi8(chunk_high, v_v),
                _mm256_cmpeq_epi8(chunk_high, v_slash)
            )
        )
    );
    
    let mask_hot_low = _mm256_movemask_epi8(hot_low) as u32;
    let mask_hot_high = _mm256_movemask_epi8(hot_high) as u32;
    
    // Подсчет нулевых байт (popcnt на масках)
    let zero_count = (mask_zero_low.count_ones() + mask_zero_high.count_ones()) as u8;
    
    // Определение пустого блока
    let is_empty = (mask_zero_low == 0xFFFFFFFF) && (mask_zero_high == 0xFFFFFFFF);
    
    // Быстрая эвристика для определения высокой энтропии
    // Если меньше 8 нулевых байт и есть hot symbols - вероятно высокая энтропия
    let high_entropy = zero_count < 8 && (mask_hot_low != 0 || mask_hot_high != 0);
    
    BlockScanResultExt {
        is_empty,
        has_metadata: has_metadata != 0,
        hot_mask_low: mask_hot_low,
        hot_mask_high: mask_hot_high,
        zero_count,
        high_entropy,
    }
}

/// Batch сканирование нескольких блоков (для лучшего cache reuse)
#[target_feature(enable = "avx2")]
pub unsafe fn scan_blocks_batch_asm(
    blocks: &[AlignedBlock],
    results: &mut [BlockScanResultExt]
) {
    assert_eq!(blocks.len(), results.len());
    
    for i in 0..blocks.len() {
        // Prefetch следующего блока
        if i + 1 < blocks.len() {
            let next_ptr = blocks[i + 1].data.as_ptr();
            asm!(
                "prefetcht0 [{ptr}]",
                ptr = in(reg) next_ptr,
                options(readonly, nostack)
            );
        }
        
        results[i] = scan_block_avx2_asm(&blocks[i]);
    }
}
