use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsStr,
    fmt, fs,
    io::{Read as _, Write as _},
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
};

use flate2::{Compression, GzBuilder};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use tar::{Builder as TarBuilder, Header as TarHeader};
use tempfile::TempDir;

use crate::service_source_lock::{LIB_REPOSITORY, LOCK_FILENAME, ServiceSourceLockV1};

const CONTRACT_RELATIVE: &str =
    "contracts/architecture/decisions/services_hardening_release_artifacts.v1.json";
const INPUT_NAMES: [&str; 8] = [
    "config.example.toml",
    "config.schema.json",
    "lib-source.bundle",
    "nixos-module.nix",
    "oci-image.tar.gz",
    "service-binary",
    "service-source.bundle",
    "systemd.service",
];
const OUTPUT_NAMES: [&str; 18] = [
    "LICENSE-APACHE",
    "LICENSE-MIT",
    "SHA256SUMS",
    "THIRD-PARTY-NOTICES.txt",
    "artifact-manifest.v1.json",
    "binary.tar.gz",
    "config.example.toml",
    "config.schema.json",
    "lib-source.bundle",
    "nixos-module.nix",
    "oci-image.tar.gz",
    "oci-image.v1.json",
    "provenance-input.v1.json",
    "radroots.service.source-lock.v1.toml",
    "sbom.cdx.json",
    "service-source.bundle",
    "source-bundles.v1.json",
    "systemd.service",
];
const SUPPORTED_TARGETS: [&str; 2] = ["aarch64-unknown-linux-gnu", "x86_64-unknown-linux-gnu"];
const SECRET_PATTERNS: [&[u8]; 7] = [
    b"-----BEGIN PRIVATE KEY-----",
    b"-----BEGIN RSA PRIVATE KEY-----",
    b"-----BEGIN EC PRIVATE KEY-----",
    b"-----BEGIN OPENSSH PRIVATE KEY-----",
    b"github_pat_",
    b"ghp_",
    b"xoxb-",
];
const MAX_CONTRACT_BYTES: usize = 65_536;
const MAX_MANIFEST_BYTES: usize = 1_048_576;
const MAX_TEXT_INPUT_BYTES: u64 = 1_048_576;
const MAX_GENERATED_DOCUMENT_BYTES: u64 = 16_777_216;
const MAX_SOURCE_LOCK_BYTES: u64 = 4_096;
const MAX_SERVICE_CARGO_LOCK_BYTES: u64 = 16_777_216;
const MAX_SERVICE_FLAKE_LOCK_BYTES: u64 = 4_194_304;
const MAX_BINARY_BYTES: u64 = 536_870_912;
const MAX_SOURCE_BUNDLE_BYTES: u64 = 1_073_741_824;
const MAX_OCI_BYTES: u64 = 2_147_483_648;
const MAX_METADATA_BYTES: usize = 33_554_432;
const MAX_GIT_OUTPUT_BYTES: usize = 65_536;
const MAX_PACKAGES: usize = 8_192;
const MAX_WORKSPACE_PACKAGES: usize = 64;
const MAX_TEXT_FIELD_BYTES: usize = 512;
const FILE_MODE: u32 = 0o644;
const DIRECTORY_MODE: u32 = 0o755;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CommandMode {
    Check,
    Write,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReleaseArtifactError {
    InvalidContract,
    InvalidServiceRoot,
    DirtyServiceSource,
    InvalidServiceMetadata,
    InvalidInputRoot,
    InvalidInputArtifact,
    InvalidSourceLock,
    InvalidSourceBundle,
    InvalidPackageInventory,
    ProtectedMaterialDetected,
    InvalidOutputRoot,
    StaleOutput,
    GenerationFailure,
}

impl ReleaseArtifactError {
    const fn code(self) -> &'static str {
        match self {
            Self::InvalidContract => "invalid_contract",
            Self::InvalidServiceRoot => "invalid_service_root",
            Self::DirtyServiceSource => "dirty_service_source",
            Self::InvalidServiceMetadata => "invalid_service_metadata",
            Self::InvalidInputRoot => "invalid_input_root",
            Self::InvalidInputArtifact => "invalid_input_artifact",
            Self::InvalidSourceLock => "invalid_source_lock",
            Self::InvalidSourceBundle => "invalid_source_bundle",
            Self::InvalidPackageInventory => "invalid_package_inventory",
            Self::ProtectedMaterialDetected => "protected_material_detected",
            Self::InvalidOutputRoot => "invalid_output_root",
            Self::StaleOutput => "stale_output",
            Self::GenerationFailure => "generation_failure",
        }
    }
}

impl fmt::Display for ReleaseArtifactError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidContract => "service release artifact contract is invalid",
            Self::InvalidServiceRoot => "service release source root is invalid",
            Self::DirtyServiceSource => "service release source contains an ungoverned change",
            Self::InvalidServiceMetadata => "service release metadata is invalid",
            Self::InvalidInputRoot => "service release input root is invalid",
            Self::InvalidInputArtifact => "service release input artifact is invalid",
            Self::InvalidSourceLock => "service release source lock is invalid",
            Self::InvalidSourceBundle => "service release source bundle is invalid",
            Self::InvalidPackageInventory => "service release package inventory is invalid",
            Self::ProtectedMaterialDetected => "service release input contains protected material",
            Self::InvalidOutputRoot => "service release output root is invalid",
            Self::StaleOutput => "service release artifact set is stale",
            Self::GenerationFailure => "service release artifacts could not be generated",
        })
    }
}

