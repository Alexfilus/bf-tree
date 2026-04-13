// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rand::prelude::*;

fn common_prefix_len_scalar(a: &[u8], b: &[u8]) -> u16 {
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

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

#[cfg(target_arch = "aarch64")]
use std::arch::aarch64::*;

use std::cmp::Ordering;

fn bytes_cmp_scalar(a: &[u8], b: &[u8]) -> Ordering {
    a.cmp(b)
}

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
fn common_prefix_len_simd(a: &[u8], b: &[u8]) -> u16 {
    let min_len = a.len().min(b.len());
    first_mismatch(a, b).unwrap_or(min_len) as u16
}

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
fn bytes_cmp_optimized(a: &[u8], b: &[u8]) -> Ordering {
    #[cfg(target_arch = "x86_64")]
    {
        if let Some(idx) = first_mismatch(a, b) {
            return a[idx].cmp(&b[idx]);
        }

        return a.len().cmp(&b.len());
    }

    #[cfg(not(target_arch = "x86_64"))]
    {
        a.cmp(b)
    }
}

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
fn first_mismatch(a: &[u8], b: &[u8]) -> Option<usize> {
    let min_len = a.len().min(b.len());
    if min_len == 0 {
        return None;
    }

    #[cfg(target_arch = "x86_64")]
    {
        if min_len >= 32 && std::arch::is_x86_feature_detected!("avx2") {
            return unsafe { first_mismatch_avx2(a, b, min_len) };
        }

        if min_len >= 16 {
            return unsafe { first_mismatch_sse2(a, b, min_len) };
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        if min_len >= 16 {
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
        let a_chunk = unsafe { _mm256_loadu_si256(a.as_ptr().add(offset) as *const __m256i) };
        let b_chunk = unsafe { _mm256_loadu_si256(b.as_ptr().add(offset) as *const __m256i) };
        let mask = _mm256_movemask_epi8(_mm256_cmpeq_epi8(a_chunk, b_chunk)) as u32;

        if mask != u32::MAX {
            return Some(offset + (!mask).trailing_zeros() as usize);
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
        let a_chunk0 = unsafe { _mm_loadu_si128(a.as_ptr().add(offset) as *const __m128i) };
        let b_chunk0 = unsafe { _mm_loadu_si128(b.as_ptr().add(offset) as *const __m128i) };
        let mask0 = _mm_movemask_epi8(_mm_cmpeq_epi8(a_chunk0, b_chunk0)) as u32;

        if mask0 != 0xFFFF {
            return Some(offset + (!mask0).trailing_zeros() as usize);
        }

        let a_chunk1 = unsafe { _mm_loadu_si128(a.as_ptr().add(offset + 16) as *const __m128i) };
        let b_chunk1 = unsafe { _mm_loadu_si128(b.as_ptr().add(offset + 16) as *const __m128i) };
        let mask1 = _mm_movemask_epi8(_mm_cmpeq_epi8(a_chunk1, b_chunk1)) as u32;

        if mask1 != 0xFFFF {
            return Some(offset + 16 + (!mask1).trailing_zeros() as usize);
        }

        offset += 32;
    }

    while offset + 16 <= min_len {
        let a_chunk = unsafe { _mm_loadu_si128(a.as_ptr().add(offset) as *const __m128i) };
        let b_chunk = unsafe { _mm_loadu_si128(b.as_ptr().add(offset) as *const __m128i) };
        let mask = _mm_movemask_epi8(_mm_cmpeq_epi8(a_chunk, b_chunk)) as u32;

        if mask != 0xFFFF {
            return Some(offset + (!mask).trailing_zeros() as usize);
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
    let a_chunk = unsafe { vld1q_u8(a) };
    let b_chunk = unsafe { vld1q_u8(b) };
    let cmp_u64: uint64x2_t = vreinterpretq_u64_u8(vceqq_u8(a_chunk, b_chunk));

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

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
fn scalar_mismatch_from(a: &[u8], b: &[u8], mut offset: usize, min_len: usize) -> Option<usize> {
    const WORD_BYTES: usize = std::mem::size_of::<usize>();

    while offset + WORD_BYTES <= min_len {
        let a_word = unsafe { std::ptr::read_unaligned(a.as_ptr().add(offset) as *const usize) };
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

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
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

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
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

fn generate_data_with_common_prefix(size: usize, common_prefix_ratio: f64) -> (Vec<u8>, Vec<u8>) {
    let mut rng = rand::thread_rng();
    let common_len = (size as f64 * common_prefix_ratio) as usize;

    let prefix: Vec<u8> = (0..common_len).map(|_| rng.gen()).collect();

    let mut a = prefix.clone();
    a.extend((0..(size - common_len)).map(|_| rng.gen::<u8>()));

    let mut b = prefix;
    b.extend((0..(size - common_len)).map(|_| rng.gen::<u8>()));

    (a, b)
}

fn bench_common_prefix_len(c: &mut Criterion) {
    let mut group = c.benchmark_group("common_prefix_len");

    for size in [16, 64, 256, 1024].iter() {
        let (a, b) = generate_data_with_common_prefix(*size, 0.5);
        group.throughput(Throughput::Bytes(*size as u64));

        group.bench_with_input(BenchmarkId::new("scalar", size), size, |bench, _| {
            bench.iter(|| common_prefix_len_scalar(black_box(&a), black_box(&b)))
        });

        #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
        group.bench_with_input(BenchmarkId::new("simd", size), size, |bench, _| {
            bench.iter(|| common_prefix_len_simd(black_box(&a), black_box(&b)))
        });
    }

    group.finish();
}

fn bench_bytes_cmp(c: &mut Criterion) {
    let mut group = c.benchmark_group("bytes_cmp");

    for size in [16, 64, 256, 1024].iter() {
        let (a, b) = generate_data_with_common_prefix(*size, 0.5);
        group.throughput(Throughput::Bytes(*size as u64));

        group.bench_with_input(BenchmarkId::new("scalar", size), size, |bench, _| {
            bench.iter(|| bytes_cmp_scalar(black_box(&a), black_box(&b)))
        });

        #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
        group.bench_with_input(BenchmarkId::new("optimized", size), size, |bench, _| {
            bench.iter(|| bytes_cmp_optimized(black_box(&a), black_box(&b)))
        });
    }

    group.finish();
}

fn bench_bytes_cmp_identical(c: &mut Criterion) {
    let mut group = c.benchmark_group("bytes_cmp_identical");

    for size in [16, 64, 256, 1024].iter() {
        let data: Vec<u8> = (0..*size).map(|i| (i % 256) as u8).collect();
        group.throughput(Throughput::Bytes(*size as u64));

        group.bench_with_input(BenchmarkId::new("scalar", size), size, |bench, _| {
            bench.iter(|| bytes_cmp_scalar(black_box(&data), black_box(&data)))
        });

        #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
        group.bench_with_input(BenchmarkId::new("optimized", size), size, |bench, _| {
            bench.iter(|| bytes_cmp_optimized(black_box(&data), black_box(&data)))
        });
    }

    group.finish();
}

fn bench_bytes_cmp_early_mismatch(c: &mut Criterion) {
    let mut group = c.benchmark_group("bytes_cmp_early_mismatch");

    for size in [16, 64, 256, 1024].iter() {
        let a: Vec<u8> = (0..*size).map(|i| (i % 256) as u8).collect();
        let mut b = a.clone();
        b[0] = 255;
        group.throughput(Throughput::Bytes(*size as u64));

        group.bench_with_input(BenchmarkId::new("scalar", size), size, |bench, _| {
            bench.iter(|| bytes_cmp_scalar(black_box(&a), black_box(&b)))
        });

        #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
        group.bench_with_input(BenchmarkId::new("optimized", size), size, |bench, _| {
            bench.iter(|| bytes_cmp_optimized(black_box(&a), black_box(&b)))
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_common_prefix_len,
    bench_bytes_cmp,
    bench_bytes_cmp_identical,
    bench_bytes_cmp_early_mismatch
);
criterion_main!(benches);
