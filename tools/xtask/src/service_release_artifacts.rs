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

use crate::safe_artifact_io::{TarGzipLimits, TraversalLimits};
use crate::service_source_lock::{LIB_REPOSITORY, validate_deferred_nix_material};
use crate::service_source_lock_v3::{
    LOCK_FILENAME, PREDECESSOR_LOCK_FILENAME, ServiceSourceLockV3,
};
use crate::{artifact_admission, exact_tree_archive, safe_artifact_io};

const CONTRACT_RELATIVE: &str =
    "contracts/architecture/decisions/services_hardening_release_artifacts.v4.json";
const INPUT_NAMES: [&str; 6] = [
    "config.example.toml",
    "config.schema.json",
    "nixos-module.nix",
    "oci-image.tar.gz",
    "service-binary",
    "systemd.service",
];
const OUTPUT_NAMES: [&str; 20] = [
    "LICENSE-APACHE",
    "LICENSE-MIT",
    "SHA256SUMS",
    "THIRD-PARTY-LICENSES.txt",
    "THIRD-PARTY-NOTICES.txt",
    "artifact-manifest.v2.json",
    "artifact-scan.v1.json",
    "binary.tar.gz",
    "config.example.toml",
    "config.schema.json",
    "lib-source.tar",
    "nixos-module.nix",
    "oci-image.tar.gz",
    "oci-image.v1.json",
    "provenance.intoto.jsonl",
    "radroots.service.source-lock.v3.toml",
    "sbom.cdx.json",
    "service-source.tar",
    "source-archives.v3.json",
    "systemd.service",
];
const SUPPORTED_TARGETS: [&str; 2] = ["aarch64-apple-darwin", "x86_64-unknown-linux-gnu"];
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
const MAX_SOURCE_ARCHIVE_BYTES: u64 = 1_073_741_824;
const MAX_SOURCE_ARCHIVE_MEMBER_BYTES: u64 = 67_108_864;
const MAX_SOURCE_ARCHIVE_MEMBERS: u64 = 65_536;
const MAX_OCI_BYTES: u64 = 2_147_483_648;
const MAX_METADATA_BYTES: usize = 33_554_432;
const MAX_GIT_OUTPUT_BYTES: usize = 65_536;
const MAX_PACKAGES: usize = 8_192;
const MAX_WORKSPACE_PACKAGES: usize = 64;
const MAX_TEXT_FIELD_BYTES: usize = 512;
const MAX_ARCHIVE_EXPANDED_BYTES: u64 = 17_179_869_184;
const MAX_ARCHIVE_PATH_BYTES: usize = 4_096;
const MAX_RELEASE_TREE_BYTES: u64 = 68_719_476_736;
const FILE_MODE: u32 = 0o644;
const DIRECTORY_MODE: u32 = 0o755;
const CANDIDATE_DIGEST_DOMAIN: &str = "sha256:";
const SECRET_SCAN_OVERLAP_BYTES: usize = 131_072;

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
    #[serde(skip)]
    license: String,
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
    manifest_path: String,
    license_file: Option<String>,
    targets: Vec<CargoTarget>,
    #[serde(skip)]
    license_texts: Vec<DependencyLicenseText>,
}