impl std::error::Error for ReleaseArtifactError {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReleaseMetadata {
    service: String,
    service_package: String,
    binary_name: String,
    version: String,
}

#[derive(Debug, Deserialize)]
struct CargoMetadata {
    packages: Vec<CargoPackage>,
    workspace_members: Vec<String>,
    resolve: Option<CargoResolve>,
}

#[derive(Debug, Deserialize)]
struct CargoPackage {
    id: String,
    name: String,
    version: String,
    source: Option<String>,
    checksum: Option<String>,
    license: Option<String>,
    targets: Vec<CargoTarget>,
}

#[derive(Debug, Deserialize)]
struct CargoTarget {
    name: String,
    kind: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CargoResolve {
    nodes: Vec<CargoNode>,
}

#[derive(Debug, Deserialize)]
struct CargoNode {
    id: String,
    dependencies: Vec<String>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
struct DigestValue {
    alg: &'static str,
    content: String,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
struct LicenseChoice {
    expression: String,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
struct SbomComponent {
    #[serde(rename = "type")]
    component_type: &'static str,
    #[serde(rename = "bom-ref")]
    bom_ref: String,
    name: String,
    version: String,
    purl: String,
    licenses: Vec<LicenseChoice>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    hashes: Vec<DigestValue>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct SbomDependency {
    #[serde(rename = "ref")]
    reference: String,
    depends_on: Vec<String>,
}

#[derive(Debug, Serialize)]
struct SbomMetadata {
    component: SbomComponent,
}

#[derive(Debug, Serialize)]
struct CycloneDxSbom {
    #[serde(rename = "bomFormat")]
    bom_format: &'static str,
    #[serde(rename = "specVersion")]
    spec_version: &'static str,
    version: u32,
    metadata: SbomMetadata,
    components: Vec<SbomComponent>,
    dependencies: Vec<SbomDependency>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
struct ArtifactRecord {
    path: String,
    byte_length: u64,
    sha256: String,
}

#[derive(Debug, Serialize)]
struct ContractVersionsDocument {
    config: u32,
    state: u32,
    admin: u32,
    status: u32,
    provider: u32,
}

#[derive(Debug, Serialize)]
struct SourceBundleDocument {
    schema: &'static str,
    contract_version: u32,
    service: String,
    service_revision: String,
    lib_repository: &'static str,
    lib_revision: String,
    service_source: ArtifactRecord,
    lib_source: ArtifactRecord,
    source_lock_sha256: String,
    workspace_catalog_sha256: String,
    cargo_lock_sha256: String,
    flake_lock_sha256: String,
}

#[derive(Debug, Serialize)]
struct OciImageDocument {
    schema: &'static str,
    contract_version: u32,
    service: String,
    version: String,
    target: String,
    image: ArtifactRecord,
}

#[derive(Debug, Serialize)]
struct ArtifactManifestDocument {
    schema: &'static str,
    contract_version: u32,
    service: String,
    version: String,
    target: String,
    source_date_epoch: u32,
    service_revision: String,
    lib_revision: String,
    rust_version: &'static str,
    host_feature_profile: &'static str,
    contract_versions: ContractVersionsDocument,
    protected_material_included: bool,
    artifacts: Vec<ArtifactRecord>,
}

#[derive(Debug, Serialize)]
struct ProvenanceInputDocument {
    schema: &'static str,
    contract_version: u32,
    predicate_type: &'static str,
    build_type: &'static str,
    builder_id: &'static str,
    service: String,
    version: String,
    target: String,
    source_date_epoch: u32,
    service_repository: String,
    service_revision: String,
    lib_repository: &'static str,
    lib_revision: String,
    source_lock_sha256: String,
    manifest_sha256: String,
    subjects: Vec<ArtifactRecord>,
    signing_required: bool,
}

#[derive(Clone, Debug)]
struct FileEvidence {
    byte_length: u64,
    sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReleaseDecision {
    schema: String,
    contract_version: u32,
    decision_state: String,
    command: String,
    modes: Vec<String>,
    required_arguments: Vec<String>,
    service_metadata_path: String,
    service_metadata_fields: Vec<String>,
    supported_targets: Vec<String>,
    input_inventory: Vec<String>,
    excluded_parent_owned_inputs: Vec<String>,
    service_root_inventory: Vec<String>,
    output_inventory: Vec<String>,
    canonical_json: String,
    checksum_format: String,
    sbom_format: String,
    provenance_posture: String,
    protected_material_scan_scope: String,
    source_cleanliness: String,
    revision_stability: String,
    no_protected_material: bool,
    maximums: ReleaseMaximums,
    negative_error_codes: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReleaseMaximums {
    text_input_bytes: u64,
    generated_document_bytes: u64,
    service_cargo_lock_bytes: u64,
    service_flake_lock_bytes: u64,
    binary_bytes: u64,
    source_bundle_bytes: u64,
    oci_bytes: u64,
    cargo_metadata_bytes: usize,
    packages: usize,
    workspace_packages: usize,
}

pub(crate) fn run(
    mode: CommandMode,
    service_root: &Path,
    input_root: &Path,
    output_root: &Path,
    target: &str,
    source_date_epoch: u32,
) -> Result<(), String> {
    run_inner(
        mode,
        service_root,
        input_root,
        output_root,
        target,
        source_date_epoch,
    )
    .map_err(|error| error.to_string())
}

fn run_inner(
    mode: CommandMode,
    service_root: &Path,
    input_root: &Path,
    output_root: &Path,
    target: &str,
    source_date_epoch: u32,
) -> Result<(), ReleaseArtifactError> {
    if !SUPPORTED_TARGETS.contains(&target) || source_date_epoch == 0 {
        return Err(ReleaseArtifactError::InvalidServiceMetadata);
    }
    let service_root = validate_git_root(service_root)?;
    let input_root = validate_exact_input_root(input_root)?;
    let (output_parent, output_root) =
        validate_output_parent(output_root, &service_root, &input_root)?;
    let initial_head = git_head(&service_root)?;
    validate_clean_git(&service_root)?;

    let metadata = read_release_metadata(&service_root)?;
    let source_lock_bytes = read_bounded_regular(
        &service_root.join(LOCK_FILENAME),
        MAX_SOURCE_LOCK_BYTES,
        ReleaseArtifactError::InvalidSourceLock,
    )?;
    let source_lock = ServiceSourceLockV1::from_canonical_bytes(&source_lock_bytes)
        .map_err(|_| ReleaseArtifactError::InvalidSourceLock)?;
    if source_lock.service() != metadata.service {
        return Err(ReleaseArtifactError::InvalidSourceLock);
    }
    validate_source_lock_files(&service_root, &source_lock)?;
    let cargo_metadata = cargo_metadata(&service_root)?;
    let (sbom, notices) = build_supply_chain_documents(&metadata, cargo_metadata)?;

    let staging = tempfile::Builder::new()
        .prefix(".radroots-service-release-")
        .tempdir_in(&output_parent)
        .map_err(|_| ReleaseArtifactError::GenerationFailure)?;
    set_directory_mode(staging.path())?;

    let static_inputs = [
        ("config.example.toml", "config.example.toml"),
        ("config.schema.json", "config.schema.json"),
        ("systemd.service", "systemd.service"),
        ("nixos-module.nix", "nixos-module.nix"),
    ];
    for (input, output) in static_inputs {
        copy_bounded(
            &input_root.join(input),
            &staging.path().join(output),
            MAX_TEXT_INPUT_BYTES,
        )?;
        validate_text_artifact(&staging.path().join(output), output)?;
    }
    copy_bounded(
        &service_root.join("LICENSE-APACHE"),
        &staging.path().join("LICENSE-APACHE"),
        MAX_TEXT_INPUT_BYTES,
    )?;
    validate_text_artifact(&staging.path().join("LICENSE-APACHE"), "LICENSE-APACHE")?;
    copy_bounded(
        &service_root.join("LICENSE-MIT"),
        &staging.path().join("LICENSE-MIT"),
        MAX_TEXT_INPUT_BYTES,
    )?;
    validate_text_artifact(&staging.path().join("LICENSE-MIT"), "LICENSE-MIT")?;
    write_generated(&staging.path().join(LOCK_FILENAME), &source_lock_bytes)?;
    create_binary_archive(
        &input_root.join("service-binary"),
        &staging.path().join("binary.tar.gz"),
        &metadata.binary_name,
        source_date_epoch,
    )?;
    let oci = copy_bounded(
        &input_root.join("oci-image.tar.gz"),
        &staging.path().join("oci-image.tar.gz"),
        MAX_OCI_BYTES,
    )?;
    let service_source = copy_bounded(
        &input_root.join("service-source.bundle"),
        &staging.path().join("service-source.bundle"),
        MAX_SOURCE_BUNDLE_BYTES,
    )?;
    let lib_source = copy_bounded(
        &input_root.join("lib-source.bundle"),
        &staging.path().join("lib-source.bundle"),
        MAX_SOURCE_BUNDLE_BYTES,
    )?;
    verify_bundle(&staging.path().join("service-source.bundle"), &initial_head)?;
    verify_bundle(
        &staging.path().join("lib-source.bundle"),
        source_lock.revision(),
    )?;
    if lib_source.sha256 != source_lock.source_archive_sha256() {
        return Err(ReleaseArtifactError::InvalidSourceBundle);
    }

    let oci_document = OciImageDocument {
        schema: "radroots.service.oci-image.v1",
        contract_version: 1,
        service: metadata.service.clone(),
        version: metadata.version.clone(),
        target: target.to_owned(),
        image: artifact_record("oci-image.tar.gz", &oci),
    };
    write_json(&staging.path().join("oci-image.v1.json"), &oci_document)?;
    let source_lock_sha256 = sha256_bytes(&source_lock_bytes);
    let source_document = SourceBundleDocument {
        schema: "radroots.service.source-bundles.v1",
        contract_version: 1,
        service: metadata.service.clone(),
        service_revision: initial_head.clone(),
        lib_repository: LIB_REPOSITORY,
        lib_revision: source_lock.revision().to_owned(),
        service_source: artifact_record("service-source.bundle", &service_source),
        lib_source: artifact_record("lib-source.bundle", &lib_source),
        source_lock_sha256: source_lock_sha256.clone(),
        workspace_catalog_sha256: source_lock.workspace_catalog_sha256().to_owned(),
        cargo_lock_sha256: source_lock.cargo_lock_sha256().to_owned(),
        flake_lock_sha256: source_lock.flake_lock_sha256().to_owned(),
    };
    write_json(
        &staging.path().join("source-bundles.v1.json"),
        &source_document,
    )?;
    write_json(&staging.path().join("sbom.cdx.json"), &sbom)?;
    write_generated(
        &staging.path().join("THIRD-PARTY-NOTICES.txt"),
        notices.as_bytes(),
    )?;

    let payload = inventory_records(staging.path())?;
    let versions = source_lock.contract_versions();
    let manifest = ArtifactManifestDocument {
        schema: "radroots.service.release-artifacts.v1",
        contract_version: 1,
        service: metadata.service.clone(),
        version: metadata.version.clone(),
        target: target.to_owned(),
        source_date_epoch,
        service_revision: initial_head.clone(),
        lib_revision: source_lock.revision().to_owned(),
        rust_version: "1.97.1",
        host_feature_profile: "service-host",
        contract_versions: ContractVersionsDocument {
            config: versions.config(),
            state: versions.state(),
            admin: versions.admin(),
            status: versions.status(),
            provider: versions.provider(),
        },
        protected_material_included: false,
        artifacts: payload,
    };
    write_json(&staging.path().join("artifact-manifest.v1.json"), &manifest)?;
    let manifest_evidence = hash_regular(
        &staging.path().join("artifact-manifest.v1.json"),
        MAX_TEXT_INPUT_BYTES,
    )?;
    let service_repository = git_remote(&service_root)?;
    let provenance = ProvenanceInputDocument {
        schema: "radroots.service.provenance-input.v1",
        contract_version: 1,
        predicate_type: "https://slsa.dev/provenance/v1",
        build_type: "https://radroots.dev/contracts/service-release-artifacts/v1",
        builder_id: "https://radroots.dev/builders/service-release-artifacts/v1",
        service: metadata.service,
        version: metadata.version,
        target: target.to_owned(),
        source_date_epoch,
        service_repository,
        service_revision: initial_head.clone(),
        lib_repository: LIB_REPOSITORY,
        lib_revision: source_lock.revision().to_owned(),
        source_lock_sha256,
        manifest_sha256: manifest_evidence.sha256,
        subjects: inventory_records(staging.path())?,
        signing_required: true,
    };
    write_json(
        &staging.path().join("provenance-input.v1.json"),
        &provenance,
    )?;
    write_checksums(staging.path())?;
    validate_exact_output_inventory(staging.path())?;
    sync_directory(staging.path())?;
    let expected_output = inventory_records(staging.path())?;

    validate_clean_git(&service_root)?;
    if git_head(&service_root)? != initial_head {
        return Err(ReleaseArtifactError::DirtyServiceSource);
    }

    if output_root.exists() {
        compare_output(staging.path(), &output_root)?;
        return Ok(());
    }
    if mode == CommandMode::Check {
        return Err(ReleaseArtifactError::StaleOutput);
    }
    let staging_path = staging.keep();
    fs::rename(&staging_path, &output_root).map_err(|_| ReleaseArtifactError::GenerationFailure)?;
    sync_directory(&output_parent)?;
    validate_output_records(&output_root, &expected_output)
}

fn read_release_metadata(root: &Path) -> Result<ReleaseMetadata, ReleaseArtifactError> {
    let bytes = read_bounded_regular(
        &root.join("Cargo.toml"),
        MAX_MANIFEST_BYTES as u64,
        ReleaseArtifactError::InvalidServiceMetadata,
    )?;
    let text =
        std::str::from_utf8(&bytes).map_err(|_| ReleaseArtifactError::InvalidServiceMetadata)?;
    let value = toml::from_str::<toml::Value>(text)
        .map_err(|_| ReleaseArtifactError::InvalidServiceMetadata)?;
    let release = value
        .get("workspace")
        .and_then(|value| value.get("metadata"))
        .and_then(|value| value.get("radroots"))
        .and_then(|value| value.get("service_release"))
        .cloned()
        .ok_or(ReleaseArtifactError::InvalidServiceMetadata)?;
    let metadata = release
        .try_into::<ReleaseMetadata>()
        .map_err(|_| ReleaseArtifactError::InvalidServiceMetadata)?;
    if !valid_snake_identifier(&metadata.service)
        || !valid_kebab_identifier(&metadata.service_package)
        || !valid_kebab_identifier(&metadata.binary_name)
        || metadata.version.len() > 128
        || !matches!(
            semver::Version::parse(&metadata.version),
            Ok(version) if version.to_string() == metadata.version
        )
    {
        return Err(ReleaseArtifactError::InvalidServiceMetadata);
    }
    Ok(metadata)
}

fn validate_source_lock_files(
    root: &Path,
    source_lock: &ServiceSourceLockV1,
) -> Result<(), ReleaseArtifactError> {
    let cargo_lock = hash_regular(&root.join("Cargo.lock"), MAX_SERVICE_CARGO_LOCK_BYTES)
        .map_err(|_| ReleaseArtifactError::InvalidSourceLock)?;
    let flake_lock = hash_regular(&root.join("flake.lock"), MAX_SERVICE_FLAKE_LOCK_BYTES)
        .map_err(|_| ReleaseArtifactError::InvalidSourceLock)?;
    if cargo_lock.sha256 == source_lock.cargo_lock_sha256()
        && flake_lock.sha256 == source_lock.flake_lock_sha256()
    {
        Ok(())
    } else {
        Err(ReleaseArtifactError::InvalidSourceLock)
    }
}

fn cargo_metadata(root: &Path) -> Result<CargoMetadata, ReleaseArtifactError> {
    let mut command = Command::new("cargo");
    command
        .args(["metadata", "--format-version", "1", "--locked", "--offline"])
        .current_dir(root);
    let bytes = command_stdout(&mut command, MAX_METADATA_BYTES)
        .map_err(|_| ReleaseArtifactError::InvalidPackageInventory)?;
    serde_json::from_slice(&bytes).map_err(|_| ReleaseArtifactError::InvalidPackageInventory)
}

fn build_supply_chain_documents(
    metadata: &ReleaseMetadata,
    cargo: CargoMetadata,
) -> Result<(CycloneDxSbom, String), ReleaseArtifactError> {
    if cargo.packages.is_empty()
        || cargo.packages.len() > MAX_PACKAGES
        || cargo.workspace_members.is_empty()
        || cargo.workspace_members.len() > MAX_WORKSPACE_PACKAGES
    {
        return Err(ReleaseArtifactError::InvalidPackageInventory);
    }
    let workspace = cargo.workspace_members.into_iter().collect::<BTreeSet<_>>();
    let mut by_id = BTreeMap::new();
    let mut root_id = None;
    for package in cargo.packages {
        validate_metadata_package(&package, workspace.contains(&package.id))?;
        if package.name == metadata.service_package
            && (root_id.replace(package.id.clone()).is_some()
                || package.version != metadata.version
                || !package.targets.iter().any(|target| {
                    target.name == metadata.binary_name
                        && target.kind.iter().any(|kind| kind == "bin")
                })
                || !workspace.contains(&package.id))
        {
            return Err(ReleaseArtifactError::InvalidPackageInventory);
        }
        if by_id.insert(package.id.clone(), package).is_some() {
            return Err(ReleaseArtifactError::InvalidPackageInventory);
        }
    }
    let root_id = root_id.ok_or(ReleaseArtifactError::InvalidPackageInventory)?;
    if by_id.len() > MAX_PACKAGES || !workspace.iter().all(|id| by_id.contains_key(id)) {
        return Err(ReleaseArtifactError::InvalidPackageInventory);
    }
    let references = by_id
        .values()
        .map(|package| (package.id.clone(), package_reference(package)))
        .collect::<BTreeMap<_, _>>();
    let root_package = by_id
        .get(&root_id)
        .ok_or(ReleaseArtifactError::InvalidPackageInventory)?;
    let root_component = sbom_component(root_package, "application")?;
    let mut components = by_id
        .values()
        .filter(|package| package.id != root_id)
        .map(|package| sbom_component(package, "library"))
        .collect::<Result<Vec<_>, _>>()?;
    components.sort();
    let resolve = cargo
        .resolve
        .ok_or(ReleaseArtifactError::InvalidPackageInventory)?;
    if resolve.nodes.len() != by_id.len() || resolve.nodes.len() > MAX_PACKAGES {
        return Err(ReleaseArtifactError::InvalidPackageInventory);
    }
    let mut seen_nodes = BTreeSet::new();
    let mut dependencies = Vec::with_capacity(resolve.nodes.len());
    for node in resolve.nodes {
        if !seen_nodes.insert(node.id.clone()) {
            return Err(ReleaseArtifactError::InvalidPackageInventory);
        }
        let reference = references
            .get(&node.id)
            .ok_or(ReleaseArtifactError::InvalidPackageInventory)?
            .clone();
        let mut depends_on = node
            .dependencies
            .iter()
            .map(|dependency| {
                references
                    .get(dependency)
                    .cloned()
                    .ok_or(ReleaseArtifactError::InvalidPackageInventory)
            })
            .collect::<Result<Vec<_>, _>>()?;
        depends_on.sort();
        depends_on.dedup();
        dependencies.push(SbomDependency {
            reference,
            depends_on,
        });
    }
    dependencies.sort_by(|left, right| left.reference.cmp(&right.reference));
    let sbom = CycloneDxSbom {
        bom_format: "CycloneDX",
        spec_version: "1.5",
        version: 1,
        metadata: SbomMetadata {
            component: root_component,
        },
        components,
        dependencies,
    };

    let mut third_party = by_id
        .values()
        .filter(|package| !workspace.contains(&package.id))
        .collect::<Vec<_>>();
    third_party.sort_by(|left, right| {
        (&left.name, &left.version, &left.source).cmp(&(&right.name, &right.version, &right.source))
    });
    let mut notices = String::from(
        "Radroots service third-party notices v1\n\nThis inventory is generated from the locked Cargo dependency graph.\n",
    );
    if third_party.is_empty() {
        notices.push_str("\nNo third-party Cargo packages are present.\n");
    } else {
        for package in third_party {
            let license = package
                .license
                .as_deref()
                .ok_or(ReleaseArtifactError::InvalidPackageInventory)?;
            let source = package
                .source
                .as_deref()
                .ok_or(ReleaseArtifactError::InvalidPackageInventory)?;
            use fmt::Write as _;
            writeln!(notices).map_err(|_| ReleaseArtifactError::GenerationFailure)?;
            writeln!(notices, "Package: {} {}", package.name, package.version)
                .map_err(|_| ReleaseArtifactError::GenerationFailure)?;
            writeln!(notices, "License: {license}")
                .map_err(|_| ReleaseArtifactError::GenerationFailure)?;
            writeln!(notices, "Source: {source}")
                .map_err(|_| ReleaseArtifactError::GenerationFailure)?;
        }
    }
    scan_bytes(notices.as_bytes())?;
    Ok((sbom, notices))
}

fn validate_metadata_package(
    package: &CargoPackage,
    workspace_member: bool,
) -> Result<(), ReleaseArtifactError> {
    for value in [&package.name, &package.version, &package.id] {
        if value.is_empty() || value.len() > MAX_TEXT_FIELD_BYTES || value.contains(['\n', '\r']) {
            return Err(ReleaseArtifactError::InvalidPackageInventory);
        }
    }
    if package
        .license
        .as_ref()
        .is_some_and(|value| !valid_metadata_text(value))
        || package
            .source
            .as_ref()
            .is_some_and(|value| !valid_public_source(value))
        || (!workspace_member && (package.source.is_none() || package.license.is_none()))
        || package
            .checksum
            .as_ref()
            .is_some_and(|value| !valid_lower_hex(value, 64))
    {
        return Err(ReleaseArtifactError::InvalidPackageInventory);
    }
    Ok(())
}

fn sbom_component(
    package: &CargoPackage,
    component_type: &'static str,
) -> Result<SbomComponent, ReleaseArtifactError> {
    let license = package
        .license
        .clone()
        .unwrap_or_else(|| "NOASSERTION".to_owned());
    let hashes = package
        .checksum
        .iter()
        .map(|checksum| DigestValue {
            alg: "SHA-256",
            content: checksum.clone(),
        })
        .collect();
    Ok(SbomComponent {
        component_type,
        bom_ref: package_reference(package),
        name: package.name.clone(),
        version: package.version.clone(),
        purl: format!("pkg:cargo/{}@{}", package.name, package.version),
        licenses: vec![LicenseChoice {
            expression: license,
        }],
        hashes,
    })
}

fn package_reference(package: &CargoPackage) -> String {
    let source = package.source.as_deref().unwrap_or("workspace");
    let source_digest = sha256_bytes(source.as_bytes());
    format!(
        "cargo:{}@{}#{}",
        package.name,
        package.version,
        &source_digest[..16]
    )
}

fn validate_exact_input_root(path: &Path) -> Result<PathBuf, ReleaseArtifactError> {
    let root = validate_absolute_directory(path, ReleaseArtifactError::InvalidInputRoot)?;
    let inventory = directory_inventory(&root, ReleaseArtifactError::InvalidInputRoot)?;
    if inventory == INPUT_NAMES.into_iter().map(str::to_owned).collect() {
        Ok(root)
    } else {
        Err(ReleaseArtifactError::InvalidInputRoot)
    }
}

fn validate_output_parent(
    output: &Path,
    service_root: &Path,
    input_root: &Path,
) -> Result<(PathBuf, PathBuf), ReleaseArtifactError> {
    if !output.is_absolute()
        || output
            .file_name()
            .and_then(OsStr::to_str)
            .is_none_or(|name| !valid_output_component(name))
    {
        return Err(ReleaseArtifactError::InvalidOutputRoot);
    }
    let parent = output
        .parent()
        .ok_or(ReleaseArtifactError::InvalidOutputRoot)?;
    let parent = validate_absolute_directory(parent, ReleaseArtifactError::InvalidOutputRoot)?;
    let output = parent.join(
        output
            .file_name()
            .ok_or(ReleaseArtifactError::InvalidOutputRoot)?,
    );
    if output.starts_with(service_root)
        || output.starts_with(input_root)
        || service_root.starts_with(&output)
        || input_root.starts_with(&output)
    {
        return Err(ReleaseArtifactError::InvalidOutputRoot);
    }
    if let Ok(metadata) = fs::symlink_metadata(&output)
        && (metadata.file_type().is_symlink() || !metadata.is_dir())
    {
        return Err(ReleaseArtifactError::InvalidOutputRoot);
    }
    Ok((parent, output))
}

fn validate_git_root(path: &Path) -> Result<PathBuf, ReleaseArtifactError> {
    let root = validate_absolute_directory(path, ReleaseArtifactError::InvalidServiceRoot)?;
    let top = git_stdout(
        &root,
        ["rev-parse", "--show-toplevel"],
        MAX_GIT_OUTPUT_BYTES,
    )
    .map_err(|_| ReleaseArtifactError::InvalidServiceRoot)?;
    let top = std::str::from_utf8(&top)
        .map_err(|_| ReleaseArtifactError::InvalidServiceRoot)?
        .trim();
    let top = fs::canonicalize(top).map_err(|_| ReleaseArtifactError::InvalidServiceRoot)?;
    if top == root {
        Ok(root)
    } else {
        Err(ReleaseArtifactError::InvalidServiceRoot)
    }
}

fn validate_absolute_directory(
    path: &Path,
    error: ReleaseArtifactError,
) -> Result<PathBuf, ReleaseArtifactError> {
    if !path.is_absolute() {
        return Err(error);
    }
    let metadata = fs::symlink_metadata(path).map_err(|_| error)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(error);
    }
    fs::canonicalize(path).map_err(|_| error)
}

fn validate_clean_git(root: &Path) -> Result<(), ReleaseArtifactError> {
    let status = git_stdout(
        root,
        ["status", "--porcelain=v1", "-z", "--untracked-files=all"],
        MAX_GIT_OUTPUT_BYTES,
    )
    .map_err(|_| ReleaseArtifactError::DirtyServiceSource)?;
    if status.is_empty() {
        Ok(())
    } else {
        Err(ReleaseArtifactError::DirtyServiceSource)
    }
}

fn git_head(root: &Path) -> Result<String, ReleaseArtifactError> {
    let bytes = git_stdout(root, ["rev-parse", "HEAD"], 128)
        .map_err(|_| ReleaseArtifactError::InvalidServiceRoot)?;
    let value = std::str::from_utf8(&bytes)
        .map_err(|_| ReleaseArtifactError::InvalidServiceRoot)?
        .trim();
    if valid_lower_hex(value, 40) {
        Ok(value.to_owned())
    } else {
        Err(ReleaseArtifactError::InvalidServiceRoot)
    }
}

fn git_remote(root: &Path) -> Result<String, ReleaseArtifactError> {
    let bytes = git_stdout(root, ["remote", "get-url", "origin"], MAX_GIT_OUTPUT_BYTES)
        .map_err(|_| ReleaseArtifactError::InvalidServiceRoot)?;
    let value = std::str::from_utf8(&bytes)
        .map_err(|_| ReleaseArtifactError::InvalidServiceRoot)?
        .trim();
    if value.len() > MAX_TEXT_FIELD_BYTES
        || !(value.starts_with("https://github.com/") || value.starts_with("ssh://git@github.com/"))
        || value.contains(['\n', '\r'])
    {
        return Err(ReleaseArtifactError::InvalidServiceRoot);
    }
    Ok(value.to_owned())
}

fn verify_bundle(path: &Path, revision: &str) -> Result<(), ReleaseArtifactError> {
    let verification = TempDir::new().map_err(|_| ReleaseArtifactError::InvalidSourceBundle)?;
    git_status(verification.path(), ["init", "--bare", "--quiet"])
        .map_err(|_| ReleaseArtifactError::InvalidSourceBundle)?;
    let output = Command::new("git")
        .args(["bundle", "verify"])
        .arg(path)
        .current_dir(verification.path())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|_| ReleaseArtifactError::InvalidSourceBundle)?;
    if !output.success() {
        return Err(ReleaseArtifactError::InvalidSourceBundle);
    }
    let mut command = Command::new("git");
    command.args(["bundle", "list-heads"]).arg(path);
    let heads = command_stdout(&mut command, MAX_GIT_OUTPUT_BYTES)
        .map_err(|_| ReleaseArtifactError::InvalidSourceBundle)?;
    if heads == format!("{revision} refs/heads/archive\n").as_bytes() {
        Ok(())
    } else {
        Err(ReleaseArtifactError::InvalidSourceBundle)
    }
}

fn create_binary_archive(
    source: &Path,
    output: &Path,
    binary_name: &str,
    source_date_epoch: u32,
) -> Result<FileEvidence, ReleaseArtifactError> {
    let source_metadata = validate_regular_input(source, MAX_BINARY_BYTES)?;
    hash_regular(source, MAX_BINARY_BYTES)?;
    let mut source_file =
        fs::File::open(source).map_err(|_| ReleaseArtifactError::InvalidInputArtifact)?;
    let output_file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(output)
        .map_err(|_| ReleaseArtifactError::GenerationFailure)?;
    let encoder = GzBuilder::new()
        .mtime(source_date_epoch)
        .operating_system(255)
        .write(output_file, Compression::best());
    let mut archive = TarBuilder::new(encoder);
    let mut header = TarHeader::new_gnu();
    header.set_size(source_metadata.len());
    header.set_mode(0o755);
    header.set_uid(0);
    header.set_gid(0);
    header.set_mtime(u64::from(source_date_epoch));
    header.set_cksum();
    archive
        .append_data(&mut header, format!("bin/{binary_name}"), &mut source_file)
        .map_err(|_| ReleaseArtifactError::GenerationFailure)?;
    let encoder = archive
        .into_inner()
        .map_err(|_| ReleaseArtifactError::GenerationFailure)?;
    let output_file = encoder
        .finish()
        .map_err(|_| ReleaseArtifactError::GenerationFailure)?;
    output_file
        .sync_all()
        .map_err(|_| ReleaseArtifactError::GenerationFailure)?;
    set_file_mode(output)?;
    validate_unchanged_input(source, &source_metadata)?;
    hash_regular(output, MAX_BINARY_BYTES + MAX_TEXT_INPUT_BYTES)
}

fn copy_bounded(
    source: &Path,
    output: &Path,
    maximum: u64,
) -> Result<FileEvidence, ReleaseArtifactError> {
    let source_metadata = validate_regular_input(source, maximum)?;
    let mut input =
        fs::File::open(source).map_err(|_| ReleaseArtifactError::InvalidInputArtifact)?;
    let mut output_file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(output)
        .map_err(|_| ReleaseArtifactError::GenerationFailure)?;
    let mut hasher = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = [0_u8; 65_536];
    loop {
        let read = input
            .read(&mut buffer)
            .map_err(|_| ReleaseArtifactError::InvalidInputArtifact)?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(read as u64)
            .ok_or(ReleaseArtifactError::InvalidInputArtifact)?;
        if total > maximum {
            return Err(ReleaseArtifactError::InvalidInputArtifact);
        }
        hasher.update(&buffer[..read]);
        output_file
            .write_all(&buffer[..read])
            .map_err(|_| ReleaseArtifactError::GenerationFailure)?;
    }
    if total != source_metadata.len() {
        return Err(ReleaseArtifactError::InvalidInputArtifact);
    }
    output_file
        .sync_all()
        .map_err(|_| ReleaseArtifactError::GenerationFailure)?;
    set_file_mode(output)?;
    validate_unchanged_input(source, &source_metadata)?;
    Ok(FileEvidence {
        byte_length: total,
        sha256: hex::encode(hasher.finalize()),
    })
}

#[derive(Default)]
struct SecretScanner {
    tail: Vec<u8>,
}

impl SecretScanner {
    fn scan(&mut self, bytes: &[u8]) -> Result<(), ReleaseArtifactError> {
        let mut combined = Vec::with_capacity(self.tail.len() + bytes.len());
        combined.extend_from_slice(&self.tail);
        combined.extend_from_slice(bytes);
        if SECRET_PATTERNS
            .iter()
            .any(|pattern| contains_bytes(&combined, pattern))
        {
            return Err(ReleaseArtifactError::ProtectedMaterialDetected);
        }
        let retained = SECRET_PATTERNS
            .iter()
            .map(|pattern| pattern.len().saturating_sub(1))
            .max()
            .unwrap_or(0)
            .min(combined.len());
        self.tail.clear();
        self.tail
            .extend_from_slice(&combined[combined.len() - retained..]);
        Ok(())
    }
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

fn scan_bytes(bytes: &[u8]) -> Result<(), ReleaseArtifactError> {
    let mut scanner = SecretScanner::default();
    scanner.scan(bytes)
}

fn validate_text_artifact(path: &Path, name: &str) -> Result<(), ReleaseArtifactError> {
    let bytes = read_bounded_regular(
        path,
        MAX_TEXT_INPUT_BYTES,
        ReleaseArtifactError::InvalidInputArtifact,
    )?;
    std::str::from_utf8(&bytes).map_err(|_| ReleaseArtifactError::InvalidInputArtifact)?;
    if bytes.contains(&0) {
        return Err(ReleaseArtifactError::InvalidInputArtifact);
    }
    scan_bytes(&bytes)?;
    if name == "config.schema.json" {
        serde_json::from_slice::<serde_json::Value>(&bytes)
            .map_err(|_| ReleaseArtifactError::InvalidInputArtifact)?;
    } else if name == "config.example.toml" {
        toml::from_str::<toml::Value>(
            std::str::from_utf8(&bytes).map_err(|_| ReleaseArtifactError::InvalidInputArtifact)?,
        )
        .map_err(|_| ReleaseArtifactError::InvalidInputArtifact)?;
    }
    Ok(())
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), ReleaseArtifactError> {
    let mut bytes =
        serde_json::to_vec(value).map_err(|_| ReleaseArtifactError::GenerationFailure)?;
    bytes.push(b'\n');
    write_generated(path, &bytes)
}

fn write_generated(path: &Path, bytes: &[u8]) -> Result<(), ReleaseArtifactError> {
    if bytes.is_empty() || bytes.len() as u64 > MAX_GENERATED_DOCUMENT_BYTES {
        return Err(ReleaseArtifactError::GenerationFailure);
    }
    scan_bytes(bytes)?;
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|_| ReleaseArtifactError::GenerationFailure)?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| ReleaseArtifactError::GenerationFailure)?;
    set_file_mode(path)
}

fn write_checksums(root: &Path) -> Result<(), ReleaseArtifactError> {
    let mut records = inventory_records(root)?;
    records.sort();
    let mut checksums = String::new();
    use fmt::Write as _;
    for record in records {
        writeln!(checksums, "{}  {}", record.sha256, record.path)
            .map_err(|_| ReleaseArtifactError::GenerationFailure)?;
    }
    write_generated(&root.join("SHA256SUMS"), checksums.as_bytes())
}

fn inventory_records(root: &Path) -> Result<Vec<ArtifactRecord>, ReleaseArtifactError> {
    let mut names = directory_inventory(root, ReleaseArtifactError::GenerationFailure)?
        .into_iter()
        .collect::<Vec<_>>();
    names.sort();
    names
        .into_iter()
        .map(|name| {
            let maximum = output_maximum(&name)?;
            let evidence = hash_regular(&root.join(&name), maximum)?;
            Ok(artifact_record(&name, &evidence))
        })
        .collect()
}

fn artifact_record(path: &str, evidence: &FileEvidence) -> ArtifactRecord {
    ArtifactRecord {
        path: path.to_owned(),
        byte_length: evidence.byte_length,
        sha256: evidence.sha256.clone(),
    }
}

fn output_maximum(name: &str) -> Result<u64, ReleaseArtifactError> {
    match name {
        "binary.tar.gz" => Ok(MAX_BINARY_BYTES + MAX_TEXT_INPUT_BYTES),
        "oci-image.tar.gz" => Ok(MAX_OCI_BYTES),
        "service-source.bundle" | "lib-source.bundle" => Ok(MAX_SOURCE_BUNDLE_BYTES),
        "LICENSE-APACHE"
        | "LICENSE-MIT"
        | "config.example.toml"
        | "config.schema.json"
        | "nixos-module.nix"
        | "radroots.service.source-lock.v1.toml"
        | "systemd.service" => Ok(MAX_TEXT_INPUT_BYTES),
        "SHA256SUMS"
        | "THIRD-PARTY-NOTICES.txt"
        | "artifact-manifest.v1.json"
        | "oci-image.v1.json"
        | "provenance-input.v1.json"
        | "sbom.cdx.json"
        | "source-bundles.v1.json" => Ok(MAX_GENERATED_DOCUMENT_BYTES),
        _ => Err(ReleaseArtifactError::GenerationFailure),
    }
}

fn hash_regular(path: &Path, maximum: u64) -> Result<FileEvidence, ReleaseArtifactError> {
    let metadata =
        fs::symlink_metadata(path).map_err(|_| ReleaseArtifactError::InvalidInputArtifact)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > maximum {
        return Err(ReleaseArtifactError::InvalidInputArtifact);
    }
    let mut file = fs::File::open(path).map_err(|_| ReleaseArtifactError::InvalidInputArtifact)?;
    let mut hasher = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = [0_u8; 65_536];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|_| ReleaseArtifactError::InvalidInputArtifact)?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(read as u64)
            .ok_or(ReleaseArtifactError::InvalidInputArtifact)?;
        if total > maximum {
            return Err(ReleaseArtifactError::InvalidInputArtifact);
        }
        hasher.update(&buffer[..read]);
    }
    if total != metadata.len() {
        return Err(ReleaseArtifactError::InvalidInputArtifact);
    }
    Ok(FileEvidence {
        byte_length: total,
        sha256: hex::encode(hasher.finalize()),
    })
}

fn read_bounded_regular(
    path: &Path,
    maximum: u64,
    error: ReleaseArtifactError,
) -> Result<Vec<u8>, ReleaseArtifactError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| error)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > maximum {
        return Err(error);
    }
    let capacity = usize::try_from(metadata.len()).map_err(|_| error)?;
    let mut bytes = Vec::with_capacity(capacity);
    fs::File::open(path)
        .map_err(|_| error)?
        .take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| error)?;
    if bytes.len() as u64 > maximum {
        Err(error)
    } else {
        Ok(bytes)
    }
}

fn validate_regular_input(path: &Path, maximum: u64) -> Result<fs::Metadata, ReleaseArtifactError> {
    let metadata =
        fs::symlink_metadata(path).map_err(|_| ReleaseArtifactError::InvalidInputArtifact)?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() == 0
        || metadata.len() > maximum
    {
        Err(ReleaseArtifactError::InvalidInputArtifact)
    } else {
        Ok(metadata)
    }
}

#[cfg(unix)]
fn validate_unchanged_input(
    path: &Path,
    expected: &fs::Metadata,
) -> Result<(), ReleaseArtifactError> {
    use std::os::unix::fs::MetadataExt as _;
    let current =
        fs::symlink_metadata(path).map_err(|_| ReleaseArtifactError::InvalidInputArtifact)?;
    if current.file_type().is_symlink()
        || !current.is_file()
        || current.dev() != expected.dev()
        || current.ino() != expected.ino()
        || current.len() != expected.len()
    {
        Err(ReleaseArtifactError::InvalidInputArtifact)
    } else {
        Ok(())
    }
}

#[cfg(not(unix))]
fn validate_unchanged_input(
    path: &Path,
    expected: &fs::Metadata,
) -> Result<(), ReleaseArtifactError> {
    let current =
        fs::symlink_metadata(path).map_err(|_| ReleaseArtifactError::InvalidInputArtifact)?;
    if current.file_type().is_symlink() || !current.is_file() || current.len() != expected.len() {
        Err(ReleaseArtifactError::InvalidInputArtifact)
    } else {
        Ok(())
    }
}

fn directory_inventory(
    root: &Path,
    error: ReleaseArtifactError,
) -> Result<BTreeSet<String>, ReleaseArtifactError> {
    let mut names = BTreeSet::new();
    for entry in fs::read_dir(root).map_err(|_| error)? {
        let entry = entry.map_err(|_| error)?;
        let file_type = entry.file_type().map_err(|_| error)?;
        if file_type.is_symlink() || !file_type.is_file() {
            return Err(error);
        }
        let name = entry.file_name().into_string().map_err(|_| error)?;
        if !names.insert(name) || names.len() > OUTPUT_NAMES.len() {
            return Err(error);
        }
    }
    Ok(names)
}

fn validate_exact_output_inventory(root: &Path) -> Result<(), ReleaseArtifactError> {
    let expected = OUTPUT_NAMES.into_iter().map(str::to_owned).collect();
    if directory_inventory(root, ReleaseArtifactError::GenerationFailure)? != expected {
        return Err(ReleaseArtifactError::GenerationFailure);
    }
    validate_directory_mode(root)?;
    for name in OUTPUT_NAMES {
        validate_file_mode(&root.join(name))?;
    }
    Ok(())
}

fn compare_output(expected: &Path, actual: &Path) -> Result<(), ReleaseArtifactError> {
    let actual = validate_absolute_directory(actual, ReleaseArtifactError::StaleOutput)?;
    if directory_inventory(&actual, ReleaseArtifactError::StaleOutput)?
        != OUTPUT_NAMES.into_iter().map(str::to_owned).collect()
    {
        return Err(ReleaseArtifactError::StaleOutput);
    }
    validate_directory_mode(&actual).map_err(|_| ReleaseArtifactError::StaleOutput)?;
    for name in OUTPUT_NAMES {
        validate_file_mode(&actual.join(name)).map_err(|_| ReleaseArtifactError::StaleOutput)?;
        let maximum = output_maximum(name)?;
        let left = hash_regular(&expected.join(name), maximum)
            .map_err(|_| ReleaseArtifactError::StaleOutput)?;
        let right = hash_regular(&actual.join(name), maximum)
            .map_err(|_| ReleaseArtifactError::StaleOutput)?;
        if left.byte_length != right.byte_length || left.sha256 != right.sha256 {
            return Err(ReleaseArtifactError::StaleOutput);
        }
    }
    Ok(())
}

fn validate_output_records(
    actual: &Path,
    expected: &[ArtifactRecord],
) -> Result<(), ReleaseArtifactError> {
    let actual = validate_absolute_directory(actual, ReleaseArtifactError::StaleOutput)?;
    validate_exact_output_inventory(&actual).map_err(|_| ReleaseArtifactError::StaleOutput)?;
    let actual_records =
        inventory_records(&actual).map_err(|_| ReleaseArtifactError::StaleOutput)?;
    if actual_records == expected {
        Ok(())
    } else {
        Err(ReleaseArtifactError::StaleOutput)
    }
}

#[cfg(unix)]
fn set_file_mode(path: &Path) -> Result<(), ReleaseArtifactError> {
    use std::os::unix::fs::PermissionsExt as _;
    fs::set_permissions(path, fs::Permissions::from_mode(FILE_MODE))
        .map_err(|_| ReleaseArtifactError::GenerationFailure)
}

#[cfg(not(unix))]
fn set_file_mode(_path: &Path) -> Result<(), ReleaseArtifactError> {
    Ok(())
}

#[cfg(unix)]
fn set_directory_mode(path: &Path) -> Result<(), ReleaseArtifactError> {
    use std::os::unix::fs::PermissionsExt as _;
    fs::set_permissions(path, fs::Permissions::from_mode(DIRECTORY_MODE))
        .map_err(|_| ReleaseArtifactError::GenerationFailure)
}

#[cfg(not(unix))]
fn set_directory_mode(_path: &Path) -> Result<(), ReleaseArtifactError> {
    Ok(())
}

#[cfg(unix)]
fn validate_file_mode(path: &Path) -> Result<(), ReleaseArtifactError> {
    use std::os::unix::fs::PermissionsExt as _;
    let mode = fs::symlink_metadata(path)
        .map_err(|_| ReleaseArtifactError::StaleOutput)?
        .permissions()
        .mode()
        & 0o777;
    if mode == FILE_MODE {
        Ok(())
    } else {
        Err(ReleaseArtifactError::StaleOutput)
    }
}

#[cfg(not(unix))]
fn validate_file_mode(_path: &Path) -> Result<(), ReleaseArtifactError> {
    Ok(())
}

#[cfg(unix)]
fn validate_directory_mode(path: &Path) -> Result<(), ReleaseArtifactError> {
    use std::os::unix::fs::PermissionsExt as _;
    let mode = fs::symlink_metadata(path)
        .map_err(|_| ReleaseArtifactError::StaleOutput)?
        .permissions()
        .mode()
        & 0o777;
    if mode == DIRECTORY_MODE {
        Ok(())
    } else {
        Err(ReleaseArtifactError::StaleOutput)
    }
}

#[cfg(not(unix))]
fn validate_directory_mode(_path: &Path) -> Result<(), ReleaseArtifactError> {
    Ok(())
}

fn sync_directory(path: &Path) -> Result<(), ReleaseArtifactError> {
    fs::File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| ReleaseArtifactError::GenerationFailure)
}

