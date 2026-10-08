//! Layout + frame composition, shared by live rendering and --capture
//! (CONTRACTS §C7/C11). One function builds the full blessed-equivalent grid.

use crate::canvas::BrailleCanvas;
use crate::chart::{draw_chart, ChartInput, Values};
use crate::jsnum::js_to_fixed;
use crate::screen::{attr_from_colors, Rect, Screen};
use crate::table::{draw_table, Row};
use crate::tags::DATTR;
pub use crate::tags::DEFAULT_COLOR;
use crate::theme::ResolvedTheme;

/// The brand/header mode + arithmetic mode (CONTRACTS §7).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mode {
    /// bug-for-bug vtop: XOR memory math, 'vtop' branding
    Parity,
    /// corrected math + ptop branding
    Fix,
}

/// Inputs for one composed frame (live state or captured fixture).
pub struct FrameState<'a> {
    pub cols: i64,
    pub rows: i64,
    pub theme: &'a ResolvedTheme,
    pub hostname: &'a str,
    pub brand: &'a str, // "vtop" | "ptop"
    pub clock: &'a str, // "HH:MM:SS " (trailing space)
    pub loadavg: (f64, f64, f64),
    pub cpu_initialized: bool,
    pub cpu_value: f64,
    /// JS `undefined` passed as the current value (uninitialized sensor)
    pub cpu_is_undefined: bool,
    pub cpu_values: &'a mut Values,
    pub cpu_scale: f64,
    pub position: i64,
    pub mem_initialized: bool,
    pub mem_value: f64,
    pub mem_is_undefined: bool,
    pub mem_values: &'a mut Values,
    pub mem_scale: f64,
    pub procs: &'a [Row],
    pub selected: i64,
    /// graph scales (h/l zoom)
    pub graph_scale: f64,
}

pub struct Layout {
    pub graph: Rect,
    pub graph2: Rect,
    pub plist: Rect,
    pub list: Rect,
    pub header_len: i64,
}

/// blessed box geometry (CONTRACTS §C11) — floor for '50%'.
pub fn layout(cols: i64, rows: i64) -> Layout {
    let graph_h = rows / 2;
    let graph = Rect {
        x: 0,
        y: 1,
        w: cols,
        h: graph_h,
    };
    let graph2_w = cols / 2;
    let graph2 = Rect {
        x: 0,
        y: graph_h + 1,
        w: graph2_w,
        h: graph_h - 2,
    };
    let plist = Rect {
        x: graph2_w,
        y: graph_h + 1,
        w: cols - graph2_w,
        h: graph_h - 2,
    };
    let list = Rect {
        x: plist.x + 1,
        y: plist.y + 1,
        w: plist.w - 2,
        h: plist.h - 3,
    };
    Layout {
        graph,
        graph2,
        plist,
        list,
        header_len: 0,
    }
}

/// Compose the full frame. Returns the finished screen (cells only).
pub fn compose(state: &mut FrameState) -> Screen {
    compose_scrolled(state, None, None)
}

pub(crate) fn compose_scrolled(
    state: &mut FrameState,
    scroll: Option<i64>,
    notice: Option<&str>,
) -> Screen {
    let mut lay = layout(state.cols, state.rows);
    let mut scr = Screen::new(state.cols as usize, state.rows as usize);

    draw_header(&mut scr, state, &mut lay, notice);
    draw_footer(&mut scr, state, &lay);
    draw_graph(&mut scr, state, &lay);
    draw_graph2(&mut scr, state, &lay);
    draw_process_list(&mut scr, state, &lay, scroll);
    scr
}

fn draw_header(scr: &mut Screen, state: &FrameState, lay: &mut Layout, notice: Option<&str>) {
    // headerText = ' {bold}vtop{/bold}{white-fg} for HOST ' — trailing open tag
    // means the trailing space IS white-fg... per CONTRACTS §C7 the trailing is
    // white-fg ({/} is not emitted here; no closing for white-fg! app.js:
    // headerText = ` {bold}vtop{/bold}{white-fg} for ${os.hostname()} `)
    let mut text = format!(
        " {{bold}}{brand}{{/bold}}{{white-fg}} for {host} ",
        brand = state.brand,
        host = state.hostname
    );
    // blessed: header TEXT element's own fg = theme.title.fg → untagged chars
    // carry it; tagged chars follow tags (bold bit / white-fg override).
    let base_attr = attr_from_colors(state.theme.title_fg, DEFAULT_COLOR);
    let mut plain_header = format!(" {} for {} ", state.brand, state.hostname);
    if let Some(version) = notice {
        text.push_str(&format!(
            "{{red-bg}} Press 'u' to upgrade to v{version} {{/red-bg}}{{/white-fg}}"
        ));
        plain_header.push_str(&format!(" Press 'u' to upgrade to v{version} "));
    }
    let normalized = crate::content::normalize(&text);
    let width = plain_header.encode_utf16().count();
    let resolved = crate::tags::parse_tags_to_sgr(&normalized);
    let wrapped = crate::wrap::wrap_content(&resolved, width, true);
    let (plain, attrs) = crate::tags::parse_tags_to_attrs_sgr_walk(
        wrapped.first().map(String::as_str).unwrap_or(""),
        base_attr,
    );
    for (i, ch) in plain.chars().enumerate() {
        scr.set_cell(i as i64, 0, ch, attrs.get(i).copied().unwrap_or(base_attr));
    }
    let end_attr = attrs.last().copied().unwrap_or(base_attr);
    for i in plain.chars().count()..width {
        scr.set_cell(i as i64, 0, ' ', end_attr);
    }
    let _ = &mut lay.header_len;

    // date: width 9, right-aligned: blessed right:0 → left = cols - 9
    let clock: Vec<char> = state.clock.chars().collect();
    let x0 = state.cols - 9;
    for (i, ch) in clock.iter().take(9).enumerate() {
        scr.set_cell(x0 + i as i64, 0, *ch, DATTR);
    }

    // loadavg element (CONTRACTS §C7): left = floor(cols/2 - 14) — this can
    // be NEGATIVE on narrow terminals (blessed renders from the clamped col 0
    // without consuming the pre-0 chars); width = cols - left - 1; one-row
    // element → wrapped lines past L0 are clipped.
    let avg = state.loadavg;
    let text = format!(
        "Load Average: {} {} {}",
        js_to_fixed(avg.0, 2),
        js_to_fixed(avg.1, 2),
        js_to_fixed(avg.2, 2)
    );
    let left = state.cols / 2 - 14;
    // blessed: wrap-at = the parent width minus the element's left (col - left,
    // NO -1); the shrink box then re-sizes the element's window to L0's real
    // width (xl = left + mwidth — the WRAPPED first line's length), so past-L0
    // lines and past-L0 cells clip.
    let width = (state.cols - left).max(1) as usize;
    let resolved = crate::tags::parse_tags_to_sgr(&text);
    let wrapped = crate::wrap::wrap_content(&resolved, width, true);
    if let Some(line) = wrapped.first() {
        let (plain, _attrs) = crate::tags::parse_tags_to_attrs_sgr_walk(line, DATTR);
        let xl = left + plain.chars().count() as i64;
        let chars: Vec<char> = plain.chars().collect();
        // blessed: cells x<0 are skipped WITHOUT consuming chars — the text
        // starts (char 0) at col 0.
        let mut ci = 0usize;
        for x in left..xl {
            if x < 0 || x >= state.cols {
                continue;
            }
            if ci >= chars.len() {
                break;
            }
            scr.set_cell(x, 0, chars[ci], DATTR);
            ci += 1;
        }
    }
}

