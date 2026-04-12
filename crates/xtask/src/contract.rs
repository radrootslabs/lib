#![forbid(unsafe_code)]

use crate::coverage::{CoveragePolicyFile, CoverageThresholds, read_coverage_policy};
use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const ROOT_RELEASE_POLICY_RELATIVE: &str =
    "contracts/release/mounted_rust_crates/publish-policy.toml";
const CONFORMANCE_ROOT_RELATIVE: &str = "spec/conformance";
const CONFORMANCE_SCHEMA_RELATIVE: &str = "spec/conformance/schema/vector.schema.json";
const RELEASE_POLICY_ENV: &str = "RADROOTS_MOUNTED_RUST_CRATE_PUBLISH_POLICY";
const EVENT_BOUNDARY_MATRIX_ENV: &str = "RADROOTS_EVENT_BOUNDARY_MATRIX";
const EVENT_BOUNDARY_MATRIX_RELATIVES: [&str; 2] = [
    "dev/docs/radroots/radrootsd/spec-coverage.md",
    "docs/radroots/radrootsd/spec-coverage.md",
];

#[derive(Debug, Deserialize)]
pub struct ContractManifest {
    pub contract: ManifestContract,
    pub surface: Surface,
    pub policy: Policy,
    pub export: Option<ManifestExports>,
}

#[derive(Debug, Deserialize)]
pub struct ManifestContract {
    pub name: String,
    pub version: String,
    pub source: String,
}