fn command_stdout(command: &mut Command, maximum: usize) -> Result<Vec<u8>, ()> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| ())?;
    let mut stdout = child.stdout.take().ok_or(())?;
    let mut bytes = Vec::new();
    if stdout
        .by_ref()
        .take(maximum as u64 + 1)
        .read_to_end(&mut bytes)
        .is_err()
        || bytes.len() > maximum
    {
        let _ = child.kill();
        let _ = child.wait();
        return Err(());
    }
    let status = child.wait().map_err(|_| ())?;
    if status.success() { Ok(bytes) } else { Err(()) }
}

fn git_stdout<const N: usize>(root: &Path, args: [&str; N], maximum: usize) -> Result<Vec<u8>, ()> {
    let mut command = Command::new("git");
    command.args(args).current_dir(root);
    command_stdout(&mut command, maximum)
}

fn git_status<const N: usize>(root: &Path, args: [&str; N]) -> Result<(), ()> {
    let status = Command::new("git")
        .args(args)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|_| ())?;
    if status.success() { Ok(()) } else { Err(()) }
}

fn valid_metadata_text(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_TEXT_FIELD_BYTES
        && !value.contains(['\n', '\r'])
        && scan_bytes(value.as_bytes()).is_ok()
}

fn valid_public_source(value: &str) -> bool {
    valid_metadata_text(value)
        && (value.starts_with("registry+https://") || value.starts_with("git+https://"))
        && !value.contains("@github.com")
}

