//! Blessed tag parser → (char, attr) stream and attr→SGR emission. CONTRACTS §C8/C9.
//!
//! attr bitfield (blessed screen attr): flags<<18 | fg<<9 | bg, 9-bit fields.
//! 0x1ff = "default / inherit terminal" (no SGR emitted).

pub const DEFAULT_COLOR: i64 = 0x1ff;
pub const DATTR: i64 = (DEFAULT_COLOR << 9) | DEFAULT_COLOR; // dattr = 0|(0x1ff<<9)|0x1ff

pub const FLAG_BOLD: i64 = 1;
pub const FLAG_UNDERLINE: i64 = 2;
pub const FLAG_BLINK: i64 = 4;
pub const FLAG_INVERSE: i64 = 8;
pub const FLAG_INVISIBLE: i64 = 16;

/// blessed Element._parseTags, restricted to the tags vtop emits (CONTRACTS §C8
/// exhaustive tag list). Produces the plain string (tags resolved).
pub fn parse_tags_to_plain(text: &str) -> String {
    // vtop's texts never contain literal '{' beyond tags; a full port of the
    // parser state machine is below in parse_tags; plain = drop all tags.
    parse_tags(text).0
}

/// Parse tags → (plain text, per-char attr vector, parallel to plain chars).
/// Faithful subset of blessed's parser: stateful fg/bg/flag stacks; {/} resets
/// to normal (default attr); param closing tags pop their stack and emit
/// the parent state (or _attr(param,false) when stack empties).
/// For vtop's tags the observable outcome simplifies to:
///   {bold}/{white-fg}/{white-bg}/{black-fg}/{red-bg} open, {/bold} etc close,
///   {/} reset-all. We implement the general small-machine to stay safe.
pub fn parse_tags(text: &str) -> (String, Vec<i64>) {
    let mut chars: Vec<(char, i64)> = Vec::with_capacity(text.len());
    let mut current = DATTR;
    let mut bg_stack: Vec<String> = Vec::new();
    let mut fg_stack: Vec<String> = Vec::new();
    let mut flag_stack: Vec<String> = Vec::new();

    let b = text.as_bytes();
    let mut i = 0usize;
    while i < b.len() {
        if b[i] == b'{' {
            // try to match an {open}, {/close}, or {/} tag
            if let Some(close) = find_tag_end(text, i) {
                let inner = &text[i + 1..close];
                i = close + 1;
                step_tag(
                    inner,
                    &mut current,
                    &mut bg_stack,
                    &mut fg_stack,
                    &mut flag_stack,
                );
                continue;
            }
        }
        // regular char
        let ch = text[i..].chars().next().unwrap();
        chars.push((ch, current));
        i += ch.len_utf8();
    }
    (
        chars.iter().map(|(c, _)| c).collect(),
        chars.into_iter().map(|(_, a)| a).collect(),
    )
}

#[allow(clippy::too_many_arguments)]
fn step_tag(
    inner: &str,
    current: &mut i64,
    bg_stack: &mut Vec<String>,
    fg_stack: &mut Vec<String>,
    flag_stack: &mut Vec<String>,
) {
    let slash = inner.starts_with('/');
    let param_full = inner.trim_start_matches('/');
    let param = param_full.replace('-', " ");
    if param.is_empty() {
        if slash {
            // {/} → normal: reset all
            *current = DATTR;
            bg_stack.clear();
            fg_stack.clear();
            flag_stack.clear();
        }
        return;
    }
    let kind = if param.ends_with(" bg") {
        Some(1)
    } else if param.ends_with(" fg") {
        Some(2)
    } else {
        None
    };
    if slash {
        // closing tag: pop the corresponding stack; re-emit parent state
        if param_full == "bold" {
            flag_stack.pop();
            *current &= !(FLAG_BOLD << 18);
            return;
        }
        match kind {
            Some(1) => {
                // NOTE: inner includes the slash already trimmed above; param_full has no '/'
                bg_stack.pop();
                let idx = bg_stack
                    .last()
                    .map(|p| attr_for_param_color(&p.replace('-', " "), "bg"))
                    .unwrap_or(DEFAULT_COLOR);
                *current = (*current & !0x1ff) | idx;
            }
            Some(2) => {
                fg_stack.pop();
                let idx = fg_stack
                    .last()
                    .map(|p| attr_for_param_color(&p.replace('-', " "), "fg"))
                    .unwrap_or(DEFAULT_COLOR);
                *current = (*current & !(0x1ff << 9)) | (idx << 9);
            }
            _ => {
                flag_stack.pop();
            }
        }
    } else {
        if param_full == "bold" {
            flag_stack.push(param_full.to_string());
            *current |= FLAG_BOLD << 18;
            return;
        }
        match kind {
            Some(1) => {
                bg_stack.push(param_full.to_string());
                let idx = attr_for_param_color(&param, "bg");
                *current = (*current & !0x1ff) | idx;
            }
            Some(2) => {
                fg_stack.push(param_full.to_string());
                let idx = attr_for_param_color(&param, "fg");
                *current = (*current & !(0x1ff << 9)) | (idx << 9);
            }
            _ => {
                flag_stack.push(param_full.to_string());
            }
        }
    }
}

