// Cost atomization: which part of js_to_fixed dominates? (exact_decimal_digits
// does a 5^k big-decimal expansion for EVERY row × 2 columns per finalize —
// O(751-digit work × 1170 calls/tick!). Test one call's cost.
use ptop::jsnum::js_to_fixed;
use std::time::Instant;

#[test]
fn js_to_fixed_cost() {
    // warm
    for i in 0..100 {
        let _ = js_to_fixed(0.4 + i as f64, 1);
    }
    let t0 = Instant::now();
    for i in 0..1000 {
        let _ = js_to_fixed(0.4 + i as f64, 1);
    }
    let per = t0.elapsed() / 1000;
    println!(
        "js_to_fixed(v,1) = {:?}/call → ×1170 rows = {:?}",
        per,
        per * 1170
    );

    // and js_to_string (cheaper path?) for comparison
    let t0 = Instant::now();
    for i in 0..1000 {
        let _ = ptop::jsnum::js_to_string(12.4 + i as f64);
    }
    let per2 = t0.elapsed() / 1000;
    println!("js_to_string      = {:?}/call", per2);
}
