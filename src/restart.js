// Restore the updater's observable process state before loading the new package.
// The JavaScript process is needed only for the vtop package explicitly installed by u.
const state = JSON.parse(process.argv[process.argv.length - 1]);
process.argv = [process.execPath, ...state.argv];
process[0] = { theme: state.theme };
let load = require;
if (typeof state.module === 'string' && !require('path').isAbsolute(state.module)) {
  // CommonJS resolves relative and bare requests at upgrade.js, not the caller's cwd.
  // Discover the same npm global prefix used by the explicit install action.
  const root = require('child_process').execFileSync('npm', ['root', '-g'], {
    encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe']
  }).trim();
  load = require('module').createRequire(require('path').join(root, 'vtop', 'upgrade.js'));
}
load(state.undefined ? undefined : state.module);
