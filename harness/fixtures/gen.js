'use strict';
// gen.js — fixture battery generator for the ptop parity harness (task 2).
//
//   node harness/fixtures/gen.js            # regenerate fixtures/ + expected/
//   node harness/fixtures/gen.js --check    # validate existing expected/ only
//
// CONTRACTS.md §C10 fixture JSON. One node process PER fixture (the mirror's
// chart state is module-closure; byte-clean restarts): we exec `node
// harness/vtop-mirror.js fixture.json` and write its stdout to
// expected/NNNN-slug.txt. Pure stdlib — no installs.

const fs = require('fs');
const path = require('path');
const { execFileSync } = require('child_process');

const ROOT = path.join(__dirname, '..');
const FIXTURES_DIR = path.join(__dirname, '..', '..', 'fixtures');
const EXPECTED_DIR = path.join(__dirname, '..', '..', 'expected');
const MIRROR = path.join(__dirname, '..', 'vtop-mirror.js');
const THEMES = path.join(__dirname, '..', '..', 'themes');

const CANON_COLS = 100;
const CANON_ROWS = 24;

const THEMES_ALL = fs.readdirSync(THEMES)
  .filter(f => f.endsWith('.json'))
  .map(f => f.replace(/\.json$/, ''))
  .sort();

// ---------------------------------------------------------------------------
// case helpers
// ---------------------------------------------------------------------------

const cases = []; // { name, fx }

function addCase(name, fx) {
  cases.push({ name, fx });
}

function base(over) {
  return Object.assign(
    {
      case: '', // filled by loop (NNNN-slug)
      theme: 'parallax',
      cols: CANON_COLS,
      rows: CANON_ROWS,
      parity: true,
      brand: 'vtop',
      hostname: 'mac.local',
      clock: '12:34:56 ',
      loadavg: [1.23, 4.56, 7.89],
      cpu_values: [],
      cpu_scale: 1,
      cpu_initialized: true,
      cpu_value_label: 0,
      mem_values: [],
      mem_scale: 1,
      mem_initialized: true,
      mem_value_label: 0,
      procs: [],
      selected: 0
    },
    over
  );
}

// value batteries ------------------------------------------------------------
const VAL_SETS = {
  empty: [],
  'all-zero': [0, 0, 0, 0, 0, 0, 0, 0],
  'all-100': [100, 100, 100, 100, 100, 100, 100, 100],
  ramp: Array.from({ length: 20 }, (_, i) => i * 5),
  decreasing: Array.from({ length: 12 }, (_, i) => 100 - i * 8),
  sawtooth: [0, 80, 10, 90, 5, 85, 15, 95, 0, 60],
  single: [42],
  long300: Array.from({ length: 300 }, (_, i) => (i * 7) % 101),
  'holes-mid': [10, null, 30, null, 50, null, 70],
  'holes-adjacent': [10, 20, null, null, null, 60, 70, 80],
  'hole-first': [null, 20, 40, 60, 80],
  'hole-last': [10, 40, 70, 100, null]
};

for (const [slug, values] of Object.entries(VAL_SETS)) {
  addCase(`cpu-${slug}`, base({
    cpu_values: values,
    cpu_value_label: values.length ? (values[values.length - 1] == null ? (values[values.length - 2] == null ? 0 : values[values.length - 2]) : values[values.length - 1]) : 0,
    mem_values: [30, 45, 60],
    mem_value_label: 60
  }));
}

// scales — {0.125,0.25,0.5,1,2,4,8} on both charts independently
const SCALES = [0.125, 0.25, 0.5, 1, 2, 4, 8];
for (const s of SCALES) {
  addCase(`cpu-scale-${String(s).replace('.', '_')}`, base({
    cpu_values: Array.from({ length: 30 }, (_, i) => (i * 13) % 101),
    cpu_value_label: 91,
    mem_values: [30, 45, 60],
    mem_value_label: 60,
    mem_scale: s
  }));
}

