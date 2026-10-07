import { mkdirSync, readFileSync, copyFileSync, chmodSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { resolve } from 'node:path';

const root = fileURLToPath(new URL('../', import.meta.url));
const run = (command, args) => execFileSync(command, args, { cwd: root, encoding: 'utf8' }).trim();
if (process.platform !== 'darwin' || process.arch !== 'arm64') {
  throw new Error('This release packages macOS Apple Silicon only; build on darwin/arm64.');
}
const pkg = JSON.parse(readFileSync(resolve(root, 'package.json'), 'utf8'));
const metadata = JSON.parse(run('cargo', ['metadata', '--locked', '--no-deps', '--format-version', '1']));
const crate = metadata.packages.find(p => p.name === 'ptop');
if (crate?.version !== pkg.version) throw new Error('Cargo and npm versions must match.');
const packageConstant = readFileSync(resolve(root, 'src/lib.rs'), 'utf8');
if (!packageConstant.includes(`NPM_PACKAGE: &str = "${pkg.name}"`)) {
  throw new Error('The native update instructions must match the npm package name.');
}
if (!run('rustc', ['--version']).startsWith('rustc 1.92.0 ')) {
  throw new Error('Use Rust 1.92.0, matching the bundled standard-library notices.');
}
execFileSync('cargo', ['build', '--locked', '--release', '--target', 'aarch64-apple-darwin', '--target-dir', resolve(root, 'target')], {
  cwd: root, stdio: 'inherit', env: { ...process.env, MACOSX_DEPLOYMENT_TARGET: '15.0' }
});
const binary = resolve(root, 'target/aarch64-apple-darwin/release/ptop');
const header = readFileSync(binary);
if (header.readUInt32LE(0) !== 0xfeedfacf || header.readUInt32LE(4) !== 0x0100000c) {
  throw new Error('Expected a native 64-bit ARM Mach-O executable.');
}
if (run(binary, ['--version']) !== pkg.version) throw new Error('Native binary version mismatch.');
mkdirSync(resolve(root, 'bin'), { recursive: true });
copyFileSync(binary, resolve(root, 'bin/ptop'));
chmodSync(resolve(root, 'bin/ptop'), 0o755);
