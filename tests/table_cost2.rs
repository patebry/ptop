// Cost attribution INSIDE the table path: finalize (sort) vs draw_table (format).
use ptop::sensors::{finalize, parse_ps, SortKey};
use std::time::Instant;

#[test]
fn table_stage_costs() {
    let ps_out = std::fs::read_to_string("tests/fixtures_ps/workload_ps.txt").unwrap();
    let groups = parse_ps(&ps_out, true);
    println!("groups: {}", groups.len());

    // warm
    for _ in 0..20 {
        let _ = finalize(&groups, 10, SortKey::Cpu);
    }

    let t0 = Instant::now();
    for _ in 0..300 {
        let _ = finalize(&groups, 10, SortKey::Cpu);
    }
    let fin = t0.elapsed() / 300;

    let sorted = finalize(&groups, 10, SortKey::Cpu);
    let t0 = Instant::now();
    for _ in 0..300 {
        let _ = ptop::table::draw_table(49, &sorted);
    }
    let fmt = t0.elapsed() / 300;

    // full per-tick path as app.rs does it: finalize + format
    let t0 = Instant::now();
    for _ in 0..300 {
        let s = finalize(&groups, 10, SortKey::Cpu);
        let _ = ptop::table::draw_table(49, &s);
    }
    let both = t0.elapsed() / 300;

    println!(
        "finalize(sort): {:?}  draw_table(fmt): {:?}  both: {:?}",
        fin, fmt, both
    );
    assert!(both.as_micros() > 0);
}