fn draw_footer(scr: &mut Screen, state: &FrameState, _lay: &Layout) {
    let commands = [
        ("dd", "Kill process"),
        ("j", "Down"),
        ("k", "Up"),
        ("g", "Jump to top"),
        ("G", "Jump to bottom"),
        ("c", "Sort by CPU"),
        ("m", "Sort by Mem"),
    ];
    let mut text = String::new();
    for (k, d) in commands {
        text.push_str("  {white-bg}{black-fg}");
        text.push_str(k);
        text.push_str("{/black-fg}{/white-bg} ");
        text.push_str(d);
    }
    if state.brand == "vtop" {
        text.push_str("{|}http://parall.ax/vtop");
    }
    // blessed pipeline: tags → SGR, then wrap (escape-aware, word-break),
    // then per-line _align with pipe-fill. Footer box h = rows - top = 1 →
    // only the first wrapped line is drawn.
    let resolved = crate::tags::parse_tags_to_sgr(&text);
    let wrapped = crate::wrap::wrap_content(&resolved, state.cols.max(0) as usize, true);
    let y = state.rows - 1;
    let footer_attr = attr_from_colors(state.theme.footer_fg, DEFAULT_COLOR);
    for (li, line) in wrapped.iter().take(1).enumerate() {
        let (plain, attrs) = crate::tags::parse_tags_to_attrs_sgr_walk(line, footer_attr);
        let cy = y - (wrapped.len() as i64 - 1 - li as i64);
        let _ = cy;
        for (i, ch) in plain.chars().enumerate() {
            scr.set_cell(
                i as i64,
                y,
                ch,
                attrs.get(i).copied().unwrap_or(footer_attr),
            );
        }
        // cells beyond content: box fill (blessed bch spaces with box attr)
        for x in plain.chars().count() as i64..state.cols {
            scr.set_cell(x, y, ' ', footer_attr);
        }
    }
}

fn draw_graph(scr: &mut Screen, state: &mut FrameState, lay: &Layout) {
    // charts[0]: width/height are PIXEL dims = (graph.width-3)*2 ×
    // (graph.height-2)*4 (CONTRACTS §C0) — computeValue/fill loops/label math
    // all run in pixel space.
    let cw = ((lay.graph.w - 3) * 2).max(0) as f64;
    let chh = ((lay.graph.h - 2) * 4).max(0) as f64;
    let mut canvas = BrailleCanvas::new(cw, chh);
    let mut input = ChartInput {
        canvas: &mut canvas,
        width: cw,
        height: chh,
        scale: state.cpu_scale,
        current_value: state.cpu_value,
        current_is_undefined: state.cpu_is_undefined,
        position: state.position,
        initialized: state.cpu_initialized,
    };
    let content = draw_chart(&mut input, state.cpu_values);
    let box_attr = attr_from_colors(state.theme.chart_fg, DEFAULT_COLOR);
    let border_attr = attr_from_colors(state.theme.chart_border_fg, DEFAULT_COLOR);
    render_box_content(
        scr,
        lay.graph,
        border_attr,
        box_attr,
        " CPU Usage ",
        &content,
        state.theme,
    );
}

fn draw_graph2(scr: &mut Screen, state: &mut FrameState, lay: &Layout) {
    let cw = ((lay.graph2.w - 3) * 2).max(0) as f64;
    let chh = ((lay.graph2.h - 2) * 4).max(0) as f64;
    let mut canvas = BrailleCanvas::new(cw, chh);
    let mut input = ChartInput {
        canvas: &mut canvas,
        width: cw,
        height: chh,
        scale: state.cpu_scale, // vtop has ONE global graph_scale (h/l); mirrors ignore mem_scale
        current_value: state.mem_value,
        current_is_undefined: state.mem_is_undefined,
        position: state.position,
        initialized: state.mem_initialized,
    };
    let content = draw_chart(&mut input, state.mem_values);
    let box_attr = attr_from_colors(state.theme.chart_fg, DEFAULT_COLOR);
    let border_attr = attr_from_colors(state.theme.chart_border_fg, DEFAULT_COLOR);
    render_box_content(
        scr,
        lay.graph2,
        border_attr,
        box_attr,
        " Memory Usage ",
        &content,
        state.theme,
    );
}

/// Render one box: border+label+content lines with blessed attr semantics.
/// Content attr walk: start attr = box dattr; ESC sequences via attrCode rules
/// (ESC[m → full default 0x1ff fields, NOT box attr).
fn render_box_content(
    scr: &mut Screen,
    rect: Rect,
    border_attr: i64,
    box_attr: i64,
    label: &str,
    content: &str,
    _theme: &ResolvedTheme,
) {
    // blessed setContent pipeline: _parseTags (tags→SGR) then _wrapContent
    // (escape-aware wrap + per-line align incl. {|} pipe) — port (wrap.rs).
    let resolved = crate::tags::parse_tags_to_sgr(content);
    // blessed wrap width = this.width - this.iwidth (FULL rect width; the
    // content loop is inset by border separately)
    let wrapped = crate::wrap::wrap_content(&resolved, rect.w.max(1) as usize, true);
    let mut lines: Vec<String> = Vec::with_capacity(wrapped.len());
    let mut line_attrs: Vec<Vec<i64>> = Vec::with_capacity(wrapped.len());
    let mut fill_attr = box_attr;
    for line in &wrapped {
        let (plain, attrs, final_cur) = crate::tags::walk_sgr_attrs3(line, box_attr, box_attr);
        // cells past this line's content carry the stream's final state —
        // NOT the last text attr (a {/bold} at the end resets it).
        fill_attr = final_cur;
        lines.push(plain);
        line_attrs.push(attrs);
    }
    scr.draw_box_fill(
        rect,
        border_attr,
        label,
        &lines,
        &line_attrs,
        box_attr,
        fill_attr,
    );
    if std::env::var("PTOP_DEBUG_LINES").is_ok() {
        eprintln!(
            "BOX {:?} label={:?} lines={} fill={}",
            rect,
            label,
            lines.len(),
            fill_attr
        );
    }
}

/// Like walk_line_attrs_with but for a NOT-yet-parsed tagged line: parse then
/// map: tagged chars absolute; untagged chars → base (element fg).
pub fn walk_tagged_line(line: &str, base: i64) -> (String, Vec<i64>) {
    // blessed-stream semantics: tags → SGR then attrCode walk from the
    // element base (fg/bg persist unless changed/reset) — CONTRACTS §C8.
    let resolved = crate::tags::parse_tags_to_sgr(line);
    crate::tags::parse_tags_to_attrs_sgr_walk(&resolved, base)
}

pub fn walk_line_attrs_with(plain: &str, tag_attrs: &[i64], base: i64) -> Vec<i64> {
    plain
        .chars()
        .enumerate()
        .map(|(i, _)| {
            let ta = tag_attrs.get(i).copied().unwrap_or(DATTR);
            if ta == DATTR {
                base
            } else {
                ta
            }
        })
        .collect()
}

pub fn walk_line_attrs(line: &str, base: i64) -> Vec<i64> {
    let (plain, attrs) = crate::tags::parse_tags(line);
    walk_line_attrs_with(&plain, &attrs, base)
}

