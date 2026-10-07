#!/usr/bin/env python3
"""Actual tmux cell-background regression at 120x21; fake sensor commands only.

Usage: python3 harness/live-render-proof.py [binary] --output /tmp/ptop-live-proof
Exits 1 for leaked backgrounds. Requires tmux; no Python packages. The terminal
emulator resolves the live ANSI stream; this does not inspect capture-sheet rows.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shlex
import shutil
import subprocess
import tempfile
import time
import uuid


def background_cells(styled):
    """Read backgrounds from tmux's canonical SGR-only cell dump."""
    result = []; row = []; bg = None
    for part in re.split(r'(\x1b\[[0-9;]*m|\n)', styled):
        if part == '\n':
            result.append(row); row = []
        elif part.startswith('\x1b['):
            codes = [int(v or 0) for v in part[2:-1].split(';')]; i = 0
            while i < len(codes):
                code = codes[i]
                if code in (0, 49): bg = None
                elif 40 <= code <= 47: bg = code - 40
                elif 100 <= code <= 107: bg = code - 92
                elif code in (38, 48):
                    mode = codes[i+1]; n = 1 if mode == 5 else 3 if mode == 2 else 0
                    if not n: raise ValueError('unknown color mode')
                    color = codes[i+2:i+2+n]
                    if code == 48: bg = color[0] if n == 1 else tuple(color)
                    i += n+1
                i += 1
        else:
            if '\x1b' in part: raise ValueError('non-SGR escape in tmux dump')
            row.extend((ch, bg) for ch in part)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('binary', nargs='?', default=str(Path(__file__).resolve().parents[1]/'target/release/ptop'))
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args(); binary = Path(args.binary).resolve()
    args.output.mkdir(parents=True, exist_ok=False)
    server = 'ptop-render-' + uuid.uuid4().hex
    prefix = [shutil.which('tmux') or 'tmux', '-L', server, '-f', '/dev/null']
    def tmux(*argv):
        return subprocess.run(prefix+list(argv), check=True, capture_output=True, timeout=5).stdout.decode()
    with tempfile.TemporaryDirectory(prefix='ptop-render-sensors-') as tmp:
        fake = Path(tmp)
        for name, body in {
            'ps': 'case "$*" in *-ewwwo*) printf "  %%CPU %%MEM COMM\\n  9.0 1.0 alpha\\n  7.0 2.0 beta\\n  3.0 3.0 gamma\\n";; *) printf "RSS COMM\\n1024 alpha\\n2048 beta\\n3072 gamma\\n";; esac',
            'free': 'printf "total used free\\nMem: 1000 100 900\\n"',
            'npm': "printf '{\"dist-tags\":{\"latest\":\"0.6.1\"}}\\n'",
        }.items():
            p = fake/name; p.write_text('#!/bin/sh\n'+body+'\n'); p.chmod(0o755)
        command = ['env', 'TERM=xterm-256color', 'LC_ALL=en_US.UTF-8',
                   'PATH='+tmp+os.pathsep+os.environ['PATH'], str(binary), '--update-interval', '300', '--no-mouse']
        report = dict(binary=str(binary), sha256=hashlib.sha256(binary.read_bytes()).hexdigest(), dimensions=[120,21], tmux_version=subprocess.check_output([prefix[0],'-V'],text=True).strip(), checkpoints=[])
        try:
            tmux('new-session','-d','-s','proof','-x','120','-y','21','/bin/sleep 86400')
            tmux('set-option','-g','status','off')
            tmux('set-option','-g','remain-on-exit','on')
            tmux('resize-window','-t','proof:0','-x','120','-y','21')
            tmux('respawn-pane','-k','-t','proof:0.0',shlex.join(command))
            time.sleep(2.6)
            for name, key, selected in [('first',None,'alpha'),('down','Down','beta'),('down-again','Down','gamma'),('up','Up','beta'),('later-frame',None,'beta')]:
                if key: tmux('send-keys','-t','proof:0.0',key)
                time.sleep(.7)
                styled = tmux('capture-pane','-p','-e','-N','-t','proof:0.0')
                plain = tmux('capture-pane','-p','-N','-t','proof:0.0')
                (args.output/(name+'.ansi')).write_text(styled)
                (args.output/(name+'.txt')).write_text(plain)
                rows = background_cells(styled)
                selected_rows = [i for i,r in enumerate(rows) if selected in ''.join(ch for ch,bg in r)]
                if len(selected_rows)!=1: raise AssertionError('fixture selection row unavailable')
                y = selected_rows[0]
                # Process box interior is x61..118 inclusive at this geometry.
                expected = {(y,x) for x in range(61,119)}
                # Intentional white shortcut key labels in the footer.
                expected.update((20,x) for x in (2,3,19,27,33,48,66,81))
                actual = {(yy,x) for yy,r in enumerate(rows) for x,(ch,bg) in enumerate(r) if bg is not None}
                issues = sorted(actual-expected); missing = sorted(expected-actual)
                report['checkpoints'].append(dict(name=name,selected=selected,background_count=len(actual),expected_background_count=66,unexpected_background_cells=issues,missing_selected_background_cells=missing,passed=not issues and not missing))
            tmux('send-keys','-t','proof:0.0','q')
            time.sleep(.2)
        finally:
            try: tmux('kill-server')
            except (subprocess.TimeoutExpired,subprocess.CalledProcessError): pass
        report['passed'] = all(c['passed'] for c in report['checkpoints']) and len(report['checkpoints'])==5
        (args.output/'report.json').write_text(json.dumps(report,indent=2)+'\n')
        print(json.dumps(dict(passed=report['passed'],sha256=report['sha256'],checkpoints=[dict(name=c['name'],passed=c['passed'],background_count=c['background_count']) for c in report['checkpoints']])))
        return 0 if report['passed'] else 1

if __name__ == '__main__':
    raise SystemExit(main())
