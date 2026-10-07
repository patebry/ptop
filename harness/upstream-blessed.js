'use strict'
// Use the installed upstream renderer; only adapt its I/O and capture encoding.
const path = require('path')
const { createRequire } = require('module')
const { PassThrough, Writable } = require('stream')
const upstreamRequire = createRequire(path.resolve(process.env.PTOP_UPSTREAM_VTOP, 'app.js'))
const blessed = upstreamRequire('blessed')
const exportsAdapter = { ...blessed }
exportsAdapter.Screen = function (options) {
  const input = new PassThrough()
  input.isTTY = true
  input.setRawMode = () => {}
  const output = new Writable({ write(chunk, encoding, done) { done() } })
  output.isTTY = true
  output.columns = options.cols
  output.rows = options.rows
  const screen = blessed.screen({ ...options, input, output, warnings: false, fullUnicode: false })
  screen.sheetRows = function (rows) {
    // Normalize the real final cell buffer to the repository's capture wire format.
    const lines = []
    for (let y = 0; y < rows; y++) {
      let result = `\x1b[${y + 1};1H`
      let previous = this.dattr
      for (let x = 0; x < this.lines[y].length;) {
        const attr = this.lines[y][x][0]
        if (x) result += `\x1b[${y + 1};${x + 1}H`
        if (attr !== previous) {
          if (previous !== this.dattr) result += '\x1b[m'
          if (attr !== this.dattr) result += this.codeAttr(attr)
        }
        previous = attr
        do { result += this.lines[y][x++][1] } while (x < this.lines[y].length && this.lines[y][x][0] === attr)
      }
      lines.push(result)
    }
    this.destroy()
    input.destroy()
    return lines.join('\n') + '\n'
  }
  return screen
}
module.exports = exportsAdapter