fn draw_process_list(scr: &mut Screen, state: &mut FrameState, lay: &Layout, scroll: Option<i64>) {
    // charts[2]: width = processList.width - 3, height = processList.height - 2 (unused)
    let table = draw_table(lay.plist.w - 3, state.procs);
    let border_attr = attr_from_colors(state.theme.table_border_fg, DEFAULT_COLOR);
    let box_attr = attr_from_colors(state.theme.table_fg, DEFAULT_COLOR);

    // content lines: title (tagged ' {bold}...{/bold}' wrapped), then list items
    let mut lines: Vec<String> = Vec::new();
    let mut line_attrs: Vec<Vec<i64>> = Vec::new();
    // blessed plist box: dattr = sattr({fg: table.fg}) — fill/stream attr
    let _plist_fill = attr_from_colors(state.theme.table_fg, DEFAULT_COLOR);

    // title line: table.title has trailing '\n' — strip it, keep single line
    let title_line = table.title.trim_end_matches('\n').to_string();
    let resolved_title = crate::tags::parse_tags_to_sgr(&title_line);
    let (tplain, tattrs) = crate::tags::parse_tags_to_attrs_sgr_walk(&resolved_title, box_attr);
    lines.push(tplain.clone());
    line_attrs.push(tattrs);

    // blessed list scroll (list has no border/scrollbar → iheight = 0):
    // visible = height; childBase = 0 while selected < visible, else
    // selected - visible + 1 (CONTRACTS §C11; verified: 9 rows sel 8 → p2 first)
    let visible = lay.list.h.max(0);
    let child_base = scroll.unwrap_or(if state.selected < visible {
        0
    } else {
        state.selected - visible + 1
    });
    let selected_bg = state.theme.table_selected_bg;
    let item_fg = state.theme.table_item_fg;
    let item_bg = state.theme.table_item_bg;
    let selected_fg = state.theme.table_selected_fg;
    for (i, row) in table
        .body
        .iter()
        .enumerate()
        .skip(child_base.max(0) as usize)
        .take(visible as usize)
    {
        let row_attr = if i as i64 == state.selected {
            let fg = if selected_fg == crate::tags::DEFAULT_COLOR {
                item_fg
            } else {
                selected_fg
            };
            (fg << 9) | selected_bg
        } else {
            (item_fg << 9) | item_bg
        };
        // item content: row text padded/clipped to list width (blessed item box
        // clips; bg fills full width incl. trailing spaces)
        let mut text = String::new();
        let mut attrs = Vec::new();
        let normalized = crate::content::normalize(row);
        let wrapped = crate::wrap::wrap_content(&normalized, lay.list.w.max(0) as usize, true);
        let (plain, parsed_attrs) = crate::tags::parse_tags_to_attrs_sgr_walk(
            wrapped.first().map(String::as_str).unwrap_or(""),
            row_attr,
        );
        for (ch, attr) in plain
            .chars()
            .zip(parsed_attrs)
            .take(lay.list.w.max(0) as usize)
        {
            text.push(ch);
            attrs.push(if i as i64 == state.selected {
                (attr & !(0x1ff << 9)) | (row_attr & (0x1ff << 9))
            } else {
                attr
            });
        }
        let trailing_attr = attrs.last().copied().unwrap_or(row_attr);
        for _ in text.chars().count()..lay.list.w.max(0) as usize {
            text.push(' ');
            attrs.push(trailing_attr);
        }
        lines.push(text);
        line_attrs.push(attrs);
        let _ = child_base;
    }

    // Content row 0 = the title (plist box walk: trailing = its walk-end
    // attr); rows 1+ = the LIST child box (dattr = DATTR — no own style) whose
    // cells paint DATTR (blessed bch fill).

    let items_len = lines.len().saturating_sub(1);
    // walk END attr for the title row (post-{/bold} state); list rows DATTR
    let (_, _, title_end) = crate::tags::walk_sgr_attrs3(
        &crate::tags::parse_tags_to_sgr(&title_line),
        box_attr,
        box_attr,
    );
    scr.draw_box_regions(
        lay.plist,
        border_attr,
        " Process List ",
        &lines,
        &line_attrs,
        box_attr,
        &[(0usize, title_end), (1 + items_len, DATTR)],
    );
    if std::env::var("PTOP_DEBUG_LINES").is_ok() {
        for (li, l) in lines.iter().enumerate() {
            eprintln!(
                "PL[{}] w={} attr0={:?} txt={:?}",
                li,
                l.chars().count(),
                line_attrs[li].first(),
                l.chars().take(24).collect::<String>()
            );
        }
    }
    let _ = (lay.list, child_base, visible);
}

// ───────────────────────── live runtime (M4) ─────────────────────────

use crate::sensors::{parse_ps, ProcGroup, SortKey};
use std::io::Write;
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Debug, PartialEq)]
pub enum Key {
    Quit,
    Down,
    Up,
    Top,
    Bottom,
    SortCpu,
    SortMem,
    ZoomIn,  // h: scale *= 2 (max 8)
    ZoomOut, // l: scale /= 2 (min 0.125)
    KillAndHalfPage,
    DoubleD, // dd kill
    Resize,
    Mouse(crossterm::event::MouseEvent),
    Page(i64),
    HalfPage(i64),
    Viewport(i64),
    Upgrade,
    UpgradeHalfPage,
    Refresh,
    UpdateNotice(Option<String>),
    SensorError(String),
}

pub struct LiveSamples {
    polls: crate::poll::PollJobs,
    pub stopped: std::sync::atomic::AtomicBool,
    pub cpu: Mutex<Option<f64>>,
    pub mem: Mutex<Option<i64>>,
    pub procs: Mutex<Option<Vec<ProcGroup>>>,
    pub clock: Mutex<String>,
    pub loadavg: Mutex<(f64, f64, f64)>,
    pub proc_poll_tx: Option<std::sync::mpsc::Sender<()>>, // wake proc sensor
    pub proc_gen: std::sync::atomic::AtomicU64,
}

/// finalize() into a shared, cheap-to-clone row slice.
fn finalize_arc(groups: &[crate::sensors::ProcGroup], cores: usize, sort: SortKey) -> Arc<[Row]> {
    Arc::from(crate::sensors::finalize(groups, cores, sort))
}

pub struct RunOpts {
    pub args: crate::cli::Args,
}

fn cpu_times_snapshot() -> Option<crate::sensors::CpuTimes> {
    #[cfg(target_os = "macos")]
    {
        mac_cpu_times()
    }
    #[cfg(not(target_os = "macos"))]
    {
        linux_cpu_times()
    }
}

#[cfg(target_os = "macos")]
fn mac_cpu_times() -> Option<crate::sensors::CpuTimes> {
    // os.cpus() on macOS reads mach host_processor_info(PROCESSOR_CPU_LOAD_INFO)
    // (kern.cp_time does not exist on modern macOS). Sum all cores into one
    // tick set the way Node's os.cpus() reports each core's times.
    unsafe {
        let mut num_cpu: libc::natural_t = 0;
        let mut cpu_ticks: libc::processor_info_array_t = std::ptr::null_mut();
        let mut num_ticks: libc::mach_msg_type_number_t = 0;
        #[allow(deprecated)] // libc deprecates in favor of a crate we won't add
        let port = libc::mach_host_self();
        let kr = libc::host_processor_info(
            port,
            libc::PROCESSOR_CPU_LOAD_INFO,
            &mut num_cpu,
            &mut cpu_ticks,
            &mut num_ticks,
        );
        if kr != libc::KERN_SUCCESS as libc::kern_return_t {
            return None;
        }
        // per-core tick order: CPU_STATE_USER, SYSTEM, IDLE, NICE
        const CPU_STATE_MAX: usize = 4;
        let mut user = 0f64;
        let mut nice = 0f64;
        let mut sys = 0f64;
        let mut idle = 0f64;
        let core_count = (num_ticks as usize / CPU_STATE_MAX).min(num_cpu as usize);
        for core in 0..core_count {
            let base = core * CPU_STATE_MAX;
            user += *cpu_ticks.add(base) as f64;
            sys += *cpu_ticks.add(base + 1) as f64;
            idle += *cpu_ticks.add(base + 2) as f64;
            nice += *cpu_ticks.add(base + 3) as f64;
        }
        #[allow(deprecated)]
        let task = libc::mach_task_self();
        libc::vm_deallocate(
            task,
            cpu_ticks as libc::vm_address_t,
            (num_ticks as usize) * std::mem::size_of::<libc::integer_t>(),
        );
        Some(crate::sensors::CpuTimes {
            user,
            nice,
            sys,
            idle,
            irq: 0.0,
        })
    }
}

