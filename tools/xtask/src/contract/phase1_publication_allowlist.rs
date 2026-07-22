use super::{
    artifact_bundle::{GeneratedArtifact, read_regular_file, with_artifact_bundle_transaction},
    phase1_publication_artifact::{
        PUBLICATION_SUCCESSOR_SUPERSEDED_PATHS, source_specs,
        validate_immutable_phase1_publication_artifact_predecessor_under_lock,
    },
};
use radroots_event_codec::wire::publication::allowlist::{
    RADROOTS_PHASE1_PUBLICATION_ALLOWLIST_CONTRACT_ID,
    RADROOTS_PHASE1_PUBLICATION_ALLOWLIST_REGISTRY_VERSION,
    RADROOTS_PHASE1_PUBLICATION_ALLOWLIST_VERSION,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, path::Path};

const SCHEMA_VERSION: u32 = 1;
const CONTRACT_ID: &str = "radroots_event_codec.phase1_publication_allowlist_v1";
const AUTHORITY_ID: &str = "phase1_publication_allowlist_v1";
const HASH_ALGORITHM: &str = "sha256_bytes_v1";
const OPERATION_ID: &str = "publication_allowlist.allow_artifact";
const CANONICAL_JSON_OPERATION_ID: &str = "publication_allowlist.allow_canonical_json";
const WRITE_COMMAND: &str = "cargo xtask contract phase1-publication-allowlist-manifest --write";

const MANIFEST_RELATIVE: &str =
    "crates/event_codec/contracts/phase1_publication_allowlist_v1.manifest.json";
const MANIFEST_SCHEMA_RELATIVE: &str =
    "crates/event_codec/contracts/phase1_publication_allowlist_v1.manifest.schema.json";
const MANIFEST_SHA256_RELATIVE: &str =
    "crates/event_codec/contracts/phase1_publication_allowlist_v1.manifest.sha256";
const GENERATED_DESCRIPTOR_RELATIVE: &str =
    "crates/event_codec/contracts/phase1_publication_allowlist_v1.descriptor.json";
const VECTOR_RELATIVE: &str = "contracts/conformance/vectors/publication/phase1_allowlist.v1.json";
const VECTOR_MIRROR_RELATIVE: &str =
    "crates/event_codec/tests/fixtures/phase1_publication_allowlist.v1.json";
const VECTOR_EXECUTOR_RELATIVE: &str = "crates/event_codec/tests/publication_allowlist.rs";
const VECTOR_EXECUTOR_TEST: &str = "publication_allowlist_conformance_vector_executes_every_case";
const OPERATIONS_RELATIVE: &str = "contracts/operations.toml";
const RELEASE_RELATIVE: &str = "contracts/releases/1.0.0-alpha.1.toml";
const CHANGELOG_RELATIVE: &str = "CHANGELOG.md";
const RELEASE_CHANGE_ID: &str = "phase1-publication-allowlist";
const CHANGELOG_MARKER: &str = "<!-- release-change: phase1-publication-allowlist -->";
const REGISTRY_RELATIVE: &str = "contracts/event_store/event_contract_registry_v7.inventory.json";
const REGISTRY_SIDECAR_RELATIVE: &str =
    "contracts/event_store/event_contract_registry_v7.inventory.sha256";
const REGISTRY_BYTE_LENGTH: usize = 158_021;
const REGISTRY_SHA256: &str = "91595544310f865bdef064ee760c227c870417a95b87b3e27278f8da74fdddea";
const REGISTRY_SIDECAR_BYTE_LENGTH: usize = 65;
const REGISTRY_SIDECAR_SHA256: &str =
    "823081e6f423c149865521b185a8517aea35ce4470fa77c165ad7ca5207f036b";
const VECTOR_AUTHOR: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const VECTOR_ZERO_ID: &str = "0000000000000000000000000000000000000000000000000000000000000000";
const VECTOR_ZERO_SIGNATURE: &str = "00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";
const VECTOR_CREATED_AT: u64 = 1_784_347_200;
const VECTOR_PHOTO_IMETA_FIELDS: &[&str] = &[
    "imeta",
    "url https://media.example/51bcf96cda2475475246e3a994aabfee7ff9d7694ba0db9bdc74408632827ad0.webp",
    "x 51bcf96cda2475475246e3a994aabfee7ff9d7694ba0db9bdc74408632827ad0",
    "m image/webp",
    "dim 1200x900",
    "size 13",
    "alt Fresh strawberries",
    "fallback https://backup.example/51bcf96cda2475475246e3a994aabfee7ff9d7694ba0db9bdc74408632827ad0.webp",
];

const PREDECESSOR_CONTRACT_ID: &str = "radroots_event_codec.phase1_publication_artifact_v1";
const PREDECESSOR_ARTIFACTS: &[ImmutableArtifactSpec] = &[
    ImmutableArtifactSpec::new(
        "crates/event_codec/contracts/phase1_publication_artifact_v1.manifest.json",
        89_464,
        "0776e1d84c9366954047e75cdf12d9acc9a7108260157c3534f22067075a385a",
    ),
    ImmutableArtifactSpec::new(
        "crates/event_codec/contracts/phase1_publication_artifact_v1.manifest.schema.json",
        11_972,
        "1d72cee2754e7ac45105d79b1ecf7d44251991be7a18ba106166e962000e8320",
    ),
    ImmutableArtifactSpec::new(
        "crates/event_codec/contracts/phase1_publication_artifact_v1.manifest.sha256",
        65,
        "586c0985b502f22241b4d90f2ecb475d43953fc3ddd66d6fbfa3b3eb9cf34444",
    ),
    ImmutableArtifactSpec::new(
        "contracts/conformance/vectors/publication/phase1_artifact.v1.json",
        23_113,
        "ec18c687d5b0710a48624ddb620d89157e6b645dbea8bb91c62e3a111d20c622",
    ),
    ImmutableArtifactSpec::new(
        "crates/event_codec/tests/fixtures/phase1_publication_artifact.v1.json",
        23_113,
        "ec18c687d5b0710a48624ddb620d89157e6b645dbea8bb91c62e3a111d20c622",
    ),
    ImmutableArtifactSpec::new(
        "crates/event_codec/tests/publication_artifact.rs",
        34_582,
        "7a31169eac4217a38cb3ef25eb9213f2f89e11fb17e76ceaf7449b34225e98af",
    ),
];

const GENERATED_ARTIFACT_PATHS: &[&str] = &[
    MANIFEST_RELATIVE,
    MANIFEST_SCHEMA_RELATIVE,
    MANIFEST_SHA256_RELATIVE,
    GENERATED_DESCRIPTOR_RELATIVE,
    VECTOR_MIRROR_RELATIVE,
];

const PUBLIC_TYPES: &[&str] = &[
    "RadrootsPhase1PublicationLeaf",
    "RadrootsPhase1AllowlistedPublicationArtifact",
    "RadrootsPhase1PublicationAllowlistError",
];

const LEAVES: &[LeafSpec] = &[
    LeafSpec::new(
        "profile",
        0,
        "supporting",
        "profile.build_authored_draft",
        "radroots.profile.metadata.v1",
    ),
    LeafSpec::new(
        "update",
        1,
        "root",
        "social.update.build_authored_draft",
        "radroots.social.update.v1",
    ),
    LeafSpec::new(
        "photo_update",
        1,
        "root",
        "social.photo_update.build_authored_draft",
        "radroots.social.photo_update.v1",
    ),
    LeafSpec::new(
        "ask",
        1,
        "root",
        "social.ask.build_authored_draft",
        "radroots.social.ask.v1",
    ),
    LeafSpec::new(
        "event_date",
        31_922,
        "root",
        "social.calendar_date_event.build_authored_draft",
        "radroots.calendar.date_event.v1",
    ),
    LeafSpec::new(
        "event_time",
        31_923,
        "root",
        "social.calendar_time_event.build_authored_draft",
        "radroots.calendar.time_event.v1",
    ),
    LeafSpec::new(
        "food_availability",
        30_402,
        "root",
        "food_availability.build_authored_draft",
        "radroots.food.availability.v1",
    ),
];

const DENIED_FAMILIES: &[&str] = &[
    "unsealed_profile",
    "unsealed_calendar_event",
    "generic_root_note",
    "reply",
    "comment",
    "deletion_request",
    "generic_calendar",
    "calendar_collection",
    "calendar_rsvp",
    "bud11_authorization",
    "ephemeral_event",
    "operational_listing",
    "generic_nip99",
    "mixed_classified_listing",
    "trade",
    "commerce_order",
    "route_delivery",
    "group",
    "operations",
];

const VALID_VECTOR_SPECS: &[ValidVectorSpec] = &[
    ValidVectorSpec::direct("sealed_profile_is_allowed", 0),
    ValidVectorSpec::direct("sealed_update_is_allowed", 1),
    ValidVectorSpec::direct("sealed_photo_update_is_allowed", 2),
    ValidVectorSpec::direct("sealed_ask_is_allowed", 3),
    ValidVectorSpec::direct("sealed_date_event_is_allowed", 4),
    ValidVectorSpec::direct("sealed_time_event_is_allowed", 5),
    ValidVectorSpec::direct("sealed_food_availability_is_allowed", 6),
    ValidVectorSpec::canonical("canonical_profile_is_allowed", 0),
    ValidVectorSpec::canonical("canonical_update_is_allowed", 1),
    ValidVectorSpec::canonical("canonical_photo_update_is_allowed", 2),
    ValidVectorSpec::canonical("canonical_ask_is_allowed", 3),
    ValidVectorSpec::canonical("canonical_date_event_is_allowed", 4),
    ValidVectorSpec::canonical("canonical_time_event_is_allowed", 5),
    ValidVectorSpec::canonical("canonical_food_availability_is_allowed", 6),
];

