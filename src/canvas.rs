//! Braille canvas — byte-faithful Rust port of drawille 1.1.0 (`harness/drawille.js`).
//!
//! CONTRACT: CONTRACTS.md §C1. The `set()` bounds guard operates on the RAW
//! f64 coordinates BEFORE flooring (so x=-0.5 is skipped, x=-0.0 draws).

pub const MAP: [[u8; 2]; 4] = [[0x01, 0x08], [0x02, 0x10], [0x04, 0x20], [0x40, 0x80]];

#[derive(Debug, Clone)]
pub struct BrailleCanvas {
    /// pixel width, always even
    pub width: f64,
    /// pixel height, always multiple of 4
    pub height: f64,
    width_floored: usize,
    height_floored: usize,
    /// width*height/8 bytes; 1 byte = 1 braille char
    pub content: Vec<u8>,
}

impl BrailleCanvas {
    pub fn new(width: f64, height: f64) -> Self {
        // JS setter: width = Math.floor(width/2)*2; height = Math.floor(height/4)*4
        let width_floored = (width as i32 / 2 * 2) as usize;
        let height_floored = (height as i32 / 4 * 4) as usize;
        // drawille stores width/height as the FLOORED values (this._width)
        let buf_len = width_floored * height_floored / 8;
        BrailleCanvas {
            width: width_floored as f64,
            height: height_floored as f64,
            width_floored,
            height_floored,
            content: vec![0u8; buf_len],
        }
    }

    /// drawille `set(x, y)` — ORs a braille dot bit. Bounds guard is checked in
    /// f64 BEFORE flooring (drawille bug/quirk preserved per CONTRACTS §C1).
    pub fn set(&mut self, x: f64, y: f64) {
        if !(x >= 0.0 && x < self.width && y >= 0.0 && y < self.height) {
            return;
        }
        let x = x.floor();
        let y = y.floor();
        let nx = (x / 2.0).floor() as usize;
        let ny = (y / 4.0).floor() as usize;
        let coord = nx + (self.width_floored / 2) * ny;
        let mask = MAP[(y as i64 % 4) as usize][(x as i64 % 2) as usize];
        if coord < self.content.len() {
            self.content[coord] |= mask;
        }
    }

    pub fn clear(&mut self) {
        self.content.fill(0);
    }

    /// drawille `frame('\n')`: for every byte, when j == width/2 push '\n' and
    /// reset j; byte 0 → ' ', else U+2800 + byte. ONE trailing '\n'. No leading.
    pub fn frame(&self) -> String {
        let mut out = String::with_capacity(self.content.len() * 4 + self.height_floored / 4 + 1);
        let row_bytes = self.width_floored / 2;
        let mut j = 0usize;
        for &b in &self.content {
            if j == row_bytes {
                out.push('\n');
                j = 0;
            }
            if b == 0 {
                out.push(' ');
            } else {
                // U+2800 + b, 0 <= b <= 255 → safe char
                out.push(char::from_u32(0x2800 + b as u32).unwrap());
            }
            j += 1;
        }
        out.push('\n');
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_frame_is_blank_rows_with_trailing_newline() {
        let c = BrailleCanvas::new(4.0, 8.0);
        // drawille: '\n' pushed when j hits row boundary BEFORE each new row,
        // plus ONE trailing '\n' → "  \n  \n" (last row's newline is the trailing)
        assert_eq!(c.frame(), "  \n  \n");
    }

    #[test]
    fn set_dots_matches_braille_layout() {
        // map[0][0]=0x01 top-left dot
        let mut c = BrailleCanvas::new(2.0, 4.0);
        c.set(0.0, 0.0);
        assert_eq!(c.content[0], 0x01);
        assert_eq!(
            c.frame().lines().next().unwrap().chars().next().unwrap(),
            '\u{2801}'
        );
        // map[3][1]=0x80 bottom-right dot
        let mut c = BrailleCanvas::new(2.0, 8.0);
        c.set(1.0, 7.0);
        assert_eq!(c.content[1], 0x80);
    }

    #[test]
    fn out_of_bounds_raw_float_guard() {
        let mut c = BrailleCanvas::new(4.0, 4.0);
        // JS-verified: buffer is 2 bytes for 4×4
        c.set(-0.5, 0.0); // guard fails: skipped (drawille checks BEFORE floor)
        assert_eq!(c.content, vec![0, 0]);
        c.set(-0.0, 0.0); // -0.0 passes guard
        assert_eq!(c.content, vec![1, 0]);
        c.set(4.0, 0.0); // x == width → fail
        c.set(0.0, 4.0); // y == height → fail
        assert_eq!(c.content, vec![1, 0]);
    }
}
