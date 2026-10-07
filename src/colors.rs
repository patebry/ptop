//! Blessed color system port — CONTRACTS §C12.
//! `match()` = nearest over 256-palette vcolors with weighted (30,59,11) RGB
//! distance, first-index-wins ties. Palette generated exactly like blessed.

pub fn vcolors() -> Vec<(u8, u8, u8)> {
    let mut v: Vec<(u8, u8, u8)> = Vec::with_capacity(256);
    // 0-15 xterm
    const XTERM: [u32; 16] = [
        0x000000, 0xcd0000, 0x00cd00, 0xcdcd00, 0x0000ee, 0xcd00cd, 0x00cdcd, 0xe5e5e5, 0x7f7f7f,
        0xff0000, 0x00ff00, 0xffff00, 0x5c5cff, 0xff00ff, 0x00ffff, 0xffffff,
    ];
    for c in XTERM {
        v.push((
            ((c >> 16) & 0xff) as u8,
            ((c >> 8) & 0xff) as u8,
            (c & 0xff) as u8,
        ));
    }
    // 16-231 cube: r,g,b 0..5 → r? r*40+55 : 0
    for r in 0..6u32 {
        for g in 0..6u32 {
            for b in 0..6u32 {
                let rr = if r > 0 { (r * 40 + 55) as u8 } else { 0 };
                let gg = if g > 0 { (g * 40 + 55) as u8 } else { 0 };
                let bb = if b > 0 { (b * 40 + 55) as u8 } else { 0 };
                v.push((rr, gg, bb));
            }
        }
    }
    // 232-255 grays: l = g*10+8
    for g in 0..24u32 {
        let l = (g * 10 + 8) as u8;
        v.push((l, l, l));
    }
    v
}

/// blessed's colors.ccolors init: at module load, an IIFE slices vcolors to
/// the FIRST 8, matches ALL 256 palette colors against that 8-color palette,
/// and CACHES the poisoned results in `_cache`. Any later `match()` whose RGB
/// equals a palette entry therefore returns the 8-color match. Port exactly.
pub fn blessed_palette_cache() -> std::collections::HashMap<u32, i64> {
    let full = vcolors();
    let mut cache = std::collections::HashMap::new();
    // colors[] = the hex strings of the palette (same RGB values as vcolors)
    for &(r, g, b) in full.iter() {
        let hash = ((r as u32) << 16) | ((g as u32) << 8) | b as u32;
        if cache.contains_key(&hash) {
            continue;
        }
        // match against the first 8 entries only
        let mut ldiff = f64::INFINITY;
        let mut li: i64 = -1;
        for (j, &(r2, g2, b2)) in full.iter().take(8).enumerate() {
            let diff = color_distance(r, g, b, r2, g2, b2);
            if diff == 0.0 {
                li = j as i64;
                break;
            }
            if diff < ldiff {
                ldiff = diff;
                li = j as i64;
            }
        }
        cache.insert(hash, li);
    }
    cache
}

use std::sync::OnceLock;

static PALETTE_CACHE: OnceLock<std::collections::HashMap<u32, i64>> = OnceLock::new();

fn palette_cache() -> &'static std::collections::HashMap<u32, i64> {
    PALETTE_CACHE.get_or_init(blessed_palette_cache)
}

/// blessed colors.match(hex) → palette index 0..=255 (first-wins ties).
pub fn match_hex(hex: &str) -> i64 {
    let (r1, g1, b1) = hex_to_rgb(hex);
    // 1) blessed's poisoned palette cache: RGB equal to a palette entry →
    //    the 8-color match recorded at module load
    let hash = ((r1 as u32) << 16) | ((g1 as u32) << 8) | b1 as u32;
    if let Some(&v) = palette_cache().get(&hash) {
        return v;
    }
    let vc = vcolors();
    let mut ldiff = f64::INFINITY;
    let mut li: i64 = -1;
    for (i, &(r2, g2, b2)) in vc.iter().enumerate() {
        let diff = color_distance(r1, g1, b1, r2, g2, b2);
        if diff == 0.0 {
            return i as i64;
        }
        if diff < ldiff {
            ldiff = diff;
            li = i as i64;
        }
    }
    li
}

fn color_distance(r1: u8, g1: u8, b1: u8, r2: u8, g2: u8, b2: u8) -> f64 {
    let (r1, g1, b1) = (r1 as f64, g1 as f64, b1 as f64);
    let (r2, g2, b2) = (r2 as f64, g2 as f64, b2 as f64);
    (30.0 * (r1 - r2)).powi(2) + (59.0 * (g1 - g2)).powi(2) + (11.0 * (b1 - b2)).powi(2)
}

