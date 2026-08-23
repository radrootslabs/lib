use std::{fmt, fs, io::Read as _, path::Path};

use serde::Deserialize;
use sha2::{Digest as _, Sha256};

use crate::service_source_lock::{
    LOCK_FILENAME, NixMaterialState, PREDECESSOR_LOCK_FILENAME, ServiceSourceLockV2,
};

const CONTRACT_RELATIVE: &str =
    "contracts/architecture/decisions/services_hardening_build_qualification.v2.json";
const FIXTURE_RELATIVE: &str = "tools/xtask/fixtures/service-build-qualification";
const MAX_CONTRACT_BYTES: usize = 32_768;
const MAX_FIXTURE_FILE_BYTES: usize = 1_048_576;

const SUPPORTED_RUST_TARGETS: [&str; 4] = [
    "aarch64-apple-darwin",
    "aarch64-unknown-linux-gnu",
    "x86_64-apple-darwin",
    "x86_64-unknown-linux-gnu",
];
const REQUIRED_XTASK_COMMANDS: [&str; 5] = [
    "cargo test --locked -p xtask service_source_lock::tests",
    "cargo test --locked -p xtask service_release_artifacts::tests",
    "cargo test --locked -p xtask service_build_qualification::tests",
    "cargo run --locked -q -p xtask -- contract validate",
    "cargo run --locked -q -p xtask -- release preflight",
];
const REQUIRED_NATIVE_COMMANDS: [&str; 6] = [
    "cargo build --locked --release",
    "cargo fmt --all --check",
    "cargo check --workspace --all-targets --locked",
    "cargo test --workspace --all-targets --locked",
    "cargo clippy --workspace --all-targets --locked -- -D warnings",
    "RUSTDOCFLAGS=-D warnings cargo doc --workspace --no-deps --locked",
];
const REQUIRED_EVIDENCE: [&str; 11] = [
    "cargo_lock",
    "source_lock",
    "package_metadata",
    "release_metadata",
    "binary_archive",
    "oci_source_artifact",
    "cyclonedx_sbom",
    "notices",
    "artifact_manifest",
    "unsigned_provenance_input",
    "checksums",
];
const DEFERRED_OUTPUTS: [&str; 6] = [
    "nix_packages",
    "nix_apps",
    "nix_checks",
    "nix_development_shells",
    "nixos_modules",
    "nix_produced_oci",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BuildQualificationError {
    InvalidContract,
    InvalidFixture,
}

impl fmt::Display for BuildQualificationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidContract => "service build qualification contract is invalid",
            Self::InvalidFixture => "service build qualification fixture is invalid",
        })
    }
}