fn valid_snake_identifier(value: &str) -> bool {
    value.len() <= 128
        && value
            .split('_')
            .all(|segment| !segment.is_empty() && valid_lower_alphanumeric(segment))
        && value.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        && value
            .as_bytes()
            .last()
            .is_some_and(u8::is_ascii_alphanumeric)
}

fn valid_kebab_identifier(value: &str) -> bool {
    value.len() <= 128
        && value
            .split('-')
            .all(|segment| !segment.is_empty() && valid_lower_alphanumeric(segment))
        && value.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        && value
            .as_bytes()
            .last()
            .is_some_and(u8::is_ascii_alphanumeric)
}

fn valid_lower_alphanumeric(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
}

fn valid_output_component(value: &str) -> bool {
    let mut components = Path::new(value).components();
    !value.is_empty()
        && value.len() <= 128
        && !value.contains(['/', '\\'])
        && matches!(components.next(), Some(Component::Normal(_)))
        && components.next().is_none()
        && value != "."
        && value != ".."
}

fn valid_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

pub(crate) fn validate_contract(workspace_root: &Path) -> Result<(), String> {
    validate_contract_inner(workspace_root).map_err(|error| error.to_string())
}

fn validate_contract_inner(workspace_root: &Path) -> Result<(), ReleaseArtifactError> {
    let bytes = read_bounded_regular(
        &workspace_root.join(CONTRACT_RELATIVE),
        MAX_CONTRACT_BYTES as u64,
        ReleaseArtifactError::InvalidContract,
    )?;
    let decision = serde_json::from_slice::<ReleaseDecision>(&bytes)
        .map_err(|_| ReleaseArtifactError::InvalidContract)?;
    validate_decision(&decision)
}

