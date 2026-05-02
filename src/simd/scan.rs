// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use std::simd::{cmp::SimdPartialEq, LaneCount, Simd, SupportedLaneCount};

#[inline]
pub(super) fn first_mismatch(a: &[u8], b: &[u8]) -> Option<usize> {
    let min_len = a.len().min(b.len());
    if min_len == 0 {
        return None;
    }

    if min_len >= 16 {
        return first_mismatch_portable_simd(a, b, min_len);
    }

    scalar_mismatch_from(a, b, 0, min_len)
}

fn first_mismatch_portable_simd(a: &[u8], b: &[u8], min_len: usize) -> Option<usize> {
    let mut offset = 0;

    while offset + 32 <= min_len {
        if let Some(first_diff) = chunk_mismatch_offset::<32>(&a[offset..], &b[offset..]) {
            return Some(offset + first_diff);
        }

        offset += 32;
    }

    while offset + 16 <= min_len {
        if let Some(first_diff) = chunk_mismatch_offset::<16>(&a[offset..], &b[offset..]) {
            return Some(offset + first_diff);
        }

        offset += 16;
    }

    scalar_mismatch_from(a, b, offset, min_len)
}

#[inline]
fn chunk_mismatch_offset<const LANES: usize>(a: &[u8], b: &[u8]) -> Option<usize>
where
    LaneCount<LANES>: SupportedLaneCount,
{
    let a_chunk = Simd::<u8, LANES>::from_slice(&a[..LANES]);
    let b_chunk = Simd::<u8, LANES>::from_slice(&b[..LANES]);

    let mask = a_chunk.simd_eq(b_chunk).to_bitmask();
    let all_equal = if LANES == 64 {
        u64::MAX
    } else {
        (1u64 << LANES) - 1
    };

    if mask == all_equal {
        None
    } else {
        Some((!mask & all_equal).trailing_zeros() as usize)
    }
}

#[inline]
fn scalar_mismatch_from(a: &[u8], b: &[u8], mut offset: usize, min_len: usize) -> Option<usize> {
    const WORD_BYTES: usize = std::mem::size_of::<usize>();

    while offset + WORD_BYTES <= min_len {
        // SAFETY: offset stays within min_len and read_unaligned supports arbitrary alignment.
        let a_word = unsafe { std::ptr::read_unaligned(a.as_ptr().add(offset) as *const usize) };
        // SAFETY: offset stays within min_len and read_unaligned supports arbitrary alignment.
        let b_word = unsafe { std::ptr::read_unaligned(b.as_ptr().add(offset) as *const usize) };
        let diff = a_word ^ b_word;

        if diff != 0 {
            return Some(offset + first_diff_byte_in_word(diff));
        }

        offset += WORD_BYTES;
    }

    while offset < min_len {
        if a[offset] != b[offset] {
            return Some(offset);
        }
        offset += 1;
    }

    None
}

#[inline]
fn first_diff_byte_in_word(diff: usize) -> usize {
    #[cfg(target_endian = "little")]
    {
        diff.trailing_zeros() as usize / 8
    }

    #[cfg(target_endian = "big")]
    {
        diff.leading_zeros() as usize / 8
    }
}