const INVALID_VECTOR_SPECS: &[InvalidVectorSpec] = &[
    InvalidVectorSpec::artifact(
        "update_claim_with_ask_marker_is_rejected",
        None,
        "publication_post_profile_invalid",
        ArtifactVectorWitness::UpdateWithAskMarker,
    ),
    InvalidVectorSpec::artifact(
        "photo_claim_with_ask_marker_is_rejected",
        None,
        "publication_post_profile_invalid",
        ArtifactVectorWitness::PhotoUpdateWithAskMarker,
    ),
    InvalidVectorSpec::artifact(
        "ask_claim_without_marker_is_rejected",
        None,
        "publication_post_profile_invalid",
        ArtifactVectorWitness::AskWithoutMarker,
    ),
    InvalidVectorSpec::artifact(
        "update_claim_with_photo_profile_is_rejected",
        None,
        "publication_post_profile_invalid",
        ArtifactVectorWitness::UpdateWithPhotoProfile,
    ),
    InvalidVectorSpec::artifact(
        "typed_date_event_with_generic_shape_is_rejected",
        Some("generic_calendar"),
        "publication_calendar_profile_invalid",
        ArtifactVectorWitness::TypedDateWithGenericShape,
    ),
    InvalidVectorSpec::artifact(
        "typed_time_event_with_generic_shape_is_rejected",
        Some("generic_calendar"),
        "publication_calendar_profile_invalid",
        ArtifactVectorWitness::TypedTimeWithGenericShape,
    ),
    InvalidVectorSpec::artifact(
        "operational_listing_profile_is_rejected",
        Some("operational_listing"),
        "publication_food_profile_invalid",
        ArtifactVectorWitness::OperationalListingProfile,
    ),
    InvalidVectorSpec::artifact(
        "generic_nip99_profile_is_rejected",
        Some("generic_nip99"),
        "publication_food_profile_invalid",
        ArtifactVectorWitness::GenericNip99Profile,
    ),
    InvalidVectorSpec::artifact(
        "mixed_food_and_operational_markers_are_rejected",
        Some("mixed_classified_listing"),
        "publication_food_profile_invalid",
        ArtifactVectorWitness::MixedFoodAndOperationalMarkers,
    ),
    InvalidVectorSpec::raw(
        "unsealed_profile_is_rejected",
        "unsealed_profile",
        RawEventVectorWitness::Profile,
    ),
    InvalidVectorSpec::raw(
        "generic_root_note_is_rejected",
        "generic_root_note",
        RawEventVectorWitness::GenericRootNote,
    ),
    InvalidVectorSpec::raw("reply_is_rejected", "reply", RawEventVectorWitness::Reply),
    InvalidVectorSpec::raw(
        "comment_is_rejected",
        "comment",
        RawEventVectorWitness::Comment,
    ),
    InvalidVectorSpec::raw(
        "deletion_request_is_rejected",
        "deletion_request",
        RawEventVectorWitness::DeletionRequest,
    ),
    InvalidVectorSpec::raw(
        "raw_date_event_is_rejected",
        "unsealed_calendar_event",
        RawEventVectorWitness::CalendarDate,
    ),
    InvalidVectorSpec::raw(
        "raw_time_event_is_rejected",
        "unsealed_calendar_event",
        RawEventVectorWitness::CalendarTime,
    ),
    InvalidVectorSpec::raw(
        "calendar_collection_is_rejected",
        "calendar_collection",
        RawEventVectorWitness::CalendarCollection,
    ),
    InvalidVectorSpec::raw(
        "calendar_rsvp_is_rejected",
        "calendar_rsvp",
        RawEventVectorWitness::CalendarRsvp,
    ),
    InvalidVectorSpec::raw(
        "bud11_authorization_is_rejected",
        "bud11_authorization",
        RawEventVectorWitness::Bud11Authorization,
    ),
    InvalidVectorSpec::raw(
        "ephemeral_event_is_rejected",
        "ephemeral_event",
        RawEventVectorWitness::Ephemeral,
    ),
    InvalidVectorSpec::raw(
        "trade_event_is_rejected",
        "trade",
        RawEventVectorWitness::Trade,
    ),
    InvalidVectorSpec::raw(
        "order_event_is_rejected",
        "commerce_order",
        RawEventVectorWitness::CommerceOrder,
    ),
    InvalidVectorSpec::non_event_route_delivery(
        "route_delivery_family_is_rejected",
        "route_delivery",
    ),
    InvalidVectorSpec::raw(
        "group_event_is_rejected",
        "group",
        RawEventVectorWitness::Group,
    ),
    InvalidVectorSpec::raw(
        "operations_event_is_rejected",
        "operations",
        RawEventVectorWitness::Operations,
    ),
];

#[derive(Clone, Copy)]
struct ValidVectorSpec {
    id: &'static str,
    case_kind: &'static str,
    surface: &'static str,
    leaf_index: usize,
}

impl ValidVectorSpec {
    const fn direct(id: &'static str, leaf_index: usize) -> Self {
        Self {
            id,
            case_kind: "publication_allowlist.allow_artifact.valid",
            surface: "sealed_artifact_json",
            leaf_index,
        }
    }

    const fn canonical(id: &'static str, leaf_index: usize) -> Self {
        Self {
            id,
            case_kind: "publication_allowlist.allow_canonical_json.valid",
            surface: "canonical_artifact_json",
            leaf_index,
        }
    }
}

#[derive(Clone, Copy)]
enum ArtifactVectorWitness {
    UpdateWithAskMarker,
    PhotoUpdateWithAskMarker,
    AskWithoutMarker,
    UpdateWithPhotoProfile,
    TypedDateWithGenericShape,
    TypedTimeWithGenericShape,
    OperationalListingProfile,
    GenericNip99Profile,
    MixedFoodAndOperationalMarkers,
}

#[derive(Clone, Copy)]
enum RawEventVectorWitness {
    Profile,
    GenericRootNote,
    Reply,
    Comment,
    DeletionRequest,
    CalendarDate,
    CalendarTime,
    CalendarCollection,
    CalendarRsvp,
    Bud11Authorization,
    Ephemeral,
    Trade,
    CommerceOrder,
    Group,
    Operations,
}

impl RawEventVectorWitness {
    const fn event_kind(self) -> u32 {
        match self {
            Self::Profile => 0,
            Self::GenericRootNote | Self::Reply => 1,
            Self::Comment => 1_111,
            Self::DeletionRequest => 5,
            Self::CalendarDate => 31_922,
            Self::CalendarTime => 31_923,
            Self::CalendarCollection => 31_924,
            Self::CalendarRsvp => 31_925,
            Self::Bud11Authorization => 24_242,
            Self::Ephemeral => 20_001,
            Self::Trade => 3_470,
            Self::CommerceOrder => 3_422,
            Self::Group => 9_007,
            Self::Operations => 78,
        }
    }
}

#[derive(Clone, Copy)]
enum InvalidVectorWitness {
    Artifact(ArtifactVectorWitness),
    RawEvent(RawEventVectorWitness),
    NonEventRouteDelivery,
}

#[derive(Clone, Copy)]
struct InvalidVectorSpec {
    id: &'static str,
    family: Option<&'static str>,
    source_error: &'static str,
    witness: InvalidVectorWitness,
}

impl InvalidVectorSpec {
    const fn artifact(
        id: &'static str,
        family: Option<&'static str>,
        source_error: &'static str,
        witness: ArtifactVectorWitness,
    ) -> Self {
        Self {
            id,
            family,
            source_error,
            witness: InvalidVectorWitness::Artifact(witness),
        }
    }

    const fn raw(id: &'static str, family: &'static str, witness: RawEventVectorWitness) -> Self {
        Self {
            id,
            family: Some(family),
            source_error: "publication_artifact_invalid_json",
            witness: InvalidVectorWitness::RawEvent(witness),
        }
    }

    const fn non_event_route_delivery(id: &'static str, family: &'static str) -> Self {
        Self {
            id,
            family: Some(family),
            source_error: "publication_artifact_invalid_json",
            witness: InvalidVectorWitness::NonEventRouteDelivery,
        }
    }

    const fn surface(self) -> &'static str {
        match self.witness {
            InvalidVectorWitness::Artifact(_) => "artifact_candidate_json",
            InvalidVectorWitness::RawEvent(_) => "raw_nip01_event_json",
            InvalidVectorWitness::NonEventRouteDelivery => "non_event_product_surface",
        }
    }
}

#[derive(Clone, Copy)]
struct ImmutableArtifactSpec {
    relative: &'static str,
    byte_length: usize,
    sha256: &'static str,
}

impl ImmutableArtifactSpec {
    const fn new(relative: &'static str, byte_length: usize, sha256: &'static str) -> Self {
        Self {
            relative,
            byte_length,
            sha256,
        }
    }
}

#[derive(Clone, Copy)]
struct LeafSpec {
    leaf: &'static str,
    kind: u32,
    publication_role: &'static str,
    authored_operation_id: &'static str,
    event_contract_id: &'static str,
}

