// Which line inside stage A? Two candidates: 5^k big-decimal expansion
// (exact_decimal_digits) and the shortest-roundtrip probe loop.
use ptop::jsnum::{js_to_fixed, js_to_string};
use std::time::Instant;

#[test]
fn micro_split() {
    // 585 rows × 2 fixed-1 calls
    let vals: Vec<f64> = (0..1170).map(|i| 0.1 + (i % 90) as f64 / 7.0).collect();

    let t0 = Instant::now();
    for &v in &vals {
        std::hint::black_box(js_to_fixed(v, 1));
    }
    let fixed1 = t0.elapsed();

    let t0 = Instant::now();
    for &v in &vals {
        std::hint::black_box(js_to_string(v));
    }
    let tostring = t0.elapsed();

    println!(
        "1170× js_to_fixed(_,1): {:?}   1170× js_to_string: {:?}",
        fixed1, tostring
    );
}
