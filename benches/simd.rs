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

#[cfg(target_arch = "x86_64")]
fn common_prefix_len_simd(a: &[u8], b: &[u8]) -> u16 {
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

#[cfg(target_arch = "aarch64")]
fn common_prefix_len_simd(a: &[u8], b: &[u8]) -> u16 {
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

use std::cmp::Ordering;

fn bytes_cmp_scalar(a: &[u8], b: &[u8]) -> Ordering {
    a.cmp(b)
}

#[cfg(target_arch = "x86_64")]
fn bytes_cmp_simd(a: &[u8], b: &[u8]) -> Ordering {
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

#[cfg(target_arch = "aarch64")]
fn bytes_cmp_simd(a: &[u8], b: &[u8]) -> Ordering {
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
        group.bench_with_input(BenchmarkId::new("simd", size), size, |bench, _| {
            bench.iter(|| bytes_cmp_simd(black_box(&a), black_box(&b)))
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
        group.bench_with_input(BenchmarkId::new("simd", size), size, |bench, _| {
            bench.iter(|| bytes_cmp_simd(black_box(&data), black_box(&data)))
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
        group.bench_with_input(BenchmarkId::new("simd", size), size, |bench, _| {
            bench.iter(|| bytes_cmp_simd(black_box(&a), black_box(&b)))
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
