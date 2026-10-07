//! ptop entry point — arg dispatch (capture vs live).
//! The binary imports the lib crate (`ptop_lib`) declared via Cargo.

use ptop::{app, capture, cli};

fn main() {
    let argv: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|s| s.to_string_lossy().into_owned())
        .collect();
    let args = cli::parse(&argv);
    if let Some(error) = &args.error {
        eprintln!("\n  error: {error}\n");
        std::process::exit(1);
    }
    if args.version {
        println!(
            "{}",
            if args.brand == "vtop" {
                "0.6.1"
            } else {
                ptop::VERSION
            }
        );
        return;
    }
    if args.help {
        print!(
            "{}",
            if args.brand == "vtop" {
                cli::PARITY_HELP
            } else {
                cli::usage()
            }
        );
        return;
    }
    if let Some(path) = args.capture.clone() {
        std::process::exit(capture::capture(&path));
    }
    let code = app::run(app::RunOpts { args });
    std::process::exit(code);
}