impl LeafSpec {
    const fn new(
        leaf: &'static str,
        kind: u32,
        publication_role: &'static str,
        authored_operation_id: &'static str,
        event_contract_id: &'static str,
    ) -> Self {
        Self {
            leaf,
            kind,
            publication_role,
            authored_operation_id,
            event_contract_id,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct FileDescriptor {
    path: String,
    byte_length: u64,
    sha256: String,
    hash_algorithm: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct SourceFileDescriptor {
    role: String,
    path: String,
    byte_length: u64,
    sha256: String,
    hash_algorithm: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct PredecessorDescriptor {
    contract_id: String,
    immutable_artifacts: Vec<FileDescriptor>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct RegistryDescriptor {
    version: u32,
    inventory: FileDescriptor,
    sidecar: FileDescriptor,
    evolution: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct LeafDescriptor {
    leaf: String,
    kind: u32,
    publication_role: String,
    authored_operation_id: String,
    event_contract_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct AllowlistDescriptor {
    version: u32,
    contract_id: String,
    operation_id: String,
    canonical_json_operation_id: String,
    input_type: String,
    output_type: String,
    allowed_leaves: Vec<LeafDescriptor>,
    kind_one_precedence: Vec<String>,
    event_policy: String,
    classified_listing_policy: String,
    denied_families: Vec<String>,
    granted_capability: String,
    excluded_capabilities: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ResultVectorDescriptor {
    canonical_path: String,
    mirror_path: String,
    byte_length: u64,
    sha256: String,
    hash_algorithm: String,
    executor_path: String,
    executor_test: String,
    valid_case_ids: Vec<String>,
    invalid_case_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ReleaseDescriptor {
    record_path: String,
    change_id: String,
    changelog_path: String,
    changelog_marker: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct AllowlistManifest {
    schema_version: u32,
    contract_id: String,
    authority_id: String,
    manifest_schema: FileDescriptor,
    predecessor: PredecessorDescriptor,
    event_contract_registry: RegistryDescriptor,
    allowlist: AllowlistDescriptor,
    predecessor_source_supersessions: Vec<String>,
    source_files: Vec<SourceFileDescriptor>,
    result_vector: ResultVectorDescriptor,
    release: ReleaseDescriptor,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct VectorSuite {
    suite: String,
    contract_version: String,
    vectors: Vec<VectorCase>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct VectorCase {
    id: String,
    kind: String,
    input: Value,
    expected: Value,
}

struct ValidatedVector {
    bytes: Vec<u8>,
    valid_case_ids: Vec<String>,
    invalid_case_ids: Vec<String>,
}

pub(crate) fn write_phase1_publication_allowlist_manifest(
    workspace_root: &Path,
) -> Result<(), String> {
    with_artifact_bundle_transaction(workspace_root, |transaction| {
        transaction.write(expected_artifacts(workspace_root)?)?;
        validate_manifest_under_lock(workspace_root)
    })
}

pub(crate) fn validate_phase1_publication_allowlist_manifest(
    workspace_root: &Path,
) -> Result<(), String> {
    with_artifact_bundle_transaction(workspace_root, |_| {
        validate_manifest_under_lock(workspace_root)
    })
}

fn validate_manifest_under_lock(workspace_root: &Path) -> Result<(), String> {
    validate_immutable_phase1_publication_artifact_predecessor_under_lock(workspace_root)?;
    for artifact in expected_artifacts(workspace_root)? {
        let actual = read_regular_file(workspace_root, artifact.relative)?;
        if actual != artifact.contents {
            return Err(format!(
                "generated Phase 1 publication allowlist contract {} is stale; run {WRITE_COMMAND}",
                artifact.relative
            ));
        }
    }

    let bytes = read_regular_file(workspace_root, MANIFEST_RELATIVE)?;
    let manifest: AllowlistManifest = serde_json::from_slice(&bytes)
        .map_err(|error| format!("parse {MANIFEST_RELATIVE}: {error}"))?;
    validate_canonical_json(MANIFEST_RELATIVE, &bytes, &manifest)?;
    validate_manifest_shape(&manifest)?;

    let schema_bytes = read_regular_file(workspace_root, MANIFEST_SCHEMA_RELATIVE)?;
    let schema: Value = serde_json::from_slice(&schema_bytes)
        .map_err(|error| format!("parse {MANIFEST_SCHEMA_RELATIVE}: {error}"))?;
    validate_canonical_json(MANIFEST_SCHEMA_RELATIVE, &schema_bytes, &schema)?;
    let instance = serde_json::to_value(&manifest)
        .map_err(|error| format!("serialize {MANIFEST_RELATIVE}: {error}"))?;
    validate_json_schema(&schema, &instance)?;

    let sidecar = read_regular_file(workspace_root, MANIFEST_SHA256_RELATIVE)?;
    if sidecar != format!("{}\n", sha256_hex(&bytes)).as_bytes() {
        return Err(format!(
            "{MANIFEST_SHA256_RELATIVE} must authenticate the exact allowlist manifest bytes"
        ));
    }
    Ok(())
}

fn expected_artifacts(workspace_root: &Path) -> Result<Vec<GeneratedArtifact>, String> {
    validate_immutable_phase1_publication_artifact_predecessor_under_lock(workspace_root)?;
    validate_source_contract(workspace_root)?;
    let schema_bytes = canonical_json_bytes(&manifest_schema())?;
    let manifest = describe_manifest(workspace_root, &schema_bytes)?;
    let manifest_bytes = canonical_json_bytes(&manifest)?;
    let sidecar_bytes = format!("{}\n", sha256_hex(&manifest_bytes)).into_bytes();
    let descriptor_bytes = canonical_json_bytes(&json!({
        "schema_version": SCHEMA_VERSION,
        "contract_id": CONTRACT_ID,
        "manifest": descriptor_for_bytes(MANIFEST_RELATIVE, &manifest_bytes)?,
        "manifest_schema": descriptor_for_bytes(MANIFEST_SCHEMA_RELATIVE, &schema_bytes)?,
        "manifest_sidecar": descriptor_for_bytes(MANIFEST_SHA256_RELATIVE, &sidecar_bytes)?,
        "predecessor_manifest_sha256": PREDECESSOR_ARTIFACTS[0].sha256,
        "event_contract_registry_v7_sha256": REGISTRY_SHA256,
    }))?;
    let vector_bytes = read_regular_file(workspace_root, VECTOR_RELATIVE)?;
    Ok(vec![
        GeneratedArtifact {
            relative: MANIFEST_RELATIVE,
            contents: manifest_bytes,
        },
        GeneratedArtifact {
            relative: MANIFEST_SCHEMA_RELATIVE,
            contents: schema_bytes,
        },
        GeneratedArtifact {
            relative: MANIFEST_SHA256_RELATIVE,
            contents: sidecar_bytes,
        },
        GeneratedArtifact {
            relative: GENERATED_DESCRIPTOR_RELATIVE,
            contents: descriptor_bytes,
        },
        GeneratedArtifact {
            relative: VECTOR_MIRROR_RELATIVE,
            contents: vector_bytes,
        },
    ])
}

fn describe_manifest(
    workspace_root: &Path,
    schema_bytes: &[u8],
) -> Result<AllowlistManifest, String> {
    let source_files = successor_source_specs(workspace_root)?
        .into_iter()
        .map(|(role, path)| source_descriptor(workspace_root, &role, &path))
        .collect::<Result<Vec<_>, _>>()?;
    let vector = validate_vector(workspace_root)?;
    Ok(AllowlistManifest {
        schema_version: SCHEMA_VERSION,
        contract_id: CONTRACT_ID.to_owned(),
        authority_id: AUTHORITY_ID.to_owned(),
        manifest_schema: descriptor_for_bytes(MANIFEST_SCHEMA_RELATIVE, schema_bytes)?,
        predecessor: PredecessorDescriptor {
            contract_id: PREDECESSOR_CONTRACT_ID.to_owned(),
            immutable_artifacts: PREDECESSOR_ARTIFACTS
                .iter()
                .map(|spec| FileDescriptor {
                    path: spec.relative.to_owned(),
                    byte_length: spec.byte_length as u64,
                    sha256: spec.sha256.to_owned(),
                    hash_algorithm: HASH_ALGORITHM.to_owned(),
                })
                .collect(),
        },
        event_contract_registry: RegistryDescriptor {
            version: RADROOTS_PHASE1_PUBLICATION_ALLOWLIST_REGISTRY_VERSION,
            inventory: descriptor_for_file(workspace_root, REGISTRY_RELATIVE)?,
            sidecar: descriptor_for_file(workspace_root, REGISTRY_SIDECAR_RELATIVE)?,
            evolution: "immutable_registry_v7_plus_additive_phase1_publication_allowlist_v1"
                .to_owned(),
        },
        allowlist: expected_allowlist_descriptor(),
        predecessor_source_supersessions: PUBLICATION_SUCCESSOR_SUPERSEDED_PATHS
            .iter()
            .map(|path| (*path).to_owned())
            .collect(),
        source_files,
        result_vector: ResultVectorDescriptor {
            canonical_path: VECTOR_RELATIVE.to_owned(),
            mirror_path: VECTOR_MIRROR_RELATIVE.to_owned(),
            byte_length: vector.bytes.len() as u64,
            sha256: sha256_hex(&vector.bytes),
            hash_algorithm: HASH_ALGORITHM.to_owned(),
            executor_path: VECTOR_EXECUTOR_RELATIVE.to_owned(),
            executor_test: VECTOR_EXECUTOR_TEST.to_owned(),
            valid_case_ids: vector.valid_case_ids,
            invalid_case_ids: vector.invalid_case_ids,
        },
        release: ReleaseDescriptor {
            record_path: RELEASE_RELATIVE.to_owned(),
            change_id: RELEASE_CHANGE_ID.to_owned(),
            changelog_path: CHANGELOG_RELATIVE.to_owned(),
            changelog_marker: CHANGELOG_MARKER.to_owned(),
        },
    })
}

fn expected_allowlist_descriptor() -> AllowlistDescriptor {
    AllowlistDescriptor {
        version: RADROOTS_PHASE1_PUBLICATION_ALLOWLIST_VERSION,
        contract_id: RADROOTS_PHASE1_PUBLICATION_ALLOWLIST_CONTRACT_ID.to_owned(),
        operation_id: OPERATION_ID.to_owned(),
        canonical_json_operation_id: CANONICAL_JSON_OPERATION_ID.to_owned(),
        input_type: "RadrootsPhase1PublicationArtifact".to_owned(),
        output_type: "RadrootsPhase1AllowlistedPublicationArtifact".to_owned(),
        allowed_leaves: LEAVES
            .iter()
            .map(|leaf| LeafDescriptor {
                leaf: leaf.leaf.to_owned(),
                kind: leaf.kind,
                publication_role: leaf.publication_role.to_owned(),
                authored_operation_id: leaf.authored_operation_id.to_owned(),
                event_contract_id: leaf.event_contract_id.to_owned(),
            })
            .collect(),
        kind_one_precedence: ["ask", "photo_update", "update"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        event_policy: "strict_typed_nip52_date_or_time_artifact_only_v1".to_owned(),
        classified_listing_policy: "raw_marker_partition_before_focused_food_profile_validation_v1"
            .to_owned(),
        denied_families: DENIED_FAMILIES
            .iter()
            .map(|family| (*family).to_owned())
            .collect(),
        granted_capability: "phase1_durable_publication_lane_entry_only_v1".to_owned(),
        excluded_capabilities: [
            "signing",
            "media_upload",
            "media_retrieval",
            "media_readiness",
            "relay_transport",
            "product_entitlement",
            "inbound_registry_admission",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
    }
}

fn validate_source_contract(workspace_root: &Path) -> Result<(), String> {
    validate_registry_identity(workspace_root)?;
    validate_operations_authority(workspace_root)?;
    validate_release_authority(workspace_root)?;
    validate_vector(workspace_root)?;

    let paths = successor_source_specs(workspace_root)?
        .into_iter()
        .map(|(_, path)| path)
        .collect::<BTreeSet<_>>();
    for path in PUBLICATION_SUCCESSOR_SUPERSEDED_PATHS {
        if !paths.contains(*path) {
            return Err(format!(
                "allowlist successor does not bind superseded predecessor source {path}"
            ));
        }
    }
    for path in GENERATED_ARTIFACT_PATHS {
        if paths.contains(*path) {
            return Err(format!(
                "allowlist source inventory recursively includes generated artifact {path}"
            ));
        }
    }
    Ok(())
}

fn validate_registry_identity(workspace_root: &Path) -> Result<(), String> {
    for (path, expected_len, expected_sha) in [
        (REGISTRY_RELATIVE, REGISTRY_BYTE_LENGTH, REGISTRY_SHA256),
        (
            REGISTRY_SIDECAR_RELATIVE,
            REGISTRY_SIDECAR_BYTE_LENGTH,
            REGISTRY_SIDECAR_SHA256,
        ),
    ] {
        let bytes = read_regular_file(workspace_root, path)?;
        if bytes.len() != expected_len || sha256_hex(&bytes) != expected_sha {
            return Err(format!(
                "immutable event-contract registry-v7 artifact {path} drifted"
            ));
        }
    }
    let inventory = read_regular_file(workspace_root, REGISTRY_RELATIVE)?;
    let value: Value = serde_json::from_slice(&inventory)
        .map_err(|error| format!("parse {REGISTRY_RELATIVE}: {error}"))?;
    if value
        .get("event_contract_registry_version")
        .and_then(Value::as_u64)
        != Some(7)
    {
        return Err("event-contract registry-v7 identity drifted".to_owned());
    }
    let sidecar = read_regular_file(workspace_root, REGISTRY_SIDECAR_RELATIVE)?;
    if sidecar != format!("{}\n", sha256_hex(&inventory)).as_bytes() {
        return Err(
            "event-contract registry-v7 sidecar does not authenticate inventory".to_owned(),
        );
    }
    Ok(())
}

fn validate_operations_authority(workspace_root: &Path) -> Result<(), String> {
    let manifest = parse_toml(workspace_root, OPERATIONS_RELATIVE)?;
    let public_types = toml_string_array(
        "shared_types.public",
        manifest
            .get("shared_types")
            .and_then(|value| value.get("public")),
    )?;
    for required in PUBLIC_TYPES {
        if public_types
            .iter()
            .filter(|candidate| candidate.as_str() == *required)
            .count()
            != 1
        {
            return Err(format!(
                "shared public types must contain {required} exactly once"
            ));
        }
    }
    let operations = manifest
        .get("operations")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| "operations.toml has no operations table".to_owned())?;
    for (key, operation_id, input) in [
        (
            "phase1_publication_allowlist_allow_artifact",
            OPERATION_ID,
            "RadrootsPhase1PublicationArtifact",
        ),
        (
            "phase1_publication_allowlist_allow_canonical_json",
            CANONICAL_JSON_OPERATION_ID,
            "Bytes",
        ),
    ] {
        let operation = operations
            .get(key)
            .and_then(toml::Value::as_table)
            .ok_or_else(|| format!("operations.toml is missing {key}"))?;
        let case_kinds: &[&str] = if operation_id == OPERATION_ID {
            &["publication_allowlist.allow_artifact.valid"]
        } else {
            &[
                "publication_allowlist.allow_canonical_json.valid",
                "publication_allowlist.allow_canonical_json.invalid",
            ]
        };
        validate_allowlist_operation(operation, operation_id, input, case_kinds)?;
    }
    Ok(())
}

fn validate_allowlist_operation(
    operation: &toml::map::Map<String, toml::Value>,
    operation_id: &str,
    input: &str,
    case_kinds: &[&str],
) -> Result<(), String> {
    for (field, expected) in [
        ("domain", "publication"),
        ("id", operation_id),
        ("stability", "beta"),
        ("error_class", "validation_error"),
        ("signing", "none"),
        ("transport", "none"),
    ] {
        if operation.get(field).and_then(toml::Value::as_str) != Some(expected) {
            return Err(format!(
                "publication allowlist operation {field} must be {expected}"
            ));
        }
    }
    if operation
        .get("deterministic")
        .and_then(toml::Value::as_bool)
        != Some(true)
        || toml_string_array("allowlist inputs", operation.get("inputs"))? != vec![input.to_owned()]
        || toml_string_array("allowlist outputs", operation.get("outputs"))?
            != vec!["RadrootsPhase1AllowlistedPublicationArtifact".to_owned()]
    {
        return Err("publication allowlist signature or determinism drifted".to_owned());
    }
    let implementation = operation
        .get("implementation")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| "publication allowlist implementation is missing".to_owned())?;
    let modules = toml_string_array(
        "allowlist implementation modules",
        implementation.get("rust_modules"),
    )?;
    for required in [
        "crates/event_codec/src/wire/publication.rs",
        "crates/event_codec/src/wire/publication/allowlist.rs",
        "crates/event_codec/src/post/inbound/registry_v7.rs",
        "crates/event_codec/src/food_availability/inbound/registry_v7.rs",
    ] {
        if !modules.iter().any(|module| module == required) {
            return Err(format!(
                "publication allowlist implementation is missing {required}"
            ));
        }
    }
    let conformance = operation
        .get("conformance")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| "publication allowlist conformance is missing".to_owned())?;
    if conformance.get("vector").and_then(toml::Value::as_str) != Some(VECTOR_RELATIVE)
        || toml_string_array("allowlist case kinds", conformance.get("case_kinds"))?
            != case_kinds
                .iter()
                .map(|case_kind| (*case_kind).to_owned())
                .collect::<Vec<_>>()
    {
        return Err("publication allowlist conformance authority drifted".to_owned());
    }
    Ok(())
}

fn validate_release_authority(workspace_root: &Path) -> Result<(), String> {
    let release = parse_toml(workspace_root, RELEASE_RELATIVE)?;
    let changes = release
        .get("changes")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| format!("{RELEASE_RELATIVE} has no changes"))?;
    let matches = changes
        .iter()
        .filter(|change| change.get("id").and_then(toml::Value::as_str) == Some(RELEASE_CHANGE_ID))
        .collect::<Vec<_>>();
    if matches.len() != 1
        || matches[0]
            .get("classification")
            .and_then(toml::Value::as_str)
            != Some("feature")
    {
        return Err(format!(
            "{RELEASE_RELATIVE} must contain one feature change {RELEASE_CHANGE_ID}"
        ));
    }
    let impacts = toml_string_array("allowlist semver impacts", matches[0].get("semver_impacts"))?;
    for required in [
        "add_exported_type",
        "add_exported_function",
        "add_exported_constant",
        "add_conformance_vector",
    ] {
        if !impacts.iter().any(|impact| impact == required) {
            return Err(format!("allowlist release change is missing {required}"));
        }
    }
    let changelog = String::from_utf8(read_regular_file(workspace_root, CHANGELOG_RELATIVE)?)
        .map_err(|error| format!("{CHANGELOG_RELATIVE} must be UTF-8: {error}"))?;
    if changelog.matches(CHANGELOG_MARKER).count() != 1 {
        return Err(format!(
            "{CHANGELOG_RELATIVE} must contain {CHANGELOG_MARKER} exactly once"
        ));
    }
    Ok(())
}

fn validate_vector(workspace_root: &Path) -> Result<ValidatedVector, String> {
    let bytes = read_regular_file(workspace_root, VECTOR_RELATIVE)?;
    let suite: VectorSuite = serde_json::from_slice(&bytes)
        .map_err(|error| format!("parse {VECTOR_RELATIVE}: {error}"))?;
    let canonical = canonical_json_bytes(
        &serde_json::from_slice::<Value>(&bytes)
            .map_err(|error| format!("parse {VECTOR_RELATIVE} as JSON value: {error}"))?,
    )?;
    if canonical != bytes {
        return Err(format!("{VECTOR_RELATIVE} must be canonical pretty JSON"));
    }
    if suite.suite != "phase1_publication_allowlist" || suite.contract_version != "1.0.0" {
        return Err(format!("{VECTOR_RELATIVE} identity drifted"));
    }
    let expected_ids = VALID_VECTOR_SPECS
        .iter()
        .map(|spec| spec.id)
        .chain(INVALID_VECTOR_SPECS.iter().map(|spec| spec.id))
        .collect::<Vec<_>>();
    let actual_ids = suite
        .vectors
        .iter()
        .map(|vector| vector.id.as_str())
        .collect::<Vec<_>>();
    if actual_ids != expected_ids {
        return Err(format!("{VECTOR_RELATIVE} case inventory or order drifted"));
    }
    if actual_ids.iter().copied().collect::<BTreeSet<_>>().len() != actual_ids.len() {
        return Err(format!("{VECTOR_RELATIVE} case ids must be unique"));
    }
    let mut denied_families = BTreeSet::new();
    for (index, vector) in suite.vectors.iter().enumerate() {
        let input = vector
            .input
            .as_object()
            .ok_or_else(|| format!("{} input must be an object", vector.id))?;
        let expected = vector
            .expected
            .as_object()
            .ok_or_else(|| format!("{} expected must be an object", vector.id))?;
        let canonical_json = input
            .get("canonical_json")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("{} must carry exact canonical_json", vector.id))?;
        let candidate: Value = serde_json::from_str(canonical_json)
            .map_err(|error| format!("{} canonical_json is invalid: {error}", vector.id))?;
        if let Some(spec) = VALID_VECTOR_SPECS.get(index).copied() {
            let leaf = LEAVES[spec.leaf_index];
            let input_keys = input.keys().map(String::as_str).collect::<BTreeSet<_>>();
            let expected_input_keys = ["canonical_json", "surface"]
                .into_iter()
                .collect::<BTreeSet<_>>();
            if input.len() != 2
                || input_keys != expected_input_keys
                || vector.kind != spec.case_kind
                || input.get("surface").and_then(Value::as_str) != Some(spec.surface)
                || expected.len() != 3
                || expected.get("decision").and_then(Value::as_str) != Some("allow")
                || expected.get("leaf").and_then(Value::as_str) != Some(leaf.leaf)
                || expected.get("event_kind").and_then(Value::as_u64) != Some(u64::from(leaf.kind))
                || candidate.get("semantic_variant").and_then(Value::as_str) != Some(leaf.leaf)
                || candidate
                    .get("authored_operation_id")
                    .and_then(Value::as_str)
                    != Some(leaf.authored_operation_id)
                || candidate.get("event_contract_id").and_then(Value::as_str)
                    != Some(leaf.event_contract_id)
                || candidate.pointer("/draft/kind").and_then(Value::as_u64)
                    != Some(u64::from(leaf.kind))
            {
                return Err(format!("{} allowed leaf descriptor drifted", vector.id));
            }
            validate_artifact_envelope_keys(&vector.id, &candidate)?;
            if spec.case_kind == "publication_allowlist.allow_canonical_json.valid"
                && canonical_json
                    != suite.vectors[spec.leaf_index]
                        .input
                        .get("canonical_json")
                        .and_then(Value::as_str)
                        .ok_or_else(|| {
                            format!("{} direct-operation counterpart is missing", vector.id)
                        })?
            {
                return Err(format!(
                    "{} must reuse the exact direct-operation artifact bytes",
                    vector.id
                ));
            }
        } else {
            let spec = INVALID_VECTOR_SPECS[index - VALID_VECTOR_SPECS.len()];
            let family = input.get("family").and_then(Value::as_str);
            if vector.kind != "publication_allowlist.allow_canonical_json.invalid"
                || input.get("surface").and_then(Value::as_str) != Some(spec.surface())
                || family != spec.family
                || expected.len() != 3
                || expected.get("decision").and_then(Value::as_str) != Some("reject")
                || expected.get("error").and_then(Value::as_str)
                    != Some("publication_allowlist_artifact_invalid")
                || expected.get("source_error").and_then(Value::as_str) != Some(spec.source_error)
            {
                return Err(format!("{} rejection authority drifted", vector.id));
            }
            if let Some(family) = family {
                denied_families.insert(family);
            }
            validate_invalid_vector_evidence(&vector.id, spec, input, &candidate)?;
        }
    }
    let expected_denied = DENIED_FAMILIES.iter().copied().collect::<BTreeSet<_>>();
    if denied_families != expected_denied {
        return Err(format!(
            "{VECTOR_RELATIVE} denied-family evidence does not match the declared taxonomy"
        ));
    }
    Ok(ValidatedVector {
        bytes,
        valid_case_ids: VALID_VECTOR_SPECS
            .iter()
            .map(|spec| spec.id.to_owned())
            .collect(),
        invalid_case_ids: INVALID_VECTOR_SPECS
            .iter()
            .map(|spec| spec.id.to_owned())
            .collect(),
    })
}

fn validate_invalid_vector_evidence(
    case_id: &str,
    spec: InvalidVectorSpec,
    input: &serde_json::Map<String, Value>,
    candidate: &Value,
) -> Result<(), String> {
    match spec.witness {
        InvalidVectorWitness::Artifact(witness) => {
            let expected_input_len = 2 + usize::from(spec.family.is_some());
            if input.len() != expected_input_len || input.get("event_kind").is_some() {
                return Err(format!("{case_id} artifact input evidence drifted"));
            }
            validate_artifact_envelope_keys(case_id, candidate)?;
            validate_artifact_vector_witness(case_id, witness, candidate)
        }
        InvalidVectorWitness::RawEvent(witness) => {
            let event_kind = input
                .get("event_kind")
                .and_then(Value::as_u64)
                .ok_or_else(|| format!("{case_id} raw event kind is missing"))?;
            if input.len() != 4 || event_kind != u64::from(witness.event_kind()) {
                return Err(format!("{case_id} raw event input evidence drifted"));
            }
            validate_raw_event_vector_witness(case_id, witness, candidate)
        }
        InvalidVectorWitness::NonEventRouteDelivery => {
            let input_keys = input.keys().map(String::as_str).collect::<BTreeSet<_>>();
            let expected_input_keys = ["canonical_json", "family", "surface"]
                .into_iter()
                .collect::<BTreeSet<_>>();
            if input.len() != 3
                || input_keys != expected_input_keys
                || candidate != &json!({"product_surface": "route_delivery"})
            {
                return Err(format!(
                    "{case_id} must witness route/delivery as an exact non-event product surface"
                ));
            }
            Ok(())
        }
    }
}

fn validate_artifact_envelope_keys(case_id: &str, candidate: &Value) -> Result<(), String> {
    let candidate = candidate
        .as_object()
        .ok_or_else(|| format!("{case_id} artifact candidate must be an object"))?;
    let actual_keys = candidate
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let expected_keys = [
        "schema_version",
        "semantic_variant",
        "authored_operation_id",
        "event_contract_id",
        "expected_author",
        "draft",
        "expected_event_id",
        "media_references",
        "artifact_digest",
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    if actual_keys != expected_keys {
        return Err(format!("{case_id} artifact envelope evidence drifted"));
    }
    Ok(())
}

struct ArtifactWitnessDescriptor {
    semantic_variant: &'static str,
    authored_operation_id: &'static str,
    event_contract_id: &'static str,
    event_kind: u32,
    tag_names: &'static [&'static str],
    content: &'static str,
    media_reference_count: usize,
}

fn artifact_witness_descriptor(witness: ArtifactVectorWitness) -> ArtifactWitnessDescriptor {
    match witness {
        ArtifactVectorWitness::UpdateWithAskMarker => ArtifactWitnessDescriptor {
            semantic_variant: "update",
            authored_operation_id: "social.update.build_authored_draft",
            event_contract_id: "radroots.social.update.v1",
            event_kind: 1,
            tag_names: &["t"],
            content: "Carrots harvested today",
            media_reference_count: 0,
        },
        ArtifactVectorWitness::PhotoUpdateWithAskMarker => ArtifactWitnessDescriptor {
            semantic_variant: "photo_update",
            authored_operation_id: "social.photo_update.build_authored_draft",
            event_contract_id: "radroots.social.photo_update.v1",
            event_kind: 1,
            tag_names: &["t", "imeta"],
            content: "Strawberries at the farm stand https://media.example/51bcf96cda2475475246e3a994aabfee7ff9d7694ba0db9bdc74408632827ad0.webp",
            media_reference_count: 2,
        },
        ArtifactVectorWitness::AskWithoutMarker => ArtifactWitnessDescriptor {
            semantic_variant: "ask",
            authored_operation_id: "social.ask.build_authored_draft",
            event_contract_id: "radroots.social.ask.v1",
            event_kind: 1,
            tag_names: &["imeta"],
            content: "When will strawberries be ready? https://media.example/51bcf96cda2475475246e3a994aabfee7ff9d7694ba0db9bdc74408632827ad0.webp",
            media_reference_count: 2,
        },
        ArtifactVectorWitness::UpdateWithPhotoProfile => ArtifactWitnessDescriptor {
            semantic_variant: "update",
            authored_operation_id: "social.update.build_authored_draft",
            event_contract_id: "radroots.social.update.v1",
            event_kind: 1,
            tag_names: &["imeta"],
            content: "Strawberries at the farm stand https://media.example/51bcf96cda2475475246e3a994aabfee7ff9d7694ba0db9bdc74408632827ad0.webp",
            media_reference_count: 2,
        },
        ArtifactVectorWitness::TypedDateWithGenericShape => ArtifactWitnessDescriptor {
            semantic_variant: "event_date",
            authored_operation_id: "social.calendar_date_event.build_authored_draft",
            event_contract_id: "radroots.calendar.date_event.v1",
            event_kind: 31_922,
            tag_names: &["d", "title"],
            content: "Saturday market in Victoria",
            media_reference_count: 0,
        },
        ArtifactVectorWitness::TypedTimeWithGenericShape => ArtifactWitnessDescriptor {
            semantic_variant: "event_time",
            authored_operation_id: "social.calendar_time_event.build_authored_draft",
            event_contract_id: "radroots.calendar.time_event.v1",
            event_kind: 31_923,
            tag_names: &["d", "title"],
            content: "A one-hour farm tour",
            media_reference_count: 0,
        },
        ArtifactVectorWitness::OperationalListingProfile => ArtifactWitnessDescriptor {
            semantic_variant: "food_availability",
            authored_operation_id: "food_availability.build_authored_draft",
            event_contract_id: "radroots.food.availability.v1",
            event_kind: 30_402,
            tag_names: &[
                "d",
                "title",
                "summary",
                "published_at",
                "location",
                "price",
                "status",
                "image",
                "radroots:price",
            ],
            content: "Fresh Nantes carrots available this week.",
            media_reference_count: 1,
        },
        ArtifactVectorWitness::GenericNip99Profile => ArtifactWitnessDescriptor {
            semantic_variant: "food_availability",
            authored_operation_id: "food_availability.build_authored_draft",
            event_contract_id: "radroots.food.availability.v1",
            event_kind: 30_402,
            tag_names: &[
                "d",
                "title",
                "summary",
                "published_at",
                "location",
                "price",
                "status",
                "image",
            ],
            content: "Fresh Nantes carrots available this week.",
            media_reference_count: 1,
        },
        ArtifactVectorWitness::MixedFoodAndOperationalMarkers => ArtifactWitnessDescriptor {
            semantic_variant: "food_availability",
            authored_operation_id: "food_availability.build_authored_draft",
            event_contract_id: "radroots.food.availability.v1",
            event_kind: 30_402,
            tag_names: &[
                "d",
                "title",
                "summary",
                "published_at",
                "location",
                "price",
                "radroots:price_unit",
                "radroots:quantity",
                "status",
                "image",
                "radroots:price",
            ],
            content: "Fresh Nantes carrots available this week.",
            media_reference_count: 1,
        },
    }
}

fn validate_artifact_vector_witness(
    case_id: &str,
    witness: ArtifactVectorWitness,
    candidate: &Value,
) -> Result<(), String> {
    let descriptor = artifact_witness_descriptor(witness);
    let tags = candidate
        .pointer("/draft/tags")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{case_id} artifact tags are missing"))?;
    let media_references = candidate
        .get("media_references")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{case_id} media references are missing"))?;
    if candidate.get("schema_version").and_then(Value::as_u64) != Some(1)
        || candidate.get("semantic_variant").and_then(Value::as_str)
            != Some(descriptor.semantic_variant)
        || candidate
            .get("authored_operation_id")
            .and_then(Value::as_str)
            != Some(descriptor.authored_operation_id)
        || candidate.get("event_contract_id").and_then(Value::as_str)
            != Some(descriptor.event_contract_id)
        || candidate.get("expected_author").and_then(Value::as_str) != Some(VECTOR_AUTHOR)
        || candidate.pointer("/draft/kind").and_then(Value::as_u64)
            != Some(u64::from(descriptor.event_kind))
        || candidate
            .pointer("/draft/created_at")
            .and_then(Value::as_u64)
            != Some(VECTOR_CREATED_AT)
        || candidate.pointer("/draft/content").and_then(Value::as_str) != Some(descriptor.content)
        || media_references.len() != descriptor.media_reference_count
        || candidate.get("artifact_digest").and_then(Value::as_str) != Some(VECTOR_ZERO_ID)
    {
        return Err(format!("{case_id} claimed artifact profile drifted"));
    }
    validate_exact_tag_names(case_id, tags, descriptor.tag_names)?;

    match witness {
        ArtifactVectorWitness::UpdateWithAskMarker => {
            require_exact_tag(case_id, tags, &["t", "radroots-ask"])?;
        }
        ArtifactVectorWitness::PhotoUpdateWithAskMarker => {
            require_exact_tag(case_id, tags, &["t", "radroots-ask"])?;
            require_exact_tag(case_id, tags, VECTOR_PHOTO_IMETA_FIELDS)?;
        }
        ArtifactVectorWitness::AskWithoutMarker | ArtifactVectorWitness::UpdateWithPhotoProfile => {
            forbid_tag_name(case_id, tags, "t")?;
            require_exact_tag(case_id, tags, VECTOR_PHOTO_IMETA_FIELDS)?;
        }
        ArtifactVectorWitness::TypedDateWithGenericShape
        | ArtifactVectorWitness::TypedTimeWithGenericShape => {
            require_exact_tag(case_id, tags, &["d", "generic-calendar"])?;
            require_exact_tag(case_id, tags, &["title", "Generic calendar event"])?;
        }
        ArtifactVectorWitness::OperationalListingProfile => {
            require_exact_tag(case_id, tags, &["radroots:price", "3", "CAD"])?;
            forbid_tag_name(case_id, tags, "radroots:price_unit")?;
            forbid_tag_name(case_id, tags, "radroots:quantity")?;
        }
        ArtifactVectorWitness::GenericNip99Profile => {
            forbid_tag_name(case_id, tags, "radroots:price")?;
            forbid_tag_name(case_id, tags, "radroots:price_unit")?;
            forbid_tag_name(case_id, tags, "radroots:quantity")?;
        }
        ArtifactVectorWitness::MixedFoodAndOperationalMarkers => {
            require_exact_tag(case_id, tags, &["radroots:price", "3", "CAD"])?;
            require_exact_tag(case_id, tags, &["radroots:price_unit", "lb"])?;
            require_exact_tag(case_id, tags, &["radroots:quantity", "24", "lb"])?;
        }
    }
    Ok(())
}

fn validate_raw_event_vector_witness(
    case_id: &str,
    witness: RawEventVectorWitness,
    candidate: &Value,
) -> Result<(), String> {
    let candidate = candidate
        .as_object()
        .ok_or_else(|| format!("{case_id} raw event must be an object"))?;
    let actual_keys = candidate
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let expected_keys = [
        "id",
        "pubkey",
        "created_at",
        "kind",
        "tags",
        "content",
        "sig",
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    let (expected_tags, expected_content) = match witness {
        RawEventVectorWitness::Profile => (
            json!([]),
            "{\"name\":\"victoria-farm\",\"display_name\":\"Victoria Farm\",\"about\":\"Seasonal produce from the Saanich Peninsula\",\"picture\":\"https://media.example/512f6a371c77694502e7d08f0b1f1080c7103ca90925bfe2fa23106aac11003a.png\",\"banner\":\"https://media.example/fbf3819415a76ea0d3ba71817578bb89c4903aa958e38f76f86578dfa8a35145.webp\",\"nip05\":\"farm@example.com\",\"bot\":false}",
        ),
        RawEventVectorWitness::GenericRootNote => (json!([]), "Generic root note"),
        RawEventVectorWitness::Reply => (
            json!([[
                "e",
                "1111111111111111111111111111111111111111111111111111111111111111",
                "",
                "reply"
            ]]),
            "Reply",
        ),
        RawEventVectorWitness::Comment => (
            json!([[
                "E",
                "1111111111111111111111111111111111111111111111111111111111111111"
            ]]),
            "Comment",
        ),
        RawEventVectorWitness::DeletionRequest => (
            json!([[
                "e",
                "1111111111111111111111111111111111111111111111111111111111111111"
            ]]),
            "Removed",
        ),
        RawEventVectorWitness::CalendarDate => (
            json!([
                ["d", "farmers-market-2026"],
                ["title", "Moss Street Farmers Market"],
                ["start", "2026-07-25"],
                ["end", "2026-07-26"],
                ["location", "Victoria, BC"],
                [
                    "image",
                    "https://events.example/0a422cbf828d421341c40c678f4cfbd6451841760db126e5f5ac3d2e06fd80b8.jpeg"
                ]
            ]),
            "Saturday market in Victoria",
        ),
        RawEventVectorWitness::CalendarTime => (
            json!([
                ["d", "farm-tour-2026"],
                ["title", "Saanich Farm Tour"],
                ["start", "1785003600"],
                ["end", "1785007200"],
                ["D", "20659"],
                ["start_tzid", "America/Vancouver"],
                [
                    "image",
                    "https://events.example/0a422cbf828d421341c40c678f4cfbd6451841760db126e5f5ac3d2e06fd80b8.jpeg"
                ]
            ]),
            "A one-hour farm tour",
        ),
        RawEventVectorWitness::CalendarCollection => (json!([["d", "victoria-calendar"]]), ""),
        RawEventVectorWitness::CalendarRsvp => (
            json!([[
                "a",
                "31922:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:market"
            ]]),
            "accepted",
        ),
        RawEventVectorWitness::Bud11Authorization => (json!([["t", "upload"]]), ""),
        RawEventVectorWitness::Ephemeral => (json!([]), "Ephemeral"),
        RawEventVectorWitness::Trade => (json!([["d", "trade-proposal"]]), ""),
        RawEventVectorWitness::CommerceOrder => (json!([["d", "order-request"]]), ""),
        RawEventVectorWitness::Group => (json!([["h", "victoria-growers"]]), ""),
        RawEventVectorWitness::Operations => (json!([["d", "internal-operation"]]), ""),
    };
    if actual_keys != expected_keys
        || candidate.get("id").and_then(Value::as_str) != Some(VECTOR_ZERO_ID)
        || candidate.get("pubkey").and_then(Value::as_str) != Some(VECTOR_AUTHOR)
        || candidate.get("created_at").and_then(Value::as_u64) != Some(VECTOR_CREATED_AT)
        || candidate.get("kind").and_then(Value::as_u64) != Some(u64::from(witness.event_kind()))
        || candidate.get("tags") != Some(&expected_tags)
        || candidate.get("content").and_then(Value::as_str) != Some(expected_content)
        || candidate.get("sig").and_then(Value::as_str) != Some(VECTOR_ZERO_SIGNATURE)
    {
        return Err(format!(
            "{case_id} raw event kind or exact required/forbidden tag shape drifted"
        ));
    }
    Ok(())
}

fn validate_exact_tag_names(
    case_id: &str,
    tags: &[Value],
    expected_names: &[&str],
) -> Result<(), String> {
    let names = tags
        .iter()
        .map(|tag| tag.get(0).and_then(Value::as_str))
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| format!("{case_id} contains an empty or non-string tag"))?;
    if names != expected_names {
        return Err(format!("{case_id} exact tag-name sequence drifted"));
    }
    Ok(())
}

fn require_exact_tag(case_id: &str, tags: &[Value], expected: &[&str]) -> Result<(), String> {
    if tags.iter().any(|tag| {
        tag.as_array().is_some_and(|fields| {
            fields.len() == expected.len()
                && fields
                    .iter()
                    .zip(expected)
                    .all(|(actual, expected)| actual.as_str() == Some(*expected))
        })
    }) {
        Ok(())
    } else {
        Err(format!("{case_id} is missing its exact semantic marker"))
    }
}

fn forbid_tag_name(case_id: &str, tags: &[Value], forbidden: &str) -> Result<(), String> {
    if tags
        .iter()
        .any(|tag| tag.get(0).and_then(Value::as_str) == Some(forbidden))
    {
        Err(format!("{case_id} contains forbidden marker {forbidden}"))
    } else {
        Ok(())
    }
}

fn successor_source_specs(workspace_root: &Path) -> Result<Vec<(String, String)>, String> {
    let mut specs = source_specs(workspace_root)?;
    specs.extend([
        (
            "publication_allowlist_vector_executor".to_owned(),
            VECTOR_EXECUTOR_RELATIVE.to_owned(),
        ),
        (
            "publication_allowlist_contract_governance".to_owned(),
            "tools/xtask/src/contract/phase1_publication_allowlist.rs".to_owned(),
        ),
    ]);
    specs.sort_by(|left, right| left.1.cmp(&right.1));
    let mut seen = BTreeSet::new();
    for (_, path) in &specs {
        if !seen.insert(path.as_str()) {
            return Err(format!(
                "publication allowlist source inventory duplicates {path}"
            ));
        }
    }
    Ok(specs)
}

fn validate_manifest_shape(manifest: &AllowlistManifest) -> Result<(), String> {
    if manifest.schema_version != SCHEMA_VERSION
        || manifest.contract_id != CONTRACT_ID
        || manifest.authority_id != AUTHORITY_ID
        || manifest.predecessor.contract_id != PREDECESSOR_CONTRACT_ID
        || manifest.allowlist != expected_allowlist_descriptor()
        || manifest.predecessor_source_supersessions
            != PUBLICATION_SUCCESSOR_SUPERSEDED_PATHS
                .iter()
                .map(|path| (*path).to_owned())
                .collect::<Vec<_>>()
        || manifest.result_vector.canonical_path != VECTOR_RELATIVE
        || manifest.result_vector.mirror_path != VECTOR_MIRROR_RELATIVE
        || manifest.release.change_id != RELEASE_CHANGE_ID
    {
        return Err(format!("{MANIFEST_RELATIVE} shape drifted"));
    }
    let expected_predecessor = PREDECESSOR_ARTIFACTS
        .iter()
        .map(|spec| (spec.relative, spec.byte_length as u64, spec.sha256))
        .collect::<Vec<_>>();
    let actual_predecessor = manifest
        .predecessor
        .immutable_artifacts
        .iter()
        .map(|descriptor| {
            (
                descriptor.path.as_str(),
                descriptor.byte_length,
                descriptor.sha256.as_str(),
            )
        })
        .collect::<Vec<_>>();
    if actual_predecessor != expected_predecessor
        || manifest.event_contract_registry.version != 7
        || manifest.event_contract_registry.inventory.byte_length != REGISTRY_BYTE_LENGTH as u64
        || manifest.event_contract_registry.inventory.sha256 != REGISTRY_SHA256
        || manifest.event_contract_registry.sidecar.byte_length
            != REGISTRY_SIDECAR_BYTE_LENGTH as u64
        || manifest.event_contract_registry.sidecar.sha256 != REGISTRY_SIDECAR_SHA256
    {
        return Err(format!(
            "{MANIFEST_RELATIVE} predecessor or registry identity drifted"
        ));
    }
    let mut paths = BTreeSet::new();
    for source in &manifest.source_files {
        if source.hash_algorithm != HASH_ALGORITHM || !paths.insert(source.path.as_str()) {
            return Err(format!("{MANIFEST_RELATIVE} source inventory is invalid"));
        }
    }
    Ok(())
}

fn manifest_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://radroots.org/contracts/event-codec/phase1-publication-allowlist-v1.schema.json",
        "title": "Radroots Phase 1 Publication Allowlist Contract",
        "type": "object",
        "additionalProperties": false,
        "required": [
            "schema_version", "contract_id", "authority_id", "manifest_schema", "predecessor",
            "event_contract_registry", "allowlist", "predecessor_source_supersessions",
            "source_files", "result_vector", "release"
        ],
        "properties": {
            "schema_version": {"const": SCHEMA_VERSION},
            "contract_id": {"const": CONTRACT_ID},
            "authority_id": {"const": AUTHORITY_ID},
            "manifest_schema": {"$ref": "#/$defs/file"},
            "predecessor": {
                "type": "object", "additionalProperties": false,
                "required": ["contract_id", "immutable_artifacts"],
                "properties": {
                    "contract_id": {"const": PREDECESSOR_CONTRACT_ID},
                    "immutable_artifacts": {"type": "array", "minItems": 6, "maxItems": 6, "items": {"$ref": "#/$defs/file"}}
                }
            },
            "event_contract_registry": {
                "type": "object", "additionalProperties": false,
                "required": ["version", "inventory", "sidecar", "evolution"],
                "properties": {
                    "version": {"const": 7},
                    "inventory": {"$ref": "#/$defs/file"},
                    "sidecar": {"$ref": "#/$defs/file"},
                    "evolution": {"const": "immutable_registry_v7_plus_additive_phase1_publication_allowlist_v1"}
                }
            },
            "allowlist": {
                "type": "object", "additionalProperties": false,
                "required": ["version", "contract_id", "operation_id", "canonical_json_operation_id", "input_type", "output_type", "allowed_leaves", "kind_one_precedence", "event_policy", "classified_listing_policy", "denied_families", "granted_capability", "excluded_capabilities"],
                "properties": {
                    "version": {"const": 1},
                    "contract_id": {"const": RADROOTS_PHASE1_PUBLICATION_ALLOWLIST_CONTRACT_ID},
                    "operation_id": {"const": OPERATION_ID},
                    "canonical_json_operation_id": {"const": CANONICAL_JSON_OPERATION_ID},
                    "input_type": {"const": "RadrootsPhase1PublicationArtifact"},
                    "output_type": {"const": "RadrootsPhase1AllowlistedPublicationArtifact"},
                    "allowed_leaves": {"type": "array", "minItems": 7, "maxItems": 7, "items": {"$ref": "#/$defs/leaf"}},
                    "kind_one_precedence": {"const": ["ask", "photo_update", "update"]},
                    "event_policy": {"const": "strict_typed_nip52_date_or_time_artifact_only_v1"},
                    "classified_listing_policy": {"const": "raw_marker_partition_before_focused_food_profile_validation_v1"},
                    "denied_families": {"type": "array", "minItems": 19, "maxItems": 19, "items": {"type": "string", "minLength": 1}},
                    "granted_capability": {"const": "phase1_durable_publication_lane_entry_only_v1"},
                    "excluded_capabilities": {"type": "array", "minItems": 7, "maxItems": 7, "items": {"type": "string", "minLength": 1}}
                }
            },
            "predecessor_source_supersessions": {"type": "array", "minItems": 8, "maxItems": 8, "items": {"type": "string", "minLength": 1}},
            "source_files": {"type": "array", "minItems": 1, "items": {"$ref": "#/$defs/source"}},
            "result_vector": {
                "type": "object", "additionalProperties": false,
                "required": ["canonical_path", "mirror_path", "byte_length", "sha256", "hash_algorithm", "executor_path", "executor_test", "valid_case_ids", "invalid_case_ids"],
                "properties": {
                    "canonical_path": {"const": VECTOR_RELATIVE},
                    "mirror_path": {"const": VECTOR_MIRROR_RELATIVE},
                    "byte_length": {"type": "integer", "minimum": 1},
                    "sha256": {"type": "string", "pattern": "^[0-9a-f]{64}$"},
                    "hash_algorithm": {"const": HASH_ALGORITHM},
                    "executor_path": {"const": VECTOR_EXECUTOR_RELATIVE},
                    "executor_test": {"const": VECTOR_EXECUTOR_TEST},
                    "valid_case_ids": {"type": "array", "minItems": 14, "maxItems": 14, "items": {"type": "string"}},
                    "invalid_case_ids": {"type": "array", "minItems": 25, "maxItems": 25, "items": {"type": "string"}}
                }
            },
            "release": {
                "type": "object", "additionalProperties": false,
                "required": ["record_path", "change_id", "changelog_path", "changelog_marker"],
                "properties": {
                    "record_path": {"const": RELEASE_RELATIVE},
                    "change_id": {"const": RELEASE_CHANGE_ID},
                    "changelog_path": {"const": CHANGELOG_RELATIVE},
                    "changelog_marker": {"const": CHANGELOG_MARKER}
                }
            }
        },
        "$defs": {
            "file": {
                "type": "object", "additionalProperties": false,
                "required": ["path", "byte_length", "sha256", "hash_algorithm"],
                "properties": {
                    "path": {"type": "string", "minLength": 1},
                    "byte_length": {"type": "integer", "minimum": 1},
                    "sha256": {"type": "string", "pattern": "^[0-9a-f]{64}$"},
                    "hash_algorithm": {"const": HASH_ALGORITHM}
                }
            },
            "source": {
                "type": "object", "additionalProperties": false,
                "required": ["role", "path", "byte_length", "sha256", "hash_algorithm"],
                "properties": {
                    "role": {"type": "string", "minLength": 1},
                    "path": {"type": "string", "minLength": 1},
                    "byte_length": {"type": "integer", "minimum": 1},
                    "sha256": {"type": "string", "pattern": "^[0-9a-f]{64}$"},
                    "hash_algorithm": {"const": HASH_ALGORITHM}
                }
            },
            "leaf": {
                "type": "object", "additionalProperties": false,
                "required": ["leaf", "kind", "publication_role", "authored_operation_id", "event_contract_id"],
                "properties": {
                    "leaf": {"type": "string", "minLength": 1},
                    "kind": {"type": "integer", "minimum": 0},
                    "publication_role": {"enum": ["supporting", "root"]},
                    "authored_operation_id": {"type": "string", "minLength": 1},
                    "event_contract_id": {"type": "string", "minLength": 1}
                }
            }
        }
    })
}

fn source_descriptor(
    workspace_root: &Path,
    role: &str,
    path: &str,
) -> Result<SourceFileDescriptor, String> {
    let bytes = read_regular_file(workspace_root, path)?;
    Ok(SourceFileDescriptor {
        role: role.to_owned(),
        path: path.to_owned(),
        byte_length: bytes.len() as u64,
        sha256: sha256_hex(&bytes),
        hash_algorithm: HASH_ALGORITHM.to_owned(),
    })
}

fn descriptor_for_file(workspace_root: &Path, path: &str) -> Result<FileDescriptor, String> {
    descriptor_for_bytes(path, &read_regular_file(workspace_root, path)?)
}

fn descriptor_for_bytes(path: &str, bytes: &[u8]) -> Result<FileDescriptor, String> {
    Ok(FileDescriptor {
        path: path.to_owned(),
        byte_length: bytes.len() as u64,
        sha256: sha256_hex(bytes),
        hash_algorithm: HASH_ALGORITHM.to_owned(),
    })
}

fn parse_toml(workspace_root: &Path, relative: &str) -> Result<toml::Value, String> {
    let bytes = read_regular_file(workspace_root, relative)?;
    let source = std::str::from_utf8(&bytes)
        .map_err(|error| format!("{relative} must be UTF-8: {error}"))?;
    toml::from_str(source).map_err(|error| format!("parse {relative}: {error}"))
}

fn toml_string_array(label: &str, value: Option<&toml::Value>) -> Result<Vec<String>, String> {
    value
        .and_then(toml::Value::as_array)
        .ok_or_else(|| format!("{label} must be an array"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("{label} values must be strings"))
        })
        .collect()
}

fn canonical_json_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, String> {
    let mut bytes =
        serde_json::to_vec_pretty(value).map_err(|error| format!("serialize JSON: {error}"))?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn validate_canonical_json<T: Serialize>(
    relative: &str,
    bytes: &[u8],
    value: &T,
) -> Result<(), String> {
    if canonical_json_bytes(value)? != bytes {
        return Err(format!("{relative} is not canonical pretty JSON"));
    }
    Ok(())
}

fn validate_json_schema(schema: &Value, instance: &Value) -> Result<(), String> {
    let validator = jsonschema::validator_for(schema)
        .map_err(|error| format!("compile {MANIFEST_SCHEMA_RELATIVE}: {error}"))?;
    let errors = validator
        .iter_errors(instance)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{MANIFEST_RELATIVE} violates its schema: {}",
            errors.join("; ")
        ))
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn workspace_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("xtask workspace root")
            .to_path_buf()
    }

    #[test]
    fn allowlist_source_inventory_is_closed_and_unique() {
        let specs = successor_source_specs(&workspace_root()).expect("allowlist sources");
        let paths = specs
            .iter()
            .map(|(_, path)| path.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(paths.len(), specs.len());
        assert!(paths.contains("crates/event_codec/src/wire/publication/allowlist.rs"));
        assert!(paths.contains(VECTOR_EXECUTOR_RELATIVE));
        assert!(paths.contains("tools/xtask/src/contract/phase1_publication_allowlist.rs"));
        for generated in GENERATED_ARTIFACT_PATHS {
            assert!(!paths.contains(generated));
        }
    }

    #[test]
    fn allowlist_authorities_and_vector_are_current() {
        let root = workspace_root();
        validate_registry_identity(&root).expect("registry identity");
        validate_operations_authority(&root).expect("operation authority");
        validate_release_authority(&root).expect("release authority");
        validate_vector(&root).expect("allowlist vector");
    }

    #[test]
    fn allowlist_vector_semantic_witnesses_fail_closed() {
        let bytes = read_regular_file(&workspace_root(), VECTOR_RELATIVE).expect("vector bytes");
        let suite: VectorSuite = serde_json::from_slice(&bytes).expect("vector suite");
        for (spec, vector) in INVALID_VECTOR_SPECS
            .iter()
            .copied()
            .zip(suite.vectors.iter().skip(VALID_VECTOR_SPECS.len()))
        {
            let input = vector.input.as_object().expect("vector input");
            let canonical_json = input
                .get("canonical_json")
                .and_then(Value::as_str)
                .expect("canonical candidate");
            let mut candidate: Value =
                serde_json::from_str(canonical_json).expect("candidate JSON");
            match spec.witness {
                InvalidVectorWitness::Artifact(_) => {
                    candidate["draft"]["content"] = Value::String("witness drift".to_owned());
                }
                InvalidVectorWitness::RawEvent(_) => {
                    candidate["content"] = Value::String("witness drift".to_owned());
                }
                InvalidVectorWitness::NonEventRouteDelivery => {
                    candidate["product_surface"] = Value::String("delivery_event".to_owned());
                }
            }
            assert!(
                validate_invalid_vector_evidence(spec.id, spec, input, &candidate).is_err(),
                "{} semantic witness accepted drift",
                spec.id
            );
        }
    }

    #[test]
    fn allowlist_manifest_schema_rejects_unknown_top_level_fields() {
        let root = workspace_root();
        let schema = manifest_schema();
        let schema_bytes = canonical_json_bytes(&schema).expect("schema bytes");
        let manifest = describe_manifest(&root, &schema_bytes).expect("manifest");
        let mut value = serde_json::to_value(manifest).expect("manifest value");
        validate_json_schema(&schema, &value).expect("valid manifest");
        value["unexpected"] = Value::Bool(true);
        validate_json_schema(&schema, &value).expect_err("unknown field must fail");
    }
}
