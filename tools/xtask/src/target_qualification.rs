use std::{fs, path::Path, process::Command};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct TargetMatrix {
    schema_version: u32,
    msrv_toolchain: String,
    current_toolchain: String,
    operating_system: Vec<OperatingSystem>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct OperatingSystem {
    name: String,
    target: String,
    cross_compiler: Option<String>,
    cross_cflags: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Catalog {
    package: Vec<CatalogPackage>,
}

#[derive(Debug, Deserialize)]
struct CatalogPackage {
    name: String,
    state: String,
    groups: Vec<String>,
}

pub fn run(workspace_root: &Path) -> Result<(), String> {
    let (matrix, packages) = load(workspace_root)?;
    for toolchain in [&matrix.msrv_toolchain, &matrix.current_toolchain] {
        run_command(
            workspace_root,
            "rustup",
            &[
                "run",
                toolchain,
                "cargo",
                "check",
                "--workspace",
                "--all-targets",
                "--locked",
            ],
        )?;
    }
    for operating_system in matrix.operating_system {
        for package in &packages {
            run_target_command(
                workspace_root,
                &operating_system,
                &[
                    "check",
                    "-p",
                    package,
                    "--lib",
                    "--target",
                    &operating_system.target,
                    "--locked",
                ],
            )?;
        }
    }
    Ok(())
}

fn load(workspace_root: &Path) -> Result<(TargetMatrix, Vec<String>), String> {
    let matrix_path = workspace_root.join("contracts/releases/target_matrix.toml");
    let matrix = toml::from_str::<TargetMatrix>(&read(&matrix_path)?)
        .map_err(|error| format!("failed to parse {}: {error}", matrix_path.display()))?;
    validate(&matrix)?;

    let catalog_path = workspace_root.join("contracts/crates/catalog.v2.toml");
    let catalog = toml::from_str::<Catalog>(&read(&catalog_path)?)
        .map_err(|error| format!("failed to parse {}: {error}", catalog_path.display()))?;
    let mut packages = catalog
        .package
        .into_iter()
        .filter(|package| {
            package.state == "active" && package.groups.iter().any(|group| group == "public_native")
        })
        .map(|package| package.name)
        .collect::<Vec<_>>();
    packages.sort();
    if packages.len() != 19 {
        return Err(format!(
            "target qualification requires exactly 19 public packages, found {}",
            packages.len()
        ));
    }
    Ok((matrix, packages))
}

fn read(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| format!("failed to read {}: {error}", path.display()))
}

fn validate(matrix: &TargetMatrix) -> Result<(), String> {
    if matrix.schema_version != 1 {
        return Err(format!(
            "target matrix schema_version must be 1, found {}",
            matrix.schema_version
        ));
    }
    if matrix.msrv_toolchain != "1.97.1" || matrix.current_toolchain != "stable" {
        return Err("target matrix must retain MSRV 1.97.1 and current stable".to_owned());
    }
    let expected = [
        (
            "linux",
            "x86_64-unknown-linux-gnu",
            Some("zig cc"),
            Some("-target x86_64-linux-gnu"),
        ),
        ("macos", "aarch64-apple-darwin", None, None),
    ];
    if matrix.operating_system.len() != expected.len()
        || expected.iter().any(|(name, target, compiler, cflags)| {
            !matrix.operating_system.iter().any(|entry| {
                entry.name == *name
                    && entry.target == *target
                    && entry.cross_compiler.as_deref() == *compiler
                    && entry.cross_cflags.as_deref() == *cflags
            })
        })
    {
        return Err(
            "target matrix must contain the exact Linux x86_64 and macOS aarch64 triples"
                .to_owned(),
        );
    }
    Ok(())
}

fn run_target_command(
    workspace_root: &Path,
    operating_system: &OperatingSystem,
    args: &[&str],
) -> Result<(), String> {
    eprintln!("cargo {}", args.join(" "));
    let mut command = Command::new("cargo");
    command.args(args).current_dir(workspace_root);
    let target_key = operating_system.target.replace('-', "_");
    if let Some(compiler) = operating_system.cross_compiler.as_deref() {
        command.env(format!("CC_{target_key}"), compiler);
    }
    if let Some(cflags) = operating_system.cross_cflags.as_deref() {
        command.env(format!("CFLAGS_{target_key}"), cflags);
    }
    let status = command
        .status()
        .map_err(|error| format!("failed to start cargo: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "target qualification failed: cargo {}",
            args.join(" ")
        ))
    }
}

fn run_command(workspace_root: &Path, program: &str, args: &[&str]) -> Result<(), String> {
    eprintln!("{program} {}", args.join(" "));
    let status = Command::new(program)
        .args(args)
        .current_dir(workspace_root)
        .status()
        .map_err(|error| format!("failed to start {program}: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "target qualification failed: {program} {}",
            args.join(" ")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{OperatingSystem, TargetMatrix, load, validate};

    #[test]
    fn current_contract_selects_exact_toolchains_targets_and_packages() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(std::path::Path::parent)
            .expect("workspace root");
        let (matrix, packages) = load(root).expect("target matrix");
        assert_eq!(matrix.msrv_toolchain, "1.97.1");
        assert_eq!(matrix.current_toolchain, "stable");
        assert_eq!(matrix.operating_system.len(), 2);
        assert_eq!(
            matrix
                .operating_system
                .iter()
                .map(|entry| (entry.name.as_str(), entry.target.as_str()))
                .collect::<Vec<_>>(),
            [
                ("linux", "x86_64-unknown-linux-gnu"),
                ("macos", "aarch64-apple-darwin"),
            ]
        );
        assert_eq!(packages.len(), 19);
    }

    #[test]
    fn unsupported_production_targets_are_rejected() {
        let mut matrix = TargetMatrix {
            schema_version: 1,
            msrv_toolchain: "1.97.1".to_owned(),
            current_toolchain: "stable".to_owned(),
            operating_system: vec![
                OperatingSystem {
                    name: "linux".to_owned(),
                    target: "x86_64-unknown-linux-gnu".to_owned(),
                    cross_compiler: Some("zig cc".to_owned()),
                    cross_cflags: Some("-target x86_64-linux-gnu".to_owned()),
                },
                OperatingSystem {
                    name: "macos".to_owned(),
                    target: "aarch64-apple-darwin".to_owned(),
                    cross_compiler: None,
                    cross_cflags: None,
                },
            ],
        };
        for (name, target) in [
            ("windows", "x86_64-pc-windows-gnu"),
            ("macos", "x86_64-apple-darwin"),
            ("linux", "aarch64-unknown-linux-gnu"),
        ] {
            matrix.operating_system.push(OperatingSystem {
                name: name.to_owned(),
                target: target.to_owned(),
                cross_compiler: None,
                cross_cflags: None,
            });
            assert!(validate(&matrix).is_err(), "accepted {target}");
            matrix.operating_system.pop();
        }
    }
}