impl std::error::Error for BuildQualificationError {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BuildQualificationDecision {
    schema: String,
    contract_version: u32,
    decision_state: String,
    predecessor: PredecessorDecision,
    qualification_scope: String,
    fixture_root: String,
    supported_rust_targets: Vec<String>,
    required_native_commands: Vec<String>,
    required_xtask_commands: Vec<String>,
    required_evidence: Vec<String>,
    fixture_source_lock: String,
    fixture_contract: String,
    release_artifact_command: String,
    source_lock_command: String,
    signing_authority: String,
    deferred_outputs: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PredecessorDecision {
    schema: String,
    filename: String,
    transition: String,
}

pub(crate) fn validate_contract(workspace_root: &Path) -> Result<(), String> {
    validate_contract_inner(workspace_root).map_err(|error| error.to_string())
}

fn validate_contract_inner(workspace_root: &Path) -> Result<(), BuildQualificationError> {
    let bytes = read_bounded(
        &workspace_root.join(CONTRACT_RELATIVE),
        MAX_CONTRACT_BYTES,
        BuildQualificationError::InvalidContract,
    )?;
    let decision = serde_json::from_slice::<BuildQualificationDecision>(&bytes)
        .map_err(|_| BuildQualificationError::InvalidContract)?;
    validate_decision(&decision)?;
    validate_fixture(workspace_root)
}

fn validate_decision(decision: &BuildQualificationDecision) -> Result<(), BuildQualificationError> {
    let exact = decision.schema == "radroots.services-hardening.build-qualification-decisions.v2"
        && decision.contract_version == 2
        && decision.decision_state == "active"
        && decision.predecessor.schema
            == "radroots.services-hardening.build-qualification-decisions.v1"
        && decision.predecessor.filename == "services_hardening_build_qualification.v1.json"
        && decision.predecessor.transition == "forward_only_replace"
        && decision.qualification_scope == "native_release_foundation"
        && decision.fixture_root == FIXTURE_RELATIVE
        && decision.supported_rust_targets == SUPPORTED_RUST_TARGETS
        && decision.required_native_commands == REQUIRED_NATIVE_COMMANDS
        && decision.required_xtask_commands == REQUIRED_XTASK_COMMANDS
        && decision.required_evidence == REQUIRED_EVIDENCE
        && decision.fixture_source_lock
            == "tools/xtask/fixtures/service-build-qualification/radroots.service.source-lock.v2.toml"
        && decision.fixture_contract == "source_lock_package_and_release_metadata_exact_agreement"
        && decision.release_artifact_command == "cargo xtask service-release-artifacts"
        && decision.source_lock_command == "cargo xtask service-source-lock"
        && decision.signing_authority == "external_only"
        && decision.deferred_outputs == DEFERRED_OUTPUTS;
    if exact {
        Ok(())
    } else {
        Err(BuildQualificationError::InvalidContract)
    }
}

fn validate_fixture(workspace_root: &Path) -> Result<(), BuildQualificationError> {
    let fixture = workspace_root.join(FIXTURE_RELATIVE);
    let lock_bytes = read_bounded(
        &fixture.join(LOCK_FILENAME),
        4_096,
        BuildQualificationError::InvalidFixture,
    )?;
    let lock = ServiceSourceLockV2::from_canonical_bytes(&lock_bytes)
        .map_err(|_| BuildQualificationError::InvalidFixture)?;
    let cargo_lock = read_bounded(
        &fixture.join("Cargo.lock"),
        MAX_FIXTURE_FILE_BYTES,
        BuildQualificationError::InvalidFixture,
    )?;
    let manifest = read_bounded(
        &fixture.join("Cargo.toml"),
        MAX_FIXTURE_FILE_BYTES,
        BuildQualificationError::InvalidFixture,
    )?;
    let manifest =
        std::str::from_utf8(&manifest).map_err(|_| BuildQualificationError::InvalidFixture)?;
    let manifest = toml::from_str::<toml::Value>(manifest)
        .map_err(|_| BuildQualificationError::InvalidFixture)?;
    let source_metadata = manifest
        .get("workspace")
        .and_then(|value| value.get("metadata"))
        .and_then(|value| value.get("radroots"))
        .and_then(|value| value.get("service_source_lock"))
        .and_then(toml::Value::as_table)
        .ok_or(BuildQualificationError::InvalidFixture)?;
    let release_metadata = manifest
        .get("workspace")
        .and_then(|value| value.get("metadata"))
        .and_then(|value| value.get("radroots"))
        .and_then(|value| value.get("service_release"))
        .and_then(toml::Value::as_table)
        .ok_or(BuildQualificationError::InvalidFixture)?;
    let versions = lock.contract_versions();
    let exact = lock.service() == "fixture_service"
        && lock.revision() == "2222222222222222222222222222222222222222"
        && lock.cargo_lock_sha256() == digest(&cargo_lock)
        && lock.nix_material_state() == NixMaterialState::Absent
        && lock.nix_lib_revision().is_none()
        && lock.flake_lock_sha256().is_none()
        && ["flake.nix", "flake.lock", PREDECESSOR_LOCK_FILENAME]
            .into_iter()
            .all(|name| {
                fs::symlink_metadata(fixture.join(name))
                    .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound)
            })
        && versions.config() == 1
        && versions.state() == 2
        && versions.admin() == 3
        && versions.status() == 4
        && versions.provider() == 5
        && manifest
            .get("package")
            .and_then(|value| value.get("version"))
            .and_then(toml::Value::as_str)
            == Some("0.1.0-alpha")
        && source_metadata.len() == 8
        && source_metadata.get("service").and_then(toml::Value::as_str) == Some("fixture_service")
        && source_metadata
            .get("host_feature_profile")
            .and_then(toml::Value::as_str)
            == Some("service-host")
        && source_metadata
            .get("nix_material")
            .and_then(toml::Value::as_str)
            == Some("absent")
        && source_metadata
            .get("config_contract_version")
            .and_then(toml::Value::as_integer)
            == Some(i64::from(versions.config()))
        && source_metadata
            .get("state_contract_version")
            .and_then(toml::Value::as_integer)
            == Some(i64::from(versions.state()))
        && source_metadata
            .get("admin_contract_version")
            .and_then(toml::Value::as_integer)
            == Some(i64::from(versions.admin()))
        && source_metadata
            .get("status_contract_version")
            .and_then(toml::Value::as_integer)
            == Some(i64::from(versions.status()))
        && source_metadata
            .get("provider_contract_version")
            .and_then(toml::Value::as_integer)
            == Some(i64::from(versions.provider()))
        && release_metadata.len() == 4
        && release_metadata
            .get("service")
            .and_then(toml::Value::as_str)
            == Some("fixture_service")
        && release_metadata
            .get("service_package")
            .and_then(toml::Value::as_str)
            == Some("fixture-service")
        && release_metadata
            .get("binary_name")
            .and_then(toml::Value::as_str)
            == Some("fixture-service")
        && release_metadata
            .get("version")
            .and_then(toml::Value::as_str)
            == Some("0.1.0-alpha");
    if exact {
        Ok(())
    } else {
        Err(BuildQualificationError::InvalidFixture)
    }
}

fn read_bounded(
    path: &Path,
    maximum: usize,
    error: BuildQualificationError,
) -> Result<Vec<u8>, BuildQualificationError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| error)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > maximum as u64 {
        return Err(error);
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
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

fn digest(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use std::error::Error as _;

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn checked_in_contract_and_fixture_are_exact() {
        let root = workspace_root();
        validate_contract_inner(root).expect("build qualification");
    }

    #[test]
    fn contract_rejects_every_independent_governed_field_drift() {
        let bytes = fs::read(workspace_root().join(CONTRACT_RELATIVE)).expect("decision");
        let canonical = serde_json::from_slice::<serde_json::Value>(&bytes).expect("decision json");
        for (pointer, replacement) in [
            ("/schema", serde_json::json!("other")),
            ("/contract_version", serde_json::json!(1)),
            ("/decision_state", serde_json::json!("draft")),
            ("/predecessor/schema", serde_json::json!("other")),
            ("/predecessor/filename", serde_json::json!("other")),
            ("/predecessor/transition", serde_json::json!("other")),
            ("/qualification_scope", serde_json::json!("other")),
            ("/fixture_root", serde_json::json!("other")),
            ("/supported_rust_targets", serde_json::json!([])),
            ("/required_native_commands", serde_json::json!([])),
            ("/required_xtask_commands", serde_json::json!([])),
            ("/required_evidence", serde_json::json!([])),
            ("/fixture_source_lock", serde_json::json!("other")),
            ("/fixture_contract", serde_json::json!("other")),
            ("/release_artifact_command", serde_json::json!("other")),
            ("/source_lock_command", serde_json::json!("other")),
            ("/signing_authority", serde_json::json!("internal")),
            ("/deferred_outputs", serde_json::json!([])),
        ] {
            let mut drifted = canonical.clone();
            *drifted.pointer_mut(pointer).expect("governed field") = replacement;
            let decision = serde_json::from_value::<BuildQualificationDecision>(drifted)
                .expect("structurally valid drift");
            assert_eq!(
                validate_decision(&decision),
                Err(BuildQualificationError::InvalidContract),
                "accepted drift at {pointer}"
            );
        }
    }

    #[test]
    fn fixture_rejects_every_identity_and_lockfile_drift() {
        for (name, from, to) in [
            (
                "Cargo.toml",
                "version = \"0.1.0-alpha\"",
                "version = \"0.1.1\"",
            ),
            (
                "Cargo.toml",
                "config_contract_version = 1",
                "config_contract_version = 9",
            ),
            (
                "Cargo.toml",
                "service_package = \"fixture-service\"",
                "service_package = \"other-service\"",
            ),
            (
                "radroots.service.source-lock.v2.toml",
                "revision = \"2222222222222222222222222222222222222222\"",
                "revision = \"3333333333333333333333333333333333333333\"",
            ),
            ("Cargo.lock", "version = 4", "version = 3"),
        ] {
            let root = copied_fixture();
            let path = root.path().join(FIXTURE_RELATIVE).join(name);
            let current = fs::read_to_string(&path).expect("fixture text");
            assert!(current.contains(from), "missing mutation anchor {from}");
            fs::write(&path, current.replacen(from, to, 1)).expect("mutated fixture");
            assert_eq!(
                validate_fixture(root.path()),
                Err(BuildQualificationError::InvalidFixture)
            );
        }

        let root = copied_fixture();
        let manifest = root.path().join(FIXTURE_RELATIVE).join("Cargo.toml");
        let mut current = fs::read_to_string(&manifest).expect("fixture manifest");
        current.push_str("\n[workspace.metadata.radroots.service_release.extra]\nvalue = 1\n");
        fs::write(manifest, current).expect("extra fixture metadata");
        assert_eq!(
            validate_fixture(root.path()),
            Err(BuildQualificationError::InvalidFixture)
        );

        for name in ["flake.nix", "flake.lock", PREDECESSOR_LOCK_FILENAME] {
            let root = copied_fixture();
            fs::write(root.path().join(FIXTURE_RELATIVE).join(name), b"unexpected")
                .expect("unexpected Nix material");
            assert_eq!(
                validate_fixture(root.path()),
                Err(BuildQualificationError::InvalidFixture),
                "accepted absent-state fixture with {name}"
            );
        }
    }

    #[test]
    fn fixture_rejects_every_independent_metadata_drift() {
        for (section, field, replacement) in [
            (
                "service_source_lock",
                "service",
                toml::Value::String("other".into()),
            ),
            (
                "service_source_lock",
                "host_feature_profile",
                toml::Value::String("other".into()),
            ),
            (
                "service_source_lock",
                "config_contract_version",
                toml::Value::Integer(9),
            ),
            (
                "service_source_lock",
                "state_contract_version",
                toml::Value::Integer(9),
            ),
            (
                "service_source_lock",
                "admin_contract_version",
                toml::Value::Integer(9),
            ),
            (
                "service_source_lock",
                "status_contract_version",
                toml::Value::Integer(9),
            ),
            (
                "service_source_lock",
                "provider_contract_version",
                toml::Value::Integer(9),
            ),
            (
                "service_release",
                "service",
                toml::Value::String("other".into()),
            ),
            (
                "service_release",
                "service_package",
                toml::Value::String("other".into()),
            ),
            (
                "service_release",
                "binary_name",
                toml::Value::String("other".into()),
            ),
            (
                "service_release",
                "version",
                toml::Value::String("0.2.0".into()),
            ),
        ] {
            let root = copied_fixture();
            let path = root.path().join(FIXTURE_RELATIVE).join("Cargo.toml");
            let current = fs::read_to_string(&path).expect("fixture manifest");
            let mut manifest = toml::from_str::<toml::Value>(&current).expect("fixture toml");
            manifest["workspace"]["metadata"]["radroots"][section][field] = replacement;
            fs::write(&path, toml::to_string(&manifest).expect("render fixture"))
                .expect("mutated fixture");
            assert_eq!(
                validate_fixture(root.path()),
                Err(BuildQualificationError::InvalidFixture),
                "accepted {section}.{field} drift"
            );
        }

        for section in ["service_source_lock", "service_release"] {
            let root = copied_fixture();
            let path = root.path().join(FIXTURE_RELATIVE).join("Cargo.toml");
            let current = fs::read_to_string(&path).expect("fixture manifest");
            let mut manifest = toml::from_str::<toml::Value>(&current).expect("fixture toml");
            manifest["workspace"]["metadata"]["radroots"][section]
                .as_table_mut()
                .expect("metadata section")
                .insert("extra".into(), toml::Value::Integer(1));
            fs::write(&path, toml::to_string(&manifest).expect("render fixture"))
                .expect("mutated fixture");
            assert_eq!(
                validate_fixture(root.path()),
                Err(BuildQualificationError::InvalidFixture),
                "accepted {section} field-count drift"
            );
        }

        let root = copied_fixture();
        let path = root.path().join(FIXTURE_RELATIVE).join("Cargo.toml");
        let current = fs::read_to_string(&path).expect("fixture manifest");
        let mut manifest = toml::from_str::<toml::Value>(&current).expect("fixture toml");
        manifest["package"]["version"] = toml::Value::String("0.2.0".into());
        fs::write(&path, toml::to_string(&manifest).expect("render fixture"))
            .expect("mutated fixture");
        assert_eq!(
            validate_fixture(root.path()),
            Err(BuildQualificationError::InvalidFixture)
        );
    }

    #[test]
    fn contract_inventory_is_literal_and_complete() {
        assert_eq!(SUPPORTED_RUST_TARGETS.len(), 4);
        assert_eq!(REQUIRED_NATIVE_COMMANDS.len(), 6);
        assert_eq!(REQUIRED_XTASK_COMMANDS.len(), 5);
        assert_eq!(REQUIRED_EVIDENCE.len(), 11);
        assert_eq!(DEFERRED_OUTPUTS.len(), 6);
    }

    #[test]
    fn errors_are_fixed_and_source_free() {
        for error in [
            BuildQualificationError::InvalidContract,
            BuildQualificationError::InvalidFixture,
        ] {
            assert!(!error.to_string().contains("fixture_service"));
            assert!(error.source().is_none());
        }
    }

    fn workspace_root() -> &'static Path {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("workspace root")
    }

    fn copied_fixture() -> TempDir {
        let root = TempDir::new().expect("fixture root");
        let destination = root.path().join(FIXTURE_RELATIVE);
        fs::create_dir_all(&destination).expect("fixture directory");
        let source = workspace_root().join(FIXTURE_RELATIVE);
        for name in ["Cargo.toml", "Cargo.lock", LOCK_FILENAME] {
            fs::copy(source.join(name), destination.join(name)).expect("fixture file");
        }
        root
    }
}
