# htop, btop, vtop and ptop: local measurements, 2026-10-07

This compares the overhead of four installed terminal monitors on one working Mac. **ptop used the least own-process resident memory; htop used the least CPU.** Both htop and btop used substantially less CPU than ptop. The tools expose different information and retain different internal sensor schedules, so this is not equivalent work or a universal ranking.

I maintain ptop. The complete final experiment is published, including results that favor the other tools. The [article explains which monitor to choose](https://www.patebryant.com/articles/htop-vs-btop-vtop-ptop); the [ptop introduction](https://www.patebryant.com/articles/ptop-rust-vtop) explains the vtop compatibility goal.

## Results

Four 30-second runs per tool. Each row shows the median and minimum–maximum of the four run values. Memory summarizes each run's mean own-process RSS. MiB means 1,048,576 bytes. CPU is percent of one core (100% = one fully occupied core), not total machine capacity.

| Tool | Median CPU | CPU range | Median own RSS | Own RSS range |
| --- | ---: | ---: | ---: | ---: |
| htop 3.2.2 | 0.31% | 0.31–0.33% | 9.97 MiB | 9.68–11.03 MiB |
| btop 1.4.7 | 2.37% | 2.34–2.42% | 19.18 MiB | 18.95–20.78 MiB |
| vtop 0.6.1 | 24.68% | 24.45–24.96% | 112.22 MiB | 110.82–113.79 MiB |
| ptop 0.1.2 | 20.70% | 20.10–21.17% | 7.50 MiB | 6.74–8.36 MiB |

Comparing those medians, ptop used **16.12% less CPU** and **93.32% less own-process RSS** than vtop. These are ratios of per-tool medians, not averages of paired percentage differences. ptop's lower memory does not make it the lowest-overhead choice on every metric: htop and btop used much less CPU.

## Environment and settings

- Date: October 7, 2026 in America/New_York; the raw UTC start time is `2026-10-08T01:12:08.484521+00:00`.
- Apple M2 Max, 12 logical CPUs, 96 GiB RAM; macOS 15.4.1, ARM64.
- Python 3.14.4; Node v24.20.0 for vtop.
- 100 × 30 pseudo-terminal, `xterm-256color`, `en_US.UTF-8`, UTC inside the child processes, truecolor environment.
- Common **requested UI refresh of 1,000 ms**; native internal collection schedules remain unchanged. In vtop 0.6.1, CPU/memory plugins poll every 200 ms and processes every 2,000 ms.
- ptop 0.1.2 uses normal ptop branding and default compatibility arithmetic: neither `--vtop-parity` nor `--fix`. The tested executable matches the local npm 0.1.2 release archive byte for byte.
- htop 3.2.2 and btop 1.4.7 are the installed versions tested, not a claim that they are the newest releases.
- Monitors run without elevation. No builds, tests, or profilers ran as part of this task during final measurement. The user's existing applications and background work continued; this was not a controlled idle laboratory.
- Four repetitions rotate `ptop, vtop, htop, btop` by one starting position each round, placing every tool in every position once. One monitor runs at a time.

Exact tool arguments (executable/configuration paths are represented by placeholders):

```text
ptop --update-interval 1000 --no-mouse --theme parallax
vtop --update-interval 1000 --no-mouse --theme parallax
htop --delay 10 --no-mouse --readonly
btop --update 1000 --config <fresh-temporary-btop.conf>
```

Each individual run has fresh `XDG_CONFIG_HOME`, `XDG_STATE_HOME`, `XDG_CACHE_HOME`, and an explicit temporary `HTOPRC`. The htop configuration contains:

```ini
htop_version=3.2.2
config_reader_min_version=3
delay=10
```

btop's own `--default-config` supplies all settings, with only `disable_mouse = true` and `save_config_on_exit = false` changed. The CLI requests the one-second update. CPU, memory, network and process panels remain; disk, temperature and GPU settings retain their defaults, with actual availability depending on the build/platform. This does not claim that GPU data was collected on this Mac. User configuration files are not changed.

## What is measured

**CPU:** `wait4` user plus system CPU for the monitor and children it reaped, divided by actual elapsed lifetime. Startup and graceful shutdown are included. There is no excluded warmup. This is not an unconditional whole-process-tree CPU measurement.

**Memory:** own-process resident set size, sampled about every 100 ms after the first terminal output. Scheduling makes the sampling interval approximate. Child memory is excluded; this is not macOS physical footprint or total process-tree memory. Each run's samples produce its mean RSS, and the summary reports the median/range of those four means.

The harness drains and discards terminal output. vtop can emit a terminfo diagnostic on this host; it is discarded too. Terminal byte counts include any diagnostics and are not a rendering-efficiency score. An initial smoke test only verified execution; its one-second samples are excluded from this final dataset.

Short runs on one active Mac cannot establish long-running memory behavior, Linux performance, or universal rankings. The live process list, background work, privileges, terminal geometry and intervals all affect results. These measurements do not prove visual or feature parity. The older 300 ms ptop 0.1.0 compatibility-mode experiment is separate and should not be combined with this one.

## Reproduce

The [harness at the measured revision](https://github.com/patebry/ptop/blob/63d0403/scripts/compare_monitors.py) requires macOS and Python 3.11+. Install the monitors separately; the harness does not install or build anything. For the npm tools used here:

```sh
npm install -g @patebryant/ptop@0.1.2 vtop@0.6.1
```

Install htop and btop using their upstream/package-manager instructions. To match these executables, use htop 3.2.2 and btop 1.4.7; other versions form a new comparison and are recorded by the script. Clone the harness revision, then run:

```sh
git clone https://github.com/patebry/ptop.git ptop-benchmark
cd ptop-benchmark
git checkout 63d0403
python3 scripts/compare_monitors.py \
  --ptop "$(command -v ptop)" \
  --vtop "$(command -v vtop)" \
  --htop "$(command -v htop)" \
  --btop "$(command -v btop)" \
  --seconds 30 --repetitions 4 --cols 100 --rows 30 \
  > comparison.jsonl
```

Check the command's exit status. Failure or interruption can leave partial JSONL and invalidates the experiment. Use explicit executable paths if your shell resolves a different release. The repository does not ship `bin/ptop` in Git; npm supplies the binary used here.

The metadata includes executable entrypoint, Node, harness and configuration hashes. A vtop launcher hash does not identify its entire dependency tree; retain its installed dependency lockfile for an exact reproduction. Version/hash matching alone cannot recreate the original live machine workload.

## Every final run

[Raw JSONL](benchmark-four-tools-2026-10-07.jsonl) is the source of truth. Sequence and repetition are zero-based. The durations below exclude the subsequent quit request; the JSONL also records elapsed lifetime including graceful shutdown.

| Sequence | Repetition | Tool | Sampled seconds | CPU % | Mean own RSS MiB | RSS samples |
| ---: | ---: | --- | ---: | ---: | ---: | ---: |
| 0 | 0 | ptop | 30.003 | 21.1695 | 8.3557 | 270 |
| 1 | 0 | vtop | 30.002 | 24.9583 | 113.4455 | 269 |
| 2 | 0 | htop | 30.004 | 0.3320 | 11.0324 | 269 |
| 3 | 0 | btop | 30.005 | 2.4199 | 20.7824 | 269 |
| 4 | 1 | vtop | 30.002 | 24.4506 | 110.8242 | 267 |
| 5 | 1 | htop | 30.004 | 0.3123 | 9.6797 | 270 |
| 6 | 1 | btop | 30.002 | 2.3572 | 18.9452 | 269 |
| 7 | 1 | ptop | 30.002 | 20.0953 | 7.4785 | 271 |
| 8 | 2 | htop | 30.001 | 0.3086 | 10.0250 | 269 |
| 9 | 2 | btop | 30.001 | 2.3412 | 19.0968 | 269 |
| 10 | 2 | ptop | 30.003 | 20.9679 | 6.7360 | 271 |
| 11 | 2 | vtop | 30.004 | 24.5181 | 110.9882 | 268 |
| 12 | 3 | btop | 30.005 | 2.3791 | 19.2609 | 269 |
| 13 | 3 | ptop | 30.002 | 20.4290 | 7.5123 | 270 |
| 14 | 3 | vtop | 30.004 | 24.8343 | 113.7894 | 258 |
| 15 | 3 | htop | 30.005 | 0.3130 | 9.9076 | 271 |

All 16 runs completed with exit code 0, positive terminal output, and valid RSS samples. An independent recomputation verified the CPU arithmetic, summaries, complete order and harness/configuration hashes.

Validation before measurement: `python3 -m unittest discover -s scripts -p 'test_*.py'` passed all 14 tests; a separate four-tool one-second smoke completed. Only the benchmark's owned process groups receive cleanup signals.