#[cfg(not(target_os = "macos"))]
fn linux_cpu_times() -> Option<crate::sensors::CpuTimes> {
    // /proc/stat 'cpu ' line: user nice system idle iowait irq softirq …
    // os-utils counts user+nice+sys+idle+irq only (no iowait/steal).
    let s = std::fs::read_to_string("/proc/stat").ok()?;
    let line = s.lines().next()?;
    if !line.starts_with("cpu ") {
        return None;
    }
    let vals: Vec<f64> = line[4..]
        .split_whitespace()
        .filter_map(|x| x.parse().ok())
        .collect();
    if vals.len() < 5 {
        return None;
    }
    Some(crate::sensors::CpuTimes {
        user: vals[0],
        nice: vals[1],
        sys: vals[2],
        idle: vals[3],
        irq: vals.get(5).copied().unwrap_or(0.0),
    })
}

fn core_count() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

fn is_mac() -> bool {
    cfg!(target_os = "macos")
}

/// CPU sensor: overlapping one-second windows sampled every 200 ms.
fn cpu_thread(samples: Arc<LiveSamples>) {
    // vtop starts a one-second measurement every 200 ms. Share endpoints
    // rather than spawning five overlapping timers and reading each twice.
    let mut history = std::collections::VecDeque::with_capacity(6);
    let mut next = std::time::Instant::now();
    while !samples.stopped.load(std::sync::atomic::Ordering::Relaxed) {
        let now = std::time::Instant::now();
        if let Some(end) = cpu_times_snapshot() {
            history.push_back((now, end));
            while history.len() > 1 && now.duration_since(history[0].0) >= Duration::from_secs(1) {
                let (_, start) = history.pop_front().unwrap();
                *samples.cpu.lock().unwrap() =
                    Some((crate::sensors::cpu_percent(start, end) * 100.0).floor());
            }
        }
        next += Duration::from_millis(200);
        std::thread::sleep(next.saturating_duration_since(std::time::Instant::now()));
    }
}

/// mem sensor thread (200ms cadence; CONTRACTS §C6).
fn mem_thread(
    samples: Arc<LiveSamples>,
    parity: bool,
    reference_sensors: bool,
    errors: std::sync::mpsc::Sender<Key>,
) {
    #[cfg(target_os = "macos")]
    let native = !reference_sensors && system_ps_is_selected();
    #[cfg(not(target_os = "macos"))]
    let _ = reference_sensors;
    // vtop's memory sensor uses os-utils totalmem() = os.totalmem()/(1024*1024)
    // (MiB), not Node's byte count — the sensor's own math is (KB-sum / 1024) XOR 2 vs
    // that MiB total (CONTRACTS §C6). hw.memsize(bytes)/1048576 matches exactly.
    let totalmem: f64 = if is_mac() {
        std::process::Command::new("sysctl")
            .arg("-n")
            .arg("hw.memsize")
            .output()
            .ok()
            .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse().ok())
            .unwrap_or(0.0)
            / 1_048_576.0
    } else {
        0.0
    };
    scheduled_polls(samples, Duration::from_millis(200), None, move |samples| {
        #[cfg(target_os = "macos")]
        if native {
            if let Ok(total_kb) = native_memory(&samples) {
                let value = mac_memory_percent(total_kb as f64, totalmem, parity);
                if value.is_finite() {
                    *samples.mem.lock().unwrap() = Some(value as i64);
                }
                return;
            }
            // A partial census must never become an artificially low reading.
            // Retry with the complete original collector, unless shutting down.
            if samples.stopped.load(std::sync::atomic::Ordering::Relaxed) {
                return;
            }
        }
        poll_memory(samples, totalmem, parity, &errors);
    });
}

#[cfg(target_os = "macos")]
fn system_ps_is_selected() -> bool {
    std::env::var_os("PATH").is_some_and(|path| system_ps_on_path(&path))
}

#[cfg(target_os = "macos")]
fn system_ps_on_path(path: &std::ffi::OsStr) -> bool {
    use std::os::unix::fs::PermissionsExt;
    let selected = std::env::split_paths(path)
        .map(|directory| directory.join("ps"))
        .find(|candidate| {
            candidate
                .metadata()
                .is_ok_and(|info| info.is_file() && info.permissions().mode() & 0o111 != 0)
        });
    selected.is_some_and(|candidate| {
        candidate.canonicalize().ok().as_deref() == Some(std::path::Path::new("/bin/ps"))
    })
}

#[cfg(target_os = "macos")]
fn native_memory(samples: &LiveSamples) -> std::io::Result<u64> {
    let snapshot = crate::macos_memory::sample()?;
    if snapshot.denied_pids.is_empty() {
        return Ok(snapshot.native_rss_kib);
    }
    let output = samples
        .polls
        .output(std::process::Command::new("/bin/ps").args(snapshot.fallback_args()?))?;
    if !output.status.success() {
        return Err(std::io::Error::other("targeted memory collector failed"));
    }
    snapshot.total_rss_kib(&output.stdout)
}

fn mac_memory_percent(total_kb: f64, totalmem: f64, parity: bool) -> f64 {
    if parity {
        crate::sensors::memory_mac_parity(total_kb, totalmem)
    } else {
        crate::sensors::memory_mac_fixed(total_kb, totalmem * 1_048_576.0)
    }
}

fn poll_memory(
    samples: Arc<LiveSamples>,
    totalmem: f64,
    parity: bool,
    errors: &std::sync::mpsc::Sender<Key>,
) {
    let (output, error) = if is_mac() {
        sensor_output(&samples, "ps", &["-caxm", "-orss,comm"])
    } else {
        sensor_output(&samples, "free", &["-m"])
    };
    if let Some(error) = error {
        if samples.stopped.load(std::sync::atomic::Ordering::Relaxed) {
            return;
        }
        if is_mac() {
            let _ = errors.send(Key::SensorError(error));
            return;
        }
        sensor_diagnostic(&error);
    }
    let stdout = String::from_utf8_lossy(&output);
    let value = if is_mac() {
        let total_kb = stdout
            .split('\n')
            .filter_map(|line| {
                let end = line
                    .find(|c: char| c.is_ascii_alphabetic())
                    .unwrap_or(line.len());
                let value = crate::sensors::js_parse_int(&line[..end]);
                value.is_finite().then_some(value)
            })
            .sum();
        mac_memory_percent(total_kb, totalmem, parity)
    } else {
        if !stdout.contains('\n') {
            let _ = errors.send(Key::SensorError(
                "TypeError: Cannot read properties of undefined (reading 'replace')".into(),
            ));
            return;
        }
        crate::sensors::memory_linux(&stdout)
    };
    if value.is_finite() {
        *samples.mem.lock().unwrap() = Some(value as i64);
    }
}

fn sensor_diagnostic(error: &str) {
    // Crossterm disables output newline translation; Node's raw mode retains it.
    // Explicit CRLF keeps command diagnostics at the same terminal columns.
    let _ = write!(
        std::io::stderr().lock(),
        "{}\r\n",
        error.replace('\n', "\r\n")
    );
}

