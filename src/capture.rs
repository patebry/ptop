//! --capture: offline frame dump driving the SAME compose pipeline as live
//! rendering (CONTRACTS §C10). Fixture JSON schema is §C10.

use crate::app::{compose_scrolled, FrameState, Mode};
use crate::chart::Values;
use crate::screen::EmitMode;
use crate::sensors::SortKey;
use crate::table::Row;
use crate::theme;
use serde_json::Value;

fn f64_or(v: Option<&Value>, dflt: f64) -> f64 {
    let v = match v {
        Some(v) => v,
        None => return dflt,
    };
    match v {
        Value::Number(n) => n.as_f64().unwrap_or(dflt),
        Value::String(s) if s == "NaN" => f64::NAN,
        Value::String(s) => s.trim().parse().unwrap_or(dflt),
        _ => dflt,
    }
}

fn string_or(v: Option<&Value>, dflt: &str) -> String {
    let v = match v {
        Some(v) => v,
        None => return dflt.to_string(),
    };
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => crate::jsnum::js_to_string(n.as_f64().unwrap_or(0.0)),
        _ => dflt.to_string(),
    }
}

/// Parse a proc row exactly like process.js display shape (JS semantics for
/// number-vs-string fields).
fn parse_row(v: &Value) -> Row {
    let get = |k: &str| v.get(k).cloned().unwrap_or(Value::Null);
    let cpu_disp = string_or(Some(&get("CPU %")), "0.0");
    let mem_disp = string_or(Some(&get("Memory %")), "0.0");
    let count = string_or(Some(&get("Count")), "0");
    let cpu_raw = f64_or(Some(&get("CPU %")), f64::NAN);
    let mem_raw = f64_or(Some(&get("Memory %")), f64::NAN);
    Row {
        command: string_or(Some(&get("Command")), ""),
        count,
        cpu: cpu_disp,
        mem: mem_disp,
        cpu_raw,
        mem_raw,
    }
}

/// Run the capture pipeline. Returns the process exit code.
pub fn capture(path: &str) -> i32 {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("ptop: cannot read fixture {}: {}", path, e);
            return 2;
        }
    };
    let fx: Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("ptop: fixture parse error: {}", e);
            return 2;
        }
    };
    let theme_name = string_or(fx.get("theme"), "parallax");
    let th = match theme::load(&theme_name) {
        Ok(t) => theme::resolve(&t),
        Err(_) => {
            println!("The theme '{}' does not exist.", theme_name);
            return 1;
        }
    };

    // hostile-fixture guard (blessed clamps on the JS side; we clamp here)
    let cols = (f64_or(fx.get("cols"), 100.0) as i64).clamp(4, 2000);
    let rows = (f64_or(fx.get("rows"), 24.0) as i64).clamp(3, 1000);
    let brand = string_or(fx.get("brand"), "vtop");
    let hostname = string_or(fx.get("hostname"), "localhost");
    let clock = string_or(fx.get("clock"), "00:00:00 ");
    let la = fx.get("loadavg");
    let loadavg = match la {
        Some(Value::Array(a)) => (
            f64_or(a.first(), 0.0),
            f64_or(a.get(1), 0.0),
            f64_or(a.get(2), 0.0),
        ),
        _ => (0.0, 0.0, 0.0),
    };

    let parse_values = |key: &str| -> Values {
        let mut v = Values::new();
        if let Some(Value::Array(arr)) = fx.get(key) {
            for (i, item) in arr.iter().enumerate() {
                let val = match item {
                    Value::Null => None,
                    Value::Number(n) => Some(n.as_f64().unwrap_or(f64::NAN)),
                    Value::String(s) if s == "NaN" => Some(f64::NAN),
                    Value::String(s) => Some(s.trim().parse().unwrap_or(f64::NAN)),
                    _ => None,
                };
                if val.is_none() {
                    // C10/CONTRACTS: JSON null = a DELETED hole (skipped by
                    // for..in), matching the mirror's fixtureArray.
                    v.map.insert(i as i64, crate::chart::Slot::Hole);
                } else {
                    v.map.insert(i as i64, crate::chart::Slot::Val(val));
                }
                v.position = i as i64;
            }
        }
        v
    };

    let mut cpu_values = parse_values("cpu_values");
    let mut mem_values = parse_values("mem_values");
    let cpu_scale = f64_or(fx.get("cpu_scale"), 1.0);
    let mem_scale = f64_or(fx.get("mem_scale"), 1.0);
    let cpu_value = f64_or(fx.get("cpu_value_label"), 0.0);
    let mem_value = f64_or(fx.get("mem_value_label"), 0.0);
    let cpu_initialized = fx
        .get("cpu_initialized")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let mem_initialized = fx
        .get("mem_initialized")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    // position is GLOBAL in vtop (one counter for both charts). The mirror
    // treats fixture values as the FINAL window (values[position] lands on
    // the LAST slot); a lone-mem fixture gives -1 → JS object key '-1',
    // iterated AFTER all array indices.
    let _ = 0i64;
    // The mirror (our oracle twin) uses position = cpuValues.length - 1 —
    // the fixture's own `position` field is INFORMATIONAL (0065/0066 taught
    // us the mirror ignores it); match the mirror.
    let position = cpu_values.length() - 1;

    let procs: Vec<Row> = fx
        .get("procs")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().map(parse_row).collect())
        .unwrap_or_default();
    let selected = f64_or(fx.get("selected"), 0.0) as i64;

    // parity/fix only affects sensor math, not static composition; capture is
    // composition + values, so mode is unused here (brand covers text diffs).
    let _ = Mode::Parity;
    let _ = SortKey::Cpu;

    let mut state = FrameState {
        cols,
        rows,
        theme: &th,
        hostname: &hostname,
        brand: &brand,
        clock: &clock,
        loadavg,
        cpu_initialized,
        cpu_value,
        cpu_is_undefined: false,
        cpu_values: &mut cpu_values,
        cpu_scale,
        position,
        mem_initialized,
        mem_value,
        mem_is_undefined: false,
        mem_values: &mut mem_values,
        mem_scale,
        procs: &procs,
        selected,
        graph_scale: 1.0,
    };
    let mut scr = compose_scrolled(
        &mut state,
        None,
        fx.get("upgrade_notice").and_then(Value::as_str),
    );
    scr.reset_prev();
    // C10: capture sheets print every cell — no EL compression (live-mode only)
    scr.set_emit_mode(EmitMode::Capture);
    print!("{}", scr.to_capture_text());
    0
}
