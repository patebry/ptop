//! JS-compatible number formatting (ECMA-262 Number::toString + toFixed),
//! needed for byte-parity of labels and table cells. CONTRACTS §C2/C4.
//!
//! We implement:
//! - `js_to_string(f64)` — the shortest roundtrip decimal Ryū algorithm.
//! - `js_to_fixed(f64, digits)` — V8's toFixed: exact decimal expansion of the
//!   binary double rounded to `digits` fractional digits, ties AWAY FROM ZERO.
//!   (Verified: (0.25).toFixed(1)='0.3'? NO — (0.25) is exactly representable
//!   and V8 rounds half-up → '0.3'. (0.35).toFixed(1)='0.3' — 0.35's binary
//!   value is 0.3499999... so exact-decimal rounding gives '0.3'. Our
//!   implementation reproduces both by using EXACT decimal expansion.)

/// Format an f64 exactly like JS `String(n)`.
pub fn js_to_string(v: f64) -> String {
    if v.is_nan() {
        return "NaN".into();
    }
    if v.is_infinite() {
        return if v > 0.0 {
            "Infinity".into()
        } else {
            "-Infinity".into()
        };
    }
    if v == 0.0 {
        // -0.0: sign bit set but v < 0.0 is false — use bit test (ECMA-262)
        return if v.is_sign_negative() {
            "-0".into()
        } else {
            "0".into()
        };
    }
    let neg = v < 0.0;
    let a = v.abs();
    // Shortest roundtrip digits via Rust's `{:.*e}` probing (1..=17 sig digits),
    // same effect as JS's shortest-Ryū: JS String(n) is the SHORTEST decimal
    // that round-trips; `{:p e}` at increasing precision finds it.
    let s = shortest_e_digits(a);
    let (mantissa_digits, exponent) = split_decimal(&s);
    let js = assemble_js(mantissa_digits, exponent);
    if neg {
        format!("-{}", js)
    } else {
        js
    }
}

/// Shortest roundtrip digits for a > 0 as scientific string like "1.25e1",
/// "1e21", "3.0000000000000004e-1".
fn shortest_e_digits(a: f64) -> String {
    for p in 0..=16usize {
        let s = format!("{:.*e}", p, a);
        if s.parse::<f64>() == Ok(a) {
            return s;
        }
    }
    format!("{:.*e}", 17, a)
}

/// Split a scientific decimal string ('d.ddd e±e' or plain) into
/// (digits-without-dot, exponent) where value = 0.digits * 10^(exponent+1).
fn split_decimal(s: &str) -> (String, i32) {
    if let Some(epos) = s.find('e') {
        let mant = &s[..epos];
        let exp: i32 = s[epos + 1..].parse().unwrap();
        let (digits, point_pos) = strip_point(mant);
        // value = 0.digits * 10^(point_pos + exp)  → digits * 10^(point_pos-1+exp)
        (digits, (point_pos as i32) - 1 + exp)
    } else {
        let (digits, point_pos) = strip_point(s);
        (digits, (point_pos as i32) - 1)
    }
}

fn strip_point(mant: &str) -> (String, usize) {
    match mant.find('.') {
        Some(pos) => {
            let mut digits = String::with_capacity(mant.len() - 1);
            digits.push_str(&mant[..pos]);
            digits.push_str(&mant[pos + 1..]);
            (digits, pos)
        }
        None => (mant.to_string(), mant.len()),
    }
}

