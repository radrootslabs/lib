//! Current SDK compiler requirements; validation does not qualify execution.

use std::{fmt, fs, io::Read as _, path::Path};

use serde::Deserialize;
use sha2::{Digest as _, Sha256};

use crate::service_source_lock_v4::{RUST_VERSION, validate_lib_compiler_inputs};

const CONTRACT: &str = "contracts/architecture/decisions/shared_sdk_compiler_preparation.v1.json";
const MAX_CONTRACT_BYTES: usize = 65_536;
const BOUND_INPUTS: [(&str, &str); 3] = [
    (
        "contracts/architecture/decisions/shared_sdk_refactor.v1.json",
        "7937dc9ee4aca42f015d3bba5b23e2b5e7cffe461589518bd6eef6305892931e",
    ),
    (
        "contracts/architecture/decisions/services_hardening_source_lock.v4.json",
        "133057de55c3691badcc3a29a9cc7009e1adb62564e4608b7441f6a2d42373bd",
    ),
    (
        "contracts/architecture/decisions/services_hardening_release_artifacts.v5.json",
        "b535c9c891889681c07031a3a6e3d3f7372501f2a233e20eb2cb2d79c9101fee",
    ),
];
const COMPONENTS: [&str; 4] = ["clippy", "rust-analyzer", "rust-src", "rustfmt"];
const DECLARED_TARGETS: [&str; 1] = ["wasm32-unknown-unknown"];
const NATIVE_TARGETS: [&str; 2] = ["aarch64-apple-darwin", "x86_64-unknown-linux-gnu"];
const APPLE_TARGETS: [&str; 2] = ["aarch64-apple-ios", "aarch64-apple-ios-sim"];
const NIX_SYSTEMS: [&str; 2] = ["aarch64-darwin", "x86_64-linux"];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PreparationError {
    InvalidContract,
    InvalidInput,
}

impl fmt::Display for PreparationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidContract => "current compiler preparation contract is invalid",
            Self::InvalidInput => "current compiler preparation input is invalid",
        })
    }
}

