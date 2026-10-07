//! Sensors — byte-faithful ports of vtop's process sensors (CONTRACTS §C4–C6).
//!
//! Process sensor parses `ps -ewwwo %cpu,%mem,comm` output EXACTLY like
//! vtop's process.js (single replace of first double-space, offset-based comm
//! extraction, grouping by comm, first-sighting insertion order, raw-sum sort).
//! CPU formula matches os-utils (snapshot Δ over 1000 ms). Memory uses the
//! XOR-buggy mac formula in parity mode, corrected arithmetic in fixed mode.

use crate::jsnum;
use std::collections::HashMap;

// ─────────────────────────── process sensor ───────────────────────────

#[derive(Debug, Clone)]
pub struct ProcGroup {
    pub comm: String,
    // JS mixed storage: first sighting stores cpu/mem as STRINGS, subsequent
    // sightings become numbers (parseFloat(prev)+parseFloat(cur)). Sort uses
    // parseFloat of the stored value. We keep both worlds faithfully:
    pub cpu_raw: f64, // final numeric used for sort + display
    pub mem_raw: f64,
    pub count: u32,
    /// first-sighting order for stable sort
    pub order: usize,
}

/// Port of process.js poll() parsing. Input: raw stdout of
/// `ps -ewwwo %cpu,%mem,comm`. Returns groups; caller (display fns) formats.
pub fn parse_ps(stdout: &str, is_mac: bool) -> Vec<ProcGroup> {
    let mut stats: HashMap<String, (JsVal, JsVal, u32, usize)> = HashMap::new();
    let mut order = 0usize;
    let lines = js_split_newlines(stdout);
    // lines[0] = '' (ditch first line)
    for (idx, line) in lines.iter().enumerate() {
        let raw = if idx == 0 {
            String::new()
        } else {
            line.clone()
        };
        let current_line = js_trim(&raw).replacen("  ", " ", 1);
        let words = js_split_space(&current_line);
        // JS: typeof words[0] !== 'undefined' && typeof words[1] !== 'undefined'
        // NOTE: '' !== undefined is TRUE — so a line '' splits to [''] →
        // words[1] undefined → skipped. Lines like 'x y z...' pass.
        if words.len() >= 2 {
            let cpu = words[0].replace(",", ".");
            let mem = words[1].replace(",", ".");
            let offset = cpu.chars().count() + mem.chars().count() + 2;
            let comm_full: String = current_line.chars().skip(offset).collect();
            let comm: String = if is_mac {
                // split('/').last
                match comm_full.rsplit('/').next() {
                    Some(s) => s.to_string(),
                    None => String::new(),
                }
            } else {
                // split('/')[0]
                match comm_full.split('/').next() {
                    Some(s) => s.to_string(),
                    None => String::new(),
                }
            };
            match stats.get_mut(&comm) {
                Some((prev_cpu, prev_mem, count, _)) => {
                    let new_cpu = js_parse_float(&prev_cpu.as_string()) + js_parse_float(&cpu);
                    let new_mem = js_parse_float(&prev_mem.as_string()) + js_parse_float(&mem);
                    *prev_cpu = JsVal::Num(new_cpu);
                    *prev_mem = JsVal::Num(new_mem);
                    *count += 1;
                }
                None => {
                    stats.insert(comm.clone(), (JsVal::Str(cpu), JsVal::Str(mem), 1, order));
                    order += 1;
                }
            }
        }
    }

    let mut out: Vec<ProcGroup> = stats
        .into_iter()
        .map(|(comm, (cpu, mem, count, order))| ProcGroup {
            comm,
            cpu_raw: cpu.to_number_lossy_from_string_or_num(),
            mem_raw: mem.to_number_lossy_from_string_or_num(),
            count,
            order,
        })
        .collect();
    out.sort_by_key(|g| g.order); // V8 object key insertion order = first sighting
    out
}

#[derive(Debug, Clone)]
enum JsVal {
    Str(String),
    Num(f64),
}

impl JsVal {
    fn as_string(&self) -> String {
        match self {
            JsVal::Str(s) => s.clone(),
            JsVal::Num(n) => jsnum::js_to_string(*n),
        }
    }
    fn to_number_lossy_from_string_or_num(&self) -> f64 {
        match self {
            JsVal::Str(s) => js_parse_float(s),
            JsVal::Num(n) => *n,
        }
    }
}

/// JS parseFloat (leading-parsing, decimal only — our domain).
pub fn js_parse_float(s: &str) -> f64 {
    let t = s.trim_start();
    let bytes = t.as_bytes();
    let mut end = 0usize;
    let mut seen_digit = false;
    let mut seen_dot = false;
    let mut i = 0usize;
    if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
        i += 1;
    }
    while i < bytes.len() {
        let b = bytes[i];
        if b.is_ascii_digit() {
            seen_digit = true;
            i += 1;
            end = i;
        } else if b == b'.' && !seen_dot {
            seen_dot = true;
            i += 1;
        } else {
            break;
        }
    }
    if !seen_digit {
        return f64::NAN;
    }
    t[..end].parse::<f64>().unwrap_or(f64::NAN)
}

