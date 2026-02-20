#![forbid(unsafe_code)]

use std::env;
use std::process::ExitCode;

fn usage() {
    eprintln!("usage:");
    eprintln!("  cargo xtask sdk export-ts [--out <dir>]");
    eprintln!("  cargo xtask sdk validate");
}

fn run_sdk(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("export-ts") => Ok(()),
        Some("validate") => Ok(()),
        _ => Err("unknown sdk subcommand".to_string()),
    }
}

fn run(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("sdk") => run_sdk(&args[1..]),
        _ => Err("unknown command".to_string()),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        usage();
        return ExitCode::from(2);
    }
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err}");
            usage();
            ExitCode::from(2)
        }
    }
}
