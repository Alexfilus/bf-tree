# SIMD Tuning Report (x86_64)

Goal: take the five tuning hypotheses suggested for closing the gap between
the modest x86 SIMD speedup and the reported ~4× ARM speedup, apply each
one in isolation on the `simd` branch, and measure the impact.

## Setup
- Workload: `bftreebench-inmemory` from `benchmark/bench_bftree.toml`
  (100M records, 16-byte keys, uniform 100% read, 16,384 MB memory budget,
  in-memory backend, 32 threads × 2 thread groups, 5 iterations × 10 s).
- Command: `env SHUMAI_FILTER="inmemory" MIMALLOC_LARGE_OS_PAGES=1 cargo run --bin bftree --release`
- Host: `kazik`, AMD EPYC-Milan (Zen 3, AVX2/BMI2/SHA-NI but no AVX-512),
  2 vCPUs / 2 physical cores, 16 GiB RAM, kernel 6.8.0-106-generic.
- Base branch: `simd` @ `9aacdde54de607a2a6fc7257564f121aeb5043d1`.
- Each variant was applied alone on top of `simd`, rebuilt, and benchmarked.
  The "Combined" row applies all five simultaneously.
- Build patch applied to all variants: enable an empty `spdk` feature on
  `bf-tree` and `benchmark` so the binary builds without the optional
  SPDK dependency (mirrors `fix-benchmark-spdk-feature`).

## Variants

| Tag | Change |
|---|---|
| baseline | Plain `simd` branch (rerun from earlier comparison report) |
| v1 | `benchmark/.cargo/config.toml` with `rustflags = ["-C", "target-cpu=native"]` |
| v2 | Fast-path `bytes_cmp_simd_16` for `len(a) == len(b) == 16` in `src/simd/compare.rs` |
| v3 | `#[inline(always)]` on `bytes_cmp_simd`, `common_prefix_len_simd`, `first_mismatch`, `first_mismatch_sse2`, `first_mismatch_sse2_from`, `scalar_mismatch_from` |
| v4 | Removed runtime `is_x86_feature_detected!("avx2")` dispatch in `first_mismatch` (dead with 16-byte keys since AVX2 path requires `min_len >= 32`) |
| v5 | Replaced `a[idx].cmp(&b[idx])` in `bytes_cmp_simd` with `unsafe { a.get_unchecked(idx).cmp(b.get_unchecked(idx)) }` |
| combined | v1 + v2 + v3 + v4 + v5 together |

## Results

Throughput is mean ops/sec across 5 iterations of each thread group.
Δ is relative to the `simd` baseline.

| Variant | Group A mean | Δ A | Group B mean | Δ B | Load (s) |
|---|---:|---:|---:|---:|---:|
| baseline (`simd`) | 1,308,360 | — | 1,788,799 | — | 92.762 |
| v1 target-cpu=native | 1,298,625 | −0.74% | 1,761,745 | −1.51% | 91.997 |
| v2 16-byte specialization | 1,311,358 | +0.23% | 1,790,198 | +0.08% | 93.349 |
| v3 inline(always) helpers | 1,283,152 | −1.93% | 1,738,312 | −2.82% | 93.067 |
| v4 no runtime AVX2 dispatch | 1,303,425 | −0.38% | 1,776,354 | −0.70% | 95.968 |
| v5 get_unchecked tail | 1,298,490 | −0.75% | 1,749,233 | −2.21% | 92.741 |
| **Combined** (v1+v2+v3+v4+v5) | 1,312,478 | **+0.31%** | 1,798,739 | **+0.56%** | 92.844 |

### Per-iteration data

