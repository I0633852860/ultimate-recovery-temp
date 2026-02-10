use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rust_recovery::simd_search::*;
use rust_recovery::simd_search_asm::*;
use rust_recovery::simd_block_scanner_asm::*;

fn bench_pattern_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("pattern_search");
    
    let haystack = vec![0u8; 1024 * 1024]; // 1MB
    let needle = b"youtube.com";
    
    group.bench_function("intrinsics_avx2", |b| {
        b.iter(|| {
            // This now calls the ASM version internally in simd_search.rs
            black_box(find_pattern_simd(&haystack, needle))
        });
    });
    
    group.bench_function("asm_avx2_direct", |b| {
        b.iter(|| {
            unsafe { black_box(find_pattern_avx2_asm(&haystack, needle)) }
        });
    });
    
    group.finish();
}

fn bench_block_scanner(c: &mut Criterion) {
    let mut group = c.benchmark_group("block_scanner");
    
    let block = AlignedBlock { data: [0x42; 64] };
    
    group.bench_function("standard_simd", |b| {
        b.iter(|| {
            black_box(scan_block_simd(&block.data[..32]))
        });
    });
    
    group.bench_function("asm_optimized_64", |b| {
        b.iter(|| {
            unsafe { black_box(scan_block_avx2_asm(&block)) }
        });
    });
    
    group.finish();
}

criterion_group!(benches, bench_pattern_search, bench_block_scanner);
criterion_main!(benches);
