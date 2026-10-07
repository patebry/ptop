'use strict'
// Run the original vtop option declaration with its installed commander version.
const fs = require('fs')
const path = require('path')
const vm = require('vm')
const { createRequire } = require('module')
const root = process.env.PTOP_UPSTREAM_VTOP
if (!root) throw new Error('PTOP_UPSTREAM_VTOP must point to installed vtop 0.6.1')
const requireUpstream = createRequire(path.resolve(root, 'app.js'))
const source = fs.readFileSync(path.join(root, 'app.js'), 'utf8')
const declaration = source.match(/  cli\n([\s\S]*?\.parse\(process.argv\))/)
if (!declaration) throw new Error('Cannot locate upstream option declaration')
const cli = new (requireUpstream('commander').Command)()
vm.runInNewContext('cli\n' + declaration[1], {
  cli,
  themes: fs.readdirSync(path.join(root, 'themes')).filter(n => n.endsWith('.json')).sort().map(n => n.slice(0, -5)).join('|'),
  VERSION: requireUpstream('./package.json').version,
  process: { argv: ['node', 'vtop', ...process.argv.slice(2)] }
})
// Capture parse state without running sensors, npm, or the terminal application.
process.stdout.write(JSON.stringify({ theme: cli.theme, mouse: cli.mouse, quitAfter: cli.quitAfter, updateInterval: cli.updateInterval }) + '\n')
