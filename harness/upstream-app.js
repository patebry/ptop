'use strict'
// Execute vtop 0.6.1 itself. Only inject I/O, sensors/time and a state-access hook;
// layout, chart, table, key and application timer algorithms remain upstream code.
const fs = require('fs')
const path = require('path')
const vm = require('vm')
const { createRequire } = require('module')
const root = process.env.PTOP_UPSTREAM_VTOP
if (!root) throw new Error('PTOP_UPSTREAM_VTOP is required')
const requireUpstream = createRequire(path.resolve(root, 'app.js'))
if (requireUpstream('./package.json').version !== '0.6.1') throw new Error('Oracle requires vtop 0.6.1')
const fixture = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'))
const adapter = require('./upstream-blessed')
const screen = new adapter.Screen({ cols: fixture.cols || 100, rows: fixture.rows || 24, terminal: 'xterm-256color' })
const blessed = { ...adapter, screen: () => screen, program: () => screen.program }
let now = 0
let timerId = 0
const timers = new Map()
const commands = []
function timer(callback, delay, repeat) {
  const id = ++timerId
  timers.set(id, { callback, due: now + Math.max(1, Number(delay) || 1), repeat })
  return id
}
function advance(milliseconds) {
  const end = now + milliseconds
  for (;;) {
    const next = [...timers].filter(([,t]) => t.due <= end).sort((a,b) => a[1].due-b[1].due || a[0]-b[0])[0]
    if (!next) break
    const [id,t] = next
    now = t.due
    if (t.repeat) t.due += t.repeat
    else timers.delete(id)
    t.callback()
  }
  now = end
}
const plugins = {
  cpu: { title:'CPU Usage', interval:200, initialized:fixture.cpu_initialized === true, currentValue:fixture.cpu_value_label || 0, poll() {} },
  memory: { title:'Memory Usage', interval:200, initialized:fixture.mem_initialized === true, currentValue:fixture.mem_value_label || 0, poll() {} },
  process: { title:'Process List', interval:2000, initialized:true, currentValue:fixture.procs || [], columns:['Command','CPU %','Count','Memory %'], sort:'cpu', poll() {} }
}
const clock = (fixture.clock || '12:34:56 ').trim().split(':').map(Number)
class FixedDate extends Date {
  getHours() { return clock[0] }
  getMinutes() { return clock[1] }
  getSeconds() { return clock[2] }
}
const source = fs.readFileSync(path.join(root, 'app.js'), 'utf8')
const hook = `  return {
    oracle(f) {
      const sparse = values => { const a = values.slice(); for(let i=0;i<a.length;i++) { if(a[i] === null) delete a[i]; else if(a[i] === 'NaN') a[i] = NaN; } return a; };
      charts[0].values = sparse(f.cpu_values || []);
      charts[1].values = sparse(f.mem_values || []);
      position = charts[0].values.length - 2;
      graphScale = f.cpu_scale || 1;
      draw();
      processListSelection.select(f.selected || 0);
      screen.render();
    },
    state() { return {position, graphScale, selected:processListSelection.selected, scroll:processListSelection.childBase, sort:charts[2].plugin.sort, disableTableUpdate, upgradeNotice}; },
    init () {`
if (!source.includes('  return {\n\n    init () {')) throw new Error('Upstream hook location changed')
const context = {
  __dirname: root, console: { log: (...args) => commands.push(['log', ...args]) }, Date: FixedDate,
  setTimeout: (fn, delay) => timer(fn, delay, 0), clearTimeout: id => timers.delete(id),
  setInterval: (fn, delay) => timer(fn, delay, delay), clearInterval: id => timers.delete(id),
  process: { argv:['node','vtop','--theme',fixture.theme || 'parallax', '--update-interval', String(fixture.update_interval || 300)], exit: code => { throw new Error('upstream exit ' + code) } },
  require: name => {
    if (name === 'blessed') return blessed
    if (name === 'commander') return new (requireUpstream('commander').Command)()
    if (name === 'os') return { hostname: () => fixture.hostname || 'mac.local', loadavg: () => fixture.loadavg || [1,2,3] }
    if (name === './upgrade.js') return { check: callback => callback(fixture.upgrade_notice || false), install: (...args) => commands.push(['install', ...args]) }
    if (name === 'child_process') return { exec: (command, callback) => { commands.push(['exec',command]); if(callback) callback(null,'','') } }
    if (name.startsWith('./sensors/')) return plugins[path.basename(name, '.js')]
    return requireUpstream(name)
  }
}
vm.runInNewContext(source.replace('  return {\n\n    init () {', hook).replace('App.init()', 'App.init(); globalThis.oracleApp = App'), context, {filename:path.join(root,'app.js')})
context.oracleApp.oracle(fixture)
const states = [context.oracleApp.state()]
for (const event of fixture.events || []) {
  if (event.advance !== undefined) advance(event.advance)
  else if (event.key) screen.program.emit('keypress', event.ch || '', event.key)
  states.push(context.oracleApp.state())
}
if (process.argv.includes('--state')) {
  screen.destroy()
  process.stdout.write(JSON.stringify({states,commands}) + '\n')
} else process.stdout.write(screen.sheetRows(fixture.rows || 24))
