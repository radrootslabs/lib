//! Candidate-bound immutable advisory snapshot admission.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fmt;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest as _, Sha256};

use crate::safe_artifact_io::{
    self, ArchiveEvidence, FileEvidence, TarGzipLimits, TraversalLimits, TraversalSnapshot,
    TraversedFile,
};

const DECISION_PATH: &str =
    "contracts/architecture/decisions/services_hardening_advisory_snapshots.v1.json";
const DECISION_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../contracts/architecture/decisions/services_hardening_advisory_snapshots.v1.json"
));
const GRADLE_INIT_SCRIPT_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/advisory_inventory.init.gradle"
));
const GRADLE_INIT_SCRIPT_SHA256: &str =
    "4c51de29877813dd679c970507602940fdf8c38a586f8d2f6d101b6a93a8b598";
const MANIFEST_NAME: &str = "manifest.json";
const RUSTSEC_ARCHIVE_NAME: &str = "rustsec-db.tar.gz";
const RUSTSEC_REPORT_NAME: &str = "rustsec-report.json";
const NVD_ARCHIVE_NAME: &str = "nvd-data.tar.gz";
const OWASP_REPORT_NAME: &str = "owasp-report.json";
const GRADLE_RAW_GRAPH_NAME: &str = "inventory.json";
const RAW_SCANNER_OUTPUT_NAME: &str = "scanner-output.json";
const SNAPSHOT_FILE_NAMES: [&str; 5] = [
    MANIFEST_NAME,
    NVD_ARCHIVE_NAME,
    OWASP_REPORT_NAME,
    RUSTSEC_ARCHIVE_NAME,
    RUSTSEC_REPORT_NAME,
];
const MAX_SNAPSHOT_MANIFEST_BYTES: u64 = 67_108_864;
const MAX_PRODUCER_REQUEST_BYTES: usize = 1_048_576;
const MAX_TOOL_MANIFEST_BYTES: usize = 16_777_216;
const MAX_TOOL_OBSERVATION_BYTES: usize = 4_194_304;
const MAX_NVD_TRACE_BYTES: usize = 16_777_216;
const MAX_PROVIDER_EVIDENCE_BYTES: usize = 67_108_864;
const MAX_REPORT_BYTES: u64 = 67_108_864;
const MAX_RAW_WORKLOAD_REPORT_BYTES: u64 = 33_554_432;
const MAX_GRADLE_GRAPH_BYTES: u64 = 8_388_608;
const MAX_GRADLE_ARTIFACT_BYTES: u64 = 17_179_869_184;
const MAX_GRADLE_COMPONENTS: usize = 65_536;
const MAX_GRADLE_EDGES: usize = 262_144;
const MAX_GRADLE_ARTIFACTS: usize = 65_536;
const MAX_GRADLE_VARIANTS: usize = 4_096;
const MAX_GRADLE_ATTRIBUTES: usize = 1_024;
const MAX_GRADLE_CAPABILITIES: usize = 1_024;
const MAX_GRADLE_REJECTED_VERSIONS: usize = 1_024;
const MAX_GRADLE_EXTERNAL_VARIANT_DEPTH: usize = 4;
const MAX_NVD_REQUESTS: usize = 1_024;
const MAX_NVD_RESPONSE_BYTES: u64 = 67_108_864;
const MAX_NVD_AGGREGATE_BYTES: u64 = 536_870_912;
const QUALIFICATION_FRESHNESS_SECONDS: u64 = 86_400;
const DEVELOPMENT_REUSE_SECONDS: u64 = 604_800;
const TOOL_REVIEW_SECONDS: u64 = 86_400;
const ANALYSIS_DEADLINE_SECONDS: u64 = 3_600;
const NON_WAIVABLE_RUSTSEC: &str = "RUSTSEC-2026-0253";
const BOUNDED_PROCESS_DECISION_SHA256: &str =
    "41e11fe9bfecfb946063bafcb4df7eb811abdf03b0eaf8df4bff0ccb382d9784";
const BOUNDED_PROCESS_SOURCE_SHA256: &str =
    "56b8de0c3c34b7481f0f8c792e37dfb0d9fc12a5b990287c979e89fd6989202e";

const TOOL_PINS: [ToolPin; 4] = [
    ToolPin {
        id: "cargo_audit",
        version: "0.22.1",
        executable_sha256: "c5cd7c0da8a9d0dff338aa1a2a30b0c723fde8201c23481f49e75be0bb77fe74",
        source_sha256: "2f4e27b0ab2d116c87c29db159ad42565cdcdccf77eb62ef0486ddd017a02da6",
        receipt_sha256: "74196dcade966311e535813d2e3bda2f9515186c700f3074acdc2849abce1303",
        projection_sha256: "c945e827744d62b46e46826c8c066e3f8f354d972fa7f069d963e287b8e7b595",
    },
    ToolPin {
        id: "owasp_dependency_check",
        version: "12.2.2",
        executable_sha256: "d683a49ec335eeca93d8707f3e8ce21d7ba63a1e619a325c6518f89c25efcdc4",
        source_sha256: "bf07fefd81af3094c5f6850423b014df44db62ce2dbad0f79079a90df675e44a",
        receipt_sha256: "83d9a668179fb5e9ad6fd5c625a9ab58b8371a6d32b59808fc8d2973957f54a2",
        projection_sha256: "1011327eeed9ed5c4835bdf8e08f5b1852cf118506563daa5ddb0a251e65fb9a",
    },
    ToolPin {
        id: "java",
        version: "21.0.12.1",
        executable_sha256: "9be1d0a740ff6502df1a762145e62860f5de4b7e17658d9cb9498da3acf9d16c",
        source_sha256: "575bb8d9d604821d8f350325b28a35e49bcffd7ec33727b41edc8d709537dada",
        receipt_sha256: "e7018157655e26443392ace45967e427aafa1dd98e2e67f94bdd04267d1d9296",
        projection_sha256: "2e45aabf8388778d521d51e1ade0d4403fae4d656773bf547e2105dbd79ae655",
    },
    ToolPin {
        id: "gradle_wrapper",
        version: "9.5.0",
        executable_sha256: "ab5c0cad16305af2e619c159c1f58dd68d07fab9c11e36701e109c0277407f7a",
        source_sha256: "553c78f50dafcd54d65b9a444649057857469edf836431389695608536d6b746",
        receipt_sha256: "a6538aaef9f5e2510cf4bbfe6d560e64ae4065e64a5c8ba0807d7448f6a89610",
        projection_sha256: "f84dc2cb5139d080b28eaee9bc67fcce9bb9acc5e6f609fc00f247b1dd282ef8",
    },
];

const CANDIDATE_REPOSITORIES: [&str; 10] = [
    "_radroots",
    "oss/lib",
    "oss/sdk",
    "oss/myc",
    "oss/rhi",
    "oss/apple_kit",
    "oss/cli",
    "oss/radrootsd",
    "oss/ios_app",
    "oss/harvestcircle",
];

const CARGO_AUDIT_ARGUMENTS: [&str; 7] = [
    "audit",
    "--db",
    "{absolute_rustsec_snapshot}",
    "--no-fetch",
    "--json",
    "--file",
    "{absolute_cargo_lock}",
];
const OWASP_UPDATE_ARGUMENTS: [&str; 12] = [
    "--updateonly",
    "--data",
    "{nvd_staging}",
    "--disableKnownExploited",
    "--disableRetireJs",
    "--disableHostedSuppressions",
    "--disableVersionCheck",
    "--disableOssIndex",
    "--disableCentral",
    "--disableNodeAudit",
    "--disableYarnAudit",
    "--disablePnpmAudit",
];
const OWASP_OFFLINE_ARGUMENTS: [&str; 20] = [
    "--noupdate",
    "--data",
    "{nvd_snapshot}",
    "--scan",
    "{gradle_inventory}",
    "--project",
    "{exact_workload_id}",
    "--format",
    "JSON",
    "--out",
    "{report}",
    "--disableKnownExploited",
    "--disableRetireJs",
    "--disableHostedSuppressions",
    "--disableVersionCheck",
    "--disableOssIndex",
    "--disableCentral",
    "--disableNodeAudit",
    "--disableYarnAudit",
    "--disablePnpmAudit",
];

#[derive(Clone, Copy)]
struct ToolPin {
    id: &'static str,
    version: &'static str,
    executable_sha256: &'static str,
    source_sha256: &'static str,
    receipt_sha256: &'static str,
    projection_sha256: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AdvisoryFailureKind {
    ArchiveRejected,
    BindingChanged,
    ExpiredSuppression,
    InventoryMismatch,
    InvalidContract,
    InvalidReport,
    InvalidSnapshot,
    KnownVulnerability,
    StaleSnapshot,
    TimedOut,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AdvisoryError {
    kind: AdvisoryFailureKind,
}

impl AdvisoryError {
    const fn new(kind: AdvisoryFailureKind) -> Self {
        Self { kind }
    }

    pub(crate) const fn kind(self) -> AdvisoryFailureKind {
        self.kind
    }
}

impl fmt::Display for AdvisoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self.kind {
            AdvisoryFailureKind::ArchiveRejected => "advisory archive admission failed",
            AdvisoryFailureKind::BindingChanged => "advisory snapshot binding changed",
            AdvisoryFailureKind::ExpiredSuppression => "advisory suppression is expired",
            AdvisoryFailureKind::InventoryMismatch => "advisory inventory differs",
            AdvisoryFailureKind::InvalidContract => "advisory contract differs",
            AdvisoryFailureKind::InvalidReport => "advisory scanner report is invalid",
            AdvisoryFailureKind::InvalidSnapshot => "advisory snapshot is invalid",
            AdvisoryFailureKind::KnownVulnerability => "advisory findings remain unsuppressed",
            AdvisoryFailureKind::StaleSnapshot => "advisory snapshot is stale",
            AdvisoryFailureKind::TimedOut => "advisory operation timed out",
            AdvisoryFailureKind::Unavailable => "mandatory advisory input is unavailable",
        })
    }
}

impl std::error::Error for AdvisoryError {}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CandidateIdentity {
    pub(crate) generation: u64,
    pub(crate) digest: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Ord, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SourceIdentity {
    pub(crate) repository: String,
    pub(crate) revision: String,
    pub(crate) tree: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WorkloadInventory {
    pub(crate) id: String,
    pub(crate) repository: String,
    pub(crate) build_root: String,
    pub(crate) package_manager: String,
    pub(crate) language: String,
    pub(crate) manifest_path: Option<String>,
    pub(crate) lockfile_path: Option<String>,
    pub(crate) project_path: Option<String>,
    pub(crate) configuration: Option<String>,
    pub(crate) dependency_count: u64,
    pub(crate) input_sha256: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FreshnessMode {
    Development,
    Qualification,
}

pub(crate) struct AdmissionRequest {
    pub(crate) candidate: CandidateIdentity,
    pub(crate) sources: Vec<SourceIdentity>,
    pub(crate) inventory: Vec<WorkloadInventory>,
    pub(crate) evaluation_epoch: u64,
    pub(crate) freshness: FreshnessMode,
    pub(crate) producer_request: Vec<u8>,
    pub(crate) step_297_tool_manifest: Vec<u8>,
    pub(crate) fresh_tool_observation: Vec<u8>,
    pub(crate) nvd_network_trace: Vec<u8>,
    pub(crate) provider_execution_evidence: Vec<u8>,
    pub(crate) admitted_gradle_graphs: Vec<AdmittedGradleGraph>,
    pub(crate) admitted_scanner_outputs: Vec<AdmittedScannerOutput>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct TrustedProducerRequest {
    schema: String,
    candidate_generation: u64,
    candidate_digest: String,
    platform: String,
    process_contract_sha256: String,
    process_runner_source_sha256: String,
    nvd_endpoint: String,
    nvd_enforcement_program_sha256: String,
    nvd_enforcement_configuration_sha256: String,
    nvd_query_keys: Vec<String>,
    workload_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct TrustedToolManifest {
    schema: String,
    candidate_generation: u64,
    candidate_digest: String,
    platform: String,
    producer_request_sha256: String,
    tool_acquisition: Vec<ToolAcquisition>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct TrustedToolObservation {
    schema: String,
    candidate_generation: u64,
    candidate_digest: String,
    platform: String,
    producer_request_sha256: String,
    tool_manifest_sha256: String,
    row_projection_sha256: String,
    observed_at_epoch: u64,
    tool_state: Vec<ToolState>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct NvdNetworkTrace {
    schema: String,
    candidate_generation: u64,
    candidate_digest: String,
    producer_request_sha256: String,
    enforcement: String,
    enforcement_program_sha256: String,
    enforcement_configuration_sha256: String,
    request: Vec<NvdRequestTrace>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct NvdRequestTrace {
    sequence: u64,
    method: String,
    scheme: String,
    authority: String,
    path: String,
    started_at_epoch: u64,
    completed_at_epoch: u64,
    query: Vec<NvdQueryTrace>,
    response_status: u16,
    response_byte_length: u64,
    response_sha256: String,
    response_start_index: u64,
    response_results_per_page: u64,
    response_total_results: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct NvdQueryTrace {
    name: String,
    value: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct TrustedProviderEvidence {
    schema: String,
    candidate_generation: u64,
    candidate_digest: String,
    platform: String,
    producer_request_sha256: String,
    candidate_advisory_input_sha256: String,
    gradle_projection: Vec<GradleProjection>,
    provider_snapshot: Vec<ProviderSnapshot>,
    process_receipt: Vec<TrustedProcessReceipt>,
    suppressions: Vec<Suppression>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct TrustedProcessReceipt {
    id: String,
    state: OperationState,
    program_sha256: String,
    runtime_sha256: Vec<String>,
    arguments_sha256: String,
    environment_sha256: String,
    environment: Vec<ProcessEnvironmentRow>,
    working_directory_sha256: String,
    working_directory: ProcessWorkingDirectory,
    path_binding: Vec<ProcessPathBinding>,
    stdin_closed: bool,
    deadline_seconds: u64,
    started_at_epoch: u64,
    completed_at_epoch: u64,
    exit_code: i32,
    stdout_byte_length: u64,
    stdout_sha256: String,
    stderr_byte_length: u64,
    stderr_sha256: String,
    input_sha256: String,
    output_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ProcessEnvironmentRow {
    name: String,
    logical_value: String,
    value_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ProcessWorkingDirectory {
    logical_uri: String,
    kind: String,
    identity_sha256: String,
    pre_execution_entry_count: u64,
    pre_execution_tree_sha256: String,
    post_execution_entry_count: u64,
    post_execution_tree_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ProcessPathBinding {
    argument_index: u16,
    logical_role: String,
    identity_sha256: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
enum ProviderId {
    OwaspNvd,
    Rustsec,
}

impl ProviderId {
    const fn as_str(self) -> &'static str {
        match self {
            Self::OwaspNvd => "owasp_nvd",
            Self::Rustsec => "rustsec",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum OperationState {
    Complete,
    Failed,
    TimedOut,
    Unavailable,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ToolAcquisition {
    id: String,
    projection: Value,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ToolState {
    id: String,
    state: OperationState,
    reviewed_at_epoch: u64,
    normalized_version: String,
    executable_sha256: String,
    source_sha256: String,
    package_receipt_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct BlobBinding {
    path: String,
    byte_length: u64,
    sha256: String,
    logical_uri: String,
    media_type: String,
    logical_role: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ProviderSnapshot {
    provider: ProviderId,
    acquisition_kind: String,
    acquisition_count: u8,
    acquisition_state: OperationState,
    acquired_at_epoch: u64,
    digest_time_epoch: u64,
    database_identity_sha256: String,
    materialized_tree_sha256: String,
    bounded_deadline_seconds: u64,
    network_mode: String,
    producer_request_sha256: String,
    network_trace_sha256: String,
    acquisition_arguments: Vec<String>,
    archive_format: String,
    archive: BlobBinding,
    archive_expanded_bytes: u64,
    archive_member_count: u64,
    archive_payload_bytes: u64,
    analysis_state: OperationState,
    analyzed_at_epoch: u64,
    analysis_environment: String,
    analysis_arguments: Vec<String>,
    report: BlobBinding,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct GradleProjection {
    workload_id: String,
    state: OperationState,
    raw_graph_byte_length: u64,
    raw_graph_sha256: String,
    init_script_sha256: String,
    wrapper_arguments: Vec<String>,
    environment_keys: Vec<String>,
    environment_sha256: String,
    exit_code: i32,
    source_revision: String,
    source_tree: String,
    input_sha256: String,
    dependency_count: u64,
    component_count: u64,
    edge_count: u64,
    artifact_count: u64,
    components: Vec<GradleComponent>,
    edges: Vec<GradleEdge>,
    artifacts: Vec<GradleArtifact>,
    canonical_graph_sha256: String,
    materialized_tree_sha256: String,
    normalization_receipt_sha256: String,
    artifact_source_roots_sha256: String,
    seed_cache_inventory_sha256: String,
    wrapper_distribution_sha256: String,
    process_receipt_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct GradleComponent {
    root: bool,
    kind: String,
    group: Option<String>,
    name: Option<String>,
    version: Option<String>,
    build_root: Option<String>,
    project_path: Option<String>,
    variant: Value,
    variant_sha256: String,
    identity_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct GradleEdge {
    from_identity_sha256: String,
    to_identity_sha256: String,
    requested: Value,
    requested_sha256: String,
    constraint: bool,
    selected_variant: Value,
    selected_variant_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct GradleArtifact {
    component_identity_sha256: String,
    component: String,
    package_ecosystem: String,
    package_namespace: String,
    package_name: String,
    package_version: String,
    artifact_sha256: String,
    artifact_name: String,
    logical_name: String,
    artifact_type: String,
    classifier: Option<String>,
    byte_length: u64,
    extension: String,
    materialized_name: String,
    variant: Value,
    variant_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Suppression {
    id: String,
    provider: ProviderId,
    advisory_id: String,
    workload_id: String,
    package_ecosystem: String,
    package_namespace: String,
    package_name: String,
    package_version: String,
    owner: String,
    rationale: String,
    created_at_epoch: u64,
    expires_at_epoch: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct SnapshotManifest {
    schema: String,
    candidate: CandidateIdentity,
    sources: Vec<SourceIdentity>,
    inventory_sha256: String,
    inventory: Vec<WorkloadInventory>,
    candidate_advisory_input_sha256: String,
    step_297_tool_manifest_sha256: String,
    fresh_tool_observation_sha256: String,
    tool_acquisition: Vec<ToolAcquisition>,
    tool_state: Vec<ToolState>,
    gradle_projection: Vec<GradleProjection>,
    provider_snapshot: Vec<ProviderSnapshot>,
    suppressions: Vec<Suppression>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Finding {
    provider: ProviderId,
    advisory_id: String,
    package_ecosystem: String,
    package_namespace: String,
    package_name: String,
    package_version: String,
    workload_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AdvisorySnapshotEvidence {
    pub(crate) candidate: CandidateIdentity,
    pub(crate) manifest: FileEvidence,
    pub(crate) source_inventory_sha256: String,
    pub(crate) inventory_sha256: String,
    pub(crate) fresh_tool_observation_sha256: String,
    pub(crate) tool_row_projection_sha256: String,
    pub(crate) rustsec_archive: ArchiveEvidence,
    pub(crate) nvd_archive: ArchiveEvidence,
    pub(crate) rustsec_report: FileEvidence,
    pub(crate) owasp_report: FileEvidence,
    pub(crate) provider_time_trace_sha256: String,
    pub(crate) finding_inventory_sha256: String,
    pub(crate) suppression_inventory_sha256: String,
    pub(crate) candidate_advisory_input_sha256: String,
    pub(crate) gradle_projection_sha256: String,
    pub(crate) provider_freshness: Vec<ProviderFreshnessEvidence>,
    pub(crate) finding_count: usize,
    pub(crate) suppression_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProviderFreshnessEvidence {
    pub(crate) provider: String,
    pub(crate) acquired_at_epoch: u64,
    pub(crate) digest_time_epoch: u64,
    pub(crate) analyzed_at_epoch: u64,
    pub(crate) database_identity_sha256: String,
    pub(crate) archive_sha256: String,
    pub(crate) report_sha256: String,
    pub(crate) network_trace_sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReportEnvelope {
    schema: String,
    provider: ProviderId,
    workload_result: Vec<WorkloadResult>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkloadResult {
    workload_id: String,
    input_sha256: String,
    dependency_count: u64,
    provider_archive_sha256: String,
    materialized_tree_sha256: String,
    tool_observation_sha256: String,
    arguments: Vec<String>,
    environment_sha256: String,
    process_receipt_sha256: String,
    database_copy: Option<ScannerDatabaseCopy>,
    exit_code: i32,
    raw_output_byte_length: u64,
    raw_output_sha256: String,
    raw_scanner_output: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ScannerDatabaseCopy {
    root_identity_sha256: String,
    source_tree_sha256: String,
    pre_scan_tree_sha256: String,
    post_scan_tree_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct RawGradleGraph {
    schema: String,
    workload_id: String,
    build_root: String,
    project_path: String,
    configuration: String,
    components: Vec<RawGradleComponent>,
    edges: Vec<RawGradleEdge>,
    artifacts: Vec<RawGradleArtifact>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct RawGradleComponentCore {
    build_root: Option<String>,
    group: Option<String>,
    kind: String,
    name: Option<String>,
    project_path: Option<String>,
    version: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct RawGradleComponent {
    build_root: Option<String>,
    group: Option<String>,
    kind: String,
    name: Option<String>,
    project_path: Option<String>,
    root: bool,
    variant: RawGradleVariantEnvelope,
    version: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct RawGradleVariantEnvelope {
    selected: Vec<RawGradleVariant>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct RawGradleVariant {
    attributes: Vec<RawGradleAttribute>,
    capabilities: Vec<RawGradleCapability>,
    external_variant: Option<Box<RawGradleVariant>>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct RawGradleAttribute {
    name: String,
    value: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct RawGradleCapability {
    group: String,
    name: String,
    version: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct RawGradleEdge {
    constraint: bool,
    from: RawGradleComponentCore,
    requested: RawGradleSelector,
    selected_variant: RawGradleVariant,
    to: RawGradleComponentCore,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct RawGradleSelector {
    attributes: Vec<RawGradleAttribute>,
    build_root: Option<String>,
    capabilities: Vec<RawGradleCapability>,
    group: Option<String>,
    kind: String,
    name: Option<String>,
    project_path: Option<String>,
    version: Option<String>,
    version_constraint: Option<RawGradleVersionConstraint>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct RawGradleVersionConstraint {
    branch: Option<String>,
    preferred: String,
    rejected: Vec<String>,
    required: String,
    strict: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct RawGradleArtifact {
    artifact_name: String,
    artifact_type: String,
    classifier: Option<String>,
    component: RawGradleComponentCore,
    extension: String,
    group: Option<String>,
    logical_name: String,
    module_version: RawGradleModuleVersion,
    name: Option<String>,
    observed_byte_length: u64,
    observed_sha256: String,
    source_path: String,
    variant: RawGradleVariant,
    version: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct RawGradleModuleVersion {
    group: String,
    name: String,
    version: String,
}

pub(crate) struct AdmittedGradleGraph {
    raw: FileEvidence,
    workload_id: String,
    components: Vec<GradleComponent>,
    edges: Vec<GradleEdge>,
    artifacts: Vec<GradleArtifact>,
    canonical_graph_sha256: String,
    materialized_tree_sha256: String,
    normalization_receipt_sha256: String,
    artifact_source_roots_sha256: String,
    source_snapshot: TraversalSnapshot,
    materialized_snapshot: TraversalSnapshot,
    _materialization: tempfile::TempDir,
}

impl AdmittedGradleGraph {
    pub(crate) fn revalidate(&self) -> Result<(), AdvisoryError> {
        self.source_snapshot
            .revalidate()
            .map_err(map_binding_error)?;
        self.materialized_snapshot
            .revalidate()
            .map_err(map_binding_error)
    }

    fn raw_evidence(&self) -> &FileEvidence {
        &self.raw
    }

    fn matches_projection(&self, projection: &GradleProjection) -> bool {
        projection.workload_id == self.workload_id
            && projection.raw_graph_byte_length == self.raw.byte_length
            && projection.raw_graph_sha256 == self.raw.sha256
            && projection.components == self.components
            && projection.edges == self.edges
            && projection.artifacts == self.artifacts
            && projection.canonical_graph_sha256 == self.canonical_graph_sha256
            && projection.materialized_tree_sha256 == self.materialized_tree_sha256
            && projection.normalization_receipt_sha256 == self.normalization_receipt_sha256
            && projection.artifact_source_roots_sha256 == self.artifact_source_roots_sha256
    }
}

pub(crate) struct AdmittedScannerOutput {
    provider: ProviderId,
    workload_id: String,
    bytes: Vec<u8>,
    evidence: FileEvidence,
    snapshot: TraversalSnapshot,
}

impl AdmittedScannerOutput {
    fn revalidate(&self) -> Result<(), AdvisoryError> {
        self.snapshot.revalidate().map_err(map_binding_error)
    }
}

fn admit_raw_scanner_output(
    root: &Path,
    provider: ProviderId,
    workload_id: &str,
) -> Result<AdmittedScannerOutput, AdvisoryError> {
    let snapshot = safe_artifact_io::traverse_regular_files(
        root,
        TraversalLimits {
            max_entries: 1,
            max_files: 1,
            max_total_bytes: MAX_RAW_WORKLOAD_REPORT_BYTES,
            max_file_bytes: MAX_RAW_WORKLOAD_REPORT_BYTES,
            max_depth: 1,
            max_path_bytes: 64,
        },
        &[],
    )
    .map_err(map_binding_error)?;
    let file = snapshot
        .files()
        .first()
        .filter(|file| {
            snapshot.entry_count() == 1
                && snapshot.files().len() == 1
                && file.relative_path() == Path::new(RAW_SCANNER_OUTPUT_NAME)
        })
        .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
    let (bytes, evidence) = snapshot
        .read_evidenced(file, MAX_RAW_WORKLOAD_REPORT_BYTES)
        .map_err(map_binding_error)?;
    if bytes.is_empty() || serde_json::from_slice::<Value>(&bytes).is_err() {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport));
    }
    snapshot.revalidate().map_err(map_binding_error)?;
    Ok(AdmittedScannerOutput {
        provider,
        workload_id: workload_id.to_owned(),
        bytes,
        evidence,
        snapshot,
    })
}

struct GradleArtifactSourceRoot<'a> {
    path: &'a Path,
    logical_role: &'static str,
    identity_sha256: &'a str,
}

struct NormalizedRawGradleGraph {
    components: Vec<GradleComponent>,
    edges: Vec<GradleEdge>,
    artifacts: Vec<GradleArtifact>,
    source_paths: Vec<PathBuf>,
}

fn admit_raw_gradle_graph(
    raw_root: &Path,
    materialization_parent: &Path,
    authority: &WorkloadAuthority,
    source_revision: &str,
    expected_dependency_count: u64,
    artifact_source_roots: &[GradleArtifactSourceRoot<'_>],
) -> Result<AdmittedGradleGraph, AdvisoryError> {
    let source_snapshot = safe_artifact_io::traverse_regular_files(
        raw_root,
        TraversalLimits {
            max_entries: 1,
            max_files: 1,
            max_total_bytes: MAX_GRADLE_GRAPH_BYTES,
            max_file_bytes: MAX_GRADLE_GRAPH_BYTES,
            max_depth: 1,
            max_path_bytes: 64,
        },
        &[],
    )
    .map_err(map_binding_error)?;
    let raw_file = source_snapshot
        .files()
        .first()
        .filter(|file| {
            source_snapshot.entry_count() == 1
                && source_snapshot.files().len() == 1
                && file.relative_path() == Path::new(GRADLE_RAW_GRAPH_NAME)
        })
        .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch))?;
    let (raw_bytes, raw_evidence) = source_snapshot
        .read_evidenced(raw_file, MAX_GRADLE_GRAPH_BYTES)
        .map_err(map_binding_error)?;
    let raw_value: Value = serde_json::from_slice(&raw_bytes)
        .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch))?;
    if canonical_json_without_lf(&raw_value) != raw_bytes {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch));
    }
    let raw: RawGradleGraph = serde_json::from_value(raw_value)
        .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch))?;
    let artifact_source_roots_sha256 = artifact_source_roots_digest(artifact_source_roots)?;
    let normalized = normalize_raw_gradle_graph(
        &raw,
        authority,
        source_revision,
        expected_dependency_count,
        artifact_source_roots,
    )?;

    if !materialization_parent.is_absolute() {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot));
    }
    safe_artifact_io::validate_trusted_output_directory(materialization_parent)
        .map_err(map_binding_error)?;
    let materialization = tempfile::Builder::new()
        .prefix(".radroots-advisory-scan-")
        .tempdir_in(materialization_parent)
        .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::Unavailable))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(
            materialization.path(),
            std::fs::Permissions::from_mode(0o700),
        )
        .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::Unavailable))?;
    }
    #[cfg(not(unix))]
    {
        return Err(AdvisoryError::new(AdvisoryFailureKind::Unavailable));
    }

    let mut materialized = BTreeMap::<(String, String), FileEvidence>::new();
    for (artifact, source) in normalized.artifacts.iter().zip(&normalized.source_paths) {
        let key = (artifact.artifact_sha256.clone(), artifact.extension.clone());
        let evidence = if let Some(existing) = materialized.get(&key) {
            let observed = safe_artifact_io::hash_regular_path(source, MAX_GRADLE_ARTIFACT_BYTES)
                .map_err(map_binding_error)?;
            if &observed != existing {
                return Err(AdvisoryError::new(AdvisoryFailureKind::BindingChanged));
            }
            observed
        } else {
            let observed = safe_artifact_io::copy_regular_to_new_path(
                source,
                &materialization.path().join(&artifact.materialized_name),
                MAX_GRADLE_ARTIFACT_BYTES,
            )
            .map_err(map_binding_error)?;
            materialized.insert(key, observed.clone());
            observed
        };
        if evidence.byte_length != artifact.byte_length
            || evidence.sha256 != artifact.artifact_sha256
        {
            return Err(AdvisoryError::new(AdvisoryFailureKind::BindingChanged));
        }
    }
    let maximum_files = u64::try_from(materialized.len().max(1))
        .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch))?;
    let materialized_snapshot = safe_artifact_io::traverse_regular_files(
        materialization.path(),
        TraversalLimits {
            max_entries: maximum_files,
            max_files: maximum_files,
            max_total_bytes: MAX_GRADLE_ARTIFACT_BYTES,
            max_file_bytes: MAX_GRADLE_ARTIFACT_BYTES,
            max_depth: 1,
            max_path_bytes: 128,
        },
        &[],
    )
    .map_err(map_binding_error)?;
    if materialized_snapshot.files().len() != materialized.len()
        || materialized_snapshot.root_permission_mode() != 0o700
        || materialized_snapshot
            .files()
            .iter()
            .any(|file| file.permission_mode() != 0o600)
    {
        return Err(AdvisoryError::new(AdvisoryFailureKind::BindingChanged));
    }
    let materialized_tree_sha256 = materialized_tree_digest(&materialized_snapshot)?;
    let canonical_graph_sha256 = gradle_graph_digest_parts(
        authority.id,
        &normalized.components,
        &normalized.edges,
        &normalized.artifacts,
    )?;
    let normalization_receipt_sha256 = gradle_normalization_receipt_digest(
        authority.id,
        raw_evidence.byte_length,
        &raw_evidence.sha256,
        &canonical_graph_sha256,
        &materialized_tree_sha256,
        &artifact_source_roots_sha256,
    )?;
    source_snapshot.revalidate().map_err(map_binding_error)?;
    materialized_snapshot
        .revalidate()
        .map_err(map_binding_error)?;
    Ok(AdmittedGradleGraph {
        raw: raw_evidence,
        workload_id: authority.id.to_owned(),
        components: normalized.components,
        edges: normalized.edges,
        artifacts: normalized.artifacts,
        canonical_graph_sha256,
        materialized_tree_sha256,
        normalization_receipt_sha256,
        artifact_source_roots_sha256,
        source_snapshot,
        materialized_snapshot,
        _materialization: materialization,
    })
}

fn normalize_raw_gradle_graph(
    raw: &RawGradleGraph,
    authority: &WorkloadAuthority,
    source_revision: &str,
    expected_dependency_count: u64,
    artifact_source_roots: &[GradleArtifactSourceRoot<'_>],
) -> Result<NormalizedRawGradleGraph, AdvisoryError> {
    if raw.schema != "radroots.gradle-advisory-graph.v1"
        || authority.package_manager != "gradle"
        || raw.workload_id != authority.id
        || raw.build_root != authority.build_root
        || raw.project_path.as_str()
            != authority
                .project_path
                .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidContract))?
        || raw.configuration.as_str()
            != authority
                .configuration
                .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidContract))?
        || raw.components.is_empty()
        || raw.components.len() > MAX_GRADLE_COMPONENTS
        || raw.edges.len() > MAX_GRADLE_EDGES
        || raw.artifacts.len() > MAX_GRADLE_ARTIFACTS
        || raw.edges.len() as u64 != expected_dependency_count
    {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch));
    }
    require_strict_canonical_order(&raw.components)?;
    require_strict_canonical_order(&raw.edges)?;
    require_strict_canonical_order(&raw.artifacts)?;

    let mut components = Vec::with_capacity(raw.components.len());
    let mut component_by_core = BTreeMap::<Vec<u8>, (String, RawGradleVariantEnvelope)>::new();
    for component in &raw.components {
        let core = raw_component_core(component);
        validate_raw_component_core(&core)?;
        validate_raw_variant_envelope(&component.variant)?;
        let variant = serde_json::to_value(&component.variant)
            .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch))?;
        let mut normalized = GradleComponent {
            root: component.root,
            kind: component.kind.clone(),
            group: component.group.clone(),
            name: component.name.clone(),
            version: component.version.clone(),
            build_root: component.build_root.clone(),
            project_path: component.project_path.clone(),
            variant,
            variant_sha256: String::new(),
            identity_sha256: String::new(),
        };
        normalized.variant_sha256 = gradle_variant_digest(&normalized.variant)?;
        normalized.identity_sha256 = gradle_component_digest(&normalized)?;
        let core_key = canonical_row_key(&core)?;
        if component_by_core
            .insert(
                core_key,
                (
                    normalized.identity_sha256.clone(),
                    component.variant.clone(),
                ),
            )
            .is_some()
        {
            return Err(AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch));
        }
        components.push(normalized);
    }
    components.sort_by(|left, right| left.identity_sha256.cmp(&right.identity_sha256));
    if components
        .windows(2)
        .any(|window| window[0].identity_sha256 >= window[1].identity_sha256)
    {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch));
    }
    let roots = components
        .iter()
        .filter(|component| component.root)
        .collect::<Vec<_>>();
    if roots.len() != 1
        || roots[0].kind != "project"
        || roots[0].build_root.as_deref() != Some(authority.build_root)
        || roots[0].project_path.as_deref() != authority.project_path
    {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch));
    }

    let mut edges = Vec::with_capacity(raw.edges.len());
    for edge in &raw.edges {
        validate_raw_component_core(&edge.from)?;
        validate_raw_component_core(&edge.to)?;
        validate_raw_selector(&edge.requested)?;
        validate_raw_variant(&edge.selected_variant, 0)?;
        let (from_identity, _) = component_by_core
            .get(&canonical_row_key(&edge.from)?)
            .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch))?;
        let (to_identity, to_variants) = component_by_core
            .get(&canonical_row_key(&edge.to)?)
            .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch))?;
        if from_identity == to_identity || !to_variants.selected.contains(&edge.selected_variant) {
            return Err(AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch));
        }
        let requested = serde_json::to_value(&edge.requested)
            .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch))?;
        let selected_variant = serde_json::to_value(&edge.selected_variant)
            .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch))?;
        edges.push(GradleEdge {
            from_identity_sha256: from_identity.clone(),
            to_identity_sha256: to_identity.clone(),
            requested_sha256: gradle_request_digest(&requested)?,
            requested,
            constraint: edge.constraint,
            selected_variant_sha256: gradle_variant_digest(&selected_variant)?,
            selected_variant,
        });
    }
    edges.sort_by(|left, right| {
        canonical_row_key(left)
            .unwrap_or_default()
            .cmp(&canonical_row_key(right).unwrap_or_default())
    });
    require_strict_canonical_order(&edges)?;
    let mut reachable = BTreeSet::from([roots[0].identity_sha256.as_str()]);
    loop {
        let before = reachable.len();
        for edge in &edges {
            if reachable.contains(edge.from_identity_sha256.as_str()) {
                reachable.insert(edge.to_identity_sha256.as_str());
            }
        }
        if reachable.len() == before {
            break;
        }
    }
    if reachable.len() != components.len() {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch));
    }

    let mut artifacts = Vec::with_capacity(raw.artifacts.len());
    let mut source_paths = Vec::with_capacity(raw.artifacts.len());
    let mut total_artifact_bytes = 0_u64;
    for artifact in &raw.artifacts {
        validate_raw_component_core(&artifact.component)?;
        validate_raw_variant(&artifact.variant, 0)?;
        let (component_identity, variants) = component_by_core
            .get(&canonical_row_key(&artifact.component)?)
            .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch))?;
        if !variants.selected.contains(&artifact.variant)
            || artifact.group != artifact.component.group
            || artifact.name != artifact.component.name
            || artifact.version != artifact.component.version
            || !valid_ascii_text(&artifact.module_version.group, 256, false)
            || !valid_ascii_text(&artifact.module_version.name, 256, false)
            || !valid_ascii_text(&artifact.module_version.version, 128, false)
            || artifact.observed_byte_length == 0
            || artifact.observed_byte_length > MAX_GRADLE_ARTIFACT_BYTES
            || !valid_hex(&artifact.observed_sha256, 64)
            || !valid_ascii_text(&artifact.artifact_name, 256, false)
            || artifact.artifact_name.contains(['/', '\\'])
            || !valid_ascii_text(&artifact.logical_name, 256, false)
            || !valid_ascii_text(&artifact.artifact_type, 64, false)
            || !valid_artifact_extension(&artifact.extension)
            || !artifact
                .artifact_name
                .ends_with(&format!(".{}", artifact.extension))
            || artifact
                .classifier
                .as_deref()
                .is_some_and(|value| !valid_ascii_text(value, 128, false))
            || !valid_ascii_text(&artifact.source_path, 4_096, false)
        {
            return Err(AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch));
        }
        let source_path = PathBuf::from(&artifact.source_path);
        let source_roots = artifact_source_roots
            .iter()
            .filter_map(|root| {
                source_path
                    .strip_prefix(root.path)
                    .ok()
                    .filter(|relative| {
                        !relative.as_os_str().is_empty()
                            && relative
                                .components()
                                .all(|component| matches!(component, Component::Normal(_)))
                    })
                    .map(|relative| (root, relative))
            })
            .collect::<Vec<_>>();
        if !valid_absolute_normal_path(&source_path) || source_roots.len() != 1 {
            return Err(AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch));
        }
        let (source_root, relative) = source_roots[0];
        let source_evidence =
            safe_artifact_io::hash_regular(source_root.path, relative, MAX_GRADLE_ARTIFACT_BYTES)
                .map_err(map_binding_error)?;
        if source_evidence.byte_length != artifact.observed_byte_length
            || source_evidence.sha256 != artifact.observed_sha256
        {
            return Err(AdvisoryError::new(AdvisoryFailureKind::BindingChanged));
        }
        total_artifact_bytes = total_artifact_bytes
            .checked_add(artifact.observed_byte_length)
            .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch))?;
        if total_artifact_bytes > MAX_GRADLE_ARTIFACT_BYTES {
            return Err(AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch));
        }
        let component = components
            .iter()
            .find(|component| &component.identity_sha256 == component_identity)
            .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch))?;
        let component_label =
            match component.kind.as_str() {
                "module" => {
                    let group = component.group.as_deref().ok_or_else(|| {
                        AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch)
                    })?;
                    let name = component.name.as_deref().ok_or_else(|| {
                        AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch)
                    })?;
                    let version = component.version.as_deref().ok_or_else(|| {
                        AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch)
                    })?;
                    if artifact.module_version.group != group
                        || artifact.module_version.name != name
                        || artifact.module_version.version != version
                    {
                        return Err(AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch));
                    }
                    format!("{group}:{name}:{version}")
                }
                "project" => {
                    let root = component.build_root.as_deref().ok_or_else(|| {
                        AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch)
                    })?;
                    let project = component.project_path.as_deref().ok_or_else(|| {
                        AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch)
                    })?;
                    if !valid_project_module_version(
                        root,
                        project,
                        &artifact.module_version,
                        source_revision,
                    ) {
                        return Err(AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch));
                    }
                    format!("{root}:{project}")
                }
                _ => {
                    return Err(AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch));
                }
            };
        let variant = serde_json::to_value(&artifact.variant)
            .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch))?;
        artifacts.push(GradleArtifact {
            component_identity_sha256: component_identity.clone(),
            component: component_label,
            package_ecosystem: "maven".to_owned(),
            package_namespace: artifact.module_version.group.clone(),
            package_name: artifact.module_version.name.clone(),
            package_version: artifact.module_version.version.clone(),
            artifact_sha256: artifact.observed_sha256.clone(),
            artifact_name: artifact.artifact_name.clone(),
            logical_name: artifact.logical_name.clone(),
            artifact_type: artifact.artifact_type.clone(),
            classifier: artifact.classifier.clone(),
            byte_length: artifact.observed_byte_length,
            extension: artifact.extension.clone(),
            materialized_name: format!("{}.{}", artifact.observed_sha256, artifact.extension),
            variant_sha256: gradle_variant_digest(&variant)?,
            variant,
        });
        source_paths.push(source_path);
    }
    let mut paired = artifacts.into_iter().zip(source_paths).collect::<Vec<_>>();
    paired.sort_by(|left, right| {
        canonical_row_key(&left.0)
            .unwrap_or_default()
            .cmp(&canonical_row_key(&right.0).unwrap_or_default())
    });
    if paired.windows(2).any(|window| window[0].0 == window[1].0) {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch));
    }
    let (artifacts, source_paths): (Vec<_>, Vec<_>) = paired.into_iter().unzip();
    Ok(NormalizedRawGradleGraph {
        components,
        edges,
        artifacts,
        source_paths,
    })
}

