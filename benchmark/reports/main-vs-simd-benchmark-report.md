# Benchmark Comparison Report: `main` vs `simd`

## Summary
- Report generated at: `2026-04-19 16:13:39 UTC`
- Command used: `env SHUMAI_FILTER="inmemory" MIMALLOC_LARGE_OS_PAGES=1 cargo run --bin bftree --release`
- Config: in-memory benchmark preset (`bftreebench-inmemory`)
- Machine: `kazik` (kernel `6.8.0-106-generic`, 2 CPUs / 2 physical cores, 16,369,008,640 bytes RAM)

## Branches
- `main` commit: `404db61382e35efb60c243403aa0fe5a83748046`
- `simd` commit: `9aacdde54de607a2a6fc7257564f121aeb5043d1`

## Parsed Results

### Throughput (ops/sec, 5 iterations each)

| Run (32 threads) | main | simd | min | max |
|---|---:|---:|---:|---:|
| Group A (5 iterations) | 1,291,575.4 | 1,393,280.4 | 1,246,899 / 1,240,899 | 1,512,242 / 1,548,123 |
| Group B (5 iterations) | 1,724,606.0 | 1,940,079.6 | 1,479,569 / 1,652,449 | 2,042,687 / 2,196,698 |

**Note:** the min/max columns show `main/simd` for each group.

### Load time
- `main`: 87.535 s
- `simd`: 85.948 s
- Change: simd is ~1.8% faster in load phase.

### Overall delta (across both groups)
- Group A throughput delta: **+7.87%**
- Group B throughput delta: **+12.49%**
- Overall mean throughput delta: **+10.52%**

### Raw throughput lists (iterations)

#### Group A (32 threads)
- `main`: `1096388, 1217828, 1319967, 1311452, 1512242`
- `simd`: `1240899, 1311710, 1361911, 1503759, 1548123`

#### Group B (32 threads)
- `main`: `1479569, 1586325, 1634706, 1879743, 2042687`
- `simd`: `1652449, 1771147, 1949564, 2130540, 2196698`

## Artifacts
- `reports/main-2026-04-19-16-06-38.log`
- `reports/simd-2026-04-19-16-09-57.log`
- `reports/main-inmemory-correct-2026-04-19-16-06-38.json`
- `reports/simd-inmemory-correct-2026-04-19-16-09-57.json`
