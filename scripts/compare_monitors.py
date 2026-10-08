#!/usr/bin/env python3
"""Compare four installed monitors in isolated macOS PTYs; emit private-data-free JSONL."""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys
import tempfile

import benchmark


TOOLS = ('ptop', 'vtop', 'htop', 'btop')
INTERVAL_MS = 1000
HTOP_CONFIG = 'htop_version=3.2.2\nconfig_reader_min_version=3\ndelay=10\n'


def parser():
    result = argparse.ArgumentParser(description=__doc__)
    for label in TOOLS:
        result.add_argument('--' + label, required=True, type=Path,
                            help='explicit installed executable; no installation or build')
    result.add_argument('--seconds', type=benchmark.duration, default=30)
    result.add_argument('--repetitions', type=int, default=4)
    result.add_argument('--cols', type=int, default=100)
    result.add_argument('--rows', type=int, default=30)
    return result


def orders(repetitions):
    """Each block of four repetitions puts every monitor in every position."""
    for repetition in range(repetitions):
        offset = repetition % len(TOOLS)
        for label in TOOLS[offset:] + TOOLS[:offset]:
            yield repetition, label


def tool_version(label, path, env):
    if label != 'btop':
        return benchmark.version(path, env)
    result = subprocess.run([str(path), '-V'], env=env, capture_output=True, timeout=5)
    output = re.sub(rb'\x1b\[[0-9;]*m', b'', result.stdout).strip()
    if result.returncode or not re.fullmatch(rb'btop version: [0-9]+\.[0-9]+\.[0-9]+', output):
        raise benchmark.BenchmarkError('btop did not return a recognizable -V version')
    return output.decode('ascii')


def isolated_environment(root):
    env = benchmark.environment(root / 'config')
    for key, directory in (('XDG_CONFIG_HOME', 'config'), ('XDG_STATE_HOME', 'state'),
                           ('XDG_CACHE_HOME', 'cache')):
        path = root / directory
        path.mkdir()
        env[key] = str(path)
    env['HTOPRC'] = str(root / 'htoprc')
    env['COLORTERM'] = 'truecolor'
    for key in ('LINES', 'COLUMNS'):
        env.pop(key, None)
    # Existing explicit config prevents loading user or system htop settings.
    (root / 'htoprc').write_text(HTOP_CONFIG)
    return env


def btop_defaults(path, env):
    result = subprocess.run([str(path), '--default-config'], env=env,
                            capture_output=True, timeout=5)
    if result.returncode:
        raise benchmark.BenchmarkError('btop could not produce its default configuration')
    text = result.stdout.decode('utf-8', errors='strict')
    for name, value in (('disable_mouse', 'true'), ('save_config_on_exit', 'false')):
        text, count = re.subn(r'^' + name + r' = (?:true|false)$', name + ' = ' + value,
                             text, flags=re.MULTILINE)
        if count != 1:
            raise benchmark.BenchmarkError('btop default configuration has an unsupported format')
    return text


def command(label, executable, root, config):
    args = [str(executable)]
    if label in ('ptop', 'vtop'):
        return args + ['--update-interval', str(INTERVAL_MS), '--no-mouse', '--theme', 'parallax']
    if label == 'htop':
        return args + ['--delay', '10', '--no-mouse', '--readonly']
    path = root / 'btop.conf'
    path.write_text(config)
    return args + ['--update', str(INTERVAL_MS), '--config', str(path)]


def hardware():
    """Only allow safe model text and aggregate counts, never host identity."""
    values = {}
    for key, name in (('machdep.cpu.brand_string', 'cpu_model'), ('hw.memsize', 'memory_bytes')):
        result = subprocess.run(['/usr/sbin/sysctl', '-n', key], capture_output=True, timeout=5)
        text = result.stdout.decode('ascii', errors='replace').strip()
        if result.returncode or not re.fullmatch(r'[A-Za-z0-9 .()+-]{1,100}', text):
            raise benchmark.BenchmarkError('could not read safe machine metadata')
        values[name] = int(text) if name == 'memory_bytes' else text
    return dict(logical_cpu_count=os.cpu_count(), **values)


