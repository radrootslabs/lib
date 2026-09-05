use std::{collections::BTreeSet, fs, path::Path, process::Command};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Contract {
    schema_version: u16,
    baseline_revision: String,
    tool: String,
    minimum_tool_version: String,
    release_type: String,
    feature_policy: String,
    packages: Vec<String>,
    baseline_override: Vec<BaselineOverride>,
}

#[derive(Debug, Deserialize)]
struct BaselineOverride {
    package: String,
    revision: String,
}

pub fn run(root: &Path) -> Result<(), String> {
    let contract = load(root)?;
    validate(&contract, 19)?;
    verify_tool(&contract)?;
    verify_revision(root, &contract.baseline_revision)?;
    for baseline in &contract.baseline_override {
        verify_revision(root, &baseline.revision)?;
    }
    for package in &contract.packages {
        let args = invocation(&contract, package);
        eprintln!("cargo {}", args.join(" "));
        let status = Command::new("cargo")
            .args(&args)
            .env("CARGO_PROFILE_DEV_DEBUG", "none")
            .current_dir(root)
            .status()
            .map_err(|error| format!("failed to start cargo-semver-checks: {error}"))?;
        if !status.success() {
            return Err(format!("public API qualification failed for {package}"));
        }
    }
    Ok(())
}

fn load(root: &Path) -> Result<Contract, String> {
    let path = root.join("contracts/releases/api_semver.toml");
    let raw = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    toml::from_str(&raw).map_err(|error| format!("failed to parse {}: {error}", path.display()))
}

fn validate(contract: &Contract, expected_packages: usize) -> Result<(), String> {
    if contract.schema_version != 1
        || contract.tool != "cargo-semver-checks"
        || contract.release_type != "major"
        || contract.feature_policy != "all"
        || contract.baseline_revision.len() < 7
    {
        return Err("invalid public API qualification contract".to_owned());
    }
    let unique = contract.packages.iter().collect::<BTreeSet<_>>();
    if unique.len() != expected_packages || unique.len() != contract.packages.len() {
        return Err(format!(
            "public API contract requires exactly {expected_packages} unique packages"
        ));
    }
    let overrides = contract
        .baseline_override
        .iter()
        .map(|baseline| baseline.package.as_str())
        .collect::<BTreeSet<_>>();
    if overrides != BTreeSet::from(["radroots", "radroots_sdk"])
        || contract
            .baseline_override
            .iter()
            .any(|baseline| baseline.revision.len() != 40)
    {
        return Err(
            "public API baseline overrides must identify the two imported front doors".to_owned(),
        );
    }
    Ok(())
}

fn verify_tool(contract: &Contract) -> Result<(), String> {
    let output = Command::new("cargo")
        .args(["semver-checks", "--version"])
        .output()
        .map_err(|error| format!("failed to start cargo-semver-checks: {error}"))?;
    if !output.status.success() {
        return Err("cargo-semver-checks is required".to_owned());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let installed = stdout
        .split_whitespace()
        .find_map(|value| semver::Version::parse(value).ok())
        .ok_or_else(|| format!("could not parse cargo-semver-checks version: {stdout}"))?;
    let minimum = semver::Version::parse(&contract.minimum_tool_version)
        .map_err(|error| format!("invalid minimum tool version: {error}"))?;
    if installed < minimum {
        return Err(format!(
            "cargo-semver-checks {} or newer is required, found {installed}",
            contract.minimum_tool_version
        ));
    }
    Ok(())
}

fn verify_revision(root: &Path, revision: &str) -> Result<(), String> {
    let status = Command::new("git")
        .args(["cat-file", "-e", &format!("{revision}^{{commit}}")])
        .current_dir(root)
        .status()
        .map_err(|error| format!("failed to inspect API baseline revision: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("API baseline revision {revision} is unavailable"))
    }
}

fn invocation(contract: &Contract, package: &str) -> Vec<String> {
    let baseline = contract
        .baseline_override
        .iter()
        .find(|baseline| baseline.package == package)
        .map_or(contract.baseline_revision.as_str(), |baseline| {
            baseline.revision.as_str()
        });
    vec![
        "semver-checks".to_owned(),
        "check-release".to_owned(),
        "--package".to_owned(),
        package.to_owned(),
        "--baseline-rev".to_owned(),
        baseline.to_owned(),
        "--all-features".to_owned(),
        "--release-type".to_owned(),
        contract.release_type.clone(),
    ]
}

#[cfg(test)]
mod tests {
    use super::{invocation, load, validate};

    #[test]
    fn current_contract_covers_all_library_packages() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(std::path::Path::parent)
            .expect("workspace root");
        let contract = load(root).expect("contract");
        validate(&contract, 19).expect("valid contract");
        let invocation = invocation(&contract, "radroots_core");
        assert!(invocation.contains(&"--all-features".to_owned()));
        assert!(invocation.ends_with(&["--release-type".to_owned(), "major".to_owned()]));
    }
}