fn raw_component_core(component: &RawGradleComponent) -> RawGradleComponentCore {
    RawGradleComponentCore {
        build_root: component.build_root.clone(),
        group: component.group.clone(),
        kind: component.kind.clone(),
        name: component.name.clone(),
        project_path: component.project_path.clone(),
        version: component.version.clone(),
    }
}

fn valid_absolute_normal_path(path: &Path) -> bool {
    path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::RootDir | Component::Normal(_)))
}

fn artifact_source_roots_digest(
    roots: &[GradleArtifactSourceRoot<'_>],
) -> Result<String, AdvisoryError> {
    if roots.is_empty() || roots.len() > 2 {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch));
    }
    let mut rows = Vec::with_capacity(roots.len());
    let mut roles = BTreeSet::new();
    for root in roots {
        if !valid_absolute_normal_path(root.path)
            || !matches!(
                root.logical_role,
                "candidate_build_output" | "governed_seed_cache"
            )
            || !roles.insert(root.logical_role)
            || !valid_hex(root.identity_sha256, 64)
        {
            return Err(AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch));
        }
        rows.push(serde_json::json!({
            "identity_sha256": root.identity_sha256,
            "logical_role": root.logical_role,
        }));
    }
    rows.sort_by(|left, right| {
        canonical_json_without_lf(left).cmp(&canonical_json_without_lf(right))
    });
    domain_json_digest(
        b"radroots-advisory-gradle-artifact-source-roots-v1\0",
        &rows,
    )
}

#[derive(Clone, Copy)]
struct WorkloadAuthority {
    id: &'static str,
    repository: &'static str,
    build_root: &'static str,
    package_manager: &'static str,
    language: &'static str,
    manifest_path: Option<&'static str>,
    lockfile_path: Option<&'static str>,
    project_path: Option<&'static str>,
    configuration: Option<&'static str>,
}

const WORKLOADS: [WorkloadAuthority; 16] = [
    cargo_workload("cli", "oss/cli", ".", "Cargo.toml", "Cargo.lock"),
    cargo_workload(
        "harvest_core",
        "oss/harvestcircle",
        "core",
        "core/Cargo.toml",
        "core/Cargo.lock",
    ),
    cargo_workload(
        "harvest_xtask",
        "oss/harvestcircle",
        "tools/xtask",
        "tools/xtask/Cargo.toml",
        "tools/xtask/Cargo.lock",
    ),
    cargo_workload(
        "ios_ffi",
        "oss/ios_app",
        "RadrootsFFI",
        "Cargo.toml",
        "Cargo.lock",
    ),
    cargo_workload("lib", "oss/lib", ".", "Cargo.toml", "Cargo.lock"),
    cargo_workload("myc", "oss/myc", ".", "Cargo.toml", "Cargo.lock"),
    cargo_workload(
        "radrootsd",
        "oss/radrootsd",
        ".",
        "Cargo.toml",
        "Cargo.lock",
    ),
    cargo_workload("rhi", "oss/rhi", ".", "Cargo.toml", "Cargo.lock"),
    cargo_workload("root", "_radroots", ".", "Cargo.toml", "Cargo.lock"),
    cargo_workload("sdk", "oss/sdk", ".", "Cargo.toml", "Cargo.lock"),
    gradle_workload(
        "app_design_system",
        ".",
        ":app:design_system",
        "desktopRuntimeClasspath",
    ),
    gradle_workload("app_desktop", ".", ":app:desktop", "runtimeClasspath"),
    gradle_workload("app_shared", ".", ":app:shared", "desktopRuntimeClasspath"),
    gradle_workload(
        "tools_design_catalog",
        ".",
        ":tools:design_catalog",
        "desktopRuntimeClasspath",
    ),
    gradle_workload(
        "build_logic_contracts",
        "build-logic",
        ":contracts",
        "runtimeClasspath",
    ),
    gradle_workload(
        "build_logic_plugins",
        "build-logic",
        ":plugins",
        "runtimeClasspath",
    ),
];

const fn cargo_workload(
    id: &'static str,
    repository: &'static str,
    build_root: &'static str,
    manifest_path: &'static str,
    lockfile_path: &'static str,
) -> WorkloadAuthority {
    WorkloadAuthority {
        id,
        repository,
        build_root,
        package_manager: "cargo",
        language: "rust",
        manifest_path: Some(manifest_path),
        lockfile_path: Some(lockfile_path),
        project_path: None,
        configuration: None,
    }
}

const fn gradle_workload(
    id: &'static str,
    build_root: &'static str,
    project_path: &'static str,
    configuration: &'static str,
) -> WorkloadAuthority {
    WorkloadAuthority {
        id,
        repository: "oss/harvestcircle",
        build_root,
        package_manager: "gradle",
        language: "kotlin",
        manifest_path: None,
        lockfile_path: None,
        project_path: Some(project_path),
        configuration: Some(configuration),
    }
}

pub(crate) fn validate_decision(root: &Path) -> Result<(), String> {
    let bytes = safe_artifact_io::read_regular_path(&root.join(DECISION_PATH), 1_048_576)
        .map_err(|_| "advisory snapshot decision admission failed".to_owned())?;
    if bytes != DECISION_BYTES {
        return Err("advisory snapshot decision differs from compiled authority".to_owned());
    }
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|_| "advisory snapshot decision is invalid".to_owned())?;
    if canonical_pretty_json(&value).as_slice() != bytes {
        return Err("advisory snapshot decision is not canonical pretty JSON".to_owned());
    }
    Ok(())
}

pub(crate) fn admit_snapshot(
    root: &Path,
    materialization_parent: &Path,
    request: &AdmissionRequest,
) -> Result<AdvisorySnapshotEvidence, AdvisoryError> {
    validate_request(request)?;
    let snapshot = safe_artifact_io::traverse_regular_files(
        root,
        TraversalLimits {
            max_entries: SNAPSHOT_FILE_NAMES.len() as u64,
            max_files: SNAPSHOT_FILE_NAMES.len() as u64,
            max_total_bytes: 4_430_233_600,
            max_file_bytes: 2_147_483_648,
            max_depth: 1,
            max_path_bytes: 128,
        },
        &[],
    )
    .map_err(map_binding_error)?;
    let files = exact_snapshot_files(&snapshot)?;
    let manifest_file = files
        .get(MANIFEST_NAME)
        .copied()
        .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch))?;
    let (manifest_bytes, manifest_evidence) = snapshot
        .read_evidenced(manifest_file, MAX_SNAPSHOT_MANIFEST_BYTES)
        .map_err(map_binding_error)?;
    let manifest: SnapshotManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot))?;
    let manifest_value: Value = serde_json::from_slice(&manifest_bytes)
        .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot))?;
    if canonical_json(&manifest_value) != manifest_bytes {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot));
    }
    validate_manifest(&manifest, request)?;

    let mut archive_evidence = BTreeMap::new();
    let mut materialized_archives = Vec::new();
    let mut report_evidences = BTreeMap::new();
    let mut findings = Vec::new();
    for graph in &request.admitted_gradle_graphs {
        graph.revalidate()?;
    }
    for provider in &manifest.provider_snapshot {
        let archive_file = files
            .get(provider.archive.path.as_str())
            .copied()
            .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch))?;
        let archive = snapshot
            .admit_deterministic_tar_gzip(archive_file, archive_limits())
            .map_err(map_archive_error)?;
        validate_blob(&provider.archive, &archive.compressed)?;
        if provider.archive_expanded_bytes != archive.expanded_bytes
            || provider.archive_member_count != archive.member_count
            || provider.archive_payload_bytes != archive.payload_bytes
        {
            return Err(AdvisoryError::new(AdvisoryFailureKind::BindingChanged));
        }
        let materialized = snapshot
            .materialize_deterministic_tar_gzip(
                archive_file,
                materialization_parent,
                archive_limits(),
            )
            .map_err(map_archive_error)?;
        let materialized_tree = materialized_tree_digest(materialized.snapshot())?;
        if materialized.evidence() != &archive
            || materialized_tree != provider.materialized_tree_sha256
            || !valid_hex(&provider.database_identity_sha256, 64)
        {
            return Err(AdvisoryError::new(AdvisoryFailureKind::BindingChanged));
        }
        materialized.revalidate().map_err(map_binding_error)?;
        archive_evidence.insert(provider.provider, archive);
        materialized_archives.push(materialized);

        let report_file = files
            .get(provider.report.path.as_str())
            .copied()
            .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch))?;
        let (report_bytes, report_evidence) = snapshot
            .read_evidenced(report_file, MAX_REPORT_BYTES)
            .map_err(map_binding_error)?;
        validate_blob(&provider.report, &report_evidence)?;
        findings.extend(parse_unsuppressed_report(
            provider,
            &report_bytes,
            request,
            &manifest.gradle_projection,
        )?);
        report_evidences.insert(provider.provider, report_evidence);
    }
    for graph in &request.admitted_gradle_graphs {
        graph.revalidate()?;
    }
    for materialized in &materialized_archives {
        materialized.revalidate().map_err(map_binding_error)?;
    }
    for output in &request.admitted_scanner_outputs {
        output.revalidate()?;
    }

    let unsuppressed_count = findings.len();
    let finding_inventory_sha256 = finding_digest(&findings)?;
    apply_suppressions(
        &mut findings,
        &manifest.suppressions,
        request.evaluation_epoch,
    )?;
    if !findings.is_empty() {
        return Err(AdvisoryError::new(AdvisoryFailureKind::KnownVulnerability));
    }
    snapshot.revalidate().map_err(map_binding_error)?;
    for materialized in &materialized_archives {
        materialized.revalidate().map_err(map_binding_error)?;
    }
    for output in &request.admitted_scanner_outputs {
        output.revalidate()?;
    }
    let rustsec = archive_evidence
        .remove(&ProviderId::Rustsec)
        .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch))?;
    let nvd = archive_evidence
        .remove(&ProviderId::OwaspNvd)
        .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch))?;
    let rustsec_report = report_evidences
        .remove(&ProviderId::Rustsec)
        .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch))?;
    let owasp_report = report_evidences
        .remove(&ProviderId::OwaspNvd)
        .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch))?;
    let provider_freshness = manifest
        .provider_snapshot
        .iter()
        .map(|provider| ProviderFreshnessEvidence {
            provider: provider.provider.as_str().to_owned(),
            acquired_at_epoch: provider.acquired_at_epoch,
            digest_time_epoch: provider.digest_time_epoch,
            analyzed_at_epoch: provider.analyzed_at_epoch,
            database_identity_sha256: provider.database_identity_sha256.clone(),
            archive_sha256: provider.archive.sha256.clone(),
            report_sha256: provider.report.sha256.clone(),
            network_trace_sha256: provider.network_trace_sha256.clone(),
        })
        .collect();
    Ok(AdvisorySnapshotEvidence {
        candidate: manifest.candidate,
        manifest: manifest_evidence,
        source_inventory_sha256: source_digest(&manifest.sources)?,
        inventory_sha256: manifest.inventory_sha256,
        fresh_tool_observation_sha256: manifest.fresh_tool_observation_sha256,
        tool_row_projection_sha256: tool_observation_digest(
            &manifest.tool_acquisition,
            &manifest.tool_state,
        )?,
        rustsec_archive: rustsec,
        nvd_archive: nvd,
        rustsec_report,
        owasp_report,
        provider_time_trace_sha256: provider_trace_digest(&manifest.provider_snapshot)?,
        finding_inventory_sha256,
        suppression_inventory_sha256: suppression_digest(&manifest.suppressions)?,
        candidate_advisory_input_sha256: manifest.candidate_advisory_input_sha256,
        gradle_projection_sha256: gradle_projection_digest(&manifest.gradle_projection)?,
        provider_freshness,
        finding_count: unsuppressed_count,
        suppression_count: manifest.suppressions.len(),
    })
}

fn exact_snapshot_files(
    snapshot: &TraversalSnapshot,
) -> Result<BTreeMap<&str, &TraversedFile>, AdvisoryError> {
    if snapshot.entry_count() != SNAPSHOT_FILE_NAMES.len() as u64
        || snapshot.files().len() != SNAPSHOT_FILE_NAMES.len()
    {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch));
    }
    let mut files = BTreeMap::new();
    for file in snapshot.files() {
        let relative = file
            .relative_path()
            .to_str()
            .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch))?;
        if Path::new(relative).components().count() != 1
            || !SNAPSHOT_FILE_NAMES.contains(&relative)
            || files.insert(relative, file).is_some()
        {
            return Err(AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch));
        }
    }
    Ok(files)
}

fn validate_request(request: &AdmissionRequest) -> Result<(), AdvisoryError> {
    if request.candidate.generation == 0
        || !valid_hex(&request.candidate.digest, 64)
        || request.evaluation_epoch == 0
        || request.sources.is_empty()
        || request.inventory.len() != WORKLOADS.len()
        || request.admitted_gradle_graphs.len()
            != WORKLOADS
                .iter()
                .filter(|workload| workload.package_manager == "gradle")
                .count()
        || request.admitted_scanner_outputs.len() != WORKLOADS.len()
    {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot));
    }
    validate_trusted_authority(request)?;
    if request.sources.len() != CANDIDATE_REPOSITORIES.len()
        || !request
            .sources
            .iter()
            .map(|source| source.repository.as_str())
            .eq(CANDIDATE_REPOSITORIES)
    {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch));
    }
    let mut repositories = BTreeSet::new();
    for source in &request.sources {
        if !valid_identifier(&source.repository)
            || !valid_hex(&source.revision, 40)
            || !valid_hex(&source.tree, 40)
            || !repositories.insert(source.repository.as_str())
        {
            return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot));
        }
    }
    for (actual, expected) in request.inventory.iter().zip(WORKLOADS) {
        if actual.id != expected.id
            || actual.repository != expected.repository
            || actual.build_root != expected.build_root
            || actual.package_manager != expected.package_manager
            || actual.language != expected.language
            || actual.manifest_path.as_deref() != expected.manifest_path
            || actual.lockfile_path.as_deref() != expected.lockfile_path
            || actual.project_path.as_deref() != expected.project_path
            || actual.configuration.as_deref() != expected.configuration
            || actual.dependency_count == 0
            || !valid_hex(&actual.input_sha256, 64)
            || !repositories.contains(actual.repository.as_str())
        {
            return Err(AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch));
        }
    }
    if !request
        .admitted_gradle_graphs
        .iter()
        .map(|graph| graph.workload_id.as_str())
        .eq(WORKLOADS
            .iter()
            .filter(|workload| workload.package_manager == "gradle")
            .map(|workload| workload.id))
    {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch));
    }
    for graph in &request.admitted_gradle_graphs {
        graph.revalidate()?;
    }
    let expected_scanner_outputs = [ProviderId::Rustsec, ProviderId::OwaspNvd]
        .into_iter()
        .flat_map(|provider| {
            WORKLOADS
                .iter()
                .filter(move |workload| match provider {
                    ProviderId::Rustsec => workload.package_manager == "cargo",
                    ProviderId::OwaspNvd => workload.package_manager == "gradle",
                })
                .map(move |workload| (provider, workload.id))
        });
    if !request
        .admitted_scanner_outputs
        .iter()
        .map(|output| (output.provider, output.workload_id.as_str()))
        .eq(expected_scanner_outputs)
    {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch));
    }
    for output in &request.admitted_scanner_outputs {
        output.revalidate()?;
    }
    Ok(())
}

fn validate_manifest(
    manifest: &SnapshotManifest,
    request: &AdmissionRequest,
) -> Result<(), AdvisoryError> {
    if manifest.schema != "radroots.advisory-snapshot.v1"
        || manifest.candidate != request.candidate
        || manifest.sources != request.sources
        || manifest.inventory != request.inventory
        || manifest.inventory_sha256 != inventory_digest(&request.inventory)?
    {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch));
    }
    let (tool_manifest, tool_observation, trace, provider_evidence) =
        validate_trusted_authority(request)?;
    if manifest.step_297_tool_manifest_sha256 != sha256(&request.step_297_tool_manifest)
        || manifest.fresh_tool_observation_sha256 != sha256(&request.fresh_tool_observation)
        || manifest.tool_acquisition != tool_manifest.tool_acquisition
        || manifest.tool_state != tool_observation.tool_state
        || manifest.gradle_projection != provider_evidence.gradle_projection
        || manifest.provider_snapshot != provider_evidence.provider_snapshot
        || manifest.suppressions != provider_evidence.suppressions
        || manifest.candidate_advisory_input_sha256
            != provider_evidence.candidate_advisory_input_sha256
    {
        return Err(AdvisoryError::new(AdvisoryFailureKind::BindingChanged));
    }
    validate_tool_acquisitions(&manifest.tool_acquisition)?;
    validate_tool_states(&manifest.tool_state, request.evaluation_epoch)?;
    validate_gradle_projections(
        &manifest.gradle_projection,
        request,
        &provider_evidence.process_receipt,
    )?;
    if manifest.provider_snapshot.len() != 2
        || manifest.provider_snapshot[0].provider != ProviderId::Rustsec
        || manifest.provider_snapshot[1].provider != ProviderId::OwaspNvd
    {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch));
    }
    for provider in &manifest.provider_snapshot {
        validate_provider(
            provider,
            request,
            &trace,
            &provider_evidence.process_receipt,
        )?;
    }
    validate_temporal_order(
        &tool_observation,
        &manifest.provider_snapshot,
        &provider_evidence.process_receipt,
    )?;
    if manifest.candidate_advisory_input_sha256 != candidate_advisory_input_digest(manifest)? {
        return Err(AdvisoryError::new(AdvisoryFailureKind::BindingChanged));
    }
    Ok(())
}

fn validate_trusted_authority(
    request: &AdmissionRequest,
) -> Result<
    (
        TrustedToolManifest,
        TrustedToolObservation,
        NvdNetworkTrace,
        TrustedProviderEvidence,
    ),
    AdvisoryError,
