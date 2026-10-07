import { test } from 'node:test';
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, realpathSync, statSync, copyFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { createHash } from 'node:crypto';

const run = (cmd, args, cwd) => execFileSync(cmd, args, { cwd, encoding: 'utf8', timeout: 180_000 });
const hash = path => createHash('sha256').update(readFileSync(path)).digest('hex');
test('npm tarball installs an unchanged standalone native command', () => {
  const root = resolve('.');
  const temp = mkdtempSync(join(tmpdir(), 'ptop-package-'));
  try {
    const pkg = JSON.parse(readFileSync('package.json', 'utf8'));
    const [packed] = JSON.parse(run('npm', ['pack', '--json', '--pack-destination', temp], root));
    const paths = packed.files.map(file => file.path);
    for (const path of paths) {
      assert.ok(['package.json', 'README.md', 'LICENSE', 'bin/ptop', 'docs/assets/ptop.png', 'docs/BENCHMARKING.md', 'docs/PERFORMANCE-2026-10-06.md', 'docs/benchmark-final-2026-10-06.jsonl', 'docs/PERFORMANCE-NPM-0.1.0.md', 'docs/benchmark-npm-0.1.0-2026-10-06.jsonl'].includes(path) || path.startsWith('licenses/'), path);
    }
    assert.ok(paths.includes('bin/ptop'));
    assert.ok(paths.includes('docs/assets/ptop.png'));
    assert.ok(paths.includes('licenses/vtop.txt'));
    assert.ok(paths.includes('licenses/rust-dependencies.txt'));
    const tarball = join(temp, packed.filename);
    // Inspect the real archive, not only npm's manifest summary.
    const archivePaths = run('tar', ['-tzf', tarball], temp).trim().split('\n');
    assert.deepEqual(archivePaths.sort(), paths.map(path => `package/${path}`).sort());
    const prefix = join(temp, 'prefix');
    run('npm', ['install', '--global', '--prefix', prefix, '--ignore-scripts', '--no-audit', '--no-fund', tarball], temp);
    const bin = join(prefix, 'bin', 'ptop');
    assert.equal(hash(realpathSync(bin)), hash(join(root, 'bin/ptop')));
    assert.ok(statSync(bin).mode & 0o111);
    assert.equal(run(bin, ['--version'], temp).trim(), pkg.version);
    assert.match(run(bin, ['--help'], temp), /^Usage: ptop/);
    const fixture = join(root, 'fixtures/0067-brand-ptop.json');
    assert.equal(run(bin, ['--capture', fixture], temp), readFileSync(join(root, 'expected/0067-brand-ptop.txt'), 'utf8'));
    run('python3', [join(root, 'harness/live-smoke.py'), bin], temp);
    // Deny access to the checkout to detect accidental runtime asset dependencies.
    const isolatedFixture = join(temp, 'frame.json');
    copyFileSync(fixture, isolatedFixture);
    const isolatedFrame = run('/usr/bin/sandbox-exec', ['-p', `(version 1)(allow default)(deny file-read* (subpath ${JSON.stringify(root)}))`, bin, '--capture', isolatedFixture], temp);
    assert.equal(isolatedFrame, readFileSync(join(root, 'expected/0067-brand-ptop.txt'), 'utf8'));
  } finally {
    rmSync(temp, { recursive: true, force: true });
  }
});
