# Matched vtop/ptop measurements, 2026-10-06

Three 20-second runs per tool, alternating order, on Apple M2 Max (12 CPUs,
96 GiB), macOS 15.4.1. Both use a 100×30 xterm-256color PTY, UTF-8, 300 ms
render interval and disabled mouse. Ptop uses predecessor-compatible defaults.
No builds, profiles or acceptance tests ran concurrently. Every run exited on q.

| Tool | Mean CPU (% of one core) | Mean own-process RSS (MiB) | Mean output bytes |
|---|---:|---:|---:|
| vtop 0.6.1 | 27.568 | 112.20 | 49,559 |
| ptop | 23.735 | 7.54 | 39,373 |

Observed means: **13.9% less CPU**,
**93.3% less own-process RSS**,
and **20.6% fewer terminal bytes**.
These short measurements describe this workload, not universal speedups.

CPU uses wait4 user+system time, including reaped sensor children, divided by
observed wall time. RSS is sampled with libproc after exec at approximately
100 ms intervals; it excludes simultaneous child RSS. The separate wait4
high-water field may include pre-exec memory and is not the table's RSS metric.
Ptop retains the original ps sensor, its 200 ms/2 s deadlines and overlapping
poll behavior; a native RSS replacement was rejected because it lacked ps's
process-port permission. Less sensor work is not the explanation for this result.

[Raw measurements](benchmark-final-2026-10-06.jsonl) preserve commands, executable
hashes, durations and exit status. Absolute executable and terminal-capture
paths have been replaced with portable labels for sharing. Numerical results
and executable hashes are unchanged; the JSONL itself is a sanitized derivative,
not the original evidence file. The original temporary terminal captures are
not distributed. The six runs came from an earlier four-tool batch.
The vtop hash identifies its launcher, not all installed Node dependencies.
Ptop release: `fc7dc104c16e0526629b172473da21a58eff7beacc374da29a887b5a6bab7722`.

For new measurements, use the self-contained [benchmark guide](BENCHMARKING.md).

## Subsequent live-rendering correction

The measurements above predate the live SGR-state repair prompted by a user
screenshot. They remain measurements of the explicitly recorded executable,
not fresh benchmarks of the repaired build. Frame-sheet equivalence did not
exercise persistent terminal color state between rows and updates.
