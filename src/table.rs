//! Table renderer — verbatim port of vtop app.js `drawTable()` (CONTRACTS §C3).

use crate::jsnum::{js_to_fixed, js_to_string};

pub const COLUMNS: [&str; 4] = ["Command", "CPU %", "Count", "Memory %"];

/// A body row: display values as JS-rendered strings (C3/C4).
#[derive(Debug, Clone)]
pub struct Row {
    pub command: String,
    pub count: String, // JS number → string
    pub cpu: String,   // toFixed(1)-formatted already (or fixture-provided)
    pub mem: String,
    /// raw summed values for sorting (process.js stores cpu/mem as strings pre-sort;
    /// sort uses parseFloat of those strings)
    pub cpu_raw: f64,
    pub mem_raw: f64,
}

impl Row {
    pub fn field(&self, column: &str) -> String {
        match column {
            "Command" => self.command.clone(),
            "CPU %" => self.cpu.clone(),
            "Count" => self.count.clone(),
            "Memory %" => self.mem.clone(),
            // JS object-miss → undefined → ' undefined' rendered; empty keeps
            // the render loop alive instead of panicking under panic=abort
            _ => String::new(),
        }
    }
}

fn js_string_repeat(s: &str, num: i64) -> String {
    if num < 0 {
        return String::new();
    }
    s.repeat(num as usize)
}

pub struct TableOut {
    pub title: String,
    pub body: Vec<String>,
    pub process_width: i64,
}

/// chart.width in chars. Fixture rows must be pre-sorted (sort happens upstream).
pub fn draw_table(width: i64, rows: &[Row]) -> TableOut {
    let mut column_lengths: std::collections::HashMap<&str, i64> = std::collections::HashMap::new();
    let mut columns: Vec<&'static str> = COLUMNS.iter().rev().copied().collect();
    let last_item = "Command"; // columns[columns.len()-1] AFTER reverse
    let minimum_width = 12;
    let padding: i64 = if width > 80 {
        3
    } else if width > 50 {
        2
    } else {
        1
    };

    loop {
        let mut total_used = 0i64;
        let mut first_length = 0i64;
        for (i, &item) in columns.iter().enumerate() {
            if item == last_item {
                column_lengths.insert(item, width - total_used);
                first_length = column_lengths[item];
            } else {
                column_lengths.insert(item, item.len() as i64 + padding);
            }
            total_used += column_lengths[item];
            let _ = i;
        }
        if first_length < minimum_width && columns.len() > 1 {
            columns.remove(0); // JS shift()
        } else {
            break;
        }
    }

    columns.reverse();

    let mut title_output = String::from("{bold}");
    for column in &columns {
        let col_text = format!(" {}", column);
        let len = column_lengths[column];
        title_output += &format!(
            "{}{}",
            col_text,
            js_string_repeat(" ", len - col_text.encode_utf16().count() as i64)
        );
    }
    title_output += "{/bold}\n";

    let mut body_output = Vec::with_capacity(rows.len());
    for row in rows {
        let mut row_text = String::new();
        for column in &columns {
            let cell = row.field(column);
            let col_text = format!(" {}", cell);
            let len = column_lengths[column];
            let cell_full = format!(
                "{}{}",
                col_text,
                js_string_repeat(" ", len - col_text.encode_utf16().count() as i64)
            );
            // slice(0, n) — truncate (chars, since ASCII-only here)
            let cut: String = crate::content::slice_units(&cell_full, len.max(0) as usize);
            row_text += &cut;
        }
        body_output.push(row_text);
    }

    let process_width = column_lengths[columns[0]];
    TableOut {
        title: title_output,
        body: body_output,
        process_width,
    }
}

/// Build display strings from sensor data (process.js tail): cpu display =
/// parseFloat(cpu_raw / cores).toFixed(1); mem display = parseFloat(mem_raw).toFixed(1).
pub fn display_row(comm: &str, count: u32, cpu_raw: f64, mem_raw: f64, cores: f64) -> Row {
    Row {
        command: comm.to_string(),
        count: js_to_string(count as f64),
        cpu: js_to_fixed(cpu_raw / cores, 1),
        mem: js_to_fixed(mem_raw, 1),
        cpu_raw,
        mem_raw,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(cmd: &str, count: u32, cpu: f64, mem: f64) -> Row {
        Row {
            command: cmd.into(),
            count: js_to_string(count as f64),
            cpu: js_to_fixed(cpu, 1),
            mem: js_to_fixed(mem, 1),
            cpu_raw: cpu,
            mem_raw: mem,
        }
    }

    #[test]
    fn one_row_min_widths_matches_js() {
        let rows = vec![row("Chrome", 4, 0.4, 1.0)];
        let out = draw_table(20, &rows);
        // JS-verified oracle: after drops, columns = [Command, CPU %]
        assert_eq!(out.title, "{bold} Command       CPU %{/bold}\n");
        assert_eq!(out.process_width, 14);
        // body: ' Chrome' + pad to 14 = ' Chrome        ', then ' 0.4' + pad to 6 = ' 0.4  '
        assert_eq!(out.body, vec![" Chrome        0.4  "]);
    }

    #[test]
    fn drop_memory_first_matches_js() {
        let rows = vec![row("a", 1, 0.0, 0.0)];
        let out = draw_table(21, &rows);
        // JS-verified: width 21 → columns [Command, CPU %], widths 15/6
        assert_eq!(out.process_width, 15);
        assert_eq!(out.title, "{bold} Command        CPU %{/bold}\n");
    }

    #[test]
    fn w24_keeps_count() {
        let rows = vec![row("Chrome", 4, 0.4, 1.0)];
        let out = draw_table(24, &rows);
        assert_eq!(out.process_width, 12); // Command = 12, boundary stays
        assert_eq!(out.title, "{bold} Command     CPU % Count{/bold}\n");
        assert_eq!(out.body, vec![" Chrome      0.4   4    "]);
    }

    #[test]
    fn wide_padding_steps() {
        let rows = vec![row("x", 0, 0.0, 0.0)];
        let out = draw_table(51, &rows); // padding 2
                                         // JS-verified oracle
        assert_eq!(
            out.title,
            "{bold} Command                    CPU %  Count  Memory % {/bold}\n"
        );
    }

    #[test]
    fn truncate_long_command_matches_js() {
        let rows = vec![row("VeryLongCommandNameThatOverflows", 4, 0.4, 1.0)];
        let out = draw_table(60, &rows);
        // padding 2 → columns all 4; widths: Command 36, CPU % 7, Count 7, Memory % 10
        assert_eq!(out.process_width, 36);
        assert_eq!(
            out.body[0],
            " VeryLongCommandNameThatOverflows    0.4    4      1.0      "
        );
    }
}
