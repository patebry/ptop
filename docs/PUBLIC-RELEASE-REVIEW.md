# Public-release preparation

This source snapshot follows the reviewed private development revision
`04ea7d7`. The public repository starts with fresh history; earlier revisions
remain in a separate private archive. The published npm `0.1.0` artifact is
unchanged.

## Source and presentation

- The keyboard loop now shares poll/read retry handling. It retains the same
  20 retries, 200 ms backoff, timeout behavior, and successful-event reset.
- An unused allocation was removed from the offline capture renderer.
- The README preview is rendered from `docs/assets/demo.json`, with synthetic
  process names, hostname and clock. It is not a capture of a user's processes.
- The timing fixture is now a synthetic 850-row workload with 585 command groups,
  including duplicates and names containing spaces. No recorded inventory is
  needed to run the tests.
- Benchmark tooling lives in this repository. Historical results remain tied
  to their original executable hashes; they are not new-release speed claims.

Regenerate the preview from a source checkout:

```sh
cargo build --locked --release
python3 scripts/preview.py
uv run --with cairosvg==2.9.1 python -c 'import cairosvg; cairosvg.svg2png(url="docs/assets/ptop.svg", write_to="docs/assets/ptop.png")'
```

CairoSVG is an optional documentation-rendering dependency, not a ptop runtime
or npm installation dependency. The preview uses the application's actual
capture composition; braille cells are rendered as dots without depending on
font coverage. Other glyphs use the available monospace font.

## Quality findings

`docs/quality-review-2026-10-06.json` is a historical comparison against
`7bcab7be8+dirty`. It is retained as dated evidence, not a current clean verdict.
Most earlier gating findings were file churn or test/data size, rather than
runtime defects. Compatibility parsing and rendering tables are intentionally
kept intact where a split would only move complexity elsewhere.

The input-loop change reduces cyclomatic complexity from 15 to 13 and cognitive
complexity from 18 to 17 against `dc02e03`, without adding helpers or allocations.
The scoped quality review still flags historical churn in the two edited Rust
symbols; no acknowledgements or suppressions were added. New benchmark lifecycle
code is reviewed and tested separately; a non-gating new symbol is not assumed
clean merely because the quality tool does not block it.

## Privacy and history

Current evidence files replace absolute executable and temporary capture paths
with portable labels. Their annotations explicitly identify these redactions;
recorded numbers and binary hashes are unchanged. Those hashes identify the
original executables, not the sanitized JSON documents.

A scan of the eleven commits through `dc02e03` found no credentials with gitleaks.
The current source snapshot no longer contains the identified personal home or
machine-specific temporary-directory paths. These are finite checks, not a
promise that an automated scanner detects every sensitive value.

## Public history boundary

The public repository was initialized from the reviewed source snapshot, without
importing the development Git database. Earlier process samples, local paths,
and committed build files remain only in the separate private development
archive. No development branches, tags, objects, or build directories were
copied into the new repository.

Historical commit identifiers in these documents identify the development
baseline for the evidence; those commits are not part of this public history.
The initial public commit uses the owner's GitHub noreply address.
