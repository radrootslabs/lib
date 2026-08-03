use std::{fs, path::Path, process::Command};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Architecture {
    repositories: Repositories,
    package: Vec<Package>,
}

#[derive(Debug, Deserialize)]
struct Repositories {
    lib: Repository,
}

#[derive(Debug, Deserialize)]
struct Repository {
    packages: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Package {
    name: String,
    features: Vec<String>,
}

#[derive(Debug, PartialEq, Eq)]
struct CargoInvocation {
    args: Vec<String>,
}

pub fn run_feature_matrix(workspace_root: &Path) -> Result<(), String> {
    for invocation in feature_matrix(workspace_root)? {
        eprintln!("cargo {}", invocation.args.join(" "));
        let status = Command::new("cargo")
            .args(&invocation.args)
            .current_dir(workspace_root)
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

fn feature_matrix(workspace_root: &Path) -> Result<Vec<CargoInvocation>, String> {
    let path = workspace_root.join("docs/specs/radroots_crates_release_v1.toml");
    let raw = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let architecture = toml::from_str::<Architecture>(&raw)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))?;
    let mut packages = architecture
        .package
        .into_iter()
        .filter(|package| {
            architecture
                .repositories
                .lib
                .packages
                .contains(&package.name)
        })
        .collect::<Vec<_>>();
    packages.sort_by(|left, right| left.name.cmp(&right.name));
    if packages.len() != 17 {
        return Err(format!(
            "library feature qualification requires exactly 17 public packages, found {}",
            packages.len()
        ));
    }

    let mut invocations = Vec::new();
    for package in packages {
        invocations.push(check_invocation(&package.name, None, true));
        invocations.push(check_invocation(&package.name, None, false));
        for feature in package.features {
            invocations.push(check_invocation(&package.name, Some(&feature), true));
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