fn sensor_output(samples: &LiveSamples, program: &str, args: &[&str]) -> (Vec<u8>, Option<String>) {
    let command = format!("{program} {}", args.join(" "));
    match samples
        .polls
        .output(std::process::Command::new(program).args(args))
    {
        Ok(output) => {
            let error = (!output.status.success()).then(|| format!(
                "Error: Command failed: {command}\n{}\n{{ code: {}, killed: false, signal: null, cmd: '{command}' }}",
                String::from_utf8_lossy(&output.stderr),
                output.status.code().map_or_else(|| "null".into(), |code| code.to_string())
            ));
            (output.stdout, error)
        }
        Err(error) => (
            Vec::new(),
            Some(format!("Error: Command failed: {command}\n{error}")),
        ),
    }
}

/// proc sensor: exact `ps -ewwwo %cpu,%mem,comm` (CONTRACTS §C4), 2000 ms.
fn proc_thread(samples: Arc<LiveSamples>, wake: Receiver<()>) {
    scheduled_polls(
        samples,
        Duration::from_millis(2000),
        Some(wake),
        |samples| {
            let (output, error) = sensor_output(&samples, "ps", &["-ewwwo", "%cpu,%mem,comm"]);
            if samples.stopped.load(std::sync::atomic::Ordering::Relaxed) {
                return;
            }
            if let Some(error) = error {
                sensor_diagnostic(&error);
            }
            let groups = parse_ps(&String::from_utf8_lossy(&output), is_mac());
            *samples.procs.lock().unwrap() = Some(groups);
            samples
                .proc_gen
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        },
    );
}

/// Match setInterval dispatch independently of child completion. Slow polls may
/// overlap; sort requests add a poll without moving the periodic deadline.
fn scheduled_polls(
    samples: Arc<LiveSamples>,
    interval: Duration,
    wake: Option<Receiver<()>>,
    poll: impl Fn(Arc<LiveSamples>) + Send + Sync + 'static,
) {
    let poll = Arc::new(poll);
    let mut next = std::time::Instant::now();
    while !samples.stopped.load(std::sync::atomic::Ordering::Relaxed) {
        let now = std::time::Instant::now();
        if now >= next {
            dispatch_poll(&samples, &poll);
            next += interval;
            if next <= now {
                next = now + interval;
            }
            continue;
        }
        match &wake {
            Some(wake) => match wake.recv_timeout(next.saturating_duration_since(now)) {
                Ok(()) => dispatch_poll(&samples, &poll),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return,
            },
            None => std::thread::sleep(next.saturating_duration_since(now)),
        }
    }
}
fn dispatch_poll(
    samples: &Arc<LiveSamples>,
    poll: &Arc<impl Fn(Arc<LiveSamples>) + Send + Sync + 'static>,
) {
    let worker_samples = Arc::clone(samples);
    let poll = Arc::clone(poll);
    samples.polls.spawn(move || poll(worker_samples));
}

fn clock_thread(samples: Arc<LiveSamples>, tx: std::sync::mpsc::Sender<Key>) {
    let mut next = std::time::Instant::now();
    while !samples.stopped.load(std::sync::atomic::Ordering::Relaxed) {
        let now = std::time::SystemTime::now();
        let secs_since_epoch = now
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        // local time: derive via libc localtime_r
        let (h, m, s) = local_hms(secs_since_epoch);
        *samples.clock.lock().unwrap() = format!("{:02}:{:02}:{:02} ", h, m, s);
        unsafe {
            let mut la = [0f64; 3];
            let n = libc::getloadavg(la.as_mut_ptr(), 3);
            if n == 3 && la.iter().all(|v| v.is_finite()) {
                *samples.loadavg.lock().unwrap() = (la[0], la[1], la[2]);
            } // on failure keep the previous sample
        }
        let _ = tx.send(Key::Refresh);
        next += Duration::from_millis(1000);
        std::thread::sleep(next.saturating_duration_since(std::time::Instant::now()));
    }
}

fn local_hms(epoch_secs: i64) -> (i64, i64, i64) {
    unsafe {
        let t: libc::time_t = epoch_secs as libc::time_t;
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&t, &mut tm);
        (tm.tm_hour as i64, tm.tm_min as i64, tm.tm_sec as i64)
    }
}

/// input thread: crossterm keys → commands (CONTRACTS §C13).
fn input_thread(tx: std::sync::mpsc::Sender<Key>, samples: Arc<LiveSamples>) {
    use crossterm::event::{Event, KeyEvent};
    let mut last_was_d = false;
    let mut errors = 0u32;
    while !samples.stopped.load(std::sync::atomic::Ordering::Relaxed) {
        let event = crossterm::event::poll(Duration::from_millis(100)).and_then(|ready| {
            if ready {
                crossterm::event::read().map(Some)
            } else {
                Ok(None)
            }
        });
        let ev = match event {
            Ok(Some(event)) => event,
            Ok(None) => continue,
            Err(_) => {
                // Poll and read errors share the same consecutive retry budget.
                // A timeout does not reset it; a successful event below does.
                errors += 1;
                if errors > 20 {
                    return;
                }
                std::thread::sleep(Duration::from_millis(200));
                continue;
            }
        };
        errors = 0;
        let key: KeyEvent = match ev {
            Event::Key(k) if k.kind != crossterm::event::KeyEventKind::Release => k,
            Event::Mouse(mouse) => {
                let _ = tx.send(Key::Mouse(mouse));
                continue;
            }
            Event::Resize(_, _) => {
                let _ = tx.send(Key::Resize);
                last_was_d = false;
                continue;
            }
            _ => continue,
        };
        if let Some(command) = decode_key(key, &mut last_was_d) {
            if tx.send(command).is_err() {
                return;
            }
        }
    }
}

fn decode_key(key: crossterm::event::KeyEvent, last_was_d: &mut bool) -> Option<Key> {
    use crossterm::event::{KeyCode, KeyModifiers};
    let previous_d = *last_was_d;
    *last_was_d = matches!(key.code, KeyCode::Char('d' | 'D'));
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('u') => return Some(Key::UpgradeHalfPage),
            KeyCode::Char('d') => {
                return Some(if previous_d {
                    Key::KillAndHalfPage
                } else {
                    Key::HalfPage(1)
                })
            }
            KeyCode::Char('b') => return Some(Key::Page(-1)),
            KeyCode::Char('f') => return Some(Key::Page(1)),
            _ => {}
        }
    }
    match key.code {
        KeyCode::Char('q' | 'Q') => Some(Key::Quit),
        KeyCode::Esc => Some(Key::Quit),
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(Key::Quit),
        KeyCode::Char('j' | 'J') | KeyCode::Down => Some(Key::Down),
        KeyCode::Char('k' | 'K') | KeyCode::Up => Some(Key::Up),
        KeyCode::Char('g') => Some(Key::Top),
        KeyCode::Char('G') => Some(Key::Bottom),
        KeyCode::Char('H') => Some(Key::Viewport(-1)),
        KeyCode::Char('M') => Some(Key::Viewport(0)),
        KeyCode::Char('L') => Some(Key::Viewport(1)),
        KeyCode::Char('u' | 'U') => Some(Key::Upgrade),
        KeyCode::Char('c' | 'C') => Some(Key::SortCpu),
        KeyCode::Char('m') => Some(Key::SortMem),
        KeyCode::Char('h') | KeyCode::Left => Some(Key::ZoomIn),
        KeyCode::Char('l') | KeyCode::Right => Some(Key::ZoomOut),
        KeyCode::Char('d' | 'D') if previous_d => Some(Key::DoubleD),
        _ => None,
    }
}

