use std::{fs, path::Path};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct PortableMatrix {
    schema_version: u32,
    no_std_target: String,
    no_std_cross_compiler: String,
    no_std_cross_cflags: String,
    wasm_target: String,
    portable_public_packages: Vec<String>,
    std_only_public_packages: Vec<String>,
}

pub fn run(workspace_root: &Path) -> Result<(), String> {
    load(workspace_root).map(|_| ())
}

fn load(workspace_root: &Path) -> Result<PortableMatrix, String> {
    let path = workspace_root.join("contracts/releases/portable_matrix.toml");
    let raw = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let matrix = toml::from_str::<PortableMatrix>(&raw)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))?;
    if matrix.schema_version != 1
        || matrix.no_std_target != "thumbv7em-none-eabihf"
        || matrix.no_std_cross_compiler != "zig cc"
        || matrix.no_std_cross_cflags != "-target thumb-freestanding-eabihf -mcpu=cortex_m4"
        || matrix.wasm_target != "wasm32-unknown-unknown"
        || !matrix.portable_public_packages.is_empty()
        || matrix.std_only_public_packages != ["radroots", "radroots_sdk"]
    {
        return Err(
            "SDK portability contract must classify both public front doors as std-only".to_owned(),
        );
    }
    Ok(matrix)
}

#[cfg(test)]
mod tests {
    use super::load;

    #[test]
    fn current_contract_rejects_nominal_front_door_portability() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(std::path::Path::parent)
            .expect("workspace root");
        let matrix = load(root).expect("portable matrix");
        assert!(matrix.portable_public_packages.is_empty());
        assert_eq!(
            matrix.std_only_public_packages,
            ["radroots", "radroots_sdk"]
        );
    }
}