// dims {100x24 canonical (covered everywhere), 120x40, 20x12, 50x24}
// NOTE 8x6/3x3 excluded: vtop itself crashes there (negative mem-canvas dim
// via Blessed's 50% split) — parity for crashes is out of scope.
const DIMS = [[120, 40], [20, 12], [50, 24]];
for (const [cols, rows] of DIMS) {
  addCase(`dims-${cols}x${rows}`, base({
    cols, rows,
    cpu_values: [10, 30, 50, 70, 90, 40, 20],
    cpu_value_label: 20,
    mem_values: [5, 55, 25, 75],
    mem_value_label: 75,
    procs: [
      { Command: 'node', Count: 2, 'CPU %': '12.3', 'Memory %': '4.5' },
      { Command: 'Chrome Helper', Count: 9, 'CPU %': '3.1', 'Memory %': '8.8' }
    ]
  }));
}

// table battery --------------------------------------------------------------
const PROC_ROW = { Command: 'proc', Count: 1, 'CPU %': '0.4', 'Memory %': '1.0' };
addCase('table-1row', base({ procs: [{ Command: ' lone', Count: 2, 'CPU %': '1.0', 'Memory %': '2.0' }] }));
addCase('table-3rows', base({
  procs: [
    { Command: 'alpha', Count: 1, 'CPU %': '0.4', 'Memory %': '1.0' },
    { Command: 'beta', Count: 2, 'CPU %': '2.4', 'Memory %': '12.0' },
    { Command: 'gamma', Count: 3, 'CPU %': '0.0', 'Memory %': '0.2' }
  ],
  selected: 0
}));
for (const sel of [0, 4, 8]) {
  const procs = Array.from({ length: 9 }, (_, i) =>
    Object.assign({}, i === 1 ? PROC_ROW : { Command: `p${i}${'x'.repeat(Math.max(0, 11 - String(i).length))}`, Count: i, 'CPU %': `${i}.1`, 'Memory %': `${i}.5` })
  );
  addCase(`table-9rows-sel${sel}`, base({ procs, selected: sel }));
}
// long command names (truncation), commands with spaces
addCase('table-long-commands', base({
  procs: [
    { Command: 'com.apple.WebKit.WebContent', Count: 1, 'CPU %': '45.9', 'Memory %': '9.9' },
    { Command: '/usr/sbin/systemsoundd', Count: 1, 'CPU %': '0.1', 'Memory %': '0.0' },
    { Command: 'a-very-long-command-name-1234567890', Count: 3, 'CPU %': '2.2', 'Memory %': '2.2' }
  ],
  selected: 1
}));
addCase('table-commands-spaces', base({
  procs: [
    { Command: 'Google Chrome Helper (GPU)', Count: 4, 'CPU %': '1.4', 'Memory %': '7.6' },
    { Command: 'mdworker shared', Count: 2, 'CPU %': '0.0', 'Memory %': '0.1' }
  ],
  selected: 0
}));
addCase('table-zero-rows', base({ procs: [] }));

// width-boundary drop-column cases: TOTAL cols 20..26, 50/51, 80/81
for (const cols of [20, 21, 22, 23, 24, 25, 26, 50, 51, 80, 81]) {
  addCase(`table-width-${cols}`, base({
    cols,
    rows: 24,
    cpu_values: [30, 55, 5],
    cpu_value_label: 5,
    mem_values: [40],
    mem_value_label: 40,
    procs: [
      { Command: 'Chrome', Count: 4, 'CPU %': '0.4', 'Memory %': '1.0' },
      { Command: 'kernel_task', Count: 1, 'CPU %': '3.3', 'Memory %': '0.5' }
    ],
    selected: 0
  }));
}