#[derive(Clone, Debug)]
struct DependencyLicenseText {
    filename: String,
    sha256: String,
    text: String,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    purl: Option<String>,
    licenses: Vec<LicenseChoice>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    hashes: Vec<DigestValue>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    properties: Vec<SbomProperty>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
struct SbomProperty {
    name: String,
    value: String,
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
    properties: Vec<SbomProperty>,
}

#[derive(Debug, Serialize)]
struct SbomComposition {
    aggregate: &'static str,
    assemblies: Vec<String>,
    dependencies: Vec<String>,
}

#[derive(Debug, Serialize)]
struct CycloneDxSbom {
    #[serde(rename = "$schema")]
    json_schema: &'static str,
    #[serde(rename = "bomFormat")]
    bom_format: &'static str,
    #[serde(rename = "specVersion")]
    spec_version: &'static str,
    version: u32,
    metadata: SbomMetadata,
    components: Vec<SbomComponent>,
    dependencies: Vec<SbomDependency>,
    compositions: Vec<SbomComposition>,
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
struct SourceArchiveDocument {
    schema: &'static str,
    contract_version: u32,
    candidate_digest: String,
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
    format: &'static str,
    compression: &'static str,
    git_history: &'static str,
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
    candidate_digest: String,
    service: String,
    version: String,
    target: String,
    source_date_epoch: u32,
    service_revision: String,
    lib_revision: String,
    rust_version: &'static str,
    host_feature_profile: &'static str,
    contract_versions: ContractVersionsDocument,
    confidentiality: ConfidentialityDocument,
    artifacts: Vec<ArtifactRecord>,
}

#[derive(Debug, Serialize)]
struct ConfidentialityDocument {
    state: &'static str,
    derived_from: ArtifactRecord,
    protected_material_included: bool,
}

#[derive(Debug, Serialize)]
struct ScanDocument {
    schema: &'static str,
    contract_version: u32,
    candidate_digest: String,
    state: &'static str,
    ruleset_sha256: String,
    scanned_artifacts: Vec<ArtifactRecord>,
    nested_members_scanned: u64,
    expanded_bytes_scanned: u64,
}

#[derive(Debug, Serialize)]
struct InTotoStatement {
    #[serde(rename = "_type")]
    statement_type: &'static str,
    subject: Vec<InTotoSubject>,
    #[serde(rename = "predicateType")]
    predicate_type: &'static str,
    predicate: SlsaPredicate,
}

#[derive(Debug, Serialize)]
struct InTotoSubject {
    name: String,
    digest: BTreeMap<&'static str, String>,
}

#[derive(Debug, Serialize)]
struct SlsaPredicate {
    #[serde(rename = "buildDefinition")]
    build_definition: SlsaBuildDefinition,
    #[serde(rename = "runDetails")]
    run_details: SlsaRunDetails,
}

#[derive(Debug, Serialize)]
struct SlsaBuildDefinition {
    #[serde(rename = "buildType")]
    build_type: &'static str,
    #[serde(rename = "externalParameters")]
    external_parameters: SlsaExternalParameters,
    #[serde(rename = "internalParameters")]
    internal_parameters: BTreeMap<String, String>,
    #[serde(rename = "resolvedDependencies")]
    resolved_dependencies: Vec<SlsaResolvedDependency>,
}

#[derive(Debug, Serialize)]
struct SlsaExternalParameters {
    candidate_digest: String,
    service: String,
    target: String,
    source_date_epoch: u32,
}

#[derive(Debug, Serialize)]
struct SlsaResolvedDependency {
    uri: String,
    digest: BTreeMap<&'static str, String>,
}

#[derive(Debug, Serialize)]
struct SlsaRunDetails {
    builder: SlsaBuilder,
    metadata: SlsaRunMetadata,
}

#[derive(Debug, Serialize)]
struct SlsaBuilder {
    id: &'static str,
}

#[derive(Debug, Serialize)]
struct SlsaRunMetadata {
    #[serde(rename = "invocationId")]
    invocation_id: String,
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
    owner_step: u32,
    predecessor: ReleasePredecessor,
    command: String,
    modes: Vec<String>,
    required_arguments: Vec<String>,
    service_metadata_path: String,
    service_metadata_fields: Vec<String>,
    service_license_path: String,
    source_lock_schema: String,
    source_lock_definition: String,
    artifact_contract_binding: String,
    artifact_admission_contract: String,
    supported_targets: Vec<String>,
    candidate_binding: String,
    lib_root_binding: String,
    binary_admission: String,
    oci_admission: String,
    input_inventory: Vec<String>,
    excluded_parent_owned_inputs: Vec<String>,
    service_root_inventory: Vec<String>,
    output_inventory: Vec<String>,
    canonical_json: String,
    checksum_format: String,
    source_archive_format: String,
    sbom_format: String,
    license_evidence: String,
    provenance_posture: String,
    protected_material_scan_scope: String,
    confidentiality_state: String,
    source_cleanliness: String,
    revision_stability: String,
    no_protected_material: bool,
    maximums: ReleaseMaximums,
    required_negative_vectors: Vec<String>,
    negative_error_codes: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReleasePredecessor {
    schema: String,
    filename: String,
    transition: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReleaseMaximums {
    text_input_bytes: u64,
    generated_document_bytes: u64,
    service_cargo_lock_bytes: u64,
    service_flake_lock_bytes: u64,
    binary_bytes: u64,
    source_archive_bytes: u64,
    source_archive_member_bytes: u64,
    source_archive_members: u64,
    oci_bytes: u64,
    artifact_scan_expanded_bytes: u64,
    cargo_metadata_bytes: usize,
    packages: usize,
    workspace_packages: usize,
}

#[derive(Clone, Copy)]
pub(crate) struct Arguments<'a> {
    pub(crate) mode: CommandMode,
    pub(crate) service_root: &'a Path,
    pub(crate) lib_root: &'a Path,
    pub(crate) input_root: &'a Path,
    pub(crate) output_root: &'a Path,
    pub(crate) target: &'a str,
    pub(crate) source_date_epoch: u32,
    pub(crate) candidate_digest: &'a str,
}

pub(crate) fn run(arguments: Arguments<'_>) -> Result<(), String> {
    run_inner(arguments).map_err(|error| error.to_string())
}

fn run_inner(arguments: Arguments<'_>) -> Result<(), ReleaseArtifactError> {
    let Arguments {
        mode,
        service_root,
        lib_root,
        input_root,
        output_root,
        target,
        source_date_epoch,
        candidate_digest,
    } = arguments;
    if !SUPPORTED_TARGETS.contains(&target)
        || source_date_epoch == 0
        || !valid_lower_hex(candidate_digest, 64)
    {
        return Err(ReleaseArtifactError::InvalidServiceMetadata);
    }
    let service_root = validate_git_root(service_root)?;
    let lib_root = validate_git_root(lib_root)?;
    if git_remote(&lib_root)? != "ssh://git@github.com/radrootslabs/lib.git"
        && git_remote(&lib_root)? != LIB_REPOSITORY
    {
        return Err(ReleaseArtifactError::InvalidServiceRoot);
    }
    let (input_root, input_snapshot) = validate_exact_input_root(input_root)?;
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
    let source_lock = ServiceSourceLockV3::from_canonical_bytes(&source_lock_bytes)
        .map_err(|_| ReleaseArtifactError::InvalidSourceLock)?;
    if source_lock.service() != metadata.service {
        return Err(ReleaseArtifactError::InvalidSourceLock);
    }
    validate_source_lock_files(&service_root, &source_lock)?;
    let cargo_metadata = cargo_metadata(&service_root)?;
    let (sbom, notices, license_texts) = build_supply_chain_documents(&metadata, cargo_metadata)?;

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
        copy_snapshot_file(
            &input_snapshot,
            input,
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
    create_binary_archive_from_snapshot(
        &input_snapshot,
        "service-binary",
        &staging.path().join("binary.tar.gz"),
        &metadata.binary_name,
        target,
        source_date_epoch,
    )?;
    let oci = copy_snapshot_file(
        &input_snapshot,
        "oci-image.tar.gz",
        &staging.path().join("oci-image.tar.gz"),
        MAX_OCI_BYTES,
    )?;
    let versions = source_lock.contract_versions();
    artifact_admission::admit_oci(
        &staging.path().join("oci-image.tar.gz"),
        staging.path(),
        &artifact_admission::OciExpectation {
            service: &metadata.service,
            binary_name: &metadata.binary_name,
            version: &metadata.version,
            service_revision: &initial_head,
            lib_revision: source_lock.revision(),
            license: &metadata.license,
            contract_versions: artifact_admission::ContractVersions {
                admin: versions.admin(),
                config: versions.config(),
                provider: versions.provider(),
                state: versions.state(),
                status: versions.status(),
            },
        },
    )
    .map_err(|_| ReleaseArtifactError::InvalidInputArtifact)?;
    input_snapshot
        .revalidate()
        .map_err(|_| ReleaseArtifactError::InvalidInputRoot)?;
    let service_source = exact_tree_archive::create(
        &service_root,
        &initial_head,
        &staging.path().join("service-source.tar"),
        u64::from(source_date_epoch),
    )
    .map_err(|_| ReleaseArtifactError::InvalidSourceBundle)?;
    let lib_mtime = exact_tree_archive::commit_timestamp(&lib_root, source_lock.revision())
        .map_err(|_| ReleaseArtifactError::InvalidSourceBundle)?;
    let lib_source = exact_tree_archive::create(
        &lib_root,
        source_lock.revision(),
        &staging.path().join("lib-source.tar"),
        lib_mtime,
    )
    .map_err(|_| ReleaseArtifactError::InvalidSourceBundle)?;
    if lib_source.sha256 != source_lock.source_archive_sha256() {
        return Err(ReleaseArtifactError::InvalidSourceBundle);
    }
    let catalog = exact_tree_archive::read_blob(
        &lib_root,
        source_lock.revision(),
        "contracts/crates/catalog.v2.toml",
        MAX_TEXT_INPUT_BYTES as usize,
    )
    .map_err(|_| ReleaseArtifactError::InvalidSourceBundle)?;
    if sha256_bytes(&catalog) != source_lock.workspace_catalog_sha256() {
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
    let source_document = SourceArchiveDocument {
        schema: "radroots.service.source-archives.v3",
        contract_version: 3,
        candidate_digest: candidate_digest.to_owned(),
        service: metadata.service.clone(),
        service_revision: initial_head.clone(),
        lib_repository: LIB_REPOSITORY,
        lib_revision: source_lock.revision().to_owned(),
        service_source: artifact_record_from_exact_tree("service-source.tar", &service_source),
        lib_source: artifact_record_from_exact_tree("lib-source.tar", &lib_source),
        source_lock_sha256: source_lock_sha256.clone(),
        workspace_catalog_sha256: source_lock.workspace_catalog_sha256().to_owned(),
        cargo_lock_sha256: source_lock.cargo_lock_sha256().to_owned(),
        flake_lock_sha256: source_lock.flake_lock_sha256().to_owned(),
        format: "ustar",
        compression: "none",
        git_history: "forbidden",
    };
    write_json(
        &staging.path().join("source-archives.v3.json"),
        &source_document,
    )?;
    write_generated(
        &staging.path().join("THIRD-PARTY-NOTICES.txt"),
        notices.as_bytes(),
    )?;
    write_generated(
        &staging.path().join("THIRD-PARTY-LICENSES.txt"),
        license_texts.as_bytes(),
    )?;

    let mut sbom = sbom;
    let sbom_artifacts = inventory_records(staging.path())?;
    reconcile_sbom_artifacts(&mut sbom, &sbom_artifacts);
    validate_cyclonedx_profile(&sbom, &sbom_artifacts)?;
    write_json(&staging.path().join("sbom.cdx.json"), &sbom)?;
    let scanned_artifacts = inventory_records(staging.path())?;
    let (nested_members_scanned, expanded_bytes_scanned) =
        scan_artifact_inventory(staging.path(), &scanned_artifacts)?;
    let scan = ScanDocument {
        schema: "radroots.service.artifact-scan.v1",
        contract_version: 1,
        candidate_digest: candidate_digest.to_owned(),
        state: "no_protected_material_detected",
        ruleset_sha256: scanner_ruleset_sha256(),
        scanned_artifacts,
        nested_members_scanned,
        expanded_bytes_scanned,
    };
    write_json(&staging.path().join("artifact-scan.v1.json"), &scan)?;
    let scan_evidence = hash_regular(
        &staging.path().join("artifact-scan.v1.json"),
        MAX_GENERATED_DOCUMENT_BYTES,
    )?;

    let payload = inventory_records(staging.path())?;
    let versions = source_lock.contract_versions();
    let manifest = ArtifactManifestDocument {
        schema: "radroots.service.release-artifacts.v2",
        contract_version: 2,
        candidate_digest: candidate_digest.to_owned(),
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
        confidentiality: ConfidentialityDocument {
            state: scan.state,
            derived_from: artifact_record("artifact-scan.v1.json", &scan_evidence),
            protected_material_included: false,
        },
        artifacts: payload.clone(),
    };
    validate_confidentiality_binding(&manifest, &scan, &scan_evidence, &payload)?;
    write_json(&staging.path().join("artifact-manifest.v2.json"), &manifest)?;
    let manifest_evidence = hash_regular(
        &staging.path().join("artifact-manifest.v2.json"),
        MAX_TEXT_INPUT_BYTES,
    )?;
    let service_repository = git_remote(&service_root)?;
    let provenance = build_provenance(
        ProvenanceInput {
            candidate_digest,
            service: &metadata.service,
            target,
            source_date_epoch,
            service_repository: &service_repository,
            service_revision: &initial_head,
            lib_revision: source_lock.revision(),
            source_lock_sha256: &source_lock_sha256,
            manifest_sha256: &manifest_evidence.sha256,
        },
        &payload,
    );
    validate_provenance_subjects(&provenance, candidate_digest, &payload)?;
    write_json(&staging.path().join("provenance.intoto.jsonl"), &provenance)?;
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

fn validate_confidentiality_binding(
    manifest: &ArtifactManifestDocument,
    scan: &ScanDocument,
    scan_evidence: &FileEvidence,
    payload: &[ArtifactRecord],
) -> Result<(), ReleaseArtifactError> {
    if manifest.candidate_digest != scan.candidate_digest
        || manifest.confidentiality.state != scan.state
        || manifest.confidentiality.protected_material_included
        || manifest.confidentiality.derived_from.path != "artifact-scan.v1.json"
        || manifest.confidentiality.derived_from.sha256 != scan_evidence.sha256
        || manifest.artifacts != payload
    {
        return Err(ReleaseArtifactError::GenerationFailure);
    }
    Ok(())
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
    let mut metadata = release
        .try_into::<ReleaseMetadata>()
        .map_err(|_| ReleaseArtifactError::InvalidServiceMetadata)?;
    metadata.license = value
        .get("workspace")
        .and_then(|workspace| workspace.get("package"))
        .or_else(|| value.get("package"))
        .and_then(|package| package.get("license"))
        .and_then(toml::Value::as_str)
        .ok_or(ReleaseArtifactError::InvalidServiceMetadata)?
        .to_owned();
    if !valid_snake_identifier(&metadata.service)
        || !valid_kebab_identifier(&metadata.service_package)
        || !valid_kebab_identifier(&metadata.binary_name)
        || metadata.version.len() > 128
        || metadata.license != "AGPL-3.0-or-later"
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
    source_lock: &ServiceSourceLockV3,
) -> Result<(), ReleaseArtifactError> {
    for name in [
        PREDECESSOR_LOCK_FILENAME,
        "radroots.service.source-lock.v1.toml",
    ] {
        match fs::symlink_metadata(root.join(name)) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            _ => return Err(ReleaseArtifactError::InvalidSourceLock),
        }
    }
    let cargo_lock = hash_regular(&root.join("Cargo.lock"), MAX_SERVICE_CARGO_LOCK_BYTES)
        .map_err(|_| ReleaseArtifactError::InvalidSourceLock)?;
    if cargo_lock.sha256 != source_lock.cargo_lock_sha256() {
        return Err(ReleaseArtifactError::InvalidSourceLock);
    }
    let flake_nix = read_bounded_regular(
        &root.join("flake.nix"),
        MAX_TEXT_INPUT_BYTES,
        ReleaseArtifactError::InvalidSourceLock,
    )?;
    let flake_lock = read_bounded_regular(
        &root.join("flake.lock"),
        MAX_SERVICE_FLAKE_LOCK_BYTES,
        ReleaseArtifactError::InvalidSourceLock,
    )?;
    let evidence = validate_deferred_nix_material(&flake_nix, &flake_lock)
        .map_err(|_| ReleaseArtifactError::InvalidSourceLock)?;
    let artifact_contract = hash_regular(
        &root.join(source_lock.artifact_contract_path()),
        MAX_TEXT_INPUT_BYTES,
    )
    .map_err(|_| ReleaseArtifactError::InvalidSourceLock)?;
    if evidence.lib_revision() == source_lock.revision()
        && evidence.flake_lock_sha256() == source_lock.flake_lock_sha256()
        && artifact_contract.sha256 == source_lock.artifact_contract_sha256()
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
    let mut metadata = serde_json::from_slice::<CargoMetadata>(&bytes)
        .map_err(|_| ReleaseArtifactError::InvalidPackageInventory)?;
    for package in &mut metadata.packages {
        if package.source.is_some() {
            package.license_texts = dependency_license_texts(package)?;
        }
    }
    Ok(metadata)
}

fn dependency_license_texts(
    package: &CargoPackage,
) -> Result<Vec<DependencyLicenseText>, ReleaseArtifactError> {
    let manifest = Path::new(&package.manifest_path);
    if !manifest.is_absolute() || manifest.file_name() != Some(OsStr::new("Cargo.toml")) {
        return Err(ReleaseArtifactError::InvalidPackageInventory);
    }
    let package_root = manifest
        .parent()
        .ok_or(ReleaseArtifactError::InvalidPackageInventory)?
        .canonicalize()
        .map_err(|_| ReleaseArtifactError::InvalidPackageInventory)?;
    let mut candidates = BTreeSet::new();
    if let Some(license_file) = package.license_file.as_deref() {
        let relative = Path::new(license_file);
        if relative.is_absolute()
            || relative
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(ReleaseArtifactError::InvalidPackageInventory);
        }
        candidates.insert(relative.to_path_buf());
    } else {
        let entries = fs::read_dir(&package_root)
            .map_err(|_| ReleaseArtifactError::InvalidPackageInventory)?;
        for (index, entry) in entries.enumerate() {
            if index >= 256 {
                return Err(ReleaseArtifactError::InvalidPackageInventory);
            }
            let entry = entry.map_err(|_| ReleaseArtifactError::InvalidPackageInventory)?;
            let filename = entry
                .file_name()
                .into_string()
                .map_err(|_| ReleaseArtifactError::InvalidPackageInventory)?;
            let uppercase = filename.to_ascii_uppercase();
            if ["LICENSE", "COPYING", "COPYRIGHT", "NOTICE"]
                .iter()
                .any(|prefix| {
                    uppercase == *prefix
                        || uppercase.starts_with(&format!("{prefix}-"))
                        || uppercase.starts_with(&format!("{prefix}."))
                })
            {
                candidates.insert(PathBuf::from(filename));
            }
        }
    }
    if candidates.is_empty() || candidates.len() > 16 {
        return Err(ReleaseArtifactError::InvalidPackageInventory);
    }
    let mut texts = Vec::with_capacity(candidates.len());
    let mut total = 0_u64;
    for relative in candidates {
        let path = package_root.join(&relative);
        let canonical = path
            .canonicalize()
            .map_err(|_| ReleaseArtifactError::InvalidPackageInventory)?;
        if canonical.parent() != Some(package_root.as_path()) {
            return Err(ReleaseArtifactError::InvalidPackageInventory);
        }
        let bytes = read_bounded_regular(
            &canonical,
            MAX_TEXT_INPUT_BYTES,
            ReleaseArtifactError::InvalidPackageInventory,
        )?;
        total = total
            .checked_add(bytes.len() as u64)
            .filter(|total| *total <= MAX_GENERATED_DOCUMENT_BYTES)
            .ok_or(ReleaseArtifactError::InvalidPackageInventory)?;
        scan_bytes(&bytes)?;
        let text =
            String::from_utf8(bytes).map_err(|_| ReleaseArtifactError::InvalidPackageInventory)?;
        if text.trim().is_empty() {
            return Err(ReleaseArtifactError::InvalidPackageInventory);
        }
        texts.push(DependencyLicenseText {
            filename: relative
                .to_str()
                .ok_or(ReleaseArtifactError::InvalidPackageInventory)?
                .to_owned(),
            sha256: sha256_bytes(text.as_bytes()),
            text,
        });
    }
    Ok(texts)
}

fn build_supply_chain_documents(
    metadata: &ReleaseMetadata,
    cargo: CargoMetadata,
) -> Result<(CycloneDxSbom, String, String), ReleaseArtifactError> {
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
        json_schema: "https://cyclonedx.org/schema/bom-1.6.schema.json",
        bom_format: "CycloneDX",
        spec_version: "1.6",
        version: 1,
        metadata: SbomMetadata {
            component: root_component,
            properties: vec![SbomProperty {
                name: "radroots:evidence:dependency-closure".to_owned(),
                value: "complete".to_owned(),
            }],
        },
        components,
        dependencies,
        compositions: Vec::new(),
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
    let mut licenses = String::from(
        "Radroots service third-party license texts v1\n\nThis file contains the exact bounded license texts admitted for each locked third-party Cargo package.\n",
    );
    if third_party.is_empty() {
        notices.push_str("\nNo third-party Cargo packages are present.\n");
        licenses.push_str("\nNo third-party Cargo packages are present.\n");
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
            if package.license_texts.is_empty() {
                return Err(ReleaseArtifactError::InvalidPackageInventory);
            }
            for license_text in &package.license_texts {
                if !valid_output_component(&license_text.filename)
                    || license_text.text.trim().is_empty()
                    || license_text.sha256 != sha256_bytes(license_text.text.as_bytes())
                {
                    return Err(ReleaseArtifactError::InvalidPackageInventory);
                }
                writeln!(
                    notices,
                    "License-Text: {} sha256:{}",
                    license_text.filename, license_text.sha256
                )
                .map_err(|_| ReleaseArtifactError::GenerationFailure)?;
                writeln!(licenses).map_err(|_| ReleaseArtifactError::GenerationFailure)?;
                writeln!(
                    licenses,
                    "===== {} {} / {} / sha256:{} =====",
                    package.name, package.version, license_text.filename, license_text.sha256
                )
                .map_err(|_| ReleaseArtifactError::GenerationFailure)?;
                licenses.push_str(&license_text.text);
                if !license_text.text.ends_with('\n') {
                    licenses.push('\n');
                }
            }
        }
    }
    scan_bytes(notices.as_bytes())?;
    scan_bytes(licenses.as_bytes())?;
    if licenses.len() as u64 > MAX_GENERATED_DOCUMENT_BYTES {
        return Err(ReleaseArtifactError::InvalidPackageInventory);
    }
    Ok((sbom, notices, licenses))
}

fn reconcile_sbom_artifacts(sbom: &mut CycloneDxSbom, artifacts: &[ArtifactRecord]) {
    let root_reference = sbom.metadata.component.bom_ref.clone();
    let mut assemblies = Vec::with_capacity(artifacts.len());
    for artifact in artifacts {
        let reference = format!("artifact:{}#{}", artifact.path, artifact.sha256);
        assemblies.push(reference.clone());
        sbom.components.push(SbomComponent {
            component_type: "file",
            bom_ref: reference.clone(),
            name: artifact.path.clone(),
            version: artifact.sha256.clone(),
            purl: None,
            licenses: sbom.metadata.component.licenses.clone(),
            hashes: vec![DigestValue {
                alg: "SHA-256",
                content: artifact.sha256.clone(),
            }],
            properties: vec![
                SbomProperty {
                    name: "radroots:artifact:byte-length".to_owned(),
                    value: artifact.byte_length.to_string(),
                },
                SbomProperty {
                    name: "radroots:ecosystem".to_owned(),
                    value: artifact_ecosystem(&artifact.path).to_owned(),
                },
            ],
        });
        sbom.dependencies.push(SbomDependency {
            reference,
            depends_on: vec![root_reference.clone()],
        });
    }
    sbom.components.sort();
    sbom.dependencies
        .sort_by(|left, right| left.reference.cmp(&right.reference));
    assemblies.sort();
    let mut dependencies =
        sbom.components
            .iter()
            .filter(|component| {
                component.properties.iter().any(|property| {
                    property.name == "radroots:ecosystem" && property.value == "cargo"
                })
            })
            .map(|component| component.bom_ref.clone())
            .collect::<Vec<_>>();
    dependencies.push(root_reference);
    dependencies.sort();
    dependencies.dedup();
    sbom.compositions = vec![SbomComposition {
        aggregate: "complete",
        assemblies,
        dependencies,
    }];
}

fn validate_cyclonedx_profile(
    sbom: &CycloneDxSbom,
    artifacts: &[ArtifactRecord],
) -> Result<(), ReleaseArtifactError> {
    let schema = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "additionalProperties": false,
        "required": ["$schema", "bomFormat", "specVersion", "version", "metadata", "components", "dependencies", "compositions"],
        "properties": {
            "$schema": {"const": "https://cyclonedx.org/schema/bom-1.6.schema.json"},
            "bomFormat": {"const": "CycloneDX"},
            "specVersion": {"const": "1.6"},
            "version": {"const": 1},
            "metadata": {"type": "object"},
            "components": {"type": "array", "minItems": 1},
            "dependencies": {"type": "array", "minItems": 1},
            "compositions": {"type": "array", "minItems": 1, "maxItems": 1}
        }
    });
    let validator =
        jsonschema::validator_for(&schema).map_err(|_| ReleaseArtifactError::GenerationFailure)?;
    let value = serde_json::to_value(sbom).map_err(|_| ReleaseArtifactError::GenerationFailure)?;
    validator
        .validate(&value)
        .map_err(|_| ReleaseArtifactError::GenerationFailure)?;

