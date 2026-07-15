use std::{fs, path::Path, process::Command};

use crate::{
    check,
    coverage_policy::{CoverageContract, evaluate_report, validate_contract},
    fs::workspace_root,
    generate, wasm,
};

pub fn run(args: &[String]) -> Result<(), String> {
    match args {
        [command] if command == "run" => run_coverage(),
        [] => Err(usage()),
        _ => Err(usage()),
    }
}

fn usage() -> String {
    "usage: cargo xtask coverage run".to_owned()
}

fn run_coverage() -> Result<(), String> {
    let root = workspace_root()?;
    let contract = load_contract(&root)?;
    validate_contract(&contract)?;
    preflight(&contract)?;
    generate::generate_ts()?;
    wasm::generate(&[])?;
    check::check()?;
    clean_report_output(&root, &contract)?;
    run_llvm_cov(&root, &contract)?;
    evaluate_report(&root, &root.join(&contract.report.output), &contract)
}

fn load_contract(root: &Path) -> Result<CoverageContract, String> {
    let path = root.join("contracts").join("coverage.toml");
    let raw = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    toml::from_str(&raw).map_err(|error| format!("failed to parse {}: {error}", path.display()))
}

fn preflight(contract: &CoverageContract) -> Result<(), String> {
    require_command("rustup", &["--version"], "install rustup")?;
    require_command(
        "rustup",
        &[
            "run",
            &contract.toolchain.rust,
            "cargo",
            "llvm-cov",
            "--version",
        ],
        "install cargo-llvm-cov for the SDK Rust toolchain",
    )?;
    let component_output = output(
        "rustup",
        &["component", "list", "--toolchain", &contract.toolchain.rust],
    )
    .map_err(|error| format!("failed to inspect Rust components: {error}"))?;
    if !component_output
        .lines()
        .any(|line| line.starts_with("llvm-tools") && line.contains("(installed)"))
    {
        return Err(format!(
            "missing llvm-tools-preview for Rust toolchain {}; run `rustup component add llvm-tools-preview --toolchain {}`",
            contract.toolchain.rust, contract.toolchain.rust
        ));
    }
    let target_output = output(
        "rustup",
        &["target", "list", "--toolchain", &contract.toolchain.rust],
    )
    .map_err(|error| format!("failed to inspect Rust targets: {error}"))?;
    let expected_target = format!("{} (installed)", contract.toolchain.wasm_target);
    if !target_output.lines().any(|line| line == expected_target) {
        return Err(format!(
            "missing Rust target {}; run `rustup target add {} --toolchain {}`",
            contract.toolchain.wasm_target, contract.toolchain.wasm_target, contract.toolchain.rust
        ));
    }
    require_command("wasm-pack", &["--version"], "install wasm-pack")?;
    require_command("pnpm", &["--version"], "install pnpm")?;
    Ok(())
}

fn require_command(command: &str, args: &[&str], install_hint: &str) -> Result<(), String> {
    output(command, args)
        .map(|_| ())
        .map_err(|error| format!("missing {command}: {error}; {install_hint}"))
}

fn output(command: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new(command)
        .args(args)
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn clean_report_output(root: &Path, contract: &CoverageContract) -> Result<(), String> {
    let output_path = root.join(&contract.report.output);
    if let Some(parent) = output_path.parent()
        && parent.exists()
    {
        fs::remove_dir_all(parent)
            .map_err(|error| format!("failed to remove {}: {error}", parent.display()))?;
    }
    Ok(())
}

fn run_llvm_cov(root: &Path, contract: &CoverageContract) -> Result<(), String> {
    let output_path = root.join(&contract.report.output);
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    let status = Command::new("rustup")
        .current_dir(root)
        .args([
            "run",
            &contract.toolchain.rust,
            "cargo",
            "llvm-cov",
            "--workspace",
            "--all-features",
            "--summary-only",
            "--json",
            "--output-path",
            &contract.report.output,
            "--ignore-filename-regex",
            &contract.report.ignore_filename_regex,
            "--no-fail-fast",
        ])
        .status()
        .map_err(|error| format!("failed to run cargo llvm-cov: {error}"))?;
    if !status.success() {
        return Err(format!("cargo llvm-cov failed with status {status}"));
    }
    Ok(())
}