> {
    if request.producer_request.len() > MAX_PRODUCER_REQUEST_BYTES
        || request.step_297_tool_manifest.len() > MAX_TOOL_MANIFEST_BYTES
        || request.fresh_tool_observation.len() > MAX_TOOL_OBSERVATION_BYTES
        || request.nvd_network_trace.len() > MAX_NVD_TRACE_BYTES
        || request.provider_execution_evidence.len() > MAX_PROVIDER_EVIDENCE_BYTES
    {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot));
    }
    let producer: TrustedProducerRequest = parse_canonical_authority(&request.producer_request)?;
    let producer_digest = sha256(&request.producer_request);
    let tool_manifest: TrustedToolManifest =
        parse_canonical_authority(&request.step_297_tool_manifest)?;
    let observation: TrustedToolObservation =
        parse_canonical_authority(&request.fresh_tool_observation)?;
    let trace: NvdNetworkTrace = parse_canonical_authority(&request.nvd_network_trace)?;
    let provider_evidence: TrustedProviderEvidence =
        parse_canonical_authority(&request.provider_execution_evidence)?;
    let row_projection =
        tool_observation_digest(&tool_manifest.tool_acquisition, &observation.tool_state)?;
    let expected_workloads = WORKLOADS
        .iter()
        .map(|workload| workload.id.to_owned())
        .collect::<Vec<_>>();
    let expected_query_keys = [
        "lastModStartDate",
        "lastModEndDate",
        "startIndex",
        "resultsPerPage",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<Vec<_>>();
    if producer.schema != "radroots.advisory-producer-request.v1"
        || producer.candidate_generation != request.candidate.generation
        || producer.candidate_digest != request.candidate.digest
        || producer.platform != "macos_aarch64"
        || producer.process_contract_sha256 != BOUNDED_PROCESS_DECISION_SHA256
        || producer.process_runner_source_sha256 != BOUNDED_PROCESS_SOURCE_SHA256
        || producer.nvd_endpoint != "/rest/json/cves/2.0"
        || !valid_hex(&producer.nvd_enforcement_program_sha256, 64)
        || !valid_hex(&producer.nvd_enforcement_configuration_sha256, 64)
        || producer.nvd_query_keys != expected_query_keys
        || producer.workload_ids != expected_workloads
        || tool_manifest.schema != "radroots.advisory-tool-manifest-projection.v1"
        || tool_manifest.candidate_generation != request.candidate.generation
        || tool_manifest.candidate_digest != request.candidate.digest
        || tool_manifest.platform != "macos_aarch64"
        || tool_manifest.producer_request_sha256 != producer_digest
        || observation.schema != "radroots.advisory-tool-observation.v1"
        || observation.candidate_generation != request.candidate.generation
        || observation.candidate_digest != request.candidate.digest
        || observation.platform != "macos_aarch64"
        || observation.producer_request_sha256 != producer_digest
        || observation.tool_manifest_sha256 != sha256(&request.step_297_tool_manifest)
        || observation.row_projection_sha256 != row_projection
        || observation.observed_at_epoch > request.evaluation_epoch
        || request
            .evaluation_epoch
            .checked_sub(observation.observed_at_epoch)
            .is_none_or(|age| age > TOOL_REVIEW_SECONDS)
        || trace.schema != "radroots.advisory-nvd-network-trace.v1"
        || trace.candidate_generation != request.candidate.generation
        || trace.candidate_digest != request.candidate.digest
        || trace.producer_request_sha256 != producer_digest
        || trace.enforcement != "deny_by_default_nvd_api_get_only_proxy"
        || trace.enforcement_program_sha256 != producer.nvd_enforcement_program_sha256
        || trace.enforcement_configuration_sha256 != producer.nvd_enforcement_configuration_sha256
        || trace.request.is_empty()
        || trace.request.len() > MAX_NVD_REQUESTS
        || provider_evidence.schema != "radroots.advisory-provider-execution-evidence.v1"
        || provider_evidence.candidate_generation != request.candidate.generation
        || provider_evidence.candidate_digest != request.candidate.digest
        || provider_evidence.platform != "macos_aarch64"
        || provider_evidence.producer_request_sha256 != producer_digest
        || provider_evidence.gradle_projection.len() != 6
        || provider_evidence.provider_snapshot.len() != 2
    {
        return Err(AdvisoryError::new(AdvisoryFailureKind::BindingChanged));
    }
    validate_tool_acquisitions(&tool_manifest.tool_acquisition)?;
    validate_tool_states(&observation.tool_state, request.evaluation_epoch)?;
    validate_process_receipts(&provider_evidence.process_receipt, request.evaluation_epoch)?;
    validate_nvd_trace(&producer, &trace, &provider_evidence.process_receipt)?;
    Ok((tool_manifest, observation, trace, provider_evidence))
}

fn validate_nvd_trace(
    producer: &TrustedProducerRequest,
    trace: &NvdNetworkTrace,
    receipts: &[TrustedProcessReceipt],
) -> Result<(), AdvisoryError> {
    let update = process_receipt(receipts, "owasp_nvd_update")?;
    let mut aggregate_bytes = 0_u64;
    let mut previous_request_completed = None;
    let mut previous_window: Option<(u64, u64, u64, u64, u64)> = None;
    for (index, row) in trace.request.iter().enumerate() {
        let values = row
            .query
            .iter()
            .map(|query| (query.name.as_str(), query.value.as_str()))
            .collect::<BTreeMap<_, _>>();
        let start = values
            .get("lastModStartDate")
            .and_then(|value| parse_report_epoch(value).ok())
            .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot))?;
        let end = values
            .get("lastModEndDate")
            .and_then(|value| parse_report_epoch(value).ok())
            .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot))?;
        let start_index = values
            .get("startIndex")
            .and_then(|value| parse_canonical_u64(value))
            .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot))?;
        let results_per_page = values
            .get("resultsPerPage")
            .and_then(|value| parse_canonical_u64(value))
            .filter(|value| (1..=2_000).contains(value))
            .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot))?;
        aggregate_bytes = aggregate_bytes
            .checked_add(row.response_byte_length)
            .filter(|value| *value <= MAX_NVD_AGGREGATE_BYTES)
            .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot))?;
        if row.sequence != index as u64 + 1
            || row.method != "GET"
            || row.scheme != "https"
            || row.authority != "services.nvd.nist.gov"
            || row.path != producer.nvd_endpoint
            || row.query.len() != producer.nvd_query_keys.len()
            || !row
                .query
                .iter()
                .map(|query| query.name.as_str())
                .eq(producer.nvd_query_keys.iter().map(String::as_str))
            || row.query.iter().any(|query| {
                !valid_bounded_text(&query.value, 128) || query.name.eq_ignore_ascii_case("apiKey")
            })
            || start > end
            || row.started_at_epoch < update.started_at_epoch
            || previous_request_completed.is_some_and(|completed| completed > row.started_at_epoch)
            || row.completed_at_epoch < row.started_at_epoch
            || row.completed_at_epoch > update.completed_at_epoch
            || row.response_status != 200
            || row.response_byte_length == 0
            || row.response_byte_length > MAX_NVD_RESPONSE_BYTES
            || !valid_hex(&row.response_sha256, 64)
            || row.response_start_index != start_index
            || row.response_results_per_page != results_per_page
            || row.response_total_results == 0
            || start_index >= row.response_total_results
        {
            return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot));
        }
        if let Some((previous_start, previous_end, previous_index, previous_page, previous_total)) =
            previous_window
        {
            if start == previous_start && end == previous_end {
                if start_index != previous_index.saturating_add(previous_page)
                    || row.response_total_results != previous_total
                {
                    return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot));
                }
            } else if start != previous_end.saturating_add(1) || start_index != 0 {
                return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot));
            }
        } else if start_index != 0 {
            return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot));
        }
        let page_complete = start_index
            .checked_add(results_per_page)
            .is_some_and(|next| next >= row.response_total_results);
        let next_same_window = trace.request.get(index + 1).is_some_and(|next| {
            let next_values = next
                .query
                .iter()
                .map(|query| (query.name.as_str(), query.value.as_str()))
                .collect::<BTreeMap<_, _>>();
            next_values.get("lastModStartDate") == values.get("lastModStartDate")
                && next_values.get("lastModEndDate") == values.get("lastModEndDate")
        });
        if page_complete == next_same_window {
            return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot));
        }
        previous_request_completed = Some(row.completed_at_epoch);
        previous_window = Some((
            start,
            end,
            start_index,
            results_per_page,
            row.response_total_results,
        ));
    }
    Ok(())
}

fn parse_canonical_u64(value: &str) -> Option<u64> {
    if value.is_empty()
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        return None;
    }
    value.parse().ok()
}

fn expected_process_receipt_ids() -> Vec<String> {
    let mut ids = vec!["owasp_nvd_update".to_owned()];
    ids.extend(
        WORKLOADS
            .iter()
            .filter(|workload| workload.package_manager == "gradle")
            .map(|workload| format!("gradle:{}", workload.id)),
    );
    ids.extend(
        WORKLOADS
            .iter()
            .filter(|workload| workload.package_manager == "cargo")
            .map(|workload| format!("cargo_audit:{}", workload.id)),
    );
    ids.extend(
        WORKLOADS
            .iter()
            .filter(|workload| workload.package_manager == "gradle")
            .map(|workload| format!("owasp_analysis:{}", workload.id)),
    );
    ids
}

fn validate_process_receipts(
    receipts: &[TrustedProcessReceipt],
    evaluation_epoch: u64,
) -> Result<(), AdvisoryError> {
    if !receipts
        .iter()
        .map(|receipt| receipt.id.as_str())
        .eq(expected_process_receipt_ids().iter().map(String::as_str))
    {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch));
    }
    let mut previous_completed = None;
    let mut working_directory_identities = BTreeSet::new();
    let mut private_environment_values = BTreeSet::new();
    let mut shared_environment_values = BTreeMap::<String, String>::new();
    for receipt in receipts {
        require_complete(receipt.state)?;
        validate_process_environment(
            &receipt.id,
            &receipt.environment,
            &mut private_environment_values,
            &mut shared_environment_values,
        )?;
        if !valid_exact_identity_token(&receipt.id, 256)
            || !valid_hex(&receipt.program_sha256, 64)
            || receipt.runtime_sha256 != expected_runtime_sha256(&receipt.id)
            || !valid_hex(&receipt.arguments_sha256, 64)
            || receipt.environment_sha256 != process_environment_digest(&receipt.environment)?
            || validate_process_working_directory(&receipt.id, &receipt.working_directory).is_err()
            || !working_directory_identities
                .insert(receipt.working_directory.identity_sha256.clone())
            || receipt.working_directory_sha256
                != process_working_directory_digest(&receipt.working_directory)?
            || receipt
                .path_binding
                .windows(2)
                .any(|rows| rows[0].argument_index >= rows[1].argument_index)
            || receipt.path_binding.iter().any(|binding| {
                !valid_exact_identity_token(&binding.logical_role, 128)
                    || !valid_hex(&binding.identity_sha256, 64)
            })
            || !receipt.stdin_closed
            || receipt.deadline_seconds != ANALYSIS_DEADLINE_SECONDS
            || receipt.started_at_epoch == 0
            || receipt.completed_at_epoch < receipt.started_at_epoch
            || previous_completed.is_some_and(|previous| previous > receipt.started_at_epoch)
            || receipt.completed_at_epoch > evaluation_epoch
            || receipt.stdout_byte_length > 67_108_864
            || receipt.stderr_byte_length > 67_108_864
            || !valid_hex(&receipt.stdout_sha256, 64)
            || !valid_hex(&receipt.stderr_sha256, 64)
            || !valid_hex(&receipt.input_sha256, 64)
            || !valid_hex(&receipt.output_sha256, 64)
        {
            return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot));
        }
        previous_completed = Some(receipt.completed_at_epoch);
    }
    if private_environment_values
        .iter()
        .any(|value| working_directory_identities.contains(value))
    {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot));
    }
    Ok(())
}

fn expected_process_environment(id: &str) -> Result<Vec<ProcessEnvironmentRow>, AdvisoryError> {
    expected_environment_names(id)
        .map(|name| {
            let logical_value = expected_environment_logical_value(name)?;
            let synthetic_value = match name {
                "LC_ALL" => "C".to_owned(),
                "TZ" => "UTC".to_owned(),
                "JAVA_HOME" => "/governed/jdk-21.0.12.1".to_owned(),
                "PATH" => "/governed/tool-manifest/bin".to_owned(),
                _ => format!("/private/{id}/{name}"),
            };
            Ok(ProcessEnvironmentRow {
                name: name.to_owned(),
                logical_value: logical_value.to_owned(),
                value_sha256: sha256(synthetic_value.as_bytes()),
            })
        })
        .collect()
}

fn expected_environment_logical_value(name: &str) -> Result<&'static str, AdvisoryError> {
    match name {
        "CARGO_HOME" => Ok("private_empty_cargo_home"),
        "GRADLE_USER_HOME" => Ok("private_seeded_gradle_user_home"),
        "HOME" => Ok("private_empty_home"),
        "JAVA_HOME" => Ok("pinned_java_21_0_12_1_home"),
        "LC_ALL" => Ok("C"),
        "PATH" => Ok("pinned_tool_manifest_only_path"),
        "TMPDIR" => Ok("private_owner_only_attempt_tmp"),
        "TZ" => Ok("UTC"),
        _ => Err(AdvisoryError::new(AdvisoryFailureKind::InvalidContract)),
    }
}

fn validate_process_environment(
    id: &str,
    environment: &[ProcessEnvironmentRow],
    private_values: &mut BTreeSet<String>,
    shared_values: &mut BTreeMap<String, String>,
) -> Result<(), AdvisoryError> {
    let expected_names = expected_environment_names(id).collect::<Vec<_>>();
    if environment.len() != expected_names.len() {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot));
    }
    for (row, expected_name) in environment.iter().zip(expected_names) {
        let expected_logical_value = expected_environment_logical_value(expected_name)?;
        if row.name != expected_name
            || row.logical_value != expected_logical_value
            || !valid_hex(&row.value_sha256, 64)
            || match expected_name {
                "LC_ALL" => row.value_sha256 != sha256(b"C"),
                "TZ" => row.value_sha256 != sha256(b"UTC"),
                "JAVA_HOME" | "PATH" => {
                    shared_values
                        .entry(expected_name.to_owned())
                        .or_insert_with(|| row.value_sha256.clone())
                        != &row.value_sha256
                }
                _ => !private_values.insert(row.value_sha256.clone()),
            }
        {
            return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot));
        }
    }
    Ok(())
}

fn expected_process_working_directory(id: &str) -> Result<ProcessWorkingDirectory, AdvisoryError> {
    let (logical_uri, kind, pre_entries, post_entries) =
        if let Some(workload) = id.strip_prefix("cargo_audit:") {
            (
                format!("rshr-private-cargo-audit://{workload}"),
                "fresh_empty_config_free",
                0,
                0,
            )
        } else if let Some(workload) = id.strip_prefix("gradle:") {
            (
                format!("rshr-candidate-gradle-build://{workload}"),
                "retained_candidate_source",
                1,
                1,
            )
        } else if let Some(workload) = id.strip_prefix("owasp_analysis:") {
            (
                format!("rshr-private-owasp-analysis://{workload}"),
                "fresh_private_analysis",
                0,
                0,
            )
        } else if id == "owasp_nvd_update" {
            (
                "rshr-private-owasp-nvd-update://attempt".to_owned(),
                "fresh_private_update",
                0,
                1,
            )
        } else {
            return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidContract));
        };
    let empty_tree = empty_directory_tree_sha256()?;
    let nonempty_tree = domain_json_digest(
        b"radroots-advisory-synthetic-working-directory-tree-v1\0",
        &serde_json::json!({"kind": kind, "process_id": id}),
    )?;
    Ok(ProcessWorkingDirectory {
        identity_sha256: domain_json_digest(
            b"radroots-advisory-synthetic-retained-directory-identity-v1\0",
            &serde_json::json!({"kind": kind, "logical_uri": logical_uri, "process_id": id}),
        )?,
        logical_uri,
        kind: kind.to_owned(),
        pre_execution_entry_count: pre_entries,
        pre_execution_tree_sha256: if pre_entries == 0 {
            empty_tree.clone()
        } else {
            nonempty_tree.clone()
        },
        post_execution_entry_count: post_entries,
        post_execution_tree_sha256: if post_entries == 0 {
            empty_tree
        } else {
            nonempty_tree
        },
    })
}

fn validate_process_working_directory(
    id: &str,
    directory: &ProcessWorkingDirectory,
) -> Result<(), AdvisoryError> {
    let expected = expected_process_working_directory(id)?;
    if directory.logical_uri != expected.logical_uri
        || directory.kind != expected.kind
        || !valid_hex(&directory.identity_sha256, 64)
        || !valid_hex(&directory.pre_execution_tree_sha256, 64)
        || !valid_hex(&directory.post_execution_tree_sha256, 64)
        || match directory.kind.as_str() {
            "fresh_empty_config_free" | "fresh_private_analysis" => {
                directory.pre_execution_entry_count != 0
                    || directory.post_execution_entry_count != 0
                    || directory.pre_execution_tree_sha256 != empty_directory_tree_sha256()?
                    || directory.post_execution_tree_sha256 != empty_directory_tree_sha256()?
            }
            "retained_candidate_source" => {
                directory.pre_execution_entry_count == 0
                    || directory.pre_execution_entry_count != directory.post_execution_entry_count
                    || directory.pre_execution_tree_sha256 != directory.post_execution_tree_sha256
            }
            "fresh_private_update" => {
                directory.pre_execution_entry_count != 0
                    || directory.post_execution_entry_count == 0
                    || directory.pre_execution_tree_sha256 != empty_directory_tree_sha256()?
            }
            _ => true,
        }
    {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot));
    }
    Ok(())
}

fn empty_directory_tree_sha256() -> Result<String, AdvisoryError> {
    domain_json_digest(
        b"radroots-advisory-empty-directory-tree-v1\0",
        &Vec::<Value>::new(),
    )
}

fn expected_environment_names(id: &str) -> impl Iterator<Item = &'static str> {
    let names: &'static [&'static str] = if id.starts_with("cargo_audit:") {
        &["CARGO_HOME", "HOME", "LC_ALL", "PATH", "TMPDIR", "TZ"]
    } else if id.starts_with("gradle:") {
        &[
            "GRADLE_USER_HOME",
            "HOME",
            "JAVA_HOME",
            "LC_ALL",
            "PATH",
            "TMPDIR",
            "TZ",
        ]
    } else {
        &["HOME", "JAVA_HOME", "LC_ALL", "PATH", "TMPDIR", "TZ"]
    };
    names.iter().copied()
}

fn expected_runtime_sha256(id: &str) -> Vec<String> {
    let values = if id.starts_with("cargo_audit:") {
        vec![TOOL_PINS[0].executable_sha256]
    } else if id.starts_with("gradle:") {
        vec![
            TOOL_PINS[3].executable_sha256,
            TOOL_PINS[2].executable_sha256,
        ]
    } else {
        vec![
            TOOL_PINS[1].executable_sha256,
            TOOL_PINS[2].executable_sha256,
        ]
    };
    values.into_iter().map(str::to_owned).collect()
}

fn process_environment_digest(
    environment: &[ProcessEnvironmentRow],
) -> Result<String, AdvisoryError> {
    domain_json_digest(b"radroots-advisory-process-environment-v1\0", environment)
}

fn process_working_directory_digest(
    working_directory: &ProcessWorkingDirectory,
) -> Result<String, AdvisoryError> {
    domain_json_digest(
        b"radroots-advisory-process-working-directory-v1\0",
        working_directory,
    )
}

fn process_receipt<'a>(
    receipts: &'a [TrustedProcessReceipt],
    id: &str,
) -> Result<&'a TrustedProcessReceipt, AdvisoryError> {
    receipts
        .iter()
        .find(|receipt| receipt.id == id)
        .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch))
}

fn process_receipt_digest(receipt: &TrustedProcessReceipt) -> Result<String, AdvisoryError> {
    domain_json_digest(b"radroots-advisory-bounded-process-receipt-v1\0", receipt)
}

fn parse_canonical_authority<T>(bytes: &[u8]) -> Result<T, AdvisoryError>
where
    T: for<'de> Deserialize<'de>,
{
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot))?;
    if canonical_json(&value) != bytes {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot));
    }
    serde_json::from_value(value)
        .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot))
}

fn validate_tool_acquisitions(actual: &[ToolAcquisition]) -> Result<(), AdvisoryError> {
    if actual.len() != TOOL_PINS.len() {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch));
    }
    for (tool, pin) in actual.iter().zip(TOOL_PINS) {
        if tool.id != pin.id
            || sha256(&canonical_json_without_lf(&tool.projection)) != pin.projection_sha256
        {
            return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidContract));
        }
    }
    Ok(())
}

fn validate_tool_states(actual: &[ToolState], evaluation_epoch: u64) -> Result<(), AdvisoryError> {
    if actual.len() != TOOL_PINS.len() {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch));
    }
    for (state, pin) in actual.iter().zip(TOOL_PINS) {
        if state.id != pin.id
            || state.normalized_version != pin.version
            || state.executable_sha256 != pin.executable_sha256
            || state.source_sha256 != pin.source_sha256
            || state.package_receipt_sha256 != pin.receipt_sha256
            || state.reviewed_at_epoch == 0
        {
            return Err(AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch));
        }
        require_complete(state.state)?;
        let age = evaluation_epoch
            .checked_sub(state.reviewed_at_epoch)
            .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::StaleSnapshot))?;
        if age > TOOL_REVIEW_SECONDS {
            return Err(AdvisoryError::new(AdvisoryFailureKind::StaleSnapshot));
        }
    }
    Ok(())
}

fn validate_gradle_projections(
    projections: &[GradleProjection],
    request: &AdmissionRequest,
    receipts: &[TrustedProcessReceipt],
) -> Result<(), AdvisoryError> {
    if sha256(GRADLE_INIT_SCRIPT_BYTES) != GRADLE_INIT_SCRIPT_SHA256 {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidContract));
    }
    let expected = WORKLOADS
        .iter()
        .filter(|workload| workload.package_manager == "gradle")
        .collect::<Vec<_>>();
    if projections.len() != expected.len() {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch));
    }
    let harvest_source = request
        .sources
        .iter()
        .find(|source| source.repository == "oss/harvestcircle")
        .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch))?;
    for ((projection, authority), admitted) in projections
        .iter()
        .zip(expected)
        .zip(&request.admitted_gradle_graphs)
    {
        let inventory = request
            .inventory
            .iter()
            .find(|row| row.id == authority.id)
            .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch))?;
        require_complete(projection.state)?;
        let receipt = process_receipt(receipts, &format!("gradle:{}", authority.id))?;
        admitted.revalidate()?;
        if !admitted.matches_projection(projection)
            || projection.workload_id != authority.id
            || projection.raw_graph_byte_length == 0
            || projection.raw_graph_byte_length > MAX_GRADLE_GRAPH_BYTES
            || !valid_hex(&projection.raw_graph_sha256, 64)
            || projection.init_script_sha256 != GRADLE_INIT_SCRIPT_SHA256
            || projection.wrapper_arguments != expected_gradle_arguments(authority)?
            || projection.environment_keys
                != [
                    "GRADLE_USER_HOME",
                    "HOME",
                    "JAVA_HOME",
                    "LC_ALL",
                    "PATH",
                    "TMPDIR",
                    "TZ",
                ]
            || !valid_hex(&projection.environment_sha256, 64)
            || projection.exit_code != 0
            || projection.source_revision != harvest_source.revision
            || projection.source_tree != harvest_source.tree
            || projection.input_sha256
                != gradle_candidate_input_digest(request, inventory, harvest_source, projection)?
            || projection.dependency_count != inventory.dependency_count
            || projection.component_count != projection.components.len() as u64
            || projection.edge_count != projection.edges.len() as u64
            || projection.artifact_count != projection.artifacts.len() as u64
            || projection.component_count == 0
            || !valid_hex(&projection.materialized_tree_sha256, 64)
            || !valid_hex(&projection.artifact_source_roots_sha256, 64)
            || !valid_hex(&projection.seed_cache_inventory_sha256, 64)
            || projection.wrapper_distribution_sha256
                != "553c78f50dafcd54d65b9a444649057857469edf836431389695608536d6b746"
            || projection.process_receipt_sha256 != process_receipt_digest(receipt)?
            || projection.canonical_graph_sha256 != gradle_graph_digest(projection)?
            || projection.normalization_receipt_sha256
                != gradle_normalization_receipt_digest(
                    &projection.workload_id,
                    projection.raw_graph_byte_length,
                    &projection.raw_graph_sha256,
                    &projection.canonical_graph_sha256,
                    &projection.materialized_tree_sha256,
                    &projection.artifact_source_roots_sha256,
                )?
            || receipt.program_sha256 != TOOL_PINS[3].executable_sha256
            || receipt.arguments_sha256
                != domain_json_digest(
                    b"radroots-advisory-process-arguments-v1\0",
                    &projection.wrapper_arguments,
                )?
            || receipt.environment_sha256 != projection.environment_sha256
            || receipt.input_sha256 != gradle_process_input_digest(projection)?
            || receipt.output_sha256 != gradle_process_output_digest(projection)?
            || receipt.path_binding
                != vec![
                    ProcessPathBinding {
                        argument_index: 5,
                        logical_role: "gradle_build_root".to_owned(),
                        identity_sha256: gradle_build_root_binding_digest(projection)?,
                    },
                    ProcessPathBinding {
                        argument_index: 7,
                        logical_role: "gradle_init_script".to_owned(),
                        identity_sha256: projection.init_script_sha256.clone(),
                    },
                    ProcessPathBinding {
                        argument_index: 10,
                        logical_role: "gradle_projection_output".to_owned(),
                        identity_sha256: projection.raw_graph_sha256.clone(),
                    },
                ]
            || receipt.exit_code != projection.exit_code
        {
            return Err(AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch));
        }
        let mut component_ids = BTreeSet::new();
        let mut component_by_id = BTreeMap::new();
        let mut previous_component = None::<&str>;
        for component in &projection.components {
            if previous_component
                .is_some_and(|previous| previous >= component.identity_sha256.as_str())
                || component.identity_sha256 != gradle_component_digest(component)?
                || component.variant_sha256 != gradle_variant_digest(&component.variant)?
                || !valid_gradle_variant_envelope(&component.variant)
                || !valid_gradle_component(component)
                || !component_ids.insert(component.identity_sha256.as_str())
            {
                return Err(AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch));
            }
            component_by_id.insert(component.identity_sha256.as_str(), component);
            previous_component = Some(&component.identity_sha256);
        }
        let roots = projection
            .components
            .iter()
            .filter(|component| {
                component.root
                    && component.kind == "project"
                    && component.build_root.as_deref() == Some(authority.build_root)
                    && component.project_path.as_deref() == authority.project_path
            })
            .collect::<Vec<_>>();
        if roots.len() != 1 {
            return Err(AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch));
        }
        let mut previous_edge = None::<Vec<u8>>;
        for edge in &projection.edges {
            let key = canonical_row_key(edge)?;
            if previous_edge
                .as_ref()
                .is_some_and(|previous| previous >= &key)
                || !component_ids.contains(edge.from_identity_sha256.as_str())
                || !component_ids.contains(edge.to_identity_sha256.as_str())
                || edge.from_identity_sha256 == edge.to_identity_sha256
                || edge.requested_sha256 != gradle_request_digest(&edge.requested)?
                || edge.selected_variant_sha256 != gradle_variant_digest(&edge.selected_variant)?
                || !valid_gradle_request(&edge.requested)
                || !valid_gradle_variant(&edge.selected_variant)
            {
                return Err(AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch));
            }
            previous_edge = Some(key);
        }
        let mut reachable = BTreeSet::from([roots[0].identity_sha256.as_str()]);
        loop {
            let before = reachable.len();
            for edge in &projection.edges {
                if reachable.contains(edge.from_identity_sha256.as_str()) {
                    reachable.insert(edge.to_identity_sha256.as_str());
                }
            }
            if reachable.len() == before {
                break;
            }
        }
        if reachable.len() != projection.components.len() {
            return Err(AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch));
        }
        let mut previous = None::<Vec<u8>>;
        let mut materialized = BTreeMap::<(&str, &str), (&str, u64)>::new();
        for artifact in &projection.artifacts {
            let key = canonical_row_key(artifact)?;
            if previous.as_ref().is_some_and(|prior| prior >= &key)
                || !component_ids.contains(artifact.component_identity_sha256.as_str())
                || !valid_bounded_text(&artifact.component, 512)
                || artifact.package_ecosystem != "maven"
                || !valid_bounded_text(&artifact.package_namespace, 256)
                || !valid_bounded_text(&artifact.package_name, 256)
                || !valid_bounded_text(&artifact.package_version, 128)
                || !valid_hex(&artifact.artifact_sha256, 64)
                || artifact.variant_sha256 != gradle_variant_digest(&artifact.variant)?
                || !valid_gradle_variant(&artifact.variant)
                || artifact.byte_length == 0
                || !valid_bounded_text(&artifact.artifact_name, 256)
                || !valid_bounded_text(&artifact.logical_name, 256)
                || !valid_bounded_text(&artifact.artifact_type, 64)
                || artifact
                    .classifier
                    .as_deref()
                    .is_some_and(|classifier| !valid_bounded_text(classifier, 128))
                || !valid_artifact_extension(&artifact.extension)
                || artifact.materialized_name
                    != format!("{}.{}", artifact.artifact_sha256, artifact.extension)
            {
                return Err(AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch));
            }
            let component = component_by_id
                .get(artifact.component_identity_sha256.as_str())
                .copied()
                .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch))?;
            if !artifact_matches_component(artifact, component, &harvest_source.revision)
                || !projection.edges.iter().any(|edge| {
                    edge.to_identity_sha256 == artifact.component_identity_sha256
                        && edge.selected_variant_sha256 == artifact.variant_sha256
                })
            {
                return Err(AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch));
            }
            let materialized_key = (
                artifact.artifact_sha256.as_str(),
                artifact.extension.as_str(),
            );
            if let Some((name, length)) = materialized.insert(
                materialized_key,
                (artifact.materialized_name.as_str(), artifact.byte_length),
            ) && (name != artifact.materialized_name || length != artifact.byte_length)
            {
                return Err(AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch));
            }
            previous = Some(key);
        }
        admitted.revalidate()?;
    }
    Ok(())
}

fn expected_gradle_arguments(workload: &WorkloadAuthority) -> Result<Vec<String>, AdvisoryError> {
    let project = workload
        .project_path
        .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidContract))?;
    let configuration = workload
        .configuration
        .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidContract))?;
    Ok([
        "--offline".to_owned(),
        "--no-daemon".to_owned(),
        "--no-parallel".to_owned(),
        "--no-configuration-cache".to_owned(),
        "--project-dir".to_owned(),
        workload.build_root.to_owned(),
        "--init-script".to_owned(),
        "{private_evidenced_init_script}".to_owned(),
        format!("{project}:rshrAdvisoryGraph"),
        format!("-PrshrAdvisoryConfiguration={configuration}"),
        "-PrshrAdvisoryOutput={private_create_new_projection}".to_owned(),
    ]
    .into_iter()
    .collect())
}

fn gradle_graph_digest(projection: &GradleProjection) -> Result<String, AdvisoryError> {
    gradle_graph_digest_parts(
        &projection.workload_id,
        &projection.components,
        &projection.edges,
        &projection.artifacts,
    )
}

fn gradle_graph_digest_parts(
    workload_id: &str,
    components: &[GradleComponent],
    edges: &[GradleEdge],
    artifacts: &[GradleArtifact],
) -> Result<String, AdvisoryError> {
    domain_json_digest(
        b"radroots-advisory-gradle-graph-v1\0",
        &serde_json::json!({
            "artifacts": artifacts,
            "components": components,
            "edges": edges,
            "workload_id": workload_id,
        }),
    )
}

fn canonical_row_key<T: Serialize>(value: &T) -> Result<Vec<u8>, AdvisoryError> {
    let value = serde_json::to_value(value)
        .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot))?;
    Ok(canonical_json_without_lf(&value))
}

