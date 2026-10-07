//! Blessed-equivalent cell grid + emission (CONTRACTS §C9/C10).
//!
//! Cells hold (char, attr) with blessed's attr bitfield: flags<<18 | fg<<9 | bg,
//! 9-bit fields, 0x1ff = "default/inherit". Emission per blessed Screen.draw:
//! cup(y,x) then attr-diff SGR runs (bg BEFORE fg), ESC[m reset from non-default.

use crate::tags::{attr_sgr_body, DATTR, DEFAULT_COLOR};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cell {
    pub ch: char,
    pub attr: i64,
}

impl Default for Cell {
    fn default() -> Self {
        Cell {
            ch: ' ',
            attr: DATTR,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i64,
    pub y: i64,
    pub w: i64,
    pub h: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmitMode {
    /// Live diff streams: BCE-style trailing erase allowed (matches blessed).
    Live,
    /// C10 capture sheets: every cell printed verbatim (matches the mirror).
    Capture,
}

#[derive(Clone)]
pub struct Screen {
    pub cols: usize,
    pub rows: usize,
    pub cells: Vec<Cell>,
    prev: Vec<Cell>,
    emit_mode: EmitMode,
}

impl Screen {
    pub fn new(cols: usize, rows: usize) -> Self {
        Screen::with_mode(cols, rows, EmitMode::Live)
    }

    pub fn with_mode(cols: usize, rows: usize, emit_mode: EmitMode) -> Self {
        Screen {
            cols,
            rows,
            cells: vec![Cell::default(); cols * rows],
            prev: vec![Cell::default(); cols * rows],
            emit_mode,
        }
    }

    pub fn clear(&mut self) {
        self.cells.fill(Cell::default());
    }

    pub fn set_cell(&mut self, x: i64, y: i64, ch: char, attr: i64) {
        if x < 0 || y < 0 || x >= self.cols as i64 || y >= self.rows as i64 {
            return;
        }
        self.cells[y as usize * self.cols + x as usize] = Cell { ch, attr };
    }

    pub fn fill_region(&mut self, attr: i64, ch: char, x0: i64, x1: i64, y0: i64, y1: i64) {
        for y in y0..y1 {
            for x in x0..x1 {
                self.set_cell(x, y, ch, attr);
            }
        }
    }

    /// blessed box render: content + border inside the box rect + label.
    /// content_lines: each line is (chars, per-char attrs) pairs already
    /// attr-final EXCEPT base-attr chars which carry `base_attr`.
    ///
    /// Draw a box whose post-content fill attr varies by content row: `fills`
    /// = [(from_content_row, fill_attr), ...] — later entries override.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_box_regions(
        &mut self,
        rect: Rect,
        border_attr: i64,
        label: &str,
        content_lines: &[String],
        content_line_attrs: &[Vec<i64>],
        content_base_attr: i64,
        fills: &[(usize, i64)],
    ) {
        let xi = rect.x;
        let xl = rect.x + rect.w;
        let yi = rect.y;
        let yl = rect.y + rect.h;
        let cxi = xi + 1;
        let cyi = yi + 1;
        let cxl = xl - 1;
        let cyl = yl - 1;
        let fill_for = |row: usize| -> i64 {
            let mut f = content_base_attr;
            for &(from, attr) in fills {
                if row >= from {
                    f = attr;
                }
            }
            f
        };
        for (li, line) in content_lines.iter().enumerate() {
            let cy = cyi + li as i64;
            if cy >= cyl {
                break;
            }
            let attrs = content_line_attrs.get(li);
            for (ci, ch) in line.chars().enumerate() {
                let cx = cxi + ci as i64;
                if cx >= cxl {
                    break;
                }
                let attr = attrs
                    .and_then(|a| a.get(ci).copied())
                    .unwrap_or(content_base_attr);
                self.set_cell(cx, cy, ch, attr);
            }
            let fa = fill_for(li);
            for x in (cxi + line.chars().count() as i64)..cxl {
                self.set_cell(x, cy, ' ', fa);
            }
        }
        for li in content_lines.len()..((cyl - cyi) as usize) {
            let fa = fill_for(li);
            for x in cxi..cxl {
                self.set_cell(x, cyi + li as i64, ' ', fa);
            }
        }
        self.draw_border(rect, border_attr, label);
    }

    /// blessed box render: content + border inside the box rect + label.
    pub fn draw_box(
        &mut self,
        rect: Rect,
        border_attr: i64,
        label: &str,
        content_lines: &[String],
        content_line_attrs: &[Vec<i64>],
        content_base_attr: i64,
    ) {
        self.draw_box_fill(
            rect,
            border_attr,
            label,
            content_lines,
            content_line_attrs,
            content_base_attr,
            content_base_attr,
        )
    }

    /// Same as draw_box but with an explicit fill attr for cells past each
    /// line's content (blessed walk-end attr).
    #[allow(clippy::too_many_arguments)]
    pub fn draw_box_fill(
        &mut self,
        rect: Rect,
        border_attr: i64,
        label: &str,
        content_lines: &[String],
        content_line_attrs: &[Vec<i64>],
        content_base_attr: i64,
        fill_attr: i64,
    ) {
        // blessed: border occupies the OUTER row/col of the rect; content area
        // = inner (xi+1..xl-1, yi+1..yl-1) where xi..xl = full rect.
        let xi = rect.x;
        let xl = rect.x + rect.w;
        let yi = rect.y;
        let yl = rect.y + rect.h;
        // draw content first (children over borders handled by caller order),
        // blessed draws content rows yi..yl (border inside rect => content area
        // is yi..yl), xi..xl.
        let cxi = xi + 1;
        let cyi = yi + 1;
        let cxl = xl - 1;
        let cyl = yl - 1;
        // blessed: the content loop covers the FULL content rect; cells past
        // the clines content get `content[ci++] || bch` = a space with the
        // walk's end attr, so trailing cells and rows all fill `fill_attr`.
        for (li, line) in content_lines.iter().enumerate() {
            let cy = cyi + li as i64;
            if cy >= cyl {
                break;
            }
            let attrs = content_line_attrs.get(li);
            for (ci, ch) in line.chars().enumerate() {
                let cx = cxi + ci as i64;
                if cx >= cxl {
                    break;
                }
                let attr = attrs
                    .and_then(|a| a.get(ci).copied())
                    .unwrap_or(content_base_attr);
                self.set_cell(cx, cy, ch, attr);
            }
            for x in (cxi + line.chars().count() as i64)..cxl {
                self.set_cell(x, cy, ' ', fill_attr);
            }
        }
        // rows past the content lines: full-row fill (blessed bch continuation)
        for y in (cyi + content_lines.len() as i64).min(cyl)..cyl {
            for x in cxi..cxl {
                self.set_cell(x, y, ' ', fill_attr);
            }
        }
        // border on top (blessed draws border after content of CHILDREN? no:
        // element render: content, then border after — borders overwrite
        // overflowing content)
        self.draw_border(rect, border_attr, label);
    }

    fn draw_border(&mut self, rect: Rect, battr: i64, label: &str) {
        let xi = rect.x;
        let xl = rect.x + rect.w;
        let yi = rect.y;
        let yl = rect.y + rect.h;
        // top edge
        for x in xi..xl {
            let ch = if x == xi {
                '┌'
            } else if x == xl - 1 {
                '┐'
            } else {
                '─'
            };
            // top border row = rect.y (blessed: border drawn at yi..yl where
            // yi/y here = rect.y because border row was decremented back... the
            // border range for 'line' boxes: rows rect.y and rect.y+rect.h-1
            // (i.e. y = yi BEFORE the ++/--; we use absolute math here)
            self.set_cell(x, rect.y, ch, battr);
        }
        // side borders
        for y in (rect.y + 1)..(rect.y + rect.h - 1) {
            self.set_cell(xi, y, '│', battr);
            self.set_cell(xl - 1, y, '│', battr);
        }
        // bottom edge
        for x in xi..xl {
            let ch = if x == xi {
                '└'
            } else if x == xl - 1 {
                '┘'
            } else {
                '─'
            };
            self.set_cell(x, rect.y + rect.h - 1, ch, battr);
        }
        let _ = (yi, yl);
        // blessed label: a shrink Box child rendered AFTER the border:
        // xi = rect.x + 2; the wrap width = min(label_len, rect.w - 3);
        // the SHRUNK box width (mwidth) = the max wrapped-line length; each
        // row fills with bch spaces to that mwidth. (Verified 20-100 cols.)
        let lattr = DATTR;
        let label_len = label.chars().count() as i64;
        let label_left = rect.x + 2;
        let wrap_w = label_len.min(rect.w - 3).max(0) as usize;
        let wrapped_label = crate::wrap::wrap_content(label, wrap_w, true);
        let mwidth = wrapped_label
            .iter()
            .map(|l| l.chars().count())
            .max()
            .unwrap_or(0) as i64;
        for (li, line) in wrapped_label.iter().enumerate() {
            let row_y = rect.y + li as i64;
            if row_y >= rect.y + rect.h {
                break;
            }
            let mut col = label_left;
            for ch in line.chars() {
                if col >= label_left + mwidth || col >= xl - 1 {
                    break;
                }
                self.set_cell(col, row_y, ch, lattr);
                col += 1;
            }
            for col in col..label_left + mwidth {
                self.set_cell(col, row_y, ' ', lattr);
            }
        }
    }

    /// The capture/first-frame text: every cell, every row — runs with SGR;
    /// this is the C10 sheet (rows joined '\n' + trailing '\n').
    pub fn to_capture_text(&self) -> String {
        let mut rows_out: Vec<String> = Vec::with_capacity(self.rows);
        for y in 0..self.rows {
            let row: Vec<Cell> = self.cells[y * self.cols..(y + 1) * self.cols].to_vec();
            let fresh: Vec<Cell> = vec![Cell { ch: '\0', attr: -1 }; row.len()];
            rows_out.push(self.emit_row_cells(y, &row, &fresh));
        }
        rows_out.join("\n") + "\n"
    }

    /// blessed draw for one row from `prev`: cup(y, run-start) + SGR + chars.
    fn emit_row_cells(&self, y: usize, row: &[Cell], prev_row: &[Cell]) -> String {
        // Capture contract (CONTRACTS §C10 = mirror sheetRows): group the row
        // into runs of contiguous changed cells with equal attr (cells equal
        // to prev terminate a run); emit per run: cup(y, run.start), reset
        // (ESC[m) when the last emitted attr was non-default, SGR when
        // non-default, chars.
        let mut out = String::new();
        let mut runs: Vec<(usize, usize, i64)> = Vec::new();
        let mut x = 0usize;
        while x < row.len() {
            if row[x] == prev_row[x] {
                x += 1;
                continue;
            }
            let attr = row[x].attr;
            let start = x;
            while x < row.len() && row[x] != prev_row[x] && row[x].attr == attr {
                x += 1;
            }
            runs.push((start, x, attr));
        }
        let mut last_attr = DATTR;
        for &(start, end, attr) in &runs {
            out.push_str(&cup(y as i64, start as i64));
            if attr != last_attr {
                if last_attr != DATTR {
                    out.push_str("\x1b[m");
                }
                if attr != DATTR {
                    out.push_str(&format!("\x1b[{}m", attr_sgr_body(attr)));
                }
                last_attr = attr;
            }
            // blessed's BCE optimization (Screen.draw): trailing all-space
            // runs erase to EOL rather than emitting each space byte. That
            // byte-optimization belongs to the LIVE emitter only — the
            // capture/mirror sheet (C10) prints every sheet cell verbatim,
            // so gate it off there.
            let trailing_space_run = end == row.len()
                && row[start..end].iter().all(|c| c.ch == ' ')
                && (attr & 0x1ff) == (DATTR & 0x1ff)
                && self.emit_mode == EmitMode::Live;
            if trailing_space_run {
                out.push_str("\x1b[K");
            } else {
                out.extend(row[start..end].iter().map(|c| c.ch));
            }
        }
        out
    }

    /// Diff vs previous buffer; emit and swap (blessed olines semantics).
    pub fn emit_diff(&mut self) -> String {
        let mut out = String::new();
        let dbg = std::env::var("PTOP_DEBUG_DIFF").is_ok();
        let mut report = String::new();
        for y in 0..self.rows {
            let row: Vec<Cell> = self.cells[y * self.cols..(y + 1) * self.cols].to_vec();
            let prev_row: Vec<Cell> = self.prev[y * self.cols..(y + 1) * self.cols].to_vec();
            if dbg {
                let changed = row
                    .iter()
                    .zip(prev_row.iter())
                    .filter(|(a, b)| a != b)
                    .count();
                report.push_str(&format!("r{}={},", y + 1, changed));
            }
            let row_bytes = self.emit_row_cells(y, &row, &prev_row);
            if !row_bytes.is_empty() && self.emit_mode == EmitMode::Live {
                // Each row encoder starts from default SGR, but the terminal
                // retains attributes across cursor moves and previous frames.
                out.push_str("\x1b[m");
            }
            out.push_str(&row_bytes);
        }
        if dbg {
            eprintln!("DIFF {}", report);
        }
        self.prev.copy_from_slice(&self.cells);
        out
    }

    /// Force the NEXT emit_diff to be a full redraw.
    /// Test helper: install `other`'s content as this screen's prev buffer
    /// (diff baseline across frames without emitting).
    pub fn prev_reset_with(&mut self, other: Screen) {
        assert_eq!(self.cols, other.cols, "cols must match");
        assert_eq!(self.rows, other.rows, "rows must match");
        self.prev = other.cells;
    }

    /// Install `other`'s CONTENT as this frame's prev (diff baseline).
    /// Used between frames: compose() creates a fresh Screen each tick, so its
    /// own prev is defaults — without this, every tick would full-paint.
    pub fn prev_from_cell_snapshot(&mut self, other: &Screen) {
        // Sizes can differ on a resize tick; a length mismatch means we want a
        // full repaint anyway — default prev does that.
        if self.prev.len() != self.cells.len() || other.cells.len() != self.cells.len() {
            return;
        }
        self.prev.copy_from_slice(&other.cells);
    }

    /// Switch this screen's emitter mode (live vs capture sheet semantics).
    pub fn set_emit_mode(&mut self, mode: EmitMode) {
        self.emit_mode = mode;
    }

    pub fn reset_prev(&mut self) {
        self.prev = vec![Cell::default(); self.cols * self.rows];
    }

    /// Plain text (chars only) — debugging aid.
    pub fn to_plain_text(&self) -> String {
        let mut rows: Vec<String> = Vec::with_capacity(self.rows);
        for y in 0..self.rows {
            let row: String = self.cells[y * self.cols..(y + 1) * self.cols]
                .iter()
                .map(|c| c.ch)
                .collect();
            rows.push(row);
        }
        rows.join("\n")
    }
}

fn cup(y: i64, x: i64) -> String {
    format!("\x1b[{};{}H", y + 1, x + 1)
}

/// blessed sattr helper: theme fg on default bg.
pub fn attr_from_colors(fg: i64, bg: i64) -> i64 {
    (fg << 9) | bg
}

pub const DEFAULT_FG: i64 = DEFAULT_COLOR;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tags::{parse_tags, DATTR};

    #[test]
    fn capture_text_basics() {
        let mut s = Screen::new(10, 3);
        s.fill_region(DATTR, 'x', 0, 5, 0, 1);
        let text = s.to_capture_text();
        assert!(text.starts_with("\x1b[1;1Hxxxxx"));
        assert!(text.ends_with("\n"));
        assert_eq!(text.split('\n').count(), 4);
    }

    #[test]
    fn attr_runs_bg_before_fg() {
        let mut s = Screen::new(20, 1);
        let attr = (1 << 18) | (7 << 9) | 135;
        s.fill_region(attr, ' ', 0, 10, 0, 1);
        let text = s.to_capture_text();
        assert!(text.contains("\x1b[1;48;5;135;37m"));
    }

    // Independent, deliberately small ANSI background interpreter. Cursor
    // addressing does not reset SGR; erase-to-end uses the current background.
    fn apply_backgrounds(
        bytes: &str,
        cells: &mut [Option<u16>],
        cols: usize,
        cursor: &mut (usize, usize),
        bg: &mut Option<u16>,
    ) {
        let mut chars = bytes.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch != '\x1b' {
                cells[cursor.1 * cols + cursor.0] = *bg;
                cursor.0 += 1;
                continue;
            }
            assert_eq!(chars.next(), Some('['));
            let mut params = String::new();
            let command = loop {
                let ch = chars.next().expect("complete CSI");
                if ch.is_ascii_alphabetic() {
                    break ch;
                }
                params.push(ch);
            };
            let values: Vec<u16> = params.split(';').map(|v| v.parse().unwrap_or(0)).collect();
            match command {
                'H' => *cursor = (values[1] as usize - 1, values[0] as usize - 1),
                'K' => cells[cursor.1 * cols + cursor.0..(cursor.1 + 1) * cols].fill(*bg),
                'm' => {
                    let mut i = 0;
                    while i < values.len() {
                        match values[i] {
                            0 | 49 => *bg = None,
                            40..=47 => *bg = Some(values[i] - 40),
                            100..=107 => *bg = Some(values[i] - 100 + 8),
                            38 | 48 if values.get(i + 1) == Some(&5) => {
                                if values[i] == 48 {
                                    *bg = Some(values[i + 2]);
                                }
                                i += 2;
                            }
                            _ => {}
                        }
                        i += 1;
                    }
                }
                _ => panic!("unexpected CSI {command}"),
            }
        }
    }

