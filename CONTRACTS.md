# ptop Parity Contracts — Implementation Law

This document is the binding contract for ptop. The Desktop spec (`ptop-SPEC.md`)
is the plan; this document corrects it against the ORACLE: vtop 0.6.1's actual
source (`app.js`, `node_modules/drawille`, `node_modules/blessed`, `sensors/`),
read byte-by-byte on 2026-10-05. **When spec and oracle disagree, the oracle
wins.** Every deviation here is marked ORACLE-DIVERGE.

## Verified corrections (2026-10-06)

These corrections supersede conflicting historical notes below:

- macOS vtop evaluates `total / 1024 ^ 2` as `(ToInt32(total / 1024) ^ 2)`,
  **not** `total / (1024 ^ 2)`. Its denominator is os-utils total memory in MiB.
  Fixed mode passes physical bytes to `memory_mac_fixed`.
- CPU polls start every 200 ms and measure a rolling one-second window, not
  independent nonoverlapping one-second windows. Linux IRQ is `/proc/stat`
  field 6 (index 5 after `cpu`); field 5 is iowait.
- Default live mode now selects vtop arithmetic/branding. `--fix` selects
  corrected arithmetic and ptop branding; `--vtop-parity` remains accepted.
- Mouse is enabled by default, with click selection and two-row wheel steps.
  `--no-mouse` disables capture. Blessed Ctrl-u/d/b/f navigation is supported.
- Live scrolling preserves its
  offset when moving upward; capture fixtures describe an initial selection.
- `just proof` must fail when an expected fixture is missing. There are 69
  checked-in fixtures. `PTOP_UPSTREAM_VTOP` additionally runs them through the
  installed original blessed/drawille packages. Whole-program parity is not
  established by a finite fixture suite.

## Actual application oracle corrections (second pass)

- `harness/upstream-app.js` runs installed vtop 0.6.1 application source,
  injecting only sensors, time and effect adapters, plus a state access hook.
  It confirms 90 original-brand frames; a separate fixed-brand fixture is an
  extension test. CLI and key traces additionally exercise actual commander and
  blessed event dispatch.
- Default CLI help/version match vtop (0.6.1); `--fix` identifies ptop. Commander
  splits short clusters, normalizes equals, ignores unknown options when
  positional arguments exist, and gives version/help their upstream precedence.
- Blessed defaults to fullUnicode=false: wide characters become `??`, combining
  marks disappear, non-wide surrogate pairs become `?`. The table slices by
  UTF-16 code units before this normalization. Content control characters and
  tabs follow blessed's original normalization; selected items suppress embedded
  foreground changes.
- The upstream j/k update-suspension branch compares a key object to strings,
  so it never executes. Default ptop likewise does not freeze table updates.
  Home/End do nothing. H/M/L combine viewport navigation with zoom/sort actions.
  Sort selection resets after 200 ms. Key/clock redraws do not advance charts.
- Upstream treats Ctrl-d and d as the same name for dd; Ctrl-u invokes the
  updater and also moves half a page. These surprising default behaviors are
  reproduced and tested using fake effect commands.
- `npm info --json vtop` controls the version notice by string inequality, not
  semver ordering. Explicit u/Ctrl-u restores the terminal and installs package
  vtop. Output chunks receive the original extra newline, and the third space-
  separated token in a stdout chunk containing `vtop.js` supplies the module path.
  Closing the installer always prints the completion message and waits one second,
  even on failure. A parsed path is required in a fresh Node process with the
  original CLI arguments and upstream process[0] theme property; relative/bare
  paths resolve at npm's installed vtop/upgrade.js location using createRequire.
  Three real-module oracle cases cover absolute, relative and bare targets.
  Missing/undefined paths reproduce a module-ID failure.
  Password challenges use node-sudo's exact -S sentinel and node-read prompt,
  hidden input and terminal modes; submitted buffers are cleared after writing
  to sudo. Fresh-Node exception stack frames differ from in-process require.
  Four original upgrade.js VM transcripts and four fake-effect PTY flows cover
  success, Ctrl-u, failed installer with a path, and missing path. Nine actual
  node-sudo/read PTY comparisons cover plain input, retry, cancellation, editing,
  line clearing, cursor/delete keys, Unicode, punctuation word deletion and empty EOF, using synthetic credentials only.

## C0. Core data facts (verified from oracle)

- drawille **1.1.0** (vendored at `harness/drawille.js`): width/height setters
  floor to multiple of 2 / 4; `content` = `Uint8Array(width*height/8)`.
