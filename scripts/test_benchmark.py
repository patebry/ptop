"""Small harness checks; no monitors, builds, or live benchmark runs."""
import os
from pathlib import Path
import sys
import unittest

import benchmark


class BenchmarkTests(unittest.TestCase):
    def test_order_alternates_complete_pairs(self):
        self.assertEqual(list(benchmark.orders(3)), [
            (0, 'vtop'), (0, 'ptop'), (1, 'ptop'), (1, 'vtop'), (2, 'vtop'), (2, 'ptop')])

    def test_nonfinite_or_unbounded_duration_is_rejected(self):
        for value in ['nan', 'inf', '0', '-1', '3601']:
            with self.assertRaises(benchmark.argparse.ArgumentTypeError):
                benchmark.duration(value)

    def test_environment_removes_fixture_and_injection_settings(self):
        from unittest.mock import patch
        with patch.dict(os.environ, {'PTOP_CAPTURE': 'private', 'NODE_OPTIONS': 'private',
                                     'DYLD_INSERT_LIBRARIES': 'private'}, clear=True):
            result = benchmark.environment('/temporary/config')
        self.assertNotIn('PTOP_CAPTURE', result)
        self.assertNotIn('NODE_OPTIONS', result)
        self.assertNotIn('DYLD_INSERT_LIBRARIES', result)
        self.assertEqual(result['TERM'], 'xterm-256color')

    def test_version_rejects_path_output_without_exposing_it(self):
        from unittest.mock import patch
        result = benchmark.subprocess.CompletedProcess([], 0, b'/private/person/program', b'')
        with patch.object(benchmark.subprocess, 'run', return_value=result):
            with self.assertRaises(benchmark.BenchmarkError) as error:
                benchmark.version(Path('unused'), {})
        self.assertNotIn('/private', str(error.exception))

    def test_owned_pty_is_drained_and_terminal_content_is_discarded(self):
        program = ('import os,tty;tty.setraw(0);os.write(1,b"private process text");'
                   'os.read(0,1)')
        result = benchmark.run([sys.executable, '-c', program], dict(os.environ),
                               1.0, 100, 30, lambda _pid: 4096)
        self.assertEqual(result['exit_code'], 0)
        self.assertEqual(result['own_rss_mean_bytes'], 4096)
        self.assertEqual(result['terminal_bytes'], len('private process text'))
        self.assertNotIn('private', str(result))

    def test_premature_exit_is_not_a_valid_measurement(self):
        with self.assertRaises(benchmark.BenchmarkError):
            benchmark.run([sys.executable, '-c', 'pass'], dict(os.environ),
                          1.0, 100, 30, lambda _pid: 4096)

    def test_descendants_are_killed_before_exited_leader_is_reaped(self):
        import select
        for graceful in (False, True):
            with self.subTest(graceful=graceful):
                read_fd, write_fd = os.pipe()
                os.set_inheritable(write_fd, True)
                program = f"""
import os, signal, time, tty
tty.setraw(0)
r, w = os.pipe()
if os.fork() == 0:
    os.close(r)
    signal.signal(signal.SIGTERM, signal.SIG_IGN)
    signal.signal(signal.SIGHUP, signal.SIG_IGN)
    os.write({write_fd}, b'R')
    os.write(w, b'R')
    while True:
        time.sleep(10)
os.close(w)
os.read(r, 1)
os.write(1, b'ready')
if {graceful!r}:
    os.read(0, 1)
os._exit(0)
"""
                try:
                    if graceful:
                        benchmark.run([sys.executable, '-c', program], dict(os.environ),
                                      1.0, 100, 30, lambda _pid: 4096)
                    else:
                        with self.assertRaises(benchmark.BenchmarkError):
                            benchmark.run([sys.executable, '-c', program], dict(os.environ),
                                          1.0, 100, 30, lambda _pid: 4096)
                    os.close(write_fd)
                    write_fd = -1
                    self.assertTrue(select.select([read_fd], [], [], 2)[0])
                    self.assertEqual(os.read(read_fd, 1), b'R')
                    # EOF proves the TERM-ignoring descendant closed its last
                    # writer, even when the leader exited before cleanup.
                    self.assertTrue(select.select([read_fd], [], [], 2)[0])
                    self.assertEqual(os.read(read_fd, 1), b'')
                finally:
                    os.close(read_fd)
                    if write_fd >= 0:
                        os.close(write_fd)

    def test_unexpected_pty_read_error_is_not_silenced(self):
        from unittest.mock import patch
        program = 'import os,time;os.write(1,b"ready");time.sleep(10)'
        failure = OSError(benchmark.errno.EBADF, 'injected read failure')
        with patch.object(benchmark.os, 'read', side_effect=failure):
            with self.assertRaises(OSError) as error:
                benchmark.run([sys.executable, '-c', program], dict(os.environ),
                              1.0, 100, 30, lambda _pid: 4096)
        self.assertEqual(error.exception.errno, benchmark.errno.EBADF)


if __name__ == '__main__':
    unittest.main()