pub fn hex_to_rgb(hex: &str) -> (u8, u8, u8) {
    // ASCII-hex validation first (a non-ASCII color like "#aé" would panic via
    // byte-slicing; JS hexToRGB → NaN-ish → rgb(0,0,0) black)
    let chars: Vec<char> = hex.chars().collect();
    let is_ascii_hex = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_hexdigit());
    let normalized: String = if hex.len() == 4
        && chars.len() == 4
        && chars[0] == '#'
        && is_ascii_hex(hex.get(1..).unwrap_or(""))
    {
        let body = &chars[1..];
        format!(
            "#{}{}{}{}{}{}",
            body[0], body[0], body[1], body[1], body[2], body[2]
        )
    } else if !hex.is_empty() && chars[0] == '#' && is_ascii_hex(hex.get(1..).unwrap_or("")) {
        hex.to_string()
    } else {
        return (0, 0, 0);
    };
    let col = u32::from_str_radix(normalized.get(1..).unwrap_or(""), 16).unwrap_or(0);
    (
        ((col >> 16) & 0xff) as u8,
        ((col >> 8) & 0xff) as u8,
        (col & 0xff) as u8,
    )
}

/// blessed colors.convert(color): '#hex' → match; literal 'fg' (not a hex,
/// not a color name) → -1 → returned as 0x1ff (default = inherit terminal).
pub const DEFAULT_COLOR: i64 = 0x1ff;

pub fn convert(color: &str) -> i64 {
    // blessed colors.convert: '#hex' → match; named colors (white=7, fg/bg/
    // default/normal → -1→default) — CONTRACTS §C12 with the name table.
    const NAMED: &[(&str, i64)] = &[
        ("default", DEFAULT_COLOR),
        ("normal", DEFAULT_COLOR),
        ("bg", DEFAULT_COLOR),
        ("fg", DEFAULT_COLOR),
        ("black", 0),
        ("red", 1),
        ("green", 2),
        ("yellow", 3),
        ("blue", 4),
        ("magenta", 5),
        ("cyan", 6),
        ("white", 7),
        ("lightblack", 8),
        ("lightred", 9),
        ("lightgreen", 10),
        ("lightyellow", 11),
        ("lightblue", 12),
        ("lightmagenta", 13),
        ("lightcyan", 14),
        ("lightwhite", 15),
        ("brightblack", 8),
        ("brightred", 9),
        ("brightgreen", 10),
        ("brightyellow", 11),
        ("brightblue", 12),
        ("brightmagenta", 13),
        ("brightcyan", 14),
        ("brightwhite", 15),
        ("grey", 8),
        ("gray", 8),
        ("lightgrey", 7),
        ("lightgray", 7),
        ("brightgrey", 7),
        ("brightgray", 7),
    ];
    let stripped = color.replace(['\u{2d}', ' '], "");
    for (name, idx) in NAMED {
        if stripped == *name {
            return *idx;
        }
    }
    if let Some(hexpart) = color.strip_prefix('#') {
        if u32::from_str_radix(hexpart, 16).is_ok() {
            return match_hex(color);
        }
    }
    DEFAULT_COLOR
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parallax_purple_matches_expected_256() {
        // #a537fd → nearest in cube: r=165→ nearest cube level 4 (215)? levels: 0,95,135,175,215,255
        // r: |165-175|=10 < |165-215| → level 3(175); g: 55 → level 1(95)? |55-55|=0 → level 1; b: 253 → level 5(255)
        // index = 16 + 3*36 + 1*6 + 5 = 16+108+6+5 = 135
        assert_eq!(match_hex("#a537fd"), 135);
    }

    #[test]
    fn cyan_border() {
        // #00ebbe: g=0xeb=235 → level 5 (255)? |235-255|=20 vs |235-215|=20 tie → FIRST wins (strict <): cube order b inner... i = 16+r*36+g*6+b; first encountered with lower index wins on tie.
        // r=0 → 0; g: 235: tie between 215 and 255 → first in iteration order of g index? The palette is built g outer, b inner: for the SAME g level? No: match iterates index 0..255 ascending; index increases with g then b;
        // g=215 (level 4) comes before g=255 (level 5) → |Δg|=20 tie → level 4 wins: i=16+0*36+4*6+b*: b=0xbe=190: |190-175|=15 |190-215|=25 → level 3(175): b=3
        // i = 16 + 0 + 4*6 + 3 = 43
        assert_eq!(match_hex("#00ebbe"), 43);
    }

    #[test]
    fn literal_fg_is_default() {
        assert_eq!(convert("fg"), DEFAULT_COLOR);
    }
}
