use std::{ffi::OsString, fs, path::Path, process::Command};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Catalog {
    package: Vec<Package>,
}

#[derive(Debug, Deserialize)]
struct Package {
    name: String,
    state: String,
    groups: Vec<String>,
}

#[derive(Debug, PartialEq, Eq)]
struct CargoInvocation {
    args: Vec<String>,
}

pub fn run_feature_matrix(workspace_root: &Path) -> Result<(), String> {
    for invocation in feature_matrix(workspace_root)? {
        eprintln!("cargo {}", invocation.args.join(" "));
        let mut command = Command::new("cargo");
        command
            .args(&invocation.args)
            .current_dir(workspace_root)
            .env("RUSTFLAGS", rustflags_with_warnings_denied());
        let status = command
            .status()
            .map_err(|error| format!("failed to start cargo: {error}"))?;
        if !status.success() {
            return Err(format!(
                "public feature qualification failed: cargo {}",
                invocation.args.join(" ")
            ));
        }
    }
    Ok(())
}

fn rustflags_with_warnings_denied() -> OsString {
    let mut rustflags = std::env::var_os("RUSTFLAGS").unwrap_or_default();
    if !rustflags.is_empty() {
        rustflags.push(" ");
    }
    rustflags.push("-Dwarnings");
    rustflags
}

fn feature_matrix(workspace_root: &Path) -> Result<Vec<CargoInvocation>, String> {
    let path = workspace_root.join("contracts/crates/catalog.v2.toml");
    let raw = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let catalog = toml::from_str::<Catalog>(&raw)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))?;
    let mut packages = catalog
        .package
        .into_iter()
        .filter(|package| {
            package.state == "active" && package.groups.iter().any(|group| group == "public_native")
        })
        .collect::<Vec<_>>();
    packages.sort_by(|left, right| left.name.cmp(&right.name));
    if packages.len() != 19 {
        return Err(format!(
            "library feature qualification requires exactly 19 public packages, found {}",
            packages.len()
        ));
    }

    let mut invocations = Vec::new();
    let feature_map = package_feature_map(workspace_root)?;
    for package in packages {
        let features = feature_map
            .get(&package.name)
            .ok_or_else(|| format!("cargo metadata omitted features for {}", package.name))?;
        invocations.push(check_invocation(&package.name, None, true));
        invocations.push(check_invocation(&package.name, None, false));
        for feature in features {
            invocations.push(check_invocation(&package.name, Some(feature), true));
        }
        invocations.push(CargoInvocation {
            args: vec![
                "check".to_owned(),
                "-p".to_owned(),
                package.name,
                "--all-targets".to_owned(),
                "--all-features".to_owned(),
                "--locked".to_owned(),
            ],
        });
    }
    Ok(invocations)
}

fn package_feature_map(
    workspace_root: &Path,
) -> Result<std::collections::BTreeMap<String, Vec<String>>, String> {
    let output = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--locked", "--no-deps"])
        .current_dir(workspace_root)
        .output()
        .map_err(|error| format!("failed to start cargo metadata: {error}"))?;
    if !output.status.success() {
        return Err("locked cargo metadata failed".to_owned());
    }
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("failed to parse cargo metadata: {error}"))?;
    let packages = metadata
        .get("packages")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "cargo metadata omitted packages".to_owned())?;
    packages
        .iter()
        .map(|package| {
            let name = package
                .get("name")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| "cargo metadata package omitted name".to_owned())?;
            let mut features = package
                .get("features")
                .and_then(serde_json::Value::as_object)
                .ok_or_else(|| format!("cargo metadata omitted features for {name}"))?
                .keys()
                .filter(|feature| feature.as_str() != "default")
                .cloned()
                .collect::<Vec<_>>();
            features.sort_unstable();
            Ok((name.to_owned(), features))
        })
        .collect()
}

fn check_invocation(
    package: &str,
    feature: Option<&str>,
    no_default_features: bool,
) -> CargoInvocation {
    let mut args = vec!["check".to_owned(), "-p".to_owned(), package.to_owned()];
    if no_default_features {
        args.push("--lib".to_owned());
        args.push("--no-default-features".to_owned());
    } else {
        args.push("--all-targets".to_owned());
    }
    if let Some(feature) = feature {
        args.push("--features".to_owned());
        args.push(feature.to_owned());
    }
    args.push("--locked".to_owned());
    CargoInvocation { args }
}

#[cfg(test)]
mod tests {
    use super::{check_invocation, feature_matrix};

    #[test]
    fn current_catalog_generates_the_complete_library_feature_matrix() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(std::path::Path::parent)
            .expect("workspace root");
        let matrix = feature_matrix(root).expect("feature matrix");
        assert!(matrix.len() > 68);
        assert!(matrix.iter().any(|entry| {
            entry
                .args
                .windows(2)
                .any(|pair| pair == ["--features", "blossom"])
                && entry
                    .args
                    .windows(2)
                    .any(|pair| pair == ["-p", "radroots_nostr"])
        }));
        assert!(matrix.iter().any(|entry| {
            entry
                .args
                .windows(2)
                .any(|pair| pair == ["-p", "radroots_geonames"])
                && entry.args.iter().any(|arg| arg == "--all-features")
        }));
    }

    #[test]
    fn individual_features_never_enable_defaults_implicitly() {
        let invocation = check_invocation("radroots_event", Some("knowledge"), true);
        assert!(
            invocation
                .args
                .iter()
                .any(|arg| arg == "--no-default-features")
        );
        assert_eq!(
            invocation
                .args
                .windows(2)
                .find(|pair| pair[0] == "--features"),
            Some(&["--features".to_owned(), "knowledge".to_owned()][..])
        );
    }
}