struct LiveState {
    selected: i64,
    scroll: i64,
    sort: SortKey,
    graph_scale: f64,
    quit: bool,
    fatal_error: Option<String>,
    upgrading: bool,
    upgrade_notice: Option<String>,
    sort_resets: std::collections::VecDeque<std::time::Instant>,
    // process-table memo: finalize() rebuilds+sorts all Rows (≈1.1ms at 585
    // rows); between ps polls (2s) the same groups slice would be re-finalized
    // EVERY draw tick. Cache by (proc generation, sort).
    cached_rows: Arc<[Row]>,
    cached_gen: u64,
    cached_sort: SortKey,
}

impl LiveState {
    fn handle_key(&mut self, k: Key, wake_tx: &std::sync::mpsc::Sender<()>, mouse_enabled: bool) {
        let now = std::time::Instant::now();
        self.settle_timers(now);
        self.handle_key_at(k, wake_tx, mouse_enabled, terminal_size(), now);
    }

    fn handle_key_at(
        &mut self,
        k: Key,
        wake_tx: &std::sync::mpsc::Sender<()>,
        mouse_enabled: bool,
        size: (u16, u16),
        now: std::time::Instant,
    ) {
        let (cols, rows) = size;
        let height = layout(cols as i64, rows as i64).list.h.max(1);
        match k {
            Key::Upgrade => self.upgrading = true,
            Key::UpgradeHalfPage => {
                self.upgrading = true;
                self.selected -= height / 2;
            }
            Key::SensorError(error) => {
                self.fatal_error = Some(error);
                self.quit = true;
            }
            Key::Refresh => {}
            Key::UpdateNotice(notice) => self.upgrade_notice = notice,
            Key::Quit => self.quit = true,
            Key::Down => {
                self.selected += 1;
            }
            Key::Up => {
                self.selected -= 1;
            }
            Key::Top => self.selected = 0,
            Key::Bottom => self.selected = i64::MAX / 2,
            Key::SortCpu => {
                if self.sort != SortKey::Cpu {
                    self.sort = SortKey::Cpu;
                    let _ = wake_tx.send(());
                    self.sort_resets.push_back(now + Duration::from_millis(200));
                }
            }
            Key::SortMem => {
                if self.sort != SortKey::Mem {
                    self.sort = SortKey::Mem;
                    let _ = wake_tx.send(());
                    self.sort_resets.push_back(now + Duration::from_millis(200));
                }
            }
            Key::ZoomIn => {
                if self.graph_scale < 8.0 {
                    self.graph_scale *= 2.0;
                }
            }
            Key::ZoomOut => {
                if self.graph_scale > 0.125 {
                    self.graph_scale /= 2.0;
                }
            }
            Key::KillAndHalfPage => {
                self.handle_key_at(Key::DoubleD, wake_tx, mouse_enabled, size, now);
                self.selected += height / 2;
            }
            Key::DoubleD => {
                // killall "<selected item content sliced to processWidth>
                // .trim()" (app.js dd handler verbatim).
                let (cols_now, _) = terminal_size();
                let plist_w = (cols_now as i64 - cols_now as i64 / 2).max(1);
                let table = crate::table::draw_table(plist_w - 3, &self.cached_rows);
                if let Some(row) = table.body.get(self.selected.max(0) as usize) {
                    let name =
                        crate::content::slice_units(row, table.process_width.max(0) as usize);
                    if !name.trim().is_empty() {
                        // Pass one literal argument; never interpret process names as shell code.
                        std::thread::spawn(move || {
                            let _ = std::process::Command::new("killall")
                                .arg("--")
                                .arg(name.trim())
                                .stdout(std::process::Stdio::null())
                                .stderr(std::process::Stdio::null())
                                .status();
                        });
                    }
                }
            }

            Key::Mouse(mouse) => {
                let list = layout(cols as i64, rows as i64).list;
                let x = mouse.column as i64;
                let y = mouse.row as i64;
                if mouse_enabled
                    && x >= list.x
                    && x < list.x + list.w
                    && y > list.y
                    && y <= list.y + list.h
                {
                    use crossterm::event::{MouseButton, MouseEventKind};
                    match mouse.kind {
                        MouseEventKind::ScrollDown => self.selected += 2,
                        MouseEventKind::ScrollUp => self.selected -= 2,
                        MouseEventKind::Down(MouseButton::Left) => {
                            self.selected = self.scroll + y - list.y - 1;
                        }
                        _ => {}
                    }
                }
            }
            Key::Page(direction) | Key::HalfPage(direction) => {
                self.selected += direction
                    * if matches!(k, Key::HalfPage(_)) {
                        height / 2
                    } else {
                        height
                    };
            }
            Key::Viewport(direction) => {
                let visible = height.min(self.cached_rows.len() as i64);
                self.selected = self.scroll
                    + match direction {
                        -1 => 0,
                        0 => visible / 2,
                        _ => visible,
                    };
                match direction {
                    -1 => self.graph_scale = (self.graph_scale * 2.0).min(8.0),
                    1 => self.graph_scale = (self.graph_scale / 2.0).max(0.125),
                    _ if self.sort != SortKey::Mem => {
                        self.sort = SortKey::Mem;
                        let _ = wake_tx.send(());
                        self.sort_resets.push_back(now + Duration::from_millis(200));
                    }
                    _ => {}
                }
            }
            Key::Resize => {
                // values ARE preserved across resize (app.js setupCharts keeps
                // charts[plugin].values; graphScale is module state, kept);
                // selection resets because the list is re-created (app.js
                // createBottom on resize → fresh processListSelection).
                self.selected = 0;
                self.scroll = 0;
            }
        }
        self.selected = self
            .selected
            .clamp(0, self.cached_rows.len().saturating_sub(1) as i64);
        if !self.upgrading {
            self.scroll = self
                .scroll
                .min(self.selected)
                .max(self.selected - height + 1);
        }
    }

    fn settle_timers(&mut self, now: std::time::Instant) {
        while self
            .sort_resets
            .front()
            .is_some_and(|deadline| now >= *deadline)
        {
            self.selected = 0;
            self.scroll = 0;
            self.sort_resets.pop_front();
        }
    }

    fn wake_deadline(&self, next_draw: std::time::Instant) -> std::time::Instant {
        self.sort_resets
            .front()
            .copied()
            .map_or(next_draw, |deadline| deadline.min(next_draw))
    }
}

