use std::{collections::BTreeSet, fs, path::Path, process::Command};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Contract {
    schema_version: u16,
    harness: String,
    engine: String,
    toolchain: String,
    smoke_runs: u32,
    max_input_bytes: usize,
    targets: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct HarnessManifest {
    bin: Vec<Bin>,
}

#[derive(Debug, Deserialize)]
struct Bin {
    name: String,
}

pub fn run(root: &Path) -> Result<(), String> {
    let contract = load(root)?;
    validate(root, &contract)?;
    run_cargo_fuzz(root, &["check".to_owned()], &contract)?;
    for target in &contract.targets {
        run_cargo_fuzz(
            root,
            &[
                "run".to_owned(),
                target.clone(),
                "--".to_owned(),
                format!("-runs={}", contract.smoke_runs),
                format!("-max_len={}", contract.max_input_bytes),
            ],
            &contract,
        )?;
    }
    Ok(())
}

fn load(root: &Path) -> Result<Contract, String> {
    let path = root.join("contracts/releases/fuzz_matrix.toml");
    let raw = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    toml::from_str(&raw).map_err(|error| format!("failed to parse {}: {error}", path.display()))
}

fn validate(root: &Path, contract: &Contract) -> Result<(), String> {
    if contract.schema_version != 1
        || contract.engine != "libfuzzer"
        || !contract.toolchain.starts_with("nightly-")
        || contract.smoke_runs < 1_000
        || contract.max_input_bytes < 16_384
    {
        return Err("invalid fuzz qualification contract".to_owned());
    }
    let expected = contract.targets.iter().collect::<BTreeSet<_>>();
    if expected.len() != 7 || expected.len() != contract.targets.len() {
        return Err("fuzz matrix requires exactly seven unique parser targets".to_owned());
    }
    let path = root.join(&contract.harness).join("Cargo.toml");
    let raw = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let manifest = toml::from_str::<HarnessManifest>(&raw)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))?;
    let actual = manifest
        .bin
        .iter()
        .map(|bin| &bin.name)
        .collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(format!(
            "fuzz target inventory drift: expected {expected:?}, found {actual:?}"
        ));
    }
    Ok(())
}

fn run_cargo_fuzz(root: &Path, args: &[String], contract: &Contract) -> Result<(), String> {
    let mut command_args = vec![
        format!("+{}", contract.toolchain),
        "fuzz".to_owned(),
        args[0].clone(),
        "--fuzz-dir".to_owned(),
        contract.harness.clone(),
    ];
    command_args.extend_from_slice(&args[1..]);
    eprintln!("cargo {}", command_args.join(" "));
    let status = Command::new("cargo")
        .args(&command_args)
        .current_dir(root)
        .status()
        .map_err(|error| format!("failed to start cargo-fuzz: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "fuzz qualification failed: cargo {}",
            command_args.join(" ")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{load, validate};

    #[test]
    fn current_contract_matches_all_parser_harnesses() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(std::path::Path::parent)
            .expect("workspace root");
        validate(root, &load(root).expect("contract")).expect("valid fuzz matrix");
    }
}
