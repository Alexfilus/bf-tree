// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

#[cfg(target_arch = "aarch64")]
use std::arch::aarch64::*;

use std::cmp::Ordering;

#[inline]
#[cfg(target_arch = "x86_64")]
pub(crate) fn bytes_cmp_simd(a: &[u8], b: &[u8]) -> Ordering {
    let min_len = std::cmp::min(a.len(), b.len());

    if min_len < 16 {
        return a.cmp(b);
    }

    let mut offset = 0;

    unsafe {
        while offset + 16 <= min_len {
            let a_chunk = _mm_loadu_si128(a.as_ptr().add(offset) as *const __m128i);
            let b_chunk = _mm_loadu_si128(b.as_ptr().add(offset) as *const __m128i);

            let cmp = _mm_cmpeq_epi8(a_chunk, b_chunk);
            let mask = _mm_movemask_epi8(cmp) as u32;

            if mask != 0xFFFF {
                let first_diff = (!mask).trailing_zeros() as usize;
                let idx = offset + first_diff;
                return a[idx].cmp(&b[idx]);
            }

            offset += 16;
        }
    }

    a[offset..].cmp(&b[offset..])
}

#[inline]
#[cfg(target_arch = "aarch64")]
pub(crate) fn bytes_cmp_simd(a: &[u8], b: &[u8]) -> Ordering {
    let min_len = std::cmp::min(a.len(), b.len());

    if min_len < 16 {
        return a.cmp(b);
    }

    let mut offset = 0;

    unsafe {
        while offset + 16 <= min_len {
            let a_chunk = vld1q_u8(a.as_ptr().add(offset));
            let b_chunk = vld1q_u8(b.as_ptr().add(offset));

            let cmp = vceqq_u8(a_chunk, b_chunk);
            let cmp_u64: uint64x2_t = vreinterpretq_u64_u8(cmp);

            let low = vgetq_lane_u64(cmp_u64, 0);
            let high = vgetq_lane_u64(cmp_u64, 1);

            if low != u64::MAX {
                let first_diff = (!low).trailing_zeros() as usize / 8;
                let idx = offset + first_diff;
                return a[idx].cmp(&b[idx]);
            }
            if high != u64::MAX {
                let first_diff = 8 + (!high).trailing_zeros() as usize / 8;
                let idx = offset + first_diff;
                return a[idx].cmp(&b[idx]);
            }

            offset += 16;
        }
    }

    a[offset..].cmp(&b[offset..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bytes_cmp_empty() {
        assert_eq!(bytes_cmp_simd(&[], &[]), Ordering::Equal);
        assert_eq!(bytes_cmp_simd(&[1], &[]), Ordering::Greater);
        assert_eq!(bytes_cmp_simd(&[], &[1]), Ordering::Less);
    }

    #[test]
    fn test_bytes_cmp_identical() {
        let data = vec![1u8; 64];
        assert_eq!(bytes_cmp_simd(&data, &data), Ordering::Equal);
    }

    #[test]
    fn test_bytes_cmp_less() {
        let a = vec![1u8; 32];
        let b = vec![2u8; 32];
        assert_eq!(bytes_cmp_simd(&a, &b), Ordering::Less);
    }

    #[test]
    fn test_bytes_cmp_greater() {
        let a = vec![2u8; 32];
        let b = vec![1u8; 32];
        assert_eq!(bytes_cmp_simd(&a, &b), Ordering::Greater);
    }

    #[test]
    fn test_bytes_cmp_length_differs() {
        let a = b"hello";
        let b = b"hello world";
        assert_eq!(bytes_cmp_simd(a, b), Ordering::Less);
        assert_eq!(bytes_cmp_simd(b, a), Ordering::Greater);
    }

    #[test]
    fn test_bytes_cmp_short() {
        let a = b"abc";
        let b = b"abd";
        assert_eq!(bytes_cmp_simd(a, b), Ordering::Less);
    }

    #[test]
    fn test_bytes_cmp_exactly_16() {
        let a = b"0123456789abcdef";
        let b = b"0123456789abcdeg";
        assert_eq!(bytes_cmp_simd(a, b), Ordering::Less);
    }

    #[test]
    fn test_bytes_cmp_difference_at_position_17() {
        let a = b"0123456789abcdefX";
        let b = b"0123456789abcdefY";
        assert_eq!(bytes_cmp_simd(a, b), Ordering::Less);
    }

    #[test]
    fn test_simd_matches_scalar() {
        use rand_test::Rng;
        let mut rng = rand_test::rng();

        for _ in 0..1000 {
            let len_a = rng.random_range(0..256);
            let len_b = rng.random_range(0..256);
            let a: Vec<u8> = (0..len_a).map(|_| rng.random()).collect();
            let b: Vec<u8> = (0..len_b).map(|_| rng.random()).collect();

            let simd_result = bytes_cmp_simd(&a, &b);
            let scalar_result = a.cmp(&b);

            assert_eq!(simd_result, scalar_result, "Mismatch for a={:?}, b={:?}", a, b);
        }
    }

    #[test]
    fn test_simd_matches_scalar_with_common_prefix() {
        use rand_test::Rng;
        let mut rng = rand_test::rng();

        for _ in 0..1000 {
            let prefix_len = rng.random_range(0..128);
            let prefix: Vec<u8> = (0..prefix_len).map(|_| rng.random()).collect();

            let suffix_a_len = rng.random_range(0..128);
            let suffix_b_len = rng.random_range(0..128);
            let suffix_a: Vec<u8> = (0..suffix_a_len).map(|_| rng.random()).collect();
            let suffix_b: Vec<u8> = (0..suffix_b_len).map(|_| rng.random()).collect();

            let mut a = prefix.clone();
            a.extend(&suffix_a);
            let mut b = prefix.clone();
            b.extend(&suffix_b);

            let simd_result = bytes_cmp_simd(&a, &b);
            let scalar_result = a.cmp(&b);

            assert_eq!(simd_result, scalar_result);
        }
    }
}
