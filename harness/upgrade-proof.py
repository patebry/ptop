#!/usr/bin/env python3
"""Original upgrade.js transcript and safe real-PTY install/restart branches."""
import json
from pathlib import Path
import subprocess
import sys
ROOT=Path(__file__).resolve().parent.parent
actual=json.loads(subprocess.check_output(['node',str(ROOT/'harness/upstream-upgrade.js')]))
expected=json.loads((ROOT/'tests/upstream-upgrade.json').read_text())
assert actual==expected, 'installed upstream updater differs from recorded contract'
for case in actual:
    restart=case['effects'][-1]
    assert restart[:2]==['require',1000],case
    assert case['process']=={'0':{'theme':'nord'}},case
subprocess.run(['node',str(ROOT/'harness/restart-proof.js')],check=True)
binary=sys.argv[1] if len(sys.argv)>1 else str(ROOT/'target/release/ptop')
for case in ('--manual-update','--manual-control-update','--manual-fixed','--upgrade','--control-upgrade','--upgrade-failed','--upgrade-empty'):
    subprocess.run([sys.executable,str(ROOT/'harness/live-smoke.py'),binary,case],check=True)
subprocess.run([sys.executable,str(ROOT/'harness/password-proof.py'),binary],check=True)
print('PASS: 9 original node-sudo/read PTY cases, 3 restart-resolution cases, 4 original upgrade.js transcripts and 4 fake-effect PTY branches; 3 ptop manual-update isolation cases')