def metadata(options, binaries):
    with tempfile.TemporaryDirectory(prefix='ptop-compare-probe-') as directory:
        env = isolated_environment(Path(directory))
        tools = {label: dict(version=tool_version(label, path, env), sha256=benchmark.digest(path))
                 for label, path in binaries.items()}
        node = shutil.which('node', path=env.get('PATH'))
        if not node:
            raise benchmark.BenchmarkError('vtop requires Node on PATH')
        node_info = dict(version=benchmark.version(Path(node), env), sha256=benchmark.digest(Path(node)))
        config = btop_defaults(binaries['btop'], env)
    info = dict(record='metadata', schema=1, experiment='four-monitor-native-defaults',
                utc=datetime.datetime.now(datetime.timezone.utc).isoformat(),
                macos=platform.mac_ver()[0], architecture=platform.machine(), hardware=hardware(),
                python=platform.python_version(), tools=tools, node=node_info,
                harness_sha256=benchmark.digest(Path(__file__)),
                measurement_harness_sha256=benchmark.digest(Path(benchmark.__file__)),
                configuration_sha256=dict(htop=hashlib.sha256(HTOP_CONFIG.encode()).hexdigest(),
                                          btop=hashlib.sha256(config.encode()).hexdigest()),
                repetitions=options.repetitions, requested_seconds=options.seconds,
                cols=options.cols, rows=options.rows, requested_ui_interval_ms=INTERVAL_MS,
                order='rotate ptop,vtop,htop,btop by one position per repetition',
                term='xterm-256color', locale='en_US.UTF-8', mouse=False,
                settings=dict(ptop='default mode; parallax; no --vtop-parity or --fix',
                              vtop='parallax; internal CPU/memory 200 ms, process 2000 ms',
                              htop='fresh minimal config; delay=10 tenths; readonly',
                              btop='fresh --default-config; mouse/save disabled; all other defaults',
                              btop_panels='cpu mem net proc; default disk/temperature/GPU collectors retained'),
                config_scope='fresh temporary config/state/cache and HTOPRC for each run',
                cpu_scope='wait4: monitor plus reaped children; launch and exit included; 100%=one core',
                memory_scope='clock-sampled monitor RSS after first terminal output; children excluded',
                timing_scope='common requested UI interval, native internal polling schedules retained')
    return info, config


def main(argv=None):
    options = parser().parse_args(argv)
    if sys.platform != 'darwin':
        raise benchmark.BenchmarkError('this comparison currently supports macOS only')
    if not 1 <= options.repetitions <= 100 or not (40 <= options.cols <= 1000 and 10 <= options.rows <= 1000):
        raise benchmark.BenchmarkError('use 1..100 repetitions, 40..1000 columns, and 10..1000 rows')
    binaries = {label: getattr(options, label).resolve() for label in TOOLS}
    if any(not path.is_file() or not os.access(path, os.X_OK) for path in binaries.values()):
        raise benchmark.BenchmarkError('all four paths must identify executable files')
    info, config = metadata(options, binaries)
    print(json.dumps(info), flush=True)
    resident = benchmark.ResidentMemory()
    for sequence, (repetition, label) in enumerate(orders(options.repetitions)):
        if benchmark.digest(binaries[label]) != info['tools'][label]['sha256']:
            raise benchmark.BenchmarkError('executable changed before measurement; discard results')
        with tempfile.TemporaryDirectory(prefix='ptop-compare-run-') as directory:
            root = Path(directory)
            env = isolated_environment(root)
            args = command(label, binaries[label], root, config)
            metrics = benchmark.run(args, env, options.seconds, options.cols, options.rows, resident)
        if benchmark.digest(binaries[label]) != info['tools'][label]['sha256']:
            raise benchmark.BenchmarkError('executable changed during measurement; discard results')
        print(json.dumps(dict(record='run', tool=label, repetition=repetition,
                              sequence=sequence, **metrics)), flush=True)
    return 0


if __name__ == '__main__':
    try:
        sys.exit(main())
    except (benchmark.BenchmarkError, OSError, ValueError, subprocess.SubprocessError):
        # Unexpected OS/probe errors can contain private executable/config paths.
        print('Comparison failed; discard partial results. Check executables and environment.', file=sys.stderr)
        sys.exit(1)
