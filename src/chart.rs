//! Chart renderer — verbatim port of vtop app.js `drawChart()` (CONTRACTS §C2).

use crate::canvas::BrailleCanvas;
use crate::jsnum::js_to_string;

/// JS array-slot semantics (CONTRACTS §C2, corrected during battle-testing):
/// - `Val(Some(f64))` — assigned number
/// - `Val(None)` — explicitly assigned `undefined`/`null`: ITERATED by for..in,
///   but renders nothing itself (computeValue → NaN). Its top-line/fill use
///   `values[pos-1]`, and its interpolation uses `values[pos]` (NaN).
/// - `Hole` — `delete`-created hole: NOT iterated by for..in.
pub enum Slot {
    Val(Option<f64>),
    Hole,
}

/// JS-array-with-holes semantics: values[position] = Some/None; iteration skips
/// holes in ascending index order; `values[pos-1]` of a hole/missing is None.
pub struct Values {
    /// highest assigned index + 1 (JS .length semantics: assignment extends)
    pub position: i64,
    /// windowed sparse storage: index → slot; Hole = deleted (skipped);
    /// Val(None) = assigned undefined (iterated, renders nothing)
    pub map: std::collections::HashMap<i64, Slot>,
}

impl Default for Values {
    fn default() -> Self {
        Values {
            position: -1,
            map: std::collections::HashMap::new(),
        }
    }
}

impl Values {
    pub fn new() -> Self {
        Values {
            position: -1,
            map: std::collections::HashMap::new(),
        }
    }

    /// Seed from fixture values (may contain nulls = explicitly-undefined
    /// slots, per JS JSON round-trip semantics: iterated). Index i = slot i.
    pub fn from_fixture(vals: &[Option<f64>]) -> Self {
        let mut v = Values::new();
        for (i, val) in vals.iter().enumerate() {
            v.map.insert(i as i64, Slot::Val(*val));
            v.position = i as i64;
        }
        v
    }

    /// values[position] = current — JS: assignment extends array length to
    /// position+1 regardless of monotonicity (position is monotone in vtop:
    /// incremented once per draw, so always position = count-1 for a fresh
    /// array; but resize re-creates `values` preserving old object, so position
    /// keeps growing while the new array restarts at slot `position`… actually
    /// charts[plugin].values is PRESERVED across resize; position keeps
    /// incrementing → assignments land at increasing indices; length grows
    /// forever until position > 5000 starts deleting. Net effect on screen:
    /// values.length = position+1 (until deletions create trailing hole edge —
    /// delete values[position-5000] removes slots below the window; length
    /// remains position+1 forever after. Iteration: for..in ascending.)
    pub fn assign(&mut self, position: i64, v: Option<f64>) {
        self.map.insert(position, Slot::Val(v));
        if position >= self.position {
            self.position = position;
        }
    }

    /// JS length: max assigned index + 1 (0 if none)
    pub fn length(&self) -> i64 {
        self.position + 1
    }

    /// JS values[i]: number, or undefined. Negative i = STRING object key
    /// ('-1') — present only if explicitly assigned; `values['-2']` → undefined.
    pub fn get(&self, i: i64) -> Option<f64> {
        match self.map.get(&i) {
            Some(Slot::Val(v)) => *v,
            _ => None,
        }
    }

    /// JS `pos in values` / for..in iteration: Hole slots are skipped.
    pub fn is_hole(&self, i: i64) -> bool {
        matches!(self.map.get(&i), Some(Slot::Hole))
    }

    /// drawChart deletion: if position > 5000, `delete values[position-5000]`.
    pub fn cull(&mut self) {
        if self.position > 5000 {
            self.map.remove(&(self.position - 5000));
        }
    }

    /// Indices visited by `for (const pos in values)`: ascending keys except
    /// deleted holes (explicit undefined IS iterated). JS for..in order:
    /// integer keys ascending FIRST, then non-integer string keys in insertion
    /// order — '-1' lands LAST.
    pub fn iter_indices(&self) -> Vec<i64> {
        let mut keys: Vec<i64> = self
            .map
            .iter()
            .filter(|(_, v)| !matches!(v, Slot::Hole))
            .map(|(k, _)| *k)
            .collect();
        keys.sort_unstable_by_key(|k| if *k < 0 { (1i64, -*k) } else { (0i64, *k) });
        keys
    }
}

/// computeValue(v) = height − floor(((height+1)/100)·v) − 1 (float math, floor).
/// Returns None when input is a hole (None) or NaN (JS NaN → f64 NaN passthrough).
fn compute_value(height: f64, input: Option<f64>) -> Option<f64> {
    let v = input?;
    let r = height - ((height + 1.0) / 100.0 * v).floor() - 1.0;
    if r.is_nan() {
        None
    } else {
        Some(r)
    }
}

pub struct ChartInput<'a> {
    pub canvas: &'a mut BrailleCanvas,
    pub width: f64,  // chart.width in chars
    pub height: f64, // chart.height in chars
    pub scale: f64,
    /// current sensor value for this tick (label + data point);
    /// `current_is_undefined`: JS passed literal `undefined` (label 'ned%' quirk)
    pub current_value: f64,
    pub current_is_undefined: bool,
    pub position: i64,
    /// initialized flag of the sensor — if false, vtop returns `false`
    /// (app.js sets graph content to `false` string → blessed renders 'false'
    /// text). CONTRACT: parity mode must reproduce this string.
    pub initialized: bool,
}