- blessed **0.1.81**.
- vtop `app.js` 647 LOC. Values: JS arrays with holes → our `Vec<Option<f64>>`.
- Chart dims: `charts[0]`: `(graph.width-3)*2` × `(graph.height-2)*4`;
  `charts[1]`: `(graph2.width-3)*2` × `((graph2.height-2)*4)`. Canvas ctor
  itself clamps to multiples: W=(W/2)*2, H=(H/4)*4.
- Blessed geometry facts (auto-padding screen): border chars sit **inside**
  the box rect (`xi++; xl--; yi++; yl--` at render). `label = itop = ibottom = ileft = iright = 1`
  for bordered boxes; label Box drawn at `rtop = childBase - itop` (row -1 rel
  content top), `rleft = 2 - ileft` = 1 col right of border. For bordered
  parents: children get `iheight=2`, `ileft=1`, `iright=1` via blessed's
  `Element.prototype.__defineGetter__` block (iheight = padding.top+padding.bottom
  + (border?2:0) + 2*(scrollbar?1:0) — actually reads: `iheight` sums tp+bt with
  border adding +2; verified: bordered box content area shrinks by border rows).
- Uninitialized chart: `graph.setContent(drawChart(k))` where drawChart
  returned `false` → blessed setContent(false) = `false || ''` = **EMPTY**
  (renders nothing; NOT the string 'false' — contracts correction.)
- Percent label splice (app.js drawChart):
  ```js
  textOutput[0] = textOutput[0].slice(0, len-4) + '{white-fg}' + ('   '+value).slice(-3) + '%{/white-fg}'
  ```
  i.e. **first line**, drop last **4 chars**, insert tag+3-char-value+%.
- Box content fill char when content ends: `bch` default = **space**, attr = box dattr.
- Footer/header/etc all have `tags: true` (parseTags) — vtop sets tags on all boxes.

## C1. BrailleCanvas — port of harness/drawille.js (NOT the spec's §5.1)

```js
set(x, y):              // f64 args
  if (!(x >= 0 && x < this.width && y >= 0 && y < this.height)) return;  // RAW float check, BEFORE floor
  x = floor(x); y = floor(y);
  nx = floor(x / 2); ny = floor(y / 4);
  coord = nx + (width / 2) * ny;
  content[coord] |= map[y % 4][x % 2];
```
- `map = [[0x01,0x08],[0x02,0x10],[0x04,0x20],[0x40,0x80]]`
- x may be negative-FRACTIONAL: `x = -0.5` → guard `x >= 0` FAILS → skipped.
  `x = -0.0` passes (0 >= 0), floors to 0, draws. Replicate exactly: on the
  Rust side the guard is `x >= 0.0 && x < width_f && y >= 0.0 && y < height_f`
  done in **f64 before flooring**.
- ORACLE-DIVERGE from spec: `x = -0.5` is NOT drawn (guard precedes floor).
- `frame()`: no leading delimiter. For each byte: if `j == width/2` push
  `'\n'`, reset j; then push space (byte 0) or `U+2800 + byte`. One **trailing**
  `'\n'`. Result: H/4 lines each `width/2` chars, trailing newline.
- `clear()`: fill(0). Buffer alloc: `width*height/8` bytes.

## C2. drawChart — verbatim from app.js (spec had `ceil` and wrong hole semantics)

```js
drawChart(chartKey):
  chart = charts[k]; c = chart.chart; c.clear()
  if (!chart.plugin.initialized) return false
  values[position] = chart.plugin.currentValue        // hole-preserving assignment
  computeValue = input => chart.height - floor(((chart.height+1)/100)*input) - 1
  if (position > 5000) delete values[position-5000]   // sparse map, holes possible
  for (const pos in values) {                          // pos: string key of NON-HOLE index, insertion order
    if (graphScale >= 1 || (graphScale < 1 && pos % (1/graphScale) === 0)) {
      p = parseInt(pos,10) + (chart.width - values.length)
      x = p*graphScale + (1-graphScale)*chart.width
      if (p > 1 && computeValue(values[pos-1]) > 0) c.set(x, computeValue(values[pos-1]))   // values[pos-1] may be a HOLE → undefined → NaN → guard false → skip
      for (let y = computeValue(values[pos-1]); y < chart.height; y++) {
        if (graphScale > 1 && p > 0 && y > 0) {
          current = computeValue(values[pos-1]); next = computeValue(values[pos])
          diff = (next-current)/graphScale
          for (let i = 0; i < graphScale; i++) {        // i f64 stepping 1.0
            c.set(x+i, y+(diff*i))
            for (let j = y+(diff*i); j < chart.height; j++) c.set(x+i, j)   // j = y+(diff*i) UNFLOORED f64; set() floors. spec's ceil() is WRONG.
          }
        } else if (graphScale <= 1) {
          c.set(x, y)
        }
      }
    }
  }
  // label splice (C0) then join('\n')
```
Hole semantics: `for..in` skips holes (indices with `= undefined`), so a hole
is never drawn; `values[pos-1]` of a hole is also undefined → computeValue
NaN → `c.set` no-ops (guard fails on NaN) → top line skipped; loop
`for y = NaN; ...` doesn't execute. Port: `Vec<Option<f64>>`; hole = None;
computeValue(None) = None → skip. `values[pos-1]`: JS object lookup of
`"3"` when `3` hole → undefined. IMPORTANT: pos-1 of index 0 → values[-1] →
undefined (property access) → NaN. Draw order == insertion order, but since
delete happens at `position-5000` (lower index), iteration is ascending for
our purposes. `values.length`: JS array `.length` INCLUDES trailing holes but
not beyond last assignment; equals `position+1` after assignment (arrays
auto-extend). Note `delete values[k]` leaves length; `values.length` counts
highest index+1. Port: keep `position: u64` and window = indices
`max(0, position-5000) ..= position`, `len = min(position+1, 5001)`.

