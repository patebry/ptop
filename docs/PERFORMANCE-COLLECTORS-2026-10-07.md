# macOS collector optimization: ptop 0.1.2 → 0.1.3 candidate

The final build used **67.91% less total CPU at the default 300 ms UI interval** and **70.62% less at 1,000 ms** in this local experiment. Memory collection still dispatches every 200 ms, process collection every 2 seconds, and the vtop-compatible arithmetic, controls, and renderer are unchanged. These measurements describe this Mac, not a guarantee for every machine or Linux.

| UI refresh | 0.1.2 CPU median (range) | 0.1.3 CPU median (range) | Reduction of medians | Median own RSS, old → new |
|---|---|---|---|---|
| 300 ms | 22.55% (21.64–25.33) | 7.24% (6.66–8.03) | 67.91% | 9.19 → 9.25 MiB |
| 1,000 ms | 24.61% (23.42–25.45) | 7.23% (6.96–7.87) | 70.62% | 9.02 → 8.86 MiB |

**100% CPU means one fully occupied core.** CPU includes ptop and the sensor children it reaped, including startup and graceful exit. RSS measures ptop itself, excluding helpers. RSS medians above summarize each 30-second run's mean, not steady-state memory.

The planned **below-7% CPU median target was missed at both intervals**. The separate ≥65% reduction target passed at both. The completion criterion is the measured substantial reduction while preserving polling, complete RSS coverage, and behavior; the missed target is retained here rather than changing the sampling rate or omitting protected processes to reach it. Absolute CPU moved with the live system workload. All final runs and earlier experiments are retained below.

## What changed

The old macOS memory collector ran `ps -caxm -orss,comm` five times per second. That asks `ps` to collect more process information than the memory graph needs. The new collector reads RSS with `PROC_PIDTASKINFO`, truncates each process to KiB as `ps` does, then uses Apple's `ps` only for PIDs denied to the native collector: `ps -x -p <pids> -o pid=,rss=`.

The fallback verifies every requested PID. Missing rows require a fresh census confirming departure; short native reads, malformed output, overflow, unresolved coverage, or command failure trigger the complete original collector. The `-x` option avoids terminal-selection work; adding `-a` would defeat the PID filter. The existing owned-child lifecycle still controls all subprocesses.

Native and Mach basic RSS read the same resident-memory ledger. Access restrictions require retaining the signed system helper for protected processes. A task-name port does not generally bypass the cross-user permission check. See Apple's [RSS implementation](https://github.com/apple-oss-distributions/xnu/blob/main/osfmk/kern/task.c#L5155-L5174) and [task-name authorization](https://github.com/apple-oss-distributions/xnu/blob/xnu-11417.101.15/bsd/kern/kern_proc.c#L5313-L5377).

The exact `ps -ewwwo %cpu,%mem,comm` process collector remains unchanged. Its decayed CPU values, command formatting, grouping, and rounding are part of compatibility. `--vtop-parity` keeps the original memory collector; a custom `ps` selected through PATH does too. Linux collection behavior is unchanged.

Release builds now use Rust's `opt-level=3` instead of `z`. In a separate four-pair, 20-second screening comparison, the speed build used less CPU in all four pairs: medians 7.841% → 7.361%, with individual improvements of 0.100–0.861 percentage points. Its binary was about 176 KiB larger. That screening is separate from the final old/new results above.

## Remaining CPU and memory behavior

A separate 30-second diagnostic measured 8.85% total CPU, about 1.90% in ptop itself and an approximate 6.96% in reaped helpers plus unmatched sampling-boundary CPU—about 79% of the total. This is diagnostic evidence, not a replacement for the paired medians or a theoretical performance floor. It suggests that further large reductions would need another verified protected-process collector or different compatibility requirements.

The diagnostic reads cumulative user/system counters from the same `proc_taskinfo` buffer already used for RSS, converting Mach ticks with `mach_timebase_info`. Its first/last sample window differs slightly from the whole-lifetime `wait4` window; subtraction is therefore approximate. See the [diagnostic record](cpu-decomposition-2026-10-07.json).

Both old and new builds show similar memory warmup over three minutes:

| 180-second run | First 30 s mean | Last 30 s mean | Increase |
|---|---|---|---|
| Old 0.1.2 control | 9.239 MiB | 12.483 MiB | 3.244 MiB |
| Final 0.1.3 candidate | 9.268 MiB | 12.549 MiB | 3.281 MiB |

