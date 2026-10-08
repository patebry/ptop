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

## Comparing htop, btop, vtop, and current ptop

The separate four-tool adapter reuses the same PTY, RSS, CPU accounting, and process cleanup. It measures ptop's normal mode, with ptop branding and its default sensor behavior; it does not pass `--vtop-parity` or `--fix`. The older paired experiment above remains available unchanged.

```sh
python3 scripts/compare_monitors.py \
  --ptop /path/to/ptop \
  --vtop /path/to/vtop \
  --htop /path/to/htop \
  --btop /path/to/btop \
  --seconds 30 --repetitions 4 --cols 100 --rows 30 \
  > comparison.jsonl
```

Four repetitions produce 16 sequential runs, about eight minutes. The order rotates from ptop/vtop/htop/btop, so every tool occupies every position once. Use a multiple of four repetitions for balanced positions. A one-second, one-repetition run can check startup and graceful exit, but is not publication-quality measurement. Run final measurements after builds and tests finish, and describe any unavoidable background activity.

All four tools receive a **requested UI refresh of 1,000 ms**, disabled mouse, a 100×30 terminal, and a fresh temporary configuration for every run. This does not equalize their internal collection work. In vtop 0.6.1, CPU and memory plugins poll every 200 ms and the process plugin every 2,000 ms independently of the draw interval. htop receives `--delay 10 --readonly`; vtop and ptop use the `parallax` theme. btop retains its default CPU, memory, network, and process panels, including default disk, temperature, and available GPU collection. Different capabilities and sensor schedules make this a comparison of the selected normal tool configurations, not equivalent work per rendered frame.

Each run gets isolated `XDG_CONFIG_HOME`, `XDG_STATE_HOME`, `XDG_CACHE_HOME`, and an explicit temporary `HTOPRC`. htop starts from a minimal configuration with a one-second delay. btop's own `--default-config` supplies its configuration, changing only `disable_mouse` and `save_config_on_exit`; `--update 1000` supplies its initial refresh. Existing user configuration files are not read or changed. The metadata records these settings, executable versions and SHA-256 hashes, Node version/hash, CPU model and logical count, RAM, macOS, and architecture. A vtop entrypoint hash does not fingerprint all its installed dependencies.

The earlier CPU and own-RSS limitations apply unchanged. There is no excluded warmup. Startup and shutdown remain included. vtop can emit a terminfo diagnostic on this machine; it is discarded with all other terminal output. Thus terminal byte counts include diagnostics and are not a rendering-efficiency score. No process names, terminal contents, hostnames, or local executable/configuration paths enter the JSONL. A failed or interrupted command leaves potentially incomplete output that must not be accepted as a completed experiment.

Validate the adapters and the existing lifecycle/privacy harness before measuring:

```sh
python3 scripts/compare_monitors.py --help
python3 -m unittest discover -s scripts -p 'test_*.py'
```

Publish every successful final run, summarize per-tool medians and ranges, and retain unfavorable results. Small differences can reflect machine activity rather than a repeatable advantage. A macOS comparison is not evidence of Linux performance, and these resource measurements do not prove feature parity or identify a universal winner.

## Comparing ptop collectors before and after an optimization

Preserve the old executable before building the new one. This adapter compares
normal ptop mode at both the default 300 ms UI interval and 1,000 ms. It pins PATH
to macOS system tools because ptop deliberately retains its original collector
when a custom `ps` is selected through PATH.

```sh
python3 scripts/benchmark_collectors.py \
  --baseline /path/to/preserved-ptop \
  --candidate /path/to/new-ptop \
  --seconds 30 --pairs 4 --intervals 300 1000 --soak-seconds 180 \
  > collectors.jsonl
```

The 16 paired runs alternate baseline/candidate and candidate/baseline at each
interval, followed by a three-minute candidate run at 300 ms. Allow about eleven
minutes. Build and test first; keep the measurement period free of other tests,
compilation, profilers, and benchmark runs. No warmup samples are excluded.

Each executable is hash-checked before and after every run. CPU and own-RSS
accounting use the same measurement primitive described above. The longer run
also reports timestamped numeric RSS samples and means for its first and last
30 seconds; a difference alone does not establish a memory leak. Require exit
status zero and a final `complete` record with all expected runs before drawing
conclusions. The adapter does not pass `--vtop-parity`, which intentionally keeps
the original reference collectors.
