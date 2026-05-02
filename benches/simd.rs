// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

#![feature(portable_simd)]

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rand::prelude::*;
use std::simd::{cmp::SimdPartialEq, LaneCount, Simd, SupportedLaneCount};

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

use std::cmp::Ordering;

fn bytes_cmp_scalar(a: &[u8], b: &[u8]) -> Ordering {
    a.cmp(b)
}

fn common_prefix_len_simd(a: &[u8], b: &[u8]) -> u16 {
    let min_len = a.len().min(b.len());
    first_mismatch(a, b).unwrap_or(min_len) as u16
}

fn bytes_cmp_optimized(a: &[u8], b: &[u8]) -> Ordering {
    #[cfg(not(target_arch = "x86_64"))]
    {
        return a.cmp(b);
    }

    #[cfg(target_arch = "x86_64")]
    if let Some(idx) = first_mismatch(a, b) {
        return a[idx].cmp(&b[idx]);
    }

    #[cfg(target_arch = "x86_64")]
    a.len().cmp(&b.len())
}

fn first_mismatch(a: &[u8], b: &[u8]) -> Option<usize> {
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
