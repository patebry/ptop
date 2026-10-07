# Published npm 0.1.0 comparison, 2026-10-06

Three alternating 20-second pairs on Apple M2 Max (12 logical CPUs, 96 GiB),
macOS 15.4.1. Both ran in a 100×30 PTY with parallax theme, 300 ms updates,
UTF-8 and mouse disabled. ptop used `--vtop-parity` for this comparison.
The ptop binary came from the published, verified npm 0.1.0 archive; vtop was
0.6.1 on Node 24.20.0. No builds, tests or profilers ran concurrently. Existing
monitors and ordinary background applications were left running.

| Tool | Mean CPU (% of one core) | Run CPU range | Mean own RSS (MiB) | Run mean RSS range (MiB) |
|---|---:|---:|---:|---:|
| vtop 0.6.1 | 27.20 | 26.75–27.73 | 111.18 | 110.56–111.88 |
| ptop 0.1.0 | 23.02 | 22.61–23.63 | 7.31 | 7.17–7.57 |

These six short runs support only a local comparison. They are not confidence
intervals, a controlled idle-host study, or proof of universal improvement.
CPU includes the monitor and children it reaped; sampled RSS excludes children.
Startup and shutdown are included in CPU lifetime. The benchmark drains and
counts terminal bytes but retains no terminal contents or process inventory.

[Raw JSONL](benchmark-npm-0.1.0-2026-10-06.jsonl) contains every run, versions,
binary/runtime hashes, durations, and sampling counts. The vtop entrypoint hash
does not pin its entire dependency tree. These results are for the published
binary, not a relabeled measurement of subsequent source changes.

Reproduce from a source checkout using [BENCHMARKING.md](BENCHMARKING.md).
