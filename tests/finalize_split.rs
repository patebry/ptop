// Split finalize into its two halves: the map (Row building) and the sort.
use ptop::jsnum::{js_to_fixed, js_to_string};
use ptop::sensors::parse_ps;

use ptop::table::Row;
use std::time::Instant;

#[test]
fn finalize_stage_split() {
    let ps_out = std::fs::read_to_string("tests/fixtures_ps/workload_ps.txt").unwrap();
    let groups = parse_ps(&ps_out, true);
    let cores = 10usize;

    // stage A: Row building only (the map)
    let t0 = Instant::now();
    for _ in 0..300 {
        let rows: Vec<Row> = groups
            .iter()
            .map(|g| Row {
                command: g.comm.clone(),
                count: js_to_string(g.count as f64),
                cpu: js_to_fixed(g.cpu_raw / cores as f64, 1),
                mem: js_to_fixed(g.mem_raw, 1),
                cpu_raw: g.cpu_raw,
                mem_raw: g.mem_raw,
            })
            .collect();
        std::hint::black_box(&rows);
    }
    let map_cost = t0.elapsed() / 300;

    // stage B: sort only, over prebuilt rows
    let rows: Vec<Row> = groups
        .iter()
        .map(|g| Row {
            command: g.comm.clone(),
            count: js_to_string(g.count as f64),
            cpu: js_to_fixed(g.cpu_raw / cores as f64, 1),
            mem: js_to_fixed(g.mem_raw, 1),
            cpu_raw: g.cpu_raw,
            mem_raw: g.mem_raw,
        })
        .collect();
    let t0 = Instant::now();
    for _ in 0..300 {
        let mut indexed: Vec<(usize, Row)> = rows
            .iter()
            .enumerate()
            .map(|(i, r)| (i, r.clone()))
            .collect();
        indexed.sort_by(|(ia, a), (ib, b)| {
            let cmp = b
                .cpu_raw
                .partial_cmp(&a.cpu_raw)
                .unwrap_or(std::cmp::Ordering::Equal);
            if cmp != std::cmp::Ordering::Equal {
                cmp
            } else {
                ia.cmp(ib)
            }
        });
        std::hint::black_box(&indexed);
    }
    let sort_cost = t0.elapsed() / 300;
    println!(
        "STAGE A (build rows): {:?}/finalize  STAGE B (sort): {:?}/finalize",
        map_cost, sort_cost
    );
}
