//! Theme loading — vtop themes byte-copied; CONTRACTS §C12.

use crate::colors;
use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Theme {
    #[allow(dead_code)]
    #[serde(default)]
    pub name: String,
    #[allow(dead_code)]
    #[serde(default)]
    pub author: String,
    pub title: ColorSpec,
    pub chart: BoxSpec,
    pub table: TableSpec,
    pub footer: ColorSpec,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ColorSpec {
    #[allow(dead_code)]
    pub fg: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct BorderSpec {
    #[allow(dead_code)]
    #[serde(rename = "type")]
    pub kind: String,
    pub fg: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct BoxSpec {
    pub fg: String,
    pub border: BorderSpec,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ItemSpec {
    #[serde(default)]
    pub fg: String,
    #[serde(default)]
    pub bg: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ItemsSpec {
    #[serde(default)]
    pub selected: ItemSpec,
    #[serde(default)]
    pub item: ItemSpec,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct TableSpec {
    pub fg: String,
    #[serde(default)]
    pub items: ItemsSpec,
    pub border: BorderSpec,
}

/// Blessed colors.convert semantics for theme fg strings (CONTRACTS §C12):
/// hex → palette index; literal 'fg' → 0x1ff (inherit default → no SGR).
#[derive(Debug, Clone)]
pub struct ResolvedTheme {
    pub title_fg: i64,
    pub chart_fg: i64,
    pub chart_border_fg: i64,
    pub table_fg: i64,
    pub table_selected_bg: i64,
    pub table_selected_fg: i64,
    pub table_item_fg: i64,
    pub table_item_bg: i64,
    pub table_border_fg: i64,
    pub footer_fg: i64,
}

pub fn resolve(t: &Theme) -> ResolvedTheme {
    ResolvedTheme {
        title_fg: colors::convert(&t.title.fg),
        chart_fg: colors::convert(&t.chart.fg),
        chart_border_fg: colors::convert(&t.chart.border.fg),
        table_fg: colors::convert(&t.table.fg),
        table_selected_bg: colors::convert(&t.table.items.selected.bg),
        table_selected_fg: colors::convert(&t.table.items.selected.fg),
        table_item_fg: colors::convert(&t.table.items.item.fg),
        table_item_bg: colors::convert(&t.table.items.item.bg),
        table_border_fg: colors::convert(&t.table.border.fg),
        footer_fg: colors::convert(&t.footer.fg),
    }
}

// Bundle themes so an installed native binary never depends on the build checkout.
const THEMES: &[(&str, &str)] = &[
    ("acid", include_str!("../themes/acid.json")),
    ("becca", include_str!("../themes/becca.json")),
    ("brew", include_str!("../themes/brew.json")),
    ("certs", include_str!("../themes/certs.json")),
    ("dark", include_str!("../themes/dark.json")),
    ("gooey", include_str!("../themes/gooey.json")),
    ("gruvbox", include_str!("../themes/gruvbox.json")),
    ("monokai", include_str!("../themes/monokai.json")),
    ("nord", include_str!("../themes/nord.json")),
    ("parallax", include_str!("../themes/parallax.json")),
    ("seti", include_str!("../themes/seti.json")),
    ("wizard", include_str!("../themes/wizard.json")),
];

/// Load one of the bundled predecessor themes.
pub fn load(name: &str) -> Result<Theme, LoadError> {
    let (_, text) = THEMES
        .iter()
        .find(|(key, _)| *key == name)
        .ok_or(LoadError)?;
    serde_json::from_str(text).map_err(|_| LoadError)
}

#[derive(Debug)]
pub struct LoadError;

/// Bundled theme names in alphabetical order.
pub fn available() -> Vec<String> {
    THEMES.iter().map(|(name, _)| (*name).to_owned()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parallax_resolves() {
        let t = load("parallax").unwrap();
        assert_eq!(t.chart.border.fg, "#00ebbe");
        let r = resolve(&t);
        assert_eq!(r.title_fg, 135);
        assert_eq!(r.chart_border_fg, 43);
        assert_eq!(r.table_fg, colors::DEFAULT_COLOR);
        assert_eq!(r.footer_fg, colors::DEFAULT_COLOR);
        assert_eq!(r.table_selected_bg, 135); // #a537fd
    }

    #[test]
    fn twelve_themes_load() {
        let avail = available();
        assert!(avail.len() >= 12);
        for name in &avail {
            assert!(load(name).is_ok(), "theme {} failed to load", name);
        }
    }
}
