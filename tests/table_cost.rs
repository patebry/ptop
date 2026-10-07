// Cost split: compose() with vs without the process table (empty procs vs 300 rows).
use ptop::app::{compose, FrameState};
use ptop::chart::Values;
use ptop::sensors::parse_ps;
use ptop::table::Row;
use ptop::theme::{load, resolve};
use std::time::Instant;

fn build<'a>(
    vals: &'a mut Values,
    memv: &'a mut Values,
    procs: &'a [Row],
    theme: &'a ptop::theme::ResolvedTheme,
    t: i64,
) -> FrameState<'a> {
    FrameState {
        cols: 100,
        rows: 24,
        theme,
        hostname: "test.local",
        brand: "vtop",
        clock: "01:02:03 ",
        loadavg: (1.0, 2.0, 3.0),
        cpu_initialized: true,
        cpu_value: 20.0,
        cpu_is_undefined: false,
        cpu_values: vals,
        cpu_scale: 1.0,
        position: t,
        mem_initialized: true,
        mem_value: 40.0,
        mem_is_undefined: false,
        mem_values: memv,
        mem_scale: 1.0,
        procs,
        selected: 0,
        graph_scale: 1.0,
    }
}

#[test]
fn compose_cost_split() {
    let theme = resolve(&load("parallax").unwrap());
    let ps_out = std::fs::read_to_string("tests/fixtures_ps/workload_ps.txt").unwrap();
    let groups = parse_ps(&ps_out, true);
    let rows = ptop::sensors::finalize(&groups, 10, ptop::sensors::SortKey::Cpu);
    println!("rows: {}", rows.len());

    // WARM
    for t in 98..100i64 {
        let mut v1 = Values::new();
        let mut m1 = Values::new();
        for t0 in 0..98i64 {
            v1.assign(t0, Some(20.0));
            m1.assign(t0, Some(40.0));
        }
        let _ = compose(&mut build(&mut v1, &mut m1, &rows, &theme, t));
    }

    // with table
    let t0 = Instant::now();
    let mut ticks = 0;
    for t in 200..500i64 {
        let mut v1 = Values::new();
        let mut m1 = Values::new();
        for t0 in 0..98i64 {
            v1.assign(t0, Some(20.0));
            m1.assign(t0, Some(40.0));
        }
        let _ = compose(&mut build(&mut v1, &mut m1, &rows, &theme, t));
        ticks += 1;
    }
    let with_table = t0.elapsed() / ticks as u32;

    // empty table
    let t0 = Instant::now();
    for t in 200..500i64 {
        let mut v1 = Values::new();
        let mut m1 = Values::new();
        for t0 in 0..98i64 {
            v1.assign(t0, Some(20.0));
            m1.assign(t0, Some(40.0));
        }
        let _ = compose(&mut build(&mut v1, &mut m1, &[], &theme, t));
    }
    let without_table = t0.elapsed() / 300;
    println!(
        "compose WITH table: {:?}/tick; WITHOUT: {:?}/tick",
        with_table, without_table
    );
}
