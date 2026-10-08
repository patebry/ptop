"""Adapter checks; live monitors and Darwin APIs are not needed."""
import contextlib
import io
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import compare_monitors as comparison


class ComparisonTests(unittest.TestCase):
    def test_four_repetitions_balance_every_tool_in_every_position(self):
        order = list(comparison.orders(4))
        self.assertEqual(len(order), 16)
        for offset in range(4):
            self.assertEqual({label for _, label in order[offset::4]}, set(comparison.TOOLS))
        for repetition in range(4):
            self.assertEqual({label for rep, label in order if rep == repetition}, set(comparison.TOOLS))

    def test_version_accepts_btop_sgr_but_rejects_paths_multiline_and_controls(self):
        for output, accepted in [(b'btop version: \x1b[1m1.4.7\x1b[0m\n', True),
                                 (b'btop version: 1.4.7\n/private/build', False),
                                 (b'/private/person/bin/btop', False),
                                 (b'\x1b[2Jbtop version: 1.4.7', False)]:
            result = subprocess.CompletedProcess([], 0, output, b'private stderr')
            with self.subTest(output=output), patch.object(comparison.subprocess, 'run', return_value=result) as run:
                if accepted:
                    self.assertEqual(comparison.tool_version('btop', Path('unused'), {}), 'btop version: 1.4.7')
                else:
                    with self.assertRaises(comparison.benchmark.BenchmarkError) as error:
                        comparison.tool_version('btop', Path('unused'), {})
                    self.assertNotIn('private', str(error.exception))
                self.assertEqual(run.call_args.args[0], ['unused', '-V'])

    def test_isolation_overrides_user_configuration_and_does_not_modify_it(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            user_config = root / 'user-htoprc'
            user_config.write_text('do not change')
            run_root = root / 'run'
            run_root.mkdir()
            with patch.dict(os.environ, {'HTOPRC': str(user_config), 'LINES': '999',
                                         'COLUMNS': '999', 'XDG_STATE_HOME': '/private/state',
                                         'PTOP_CAPTURE': '/private/capture', 'NODE_OPTIONS': 'private'}):
                env = comparison.isolated_environment(run_root)
            self.assertEqual(user_config.read_text(), 'do not change')
            self.assertEqual(env['HTOPRC'], str(run_root / 'htoprc'))
            for key in ('XDG_CONFIG_HOME', 'XDG_STATE_HOME', 'XDG_CACHE_HOME'):
                self.assertTrue(Path(env[key]).is_relative_to(run_root))
                self.assertTrue(Path(env[key]).is_dir())
            for key in ('LINES', 'COLUMNS', 'PTOP_CAPTURE', 'NODE_OPTIONS'):
                self.assertNotIn(key, env)

    def test_btop_defaults_only_change_mouse_and_save(self):
        original = 'disable_mouse = false\nsave_config_on_exit = true\nshown_boxes = "cpu mem net proc"\n'
        result = subprocess.CompletedProcess([], 0, original.encode(), b'')
        with patch.object(comparison.subprocess, 'run', return_value=result):
            config = comparison.btop_defaults(Path('btop'), {})
        self.assertEqual(config, original.replace('disable_mouse = false', 'disable_mouse = true')
                         .replace('save_config_on_exit = true', 'save_config_on_exit = false'))
        for output in ('disable_mouse = false\n', original + 'disable_mouse = true\n'):
            result = subprocess.CompletedProcess([], 0, output.encode(), b'')
            with patch.object(comparison.subprocess, 'run', return_value=result):
                with self.assertRaises(comparison.benchmark.BenchmarkError):
                    comparison.btop_defaults(Path('btop'), {})

    def test_commands_request_same_ui_interval_and_default_ptop(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for label in ('ptop', 'vtop'):
                self.assertEqual(comparison.command(label, Path(label), root, ''),
                                 [label, '--update-interval', '1000', '--no-mouse', '--theme', 'parallax'])
            self.assertEqual(comparison.command('htop', Path('htop'), root, ''),
                             ['htop', '--delay', '10', '--no-mouse', '--readonly'])
            args = comparison.command('btop', Path('btop'), root, 'safe defaults')
            self.assertEqual(args[:4], ['btop', '--update', '1000', '--config'])
            self.assertEqual(Path(args[4]).read_text(), 'safe defaults')

    def test_main_uses_fresh_configuration_each_run_and_emits_only_metrics(self):
        with tempfile.TemporaryDirectory() as directory:
            executable = Path(directory) / 'monitor'
            executable.write_text('unused')
            executable.chmod(0o700)
            argv = [item for tool in comparison.TOOLS for item in ('--' + tool, str(executable))]
            argv += ['--repetitions', '1', '--seconds', '1']
            info = {'record': 'metadata', 'tools': {tool: {'sha256': 'hash'} for tool in comparison.TOOLS}}
            roots = []

            def fake_run(args, env, seconds, cols, rows, resident):
                roots.append(Path(env['HTOPRC']).parent)
                self.assertTrue(roots[-1].exists())
                self.assertEqual((seconds, cols, rows), (1, 100, 30))
                return {'exit_code': 0, 'terminal_bytes': 100, 'own_rss_mean_bytes': 4096}

            output = io.StringIO()
            with patch.object(comparison.sys, 'platform', 'darwin'), \
                    patch.object(comparison, 'metadata', return_value=(info, 'defaults')), \
                    patch.object(comparison.benchmark, 'ResidentMemory'), \
                    patch.object(comparison.benchmark, 'digest', return_value='hash'), \
                    patch.object(comparison.benchmark, 'run', side_effect=fake_run), \
                    contextlib.redirect_stdout(output):
                self.assertEqual(comparison.main(argv), 0)
            self.assertEqual(len(set(roots)), 4)
            self.assertTrue(all(not root.exists() for root in roots))
            records = [json.loads(line) for line in output.getvalue().splitlines()]
            self.assertEqual([record['sequence'] for record in records[1:]], [0, 1, 2, 3])
            self.assertNotIn(directory, output.getvalue())


if __name__ == '__main__':
    unittest.main()
