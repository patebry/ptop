#!/usr/bin/env python3
"""macOS hybrid RSS checks; reports aggregates, never process names or commands.

Build ptop first. The scratch driver includes the production native module.
Live totals are bracketed by legacy scans, not claimed to be simultaneous.
The PTY observer sees only this harness's ptop children; short child lifetimes
make its launch counts a lower bound. cadence-proof.py remains the precise
scheduler test with controlled slow sensors.
"""
import argparse
import ctypes
from contextlib import contextmanager
import errno
import fcntl
import hashlib
import json
import os
import pty
import select
import signal
import statistics
import struct
import subprocess
import sys
import tempfile
import termios
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SYSTEM_PATH = '/usr/bin:/bin:/usr/sbin:/sbin'
sys.path.insert(0, str(ROOT / 'scripts'))
from benchmark import child_exited


def compile_driver(directory, binary):
    candidates = list((binary.parent / 'deps').glob('liblibc-*.rlib'))
    candidates += list((ROOT / 'target').rglob('liblibc-*.rlib'))
    if not candidates:
        raise RuntimeError('Build ptop with cargo before running this proof')
    library = max(candidates, key=lambda path: path.stat().st_mtime_ns)
    source = directory / 'memory_probe.rs'
    source.write_text('''#[path = %s] mod macos_memory;
fn main() -> std::io::Result<()> {
    let started = std::time::Instant::now();
    let snapshot = macos_memory::sample()?;
    let total = if snapshot.denied_pids.is_empty() {
        snapshot.native_rss_kib
    } else {
        let output = std::process::Command::new("/bin/ps")
            .args(snapshot.fallback_args()?).output()?;
        if !output.status.success() {
            return Err(std::io::Error::other("denied-PID ps failed"));
        }
        snapshot.total_rss_kib(&output.stdout)?
    };
    println!("{{\\\"native_kib\\\":{},\\\"denied_count\\\":{},\\\"hybrid_kib\\\":{},\\\"elapsed_ms\\\":{}}}",
        snapshot.native_rss_kib, snapshot.denied_pids.len(), total,
        started.elapsed().as_secs_f64() * 1000.0);
    Ok(())
}
''' % json.dumps(str(ROOT / 'src/macos_memory.rs')))
    driver = directory / 'memory_probe'
    subprocess.run(['rustc', '--edition=2021', '-O', '-Clto', '-Cpanic=abort', '-Awarnings', '--extern',
                    'libc=' + str(library), '-L', 'dependency=' + str(library.parent),
                    str(source), '-o', str(driver)], check=True, capture_output=True)
    return driver


def legacy_total():
    output = subprocess.check_output(['/bin/ps', '-caxm', '-orss,comm'], timeout=10)
    # Discard command fields immediately; only aggregate RSS leaves this function.
    values = (line.split(None, 1)[0] for line in output.splitlines() if line.strip())
    return sum(int(value) for value in values if value.isdigit())


