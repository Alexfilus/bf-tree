// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use std::cmp::Ordering;

use super::scan::first_mismatch;

#[inline]
pub(crate) fn bytes_cmp_simd(a: &[u8], b: &[u8]) -> Ordering {
    if let Some(idx) = first_mismatch(a, b) {
        return a[idx].cmp(&b[idx]);
    }

    a.len().cmp(&b.len())
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
    fn test_bytes_cmp_difference_after_32_byte_boundary() {
        let mut a = vec![7u8; 65];
        let mut b = a.clone();
        b[48] = 8;
        assert_eq!(bytes_cmp_simd(&a, &b), Ordering::Less);

        a[64] = 9;
        b[64] = 9;
        assert_eq!(bytes_cmp_simd(&a, &b), Ordering::Less);
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

            assert_eq!(
                simd_result, scalar_result,
                "Mismatch for a={:?}, b={:?}",
                a, b
            );
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