fn gradle_process_input_digest(projection: &GradleProjection) -> Result<String, AdvisoryError> {
    domain_json_digest(
        b"radroots-advisory-gradle-process-input-v1\0",
        &serde_json::json!({
            "artifact_source_roots_sha256": projection.artifact_source_roots_sha256,
            "init_script_sha256": projection.init_script_sha256,
            "input_sha256": projection.input_sha256,
            "seed_cache_inventory_sha256": projection.seed_cache_inventory_sha256,
            "source_revision": projection.source_revision,
            "source_tree": projection.source_tree,
            "wrapper_distribution_sha256": projection.wrapper_distribution_sha256,
            "workload_id": projection.workload_id,
        }),
    )
}

fn gradle_build_root_binding_digest(
    projection: &GradleProjection,
) -> Result<String, AdvisoryError> {
    domain_json_digest(
        b"radroots-advisory-gradle-build-root-binding-v1\0",
        &serde_json::json!({
            "source_revision": projection.source_revision,
            "source_tree": projection.source_tree,
            "workload_id": projection.workload_id,
        }),
    )
}

fn gradle_candidate_input_digest(
    request: &AdmissionRequest,
    inventory: &WorkloadInventory,
    source: &SourceIdentity,
    projection: &GradleProjection,
) -> Result<String, AdvisoryError> {
    domain_json_digest(
        b"radroots-advisory-gradle-candidate-input-v1\0",
        &serde_json::json!({
            "artifact_source_roots_sha256": projection.artifact_source_roots_sha256,
            "candidate": request.candidate,
            "dependency_control_sha256": inventory.input_sha256,
            "fresh_tool_observation_sha256": sha256(&request.fresh_tool_observation),
            "init_script_sha256": projection.init_script_sha256,
            "seed_cache_inventory_sha256": projection.seed_cache_inventory_sha256,
            "source": source,
            "step_297_tool_manifest_sha256": sha256(&request.step_297_tool_manifest),
            "workload": inventory,
            "wrapper_distribution_sha256": projection.wrapper_distribution_sha256,
        }),
    )
}

fn gradle_process_output_digest(projection: &GradleProjection) -> Result<String, AdvisoryError> {
    domain_json_digest(
        b"radroots-advisory-gradle-process-output-v1\0",
        &serde_json::json!({
            "raw_graph_byte_length": projection.raw_graph_byte_length,
            "raw_graph_sha256": projection.raw_graph_sha256,
            "workload_id": projection.workload_id,
        }),
    )
}

fn gradle_normalization_receipt_digest(
    workload_id: &str,
    raw_graph_byte_length: u64,
    raw_graph_sha256: &str,
    canonical_graph_sha256: &str,
    materialized_tree_sha256: &str,
    artifact_source_roots_sha256: &str,
) -> Result<String, AdvisoryError> {
    domain_json_digest(
        b"radroots-advisory-gradle-normalization-v1\0",
        &serde_json::json!({
            "artifact_source_roots_sha256": artifact_source_roots_sha256,
            "canonical_graph_sha256": canonical_graph_sha256,
            "materialized_tree_sha256": materialized_tree_sha256,
            "raw_graph_byte_length": raw_graph_byte_length,
            "raw_graph_sha256": raw_graph_sha256,
            "workload_id": workload_id,
        }),
    )
}

fn gradle_component_digest(component: &GradleComponent) -> Result<String, AdvisoryError> {
    domain_json_digest(
        b"radroots-advisory-gradle-component-v1\0",
        &serde_json::json!({
            "build_root": component.build_root,
            "group": component.group,
            "kind": component.kind,
            "name": component.name,
            "project_path": component.project_path,
            "root": component.root,
            "variant": component.variant,
            "version": component.version,
        }),
    )
}

fn gradle_variant_digest(variant: &Value) -> Result<String, AdvisoryError> {
    domain_json_digest(b"radroots-advisory-gradle-variant-v1\0", variant)
}

fn gradle_request_digest(request: &Value) -> Result<String, AdvisoryError> {
    domain_json_digest(b"radroots-advisory-gradle-request-v1\0", request)
}

fn valid_gradle_variant(value: &Value) -> bool {
    serde_json::from_value::<RawGradleVariant>(value.clone())
        .ok()
        .is_some_and(|variant| validate_raw_variant(&variant, 0).is_ok())
}

fn valid_gradle_variant_envelope(value: &Value) -> bool {
    serde_json::from_value::<RawGradleVariantEnvelope>(value.clone())
        .ok()
        .is_some_and(|envelope| validate_raw_variant_envelope(&envelope).is_ok())
}

fn valid_gradle_request(value: &Value) -> bool {
    serde_json::from_value::<RawGradleSelector>(value.clone())
        .ok()
        .is_some_and(|selector| validate_raw_selector(&selector).is_ok())
}

fn validate_raw_variant_envelope(envelope: &RawGradleVariantEnvelope) -> Result<(), AdvisoryError> {
    if envelope.selected.is_empty() || envelope.selected.len() > MAX_GRADLE_VARIANTS {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch));
    }
    require_strict_canonical_order(&envelope.selected)?;
    for variant in &envelope.selected {
        validate_raw_variant(variant, 0)?;
    }
    Ok(())
}

fn validate_raw_variant(variant: &RawGradleVariant, depth: usize) -> Result<(), AdvisoryError> {
    if depth > MAX_GRADLE_EXTERNAL_VARIANT_DEPTH
        || variant.attributes.len() > MAX_GRADLE_ATTRIBUTES
        || variant.capabilities.len() > MAX_GRADLE_CAPABILITIES
    {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch));
    }
    require_strict_canonical_order(&variant.attributes)?;
    require_strict_canonical_order(&variant.capabilities)?;
    let mut attribute_names = BTreeSet::new();
    for attribute in &variant.attributes {
        if !valid_ascii_text(&attribute.name, 512, false)
            || !valid_ascii_text(&attribute.value, 512, true)
            || !attribute_names.insert(attribute.name.as_str())
        {
            return Err(AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch));
        }
    }
    for capability in &variant.capabilities {
        if !valid_ascii_text(&capability.group, 256, true)
            || !valid_ascii_text(&capability.name, 256, false)
            || capability
                .version
                .as_deref()
                .is_some_and(|value| !valid_ascii_text(value, 128, true))
        {
            return Err(AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch));
        }
    }
    if let Some(external) = &variant.external_variant {
        validate_raw_variant(external, depth.saturating_add(1))?;
    }
    Ok(())
}

fn validate_raw_selector(selector: &RawGradleSelector) -> Result<(), AdvisoryError> {
    if selector.attributes.len() > MAX_GRADLE_ATTRIBUTES
        || selector.capabilities.len() > MAX_GRADLE_CAPABILITIES
    {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch));
    }
    require_strict_canonical_order(&selector.attributes)?;
    require_strict_canonical_order(&selector.capabilities)?;
    let variant = RawGradleVariant {
        attributes: selector.attributes.clone(),
        capabilities: selector.capabilities.clone(),
        external_variant: None,
    };
    validate_raw_variant(&variant, 0)?;
    match selector.kind.as_str() {
        "module" => {
            if selector
                .group
                .as_deref()
                .is_none_or(|value| !valid_ascii_text(value, 256, false))
                || selector
                    .name
                    .as_deref()
                    .is_none_or(|value| !valid_ascii_text(value, 256, false))
                || selector
                    .version
                    .as_deref()
                    .is_none_or(|value| !valid_ascii_text(value, 512, true))
                || selector.build_root.is_some()
                || selector.project_path.is_some()
            {
                return Err(AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch));
            }
            let constraint = selector
                .version_constraint
                .as_ref()
                .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch))?;
            if constraint.rejected.len() > MAX_GRADLE_REJECTED_VERSIONS
                || constraint
                    .branch
                    .as_deref()
                    .is_some_and(|value| !valid_ascii_text(value, 256, true))
                || !valid_ascii_text(&constraint.preferred, 512, true)
                || !valid_ascii_text(&constraint.required, 512, true)
                || !valid_ascii_text(&constraint.strict, 512, true)
                || constraint
                    .rejected
                    .iter()
                    .any(|value| !valid_ascii_text(value, 512, false))
                || constraint
                    .rejected
                    .windows(2)
                    .any(|window| window[0] >= window[1])
            {
                return Err(AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch));
            }
        }
        "project" => {
            if selector.group.is_some()
                || selector.name.is_some()
                || selector.version.is_some()
                || selector.version_constraint.is_some()
                || selector
                    .build_root
                    .as_deref()
                    .is_none_or(|value| !valid_gradle_build_root(value))
                || selector
                    .project_path
                    .as_deref()
                    .is_none_or(|value| !valid_gradle_project_path(value))
            {
                return Err(AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch));
            }
        }
        _ => {
            return Err(AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch));
        }
    }
    Ok(())
}

fn validate_raw_component_core(core: &RawGradleComponentCore) -> Result<(), AdvisoryError> {
    match core.kind.as_str() {
        "module"
            if core
                .group
                .as_deref()
                .is_some_and(|value| valid_ascii_text(value, 256, false))
                && core
                    .name
                    .as_deref()
                    .is_some_and(|value| valid_ascii_text(value, 256, false))
                && core
                    .version
                    .as_deref()
                    .is_some_and(|value| valid_ascii_text(value, 128, false))
                && core.build_root.is_none()
                && core.project_path.is_none() =>
        {
            Ok(())
        }
        "project"
            if core.group.is_none()
                && core.name.is_none()
                && core.version.is_none()
                && core
                    .build_root
                    .as_deref()
                    .is_some_and(valid_gradle_build_root)
                && core
                    .project_path
                    .as_deref()
                    .is_some_and(valid_gradle_project_path) =>
        {
            Ok(())
        }
        _ => Err(AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch)),
    }
}

fn require_strict_canonical_order<T: Serialize>(values: &[T]) -> Result<(), AdvisoryError> {
    let mut previous = None::<Vec<u8>>;
    for value in values {
        let key = canonical_row_key(value)?;
        if previous.as_ref().is_some_and(|prior| prior >= &key) {
            return Err(AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch));
        }
        previous = Some(key);
    }
    Ok(())
}

fn valid_ascii_text(value: &str, maximum: usize, allow_empty: bool) -> bool {
    (allow_empty || !value.is_empty())
        && value.len() <= maximum
        && value
            .bytes()
            .all(|byte| byte.is_ascii() && !byte.is_ascii_control())
}

fn valid_gradle_build_root(value: &str) -> bool {
    matches!(value, "." | "build-logic")
}

fn valid_gradle_project_path(value: &str) -> bool {
    valid_ascii_text(value, 256, false)
        && value.starts_with(':')
        && !value.ends_with(':')
        && value
            .split(':')
            .skip(1)
            .all(|component| !component.is_empty() && valid_ascii_text(component, 128, false))
}

fn artifact_matches_component(
    artifact: &GradleArtifact,
    component: &GradleComponent,
    _source_revision: &str,
) -> bool {
    match component.kind.as_str() {
        "module" => {
            let (Some(group), Some(name), Some(version)) = (
                component.group.as_deref(),
                component.name.as_deref(),
                component.version.as_deref(),
            ) else {
                return false;
            };
            artifact.component == format!("{group}:{name}:{version}")
                && artifact.package_ecosystem == "maven"
                && artifact.package_namespace == group
                && artifact.package_name == name
                && artifact.package_version == version
        }
        "project" => {
            let (Some(root), Some(project)) = (
                component.build_root.as_deref(),
                component.project_path.as_deref(),
            ) else {
                return false;
            };
            artifact.component == format!("{root}:{project}")
                && artifact.package_ecosystem == "maven"
                && valid_project_package_identity(
                    root,
                    project,
                    &artifact.package_namespace,
                    &artifact.package_name,
                    &artifact.package_version,
                )
                && artifact.classifier.is_none()
        }
        _ => false,
    }
}

fn valid_project_module_version(
    build_root: &str,
    project_path: &str,
    module_version: &RawGradleModuleVersion,
    source_revision: &str,
) -> bool {
    valid_hex(source_revision, 40)
        && valid_project_package_identity(
            build_root,
            project_path,
            &module_version.group,
            &module_version.name,
            &module_version.version,
        )
}

fn valid_project_package_identity(
    build_root: &str,
    project_path: &str,
    group: &str,
    name: &str,
    version: &str,
) -> bool {
    version == "unspecified"
        && matches!(
            (build_root, project_path, group, name),
            (
                ".",
                ":app:design_system",
                "harvestcircle.app",
                "design_system"
            ) | (".", ":app:shared", "harvestcircle.app", "shared")
                | (
                    "build-logic",
                    ":contracts",
                    "harvestcircle-build-logic",
                    "contracts"
                )
        )
}

fn valid_gradle_component(component: &GradleComponent) -> bool {
    match component.kind.as_str() {
        "module" => {
            component
                .group
                .as_deref()
                .is_some_and(|value| valid_bounded_text(value, 256))
                && component
                    .name
                    .as_deref()
                    .is_some_and(|value| valid_bounded_text(value, 256))
                && component
                    .version
                    .as_deref()
                    .is_some_and(|value| valid_bounded_text(value, 128))
                && component.build_root.is_none()
                && component.project_path.is_none()
                && !component.root
        }
        "project" => {
            component.group.is_none()
                && component.name.is_none()
                && component.version.is_none()
                && component
                    .build_root
                    .as_deref()
                    .is_some_and(|value| value == "." || valid_identifier(value))
                && component
                    .project_path
                    .as_deref()
                    .is_some_and(|value| value.starts_with(':') && valid_bounded_text(value, 256))
        }
        _ => false,
    }
}

fn valid_artifact_extension(extension: &str) -> bool {
    matches!(
        extension,
        "aar" | "jar" | "js" | "klib" | "module" | "pom" | "wasm" | "zip"
    )
}

fn validate_provider(
    provider: &ProviderSnapshot,
    request: &AdmissionRequest,
    trace: &NvdNetworkTrace,
    receipts: &[TrustedProcessReceipt],
) -> Result<(), AdvisoryError> {
    require_complete(provider.acquisition_state)?;
    require_complete(provider.analysis_state)?;
    if provider.acquisition_count != 1
        || provider.bounded_deadline_seconds != ANALYSIS_DEADLINE_SECONDS
        || !valid_hex(&provider.network_trace_sha256, 64)
        || !valid_hex(&provider.producer_request_sha256, 64)
        || provider.producer_request_sha256 != sha256(&request.producer_request)
        || !valid_hex(&provider.database_identity_sha256, 64)
        || provider.archive_format
            != "deterministic_tar_gzip_exact_bytes_normalized_headers_sorted_members"
        || provider.archive_expanded_bytes == 0
        || provider.archive_member_count == 0
        || provider.archive_payload_bytes == 0
        || provider.acquired_at_epoch == 0
        || provider.digest_time_epoch == 0
        || provider.analyzed_at_epoch == 0
        || provider.acquired_at_epoch > provider.digest_time_epoch
        || provider.digest_time_epoch > provider.analyzed_at_epoch
        || provider.analyzed_at_epoch > request.evaluation_epoch
    {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot));
    }
    let maximum_age = match request.freshness {
        FreshnessMode::Development => DEVELOPMENT_REUSE_SECONDS,
        FreshnessMode::Qualification => QUALIFICATION_FRESHNESS_SECONDS,
    };
    let age = request
        .evaluation_epoch
        .checked_sub(provider.digest_time_epoch)
        .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::StaleSnapshot))?;
    if age > maximum_age {
        return Err(AdvisoryError::new(AdvisoryFailureKind::StaleSnapshot));
    }
    let (kind, network, archive, report, acquisition_arguments, environment, expected_arguments) =
        match provider.provider {
            ProviderId::Rustsec => (
                "externally_admitted_immutable_snapshot",
                "external_snapshot_admission",
                RUSTSEC_ARCHIVE_NAME,
                RUSTSEC_REPORT_NAME,
                &[] as &[&str],
                "fresh_private_config_free_workdir_home_and_cargo_home",
                CARGO_AUDIT_ARGUMENTS.as_slice(),
            ),
            ProviderId::OwaspNvd => (
                "bounded_nvd_update_only",
                "contracted_fetch_only",
                NVD_ARCHIVE_NAME,
                OWASP_REPORT_NAME,
                OWASP_UPDATE_ARGUMENTS.as_slice(),
                "replacement_allowlist_private_data_and_output",
                OWASP_OFFLINE_ARGUMENTS.as_slice(),
            ),
        };
    if provider.acquisition_kind != kind
        || provider.network_mode != network
        || provider.archive.path != archive
        || provider.report.path != report
        || provider.acquisition_arguments
            != acquisition_arguments
                .iter()
                .map(|argument| (*argument).to_owned())
                .collect::<Vec<_>>()
        || provider.analysis_environment != environment
        || provider.analysis_arguments
            != expected_arguments
                .iter()
                .map(|argument| (*argument).to_owned())
                .collect::<Vec<_>>()
        || !valid_provider_blob_binding(provider.provider, &provider.archive, true)
        || !valid_provider_blob_binding(provider.provider, &provider.report, false)
    {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot));
    }
    match provider.provider {
        ProviderId::Rustsec => {
            if provider.network_trace_sha256 != "0".repeat(64) {
                return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot));
            }
        }
        ProviderId::OwaspNvd => {
            let receipt = process_receipt(receipts, "owasp_nvd_update")?;
            if provider.network_trace_sha256 != sha256(&request.nvd_network_trace)
                || provider.producer_request_sha256 != trace.producer_request_sha256
                || receipt.program_sha256 != TOOL_PINS[1].executable_sha256
                || receipt.arguments_sha256
                    != domain_json_digest(
                        b"radroots-advisory-process-arguments-v1\0",
                        &provider.acquisition_arguments,
                    )?
                || receipt.output_sha256
                    != nvd_update_output_digest(
                        &provider.materialized_tree_sha256,
                        &provider.network_trace_sha256,
                    )?
                || receipt.input_sha256 != sha256(&request.producer_request)
                || receipt.path_binding
                    != vec![ProcessPathBinding {
                        argument_index: 2,
                        logical_role: "nvd_update_data".to_owned(),
                        identity_sha256: provider.materialized_tree_sha256.clone(),
                    }]
                || receipt.completed_at_epoch != provider.acquired_at_epoch
                || receipt.exit_code != 0
            {
                return Err(AdvisoryError::new(AdvisoryFailureKind::BindingChanged));
            }
        }
    }
    Ok(())
}

fn require_complete(state: OperationState) -> Result<(), AdvisoryError> {
    match state {
        OperationState::Complete => Ok(()),
        OperationState::TimedOut => Err(AdvisoryError::new(AdvisoryFailureKind::TimedOut)),
        OperationState::Unavailable => Err(AdvisoryError::new(AdvisoryFailureKind::Unavailable)),
        OperationState::Failed => Err(AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot)),
    }
}

fn validate_temporal_order(
    observation: &TrustedToolObservation,
    providers: &[ProviderSnapshot],
    receipts: &[TrustedProcessReceipt],
) -> Result<(), AdvisoryError> {
    if observation.observed_at_epoch == 0
        || observation
            .tool_state
            .iter()
            .any(|state| state.reviewed_at_epoch > observation.observed_at_epoch)
        || providers
            .iter()
            .any(|provider| provider.acquired_at_epoch < observation.observed_at_epoch)
    {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot));
    }
    for provider in providers {
        let prefix = match provider.provider {
            ProviderId::Rustsec => "cargo_audit:",
            ProviderId::OwaspNvd => "owasp_analysis:",
        };
        let scans = receipts
            .iter()
            .filter(|receipt| receipt.id.starts_with(prefix))
            .collect::<Vec<_>>();
        if scans.is_empty()
            || scans
                .iter()
                .any(|receipt| receipt.started_at_epoch < provider.digest_time_epoch)
            || scans.iter().map(|receipt| receipt.completed_at_epoch).max()
                != Some(provider.analyzed_at_epoch)
        {
            return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot));
        }
        if provider.provider == ProviderId::OwaspNvd {
            let update = process_receipt(receipts, "owasp_nvd_update")?;
            if update.started_at_epoch < observation.observed_at_epoch
                || update.completed_at_epoch != provider.acquired_at_epoch
            {
                return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot));
            }
            for scan in scans {
                let workload = scan
                    .id
                    .strip_prefix("owasp_analysis:")
                    .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot))?;
                let gradle = process_receipt(receipts, &format!("gradle:{workload}"))?;
                if gradle.completed_at_epoch > scan.started_at_epoch {
                    return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot));
                }
            }
        }
    }
    Ok(())
}

fn parse_unsuppressed_report(
    snapshot: &ProviderSnapshot,
    bytes: &[u8],
    request: &AdmissionRequest,
    gradle_projections: &[GradleProjection],
) -> Result<Vec<Finding>, AdvisoryError> {
    let (_, _, _, provider_evidence) = validate_trusted_authority(request)?;
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
    if canonical_json(&value) != bytes {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport));
    }
    let report: ReportEnvelope = serde_json::from_value(value)
        .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
    if report.schema != "radroots.advisory-unsuppressed-report.v1"
        || report.provider != snapshot.provider
    {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport));
    }
    let expected_ids = WORKLOADS
        .iter()
        .filter(|workload| match snapshot.provider {
            ProviderId::Rustsec => workload.package_manager == "cargo",
            ProviderId::OwaspNvd => workload.package_manager == "gradle",
        })
        .map(|workload| workload.id)
        .collect::<Vec<_>>();
    if report.workload_result.len() != expected_ids.len()
        || !report
            .workload_result
            .iter()
            .map(|result| result.workload_id.as_str())
            .eq(expected_ids)
    {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch));
    }
    let mut findings = Vec::new();
    let mut database_copy_roots = BTreeSet::new();
    for result in report.workload_result {
        let inventory = request
            .inventory
            .iter()
            .find(|inventory| inventory.id == result.workload_id)
            .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch))?;
        validate_workload_execution(snapshot, &result, inventory, request)?;
        let admitted_output = request
            .admitted_scanner_outputs
            .iter()
            .find(|output| {
                output.provider == snapshot.provider && output.workload_id == result.workload_id
            })
            .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch))?;
        admitted_output.revalidate()?;
        if admitted_output.bytes.as_slice() != result.raw_scanner_output.as_bytes()
            || admitted_output.evidence.byte_length != result.raw_output_byte_length
            || admitted_output.evidence.sha256 != result.raw_output_sha256
        {
            return Err(AdvisoryError::new(AdvisoryFailureKind::BindingChanged));
        }
        if let Some(copy) = &result.database_copy
            && !database_copy_roots.insert(copy.root_identity_sha256.clone())
        {
            return Err(AdvisoryError::new(AdvisoryFailureKind::BindingChanged));
        }
        let raw: Value = serde_json::from_slice(&admitted_output.bytes)
            .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
        match snapshot.provider {
            ProviderId::Rustsec => parse_rustsec_output(
                &result.workload_id,
                &raw,
                inventory.dependency_count,
                &snapshot.database_identity_sha256,
                &snapshot.materialized_tree_sha256,
                snapshot.digest_time_epoch,
                &mut findings,
            )?,
            ProviderId::OwaspNvd => {
                let projection = gradle_projections
                    .iter()
                    .find(|projection| projection.workload_id == result.workload_id)
                    .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch))?;
                parse_owasp_output(
                    &result.workload_id,
                    &raw,
                    projection,
                    &snapshot.database_identity_sha256,
                    &snapshot.materialized_tree_sha256,
                    result
                        .arguments
                        .get(4)
                        .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?,
                    process_receipt(
                        &provider_evidence.process_receipt,
                        &format!("owasp_analysis:{}", result.workload_id),
                    )?
                    .started_at_epoch,
                    process_receipt(
                        &provider_evidence.process_receipt,
                        &format!("owasp_analysis:{}", result.workload_id),
                    )?
                    .completed_at_epoch,
                    snapshot.digest_time_epoch,
                    &mut findings,
                )?
            }
        }
        admitted_output.revalidate()?;
    }
    findings.sort_by(|left, right| finding_key(left).cmp(&finding_key(right)));
    if findings.windows(2).any(|window| window[0] == window[1]) {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport));
    }
    Ok(findings)
}

fn validate_workload_execution(
    snapshot: &ProviderSnapshot,
    result: &WorkloadResult,
    inventory: &WorkloadInventory,
    request: &AdmissionRequest,
) -> Result<(), AdvisoryError> {
    let raw = result.raw_scanner_output.as_bytes();
    let (tool_manifest, observation, _, provider_evidence) = validate_trusted_authority(request)?;
    let expected_tool_observation =
        tool_observation_digest(&tool_manifest.tool_acquisition, &observation.tool_state)?;
    let receipt_id = match snapshot.provider {
        ProviderId::Rustsec => format!("cargo_audit:{}", result.workload_id),
        ProviderId::OwaspNvd => format!("owasp_analysis:{}", result.workload_id),
    };
    let receipt = process_receipt(&provider_evidence.process_receipt, &receipt_id)?;
    let program_sha256 = match snapshot.provider {
        ProviderId::Rustsec => TOOL_PINS[0].executable_sha256,
        ProviderId::OwaspNvd => TOOL_PINS[1].executable_sha256,
    };
    let database_copy_valid = match (snapshot.provider, result.database_copy.as_ref()) {
        (ProviderId::Rustsec, None) => true,
        (ProviderId::OwaspNvd, Some(copy)) => {
            valid_hex(&copy.root_identity_sha256, 64)
                && copy.source_tree_sha256 == snapshot.materialized_tree_sha256
                && copy.pre_scan_tree_sha256 == snapshot.materialized_tree_sha256
                && copy.post_scan_tree_sha256 == snapshot.materialized_tree_sha256
        }
        _ => false,
    };
    if raw.is_empty()
        || raw.len() as u64 > MAX_RAW_WORKLOAD_REPORT_BYTES
        || result.input_sha256 != inventory.input_sha256
        || result.dependency_count != inventory.dependency_count
        || result.provider_archive_sha256 != snapshot.archive.sha256
        || result.materialized_tree_sha256 != snapshot.materialized_tree_sha256
        || result.tool_observation_sha256 != expected_tool_observation
        || result.raw_output_byte_length != raw.len() as u64
        || result.raw_output_sha256 != sha256(raw)
        || !valid_hex(&result.environment_sha256, 64)
        || result.process_receipt_sha256 != process_receipt_digest(receipt)?
        || !database_copy_valid
        || receipt.program_sha256 != program_sha256
        || receipt.arguments_sha256
            != domain_json_digest(
                b"radroots-advisory-process-arguments-v1\0",
                &result.arguments,
            )?
        || receipt.environment_sha256 != result.environment_sha256
        || receipt.input_sha256
            != scanner_process_input_digest(snapshot.provider, result, &expected_tool_observation)?
        || receipt.output_sha256 != result.raw_output_sha256
        || match snapshot.provider {
            ProviderId::Rustsec => {
                receipt.stdout_byte_length != result.raw_output_byte_length
                    || receipt.stdout_sha256 != result.raw_output_sha256
            }
            ProviderId::OwaspNvd => {
                receipt.stdout_byte_length != 0 || receipt.stdout_sha256 != sha256(&[])
            }
        }
        || receipt.exit_code != result.exit_code
        || receipt.completed_at_epoch > snapshot.analyzed_at_epoch
        || receipt.path_binding
            != scanner_path_bindings(snapshot.provider, result, inventory, request)?
        || !valid_scanner_arguments(snapshot.provider, &result.arguments)
        || match snapshot.provider {
            ProviderId::Rustsec => !matches!(result.exit_code, 0 | 1),
            ProviderId::OwaspNvd => result.exit_code != 0,
        }
    {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport));
    }
    Ok(())
}

fn scanner_path_bindings(
    provider: ProviderId,
    result: &WorkloadResult,
    inventory: &WorkloadInventory,
    request: &AdmissionRequest,
) -> Result<Vec<ProcessPathBinding>, AdvisoryError> {
    match provider {
        ProviderId::Rustsec => Ok(vec![
            ProcessPathBinding {
                argument_index: 2,
                logical_role: "rustsec_database".to_owned(),
                identity_sha256: result.materialized_tree_sha256.clone(),
            },
            ProcessPathBinding {
                argument_index: 6,
                logical_role: "cargo_lock".to_owned(),
                identity_sha256: inventory.input_sha256.clone(),
            },
        ]),
        ProviderId::OwaspNvd => {
            let projection = validate_projection_for_workload(&result.workload_id, request)?;
            let database_copy = result
                .database_copy
                .as_ref()
                .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
            Ok(vec![
                ProcessPathBinding {
                    argument_index: 2,
                    logical_role: "nvd_database".to_owned(),
                    identity_sha256: scanner_database_copy_digest(database_copy)?,
                },
                ProcessPathBinding {
                    argument_index: 4,
                    logical_role: "gradle_scan_projection".to_owned(),
                    identity_sha256: projection.materialized_tree_sha256.clone(),
                },
                ProcessPathBinding {
                    argument_index: 10,
                    logical_role: "raw_report_output".to_owned(),
                    identity_sha256: result.raw_output_sha256.clone(),
                },
            ])
        }
    }
}

fn scanner_database_copy_digest(copy: &ScannerDatabaseCopy) -> Result<String, AdvisoryError> {
    domain_json_digest(b"radroots-advisory-scanner-database-copy-v1\0", copy)
}

fn validate_projection_for_workload(
    workload_id: &str,
    request: &AdmissionRequest,
) -> Result<GradleProjection, AdvisoryError> {
    let (_, _, _, evidence) = validate_trusted_authority(request)?;
    evidence
        .gradle_projection
        .iter()
        .find(|projection| projection.workload_id == workload_id)
        .cloned()
        .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch))
}

fn nvd_update_output_digest(
    materialized_tree_sha256: &str,
    network_trace_sha256: &str,
) -> Result<String, AdvisoryError> {
    domain_json_digest(
        b"radroots-advisory-nvd-update-output-v1\0",
        &serde_json::json!({
            "materialized_tree_sha256": materialized_tree_sha256,
            "network_trace_sha256": network_trace_sha256,
        }),
    )
}

fn scanner_process_input_digest(
    provider: ProviderId,
    result: &WorkloadResult,
    tool_observation_sha256: &str,
) -> Result<String, AdvisoryError> {
    scanner_process_input_digest_fields(
        provider,
        &result.workload_id,
        &result.input_sha256,
        result.dependency_count,
        &result.provider_archive_sha256,
        &result.materialized_tree_sha256,
        &result
            .database_copy
            .as_ref()
            .map(scanner_database_copy_digest)
            .transpose()?
            .unwrap_or_else(|| "none".to_owned()),
        tool_observation_sha256,
    )
}

#[allow(clippy::too_many_arguments)]
fn scanner_process_input_digest_fields(
    provider: ProviderId,
    workload_id: &str,
    input_sha256: &str,
    dependency_count: u64,
    provider_archive_sha256: &str,
    materialized_tree_sha256: &str,
    database_copy_sha256: &str,
    tool_observation_sha256: &str,
) -> Result<String, AdvisoryError> {
    domain_json_digest(
        b"radroots-advisory-scanner-input-v1\0",
        &serde_json::json!({
            "dependency_count": dependency_count,
            "database_copy_sha256": database_copy_sha256,
            "input_sha256": input_sha256,
            "materialized_tree_sha256": materialized_tree_sha256,
            "provider": provider,
            "provider_archive_sha256": provider_archive_sha256,
            "tool_observation_sha256": tool_observation_sha256,
            "workload_id": workload_id,
        }),
    )
}

