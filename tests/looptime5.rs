// Full loop period at steady state with 585-row table + 1ms sleep, post-memoization.
use ptop::app::{compose, FrameState};
use ptop::chart::Values;
use ptop::sensors::{finalize, parse_ps, SortKey};
use ptop::theme::{load, resolve};
use std::time::{Duration, Instant};

#[test]
fn steady_state_period_585_rows() {
    let theme = resolve(&load("parallax").unwrap());
    let ps_out = std::fs::read_to_string("tests/fixtures_ps/workload_ps.txt").unwrap();
    let groups = parse_ps(&ps_out, true);
    let rows = finalize(&groups, 10, SortKey::Cpu);
    let mut vals = Values::new();
    let mut memv = Values::new();
    for t in 0..98i64 {
        vals.assign(t, Some(20.0));
        memv.assign(t, Some(40.0));
    }
    let t0 = Instant::now();
    let mut ticks = 0u32;
    for t in 98..398i64 {
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
        std::thread::sleep(Duration::from_millis(1));
        ticks += 1;
    }
    let per = t0.elapsed() / ticks;
    println!("steady 585-row ticks: {:?}/tick", per);
    println!(
        "(includes 1.25ms sleep) — compose portion ≈ {:?}",
        per - Duration::from_millis(1)
    );
}