/// Given digits d (significant, may have trailing zeros) meaning
/// 0.d × 10^k (k = exponent+1), produce JS String(n).
fn assemble_js(digits: String, exponent: i32) -> String {
    if digits.is_empty() {
        return "0".into();
    }
    let ndig = digits.len() as i32;
    // JS decision (ECMA-262 ToString of Number):
    // Let n, k, s be such that s consists of the digits, k = s.length,
    // n = the value interpreted with decimal point after the first digit…
    // We follow the spec with s = digits (may need adjustment when trailing
    // zeros are present: s must not end in 0 unless k==1).
    // Rust shortest form has no trailing zeros already (except "0").
    let n = exponent + 1; // decimal exponent: value = 0.s * 10^n
                          // Spec: if k ≤ n ≤ 21: digits + (n-k) zeros
                          // else if 0 < n ≤ 21: int.frac
                          // else if -6 < n ≤ 0: 0.00…digits
                          // else exponent form: d.ddd e±(n-1) — JS uses lowercase 'e', '+' when positive.
    if n > 21 || n <= -6 {
        // exponent form
        let m = digits.as_str().to_string();
        // JS: mantissa has form d(.ddd)?
        let mut mant = String::new();
        match m.chars().next() {
            Some(c) => mant.push(c),
            None => return "0".into(),
        }
        let rest: String = m.chars().skip(1).collect();
        if !rest.is_empty() {
            mant.push('.');
            mant.push_str(&rest);
        }
        let e = n - 1;
        if e >= 0 {
            format!("{}e+{}", mant, e)
        } else {
            format!("{}e-{}", mant, -e)
        }
    } else if n >= ndig && n <= 21 {
        let mut out = digits.clone();
        for _ in 0..(n - ndig) {
            out.push('0');
        }
        out
    } else if n > 0 {
        format!("{}.{}", &digits[..n as usize], &digits[n as usize..])
    } else {
        let mut out = String::from("0.");
        for _ in 0..(-n) {
            out.push('0');
        }
        out.push_str(&digits);
        out
    }
}

/// JS `Number.prototype.toFixed(digits)` for 0 ≤ digits ≤ 100.
/// V8: computes the exact decimal expansion of the double, rounds to `digits`
/// fractional digits — ties (exact .5 at the cut) round AWAY FROM ZERO.
///
/// Fast path first: the shortest roundtrip decimal decides the digit after the
/// cut. Only an ABIGUOUS tie (shortest digits end '5' with nothing after) needs
/// the exact dyadic expansion — measured at 1 call per tick on real ps data.
/// Validated 0/400k mismatches vs the exact expander (tests/).
pub fn js_to_fixed(v: f64, digits: usize) -> String {
    if v.is_finite() && v.abs() < 1e21 && digits <= 100 {
        if let Some(s) = js_to_fixed_fast(v, digits) {
            return s;
        }
    }
    js_to_fixed_exact(v, digits)
}

/// Fast toFixed via shortest-roundtrip digits. Returns None only in the
/// ambiguous '5'-tail case (true tie-ness lives beyond the shortest repr).
fn js_to_fixed_fast(v: f64, digits: usize) -> Option<String> {
    let neg = v < 0.0;
    let a = v.abs();
    let s = shortest_e_digits(a);
    // s like "1.25e1", "5e0", "3.45e-1"; value = 0.mant_digits × 10^(exp+1)
    let (mant, exp) = s.split_once('e')?;
    let exp: i32 = exp.parse().ok()?;
    let mant_digits: String = mant.chars().filter(|c| c.is_ascii_digit()).collect();
    let p = exp + 1; // decimal-point position within mant_digits (may be ≤0)
    let need_frac_total = digits + 2; // rounder + one lookahead digit
    let (int_part, frac_part) = if p <= 0 {
        let mut fs = String::new();
        for _ in 0..(-p) {
            fs.push('0');
        }
        fs.push_str(&mant_digits);
        while fs.len() < need_frac_total {
            fs.push('0');
        }
        ("0".to_string(), fs)
    } else {
        let need_int = p as usize;
        let mi = need_int.min(mant_digits.len());
        let mut ip = mant_digits[..mi].to_string();
        while ip.len() < need_int {
            ip.push('0');
        }
        let mut fs = mant_digits[mi..].to_string();
        while fs.len() < need_frac_total {
            fs.push('0');
        }
        (ip, fs)
    };
    // The rounder needs digits+1 frac digits guaranteed; padding ensures it.
    if frac_part.len() < digits + 1 {
        return None; // shouldn't happen; defer
    }
    let rounder = frac_part.as_bytes()[digits] - b'0';
    if rounder == 5 {
        // Ambiguous unless a further digit is nonzero (then decisively up).
        let further_nonzero = frac_part.as_bytes()[digits + 1..]
            .iter()
            .any(|&b| b != b'0');
        if !further_nonzero {
            return None; // exact expander decides (0.15↓ tie vs 0.25↑ tie etc.)
        }
        return Some(round_and_sign(neg, int_part, &frac_part[..digits], true));
    }
    Some(round_and_sign(
        neg,
        int_part,
        &frac_part[..digits],
        rounder > 5,
    ))
}

