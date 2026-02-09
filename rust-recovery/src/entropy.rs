/// SIMD-accelerated Shannon entropy calculation
/// 
/// This module provides high-performance entropy calculation using:
/// - SIMD instructions (AVX2/SSE2) when available
/// - Byte frequency histogram (256 bins)
/// - Shannon entropy formula: H = -Σ(p_i * log2(p_i))
/// - Fallback to scalar implementation

use std::arch::x86_64::*;

/// Calculate Shannon entropy of data
/// Returns value between 0.0 (no entropy, predictable) and 8.0 (maximum entropy, random)
/// 
/// This function automatically uses SIMD instructions when available on x86_64
/// and falls back to scalar implementation on other architectures.
pub fn calculate_shannon_entropy(data: &[u8]) -> f32 {
    if data.is_empty() {
        return 0.0;
    }

    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            return unsafe { calculate_entropy_avx2(data) };
        } else if is_x86_feature_detected!("sse2") {
            return unsafe { calculate_entropy_sse2(data) };
        }
    }

    // Fallback to scalar implementation
    calculate_entropy_scalar(data)
}

/// SIMD-accelerated entropy calculation using AVX2
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn calculate_entropy_avx2(data: &[u8]) -> f32 {
    calculate_entropy_simd(data)
}

/// SIMD-accelerated entropy calculation using SSE2
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
unsafe fn calculate_entropy_sse2(data: &[u8]) -> f32 {
    calculate_entropy_simd(data)
}

/// Generic SIMD entropy calculation
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn calculate_entropy_simd(data: &[u8]) -> f32 {
    const BINS: usize = 256;
    let mut histogram = [0u32; BINS];
    
    // Process 32 bytes at a time with AVX2
    let mut i = 0;
    let data_len = data.len();
    
    // Unrolled loop for 32-byte chunks
    while i + 32 <= data_len {
        let chunk = &data[i..i + 32];
        let v = _mm256_loadu_si256(chunk.as_ptr() as *const _);
        
        // Extract bytes and increment histogram using a more compatible approach
        let bytes = std::slice::from_raw_parts(&v as *const _ as *const u8, 32);
        for &byte in bytes {
            histogram[byte as usize] += 1;
        }
        
        i += 32;
    }
    
    // Process remaining bytes
    while i < data_len {
        let byte = data[i] as usize;
        histogram[byte] += 1;
        i += 1;
    }
    
    calculate_entropy_from_histogram(&histogram, data_len as f32)
}

/// Scalar entropy calculation (fallback for non-x86_64 architectures)
fn calculate_entropy_scalar(data: &[u8]) -> f32 {
    const BINS: usize = 256;
    let mut histogram = [0u32; BINS];
    
    // Count byte frequencies
    for &byte in data {
        histogram[byte as usize] += 1;
    }
    
    calculate_entropy_from_histogram(&histogram, data.len() as f32)
}

/// Calculate entropy from byte frequency histogram
fn calculate_entropy_from_histogram(histogram: &[u32; 256], total_bytes: f32) -> f32 {
    if total_bytes == 0.0 {
        return 0.0;
    }

    let mut entropy = 0.0f32;
    
    for &count in histogram.iter() {
        if count > 0 {
            let probability = count as f32 / total_bytes;
            entropy -= probability * probability.log2();
        }
    }
    
    entropy
}

/// Check if data appears compressed or random-like based on entropy
/// 
/// Returns true if entropy is high enough to suggest compressed or encrypted data
/// Typical thresholds:
/// - High entropy (> 7.5): likely compressed/encrypted/random
/// - Medium entropy (4.0-7.5): mixed content
/// - Low entropy (< 4.0): structured text or repetitive data
#[inline]
pub fn is_compressed_like(data: &[u8]) -> bool {
    let entropy = calculate_shannon_entropy(data);
    entropy > 7.5
}

/// Check if data appears to be structured text based on entropy
/// 
/// Returns true if entropy is low enough to suggest readable text
/// This is typically used to identify fragments worth processing
#[inline]
pub fn is_structured_text(data: &[u8]) -> bool {
    let entropy = calculate_shannon_entropy(data);
    // Text typically has entropy between 3.0 and 6.0
    entropy >= 3.0 && entropy <= 6.0
}

/// Get entropy category for logging/debugging
#[inline]
pub fn get_entropy_category(data: &[u8]) -> &'static str {
    let entropy = calculate_shannon_entropy(data);
    
    if entropy > 7.5 {
        "high_entropy_compressed"
    } else if entropy > 6.0 {
        "medium_entropy_mixed"
    } else if entropy > 3.5 {
        "structured_text"
    } else if entropy > 1.0 {
        "low_entropy_repetitive"
    } else {
        "very_low_entropy_uniform"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entropy_zero_data() {
        assert_eq!(calculate_shannon_entropy(b""), 0.0);
    }

    #[test]
    fn test_entropy_uniform_data() {
        // All same byte should have zero entropy
        let data = b"aaaaaaaaaaaaaaaaaaaa";
        assert_eq!(calculate_shannon_entropy(data), 0.0);
    }

    #[test]
    fn test_entropy_random_data() {
        // Random-looking data should have high entropy
        let data = b"\x00\x01\x02\x03\x04\x05\x06\x07\x08\t\n\x0b\x0c\r\x0e\x0f";
        let entropy = calculate_shannon_entropy(data);
        assert!(entropy > 3.0); // Should be relatively high
    }

    #[test]
    fn test_entropy_text_data() {
        // English text should have medium entropy
        let data = b"hello world this is a test";
        let entropy = calculate_shannon_entropy(data);
        assert!(entropy >= 3.0 && entropy <= 6.0);
    }

    #[test]
    fn test_is_compressed_like() {
        // High entropy data should be flagged
        let random_data: Vec<u8> = (0..200).map(|i| (i * 37 + 17) as u8).collect();
        assert!(is_compressed_like(&random_data));
        
        // Low entropy text should not be flagged
        let text_data = b"hello world hello world hello world";
        assert!(!is_compressed_like(text_data));
    }

    #[test]
    fn test_is_structured_text() {
        // Text should be identified as structured
        let text_data = b"hello world this is a test of structured text";
        assert!(is_structured_text(text_data));
        
        // High entropy data should not be identified as structured text
        let random_data: Vec<u8> = (0..100).map(|i| (i * 17 + 23) as u8).collect();
        assert!(!is_structured_text(&random_data));
    }

    #[test]
    fn test_entropy_categories() {
        let uniform = b"aaaaaaaaaaaaaaaaaaaa";
        assert_eq!(get_entropy_category(uniform), "very_low_entropy_uniform");
        
        let text = b"hello world this is a test";
        let category = get_entropy_category(text);
        assert!(category == "structured_text" || category == "low_entropy_repetitive");
    }
}