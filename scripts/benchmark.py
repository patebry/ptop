#!/usr/bin/env python3
"""Compare explicit ptop/vtop executables in matched macOS PTYs; emit JSONL.

Run on an otherwise quiet machine. CPU includes launch, shutdown, and reaped
sensor children; RSS samples describe only the monitor process after output.
Terminal content is discarded, and executable paths are omitted from results.
"""
import argparse
import ctypes
import datetime
import errno
import fcntl
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import pty
import re
import select
import shutil
import signal
import struct
import subprocess
import sys
import tempfile
import termios
import time


class BenchmarkError(Exception):
    pass


def duration(value):
    seconds = float(value)
    if not math.isfinite(seconds) or not 1 <= seconds <= 3600:
        raise argparse.ArgumentTypeError('seconds must be between 1 and 3600')
    return seconds


def parser():
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument('--ptop', required=True, type=Path, help='explicit ptop executable')
    result.add_argument('--vtop', required=True, type=Path, help='explicit vtop executable')
    result.add_argument('--seconds', type=duration, default=20, help='seconds per run (default: 20)')
    result.add_argument('--repetitions', type=int, default=3, help='matched pairs (default: 3)')
    result.add_argument('--cols', type=int, default=100)
    result.add_argument('--rows', type=int, default=30)
    return result


def digest(path):
    with path.open('rb') as source:
        return hashlib.file_digest(source, 'sha256').hexdigest()


def version(path, env):
    result = subprocess.run([str(path), '--version'], env=env, capture_output=True, timeout=5)
    text = result.stdout.decode('utf-8', errors='replace').strip()
    # Expected version output is short text. Never persist unexpected output
    # that could contain a traceback, executable path, or other private data.
    if result.returncode or not re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9 ._()+-]{0,120}', text):
        raise BenchmarkError('an executable did not return a recognizable --version')
    return text


def environment(config):
    env = {key: value for key, value in os.environ.items()
           if not key.startswith(('PTOP_', 'VTOP_', 'RTOP_', 'HTOP_ORACLE_', 'DYLD_'))}
    env.pop('NODE_OPTIONS', None)
    env.update(TERM='xterm-256color', LC_ALL='en_US.UTF-8', LC_CTYPE='en_US.UTF-8',
               TZ='UTC', XDG_CONFIG_HOME=str(config))
    return env


class ResidentMemory:
    """Darwin proc_taskinfo: resident_size is the second uint64 in 96 bytes."""
    def __init__(self):
        self.library = ctypes.CDLL('/usr/lib/libproc.dylib')
        self.library.proc_pidinfo.argtypes = [ctypes.c_int, ctypes.c_int, ctypes.c_uint64,
                                             ctypes.c_void_p, ctypes.c_int]
        self.library.proc_pidinfo.restype = ctypes.c_int
        self.buffer = ctypes.create_string_buffer(96)

    def __call__(self, pid):
        if self.library.proc_pidinfo(pid, 4, 0, self.buffer, 96) != 96:
            return None
        return struct.unpack_from('=Q', self.buffer.raw, 8)[0]


def child_exited(pid):
    """Observe our child without reaping the process-group ownership anchor."""
    if hasattr(os, 'waitid'):
        return os.waitid(os.P_PID, pid, os.WEXITED | os.WNOHANG | os.WNOWAIT) is not None
    if sys.platform != 'darwin':
        raise BenchmarkError('non-reaping child observation is unavailable')
    # Darwin sys/wait.h: P_PID=1, WEXITED=4, WNOHANG=1, WNOWAIT=32.
    # siginfo_t starts with three ints followed by pid_t; reserve an aligned
    # buffer larger than its 104-byte ABI representation on supported Macs.
    info = (ctypes.c_uint64 * 16)()
    library = ctypes.CDLL(None, use_errno=True)
    waitid = library.waitid
    waitid.argtypes = [ctypes.c_uint, ctypes.c_uint, ctypes.c_void_p, ctypes.c_int]
    waitid.restype = ctypes.c_int
    if waitid(1, pid, info, 4 | 1 | 32) != 0:
        code = ctypes.get_errno()
        raise OSError(code, os.strerror(code))
    return ctypes.c_int.from_buffer(info, 12).value == pid