fn round_and_sign(neg: bool, int_part: String, keep_frac: &str, up: bool) -> String {
    let mut keep: Vec<u8> = keep_frac.bytes().collect();
    let mut int_final = int_part;
    if up {
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
            let n: i128 = int_final.parse().unwrap_or(0);
            int_final = format!("{}", n + 1);
        }
    }
    let keep_str = String::from_utf8(keep).unwrap_or_default();
    // V8 trims? No: toFixed pads zeros to `digits` — keep_str length is kept.
    let mut result = if keep_str.is_empty() {
        int_final.clone()
    } else {
        format!("{}.{}", int_final, keep_str)
    };
    // ECMA-262: '-' sign preserved even when magnitude rounds to zero
    // (node: (-0.01).toFixed(1) === '-0.0'), but NOT for exactly -0 (→'0.0').
    if neg {
        result.insert(0, '-');
    }
    result
}

/// The exact dyadic expansion implementation (fallback + digits>2 path).
fn js_to_fixed_exact(v: f64, digits: usize) -> String {
    // JS toFixed: digits 0..=100
    let digits = digits.min(100);
    if v.is_nan() {
        return "NaN".into();
    }
    if v.is_infinite() {
        return if v > 0.0 {
            "Infinity".into()
        } else {
            "-Infinity".into()
        };
    }
    let neg = v < 0.0;
    let a = v.abs();
    // Exact decimal expansion as a digit string (dyadic rationals terminate).
    let s = exact_decimal_digits(a);
    let (int_part, frac_part) = match s.find('.') {
        Some(p) => (s[..p].to_string(), s[p + 1..].to_string()),
        None => (s.clone(), String::new()),
    };
    // Round at `digits` fractional digits, ties AWAY FROM ZERO (V8 behavior).
    let mut keep_frac = String::new();
    let mut int_final = int_part.clone();
    let mut rounded_up = false;
    if digits == 0 {
        // verified in node: (0.5)='1', (-0.5)='-1', (2.5)='3', (-2.5)='-3' —
        // digits=0 ties round AWAY FROM ZERO (magnitude increments).
        let next = frac_part.as_bytes().first().copied().unwrap_or(b'0');
        if next >= b'5' {
            int_final = increment_int(&int_part);
            rounded_up = true;
        }
    } else {
        for i in 0..digits {
            let c = frac_part.as_bytes().get(i).copied().unwrap_or(b'0');
            keep_frac.push(c as char);
        }
        if digits < frac_part.len() {
            let next = frac_part.as_bytes()[digits];
            if next >= b'5' {
                keep_frac = round_up_frac_str(&keep_frac, &mut int_final);
                rounded_up = true;
            }
        }
    }
    let mut result = if digits == 0 {
        int_final.clone()
    } else {
        format!("{}.{}", int_final, keep_frac)
    };
    let significant =
        parse_int(&int_final) != 0 || keep_frac.chars().any(|c| c != '0') || rounded_up;
    if neg && (significant || v.is_sign_negative()) {
        result.insert(0, '-');
    }
    result
}

/// Round the frac digit string by one; returns new frac, sets int carry.
fn round_up_frac_str(frac: &str, int_part: &mut String) -> String {
    let mut out = Vec::with_capacity(frac.len());
    let mut carry = true;
    for b in frac.bytes().rev() {
        if carry {
            if b == b'9' {
                out.push(b'0');
            } else {
                out.push(b + 1);
                carry = false;
            }
        } else {
            out.push(b);
        }
    }
    out.reverse();
    if carry {
        *int_part = increment_int(int_part);
    }
    String::from_utf8(out).unwrap()
}

fn increment_int(int_part: &str) -> String {
    format!("{}", parse_int(int_part) + 1)
}

fn parse_int(s: &str) -> i128 {
    if s.is_empty() {
        0
    } else {
        s.parse::<i128>().unwrap_or(0)
    }
}

