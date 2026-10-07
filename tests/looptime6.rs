// Break compose into stages at 585 rows: header/footer/charts vs process list.
use ptop::app::{compose, FrameState};
use ptop::chart::Values;
use ptop::sensors::{finalize, parse_ps, SortKey};
use ptop::theme::{load, resolve};
use std::time::{Duration, Instant};

#[test]
fn stages() {
    let theme = resolve(&load("parallax").unwrap());
    let ps_out = std::fs::read_to_string("tests/fixtures_ps/workload_ps.txt").unwrap();
    let rows = finalize(&parse_ps(&ps_out, true), 10, SortKey::Cpu);
    let mut vals = Values::new();
    let mut memv = Values::new();
    for t in 0..98i64 {
        vals.assign(t, Some(20.0));
        memv.assign(t, Some(40.0));
    }

    // full compose, fresh screens, repeated
    let t0 = Instant::now();
    for t in 98..198i64 {
        let mut frame = compose(&mut FrameState {
            cols: 100,
            rows: 24,
            theme: &theme,
            hostname: "test.local",
            brand: "vtop",
            clock: "01:02:03 ",
            loadavg: (1.0, 2.0, 3.0),
            cpu_initialized: true,
            cpu_value: 20.0,
            cpu_is_undefined: false,
            cpu_values: &mut vals,
            cpu_scale: 1.0,
            position: t,
            mem_initialized: true,
            mem_value: 40.0,
            mem_is_undefined: false,
            mem_values: &mut memv,
            mem_scale: 1.0,
            procs: &rows,
            selected: 0,
            graph_scale: 1.0,
        });
        let _ = frame.emit_diff();
    }
    let compose_cost = t0.elapsed() / 100;
    // diff only (reuse same screen twice → 0 diff but measures loop overhead)
    let t0 = Instant::now();
    for _ in 0..100 {
        let bytes = "";
        std::hint::black_box(bytes.len());
        std::thread::sleep(Duration::from_micros(0));
    }
    let noop = t0.elapsed() / 100;
    println!(
        "compose+diff(585 rows): {:?}; noop: {:?}",
        compose_cost, noop
    );
}
