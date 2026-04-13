// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

#[cfg(target_arch = "aarch64")]
use std::arch::aarch64::*;

#[inline]
pub(super) fn first_mismatch(a: &[u8], b: &[u8]) -> Option<usize> {
    let min_len = a.len().min(b.len());
    if min_len == 0 {
        return None;
    }

    #[cfg(target_arch = "x86_64")]
    {
        if min_len >= 32 && std::arch::is_x86_feature_detected!("avx2") {
            // SAFETY: The runtime check above guarantees AVX2 is available.
            return unsafe { first_mismatch_avx2(a, b, min_len) };
        }

        if min_len >= 16 {
            // SAFETY: SSE2 is part of the x86_64 baseline.
            return unsafe { first_mismatch_sse2(a, b, min_len) };
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        if min_len >= 16 {
            // SAFETY: NEON is required on aarch64.
            return unsafe { first_mismatch_neon(a, b, min_len) };
        }
    }

    scalar_mismatch_from(a, b, 0, min_len)
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn first_mismatch_avx2(a: &[u8], b: &[u8], min_len: usize) -> Option<usize> {
    let mut offset = 0;

    while offset + 32 <= min_len {
        // SAFETY: offset stays within min_len, and loadu handles unaligned pointers.
        let a_chunk = unsafe { _mm256_loadu_si256(a.as_ptr().add(offset) as *const __m256i) };
        // SAFETY: offset stays within min_len, and loadu handles unaligned pointers.
        let b_chunk = unsafe { _mm256_loadu_si256(b.as_ptr().add(offset) as *const __m256i) };

        let cmp = _mm256_cmpeq_epi8(a_chunk, b_chunk);
        let mask = _mm256_movemask_epi8(cmp) as u32;

        if mask != u32::MAX {
            let first_diff = (!mask).trailing_zeros() as usize;
            return Some(offset + first_diff);
        }

        offset += 32;
    }

    unsafe { first_mismatch_sse2_from(a, b, offset, min_len) }
}

#[cfg(target_arch = "x86_64")]
unsafe fn first_mismatch_sse2(a: &[u8], b: &[u8], min_len: usize) -> Option<usize> {
    unsafe { first_mismatch_sse2_from(a, b, 0, min_len) }
}

#[cfg(target_arch = "x86_64")]
unsafe fn first_mismatch_sse2_from(
    a: &[u8],
    b: &[u8],
    mut offset: usize,
    min_len: usize,
) -> Option<usize> {
    while offset + 32 <= min_len {
        // SAFETY: offset stays within min_len, and loadu handles unaligned pointers.
        let a_chunk0 = unsafe { _mm_loadu_si128(a.as_ptr().add(offset) as *const __m128i) };
        // SAFETY: offset stays within min_len, and loadu handles unaligned pointers.
        let b_chunk0 = unsafe { _mm_loadu_si128(b.as_ptr().add(offset) as *const __m128i) };
        let mask0 = _mm_movemask_epi8(_mm_cmpeq_epi8(a_chunk0, b_chunk0)) as u32;

        if mask0 != 0xFFFF {
            let first_diff = (!mask0).trailing_zeros() as usize;
            return Some(offset + first_diff);
        }

        // SAFETY: offset + 16 stays within min_len, and loadu handles unaligned pointers.
        let a_chunk1 = unsafe { _mm_loadu_si128(a.as_ptr().add(offset + 16) as *const __m128i) };
        // SAFETY: offset + 16 stays within min_len, and loadu handles unaligned pointers.
        let b_chunk1 = unsafe { _mm_loadu_si128(b.as_ptr().add(offset + 16) as *const __m128i) };
        let mask1 = _mm_movemask_epi8(_mm_cmpeq_epi8(a_chunk1, b_chunk1)) as u32;

        if mask1 != 0xFFFF {
            let first_diff = (!mask1).trailing_zeros() as usize;
            return Some(offset + 16 + first_diff);
        }

        offset += 32;
    }

    while offset + 16 <= min_len {
        // SAFETY: offset stays within min_len, and loadu handles unaligned pointers.
        let a_chunk = unsafe { _mm_loadu_si128(a.as_ptr().add(offset) as *const __m128i) };
        // SAFETY: offset stays within min_len, and loadu handles unaligned pointers.
        let b_chunk = unsafe { _mm_loadu_si128(b.as_ptr().add(offset) as *const __m128i) };

        let mask = _mm_movemask_epi8(_mm_cmpeq_epi8(a_chunk, b_chunk)) as u32;
        if mask != 0xFFFF {
            let first_diff = (!mask).trailing_zeros() as usize;
            return Some(offset + first_diff);
        }

        offset += 16;
    }

    scalar_mismatch_from(a, b, offset, min_len)
}

#[cfg(target_arch = "aarch64")]
unsafe fn first_mismatch_neon(a: &[u8], b: &[u8], min_len: usize) -> Option<usize> {
    let mut offset = 0;

    while offset + 32 <= min_len {
        if let Some(first_diff) =
            unsafe { neon_chunk_mismatch_offset(a.as_ptr().add(offset), b.as_ptr().add(offset)) }
        {
            return Some(offset + first_diff);
        }
        if let Some(first_diff) = unsafe {
            neon_chunk_mismatch_offset(a.as_ptr().add(offset + 16), b.as_ptr().add(offset + 16))
        } {
            return Some(offset + 16 + first_diff);
        }

        offset += 32;
    }

    while offset + 16 <= min_len {
        if let Some(first_diff) =
            unsafe { neon_chunk_mismatch_offset(a.as_ptr().add(offset), b.as_ptr().add(offset)) }
        {
            return Some(offset + first_diff);
        }

        offset += 16;
    }

    scalar_mismatch_from(a, b, offset, min_len)
}

#[cfg(target_arch = "aarch64")]
unsafe fn neon_chunk_mismatch_offset(a: *const u8, b: *const u8) -> Option<usize> {
    // SAFETY: The caller ensures the pointers reference at least 16 readable bytes.
    let a_chunk = unsafe { vld1q_u8(a) };
    // SAFETY: The caller ensures the pointers reference at least 16 readable bytes.
    let b_chunk = unsafe { vld1q_u8(b) };

    let cmp = vceqq_u8(a_chunk, b_chunk);
    let cmp_u64: uint64x2_t = vreinterpretq_u64_u8(cmp);

    let low = vgetq_lane_u64(cmp_u64, 0);
    if low != u64::MAX {
        return Some(first_diff_byte_in_all_ones_mask_u64(low));
    }

    let high = vgetq_lane_u64(cmp_u64, 1);
    if high != u64::MAX {
        return Some(8 + first_diff_byte_in_all_ones_mask_u64(high));
    }

    None
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

#[inline]
fn first_diff_byte_in_all_ones_mask_u64(eq_mask: u64) -> usize {
    #[cfg(target_endian = "little")]
    {
        (!eq_mask).trailing_zeros() as usize / 8
    }

    #[cfg(target_endian = "big")]
    {
        (!eq_mask).leading_zeros() as usize / 8
    }
}