def darwin_group_has_no_live_members(pgid):
    """Fail closed when checking Darwin's zombie-only killpg EPERM case."""
    library = ctypes.CDLL('/usr/lib/libproc.dylib', use_errno=True)
    library.proc_listpids.argtypes = [ctypes.c_uint, ctypes.c_uint, ctypes.c_void_p, ctypes.c_int]
    library.proc_listpids.restype = ctypes.c_int
    library.proc_pidinfo.argtypes = [ctypes.c_int, ctypes.c_int, ctypes.c_uint64,
                                    ctypes.c_void_p, ctypes.c_int]
    library.proc_pidinfo.restype = ctypes.c_int
    size = library.proc_listpids(2, pgid, None, 0)
    if size <= 0:
        return False
    for _ in range(3):
        members = (ctypes.c_int * (size // 4 + 16))()
        ctypes.set_errno(0)
        used = library.proc_listpids(2, pgid, members, ctypes.sizeof(members))
        if used < 0 or (used == 0 and ctypes.get_errno()):
            return False
        if used >= ctypes.sizeof(members):
            size = used * 2
            continue
        for member in list(members)[:used // 4]:
            info = ctypes.create_string_buffer(136)  # Darwin proc_bsdinfo
            ctypes.set_errno(0)
            count = library.proc_pidinfo(member, 3, 0, info, len(info))
            if count == 0 and ctypes.get_errno() == errno.ESRCH:
                continue
            if count != len(info) or struct.unpack_from('=I', info, 4)[0] != 5:  # SZOMB
                return False
        return True
    return False


def signal_owned_group(pid, sig):
    try:
        os.killpg(pid, sig)
    except ProcessLookupError:
        pass
    except PermissionError:
        if not (sys.platform == 'darwin' and darwin_group_has_no_live_members(pid)):
            raise


def run(command, env, seconds, cols, rows, resident):
    """One owned session; no terminal contents survive a read call."""
    started = time.monotonic()
    pid, fd = pty.fork()
    if pid == 0:
        try:
            fcntl.ioctl(0, termios.TIOCSWINSZ, struct.pack('HHHH', rows, cols, 0, 0))
            os.execve(command[0], command, env)
        except BaseException:
            os.write(2, b'benchmark: executable could not start\n')
            os._exit(127)
    usage = None
    status = None
    output_bytes = 0
    rss = []

    def drain(timeout):
        nonlocal output_bytes
        if select.select([fd], [], [], max(0, timeout))[0]:
            try:
                output_bytes += len(os.read(fd, 65536))
            except OSError as error:
                if error.errno != errno.EIO:
                    raise
                # PTY EOF is commonly reported as EIO.

    try:
        fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack('HHHH', rows, cols, 0, 0))
        deadline = started + seconds
        next_sample = started
        while time.monotonic() < deadline:
            drain(min(.05, deadline - time.monotonic()))
            if child_exited(pid):
                raise BenchmarkError('monitor exited before the requested duration')
            now = time.monotonic()
            if output_bytes and now >= next_sample:
                value = resident(pid)
                if value is not None:
                    rss.append(value)
                next_sample = now + .1
        measured = time.monotonic() - started
        os.write(fd, b'q')
        deadline = time.monotonic() + 3
        while not child_exited(pid) and time.monotonic() < deadline:
            drain(.05)  # Keep draining while the child flushes its final frame.
        if not child_exited(pid):
            raise BenchmarkError('monitor did not quit gracefully')
        elapsed = time.monotonic() - started
    finally:
        # Keep the leader unreaped until ALL group signals have been sent.
        # Even an already-exited leader can leave a TERM-ignoring descendant;
        # its zombie reserves the PID/PGID, preventing signals to a reused ID.
        try:
            os.close(fd)
            signal_owned_group(pid, signal.SIGTERM)
            time.sleep(.1)
            signal_owned_group(pid, signal.SIGKILL)
        finally:
            _, status, usage = os.wait4(pid, 0)
    if status != 0 or not rss:
        raise BenchmarkError('monitor failed or own-process RSS could not be sampled')
    cpu = usage.ru_utime + usage.ru_stime
    return dict(elapsed_seconds=elapsed, sampled_seconds=measured,
                cpu_user_seconds=usage.ru_utime, cpu_system_seconds=usage.ru_stime,
                cpu_percent=100 * cpu / elapsed, rss_samples=len(rss),
                own_rss_mean_bytes=sum(rss) / len(rss), own_rss_max_bytes=max(rss),
                terminal_bytes=output_bytes, exit_code=os.waitstatus_to_exitcode(status))


def orders(repetitions):
    for repetition in range(repetitions):
        for label in (('vtop', 'ptop') if repetition % 2 == 0 else ('ptop', 'vtop')):
            yield repetition, label


def main(argv=None):
    options = parser().parse_args(argv)
    if sys.platform != 'darwin':
        raise BenchmarkError('this benchmark currently supports macOS only')
    if not 1 <= options.repetitions <= 100 or not (40 <= options.cols <= 1000 and 10 <= options.rows <= 1000):
        raise BenchmarkError('use 1..100 repetitions, 40..1000 columns, and 10..1000 rows')
    binaries = {label: getattr(options, label).resolve() for label in ('ptop', 'vtop')}
    if any(not path.is_file() or not os.access(path, os.X_OK) for path in binaries.values()):
        raise BenchmarkError('both executable paths must identify executable files')
    with tempfile.TemporaryDirectory(prefix='ptop-benchmark-') as config:
        env = environment(config)
        info = {label: dict(version=version(path, env), sha256=digest(path))
                for label, path in binaries.items()}
        node = shutil.which('node', path=env.get('PATH'))
        node_info = dict(version=version(Path(node), env), sha256=digest(Path(node))) if node else None
        metadata = dict(record='metadata', schema=1, utc=datetime.datetime.now(datetime.timezone.utc).isoformat(),
                        macos=platform.mac_ver()[0], architecture=platform.machine(),
                        python=platform.python_version(), tools=info, node=node_info,
                        repetitions=options.repetitions, requested_seconds=options.seconds,
                        cols=options.cols, rows=options.rows, interval_ms=300, theme='parallax',
                        term='xterm-256color', locale='en_US.UTF-8', mouse=False,
                        cpu_scope='wait4: monitor plus reaped children; launch and exit included',
                        memory_scope='clock-sampled monitor RSS after first terminal output; children excluded')
        print(json.dumps(metadata), flush=True)
        resident = ResidentMemory()
        for repetition, label in orders(options.repetitions):
            command = [str(binaries[label]), '--update-interval', '300', '--no-mouse', '--theme', 'parallax']
            if label == 'ptop':
                command.append('--vtop-parity')
            metrics = run(command, env, options.seconds, options.cols, options.rows, resident)
            if digest(binaries[label]) != info[label]['sha256']:
                raise BenchmarkError('executable changed during measurement; discard results')
            print(json.dumps(dict(record='run', tool=label, repetition=repetition, **metrics)), flush=True)
    return 0


if __name__ == '__main__':
    try:
        sys.exit(main())
    except (BenchmarkError, OSError, subprocess.SubprocessError) as error:
        message = str(error) if isinstance(error, BenchmarkError) else type(error).__name__
        print('benchmark: ' + message, file=sys.stderr)
        sys.exit(1)
    except KeyboardInterrupt:
        print('benchmark: interrupted', file=sys.stderr)
        sys.exit(130)
