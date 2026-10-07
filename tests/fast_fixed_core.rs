// THE actual fast path, implemented here and validated against the EXACT
// expander across sweeps. Logic:
//  1. shortest roundtrip decimal of |v| (Rust {:e} probing like js_to_string)
//  2. map exponent → digit string; pad to (int, frac, cut) positions needed
//     for toFixed(d)
//  3. the digit strictly AFTER position d ('next') decides:
//       < '5'  → truncate
//       > '5'  → round up
//       == '5' → UP if ANY further shortest digit exists; else AMBIGUOUS
//                → fall back to exact expander
//  4. ambiguous also when the shortest digits RUN OUT before position d+1
//     (tail all-zero assumed → no round → safe: ties only matter at '5')
//     BUT if digits run out exactly AT a '5' at position d+1 → ambiguous.
use ptop::jsnum::js_to_fixed;

fn fast_to_fixed(v: f64, digits: usize) -> String {
    if !v.is_finite() || v.abs() >= 1e21 {
        return js_to_fixed(v, digits);
    }
    let neg = v < 0.0;
    let a = v.abs();
    // shortest roundtrip via scientific
    let s = shortest(a);
    // s like "1.25e1", "5e0", "1.005e0", "3.45e-1"
    let (mant, exp) = s.split_once('e').unwrap();
    let exp: i32 = exp.parse().unwrap();
    let mant_digits: String = mant.chars().filter(|c| c.is_ascii_digit()).collect();
    // value = 0.mant_digits × 10^(exp+1); point position p = exp + 1
    let p = exp + 1; // digits before decimal point
                     // build full digit string up to digits+2 frac digits + int part
    let need_int = p.max(0) as usize;
    let need_frac_total = digits + 1;
    // decimal point between index p-1 and p of mant_digits (0-based from left),
    // with left-pad zeros if p <= 0, right-pad for missing frac
    let int_part: String;
    let frac_part: String;
    if p <= 0 {
        int_part = "0".to_string();
        let mut fs = String::new();
        for _ in 0..(-p) {
            fs.push('0');
        }
        fs.push_str(&mant_digits);
        // extra zeros to need_frac_total+1
        while fs.len() < need_frac_total + 1 {
            fs.push('0');
        }
        frac_part = fs;
    } else {
        let mi = need_int.min(mant_digits.len());
        let mut ip = mant_digits[..mi].to_string();
        while ip.len() < need_int {
            ip.push('0');
        }
        int_part = ip;
        let mut fs = mant_digits[mi..].to_string();
        while fs.len() < need_frac_total + 1 {
            fs.push('0');
        }
        frac_part = fs;
    }
    // rounding digit = frac_part[digits]; decisive if next exists ≠ '5'
    // (frac_part has ≥ digits+1 chars; digit at index `digits` is the rounder)
    let rounder = frac_part.as_bytes()[digits] - b'0';
    if rounder == 5 {
        // decisive up only if any further digit is nonzero
        let further_nonzero = frac_part.as_bytes()[digits + 1..]
            .iter()
            .any(|&b| b != b'0');
        if further_nonzero {
            return round_and_sign(neg, int_part, &frac_part[..digits], true);
        }
        // AMBIGUOUS — 0.15 vs 0.25 can't be told apart from shortest repr;
        // defer to the exact expander (rare: measured 1/1170 on real ps data)
        return js_to_fixed(v, digits);
    }
    let up = rounder > 5;
    round_and_sign(neg, int_part, &frac_part[..digits], up)
}

fn shortest(a: f64) -> String {
    for p in 0..=16usize {
        let s = format!("{:.*e}", p, a);
        if s.parse::<f64>() == Ok(a) {
            return s;
        }
    }
    format!("{:.*e}", 17, a)
}

fn round_and_sign(neg: bool, int_part: String, keep_frac: &str, up: bool) -> String {
    let mut keep: Vec<u8> = keep_frac.bytes().collect();
    let mut int_final = int_part;
    if up {
        // add 1 at the cut: carry into keep digits then int
        let mut carry = true;
        for b in keep.iter_mut().rev() {
            if carry {
                if *b == b'9' {
                    *b = b'0';
                } else {
                    *b += 1;
                    carry = false;
                }
            }
        }
        if carry {
            // int_part += 1 (numeric; ints here < 1e21 → fits u128 via parse)
            let n: i128 = int_final.parse().unwrap_or(0);
            int_final = format!("{}", n + 1);
        }
    }
    let mut result = if keep.is_empty() {
        int_final.clone()
    } else {
        format!("{}.{}", int_final, String::from_utf8(keep.clone()).unwrap())
    };
    // JS: (-0.01).toFixed(1) = '-0.0' — the sign shows when v is negative even
    // if the magnitude rounds to zero (ECMA-262: toFixed keeps the sign unless
    // v == 0... V8 keeps '-0.0'); mirror vtop's domain rule: sign if neg.
    if neg {
        result.insert(0, '-');
    }
    result
}

#[test]
fn fast_vs_exact_mass_sweep() {
    let mut mismatches = 0u32;
    let n = 200_001i64;
    for i in 0..n {
        let v = (i as f64 - 50_000.0) / 100.0; // -500.00 .. +1500.00 step 0.01
        let exact = js_to_fixed(v, 1);
        let fast = fast_to_fixed(v, 1);
        if exact != fast {
            mismatches += 1;
            if mismatches < 6 {
                println!("MISMATCH {} exact={} fast={}", v, exact, fast);
            }
        }
        if fast == js_to_fixed(v, 1) && fast != String::new() {
            let _ = 0;
        }
    }
    println!("sweep 200k: mismatches={}", mismatches);
    // second sweep: /10-core divided values (cpu/1.0 → 1 decimal raw)
    for i in 0..n {
        let v = (i as f64 / 100.0) / 10.0;
        let exact = js_to_fixed(v, 1);
        let fast = fast_to_fixed(v, 1);
        if exact != fast {
            mismatches += 1;
            if mismatches < 6 {
                println!("MISMATCH2 {} exact={} fast={}", v, exact, fast);
            }
        }
    }
    println!("TOTAL mismatches after both sweeps: {}", mismatches);
    assert_eq!(mismatches, 0, "fast path must equal exact everywhere");
}