def aggregate_probe(driver, samples):
    physical_kib = int(subprocess.check_output(['/usr/sbin/sysctl', '-n', 'hw.memsize'])) // 1024
    # Independent process snapshots have churn and include different ps/driver
    # processes. Permit at most 0.1 percentage point of physical memory, with a
    # 32MiB floor, outside the two reference totals. Report every observed gap.
    allowance_kib = max(32768, physical_kib // 1000)
    rows = []
    for _ in range(samples):
        started = time.monotonic()
        before = legacy_total()
        result = json.loads(subprocess.check_output([str(driver)], timeout=10))
        after = legacy_total()
        low, high = sorted((before, after))
        gap = max(low - result['hybrid_kib'], result['hybrid_kib'] - high, 0)
        rows.append(dict(result, reference_before_kib=before, reference_after_kib=after,
                         outside_bracket_kib=gap, bracket_ms=(time.monotonic()-started)*1000,
                         passed=gap <= allowance_kib))
    return dict(passed=all(row['passed'] for row in rows), samples=rows,
                allowance_kib=allowance_kib, physical_kib=physical_kib,
                max_outside_bracket_kib=max(row['outside_bracket_kib'] for row in rows),
                median_hybrid_ms=statistics.median(row['elapsed_ms'] for row in rows))


class OwnedProcesses:
    """Read only direct children of the ptop PID supplied by the harness."""
    def __init__(self):
        self.proc = ctypes.CDLL('/usr/lib/libproc.dylib', use_errno=True)
        self.proc.proc_listchildpids.argtypes = [ctypes.c_int, ctypes.c_void_p, ctypes.c_int]
        self.proc.proc_listchildpids.restype = ctypes.c_int
        self.proc.proc_pidinfo.argtypes = [ctypes.c_int, ctypes.c_int, ctypes.c_uint64,
                                          ctypes.c_void_p, ctypes.c_int]
        self.proc.proc_pidinfo.restype = ctypes.c_int

    def children(self, parent):
        buffer = (ctypes.c_int * 4096)()
        count = self.proc.proc_listchildpids(parent, buffer, ctypes.sizeof(buffer))
        if count < 0:
            return []
        return [pid for pid in buffer[:count] if pid > 0]

    def stopped(self, pid):
        buffer = ctypes.create_string_buffer(512)
        size = self.proc.proc_pidinfo(pid, 3, 0, buffer, len(buffer))
        return size >= 8 and struct.unpack_from('I', buffer, 4)[0] == 4  # SSTOP

    def present(self, pid):
        # Darwin kill(pid,0) can reject an unreaped zombie. Apple's ps also sees
        # zombies, so require actual absence, not just inability to signal it.
        result = subprocess.run(['/bin/ps', '-x', '-p', str(pid), '-o', 'pid='],
                                capture_output=True, timeout=5)
        if result.returncode not in (0, 1):
            raise RuntimeError('Could not verify sensor absence')
        return str(pid).encode() in result.stdout.split()

    @staticmethod
    def signal_sensor(pid, sig):
        try:
            os.kill(pid, sig)
        except ProcessLookupError:
            # Darwin can list an unreaped exited child yet reject kill(2).
            # The suspended parent still anchors its PID; there is no reuse.
            return False
        return True

    @contextmanager
    def suspended(self, child):
        # Popen leader is held unreaped throughout all signaling. Once stopped,
        # it cannot reap its sensor children, anchoring those PIDs as well.
        os.kill(child.pid, signal.SIGSTOP)
        try:
            deadline = time.monotonic() + 1
            while not self.stopped(child.pid):
                if child_exited(child.pid) or time.monotonic() >= deadline:
                    raise RuntimeError('Could not suspend owned ptop leader')
                time.sleep(.001)
            yield
        finally:
            os.kill(child.pid, signal.SIGCONT)

    def observe_new(self, child, seen, retain_hybrid):
        observations, retained = [], None
        with self.suspended(child):
            for pid in self.children(child.pid):
                if pid in seen:
                    continue
                if not self.signal_sensor(pid, signal.SIGSTOP):
                    continue
                try:
                    kind, stopped = self.inspect_sensor(pid)
                    observations.append((pid, kind))
                    if retain_hybrid and kind == 'hybrid_memory' and stopped:
                        retained = pid
                        break
                finally:
                    if pid != retained:
                        self.signal_sensor(pid, signal.SIGCONT)
        return observations, retained

    def inspect_sensor(self, pid):
        # /bin/ps is setuid-root: its argv/BSD details cannot be inspected through
        # our unprivileged libproc calls. Ask Apple's ps about ONLY this owned,
        # suspended child while its parent is suspended and cannot reap its PID.
        result = subprocess.run(['/bin/ps', '-x', '-p', str(pid), '-o', 'state=,command='],
                                capture_output=True, timeout=5)
        words = result.stdout.split()
        stopped = bool(words and words[0].startswith(b'T'))
        return self.classify(words), stopped

    @staticmethod
    def classify(args):
        if b'-ewwwo' in args:
            return 'process'
        if b'-caxm' in args:
            return 'legacy_memory'
        if b'pid=,rss=' in args and b'-p' in args:
            return 'hybrid_memory'
        return 'other'


def drain(master):
    while select.select([master], [], [], 0)[0]:
        try:
            if not os.read(master, 65536):
                return
        except OSError as error:
            if error.errno != errno.EIO:
                raise
            return


def phase_report(times, interval=.2):
    if len(times) < 2:
        return dict(observed=len(times), maximum_phase_error_ms=None)
    elapsed = [stamp - times[0] for stamp in times]
    # Missing observations do not turn a 400ms gap into a slower claimed poll.
    errors = [abs(stamp - round(stamp / interval) * interval) for stamp in elapsed]
    return dict(observed=len(times), maximum_phase_error_ms=max(errors)*1000,
                span_seconds=elapsed[-1])


def observe_sensors(observer, child, master, stop_hybrid):
    seen, events = set(), []
    stopped = None
    started = time.monotonic()
    while not child_exited(child.pid) and time.monotonic()-started < (6 if stop_hybrid else 1.5):
        drain(master)
        if any(pid not in seen for pid in observer.children(child.pid)):
            observations, stopped = observer.observe_new(child, seen, stop_hybrid and time.monotonic()-started >= 3)
            for pid, kind in observations:
                if kind != 'other':
                    seen.add(pid)
                    events.append((kind, time.monotonic()-started))
        if stopped is not None:
            break
        time.sleep(.002)
    return events, stopped


def cleanup_owned(observer, child):
    if child.returncode is not None:
        return
    if not child_exited(child.pid):
        with observer.suspended(child):
            for pid in observer.children(child.pid):
                observer.signal_sensor(pid, signal.SIGKILL)
        child.kill()
    child.wait()


def finish_pty(child, master, slave, initial, events, stopped, observer):
    quit_at = time.monotonic()
    os.write(master, b'q')
    while not child_exited(child.pid) and time.monotonic()-quit_at < 2:
        drain(master)
        time.sleep(.005)
    if not child_exited(child.pid):
        raise RuntimeError('ptop did not quit within two seconds')
    restored = termios.tcgetattr(slave) == initial
    sensor_survived = stopped is not None and observer.present(stopped)
    if sensor_survived:
        raise RuntimeError('Stopped sensor survived ptop shutdown')
    status = child.wait()
    phases = {kind: phase_report([stamp for event, stamp in events if event == kind])
              for kind in ('process', 'legacy_memory', 'hybrid_memory')}
    return dict(exit_code=status, quit_ms=(time.monotonic()-quit_at)*1000,
                terminal_restored=restored, stopped_hybrid=stopped is not None,
                stopped_child_reaped=not sensor_survived, observations=phases,
                passed=status == 0 and restored)


def pty_probe(binary, extra_args=(), path=SYSTEM_PATH, stop_hybrid=False):
    observer = OwnedProcesses()
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 24, 100, 0, 0))
    initial = termios.tcgetattr(slave)
    env = dict(os.environ, PATH=path, TERM='xterm-256color')
    child = subprocess.Popen([str(binary), '--update-interval', '300', '--no-mouse', *extra_args],
                             stdin=slave, stdout=slave, stderr=slave, env=env, start_new_session=True)
    stopped = None
    try:
        events, stopped = observe_sensors(observer, child, master, stop_hybrid)
        return finish_pty(child, master, slave, initial, events, stopped, observer)
    finally:
        cleanup_owned(observer, child)
        os.close(master)
        os.close(slave)


