#![forbid(unsafe_code)]

mod contract;
mod export_ts;

use std::env;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn usage() {
    eprintln!("usage:");
    eprintln!("  cargo xtask sdk export-ts-models [--out <dir>]");
    eprintln!("  cargo xtask sdk export-ts-constants [--out <dir>]");
    eprintln!("  cargo xtask sdk validate");
}

fn workspace_root() -> Result<PathBuf, String> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let Some(crates_dir) = manifest_dir.parent() else {
        return Err("failed to resolve crates dir".to_string());
    };
    let Some(root) = crates_dir.parent() else {
        return Err("failed to resolve workspace root".to_string());
    };
    Ok(root.to_path_buf())
}

fn parse_out_dir(args: &[String], workspace_root: &Path) -> Result<PathBuf, String> {
    if args.is_empty() {
        return Ok(workspace_root.join("target").join("sdk-export"));
    }
    if args.len() == 2 && args[0] == "--out" {
        return Ok(PathBuf::from(&args[1]));
    }
    Err("invalid export args, expected --out <dir>".to_string())
}

fn export_ts_models(args: &[String]) -> Result<(), String> {
    let root = workspace_root()?;
    let out_dir = parse_out_dir(args, &root)?;
    export_ts::export_ts_models(&root, &out_dir)?;
    eprintln!("exported ts models to {}", out_dir.display());
    Ok(())
}

fn export_ts_constants(args: &[String]) -> Result<(), String> {
    let root = workspace_root()?;
    let out_dir = parse_out_dir(args, &root)?;
    export_ts::export_ts_constants(&root, &out_dir)?;
    eprintln!("exported ts constants to {}", out_dir.display());
    Ok(())
}

fn validate_contract() -> Result<(), String> {
    let root = workspace_root()?;
    let bundle = contract::load_contract_bundle(&root)?;
    contract::validate_contract_bundle(&bundle)?;
    eprintln!(
        "validated contract {} {}",
        bundle.manifest.contract.name, bundle.manifest.contract.version
    );
    eprintln!("contract root: {}", bundle.root.display());
    Ok(())
}

fn run_sdk(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("export-ts-models") => export_ts_models(&args[1..]),
        Some("export-ts-constants") => export_ts_constants(&args[1..]),
        Some("validate") => validate_contract(),
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