fn valid_scanner_arguments(provider: ProviderId, arguments: &[String]) -> bool {
    match provider {
        ProviderId::Rustsec => {
            arguments.len() == CARGO_AUDIT_ARGUMENTS.len()
                && arguments[0] == "audit"
                && arguments[1] == "--db"
                && absolute_path(&arguments[2])
                && arguments[3..6] == ["--no-fetch", "--json", "--file"]
                && absolute_path(&arguments[6])
                && !arguments
                    .iter()
                    .any(|argument| matches!(argument.as_str(), "--ignore" | "--stale"))
        }
        ProviderId::OwaspNvd => {
            if arguments.len() != OWASP_OFFLINE_ARGUMENTS.len()
                || arguments[0] != "--noupdate"
                || arguments[1] != "--data"
                || !absolute_path(&arguments[2])
                || arguments[3] != "--scan"
                || !absolute_path(&arguments[4])
                || arguments[5] != "--project"
                || !WORKLOADS.iter().any(|workload| {
                    workload.package_manager == "gradle" && workload.id == arguments[6]
                })
                || arguments[7] != "--format"
                || arguments[8] != "JSON"
                || arguments[9] != "--out"
                || !absolute_path(&arguments[10])
            {
                return false;
            }
            let required = &OWASP_OFFLINE_ARGUMENTS[11..];
            arguments[11..]
                == required
                    .iter()
                    .map(|argument| (*argument).to_owned())
                    .collect::<Vec<_>>()
                && !arguments.iter().any(|argument| {
                    argument == "--suppression" || argument.starts_with("--failOnCVSS")
                })
        }
    }
}

fn absolute_path(value: &str) -> bool {
    !value.is_empty() && Path::new(value).is_absolute() && !value.contains('\0')
}

fn parse_rustsec_output(
    workload_id: &str,
    output: &Value,
    expected_dependency_count: u64,
    expected_database_identity: &str,
    materialized_tree_sha256: &str,
    database_digest_time_epoch: u64,
    findings: &mut Vec<Finding>,
) -> Result<(), AdvisoryError> {
    validate_json_bounds(output)?;
    let object = output
        .as_object()
        .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
    let expected_keys = [
        "database",
        "lockfile",
        "settings",
        "vulnerabilities",
        "warnings",
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    if object.keys().map(String::as_str).collect::<BTreeSet<_>>() != expected_keys {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport));
    }
    let database = object["database"]
        .as_object()
        .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
    if database.keys().map(String::as_str).collect::<BTreeSet<_>>()
        != ["advisory-count", "last-commit", "last-updated"]
            .into_iter()
            .collect()
        || database["advisory-count"]
            .as_u64()
            .is_none_or(|count| count == 0)
        || !database["last-commit"]
            .as_str()
            .is_some_and(|revision| valid_hex(revision, 40))
        || database["last-updated"]
            .as_str()
            .and_then(|timestamp| parse_report_epoch(timestamp).ok())
            != Some(database_digest_time_epoch)
    {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport));
    }
    let database_metadata_sha256 = domain_json_digest(
        b"radroots-advisory-rustsec-database-metadata-v1\0",
        database,
    )?;
    if provider_database_identity(
        ProviderId::Rustsec,
        materialized_tree_sha256,
        &database_metadata_sha256,
    )? != expected_database_identity
    {
        return Err(AdvisoryError::new(AdvisoryFailureKind::BindingChanged));
    }
    let lockfile = object["lockfile"]
        .as_object()
        .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
    if lockfile.keys().map(String::as_str).collect::<Vec<_>>() != ["dependency-count"]
        || lockfile["dependency-count"].as_u64() != Some(expected_dependency_count)
    {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch));
    }
    let settings = object["settings"]
        .as_object()
        .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
    if settings.keys().map(String::as_str).collect::<BTreeSet<_>>()
        != [
            "ignore",
            "informational_warnings",
            "severity",
            "target_arch",
            "target_os",
        ]
        .into_iter()
        .collect()
        || settings["ignore"]
            .as_array()
            .is_none_or(|value| !value.is_empty())
        || settings["target_arch"]
            .as_array()
            .is_none_or(|value| !value.is_empty())
        || settings["target_os"]
            .as_array()
            .is_none_or(|value| !value.is_empty())
        || !settings["severity"].is_null()
        || settings["informational_warnings"]
            != serde_json::json!(["unmaintained", "unsound", "notice"])
    {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport));
    }
    if !object["warnings"].is_object() {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport));
    }
    let vulnerabilities_object = object["vulnerabilities"]
        .as_object()
        .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
    if vulnerabilities_object
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>()
        != ["count", "found", "list"].into_iter().collect()
    {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport));
    }
    let vulnerabilities = vulnerabilities_object["list"]
        .as_array()
        .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
    for vulnerability in vulnerabilities {
        let vulnerability_object = vulnerability
            .as_object()
            .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
        if vulnerability_object
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>()
            != ["advisory", "affected", "package", "versions"]
                .into_iter()
                .collect()
        {
            return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport));
        }
        let (package, version) = validate_rustsec_package(&vulnerability_object["package"])?;
        let advisory_id = validate_rustsec_advisory(&vulnerability_object["advisory"], package)?;
        validate_rustsec_affected(&vulnerability_object["affected"])?;
        validate_rustsec_versions(&vulnerability_object["versions"])?;
        findings.push(Finding {
            provider: ProviderId::Rustsec,
            advisory_id: advisory_id.to_owned(),
            package_ecosystem: "cargo".to_owned(),
            package_namespace: String::new(),
            package_name: package.to_owned(),
            package_version: version.to_owned(),
            workload_id: workload_id.to_owned(),
        });
    }
    let found = vulnerabilities_object["found"]
        .as_bool()
        .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
    if found != !vulnerabilities.is_empty()
        || vulnerabilities_object["count"].as_u64() != Some(vulnerabilities.len() as u64)
    {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport));
    }
    let warnings = object["warnings"]
        .as_object()
        .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
    for (kind, rows) in warnings {
        if !matches!(
            kind.as_str(),
            "notice" | "unmaintained" | "unsound" | "yanked"
        ) {
            return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport));
        }
        let rows = rows
            .as_array()
            .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
        if kind == "yanked" && !rows.is_empty() {
            return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport));
        }
        for warning in rows {
            let warning_object = warning
                .as_object()
                .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
            if warning_object
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>()
                != ["advisory", "affected", "kind", "package", "versions"]
                    .into_iter()
                    .collect()
                || warning_object["kind"].as_str() != Some(kind)
            {
                return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport));
            }
            let (package_name, package_version) =
                validate_rustsec_package(&warning_object["package"])?;
            let advisory_id = validate_rustsec_advisory(&warning_object["advisory"], package_name)?;
            validate_rustsec_affected(&warning_object["affected"])?;
            validate_rustsec_versions(&warning_object["versions"])?;
            findings.push(Finding {
                provider: ProviderId::Rustsec,
                advisory_id: advisory_id.to_owned(),
                package_ecosystem: "cargo".to_owned(),
                package_namespace: String::new(),
                package_name: package_name.to_owned(),
                package_version: package_version.to_owned(),
                workload_id: workload_id.to_owned(),
            });
        }
    }
    Ok(())
}

fn validate_rustsec_advisory<'a>(
    value: &'a Value,
    package_name: &str,
) -> Result<&'a str, AdvisoryError> {
    let object = value
        .as_object()
        .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
    if !object_keys_exact(
        object,
        &[
            "aliases",
            "categories",
            "collection",
            "cvss",
            "date",
            "description",
            "expect-deleted",
            "id",
            "informational",
            "keywords",
            "license",
            "package",
            "references",
            "related",
            "source",
            "title",
            "url",
            "withdrawn",
        ],
        &[],
    ) || object["package"].as_str() != Some(package_name)
        || !valid_rustsec_date(&object["date"])
        || !valid_text_allow_empty_value(&object["title"], 4_096)
        || !valid_text_allow_empty_value(&object["description"], 1_048_576)
        || !valid_exact_string_array(&object["aliases"], 4_096, 256)
        || !valid_exact_string_array(&object["related"], 4_096, 256)
        || !valid_exact_string_array(&object["categories"], 256, 128)
        || !valid_exact_string_array(&object["keywords"], 4_096, 256)
        || !valid_exact_string_array(&object["references"], 65_536, 4_096)
        || !optional_rustsec_text(&object["collection"], 64)
        || !optional_rustsec_text(&object["cvss"], 256)
        || !optional_rustsec_text(&object["informational"], 64)
        || !optional_rustsec_text(&object["source"], 4_096)
        || !optional_rustsec_text(&object["url"], 4_096)
        || !(object["withdrawn"].is_null() || valid_rustsec_date(&object["withdrawn"]))
        || !object["license"]
            .as_str()
            .is_some_and(|value| valid_bounded_text(value, 128))
        || object["expect-deleted"].as_bool().is_none()
    {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport));
    }
    let advisory_id = required_text(object.get("id"))?;
    if !valid_advisory_id(ProviderId::Rustsec, advisory_id) {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport));
    }
    Ok(advisory_id)
}

fn validate_rustsec_package(value: &Value) -> Result<(&str, &str), AdvisoryError> {
    let object = value
        .as_object()
        .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
    if !object_keys_exact(
        object,
        &["checksum", "name", "replace", "source", "version"],
        &["dependencies"],
    ) || !optional_rustsec_text(&object["source"], 4_096)
        || !(object["checksum"].is_null()
            || object["checksum"]
                .as_str()
                .is_some_and(|value| valid_hex(value, 64)))
        || !(object["replace"].is_null() || valid_rustsec_dependency(&object["replace"]))
        || object.get("dependencies").is_some_and(|value| {
            value.as_array().is_none_or(|rows| {
                rows.is_empty()
                    || rows.len() > 65_536
                    || rows.iter().any(|row| !valid_rustsec_dependency(row))
            })
        })
    {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport));
    }
    let name = required_text(object.get("name"))?;
    let version = required_text(object.get("version"))?;
    if !valid_exact_identity_token(name, 256) || !valid_exact_identity_token(version, 128) {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport));
    }
    Ok((name, version))
}

fn valid_rustsec_dependency(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    object_keys_exact(object, &["name", "source", "version"], &[])
        && object["name"]
            .as_str()
            .is_some_and(|value| valid_exact_identity_token(value, 256))
        && object["version"]
            .as_str()
            .is_some_and(|value| valid_exact_identity_token(value, 128))
        && optional_rustsec_text(&object["source"], 4_096)
}

fn validate_rustsec_versions(value: &Value) -> Result<(), AdvisoryError> {
    let object = value
        .as_object()
        .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
    if !object_keys_exact(object, &["patched", "unaffected"], &[])
        || !valid_exact_string_array(&object["patched"], 65_536, 256)
        || !valid_exact_string_array(&object["unaffected"], 65_536, 256)
    {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport));
    }
    Ok(())
}

fn validate_rustsec_affected(value: &Value) -> Result<(), AdvisoryError> {
    if value.is_null() {
        return Ok(());
    }
    let object = value
        .as_object()
        .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
    let functions = object["functions"]
        .as_object()
        .filter(|rows| rows.len() <= 65_536)
        .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
    if !object_keys_exact(object, &["arch", "functions", "os"], &[])
        || !valid_exact_string_array(&object["arch"], 1_024, 128)
        || !valid_exact_string_array(&object["os"], 1_024, 128)
        || functions.iter().any(|(path, versions)| {
            !valid_rust_function_path(path) || !valid_exact_string_array(versions, 65_536, 256)
        })
    {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport));
    }
    Ok(())
}

fn valid_rust_function_path(value: &str) -> bool {
    let mut components = value.split("::");
    let valid_component = |component: &str| {
        let mut chars = component.chars();
        chars
            .next()
            .is_some_and(|first| first.is_ascii_alphabetic() || matches!(first, '_' | '<'))
            && chars.all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '_' | '<' | '>' | ',')
            })
    };
    let first = components.next().is_some_and(valid_component);
    let rest = components.collect::<Vec<_>>();
    first && !rest.is_empty() && rest.into_iter().all(valid_component)
}

fn valid_rustsec_date(value: &Value) -> bool {
    value.as_str().is_some_and(|value| {
        let bytes = value.as_bytes();
        bytes.len() == 10
            && bytes[4] == b'-'
            && bytes[7] == b'-'
            && bytes
                .iter()
                .enumerate()
                .all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit())
            && parse_report_epoch(&format!("{value}T00:00:00Z")).is_ok()
    })
}

fn valid_text_allow_empty_value(value: &Value, maximum: usize) -> bool {
    value
        .as_str()
        .is_some_and(|value| valid_text_allow_empty(value, maximum))
}

fn optional_rustsec_text(value: &Value, maximum: usize) -> bool {
    value.is_null()
        || value
            .as_str()
            .is_some_and(|value| valid_bounded_text(value, maximum))
}

fn valid_exact_string_array(value: &Value, maximum_rows: usize, maximum_text: usize) -> bool {
    value
        .as_array()
        .filter(|rows| rows.len() <= maximum_rows)
        .is_some_and(|rows| {
            rows.iter().all(|row| {
                row.as_str()
                    .is_some_and(|value| valid_bounded_text(value, maximum_text))
            })
        })
}

#[allow(clippy::too_many_arguments)]
fn parse_owasp_output(
    workload_id: &str,
    output: &Value,
    projection: &GradleProjection,
    expected_database_identity: &str,
    materialized_tree_sha256: &str,
    scan_root: &str,
    scan_started_at_epoch: u64,
    scan_completed_at_epoch: u64,
    database_digest_time_epoch: u64,
    findings: &mut Vec<Finding>,
) -> Result<(), AdvisoryError> {
    validate_json_bounds(output)?;
    let object = output
        .as_object()
        .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
    if !object_keys_exact(
        object,
        &["dependencies", "projectInfo", "reportSchema", "scanInfo"],
        &[],
    ) || object["reportSchema"].as_str() != Some("1.1")
    {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport));
    }
    let scan_info = object["scanInfo"]
        .as_object()
        .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
    if !object_keys_exact(scan_info, &["dataSource", "engineVersion"], &[])
        || scan_info["engineVersion"].as_str() != Some("12.2.2")
    {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport));
    }
    let data_source = scan_info["dataSource"]
        .as_array()
        .filter(|sources| !sources.is_empty())
        .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
    if data_source.len() > 64 {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport));
    }
    let mut previous_source = None::<Vec<u8>>;
    let mut observed_database_epoch = None::<u64>;
    for source in data_source {
        let source = source
            .as_object()
            .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
        if !object_keys_exact(source, &["name", "timestamp"], &[])
            || !required_text(source.get("name"))?.starts_with("NVD CVE ")
        {
            return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport));
        }
        let epoch = parse_report_epoch(required_text(source.get("timestamp"))?)?;
        if epoch == 0 || epoch > database_digest_time_epoch {
            return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport));
        }
        observed_database_epoch =
            Some(observed_database_epoch.map_or(epoch, |value| value.max(epoch)));
        let key = canonical_json_without_lf(&Value::Object(source.clone()));
        if previous_source
            .as_ref()
            .is_some_and(|previous| previous >= &key)
        {
            return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport));
        }
        previous_source = Some(key);
    }
    if observed_database_epoch != Some(database_digest_time_epoch) {
        return Err(AdvisoryError::new(AdvisoryFailureKind::BindingChanged));
    }
    let database_metadata_sha256 = domain_json_digest(
        b"radroots-advisory-owasp-database-metadata-v1\0",
        data_source,
    )?;
    if provider_database_identity(
        ProviderId::OwaspNvd,
        materialized_tree_sha256,
        &database_metadata_sha256,
    )? != expected_database_identity
    {
        return Err(AdvisoryError::new(AdvisoryFailureKind::BindingChanged));
    }
    let project = object["projectInfo"]
        .as_object()
        .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
    if !object_keys_exact(
        project,
        &["credits", "name", "reportDate"],
        &["artifactID", "groupID", "version"],
    ) || project["name"].as_str() != Some(workload_id)
        || project
            .get("groupID")
            .is_some_and(|value| !value.is_string())
        || project
            .get("artifactID")
            .is_some_and(|value| !value.is_string())
        || project
            .get("version")
            .is_some_and(|value| !value.is_string())
        || !valid_owasp_credits(&project["credits"])
    {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport));
    }
    let report_epoch = parse_report_epoch(required_text(project.get("reportDate"))?)?;
    if report_epoch < scan_started_at_epoch || report_epoch > scan_completed_at_epoch {
        return Err(AdvisoryError::new(AdvisoryFailureKind::BindingChanged));
    }
    if !absolute_path(scan_root) || scan_root.ends_with('/') {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport));
    }
    let dependencies = object["dependencies"]
        .as_array()
        .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
    let mut expected = BTreeMap::<&str, Vec<&GradleArtifact>>::new();
    for artifact in &projection.artifacts {
        expected
            .entry(artifact.materialized_name.as_str())
            .or_default()
            .push(artifact);
    }
    if dependencies.len() < expected.len()
        || dependencies.len() > expected.len().saturating_add(65_536)
    {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch));
    }
    let mut observed = BTreeSet::new();
    let mut parsed_dependencies = Vec::with_capacity(dependencies.len());
    for dependency in dependencies {
        let dependency = dependency
            .as_object()
            .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
        if !object_keys_exact(
            dependency,
            &["evidenceCollected", "fileName", "filePath", "isVirtual"],
            &[
                "description",
                "includedBy",
                "license",
                "md5",
                "packages",
                "projectReferences",
                "relatedDependencies",
                "sha1",
                "sha256",
                "vulnerabilities",
                "vulnerabilityIds",
            ],
        ) || dependency.contains_key("suppressedVulnerabilities")
            || dependency.contains_key("suppressedVulnerabilityIds")
            || !valid_owasp_evidence(&dependency["evidenceCollected"])
            || !optional_bounded_string(dependency.get("description"), 65_536)
            || !optional_bounded_string(dependency.get("license"), 65_536)
            || !valid_sorted_string_array(dependency.get("projectReferences"), 16_384, 1024)?
        {
            return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport));
        }
        let file_name = required_text(dependency.get("fileName"))?;
        let file_path = required_text(dependency.get("filePath"))?;
        let is_virtual = dependency
            .get("isVirtual")
            .and_then(Value::as_bool)
            .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
        let digest = dependency.get("sha256").and_then(Value::as_str);
        if is_virtual {
            if dependency.contains_key("md5")
                || dependency.contains_key("sha1")
                || dependency.contains_key("sha256")
                || !file_path.starts_with(&format!("{scan_root}/"))
            {
                return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport));
            }
        } else if !dependency
            .get("md5")
            .and_then(Value::as_str)
            .is_some_and(|value| valid_hex(value, 32))
            || !dependency
                .get("sha1")
                .and_then(Value::as_str)
                .is_some_and(|value| valid_hex(value, 40))
            || !digest.is_some_and(|value| valid_hex(value, 64))
            || file_path != format!("{scan_root}/{file_name}")
        {
            return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport));
        }
        let package_ids = parse_owasp_identifiers(dependency.get("packages"), true)?;
        let vulnerability_ids = parse_owasp_identifiers(dependency.get("vulnerabilityIds"), true)?;
        let vulnerabilities = parse_owasp_vulnerabilities(dependency.get("vulnerabilities"))?;
        let vulnerable_software_ids = vulnerabilities
            .iter()
            .flat_map(|vulnerability| vulnerability.vulnerable_software_ids.iter())
            .collect::<BTreeSet<_>>();
        if vulnerability_ids
            .iter()
            .any(|id| !valid_owasp_software_identifier(id))
            || (!vulnerability_ids.is_empty() && vulnerabilities.is_empty())
            || vulnerability_ids
                .iter()
                .any(|id| !vulnerable_software_ids.contains(id))
        {
            return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport));
        }
        let included_by = parse_owasp_included_by(dependency.get("includedBy"))?;
        let related = parse_owasp_related(dependency.get("relatedDependencies"), &expected)?;
        let mut aliases = package_ids.clone();
        aliases.extend(related.aliases);
        aliases.sort();
        if aliases.windows(2).any(|rows| rows[0] == rows[1]) {
            return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport));
        }
        if is_virtual {
            if package_ids.is_empty() || included_by.is_empty() {
                return Err(AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch));
            }
            if expected.contains_key(file_name) {
                return Err(AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch));
            }
        } else {
            let artifacts = expected
                .get(file_name)
                .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch))?;
            let digest =
                digest.ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
            if artifacts
                .iter()
                .any(|artifact| artifact.artifact_sha256 != digest)
                || !observed.insert(file_name.to_owned())
                || package_ids.iter().any(|id| {
                    !artifacts
                        .iter()
                        .any(|artifact| package_identifier_compatible(id, artifact))
                })
            {
                return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport));
            }
        }
        parsed_dependencies.push(ParsedOwaspDependency {
            aliases,
            included_by,
            is_virtual,
            file_name: file_name.to_owned(),
            package_ids,
            vulnerabilities,
        });
    }
    if observed.len() != expected.len() {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch));
    }
    validate_owasp_lineage(workload_id, &parsed_dependencies)?;
    for dependency in &parsed_dependencies {
        let advisory_ids = dependency
            .vulnerabilities
            .iter()
            .map(|vulnerability| vulnerability.advisory_id.as_str())
            .collect::<BTreeSet<_>>();
        if dependency.is_virtual {
            for package_id in &dependency.package_ids {
                let (ecosystem, namespace, name, version) = parse_exact_package_url(package_id)?;
                for advisory_id in &advisory_ids {
                    findings.push(Finding {
                        provider: ProviderId::OwaspNvd,
                        advisory_id: (*advisory_id).to_owned(),
                        package_ecosystem: ecosystem.clone(),
                        package_namespace: namespace.clone(),
                        package_name: name.clone(),
                        package_version: version.clone(),
                        workload_id: workload_id.to_owned(),
                    });
                }
            }
        } else {
            let artifacts = expected
                .get(dependency.file_name.as_str())
                .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch))?;
            for artifact in artifacts {
                for advisory_id in &advisory_ids {
                    findings.push(Finding {
                        provider: ProviderId::OwaspNvd,
                        advisory_id: (*advisory_id).to_owned(),
                        package_ecosystem: artifact.package_ecosystem.clone(),
                        package_namespace: artifact.package_namespace.clone(),
                        package_name: artifact.package_name.clone(),
                        package_version: artifact.package_version.clone(),
                        workload_id: workload_id.to_owned(),
                    });
                }
            }
        }
    }
    Ok(())
}

#[derive(Clone)]
struct ParsedOwaspVulnerability {
    advisory_id: String,
    vulnerable_software_ids: BTreeSet<String>,
}

struct ParsedOwaspDependency {
    aliases: Vec<String>,
    included_by: Vec<String>,
    is_virtual: bool,
    file_name: String,
    package_ids: Vec<String>,
    vulnerabilities: Vec<ParsedOwaspVulnerability>,
}

struct ParsedOwaspRelated {
    aliases: Vec<String>,
}

fn object_keys_exact(
    object: &serde_json::Map<String, Value>,
    required: &[&str],
    optional: &[&str],
) -> bool {
    required.iter().all(|key| object.contains_key(*key))
        && object
            .keys()
            .all(|key| required.contains(&key.as_str()) || optional.contains(&key.as_str()))
        && object.len() >= required.len()
        && object.len() <= required.len() + optional.len()
}

fn validate_json_bounds(value: &Value) -> Result<(), AdvisoryError> {
    fn visit(value: &Value, depth: usize, nodes: &mut usize) -> bool {
        if depth > 32 || *nodes >= 1_000_000 {
            return false;
        }
        *nodes += 1;
        match value {
            Value::String(value) => valid_bounded_text(value, 1_048_576),
            Value::Array(values) => {
                values.len() <= 65_536 && values.iter().all(|value| visit(value, depth + 1, nodes))
            }
            Value::Object(values) => {
                values.len() <= 1_024
                    && values.keys().all(|key| valid_bounded_text(key, 256))
                    && values.values().all(|value| visit(value, depth + 1, nodes))
            }
            _ => true,
        }
    }
    if visit(value, 0, &mut 0) {
        Ok(())
    } else {
        Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport))
    }
}

fn parse_report_epoch(value: &str) -> Result<u64, AdvisoryError> {
    let bytes = value.as_bytes();
    if bytes.len() < 20
        || bytes.get(4) != Some(&b'-')
        || bytes.get(7) != Some(&b'-')
        || bytes.get(10) != Some(&b'T')
        || bytes.get(13) != Some(&b':')
        || bytes.get(16) != Some(&b':')
        || bytes.last() != Some(&b'Z')
        || (bytes.len() > 20
            && (bytes.get(19) != Some(&b'.')
                || !bytes[20..bytes.len() - 1].iter().all(u8::is_ascii_digit)))
    {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport));
    }
    fn number(bytes: &[u8]) -> Option<u64> {
        bytes.iter().try_fold(0_u64, |value, byte| {
            byte.is_ascii_digit()
                .then(|| value * 10 + u64::from(byte - b'0'))
        })
    }
    let year = i64::try_from(number(&bytes[0..4]).unwrap_or_default()).unwrap_or_default();
    let month = number(&bytes[5..7]).unwrap_or_default();
    let day = number(&bytes[8..10]).unwrap_or_default();
    let hour = number(&bytes[11..13]).unwrap_or(24);
    let minute = number(&bytes[14..16]).unwrap_or(60);
    let second = number(&bytes[17..19]).unwrap_or(60);
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days_in_month = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => 0,
    };
    if year < 1970 || day == 0 || day > days_in_month || hour > 23 || minute > 59 || second > 59 {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport));
    }
    let adjusted_year = year - i64::from(month <= 2);
    let era = adjusted_year.div_euclid(400);
    let year_of_era = adjusted_year - era * 400;
    let adjusted_month = i64::try_from(month).unwrap_or_default() + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * adjusted_month + 2) / 5 + i64::try_from(day).unwrap_or_default() - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    let days = era * 146_097 + day_of_era - 719_468;
    u64::try_from(days)
        .ok()
        .and_then(|days| days.checked_mul(86_400))
        .and_then(|value| value.checked_add(hour * 3_600 + minute * 60 + second))
        .filter(|value| *value > 0)
        .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))
}

fn format_report_epoch(value: u64) -> Result<String, AdvisoryError> {
    let days = i64::try_from(value / 86_400)
        .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot))?;
    let seconds = value % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    Ok(format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        seconds / 3_600,
        seconds % 3_600 / 60,
        seconds % 60,
    ))
}

fn optional_bounded_string(value: Option<&Value>, maximum: usize) -> bool {
    value.is_none_or(|value| {
        value
            .as_str()
            .is_some_and(|value| valid_bounded_text(value, maximum))
    })
}

fn valid_text_allow_empty(value: &str, maximum: usize) -> bool {
    value.len() <= maximum && !value.contains('\0') && !value.chars().any(char::is_control)
}

fn valid_sorted_string_array(
    value: Option<&Value>,
    maximum_rows: usize,
    maximum_bytes: usize,
) -> Result<bool, AdvisoryError> {
    let Some(value) = value else {
        return Ok(true);
    };
    let rows = value
        .as_array()
        .filter(|rows| rows.len() <= maximum_rows)
        .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
    let mut previous = None;
    for row in rows {
        let row = row
            .as_str()
            .filter(|row| valid_bounded_text(row, maximum_bytes))
            .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
        if previous.is_some_and(|previous| previous >= row) {
            return Ok(false);
        }
        previous = Some(row);
    }
    Ok(true)
}

fn valid_owasp_credits(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    const CREDITS: [(&str, &str); 5] = [
        (
            "CISA",
            "This report may contain data retrieved from the CISA Known Exploited Vulnerability Catalog: https://www.cisa.gov/known-exploited-vulnerabilities-catalog",
        ),
        (
            "NPM",
            "This report may contain data retrieved from the Github Advisory Database (via NPM Audit API): https://github.com/advisories/",
        ),
        (
            "NVD",
            "This product uses the NVD API but is not endorsed or certified by the NVD. This report contains data retrieved from the National Vulnerability Database: https://nvd.nist.gov",
        ),
        (
            "OSSINDEX",
            "This report may contain data retrieved from the Sonatype Guide OSS Index API: https://www.sonatype.com/products/sonatype-guide/oss-index-users",
        ),
        (
            "RETIREJS",
            "This report may contain data retrieved from the RetireJS community: https://retirejs.github.io/retire.js/",
        ),
    ];
    object.len() == CREDITS.len()
        && CREDITS
            .iter()
            .all(|(key, value)| object.get(*key).and_then(Value::as_str) == Some(*value))
}

fn valid_owasp_evidence(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    if !object_keys_exact(
        object,
        &["productEvidence", "vendorEvidence", "versionEvidence"],
        &[],
    ) {
        return false;
    }
    [
        ("productEvidence", "product"),
        ("vendorEvidence", "vendor"),
        ("versionEvidence", "version"),
    ]
    .into_iter()
    .all(|(key, kind)| {
        object[key]
            .as_array()
            .filter(|rows| rows.len() <= 65_536)
            .is_some_and(|rows| {
                rows.iter().all(|row| {
                    row.as_object().is_some_and(|row| {
                        object_keys_exact(
                            row,
                            &["confidence", "name", "source", "type", "value"],
                            &[],
                        ) && row["type"].as_str() == Some(kind)
                            && ["confidence", "name", "source", "value"].iter().all(|key| {
                                row[*key]
                                    .as_str()
                                    .is_some_and(|value| valid_bounded_text(value, 4096))
                            })
                    })
                })
            })
    })
}

fn parse_owasp_identifiers(
    value: Option<&Value>,
    allow_confidence: bool,
) -> Result<Vec<String>, AdvisoryError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let rows = value
        .as_array()
        .filter(|rows| !rows.is_empty() && rows.len() <= 65_536)
        .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
    let mut result = Vec::with_capacity(rows.len());
    for row in rows {
        let row = row
            .as_object()
            .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
        let optional = if allow_confidence {
            ["confidence", "notes", "url"].as_slice()
        } else {
            ["notes", "url"].as_slice()
        };
        if !object_keys_exact(row, &["id"], optional)
            || optional
                .iter()
                .any(|key| !optional_bounded_string(row.get(*key), 4096))
        {
            return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport));
        }
        result.push(required_text(row.get("id"))?.to_owned());
    }
    if result.windows(2).any(|rows| rows[0] >= rows[1]) {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport));
    }
    Ok(result)
}

