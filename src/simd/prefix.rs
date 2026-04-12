// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

#[cfg(target_arch = "aarch64")]
use std::arch::aarch64::*;

#[inline]
#[cfg(target_arch = "x86_64")]
pub(crate) fn common_prefix_len_simd(a: &[u8], b: &[u8]) -> u16 {
    let min_len = std::cmp::min(a.len(), b.len());

    if min_len < 16 {
        return common_prefix_len_scalar(a, b);
    }

    let mut prefix_len: usize = 0;

    unsafe {
        while prefix_len + 16 <= min_len {
            let a_chunk = _mm_loadu_si128(a.as_ptr().add(prefix_len) as *const __m128i);
            let b_chunk = _mm_loadu_si128(b.as_ptr().add(prefix_len) as *const __m128i);

            let cmp = _mm_cmpeq_epi8(a_chunk, b_chunk);
            let mask = _mm_movemask_epi8(cmp) as u32;

            if mask != 0xFFFF {
                let first_diff = (!mask).trailing_zeros() as usize;
                return (prefix_len + first_diff) as u16;
            }

            prefix_len += 16;
        }
    }

    while prefix_len < min_len {
        if a[prefix_len] != b[prefix_len] {
            return prefix_len as u16;
        }
        prefix_len += 1;
    }

    prefix_len as u16
}

#[inline]
#[cfg(target_arch = "aarch64")]
pub(crate) fn common_prefix_len_simd(a: &[u8], b: &[u8]) -> u16 {
    let min_len = std::cmp::min(a.len(), b.len());

    if min_len < 16 {
        return common_prefix_len_scalar(a, b);
    }

    let mut prefix_len: usize = 0;

    unsafe {
        while prefix_len + 16 <= min_len {
            let a_chunk = vld1q_u8(a.as_ptr().add(prefix_len));
            let b_chunk = vld1q_u8(b.as_ptr().add(prefix_len));

            let cmp = vceqq_u8(a_chunk, b_chunk);
            let cmp_u64: uint64x2_t = vreinterpretq_u64_u8(cmp);

            let low = vgetq_lane_u64(cmp_u64, 0);
            let high = vgetq_lane_u64(cmp_u64, 1);

            if low != u64::MAX {
                let first_diff = (!low).trailing_zeros() as usize / 8;
                return (prefix_len + first_diff) as u16;
            }
            if high != u64::MAX {
                let first_diff = 8 + (!high).trailing_zeros() as usize / 8;
                return (prefix_len + first_diff) as u16;
            }

            prefix_len += 16;
        }
    }

    while prefix_len < min_len {
        if a[prefix_len] != b[prefix_len] {
            return prefix_len as u16;
        }
        prefix_len += 1;
    }

    prefix_len as u16
}

#[inline]
pub(crate) fn common_prefix_len_scalar(a: &[u8], b: &[u8]) -> u16 {
    let mut prefix_len = 0;
    let min_len = std::cmp::min(a.len(), b.len());
    for i in 0..min_len {
        if a[i] == b[i] {
            prefix_len += 1;
        } else {
            break;
        }
    }
    prefix_len
}

#[cfg(test)]
mod tests {
    use super::*;

    fn common_prefix_len_reference(a: &[u8], b: &[u8]) -> u16 {
        let mut prefix_len = 0;
        for i in 0..std::cmp::min(a.len(), b.len()) {
            if a[i] == b[i] {
                prefix_len += 1;
            } else {
                break;
            }
        }
        prefix_len
    }

    #[test]
    fn test_common_prefix_empty() {
        assert_eq!(common_prefix_len_simd(&[], &[]), 0);
        assert_eq!(common_prefix_len_simd(&[1, 2, 3], &[]), 0);
        assert_eq!(common_prefix_len_simd(&[], &[1, 2, 3]), 0);
    }

    #[test]
    fn test_common_prefix_identical() {
        let data = vec![1u8; 64];
        assert_eq!(common_prefix_len_simd(&data, &data), 64);
    }

    #[test]
    fn test_common_prefix_no_match() {
        let a = vec![1u8; 32];
        let b = vec![2u8; 32];
        assert_eq!(common_prefix_len_simd(&a, &b), 0);
    }

    #[test]
    fn test_common_prefix_partial() {
        let a = b"hello world this is a test";
        let b = b"hello world this is different";
        let expected = common_prefix_len_reference(a, b);
        assert_eq!(common_prefix_len_simd(a, b), expected);
    }

    #[test]
    fn test_common_prefix_short() {
        let a = b"hello";
        let b = b"help";
        assert_eq!(common_prefix_len_simd(a, b), 3);
    }

    #[test]
    fn test_common_prefix_exactly_16() {
        let a = b"0123456789abcdefXXX";
        let b = b"0123456789abcdefYYY";
        assert_eq!(common_prefix_len_simd(a, b), 16);
    }

    #[test]
    fn test_common_prefix_boundary() {
        for len in 0..100 {
            let a: Vec<u8> = (0..len).map(|i| (i % 256) as u8).collect();
            let b = a.clone();
            assert_eq!(common_prefix_len_simd(&a, &b), len as u16);
        }
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

            let simd_result = common_prefix_len_simd(&a, &b);
            let scalar_result = common_prefix_len_reference(&a, &b);

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

            let simd_result = common_prefix_len_simd(&a, &b);
            let scalar_result = common_prefix_len_reference(&a, &b);

            assert_eq!(simd_result, scalar_result);
        }
    }
}
