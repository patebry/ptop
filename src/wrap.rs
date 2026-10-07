//! blessed _wrapContent port (escape-aware, word-break backscan ≤10, and the
//! per-line _align including the '{|}' pipe-fill). CONTRACTS §C9/C11.
//!
//! Input: one line of tag-RESOLVED content (SGR escapes embedded) + width.
//! Output: the wrapped real lines (with SGR runs re-attached by blessed
//! semantics: parts keep their escapes).

/// Measure ESC-aware width (each ESC[...m counts 0).
fn real_len(line: &str) -> usize {
    let b: Vec<char> = line.chars().collect();
    let mut total = 0usize;
    let mut i = 0usize;
    while i < b.len() {
        if b[i] == '\x1b' {
            // skip ESC [ ... m
            let mut j = i + 1;
            if j < b.len() && b[j] == '[' {
                j += 1;
                while j < b.len() && (b[j].is_ascii_digit() || b[j] == ';') {
                    j += 1;
                }
                if j < b.len() && b[j] == 'm' {
                    i = j + 1;
                    continue;
                }
            }
            i = j;
            continue;
        }
        total += 1;
        i += 1;
    }
    total
}

/// per rendered-line align: blessed _align with '{|}' pipe-fill support.
pub fn align_line(line: &str, width: usize) -> String {
    let cline = strip_sgr(line);
    if !cline.contains("{|}") {
        return line.to_string();
    }
    // blessed: parts = line.split('{|}') — measure tag-stripped halves,
    // filler = max(width - L - R, 0) between them.
    let parts: Vec<&str> = line.split("{|}").collect();
    if parts.len() == 2 {
        let l_plain_len = strip_sgr(parts[0]).chars().count();
        let r_plain_len = strip_sgr(parts[1]).chars().count();
        // NOTE: blessed measures cline (the whole line) split by '{|}' —
        // identical to the halves here.
        let s2 = width.saturating_sub(l_plain_len + r_plain_len);
        return format!("{}{}{}", parts[0], " ".repeat(s2), parts[1]);
    }
    line.to_string()
}

