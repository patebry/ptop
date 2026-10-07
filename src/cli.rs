//! Hand-rolled argv parser (CONTRACTS §C14). Mirrors vtop's commander options
//! with JavaScript integer-prefix parsing and Node timer bounds.

#[derive(Debug, Clone)]
pub struct Args {
    pub theme: String,
    pub quit_after: i64,      // 0 = run forever
    pub update_interval: i64, // 300
    pub mouse: bool,
    pub parity: bool,            // --vtop-parity
    pub capture: Option<String>, // --capture PATH
    pub version: bool,
    pub help: bool,
    pub error: Option<String>,
    pub brand: &'static str,
}

impl Default for Args {
    fn default() -> Self {
        Args {
            theme: "parallax".into(),
            quit_after: 0,
            update_interval: 300,
            mouse: true,
            parity: true,
            capture: None,
            version: false,
            help: false,
            error: None,
            brand: "ptop",
        }
    }
}

/// JavaScript parseInt-style signed decimal prefix, with leading whitespace.
pub fn parse_lenient_int(s: &str) -> Option<i64> {
    let trimmed = s.trim_start();
    let sign = usize::from(trimmed.starts_with(['+', '-']));
    let end = sign
        + trimmed[sign..]
            .bytes()
            .take_while(u8::is_ascii_digit)
            .count();
    if end == sign {
        None
    } else {
        trimmed[..end].parse().ok()
    }
}

/// Commander 2.11 splits short clusters and long `--option=value` tokens.
fn normalize(argv: &[String]) -> Vec<String> {
    let mut normalized = Vec::new();
    let mut literal = false;
    let mut capture_value = false;
    for arg in argv {
        if literal || capture_value {
            normalized.push(arg.clone());
            capture_value = false;
        } else if arg == "--" {
            normalized.push(arg.clone());
            literal = true;
        } else if arg.starts_with("--") {
            if let Some((flag, value)) = arg.split_once('=') {
                normalized.extend([flag.into(), value.into()]);
            } else {
                normalized.push(arg.clone());
                capture_value = arg == "--capture";
            }
        } else if arg.starts_with('-') && arg.len() > 1 {
            normalized.extend(arg[1..].chars().map(|c| format!("-{c}")));
        } else {
            normalized.push(arg.clone());
        }
    }
    normalized
}

pub fn parse(argv: &[String]) -> Args {
    let mut a = Args::default();
    let argv = normalize(argv);
    let mut i = 0;
    let mut positional = false;
    while i < argv.len() {
        let arg = argv[i].as_str();
        match arg {
            "-V" | "--version" => {
                a.version = true;
                break;
            }
            "-h" | "--help" => a.help = true,
            "--" => {
                positional |= i + 1 < argv.len();
                break;
            }
            "--no-mouse" => a.mouse = false,
            "--vtop-parity" => {
                a.parity = true;
                a.brand = "vtop";
            }
            "--fix" => {
                a.parity = false;
                a.brand = "ptop";
            }
            "--capture" => {
                i += 1;
                a.capture = argv.get(i).cloned();
                if a.capture.is_none() {
                    a.error = Some("option `--capture <fixture.json>' argument missing".into());
                    break;
                }
            }
            "-t" | "--theme" | "--quit-after" | "--update-interval" => {
                if let Some(next) = argv.get(i + 1).filter(|s| !s.starts_with('-') || *s == "-") {
                    match arg {
                        "-t" | "--theme" => a.theme = next.clone(),
                        "--quit-after" => {
                            a.quit_after = if next == "0" {
                                0
                            } else {
                                let seconds = parse_lenient_int(next).unwrap_or(-1);
                                if (1..=2_147_483).contains(&seconds) {
                                    seconds
                                } else {
                                    -1
                                }
                            };
                        }
                        _ => a.update_interval = parse_lenient_int(next).unwrap_or(1),
                    }
                    i += 1;
                }
            }
            _ if arg.starts_with('-') && arg != "-" => {
                a.error
                    .get_or_insert_with(|| format!("unknown option `{arg}'"));
                if argv.get(i + 1).is_some_and(|s| !s.starts_with('-')) {
                    i += 1;
                }
            }
            _ => positional = true,
        }
        i += 1;
    }
    if positional {
        a.help = false;
        a.error = None;
    }
    if a.help || a.version {
        a.error = None;
    }
    if !(1..=2_147_483_647).contains(&a.update_interval) {
        a.update_interval = 1;
    }
    a
}

/// Compatibility text from vtop 0.6.1 / commander 2.11.0 helpInformation().
pub const PARITY_HELP: &str = "
  Usage: vtop [options]


  Options:

    -t, --theme  [name]               set the vtop theme [acid|becca|brew|certs|dark|gooey|gruvbox|monokai|nord|parallax|seti|wizard]
    --no-mouse                        Disables mouse interactivity
    --quit-after [seconds]            Quits vtop after interval
    --update-interval [milliseconds]  Interval between updates
    -V, --version                     output the version number
    -h, --help                        output usage information
";

pub const HELP: &str = "\
Usage: ptop [options]

Options:
  -t, --theme [name]                set the theme [acid, becca, brew, certs, dark, gooey, gruvbox, monokai, nord, parallax, seti, wizard]
  --no-mouse                        disable mouse input
  --quit-after [seconds]            quits after interval
  --update-interval [milliseconds]  interval between updates
  --vtop-parity                     bug-for-bug vtop mode (XOR memory math, vtop branding)
  --fix                             fixed sensor math + ptop branding
  --capture <fixture.json>          offline frame dump for the proof harness
  -V, --version                     output the version number
  -h, --help                        output usage information
";

pub fn usage() -> &'static str {
    HELP
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parity_and_mouse_are_default_with_explicit_opt_outs() {
        let default = parse(&[]);
        assert!(default.parity && default.mouse);
        assert_eq!(default.brand, "ptop");
        assert_eq!(parse(&["--vtop-parity".into()]).brand, "vtop");
        let fixed = parse(&["--fix".into(), "--no-mouse".into()]);
        assert!(!fixed.parity && !fixed.mouse);
        assert_eq!(fixed.brand, "ptop");
        assert!(parse(&["--fix".into(), "--vtop-parity".into()]).parity);
        assert!(!parse(&["--vtop-parity".into(), "--fix".into()]).parity);
    }
    #[test]
    fn commander_errors_and_node_timer_coercion() {
        assert_eq!(
            parse(&["--bogus".into()]).error.as_deref(),
            Some("unknown option `--bogus'")
        );
        assert!(parse(&["--".into(), "--bogus".into()]).error.is_none());
        assert_eq!(
            parse(&["-tnord".into()]).error.as_deref(),
            Some("unknown option `-n'")
        );
        assert_eq!(parse(&["--update-interval=abc".into()]).update_interval, 1);
        assert_eq!(
            parse(&["--update-interval=300ms".into()]).update_interval,
            300
        );
        assert_eq!(
            parse(&["--update-interval=2147483648".into()]).update_interval,
            1
        );
        assert_eq!(parse(&["--quit-after=abc".into()]).quit_after, -1);
        assert_eq!(parse_lenient_int("  +25ms"), Some(25));
    }
}