The difference in growth is only about 0.036 MiB. This supports treating the observed warmup as existing behavior rather than a new collector regression. It does not establish a permanent plateau, prove the allocator cause, or guarantee leak freedom over longer runs. The matched short-run RSS differences also remain within the planned baseline +2 MiB bound.

## Method and validation

- Apple M2 Max, 12 logical CPUs, 96 GiB RAM, macOS 15.4.1, ARM64; Rust 1.92.0, deployment target macOS 15.0.
- Four alternating baseline/candidate pairs at each UI interval; 30 seconds per run, 100×30 PTY, parallax theme, mouse disabled. No excluded warmup.
- Normal ptop behavior, with neither `--vtop-parity` nor `--fix`; system PATH ensures the native collector is selected. Internal sensor cadences are unchanged.
- No concurrent local builds, tests, profilers, or other benchmark runs during measurement. Normal desktop/background activity remained; existing user programs were not stopped.
- Executable hashes checked before/after every run; terminal contents discarded. Reports contain no process names, command lines, hostnames, PIDs, or user paths.
- All 19 final records are present: metadata, 16 paired runs, one 180-second candidate soak, and the completion record. A separate old-build soak used the same measurement and retention primitives.
- Rust tests, fmt/clippy, 91 saved frame comparisons, upstream CLI/key/timer/cadence/error/updater checks, live terminal color replay, terminal restoration, and npm archive/install checks passed. Final source passed macOS and Linux CI.
- The final native proof passed 20 bracketed live aggregate comparisons, actual default/reference/PATH-override checks, and deliberately paused helper cleanup. Snapshots are not simultaneous; the proof reports its explicit tolerance and every gap. Its observer pauses processes, so it is functional evidence, not a performance benchmark.

A finite suite does not establish universal feature or pixel parity. Linux CI is compatibility evidence, not a Linux performance measurement. Earlier htop/btop/vtop comparisons involve different work and are not rerun results for this build.

## Every final paired result

| UI refresh | Pair | 0.1.2 CPU | 0.1.3 CPU |
|---|---|---|---|
| 300 ms | 1 | 22.855% | 7.578% |
| 300 ms | 2 | 25.328% | 8.033% |
| 300 ms | 3 | 22.254% | 6.896% |
| 300 ms | 4 | 21.635% | 6.663% |
| 1,000 ms | 1 | 23.852% | 7.868% |
| 1,000 ms | 2 | 25.448% | 7.124% |
| 1,000 ms | 3 | 25.378% | 7.339% |
| 1,000 ms | 4 | 23.422% | 6.960% |

## Artifacts and reproduction

Baseline source: `024a9cf`; final production source: `e18691f`. The final artifact uses the committed speed profile. The screening artifact was built with an environment override before that profile was committed; its distinct hash is recorded separately.

- Baseline 0.1.2 SHA-256: `c5092ae8aed70e5753885ba2233f6eba40a7419a71ac05b668eb6c77069a48c7`
- Final 0.1.3 SHA-256: `4c7af577effd972652411298498902427d7c67b38539e92fdfbb3347bb72e9c9`
- [Final complete benchmark](benchmark-collectors-2026-10-07.jsonl)
- [Initial hybrid collector with the size profile](benchmark-collectors-initial-2026-10-07.jsonl)
- [Size/speed profile screening](benchmark-opt-level-2026-10-07.jsonl)
- [Old-build memory retention control](benchmark-baseline-retention-2026-10-07.json)
- [Final native collector functional proof](memory-collector-proof-2026-10-07.json)
- [Separate CPU decomposition](cpu-decomposition-2026-10-07.json)

Dates use the author's local October 7 date; machine-readable timestamps are October 8 UTC. Follow [BENCHMARKING.md](BENCHMARKING.md) for accounting details and limitations. Preserve the old executable before rebuilding, then run:

```sh
python3 scripts/benchmark_collectors.py \
  --baseline /path/to/preserved-ptop-0.1.2 \
  --candidate /path/to/ptop-0.1.3 \
  --seconds 30 --pairs 4 --intervals 300 1000 --soak-seconds 180 \
  > collectors.jsonl
```

The old-only retention control uses `benchmark_collectors.measure` with `record='soak'`, `tool='baseline'`, 180 seconds, 300 ms, 100×30 dimensions, the preserved old binary/hash, and `benchmark.ResidentMemory()`. It returns the same first/last-window statistics and numeric RSS series used by the main experiment.
