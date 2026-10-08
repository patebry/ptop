#!/usr/bin/env python3
"""Compare two ptop builds in default mode; emit private-data-free macOS JSONL.

Run after builds and tests finish, on an otherwise quiet machine. Keep every
record, including unfavorable runs. A completed experiment ends with a
completion record; partial output is not a valid completed experiment.
"""
import argparse
import datetime
import json
import os
from pathlib import Path
import platform
import statistics
import subprocess
import sys
import tempfile
import time

import benchmark
from compare_monitors import hardware


TOOLS = ('baseline', 'candidate')
SYSTEM_PATH = '/usr/bin:/bin:/usr/sbin:/sbin'


def parser():
    result = argparse.ArgumentParser(description=__doc__)
    for label in TOOLS:
        result.add_argument('--' + label, required=True, type=Path,
                            help='explicit executable; no installation or build')
    result.add_argument('--seconds', type=benchmark.duration, default=30,
                        help='seconds per paired run (default: 30)')
    result.add_argument('--pairs', type=int, default=4)
    result.add_argument('--intervals', type=int, nargs='+', choices=(300, 1000),
                        default=[300, 1000], help='UI intervals in ms (default: 300 1000)')
    result.add_argument('--cols', type=int, default=100)
    result.add_argument('--rows', type=int, default=30)
    result.add_argument('--soak-seconds', type=float, default=0,
                        help='optional candidate soak at 300 ms; 0 or 60..3600 seconds')
    return result


def schedule(options):
    for interval in options.intervals:
        for pair in range(options.pairs):
            for label in (TOOLS if pair % 2 == 0 else TOOLS[::-1]):
                yield dict(record='run', tool=label, pair=pair,
                           interval_ms=interval, requested_seconds=options.seconds)
    if options.soak_seconds:
        yield dict(record='soak', tool='candidate', interval_ms=300,
                   requested_seconds=options.soak_seconds)


def environment(root):
    env = benchmark.environment(root)
    # The optimized collector deliberately respects a user-overridden ps.
    # Pin system tools here so the experiment measures native default behavior.
    env['PATH'] = SYSTEM_PATH
    for key in ('LINES', 'COLUMNS'):
        env.pop(key, None)
    return env


class TimedResident:
    """Record only elapsed time and own RSS; never retain a PID or command."""
    def __init__(self, reader):
        self.reader = reader
        self.started = time.monotonic()
        self.samples = []

    def __call__(self, pid):
        value = self.reader(pid)
        if value is not None:
            self.samples.append([time.monotonic() - self.started, value])
        return value

    def retention(self, seconds):
        first = [value for elapsed, value in self.samples if 0 <= elapsed < 30]
        last = [value for elapsed, value in self.samples if seconds - 30 <= elapsed <= seconds]
        if len(first) < 2 or len(last) < 2:
            raise benchmark.BenchmarkError('soak lacks RSS coverage in its first or last window')
        first_mean = statistics.mean(first)
        last_mean = statistics.mean(last)
        return dict(window_seconds=30, first_window_samples=len(first),
                    last_window_samples=len(last), first_30s_mean_bytes=first_mean,
                    last_30s_mean_bytes=last_mean, last_minus_first_bytes=last_mean - first_mean,
                    samples_elapsed_seconds_rss_bytes=self.samples)


def validate(options):
    if sys.platform != 'darwin':
        raise benchmark.BenchmarkError('collector measurements currently support macOS only')
    if not 1 <= options.pairs <= 100 or not (40 <= options.cols <= 1000 and 10 <= options.rows <= 1000):
        raise benchmark.BenchmarkError('use 1..100 pairs, 40..1000 columns, and 10..1000 rows')
    if len(set(options.intervals)) != len(options.intervals):
        raise benchmark.BenchmarkError('UI intervals must be unique')
    if options.soak_seconds != 0 and not 60 <= options.soak_seconds <= 3600:
        raise benchmark.BenchmarkError('soak duration must be zero or 60..3600 seconds')
    binaries = {label: getattr(options, label).resolve() for label in TOOLS}
    if any(not path.is_file() or not os.access(path, os.X_OK) for path in binaries.values()):
        raise benchmark.BenchmarkError('both paths must identify executable files')
    if binaries['baseline'] == binaries['candidate']:
        raise benchmark.BenchmarkError('preserve the baseline in a separate executable file')
    return binaries