fn validate_decision(decision: &ReleaseDecision) -> Result<(), ReleaseArtifactError> {
    let errors = [
        ReleaseArtifactError::InvalidContract,
        ReleaseArtifactError::InvalidServiceRoot,
        ReleaseArtifactError::DirtyServiceSource,
        ReleaseArtifactError::InvalidServiceMetadata,
        ReleaseArtifactError::InvalidInputRoot,
        ReleaseArtifactError::InvalidInputArtifact,
        ReleaseArtifactError::InvalidSourceLock,
        ReleaseArtifactError::InvalidSourceBundle,
        ReleaseArtifactError::InvalidPackageInventory,
        ReleaseArtifactError::ProtectedMaterialDetected,
        ReleaseArtifactError::InvalidOutputRoot,
        ReleaseArtifactError::StaleOutput,
        ReleaseArtifactError::GenerationFailure,
    ];
    if decision.schema != "radroots.services-hardening.release-artifacts-decisions.v1"
        || decision.contract_version != 1
        || decision.decision_state != "active"
        || decision.command != "cargo xtask service-release-artifacts"
        || decision.modes != ["check", "write"]
        || decision.required_arguments
            != [
                "mode",
                "service_root",
                "input_root",
                "output_root",
                "target",
                "source_date_epoch",
            ]
        || decision.service_metadata_path
            != "Cargo.toml.workspace.metadata.radroots.service_release"
        || decision.service_metadata_fields
            != ["service", "service_package", "binary_name", "version"]
        || decision.supported_targets != SUPPORTED_TARGETS
        || decision.input_inventory != INPUT_NAMES
        || decision.excluded_parent_owned_inputs != ["backup_restore_runbook", "operator_runbook"]
        || decision.service_root_inventory != ["LICENSE-APACHE", "LICENSE-MIT", LOCK_FILENAME]
        || decision.output_inventory != OUTPUT_NAMES
        || decision.canonical_json != "compact_utf8_json_with_one_final_lf"
        || decision.checksum_format != "sha256_lower_hex_two_spaces_path_lf_sorted_by_path"
        || decision.sbom_format != "cyclonedx_json_1_5_locked_cargo_graph"
        || decision.provenance_posture
            != "deterministic_unsigned_slsa_v1_signing_input_external_keys_only"
        || decision.protected_material_scan_scope
            != "all_textual_release_inputs_and_generated_documents"
        || decision.source_cleanliness != "no_tracked_staged_or_untracked_changes"
        || decision.revision_stability != "same_service_head_before_and_after_generation"
        || !decision.no_protected_material
        || decision.maximums.text_input_bytes != MAX_TEXT_INPUT_BYTES
        || decision.maximums.generated_document_bytes != MAX_GENERATED_DOCUMENT_BYTES
        || decision.maximums.service_cargo_lock_bytes != MAX_SERVICE_CARGO_LOCK_BYTES
        || decision.maximums.service_flake_lock_bytes != MAX_SERVICE_FLAKE_LOCK_BYTES
        || decision.maximums.binary_bytes != MAX_BINARY_BYTES
        || decision.maximums.source_bundle_bytes != MAX_SOURCE_BUNDLE_BYTES
        || decision.maximums.oci_bytes != MAX_OCI_BYTES
        || decision.maximums.cargo_metadata_bytes != MAX_METADATA_BYTES
        || decision.maximums.packages != MAX_PACKAGES
        || decision.maximums.workspace_packages != MAX_WORKSPACE_PACKAGES
        || decision.negative_error_codes != errors.map(ReleaseArtifactError::code)
    {
        return Err(ReleaseArtifactError::InvalidContract);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use crate::service_source_lock::{
        ContractVersions, ServiceSourceLockParts, ServiceSourceLockV1,
    };

    use super::*;

    struct ReleaseFixture {
        _root: TempDir,
        service: PathBuf,
        input: PathBuf,
        output_a: PathBuf,
        output_b: PathBuf,
    }

    impl ReleaseFixture {
        fn new() -> Self {
            let root = TempDir::new().expect("fixture root");
            let service = root.path().join("service");
            let lib = root.path().join("lib");
            let input = root.path().join("input");
            fs::create_dir_all(service.join("src")).expect("service source");
            fs::create_dir_all(lib.join("contracts/crates")).expect("Lib contracts");
            fs::create_dir(&input).expect("input root");

            write_file(
                &lib.join("contracts/crates/catalog.v2.toml"),
                b"schema_version = 2\n",
            );
            write_file(&lib.join("README.md"), b"fixture Lib source\n");
            initialize_git(&lib, "https://github.com/radrootslabs/lib");
            let lib_revision = git_output(&lib, &["rev-parse", "HEAD"]);
            create_bundle(&lib, &input.join("lib-source.bundle"));
            let lib_bundle = fs::read(input.join("lib-source.bundle")).expect("Lib bundle");
            let catalog =
                fs::read(lib.join("contracts/crates/catalog.v2.toml")).expect("workspace catalog");

            write_file(
                &service.join("Cargo.toml"),
                br#"[package]
name = "fixture-service"
version = "0.1.0-alpha"
edition = "2024"

[[bin]]
name = "fixture-service"
path = "src/main.rs"

[workspace]
resolver = "3"

[workspace.metadata.radroots.service_release]
service = "fixture_service"
service_package = "fixture-service"
binary_name = "fixture-service"
version = "0.1.0-alpha"
"#,
            );
            write_file(&service.join("src/main.rs"), b"fn main() {}\n");
            write_file(
                &service.join("Cargo.lock"),
                br#"# This file is automatically @generated by Cargo.
# It is not intended for manual editing.
version = 4

[[package]]
name = "fixture-service"
version = "0.1.0-alpha"
"#,
            );
            write_file(
                &service.join("flake.lock"),
                b"{\"nodes\":{},\"root\":\"root\",\"version\":7}\n",
            );
            write_file(
                &service.join("LICENSE-APACHE"),
                b"Apache-2.0 fixture license\n",
            );
            write_file(&service.join("LICENSE-MIT"), b"MIT fixture license\n");
            let cargo_lock = fs::read(service.join("Cargo.lock")).expect("Cargo lock");
            let flake_lock = fs::read(service.join("flake.lock")).expect("flake lock");
            let source_lock = ServiceSourceLockV1::new(ServiceSourceLockParts {
                service: "fixture_service",
                revision: &lib_revision,
                workspace_catalog_sha256: &sha256_bytes(&catalog),
                source_archive_sha256: &sha256_bytes(&lib_bundle),
                cargo_lock_sha256: &sha256_bytes(&cargo_lock),
                flake_lock_sha256: &sha256_bytes(&flake_lock),
                contract_versions: ContractVersions::new(1, 1, 1, 1, 1),
            })
            .expect("source lock");
            write_file(&service.join(LOCK_FILENAME), source_lock.canonical_bytes());
            initialize_git(&service, "https://github.com/radrootslabs/fixture-service");
            create_bundle(&service, &input.join("service-source.bundle"));

            for (name, bytes) in [
                ("config.example.toml", b"enabled = true\n".as_slice()),
                ("config.schema.json", b"{\"type\":\"object\"}\n".as_slice()),
                ("nixos-module.nix", b"{ ... }: {}\n".as_slice()),
                (
                    "systemd.service",
                    b"[Service]\nExecStart=/usr/bin/fixture-service\n".as_slice(),
                ),
            ] {
                write_file(&input.join(name), bytes);
            }
            write_file(
                &input.join("service-binary"),
                b"fixture service executable\0\xff",
            );
            write_file(
                &input.join("oci-image.tar.gz"),
                b"fixture OCI archive\0\xff",
            );

            Self {
                output_a: root.path().join("release-a"),
                output_b: root.path().join("release-b"),
                _root: root,
                service,
                input,
            }
        }

        fn write(&self, output: &Path) -> Result<(), ReleaseArtifactError> {
            run_inner(
                CommandMode::Write,
                &self.service,
                &self.input,
                output,
                "x86_64-unknown-linux-gnu",
                1_700_000_000,
            )
        }

        fn check(&self, output: &Path) -> Result<(), ReleaseArtifactError> {
            run_inner(
                CommandMode::Check,
                &self.service,
                &self.input,
                output,
                "x86_64-unknown-linux-gnu",
                1_700_000_000,
            )
        }
    }

    fn write_file(path: &Path, bytes: &[u8]) {
        fs::write(path, bytes).expect("write fixture file");
    }

    fn git(root: &Path, args: &[&str]) {
        let status = Command::new("git")
            .args(args)
            .current_dir(root)
            .status()
            .expect("run Git");
        assert!(status.success(), "git {args:?}");
    }

    fn git_output(root: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .expect("run Git");
        assert!(output.status.success(), "git {args:?}");
        String::from_utf8(output.stdout)
            .expect("Git UTF-8")
            .trim()
            .to_owned()
    }

    fn initialize_git(root: &Path, remote: &str) {
        git(root, &["init", "--quiet"]);
        git(root, &["config", "user.email", "fixture@radroots.test"]);
        git(root, &["config", "user.name", "Radroots Fixture"]);
        git(root, &["remote", "add", "origin", remote]);
        git(root, &["add", "."]);
        git(root, &["commit", "--quiet", "-m", "fixture"]);
        git(root, &["branch", "-M", "archive"]);
    }

    fn create_bundle(root: &Path, output: &Path) {
        let status = Command::new("git")
            .args(["bundle", "create"])
            .arg(output)
            .arg("refs/heads/archive")
            .current_dir(root)
            .status()
            .expect("create source bundle");
        assert!(status.success());
    }

    fn sample_metadata() -> ReleaseMetadata {
        ReleaseMetadata {
            service: "fixture_service".to_owned(),
            service_package: "fixture-service".to_owned(),
            binary_name: "fixture-service".to_owned(),
            version: "0.1.0-alpha".to_owned(),
        }
    }

    fn package(
        id: &str,
        name: &str,
        source: Option<&str>,
        checksum: Option<&str>,
        license: Option<&str>,
        binary: bool,
    ) -> CargoPackage {
        CargoPackage {
            id: id.to_owned(),
            name: name.to_owned(),
            version: "0.1.0-alpha".to_owned(),
            source: source.map(str::to_owned),
            checksum: checksum.map(str::to_owned),
            license: license.map(str::to_owned),
            targets: if binary {
                vec![CargoTarget {
                    name: name.to_owned(),
                    kind: vec!["bin".to_owned()],
                }]
            } else {
                Vec::new()
            },
        }
    }

    fn sample_cargo_metadata() -> CargoMetadata {
        let root_id = "path+file:///fixture#fixture-service@0.1.0-alpha";
        let dependency_id = "registry+https://example.invalid#index@0.1.0-alpha";
        CargoMetadata {
            packages: vec![
                package(root_id, "fixture-service", None, None, Some("MIT"), true),
                package(
                    dependency_id,
                    "dependency",
                    Some("registry+https://github.com/rust-lang/crates.io-index"),
                    Some(&"a".repeat(64)),
                    Some("Apache-2.0"),
                    false,
                ),
            ],
            workspace_members: vec![root_id.to_owned()],
            resolve: Some(CargoResolve {
                nodes: vec![
                    CargoNode {
                        id: dependency_id.to_owned(),
                        dependencies: Vec::new(),
                    },
                    CargoNode {
                        id: root_id.to_owned(),
                        dependencies: vec![dependency_id.to_owned()],
                    },
                ],
            }),
        }
    }

    #[test]
    fn contract_matches_the_checked_in_decision() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("workspace root");
        validate_contract_inner(root).expect("release decision");
    }

    #[test]
    fn contract_rejects_every_independent_governed_field_drift() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("workspace root");
        let bytes = fs::read(root.join(CONTRACT_RELATIVE)).expect("decision");
        let canonical = serde_json::from_slice::<serde_json::Value>(&bytes).expect("decision json");
        for (pointer, replacement) in [
            ("/schema", serde_json::json!("other")),
            ("/contract_version", serde_json::json!(2)),
            ("/decision_state", serde_json::json!("draft")),
            ("/command", serde_json::json!("other")),
            ("/modes", serde_json::json!([])),
            ("/required_arguments", serde_json::json!([])),
            ("/service_metadata_path", serde_json::json!("other")),
            ("/service_metadata_fields", serde_json::json!([])),
            ("/supported_targets", serde_json::json!([])),
            ("/input_inventory", serde_json::json!([])),
            ("/excluded_parent_owned_inputs", serde_json::json!([])),
            ("/service_root_inventory", serde_json::json!([])),
            ("/output_inventory", serde_json::json!([])),
            ("/canonical_json", serde_json::json!("other")),
            ("/checksum_format", serde_json::json!("other")),
            ("/sbom_format", serde_json::json!("other")),
            ("/provenance_posture", serde_json::json!("other")),
            ("/protected_material_scan_scope", serde_json::json!("other")),
            ("/source_cleanliness", serde_json::json!("other")),
            ("/revision_stability", serde_json::json!("other")),
            ("/no_protected_material", serde_json::json!(false)),
            ("/maximums/text_input_bytes", serde_json::json!(1)),
            ("/maximums/generated_document_bytes", serde_json::json!(1)),
            ("/maximums/service_cargo_lock_bytes", serde_json::json!(1)),
            ("/maximums/service_flake_lock_bytes", serde_json::json!(1)),
            ("/maximums/binary_bytes", serde_json::json!(1)),
            ("/maximums/source_bundle_bytes", serde_json::json!(1)),
            ("/maximums/oci_bytes", serde_json::json!(1)),
            ("/maximums/cargo_metadata_bytes", serde_json::json!(1)),
            ("/maximums/packages", serde_json::json!(1)),
            ("/maximums/workspace_packages", serde_json::json!(1)),
            ("/negative_error_codes", serde_json::json!([])),
        ] {
            let mut drifted = canonical.clone();
            *drifted.pointer_mut(pointer).expect("governed field") = replacement;
            let decision = serde_json::from_value::<ReleaseDecision>(drifted)
                .expect("structurally valid drift");
            assert_eq!(
                validate_decision(&decision),
                Err(ReleaseArtifactError::InvalidContract),
                "accepted drift at {pointer}"
            );
        }
    }

    #[test]
    fn exact_inventory_and_limits_are_literal() {
        assert_eq!(INPUT_NAMES.len(), 8);
        assert_eq!(OUTPUT_NAMES.len(), 18);
        assert_eq!(MAX_TEXT_INPUT_BYTES, 1_048_576);
        assert_eq!(MAX_GENERATED_DOCUMENT_BYTES, 16_777_216);
        assert_eq!(MAX_SERVICE_CARGO_LOCK_BYTES, 16_777_216);
        assert_eq!(MAX_SERVICE_FLAKE_LOCK_BYTES, 4_194_304);
        assert_eq!(MAX_BINARY_BYTES, 536_870_912);
        assert_eq!(MAX_SOURCE_BUNDLE_BYTES, 1_073_741_824);
        assert_eq!(MAX_OCI_BYTES, 2_147_483_648);
        assert_eq!(MAX_METADATA_BYTES, 33_554_432);
        assert_eq!(MAX_PACKAGES, 8_192);
        assert_eq!(MAX_WORKSPACE_PACKAGES, 64);
    }

    #[test]
    fn supply_chain_documents_are_deterministic_and_complete() {
        let root_id = "path+file:///fixture#fixture-service@0.1.0-alpha";
        let dependency_id = "registry+https://example.invalid#index@0.1.0-alpha";
        let cargo = CargoMetadata {
            packages: vec![
                package(
                    dependency_id,
                    "dependency",
                    Some("registry+https://github.com/rust-lang/crates.io-index"),
                    Some(&"a".repeat(64)),
                    Some("Apache-2.0"),
                    false,
                ),
                package(root_id, "fixture-service", None, None, Some("MIT"), true),
            ],
            workspace_members: vec![root_id.to_owned()],
            resolve: Some(CargoResolve {
                nodes: vec![
                    CargoNode {
                        id: dependency_id.to_owned(),
                        dependencies: Vec::new(),
                    },
                    CargoNode {
                        id: root_id.to_owned(),
                        dependencies: vec![dependency_id.to_owned()],
                    },
                ],
            }),
        };
        let (sbom, notices) =
            build_supply_chain_documents(&sample_metadata(), cargo).expect("documents");
        let bytes = serde_json::to_vec(&sbom).expect("SBOM JSON");
        assert_eq!(serde_json::to_vec(&sbom).expect("SBOM JSON"), bytes);
        assert_eq!(sbom.bom_format, "CycloneDX");
        assert_eq!(sbom.spec_version, "1.5");
        assert_eq!(sbom.metadata.component.name, "fixture-service");
        assert_eq!(sbom.components.len(), 1);
        assert_eq!(sbom.dependencies.len(), 2);
        assert!(notices.contains("Package: dependency 0.1.0-alpha"));
        assert!(notices.contains("License: Apache-2.0"));
        assert!(notices.contains("registry+https://github.com/rust-lang/crates.io-index"));
    }

    #[test]
    fn private_or_incomplete_dependency_evidence_is_rejected() {
        let root_id = "root";
        for dependency in [
            package(
                "dep",
                "dep",
                Some("git+ssh://git@github.com/private/repo"),
                Some(&"a".repeat(64)),
                Some("MIT"),
                false,
            ),
            package(
                "dep",
                "dep",
                Some("registry+https://github.com/rust-lang/crates.io-index"),
                Some(&"a".repeat(64)),
                None,
                false,
            ),
        ] {
            let cargo = CargoMetadata {
                packages: vec![
                    package(root_id, "fixture-service", None, None, Some("MIT"), true),
                    dependency,
                ],
                workspace_members: vec![root_id.to_owned()],
                resolve: Some(CargoResolve {
                    nodes: vec![
                        CargoNode {
                            id: root_id.to_owned(),
                            dependencies: vec!["dep".to_owned()],
                        },
                        CargoNode {
                            id: "dep".to_owned(),
                            dependencies: Vec::new(),
                        },
                    ],
                }),
            };
            assert!(matches!(
                build_supply_chain_documents(&sample_metadata(), cargo),
                Err(ReleaseArtifactError::InvalidPackageInventory)
            ));
        }
    }

    #[test]
    fn package_metadata_rejects_each_independent_field_drift() {
        let invalid = |package: CargoPackage, workspace_member| {
            assert_eq!(
                validate_metadata_package(&package, workspace_member),
                Err(ReleaseArtifactError::InvalidPackageInventory)
            );
        };
        for field in ["name", "version", "id"] {
            for value in [
                String::new(),
                "x".repeat(MAX_TEXT_FIELD_BYTES + 1),
                "x\ny".into(),
            ] {
                let mut candidate =
                    package("root", "fixture-service", None, None, Some("MIT"), true);
                match field {
                    "name" => candidate.name = value,
                    "version" => candidate.version = value,
                    "id" => candidate.id = value,
                    _ => unreachable!(),
                }
                invalid(candidate, true);
            }
        }

        for license in [
            Some(""),
            Some("bad\nlicense"),
            Some("-----BEGIN PRIVATE KEY-----"),
        ] {
            invalid(
                package("root", "fixture-service", None, None, license, true),
                true,
            );
        }
        for source in [
            Some(""),
            Some("path+file:///private"),
            Some("git+ssh://git@github.com/private/repo"),
        ] {
            invalid(
                package(
                    "dep",
                    "dep",
                    source,
                    Some(&"a".repeat(64)),
                    Some("MIT"),
                    false,
                ),
                false,
            );
        }
        invalid(
            package(
                "dep",
                "dep",
                None,
                Some(&"a".repeat(64)),
                Some("MIT"),
                false,
            ),
            false,
        );
        invalid(
            package(
                "dep",
                "dep",
                Some("registry+https://github.com/rust-lang/crates.io-index"),
                Some(&"a".repeat(64)),
                None,
                false,
            ),
            false,
        );
        for checksum in ["a".repeat(63), "g".repeat(64)] {
            invalid(
                package(
                    "root",
                    "fixture-service",
                    None,
                    Some(&checksum),
                    Some("MIT"),
                    true,
                ),
                true,
            );
        }
    }

    #[test]
    fn supply_chain_graph_rejects_each_independent_structural_drift() {
        let invalid = |cargo| {
            assert!(matches!(
                build_supply_chain_documents(&sample_metadata(), cargo),
                Err(ReleaseArtifactError::InvalidPackageInventory)
            ));
        };

        let mut cargo = sample_cargo_metadata();
        cargo.packages.clear();
        invalid(cargo);
        let mut cargo = sample_cargo_metadata();
        cargo.workspace_members.clear();
        invalid(cargo);
        let mut cargo = sample_cargo_metadata();
        cargo.packages[0].version = "0.2.0".into();
        invalid(cargo);
        let mut cargo = sample_cargo_metadata();
        cargo.packages[0].targets.clear();
        invalid(cargo);
        let mut cargo = sample_cargo_metadata();
        cargo.packages[0].name = "other".into();
        invalid(cargo);
        let mut cargo = sample_cargo_metadata();
        cargo.packages.push(package(
            "second-root",
            "fixture-service",
            None,
            None,
            Some("MIT"),
            true,
        ));
        cargo.workspace_members.push("second-root".into());
        invalid(cargo);
        let mut cargo = sample_cargo_metadata();
        cargo.packages[1].id = cargo.packages[0].id.clone();
        invalid(cargo);
        let mut cargo = sample_cargo_metadata();
        cargo.workspace_members.push("missing".into());
        invalid(cargo);
        let mut cargo = sample_cargo_metadata();
        cargo.resolve = None;
        invalid(cargo);
        let mut cargo = sample_cargo_metadata();
        cargo.resolve.as_mut().expect("resolve").nodes.clear();
        invalid(cargo);
        let mut cargo = sample_cargo_metadata();
        let duplicate = cargo.resolve.as_ref().expect("resolve").nodes[0].id.clone();
        cargo.resolve.as_mut().expect("resolve").nodes[1].id = duplicate;
        invalid(cargo);
        let mut cargo = sample_cargo_metadata();
        cargo.resolve.as_mut().expect("resolve").nodes[0].id = "missing".into();
        invalid(cargo);
        let mut cargo = sample_cargo_metadata();
        cargo.resolve.as_mut().expect("resolve").nodes[1]
            .dependencies
            .push("missing".into());
        invalid(cargo);

        let root_id = "path+file:///fixture#fixture-service@0.1.0-alpha";
        let root_only = CargoMetadata {
            packages: vec![package(
                root_id,
                "fixture-service",
                None,
                None,
                Some("MIT"),
                true,
            )],
            workspace_members: vec![root_id.into()],
            resolve: Some(CargoResolve {
                nodes: vec![CargoNode {
                    id: root_id.into(),
                    dependencies: Vec::new(),
                }],
            }),
        };
        let (_, notices) =
            build_supply_chain_documents(&sample_metadata(), root_only).expect("root-only graph");
        assert!(notices.contains("No third-party Cargo packages are present."));
    }

    #[test]
    fn file_admission_predicates_reject_each_independent_drift() {
        let root = TempDir::new().expect("file fixture");
        let regular = root.path().join("regular");
        write_file(&regular, b"bytes");
        let empty = root.path().join("empty");
        write_file(&empty, b"");
        let oversized = root.path().join("oversized");
        fs::File::create(&oversized)
            .and_then(|file| file.set_len(6))
            .expect("sparse file");
        assert!(matches!(
            hash_regular(root.path(), 5),
            Err(ReleaseArtifactError::InvalidInputArtifact)
        ));
        assert!(matches!(
            hash_regular(&oversized, 5),
            Err(ReleaseArtifactError::InvalidInputArtifact)
        ));
        assert!(matches!(
            validate_regular_input(&empty, 5),
            Err(ReleaseArtifactError::InvalidInputArtifact)
        ));
        assert!(matches!(
            validate_regular_input(root.path(), 5),
            Err(ReleaseArtifactError::InvalidInputArtifact)
        ));
        assert!(matches!(
            validate_regular_input(&oversized, 5),
            Err(ReleaseArtifactError::InvalidInputArtifact)
        ));
        assert_eq!(
            validate_absolute_directory(&regular, ReleaseArtifactError::InvalidInputRoot),
            Err(ReleaseArtifactError::InvalidInputRoot)
        );
        assert_eq!(
            validate_absolute_directory(
                Path::new("relative"),
                ReleaseArtifactError::InvalidInputRoot
            ),
            Err(ReleaseArtifactError::InvalidInputRoot)
        );
        assert_eq!(
            output_maximum("unknown"),
            Err(ReleaseArtifactError::GenerationFailure)
        );

        #[cfg(unix)]
        {
            let symlink = root.path().join("symlink");
            std::os::unix::fs::symlink(&regular, &symlink).expect("symlink");
            assert!(matches!(
                hash_regular(&symlink, 5),
                Err(ReleaseArtifactError::InvalidInputArtifact)
            ));
            assert!(matches!(
                validate_regular_input(&symlink, 5),
                Err(ReleaseArtifactError::InvalidInputArtifact)
            ));
            assert_eq!(
                validate_absolute_directory(&symlink, ReleaseArtifactError::InvalidInputRoot),
                Err(ReleaseArtifactError::InvalidInputRoot)
            );
        }
    }

    #[test]
    fn release_metadata_and_text_admission_reject_each_field_drift() {
        let root = TempDir::new().expect("metadata fixture");
        let canonical = r#"[workspace.metadata.radroots.service_release]
service = "fixture_service"
service_package = "fixture-service"
binary_name = "fixture-service"
version = "0.1.0-alpha"
"#;
        for (from, to) in [
            ("fixture_service", "Fixture"),
            (
                "service_package = \"fixture-service\"",
                "service_package = \"fixture--service\"",
            ),
            (
                "binary_name = \"fixture-service\"",
                "binary_name = \"fixture--service\"",
            ),
            ("version = \"0.1.0-alpha\"", "version = \"invalid\""),
        ] {
            write_file(
                &root.path().join("Cargo.toml"),
                canonical.replacen(from, to, 1).as_bytes(),
            );
            assert!(matches!(
                read_release_metadata(root.path()),
                Err(ReleaseArtifactError::InvalidServiceMetadata)
            ));
        }
        write_file(
            &root.path().join("Cargo.toml"),
            canonical
                .replacen("0.1.0-alpha", &"a".repeat(129), 1)
                .as_bytes(),
        );
        assert!(matches!(
            read_release_metadata(root.path()),
            Err(ReleaseArtifactError::InvalidServiceMetadata)
        ));

        for (name, bytes) in [
            ("plain.txt", b"contains\0nul".as_slice()),
            ("config.schema.json", b"not json".as_slice()),
            ("config.example.toml", b"not = [toml".as_slice()),
        ] {
            let path = root.path().join(name);
            write_file(&path, bytes);
            assert_eq!(
                validate_text_artifact(&path, name),
                Err(ReleaseArtifactError::InvalidInputArtifact)
            );
        }
        assert_eq!(
            write_generated(&root.path().join("empty-generated"), b""),
            Err(ReleaseArtifactError::GenerationFailure)
        );
    }

    #[test]
    fn output_scope_remote_and_inventory_reject_each_drift() {
        let root = TempDir::new().expect("output fixture");
        let service = root.path().join("service");
        let input = root.path().join("input");
        let output_parent = root.path().join("output");
        fs::create_dir(&service).expect("service");
        fs::create_dir(&input).expect("input");
        fs::create_dir(&output_parent).expect("output");
        assert_eq!(
            validate_output_parent(Path::new("relative"), &service, &input),
            Err(ReleaseArtifactError::InvalidOutputRoot)
        );
        assert_eq!(
            validate_output_parent(root.path(), &service, &input),
            Err(ReleaseArtifactError::InvalidOutputRoot)
        );
        let file_output = output_parent.join("file");
        write_file(&file_output, b"file");
        assert_eq!(
            validate_output_parent(&file_output, &service, &input),
            Err(ReleaseArtifactError::InvalidOutputRoot)
        );

        let inventory = root.path().join("inventory");
        fs::create_dir(&inventory).expect("inventory");
        fs::create_dir(inventory.join("directory-entry")).expect("directory entry");
        assert_eq!(
            directory_inventory(&inventory, ReleaseArtifactError::InvalidInputRoot),
            Err(ReleaseArtifactError::InvalidInputRoot)
        );
        fs::remove_dir(inventory.join("directory-entry")).expect("remove entry");
        for index in 0..=OUTPUT_NAMES.len() {
            write_file(&inventory.join(format!("file-{index}")), b"x");
        }
        assert_eq!(
            directory_inventory(&inventory, ReleaseArtifactError::InvalidInputRoot),
            Err(ReleaseArtifactError::InvalidInputRoot)
        );

        write_file(&service.join("fixture"), b"fixture");
        initialize_git(&service, "https://example.invalid/service");
        assert_eq!(
            git_remote(&service),
            Err(ReleaseArtifactError::InvalidServiceRoot)
        );
        git(
            &service,
            &[
                "remote",
                "set-url",
                "origin",
                "ssh://git@github.com/user/repo\nbad",
            ],
        );
        assert_eq!(
            git_remote(&service),
            Err(ReleaseArtifactError::InvalidServiceRoot)
        );
        git(
            &service,
            &[
                "remote",
                "set-url",
                "origin",
                &format!("https://github.com/{}", "a".repeat(MAX_TEXT_FIELD_BYTES)),
            ],
        );
        assert_eq!(
            git_remote(&service),
            Err(ReleaseArtifactError::InvalidServiceRoot)
        );
    }

    #[test]
    fn remaining_release_boundaries_fail_closed() {
        let fixture = ReleaseFixture::new();
        assert_eq!(
            fixture.check(&fixture.output_a),
            Err(ReleaseArtifactError::StaleOutput)
        );

        let source_lock = ServiceSourceLockV1::from_canonical_bytes(
            &fs::read(fixture.service.join(LOCK_FILENAME)).expect("source lock"),
        )
        .expect("source lock");
        fs::write(fixture.service.join("flake.lock"), b"different").expect("flake drift");
        assert_eq!(
            validate_source_lock_files(&fixture.service, &source_lock),
            Err(ReleaseArtifactError::InvalidSourceLock)
        );

        let mut packages = sample_cargo_metadata();
        packages.packages = (0..=MAX_PACKAGES)
            .map(|index| {
                package(
                    &format!("id-{index}"),
                    "dep",
                    None,
                    None,
                    Some("MIT"),
                    false,
                )
            })
            .collect();
        assert!(matches!(
            build_supply_chain_documents(&sample_metadata(), packages),
            Err(ReleaseArtifactError::InvalidPackageInventory)
        ));
        let mut workspace = sample_cargo_metadata();
        workspace.workspace_members = (0..=MAX_WORKSPACE_PACKAGES)
            .map(|index| format!("member-{index}"))
            .collect();
        assert!(matches!(
            build_supply_chain_documents(&sample_metadata(), workspace),
            Err(ReleaseArtifactError::InvalidPackageInventory)
        ));
        for (target_name, kind) in [("other", "bin"), ("fixture-service", "lib")] {
            let mut cargo = sample_cargo_metadata();
            cargo.packages[0].targets = vec![CargoTarget {
                name: target_name.into(),
                kind: vec![kind.into()],
            }];
            assert!(matches!(
                build_supply_chain_documents(&sample_metadata(), cargo),
                Err(ReleaseArtifactError::InvalidPackageInventory)
            ));
        }
        let mut outside_workspace = sample_cargo_metadata();
        outside_workspace.workspace_members = vec!["other".into()];
        assert!(matches!(
            build_supply_chain_documents(&sample_metadata(), outside_workspace),
            Err(ReleaseArtifactError::InvalidPackageInventory)
        ));

        let scope = TempDir::new().expect("scope fixture");
        let service = scope.path().join("service");
        let input_parent = TempDir::new().expect("input scope");
        let input = input_parent.path().join("input");
        fs::create_dir(&service).expect("service");
        fs::create_dir(&input).expect("input");
        assert_eq!(
            validate_output_parent(&scope.path().join("bad\\name"), &service, &input),
            Err(ReleaseArtifactError::InvalidOutputRoot)
        );
        assert_eq!(
            validate_output_parent(input_parent.path(), &service, &input),
            Err(ReleaseArtifactError::InvalidOutputRoot)
        );
        #[cfg(unix)]
        {
            let foreign = scope.path().join("foreign");
            fs::create_dir(&foreign).expect("foreign");
            let output = scope.path().join("output-link");
            std::os::unix::fs::symlink(&foreign, &output).expect("output symlink");
            assert_eq!(
                validate_output_parent(&output, &service, &input),
                Err(ReleaseArtifactError::InvalidOutputRoot)
            );
        }

        write_file(&service.join("tracked"), b"tracked");
        initialize_git(&service, "https://github.com/radrootslabs/service");
        let child = service.join("child");
        fs::create_dir(&child).expect("child");
        assert_eq!(
            validate_git_root(&child),
            Err(ReleaseArtifactError::InvalidServiceRoot)
        );
        write_file(&service.join("untracked"), b"dirty");
        assert_eq!(
            validate_clean_git(&service),
            Err(ReleaseArtifactError::DirtyServiceSource)
        );

        assert_eq!(
            verify_bundle(
                &fixture.input.join("service-source.bundle"),
                &"a".repeat(40)
            ),
            Err(ReleaseArtifactError::InvalidSourceBundle)
        );
        assert_eq!(
            write_generated(
                &scope.path().join("oversized-generated"),
                &vec![b'x'; MAX_GENERATED_DOCUMENT_BYTES as usize + 1]
            ),
            Err(ReleaseArtifactError::GenerationFailure)
        );
    }

    #[test]
    fn identifier_predicates_reject_each_independent_boundary() {
        for value in ["", "1service", "service_", "service__name", "service-name"] {
            assert!(!valid_snake_identifier(value), "{value}");
        }
        assert!(!valid_snake_identifier(&"a".repeat(129)));
        for value in ["", "1service", "service-", "service--name", "service_name"] {
            assert!(!valid_kebab_identifier(value), "{value}");
        }
        assert!(!valid_kebab_identifier(&"a".repeat(129)));
        assert!(!valid_metadata_text(""));
        assert!(!valid_metadata_text(&"a".repeat(MAX_TEXT_FIELD_BYTES + 1)));
        assert!(!valid_metadata_text("bad\rvalue"));
        assert!(!valid_metadata_text("-----BEGIN PRIVATE KEY-----"));
        assert!(!valid_public_source("registry+http://example.invalid"));
        assert!(!valid_public_source(
            "registry+https://user@github.com/private"
        ));
        assert!(!valid_lower_hex("a", 2));
        assert!(!valid_lower_hex("ag", 2));
    }

    #[test]
    fn binary_archive_is_reproducible_and_metadata_is_fixed() {
        let root = TempDir::new().expect("archive fixture");
        let source = root.path().join("service");
        let first = root.path().join("first.tar.gz");
        let second = root.path().join("second.tar.gz");
        write_file(&source, b"exact executable bytes");
        create_binary_archive(&source, &first, "fixture-service", 1_700_000_000)
            .expect("first archive");
        create_binary_archive(&source, &second, "fixture-service", 1_700_000_000)
            .expect("second archive");
        assert_eq!(
            fs::read(&first).expect("first"),
            fs::read(&second).expect("second")
        );

        let decoder = flate2::read::GzDecoder::new(fs::File::open(first).expect("archive"));
        let mut archive = tar::Archive::new(decoder);
        let mut entries = archive.entries().expect("entries");
        let mut entry = entries.next().expect("one entry").expect("entry");
        assert_eq!(
            entry.path().expect("path").as_ref(),
            Path::new("bin/fixture-service")
        );
        assert_eq!(entry.header().mode().expect("mode"), 0o755);
        assert_eq!(entry.header().mtime().expect("mtime"), 1_700_000_000);
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).expect("archive bytes");
        assert_eq!(bytes, b"exact executable bytes");
        assert!(entries.next().is_none());
    }

    #[test]
    fn full_artifact_set_is_reproducible_immutable_and_verifiable() {
        let fixture = ReleaseFixture::new();
        fixture.write(&fixture.output_a).expect("first release");
        fixture.write(&fixture.output_b).expect("second release");
        fixture.check(&fixture.output_a).expect("check release");
        assert_eq!(
            directory_inventory(&fixture.output_a, ReleaseArtifactError::StaleOutput)
                .expect("first inventory"),
            OUTPUT_NAMES.into_iter().map(str::to_owned).collect()
        );
        for name in OUTPUT_NAMES {
            assert_eq!(
                fs::read(fixture.output_a.join(name)).expect("first output"),
                fs::read(fixture.output_b.join(name)).expect("second output"),
                "{name}"
            );
        }

        let sums =
            fs::read_to_string(fixture.output_a.join("SHA256SUMS")).expect("checksum inventory");
        let mut prior = "";
        let mut count = 0;
        for line in sums.lines() {
            let (digest, name) = line.split_once("  ").expect("checksum line");
            assert!(prior < name);
            assert!(valid_lower_hex(digest, 64));
            assert_eq!(
                digest,
                hash_regular(
                    &fixture.output_a.join(name),
                    output_maximum(name).expect("known output")
                )
                .expect("artifact hash")
                .sha256
            );
            prior = name;
            count += 1;
        }
        assert_eq!(count, OUTPUT_NAMES.len() - 1);

        for name in [
            "artifact-manifest.v1.json",
            "oci-image.v1.json",
            "provenance-input.v1.json",
            "sbom.cdx.json",
            "source-bundles.v1.json",
        ] {
            let bytes = fs::read(fixture.output_a.join(name)).expect("JSON output");
            assert_eq!(bytes.last(), Some(&b'\n'));
            assert_eq!(bytes.iter().filter(|byte| **byte == b'\n').count(), 1);
            serde_json::from_slice::<serde_json::Value>(&bytes).expect("valid JSON");
        }
        let provenance: serde_json::Value = serde_json::from_slice(
            &fs::read(fixture.output_a.join("provenance-input.v1.json")).expect("provenance"),
        )
        .expect("provenance JSON");
        assert_eq!(provenance["signing_required"], true);

        write_file(
            &fixture.output_a.join("config.example.toml"),
            b"tampered = true\n",
        );
        assert_eq!(
            fixture.check(&fixture.output_a),
            Err(ReleaseArtifactError::StaleOutput)
        );
        assert_eq!(
            fixture.write(&fixture.output_a),
            Err(ReleaseArtifactError::StaleOutput)
        );
    }

    #[test]
    fn protected_text_and_invalid_inventory_fail_closed() {
        let fixture = ReleaseFixture::new();
        write_file(
            &fixture.input.join("config.example.toml"),
            b"-----BEGIN PRIVATE KEY-----\n",
        );
        assert_eq!(
            fixture.write(&fixture.output_a),
            Err(ReleaseArtifactError::ProtectedMaterialDetected)
        );
        write_file(
            &fixture.input.join("config.example.toml"),
            b"enabled = true\n",
        );
        write_file(&fixture.input.join("unexpected"), b"unexpected\n");
        assert_eq!(
            fixture.write(&fixture.output_a),
            Err(ReleaseArtifactError::InvalidInputRoot)
        );
    }

    #[test]
    fn source_lock_and_source_bundle_drift_fail_closed() {
        let fixture = ReleaseFixture::new();
        write_file(
            &fixture.service.join("Cargo.lock"),
            b"version = 4\n# changed after source lock\n",
        );
        git(&fixture.service, &["add", "Cargo.lock"]);
        git(
            &fixture.service,
            &["commit", "--quiet", "-m", "change lock"],
        );
        assert_eq!(
            fixture.write(&fixture.output_a),
            Err(ReleaseArtifactError::InvalidSourceLock)
        );

        let fixture = ReleaseFixture::new();
        let mut bundle = fs::read(fixture.input.join("lib-source.bundle")).expect("bundle");
        bundle[0] ^= 0xff;
        write_file(&fixture.input.join("lib-source.bundle"), &bundle);
        assert_eq!(
            fixture.write(&fixture.output_a),
            Err(ReleaseArtifactError::InvalidSourceBundle)
        );
    }

    #[test]
    fn output_name_is_one_bounded_component() {
        assert!(valid_output_component("release-v1"));
        for value in ["", ".", "..", "a/b", "/absolute", "a\\b"] {
            assert!(!valid_output_component(value), "{value}");
        }
        assert!(!valid_output_component(&"a".repeat(129)));
    }

    #[test]
    fn output_scope_and_target_admission_are_fail_closed() {
        let fixture = ReleaseFixture::new();
        assert_eq!(
            fixture.write(&fixture.service.join("release")),
            Err(ReleaseArtifactError::InvalidOutputRoot)
        );
        assert_eq!(
            fixture.write(&fixture.input.join("release")),
            Err(ReleaseArtifactError::InvalidOutputRoot)
        );
        assert_eq!(
            run_inner(
                CommandMode::Write,
                &fixture.service,
                &fixture.input,
                &fixture.output_a,
                "x86_64-apple-darwin",
                1_700_000_000,
            ),
            Err(ReleaseArtifactError::InvalidServiceMetadata)
        );
        assert_eq!(
            run_inner(
                CommandMode::Write,
                &fixture.service,
                &fixture.input,
                &fixture.output_a,
                "x86_64-unknown-linux-gnu",
                0,
            ),
            Err(ReleaseArtifactError::InvalidServiceMetadata)
        );
    }

    #[test]
    fn secret_scanner_detects_a_pattern_across_chunk_boundaries() {
        let mut scanner = SecretScanner::default();
        scanner.scan(b"prefix -----BEGIN OPENSSH").expect("prefix");
        assert_eq!(
            scanner.scan(b" PRIVATE KEY----- suffix"),
            Err(ReleaseArtifactError::ProtectedMaterialDetected)
        );
    }

    #[test]
    fn identifiers_and_sources_are_closed() {
        assert!(valid_snake_identifier("fixture_service"));
        assert!(!valid_snake_identifier("fixture__service"));
        assert!(valid_kebab_identifier("fixture-service"));
        assert!(!valid_kebab_identifier("fixture--service"));
        assert!(valid_public_source(
            "registry+https://github.com/rust-lang/crates.io-index"
        ));
        assert!(valid_public_source(
            "git+https://github.com/radrootslabs/lib?rev=1111111111111111111111111111111111111111#1111111111111111111111111111111111111111"
        ));
        assert!(!valid_public_source(
            "git+ssh://git@github.com/private/repo"
        ));
        assert!(!valid_public_source("path+file:///secret"));
    }

    #[test]
    fn errors_are_stable_and_source_free() {
        let errors = [
            ReleaseArtifactError::InvalidContract,
            ReleaseArtifactError::InvalidServiceRoot,
            ReleaseArtifactError::DirtyServiceSource,
            ReleaseArtifactError::InvalidServiceMetadata,
            ReleaseArtifactError::InvalidInputRoot,
            ReleaseArtifactError::InvalidInputArtifact,
            ReleaseArtifactError::InvalidSourceLock,
            ReleaseArtifactError::InvalidSourceBundle,
            ReleaseArtifactError::InvalidPackageInventory,
            ReleaseArtifactError::ProtectedMaterialDetected,
            ReleaseArtifactError::InvalidOutputRoot,
            ReleaseArtifactError::StaleOutput,
            ReleaseArtifactError::GenerationFailure,
        ];
        for error in errors {
            let display = error.to_string();
            let debug = format!("{error:?}");
            assert!(!display.contains('/'));
            assert!(!display.contains("secret"));
            assert!(!debug.contains('/'));
            assert!(std::error::Error::source(&error).is_none());
        }
    }
}