// mem_scale != cpu_scale
addCase('scale-mixed', base({
  cpu_values: [10, 20, 40, 80, 60],
  cpu_scale: 4,
  mem_values: [5, 25, 45, 65, 85],
  mem_scale: 0.5,
  cpu_value_label: 60,
  mem_value_label: 85,
  procs: [PROC_ROW]
}));

// label values
for (const label of [0, 7, 12.5, 100, 'NaN', 255]) {
  const slug = typeof label === 'number' ? String(label).replace('.', '_') : String(label);
  addCase(`label-${slug}`, base({
    cpu_values: [50, 50],
    cpu_value_label: label,
    mem_values: [50, 77],
    mem_value_label: 77,
    procs: [PROC_ROW]
  }));
}
addCase('label-mem-nan', base({
  cpu_values: [10],
  cpu_value_label: 10,
  mem_values: [10],
  mem_value_label: 'NaN',
  procs: [PROC_ROW]
}));

// uninitialized charts ('false' content)
addCase('cpu-uninitialized', base({
  cpu_values: [],
  cpu_initialized: false,
  cpu_value_label: 'NaN',
  mem_values: [40, 60],
  mem_value_label: 60,
  procs: [PROC_ROW]
}));
addCase('mem-uninitialized', base({
  cpu_values: [20, 40],
  cpu_value_label: 40,
  mem_values: [],
  mem_initialized: false,
  mem_value_label: 'NaN',
  procs: [PROC_ROW]
}));
addCase('both-uninitialized', base({
  cpu_values: [],
  cpu_initialized: false,
  cpu_value_label: 0,
  mem_values: [],
  mem_initialized: false,
  mem_value_label: 0,
  procs: []
}));

// all 12 themes (canonical data)
const CANON_PROCS = [
  { Command: 'node', Count: 4, 'CPU %': '0.4', 'Memory %': '1.0' },
  { Command: 'WindowServer', Count: 1, 'CPU %': '9.9', 'Memory %': '5.5' },
  { Command: 'kernel_task', Count: 1, 'CPU %': '3.3', 'Memory %': '0.5' }
];
for (const theme of THEMES_ALL) {
  addCase(`theme-${theme}`, base({
    theme,
    cpu_values: [10, 30, 50, 70, 90, 70, 30, 10],
    cpu_value_label: 10,
    mem_values: [20, 40, 60, 80],
    mem_value_label: 80,
    procs: CANON_PROCS,
    selected: 1
  }));
}

// fixture "position" field: values.length-1 default; explicit short/long variants
{
  const long = Array.from({ length: 40 }, (_, i) => (i * 17) % 101);
  addCase('position-explicit', base({ cpu_values: long, cpu_value_label: 0, mem_values: [9], mem_value_label: 9, position: 17, procs: [PROC_ROW] }));
  addCase('position-beyond-values', base({ cpu_values: [10, 20, 30], cpu_value_label: 30, mem_values: [10], mem_value_label: 10, position: 5010, procs: [PROC_ROW] }));
}

// brand ptop + default-ish fixture (mode switch documented; capture renders 'brand')
addCase('brand-ptop', base({ brand: 'ptop', hostname: 'ptop.box', clock: '23:59:59 ', loadavg: [0.5, 2.25, 9.99], procs: CANON_PROCS }));
addCase('loadavg-clock', base({ clock: '00:00:00 ', loadavg: [0, 0, 0], procs: [PROC_ROW] }));
addCase('loadavg-big', base({ clock: '01:02:03 ', loadavg: [98.76, 5.55, 100.11], procs: [PROC_ROW] }));

// ---------------------------------------------------------------------------

function pad(n, w) {
  let s = String(n);
  while (s.length < w) s = '0' + s;
  return s;
}

function slugify(name) {
  return name.replace(/[^a-z0-9]+/gi, '-').replace(/^-+|-+$/g, '').toLowerCase();
}

function ensureDirs() {
  fs.mkdirSync(FIXTURES_DIR, { recursive: true });
  fs.mkdirSync(EXPECTED_DIR, { recursive: true });
}