def override_probe(directory, binary):
    fake = directory / 'ps'
    log = directory / 'fixture-polls.jsonl'
    fake.write_text('#!' + sys.executable + '\n' +
                    'import json,sys,time\n' +
                    'kind="process" if "-ewwwo" in sys.argv else "memory"\n' +
                    'with open(' + repr(str(log)) + ', "a") as f: f.write(json.dumps([kind,time.monotonic()])+"\\n")\n' +
                    'print("  %CPU %MEM COMM\\n  9.0 1.0 fixture" if kind=="process" else "RSS COMM\\n1024 fixture")\n')
    fake.chmod(0o755)
    result = pty_probe(binary, path=str(directory) + os.pathsep + SYSTEM_PATH)
    rows = [json.loads(line) for line in log.read_text().splitlines()] if log.exists() else []
    memory = [stamp for kind, stamp in rows if kind == 'memory']
    result['fixture_memory_cadence'] = phase_report(memory)
    result['fixture_process_polls'] = sum(kind == 'process' for kind, _ in rows)
    result['passed'] &= len(memory) >= 5 and result['fixture_process_polls'] >= 1
    result['passed'] &= result['observations']['hybrid_memory']['observed'] == 0
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('binary', nargs='?', type=Path, default=ROOT/'target/release/ptop')
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--samples', type=int, default=20)
    args = parser.parse_args()
    if sys.platform != 'darwin' or args.samples < 1:
        parser.error('This live proof requires macOS and at least one sample')
    binary = args.binary.resolve()
    with tempfile.TemporaryDirectory(prefix='ptop-memory-proof-') as tmp:
        directory = Path(tmp)
        aggregate = aggregate_probe(compile_driver(directory, binary), args.samples)
        default = pty_probe(binary, stop_hybrid=True)
        reference = pty_probe(binary, extra_args=['--vtop-parity'])
        override = override_probe(directory, binary)
    expected_hybrid = any(row['denied_count'] for row in aggregate['samples'])
    if expected_hybrid:
        default['passed'] &= default['stopped_hybrid']
        default['passed'] &= default['observations']['hybrid_memory']['observed'] >= 3
    reference['passed'] &= reference['observations']['legacy_memory']['observed'] >= 3
    reference['passed'] &= reference['observations']['hybrid_memory']['observed'] == 0
    report = dict(binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
                  passed=all(part['passed'] for part in (aggregate, default, reference, override)),
                  aggregate=aggregate, default_pty=default, reference_pty=reference, override_pty=override,
                  limits=['Independent live snapshots include process churn and measurement processes.',
                          'Child observations are a lower bound, not an exact count of scheduler dispatches.',
                          'The observer briefly suspends owned processes: this is functional evidence, not a CPU benchmark.',
                          'cadence-proof.py separately verifies fixed200ms scheduling and overlap.'])
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(dict(passed=report['passed'], aggregate=aggregate['passed'],
                          default=default['passed'], reference=reference['passed'], override=override['passed'],
                          maximum_gap_kib=aggregate['max_outside_bracket_kib'], report=str(args.output))))
    return 0 if report['passed'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
