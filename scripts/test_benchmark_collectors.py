"""Collector adapter checks; no real monitors or long measurements."""
import contextlib
import io
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import benchmark_collectors as collectors


class CollectorBenchmarkTests(unittest.TestCase):
    def options(self, directory, *extra):
        paths = [Path(directory) / label for label in collectors.TOOLS]
        for path in paths:
            path.write_text('placeholder')
            path.chmod(0o700)
        args = ['--baseline', str(paths[0]), '--candidate', str(paths[1]), *extra]
        return collectors.parser().parse_args(args), args

    def test_both_intervals_have_complete_balanced_pairs(self):
        with tempfile.TemporaryDirectory() as directory:
            options, _ = self.options(directory, '--soak-seconds', '180')
        records = list(collectors.schedule(options))
        self.assertEqual(len(records), 17)
        for interval in (300, 1000):
            runs = [row for row in records if row['record'] == 'run' and row['interval_ms'] == interval]
            self.assertEqual([row['tool'] for row in runs],
                             ['baseline', 'candidate', 'candidate', 'baseline'] * 2)
            self.assertEqual([row['pair'] for row in runs], [0, 0, 1, 1, 2, 2, 3, 3])
        self.assertEqual(records[-1], dict(record='soak', tool='candidate',
                                          interval_ms=300, requested_seconds=180))

    def test_native_experiment_excludes_path_overrides_and_fixture_environment(self):
        with patch.dict(os.environ, {'PATH': '/private/fake-tools', 'PTOP_CAPTURE': 'private',
                                     'LINES': '999', 'COLUMNS': '999'}):
            env = collectors.environment('/temporary/config')
        self.assertEqual(env['PATH'], collectors.SYSTEM_PATH)
        self.assertEqual(env['XDG_CONFIG_HOME'], '/temporary/config')
        for key in ('PTOP_CAPTURE', 'LINES', 'COLUMNS'):
            self.assertNotIn(key, env)

    def test_soak_records_only_elapsed_time_and_rss_and_uses_nonoverlapping_windows(self):
        with patch.object(collectors.time, 'monotonic', side_effect=[100, 100, 129.9, 130, 249.9, 250, 279.9]):
            sampler = collectors.TimedResident(lambda _pid: 4096)
            for _ in range(6):
                self.assertEqual(sampler(424242), 4096)
        result = sampler.retention(180)
        self.assertEqual((result['first_window_samples'], result['last_window_samples']), (2, 2))
        self.assertEqual(result['last_minus_first_bytes'], 0)
        self.assertNotIn('424242', json.dumps(result))
        self.assertEqual(result['samples_elapsed_seconds_rss_bytes'][0], [0, 4096])

    def test_missing_rss_and_missing_soak_window_fail_without_inventing_zero(self):
        sampler = collectors.TimedResident(lambda _pid: None)
        self.assertIsNone(sampler(123))
        self.assertEqual(sampler.samples, [])
        with self.assertRaises(collectors.benchmark.BenchmarkError):
            sampler.retention(180)

    def test_validation_rejects_nonfinite_soak_duplicate_intervals_and_same_binary(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(collectors.sys, 'platform', 'darwin'):
            for extra in (('--soak-seconds', 'nan'), ('--soak-seconds', 'inf'),
                          ('--soak-seconds', '-1'), ('--soak-seconds', '59'),
                          ('--intervals', '300', '300'), ('--pairs', '0')):
                options, _ = self.options(directory, *extra)
                with self.subTest(extra=extra), self.assertRaises(collectors.benchmark.BenchmarkError):
                    collectors.validate(options)
            options, _ = self.options(directory)
            options.candidate = options.baseline
            with self.assertRaises(collectors.benchmark.BenchmarkError):
                collectors.validate(options)

    def test_measure_rejects_changed_binary_before_and_after_run(self):
        with tempfile.TemporaryDirectory() as directory:
            options, _ = self.options(directory, '--pairs', '1', '--seconds', '1')
            binaries = {label: getattr(options, label) for label in collectors.TOOLS}
            info = {'tools': {'baseline': {'sha256': 'expected'}}}
            spec = next(collectors.schedule(options))
            for hashes, calls in [(['changed'], 0), (['expected', 'changed'], 1)]:
                with patch.object(collectors.benchmark, 'digest', side_effect=hashes), \
                        patch.object(collectors.benchmark, 'run', return_value={}) as run:
                    with self.assertRaises(collectors.benchmark.BenchmarkError):
                        collectors.measure(options, binaries, info, spec, lambda _pid: 4096)
                    self.assertEqual(run.call_count, calls)

    def test_main_preserves_default_commands_private_output_and_completion(self):
        with tempfile.TemporaryDirectory() as directory:
            _, args = self.options(directory, '--pairs', '1', '--seconds', '1')
            info = {'record': 'metadata', 'tools': {label: {'sha256': 'hash'} for label in collectors.TOOLS}}
            roots = []
            intervals = []

            def fake_run(command, env, seconds, cols, rows, resident):
                roots.append(Path(env['XDG_CONFIG_HOME']))
                intervals.append(command[2])
                self.assertEqual(command[1:], ['--update-interval', command[2], '--no-mouse', '--theme', 'parallax'])
                self.assertEqual((seconds, cols, rows), (1, 100, 30))
                self.assertTrue(roots[-1].exists())
                return dict(exit_code=0, terminal_bytes=10, own_rss_mean_bytes=4096)

            output = io.StringIO()
            with patch.object(collectors.sys, 'platform', 'darwin'), \
                    patch.object(collectors, 'metadata', return_value=info), \
                    patch.object(collectors.benchmark, 'ResidentMemory'), \
                    patch.object(collectors.benchmark, 'digest', return_value='hash'), \
                    patch.object(collectors.benchmark, 'run', side_effect=fake_run), \
                    contextlib.redirect_stdout(output):
                self.assertEqual(collectors.main(args), 0)
            self.assertEqual(intervals, ['300', '300', '1000', '1000'])
            self.assertEqual(len(set(roots)), 4)
            self.assertTrue(all(not root.exists() for root in roots))
            records = [json.loads(line) for line in output.getvalue().splitlines()]
            self.assertEqual(records[-1], dict(record='complete', measured_records=4, paired_runs=4, soak_runs=0))
            self.assertNotIn(directory, output.getvalue())

    def test_failed_run_has_no_completion_marker(self):
        with tempfile.TemporaryDirectory() as directory:
            _, args = self.options(directory, '--pairs', '1', '--seconds', '1')
            output = io.StringIO()
            with patch.object(collectors.sys, 'platform', 'darwin'), \
                    patch.object(collectors, 'metadata', return_value={'record': 'metadata'}), \
                    patch.object(collectors.benchmark, 'ResidentMemory'), \
                    patch.object(collectors, 'measure', side_effect=collectors.benchmark.BenchmarkError('failure')), \
                    contextlib.redirect_stdout(output):
                with self.assertRaises(collectors.benchmark.BenchmarkError):
                    collectors.main(args)
            self.assertEqual([json.loads(line)['record'] for line in output.getvalue().splitlines()], ['metadata'])


if __name__ == '__main__':
    unittest.main()
