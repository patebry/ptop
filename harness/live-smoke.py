#!/usr/bin/env python3
"""Exercise the actual terminal/input loop, with harmless ps/killall substitutes."""
import fcntl
import os
from pathlib import Path
import pty
import select
import struct
import subprocess
import sys
import tempfile
import termios
import time

upgrade_mode = any(arg.startswith("--upgrade") or arg=="--control-upgrade" for arg in sys.argv)
manual_mode = any(arg.startswith("--manual") for arg in sys.argv)
upgrade_empty = "--upgrade-empty" in sys.argv
upgrade_failed = "--upgrade-failed" in sys.argv
binary = str(Path(sys.argv[1] if len(sys.argv) > 1 else "target/release/ptop").resolve())
with tempfile.TemporaryDirectory(prefix="ptop-live-") as directory:
    root = Path(directory)
    for name, body in {
        "npm": "import os,sys\nfrom pathlib import Path\nPath(os.environ['PTOP_NPM_LOG']).write_text(repr(sys.argv[1:]))\nprint('{\"dist-tags\":{\"latest\":\"" + ("9.9.9" if upgrade_mode else "0.6.1") + "\"}}')",
        "sudo": "import json,os,sys,termios\nfrom pathlib import Path\nPath(os.environ['PTOP_SUDO_LOG']).write_text(json.dumps({'args':sys.argv[1:], 'piped':not os.isatty(0)}))\nprint('up to date' if os.environ.get('PTOP_UPGRADE_EMPTY') else 'link -> /fixture/vtop.js')\nsys.exit(1 if os.environ.get('PTOP_UPGRADE_FAILED') else 0)",
        "node": "import json,os,sys\nfrom pathlib import Path\nPath(os.environ['PTOP_RELAUNCH_LOG']).write_text(json.dumps(sys.argv[1:]))\nsys.exit(1 if json.loads(sys.argv[-1]).get('module') is False else 0)",

        "ps": "import sys\nprint('  %CPU %MEM COMM\\n  9.0 1.0 alpha-fixture\\n  6.0 1.0 beta-fixture\\n  3.0 1.0 gamma-fixture' if '-ewwwo' in sys.argv else 'RSS COMM\\n1024 fixture')",
        "killall": "import os,sys\nfrom pathlib import Path\nassert len(sys.argv)==3 and sys.argv[1]=='--'\nPath(os.environ['PTOP_KILL_LOG']).write_text(sys.argv[2])",
    }.items():
        executable = root / name
        executable.write_text(f"#!{sys.executable}\n{body}\n")
        executable.chmod(0o755)
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 160 if upgrade_mode else 100, 0, 0))
    env = dict(os.environ, PATH=str(root) + os.pathsep + os.environ["PATH"], TERM="xterm-256color", HOSTNAME="test", PTOP_NPM_LOG=str(root / "npm.log"), PTOP_KILL_LOG=str(root / "kill.log"), PTOP_SUDO_LOG=str(root / "sudo.log"), PTOP_RELAUNCH_LOG=str(root / "relaunch.log"))
    if upgrade_empty: env['PTOP_UPGRADE_EMPTY']='1'
    if upgrade_failed: env['PTOP_UPGRADE_FAILED']='1'
    before = termios.tcgetattr(slave)
    brand_args = ["--vtop-parity"] if upgrade_mode else (["--fix"] if "--manual-fixed" in sys.argv else [])
    process = subprocess.Popen([binary, *brand_args, "--update-interval", "50", "--quit-after", "5"], stdin=slave, stdout=slave, stderr=slave, env=env)
    output = bytearray()

    def drain(seconds):
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            ready, _, _ = select.select([master], [], [], max(0, deadline - time.monotonic()))
            if ready:
                output.extend(os.read(master, 65536))

    def wait_for_kill(expected):
        deadline = time.monotonic() + 2
        observed = None
        while time.monotonic() < deadline:
            assert process.poll() is None, f"monitor exited before kill target {expected!r}: status {process.returncode}"
            try:
                observed = (root / "kill.log").read_text()
            except FileNotFoundError:
                observed = None
            if observed == expected:
                return
            drain(.05)
        raise AssertionError(f"kill target did not become {expected!r} within 2s; observed {observed!r}")

    try:
        deadline = time.monotonic() + 3
        while time.monotonic() < deadline:
            drain(.05)
            if b"alpha-fixture" in output and b"beta-fixture" in output and (not upgrade_mode or b"9.9.9" in output):
                break
        assert b"alpha-fixture" in output and b"beta-fixture" in output, "process rows never rendered"
        if manual_mode:
            assert b"9.9.9" not in output
            os.write(master, b"\x15" if "--manual-control-update" in sys.argv else b"u")
            deadline = time.monotonic() + 3
            while time.monotonic() < deadline and process.poll() is None:
                drain(.05)
            drain(.05)
            message = b"npm install -g @patebryant/ptop"
            assert message in output, repr(output[-1000:])
            assert output.index(b"\x1b[?1049l") < output.index(message), "manual message printed before terminal restoration"
        elif upgrade_mode:
            import json
            # Long hostnames and centered load text can obscure the prefix.
            # Exact notice layout is covered separately by renderer fixtures.
            assert b"9.9.9" in output, "update notice version missing"
            os.write(master, b"\x15" if "--control-upgrade" in sys.argv else b"u")
            deadline = time.monotonic() + 4
            while time.monotonic() < deadline and process.poll() is None:
                drain(.05)
            assert (root / "relaunch.log").exists(), repr(output[-1500:])
            sudo = json.loads((root / "sudo.log").read_text())
            assert sudo == {"args": ["-S", "-p", "#node-sudo-passwd#", "npm", "install", "-g", "vtop"], "piped": True}
            restart = json.loads((root / "relaunch.log").read_text())
            assert restart[0] == "-e"
            assert json.loads(restart[-1]) == {"module": False if upgrade_empty else "/fixture/vtop.js", "undefined": False, "theme": "parallax", "argv": [binary, "--update-interval", "50", "--quit-after", "5"]}
            assert (b"up to date\r\n\r\n" if upgrade_empty else b"link -> /fixture/vtop.js\r\n\r\n") in output
            assert b"Installing vtop update..." in output
            assert b"Finished updating. Clearing cache and relaunching..." in output
        else:
            os.write(master, b"j")
            drain(0.2)
            os.write(master, b"dd")
            wait_for_kill("beta-fixture")
            os.write(master, b"\x1b[<0;53;18M")
            drain(0.1)
            os.write(master, b"dd")
            wait_for_kill("gamma-fixture")
            os.write(master, b"\x1b[<64;53;18M")
            drain(0.1)
            os.write(master, b"dd")
            wait_for_kill("alpha-fixture")
            os.write(master, b"q")
            drain(0.2)
        assert process.wait(timeout=2) == (1 if upgrade_empty else 0)
        assert b"\x1b[?1049l" in output, "alternate screen not restored"
        assert termios.tcgetattr(slave) == before, "terminal settings not restored"
        if not upgrade_mode:
            for effect in ("npm.log", "sudo.log", "relaunch.log"):
                assert not (root / effect).exists(), f"ptop branding triggered forbidden updater effect: {effect}"
        else:
            assert (root / "npm.log").read_text() == "['info', '--json', 'vtop']"
        if manual_mode:
            print("PASS: default ptop manual update, no npm/sudo/restart, exact terminal restoration")
        else:
            print("PASS: updater notice, explicit install action and relaunch (all effects faked)" if upgrade_mode else "PASS: live rows, keys, mouse click/wheel, dd literal target, quit and terminal restoration")
    finally:
        if process.poll() is None:
            process.kill()
            process.wait()
        os.close(master)
        os.close(slave)