    let component_references = sbom
        .components
        .iter()
        .map(|component| component.bom_ref.as_str())
        .chain(std::iter::once(sbom.metadata.component.bom_ref.as_str()))
        .collect::<BTreeSet<_>>();
    if component_references.len() != sbom.components.len() + 1
        || sbom.dependencies.len() != component_references.len()
        || sbom.dependencies.iter().any(|dependency| {
            !component_references.contains(dependency.reference.as_str())
                || dependency
                    .depends_on
                    .iter()
                    .any(|reference| !component_references.contains(reference.as_str()))
        })
    {
        return Err(ReleaseArtifactError::GenerationFailure);
    }
    let composition = sbom
        .compositions
        .first()
        .ok_or(ReleaseArtifactError::GenerationFailure)?;
    let expected_artifacts = artifacts
        .iter()
        .map(|artifact| format!("artifact:{}#{}", artifact.path, artifact.sha256))
        .collect::<BTreeSet<_>>();
    if composition.aggregate != "complete"
        || composition.assemblies.iter().collect::<BTreeSet<_>>()
            != expected_artifacts.iter().collect::<BTreeSet<_>>()
        || artifacts.iter().any(|artifact| {
            sbom.components
                .iter()
                .filter(|component| {
                    component.bom_ref == format!("artifact:{}#{}", artifact.path, artifact.sha256)
                        && component.hashes
                            == [DigestValue {
                                alg: "SHA-256",
                                content: artifact.sha256.clone(),
                            }]
                })
                .count()
                != 1
        })
    {
        return Err(ReleaseArtifactError::GenerationFailure);
    }
    Ok(())
}

fn validate_provenance_subjects(
    provenance: &InTotoStatement,
    candidate_digest: &str,
    artifacts: &[ArtifactRecord],
) -> Result<(), ReleaseArtifactError> {
    let expected = artifacts
        .iter()
        .map(|artifact| (artifact.path.as_str(), artifact.sha256.as_str()))
        .collect::<BTreeSet<_>>();
    let observed = provenance
        .subject
        .iter()
        .filter_map(|subject| {
            subject
                .digest
                .get("sha256")
                .map(|digest| (subject.name.as_str(), digest.as_str()))
        })
        .collect::<BTreeSet<_>>();
    if provenance.statement_type != "https://in-toto.io/Statement/v1"
        || provenance.predicate_type != "https://slsa.dev/provenance/v1"
        || provenance
            .predicate
            .build_definition
            .external_parameters
            .candidate_digest
            != format!("{CANDIDATE_DIGEST_DOMAIN}{candidate_digest}")
        || observed != expected
        || provenance.subject.len() != expected.len()
    {
        return Err(ReleaseArtifactError::GenerationFailure);
    }
    Ok(())
}

fn artifact_ecosystem(path: &str) -> &'static str {
    if path == "oci-image.tar.gz" || path == "nixos-module.nix" {
        "nix"
    } else if path.ends_with("-source.tar") {
        "git"
    } else if path == "binary.tar.gz" {
        "native"
    } else {
        "release"
    }
}

struct ProvenanceInput<'a> {
    candidate_digest: &'a str,
    service: &'a str,
    target: &'a str,
    source_date_epoch: u32,
    service_repository: &'a str,
    service_revision: &'a str,
    lib_revision: &'a str,
    source_lock_sha256: &'a str,
    manifest_sha256: &'a str,
}

fn build_provenance(input: ProvenanceInput<'_>, artifacts: &[ArtifactRecord]) -> InTotoStatement {
    let ProvenanceInput {
        candidate_digest,
        service,
        target,
        source_date_epoch,
        service_repository,
        service_revision,
        lib_revision,
        source_lock_sha256,
        manifest_sha256,
    } = input;
    let mut subject = artifacts
        .iter()
        .map(|artifact| InTotoSubject {
            name: artifact.path.clone(),
            digest: BTreeMap::from([("sha256", artifact.sha256.clone())]),
        })
        .collect::<Vec<_>>();
    subject.sort_by(|left, right| left.name.cmp(&right.name));
    let mut resolved_dependencies = vec![
        SlsaResolvedDependency {
            uri: format!("git+{service_repository}@{service_revision}"),
            digest: BTreeMap::from([("gitCommit", service_revision.to_owned())]),
        },
        SlsaResolvedDependency {
            uri: format!("git+{LIB_REPOSITORY}@{lib_revision}"),
            digest: BTreeMap::from([("gitCommit", lib_revision.to_owned())]),
        },
        SlsaResolvedDependency {
            uri: format!("file:{LOCK_FILENAME}"),
            digest: BTreeMap::from([("sha256", source_lock_sha256.to_owned())]),
        },
        SlsaResolvedDependency {
            uri: "file:artifact-manifest.v2.json".to_owned(),
            digest: BTreeMap::from([("sha256", manifest_sha256.to_owned())]),
        },
    ];
    resolved_dependencies.sort_by(|left, right| left.uri.cmp(&right.uri));
    let invocation_id = sha256_bytes(
        format!(
            "radroots.service.slsa.invocation.v1\0{candidate_digest}\0{service}\0{target}\0{manifest_sha256}"
        )
        .as_bytes(),
    );
    InTotoStatement {
        statement_type: "https://in-toto.io/Statement/v1",
        subject,
        predicate_type: "https://slsa.dev/provenance/v1",
        predicate: SlsaPredicate {
            build_definition: SlsaBuildDefinition {
                build_type: "https://radroots.dev/contracts/service-release-artifacts/v2",
                external_parameters: SlsaExternalParameters {
                    candidate_digest: format!("{CANDIDATE_DIGEST_DOMAIN}{candidate_digest}"),
                    service: service.to_owned(),
                    target: target.to_owned(),
                    source_date_epoch,
                },
                internal_parameters: BTreeMap::new(),
                resolved_dependencies,
            },
            run_details: SlsaRunDetails {
                builder: SlsaBuilder {
                    id: "https://radroots.dev/builders/service-release-artifacts/v2",
                },
                metadata: SlsaRunMetadata { invocation_id },
            },
        },
    }
}

fn scanner_ruleset_sha256() -> String {
    let mut bytes = b"radroots.service.artifact-scan.rules.v1\0".to_vec();
    for pattern in SECRET_PATTERNS {
        bytes.extend_from_slice(pattern);
        bytes.push(0);
    }
    sha256_bytes(&bytes)
}

fn scan_artifact_inventory(
    root: &Path,
    artifacts: &[ArtifactRecord],
) -> Result<(u64, u64), ReleaseArtifactError> {
    let mut nested_members = 0_u64;
    let mut expanded_bytes = 0_u64;
    for artifact in artifacts {
        let path = root.join(&artifact.path);
        expanded_bytes = expanded_bytes
            .checked_add(scan_regular_file(&path, artifact.byte_length)?)
            .ok_or(ReleaseArtifactError::InvalidInputArtifact)?;
        if artifact.path == "service-source.tar" || artifact.path == "lib-source.tar" {
            let (members, bytes) = scan_tar_members(&path, false)?;
            nested_members = nested_members
                .checked_add(members)
                .ok_or(ReleaseArtifactError::InvalidInputArtifact)?;
            expanded_bytes = expanded_bytes
                .checked_add(bytes)
                .ok_or(ReleaseArtifactError::InvalidInputArtifact)?;
        } else if artifact.path == "binary.tar.gz" {
            let (members, bytes) = scan_tar_gzip_members(&path)?;
            nested_members = nested_members
                .checked_add(members)
                .ok_or(ReleaseArtifactError::InvalidInputArtifact)?;
            expanded_bytes = expanded_bytes
                .checked_add(bytes)
                .ok_or(ReleaseArtifactError::InvalidInputArtifact)?;
        } else if artifact.path == "oci-image.tar.gz" {
            let limits = TarGzipLimits {
                max_compressed_bytes: MAX_OCI_BYTES,
                max_expanded_bytes: MAX_ARCHIVE_EXPANDED_BYTES,
                max_members: 65_536,
                max_member_bytes: MAX_ARCHIVE_EXPANDED_BYTES,
                max_payload_bytes: MAX_ARCHIVE_EXPANDED_BYTES,
                max_depth: 64,
                max_path_bytes: MAX_ARCHIVE_PATH_BYTES,
            };
            let materialized =
                safe_artifact_io::materialize_tar_gzip_path(path.as_path(), root, limits)
                    .map_err(|_| ReleaseArtifactError::InvalidInputArtifact)?;
            for file in materialized.snapshot().files() {
                let relative = file
                    .relative_path()
                    .to_str()
                    .ok_or(ReleaseArtifactError::InvalidInputArtifact)?;
                validate_scanned_path(relative)?;
                let evidence = materialized
                    .snapshot()
                    .hash(file, MAX_ARCHIVE_EXPANDED_BYTES)
                    .map_err(|_| ReleaseArtifactError::InvalidInputArtifact)?;
                expanded_bytes = expanded_bytes
                    .checked_add(scan_regular_file(
                        &materialized.root().join(file.relative_path()),
                        evidence.byte_length,
                    )?)
                    .ok_or(ReleaseArtifactError::InvalidInputArtifact)?;
                nested_members = nested_members
                    .checked_add(1)
                    .ok_or(ReleaseArtifactError::InvalidInputArtifact)?;
                if relative.ends_with("/layer.tar") {
                    let (members, bytes) =
                        scan_tar_members(&materialized.root().join(file.relative_path()), true)?;
                    nested_members = nested_members
                        .checked_add(members)
                        .ok_or(ReleaseArtifactError::InvalidInputArtifact)?;
                    expanded_bytes = expanded_bytes
                        .checked_add(bytes)
                        .ok_or(ReleaseArtifactError::InvalidInputArtifact)?;
                }
            }
            materialized
                .revalidate()
                .map_err(|_| ReleaseArtifactError::InvalidInputArtifact)?;
        }
    }
    Ok((nested_members, expanded_bytes))
}

fn scan_tar_gzip_members(path: &Path) -> Result<(u64, u64), ReleaseArtifactError> {
    let file = fs::File::open(path).map_err(|_| ReleaseArtifactError::InvalidInputArtifact)?;
    let decoder = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);
    let mut members = 0_u64;
    let mut bytes = 0_u64;
    for entry in archive
        .entries()
        .map_err(|_| ReleaseArtifactError::InvalidInputArtifact)?
    {
        let mut entry = entry.map_err(|_| ReleaseArtifactError::InvalidInputArtifact)?;
        members = members
            .checked_add(1)
            .filter(|count| *count <= 4)
            .ok_or(ReleaseArtifactError::InvalidInputArtifact)?;
        let path = entry
            .path()
            .map_err(|_| ReleaseArtifactError::InvalidInputArtifact)?;
        let path = path
            .to_str()
            .ok_or(ReleaseArtifactError::InvalidInputArtifact)?;
        validate_scanned_path(path)?;
        if entry.header().entry_type().is_file() {
            let size = entry.size();
            if size > MAX_BINARY_BYTES {
                return Err(ReleaseArtifactError::InvalidInputArtifact);
            }
            bytes = bytes
                .checked_add(scan_reader(&mut entry, size)?)
                .filter(|count| *count <= MAX_BINARY_BYTES)
                .ok_or(ReleaseArtifactError::InvalidInputArtifact)?;
        } else if !entry.header().entry_type().is_dir() {
            return Err(ReleaseArtifactError::InvalidInputArtifact);
        }
    }
    if members == 0 {
        Err(ReleaseArtifactError::InvalidInputArtifact)
    } else {
        Ok((members, bytes))
    }
}

fn scan_tar_members(path: &Path, links_allowed: bool) -> Result<(u64, u64), ReleaseArtifactError> {
    let file = fs::File::open(path).map_err(|_| ReleaseArtifactError::InvalidInputArtifact)?;
    let mut archive = tar::Archive::new(file);
    let mut members = 0_u64;
    let mut bytes = 0_u64;
    for entry in archive
        .entries()
        .map_err(|_| ReleaseArtifactError::InvalidInputArtifact)?
    {
        let mut entry = entry.map_err(|_| ReleaseArtifactError::InvalidInputArtifact)?;
        let path = entry
            .path()
            .map_err(|_| ReleaseArtifactError::InvalidInputArtifact)?;
        let path = path
            .to_str()
            .ok_or(ReleaseArtifactError::InvalidInputArtifact)?;
        validate_scanned_path(path)?;
        members = members
            .checked_add(1)
            .filter(|count| *count <= MAX_SOURCE_ARCHIVE_MEMBERS)
            .ok_or(ReleaseArtifactError::InvalidInputArtifact)?;
        if entry.header().entry_type().is_file() {
            let size = entry.size();
            if !links_allowed && size > MAX_SOURCE_ARCHIVE_MEMBER_BYTES {
                return Err(ReleaseArtifactError::InvalidInputArtifact);
            }
            let scanned = scan_reader(&mut entry, size)?;
            bytes = bytes
                .checked_add(scanned)
                .filter(|count| *count <= MAX_ARCHIVE_EXPANDED_BYTES)
                .ok_or(ReleaseArtifactError::InvalidInputArtifact)?;
        } else if !links_allowed
            || !(entry.header().entry_type().is_dir()
                || entry.header().entry_type().is_symlink()
                || entry.header().entry_type().is_hard_link())
        {
            return Err(ReleaseArtifactError::InvalidInputArtifact);
        }
    }
    if members == 0 {
        Err(ReleaseArtifactError::InvalidInputArtifact)
    } else {
        Ok((members, bytes))
    }
}

fn scan_regular_file(path: &Path, expected_length: u64) -> Result<u64, ReleaseArtifactError> {
    let mut file = fs::File::open(path).map_err(|_| ReleaseArtifactError::InvalidInputArtifact)?;
    scan_reader(&mut file, expected_length)
}

fn scan_reader(
    reader: &mut impl std::io::Read,
    expected_length: u64,
) -> Result<u64, ReleaseArtifactError> {
    let mut scanner = SecretScanner::default();
    let mut total = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|_| ReleaseArtifactError::InvalidInputArtifact)?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(read as u64)
            .filter(|value| *value <= expected_length)
            .ok_or(ReleaseArtifactError::InvalidInputArtifact)?;
        scanner.scan(&buffer[..read])?;
    }
    if total == expected_length {
        Ok(total)
    } else {
        Err(ReleaseArtifactError::InvalidInputArtifact)
    }
}

