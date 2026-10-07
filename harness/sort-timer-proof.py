#!/usr/bin/env python3
"""Compare upstream timer traces, then verify real PTY deadline wakeups.

Only fixture ps/npm executables run. No process actions or installations occur.
"""
import fcntl
import json
import os
from pathlib import Path
import pty
import re
import select
import struct
import subprocess
import sys
import tempfile
import termios
import time

fixture = Path('tests/upstream-sort-timers.json')
result = subprocess.run(['node', 'harness/upstream-app.js', str(fixture), '--state'], capture_output=True, check=True, timeout=15)
assert json.loads(result.stdout) == {'states': json.loads(fixture.read_text())['expected'], 'commands': []}
binary = str(Path(sys.argv[1] if len(sys.argv) > 1 else 'target/debug/ptop').resolve())
with tempfile.TemporaryDirectory(prefix='ptop-sort-timers-') as directory:
    root = Path(directory)
    for name, body in {
        'npm': "print('{\"dist-tags\":{\"latest\":\"0.6.1\"}}')",
        'ps': "import sys\nprint('  %CPU %MEM COMM\\n  9.0 1.0 alpha-fixture\\n  6.0 1.0 beta-fixture\\n  3.0 1.0 gamma-fixture' if '-ewwwo' in sys.argv else 'RSS COMM\\n1024 fixture')",
    }.items():
        path = root / name
        path.write_text(f'#!{sys.executable}\n{body}\n')
        path.chmod(0o755)
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 24, 100, 0, 0))
    process = subprocess.Popen([binary, '--update-interval', '5000', '--quit-after', '30'], stdin=slave, stdout=slave, stderr=slave, env=dict(os.environ, PATH=str(root)+os.pathsep+os.environ['PATH'], TERM='xterm-256color'))
    output = bytearray()
    consumed = 0
    selected = re.compile(rb'\x1b\[[0-9;]*48;5;135(?:;[0-9]+)*m ([a-z]+)-fixture')

    def drain(seconds):
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            ready, _, _ = select.select([master], [], [], max(0, deadline-time.monotonic()))
            if ready:
                output.extend(os.read(master, 65536))

    def wait_selected(expected, seconds):
        # Keep the entire stream and consume individual selection events. A
        # single read may contain both the key repaint and a timer repaint.
        global consumed
        deadline = time.monotonic() + seconds
        while True:
            events = selected.findall(output)
            while consumed < len(events):
                value = events[consumed]
                consumed += 1
                if value == expected:
                    return time.monotonic()
            assert process.poll() is None, 'monitor exited during timer proof'
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                return None
            drain(min(.01, remaining))

    try:
        assert wait_selected(b'alpha', 8), 'initial process selection never rendered'
        # Two callbacks must be pending together. A heavily descheduled harness
        # can miss their 200ms observation window; retry that measured scheduling
        # miss, but never count it as a passing timer observation.
        completed = False
        for attempt in range(5):
            os.write(master, b'cg')
            drain(.35)
            consumed = len(selected.findall(output))
            os.write(master, b'j')
            assert wait_selected(b'beta', 1), 'navigation did not select beta'
            start = time.monotonic()
            os.write(master, b'm')
            drain(.14)
            if time.monotonic() - start >= .19:
                drain(.5)
                os.write(master, b'g')
                wait_selected(b'alpha', 1)
                continue
            second_sent = time.monotonic()
            os.write(master, b'c')
            first_reset = wait_selected(b'alpha', 1)
            assert first_reset, 'first sort callback did not reset selection'
            if first_reset - second_sent >= .15:
                # Insufficient time remains to observe navigation before the
                # second callback. This attempt proves nothing about that timer.
                drain(.5)
                continue
            os.write(master, b'j')
            assert wait_selected(b'beta', 1), 'navigation between callbacks missing'
            assert wait_selected(b'alpha', 1), 'second independent sort callback was lost'
            completed = True
            break
        assert completed, 'CI scheduling never provided an observable overlapping timer window'
        os.write(master,b'q')
        drain(0.2)
        assert process.wait(timeout=2)==0
        assert termios.tcgetattr(slave)[3] & termios.ICANON
        print('PASS: actual upstream queued timer trace and two observed independent selection resets at 5-second draw interval')
    finally:
        if process.poll() is None:
            process.kill()
            process.wait()
        os.close(master)
        os.close(slave)
