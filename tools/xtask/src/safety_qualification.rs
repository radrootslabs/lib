use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::Command;

use serde::Deserialize;

const CONTRACT_PATH: &str = "contracts/releases/safety_matrix.toml";

#[derive(Debug, Deserialize)]
struct Contract {
    schema_version: u16,
    toolchain: String,
    miri_flags: Vec<String>,
    miri: Vec<MiriLane>,
    sanitizer: Vec<SanitizerLane>,
    exception: Vec<Exception>,
}

#[derive(Debug, Deserialize)]
struct MiriLane {
    package: String,
    filter: String,
    authority: String,
}

#[derive(Debug, Deserialize)]
struct SanitizerLane {
    kind: String,
    targets: Vec<String>,
    packages: Vec<String>,
    authority: String,
}

#[derive(Debug, Deserialize)]
struct Exception {
    lane: String,
    targets: Vec<String>,
    owner: String,
    expires: String,
    reason: String,
}

#[derive(Debug, Deserialize)]
struct Metadata {
    packages: Vec<MetadataPackage>,
}

#[derive(Debug, Deserialize)]
struct MetadataPackage {
    name: String,
}

pub fn run(root: &Path) -> Result<(), String> {
    let contract = load(root)?;
    let packages = workspace_packages(root)?;
    validate(&contract, &packages)?;
    qualify_miri(root, &contract)?;
    qualify_sanitizers(root, &contract)?;
    Ok(())
}

fn load(root: &Path) -> Result<Contract, String> {
    let path = root.join(CONTRACT_PATH);
    let raw = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    toml::from_str(&raw).map_err(|error| format!("failed to parse {}: {error}", path.display()))
}

fn workspace_packages(root: &Path) -> Result<BTreeSet<String>, String> {
    let output = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--no-deps", "--locked"])
        .current_dir(root)
        .output()
        .map_err(|error| format!("failed to start cargo metadata: {error}"))?;
    if !output.status.success() {
        return Err("cargo metadata failed while validating the safety matrix".to_owned());
    }
    let metadata: Metadata = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("failed to decode cargo metadata: {error}"))?;
    Ok(metadata
        .packages
        .into_iter()
        .map(|package| package.name)
        .collect())
}

fn validate(contract: &Contract, packages: &BTreeSet<String>) -> Result<(), String> {
    if contract.schema_version != 1
        || !contract.toolchain.starts_with("nightly-")
        || contract.miri_flags != ["-Zmiri-strict-provenance", "-Zmiri-disable-isolation"]
        || contract.miri.len() != 8
        || contract.sanitizer.len() != 1
    {
        return Err("invalid safety qualification contract".to_owned());
    }

    let miri = contract
        .miri
        .iter()
        .map(|lane| (&lane.package, &lane.filter))
        .collect::<BTreeSet<_>>();
    if miri.len() != contract.miri.len() {
        return Err("Miri package/filter pairs must be unique".to_owned());
    }
    for lane in &contract.miri {
        validate_identifier("Miri package", &lane.package)?;
        validate_test_filter(&lane.filter)?;
        validate_authority(&lane.authority)?;
        if !packages.contains(&lane.package) {
            return Err(format!(
                "Miri package {} is not in the workspace",
                lane.package
            ));
        }
    }

    for lane in &contract.sanitizer {
        if lane.kind != "address" || lane.targets.len() != 4 || lane.packages.len() != 3 {
            return Err("native sanitizer authority must cover address checks on four hosts and three boundaries".to_owned());
        }
        validate_authority(&lane.authority)?;
        let targets = lane.targets.iter().collect::<BTreeSet<_>>();
        let lane_packages = lane.packages.iter().collect::<BTreeSet<_>>();
        if targets.len() != lane.targets.len() || lane_packages.len() != lane.packages.len() {
            return Err("sanitizer targets and packages must be unique".to_owned());
        }
        for package in &lane.packages {
            validate_identifier("sanitizer package", package)?;
            if !packages.contains(package) {
                return Err(format!(
                    "sanitizer package {package} is not in the workspace"
                ));
            }
        }
    }

    if contract.exception.len() != 1 {
        return Err(
            "the unsupported sanitizer target authority must contain one bounded exception"
                .to_owned(),
        );
    }
    let exception = &contract.exception[0];
    if exception.lane != "sanitizer"
        || exception.owner != "radroots-security"
        || exception.expires != "2026-10-01"
        || exception.targets.len() != 3
        || exception.reason.trim().is_empty()
    {
        return Err("invalid sanitizer target exception".to_owned());
    }
    Ok(())
}

fn validate_identifier(label: &str, value: &str) -> Result<(), String> {
    if value.is_empty()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err(format!("{label} must be a lowercase snake_case identifier"));
    }
    Ok(())
}

