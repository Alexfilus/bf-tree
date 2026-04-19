# Benchmark Comparison Report: `main` vs `simd`

## Summary
- Report generated at: `2026-04-19 16:28 UTC`
- Command used: `env SHUMAI_FILTER="inmemory" MIMALLOC_LARGE_OS_PAGES=1 cargo run --bin bftree --release`
- Config: in-memory benchmark preset (`bftreebench-inmemory`) from `benchmark/bench_bftree.toml`
  - 100,000,000 records, 16 KiB key length, uniform read-only workload (read=100%)
  - 16,384 MB memory budget, in-memory storage backend (`:memory:`)
  - 2 thread groups of 32 threads, 5 iterations each, 10 s per iteration
- Machine: `kazik` (kernel `6.8.0-106-generic`, 2 CPUs / 2 physical cores, 16,369,008,640 bytes RAM)
- Build patch applied to both branches: enable an empty `spdk` feature on the `bf-tree` and `benchmark` crates so the benchmark binary builds without the optional SPDK dependency (mirrors `fix-benchmark-spdk-feature`).

## Branches
- `main` commit: `404db61382e35efb60c243403aa0fe5a83748046`
- `simd` commit: `9aacdde54de607a2a6fc7257564f121aeb5043d1`

## Parsed Results

### Throughput (ops/sec, 5 iterations each)

| Run (32 threads) | main mean | simd mean | main min / max | simd min / max | Δ mean |
|---|---:|---:|---:|---:|---:|
| Group A | 1,302,579.8 | 1,308,360.2 | 1,095,795 / 1,493,919 | 1,134,968 / 1,484,002 | **+0.44%** |
| Group B | 1,766,335.6 | 1,788,799.2 | 1,532,515 / 1,990,828 | 1,542,465 / 2,018,389 | **+1.27%** |

### Load time
- `main`: 92.911 s
- `simd`: 92.762 s
- Change: simd is ~0.16% faster in the load phase (effectively a tie).

### Overall delta (across both groups)
- Group A throughput delta: **+0.44%**
- Group B throughput delta: **+1.27%**
- Overall mean throughput delta: **+0.86%**

### Raw throughput lists (iterations)

#### Group A (32 threads)
- `main`: `1095795, 1237032, 1329307, 1356846, 1493919`
- `simd`: `1134968, 1267309, 1290259, 1365263, 1484002`

#### Group B (32 threads)
- `main`: `1532515, 1610445, 1763967, 1933923, 1990828`
- `simd`: `1542465, 1666558, 1780212, 1936372, 2018389`

## Observations
- The SIMD branch is consistently a hair faster on this workload, but the gap is small (~1% on average) and well within the variance between iterations on this 2-core box.
- Both branches show the typical warm-up curve: throughput climbs by ~30–35% between iteration 0 and iteration 4 in each group, presumably as the hot working set settles into MiMalloc / page tables.
- Group B (the second 32-thread run, which reuses the loaded tree) is noticeably faster than Group A on both branches — a fully-warm cache effect rather than a code change.
- This benchmark is read-only against an in-memory `:memory:` backend, so the SIMD scan paths in inner-node search are exercised but no I/O paths are. Workloads that hit storage or do range scans may see a different SIMD impact and are not measured here.
- Hardware caveat: the host has only 2 cores while the benchmark spawns 32 worker threads, so results reflect oversubscribed scheduling rather than peak many-core throughput.

## Artifacts
- `reports/main-inmemory-2026-04-19-16-19-38.json`
- `reports/simd-inmemory-2026-04-19-16-23-57.json`
- Previous run (kept for reference):
  - `reports/main-inmemory-correct-2026-04-19-16-06-38.json`
  - `reports/simd-inmemory-correct-2026-04-19-16-09-57.json`
