# Live terminal color-state repair, 2026-10-06

A user screenshot showed purple blocks in graph history and behind unselected
process values. The cell grid was correct, but the live ANSI emitter incorrectly
assumed default SGR at each row. Terminal cursor movement preserves attributes,
so the selected row's purple background leaked into subsequent rows and frames.

The live emitter now resets SGR before each nonempty changed row. Capture-sheet
bytes remain unchanged. This also resets inherited foreground and text flags.
The dotted braille graphs and hard command truncation are upstream styling;
they are separate from the unintended background leakage.

The regression replays ANSI background state across rows and frames, including
clearing a selected cell. It fails on the original emitter (purple 135 instead
of default background) and passes on the repaired emitter. Formatting, clippy,
42 unit tests, eight integration tests, 91 frame fixtures (90 against the actual
upstream application plus one brand extension), and live interaction smoke tests
pass. The prior frame-only proof did not establish continuous live visual parity.

Repaired release SHA-256:
`178704a018de2b97515c5c3598a197852f8e9758c7302dc2bb767f257401b925`.

The earlier performance report retains the measured pre-repair binary hash;
its numbers have not been relabeled as measurements of this release. The
unsuppressed structural quality report has 26 gating findings against original
HEAD, including broad pre-existing work, test size and churn; it is not clean.

Independent tmux replay at 120×21 verified five live frames/selections. The old
binary accumulated 111 to 230 colored-background cells. The repaired binary
retained exactly 66 intended cells: 58 in the selected process row and eight
white footer key labels. Graphs and unselected rows retained default backgrounds.

Repeat the live check with tmux installed:

```sh
python3 harness/live-render-proof.py --output /tmp/ptop-live-proof-fresh
```

[Old-build evidence](../harness/live-render-old-proof.json) and
[repaired-build evidence](../harness/live-render-new-proof.json) retain hashes
and per-checkpoint background-cell results. The old graph alone had 21–45
incorrectly filled cells; the repaired graph had zero at every checkpoint.