fn js_trim(s: &str) -> String {
    // JS trim: strip ECMAScript whitespace (superset of Rust trim for our input)
    s.trim_matches(|c: char| {
        matches!(
            c,
            ' ' | '\t' | '\n' | '\r' | '\u{b}' | '\u{c}' | '\u{a0}' | '\u{feff}'
        )
    })
    .to_string()
}

fn js_split_newlines(s: &str) -> Vec<String> {
    s.split('\n')
        .map(|x| x.trim_end_matches('\r').to_string())
        .collect()
}

fn js_split_space(s: &str) -> Vec<String> {
    if s.is_empty() {
        vec![String::new()]
    } else {
        s.split(' ').map(|x| x.to_string()).collect()
    }
}

/// process.js display tail: cpu display = parseFloat(raw/cores).toFixed(1),
/// mem display = parseFloat(raw).toFixed(1). Sort by raw (cpu or mem).
pub fn finalize(groups: &[ProcGroup], cores: usize, sort: SortKey) -> Vec<crate::table::Row> {
    let rows: Vec<crate::table::Row> = groups
        .iter()
        .map(|g| crate::table::Row {
            command: g.comm.clone(),
            count: jsnum::js_to_string(g.count as f64),
            cpu: jsnum::js_to_fixed(g.cpu_raw / cores as f64, 1),
            mem: jsnum::js_to_fixed(g.mem_raw, 1),
            cpu_raw: g.cpu_raw,
            mem_raw: g.mem_raw,
        })
        .collect();
    let mut indexed: Vec<(usize, crate::table::Row)> = rows.into_iter().enumerate().collect();
    // V8 sort: stable; comparator parseFloat(b[sort]) - parseFloat(a[sort])
    indexed.sort_by(|(ia, a), (ib, b)| {
        let (va, vb) = match sort {
            SortKey::Cpu => (b.cpu_raw, a.cpu_raw),
            SortKey::Mem => (b.mem_raw, a.mem_raw),
        };
        let cmp = js_compare_f64(va, vb);
        if cmp != std::cmp::Ordering::Equal {
            cmp
        } else {
            ia.cmp(ib)
        }
    });
    indexed.into_iter().map(|(_, r)| r).collect()
}

fn js_compare_f64(a: f64, b: f64) -> std::cmp::Ordering {
    match (a, b) {
        (x, y) if x.is_nan() || y.is_nan() => std::cmp::Ordering::Equal,
        (_, _) if a < b => std::cmp::Ordering::Less,
        (_, _) if a > b => std::cmp::Ordering::Greater,
        _ => std::cmp::Ordering::Equal,
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SortKey {
    Cpu,
    Mem,
}

// ─────────────────────────── memory sensor ───────────────────────────

/// mac parity math (CONTRACTS §C6): buggy XOR formula. `rss_sum_kb` = Σ parseInt
/// of numeric-leading lines from `ps -caxm -orss,comm`; `totalmem_mib`
/// is os-utils totalmem() = physical bytes / 1048576 (NOT raw bytes).
pub fn memory_mac_parity(rss_sum_kb: f64, totalmem_mib: f64) -> f64 {
    // Division binds before XOR. JS bitwise coercion truncates and wraps to i32.
    let quotient = rss_sum_kb / 1024.0;
    let integer = if quotient.is_finite() {
        quotient.trunc().rem_euclid(4_294_967_296.0) as u32 as i32
    } else {
        0
    };
    let usedmem = (integer ^ 2) as f64;
    let freemem = totalmem_mib - usedmem;
    let per = freemem / totalmem_mib;
    js_round((1.0 - per) * 100.0)
}

/// mac fixed path: percent of physical memory used by listed RSS.
pub fn memory_mac_fixed(rss_sum_kb: f64, totalmem: f64) -> f64 {
    let used = rss_sum_kb * 1024.0;
    js_round(100.0 * used / totalmem).clamp(0.0, 100.0)
}

/// linux: free -m line 1: used=col[2], total=col[1] (vtop uses parseInt of fields).
pub fn memory_linux(stdout_free_m: &str) -> f64 {
    let line = stdout_free_m.split('\n').nth(1).unwrap_or("");
    let data: Vec<&str> = js_split_ws(line).collect();
    let used = js_parse_int(data.get(2).copied().unwrap_or(""));
    let total = js_parse_int(data.get(1).copied().unwrap_or(""));
    js_round(100.0 * used / total)
}

fn js_split_ws(s: &str) -> impl Iterator<Item = &str> {
    s.split_whitespace()
}

/// JS parseInt (radix 10, leading numeric parse).
pub fn js_parse_int(s: &str) -> f64 {
    let t = s.trim_start();
    let bytes = t.as_bytes();
    let mut end = 0usize;
    let mut i = 0usize;
    if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
        i += 1;
    }
    let mut seen_digit = false;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        seen_digit = true;
        i += 1;
        end = i;
    }
    if !seen_digit {
        return f64::NAN;
    }
    t[..end].parse::<f64>().unwrap_or(f64::NAN)
}