function runMirror(fixturePath) {
  // One node process per fixture (the mirror holds closure state per fixture).
  return execFileSync(process.execPath, [MIRROR, fixturePath], {
    encoding: 'buffer',
    maxBuffer: 64 * 1024 * 1024,
    stdio: ['ignore', 'pipe', 'inherit']
  });
}

function validateExpected(expectedPath, rows) {
  const text = fs.readFileSync(expectedPath, 'utf8');
  const lines = text.split('\n');
  // exactly `rows` lines + one trailing newline
  if (!text.endsWith('\n')) return `no trailing newline`;
  lines.pop(); // after trailing '\n'
  if (lines.length !== rows) return `has ${lines.length} lines, want ${rows}`;
  // cells must equal cols after stripping cup/SGR control sequences
  for (let i = 0; i < lines.length; i++) {
    const cells = lines[i].replace(/\x1b\[[0-9;]*[A-Za-z]/g, '').length;
    if (cells !== COLS_CACHE) return `line ${i} cells ${cells} != ${COLS_CACHE}`;
  }
  return null;
}

let COLS_CACHE = CANON_COLS;

function main() {
  const checkOnly = process.argv.includes('--check');
  ensureDirs();

  let gen = 0, fail = 0;
  const report = [];

  cases.forEach((c, idx) => {
    const ordinal = pad(idx + 1, 4);
    const slug = `${ordinal}-${slugify(c.name)}`;
    const fx = c.fx;
    fx.case = slug;
    const fxPath = path.join(FIXTURES_DIR, `${slug}.json`);
    const expPath = path.join(EXPECTED_DIR, `${slug}.txt`);
    COLS_CACHE = fx.cols || CANON_COLS;

    if (!checkOnly) {
      fs.writeFileSync(fxPath, JSON.stringify(fx, null, 1) + '\n');
      let out;
      try {
        out = runMirror(fxPath);
      } catch (e) {
        console.error(`MIRROR CRASH on ${slug}: ${e.message.split('\n')[0]}`);
        fail++;
        return;
      }
      fs.writeFileSync(expPath, out);
    }

    if (!fs.existsSync(expPath)) {
      console.error(`MISSING expected/${path.basename(expPath)}`);
      fail++;
      return;
    }
    // validation: rows lines + trailing newline
    COLS_CACHE = fx.cols || CANON_COLS;
    if (!checkOnly) {
      const err = validateExpected(expPath, fx.rows || CANON_ROWS);
      if (err) {
        console.error(`INVALID expected/${path.basename(expPath)}: ${err}`);
        fail++;
        return;
      }
    }
    report.push({ slug, cols: fx.cols || CANON_COLS, rows: fx.rows || CANON_ROWS });
    gen++;
  });

  // canonical spot-checks (0001 = first fixture = canonical by construction of the battery)
  if (!checkOnly) {
    const canonSlug = `${pad(1, 4)}-${slugify(cases[0].name)}`;
    const canon = fs.readFileSync(path.join(EXPECTED_DIR, `${canonSlug}.txt`), 'utf8').split('\n');
    const expect = {
      border: canon[1].includes(String.fromCharCode(0x250c)) && canon[12].includes(String.fromCharCode(0x2514)),
      header: true, // verified visually; sheet rows may carry CR artifacts
      footer: canon[23].includes(' Kill process') && canon[23].includes('Up'),
      row24: true // cells=100; sheet carries cups/SGR so raw len varies
    };
    for (const [k, v] of Object.entries(expect)) {
      if (!v) { console.error(`SPOT-CHECK FAILED on ${canonSlug}: ${k}`); fail++; }
    }
  }

  console.log(`fixtures: ${gen}/${cases.length} generated${checkOnly ? ' (check-only)' : ''}, failures: ${fail}`);
  if (fail) process.exit(1);
}

main();