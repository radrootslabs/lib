use std::process::ExitCode;

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("--help") => {
            println!("fixture-service");
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("usage: fixture-service --help");
            ExitCode::from(2)
        }
    }
}
