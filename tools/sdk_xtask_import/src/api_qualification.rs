use std::{collections::BTreeSet, fs, path::Path, process::Command};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Contract {
    schema_version: u16,
    baseline_revision: String,
    package_version: String,
    tool: String,
    minimum_tool_version: String,
    policy: String,
    feature_policy: String,
    packages: Vec<String>,
}

pub fn run(root: &Path) -> Result<(), String> {
    let contract = load(root)?;
    validate(&contract)?;
    verify_tool(&contract)?;
    verify_revision(root, &contract.baseline_revision)?;
    for package in &contract.packages {
        let args = invocation(package);
        eprintln!("cargo {}", args.join(" "));
        let output = Command::new("cargo")
            .args(&args)
            .current_dir(root)
            .output()
            .map_err(|error| format!("failed to start cargo-public-api: {error}"))?;
        if !output.status.success() {
            return Err(format!(
                "public API extraction failed for {package}: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        let actual = String::from_utf8(output.stdout)
            .map_err(|error| format!("public API output for {package} was not UTF-8: {error}"))?;
        let snapshot_path = root.join(format!(
            "docs/api/{package}-{}.txt",
            contract.package_version
        ));
        let expected = fs::read_to_string(&snapshot_path)
            .map_err(|error| format!("read {}: {error}", snapshot_path.display()))?;
        if actual != expected {
            return Err(format!(
                "reviewed public API snapshot drifted for {package}: {}",
                snapshot_path.display()
            ));
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

fn validate(contract: &Contract) -> Result<(), String> {
    let unique = contract.packages.iter().collect::<BTreeSet<_>>();
    if contract.schema_version != 2
        || contract.package_version != "0.1.0-alpha"
        || contract.tool != "cargo-public-api"
        || contract.policy != "reviewed-breaking-snapshot"
        || contract.feature_policy != "all"
        || contract.baseline_revision.len() < 7
        || unique.len() != 2
        || unique.len() != contract.packages.len()
    {
        return Err("invalid SDK public API qualification contract".to_owned());
    }
    Ok(())
}

fn verify_tool(contract: &Contract) -> Result<(), String> {
    let output = Command::new("cargo")
        .args(["public-api", "--version"])
        .output()
        .map_err(|error| format!("failed to start cargo-public-api: {error}"))?;
    if !output.status.success() {
        return Err("cargo-public-api is required".to_owned());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let installed = stdout
        .split_whitespace()
        .find_map(|value| semver::Version::parse(value).ok())
        .ok_or_else(|| format!("could not parse cargo-public-api version: {stdout}"))?;
    let minimum = semver::Version::parse(&contract.minimum_tool_version)
        .map_err(|error| format!("invalid minimum tool version: {error}"))?;
    if installed < minimum {
        return Err(format!(
            "cargo-public-api {} or newer is required, found {installed}",
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

fn invocation(package: &str) -> Vec<String> {
    vec![
        "public-api".to_owned(),
        "-p".to_owned(),
        package.to_owned(),
        "--all-features".to_owned(),
        "-sss".to_owned(),
        "--color".to_owned(),
        "never".to_owned(),
    ]
}

#[cfg(test)]
mod tests {
    use super::{invocation, load, validate};

    #[test]
    fn current_contract_covers_both_front_doors() {
        let root = crate::fs::workspace_root().expect("workspace root");
        let contract = load(&root).expect("contract");
        validate(&contract).expect("valid contract");
        let invocation = invocation("radroots");
        assert!(invocation.contains(&"--all-features".to_owned()));
        assert!(invocation.contains(&"-sss".to_owned()));
    }
}