fn strip_sgr(line: &str) -> String {
    // remove ESC[...m AND '{|}'? blessed cline keeps {|} inside cline (splits
    // on it later). We keep the token.
    let b: Vec<char> = line.chars().collect();
    let mut out = String::with_capacity(b.len());
    let mut i = 0usize;
    while i < b.len() {
        if b[i] == '\x1b' {
            let mut j = i + 1;
            if j < b.len() && b[j] == '[' {
                j += 1;
                while j < b.len() && (b[j].is_ascii_digit() || b[j] == ';') {
                    j += 1;
                }
                if j < b.len() && b[j] == 'm' {
                    i = j + 1;
                    continue;
                }
            }
            i = j;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    out
}

/// blessed _wrapContent: returns wrapped lines (real lines, escapes kept).
/// `wrap` = the element's wrap option (boxes default true; the mirror uses
/// the same).
pub fn wrap_content(content: &str, width: usize, wrap: bool) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let lines: Vec<&str> = content.split('\n').collect();
    if content.is_empty() {
        out.push(content.to_string());
        return out;
    }
    'main: for line in lines {
        let mut line = line.to_string();
        loop {
            if real_len(&line) <= width && out_is_short(&line, width) {
                break;
            }
            // find ESC-aware cut at width
            let b: Vec<char> = line.chars().collect();
            let mut i = 0usize;
            let mut total = 0usize;
            let mut cut_found = false;
            while i < b.len() {
                let c = b[i];
                if c == '\x1b' {
                    // consume escape (blessed: `while (line[i] === '\x1b') { while
                    // (line[i] && line[i++] !== 'm'); }` — advances past 'm')
                    let mut j = i + 1;
                    if j < b.len() && b[j] == '[' {
                        j += 1;
                        while j < b.len() && (b[j].is_ascii_digit() || b[j] == ';') {
                            j += 1;
                        }
                        if j < b.len() && b[j] == 'm' {
                            i = j + 1;
                            continue;
                        }
                    } else {
                        // bare ESC scan like blessed (unlikely here)
                        while i < b.len() && b[i] != 'm' {
                            i += 1;
                        }
                        i += 1;
                        continue;
                    }
                    i = j;
                    continue;
                }
                i += 1;
                total += 1;
                if total == width {
                    cut_found = true;
                    break;
                }
            }
            if !cut_found {
                break; // fits
            }
            // blessed: i++ after hitting width (`if (++total === width) { i++; …`).
            // NOTE my loop above increments i BEFORE total, so my i already
            // equals blessed's post-`i++` raw index — no extra +1 here.
            let i_cut = i;
            if !wrap {
                let part: String = b[..i_cut.min(b.len())].iter().collect();
                out.push(align_line(&part, width));
                continue 'main;
            }
            // word-break backscan (blessed: while (j > i - 10 && j > 0 &&
            // line[--j] !== ' '); — tests raw chars i-1 … i-10 inclusive)
            let mut j = i_cut;
            if i_cut != line.chars().count() {
                let bb: Vec<char> = line.chars().collect();
                while j > i_cut.saturating_sub(10) && j > 0 {
                    j -= 1;
                    if bb[j] == ' ' {
                        break;
                    }
                }
                if j < bb.len() && bb[j] == ' ' {
                    j += 1;
                } else {
                    j = i_cut;
                }
            } else {
                j = i_cut;
            }
            i = j;
            let part: String = b[..i].iter().collect();
            let rest: String = b[i.min(b.len())..].iter().collect();
            out.push(align_line(&part, width));
            line = rest;
            if line.is_empty() {
                continue 'main;
            }
            if line
                .chars()
                .all(|c| c == '\x1b' || c == '[' || c == ';' || c.is_ascii_digit() || c == 'm')
            {
                // blessed: pure-SGR remainder attaches to the previous line
                if let Some(last) = out.last_mut() {
                    last.push_str(&line);
                }
                continue 'main;
            }
        }
        out.push(align_line(&line, width));
    }
    out
}

fn out_is_short(line: &str, width: usize) -> bool {
    real_len(line) <= width
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn footer_pipe_wrap() {
        let left = "  {white-bg}{black-fg}dd{/black-fg}{/white-bg} Kill process  {white-bg}{black-fg}j{/black-fg}{/white-bg} Down  {white-bg}{black-fg}k{/black-fg}{/white-bg} Up  {white-bg}{black-fg}g{/black-fg}{/white-bg} Jump to top  {white-bg}{black-fg}G{/black-fg}{/white-bg} Jump to bottom  {white-bg}{black-fg}c{/black-fg}{/white-bg} Sort by CPU  {white-bg}{black-fg}m{/black-fg}{/white-bg} Sort by Mem";
        let content = format!("{}{{|}}http://parall.ax/vtop", left);
        // blessed: _parseTags FIRST (tags → SGR), then wrap
        let resolved = crate::tags::parse_tags_to_sgr(&content);
        let lines = wrap_content(&resolved, 100, true);
        let plain0 = strip_sgr(&lines[0]);
        // clines line 0 = 89 chars ('...m Sort by'); the box's cell-fill pads
        // remaining COLUMNS with spaces (screen-level, not wrap-level).
        assert_eq!(
            plain0,
            "  dd Kill process  j Down  k Up  g Jump to top  G Jump to bottom  c Sort by CPU  m Sort by "
        );
        // 'Memhttp://parall.ax/vtop' wrapped to line 1 with pipe-fill
        assert_eq!(lines.len(), 2);
        let plain1 = strip_sgr(&lines[1]);
        // filler = 100 - 3 - 21 = 76 spaces between Mem and URL
        assert!(plain1.starts_with("Mem") && plain1.contains("http://parall.ax/vtop"));
        assert_eq!(plain1.len(), 3 + 76 + 21);
    }

    #[test]
    fn header_short_fits() {
        let lines = wrap_content(" vtop for mac.local ", 30, true);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0], " vtop for mac.local ");
    }
}
