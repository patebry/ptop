#!/usr/bin/env python3
"""Compare every checked-in fixture and fail on missing expectations."""
from pathlib import Path
import subprocess
import os
import json

fixtures = sorted(Path("fixtures").glob("*.json"))
if not fixtures:
    raise SystemExit("FAIL: no fixtures")
failures = []
for fixture in fixtures:
    expected = Path("expected", fixture.stem + ".txt")
    result = subprocess.run(["target/release/ptop", "--vtop-parity", "--capture", str(fixture)], capture_output=True)
    if os.environ.get("PTOP_UPSTREAM_VTOP"):
        driver = "harness/upstream-app.js" if json.loads(fixture.read_text()).get("brand", "vtop") == "vtop" else "harness/vtop-mirror.js"
        oracle = subprocess.run(["node", driver, str(fixture)], capture_output=True, timeout=15)
        if oracle.returncode or result.stdout != oracle.stdout:
            failures.append(str(fixture) + " (upstream app)")
    if not expected.is_file() or result.returncode or result.stdout != expected.read_bytes():
        failures.append(str(fixture))
if failures:
    raise SystemExit("FAIL: " + ", ".join(failures))
print(f"PASS: {len(fixtures)} byte-exact frame fixtures")
if os.environ.get("PTOP_UPSTREAM_VTOP"):
    print(f"PASS: {len(fixtures)} frames against installed upstream app (fixed-brand extension uses renderer adapter)")