Percent string: `percent = '   ' + currentValue` (string concat:
`'   ' + 7` → `'   7'`; `'   ' + 12.5` → `'   12.5'`); `.slice(-3)`.
currentValue is vtop's number → our f64; JS number→string via Ryū semantics.
Values arriving at drawChart are integers (floors from sensors) except table;
label: format_number_js(currentValue).slice(-3).

## C3. drawTable — verbatim from app.js

```js
drawTable(chartKey): # charts[2], width = processList.width - 3, no canvas, no height use
  columnLengths = {}
  columns = ['Command','CPU %','Count','Memory %'].slice(0).reverse()   # ['Memory %','Count','CPU %','Command']
  lastItem = columns[columns.length-1]  # 'Command'
  minimumWidth = 12; padding = 1; if width>50 padding=2; if width>80 padding=3
  do {
    totalUsed = 0; firstLength = 0
    for (const column in columns) {
      item = columns[column]
      if (item === lastItem) { columnLengths[item] = width - totalUsed; firstLength = columnLengths[item] }
      else { columnLengths[item] = item.length + padding }
      totalUsed += columnLengths[item]
    }
    if (firstLength < minimumWidth && columns.length > 1) { totalUsed=0; columns.shift(); removeColumn=true }
    else removeColumn=false
  } while(removeColumn)
  # shift() drops the FIRST element of the reversed array = LAST column ('Memory %'),
  # then 'Command' becomes new lastItem... NO: lastItem is captured BEFORE the loop. It stays 'Command'.
  # After shift, first item is 'Count'... lastItem remains 'Command'.
  columns.reverse()   # back to ['Command','CPU %','Count',(...)]
  titleOutput = '{bold}'
  for (const headerColumn in columns) {  # display order
    colText = ' ' + columns[headerColumn]
    titleOutput += colText + stringRepeat(' ', columnLengths[columns[headerColumn]] - colText.length)
  }
  titleOutput += '{/bold}' + '\n'
  bodyOutput = []
  for (const row in chart.plugin.currentValue) {
    currentRow = rows[row]
    rowText = ''
    for (const bodyColumn in columns) {
      colText = ' ' + currentRow[columns[bodyColumn]]
      rowText += (colText + stringRepeat(' ', columnLengths[columns[bodyColumn]] - colText.length)).slice(0, columnLengths[columns[bodyColumn]])
    }
    bodyOutput.push(rowText)
  }
  return { title: titleOutput, body: bodyOutput, processWidth: columnLengths[columns[0]] }
```
Notes:
- processWidth = columnLengths of columns[0] AFTER final reverse — i.e. the
  first displayed column ('Command' unless 'Command' was dropped; Command can
  never be dropped since it owns the remainder: length is chart.width-minus-others,
  and the loop exits when firstLength >= 12 — could a non-'Command' column end
  up first? Only if ALL of Memory%, Count, CPU% got shifted — then
  columns = ['Command'] length 1 → loop condition `columns.length > 1` false →
  break with Command alone). ORACLE-DIVERGE from spec §5.3 (spec said Command
  dropped first; source shifts 'Memory %' first, i.e. from the END of the
  display order).
- `columnLengths` persists ACROSS do-iterations (stale keys linger; harmless
  except when re-allocating: lengths recomputed per column each pass).
- body cell: `' ' + cell` then truncate to columnLength — never padded AFTER
  truncate because truncation only bites when longer. JS string pad via
  stringRepeat (num<0 → ''). `.slice(0,n)` bites when colText longer.
  Numbers from row objects: JS number-to-string semantics (Count is integer).
