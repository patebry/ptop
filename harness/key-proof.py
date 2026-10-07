#!/usr/bin/env python3
"""Verify the Rust unit test's recorded trace against the installed application."""
import json
import subprocess
from pathlib import Path
fixture = Path('tests/upstream-key-trace.json')
expected = json.loads(fixture.read_text())['expected']
result = subprocess.run(['node', 'harness/upstream-app.js', str(fixture), '--state'], capture_output=True, check=True, timeout=15)
actual = json.loads(result.stdout)
assert actual['states'] == expected, (actual['states'], expected)
assert actual['commands'] == [['install', 'vtop', [{'theme': 'parallax'}]]]
print(f'PASS: {len(expected)-1} actual-upstream key/timer transitions')