    #[test]
    fn live_sgr_does_not_leak_across_rows_or_frames() {
        let mut screen = Screen::new(4, 2);
        let selected = (7 << 9) | 135;
        screen.set_cell(0, 0, 'S', selected);
        screen.set_cell(0, 1, 'x', DATTR);
        let mut backgrounds = vec![None; 8];
        let mut cursor = (0, 0);
        let mut bg = Some(2); // inherited terminal state must also be reset
        apply_backgrounds(
            &screen.emit_diff(),
            &mut backgrounds,
            4,
            &mut cursor,
            &mut bg,
        );
        assert_eq!(backgrounds[0], Some(135));
        assert_eq!(
            backgrounds[4], None,
            "default row inherited selection background"
        );

        screen.set_cell(1, 1, 'S', selected);
        apply_backgrounds(
            &screen.emit_diff(),
            &mut backgrounds,
            4,
            &mut cursor,
            &mut bg,
        );
        assert_eq!(bg, Some(135));
        screen.set_cell(1, 0, 'x', DATTR);
        screen.set_cell(1, 1, ' ', DATTR);
        apply_backgrounds(
            &screen.emit_diff(),
            &mut backgrounds,
            4,
            &mut cursor,
            &mut bg,
        );
        assert_eq!(
            backgrounds[1], None,
            "next frame inherited selection background"
        );
        assert_eq!(
            backgrounds[5], None,
            "selection removal retained its background"
        );
    }

    #[test]
    fn header_tags_render() {
        let (plain, attrs) = parse_tags(" {bold}vtop{/bold}{white-fg} for h {/}");
        assert_eq!(plain, " vtop for h ");
        assert!(attrs[1] >> 18 & 1 == 1);
        assert!(attrs[5] >> 18 & 1 == 0);
    }
}