def metadata(options, binaries):
    with tempfile.TemporaryDirectory(prefix='ptop-collectors-probe-') as directory:
        env = environment(Path(directory))
        tools = {label: dict(sha256=benchmark.digest(path), version=benchmark.version(path, env))
                 for label, path in binaries.items()}
    return dict(record='metadata', schema=1, experiment='ptop-default-collector-comparison',
                utc=datetime.datetime.now(datetime.timezone.utc).isoformat(),
                macos=platform.mac_ver()[0], architecture=platform.machine(), hardware=hardware(),
                python=platform.python_version(), tools=tools,
                harness_sha256=benchmark.digest(Path(__file__)),
                measurement_harness_sha256=benchmark.digest(Path(benchmark.__file__)),
                pairs_per_interval=options.pairs, intervals_ms=options.intervals,
                requested_seconds=options.seconds, soak_seconds=options.soak_seconds,
                soak_interval_ms=300, cols=options.cols, rows=options.rows,
                order='each interval: baseline/candidate on even pairs, candidate/baseline on odd pairs',
                settings='default mode; parallax; mouse disabled; no --vtop-parity or --fix',
                system_tools='PATH=/usr/bin:/bin:/usr/sbin:/sbin; user ps overrides excluded',
                term='xterm-256color', locale='en_US.UTF-8',
                cpu_scope='wait4: monitor plus reaped children; launch and exit included; 100%=one core',
                memory_scope='monitor own RSS after first terminal output; children excluded',
                soak_scope='first/last 30 requested seconds; no warmup exclusion; difference is not proof of a leak',
                completion_required=True)


def measure(options, binaries, info, spec, reader):
    label = spec['tool']
    path = binaries[label]
    expected = info['tools'][label]['sha256']
    if benchmark.digest(path) != expected:
        raise benchmark.BenchmarkError('executable changed before measurement; discard results')
    with tempfile.TemporaryDirectory(prefix='ptop-collectors-run-') as directory:
        env = environment(Path(directory))
        command = [str(path), '--update-interval', str(spec['interval_ms']),
                   '--no-mouse', '--theme', 'parallax']
        resident = TimedResident(reader)
        metrics = benchmark.run(command, env, spec['requested_seconds'],
                                options.cols, options.rows, resident)
    if benchmark.digest(path) != expected:
        raise benchmark.BenchmarkError('executable changed during measurement; discard results')
    if spec['record'] == 'soak':
        metrics['retention'] = resident.retention(spec['requested_seconds'])
    return metrics


def main(argv=None):
    options = parser().parse_args(argv)
    binaries = validate(options)
    info = metadata(options, binaries)
    print(json.dumps(info), flush=True)
    reader = benchmark.ResidentMemory()
    count = 0
    for sequence, spec in enumerate(schedule(options)):
        metrics = measure(options, binaries, info, spec, reader)
        print(json.dumps(dict(spec, sequence=sequence, **metrics)), flush=True)
        count += 1
    print(json.dumps(dict(record='complete', measured_records=count,
                          paired_runs=2 * options.pairs * len(options.intervals),
                          soak_runs=int(bool(options.soak_seconds)))), flush=True)
    return 0


if __name__ == '__main__':
    try:
        sys.exit(main())
    except (benchmark.BenchmarkError, OSError, ValueError, subprocess.SubprocessError):
        # Probe and OS exceptions can contain private executable/configuration paths.
        print('Collector comparison failed; discard partial results. Check executables and environment.',
              file=sys.stderr)
        sys.exit(1)
    except KeyboardInterrupt:
        print('Collector comparison interrupted; discard partial results.', file=sys.stderr)
        sys.exit(130)