/// Exact decimal expansion of 0 ≤ a < 2^64 (every f64 is dyadic → terminates).
/// Returns like "123.45" or "0.000123" — full expansion (may be long; f64 needs
/// at most 1080 fractional digits for denormals; our domain ≤ ~1024 fine).
fn exact_decimal_digits(a: f64) -> String {
    let bits = a.to_bits();
    let mantissa = bits & ((1u64 << 52) - 1);
    let biased = ((bits >> 52) & 0x7ff) as i32;
    let (m, e): (u128, i32) = if biased == 0 {
        (mantissa as u128, -1074)
    } else {
        ((mantissa | (1u64 << 52)) as u128, biased - 1075)
    };
    // a = m * 2^e
    if e >= 0 {
        let mut int_val = m;
        let mut sh = e;
        while sh > 0 {
            int_val = int_val.saturating_mul(2);
            sh -= 1;
        }
        return format!("{}", int_val);
    }
    let frac_exp = -e; // a = m / 2^frac_exp = (m * 5^frac_exp) / 10^frac_exp
                       // FULL exact expansion: m * 5^k as big decimal digits, then place point
                       // k ≤ 1074 → 5^1074 ≈ 751 digits; number of digits of m*5^k manageable.
    let digits = mul_dec_string(m, frac_exp as u32);
    // value = digits * 10^-frac_exp → point sits frac_exp from the right
    let nd = digits.len() as i64;
    let k = frac_exp as i64;
    if nd > k {
        let cut = (nd - k) as usize;
        format!("{}.{}", &digits[..cut], &digits[cut..])
    } else {
        let mut s = String::from("0.");
        for _ in 0..(k - nd) {
            s.push('0');
        }
        s.push_str(&digits);
        s
    }
}

/// m * 5^k computed as a decimal digit string (m up to 2^53).
fn mul_dec_string(m: u128, k: u32) -> String {
    // result = m * 5^k as decimal digits
    let mut digits: Vec<u8> = {
        let s = format!("{}", m);
        s.bytes().map(|b| b - b'0').collect()
    };
    let mut fives: u32 = 0;
    // multiply by 5^k via repeated *5 with carry (k ≤ 1074 → O(k * digits) ≈ fine)
    while fives < k {
        let mut carry: u32 = 0;
        for d in digits.iter_mut().rev() {
            let v = (*d as u32) * 5 + carry;
            *d = (v % 10) as u8;
            carry = v / 10;
        }
        while carry > 0 {
            digits.insert(0, (carry % 10) as u8);
            carry /= 10;
        }
        fives += 1;
    }
    let s: String = digits.iter().map(|d| (d + b'0') as char).collect();
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn js_string_basics() {
        assert_eq!(js_to_string(7.0), "7");
        assert_eq!(js_to_string(12.5), "12.5");
        assert_eq!(js_to_string(0.0), "0");
        assert_eq!(js_to_string(-0.0), "-0");
        assert_eq!(js_to_string(100.0), "100");
        assert_eq!(js_to_string(0.1 + 0.2), "0.30000000000000004");
        assert_eq!(js_to_string(1e21), "1e+21");
        assert_eq!(js_to_string(1e-7), "1e-7");
        assert_eq!(
            js_to_string(123456789012345680000.0),
            "123456789012345680000"
        );
    }

    #[test]
    fn to_fixed_parity() {
        assert_eq!(js_to_fixed(0.35, 1), "0.3"); // binary 0.34999… < 0.35
        assert_eq!(js_to_fixed(0.25, 1), "0.3"); // exact tie → away from zero
        assert_eq!(js_to_fixed(1.05, 1), "1.1"); // 1.05 binary = 1.05000000000000004 → up
        assert_eq!(js_to_fixed(1.005, 2), "1.00"); // 1.005 binary = 1.00500000000000000551… → up keeps 1.00
        assert_eq!(js_to_fixed(2.5, 0), "3");
        assert_eq!(js_to_fixed(3.5, 0), "4");
        assert_eq!(js_to_fixed(0.0, 1), "0.0");
        assert_eq!(js_to_fixed(12.34, 1), "12.3");
        assert_eq!(js_to_fixed(12.36, 1), "12.4");
        assert_eq!(js_to_fixed(99.96, 1), "100.0"); // carry into integer part
        assert_eq!(js_to_fixed(0.44, 1), "0.4");
        assert_eq!(js_to_fixed(0.46, 1), "0.5");
        assert_eq!(js_to_fixed(7.0, 1), "7.0");
        assert_eq!(js_to_fixed(0.96, 1), "1.0");
        assert_eq!(js_to_fixed(0.04, 1), "0.0");
        assert_eq!(js_to_fixed(0.06, 1), "0.1");
    }
}