fn validate_test_filter(value: &str) -> Result<(), String> {
    let segments = value.split("::").collect::<Vec<_>>();
    if segments.len() < 3 {
        return Err("Miri filter must be a fully qualified test path".to_owned());
    }
    for segment in segments {
        validate_identifier("Miri test path segment", segment)?;
    }
    Ok(())
}

fn validate_authority(value: &str) -> Result<(), String> {
    if value.is_empty()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err("safety authority must be a lowercase kebab-case identifier".to_owned());
    }
    Ok(())
}

fn qualify_miri(root: &Path, contract: &Contract) -> Result<(), String> {
    let flags = contract.miri_flags.join(" ");
    for lane in &contract.miri {
        verify_test_exists(root, lane)?;
        let args = [
            format!("+{}", contract.toolchain),
            "miri".to_owned(),
            "test".to_owned(),
            "--locked".to_owned(),
            "-p".to_owned(),
            lane.package.clone(),
            "--lib".to_owned(),
            lane.filter.clone(),
            "--".to_owned(),
            "--exact".to_owned(),
        ];
        run_cargo(root, &args, &[("MIRIFLAGS", flags.as_str())], "Miri")?;
    }
    Ok(())
}

fn verify_test_exists(root: &Path, lane: &MiriLane) -> Result<(), String> {
    let args = [
        "test",
        "--locked",
        "-p",
        lane.package.as_str(),
        "--lib",
        lane.filter.as_str(),
        "--",
        "--exact",
        "--list",
    ];
    let output = Command::new("cargo")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|error| format!("failed to enumerate Miri test {}: {error}", lane.filter))?;
    if !output.status.success() {
        return Err(format!(
            "failed to enumerate Miri test {} in {}",
            lane.filter, lane.package
        ));
    }
    let stdout = String::from_utf8(output.stdout)
        .map_err(|error| format!("test enumeration emitted non-UTF-8 output: {error}"))?;
    let expected = format!("{}: test", lane.filter);
    if stdout.lines().any(|line| line == expected) {
        Ok(())
    } else {
        Err(format!(
            "Miri authority {} does not resolve to exactly one library test in {}",
            lane.filter, lane.package
        ))
    }
}

fn qualify_sanitizers(root: &Path, contract: &Contract) -> Result<(), String> {
    let host = rustc_host(root, &contract.toolchain)?;
    let mut matched = false;
    for lane in &contract.sanitizer {
        if !lane.targets.iter().any(|target| target == &host) {
            continue;
        }
        matched = true;
        let mut args = vec![
            format!("+{}", contract.toolchain),
            "test".to_owned(),
            "--locked".to_owned(),
            "--target".to_owned(),
            host.clone(),
        ];
        for package in &lane.packages {
            args.push("-p".to_owned());
            args.push(package.clone());
        }
        args.push("--lib".to_owned());
        let rustflags = format!("-Zsanitizer={}", lane.kind);
        run_cargo(
            root,
            &args,
            &[
                ("RUSTFLAGS", rustflags.as_str()),
                ("RUSTDOCFLAGS", rustflags.as_str()),
                ("ASAN_OPTIONS", "detect_leaks=1:halt_on_error=1"),
            ],
            "sanitizer",
        )?;
    }
    if matched {
        return Ok(());
    }
    if contract
        .exception
        .iter()
        .any(|exception| exception.targets.iter().any(|target| target == &host))
    {
        eprintln!("sanitizer qualification excluded for governed target {host}");
        return Ok(());
    }
    Err(format!(
        "host {host} has neither a sanitizer lane nor a governed exception"
    ))
}

fn rustc_host(root: &Path, toolchain: &str) -> Result<String, String> {
    let output = Command::new("rustc")
        .args([format!("+{toolchain}"), "-vV".to_owned()])
        .current_dir(root)
        .output()
        .map_err(|error| format!("failed to start rustc: {error}"))?;
    if !output.status.success() {
        return Err(format!("rustc +{toolchain} -vV failed"));
    }
    let stdout = String::from_utf8(output.stdout)
        .map_err(|error| format!("rustc -vV emitted non-UTF-8 output: {error}"))?;
    stdout
        .lines()
        .find_map(|line| line.strip_prefix("host: ").map(str::to_owned))
        .ok_or_else(|| "rustc -vV did not report a host target".to_owned())
}

fn run_cargo(
    root: &Path,
    args: &[String],
    environment: &[(&str, &str)],
    label: &str,
) -> Result<(), String> {
    eprintln!("cargo {}", args.join(" "));
    let status = Command::new("cargo")
        .args(args)
        .envs(environment.iter().copied())
        .current_dir(root)
        .status()
        .map_err(|error| format!("failed to start {label} qualification: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "{label} qualification failed: cargo {}",
            args.join(" ")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{load, validate, workspace_packages};

    #[test]
    fn current_contract_covers_governed_safety_boundaries() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(std::path::Path::parent)
            .expect("workspace root");
        let packages = workspace_packages(root).expect("workspace packages");
        validate(&load(root).expect("contract"), &packages).expect("valid safety matrix");
    }
}
