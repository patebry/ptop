'use strict';
// vtop-mirror.js — JS parity mirror for ptop (--vtop-parity --capture fixtures).
//
//   node harness/vtop-mirror.js fixture.json > frame.txt
//
// Ports vtop 0.6.1 app.js VERBATIM (drawChart / drawTable / drawHeader /
// drawFooter / draw / box construction), adapted:
//   * `blessed`     -> ./blessed-core (faithful 0.1.81 subset, CONTRACTS C8/C9)
//   * `drawille`    -> ./drawille (vendored copy of vtop's 1.1.0)
//   * sensors       -> fixture data (CONTRACTS C10 schema)
//   * commander/cli -> fixture file; ONE draw() + ONE screen.render()
//     (createBottom's init-time render is omitted so the single frame starts
//     from a clean all-default olines buffer and emits EVERYTHING).
//
// Output: the byte stream Screen.draw would emit (cup = ESC[y+1;x+1H, SGR
// runs per C9, no BCE/ACS/cuf), plus one final '\n' after the sheet (C10).
// Values: JSON nulls are JS holes ('for..in' skips); the string 'NaN' becomes
// NaN. `parity` is always true in fixtures v1; `brand` swaps header text.

const App = ((() => {
  // Load in required libs
  const Canvas = process.env.PTOP_UPSTREAM_VTOP
    ? require(require.resolve('drawille', { paths: [process.env.PTOP_UPSTREAM_VTOP] }))
    : require('./drawille')
  const blessed = process.env.PTOP_UPSTREAM_VTOP ? require('./upstream-blessed') : require('./blessed-core')
  const fs = require('fs')
  const path = require('path')
  const os = require('os')

  let screen
  const charts = []
  let loadedTheme
  let program
  let screenDims = { cols: 100, rows: 24 }

  let upgradeNotice = false
  let disableTableUpdate = false
  let disableTableUpdateTimeout = setTimeout(() => {}, 0)

  let graphScale = 1

  // Private variables

  /**
   * This is the number of data points drawn
   * @type {Number}
   */
  let position = 0

  const size = {
    pixel: {
      width: 0,
      height: 0
    },
    character: {
      width: 0,
      height: 0
    }
  }

  // @todo: move this into charts array
  // This is an instance of Blessed Box
  let graph

  let graph2
  let processList
  let processListSelection

  // Fixture data stand-in for sensors
  let fixture = null
  let brand = 'vtop'
  let hostnameStr = ''

  const fixtureArray = (values, fallback) => {
    // JSON null -> hole (a real sparse array: for..in skips it). 'NaN' -> NaN.
    const arr = []
    const src = Array.isArray(values) ? values : (fallback || [])
    for (let i = 0; i < src.length; i++) {
      const v = src[i]
      if (v === null || v === undefined) {
        arr.length = i + 1 // extend with a hole
        continue
      }
      if (typeof v === 'string' && v === 'NaN') {
        arr[i] = NaN
        continue
      }
      arr[i] = +v
    }
    return arr
  }

  // Private functions

  /**
   * Draw header
   * @param  {string} left  This is the text to go on the left
   * @param  {string} right This is the text for the right
   * @return {void}
   */
  const drawHeader = () => {
    let headerText
    let headerTextNoTags
    if (upgradeNotice) {
      upgradeNotice = `${upgradeNotice}`
      headerText = ` {bold}${brand}{/bold}{white-fg} for ${hostnameStr} {red-bg} Press 'u' to upgrade to v${upgradeNotice} {/red-bg}{/white-fg}`
      headerTextNoTags = ` ${brand} for ${hostnameStr}  Press 'u' to upgrade to v${upgradeNotice} `
    } else {
      headerText = ` {bold}${brand}{/bold}{white-fg} for ${hostnameStr} `
      headerTextNoTags = ` ${brand} for ${hostnameStr} `
    }

    const header = blessed.Text({
      top: 'top',
      left: 'left',
      width: headerTextNoTags.length,
      height: '1',
      fg: loadedTheme.title.fg,
      content: headerText,
      tags: true
    })
    const date = blessed.Text({
      top: 'top',
      right: 0,
      width: 9,
      height: '1',
      align: 'right',
      content: '',
      tags: true
    })
    const loadAverage = blessed.Text({
      top: 'top',
      height: '1',
      align: 'center',
      content: '',
      tags: true,
      left: Math.floor(program.cols / 2 - (28 / 2))
    })
    screen.append(header)
    screen.append(date)
    screen.append(loadAverage)

    const zeroPad = input => (`0${input}`).slice(-2)

    const updateTime = () => {
      const time = new Date()
      date.setContent(`${zeroPad(time.getHours())}:${zeroPad(time.getMinutes())}:${zeroPad(time.getSeconds())} `)
    }

    const updateLoadAverage = () => {
      const avg = fixture.loadavg
      loadAverage.setContent(`Load Average: ${avg[0].toFixed(2)} ${avg[1].toFixed(2)} ${avg[2].toFixed(2)}`)
    }

    // One frame: fixture-provided clock (verbatim new Date() replaced), then loadavg.
    if (fixture.clock != null) {
      date.setContent(fixture.clock)
    } else {
      updateTime()
    }
    updateLoadAverage()
  }

  /**
   * Draw the footer
   *
   * @todo This appears to break on some viewports
   */
  const drawFooter = () => {
    const commands = {
      'dd': 'Kill process',
      'j': 'Down',
      'k': 'Up',
      'g': 'Jump to top',
      'G': 'Jump to bottom',
      'c': 'Sort by CPU',
      'm': 'Sort by Mem'
    }
    let text = ''
    for (const c in commands) {
      const command = commands[c]
      text += `  {white-bg}{black-fg}${c}{/black-fg}{/white-bg} ${command}`
    }
    if (brand === 'vtop') text += '{|}http://parall.ax/vtop'
    const footerRight = blessed.Box({
      width: '100%',
      top: program.rows - 1,
      tags: true,
      fg: loadedTheme.footer.fg
    })
    footerRight.setContent(text)
    screen.append(footerRight)
  }

  /**
   * Repeats a string
   * @var string The string to repeat
   * @var integer The number of times to repeat
   * @return {string} The repeated chars as a string.
   */
  const stringRepeat = (string, num) => {
    if (num < 0) {
      return ''
    }
    return new Array(num + 1).join(string)
  }

  /**
   * This draws a chart
   * @param  {int} chartKey The key of the chart.
   * @return {string}       The text output to draw.
   */
  const drawChart = chartKey => {
    const chart = charts[chartKey]
    const c = chart.chart
    c.clear()

    if (!charts[chartKey].plugin.initialized) {
      return false
    }

    const dataPointsToKeep = 5000

    charts[chartKey].values[position] = charts[chartKey].plugin.currentValue

    const computeValue = input => chart.height - Math.floor(((chart.height + 1) / 100) * input) - 1

    if (position > dataPointsToKeep) {
      delete charts[chartKey].values[position - dataPointsToKeep]
    }

    for (const pos in charts[chartKey].values) {
      if (graphScale >= 1 || (graphScale < 1 && pos % (1 / graphScale) === 0)) {
        const p = parseInt(pos, 10) + (chart.width - charts[chartKey].values.length)
        // calculated x-value based on graphScale
        const x = (p * graphScale) + ((1 - graphScale) * chart.width)

        // draws top line of chart
        if (p > 1 && computeValue(charts[chartKey].values[pos - 1]) > 0) {
          c.set(x, computeValue(charts[chartKey].values[pos - 1]))
        }

        // Start deleting old data points to improve performance
        // @todo: This is not be the best place to do this

        // fills all area underneath top line
        for (let y = computeValue(charts[chartKey].values[pos - 1]); y < chart.height; y++) {
          if (graphScale > 1 && p > 0 && y > 0) {
            const current = computeValue(charts[chartKey].values[pos - 1])
            const next = computeValue(charts[chartKey].values[pos])
            const diff = (next - current) / graphScale

            // adds columns between data if graph is zoomed in, takes average where data is missing to make smooth curve
            for (let i = 0; i < graphScale; i++) {
              c.set(x + i, y + (diff * i))
              for (let j = y + (diff * i); j < chart.height; j++) {
                c.set(x + i, j)
              }
            }
          } else if (graphScale <= 1) {
            // magic number used to calculate when to draw a value onto the chart
            // @TODO: Remove this?
            // var allowedPValues = (charts[chartKey].values.length - ((graphScale * charts[chartKey].values.length) + 1)) * -1
            c.set(x, y)
          }
        }
      }
    }

    // Add percentage to top right of the chart by splicing it into the braille data
    const textOutput = c.frame().split('\n')
    const percent = `   ${chart.plugin.currentValue}`
    textOutput[0] = `${textOutput[0].slice(0, textOutput[0].length - 4)}{white-fg}${percent.slice(-3)}%{/white-fg}`

    return textOutput.join('\n')
  }

  /**
   * Draws a table.
   * @param  {int} chartKey The key of the chart.
   * @return {string}       The text output to draw.
   */
  const drawTable = chartKey => {
    const chart = charts[chartKey]
    const columnLengths = {}
    // Clone the column array
    const columns = chart.plugin.columns.slice(0)
    columns.reverse()
    let removeColumn = false
    const lastItem = columns[columns.length - 1]

    const minimumWidth = 12
    let padding = 1

    if (chart.width > 50) {
      padding = 2
    }

    if (chart.width > 80) {
      padding = 3
    }
    // Keep trying to reduce the number of columns
    do {
      let totalUsed = 0
      let firstLength = 0
      // var totalColumns = columns.length
      // Allocate space for each column in reverse order
      for (const column in columns) {
        const item = columns[column]
        // NOTE: app.js has a stray `i++` here mutating a module-scope var
        // from a dead loop — verified harmless no-op; not ported (C3).
        // If on the last column (actually first because of array order)
        // then use up all the available space
        if (item === lastItem) {
          columnLengths[item] = chart.width - totalUsed
          firstLength = columnLengths[item]
        } else {
          columnLengths[item] = item.length + padding
        }
        totalUsed += columnLengths[item]
      }
      if (firstLength < minimumWidth && columns.length > 1) {
        totalUsed = 0
        columns.shift()
        removeColumn = true
      } else {
        removeColumn = false
      }
    } while (removeColumn)

    // And back again
    columns.reverse()
    let titleOutput = '{bold}'
    for (const headerColumn in columns) {
      var colText = ` ${columns[headerColumn]}`
      titleOutput += (colText + stringRepeat(' ', columnLengths[columns[headerColumn]] - colText.length))
    }
    titleOutput += '{/bold}' + '\n'

    const bodyOutput = []
    for (const row in chart.plugin.currentValue) {
      const currentRow = chart.plugin.currentValue[row]
      let rowText = ''
      for (const bodyColumn in columns) {
        let colText = ` ${currentRow[columns[bodyColumn]]}`
        rowText += (colText + stringRepeat(' ', columnLengths[columns[bodyColumn]] - colText.length)).slice(0, columnLengths[columns[bodyColumn]])
      }
      bodyOutput.push(rowText)
    }
    return {
      title: titleOutput,
      body: bodyOutput,
      processWidth: columnLengths[columns[0]]
    }
  }

  // This is set to the current items displayed
  let currentItems = []
  let processWidth = 0
  /**
   * Overall draw function, this should poll and draw results of
   * the loaded sensors.
   */
  const draw = () => {
    // Fixture-driven one-shot: `position` is already the index of the newest
    // sample in the fixture arrays (each fixture = the state after vtop's
    // latest draw, so no position++ here; values[position] is materialized).
    const chartKey = 0
    graph.setContent(drawChart(chartKey))
    graph2.setContent(drawChart(chartKey + 1))

    if (!disableTableUpdate) {
      const table = drawTable(chartKey + 2)
      processList.setContent(table.title)

      // If we keep the stat numbers the same immediately, then update them
      // after, the focus will follow. This is a hack.

      const existingStats = {}
      // Slice the start process off, then store the full stat,
      // so we can inject the same stat onto the new order for a brief render
      // cycle.
      for (var stat in currentItems) {
        var thisStat = currentItems[stat]
        existingStats[thisStat.slice(0, table.processWidth)] = thisStat
      }
      processWidth = table.processWidth
      // Smush on to new stats
      const tempStats = []
      for (let stat in table.body) {
        let thisStat = table.body[stat]
        tempStats.push(existingStats[thisStat.slice(0, table.processWidth)])
      }
      // Move cursor position with temp stats
      // processListSelection.setItems(tempStats);

      // Update the numbers
      processListSelection.setItems(table.body)

      // Fixture selection (C10): select AFTER setItems (blessed's setItems
      // reselects by content match / index clamp).
      if (typeof fixture.selected === 'number') {
        processListSelection.select(fixture.selected)
      }

      processListSelection.focus()

      currentItems = table.body
    }

    screen.render()
  }

  // Public function (just the entry point)
  return {

    init (fixtureData) {
      fixture = fixtureData || {}

      if (fixture.parity === false) {
        throw new Error('fixtures v1 always have parity: true')
      }

      // Values: 'NaN' string -> NaN; JSON null -> hole. (C10)
      const cpuValues = fixtureArray(fixture.cpu_values, [])
      const memValues = fixtureArray(fixture.mem_values, [])
      graphScale = fixture.cpu_scale == null ? 1 : +fixture.cpu_scale

      brand = fixture.brand || 'vtop'
      hostnameStr = fixture.hostname == null ? os.hostname() : String(fixture.hostname)

      // The window state after vtop's latest draw: position = last index.
      position = cpuValues.length - 1

      let theme
      if (typeof fixture.theme !== 'undefined') {
        theme = fixture.theme
      } else {
        theme = 'parallax'
      }

      try {
        loadedTheme = require(path.join(__dirname, '..', 'themes', `${theme}.json`))
      } catch (e) {
        console.log(`The theme '${theme}' does not exist.`)
        process.exit(1)
      }

      // Create a screen object.
      screen = new blessed.Screen({
        cols: fixture.cols || 100,
        rows: fixture.rows || 24,
        terminal: 'xterm-256color'
      })
      program = screen.program
      screenDims.cols = program.cols
      screenDims.rows = program.rows

      screen.alloc()

      drawHeader()

      // setInterval(drawHeader, 1000);
      drawFooter()

      graph = blessed.Box({
        top: 1,
        left: 'left',
        width: '100%',
        height: '50%',
        content: '',
        fg: loadedTheme.chart.fg,
        tags: true,
        border: loadedTheme.chart.border
      })

      screen.append(graph)

      let graph2appended = false

      const createBottom = () => {
        if (graph2appended) {
          screen.remove(graph2)
          screen.remove(processList)
        }
        graph2appended = true
        graph2 = blessed.Box({
          top: graph.height + 1,
          left: 'left',
          width: '50%',
          height: graph.height - 2,
          content: '',
          fg: loadedTheme.chart.fg,
          tags: true,
          border: loadedTheme.chart.border
        })
        screen.append(graph2)

        processList = blessed.Box({
          top: graph.height + 1,
          left: '50%',
          width: screen.width - graph2.width,
          height: graph.height - 2,
          keys: true,
          mouse: true,
          fg: loadedTheme.table.fg,
          tags: true,
          border: loadedTheme.table.border
        })
        screen.append(processList)

        processListSelection = blessed.List({
          height: processList.height - 3,
          top: 1,
          width: processList.width - 2,
          left: 0,
          keys: true,
          vi: true,
          search (jump) {
            // @TODO
            // jump('string of thing to jump to');
          },
          style: loadedTheme.table.items,
          mouse: true
        })
        processList.append(processListSelection)
        processListSelection.focus()
        // NOTE: app.js calls screen.render() here; omitted — the mirror emits
        // its single fixture frame from a clean buffer (see module header).
      }

      screen.on('resize', () => {
        createBottom()
      })
      createBottom()

      screen.append(graph)
      screen.append(processList)

      // Render the screen.
      // (app.js renders the empty-layout frame here; omitted for single-frame
      // capture — see module header.)

      const setupCharts = () => {
        size.pixel.width = (graph.width - 2) * 2
        size.pixel.height = (graph.height - 2) * 4

        const plugins = ['cpu', 'memory', 'process']

        for (const plugin in plugins) {
          let width
          let height
          let currentCanvas
          // @todo Refactor this
          switch (plugins[plugin]) {
            case 'cpu':
              width = (graph.width - 3) * 2
              height = (graph.height - 2) * 4
              currentCanvas = new Canvas(width, height)
              break
            case 'memory':
              width = (graph2.width - 3) * 2
              height = ((graph2.height - 2) * 4)
              currentCanvas = new Canvas(width, height)
              break
            case 'process':
              width = processList.width - 3
              height = processList.height - 2
              break
          }

          // Fixture data preserves the recorded values directly (the sensor
          // equivalent of `typeof charts[plugin].values !== 'undefined'`).
          charts[plugin] = {
            chart: currentCanvas,
            values: [],
            plugin: null,
            width,
            height
          }
        }
        // (labels are set after fixture plugins are wired — see below)
      }

      // Sensor stand-ins (fixture data), wired after construction so the
      // charts entries keep app.js's exact shape.
      const pluginData = [
        {
          title: 'CPU Usage',
          interval: 200,
          initialized: fixture.cpu_initialized === true,
          currentValue: (typeof fixture.cpu_value_label === 'string' && fixture.cpu_value_label === 'NaN')
            ? NaN
            : (fixture.cpu_value_label == null ? 0 : +fixture.cpu_value_label),
          columns: undefined
        },
        {
          title: 'Memory Usage',
          interval: 200,
          initialized: fixture.mem_initialized === true,
          currentValue: (typeof fixture.mem_value_label === 'string' && fixture.mem_value_label === 'NaN')
            ? NaN
            : (fixture.mem_value_label == null ? 0 : +fixture.mem_value_label),
          columns: undefined
        },
        {
          title: 'Process List',
          interval: 2000,
          initialized: true,
          currentValue: fixture.procs == null ? [] : fixture.procs,
          columns: ['Command', 'CPU %', 'Count', 'Memory %'],
          sort: fixture.sort || 'cpu'
        }
      ]

      setupCharts()
      charts[0].plugin = pluginData[0]
      charts[1].plugin = pluginData[1]
      charts[2].plugin = pluginData[2]
      charts[0].values = cpuValues
      charts[1].values = memValues

      // @TODO Make this less hard-codey (verbatim app.js, after plugins exist)
      graph.setLabel(` ${charts[0].plugin.title} `)
      graph2.setLabel(` ${charts[1].plugin.title} `)
      processList.setLabel(` ${charts[2].plugin.title} `)

      // The fixture is the state after vtop's last draw() — run its body once.
      draw()

      
      // C10 sheet: rows joined with LF + one trailing newline. blessed's
      // draw() emits cup-positioned runs per row; convert the first-frame
      // stream into bucketed per-row runs so ptop --capture can diff it.
      // C10 sheet: final cells with attr runs per row (fresh prev).
      return screen.sheetRows(fixture.rows || 24)
    }
  }
})())

