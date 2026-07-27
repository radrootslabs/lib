#![allow(dead_code)]

use super::artifact_bundle::{
    GeneratedArtifact, read_regular_file, with_artifact_bundle_transaction,
};
use super::nip09_reconciliation::{
    nip09_predecessor_production_source_paths_under_lock,
    validate_nip09_predecessor_production_sources_under_lock,
    validate_nip09_reconciliation_manifest_under_lock,
};
use quote::ToTokens;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use syn::visit::Visit;
use syn::{Expr, Item, Lit};

const SCHEMA_VERSION: u32 = 1;
const CONTRACT_ID: &str = "radroots_event_store.food_availability_projection_v1";
const HOOK_ID: &str = "food_availability_projection_v1";
const MIGRATION_VERSION: u32 = 3;
const MIGRATION_NAME: &str = "food_availability_projection";
const PROJECTION_VERSION: u32 = 1;
const ADDRESSABLE_FEED_VERSION: u32 = 1;
const EVENT_CONTRACT_REGISTRY_VERSION: u32 = 7;
const FOOD_AVAILABILITY_KIND: u32 = 30_402;
const UNRELATED_ADDRESSABLE_KIND: u32 = 30_340;
const FOOD_CONTRACT_ID: &str = "radroots.food.availability.v1";
const OPERATIONAL_LISTING_CONTRACT_ID: &str = "radroots.operational_listing.published.v1";
const FARM_PROFILE_CONTRACT_ID: &str = "radroots.farm.profile.v1";
const DELETION_CONTRACT_ID: &str = "radroots.social.deletion_request.v1";
const ADMISSION_AUTHORITY: &str = "event_contract_registry_v7";
const CURRENT_VISIBILITY_AUTHORITY: &str = "radroots_event_store_current_visibility_v1";
const POST_CORE_CAPABILITY: &str = "apply_v2";
const SCOPE_FINGERPRINT_SHA256: &str =
    "8b63c5ddc48a2cc7db69295238b96d5f814dba50427c80b4d0079f061e6d3de0";
const SCHEMA_SHA256: &str = "dd12467e04addcbddb5ea0f386c12a8ac05ef5ebaaf949f24dd2c62745f5aaac";

const PREDECESSOR_HOOK_ID: &str = "nip09_reconciliation_v1";
const PREDECESSOR_MANIFEST_RELATIVE: &str =
    "crates/event_store/contracts/nip09_reconciliation_v1.manifest.json";
const PREDECESSOR_MANIFEST_BYTE_LENGTH: usize = 537_538;
const PREDECESSOR_MANIFEST_SHA256: &str =
    "74af832420ffbaa9805e89df3c0b34f126a443e1598f757e3372f407f9003b77";

const MANIFEST_RELATIVE: &str =
    "crates/event_store/contracts/food_availability_projection_v1.manifest.json";
const MANIFEST_SCHEMA_RELATIVE: &str =
    "crates/event_store/contracts/food_availability_projection_v1.manifest.schema.json";
const MANIFEST_SHA256_RELATIVE: &str =
    "crates/event_store/contracts/food_availability_projection_v1.manifest.sha256";
const GENERATED_DESCRIPTOR_RELATIVE: &str =
    "crates/event_store/src/generated/food_availability_projection_manifest.rs";
const MIGRATIONS_SOURCE_RELATIVE: &str = "crates/event_store/src/migrations.rs";
const MIGRATION_UP_RELATIVE: &str =
    "crates/event_store/migrations/0003_food_availability_projection.up.sql";
const MIGRATION_DOWN_RELATIVE: &str =
    "crates/event_store/migrations/0003_food_availability_projection.down.sql";
const REGISTRY_INVENTORY_RELATIVE: &str =
    "contracts/event_store/event_contract_registry_v7.inventory.json";
const FOOD_PROFILE_VECTOR_RELATIVE: &str =
    "contracts/conformance/vectors/food_availability/profile.v1.json";
const RESULT_VECTOR_CANONICAL_RELATIVE: &str =
    "contracts/conformance/vectors/event_store/food_availability_projection.v1.json";
const RESULT_VECTOR_MIRROR_RELATIVE: &str =
    "crates/event_store/tests/fixtures/food_availability_projection.v1.json";
const RESULT_VECTOR_EXECUTOR_RELATIVE: &str =
    "crates/event_store/tests/food_availability_projection_v1_result_vector.rs";
const RESULT_VECTOR_EXECUTOR_ID: &str =
    "radroots_event_store.food_availability_projection_v1.result_vector_executor.v1";
const RESULT_VECTOR_EXECUTOR_TEST: &str = "food_availability_projection_v1_result_vector";
const SOURCE_GENERATION_ACTIVE_SENTINEL: &str = "active";
const WRITE_COMMAND: &str = "cargo xtask contract food-availability-projection-manifest --write";
const HASH_ALGORITHM: &str = "sha256_bytes_v1";
const EVENT_STORE_LIB_RELATIVE: &str = "crates/event_store/src/lib.rs";
const EVENT_STORE_MODEL_RELATIVE: &str = "crates/event_store/src/model.rs";

const IMMUTABLE_MANIFEST_BYTES: &[u8] = include_bytes!(
    "../../../../crates/event_store/contracts/food_availability_projection_v1.manifest.json"
);
const IMMUTABLE_MANIFEST_SCHEMA_BYTES: &[u8] = include_bytes!(
    "../../../../crates/event_store/contracts/food_availability_projection_v1.manifest.schema.json"
);
const IMMUTABLE_MANIFEST_SHA256_BYTES: &[u8] = include_bytes!(
    "../../../../crates/event_store/contracts/food_availability_projection_v1.manifest.sha256"
);
const IMMUTABLE_GENERATED_DESCRIPTOR_BYTES: &[u8] = include_bytes!(
    "../../../../crates/event_store/src/generated/food_availability_projection_manifest.rs"
);
const IMMUTABLE_RESULT_VECTOR_BYTES: &[u8] = include_bytes!(
    "../../../../contracts/conformance/vectors/event_store/food_availability_projection.v1.json"
);

#[derive(Clone, Copy)]
struct ImmutableArtifactSpec {
    relative: &'static str,
    byte_length: usize,
    sha256: &'static str,
}

const IMMUTABLE_PREDECESSOR_ARTIFACTS: [ImmutableArtifactSpec; 9] = [
    ImmutableArtifactSpec {
        relative: MANIFEST_RELATIVE,
        byte_length: 17_455,
        sha256: "33b93a3c87ce428e8aa6f5e92643c77203d9aa006c53ce96f3562fe6d68ffd23",
    },
    ImmutableArtifactSpec {
        relative: MANIFEST_SCHEMA_RELATIVE,
        byte_length: 7_964,
        sha256: "39171f6ef872a8d1483bc3d55049df5e0d110d9131c5adb4450b7c418f546910",
    },
    ImmutableArtifactSpec {
        relative: MANIFEST_SHA256_RELATIVE,
        byte_length: 65,
        sha256: "4ac4c79a946ccb1a11726cbafc18e2e016f08f3f6797964400dea3494c66dbc5",
    },
    ImmutableArtifactSpec {
        relative: GENERATED_DESCRIPTOR_RELATIVE,
        byte_length: 21_437,
        sha256: "90908da53ab9572f45f5916ccc2652736b7ea26ba6dd202a4f69af1e651b564b",
    },
    ImmutableArtifactSpec {
        relative: RESULT_VECTOR_CANONICAL_RELATIVE,
        byte_length: 103_659,
        sha256: "fca2b71b47736ed04ed1e908823b65b3fc3cf0366cb162128369fe328295bb63",
    },
    ImmutableArtifactSpec {
        relative: RESULT_VECTOR_MIRROR_RELATIVE,
        byte_length: 103_659,
        sha256: "fca2b71b47736ed04ed1e908823b65b3fc3cf0366cb162128369fe328295bb63",
    },
    ImmutableArtifactSpec {
        relative: RESULT_VECTOR_EXECUTOR_RELATIVE,
        byte_length: 34_075,
        sha256: "9e8e11abae7bbc7dda30eab6f0a79074ffc3761aa6b955cff58c4c62fa581aa3",
    },
    ImmutableArtifactSpec {
        relative: MIGRATION_UP_RELATIVE,
        byte_length: 23_683,
        sha256: "4e7edfb981b25f76055efc7802ec30b4034eeae9b9c0809ea4ea7c574678748a",
    },
    ImmutableArtifactSpec {
        relative: MIGRATION_DOWN_RELATIVE,
        byte_length: 1_755,
        sha256: "29d663320109d9dd0df6a00b6a53d8d988438d01f7a66960a9d4ba3482ffffb8",
    },
];

const GOVERNED_PUBLIC_API_MODULES: &[&str] = &[
    "addressable_transition_feed_v1",
    "current_visibility_v1",
    "food_availability_projection_v1",
];

#[derive(Clone, Copy)]
struct SourceSpec {
    role: &'static str,
    path: &'static str,
}

const SOURCE_SPECS: &[SourceSpec] = &[
    SourceSpec {
        role: "workspace_dependency_authority",
        path: "Cargo.toml",
    },
    SourceSpec {
        role: "event_store_dependency_authority",
        path: "crates/event_store/Cargo.toml",
    },
    SourceSpec {
        role: "blossom_public_surface",
        path: "crates/blossom/src/lib.rs",
    },
    SourceSpec {
        role: "blossom_sha256_value_object",
        path: "crates/blossom/src/hash.rs",
    },
    SourceSpec {
        role: "food_event_contract",
        path: "crates/event/src/food_availability.rs",
    },
    SourceSpec {
        role: "food_admission",
        path: "crates/event_codec/src/food_availability/admission.rs",
    },
    SourceSpec {
        role: "registry_v7_admission",
        path: "crates/event_codec/src/admission/registry_v7.rs",
    },
    SourceSpec {
        role: "registry_v7_food_projection",
        path: "crates/event_codec/src/food_availability/inbound/registry_v7.rs",
    },
    SourceSpec {
        role: "event_store_error_surface",
        path: "crates/event_store/src/error.rs",
    },
    SourceSpec {
        role: "generated_descriptor_registration",
        path: "crates/event_store/src/generated.rs",
    },
    SourceSpec {
        role: "public_surface",
        path: "crates/event_store/src/lib.rs",
    },
    SourceSpec {
        role: "migration_registry",
        path: MIGRATIONS_SOURCE_RELATIVE,
    },
    SourceSpec {
        role: "model_registration",
        path: "crates/event_store/src/model.rs",
    },
    SourceSpec {
        role: "schema_hooks",
        path: "crates/event_store/src/schema.rs",
    },
    SourceSpec {
        role: "store_ingest_and_wal_authority",
        path: "crates/event_store/src/store.rs",
    },
    SourceSpec {
        role: "addressable_transition_feed_model",
        path: "crates/event_store/src/model/addressable_transition_feed_v1.rs",
    },
    SourceSpec {
        role: "current_visibility_model",
        path: "crates/event_store/src/model/current_visibility_v1.rs",
    },
    SourceSpec {
        role: "food_projection_model",
        path: "crates/event_store/src/model/food_availability_projection_v1.rs",
    },
    SourceSpec {
        role: "addressable_transition_feed_store",
        path: "crates/event_store/src/store/addressable_transition_feed_v1.rs",
    },
    SourceSpec {
        role: "current_visibility_store",
        path: "crates/event_store/src/store/current_visibility_v1.rs",
    },
    SourceSpec {
        role: "food_projection_store",
        path: "crates/event_store/src/store/food_availability_projection_v1.rs",
    },
    SourceSpec {
        role: "predecessor_protocol_storage",
        path: "crates/event_store/src/store/protocol_storage_v1.rs",
    },
    SourceSpec {
        role: "predecessor_active_state_fast_validation",
        path: "crates/event_store/src/nip09/reconciliation_v1.rs",
    },
    SourceSpec {
        role: "post_core_capabilities",
        path: "crates/event_store/src/store/post_core_extension_capabilities.rs",
    },
    SourceSpec {
        role: "post_core_dispatcher",
        path: "crates/event_store/src/store/post_core_extension_dispatcher.rs",
    },
    SourceSpec {
        role: "post_core_v2_extension",
        path: "crates/event_store/src/store/post_core_extensions_v2.rs",
    },
    SourceSpec {
        role: "post_core_v2_storage",
        path: "crates/event_store/src/store/post_core_storage_v2.rs",
    },
    SourceSpec {
        role: "predecessor_post_core_v1_extension",
        path: "crates/event_store/src/store/post_core_extensions_v1.rs",
    },
    SourceSpec {
        role: "predecessor_post_core_v1_storage",
        path: "crates/event_store/src/store/post_core_storage_v1.rs",
    },
];

const PREDECESSOR_SUPERSEDED_SOURCE_PATHS: &[&str] = &[
    "crates/blossom/src/hash.rs",
    "crates/blossom/src/lib.rs",
    "crates/event/src/food_availability.rs",
    "crates/event_codec/src/admission/registry_v7.rs",
    "crates/event_codec/src/food_availability/admission.rs",
    "crates/event_codec/src/food_availability/inbound/registry_v7.rs",
    "crates/event_store/src/error.rs",
    "crates/event_store/src/generated.rs",
    "crates/event_store/src/lib.rs",
    "crates/event_store/src/migrations.rs",
    "crates/event_store/src/model.rs",
    "crates/event_store/src/nip09/reconciliation_v1.rs",
    "crates/event_store/src/schema.rs",
    "crates/event_store/src/store.rs",
    "crates/event_store/src/store/post_core_extension_capabilities.rs",
    "crates/event_store/src/store/post_core_extension_dispatcher.rs",
    "crates/event_store/src/store/post_core_extensions_v1.rs",
    "crates/event_store/src/store/post_core_storage_v1.rs",
    "crates/event_store/src/store/protocol_storage_v1.rs",
];

const EXPECTED_CATALOG_OBJECTS: &[&str] = &[
    "radroots_event_store_addressable_feed_generation_insert",
    "radroots_event_store_addressable_feed_integrity_v1",
    "radroots_event_store_addressable_feed_transition_insert",
    "radroots_event_store_addressable_transition_coordinate_idx",
    "radroots_event_store_current_visibility_head_lookup_idx",
    "radroots_event_store_current_visibility_v1",
    "radroots_event_store_food_availability_author_idx",
    "radroots_event_store_food_availability_cursor",
    "radroots_event_store_food_availability_cursor_delete_guard",
    "radroots_event_store_food_availability_cursor_insert_guard",
    "radroots_event_store_food_availability_cursor_update_guard",
    "radroots_event_store_food_availability_image",
    "radroots_event_store_food_availability_image_delete_guard",
    "radroots_event_store_food_availability_image_insert_guard",
    "radroots_event_store_food_availability_image_update_guard",
    "radroots_event_store_food_availability_projection",
    "radroots_event_store_food_availability_projection_delete_guard",
    "radroots_event_store_food_availability_projection_insert_guard",
    "radroots_event_store_food_availability_projection_update_guard",
    "radroots_event_store_food_availability_read_v1",
    "radroots_event_store_food_availability_recent_idx",
    "radroots_event_store_food_availability_search_delete",
    "radroots_event_store_food_availability_search_fts",
    "radroots_event_store_food_availability_search_fts_config",
    "radroots_event_store_food_availability_search_fts_content",
    "radroots_event_store_food_availability_search_fts_data",
    "radroots_event_store_food_availability_search_fts_docsize",
    "radroots_event_store_food_availability_search_fts_idx",
    "radroots_event_store_food_availability_search_insert",
    "radroots_event_store_food_availability_status_idx",
    "radroots_event_store_nip09_address_target_visibility_lookup_idx",
];

const EXPECTED_CATALOG_TABLES: &[&str] = &[
    "radroots_event_store_addressable_feed_integrity_v1",
    "radroots_event_store_food_availability_cursor",
    "radroots_event_store_food_availability_image",
    "radroots_event_store_food_availability_projection",
    "radroots_event_store_food_availability_search_fts",
    "radroots_event_store_food_availability_search_fts_config",
    "radroots_event_store_food_availability_search_fts_content",
    "radroots_event_store_food_availability_search_fts_data",
    "radroots_event_store_food_availability_search_fts_docsize",
    "radroots_event_store_food_availability_search_fts_idx",
];

const EXPECTED_CATALOG_FTS5_TABLES: &[&str] =
    &["radroots_event_store_food_availability_search_fts"];

const ENTRY_POINTS: &[(&str, &str)] = &[
    (
        "registry_v7_admission",
        "radroots_event_codec::admit_verified_event_registry_v7",
    ),
    (
        "migration_registry",
        "radroots_event_store::migrations::EVENT_STORE_MIGRATIONS[2]",
    ),
    (
        "migration_apply_hook",
        "radroots_event_store::schema::apply_migration_hook",
    ),
    (
        "migration_validation_hook",
        "radroots_event_store::schema::validate_migration_hook_state",
    ),
    (
        "post_core_extension",
        "radroots_event_store::store::PostCoreExtensionCapabilities::apply_v2",
    ),
    (
        "addressable_transition_feed",
        "radroots_event_store::RadrootsEventStore::addressable_transition_page_v1",
    ),
    (
        "current_visibility",
        "radroots_event_store::RadrootsEventStore::current_event_visibility_v1",
    ),
    (
        "event_visibility_batch",
        "radroots_event_store::RadrootsEventStore::event_visibilities",
    ),
    (
        "projection_lookup",
        "radroots_event_store::RadrootsEventStore::food_availability_v1",
    ),
    (
        "projection_recent",
        "radroots_event_store::RadrootsEventStore::recent_food_availability_v1",
    ),
    (
        "projection_search",
        "radroots_event_store::RadrootsEventStore::search_food_availability_v1",
    ),
    (
        "projection_audit",
        "radroots_event_store::RadrootsEventStore::audit_food_availability_projection_v1",
    ),
    ("result_vector_executor", RESULT_VECTOR_EXECUTOR_TEST),
];