/// Live interactive loop. Returns exit code.
pub fn run(opts: RunOpts) -> i32 {
    use crossterm::ExecutableCommand;
    // #9: any thread panic under panic=abort would skip all restore paths —
    // install a hook that restores the terminal BEFORE the abort fires.
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = crossterm::terminal::disable_raw_mode();
        let mut out = std::io::stdout();
        let _ = out.execute(crossterm::event::DisableMouseCapture);
        let _ = out.execute(crossterm::terminal::LeaveAlternateScreen);
        let _ = out.execute(crossterm::cursor::Show);
        hook(info);
    }));
    let mut stdout = std::io::stdout();
    let theme_name = opts.args.theme.clone();
    let th_raw = match crate::theme::load(&theme_name) {
        Ok(t) => t,
        Err(_) => {
            println!("The theme '{}' does not exist.", theme_name);
            return 1;
        }
    };
    let th = Arc::new(crate::theme::resolve(&th_raw));
    let brand = opts.args.brand;
    let parity = opts.args.parity;
    let reference_sensors = opts.args.reference_sensors;
    let interval = opts.args.update_interval.max(1) as u64;

    let samples = Arc::new(LiveSamples {
        polls: Default::default(),
        stopped: std::sync::atomic::AtomicBool::new(false),
        cpu: Mutex::new(None),
        mem: Mutex::new(None),
        procs: Mutex::new(None),
        clock: Mutex::new("00:00:00 ".into()),
        loadavg: Mutex::new((0.0, 0.0, 0.0)),
        proc_poll_tx: None,
        proc_gen: std::sync::atomic::AtomicU64::new(0),
    });
    let mut st = LiveState {
        selected: 0,
        scroll: 0,
        sort: SortKey::Cpu,
        graph_scale: 1.0,
        quit: false,
        fatal_error: None,
        upgrading: false,
        upgrade_notice: None,
        sort_resets: Default::default(),
        cached_rows: Arc::from(Vec::new()),
        cached_gen: 0,
        cached_sort: SortKey::Cpu,
    };

    let (tx, rx) = channel::<Key>();
    let signal_tx = tx.clone();
    let signals = match crate::signals::Guard::install(move || {
        let _ = signal_tx.send(Key::Quit);
    }) {
        Ok(signals) => signals,
        Err(error) => {
            eprintln!("{error}");
            return 1;
        }
    };
    // terminal setup (blessed-equivalent transitions; CONTRACTS §C9)
    let _ = crossterm::terminal::enable_raw_mode();
    let _ = stdout.execute(crossterm::terminal::EnterAlternateScreen);
    let _ = stdout.execute(crossterm::cursor::Hide);
    if opts.args.mouse {
        let _ = stdout.execute(crossterm::event::EnableMouseCapture);
    }

    let (wake_tx, wake_rx) = channel::<()>();
    if opts.args.quit_after != 0 {
        // #8: send Quit through the input channel so the main loop restores
        // the terminal itself (no process::exit from a side thread).
        let txq = tx.clone();
        let secs = opts.args.quit_after;
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(secs.max(0) as u64));
            let _ = txq.send(Key::Quit);
        });
    }
    let samples_in = samples.clone();
    std::thread::spawn(move || cpu_thread(samples_in));
    let samples_in = samples.clone();
    let error_tx = tx.clone();
    std::thread::spawn(move || mem_thread(samples_in, parity, reference_sensors, error_tx));
    let samples_in = samples.clone();
    std::thread::spawn(move || proc_thread(samples_in, wake_rx));
    let samples_in = samples.clone();
    let clock_tx = tx.clone();
    std::thread::spawn(move || clock_thread(samples_in, clock_tx));
    if brand == "vtop" {
        let upgrade_tx = tx.clone();
        std::thread::spawn(move || {
            let _ = upgrade_tx.send(Key::UpdateNotice(crate::upgrade::check()));
        });
    }
    let samples_in = samples.clone();
    let input = std::thread::spawn(move || input_thread(tx, samples_in));

    let hostname = hostname_string();
    let cores = core_count();
    let mut cpu_values = Values::new();
    let mut mem_values = Values::new();
    let mut position: i64 = 0;
    let mut rows_screen = Screen::new(0, 0);
    let mut next_draw = std::time::Instant::now() + Duration::from_millis(interval);
    let mut cpu_now = None;
    let mut mem_now = None;
    let mut rendered_scale = 1.0;

    loop {
        let now = std::time::Instant::now();
        st.settle_timers(now);
        let draw_due = now >= next_draw;
        if draw_due {
            position += 1;
            next_draw = now + Duration::from_millis(interval);
            cpu_now = *samples.cpu.lock().unwrap();
            mem_now = *samples.mem.lock().unwrap();
            rendered_scale = st.graph_scale;
        }
        // drain commands (latest behaviors already ordered)
        while let Ok(k) = rx.try_recv() {
            st.handle_key(k, &wake_tx, opts.args.mouse);
        }
        if st.quit || st.upgrading {
            break;
        }

        let (cols, rows) = terminal_size();
        if cols == 0 || rows == 0 {
            std::thread::sleep(Duration::from_millis(interval));
            continue;
        }

        let cpu_val = cpu_now.unwrap_or(0.0);
        let mem_val = mem_now.unwrap_or(0);
        let cpu_init = cpu_now.is_some();
        let mem_init = mem_now.is_some();
        // values[position] = currentValue — assigned inside draw_chart (C2); the
        // uninitialized case passes NaN + is_undefined so the slot materializes
        // as JS 'undefined' (iterated but renders nothing).
        let clock = samples.clock.lock().unwrap().clone();
        let loadavg = *samples.loadavg.lock().unwrap();
        let gen = samples.proc_gen.load(std::sync::atomic::Ordering::Relaxed);
        let procs_rows: Arc<[Row]> = if !draw_due {
            st.cached_rows.clone()
        } else {
            // finalize() reconstructs+sorts every Row (≈1.1ms for 585 rows);
            // between ps polls the data is identical, so reuse the last build
            // unless the generation or sort key moved.
            let cache_hit = gen == st.cached_gen && st.sort == st.cached_sort;
            if cache_hit {
                st.cached_rows.clone()
            } else {
                st.cached_gen = gen;
                st.cached_sort = st.sort;
                let rows: Arc<[Row]> = samples
                    .procs
                    .lock()
                    .unwrap()
                    .as_ref()
                    .map(|groups| finalize_arc(groups, cores, st.sort))
                    .unwrap_or_default();
                st.cached_rows = rows.clone();
                rows
            }
        };

        st.selected = st
            .selected
            .clamp(0, procs_rows.len().saturating_sub(1) as i64);
        let height = layout(cols as i64, rows as i64).list.h.max(1);
        st.scroll = st.scroll.min(st.selected).max(st.selected - height + 1);
        let mut state = FrameState {
            cols: cols as i64,
            rows: rows as i64,
            theme: &th,
            hostname: &hostname,
            brand,
            clock: &clock,
            loadavg,
            cpu_initialized: cpu_init,
            cpu_value: cpu_val,
            cpu_is_undefined: !cpu_init,
            cpu_values: &mut cpu_values,
            cpu_scale: rendered_scale,
            position,
            mem_initialized: mem_init,
            mem_value: mem_val as f64,
            mem_is_undefined: !mem_init,
            mem_values: &mut mem_values,
            mem_scale: rendered_scale,
            procs: &procs_rows,
            selected: st.selected,
            graph_scale: st.graph_scale,
        };
        // procs_rows is Arc<[Row]> — hand-back to the memo was a refcount bump.
        let mut frame = compose_scrolled(&mut state, Some(st.scroll), st.upgrade_notice.as_deref());
        frame.reset_prev_if_needed(&rows_screen);
        // carry the previous frame's CONTENT as this frame's diff baseline
        // (emit_diff then swaps prev to the new content for the next tick)
        frame.prev_from_cell_snapshot(&rows_screen);
        let bytes = frame.emit_diff();
        rows_screen = frame;
        if !bytes.is_empty() {
            let _ = stdout.write_all(bytes.as_bytes());
            let _ = stdout.flush();
        }

        match rx.recv_timeout(
            st.wake_deadline(next_draw)
                .saturating_duration_since(std::time::Instant::now()),
        ) {
            Ok(k) => st.handle_key(k, &wake_tx, opts.args.mouse),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
        if st.quit || st.upgrading {
            break;
        }
    }

    samples
        .stopped
        .store(true, std::sync::atomic::Ordering::Relaxed);
    let _ = input.join();
    // restore terminal (blessed's exit path: alt off, cursor, cooked)
    let _ = stdout.execute(crossterm::event::DisableMouseCapture);
    let _ = stdout.execute(crossterm::cursor::Show);
    let _ = stdout.execute(crossterm::terminal::LeaveAlternateScreen);
    let _ = crossterm::terminal::disable_raw_mode();
    samples.polls.shutdown();
    drop(signals);
    if let Some(error) = st.fatal_error {
        eprintln!("{error}");
        1
    } else if st.upgrading {
        if brand == "vtop" {
            crate::upgrade::install(&theme_name)
        } else {
            println!(
                "To update ptop, run:\n  npm install -g {}",
                crate::NPM_PACKAGE
            );
            0
        }
    } else {
        0
    }
}

