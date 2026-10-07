# Reproducing a local benchmark

Run this script from a source checkout; benchmark scripts are not included in the npm package. The benchmark requires macOS and Python 3.11 or newer. It uses only the Python standard library and macOS `libproc`. Supply executables that are already installed or built; the script does not install packages or build source.

```sh
python3 scripts/benchmark.py \
  --ptop /path/to/ptop \
  --vtop /path/to/vtop \
  --seconds 20 --repetitions 3 --cols 100 --rows 30 \
  > benchmark.jsonl
```

Use the actual executable paths, including a published ptop binary if comparing a release. A vtop launcher must be executable and its Node runtime must be available on `PATH`. Version probes run before measurements. The output records both executables' reported versions and entrypoint SHA-256 hashes, plus the Node version/hash when available. A launcher hash does not pin its complete dependency tree; retain the installed vtop version and lockfile separately. ptop measurements use `--vtop-parity`; the version probe reports ptop's native `--version`.

Run on a quiet machine with no builds, tests, profilers, or other benchmark runs. Each pair uses the same terminal dimensions, 300 ms update interval, `parallax` theme, disabled mouse, locale, and requested duration. Order alternates: vtop/ptop, then ptop/vtop. Repeat enough pairs to assess variation; small differences can be noise.

## What the numbers mean

- **CPU:** `wait4` user and system time for the monitor, including sensor children it reaped, divided by actual elapsed lifetime. Startup and graceful shutdown are included; there is no hidden warmup exclusion. 100% represents one fully occupied CPU core.
- **Memory:** the monitor's own resident memory, sampled approximately every 100 ms after its first terminal output. Mean and maximum RSS exclude sensor children. These are not whole-process-tree memory or macOS physical-footprint measurements.
- **Duration:** `sampled_seconds` covers the requested run before sending `q`; `elapsed_seconds` also includes shutdown. Both are recorded. Early exit, failed RSS sampling, changed executable contents, or failure to quit gracefully makes the command fail.
- **Privacy:** terminal contents are drained and discarded. Results include only the number of terminal bytes, never process names, command lines, terminal captures, hostnames, or executable paths. Unexpected version output is rejected instead of recorded.

The first JSON line describes the environment and method; subsequent lines contain individual runs. Check the script's exit status before accepting a result file: an interrupted or failed experiment can leave partial JSONL output. Compare repeated paired results, and state the machine, versions, duration, dimensions, and variability with any published conclusion. This measures a specific workload; it does not prove visual parity or universal performance superiority.

Only the benchmark's own PTY child and its process group receive cleanup signals. Existing monitors are not touched. The harness creates only a temporary configuration directory.

Harness checks (no live monitor benchmark):

```sh
python3 scripts/benchmark.py --help
python3 -m unittest discover -s scripts -p 'test_benchmark.py'
```