| Variant | Group A iterations | Group B iterations |
|---|---|---|
| baseline | 1134968, 1267309, 1290259, 1365263, 1484002 | 1542465, 1666558, 1780212, 1936372, 2018389 |
| v1 | 1156480, 1238891, 1266956, 1367225, 1463573 | 1539204, 1631714, 1769270, 1877325, 1991212 |
| v2 | 1111974, 1240370, 1339088, 1415471, 1449885 | 1536640, 1659852, 1795926, 1939487, 2019087 |
| v3 | 1096675, 1228105, 1254526, 1368689, 1467766 | 1482684, 1618845, 1742566, 1884605, 1962860 |
| v4 | 1132303, 1217106, 1314048, 1368363, 1485304 | 1512878, 1617819, 1789485, 1917951, 2043635 |
| v5 | 1144689, 1256822, 1274522, 1343712, 1472703 | 1498443, 1631950, 1770850, 1896201, 1948723 |
| combined | 1152860, 1225615, 1324330, 1387982, 1471604 | 1591210, 1673342, 1798897, 1897941, 2032305 |

## Reading the results

Every variant lands within roughly ±3 % of baseline, and even the
"combined" run is only +0.3 %–0.6 % faster — well below the iteration-to-
iteration jitter on this machine (the per-iteration spread inside a single
group is already ~30 %, dominated by the warm-up curve from iteration 0
to iteration 4). In other words: **none of the five tuning ideas moved the
needle on this workload + this host**.

Why nothing moved:

- **The host has 2 cores and the workload spawns 32 threads.** With
  16× over-subscription on Zen 3 vCPUs, the bottleneck is scheduler
  contention and L3/memory-bandwidth pressure, not the cost of a
  16-byte comparison. Shaving a few cycles off `bytes_cmp` is invisible
  behind preemption latency.
- **The x86 baseline was already SIMD.** `slice::cmp` for `&[u8]` resolves
  to `memcmp`, which on glibc/x86 is SSE/AVX-vectorized with FSRM-fast
  paths on Zen 3. The hand-rolled SSE2 path replaced one SIMD call with
  a different SIMD call — same big-O cycle count.
- **AVX2 dispatch was already dead code** on this benchmark. Inner-node
  and leaf-node compares operate on `16 - prefix_len` bytes, never
  reaching the `min_len >= 32` threshold the AVX2 path requires. v4
  confirmed that removing the runtime check is a no-op (−0.4 %).
- **`#[inline(always)]` slightly hurt** (−2 to −3 %), most likely from
  worse register allocation in the bsearch hot loop or icache pressure
  from the duplicated SSE2 body. Default `#[inline]` was already
  inlining through LTO.
- **The reported 4× ARM win was probably measured on a many-core box.**
  On ARM the stdlib `slice::cmp` path is slower than glibc's NEON-tuned
  `memcmp`, so a hand-rolled NEON helper has more headroom. But you only
  *see* that headroom once you have the cores to remove the scheduling
  bottleneck. To replicate apples-to-apples we need the same workload
  on a ≥16-core x86 host.

## Recommendation

1. **Re-run on a many-core x86 host first** (≥16 physical cores, AVX2-
   capable). Until the scheduler is no longer the bottleneck, no SIMD
   tweak can show a real delta.
2. Once that's done, of the five hypotheses only v2 (16-byte fast-path)
   showed any positive trend; v3 and v5 were neutral-to-negative and
   should be dropped. v4 (drop runtime AVX2 check) is a free
   simplification when key length never exceeds the SSE2 chunk.
3. The longer-term gains likely come from elsewhere: vectorize the
   inner-node bsearch itself (compare a key against multiple separators
   at once), or store keys/separators in a layout that lets one AVX2
   load fetch several candidates.

## Artifacts

- Per-variant logs and JSON in `benchmark/reports/`:
  - `v1-targetcpu-2026-04-20-11-20-11.{log,json}`
  - `v2-16byte-2026-04-20-11-24-34.{log,json}`
  - `v3-inlinealways-2026-04-20-11-29-29.{log,json}`
  - `v4-noruntime-2026-04-20-11-34-37.{log,json}`
  - `v5-unchecked-2026-04-20-11-38-59.{log,json}`
  - `combined-2026-04-20-11-44-12.{log,json}`
- Baseline numbers come from the prior comparison report:
  - `simd-inmemory-2026-04-19-16-23-57.json`