impl Screen {
    fn reset_prev_if_needed(&mut self, old: &Screen) {
        // full redraw only when the size changed (otherwise true diff-emit)
        if old.cols != self.cols || old.rows != self.rows {
            self.reset_prev();
        }
    }
}

fn hostname_string() -> String {
    std::env::var("HOSTNAME").unwrap_or_else(|_| {
        let out = std::process::Command::new("hostname").output();
        match out {
            Ok(o) => String::from_utf8_lossy(&o.stdout).trim().to_string(),
            Err(_) => "localhost".into(),
        }
    })
}

fn terminal_size() -> (u16, u16) {
    crossterm::terminal::size().unwrap_or_default()
}

#[cfg(test)]
mod live_tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    #[test]
    fn consecutive_d_keys_match_vtop_and_other_keys_break_sequence() {
        let mut last = false;
        let d = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE);
        assert_eq!(decode_key(d, &mut last), None);
        assert_eq!(decode_key(d, &mut last), Some(Key::DoubleD));
        assert_eq!(decode_key(d, &mut last), Some(Key::DoubleD));
        decode_key(
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE),
            &mut last,
        );
        assert_eq!(decode_key(d, &mut last), None);
        decode_key(
            KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL),
            &mut last,
        );
        assert_eq!(decode_key(d, &mut last), Some(Key::DoubleD));
    }

    #[test]
    fn navigation_clamps_without_losing_frozen_rows() {
        let rows = crate::sensors::finalize(
            &parse_ps(
                "%CPU %MEM COMM\n9.0 1.0 alpha\n6.0 1.0 beta\n3.0 1.0 gamma\n",
                false,
            ),
            1,
            SortKey::Cpu,
        );
        let mut state = LiveState {
            selected: 0,
            scroll: 0,
            sort: SortKey::Cpu,
            graph_scale: 1.0,
            quit: false,
            fatal_error: None,
            upgrading: false,
            upgrade_notice: None,
            sort_resets: Default::default(),
            cached_rows: Arc::from(rows),
            cached_gen: 1,
            cached_sort: SortKey::Cpu,
        };
        let (tx, _rx) = channel();
        state.handle_key(Key::Bottom, &tx, false);
        assert_eq!(state.selected, 2);
        state.handle_key(Key::Up, &tx, false);
        assert_eq!(state.selected, 1);
        assert_eq!(state.cached_rows.len(), 3);
        state.handle_key(Key::Top, &tx, false);
        state.handle_key(Key::Up, &tx, false);
        assert_eq!(state.selected, 0);
        state.handle_key(Key::Down, &tx, false);
        assert_eq!(state.selected, 1);
    }

    #[test]
    fn each_sort_transition_keeps_its_timer_and_wakes_before_slow_draw() {
        let start = std::time::Instant::now();
        let mut state = LiveState {
            selected: 2,
            scroll: 1,
            sort: SortKey::Cpu,
            graph_scale: 1.0,
            quit: false,
            fatal_error: None,
            upgrading: false,
            upgrade_notice: None,
            sort_resets: Default::default(),
            cached_rows: Arc::from(Vec::new()),
            cached_gen: 0,
            cached_sort: SortKey::Cpu,
        };
        let (tx, _rx) = channel();
        state.handle_key_at(Key::SortMem, &tx, false, (100, 24), start);
        state.handle_key_at(
            Key::SortCpu,
            &tx,
            false,
            (100, 24),
            start + Duration::from_millis(100),
        );
        let slow_draw = start + Duration::from_secs(5);
        assert_eq!(
            state.wake_deadline(slow_draw),
            start + Duration::from_millis(200)
        );
        state.selected = 2;
        state.settle_timers(start + Duration::from_millis(200));
        assert_eq!(state.selected, 0);
        assert_eq!(
            state.wake_deadline(slow_draw),
            start + Duration::from_millis(300)
        );
        state.selected = 1;
        state.settle_timers(start + Duration::from_millis(300));
        assert_eq!(state.selected, 0);
        assert_eq!(state.wake_deadline(slow_draw), slow_draw);
    }

    #[test]
    fn blessed_control_navigation_is_supported() {
        for (ch, expected) in [
            ('u', Key::UpgradeHalfPage),
            ('d', Key::HalfPage(1)),
            ('b', Key::Page(-1)),
            ('f', Key::Page(1)),
        ] {
            assert_eq!(
                decode_key(
                    KeyEvent::new(KeyCode::Char(ch), KeyModifiers::CONTROL),
                    &mut false
                ),
                Some(expected)
            );
        }
    }
    #[test]
    fn keys_and_sort_timer_match_recorded_actual_upstream_app() {
        assert_upstream_trace(include_str!("../tests/upstream-key-trace.json"));
        assert_upstream_trace(include_str!("../tests/upstream-sort-timers.json"));
    }

    fn assert_upstream_trace(input: &str) {
        let fixture: serde_json::Value = serde_json::from_str(input).unwrap();
        let groups = parse_ps("%CPU %MEM COMM\n9.0 1.0 alpha\n", false);
        let row = crate::sensors::finalize(&groups, 1, SortKey::Cpu).remove(0);
        let mut state = LiveState {
            selected: fixture["selected"].as_i64().unwrap_or(0),
            scroll: 0,
            sort: SortKey::Cpu,
            graph_scale: 1.0,
            quit: false,
            fatal_error: None,
            upgrading: false,
            upgrade_notice: None,
            sort_resets: Default::default(),
            cached_rows: Arc::from(vec![row; 30]),
            cached_gen: 1,
            cached_sort: SortKey::Cpu,
        };
        let (tx, _rx) = channel();
        let start = std::time::Instant::now();
        let mut elapsed = 0;
        let mut last = false;
        for (index, event) in fixture["events"].as_array().unwrap().iter().enumerate() {
            if let Some(ms) = event["advance"].as_u64() {
                elapsed += ms;
            }
            let now = start + Duration::from_millis(elapsed);
            state.settle_timers(now);
            if let Some(key) = event.get("key") {
                let name = key["name"].as_str().unwrap();
                let shift = key["shift"].as_bool().unwrap_or(false);
                let code = match name {
                    "home" => KeyCode::Home,
                    "end" => KeyCode::End,
                    "up" => KeyCode::Up,
                    _ => KeyCode::Char(if shift {
                        name.chars().next().unwrap().to_ascii_uppercase()
                    } else {
                        name.chars().next().unwrap()
                    }),
                };
                let mut modifiers = KeyModifiers::NONE;
                if shift {
                    modifiers |= KeyModifiers::SHIFT;
                }
                if key["ctrl"].as_bool().unwrap_or(false) {
                    modifiers |= KeyModifiers::CONTROL;
                }
                if let Some(command) = decode_key(KeyEvent::new(code, modifiers), &mut last) {
                    state.handle_key_at(command, &tx, true, (100, 24), now);
                }
            }
            let expected = &fixture["expected"][index + 1];
            assert_eq!(
                state.selected,
                expected["selected"].as_i64().unwrap(),
                "selection event {index}: {event}"
            );
            assert_eq!(
                state.scroll,
                expected["scroll"].as_i64().unwrap(),
                "scroll event {index}: {event}"
            );
            assert_eq!(
                state.graph_scale,
                expected["graphScale"].as_f64().unwrap(),
                "scale event {index}"
            );
            assert_eq!(
                if state.sort == SortKey::Cpu {
                    "cpu"
                } else {
                    "mem"
                },
                expected["sort"].as_str().unwrap()
            );
        }
        assert_eq!(
            state.upgrading,
            fixture["events"]
                .as_array()
                .unwrap()
                .iter()
                .any(|event| event["key"]["name"] == "u")
        );
    }
}