fn validate_scanned_path(path: &str) -> Result<(), ReleaseArtifactError> {
    let sensitive = Path::new(path).components().any(|component| {
        let Component::Normal(value) = component else {
            return true;
        };
        let value = value.to_string_lossy().to_ascii_lowercase();
        matches!(
            value.as_str(),
            ".git"
                | ".ssh"
                | ".aws"
                | ".env"
                | "credentials"
                | "secrets"
                | "id_rsa"
                | "id_ed25519"
                | "private_key"
        )
    });
    if sensitive || path.len() > MAX_ARCHIVE_PATH_BYTES {
        Err(ReleaseArtifactError::ProtectedMaterialDetected)
    } else {
        Ok(())
    }
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
        purl: Some(format!("pkg:cargo/{}@{}", package.name, package.version)),
        licenses: vec![LicenseChoice {
            expression: license,
        }],
        hashes,
        properties: vec![SbomProperty {
            name: "radroots:ecosystem".to_owned(),
            value: "cargo".to_owned(),
        }],
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

fn validate_exact_input_root(
    path: &Path,
) -> Result<(PathBuf, safe_artifact_io::TraversalSnapshot), ReleaseArtifactError> {
    let root = validate_absolute_directory(path, ReleaseArtifactError::InvalidInputRoot)?;
    let snapshot = flat_directory_snapshot(&root, ReleaseArtifactError::InvalidInputRoot)?;
    let inventory = snapshot_inventory(&snapshot, ReleaseArtifactError::InvalidInputRoot)?;
    if inventory == INPUT_NAMES.into_iter().map(str::to_owned).collect() {
        Ok((root, snapshot))
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

#[cfg(test)]
fn create_binary_archive(
    source: &Path,
    output: &Path,
    binary_name: &str,
    source_date_epoch: u32,
) -> Result<FileEvidence, ReleaseArtifactError> {
    let (_stable, stable_source) = binary_staging(output)?;
    let source_evidence =
        safe_artifact_io::copy_regular_to_new_path(source, &stable_source, MAX_BINARY_BYTES)
            .map_err(|_| ReleaseArtifactError::InvalidInputArtifact)?;
    if source_evidence.byte_length == 0 {
        return Err(ReleaseArtifactError::InvalidInputArtifact);
    }
    write_binary_archive(
        &stable_source,
        output,
        binary_name,
        source_date_epoch,
        source_evidence,
    )
}

fn create_binary_archive_from_snapshot(
    snapshot: &safe_artifact_io::TraversalSnapshot,
    source_name: &str,
    output: &Path,
    binary_name: &str,
    target: &str,
    source_date_epoch: u32,
) -> Result<FileEvidence, ReleaseArtifactError> {
    let source = snapshot_file(snapshot, source_name)?;
    let (_stable, stable_source) = binary_staging(output)?;
    let source_evidence = snapshot
        .copy_to_new_path(source, &stable_source, MAX_BINARY_BYTES)
        .map_err(|_| ReleaseArtifactError::InvalidInputArtifact)?;
    if source_evidence.byte_length == 0 {
        return Err(ReleaseArtifactError::InvalidInputArtifact);
    }
    artifact_admission::admit_binary(&stable_source, target, true)
        .map_err(|_| ReleaseArtifactError::InvalidInputArtifact)?;
    write_binary_archive(
        &stable_source,
        output,
        binary_name,
        source_date_epoch,
        source_evidence,
    )
}

fn binary_staging(output: &Path) -> Result<(TempDir, PathBuf), ReleaseArtifactError> {
    let parent = output
        .parent()
        .ok_or(ReleaseArtifactError::GenerationFailure)?;
    let stable = tempfile::Builder::new()
        .prefix(".radroots-binary-input-")
        .tempdir_in(parent)
        .map_err(|_| ReleaseArtifactError::GenerationFailure)?;
    let stable_root = stable
        .path()
        .canonicalize()
        .map_err(|_| ReleaseArtifactError::GenerationFailure)?;
    Ok((stable, stable_root.join("admitted-binary")))
}

fn write_binary_archive(
    stable_source: &Path,
    output: &Path,
    binary_name: &str,
    source_date_epoch: u32,
    source_evidence: safe_artifact_io::FileEvidence,
) -> Result<FileEvidence, ReleaseArtifactError> {
    let mut source_file =
        fs::File::open(stable_source).map_err(|_| ReleaseArtifactError::GenerationFailure)?;
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
    header.set_size(source_evidence.byte_length);
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
    admit_binary_archive(output)?;
    hash_regular(output, MAX_BINARY_BYTES + MAX_TEXT_INPUT_BYTES)
}

fn snapshot_file<'a>(
    snapshot: &'a safe_artifact_io::TraversalSnapshot,
    name: &str,
) -> Result<&'a safe_artifact_io::TraversedFile, ReleaseArtifactError> {
    snapshot
        .files()
        .iter()
        .find(|file| file.relative_path() == Path::new(name))
        .ok_or(ReleaseArtifactError::InvalidInputRoot)
}

fn copy_snapshot_file(
    snapshot: &safe_artifact_io::TraversalSnapshot,
    source_name: &str,
    output: &Path,
    maximum: u64,
) -> Result<FileEvidence, ReleaseArtifactError> {
    let evidence = snapshot
        .copy_to_new_path(snapshot_file(snapshot, source_name)?, output, maximum)
        .map_err(|_| ReleaseArtifactError::InvalidInputArtifact)?;
    set_file_mode(output)?;
    Ok(FileEvidence {
        byte_length: evidence.byte_length,
        sha256: evidence.sha256,
    })
}

fn copy_bounded(
    source: &Path,
    output: &Path,
    maximum: u64,
) -> Result<FileEvidence, ReleaseArtifactError> {
    let evidence = safe_artifact_io::copy_regular_to_new_path(source, output, maximum)
        .map_err(|_| ReleaseArtifactError::InvalidInputArtifact)?;
    set_file_mode(output)?;
    Ok(FileEvidence {
        byte_length: evidence.byte_length,
        sha256: evidence.sha256,
    })
}

fn admit_binary_archive(path: &Path) -> Result<(), ReleaseArtifactError> {
    let limits = TarGzipLimits {
        max_compressed_bytes: MAX_BINARY_BYTES + MAX_TEXT_INPUT_BYTES,
        max_expanded_bytes: MAX_BINARY_BYTES + MAX_TEXT_INPUT_BYTES,
        max_members: 4,
        max_member_bytes: MAX_BINARY_BYTES,
        max_payload_bytes: MAX_BINARY_BYTES,
        max_depth: 4,
        max_path_bytes: MAX_ARCHIVE_PATH_BYTES,
    };
    safe_artifact_io::admit_tar_gzip_path(path, limits)
        .map(|_| ())
        .map_err(|_| ReleaseArtifactError::InvalidInputArtifact)
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
        if contains_secret(&combined) {
            return Err(ReleaseArtifactError::ProtectedMaterialDetected);
        }
        let retained = SECRET_SCAN_OVERLAP_BYTES.min(combined.len());
        self.tail.clear();
        self.tail
            .extend_from_slice(&combined[combined.len() - retained..]);
        Ok(())
    }
}

fn contains_secret(bytes: &[u8]) -> bool {
    let pem = [
        (
            b"-----BEGIN PRIVATE KEY-----".as_slice(),
            b"-----END PRIVATE KEY-----".as_slice(),
        ),
        (
            b"-----BEGIN RSA PRIVATE KEY-----".as_slice(),
            b"-----END RSA PRIVATE KEY-----".as_slice(),
        ),
        (
            b"-----BEGIN EC PRIVATE KEY-----".as_slice(),
            b"-----END EC PRIVATE KEY-----".as_slice(),
        ),
        (
            b"-----BEGIN OPENSSH PRIVATE KEY-----".as_slice(),
            b"-----END OPENSSH PRIVATE KEY-----".as_slice(),
        ),
    ]
    .iter()
    .any(|(begin, end)| contains_pem_secret(bytes, begin, end));
    let github_pat = contains_prefixed_secret(bytes, b"github_pat_", 50, 128, is_token_byte);
    let ghp = contains_prefixed_secret(bytes, b"ghp_", 36, 36, |byte| byte.is_ascii_alphanumeric());
    let slack = contains_prefixed_secret(bytes, b"xoxb-", 20, 128, |byte| {
        byte.is_ascii_digit() || byte == b'-'
    });
    pem || github_pat || ghp || slack
}

fn contains_pem_secret(bytes: &[u8], begin: &[u8], end: &[u8]) -> bool {
    let Some(begin_at) = bytes
        .windows(begin.len())
        .position(|window| window == begin)
    else {
        return false;
    };
    let body = &bytes[begin_at + begin.len()..];
    if !body.starts_with(b"\n") && !body.starts_with(b"\r\n") {
        return false;
    }
    let Some(end_at) = body.windows(end.len()).position(|window| window == end) else {
        return false;
    };
    let encoded = &body[..end_at];
    encoded.iter().all(|byte| {
        byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/' | b'=' | b'\n' | b'\r')
    }) && encoded
        .iter()
        .filter(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/' | b'='))
        .count()
        >= 32
}

fn contains_prefixed_secret(
    bytes: &[u8],
    prefix: &[u8],
    minimum_suffix: usize,
    maximum_suffix: usize,
    valid_suffix: impl Fn(u8) -> bool,
) -> bool {
    bytes
        .windows(prefix.len())
        .enumerate()
        .any(|(index, window)| {
            if window != prefix
                || index
                    .checked_sub(1)
                    .is_some_and(|prior| is_token_byte(bytes[prior]))
            {
                return false;
            }
            let start = index + prefix.len();
            let suffix = bytes[start..]
                .iter()
                .take_while(|byte| valid_suffix(**byte))
                .count();
            (minimum_suffix..=maximum_suffix).contains(&suffix)
                && bytes
                    .get(start + suffix)
                    .is_none_or(|byte| !is_token_byte(*byte))
        })
}

fn is_token_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
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
    inventory_records_impl(root, || {})
}

fn inventory_records_impl<F>(
    root: &Path,
    after_snapshot: F,
) -> Result<Vec<ArtifactRecord>, ReleaseArtifactError>
where
    F: FnOnce(),
{
    let snapshot = flat_directory_snapshot(root, ReleaseArtifactError::GenerationFailure)?;
    let names = snapshot_inventory(&snapshot, ReleaseArtifactError::GenerationFailure)?;
    after_snapshot();
    if snapshot.root_permission_mode() != DIRECTORY_MODE {
        return Err(ReleaseArtifactError::GenerationFailure);
    }
    let mut records = Vec::with_capacity(names.len());
    for name in names {
        let maximum = output_maximum(&name)?;
        let file = snapshot
            .files()
            .iter()
            .find(|file| file.relative_path() == Path::new(&name))
            .ok_or(ReleaseArtifactError::GenerationFailure)?;
        if file.permission_mode() != FILE_MODE {
            return Err(ReleaseArtifactError::GenerationFailure);
        }
        let evidence = snapshot
            .hash(file, maximum)
            .map_err(|_| ReleaseArtifactError::GenerationFailure)?;
        records.push(artifact_record(
            &name,
            &FileEvidence {
                byte_length: evidence.byte_length,
                sha256: evidence.sha256,
            },
        ));
    }
    snapshot
        .revalidate()
        .map_err(|_| ReleaseArtifactError::GenerationFailure)?;
    Ok(records)
}

fn artifact_record(path: &str, evidence: &FileEvidence) -> ArtifactRecord {
    ArtifactRecord {
        path: path.to_owned(),
        byte_length: evidence.byte_length,
        sha256: evidence.sha256.clone(),
    }
}

fn artifact_record_from_exact_tree(
    path: &str,
    evidence: &exact_tree_archive::ExactTreeArchiveEvidence,
) -> ArtifactRecord {
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
        "service-source.tar" | "lib-source.tar" => Ok(MAX_SOURCE_ARCHIVE_BYTES),
        "LICENSE-APACHE"
        | "LICENSE-MIT"
        | "config.example.toml"
        | "config.schema.json"
        | "nixos-module.nix"
        | "radroots.service.source-lock.v3.toml"
        | "systemd.service" => Ok(MAX_TEXT_INPUT_BYTES),
        "SHA256SUMS"
        | "THIRD-PARTY-LICENSES.txt"
        | "THIRD-PARTY-NOTICES.txt"
        | "artifact-manifest.v2.json"
        | "artifact-scan.v1.json"
        | "oci-image.v1.json"
        | "provenance.intoto.jsonl"
        | "sbom.cdx.json"
        | "source-archives.v3.json" => Ok(MAX_GENERATED_DOCUMENT_BYTES),
        _ => Err(ReleaseArtifactError::GenerationFailure),
    }
}

fn hash_regular(path: &Path, maximum: u64) -> Result<FileEvidence, ReleaseArtifactError> {
    let evidence = safe_artifact_io::hash_regular_path(path, maximum)
        .map_err(|_| ReleaseArtifactError::InvalidInputArtifact)?;
    Ok(FileEvidence {
        byte_length: evidence.byte_length,
        sha256: evidence.sha256,
    })
}

fn read_bounded_regular(
    path: &Path,
    maximum: u64,
    error: ReleaseArtifactError,
) -> Result<Vec<u8>, ReleaseArtifactError> {
    safe_artifact_io::read_regular_path(path, maximum).map_err(|_| error)
}

#[cfg(test)]
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

#[cfg(all(test, unix))]
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

#[cfg(all(test, not(unix)))]
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

#[cfg(test)]
fn directory_inventory(
    root: &Path,
    error: ReleaseArtifactError,
) -> Result<BTreeSet<String>, ReleaseArtifactError> {
    let snapshot = flat_directory_snapshot(root, error)?;
    snapshot_inventory(&snapshot, error)
}

fn flat_directory_snapshot(
    root: &Path,
    error: ReleaseArtifactError,
) -> Result<safe_artifact_io::TraversalSnapshot, ReleaseArtifactError> {
    let limits = TraversalLimits {
        max_entries: (OUTPUT_NAMES.len() + 1) as u64,
        max_files: (OUTPUT_NAMES.len() + 1) as u64,
        max_total_bytes: MAX_RELEASE_TREE_BYTES,
        max_file_bytes: MAX_ARCHIVE_EXPANDED_BYTES,
        max_depth: 1,
        max_path_bytes: MAX_ARCHIVE_PATH_BYTES,
    };
    safe_artifact_io::traverse_regular_files(root, limits, &[]).map_err(|_| error)
}

fn snapshot_inventory(
    snapshot: &safe_artifact_io::TraversalSnapshot,
    error: ReleaseArtifactError,
) -> Result<BTreeSet<String>, ReleaseArtifactError> {
    if snapshot.entry_count() != snapshot.files().len() as u64 {
        return Err(error);
    }
    let mut names = BTreeSet::new();
    for entry in snapshot.files() {
        let path = entry.relative_path();
        if path.components().count() != 1 {
            return Err(error);
        }
        let name = path
            .file_name()
            .and_then(OsStr::to_str)
            .ok_or(error)?
            .to_owned();
        if !names.insert(name) || names.len() > OUTPUT_NAMES.len() {
            return Err(error);
        }
    }
    snapshot.revalidate().map_err(|_| error)?;
    Ok(names)
}

fn validate_exact_output_inventory(root: &Path) -> Result<(), ReleaseArtifactError> {
    let expected = OUTPUT_NAMES.into_iter().map(str::to_owned).collect();
    let actual = inventory_records(root)?
        .into_iter()
        .map(|record| record.path)
        .collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(ReleaseArtifactError::GenerationFailure);
    }
    Ok(())
}