#[derive(Debug, Deserialize)]
pub struct Surface {
    pub model_crates: Vec<String>,
    pub algorithm_crates: Vec<String>,
    pub wasm_crates: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct Policy {
    pub exclude_internal_workspace_crates: bool,
    pub require_reproducible_exports: bool,
    pub require_conformance_vectors: bool,
}

#[derive(Debug, Deserialize)]
pub struct ManifestExports {
    pub ts: Option<ManifestLanguagePackages>,
}

#[derive(Debug, Deserialize)]
pub struct ManifestLanguagePackages {
    pub packages: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct OperationsContractManifest {
    pub contract: ManifestContract,
    pub public: PublicContract,
    pub shared_types: SharedTypesContract,
    pub errors: ErrorClassesContract,
    pub operations: BTreeMap<String, PublicOperationContract>,
    pub implementation_provenance: Option<ImplementationProvenance>,
}

#[derive(Debug, Deserialize)]
pub struct PublicContract {
    pub domains: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct SharedTypesContract {
    pub public: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct ErrorClassesContract {
    pub classes: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct ImplementationProvenance {
    pub model_crates: Vec<String>,
    pub algorithm_crates: Vec<String>,
    pub wasm_crates: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct PublicOperationContract {
    pub domain: String,
    pub id: String,
    pub stability: String,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub error_class: String,
    #[allow(dead_code)]
    pub deterministic: bool,
    pub signing: String,
    pub transport: String,
    pub implementation: PublicOperationImplementation,
    pub conformance: PublicOperationConformance,
}

#[derive(Debug, Deserialize)]
pub struct PublicOperationImplementation {
    pub rust_modules: Vec<String>,
    pub rust_types: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct PublicOperationConformance {
    pub vector: String,
}

#[derive(Debug, Deserialize)]
pub struct VersionPolicy {
    pub contract: VersionContract,
    pub semver: SemverRules,
    pub compatibility: CompatibilityRules,
}

#[derive(Debug, Deserialize)]
pub struct VersionContract {
    pub version: String,
    pub stability: String,
}

#[derive(Debug, Deserialize)]
pub struct SemverRules {
    pub major_on: Vec<String>,
    pub minor_on: Vec<String>,
    pub patch_on: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct CompatibilityRules {
    pub requires_conformance_pass: bool,
    pub requires_export_manifest_diff: bool,
    pub requires_release_notes: bool,
}

#[derive(Debug, Deserialize)]
pub struct ExportMapping {
    pub language: ExportLanguage,
    pub packages: BTreeMap<String, String>,
    pub artifacts: Option<ExportArtifacts>,
}

#[derive(Debug, Deserialize)]
pub struct ExportLanguage {
    pub id: String,
    pub repository: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct ExportArtifacts {
    pub models_dir: Option<String>,
    pub constants_dir: Option<String>,
    pub wasm_dist_dir: Option<String>,
    pub manifest_file: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SdkExportMapping {
    pub language: ExportLanguage,
    pub sdk: SdkExportSdk,
    pub operations: BTreeMap<String, String>,
    pub shared_types: BTreeMap<String, String>,
    pub artifacts: Option<SdkExportArtifacts>,
}

#[derive(Debug, Deserialize)]
pub struct SdkExportSdk {
    pub package: String,
    pub module_format: Option<String>,
    pub deterministic_codec: String,
    pub signing: String,
    pub networking: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct SdkExportArtifacts {
    pub models_dir: Option<String>,
    pub runtime_dir: Option<String>,
    pub wasm_dist_dir: Option<String>,
    pub manifest_file: Option<String>,
}

#[derive(Debug)]
pub struct ContractBundle {
    pub root: PathBuf,
    pub manifest: ContractManifest,
    pub version: VersionPolicy,
    pub exports: Vec<ExportMapping>,
    pub operations_manifest: Option<OperationsContractManifest>,
    pub sdk_exports: Vec<SdkExportMapping>,
}

#[derive(Debug, Deserialize)]
struct WorkspaceCargoManifest {
    workspace: WorkspaceSection,
}

#[derive(Debug, Deserialize)]
struct WorkspaceSection {
    members: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct PackageCargoManifest {
    package: PackageSection,
}

#[derive(Debug, Deserialize)]
struct PackageSection {
    name: String,
    publish: Option<PackagePublish>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
enum PackagePublish {
    Bool(bool),
    Registries(Vec<String>),
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Deserialize)]
struct CoverageRequiredFile {
    required: CoverageRequiredSection,
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Deserialize)]
struct CoverageRequiredSection {
    crates: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EventBoundaryRow {
    domain: String,
    kind: String,
    radroots_type: String,
    rpc_methods: BTreeSet<String>,
}

#[derive(Clone, Copy)]
struct EventBoundarySourceWitness {
    relative_path: &'static str,
    required_fragments: &'static [&'static str],
}

#[derive(Clone, Copy)]
struct EventBoundaryExpectation {
    domain: &'static str,
    kind: &'static str,
    radroots_type: &'static str,
    rpc_methods: &'static [&'static str],
    witnesses: &'static [EventBoundarySourceWitness],
}

const PROFILE_WITNESSES: [EventBoundarySourceWitness; 2] = [
    EventBoundarySourceWitness {
        relative_path: "crates/events/src/profile.rs",
        required_fragments: &["pub struct RadrootsProfile"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/events/src/kinds.rs",
        required_fragments: &["pub const KIND_PROFILE: u32 = 0;"],
    },
];

const FOLLOW_WITNESSES: [EventBoundarySourceWitness; 2] = [
    EventBoundarySourceWitness {
        relative_path: "crates/events/src/follow.rs",
        required_fragments: &["pub struct RadrootsFollow"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/events/src/kinds.rs",
        required_fragments: &["pub const KIND_FOLLOW: u32 = 3;"],
    },
];

const POST_WITNESSES: [EventBoundarySourceWitness; 2] = [
    EventBoundarySourceWitness {
        relative_path: "crates/events/src/post.rs",
        required_fragments: &["pub struct RadrootsPost"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/events/src/kinds.rs",
        required_fragments: &["pub const KIND_POST: u32 = 1;"],
    },
];

const COMMENT_WITNESSES: [EventBoundarySourceWitness; 2] = [
    EventBoundarySourceWitness {
        relative_path: "crates/events/src/comment.rs",
        required_fragments: &["pub struct RadrootsComment"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/events/src/kinds.rs",
        required_fragments: &["pub const KIND_COMMENT: u32 = 1111;"],
    },
];

const REACTION_WITNESSES: [EventBoundarySourceWitness; 2] = [
    EventBoundarySourceWitness {
        relative_path: "crates/events/src/reaction.rs",
        required_fragments: &["pub struct RadrootsReaction"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/events/src/kinds.rs",
        required_fragments: &["pub const KIND_REACTION: u32 = 7;"],
    },
];

const SEAL_WITNESSES: [EventBoundarySourceWitness; 2] = [
    EventBoundarySourceWitness {
        relative_path: "crates/events/src/seal.rs",
        required_fragments: &["pub struct RadrootsSeal"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/events/src/kinds.rs",
        required_fragments: &["pub const KIND_SEAL: u32 = 13;"],
    },
];

const MESSAGE_WITNESSES: [EventBoundarySourceWitness; 4] = [
    EventBoundarySourceWitness {
        relative_path: "crates/events/src/message.rs",
        required_fragments: &["pub struct RadrootsMessage"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/events/src/kinds.rs",
        required_fragments: &["pub const KIND_MESSAGE: u32 = 14;"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/nostr/src/nip17.rs",
        required_fragments: &[
            "pub async fn radroots_nostr_wrap_message<T>(",
            "KIND_MESSAGE =>",
        ],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/nostr/src/lib.rs",
        required_fragments: &["radroots_nostr_wrap_message"],
    },
];

const MESSAGE_FILE_WITNESSES: [EventBoundarySourceWitness; 4] = [
    EventBoundarySourceWitness {
        relative_path: "crates/events/src/message_file.rs",
        required_fragments: &["pub struct RadrootsMessageFile"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/events/src/kinds.rs",
        required_fragments: &["pub const KIND_MESSAGE_FILE: u32 = 15;"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/nostr/src/nip17.rs",
        required_fragments: &[
            "pub async fn radroots_nostr_wrap_message_file<T>(",
            "KIND_MESSAGE_FILE =>",
        ],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/nostr/src/lib.rs",
        required_fragments: &["radroots_nostr_wrap_message_file"],
    },
];

const GIFT_WRAP_WITNESSES: [EventBoundarySourceWitness; 4] = [
    EventBoundarySourceWitness {
        relative_path: "crates/events/src/gift_wrap.rs",
        required_fragments: &["pub struct RadrootsGiftWrap"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/events/src/kinds.rs",
        required_fragments: &["pub const KIND_GIFT_WRAP: u32 = 1059;"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/nostr/src/nip17.rs",
        required_fragments: &["pub async fn radroots_nostr_unwrap_gift_wrap<T>("],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/nostr/src/lib.rs",
        required_fragments: &["radroots_nostr_unwrap_gift_wrap"],
    },
];

const LIST_WITNESSES: [EventBoundarySourceWitness; 2] = [
    EventBoundarySourceWitness {
        relative_path: "crates/events/src/list.rs",
        required_fragments: &["pub struct RadrootsList"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/events/src/kinds.rs",
        required_fragments: &[
            "pub const KIND_LIST_MUTE: u32 = 10000;",
            "pub const KIND_LIST_GOOD_WIKI_RELAYS: u32 = 10102;",
        ],
    },
];

const LIST_SET_WITNESSES: [EventBoundarySourceWitness; 2] = [
    EventBoundarySourceWitness {
        relative_path: "crates/events/src/list_set.rs",
        required_fragments: &["pub struct RadrootsListSet"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/events/src/kinds.rs",
        required_fragments: &[
            "pub const KIND_LIST_SET_FOLLOW: u32 = 30000;",
            "pub const KIND_LIST_SET_MEDIA_STARTER_PACK: u32 = 39092;",
        ],
    },
];

const APP_DATA_WITNESSES: [EventBoundarySourceWitness; 2] = [
    EventBoundarySourceWitness {
        relative_path: "crates/events/src/app_data.rs",
        required_fragments: &["pub struct RadrootsAppData"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/events/src/kinds.rs",
        required_fragments: &["pub const KIND_APP_DATA: u32 = 30078;"],
    },
];

const APP_HANDLER_WITNESSES: [EventBoundarySourceWitness; 2] = [
    EventBoundarySourceWitness {
        relative_path: "crates/events/src/kinds.rs",
        required_fragments: &["pub const KIND_APPLICATION_HANDLER: u32 = 31990;"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/nostr/src/events/application_handler.rs",
        required_fragments: &["pub fn radroots_nostr_build_application_handler_event("],
    },
];

const FARM_WITNESSES: [EventBoundarySourceWitness; 2] = [
    EventBoundarySourceWitness {
        relative_path: "crates/events/src/farm.rs",
        required_fragments: &["pub struct RadrootsFarm"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/events/src/kinds.rs",
        required_fragments: &["pub const KIND_FARM: u32 = 30340;"],
    },
];

const PLOT_WITNESSES: [EventBoundarySourceWitness; 2] = [
    EventBoundarySourceWitness {
        relative_path: "crates/events/src/plot.rs",
        required_fragments: &["pub struct RadrootsPlot"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/events/src/kinds.rs",
        required_fragments: &["pub const KIND_PLOT: u32 = 30350;"],
    },
];

const COOP_WITNESSES: [EventBoundarySourceWitness; 2] = [
    EventBoundarySourceWitness {
        relative_path: "crates/events/src/coop.rs",
        required_fragments: &["pub struct RadrootsCoop"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/events/src/kinds.rs",
        required_fragments: &["pub const KIND_COOP: u32 = 30360;"],
    },
];

const DOCUMENT_WITNESSES: [EventBoundarySourceWitness; 2] = [
    EventBoundarySourceWitness {
        relative_path: "crates/events/src/document.rs",
        required_fragments: &["pub struct RadrootsDocument"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/events/src/kinds.rs",
        required_fragments: &["pub const KIND_DOCUMENT: u32 = 30361;"],
    },
];

const RESOURCE_AREA_WITNESSES: [EventBoundarySourceWitness; 2] = [
    EventBoundarySourceWitness {
        relative_path: "crates/events/src/resource_area.rs",
        required_fragments: &["pub struct RadrootsResourceArea"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/events/src/kinds.rs",
        required_fragments: &["pub const KIND_RESOURCE_AREA: u32 = 30370;"],
    },
];

const RESOURCE_CAP_WITNESSES: [EventBoundarySourceWitness; 2] = [
    EventBoundarySourceWitness {
        relative_path: "crates/events/src/resource_cap.rs",
        required_fragments: &["pub struct RadrootsResourceHarvestCap"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/events/src/kinds.rs",
        required_fragments: &["pub const KIND_RESOURCE_HARVEST_CAP: u32 = 30371;"],
    },
];

const LISTING_WITNESSES: [EventBoundarySourceWitness; 2] = [
    EventBoundarySourceWitness {
        relative_path: "crates/events/src/listing.rs",
        required_fragments: &["pub struct RadrootsListing"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/events/src/kinds.rs",
        required_fragments: &["pub const KIND_LISTING: u32 = 30402;"],
    },
];

const LISTING_DRAFT_WITNESSES: [EventBoundarySourceWitness; 2] = [
    EventBoundarySourceWitness {
        relative_path: "crates/events/src/listing.rs",
        required_fragments: &["pub struct RadrootsListing"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/events/src/kinds.rs",
        required_fragments: &["pub const KIND_LISTING_DRAFT: u32 = 30403;"],
    },
];

const DVM_REQUEST_WITNESSES: [EventBoundarySourceWitness; 2] = [
    EventBoundarySourceWitness {
        relative_path: "crates/events/src/job_request.rs",
        required_fragments: &["pub struct RadrootsJobRequest"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/events/src/kinds.rs",
        required_fragments: &[
            "pub const KIND_JOB_REQUEST_MIN: u32 = 5000;",
            "pub const KIND_JOB_REQUEST_MAX: u32 = 5999;",
        ],
    },
];

const DVM_RESULT_WITNESSES: [EventBoundarySourceWitness; 2] = [
    EventBoundarySourceWitness {
        relative_path: "crates/events/src/job_result.rs",
        required_fragments: &["pub struct RadrootsJobResult"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/events/src/kinds.rs",
        required_fragments: &[
            "pub const KIND_JOB_RESULT_MIN: u32 = 6000;",
            "pub const KIND_JOB_RESULT_MAX: u32 = 6999;",
        ],
    },
];

const DVM_FEEDBACK_WITNESSES: [EventBoundarySourceWitness; 2] = [
    EventBoundarySourceWitness {
        relative_path: "crates/events/src/job_feedback.rs",
        required_fragments: &["pub struct RadrootsJobFeedback"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/events/src/kinds.rs",
        required_fragments: &["pub const KIND_JOB_FEEDBACK: u32 = 7000;"],
    },
];

const TRADE_LISTING_WITNESSES: [EventBoundarySourceWitness; 4] = [
    EventBoundarySourceWitness {
        relative_path: "crates/trade/src/listing/dvm.rs",
        required_fragments: &["pub struct TradeListingEnvelope<T>"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/trade/src/listing/kinds.rs",
        required_fragments: &[
            "pub const KIND_TRADE_LISTING_VALIDATE_REQ: u16 = 5321;",
            "pub const KIND_TRADE_LISTING_VALIDATE_RES: u16 = 6321;",
            "pub const KIND_TRADE_LISTING_ORDER_REQ: u16 = 5322;",
            "pub const KIND_TRADE_LISTING_ORDER_RES: u16 = 6322;",
        ],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/events/src/trade.rs",
        required_fragments: &["pub struct RadrootsTradeEnvelope<T>"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/events/src/kinds.rs",
        required_fragments: &[
            "pub const KIND_TRADE_LISTING_VALIDATE_REQ: u32 = 5321;",
            "pub const KIND_TRADE_LISTING_ORDER_REQ: u32 = KIND_TRADE_ORDER_REQUEST;",
        ],
    },
];

const RELAY_DOC_WITNESSES: [EventBoundarySourceWitness; 2] = [
    EventBoundarySourceWitness {
        relative_path: "crates/events/src/relay_document.rs",
        required_fragments: &["pub struct RadrootsRelayDocument"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/nostr/src/nip11.rs",
        required_fragments: &[
            "pub async fn fetch_nip11(ws_url: &str) -> Option<RadrootsRelayDocument>",
        ],
    },
];

const CANONICAL_EVENT_BOUNDARY_EXPECTATIONS: [EventBoundaryExpectation; 26] = [
    EventBoundaryExpectation {
        domain: "profile",
        kind: "0",
        radroots_type: "RadrootsProfile",
        rpc_methods: &[
            "events.profile.publish",
            "events.profile.list",
            "events.profile.get",
        ],
        witnesses: &PROFILE_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "follow",
        kind: "3",
        radroots_type: "RadrootsFollow",
        rpc_methods: &[
            "events.follow.publish",
            "events.follow.list",
            "events.follow.get",
        ],
        witnesses: &FOLLOW_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "post",
        kind: "1",
        radroots_type: "RadrootsPost",
        rpc_methods: &["events.post.publish", "events.post.list", "events.post.get"],
        witnesses: &POST_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "comment",
        kind: "1111",
        radroots_type: "RadrootsComment",
        rpc_methods: &[
            "events.comment.publish",
            "events.comment.list",
            "events.comment.get",
        ],
        witnesses: &COMMENT_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "reaction",
        kind: "7",
        radroots_type: "RadrootsReaction",
        rpc_methods: &[
            "events.reaction.publish",
            "events.reaction.list",
            "events.reaction.get",
        ],
        witnesses: &REACTION_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "seal",
        kind: "13",
        radroots_type: "RadrootsSeal",
        rpc_methods: &["events.seal.encode", "events.seal.decode"],
        witnesses: &SEAL_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "message",
        kind: "14",
        radroots_type: "RadrootsMessage",
        rpc_methods: &[
            "events.message.publish",
            "events.message.list",
            "events.message.get",
        ],
        witnesses: &MESSAGE_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "message_file",
        kind: "15",
        radroots_type: "RadrootsMessageFile",
        rpc_methods: &[
            "events.message_file.publish",
            "events.message_file.list",
            "events.message_file.get",
        ],
        witnesses: &MESSAGE_FILE_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "gift_wrap",
        kind: "1059",
        radroots_type: "RadrootsGiftWrap",
        rpc_methods: &[
            "events.gift_wrap.publish",
            "events.gift_wrap.list",
            "events.gift_wrap.get",
        ],
        witnesses: &GIFT_WRAP_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "list",
        kind: "10000..10102",
        radroots_type: "RadrootsList",
        rpc_methods: &["events.list.publish", "events.list.list", "events.list.get"],
        witnesses: &LIST_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "list_set",
        kind: "30000..39092",
        radroots_type: "RadrootsListSet",
        rpc_methods: &[
            "events.list_set.publish",
            "events.list_set.list",
            "events.list_set.get",
        ],
        witnesses: &LIST_SET_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "app_data",
        kind: "30078",
        radroots_type: "RadrootsAppData",
        rpc_methods: &[
            "events.app_data.publish",
            "events.app_data.list",
            "events.app_data.get",
        ],
        witnesses: &APP_DATA_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "app_handler",
        kind: "31990",
        radroots_type: "KIND_APPLICATION_HANDLER",
        rpc_methods: &[
            "events.app_handler.publish",
            "events.app_handler.list",
            "events.app_handler.get",
        ],
        witnesses: &APP_HANDLER_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "farm",
        kind: "30340",
        radroots_type: "RadrootsFarm",
        rpc_methods: &["events.farm.publish", "events.farm.list", "events.farm.get"],
        witnesses: &FARM_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "plot",
        kind: "30350",
        radroots_type: "RadrootsPlot",
        rpc_methods: &["events.plot.publish", "events.plot.list", "events.plot.get"],
        witnesses: &PLOT_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "coop",
        kind: "30360",
        radroots_type: "RadrootsCoop",
        rpc_methods: &["events.coop.publish", "events.coop.list", "events.coop.get"],
        witnesses: &COOP_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "document",
        kind: "30361",
        radroots_type: "RadrootsDocument",
        rpc_methods: &[
            "events.document.publish",
            "events.document.list",
            "events.document.get",
        ],
        witnesses: &DOCUMENT_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "resource_area",
        kind: "30370",
        radroots_type: "RadrootsResourceArea",
        rpc_methods: &[
            "events.resource_area.publish",
            "events.resource_area.list",
            "events.resource_area.get",
        ],
        witnesses: &RESOURCE_AREA_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "resource_cap",
        kind: "30371",
        radroots_type: "RadrootsResourceHarvestCap",
        rpc_methods: &[
            "events.resource_cap.publish",
            "events.resource_cap.list",
            "events.resource_cap.get",
        ],
        witnesses: &RESOURCE_CAP_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "listing",
        kind: "30402",
        radroots_type: "RadrootsListing",
        rpc_methods: &[
            "events.listing.publish",
            "events.listing.list",
            "events.listing.get",
        ],
        witnesses: &LISTING_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "listing_draft",
        kind: "30403",
        radroots_type: "RadrootsListing",
        rpc_methods: &[
            "events.listing_draft.publish",
            "events.listing_draft.list",
            "events.listing_draft.get",
        ],
        witnesses: &LISTING_DRAFT_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "dvm_request",
        kind: "5000-5999",
        radroots_type: "RadrootsJobRequest",
        rpc_methods: &[
            "events.dvm_request.publish",
            "events.dvm_request.list",
            "events.dvm_request.get",
        ],
        witnesses: &DVM_REQUEST_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "dvm_result",
        kind: "6000-6999",
        radroots_type: "RadrootsJobResult",
        rpc_methods: &[
            "events.dvm_result.publish",
            "events.dvm_result.list",
            "events.dvm_result.get",
        ],
        witnesses: &DVM_RESULT_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "dvm_feedback",
        kind: "7000",
        radroots_type: "RadrootsJobFeedback",
        rpc_methods: &[
            "events.dvm_feedback.publish",
            "events.dvm_feedback.list",
            "events.dvm_feedback.get",
        ],
        witnesses: &DVM_FEEDBACK_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "trade:listing",
        kind: "5321/6321/5322/6322",
        radroots_type: "TradeListingEnvelope",
        rpc_methods: &[
            "domains.trade.listing.validate",
            "domains.trade.listing.order",
            "domains.trade.listing.series.get",
            "domains.trade.listing.dvm.list",
        ],
        witnesses: &TRADE_LISTING_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "relay_doc",
        kind: "N/A",
        radroots_type: "RadrootsRelayDocument",
        rpc_methods: &["system.relay_doc.get"],
        witnesses: &RELAY_DOC_WITNESSES,
    },
];

#[derive(Debug, Deserialize)]
struct ReleaseContractFile {
    release: ReleaseSection,
    #[serde(default)]
    classification: ReleaseClassification,
    #[serde(default)]
    publish: Option<ReleaseCrateSet>,
    #[serde(default)]
    internal: Option<ReleaseCrateSet>,
    publish_order: ReleaseCrateSet,
}

#[derive(Debug, Default, Deserialize)]
struct ReleaseClassification {
    #[serde(default)]
    public: Vec<String>,
    #[serde(default)]
    internal: Vec<String>,
    #[serde(default)]
    deferred: Vec<String>,
    #[serde(default)]
    retired: Vec<String>,
    #[serde(default)]
    yank_only: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ReleaseSection {
    version: String,
}

#[derive(Debug, Deserialize)]
struct ReleaseCrateSet {
    crates: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConformanceVectorFile {
    suite: String,
    contract_version: String,
    vectors: Vec<ConformanceVectorEntry>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConformanceVectorEntry {
    id: String,
    kind: String,
    input: Value,
    expected: Value,
}

impl ReleaseContractFile {
    fn uses_classification(&self) -> bool {
        !self.classification.public.is_empty()
            || !self.classification.internal.is_empty()
            || !self.classification.deferred.is_empty()
            || !self.classification.retired.is_empty()
            || !self.classification.yank_only.is_empty()
    }

    fn public_crates(&self) -> Vec<String> {
        if self.uses_classification() {
            return self.classification.public.clone();
        }
        self.publish
            .as_ref()
            .map(|set| set.crates.clone())
            .unwrap_or_default()
    }

    fn internal_crates(&self) -> Vec<String> {
        if self.uses_classification() {
            return self.classification.internal.clone();
        }
        self.internal
            .as_ref()
            .map(|set| set.crates.clone())
            .unwrap_or_default()
    }

    fn deferred_crates(&self) -> Vec<String> {
        self.classification.deferred.clone()
    }

    fn retired_crates(&self) -> Vec<String> {
        self.classification.retired.clone()
    }

    fn yank_only_crates(&self) -> Vec<String> {
        self.classification.yank_only.clone()
    }
}

fn parse_toml<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, String> {
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(e) => return Err(format!("read {}: {e}", path.display())),
    };
    match toml::from_str::<T>(&raw) {
        Ok(parsed) => Ok(parsed),
        Err(e) => Err(format!("parse {}: {e}", path.display())),
    }
}

fn parse_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, String> {
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(e) => return Err(format!("read {}: {e}", path.display())),
    };
    match serde_json::from_str::<T>(&raw) {
        Ok(parsed) => Ok(parsed),
        Err(e) => Err(format!("parse {}: {e}", path.display())),
    }
}

fn resolve_event_boundary_matrix_path_with_override(
    workspace_root: &Path,
    event_boundary_override: Option<PathBuf>,
) -> Result<PathBuf, String> {
    if let Some(path) = event_boundary_override {
        if !path.is_file() {
            return Err(format!(
                "{EVENT_BOUNDARY_MATRIX_ENV} points to a missing canonical event matrix file: {}",
                path.display()
            ));
        }
        return Ok(path);
    }

    for ancestor in workspace_root.ancestors() {
        for relative in EVENT_BOUNDARY_MATRIX_RELATIVES {
            let candidate = ancestor.join(relative);
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
    }

    Err(format!(
        "canonical event matrix not found; set {EVENT_BOUNDARY_MATRIX_ENV} or provide one of: {}",
        EVENT_BOUNDARY_MATRIX_RELATIVES.join(", ")
    ))
}

fn parse_event_boundary_matrix(path: &Path) -> Result<BTreeMap<String, EventBoundaryRow>, String> {
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(e) => return Err(format!("read {}: {e}", path.display())),
    };
    let mut rows = BTreeMap::new();
    let mut in_table = false;
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed == "| Domain | Kind | Radroots Type | RPC Methods | Notes |" {
            in_table = true;
            continue;
        }
        if !in_table {
            continue;
        }
        if trimmed.is_empty() {
            break;
        }
        if trimmed == "| --- | --- | --- | --- | --- |" {
            continue;
        }
        if !trimmed.starts_with('|') {
            break;
        }
        let columns = trimmed
            .trim_matches('|')
            .split('|')
            .map(|part| part.trim())
            .collect::<Vec<_>>();
        if columns.len() != 5 {
            return Err(format!(
                "canonical event matrix row in {} must have exactly 5 columns: {}",
                path.display(),
                trimmed
            ));
        }
        let domain = columns[0].to_string();
        if domain.is_empty() {
            return Err(format!(
                "canonical event matrix row in {} must define a non-empty domain",
                path.display()
            ));
        }
        let rpc_methods = columns[3]
            .split(',')
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(|item| item.to_string())
            .collect::<BTreeSet<_>>();
        if rpc_methods.is_empty() {
            return Err(format!(
                "canonical event matrix row {} in {} must define rpc methods",
                domain,
                path.display()
            ));
        }
        let row = EventBoundaryRow {
            domain: domain.clone(),
            kind: columns[1].to_string(),
            radroots_type: columns[2].to_string(),
            rpc_methods,
        };
        if rows.insert(domain.clone(), row).is_some() {
            return Err(format!(
                "canonical event matrix {} has duplicate domain row {}",
                path.display(),
                domain
            ));
        }
    }

    if rows.is_empty() {
        return Err(format!(
            "canonical event matrix {} does not contain the coverage table",
            path.display()
        ));
    }

    Ok(rows)
}

fn validate_event_boundary_source_witness(
    workspace_root: &Path,
    domain: &str,
    witness: &EventBoundarySourceWitness,
) -> Result<(), String> {
    let path = workspace_root.join(witness.relative_path);
    let source = match fs::read_to_string(&path) {
        Ok(source) => source,
        Err(e) => return Err(format!("read {}: {e}", path.display())),
    };
    for fragment in witness.required_fragments {
        if !source.contains(fragment) {
            return Err(format!(
                "canonical event row {} is missing required implementation fragment {} in {}",
                domain,
                fragment,
                path.display()
            ));
        }
    }
    Ok(())
}

fn validate_canonical_event_boundary_with_override(
    workspace_root: &Path,
    event_boundary_override: Option<PathBuf>,
) -> Result<(), String> {
    let matrix_path =
        resolve_event_boundary_matrix_path_with_override(workspace_root, event_boundary_override)?;
    let rows = parse_event_boundary_matrix(&matrix_path)?;
    let expected_domains = CANONICAL_EVENT_BOUNDARY_EXPECTATIONS
        .iter()
        .map(|row| row.domain.to_string())
        .collect::<BTreeSet<_>>();
    let actual_domains = rows.keys().cloned().collect::<BTreeSet<_>>();
    if actual_domains != expected_domains {
        let missing = expected_domains
            .difference(&actual_domains)
            .cloned()
            .collect::<BTreeSet<_>>();
        let extra = actual_domains
            .difference(&expected_domains)
            .cloned()
            .collect::<BTreeSet<_>>();
        return Err(format!(
            "canonical event matrix {} is missing rows: {}; and includes unexpected rows: {}",
            matrix_path.display(),
            join_set(&missing),
            join_set(&extra)
        ));
    }

    for expectation in CANONICAL_EVENT_BOUNDARY_EXPECTATIONS {
        let row = rows.get(expectation.domain).ok_or_else(|| {
            format!(
                "canonical event matrix {} is missing required row {}",
                matrix_path.display(),
                expectation.domain
            )
        })?;
        if row.kind != expectation.kind {
            return Err(format!(
                "canonical event row {} kind drift: expected {}, got {}",
                expectation.domain, expectation.kind, row.kind
            ));
        }
        if row.radroots_type != expectation.radroots_type {
            return Err(format!(
                "canonical event row {} type drift: expected {}, got {}",
                expectation.domain, expectation.radroots_type, row.radroots_type
            ));
        }
        let expected_methods = expectation
            .rpc_methods
            .iter()
            .map(|method| (*method).to_string())
            .collect::<BTreeSet<_>>();
        if row.rpc_methods != expected_methods {
            return Err(format!(
                "canonical event row {} rpc drift: expected {}, got {}",
                expectation.domain,
                join_set(&expected_methods),
                join_set(&row.rpc_methods)
            ));
        }
        for witness in expectation.witnesses {
            validate_event_boundary_source_witness(workspace_root, expectation.domain, witness)?;
        }
    }

    Ok(())
}

pub fn validate_canonical_event_boundary(workspace_root: &Path) -> Result<(), String> {
    validate_canonical_event_boundary_with_override(workspace_root, None)
}

fn contract_root(workspace_root: &Path) -> PathBuf {
    workspace_root.join("spec")
}

fn conformance_root(workspace_root: &Path) -> PathBuf {
    workspace_root.join(CONFORMANCE_ROOT_RELATIVE)
}

fn conformance_schema_path(workspace_root: &Path) -> PathBuf {
    workspace_root.join(CONFORMANCE_SCHEMA_RELATIVE)
}

fn required_field_set(value: &Value, field: &str, path: &Path) -> Result<BTreeSet<String>, String> {
    let required = value
        .as_array()
        .ok_or_else(|| format!("{field} in {} must be an array", path.display()))?;
    let mut names = BTreeSet::new();
    for item in required {
        let name = item
            .as_str()
            .ok_or_else(|| format!("{field} in {} must contain strings", path.display()))?;
        if name.trim().is_empty() {
            return Err(format!(
                "{field} in {} must not contain empty names",
                path.display()
            ));
        }
        names.insert(name.to_string());
    }
    Ok(names)
}

fn validate_string_schema_property(
    property: &Value,
    field: &str,
    path: &Path,
    min_length: Option<u64>,
    pattern: Option<&str>,
) -> Result<(), String> {
    let property = property
        .as_object()
        .ok_or_else(|| format!("{field} schema in {} must be an object", path.display()))?;
    let kind = property
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{field} schema in {} must declare type", path.display()))?;
    if kind != "string" {
        return Err(format!(
            "{field} schema in {} must use type=string",
            path.display()
        ));
    }
    if let Some(expected) = min_length {
        let actual = property
            .get("minLength")
            .and_then(Value::as_u64)
            .ok_or_else(|| format!("{field} schema in {} must set minLength", path.display()))?;
        if actual != expected {
            return Err(format!(
                "{field} schema in {} must set minLength={expected}",
                path.display()
            ));
        }
    }
    if let Some(expected) = pattern {
        let actual = property
            .get("pattern")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("{field} schema in {} must set pattern", path.display()))?;
        if actual != expected {
            return Err(format!(
                "{field} schema in {} must set pattern {}",
                path.display(),
                expected
            ));
        }
    }
    Ok(())
}

fn validate_conformance_schema(workspace_root: &Path) -> Result<(), String> {
    let path = conformance_schema_path(workspace_root);
    let schema = parse_json::<Value>(&path)?;
    let schema_obj = schema.as_object().ok_or_else(|| {
        format!(
            "conformance schema {} must be a JSON object",
            path.display()
        )
    })?;
    let schema_type = schema_obj
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("conformance schema {} must declare type", path.display()))?;
    if schema_type != "object" {
        return Err(format!(
            "conformance schema {} must use type=object",
            path.display()
        ));
    }
    let additional = schema_obj
        .get("additionalProperties")
        .and_then(Value::as_bool)
        .ok_or_else(|| {
            format!(
                "conformance schema {} must declare additionalProperties",
                path.display()
            )
        })?;
    if additional {
        return Err(format!(
            "conformance schema {} must disallow additionalProperties",
            path.display()
        ));
    }
    let root_required = required_field_set(
        schema_obj.get("required").ok_or_else(|| {
            format!(
                "conformance schema {} missing required list",
                path.display()
            )
        })?,
        "required",
        &path,
    )?;
    let expected_root_required = BTreeSet::from([
        "suite".to_string(),
        "contract_version".to_string(),
        "vectors".to_string(),
    ]);
    if root_required != expected_root_required {
        return Err(format!(
            "conformance schema {} must require suite, contract_version, and vectors",
            path.display()
        ));
    }
    let properties = schema_obj
        .get("properties")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            format!(
                "conformance schema {} missing properties map",
                path.display()
            )
        })?;
    validate_string_schema_property(
        properties.get("suite").ok_or_else(|| {
            format!(
                "conformance schema {} missing suite property",
                path.display()
            )
        })?,
        "suite",
        &path,
        Some(1),
        None,
    )?;
    validate_string_schema_property(
        properties.get("contract_version").ok_or_else(|| {
            format!(
                "conformance schema {} missing contract_version property",
                path.display()
            )
        })?,
        "contract_version",
        &path,
        None,
        Some("^[0-9]+\\.[0-9]+\\.[0-9]+$"),
    )?;
    let vectors = properties
        .get("vectors")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            format!(
                "conformance schema {} missing vectors property",
                path.display()
            )
        })?;
    let vectors_type = vectors
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("vectors schema in {} must declare type", path.display()))?;
    if vectors_type != "array" {
        return Err(format!(
            "vectors schema in {} must use type=array",
            path.display()
        ));
    }
    let items = vectors
        .get("items")
        .and_then(Value::as_object)
        .ok_or_else(|| format!("vectors schema in {} must define items", path.display()))?;
    let items_type = items
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("vector item schema in {} must declare type", path.display()))?;
    if items_type != "object" {
        return Err(format!(
            "vector item schema in {} must use type=object",
            path.display()
        ));
    }
    let items_additional = items
        .get("additionalProperties")
        .and_then(Value::as_bool)
        .ok_or_else(|| {
            format!(
                "vector item schema in {} must declare additionalProperties",
                path.display()
            )
        })?;
    if items_additional {
        return Err(format!(
            "vector item schema in {} must disallow additionalProperties",
            path.display()
        ));
    }
    let item_required = required_field_set(
        items.get("required").ok_or_else(|| {
            format!(
                "vector item schema in {} missing required list",
                path.display()
            )
        })?,
        "required",
        &path,
    )?;
    let expected_item_required = BTreeSet::from([
        "expected".to_string(),
        "id".to_string(),
        "input".to_string(),
        "kind".to_string(),
    ]);
    if item_required != expected_item_required {
        return Err(format!(
            "vector item schema in {} must require id, kind, input, and expected",
            path.display()
        ));
    }
    let item_properties = items
        .get("properties")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            format!(
                "vector item schema in {} missing properties",
                path.display()
            )
        })?;
    validate_string_schema_property(
        item_properties.get("id").ok_or_else(|| {
            format!(
                "vector item schema in {} missing id property",
                path.display()
            )
        })?,
        "id",
        &path,
        Some(1),
        None,
    )?;
    validate_string_schema_property(
        item_properties.get("kind").ok_or_else(|| {
            format!(
                "vector item schema in {} missing kind property",
                path.display()
            )
        })?,
        "kind",
        &path,
        Some(1),
        None,
    )?;
    for field in ["input", "expected"] {
        let property = item_properties.get(field).ok_or_else(|| {
            format!(
                "vector item schema in {} missing {} property",
                path.display(),
                field
            )
        })?;
        if !property.is_object() {
            return Err(format!(
                "vector item schema in {} must define {} as an object schema",
                path.display(),
                field
            ));
        }
    }
    Ok(())
}

fn base_contract_version(version: &str) -> &str {
    version.split_once('-').map_or(version, |(base, _)| base)
}

#[derive(Debug)]
struct WorkspacePackageRecord {
    name: String,
    #[cfg_attr(not(test), allow(dead_code))]
    manifest_path: PathBuf,
    publish_enabled: bool,
    publish: Option<PackagePublish>,
    manifest_value: toml::Value,
}

fn workspace_package_records(workspace_root: &Path) -> Result<Vec<WorkspacePackageRecord>, String> {
    let workspace_manifest =
        parse_toml::<WorkspaceCargoManifest>(&workspace_root.join("Cargo.toml"))?;
    let mut records = Vec::with_capacity(workspace_manifest.workspace.members.len());
    for member in workspace_manifest.workspace.members {
        let manifest_path = workspace_root.join(&member).join("Cargo.toml");
        let raw = match fs::read_to_string(&manifest_path) {
            Ok(raw) => raw,
            Err(e) => return Err(format!("read {}: {e}", manifest_path.display())),
        };
        let manifest_value = match toml::from_str::<toml::Value>(&raw) {
            Ok(value) => value,
            Err(e) => return Err(format!("parse {}: {e}", manifest_path.display())),
        };
        let package_manifest = match toml::from_str::<PackageCargoManifest>(&raw) {
            Ok(manifest) => manifest,
            Err(e) => return Err(format!("parse {}: {e}", manifest_path.display())),
        };
        let name = package_manifest.package.name;
        let publish_enabled = package_publish_enabled(package_manifest.package.publish.as_ref());
        let publish = package_manifest.package.publish.clone();
        records.push(WorkspacePackageRecord {
            name,
            manifest_path,
            publish_enabled,
            publish,
            manifest_value,
        });
    }
    Ok(records)
}

fn workspace_package_names(workspace_root: &Path) -> Result<Vec<String>, String> {
    Ok(workspace_package_records(workspace_root)?
        .into_iter()
        .map(|record| record.name)
        .collect())
}

#[cfg_attr(not(test), allow(dead_code))]
fn workspace_package_manifests(workspace_root: &Path) -> Result<BTreeMap<String, PathBuf>, String> {
    let mut manifests = BTreeMap::new();
    for record in workspace_package_records(workspace_root)? {
        if manifests
            .insert(record.name, record.manifest_path)
            .is_some()
        {
            return Err("duplicate workspace package name in manifest map".to_string());
        }
    }
    Ok(manifests)
}

fn load_coverage_policy(
    contract_root: &Path,
) -> Result<crate::coverage::CoveragePolicyFile, String> {
    read_coverage_policy(&coverage_root(contract_root).join("policy.toml"))
}

fn coverage_root(contract_root: &Path) -> PathBuf {
    contract_root
        .parent()
        .unwrap_or(contract_root)
        .join("policy")
        .join("coverage")
}

#[cfg_attr(not(test), allow(dead_code))]
fn root_release_policy_path(workspace_root: &Path) -> PathBuf {
    workspace_root.join(ROOT_RELEASE_POLICY_RELATIVE)
}

fn resolve_release_contract_path_with_override(
    workspace_root: &Path,
    release_policy_override: Option<PathBuf>,
) -> Result<Option<PathBuf>, String> {
    if let Some(path) = release_policy_override {
        if !path.is_file() {
            return Err(format!(
                "{RELEASE_POLICY_ENV} points to a missing release policy file: {}",
                path.display()
            ));
        }
        return Ok(Some(path));
    }

    for ancestor in workspace_root.ancestors() {
        let candidate = ancestor.join(ROOT_RELEASE_POLICY_RELATIVE);
        if candidate.is_file() {
            return Ok(Some(candidate));
        }
    }

    Ok(None)
}

fn resolve_release_contract_path(workspace_root: &Path) -> Result<Option<PathBuf>, String> {
    resolve_release_contract_path_with_override(
        workspace_root,
        env::var_os(RELEASE_POLICY_ENV).map(PathBuf::from),
    )
}

fn load_release_contract(
    workspace_root: &Path,
    contract_root: &Path,
) -> Result<ReleaseContractFile, String> {
    load_release_contract_with_override(
        workspace_root,
        contract_root,
        env::var_os(RELEASE_POLICY_ENV).map(PathBuf::from),
    )
}

fn load_release_contract_with_override(
    workspace_root: &Path,
    _contract_root: &Path,
    release_policy_override: Option<PathBuf>,
) -> Result<ReleaseContractFile, String> {
    let path =
        resolve_release_contract_path_with_override(workspace_root, release_policy_override)?
            .ok_or_else(|| {
                format!(
                    "release publish policy not found; expected {}",
                    ROOT_RELEASE_POLICY_RELATIVE
                )
            })?;
    parse_toml::<ReleaseContractFile>(&path)
}

fn package_publish_enabled(publish: Option<&PackagePublish>) -> bool {
    match publish {
        None => true,
        Some(PackagePublish::Bool(flag)) => *flag,
        Some(PackagePublish::Registries(registries)) => !registries.is_empty(),
    }
}

#[cfg_attr(not(test), allow(dead_code))]
fn workspace_package_publish_flags(
    workspace_root: &Path,
) -> Result<BTreeMap<String, bool>, String> {
    let mut flags = BTreeMap::new();
    for record in workspace_package_records(workspace_root)? {
        if flags
            .insert(record.name.clone(), record.publish_enabled)
            .is_some()
        {
            return Err(format!("duplicate workspace package name {}", record.name));
        }
    }
    Ok(flags)
}

fn workspace_package_publish_configs(
    workspace_root: &Path,
) -> Result<BTreeMap<String, Option<PackagePublish>>, String> {
    let mut configs = BTreeMap::new();
    for record in workspace_package_records(workspace_root)? {
        if configs
            .insert(record.name.clone(), record.publish.clone())
            .is_some()
        {
            return Err(format!("duplicate workspace package name {}", record.name));
        }
    }
    Ok(configs)
}

fn read_workspace_package_dependencies(
    workspace_root: &Path,
) -> Result<BTreeMap<String, BTreeSet<String>>, String> {
    let package_records = workspace_package_records(workspace_root)?;
    let workspace_names = package_records
        .iter()
        .map(|record| record.name.clone())
        .collect::<BTreeSet<_>>();

    let mut deps = BTreeMap::new();
    for record in package_records {
        let mut package_deps = BTreeSet::new();
        for section in ["dependencies", "build-dependencies"] {
            let Some(table) = record
                .manifest_value
                .get(section)
                .and_then(toml::Value::as_table)
            else {
                continue;
            };
            for dep_name in table.keys() {
                if workspace_names.contains(dep_name) {
                    package_deps.insert(dep_name.clone());
                }
            }
        }
        deps.insert(record.name, package_deps);
    }

    Ok(deps)
}

fn join_set(items: &BTreeSet<String>) -> String {
    items.iter().cloned().collect::<Vec<_>>().join(", ")
}

fn collect_unique_set(items: &[String], field: &str) -> Result<BTreeSet<String>, String> {
    let mut set = BTreeSet::new();
    for item in items {
        if item.trim().is_empty() {
            return Err(format!("{field} contains an empty crate name"));
        }
        if !set.insert(item.clone()) {
            return Err(format!("{field} has duplicate crate {}", item));
        }
    }
    Ok(set)
}

fn collect_non_empty_set(items: &[String], field: &str) -> Result<BTreeSet<String>, String> {
    let mut set = BTreeSet::new();
    for item in items {
        if item.trim().is_empty() {
            return Err(format!("{field} contains an empty value"));
        }
        if !set.insert(item.clone()) {
            return Err(format!("{field} has duplicate value {}", item));
        }
    }
    Ok(set)
}

fn ts_curated_package_set(bundle: &ContractBundle) -> Result<Option<BTreeSet<String>>, String> {
    let Some(export_targets) = bundle.manifest.export.as_ref() else {
        return Ok(None);
    };
    let Some(ts_export) = export_targets.ts.as_ref() else {
        return Ok(None);
    };
    let packages = collect_non_empty_set(&ts_export.packages, "manifest export.ts.packages")?;
    if packages.is_empty() {
        return Err("manifest export.ts.packages must not be empty".to_string());
    }
    Ok(Some(packages))
}

fn validate_operations_contract(
    bundle: &ContractBundle,
    operations_manifest: &OperationsContractManifest,
    workspace_root: &Path,
) -> Result<(), String> {
    validate_conformance_schema(workspace_root)?;
    let conformance_root = conformance_root(workspace_root);
    if operations_manifest.contract.name.trim().is_empty() {
        return Err("operations contract name is required".to_string());
    }
    if operations_manifest.contract.version.trim().is_empty() {
        return Err("operations contract version is required".to_string());
    }
    if operations_manifest.contract.source.trim().is_empty() {
        return Err("operations contract source is required".to_string());
    }
    if operations_manifest.contract.name != bundle.manifest.contract.name {
        return Err("operations contract name must match manifest contract name".to_string());
    }
    if operations_manifest.contract.version != bundle.manifest.contract.version {
        return Err("operations contract version must match manifest contract version".to_string());
    }
    if operations_manifest.contract.source != bundle.manifest.contract.source {
        return Err("operations contract source must match manifest contract source".to_string());
    }

    let domains = collect_non_empty_set(&operations_manifest.public.domains, "public.domains")?;
    if domains.is_empty() {
        return Err("public.domains must not be empty".to_string());
    }
    let shared_types = collect_non_empty_set(
        &operations_manifest.shared_types.public,
        "shared_types.public",
    )?;
    if shared_types.is_empty() {
        return Err("shared_types.public must not be empty".to_string());
    }
    let error_classes =
        collect_non_empty_set(&operations_manifest.errors.classes, "errors.classes")?;
    if error_classes.is_empty() {
        return Err("errors.classes must not be empty".to_string());
    }
    if operations_manifest.operations.is_empty() {
        return Err("operations map must not be empty".to_string());
    }

    if let Some(provenance) = &operations_manifest.implementation_provenance {
        let manifest_models = collect_unique_set(
            &bundle.manifest.surface.model_crates,
            "surface.model_crates",
        )?;
        let manifest_algorithms = collect_unique_set(
            &bundle.manifest.surface.algorithm_crates,
            "surface.algorithm_crates",
        )?;
        let manifest_wasm =
            collect_unique_set(&bundle.manifest.surface.wasm_crates, "surface.wasm_crates")?;
        let provenance_models = collect_unique_set(
            &provenance.model_crates,
            "implementation_provenance.model_crates",
        )?;
        let provenance_algorithms = collect_unique_set(
            &provenance.algorithm_crates,
            "implementation_provenance.algorithm_crates",
        )?;
        let provenance_wasm = collect_unique_set(
            &provenance.wasm_crates,
            "implementation_provenance.wasm_crates",
        )?;
        if provenance_models != manifest_models
            || provenance_algorithms != manifest_algorithms
            || provenance_wasm != manifest_wasm
        {
            return Err(
                "operations implementation_provenance must match manifest surface crates"
                    .to_string(),
            );
        }
    }

    let mut operation_ids = BTreeSet::new();
    for (operation_key, operation) in &operations_manifest.operations {
        if operation_key.trim().is_empty() {
            return Err("operations map contains an empty key".to_string());
        }
        if operation.domain.trim().is_empty() {
            return Err(format!("operation {} domain is required", operation_key));
        }
        if !domains.contains(&operation.domain) {
            return Err(format!(
                "operation {} references unknown domain {}",
                operation_key, operation.domain
            ));
        }
        if operation.id.trim().is_empty() {
            return Err(format!("operation {} id is required", operation_key));
        }
        if !operation_ids.insert(operation.id.clone()) {
            return Err(format!("operations has duplicate id {}", operation.id));
        }
        if operation.stability.trim().is_empty() {
            return Err(format!("operation {} stability is required", operation.id));
        }
        if !operation.deterministic {
            return Err(format!(
                "operation {} deterministic must be true for the public contract",
                operation.id
            ));
        }
        if operation.inputs.is_empty() {
            return Err(format!(
                "operation {} inputs must not be empty",
                operation.id
            ));
        }
        let _ = collect_non_empty_set(
            &operation.inputs,
            &format!("operation {} inputs", operation.id),
        )?;
        if operation.outputs.is_empty() {
            return Err(format!(
                "operation {} outputs must not be empty",
                operation.id
            ));
        }
        let _ = collect_non_empty_set(
            &operation.outputs,
            &format!("operation {} outputs", operation.id),
        )?;
        if !error_classes.contains(&operation.error_class) {
            return Err(format!(
                "operation {} references unknown error class {}",
                operation.id, operation.error_class
            ));
        }
        if operation.signing.trim().is_empty() {
            return Err(format!("operation {} signing is required", operation.id));
        }
        if operation.transport.trim().is_empty() {
            return Err(format!("operation {} transport is required", operation.id));
        }
        if operation.implementation.rust_modules.is_empty() {
            return Err(format!(
                "operation {} implementation.rust_modules must not be empty",
                operation.id
            ));
        }
        let _ = collect_non_empty_set(
            &operation.implementation.rust_types,
            &format!("operation {} implementation.rust_types", operation.id),
        )?;
        for rust_module in &operation.implementation.rust_modules {
            if rust_module.trim().is_empty() {
                return Err(format!(
                    "operation {} implementation.rust_modules contains an empty value",
                    operation.id
                ));
            }
            let path = workspace_root.join(rust_module);
            if !path.is_file() {
                return Err(format!(
                    "operation {} references missing rust module {}",
                    operation.id, rust_module
                ));
            }
        }
        if operation.conformance.vector.trim().is_empty() {
            return Err(format!(
                "operation {} conformance.vector is required",
                operation.id
            ));
        }
        if !operation
            .conformance
            .vector
            .starts_with("spec/conformance/")
        {
            return Err(format!(
                "operation {} conformance.vector must live under spec/conformance/",
                operation.id
            ));
        }
        let vector_path = workspace_root.join(&operation.conformance.vector);
        if !vector_path.starts_with(&conformance_root) {
            return Err(format!(
                "operation {} conformance.vector must resolve under {}",
                operation.id,
                conformance_root.display()
            ));
        }
        let vector = parse_json::<ConformanceVectorFile>(&vector_path)?;
        if vector.suite.trim().is_empty() {
            return Err(format!(
                "operation {} conformance vector suite must not be empty",
                operation.id
            ));
        }
        if vector.vectors.is_empty() {
            return Err(format!(
                "operation {} conformance vector must contain at least one vector",
                operation.id
            ));
        }
        if vector.contract_version != base_contract_version(&operations_manifest.contract.version) {
            return Err(format!(
                "operation {} conformance vector version {} must match contract version {}",
                operation.id,
                vector.contract_version,
                base_contract_version(&operations_manifest.contract.version)
            ));
        }
        for entry in vector.vectors {
            if entry.id.trim().is_empty() || entry.kind.trim().is_empty() {
                return Err(format!(
                    "operation {} conformance vector entries must define non-empty id and kind",
                    operation.id
                ));
            }
        }
    }

    if bundle.sdk_exports.is_empty() {
        return Err(
            "sdk-exports must define at least one operation-based language mapping".to_string(),
        );
    }

    let ts_packages = ts_curated_package_set(bundle)?;
    let mut has_ts_mapping = false;
    for mapping in &bundle.sdk_exports {
        if mapping.language.id.trim().is_empty() {
            return Err("sdk export language.id is required".to_string());
        }
        if mapping.language.repository.trim().is_empty() {
            return Err(format!(
                "sdk export language.repository is required for {}",
                mapping.language.id
            ));
        }
        if mapping.language.id == "ts" {
            has_ts_mapping = true;
        }
        if mapping.sdk.package.trim().is_empty() {
            return Err(format!(
                "sdk export package is required for {}",
                mapping.language.id
            ));
        }
        if mapping.sdk.deterministic_codec.trim().is_empty()
            || mapping.sdk.signing.trim().is_empty()
            || mapping.sdk.networking.trim().is_empty()
        {
            return Err(format!(
                "sdk runtime fields must be non-empty for {}",
                mapping.language.id
            ));
        }
        if let Some(module_format) = mapping.sdk.module_format.as_deref() {
            if module_format.trim().is_empty() {
                return Err(format!(
                    "sdk module_format must be non-empty for {}",
                    mapping.language.id
                ));
            }
        }
        if mapping.operations.is_empty() {
            return Err(format!(
                "sdk export operations map is required for {}",
                mapping.language.id
            ));
        }
        for (operation_id, symbol) in &mapping.operations {
            if !operation_ids.contains(operation_id) {
                return Err(format!(
                    "sdk export {} references unknown operation {}",
                    mapping.language.id, operation_id
                ));
            }
            if symbol.trim().is_empty() {
                return Err(format!(
                    "sdk export {} must map operation {} to a non-empty symbol",
                    mapping.language.id, operation_id
                ));
            }
        }
        if mapping.shared_types.is_empty() {
            return Err(format!(
                "sdk export shared_types map is required for {}",
                mapping.language.id
            ));
        }
        for (shared_type, symbol) in &mapping.shared_types {
            if !shared_types.contains(shared_type) {
                return Err(format!(
                    "sdk export {} references unknown shared type {}",
                    mapping.language.id, shared_type
                ));
            }
            if symbol.trim().is_empty() {
                return Err(format!(
                    "sdk export {} must map shared type {} to a non-empty symbol",
                    mapping.language.id, shared_type
                ));
            }
        }
        if mapping.language.id == "ts" {
            if operation_ids != mapping.operations.keys().cloned().collect::<BTreeSet<_>>() {
                return Err(
                    "sdk export ts must cover every public operation in operations.toml"
                        .to_string(),
                );
            }
            if let Some(expected_packages) = ts_packages.as_ref() {
                if expected_packages.len() != 1 {
                    return Err(
                        "manifest export.ts.packages must define exactly one curated ts package"
                            .to_string(),
                    );
                }
                let expected_package = expected_packages
                    .iter()
                    .next()
                    .expect("single-package ts export set");
                if mapping.sdk.package != *expected_package {
                    return Err(format!(
                        "sdk export ts package {} must match manifest export.ts.packages {}",
                        mapping.sdk.package, expected_package
                    ));
                }
            }
            let artifacts = mapping
                .artifacts
                .as_ref()
                .ok_or_else(|| "sdk export artifacts map is required for ts".to_string())?;
            for (field, value) in [
                ("models_dir", artifacts.models_dir.as_ref()),
                ("runtime_dir", artifacts.runtime_dir.as_ref()),
                ("wasm_dist_dir", artifacts.wasm_dist_dir.as_ref()),
                ("manifest_file", artifacts.manifest_file.as_ref()),
            ] {
                if value.is_none_or(|raw| raw.trim().is_empty()) {
                    return Err(format!(
                        "sdk export artifacts.{field} must be non-empty for ts"
                    ));
                }
            }
        }
    }
    if !has_ts_mapping {
        return Err("sdk-exports must include a ts mapping".to_string());
    }

    Ok(())
}

fn package_field_configured(table: &toml::value::Table, field: &str) -> bool {
    let Some(value) = table.get(field) else {
        return false;
    };
    match value {
        toml::Value::String(raw) => !raw.trim().is_empty(),
        toml::Value::Table(inner) => inner
            .get("workspace")
            .and_then(toml::Value::as_bool)
            .is_some_and(|configured| configured),
        _ => false,
    }
}

fn validate_publish_package_metadata(
    workspace_root: &Path,
    publish_crates: &BTreeSet<String>,
) -> Result<(), String> {
    let mut package_tables = BTreeMap::new();
    for record in workspace_package_records(workspace_root)? {
        if package_tables
            .insert(record.name, record.manifest_value)
            .is_some()
        {
            return Err("duplicate workspace package name in package metadata map".to_string());
        }
    }
    for crate_name in publish_crates {
        let parsed = match package_tables.get(crate_name) {
            Some(parsed) => parsed,
            None => {
                return Err(format!(
                    "publish crate {} has no workspace manifest",
                    crate_name
                ));
            }
        };
        let package = parsed
            .get("package")
            .and_then(toml::Value::as_table)
            .expect("workspace package records include [package] table");

        if !package_field_configured(package, "description") {
            return Err(format!(
                "publish crate {} must define a non-empty package.description",
                crate_name
            ));
        }
        for field in ["repository", "homepage", "documentation", "readme"] {
            if !package_field_configured(package, field) {
                return Err(format!(
                    "publish crate {} must configure package.{}",
                    crate_name, field
                ));
            }
        }
    }
    Ok(())
}

fn parse_coverage_percent(raw: &str, field: &str, crate_name: &str) -> Result<f64, String> {
    match raw.parse::<f64>() {
        Ok(value) => Ok(value),
        Err(e) => Err(format!("parse {} for {}: {e}", field, crate_name)),
    }
}

fn load_coverage_refresh_rows(
    workspace_root: &Path,
) -> Result<BTreeMap<String, (String, f64, f64, f64, f64)>, String> {
    let report_path = workspace_root
        .join("target")
        .join("coverage")
        .join("coverage-refresh.tsv");
    let raw = match fs::read_to_string(&report_path) {
        Ok(raw) => raw,
        Err(e) => return Err(format!("read {}: {e}", report_path.display())),
    };
    let mut rows = BTreeMap::new();
    for line in raw.lines().skip(1) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let parts = trimmed.split('\t').collect::<Vec<_>>();
        if parts.len() < 6 {
            return Err(format!(
                "coverage row must have at least 6 columns in {}: {}",
                report_path.display(),
                trimmed
            ));
        }
        let crate_name = parts[0].to_string();
        let status = parts[1].to_string();
        let exec = parse_coverage_percent(parts[2], "exec", &crate_name)?;
        let func = parse_coverage_percent(parts[3], "func", &crate_name)?;
        let branch = parse_coverage_percent(parts[4], "branch", &crate_name)?;
        let region = parse_coverage_percent(parts[5], "region", &crate_name)?;
        if rows
            .insert(crate_name.clone(), (status, exec, func, branch, region))
            .is_some()
        {
            return Err(format!(
                "duplicate coverage row for crate {} in {}",
                crate_name,
                report_path.display()
            ));
        }
    }
    Ok(rows)
}

#[cfg_attr(not(test), allow(dead_code))]
fn validate_required_coverage_summary(
    workspace_root: &Path,
    required_crates: &BTreeSet<String>,
    thresholds: CoverageThresholds,
) -> Result<(), String> {
    let rows = load_coverage_refresh_rows(workspace_root)?;
    for crate_name in required_crates {
        let (status, exec, func, branch, region) = rows.get(crate_name).ok_or_else(|| {
            format!(
                "required coverage crate {} missing from coverage-refresh.tsv",
                crate_name
            )
        })?;
        if status != "pass" {
            return Err(format!(
                "required coverage crate {} has non-pass status {}",
                crate_name, status
            ));
        }
        if *exec < thresholds.fail_under_exec_lines
            || *func < thresholds.fail_under_functions
            || *branch < thresholds.fail_under_branches
            || *region < thresholds.fail_under_regions
        {
            return Err(format!(
                "required coverage crate {} must satisfy coverage policy {},{},{},{}, found {}/{}/{}/{}",
                crate_name,
                thresholds.fail_under_exec_lines,
                thresholds.fail_under_functions,
                thresholds.fail_under_branches,
                thresholds.fail_under_regions,
                exec,
                func,
                branch,
                region
            ));
        }
    }
    Ok(())
}

fn validate_required_coverage_summary_with_policy(
    workspace_root: &Path,
    required_crates: &BTreeSet<String>,
    policy: &CoveragePolicyFile,
) -> Result<(), String> {
    let rows = load_coverage_refresh_rows(workspace_root)?;
    for crate_name in required_crates {
        let (status, exec, func, branch, region) = rows.get(crate_name).ok_or_else(|| {
            format!(
                "required coverage crate {} missing from coverage-refresh.tsv",
                crate_name
            )
        })?;
        if status != "pass" {
            return Err(format!(
                "required coverage crate {} has non-pass status {}",
                crate_name, status
            ));
        }
        let thresholds = policy.thresholds_for_scope(crate_name);
        if *exec < thresholds.fail_under_exec_lines
            || *func < thresholds.fail_under_functions
            || *branch < thresholds.fail_under_branches
            || *region < thresholds.fail_under_regions
        {
            return Err(format!(
                "required coverage crate {} must satisfy coverage policy {},{},{},{}, found {}/{}/{}/{}",
                crate_name,
                thresholds.fail_under_exec_lines,
                thresholds.fail_under_functions,
                thresholds.fail_under_branches,
                thresholds.fail_under_regions,
                exec,
                func,
                branch,
                region
            ));
        }
    }
    Ok(())
}

const CORE_UNIT_DIMENSION_ENUM: &str = "RadrootsCoreUnitDimension";
const CORE_UNIT_DIMENSION_ORDER: [&str; 3] = ["Count", "Mass", "Volume"];

fn extract_enum_body<'a>(source: &'a str, enum_name: &str) -> Result<&'a str, String> {
    let marker = format!("pub enum {enum_name}");
    let enum_start = match source.find(&marker) {
        Some(index) => index,
        None => return Err(format!("missing enum {enum_name}")),
    };
    let after_start = &source[enum_start..];
    let open_rel = match after_start.find('{') {
        Some(index) => index,
        None => return Err(format!("missing opening brace for enum {enum_name}")),
    };
    let open_idx = enum_start + open_rel;
    let mut depth = 0usize;
    for (offset, ch) in source[open_idx..].char_indices() {
        if ch == '{' {
            depth += 1;
            continue;
        }
        if ch != '}' {
            continue;
        }
        depth = depth.saturating_sub(1);
        if depth == 0 {
            let close_idx = open_idx + offset;
            return Ok(&source[(open_idx + 1)..close_idx]);
        }
    }
    Err(format!("missing closing brace for enum {enum_name}"))
}

fn parse_enum_variants(enum_body: &str) -> Vec<String> {
    enum_body
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") {
                return None;
            }
            let before_comma = trimmed
                .split_once(',')
                .map_or(trimmed, |(head, _)| head)
                .trim();
            if before_comma.is_empty() {
                return None;
            }
            let before_discriminant = before_comma
                .split_once('=')
                .map_or(before_comma, |(head, _)| head)
                .trim();
            if before_discriminant.is_empty() {
                return None;
            }
            let ident = before_discriminant
                .split_whitespace()
                .next()
                .unwrap_or_default();
            Some(ident.to_string())
        })
        .collect()
}

fn validate_core_unit_dimension_variant_order(workspace_root: &Path) -> Result<(), String> {
    let source_path = workspace_root
        .join("crates")
        .join("core")
        .join("src")
        .join("unit.rs");
    let source = match fs::read_to_string(&source_path) {
        Ok(source) => source,
        Err(e) => return Err(format!("read {}: {e}", source_path.display())),
    };
    let enum_body = extract_enum_body(&source, CORE_UNIT_DIMENSION_ENUM)?;
    let variants = parse_enum_variants(enum_body);
    let expected = CORE_UNIT_DIMENSION_ORDER
        .iter()
        .map(|item| (*item).to_string())
        .collect::<Vec<_>>();
    if variants != expected {
        return Err(format!(
            "core unit dimension variant order must be {} but was {}",
            CORE_UNIT_DIMENSION_ORDER.join(", "),
            variants.join(", ")
        ));
    }
    Ok(())
}

fn validate_coverage_policy_parity(
    workspace_root: &Path,
    contract_root: &Path,
) -> Result<(), String> {
    let policy = load_coverage_policy(contract_root)?;
    let release = load_release_contract(workspace_root, contract_root)?;
    let thresholds = policy.thresholds();
    if thresholds.fail_under_exec_lines != 100.0
        || thresholds.fail_under_functions != 100.0
        || thresholds.fail_under_regions != 100.0
        || thresholds.fail_under_branches != 100.0
        || !thresholds.require_branches
    {
        return Err(
            "coverage policy must enforce 100/100/100/100 with required branches".to_string(),
        );
    }

    let required_packages = policy
        .required_crate_entries()
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let public_packages = collect_unique_set(
        &release.public_crates(),
        if release.uses_classification() {
            "classification.public"
        } else {
            "publish.crates"
        },
    )?;
    if public_packages != required_packages {
        let missing = public_packages
            .difference(&required_packages)
            .cloned()
            .collect::<BTreeSet<_>>();
        let extra = required_packages
            .difference(&public_packages)
            .cloned()
            .collect::<BTreeSet<_>>();
        return Err(format!(
            "coverage policy missing public crates: {}; coverage policy includes non-public crates: {}",
            join_set(&missing),
            join_set(&extra)
        ));
    }

    Ok(())
}

fn publish_config_is_public(publish: Option<&PackagePublish>) -> bool {
    matches!(
        publish,
        Some(PackagePublish::Registries(registries))
            if registries.len() == 1 && registries[0] == "crates-io"
    )
}

fn publish_config_is_non_public(publish: Option<&PackagePublish>) -> bool {
    matches!(publish, Some(PackagePublish::Bool(false)))
}

fn validate_release_publish_policy(
    workspace_root: &Path,
    contract_root: &Path,
    contract_version: &str,
) -> Result<(), String> {
    let release = load_release_contract(workspace_root, contract_root)?;
    if release.release.version.trim().is_empty() {
        return Err("release.version must not be empty".to_string());
    }
    if release.release.version != contract_version {
        return Err(format!(
            "release.version {} must match contract version {}",
            release.release.version, contract_version
        ));
    }

    let workspace_packages = workspace_package_names(workspace_root)?
        .into_iter()
        .collect::<BTreeSet<_>>();
    let uses_classification = release.uses_classification();
    let public_field = if uses_classification {
        "classification.public"
    } else {
        "publish.crates"
    };
    let internal_field = if uses_classification {
        "classification.internal"
    } else {
        "internal.crates"
    };

    let public_set = collect_unique_set(&release.public_crates(), public_field)?;
    let internal_set = collect_unique_set(&release.internal_crates(), internal_field)?;
    let deferred_set = collect_unique_set(&release.deferred_crates(), "classification.deferred")?;
    let retired_set = collect_unique_set(&release.retired_crates(), "classification.retired")?;
    let yank_only_set =
        collect_unique_set(&release.yank_only_crates(), "classification.yank_only")?;
    let publish_order = &release.publish_order.crates;
    let publish_order_set = collect_unique_set(publish_order, "publish_order.crates")?;

    let class_sets = [
        ("public", &public_set),
        ("internal", &internal_set),
        ("deferred", &deferred_set),
        ("retired", &retired_set),
        ("yank-only", &yank_only_set),
    ];
    for idx in 0..class_sets.len() {
        for other_idx in (idx + 1)..class_sets.len() {
            let overlap = class_sets[idx]
                .1
                .intersection(class_sets[other_idx].1)
                .cloned()
                .collect::<BTreeSet<_>>();
            if !overlap.is_empty() {
                return Err(format!(
                    "release classification overlap is not allowed between {} and {}: {}",
                    class_sets[idx].0,
                    class_sets[other_idx].0,
                    join_set(&overlap)
                ));
            }
        }
    }

    let mut combined = public_set.clone();
    combined.extend(internal_set.iter().cloned());
    combined.extend(deferred_set.iter().cloned());
    combined.extend(retired_set.iter().cloned());
    combined.extend(yank_only_set.iter().cloned());
    if combined != workspace_packages {
        let missing = workspace_packages
            .difference(&combined)
            .cloned()
            .collect::<BTreeSet<_>>();
        let extra = combined
            .difference(&workspace_packages)
            .cloned()
            .collect::<BTreeSet<_>>();
        return Err(format!(
            "release classification sets are missing workspace crates: {}; release classification sets include unknown crates: {}",
            join_set(&missing),
            join_set(&extra)
        ));
    }

    if publish_order_set != public_set {
        let missing = public_set
            .difference(&publish_order_set)
            .cloned()
            .collect::<BTreeSet<_>>();
        let extra = publish_order_set
            .difference(&public_set)
            .cloned()
            .collect::<BTreeSet<_>>();
        return Err(format!(
            "publish_order.crates is missing publish crates: {}; publish_order.crates has non-publish crates: {}",
            join_set(&missing),
            join_set(&extra)
        ));
    }

    let order_index = publish_order
        .iter()
        .enumerate()
        .map(|(idx, name)| (name.clone(), idx))
        .collect::<BTreeMap<_, _>>();
    let dependencies = read_workspace_package_dependencies(workspace_root)
        .expect("workspace package manifests were already parsed");
    for crate_name in &public_set {
        let crate_deps = &dependencies[crate_name];
        let crate_order = order_index[crate_name];
        for dep in crate_deps {
            if !public_set.contains(dep) {
                continue;
            }
            let dep_order = order_index[dep];
            if dep_order >= crate_order {
                return Err(format!(
                    "publish order must place dependency {} before {}",
                    dep, crate_name
                ));
            }
        }
    }

    let publish_configs = workspace_package_publish_configs(workspace_root)
        .expect("workspace publish configs are stable");
    for crate_name in &public_set {
        let publish = publish_configs[crate_name].as_ref();
        if !publish_config_is_public(publish) {
            return Err(format!(
                "public crate {} must set publish = [\"crates-io\"]",
                crate_name
            ));
        }
    }
    for crate_name in internal_set
        .iter()
        .chain(deferred_set.iter())
        .chain(retired_set.iter())
        .chain(yank_only_set.iter())
    {
        let publish = publish_configs[crate_name].as_ref();
        if !publish_config_is_non_public(publish) {
            return Err(format!(
                "non-public crate {} must set publish = false",
                crate_name
            ));
        }
    }

    Ok(())
}

pub fn validate_release_preflight(workspace_root: &Path) -> Result<(), String> {
    validate_release_preflight_with_override(workspace_root, None)
}

pub fn validate_release_preflight_with_override(
    workspace_root: &Path,
    release_policy_override: Option<PathBuf>,
) -> Result<(), String> {
    let bundle = load_contract_bundle(workspace_root)?;
    validate_contract_bundle_with_release_policy_override(
        &bundle,
        release_policy_override.clone(),
    )?;
    let release =
        load_release_contract_with_override(workspace_root, &bundle.root, release_policy_override)?;
    let policy =
        load_coverage_policy(&bundle.root).expect("validated contract includes coverage policy");
    let publish_crates = collect_unique_set(
        &release.public_crates(),
        if release.uses_classification() {
            "classification.public"
        } else {
            "publish.crates"
        },
    )
    .expect("validated contract enforces unique public crates");
    let required_crate_list = policy
        .required_crates()
        .expect("validated contract includes required crates");
    let required_crates = collect_unique_set(&required_crate_list, "required.crates")
        .expect("validated contract enforces unique required.crates");
    validate_publish_package_metadata(workspace_root, &publish_crates)?;
    validate_required_coverage_summary_with_policy(workspace_root, &required_crates, &policy)?;
    Ok(())
}

fn validate_contract_bundle_with_release_policy_override(
    bundle: &ContractBundle,
    release_policy_override: Option<PathBuf>,
) -> Result<(), String> {
    if bundle.manifest.contract.name.trim().is_empty() {
        return Err("contract name is required".to_string());
    }
    if bundle.manifest.contract.version.trim().is_empty() {
        return Err("contract version is required".to_string());
    }
    if bundle.manifest.contract.source.trim().is_empty() {
        return Err("contract source is required".to_string());
    }
    if bundle.manifest.surface.model_crates.is_empty() {
        return Err("contract surface.model_crates must not be empty".to_string());
    }
    if bundle.manifest.surface.algorithm_crates.is_empty() {
        return Err("contract surface.algorithm_crates must not be empty".to_string());
    }
    if bundle.manifest.surface.wasm_crates.is_empty() {
        return Err("contract surface.wasm_crates must not be empty".to_string());
    }
    if bundle.exports.is_empty() {
        return Err("at least one language export mapping is required".to_string());
    }
    let ts_packages = ts_curated_package_set(bundle)?;
    for mapping in &bundle.exports {
        if mapping.language.id.trim().is_empty() {
            return Err("language.id is required".to_string());
        }
        if mapping.language.repository.trim().is_empty() {
            return Err(format!(
                "language.repository is required for {}",
                mapping.language.id
            ));
        }
        if mapping.packages.is_empty() {
            return Err(format!(
                "packages map is required for {}",
                mapping.language.id
            ));
        }
        if mapping.language.id == "ts" {
            let artifacts = match mapping.artifacts.as_ref() {
                Some(artifacts) => artifacts,
                None => return Err("artifacts map is required for ts".to_string()),
            };
            if artifacts
                .models_dir
                .as_deref()
                .is_none_or(|value| value.trim().is_empty())
                || artifacts
                    .constants_dir
                    .as_deref()
                    .is_none_or(|value| value.trim().is_empty())
                || artifacts
                    .wasm_dist_dir
                    .as_deref()
                    .is_none_or(|value| value.trim().is_empty())
                || artifacts
                    .manifest_file
                    .as_deref()
                    .is_none_or(|value| value.trim().is_empty())
            {
                return Err("artifacts fields must be non-empty for ts".to_string());
            }
            if let Some(expected_packages) = ts_packages.as_ref() {
                let mapped_packages = mapping.packages.values().cloned().collect::<BTreeSet<_>>();
                if mapped_packages != *expected_packages {
                    return Err(format!(
                        "ts export packages {} must match manifest export.ts.packages {}",
                        join_set(&mapped_packages),
                        join_set(expected_packages)
                    ));
                }
            }
        }
    }
    if bundle.version.contract.version.trim().is_empty() {
        return Err("version.contract.version is required".to_string());
    }
    if bundle.version.contract.stability.trim().is_empty() {
        return Err("version.contract.stability is required".to_string());
    }
    if bundle.version.semver.major_on.is_empty()
        || bundle.version.semver.minor_on.is_empty()
        || bundle.version.semver.patch_on.is_empty()
    {
        return Err("version.semver rules must all be non-empty".to_string());
    }
    if !bundle.version.compatibility.requires_conformance_pass {
        return Err("compatibility.requires_conformance_pass must be true".to_string());
    }
    if !bundle.version.compatibility.requires_export_manifest_diff {
        return Err("compatibility.requires_export_manifest_diff must be true".to_string());
    }
    if !bundle.version.compatibility.requires_release_notes {
        return Err("compatibility.requires_release_notes must be true".to_string());
    }
    if !bundle.manifest.policy.exclude_internal_workspace_crates
        || !bundle.manifest.policy.require_reproducible_exports
        || !bundle.manifest.policy.require_conformance_vectors
    {
        return Err("contract policy flags must all be true".to_string());
    }
    let workspace_root = bundle
        .root
        .parent()
        .expect("contract root must have a workspace parent");
    if let Some(operations_manifest) = bundle.operations_manifest.as_ref() {
        validate_operations_contract(bundle, operations_manifest, workspace_root)?;
    }
    validate_core_unit_dimension_variant_order(workspace_root)?;
    validate_coverage_policy_parity(workspace_root, &bundle.root)?;
    if resolve_release_contract_path_with_override(workspace_root, release_policy_override.clone())
        .expect("validated release contract path resolution should not fail")
        .is_some()
    {
        validate_release_publish_policy_with_override(
            workspace_root,
            &bundle.root,
            bundle.version.contract.version.as_str(),
            release_policy_override,
        )?;
    }
    Ok(())
}

fn validate_release_publish_policy_with_override(
    workspace_root: &Path,
    contract_root: &Path,
    contract_version: &str,
    release_policy_override: Option<PathBuf>,
) -> Result<(), String> {
    let release = load_release_contract_with_override(
        workspace_root,
        contract_root,
        release_policy_override,
    )?;
    if release.release.version.trim().is_empty() {
        return Err("release.version must not be empty".to_string());
    }
    if release.release.version != contract_version {
        return Err(format!(
            "release.version {} must match contract version {}",
            release.release.version, contract_version
        ));
    }

    let workspace_packages = workspace_package_names(workspace_root)?
        .into_iter()
        .collect::<BTreeSet<_>>();
    let uses_classification = release.uses_classification();
    let public_field = if uses_classification {
        "classification.public"
    } else {
        "publish.crates"
    };
    let internal_field = if uses_classification {
        "classification.internal"
    } else {
        "internal.crates"
    };

    let public_set = collect_unique_set(&release.public_crates(), public_field)?;
    let internal_set = collect_unique_set(&release.internal_crates(), internal_field)?;
    let deferred_set = collect_unique_set(&release.deferred_crates(), "classification.deferred")?;
    let retired_set = collect_unique_set(&release.retired_crates(), "classification.retired")?;
    let yank_only_set =
        collect_unique_set(&release.yank_only_crates(), "classification.yank_only")?;
    let publish_order = &release.publish_order.crates;
    let publish_order_set = collect_unique_set(publish_order, "publish_order.crates")?;

    let class_sets = [
        ("public", &public_set),
        ("internal", &internal_set),
        ("deferred", &deferred_set),
        ("retired", &retired_set),
        ("yank-only", &yank_only_set),
    ];
    for idx in 0..class_sets.len() {
        for other_idx in (idx + 1)..class_sets.len() {
            let overlap = class_sets[idx]
                .1
                .intersection(class_sets[other_idx].1)
                .cloned()
                .collect::<BTreeSet<_>>();
            if !overlap.is_empty() {
                return Err(format!(
                    "release classification overlap is not allowed between {} and {}: {}",
                    class_sets[idx].0,
                    class_sets[other_idx].0,
                    join_set(&overlap)
                ));
            }
        }
    }

    let mut combined = public_set.clone();
    combined.extend(internal_set.iter().cloned());
    combined.extend(deferred_set.iter().cloned());
    combined.extend(retired_set.iter().cloned());
    combined.extend(yank_only_set.iter().cloned());
    if combined != workspace_packages {
        let missing = workspace_packages
            .difference(&combined)
            .cloned()
            .collect::<BTreeSet<_>>();
        let extra = combined
            .difference(&workspace_packages)
            .cloned()
            .collect::<BTreeSet<_>>();
        return Err(format!(
            "release classification sets are missing workspace crates: {}; release classification sets include unknown crates: {}",
            join_set(&missing),
            join_set(&extra)
        ));
    }

    if publish_order_set != public_set {
        let missing = public_set
            .difference(&publish_order_set)
            .cloned()
            .collect::<BTreeSet<_>>();
        let extra = publish_order_set
            .difference(&public_set)
            .cloned()
            .collect::<BTreeSet<_>>();
        return Err(format!(
            "publish_order.crates is missing publish crates: {}; publish_order.crates has non-publish crates: {}",
            join_set(&missing),
            join_set(&extra)
        ));
    }

    let order_index = publish_order
        .iter()
        .enumerate()
        .map(|(idx, name)| (name.clone(), idx))
        .collect::<BTreeMap<_, _>>();
    let dependencies = read_workspace_package_dependencies(workspace_root)
        .expect("workspace package manifests were already parsed");
    for crate_name in &public_set {
        let crate_deps = &dependencies[crate_name];
        let crate_order = order_index[crate_name];
        for dep in crate_deps {
            if !public_set.contains(dep) {
                continue;
            }
            let dep_order = order_index[dep];
            if dep_order >= crate_order {
                return Err(format!(
                    "publish order must place dependency {} before {}",
                    dep, crate_name
                ));
            }
        }
    }

    let publish_configs = workspace_package_publish_configs(workspace_root)
        .expect("workspace publish configs are stable");
    for crate_name in &public_set {
        let publish = publish_configs[crate_name].as_ref();
        if !publish_config_is_public(publish) {
            return Err(format!(
                "public crate {} must set publish = [\"crates-io\"]",
                crate_name
            ));
        }
    }
    for crate_name in internal_set
        .iter()
        .chain(deferred_set.iter())
        .chain(retired_set.iter())
        .chain(yank_only_set.iter())
    {
        let publish = publish_configs[crate_name].as_ref();
        if !publish_config_is_non_public(publish) {
            return Err(format!(
                "non-public crate {} must set publish = false",
                crate_name
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
pub fn synthetic_release_policy_for_workspace(workspace_root: &Path) -> Result<String, String> {
    let bundle = load_contract_bundle(workspace_root)?;
    let publish_configs = workspace_package_publish_configs(workspace_root)?;
    let dependencies = read_workspace_package_dependencies(workspace_root)?;

    let mut public = BTreeSet::new();
    let mut internal = BTreeSet::new();
    for (crate_name, publish) in &publish_configs {
        if publish_config_is_public(publish.as_ref()) {
            public.insert(crate_name.clone());
        } else {
            internal.insert(crate_name.clone());
        }
    }

    let mut in_degree = BTreeMap::new();
    let mut dependents = BTreeMap::<String, BTreeSet<String>>::new();
    for crate_name in &public {
        in_degree.insert(crate_name.clone(), 0usize);
        dependents.insert(crate_name.clone(), BTreeSet::new());
    }
    for crate_name in &public {
        for dep in &dependencies[crate_name] {
            if !public.contains(dep) {
                continue;
            }
            *in_degree
                .get_mut(crate_name)
                .expect("public crate present in indegree map") += 1;
            dependents
                .get_mut(dep)
                .expect("public dependency present in dependents map")
                .insert(crate_name.clone());
        }
    }

    let mut ready = in_degree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(crate_name, _)| crate_name.clone())
        .collect::<BTreeSet<_>>();
    let mut publish_order = Vec::new();
    while let Some(crate_name) = ready.pop_first() {
        publish_order.push(crate_name.clone());
        for dependent in dependents[&crate_name].clone() {
            let degree = in_degree
                .get_mut(&dependent)
                .expect("dependent crate present in indegree map");
            *degree -= 1;
            if *degree == 0 {
                ready.insert(dependent);
            }
        }
    }
    if publish_order.len() != public.len() {
        return Err("public crate dependency graph contains a cycle".to_string());
    }

    let public = public.into_iter().collect::<Vec<_>>();
    let internal = internal.into_iter().collect::<Vec<_>>();
    Ok(format!(
        "[release]\nversion = \"{}\"\n\n[classification]\npublic = {}\ninternal = {}\ndeferred = []\nretired = []\nyank_only = []\n\n[publish_order]\ncrates = {}\n",
        bundle.version.contract.version,
        toml_inline_array(&public),
        toml_inline_array(&internal),
        toml_inline_array(&publish_order),
    ))
}

#[cfg(test)]
fn toml_inline_array(values: &[String]) -> String {
    let joined = values
        .iter()
        .map(|value| format!("\"{value}\""))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{joined}]")
}

pub fn load_contract_bundle(workspace_root: &Path) -> Result<ContractBundle, String> {
    let root = contract_root(workspace_root);
    let manifest = parse_toml::<ContractManifest>(&root.join("manifest.toml"))?;
    let version = parse_toml::<VersionPolicy>(&root.join("version.toml"))?;
    let operations_manifest_path = root.join("operations.toml");
    let operations_manifest = if operations_manifest_path.is_file() {
        Some(parse_toml::<OperationsContractManifest>(
            &operations_manifest_path,
        )?)
    } else {
        None
    };
    let exports_dir = root.join("exports");
    let mut exports = Vec::new();
    let read_dir = match fs::read_dir(&exports_dir) {
        Ok(read_dir) => read_dir,
        Err(e) => return Err(format!("read dir {}: {e}", exports_dir.display())),
    };
    let mut entries = read_dir.filter_map(Result::ok).collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
            continue;
        }
        exports.push(parse_toml::<ExportMapping>(&path)?);
    }
    let sdk_exports = load_sdk_exports(&root)?;
    Ok(ContractBundle {
        root,
        manifest,
        version,
        exports,
        operations_manifest,
        sdk_exports,
    })
}

fn load_sdk_exports(contract_root: &Path) -> Result<Vec<SdkExportMapping>, String> {
    let exports_dir = contract_root.join("sdk-exports");
    if !exports_dir.exists() {
        return Ok(Vec::new());
    }
    let read_dir = match fs::read_dir(&exports_dir) {
        Ok(read_dir) => read_dir,
        Err(e) => return Err(format!("read dir {}: {e}", exports_dir.display())),
    };
    let mut entries = read_dir.filter_map(Result::ok).collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.file_name());
    let mut mappings = Vec::new();
    for entry in entries {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
            continue;
        }
        mappings.push(parse_toml::<SdkExportMapping>(&path)?);
    }
    Ok(mappings)
}

pub fn validate_contract_bundle(bundle: &ContractBundle) -> Result<(), String> {
    if bundle.manifest.contract.name.trim().is_empty() {
        return Err("contract name is required".to_string());
    }
    if bundle.manifest.contract.version.trim().is_empty() {
        return Err("contract version is required".to_string());
    }
    if bundle.manifest.contract.source.trim().is_empty() {
        return Err("contract source is required".to_string());
    }
    if bundle.manifest.surface.model_crates.is_empty() {
        return Err("contract surface.model_crates must not be empty".to_string());
    }
    if bundle.manifest.surface.algorithm_crates.is_empty() {
        return Err("contract surface.algorithm_crates must not be empty".to_string());
    }
    if bundle.manifest.surface.wasm_crates.is_empty() {
        return Err("contract surface.wasm_crates must not be empty".to_string());
    }
    if bundle.exports.is_empty() {
        return Err("at least one language export mapping is required".to_string());
    }
    let ts_packages = ts_curated_package_set(bundle)?;
    for mapping in &bundle.exports {
        if mapping.language.id.trim().is_empty() {
            return Err("language.id is required".to_string());
        }
        if mapping.language.repository.trim().is_empty() {
            return Err(format!(
                "language.repository is required for {}",
                mapping.language.id
            ));
        }
        if mapping.packages.is_empty() {
            return Err(format!(
                "packages map is required for {}",
                mapping.language.id
            ));
        }
        if mapping.language.id == "ts" {
            let artifacts = match mapping.artifacts.as_ref() {
                Some(artifacts) => artifacts,
                None => return Err("artifacts map is required for ts".to_string()),
            };
            if artifacts
                .models_dir
                .as_deref()
                .is_none_or(|value| value.trim().is_empty())
                || artifacts
                    .constants_dir
                    .as_deref()
                    .is_none_or(|value| value.trim().is_empty())
                || artifacts
                    .wasm_dist_dir
                    .as_deref()
                    .is_none_or(|value| value.trim().is_empty())
                || artifacts
                    .manifest_file
                    .as_deref()
                    .is_none_or(|value| value.trim().is_empty())
            {
                return Err("artifacts fields must be non-empty for ts".to_string());
            }
            if let Some(expected_packages) = ts_packages.as_ref() {
                let mapped_packages = mapping.packages.values().cloned().collect::<BTreeSet<_>>();
                if mapped_packages != *expected_packages {
                    return Err(format!(
                        "ts export packages {} must match manifest export.ts.packages {}",
                        join_set(&mapped_packages),
                        join_set(expected_packages)
                    ));
                }
            }
        }
    }
    if bundle.version.contract.version.trim().is_empty() {
        return Err("version.contract.version is required".to_string());
    }
    if bundle.version.contract.stability.trim().is_empty() {
        return Err("version.contract.stability is required".to_string());
    }
    if bundle.version.semver.major_on.is_empty()
        || bundle.version.semver.minor_on.is_empty()
        || bundle.version.semver.patch_on.is_empty()
    {
        return Err("version.semver rules must all be non-empty".to_string());
    }
    if !bundle.version.compatibility.requires_conformance_pass {
        return Err("compatibility.requires_conformance_pass must be true".to_string());
    }
    if !bundle.version.compatibility.requires_export_manifest_diff {
        return Err("compatibility.requires_export_manifest_diff must be true".to_string());
    }
    if !bundle.version.compatibility.requires_release_notes {
        return Err("compatibility.requires_release_notes must be true".to_string());
    }
    if !bundle.manifest.policy.exclude_internal_workspace_crates
        || !bundle.manifest.policy.require_reproducible_exports
        || !bundle.manifest.policy.require_conformance_vectors
    {
        return Err("contract policy flags must all be true".to_string());
    }
    let workspace_root = bundle
        .root
        .parent()
        .expect("contract root must have a workspace parent");
    if let Some(operations_manifest) = bundle.operations_manifest.as_ref() {
        validate_operations_contract(bundle, operations_manifest, workspace_root)?;
    }
    validate_core_unit_dimension_variant_order(workspace_root)?;
    validate_coverage_policy_parity(workspace_root, &bundle.root)?;
    if resolve_release_contract_path(workspace_root)
        .expect("validated release contract path resolution should not fail")
        .is_some()
    {
        validate_release_publish_policy(
            workspace_root,
            &bundle.root,
            bundle.version.contract.version.as_str(),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn workspace_root() -> PathBuf {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        manifest_dir
            .join("../..")
            .canonicalize()
            .expect("canonical workspace root")
    }

    fn temp_root(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("radroots_xtask_{prefix}_{nanos}"));
        fs::create_dir_all(&root).expect("create temp root");
        root
    }

    fn write_file(path: &Path, content: &str) {
        let _ = fs::create_dir_all(path.parent().unwrap_or(Path::new("")));
        fs::write(path, content).expect("write file");
    }

    fn strict_thresholds() -> CoverageThresholds {
        CoverageThresholds {
            fail_under_exec_lines: 100.0,
            fail_under_functions: 100.0,
            fail_under_regions: 100.0,
            fail_under_branches: 100.0,
            require_branches: true,
        }
    }

    fn create_synthetic_workspace(prefix: &str) -> PathBuf {
        let root = temp_root(prefix);
        write_file(
            &root.join("Cargo.toml"),
            r#"[workspace]
members = ["crates/a", "crates/b"]
resolver = "2"
"#,
        );
        write_file(
            &root.join("crates").join("a").join("Cargo.toml"),
            r#"[package]
name = "radroots_a"
publish = ["crates-io"]
version = "0.1.0"
edition = "2024"
description = "crate a"
repository = "https://example.com/a"
homepage = "https://example.com/a"
documentation = "https://docs.example.com/a"
readme = "README"
"#,
        );
        write_file(
            &root.join("crates").join("b").join("Cargo.toml"),
            r#"[package]
name = "radroots_b"
version = "0.1.0"
edition = "2024"
publish = false
"#,
        );
        write_file(
            &root.join("crates").join("core").join("src").join("unit.rs"),
            r#"pub enum RadrootsCoreUnitDimension {
    Count,
    Mass,
    Volume,
}
"#,
        );

        write_file(
            &root.join("spec").join("manifest.toml"),
            r#"[contract]
name = "radroots_contract"
version = "1.0.0"
source = "synthetic"

[surface]
model_crates = ["radroots_a"]
algorithm_crates = ["radroots_b"]
wasm_crates = ["radroots_a_wasm"]

[export.ts]
packages = ["@radroots/sdk"]

[policy]
exclude_internal_workspace_crates = true
require_reproducible_exports = true
require_conformance_vectors = true
"#,
        );
        write_file(
            &root.join("spec").join("version.toml"),
            r#"[contract]
version = "1.0.0"
stability = "alpha"

[semver]
major_on = ["breaking"]
minor_on = ["feature"]
patch_on = ["fix"]

[compatibility]
requires_conformance_pass = true
requires_export_manifest_diff = true
requires_release_notes = true
"#,
        );
        write_file(
            &root.join("spec").join("exports").join("ts.toml"),
            r#"[language]
id = "ts"
repository = "sdk-typescript"

[packages]
"radroots_a" = "@radroots/sdk"

[artifacts]
models_dir = "src/generated"
constants_dir = "src/generated"
wasm_dist_dir = "dist"
manifest_file = "export-manifest.json"
"#,
        );
        write_file(
            &root.join("policy").join("coverage").join("policy.toml"),
            r#"[gate]
fail_under_exec_lines = 100.0
fail_under_functions = 100.0
fail_under_regions = 100.0
fail_under_branches = 100.0
require_branches = true

[required]
crates = ["radroots_a"]
"#,
        );
        write_file(
            &root_release_policy_path(&root),
            r#"[release]
version = "1.0.0"

[publish]
crates = ["radroots_a"]

[internal]
crates = ["radroots_b"]

[publish_order]
crates = ["radroots_a"]
"#,
        );
        write_file(
            &root
                .join("target")
                .join("coverage")
                .join("coverage-refresh.tsv"),
            "crate\tstatus\texec\tfunc\tbranch\tregion\treport\nradroots_a\tpass\t100.0\t100.0\t100.0\t100.0\tfile\nradroots_b\tpass\t100.0\t100.0\t100.0\t100.0\tfile\n",
        );
        root
    }

    fn add_operation_contract_files(root: &Path) {
        write_file(
            &root.join("spec").join("operations.toml"),
            r#"[contract]
name = "radroots_contract"
version = "1.0.0"
source = "synthetic"

[public]
domains = ["profile", "farm", "listing", "trade"]

[shared_types]
public = [
  "WireEventParts",
  "UnsignedEventDraft",
  "RadrootsNostrEvent",
  "RadrootsNostrEventRef",
  "RadrootsNostrEventPtr",
  "RadrootsTradeListingAddress",
  "RadrootsProfile",
  "RadrootsFarm",
  "RadrootsListing",
]

[errors]
classes = ["encode_error", "parse_error", "validation_error", "address_error"]

[implementation_provenance]
model_crates = ["radroots_a"]
algorithm_crates = ["radroots_b"]
wasm_crates = ["radroots_a_wasm"]

[operations.profile_build_draft]
domain = "profile"
id = "profile.build_draft"
stability = "beta"
inputs = ["RadrootsProfile", "RadrootsProfileType?"]
outputs = ["WireEventParts"]
error_class = "encode_error"
deterministic = true
signing = "native"
transport = "native"

[operations.profile_build_draft.implementation]
rust_modules = ["crates/core/src/unit.rs"]
rust_types = ["radroots_events::profile::RadrootsProfile"]

[operations.profile_build_draft.conformance]
vector = "spec/conformance/vectors/profile/build_draft.v1.json"

[operations.listing_build_draft]
domain = "listing"
id = "listing.build_draft"
stability = "beta"
inputs = ["RadrootsListing"]
outputs = ["WireEventParts"]
error_class = "encode_error"
deterministic = true
signing = "native"
transport = "native"

[operations.listing_build_draft.implementation]
rust_modules = ["crates/core/src/unit.rs"]
rust_types = ["radroots_events::listing::RadrootsListing"]

[operations.listing_build_draft.conformance]
vector = "spec/conformance/vectors/listing/build_draft.v1.json"
"#,
        );
        write_file(
            &root.join("spec").join("sdk-exports").join("ts.toml"),
            r#"[language]
id = "ts"
repository = "sdk-typescript"

[sdk]
package = "@radroots/sdk"
module_format = "esm"
deterministic_codec = "wasm"
signing = "native"
networking = "native"

[operations]
"profile.build_draft" = "profile.buildDraft"
"listing.build_draft" = "listing.buildDraft"

[shared_types]
"WireEventParts" = "WireEventParts"
"UnsignedEventDraft" = "UnsignedEventDraft"
"RadrootsNostrEvent" = "RadrootsNostrEvent"
"RadrootsListing" = "RadrootsListing"

[artifacts]
models_dir = "src/generated"
runtime_dir = "src/runtime"
wasm_dist_dir = "dist"
manifest_file = "export-manifest.json"
"#,
        );
        write_file(
            &root
                .join("spec")
                .join("conformance")
                .join("schema")
                .join("vector.schema.json"),
            r#"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "$id": "https://radroots.org/sdk/conformance/vector.schema.json",
  "title": "radroots sdk conformance vector",
  "type": "object",
  "required": ["suite", "contract_version", "vectors"],
  "properties": {
    "suite": {
      "type": "string",
      "minLength": 1
    },
    "contract_version": {
      "type": "string",
      "pattern": "^[0-9]+\\.[0-9]+\\.[0-9]+$"
    },
    "vectors": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["id", "kind", "input", "expected"],
        "properties": {
          "id": {
            "type": "string",
            "minLength": 1
          },
          "kind": {
            "type": "string",
            "minLength": 1
          },
          "input": {},
          "expected": {}
        },
        "additionalProperties": false
      }
    }
  },
  "additionalProperties": false
}
"#,
        );
        write_file(
            &root
                .join("spec")
                .join("conformance")
                .join("vectors")
                .join("profile")
                .join("build_draft.v1.json"),
            r#"{
  "suite": "profile",
  "contract_version": "1.0.0",
  "vectors": [
    {
      "id": "profile_build_draft_minimal_001",
      "kind": "profile.build_draft",
      "input": {},
      "expected": {}
    }
  ]
}
"#,
        );
        write_file(
            &root
                .join("spec")
                .join("conformance")
                .join("vectors")
                .join("listing")
                .join("build_draft.v1.json"),
            r#"{
  "suite": "listing",
  "contract_version": "1.0.0",
  "vectors": [
    {
      "id": "listing_build_draft_minimal_001",
      "kind": "listing.build_draft",
      "input": {},
      "expected": {}
    }
  ]
}
"#,
        );
    }

    fn write_root_release_policy(root: &Path, raw: &str) {
        write_file(&root.join(ROOT_RELEASE_POLICY_RELATIVE), raw);
    }

    fn configure_root_release_policy_workspace(root: &Path) {
        write_file(
            &root.join("Cargo.toml"),
            r#"[workspace]
members = ["crates/a", "crates/b", "crates/c", "crates/d", "crates/e"]
resolver = "2"
"#,
        );
        for crate_name in ["c", "d", "e"] {
            write_file(
                &root.join("crates").join(crate_name).join("Cargo.toml"),
                &format!(
                    r#"[package]
name = "radroots_{crate_name}"
version = "0.1.0"
edition = "2024"
publish = false
"#
                ),
            );
        }
        write_file(
            &root.join("policy").join("coverage").join("policy.toml"),
            r#"[gate]
fail_under_exec_lines = 100.0
fail_under_functions = 100.0
fail_under_regions = 100.0
fail_under_branches = 100.0
require_branches = true

[required]
crates = ["radroots_a"]
"#,
        );
        write_file(
            &root
                .join("target")
                .join("coverage")
                .join("coverage-refresh.tsv"),
            "crate\tstatus\texec\tfunc\tbranch\tregion\treport\nradroots_a\tpass\t100.0\t100.0\t100.0\t100.0\tfile\nradroots_b\tpass\t100.0\t100.0\t100.0\t100.0\tfile\nradroots_c\tpass\t100.0\t100.0\t100.0\t100.0\tfile\nradroots_d\tpass\t100.0\t100.0\t100.0\t100.0\tfile\nradroots_e\tpass\t100.0\t100.0\t100.0\t100.0\tfile\n",
        );
        let _ = fs::remove_file(root_release_policy_path(&root));
    }

    #[test]
    fn validate_current_contract_bundle() {
        let root = workspace_root();
        let bundle = load_contract_bundle(&root).expect("load contract");
        validate_contract_bundle(&bundle).expect("validate contract");
    }

    #[test]
    fn validate_current_canonical_event_boundary() {
        let root = workspace_root();
        validate_canonical_event_boundary(&root).expect("validate canonical event boundary");
    }

    #[test]
    fn canonical_event_boundary_reports_row_drift() {
        let root = workspace_root();
        let matrix_path =
            resolve_event_boundary_matrix_path_with_override(&root, None).expect("matrix path");
        let raw = fs::read_to_string(&matrix_path).expect("read matrix");
        let drifted = raw.replacen(
            "| message | 14 | RadrootsMessage |",
            "| message | 999 | RadrootsMessage |",
            1,
        );
        let temp = temp_root("event_boundary_drift");
        let override_path = temp.join("spec-coverage.md");
        write_file(&override_path, &drifted);

        let err = validate_canonical_event_boundary_with_override(&root, Some(override_path))
            .expect_err("message kind drift should fail");
        assert!(err.contains("message kind drift"));

        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn validate_synthetic_operation_contract_bundle() {
        let root = create_synthetic_workspace("operation_contract_bundle");
        add_operation_contract_files(&root);
        let bundle = load_contract_bundle(&root).expect("load contract");
        validate_contract_bundle(&bundle).expect("validate contract");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn ts_export_mapping_covers_model_and_wasm_surface() {
        let root = workspace_root();
        let bundle = load_contract_bundle(&root).expect("load contract");
        let ts = bundle
            .exports
            .iter()
            .find(|mapping| mapping.language.id == "ts")
            .expect("ts export mapping");
        let expected = bundle
            .manifest
            .surface
            .model_crates
            .iter()
            .chain(bundle.manifest.surface.wasm_crates.iter())
            .cloned()
            .collect::<BTreeSet<_>>();
        let mapped = ts.packages.keys().cloned().collect::<BTreeSet<_>>();
        assert_eq!(mapped, expected);
    }

    #[test]
    fn exports_follow_package_scope_rules() {
        let root = workspace_root();
        let bundle = load_contract_bundle(&root).expect("load contract");
        for mapping in &bundle.exports {
            if mapping.language.id == "ts" {
                let packages = mapping.packages.values().cloned().collect::<BTreeSet<_>>();
                assert_eq!(
                    packages,
                    ["@radroots/sdk".to_string()].into_iter().collect()
                );
            } else {
                for package in mapping.packages.values() {
                    assert!(!package.trim().is_empty());
                }
            }
        }
    }

    #[test]
    fn non_ts_exports_only_include_model_surface() {
        let root = workspace_root();
        let bundle = load_contract_bundle(&root).expect("load contract");
        let expected = bundle
            .manifest
            .surface
            .model_crates
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        for mapping in &bundle.exports {
            if mapping.language.id == "ts" {
                continue;
            }
            let mapped = mapping.packages.keys().cloned().collect::<BTreeSet<_>>();
            assert_eq!(mapped, expected);
        }
    }

    #[test]
    fn parses_enum_variants_in_declared_order() {
        let source = r#"
pub enum RadrootsCoreUnitDimension {
    Count,
    Mass,
    Volume,
}
"#;
        let enum_body = extract_enum_body(source, "RadrootsCoreUnitDimension").expect("enum body");
        let variants = parse_enum_variants(enum_body);
        assert_eq!(variants, vec!["Count", "Mass", "Volume"]);
    }

    #[test]
    fn fails_when_enum_order_does_not_match_contract() {
        let source = r#"
pub enum RadrootsCoreUnitDimension {
    Mass,
    Count,
    Volume,
}
"#;
        let enum_body = extract_enum_body(source, "RadrootsCoreUnitDimension").expect("enum body");
        let variants = parse_enum_variants(enum_body);
        let expected = CORE_UNIT_DIMENSION_ORDER
            .iter()
            .map(|item| (*item).to_string())
            .collect::<Vec<_>>();
        assert_ne!(variants, expected);
    }

    #[test]
    fn coverage_policy_matches_public_release_crates() {
        let root = workspace_root();
        let release =
            load_release_contract(&root, &root.join("spec")).expect("root release policy");
        let public_names = release.public_crates().into_iter().collect::<BTreeSet<_>>();
        let policy = load_coverage_policy(&root.join("spec")).expect("coverage policy");
        let required_names = policy
            .required_crates()
            .expect("required crates")
            .into_iter()
            .collect::<BTreeSet<_>>();
        assert_eq!(public_names, required_names);
    }

    #[test]
    fn coverage_required_crates_match_policy_required_status() {
        let root = workspace_root();
        let contract_root = root.join("spec");
        let policy = load_coverage_policy(&contract_root).expect("coverage policy");
        let required = CoverageRequiredFile {
            required: CoverageRequiredSection {
                crates: policy.required_crates().expect("coverage required"),
            },
        };
        let required_names = required
            .required
            .crates
            .into_iter()
            .collect::<BTreeSet<_>>();
        let policy_required = policy
            .required_crates()
            .expect("policy required crates")
            .into_iter()
            .collect::<BTreeSet<_>>();
        assert_eq!(required_names, policy_required);
    }

    #[test]
    fn coverage_policy_required_crates_report_policy_errors() {
        let missing_root = temp_root("load_coverage_required_missing_policy");
        let missing_err =
            load_coverage_policy(&missing_root).expect_err("missing policy should fail");
        assert!(missing_err.contains("policy.toml"));
        let _ = fs::remove_dir_all(&missing_root);

        let duplicate_root =
            create_synthetic_workspace("load_coverage_required_duplicate_required");
        let contract_root = duplicate_root.join("spec");
        let coverage_root = coverage_root(&contract_root);
        write_file(
            &coverage_root.join("policy.toml"),
            "[gate]\nfail_under_exec_lines = 100.0\nfail_under_functions = 100.0\nfail_under_regions = 100.0\nfail_under_branches = 100.0\nrequire_branches = true\n\n[required]\ncrates = [\"radroots_a\", \"radroots_a\"]\n",
        );
        let duplicate_err =
            load_coverage_policy(&contract_root).expect_err("duplicate required crates");
        assert!(duplicate_err.contains("duplicate crate"));
        let _ = fs::remove_dir_all(&duplicate_root);
    }

    #[test]
    fn package_field_configured_accepts_workspace_table() {
        let mut package = toml::value::Table::new();
        let mut repository = toml::value::Table::new();
        repository.insert("workspace".to_string(), toml::Value::Boolean(true));
        package.insert("repository".to_string(), toml::Value::Table(repository));
        assert!(package_field_configured(&package, "repository"));
    }

    #[test]
    fn validate_required_coverage_summary_enforces_strict_threshold() {
        let root = temp_root("coverage_summary");
        let coverage_dir = root.join("target").join("coverage");
        fs::create_dir_all(&coverage_dir).expect("create coverage dir");
        fs::write(
            coverage_dir.join("coverage-refresh.tsv"),
            "crate\tstatus\texec\tfunc\tbranch\tregion\treport\nradroots_core\tpass\t100.0\t100.0\t100.0\t100.0\tfile\n",
        )
        .expect("write coverage file");
        let required = ["radroots_core".to_string()]
            .into_iter()
            .collect::<BTreeSet<_>>();
        validate_required_coverage_summary(&root, &required, strict_thresholds())
            .expect("coverage summary");

        fs::write(
            coverage_dir.join("coverage-refresh.tsv"),
            "crate\tstatus\texec\tfunc\tbranch\tregion\treport\nradroots_core\tpass\t100.0\t99.9\t100.0\t100.0\tfile\n",
        )
        .expect("write function coverage file");
        let func_err = validate_required_coverage_summary(&root, &required, strict_thresholds())
            .expect_err("function coverage below 100");
        assert!(func_err.contains("must satisfy coverage policy"));

        fs::write(
            coverage_dir.join("coverage-refresh.tsv"),
            "crate\tstatus\texec\tfunc\tbranch\tregion\treport\nradroots_core\tpass\t100.0\t100.0\t99.9\t100.0\tfile\n",
        )
        .expect("write branch coverage file");
        let branch_err = validate_required_coverage_summary(&root, &required, strict_thresholds())
            .expect_err("branch coverage below 100");
        assert!(branch_err.contains("must satisfy coverage policy"));

        fs::write(
            coverage_dir.join("coverage-refresh.tsv"),
            "crate\tstatus\texec\tfunc\tbranch\tregion\treport\nradroots_core\tpass\t100.0\t100.0\t100.0\t99.9\tfile\n",
        )
        .expect("write region coverage file");
        let region_err = validate_required_coverage_summary(&root, &required, strict_thresholds())
            .expect_err("region coverage below 100");
        assert!(region_err.contains("must satisfy coverage policy"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn validate_required_coverage_summary_with_policy_honors_scope_override() {
        let root = temp_root("coverage_summary_override");
        let coverage_dir = root.join("target").join("coverage");
        fs::create_dir_all(&coverage_dir).expect("create coverage dir");
        fs::write(
            coverage_dir.join("coverage-refresh.tsv"),
            "crate\tstatus\texec\tfunc\tbranch\tregion\treport\nradroots_events_codec\tpass\t100.0\t100.0\t100.0\t99.946385\tfile\n",
        )
        .expect("write coverage file");
        let policy_dir = root.join("policy").join("coverage");
        fs::create_dir_all(&policy_dir).expect("create policy dir");
        fs::write(
            policy_dir.join("policy.toml"),
            "[gate]\nfail_under_exec_lines = 100.0\nfail_under_functions = 100.0\nfail_under_regions = 100.0\nfail_under_branches = 100.0\nrequire_branches = true\n\n[overrides.radroots_events_codec]\nfail_under_exec_lines = 100.0\nfail_under_functions = 100.0\nfail_under_regions = 99.946\nfail_under_branches = 100.0\ntemporary = true\nreason = \"publish 0.1.0-alpha.2 temporary coverage override\"\n\n[required]\ncrates = [\"radroots_events_codec\"]\n",
        )
        .expect("write coverage policy");
        let required = ["radroots_events_codec".to_string()]
            .into_iter()
            .collect::<BTreeSet<_>>();
        let policy = read_coverage_policy(&policy_dir.join("policy.toml"))
            .expect("parse override coverage policy");
        validate_required_coverage_summary_with_policy(&root, &required, &policy)
            .expect("coverage summary should honor override");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn validate_publish_package_metadata_requires_description() {
        let root = temp_root("publish_metadata");
        fs::create_dir_all(root.join("crates").join("a")).expect("create crate dir");
        fs::write(
            root.join("Cargo.toml"),
            r#"[workspace]
members = ["crates/a"]
"#,
        )
        .expect("write workspace manifest");
        fs::write(
            root.join("crates").join("a").join("Cargo.toml"),
            r#"[package]
name = "radroots_a"
version = "0.1.0"
edition = "2024"
repository = { workspace = true }
homepage = { workspace = true }
documentation = "https://docs.rs/radroots_a"
readme = { workspace = true }
"#,
        )
        .expect("write package manifest");
        let publish = ["radroots_a".to_string()]
            .into_iter()
            .collect::<BTreeSet<_>>();
        let err =
            validate_publish_package_metadata(&root, &publish).expect_err("missing description");
        assert!(err.contains("package.description"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn synthetic_workspace_validates_contract_and_release_preflight() {
        let root = create_synthetic_workspace("synthetic_valid");
        let bundle = load_contract_bundle(&root).expect("load synthetic bundle");
        validate_contract_bundle(&bundle).expect("validate synthetic bundle");
        validate_release_preflight(&root).expect("validate synthetic preflight");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn helper_functions_cover_error_paths() {
        let empty = collect_unique_set(&["".to_string()], "field").expect_err("empty value");
        assert!(empty.contains("field contains an empty crate name"));
        let duplicate = collect_unique_set(&["a".to_string(), "a".to_string()], "field")
            .expect_err("duplicate value");
        assert!(duplicate.contains("field has duplicate crate a"));

        let values = ["b".to_string(), "a".to_string()];
        let set = collect_unique_set(&values, "field").expect("unique values");
        assert_eq!(join_set(&set), "a, b".to_string());

        assert!(package_publish_enabled(None));
        assert!(package_publish_enabled(Some(&PackagePublish::Bool(true))));
        assert!(!package_publish_enabled(Some(&PackagePublish::Bool(false))));
        assert!(package_publish_enabled(Some(&PackagePublish::Registries(
            vec!["crates-io".to_string(),]
        ))));
        assert!(!package_publish_enabled(Some(&PackagePublish::Registries(
            Vec::new()
        ))));

        let mut package = toml::value::Table::new();
        package.insert("description".to_string(), toml::Value::Integer(42));
        assert!(!package_field_configured(&package, "description"));

        assert!(!publish_config_is_public(None));
        assert!(!publish_config_is_public(Some(&PackagePublish::Bool(true))));
        assert!(publish_config_is_public(Some(&PackagePublish::Registries(
            vec!["crates-io".to_string(),]
        ))));
        assert!(!publish_config_is_public(Some(
            &PackagePublish::Registries(vec!["crates-io".to_string(), "mirror".to_string(),])
        )));
        assert!(!publish_config_is_public(Some(
            &PackagePublish::Registries(vec!["mirror".to_string(),])
        )));

        assert!(!publish_config_is_non_public(None));
        assert!(!publish_config_is_non_public(Some(&PackagePublish::Bool(
            true
        ))));
        assert!(publish_config_is_non_public(Some(&PackagePublish::Bool(
            false
        ))));
        assert!(!publish_config_is_non_public(Some(
            &PackagePublish::Registries(vec!["crates-io".to_string(),])
        )));
    }

    #[test]
    fn release_contract_helpers_cover_classification_and_env_override_paths() {
        let release = ReleaseSection {
            version: "1.0.0".to_string(),
        };
        let empty_order = ReleaseCrateSet { crates: Vec::new() };

        let legacy = ReleaseContractFile {
            release: ReleaseSection {
                version: release.version.clone(),
            },
            classification: ReleaseClassification::default(),
            publish: Some(ReleaseCrateSet {
                crates: vec!["radroots_public".to_string()],
            }),
            internal: Some(ReleaseCrateSet {
                crates: vec!["radroots_internal".to_string()],
            }),
            publish_order: ReleaseCrateSet {
                crates: empty_order.crates.clone(),
            },
        };
        assert!(!legacy.uses_classification());
        assert_eq!(legacy.public_crates(), vec!["radroots_public".to_string()]);
        assert_eq!(
            legacy.internal_crates(),
            vec!["radroots_internal".to_string()]
        );

        let empty_legacy = ReleaseContractFile {
            release: ReleaseSection {
                version: release.version.clone(),
            },
            classification: ReleaseClassification::default(),
            publish: None,
            internal: None,
            publish_order: ReleaseCrateSet {
                crates: empty_order.crates.clone(),
            },
        };
        assert!(!empty_legacy.uses_classification());
        assert_eq!(empty_legacy.public_crates(), Vec::<String>::new());
        assert_eq!(empty_legacy.internal_crates(), Vec::<String>::new());

        let internal = ReleaseContractFile {
            release: ReleaseSection {
                version: release.version.clone(),
            },
            classification: ReleaseClassification {
                internal: vec!["radroots_internal_only".to_string()],
                ..ReleaseClassification::default()
            },
            publish: None,
            internal: None,
            publish_order: ReleaseCrateSet {
                crates: empty_order.crates.clone(),
            },
        };
        assert!(internal.uses_classification());

        let deferred = ReleaseContractFile {
            release: ReleaseSection {
                version: release.version.clone(),
            },
            classification: ReleaseClassification {
                deferred: vec!["radroots_deferred".to_string()],
                ..ReleaseClassification::default()
            },
            publish: None,
            internal: None,
            publish_order: ReleaseCrateSet {
                crates: empty_order.crates.clone(),
            },
        };
        assert!(deferred.uses_classification());
        assert_eq!(
            deferred.deferred_crates(),
            vec!["radroots_deferred".to_string()]
        );

        let retired = ReleaseContractFile {
            release: ReleaseSection {
                version: release.version.clone(),
            },
            classification: ReleaseClassification {
                retired: vec!["radroots_retired".to_string()],
                ..ReleaseClassification::default()
            },
            publish: None,
            internal: None,
            publish_order: ReleaseCrateSet {
                crates: empty_order.crates.clone(),
            },
        };
        assert!(retired.uses_classification());
        assert_eq!(
            retired.retired_crates(),
            vec!["radroots_retired".to_string()]
        );

        let yank_only = ReleaseContractFile {
            release,
            classification: ReleaseClassification {
                yank_only: vec!["radroots_yank_only".to_string()],
                ..ReleaseClassification::default()
            },
            publish: None,
            internal: None,
            publish_order: empty_order,
        };
        assert!(yank_only.uses_classification());
        assert_eq!(
            yank_only.yank_only_crates(),
            vec!["radroots_yank_only".to_string()]
        );

        let root = create_synthetic_workspace("release_contract_env_override");
        let policy_path = root_release_policy_path(&root);
        let resolved =
            resolve_release_contract_path_with_override(&root, Some(policy_path.clone()))
                .expect("existing override policy should resolve");
        assert_eq!(resolved, Some(policy_path));

        let missing_policy = root.join("missing-release-policy.toml");
        let err = resolve_release_contract_path_with_override(&root, Some(missing_policy.clone()))
            .expect_err("missing env policy should fail");
        assert!(err.contains(RELEASE_POLICY_ENV));
        assert!(err.contains("missing release policy file"));
        assert!(err.contains(&missing_policy.display().to_string()));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn workspace_package_manifests_reject_duplicate_package_names() {
        let root = temp_root("workspace_manifest_duplicates");
        write_file(
            &root.join("Cargo.toml"),
            r#"[workspace]
members = ["crates/a", "crates/b"]
"#,
        );
        let package_manifest =
            "[package]\nname = \"duplicate\"\nversion = \"0.1.0\"\nedition = \"2024\"\n";
        write_file(
            &root.join("crates").join("a").join("Cargo.toml"),
            package_manifest,
        );
        write_file(
            &root.join("crates").join("b").join("Cargo.toml"),
            package_manifest,
        );
        let err = workspace_package_manifests(&root)
            .expect_err("duplicate package names in manifest map");
        assert!(err.contains("duplicate workspace package name in manifest map"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn coverage_refresh_parsing_and_summary_errors_are_reported() {
        let root = temp_root("coverage_refresh_errors");
        let coverage_dir = root.join("target").join("coverage");
        fs::create_dir_all(&coverage_dir).expect("create coverage dir");

        write_file(
            &coverage_dir.join("coverage-refresh.tsv"),
            "crate\tstatus\texec\tfunc\tbranch\tregion\treport\nbad-row\n",
        );
        let bad_row = load_coverage_refresh_rows(&root).expect_err("invalid coverage row");
        assert!(bad_row.contains("at least 6 columns"));

        write_file(
            &coverage_dir.join("coverage-refresh.tsv"),
            "crate\tstatus\texec\tfunc\tbranch\tregion\treport\nradroots_a\tpass\tnot-a-number\t100\t100\t100\tfile\n",
        );
        let bad_percent = load_coverage_refresh_rows(&root).expect_err("invalid coverage percent");
        assert!(bad_percent.contains("parse exec"));

        write_file(
            &coverage_dir.join("coverage-refresh.tsv"),
            "crate\tstatus\texec\tfunc\tbranch\tregion\treport\nradroots_a\tpass\t100\t100\t100\tnot-a-number\tfile\n",
        );
        let bad_region =
            load_coverage_refresh_rows(&root).expect_err("invalid region coverage percent");
        assert!(bad_region.contains("parse region"));

        write_file(
            &coverage_dir.join("coverage-refresh.tsv"),
            "crate\tstatus\texec\tfunc\tbranch\tregion\treport\nradroots_a\tpass\t100\t100\t100\t100\tfile\nradroots_a\tpass\t100\t100\t100\t100\tfile\n",
        );
        let duplicate_row = load_coverage_refresh_rows(&root).expect_err("duplicate coverage row");
        assert!(duplicate_row.contains("duplicate coverage row"));

        write_file(
            &coverage_dir.join("coverage-refresh.tsv"),
            "crate\tstatus\texec\tfunc\tbranch\tregion\treport\nradroots_a\tfail\t100\t100\t100\t100\tfile\n",
        );
        let required = ["radroots_a".to_string()]
            .into_iter()
            .collect::<BTreeSet<_>>();
        let non_pass = validate_required_coverage_summary(&root, &required, strict_thresholds())
            .expect_err("non-pass status");
        assert!(non_pass.contains("non-pass status"));

        write_file(
            &coverage_dir.join("coverage-refresh.tsv"),
            "crate\tstatus\texec\tfunc\tbranch\tregion\treport\nradroots_a\tpass\t99.9\t100\t100\t100\tfile\n",
        );
        let below_100 = validate_required_coverage_summary(&root, &required, strict_thresholds())
            .expect_err("coverage below 100");
        assert!(below_100.contains("must satisfy coverage policy"));

        let missing = ["missing".to_string()].into_iter().collect::<BTreeSet<_>>();
        let missing_err = validate_required_coverage_summary(&root, &missing, strict_thresholds())
            .expect_err("missing required row");
        assert!(missing_err.contains("missing from coverage-refresh.tsv"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn enum_extract_and_parse_error_paths_are_reported() {
        let missing = extract_enum_body("pub struct X;", "RadrootsCoreUnitDimension")
            .expect_err("missing enum");
        assert!(missing.contains("missing enum"));

        let missing_brace = extract_enum_body(
            "pub enum RadrootsCoreUnitDimension",
            "RadrootsCoreUnitDimension",
        )
        .expect_err("missing opening brace");
        assert!(missing_brace.contains("missing opening brace"));

        let missing_close = extract_enum_body(
            "pub enum RadrootsCoreUnitDimension { Count, Mass",
            "RadrootsCoreUnitDimension",
        )
        .expect_err("missing closing brace");
        assert!(missing_close.contains("missing closing brace"));

        let variants = parse_enum_variants(
            r#"
            ,
            = 1,
            // skip
            #![cfg(test)]
            Count,
            "#,
        );
        assert_eq!(variants, vec!["Count".to_string()]);

        let nested = extract_enum_body(
            "pub enum RadrootsCoreUnitDimension { Count = { 1 }, Mass = 2 }",
            "RadrootsCoreUnitDimension",
        )
        .expect("nested braces in enum body");
        assert!(nested.contains("Count"));
    }

    #[test]
    fn coverage_policy_parity_reports_contract_errors() {
        let root = create_synthetic_workspace("coverage_policy_errors");
        let contract_root = root.join("spec");
        let coverage_root = coverage_root(&contract_root);

        write_file(
            &coverage_root.join("policy.toml"),
            r#"[gate]
fail_under_exec_lines = 100.0
fail_under_functions = 100.0
fail_under_regions = 100.0
fail_under_branches = 100.0
require_branches = true

[required]
crates = []
"#,
        );
        let empty_required =
            validate_coverage_policy_parity(&root, &contract_root).expect_err("empty required");
        assert!(empty_required.contains("required crates list must not be empty"));

        write_file(
            &coverage_root.join("policy.toml"),
            r#"[gate]
fail_under_exec_lines = 99.0
fail_under_functions = 100.0
fail_under_regions = 100.0
fail_under_branches = 100.0
require_branches = true

[required]
crates = ["radroots_a"]
"#,
        );
        let invalid_gate = validate_coverage_policy_parity(&root, &contract_root)
            .expect_err("invalid policy thresholds");
        assert!(invalid_gate.contains("100/100/100/100"));

        write_file(
            &coverage_root.join("policy.toml"),
            r#"[gate]
fail_under_exec_lines = 100.0
fail_under_functions = 99.0
fail_under_regions = 100.0
fail_under_branches = 100.0
require_branches = true

[required]
crates = ["radroots_a"]
"#,
        );
        let invalid_functions = validate_coverage_policy_parity(&root, &contract_root)
            .expect_err("invalid function threshold");
        assert!(invalid_functions.contains("100/100/100/100"));

        write_file(
            &coverage_root.join("policy.toml"),
            r#"[gate]
fail_under_exec_lines = 100.0
fail_under_functions = 100.0
fail_under_regions = 99.0
fail_under_branches = 100.0
require_branches = true

[required]
crates = ["radroots_a"]
"#,
        );
        let invalid_regions = validate_coverage_policy_parity(&root, &contract_root)
            .expect_err("invalid region threshold");
        assert!(invalid_regions.contains("100/100/100/100"));

        write_file(
            &coverage_root.join("policy.toml"),
            r#"[gate]
fail_under_exec_lines = 100.0
fail_under_functions = 100.0
fail_under_regions = 100.0
fail_under_branches = 99.0
require_branches = true

[required]
crates = ["radroots_a"]
"#,
        );
        let invalid_branches = validate_coverage_policy_parity(&root, &contract_root)
            .expect_err("invalid branch threshold");
        assert!(invalid_branches.contains("100/100/100/100"));

        write_file(
            &coverage_root.join("policy.toml"),
            r#"[gate]
fail_under_exec_lines = 100.0
fail_under_functions = 100.0
fail_under_regions = 100.0
fail_under_branches = 100.0
require_branches = true

[required]
crates = ["radroots_a", "radroots_a"]
"#,
        );
        let duplicate_required = validate_coverage_policy_parity(&root, &contract_root)
            .expect_err("duplicate required crate");
        assert!(duplicate_required.contains("duplicate crate"));

        write_file(
            &coverage_root.join("policy.toml"),
            r#"[gate]
fail_under_exec_lines = 100.0
fail_under_functions = 100.0
fail_under_regions = 100.0
fail_under_branches = 100.0
require_branches = false

[required]
crates = ["radroots_a"]
"#,
        );
        let branches_optional = validate_coverage_policy_parity(&root, &contract_root)
            .expect_err("branches must be required");
        assert!(branches_optional.contains("required branches"));

        write_file(
            &coverage_root.join("policy.toml"),
            r#"[gate]
fail_under_exec_lines = 100.0
fail_under_functions = 100.0
fail_under_regions = 100.0
fail_under_branches = 100.0
require_branches = true

[required]
crates = ["radroots_b"]
"#,
        );
        let missing_workspace = validate_coverage_policy_parity(&root, &contract_root)
            .expect_err("missing public crate in policy");
        assert!(missing_workspace.contains("missing public crates"));

        write_file(
            &coverage_root.join("policy.toml"),
            r#"[gate]
fail_under_exec_lines = 100.0
fail_under_functions = 100.0
fail_under_regions = 100.0
fail_under_branches = 100.0
require_branches = true

[required]
crates = ["unknown"]
"#,
        );
        let required_unknown = validate_coverage_policy_parity(&root, &contract_root)
            .expect_err("non-public required crate");
        assert!(required_unknown.contains("includes non-public crates"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn release_publish_policy_reports_contract_errors() {
        let root = create_synthetic_workspace("release_policy_errors");
        let contract_root = root.join("spec");
        let release_policy_path = root_release_policy_path(&root);

        write_file(
            &release_policy_path,
            r#"[release]
version = ""

[publish]
crates = ["radroots_a"]

[internal]
crates = ["radroots_b"]

[publish_order]
crates = ["radroots_a"]
"#,
        );
        let empty_version = validate_release_publish_policy(&root, &contract_root, "1.0.0")
            .expect_err("empty release version");
        assert!(empty_version.contains("must not be empty"));

        write_file(
            &release_policy_path,
            r#"[release]
version = "2.0.0"

[publish]
crates = ["radroots_a"]

[internal]
crates = ["radroots_b"]

[publish_order]
crates = ["radroots_a"]
"#,
        );
        let version_mismatch = validate_release_publish_policy(&root, &contract_root, "1.0.0")
            .expect_err("release version mismatch");
        assert!(version_mismatch.contains("must match contract version"));

        write_file(
            &release_policy_path,
            r#"[release]
version = "1.0.0"

[publish]
crates = ["radroots_a"]

[internal]
crates = ["radroots_a"]

[publish_order]
crates = ["radroots_a"]
"#,
        );
        let overlap = validate_release_publish_policy(&root, &contract_root, "1.0.0")
            .expect_err("publish/internal overlap");
        assert!(overlap.contains("overlap is not allowed"));

        write_file(
            &release_policy_path,
            r#"[release]
version = "1.0.0"

[publish]
crates = ["radroots_a"]

[internal]
crates = []

[publish_order]
crates = ["radroots_a"]
"#,
        );
        let missing_workspace = validate_release_publish_policy(&root, &contract_root, "1.0.0")
            .expect_err("missing workspace crate");
        assert!(missing_workspace.contains("missing workspace crates"));

        write_file(
            &release_policy_path,
            r#"[release]
version = "1.0.0"

[publish]
crates = ["radroots_a"]

[internal]
crates = ["radroots_b"]

[publish_order]
crates = []
"#,
        );
        let missing_publish_order = validate_release_publish_policy(&root, &contract_root, "1.0.0")
            .expect_err("missing publish order entries");
        assert!(missing_publish_order.contains("missing publish crates"));

        write_file(
            &release_policy_path,
            r#"[release]
version = "1.0.0"

[publish]
crates = ["radroots_a"]

[internal]
crates = ["radroots_b"]

[publish_order]
crates = ["radroots_a", "radroots_b"]
"#,
        );
        let extra_publish_order = validate_release_publish_policy(&root, &contract_root, "1.0.0")
            .expect_err("extra publish order entries");
        assert!(extra_publish_order.contains("non-publish crates"));

        write_file(
            &root.join("crates").join("a").join("Cargo.toml"),
            r#"[package]
name = "radroots_a"
publish = ["crates-io"]
version = "0.1.0"
edition = "2024"
description = "crate a"
repository = "https://example.com/a"
homepage = "https://example.com/a"
documentation = "https://docs.example.com/a"
readme = "README"

[dependencies]
radroots_b = { path = "../b" }
"#,
        );
        write_file(
            &root.join("crates").join("b").join("Cargo.toml"),
            r#"[package]
name = "radroots_b"
version = "0.1.0"
edition = "2024"
description = "crate b"
repository = "https://example.com/b"
homepage = "https://example.com/b"
documentation = "https://docs.example.com/b"
readme = "README"
"#,
        );
        write_file(
            &release_policy_path,
            r#"[release]
version = "1.0.0"

[publish]
crates = ["radroots_a", "radroots_b"]

[internal]
crates = []

[publish_order]
crates = ["radroots_a", "radroots_b"]
"#,
        );
        let dependency_order = validate_release_publish_policy(&root, &contract_root, "1.0.0")
            .expect_err("dependency order violation");
        assert!(dependency_order.contains("must place dependency"));

        write_file(
            &release_policy_path,
            r#"[release]
version = "1.0.0"

[publish]
crates = ["radroots_a"]

[internal]
crates = ["radroots_b"]

[publish_order]
crates = ["radroots_a"]
"#,
        );
        write_file(
            &root.join("crates").join("b").join("Cargo.toml"),
            r#"[package]
name = "radroots_b"
version = "0.1.0"
edition = "2024"
publish = false
"#,
        );
        validate_release_publish_policy(&root, &contract_root, "1.0.0")
            .expect("internal dependency should be ignored in publish ordering");

        write_file(
            &root.join("crates").join("a").join("Cargo.toml"),
            r#"[package]
name = "radroots_a"
version = "0.1.0"
edition = "2024"
publish = false
"#,
        );
        write_file(
            &release_policy_path,
            r#"[release]
version = "1.0.0"

[publish]
crates = ["radroots_a"]

[internal]
crates = ["radroots_b"]

[publish_order]
crates = ["radroots_a"]
"#,
        );
        let publish_flag = validate_release_publish_policy(&root, &contract_root, "1.0.0")
            .expect_err("publish crate must be publishable");
        assert!(publish_flag.contains("must set publish = [\"crates-io\"]"));

        write_file(
            &root.join("crates").join("a").join("Cargo.toml"),
            r#"[package]
name = "radroots_a"
publish = ["crates-io"]
version = "0.1.0"
edition = "2024"
description = "crate a"
repository = "https://example.com/a"
homepage = "https://example.com/a"
documentation = "https://docs.example.com/a"
readme = "README"
"#,
        );
        write_file(
            &root.join("crates").join("b").join("Cargo.toml"),
            r#"[package]
name = "radroots_b"
version = "0.1.0"
edition = "2024"
"#,
        );
        let internal_flag = validate_release_publish_policy(&root, &contract_root, "1.0.0")
            .expect_err("internal crate must be non-publishable");
        assert!(internal_flag.contains("non-public crate"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn validate_contract_bundle_reports_required_field_errors() {
        let root = create_synthetic_workspace("contract_bundle_errors");

        let assert_bundle_error = |expected: &str, mutator: fn(&mut ContractBundle)| {
            let mut bundle = load_contract_bundle(&root).expect("load bundle");
            mutator(&mut bundle);
            let err = validate_contract_bundle(&bundle).expect_err("bundle validation error");
            assert!(err.contains(expected), "expected `{expected}` in `{err}`");
        };

        assert_bundle_error("contract name is required", |bundle| {
            bundle.manifest.contract.name.clear();
        });
        assert_bundle_error("contract version is required", |bundle| {
            bundle.manifest.contract.version.clear();
        });
        assert_bundle_error("contract source is required", |bundle| {
            bundle.manifest.contract.source.clear();
        });
        assert_bundle_error("surface.model_crates must not be empty", |bundle| {
            bundle.manifest.surface.model_crates.clear();
        });
        assert_bundle_error("surface.algorithm_crates must not be empty", |bundle| {
            bundle.manifest.surface.algorithm_crates.clear();
        });
        assert_bundle_error("surface.wasm_crates must not be empty", |bundle| {
            bundle.manifest.surface.wasm_crates.clear();
        });
        assert_bundle_error(
            "at least one language export mapping is required",
            |bundle| {
                bundle.exports.clear();
            },
        );
        assert_bundle_error("language.id is required", |bundle| {
            bundle.exports[0].language.id.clear();
        });
        assert_bundle_error("language.repository is required", |bundle| {
            bundle.exports[0].language.repository.clear();
        });
        assert_bundle_error("packages map is required", |bundle| {
            bundle.exports[0].packages.clear();
        });
        assert_bundle_error("artifacts fields must be non-empty for ts", |bundle| {
            bundle.exports[0]
                .artifacts
                .as_mut()
                .expect("ts artifacts")
                .models_dir = Some(String::new());
        });
        assert_bundle_error("artifacts fields must be non-empty for ts", |bundle| {
            bundle.exports[0]
                .artifacts
                .as_mut()
                .expect("ts artifacts")
                .constants_dir = Some(String::new());
        });
        assert_bundle_error("artifacts fields must be non-empty for ts", |bundle| {
            bundle.exports[0]
                .artifacts
                .as_mut()
                .expect("ts artifacts")
                .wasm_dist_dir = Some(String::new());
        });
        assert_bundle_error("artifacts fields must be non-empty for ts", |bundle| {
            bundle.exports[0]
                .artifacts
                .as_mut()
                .expect("ts artifacts")
                .manifest_file = Some(String::new());
        });
        assert_bundle_error("version.contract.version is required", |bundle| {
            bundle.version.contract.version.clear();
        });
        assert_bundle_error("version.contract.stability is required", |bundle| {
            bundle.version.contract.stability.clear();
        });
        assert_bundle_error("version.semver rules must all be non-empty", |bundle| {
            bundle.version.semver.major_on.clear();
        });
        assert_bundle_error("version.semver rules must all be non-empty", |bundle| {
            bundle.version.semver.minor_on.clear();
        });
        assert_bundle_error("version.semver rules must all be non-empty", |bundle| {
            bundle.version.semver.patch_on.clear();
        });
        assert_bundle_error(
            "compatibility.requires_conformance_pass must be true",
            |bundle| {
                bundle.version.compatibility.requires_conformance_pass = false;
            },
        );
        assert_bundle_error(
            "compatibility.requires_export_manifest_diff must be true",
            |bundle| {
                bundle.version.compatibility.requires_export_manifest_diff = false;
            },
        );
        assert_bundle_error(
            "compatibility.requires_release_notes must be true",
            |bundle| {
                bundle.version.compatibility.requires_release_notes = false;
            },
        );
        assert_bundle_error("contract policy flags must all be true", |bundle| {
            bundle.manifest.policy.exclude_internal_workspace_crates = false;
        });
        assert_bundle_error("contract policy flags must all be true", |bundle| {
            bundle.manifest.policy.require_reproducible_exports = false;
        });
        assert_bundle_error("contract policy flags must all be true", |bundle| {
            bundle.manifest.policy.require_conformance_vectors = false;
        });

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn validate_contract_bundle_reports_operation_contract_errors() {
        let root = create_synthetic_workspace("operation_contract_bundle_errors");
        add_operation_contract_files(&root);

        let assert_bundle_error = |expected: &str, mutator: fn(&mut ContractBundle)| {
            let mut bundle = load_contract_bundle(&root).expect("load bundle");
            mutator(&mut bundle);
            let err = validate_contract_bundle(&bundle).expect_err("bundle validation error");
            assert!(err.contains(expected), "expected `{expected}` in `{err}`");
        };

        assert_bundle_error("public.domains must not be empty", |bundle| {
            bundle
                .operations_manifest
                .as_mut()
                .expect("operations manifest")
                .public
                .domains
                .clear();
        });
        assert_bundle_error(
            "sdk-exports must define at least one operation-based language mapping",
            |bundle| {
                bundle.sdk_exports.clear();
            },
        );
        assert_bundle_error(
            "sdk export ts must cover every public operation",
            |bundle| {
                bundle.sdk_exports[0]
                    .operations
                    .remove("listing.build_draft");
            },
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn validate_contract_bundle_requires_real_conformance_assets() {
        let missing_schema_root = create_synthetic_workspace("operation_contract_missing_schema");
        add_operation_contract_files(&missing_schema_root);
        let _ = fs::remove_file(conformance_schema_path(&missing_schema_root));
        let bundle = load_contract_bundle(&missing_schema_root).expect("load bundle");
        let err = validate_contract_bundle(&bundle).expect_err("missing schema should fail");
        assert!(err.contains("vector.schema.json"));
        let _ = fs::remove_dir_all(&missing_schema_root);

        let invalid_vector_root = create_synthetic_workspace("operation_contract_invalid_vector");
        add_operation_contract_files(&invalid_vector_root);
        write_file(
            &invalid_vector_root
                .join("spec")
                .join("conformance")
                .join("vectors")
                .join("profile")
                .join("build_draft.v1.json"),
            r#"{
  "suite": "profile",
  "contract_version": "1.0.0",
  "vectors": [
    {
      "id": "profile_build_draft_minimal_001",
      "kind": "profile.build_draft",
      "input": {}
    }
  ]
}
"#,
        );
        let bundle = load_contract_bundle(&invalid_vector_root).expect("load bundle");
        let err = validate_contract_bundle(&bundle).expect_err("invalid vector should fail");
        assert!(err.contains("build_draft.v1.json"));
        assert!(err.contains("parse"));
        let _ = fs::remove_dir_all(&invalid_vector_root);

        let root = create_synthetic_workspace("operation_contract_vector_path");
        add_operation_contract_files(&root);
        let mut bundle = load_contract_bundle(&root).expect("load bundle");
        bundle
            .operations_manifest
            .as_mut()
            .expect("operations manifest")
            .operations
            .get_mut("profile_build_draft")
            .expect("profile operation")
            .conformance
            .vector = "conformance/vectors/profile/build_draft.v1.json".to_string();
        let err = validate_contract_bundle(&bundle).expect_err("legacy path should fail");
        assert!(err.contains("must live under spec/conformance/"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn parse_toml_and_publish_flags_report_failures() {
        let missing = temp_root("parse_toml_missing");
        let read_err =
            parse_toml::<WorkspaceCargoManifest>(&missing.join("Cargo.toml")).expect_err("missing");
        assert!(read_err.contains("read"));
        let _ = fs::remove_dir_all(&missing);

        let invalid = temp_root("parse_toml_invalid");
        write_file(&invalid.join("Cargo.toml"), "[workspace]\nmembers = [");
        let parse_err = parse_toml::<WorkspaceCargoManifest>(&invalid.join("Cargo.toml"))
            .expect_err("invalid manifest");
        assert!(parse_err.contains("parse"));
        let _ = fs::remove_dir_all(&invalid);

        let contract_manifest_missing = temp_root("parse_contract_manifest_missing");
        let contract_manifest_read_err =
            parse_toml::<ContractManifest>(&contract_manifest_missing.join("manifest.toml"))
                .expect_err("missing contract manifest");
        assert!(contract_manifest_read_err.contains("read"));
        let _ = fs::remove_dir_all(&contract_manifest_missing);

        let contract_manifest_invalid = temp_root("parse_contract_manifest_invalid");
        write_file(
            &contract_manifest_invalid.join("manifest.toml"),
            "[contract",
        );
        let contract_manifest_parse_err =
            parse_toml::<ContractManifest>(&contract_manifest_invalid.join("manifest.toml"))
                .expect_err("invalid contract manifest");
        assert!(contract_manifest_parse_err.contains("parse"));
        let _ = fs::remove_dir_all(&contract_manifest_invalid);

        let version_missing = temp_root("parse_version_policy_missing");
        let version_read_err = parse_toml::<VersionPolicy>(&version_missing.join("version.toml"))
            .expect_err("missing version policy");
        assert!(version_read_err.contains("read"));
        let _ = fs::remove_dir_all(&version_missing);

        let version_invalid = temp_root("parse_version_policy_invalid");
        write_file(&version_invalid.join("version.toml"), "[version");
        let version_parse_err = parse_toml::<VersionPolicy>(&version_invalid.join("version.toml"))
            .expect_err("invalid version policy");
        assert!(version_parse_err.contains("parse"));
        let _ = fs::remove_dir_all(&version_invalid);

        let release_missing = temp_root("parse_release_contract_missing");
        let release_read_err =
            parse_toml::<ReleaseContractFile>(&release_missing.join("publish-set.toml"))
                .expect_err("missing release contract");
        assert!(release_read_err.contains("read"));
        let _ = fs::remove_dir_all(&release_missing);

        let release_invalid = temp_root("parse_release_contract_invalid");
        write_file(&release_invalid.join("publish-set.toml"), "[release");
        let release_parse_err =
            parse_toml::<ReleaseContractFile>(&release_invalid.join("publish-set.toml"))
                .expect_err("invalid release contract");
        assert!(release_parse_err.contains("parse"));
        let _ = fs::remove_dir_all(&release_invalid);

        let export_missing = temp_root("parse_export_mapping_missing");
        let export_read_err = parse_toml::<ExportMapping>(&export_missing.join("model.toml"))
            .expect_err("missing export mapping");
        assert!(export_read_err.contains("read"));
        let _ = fs::remove_dir_all(&export_missing);

        let export_invalid = temp_root("parse_export_mapping_invalid");
        write_file(&export_invalid.join("model.toml"), "[export");
        let export_parse_err = parse_toml::<ExportMapping>(&export_invalid.join("model.toml"))
            .expect_err("invalid export mapping");
        assert!(export_parse_err.contains("parse"));
        let _ = fs::remove_dir_all(&export_invalid);

        let dup = temp_root("publish_flags_duplicate");
        write_file(
            &dup.join("Cargo.toml"),
            r#"[workspace]
members = ["crates/a", "crates/b"]
"#,
        );
        let member_manifest =
            "[package]\nname = \"duplicate\"\nversion = \"0.1.0\"\nedition = \"2024\"\n";
        write_file(
            &dup.join("crates").join("a").join("Cargo.toml"),
            member_manifest,
        );
        write_file(
            &dup.join("crates").join("b").join("Cargo.toml"),
            member_manifest,
        );
        let dup_err = workspace_package_publish_flags(&dup).expect_err("duplicate publish flags");
        assert!(dup_err.contains("duplicate workspace package name"));
        let _ = fs::remove_dir_all(&dup);
    }

    #[test]
    fn workspace_package_records_and_callers_report_member_manifest_errors() {
        let root = temp_root("workspace_package_record_errors");
        write_file(
            &root.join("Cargo.toml"),
            r#"[workspace]
members = ["crates/a"]
"#,
        );

        let read_err =
            workspace_package_records(&root).expect_err("missing member manifest should fail");
        assert!(read_err.contains("read"));

        let names_err = workspace_package_names(&root).expect_err("names should fail");
        assert!(names_err.contains("read"));
        let manifests_err = workspace_package_manifests(&root).expect_err("manifests should fail");
        assert!(manifests_err.contains("read"));
        let flags_err = workspace_package_publish_flags(&root).expect_err("flags should fail");
        assert!(flags_err.contains("read"));
        let deps_err = read_workspace_package_dependencies(&root).expect_err("deps should fail");
        assert!(deps_err.contains("read"));

        let publish = ["radroots_a".to_string()]
            .into_iter()
            .collect::<BTreeSet<_>>();
        let publish_err =
            validate_publish_package_metadata(&root, &publish).expect_err("publish metadata");
        assert!(publish_err.contains("read"));

        write_file(
            &root.join("crates").join("a").join("Cargo.toml"),
            "[package",
        );
        let parse_value_err =
            workspace_package_records(&root).expect_err("invalid toml should fail");
        assert!(parse_value_err.contains("parse"));

        write_file(
            &root.join("crates").join("a").join("Cargo.toml"),
            r#"[workspace]
resolver = "2"
"#,
        );
        let parse_package_err =
            workspace_package_records(&root).expect_err("missing package table should fail");
        assert!(parse_package_err.contains("parse"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn workspace_package_manifests_success_and_publish_metadata_duplicate_names() {
        let root = create_synthetic_workspace("workspace_manifest_success");
        let manifests = workspace_package_manifests(&root).expect("workspace manifests");
        assert_eq!(manifests.len(), 2);
        assert!(manifests.contains_key("radroots_a"));
        assert!(manifests.contains_key("radroots_b"));

        write_file(
            &root.join("crates").join("b").join("Cargo.toml"),
            r#"[package]
name = "radroots_a"
version = "0.1.0"
edition = "2024"
description = "crate b duplicate name"
repository = "https://example.com/b"
homepage = "https://example.com/b"
documentation = "https://docs.example.com/b"
readme = "README"
publish = false
"#,
        );
        let publish = ["radroots_a".to_string()]
            .into_iter()
            .collect::<BTreeSet<_>>();
        let duplicate_err =
            validate_publish_package_metadata(&root, &publish).expect_err("duplicate package map");
        assert!(duplicate_err.contains("duplicate workspace package name"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn workspace_package_publish_configs_cover_success_and_duplicate_names() {
        let root = create_synthetic_workspace("workspace_publish_configs");
        let flags = workspace_package_publish_flags(&root).expect("publish flags");
        assert_eq!(flags["radroots_a"], true);
        assert_eq!(flags["radroots_b"], false);

        let configs = workspace_package_publish_configs(&root).expect("publish configs");
        assert_eq!(
            configs["radroots_a"],
            Some(PackagePublish::Registries(vec!["crates-io".to_string()]))
        );
        assert_eq!(configs["radroots_b"], Some(PackagePublish::Bool(false)));

        write_file(
            &root.join("crates").join("b").join("Cargo.toml"),
            r#"[package]
name = "radroots_a"
version = "0.1.0"
edition = "2024"
publish = false
"#,
        );
        let duplicate_err = workspace_package_publish_configs(&root)
            .expect_err("duplicate package name in publish configs");
        assert!(duplicate_err.contains("duplicate workspace package name"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn workspace_package_publish_configs_report_workspace_record_errors() {
        let root = temp_root("workspace_publish_configs_errors");
        let err = workspace_package_publish_configs(&root)
            .expect_err("missing workspace manifest should fail");
        assert!(err.contains("Cargo.toml"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn coverage_release_and_bundle_loaders_report_parse_and_read_errors() {
        let root = create_synthetic_workspace("coverage_release_loader_errors");
        let contract_root = root.join("spec");
        let coverage_root = coverage_root(&contract_root);
        let release_policy_path = root_release_policy_path(&root);

        let missing_workspace = temp_root("coverage_missing_workspace_manifest");
        let policy_workspace_err =
            validate_coverage_policy_parity(&missing_workspace, &contract_root)
                .expect_err("coverage release policy lookup error");
        assert!(policy_workspace_err.contains("release publish policy not found"));
        let _ = fs::remove_dir_all(&missing_workspace);

        let _ = fs::remove_file(coverage_root.join("policy.toml"));
        let policy_load_err = validate_coverage_policy_parity(&root, &contract_root)
            .expect_err("coverage policy read error");
        assert!(policy_load_err.contains("policy.toml"));
        write_file(
            &coverage_root.join("policy.toml"),
            r#"[gate]
fail_under_exec_lines = 100.0
fail_under_functions = 100.0
fail_under_regions = 100.0
fail_under_branches = 100.0
require_branches = true

[required]
crates = ["radroots_a", "radroots_b"]
"#,
        );

        let missing_release = temp_root("release_missing_workspace_manifest");
        write_root_release_policy(
            &missing_release,
            r#"[release]
version = "1.0.0"

[publish]
crates = ["radroots_a"]

[internal]
crates = ["radroots_b"]

[publish_order]
crates = ["radroots_a"]
"#,
        );
        let release_workspace_err =
            validate_release_publish_policy(&missing_release, &contract_root, "1.0.0")
                .expect_err("release workspace read error");
        assert!(release_workspace_err.contains("Cargo.toml"));
        let _ = fs::remove_dir_all(&missing_release);

        let _ = fs::remove_file(&release_policy_path);
        let release_load_err = validate_release_publish_policy(&root, &contract_root, "1.0.0")
            .expect_err("release contract read error");
        assert!(release_load_err.contains(ROOT_RELEASE_POLICY_RELATIVE));

        write_file(
            &release_policy_path,
            r#"[release]
version = "1.0.0"

[publish]
crates = ["radroots_a", "radroots_a"]

[internal]
crates = ["radroots_b"]

[publish_order]
crates = ["radroots_a"]
"#,
        );
        let duplicate_publish = validate_release_publish_policy(&root, &contract_root, "1.0.0")
            .expect_err("duplicate publish crates");
        assert!(duplicate_publish.contains("publish.crates has duplicate crate"));

        write_file(
            &release_policy_path,
            r#"[release]
version = "1.0.0"

[publish]
crates = ["radroots_a"]

[internal]
crates = ["radroots_b", "radroots_b"]

[publish_order]
crates = ["radroots_a"]
"#,
        );
        let duplicate_internal = validate_release_publish_policy(&root, &contract_root, "1.0.0")
            .expect_err("duplicate internal crates");
        assert!(duplicate_internal.contains("internal.crates has duplicate crate"));

        write_file(
            &release_policy_path,
            r#"[release]
version = "1.0.0"

[publish]
crates = ["radroots_a"]

[internal]
crates = ["radroots_b"]

[publish_order]
crates = ["radroots_a", "radroots_a"]
"#,
        );
        let duplicate_order = validate_release_publish_policy(&root, &contract_root, "1.0.0")
            .expect_err("duplicate publish order");
        assert!(duplicate_order.contains("publish_order.crates has duplicate crate"));

        write_file(
            &release_policy_path,
            r#"[release]
version = "1.0.0"

[publish]
crates = ["radroots_a"]

[internal]
crates = ["radroots_b"]

[publish_order]
crates = ["radroots_a"]
"#,
        );
        write_file(
            &root.join("crates").join("a").join("Cargo.toml"),
            "[package",
        );
        let dependency_err = validate_release_publish_policy(&root, &contract_root, "1.0.0")
            .expect_err("workspace dependency parse error");
        assert!(dependency_err.contains("parse"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn load_release_contract_with_override_reports_override_and_missing_policy_errors() {
        let root = create_synthetic_workspace("release_contract_loader_errors");
        let contract_root = root.join("spec");

        let missing_override = root.join("missing-release-policy.toml");
        let override_err = load_release_contract_with_override(
            &root,
            &contract_root,
            Some(missing_override.clone()),
        )
        .expect_err("missing override should fail");
        assert!(override_err.contains(RELEASE_POLICY_ENV));
        assert!(override_err.contains("missing release policy file"));

        let _ = fs::remove_file(root_release_policy_path(&root));
        let missing_policy_err = load_release_contract_with_override(&root, &contract_root, None)
            .expect_err("missing release policy should fail");
        assert!(missing_policy_err.contains("release publish policy not found"));
        assert!(missing_policy_err.contains(ROOT_RELEASE_POLICY_RELATIVE));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn root_release_policy_preflight_covers_classification_variants() {
        let root = create_synthetic_workspace("root_release_policy_classifications");
        configure_root_release_policy_workspace(&root);
        write_root_release_policy(
            &root,
            r#"[release]
version = "1.0.0"

[classification]
public = ["radroots_a"]
internal = ["radroots_b"]
deferred = ["radroots_c"]
retired = ["radroots_d"]
yank_only = ["radroots_e"]

[publish_order]
crates = ["radroots_a"]
"#,
        );

        let bundle = load_contract_bundle(&root).expect("load root release policy bundle");
        validate_contract_bundle(&bundle).expect("validate root release policy bundle");
        validate_release_preflight(&root).expect("validate root release policy preflight");

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn root_release_policy_reports_deferred_retired_and_yank_only_errors() {
        for (label, policy_body, expected) in [
            (
                "deferred",
                r#"[release]
version = "1.0.0"

[classification]
public = ["radroots_a"]
internal = ["radroots_b"]
deferred = ["radroots_c", "radroots_c"]
retired = ["radroots_d"]
yank_only = ["radroots_e"]

[publish_order]
crates = ["radroots_a"]
"#,
                "classification.deferred has duplicate crate radroots_c",
            ),
            (
                "retired",
                r#"[release]
version = "1.0.0"

[classification]
public = ["radroots_a"]
internal = ["radroots_b"]
deferred = ["radroots_c"]
retired = [""]
yank_only = ["radroots_e"]

[publish_order]
crates = ["radroots_a"]
"#,
                "classification.retired contains an empty crate name",
            ),
            (
                "yank_only",
                r#"[release]
version = "1.0.0"

[classification]
public = ["radroots_a"]
internal = ["radroots_b"]
deferred = ["radroots_c"]
retired = ["radroots_d"]
yank_only = ["radroots_e", "radroots_e"]

[publish_order]
crates = ["radroots_a"]
"#,
                "classification.yank_only has duplicate crate radroots_e",
            ),
        ] {
            let root = create_synthetic_workspace(&format!("root_release_policy_{label}_error"));
            configure_root_release_policy_workspace(&root);
            write_root_release_policy(&root, policy_body);

            let err = validate_release_publish_policy(&root, &root.join("spec"), "1.0.0")
                .expect_err("invalid non-public classification should fail");
            assert!(err.contains(expected), "{label} err: {err}");

            let _ = fs::remove_dir_all(&root);
        }
    }

    #[test]
    fn validate_release_preflight_reports_each_stage_error() {
        let missing_contract_root = temp_root("preflight_missing_contract");
        let missing_contract_err =
            validate_release_preflight(&missing_contract_root).expect_err("missing contract");
        assert!(missing_contract_err.contains("manifest.toml"));
        let _ = fs::remove_dir_all(&missing_contract_root);

        let invalid_bundle = create_synthetic_workspace("preflight_invalid_bundle");
        write_file(
            &invalid_bundle.join("spec").join("manifest.toml"),
            r#"[contract]
name = "radroots_contract"
version = "1.0.0"
source = "synthetic"

[surface]
model_crates = ["radroots_a"]
algorithm_crates = ["radroots_b"]
wasm_crates = ["radroots_a_wasm"]

[export.ts]
packages = ["@radroots/sdk"]

[policy]
exclude_internal_workspace_crates = false
require_reproducible_exports = true
require_conformance_vectors = true
"#,
        );
        let invalid_bundle_err =
            validate_release_preflight(&invalid_bundle).expect_err("bundle validation");
        assert!(invalid_bundle_err.contains("contract policy flags must all be true"));
        let _ = fs::remove_dir_all(&invalid_bundle);

        let missing_release = create_synthetic_workspace("preflight_missing_release");
        let _ = fs::remove_file(root_release_policy_path(&missing_release));
        let missing_release_err =
            validate_release_preflight(&missing_release).expect_err("missing release");
        assert!(missing_release_err.contains(ROOT_RELEASE_POLICY_RELATIVE));
        let _ = fs::remove_dir_all(&missing_release);

        let missing_required = create_synthetic_workspace("preflight_missing_required");
        let _ = fs::remove_file(
            missing_required
                .join("policy")
                .join("coverage")
                .join("policy.toml"),
        );
        let missing_required_err =
            validate_release_preflight(&missing_required).expect_err("missing required list");
        assert!(missing_required_err.contains("policy.toml"));
        let _ = fs::remove_dir_all(&missing_required);

        let duplicate_publish = create_synthetic_workspace("preflight_duplicate_publish");
        write_file(
            &root_release_policy_path(&duplicate_publish),
            r#"[release]
version = "1.0.0"

[publish]
crates = ["radroots_a", "radroots_a"]

[internal]
crates = ["radroots_b"]

[publish_order]
crates = ["radroots_a"]
"#,
        );
        let duplicate_publish_err =
            validate_release_preflight(&duplicate_publish).expect_err("duplicate publish crates");
        assert!(duplicate_publish_err.contains("publish.crates has duplicate crate"));
        let _ = fs::remove_dir_all(&duplicate_publish);

        let duplicate_required = create_synthetic_workspace("preflight_duplicate_required");
        write_file(
            &duplicate_required
                .join("policy")
                .join("coverage")
                .join("policy.toml"),
            "[gate]\nfail_under_exec_lines = 100.0\nfail_under_functions = 100.0\nfail_under_regions = 100.0\nfail_under_branches = 100.0\nrequire_branches = true\n\n[required]\ncrates = [\"radroots_a\", \"radroots_a\"]\n",
        );
        let duplicate_required_err =
            validate_release_preflight(&duplicate_required).expect_err("duplicate required crates");
        assert!(duplicate_required_err.contains("duplicate crate"));
        let _ = fs::remove_dir_all(&duplicate_required);

        let publish_metadata = create_synthetic_workspace("preflight_publish_metadata");
        write_file(
            &publish_metadata.join("crates").join("a").join("Cargo.toml"),
            r#"[package]
name = "radroots_a"
publish = ["crates-io"]
version = "0.1.0"
edition = "2024"
"#,
        );
        let publish_metadata_err =
            validate_release_preflight(&publish_metadata).expect_err("publish metadata validation");
        assert!(publish_metadata_err.contains("must define a non-empty package.description"));
        let _ = fs::remove_dir_all(&publish_metadata);

        let missing_coverage_row = create_synthetic_workspace("preflight_missing_coverage_row");
        write_file(
            &missing_coverage_row
                .join("target")
                .join("coverage")
                .join("coverage-refresh.tsv"),
            "crate\tstatus\texec\tfunc\tbranch\tregion\treport\n",
        );
        let missing_coverage_row_err = validate_release_preflight(&missing_coverage_row)
            .expect_err("required coverage refresh row missing");
        assert!(missing_coverage_row_err.contains("missing from coverage-refresh.tsv"));
        let _ = fs::remove_dir_all(&missing_coverage_row);
    }

    #[test]
    fn load_contract_bundle_and_validation_report_version_export_and_coverage_errors() {
        let root = create_synthetic_workspace("bundle_version_export_and_coverage_errors");
        write_file(&root.join("spec").join("version.toml"), "[contract");
        let version_parse_err = load_contract_bundle(&root).expect_err("invalid version file");
        assert!(version_parse_err.contains("version.toml"));

        write_file(
            &root.join("spec").join("version.toml"),
            r#"[contract]
version = "1.0.0"
stability = "alpha"

[semver]
major_on = ["breaking"]
minor_on = ["feature"]
patch_on = ["fix"]

[compatibility]
requires_conformance_pass = true
requires_export_manifest_diff = true
requires_release_notes = true
"#,
        );
        write_file(
            &root.join("spec").join("exports").join("ts.toml"),
            "[language",
        );
        let export_parse_err = load_contract_bundle(&root).expect_err("invalid export mapping");
        assert!(export_parse_err.contains("ts.toml"));

        write_file(
            &root.join("spec").join("exports").join("ts.toml"),
            r#"[language]
id = "ts"
repository = "sdk-typescript"

[packages]
"radroots_a" = "@radroots/sdk"

[artifacts]
models_dir = "src/generated"
constants_dir = "src/generated"
wasm_dist_dir = "dist"
manifest_file = "export-manifest.json"
"#,
        );
        let bundle = load_contract_bundle(&root).expect("load bundle");
        write_file(
            &root.join("crates").join("core").join("src").join("unit.rs"),
            r#"pub enum RadrootsCoreUnitDimension {
Mass,
Count,
Volume,
}
"#,
        );
        let core_err = validate_contract_bundle(&bundle).expect_err("core unit mismatch");
        assert!(core_err.contains("variant order must be"));

        write_file(
            &root.join("crates").join("core").join("src").join("unit.rs"),
            r#"pub enum RadrootsCoreUnitDimension {
Count,
Mass,
Volume,
}
"#,
        );
        write_file(
            &root.join("policy").join("coverage").join("policy.toml"),
            r#"[gate]
fail_under_exec_lines = 100.0
fail_under_functions = 100.0
fail_under_regions = 100.0
fail_under_branches = 100.0
require_branches = false

[required]
crates = ["radroots_a", "radroots_b"]
"#,
        );
        let policy_err = validate_contract_bundle(&bundle).expect_err("coverage policy validation");
        assert!(policy_err.contains("100/100/100/100"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn coverage_summary_and_core_enum_additional_error_paths() {
        let coverage_root = temp_root("coverage_summary_additional_errors");
        write_file(
            &coverage_root
                .join("target")
                .join("coverage")
                .join("coverage-refresh.tsv"),
            "crate\tstatus\texec\tfunc\tbranch\tregion\treport\nradroots_a\tpass\t100\tbad\t100\t100\tfile\n",
        );
        let func_err = load_coverage_refresh_rows(&coverage_root).expect_err("func parse error");
        assert!(func_err.contains("parse func"));
        write_file(
            &coverage_root
                .join("target")
                .join("coverage")
                .join("coverage-refresh.tsv"),
            "crate\tstatus\texec\tfunc\tbranch\tregion\treport\nradroots_a\tpass\t100\t100\tbad\t100\tfile\n",
        );
        let branch_err =
            load_coverage_refresh_rows(&coverage_root).expect_err("branch parse error");
        assert!(branch_err.contains("parse branch"));
        let _ = fs::remove_dir_all(&coverage_root);

        let missing_refresh_root = temp_root("coverage_summary_missing_refresh");
        let required = ["radroots_a".to_string()]
            .into_iter()
            .collect::<BTreeSet<_>>();
        let missing_refresh_err = validate_required_coverage_summary(
            &missing_refresh_root,
            &required,
            strict_thresholds(),
        )
        .expect_err("missing refresh should fail");
        assert!(missing_refresh_err.contains("coverage-refresh.tsv"));
        let _ = fs::remove_dir_all(&missing_refresh_root);

        let enum_root = temp_root("core_unit_missing_enum");
        write_file(
            &enum_root
                .join("crates")
                .join("core")
                .join("src")
                .join("unit.rs"),
            "pub struct NotTheEnum;",
        );
        let enum_err =
            validate_core_unit_dimension_variant_order(&enum_root).expect_err("missing enum");
        assert!(enum_err.contains("missing enum"));
        let _ = fs::remove_dir_all(&enum_root);
    }

    #[test]
    fn publish_metadata_and_coverage_refresh_report_missing_paths() {
        let root = temp_root("publish_missing_manifest");
        write_file(
            &root.join("Cargo.toml"),
            r#"[workspace]
members = ["crates/a"]
"#,
        );
        write_file(
            &root.join("crates").join("a").join("Cargo.toml"),
            r#"[package]
name = "radroots_a"
version = "0.1.0"
edition = "2024"
description = "crate a"
repository = { workspace = true }
homepage = { workspace = true }
readme = { workspace = true }
"#,
        );
        let missing_manifest = ["radroots_b".to_string()]
            .into_iter()
            .collect::<BTreeSet<_>>();
        let missing_err = validate_publish_package_metadata(&root, &missing_manifest)
            .expect_err("missing workspace manifest");
        assert!(missing_err.contains("has no workspace manifest"));

        let missing_field = ["radroots_a".to_string()]
            .into_iter()
            .collect::<BTreeSet<_>>();
        let field_err = validate_publish_package_metadata(&root, &missing_field)
            .expect_err("missing configured field");
        assert!(field_err.contains("must configure package.documentation"));

        let refresh_missing =
            load_coverage_refresh_rows(&root).expect_err("missing coverage-refresh.tsv");
        assert!(refresh_missing.contains("coverage-refresh.tsv"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn coverage_refresh_parser_skips_blank_lines() {
        let root = temp_root("coverage_refresh_blank_lines");
        write_file(
            &root
                .join("target")
                .join("coverage")
                .join("coverage-refresh.tsv"),
            "crate\tstatus\texec\tfunc\tbranch\tregion\treport\n\nradroots_a\tpass\t100\t100\t100\t100\tfile\n",
        );
        let rows = load_coverage_refresh_rows(&root).expect("rows");
        assert_eq!(rows.len(), 1);
        assert!(rows.contains_key("radroots_a"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn core_unit_dimension_validation_reports_missing_and_mismatch() {
        let missing = temp_root("core_unit_missing");
        let missing_err = validate_core_unit_dimension_variant_order(&missing)
            .expect_err("missing unit file should fail");
        assert!(missing_err.contains("unit.rs"));
        let _ = fs::remove_dir_all(&missing);

        let mismatch = temp_root("core_unit_mismatch");
        write_file(
            &mismatch
                .join("crates")
                .join("core")
                .join("src")
                .join("unit.rs"),
            r#"pub enum RadrootsCoreUnitDimension {
Mass,
Count,
Volume,
}
"#,
        );
        let mismatch_err = validate_core_unit_dimension_variant_order(&mismatch)
            .expect_err("mismatched enum order should fail");
        assert!(mismatch_err.contains("variant order must be"));
        let _ = fs::remove_dir_all(&mismatch);
    }

    #[test]
    fn coverage_and_release_additional_error_branches_are_reported() {
        let root = create_synthetic_workspace("coverage_release_extra_errors");
        let contract_root = root.join("spec");
        let coverage_root = coverage_root(&contract_root);
        let release_policy_path = root_release_policy_path(&root);

        write_file(
            &coverage_root.join("policy.toml"),
            r#"[gate]
fail_under_exec_lines = 100.0
fail_under_functions = 100.0
fail_under_regions = 100.0
fail_under_branches = 100.0
require_branches = true

[required]
crates = ["radroots_a", "radroots_b", "radroots_extra"]
"#,
        );
        let coverage_extra = validate_coverage_policy_parity(&root, &contract_root)
            .expect_err("coverage non-public crate");
        assert!(coverage_extra.contains("includes non-public crates"));

        write_file(
            &coverage_root.join("policy.toml"),
            r#"[gate]
fail_under_exec_lines = 100.0
fail_under_functions = 100.0
fail_under_regions = 100.0
fail_under_branches = 100.0
require_branches = true

[required]
crates = ["radroots_b"]
"#,
        );
        let required_list_mismatch = validate_coverage_policy_parity(&root, &contract_root)
            .expect_err("required list must match public crates");
        assert!(required_list_mismatch.contains("missing public crates"));

        write_file(
            &release_policy_path,
            r#"[release]
version = "1.0.0"

[publish]
crates = ["radroots_a", "radroots_b", "radroots_extra"]

[internal]
crates = []

[publish_order]
crates = ["radroots_a", "radroots_b"]
"#,
        );
        let release_extra = validate_release_publish_policy(&root, &contract_root, "1.0.0")
            .expect_err("release extra crate");
        assert!(release_extra.contains("include unknown crates"));

        write_file(
            &release_policy_path,
            r#"[release]
version = "1.0.0"

[publish]
crates = ["radroots_a"]

[internal]
crates = ["radroots_b"]

[publish_order]
crates = ["radroots_a", "radroots_b"]
"#,
        );
        let publish_order_extra = validate_release_publish_policy(&root, &contract_root, "1.0.0")
            .expect_err("publish order non-publish crate");
        assert!(publish_order_extra.contains("non-publish crates"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn load_contract_bundle_reports_exports_dir_errors_and_skips_non_toml() {
        let root = create_synthetic_workspace("bundle_exports_dir_errors");
        let exports_dir = root.join("spec").join("exports");
        let _ = fs::remove_dir_all(&exports_dir);
        write_file(&exports_dir, "not-a-dir");

        let dir_err = load_contract_bundle(&root).expect_err("exports path must be a directory");
        assert!(dir_err.contains("read dir"));

        let _ = fs::remove_file(&exports_dir);
        fs::create_dir_all(&exports_dir).expect("recreate exports dir");
        write_file(
            &exports_dir.join("typescript.toml"),
            r#"[language]
id = "ts"
repository = "sdk-typescript"

[packages]
radroots_a = "@radroots/sdk"
radroots_b = "@radroots/sdk"

[artifacts]
models_dir = "src/generated"
constants_dir = "src/generated"
wasm_dist_dir = "dist"
manifest_file = "export-manifest.json"
"#,
        );
        write_file(&exports_dir.join("README.txt"), "ignore");
        let bundle = load_contract_bundle(&root).expect("load bundle");
        assert_eq!(bundle.exports.len(), 1);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn validate_contract_bundle_reports_ts_artifacts_and_release_policy_errors() {
        let missing_artifacts = create_synthetic_workspace("bundle_missing_ts_artifacts");
        let mut no_artifacts_bundle =
            load_contract_bundle(&missing_artifacts).expect("load missing artifacts bundle");
        no_artifacts_bundle.exports[0].artifacts = None;
        let artifacts_err =
            validate_contract_bundle(&no_artifacts_bundle).expect_err("missing ts artifacts");
        assert!(artifacts_err.contains("artifacts map is required for ts"));
        let _ = fs::remove_dir_all(&missing_artifacts);

        let curated_package_mismatch = create_synthetic_workspace("bundle_ts_package_mismatch");
        let mut mismatch_bundle =
            load_contract_bundle(&curated_package_mismatch).expect("load mismatch bundle");
        mismatch_bundle.exports[0]
            .packages
            .insert("radroots_a".to_string(), "@radroots/other".to_string());
        assert_eq!(
            ts_curated_package_set(&mismatch_bundle).expect("ts package set"),
            Some(["@radroots/sdk".to_string()].into_iter().collect())
        );
        assert_eq!(
            mismatch_bundle.exports[0]
                .packages
                .values()
                .cloned()
                .collect::<BTreeSet<_>>(),
            ["@radroots/other".to_string()].into_iter().collect()
        );
        let package_err =
            validate_contract_bundle(&mismatch_bundle).expect_err("ts package mismatch");
        assert!(package_err.contains("must match manifest export.ts.packages"));
        let _ = fs::remove_dir_all(&curated_package_mismatch);

        let release_error_root = create_synthetic_workspace("bundle_release_policy_error");
        write_file(
            &root_release_policy_path(&release_error_root),
            r#"[release]
version = "1.0.0"

[publish]
crates = ["radroots_a"]

[internal]
crates = ["radroots_b"]

[publish_order]
crates = []
"#,
        );
        let bundle = load_contract_bundle(&release_error_root).expect("load release error bundle");
        let release_err = validate_contract_bundle(&bundle).expect_err("release policy failure");
        assert!(release_err.contains("publish_order.crates is missing publish crates"));
        let _ = fs::remove_dir_all(&release_error_root);
    }
}