/// Port of drawChart. Returns the final text (frame+label splice), or the JS
/// `false` rendering when the sensor is uninitialized.
pub fn draw_chart(input: &mut ChartInput, values: &mut Values) -> String {
    input.canvas.clear();
    if !input.initialized {
        // vtop: graph.setContent(drawChart) where drawChart returned JS false →
        // blessed setContent(false) = `false || ''` = EMPTY content.
        return String::new();
    }
    let position = input.position;
    values.assign(position, Some(input.current_value));
    let compute_value_h = |v: Option<f64>| compute_value(input.height, v);

    if position > 5000 {
        values.cull();
    }

    for pos in values.iter_indices() {
        // graphScale >= 1 || (graphScale < 1 && pos % (1/graphScale) === 0)
        if input.scale >= 1.0 || (input.scale < 1.0 && pos as f64 % (1.0 / input.scale) == 0.0) {
            let p = pos as f64 + (input.width - values.length() as f64);
            let x = p * input.scale + (1.0 - input.scale) * input.width;

            // top line: values[pos-1] may be hole/None (JS: NaN → skip)
            let prev = values.get(pos - 1);
            if p > 1.0 {
                if let Some(cv) = compute_value_h(prev) {
                    if cv > 0.0 {
                        input.canvas.set(x, cv);
                    }
                }
            }

            // fill under the line
            let mut y_opt = compute_value_h(prev);
            while let Some(y) = y_opt {
                if y >= input.height {
                    break;
                }
                if input.scale > 1.0 && p > 0.0 && y > 0.0 {
                    let current = compute_value_h(prev);
                    let next = compute_value_h(values.get(pos));
                    let diff = next.unwrap_or(f64::NAN) - current.unwrap_or(f64::NAN);
                    let diff = diff / input.scale;
                    // for (let i = 0; i < graphScale; i++) — i is f64
                    let mut i = 0.0f64;
                    while i < input.scale {
                        input.canvas.set(x + i, y + diff * i);
                        // for (let j = y + diff*i; j < height; j++) — j f64, floors at set()
                        let mut j = y + diff * i;
                        while j < input.height {
                            input.canvas.set(x + i, j);
                            j += 1.0;
                        }
                        i += 1.0;
                    }
                } else if input.scale <= 1.0 {
                    input.canvas.set(x, y);
                }
                y_opt = y_opt.map(|y| y + 1.0);
            }
        }
    }

    // label splice
    let frame = input.canvas.frame();
    let mut lines: Vec<String> = frame.split('\n').map(|s| s.to_string()).collect();
    // frame() → split('\n') yields a final EMPTY element (trailing '\n'); JS
    // does the same and joins back — preserve indexing exactly: line 0 = data.
    // JS: textOutput[0].slice(0, len-4) + '{white-fg}' + percent.slice(-3) + '%{/white-fg}'
    // JS: percent = '   ' + currentValue — undefined → '   undefined', NaN → '   NaN'
    let percent = if input.current_is_undefined {
        // JS: percent = '   ' + undefined → '   undefined' → slice(-3) = 'ned'
        "   undefined".to_string()
    } else {
        format!("   {}", js_to_string(input.current_value))
    };
    let percent = percent[(percent.len().saturating_sub(3))..].to_string();
    let first = &mut lines[0];
    let cut = first.chars().count().saturating_sub(4);
    let new_first: String = {
        let head: String = first.chars().take(cut).collect();
        format!("{}{{white-fg}}{}%{{/white-fg}}", head, percent)
    };
    lines[0] = new_first;
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canvas(w: f64, h: f64) -> BrailleCanvas {
        BrailleCanvas::new(w, h)
    }

    #[test]
    fn uninitialized_renders_false() {
        let mut c = canvas(6.0, 8.0);
        let mut input = ChartInput {
            canvas: &mut c,
            width: 3.0,
            height: 2.0,
            scale: 1.0,
            current_value: 50.0,
            current_is_undefined: false,
            position: 0,
            initialized: false,
        };
        let mut vals = Values::new();
        assert_eq!(draw_chart(&mut input, &mut vals), "");
    }

    #[test]
    fn single_value_scale1() {
        let mut c = canvas(6.0, 8.0);
        let mut input = ChartInput {
            canvas: &mut c,
            width: 3.0,
            height: 2.0,
            scale: 1.0,
            current_value: 100.0,
            current_is_undefined: false,
            position: 0,
            initialized: true,
        };
        let mut vals = Values::new();
        let out = draw_chart(&mut input, &mut vals);
        // chart.height=2: computeValue(100) = 2 - floor(3/100*100) - 1 = 2-3-1 = -2 → skipped (guard)
        // computeValue(0)=1 → fill y=1: set(x,1)
        // label: first line splice
        assert!(out.contains("{white-fg}"));
    }

    #[test]
    fn label_splice_exact() {
        // Verify the JS splice arithmetic: first line 'abcdefg' → slice(0,3) + %tags
        // We test via a value and known canvas: width 8 → line 4 chars.
        let mut c = canvas(8.0, 4.0);
        let mut input = ChartInput {
            canvas: &mut c,
            width: 4.0,
            height: 1.0,
            scale: 1.0,
            current_value: 7.0,
            current_is_undefined: false,
            position: 0,
            initialized: true,
        };
        // height=1: computeValue(7) = 1 - floor(2/100*7) - 1 = 1 - 0 - 1 = 0 → set(x,0)
        let mut vals = Values::new();
        let out = draw_chart(&mut input, &mut vals);
        let line0 = out.split('\n').next().unwrap();
        // chars: 4 canvas chars: first is braille(0x01) at x= (0 + (4-1))*1 + 0*4 = 3
        // line = "   ⠁" (0-based col 3). slice(0, len-4)='' + '{white-fg}   7'... percent='   7'.slice(-3)='  7'
        assert_eq!(line0, "{white-fg}  7%{/white-fg}");
    }
}