pub fn js_round(v: f64) -> f64 {
    // JS Math.round: half toward +Infinity (NOT half-away!)
    if v.is_nan() {
        return f64::NAN;
    }
    (v + 0.5).floor()
}

// ─────────────────────────── cpu sensor ───────────────────────────

#[derive(Debug, Clone, Copy, Default)]
pub struct CpuTimes {
    pub user: f64,
    pub nice: f64,
    pub sys: f64,
    pub idle: f64,
    pub irq: f64,
}

impl CpuTimes {
    pub fn total(&self) -> f64 {
        self.user + self.nice + self.sys + self.idle + self.irq
    }
}

/// os-utils math: (1 − idleΔ/totalΔ); caller floors ×100.
pub fn cpu_percent(start: CpuTimes, end: CpuTimes) -> f64 {
    let idle = end.idle - start.idle;
    let total = end.total() - start.total();
    1.0 - idle / total
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_matches_javascript_precedence_and_units() {
        // Independently evaluated with the installed vtop expression in Node.
        assert_eq!(memory_mac_parity(1024.0, 4.0), 75.0);
        assert_eq!(memory_mac_parity(2048.0, 4.0), 0.0);
        assert_eq!(memory_mac_parity(4096.0, 8.0), 75.0);
        assert_eq!(memory_mac_parity(4_398_046_512_128.0, 4.0), 75.0);
        assert_eq!(memory_mac_fixed(1024.0, 4_194_304.0), 25.0);
    }

    #[test]
    fn ps_parse_darwin_vs_linux() {
        let out = "\
  %CPU %MEM COMM\n\
  12.0  2.5 /usr/sbin/kernel_task\n\
   6.5  1.0 /Applications/Safari.app/Contents/MacOS/Safari\n\
   1.2  0.1 kworker/0:1\n";
        let mac = parse_ps(out, true);
        assert_eq!(mac[0].comm, "kernel_task");
        assert_eq!(mac[1].comm, "Safari");
        assert_eq!(mac[2].comm, "0:1"); // mac rule: after last '/'
        let linux = parse_ps(out, false);
        // linux rule keeps before first '/': kworker/0:1 → 'kworker';
        // absolute paths like /usr/sbin/kernel_task → '' (vtop's real linux behavior)
        let k = linux
            .iter()
            .find(|g| g.comm.starts_with("kworker"))
            .unwrap();
        assert_eq!(k.comm, "kworker");
        assert!(linux
            .iter()
            .any(|g| g.comm.is_empty() || g.comm.contains("usr")));
    }

    #[test]
    fn process_sort_is_descending_and_stable_like_vtop() {
        let groups = parse_ps(
            "%CPU %MEM COMM\n9.0 1.0 alpha\n6.0 3.0 beta\n3.0 3.0 gamma\n",
            false,
        );
        let cpu = finalize(&groups, 1, SortKey::Cpu);
        assert_eq!(
            cpu.iter().map(|r| r.command.as_str()).collect::<Vec<_>>(),
            ["alpha", "beta", "gamma"]
        );
        let mem = finalize(&groups, 1, SortKey::Mem);
        assert_eq!(
            mem.iter().map(|r| r.command.as_str()).collect::<Vec<_>>(),
            ["beta", "gamma", "alpha"]
        );
        assert_eq!(js_compare_f64(f64::NAN, 1.0), std::cmp::Ordering::Equal);
    }

    #[test]
    fn grouping_and_counts() {
        let out = "\
%CPU %MEM COMM\n\
 1.0 0.5 chrome\n 2.0 0.7 chrome\n 0.1 0.1 zsh\n";
        let g = parse_ps(out, false);
        let chrome = g.iter().find(|x| x.comm == "chrome").unwrap();
        assert_eq!(chrome.count, 2);
        assert_eq!(chrome.cpu_raw, 3.0);
        assert_eq!(chrome.mem_raw, 1.2);
    }

    #[test]
    fn js_round_is_half_up_not_half_away() {
        assert_eq!(js_round(0.5), 1.0);
        assert_eq!(js_round(-0.5), 0.0); // JS: Math.round(-0.5) = -0 → 0
        assert_eq!(js_round(-1.5), -1.0);
        assert_eq!(js_round(2.5), 3.0);
    }

    #[test]
    fn memory_linux_free_m() {
        let out = "              total        used        free\n\
Mem:          16384        8192        4096\n";
        assert_eq!(memory_linux(out), 50.0);
    }
}
