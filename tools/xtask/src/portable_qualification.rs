use std::{fs, path::Path, process::Command};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct PortableMatrix {
    schema_version: u32,
    no_std_target: String,
    no_std_cross_compiler: String,
    no_std_cross_cflags: String,
    wasm_target: String,
    package: Vec<PortablePackage>,
}

#[derive(Debug, Deserialize)]
struct PortablePackage {
    name: String,
    wasm: bool,
}

pub fn run(workspace_root: &Path) -> Result<(), String> {
    let matrix = load(workspace_root)?;
    for package in matrix.package {
        check(
            workspace_root,
            &package.name,
            &matrix.no_std_target,
            Some(&matrix.no_std_cross_compiler),
            Some(&matrix.no_std_cross_cflags),
        )?;
        if package.wasm {
            check(
                workspace_root,
                &package.name,
                &matrix.wasm_target,
                None,
                None,
            )?;
        }
    }
    Ok(())
}

fn load(workspace_root: &Path) -> Result<PortableMatrix, String> {
    let path = workspace_root.join("contracts/releases/portable_matrix.toml");
    let raw = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let mut matrix = toml::from_str::<PortableMatrix>(&raw)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))?;
    if matrix.schema_version != 1
        || matrix.no_std_target != "thumbv7em-none-eabihf"
        || matrix.no_std_cross_compiler != "zig cc"
        || matrix.no_std_cross_cflags != "-target thumb-freestanding-eabihf -mcpu=cortex_m4"
        || matrix.wasm_target != "wasm32-unknown-unknown"
    {
        return Err(
            "portable matrix must retain schema 1 and the governed no_std/WASM targets".to_owned(),
        );
    }
    matrix
        .package
        .sort_by(|left, right| left.name.cmp(&right.name));
    let names = matrix
        .package
        .iter()
        .map(|package| package.name.as_str())
        .collect::<Vec<_>>();
    let expected = [
        "radroots_blossom",
        "radroots_core",
        "radroots_event",
        "radroots_event_codec",
        "radroots_identity",
        "radroots_nostr",
        "radroots_protocol",
        "radroots_secrets",
        "radroots_signing",
        "radroots_trade",
        "radroots_transport",
    ];
    if names != expected || matrix.package.iter().any(|package| !package.wasm) {
        return Err(
            "portable matrix must contain exactly the 11 declared no_std and WASM packages"
                .to_owned(),
        );
    }
    Ok(matrix)
}

fn check(
    workspace_root: &Path,
    package: &str,
    target: &str,
    cross_compiler: Option<&str>,
    cross_cflags: Option<&str>,
) -> Result<(), String> {
    let args = [
        "check",
        "-p",
        package,
        "--lib",
        "--no-default-features",
        "--target",
        target,
        "--locked",
    ];
    eprintln!("cargo {}", args.join(" "));
    let mut command = Command::new("cargo");
    command.args(args).current_dir(workspace_root);
    if let Some(cross_compiler) = cross_compiler {
        command.env(format!("CC_{}", target.replace('-', "_")), cross_compiler);
    }
    if let Some(cross_cflags) = cross_cflags {
        command.env(format!("CFLAGS_{}", target.replace('-', "_")), cross_cflags);
    }
    let status = command
        .status()
        .map_err(|error| format!("failed to start cargo: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "portable qualification failed for {package} on {target}"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::load;

    #[test]
    fn current_contract_selects_exact_portable_public_packages() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(std::path::Path::parent)
            .expect("workspace root");
        let matrix = load(root).expect("portable matrix");
        assert_eq!(matrix.package.len(), 11);
        assert!(matrix.package.iter().all(|package| package.wasm));
    }
}