impl std::error::Error for PreparationError {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Decision {
    schema: String,
    contract_version: u32,
    state: String,
    stable: String,
    msrv: String,
    edition: String,
    source_inputs: Vec<BoundInput>,
    required_declared_components: Vec<String>,
    required_declared_targets: Vec<String>,
    native_targets: Vec<String>,
    apple_artifact_targets: Vec<String>,
    nix_systems: Vec<String>,
    dedicated_toolchains_separate: bool,
    historical_receipts_immutable: bool,
    declaration_validation_proves_execution: bool,
    execution_evidence: String,
    publication_authorized: bool,
    candidate_activation_authorized: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BoundInput {
    path: String,
    sha256: String,
}

pub(crate) fn validate_contract(root: &Path) -> Result<(), String> {
    validate(root).map_err(|error| error.to_string())
}

fn validate(root: &Path) -> Result<(), PreparationError> {
    let bytes = read_bounded(
        root,
        CONTRACT,
        MAX_CONTRACT_BYTES,
        PreparationError::InvalidContract,
    )?;
    let decision: Decision =
        serde_json::from_slice(&bytes).map_err(|_| PreparationError::InvalidContract)?;
    validate_decision(&decision)?;
    validate_bound_inputs(root)?;
    validate_current_inputs(root)
}

fn validate_decision(value: &Decision) -> Result<(), PreparationError> {
    let exact = value.schema == "radroots.shared_sdk.compiler_preparation.v1"
        && value.contract_version == 1
        && value.state == "active_requirements_execution_unclaimed"
        && value.stable == RUST_VERSION
        && value.msrv == RUST_VERSION
        && value.edition == "2024"
        && value.source_inputs.len() == BOUND_INPUTS.len()
        && value
            .source_inputs
            .iter()
            .zip(BOUND_INPUTS)
            .all(|(input, (path, digest))| input.path == path && input.sha256 == digest)
        && value.required_declared_components == COMPONENTS
        && value.required_declared_targets == DECLARED_TARGETS
        && value.native_targets == NATIVE_TARGETS
        && value.apple_artifact_targets == APPLE_TARGETS
        && value.nix_systems == NIX_SYSTEMS
        && value.dedicated_toolchains_separate
        && value.historical_receipts_immutable
        && !value.declaration_validation_proves_execution
        && value.execution_evidence == "separate_source_scoped_receipts"
        && !value.publication_authorized
        && !value.candidate_activation_authorized;
    if exact {
        Ok(())
    } else {
        Err(PreparationError::InvalidContract)
    }
}

fn validate_bound_inputs(root: &Path) -> Result<(), PreparationError> {
    for (path, expected) in BOUND_INPUTS {
        let bytes = read_bounded(
            root,
            path,
            MAX_CONTRACT_BYTES,
            PreparationError::InvalidContract,
        )?;
        if hex::encode(Sha256::digest(&bytes)) != expected {
            return Err(PreparationError::InvalidContract);
        }
    }
    Ok(())
}

fn validate_current_inputs(root: &Path) -> Result<(), PreparationError> {
    let read = |path, limit| read_bounded(root, path, limit, PreparationError::InvalidInput);
    let manifest = read("Cargo.toml", 1_048_576)?;
    let toolchain = read("rust-toolchain.toml", 65_536)?;
    let catalog = read("contracts/crates/catalog.v2.toml", 4_194_304)?;
    validate_lib_compiler_inputs(&manifest, &toolchain, &catalog)
        .map_err(|_| PreparationError::InvalidInput)?;
    let toolchain = std::str::from_utf8(&toolchain).map_err(|_| PreparationError::InvalidInput)?;
    let toolchain: toml::Value =
        toml::from_str(toolchain).map_err(|_| PreparationError::InvalidInput)?;
    for (field, required) in [
        ("components", COMPONENTS.as_slice()),
        ("targets", DECLARED_TARGETS.as_slice()),
    ] {
        let declared = toolchain
            .get("toolchain")
            .and_then(|value| value.get(field))
            .and_then(toml::Value::as_array)
            .ok_or(PreparationError::InvalidInput)?;
        let declared = declared
            .iter()
            .map(toml::Value::as_str)
            .collect::<Option<Vec<_>>>()
            .ok_or(PreparationError::InvalidInput)?;
        if !required.iter().all(|value| declared.contains(value)) {
            return Err(PreparationError::InvalidInput);
        }
    }
    Ok(())
}

fn read_bounded(
    root: &Path,
    relative: &str,
    maximum: usize,
    error: PreparationError,
) -> Result<Vec<u8>, PreparationError> {
    let path = root.join(relative);
    let metadata = fs::symlink_metadata(&path).map_err(|_| error)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > maximum as u64 {
        return Err(error);
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|_| error)?
        .take(maximum as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| error)?;
    if bytes.len() > maximum {
        Err(error)
    } else {
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    fn root() -> &'static Path {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("workspace root")
    }

    fn copy_inputs(paths: &[&str]) -> TempDir {
        let destination = TempDir::new().expect("input fixture");
        for path in paths {
            let target = destination.path().join(path);
            fs::create_dir_all(target.parent().expect("parent")).expect("parent");
            fs::copy(root().join(path), target).expect("copy input");
        }
        destination
    }

    #[test]
    fn current_requirements_and_bound_source_inputs_validate() {
        validate(root()).expect("current compiler requirements");
    }

    #[test]
    fn independent_contract_drift_cannot_change_compiler_matrix_or_claims() {
        let bytes = fs::read(root().join(CONTRACT)).expect("contract");
        let canonical: serde_json::Value = serde_json::from_slice(&bytes).expect("JSON");
        for (pointer, replacement) in [
            ("/schema", json!("other")),
            ("/contract_version", json!(2)),
            ("/state", json!("qualified")),
            ("/stable", json!("1.97.1")),
            ("/msrv", json!("1.98.0")),
            ("/edition", json!("2021")),
            ("/source_inputs/0/path", json!("../other")),
            ("/source_inputs/1/sha256", json!("0".repeat(64))),
            ("/required_declared_components", json!([])),
            ("/required_declared_targets", json!([])),
            ("/native_targets", json!([])),
            ("/apple_artifact_targets", json!([])),
            ("/nix_systems", json!([])),
            ("/dedicated_toolchains_separate", json!(false)),
            ("/historical_receipts_immutable", json!(false)),
            ("/declaration_validation_proves_execution", json!(true)),
            ("/execution_evidence", json!("embedded_in_contract")),
            ("/publication_authorized", json!(true)),
            ("/candidate_activation_authorized", json!(true)),
        ] {
            let mut changed = canonical.clone();
            *changed.pointer_mut(pointer).expect("governed field") = replacement;
            let decision = serde_json::from_value(changed).expect("structurally valid drift");
            assert_eq!(
                validate_decision(&decision),
                Err(PreparationError::InvalidContract),
                "{pointer}"
            );
        }
        let mut extra = canonical;
        extra["unrecognized_claim"] = json!(true);
        assert!(serde_json::from_value::<Decision>(extra).is_err());
    }

    #[test]
    fn exact_bound_contract_bytes_cannot_be_substituted() {
        let paths: Vec<_> = BOUND_INPUTS.iter().map(|(path, _)| *path).collect();
        for path in paths.iter() {
            let fixture = copy_inputs(&paths);
            validate_bound_inputs(fixture.path()).expect("original inputs");
            fs::write(fixture.path().join(path), b"{}\n").expect("substitute input");
            assert_eq!(
                validate_bound_inputs(fixture.path()),
                Err(PreparationError::InvalidContract)
            );
        }
    }

    #[test]
    fn current_inputs_require_the_compiler_components_and_wasm_declaration() {
        for from in [
            "1.99.0",
            "clippy",
            "rust-analyzer",
            "rust-src",
            "rustfmt",
            "wasm32-unknown-unknown",
        ] {
            let fixture = copy_inputs(&[
                "Cargo.toml",
                "rust-toolchain.toml",
                "contracts/crates/catalog.v2.toml",
            ]);
            validate_current_inputs(fixture.path()).expect("current input fixture");
            let path = fixture.path().join("rust-toolchain.toml");
            let original = fs::read_to_string(&path).expect("selector");
            assert!(original.contains(from));
            fs::write(path, original.replace(from, "other")).expect("selector drift");
            assert_eq!(
                validate_current_inputs(fixture.path()),
                Err(PreparationError::InvalidInput),
                "{from}"
            );
        }
    }
}