- The i++ stray in the loop body (`i++`) mutates an outer `var i` — harmless in
  JS (var i = 0 from the for loop earlier in module? No — the `var i` in
  drawHeader: `for (var i = 0...)` — module-scope var i. Harmless: i is unused
  elsewhere afterward). DO NOT port (no-op).
- The dead tempStats/existingStats/setItems-commented block is not ported.
- `chart.plugin.currentValue` rows are objects with keys Command, Count, CPU %,
  Memory %, plus lowercase cpu/mem raw fields. Column keys select exactly the
  capitalized display fields. JS number semantics: `'Count': 4` → renders '4'.

## C4. Sensor: process.js — verbatim

```js
exec('ps -ewwwo %cpu,%mem,comm')           exec cwd: user's; env: full user env
stdout.split('\n'); lines[0] = ''          # header line becomes '' → parsed as: words[0]='' words[1]='' → cpu="" mem="" offset=0+0+2=2, comm=remaining of trimmed line? line is '' → skip? parse: words[0] undefined? ''.split(' ') = [''] length 1 → words[1] undefined → skipped. GOOD: header skipped by construction (first line of ps is header '  %CPU %MEM COMM'; actually line[0] = '' REPLACED)
for line in lines:
  currentLine = line.trim().replace('  ', ' ')    # ONCE (JS replace is single)
  words = currentLine.split(' ')
  if words[0] !== undefined && words[1] !== undefined:   # NOTE: '' !== undefined → TRUE, so empty lines pass!
    cpu = words[0].replace(',', '.')              # string
    mem = words[1].replace(',', '.')
    offset = cpu.length + mem.length + 2
    comm = currentLine.slice(offset)
    if darwin: comm = comm.split('/').last        # keep after last '/'
    else: comm = comm.split('/')[0]               # keep before first '/'
    stats[comm] = exists ? { cpu: parseFloat(prev.cpu)+parseFloat(cpu), mem: ..., count+1 } : { cpu: string, mem: string, count: 1 }
```
- ⚠ words[0] === '' for blank lines: cpu = '' ; mem = '' (words length 1 →
  words[1] undefined → SKIPPED). Only lines with ≥2 space-separated tokens
  parse. parseFloat('') = NaN → group totals become NaN (a real vtop bug —
  a comm line with empty cpu like `ps` header remnant: header line is
  replaced by '' so no; lines like `  0.0` alone won't split into 2).
- Group aggregation on existing: cpu stored as STRING on first sighting, then
  parseFloat(prev) + parseFloat(cur) on repeat — port exactly (mixed string/number).
- Display: `cpuRounded = parseFloat(stats.cpu / os.cpus().length).toFixed(1)`;
  `memRounded = parseFloat(stats.mem).toFixed(1)`;
  `'Count': stats.count` (number); `'CPU %': cpuRounded` etc.
- toFixed(1) rounding: JS toFixed = round-half-away-from-zero on the decimal
  string (V8 uses correct decimal rounding of the BINARY double — note:
  toFixed uses the double's exact decimal expansion; 0.35 → '0.3' or '0.4'
  depending on binary representation. Port with exact f64 decimal rounding:
  use ryu/`fmt` style that ROUND HALF EVEN on exact decimal? NO — toFixed
  performs correct rounding of the exact binary value: produce the decimal
  string with n digits by rounding the EXACT binary value (ties: exact decimal
  halves occur when the binary value is exactly representable at n+1 digits —
  the 'round half up' vs 'even' question: V8's toFixed rounds half AWAY FROM
  ZERO on exact ties; e.g. (0.25).toFixed(1) = '0.3', (0.35).toFixed(1)='0.4'? NO:
  (0.3*100)... The safe approach: implement exact-decimal rounding of the f64's
  true value with ties-away-from-zero, matching V8.) Use the `ryu`-based
  `js-num-to-string` semantics for the integer/count conversion, and a
  hand-written toFixed(1).
- Sort: `statsArray.sort((a,b) => parseFloat(b[sort]) - parseFloat(a[sort]))`
  where sort ∈ 'cpu'|'mem' = the RAW (per-core-summed, unaveraged) numbers as
  strings; V8 sort is stable → ties keep insertion order.
  Insertion order = first-sighting order of comm in ps output.
- ps invoked exactly: `ps -ewwwo %cpu,%mem,comm` (no shell: childProcess.exec
  uses /bin/sh -c; byte-identical argv to shell).

## C5. Sensor cpu.js + os-utils

- `os-utils.cpuUsage(cb)` = snapshot `os.cpus()` sums (idle/total over ALL cpus,
  total = user+nice+sys+idle+irq), wait 1000 ms, snapshot again,
  `perc = (1 - (endIdle-startIdle)/(endTotal-startTotal))`, cb(perc);
  sensor does `floor(v*100)`, sets initialized=true.
- vtop polls CPU every 200 ms, each measuring a 1000 ms interval. Rust keeps
  a bounded deque of native snapshots and publishes overlapping one-second
  deltas every 200 ms after warmup. No process or thread is spawned per sample.
- macOS snapshots use `host_processor_info`; Linux reads `/proc/stat` fields
  user, nice, system, idle, irq (index 5 after `cpu`).
- Edge: totalΔ == 0 → JS: perc = NaN → floor(NaN*100) = NaN →
  `chart.plugin.currentValue = NaN` → drawChart: computeValue(NaN)=NaN →
  skips (set guard). Label: `'   NaN'.slice(-3)` = 'NaN'. Reproduce: label
  'NaN%'? '%'+... produces slice(-3)+'%' → 'NaN%'? wait percent = '   '+NaN →
  '   NaN'.slice(-3) = 'NaN' → label 'NaN%'. Keep this behavior (it's rare).

## C6. Sensor memory.js — verified upstream arithmetic

- Linux invokes `free -m`, reads used/total from the second output line and
  computes `Math.round(100 * used / total)`.
- macOS invokes `ps -caxm -orss,comm` and sums numeric-leading RSS values in KiB.
  Vtop evaluates `total / 1024 ^ 2` as `(ToInt32(total / 1024) ^ 2)` because
  division binds before XOR. ToInt32 truncates, wraps modulo 2^32 and interprets
  the result as signed. `os-utils.totalmem()` returns MiB.
- The upstream result is `Math.round((1 - (totalMiB - usedmem) / totalMiB) * 100)`.
  Do not simplify this expression or move XOR into the denominator.
- Fixed mode computes `round(100 * RSS_KiB * 1024 / physical_bytes)`, clamped
  to 0..100. Its input denominator is bytes, not MiB.
- Numeric-leading parsing strips from the first ASCII letter before parseInt;
  headers and nonnumeric rows are skipped.

## C7. Chrome — header/footer/clock/loadavg

- Header text (no upgrade notice): ` {bold}vtop{/bold}{white-fg} for HOSTNAME {/}`? EXACT:
  `' {bold}vtop{/bold}{white-fg} for ' + hostname + ' '`
  (tags: after {/bold} the white-fg starts, never closed until end → trailing
  space is white-fg). headerTextNoTags = ' vtop for HOSTNAME ' .
  width = headerTextNoTags.length. Parity: 'vtop'; default brand: 'ptop'.
- date: width 9 right-aligned content `'HH:MM:SS '` (trailing space).
- load avg: `'Load Average: ' + a.toFixed(2)+' '+b.toFixed(2)+' '+c.toFixed(2)`,
  width 28 centered at `left = floor(cols/2 - 14)`.
  1-second intervals for both; rendered through screen.render.
- Footer: for c of ['dd','j','k','g','G','c','m'] (keys of commands object):
  text += '  {white-bg}{black-fg}' + key + '{/black-fg}{/white-bg} ' + desc
  descs: dd Kill process, j Down, k Up, g Jump to top, G Jump to bottom,
  c Sort by CPU, m Sort by Mem; then + '{|}http://parall.ax/vtop'.
  fg: theme.footer.fg. Position: top rows-1, width 100%.
  The {|} token: content = left part, spaces fill, then right part.
  NOTE footer includes g/G (spec §4.3's correction listed fewer; oracle has all 7).

## C8. Blessing — tags → SGR (blessed 0.1.81 Program._attr)

Tags used by vtop (exhaustive):
`{bold}` `{/bold}` `{white-fg}` `{/white-fg}` `{white-bg}` `{black-fg}`
`{/black-fg}` `{/white-bg}` `{red-bg}` `{/red-bg}` `{/}`.
SGR mapping: bold=1; /bold=22; white-fg=37; /white-fg=39; white-bg=47;
black-fg=30; /black-fg=39; red-bg=41; /red-bg=49; {/}=ESC[m (normal).
Parse algorithm: blessed Element._parseTags — see oracle; stateful stacks
fg/bg/flag; {/}→normal+clear stacks; other-*/  → single attr reset via
_attr(param,false). Then attrCode converts SGR strings back into attr ints.
For ptop's capture: we need the FINAL GRID, so port the tag parser to produce
(char, attr) cell streams directly (skip SGR round-trip, but FINAL attrs must
match attrCode semantics), and the terminal emission must produce byte streams
matching blessed's Screen.draw (below).

## C9. Grid → bytes (blessed 0.1.81 Screen.draw)

- dattr = (0 << 18) | (0x1ff << 9) | 0x1ff  (default fg/bg = 0x1ff = default)
- attr diffs: when attr changes: if prev != dattr: emit ESC[m; if new != dattr:
  emit 'ESC[' + [flags(bold,ul,blink,inverse,invisible: 1;4;5;7;8)] + [bg
  (if != 0x1ff: <16 → 40+n or 100+n-8; else 48;5;n + ';')] + [fg: 30+n |
  90+n-8 | 38;5;n + ';'] (strip trailing ';' + 'm') — **bg BEFORE fg**.
- Movement: blessed uses tput cup(y,x) = 'ESC[y+1;H' formatted `ESC[{row};{col}H`
  (ansi.0 cup string: 'ESC[%i%p1%d;%p2%dH') → ptop emits `ESC[y{+1};x{+1}H`.
- BCE (back_color_erase) optimization + ACS + cuf optimizations: ptop v1 SKIPS
  these (ptop's own emitter may differ in escape stream; rendered grid is the
  contract — spec §7 stretch goal: diff-stream parity NOT required).
- Capture mode prints the grid text with attrs — see C10.

## C10. Capture mode (the diff protocol)

`--capture fixtures/case-N.json`: load JSON:
```
{ "theme": "parallax", "cols": W, "rows": R, "parity": true,
  "cpu_values": [...or holes as null...], "cpu_scale": 1, "mem_values": [...],
  "mem_scale": 1, "mem_value_label": 42, "cpu_value_label": 7,
  "cpu_initialized": true, "mem_initialized": true,
  "procs": [{"Command": "...", "Count": 4, "CPU %": "0.4", "Memory %": "1.0"}],
  "clock": "12:34:56 ", "loadavg": [1.23, 4.56, 7.89],
  "hostname": "mac.local", "brand": "vtop"|"ptop", "selected": 0, "scroll": 0 }
```
Render the FULL 24×100 (or W×R) frame through the blessed-equivalent renderer
and print: for each screen row, the row's content with SGR attrs applied as
blessed-would-emit (using C9 run logic at row start: emit attrs only when
they change; movement: start each row with `ESC[r+1;1H`), then a final newline
after the sheet. This is `actual-N.txt`.
The JS mirror (B1) produces the same bytes via the REAL blessed 0.1.81
(`require('blessed')` with a fake program on a fake TTY buffer) — the contract
is byte-equality of these two files.

## C11. Layout & boxes (ratatui-free where parity binds)

Boxes (from app.js): graph {top1, left0, w=cols, h=floor(rows/2)}, 
graph2 {top: graph.height+1, w=floor(cols/2), h=graph.height-2},
processList {top: graph.height+1, left: graph2.width, w=cols-graph2.width,
h=graph.height-2}, list {top:1(left-of-parent-content), width: pl.width-2,
height: pl.height-3}.
- blessed '50%' width = Math.floor(cols/2) etc. (blessed uses `parseInt` style
  rounding — actually percentage in blessed: `Math.floor(pct/100*max)`; verify:
  blessed helpers.parseCalc? In blessed, '50%' → half? blessed's `Element positioning:
  '50%' resolves as Math.floor(max * 0.5)?` — for parity cases only 100/24-ish
  even dims matter; assert even on fixture inputs.)
- Border chars: '┌┐└┘─│', battr = sattr(border style). Label: at row box.top,
  starting col box.left+2... for bordered box: label Box parent-relative:
  rleft = 2 - ileft: ileft for bordered = 1? → rleft = 1 → abs left = box.left+1+1
  hmm: parent content top-left = left+1, top+1 (border inside). label.rleft=2-ileft
  where ileft=1 → 1 → label.abs.left = content.left + 1 = box.left + 2.
  rtop = childBase - itop = -1 → label.abs.top = content.top - 1 = box.top.
  Label content ' ' + title + ' ' (set with leading/trailing space) shrinks to
  fit; label style: this.style.label (theme may not define → default: box's
  own fg? blessed Box label default style {} → dattr of label element:
  sattr({}) → fg undefined → colors.convert(undefined) → ? verify: style.fg
  undefined → convert(undefined) → typeof undefined → not string/number/array
  → -1 → 0x1ff DEFAULT. So label renders with DEFAULT terminal fg unless theme
  defines label colors — vtop themes don't define label styles → label is
  DEFAULT FG. (Verify against themes: vtop themes have no 'label' key.)
- The label content includes SGR? No: plain text. BUT labels sit on the border
  row; the border is drawn first, then children render over → label overwrites
  border chars (attr = label dattr = default attr: {bold? no} default fg+bg,
  no bold).
- processList list items: createItem → Box {content, tags: parseTags (true),
  height 1, right: (scrollbar?1:0) → 0 (no scrollbar? blessed list scrollbar
  only if style.scrollbar defined; vtop defines items.selected bg — not
  scrollbar) → items occupy full width, aligned left, no wrap? items content
  may exceed width → item Box wraps? items have no scrollable; the ITEM boxes
  are height 1 with shrink=false → content CLIPS at width (no wrap: options has
  no wrap → defaults true! blessed default wrap = true (unless scrollable?).
  Hmm: the item Box: 'wrap' defaults true; item content longer than width →
  wraps to second line → but item height fixed 1 → second line not drawn
  (render loop only draws height rows) → effectively clipped. Port: clip.)
- Selected row: item Box is selected when index == list.selected → its style
  resolves bg from theme items.selected.bg → bg renders behind WHOLE item row
  INCLUDING trailing spaces to box width (item Box width = parent width minus
  iright? autoPadding: item width = parent width - ileft - iright - scrollbar...
  createItem: right: scrollbar?1:0 → with autoPadding item.position has no
  left/right → width defaults to parent's content width minus? blessed Box
  default width = parent content width. So full content width row bg.)
- list scrolls: scrollTo(selected) → childBase = clamp; visible = height - iheight(2);
  childBase = selected - visible + 1 when selected beyond.
  Items drawn top = list content top + i - childBase.
- The table title (drawn via processList.setContent(table.title)) occupies row
  list top - 1? NO: processList box content = title (1 line ' {bold}…' with
  SGR). The list is a CHILD box at top 1 of parent → parent content row 0 = title,
  rows 1.. = items. The processList box has NO scroll bars; title row attr:
  starts with {bold} → bold on; the text ' Command  CPU %  Count  Memory % '
  bold; trailing spaces after {/bold}… title may exceed width → clipped at
  processList content width (box clips at border, no wrap for box? box wrap
  default true → wraps into row 1: but list child overdraws row 1 → harmless.
  Actually title row: content row 0 of processList box; wraps to row 1 if
  longer; list (child) renders rows 0.. over parent's rows but child top = 1 →
  parent row 1 = title wrap row... child items drawn starting row 1 too — draw
  order: parent renders (title incl. wrap), then child list renders over rows
  1.. → any wrap row of title gets overwritten by items. Match by drawing
  items after title.)
- The 'dd' selected-process name: slice(0, processWidth).trim() of the selected
  item content (which may include SGR from selected bg? content = ritems string,
  no SGR — but WAIT: dd uses processListSelection.getItem(selected).content =
  the raw content string incl {bold}? items content = table body row text (no
  tags) → clean. dd = killall "name".
- IMPORTANT vtop bug: 'Count' column header in title vs body: body rows store
  'Count' as NUMBER from stats (count): render '4'... but Count values in body
  via currentRow['Count'] = number → JS concat ' '+4 = ' 4'. Port to string.

## C12. Theme → colors

- themes byte-copied. Load: require(json) → {name, author, title:{fg},
  chart:{fg, border:{type,fg}}, table:{fg, items:{selected:{bg,fg}, item:{fg}},
  border:{type,fg}}, footer:{fg}}.
- sattr style → fg: colors.convert(themeFG): 'fg' string → not #hex, not name →
  match(undefined...) wait: convert('fg'): color='fg' after strip [- ] → 'fg'
  not in colorNames → match('fg') → hex[0] !== '#' → -1 → ...convert returns
  color !== -1 ? color : 0x1ff → hmm match returns -1 then convert's
  `return color !== -1 ? color : 0x1ff` = 0x1ff = DEFAULT. Good: literal 'fg'
  → default → NO SGR emitted for that fg (inherits terminal).
- hex → match() nearest over vcolors (256 palette built from xterm spec:
  0-15 xterm values listed, 16-231 cube r? r*40+55 if nonzero else 0, 232-255
  gray l=g*10+8), colorDistance = (30Δr)² + (59Δg)² + (11Δb)², FIRST index
  wins ties (strict <). Emits 38;5;n / 48;5;n (>15) or 30-37/90-97, 40-47/100-107.
- The 12 themes byte-equal the copies in themes/.

## C13. Runtime shape (Rust)

- Threads: render loop, crossterm input, CPU/clock sampling and fixed-deadline
  memory/process dispatch. Owned workers collect concurrent subprocess output;
  shutdown cancels and reaps outstanding children before joining workers.
  CPU keeps a rolling one-second native snapshot window.
- State: Arc<Mutex<Option<Sample>>> cpu, mem; Arc<Mutex<Option<Vec<ProcRow>>>>.
- Keys: j/k scroll list selection (also up/down? no — vtop binds key 'up'/'down'
  only to disableTableUpdate; the list has keys: true → j/k/up/down handled by
  blessed list's internal key handler (vi + keys). ptop: j=down, k=up, g=top,
  G=bottom, c=sort cpu, m=sort mem, dd=kill, q/Esc/Ctrl-C quit, h/l zoom.
  j/k ALSO set disableTableUpdate for 1000ms (table pauses during scrolling).
- 'c'/'m' → sort change triggers immediate poll of process sensor; select(0)
  after 200ms.
- resize: recompute charts (values preserved), rebuild boxes; childBase/
  childOffset preserved? createBottom on resize re-creates list → selection
  preserved? list re-created: selection state resets... vtop: processListSelection
  is NEW on resize: items re-set, selected=0. Port: reset selection on resize.
  But setupCharts preserves values.
- quit: process.exit(0) — blessed's exit handlers on SIGINT? vtop does
  process.exit directly on key → NO terminal restore?! blessed screen's
  destructor binds on SIGINT? Actually blessed screen() installs process exit
  handlers via screen.on('destroy')... vtop process.exit(0) without destroy →
  blessed 0.1.81 screen sets `process.on('exit')`? No — but the terminal
  would be left raw... vtop quirk: blessed's program._boundProgramStatuses
  handles 'exit' → restores! blessed program.js binds process statuses
  ('exit','SIGINT',...) → program._finalize restores. Ctrl-C: blessed handles
  key event, vtop calls process.exit → blessed's bound exit handler restores
  terminal. Port: restore on exit ALWAYS (we're better: RAII Drop).
- --quit-after sec: setTimeout process.exit(0) — no restore flush? same path.
- --update-interval: parseInt → NaN?? commander passes '300' default; vtop
  parseInt('300ms')=300; parseInt('abc')=NaN → setInterval(fn, NaN) = 1ms
  (Node treats NaN delay as 1ms → busy loop). Reproduce safely: NaN (no
  integer prefix) → interval 1 ms, matching Node timer coercion. Intervals
  outside 1..2147483647 also become 1 ms.

## C14. CLI + errors

- `-V/--version`: print version & exit 0 (commander prints 'x.y.z\n').
- Unknown theme X: `console.log('The theme ' + X + ' does not exist.')` →
  stdout 'The theme 'X' does not exist.' + exit 1. (vtop prints with quotes.)
- help: commander-generated; ptop uses own help text (not parity-critical; keep
  simple, but -h must not crash).
- vtop's commander options: `-t, --theme [name]` default 'parallax';
  `--no-mouse`; `--quit-after [seconds]` default '0'; `--update-interval [ms]`
  default '300'. `ptop` mirrors + adds --vtop-parity, --capture PATH, (fix
  implied default; provide --vtop-parity only; --fix accepted as alias for
  default? spec lists --fix; accept and ignore→default). version flag.
- ptop version: ptop's own semver (0.1.0), printed like commander -V.

## C15. Acceptance (testable, self-imposed)

1. Release build succeeds. Efficiency is measured against the pinned originals
   at matched refresh settings; publish the measured CPU/RSS values, not assumed
   limits from the early implementation specification.
2. 91 fixtures match byte for byte: 90 against the installed original app,
   one against the renderer adapter for native ptop branding. Default
   behavior uses ptop branding without the predecessor URL and upstream sensor
   arithmetic; explicit --vtop-parity restores predecessor branding for checks.
3. q/Esc/Ctrl-C restore terminal (alt off, cursor, cooked).
4. Missing theme message byte-exact: `The theme 'x' does not exist.` stdout, exit 1.
5. clippy -D warnings, fmt, tests green.
6. Real PTY tests cover navigation, mouse, literal kill targets with fake effects,
   sort deadlines, sensor cadence, and updater interaction/restart contracts.

## Live sensor dispatch and shutdown

Memory and process polls start immediately, then dispatch at fixed 200 ms and
2000 ms deadlines independently of subprocess latency, matching upstream
setInterval. Sort changes add a process poll without resetting its periodic
deadline. Slow polls overlap. The real-app cadence proof uses delayed fake ps
commands for both installed vtop and ptop. On exit ptop restores the terminal,
then kills and reaps its own outstanding sensor children through owned Child
handles and joins their workers; it does not leave sensor helpers running.

Sensor command failures retain stdout, stderr and exit status. macOS memory
command errors terminate with status 1 after terminal restoration, as upstream's
throw does. Process and Linux-memory errors print diagnostics and still parse
stdout. Linux memory with a missing second output line terminates instead of
silently hiding the parse failure. Diagnostic stack frames are implementation
identity, not byte-equality claims. External SIGTERM, SIGINT and SIGQUIT wake
the event loop through a self-pipe and restore terminal state before exit 0,
matching blessed. Repeated/burst signals and normal q teardown have owned-PTY
proofs. Sensor commands have owned process groups; cleanup retains the leader
unreaped until both pipes close, so orphaned pipe holders can be canceled
without signaling a reused PID.
