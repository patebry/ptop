#!/usr/bin/env python3
"""Compare actual commander CLI exits and output with the Rust executable."""
import os
import subprocess

if not os.environ.get('PTOP_UPSTREAM_VTOP'):
    raise SystemExit('PTOP_UPSTREAM_VTOP is required')
cases = [
    ['--help'], ['-h'], ['--version'], ['-V'], ['--bogus'], ['-tnord'],
    ['--help', '--bogus'], ['--bogus', '--version'], ['-hV'], ['-Vh'],
    ['--update-interval=-2'], ['--theme', '--bad'], ['--no-mouse=false', '--version'],
    ['--theme=-', '--version'], ['--theme=', '--help'], ['--', '--bogus', '--help'],
]
# The terminator case intentionally doesn't exit; assess it as Rust unit parse state.
for args in cases[:-1]:
    expected = subprocess.run(['node', 'harness/cli-oracle.js', *args], capture_output=True, timeout=5)
    actual = subprocess.run(['target/release/ptop', '--vtop-parity', *args], capture_output=True, timeout=5)
    assert (actual.returncode, actual.stdout, actual.stderr) == (expected.returncode, expected.stdout, expected.stderr), (args, actual, expected)
print(f'PASS: {len(cases)-1} upstream commander output/error/exit cases')