const PUBLIC_API: &[&str] = &[
    "RADROOTS_ADDRESSABLE_TRANSITION_CURSOR_JSON_MAX_BYTES_V1",
    "RADROOTS_ADDRESSABLE_TRANSITION_D_TAG_MAX_BYTES_V1",
    "RADROOTS_ADDRESSABLE_TRANSITION_FEED_VERSION_V1",
    "RADROOTS_ADDRESSABLE_TRANSITION_PAGE_LIMIT_MAX_V1",
    "RADROOTS_ADDRESSABLE_TRANSITION_PAGE_RAW_JSON_MAX_BYTES_V1",
    "RADROOTS_ADDRESSABLE_TRANSITION_PAGE_SCAN_MAX_V1",
    "RADROOTS_ADDRESSABLE_TRANSITION_SCOPE_KIND_MAX_V1",
    "RADROOTS_FOOD_AVAILABILITY_PROJECTION_APPLY_PAGE_LIMIT_V1",
    "RADROOTS_FOOD_AVAILABILITY_PROJECTION_VERSION_V1",
    "RADROOTS_FOOD_AVAILABILITY_SEARCH_QUERY_MAX_BYTES_V1",
    "RADROOTS_FOOD_AVAILABILITY_SEARCH_QUERY_MAX_TERMS_V1",
    "RadrootsAddressableTransitionCauseV1",
    "RadrootsAddressableTransitionCoordinateV1",
    "RadrootsAddressableTransitionCursorV1",
    "RadrootsAddressableTransitionEventReferenceV1",
    "RadrootsAddressableTransitionOriginV1",
    "RadrootsAddressableTransitionPageV1",
    "RadrootsAddressableTransitionRawHeadDecisionV1",
    "RadrootsAddressableTransitionScopeFingerprintV1",
    "RadrootsAddressableTransitionScopeV1",
    "RadrootsAddressableTransitionV1",
    "RadrootsAddressableTransitionVisibilityV1",
    "RadrootsCurrentEventVisibilityV1",
    "RadrootsCurrentVisibilityDecisionV1",
    "RadrootsFoodAvailabilitySearchQueryV1",
    "RadrootsFoodAvailabilityStatusFilterV1",
    "RadrootsNip09SuppressionEvidenceV1",
    "RadrootsNip09SuppressionOutcome",
    "RadrootsNip09SuppressionReason",
    "RadrootsStoreProducedCanonicalEventV1",
    "RadrootsStoredFoodAvailabilityImageV1",
    "RadrootsStoredFoodAvailabilityV1",
];

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct FoodAvailabilityProjectionManifest {
    schema_version: u32,
    contract_id: String,
    hook_id: String,
    manifest_schema: FileDescriptor,
    predecessor: PredecessorDescriptor,
    migration: MigrationDescriptor,
    profile: ProfileDescriptor,
    registry_inventory: FileDescriptor,
    food_profile_vector: FileDescriptor,
    entry_points: Vec<EntryPointDescriptor>,
    source_files: Vec<SourceFileDescriptor>,
    public_api: Vec<String>,
    result_vector: ResultVectorDescriptor,
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
struct PredecessorDescriptor {
    hook_id: String,
    manifest: FileDescriptor,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct MigrationDescriptor {
    version: u32,
    name: String,
    up: FileDescriptor,
    down: FileDescriptor,
    schema_sha256: String,
    catalog: CatalogDescriptor,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct CatalogDescriptor {
    objects: Vec<String>,
    tables: Vec<String>,
    fts5_tables: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ProfileDescriptor {
    event_contract_registry_version: u32,
    addressable_feed_version: u32,
    projection_version: u32,
    scope_kinds: Vec<u32>,
    scope_fingerprint_sha256: String,
    food_contract_id: String,
    admission_authority: String,
    current_visibility_authority: String,
    post_core_capability: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct EntryPointDescriptor {
    role: String,
    rust_path: String,
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
struct ResultVectorDescriptor {
    canonical_path: String,
    mirror_path: String,
    byte_length: u64,
    sha256: String,
    hash_algorithm: String,
    executor_id: String,
    executor_path: String,
    executor_test: String,
    executor_byte_length: u64,
    executor_sha256: String,
    executor_hash_algorithm: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ProjectionResultVector {
    schema_version: u32,
    contract_id: String,
    feed_version: u32,
    projection_version: u32,
    scope_kinds: Vec<u32>,
    cases: Vec<ProjectionCase>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ProjectionCase {
    id: String,
    events: Vec<ObservedEvent>,
    expected: ExpectedCase,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ObservedEvent {
    role: ProjectionInputRole,
    observed_at_ms: i64,
    expected_ingest: ExpectedIngest,
    event: SignedEvent,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum ProjectionInputRole {
    ScopedFood,
    ScopedNonFood,
    UnrelatedAddressable,
    Causal,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ExpectedIngest {
    admission_status: String,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    admission_code: RequiredNullable<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    contract_id: RequiredNullable<String>,
    event_class: String,
    valid_stream_eligible: bool,
    raw_head_decision: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SignedEvent {
    id: String,
    pubkey: String,
    created_at: u64,
    kind: u32,
    tags: Vec<Vec<String>>,
    content: String,
    sig: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ExpectedCase {
    coordinate: ExpectedCoordinate,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    projection: RequiredNullable<ExpectedProjection>,
    searches: Vec<ExpectedSearch>,
    transition_page: ExpectedTransitionPage,
    event_visibility: Vec<ExpectedVisibility>,
    historical_visibility_witnesses: Vec<ExpectedHistoricalVisibilityWitness>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ExpectedCoordinate {
    kind: u32,
    pubkey: String,
    d_tag: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ExpectedProjection {
    event_id: String,
    content: String,
    title: String,
    summary: String,
    published_at: u64,
    location: String,
    price_amount: String,
    price_currency: String,
    price_unit: String,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    quantity_amount: RequiredNullable<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    quantity_unit: RequiredNullable<String>,
    status: String,
    diagnostics: Vec<String>,
    images: Vec<ExpectedImage>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ExpectedImage {
    #[serde(deserialize_with = "deserialize_required_nullable")]
    url: RequiredNullable<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    width: RequiredNullable<u32>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    height: RequiredNullable<u32>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    blossom_sha256: RequiredNullable<String>,
    diagnostics: Vec<String>,
    qualifies: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ExpectedSearch {
    query: String,
    event_ids: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ExpectedTransitionPage {
    source_high_water: i64,
    has_more: bool,
    next_cursor: ExpectedTransitionCursor,
    transitions: Vec<ExpectedTransition>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ExpectedTransitionCursor {
    source_generation: String,
    feed_version: u32,
    scope_fingerprint: String,
    last_transition_seq: i64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ExpectedTransition {
    transition_seq: i64,
    source_generation: String,
    origin: String,
    coordinate: ExpectedCoordinate,
    raw_head: ExpectedEventReference,
    raw_head_created_at: u64,
    admission_status: String,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    admission_code: RequiredNullable<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    contract_id: RequiredNullable<String>,
    visibility: String,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    suppression: RequiredNullable<ExpectedSuppressionEvidence>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    cause_event: RequiredNullable<ExpectedTransitionCause>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    canonical_visible_event: RequiredNullable<ExpectedCanonicalVisibleEvent>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    retracted_event: RequiredNullable<ExpectedEventReference>,
    raw_head_decision: String,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ExpectedEventReference {
    event_id: String,
    event_seq: i64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ExpectedSuppressionEvidence {
    outcome: String,
    reason: String,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    event_reference_request_id: RequiredNullable<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    address_reference_request_id: RequiredNullable<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    address_reference_cutoff: RequiredNullable<u64>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ExpectedTransitionCause {
    event: ExpectedEventReference,
    pubkey: String,
    created_at: u64,
    kind: u32,
    admission_status: String,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    admission_code: RequiredNullable<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    contract_id: RequiredNullable<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ExpectedCanonicalVisibleEvent {
    event: ExpectedEventReference,
    raw_json_sha256: String,
    admission_status: String,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    contract_id: RequiredNullable<String>,
    event_class: String,
    valid_stream_eligible: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ExpectedVisibility {
    event: ExpectedEventReference,
    source_generation: String,
    admission_status: String,
    decision: String,
    is_raw_head: bool,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    raw_head_event_id: RequiredNullable<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    suppression: RequiredNullable<ExpectedSuppressionEvidence>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ExpectedHistoricalVisibilityWitness {
    transition_seq: i64,
    event_id: String,
    final_decision: String,
}

#[derive(Debug, Serialize)]
struct RequiredNullable<T>(Option<T>);

fn deserialize_required_nullable<'de, D, T>(
    deserializer: D,
) -> Result<RequiredNullable<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(RequiredNullable)
}

impl<'de, T> Deserialize<'de> for RequiredNullable<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Option::<T>::deserialize(deserializer).map(Self)
    }
}

pub(crate) fn write_food_availability_projection_manifest(
    workspace_root: &Path,
) -> Result<(), String> {
    with_artifact_bundle_transaction(workspace_root, |transaction| {
        transaction.write(immutable_generated_artifacts())?;
        validate_food_availability_projection_manifest_under_lock(workspace_root)
    })
}

pub(crate) fn validate_food_availability_projection_manifest(
    workspace_root: &Path,
) -> Result<(), String> {
    with_artifact_bundle_transaction(workspace_root, |_| {
        validate_food_availability_projection_manifest_under_lock(workspace_root)
    })
}

pub(super) fn validate_food_availability_projection_manifest_under_lock(
    workspace_root: &Path,
) -> Result<(), String> {
    validate_nip09_reconciliation_manifest_under_lock(workspace_root)?;

    let manifest_bytes = read_regular_file(workspace_root, MANIFEST_RELATIVE)?;
    let manifest_value: Value = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| format!("parse {MANIFEST_RELATIVE}: {error}"))?;
    let manifest: FoodAvailabilityProjectionManifest =
        serde_json::from_value(manifest_value.clone())
            .map_err(|error| format!("parse typed {MANIFEST_RELATIVE}: {error}"))?;
    validate_canonical_json(MANIFEST_RELATIVE, &manifest_bytes, &manifest)?;

    let schema_bytes = read_regular_file(workspace_root, MANIFEST_SCHEMA_RELATIVE)?;
    let schema: Value = serde_json::from_slice(&schema_bytes)
        .map_err(|error| format!("parse {MANIFEST_SCHEMA_RELATIVE}: {error}"))?;
    validate_canonical_json(MANIFEST_SCHEMA_RELATIVE, &schema_bytes, &schema)?;
    validate_manifest_json_schema(&schema, &manifest_value)?;
    validate_manifest_shape(&manifest)?;

    let digest_bytes = read_regular_file(workspace_root, MANIFEST_SHA256_RELATIVE)?;
    validate_digest_sidecar(MANIFEST_SHA256_RELATIVE, &digest_bytes)?;
    if digest_bytes != format!("{}\n", sha256_hex(&manifest_bytes)).as_bytes() {
        return Err(format!(
            "{MANIFEST_SHA256_RELATIVE} must match the checked-in manifest bytes"
        ));
    }

    if manifest.migration.up.sha256 != IMMUTABLE_PREDECESSOR_ARTIFACTS[7].sha256
        || manifest.migration.down.sha256 != IMMUTABLE_PREDECESSOR_ARTIFACTS[8].sha256
        || manifest.result_vector.sha256 != IMMUTABLE_PREDECESSOR_ARTIFACTS[4].sha256
        || manifest.result_vector.executor_sha256 != IMMUTABLE_PREDECESSOR_ARTIFACTS[6].sha256
    {
        return Err(format!(
            "{MANIFEST_RELATIVE} does not describe the immutable FoodAvailability predecessor identity"
        ));
    }

    let vector_bytes = read_regular_file(workspace_root, RESULT_VECTOR_CANONICAL_RELATIVE)?;
    let mirror_bytes = read_regular_file(workspace_root, RESULT_VECTOR_MIRROR_RELATIVE)?;
    if vector_bytes != mirror_bytes {
        return Err(format!(
            "{RESULT_VECTOR_MIRROR_RELATIVE} must exactly mirror {RESULT_VECTOR_CANONICAL_RELATIVE}"
        ));
    }
    let vector: ProjectionResultVector = serde_json::from_slice(&vector_bytes)
        .map_err(|error| format!("parse {RESULT_VECTOR_CANONICAL_RELATIVE}: {error}"))?;
    validate_canonical_json(RESULT_VECTOR_CANONICAL_RELATIVE, &vector_bytes, &vector)?;
    validate_result_vector(&vector)?;

    for artifact in IMMUTABLE_PREDECESSOR_ARTIFACTS {
        let actual = read_regular_file(workspace_root, artifact.relative)?;
        if actual.len() != artifact.byte_length || sha256_hex(&actual) != artifact.sha256 {
            return Err(format!(
                "immutable FoodAvailability predecessor artifact {} does not match its authenticated byte identity",
                artifact.relative
            ));
        }
    }
    Ok(())
}

fn immutable_generated_artifacts() -> Vec<GeneratedArtifact> {
    vec![
        GeneratedArtifact {
            relative: MANIFEST_RELATIVE,
            contents: IMMUTABLE_MANIFEST_BYTES.to_vec(),
        },
        GeneratedArtifact {
            relative: MANIFEST_SCHEMA_RELATIVE,
            contents: IMMUTABLE_MANIFEST_SCHEMA_BYTES.to_vec(),
        },
        GeneratedArtifact {
            relative: MANIFEST_SHA256_RELATIVE,
            contents: IMMUTABLE_MANIFEST_SHA256_BYTES.to_vec(),
        },
        GeneratedArtifact {
            relative: GENERATED_DESCRIPTOR_RELATIVE,
            contents: IMMUTABLE_GENERATED_DESCRIPTOR_BYTES.to_vec(),
        },
        GeneratedArtifact {
            relative: RESULT_VECTOR_MIRROR_RELATIVE,
            contents: IMMUTABLE_RESULT_VECTOR_BYTES.to_vec(),
        },
    ]
}

fn expected_artifacts(workspace_root: &Path) -> Result<Vec<GeneratedArtifact>, String> {
    let schema = manifest_schema();
    let schema_bytes = canonical_json_bytes(&schema)?;
    let manifest = describe_manifest(workspace_root, &schema_bytes)?;
    let manifest_bytes = canonical_json_bytes(&manifest)?;
    let manifest_sha256 = sha256_hex(&manifest_bytes);
    let descriptor = generated_descriptor(&manifest, &manifest_bytes, &manifest_sha256);
    let vector_bytes = read_regular_file(workspace_root, RESULT_VECTOR_CANONICAL_RELATIVE)?;

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
            contents: format!("{manifest_sha256}\n").into_bytes(),
        },
        GeneratedArtifact {
            relative: GENERATED_DESCRIPTOR_RELATIVE,
            contents: descriptor.into_bytes(),
        },
        GeneratedArtifact {
            relative: RESULT_VECTOR_MIRROR_RELATIVE,
            contents: vector_bytes,
        },
    ])
}

fn describe_manifest(
    workspace_root: &Path,
    schema_bytes: &[u8],
) -> Result<FoodAvailabilityProjectionManifest, String> {
    validate_source_contract(workspace_root)?;
    validate_predecessor_production_source_coverage(workspace_root, &[])?;

    let predecessor_bytes = read_regular_file(workspace_root, PREDECESSOR_MANIFEST_RELATIVE)?;
    if predecessor_bytes.len() != PREDECESSOR_MANIFEST_BYTE_LENGTH
        || sha256_hex(&predecessor_bytes) != PREDECESSOR_MANIFEST_SHA256
    {
        return Err(format!(
            "{PREDECESSOR_MANIFEST_RELATIVE} does not match the immutable predecessor identity"
        ));
    }

    let vector_bytes = read_regular_file(workspace_root, RESULT_VECTOR_CANONICAL_RELATIVE)?;
    let vector: ProjectionResultVector = serde_json::from_slice(&vector_bytes)
        .map_err(|error| format!("parse {RESULT_VECTOR_CANONICAL_RELATIVE}: {error}"))?;
    validate_canonical_json(RESULT_VECTOR_CANONICAL_RELATIVE, &vector_bytes, &vector)?;
    validate_result_vector(&vector)?;

    let migration_source = read_regular_file(workspace_root, MIGRATIONS_SOURCE_RELATIVE)?;
    let catalog = catalog_from_migration_source(&migration_source)?;
    validate_catalog(&catalog)?;
    let executor = descriptor_for_file(workspace_root, RESULT_VECTOR_EXECUTOR_RELATIVE)?;

    let source_files = SOURCE_SPECS
        .iter()
        .map(|spec| {
            let bytes = if spec.path == MIGRATIONS_SOURCE_RELATIVE {
                migration_source.clone()
            } else {
                read_regular_file(workspace_root, spec.path)?
            };
            Ok(SourceFileDescriptor {
                role: spec.role.to_owned(),
                path: spec.path.to_owned(),
                byte_length: byte_length(spec.path, &bytes)?,
                sha256: sha256_hex(&bytes),
                hash_algorithm: HASH_ALGORITHM.to_owned(),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    Ok(FoodAvailabilityProjectionManifest {
        schema_version: SCHEMA_VERSION,
        contract_id: CONTRACT_ID.to_owned(),
        hook_id: HOOK_ID.to_owned(),
        manifest_schema: descriptor_for_bytes(MANIFEST_SCHEMA_RELATIVE, schema_bytes)?,
        predecessor: PredecessorDescriptor {
            hook_id: PREDECESSOR_HOOK_ID.to_owned(),
            manifest: descriptor_for_bytes(PREDECESSOR_MANIFEST_RELATIVE, &predecessor_bytes)?,
        },
        migration: MigrationDescriptor {
            version: MIGRATION_VERSION,
            name: MIGRATION_NAME.to_owned(),
            up: descriptor_for_file(workspace_root, MIGRATION_UP_RELATIVE)?,
            down: descriptor_for_file(workspace_root, MIGRATION_DOWN_RELATIVE)?,
            schema_sha256: SCHEMA_SHA256.to_owned(),
            catalog,
        },
        profile: ProfileDescriptor {
            event_contract_registry_version: EVENT_CONTRACT_REGISTRY_VERSION,
            addressable_feed_version: ADDRESSABLE_FEED_VERSION,
            projection_version: PROJECTION_VERSION,
            scope_kinds: vec![FOOD_AVAILABILITY_KIND],
            scope_fingerprint_sha256: SCOPE_FINGERPRINT_SHA256.to_owned(),
            food_contract_id: FOOD_CONTRACT_ID.to_owned(),
            admission_authority: ADMISSION_AUTHORITY.to_owned(),
            current_visibility_authority: CURRENT_VISIBILITY_AUTHORITY.to_owned(),
            post_core_capability: POST_CORE_CAPABILITY.to_owned(),
        },
        registry_inventory: descriptor_for_file(workspace_root, REGISTRY_INVENTORY_RELATIVE)?,
        food_profile_vector: descriptor_for_file(workspace_root, FOOD_PROFILE_VECTOR_RELATIVE)?,
        entry_points: ENTRY_POINTS
            .iter()
            .map(|(role, rust_path)| EntryPointDescriptor {
                role: (*role).to_owned(),
                rust_path: (*rust_path).to_owned(),
            })
            .collect(),
        source_files,
        public_api: PUBLIC_API.iter().map(|name| (*name).to_owned()).collect(),
        result_vector: ResultVectorDescriptor {
            canonical_path: RESULT_VECTOR_CANONICAL_RELATIVE.to_owned(),
            mirror_path: RESULT_VECTOR_MIRROR_RELATIVE.to_owned(),
            byte_length: byte_length(RESULT_VECTOR_CANONICAL_RELATIVE, &vector_bytes)?,
            sha256: sha256_hex(&vector_bytes),
            hash_algorithm: HASH_ALGORITHM.to_owned(),
            executor_id: RESULT_VECTOR_EXECUTOR_ID.to_owned(),
            executor_path: RESULT_VECTOR_EXECUTOR_RELATIVE.to_owned(),
            executor_test: RESULT_VECTOR_EXECUTOR_TEST.to_owned(),
            executor_byte_length: executor.byte_length,
            executor_sha256: executor.sha256,
            executor_hash_algorithm: HASH_ALGORITHM.to_owned(),
        },
    })
}

fn validate_predecessor_production_source_coverage(
    workspace_root: &Path,
    downstream_nip09_superseded_paths: &[&str],
) -> Result<(), String> {
    for path in PREDECESSOR_SUPERSEDED_SOURCE_PATHS {
        if !SOURCE_SPECS.iter().any(|source| source.path == *path) {
            return Err(format!(
                "successor supersession source `{path}` is not current-byte-bound"
            ));
        }
    }
    let superseded_paths = PREDECESSOR_SUPERSEDED_SOURCE_PATHS
        .iter()
        .copied()
        .chain(downstream_nip09_superseded_paths.iter().copied())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    validate_nip09_predecessor_production_sources_under_lock(workspace_root, &superseded_paths)
}

pub(super) fn validate_food_availability_projection_predecessor_production_sources_under_lock(
    workspace_root: &Path,
    superseded_paths: &[&str],
) -> Result<(), String> {
    validate_food_availability_projection_manifest_under_lock(workspace_root)?;
    let manifest_bytes = read_regular_file(workspace_root, MANIFEST_RELATIVE)?;
    let manifest: FoodAvailabilityProjectionManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| format!("parse {MANIFEST_RELATIVE}: {error}"))?;

    let (food_superseded_paths, nip09_superseded_paths) =
        partition_downstream_predecessor_supersessions(workspace_root, superseded_paths)?;
    validate_food_predecessor_source_inventory(&manifest, &food_superseded_paths, |spec| {
        describe_source_file(workspace_root, spec)
    })?;
    require_predecessor_file_match(
        "registry inventory",
        &manifest.registry_inventory,
        &descriptor_for_file(workspace_root, REGISTRY_INVENTORY_RELATIVE)?,
    )?;
    require_predecessor_file_match(
        "FoodAvailability profile vector",
        &manifest.food_profile_vector,
        &descriptor_for_file(workspace_root, FOOD_PROFILE_VECTOR_RELATIVE)?,
    )?;
    validate_predecessor_production_source_coverage(workspace_root, &nip09_superseded_paths)
}

fn partition_downstream_predecessor_supersessions<'a>(
    workspace_root: &Path,
    superseded_paths: &'a [&'a str],
) -> Result<(Vec<&'a str>, Vec<&'a str>), String> {
    let superseded = superseded_paths.iter().copied().collect::<BTreeSet<_>>();
    if superseded.len() != superseded_paths.len() {
        return Err("successor predecessor-source supersession paths must be unique".to_owned());
    }
    let food_paths = SOURCE_SPECS
        .iter()
        .map(|source| source.path)
        .collect::<BTreeSet<_>>();
    let nip09_paths = nip09_predecessor_production_source_paths_under_lock(workspace_root)?;
    let mut food_superseded_paths = Vec::new();
    let mut nip09_superseded_paths = Vec::new();
    for path in superseded_paths {
        if food_paths.contains(path) {
            food_superseded_paths.push(*path);
        }
        if nip09_paths.contains(*path) {
            nip09_superseded_paths.push(*path);
        }
        if !food_paths.contains(path) && !nip09_paths.contains(*path) {
            return Err(format!(
                "successor supersession path `{path}` is not bound by either the FoodAvailability or NIP-09 predecessor"
            ));
        }
    }
    Ok((food_superseded_paths, nip09_superseded_paths))
}

fn validate_food_predecessor_source_inventory<Describe>(
    manifest: &FoodAvailabilityProjectionManifest,
    superseded_paths: &[&str],
    mut describe: Describe,
) -> Result<(), String>
where
    Describe: FnMut(SourceSpec) -> Result<SourceFileDescriptor, String>,
{
    let superseded = superseded_paths.iter().copied().collect::<BTreeSet<_>>();
    if superseded.len() != superseded_paths.len() {
        return Err("successor predecessor-source supersession paths must be unique".to_owned());
    }

    let predecessor_paths = SOURCE_SPECS
        .iter()
        .map(|source| source.path)
        .collect::<BTreeSet<_>>();
    if let Some(path) = superseded
        .iter()
        .find(|path| !predecessor_paths.contains(**path))
    {
        return Err(format!(
            "successor supersession path `{path}` is not a FoodAvailability predecessor-bound production source"
        ));
    }

    if manifest.source_files.len() != SOURCE_SPECS.len() {
        return Err(
            "immutable FoodAvailability predecessor source inventory is incomplete".to_owned(),
        );
    }
    for (expected, spec) in manifest.source_files.iter().zip(SOURCE_SPECS) {
        if expected.role != spec.role || expected.path != spec.path {
            return Err(format!(
                "immutable FoodAvailability predecessor source inventory drifted at `{}`",
                spec.path
            ));
        }
        if superseded.contains(spec.path) {
            continue;
        }
        let current = describe(*spec)?;
        if current != *expected {
            return Err(format!(
                "unchanged FoodAvailability predecessor source authority `{}` drifted from the immutable manifest",
                spec.path
            ));
        }
    }
    Ok(())
}

fn require_predecessor_file_match(
    label: &str,
    expected: &FileDescriptor,
    current: &FileDescriptor,
) -> Result<(), String> {
    if current != expected {
        return Err(format!(
            "unchanged FoodAvailability predecessor {label} `{}` drifted from the immutable manifest",
            expected.path
        ));
    }
    Ok(())
}

fn describe_source_file(
    workspace_root: &Path,
    spec: SourceSpec,
) -> Result<SourceFileDescriptor, String> {
    let bytes = read_regular_file(workspace_root, spec.path)?;
    Ok(SourceFileDescriptor {
        role: spec.role.to_owned(),
        path: spec.path.to_owned(),
        byte_length: byte_length(spec.path, &bytes)?,
        sha256: sha256_hex(&bytes),
        hash_algorithm: HASH_ALGORITHM.to_owned(),
    })
}

fn descriptor_for_file(workspace_root: &Path, relative: &str) -> Result<FileDescriptor, String> {
    let bytes = read_regular_file(workspace_root, relative)?;
    descriptor_for_bytes(relative, &bytes)
}

fn descriptor_for_bytes(relative: &str, bytes: &[u8]) -> Result<FileDescriptor, String> {
    Ok(FileDescriptor {
        path: relative.to_owned(),
        byte_length: byte_length(relative, bytes)?,
        sha256: sha256_hex(bytes),
        hash_algorithm: HASH_ALGORITHM.to_owned(),
    })
}

fn byte_length(relative: &str, bytes: &[u8]) -> Result<u64, String> {
    u64::try_from(bytes.len()).map_err(|_| format!("{relative} byte length does not fit u64"))
}

fn catalog_from_migration_source(bytes: &[u8]) -> Result<CatalogDescriptor, String> {
    let source = std::str::from_utf8(bytes)
        .map_err(|error| format!("crates/event_store/src/migrations.rs must be UTF-8: {error}"))?;
    let syntax = syn::parse_file(source)
        .map_err(|error| format!("parse crates/event_store/src/migrations.rs: {error}"))?;
    Ok(CatalogDescriptor {
        objects: extract_string_array_const(&syntax, "EVENT_STORE_FOOD_AVAILABILITY_OBJECT_NAMES")?,
        tables: extract_string_array_const(&syntax, "EVENT_STORE_FOOD_AVAILABILITY_TABLE_NAMES")?,
        fts5_tables: extract_string_array_const(
            &syntax,
            "EVENT_STORE_FOOD_AVAILABILITY_FTS5_TABLE_NAMES",
        )?,
    })
}

fn extract_string_array_const(syntax: &syn::File, name: &str) -> Result<Vec<String>, String> {
    let expression = syntax.items.iter().find_map(|item| match item {
        Item::Const(item) if item.ident == name => Some(item.expr.as_ref()),
        Item::Static(item) if item.ident == name => Some(item.expr.as_ref()),
        _ => None,
    });
    let expression = expression.ok_or_else(|| {
        format!("crates/event_store/src/migrations.rs is missing catalog constant {name}")
    })?;
    let Expr::Array(array) = strip_expression_wrappers(expression) else {
        return Err(format!(
            "crates/event_store/src/migrations.rs catalog constant {name} must be a literal string array"
        ));
    };
    array
        .elems
        .iter()
        .map(|element| match strip_expression_wrappers(element) {
            Expr::Lit(literal) => match &literal.lit {
                Lit::Str(value) => Ok(value.value()),
                _ => Err(format!(
                    "catalog constant {name} must contain string literals"
                )),
            },
            _ => Err(format!(
                "catalog constant {name} must contain string literals"
            )),
        })
        .collect()
}

fn strip_expression_wrappers(mut expression: &Expr) -> &Expr {
    loop {
        expression = match expression {
            Expr::Reference(reference) => &reference.expr,
            Expr::Group(group) => &group.expr,
            Expr::Paren(paren) => &paren.expr,
            _ => return expression,
        };
    }
}

fn validate_catalog(catalog: &CatalogDescriptor) -> Result<(), String> {
    let expected_objects = EXPECTED_CATALOG_OBJECTS
        .iter()
        .map(|name| (*name).to_owned())
        .collect::<Vec<_>>();
    let expected_tables = EXPECTED_CATALOG_TABLES
        .iter()
        .map(|name| (*name).to_owned())
        .collect::<Vec<_>>();
    let expected_fts5 = EXPECTED_CATALOG_FTS5_TABLES
        .iter()
        .map(|name| (*name).to_owned())
        .collect::<Vec<_>>();
    if catalog.objects != expected_objects
        || catalog.tables != expected_tables
        || catalog.fts5_tables != expected_fts5
    {
        return Err(
            "FoodAvailability migration catalog differs from the successor contract".to_owned(),
        );
    }
    validate_unique(
        "migration catalog object",
        catalog.objects.iter().map(String::as_str),
    )?;
    validate_unique(
        "migration catalog table",
        catalog.tables.iter().map(String::as_str),
    )?;
    Ok(())
}

fn validate_migration_guard_limits(workspace_root: &Path) -> Result<(), String> {
    let addressable_model = read_regular_file(
        workspace_root,
        "crates/event_store/src/model/addressable_transition_feed_v1.rs",
    )?;
    let food_model = read_regular_file(
        workspace_root,
        "crates/event_store/src/model/food_availability_projection_v1.rs",
    )?;
    let migration = read_regular_file(workspace_root, MIGRATION_UP_RELATIVE)?;
    validate_migration_guard_limit_sources(
        std::str::from_utf8(&addressable_model).map_err(|error| {
            format!(
                "crates/event_store/src/model/addressable_transition_feed_v1.rs must be UTF-8: {error}"
            )
        })?,
        std::str::from_utf8(&food_model).map_err(|error| {
            format!(
                "crates/event_store/src/model/food_availability_projection_v1.rs must be UTF-8: {error}"
            )
        })?,
        std::str::from_utf8(&migration)
            .map_err(|error| format!("{MIGRATION_UP_RELATIVE} must be UTF-8: {error}"))?,
    )
}

fn validate_migration_guard_limit_sources(
    addressable_model_source: &str,
    food_model_source: &str,
    migration_source: &str,
) -> Result<(), String> {
    let addressable_model = syn::parse_file(addressable_model_source).map_err(|error| {
        format!("parse crates/event_store/src/model/addressable_transition_feed_v1.rs: {error}")
    })?;
    let food_model = syn::parse_file(food_model_source).map_err(|error| {
        format!("parse crates/event_store/src/model/food_availability_projection_v1.rs: {error}")
    })?;
    let scan_max = extract_u32_literal_const(
        &addressable_model,
        "RADROOTS_ADDRESSABLE_TRANSITION_PAGE_SCAN_MAX_V1",
    )?;
    let page_max = extract_u32_literal_const(
        &addressable_model,
        "RADROOTS_ADDRESSABLE_TRANSITION_PAGE_LIMIT_MAX_V1",
    )?;
    let apply_limit = find_const_expression(
        &food_model,
        "RADROOTS_FOOD_AVAILABILITY_PROJECTION_APPLY_PAGE_LIMIT_V1",
    )?;
    if compact_tokens(apply_limit) != "RADROOTS_ADDRESSABLE_TRANSITION_PAGE_LIMIT_MAX_V1" {
        return Err(
            "FoodAvailability projection apply limit must alias the governed addressable page limit"
                .to_owned(),
        );
    }

    let compact_sql = migration_source
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    let scan_guard = extract_direct_sql_guard(
        &compact_sql,
        "ORNEW.last_transition_seq-OLD.last_transition_seq>",
        1,
        "FoodAvailability cursor scan delta",
    )?;
    let row_count_guard = extract_direct_sql_guard(
        &compact_sql,
        "ORabs(NEW.projected_row_count-OLD.projected_row_count)>",
        2,
        "FoodAvailability projected-row delta",
    )?;
    if scan_guard != scan_max {
        return Err(format!(
            "{MIGRATION_UP_RELATIVE} cursor scan delta {scan_guard} differs from RADROOTS_ADDRESSABLE_TRANSITION_PAGE_SCAN_MAX_V1={scan_max}"
        ));
    }
    if row_count_guard != page_max {
        return Err(format!(
            "{MIGRATION_UP_RELATIVE} projected-row delta {row_count_guard} differs from RADROOTS_FOOD_AVAILABILITY_PROJECTION_APPLY_PAGE_LIMIT_V1={page_max}"
        ));
    }
    let visibility_index = "radroots_event_store_nip09_address_target_visibility_lookup_idx";
    let index_definition = format!(
        "CREATEINDEX{visibility_index}ONradroots_event_store_nip09_address_target(source_generation,target_kind,target_pubkey,target_d_tag,inclusive_cutoffDESC,request_event_idASC);"
    );
    if compact_sql.matches(&index_definition).count() != 1
        || compact_sql
            .matches(&format!("INDEXEDBY{visibility_index}"))
            .count()
            != 3
    {
        return Err(format!(
            "{MIGRATION_UP_RELATIVE} must define and force the canonical NIP-09 address-target visibility index"
        ));
    }
    if compact_sql
        .matches("ORDERBYtarget.request_event_idLIMIT1")
        .count()
        != 1
        || compact_sql
            .matches("ORDERBYtarget.inclusive_cutoffDESC,target.request_event_idLIMIT1")
            .count()
            != 2
    {
        return Err(format!(
            "{MIGRATION_UP_RELATIVE} must use the canonical NIP-09 visibility ordering"
        ));
    }
    Ok(())
}

fn find_const_expression<'a>(syntax: &'a syn::File, name: &str) -> Result<&'a Expr, String> {
    syntax
        .items
        .iter()
        .find_map(|item| match item {
            Item::Const(item) if item.ident == name => Some(item.expr.as_ref()),
            _ => None,
        })
        .ok_or_else(|| format!("governed Rust source is missing constant {name}"))
}

fn extract_u32_literal_const(syntax: &syn::File, name: &str) -> Result<u32, String> {
    let expression = strip_expression_wrappers(find_const_expression(syntax, name)?);
    let Expr::Lit(literal) = expression else {
        return Err(format!("governed constant {name} must be a u32 literal"));
    };
    let Lit::Int(value) = &literal.lit else {
        return Err(format!("governed constant {name} must be a u32 literal"));
    };
    value
        .base10_parse::<u32>()
        .map_err(|error| format!("parse governed constant {name}: {error}"))
}

fn extract_direct_sql_guard(
    compact_sql: &str,
    marker: &str,
    expected_occurrences: usize,
    label: &str,
) -> Result<u32, String> {
    let suffixes = compact_sql
        .match_indices(marker)
        .map(|(offset, _)| &compact_sql[offset + marker.len()..])
        .collect::<Vec<_>>();
    if suffixes.len() != expected_occurrences {
        return Err(format!(
            "{MIGRATION_UP_RELATIVE} must contain exactly {expected_occurrences} {label} guard occurrence(s)"
        ));
    }
    let direct_values = suffixes
        .iter()
        .filter_map(|suffix| {
            let digits = suffix
                .chars()
                .take_while(char::is_ascii_digit)
                .collect::<String>();
            (!digits.is_empty()).then_some(digits)
        })
        .collect::<Vec<_>>();
    if direct_values.len() != 1 {
        return Err(format!(
            "{MIGRATION_UP_RELATIVE} must contain exactly one direct numeric {label} guard"
        ));
    }
    direct_values[0]
        .parse::<u32>()
        .map_err(|error| format!("parse {label} guard: {error}"))
}

fn validate_fast_active_hook_source(source: &str) -> Result<(), String> {
    let syntax = syn::parse_file(source).map_err(|error| {
        format!("parse crates/event_store/src/nip09/reconciliation_v1.rs: {error}")
    })?;
    let function = syntax
        .items
        .iter()
        .find_map(|item| match item {
            Item::Fn(function) if function.sig.ident == "validate_active_hook_state_fast" => {
                Some(function)
            }
            _ => None,
        })
        .ok_or_else(|| {
            "predecessor source is missing validate_active_hook_state_fast".to_owned()
        })?;
    let body = compact_tokens(&function.block);
    let expected = "{validate_rebuild_marker_absent(connection).await?;validate_structural_source_state_fast(connection).await.map(|_|())}";
    if body != expected {
        return Err(
            "validate_active_hook_state_fast must remain the constant-cost rebuild-marker and structural-source check"
                .to_owned(),
        );
    }
    Ok(())
}

const FOOD_READ_AUTHORITY_JOINS: &str = "JOIN radroots_event_store_source_state AS source ON source.singleton = 1 AND source.active_generation = projection.source_generation JOIN radroots_event_store_food_availability_cursor AS cursor ON cursor.singleton = 1 AND cursor.source_generation = projection.source_generation JOIN radroots_event_store_addressable_head_state AS head ON head.source_generation = projection.source_generation AND head.kind = 30402 AND head.pubkey = projection.pubkey AND head.d_tag = projection.d_tag AND head.raw_head_event_id = projection.event_id AND head.raw_head_event_seq = projection.event_seq AND head.raw_head_created_at = projection.created_at AND head.admission_status = 'admitted' AND head.admission_code IS NULL AND head.contract_id = projection.contract_id AND head.visibility = 'visible' AND head.nip09_outcome = 'visible'";
const FOOD_RECENT_SOURCE_FIRST_AUTHORITY_JOINS: &str = "FROM radroots_event_store_source_state AS source CROSS JOIN radroots_event_store_food_availability_read_v1 AS projection ON source.singleton = 1 AND source.active_generation = projection.source_generation CROSS JOIN radroots_event_store_food_availability_cursor AS cursor ON cursor.singleton = 1 AND cursor.source_generation = projection.source_generation CROSS JOIN radroots_event_store_addressable_head_state AS head ON head.source_generation = projection.source_generation AND head.kind = 30402 AND head.pubkey = projection.pubkey AND head.d_tag = projection.d_tag AND head.raw_head_event_id = projection.event_id AND head.raw_head_event_seq = projection.event_seq AND head.raw_head_created_at = projection.created_at AND head.admission_status = 'admitted' AND head.admission_code IS NULL AND head.contract_id = projection.contract_id AND head.visibility = 'visible' AND head.nip09_outcome = 'visible'";

#[derive(Default)]
struct SqlxQueryRouteCollector {
    constants: Vec<String>,
    malformed: bool,
}

impl<'ast> Visit<'ast> for SqlxQueryRouteCollector {
    fn visit_expr_call(&mut self, expression: &'ast syn::ExprCall) {
        if compact_tokens(expression.func.as_ref()) == "sqlx::query" {
            match expression.args.iter().collect::<Vec<_>>().as_slice() {
                [Expr::Path(path)]
                    if path.qself.is_none()
                        && path.path.leading_colon.is_none()
                        && path.path.segments.len() == 1 =>
                {
                    self.constants.push(path.path.segments[0].ident.to_string());
                }
                _ => self.malformed = true,
            }
        }
        syn::visit::visit_expr_call(self, expression);
    }
}

fn validate_food_read_query_sources(source: &str) -> Result<(), String> {
    let syntax = syn::parse_file(source).map_err(|error| {
        format!("parse crates/event_store/src/store/food_availability_projection_v1.rs: {error}")
    })?;
    let expected_constants = [
        "FOOD_AVAILABILITY_POINT_QUERY_V1",
        "FOOD_AVAILABILITY_RECENT_QUERY_V1",
        "FOOD_AVAILABILITY_RECENT_STATUS_QUERY_V1",
        "FOOD_AVAILABILITY_SEARCH_QUERY_V1",
    ];
    let mut query_values = BTreeMap::new();
    let mut query_constant_names = BTreeSet::new();
    for item in &syntax.items {
        let Item::Const(item_const) = item else {
            continue;
        };
        let name = item_const.ident.to_string();
        if !name.starts_with("FOOD_AVAILABILITY_") || !name.ends_with("_QUERY_V1") {
            continue;
        }
        query_constant_names.insert(name.clone());
        if compact_tokens(&item_const.vis) != "pub(super)"
            || compact_tokens(item_const.ty.as_ref()) != "&str"
        {
            return Err(format!(
                "governed Food query constant {name} must be pub(super) const &str"
            ));
        }
        let Expr::Lit(literal) = strip_expression_wrappers(item_const.expr.as_ref()) else {
            return Err(format!(
                "governed Food query constant {name} must contain one string literal"
            ));
        };
        let Lit::Str(value) = &literal.lit else {
            return Err(format!(
                "governed Food query constant {name} must contain one string literal"
            ));
        };
        query_values.insert(name, value.value());
    }
    let expected_constant_names = expected_constants
        .iter()
        .map(|name| (*name).to_owned())
        .collect::<BTreeSet<_>>();
    if query_constant_names != expected_constant_names
        || query_values.len() != expected_constants.len()
    {
        return Err("governed Food query constant inventory is not exact".to_owned());
    }

    let expected_methods: [(&str, &[&str]); 3] = [
        (
            "food_availability_v1",
            &["FOOD_AVAILABILITY_POINT_QUERY_V1"],
        ),
        (
            "recent_food_availability_v1",
            &[
                "FOOD_AVAILABILITY_RECENT_QUERY_V1",
                "FOOD_AVAILABILITY_RECENT_STATUS_QUERY_V1",
            ],
        ),
        (
            "search_food_availability_v1",
            &["FOOD_AVAILABILITY_SEARCH_QUERY_V1"],
        ),
    ];
    let mut observed_methods = BTreeSet::new();
    for item in &syntax.items {
        let Item::Impl(item_impl) = item else {
            continue;
        };
        if compact_tokens(item_impl.self_ty.as_ref()) != "RadrootsEventStore" {
            continue;
        }
        for item in &item_impl.items {
            let syn::ImplItem::Fn(function) = item else {
                continue;
            };
            let method = function.sig.ident.to_string();
            let Some((_, expected_routes)) = expected_methods
                .iter()
                .find(|(expected, _)| *expected == method.as_str())
            else {
                continue;
            };
            if !observed_methods.insert(method.clone()) {
                return Err(format!("duplicate governed Food read method {method}"));
            }
            let mut collector = SqlxQueryRouteCollector::default();
            collector.visit_block(&function.block);
            if collector.malformed
                || collector.constants
                    != expected_routes
                        .iter()
                        .map(|name| (*name).to_owned())
                        .collect::<Vec<_>>()
            {
                return Err(format!(
                    "Food read method {method} does not route through its exact governed query constant(s)"
                ));
            }
        }
    }
    if observed_methods.len() != expected_methods.len() {
        return Err("governed Food read query inventory is incomplete".to_owned());
    }
    for constant in expected_constants {
        let query = query_values
            .get(constant)
            .expect("exact governed query constant inventory checked above");
        let normalized = query.split_whitespace().collect::<Vec<_>>().join(" ");
        let authority_joins = if constant == "FOOD_AVAILABILITY_RECENT_QUERY_V1" {
            FOOD_RECENT_SOURCE_FIRST_AUTHORITY_JOINS
        } else {
            FOOD_READ_AUTHORITY_JOINS
        };
        if normalized.contains("radroots_event_store_current_visibility_v1")
            || normalized.matches(authority_joins).count() != 1
        {
            return Err(
                "every Food point/recent/search query must use the exact fail-closed persisted source/cursor/head authority joins"
                    .to_owned(),
            );
        }
        let start = normalized
            .find(authority_joins)
            .expect("exact authority joins occurrence checked above");
        let suffix = &normalized[start..];
        let end = [" WHERE ", " ORDER BY "]
            .iter()
            .filter_map(|marker| suffix.find(marker))
            .min()
            .ok_or_else(|| {
                "governed Food read query has no clause after its authority joins".to_owned()
            })?;
        if &suffix[..end] != authority_joins {
            return Err(
                "Food read source/cursor/head authority joins contain an ungoverned predicate or disjunction"
                    .to_owned(),
            );
        }
    }
    Ok(())
}

fn validate_food_projection_audit_authority(source: &str) -> Result<(), String> {
    const EXPECTED_HEAD_QUERY: &str = "SELECT pubkey, d_tag, raw_head_event_id, raw_head_event_seq, raw_head_created_at FROM radroots_event_store_addressable_head_state WHERE source_generation = ? AND kind = 30402 AND admission_status = 'admitted' AND admission_code IS NULL AND contract_id = ? AND visibility = 'visible' AND nip09_outcome = 'visible' ORDER BY pubkey, d_tag";
    const SOURCE_TRANSITION_QUERY: &str = "SELECT EXISTS(SELECT 1 FROM radroots_event_store_addressable_head_transition AS transition WHERE transition.transition_seq = ? AND transition.source_generation = ? AND transition.source_generation = (SELECT active_generation FROM radroots_event_store_source_state WHERE singleton = 1) AND transition.kind = 30402 AND transition.pubkey = ? AND transition.d_tag = ? AND transition.raw_head_event_id = ? AND transition.raw_head_event_seq = ? AND transition.raw_head_created_at = ? AND transition.visible_event_id = ? AND transition.visible_event_seq = ? AND transition.admission_status = 'admitted' AND transition.admission_code IS NULL AND transition.contract_id = ? AND transition.visibility = 'visible' AND transition.nip09_outcome = 'visible' AND transition.raw_head_decision IN ('baseline_rebuild', 'applied') AND transition.transition_seq = (SELECT MAX(candidate.transition_seq) FROM radroots_event_store_addressable_head_transition AS candidate WHERE candidate.source_generation = transition.source_generation AND candidate.kind = transition.kind AND candidate.pubkey = transition.pubkey AND candidate.d_tag = transition.d_tag AND candidate.raw_head_decision IN ('baseline_rebuild', 'applied')))";

    let syntax = syn::parse_file(source).map_err(|error| {
        format!("parse crates/event_store/src/store/food_availability_projection_v1.rs: {error}")
    })?;
    let audit_methods = syntax
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Impl(item_impl)
                if compact_tokens(item_impl.self_ty.as_ref()) == "RadrootsEventStore" =>
            {
                Some(item_impl)
            }
            _ => None,
        })
        .flat_map(|item_impl| item_impl.items.iter())
        .filter_map(|item| match item {
            syn::ImplItem::Fn(function)
                if function.sig.ident == "audit_food_availability_projection_v1" =>
            {
                Some(function)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let [audit] = audit_methods.as_slice() else {
        return Err(format!(
            "Food projection store must define exactly one public audit method; found {}",
            audit_methods.len()
        ));
    };
    let audit_visibility = compact_tokens(&audit.vis);
    let audit_signature = compact_tokens(&audit.sig);
    if audit_visibility != "pub"
        || audit_signature
            != "asyncfnaudit_food_availability_projection_v1(&self,)->Result<(),RadrootsEventStoreError>"
    {
        return Err(format!(
            "Food projection audit method signature drifted: visibility `{audit_visibility}`, signature `{audit_signature}`"
        ));
    }
    let expected_audit: syn::Block = syn::parse_str(
        "{ let mut tx = self.begin_write_transaction().await?; validate_food_availability_projection_hook_v1(&mut tx).await?; tx.commit().await?; Ok(()) }",
    )
    .map_err(|error| format!("parse governed Food audit body: {error}"))?;
    if compact_tokens(&audit.block) != compact_tokens(&expected_audit) {
        return Err(
            "Food projection audit must use the exact serialized write transaction, exhaustive validator, commit route"
                .to_owned(),
        );
    }

    let exact_free_function = |name: &str| -> Result<&syn::ItemFn, String> {
        let functions = syntax
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Fn(function) if function.sig.ident == name => Some(function),
                _ => None,
            })
            .collect::<Vec<_>>();
        let [function] = functions.as_slice() else {
            return Err(format!(
                "Food projection store must define exactly one `{name}` function; found {}",
                functions.len()
            ));
        };
        Ok(function)
    };

    let exhaustive = exact_free_function("validate_food_availability_projection_hook_v1")?;
    let exhaustive_visibility = compact_tokens(&exhaustive.vis);
    let exhaustive_signature = compact_tokens(&exhaustive.sig);
    if exhaustive_visibility != "pub(crate)"
        || exhaustive_signature
            != "asyncfnvalidate_food_availability_projection_hook_v1(connection:&mutSqliteConnection,)->Result<(),RadrootsEventStoreError>"
    {
        return Err(format!(
            "Food exhaustive audit signature drifted: visibility `{exhaustive_visibility}`, signature `{exhaustive_signature}`"
        ));
    }
    let expected_exhaustive: syn::Block = syn::parse_str(
        r#"{
            let state = food_availability_projection_cursor_state_fast_v1(connection).await?;
            let generation = state.feed_cursor.source_generation();

            let rows = sqlx::query(
                "SELECT source_generation, pubkey, d_tag, event_id, event_seq, created_at, contract_id, content, title, summary, published_at, location, price_amount, price_currency, price_unit, quantity_amount, quantity_unit, status, diagnostic_codes_json, source_transition_seq, immutable_raw_json, stored_images_json FROM radroots_event_store_food_availability_read_v1 WHERE source_generation = ? ORDER BY pubkey, d_tag",
            )
            .bind(generation.as_bytes().as_slice())
            .fetch_all(&mut *connection)
            .await?;
            let mut actual_coordinates = Vec::with_capacity(rows.len());
            for row in rows {
                let projection = load_and_validate_projection_row(row)?;
                validate_projection_source_transition(connection, &projection).await?;
                validate_fts_row(connection, &projection).await?;
                actual_coordinates.push((
                    projection.pubkey().as_str().to_owned(),
                    projection.identifier().as_str().to_owned(),
                    projection.event_id().as_str().to_owned(),
                    projection.event_seq(),
                    i64_from_u64("food.created_at", projection.created_at())?,
                ));
            }
            let actual_row_count = i64::try_from(actual_coordinates.len())
                .map_err(|_| projection_drift("projection row count exceeds i64"))?;
            if actual_row_count != state.projected_row_count {
                return Err(projection_drift(format!(
                    "projection row count {} differs from sealed count {}",
                    actual_row_count, state.projected_row_count,
                )));
            }
            let expected_coordinates = sqlx::query(
                "SELECT pubkey, d_tag, raw_head_event_id, raw_head_event_seq, raw_head_created_at FROM radroots_event_store_addressable_head_state WHERE source_generation = ? AND kind = 30402 AND admission_status = 'admitted' AND admission_code IS NULL AND contract_id = ? AND visibility = 'visible' AND nip09_outcome = 'visible' ORDER BY pubkey, d_tag",
            )
            .bind(generation.as_bytes().as_slice())
            .bind(FOOD_AVAILABILITY_CONTRACT_ID)
            .fetch_all(&mut *connection)
            .await?
            .into_iter()
            .map(|row| {
                Ok::<_, sqlx::Error>((
                    row.try_get::<String, _>("pubkey")?,
                    row.try_get::<String, _>("d_tag")?,
                    row.try_get::<String, _>("raw_head_event_id")?,
                    row.try_get::<i64, _>("raw_head_event_seq")?,
                    row.try_get::<i64, _>("raw_head_created_at")?,
                ))
            })
            .collect::<Result<Vec<_>, _>>()?;
            if actual_coordinates != expected_coordinates {
                return Err(projection_drift(
                    "projection coordinate witnesses do not equal the current admitted, visible FoodAvailability heads",
                ));
            }
            let fts_count: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM radroots_event_store_food_availability_search_fts",
            )
            .fetch_one(&mut *connection)
            .await?;
            if fts_count != state.projected_row_count {
                return Err(projection_drift(format!(
                    "FoodAvailability FTS row count {fts_count} differs from sealed count {}",
                    state.projected_row_count,
                )));
            }
            #[cfg(test)]
            wait_at_food_availability_audit_fts_checkpoint().await;
            sqlx::query(
                "INSERT INTO radroots_event_store_food_availability_search_fts(radroots_event_store_food_availability_search_fts) VALUES('integrity-check')",
            )
            .execute(&mut *connection)
            .await
            .map_err(|error| projection_drift(format!("FoodAvailability FTS integrity check failed: {error}")))?;
            Ok(())
        }"#,
    )
    .map_err(|error| format!("parse governed exhaustive Food audit body: {error}"))?;
    if compact_tokens(&exhaustive.block) != compact_tokens(&expected_exhaustive) {
        return Err(
            "Food exhaustive audit complete function body drifted from the exact fail-closed authority seal"
                .to_owned(),
        );
    }

    let row_loops = exhaustive
        .block
        .stmts
        .iter()
        .filter_map(|statement| match statement {
            syn::Stmt::Expr(Expr::ForLoop(expression), _)
                if compact_tokens(&expression.pat) == "row"
                    && compact_tokens(&expression.expr) == "rows" =>
            {
                Some(expression)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let [row_loop] = row_loops.as_slice() else {
        return Err(format!(
            "Food exhaustive audit must contain exactly one direct projection-row loop; found {}",
            row_loops.len()
        ));
    };
    let expected_row_loop: syn::Block = syn::parse_str(
        r#"{
            let projection = load_and_validate_projection_row(row)?;
            validate_projection_source_transition(connection, &projection).await?;
            validate_fts_row(connection, &projection).await?;
            actual_coordinates.push((
                projection.pubkey().as_str().to_owned(),
                projection.identifier().as_str().to_owned(),
                projection.event_id().as_str().to_owned(),
                projection.event_seq(),
                i64_from_u64("food.created_at", projection.created_at())?,
            ));
        }"#,
    )
    .map_err(|error| format!("parse governed Food projection-row audit: {error}"))?;
    if compact_tokens(&row_loop.body) != compact_tokens(&expected_row_loop) {
        return Err(
            "Food exhaustive audit must validate each loaded projection's exact source transition before its FTS row and collect the five coordinate witnesses"
                .to_owned(),
        );
    }

    for (label, expected) in [
        (
            "checked actual coordinate cardinality",
            r#"let actual_row_count = i64::try_from(actual_coordinates.len())
                .map_err(|_| projection_drift("projection row count exceeds i64"))?;"#,
        ),
        (
            "sealed row-count equality",
            r#"if actual_row_count != state.projected_row_count {
                return Err(projection_drift(format!(
                    "projection row count {} differs from sealed count {}",
                    actual_row_count, state.projected_row_count,
                )));
            }"#,
        ),
        (
            "fail-closed coordinate equality",
            r#"if actual_coordinates != expected_coordinates {
                return Err(projection_drift(
                    "projection coordinate witnesses do not equal the current admitted, visible FoodAvailability heads",
                ));
            }"#,
        ),
    ] {
        let expected: syn::Stmt = syn::parse_str(expected)
            .map_err(|error| format!("parse governed Food {label} statement: {error}"))?;
        let occurrences = exhaustive
            .block
            .stmts
            .iter()
            .filter(|statement| compact_tokens(*statement) == compact_tokens(&expected))
            .count();
        if occurrences != 1 {
            return Err(format!(
                "Food exhaustive audit must contain exactly one {label} statement; found {occurrences}"
            ));
        }
    }

    let expected_coordinates = exhaustive
        .block
        .stmts
        .iter()
        .filter_map(|statement| match statement {
            syn::Stmt::Local(local)
                if matches!(&local.pat, syn::Pat::Ident(ident) if ident.ident == "expected_coordinates") =>
            {
                local.init.as_ref().map(|init| init.expr.as_ref())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let [expected_coordinates] = expected_coordinates.as_slice() else {
        return Err(format!(
            "Food exhaustive audit must define exactly one expected-coordinate query; found {}",
            expected_coordinates.len()
        ));
    };
    let expected_coordinates_expression: Expr = syn::parse_str(&format!(
        r#"sqlx::query({EXPECTED_HEAD_QUERY:?},)
            .bind(generation.as_bytes().as_slice())
            .bind(FOOD_AVAILABILITY_CONTRACT_ID)
            .fetch_all(&mut *connection)
            .await?
            .into_iter()
            .map(|row| {{
                Ok::<_, sqlx::Error>((
                    row.try_get::<String, _>("pubkey")?,
                    row.try_get::<String, _>("d_tag")?,
                    row.try_get::<String, _>("raw_head_event_id")?,
                    row.try_get::<i64, _>("raw_head_event_seq")?,
                    row.try_get::<i64, _>("raw_head_created_at")?,
                ))
            }})
            .collect::<Result<Vec<_>, _>>()?"#,
    ))
    .map_err(|error| format!("parse governed Food expected-coordinate query: {error}"))?;
    let actual_expected_coordinates = compact_tokens(*expected_coordinates);
    let governed_expected_coordinates = compact_tokens(&expected_coordinates_expression);
    if actual_expected_coordinates != governed_expected_coordinates {
        return Err(format!(
            "Food exhaustive audit expected-head query must bind the active generation and Food contract and map the exact five admitted visible head witnesses: expected `{governed_expected_coordinates}`, found `{actual_expected_coordinates}`"
        ));
    }

    let source_transition = exact_free_function("validate_projection_source_transition")?;
    let expected_source_transition: syn::Block = syn::parse_str(&format!(
        r#"{{
            let authoritative: i64 = sqlx::query_scalar({SOURCE_TRANSITION_QUERY:?},)
                .bind(projection.source_transition_seq())
                .bind(projection.source_generation().as_bytes().as_slice())
                .bind(projection.pubkey().as_str())
                .bind(projection.identifier().as_str())
                .bind(projection.event_id().as_str())
                .bind(projection.event_seq())
                .bind(i64_from_u64("food.created_at", projection.created_at())?)
                .bind(projection.event_id().as_str())
                .bind(projection.event_seq())
                .bind(FOOD_AVAILABILITY_CONTRACT_ID)
                .fetch_one(&mut *connection)
                .await?;
            if authoritative != 1 {{
                return Err(projection_drift(
                    "stored FoodAvailability source transition is not authoritative for its projection",
                ));
            }}
            Ok(())
        }}"#,
    ))
    .map_err(|error| format!("parse governed Food source-transition authority: {error}"))?;
    if compact_tokens(&source_transition.block) != compact_tokens(&expected_source_transition) {
        return Err(
            "Food source-transition audit must bind the exact active-generation, coordinate, head, visibility, admission, contract, and latest applied transition authority"
                .to_owned(),
        );
    }
    Ok(())
}

const SOURCE_CAPACITY_HOOK_MATCH_TOKENS: &str = "migration.hook,EventStoreMigrationHook::Nip09ReconciliationV1|EventStoreMigrationHook::FoodAvailabilityProjectionV1";

#[derive(Default)]
struct SourceCapacityAuthorityCollector {
    pending_hook_calls: usize,
    temp_schema_calls: usize,
    capacity_calls: usize,
    apply_migration_up_calls: usize,
    hook_matches: Vec<String>,
}

impl<'ast> Visit<'ast> for SourceCapacityAuthorityCollector {
    fn visit_expr_call(&mut self, expression: &'ast syn::ExprCall) {
        match compact_tokens(expression.func.as_ref()).as_str() {
            "has_pending_source_capacity_hook" => self.pending_hook_calls += 1,
            "validate_event_store_temp_schema_with_registry" => self.temp_schema_calls += 1,
            "validate_reconciliation_capacity" => self.capacity_calls += 1,
            "apply_migration_up" => self.apply_migration_up_calls += 1,
            _ => {}
        }
        syn::visit::visit_expr_call(self, expression);
    }

    fn visit_expr_macro(&mut self, expression: &'ast syn::ExprMacro) {
        if compact_tokens(&expression.mac.path) == "matches" {
            let tokens = compact_tokens(&expression.mac.tokens);
            if tokens.contains("EventStoreMigrationHook::Nip09ReconciliationV1")
                || tokens.contains("EventStoreMigrationHook::FoodAvailabilityProjectionV1")
            {
                self.hook_matches.push(tokens);
            }
        }
        syn::visit::visit_expr_macro(self, expression);
    }
}

fn exact_source_capacity_function<'a>(
    syntax: &'a syn::File,
    name: &str,
) -> Result<&'a syn::ItemFn, String> {
    let functions = syntax
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Fn(function) if function.sig.ident == name => Some(function),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [function] = functions.as_slice() else {
        return Err(format!(
            "Food schema source-capacity authority must define exactly one `{name}` function; found {}",
            functions.len()
        ));
    };
    Ok(function)
}

fn governed_statement_tokens(label: &str, source: &str) -> Result<String, String> {
    let block: syn::Block = syn::parse_str(&format!("{{ {source} }}"))
        .map_err(|error| format!("parse governed Food {label} statement: {error}"))?;
    let [statement] = block.stmts.as_slice() else {
        return Err(format!(
            "governed Food {label} source must parse as exactly one statement"
        ));
    };
    Ok(compact_tokens(statement))
}

fn exact_top_level_statement_index(
    block: &syn::Block,
    label: &str,
    expected_tokens: &str,
) -> Result<usize, String> {
    let indices = block
        .stmts
        .iter()
        .enumerate()
        .filter_map(|(index, statement)| {
            (compact_tokens(statement) == expected_tokens).then_some(index)
        })
        .collect::<Vec<_>>();
    let [index] = indices.as_slice() else {
        return Err(format!(
            "Food schema source-capacity authority must contain exactly one top-level {label} statement; found {}",
            indices.len()
        ));
    };
    Ok(*index)
}

fn validate_source_capacity_authority(source: &str) -> Result<(), String> {
    let syntax = syn::parse_file(source)
        .map_err(|error| format!("parse crates/event_store/src/schema.rs: {error}"))?;

    let pending = exact_source_capacity_function(&syntax, "has_pending_source_capacity_hook")?;
    let expected_pending: syn::Block = syn::parse_str(
        r#"{
            let current_version = match status {
                RadrootsEventStoreSchemaStatus::Uninitialized => return false,
                RadrootsEventStoreSchemaStatus::UnledgeredBaseline => registry[0].version,
                RadrootsEventStoreSchemaStatus::Managed { version } => *version,
            };
            registry.iter().any(|migration| {
                migration.version > current_version
                    && matches!(
                        migration.hook,
                        EventStoreMigrationHook::Nip09ReconciliationV1
                            | EventStoreMigrationHook::FoodAvailabilityProjectionV1
                    )
            })
        }"#,
    )
    .map_err(|error| format!("parse governed Food pending-capacity selector: {error}"))?;
    if compact_tokens(&pending.block) != compact_tokens(&expected_pending) {
        return Err(
            "Food schema source-capacity pending-hook selector must cover exactly the NIP-09 and Food projection rebuild hooks"
                .to_owned(),
        );
    }

    let outer = exact_source_capacity_function(
        &syntax,
        "migrate_event_store_schema_with_registry_and_generation_provider",
    )?;
    let expected_outer_preflight = governed_statement_tokens(
        "outer preflight",
        r#"
            if has_pending_source_capacity_hook(&status, registry) {
                let mut connection = pool.acquire().await?;
                validate_event_store_temp_schema_with_registry(&mut connection, registry).await?;
                validate_reconciliation_capacity(&mut connection, reconciliation_limits).await?;
            }
        "#,
    )?;
    let expected_begin_immediate = governed_statement_tokens(
        "BEGIN IMMEDIATE",
        r#"let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;"#,
    )?;
    let preflight_index = exact_top_level_statement_index(
        &outer.block,
        "outer preflight",
        &expected_outer_preflight,
    )?;
    let begin_immediate_index = exact_top_level_statement_index(
        &outer.block,
        "BEGIN IMMEDIATE",
        &expected_begin_immediate,
    )?;
    if preflight_index >= begin_immediate_index {
        return Err(
            "Food schema source-capacity outer preflight must complete before BEGIN IMMEDIATE"
                .to_owned(),
        );
    }
    let mut outer_authority = SourceCapacityAuthorityCollector::default();
    outer_authority.visit_block(&outer.block);
    if outer_authority.pending_hook_calls != 1
        || outer_authority.temp_schema_calls != 1
        || outer_authority.capacity_calls != 1
        || outer_authority.apply_migration_up_calls != 0
        || !outer_authority.hook_matches.is_empty()
    {
        return Err(format!(
            "Food schema source-capacity outer preflight call inventory is not exact: pending={}, temp_schema={}, capacity={}, apply_up={}, hook_matches={}",
            outer_authority.pending_hook_calls,
            outer_authority.temp_schema_calls,
            outer_authority.capacity_calls,
            outer_authority.apply_migration_up_calls,
            outer_authority.hook_matches.len()
        ));
    }

    let inner = exact_source_capacity_function(&syntax, "migrate_schema_on_connection")?;
    let mut migration_loops = Vec::new();
    for statement in &inner.block.stmts {
        let syn::Stmt::Expr(Expr::ForLoop(expression), _) = statement else {
            continue;
        };
        let mut collector = SourceCapacityAuthorityCollector::default();
        collector.visit_block(&expression.body);
        if collector.apply_migration_up_calls > 0 {
            migration_loops.push((expression, collector));
        }
    }
    let [(migration_loop, inner_authority)] = migration_loops.as_slice() else {
        return Err(format!(
            "Food schema source-capacity authority must define exactly one migration-application loop; found {}",
            migration_loops.len()
        ));
    };
    if inner_authority.pending_hook_calls != 0
        || inner_authority.temp_schema_calls != 0
        || inner_authority.capacity_calls != 1
        || inner_authority.apply_migration_up_calls != 1
        || inner_authority.hook_matches.len() != 1
        || inner_authority.hook_matches[0] != SOURCE_CAPACITY_HOOK_MATCH_TOKENS
    {
        return Err(format!(
            "Food schema source-capacity in-transaction recheck inventory is not exact: pending={}, temp_schema={}, capacity={}, apply_up={}, hook_matches={:?}",
            inner_authority.pending_hook_calls,
            inner_authority.temp_schema_calls,
            inner_authority.capacity_calls,
            inner_authority.apply_migration_up_calls,
            inner_authority.hook_matches
        ));
    }
    let expected_inner_recheck = governed_statement_tokens(
        "in-transaction recheck",
        r#"
            if matches!(
                migration.hook,
                EventStoreMigrationHook::Nip09ReconciliationV1
                    | EventStoreMigrationHook::FoodAvailabilityProjectionV1
            ) {
                validate_reconciliation_capacity(connection, reconciliation_limits).await?;
            }
        "#,
    )?;
    let expected_apply = governed_statement_tokens(
        "migration DDL application",
        "apply_migration_up(connection, registry, migration).await?;",
    )?;
    let recheck_index = exact_top_level_statement_index(
        &migration_loop.body,
        "in-transaction recheck",
        &expected_inner_recheck,
    )?;
    let apply_index = exact_top_level_statement_index(
        &migration_loop.body,
        "migration DDL application",
        &expected_apply,
    )?;
    if recheck_index.checked_add(1) != Some(apply_index) {
        return Err(
            "Food schema source-capacity in-transaction recheck must occur immediately before migration DDL"
                .to_owned(),
        );
    }

    Ok(())
}

fn validate_source_contract(workspace_root: &Path) -> Result<(), String> {
    validate_blossom_dependency_authority(workspace_root)?;
    validate_public_api_authority(workspace_root)?;
    validate_migration_guard_limits(workspace_root)?;
    require_source_markers(
        workspace_root,
        "crates/blossom/src/lib.rs",
        &["pubusehash::{", "RadrootsBlossomSha256"],
    )?;
    require_source_markers(
        workspace_root,
        "crates/blossom/src/hash.rs",
        &[
            "pubstructRadrootsBlossomSha256([u8;SHA256_BYTES])",
            "pubfnfrom_hex(value:&str)->Result<Self,RadrootsBlossomError>",
            "pubconstfnas_bytes(&self)->&[u8;SHA256_BYTES]",
            "pubfnto_hex(self)->String",
        ],
    )?;
    require_source_markers(
        workspace_root,
        "crates/event_codec/src/admission/registry_v7.rs",
        &[
            "pubfnadmit_verified_event_registry_v7",
            "project_verified_food_availability_event_registry_v7",
            "RADROOTS_FOOD_AVAILABILITY_CONTRACT_ID",
        ],
    )?;
    require_source_markers(
        workspace_root,
        MIGRATIONS_SOURCE_RELATIVE,
        &[
            "EventStoreMigrationHook::FoodAvailabilityProjectionV1",
            "version:3",
            "name:\"food_availability_projection\"",
            "include_str!(\"../migrations/0003_food_availability_projection.up.sql\")",
            "include_str!(\"../migrations/0003_food_availability_projection.down.sql\")",
            "validate_generated_food_availability_projection_manifest_descriptor()",
        ],
    )?;
    let schema_hooks = read_regular_file(workspace_root, "crates/event_store/src/schema.rs")?;
    let schema_hooks_source = std::str::from_utf8(&schema_hooks)
        .map_err(|error| format!("crates/event_store/src/schema.rs must be UTF-8: {error}"))?;
    validate_source_capacity_authority(schema_hooks_source)?;
    require_source_markers(
        workspace_root,
        "crates/event_store/src/schema.rs",
        &[
            "apply_food_availability_projection_hook_v1",
            "validate_food_availability_projection_hook_state_fast_v1",
            "has_pending_source_capacity_hook",
            "EventStoreMigrationHook::Nip09ReconciliationV1|EventStoreMigrationHook::FoodAvailabilityProjectionV1",
            "EventStoreMigrationHook::FoodAvailabilityProjectionV1",
        ],
    )?;
    require_source_markers(
        workspace_root,
        "crates/event_store/src/model/addressable_transition_feed_v1.rs",
        &[
            "RADROOTS_ADDRESSABLE_TRANSITION_FEED_VERSION_V1:u32=1",
            "pubfnfood_availability()->Self",
            "Self::new([30_402])",
            "SCOPE_FINGERPRINT_DOMAIN_V1",
        ],
    )?;
    require_source_markers(
        workspace_root,
        "crates/event_store/src/model/food_availability_projection_v1.rs",
        &[
            "RadrootsBlossomSha256",
            "pubconstfnblossom_sha256(&self)->Option<RadrootsBlossomSha256>",
        ],
    )?;
    require_source_markers(
        workspace_root,
        "crates/event_store/src/store/post_core_extension_dispatcher.rs",
        &["capabilities.apply_v1", "capabilities.apply_v2"],
    )?;
    let dispatcher = compact_source(
        workspace_root,
        "crates/event_store/src/store/post_core_extension_dispatcher.rs",
    )?;
    let v1 = dispatcher
        .find("capabilities.apply_v1")
        .ok_or_else(|| "post-core dispatcher is missing apply_v1".to_owned())?;
    let v2 = dispatcher
        .find("capabilities.apply_v2")
        .ok_or_else(|| "post-core dispatcher is missing apply_v2".to_owned())?;
    if v1 >= v2 {
        return Err("post-core dispatcher must apply v1 before additive v2".to_owned());
    }
    require_source_markers(
        workspace_root,
        "crates/event_store/src/store/post_core_extension_capabilities.rs",
        &["pub(super)asyncfnapply_v2", "apply_post_core_extensions_v2"],
    )?;
    require_source_markers(
        workspace_root,
        "crates/event_store/src/store/post_core_extensions_v2.rs",
        &["apply_pending_food_availability_transitions"],
    )?;
    let food_projection_store = read_regular_file(
        workspace_root,
        "crates/event_store/src/store/food_availability_projection_v1.rs",
    )?;
    let food_projection_store_source = std::str::from_utf8(&food_projection_store).map_err(
        |error| {
            format!(
                "crates/event_store/src/store/food_availability_projection_v1.rs must be UTF-8: {error}"
            )
        },
    )?;
    validate_food_read_query_sources(food_projection_store_source)?;
    validate_food_projection_audit_authority(food_projection_store_source)?;
    require_source_markers(
        workspace_root,
        "crates/event_store/src/store/food_availability_projection_v1.rs",
        &[
            "pubasyncfnfood_availability_v1",
            "pubasyncfnrecent_food_availability_v1",
            "pubasyncfnsearch_food_availability_v1",
            "pubasyncfnaudit_food_availability_projection_v1",
            "apply_food_availability_projection_hook_v1",
            "validate_food_availability_projection_hook_v1",
        ],
    )?;
    require_source_markers(
        workspace_root,
        "crates/event_store/src/store/current_visibility_v1.rs",
        &[
            "FROMradroots_event_store_addressable_head_stateASstate",
            "state.raw_head_event_id",
            "state.nip09_outcome",
        ],
    )?;
    let predecessor_fast_source = read_regular_file(
        workspace_root,
        "crates/event_store/src/nip09/reconciliation_v1.rs",
    )?;
    validate_fast_active_hook_source(std::str::from_utf8(&predecessor_fast_source).map_err(
        |error| format!("crates/event_store/src/nip09/reconciliation_v1.rs must be UTF-8: {error}"),
    )?)?;
    require_source_markers(
        workspace_root,
        "crates/event_store/src/store.rs",
        &[
            "dispatch_post_core_extensions",
            "PRAGMAmain.journal_mode=WAL",
            "SqliteFileJournalModeNotWal",
        ],
    )?;
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PublicUseRoute {
    segments: Vec<String>,
    exported_name: String,
    renamed: bool,
    glob: bool,
    absolute: bool,
    attributes: Vec<String>,
}

fn validate_public_api_authority(workspace_root: &Path) -> Result<(), String> {
    let model_bytes = read_regular_file(workspace_root, EVENT_STORE_MODEL_RELATIVE)?;
    let model_source = std::str::from_utf8(&model_bytes)
        .map_err(|error| format!("{EVENT_STORE_MODEL_RELATIVE} must be UTF-8 Rust: {error}"))?;
    let lib_bytes = read_regular_file(workspace_root, EVENT_STORE_LIB_RELATIVE)?;
    let lib_source = std::str::from_utf8(&lib_bytes)
        .map_err(|error| format!("{EVENT_STORE_LIB_RELATIVE} must be UTF-8 Rust: {error}"))?;
    let advertised = PUBLIC_API
        .iter()
        .map(|name| (*name).to_owned())
        .collect::<Vec<_>>();
    validate_public_api_sources(model_source, lib_source, &advertised)
}

fn validate_public_api_sources(
    model_source: &str,
    lib_source: &str,
    advertised: &[String],
) -> Result<(), String> {
    let model = syn::parse_file(model_source)
        .map_err(|error| format!("parse {EVENT_STORE_MODEL_RELATIVE}: {error}"))?;
    let lib = syn::parse_file(lib_source)
        .map_err(|error| format!("parse {EVENT_STORE_LIB_RELATIVE}: {error}"))?;

    let governed_modules = GOVERNED_PUBLIC_API_MODULES
        .iter()
        .map(|module| (*module).to_owned())
        .collect::<BTreeSet<_>>();
    let mut represented_modules = BTreeSet::new();
    let mut governed_exports = BTreeSet::new();
    for route in collect_top_level_public_use_routes(&model) {
        let Some(module) = route.segments.first() else {
            continue;
        };
        if !governed_modules.contains(module) {
            continue;
        }
        if route.absolute || route.renamed || route.glob || route.segments.len() != 2 {
            return Err(format!(
                "{EVENT_STORE_MODEL_RELATIVE} governed public use `{}` must be a direct, non-renamed symbol re-export",
                route.segments.join("::")
            ));
        }
        represented_modules.insert(module.clone());
        if !governed_exports.insert(route.exported_name.clone()) {
            return Err(format!(
                "{EVENT_STORE_MODEL_RELATIVE} exports governed symbol `{}` more than once",
                route.exported_name
            ));
        }
    }
    if represented_modules != governed_modules {
        return Err(format!(
            "{EVENT_STORE_MODEL_RELATIVE} governed public API modules differ: expected {governed_modules:?}, found {represented_modules:?}"
        ));
    }

    let advertised_exports = advertised.iter().cloned().collect::<BTreeSet<_>>();
    if advertised_exports.len() != advertised.len() {
        return Err("successor PUBLIC_API contains duplicate symbols".to_owned());
    }
    if advertised_exports != governed_exports {
        let missing = governed_exports
            .difference(&advertised_exports)
            .cloned()
            .collect::<Vec<_>>();
        let unexpected = advertised_exports
            .difference(&governed_exports)
            .cloned()
            .collect::<Vec<_>>();
        return Err(format!(
            "successor PUBLIC_API is not exhaustive for governed model exports; missing {missing:?}, unexpected {unexpected:?}"
        ));
    }

    let sqlite_cfg = "#[cfg(feature=\"sqlite\")]";
    let mut crate_root_exports = BTreeSet::new();
    for route in collect_top_level_public_use_routes(&lib) {
        if route.segments.first().map(String::as_str) != Some("model") {
            continue;
        }
        if route.attributes.as_slice() != [sqlite_cfg]
            || route.absolute
            || route.renamed
            || route.glob
            || route.segments.len() != 2
        {
            return Err(format!(
                "{EVENT_STORE_LIB_RELATIVE} public model use `{}` must be a direct, non-renamed #[cfg(feature = \"sqlite\")] re-export",
                route.segments.join("::")
            ));
        }
        if !crate_root_exports.insert(route.exported_name.clone()) {
            return Err(format!(
                "{EVENT_STORE_LIB_RELATIVE} exports model symbol `{}` more than once",
                route.exported_name
            ));
        }
    }
    let missing_at_crate_root = governed_exports
        .difference(&crate_root_exports)
        .cloned()
        .collect::<Vec<_>>();
    if !missing_at_crate_root.is_empty() {
        return Err(format!(
            "{EVENT_STORE_LIB_RELATIVE} does not re-export governed successor symbols {missing_at_crate_root:?}"
        ));
    }
    Ok(())
}

fn collect_top_level_public_use_routes(file: &syn::File) -> Vec<PublicUseRoute> {
    let mut routes = Vec::new();
    for item in &file.items {
        let Item::Use(item_use) = item else {
            continue;
        };
        if !matches!(item_use.vis, syn::Visibility::Public(_)) {
            continue;
        }
        let attributes = item_use
            .attrs
            .iter()
            .map(compact_tokens)
            .collect::<Vec<_>>();
        let mut segments = Vec::new();
        flatten_public_use_tree(
            &item_use.tree,
            &mut segments,
            item_use.leading_colon.is_some(),
            &attributes,
            &mut routes,
        );
    }
    routes
}

fn flatten_public_use_tree(
    tree: &syn::UseTree,
    segments: &mut Vec<String>,
    absolute: bool,
    attributes: &[String],
    routes: &mut Vec<PublicUseRoute>,
) {
    match tree {
        syn::UseTree::Path(path) => {
            segments.push(path.ident.to_string());
            flatten_public_use_tree(&path.tree, segments, absolute, attributes, routes);
            segments.pop();
        }
        syn::UseTree::Name(name) => {
            let exported_name = name.ident.to_string();
            let mut route_segments = segments.clone();
            route_segments.push(exported_name.clone());
            routes.push(PublicUseRoute {
                segments: route_segments,
                exported_name,
                renamed: false,
                glob: false,
                absolute,
                attributes: attributes.to_vec(),
            });
        }
        syn::UseTree::Rename(rename) => {
            let mut route_segments = segments.clone();
            route_segments.push(rename.ident.to_string());
            routes.push(PublicUseRoute {
                segments: route_segments,
                exported_name: rename.rename.to_string(),
                renamed: true,
                glob: false,
                absolute,
                attributes: attributes.to_vec(),
            });
        }
        syn::UseTree::Glob(_) => {
            let mut route_segments = segments.clone();
            route_segments.push("*".to_owned());
            routes.push(PublicUseRoute {
                segments: route_segments,
                exported_name: "*".to_owned(),
                renamed: false,
                glob: true,
                absolute,
                attributes: attributes.to_vec(),
            });
        }
        syn::UseTree::Group(group) => {
            for item in &group.items {
                flatten_public_use_tree(item, segments, absolute, attributes, routes);
            }
        }
    }
}

fn compact_tokens(tokens: &impl ToTokens) -> String {
    tokens
        .to_token_stream()
        .to_string()
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

fn validate_blossom_dependency_authority(workspace_root: &Path) -> Result<(), String> {
    let workspace = parse_toml_value(workspace_root, "Cargo.toml")?;
    let event_store = parse_toml_value(workspace_root, "crates/event_store/Cargo.toml")?;
    validate_blossom_dependency_values(&workspace, &event_store)
}

fn parse_toml_value(workspace_root: &Path, relative: &str) -> Result<toml::Value, String> {
    let bytes = read_regular_file(workspace_root, relative)?;
    let source = std::str::from_utf8(&bytes)
        .map_err(|error| format!("{relative} must be UTF-8 TOML: {error}"))?;
    toml::from_str(source).map_err(|error| format!("parse {relative}: {error}"))
}

fn validate_blossom_dependency_values(
    workspace: &toml::Value,
    event_store: &toml::Value,
) -> Result<(), String> {
    let workspace_dependency = workspace
        .get("workspace")
        .and_then(|value| value.get("dependencies"))
        .and_then(|value| value.get("radroots_blossom"))
        .and_then(toml::Value::as_table)
        .ok_or_else(|| "Cargo.toml must define workspace dependency radroots_blossom".to_owned())?;
    if workspace_dependency
        .get("package")
        .and_then(toml::Value::as_str)
        != Some("radroots-blossom")
        || workspace_dependency
            .get("path")
            .and_then(toml::Value::as_str)
            != Some("crates/blossom")
        || workspace_dependency
            .get("version")
            .and_then(toml::Value::as_str)
            != Some("=0.1.0")
        || workspace_dependency
            .get("default-features")
            .and_then(toml::Value::as_bool)
            != Some(false)
    {
        return Err(
            "Cargo.toml radroots_blossom dependency must pin the standalone crate with defaults disabled"
                .to_owned(),
        );
    }

    let event_store_dependency = event_store
        .get("dependencies")
        .and_then(|value| value.get("radroots_blossom"))
        .and_then(toml::Value::as_table)
        .ok_or_else(|| {
            "crates/event_store/Cargo.toml must directly depend on radroots_blossom".to_owned()
        })?;
    let features = event_store_dependency
        .get("features")
        .and_then(toml::Value::as_array)
        .map(|features| {
            features
                .iter()
                .filter_map(toml::Value::as_str)
                .collect::<Vec<_>>()
        });
    if event_store_dependency
        .get("workspace")
        .and_then(toml::Value::as_bool)
        != Some(true)
        || event_store_dependency
            .get("default-features")
            .and_then(toml::Value::as_bool)
            != Some(false)
        || features.as_deref() != Some(&["std"][..])
    {
        return Err(
            "crates/event_store/Cargo.toml radroots_blossom dependency must select only the governed std feature"
                .to_owned(),
        );
    }
    Ok(())
}

fn require_source_markers(
    workspace_root: &Path,
    relative: &str,
    markers: &[&str],
) -> Result<(), String> {
    let compact = compact_source(workspace_root, relative)?;
    for marker in markers {
        if !compact.contains(marker) {
            return Err(format!(
                "{relative} is missing required successor route `{marker}`"
            ));
        }
    }
    Ok(())
}

fn compact_source(workspace_root: &Path, relative: &str) -> Result<String, String> {
    let bytes = read_regular_file(workspace_root, relative)?;
    let source = std::str::from_utf8(&bytes)
        .map_err(|error| format!("{relative} must be UTF-8 Rust source: {error}"))?;
    let syntax = syn::parse_file(source)
        .map_err(|error| format!("parse governed Rust source {relative}: {error}"))?;
    Ok(syntax
        .into_token_stream()
        .to_string()
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect())
}

fn validate_result_vector(vector: &ProjectionResultVector) -> Result<(), String> {
    if vector.schema_version != SCHEMA_VERSION
        || vector.contract_id != CONTRACT_ID
        || vector.feed_version != ADDRESSABLE_FEED_VERSION
        || vector.projection_version != PROJECTION_VERSION
        || vector.scope_kinds != [FOOD_AVAILABILITY_KIND]
    {
        return Err(format!(
            "{RESULT_VECTOR_CANONICAL_RELATIVE} has an invalid successor identity"
        ));
    }
    let required_cases = [
        "visible_food_availability_projects_and_searches",
        "invalid_same_timestamp_winner_retracts_projection",
        "blossom_digest_and_image_diagnostics_are_preserved",
        "authorized_address_deletion_retracts_projection",
        "wrong_author_address_deletion_preserves_projection",
        "post_cutoff_replacement_restores_projection",
        "operational_listing_head_retracts_food_projection",
        "food_head_after_operational_listing_restores_projection",
        "food_feed_cursor_advances_across_unrelated_addressable_traffic",
    ];
    if vector.cases.len() != required_cases.len() {
        return Err(format!(
            "{RESULT_VECTOR_CANONICAL_RELATIVE} must contain exactly the nine required cases"
        ));
    }
    validate_unique(
        "result-vector case id",
        vector.cases.iter().map(|case| case.id.as_str()),
    )?;
    if vector
        .cases
        .iter()
        .map(|case| case.id.as_str())
        .ne(required_cases)
    {
        return Err(format!(
            "{RESULT_VECTOR_CANONICAL_RELATIVE} case order or inventory is not canonical"
        ));
    }

    for case in &vector.cases {
        let transition_page = &case.expected.transition_page;
        if case.events.is_empty() || transition_page.transitions.is_empty() {
            return Err(format!(
                "{} must contain input events and scoped transitions",
                case.id
            ));
        }
        if case.expected.coordinate.kind != FOOD_AVAILABILITY_KIND {
            return Err(format!(
                "{} coordinate kind must be {FOOD_AVAILABILITY_KIND}",
                case.id
            ));
        }
        validate_hex(
            &format!("{} coordinate pubkey", case.id),
            &case.expected.coordinate.pubkey,
            64,
        )?;
        if case.expected.coordinate.d_tag.is_empty() {
            return Err(format!("{} coordinate d_tag must not be empty", case.id));
        }

        let mut event_ids = BTreeSet::new();
        let mut food_event_ids = BTreeSet::new();
        let mut scoped_coordinate_event_ids = BTreeSet::new();
        let mut events_by_id = BTreeMap::new();
        for (index, observed) in case.events.iter().enumerate() {
            if observed.observed_at_ms < 0 {
                return Err(format!(
                    "{} event observed_at_ms must be non-negative",
                    case.id
                ));
            }
            validate_hex(&format!("{} event id", case.id), &observed.event.id, 64)?;
            validate_hex(
                &format!("{} event pubkey", case.id),
                &observed.event.pubkey,
                64,
            )?;
            validate_hex(
                &format!("{} event signature", case.id),
                &observed.event.sig,
                128,
            )?;
            if observed.event.tags.iter().any(Vec::is_empty)
                || !event_ids.insert(observed.event.id.as_str())
            {
                return Err(format!(
                    "{} contains an invalid or duplicate signed event",
                    case.id
                ));
            }
            validate_expected_ingest(&case.id, observed)?;
            match observed.role {
                ProjectionInputRole::ScopedFood | ProjectionInputRole::ScopedNonFood => {
                    let d_tags = observed
                        .event
                        .tags
                        .iter()
                        .filter(|tag| tag.first().map(String::as_str) == Some("d"))
                        .filter_map(|tag| tag.get(1).map(String::as_str))
                        .collect::<Vec<_>>();
                    if observed.event.kind != FOOD_AVAILABILITY_KIND
                        || observed.event.pubkey != case.expected.coordinate.pubkey
                        || d_tags.as_slice() != [case.expected.coordinate.d_tag.as_str()]
                    {
                        return Err(format!(
                            "{} scoped input does not match its kind-30402 coordinate",
                            case.id
                        ));
                    }
                    scoped_coordinate_event_ids.insert(observed.event.id.as_str());
                    if observed.role == ProjectionInputRole::ScopedFood {
                        food_event_ids.insert(observed.event.id.as_str());
                    }
                }
                ProjectionInputRole::UnrelatedAddressable => {
                    let d_tags = observed
                        .event
                        .tags
                        .iter()
                        .filter(|tag| tag.first().map(String::as_str) == Some("d"))
                        .filter_map(|tag| tag.get(1).map(String::as_str))
                        .collect::<Vec<_>>();
                    if observed.event.kind != UNRELATED_ADDRESSABLE_KIND
                        || d_tags.len() != 1
                        || d_tags[0].is_empty()
                    {
                        return Err(format!(
                            "{} unrelated addressable input must be a kind-{UNRELATED_ADDRESSABLE_KIND} event with one non-empty d tag",
                            case.id
                        ));
                    }
                }
                ProjectionInputRole::Causal if observed.event.kind != 5 => {
                    return Err(format!(
                        "{} causal input must be a kind-5 deletion request",
                        case.id
                    ));
                }
                ProjectionInputRole::Causal => {}
            }
            let event_seq = i64::try_from(index + 1)
                .map_err(|_| format!("{} event sequence exceeds i64", case.id))?;
            events_by_id.insert(observed.event.id.as_str(), (event_seq, observed));
        }
        if food_event_ids.is_empty() {
            return Err(format!("{} has no scoped FoodAvailability input", case.id));
        }
        let coordinate_address = format!(
            "{}:{}:{}",
            case.expected.coordinate.kind,
            case.expected.coordinate.pubkey,
            case.expected.coordinate.d_tag
        );
        for observed in case
            .events
            .iter()
            .filter(|observed| observed.role == ProjectionInputRole::Causal)
        {
            let references_scope = observed.event.tags.iter().any(|tag| {
                (tag.first().map(String::as_str) == Some("a")
                    && tag.get(1).map(String::as_str) == Some(coordinate_address.as_str()))
                    || (tag.first().map(String::as_str) == Some("e")
                        && tag.get(1).is_some_and(|event_id| {
                            scoped_coordinate_event_ids.contains(event_id.as_str())
                        }))
            });
            if !references_scope {
                return Err(format!(
                    "{} causal input does not reference the scoped coordinate or event",
                    case.id
                ));
            }
        }

        let projection_id = case
            .expected
            .projection
            .0
            .as_ref()
            .map(|projection| projection.event_id.as_str());
        if projection_id.is_some_and(|id| !food_event_ids.contains(id)) {
            return Err(format!(
                "{} projection must reference a scoped FoodAvailability input event",
                case.id
            ));
        }
        if let Some(projection) = case.expected.projection.0.as_ref() {
            for image in &projection.images {
                if let Some(digest) = image.blossom_sha256.0.as_deref() {
                    validate_sha256(&format!("{} Blossom image digest", case.id), digest)?;
                }
            }
        }
        for search in &case.expected.searches {
            if search.query.trim().is_empty() {
                return Err(format!("{} search query must not be empty", case.id));
            }
            validate_unique(
                &format!("{} search event id", case.id),
                search.event_ids.iter().map(String::as_str),
            )?;
            for event_id in &search.event_ids {
                validate_hex(&format!("{} search event id", case.id), event_id, 64)?;
                if Some(event_id.as_str()) != projection_id {
                    return Err(format!(
                        "{} search result must equal the current projection",
                        case.id
                    ));
                }
            }
        }

        let expected_high_water = i64::try_from(case.events.len())
            .map_err(|_| format!("{} input event count exceeds i64", case.id))?;
        if transition_page.source_high_water != expected_high_water
            || transition_page.has_more
            || transition_page.next_cursor.source_generation != SOURCE_GENERATION_ACTIVE_SENTINEL
            || transition_page.next_cursor.feed_version != ADDRESSABLE_FEED_VERSION
            || transition_page.next_cursor.scope_fingerprint != SCOPE_FINGERPRINT_SHA256
            || transition_page.next_cursor.last_transition_seq != expected_high_water
        {
            return Err(format!(
                "{} transition page does not seal the complete active-source interval",
                case.id
            ));
        }
        let expected_transition_sequences = case
            .events
            .iter()
            .enumerate()
            .filter(|(_, observed)| observed.role != ProjectionInputRole::UnrelatedAddressable)
            .map(|(index, _)| {
                i64::try_from(index + 1)
                    .map_err(|_| format!("{} transition sequence exceeds i64", case.id))
            })
            .collect::<Result<BTreeSet<_>, _>>()?;
        let actual_transition_sequences = transition_page
            .transitions
            .iter()
            .map(|transition| transition.transition_seq)
            .collect::<BTreeSet<_>>();
        if actual_transition_sequences.len() != transition_page.transitions.len()
            || actual_transition_sequences != expected_transition_sequences
        {
            return Err(format!(
                "{} scoped transition sequences do not exactly skip unrelated addressable traffic",
                case.id
            ));
        }

        let mut prior_visible: Option<&ExpectedEventReference> = None;
        let mut last_cause_event_seq = 0_i64;
        let mut last_transition_seq = 0_i64;
        for transition in &transition_page.transitions {
            if transition.transition_seq <= last_transition_seq
                || transition.transition_seq > expected_high_water
                || transition.source_generation != SOURCE_GENERATION_ACTIVE_SENTINEL
                || transition.origin != "incremental"
                || transition.coordinate != case.expected.coordinate
            {
                return Err(format!(
                    "{} transition {} has an invalid authority witness",
                    case.id, transition.transition_seq
                ));
            }
            last_transition_seq = transition.transition_seq;
            let raw_head = validate_vector_event_reference(
                &case.id,
                "raw head",
                &transition.raw_head,
                &events_by_id,
            )?;
            if !matches!(
                raw_head.role,
                ProjectionInputRole::ScopedFood | ProjectionInputRole::ScopedNonFood
            ) || transition.raw_head_created_at != raw_head.event.created_at
                || transition.admission_status != raw_head.expected_ingest.admission_status
                || transition.admission_code.0 != raw_head.expected_ingest.admission_code.0
                || transition.contract_id.0 != raw_head.expected_ingest.contract_id.0
            {
                return Err(format!(
                    "{} transition {} raw-head metadata is not canonical",
                    case.id, transition.transition_seq
                ));
            }
            let cause = transition.cause_event.0.as_ref().ok_or_else(|| {
                format!(
                    "{} incremental transition {} must authenticate its cause",
                    case.id, transition.transition_seq
                )
            })?;
            let cause_source = validate_vector_event_reference(
                &case.id,
                "transition cause",
                &cause.event,
                &events_by_id,
            )?;
            if cause.event.event_seq != transition.transition_seq
                || cause.event.event_seq <= last_cause_event_seq
                || cause.pubkey != cause_source.event.pubkey
                || cause.created_at != cause_source.event.created_at
                || cause.kind != cause_source.event.kind
                || cause.admission_status != cause_source.expected_ingest.admission_status
                || cause.admission_code.0 != cause_source.expected_ingest.admission_code.0
                || cause.contract_id.0 != cause_source.expected_ingest.contract_id.0
                || transition.raw_head_decision != cause_source.expected_ingest.raw_head_decision
            {
                return Err(format!(
                    "{} transition {} cause metadata is not canonical",
                    case.id, transition.transition_seq
                ));
            }
            last_cause_event_seq = cause.event.event_seq;

            validate_transition_decision_shape(&case.id, transition)?;
            if let Some(evidence) = transition.suppression.0.as_ref() {
                validate_suppression_evidence(
                    &case.id,
                    evidence,
                    &events_by_id,
                    raw_head.event.kind,
                    raw_head.event.created_at,
                )?;
            }
            if let Some(canonical) = transition.canonical_visible_event.0.as_ref() {
                let source = validate_vector_event_reference(
                    &case.id,
                    "canonical visible event",
                    &canonical.event,
                    &events_by_id,
                )?;
                if canonical.event != transition.raw_head
                    || canonical.admission_status != transition.admission_status
                    || canonical.contract_id.0 != transition.contract_id.0
                    || canonical.event_class != source.expected_ingest.event_class
                    || canonical.valid_stream_eligible
                        != source.expected_ingest.valid_stream_eligible
                {
                    return Err(format!(
                        "{} transition {} canonical event metadata is invalid",
                        case.id, transition.transition_seq
                    ));
                }
                validate_sha256(
                    &format!("{} canonical raw JSON digest", case.id),
                    &canonical.raw_json_sha256,
                )?;
                let raw_json = serde_json::to_vec(&source.event)
                    .map_err(|error| format!("serialize {} signed event: {error}", case.id))?;
                if sha256_hex(&raw_json) != canonical.raw_json_sha256 {
                    return Err(format!(
                        "{} transition {} canonical raw JSON digest is stale",
                        case.id, transition.transition_seq
                    ));
                }
            }
            if let Some(retracted) = transition.retracted_event.0.as_ref() {
                validate_vector_event_reference(
                    &case.id,
                    "retracted event",
                    retracted,
                    &events_by_id,
                )?;
                if retracted.event_seq >= transition.transition_seq {
                    return Err(format!(
                        "{} transition {} retracts a non-prior event",
                        case.id, transition.transition_seq
                    ));
                }
            }
            let next_visible = transition
                .canonical_visible_event
                .0
                .as_ref()
                .map(|event| &event.event);
            let expected_retracted = (prior_visible != next_visible)
                .then_some(prior_visible)
                .flatten();
            if transition.retracted_event.0.as_ref() != expected_retracted {
                return Err(format!(
                    "{} transition {} has invalid retraction lineage",
                    case.id, transition.transition_seq
                ));
            }
            prior_visible = next_visible;
        }

        validate_unique(
            &format!("{} visibility event id", case.id),
            case.expected
                .event_visibility
                .iter()
                .map(|visibility| visibility.event.event_id.as_str()),
        )?;
        if case.expected.event_visibility.len() != event_ids.len()
            || case
                .expected
                .event_visibility
                .iter()
                .any(|visibility| !event_ids.contains(visibility.event.event_id.as_str()))
        {
            return Err(format!(
                "{} visibility expectations must cover every input event exactly once",
                case.id
            ));
        }
        let current_scoped_raw_head = transition_page
            .transitions
            .last()
            .expect("non-empty transition page checked above")
            .raw_head
            .event_id
            .as_str();
        for visibility in &case.expected.event_visibility {
            let source = validate_vector_event_reference(
                &case.id,
                "current visibility event",
                &visibility.event,
                &events_by_id,
            )?;
            let expected_raw_head = match source.role {
                ProjectionInputRole::Causal => None,
                ProjectionInputRole::ScopedFood | ProjectionInputRole::ScopedNonFood => {
                    Some(current_scoped_raw_head)
                }
                ProjectionInputRole::UnrelatedAddressable => Some(source.event.id.as_str()),
            };
            let expected_is_raw_head =
                expected_raw_head.is_none_or(|event_id| event_id == source.event.id.as_str());
            if visibility.source_generation != SOURCE_GENERATION_ACTIVE_SENTINEL
                || visibility.admission_status != source.expected_ingest.admission_status
                || visibility.is_raw_head != expected_is_raw_head
                || visibility.raw_head_event_id.0.as_deref() != expected_raw_head
            {
                return Err(format!(
                    "{} current visibility witness for {} is not authoritative",
                    case.id, source.event.id
                ));
            }
            if let Some(evidence) = visibility.suppression.0.as_ref() {
                validate_suppression_evidence(
                    &case.id,
                    evidence,
                    &events_by_id,
                    source.event.kind,
                    source.event.created_at,
                )?;
            }
            let coherent = match visibility.decision.as_str() {
                "visible" => {
                    visibility.admission_status == "admitted"
                        && visibility.is_raw_head
                        && visibility
                            .suppression
                            .0
                            .as_ref()
                            .is_some_and(|evidence| evidence.outcome == "visible")
                }
                "not_current" => {
                    visibility.admission_status == "admitted"
                        && !visibility.is_raw_head
                        && visibility.suppression.0.is_some()
                }
                "suppressed" => {
                    visibility.admission_status == "admitted"
                        && visibility.is_raw_head
                        && visibility
                            .suppression
                            .0
                            .as_ref()
                            .is_some_and(|evidence| evidence.outcome == "suppressed")
                }
                "not_admitted" => {
                    visibility.admission_status != "admitted" && visibility.suppression.0.is_none()
                }
                _ => false,
            };
            if !coherent {
                return Err(format!(
                    "{} current visibility decision for {} is incoherent",
                    case.id, source.event.id
                ));
            }
        }

        let expected_historical = transition_page
            .transitions
            .iter()
            .filter_map(|transition| {
                let visible = transition.canonical_visible_event.0.as_ref()?;
                let final_visibility = case
                    .expected
                    .event_visibility
                    .iter()
                    .find(|visibility| visibility.event.event_id == visible.event.event_id)?;
                (final_visibility.decision != "visible").then_some((
                    transition.transition_seq,
                    visible.event.event_id.as_str(),
                    final_visibility.decision.as_str(),
                ))
            })
            .collect::<BTreeSet<_>>();
        let actual_historical = case
            .expected
            .historical_visibility_witnesses
            .iter()
            .map(|witness| {
                (
                    witness.transition_seq,
                    witness.event_id.as_str(),
                    witness.final_decision.as_str(),
                )
            })
            .collect::<BTreeSet<_>>();
        if actual_historical.len() != case.expected.historical_visibility_witnesses.len()
            || actual_historical != expected_historical
        {
            return Err(format!(
                "{} historical visibility witnesses do not cover every transition-time payload with a divergent final decision",
                case.id
            ));
        }
    }
    Ok(())
}

fn validate_expected_ingest(case_id: &str, observed: &ObservedEvent) -> Result<(), String> {
    let expected = &observed.expected_ingest;
    let (event_class, admitted_contract, required_decision, invalid_allowed) = match observed.role {
        ProjectionInputRole::ScopedFood => ("addressable", FOOD_CONTRACT_ID, None, true),
        ProjectionInputRole::ScopedNonFood => (
            "addressable",
            OPERATIONAL_LISTING_CONTRACT_ID,
            Some("applied"),
            false,
        ),
        ProjectionInputRole::UnrelatedAddressable => (
            "addressable",
            FARM_PROFILE_CONTRACT_ID,
            Some("applied"),
            false,
        ),
        ProjectionInputRole::Causal => (
            "regular",
            DELETION_CONTRACT_ID,
            Some("not_head_selected"),
            false,
        ),
    };
    let known_raw_head_decision = matches!(
        expected.raw_head_decision.as_str(),
        "applied"
            | "not_head_selected"
            | "skipped_older"
            | "skipped_same_timestamp_higher_event_id"
            | "malformed_coordinate"
    );
    let coherent_admission = match expected.admission_status.as_str() {
        "admitted" => {
            expected.admission_code.0.is_none()
                && expected.contract_id.0.as_deref() == Some(admitted_contract)
                && expected.valid_stream_eligible
        }
        "invalid" | "unsupported" => {
            invalid_allowed
                && expected.admission_code.0.is_some()
                && expected.contract_id.0.is_none()
                && !expected.valid_stream_eligible
        }
        _ => false,
    };
    if expected.event_class != event_class
        || !known_raw_head_decision
        || required_decision.is_some_and(|decision| expected.raw_head_decision != decision)
        || !coherent_admission
    {
        return Err(format!(
            "{case_id} input {} has an incoherent ingest witness",
            observed.event.id
        ));
    }
    Ok(())
}

fn validate_vector_event_reference<'a>(
    case_id: &str,
    label: &str,
    reference: &ExpectedEventReference,
    events: &BTreeMap<&'a str, (i64, &'a ObservedEvent)>,
) -> Result<&'a ObservedEvent, String> {
    validate_hex(
        &format!("{case_id} {label} event id"),
        &reference.event_id,
        64,
    )?;
    let (expected_sequence, observed) = events
        .get(reference.event_id.as_str())
        .copied()
        .ok_or_else(|| format!("{case_id} {label} must reference an input event"))?;
    if reference.event_seq != expected_sequence {
        return Err(format!(
            "{case_id} {label} sequence does not match input order"
        ));
    }
    Ok(observed)
}

fn validate_transition_decision_shape(
    case_id: &str,
    transition: &ExpectedTransition,
) -> Result<(), String> {
    let coherent = match transition.admission_status.as_str() {
        "admitted" => {
            transition.admission_code.0.is_none()
                && transition.contract_id.0.as_deref().is_some_and(|contract| {
                    contract == FOOD_CONTRACT_ID || contract == OPERATIONAL_LISTING_CONTRACT_ID
                })
                && transition.suppression.0.as_ref().is_some_and(|evidence| {
                    match transition.visibility.as_str() {
                        "visible" => {
                            evidence.outcome == "visible"
                                && transition.canonical_visible_event.0.is_some()
                        }
                        "suppressed" => {
                            evidence.outcome == "suppressed"
                                && transition.canonical_visible_event.0.is_none()
                        }
                        _ => false,
                    }
                })
        }
        "invalid" | "unsupported" => {
            transition.admission_code.0.is_some()
                && transition.contract_id.0.is_none()
                && transition.visibility == "not_admitted"
                && transition.suppression.0.is_none()
                && transition.canonical_visible_event.0.is_none()
        }
        _ => false,
    };
    if !coherent {
        return Err(format!(
            "{case_id} transition {} has an incoherent admission/visibility witness",
            transition.transition_seq
        ));
    }
    Ok(())
}

fn validate_suppression_evidence(
    case_id: &str,
    evidence: &ExpectedSuppressionEvidence,
    events: &BTreeMap<&str, (i64, &ObservedEvent)>,
    target_kind: u32,
    target_created_at: u64,
) -> Result<(), String> {
    for (label, request_id) in [
        (
            "event-reference request",
            evidence.event_reference_request_id.0.as_deref(),
        ),
        (
            "address-reference request",
            evidence.address_reference_request_id.0.as_deref(),
        ),
    ] {
        if let Some(request_id) = request_id {
            validate_hex(&format!("{case_id} {label}"), request_id, 64)?;
            if events
                .get(request_id)
                .is_none_or(|(_, event)| event.role != ProjectionInputRole::Causal)
            {
                return Err(format!(
                    "{case_id} {label} must reference a causal input event"
                ));
            }
        }
    }
    let event_reference = evidence.event_reference_request_id.0.is_some();
    let address_reference = evidence.address_reference_request_id.0.is_some();
    let cutoff = evidence.address_reference_cutoff.0;
    let coherent = match evidence.reason.as_str() {
        "deletion_request_immune" => {
            target_kind == 5
                && evidence.outcome == "visible"
                && !event_reference
                && !address_reference
                && cutoff.is_none()
        }
        "deletion_no_authorized_reference" | "deletion_request_author_mismatch" => {
            target_kind != 5
                && evidence.outcome == "visible"
                && !event_reference
                && !address_reference
                && cutoff.is_none()
        }
        "deletion_address_cutoff_precedes_target" => {
            target_kind != 5
                && evidence.outcome == "visible"
                && !event_reference
                && address_reference
                && cutoff.is_some_and(|cutoff| cutoff < target_created_at)
        }
        "deletion_event_id_reference" => {
            target_kind != 5
                && evidence.outcome == "suppressed"
                && event_reference
                && cutoff.is_none_or(|cutoff| cutoff < target_created_at)
                && (address_reference == cutoff.is_some())
        }
        "deletion_address_reference" => {
            target_kind != 5
                && evidence.outcome == "suppressed"
                && !event_reference
                && address_reference
                && cutoff.is_some_and(|cutoff| cutoff >= target_created_at)
        }
        "deletion_event_id_and_address_reference" => {
            target_kind != 5
                && evidence.outcome == "suppressed"
                && event_reference
                && address_reference
                && cutoff.is_some_and(|cutoff| cutoff >= target_created_at)
        }
        _ => false,
    };
    if !coherent {
        return Err(format!("{case_id} suppression evidence is incoherent"));
    }
    Ok(())
}

fn validate_manifest_shape(manifest: &FoodAvailabilityProjectionManifest) -> Result<(), String> {
    let expected_public_api = PUBLIC_API
        .iter()
        .map(|name| (*name).to_owned())
        .collect::<Vec<_>>();
    if manifest.schema_version != SCHEMA_VERSION
        || manifest.contract_id != CONTRACT_ID
        || manifest.hook_id != HOOK_ID
        || manifest.predecessor.hook_id != PREDECESSOR_HOOK_ID
        || manifest.predecessor.manifest.sha256 != PREDECESSOR_MANIFEST_SHA256
        || manifest.migration.version != MIGRATION_VERSION
        || manifest.migration.name != MIGRATION_NAME
        || manifest.migration.schema_sha256 != SCHEMA_SHA256
        || manifest.profile.event_contract_registry_version != EVENT_CONTRACT_REGISTRY_VERSION
        || manifest.profile.addressable_feed_version != ADDRESSABLE_FEED_VERSION
        || manifest.profile.projection_version != PROJECTION_VERSION
        || manifest.profile.scope_kinds != [FOOD_AVAILABILITY_KIND]
        || manifest.profile.scope_fingerprint_sha256 != SCOPE_FINGERPRINT_SHA256
        || manifest.profile.food_contract_id != FOOD_CONTRACT_ID
        || manifest.profile.admission_authority != ADMISSION_AUTHORITY
        || manifest.profile.current_visibility_authority != CURRENT_VISIBILITY_AUTHORITY
        || manifest.profile.post_core_capability != POST_CORE_CAPABILITY
        || manifest.public_api != expected_public_api
        || manifest.result_vector.executor_id != RESULT_VECTOR_EXECUTOR_ID
    {
        return Err(format!(
            "{MANIFEST_RELATIVE} does not describe the FoodAvailability projection-v1 successor"
        ));
    }
    validate_catalog(&manifest.migration.catalog)?;
    validate_unique(
        "source role",
        manifest
            .source_files
            .iter()
            .map(|source| source.role.as_str()),
    )?;
    validate_unique(
        "source path",
        manifest
            .source_files
            .iter()
            .map(|source| source.path.as_str()),
    )?;
    validate_unique(
        "entry-point role",
        manifest
            .entry_points
            .iter()
            .map(|entry| entry.role.as_str()),
    )?;
    validate_unique("public API", manifest.public_api.iter().map(String::as_str))?;
    Ok(())
}

fn generated_descriptor(
    manifest: &FoodAvailabilityProjectionManifest,
    manifest_bytes: &[u8],
    manifest_sha256: &str,
) -> String {
    let manifest_json = std::str::from_utf8(manifest_bytes)
        .expect("canonical JSON serialization always produces UTF-8");
    let manifest_literal = format!("{manifest_json:?}");
    format!(
        "// @generated by `cargo xtask contract food-availability-projection-manifest --write`; do not edit.\n\
#![allow(dead_code)]\n\n\
pub(crate) const FOOD_AVAILABILITY_PROJECTION_MANIFEST_JSON: &str = {manifest_literal};\n\
pub(crate) const FOOD_AVAILABILITY_PROJECTION_MANIFEST_BYTE_LENGTH: usize = {};\n\
pub(crate) const FOOD_AVAILABILITY_PROJECTION_MANIFEST_SHA256: &str =\n    \"{manifest_sha256}\";\n\
pub(crate) const FOOD_AVAILABILITY_PROJECTION_MANIFEST_SCHEMA_VERSION: u32 = {};\n\
pub(crate) const FOOD_AVAILABILITY_PROJECTION_CONTRACT_ID: &str =\n    \"{}\";\n\
pub(crate) const FOOD_AVAILABILITY_PROJECTION_HOOK_ID: &str = \"{}\";\n\
pub(crate) const FOOD_AVAILABILITY_PROJECTION_MIGRATION_VERSION: u32 = {};\n\
pub(crate) const FOOD_AVAILABILITY_PROJECTION_MIGRATION_NAME: &str = \"{}\";\n\
pub(crate) const FOOD_AVAILABILITY_PROJECTION_VERSION: u32 = {};\n\
pub(crate) const FOOD_AVAILABILITY_PROJECTION_FEED_VERSION: u32 = {};\n\
pub(crate) const FOOD_AVAILABILITY_PROJECTION_EVENT_CONTRACT_REGISTRY_VERSION: u32 = {};\n\
pub(crate) const FOOD_AVAILABILITY_PROJECTION_MIGRATION_UP_BYTE_LENGTH: usize = {};\n\
pub(crate) const FOOD_AVAILABILITY_PROJECTION_MIGRATION_UP_SHA256: &str =\n    \"{}\";\n\
pub(crate) const FOOD_AVAILABILITY_PROJECTION_MIGRATION_DOWN_BYTE_LENGTH: usize = {};\n\
pub(crate) const FOOD_AVAILABILITY_PROJECTION_MIGRATION_DOWN_SHA256: &str =\n    \"{}\";\n\
pub(crate) const FOOD_AVAILABILITY_PROJECTION_SCHEMA_SHA256: &str =\n    \"{}\";\n\
pub(crate) const FOOD_AVAILABILITY_PROJECTION_SCOPE_KINDS: &[u32] = &[30_402];\n\
pub(crate) const FOOD_AVAILABILITY_PROJECTION_SCOPE_FINGERPRINT_SHA256: &str =\n    \"{}\";\n\
pub(crate) const FOOD_AVAILABILITY_PROJECTION_PREDECESSOR_MANIFEST_SHA256: &str =\n    \"{}\";\n\
pub(crate) const FOOD_AVAILABILITY_PROJECTION_RESULT_VECTOR_SHA256: &str =\n    \"{}\";\n\
pub(crate) const FOOD_AVAILABILITY_PROJECTION_RESULT_VECTOR_EXECUTOR_ID: &str =\n    \"{}\";\n\
pub(crate) const FOOD_AVAILABILITY_PROJECTION_RESULT_VECTOR_EXECUTOR_SHA256: &str =\n    \"{}\";\n",
        manifest_bytes.len(),
        manifest.schema_version,
        manifest.contract_id,
        manifest.hook_id,
        manifest.migration.version,
        manifest.migration.name,
        manifest.profile.projection_version,
        manifest.profile.addressable_feed_version,
        manifest.profile.event_contract_registry_version,
        manifest.migration.up.byte_length,
        manifest.migration.up.sha256,
        manifest.migration.down.byte_length,
        manifest.migration.down.sha256,
        manifest.migration.schema_sha256,
        manifest.profile.scope_fingerprint_sha256,
        manifest.predecessor.manifest.sha256,
        manifest.result_vector.sha256,
        manifest.result_vector.executor_id,
        manifest.result_vector.executor_sha256,
    )
}

fn manifest_schema() -> Value {
    let hash = json!({"type": "string", "pattern": "^[0-9a-f]{64}$"});
    let path = json!({
        "type": "string",
        "pattern": "^[A-Za-z0-9_-][A-Za-z0-9._-]*(?:/[A-Za-z0-9_-][A-Za-z0-9._-]*)*$"
    });
    let file = json!({
        "type": "object",
        "required": ["path", "byte_length", "sha256", "hash_algorithm"],
        "properties": {
            "path": path,
            "byte_length": {"type": "integer", "minimum": 1},
            "sha256": hash,
            "hash_algorithm": {"const": HASH_ALGORITHM}
        },
        "additionalProperties": false
    });
    let string_array = json!({
        "type": "array",
        "items": {"type": "string", "minLength": 1},
        "uniqueItems": true
    });
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://radroots.org/core/event-store/food-availability-projection-manifest-v1.schema.json",
        "title": "Radroots event-store FoodAvailability projection manifest v1",
        "type": "object",
        "required": [
            "schema_version", "contract_id", "hook_id", "manifest_schema", "predecessor",
            "migration", "profile", "registry_inventory", "food_profile_vector",
            "entry_points", "source_files", "public_api", "result_vector"
        ],
        "properties": {
            "schema_version": {"const": SCHEMA_VERSION},
            "contract_id": {"const": CONTRACT_ID},
            "hook_id": {"const": HOOK_ID},
            "manifest_schema": {"$ref": "#/$defs/file"},
            "predecessor": {
                "type": "object",
                "required": ["hook_id", "manifest"],
                "properties": {
                    "hook_id": {"const": PREDECESSOR_HOOK_ID},
                    "manifest": {"$ref": "#/$defs/file"}
                },
                "additionalProperties": false
            },
            "migration": {
                "type": "object",
                "required": ["version", "name", "up", "down", "schema_sha256", "catalog"],
                "properties": {
                    "version": {"const": MIGRATION_VERSION},
                    "name": {"const": MIGRATION_NAME},
                    "up": {"$ref": "#/$defs/file"},
                    "down": {"$ref": "#/$defs/file"},
                    "schema_sha256": {"const": SCHEMA_SHA256},
                    "catalog": {
                        "type": "object",
                        "required": ["objects", "tables", "fts5_tables"],
                        "properties": {
                            "objects": string_array,
                            "tables": string_array,
                            "fts5_tables": string_array
                        },
                        "additionalProperties": false
                    }
                },
                "additionalProperties": false
            },
            "profile": {
                "type": "object",
                "required": [
                    "event_contract_registry_version", "addressable_feed_version",
                    "projection_version", "scope_kinds", "scope_fingerprint_sha256",
                    "food_contract_id", "admission_authority", "current_visibility_authority",
                    "post_core_capability"
                ],
                "properties": {
                    "event_contract_registry_version": {"const": EVENT_CONTRACT_REGISTRY_VERSION},
                    "addressable_feed_version": {"const": ADDRESSABLE_FEED_VERSION},
                    "projection_version": {"const": PROJECTION_VERSION},
                    "scope_kinds": {
                        "type": "array", "prefixItems": [{"const": FOOD_AVAILABILITY_KIND}],
                        "minItems": 1, "maxItems": 1
                    },
                    "scope_fingerprint_sha256": {"const": SCOPE_FINGERPRINT_SHA256},
                    "food_contract_id": {"const": FOOD_CONTRACT_ID},
                    "admission_authority": {"const": ADMISSION_AUTHORITY},
                    "current_visibility_authority": {"const": CURRENT_VISIBILITY_AUTHORITY},
                    "post_core_capability": {"const": POST_CORE_CAPABILITY}
                },
                "additionalProperties": false
            },
            "registry_inventory": {"$ref": "#/$defs/file"},
            "food_profile_vector": {"$ref": "#/$defs/file"},
            "entry_points": {
                "type": "array", "minItems": 1,
                "items": {
                    "type": "object", "required": ["role", "rust_path"],
                    "properties": {
                        "role": {"type": "string", "minLength": 1},
                        "rust_path": {"type": "string", "minLength": 1}
                    },
                    "additionalProperties": false
                }
            },
            "source_files": {
                "type": "array", "minItems": 1,
                "items": {"$ref": "#/$defs/source_file"}
            },
            "public_api": string_array,
            "result_vector": {
                "type": "object",
                "required": [
                    "canonical_path", "mirror_path", "byte_length", "sha256", "hash_algorithm",
                    "executor_id", "executor_path", "executor_test", "executor_byte_length",
                    "executor_sha256", "executor_hash_algorithm"
                ],
                "properties": {
                    "canonical_path": {"const": RESULT_VECTOR_CANONICAL_RELATIVE},
                    "mirror_path": {"const": RESULT_VECTOR_MIRROR_RELATIVE},
                    "byte_length": {"type": "integer", "minimum": 1},
                    "sha256": {"type": "string", "pattern": "^[0-9a-f]{64}$"},
                    "hash_algorithm": {"const": HASH_ALGORITHM},
                    "executor_id": {"const": RESULT_VECTOR_EXECUTOR_ID},
                    "executor_path": {"const": RESULT_VECTOR_EXECUTOR_RELATIVE},
                    "executor_test": {"const": RESULT_VECTOR_EXECUTOR_TEST},
                    "executor_byte_length": {"type": "integer", "minimum": 1},
                    "executor_sha256": {"type": "string", "pattern": "^[0-9a-f]{64}$"},
                    "executor_hash_algorithm": {"const": HASH_ALGORITHM}
                },
                "additionalProperties": false
            }
        },
        "$defs": {
            "file": file,
            "source_file": {
                "type": "object",
                "required": ["role", "path", "byte_length", "sha256", "hash_algorithm"],
                "properties": {
                    "role": {"type": "string", "minLength": 1},
                    "path": {
                        "type": "string",
                        "pattern": "^[A-Za-z0-9_-][A-Za-z0-9._-]*(?:/[A-Za-z0-9_-][A-Za-z0-9._-]*)*$"
                    },
                    "byte_length": {"type": "integer", "minimum": 1},
                    "sha256": {"type": "string", "pattern": "^[0-9a-f]{64}$"},
                    "hash_algorithm": {"const": HASH_ALGORITHM}
                },
                "additionalProperties": false
            }
        },
        "additionalProperties": false
    })
}

fn validate_manifest_json_schema(schema: &Value, manifest: &Value) -> Result<(), String> {
    jsonschema::draft202012::meta::validate(schema).map_err(|error| {
        format!(
            "{MANIFEST_SCHEMA_RELATIVE} is not a valid JSON Schema Draft 2020-12 document: {error}"
        )
    })?;
    let validator = jsonschema::draft202012::options()
        .build(schema)
        .map_err(|error| {
            format!("compile {MANIFEST_SCHEMA_RELATIVE} as JSON Schema Draft 2020-12: {error}")
        })?;
    validator.validate(manifest).map_err(|error| {
        format!(
            "{MANIFEST_RELATIVE} violates {MANIFEST_SCHEMA_RELATIVE} at {}: {error}",
            error.instance_path()
        )
    })
}

fn canonical_json_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, String> {
    let mut bytes =
        serde_json::to_vec_pretty(value).map_err(|error| format!("serialize JSON: {error}"))?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn validate_canonical_json<T: Serialize>(
    relative: &str,
    actual: &[u8],
    value: &T,
) -> Result<(), String> {
    if actual.starts_with(&[0xef, 0xbb, 0xbf]) {
        return Err(format!("{relative} must not contain a UTF-8 BOM"));
    }
    if actual.contains(&b'\r') {
        return Err(format!("{relative} must use LF line endings"));
    }
    if actual != canonical_json_bytes(value)? {
        return Err(format!(
            "{relative} must use canonical two-space JSON formatting and end with exactly one LF"
        ));
    }
    Ok(())
}

fn validate_unique<'a>(label: &str, values: impl Iterator<Item = &'a str>) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for value in values {
        if value.is_empty() || !seen.insert(value) {
            return Err(format!("empty or duplicate {label}: {value}"));
        }
    }
    Ok(())
}

fn validate_digest_sidecar(relative: &str, bytes: &[u8]) -> Result<(), String> {
    if bytes.len() != 65 || bytes[64] != b'\n' {
        return Err(format!(
            "{relative} must contain 64 lowercase hexadecimal bytes and one LF"
        ));
    }
    let digest = std::str::from_utf8(&bytes[..64])
        .map_err(|error| format!("{relative} must be UTF-8: {error}"))?;
    validate_sha256(relative, digest)
}

fn validate_sha256(label: &str, value: &str) -> Result<(), String> {
    validate_hex(label, value, 64)
}

fn validate_hex(label: &str, value: &str, expected_length: usize) -> Result<(), String> {
    if value.len() != expected_length
        || !value
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(format!(
            "{label} must contain exactly {expected_length} lowercase hexadecimal bytes"
        ));
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn stale_error(relative: &str) -> String {
    format!("{relative} is stale; run `{WRITE_COMMAND}`")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn repository_root() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("xtask manifest has a workspace root")
            .to_path_buf()
    }

    fn immutable_manifest() -> FoodAvailabilityProjectionManifest {
        serde_json::from_slice(IMMUTABLE_MANIFEST_BYTES)
            .expect("immutable FoodAvailability manifest")
    }

    fn copy_file(source_root: &Path, destination_root: &Path, relative: &str) {
        let destination = destination_root.join(relative);
        fs::create_dir_all(destination.parent().expect("fixture parent"))
            .expect("create fixture parent");
        fs::copy(source_root.join(relative), destination).expect("copy fixture");
    }

    fn immutable_artifact_workspace() -> tempfile::TempDir {
        const NIP09_ARTIFACTS: &[&str] = &[
            "crates/event_store/contracts/nip09_reconciliation_v1.manifest.json",
            "crates/event_store/contracts/nip09_reconciliation_v1.manifest.schema.json",
            "crates/event_store/contracts/nip09_reconciliation_v1.manifest.sha256",
            "crates/event_store/src/generated/nip09_reconciliation_manifest.rs",
            "contracts/conformance/vectors/event_store/nip09_reconciliation.v1.json",
            "crates/event_store/tests/fixtures/nip09_reconciliation.v1.json",
            "crates/event_store/src/nip09/reconciliation_v1/result_vector_executor.rs",
            "crates/event_store/migrations/0001_event_store.up.sql",
            "crates/event_store/migrations/0001_event_store.down.sql",
            "crates/event_store/migrations/0002_nip09.up.sql",
            "crates/event_store/migrations/0002_nip09.down.sql",
        ];

        let workspace = tempfile::TempDir::new().expect("workspace");
        let repository = repository_root();
        for relative in NIP09_ARTIFACTS.iter().copied().chain(
            IMMUTABLE_PREDECESSOR_ARTIFACTS
                .iter()
                .map(|artifact| artifact.relative),
        ) {
            copy_file(&repository, workspace.path(), relative);
        }
        workspace
    }

    #[test]
    fn immutable_food_predecessor_artifacts_match_authenticated_identities() {
        let root = repository_root();
        for artifact in IMMUTABLE_PREDECESSOR_ARTIFACTS {
            let bytes = read_regular_file(&root, artifact.relative).expect("immutable artifact");
            assert_eq!(bytes.len(), artifact.byte_length, "{}", artifact.relative);
            assert_eq!(sha256_hex(&bytes), artifact.sha256, "{}", artifact.relative);
        }
    }

    #[test]
    fn legacy_writer_restores_only_generated_immutable_artifacts() {
        let workspace = immutable_artifact_workspace();
        fs::write(workspace.path().join(MANIFEST_RELATIVE), b"tampered\n")
            .expect("tamper manifest");
        fs::write(
            workspace.path().join(RESULT_VECTOR_MIRROR_RELATIVE),
            b"tampered\n",
        )
        .expect("tamper result-vector mirror");
        let changed_source = workspace.path().join("crates/event_store/src/error.rs");
        fs::create_dir_all(changed_source.parent().expect("source parent"))
            .expect("create source parent");
        fs::write(&changed_source, b"successor-owned source bytes\n")
            .expect("write changed source");

        write_food_availability_projection_manifest(workspace.path())
            .expect("restore immutable generated artifacts");
        assert_eq!(
            fs::read(workspace.path().join(MANIFEST_RELATIVE)).expect("restored manifest"),
            IMMUTABLE_MANIFEST_BYTES
        );
        assert_eq!(
            fs::read(workspace.path().join(RESULT_VECTOR_MIRROR_RELATIVE))
                .expect("restored result-vector mirror"),
            IMMUTABLE_RESULT_VECTOR_BYTES
        );
        assert_eq!(
            fs::read(changed_source).expect("changed source"),
            b"successor-owned source bytes\n"
        );
    }

    #[test]
    fn legacy_writer_cannot_rebaseline_an_authored_immutable_artifact() {
        let workspace = immutable_artifact_workspace();
        fs::write(
            workspace.path().join(RESULT_VECTOR_EXECUTOR_RELATIVE),
            b"tampered\n",
        )
        .expect("tamper executor");
        let error = write_food_availability_projection_manifest(workspace.path())
            .expect_err("immutable executor drift must fail");
        assert!(
            error.contains("immutable FoodAvailability predecessor artifact")
                && error.contains(RESULT_VECTOR_EXECUTOR_RELATIVE),
            "{error}"
        );
    }

    #[test]
    fn predecessor_source_inventory_rejects_duplicate_unknown_and_unsuperseded_drift() {
        let manifest = immutable_manifest();
        let descriptors = manifest.source_files.clone();
        let describe = |spec: SourceSpec| {
            descriptors
                .iter()
                .find(|source| source.path == spec.path)
                .cloned()
                .ok_or_else(|| format!("missing fixture source {}", spec.path))
        };
        validate_food_predecessor_source_inventory(&manifest, &[], describe)
            .expect("complete immutable predecessor inventory");

        let path = SOURCE_SPECS[0].path;
        let error = validate_food_predecessor_source_inventory(&manifest, &[path, path], |_| {
            unreachable!("duplicates fail before source reads")
        })
        .expect_err("duplicate supersession must fail");
        assert!(error.contains("must be unique"), "{error}");

        let error = validate_food_predecessor_source_inventory(
            &manifest,
            &["crates/event_store/src/not_bound.rs"],
            |_| unreachable!("unknown paths fail before source reads"),
        )
        .expect_err("unknown supersession must fail");
        assert!(
            error.contains("not a FoodAvailability predecessor-bound"),
            "{error}"
        );

        let drift_path = SOURCE_SPECS[0].path;
        let descriptors = manifest.source_files.clone();
        let error = validate_food_predecessor_source_inventory(&manifest, &[], |spec| {
            let mut source = descriptors
                .iter()
                .find(|source| source.path == spec.path)
                .cloned()
                .expect("fixture source");
            if spec.path == drift_path {
                source.sha256 = "00".repeat(32);
            }
            Ok(source)
        })
        .expect_err("unsuperseded source drift must fail");
        assert!(
            error.contains("unchanged FoodAvailability predecessor source authority")
                && error.contains(drift_path),
            "{error}"
        );

        let descriptors = manifest.source_files.clone();
        validate_food_predecessor_source_inventory(&manifest, &[drift_path], |spec| {
            assert_ne!(spec.path, drift_path, "superseded source must not be read");
            descriptors
                .iter()
                .find(|source| source.path == spec.path)
                .cloned()
                .ok_or_else(|| format!("missing fixture source {}", spec.path))
        })
        .expect("an explicitly superseded source is delegated to the successor");
    }

    #[test]
    fn downstream_nip09_only_supersession_is_transitively_validated() {
        let source_maintenance_superseded_paths =
            super::super::source_maintenance::predecessor_superseded_source_paths();

        let root = repository_root();
        let (food_paths, nip09_paths) = partition_downstream_predecessor_supersessions(
            &root,
            &[
                "crates/event_store/src/error.rs",
                "crates/event_store/src/store/protocol_reconciliation_v1.rs",
            ],
        )
        .expect("overlapping and NIP-09-only predecessor ownership");
        assert_eq!(food_paths, ["crates/event_store/src/error.rs"]);
        assert_eq!(
            nip09_paths,
            [
                "crates/event_store/src/error.rs",
                "crates/event_store/src/store/protocol_reconciliation_v1.rs",
            ]
        );
        validate_food_availability_projection_predecessor_production_sources_under_lock(
            &root,
            source_maintenance_superseded_paths,
        )
        .expect("Food and transitive NIP-09 successor source coverage");

        let mut duplicate = source_maintenance_superseded_paths.to_vec();
        duplicate.push("crates/event_store/src/store/protocol_reconciliation_v1.rs");
        let error =
            validate_food_availability_projection_predecessor_production_sources_under_lock(
                &root, &duplicate,
            )
            .expect_err("duplicate transitive supersession must fail");
        assert!(error.contains("must be unique"), "{error}");

        let mut unknown = source_maintenance_superseded_paths.to_vec();
        unknown.push("crates/event_store/src/store/not_predecessor_bound.rs");
        let error =
            validate_food_availability_projection_predecessor_production_sources_under_lock(
                &root, &unknown,
            )
            .expect_err("unknown transitive supersession must fail");
        assert!(
            error.contains("not bound by either the FoodAvailability or NIP-09 predecessor"),
            "{error}"
        );
    }

    #[test]
    fn food_scope_fingerprint_is_pinned() {
        let mut hasher = Sha256::new();
        hasher.update(b"radroots.addressable-transition-scope.v1\0");
        hasher.update(1_u32.to_be_bytes());
        hasher.update(FOOD_AVAILABILITY_KIND.to_be_bytes());
        assert_eq!(hex::encode(hasher.finalize()), SCOPE_FINGERPRINT_SHA256);
    }

    #[test]
    fn blossom_dependency_authority_requires_the_direct_std_only_edge() {
        let root = repository_root();
        let workspace = parse_toml_value(&root, "Cargo.toml").expect("workspace manifest");
        let mut event_store =
            parse_toml_value(&root, "crates/event_store/Cargo.toml").expect("event-store manifest");
        validate_blossom_dependency_values(&workspace, &event_store)
            .expect("governed Blossom dependency");

        event_store
            .get_mut("dependencies")
            .and_then(toml::Value::as_table_mut)
            .and_then(|dependencies| dependencies.get_mut("radroots_blossom"))
            .and_then(toml::Value::as_table_mut)
            .expect("Blossom dependency table")
            .insert("features".to_owned(), toml::Value::Array(Vec::new()));
        let error = validate_blossom_dependency_values(&workspace, &event_store)
            .expect_err("missing std feature must fail");
        assert!(error.contains("governed std feature"), "{error}");
    }

    #[test]
    fn migration_cursor_guards_are_bound_to_governed_rust_limits() {
        let root = repository_root();
        let addressable_model = read_regular_file(
            &root,
            "crates/event_store/src/model/addressable_transition_feed_v1.rs",
        )
        .expect("addressable model");
        let addressable_model = std::str::from_utf8(&addressable_model).expect("UTF-8 model");
        let food_model = read_regular_file(
            &root,
            "crates/event_store/src/model/food_availability_projection_v1.rs",
        )
        .expect("food model");
        let food_model = std::str::from_utf8(&food_model).expect("UTF-8 model");
        let migration =
            read_regular_file(&root, MIGRATION_UP_RELATIVE).expect("projection migration");
        let migration = std::str::from_utf8(&migration).expect("UTF-8 migration");
        validate_migration_guard_limit_sources(addressable_model, food_model, migration)
            .expect("governed cursor guards");

        let stale_scan = migration.replacen(
            "NEW.last_transition_seq - OLD.last_transition_seq > 1024",
            "NEW.last_transition_seq - OLD.last_transition_seq > 1023",
            1,
        );
        assert_ne!(stale_scan, migration, "scan mutation must apply");
        let error =
            validate_migration_guard_limit_sources(addressable_model, food_model, &stale_scan)
                .expect_err("stale scan guard must fail");
        assert!(error.contains("cursor scan delta"), "{error}");

        let stale_rows = migration.replacen(
            "abs(NEW.projected_row_count - OLD.projected_row_count) > 64",
            "abs(NEW.projected_row_count - OLD.projected_row_count) > 63",
            1,
        );
        assert_ne!(stale_rows, migration, "row-count mutation must apply");
        let error =
            validate_migration_guard_limit_sources(addressable_model, food_model, &stale_rows)
                .expect_err("stale row-count guard must fail");
        assert!(error.contains("projected-row delta"), "{error}");

        let detached_apply_limit = food_model.replacen(
            "RADROOTS_ADDRESSABLE_TRANSITION_PAGE_LIMIT_MAX_V1;",
            "63;",
            1,
        );
        assert_ne!(
            detached_apply_limit, food_model,
            "apply-limit mutation must apply"
        );
        let error = validate_migration_guard_limit_sources(
            addressable_model,
            &detached_apply_limit,
            migration,
        )
        .expect_err("detached apply limit must fail");
        assert!(error.contains("must alias"), "{error}");

        let stale_visibility_index = migration.replacen(
            "INDEXED BY radroots_event_store_nip09_address_target_visibility_lookup_idx",
            "INDEXED BY radroots_event_store_nip09_address_target_lookup_idx",
            1,
        );
        assert_ne!(
            stale_visibility_index, migration,
            "visibility-index mutation must apply"
        );
        let error = validate_migration_guard_limit_sources(
            addressable_model,
            food_model,
            &stale_visibility_index,
        )
        .expect_err("unforced visibility index must fail");
        assert!(error.contains("visibility index"), "{error}");

        let stale_visibility_order = migration.replacen(
            "ORDER BY target.inclusive_cutoff DESC, target.request_event_id",
            "ORDER BY target.inclusive_cutoff DESC, request.request_event_id",
            1,
        );
        assert_ne!(
            stale_visibility_order, migration,
            "visibility-order mutation must apply"
        );
        let error = validate_migration_guard_limit_sources(
            addressable_model,
            food_model,
            &stale_visibility_order,
        )
        .expect_err("non-indexed visibility ordering must fail");
        assert!(error.contains("visibility ordering"), "{error}");
    }

    #[test]
    fn food_read_queries_require_the_exact_persisted_head_authority_join() {
        let root = repository_root();
        let source = read_regular_file(
            &root,
            "crates/event_store/src/store/food_availability_projection_v1.rs",
        )
        .expect("Food projection store source");
        let source = std::str::from_utf8(&source).expect("UTF-8 Food projection store source");
        validate_food_read_query_sources(source).expect("governed Food read queries");

        for (predicate, replacement) in [
            ("source.singleton = 1 AND ", ""),
            (
                "source.active_generation = projection.source_generation",
                "source.active_generation != projection.source_generation",
            ),
            ("cursor.singleton = 1", "cursor.singleton = 2"),
            (
                "cursor.source_generation = projection.source_generation",
                "cursor.source_generation != projection.source_generation",
            ),
            (
                "head.source_generation = projection.source_generation",
                "head.source_generation = source.active_generation",
            ),
            ("head.kind = 30402", "head.kind = 30403"),
            (
                "head.pubkey = projection.pubkey",
                "head.pubkey != projection.pubkey",
            ),
            (
                "head.d_tag = projection.d_tag",
                "head.d_tag != projection.d_tag",
            ),
            (
                "head.raw_head_event_id = projection.event_id",
                "head.raw_head_event_id != projection.event_id",
            ),
            (
                "head.raw_head_event_seq = projection.event_seq",
                "head.raw_head_event_seq != projection.event_seq",
            ),
            (
                "head.raw_head_created_at = projection.created_at",
                "head.raw_head_created_at != projection.created_at",
            ),
            (
                "head.admission_status = 'admitted'",
                "head.admission_status != 'admitted'",
            ),
            (
                "head.admission_code IS NULL",
                "head.admission_code IS NOT NULL",
            ),
            (
                "head.contract_id = projection.contract_id",
                "head.contract_id != projection.contract_id",
            ),
            (
                "head.visibility = 'visible'",
                "head.visibility != 'visible'",
            ),
            (
                "head.nip09_outcome = 'visible'",
                "head.nip09_outcome != 'visible'",
            ),
        ] {
            let mutated = source.replacen(predicate, replacement, 1);
            assert_ne!(
                mutated, source,
                "predicate mutation must apply: {predicate}"
            );
            let error = validate_food_read_query_sources(&mutated)
                .expect_err("weakened head-authority predicate must fail");
            assert!(error.contains("fail-closed"), "{predicate}: {error}");
        }

        let unfenced_recent = source.replacen(
            "FROM radroots_event_store_source_state AS source CROSS JOIN radroots_event_store_food_availability_read_v1 AS projection ON",
            "FROM radroots_event_store_food_availability_read_v1 AS projection JOIN radroots_event_store_source_state AS source ON",
            1,
        );
        assert_ne!(
            unfenced_recent, source,
            "recent source-first fence mutation must apply"
        );
        let error = validate_food_read_query_sources(&unfenced_recent)
            .expect_err("unfenced recent query must fail");
        assert!(error.contains("fail-closed"), "{error}");

        let rerouted = source.replacen(
            "sqlx::query(FOOD_AVAILABILITY_POINT_QUERY_V1)",
            "sqlx::query(FOOD_AVAILABILITY_RECENT_QUERY_V1)",
            1,
        );
        assert_ne!(rerouted, source, "query-route mutation must apply");
        let error = validate_food_read_query_sources(&rerouted)
            .expect_err("wrong governed query route must fail");
        assert!(error.contains("exact governed query"), "{error}");

        let dynamic = source.replacen(
            "sqlx::query(FOOD_AVAILABILITY_POINT_QUERY_V1)",
            "sqlx::query(query_text)",
            1,
        );
        assert_ne!(dynamic, source, "dynamic-route mutation must apply");
        let error =
            validate_food_read_query_sources(&dynamic).expect_err("arbitrary query path must fail");
        assert!(error.contains("exact governed query"), "{error}");
    }

    #[test]
    fn food_projection_audit_authority_is_exact_and_fail_closed() {
        let root = repository_root();
        let source = read_regular_file(
            &root,
            "crates/event_store/src/store/food_availability_projection_v1.rs",
        )
        .expect("Food projection store source");
        let source = std::str::from_utf8(&source).expect("UTF-8 Food projection store source");
        validate_food_projection_audit_authority(source)
            .expect("governed exhaustive Food projection audit");

        let route_mutations = [
            (
                "audit transaction downgrade",
                source.replacen(
                    "self.begin_write_transaction().await?",
                    "self.pool.begin().await?",
                    1,
                ),
            ),
            (
                "exhaustive audit visibility weakening",
                source.replacen(
                    "pub(crate) async fn validate_food_availability_projection_hook_v1(",
                    "pub async fn validate_food_availability_projection_hook_v1(",
                    1,
                ),
            ),
            (
                "exhaustive audit early success",
                source.replacen(
                    "pub(crate) async fn validate_food_availability_projection_hook_v1(\n    connection: &mut SqliteConnection,\n) -> Result<(), RadrootsEventStoreError> {\n    let state =",
                    "pub(crate) async fn validate_food_availability_projection_hook_v1(\n    connection: &mut SqliteConnection,\n) -> Result<(), RadrootsEventStoreError> {\n    return Ok(());\n    let state =",
                    1,
                ),
            ),
            (
                "source-transition validation omission",
                source.replacen(
                    "        validate_projection_source_transition(connection, &projection).await?;\n",
                    "",
                    1,
                ),
            ),
            (
                "source-transition validation reorder",
                source.replacen(
                    "        validate_projection_source_transition(connection, &projection).await?;\n        validate_fts_row(connection, &projection).await?;",
                    "        validate_fts_row(connection, &projection).await?;\n        validate_projection_source_transition(connection, &projection).await?;",
                    1,
                ),
            ),
            (
                "source-transition sequence rebind",
                {
                    let (prefix, suffix) = source
                        .rsplit_once(".bind(projection.source_transition_seq())")
                        .expect("source-transition helper bind");
                    format!("{prefix}.bind(projection.event_seq()){suffix}")
                },
            ),
            (
                "coordinate cardinality bypass",
                source.replacen(
                    "    let actual_row_count = i64::try_from(actual_coordinates.len())\n        .map_err(|_| projection_drift(\"projection row count exceeds i64\"))?;",
                    "    let actual_row_count = state.projected_row_count;",
                    1,
                ),
            ),
            (
                "sealed row-count comparison inversion",
                source.replacen(
                    "if actual_row_count != state.projected_row_count",
                    "if actual_row_count == state.projected_row_count",
                    1,
                ),
            ),
            (
                "coordinate equality omission",
                source.replacen(
                    "    if actual_coordinates != expected_coordinates {\n        return Err(projection_drift(\n            \"projection coordinate witnesses do not equal the current admitted, visible FoodAvailability heads\",\n        ));\n    }\n",
                    "",
                    1,
                ),
            ),
            (
                "coordinate overwrite before equality",
                source.replacen(
                    "    if actual_coordinates != expected_coordinates {",
                    "    actual_coordinates = expected_coordinates.clone();\n    if actual_coordinates != expected_coordinates {",
                    1,
                ),
            ),
        ];
        for (label, mutation) in route_mutations {
            assert_ne!(mutation, source, "audit-route mutation must apply: {label}");
            let error = match validate_food_projection_audit_authority(&mutation) {
                Ok(()) => panic!("audit-route mutation must fail: {label}"),
                Err(error) => error,
            };
            assert!(!error.is_empty());
        }

        let expected_head_query = "SELECT pubkey, d_tag, raw_head_event_id, raw_head_event_seq, raw_head_created_at FROM radroots_event_store_addressable_head_state WHERE source_generation = ? AND kind = 30402 AND admission_status = 'admitted' AND admission_code IS NULL AND contract_id = ? AND visibility = 'visible' AND nip09_outcome = 'visible' ORDER BY pubkey, d_tag";
        for (needle, replacement) in [
            (", raw_head_created_at", ""),
            ("source_generation = ?", "source_generation IS NOT NULL"),
            ("kind = 30402", "kind = 30340"),
            (
                "admission_status = 'admitted'",
                "admission_status != 'admitted'",
            ),
            ("admission_code IS NULL", "admission_code IS NOT NULL"),
            ("contract_id = ?", "contract_id IS NOT NULL"),
            ("visibility = 'visible'", "visibility != 'visible'"),
            ("nip09_outcome = 'visible'", "nip09_outcome != 'visible'"),
            ("ORDER BY pubkey, d_tag", "ORDER BY d_tag, pubkey"),
        ] {
            let mutated_query = expected_head_query.replacen(needle, replacement, 1);
            assert_ne!(mutated_query, expected_head_query);
            let mutation = source.replacen(expected_head_query, &mutated_query, 1);
            assert_ne!(
                mutation, source,
                "expected-head mutation must apply: {needle}"
            );
            let error = validate_food_projection_audit_authority(&mutation)
                .expect_err("expected-head mutation must fail");
            assert!(!error.is_empty());
        }

        let source_transition_query = "SELECT EXISTS(SELECT 1 FROM radroots_event_store_addressable_head_transition AS transition WHERE transition.transition_seq = ? AND transition.source_generation = ? AND transition.source_generation = (SELECT active_generation FROM radroots_event_store_source_state WHERE singleton = 1) AND transition.kind = 30402 AND transition.pubkey = ? AND transition.d_tag = ? AND transition.raw_head_event_id = ? AND transition.raw_head_event_seq = ? AND transition.raw_head_created_at = ? AND transition.visible_event_id = ? AND transition.visible_event_seq = ? AND transition.admission_status = 'admitted' AND transition.admission_code IS NULL AND transition.contract_id = ? AND transition.visibility = 'visible' AND transition.nip09_outcome = 'visible' AND transition.raw_head_decision IN ('baseline_rebuild', 'applied') AND transition.transition_seq = (SELECT MAX(candidate.transition_seq) FROM radroots_event_store_addressable_head_transition AS candidate WHERE candidate.source_generation = transition.source_generation AND candidate.kind = transition.kind AND candidate.pubkey = transition.pubkey AND candidate.d_tag = transition.d_tag AND candidate.raw_head_decision IN ('baseline_rebuild', 'applied')))";
        for (needle, replacement) in [
            (
                "transition.transition_seq = ?",
                "transition.transition_seq > 0",
            ),
            (
                "transition.source_generation = ?",
                "transition.source_generation IS NOT NULL",
            ),
            (
                "transition.source_generation = (SELECT active_generation FROM radroots_event_store_source_state WHERE singleton = 1)",
                "transition.source_generation IS NOT NULL",
            ),
            ("transition.kind = 30402", "transition.kind = 30340"),
            ("transition.pubkey = ?", "transition.pubkey IS NOT NULL"),
            ("transition.d_tag = ?", "transition.d_tag IS NOT NULL"),
            (
                "transition.raw_head_event_id = ?",
                "transition.raw_head_event_id IS NOT NULL",
            ),
            (
                "transition.raw_head_event_seq = ?",
                "transition.raw_head_event_seq > 0",
            ),
            (
                "transition.raw_head_created_at = ?",
                "transition.raw_head_created_at > 0",
            ),
            (
                "transition.visible_event_id = ?",
                "transition.visible_event_id IS NOT NULL",
            ),
            (
                "transition.visible_event_seq = ?",
                "transition.visible_event_seq > 0",
            ),
            (
                "transition.admission_status = 'admitted'",
                "transition.admission_status != 'admitted'",
            ),
            (
                "transition.admission_code IS NULL",
                "transition.admission_code IS NOT NULL",
            ),
            (
                "transition.contract_id = ?",
                "transition.contract_id IS NOT NULL",
            ),
            (
                "transition.visibility = 'visible'",
                "transition.visibility != 'visible'",
            ),
            (
                "transition.nip09_outcome = 'visible'",
                "transition.nip09_outcome != 'visible'",
            ),
            (
                "transition.raw_head_decision IN ('baseline_rebuild', 'applied')",
                "transition.raw_head_decision = 'applied'",
            ),
            (
                "transition.transition_seq = (SELECT MAX(candidate.transition_seq)",
                "transition.transition_seq <= (SELECT MAX(candidate.transition_seq)",
            ),
        ] {
            let mutated_query = source_transition_query.replacen(needle, replacement, 1);
            assert_ne!(mutated_query, source_transition_query);
            let mutation = source.replacen(source_transition_query, &mutated_query, 1);
            assert_ne!(
                mutation, source,
                "source-transition mutation must apply: {needle}"
            );
            let error = validate_food_projection_audit_authority(&mutation)
                .expect_err("source-transition mutation must fail");
            assert!(!error.is_empty());
        }
    }

    #[test]
    fn immutable_public_api_inventory_is_exact() {
        let mut manifest = immutable_manifest();
        assert_eq!(
            manifest.public_api,
            PUBLIC_API
                .iter()
                .map(|name| (*name).to_owned())
                .collect::<Vec<_>>()
        );
        manifest
            .public_api
            .retain(|name| name != "RadrootsAddressableTransitionCauseV1");
        let error = validate_manifest_shape(&manifest)
            .expect_err("generated manifest with an omitted public symbol must fail");
        assert!(error.contains("successor"), "{error}");
    }

    #[test]
    fn result_vector_is_strict_canonical_and_complete() {
        let root = repository_root();
        let bytes = read_regular_file(&root, RESULT_VECTOR_CANONICAL_RELATIVE).expect("vector");
        let vector: ProjectionResultVector = serde_json::from_slice(&bytes).expect("strict vector");
        validate_canonical_json(RESULT_VECTOR_CANONICAL_RELATIVE, &bytes, &vector)
            .expect("canonical vector");
        validate_result_vector(&vector).expect("complete vector");

        let mut value: Value = serde_json::from_slice(&bytes).expect("vector value");
        value["cases"][2]["expected"]["projection"]
            .as_object_mut()
            .expect("projection")
            .remove("quantity_amount");
        let missing = serde_json::from_value::<ProjectionResultVector>(value)
            .expect_err("required nullable field must be present");
        assert!(missing.to_string().contains("quantity_amount"));

        let mut invalid_sequence: Value =
            serde_json::from_slice(&bytes).expect("vector value for sequence mutation");
        invalid_sequence["cases"][0]["expected"]["transition_page"]["transitions"][0]["transition_seq"] =
            json!(2);
        let invalid_sequence: ProjectionResultVector =
            serde_json::from_value(invalid_sequence).expect("typed sequence mutation");
        let error = validate_result_vector(&invalid_sequence)
            .expect_err("incorrect scoped transition sequence must fail");
        assert!(error.contains("exactly skip unrelated"), "{error}");

        let mut invalid_generation: Value =
            serde_json::from_slice(&bytes).expect("vector value for generation mutation");
        invalid_generation["cases"][0]["expected"]["transition_page"]["transitions"][0]["source_generation"] =
            json!("fixture-generation");
        let invalid_generation: ProjectionResultVector =
            serde_json::from_value(invalid_generation).expect("typed generation mutation");
        let error = validate_result_vector(&invalid_generation)
            .expect_err("non-authoritative source-generation sentinel must fail");
        assert!(error.contains("invalid authority witness"), "{error}");

        let mut missing_cause: Value =
            serde_json::from_slice(&bytes).expect("vector value for cause mutation");
        missing_cause["cases"][0]["expected"]["transition_page"]["transitions"][0]["cause_event"] =
            Value::Null;
        let missing_cause: ProjectionResultVector =
            serde_json::from_value(missing_cause).expect("typed nullable cause mutation");
        let error = validate_result_vector(&missing_cause)
            .expect_err("incremental transition without a cause must fail");
        assert!(error.contains("must authenticate its cause"), "{error}");

        let mut stale_canonical_digest: Value =
            serde_json::from_slice(&bytes).expect("vector value for digest mutation");
        stale_canonical_digest["cases"][0]["expected"]["transition_page"]["transitions"][0]["canonical_visible_event"]
            ["raw_json_sha256"] = json!("00".repeat(32));
        let stale_canonical_digest: ProjectionResultVector =
            serde_json::from_value(stale_canonical_digest).expect("typed digest mutation");
        let error = validate_result_vector(&stale_canonical_digest)
            .expect_err("stale canonical raw JSON digest must fail");
        assert!(
            error.contains("canonical raw JSON digest is stale"),
            "{error}"
        );

        let mut mislabeled_operational: Value =
            serde_json::from_slice(&bytes).expect("vector value for Operational role mutation");
        mislabeled_operational["cases"][6]["events"][1]["role"] = json!("scoped_food");
        let mislabeled_operational: ProjectionResultVector =
            serde_json::from_value(mislabeled_operational)
                .expect("typed Operational role mutation");
        let error = validate_result_vector(&mislabeled_operational)
            .expect_err("Operational head mislabeled as Food must fail");
        assert!(error.contains("incoherent ingest witness"), "{error}");

        let mut missing_historical: Value =
            serde_json::from_slice(&bytes).expect("vector value for historical witness mutation");
        missing_historical["cases"][7]["expected"]["historical_visibility_witnesses"] = json!([]);
        let missing_historical: ProjectionResultVector =
            serde_json::from_value(missing_historical).expect("typed historical witness mutation");
        let error = validate_result_vector(&missing_historical)
            .expect_err("missing divergent historical payload witnesses must fail");
        assert!(error.contains("historical visibility witnesses"), "{error}");

        let mut mislabeled_unrelated: Value =
            serde_json::from_slice(&bytes).expect("vector value for unrelated role mutation");
        mislabeled_unrelated["cases"][8]["events"][1]["role"] = json!("scoped_food");
        mislabeled_unrelated["cases"][8]["events"][1]["expected_ingest"]["contract_id"] =
            json!(FOOD_CONTRACT_ID);
        let mislabeled_unrelated: ProjectionResultVector =
            serde_json::from_value(mislabeled_unrelated).expect("typed unrelated role mutation");
        let error = validate_result_vector(&mislabeled_unrelated)
            .expect_err("unrelated addressable input mislabeled as scoped must fail");
        assert!(error.contains("scoped input does not match"), "{error}");

        let mut leaked_unrelated_transition: Value =
            serde_json::from_slice(&bytes).expect("vector value for feed-scope mutation");
        let mut leaked =
            leaked_unrelated_transition["cases"][8]["expected"]["transition_page"]["transitions"]
                [0]
            .clone();
        leaked["transition_seq"] = json!(2);
        leaked_unrelated_transition["cases"][8]["expected"]["transition_page"]["transitions"]
            .as_array_mut()
            .expect("transition array")
            .insert(1, leaked);
        let leaked_unrelated_transition: ProjectionResultVector =
            serde_json::from_value(leaked_unrelated_transition).expect("typed feed-scope mutation");
        let error = validate_result_vector(&leaked_unrelated_transition)
            .expect_err("unrelated addressable traffic in the scoped feed must fail");
        assert!(error.contains("exactly skip unrelated"), "{error}");

        let mut stale_high_water: Value =
            serde_json::from_slice(&bytes).expect("vector value for high-water mutation");
        stale_high_water["cases"][8]["expected"]["transition_page"]["source_high_water"] = json!(2);
        let stale_high_water: ProjectionResultVector =
            serde_json::from_value(stale_high_water).expect("typed high-water mutation");
        let error = validate_result_vector(&stale_high_water)
            .expect_err("cursor high-water that omits unrelated traffic must fail");
        assert!(error.contains("complete active-source interval"), "{error}");
    }

    #[test]
    fn schema_rejects_unknown_manifest_fields() {
        let schema: Value = serde_json::from_slice(IMMUTABLE_MANIFEST_SCHEMA_BYTES)
            .expect("immutable manifest schema");
        let manifest = immutable_manifest();
        let mut value = serde_json::to_value(manifest).expect("manifest value");
        value
            .as_object_mut()
            .expect("manifest object")
            .insert("unknown".to_owned(), Value::Bool(true));
        let error = validate_manifest_json_schema(&schema, &value)
            .expect_err("unknown manifest field must fail");
        assert!(error.contains("Additional properties"), "{error}");
    }

    #[test]
    fn generated_descriptor_covers_runtime_pointer_constants() {
        let manifest = immutable_manifest();
        let descriptor = std::str::from_utf8(IMMUTABLE_GENERATED_DESCRIPTOR_BYTES)
            .expect("UTF-8 immutable generated descriptor");
        assert_eq!(manifest.migration.schema_sha256, SCHEMA_SHA256);
        for name in [
            "FOOD_AVAILABILITY_PROJECTION_MANIFEST_SCHEMA_VERSION",
            "FOOD_AVAILABILITY_PROJECTION_CONTRACT_ID",
            "FOOD_AVAILABILITY_PROJECTION_HOOK_ID",
            "FOOD_AVAILABILITY_PROJECTION_MIGRATION_VERSION",
            "FOOD_AVAILABILITY_PROJECTION_MIGRATION_UP_SHA256",
            "FOOD_AVAILABILITY_PROJECTION_MIGRATION_DOWN_SHA256",
            "FOOD_AVAILABILITY_PROJECTION_SCOPE_KINDS",
            "FOOD_AVAILABILITY_PROJECTION_SCOPE_FINGERPRINT_SHA256",
            "FOOD_AVAILABILITY_PROJECTION_PREDECESSOR_MANIFEST_SHA256",
            "FOOD_AVAILABILITY_PROJECTION_RESULT_VECTOR_EXECUTOR_SHA256",
        ] {
            assert!(descriptor.contains(name), "missing {name}");
        }
        for (name, value) in [
            ("FOOD_AVAILABILITY_PROJECTION_HOOK_ID", manifest.hook_id),
            (
                "FOOD_AVAILABILITY_PROJECTION_MIGRATION_NAME",
                manifest.migration.name,
            ),
        ] {
            let expected = format!("pub(crate) const {name}: &str = {value:?};\n");
            assert!(
                descriptor.contains(&expected),
                "{name} must use the rustfmt-stable one-line assignment"
            );
        }
        syn::parse_file(descriptor).expect("generated descriptor parses as Rust");
    }
}