fn parse_owasp_included_by(value: Option<&Value>) -> Result<Vec<String>, AdvisoryError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let rows = value
        .as_array()
        .filter(|rows| !rows.is_empty() && rows.len() <= 65_536)
        .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
    let mut result = Vec::with_capacity(rows.len());
    let mut previous = None::<Vec<u8>>;
    for row in rows {
        let row = row
            .as_object()
            .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
        if !object_keys_exact(row, &["reference"], &["type"])
            || !optional_bounded_string(row.get("type"), 256)
        {
            return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport));
        }
        let key = canonical_json_without_lf(&Value::Object(row.clone()));
        if previous.as_ref().is_some_and(|previous| previous >= &key) {
            return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport));
        }
        previous = Some(key);
        result.push(required_text(row.get("reference"))?.to_owned());
    }
    Ok(result)
}

fn parse_owasp_related(
    value: Option<&Value>,
    expected: &BTreeMap<&str, Vec<&GradleArtifact>>,
) -> Result<ParsedOwaspRelated, AdvisoryError> {
    let Some(value) = value else {
        return Ok(ParsedOwaspRelated {
            aliases: Vec::new(),
        });
    };
    let rows = value
        .as_array()
        .filter(|rows| !rows.is_empty() && rows.len() <= 65_536)
        .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
    let mut aliases = Vec::new();
    let mut previous = None::<Vec<u8>>;
    for row in rows {
        let row = row
            .as_object()
            .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
        if !object_keys_exact(
            row,
            &["fileName", "filePath", "isVirtual"],
            &["md5", "packageIds", "sha1", "sha256"],
        ) {
            return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport));
        }
        let is_virtual = row["isVirtual"]
            .as_bool()
            .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
        let file_name = required_text(row.get("fileName"))?;
        required_text(row.get("filePath"))?;
        if is_virtual {
            if row.contains_key("md5") || row.contains_key("sha1") || row.contains_key("sha256") {
                return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport));
            }
        } else if !row
            .get("md5")
            .and_then(Value::as_str)
            .is_some_and(|value| valid_hex(value, 32))
            || !row
                .get("sha1")
                .and_then(Value::as_str)
                .is_some_and(|value| valid_hex(value, 40))
            || !row
                .get("sha256")
                .and_then(Value::as_str)
                .is_some_and(|value| valid_hex(value, 64))
        {
            return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport));
        }
        if let (Some(expected), Some(digest)) = (
            expected.get(file_name),
            row.get("sha256").and_then(Value::as_str),
        ) && expected
            .iter()
            .any(|artifact| artifact.artifact_sha256 != digest)
        {
            return Err(AdvisoryError::new(AdvisoryFailureKind::BindingChanged));
        }
        aliases.extend(parse_owasp_identifiers(row.get("packageIds"), false)?);
        let key = canonical_json_without_lf(&Value::Object(row.clone()));
        if previous.as_ref().is_some_and(|previous| previous >= &key) {
            return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport));
        }
        previous = Some(key);
    }
    Ok(ParsedOwaspRelated { aliases })
}

fn parse_owasp_vulnerabilities(
    value: Option<&Value>,
) -> Result<Vec<ParsedOwaspVulnerability>, AdvisoryError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let rows = value
        .as_array()
        .filter(|rows| !rows.is_empty() && rows.len() <= 65_536)
        .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
    let mut result = Vec::with_capacity(rows.len());
    for row in rows {
        let row = row
            .as_object()
            .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
        if !object_keys_exact(
            row,
            &[
                "description",
                "name",
                "notes",
                "references",
                "source",
                "vulnerableSoftware",
            ],
            &["cvssv2", "cvssv3", "cvssv4", "cwes", "severity", "unscored"],
        ) || row["source"].as_str() != Some("NVD")
            || !optional_bounded_string(row.get("severity"), 128)
            || row
                .get("unscored")
                .is_some_and(|value| value.as_str() != Some("true"))
            || !["description", "notes"].iter().all(|key| {
                row[*key]
                    .as_str()
                    .is_some_and(|value| valid_text_allow_empty(value, 1_048_576))
            })
            || !valid_owasp_references(&row["references"])
            || !valid_sorted_string_array(row.get("cwes"), 65_536, 256)?
            || !valid_owasp_cvss(row.get("cvssv2"), OwaspCvssVersion::V2)
            || !valid_owasp_cvss(row.get("cvssv3"), OwaspCvssVersion::V3)
            || !valid_owasp_cvss(row.get("cvssv4"), OwaspCvssVersion::V4)
        {
            return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport));
        }
        let advisory_id = required_text(row.get("name"))?;
        let vulnerable_software_ids = parse_owasp_vulnerable_software(&row["vulnerableSoftware"])?;
        if !valid_advisory_id(ProviderId::OwaspNvd, advisory_id) {
            return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport));
        }
        result.push(ParsedOwaspVulnerability {
            advisory_id: advisory_id.to_owned(),
            vulnerable_software_ids,
        });
    }
    if result
        .windows(2)
        .any(|rows| rows[0].advisory_id >= rows[1].advisory_id)
    {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport));
    }
    Ok(result)
}

fn valid_owasp_references(value: &Value) -> bool {
    value
        .as_array()
        .filter(|rows| rows.len() <= 65_536)
        .is_some_and(|rows| {
            rows.iter().all(|row| {
                row.as_object().is_some_and(|row| {
                    object_keys_exact(row, &["source"], &["name", "url"])
                        && ["source", "name", "url"]
                            .iter()
                            .all(|key| optional_bounded_string(row.get(*key), 4096))
                })
            })
        })
}

fn parse_owasp_vulnerable_software(value: &Value) -> Result<BTreeSet<String>, AdvisoryError> {
    let rows = value
        .as_array()
        .filter(|rows| !rows.is_empty() && rows.len() <= 65_536)
        .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
    let mut identifiers = BTreeSet::new();
    for row in rows {
        let row = row
            .as_object()
            .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
        if !object_keys_exact(row, &["software"], &[]) {
            return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport));
        }
        let software = row["software"]
            .as_object()
            .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
        if !object_keys_exact(
            software,
            &["id"],
            &[
                "versionEndExcluding",
                "versionEndIncluding",
                "versionStartExcluding",
                "versionStartIncluding",
                "vulnerabilityIdMatched",
                "vulnerable",
            ],
        ) || software
            .get("vulnerabilityIdMatched")
            .is_some_and(|value| value.as_str() != Some("true"))
            || software
                .get("vulnerable")
                .is_some_and(|value| value.as_str() != Some("false"))
            || [
                "versionEndExcluding",
                "versionEndIncluding",
                "versionStartExcluding",
                "versionStartIncluding",
            ]
            .iter()
            .any(|key| !optional_bounded_string(software.get(*key), 256))
        {
            return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport));
        }
        let identifier = required_text(software.get("id"))?;
        if !valid_owasp_software_identifier(identifier)
            || !identifiers.insert(identifier.to_owned())
        {
            return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport));
        }
    }
    Ok(identifiers)
}

fn valid_owasp_software_identifier(value: &str) -> bool {
    valid_bounded_text(value, 4096)
        && (value.starts_with("cpe:2.3:")
            || value.starts_with("cpe:/")
            || parse_exact_package_url(value).is_ok())
}

#[derive(Clone, Copy)]
enum OwaspCvssVersion {
    V2,
    V3,
    V4,
}

fn valid_owasp_cvss(value: Option<&Value>, version: OwaspCvssVersion) -> bool {
    let Some(value) = value else {
        return true;
    };
    let Some(object) = value.as_object() else {
        return false;
    };
    let (required, optional, numeric): (&[&str], &[&str], &[&str]) = match version {
        OwaspCvssVersion::V2 => (
            &[
                "accessComplexity",
                "accessVector",
                "authenticationr",
                "availabilityImpact",
                "confidentialityImpact",
                "integrityImpact",
                "score",
                "severity",
            ],
            &[
                "acInsufInfo",
                "exploitabilityScore",
                "impactScore",
                "obtainAllPrivilege",
                "obtainOtherPrivilege",
                "obtainUserPrivilege",
                "userInteractionRequired",
                "version",
            ],
            &["score"],
        ),
        OwaspCvssVersion::V3 => (
            &[
                "attackComplexity",
                "attackVector",
                "availabilityImpact",
                "baseScore",
                "baseSeverity",
                "confidentialityImpact",
                "integrityImpact",
                "privilegesRequired",
                "scope",
                "userInteraction",
            ],
            &["exploitabilityScore", "impactScore", "version"],
            &["baseScore"],
        ),
        OwaspCvssVersion::V4 => (
            &[],
            &[
                "attackComplexity",
                "attackRequirements",
                "attackVector",
                "automatable",
                "availabilityRequirements",
                "baseScore",
                "baseSeverity",
                "confidentialityRequirements",
                "environmentalScore",
                "environmentalSeverity",
                "exploitMaturity",
                "integrityRequirements",
                "modifiedAttackComplexity",
                "modifiedAttackRequirements",
                "modifiedAttackVector",
                "modifiedPrivilegesRequired",
                "modifiedSubsequentSystemAvailability",
                "modifiedSubsequentSystemConfidentiality",
                "modifiedSubsequentSystemIntegrity",
                "modifiedUserInteraction",
                "modifiedVulnerableSystemAvailability",
                "modifiedVulnerableSystemConfidentiality",
                "modifiedVulnerableSystemIntegrity",
                "privilegesRequired",
                "providerUrgency",
                "recovery",
                "safety",
                "source",
                "subsequentSystemAvailability",
                "subsequentSystemConfidentiality",
                "subsequentSystemIntegrity",
                "threatScore",
                "threatSeverity",
                "type",
                "userInteraction",
                "valueDensity",
                "vectorString",
                "version",
                "vulnerabilityResponseEffort",
                "vulnerableSystemAvailability",
                "vulnerableSystemConfidentiality",
                "vulnerableSystemIntegrity",
            ],
            &["baseScore", "environmentalScore", "threatScore"],
        ),
    };
    object_keys_exact(object, required, optional)
        && object.iter().all(|(key, value)| {
            if numeric.contains(&key.as_str()) {
                value.as_f64().is_some_and(|value| value.is_finite())
            } else {
                value
                    .as_str()
                    .is_some_and(|value| valid_bounded_text(value, 4096))
            }
        })
}

fn validate_owasp_lineage(
    workload_id: &str,
    dependencies: &[ParsedOwaspDependency],
) -> Result<(), AdvisoryError> {
    const ROOT: usize = usize::MAX;
    let mut aliases = BTreeMap::<String, usize>::new();
    for root in [workload_id.to_owned(), format!("project:{workload_id}")] {
        if aliases.insert(root, ROOT).is_some() {
            return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport));
        }
    }
    for (index, dependency) in dependencies.iter().enumerate() {
        for alias in &dependency.aliases {
            if aliases.insert(alias.clone(), index).is_some() {
                return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport));
            }
        }
    }
    let mut states = vec![0_u8; dependencies.len()];
    fn reaches_root(
        index: usize,
        dependencies: &[ParsedOwaspDependency],
        aliases: &BTreeMap<String, usize>,
        states: &mut [u8],
    ) -> Result<bool, AdvisoryError> {
        if !dependencies[index].is_virtual {
            return Ok(true);
        }
        match states[index] {
            1 => return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport)),
            2 => return Ok(true),
            3 => return Ok(false),
            _ => {}
        }
        states[index] = 1;
        let mut reached = false;
        for parent in &dependencies[index].included_by {
            let target = aliases
                .get(parent)
                .copied()
                .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch))?;
            reached |= target == usize::MAX || reaches_root(target, dependencies, aliases, states)?;
        }
        states[index] = if reached { 2 } else { 3 };
        Ok(reached)
    }
    for index in 0..dependencies.len() {
        if dependencies[index].is_virtual
            && !reaches_root(index, dependencies, &aliases, &mut states)?
        {
            return Err(AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch));
        }
    }
    Ok(())
}

fn parse_exact_package_url(value: &str) -> Result<(String, String, String, String), AdvisoryError> {
    if !valid_bounded_text(value, 512)
        || value.contains(['%', '?', '#'])
        || !value.starts_with("pkg:maven/")
    {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport));
    }
    let body = &value["pkg:maven/".len()..];
    let (path, version) = body
        .split_once('@')
        .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
    let (namespace, name) = path
        .rsplit_once('/')
        .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
    if !valid_exact_identity_token(namespace, 256)
        || !valid_exact_identity_token(name, 256)
        || !valid_exact_identity_token(version, 128)
    {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport));
    }
    Ok((
        "maven".to_owned(),
        namespace.to_owned(),
        name.to_owned(),
        version.to_owned(),
    ))
}

fn package_identifier_compatible(value: &str, artifact: &GradleArtifact) -> bool {
    let parsed = parse_exact_package_url(value).ok();
    artifact.package_ecosystem == "maven"
        && valid_exact_identity_token(&artifact.package_namespace, 256)
        && parsed
            == Some((
                "maven".to_owned(),
                artifact.package_namespace.clone(),
                artifact.package_name.clone(),
                artifact.package_version.clone(),
            ))
        && value
            == format!(
                "pkg:maven/{}/{}@{}",
                artifact.package_namespace, artifact.package_name, artifact.package_version
            )
}

fn required_text(value: Option<&Value>) -> Result<&str, AdvisoryError> {
    value
        .and_then(Value::as_str)
        .filter(|value| valid_bounded_text(value, 256))
        .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))
}

fn validate_suppression_inventory(
    suppressions: &[Suppression],
    evaluation_epoch: u64,
) -> Result<(), AdvisoryError> {
    let mut previous = None;
    let mut identifiers = BTreeSet::new();
    for suppression in suppressions {
        if suppression.advisory_id == NON_WAIVABLE_RUSTSEC
            || !valid_advisory_id(suppression.provider, &suppression.advisory_id)
            || !valid_exact_identity_token(&suppression.id, 128)
            || !matches!(suppression.package_ecosystem.as_str(), "cargo" | "maven")
            || match suppression.package_ecosystem.as_str() {
                "cargo" => !suppression.package_namespace.is_empty(),
                "maven" => !valid_exact_identity_token(&suppression.package_namespace, 256),
                _ => true,
            }
            || !valid_exact_identity_token(&suppression.package_name, 256)
            || !valid_exact_identity_token(&suppression.package_version, 128)
            || !WORKLOADS
                .iter()
                .any(|workload| workload.id == suppression.workload_id)
            || !valid_bounded_text(&suppression.owner, 256)
            || !valid_bounded_text(&suppression.rationale, 1024)
            || !identifiers.insert(suppression.id.as_str())
            || suppression.created_at_epoch == 0
            || suppression.created_at_epoch >= suppression.expires_at_epoch
            || suppression.created_at_epoch > evaluation_epoch
        {
            return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot));
        }
        if evaluation_epoch >= suppression.expires_at_epoch {
            return Err(AdvisoryError::new(AdvisoryFailureKind::ExpiredSuppression));
        }
        let key = suppression_key(suppression);
        if previous.as_ref().is_some_and(|value| value >= &key) {
            return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot));
        }
        previous = Some(key);
    }
    Ok(())
}

fn apply_suppressions(
    findings: &mut Vec<Finding>,
    suppressions: &[Suppression],
    evaluation_epoch: u64,
) -> Result<(), AdvisoryError> {
    validate_suppression_inventory(suppressions, evaluation_epoch)?;
    for suppression in suppressions {
        let matches = findings
            .iter()
            .enumerate()
            .filter(|(_, finding)| suppression_matches(suppression, finding))
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot));
        }
        findings.remove(matches[0]);
    }
    Ok(())
}

fn suppression_matches(suppression: &Suppression, finding: &Finding) -> bool {
    suppression.provider == finding.provider
        && suppression.advisory_id == finding.advisory_id
        && suppression.package_ecosystem == finding.package_ecosystem
        && suppression.package_namespace == finding.package_namespace
        && suppression.package_name == finding.package_name
        && suppression.package_version == finding.package_version
        && suppression.workload_id == finding.workload_id
}

fn finding_key(finding: &Finding) -> (&str, &str, &str, &str, &str, &str, &str) {
    (
        finding.provider.as_str(),
        &finding.advisory_id,
        &finding.package_ecosystem,
        &finding.package_namespace,
        &finding.package_name,
        &finding.package_version,
        &finding.workload_id,
    )
}

fn suppression_key(suppression: &Suppression) -> (&str, &str, &str, &str, &str, &str, &str, &str) {
    (
        suppression.provider.as_str(),
        &suppression.advisory_id,
        &suppression.workload_id,
        &suppression.package_ecosystem,
        &suppression.package_namespace,
        &suppression.package_name,
        &suppression.package_version,
        &suppression.id,
    )
}

fn validate_blob(expected: &BlobBinding, actual: &FileEvidence) -> Result<(), AdvisoryError> {
    if expected.byte_length != actual.byte_length || expected.sha256 != actual.sha256 {
        Err(AdvisoryError::new(AdvisoryFailureKind::BindingChanged))
    } else {
        Ok(())
    }
}

fn valid_blob_binding(binding: &BlobBinding) -> bool {
    let maximum = if binding.media_type == "application/gzip" {
        2_147_483_648
    } else {
        MAX_REPORT_BYTES
    };
    binding.byte_length > 0
        && binding.byte_length <= maximum
        && valid_hex(&binding.sha256, 64)
        && SNAPSHOT_FILE_NAMES.contains(&binding.path.as_str())
        && binding.logical_uri == format!("extbuild-cas://sha256/{}", binding.sha256)
        && matches!(
            binding.media_type.as_str(),
            "application/gzip" | "application/json"
        )
        && matches!(
            binding.logical_role.as_str(),
            "rustsec_database_snapshot"
                | "owasp_nvd_database_snapshot"
                | "rustsec_unsuppressed_report"
                | "owasp_unsuppressed_report"
        )
}

fn valid_provider_blob_binding(provider: ProviderId, binding: &BlobBinding, archive: bool) -> bool {
    if !valid_blob_binding(binding) {
        return false;
    }
    matches!(
        (
            provider,
            archive,
            binding.path.as_str(),
            binding.media_type.as_str(),
            binding.logical_role.as_str()
        ),
        (
            ProviderId::Rustsec,
            true,
            RUSTSEC_ARCHIVE_NAME,
            "application/gzip",
            "rustsec_database_snapshot"
        ) | (
            ProviderId::Rustsec,
            false,
            RUSTSEC_REPORT_NAME,
            "application/json",
            "rustsec_unsuppressed_report"
        ) | (
            ProviderId::OwaspNvd,
            true,
            NVD_ARCHIVE_NAME,
            "application/gzip",
            "owasp_nvd_database_snapshot"
        ) | (
            ProviderId::OwaspNvd,
            false,
            OWASP_REPORT_NAME,
            "application/json",
            "owasp_unsuppressed_report"
        )
    )
}

fn inventory_digest(inventory: &[WorkloadInventory]) -> Result<String, AdvisoryError> {
    domain_json_digest(b"radroots-advisory-workload-inventory-v1\0", inventory)
}

fn source_digest(sources: &[SourceIdentity]) -> Result<String, AdvisoryError> {
    domain_json_digest(b"radroots-advisory-source-inventory-v1\0", sources)
}

fn tool_observation_digest(
    acquisitions: &[ToolAcquisition],
    states: &[ToolState],
) -> Result<String, AdvisoryError> {
    domain_json_digest(
        b"radroots-advisory-tool-observation-v1\0",
        &serde_json::json!({"acquisition": acquisitions, "state": states}),
    )
}

fn provider_trace_digest(providers: &[ProviderSnapshot]) -> Result<String, AdvisoryError> {
    domain_json_digest(b"radroots-advisory-provider-trace-v1\0", providers)
}

fn provider_database_identity(
    provider: ProviderId,
    materialized_tree_sha256: &str,
    report_database_metadata_sha256: &str,
) -> Result<String, AdvisoryError> {
    domain_json_digest(
        b"radroots-advisory-provider-database-identity-v1\0",
        &serde_json::json!({
            "materialized_tree_sha256": materialized_tree_sha256,
            "provider": provider,
            "report_database_metadata_sha256": report_database_metadata_sha256,
        }),
    )
}

fn database_identity_from_report(
    provider: ProviderId,
    report: &[u8],
    materialized_tree_sha256: &str,
) -> Result<String, AdvisoryError> {
    let envelope: ReportEnvelope = serde_json::from_slice(report)
        .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
    if envelope.provider != provider || envelope.workload_result.is_empty() {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport));
    }
    let mut metadata = None::<String>;
    for result in envelope.workload_result {
        let raw: Value = serde_json::from_str(&result.raw_scanner_output)
            .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
        let current = match provider {
            ProviderId::Rustsec => domain_json_digest(
                b"radroots-advisory-rustsec-database-metadata-v1\0",
                raw.get("database")
                    .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?,
            )?,
            ProviderId::OwaspNvd => domain_json_digest(
                b"radroots-advisory-owasp-database-metadata-v1\0",
                raw.pointer("/scanInfo/dataSource")
                    .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?,
            )?,
        };
        if metadata.as_ref().is_some_and(|value| value != &current) {
            return Err(AdvisoryError::new(AdvisoryFailureKind::BindingChanged));
        }
        metadata = Some(current);
    }
    provider_database_identity(
        provider,
        materialized_tree_sha256,
        &metadata.ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?,
    )
}

fn finding_digest(findings: &[Finding]) -> Result<String, AdvisoryError> {
    let rows = findings
        .iter()
        .map(|finding| {
            serde_json::json!({
                "advisory_id": finding.advisory_id,
                "package_ecosystem": finding.package_ecosystem,
                "package_namespace": finding.package_namespace,
                "package_name": finding.package_name,
                "package_version": finding.package_version,
                "provider": finding.provider,
                "workload_id": finding.workload_id,
            })
        })
        .collect::<Vec<_>>();
    domain_json_digest(b"radroots-advisory-finding-inventory-v1\0", &rows)
}

fn suppression_digest(suppressions: &[Suppression]) -> Result<String, AdvisoryError> {
    domain_json_digest(
        b"radroots-advisory-suppression-inventory-v1\0",
        suppressions,
    )
}

fn gradle_projection_digest(projections: &[GradleProjection]) -> Result<String, AdvisoryError> {
    domain_json_digest(b"radroots-advisory-gradle-projection-v1\0", projections)
}

fn candidate_advisory_input_digest(manifest: &SnapshotManifest) -> Result<String, AdvisoryError> {
    domain_json_digest(
        b"radroots-advisory-candidate-input-v1\0",
        &serde_json::json!({
            "candidate": manifest.candidate,
            "fresh_tool_observation_sha256": manifest.fresh_tool_observation_sha256,
            "gradle_projection": manifest.gradle_projection,
            "inventory": manifest.inventory,
            "provider_snapshot": manifest.provider_snapshot,
            "sources": manifest.sources,
            "step_297_tool_manifest_sha256": manifest.step_297_tool_manifest_sha256,
            "suppressions": manifest.suppressions,
            "tool_acquisition": manifest.tool_acquisition,
            "tool_state": manifest.tool_state,
        }),
    )
}

fn domain_json_digest<T: Serialize + ?Sized>(
    domain: &[u8],
    value: &T,
) -> Result<String, AdvisoryError> {
    let value = serde_json::to_value(value)
        .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot))?;
    let bytes = canonical_json_without_lf(&value);
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(bytes);
    Ok(hex::encode(hasher.finalize()))
}

fn materialized_tree_digest(snapshot: &TraversalSnapshot) -> Result<String, AdvisoryError> {
    let directories = snapshot
        .directories()
        .map(|(path, mode)| {
            path.to_str()
                .map(|path| serde_json::json!({"mode": mode, "path": path}))
                .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut files = Vec::with_capacity(snapshot.files().len());
    for file in snapshot.files() {
        let evidence = snapshot
            .hash(file, 17_179_869_184)
            .map_err(map_binding_error)?;
        let path = file
            .relative_path()
            .to_str()
            .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot))?;
        files.push(serde_json::json!({
            "byte_length": evidence.byte_length,
            "mode": file.permission_mode(),
            "path": path,
            "sha256": evidence.sha256,
        }));
    }
    snapshot.revalidate().map_err(map_binding_error)?;
    domain_json_digest(
        b"radroots-advisory-materialized-tree-v1\0",
        &serde_json::json!({
            "directories": directories,
            "entry_count": snapshot.entry_count(),
            "files": files,
            "total_bytes": snapshot.total_bytes(),
        }),
    )
}

fn archive_limits() -> TarGzipLimits {
    TarGzipLimits {
        max_compressed_bytes: 2_147_483_648,
        max_expanded_bytes: 17_179_869_184,
        max_members: 65_536,
        max_member_bytes: 17_179_869_184,
        max_payload_bytes: 17_179_869_184,
        max_depth: 64,
        max_path_bytes: 4_096,
    }
}

fn map_binding_error(error: safe_artifact_io::ArtifactIoError) -> AdvisoryError {
    match error.kind() {
        safe_artifact_io::ArtifactIoFailureKind::ChangedDuringRead => {
            AdvisoryError::new(AdvisoryFailureKind::BindingChanged)
        }
        _ => AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot),
    }
}

fn map_archive_error(error: safe_artifact_io::ArtifactIoError) -> AdvisoryError {
    match error.kind() {
        safe_artifact_io::ArtifactIoFailureKind::ChangedDuringRead => {
            AdvisoryError::new(AdvisoryFailureKind::BindingChanged)
        }
        _ => AdvisoryError::new(AdvisoryFailureKind::ArchiveRejected),
    }
}

fn canonical_json(value: &Value) -> Vec<u8> {
    let mut bytes = canonical_json_without_lf(value);
    bytes.push(b'\n');
    bytes
}

fn canonical_pretty_json(value: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec_pretty(value).unwrap_or_default();
    bytes.push(b'\n');
    bytes
}

fn canonical_json_without_lf(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).unwrap_or_default()
}

fn sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn valid_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_identifier(value: &str) -> bool {
    valid_bounded_text(value, 256)
        && !Path::new(value).is_absolute()
        && Path::new(value)
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
        && !value.contains('\\')
}

fn valid_bounded_text(value: &str, maximum: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum
        && !value.contains('\0')
        && !value.chars().any(char::is_control)
}

fn valid_exact_identity_token(value: &str, maximum: usize) -> bool {
    valid_bounded_text(value, maximum)
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(byte, b'-' | b'_' | b'.' | b'/' | b':' | b'@' | b'+')
        })
        && !value.contains("..")
}

fn valid_advisory_id(provider: ProviderId, value: &str) -> bool {
    let (prefix, minimum_digits) = match provider {
        ProviderId::Rustsec => ("RUSTSEC-", 9),
        ProviderId::OwaspNvd => ("CVE-", 9),
    };
    if !value.starts_with(prefix) || value.len() < prefix.len() + minimum_digits {
        return false;
    }
    let remainder = &value[prefix.len()..];
    let Some((year, sequence)) = remainder.split_once('-') else {
        return false;
    };
    year.len() == 4
        && year.bytes().all(|byte| byte.is_ascii_digit())
        && sequence.len() >= 4
        && sequence.bytes().all(|byte| byte.is_ascii_digit())
}

pub(crate) fn self_test() -> Result<(), String> {
    self_test_suite().map_err(|error| error.to_string())?;
    verify_gradle_raw_bridge().map_err(|error| error.to_string())?;
    verify_bounded_process_surface().map_err(|error| error.to_string())
}

fn verify_gradle_raw_bridge() -> Result<(), AdvisoryError> {
    let fixture = trusted_tempdir(".radroots-gradle-bridge-")?;
    let raw_root = fixture.path().join("raw");
    let source_root = fixture.path().join("source");
    let materialization_parent = fixture.path().join("materialized");
    for directory in [&raw_root, &source_root, &materialization_parent] {
        std::fs::create_dir(directory)
            .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::Unavailable))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o700))
                .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::Unavailable))?;
        }
    }
    let artifact_path = source_root.join("example-1.0.0.jar");
    let artifact_bytes = b"synthetic-gradle-artifact";
    std::fs::write(&artifact_path, artifact_bytes)
        .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::Unavailable))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&artifact_path, std::fs::Permissions::from_mode(0o600))
            .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::Unavailable))?;
    }
    let variant = RawGradleVariant {
        attributes: vec![RawGradleAttribute {
            name: "org.gradle.usage".to_owned(),
            value: "java-runtime".to_owned(),
        }],
        capabilities: Vec::new(),
        external_variant: None,
    };
    let root_core = RawGradleComponentCore {
        build_root: Some(".".to_owned()),
        group: None,
        kind: "project".to_owned(),
        name: None,
        project_path: Some(":app:design_system".to_owned()),
        version: None,
    };
    let module_core = RawGradleComponentCore {
        build_root: None,
        group: Some("com.example".to_owned()),
        kind: "module".to_owned(),
        name: Some("example".to_owned()),
        project_path: None,
        version: Some("1.0.0".to_owned()),
    };
    let mut components = vec![
        RawGradleComponent {
            build_root: root_core.build_root.clone(),
            group: root_core.group.clone(),
            kind: root_core.kind.clone(),
            name: root_core.name.clone(),
            project_path: root_core.project_path.clone(),
            root: true,
            variant: RawGradleVariantEnvelope {
                selected: vec![variant.clone()],
            },
            version: root_core.version.clone(),
        },
        RawGradleComponent {
            build_root: module_core.build_root.clone(),
            group: module_core.group.clone(),
            kind: module_core.kind.clone(),
            name: module_core.name.clone(),
            project_path: module_core.project_path.clone(),
            root: false,
            variant: RawGradleVariantEnvelope {
                selected: vec![variant.clone()],
            },
            version: module_core.version.clone(),
        },
    ];
    components.sort_by(|left, right| {
        canonical_row_key(left)
            .unwrap_or_default()
            .cmp(&canonical_row_key(right).unwrap_or_default())
    });
    let raw = RawGradleGraph {
        schema: "radroots.gradle-advisory-graph.v1".to_owned(),
        workload_id: "app_design_system".to_owned(),
        build_root: ".".to_owned(),
        project_path: ":app:design_system".to_owned(),
        configuration: "desktopRuntimeClasspath".to_owned(),
        components,
        edges: vec![RawGradleEdge {
            constraint: false,
            from: root_core,
            requested: RawGradleSelector {
                attributes: Vec::new(),
                build_root: None,
                capabilities: Vec::new(),
                group: Some("com.example".to_owned()),
                kind: "module".to_owned(),
                name: Some("example".to_owned()),
                project_path: None,
                version: Some("1.0.0".to_owned()),
                version_constraint: Some(RawGradleVersionConstraint {
                    branch: None,
                    preferred: String::new(),
                    rejected: Vec::new(),
                    required: "1.0.0".to_owned(),
                    strict: String::new(),
                }),
            },
            selected_variant: variant.clone(),
            to: module_core.clone(),
        }],
        artifacts: vec![RawGradleArtifact {
            artifact_name: "example-1.0.0.jar".to_owned(),
            artifact_type: "jar".to_owned(),
            classifier: None,
            component: module_core,
            extension: "jar".to_owned(),
            group: Some("com.example".to_owned()),
            logical_name: "example".to_owned(),
            module_version: RawGradleModuleVersion {
                group: "com.example".to_owned(),
                name: "example".to_owned(),
                version: "1.0.0".to_owned(),
            },
            name: Some("example".to_owned()),
            observed_byte_length: artifact_bytes.len() as u64,
            observed_sha256: sha256(artifact_bytes),
            source_path: artifact_path
                .to_str()
                .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot))?
                .to_owned(),
            variant,
            version: Some("1.0.0".to_owned()),
        }],
    };
    let raw_value = serde_json::to_value(raw)
        .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot))?;
    std::fs::write(
        raw_root.join(GRADLE_RAW_GRAPH_NAME),
        canonical_json_without_lf(&raw_value),
    )
    .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::Unavailable))?;
    let authority = WORKLOADS
        .iter()
        .find(|workload| workload.id == "app_design_system")
        .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidContract))?;
    let source_root_identity = "3".repeat(64);
    let source_roots = [GradleArtifactSourceRoot {
        path: &source_root,
        logical_role: "candidate_build_output",
        identity_sha256: &source_root_identity,
    }];
    let admitted = admit_raw_gradle_graph(
        &raw_root,
        &materialization_parent,
        authority,
        &"a".repeat(40),
        1,
        &source_roots,
    )?;
    if admitted.workload_id != authority.id
        || admitted.raw_evidence().byte_length == 0
        || admitted.components.len() != 2
        || admitted.edges.len() != 1
        || admitted.artifacts.len() != 1
        || admitted.artifacts[0].package_namespace != "com.example"
        || !valid_hex(&admitted.canonical_graph_sha256, 64)
        || !valid_hex(&admitted.materialized_tree_sha256, 64)
        || !valid_hex(&admitted.normalization_receipt_sha256, 64)
    {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot));
    }
    admitted.revalidate()
}

