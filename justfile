# ptop — build, test, proof
default:
    @just --list

build:
    cargo build --release

test:
    cargo test

fmt:
    cargo fmt --all -- --check

clippy:
    cargo clippy --all-targets -- -D warnings

proof:
    cargo build --release
    python3 harness/proof.py

live-smoke:
    cargo build --release
    python3 harness/live-smoke.py

fixtures:
    node harness/fixtures/gen.js

ci: fmt clippy test proof live-smoke

dist:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo build --release
    mkdir -p dist
    shasum -a 256 target/release/ptop > dist/SHASUMS256.txt