/// blessed _parseTags output: tags → SGR-embedded string (what _wrapContent
/// consumes). CONTRACTS §C8. Supports vtop's exhaustive tag set + `{/}`.
pub fn parse_tags_to_sgr(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let b = text.as_bytes();
    let mut i = 0usize;
    while i < b.len() {
        if b[i] == b'{' {
            if let Some(close) = find_tag_end(text, i) {
                let inner = &text[i + 1..close];
                i = close + 1;
                let slash = inner.starts_with('/');
                let param_full = inner.trim_start_matches('/');
                if param_full.is_empty() {
                    if slash {
                        out.push_str("\x1b[m"); // {/} = normal
                    }
                    continue;
                }
                if param_full == "bold" {
                    out.push_str(if slash { "\x1b[22m" } else { "\x1b[1m" });
                    continue;
                }
                let param = param_full.replace('-', " ");
                let (base, false_form) = sgr_for_8color(&param);
                match (base, false_form) {
                    (Some(code), _) => {
                        if slash {
                            out.push_str("\x1b[39m"); // /<color>-fg … blessed emits 39 for fg closes
                        } else {
                            out.push_str(&format!("\x1b[{}m", code));
                        }
                    }
                    (None, Some(code)) => {
                        // bg colors: /<color>-bg → 49
                        if slash {
                            out.push_str("\x1b[49m");
                        } else {
                            out.push_str(&format!("\x1b[{}m", code));
                        }
                    }
                    (None, None) => out.push_str(&format!("{{{}}}", inner)),
                }
                continue;
            }
        }
        let ch = text[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// 8-color SGR table: returns (fg_code, bg_code) — "white fg" → Some 37/None…
fn sgr_for_8color(param: &str) -> (Option<u8>, Option<u8>) {
    match param {
        "white fg" => (Some(37), None),
        "black fg" => (Some(30), None),
        "red fg" => (Some(31), None),
        "red bg" => (None, Some(41)),
        "white bg" => (None, Some(47)),
        "black bg" => (None, Some(40)),
        _ => (None, None),
    }
}

/// Walk an SGR-embedded line from a `base` attr, blessed-attrCode style:
/// returns (plain chars, per-char attrs). Semantics per CONTRACTS §C8/C9:
/// SGR 1 → bold flag; 22/4/5/7/8/27/28 flag ops; 39 → fg default;
/// 49 → bg default; 30-37/90-97 → fg; 40-47/100-107 → bg; 38;5;n/48;5;n →
/// 256-color; ESC[m → RESET TO DEFAULT ATTR (0x1ff fields, flags cleared —
/// NOT the base).
pub fn parse_tags_to_attrs_sgr_walk(line: &str, base: i64) -> (String, Vec<i64>) {
    let (p, a, _f) = walk_sgr_attrs(line, base, base);
    (p, a)
}

/// Triple form: also returns the walk's final attr state (cells past the
/// content use it — blessed `content[ci++] || bch`).
pub fn walk_sgr_attrs3(line: &str, base: i64, dattr: i64) -> (String, Vec<i64>, i64) {
    walk_sgr_attrs(line, base, dattr)
}

/// The same walk, with an EXPLICIT reset-target `dattr` (blessed passes the
/// ELEMENT's dattr to attrCode — ESC[m resets to element defaults, and the
/// content cells after the text carry that attr).
pub fn walk_sgr_attrs(line: &str, base: i64, dattr: i64) -> (String, Vec<i64>, i64) {
    let b: Vec<char> = line.chars().collect();
    let mut plain = String::with_capacity(b.len());
    let mut attrs = Vec::with_capacity(b.len());
    let mut cur = base;
    let mut i = 0usize;
    while i < b.len() {
        if b[i] == '\x1b' {
            // parse the SGR run
            let mut j = i + 1;
            if j < b.len() && b[j] == '[' {
                j += 1;
                let start = j;
                while j < b.len() && (b[j].is_ascii_digit() || b[j] == ';') {
                    j += 1;
                }
                if j < b.len() && b[j] == 'm' {
                    // NOTE: slice by CHAR indices (braille chars are multi-byte!)
                    let params_str: String = b[start..j].iter().collect();
                    let params: Vec<&str> = params_str.split(';').collect();
                    cur = apply_sgr_params(&params, cur, dattr);
                    i = j + 1;
                    continue;
                }
            }
            i = j;
            continue;
        }
        plain.push(b[i]);
        attrs.push(cur);
        i += 1;
    }
    (plain, attrs, cur)
}

fn apply_sgr_params(params: &[&str], cur: i64, dattr: i64) -> i64 {
    let mut flags = (cur >> 18) & 0x1ff;
    let mut fg = (cur >> 9) & 0x1ff;
    let mut bg = cur & 0x1ff;
    let dflags = (dattr >> 18) & 0x1ff;
    let dfg = (dattr >> 9) & 0x1ff;
    let dbg = dattr & 0x1ff;
    let mut k = 0usize;
    let get = |idx: usize, params: &[&str]| -> i64 {
        params
            .get(idx)
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0)
    };
    while k < params.len() {
        let c = if params[k].is_empty() {
            0
        } else {
            match params[k].parse::<i64>() {
                Ok(v) => v,
                Err(_) => {
                    k += 1;
                    continue;
                }
            }
        };
        match c {
            0 => {
                // blessed attrCode case 0: reset to the ELEMENT's dattr
                bg = dbg;
                fg = dfg;
                flags = dflags;
            }
            1 => flags |= FLAG_BOLD,
            22 => flags &= !FLAG_BOLD,
            4 => flags |= FLAG_UNDERLINE,
            24 => flags &= !FLAG_UNDERLINE,
            5 => flags |= FLAG_BLINK,
            25 => flags &= !FLAG_BLINK,
            7 => flags |= FLAG_INVERSE,
            27 => flags &= !FLAG_INVERSE,
            8 => flags |= FLAG_INVISIBLE,
            28 => flags &= !FLAG_INVISIBLE,
            39 => fg = dfg,
            49 => bg = dbg,
            100 => {
                fg = dfg;
                bg = dbg;
            }
            v if (40..=47).contains(&v) => bg = v - 40,
            v if (100..=107).contains(&v) => bg = (v - 100) + 8,
            v if (30..=37).contains(&v) => fg = v - 30,
            v if (90..=97).contains(&v) => fg = (v - 90) + 8,
            48 => {
                if params.get(k + 1).copied().unwrap_or("") == "5" {
                    bg = get(k + 2, params);
                    k += 2;
                }
            }
            38 => {
                if params.get(k + 1).copied().unwrap_or("") == "5" {
                    fg = get(k + 2, params);
                    k += 2;
                }
            }
            _ => {}
        }
        k += 1;
    }
    (flags << 18) | (fg << 9) | bg
}

fn find_tag_end(text: &str, start: usize) -> Option<usize> {
    text[start..].find('}').map(|p| start + p)
}

fn attr_for_param_color(param: &str, kind: &str) -> i64 {
    let p = param.replace('-', " ");
    let name = p.trim();
    match (kind, name) {
        ("fg", "black") => 0,
        ("fg", "red") => 1,
        ("fg", "green") => 2,
        ("fg", "yellow") => 3,
        ("fg", "blue") => 4,
        ("fg", "magenta") => 5,
        ("fg", "cyan") => 6,
        ("fg", "white") => 7,
        ("bg", "black") => 0,
        ("bg", "red") => 1,
        ("bg", "green") => 2,
        ("bg", "yellow") => 3,
        ("bg", "blue") => 4,
        ("bg", "magenta") => 5,
        ("bg", "cyan") => 6,
        ("bg", "white") => 7,
        _ => DEFAULT_COLOR,
    }
}

#[allow(dead_code)]
fn attr_for_param(param: &str, closed: bool) -> i64 {
    let _ = (closed, param);
    if param == "bold" {
        FLAG_BOLD
    } else {
        0
    }
}

/// attr → SGR per blessed Screen.draw (bg before fg; ESC[m reset handling is
/// in the emitter). Returns the SGR body (without ESC[ and 'm').
pub fn attr_sgr_body(attr: i64) -> String {
    let flags = (attr >> 18) & 0x1ff;
    let fg = (attr >> 9) & 0x1ff;
    let bg = attr & 0x1ff;
    let mut out = String::new();
    if flags & FLAG_BOLD != 0 {
        out += "1;";
    }
    if flags & FLAG_UNDERLINE != 0 {
        out += "4;";
    }
    if flags & FLAG_BLINK != 0 {
        out += "5;";
    }
    if flags & FLAG_INVERSE != 0 {
        out += "7;";
    }
    if flags & FLAG_INVISIBLE != 0 {
        out += "8;";
    }
    if bg != DEFAULT_COLOR {
        if bg < 16 {
            if bg < 8 {
                out += &format!("{};", bg + 40);
            } else {
                out += &format!("{};", (bg - 8) + 100);
            }
        } else {
            out += &format!("48;5;{};", bg);
        }
    }
    if fg != DEFAULT_COLOR {
        if fg < 16 {
            if fg < 8 {
                out += &format!("{};", fg + 30);
            } else {
                out += &format!("{};", (fg - 8) + 90);
            }
        } else {
            out += &format!("38;5;{};", fg);
        }
    }
    if out.ends_with(';') {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_tags() {
        let (plain, attrs) = parse_tags(" {bold}vtop{/bold}{white-fg} for h {/}");
        assert_eq!(plain, " vtop for h ");
        // ' vtop' bold → then plain
        assert_eq!(attrs[1] >> 18 & FLAG_BOLD, FLAG_BOLD);
        assert_eq!(attrs[6] >> 18 & FLAG_BOLD, 0); // after {/bold}
    }
}