fn synthetic_admitted_gradle_graphs(
    fixture_root: &Path,
    materialization_parent: &Path,
) -> Result<Vec<AdmittedGradleGraph>, AdvisoryError> {
    let mut admitted = Vec::new();
    for (index, authority) in WORKLOADS
        .iter()
        .filter(|workload| workload.package_manager == "gradle")
        .enumerate()
    {
        let workload_root = fixture_root.join(authority.id);
        let raw_root = workload_root.join("raw");
        let source_root = workload_root.join("source");
        for directory in [&workload_root, &raw_root, &source_root] {
            std::fs::create_dir(directory)
                .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::Unavailable))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt as _;
                std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o700))
                    .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::Unavailable))?;
            }
        }
        let package_name = format!("fixture-{}", authority.id.replace('_', "-"));
        let artifact_name = format!("{package_name}-1.0.0.jar");
        let artifact_path = source_root.join(&artifact_name);
        let artifact_bytes = format!("synthetic-gradle-artifact-{index}").into_bytes();
        std::fs::write(&artifact_path, &artifact_bytes)
            .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::Unavailable))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&artifact_path, std::fs::Permissions::from_mode(0o600))
                .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::Unavailable))?;
        }
        let variant = RawGradleVariant {
            attributes: vec![RawGradleAttribute {
                name: "org.gradle.usage".to_owned(),
                value: "java-runtime".to_owned(),
            }],
            capabilities: Vec::new(),
            external_variant: None,
        };
        let root_core = RawGradleComponentCore {
            build_root: Some(authority.build_root.to_owned()),
            group: None,
            kind: "project".to_owned(),
            name: None,
            project_path: authority.project_path.map(str::to_owned),
            version: None,
        };
        let module_core = RawGradleComponentCore {
            build_root: None,
            group: Some("com.example".to_owned()),
            kind: "module".to_owned(),
            name: Some(package_name.clone()),
            project_path: None,
            version: Some("1.0.0".to_owned()),
        };
        let mut components = vec![
            RawGradleComponent {
                build_root: root_core.build_root.clone(),
                group: root_core.group.clone(),
                kind: root_core.kind.clone(),
                name: root_core.name.clone(),
                project_path: root_core.project_path.clone(),
                root: true,
                variant: RawGradleVariantEnvelope {
                    selected: vec![variant.clone()],
                },
                version: root_core.version.clone(),
            },
            RawGradleComponent {
                build_root: module_core.build_root.clone(),
                group: module_core.group.clone(),
                kind: module_core.kind.clone(),
                name: module_core.name.clone(),
                project_path: module_core.project_path.clone(),
                root: false,
                variant: RawGradleVariantEnvelope {
                    selected: vec![variant.clone()],
                },
                version: module_core.version.clone(),
            },
        ];
        components.sort_by(|left, right| {
            canonical_row_key(left)
                .unwrap_or_default()
                .cmp(&canonical_row_key(right).unwrap_or_default())
        });
        let raw = RawGradleGraph {
            schema: "radroots.gradle-advisory-graph.v1".to_owned(),
            workload_id: authority.id.to_owned(),
            build_root: authority.build_root.to_owned(),
            project_path: authority
                .project_path
                .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidContract))?
                .to_owned(),
            configuration: authority
                .configuration
                .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidContract))?
                .to_owned(),
            components,
            edges: vec![RawGradleEdge {
                constraint: false,
                from: root_core,
                requested: RawGradleSelector {
                    attributes: Vec::new(),
                    build_root: None,
                    capabilities: Vec::new(),
                    group: Some("com.example".to_owned()),
                    kind: "module".to_owned(),
                    name: Some(package_name.clone()),
                    project_path: None,
                    version: Some("1.0.0".to_owned()),
                    version_constraint: Some(RawGradleVersionConstraint {
                        branch: None,
                        preferred: String::new(),
                        rejected: Vec::new(),
                        required: "1.0.0".to_owned(),
                        strict: String::new(),
                    }),
                },
                selected_variant: variant.clone(),
                to: module_core.clone(),
            }],
            artifacts: vec![RawGradleArtifact {
                artifact_name,
                artifact_type: "jar".to_owned(),
                classifier: None,
                component: module_core,
                extension: "jar".to_owned(),
                group: Some("com.example".to_owned()),
                logical_name: package_name.clone(),
                module_version: RawGradleModuleVersion {
                    group: "com.example".to_owned(),
                    name: package_name.clone(),
                    version: "1.0.0".to_owned(),
                },
                name: Some(package_name),
                observed_byte_length: artifact_bytes.len() as u64,
                observed_sha256: sha256(&artifact_bytes),
                source_path: artifact_path
                    .to_str()
                    .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot))?
                    .to_owned(),
                variant,
                version: Some("1.0.0".to_owned()),
            }],
        };
        let value = serde_json::to_value(raw)
            .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot))?;
        std::fs::write(
            raw_root.join(GRADLE_RAW_GRAPH_NAME),
            canonical_json_without_lf(&value),
        )
        .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::Unavailable))?;
        let source_root_identity = format!("{:064x}", index + 20_000);
        let source_roots = [GradleArtifactSourceRoot {
            path: &source_root,
            logical_role: "candidate_build_output",
            identity_sha256: &source_root_identity,
        }];
        admitted.push(admit_raw_gradle_graph(
            &raw_root,
            materialization_parent,
            authority,
            &"1".repeat(40),
            1,
            &source_roots,
        )?);
    }
    Ok(admitted)
}

pub(crate) fn write_fixture_scanner_output(mode: &str) -> Result<(), String> {
    match mode {
        "report" => {
            println!("radroots-advisory-fixture-report-v1");
            Ok(())
        }
        "timeout" => {
            std::thread::sleep(Duration::from_secs(5));
            Ok(())
        }
        _ => Err("unknown advisory fixture scanner mode".to_owned()),
    }
}

fn verify_bounded_process_surface() -> Result<(), AdvisoryError> {
    let executable = std::env::current_exe()
        .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::Unavailable))?;
    let working =
        tempfile::tempdir().map_err(|_| AdvisoryError::new(AdvisoryFailureKind::Unavailable))?;
    let environment = private_process_environment(working.path())?;
    let output = crate::bounded_process::run(
        &crate::bounded_process::ProcessRequest::new(executable.clone())
            .arg("advisory-snapshot-fixture-scanner")
            .arg("--mode")
            .arg("report")
            .current_dir(working.path())
            .environment(environment)
            .deadline(Duration::from_secs(2))
            .output_limits(1024, 1024),
    )
    .map_err(map_process_error)?;
    if !output.status().success()
        || output.stdout() != b"radroots-advisory-fixture-report-v1\n"
        || !output.stderr().is_empty()
    {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidReport));
    }
    let error = crate::bounded_process::run(
        &crate::bounded_process::ProcessRequest::new(executable)
            .arg("advisory-snapshot-fixture-scanner")
            .arg("--mode")
            .arg("timeout")
            .current_dir(working.path())
            .environment(private_process_environment(working.path())?)
            .deadline(Duration::from_millis(50))
            .output_limits(1024, 1024),
    )
    .err()
    .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot))?;
    if error.kind() != crate::bounded_process::ProcessFailureKind::DeadlineExceeded {
        return Err(map_process_error(error));
    }
    Ok(())
}

fn private_process_environment(
    root: &Path,
) -> Result<crate::bounded_process::ReplacementEnvironment, AdvisoryError> {
    let mut environment = crate::bounded_process::ReplacementEnvironment::default();
    for (name, value) in [
        ("CARGO_HOME", root.as_os_str().to_owned()),
        ("GRADLE_USER_HOME", root.as_os_str().to_owned()),
        ("HOME", root.as_os_str().to_owned()),
        ("LC_ALL", OsString::from("C")),
        ("PATH", OsString::from("/usr/bin:/bin")),
        ("TMPDIR", root.as_os_str().to_owned()),
        ("TZ", OsString::from("UTC")),
    ] {
        environment.insert(name, value).map_err(map_process_error)?;
    }
    Ok(environment)
}

fn map_process_error(error: crate::bounded_process::ProcessError) -> AdvisoryError {
    match error.kind() {
        crate::bounded_process::ProcessFailureKind::DeadlineExceeded => {
            AdvisoryError::new(AdvisoryFailureKind::TimedOut)
        }
        crate::bounded_process::ProcessFailureKind::Spawn => {
            AdvisoryError::new(AdvisoryFailureKind::Unavailable)
        }
        _ => AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot),
    }
}

fn self_test_suite() -> Result<(), AdvisoryError> {
    let fixture = SyntheticFixture::new()?;
    let evidence = admit_snapshot(
        fixture.root(),
        fixture.materialization_parent.path(),
        &fixture.request,
    )?;
    if evidence.candidate != fixture.request.candidate
        || evidence.inventory_sha256 != inventory_digest(&fixture.request.inventory)?
        || evidence.finding_count != 0
        || evidence.suppression_count != 0
        || evidence.rustsec_archive.compressed.byte_length == 0
        || evidence.nvd_archive.compressed.byte_length == 0
        || evidence.provider_freshness.len() != 2
    {
        return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot));
    }

    missing_inventory_vector()?;
    unavailable_provider_vector()?;
    stale_snapshot_vector()?;
    timed_out_operation_vector()?;
    known_vulnerable_vector()?;
    expired_suppression_vector()?;
    Ok(())
}

fn missing_inventory_vector() -> Result<(), AdvisoryError> {
    let mut missing = SyntheticFixture::new()?;
    missing.manifest.inventory.pop();
    missing.seal()?;
    require_failure(&missing, AdvisoryFailureKind::InventoryMismatch)
}

fn unavailable_provider_vector() -> Result<(), AdvisoryError> {
    let mut unavailable = SyntheticFixture::new()?;
    unavailable.manifest.tool_state[0].state = OperationState::Unavailable;
    unavailable.seal()?;
    require_failure(&unavailable, AdvisoryFailureKind::Unavailable)
}

fn stale_snapshot_vector() -> Result<(), AdvisoryError> {
    let mut stale = SyntheticFixture::new()?;
    stale.manifest.provider_snapshot[0].digest_time_epoch = stale
        .request
        .evaluation_epoch
        .checked_sub(QUALIFICATION_FRESHNESS_SECONDS + 1)
        .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot))?;
    stale.manifest.provider_snapshot[0].acquired_at_epoch = stale.manifest.provider_snapshot[0]
        .digest_time_epoch
        .checked_sub(1)
        .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot))?;
    stale.seal()?;
    require_failure(&stale, AdvisoryFailureKind::StaleSnapshot)
}

fn timed_out_operation_vector() -> Result<(), AdvisoryError> {
    let mut timeout = SyntheticFixture::new()?;
    timeout.manifest.provider_snapshot[1].analysis_state = OperationState::TimedOut;
    timeout.seal()?;
    require_failure(&timeout, AdvisoryFailureKind::TimedOut)
}

fn known_vulnerable_vector() -> Result<(), AdvisoryError> {
    let mut vulnerable = SyntheticFixture::new()?;
    vulnerable.set_rustsec_finding(NON_WAIVABLE_RUSTSEC, "lru", "0.16.4")?;
    require_failure(&vulnerable, AdvisoryFailureKind::KnownVulnerability)
}

fn expired_suppression_vector() -> Result<(), AdvisoryError> {
    let mut expired = SyntheticFixture::new()?;
    let expired_artifact = expired
        .manifest
        .gradle_projection
        .first()
        .and_then(|projection| projection.artifacts.first())
        .cloned()
        .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot))?;
    expired.set_owasp_finding(
        "CVE-2099-0001",
        &expired_artifact.package_namespace,
        &expired_artifact.package_name,
        &expired_artifact.package_version,
    )?;
    expired.manifest.suppressions.push(Suppression {
        provider: ProviderId::OwaspNvd,
        advisory_id: "CVE-2099-0001".to_owned(),
        id: "synthetic-expired".to_owned(),
        package_ecosystem: "maven".to_owned(),
        package_namespace: expired_artifact.package_namespace,
        package_name: expired_artifact.package_name,
        package_version: expired_artifact.package_version,
        workload_id: "app_design_system".to_owned(),
        owner: "security@example.invalid".to_owned(),
        rationale: "synthetic expiry vector".to_owned(),
        created_at_epoch: expired.request.evaluation_epoch - 100,
        expires_at_epoch: expired.request.evaluation_epoch,
    });
    expired.seal()?;
    require_failure(&expired, AdvisoryFailureKind::ExpiredSuppression)
}

fn require_failure(
    fixture: &SyntheticFixture,
    expected: AdvisoryFailureKind,
) -> Result<(), AdvisoryError> {
    let error = admit_snapshot(
        fixture.root(),
        fixture.materialization_parent.path(),
        &fixture.request,
    )
    .err()
    .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot))?;
    if error.kind() == expected {
        Ok(())
    } else {
        Err(AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot))
    }
}

struct SyntheticFixture {
    directory: tempfile::TempDir,
    materialization_parent: tempfile::TempDir,
    _gradle_fixture_root: tempfile::TempDir,
    scanner_fixture_root: tempfile::TempDir,
    scanner_generation: u64,
    request: AdmissionRequest,
    manifest: SnapshotManifest,
    process_receipts: Vec<TrustedProcessReceipt>,
}

impl SyntheticFixture {
    fn new() -> Result<Self, AdvisoryError> {
        use std::fs;

        let directory = tempfile::tempdir()
            .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot))?;
        let materialization_parent = trusted_tempdir(".rshr-advisory-materialization-")?;
        let gradle_fixture_root = trusted_tempdir(".rshr-advisory-gradle-input-")?;
        let scanner_fixture_root = trusted_tempdir(".rshr-advisory-scanner-output-")?;
        let evaluation_epoch = 1_000_000;
        let candidate = CandidateIdentity {
            generation: 1,
            digest: "9".repeat(64),
        };
        let sources = synthetic_sources();
        let inventory = synthetic_inventory();
        let producer_request = synthetic_producer_request(&candidate);
        let producer_request_bytes = canonical_authority_bytes(&producer_request)?;
        let producer_request_sha256 = sha256(&producer_request_bytes);
        let rustsec_archive = deterministic_archive(b"synthetic-rustsec")?;
        let nvd_archive = deterministic_archive(b"synthetic-nvd")?;
        fs::write(
            directory.path().join(RUSTSEC_ARCHIVE_NAME),
            &rustsec_archive,
        )
        .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot))?;
        fs::write(directory.path().join(NVD_ARCHIVE_NAME), &nvd_archive)
            .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot))?;
        let tool_acquisition = expected_tool_acquisitions()?;
        let tool_state = synthetic_tool_states(evaluation_epoch);
        let tool_observation_sha256 = tool_observation_digest(&tool_acquisition, &tool_state)?;
        let (rustsec_tree, rustsec_archive_evidence) =
            synthetic_archive_tree_digest(&rustsec_archive, materialization_parent.path())?;
        let (nvd_tree, nvd_archive_evidence) =
            synthetic_archive_tree_digest(&nvd_archive, materialization_parent.path())?;
        let nvd_trace = synthetic_nvd_trace(&candidate, &producer_request_sha256);
        let nvd_trace_bytes = canonical_authority_bytes(&nvd_trace)?;
        let admitted_gradle_graphs = synthetic_admitted_gradle_graphs(
            gradle_fixture_root.path(),
            materialization_parent.path(),
        )?;
        let mut gradle_projection =
            synthetic_gradle_projections(&admitted_gradle_graphs, &inventory, &sources)?;
        let mut process_receipts = vec![synthetic_process_receipt(
            "owasp_nvd_update",
            TOOL_PINS[1].executable_sha256,
            &strings(&OWASP_UPDATE_ARGUMENTS),
            &producer_request_sha256,
            &nvd_update_output_digest(&nvd_tree, &sha256(&nvd_trace_bytes))?,
            vec![ProcessPathBinding {
                argument_index: 2,
                logical_role: "nvd_update_data".to_owned(),
                identity_sha256: nvd_tree.clone(),
            }],
            evaluation_epoch - 3_000,
            evaluation_epoch - 2_990,
            0,
        )?];
        attach_gradle_receipts(
            &mut gradle_projection,
            &mut process_receipts,
            evaluation_epoch,
        )?;
        let rustsec_report = synthetic_report(
            ProviderId::Rustsec,
            &inventory,
            &gradle_projection,
            &sha256(&rustsec_archive),
            &rustsec_tree,
            &tool_observation_sha256,
            &mut process_receipts,
            evaluation_epoch,
        )?;
        let owasp_report = synthetic_report(
            ProviderId::OwaspNvd,
            &inventory,
            &gradle_projection,
            &sha256(&nvd_archive),
            &nvd_tree,
            &tool_observation_sha256,
            &mut process_receipts,
            evaluation_epoch,
        )?;
        fs::write(directory.path().join(RUSTSEC_REPORT_NAME), &rustsec_report)
            .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot))?;
        fs::write(directory.path().join(OWASP_REPORT_NAME), &owasp_report)
            .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot))?;

        let provider_snapshot = vec![
            synthetic_provider(
                ProviderId::Rustsec,
                &rustsec_archive,
                &rustsec_report,
                &rustsec_tree,
                &rustsec_archive_evidence,
                &producer_request_sha256,
                &nvd_trace_bytes,
                evaluation_epoch,
            )?,
            synthetic_provider(
                ProviderId::OwaspNvd,
                &nvd_archive,
                &owasp_report,
                &nvd_tree,
                &nvd_archive_evidence,
                &producer_request_sha256,
                &nvd_trace_bytes,
                evaluation_epoch,
            )?,
        ];
        let mut manifest = SnapshotManifest {
            schema: "radroots.advisory-snapshot.v1".to_owned(),
            candidate: candidate.clone(),
            sources: sources.clone(),
            inventory_sha256: inventory_digest(&inventory)?,
            inventory: inventory.clone(),
            candidate_advisory_input_sha256: String::new(),
            step_297_tool_manifest_sha256: String::new(),
            fresh_tool_observation_sha256: String::new(),
            tool_acquisition,
            tool_state,
            gradle_projection,
            provider_snapshot,
            suppressions: Vec::new(),
        };
        let request = AdmissionRequest {
            candidate,
            sources,
            inventory,
            evaluation_epoch,
            freshness: FreshnessMode::Qualification,
            producer_request: producer_request_bytes,
            step_297_tool_manifest: Vec::new(),
            fresh_tool_observation: Vec::new(),
            nvd_network_trace: nvd_trace_bytes,
            provider_execution_evidence: Vec::new(),
            admitted_gradle_graphs,
            admitted_scanner_outputs: Vec::new(),
        };
        manifest.candidate_advisory_input_sha256 = candidate_advisory_input_digest(&manifest)?;
        let mut fixture = Self {
            directory,
            materialization_parent,
            _gradle_fixture_root: gradle_fixture_root,
            scanner_fixture_root,
            scanner_generation: 0,
            request,
            manifest,
            process_receipts,
        };
        fixture.seal()?;
        Ok(fixture)
    }

    fn root(&self) -> &Path {
        self.directory.path()
    }

    fn seal(&mut self) -> Result<(), AdvisoryError> {
        let producer_request = sha256(&self.request.producer_request);
        let tool_manifest = TrustedToolManifest {
            schema: "radroots.advisory-tool-manifest-projection.v1".to_owned(),
            candidate_generation: self.request.candidate.generation,
            candidate_digest: self.request.candidate.digest.clone(),
            platform: "macos_aarch64".to_owned(),
            producer_request_sha256: producer_request.clone(),
            tool_acquisition: self.manifest.tool_acquisition.clone(),
        };
        self.request.step_297_tool_manifest = canonical_authority_bytes(&tool_manifest)?;
        let observation = TrustedToolObservation {
            schema: "radroots.advisory-tool-observation.v1".to_owned(),
            candidate_generation: self.request.candidate.generation,
            candidate_digest: self.request.candidate.digest.clone(),
            platform: "macos_aarch64".to_owned(),
            producer_request_sha256: producer_request.clone(),
            tool_manifest_sha256: sha256(&self.request.step_297_tool_manifest),
            row_projection_sha256: tool_observation_digest(
                &self.manifest.tool_acquisition,
                &self.manifest.tool_state,
            )?,
            observed_at_epoch: self.request.evaluation_epoch - 4_000,
            tool_state: self.manifest.tool_state.clone(),
        };
        self.request.fresh_tool_observation = canonical_authority_bytes(&observation)?;
        self.manifest.step_297_tool_manifest_sha256 = sha256(&self.request.step_297_tool_manifest);
        self.manifest.fresh_tool_observation_sha256 = sha256(&self.request.fresh_tool_observation);
        self.refresh_gradle_receipts()?;
        self.refresh_admitted_scanner_outputs()?;
        self.manifest.candidate_advisory_input_sha256 =
            candidate_advisory_input_digest(&self.manifest)?;
        let provider_evidence = TrustedProviderEvidence {
            schema: "radroots.advisory-provider-execution-evidence.v1".to_owned(),
            candidate_generation: self.request.candidate.generation,
            candidate_digest: self.request.candidate.digest.clone(),
            platform: "macos_aarch64".to_owned(),
            producer_request_sha256: producer_request,
            candidate_advisory_input_sha256: self.manifest.candidate_advisory_input_sha256.clone(),
            gradle_projection: self.manifest.gradle_projection.clone(),
            provider_snapshot: self.manifest.provider_snapshot.clone(),
            process_receipt: self.process_receipts.clone(),
            suppressions: self.manifest.suppressions.clone(),
        };
        self.request.provider_execution_evidence = canonical_authority_bytes(&provider_evidence)?;
        let value = serde_json::to_value(&self.manifest)
            .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot))?;
        std::fs::write(self.root().join(MANIFEST_NAME), canonical_json(&value))
            .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot))
    }

    fn refresh_admitted_scanner_outputs(&mut self) -> Result<(), AdvisoryError> {
        self.scanner_generation = self
            .scanner_generation
            .checked_add(1)
            .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot))?;
        let generation_root = self
            .scanner_fixture_root
            .path()
            .join(format!("generation-{}", self.scanner_generation));
        std::fs::create_dir(&generation_root)
            .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::Unavailable))?;
        set_private_directory_mode(&generation_root)?;
        let mut admitted = Vec::with_capacity(WORKLOADS.len());
        for provider in [ProviderId::Rustsec, ProviderId::OwaspNvd] {
            let provider_root = generation_root.join(provider.as_str());
            std::fs::create_dir(&provider_root)
                .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::Unavailable))?;
            set_private_directory_mode(&provider_root)?;
            let report = self.read_report_value(provider)?;
            let rows = report["workload_result"]
                .as_array()
                .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
            for row in rows {
                let workload_id = required_text(row.get("workload_id"))?;
                let raw = row
                    .get("raw_scanner_output")
                    .and_then(Value::as_str)
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
                let workload_root = provider_root.join(workload_id);
                std::fs::create_dir(&workload_root)
                    .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::Unavailable))?;
                set_private_directory_mode(&workload_root)?;
                let output = workload_root.join(RAW_SCANNER_OUTPUT_NAME);
                std::fs::write(&output, raw.as_bytes())
                    .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::Unavailable))?;
                set_private_file_mode(&output)?;
                admitted.push(admit_raw_scanner_output(
                    &workload_root,
                    provider,
                    workload_id,
                )?);
            }
        }
        self.request.admitted_scanner_outputs = admitted;
        Ok(())
    }

    fn refresh_gradle_receipts(&mut self) -> Result<(), AdvisoryError> {
        let source = self
            .request
            .sources
            .iter()
            .find(|source| source.repository == "oss/harvestcircle")
            .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch))?;
        for projection in &mut self.manifest.gradle_projection {
            let inventory = self
                .request
                .inventory
                .iter()
                .find(|row| row.id == projection.workload_id)
                .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch))?;
            projection.input_sha256 =
                gradle_candidate_input_digest(&self.request, inventory, source, projection)?;
            let receipt = self
                .process_receipts
                .iter_mut()
                .find(|receipt| receipt.id == format!("gradle:{}", projection.workload_id))
                .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch))?;
            receipt.input_sha256 = gradle_process_input_digest(projection)?;
            receipt.output_sha256 = gradle_process_output_digest(projection)?;
            receipt.path_binding = vec![
                ProcessPathBinding {
                    argument_index: 5,
                    logical_role: "gradle_build_root".to_owned(),
                    identity_sha256: gradle_build_root_binding_digest(projection)?,
                },
                ProcessPathBinding {
                    argument_index: 7,
                    logical_role: "gradle_init_script".to_owned(),
                    identity_sha256: projection.init_script_sha256.clone(),
                },
                ProcessPathBinding {
                    argument_index: 10,
                    logical_role: "gradle_projection_output".to_owned(),
                    identity_sha256: projection.raw_graph_sha256.clone(),
                },
            ];
            projection.process_receipt_sha256 = process_receipt_digest(receipt)?;
        }
        Ok(())
    }

    fn set_rustsec_finding(
        &mut self,
        advisory_id: &str,
        package: &str,
        version: &str,
    ) -> Result<(), AdvisoryError> {
        let mut report = self.read_report_value(ProviderId::Rustsec)?;
        let raw = report["workload_result"][0]["raw_scanner_output"]
            .as_str()
            .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
        let mut raw: Value = serde_json::from_str(raw)
            .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
        raw["vulnerabilities"] = serde_json::json!({
            "count": 1,
            "found": true,
            "list": [{
                "advisory": {
                    "aliases": [],
                    "categories": [],
                    "collection": "crates",
                    "cvss": null,
                    "date": "2099-01-01",
                    "description": "synthetic known-vulnerable RustSec fixture",
                    "expect-deleted": false,
                    "id": advisory_id,
                    "informational": null,
                    "keywords": [],
                    "license": "CC0-1.0",
                    "package": package,
                    "references": [],
                    "related": [],
                    "source": "registry+https://github.com/rust-lang/crates.io-index",
                    "title": "synthetic known-vulnerable fixture",
                    "url": null,
                    "withdrawn": null
                },
                "affected": null,
                "package": {
                    "checksum": null,
                    "name": package,
                    "replace": null,
                    "source": "registry+https://github.com/rust-lang/crates.io-index",
                    "version": version
                },
                "versions": {"patched": [], "unaffected": []}
            }]
        });
        set_raw_output(&mut report["workload_result"][0], &raw, 1)?;
        self.replace_report(ProviderId::Rustsec, report)
    }

    fn set_owasp_finding(
        &mut self,
        advisory_id: &str,
        package_namespace: &str,
        package: &str,
        version: &str,
    ) -> Result<(), AdvisoryError> {
        let mut report = self.read_report_value(ProviderId::OwaspNvd)?;
        let raw = report["workload_result"][0]["raw_scanner_output"]
            .as_str()
            .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
        let mut raw: Value = serde_json::from_str(raw)
            .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
        let dependency = raw["dependencies"]
            .as_array_mut()
            .and_then(|rows| rows.first_mut())
            .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
        dependency["packages"] = serde_json::json!([{
            "id": format!("pkg:maven/{package_namespace}/{package}@{version}")
        }]);
        let matched_cpe =
            format!("cpe:2.3:a:{package_namespace}:{package}:{version}:*:*:*:*:*:*:*");
        dependency["vulnerabilityIds"] = serde_json::json!([{
            "id": matched_cpe
        }]);
        dependency["vulnerabilities"] = serde_json::json!([{
            "description": "synthetic known-vulnerable fixture",
            "name": advisory_id,
            "notes": "synthetic unsuppressed scanner output",
            "references": [],
            "source": "NVD",
            "vulnerableSoftware": [{"software": {"id": matched_cpe}}]
        }]);
        set_raw_output(&mut report["workload_result"][0], &raw, 0)?;
        self.replace_report(ProviderId::OwaspNvd, report)
    }

    fn read_report_value(&self, provider: ProviderId) -> Result<Value, AdvisoryError> {
        let name = report_name(provider);
        let bytes = std::fs::read(self.root().join(name))
            .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot))?;
        serde_json::from_slice(&bytes)
            .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))
    }

    fn replace_report(
        &mut self,
        provider: ProviderId,
        mut report: Value,
    ) -> Result<(), AdvisoryError> {
        self.refresh_report_receipts(provider, &mut report)?;
        let bytes = canonical_json(&report);
        let name = report_name(provider);
        std::fs::write(self.root().join(name), &bytes)
            .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot))?;
        let snapshot = self
            .manifest
            .provider_snapshot
            .iter_mut()
            .find(|snapshot| snapshot.provider == provider)
            .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot))?;
        snapshot.report = blob(name, &bytes)?;
        self.seal()
    }

    fn refresh_report_receipts(
        &mut self,
        provider: ProviderId,
        report: &mut Value,
    ) -> Result<(), AdvisoryError> {
        let rows = report["workload_result"]
            .as_array_mut()
            .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
        for row in rows {
            let workload_id = required_text(row.get("workload_id"))?;
            let prefix = match provider {
                ProviderId::Rustsec => "cargo_audit",
                ProviderId::OwaspNvd => "owasp_analysis",
            };
            let receipt = self
                .process_receipts
                .iter_mut()
                .find(|receipt| receipt.id == format!("{prefix}:{workload_id}"))
                .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch))?;
            let raw = row
                .get("raw_scanner_output")
                .and_then(Value::as_str)
                .filter(|raw| !raw.is_empty() && raw.len() <= MAX_REPORT_BYTES as usize)
                .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
            receipt.exit_code = row["exit_code"]
                .as_i64()
                .and_then(|value| i32::try_from(value).ok())
                .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
            if provider == ProviderId::Rustsec {
                receipt.stdout_byte_length = raw.len() as u64;
                receipt.stdout_sha256 = sha256(raw.as_bytes());
                receipt.output_sha256 = receipt.stdout_sha256.clone();
            } else {
                receipt.stdout_byte_length = 0;
                receipt.stdout_sha256 = sha256(&[]);
                receipt.output_sha256 = sha256(raw.as_bytes());
                let report_binding = receipt
                    .path_binding
                    .iter_mut()
                    .find(|binding| binding.logical_role == "raw_report_output")
                    .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot))?;
                report_binding.identity_sha256 = sha256(raw.as_bytes());
            }
            row["process_receipt_sha256"] = Value::from(process_receipt_digest(receipt)?);
        }
        Ok(())
    }
}