fn compare_output(expected: &Path, actual: &Path) -> Result<(), ReleaseArtifactError> {
    let actual = validate_absolute_directory(actual, ReleaseArtifactError::StaleOutput)?;
    let expected_records =
        inventory_records(expected).map_err(|_| ReleaseArtifactError::StaleOutput)?;
    let actual_records =
        inventory_records(&actual).map_err(|_| ReleaseArtifactError::StaleOutput)?;
    if expected_records != actual_records {
        return Err(ReleaseArtifactError::StaleOutput);
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

#[cfg(all(test, unix))]
fn validate_file_mode(path: &Path) -> Result<(), ReleaseArtifactError> {
    use std::os::unix::fs::PermissionsExt as _;
    let mode = fs::symlink_metadata(path)
        .map_err(|_| ReleaseArtifactError::StaleOutput)?
        .permissions()
        .mode()
        & 0o7777;
    if mode == FILE_MODE {
        Ok(())
    } else {
        Err(ReleaseArtifactError::StaleOutput)
    }
}

#[cfg(all(test, not(unix)))]
fn validate_file_mode(_path: &Path) -> Result<(), ReleaseArtifactError> {
    Ok(())
}

#[cfg(all(test, unix))]
fn validate_directory_mode(path: &Path) -> Result<(), ReleaseArtifactError> {
    use std::os::unix::fs::PermissionsExt as _;
    let mode = fs::symlink_metadata(path)
        .map_err(|_| ReleaseArtifactError::StaleOutput)?
        .permissions()
        .mode()
        & 0o7777;
    if mode == DIRECTORY_MODE {
        Ok(())
    } else {
        Err(ReleaseArtifactError::StaleOutput)
    }
}

#[cfg(all(test, not(unix)))]
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

fn valid_metadata_text(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_TEXT_FIELD_BYTES
        && !value.contains(['\n', '\r'])
        && !SECRET_PATTERNS.iter().any(|pattern| {
            value
                .as_bytes()
                .windows(pattern.len())
                .any(|window| window == *pattern)
        })
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
    if decision.schema != "radroots.services-hardening.release-artifacts-decisions.v4"
        || decision.contract_version != 4
        || decision.decision_state != "active"
        || decision.owner_step != 305
        || decision.predecessor.schema
            != "radroots.services-hardening.release-artifacts-decisions.v3"
        || decision.predecessor.filename != "services_hardening_release_artifacts.v3.json"
        || decision.predecessor.transition != "forward_only_replace"
        || decision.command != "cargo xtask service-release-artifacts"
        || decision.modes != ["check", "write"]
        || decision.required_arguments
            != [
                "mode",
                "service_root",
                "lib_root",
                "input_root",
                "output_root",
                "target",
                "source_date_epoch",
                "candidate_digest",
            ]
        || decision.service_metadata_path
            != "Cargo.toml.workspace.metadata.radroots.service_release"
        || decision.service_metadata_fields
            != ["service", "service_package", "binary_name", "version"]
        || decision.service_license_path
            != "Cargo.toml.workspace.package.license_or_package.license"
        || decision.source_lock_schema != "radroots.service.source-lock.v3"
        || decision.source_lock_definition
            != "contracts/architecture/decisions/services_hardening_source_lock.v3.json"
        || decision.artifact_contract_binding
            != "source_lock_exact_regular_file_bytes_in_same_service_revision"
        || decision.artifact_admission_contract
            != "contracts/architecture/decisions/services_hardening_artifact_admission.v1.json"
        || decision.supported_targets != SUPPORTED_TARGETS
        || decision.candidate_binding != "explicit_sha256_candidate_identity_digest"
        || decision.lib_root_binding
            != "canonical_public_lib_git_root_containing_the_locked_revision"
        || decision.binary_admission
            != "exact_format_architecture_linkage_structural_and_native_bounded_help_smoke"
        || decision.oci_admission
            != "safe_materialization_exact_manifest_config_AGPL_labels_layers_and_entrypoint"
        || decision.input_inventory != INPUT_NAMES
        || decision.excluded_parent_owned_inputs != ["backup_restore_runbook", "operator_runbook"]
        || decision.service_root_inventory != ["LICENSE-APACHE", "LICENSE-MIT", LOCK_FILENAME]
        || decision.output_inventory != OUTPUT_NAMES
        || decision.canonical_json != "compact_utf8_json_with_one_final_lf"
        || decision.checksum_format != "sha256_lower_hex_two_spaces_path_lf_sorted_by_path"
        || decision.source_archive_format
            != "canonical_uncompressed_ustar_exact_git_revision_tree_without_history"
        || decision.sbom_format != "cyclonedx_json_1_6_complete_cargo_nix_and_artifact_closure"
        || decision.license_evidence != "exact_dependency_attribution_with_bounded_license_texts"
        || decision.provenance_posture
            != "candidate_derived_unsigned_intoto_statement_slsa_v1_exact_manifest_subjects"
        || decision.protected_material_scan_scope
            != "all_artifact_bytes_and_bounded_nested_binary_oci_layer_and_source_archive_payloads"
        || decision.confidentiality_state != "derived_only_from_the_exact_artifact_scan_record"
        || decision.source_cleanliness != "no_tracked_staged_or_untracked_changes"
        || decision.revision_stability != "same_service_head_before_and_after_generation"
        || !decision.no_protected_material
        || decision.maximums.text_input_bytes != MAX_TEXT_INPUT_BYTES
        || decision.maximums.generated_document_bytes != MAX_GENERATED_DOCUMENT_BYTES
        || decision.maximums.service_cargo_lock_bytes != MAX_SERVICE_CARGO_LOCK_BYTES
        || decision.maximums.service_flake_lock_bytes != MAX_SERVICE_FLAKE_LOCK_BYTES
        || decision.maximums.binary_bytes != MAX_BINARY_BYTES
        || decision.maximums.source_archive_bytes != MAX_SOURCE_ARCHIVE_BYTES
        || decision.maximums.source_archive_member_bytes != MAX_SOURCE_ARCHIVE_MEMBER_BYTES
        || decision.maximums.source_archive_members != MAX_SOURCE_ARCHIVE_MEMBERS
        || decision.maximums.oci_bytes != MAX_OCI_BYTES
        || decision.maximums.artifact_scan_expanded_bytes != MAX_ARCHIVE_EXPANDED_BYTES
        || decision.maximums.cargo_metadata_bytes != MAX_METADATA_BYTES
        || decision.maximums.packages != MAX_PACKAGES
        || decision.maximums.workspace_packages != MAX_WORKSPACE_PACKAGES
        || decision.required_negative_vectors
            != [
                "cyclonedx_schema_drift",
                "missing_dependency_component",
                "unreconciled_artifact_subject",
                "missing_or_mismatched_license_text",
                "invented_candidate_digest",
                "git_history_bundle",
                "secret_in_binary",
                "secret_in_oci_layer",
                "secret_in_source_archive",
                "sensitive_archive_path",
                "scan_confidentiality_mismatch",
            ]
        || decision.negative_error_codes != errors.map(ReleaseArtifactError::code)
    {
        return Err(ReleaseArtifactError::InvalidContract);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};
    use std::process::Command;

    use crate::service_source_lock::ContractVersions;
    use crate::service_source_lock_v3::FixtureParts;

    use super::*;

    struct ReleaseFixture {
        _root: TempDir,
        service: PathBuf,
        lib: PathBuf,
        input: PathBuf,
        output_a: PathBuf,
        output_b: PathBuf,
    }

    impl ReleaseFixture {
        fn new() -> Self {
            let root = TempDir::new().expect("fixture root");
            let canonical_root = root.path().canonicalize().expect("canonical fixture root");
            let service = canonical_root.join("service");
            let lib = canonical_root.join("lib");
            let input = canonical_root.join("input");
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
            let lib_archive_path = canonical_root.join("lib-source.tar");
            let lib_archive = exact_tree_archive::create(
                &lib,
                &lib_revision,
                &lib_archive_path,
                exact_tree_archive::commit_timestamp(&lib, &lib_revision)
                    .expect("Lib commit timestamp"),
            )
            .expect("Lib source archive");
            let catalog =
                fs::read(lib.join("contracts/crates/catalog.v2.toml")).expect("workspace catalog");

            write_file(
                &service.join("Cargo.toml"),
                br#"[package]
name = "fixture-service"
version = "0.1.0-alpha"
edition = "2024"
license = "AGPL-3.0-or-later"

[[bin]]
name = "fixture-service"
path = "src/main.rs"

[workspace]
resolver = "3"

[workspace.metadata.radroots.service_release]
service = "myc"
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
                &service.join("flake.nix"),
                format!(
                    "{{\n  inputs.lib = {{\n    url = \"github:radrootslabs/lib/{lib_revision}\";\n    flake = false;\n  }};\n  outputs = {{ ... }}: {{ }};\n}}\n"
                )
                .as_bytes(),
            );
            write_file(
                &service.join("flake.lock"),
                format!(
                    "{{\"nodes\":{{\"root\":{{\"inputs\":{{\"lib\":\"lib\"}}}},\"lib\":{{\"locked\":{{\"lastModified\":1,\"narHash\":\"sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=\",\"owner\":\"radrootslabs\",\"repo\":\"lib\",\"rev\":\"{lib_revision}\",\"type\":\"github\"}},\"original\":{{\"owner\":\"radrootslabs\",\"repo\":\"lib\",\"rev\":\"{lib_revision}\",\"type\":\"github\"}}}}}},\"root\":\"root\",\"version\":7}}\n"
                )
                .as_bytes(),
            );
            write_file(
                &service.join("LICENSE-APACHE"),
                b"Apache-2.0 fixture license\n",
            );
            write_file(&service.join("LICENSE-MIT"), b"MIT fixture license\n");
            let artifact_contract_path =
                service.join("contracts/release/myc-artifact-contract.v3.json");
            fs::create_dir_all(
                artifact_contract_path
                    .parent()
                    .expect("artifact contract parent"),
            )
            .expect("artifact contract directory");
            write_file(&artifact_contract_path, b"{}\n");
            let cargo_lock = fs::read(service.join("Cargo.lock")).expect("Cargo lock");
            let flake_lock = fs::read(service.join("flake.lock")).expect("flake lock");
            let artifact_contract = fs::read(&artifact_contract_path).expect("artifact contract");
            let source_lock = ServiceSourceLockV3::fixture(FixtureParts {
                service: "myc",
                revision: &lib_revision,
                workspace_catalog_sha256: &sha256_bytes(&catalog),
                source_archive_sha256: &lib_archive.sha256,
                cargo_lock_sha256: &sha256_bytes(&cargo_lock),
                flake_lock_sha256: &sha256_bytes(&flake_lock),
                artifact_contract_sha256: &sha256_bytes(&artifact_contract),
                contract_versions: ContractVersions::new(1, 1, 1, 1, 1),
            })
            .expect("source lock");
            write_file(&service.join(LOCK_FILENAME), source_lock.canonical_bytes());
            initialize_git(&service, "https://github.com/radrootslabs/fixture-service");
            let service_revision = git_output(&service, &["rev-parse", "HEAD"]);
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
            let status = Command::new("rustc")
                .args(["--edition=2024", "src/main.rs", "-o"])
                .arg(input.join("service-binary"))
                .current_dir(&service)
                .status()
                .expect("compile fixture service binary");
            assert!(status.success(), "compile fixture service binary");
            create_oci_fixture(
                &input.join("oci-image.tar.gz"),
                &service_revision,
                &lib_revision,
            );

            Self {
                output_a: canonical_root.join("release-a"),
                output_b: canonical_root.join("release-b"),
                _root: root,
                service,
                lib,
                input,
            }
        }

        fn write(&self, output: &Path) -> Result<(), ReleaseArtifactError> {
            run_inner(Arguments {
                mode: CommandMode::Write,
                service_root: &self.service,
                lib_root: &self.lib,
                input_root: &self.input,
                output_root: output,
                target: native_fixture_target(),
                source_date_epoch: 1_700_000_000,
                candidate_digest: &"a".repeat(64),
            })
        }

        fn check(&self, output: &Path) -> Result<(), ReleaseArtifactError> {
            run_inner(Arguments {
                mode: CommandMode::Check,
                service_root: &self.service,
                lib_root: &self.lib,
                input_root: &self.input,
                output_root: output,
                target: native_fixture_target(),
                source_date_epoch: 1_700_000_000,
                candidate_digest: &"a".repeat(64),
            })
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

    fn native_fixture_target() -> &'static str {
        if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            "aarch64-apple-darwin"
        } else {
            "x86_64-unknown-linux-gnu"
        }
    }

    fn create_oci_fixture(output: &Path, service_revision: &str, lib_revision: &str) {
        let entrypoint =
            "/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-fixture-service/bin/fixture-service";
        let mut layer = Vec::new();
        {
            let mut tar = TarBuilder::new(&mut layer);
            let bytes = b"fixture executable";
            let mut header = TarHeader::new_gnu();
            header.set_size(bytes.len() as u64);
            header.set_mode(0o755);
            header.set_uid(0);
            header.set_gid(0);
            header.set_mtime(0);
            header.set_cksum();
            tar.append_data(
                &mut header,
                entrypoint.trim_start_matches('/'),
                bytes.as_slice(),
            )
            .expect("write layer entrypoint");
            tar.finish().expect("finish layer");
        }
        let layer_digest = sha256_bytes(&layer);
        let layer_name = format!("{layer_digest}/layer.tar");
        let labels = artifact_admission::OciExpectation {
            service: "myc",
            binary_name: "fixture-service",
            version: "0.1.0-alpha",
            service_revision,
            lib_revision,
            license: "AGPL-3.0-or-later",
            contract_versions: artifact_admission::ContractVersions {
                admin: 1,
                config: 1,
                provider: 1,
                state: 1,
                status: 1,
            },
        };
        let labels = artifact_admission::fixture_labels(&labels);
        let config = serde_json::to_vec(&serde_json::json!({
            "architecture": "amd64",
            "config": {
                "Entrypoint": [entrypoint],
                "Env": ["SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt"],
                "Labels": labels,
                "StopSignal": "SIGTERM",
                "User": "65532:65532",
                "WorkingDir": "/"
            },
            "created": "1970-01-01T00:00:01+00:00",
            "os": "linux",
            "rootfs": {"diff_ids": [format!("sha256:{layer_digest}")], "type": "layers"}
        }))
        .expect("serialize config");
        let config_name = format!("{}.json", sha256_bytes(&config));
        let manifest = serde_json::to_vec(&serde_json::json!([{
            "Config": config_name,
            "Layers": [layer_name],
            "RepoTags": ["myc:0.1.0-alpha"]
        }]))
        .expect("serialize manifest");
        let repositories = serde_json::to_vec(&serde_json::json!({
            "myc": {"0.1.0-alpha": layer_digest}
        }))
        .expect("serialize repositories");
        let mut members = vec![
            (config_name, false, config),
            (format!("{layer_digest}/"), true, Vec::new()),
            (format!("{layer_digest}/VERSION"), false, b"1.0".to_vec()),
            (format!("{layer_digest}/json"), false, b"{}".to_vec()),
            (layer_name, false, layer),
            ("manifest.json".to_owned(), false, manifest),
            ("repositories".to_owned(), false, repositories),
        ];
        members.sort_by(|left, right| left.0.as_bytes().cmp(right.0.as_bytes()));
        let output_file = fs::File::create(output).expect("create OCI fixture");
        let encoder = GzBuilder::new()
            .mtime(0)
            .operating_system(255)
            .write(output_file, Compression::best());
        let mut archive = TarBuilder::new(encoder);
        for (name, directory, bytes) in members {
            let mut header = TarHeader::new_gnu();
            header.set_entry_type(if directory {
                tar::EntryType::Directory
            } else {
                tar::EntryType::Regular
            });
            header.set_size(bytes.len() as u64);
            header.set_mode(if directory { 0o755 } else { 0o644 });
            header.set_uid(0);
            header.set_gid(0);
            header.set_mtime(0);
            header.set_cksum();
            archive
                .append_data(&mut header, name, bytes.as_slice())
                .expect("write OCI member");
        }
        let encoder = archive.into_inner().expect("finish OCI fixture tar");
        let file = encoder.finish().expect("finish OCI fixture gzip");
        file.sync_all().expect("sync OCI fixture");
    }

    fn sample_metadata() -> ReleaseMetadata {
        ReleaseMetadata {
            service: "fixture_service".to_owned(),
            service_package: "fixture-service".to_owned(),
            binary_name: "fixture-service".to_owned(),
            version: "0.1.0-alpha".to_owned(),
            license: "AGPL-3.0-or-later".to_owned(),
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
            manifest_path: format!("/fixture/{name}/Cargo.toml"),
            license_file: None,
            targets: if binary {
                vec![CargoTarget {
                    name: name.to_owned(),
                    kind: vec!["bin".to_owned()],
                }]
            } else {
                Vec::new()
            },
            license_texts: source
                .map(|_| {
                    let text = "fixture dependency license text\n".to_owned();
                    vec![DependencyLicenseText {
                        filename: "LICENSE".to_owned(),
                        sha256: sha256_bytes(text.as_bytes()),
                        text,
                    }]
                })
                .unwrap_or_default(),
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
    fn oci_admission_reconciles_manifest_config_and_layer_content() {
        let temporary = TempDir::new().unwrap();
        let root = temporary.path().canonicalize().unwrap();
        let original = root.join("original.tar.gz");
        let revision = "a".repeat(40);
        create_oci_fixture(&original, &revision, &revision);
        let expected = artifact_admission::OciExpectation {
            service: "myc",
            binary_name: "fixture-service",
            version: "0.1.0-alpha",
            service_revision: &revision,
            lib_revision: &revision,
            license: "AGPL-3.0-or-later",
            contract_versions: artifact_admission::ContractVersions {
                admin: 1,
                config: 1,
                provider: 1,
                state: 1,
                status: 1,
            },
        };
        artifact_admission::admit_oci(&original, &root, &expected).unwrap();
        let decoder = flate2::read::GzDecoder::new(fs::File::open(&original).unwrap());
        let mut archive = tar::Archive::new(decoder);
        let mut original_members = BTreeMap::new();
        let mut directories = BTreeSet::new();
        for entry in archive.entries().unwrap() {
            let mut entry = entry.unwrap();
            let name = entry.path().unwrap().to_str().unwrap().to_owned();
            if entry.header().entry_type().is_dir() {
                directories.insert(name.clone());
            }
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).unwrap();
            original_members.insert(name, bytes);
        }
        for case in 0..14 {
            let mut members = original_members.clone();
            let mut manifest: Value = serde_json::from_slice(&members["manifest.json"]).unwrap();
            match case {
                0 => manifest = json!([]),
                1 => manifest[0]["Config"] = json!("wrong.extension"),
                2 => manifest[0]["Config"] = json!("invalid.json"),
                3 => manifest[0]["RepoTags"] = json!(["wrong:tag"]),
                4 => manifest[0]["Layers"] = json!([]),
                5 => manifest[0]["Layers"] = json!(["one", "two", "three"]),
                6 => {
                    manifest[0]["Layers"] = json!([
                        manifest[0]["Layers"][0].clone(),
                        manifest[0]["Layers"][0].clone()
                    ])
                }
                7 => {
                    members
                        .get_mut(manifest[0]["Config"].as_str().unwrap())
                        .unwrap()
                        .push(b' ');
                }
                8..=10 => {
                    let old_name = manifest[0]["Config"].as_str().unwrap().to_owned();
                    let mut config: Value =
                        serde_json::from_slice(&members.remove(&old_name).unwrap()).unwrap();
                    match case {
                        8 => config["rootfs"]["type"] = json!("unknown"),
                        9 => config["rootfs"]["diff_ids"] = json!([]),
                        _ => {
                            config["rootfs"]["diff_ids"] =
                                json!([format!("sha256:{}", "0".repeat(64))])
                        }
                    }
                    let bytes = serde_json::to_vec(&config).unwrap();
                    let name = format!("{}.json", sha256_bytes(&bytes));
                    members.insert(name.clone(), bytes);
                    manifest[0]["Config"] = json!(name);
                }
                11 => {
                    members.remove(manifest[0]["Layers"][0].as_str().unwrap());
                }
                12 => {
                    members.insert("unexpected".to_owned(), b"extra".to_vec());
                }
                _ => {}
            }
            members.insert(
                "manifest.json".to_owned(),
                serde_json::to_vec(&manifest).unwrap(),
            );
            let path = root.join(format!("invalid-{case}.tar.gz"));
            let encoder = GzBuilder::new()
                .mtime(0)
                .operating_system(255)
                .write(fs::File::create(&path).unwrap(), Compression::best());
            let mut archive = TarBuilder::new(encoder);
            for (name, bytes) in members {
                let mut header = TarHeader::new_gnu();
                let directory = directories.contains(&name);
                header.set_entry_type(if directory {
                    tar::EntryType::Directory
                } else {
                    tar::EntryType::Regular
                });
                header.set_size(bytes.len() as u64);
                header.set_mode(if directory { 0o755 } else { 0o644 });
                header.set_uid(0);
                header.set_gid(0);
                header.set_mtime(0);
                header.set_cksum();
                archive
                    .append_data(&mut header, name, bytes.as_slice())
                    .unwrap();
            }
            archive.into_inner().unwrap().finish().unwrap();
            let admitted = artifact_admission::admit_oci(&path, &root, &expected);
            if case == 13 {
                admitted.expect("rewritten unmodified OCI fixture remains valid");
            } else {
                assert!(admitted.is_err(), "OCI case {case}");
            }
        }
        for case in 0..5 {
            let mut changed = artifact_admission::OciExpectation { ..expected };
            match case {
                0 => changed.license = "MIT",
                1 => changed.service = "../escape",
                2 => changed.binary_name = "../escape",
                3 => changed.service_revision = "invalid",
                _ => changed.lib_revision = "invalid",
            }
            assert!(artifact_admission::admit_oci(&original, &root, &changed).is_err());
        }
    }

    #[test]
    fn dependency_license_admission_preserves_exact_text_and_rejects_missing_or_unsafe_paths() {
        let temporary = TempDir::new().unwrap();
        let root = temporary.path().canonicalize().unwrap();
        let mut dependency = package(
            "fixture",
            "fixture",
            Some("registry+https://github.com/rust-lang/crates.io-index"),
            Some(&"a".repeat(64)),
            Some("MIT"),
            false,
        );
        dependency.manifest_path = root.join("Cargo.toml").to_str().unwrap().to_owned();
        fs::write(root.join("Cargo.toml"), b"fixture").unwrap();
        assert!(dependency_license_texts(&dependency).is_err());
        let text = "Fixture copyright\nPermission is granted.\n";
        fs::write(root.join("LICENSE"), text).unwrap();
        fs::write(root.join("COPYING-MIT"), "Copying terms\n").unwrap();
        fs::write(root.join("NOTICE.txt"), "Notice terms\n").unwrap();
        let texts = dependency_license_texts(&dependency).unwrap();
        assert_eq!(
            texts
                .iter()
                .map(|row| row.filename.as_str())
                .collect::<Vec<_>>(),
            ["COPYING-MIT", "LICENSE", "NOTICE.txt"]
        );
        assert_eq!(texts[1].text, text);
        assert_eq!(texts[1].sha256, sha256_bytes(text.as_bytes()));
        dependency.license_file = Some("LICENSE".to_owned());
        assert_eq!(dependency_license_texts(&dependency).unwrap().len(), 1);
        for path in ["../outside", "/absolute", "./LICENSE", "missing"] {
            dependency.license_file = Some(path.to_owned());
            assert!(dependency_license_texts(&dependency).is_err(), "{path}");
        }
        for path in [
            "relative/Cargo.toml".to_owned(),
            root.join("not-a-manifest").to_str().unwrap().to_owned(),
        ] {
            dependency.manifest_path = path;
            assert!(dependency_license_texts(&dependency).is_err());
        }
    }

    #[test]
    fn dependency_license_content_and_inventory_are_bounded() {
        let temporary = TempDir::new().unwrap();
        let root = temporary.path().canonicalize().unwrap();
        let mut dependency = package(
            "fixture",
            "fixture",
            Some("registry+https://github.com/rust-lang/crates.io-index"),
            Some(&"a".repeat(64)),
            Some("MIT"),
            false,
        );
        dependency.manifest_path = root.join("Cargo.toml").to_str().unwrap().to_owned();
        dependency.license_file = Some("LICENSE".to_owned());
        for bytes in [
            b" \n\t".as_slice(),
            &[255, 254],
            b"-----BEGIN PRIVATE KEY-----\nAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\n-----END PRIVATE KEY-----",
        ] {
            fs::write(root.join("LICENSE"), bytes).unwrap();
            assert!(dependency_license_texts(&dependency).is_err());
        }
        let file = fs::File::create(root.join("LICENSE")).unwrap();
        file.set_len(MAX_TEXT_INPUT_BYTES + 1).unwrap();
        assert!(dependency_license_texts(&dependency).is_err());
        drop(file);
        fs::write(root.join("LICENSE"), b"Valid terms\n").unwrap();
        dependency.license_file = None;
        for index in 0..16 {
            fs::write(root.join(format!("LICENSE-{index}")), b"Terms\n").unwrap();
        }
        assert!(dependency_license_texts(&dependency).is_err());
        for index in 0..256 {
            fs::write(root.join(format!("ordinary-{index}")), b"").unwrap();
        }
        assert!(dependency_license_texts(&dependency).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn dependency_license_paths_cannot_escape_the_package_root() {
        use std::os::unix::fs::symlink;
        let temporary = TempDir::new().unwrap();
        let root = temporary.path().canonicalize().unwrap();
        let package_root = root.join("package");
        fs::create_dir(&package_root).unwrap();
        let mut dependency = package(
            "fixture",
            "fixture",
            Some("registry+https://github.com/rust-lang/crates.io-index"),
            Some(&"a".repeat(64)),
            Some("MIT"),
            false,
        );
        dependency.manifest_path = package_root.join("Cargo.toml").to_str().unwrap().to_owned();
        fs::write(root.join("outside"), b"Outside terms\n").unwrap();
        symlink(root.join("outside"), package_root.join("LICENSE")).unwrap();
        assert!(dependency_license_texts(&dependency).is_err());
        fs::create_dir(package_root.join("nested")).unwrap();
        fs::write(package_root.join("nested/terms"), b"Nested terms\n").unwrap();
        dependency.license_file = Some("nested/terms".to_owned());
        assert!(dependency_license_texts(&dependency).is_err());
        assert_eq!(fs::read(root.join("outside")).unwrap(), b"Outside terms\n");
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
            ("/contract_version", serde_json::json!(1)),
            ("/decision_state", serde_json::json!("draft")),
            ("/owner_step", serde_json::json!(1)),
            ("/predecessor/schema", serde_json::json!("other")),
            ("/predecessor/filename", serde_json::json!("other")),
            ("/predecessor/transition", serde_json::json!("other")),
            ("/command", serde_json::json!("other")),
            ("/modes", serde_json::json!([])),
            ("/required_arguments", serde_json::json!([])),
            ("/service_metadata_path", serde_json::json!("other")),
            ("/service_metadata_fields", serde_json::json!([])),
            ("/service_license_path", serde_json::json!("other")),
            ("/source_lock_schema", serde_json::json!("other")),
            ("/source_lock_definition", serde_json::json!("other")),
            ("/artifact_contract_binding", serde_json::json!("other")),
            ("/artifact_admission_contract", serde_json::json!("other")),
            ("/supported_targets", serde_json::json!([])),
            ("/candidate_binding", serde_json::json!("other")),
            ("/lib_root_binding", serde_json::json!("other")),
            ("/binary_admission", serde_json::json!("other")),
            ("/oci_admission", serde_json::json!("other")),
            ("/input_inventory", serde_json::json!([])),
            ("/excluded_parent_owned_inputs", serde_json::json!([])),
            ("/service_root_inventory", serde_json::json!([])),
            ("/output_inventory", serde_json::json!([])),
            ("/canonical_json", serde_json::json!("other")),
            ("/checksum_format", serde_json::json!("other")),
            ("/source_archive_format", serde_json::json!("other")),
            ("/sbom_format", serde_json::json!("other")),
            ("/license_evidence", serde_json::json!("other")),
            ("/provenance_posture", serde_json::json!("other")),
            ("/protected_material_scan_scope", serde_json::json!("other")),
            ("/confidentiality_state", serde_json::json!("other")),
            ("/source_cleanliness", serde_json::json!("other")),
            ("/revision_stability", serde_json::json!("other")),
            ("/no_protected_material", serde_json::json!(false)),
            ("/maximums/text_input_bytes", serde_json::json!(1)),
            ("/maximums/generated_document_bytes", serde_json::json!(1)),
            ("/maximums/service_cargo_lock_bytes", serde_json::json!(1)),
            ("/maximums/service_flake_lock_bytes", serde_json::json!(1)),
            ("/maximums/binary_bytes", serde_json::json!(1)),
            ("/maximums/source_archive_bytes", serde_json::json!(1)),
            (
                "/maximums/source_archive_member_bytes",
                serde_json::json!(1),
            ),
            ("/maximums/source_archive_members", serde_json::json!(1)),
            ("/maximums/oci_bytes", serde_json::json!(1)),
            (
                "/maximums/artifact_scan_expanded_bytes",
                serde_json::json!(1),
            ),
            ("/maximums/cargo_metadata_bytes", serde_json::json!(1)),
            ("/maximums/packages", serde_json::json!(1)),
            ("/maximums/workspace_packages", serde_json::json!(1)),
            ("/required_negative_vectors", serde_json::json!([])),
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
        assert_eq!(INPUT_NAMES.len(), 6);
        assert_eq!(OUTPUT_NAMES.len(), 20);
        assert_eq!(MAX_TEXT_INPUT_BYTES, 1_048_576);
        assert_eq!(MAX_GENERATED_DOCUMENT_BYTES, 16_777_216);
        assert_eq!(MAX_SERVICE_CARGO_LOCK_BYTES, 16_777_216);
        assert_eq!(MAX_SERVICE_FLAKE_LOCK_BYTES, 4_194_304);
        assert_eq!(MAX_BINARY_BYTES, 536_870_912);
        assert_eq!(MAX_SOURCE_ARCHIVE_BYTES, 1_073_741_824);
        assert_eq!(MAX_SOURCE_ARCHIVE_MEMBER_BYTES, 67_108_864);
        assert_eq!(MAX_SOURCE_ARCHIVE_MEMBERS, 65_536);
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
        let (sbom, notices, licenses) =
            build_supply_chain_documents(&sample_metadata(), cargo).expect("documents");
        let bytes = serde_json::to_vec(&sbom).expect("SBOM JSON");
        assert_eq!(serde_json::to_vec(&sbom).expect("SBOM JSON"), bytes);
        assert_eq!(sbom.bom_format, "CycloneDX");
        assert_eq!(sbom.spec_version, "1.6");
        assert_eq!(sbom.metadata.component.name, "fixture-service");
        assert_eq!(sbom.components.len(), 1);
        assert_eq!(sbom.dependencies.len(), 2);
        assert!(notices.contains("Package: dependency 0.1.0-alpha"));
        assert!(notices.contains("License: Apache-2.0"));
        assert!(notices.contains("registry+https://github.com/rust-lang/crates.io-index"));
        assert!(notices.contains("License-Text: LICENSE sha256:"));
        assert!(licenses.contains("fixture dependency license text"));
    }

    #[test]
    fn license_documents_require_exact_bounded_nonempty_text_and_safe_filenames() {
        build_supply_chain_documents(&sample_metadata(), sample_cargo_metadata()).unwrap();
        for case in ["missing", "filename", "empty", "oversized"] {
            let mut changed = sample_cargo_metadata();
            let package = &mut changed.packages[1];
            match case {
                "missing" => package.license_texts.clear(),
                "filename" => package.license_texts[0].filename = "../LICENSE".into(),
                "empty" => package.license_texts[0].text = " \n".into(),
                "oversized" => {
                    package.license_texts[0].text =
                        "x".repeat(MAX_GENERATED_DOCUMENT_BYTES as usize + 1)
                }
                _ => unreachable!(),
            }
            if let Some(text) = package.license_texts.first_mut() {
                text.sha256 = sha256_bytes(text.text.as_bytes());
            }
            assert!(
                matches!(
                    build_supply_chain_documents(&sample_metadata(), changed),
                    Err(ReleaseArtifactError::InvalidPackageInventory)
                ),
                "{case}"
            );
        }
        let mut without_newline = sample_cargo_metadata();
        let text = &mut without_newline.packages[1].license_texts[0];
        text.text = "Exact license terms".into();
        text.sha256 = sha256_bytes(text.text.as_bytes());
        let (_, _, document) =
            build_supply_chain_documents(&sample_metadata(), without_newline).unwrap();
        assert!(document.ends_with("Exact license terms\n"));
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
        let (_, notices, licenses) =
            build_supply_chain_documents(&sample_metadata(), root_only).expect("root-only graph");
        assert!(notices.contains("No third-party Cargo packages are present."));
        assert!(licenses.contains("No third-party Cargo packages are present."));
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

    #[cfg(unix)]
    #[test]
    fn snapshot_backed_inventory_rejects_file_and_root_mode_races() {
        use std::os::unix::fs::PermissionsExt as _;

        let directory = TempDir::new().expect("inventory fixture");
        let root = directory
            .path()
            .canonicalize()
            .expect("canonical inventory fixture");
        set_directory_mode(&root).expect("directory mode");
        let file = root.join("LICENSE-MIT");
        write_file(&file, b"license\n");
        set_file_mode(&file).expect("file mode");

        assert_eq!(
            inventory_records_impl(&root, || {
                fs::set_permissions(&file, fs::Permissions::from_mode(0o600))
                    .expect("change file mode");
            }),
            Err(ReleaseArtifactError::GenerationFailure)
        );
        set_file_mode(&file).expect("restore file mode");
        assert_eq!(
            inventory_records_impl(&root, || {
                fs::set_permissions(&root, fs::Permissions::from_mode(0o700))
                    .expect("change root mode");
            }),
            Err(ReleaseArtifactError::GenerationFailure)
        );
        set_directory_mode(&root).expect("restore directory mode");
        fs::set_permissions(&file, fs::Permissions::from_mode(0o4644))
            .expect("add file special mode bit");
        assert_eq!(
            inventory_records(&root),
            Err(ReleaseArtifactError::GenerationFailure)
        );
        set_file_mode(&file).expect("restore file mode");
        fs::set_permissions(&root, fs::Permissions::from_mode(0o1755))
            .expect("add directory special mode bit");
        assert_eq!(
            inventory_records(&root),
            Err(ReleaseArtifactError::GenerationFailure)
        );
    }

    #[test]
    fn remaining_release_boundaries_fail_closed() {
        let fixture = ReleaseFixture::new();
        assert_eq!(
            fixture.check(&fixture.output_a),
            Err(ReleaseArtifactError::StaleOutput)
        );

        let source_lock = ServiceSourceLockV3::from_canonical_bytes(
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
            write_generated(
                &scope.path().join("oversized-generated"),
                &vec![b'x'; MAX_GENERATED_DOCUMENT_BYTES as usize + 1]
            ),
            Err(ReleaseArtifactError::GenerationFailure)
        );
    }

    #[test]
    fn low_level_release_admission_and_comparison_branches_are_qualified() {
        let root = TempDir::new().expect("low-level release fixture");
        let regular = root.path().join("regular");
        write_file(&regular, b"same");
        assert_eq!(
            read_bounded_regular(&regular, 4, ReleaseArtifactError::InvalidInputArtifact)
                .expect("bounded regular file"),
            b"same"
        );
        let directory = root.path().join("directory");
        fs::create_dir(&directory).expect("directory fixture");
        assert_eq!(
            read_bounded_regular(&directory, 4, ReleaseArtifactError::InvalidInputArtifact),
            Err(ReleaseArtifactError::InvalidInputArtifact)
        );
        let oversized = root.path().join("oversized");
        write_file(&oversized, b"12345");
        assert_eq!(
            read_bounded_regular(&oversized, 4, ReleaseArtifactError::InvalidInputArtifact),
            Err(ReleaseArtifactError::InvalidInputArtifact)
        );

        #[cfg(unix)]
        {
            use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};

            let symlink = root.path().join("regular-link");
            std::os::unix::fs::symlink(&regular, &symlink).expect("regular symlink");
            assert_eq!(
                read_bounded_regular(&symlink, 4, ReleaseArtifactError::InvalidInputArtifact),
                Err(ReleaseArtifactError::InvalidInputArtifact)
            );

            let expected = fs::symlink_metadata(&regular).expect("regular metadata");
            assert_eq!(
                validate_unchanged_input(&symlink, &expected),
                Err(ReleaseArtifactError::InvalidInputArtifact)
            );
            assert_eq!(
                validate_unchanged_input(&directory, &expected),
                Err(ReleaseArtifactError::InvalidInputArtifact)
            );

            let other = root.path().join("other");
            write_file(&other, b"same");
            assert_eq!(
                validate_unchanged_input(&other, &expected),
                Err(ReleaseArtifactError::InvalidInputArtifact)
            );

            let device_metadata = fs::symlink_metadata("/dev/null").expect("device metadata");
            assert_ne!(device_metadata.dev(), expected.dev());
            assert_eq!(
                validate_unchanged_input(&regular, &device_metadata),
                Err(ReleaseArtifactError::InvalidInputArtifact)
            );

            let same_inode = fs::symlink_metadata(&regular).expect("same-inode metadata");
            write_file(&regular, b"changed length");
            assert_eq!(
                validate_unchanged_input(&regular, &same_inode),
                Err(ReleaseArtifactError::InvalidInputArtifact)
            );

            let inventory = root.path().join("symlink-inventory");
            fs::create_dir(&inventory).expect("symlink inventory");
            std::os::unix::fs::symlink(&other, inventory.join("entry")).expect("inventory symlink");
            assert_eq!(
                directory_inventory(&inventory, ReleaseArtifactError::InvalidInputRoot),
                Err(ReleaseArtifactError::InvalidInputRoot)
            );

            let mode_file = root.path().join("mode-file");
            write_file(&mode_file, b"mode");
            fs::set_permissions(&mode_file, fs::Permissions::from_mode(0o600))
                .expect("set invalid file mode");
            assert_eq!(
                validate_file_mode(&mode_file),
                Err(ReleaseArtifactError::StaleOutput)
            );
            fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
                .expect("set invalid directory mode");
            assert_eq!(
                validate_directory_mode(&directory),
                Err(ReleaseArtifactError::StaleOutput)
            );
        }

        let incomplete = root.path().join("incomplete-output");
        fs::create_dir(&incomplete).expect("incomplete output");
        write_file(&incomplete.join("LICENSE-MIT"), b"x");
        assert_eq!(
            validate_exact_output_inventory(&incomplete),
            Err(ReleaseArtifactError::GenerationFailure)
        );

        let expected = root.path().join("expected-output");
        let actual = root.path().join("actual-output");
        fs::create_dir(&expected).expect("expected output");
        fs::create_dir(&actual).expect("actual output");
        set_directory_mode(&expected).expect("expected directory mode");
        set_directory_mode(&actual).expect("actual directory mode");
        for name in OUTPUT_NAMES {
            write_file(&expected.join(name), b"a");
            write_file(&actual.join(name), b"a");
            set_file_mode(&expected.join(name)).expect("expected file mode");
            set_file_mode(&actual.join(name)).expect("actual file mode");
        }
        compare_output(&expected, &actual).expect("matching output");
        let records = inventory_records(&actual).expect("actual records");
        validate_output_records(&actual, &records).expect("matching records");
        write_file(&actual.join("LICENSE-MIT"), b"b");
        set_file_mode(&actual.join("LICENSE-MIT")).expect("restored file mode");
        assert_eq!(
            compare_output(&expected, &actual),
            Err(ReleaseArtifactError::StaleOutput)
        );

        let mut oversized_stdout = Command::new("sh");
        oversized_stdout.args(["-c", "printf 12345"]);
        assert_eq!(command_stdout(&mut oversized_stdout, 4), Err(()));
        let mut failed_stdout = Command::new("sh");
        failed_stdout.args(["-c", "exit 7"]);
        assert_eq!(command_stdout(&mut failed_stdout, 4), Err(()));
    }

    #[test]
    fn release_service_and_workspace_binding_fail_closed() {
        let fixture = ReleaseFixture::new();
        let mismatched = fs::read_to_string(fixture.service.join(LOCK_FILENAME))
            .expect("source lock")
            .replace("service = \"myc\"", "service = \"rhi\"");
        write_file(&fixture.service.join(LOCK_FILENAME), mismatched.as_bytes());
        git(&fixture.service, &["add", LOCK_FILENAME]);
        git(
            &fixture.service,
            &["commit", "--quiet", "-m", "mismatched service lock"],
        );
        assert_eq!(
            fixture.check(&fixture.output_a),
            Err(ReleaseArtifactError::InvalidSourceLock)
        );

        let mut cargo = sample_cargo_metadata();
        let dependency_id = cargo.packages[1].id.clone();
        cargo.packages[0].source =
            Some("registry+https://github.com/rust-lang/crates.io-index".into());
        cargo.packages[0].checksum = Some("a".repeat(64));
        cargo.workspace_members = vec![dependency_id];
        assert!(matches!(
            build_supply_chain_documents(&sample_metadata(), cargo),
            Err(ReleaseArtifactError::InvalidPackageInventory)
        ));
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
        let root = root
            .path()
            .canonicalize()
            .expect("canonical archive fixture");
        let source = root.join("service");
        let first = root.join("first.tar.gz");
        let second = root.join("second.tar.gz");
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
            "artifact-manifest.v2.json",
            "artifact-scan.v1.json",
            "oci-image.v1.json",
            "provenance.intoto.jsonl",
            "sbom.cdx.json",
            "source-archives.v3.json",
        ] {
            let bytes = fs::read(fixture.output_a.join(name)).expect("JSON output");
            assert_eq!(bytes.last(), Some(&b'\n'));
            assert_eq!(bytes.iter().filter(|byte| **byte == b'\n').count(), 1);
            serde_json::from_slice::<serde_json::Value>(&bytes).expect("valid JSON");
        }
        let provenance: serde_json::Value = serde_json::from_slice(
            &fs::read(fixture.output_a.join("provenance.intoto.jsonl")).expect("provenance"),
        )
        .expect("provenance JSON");
        assert_eq!(provenance["_type"], "https://in-toto.io/Statement/v1");
        assert_eq!(
            provenance["predicateType"],
            "https://slsa.dev/provenance/v1"
        );
        assert_eq!(
            provenance["predicate"]["buildDefinition"]["externalParameters"]["candidate_digest"],
            format!("sha256:{}", "a".repeat(64))
        );

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
    fn qualified_nix_material_is_required_and_preserved() {
        let fixture = ReleaseFixture::new();
        fixture
            .write(&fixture.output_a)
            .expect("qualified-Nix release");

        let document: serde_json::Value = serde_json::from_slice(
            &fs::read(fixture.output_a.join("source-archives.v3.json"))
                .expect("source archive document"),
        )
        .expect("source archive JSON");
        assert_eq!(document["schema"], "radroots.service.source-archives.v3");
        assert_eq!(document["contract_version"], 3);
        assert!(valid_lower_hex(
            document["flake_lock_sha256"]
                .as_str()
                .expect("flake digest"),
            64
        ));

        let lock = ServiceSourceLockV3::from_canonical_bytes(
            &fs::read(fixture.service.join(LOCK_FILENAME)).expect("source lock"),
        )
        .expect("valid source lock");
        for name in ["flake.nix", "flake.lock"] {
            let path = fixture.service.join(name);
            let original = fs::read(&path).expect("qualified Nix file");
            fs::remove_file(&path).expect("remove qualified Nix file");
            assert_eq!(
                validate_source_lock_files(&fixture.service, &lock),
                Err(ReleaseArtifactError::InvalidSourceLock),
                "accepted missing qualified-Nix file {name}"
            );
            write_file(&path, &original);
        }
        write_file(
            &fixture.service.join(PREDECESSOR_LOCK_FILENAME),
            b"unexpected",
        );
        assert_eq!(
            validate_source_lock_files(&fixture.service, &lock),
            Err(ReleaseArtifactError::InvalidSourceLock)
        );
    }

    #[test]
    fn protected_text_and_invalid_inventory_fail_closed() {
        let fixture = ReleaseFixture::new();
        let mut secret = b"-----BEGIN PRIVATE KEY-----\n".to_vec();
        secret.extend_from_slice(&[b'A'; 48]);
        secret.extend_from_slice(b"\n-----END PRIVATE KEY-----\n");
        write_file(&fixture.input.join("config.example.toml"), &secret);
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
    fn source_lock_and_source_archive_drift_fail_closed() {
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
        let current = ServiceSourceLockV3::from_canonical_bytes(
            &fs::read(fixture.service.join(LOCK_FILENAME)).expect("source lock"),
        )
        .expect("source lock");
        let drifted = String::from_utf8(current.canonical_bytes().to_vec())
            .expect("UTF-8 lock")
            .replace(current.source_archive_sha256(), &"0".repeat(64));
        write_file(&fixture.service.join(LOCK_FILENAME), drifted.as_bytes());
        git(&fixture.service, &["add", LOCK_FILENAME]);
        git(
            &fixture.service,
            &["commit", "--quiet", "-m", "drift source archive digest"],
        );
        create_oci_fixture(
            &fixture.input.join("oci-image.tar.gz"),
            &git_output(&fixture.service, &["rev-parse", "HEAD"]),
            current.revision(),
        );
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
            run_inner(Arguments {
                mode: CommandMode::Write,
                service_root: &fixture.service,
                lib_root: &fixture.lib,
                input_root: &fixture.input,
                output_root: &fixture.output_a,
                target: "x86_64-apple-darwin",
                source_date_epoch: 1_700_000_000,
                candidate_digest: &"a".repeat(64),
            }),
            Err(ReleaseArtifactError::InvalidServiceMetadata)
        );
        assert_eq!(
            run_inner(Arguments {
                mode: CommandMode::Write,
                service_root: &fixture.service,
                lib_root: &fixture.lib,
                input_root: &fixture.input,
                output_root: &fixture.output_a,
                target: "x86_64-unknown-linux-gnu",
                source_date_epoch: 0,
                candidate_digest: &"a".repeat(64),
            }),
            Err(ReleaseArtifactError::InvalidServiceMetadata)
        );
        assert_eq!(
            run_inner(Arguments {
                mode: CommandMode::Write,
                service_root: &fixture.service,
                lib_root: &fixture.lib,
                input_root: &fixture.input,
                output_root: &fixture.output_a,
                target: native_fixture_target(),
                source_date_epoch: 1_700_000_000,
                candidate_digest: &"A".repeat(64),
            }),
            Err(ReleaseArtifactError::InvalidServiceMetadata)
        );
    }

    #[test]
    fn secret_scanner_detects_a_pattern_across_chunk_boundaries() {
        let mut scanner = SecretScanner::default();
        scanner.scan(b"prefix -----BEGIN OPENSSH").expect("prefix");
        assert_eq!(
            scanner.scan(
                b" PRIVATE KEY-----\nAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\n-----END OPENSSH PRIVATE KEY----- suffix"
            ),
            Err(ReleaseArtifactError::ProtectedMaterialDetected)
        );
    }

    #[test]
    fn release_metadata_rejects_well_typed_but_invalid_identifiers_and_versions() {
        let temporary = TempDir::new().unwrap();
        let original: toml::Value = toml::from_str(
            r#"
            [workspace.package]
            license = "AGPL-3.0-or-later"
            [workspace.metadata.radroots.service_release]
            service = "myc"
            service_package = "fixture-service"
            binary_name = "fixture-service"
            version = "0.1.0-alpha"
        "#,
        )
        .unwrap();
        let manifest = temporary.path().join("Cargo.toml");
        fs::write(&manifest, toml::to_string(&original).unwrap()).unwrap();
        read_release_metadata(temporary.path()).unwrap();
        for (field, replacement) in [
            ("service", "Invalid".to_owned()),
            ("service_package", "Invalid".to_owned()),
            ("binary_name", "../escape".to_owned()),
            ("version", "x".repeat(129)),
            ("version", "invalid".to_owned()),
            ("version", "01.0.0".to_owned()),
        ] {
            let mut value = original.clone();
            value["workspace"]["metadata"]["radroots"]["service_release"][field] =
                toml::Value::String(replacement);
            fs::write(&manifest, toml::to_string(&value).unwrap()).unwrap();
            assert!(matches!(
                read_release_metadata(temporary.path()),
                Err(ReleaseArtifactError::InvalidServiceMetadata)
            ));
        }
        let mut value = original;
        value["workspace"]["package"]["license"] = toml::Value::String("MIT".to_owned());
        fs::write(&manifest, toml::to_string(&value).unwrap()).unwrap();
        assert!(matches!(
            read_release_metadata(temporary.path()),
            Err(ReleaseArtifactError::InvalidServiceMetadata)
        ));
    }

    #[test]
    fn sbom_dependency_edges_composition_and_artifact_hashes_are_exact() {
        let artifacts = vec![ArtifactRecord {
            path: "binary.tar.gz".to_owned(),
            byte_length: 42,
            sha256: "a".repeat(64),
        }];
        for case in 0..7 {
            let (mut sbom, _, _) =
                build_supply_chain_documents(&sample_metadata(), sample_cargo_metadata()).unwrap();
            reconcile_sbom_artifacts(&mut sbom, &artifacts);
            validate_cyclonedx_profile(&sbom, &artifacts).unwrap();
            match case {
                0 => sbom.components[0].bom_ref = sbom.metadata.component.bom_ref.clone(),
                1 => {
                    sbom.dependencies.pop();
                }
                2 => sbom.dependencies[0].reference = "unknown".to_owned(),
                3 => sbom.dependencies[0].depends_on.push("unknown".to_owned()),
                4 => sbom.compositions[0].aggregate = "incomplete",
                5 => sbom.compositions[0].assemblies.clear(),
                _ => {
                    let artifact = sbom
                        .components
                        .iter_mut()
                        .find(|row| row.bom_ref.starts_with("artifact:"))
                        .unwrap();
                    artifact.hashes[0].content = "b".repeat(64);
                }
            }
            assert_eq!(
                validate_cyclonedx_profile(&sbom, &artifacts),
                Err(ReleaseArtifactError::GenerationFailure),
                "SBOM case {case}"
            );
        }
    }

    #[test]
    fn schema_reconciliation_attribution_and_subjects_fail_closed() {
        let (mut sbom, _, _) =
            build_supply_chain_documents(&sample_metadata(), sample_cargo_metadata())
                .expect("supply-chain documents");
        let artifacts = vec![ArtifactRecord {
            path: "binary.tar.gz".to_owned(),
            byte_length: 42,
            sha256: "a".repeat(64),
        }];
        reconcile_sbom_artifacts(&mut sbom, &artifacts);
        validate_cyclonedx_profile(&sbom, &artifacts).expect("CycloneDX profile");
        sbom.spec_version = "1.5";
        assert_eq!(
            validate_cyclonedx_profile(&sbom, &artifacts),
            Err(ReleaseArtifactError::GenerationFailure)
        );
        sbom.spec_version = "1.6";
        sbom.components.pop();
        assert_eq!(
            validate_cyclonedx_profile(&sbom, &artifacts),
            Err(ReleaseArtifactError::GenerationFailure)
        );

        let mut cargo = sample_cargo_metadata();
        cargo.packages[1].license_texts[0].sha256 = "b".repeat(64);
        assert!(matches!(
            build_supply_chain_documents(&sample_metadata(), cargo),
            Err(ReleaseArtifactError::InvalidPackageInventory)
        ));

        let mut provenance = build_provenance(
            ProvenanceInput {
                candidate_digest: &"a".repeat(64),
                service: "myc",
                target: native_fixture_target(),
                source_date_epoch: 1_700_000_000,
                service_repository: "https://github.com/radrootslabs/mycelium",
                service_revision: &"1".repeat(40),
                lib_revision: &"2".repeat(40),
                source_lock_sha256: &"3".repeat(64),
                manifest_sha256: &"4".repeat(64),
            },
            &artifacts,
        );
        validate_provenance_subjects(&provenance, &"a".repeat(64), &artifacts)
            .expect("exact provenance subjects");
        provenance.statement_type = "unknown";
        assert!(validate_provenance_subjects(&provenance, &"a".repeat(64), &artifacts).is_err());
        provenance.statement_type = "https://in-toto.io/Statement/v1";
        provenance.predicate_type = "unknown";
        assert!(validate_provenance_subjects(&provenance, &"a".repeat(64), &artifacts).is_err());
        provenance.predicate_type = "https://slsa.dev/provenance/v1";
        assert!(validate_provenance_subjects(&provenance, &"b".repeat(64), &artifacts).is_err());
        provenance.subject.clear();
        assert_eq!(
            validate_provenance_subjects(&provenance, &"a".repeat(64), &artifacts),
            Err(ReleaseArtifactError::GenerationFailure)
        );

        let scan_evidence = FileEvidence {
            byte_length: 7,
            sha256: "5".repeat(64),
        };
        let scan = ScanDocument {
            schema: "radroots.service.artifact-scan.v1",
            contract_version: 1,
            candidate_digest: "a".repeat(64),
            state: "no_protected_material_detected",
            ruleset_sha256: "6".repeat(64),
            scanned_artifacts: artifacts.clone(),
            nested_members_scanned: 1,
            expanded_bytes_scanned: 42,
        };
        for case in 0..6 {
            let mut manifest = ArtifactManifestDocument {
                schema: "radroots.service.release-artifacts.v2",
                contract_version: 2,
                candidate_digest: "a".repeat(64),
                service: "myc".to_owned(),
                version: "0.1.0-alpha".to_owned(),
                target: native_fixture_target().to_owned(),
                source_date_epoch: 1_700_000_000,
                service_revision: "1".repeat(40),
                lib_revision: "2".repeat(40),
                rust_version: "1.97.1",
                host_feature_profile: "service-host",
                contract_versions: ContractVersionsDocument {
                    config: 1,
                    state: 1,
                    admin: 1,
                    status: 1,
                    provider: 1,
                },
                confidentiality: ConfidentialityDocument {
                    state: scan.state,
                    derived_from: artifact_record("artifact-scan.v1.json", &scan_evidence),
                    protected_material_included: false,
                },
                artifacts: artifacts.clone(),
            };
            validate_confidentiality_binding(&manifest, &scan, &scan_evidence, &artifacts)
                .expect("derived confidentiality");
            match case {
                0 => manifest.confidentiality.state = "invented_clean_state",
                1 => manifest.candidate_digest = "b".repeat(64),
                2 => manifest.confidentiality.protected_material_included = true,
                3 => manifest.confidentiality.derived_from.path = "unknown".to_owned(),
                4 => manifest.confidentiality.derived_from.sha256 = "b".repeat(64),
                _ => manifest.artifacts.clear(),
            }
            assert_eq!(
                validate_confidentiality_binding(&manifest, &scan, &scan_evidence, &artifacts),
                Err(ReleaseArtifactError::GenerationFailure)
            );
        }
    }

    #[test]
    fn secret_path_and_history_archive_vectors_fail_closed() {
        let root = TempDir::new().expect("archive vector root");
        let mut token = b"ghp_".to_vec();
        token.extend(std::iter::repeat_n(b'A', 36));

        let source = root.path().join("source.tar");
        write_tar_fixture(&source, "src/value.bin", &token, false);
        assert_eq!(
            scan_tar_members(&source, false),
            Err(ReleaseArtifactError::ProtectedMaterialDetected)
        );

        let sensitive = root.path().join("sensitive.tar");
        write_tar_fixture(&sensitive, ".git/config", b"clean", false);
        assert_eq!(
            scan_tar_members(&sensitive, false),
            Err(ReleaseArtifactError::ProtectedMaterialDetected)
        );

        let binary = root.path().join("binary.tar.gz");
        write_tar_fixture(&binary, "bin/service", &token, true);
        assert_eq!(
            scan_tar_gzip_members(&binary),
            Err(ReleaseArtifactError::ProtectedMaterialDetected)
        );

        let layer = root.path().join("layer.tar");
        write_tar_fixture(&layer, "nix/store/service", &token, false);
        assert_eq!(
            scan_tar_members(&layer, true),
            Err(ReleaseArtifactError::ProtectedMaterialDetected)
        );

        let bundle = root.path().join("history.bundle");
        write_file(&bundle, b"not an exact-tree archive");
        assert_eq!(
            scan_tar_members(&bundle, false),
            Err(ReleaseArtifactError::InvalidInputArtifact)
        );
    }

    #[test]
    fn archive_scanning_enforces_member_types_counts_lengths_and_protected_paths() {
        let root = TempDir::new().unwrap();
        let path = root.path().join("input.tar");
        for kind in [
            tar::EntryType::Directory,
            tar::EntryType::Symlink,
            tar::EntryType::Link,
            tar::EntryType::Fifo,
        ] {
            let mut builder = TarBuilder::new(Vec::new());
            let mut header = TarHeader::new_gnu();
            header.set_path("member").unwrap();
            header.set_entry_type(kind);
            header.set_size(0);
            if kind.is_symlink() || kind.is_hard_link() {
                header.set_link_name("target").unwrap();
            }
            header.set_cksum();
            builder.append(&header, &[][..]).unwrap();
            let bytes = builder.into_inner().unwrap();
            fs::write(&path, &bytes).unwrap();
            assert!(scan_tar_members(&path, false).is_err());
            assert_eq!(
                scan_tar_members(&path, true).is_ok(),
                kind != tar::EntryType::Fifo
            );
            let gzip = root.path().join("binary.tar.gz");
            let mut encoder = GzBuilder::new().write(Vec::new(), Compression::best());
            encoder.write_all(&bytes).unwrap();
            fs::write(&gzip, encoder.finish().unwrap()).unwrap();
            assert_eq!(scan_tar_gzip_members(&gzip).is_ok(), kind.is_dir());
        }
        let empty = TarBuilder::new(Vec::new()).into_inner().unwrap();
        fs::write(&path, &empty).unwrap();
        assert!(scan_tar_members(&path, true).is_err());
        let mut encoder = GzBuilder::new().write(Vec::new(), Compression::best());
        encoder.write_all(&empty).unwrap();
        fs::write(&path, encoder.finish().unwrap()).unwrap();
        assert!(scan_tar_gzip_members(&path).is_err());
        for length in [2, 4] {
            assert!(scan_reader(&mut &b"abc"[..], length).is_err());
        }
        assert_eq!(scan_reader(&mut &b"abc"[..], 3).unwrap(), 3);
        for path in [
            "/absolute",
            "../escape",
            "src/.ssh/config",
            &"a".repeat(MAX_ARCHIVE_PATH_BYTES + 1),
        ] {
            assert_eq!(
                validate_scanned_path(path),
                Err(ReleaseArtifactError::ProtectedMaterialDetected)
            );
        }
    }

    fn write_tar_fixture(path: &Path, member: &str, bytes: &[u8], gzip: bool) {
        let mut header = TarHeader::new_gnu();
        header.set_entry_type(tar::EntryType::Regular);
        header.set_size(bytes.len() as u64);
        header.set_mode(0o644);
        header.set_uid(0);
        header.set_gid(0);
        header.set_mtime(1);
        header.set_cksum();
        if gzip {
            let output = fs::File::create(path).expect("gzip tar fixture");
            let encoder = GzBuilder::new()
                .mtime(1)
                .operating_system(255)
                .write(output, Compression::best());
            let mut archive = TarBuilder::new(encoder);
            archive
                .append_data(&mut header, member, bytes)
                .expect("gzip tar member");
            let encoder = archive.into_inner().expect("gzip tar archive");
            encoder.finish().expect("gzip tar finish");
        } else {
            let output = fs::File::create(path).expect("tar fixture");
            let mut archive = TarBuilder::new(output);
            archive
                .append_data(&mut header, member, bytes)
                .expect("tar member");
            archive.finish().expect("tar finish");
        }
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