// ---------------------------------------------------------------------------


// Convert blessed first-frame cup+SGR+text stream into the C10 sheet form.
function streamToSheet(stream, rows) {
  const out = {};
  let i = 0;
  let curY = null;
  // SGR state machine: track pending SGR to stamp when text lands
  while (i < stream.length) {
    if (stream[i] === '\x1b') {
      const cup = /^\x1b\[(\d+);(\d+)H/.exec(stream.slice(i));
      if (cup) {
        curY = parseInt(cup[1], 10) - 1;
        out[curY] = out[curY] || '';
        i += cup[0].length;
        continue;
      }
      const sgr = /^\x1b\[[0-9;]*m/.exec(stream.slice(i));
      if (sgr) {
        if (curY != null) out[curY] = (out[curY] || '') + sgr[0];
        i += sgr[0].length;
        continue;
      }
      i += 1;
      continue;
    }
    if (curY != null) {
      out[curY] = (out[curY] || '') + stream[i];
    }
    i += 1;
  }
  const sheet = [];
  for (let r = 0; r < rows; r++) sheet.push(out[r] || '');
  return sheet.join('\n') + '\n';
}

const fs = require('fs')

function main() {
  const file = process.argv[2]
  if (!file) {
    process.stderr.write('usage: node harness/vtop-mirror.js fixture.json\n')
    process.exit(2)
  }
  const fixture = JSON.parse(fs.readFileSync(file, 'utf8'))
  // charts/state live in the App closure: run once per process — gen.js and
  // the proof loop spawn a fresh node per fixture.
  const bytes = App.init(fixture)
  process.stdout.write(bytes)
}

main()