fn expected_tool_acquisitions() -> Result<Vec<ToolAcquisition>, AdvisoryError> {
    let decision: Value = serde_json::from_slice(DECISION_BYTES)
        .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::InvalidContract))?;
    let acquisitions = decision
        .get("tool_acquisition")
        .and_then(Value::as_object)
        .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidContract))?;
    TOOL_PINS
        .iter()
        .map(|pin| {
            acquisitions
                .get(pin.id)
                .cloned()
                .map(|projection| ToolAcquisition {
                    id: pin.id.to_owned(),
                    projection,
                })
                .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InvalidContract))
        })
        .collect()
}

fn synthetic_sources() -> Vec<SourceIdentity> {
    CANDIDATE_REPOSITORIES
        .into_iter()
        .map(|repository| SourceIdentity {
            repository: repository.to_owned(),
            revision: "1".repeat(40),
            tree: "2".repeat(40),
        })
        .collect()
}

fn synthetic_inventory() -> Vec<WorkloadInventory> {
    WORKLOADS
        .iter()
        .enumerate()
        .map(|(index, workload)| WorkloadInventory {
            id: workload.id.to_owned(),
            repository: workload.repository.to_owned(),
            build_root: workload.build_root.to_owned(),
            package_manager: workload.package_manager.to_owned(),
            language: workload.language.to_owned(),
            manifest_path: workload.manifest_path.map(str::to_owned),
            lockfile_path: workload.lockfile_path.map(str::to_owned),
            project_path: workload.project_path.map(str::to_owned),
            configuration: workload.configuration.map(str::to_owned),
            dependency_count: 1,
            input_sha256: format!("{:064x}", index + 1),
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn synthetic_provider(
    provider: ProviderId,
    archive: &[u8],
    report: &[u8],
    materialized_tree_sha256: &str,
    archive_evidence: &ArchiveEvidence,
    producer_request_sha256: &str,
    nvd_trace: &[u8],
    evaluation_epoch: u64,
) -> Result<ProviderSnapshot, AdvisoryError> {
    let (
        acquisition_kind,
        archive_name,
        report_name,
        acquisition_arguments,
        analysis_environment,
        analysis_arguments,
    ) = match provider {
        ProviderId::Rustsec => (
            "externally_admitted_immutable_snapshot",
            RUSTSEC_ARCHIVE_NAME,
            RUSTSEC_REPORT_NAME,
            Vec::new(),
            "fresh_private_config_free_workdir_home_and_cargo_home",
            strings(&CARGO_AUDIT_ARGUMENTS),
        ),
        ProviderId::OwaspNvd => (
            "bounded_nvd_update_only",
            NVD_ARCHIVE_NAME,
            OWASP_REPORT_NAME,
            strings(&OWASP_UPDATE_ARGUMENTS),
            "replacement_allowlist_private_data_and_output",
            strings(&OWASP_OFFLINE_ARGUMENTS),
        ),
    };
    Ok(ProviderSnapshot {
        provider,
        acquisition_kind: acquisition_kind.to_owned(),
        acquisition_count: 1,
        acquisition_state: OperationState::Complete,
        acquired_at_epoch: evaluation_epoch - 2_990,
        digest_time_epoch: evaluation_epoch - 2_900,
        database_identity_sha256: database_identity_from_report(
            provider,
            report,
            materialized_tree_sha256,
        )?,
        materialized_tree_sha256: materialized_tree_sha256.to_owned(),
        bounded_deadline_seconds: ANALYSIS_DEADLINE_SECONDS,
        network_mode: match provider {
            ProviderId::Rustsec => "external_snapshot_admission".to_owned(),
            ProviderId::OwaspNvd => "contracted_fetch_only".to_owned(),
        },
        network_trace_sha256: match provider {
            ProviderId::Rustsec => "0".repeat(64),
            ProviderId::OwaspNvd => sha256(nvd_trace),
        },
        producer_request_sha256: producer_request_sha256.to_owned(),
        acquisition_arguments,
        archive_format: "deterministic_tar_gzip_exact_bytes_normalized_headers_sorted_members"
            .to_owned(),
        archive: blob(archive_name, archive)?,
        archive_expanded_bytes: archive_evidence.expanded_bytes,
        archive_member_count: archive_evidence.member_count,
        archive_payload_bytes: archive_evidence.payload_bytes,
        analysis_state: OperationState::Complete,
        analyzed_at_epoch: evaluation_epoch
            - match provider {
                ProviderId::Rustsec => 2_505,
                ProviderId::OwaspNvd => 2_245,
            },
        analysis_environment: analysis_environment.to_owned(),
        analysis_arguments,
        report: blob(report_name, report)?,
    })
}

fn blob(path: &str, bytes: &[u8]) -> Result<BlobBinding, AdvisoryError> {
    let (media_type, logical_role) = match path {
        RUSTSEC_ARCHIVE_NAME => ("application/gzip", "rustsec_database_snapshot"),
        NVD_ARCHIVE_NAME => ("application/gzip", "owasp_nvd_database_snapshot"),
        RUSTSEC_REPORT_NAME => ("application/json", "rustsec_unsuppressed_report"),
        OWASP_REPORT_NAME => ("application/json", "owasp_unsuppressed_report"),
        _ => return Err(AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot)),
    };
    let digest = sha256(bytes);
    Ok(BlobBinding {
        path: path.to_owned(),
        byte_length: bytes.len() as u64,
        logical_uri: format!("extbuild-cas://sha256/{digest}"),
        media_type: media_type.to_owned(),
        logical_role: logical_role.to_owned(),
        sha256: digest,
    })
}

fn strings<const N: usize>(values: &[&str; N]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

fn report_name(provider: ProviderId) -> &'static str {
    match provider {
        ProviderId::Rustsec => RUSTSEC_REPORT_NAME,
        ProviderId::OwaspNvd => OWASP_REPORT_NAME,
    }
}

fn canonical_authority_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, AdvisoryError> {
    let value = serde_json::to_value(value)
        .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot))?;
    Ok(canonical_json(&value))
}

fn synthetic_tool_states(evaluation_epoch: u64) -> Vec<ToolState> {
    TOOL_PINS
        .iter()
        .map(|pin| ToolState {
            id: pin.id.to_owned(),
            state: OperationState::Complete,
            reviewed_at_epoch: evaluation_epoch - 4_100,
            normalized_version: pin.version.to_owned(),
            executable_sha256: pin.executable_sha256.to_owned(),
            source_sha256: pin.source_sha256.to_owned(),
            package_receipt_sha256: pin.receipt_sha256.to_owned(),
        })
        .collect()
}

fn synthetic_nvd_trace(
    candidate: &CandidateIdentity,
    producer_request_sha256: &str,
) -> NvdNetworkTrace {
    let enforcement_program_sha256 = "7".repeat(64);
    let enforcement_configuration_sha256 = "8".repeat(64);
    NvdNetworkTrace {
        schema: "radroots.advisory-nvd-network-trace.v1".to_owned(),
        candidate_generation: candidate.generation,
        candidate_digest: candidate.digest.clone(),
        producer_request_sha256: producer_request_sha256.to_owned(),
        enforcement: "deny_by_default_nvd_api_get_only_proxy".to_owned(),
        enforcement_program_sha256,
        enforcement_configuration_sha256,
        request: vec![NvdRequestTrace {
            sequence: 1,
            method: "GET".to_owned(),
            scheme: "https".to_owned(),
            authority: "services.nvd.nist.gov".to_owned(),
            path: "/rest/json/cves/2.0".to_owned(),
            started_at_epoch: 997_001,
            completed_at_epoch: 997_009,
            query: [
                ("lastModStartDate", "1970-01-11T10:00:00Z"),
                ("lastModEndDate", "1970-01-11T10:01:40Z"),
                ("startIndex", "0"),
                ("resultsPerPage", "2000"),
            ]
            .into_iter()
            .map(|(name, value)| NvdQueryTrace {
                name: name.to_owned(),
                value: value.to_owned(),
            })
            .collect(),
            response_status: 200,
            response_byte_length: 128,
            response_sha256: "5".repeat(64),
            response_start_index: 0,
            response_results_per_page: 2_000,
            response_total_results: 1,
        }],
    }
}

fn synthetic_producer_request(candidate: &CandidateIdentity) -> TrustedProducerRequest {
    TrustedProducerRequest {
        schema: "radroots.advisory-producer-request.v1".to_owned(),
        candidate_generation: candidate.generation,
        candidate_digest: candidate.digest.clone(),
        platform: "macos_aarch64".to_owned(),
        process_contract_sha256: BOUNDED_PROCESS_DECISION_SHA256.to_owned(),
        process_runner_source_sha256: BOUNDED_PROCESS_SOURCE_SHA256.to_owned(),
        nvd_endpoint: "/rest/json/cves/2.0".to_owned(),
        nvd_enforcement_program_sha256: "7".repeat(64),
        nvd_enforcement_configuration_sha256: "8".repeat(64),
        nvd_query_keys: [
            "lastModStartDate",
            "lastModEndDate",
            "startIndex",
            "resultsPerPage",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
        workload_ids: WORKLOADS
            .iter()
            .map(|workload| workload.id.to_owned())
            .collect(),
    }
}

#[allow(clippy::too_many_arguments)]
fn synthetic_process_receipt(
    id: &str,
    program_sha256: &str,
    arguments: &[String],
    input_sha256: &str,
    output_sha256: &str,
    path_binding: Vec<ProcessPathBinding>,
    started_at_epoch: u64,
    completed_at_epoch: u64,
    exit_code: i32,
) -> Result<TrustedProcessReceipt, AdvisoryError> {
    let environment = expected_process_environment(id)?;
    let working_directory = expected_process_working_directory(id)?;
    Ok(TrustedProcessReceipt {
        id: id.to_owned(),
        state: OperationState::Complete,
        program_sha256: program_sha256.to_owned(),
        runtime_sha256: expected_runtime_sha256(id),
        arguments_sha256: domain_json_digest(
            b"radroots-advisory-process-arguments-v1\0",
            arguments,
        )?,
        environment_sha256: process_environment_digest(&environment)?,
        environment,
        working_directory_sha256: process_working_directory_digest(&working_directory)?,
        working_directory,
        path_binding,
        stdin_closed: true,
        deadline_seconds: ANALYSIS_DEADLINE_SECONDS,
        started_at_epoch,
        completed_at_epoch,
        exit_code,
        stdout_byte_length: 0,
        stdout_sha256: sha256(&[]),
        stderr_byte_length: 0,
        stderr_sha256: sha256(&[]),
        input_sha256: input_sha256.to_owned(),
        output_sha256: output_sha256.to_owned(),
    })
}

fn attach_gradle_receipts(
    projections: &mut [GradleProjection],
    receipts: &mut Vec<TrustedProcessReceipt>,
    evaluation_epoch: u64,
) -> Result<(), AdvisoryError> {
    for (index, projection) in projections.iter_mut().enumerate() {
        let receipt = synthetic_process_receipt(
            &format!("gradle:{}", projection.workload_id),
            TOOL_PINS[3].executable_sha256,
            &projection.wrapper_arguments,
            &gradle_process_input_digest(projection)?,
            &gradle_process_output_digest(projection)?,
            vec![
                ProcessPathBinding {
                    argument_index: 5,
                    logical_role: "gradle_build_root".to_owned(),
                    identity_sha256: gradle_build_root_binding_digest(projection)?,
                },
                ProcessPathBinding {
                    argument_index: 7,
                    logical_role: "gradle_init_script".to_owned(),
                    identity_sha256: projection.init_script_sha256.clone(),
                },
                ProcessPathBinding {
                    argument_index: 10,
                    logical_role: "gradle_projection_output".to_owned(),
                    identity_sha256: projection.raw_graph_sha256.clone(),
                },
            ],
            evaluation_epoch - 2_800 + index as u64 * 10,
            evaluation_epoch - 2_795 + index as u64 * 10,
            0,
        )?;
        projection.environment_sha256 = receipt.environment_sha256.clone();
        projection.process_receipt_sha256 = process_receipt_digest(&receipt)?;
        receipts.push(receipt);
    }
    Ok(())
}

fn synthetic_gradle_projections(
    admitted_graphs: &[AdmittedGradleGraph],
    inventory: &[WorkloadInventory],
    sources: &[SourceIdentity],
) -> Result<Vec<GradleProjection>, AdvisoryError> {
    let source = sources
        .iter()
        .find(|source| source.repository == "oss/harvestcircle")
        .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch))?;
    WORKLOADS
        .iter()
        .enumerate()
        .filter(|(_, workload)| workload.package_manager == "gradle")
        .zip(admitted_graphs)
        .map(|((index, workload), admitted)| {
            let inventory = inventory
                .iter()
                .find(|row| row.id == workload.id)
                .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch))?;
            admitted.revalidate()?;
            Ok(GradleProjection {
                workload_id: workload.id.to_owned(),
                state: OperationState::Complete,
                raw_graph_byte_length: admitted.raw.byte_length,
                raw_graph_sha256: admitted.raw.sha256.clone(),
                init_script_sha256: GRADLE_INIT_SCRIPT_SHA256.to_owned(),
                wrapper_arguments: expected_gradle_arguments(workload)?,
                environment_keys: [
                    "GRADLE_USER_HOME",
                    "HOME",
                    "JAVA_HOME",
                    "LC_ALL",
                    "PATH",
                    "TMPDIR",
                    "TZ",
                ]
                .into_iter()
                .map(str::to_owned)
                .collect(),
                environment_sha256: format!("{:064x}", index + 4_000),
                exit_code: 0,
                source_revision: source.revision.clone(),
                source_tree: source.tree.clone(),
                input_sha256: inventory.input_sha256.clone(),
                dependency_count: inventory.dependency_count,
                component_count: admitted.components.len() as u64,
                edge_count: admitted.edges.len() as u64,
                artifact_count: admitted.artifacts.len() as u64,
                components: admitted.components.clone(),
                edges: admitted.edges.clone(),
                artifacts: admitted.artifacts.clone(),
                canonical_graph_sha256: admitted.canonical_graph_sha256.clone(),
                materialized_tree_sha256: admitted.materialized_tree_sha256.clone(),
                normalization_receipt_sha256: admitted.normalization_receipt_sha256.clone(),
                artifact_source_roots_sha256: admitted.artifact_source_roots_sha256.clone(),
                seed_cache_inventory_sha256: format!("{:064x}", index + 6_000),
                wrapper_distribution_sha256:
                    "553c78f50dafcd54d65b9a444649057857469edf836431389695608536d6b746".to_owned(),
                process_receipt_sha256: format!("{:064x}", index + 7_000),
            })
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn synthetic_report(
    provider: ProviderId,
    inventory: &[WorkloadInventory],
    gradle_projections: &[GradleProjection],
    archive_sha256: &str,
    materialized_tree_sha256: &str,
    tool_observation_sha256: &str,
    receipts: &mut Vec<TrustedProcessReceipt>,
    evaluation_epoch: u64,
) -> Result<Vec<u8>, AdvisoryError> {
    let mut workload_result = Vec::new();
    for workload in WORKLOADS.iter().filter(|workload| match provider {
        ProviderId::Rustsec => workload.package_manager == "cargo",
        ProviderId::OwaspNvd => workload.package_manager == "gradle",
    }) {
        let inventory = inventory
            .iter()
            .find(|row| row.id == workload.id)
            .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch))?;
        let raw = match provider {
            ProviderId::Rustsec => serde_json::json!({
                "database": {
                    "advisory-count": 1,
                    "last-commit": "6".repeat(40),
                    "last-updated": format_report_epoch(evaluation_epoch - 2_900)?
                },
                "lockfile": {"dependency-count": inventory.dependency_count},
                "settings": {
                    "ignore": [],
                    "informational_warnings": ["unmaintained", "unsound", "notice"],
                    "severity": null,
                    "target_arch": [],
                    "target_os": []
                },
                "vulnerabilities": {"count": 0, "found": false, "list": []},
                "warnings": {}
            }),
            ProviderId::OwaspNvd => {
                let projection = gradle_projections
                    .iter()
                    .find(|projection| projection.workload_id == workload.id)
                    .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch))?;
                let dependencies = projection
                    .artifacts
                    .iter()
                    .map(|artifact| {
                        serde_json::json!({
                            "evidenceCollected": {
                                "productEvidence": [],
                                "vendorEvidence": [],
                                "versionEvidence": []
                            },
                            "fileName": artifact.materialized_name,
                            "filePath": format!(
                                "/private/advisory/scan/{}/{}",
                                workload.id,
                                artifact.materialized_name,
                            ),
                            "isVirtual": false,
                            "md5": "0".repeat(32),
                            "packages": [{
                                "id": format!(
                                    "pkg:maven/{}/{}@{}",
                                    artifact.package_namespace,
                                    artifact.package_name,
                                    artifact.package_version,
                                )
                            }],
                            "sha1": "0".repeat(40),
                            "sha256": artifact.artifact_sha256
                        })
                    })
                    .collect::<Vec<_>>();
                serde_json::json!({
                    "dependencies": dependencies,
                    "projectInfo": {
                        "credits": {
                            "CISA": "This report may contain data retrieved from the CISA Known Exploited Vulnerability Catalog: https://www.cisa.gov/known-exploited-vulnerabilities-catalog",
                            "NPM": "This report may contain data retrieved from the Github Advisory Database (via NPM Audit API): https://github.com/advisories/",
                            "NVD": "This product uses the NVD API but is not endorsed or certified by the NVD. This report contains data retrieved from the National Vulnerability Database: https://nvd.nist.gov",
                            "OSSINDEX": "This report may contain data retrieved from the Sonatype Guide OSS Index API: https://www.sonatype.com/products/sonatype-guide/oss-index-users",
                            "RETIREJS": "This report may contain data retrieved from the RetireJS community: https://retirejs.github.io/retire.js/"
                        },
                        "name": workload.id,
                        "reportDate": format_report_epoch(
                            evaluation_epoch - 2_300 + workload_result.len() as u64 * 10,
                        )?
                    },
                    "reportSchema": "1.1",
                    "scanInfo": {
                        "dataSource": [{
                            "name": "NVD CVE Checked",
                            "timestamp": format_report_epoch(evaluation_epoch - 2_900)?
                        }],
                        "engineVersion": "12.2.2"
                    }
                })
            }
        };
        let raw_scanner_output = serde_json::to_string(&raw)
            .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
        let arguments = synthetic_scanner_arguments(provider, workload.id);
        let prefix = match provider {
            ProviderId::Rustsec => "cargo_audit",
            ProviderId::OwaspNvd => "owasp_analysis",
        };
        let program_sha256 = match provider {
            ProviderId::Rustsec => TOOL_PINS[0].executable_sha256,
            ProviderId::OwaspNvd => TOOL_PINS[1].executable_sha256,
        };
        let exit_code = 0;
        let database_copy = (provider == ProviderId::OwaspNvd).then(|| ScannerDatabaseCopy {
            root_identity_sha256: format!("{:064x}", workload_result.len() + 30_000),
            source_tree_sha256: materialized_tree_sha256.to_owned(),
            pre_scan_tree_sha256: materialized_tree_sha256.to_owned(),
            post_scan_tree_sha256: materialized_tree_sha256.to_owned(),
        });
        let path_binding = match provider {
            ProviderId::Rustsec => vec![
                ProcessPathBinding {
                    argument_index: 2,
                    logical_role: "rustsec_database".to_owned(),
                    identity_sha256: materialized_tree_sha256.to_owned(),
                },
                ProcessPathBinding {
                    argument_index: 6,
                    logical_role: "cargo_lock".to_owned(),
                    identity_sha256: inventory.input_sha256.clone(),
                },
            ],
            ProviderId::OwaspNvd => {
                let projection = gradle_projections
                    .iter()
                    .find(|projection| projection.workload_id == workload.id)
                    .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::InventoryMismatch))?;
                vec![
                    ProcessPathBinding {
                        argument_index: 2,
                        logical_role: "nvd_database".to_owned(),
                        identity_sha256: scanner_database_copy_digest(
                            database_copy.as_ref().ok_or_else(|| {
                                AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot)
                            })?,
                        )?,
                    },
                    ProcessPathBinding {
                        argument_index: 4,
                        logical_role: "gradle_scan_projection".to_owned(),
                        identity_sha256: projection.materialized_tree_sha256.clone(),
                    },
                    ProcessPathBinding {
                        argument_index: 10,
                        logical_role: "raw_report_output".to_owned(),
                        identity_sha256: sha256(raw_scanner_output.as_bytes()),
                    },
                ]
            }
        };
        let timing_offset = match provider {
            ProviderId::Rustsec => 2_600,
            ProviderId::OwaspNvd => 2_300,
        };
        let started_at_epoch = evaluation_epoch - timing_offset + workload_result.len() as u64 * 10;
        let mut receipt = synthetic_process_receipt(
            &format!("{prefix}:{}", workload.id),
            program_sha256,
            &arguments,
            &scanner_process_input_digest_fields(
                provider,
                workload.id,
                &inventory.input_sha256,
                inventory.dependency_count,
                archive_sha256,
                materialized_tree_sha256,
                &database_copy
                    .as_ref()
                    .map(scanner_database_copy_digest)
                    .transpose()?
                    .unwrap_or_else(|| "none".to_owned()),
                tool_observation_sha256,
            )?,
            &sha256(raw_scanner_output.as_bytes()),
            path_binding,
            started_at_epoch,
            started_at_epoch + 5,
            exit_code,
        )?;
        if provider == ProviderId::Rustsec {
            receipt.stdout_byte_length = raw_scanner_output.len() as u64;
            receipt.stdout_sha256 = sha256(raw_scanner_output.as_bytes());
        }
        let process_receipt_sha256 = process_receipt_digest(&receipt)?;
        let environment_sha256 = receipt.environment_sha256.clone();
        receipts.push(receipt);
        workload_result.push(WorkloadResultFixture {
            workload_id: workload.id.to_owned(),
            input_sha256: inventory.input_sha256.clone(),
            dependency_count: inventory.dependency_count,
            provider_archive_sha256: archive_sha256.to_owned(),
            materialized_tree_sha256: materialized_tree_sha256.to_owned(),
            tool_observation_sha256: tool_observation_sha256.to_owned(),
            arguments,
            environment_sha256,
            process_receipt_sha256,
            database_copy,
            exit_code,
            raw_output_byte_length: raw_scanner_output.len() as u64,
            raw_output_sha256: sha256(raw_scanner_output.as_bytes()),
            raw_scanner_output,
        });
    }
    let value = serde_json::json!({
        "provider": provider,
        "schema": "radroots.advisory-unsuppressed-report.v1",
        "workload_result": workload_result
    });
    serde_json::from_value::<ReportEnvelope>(value.clone())
        .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
    Ok(canonical_json(&value))
}

#[derive(Serialize)]
struct WorkloadResultFixture {
    workload_id: String,
    input_sha256: String,
    dependency_count: u64,
    provider_archive_sha256: String,
    materialized_tree_sha256: String,
    tool_observation_sha256: String,
    arguments: Vec<String>,
    environment_sha256: String,
    process_receipt_sha256: String,
    database_copy: Option<ScannerDatabaseCopy>,
    exit_code: i32,
    raw_output_byte_length: u64,
    raw_output_sha256: String,
    raw_scanner_output: String,
}

fn synthetic_scanner_arguments(provider: ProviderId, workload_id: &str) -> Vec<String> {
    match provider {
        ProviderId::Rustsec => [
            "audit".to_owned(),
            "--db".to_owned(),
            "/private/advisory/rustsec-db".to_owned(),
            "--no-fetch".to_owned(),
            "--json".to_owned(),
            "--file".to_owned(),
            format!("/private/advisory/locks/{workload_id}/Cargo.lock"),
        ]
        .into_iter()
        .collect(),
        ProviderId::OwaspNvd => {
            let mut arguments = strings(&OWASP_OFFLINE_ARGUMENTS);
            arguments[2] = "/private/advisory/nvd-data".to_owned();
            arguments[4] = format!("/private/advisory/scan/{workload_id}");
            arguments[6] = workload_id.to_owned();
            arguments[10] = format!("/private/advisory/report/{workload_id}");
            arguments
        }
    }
}

fn set_raw_output(result: &mut Value, raw: &Value, exit_code: i32) -> Result<(), AdvisoryError> {
    let raw = serde_json::to_string(raw)
        .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::InvalidReport))?;
    result["exit_code"] = Value::from(exit_code);
    result["raw_output_byte_length"] = Value::from(raw.len() as u64);
    result["raw_output_sha256"] = Value::from(sha256(raw.as_bytes()));
    result["raw_scanner_output"] = Value::from(raw);
    Ok(())
}

fn synthetic_archive_tree_digest(
    archive: &[u8],
    materialization_parent: &Path,
) -> Result<(String, ArchiveEvidence), AdvisoryError> {
    let source = tempfile::tempdir()
        .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot))?;
    std::fs::write(source.path().join("snapshot.tar.gz"), archive)
        .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot))?;
    let snapshot = safe_artifact_io::traverse_regular_files(
        source.path(),
        TraversalLimits {
            max_entries: 1,
            max_files: 1,
            max_total_bytes: archive.len() as u64,
            max_file_bytes: archive.len() as u64,
            max_depth: 1,
            max_path_bytes: 64,
        },
        &[],
    )
    .map_err(map_archive_error)?;
    let file = snapshot
        .files()
        .first()
        .ok_or_else(|| AdvisoryError::new(AdvisoryFailureKind::ArchiveRejected))?;
    let materialized = snapshot
        .materialize_deterministic_tar_gzip(file, materialization_parent, archive_limits())
        .map_err(map_archive_error)?;
    let digest = materialized_tree_digest(materialized.snapshot())?;
    let evidence = materialized.evidence().clone();
    materialized.revalidate().map_err(map_binding_error)?;
    snapshot.revalidate().map_err(map_binding_error)?;
    Ok((digest, evidence))
}

fn deterministic_archive(payload: &[u8]) -> Result<Vec<u8>, AdvisoryError> {
    use flate2::{Compression, GzBuilder};
    use tar::{Builder, EntryType, Header};

    let encoder = GzBuilder::new()
        .mtime(0)
        .operating_system(255)
        .write(Vec::new(), Compression::new(6));
    let mut builder = Builder::new(encoder);
    let mut directory = Header::new_gnu();
    directory
        .set_path("snapshot/")
        .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot))?;
    directory.set_entry_type(EntryType::Directory);
    directory.set_mode(0o755);
    directory.set_uid(0);
    directory.set_gid(0);
    directory.set_mtime(0);
    directory.set_size(0);
    directory.set_cksum();
    builder
        .append(&directory, &[][..])
        .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot))?;
    let mut header = Header::new_gnu();
    header
        .set_path("snapshot/data.json")
        .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot))?;
    header.set_entry_type(EntryType::Regular);
    header.set_mode(0o644);
    header.set_uid(0);
    header.set_gid(0);
    header.set_mtime(0);
    header.set_size(payload.len() as u64);
    header.set_cksum();
    builder
        .append(&header, payload)
        .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot))?;
    let encoder = builder
        .into_inner()
        .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot))?;
    encoder
        .finish()
        .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::InvalidSnapshot))
}

fn trusted_tempdir(prefix: &str) -> Result<tempfile::TempDir, AdvisoryError> {
    let current = std::env::current_dir()
        .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::Unavailable))?;
    tempfile::Builder::new()
        .prefix(prefix)
        .tempdir_in(current)
        .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::Unavailable))
}

fn set_private_directory_mode(path: &Path) -> Result<(), AdvisoryError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
            .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::Unavailable))
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Err(AdvisoryError::new(AdvisoryFailureKind::Unavailable))
    }
}

fn set_private_file_mode(path: &Path) -> Result<(), AdvisoryError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .map_err(|_| AdvisoryError::new(AdvisoryFailureKind::Unavailable))
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Err(AdvisoryError::new(AdvisoryFailureKind::Unavailable))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_vulnerable_fixture() {
        let fixture = SyntheticFixture::new().expect("construct baseline advisory fixture");
        let provider: TrustedProviderEvidence =
            parse_canonical_authority(&fixture.request.provider_execution_evidence)
                .expect("parse provider evidence");
        validate_process_receipts(&provider.process_receipt, fixture.request.evaluation_epoch)
            .expect("validate process receipts");
        validate_trusted_authority(&fixture.request).expect("validate trusted authority");
        validate_request(&fixture.request).expect("validate baseline request");
        validate_manifest(&fixture.manifest, &fixture.request).expect("validate baseline manifest");
        admit_snapshot(
            fixture.root(),
            fixture.materialization_parent.path(),
            &fixture.request,
        )
        .expect("admit baseline advisory fixture");

        let mut fixture = SyntheticFixture::new().expect("construct vulnerable fixture");
        fixture
            .set_rustsec_finding(NON_WAIVABLE_RUSTSEC, "lru", "0.16.4")
            .expect("install known vulnerability");
        let error = admit_snapshot(
            fixture.root(),
            fixture.materialization_parent.path(),
            &fixture.request,
        )
        .expect_err("known vulnerability must fail closed");
        assert_eq!(error.kind(), AdvisoryFailureKind::KnownVulnerability);
    }

    #[test]
    fn missing_inventory_is_rejected() {
        missing_inventory_vector().expect("missing inventory must fail closed");
    }

    #[test]
    fn unavailable_provider_is_nonpass() {
        unavailable_provider_vector().expect("unavailable provider must be non-pass");
    }

    #[test]
    fn stale_snapshot_is_rejected() {
        stale_snapshot_vector().expect("stale snapshot must fail closed");
    }

    #[test]
    fn timed_out_operation_is_nonpass() {
        timed_out_operation_vector().expect("timed-out operation must be non-pass");
    }

    #[test]
    fn expired_suppression_is_rejected() {
        expired_suppression_vector().expect("expired suppression must fail closed");
    }
}
