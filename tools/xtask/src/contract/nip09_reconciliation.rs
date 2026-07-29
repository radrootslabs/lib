#![allow(dead_code)]

use super::artifact_bundle::{
    GeneratedArtifact, read_regular_file, validate_workspace_path, with_artifact_bundle_transaction,
};
use super::registry_v7::validate_event_contract_registry_v7_inventory_under_lock;
use quote::ToTokens;
use semver::Version;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::path::Path;

const SCHEMA_VERSION: u32 = 1;
const HOOK_ID: &str = "nip09_reconciliation_v1";
const MIGRATION_VERSION: u32 = 2;
const MIGRATION_NAME: &str = "nip09";
const SOURCE_MAINTENANCE_MIGRATION_VERSION: u32 = 4;
const SOURCE_MAINTENANCE_MIGRATION_NAME: &str = "source_maintenance";
const SOURCE_MAINTENANCE_PRIVILEGED_TERMINALS: [&str; 9] = [
    "apply_source_maintenance_hook_v1",
    "preflight_unique_raw_source_append_v1",
    "raw_source_capacity_delta_v1",
    "advance_source_capacity_after_insert_v1",
    "preflight_source_generation_append_v1",
    "bind_source_capacity_to_generation_v1",
    "validate_source_capacity_authority_fast_v1",
    "validate_source_capacity_authority_full_v1",
    "validate_no_persisted_ephemeral_raw_rows_v1",
];
const PRIVILEGED_TERMINAL_NAMES: [&str; 21] = [
    "validate_event_store_temp_schema",
    "validate_main_database_encoding",
    "validate_rollback_preserves_source_generation_history",
    "apply_migration_up",
    "apply_migration_down",
    "apply_migration_hook",
    "validate_migration_hook_state",
    "validate_active_hook_state_fast",
    "ingest_event_protocol_reconciliation_v1",
    "dispatch_post_core_extensions",
    "apply_post_core_extensions_v1",
    "validate_protocol_post_extensions",
    "apply_source_maintenance_hook_v1",
    "preflight_unique_raw_source_append_v1",
    "raw_source_capacity_delta_v1",
    "advance_source_capacity_after_insert_v1",
    "preflight_source_generation_append_v1",
    "bind_source_capacity_to_generation_v1",
    "validate_source_capacity_authority_fast_v1",
    "validate_source_capacity_authority_full_v1",
    "validate_no_persisted_ephemeral_raw_rows_v1",
];
const RECONCILIATION_VERSION: u32 = 1;
const ADDRESSABLE_FEED_VERSION: u32 = 1;
const EVENT_CONTRACT_REGISTRY_VERSION: u32 = 7;
const SQLITE_I64_MAX_U64: u64 = i64::MAX as u64;
const SCHEMA_SHA256: &str = "1fee6b2bb8cdc4602d9c89fecd97c3f51312b9a4339dbf5049b04c692ba50b12";

const MIGRATION_UP_RELATIVE: &str = "crates/event_store/migrations/0002_nip09.up.sql";
const MIGRATION_DOWN_RELATIVE: &str = "crates/event_store/migrations/0002_nip09.down.sql";
const MIGRATION_V1_UP_RELATIVE: &str = "crates/event_store/migrations/0001_event_store.up.sql";
const MIGRATION_V1_DOWN_RELATIVE: &str = "crates/event_store/migrations/0001_event_store.down.sql";
const MIGRATION_V1_UP_SHA256: &str =
    "4c03906a1cffd418a48d40907aa9a1ca51bb41766cff7250c4dfc7c2fd6eddde";
const MIGRATION_V1_DOWN_SHA256: &str =
    "fa84d587f657f601947eaeb9cd239c962a48f6fcdce723588476e8d22f3c1f53";
const RUST_TOOLCHAIN_RELATIVE: &str = "rust-toolchain.toml";
const REGISTRY_INVENTORY_RELATIVE: &str =
    "contracts/event_store/event_contract_registry_v7.inventory.json";
const RESULT_VECTOR_CANONICAL_RELATIVE: &str =
    "contracts/conformance/vectors/event_store/nip09_reconciliation.v1.json";
const RESULT_VECTOR_MIRROR_RELATIVE: &str =
    "crates/event_store/tests/fixtures/nip09_reconciliation.v1.json";
const RESULT_VECTOR_EXECUTOR_RELATIVE: &str =
    "crates/event_store/src/nip09/reconciliation_v1/result_vector_executor.rs";
const RESULT_VECTOR_EXECUTOR_ID: &str =
    "radroots_event_store.nip09_reconciliation_v1.result_vector_executor.v1";
const RESULT_VECTOR_EXECUTOR_TEST: &str = "nip09_reconciliation_v1_result_vector";
const RESULT_VECTOR_INCLUDE_PATH: &str = "../../../tests/fixtures/nip09_reconciliation.v1.json";
const EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE: &str = "crates/event_store/src/migrations.rs";
const EVENT_STORE_SCHEMA_SOURCE_RELATIVE: &str = "crates/event_store/src/schema.rs";
const EVENT_STORE_LIB_SOURCE_RELATIVE: &str = "crates/event_store/src/lib.rs";
const EVENT_STORE_SOURCE_ROOT_RELATIVE: &str = "crates/event_store/src";
const EVENT_STORE_STORE_SOURCE_RELATIVE: &str = "crates/event_store/src/store.rs";
const EVENT_STORE_STORE_MODULE_ROOT_RELATIVE: &str = "crates/event_store/src/store";
const EVENT_STORE_STORE_ROOT_BASELINE_SHA256: &str =
    "d277b8ec0cdca325c54f60e2ae685bb9afbbf470d2eb68d8624a3d8270bc8fac";
const EVENT_STORE_MIGRATION_IMPL_BASELINE_SHA256: &str =
    "69f3c730f8a4f3a4af0028c74f6903126def01ceb63eed8a173e696ce291dc09";
const EVENT_CRATE_ROOT_BASELINE_SHA256: &str =
    "ec032e602c1afaebc5a998e74cd1e70066afd9a07b82bce2d0d7307caac15cd6";
const EVENT_CODEC_CRATE_ROOT_BASELINE_SHA256: &str =
    "8c29ceccb06abc94279db3257d52e14ad721e2eb302e54fbf26f8856d0db0b53";
const BLOSSOM_CRATE_ROOT_BASELINE_SHA256: &str =
    "5066faee05fad71a94be757767f67bf99a10e5637acc41522cefe7a37eb0b4e4";
const ROUTE_FACADE_BASELINE_SHA256: &str =
    "8891c7824e4db6de269f2b833f2cb25510967034145423acdbac559b3ad5a52d";
const ROUTE_FACADE_BASELINE_SOURCES: [&str; 17] = [
    "crates/event/src/trade.rs",
    "crates/event_codec/src/admission.rs",
    "crates/event_codec/src/comment/mod.rs",
    "crates/event_codec/src/food_availability/mod.rs",
    "crates/event_codec/src/post/mod.rs",
    "crates/event_codec/src/profile/mod.rs",
    "crates/event_codec/src/reply/mod.rs",
    "crates/transport/src/kind.rs",
    "crates/transport/src/lib.rs",
    "crates/transport/src/target.rs",
    "crates/event_store/src/error.rs",
    "crates/event_store/src/generated.rs",
    "crates/event_store/src/model.rs",
    "crates/event_store/src/nip09.rs",
    "crates/event_store/src/schema.rs",
    POST_CORE_EXTENSION_SOURCE_RELATIVE,
    POST_CORE_STORAGE_SOURCE_RELATIVE,
];
#[derive(Clone, Copy)]
struct GovernedSourceTreeBaselineSpec {
    root: &'static str,
    sha256: &'static str,
}

const GOVERNED_SUPPORT_SOURCE_TREE_BASELINES: [GovernedSourceTreeBaselineSpec; 5] = [
    GovernedSourceTreeBaselineSpec {
        root: "crates/core/src",
        sha256: "457263c17ea90679d348cfdfb2f1b7b151a0185223315a748de2acc6a3f661d6",
    },
    GovernedSourceTreeBaselineSpec {
        root: "crates/event/src",
        sha256: "0468476622c9b68464383b3d9e7ef418e942fa7388e289aef7ee9e6aee87bbd2",
    },
    GovernedSourceTreeBaselineSpec {
        root: "crates/event_codec/src",
        sha256: "bc7b6fd6e9fb995dc6904c76ebeab51bf9eb0bc1dbf5f447e41849095b18effe",
    },
    GovernedSourceTreeBaselineSpec {
        root: "crates/blossom/src",
        sha256: "2f1dbfe352e1901f5649984464a8b4ed065323c15a21b39a9b476351f9a430a1",
    },
    GovernedSourceTreeBaselineSpec {
        root: "crates/transport/src",
        sha256: "79446e9acaaa4eaa5e00158ddea5ee619fef8535cd788256595f8120ebd67a2a",
    },
];
const SUPPORT_ITEM_MACRO_SOURCE_ALLOWLIST: [&str; 2] = [
    "crates/event/src/contract/registry_v7.rs",
    "crates/event/src/ids.rs",
];
const EVENT_STORE_PROTOCOL_RECONCILIATION_SOURCE_RELATIVE: &str =
    "crates/event_store/src/store/protocol_reconciliation_v1.rs";
const EVENT_STORE_PROTOCOL_STORAGE_SOURCE_RELATIVE: &str =
    "crates/event_store/src/store/protocol_storage_v1.rs";
const LOCAL_SQLITE_SOURCE_RELATIVE: &str = "crates/libsqlite3_sys_3_53_3";
const LOCAL_SQLITE_PACKAGE: &str = "libsqlite3-sys";
const LOCAL_SQLITE_VERSION: &str = "0.37.0";
const LOCAL_SQLITE_LOCK_SOURCE: &str = "path+crates/libsqlite3_sys_3_53_3";
const LOCAL_SOURCE_TREE_ALGORITHM: &str = "canonical_json_file_inventory_sha256_v1";
const LOCAL_SQLITE_REQUIRED_FILES: [&str; 15] = [
    "Cargo.toml",
    "LICENSE",
    "bindgen-bindings/bindgen_3.34.1.rs",
    "bindgen-bindings/bindgen_3.34.1_ext.rs",
    "build.rs",
    "sqlite3/bindgen_bundled_version.rs",
    "sqlite3/bindgen_bundled_version_ext.rs",
    "sqlite3/sqlite3.c",
    "sqlite3/sqlite3.h",
    "sqlite3/sqlite3ext.h",
    "sqlite3/wasm32-wasi-vfs.c",
    "src/error.rs",
    "src/lib.rs",
    "wrapper.h",
    "wrapper_ext.h",
];
const CARGO_LOCK_RELATIVE: &str = "Cargo.lock";
const WORKSPACE_CARGO_MANIFEST_RELATIVE: &str = "Cargo.toml";
const EVENT_STORE_CARGO_MANIFEST_RELATIVE: &str = "crates/event_store/Cargo.toml";
const EVENT_CODEC_CARGO_MANIFEST_RELATIVE: &str = "crates/event_codec/Cargo.toml";
const EVENT_CARGO_MANIFEST_RELATIVE: &str = "crates/event/Cargo.toml";
const CORE_CARGO_MANIFEST_RELATIVE: &str = "crates/core/Cargo.toml";
const BLOSSOM_CARGO_MANIFEST_RELATIVE: &str = "crates/blossom/Cargo.toml";
const TRANSPORT_CARGO_MANIFEST_RELATIVE: &str = "crates/transport/Cargo.toml";
const CARGO_CONFIG_RELATIVE: &str = ".cargo/config.toml";
const GOVERNED_WORKSPACE_DEPENDENCY_NAMES: [&str; 23] = [
    "dto_bindgen",
    "futures",
    "getrandom",
    "hex",
    "jiff-tzdb",
    "mediatype",
    "nostr",
    "radroots_blossom",
    "radroots_core",
    "radroots_event",
    "radroots_event_codec",
    "radroots_transport",
    "rust_decimal",
    "secp256k1",
    "serde",
    "serde_json",
    "sha2",
    "sqlx",
    "tempfile",
    "thiserror",
    "tokio",
    "unicode-general-category",
    "url_nostd",
];
const GOVERNED_DEPENDENCY_TABLE_SHA256: [(&str, &str); 7] = [
    (
        CORE_CARGO_MANIFEST_RELATIVE,
        "79f6cee54c1a4f7ca0d029c6cc6ef1493ee9d441e231634a04885ac85d793759",
    ),
    (
        EVENT_CARGO_MANIFEST_RELATIVE,
        "d37e85cb3be5a72471b9bdf3ed405433dd558cb546c24a63f9c420529eaede5a",
    ),
    (
        EVENT_CODEC_CARGO_MANIFEST_RELATIVE,
        "7f34026a9d53ab8e2bb2dcf79d00850790b19f82b218dc9b9f25e3a4999a32be",
    ),
    (
        BLOSSOM_CARGO_MANIFEST_RELATIVE,
        "5029df69fa04b89850e4411c3a57ead718bbad42bac43404602eddb7b24e8b16",
    ),
    (
        EVENT_STORE_CARGO_MANIFEST_RELATIVE,
        "8ecd900f34ea701429e326911dff0ed49364b4b5aee654ee9f68b0c1e1bf7cbf",
    ),
    (
        TRANSPORT_CARGO_MANIFEST_RELATIVE,
        "7839356e3ba830b758327552bfa6295747d5073ac023f5d2ab47bdd0c6866972",
    ),
    (
        "Cargo.toml#governed-workspace-dependencies",
        "4407cc5e0b79b323d98d1ec8988c3e597315d435463758793a4f3df886e62997",
    ),
];

const MANIFEST_RELATIVE: &str =
    "crates/event_store/contracts/nip09_reconciliation_v1.manifest.json";
const MANIFEST_SCHEMA_RELATIVE: &str =
    "crates/event_store/contracts/nip09_reconciliation_v1.manifest.schema.json";
const MANIFEST_SHA256_RELATIVE: &str =
    "crates/event_store/contracts/nip09_reconciliation_v1.manifest.sha256";
const GENERATED_DESCRIPTOR_RELATIVE: &str =
    "crates/event_store/src/generated/nip09_reconciliation_manifest.rs";
const WRITE_COMMAND: &str = "cargo xtask contract nip09-reconciliation-manifest --write";

const IMMUTABLE_MANIFEST_BYTES: &[u8] = include_bytes!(
    "../../../../crates/event_store/contracts/nip09_reconciliation_v1.manifest.json"
);
const IMMUTABLE_MANIFEST_SCHEMA_BYTES: &[u8] = include_bytes!(
    "../../../../crates/event_store/contracts/nip09_reconciliation_v1.manifest.schema.json"
);
const IMMUTABLE_MANIFEST_SHA256_BYTES: &[u8] = include_bytes!(
    "../../../../crates/event_store/contracts/nip09_reconciliation_v1.manifest.sha256"
);
const IMMUTABLE_GENERATED_DESCRIPTOR_BYTES: &[u8] =
    include_bytes!("../../../../crates/event_store/src/generated/nip09_reconciliation_manifest.rs");

#[derive(Clone, Copy)]
struct ImmutableArtifactSpec {
    relative: &'static str,
    byte_length: usize,
    sha256: &'static str,
}

const IMMUTABLE_PREDECESSOR_ARTIFACTS: [ImmutableArtifactSpec; 11] = [
    ImmutableArtifactSpec {
        relative: MANIFEST_RELATIVE,
        byte_length: 537_538,
        sha256: "74af832420ffbaa9805e89df3c0b34f126a443e1598f757e3372f407f9003b77",
    },
    ImmutableArtifactSpec {
        relative: MANIFEST_SCHEMA_RELATIVE,
        byte_length: 28_805,
        sha256: "eac277641b197ec2e7690ae0a513640a4d93d5be713f1e96e9932cbd75cbfc58",
    },
    ImmutableArtifactSpec {
        relative: MANIFEST_SHA256_RELATIVE,
        byte_length: 65,
        sha256: "1b4513933ecc96d7f07e48e27bd029bb2b791ebe9771ce516ba2cb3bb7b24080",
    },
    ImmutableArtifactSpec {
        relative: GENERATED_DESCRIPTOR_RELATIVE,
        byte_length: 586_039,
        sha256: "406a760e9bed1e8fc89c8e7ae0976c7eff844de7427a3f473528c895439500b3",
    },
    ImmutableArtifactSpec {
        relative: RESULT_VECTOR_CANONICAL_RELATIVE,
        byte_length: 10_405,
        sha256: "31cd9507734ff3308436881622a626b9782b75b548d9f5e159e4125621855b9c",
    },
    ImmutableArtifactSpec {
        relative: RESULT_VECTOR_MIRROR_RELATIVE,
        byte_length: 10_405,
        sha256: "31cd9507734ff3308436881622a626b9782b75b548d9f5e159e4125621855b9c",
    },
    ImmutableArtifactSpec {
        relative: RESULT_VECTOR_EXECUTOR_RELATIVE,
        byte_length: 18_451,
        sha256: "9e9cc8d2f2382da6c73e78d73d41015559a5a3649b797cce13d7aafa9bbcc8d7",
    },
    ImmutableArtifactSpec {
        relative: MIGRATION_V1_UP_RELATIVE,
        byte_length: 10_712,
        sha256: MIGRATION_V1_UP_SHA256,
    },
    ImmutableArtifactSpec {
        relative: MIGRATION_V1_DOWN_RELATIVE,
        byte_length: 522,
        sha256: MIGRATION_V1_DOWN_SHA256,
    },
    ImmutableArtifactSpec {
        relative: MIGRATION_UP_RELATIVE,
        byte_length: 81_614,
        sha256: "0c1730ff36eaebd285f9c0c94b9b7346af60266afa55c24a18e30446d369581a",
    },
    ImmutableArtifactSpec {
        relative: MIGRATION_DOWN_RELATIVE,
        byte_length: 4_807,
        sha256: "c51a099d9501f1e692c13d2226296a68ed9e6bfa5e8e46b2f12c6574dbe59e31",
    },
];

const RUNTIME_DEPENDENCY_ALGORITHM: &str = "cargo_lock_resolved_semantic_subgraph_v1";
const RUST_PRODUCTION_AST_SHA256_ALGORITHM: &str = "rust_production_ast_sha256_v1";
const RUST_FULL_AST_SHA256_ALGORITHM: &str = "rust_full_ast_sha256_v1";
const IMPL_RESOLUTION_WITNESS_ALGORITHM: &str =
    "protected_v1_resolution_relevant_impl_ast_sha256_v2";
const IMPL_RESOLUTION_SOURCE_ROOTS: [&str; 5] = [
    "crates/core/src",
    "crates/event/src",
    "crates/event_codec/src",
    "crates/blossom/src",
    EVENT_STORE_SOURCE_ROOT_RELATIVE,
];
const POST_CORE_SQL_CAPABILITY_ALGORITHM: &str = "post_core_storage_sql_bind_capability_v3";
const POST_CORE_CAPABILITIES_SOURCE_RELATIVE: &str =
    "crates/event_store/src/store/post_core_extension_capabilities.rs";
const POST_CORE_DISPATCHER_SOURCE_RELATIVE: &str =
    "crates/event_store/src/store/post_core_extension_dispatcher.rs";
const POST_CORE_EXTENSION_SOURCE_RELATIVE: &str =
    "crates/event_store/src/store/post_core_extensions_v1.rs";
const POST_CORE_STORAGE_SOURCE_RELATIVE: &str =
    "crates/event_store/src/store/post_core_storage_v1.rs";
const POST_CORE_EXTENSION_ROOT: &str = "apply_post_core_extensions_v1";
const PRIVILEGED_STORE_MODULE_SOURCES: [&str; 6] = [
    POST_CORE_CAPABILITIES_SOURCE_RELATIVE,
    POST_CORE_DISPATCHER_SOURCE_RELATIVE,
    POST_CORE_EXTENSION_SOURCE_RELATIVE,
    POST_CORE_STORAGE_SOURCE_RELATIVE,
    EVENT_STORE_PROTOCOL_RECONCILIATION_SOURCE_RELATIVE,
    EVENT_STORE_PROTOCOL_STORAGE_SOURCE_RELATIVE,
];
const PRIVILEGED_STORE_MODULE_NAMES: [&str; 6] = [
    "post_core_extension_capabilities",
    "post_core_extension_dispatcher",
    "post_core_extensions_v1",
    "post_core_storage_v1",
    "protocol_reconciliation_v1",
    "protocol_storage_v1",
];
const SUCCESSOR_08C_STORE_MODULE_SOURCES: [&str; 5] = [
    "crates/event_store/src/store/addressable_transition_feed_v1.rs",
    "crates/event_store/src/store/current_visibility_v1.rs",
    "crates/event_store/src/store/food_availability_projection_v1.rs",
    "crates/event_store/src/store/post_core_extensions_v2.rs",
    "crates/event_store/src/store/post_core_storage_v2.rs",
];
const SUCCESSOR_08C_STORE_MODULE_NAMES: [&str; 5] = [
    "addressable_transition_feed_v1",
    "current_visibility_v1",
    "food_availability_projection_v1",
    "post_core_extensions_v2",
    "post_core_storage_v2",
];
const SUCCESSOR_08C_EXCLUSIVE_SOURCE_PATHS: [&str; 8] = [
    "crates/event_store/src/model/addressable_transition_feed_v1.rs",
    "crates/event_store/src/model/current_visibility_v1.rs",
    "crates/event_store/src/model/food_availability_projection_v1.rs",
    "crates/event_store/src/store/addressable_transition_feed_v1.rs",
    "crates/event_store/src/store/current_visibility_v1.rs",
    "crates/event_store/src/store/food_availability_projection_v1.rs",
    "crates/event_store/src/store/post_core_extensions_v2.rs",
    "crates/event_store/src/store/post_core_storage_v2.rs",
];
const SUCCESSOR_08D_SOURCE_PATHS: [&str; 1] = ["crates/event_store/src/source_maintenance_v1.rs"];
const SUCCESSOR_08D_LIB_MODULES: [&str; 1] = ["source_maintenance_v1"];
const RAW_SOURCE_REBUILD_SOURCE_RELATIVE: &str =
    "crates/event_store/src/nip09/reconciliation_v1/raw_source_rebuild.rs";
const RAW_SOURCE_REBUILD_TEST_SOURCE_RELATIVE: &str =
    "crates/event_store/src/store/raw_source_rebuild_v1_tests.rs";
const SUCCESSOR_08D1_EXCLUSIVE_SOURCE_PATHS: [&str; 5] = [
    "crates/event_store/src/generated/raw_source_rebuild_manifest.rs",
    "crates/event_store/src/model/raw_source_rebuild_v1.rs",
    RAW_SOURCE_REBUILD_SOURCE_RELATIVE,
    "crates/event_store/src/nip09/reconciliation_v1/visibility_oracle_v1.rs",
    RAW_SOURCE_REBUILD_TEST_SOURCE_RELATIVE,
];
const EVENT_STORE_FIXED_PUBLIC_REEXPORTS: [&str; 40] = [
    "error::RadrootsEventStoreError",
    "error::RadrootsEventStoreReconciliationResource",
    "migrations::RADROOTS_EVENT_STORE_SCHEMA_VERSION_CURRENT",
    "migrations::RADROOTS_EVENT_STORE_SCHEMA_VERSION_MIN",
    "model::RADROOTS_TRANSPORT_OBSERVATION_MESSAGE_MAX_BYTES",
    "model::RadrootsEventAdmissionStatus",
    "model::RadrootsEventIngest",
    "model::RadrootsEventIngestReceipt",
    "model::RadrootsEventPersistence",
    "model::RadrootsEventStoreSourceGeneration",
    "model::RadrootsEventStoreStatusSummary",
    "model::RadrootsEventVisibility",
    "model::RadrootsProjectionCursor",
    "model::RadrootsProjectionRebuildPrior",
    "model::RadrootsProjectionRebuildTicket",
    "model::RadrootsRawHeadDecision",
    "model::RadrootsStoredEventTag",
    "model::RadrootsStoredRawEvent",
    "model::RadrootsStoredRawEventHead",
    "model::RadrootsStoredSellerReservation",
    "model::RadrootsStoredSellerReservationLine",
    "model::RadrootsStoredTradeMissingParent",
    "model::RadrootsStoredTradeMutation",
    "model::RadrootsStoredTradeMutationParent",
    "model::RadrootsStoredTradeTransportEnvelope",
    "model::RadrootsStoredValidEvent",
    "model::RadrootsStoredVisibleEvent",
    "model::RadrootsStoredVisibleEventHead",
    "model::RadrootsTradeProjectionCheckpoint",
    "model::RadrootsTransportObservation",
    "model::RadrootsTransportObservationMessage",
    "model::RadrootsTransportObservationType",
    "model::StoredEventClass",
    "schema::RadrootsEventStoreSchemaStatus",
    "schema::inspect_event_store_schema_status",
    "store::RADROOTS_EVENT_STORE_CONTRACT_QUERY_LIMIT_MAX",
    "store::RADROOTS_EVENT_STORE_QUERY_LIMIT_MAX",
    "store::RadrootsEventStore",
    "store::RadrootsTransportObservationRow",
    "store::inspect_event_store_status",
];
const SUCCESSOR_08C_PUBLIC_REEXPORTS: [&str; 32] = [
    "model::RADROOTS_ADDRESSABLE_TRANSITION_CURSOR_JSON_MAX_BYTES_V1",
    "model::RADROOTS_ADDRESSABLE_TRANSITION_D_TAG_MAX_BYTES_V1",
    "model::RADROOTS_ADDRESSABLE_TRANSITION_FEED_VERSION_V1",
    "model::RADROOTS_ADDRESSABLE_TRANSITION_PAGE_LIMIT_MAX_V1",
    "model::RADROOTS_ADDRESSABLE_TRANSITION_PAGE_RAW_JSON_MAX_BYTES_V1",
    "model::RADROOTS_ADDRESSABLE_TRANSITION_PAGE_SCAN_MAX_V1",
    "model::RADROOTS_ADDRESSABLE_TRANSITION_SCOPE_KIND_MAX_V1",
    "model::RADROOTS_FOOD_AVAILABILITY_PROJECTION_APPLY_PAGE_LIMIT_V1",
    "model::RADROOTS_FOOD_AVAILABILITY_PROJECTION_VERSION_V1",
    "model::RADROOTS_FOOD_AVAILABILITY_SEARCH_QUERY_MAX_BYTES_V1",
    "model::RADROOTS_FOOD_AVAILABILITY_SEARCH_QUERY_MAX_TERMS_V1",
    "model::RadrootsAddressableTransitionCauseV1",
    "model::RadrootsAddressableTransitionCoordinateV1",
    "model::RadrootsAddressableTransitionCursorV1",
    "model::RadrootsAddressableTransitionEventReferenceV1",
    "model::RadrootsAddressableTransitionOriginV1",
    "model::RadrootsAddressableTransitionPageV1",
    "model::RadrootsAddressableTransitionRawHeadDecisionV1",
    "model::RadrootsAddressableTransitionScopeFingerprintV1",
    "model::RadrootsAddressableTransitionScopeV1",
    "model::RadrootsAddressableTransitionV1",
    "model::RadrootsAddressableTransitionVisibilityV1",
    "model::RadrootsCurrentEventVisibilityV1",
    "model::RadrootsCurrentVisibilityDecisionV1",
    "model::RadrootsFoodAvailabilitySearchQueryV1",
    "model::RadrootsFoodAvailabilityStatusFilterV1",
    "model::RadrootsNip09SuppressionEvidenceV1",
    "model::RadrootsNip09SuppressionOutcome",
    "model::RadrootsNip09SuppressionReason",
    "model::RadrootsStoreProducedCanonicalEventV1",
    "model::RadrootsStoredFoodAvailabilityImageV1",
    "model::RadrootsStoredFoodAvailabilityV1",
];
const SUCCESSOR_08D_RETIRED_PUBLIC_REEXPORTS: [&str; 1] =
    ["error::RadrootsEventStoreReconciliationResource"];
const SUCCESSOR_08D_PUBLIC_REEXPORTS: [&str; 7] = [
    "error::RADROOTS_EVENT_STORE_RAW_EVENT_COUNT_LIMIT_V1",
    "error::RADROOTS_EVENT_STORE_RAW_EVENT_TEXT_BYTES_LIMIT_V1",
    "error::RADROOTS_EVENT_STORE_RAW_TAG_COUNT_LIMIT_V1",
    "error::RADROOTS_EVENT_STORE_RAW_TAG_TEXT_BYTES_LIMIT_V1",
    "error::RADROOTS_EVENT_STORE_RETAINED_SOURCE_GENERATION_LIMIT_V1",
    "error::RadrootsEventStoreSourceCapacityResourceV1",
    "source_maintenance_v1::RadrootsEventStoreSourceCapacityV1",
];
const SUCCESSOR_08D1_PUBLIC_REEXPORTS: [&str; 6] = [
    "error::RADROOTS_EVENT_STORE_PROJECTION_CURSOR_COUNT_LIMIT_V1",
    "error::RadrootsEventStoreCallerInboundForeignKeyV1",
    "error::RadrootsEventStoreRawSourceRebuildDriftV1",
    "model::RadrootsEventStoreActiveProductStateDigestV1",
    "model::RadrootsEventStoreImmutableRawDigestV1",
    "model::RadrootsEventStoreRawSourceRebuildReportV1",
];
const POST_CORE_STORAGE_METHODS: [&str; 4] = [
    "new",
    "quarantine_trade",
    "persist_trade_projection",
    "upsert_transport_observation",
];
const POST_CORE_SQL_ALLOWED_CAPABILITIES: [PostCoreSqlOperationCapabilitySpec; 10] = [
    PostCoreSqlOperationCapabilitySpec::new("delete", "trade_missing_parent"),
    PostCoreSqlOperationCapabilitySpec::new("insert", "seller_inventory_reservation"),
    PostCoreSqlOperationCapabilitySpec::new("insert", "seller_inventory_reservation_line"),
    PostCoreSqlOperationCapabilitySpec::new("insert", "trade_missing_parent"),
    PostCoreSqlOperationCapabilitySpec::new("insert", "trade_mutation"),
    PostCoreSqlOperationCapabilitySpec::new("insert", "trade_mutation_parent"),
    PostCoreSqlOperationCapabilitySpec::new("insert", "trade_projection_quarantine"),
    PostCoreSqlOperationCapabilitySpec::new("insert", "trade_transport_envelope"),
    PostCoreSqlOperationCapabilitySpec::new("select", "trade_mutation"),
    PostCoreSqlOperationCapabilitySpec::new("upsert", "event_transport_observation"),
];
const POST_CORE_SQL_FORBIDDEN_CLASSES: [&str; 16] = [
    "ambient_side_effect_authority",
    "attached_or_schema_qualified_database",
    "comment_or_multiple_statement",
    "cte_or_compound_statement",
    "ddl_or_trigger",
    "dynamic_sql",
    "extension_raw_transaction_authority",
    "extension_sqlx_authority",
    "pragma_or_vacuum",
    "protocol_authority_access",
    "quoted_identifier",
    "schema_authority_access",
    "transaction_escape",
    "unbounded_storage_api",
    "unsupported_query_constructor",
    "write_outside_allowed_operation_table_pair",
];

#[derive(Clone, Copy)]
struct RuntimeDependencyRootSpec {
    owner: &'static str,
    name: &'static str,
    expected_version: Option<&'static str>,
}

const RUNTIME_DEPENDENCY_ROOTS: [RuntimeDependencyRootSpec; 10] = [
    RuntimeDependencyRootSpec {
        owner: "radroots_event_store",
        name: "hex",
        expected_version: None,
    },
    RuntimeDependencyRootSpec {
        owner: "radroots_event_store",
        name: "getrandom",
        expected_version: Some("0.2.17"),
    },
    RuntimeDependencyRootSpec {
        owner: "radroots_event_codec",
        name: "nostr",
        expected_version: None,
    },
    RuntimeDependencyRootSpec {
        owner: "radroots_event_store",
        name: "serde_json",
        expected_version: None,
    },
    RuntimeDependencyRootSpec {
        owner: "radroots_event_store",
        name: "sha2",
        expected_version: None,
    },
    RuntimeDependencyRootSpec {
        owner: "radroots_event_store",
        name: "sqlx",
        expected_version: Some("0.9.0"),
    },
    RuntimeDependencyRootSpec {
        owner: "radroots_event",
        name: "jiff-tzdb",
        expected_version: None,
    },
    RuntimeDependencyRootSpec {
        owner: "radroots_event",
        name: "unicode-general-category",
        expected_version: None,
    },
    RuntimeDependencyRootSpec {
        owner: "radroots_event",
        name: "url",
        expected_version: None,
    },
    RuntimeDependencyRootSpec {
        owner: "radroots_event_store",
        name: "tokio",
        expected_version: Some("1.50.0"),
    },
];

#[derive(Clone, Copy)]
struct CargoPackageFeatureSpec {
    // The reconciliation manifest predates the Cargo package-name migration and
    // records the Rust crate identifier. Keep that immutable identifier separate
    // from the package name validated in the current Cargo manifest.
    package: &'static str,
    cargo_package_name: &'static str,
    manifest_path: &'static str,
    default_features_enabled: bool,
    selected_features: &'static [&'static str],
    relevant_feature_definitions: &'static [&'static str],
}

const CARGO_PACKAGE_FEATURE_SPECS: &[CargoPackageFeatureSpec] = &[
    CargoPackageFeatureSpec {
        package: "radroots_core",
        cargo_package_name: "radroots_core",
        manifest_path: CORE_CARGO_MANIFEST_RELATIVE,
        default_features_enabled: false,
        selected_features: &["serde", "std"],
        relevant_feature_definitions: &["default", "serde", "std"],
    },
    CargoPackageFeatureSpec {
        package: "radroots_event_store",
        cargo_package_name: "radroots_event_store",
        manifest_path: EVENT_STORE_CARGO_MANIFEST_RELATIVE,
        default_features_enabled: true,
        selected_features: &["runtime-tokio", "sqlite"],
        relevant_feature_definitions: &["default", "runtime-tokio", "sqlite"],
    },
    CargoPackageFeatureSpec {
        package: "radroots_event_codec",
        cargo_package_name: "radroots_event_codec",
        manifest_path: EVENT_CODEC_CARGO_MANIFEST_RELATIVE,
        default_features_enabled: false,
        selected_features: &["nostr", "serde", "serde_json", "std"],
        relevant_feature_definitions: &["nostr", "serde", "serde_json", "std"],
    },
    CargoPackageFeatureSpec {
        package: "radroots_event",
        cargo_package_name: "radroots_event",
        manifest_path: EVENT_CARGO_MANIFEST_RELATIVE,
        default_features_enabled: false,
        selected_features: &["serde", "std"],
        relevant_feature_definitions: &["serde", "std"],
    },
    CargoPackageFeatureSpec {
        package: "radroots_blossom",
        cargo_package_name: "radroots_blossom",
        manifest_path: BLOSSOM_CARGO_MANIFEST_RELATIVE,
        default_features_enabled: false,
        selected_features: &["std"],
        relevant_feature_definitions: &["std"],
    },
];

#[derive(Clone, Copy)]
struct CargoDependencyFeatureSpec {
    name: &'static str,
}

const EVENT_STORE_DEPENDENCY_FEATURE_SPECS: &[CargoDependencyFeatureSpec] = &[
    CargoDependencyFeatureSpec { name: "getrandom" },
    CargoDependencyFeatureSpec {
        name: "radroots_event",
    },
    CargoDependencyFeatureSpec {
        name: "radroots_event_codec",
    },
    CargoDependencyFeatureSpec { name: "sqlx" },
];

#[derive(Clone, Copy)]
struct EntryPointSpec {
    role: &'static str,
    rust_path: &'static str,
    source_path: &'static str,
    callable: CallableSpec,
}

#[derive(Clone, Copy)]
enum CallableSpec {
    Free {
        module_path: &'static [&'static str],
        name: &'static str,
        visibility: RouteVisibility,
    },
    Associated {
        owner: &'static str,
        name: &'static str,
        visibility: RouteVisibility,
    },
}

const ENTRY_POINT_SPECS: &[EntryPointSpec] = &[
    EntryPointSpec {
        role: "event_head_candidate",
        rust_path: "radroots_event::event_head::v1::event_head_candidate_for_nip01_event_v1",
        source_path: "crates/event/src/event_head/v1.rs",
        callable: CallableSpec::Free {
            module_path: &[],
            name: "event_head_candidate_for_nip01_event_v1",
            visibility: RouteVisibility::Public,
        },
    },
    EntryPointSpec {
        role: "event_head_selection",
        rust_path: "radroots_event::event_head::v1::select_event_head_v1",
        source_path: "crates/event/src/event_head/v1.rs",
        callable: CallableSpec::Free {
            module_path: &[],
            name: "select_event_head_v1",
            visibility: RouteVisibility::Public,
        },
    },
    EntryPointSpec {
        role: "event_contract_lookup",
        rust_path: "radroots_event::contract::registry_v7::event_contract_registry_v7",
        source_path: "crates/event/src/contract/registry_v7.rs",
        callable: CallableSpec::Free {
            module_path: &[],
            name: "event_contract_registry_v7",
            visibility: RouteVisibility::Public,
        },
    },
    EntryPointSpec {
        role: "event_contract_validation",
        rust_path: "radroots_event::contract::registry_v7::validate_event_contract_registry_v7",
        source_path: "crates/event/src/contract/registry_v7.rs",
        callable: CallableSpec::Free {
            module_path: &[],
            name: "validate_event_contract_registry_v7",
            visibility: RouteVisibility::Public,
        },
    },
    EntryPointSpec {
        role: "event_verification",
        rust_path: "radroots_event_codec::verification::v1::verify_nip01_event_v1",
        source_path: "crates/event_codec/src/verification/v1.rs",
        callable: CallableSpec::Free {
            module_path: &[],
            name: "verify_nip01_event_v1",
            visibility: RouteVisibility::Public,
        },
    },
    EntryPointSpec {
        role: "event_admission",
        rust_path: "radroots_event_codec::admission::registry_v7::admit_verified_event_registry_v7",
        source_path: "crates/event_codec/src/admission/registry_v7.rs",
        callable: CallableSpec::Free {
            module_path: &[],
            name: "admit_verified_event_registry_v7",
            visibility: RouteVisibility::Public,
        },
    },
    EntryPointSpec {
        role: "nip09_deletion_projection",
        rust_path: "radroots_event_codec::deletion::reconciliation_v1::inbound::project_verified_nip09_deletion_request_event_v1",
        source_path: "crates/event_codec/src/deletion/reconciliation_v1.rs",
        callable: CallableSpec::Free {
            module_path: &["inbound"],
            name: "project_verified_nip09_deletion_request_event_v1",
            visibility: RouteVisibility::Public,
        },
    },
    EntryPointSpec {
        role: "nip09_deletion_admission",
        rust_path: "radroots_event_codec::deletion::reconciliation_v1::admission::admit_verified_nip09_deletion_request_event_v1",
        source_path: "crates/event_codec/src/deletion/reconciliation_v1.rs",
        callable: CallableSpec::Free {
            module_path: &["admission"],
            name: "admit_verified_nip09_deletion_request_event_v1",
            visibility: RouteVisibility::Public,
        },
    },
    EntryPointSpec {
        role: "nip09_suppression_evaluation",
        rust_path: "radroots_event_codec::deletion::reconciliation_v1::evaluator::evaluate_nip09_suppression_v1",
        source_path: "crates/event_codec/src/deletion/reconciliation_v1.rs",
        callable: CallableSpec::Free {
            module_path: &["evaluator"],
            name: "evaluate_nip09_suppression_v1",
            visibility: RouteVisibility::Public,
        },
    },
    EntryPointSpec {
        role: "nip09_suppression_evaluation_borrowed_requests",
        rust_path: "radroots_event_codec::deletion::reconciliation_v1::evaluator::evaluate_nip09_suppression_from_borrowed_requests_v1",
        source_path: "crates/event_codec/src/deletion/reconciliation_v1.rs",
        callable: CallableSpec::Free {
            module_path: &["evaluator"],
            name: "evaluate_nip09_suppression_from_borrowed_requests_v1",
            visibility: RouteVisibility::Public,
        },
    },
    EntryPointSpec {
        role: "raw_signed_event_ingest",
        rust_path: "radroots_event_store::model::reconciliation_v1::RadrootsEventIngest::from_signed_event_reconciliation_v1",
        source_path: "crates/event_store/src/model/ingest_reconciliation_v1.rs",
        callable: CallableSpec::Associated {
            owner: "RadrootsEventIngest",
            name: "from_signed_event_reconciliation_v1",
            visibility: RouteVisibility::Crate,
        },
    },
    EntryPointSpec {
        role: "raw_json_event_ingest",
        rust_path: "radroots_event_store::model::reconciliation_v1::RadrootsEventIngest::from_raw_json_reconciliation_v1",
        source_path: "crates/event_store/src/model/ingest_reconciliation_v1.rs",
        callable: CallableSpec::Associated {
            owner: "RadrootsEventIngest",
            name: "from_raw_json_reconciliation_v1",
            visibility: RouteVisibility::Crate,
        },
    },
    EntryPointSpec {
        role: "reconciliation_hook",
        rust_path: "radroots_event_store::nip09::reconciliation_v1::apply_reconciliation_hook",
        source_path: "crates/event_store/src/nip09/reconciliation_v1.rs",
        callable: CallableSpec::Free {
            module_path: &[],
            name: "apply_reconciliation_hook",
            visibility: RouteVisibility::Crate,
        },
    },
    EntryPointSpec {
        role: "reconciliation_state_validation",
        rust_path: "radroots_event_store::nip09::reconciliation_v1::validate_active_hook_state_fast",
        source_path: "crates/event_store/src/nip09/reconciliation_v1.rs",
        callable: CallableSpec::Free {
            module_path: &[],
            name: "validate_active_hook_state_fast",
            visibility: RouteVisibility::Crate,
        },
    },
];

#[derive(Clone, Copy)]
struct SemanticDependencySpec {
    id: &'static str,
    canonical_path: &'static str,
    mirror_path: Option<&'static str>,
    executors: &'static [&'static str],
}

const SEMANTIC_DEPENDENCY_SPECS: &[SemanticDependencySpec] = &[
    SemanticDependencySpec {
        id: "profile_verified_event_v1",
        canonical_path: "contracts/conformance/vectors/profile/verified_event.v1.json",
        mirror_path: Some("crates/event_codec/tests/fixtures/profile_verified_event.v1.json"),
        executors: &[
            "radroots_event_codec::profile::inbound::registry_v7::parse_inbound_profile_metadata_registry_v7",
        ],
    },
    SemanticDependencySpec {
        id: "nip01_wire_v1",
        canonical_path: "contracts/conformance/vectors/event/nip01_wire.v1.json",
        mirror_path: None,
        executors: &[
            "radroots_event::wire::v1::RadrootsNip01EventWire::parse_json",
            "radroots_event::wire::v1::compute_canonical_nip01_event_id_v1",
        ],
    },
    SemanticDependencySpec {
        id: "verified_admission_v1",
        canonical_path: "contracts/conformance/vectors/event/verified_admission.v1.json",
        mirror_path: Some("crates/event_codec/tests/fixtures/verified_admission.v1.json"),
        executors: &[
            "radroots_event_codec::verification::v1::verify_nip01_event_v1",
            "radroots_event_codec::admission::registry_v7::admit_verified_event_registry_v7",
        ],
    },
    SemanticDependencySpec {
        id: "deletion_verified_profile_v1",
        canonical_path: "contracts/conformance/vectors/deletion/verified_profile.v1.json",
        mirror_path: Some("crates/event_codec/tests/fixtures/deletion_verified_profile.v1.json"),
        executors: &[
            "radroots_event_codec::deletion::reconciliation_v1::inbound::project_verified_nip09_deletion_request_event_v1",
            "radroots_event_codec::deletion::reconciliation_v1::admission::admit_verified_nip09_deletion_request_event_v1",
        ],
    },
    SemanticDependencySpec {
        id: "deletion_suppression_v1",
        canonical_path: "contracts/conformance/vectors/deletion/suppression.v1.json",
        mirror_path: Some("crates/event_codec/tests/fixtures/deletion_suppression.v1.json"),
        executors: &[
            "radroots_event_codec::deletion::reconciliation_v1::evaluator::evaluate_nip09_suppression_v1",
            "radroots_event_codec::deletion::reconciliation_v1::evaluator::evaluate_nip09_suppression_from_borrowed_requests_v1",
        ],
    },
    SemanticDependencySpec {
        id: "post_verified_profiles_v1",
        canonical_path: "contracts/conformance/vectors/post/verified_profiles.v1.json",
        mirror_path: Some("crates/event_codec/tests/fixtures/post_verified_profiles.v1.json"),
        executors: &[
            "radroots_event_codec::post::inbound::registry_v7::project_verified_post_event_registry_v7",
        ],
    },
    SemanticDependencySpec {
        id: "comment_verified_profile_v1",
        canonical_path: "contracts/conformance/vectors/comment/verified_profile.v1.json",
        mirror_path: Some("crates/event_codec/tests/fixtures/comment_verified_profile.v1.json"),
        executors: &[
            "radroots_event_codec::comment::inbound::registry_v7::project_verified_nip22_comment_event_registry_v7",
        ],
    },
    SemanticDependencySpec {
        id: "food_availability_profile_v1",
        canonical_path: "contracts/conformance/vectors/food_availability/profile.v1.json",
        mirror_path: Some("crates/event_codec/tests/fixtures/food_availability_profile.v1.json"),
        executors: &[
            "radroots_event_codec::food_availability::inbound::registry_v7::project_verified_food_availability_event_registry_v7",
        ],
    },
];

#[derive(Clone, Copy)]
struct FrozenSourceSpec {
    role: &'static str,
    path: &'static str,
}

const FROZEN_SOURCE_SPECS: &[FrozenSourceSpec] = &[
    FrozenSourceSpec {
        role: "event_contract_registry_v7_facade",
        path: "crates/event/src/contract.rs",
    },
    FrozenSourceSpec {
        role: "event_contract_registry_v7",
        path: "crates/event/src/contract/registry_v7.rs",
    },
    FrozenSourceSpec {
        role: "event_head_v1_facade",
        path: "crates/event/src/event_head.rs",
    },
    FrozenSourceSpec {
        role: "event_head_v1",
        path: "crates/event/src/event_head/v1.rs",
    },
    FrozenSourceSpec {
        role: "event_wire_v1_facade",
        path: "crates/event/src/wire.rs",
    },
    FrozenSourceSpec {
        role: "event_wire_v1",
        path: "crates/event/src/wire/v1.rs",
    },
    FrozenSourceSpec {
        role: "event_envelope_semantics",
        path: "crates/event/src/envelope.rs",
    },
    FrozenSourceSpec {
        role: "event_identifier_semantics",
        path: "crates/event/src/ids.rs",
    },
    FrozenSourceSpec {
        role: "event_trade_content_and_conversion_semantics",
        path: "crates/event/src/trade.rs",
    },
    FrozenSourceSpec {
        role: "event_kind_constants",
        path: "crates/event/src/kinds.rs",
    },
    FrozenSourceSpec {
        role: "event_tag_constants",
        path: "crates/event/src/tags.rs",
    },
    FrozenSourceSpec {
        role: "event_signed_draft_semantics",
        path: "crates/event/src/draft.rs",
    },
    FrozenSourceSpec {
        role: "calendar_contract_primitives",
        path: "crates/event/src/calendar.rs",
    },
    FrozenSourceSpec {
        role: "classified_listing_partition_semantics",
        path: "crates/event/src/classified_listing.rs",
    },
    FrozenSourceSpec {
        role: "profile_contract_primitives",
        path: "crates/event/src/profile.rs",
    },
    FrozenSourceSpec {
        role: "post_contract_primitives",
        path: "crates/event/src/post.rs",
    },
    FrozenSourceSpec {
        role: "comment_contract_primitives",
        path: "crates/event/src/comment.rs",
    },
    FrozenSourceSpec {
        role: "food_availability_contract_primitives",
        path: "crates/event/src/food_availability.rs",
    },
    FrozenSourceSpec {
        role: "deletion_contract_primitives",
        path: "crates/event/src/deletion.rs",
    },
    FrozenSourceSpec {
        role: "relay_hint_parsing_semantics",
        path: "crates/event/src/relay_hint.rs",
    },
    FrozenSourceSpec {
        role: "event_media_typestate_semantics",
        path: "crates/event/src/media.rs",
    },
    FrozenSourceSpec {
        role: "event_social_metadata_primitives",
        path: "crates/event/src/social.rs",
    },
    FrozenSourceSpec {
        role: "blossom_hash_path_semantics",
        path: "crates/blossom/src/hash.rs",
    },
    FrozenSourceSpec {
        role: "blossom_blob_url_semantics",
        path: "crates/blossom/src/url.rs",
    },
    FrozenSourceSpec {
        role: "event_verification_v1_facade",
        path: "crates/event_codec/src/verification.rs",
    },
    FrozenSourceSpec {
        role: "event_verification_v1",
        path: "crates/event_codec/src/verification/v1.rs",
    },
    FrozenSourceSpec {
        role: "registry_v7_admission",
        path: "crates/event_codec/src/admission/registry_v7.rs",
    },
    FrozenSourceSpec {
        role: "registry_v7_profile_projection",
        path: "crates/event_codec/src/profile/inbound/registry_v7.rs",
    },
    FrozenSourceSpec {
        role: "registry_v7_profile_projection_facade",
        path: "crates/event_codec/src/profile/inbound.rs",
    },
    FrozenSourceSpec {
        role: "registry_v7_post_projection",
        path: "crates/event_codec/src/post/inbound/registry_v7.rs",
    },
    FrozenSourceSpec {
        role: "registry_v7_post_projection_facade",
        path: "crates/event_codec/src/post/inbound.rs",
    },
    FrozenSourceSpec {
        role: "registry_v7_reply_projection",
        path: "crates/event_codec/src/reply/inbound/registry_v7.rs",
    },
    FrozenSourceSpec {
        role: "registry_v7_reply_projection_facade",
        path: "crates/event_codec/src/reply/inbound.rs",
    },
    FrozenSourceSpec {
        role: "registry_v7_comment_projection",
        path: "crates/event_codec/src/comment/inbound/registry_v7.rs",
    },
    FrozenSourceSpec {
        role: "registry_v7_comment_projection_facade",
        path: "crates/event_codec/src/comment/inbound.rs",
    },
    FrozenSourceSpec {
        role: "nip09_deletion_module_facade",
        path: "crates/event_codec/src/deletion/mod.rs",
    },
    FrozenSourceSpec {
        role: "nip09_deletion_reconciliation_v1_module",
        path: "crates/event_codec/src/deletion/reconciliation_v1.rs",
    },
    FrozenSourceSpec {
        role: "registry_v7_food_availability_projection",
        path: "crates/event_codec/src/food_availability/inbound/registry_v7.rs",
    },
    FrozenSourceSpec {
        role: "registry_v7_food_availability_projection_facade",
        path: "crates/event_codec/src/food_availability/inbound.rs",
    },
    FrozenSourceSpec {
        role: "event_store_reconciliation_model_v1",
        path: "crates/event_store/src/model/reconciliation_v1.rs",
    },
    FrozenSourceSpec {
        role: "event_store_typed_error_semantics",
        path: "crates/event_store/src/error.rs",
    },
    FrozenSourceSpec {
        role: "event_store_ingest_reconciliation_v1",
        path: "crates/event_store/src/model/ingest_reconciliation_v1.rs",
    },
    FrozenSourceSpec {
        role: "event_store_nip09_reconciliation_v1",
        path: "crates/event_store/src/nip09/reconciliation_v1.rs",
    },
    FrozenSourceSpec {
        role: "event_store_protocol_reconciliation_v1",
        path: EVENT_STORE_PROTOCOL_RECONCILIATION_SOURCE_RELATIVE,
    },
    FrozenSourceSpec {
        role: "event_store_protocol_storage_v1",
        path: EVENT_STORE_PROTOCOL_STORAGE_SOURCE_RELATIVE,
    },
];

#[derive(Clone, Copy, Eq, PartialEq)]
enum RouteVisibility {
    Inherited,
    Public,
    Crate,
}

impl RouteVisibility {
    const fn label(self) -> &'static str {
        match self {
            Self::Inherited => "private",
            Self::Public => "public",
            Self::Crate => "crate",
        }
    }
}

#[derive(Clone, Copy)]
struct RustItemWitnessRootSpec {
    role: &'static str,
    path: &'static str,
    callable: RustWitnessCallable,
    binding: RustWitnessBinding,
    required_call_sequence: &'static [&'static str],
}

#[derive(Clone, Copy)]
enum RustWitnessCallable {
    Free {
        name: &'static str,
    },
    Associated {
        owner: &'static str,
        name: &'static str,
    },
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum RustWitnessBinding {
    SelfAst,
    AstClosure,
}

const RUST_ITEM_WITNESS_ROOT_SPECS: &[RustItemWitnessRootSpec] = &[
    RustItemWitnessRootSpec {
        role: "event_store_open_memory_migration_route_v1",
        path: "crates/event_store/src/store.rs",
        callable: RustWitnessCallable::Associated {
            owner: "RadrootsEventStore",
            name: "open_memory",
        },
        binding: RustWitnessBinding::SelfAst,
        required_call_sequence: &["fn:migrate_event_store_schema"],
    },
    RustItemWitnessRootSpec {
        role: "event_store_open_file_migration_route_v1",
        path: "crates/event_store/src/store.rs",
        callable: RustWitnessCallable::Associated {
            owner: "RadrootsEventStore",
            name: "open_file",
        },
        binding: RustWitnessBinding::SelfAst,
        required_call_sequence: &["fn:migrate_event_store_schema"],
    },
    RustItemWitnessRootSpec {
        role: "event_store_open_pool_migration_route_v1",
        path: "crates/event_store/src/store.rs",
        callable: RustWitnessCallable::Associated {
            owner: "RadrootsEventStore",
            name: "open_pool",
        },
        binding: RustWitnessBinding::SelfAst,
        required_call_sequence: &["fn:migrate_event_store_schema"],
    },
    RustItemWitnessRootSpec {
        role: "event_store_schema_status_inspection_route_v1",
        path: "crates/event_store/src/store.rs",
        callable: RustWitnessCallable::Associated {
            owner: "RadrootsEventStore",
            name: "schema_status",
        },
        binding: RustWitnessBinding::SelfAst,
        required_call_sequence: &["fn:inspect_event_store_schema_status"],
    },
    RustItemWitnessRootSpec {
        role: "event_store_explicit_migration_route_v1",
        path: "crates/event_store/src/store.rs",
        callable: RustWitnessCallable::Associated {
            owner: "RadrootsEventStore",
            name: "migrate_to_current_schema",
        },
        binding: RustWitnessBinding::SelfAst,
        required_call_sequence: &["fn:migrate_event_store_schema"],
    },
    RustItemWitnessRootSpec {
        role: "event_store_owned_ingest_transaction_factory_v1",
        path: "crates/event_store/src/store.rs",
        callable: RustWitnessCallable::Associated {
            owner: "RadrootsEventStore",
            name: "begin_write_transaction",
        },
        binding: RustWitnessBinding::SelfAst,
        required_call_sequence: &["method:begin_with"],
    },
    RustItemWitnessRootSpec {
        role: "event_store_owned_ingest_route_v1",
        path: "crates/event_store/src/store.rs",
        callable: RustWitnessCallable::Associated {
            owner: "RadrootsEventStore",
            name: "ingest_event",
        },
        binding: RustWitnessBinding::SelfAst,
        required_call_sequence: &[
            "method:begin_write_transaction",
            "fn:ingest_event_in_transaction",
            "method:commit",
            "method:rollback",
            "fn:preserve_ingest_primary_failure",
        ],
    },
    RustItemWitnessRootSpec {
        role: "event_store_borrowed_transaction_ingest_route_v1",
        path: "crates/event_store/src/store.rs",
        callable: RustWitnessCallable::Associated {
            owner: "RadrootsEventStore",
            name: "ingest_event_in_transaction",
        },
        binding: RustWitnessBinding::SelfAst,
        required_call_sequence: &[
            "fn:sqlx::Acquire::begin",
            "fn:ingest_event_in_transaction",
            "method:commit",
            "method:rollback",
            "fn:preserve_ingest_primary_failure",
        ],
    },
    RustItemWitnessRootSpec {
        role: "event_store_extensible_ingest_route_v1",
        path: "crates/event_store/src/store.rs",
        callable: RustWitnessCallable::Free {
            name: "ingest_event_in_transaction",
        },
        binding: RustWitnessBinding::SelfAst,
        required_call_sequence: &[
            "fn:crate::schema::validate_event_store_temp_schema",
            "fn:ingest_event_protocol_reconciliation_v1",
            "fn:PostCoreExtensionCapabilities::new",
            "fn:dispatch_post_core_extensions",
            "fn:validate_protocol_post_extensions",
        ],
    },
];

#[derive(Clone, Copy)]
struct ModuleRouteSpec {
    visibility: RouteVisibility,
    name: &'static str,
}

#[derive(Clone, Copy)]
struct UseRouteSpec {
    visibility: RouteVisibility,
    path: &'static str,
}

#[derive(Clone, Copy)]
struct SourceRouteWitnessSpec {
    role: &'static str,
    path: &'static str,
    modules: &'static [ModuleRouteSpec],
    uses: &'static [UseRouteSpec],
}

const SOURCE_ROUTE_WITNESS_SPECS: &[SourceRouteWitnessSpec] = &[
    SourceRouteWitnessSpec {
        role: "event_crate_module_routes",
        path: "crates/event/src/lib.rs",
        modules: &[
            ModuleRouteSpec {
                visibility: RouteVisibility::Public,
                name: "calendar",
            },
            ModuleRouteSpec {
                visibility: RouteVisibility::Public,
                name: "classified_listing",
            },
            ModuleRouteSpec {
                visibility: RouteVisibility::Public,
                name: "comment",
            },
            ModuleRouteSpec {
                visibility: RouteVisibility::Public,
                name: "contract",
            },
            ModuleRouteSpec {
                visibility: RouteVisibility::Public,
                name: "deletion",
            },
            ModuleRouteSpec {
                visibility: RouteVisibility::Public,
                name: "draft",
            },
            ModuleRouteSpec {
                visibility: RouteVisibility::Public,
                name: "envelope",
            },
            ModuleRouteSpec {
                visibility: RouteVisibility::Public,
                name: "event_head",
            },
            ModuleRouteSpec {
                visibility: RouteVisibility::Public,
                name: "food_availability",
            },
            ModuleRouteSpec {
                visibility: RouteVisibility::Public,
                name: "ids",
            },
            ModuleRouteSpec {
                visibility: RouteVisibility::Public,
                name: "kinds",
            },
            ModuleRouteSpec {
                visibility: RouteVisibility::Public,
                name: "media",
            },
            ModuleRouteSpec {
                visibility: RouteVisibility::Public,
                name: "post",
            },
            ModuleRouteSpec {
                visibility: RouteVisibility::Public,
                name: "profile",
            },
            ModuleRouteSpec {
                visibility: RouteVisibility::Public,
                name: "relay_hint",
            },
            ModuleRouteSpec {
                visibility: RouteVisibility::Public,
                name: "social",
            },
            ModuleRouteSpec {
                visibility: RouteVisibility::Public,
                name: "tags",
            },
            ModuleRouteSpec {
                visibility: RouteVisibility::Public,
                name: "wire",
            },
        ],
        uses: &[],
    },
    SourceRouteWitnessSpec {
        role: "event_contract_registry_v7_route",
        path: "crates/event/src/contract.rs",
        modules: &[ModuleRouteSpec {
            visibility: RouteVisibility::Public,
            name: "registry_v7",
        }],
        uses: &[UseRouteSpec {
            visibility: RouteVisibility::Public,
            path: "registry_v7::*",
        }],
    },
    SourceRouteWitnessSpec {
        role: "blossom_crate_module_routes",
        path: "crates/blossom/src/lib.rs",
        modules: &[
            ModuleRouteSpec {
                visibility: RouteVisibility::Public,
                name: "hash",
            },
            ModuleRouteSpec {
                visibility: RouteVisibility::Public,
                name: "url",
            },
        ],
        uses: &[],
    },
    SourceRouteWitnessSpec {
        role: "event_head_v1_route",
        path: "crates/event/src/event_head.rs",
        modules: &[ModuleRouteSpec {
            visibility: RouteVisibility::Public,
            name: "v1",
        }],
        uses: &[UseRouteSpec {
            visibility: RouteVisibility::Public,
            path: "v1::*",
        }],
    },
    SourceRouteWitnessSpec {
        role: "event_wire_v1_route",
        path: "crates/event/src/wire.rs",
        modules: &[ModuleRouteSpec {
            visibility: RouteVisibility::Public,
            name: "v1",
        }],
        uses: &[UseRouteSpec {
            visibility: RouteVisibility::Public,
            path: "v1::*",
        }],
    },
    SourceRouteWitnessSpec {
        role: "event_codec_crate_module_routes",
        path: "crates/event_codec/src/lib.rs",
        modules: &[
            ModuleRouteSpec {
                visibility: RouteVisibility::Public,
                name: "admission",
            },
            ModuleRouteSpec {
                visibility: RouteVisibility::Public,
                name: "comment",
            },
            ModuleRouteSpec {
                visibility: RouteVisibility::Public,
                name: "deletion",
            },
            ModuleRouteSpec {
                visibility: RouteVisibility::Public,
                name: "food_availability",
            },
            ModuleRouteSpec {
                visibility: RouteVisibility::Public,
                name: "post",
            },
            ModuleRouteSpec {
                visibility: RouteVisibility::Public,
                name: "profile",
            },
            ModuleRouteSpec {
                visibility: RouteVisibility::Public,
                name: "reply",
            },
            ModuleRouteSpec {
                visibility: RouteVisibility::Public,
                name: "verification",
            },
        ],
        uses: &[],
    },
    SourceRouteWitnessSpec {
        role: "event_codec_verification_v1_route",
        path: "crates/event_codec/src/verification.rs",
        modules: &[ModuleRouteSpec {
            visibility: RouteVisibility::Public,
            name: "v1",
        }],
        uses: &[UseRouteSpec {
            visibility: RouteVisibility::Public,
            path: "v1::*",
        }],
    },
    SourceRouteWitnessSpec {
        role: "event_codec_registry_v7_admission_route",
        path: "crates/event_codec/src/admission.rs",
        modules: &[ModuleRouteSpec {
            visibility: RouteVisibility::Public,
            name: "registry_v7",
        }],
        uses: &[
            UseRouteSpec {
                visibility: RouteVisibility::Public,
                path: "registry_v7::RadrootsRegistryV7AdmissionDecision",
            },
            UseRouteSpec {
                visibility: RouteVisibility::Public,
                path: "registry_v7::admit_verified_event_registry_v7",
            },
        ],
    },
    SourceRouteWitnessSpec {
        role: "event_codec_profile_registry_v7_route",
        path: "crates/event_codec/src/profile/inbound.rs",
        modules: &[ModuleRouteSpec {
            visibility: RouteVisibility::Public,
            name: "registry_v7",
        }],
        uses: &[UseRouteSpec {
            visibility: RouteVisibility::Public,
            path: "registry_v7::*",
        }],
    },
    SourceRouteWitnessSpec {
        role: "event_codec_profile_inbound_route",
        path: "crates/event_codec/src/profile/mod.rs",
        modules: &[ModuleRouteSpec {
            visibility: RouteVisibility::Public,
            name: "inbound",
        }],
        uses: &[],
    },
    SourceRouteWitnessSpec {
        role: "event_codec_post_registry_v7_route",
        path: "crates/event_codec/src/post/inbound.rs",
        modules: &[ModuleRouteSpec {
            visibility: RouteVisibility::Public,
            name: "registry_v7",
        }],
        uses: &[UseRouteSpec {
            visibility: RouteVisibility::Public,
            path: "registry_v7::*",
        }],
    },
    SourceRouteWitnessSpec {
        role: "event_codec_post_inbound_route",
        path: "crates/event_codec/src/post/mod.rs",
        modules: &[ModuleRouteSpec {
            visibility: RouteVisibility::Public,
            name: "inbound",
        }],
        uses: &[],
    },
    SourceRouteWitnessSpec {
        role: "event_codec_reply_registry_v7_route",
        path: "crates/event_codec/src/reply/inbound.rs",
        modules: &[ModuleRouteSpec {
            visibility: RouteVisibility::Public,
            name: "registry_v7",
        }],
        uses: &[UseRouteSpec {
            visibility: RouteVisibility::Public,
            path: "registry_v7::*",
        }],
    },
    SourceRouteWitnessSpec {
        role: "event_codec_reply_inbound_route",
        path: "crates/event_codec/src/reply/mod.rs",
        modules: &[ModuleRouteSpec {
            visibility: RouteVisibility::Public,
            name: "inbound",
        }],
        uses: &[],
    },
    SourceRouteWitnessSpec {
        role: "event_codec_comment_registry_v7_route",
        path: "crates/event_codec/src/comment/inbound.rs",
        modules: &[ModuleRouteSpec {
            visibility: RouteVisibility::Public,
            name: "registry_v7",
        }],
        uses: &[UseRouteSpec {
            visibility: RouteVisibility::Public,
            path: "registry_v7::*",
        }],
    },
    SourceRouteWitnessSpec {
        role: "event_codec_comment_inbound_route",
        path: "crates/event_codec/src/comment/mod.rs",
        modules: &[ModuleRouteSpec {
            visibility: RouteVisibility::Public,
            name: "inbound",
        }],
        uses: &[],
    },
    SourceRouteWitnessSpec {
        role: "event_codec_food_availability_registry_v7_route",
        path: "crates/event_codec/src/food_availability/inbound.rs",
        modules: &[ModuleRouteSpec {
            visibility: RouteVisibility::Public,
            name: "registry_v7",
        }],
        uses: &[UseRouteSpec {
            visibility: RouteVisibility::Public,
            path: "registry_v7::*",
        }],
    },
    SourceRouteWitnessSpec {
        role: "event_codec_food_availability_inbound_route",
        path: "crates/event_codec/src/food_availability/mod.rs",
        modules: &[ModuleRouteSpec {
            visibility: RouteVisibility::Public,
            name: "inbound",
        }],
        uses: &[],
    },
    SourceRouteWitnessSpec {
        role: "event_codec_deletion_reconciliation_v1_route",
        path: "crates/event_codec/src/deletion/mod.rs",
        modules: &[ModuleRouteSpec {
            visibility: RouteVisibility::Public,
            name: "reconciliation_v1",
        }],
        uses: &[],
    },
    SourceRouteWitnessSpec {
        role: "event_store_crate_module_routes",
        path: "crates/event_store/src/lib.rs",
        modules: &[
            ModuleRouteSpec {
                visibility: RouteVisibility::Inherited,
                name: "error",
            },
            ModuleRouteSpec {
                visibility: RouteVisibility::Inherited,
                name: "generated",
            },
            ModuleRouteSpec {
                visibility: RouteVisibility::Inherited,
                name: "migrations",
            },
            ModuleRouteSpec {
                visibility: RouteVisibility::Inherited,
                name: "model",
            },
            ModuleRouteSpec {
                visibility: RouteVisibility::Inherited,
                name: "nip09",
            },
            ModuleRouteSpec {
                visibility: RouteVisibility::Inherited,
                name: "schema",
            },
            ModuleRouteSpec {
                visibility: RouteVisibility::Inherited,
                name: "store",
            },
        ],
        uses: &[
            UseRouteSpec {
                visibility: RouteVisibility::Public,
                path: "error::RadrootsEventStoreError",
            },
            UseRouteSpec {
                visibility: RouteVisibility::Public,
                path: "error::RadrootsEventStoreReconciliationResource",
            },
            UseRouteSpec {
                visibility: RouteVisibility::Public,
                path: "migrations::RADROOTS_EVENT_STORE_SCHEMA_VERSION_CURRENT",
            },
            UseRouteSpec {
                visibility: RouteVisibility::Public,
                path: "migrations::RADROOTS_EVENT_STORE_SCHEMA_VERSION_MIN",
            },
            UseRouteSpec {
                visibility: RouteVisibility::Public,
                path: "model::RadrootsEventAdmissionStatus",
            },
            UseRouteSpec {
                visibility: RouteVisibility::Public,
                path: "model::RadrootsEventIngest",
            },
            UseRouteSpec {
                visibility: RouteVisibility::Public,
                path: "model::RadrootsEventIngestReceipt",
            },
            UseRouteSpec {
                visibility: RouteVisibility::Public,
                path: "model::RadrootsEventPersistence",
            },
            UseRouteSpec {
                visibility: RouteVisibility::Public,
                path: "model::RadrootsEventStoreSourceGeneration",
            },
            UseRouteSpec {
                visibility: RouteVisibility::Public,
                path: "model::RadrootsEventStoreStatusSummary",
            },
            UseRouteSpec {
                visibility: RouteVisibility::Public,
                path: "model::RadrootsRawHeadDecision",
            },
            UseRouteSpec {
                visibility: RouteVisibility::Public,
                path: "model::RadrootsStoredRawEvent",
            },
            UseRouteSpec {
                visibility: RouteVisibility::Public,
                path: "model::RadrootsStoredRawEventHead",
            },
            UseRouteSpec {
                visibility: RouteVisibility::Public,
                path: "model::StoredEventClass",
            },
            UseRouteSpec {
                visibility: RouteVisibility::Public,
                path: "schema::RadrootsEventStoreSchemaStatus",
            },
            UseRouteSpec {
                visibility: RouteVisibility::Public,
                path: "schema::inspect_event_store_schema_status",
            },
            UseRouteSpec {
                visibility: RouteVisibility::Public,
                path: "store::RadrootsEventStore",
            },
            UseRouteSpec {
                visibility: RouteVisibility::Public,
                path: "store::inspect_event_store_status",
            },
            UseRouteSpec {
                visibility: RouteVisibility::Public,
                path: "store::RADROOTS_EVENT_STORE_QUERY_LIMIT_MAX",
            },
        ],
    },
    SourceRouteWitnessSpec {
        role: "event_store_generated_nip09_manifest_route",
        path: "crates/event_store/src/generated.rs",
        modules: &[ModuleRouteSpec {
            visibility: RouteVisibility::Crate,
            name: "nip09_reconciliation_manifest",
        }],
        uses: &[],
    },
    SourceRouteWitnessSpec {
        role: "event_store_reconciliation_model_routes",
        path: "crates/event_store/src/model.rs",
        modules: &[
            ModuleRouteSpec {
                visibility: RouteVisibility::Inherited,
                name: "ingest_reconciliation_v1",
            },
            ModuleRouteSpec {
                visibility: RouteVisibility::Crate,
                name: "reconciliation_v1",
            },
        ],
        uses: &[
            UseRouteSpec {
                visibility: RouteVisibility::Public,
                path: "reconciliation_v1::RadrootsEventAdmissionStatus",
            },
            UseRouteSpec {
                visibility: RouteVisibility::Public,
                path: "reconciliation_v1::RadrootsEventIngest",
            },
            UseRouteSpec {
                visibility: RouteVisibility::Public,
                path: "reconciliation_v1::RadrootsEventIngestReceipt",
            },
            UseRouteSpec {
                visibility: RouteVisibility::Public,
                path: "reconciliation_v1::RadrootsEventPersistence",
            },
            UseRouteSpec {
                visibility: RouteVisibility::Public,
                path: "reconciliation_v1::RadrootsEventStoreSourceGeneration",
            },
            UseRouteSpec {
                visibility: RouteVisibility::Public,
                path: "reconciliation_v1::RadrootsRawHeadDecision",
            },
            UseRouteSpec {
                visibility: RouteVisibility::Public,
                path: "reconciliation_v1::RadrootsStoredRawEvent",
            },
            UseRouteSpec {
                visibility: RouteVisibility::Public,
                path: "reconciliation_v1::RadrootsStoredRawEventHead",
            },
            UseRouteSpec {
                visibility: RouteVisibility::Public,
                path: "reconciliation_v1::StoredEventClass",
            },
        ],
    },
    SourceRouteWitnessSpec {
        role: "event_store_nip09_reconciliation_route",
        path: "crates/event_store/src/nip09.rs",
        modules: &[ModuleRouteSpec {
            visibility: RouteVisibility::Crate,
            name: "reconciliation_v1",
        }],
        uses: &[],
    },
    SourceRouteWitnessSpec {
        role: "event_store_protocol_module_routes_v1",
        path: "crates/event_store/src/store.rs",
        modules: &[
            ModuleRouteSpec {
                visibility: RouteVisibility::Inherited,
                name: "post_core_extension_capabilities",
            },
            ModuleRouteSpec {
                visibility: RouteVisibility::Inherited,
                name: "post_core_extension_dispatcher",
            },
            ModuleRouteSpec {
                visibility: RouteVisibility::Inherited,
                name: "post_core_extensions_v1",
            },
            ModuleRouteSpec {
                visibility: RouteVisibility::Inherited,
                name: "post_core_storage_v1",
            },
            ModuleRouteSpec {
                visibility: RouteVisibility::Inherited,
                name: "protocol_reconciliation_v1",
            },
            ModuleRouteSpec {
                visibility: RouteVisibility::Inherited,
                name: "protocol_storage_v1",
            },
        ],
        uses: &[
            UseRouteSpec {
                visibility: RouteVisibility::Inherited,
                path: "self::post_core_extension_capabilities::PostCoreExtensionCapabilities",
            },
            UseRouteSpec {
                visibility: RouteVisibility::Inherited,
                path: "self::post_core_extension_dispatcher::dispatch_post_core_extensions",
            },
            UseRouteSpec {
                visibility: RouteVisibility::Inherited,
                path: "self::protocol_reconciliation_v1::ingest_event_protocol_reconciliation_v1",
            },
            UseRouteSpec {
                visibility: RouteVisibility::Inherited,
                path: "self::protocol_reconciliation_v1::validate_protocol_post_extensions",
            },
            UseRouteSpec {
                visibility: RouteVisibility::Inherited,
                path: "self::protocol_storage_v1::RawHeadSnapshot",
            },
            UseRouteSpec {
                visibility: RouteVisibility::Inherited,
                path: "self::protocol_storage_v1::raw_head_coordinate_for_stored_event",
            },
            UseRouteSpec {
                visibility: RouteVisibility::Inherited,
                path: "self::protocol_storage_v1::raw_head_snapshot_in_transaction",
            },
            UseRouteSpec {
                visibility: RouteVisibility::Inherited,
                path: "self::protocol_storage_v1::stored_raw_event_from_row",
            },
            UseRouteSpec {
                visibility: RouteVisibility::Inherited,
                path: "crate::RadrootsEventStoreError",
            },
            UseRouteSpec {
                visibility: RouteVisibility::Inherited,
                path: "crate::model::RadrootsEventIngest",
            },
            UseRouteSpec {
                visibility: RouteVisibility::Inherited,
                path: "crate::model::RadrootsEventIngestReceipt",
            },
        ],
    },
];

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Nip09ReconciliationManifest {
    schema_version: u32,
    hook_id: String,
    manifest_schema: FileDescriptor,
    migration: MigrationDescriptor,
    profile: ReconciliationProfileDescriptor,
    cargo_feature_profile: CargoFeatureProfileDescriptor,
    entry_points: Vec<EntryPointDescriptor>,
    registry_inventory: FileDescriptor,
    semantic_dependencies: Vec<SemanticDependencyDescriptor>,
    runtime_dependency_policy: RuntimeDependencyPolicyDescriptor,
    runtime_dependencies: Vec<RuntimeDependencyDescriptor>,
    local_runtime_sources: Vec<LocalRuntimeSourceDescriptor>,
    frozen_sources: Vec<FrozenSourceDescriptor>,
    source_route_witnesses: Vec<SourceRouteWitnessDescriptor>,
    rust_item_witnesses: Vec<RustItemWitnessDescriptor>,
    rust_fragment_witnesses: Vec<RustFragmentWitnessDescriptor>,
    impl_resolution_witness: ImplResolutionWitnessDescriptor,
    post_core_sql_capability: PostCoreSqlCapabilityDescriptor,
    result_vector: MirroredFileDescriptor,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct MigrationDescriptor {
    version: u32,
    name: String,
    up_byte_length: u64,
    up_sha256: String,
    down_byte_length: u64,
    down_sha256: String,
    schema_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ReconciliationProfileDescriptor {
    reconciliation_version: u32,
    addressable_feed_version: u32,
    event_contract_registry_version: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct CargoFeatureProfileDescriptor {
    packages: Vec<CargoPackageFeatureDescriptor>,
    event_store_dependencies: Vec<CargoDependencyFeatureDescriptor>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct CargoPackageFeatureDescriptor {
    package: String,
    manifest_path: String,
    default_features_enabled: bool,
    selected_features: Vec<String>,
    feature_definitions: Vec<CargoFeatureDefinitionDescriptor>,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
struct CargoFeatureDefinitionDescriptor {
    name: String,
    enables: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
struct CargoDependencyFeatureDescriptor {
    name: String,
    default_features: bool,
    optional: bool,
    features: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct EntryPointDescriptor {
    role: String,
    rust_path: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct FileDescriptor {
    path: String,
    byte_length: u64,
    sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct SemanticDependencyDescriptor {
    id: String,
    canonical_path: String,
    mirror_path: Option<String>,
    byte_length: u64,
    sha256: String,
    executors: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct RuntimeDependencyPolicyDescriptor {
    algorithm: String,
    roots: Vec<RuntimeDependencyRootDescriptor>,
    exclusions: Vec<RuntimeDependencyExclusionDescriptor>,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
struct RuntimeDependencyRootDescriptor {
    owner: String,
    name: String,
    version: String,
    source: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct RuntimeDependencyExclusionDescriptor {
    owner: String,
    name: String,
    reason: String,
    bound_by: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
struct RuntimeDependencyIdentityDescriptor {
    name: String,
    version: String,
    source: String,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
struct RuntimeDependencyDescriptor {
    name: String,
    version: String,
    source: String,
    checksum: Option<String>,
    dependencies: Vec<RuntimeDependencyIdentityDescriptor>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct LocalRuntimeSourceDescriptor {
    package: String,
    version: String,
    path: String,
    patch_registry: String,
    patch_dependency: String,
    activation_route: Vec<String>,
    feature_definitions: Vec<CargoFeatureDefinitionDescriptor>,
    tree_algorithm: String,
    files: Vec<FileDescriptor>,
    tree_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct FrozenSourceDescriptor {
    role: String,
    path: String,
    hash_algorithm: String,
    canonical_byte_length: u64,
    sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct SourceRouteWitnessDescriptor {
    role: String,
    path: String,
    routes: Vec<String>,
    sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct RustItemWitnessDescriptor {
    role: String,
    path: String,
    item: String,
    root: bool,
    binding: String,
    local_call_sequence: Vec<String>,
    required_call_sequence: Vec<String>,
    ast_sha256: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct RustFragmentWitnessDescriptor {
    role: String,
    path: String,
    selector: String,
    ast_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ImplResolutionWitnessDescriptor {
    algorithm: String,
    roots: Vec<String>,
    protected_self_types: Vec<String>,
    impls: Vec<ImplResolutionItemDescriptor>,
    sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
struct ImplResolutionItemDescriptor {
    path: String,
    self_type: String,
    trait_path: Option<String>,
    member: Option<String>,
    impl_header_sha256: String,
    ast_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct PostCoreSqlCapabilityDescriptor {
    algorithm: String,
    capabilities_path: String,
    capability_type: String,
    capability_struct_ast_sha256: String,
    capability_constructor_ast_sha256: String,
    capability_v1_method_ast_sha256: String,
    dispatcher_path: String,
    dispatcher_root: String,
    dispatcher_signature_sha256: String,
    dispatcher_v1_prefix_sha256: String,
    extension_path: String,
    extension_ast_sha256: String,
    storage_path: String,
    storage_ast_sha256: String,
    root: String,
    storage_methods: Vec<String>,
    statements: Vec<PostCoreSqlStatementDescriptor>,
    allowed_capabilities: Vec<PostCoreSqlOperationCapabilityDescriptor>,
    forbidden_classes: Vec<String>,
}

#[derive(Clone, Copy)]
struct PostCoreSqlOperationCapabilitySpec {
    operation: &'static str,
    table: &'static str,
}

impl PostCoreSqlOperationCapabilitySpec {
    const fn new(operation: &'static str, table: &'static str) -> Self {
        Self { operation, table }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
struct PostCoreSqlOperationCapabilityDescriptor {
    operation: String,
    table: String,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
struct PostCoreSqlStatementDescriptor {
    function: String,
    operation: String,
    tables: Vec<String>,
    terminal: String,
    sql_sha256: String,
    placeholder_count: u64,
    bind_expressions: Vec<String>,
}

struct PostCoreSqlTerminal {
    sql: String,
    terminal: String,
    bind_expressions: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct MirroredFileDescriptor {
    canonical_path: String,
    mirror_path: String,
    byte_length: u64,
    sha256: String,
    executor_id: String,
    executor_path: String,
    executor_test: String,
    executor_hash_algorithm: String,
    executor_canonical_byte_length: u64,
    executor_sha256: String,
}

#[derive(Debug, Deserialize)]
struct CargoLock {
    package: Vec<CargoLockPackage>,
}

#[derive(Clone, Debug, Deserialize)]
struct CargoLockPackage {
    name: String,
    version: String,
    source: Option<String>,
    checksum: Option<String>,
    #[serde(default)]
    dependencies: Vec<String>,
}

#[derive(Debug)]
struct CargoLockDependency {
    name: String,
    version: Option<String>,
    source: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ReconciliationResultVector {
    schema_version: u32,
    hook_id: String,
    cases: Vec<ReconciliationResultCase>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ReconciliationResultCase {
    id: String,
    source_generation_hex: String,
    input_events: Vec<ObservedSignedEvent>,
    expected: ReconciliationExpectedResult,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ObservedSignedEvent {
    observed_at_ms: i64,
    event: SignedEvent,
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
struct ReconciliationExpectedResult {
    raw_event_count: u64,
    coordinate_count: u64,
    request_count: u64,
    event_target_count: u64,
    address_target_count: u64,
    transition_count: u64,
    state: ReconciliationExpectedState,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ReconciliationExpectedState {
    kind: u32,
    pubkey: String,
    d_tag: String,
    raw_head_event_id: String,
    admission_status: String,
    contract_id: String,
    visibility: String,
    nip09_outcome: String,
    nip09_reason: String,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    event_reference_request_id: RequiredNullable<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    address_reference_request_id: RequiredNullable<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    address_reference_cutoff: RequiredNullable<u64>,
}

#[derive(Debug, Serialize)]
#[serde(transparent)]
struct RequiredNullable<T>(Option<T>);

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

fn deserialize_required_nullable<'de, D, T>(
    deserializer: D,
) -> Result<RequiredNullable<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(RequiredNullable)
}

pub(crate) fn write_nip09_reconciliation_manifest(workspace_root: &Path) -> Result<(), String> {
    with_artifact_bundle_transaction(workspace_root, |transaction| {
        transaction.write(vec![
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
        ])?;
        validate_nip09_reconciliation_manifest_under_lock(workspace_root)
    })
}

pub(crate) fn validate_nip09_reconciliation_manifest(workspace_root: &Path) -> Result<(), String> {
    with_artifact_bundle_transaction(workspace_root, |_| {
        validate_nip09_reconciliation_manifest_under_lock(workspace_root)
    })
}

pub(super) fn validate_nip09_reconciliation_manifest_under_lock(
    workspace_root: &Path,
) -> Result<(), String> {
    let manifest_bytes = read_regular_file(workspace_root, MANIFEST_RELATIVE)?;
    let manifest_value: Value = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| format!("parse {MANIFEST_RELATIVE}: {error}"))?;
    let manifest: Nip09ReconciliationManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| format!("parse {MANIFEST_RELATIVE}: {error}"))?;
    validate_canonical_json(MANIFEST_RELATIVE, &manifest_bytes, &manifest)?;

    let schema_bytes = read_regular_file(workspace_root, MANIFEST_SCHEMA_RELATIVE)?;
    let schema: Value = serde_json::from_slice(&schema_bytes)
        .map_err(|error| format!("parse {MANIFEST_SCHEMA_RELATIVE}: {error}"))?;
    validate_canonical_json(MANIFEST_SCHEMA_RELATIVE, &schema_bytes, &schema)?;
    validate_manifest_json_schema(&schema, &manifest_value)?;
    let digest_bytes = read_regular_file(workspace_root, MANIFEST_SHA256_RELATIVE)?;
    validate_digest_sidecar(MANIFEST_SHA256_RELATIVE, &digest_bytes)?;
    let actual_digest = std::str::from_utf8(&digest_bytes[..64])
        .map_err(|error| format!("{MANIFEST_SHA256_RELATIVE} must be UTF-8: {error}"))?;
    if actual_digest != sha256_hex(&manifest_bytes) {
        return Err(format!(
            "{MANIFEST_SHA256_RELATIVE} must match the checked-in manifest bytes"
        ));
    }

    if manifest.schema_version != SCHEMA_VERSION
        || manifest.hook_id != HOOK_ID
        || manifest.migration.version != MIGRATION_VERSION
        || manifest.migration.name != MIGRATION_NAME
        || manifest.migration.up_sha256 != IMMUTABLE_PREDECESSOR_ARTIFACTS[9].sha256
        || manifest.migration.down_sha256 != IMMUTABLE_PREDECESSOR_ARTIFACTS[10].sha256
        || manifest.migration.schema_sha256 != SCHEMA_SHA256
        || manifest.profile.reconciliation_version != RECONCILIATION_VERSION
        || manifest.profile.addressable_feed_version != ADDRESSABLE_FEED_VERSION
        || manifest.profile.event_contract_registry_version != EVENT_CONTRACT_REGISTRY_VERSION
    {
        return Err(format!(
            "{MANIFEST_RELATIVE} does not describe the immutable NIP-09 predecessor identity"
        ));
    }

    let vector_bytes = read_regular_file(workspace_root, RESULT_VECTOR_CANONICAL_RELATIVE)?;
    let mirror_bytes = read_regular_file(workspace_root, RESULT_VECTOR_MIRROR_RELATIVE)?;
    if vector_bytes != mirror_bytes {
        return Err(format!(
            "{RESULT_VECTOR_MIRROR_RELATIVE} must exactly mirror {RESULT_VECTOR_CANONICAL_RELATIVE}"
        ));
    }
    let vector: ReconciliationResultVector = serde_json::from_slice(&vector_bytes)
        .map_err(|error| format!("parse {RESULT_VECTOR_CANONICAL_RELATIVE}: {error}"))?;
    validate_canonical_json(RESULT_VECTOR_CANONICAL_RELATIVE, &vector_bytes, &vector)?;
    validate_result_vector(&vector)?;

    for artifact in IMMUTABLE_PREDECESSOR_ARTIFACTS {
        let bytes = read_regular_file(workspace_root, artifact.relative)?;
        if bytes.len() != artifact.byte_length || sha256_hex(&bytes) != artifact.sha256 {
            return Err(format!(
                "immutable NIP-09 predecessor artifact {} does not match its authenticated byte identity",
                artifact.relative
            ));
        }
    }

    Ok(())
}

pub(super) fn nip09_predecessor_production_source_paths_under_lock(
    workspace_root: &Path,
) -> Result<BTreeSet<String>, String> {
    let manifest_bytes = read_regular_file(workspace_root, MANIFEST_RELATIVE)?;
    let manifest: Nip09ReconciliationManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| format!("parse {MANIFEST_RELATIVE}: {error}"))?;
    Ok(nip09_predecessor_production_source_paths(&manifest)
        .into_iter()
        .map(str::to_owned)
        .collect())
}

fn nip09_predecessor_production_source_paths(
    manifest: &Nip09ReconciliationManifest,
) -> BTreeSet<&str> {
    let mut paths = manifest
        .frozen_sources
        .iter()
        .map(|source| source.path.as_str())
        .chain(
            manifest
                .cargo_feature_profile
                .packages
                .iter()
                .map(|package| package.manifest_path.as_str()),
        )
        .chain(
            manifest
                .source_route_witnesses
                .iter()
                .map(|source| source.path.as_str()),
        )
        .chain(
            manifest
                .rust_item_witnesses
                .iter()
                .map(|source| source.path.as_str()),
        )
        .chain(
            manifest
                .rust_fragment_witnesses
                .iter()
                .map(|source| source.path.as_str()),
        )
        .chain(
            manifest
                .impl_resolution_witness
                .impls
                .iter()
                .map(|source| source.path.as_str()),
        )
        .collect::<BTreeSet<_>>();
    paths.extend([
        POST_CORE_CAPABILITIES_SOURCE_RELATIVE,
        POST_CORE_DISPATCHER_SOURCE_RELATIVE,
        POST_CORE_EXTENSION_SOURCE_RELATIVE,
        POST_CORE_STORAGE_SOURCE_RELATIVE,
    ]);
    paths
}

pub(super) fn validate_nip09_predecessor_production_sources_under_lock(
    workspace_root: &Path,
    superseded_paths: &[&str],
) -> Result<(), String> {
    let manifest_bytes = read_regular_file(workspace_root, MANIFEST_RELATIVE)?;
    let manifest: Nip09ReconciliationManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| format!("parse {MANIFEST_RELATIVE}: {error}"))?;
    let superseded = superseded_paths.iter().copied().collect::<BTreeSet<_>>();
    if superseded.len() != superseded_paths.len() {
        return Err("successor predecessor-source supersession paths must be unique".to_owned());
    }

    let predecessor_paths = nip09_predecessor_production_source_paths(&manifest);
    if let Some(path) = superseded
        .iter()
        .find(|path| !predecessor_paths.contains(**path))
    {
        return Err(format!(
            "successor supersession path `{path}` is not a predecessor-bound production source"
        ));
    }

    if manifest.frozen_sources.len() != FROZEN_SOURCE_SPECS.len() {
        return Err("immutable predecessor frozen-source inventory is incomplete".to_owned());
    }
    for (expected, spec) in manifest.frozen_sources.iter().zip(FROZEN_SOURCE_SPECS) {
        if expected.role != spec.role || expected.path != spec.path {
            return Err(format!(
                "immutable predecessor frozen-source inventory drifted at `{}`",
                spec.path
            ));
        }
        if superseded.contains(spec.path) {
            continue;
        }
        let current = describe_frozen_source(workspace_root, *spec)?;
        require_predecessor_frozen_source_match(expected, &current)?;
    }

    if manifest.source_route_witnesses.len() != SOURCE_ROUTE_WITNESS_SPECS.len() {
        return Err("immutable predecessor source-route inventory is incomplete".to_owned());
    }
    for (expected, spec) in manifest
        .source_route_witnesses
        .iter()
        .zip(SOURCE_ROUTE_WITNESS_SPECS)
    {
        if expected.role != spec.role || expected.path != spec.path {
            return Err(format!(
                "immutable predecessor source-route inventory drifted at `{}`",
                spec.path
            ));
        }
        if superseded.contains(spec.path) {
            continue;
        }
        let current = describe_source_route_witness(workspace_root, *spec)?;
        if current != *expected {
            return Err(format!(
                "unchanged predecessor source-route authority `{}` drifted",
                spec.path
            ));
        }
    }

    validate_predecessor_witness_subset(
        "Rust item",
        &manifest.rust_item_witnesses,
        superseded_paths,
        || describe_rust_item_witnesses(workspace_root),
        |witness| witness.path.as_str(),
    )?;
    validate_predecessor_witness_subset(
        "Rust fragment",
        &manifest.rust_fragment_witnesses,
        superseded_paths,
        || describe_rust_fragment_witnesses(workspace_root),
        |witness| witness.path.as_str(),
    )?;

    validate_predecessor_impl_resolution_authority(workspace_root, &manifest, superseded_paths)?;

    let post_core_paths = [
        POST_CORE_CAPABILITIES_SOURCE_RELATIVE,
        POST_CORE_DISPATCHER_SOURCE_RELATIVE,
        POST_CORE_EXTENSION_SOURCE_RELATIVE,
        POST_CORE_STORAGE_SOURCE_RELATIVE,
    ];
    let superseded_post_core_count = post_core_paths
        .iter()
        .filter(|path| superseded.contains(**path))
        .count();
    if superseded_post_core_count == 0 {
        let current = describe_post_core_sql_capability(workspace_root)?;
        if current != manifest.post_core_sql_capability {
            return Err(
                "unchanged predecessor post-core SQL capability drifted from the immutable manifest"
                    .to_owned(),
            );
        }
    } else if superseded_post_core_count != post_core_paths.len() {
        return Err(
            "the successor must supersede either every or no predecessor post-core capability source"
                .to_owned(),
        );
    }

    // The v1 manifest retains immutable evidence for the source tree used when
    // it was issued. Current SQLite provenance is governed by the release
    // contract and must not require that historical tree to remain vendored.
    Ok(())
}

fn validate_predecessor_impl_resolution_authority(
    workspace_root: &Path,
    manifest: &Nip09ReconciliationManifest,
    superseded_paths: &[&str],
) -> Result<(), String> {
    let superseded = superseded_paths.iter().copied().collect::<BTreeSet<_>>();
    let predecessor_impl_paths = manifest
        .impl_resolution_witness
        .impls
        .iter()
        .map(|item| item.path.as_str())
        .collect::<BTreeSet<_>>();
    let expected_impls = manifest
        .impl_resolution_witness
        .impls
        .iter()
        .filter(|item| !superseded.contains(item.path.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    if expected_impls.is_empty() {
        return Ok(());
    }

    let excluded_paths = superseded_paths
        .iter()
        .copied()
        .chain(SUCCESSOR_08C_EXCLUSIVE_SOURCE_PATHS)
        .chain(SUCCESSOR_08D_SOURCE_PATHS)
        .chain(SUCCESSOR_08D1_EXCLUSIVE_SOURCE_PATHS)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let predecessor_protected_members = manifest
        .impl_resolution_witness
        .impls
        .iter()
        .filter_map(|item| item.member.as_ref())
        .filter(|member| member.as_str() != "<macro>")
        .cloned()
        .collect::<BTreeSet<_>>();
    let current_impls = describe_impl_resolution_witness_excluding_paths(
        workspace_root,
        &excluded_paths,
        &manifest.impl_resolution_witness.protected_self_types,
        &predecessor_protected_members,
    )?
    .impls
    .into_iter()
    .filter(|item| {
        predecessor_impl_paths.contains(item.path.as_str())
            && !superseded.contains(item.path.as_str())
    })
    .collect::<Vec<_>>();
    if current_impls == expected_impls {
        return Ok(());
    }

    let current = current_impls.iter().collect::<BTreeSet<_>>();
    let expected = expected_impls.iter().collect::<BTreeSet<_>>();
    let missing = expected.difference(&current).copied().collect::<Vec<_>>();
    let unexpected = current.difference(&expected).copied().collect::<Vec<_>>();
    Err(format!(
        "unchanged predecessor impl-resolution authority drifted from the immutable manifest: expected {} entries, found {}; missing {missing:?}; unexpected {unexpected:?}",
        expected_impls.len(),
        current_impls.len(),
    ))
}

fn require_predecessor_frozen_source_match(
    expected: &FrozenSourceDescriptor,
    current: &FrozenSourceDescriptor,
) -> Result<(), String> {
    if current != expected {
        return Err(format!(
            "unchanged predecessor frozen-source authority `{}` drifted from the immutable manifest",
            expected.path
        ));
    }
    Ok(())
}

fn validate_predecessor_witness_subset<T, Describe, PathOf>(
    label: &str,
    expected: &[T],
    superseded_paths: &[&str],
    describe: Describe,
    path_of: PathOf,
) -> Result<(), String>
where
    T: Clone + PartialEq,
    Describe: FnOnce() -> Result<Vec<T>, String>,
    PathOf: Fn(&T) -> &str,
{
    let superseded = superseded_paths.iter().copied().collect::<BTreeSet<_>>();
    let expected = expected
        .iter()
        .filter(|witness| !superseded.contains(path_of(witness)))
        .cloned()
        .collect::<Vec<_>>();
    if expected.is_empty() {
        return Ok(());
    }
    let current = describe()?
        .into_iter()
        .filter(|witness| !superseded.contains(path_of(witness)))
        .collect::<Vec<_>>();
    if current != expected {
        return Err(format!(
            "unchanged predecessor {label} witnesses drifted from the immutable manifest"
        ));
    }
    Ok(())
}

fn expected_artifacts(workspace_root: &Path) -> Result<Vec<GeneratedArtifact>, String> {
    let manifest = expected_manifest(workspace_root)?;
    validate_manifest_shape(workspace_root, &manifest)?;
    let manifest_bytes = canonical_json_bytes(&manifest)?;
    let manifest_sha256 = sha256_hex(&manifest_bytes);
    let schema_bytes = canonical_json_bytes(&manifest_schema())?;
    let descriptor = generated_descriptor(&manifest, &manifest_bytes, &manifest_sha256);

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
    ])
}

fn expected_manifest(workspace_root: &Path) -> Result<Nip09ReconciliationManifest, String> {
    validate_event_contract_registry_v7_inventory_under_lock(workspace_root).map_err(|error| {
        format!(
            "event-contract registry-v7 inventory must be fresh before generating the NIP-09 manifest: {error}"
        )
    })?;
    validate_governed_compiler_inputs(workspace_root)?;
    validate_governed_support_source_tree_baselines(workspace_root)?;
    validate_route_facade_baselines(workspace_root)?;
    describe_post_core_extension_boundary(workspace_root, true)?;
    validate_current_event_store_successor_authority(workspace_root)?;
    describe_nip09_v1_manifest(workspace_root)
}

fn describe_nip09_v1_manifest(
    workspace_root: &Path,
) -> Result<Nip09ReconciliationManifest, String> {
    let (up_byte_length, up_sha256) = describe_file(workspace_root, MIGRATION_UP_RELATIVE)?;
    let (down_byte_length, down_sha256) = describe_file(workspace_root, MIGRATION_DOWN_RELATIVE)?;
    let (registry_byte_length, registry_sha256) =
        describe_file(workspace_root, REGISTRY_INVENTORY_RELATIVE)?;
    let manifest_schema_bytes = canonical_json_bytes(&manifest_schema())?;

    let semantic_dependencies = SEMANTIC_DEPENDENCY_SPECS
        .iter()
        .map(|spec| describe_semantic_dependency(workspace_root, *spec))
        .collect::<Result<Vec<_>, _>>()?;
    let frozen_sources = FROZEN_SOURCE_SPECS
        .iter()
        .map(|spec| describe_frozen_source(workspace_root, *spec))
        .collect::<Result<Vec<_>, _>>()?;
    let source_route_witnesses = SOURCE_ROUTE_WITNESS_SPECS
        .iter()
        .map(|spec| describe_source_route_witness(workspace_root, *spec))
        .collect::<Result<Vec<_>, _>>()?;
    let rust_item_witnesses = describe_rust_item_witnesses(workspace_root)?;
    let rust_fragment_witnesses = describe_rust_fragment_witnesses(workspace_root)?;
    let impl_resolution_witness = describe_impl_resolution_witness(workspace_root)?;
    let post_core_sql_capability = describe_post_core_sql_capability(workspace_root)?;
    validate_entry_point_sources(workspace_root)?;
    let result_vector = describe_result_vector(workspace_root)?;
    let cargo_lock = read_regular_file(workspace_root, CARGO_LOCK_RELATIVE)?;
    let (runtime_dependency_policy, runtime_dependencies) =
        runtime_dependencies_from_lock(&cargo_lock)?;
    let cargo_feature_profile = describe_cargo_feature_profile(workspace_root)?;
    let local_runtime_sources = vec![describe_local_sqlite_source(workspace_root)?];

    Ok(Nip09ReconciliationManifest {
        schema_version: SCHEMA_VERSION,
        hook_id: HOOK_ID.to_owned(),
        manifest_schema: FileDescriptor {
            path: MANIFEST_SCHEMA_RELATIVE.to_owned(),
            byte_length: byte_length(MANIFEST_SCHEMA_RELATIVE, &manifest_schema_bytes)?,
            sha256: sha256_hex(&manifest_schema_bytes),
        },
        migration: MigrationDescriptor {
            version: MIGRATION_VERSION,
            name: MIGRATION_NAME.to_owned(),
            up_byte_length,
            up_sha256,
            down_byte_length,
            down_sha256,
            schema_sha256: SCHEMA_SHA256.to_owned(),
        },
        profile: ReconciliationProfileDescriptor {
            reconciliation_version: RECONCILIATION_VERSION,
            addressable_feed_version: ADDRESSABLE_FEED_VERSION,
            event_contract_registry_version: EVENT_CONTRACT_REGISTRY_VERSION,
        },
        cargo_feature_profile,
        entry_points: expected_entry_points(),
        registry_inventory: FileDescriptor {
            path: REGISTRY_INVENTORY_RELATIVE.to_owned(),
            byte_length: registry_byte_length,
            sha256: registry_sha256,
        },
        semantic_dependencies,
        runtime_dependency_policy,
        runtime_dependencies,
        local_runtime_sources,
        frozen_sources,
        source_route_witnesses,
        rust_item_witnesses,
        rust_fragment_witnesses,
        impl_resolution_witness,
        post_core_sql_capability,
        result_vector,
    })
}

fn expected_entry_points() -> Vec<EntryPointDescriptor> {
    ENTRY_POINT_SPECS
        .iter()
        .map(|spec| EntryPointDescriptor {
            role: spec.role.to_owned(),
            rust_path: spec.rust_path.to_owned(),
        })
        .collect()
}

fn describe_semantic_dependency(
    workspace_root: &Path,
    spec: SemanticDependencySpec,
) -> Result<SemanticDependencyDescriptor, String> {
    let canonical = read_regular_file(workspace_root, spec.canonical_path)?;
    if let Some(mirror_path) = spec.mirror_path {
        let mirror = read_regular_file(workspace_root, mirror_path)?;
        if mirror != canonical {
            return Err(format!(
                "semantic dependency mirror {mirror_path} must exactly match {}",
                spec.canonical_path
            ));
        }
    }
    Ok(SemanticDependencyDescriptor {
        id: spec.id.to_owned(),
        canonical_path: spec.canonical_path.to_owned(),
        mirror_path: spec.mirror_path.map(str::to_owned),
        byte_length: byte_length(spec.canonical_path, &canonical)?,
        sha256: sha256_hex(&canonical),
        executors: spec
            .executors
            .iter()
            .map(|value| (*value).to_owned())
            .collect(),
    })
}

fn describe_frozen_source(
    workspace_root: &Path,
    spec: FrozenSourceSpec,
) -> Result<FrozenSourceDescriptor, String> {
    let bytes = read_regular_file(workspace_root, spec.path)?;
    describe_frozen_source_bytes(spec, &bytes)
}

fn describe_frozen_source_bytes(
    spec: FrozenSourceSpec,
    bytes: &[u8],
) -> Result<FrozenSourceDescriptor, String> {
    let canonical = canonical_rust_ast(spec.path, bytes, RustAstProfile::Production)?;
    let file = syn::parse_file(
        std::str::from_utf8(&canonical)
            .map_err(|error| format!("{} canonical source must be UTF-8: {error}", spec.path))?,
    )
    .map_err(|error| format!("parse canonical {} source: {error}", spec.path))?;
    validate_compiler_macro_inputs(spec.path, &file, &[])?;
    Ok(FrozenSourceDescriptor {
        role: spec.role.to_owned(),
        path: spec.path.to_owned(),
        hash_algorithm: RUST_PRODUCTION_AST_SHA256_ALGORITHM.to_owned(),
        canonical_byte_length: byte_length(spec.path, &canonical)?,
        sha256: sha256_hex(&canonical),
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RustAstProfile {
    Production,
    Full,
}

fn canonical_rust_ast(
    relative: &str,
    bytes: &[u8],
    profile: RustAstProfile,
) -> Result<Vec<u8>, String> {
    let source = std::str::from_utf8(bytes)
        .map_err(|error| format!("{relative} must be UTF-8 Rust source: {error}"))?;
    let mut file =
        syn::parse_file(source).map_err(|error| format!("parse {relative} as Rust: {error}"))?;
    syn::visit_mut::VisitMut::visit_file_mut(&mut RawIdentifierNormalizer, &mut file);
    syn::visit_mut::VisitMut::visit_file_mut(&mut DocumentationAstNormalizer, &mut file);
    if profile == RustAstProfile::Production {
        let mut normalizer = ProductionAstNormalizer::new(relative);
        syn::visit_mut::VisitMut::visit_file_mut(&mut normalizer, &mut file);
        normalizer.finish()?;
        audit_no_test_conditionals(relative, &file)?;
    }
    syn::visit_mut::VisitMut::visit_file_mut(&mut OptionalPunctuationNormalizer, &mut file);
    let canonical = prettyplease::unparse(&file).into_bytes();
    if canonical.is_empty() {
        return Err(format!(
            "{relative} canonical {profile:?} Rust AST must not be empty"
        ));
    }
    Ok(canonical)
}

pub(super) fn canonical_production_rust_bytes(
    relative: &str,
    bytes: &[u8],
) -> Result<Vec<u8>, String> {
    canonical_rust_ast(relative, bytes, RustAstProfile::Production)
}

fn parse_canonical_production_rust(relative: &str, bytes: &[u8]) -> Result<syn::File, String> {
    let canonical = canonical_rust_ast(relative, bytes, RustAstProfile::Production)?;
    let canonical = std::str::from_utf8(&canonical)
        .map_err(|error| format!("{relative} canonical Rust AST must be UTF-8: {error}"))?;
    syn::parse_file(canonical)
        .map_err(|error| format!("parse canonical {relative} Rust AST: {error}"))
}

fn validate_compiler_macro_inputs(
    relative: &str,
    file: &syn::File,
    expected: &[String],
) -> Result<(), String> {
    use syn::visit::Visit;

    struct Audit {
        inputs: Vec<String>,
    }

    impl<'ast> Visit<'ast> for Audit {
        fn visit_macro(&mut self, item: &'ast syn::Macro) {
            if item.path.segments.last().is_some_and(|segment| {
                matches!(
                    segment.ident.to_string().as_str(),
                    "include" | "include_bytes" | "include_str" | "env" | "option_env"
                )
            }) {
                self.inputs.push(compact_tokens(item));
            }
            syn::visit::visit_macro(self, item);
        }
    }

    let mut audit = Audit { inputs: Vec::new() };
    audit.visit_file(file);
    if audit.inputs != expected {
        return Err(format!(
            "{relative} compiler macro inputs drifted: expected {expected:?}, found {:?}",
            audit.inputs
        ));
    }
    Ok(())
}

fn expected_event_store_migration_compiler_inputs(
    workspace_root: &Path,
    file: &syn::File,
) -> Result<Vec<String>, String> {
    let relative = EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE;
    let registry = exact_executor_const(file, relative, "EVENT_STORE_MIGRATIONS")?;
    let syn::Expr::Array(entries) = peel_expression(&registry.expr) else {
        return Err(format!(
            "{relative} EVENT_STORE_MIGRATIONS must remain a direct array"
        ));
    };
    if entries.elems.len() < 2 {
        return Err(format!(
            "{relative} EVENT_STORE_MIGRATIONS must retain versions 1 and 2"
        ));
    }

    fn raw_field<'a>(
        relative: &str,
        entry: &'a syn::ExprStruct,
        name: &str,
    ) -> Result<&'a syn::Expr, String> {
        let fields = entry
            .fields
            .iter()
            .filter(|field| matches!(&field.member, syn::Member::Named(actual) if actual == name))
            .collect::<Vec<_>>();
        let [field] = fields.as_slice() else {
            return Err(format!(
                "{relative} future-compatible migration entry must contain field `{name}` exactly once"
            ));
        };
        Ok(&field.expr)
    }

    fn field<'a>(
        relative: &str,
        entry: &'a syn::ExprStruct,
        name: &str,
    ) -> Result<&'a syn::Expr, String> {
        Ok(peel_expression(raw_field(relative, entry, name)?))
    }

    fn integer_field(relative: &str, entry: &syn::ExprStruct, name: &str) -> Result<u32, String> {
        let syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Int(value),
            ..
        }) = field(relative, entry, name)?
        else {
            return Err(format!(
                "{relative} migration field `{name}` must be an integer literal"
            ));
        };
        value
            .base10_parse()
            .map_err(|error| format!("{relative} migration field `{name}` is invalid: {error}"))
    }

    fn string_field(relative: &str, entry: &syn::ExprStruct, name: &str) -> Result<String, String> {
        let syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Str(value),
            ..
        }) = field(relative, entry, name)?
        else {
            return Err(format!(
                "{relative} migration field `{name}` must be a string literal"
            ));
        };
        Ok(value.value())
    }

    fn include_str_field(
        relative: &str,
        entry: &syn::ExprStruct,
        name: &str,
        expected_path: &str,
    ) -> Result<String, String> {
        let syn::Expr::Macro(expression) = field(relative, entry, name)? else {
            return Err(format!(
                "{relative} migration field `{name}` must be an include_str! compiler input"
            ));
        };
        if !expression.mac.path.is_ident("include_str") {
            return Err(format!(
                "{relative} migration field `{name}` must use include_str!"
            ));
        }
        let path = syn::parse2::<syn::LitStr>(expression.mac.tokens.clone())
            .map_err(|error| format!("{relative} migration field `{name}` path: {error}"))?;
        if path.value() != expected_path {
            return Err(format!(
                "{relative} migration field `{name}` must include `{expected_path}`, found `{}`",
                path.value()
            ));
        }
        Ok(compact_tokens(&expression.mac))
    }

    let mut inputs = Vec::with_capacity(entries.elems.len() * 2);
    for (index, expression) in entries.elems.iter().enumerate() {
        let syn::Expr::Struct(entry) = peel_expression(expression) else {
            return Err(format!(
                "{relative} EVENT_STORE_MIGRATIONS entry {index} must be an EventStoreMigration struct literal"
            ));
        };
        if compact_tokens(&entry.path) != "EventStoreMigration" {
            return Err(format!(
                "{relative} EVENT_STORE_MIGRATIONS entry {index} must construct EventStoreMigration"
            ));
        }
        let version = integer_field(relative, entry, "version")?;
        let expected_version = u32::try_from(index + 1)
            .map_err(|_| format!("{relative} migration registry is too large"))?;
        if version != expected_version {
            return Err(format!(
                "{relative} migration registry must remain sequential: expected {expected_version}, found {version}"
            ));
        }
        let name = string_field(relative, entry, "name")?;
        if name.is_empty()
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        {
            return Err(format!(
                "{relative} migration {version} name must be non-empty lowercase snake_case"
            ));
        }
        let hookless = if version > 2 {
            let expected_authority = match (version, name.as_str()) {
                (3, "food_availability_projection") => [
                    (
                        "hook",
                        "EventStoreMigrationHook::FoodAvailabilityProjectionV1",
                    ),
                    (
                        "hook_manifest_sha256",
                        "Some(food_manifest::FOOD_AVAILABILITY_PROJECTION_MANIFEST_SHA256,)",
                    ),
                    (
                        "event_contract_registry_version",
                        "Some(food_manifest::FOOD_AVAILABILITY_PROJECTION_EVENT_CONTRACT_REGISTRY_VERSION,)",
                    ),
                ],
                (SOURCE_MAINTENANCE_MIGRATION_VERSION, SOURCE_MAINTENANCE_MIGRATION_NAME) => [
                    ("hook", "EventStoreMigrationHook::SourceMaintenanceV1"),
                    (
                        "hook_manifest_sha256",
                        "Some(source_maintenance_manifest::SOURCE_MAINTENANCE_MANIFEST_SHA256,)",
                    ),
                    (
                        "event_contract_registry_version",
                        "Some(source_maintenance_manifest::SOURCE_MAINTENANCE_EVENT_CONTRACT_REGISTRY_VERSION,)",
                    ),
                ],
                _ => [
                    ("hook", "EventStoreMigrationHook::None"),
                    ("hook_manifest_sha256", "None"),
                    ("event_contract_registry_version", "None"),
                ],
            };
            let hookless = expected_authority[0].1 == "EventStoreMigrationHook::None";
            for (field_name, expected) in expected_authority {
                let actual = compact_tokens(field(relative, entry, field_name)?);
                if actual != expected {
                    return Err(format!(
                        "{relative} post-v2 migration {version} has invalid versioned hook authority: field `{field_name}` expected `{expected}`, found `{actual}`"
                    ));
                }
            }
            if hookless {
                let replacements =
                    compact_tokens(raw_field(relative, entry, "replaced_object_names")?);
                if replacements != "&[]" {
                    return Err(format!(
                        "{relative} hookless post-v2 migration {version} must not declare predecessor replacements without separately authenticated successor authority; found `{replacements}`"
                    ));
                }
            }
            hookless
        } else {
            false
        };
        for direction in ["up", "down"] {
            let expected_path = format!("../migrations/{version:04}_{name}.{direction}.sql");
            inputs.push(include_str_field(
                relative,
                entry,
                &format!("{direction}_sql"),
                &expected_path,
            )?);
            if version > 2 && hookless {
                validate_hookless_post_v2_migration_sql_isolated(
                    workspace_root,
                    version,
                    direction,
                    &format!("crates/event_store/migrations/{version:04}_{name}.{direction}.sql"),
                )?;
            }
        }
    }

    let current = exact_executor_const(
        file,
        relative,
        "RADROOTS_EVENT_STORE_SCHEMA_VERSION_CURRENT",
    )?;
    let syn::Expr::Lit(syn::ExprLit {
        lit: syn::Lit::Int(current),
        ..
    }) = peel_expression(&current.expr)
    else {
        return Err(format!(
            "{relative} RADROOTS_EVENT_STORE_SCHEMA_VERSION_CURRENT must be an integer literal"
        ));
    };
    if current.base10_parse::<usize>().ok() != Some(entries.elems.len()) {
        return Err(format!(
            "{relative} current schema version must equal the migration registry length"
        ));
    }
    Ok(inputs)
}

fn validate_hookless_post_v2_migration_sql_isolated(
    workspace_root: &Path,
    version: u32,
    direction: &str,
    relative: &str,
) -> Result<(), String> {
    let protected = protected_v1_migration_object_names(workspace_root)?;
    let bytes = read_regular_file(workspace_root, relative)?;
    let source = std::str::from_utf8(&bytes)
        .map_err(|error| format!("{relative} must be UTF-8 SQL: {error}"))?;
    let identifiers = sqlite_sql_identifiers(source)?;
    let forbidden_ambient = [
        "analyze",
        "attach",
        "begin",
        "commit",
        "detach",
        "end",
        "eval",
        "load_extension",
        "pragma",
        "reindex",
        "release",
        "rollback",
        "savepoint",
        "writable_schema",
        "vacuum",
        "writefile",
    ];
    if let Some(identifier) = identifiers.iter().find(|identifier| {
        protected.contains(identifier.as_str())
            || identifier.starts_with("sqlite_")
            || forbidden_ambient.contains(&identifier.as_str())
    }) {
        return Err(format!(
            "{relative} hookless post-v2 migration {version} {direction} SQL references protected v1 object or ambient schema authority `{identifier}`; use a separately authenticated migration-bound extension contract"
        ));
    }
    let owned = hookless_migration_owned_names(workspace_root, version)?;
    validate_hookless_migration_owned_ddl(relative, version, direction, source, &owned)?;
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
struct HooklessMigrationOwnedNames {
    objects: BTreeSet<String>,
    tables: BTreeSet<String>,
}

fn hookless_migration_owned_names(
    workspace_root: &Path,
    version: u32,
) -> Result<HooklessMigrationOwnedNames, String> {
    let bytes = read_regular_file(workspace_root, EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE)?;
    let file = parse_canonical_production_rust(EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE, &bytes)?;
    let entry = exact_const_struct_array_element(
        EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
        &file,
        "EVENT_STORE_MIGRATIONS",
        "version",
        u64::from(version),
    )?;

    fn field<'a>(entry: &'a syn::ExprStruct, name: &str) -> Result<&'a syn::Expr, String> {
        let matches = entry
            .fields
            .iter()
            .filter(|field| matches!(&field.member, syn::Member::Named(actual) if actual == name))
            .collect::<Vec<_>>();
        let [field] = matches.as_slice() else {
            return Err(format!(
                "{EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE} hookless migration must declare field `{name}` exactly once; found {}",
                matches.len()
            ));
        };
        Ok(&field.expr)
    }

    fn names(
        file: &syn::File,
        entry: &syn::ExprStruct,
        field_name: &str,
    ) -> Result<BTreeSet<String>, String> {
        let mut expression = peel_expression(field(entry, field_name)?);
        if let syn::Expr::Path(path) = expression
            && path.qself.is_none()
            && path.path.segments.len() == 1
        {
            let name = path.path.segments[0].ident.to_string();
            let constant =
                exact_executor_const(file, EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE, &name)?;
            expression = peel_expression(&constant.expr);
        }
        let syn::Expr::Array(array) = expression else {
            return Err(format!(
                "{EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE} hookless migration field `{field_name}` must be a direct string array or exact constant reference"
            ));
        };
        let mut names = BTreeSet::new();
        for value in &array.elems {
            let syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(value),
                ..
            }) = peel_expression(value)
            else {
                return Err(format!(
                    "{EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE} hookless migration field `{field_name}` may contain only string literals"
                ));
            };
            let value = value.value().to_ascii_lowercase();
            if !names.insert(value.clone()) {
                return Err(format!(
                    "{EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE} hookless migration field `{field_name}` duplicates `{value}`"
                ));
            }
        }
        Ok(names)
    }

    let objects = names(&file, entry, "owned_object_names")?;
    let tables = names(&file, entry, "owned_table_names")?;
    let fts5_tables = names(&file, entry, "fts5_table_names")?;
    if objects.is_empty() || tables.is_empty() || !tables.is_subset(&objects) {
        return Err(format!(
            "{EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE} hookless migration {version} must declare non-empty owned tables that are a subset of its owned objects"
        ));
    }
    if !fts5_tables.is_empty() {
        return Err(format!(
            "{EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE} hookless migration {version} must not declare FTS5 tables; virtual-table migrations require an authenticated hook"
        ));
    }
    Ok(HooklessMigrationOwnedNames { objects, tables })
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum HooklessSqlToken {
    Bare(String),
    QuotedIdentifier(String),
    StringLiteral(String),
    Number(String),
    Symbol(char),
    Operator(String),
}

impl HooklessSqlToken {
    fn bare(&self) -> Option<&str> {
        match self {
            Self::Bare(value) => Some(value),
            _ => None,
        }
    }

    fn identifier(&self) -> Option<&str> {
        match self {
            Self::Bare(value) | Self::QuotedIdentifier(value) | Self::StringLiteral(value) => {
                Some(value)
            }
            Self::Number(_) | Self::Symbol(_) | Self::Operator(_) => None,
        }
    }

    fn is_symbol(&self, expected: char) -> bool {
        matches!(self, Self::Symbol(actual) if *actual == expected)
    }
}

fn lex_hookless_migration_sql(
    relative: &str,
    source: &str,
) -> Result<Vec<Vec<HooklessSqlToken>>, String> {
    let bytes = source.as_bytes();
    let mut statements = Vec::new();
    let mut statement = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte.is_ascii_whitespace() {
            index += 1;
        } else if byte == b'-' && bytes.get(index + 1) == Some(&b'-') {
            index += 2;
            while index < bytes.len() && bytes[index] != b'\n' {
                index += 1;
            }
        } else if byte == b'/' && bytes.get(index + 1) == Some(&b'*') {
            index += 2;
            let mut closed = false;
            while index + 1 < bytes.len() {
                if bytes[index] == b'*' && bytes[index + 1] == b'/' {
                    index += 2;
                    closed = true;
                    break;
                }
                index += 1;
            }
            if !closed {
                return Err(format!(
                    "{relative} hookless migration SQL contains an unterminated block comment"
                ));
            }
        } else if byte == b';' {
            if statement.is_empty() {
                return Err(format!(
                    "{relative} hookless migration SQL contains an empty statement"
                ));
            }
            statements.push(std::mem::take(&mut statement));
            index += 1;
        } else if byte == b'\'' {
            index += 1;
            let mut value = Vec::new();
            let mut closed = false;
            while index < bytes.len() {
                if bytes[index] == b'\'' {
                    if bytes.get(index + 1) == Some(&b'\'') {
                        value.push(b'\'');
                        index += 2;
                    } else {
                        index += 1;
                        closed = true;
                        break;
                    }
                } else {
                    value.push(bytes[index]);
                    index += 1;
                }
            }
            if !closed {
                return Err(format!(
                    "{relative} hookless migration SQL contains an unterminated string literal"
                ));
            }
            let value = String::from_utf8(value)
                .map_err(|error| format!("{relative} SQL string literal must be UTF-8: {error}"))?;
            statement.push(HooklessSqlToken::StringLiteral(value.to_ascii_lowercase()));
        } else if matches!(byte, b'"' | b'`' | b'[') {
            let opener = byte;
            let closer = if opener == b'[' { b']' } else { opener };
            index += 1;
            let mut value = Vec::new();
            let mut closed = false;
            while index < bytes.len() {
                if bytes[index] == closer {
                    if opener != b'[' && bytes.get(index + 1) == Some(&closer) {
                        value.push(closer);
                        index += 2;
                    } else {
                        index += 1;
                        closed = true;
                        break;
                    }
                } else {
                    value.push(bytes[index]);
                    index += 1;
                }
            }
            if !closed {
                return Err(format!(
                    "{relative} hookless migration SQL contains an unterminated quoted identifier"
                ));
            }
            let value = String::from_utf8(value).map_err(|error| {
                format!("{relative} SQL quoted identifier must be UTF-8: {error}")
            })?;
            statement.push(HooklessSqlToken::QuotedIdentifier(
                value.to_ascii_lowercase(),
            ));
        } else if byte.is_ascii_alphabetic() || byte == b'_' {
            let start = index;
            index += 1;
            while index < bytes.len()
                && (bytes[index].is_ascii_alphanumeric() || matches!(bytes[index], b'_' | b'$'))
            {
                index += 1;
            }
            statement.push(HooklessSqlToken::Bare(
                source[start..index].to_ascii_lowercase(),
            ));
        } else if byte.is_ascii_digit() {
            let start = index;
            index += 1;
            while index < bytes.len()
                && (bytes[index].is_ascii_alphanumeric() || matches!(bytes[index], b'_' | b'.'))
            {
                index += 1;
            }
            statement.push(HooklessSqlToken::Number(source[start..index].to_owned()));
        } else if matches!(byte, b'(' | b')' | b',' | b'.') {
            statement.push(HooklessSqlToken::Symbol(char::from(byte)));
            index += 1;
        } else if matches!(
            byte,
            b'=' | b'<' | b'>' | b'!' | b'+' | b'-' | b'*' | b'/' | b'%' | b'|' | b'&' | b'~'
        ) {
            let start = index;
            index += 1;
            while index < bytes.len()
                && matches!(
                    bytes[index],
                    b'=' | b'<' | b'>' | b'!' | b'+' | b'-' | b'*' | b'/' | b'%' | b'|' | b'&'
                )
            {
                index += 1;
            }
            statement.push(HooklessSqlToken::Operator(source[start..index].to_owned()));
        } else {
            return Err(format!(
                "{relative} hookless migration SQL contains unsupported byte `{}`",
                char::from(byte)
            ));
        }
    }
    if !statement.is_empty() {
        statements.push(statement);
    }
    if statements.is_empty() {
        return Err(format!(
            "{relative} hookless migration SQL must contain at least one statement"
        ));
    }
    Ok(statements)
}

fn hookless_qualified_identifier(
    relative: &str,
    tokens: &[HooklessSqlToken],
    index: &mut usize,
) -> Result<(String, usize), String> {
    let first_index = *index;
    let first = tokens
        .get(*index)
        .and_then(HooklessSqlToken::identifier)
        .ok_or_else(|| {
            format!("{relative} hookless migration DDL expected an owned object identifier")
        })?
        .to_owned();
    *index += 1;
    if tokens.get(*index).is_some_and(|token| token.is_symbol('.')) {
        if first != "main" {
            return Err(format!(
                "{relative} hookless migration DDL may qualify owned objects only with `main`"
            ));
        }
        *index += 1;
        let identifier_index = *index;
        let identifier = tokens
            .get(*index)
            .and_then(HooklessSqlToken::identifier)
            .ok_or_else(|| {
                format!("{relative} hookless migration DDL has an incomplete main-qualified name")
            })?
            .to_owned();
        *index += 1;
        Ok((identifier, identifier_index))
    } else {
        Ok((first, first_index))
    }
}

fn hookless_matching_right_paren(
    relative: &str,
    tokens: &[HooklessSqlToken],
    left: usize,
) -> Result<usize, String> {
    let mut depth = 0usize;
    for (index, token) in tokens.iter().enumerate().skip(left) {
        if token.is_symbol('(') {
            depth += 1;
        } else if token.is_symbol(')') {
            depth = depth
                .checked_sub(1)
                .ok_or_else(|| format!("{relative} hookless migration DDL has an unmatched `)`"))?;
            if depth == 0 {
                return Ok(index);
            }
        }
    }
    Err(format!(
        "{relative} hookless migration DDL has an unmatched `(`"
    ))
}

fn validate_hookless_migration_owned_ddl(
    relative: &str,
    version: u32,
    direction: &str,
    source: &str,
    owned: &HooklessMigrationOwnedNames,
) -> Result<(), String> {
    let statements = lex_hookless_migration_sql(relative, source)?;
    let forbidden_words = [
        "alter",
        "analyze",
        "attach",
        "begin",
        "call",
        "commit",
        "delete",
        "detach",
        "do",
        "end",
        "eval",
        "from",
        "insert",
        "intersect",
        "join",
        "load_extension",
        "pragma",
        "reindex",
        "release",
        "replace",
        "returning",
        "rollback",
        "savepoint",
        "select",
        "trigger",
        "union",
        "update",
        "vacuum",
        "values",
        "view",
        "virtual",
        "with",
        "writefile",
    ];
    let allowed_paren_leaders = ["check", "default", "in", "key", "unique"];
    let mut touched = BTreeSet::new();

    for tokens in &statements {
        if let Some(word) = tokens
            .iter()
            .filter_map(HooklessSqlToken::bare)
            .find(|word| forbidden_words.contains(word))
        {
            return Err(format!(
                "{relative} hookless post-v2 migration {version} {direction} SQL uses forbidden non-DDL authority `{word}`; backfills and executable schema objects require a separately authenticated migration hook"
            ));
        }
        let mut depth = 0usize;
        for token in tokens {
            if token.is_symbol('(') {
                depth += 1;
            } else if token.is_symbol(')') {
                depth = depth.checked_sub(1).ok_or_else(|| {
                    format!("{relative} hookless migration DDL has an unmatched `)`")
                })?;
            }
        }
        if depth != 0 {
            return Err(format!(
                "{relative} hookless migration DDL has unbalanced parentheses"
            ));
        }

        let mut structural_paren_leaders = BTreeSet::new();
        let mut index = 0usize;
        let command = tokens
            .get(index)
            .and_then(HooklessSqlToken::bare)
            .ok_or_else(|| {
                format!("{relative} hookless migration statement must begin with CREATE or DROP")
            })?;
        index += 1;
        match (direction, command) {
            ("up", "create") => {
                let mut object_kind = tokens
                    .get(index)
                    .and_then(HooklessSqlToken::bare)
                    .ok_or_else(|| {
                        format!("{relative} hookless CREATE must name TABLE or INDEX")
                    })?;
                if object_kind == "unique" {
                    index += 1;
                    object_kind = tokens
                        .get(index)
                        .and_then(HooklessSqlToken::bare)
                        .ok_or_else(|| {
                            format!("{relative} hookless CREATE UNIQUE must name INDEX")
                        })?;
                    if object_kind != "index" {
                        return Err(format!(
                            "{relative} hookless CREATE UNIQUE supports only INDEX"
                        ));
                    }
                }
                if !matches!(object_kind, "table" | "index") {
                    return Err(format!(
                        "{relative} hookless CREATE supports only TABLE and INDEX"
                    ));
                }
                index += 1;
                let (object, _) = hookless_qualified_identifier(relative, tokens, &mut index)?;
                let object_is_table = owned.tables.contains(&object);
                if !owned.objects.contains(&object)
                    || (object_kind == "table") != object_is_table
                    || !touched.insert(object.clone())
                {
                    return Err(format!(
                        "{relative} hookless CREATE {object_kind} target `{object}` must be a unique object of the matching kind declared by migration {version}"
                    ));
                }
                if object_kind == "table" {
                    if !tokens.get(index).is_some_and(|token| token.is_symbol('(')) {
                        return Err(format!(
                            "{relative} hookless CREATE TABLE must use an explicit column definition, not AS SELECT"
                        ));
                    }
                    structural_paren_leaders.insert(index.saturating_sub(1));
                    let right = hookless_matching_right_paren(relative, tokens, index)?;
                    for reference in tokens
                        .iter()
                        .enumerate()
                        .filter(|(_, token)| token.bare() == Some("references"))
                        .map(|(index, _)| index)
                    {
                        let mut reference_index = reference + 1;
                        let (table, table_token) =
                            hookless_qualified_identifier(relative, tokens, &mut reference_index)?;
                        if !owned.tables.contains(&table) {
                            return Err(format!(
                                "{relative} hookless CREATE TABLE reference `{table}` is outside migration {version}'s owned tables"
                            ));
                        }
                        structural_paren_leaders.insert(table_token);
                    }
                    let suffix = &tokens[right + 1..];
                    let suffix_words = suffix
                        .iter()
                        .map(|token| match token {
                            HooklessSqlToken::Bare(word) => Ok(word.as_str()),
                            HooklessSqlToken::Symbol(',') => Ok(","),
                            _ => Err(format!(
                                "{relative} hookless CREATE TABLE has unsupported trailing syntax"
                            )),
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    if !matches!(
                        suffix_words.as_slice(),
                        [] | ["strict"]
                            | ["without", "rowid"]
                            | ["strict", ",", "without", "rowid"]
                            | ["without", "rowid", ",", "strict"]
                    ) {
                        return Err(format!(
                            "{relative} hookless CREATE TABLE supports only deterministic STRICT and WITHOUT ROWID suffixes"
                        ));
                    }
                } else {
                    if tokens.get(index).and_then(HooklessSqlToken::bare) != Some("on") {
                        return Err(format!(
                            "{relative} hookless CREATE INDEX must directly name its owned table"
                        ));
                    }
                    index += 1;
                    let (table, table_token) =
                        hookless_qualified_identifier(relative, tokens, &mut index)?;
                    if !owned.tables.contains(&table) {
                        return Err(format!(
                            "{relative} hookless CREATE INDEX table `{table}` is outside migration {version}'s owned tables"
                        ));
                    }
                    if !tokens.get(index).is_some_and(|token| token.is_symbol('(')) {
                        return Err(format!(
                            "{relative} hookless CREATE INDEX must declare an explicit key list"
                        ));
                    }
                    structural_paren_leaders.insert(table_token);
                    let _ = hookless_matching_right_paren(relative, tokens, index)?;
                }
            }
            ("down", "drop") => {
                let object_kind = tokens
                    .get(index)
                    .and_then(HooklessSqlToken::bare)
                    .ok_or_else(|| format!("{relative} hookless DROP must name TABLE or INDEX"))?;
                if !matches!(object_kind, "table" | "index") {
                    return Err(format!(
                        "{relative} hookless DROP supports only TABLE and INDEX"
                    ));
                }
                index += 1;
                let (object, _) = hookless_qualified_identifier(relative, tokens, &mut index)?;
                let object_is_table = owned.tables.contains(&object);
                if index != tokens.len()
                    || !owned.objects.contains(&object)
                    || (object_kind == "table") != object_is_table
                    || !touched.insert(object.clone())
                {
                    return Err(format!(
                        "{relative} hookless DROP {object_kind} target `{object}` must be a unique exact object of the matching kind declared by migration {version}"
                    ));
                }
            }
            ("up", _) => {
                return Err(format!(
                    "{relative} hookless post-v2 up migration {version} may contain only CREATE TABLE/INDEX statements"
                ));
            }
            ("down", _) => {
                return Err(format!(
                    "{relative} hookless post-v2 down migration {version} may contain only DROP TABLE/INDEX statements"
                ));
            }
            _ => {
                return Err(format!(
                    "{relative} hookless migration direction `{direction}` is unsupported"
                ));
            }
        }

        for (index, pair) in tokens.windows(2).enumerate() {
            if pair[1].is_symbol('(')
                && pair[0].identifier().is_some()
                && !structural_paren_leaders.contains(&index)
                && !pair[0]
                    .bare()
                    .is_some_and(|word| allowed_paren_leaders.contains(&word))
            {
                return Err(format!(
                    "{relative} hookless migration DDL contains unsupported function or indirect call `{}`",
                    pair[0].identifier().unwrap_or("<unknown>")
                ));
            }
        }
    }

    if touched != owned.objects {
        let missing = owned
            .objects
            .difference(&touched)
            .cloned()
            .collect::<Vec<_>>();
        let extra = touched
            .difference(&owned.objects)
            .cloned()
            .collect::<Vec<_>>();
        return Err(format!(
            "{relative} hookless post-v2 migration {version} {direction} DDL must cover exactly its declared owned objects; missing {missing:?}, extra {extra:?}"
        ));
    }
    Ok(())
}

fn protected_v1_migration_object_names(workspace_root: &Path) -> Result<BTreeSet<String>, String> {
    let mut protected = BTreeSet::new();
    let migrations_bytes =
        read_regular_file(workspace_root, EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE)?;
    let migrations =
        parse_canonical_production_rust(EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE, &migrations_bytes)?;
    validate_event_store_migrations_import_authority(
        EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
        &migrations,
    )?;
    for name in [
        "EVENT_STORE_BASELINE_OBJECT_NAMES",
        "EVENT_STORE_BASELINE_TABLE_NAMES",
        "EVENT_STORE_NIP09_OBJECT_NAMES",
        "EVENT_STORE_NIP09_TABLE_NAMES",
    ] {
        let constant =
            exact_executor_const(&migrations, EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE, name)?;
        let syn::Expr::Reference(reference) = constant.expr.as_ref() else {
            return Err(format!(
                "{EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE} {name} must remain a direct referenced array"
            ));
        };
        let syn::Expr::Array(values) = peel_expression(&reference.expr) else {
            return Err(format!(
                "{EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE} {name} must remain a direct referenced array"
            ));
        };
        for value in &values.elems {
            let syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(value),
                ..
            }) = peel_expression(value)
            else {
                return Err(format!(
                    "{EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE} {name} may contain only string literals"
                ));
            };
            protected.insert(value.value().to_ascii_lowercase());
        }
    }
    let ledger = exact_executor_const(
        &migrations,
        EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
        "EVENT_STORE_LEDGER_NAME",
    )?;
    let syn::Expr::Lit(syn::ExprLit {
        lit: syn::Lit::Str(ledger),
        ..
    }) = peel_expression(&ledger.expr)
    else {
        return Err(format!(
            "{EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE} EVENT_STORE_LEDGER_NAME must remain a string literal"
        ));
    };
    protected.insert(ledger.value().to_ascii_lowercase());

    for relative in [
        MIGRATION_V1_UP_RELATIVE,
        MIGRATION_V1_DOWN_RELATIVE,
        MIGRATION_UP_RELATIVE,
        MIGRATION_DOWN_RELATIVE,
    ] {
        let bytes = read_regular_file(workspace_root, relative)?;
        let source = std::str::from_utf8(&bytes)
            .map_err(|error| format!("{relative} must be UTF-8 SQL: {error}"))?;
        let identifiers = sqlite_sql_identifiers(source)?;
        let mut index = 0;
        while index < identifiers.len() {
            if identifiers[index] != "create" {
                index += 1;
                continue;
            }
            index += 1;
            while index < identifiers.len()
                && matches!(
                    identifiers[index].as_str(),
                    "temp" | "temporary" | "unique" | "virtual"
                )
            {
                index += 1;
            }
            if index >= identifiers.len()
                || !matches!(
                    identifiers[index].as_str(),
                    "index" | "table" | "trigger" | "view"
                )
            {
                continue;
            }
            index += 1;
            while index < identifiers.len()
                && matches!(identifiers[index].as_str(), "if" | "not" | "exists")
            {
                index += 1;
            }
            if let Some(name) = identifiers.get(index) {
                protected.insert(name.clone());
            }
        }
    }
    protected.extend(
        POST_CORE_SQL_ALLOWED_CAPABILITIES
            .iter()
            .map(|capability| capability.table.to_owned()),
    );
    if protected.is_empty() {
        return Err("v1 migration protected-object inventory must not be empty".to_owned());
    }
    Ok(protected)
}

fn sqlite_sql_identifiers(source: &str) -> Result<Vec<String>, String> {
    let bytes = source.as_bytes();
    let mut identifiers = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index].is_ascii_whitespace() {
            index += 1;
        } else if bytes[index] == b'-' && bytes.get(index + 1) == Some(&b'-') {
            index += 2;
            while index < bytes.len() && bytes[index] != b'\n' {
                index += 1;
            }
        } else if bytes[index] == b'/' && bytes.get(index + 1) == Some(&b'*') {
            index += 2;
            let mut closed = false;
            while index + 1 < bytes.len() {
                if bytes[index] == b'*' && bytes[index + 1] == b'/' {
                    index += 2;
                    closed = true;
                    break;
                }
                index += 1;
            }
            if !closed {
                return Err("SQLite SQL contains an unterminated block comment".to_owned());
            }
        } else if bytes[index] == b'\'' {
            index += 1;
            let mut literal = Vec::new();
            let mut closed = false;
            while index < bytes.len() {
                if bytes[index] == b'\'' {
                    if bytes.get(index + 1) == Some(&b'\'') {
                        literal.push(b'\'');
                        index += 2;
                    } else {
                        index += 1;
                        closed = true;
                        break;
                    }
                } else {
                    literal.push(bytes[index]);
                    index += 1;
                }
            }
            if !closed {
                return Err("SQLite SQL contains an unterminated string literal".to_owned());
            }
            let literal = String::from_utf8(literal)
                .map_err(|error| format!("SQLite string literal must be UTF-8: {error}"))?;
            // SQLite accepts single-quoted tokens as identifiers in identifier positions.
            // Treat every literal spelling as a candidate identifier so that this legacy
            // compatibility rule cannot bypass the protected-object inventory.
            identifiers.push(literal.to_ascii_lowercase());
        } else if matches!(bytes[index], b'"' | b'`' | b'[') {
            let opener = bytes[index];
            let closer = if opener == b'[' { b']' } else { opener };
            index += 1;
            let mut identifier = Vec::new();
            let mut closed = false;
            while index < bytes.len() {
                if bytes[index] == closer {
                    if opener != b'[' && bytes.get(index + 1) == Some(&closer) {
                        identifier.push(closer);
                        index += 2;
                    } else {
                        index += 1;
                        closed = true;
                        break;
                    }
                } else {
                    identifier.push(bytes[index]);
                    index += 1;
                }
            }
            if !closed {
                return Err("SQLite SQL contains an unterminated quoted identifier".to_owned());
            }
            let identifier = std::str::from_utf8(&identifier)
                .map_err(|error| format!("SQLite quoted identifier must be UTF-8: {error}"))?;
            identifiers.push(identifier.to_ascii_lowercase());
        } else if bytes[index].is_ascii_alphabetic() || bytes[index] == b'_' {
            let start = index;
            index += 1;
            while index < bytes.len()
                && (bytes[index].is_ascii_alphanumeric() || matches!(bytes[index], b'_' | b'$'))
            {
                index += 1;
            }
            identifiers.push(source[start..index].to_ascii_lowercase());
        } else {
            index += 1;
        }
    }
    Ok(identifiers)
}

struct RawIdentifierNormalizer;

impl syn::visit_mut::VisitMut for RawIdentifierNormalizer {
    fn visit_ident_mut(&mut self, ident: &mut proc_macro2::Ident) {
        if let Some(normalized) = normalize_raw_identifier(ident) {
            *ident = normalized;
        }
    }

    fn visit_macro_mut(&mut self, item: &mut syn::Macro) {
        syn::visit_mut::visit_macro_mut(self, item);
        item.tokens = normalize_raw_identifier_tokens(item.tokens.clone());
    }
}

fn normalize_raw_identifier_tokens(tokens: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    use proc_macro2::{Group, TokenStream, TokenTree};

    let mut normalized = TokenStream::new();
    for token in tokens {
        match token {
            TokenTree::Group(group) => {
                let mut replacement = Group::new(
                    group.delimiter(),
                    normalize_raw_identifier_tokens(group.stream()),
                );
                replacement.set_span(group.span());
                normalized.extend([TokenTree::Group(replacement)]);
            }
            TokenTree::Ident(ident) => {
                normalized.extend([TokenTree::Ident(
                    normalize_raw_identifier(&ident).unwrap_or(ident),
                )]);
            }
            token => normalized.extend([token]),
        }
    }
    normalized
}

fn normalize_raw_identifier(ident: &proc_macro2::Ident) -> Option<proc_macro2::Ident> {
    let spelling = ident.to_string();
    let canonical = spelling.strip_prefix("r#")?;
    let mut normalized = syn::parse_str::<proc_macro2::Ident>(canonical).ok()?;
    normalized.set_span(ident.span());
    Some(normalized)
}

struct DocumentationAstNormalizer;

impl syn::visit_mut::VisitMut for DocumentationAstNormalizer {
    fn visit_file_mut(&mut self, file: &mut syn::File) {
        strip_documentation_attributes(&mut file.attrs);
        syn::visit_mut::visit_file_mut(self, file);
    }

    fn visit_item_mut(&mut self, item: &mut syn::Item) {
        if let Some(attributes) = item_attributes_mut(item) {
            strip_documentation_attributes(attributes);
        }
        syn::visit_mut::visit_item_mut(self, item);
    }

    fn visit_impl_item_mut(&mut self, item: &mut syn::ImplItem) {
        if let Some(attributes) = impl_item_attributes_mut(item) {
            strip_documentation_attributes(attributes);
        }
        syn::visit_mut::visit_impl_item_mut(self, item);
    }

    fn visit_trait_item_mut(&mut self, item: &mut syn::TraitItem) {
        if let Some(attributes) = trait_item_attributes_mut(item) {
            strip_documentation_attributes(attributes);
        }
        syn::visit_mut::visit_trait_item_mut(self, item);
    }

    fn visit_foreign_item_mut(&mut self, item: &mut syn::ForeignItem) {
        if let Some(attributes) = foreign_item_attributes_mut(item) {
            strip_documentation_attributes(attributes);
        }
        syn::visit_mut::visit_foreign_item_mut(self, item);
    }

    fn visit_field_mut(&mut self, field: &mut syn::Field) {
        strip_documentation_attributes(&mut field.attrs);
        syn::visit_mut::visit_field_mut(self, field);
    }

    fn visit_variant_mut(&mut self, variant: &mut syn::Variant) {
        strip_documentation_attributes(&mut variant.attrs);
        syn::visit_mut::visit_variant_mut(self, variant);
    }

    fn visit_arm_mut(&mut self, arm: &mut syn::Arm) {
        strip_documentation_attributes(&mut arm.attrs);
        syn::visit_mut::visit_arm_mut(self, arm);
    }

    fn visit_stmt_mut(&mut self, statement: &mut syn::Stmt) {
        if let Some(attributes) = statement_attributes_mut(statement) {
            strip_documentation_attributes(attributes);
        }
        syn::visit_mut::visit_stmt_mut(self, statement);
    }

    fn visit_expr_mut(&mut self, expression: &mut syn::Expr) {
        if let Some(attributes) = expression_attributes_mut(expression) {
            strip_documentation_attributes(attributes);
        }
        syn::visit_mut::visit_expr_mut(self, expression);
    }
}

fn strip_documentation_attributes(attributes: &mut Vec<syn::Attribute>) {
    attributes.retain(|attribute| !attribute.path().is_ident("doc"));
}

struct OptionalPunctuationNormalizer;

impl syn::visit_mut::VisitMut for OptionalPunctuationNormalizer {
    fn visit_signature_mut(&mut self, signature: &mut syn::Signature) {
        if !signature.inputs.is_empty() && !signature.inputs.trailing_punct() {
            signature.inputs.push_punct(Default::default());
        }
        syn::visit_mut::visit_signature_mut(self, signature);
    }

    fn visit_pat_tuple_mut(&mut self, pattern: &mut syn::PatTuple) {
        if pattern.elems.len() > 1 && !pattern.elems.trailing_punct() {
            pattern.elems.push_punct(Default::default());
        }
        syn::visit_mut::visit_pat_tuple_mut(self, pattern);
    }

    fn visit_pat_tuple_struct_mut(&mut self, pattern: &mut syn::PatTupleStruct) {
        if pattern.elems.len() > 1 && !pattern.elems.trailing_punct() {
            pattern.elems.push_punct(Default::default());
        }
        syn::visit_mut::visit_pat_tuple_struct_mut(self, pattern);
    }
}

#[derive(Clone)]
enum SimplifiedCfg {
    False,
    Residual(Box<syn::Meta>),
    True,
}

fn simplify_cfg_without_test(meta: &syn::Meta) -> Result<SimplifiedCfg, String> {
    use syn::parse::Parser;

    match meta {
        syn::Meta::Path(path) if path.is_ident("test") => Ok(SimplifiedCfg::False),
        syn::Meta::Path(_) | syn::Meta::NameValue(_) => {
            Ok(SimplifiedCfg::Residual(Box::new(meta.clone())))
        }
        syn::Meta::List(list) => {
            let arguments =
                syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated
                    .parse2(list.tokens.clone())
                    .map_err(|error| {
                        format!(
                            "parse cfg predicate `{}` while canonicalizing production Rust: {error}",
                            list.to_token_stream()
                        )
                    })?;
            let values = arguments
                .iter()
                .map(simplify_cfg_without_test)
                .collect::<Result<Vec<_>, _>>()?;
            if list.path.is_ident("all") {
                if values
                    .iter()
                    .any(|value| matches!(value, SimplifiedCfg::False))
                {
                    return Ok(SimplifiedCfg::False);
                }
                let residuals = values
                    .into_iter()
                    .filter_map(|value| match value {
                        SimplifiedCfg::Residual(meta) => Some(*meta),
                        SimplifiedCfg::False | SimplifiedCfg::True => None,
                    })
                    .collect::<Vec<_>>();
                simplify_cfg_list(&list.path, residuals, SimplifiedCfg::True)
            } else if list.path.is_ident("any") {
                if values
                    .iter()
                    .any(|value| matches!(value, SimplifiedCfg::True))
                {
                    return Ok(SimplifiedCfg::True);
                }
                let residuals = values
                    .into_iter()
                    .filter_map(|value| match value {
                        SimplifiedCfg::Residual(meta) => Some(*meta),
                        SimplifiedCfg::False | SimplifiedCfg::True => None,
                    })
                    .collect::<Vec<_>>();
                simplify_cfg_list(&list.path, residuals, SimplifiedCfg::False)
            } else if list.path.is_ident("not") {
                let [value] = values.as_slice() else {
                    return Err(format!(
                        "cfg `not` predicate must have exactly one argument; found {}",
                        values.len()
                    ));
                };
                match value {
                    SimplifiedCfg::False => Ok(SimplifiedCfg::True),
                    SimplifiedCfg::True => Ok(SimplifiedCfg::False),
                    SimplifiedCfg::Residual(meta) => {
                        let meta = syn::parse2(quote::quote!(not(#meta))).map_err(|error| {
                            format!("canonicalize cfg `not` predicate: {error}")
                        })?;
                        Ok(SimplifiedCfg::Residual(Box::new(meta)))
                    }
                }
            } else {
                Ok(SimplifiedCfg::Residual(Box::new(meta.clone())))
            }
        }
    }
}

fn simplify_cfg_list(
    path: &syn::Path,
    residuals: Vec<syn::Meta>,
    empty: SimplifiedCfg,
) -> Result<SimplifiedCfg, String> {
    match residuals.as_slice() {
        [] => Ok(empty),
        [only] => Ok(SimplifiedCfg::Residual(Box::new(only.clone()))),
        _ => {
            let meta = syn::parse2(quote::quote!(#path(#(#residuals),*)))
                .map_err(|error| format!("canonicalize cfg predicate: {error}"))?;
            Ok(SimplifiedCfg::Residual(Box::new(meta)))
        }
    }
}

fn normalize_production_attributes(attributes: &mut Vec<syn::Attribute>) -> Result<bool, String> {
    normalize_production_attributes_depth(attributes, 0)
}

fn normalize_production_attributes_depth(
    attributes: &mut Vec<syn::Attribute>,
    depth: usize,
) -> Result<bool, String> {
    use syn::parse::Parser;

    if depth > 16 {
        return Err("cfg_attr nesting exceeds the supported depth of 16".to_owned());
    }
    let mut normalized = Vec::with_capacity(attributes.len());
    for attribute in std::mem::take(attributes) {
        if attribute.path().is_ident("doc") {
            continue;
        } else if attribute.path().is_ident("cfg") {
            let predicate = attribute
                .parse_args::<syn::Meta>()
                .map_err(|error| format!("parse `{}`: {error}", attribute.to_token_stream()))?;
            match simplify_cfg_without_test(&predicate)? {
                SimplifiedCfg::False => return Ok(false),
                SimplifiedCfg::True => {}
                SimplifiedCfg::Residual(predicate) => {
                    normalized.extend(
                        syn::Attribute::parse_outer
                            .parse2(quote::quote!(#[cfg(#predicate)]))
                            .map_err(|error| {
                                format!("canonicalize `{}`: {error}", attribute.to_token_stream())
                            })?,
                    );
                }
            }
        } else if attribute.path().is_ident("cfg_attr") {
            let arguments =
                syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated
                    .parse2(match &attribute.meta {
                        syn::Meta::List(list) => list.tokens.clone(),
                        _ => {
                            return Err(format!(
                                "`{}` must be a cfg_attr list",
                                attribute.to_token_stream()
                            ));
                        }
                    })
                    .map_err(|error| format!("parse `{}`: {error}", attribute.to_token_stream()))?;
            let mut arguments = arguments.into_iter();
            let condition = arguments.next().ok_or_else(|| {
                format!(
                    "`{}` must contain a cfg_attr condition",
                    attribute.to_token_stream()
                )
            })?;
            let nested = arguments.collect::<Vec<_>>();
            if nested.is_empty() {
                return Err(format!(
                    "`{}` must contain at least one conditional attribute",
                    attribute.to_token_stream()
                ));
            }
            match simplify_cfg_without_test(&condition)? {
                SimplifiedCfg::False => {}
                SimplifiedCfg::True => {
                    let mut expanded = Vec::new();
                    for meta in nested {
                        expanded.extend(
                            syn::Attribute::parse_outer
                                .parse2(quote::quote!(#[#meta]))
                                .map_err(|error| {
                                    format!(
                                        "expand production `{}`: {error}",
                                        attribute.to_token_stream()
                                    )
                                })?,
                        );
                    }
                    if !normalize_production_attributes_depth(&mut expanded, depth + 1)? {
                        return Ok(false);
                    }
                    normalized.extend(expanded);
                }
                SimplifiedCfg::Residual(condition) => {
                    let nested = nested
                        .into_iter()
                        .filter(|meta| !meta.path().is_ident("doc"))
                        .collect::<Vec<_>>();
                    if nested
                        .iter()
                        .any(|meta| meta.path().is_ident("cfg") || meta.path().is_ident("cfg_attr"))
                    {
                        return Err(format!(
                            "residual `{}` contains nested conditional attributes whose production semantics cannot be flattened safely",
                            attribute.to_token_stream()
                        ));
                    }
                    if nested.is_empty() {
                        continue;
                    }
                    normalized.extend(
                        syn::Attribute::parse_outer
                            .parse2(quote::quote!(#[cfg_attr(#condition, #(#nested),*)]))
                            .map_err(|error| {
                                format!("canonicalize `{}`: {error}", attribute.to_token_stream())
                            })?,
                    );
                }
            }
        } else {
            normalized.push(attribute);
        }
    }
    *attributes = normalized;
    Ok(true)
}

fn normalize_production_macro_tokens(
    tokens: proc_macro2::TokenStream,
) -> Result<proc_macro2::TokenStream, String> {
    use proc_macro2::{Delimiter, Group, TokenStream, TokenTree};
    use syn::parse::Parser;

    let tokens = tokens.into_iter().collect::<Vec<_>>();
    let mut normalized = TokenStream::new();
    let mut index = 0;
    while index < tokens.len() {
        if matches!(
            tokens.get(index),
            Some(TokenTree::Punct(punctuation)) if punctuation.as_char() == '#'
        ) && matches!(
            tokens.get(index + 1),
            Some(TokenTree::Group(group)) if group.delimiter() == Delimiter::Bracket
        ) {
            let TokenTree::Group(group) = &tokens[index + 1] else {
                unreachable!("matched bracket group");
            };
            let attribute_tokens = group.stream();
            let attributes = syn::Attribute::parse_outer
                .parse2(quote::quote!(#[#attribute_tokens]))
                .map_err(|error| {
                    format!(
                        "parse conditional attribute in macro token body `{}`: {error}",
                        group.stream()
                    )
                })?;
            let mut attributes = attributes;
            let had_cfg = attributes
                .iter()
                .any(|attribute| attribute.path().is_ident("cfg"));
            if !normalize_production_attributes(&mut attributes)? {
                if had_cfg {
                    return Err(format!(
                        "pure-test cfg macro fragment `#[{}]` cannot be removed safely from opaque macro syntax",
                        group.stream()
                    ));
                }
                index += 2;
                continue;
            }
            for attribute in attributes {
                attribute.to_tokens(&mut normalized);
            }
            index += 2;
            continue;
        }
        match tokens[index].clone() {
            TokenTree::Group(group) => {
                let mut replacement = Group::new(
                    group.delimiter(),
                    normalize_production_macro_tokens(group.stream())?,
                );
                replacement.set_span(group.span());
                normalized.extend([TokenTree::Group(replacement)]);
            }
            token => normalized.extend([token]),
        }
        index += 1;
    }
    Ok(normalized)
}

fn audit_no_test_conditionals(relative: &str, file: &syn::File) -> Result<(), String> {
    use syn::visit::Visit;

    struct Audit {
        conditional: Option<String>,
    }

    impl<'ast> Visit<'ast> for Audit {
        fn visit_attribute(&mut self, attribute: &'ast syn::Attribute) {
            if self.conditional.is_none()
                && matches!(
                    attribute
                        .path()
                        .get_ident()
                        .map(ToString::to_string)
                        .as_deref(),
                    Some("cfg" | "cfg_attr")
                )
                && syntax_contains_ident(attribute, "test")
            {
                self.conditional = Some(attribute.to_token_stream().to_string());
            }
            syn::visit::visit_attribute(self, attribute);
        }

        fn visit_macro(&mut self, item: &'ast syn::Macro) {
            if self.conditional.is_none()
                && let Ok(Some(conditional)) = first_macro_test_conditional(item.tokens.clone())
            {
                self.conditional = Some(conditional);
            }
            syn::visit::visit_macro(self, item);
        }
    }

    let mut audit = Audit { conditional: None };
    audit.visit_file(file);
    if let Some(conditional) = audit.conditional {
        return Err(format!(
            "{relative} production Rust AST retains unsupported test conditional `{conditional}`"
        ));
    }
    Ok(())
}

fn first_macro_test_conditional(
    tokens: proc_macro2::TokenStream,
) -> Result<Option<String>, String> {
    use proc_macro2::{Delimiter, TokenTree};
    use syn::parse::Parser;

    let tokens = tokens.into_iter().collect::<Vec<_>>();
    let mut index = 0;
    while index < tokens.len() {
        if matches!(
            tokens.get(index),
            Some(TokenTree::Punct(punctuation)) if punctuation.as_char() == '#'
        ) && matches!(
            tokens.get(index + 1),
            Some(TokenTree::Group(group)) if group.delimiter() == Delimiter::Bracket
        ) {
            let TokenTree::Group(group) = &tokens[index + 1] else {
                unreachable!("matched bracket group");
            };
            let attribute_tokens = group.stream();
            let attributes = syn::Attribute::parse_outer
                .parse2(quote::quote!(#[#attribute_tokens]))
                .map_err(|error| {
                    format!(
                        "parse conditional attribute while auditing macro token body `{}`: {error}",
                        group.stream()
                    )
                })?;
            if let Some(attribute) = attributes.iter().find(|attribute| {
                matches!(
                    attribute
                        .path()
                        .get_ident()
                        .map(ToString::to_string)
                        .as_deref(),
                    Some("cfg" | "cfg_attr")
                ) && syntax_contains_ident(*attribute, "test")
            }) {
                return Ok(Some(attribute.to_token_stream().to_string()));
            }
            index += 2;
            continue;
        }
        if let TokenTree::Group(group) = tokens[index].clone()
            && let Some(conditional) = first_macro_test_conditional(group.stream())?
        {
            return Ok(Some(conditional));
        }
        index += 1;
    }
    Ok(None)
}

struct ProductionAstNormalizer {
    relative: String,
    error: Option<String>,
}

impl ProductionAstNormalizer {
    fn new(relative: &str) -> Self {
        Self {
            relative: relative.to_owned(),
            error: None,
        }
    }

    fn finish(self) -> Result<(), String> {
        self.error.map_or(Ok(()), Err)
    }

    fn normalize_attributes(&mut self, attributes: &mut Vec<syn::Attribute>) -> bool {
        if self.error.is_some() {
            return false;
        }
        match normalize_production_attributes(attributes) {
            Ok(keep) => keep,
            Err(error) => {
                self.error = Some(format!("{} {error}", self.relative));
                false
            }
        }
    }

    fn normalize_fields(&mut self, fields: &mut syn::Fields) {
        match fields {
            syn::Fields::Named(fields) => {
                fields.named = std::mem::take(&mut fields.named)
                    .into_iter()
                    .filter_map(|mut field| {
                        self.normalize_attributes(&mut field.attrs).then_some(field)
                    })
                    .collect();
            }
            syn::Fields::Unnamed(fields) => {
                fields.unnamed = std::mem::take(&mut fields.unnamed)
                    .into_iter()
                    .filter_map(|mut field| {
                        self.normalize_attributes(&mut field.attrs).then_some(field)
                    })
                    .collect();
            }
            syn::Fields::Unit => {}
        }
    }
}

impl syn::visit_mut::VisitMut for ProductionAstNormalizer {
    fn visit_file_mut(&mut self, file: &mut syn::File) {
        if !self.normalize_attributes(&mut file.attrs) {
            self.error.get_or_insert_with(|| {
                format!(
                    "{} must not be an entirely test-only frozen Rust source",
                    self.relative
                )
            });
            return;
        }
        file.items.retain_mut(|item| {
            item_attributes_mut(item).is_none_or(|attributes| self.normalize_attributes(attributes))
        });
        syn::visit_mut::visit_file_mut(self, file);
    }

    fn visit_item_mod_mut(&mut self, item: &mut syn::ItemMod) {
        if let Some((_, items)) = &mut item.content {
            items.retain_mut(|item| {
                item_attributes_mut(item)
                    .is_none_or(|attributes| self.normalize_attributes(attributes))
            });
        }
        syn::visit_mut::visit_item_mod_mut(self, item);
    }

    fn visit_item_impl_mut(&mut self, item: &mut syn::ItemImpl) {
        item.items.retain_mut(|item| {
            impl_item_attributes_mut(item)
                .is_none_or(|attributes| self.normalize_attributes(attributes))
        });
        syn::visit_mut::visit_item_impl_mut(self, item);
    }

    fn visit_item_trait_mut(&mut self, item: &mut syn::ItemTrait) {
        item.items.retain_mut(|item| {
            trait_item_attributes_mut(item)
                .is_none_or(|attributes| self.normalize_attributes(attributes))
        });
        syn::visit_mut::visit_item_trait_mut(self, item);
    }

    fn visit_item_foreign_mod_mut(&mut self, item: &mut syn::ItemForeignMod) {
        item.items.retain_mut(|item| {
            foreign_item_attributes_mut(item)
                .is_none_or(|attributes| self.normalize_attributes(attributes))
        });
        syn::visit_mut::visit_item_foreign_mod_mut(self, item);
    }

    fn visit_item_struct_mut(&mut self, item: &mut syn::ItemStruct) {
        self.normalize_fields(&mut item.fields);
        syn::visit_mut::visit_item_struct_mut(self, item);
    }

    fn visit_item_enum_mut(&mut self, item: &mut syn::ItemEnum) {
        item.variants = std::mem::take(&mut item.variants)
            .into_iter()
            .filter_map(|mut variant| {
                self.normalize_attributes(&mut variant.attrs)
                    .then_some(variant)
            })
            .collect();
        for variant in &mut item.variants {
            self.normalize_fields(&mut variant.fields);
        }
        syn::visit_mut::visit_item_enum_mut(self, item);
    }

    fn visit_item_union_mut(&mut self, item: &mut syn::ItemUnion) {
        item.fields.named = std::mem::take(&mut item.fields.named)
            .into_iter()
            .filter_map(|mut field| self.normalize_attributes(&mut field.attrs).then_some(field))
            .collect();
        syn::visit_mut::visit_item_union_mut(self, item);
    }

    fn visit_block_mut(&mut self, block: &mut syn::Block) {
        block.stmts.retain_mut(|statement| {
            statement_attributes_mut(statement)
                .is_none_or(|attributes| self.normalize_attributes(attributes))
        });
        syn::visit_mut::visit_block_mut(self, block);
    }

    fn visit_expr_match_mut(&mut self, expression: &mut syn::ExprMatch) {
        expression
            .arms
            .retain_mut(|arm| self.normalize_attributes(&mut arm.attrs));
        syn::visit_mut::visit_expr_match_mut(self, expression);
    }

    fn visit_expr_mut(&mut self, expression: &mut syn::Expr) {
        if let Some(attributes) = expression_attributes_mut(expression)
            && !self.normalize_attributes(attributes)
        {
            self.error.get_or_insert_with(|| {
                format!(
                    "{} contains test-only cfg on an expression outside a removable statement",
                    self.relative
                )
            });
            return;
        }
        syn::visit_mut::visit_expr_mut(self, expression);
    }

    fn visit_macro_mut(&mut self, item: &mut syn::Macro) {
        match normalize_production_macro_tokens(item.tokens.clone()) {
            Ok(tokens) => item.tokens = tokens,
            Err(error) => {
                self.error = Some(format!("{} {error}", self.relative));
                return;
            }
        }
        syn::visit_mut::visit_macro_mut(self, item);
    }
}

fn item_attributes_mut(item: &mut syn::Item) -> Option<&mut Vec<syn::Attribute>> {
    match item {
        syn::Item::Const(item) => Some(&mut item.attrs),
        syn::Item::Enum(item) => Some(&mut item.attrs),
        syn::Item::ExternCrate(item) => Some(&mut item.attrs),
        syn::Item::Fn(item) => Some(&mut item.attrs),
        syn::Item::ForeignMod(item) => Some(&mut item.attrs),
        syn::Item::Impl(item) => Some(&mut item.attrs),
        syn::Item::Macro(item) => Some(&mut item.attrs),
        syn::Item::Mod(item) => Some(&mut item.attrs),
        syn::Item::Static(item) => Some(&mut item.attrs),
        syn::Item::Struct(item) => Some(&mut item.attrs),
        syn::Item::Trait(item) => Some(&mut item.attrs),
        syn::Item::TraitAlias(item) => Some(&mut item.attrs),
        syn::Item::Type(item) => Some(&mut item.attrs),
        syn::Item::Union(item) => Some(&mut item.attrs),
        syn::Item::Use(item) => Some(&mut item.attrs),
        syn::Item::Verbatim(_) => None,
        _ => None,
    }
}

fn impl_item_attributes_mut(item: &mut syn::ImplItem) -> Option<&mut Vec<syn::Attribute>> {
    match item {
        syn::ImplItem::Const(item) => Some(&mut item.attrs),
        syn::ImplItem::Fn(item) => Some(&mut item.attrs),
        syn::ImplItem::Type(item) => Some(&mut item.attrs),
        syn::ImplItem::Macro(item) => Some(&mut item.attrs),
        syn::ImplItem::Verbatim(_) => None,
        _ => None,
    }
}

fn trait_item_attributes_mut(item: &mut syn::TraitItem) -> Option<&mut Vec<syn::Attribute>> {
    match item {
        syn::TraitItem::Const(item) => Some(&mut item.attrs),
        syn::TraitItem::Fn(item) => Some(&mut item.attrs),
        syn::TraitItem::Type(item) => Some(&mut item.attrs),
        syn::TraitItem::Macro(item) => Some(&mut item.attrs),
        syn::TraitItem::Verbatim(_) => None,
        _ => None,
    }
}

fn foreign_item_attributes_mut(item: &mut syn::ForeignItem) -> Option<&mut Vec<syn::Attribute>> {
    match item {
        syn::ForeignItem::Fn(item) => Some(&mut item.attrs),
        syn::ForeignItem::Static(item) => Some(&mut item.attrs),
        syn::ForeignItem::Type(item) => Some(&mut item.attrs),
        syn::ForeignItem::Macro(item) => Some(&mut item.attrs),
        syn::ForeignItem::Verbatim(_) => None,
        _ => None,
    }
}

fn statement_attributes_mut(statement: &mut syn::Stmt) -> Option<&mut Vec<syn::Attribute>> {
    match statement {
        syn::Stmt::Local(statement) => Some(&mut statement.attrs),
        syn::Stmt::Item(item) => item_attributes_mut(item),
        syn::Stmt::Expr(expression, _) => expression_attributes_mut(expression),
        syn::Stmt::Macro(statement) => Some(&mut statement.attrs),
    }
}

fn expression_attributes_mut(expression: &mut syn::Expr) -> Option<&mut Vec<syn::Attribute>> {
    match expression {
        syn::Expr::Array(expression) => Some(&mut expression.attrs),
        syn::Expr::Assign(expression) => Some(&mut expression.attrs),
        syn::Expr::Async(expression) => Some(&mut expression.attrs),
        syn::Expr::Await(expression) => Some(&mut expression.attrs),
        syn::Expr::Binary(expression) => Some(&mut expression.attrs),
        syn::Expr::Block(expression) => Some(&mut expression.attrs),
        syn::Expr::Break(expression) => Some(&mut expression.attrs),
        syn::Expr::Call(expression) => Some(&mut expression.attrs),
        syn::Expr::Cast(expression) => Some(&mut expression.attrs),
        syn::Expr::Closure(expression) => Some(&mut expression.attrs),
        syn::Expr::Const(expression) => Some(&mut expression.attrs),
        syn::Expr::Continue(expression) => Some(&mut expression.attrs),
        syn::Expr::Field(expression) => Some(&mut expression.attrs),
        syn::Expr::ForLoop(expression) => Some(&mut expression.attrs),
        syn::Expr::Group(expression) => Some(&mut expression.attrs),
        syn::Expr::If(expression) => Some(&mut expression.attrs),
        syn::Expr::Index(expression) => Some(&mut expression.attrs),
        syn::Expr::Infer(expression) => Some(&mut expression.attrs),
        syn::Expr::Let(expression) => Some(&mut expression.attrs),
        syn::Expr::Lit(expression) => Some(&mut expression.attrs),
        syn::Expr::Loop(expression) => Some(&mut expression.attrs),
        syn::Expr::Macro(expression) => Some(&mut expression.attrs),
        syn::Expr::Match(expression) => Some(&mut expression.attrs),
        syn::Expr::MethodCall(expression) => Some(&mut expression.attrs),
        syn::Expr::Paren(expression) => Some(&mut expression.attrs),
        syn::Expr::Path(expression) => Some(&mut expression.attrs),
        syn::Expr::Range(expression) => Some(&mut expression.attrs),
        syn::Expr::RawAddr(expression) => Some(&mut expression.attrs),
        syn::Expr::Reference(expression) => Some(&mut expression.attrs),
        syn::Expr::Repeat(expression) => Some(&mut expression.attrs),
        syn::Expr::Return(expression) => Some(&mut expression.attrs),
        syn::Expr::Struct(expression) => Some(&mut expression.attrs),
        syn::Expr::Try(expression) => Some(&mut expression.attrs),
        syn::Expr::TryBlock(expression) => Some(&mut expression.attrs),
        syn::Expr::Tuple(expression) => Some(&mut expression.attrs),
        syn::Expr::Unary(expression) => Some(&mut expression.attrs),
        syn::Expr::Unsafe(expression) => Some(&mut expression.attrs),
        syn::Expr::While(expression) => Some(&mut expression.attrs),
        syn::Expr::Yield(expression) => Some(&mut expression.attrs),
        syn::Expr::Verbatim(_) => None,
        _ => None,
    }
}

fn describe_source_route_witness(
    workspace_root: &Path,
    spec: SourceRouteWitnessSpec,
) -> Result<SourceRouteWitnessDescriptor, String> {
    let bytes = read_regular_file(workspace_root, spec.path)?;
    let source = std::str::from_utf8(&bytes)
        .map_err(|error| format!("{} must be UTF-8 Rust source: {error}", spec.path))?;
    let routes = validate_source_route_source(spec.path, source, spec)?;
    let canonical = canonical_json_bytes(&routes)?;
    Ok(SourceRouteWitnessDescriptor {
        role: spec.role.to_owned(),
        path: spec.path.to_owned(),
        routes,
        sha256: sha256_hex(&canonical),
    })
}

fn describe_impl_resolution_witness(
    workspace_root: &Path,
) -> Result<ImplResolutionWitnessDescriptor, String> {
    describe_impl_resolution_witness_excluding_paths(workspace_root, &[], &[], &BTreeSet::new())
}

fn describe_impl_resolution_witness_excluding_paths(
    workspace_root: &Path,
    excluded_paths: &[&str],
    frozen_protected_self_types: &[String],
    frozen_protected_members: &BTreeSet<String>,
) -> Result<ImplResolutionWitnessDescriptor, String> {
    use syn::visit::Visit;

    fn impl_member_name(item: &syn::ImplItem) -> Option<String> {
        match item {
            syn::ImplItem::Const(item) => Some(normalized_identifier(&item.ident)),
            syn::ImplItem::Fn(item) => Some(normalized_identifier(&item.sig.ident)),
            syn::ImplItem::Type(item) => Some(normalized_identifier(&item.ident)),
            syn::ImplItem::Macro(_) | syn::ImplItem::Verbatim(_) => None,
            _ => None,
        }
    }

    struct TypeDeclarationAudit {
        names: BTreeSet<String>,
    }

    impl<'ast> Visit<'ast> for TypeDeclarationAudit {
        fn visit_item_enum(&mut self, item: &'ast syn::ItemEnum) {
            self.names.insert(normalized_identifier(&item.ident));
            syn::visit::visit_item_enum(self, item);
        }

        fn visit_item_struct(&mut self, item: &'ast syn::ItemStruct) {
            self.names.insert(normalized_identifier(&item.ident));
            syn::visit::visit_item_struct(self, item);
        }

        fn visit_item_trait(&mut self, item: &'ast syn::ItemTrait) {
            self.names.insert(normalized_identifier(&item.ident));
            syn::visit::visit_item_trait(self, item);
        }

        fn visit_item_type(&mut self, item: &'ast syn::ItemType) {
            self.names.insert(normalized_identifier(&item.ident));
            syn::visit::visit_item_type(self, item);
        }

        fn visit_item_union(&mut self, item: &'ast syn::ItemUnion) {
            self.names.insert(normalized_identifier(&item.ident));
            syn::visit::visit_item_union(self, item);
        }
    }

    struct ImplAudit<'a> {
        relative: &'a str,
        protected: &'a BTreeSet<String>,
        local_nominals: &'a BTreeSet<String>,
        aliases: &'a BTreeSet<String>,
        protected_members: &'a BTreeSet<String>,
        impls: Vec<ImplResolutionItemDescriptor>,
    }

    impl<'ast> Visit<'ast> for ImplAudit<'_> {
        fn visit_item_impl(&mut self, item: &'ast syn::ItemImpl) {
            let protected_receiver = self
                .protected
                .iter()
                .any(|name| syntax_contains_ident(item.self_ty.as_ref(), name));
            let protected_trait = item.trait_.as_ref().is_some_and(|(_, path, _)| {
                self.protected
                    .iter()
                    .any(|name| syntax_contains_ident(path, name))
            });
            let direct_unrelated_local_nominal = match item.self_ty.as_ref() {
                syn::Type::Path(path)
                    if path.qself.is_none()
                        && path.path.leading_colon.is_none()
                        && path.path.segments.len() == 1 =>
                {
                    let segment = path.path.segments.first().expect("one path segment");
                    let name = normalized_identifier(&segment.ident);
                    self.local_nominals.contains(&name)
                        && !self.aliases.contains(&name)
                        && !self.protected.contains(&name)
                }
                _ => false,
            };
            if item.trait_.is_none()
                && !protected_receiver
                && !protected_trait
                && direct_unrelated_local_nominal
            {
                syn::visit::visit_item_impl(self, item);
                return;
            }
            let mut header = item.clone();
            header.items.clear();
            let impl_header_sha256 = sha256_hex(compact_tokens(&header).as_bytes());
            if let Some((_, trait_path, _)) = &item.trait_ {
                self.impls.push(ImplResolutionItemDescriptor {
                    path: self.relative.to_owned(),
                    self_type: compact_tokens(item.self_ty.as_ref()),
                    trait_path: Some(compact_tokens(trait_path)),
                    member: None,
                    impl_header_sha256,
                    ast_sha256: sha256_hex(compact_tokens(item).as_bytes()),
                });
            } else {
                for member in &item.items {
                    let member_name = impl_member_name(member);
                    let selected = member_name
                        .as_ref()
                        .is_some_and(|name| self.protected_members.contains(name))
                        || matches!(member, syn::ImplItem::Macro(_));
                    if selected {
                        self.impls.push(ImplResolutionItemDescriptor {
                            path: self.relative.to_owned(),
                            self_type: compact_tokens(item.self_ty.as_ref()),
                            trait_path: None,
                            member: member_name.or_else(|| Some("<macro>".to_owned())),
                            impl_header_sha256: impl_header_sha256.clone(),
                            ast_sha256: sha256_hex(compact_tokens(member).as_bytes()),
                        });
                    }
                }
            }
            syn::visit::visit_item_impl(self, item);
        }
    }

    struct TypeAliasAudit {
        aliases: Vec<(String, syn::Type)>,
        use_aliases: Vec<(String, String)>,
        local_nominals: BTreeSet<String>,
    }

    impl<'ast> Visit<'ast> for TypeAliasAudit {
        fn visit_item_enum(&mut self, item: &'ast syn::ItemEnum) {
            self.local_nominals
                .insert(normalized_identifier(&item.ident));
            syn::visit::visit_item_enum(self, item);
        }

        fn visit_item_struct(&mut self, item: &'ast syn::ItemStruct) {
            self.local_nominals
                .insert(normalized_identifier(&item.ident));
            syn::visit::visit_item_struct(self, item);
        }

        fn visit_item_type(&mut self, item: &'ast syn::ItemType) {
            self.aliases
                .push((normalized_identifier(&item.ident), (*item.ty).clone()));
            syn::visit::visit_item_type(self, item);
        }

        fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
            let prefix = if item.leading_colon.is_some() {
                "::"
            } else {
                ""
            };
            let mut routes = Vec::new();
            flatten_use_tree(prefix, &item.tree, &mut routes);
            for route in routes {
                let Some((source, alias)) = route.rsplit_once(" as ") else {
                    continue;
                };
                let Some(source) = source.rsplit("::").next() else {
                    continue;
                };
                self.use_aliases.push((
                    normalized_identifier_spelling(alias),
                    normalized_identifier_spelling(source),
                ));
            }
            syn::visit::visit_item_use(self, item);
        }

        fn visit_item_union(&mut self, item: &'ast syn::ItemUnion) {
            self.local_nominals
                .insert(normalized_identifier(&item.ident));
            syn::visit::visit_item_union(self, item);
        }
    }

    struct ResolutionMemberAudit {
        names: BTreeSet<String>,
    }

    impl<'ast> Visit<'ast> for ResolutionMemberAudit {
        fn visit_expr_method_call(&mut self, expression: &'ast syn::ExprMethodCall) {
            self.names.insert(normalized_identifier(&expression.method));
            syn::visit::visit_expr_method_call(self, expression);
        }

        fn visit_expr_path(&mut self, expression: &'ast syn::ExprPath) {
            if (expression.qself.is_some() || expression.path.segments.len() > 1)
                && let Some(segment) = expression.path.segments.last()
            {
                self.names.insert(normalized_identifier(&segment.ident));
            }
            syn::visit::visit_expr_path(self, expression);
        }

        fn visit_type_path(&mut self, item: &'ast syn::TypePath) {
            if (item.qself.is_some() || item.path.segments.len() > 1)
                && let Some(segment) = item.path.segments.last()
            {
                self.names.insert(normalized_identifier(&segment.ident));
            }
            syn::visit::visit_type_path(self, item);
        }
    }

    let excluded_paths = excluded_paths.iter().copied().collect::<BTreeSet<_>>();
    let mut protected_paths = FROZEN_SOURCE_SPECS
        .iter()
        .map(|spec| spec.path)
        .chain(
            ROUTE_FACADE_BASELINE_SOURCES
                .iter()
                .copied()
                .filter(|relative| !relative.starts_with("crates/transport/")),
        )
        .chain(ENTRY_POINT_SPECS.iter().map(|spec| spec.source_path))
        .chain(SOURCE_ROUTE_WITNESS_SPECS.iter().map(|spec| spec.path))
        .chain([
            EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
            EVENT_STORE_STORE_SOURCE_RELATIVE,
            POST_CORE_CAPABILITIES_SOURCE_RELATIVE,
            POST_CORE_DISPATCHER_SOURCE_RELATIVE,
        ])
        .filter(|relative| !excluded_paths.contains(relative))
        .collect::<Vec<_>>();
    protected_paths.sort_unstable();
    protected_paths.dedup();

    let mut declaration_audit = TypeDeclarationAudit {
        names: BTreeSet::new(),
    };
    for relative in &protected_paths {
        let bytes = read_regular_file(workspace_root, relative)?;
        let file = parse_canonical_production_rust(relative, &bytes)?;
        declaration_audit.visit_file(&file);
    }
    declaration_audit
        .names
        .extend(frozen_protected_self_types.iter().cloned());
    if declaration_audit.names.is_empty() {
        return Err("protected v1 impl-resolution type inventory must not be empty".to_owned());
    }

    let mut source_paths = Vec::new();
    let mut alias_audit = TypeAliasAudit {
        aliases: Vec::new(),
        use_aliases: Vec::new(),
        local_nominals: BTreeSet::new(),
    };
    for root in IMPL_RESOLUTION_SOURCE_ROOTS {
        for relative in governed_regular_file_inventory(workspace_root, root)? {
            if excluded_paths.contains(relative.as_str()) {
                continue;
            }
            if !relative.ends_with(".rs") {
                return Err(format!(
                    "{root} impl-resolution source inventory may contain only Rust files; found {relative}"
                ));
            }
            let bytes = read_regular_file(workspace_root, &relative)?;
            let file = parse_canonical_production_rust(&relative, &bytes)?;
            alias_audit.visit_file(&file);
            source_paths.push(relative);
        }
    }
    loop {
        let mut newly_protected = alias_audit
            .aliases
            .iter()
            .filter(|(alias, target)| {
                !declaration_audit.names.contains(alias)
                    && declaration_audit
                        .names
                        .iter()
                        .any(|name| syntax_contains_ident(target, name))
            })
            .map(|(alias, _)| alias.clone())
            .collect::<Vec<_>>();
        newly_protected.extend(
            alias_audit
                .use_aliases
                .iter()
                .filter(|(alias, source)| {
                    !declaration_audit.names.contains(alias)
                        && declaration_audit.names.contains(source)
                })
                .map(|(alias, _)| alias.clone()),
        );
        newly_protected.sort();
        newly_protected.dedup();
        if newly_protected.is_empty() {
            break;
        }
        declaration_audit.names.extend(newly_protected);
    }

    let mut protected_member_audit = ResolutionMemberAudit {
        names: BTreeSet::new(),
    };
    let mut protected_member_paths = FROZEN_SOURCE_SPECS
        .iter()
        .map(|spec| spec.path)
        .chain([
            POST_CORE_EXTENSION_SOURCE_RELATIVE,
            POST_CORE_STORAGE_SOURCE_RELATIVE,
        ])
        .filter(|relative| !excluded_paths.contains(relative))
        .collect::<Vec<_>>();
    protected_member_paths.sort_unstable();
    protected_member_paths.dedup();
    for relative in protected_member_paths {
        let bytes = read_regular_file(workspace_root, relative)?;
        let file = parse_canonical_production_rust(relative, &bytes)?;
        protected_member_audit.visit_file(&file);
    }
    protected_member_audit
        .names
        .extend(frozen_protected_members.iter().cloned());

    if !excluded_paths.contains(EVENT_STORE_STORE_SOURCE_RELATIVE) {
        let store_bytes = read_regular_file(workspace_root, EVENT_STORE_STORE_SOURCE_RELATIVE)?;
        let store_file =
            parse_canonical_production_rust(EVENT_STORE_STORE_SOURCE_RELATIVE, &store_bytes)?;
        let store_free_functions = store_file
            .items
            .iter()
            .filter_map(|item| match item {
                syn::Item::Fn(function) => Some((function.sig.ident.to_string(), function)),
                _ => None,
            })
            .collect::<BTreeMap<_, _>>();
        let mut local_resolution_queue = VecDeque::new();
        for spec in RUST_ITEM_WITNESS_ROOT_SPECS {
            match spec.callable {
                RustWitnessCallable::Associated { owner, name } => {
                    let function = exact_associated_function(
                        EVENT_STORE_STORE_SOURCE_RELATIVE,
                        &store_file,
                        owner,
                        name,
                    )?;
                    protected_member_audit.visit_impl_item_fn(function);
                    local_resolution_queue.extend(
                        WitnessedFunction::Associated(function)
                            .collect_call_routes()
                            .into_iter()
                            .filter_map(|route| route.strip_prefix("fn:").map(str::to_owned))
                            .filter(|route| !route.contains("::"))
                            .filter(|route| store_free_functions.contains_key(route)),
                    );
                }
                RustWitnessCallable::Free { name } => {
                    local_resolution_queue.push_back(name.to_owned());
                }
            }
        }
        let mut visited_local_resolution_functions = BTreeSet::new();
        while let Some(name) = local_resolution_queue.pop_front() {
            if !visited_local_resolution_functions.insert(name.clone()) {
                continue;
            }
            let function = store_free_functions.get(&name).ok_or_else(|| {
                format!(
                    "{EVENT_STORE_STORE_SOURCE_RELATIVE} v1 resolution closure references missing local function `{name}`"
                )
            })?;
            protected_member_audit.visit_item_fn(function);
            local_resolution_queue.extend(
                WitnessedFunction::Free(function)
                    .collect_call_routes()
                    .into_iter()
                    .filter_map(|route| route.strip_prefix("fn:").map(str::to_owned))
                    .filter(|route| !route.contains("::"))
                    .filter(|route| store_free_functions.contains_key(route)),
            );
        }
    }
    for spec in ENTRY_POINT_SPECS {
        if let CallableSpec::Associated { name, .. } = spec.callable {
            protected_member_audit.names.insert(name.to_owned());
        }
    }
    for spec in RUST_ITEM_WITNESS_ROOT_SPECS {
        if let RustWitnessCallable::Associated { name, .. } = spec.callable {
            protected_member_audit.names.insert(name.to_owned());
        }
        protected_member_audit.names.extend(
            spec.required_call_sequence
                .iter()
                .filter_map(|route| route.rsplit([':', '.']).next())
                .map(str::to_owned),
        );
    }

    let mut impls = Vec::new();
    let mut alias_names = alias_audit
        .aliases
        .iter()
        .map(|(alias, _)| alias.clone())
        .collect::<BTreeSet<_>>();
    alias_names.extend(
        alias_audit
            .use_aliases
            .iter()
            .map(|(alias, _)| alias.clone()),
    );
    for relative in source_paths {
        let bytes = read_regular_file(workspace_root, &relative)?;
        let file = parse_canonical_production_rust(&relative, &bytes)?;
        let mut audit = ImplAudit {
            relative: &relative,
            protected: &declaration_audit.names,
            local_nominals: &alias_audit.local_nominals,
            aliases: &alias_names,
            protected_members: &protected_member_audit.names,
            impls: Vec::new(),
        };
        audit.visit_file(&file);
        impls.append(&mut audit.impls);
    }
    impls.sort();
    let sha256 = sha256_hex(&canonical_json_bytes(&impls)?);
    Ok(ImplResolutionWitnessDescriptor {
        algorithm: IMPL_RESOLUTION_WITNESS_ALGORITHM.to_owned(),
        roots: IMPL_RESOLUTION_SOURCE_ROOTS
            .iter()
            .map(|root| (*root).to_owned())
            .collect(),
        protected_self_types: declaration_audit.names.into_iter().collect(),
        impls,
        sha256,
    })
}

fn normalized_identifier(identifier: &syn::Ident) -> String {
    normalized_identifier_spelling(&identifier.to_string())
}

fn normalized_identifier_spelling(spelling: &str) -> String {
    spelling.strip_prefix("r#").unwrap_or(spelling).to_owned()
}

fn validate_source_route_source(
    relative: &str,
    source: &str,
    spec: SourceRouteWitnessSpec,
) -> Result<Vec<String>, String> {
    if matches!(
        relative,
        "crates/event/src/lib.rs"
            | "crates/event_codec/src/lib.rs"
            | "crates/blossom/src/lib.rs"
            | EVENT_STORE_LIB_SOURCE_RELATIVE
    ) {
        validate_raw_crate_attributes(relative, source)?;
    }
    let file = parse_canonical_production_rust(relative, source.as_bytes())?;
    let mut routes = Vec::new();
    for module in spec.modules {
        validate_module_route(relative, &file, *module)?;
        routes.push(format!("mod:{}:{}", module.visibility.label(), module.name));
    }
    let actual_uses = collect_top_level_use_routes_with_attributes(&file);
    for expected in spec.uses {
        let matches = actual_uses
            .iter()
            .filter(|(_, path, _)| path == expected.path)
            .collect::<Vec<_>>();
        let [(visibility, _, attributes)] = matches.as_slice() else {
            return Err(format!(
                "{} must contain exactly one structured use route `{}`; found {}",
                relative,
                expected.path,
                matches.len()
            ));
        };
        if *visibility != expected.visibility {
            return Err(format!(
                "{} use route `{}` must have {} visibility",
                relative,
                expected.path,
                expected.visibility.label()
            ));
        }
        let expected_attributes = expected_source_use_route_attributes(relative);
        if attributes.as_slice() != expected_attributes {
            return Err(format!(
                "{relative} use route `{}` attributes drifted: expected {expected_attributes:?}, found {attributes:?}",
                expected.path
            ));
        }
        routes.push(format!(
            "use:{}:{}",
            expected.visibility.label(),
            expected.path
        ));
    }
    validate_source_route_resolution_authority(relative, &file, spec)?;
    Ok(routes)
}

fn validate_raw_crate_attributes(relative: &str, source: &str) -> Result<(), String> {
    let file =
        syn::parse_file(source).map_err(|error| format!("parse raw {relative} Rust: {error}"))?;
    let expected: &[&str] = match relative {
        "crates/event/src/lib.rs" => &[
            "#![cfg_attr(coverage_nightly,feature(coverage_attribute))]",
            "#![cfg_attr(all(not(feature=\"std\"),not(test)),no_std)]",
            "#![forbid(unsafe_code)]",
        ],
        "crates/event_codec/src/lib.rs" => &[
            "#![cfg_attr(all(not(feature=\"std\"),not(test)),no_std)]",
            "#![cfg_attr(coverage_nightly,feature(coverage_attribute))]",
            "#![forbid(unsafe_code)]",
        ],
        "crates/blossom/src/lib.rs" => &[
            "#![doc=include_str!(\"../README.md\")]",
            "#![cfg_attr(not(feature=\"std\"),no_std)]",
            "#![cfg_attr(coverage_nightly,feature(coverage_attribute))]",
            "#![forbid(unsafe_code)]",
        ],
        EVENT_STORE_LIB_SOURCE_RELATIVE => &[
            "#![cfg_attr(coverage_nightly,feature(coverage_attribute))]",
            "#![forbid(unsafe_code)]",
        ],
        _ => return Err(format!("unsupported raw crate-attribute target {relative}")),
    };
    let actual = file.attrs.iter().map(compact_tokens).collect::<Vec<_>>();
    if actual != expected {
        return Err(format!(
            "{relative} raw crate attributes drifted: expected {expected:?}, found {actual:?}"
        ));
    }
    Ok(())
}

fn validate_source_route_resolution_authority(
    relative: &str,
    file: &syn::File,
    spec: SourceRouteWitnessSpec,
) -> Result<(), String> {
    if matches!(
        relative,
        "crates/event/src/lib.rs" | "crates/event_codec/src/lib.rs" | "crates/blossom/src/lib.rs"
    ) {
        return validate_public_crate_root_resolution_authority(relative, file);
    }
    if relative == EVENT_STORE_LIB_SOURCE_RELATIVE {
        return validate_event_store_lib_witness_resolution_authority(relative, file);
    }

    let expected_modules = spec
        .modules
        .iter()
        .map(|module| module.name)
        .collect::<BTreeSet<_>>();
    let expected_uses = spec
        .uses
        .iter()
        .map(|item_use| item_use.path)
        .collect::<BTreeSet<_>>();
    let expected_use_bindings = spec
        .uses
        .iter()
        .filter_map(|item_use| use_route_local_binding(item_use.path))
        .collect::<BTreeSet<_>>();
    let protected_bindings = protected_resolution_bindings(file);

    for item in &file.items {
        match item {
            syn::Item::Mod(module)
                if !expected_modules.contains(module.ident.to_string().as_str()) =>
            {
                if protected_bindings.contains(&module.ident.to_string()) {
                    return Err(format!(
                        "{relative} unexpected module `{}` collides with a governed resolution binding",
                        module.ident
                    ));
                }
                if module.content.is_some() {
                    return Err(format!(
                        "{relative} unexpected module `{}` must not inject inline source",
                        module.ident
                    ));
                }
                if module.attrs.iter().any(module_attribute_can_retarget)
                    || module
                        .attrs
                        .iter()
                        .any(|attribute| !compact_tokens(attribute).starts_with("#[cfg(feature="))
                {
                    return Err(format!(
                        "{relative} unexpected module `{}` may use only feature gating and an implicit external source route",
                        module.ident
                    ));
                }
            }
            syn::Item::Use(item_use) => {
                let attributes = item_use
                    .attrs
                    .iter()
                    .map(compact_tokens)
                    .collect::<Vec<_>>();
                let prefix = if item_use.leading_colon.is_some() {
                    "::"
                } else {
                    ""
                };
                let mut paths = Vec::new();
                flatten_use_tree(prefix, &item_use.tree, &mut paths);
                for path in paths {
                    if expected_uses.contains(path.as_str()) {
                        continue;
                    }
                    if path.ends_with("::*") || path.contains(" as ") {
                        return Err(format!(
                            "{relative} unexpected use route `{path}` must not use glob or alias name resolution"
                        ));
                    }
                    if use_route_local_binding(&path)
                        .is_some_and(|binding| expected_use_bindings.contains(binding))
                    {
                        return Err(format!(
                            "{relative} unexpected use route `{path}` shadows a governed route binding"
                        ));
                    }
                    if !attributes.is_empty()
                        && !is_allowed_existing_conditional_source_use(
                            relative,
                            route_visibility(&item_use.vis),
                            &path,
                            &attributes,
                        )
                    {
                        return Err(format!(
                            "{relative} unexpected use route `{path}` must not introduce conditional resolution"
                        ));
                    }
                }
            }
            syn::Item::ExternCrate(item) => {
                let attributes = item.attrs.iter().map(compact_tokens).collect::<Vec<_>>();
                if relative != "crates/event_codec/src/profile/mod.rs"
                    || item.ident != "alloc"
                    || item.rename.is_some()
                    || attributes != ["#[cfg(not(feature=\"std\"))]"]
                {
                    return Err(format!(
                        "{relative} must not introduce extern-crate namespace `{}`",
                        item.ident
                    ));
                }
            }
            syn::Item::Macro(item) => {
                return Err(format!(
                    "{relative} must not introduce top-level macro source `{}`",
                    compact_tokens(item)
                ));
            }
            _ => {}
        }
    }
    Ok(())
}

fn validate_event_store_lib_witness_resolution_authority(
    relative: &str,
    file: &syn::File,
) -> Result<(), String> {
    let file_attributes = file.attrs.iter().map(compact_tokens).collect::<Vec<_>>();
    if file_attributes != ["#![forbid(unsafe_code)]"] {
        return Err(format!(
            "{relative} crate attributes drifted outside the v1 resolution witness"
        ));
    }
    if file
        .items
        .iter()
        .any(|item| !matches!(item, syn::Item::Mod(_) | syn::Item::Use(_)))
    {
        return Err(format!(
            "{relative} may contain only private module routes and public reexports"
        ));
    }

    let mut module_names = BTreeSet::new();
    let mut local_bindings = BTreeSet::new();
    for item in &file.items {
        match item {
            syn::Item::Mod(module) => {
                let name = normalized_identifier(&module.ident);
                if !module_names.insert(name.clone()) {
                    return Err(format!(
                        "{relative} contains duplicate production module route `{name}`"
                    ));
                }
                let attributes = module.attrs.iter().map(compact_tokens).collect::<Vec<_>>();
                if !is_inherited_visibility(&module.vis)
                    || module.content.is_some()
                    || module.attrs.iter().any(module_attribute_can_retarget)
                    || attributes != ["#[cfg(feature=\"sqlite\")]"]
                {
                    return Err(format!(
                        "{relative} module `{name}` must be a private non-retargeted external `sqlite` route"
                    ));
                }
            }
            syn::Item::Use(item_use) => {
                let attributes = item_use
                    .attrs
                    .iter()
                    .map(compact_tokens)
                    .collect::<Vec<_>>();
                if route_visibility(&item_use.vis) != Some(RouteVisibility::Public)
                    || item_use.leading_colon.is_some()
                    || attributes != ["#[cfg(feature=\"sqlite\")]"]
                {
                    return Err(format!(
                        "{relative} reexports must be public non-absolute `sqlite` feature routes"
                    ));
                }
                let mut routes = Vec::new();
                flatten_use_tree("", &item_use.tree, &mut routes);
                for route in routes {
                    if route.ends_with("::*") || route.contains(" as ") {
                        return Err(format!(
                            "{relative} reexport `{route}` must not use glob or alias resolution"
                        ));
                    }
                    if route.split_once("::").is_none() {
                        return Err(format!(
                            "{relative} reexport `{route}` must use a module-qualified path"
                        ));
                    }
                    let binding = use_route_local_binding(&route)
                        .ok_or_else(|| format!("{relative} reexport `{route}` has no binding"))?;
                    if !local_bindings.insert(binding.to_owned()) {
                        return Err(format!(
                            "{relative} reexports duplicate local binding `{binding}`"
                        ));
                    }
                }
            }
            _ => unreachable!("item inventory validated above"),
        }
    }

    let required_modules = [
        "error",
        "generated",
        "migrations",
        "model",
        "nip09",
        "schema",
        "store",
    ];
    if required_modules
        .iter()
        .any(|module| !module_names.contains(*module))
    {
        return Err(format!(
            "{relative} must retain every v1 event-store module route"
        ));
    }
    Ok(())
}

fn is_allowed_existing_conditional_source_use(
    relative: &str,
    visibility: Option<RouteVisibility>,
    path: &str,
    attributes: &[String],
) -> bool {
    (relative == "crates/event_codec/src/admission.rs"
        && visibility == Some(RouteVisibility::Inherited)
        && path == "alloc::boxed::Box"
        && attributes == ["#[cfg(not(feature=\"std\"))]"])
        || (relative == "crates/event_codec/src/verification.rs"
            && visibility == Some(RouteVisibility::Public)
            && path.starts_with("crate::knowledge::verification::")
            && attributes == ["#[cfg(feature=\"knowledge\")]"])
}

fn validate_public_crate_root_resolution_authority(
    relative: &str,
    file: &syn::File,
) -> Result<(), String> {
    let protected_bindings = protected_resolution_bindings(file);
    let expected_file_attributes: &[&str] = match relative {
        "crates/event/src/lib.rs"
        | "crates/event_codec/src/lib.rs"
        | "crates/blossom/src/lib.rs" => &["#![forbid(unsafe_code)]"],
        _ => {
            return Err(format!(
                "unsupported public crate-root audit target {relative}"
            ));
        }
    };
    let file_attributes = file.attrs.iter().map(compact_tokens).collect::<Vec<_>>();
    if file_attributes != expected_file_attributes {
        return Err(format!(
            "{relative} crate attributes drifted: expected {expected_file_attributes:?}, found {file_attributes:?}"
        ));
    }
    let expected_baseline_sha256 = match relative {
        "crates/event/src/lib.rs" => EVENT_CRATE_ROOT_BASELINE_SHA256,
        "crates/event_codec/src/lib.rs" => EVENT_CODEC_CRATE_ROOT_BASELINE_SHA256,
        "crates/blossom/src/lib.rs" => BLOSSOM_CRATE_ROOT_BASELINE_SHA256,
        _ => unreachable!("validated public crate-root audit target"),
    };
    let actual_baseline_sha256 = sha256_hex(prettyplease::unparse(file).as_bytes());
    if actual_baseline_sha256 != expected_baseline_sha256 {
        return Err(format!(
            "{relative} production crate-root baseline drifted: expected {expected_baseline_sha256}, found {actual_baseline_sha256}"
        ));
    }

    for item in &file.items {
        if !matches!(
            item,
            syn::Item::ExternCrate(_) | syn::Item::Macro(_) | syn::Item::Mod(_) | syn::Item::Use(_)
        ) {
            let attributes = item_attributes(item)
                .unwrap_or_default()
                .iter()
                .map(compact_tokens)
                .collect::<Vec<_>>();
            let expected_attributes: &[&str] = match item {
                syn::Item::Struct(item)
                    if relative == "crates/event/src/lib.rs"
                        && matches!(
                            item.ident.to_string().as_str(),
                            "RadrootsEventRef" | "RadrootsEventPtr"
                        ) =>
                {
                    &[
                        "#[cfg_attr(feature=\"serde\",derive(serde::Serialize,serde::Deserialize))]",
                        "#[cfg_attr(feature=\"dto-bindgen\",derive(dto_bindgen::Dto))]",
                        "#[cfg_attr(feature=\"dto-bindgen\",dto(export))]",
                        "#[derive(Clone,Debug,PartialEq,Eq)]",
                    ]
                }
                _ => &[],
            };
            if attributes != expected_attributes {
                return Err(format!(
                    "{relative} top-level item `{}` has unsupported source-generating attributes: expected {expected_attributes:?}, found {attributes:?}",
                    public_crate_root_item_label(item)
                ));
            }
        }
        match item {
            syn::Item::Macro(item) => {
                return Err(format!(
                    "{relative} must not define or invoke top-level macros outside governed source files: `{}`",
                    compact_tokens(item)
                ));
            }
            syn::Item::ExternCrate(item) => {
                let attributes = item.attrs.iter().map(compact_tokens).collect::<Vec<_>>();
                let expected_attributes: &[&str] = if relative == "crates/blossom/src/lib.rs" {
                    &[]
                } else {
                    &["#[cfg(not(feature=\"std\"))]"]
                };
                if item.ident != "alloc"
                    || item.rename.is_some()
                    || attributes != expected_attributes
                {
                    return Err(format!(
                        "{relative} permits only its exact governed `extern crate alloc` route"
                    ));
                }
            }
            syn::Item::Mod(module) => {
                let name = module.ident.to_string();
                if module.content.is_some()
                    || module.attrs.iter().any(module_attribute_can_retarget)
                    || module
                        .attrs
                        .iter()
                        .any(|attribute| attribute.path().is_ident("macro_use"))
                {
                    return Err(format!(
                        "{relative} module `{name}` must be a non-retargeted external route without macro import"
                    ));
                }
                if module
                    .attrs
                    .iter()
                    .any(|attribute| !compact_tokens(attribute).starts_with("#[cfg(feature="))
                {
                    return Err(format!(
                        "{relative} module `{name}` may use only a feature cfg attribute"
                    ));
                }
                if !specifically_governed_source_route_module(relative, &name)
                    && protected_bindings.contains(&name)
                {
                    return Err(format!(
                        "{relative} additional module `{name}` collides with a governed resolution binding"
                    ));
                }
            }
            syn::Item::Use(item_use) => {
                if item_use.leading_colon.is_some() {
                    return Err(format!(
                        "{relative} use routes must not change absolute-path resolution"
                    ));
                }
                let attributes = item_use
                    .attrs
                    .iter()
                    .map(compact_tokens)
                    .collect::<Vec<_>>();
                if attributes.iter().any(|attribute| {
                    !attribute.starts_with("#[cfg(feature=")
                        && attribute != "#[cfg(not(feature=\"std\"))]"
                }) {
                    return Err(format!(
                        "{relative} use routes may use only exact feature cfg attributes"
                    ));
                }
                let mut routes = Vec::new();
                flatten_use_tree("", &item_use.tree, &mut routes);
                for route in routes {
                    if route.ends_with("::*") || route.contains(" as ") {
                        return Err(format!(
                            "{relative} use route `{route}` must not use glob or alias name resolution"
                        ));
                    }
                    let _binding = use_route_local_binding(&route).ok_or_else(|| {
                        format!("{relative} use route `{route}` has no local binding")
                    })?;
                    let existing_private_alloc = relative == "crates/event/src/lib.rs"
                        && route.starts_with("alloc::")
                        && route_visibility(&item_use.vis) == Some(RouteVisibility::Inherited)
                        && attributes == ["#[cfg(not(feature=\"std\"))]"];
                    if route_visibility(&item_use.vis) == Some(RouteVisibility::Inherited)
                        && !existing_private_alloc
                    {
                        return Err(format!(
                            "{relative} additional private use route `{route}` may alter child resolution"
                        ));
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn specifically_governed_source_route_module(relative: &str, name: &str) -> bool {
    SOURCE_ROUTE_WITNESS_SPECS
        .iter()
        .find(|spec| spec.path == relative)
        .is_some_and(|spec| spec.modules.iter().any(|module| module.name == name))
}

fn protected_resolution_bindings(file: &syn::File) -> BTreeSet<String> {
    use syn::visit::Visit;

    struct Collector {
        bindings: BTreeSet<String>,
    }

    impl<'ast> Visit<'ast> for Collector {
        fn visit_path(&mut self, path: &'ast syn::Path) {
            if let Some(segment) = path.segments.first() {
                self.bindings.insert(segment.ident.to_string());
            }
            syn::visit::visit_path(self, path);
        }
    }

    let mut collector = Collector {
        bindings: [
            "alloc",
            "core",
            "dto_bindgen",
            "getrandom",
            "hex",
            "jiff",
            "jiff_tzdb",
            "nostr",
            "radroots_blossom",
            "radroots_core",
            "radroots_event",
            "radroots_event_codec",
            "radroots_transport",
            "serde",
            "serde_json",
            "sha2",
            "sqlx",
            "std",
            "thiserror",
            "url_nostd",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
    };
    collector.visit_file(file);
    collector.bindings
}

fn item_attributes(item: &syn::Item) -> Option<&[syn::Attribute]> {
    match item {
        syn::Item::Const(item) => Some(&item.attrs),
        syn::Item::Enum(item) => Some(&item.attrs),
        syn::Item::ExternCrate(item) => Some(&item.attrs),
        syn::Item::Fn(item) => Some(&item.attrs),
        syn::Item::ForeignMod(item) => Some(&item.attrs),
        syn::Item::Impl(item) => Some(&item.attrs),
        syn::Item::Macro(item) => Some(&item.attrs),
        syn::Item::Mod(item) => Some(&item.attrs),
        syn::Item::Static(item) => Some(&item.attrs),
        syn::Item::Struct(item) => Some(&item.attrs),
        syn::Item::Trait(item) => Some(&item.attrs),
        syn::Item::TraitAlias(item) => Some(&item.attrs),
        syn::Item::Type(item) => Some(&item.attrs),
        syn::Item::Union(item) => Some(&item.attrs),
        syn::Item::Use(item) => Some(&item.attrs),
        syn::Item::Verbatim(_) => None,
        _ => None,
    }
}

fn public_crate_root_item_label(item: &syn::Item) -> String {
    match item {
        syn::Item::Const(item) => item.ident.to_string(),
        syn::Item::Enum(item) => item.ident.to_string(),
        syn::Item::Fn(item) => item.sig.ident.to_string(),
        syn::Item::Static(item) => item.ident.to_string(),
        syn::Item::Struct(item) => item.ident.to_string(),
        syn::Item::Trait(item) => item.ident.to_string(),
        syn::Item::TraitAlias(item) => item.ident.to_string(),
        syn::Item::Type(item) => item.ident.to_string(),
        syn::Item::Union(item) => item.ident.to_string(),
        _ => compact_tokens(item),
    }
}

fn validate_module_route(
    relative: &str,
    file: &syn::File,
    expected: ModuleRouteSpec,
) -> Result<(), String> {
    let modules = file
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Mod(module) if module.ident == expected.name => Some(module),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [module] = modules.as_slice() else {
        return Err(format!(
            "{relative} must contain exactly one module route `{}`; found {}",
            expected.name,
            modules.len()
        ));
    };
    if route_visibility(&module.vis) != Some(expected.visibility) {
        return Err(format!(
            "{relative} module route `{}` must have {} visibility",
            expected.name,
            expected.visibility.label()
        ));
    }
    if module.content.is_some() {
        return Err(format!(
            "{relative} module route `{}` must use the implicit external source file",
            expected.name
        ));
    }
    if module.attrs.iter().any(module_attribute_can_retarget) {
        return Err(format!(
            "{relative} module route `{}` must not use direct or conditional #[path] retargeting",
            expected.name
        ));
    }
    let actual_attributes = module.attrs.iter().map(compact_tokens).collect::<Vec<_>>();
    let expected_attributes = expected_source_module_route_attributes(relative, expected.name);
    if actual_attributes.as_slice() != expected_attributes {
        return Err(format!(
            "{relative} module route `{}` attributes drifted: expected {expected_attributes:?}, found {actual_attributes:?}",
            expected.name
        ));
    }
    Ok(())
}

fn expected_source_module_route_attributes(
    relative: &str,
    module: &str,
) -> &'static [&'static str] {
    match (relative, module) {
        ("crates/event_codec/src/lib.rs", "admission") => &["#[cfg(feature=\"serde_json\")]"],
        ("crates/event_codec/src/profile/mod.rs", "inbound") => &["#[cfg(feature=\"serde_json\")]"],
        (EVENT_STORE_LIB_SOURCE_RELATIVE, _) => &["#[cfg(feature=\"sqlite\")]"],
        _ => &[],
    }
}

fn expected_source_use_route_attributes(relative: &str) -> &'static [&'static str] {
    match relative {
        EVENT_STORE_LIB_SOURCE_RELATIVE => &["#[cfg(feature=\"sqlite\")]"],
        _ => &[],
    }
}

fn module_attribute_can_retarget(attribute: &syn::Attribute) -> bool {
    attribute.path().is_ident("path")
        || (attribute.path().is_ident("cfg_attr") && syntax_contains_ident(attribute, "path"))
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PrivilegedStoreCallSite {
    relative: String,
    function: String,
    route: String,
}

fn validate_exact_top_level_imports(
    relative: &str,
    file: &syn::File,
    expected: &[&str],
) -> Result<(), String> {
    let actual = file
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Use(item) => Some(compact_tokens(item)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let expected = expected
        .iter()
        .map(|item| compact_source_tokens(item))
        .collect::<Vec<_>>();
    if actual != expected {
        return Err(format!(
            "{relative} production top-level import authority drifted: expected {expected:?}, found {actual:?}"
        ));
    }
    Ok(())
}

fn validate_event_store_migrations_import_authority(
    relative: &str,
    file: &syn::File,
) -> Result<(), String> {
    validate_exact_top_level_imports(
        relative,
        file,
        &[
            "use crate::RadrootsEventStoreError;",
            "use crate::generated::food_availability_projection_manifest as food_manifest;",
            "use crate::generated::nip09_reconciliation_manifest as nip09_manifest;",
            "use crate::generated::source_maintenance_manifest;",
            "use sha2::{Digest, Sha256};",
            "use std::collections::BTreeSet;",
        ],
    )
}

fn validate_event_store_schema_import_authority(
    relative: &str,
    file: &syn::File,
) -> Result<(), String> {
    validate_exact_top_level_imports(
        relative,
        file,
        &[
            r#"use crate::migrations::{
                EVENT_STORE_LEDGER_CREATE_DDL, EVENT_STORE_LEDGER_DDL, EVENT_STORE_LEDGER_NAME,
                EVENT_STORE_MIGRATIONS, EventStoreMigration, EventStoreMigrationHook,
                RADROOTS_EVENT_STORE_SCHEMA_VERSION_CURRENT, RADROOTS_EVENT_STORE_SCHEMA_VERSION_MIN,
                is_event_store_governed_schema_name, is_event_store_owned_table_name,
                migration_for_version, sqlite_identifier_starts_with,
                validate_embedded_migration_registry, validate_migration_registry,
            };"#,
            "use crate::{RadrootsEventStoreError, RadrootsEventStoreRawSourceRebuildDriftV1};",
            "use sha2::{Digest, Sha256};",
            "use sqlx::{Row, Sqlite, SqliteConnection, SqlitePool, Transaction};",
            "use std::collections::{BTreeMap, BTreeSet};",
            r#"use crate::nip09::reconciliation_v1::{
                OsSourceGenerationProvider, ReconciliationCapacityLimits,
                SourceGenerationProvider, apply_reconciliation_hook,
                validate_active_hook_state_fast, validate_reconciliation_capacity,
            };"#,
            r#"use crate::source_maintenance_v1::{
                apply_source_maintenance_hook_v1,
                validate_no_persisted_ephemeral_raw_rows_v1,
                validate_source_capacity_authority_full_v1,
            };"#,
            r#"use crate::store::food_availability_projection_v1::{
                apply_food_availability_projection_hook_v1,
                validate_food_availability_projection_hook_state_fast_v1,
            };"#,
        ],
    )
}

pub(super) fn validate_current_event_store_successor_authority(
    workspace_root: &Path,
) -> Result<(), String> {
    validate_privileged_store_authority(workspace_root)?;

    let migrations_bytes =
        read_regular_file(workspace_root, EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE)?;
    let migrations =
        parse_canonical_production_rust(EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE, &migrations_bytes)?;
    validate_event_store_migrations_import_authority(
        EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
        &migrations,
    )?;
    let expected_inputs =
        expected_event_store_migration_compiler_inputs(workspace_root, &migrations)?;
    validate_compiler_macro_inputs(
        EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
        &migrations,
        &expected_inputs,
    )?;
    validate_migration_registry_reachability(EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE, &migrations)?;
    validate_manifest_validator_reachability(EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE, &migrations)?;
    validate_source_maintenance_manifest_validator_reachability(
        EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
        &migrations,
    )?;
    validate_event_store_schema_name_matchers(EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE, &migrations)?;
    validate_source_maintenance_migration_bindings(
        EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
        &migrations,
    )?;
    validate_event_store_migration_support_authority(
        EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
        &migrations,
    )?;

    let schema_bytes = read_regular_file(workspace_root, EVENT_STORE_SCHEMA_SOURCE_RELATIVE)?;
    let schema =
        parse_canonical_production_rust(EVENT_STORE_SCHEMA_SOURCE_RELATIVE, &schema_bytes)?;
    validate_event_store_schema_import_authority(EVENT_STORE_SCHEMA_SOURCE_RELATIVE, &schema)?;
    validate_schema_runtime_reachability(EVENT_STORE_SCHEMA_SOURCE_RELATIVE, &schema)?;
    validate_schema_migration_execution_authority(EVENT_STORE_SCHEMA_SOURCE_RELATIVE, &schema)?;
    validate_source_maintenance_schema_dispatch(EVENT_STORE_SCHEMA_SOURCE_RELATIVE, &schema)?;
    validate_source_generation_rollback_authority(EVENT_STORE_SCHEMA_SOURCE_RELATIVE, &schema)?;

    let store_bytes = read_regular_file(workspace_root, EVENT_STORE_STORE_SOURCE_RELATIVE)?;
    let store = parse_canonical_production_rust(EVENT_STORE_STORE_SOURCE_RELATIVE, &store_bytes)?;
    validate_sqlite_encoding_preflight_authority(EVENT_STORE_STORE_SOURCE_RELATIVE, &store)?;
    validate_source_maintenance_runtime_token_authority(workspace_root)
}

fn validate_privileged_store_authority(workspace_root: &Path) -> Result<(), String> {
    let lib_bytes = read_regular_file(workspace_root, EVENT_STORE_LIB_SOURCE_RELATIVE)?;
    let lib_source = std::str::from_utf8(&lib_bytes).map_err(|error| {
        format!("{EVENT_STORE_LIB_SOURCE_RELATIVE} must be UTF-8 Rust source: {error}")
    })?;
    validate_raw_crate_attributes(EVENT_STORE_LIB_SOURCE_RELATIVE, lib_source)?;
    let lib = parse_canonical_production_rust(EVENT_STORE_LIB_SOURCE_RELATIVE, &lib_bytes)?;
    validate_event_store_lib_resolution_authority(EVENT_STORE_LIB_SOURCE_RELATIVE, &lib)?;
    validate_event_store_privileged_terminal_authority(workspace_root)?;

    let mut actual_module_sources =
        governed_regular_file_inventory(workspace_root, EVENT_STORE_STORE_MODULE_ROOT_RELATIVE)?;
    actual_module_sources.sort();
    if let Some(relative) = actual_module_sources
        .iter()
        .find(|relative| !relative.ends_with(".rs"))
    {
        return Err(format!(
            "{EVENT_STORE_STORE_MODULE_ROOT_RELATIVE} may contain only auditable Rust source files; found {relative}"
        ));
    }
    let expected_module_sources = PRIVILEGED_STORE_MODULE_SOURCES
        .into_iter()
        .chain(SUCCESSOR_08C_STORE_MODULE_SOURCES)
        .chain([RAW_SOURCE_REBUILD_TEST_SOURCE_RELATIVE])
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    let actual_module_sources = actual_module_sources.into_iter().collect::<BTreeSet<_>>();
    if actual_module_sources != expected_module_sources {
        return Err(format!(
            "{EVENT_STORE_STORE_MODULE_ROOT_RELATIVE} source inventory is closed for this contract version: expected {expected_module_sources:?}, found {actual_module_sources:?}"
        ));
    }

    let mut source_paths = vec![EVENT_STORE_STORE_SOURCE_RELATIVE.to_owned()];
    source_paths.extend(PRIVILEGED_STORE_MODULE_SOURCES.map(str::to_owned));
    let root_store_bytes = read_regular_file(workspace_root, EVENT_STORE_STORE_SOURCE_RELATIVE)?;
    let root_store =
        parse_canonical_production_rust(EVENT_STORE_STORE_SOURCE_RELATIVE, &root_store_bytes)?;
    validate_privileged_store_module_routes(EVENT_STORE_STORE_SOURCE_RELATIVE, &root_store)?;
    let mut privileged_imports = Vec::new();
    let mut privileged_calls = Vec::new();
    for relative in source_paths {
        let bytes = read_regular_file(workspace_root, &relative)?;
        let file = parse_canonical_production_rust(&relative, &bytes)?;
        if relative == EVENT_STORE_STORE_SOURCE_RELATIVE {
            validate_privileged_store_module_routes(&relative, &file)?;
        }
        privileged_imports.extend(collect_privileged_store_imports(&relative, &file)?);
        let mut audit = PrivilegedStoreReferenceAudit {
            relative: &relative,
            current_function: None,
            direct_privileged_callee: false,
            call_sites: Vec::new(),
            error: None,
        };
        syn::visit::Visit::visit_file(&mut audit, &file);
        privileged_calls.extend(audit.finish()?);
    }

    let expected_imports = [
        (
            EVENT_STORE_STORE_SOURCE_RELATIVE,
            "self::post_core_extension_capabilities::PostCoreExtensionCapabilities",
        ),
        (
            EVENT_STORE_STORE_SOURCE_RELATIVE,
            "self::post_core_extension_dispatcher::dispatch_post_core_extensions",
        ),
        (
            EVENT_STORE_STORE_SOURCE_RELATIVE,
            "self::protocol_reconciliation_v1::ingest_event_protocol_reconciliation_v1",
        ),
        (
            EVENT_STORE_STORE_SOURCE_RELATIVE,
            "self::protocol_reconciliation_v1::validate_protocol_post_extensions",
        ),
        (
            EVENT_STORE_STORE_SOURCE_RELATIVE,
            "self::protocol_storage_v1::raw_head_snapshot_in_transaction",
        ),
        (
            EVENT_STORE_STORE_SOURCE_RELATIVE,
            "self::protocol_storage_v1::stored_raw_event_from_row",
        ),
        (
            POST_CORE_CAPABILITIES_SOURCE_RELATIVE,
            "super::post_core_extensions_v1::apply_post_core_extensions_v1",
        ),
        (
            POST_CORE_CAPABILITIES_SOURCE_RELATIVE,
            "super::post_core_storage_v1::PostCoreStorageV1",
        ),
        (
            POST_CORE_CAPABILITIES_SOURCE_RELATIVE,
            "super::protocol_reconciliation_v1::ProtocolReconciliationV1IngestResult",
        ),
        (
            POST_CORE_DISPATCHER_SOURCE_RELATIVE,
            "super::post_core_extension_capabilities::PostCoreExtensionCapabilities",
        ),
        (
            POST_CORE_DISPATCHER_SOURCE_RELATIVE,
            "super::protocol_reconciliation_v1::ProtocolReconciliationV1IngestResult",
        ),
        (
            POST_CORE_EXTENSION_SOURCE_RELATIVE,
            "super::post_core_storage_v1::PostCoreStorageV1",
        ),
        (
            POST_CORE_EXTENSION_SOURCE_RELATIVE,
            "super::post_core_storage_v1::TradeProjectionWrite",
        ),
        (
            POST_CORE_EXTENSION_SOURCE_RELATIVE,
            "super::protocol_reconciliation_v1::ProtocolReconciliationV1IngestResult",
        ),
        (
            EVENT_STORE_PROTOCOL_RECONCILIATION_SOURCE_RELATIVE,
            "super::protocol_storage_v1::raw_head_snapshot_in_transaction",
        ),
        (
            EVENT_STORE_PROTOCOL_RECONCILIATION_SOURCE_RELATIVE,
            "super::protocol_storage_v1::stored_raw_event_from_row",
        ),
        (
            EVENT_STORE_PROTOCOL_RECONCILIATION_SOURCE_RELATIVE,
            "crate::source_maintenance_v1::advance_source_capacity_after_insert_v1",
        ),
        (
            EVENT_STORE_PROTOCOL_RECONCILIATION_SOURCE_RELATIVE,
            "crate::source_maintenance_v1::preflight_unique_raw_source_append_v1",
        ),
        (
            EVENT_STORE_PROTOCOL_RECONCILIATION_SOURCE_RELATIVE,
            "crate::source_maintenance_v1::raw_source_capacity_delta_v1",
        ),
        (
            EVENT_STORE_PROTOCOL_RECONCILIATION_SOURCE_RELATIVE,
            "crate::source_maintenance_v1::validate_source_capacity_authority_fast_v1",
        ),
    ]
    .into_iter()
    .map(|(relative, route)| (relative.to_owned(), route.to_owned()))
    .collect::<Vec<_>>();
    if privileged_imports != expected_imports {
        return Err(format!(
            "event-store privileged cross-module import routes drifted: expected {expected_imports:?}, found {privileged_imports:?}"
        ));
    }

    let expected_calls = [
        (
            EVENT_STORE_STORE_SOURCE_RELATIVE,
            "associated:source_capacity_v1",
            "crate::source_maintenance_v1::validate_source_capacity_authority_fast_v1",
        ),
        (
            EVENT_STORE_STORE_SOURCE_RELATIVE,
            "free:inspect_event_store_status",
            "crate::schema::validate_event_store_temp_schema",
        ),
        (
            EVENT_STORE_STORE_SOURCE_RELATIVE,
            "free:configure_pool",
            "validate_main_database_encoding",
        ),
        (
            EVENT_STORE_STORE_SOURCE_RELATIVE,
            "free:configure_pool",
            "crate::schema::validate_event_store_temp_schema",
        ),
        (
            EVENT_STORE_STORE_SOURCE_RELATIVE,
            "free:prepare_raw_source_repair_connection_v1",
            "validate_main_database_encoding",
        ),
        (
            EVENT_STORE_STORE_SOURCE_RELATIVE,
            "free:validate_raw_source_repair_canonical_lock_domain_v1",
            "validate_main_database_encoding",
        ),
        (
            EVENT_STORE_STORE_SOURCE_RELATIVE,
            "free:ingest_event_in_transaction",
            "crate::schema::validate_event_store_temp_schema",
        ),
        (
            EVENT_STORE_STORE_SOURCE_RELATIVE,
            "free:ingest_event_in_transaction",
            "ingest_event_protocol_reconciliation_v1",
        ),
        (
            EVENT_STORE_STORE_SOURCE_RELATIVE,
            "free:ingest_event_in_transaction",
            "PostCoreExtensionCapabilities::new",
        ),
        (
            EVENT_STORE_STORE_SOURCE_RELATIVE,
            "free:ingest_event_in_transaction",
            "dispatch_post_core_extensions",
        ),
        (
            EVENT_STORE_STORE_SOURCE_RELATIVE,
            "free:ingest_event_in_transaction",
            "validate_protocol_post_extensions",
        ),
        (
            POST_CORE_CAPABILITIES_SOURCE_RELATIVE,
            "associated:apply_v1",
            "PostCoreStorageV1::new",
        ),
        (
            POST_CORE_CAPABILITIES_SOURCE_RELATIVE,
            "associated:apply_v1",
            "apply_post_core_extensions_v1",
        ),
        (
            EVENT_STORE_PROTOCOL_RECONCILIATION_SOURCE_RELATIVE,
            "free:ingest_event_protocol_reconciliation_v1",
            "validate_source_capacity_authority_fast_v1",
        ),
        (
            EVENT_STORE_PROTOCOL_RECONCILIATION_SOURCE_RELATIVE,
            "free:ingest_event_protocol_reconciliation_v1",
            "raw_source_capacity_delta_v1",
        ),
        (
            EVENT_STORE_PROTOCOL_RECONCILIATION_SOURCE_RELATIVE,
            "free:ingest_event_protocol_reconciliation_v1",
            "preflight_unique_raw_source_append_v1",
        ),
        (
            EVENT_STORE_PROTOCOL_RECONCILIATION_SOURCE_RELATIVE,
            "free:ingest_event_protocol_reconciliation_v1",
            "advance_source_capacity_after_insert_v1",
        ),
        (
            EVENT_STORE_PROTOCOL_RECONCILIATION_SOURCE_RELATIVE,
            "free:read_protocol_post_extension_authority_seal",
            "validate_source_capacity_authority_fast_v1",
        ),
    ]
    .into_iter()
    .map(|(relative, function, route)| PrivilegedStoreCallSite {
        relative: relative.to_owned(),
        function: function.to_owned(),
        route: route.to_owned(),
    })
    .collect::<Vec<_>>();
    if privileged_calls != expected_calls {
        return Err(format!(
            "event-store privileged call-site cardinality or order drifted: expected {expected_calls:?}, found {privileged_calls:?}"
        ));
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct PrivilegedTerminalDefinition {
    relative: String,
    name: String,
}

fn validate_event_store_privileged_terminal_authority(workspace_root: &Path) -> Result<(), String> {
    let source_paths =
        governed_regular_file_inventory(workspace_root, EVENT_STORE_SOURCE_ROOT_RELATIVE)?
            .into_iter()
            .filter(|relative| relative.ends_with(".rs"))
            .collect::<Vec<_>>();
    let mut definitions = Vec::new();
    let mut calls = Vec::new();
    let mut imports = Vec::new();
    for relative in source_paths {
        let bytes = read_regular_file(workspace_root, &relative)?;
        let file = parse_canonical_production_rust(&relative, &bytes)?;
        if !SUCCESSOR_08C_EXCLUSIVE_SOURCE_PATHS.contains(&relative.as_str()) {
            let expected_macro_inputs = match relative.as_str() {
                EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE => {
                    expected_event_store_migration_compiler_inputs(workspace_root, &file)?
                }
                RESULT_VECTOR_EXECUTOR_RELATIVE => [
                    "include_bytes!(\"../../../tests/fixtures/nip09_reconciliation.v1.json\")",
                    "include_str!(\"../../../migrations/0001_event_store.up.sql\")",
                    "include_str!(\"../../../migrations/0002_nip09.up.sql\")",
                ]
                .map(str::to_owned)
                .to_vec(),
                RAW_SOURCE_REBUILD_TEST_SOURCE_RELATIVE => [
                    "include_bytes!(\"../../tests/fixtures/food_availability_projection.v1.json\")",
                    "include_bytes!(\"../../tests/fixtures/nip09_reconciliation.v1.json\")",
                ]
                .map(str::to_owned)
                .to_vec(),
                _ => Vec::new(),
            };
            validate_compiler_macro_inputs(&relative, &file, &expected_macro_inputs)?;
            validate_event_store_module_source_graph(&relative, &file)?;
            validate_event_store_trait_impl_authority(&relative, &file)?;
        }
        let mut audit = PrivilegedTerminalAudit {
            relative: &relative,
            current_function: None,
            direct_callee: false,
            scope_depth: 0,
            definitions: Vec::new(),
            calls: Vec::new(),
            imports: Vec::new(),
            error: None,
        };
        syn::visit::Visit::visit_file(&mut audit, &file);
        let PrivilegedTerminalAuthority {
            definitions: mut file_definitions,
            calls: mut file_calls,
            imports: mut file_imports,
        } = audit.finish()?;
        definitions.append(&mut file_definitions);
        calls.append(&mut file_calls);
        imports.append(&mut file_imports);
    }
    definitions.sort();
    calls.sort_by(|left, right| {
        (&left.relative, &left.function, &left.route).cmp(&(
            &right.relative,
            &right.function,
            &right.route,
        ))
    });
    imports.sort();

    let mut expected_imports = [
        (
            EVENT_STORE_STORE_SOURCE_RELATIVE,
            "self::post_core_extension_dispatcher::dispatch_post_core_extensions",
        ),
        (
            EVENT_STORE_STORE_SOURCE_RELATIVE,
            "self::protocol_reconciliation_v1::ingest_event_protocol_reconciliation_v1",
        ),
        (
            EVENT_STORE_STORE_SOURCE_RELATIVE,
            "self::protocol_reconciliation_v1::validate_protocol_post_extensions",
        ),
        (
            POST_CORE_CAPABILITIES_SOURCE_RELATIVE,
            "super::post_core_extensions_v1::apply_post_core_extensions_v1",
        ),
        (
            EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
            "crate::nip09::reconciliation_v1::validate_active_hook_state_fast",
        ),
        (
            EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
            "crate::source_maintenance_v1::apply_source_maintenance_hook_v1",
        ),
        (
            EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
            "crate::source_maintenance_v1::validate_no_persisted_ephemeral_raw_rows_v1",
        ),
        (
            EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
            "crate::source_maintenance_v1::validate_source_capacity_authority_full_v1",
        ),
        (
            EVENT_STORE_PROTOCOL_RECONCILIATION_SOURCE_RELATIVE,
            "crate::source_maintenance_v1::advance_source_capacity_after_insert_v1",
        ),
        (
            EVENT_STORE_PROTOCOL_RECONCILIATION_SOURCE_RELATIVE,
            "crate::source_maintenance_v1::preflight_unique_raw_source_append_v1",
        ),
        (
            EVENT_STORE_PROTOCOL_RECONCILIATION_SOURCE_RELATIVE,
            "crate::source_maintenance_v1::raw_source_capacity_delta_v1",
        ),
        (
            EVENT_STORE_PROTOCOL_RECONCILIATION_SOURCE_RELATIVE,
            "crate::source_maintenance_v1::validate_source_capacity_authority_fast_v1",
        ),
        (
            RAW_SOURCE_REBUILD_SOURCE_RELATIVE,
            "crate::source_maintenance_v1::preflight_source_generation_append_v1",
        ),
        (
            RAW_SOURCE_REBUILD_SOURCE_RELATIVE,
            "crate::source_maintenance_v1::validate_source_capacity_authority_fast_v1",
        ),
        (
            RAW_SOURCE_REBUILD_SOURCE_RELATIVE,
            "crate::source_maintenance_v1::validate_source_capacity_authority_full_v1",
        ),
        (
            RAW_SOURCE_REBUILD_SOURCE_RELATIVE,
            "super::validate_active_hook_state_fast",
        ),
    ]
    .into_iter()
    .map(|(relative, route)| (relative.to_owned(), route.to_owned()))
    .collect::<Vec<_>>();
    expected_imports.sort();
    if imports != expected_imports {
        return Err(format!(
            "event-store SourceMaintenance privileged import authority drifted: expected {expected_imports:?}, found {imports:?}"
        ));
    }

    let mut expected_definitions = [
        (EVENT_STORE_SCHEMA_SOURCE_RELATIVE, "apply_migration_down"),
        (EVENT_STORE_SCHEMA_SOURCE_RELATIVE, "apply_migration_hook"),
        (EVENT_STORE_SCHEMA_SOURCE_RELATIVE, "apply_migration_up"),
        (
            EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
            "validate_event_store_temp_schema",
        ),
        (
            EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
            "validate_migration_hook_state",
        ),
        (
            EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
            "validate_rollback_preserves_source_generation_history",
        ),
        (
            EVENT_STORE_STORE_SOURCE_RELATIVE,
            "validate_main_database_encoding",
        ),
        (
            "crates/event_store/src/nip09/reconciliation_v1.rs",
            "validate_active_hook_state_fast",
        ),
        (
            POST_CORE_DISPATCHER_SOURCE_RELATIVE,
            "dispatch_post_core_extensions",
        ),
        (
            POST_CORE_EXTENSION_SOURCE_RELATIVE,
            "apply_post_core_extensions_v1",
        ),
        (
            EVENT_STORE_PROTOCOL_RECONCILIATION_SOURCE_RELATIVE,
            "ingest_event_protocol_reconciliation_v1",
        ),
        (
            EVENT_STORE_PROTOCOL_RECONCILIATION_SOURCE_RELATIVE,
            "validate_protocol_post_extensions",
        ),
        (
            "crates/event_store/src/source_maintenance_v1.rs",
            "advance_source_capacity_after_insert_v1",
        ),
        (
            "crates/event_store/src/source_maintenance_v1.rs",
            "apply_source_maintenance_hook_v1",
        ),
        (
            "crates/event_store/src/source_maintenance_v1.rs",
            "bind_source_capacity_to_generation_v1",
        ),
        (
            "crates/event_store/src/source_maintenance_v1.rs",
            "preflight_source_generation_append_v1",
        ),
        (
            "crates/event_store/src/source_maintenance_v1.rs",
            "preflight_unique_raw_source_append_v1",
        ),
        (
            "crates/event_store/src/source_maintenance_v1.rs",
            "raw_source_capacity_delta_v1",
        ),
        (
            "crates/event_store/src/source_maintenance_v1.rs",
            "validate_no_persisted_ephemeral_raw_rows_v1",
        ),
        (
            "crates/event_store/src/source_maintenance_v1.rs",
            "validate_source_capacity_authority_fast_v1",
        ),
        (
            "crates/event_store/src/source_maintenance_v1.rs",
            "validate_source_capacity_authority_full_v1",
        ),
    ]
    .into_iter()
    .map(|(relative, name)| PrivilegedTerminalDefinition {
        relative: relative.to_owned(),
        name: name.to_owned(),
    })
    .collect::<Vec<_>>();
    expected_definitions.sort();
    if definitions != expected_definitions {
        return Err(format!(
            "event-store privileged terminal definitions drifted: expected {expected_definitions:?}, found {definitions:?}"
        ));
    }

    let mut expected_calls = [
        (
            EVENT_STORE_STORE_SOURCE_RELATIVE,
            "free:configure_pool",
            "validate_main_database_encoding",
        ),
        (
            EVENT_STORE_STORE_SOURCE_RELATIVE,
            "free:configure_pool",
            "crate::schema::validate_event_store_temp_schema",
        ),
        (
            EVENT_STORE_STORE_SOURCE_RELATIVE,
            "free:ingest_event_in_transaction",
            "PostCoreExtensionCapabilities::new",
        ),
        (
            EVENT_STORE_STORE_SOURCE_RELATIVE,
            "free:ingest_event_in_transaction",
            "dispatch_post_core_extensions",
        ),
        (
            EVENT_STORE_STORE_SOURCE_RELATIVE,
            "free:ingest_event_in_transaction",
            "crate::schema::validate_event_store_temp_schema",
        ),
        (
            EVENT_STORE_STORE_SOURCE_RELATIVE,
            "free:ingest_event_in_transaction",
            "ingest_event_protocol_reconciliation_v1",
        ),
        (
            EVENT_STORE_STORE_SOURCE_RELATIVE,
            "free:ingest_event_in_transaction",
            "validate_protocol_post_extensions",
        ),
        (
            EVENT_STORE_STORE_SOURCE_RELATIVE,
            "free:inspect_event_store_status",
            "crate::schema::validate_event_store_temp_schema",
        ),
        (
            EVENT_STORE_STORE_SOURCE_RELATIVE,
            "free:prepare_raw_source_repair_connection_v1",
            "validate_main_database_encoding",
        ),
        (
            EVENT_STORE_STORE_SOURCE_RELATIVE,
            "free:validate_raw_source_repair_canonical_lock_domain_v1",
            "validate_main_database_encoding",
        ),
        (
            POST_CORE_CAPABILITIES_SOURCE_RELATIVE,
            "associated:apply_v1",
            "PostCoreStorageV1::new",
        ),
        (
            POST_CORE_CAPABILITIES_SOURCE_RELATIVE,
            "associated:apply_v1",
            "apply_post_core_extensions_v1",
        ),
        (
            "crates/event_store/src/nip09/reconciliation_v1.rs",
            "free:apply_reconciliation_hook",
            "crate::source_maintenance_v1::bind_source_capacity_to_generation_v1",
        ),
        (
            "crates/event_store/src/nip09/reconciliation_v1.rs",
            "free:apply_reconciliation_hook",
            "crate::source_maintenance_v1::preflight_source_generation_append_v1",
        ),
        (
            "crates/event_store/src/nip09/reconciliation_v1.rs",
            "free:apply_reconciliation_hook",
            "validate_active_hook_state_fast",
        ),
        (
            EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
            "free:apply_migration_hook",
            "apply_source_maintenance_hook_v1",
        ),
        (
            EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
            "free:migrate_event_store_schema_with_registry_and_generation_provider",
            "validate_no_persisted_ephemeral_raw_rows_v1",
        ),
        (
            EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
            "free:migrate_schema_on_connection",
            "apply_migration_hook",
        ),
        (
            EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
            "free:migrate_schema_on_connection",
            "apply_migration_up",
        ),
        (
            EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
            "free:migrate_schema_on_connection",
            "apply_migration_up",
        ),
        (
            EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
            "free:migrate_schema_on_connection",
            "validate_no_persisted_ephemeral_raw_rows_v1",
        ),
        (
            EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
            "free:rollback_schema_on_connection",
            "apply_migration_down",
        ),
        (
            EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
            "free:rollback_schema_on_connection",
            "validate_rollback_preserves_source_generation_history",
        ),
        (
            EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
            "free:validate_applied_migration_hooks",
            "validate_migration_hook_state",
        ),
        (
            EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
            "free:validate_migration_hook_state",
            "validate_active_hook_state_fast",
        ),
        (
            EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
            "free:validate_migration_hook_state",
            "validate_source_capacity_authority_full_v1",
        ),
        (
            "crates/event_store/src/source_maintenance_v1.rs",
            "free:advance_source_capacity_after_insert_v1",
            "validate_source_capacity_authority_fast_v1",
        ),
        (
            "crates/event_store/src/source_maintenance_v1.rs",
            "free:apply_source_maintenance_hook_v1",
            "validate_no_persisted_ephemeral_raw_rows_v1",
        ),
        (
            "crates/event_store/src/source_maintenance_v1.rs",
            "free:apply_source_maintenance_hook_v1",
            "validate_source_capacity_authority_full_v1",
        ),
        (
            "crates/event_store/src/source_maintenance_v1.rs",
            "free:bind_source_capacity_to_generation_v1",
            "validate_source_capacity_authority_fast_v1",
        ),
        (
            "crates/event_store/src/source_maintenance_v1.rs",
            "free:preflight_source_generation_append_v1",
            "validate_source_capacity_authority_fast_v1",
        ),
        (
            "crates/event_store/src/source_maintenance_v1.rs",
            "free:preflight_unique_raw_source_append_v1",
            "validate_source_capacity_authority_fast_v1",
        ),
        (
            "crates/event_store/src/source_maintenance_v1.rs",
            "free:validate_source_capacity_authority_full_v1",
            "validate_no_persisted_ephemeral_raw_rows_v1",
        ),
        (
            "crates/event_store/src/source_maintenance_v1.rs",
            "free:validate_source_capacity_authority_full_v1",
            "validate_source_capacity_authority_fast_v1",
        ),
        (
            EVENT_STORE_STORE_SOURCE_RELATIVE,
            "associated:source_capacity_v1",
            "crate::source_maintenance_v1::validate_source_capacity_authority_fast_v1",
        ),
        (
            EVENT_STORE_PROTOCOL_RECONCILIATION_SOURCE_RELATIVE,
            "free:ingest_event_protocol_reconciliation_v1",
            "advance_source_capacity_after_insert_v1",
        ),
        (
            EVENT_STORE_PROTOCOL_RECONCILIATION_SOURCE_RELATIVE,
            "free:ingest_event_protocol_reconciliation_v1",
            "preflight_unique_raw_source_append_v1",
        ),
        (
            EVENT_STORE_PROTOCOL_RECONCILIATION_SOURCE_RELATIVE,
            "free:ingest_event_protocol_reconciliation_v1",
            "raw_source_capacity_delta_v1",
        ),
        (
            EVENT_STORE_PROTOCOL_RECONCILIATION_SOURCE_RELATIVE,
            "free:ingest_event_protocol_reconciliation_v1",
            "validate_source_capacity_authority_fast_v1",
        ),
        (
            EVENT_STORE_PROTOCOL_RECONCILIATION_SOURCE_RELATIVE,
            "free:read_protocol_post_extension_authority_seal",
            "validate_source_capacity_authority_fast_v1",
        ),
        (
            RAW_SOURCE_REBUILD_SOURCE_RELATIVE,
            "free:rebuild_from_raw_v1_in_transaction_inner",
            "crate::source_maintenance_v1::bind_source_capacity_to_generation_v1",
        ),
        (
            RAW_SOURCE_REBUILD_SOURCE_RELATIVE,
            "free:rebuild_from_raw_v1_in_transaction_inner",
            "preflight_source_generation_append_v1",
        ),
        (
            RAW_SOURCE_REBUILD_SOURCE_RELATIVE,
            "free:rebuild_from_raw_v1_in_transaction_inner",
            "validate_active_hook_state_fast",
        ),
        (
            RAW_SOURCE_REBUILD_SOURCE_RELATIVE,
            "free:rebuild_from_raw_v1_in_transaction_inner",
            "validate_source_capacity_authority_fast_v1",
        ),
        (
            RAW_SOURCE_REBUILD_SOURCE_RELATIVE,
            "free:rebuild_from_raw_v1_in_transaction_inner",
            "validate_source_capacity_authority_full_v1",
        ),
        (
            RAW_SOURCE_REBUILD_SOURCE_RELATIVE,
            "free:validate_source_lineage_for_rebuild_v1",
            "validate_source_capacity_authority_fast_v1",
        ),
    ]
    .into_iter()
    .map(|(relative, function, route)| PrivilegedStoreCallSite {
        relative: relative.to_owned(),
        function: function.to_owned(),
        route: route.to_owned(),
    })
    .collect::<Vec<_>>();
    expected_calls.sort_by(|left, right| {
        (&left.relative, &left.function, &left.route).cmp(&(
            &right.relative,
            &right.function,
            &right.route,
        ))
    });
    if calls != expected_calls {
        return Err(format!(
            "event-store privileged terminal call authority drifted: expected {expected_calls:?}, found {calls:?}"
        ));
    }
    Ok(())
}

fn validate_event_store_module_source_graph(
    relative: &str,
    file: &syn::File,
) -> Result<(), String> {
    if matches!(
        relative,
        EVENT_STORE_LIB_SOURCE_RELATIVE
            | EVENT_STORE_STORE_SOURCE_RELATIVE
            | "crates/event_store/src/generated.rs"
            | "crates/event_store/src/model.rs"
            | "crates/event_store/src/nip09.rs"
    ) {
        return Ok(());
    }

    use syn::visit::Visit;

    struct Audit {
        modules: Vec<String>,
    }

    impl<'ast> Visit<'ast> for Audit {
        fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
            self.modules.push(compact_tokens(item));
            syn::visit::visit_item_mod(self, item);
        }
    }

    let mut audit = Audit {
        modules: Vec::new(),
    };
    audit.visit_file(file);
    if relative == "crates/event_store/src/nip09/reconciliation_v1.rs" {
        let expected = ["modraw_source_rebuild;", "modvisibility_oracle_v1;"];
        if audit.modules == expected {
            return Ok(());
        }
        return Err(format!(
            "{relative} raw-source rebuild module graph drifted: expected {expected:?}, found {:?}",
            audit.modules
        ));
    }
    if !audit.modules.is_empty() {
        return Err(format!(
            "{relative} event-store production module source graph is closed outside governed facade roots; found {:?}",
            audit.modules
        ));
    }
    Ok(())
}

fn validate_event_store_trait_impl_authority(
    relative: &str,
    file: &syn::File,
) -> Result<(), String> {
    use syn::visit::Visit;

    struct Audit {
        trait_impls: Vec<(String, String)>,
        inherent_self_types: Vec<String>,
        inherent_impls: Vec<String>,
        item_macros: Vec<String>,
    }

    impl<'ast> Visit<'ast> for Audit {
        fn visit_item_impl(&mut self, item: &'ast syn::ItemImpl) {
            if let Some((_, trait_path, _)) = &item.trait_ {
                self.trait_impls.push((
                    compact_tokens(trait_path),
                    compact_tokens(item.self_ty.as_ref()),
                ));
            } else {
                self.inherent_self_types
                    .push(compact_tokens(item.self_ty.as_ref()));
                self.inherent_impls.push(compact_tokens(item));
            }
            syn::visit::visit_item_impl(self, item);
        }

        fn visit_item_macro(&mut self, item: &'ast syn::ItemMacro) {
            self.item_macros.push(compact_tokens(item));
            syn::visit::visit_item_macro(self, item);
        }
    }

    let mut audit = Audit {
        trait_impls: Vec::new(),
        inherent_self_types: Vec::new(),
        inherent_impls: Vec::new(),
        item_macros: Vec::new(),
    };
    audit.visit_file(file);
    let item_macro_authority_is_valid = if relative == EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE {
        matches!(
            audit.item_macros.as_slice(),
            [item] if item.starts_with("macro_rules!event_store_ledger_ddl")
        )
    } else {
        audit.item_macros.is_empty()
    };
    if !item_macro_authority_is_valid {
        return Err(format!(
            "{relative} event-store item macro authority is closed; found {:?}",
            audit.item_macros,
        ));
    }
    let expected: &[(&str, &str)] = match relative {
        "crates/event_store/src/error.rs" => &[
            (
                "core::fmt::Display",
                "RadrootsEventStoreSourceCapacityResourceV1",
            ),
            (
                "core::fmt::Display",
                "RadrootsEventStoreRawSourceRebuildDriftV1",
            ),
            (
                "core::fmt::Display",
                "RadrootsEventStoreCallerInboundForeignKeyV1",
            ),
            ("From<RadrootsTransportError>", "RadrootsEventStoreError"),
        ],
        "crates/event_store/src/nip09/reconciliation_v1.rs" => {
            &[("SourceGenerationProvider", "OsSourceGenerationProvider")]
        }
        "crates/event_store/src/model.rs" => &[
            ("AsRef<str>", "RadrootsTransportObservationMessage"),
            ("core::ops::Deref", "RadrootsTransportObservationMessage"),
        ],
        RESULT_VECTOR_EXECUTOR_RELATIVE => &[("SourceGenerationProvider", "FixedGeneration")],
        RAW_SOURCE_REBUILD_TEST_SOURCE_RELATIVE => &[
            ("SourceGenerationProvider", "FixedGeneration"),
            ("SourceGenerationProvider", "PanickingGeneration"),
            ("SourceGenerationProvider", "FailingGeneration"),
        ],
        _ => &[],
    };
    let actual_trait_impls = audit
        .trait_impls
        .iter()
        .map(|(trait_path, self_type)| (trait_path.as_str(), self_type.as_str()))
        .collect::<Vec<_>>();
    if actual_trait_impls != expected {
        return Err(format!(
            "{relative} event-store trait impl authority drifted: expected {expected:?}, found {actual_trait_impls:?}"
        ));
    }
    let expected_inherent_self_types: &[&str] = match relative {
        "crates/event_store/src/error.rs" => &[
            "RadrootsEventStoreSourceCapacityResourceV1",
            "RadrootsEventStoreRawSourceRebuildDriftV1",
        ],
        EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE => &[
            "EventStoreMigrationHook",
            "GeneratedManifestMetadataAxis<'_>",
        ],
        "crates/event_store/src/model.rs" => &[
            "RadrootsTransportObservationMessage",
            "RadrootsTransportObservationType",
            "RadrootsTransportObservation",
            "RadrootsStoredValidEvent",
            "RadrootsStoredVisibleEvent",
            "RadrootsStoredVisibleEventHead",
            "RadrootsProjectionCursor",
            "RadrootsProjectionRebuildTicket",
        ],
        "crates/event_store/src/model/ingest_reconciliation_v1.rs" => &["RadrootsEventIngest"],
        "crates/event_store/src/model/reconciliation_v1.rs" => &[
            "RadrootsEventAdmissionStatus",
            "StoredEventClass",
            "RadrootsEventPersistence",
            "RadrootsEventIngest",
            "RadrootsRawHeadDecision",
            "RadrootsEventStoreSourceGeneration",
        ],
        "crates/event_store/src/model/raw_source_rebuild_v1.rs" => &[
            "RadrootsEventStoreImmutableRawDigestV1",
            "RadrootsEventStoreActiveProductStateDigestV1",
            "RadrootsEventStoreRawSourceRebuildReportV1",
        ],
        "crates/event_store/src/nip09/reconciliation_v1.rs" => &[
            "ReconciliationCapacityLimits",
            "ReconciliationCapacity",
            "EventAdmission",
            "SourceRebuildBaseline",
            "ReconciliationRowEffect",
            "ReconciliationAuthorityComparison",
            "ReconciliationCardinality",
            "RequestIndex",
            "ReconciliationPaginationAxis<'_>",
            "TransitionOrigin",
        ],
        RAW_SOURCE_REBUILD_SOURCE_RELATIVE => &["RawSourceRebuildCallerSchemaLimitsV1"],
        "crates/event_store/src/nip09/reconciliation_v1/visibility_oracle_v1.rs" => {
            &["OracleRequestIndexV1<'a>"]
        }
        EVENT_STORE_STORE_SOURCE_RELATIVE => &["RadrootsEventStore"],
        "crates/event_store/src/source_maintenance_v1.rs" => {
            &["RadrootsEventStoreSourceCapacityV1"]
        }
        POST_CORE_CAPABILITIES_SOURCE_RELATIVE => &["PostCoreExtensionCapabilities<'borrow,'db>"],
        POST_CORE_STORAGE_SOURCE_RELATIVE => {
            &["TradeProjectionWrite<'a>", "PostCoreStorageV1<'borrow,'db>"]
        }
        _ => &[],
    };
    let actual_inherent_self_types = audit
        .inherent_self_types
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    if actual_inherent_self_types != expected_inherent_self_types {
        return Err(format!(
            "{relative} event-store inherent impl authority drifted: expected {expected_inherent_self_types:?}, found {actual_inherent_self_types:?}"
        ));
    }
    if relative == EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE {
        let food_id_arm = exact_associated_match_arm(
            relative,
            file,
            "EventStoreMigrationHook",
            "id",
            "FoodAvailabilityProjectionV1",
        )?;
        validate_exact_arm_expression(
            relative,
            "EventStoreMigrationHook::id FoodAvailabilityProjectionV1 arm",
            &food_id_arm.body,
            "food_manifest::FOOD_AVAILABILITY_PROJECTION_HOOK_ID",
        )?;
        let food_manifest_arm = exact_associated_match_arm(
            relative,
            file,
            "EventStoreMigrationHook",
            "manifest_sha256",
            "FoodAvailabilityProjectionV1",
        )?;
        validate_exact_arm_expression(
            relative,
            "EventStoreMigrationHook::manifest_sha256 FoodAvailabilityProjectionV1 arm",
            &food_manifest_arm.body,
            "Some(food_manifest::FOOD_AVAILABILITY_PROJECTION_MANIFEST_SHA256)",
        )?;
        let source_id_arm = exact_associated_match_arm(
            relative,
            file,
            "EventStoreMigrationHook",
            "id",
            "SourceMaintenanceV1",
        )?;
        validate_exact_arm_expression(
            relative,
            "EventStoreMigrationHook::id SourceMaintenanceV1 arm",
            &source_id_arm.body,
            "source_maintenance_manifest::SOURCE_MAINTENANCE_HOOK_ID",
        )?;
        let source_manifest_arm = exact_associated_match_arm(
            relative,
            file,
            "EventStoreMigrationHook",
            "manifest_sha256",
            "SourceMaintenanceV1",
        )?;
        validate_exact_arm_expression(
            relative,
            "EventStoreMigrationHook::manifest_sha256 SourceMaintenanceV1 arm",
            &source_manifest_arm.body,
            "Some(source_maintenance_manifest::SOURCE_MAINTENANCE_MANIFEST_SHA256)",
        )?;

        let migration_impls = file
            .items
            .iter()
            .filter_map(|item| match item {
                syn::Item::Impl(item)
                    if item.trait_.is_none()
                        && compact_tokens(item.self_ty.as_ref()) == "EventStoreMigrationHook" =>
                {
                    Some(item)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let [migration_impl] = migration_impls.as_slice() else {
            return Err(format!(
                "{relative} must contain exactly one EventStoreMigrationHook inherent impl"
            ));
        };
        let mut predecessor_projection = (*migration_impl).clone();
        let mut removed_food_arms = 0usize;
        let mut removed_source_maintenance_arms = 0usize;
        for item in &mut predecessor_projection.items {
            let syn::ImplItem::Fn(function) = item else {
                continue;
            };
            for statement in &mut function.block.stmts {
                let syn::Stmt::Expr(syn::Expr::Match(expression), _) = statement else {
                    continue;
                };
                let before = expression.arms.len();
                expression.arms = expression
                    .arms
                    .iter()
                    .filter(|arm| !syntax_contains_ident(&arm.pat, "FoodAvailabilityProjectionV1"))
                    .cloned()
                    .collect();
                removed_food_arms += before - expression.arms.len();
                let before = expression.arms.len();
                expression.arms = expression
                    .arms
                    .iter()
                    .filter(|arm| !syntax_contains_ident(&arm.pat, "SourceMaintenanceV1"))
                    .cloned()
                    .collect();
                removed_source_maintenance_arms += before - expression.arms.len();
            }
        }
        if removed_food_arms != 2 {
            return Err(format!(
                "{relative} successor migration hook impl must add exactly two FoodAvailabilityProjectionV1 arms; found {removed_food_arms}"
            ));
        }
        if removed_source_maintenance_arms != 2 {
            return Err(format!(
                "{relative} authenticated SourceMaintenance migration hook impl must add exactly two SourceMaintenanceV1 arms; found {removed_source_maintenance_arms}"
            ));
        }
        let predecessor_inherent_impls = vec![compact_tokens(&predecessor_projection)];
        let actual_sha256 = sha256_hex(&canonical_json_bytes(&predecessor_inherent_impls)?);
        if actual_sha256 != EVENT_STORE_MIGRATION_IMPL_BASELINE_SHA256 {
            return Err(format!(
                "{relative} migration inherent impl baseline drifted: expected {EVENT_STORE_MIGRATION_IMPL_BASELINE_SHA256}, found {actual_sha256}"
            ));
        }
    }
    Ok(())
}

struct PrivilegedTerminalAudit<'a> {
    relative: &'a str,
    current_function: Option<String>,
    direct_callee: bool,
    scope_depth: usize,
    definitions: Vec<PrivilegedTerminalDefinition>,
    calls: Vec<PrivilegedStoreCallSite>,
    imports: Vec<(String, String)>,
    error: Option<String>,
}

struct PrivilegedTerminalAuthority {
    definitions: Vec<PrivilegedTerminalDefinition>,
    calls: Vec<PrivilegedStoreCallSite>,
    imports: Vec<(String, String)>,
}

impl PrivilegedTerminalAudit<'_> {
    fn finish(self) -> Result<PrivilegedTerminalAuthority, String> {
        if let Some(error) = self.error {
            return Err(error);
        }
        Ok(PrivilegedTerminalAuthority {
            definitions: self.definitions,
            calls: self.calls,
            imports: self.imports,
        })
    }

    fn fail(&mut self, reason: impl Into<String>) {
        if self.error.is_none() {
            self.error = Some(format!(
                "{} privileged terminal authority {}",
                self.relative,
                reason.into()
            ));
        }
    }
}

impl<'ast> syn::visit::Visit<'ast> for PrivilegedTerminalAudit<'_> {
    fn visit_item_fn(&mut self, function: &'ast syn::ItemFn) {
        let name = function.sig.ident.to_string();
        if is_privileged_terminal(&name) {
            if self.current_function.is_some()
                || !is_authoritative_privileged_terminal_definition(self.relative, &name)
            {
                self.fail(format!(
                    "shadows privileged authority with function `{name}`"
                ));
                return;
            }
            self.definitions.push(PrivilegedTerminalDefinition {
                relative: self.relative.to_owned(),
                name: name.clone(),
            });
        }
        let previous = self.current_function.replace(format!("free:{name}"));
        syn::visit::visit_item_fn(self, function);
        self.current_function = previous;
    }

    fn visit_impl_item_fn(&mut self, function: &'ast syn::ImplItemFn) {
        if is_privileged_terminal(&function.sig.ident.to_string()) {
            self.fail(format!(
                "shadows privileged authority with associated function `{}`",
                function.sig.ident
            ));
            return;
        }
        let previous = self
            .current_function
            .replace(format!("associated:{}", function.sig.ident));
        syn::visit::visit_impl_item_fn(self, function);
        self.current_function = previous;
    }

    fn visit_trait_item_fn(&mut self, function: &'ast syn::TraitItemFn) {
        if is_privileged_terminal(&function.sig.ident.to_string()) {
            self.fail(format!(
                "shadows privileged authority with trait function `{}`",
                function.sig.ident
            ));
            return;
        }
        let previous = self
            .current_function
            .replace(format!("trait:{}", function.sig.ident));
        syn::visit::visit_trait_item_fn(self, function);
        self.current_function = previous;
    }

    fn visit_item_use(&mut self, item_use: &'ast syn::ItemUse) {
        let mut routes = Vec::new();
        flatten_use_tree("", &item_use.tree, &mut routes);
        if let Some(route) = routes.iter().find(|route| route.ends_with("::*")) {
            self.fail(format!(
                "uses unauditable glob import `{route}` in privileged source scope"
            ));
            return;
        }
        if let Some(route) = routes.iter().find(|route| {
            let source = route
                .split_once(" as ")
                .map_or(route.as_str(), |(source, _)| source);
            route.contains(" as ")
                && (use_route_local_binding(source)
                    .is_some_and(is_privileged_terminal_or_storage_type)
                    || use_route_local_binding(route)
                        .is_some_and(is_privileged_terminal_or_storage_type))
        }) {
            self.fail(format!(
                "aliases or reexports privileged source terminal through `{route}`"
            ));
            return;
        }
        for route in routes
            .iter()
            .filter(|route| use_route_local_binding(route).is_some_and(is_privileged_terminal))
        {
            if self.scope_depth != 0
                || !is_inherited_visibility(&item_use.vis)
                || !item_use.attrs.is_empty()
                || !is_approved_privileged_terminal_import(self.relative, route)
            {
                self.fail(format!(
                    "privileged terminal import `{route}` must be an exact private top-level approved route"
                ));
                return;
            }
            self.imports
                .push((self.relative.to_owned(), route.to_owned()));
        }
        if let Some(route) = routes.iter().find(|route| {
            use_route_local_binding(route).is_some_and(is_privileged_terminal_or_storage_type)
                && (self.scope_depth != 0 || !is_inherited_visibility(&item_use.vis))
        }) {
            self.fail(format!(
                "privileged terminal import `{route}` must remain private and top-level"
            ));
            return;
        }
        syn::visit::visit_item_use(self, item_use);
    }

    fn visit_block(&mut self, block: &'ast syn::Block) {
        self.scope_depth += 1;
        syn::visit::visit_block(self, block);
        self.scope_depth -= 1;
    }

    fn visit_item_mod(&mut self, module: &'ast syn::ItemMod) {
        if is_privileged_terminal(&module.ident.to_string()) {
            self.fail(format!(
                "shadows privileged authority with module `{}`",
                module.ident
            ));
            return;
        }
        if module.content.is_some() {
            self.scope_depth += 1;
            syn::visit::visit_item_mod(self, module);
            self.scope_depth -= 1;
        } else {
            syn::visit::visit_item_mod(self, module);
        }
    }

    fn visit_pat_ident(&mut self, pattern: &'ast syn::PatIdent) {
        if is_privileged_terminal(&pattern.ident.to_string()) {
            self.fail(format!(
                "shadows privileged authority with binding `{}`",
                pattern.ident
            ));
            return;
        }
        syn::visit::visit_pat_ident(self, pattern);
    }

    fn visit_item_const(&mut self, item: &'ast syn::ItemConst) {
        if is_privileged_terminal(&item.ident.to_string()) {
            self.fail(format!(
                "shadows privileged authority with const `{}`",
                item.ident
            ));
            return;
        }
        syn::visit::visit_item_const(self, item);
    }

    fn visit_item_static(&mut self, item: &'ast syn::ItemStatic) {
        if is_privileged_terminal(&item.ident.to_string()) {
            self.fail(format!(
                "shadows privileged authority with static `{}`",
                item.ident
            ));
            return;
        }
        syn::visit::visit_item_static(self, item);
    }

    fn visit_item_type(&mut self, item: &'ast syn::ItemType) {
        if is_privileged_terminal(&item.ident.to_string()) {
            self.fail(format!(
                "shadows privileged authority with type alias `{}`",
                item.ident
            ));
            return;
        }
        syn::visit::visit_item_type(self, item);
    }

    fn visit_item_struct(&mut self, item: &'ast syn::ItemStruct) {
        if is_privileged_terminal(&item.ident.to_string()) {
            self.fail(format!(
                "shadows privileged authority with struct constructor `{}`",
                item.ident
            ));
            return;
        }
        syn::visit::visit_item_struct(self, item);
    }

    fn visit_item_enum(&mut self, item: &'ast syn::ItemEnum) {
        if is_privileged_terminal(&item.ident.to_string()) {
            self.fail(format!(
                "shadows privileged authority with enum `{}`",
                item.ident
            ));
            return;
        }
        syn::visit::visit_item_enum(self, item);
    }

    fn visit_item_union(&mut self, item: &'ast syn::ItemUnion) {
        if is_privileged_terminal(&item.ident.to_string()) {
            self.fail(format!(
                "shadows privileged authority with union `{}`",
                item.ident
            ));
            return;
        }
        syn::visit::visit_item_union(self, item);
    }

    fn visit_expr_call(&mut self, expression: &'ast syn::ExprCall) {
        if let Some(route) = direct_expression_call_route(expression)
            && (route
                .rsplit("::")
                .next()
                .is_some_and(is_privileged_terminal)
                || route
                    .split("::")
                    .collect::<Vec<_>>()
                    .windows(2)
                    .any(|segments| {
                        segments == ["PostCoreExtensionCapabilities", "new"]
                            || segments == ["PostCoreStorageV1", "new"]
                    }))
        {
            let Some(function) = self.current_function.clone() else {
                self.fail(format!(
                    "calls privileged terminal `{route}` outside a function"
                ));
                return;
            };
            self.calls.push(PrivilegedStoreCallSite {
                relative: self.relative.to_owned(),
                function,
                route,
            });
            let previous = self.direct_callee;
            self.direct_callee = true;
            syn::visit::Visit::visit_expr(self, expression.func.as_ref());
            self.direct_callee = previous;
            for argument in &expression.args {
                syn::visit::Visit::visit_expr(self, argument);
            }
            return;
        }
        syn::visit::visit_expr_call(self, expression);
    }

    fn visit_expr_path(&mut self, expression: &'ast syn::ExprPath) {
        if expression
            .path
            .segments
            .last()
            .is_some_and(|segment| is_privileged_terminal(&segment.ident.to_string()))
            && !self.direct_callee
        {
            self.fail(format!(
                "takes or aliases privileged terminal value `{}`",
                compact_tokens(expression)
            ));
            return;
        }
        syn::visit::visit_expr_path(self, expression);
    }

    fn visit_macro(&mut self, item: &'ast syn::Macro) {
        if PRIVILEGED_TERMINAL_NAMES
            .iter()
            .chain(["PostCoreExtensionCapabilities", "PostCoreStorageV1"].iter())
            .any(|name| syntax_contains_ident(item, name))
        {
            self.fail(format!(
                "references privileged terminal through macro `{}`",
                compact_tokens(item)
            ));
            return;
        }
        syn::visit::visit_macro(self, item);
    }
}

fn is_privileged_terminal(name: &str) -> bool {
    PRIVILEGED_TERMINAL_NAMES.contains(&name)
}

fn is_privileged_terminal_or_storage_type(name: &str) -> bool {
    is_privileged_terminal(name)
        || matches!(name, "PostCoreExtensionCapabilities" | "PostCoreStorageV1")
}

fn is_authoritative_privileged_terminal_definition(relative: &str, name: &str) -> bool {
    matches!(
        (relative, name),
        (EVENT_STORE_SCHEMA_SOURCE_RELATIVE, "apply_migration_down")
            | (EVENT_STORE_SCHEMA_SOURCE_RELATIVE, "apply_migration_hook")
            | (EVENT_STORE_SCHEMA_SOURCE_RELATIVE, "apply_migration_up")
            | (
                EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
                "validate_event_store_temp_schema"
            )
            | (
                EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
                "validate_migration_hook_state"
            )
            | (
                EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
                "validate_rollback_preserves_source_generation_history"
            )
            | (
                "crates/event_store/src/nip09/reconciliation_v1.rs",
                "validate_active_hook_state_fast"
            )
            | (
                EVENT_STORE_STORE_SOURCE_RELATIVE,
                "validate_main_database_encoding"
            )
            | (
                POST_CORE_DISPATCHER_SOURCE_RELATIVE,
                "dispatch_post_core_extensions"
            )
            | (
                POST_CORE_EXTENSION_SOURCE_RELATIVE,
                "apply_post_core_extensions_v1"
            )
            | (
                EVENT_STORE_PROTOCOL_RECONCILIATION_SOURCE_RELATIVE,
                "ingest_event_protocol_reconciliation_v1"
            )
            | (
                EVENT_STORE_PROTOCOL_RECONCILIATION_SOURCE_RELATIVE,
                "validate_protocol_post_extensions"
            )
    ) || (relative == "crates/event_store/src/source_maintenance_v1.rs"
        && SOURCE_MAINTENANCE_PRIVILEGED_TERMINALS.contains(&name))
}

fn is_approved_privileged_terminal_import(relative: &str, route: &str) -> bool {
    matches!(
        (relative, route),
        (
            EVENT_STORE_STORE_SOURCE_RELATIVE,
            "self::post_core_extension_dispatcher::dispatch_post_core_extensions"
        ) | (
            EVENT_STORE_STORE_SOURCE_RELATIVE,
            "self::protocol_reconciliation_v1::ingest_event_protocol_reconciliation_v1"
        ) | (
            EVENT_STORE_STORE_SOURCE_RELATIVE,
            "self::protocol_reconciliation_v1::validate_protocol_post_extensions"
        ) | (
            POST_CORE_CAPABILITIES_SOURCE_RELATIVE,
            "super::post_core_extensions_v1::apply_post_core_extensions_v1"
        ) | (
            EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
            "crate::nip09::reconciliation_v1::validate_active_hook_state_fast"
        ) | (
            EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
            "crate::source_maintenance_v1::apply_source_maintenance_hook_v1"
        ) | (
            EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
            "crate::source_maintenance_v1::validate_no_persisted_ephemeral_raw_rows_v1"
        ) | (
            EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
            "crate::source_maintenance_v1::validate_source_capacity_authority_full_v1"
        ) | (
            EVENT_STORE_PROTOCOL_RECONCILIATION_SOURCE_RELATIVE,
            "crate::source_maintenance_v1::advance_source_capacity_after_insert_v1"
        ) | (
            EVENT_STORE_PROTOCOL_RECONCILIATION_SOURCE_RELATIVE,
            "crate::source_maintenance_v1::preflight_unique_raw_source_append_v1"
        ) | (
            EVENT_STORE_PROTOCOL_RECONCILIATION_SOURCE_RELATIVE,
            "crate::source_maintenance_v1::raw_source_capacity_delta_v1"
        ) | (
            EVENT_STORE_PROTOCOL_RECONCILIATION_SOURCE_RELATIVE,
            "crate::source_maintenance_v1::validate_source_capacity_authority_fast_v1"
        ) | (
            RAW_SOURCE_REBUILD_SOURCE_RELATIVE,
            "crate::source_maintenance_v1::preflight_source_generation_append_v1"
        ) | (
            RAW_SOURCE_REBUILD_SOURCE_RELATIVE,
            "crate::source_maintenance_v1::validate_source_capacity_authority_fast_v1"
        ) | (
            RAW_SOURCE_REBUILD_SOURCE_RELATIVE,
            "crate::source_maintenance_v1::validate_source_capacity_authority_full_v1"
        ) | (
            RAW_SOURCE_REBUILD_SOURCE_RELATIVE,
            "super::validate_active_hook_state_fast"
        )
    )
}

fn validate_event_store_lib_resolution_authority(
    relative: &str,
    file: &syn::File,
) -> Result<(), String> {
    let file_attributes = file.attrs.iter().map(compact_tokens).collect::<Vec<_>>();
    let expected_file_attributes = ["#![forbid(unsafe_code)]"];
    if file_attributes != expected_file_attributes {
        return Err(format!(
            "{relative} crate attributes drifted: expected {expected_file_attributes:?}, found {file_attributes:?}"
        ));
    }

    let modules = file
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Mod(module) => Some(module),
            _ => None,
        })
        .collect::<Vec<_>>();
    let required_modules = [
        "error",
        "generated",
        "migrations",
        "model",
        "nip09",
        "schema",
        "store",
    ];
    let mut module_names = BTreeSet::new();
    for module in &modules {
        let name = module.ident.to_string();
        if !module_names.insert(name.clone()) {
            return Err(format!(
                "{relative} contains duplicate production module route `{name}`"
            ));
        }
        let attributes = module.attrs.iter().map(compact_tokens).collect::<Vec<_>>();
        if !is_inherited_visibility(&module.vis)
            || module.content.is_some()
            || attributes != ["#[cfg(feature=\"sqlite\")]"]
        {
            return Err(format!(
                "{relative} module `{}` must remain an exact private external `sqlite` feature route",
                module.ident
            ));
        }
    }
    let predecessor_module_names = required_modules.into_iter().collect::<BTreeSet<_>>();
    if !predecessor_module_names.is_subset(
        &module_names
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
    ) {
        return Err(format!(
            "{relative} must retain every predecessor-governed private `sqlite` module; found {module_names:?}"
        ));
    }
    let required_module_names = predecessor_module_names
        .into_iter()
        .chain(SUCCESSOR_08D_LIB_MODULES)
        .collect::<BTreeSet<_>>();
    if module_names
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>()
        != required_module_names
    {
        return Err(format!(
            "{relative} must contain exactly the predecessor modules plus the authenticated SourceMaintenance private `sqlite` module; found {module_names:?}"
        ));
    }

    if file
        .items
        .iter()
        .any(|item| !matches!(item, syn::Item::Mod(_) | syn::Item::Use(_)))
    {
        return Err(format!(
            "{relative} may contain only the exact governed modules and public reexports"
        ));
    }
    let mut actual_uses = Vec::new();
    let mut local_bindings = BTreeSet::new();
    for item in &file.items {
        let syn::Item::Use(item_use) = item else {
            continue;
        };
        let attributes = item_use
            .attrs
            .iter()
            .map(compact_tokens)
            .collect::<Vec<_>>();
        if route_visibility(&item_use.vis) != Some(RouteVisibility::Public)
            || item_use.leading_colon.is_some()
            || attributes != ["#[cfg(feature=\"sqlite\")]"]
        {
            return Err(format!(
                "{relative} reexports must remain exact public `sqlite` feature routes"
            ));
        }
        let mut routes = Vec::new();
        flatten_use_tree("", &item_use.tree, &mut routes);
        if routes
            .iter()
            .any(|route| route.ends_with("::*") || route.contains(" as "))
        {
            return Err(format!(
                "{relative} reexports must not use glob or alias name resolution"
            ));
        }
        for route in &routes {
            let Some((_module, _)) = route.split_once("::") else {
                return Err(format!(
                    "{relative} reexport `{route}` must use an explicit module-qualified path"
                ));
            };
            let binding = use_route_local_binding(route)
                .ok_or_else(|| format!("{relative} reexport `{route}` has no local binding"))?;
            if !local_bindings.insert(binding.to_owned()) {
                return Err(format!(
                    "{relative} reexports duplicate local binding `{binding}`"
                ));
            }
            let inherited_current = EVENT_STORE_FIXED_PUBLIC_REEXPORTS.contains(&route.as_str())
                && !SUCCESSOR_08D_RETIRED_PUBLIC_REEXPORTS.contains(&route.as_str());
            if !inherited_current
                && !SUCCESSOR_08C_PUBLIC_REEXPORTS.contains(&route.as_str())
                && !SUCCESSOR_08D_PUBLIC_REEXPORTS.contains(&route.as_str())
                && !SUCCESSOR_08D1_PUBLIC_REEXPORTS.contains(&route.as_str())
            {
                return Err(format!(
                    "{relative} public export inventory is closed for this contract version; found unsupported reexport `{route}`"
                ));
            }
        }
        actual_uses.extend(routes);
    }
    let expected_uses = EVENT_STORE_FIXED_PUBLIC_REEXPORTS
        .into_iter()
        .filter(|route| !SUCCESSOR_08D_RETIRED_PUBLIC_REEXPORTS.contains(route))
        .chain(SUCCESSOR_08C_PUBLIC_REEXPORTS)
        .chain(SUCCESSOR_08D_PUBLIC_REEXPORTS)
        .chain(SUCCESSOR_08D1_PUBLIC_REEXPORTS)
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    let actual_use_set = actual_uses.iter().cloned().collect::<BTreeSet<_>>();
    if actual_uses.len() != expected_uses.len() || actual_use_set != expected_uses {
        return Err(format!(
            "{relative} public export inventory drifted: expected {expected_uses:?}, found {actual_uses:?}"
        ));
    }
    Ok(())
}

fn validate_privileged_store_module_routes(relative: &str, file: &syn::File) -> Result<(), String> {
    let modules = file
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Mod(module) => Some(module),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut actual = BTreeSet::new();
    for module in modules {
        let name = module.ident.to_string();
        if !actual.insert(name.clone()) {
            return Err(format!(
                "{relative} contains duplicate production module route `{name}`"
            ));
        }
        let predecessor_module = PRIVILEGED_STORE_MODULE_NAMES.contains(&name.as_str());
        let successor_module = SUCCESSOR_08C_STORE_MODULE_NAMES.contains(&name.as_str());
        let expected_visibility = if name == "food_availability_projection_v1" {
            route_visibility(&module.vis) == Some(RouteVisibility::Crate)
        } else {
            is_inherited_visibility(&module.vis)
        };
        if (!predecessor_module && !successor_module)
            || !expected_visibility
            || module.content.is_some()
            || !module.attrs.is_empty()
        {
            return Err(format!(
                "{relative} module `{}` is outside the exact private governed module inventory",
                module.ident
            ));
        }
    }
    let expected = PRIVILEGED_STORE_MODULE_NAMES
        .into_iter()
        .chain(SUCCESSOR_08C_STORE_MODULE_NAMES)
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(format!(
            "{relative} governed module inventory drifted: expected {expected:?}, found {actual:?}"
        ));
    }
    // The immutable predecessor hashes the v1 root. Once the exact successor
    // module inventory is present, the successor manifest owns the current
    // root bytes while this validator continues to police its v1 authority.
    Ok(())
}

fn collect_privileged_store_imports(
    relative: &str,
    file: &syn::File,
) -> Result<Vec<(String, String)>, String> {
    let mut audit = PrivilegedStoreImportAudit {
        relative,
        scope_depth: 0,
        imports: Vec::new(),
        error: None,
    };
    syn::visit::Visit::visit_file(&mut audit, file);
    audit.finish()
}

struct PrivilegedStoreImportAudit<'a> {
    relative: &'a str,
    scope_depth: usize,
    imports: Vec<(String, String)>,
    error: Option<String>,
}

impl PrivilegedStoreImportAudit<'_> {
    fn finish(self) -> Result<Vec<(String, String)>, String> {
        self.error.map_or(Ok(self.imports), Err)
    }

    fn fail(&mut self, reason: impl Into<String>) {
        if self.error.is_none() {
            self.error = Some(format!(
                "{} privileged store import authority {}",
                self.relative,
                reason.into()
            ));
        }
    }
}

impl<'ast> syn::visit::Visit<'ast> for PrivilegedStoreImportAudit<'_> {
    fn visit_item_use(&mut self, item_use: &'ast syn::ItemUse) {
        let prefix = if item_use.leading_colon.is_some() {
            "::"
        } else {
            ""
        };
        let mut routes = Vec::new();
        flatten_use_tree(prefix, &item_use.tree, &mut routes);
        for route in routes
            .into_iter()
            .filter(|route| is_privileged_store_import_route(route))
        {
            if self.scope_depth != 0 {
                self.fail(format!(
                    "route `{route}` must not be imported or aliased from a nested scope"
                ));
                return;
            }
            if !is_inherited_visibility(&item_use.vis) || !item_use.attrs.is_empty() {
                self.fail(format!("route `{route}` must be unconditional and private"));
                return;
            }
            self.imports.push((self.relative.to_owned(), route));
        }
        syn::visit::visit_item_use(self, item_use);
    }

    fn visit_block(&mut self, block: &'ast syn::Block) {
        self.scope_depth += 1;
        syn::visit::visit_block(self, block);
        self.scope_depth -= 1;
    }

    fn visit_item_mod(&mut self, module: &'ast syn::ItemMod) {
        if module.content.is_some() {
            self.scope_depth += 1;
            syn::visit::visit_item_mod(self, module);
            self.scope_depth -= 1;
        } else {
            syn::visit::visit_item_mod(self, module);
        }
    }
}

fn is_privileged_store_import_route(route: &str) -> bool {
    let source_route = route.split_once(" as ").map_or(route, |(source, _)| source);
    [
        "post_core_extension_capabilities",
        "post_core_extension_dispatcher",
        "post_core_extensions_v1",
        "post_core_storage_v1",
        "protocol_reconciliation_v1",
        "protocol_storage_v1",
    ]
    .iter()
    .any(|segment| source_route.split("::").any(|actual| actual == *segment))
        || use_route_local_binding(source_route).is_some_and(|binding| {
            is_governed_store_import_binding(binding) || is_privileged_terminal(binding)
        })
        || use_route_local_binding(route).is_some_and(|binding| {
            is_governed_store_import_binding(binding) || is_privileged_terminal(binding)
        })
        || route.ends_with("::*")
}

fn use_route_local_binding(route: &str) -> Option<&str> {
    if route.ends_with("::*") {
        return None;
    }
    route
        .rsplit_once(" as ")
        .map(|(_, alias)| alias)
        .or_else(|| route.rsplit("::").next())
}

fn is_governed_store_import_binding(binding: &str) -> bool {
    matches!(
        binding,
        "Clone"
            | "Debug"
            | "Eq"
            | "PartialEq"
            | "PostCoreExtensionCapabilities"
            | "PostCoreStorageV1"
            | "dispatch_post_core_extensions"
            | "apply_post_core_extensions_v1"
            | "format"
            | "include_bytes"
            | "include_str"
            | "ingest_event_protocol_reconciliation_v1"
            | "matches"
            | "validate_event_store_temp_schema"
            | "validate_protocol_post_extensions"
    )
}

fn is_privileged_store_value_binding(binding: &str) -> bool {
    is_privileged_terminal(binding)
}

struct PrivilegedStoreReferenceAudit<'a> {
    relative: &'a str,
    current_function: Option<String>,
    direct_privileged_callee: bool,
    call_sites: Vec<PrivilegedStoreCallSite>,
    error: Option<String>,
}

impl PrivilegedStoreReferenceAudit<'_> {
    fn finish(self) -> Result<Vec<PrivilegedStoreCallSite>, String> {
        self.error.map_or(Ok(self.call_sites), Err)
    }

    fn fail(&mut self, reason: impl Into<String>) {
        if self.error.is_none() {
            self.error = Some(format!(
                "{} privileged store authority {}",
                self.relative,
                reason.into()
            ));
        }
    }
}

impl<'ast> syn::visit::Visit<'ast> for PrivilegedStoreReferenceAudit<'_> {
    fn visit_item_fn(&mut self, function: &'ast syn::ItemFn) {
        if is_privileged_store_value_binding(&function.sig.ident.to_string())
            && (self.current_function.is_some()
                || !is_authoritative_privileged_store_definition(
                    self.relative,
                    &function.sig.ident.to_string(),
                ))
        {
            self.fail(format!(
                "shadows privileged authority with function `{}`",
                function.sig.ident
            ));
            return;
        }
        let previous = self
            .current_function
            .replace(format!("free:{}", function.sig.ident));
        syn::visit::visit_item_fn(self, function);
        self.current_function = previous;
    }

    fn visit_impl_item_fn(&mut self, function: &'ast syn::ImplItemFn) {
        let previous = self
            .current_function
            .replace(format!("associated:{}", function.sig.ident));
        syn::visit::visit_impl_item_fn(self, function);
        self.current_function = previous;
    }

    fn visit_trait_item_fn(&mut self, function: &'ast syn::TraitItemFn) {
        let previous = self
            .current_function
            .replace(format!("trait:{}", function.sig.ident));
        syn::visit::visit_trait_item_fn(self, function);
        self.current_function = previous;
    }

    fn visit_item_mod(&mut self, module: &'ast syn::ItemMod) {
        if self.relative != EVENT_STORE_STORE_SOURCE_RELATIVE
            || self.current_function.is_some()
            || module.content.is_some()
        {
            self.fail(format!(
                "introduces unsupported production module `{}`",
                module.ident
            ));
            return;
        }
        syn::visit::visit_item_mod(self, module);
    }

    fn visit_item_type(&mut self, item: &'ast syn::ItemType) {
        if expression_contains_privileged_store_authority(item)
            || syntax_contains_ident(item, "PostCoreStorageV1")
        {
            self.fail(format!(
                "aliases privileged authority through type `{}`",
                item.ident
            ));
            return;
        }
        syn::visit::visit_item_type(self, item);
    }

    fn visit_type_path(&mut self, path: &'ast syn::TypePath) {
        if self.relative != POST_CORE_EXTENSION_SOURCE_RELATIVE
            && self.relative != POST_CORE_STORAGE_SOURCE_RELATIVE
            && self.relative != POST_CORE_CAPABILITIES_SOURCE_RELATIVE
            && path
                .path
                .segments
                .iter()
                .any(|segment| segment.ident == "PostCoreStorageV1")
        {
            self.fail(format!(
                "aliases `PostCoreStorageV1` through root-store type path `{}`",
                compact_tokens(path)
            ));
            return;
        }
        syn::visit::visit_type_path(self, path);
    }

    fn visit_pat_ident(&mut self, pattern: &'ast syn::PatIdent) {
        if is_privileged_store_value_binding(&pattern.ident.to_string()) {
            self.fail(format!(
                "shadows privileged authority with binding `{}`",
                pattern.ident
            ));
            return;
        }
        syn::visit::visit_pat_ident(self, pattern);
    }

    fn visit_item_const(&mut self, item: &'ast syn::ItemConst) {
        if is_privileged_store_value_binding(&item.ident.to_string()) {
            self.fail(format!(
                "shadows privileged authority with const `{}`",
                item.ident
            ));
            return;
        }
        syn::visit::visit_item_const(self, item);
    }

    fn visit_item_static(&mut self, item: &'ast syn::ItemStatic) {
        if is_privileged_store_value_binding(&item.ident.to_string()) {
            self.fail(format!(
                "shadows privileged authority with static `{}`",
                item.ident
            ));
            return;
        }
        syn::visit::visit_item_static(self, item);
    }

    fn visit_item_struct(&mut self, item: &'ast syn::ItemStruct) {
        if is_privileged_store_value_binding(&item.ident.to_string()) {
            self.fail(format!(
                "shadows privileged authority with struct constructor `{}`",
                item.ident
            ));
            return;
        }
        syn::visit::visit_item_struct(self, item);
    }

    fn visit_item_enum(&mut self, item: &'ast syn::ItemEnum) {
        if self.relative == EVENT_STORE_STORE_SOURCE_RELATIVE
            && item.ident == "PoolTempSchemaPolicy"
        {
            let expected = syn::parse_str::<syn::ItemEnum>(
                r#"#[derive(Clone, Copy)]
                enum PoolTempSchemaPolicy {
                    Standard,
                    RawSourceRepairV1,
                }"#,
            )
            .expect("parse governed pool TEMP-schema policy");
            if compact_tokens(item) != compact_tokens(&expected) {
                self.fail("raw-source rebuild pool TEMP-schema policy drifted");
            }
            return;
        }
        if is_privileged_store_value_binding(&item.ident.to_string()) {
            self.fail(format!(
                "shadows privileged authority with enum `{}`",
                item.ident
            ));
            return;
        }
        syn::visit::visit_item_enum(self, item);
    }

    fn visit_item_union(&mut self, item: &'ast syn::ItemUnion) {
        if is_privileged_store_value_binding(&item.ident.to_string()) {
            self.fail(format!(
                "shadows privileged authority with union `{}`",
                item.ident
            ));
            return;
        }
        syn::visit::visit_item_union(self, item);
    }

    fn visit_item_extern_crate(&mut self, item: &'ast syn::ItemExternCrate) {
        self.fail(format!(
            "introduces unsupported extern-crate namespace `{}`",
            item.ident
        ));
    }

    fn visit_expr_call(&mut self, expression: &'ast syn::ExprCall) {
        if let Some(route) = direct_expression_call_route(expression)
            && is_privileged_store_call_route(&route)
        {
            let Some(function) = self.current_function.clone() else {
                self.fail(format!("call `{route}` is outside a function"));
                return;
            };
            self.call_sites.push(PrivilegedStoreCallSite {
                relative: self.relative.to_owned(),
                function,
                route,
            });
            let previous = self.direct_privileged_callee;
            self.direct_privileged_callee = true;
            syn::visit::Visit::visit_expr(self, expression.func.as_ref());
            self.direct_privileged_callee = previous;
            for attribute in &expression.attrs {
                syn::visit::Visit::visit_attribute(self, attribute);
            }
            for argument in &expression.args {
                syn::visit::Visit::visit_expr(self, argument);
            }
            return;
        }
        if expression_contains_privileged_store_authority(expression.func.as_ref()) {
            self.fail(format!(
                "uses an indirect or unsupported privileged callee `{}`",
                compact_tokens(expression.func.as_ref())
            ));
            return;
        }
        syn::visit::visit_expr_call(self, expression);
    }

    fn visit_expr_path(&mut self, expression: &'ast syn::ExprPath) {
        if expression_contains_privileged_store_authority(expression)
            && !self.direct_privileged_callee
        {
            self.fail(format!(
                "takes or aliases privileged value `{}` outside an approved direct call",
                compact_tokens(expression)
            ));
            return;
        }
        syn::visit::visit_expr_path(self, expression);
    }

    fn visit_expr_struct(&mut self, expression: &'ast syn::ExprStruct) {
        if expression
            .path
            .segments
            .iter()
            .any(|segment| segment.ident == "PostCoreStorageV1")
        {
            self.fail("constructs `PostCoreStorageV1` outside its governed constructor");
            return;
        }
        syn::visit::visit_expr_struct(self, expression);
    }

    fn visit_macro(&mut self, item: &'ast syn::Macro) {
        if expression_contains_privileged_store_authority(item) {
            self.fail(format!(
                "references privileged authority through macro `{}`",
                compact_tokens(item)
            ));
            return;
        }
        let macro_name = item
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string())
            .unwrap_or_default();
        if item.path.leading_colon.is_some()
            || item.path.segments.len() != 1
            || !matches!(macro_name.as_str(), "format" | "matches")
        {
            self.fail(format!(
                "uses unsupported or non-builtin-resolved production macro `{}`",
                compact_tokens(&item.path)
            ));
            return;
        }
        if macro_name == "format" {
            use syn::parse::Parser;

            let Ok(arguments) =
                syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated
                    .parse2(item.tokens.clone())
            else {
                self.fail("uses a format! body that cannot be structurally audited");
                return;
            };
            let Some(first) = arguments.first() else {
                self.fail("uses format! without a literal format string");
                return;
            };
            if !matches!(
                first,
                syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Str(_),
                    ..
                })
            ) {
                self.fail("uses format! without a literal format string");
                return;
            }
            for argument in arguments.iter().skip(1) {
                if !is_pure_privileged_format_argument(argument) {
                    self.fail(format!(
                        "uses side-effect-capable format! argument `{}`",
                        compact_tokens(argument)
                    ));
                    return;
                }
                syn::visit::Visit::visit_expr(self, argument);
            }
            return;
        }

        use syn::parse::Parser;
        let parser = |input: syn::parse::ParseStream<'_>| {
            let expression = input.parse::<syn::Expr>()?;
            input.parse::<syn::Token![,]>()?;
            let pattern = syn::Pat::parse_multi_with_leading_vert(input)?;
            let guard = if input.peek(syn::Token![if]) {
                input.parse::<syn::Token![if]>()?;
                Some(input.parse::<syn::Expr>()?)
            } else {
                None
            };
            if !input.is_empty() {
                return Err(input.error("unexpected tokens after matches! pattern"));
            }
            Ok((expression, pattern, guard))
        };
        let Ok((expression, pattern, guard)) = parser.parse2(item.tokens.clone()) else {
            self.fail("uses a matches! body that cannot be structurally audited");
            return;
        };
        if !is_pure_privileged_format_argument(&expression)
            || guard
                .as_ref()
                .is_some_and(|guard| !is_pure_privileged_format_argument(guard))
        {
            self.fail("uses a side-effect-capable matches! expression or guard");
            return;
        }
        syn::visit::Visit::visit_expr(self, &expression);
        syn::visit::Visit::visit_pat(self, &pattern);
        if let Some(guard) = &guard {
            syn::visit::Visit::visit_expr(self, guard);
        }
    }

    fn visit_attribute(&mut self, attribute: &'ast syn::Attribute) {
        if !is_allowed_privileged_store_attribute(attribute) {
            self.fail(format!(
                "uses unsupported production attribute `{}`",
                compact_tokens(attribute)
            ));
            return;
        }
        syn::visit::visit_attribute(self, attribute);
    }
}

fn is_pure_privileged_format_argument(expression: &syn::Expr) -> bool {
    match expression {
        syn::Expr::Call(expression) => {
            function_call_matches(expression, "sqlite_error_primary_result_code")
                && expression.args.len() == 1
                && expression
                    .args
                    .first()
                    .is_some_and(is_pure_privileged_format_argument)
        }
        syn::Expr::Field(expression) => {
            is_pure_privileged_format_argument(expression.base.as_ref())
        }
        syn::Expr::Group(expression) => {
            is_pure_privileged_format_argument(expression.expr.as_ref())
        }
        syn::Expr::Lit(_) => true,
        syn::Expr::MethodCall(expression) => {
            expression.method == "len"
                && expression.turbofish.is_none()
                && expression.args.is_empty()
                && is_pure_privileged_format_argument(expression.receiver.as_ref())
        }
        syn::Expr::Paren(expression) => {
            is_pure_privileged_format_argument(expression.expr.as_ref())
        }
        syn::Expr::Path(expression) => {
            expression.qself.is_none()
                && expression.path.leading_colon.is_none()
                && expression.path.segments.len() == 1
        }
        syn::Expr::Reference(expression) => {
            is_pure_privileged_format_argument(expression.expr.as_ref())
        }
        _ => false,
    }
}

fn is_authoritative_privileged_store_definition(relative: &str, name: &str) -> bool {
    matches!(
        (relative, name),
        (
            EVENT_STORE_STORE_SOURCE_RELATIVE,
            "validate_main_database_encoding"
        ) | (
            POST_CORE_DISPATCHER_SOURCE_RELATIVE,
            "dispatch_post_core_extensions"
        ) | (
            POST_CORE_EXTENSION_SOURCE_RELATIVE,
            "apply_post_core_extensions_v1"
        ) | (
            EVENT_STORE_PROTOCOL_RECONCILIATION_SOURCE_RELATIVE,
            "ingest_event_protocol_reconciliation_v1"
        ) | (
            EVENT_STORE_PROTOCOL_RECONCILIATION_SOURCE_RELATIVE,
            "validate_protocol_post_extensions"
        )
    )
}

fn is_allowed_privileged_store_attribute(attribute: &syn::Attribute) -> bool {
    if attribute.path().is_ident("derive") {
        use syn::parse::Parser;

        let Ok(derives) =
            syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated.parse2(
                match &attribute.meta {
                    syn::Meta::List(list) => list.tokens.clone(),
                    _ => return false,
                },
            )
        else {
            return false;
        };
        return !derives.is_empty()
            && derives.iter().all(|derive| {
                derive.leading_colon.is_none()
                    && derive.segments.len() == 1
                    && matches!(
                        derive
                            .segments
                            .first()
                            .map(|segment| segment.ident.to_string())
                            .as_deref(),
                        Some("Clone" | "Debug" | "Eq" | "PartialEq")
                    )
            });
    }
    matches!(
        compact_tokens(attribute).as_str(),
        "#[allow(clippy::too_many_arguments)]" | "#[cfg_attr(coverage_nightly,coverage(off))]"
    )
}

fn is_privileged_store_call_route(route: &str) -> bool {
    route
        .rsplit("::")
        .next()
        .is_some_and(is_privileged_terminal)
        || route
            .split("::")
            .collect::<Vec<_>>()
            .windows(2)
            .any(|segments| {
                segments == ["PostCoreExtensionCapabilities", "new"]
                    || segments == ["PostCoreStorageV1", "new"]
            })
}

fn expression_contains_privileged_store_authority(node: &impl ToTokens) -> bool {
    PRIVILEGED_TERMINAL_NAMES
        .iter()
        .any(|ident| syntax_contains_ident(node, ident))
        || (["PostCoreExtensionCapabilities", "PostCoreStorageV1"]
            .iter()
            .any(|name| syntax_contains_ident(node, name))
            && syntax_contains_ident(node, "new"))
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

fn route_visibility(visibility: &syn::Visibility) -> Option<RouteVisibility> {
    match visibility {
        syn::Visibility::Inherited => Some(RouteVisibility::Inherited),
        syn::Visibility::Public(_) => Some(RouteVisibility::Public),
        syn::Visibility::Restricted(restricted)
            if restricted.in_token.is_none() && restricted.path.is_ident("crate") =>
        {
            Some(RouteVisibility::Crate)
        }
        syn::Visibility::Restricted(_) => None,
    }
}

fn collect_top_level_use_routes(file: &syn::File) -> Vec<(RouteVisibility, String)> {
    let mut routes = Vec::new();
    for item in &file.items {
        let syn::Item::Use(item_use) = item else {
            continue;
        };
        let Some(visibility) = route_visibility(&item_use.vis) else {
            continue;
        };
        let prefix = if item_use.leading_colon.is_some() {
            "::"
        } else {
            ""
        };
        let mut paths = Vec::new();
        flatten_use_tree(prefix, &item_use.tree, &mut paths);
        routes.extend(paths.into_iter().map(|path| (visibility, path)));
    }
    routes
}

fn collect_top_level_use_routes_with_attributes(
    file: &syn::File,
) -> Vec<(RouteVisibility, String, Vec<String>)> {
    let mut routes = Vec::new();
    for item in &file.items {
        let syn::Item::Use(item_use) = item else {
            continue;
        };
        let Some(visibility) = route_visibility(&item_use.vis) else {
            continue;
        };
        let prefix = if item_use.leading_colon.is_some() {
            "::"
        } else {
            ""
        };
        let attributes = item_use
            .attrs
            .iter()
            .map(compact_tokens)
            .collect::<Vec<_>>();
        let mut paths = Vec::new();
        flatten_use_tree(prefix, &item_use.tree, &mut paths);
        routes.extend(
            paths
                .into_iter()
                .map(|path| (visibility, path, attributes.clone())),
        );
    }
    routes
}

fn flatten_use_tree(prefix: &str, tree: &syn::UseTree, paths: &mut Vec<String>) {
    match tree {
        syn::UseTree::Path(path) => {
            let prefix = format!("{prefix}{}::", path.ident);
            flatten_use_tree(&prefix, &path.tree, paths);
        }
        syn::UseTree::Name(name) => paths.push(format!("{prefix}{}", name.ident)),
        syn::UseTree::Rename(rename) => {
            paths.push(format!("{prefix}{} as {}", rename.ident, rename.rename))
        }
        syn::UseTree::Glob(_) => paths.push(format!("{prefix}*")),
        syn::UseTree::Group(group) => {
            for item in &group.items {
                flatten_use_tree(prefix, item, paths);
            }
        }
    }
}

fn validate_entry_point_sources(workspace_root: &Path) -> Result<(), String> {
    for spec in ENTRY_POINT_SPECS {
        let bytes = read_regular_file(workspace_root, spec.source_path)?;
        let source = std::str::from_utf8(&bytes).map_err(|error| {
            format!(
                "{} must be UTF-8 Rust source for entry point {}: {error}",
                spec.source_path, spec.rust_path
            )
        })?;
        validate_entry_point_source(spec.source_path, source, *spec)?;
    }
    Ok(())
}

fn validate_entry_point_source(
    relative: &str,
    source: &str,
    spec: EntryPointSpec,
) -> Result<(), String> {
    let file =
        syn::parse_file(source).map_err(|error| format!("parse {relative} as Rust: {error}"))?;
    match spec.callable {
        CallableSpec::Free {
            module_path,
            name,
            visibility,
        } => {
            let items = inline_module_items(relative, &file, module_path)?;
            let matches = items
                .iter()
                .filter_map(|item| match item {
                    syn::Item::Fn(function) if function.sig.ident == name => Some(function),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let [function] = matches.as_slice() else {
                return Err(format!(
                    "{relative} must define exactly one structured entry-point function `{name}` for {}; found {}",
                    spec.rust_path,
                    matches.len()
                ));
            };
            validate_callable_visibility(relative, spec.rust_path, &function.vis, visibility)
        }
        CallableSpec::Associated {
            owner,
            name,
            visibility,
        } => {
            let matches = file
                .items
                .iter()
                .filter_map(|item| match item {
                    syn::Item::Impl(item_impl)
                        if item_impl.trait_.is_none()
                            && simple_type_name(&item_impl.self_ty).as_deref() == Some(owner) =>
                    {
                        Some(item_impl)
                    }
                    _ => None,
                })
                .flat_map(|item_impl| item_impl.items.iter())
                .filter_map(|item| match item {
                    syn::ImplItem::Fn(function) if function.sig.ident == name => Some(function),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let [function] = matches.as_slice() else {
                return Err(format!(
                    "{relative} must define exactly one structured associated entry-point function `{owner}::{name}` for {}; found {}",
                    spec.rust_path,
                    matches.len()
                ));
            };
            validate_callable_visibility(relative, spec.rust_path, &function.vis, visibility)
        }
    }
}

fn inline_module_items<'a>(
    relative: &str,
    file: &'a syn::File,
    module_path: &[&str],
) -> Result<&'a [syn::Item], String> {
    let mut items = file.items.as_slice();
    for segment in module_path {
        let modules = items
            .iter()
            .filter_map(|item| match item {
                syn::Item::Mod(module) if module.ident == segment => Some(module),
                _ => None,
            })
            .collect::<Vec<_>>();
        let [module] = modules.as_slice() else {
            return Err(format!(
                "{relative} must contain exactly one inline module `{segment}`; found {}",
                modules.len()
            ));
        };
        let Some((_, nested_items)) = module.content.as_ref() else {
            return Err(format!(
                "{relative} module `{segment}` must be inline for structural entry-point validation"
            ));
        };
        items = nested_items;
    }
    Ok(items)
}

fn simple_type_name(ty: &syn::Type) -> Option<String> {
    let syn::Type::Path(path) = ty else {
        return None;
    };
    if path.qself.is_some() {
        return None;
    }
    path.path
        .segments
        .last()
        .map(|segment| segment.ident.to_string())
}

fn validate_callable_visibility(
    relative: &str,
    rust_path: &str,
    actual: &syn::Visibility,
    expected: RouteVisibility,
) -> Result<(), String> {
    if route_visibility(actual) != Some(expected) {
        return Err(format!(
            "{relative} entry point `{rust_path}` must have {} visibility",
            expected.label()
        ));
    }
    Ok(())
}

enum WitnessedFunction<'a> {
    Free(&'a syn::ItemFn),
    Associated(&'a syn::ImplItemFn),
}

impl WitnessedFunction<'_> {
    fn canonical_ast(&self) -> String {
        match self {
            Self::Free(function) => function.to_token_stream().to_string(),
            Self::Associated(function) => function.to_token_stream().to_string(),
        }
    }

    fn collect_call_routes(&self) -> Vec<String> {
        use syn::visit::Visit;

        let mut collector = RustCallRouteCollector { routes: Vec::new() };
        match self {
            Self::Free(function) => collector.visit_item_fn(function),
            Self::Associated(function) => collector.visit_impl_item_fn(function),
        }
        collector.routes
    }
}

struct RustCallRouteCollector {
    routes: Vec<String>,
}

impl<'ast> syn::visit::Visit<'ast> for RustCallRouteCollector {
    fn visit_expr_call(&mut self, expression: &'ast syn::ExprCall) {
        if let syn::Expr::Path(path) = expression.func.as_ref()
            && path.qself.is_none()
        {
            let path = path
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect::<Vec<_>>()
                .join("::");
            self.routes.push(format!("fn:{path}"));
        }
        syn::visit::visit_expr_call(self, expression);
    }

    fn visit_expr_method_call(&mut self, expression: &'ast syn::ExprMethodCall) {
        self.routes.push(format!("method:{}", expression.method));
        syn::visit::visit_expr_method_call(self, expression);
    }
}

fn describe_rust_item_witnesses(
    workspace_root: &Path,
) -> Result<Vec<RustItemWitnessDescriptor>, String> {
    let paths = RUST_ITEM_WITNESS_ROOT_SPECS
        .iter()
        .map(|spec| spec.path)
        .collect::<BTreeSet<_>>();
    let paths = paths.into_iter().collect::<Vec<_>>();
    let [relative] = paths.as_slice() else {
        return Err("Rust item witness roots must name exactly one source file".to_owned());
    };
    let bytes = read_regular_file(workspace_root, relative)?;
    let source = std::str::from_utf8(&bytes)
        .map_err(|error| format!("{relative} must be UTF-8 Rust source: {error}"))?;
    describe_rust_item_witnesses_source(relative, source, RUST_ITEM_WITNESS_ROOT_SPECS)
}

fn describe_rust_item_witnesses_source(
    relative: &str,
    source: &str,
    roots: &[RustItemWitnessRootSpec],
) -> Result<Vec<RustItemWitnessDescriptor>, String> {
    let file = parse_canonical_production_rust(relative, source.as_bytes())?;
    let mut free_functions = BTreeMap::new();
    for item in &file.items {
        let syn::Item::Fn(function) = item else {
            continue;
        };
        let name = function.sig.ident.to_string();
        if free_functions.insert(name.clone(), function).is_some() {
            return Err(format!(
                "{relative} contains duplicate top-level function `{name}`"
            ));
        }
    }

    let mut root_free_specs = BTreeMap::new();
    let mut witnesses = Vec::new();
    let mut queue = VecDeque::new();
    for root in roots {
        if root.path != relative {
            return Err(format!(
                "Rust item witness root {} must use source path {relative}",
                root.role
            ));
        }
        match root.callable {
            RustWitnessCallable::Free { name } => {
                if root_free_specs.insert(name, *root).is_some() {
                    return Err(format!(
                        "Rust item witness roots contain duplicate free function `{name}`"
                    ));
                }
                if root.binding == RustWitnessBinding::AstClosure {
                    queue.push_back(name.to_owned());
                } else {
                    let function = free_functions.get(name).ok_or_else(|| {
                        format!(
                            "{relative} route-only witness references missing function `{name}`"
                        )
                    })?;
                    validate_route_only_free_function(relative, name, function)?;
                    let item = WitnessedFunction::Free(function);
                    let call_routes = match root.binding {
                        RustWitnessBinding::SelfAst => item.collect_call_routes(),
                        RustWitnessBinding::AstClosure => {
                            unreachable!("AST closure roots are queued")
                        }
                    };
                    validate_required_call_sequence(
                        relative,
                        name,
                        &call_routes,
                        root.required_call_sequence,
                    )?;
                    witnesses.push(RustItemWitnessDescriptor {
                        role: root.role.to_owned(),
                        path: relative.to_owned(),
                        item: name.to_owned(),
                        root: true,
                        binding: match root.binding {
                            RustWitnessBinding::SelfAst => "self_ast",
                            RustWitnessBinding::AstClosure => {
                                unreachable!("AST closure roots are queued")
                            }
                        }
                        .to_owned(),
                        local_call_sequence: required_local_call_sequence(
                            relative,
                            name,
                            root.required_call_sequence,
                            &free_functions,
                        )?,
                        required_call_sequence: root
                            .required_call_sequence
                            .iter()
                            .map(|route| (*route).to_owned())
                            .collect(),
                        ast_sha256: (root.binding == RustWitnessBinding::SelfAst)
                            .then(|| sha256_hex(item.canonical_ast().as_bytes())),
                    });
                }
            }
            RustWitnessCallable::Associated { owner, name } => {
                let function = exact_associated_function(relative, &file, owner, name)?;
                if root.binding == RustWitnessBinding::SelfAst {
                    validate_route_only_associated_function(relative, owner, name, function)?;
                }
                let item = WitnessedFunction::Associated(function);
                let call_routes = match root.binding {
                    RustWitnessBinding::SelfAst => item.collect_call_routes(),
                    RustWitnessBinding::AstClosure => item.collect_call_routes(),
                };
                let local_calls = local_call_sequence(
                    relative,
                    &format!("{owner}::{name}"),
                    &call_routes,
                    &free_functions,
                )?;
                validate_required_call_sequence(
                    relative,
                    &format!("{owner}::{name}"),
                    &call_routes,
                    root.required_call_sequence,
                )?;
                let (binding, local_call_sequence, ast_sha256) = match root.binding {
                    RustWitnessBinding::SelfAst => (
                        "self_ast",
                        required_local_call_sequence(
                            relative,
                            &format!("{owner}::{name}"),
                            root.required_call_sequence,
                            &free_functions,
                        )?,
                        Some(sha256_hex(item.canonical_ast().as_bytes())),
                    ),
                    RustWitnessBinding::AstClosure => {
                        queue.extend(local_calls.iter().cloned());
                        (
                            "ast_closure",
                            local_calls,
                            Some(sha256_hex(item.canonical_ast().as_bytes())),
                        )
                    }
                };
                witnesses.push(RustItemWitnessDescriptor {
                    role: root.role.to_owned(),
                    path: relative.to_owned(),
                    item: format!("{owner}::{name}"),
                    root: true,
                    binding: binding.to_owned(),
                    local_call_sequence,
                    required_call_sequence: root
                        .required_call_sequence
                        .iter()
                        .map(|route| (*route).to_owned())
                        .collect(),
                    ast_sha256,
                });
            }
        }
    }

    let mut visited = BTreeSet::new();
    while let Some(name) = queue.pop_front() {
        if !visited.insert(name.clone()) {
            continue;
        }
        let function = free_functions.get(&name).ok_or_else(|| {
            format!("{relative} local call graph references missing function `{name}`")
        })?;
        let item = WitnessedFunction::Free(function);
        let call_routes = item.collect_call_routes();
        let local_calls = local_call_sequence(relative, &name, &call_routes, &free_functions)?;
        queue.extend(local_calls.iter().cloned());
        let root = root_free_specs.get(name.as_str());
        if let Some(root) = root {
            validate_required_call_sequence(
                relative,
                &name,
                &call_routes,
                root.required_call_sequence,
            )?;
            if name == "ingest_event_protocol_reconciliation_v1" {
                validate_protocol_core_structure(relative, function)?;
            }
        }
        witnesses.push(RustItemWitnessDescriptor {
            role: root.map_or_else(
                || format!("event_store_incremental_local_dependency_{name}_v1"),
                |root| root.role.to_owned(),
            ),
            path: relative.to_owned(),
            item: name,
            root: root.is_some(),
            binding: "ast_closure".to_owned(),
            local_call_sequence: local_calls,
            required_call_sequence: root
                .map(|root| {
                    root.required_call_sequence
                        .iter()
                        .map(|route| (*route).to_owned())
                        .collect()
                })
                .unwrap_or_default(),
            ast_sha256: Some(sha256_hex(item.canonical_ast().as_bytes())),
        });
    }
    witnesses.sort_by(|left, right| left.item.cmp(&right.item));
    Ok(witnesses)
}

fn required_local_call_sequence(
    relative: &str,
    item: &str,
    required: &[&str],
    free_functions: &BTreeMap<String, &syn::ItemFn>,
) -> Result<Vec<String>, String> {
    let required = required
        .iter()
        .map(|route| (*route).to_owned())
        .collect::<Vec<_>>();
    local_call_sequence(relative, item, &required, free_functions)
}

fn validate_route_only_associated_function(
    relative: &str,
    owner: &str,
    name: &str,
    function: &syn::ImplItemFn,
) -> Result<(), String> {
    let (expected_signature, expected_body) = match (owner, name) {
        ("RadrootsEventStore", "open_memory") => (
            "async fn open_memory()->Result<Self,RadrootsEventStoreError>",
            "{let options=SqliteConnectOptions::from_str(\"sqlite::memory:\")?;let pool=SqlitePoolOptions::new().max_connections(1).connect_with(options).await?;configure_pool(&pool,false).await?;migrate_event_store_schema(&pool).await?;Ok(Self{pool})}",
        ),
        ("RadrootsEventStore", "open_file") => (
            "async fn open_file(path:impl AsRef<Path>,)->Result<Self,RadrootsEventStoreError>",
            "{let options=SqliteConnectOptions::new().filename(path).create_if_missing(true);let pool=SqlitePoolOptions::new().max_connections(1).connect_with(options).await?;configure_pool(&pool,true).await?;migrate_event_store_schema(&pool).await?;Ok(Self{pool})}",
        ),
        ("RadrootsEventStore", "open_pool") => (
            "async fn open_pool(pool:SqlitePool,file_backed:bool,)->Result<Self,RadrootsEventStoreError>",
            "{configure_pool(&pool,file_backed).await?;migrate_event_store_schema(&pool).await?;Ok(Self{pool})}",
        ),
        ("RadrootsEventStore", "schema_status") => (
            "async fn schema_status(&self,)->Result<RadrootsEventStoreSchemaStatus,RadrootsEventStoreError>",
            "{inspect_event_store_schema_status(&self.pool).await}",
        ),
        ("RadrootsEventStore", "migrate_to_current_schema") => (
            "async fn migrate_to_current_schema(&self,)->Result<(),RadrootsEventStoreError>",
            "{migrate_event_store_schema(&self.pool).await}",
        ),
        ("RadrootsEventStore", "begin_write_transaction") => (
            "async fn begin_write_transaction(&self,)->Result<sqlx::Transaction<'static,sqlx::Sqlite>,RadrootsEventStoreError>",
            "{Ok(self.pool.begin_with(\"BEGIN IMMEDIATE\").await?)}",
        ),
        ("RadrootsEventStore", "ingest_event") => (
            "async fn ingest_event(&self,ingest:RadrootsEventIngest,)->Result<RadrootsEventIngestReceipt,RadrootsEventStoreError>",
            "{let mut tx=self.begin_write_transaction().await?;match ingest_event_in_transaction(&mut tx,ingest).await{Ok(receipt)=>{tx.commit().await?;Ok(receipt)}Err(error)=>{let rollback=tx.rollback().await;preserve_ingest_primary_failure(error,rollback)}}}",
        ),
        ("RadrootsEventStore", "ingest_event_in_transaction") => (
            "async fn ingest_event_in_transaction(&self,tx:&mut sqlx::Transaction<'_,sqlx::Sqlite>,ingest:RadrootsEventIngest,)->Result<RadrootsEventIngestReceipt,RadrootsEventStoreError>",
            "{let mut savepoint=sqlx::Acquire::begin(&mut*tx).await?;match ingest_event_in_transaction(&mut savepoint,ingest).await{Ok(receipt)=>{savepoint.commit().await?;Ok(receipt)}Err(error)=>{let rollback=savepoint.rollback().await;preserve_ingest_primary_failure(error,rollback)}}}",
        ),
        _ => return Ok(()),
    };
    if !matches!(function.vis, syn::Visibility::Public(_))
        || !function.attrs.is_empty()
        || compact_tokens(&function.sig) != compact_source_tokens(expected_signature)
        || compact_tokens(&function.block) != compact_source_tokens(expected_body)
    {
        return Err(format!(
            "{relative} route-only wrapper `{owner}::{name}` signature, control flow, or argument binding drifted (signature `{}`, body `{}`)",
            compact_tokens(&function.sig),
            compact_tokens(&function.block)
        ));
    }
    Ok(())
}

fn validate_route_only_free_function(
    relative: &str,
    name: &str,
    function: &syn::ItemFn,
) -> Result<(), String> {
    if name != "ingest_event_in_transaction" {
        return Ok(());
    }
    let expected_signature = "async fn ingest_event_in_transaction(tx:&mut sqlx::Transaction<'_,sqlx::Sqlite>,ingest:RadrootsEventIngest,)->Result<RadrootsEventIngestReceipt,RadrootsEventStoreError>";
    let expected = "{crate::schema::validate_event_store_temp_schema(tx).await?;let result=ingest_event_protocol_reconciliation_v1(tx,&ingest).await?;{let mut capabilities=PostCoreExtensionCapabilities::new(tx);dispatch_post_core_extensions(&mut capabilities,&ingest,&result).await?;}validate_protocol_post_extensions(tx,&result).await?;Ok(result.receipt)}";
    if !matches!(function.vis, syn::Visibility::Inherited)
        || !function.attrs.is_empty()
        || compact_tokens(&function.sig) != compact_source_tokens(expected_signature)
        || compact_tokens(&function.block) != compact_source_tokens(expected)
    {
        return Err(format!(
            "{relative} extensible ingest wrapper signature, core/extensions/seal control flow, or argument binding drifted"
        ));
    }
    Ok(())
}

fn direct_statement_call_routes(block: &syn::Block) -> Vec<String> {
    block
        .stmts
        .iter()
        .filter_map(|statement| match statement {
            syn::Stmt::Local(local) => local
                .init
                .as_ref()
                .and_then(|initializer| direct_call_route(&initializer.expr)),
            syn::Stmt::Expr(expression, _) => direct_call_route(expression),
            syn::Stmt::Item(_) | syn::Stmt::Macro(_) => None,
        })
        .collect()
}

fn direct_call_route(expression: &syn::Expr) -> Option<String> {
    let expression = direct_terminal_expression(expression)?;
    match expression {
        syn::Expr::Call(call) => {
            let syn::Expr::Path(path) = call.func.as_ref() else {
                return None;
            };
            if path.qself.is_some() {
                return None;
            }
            Some(format!(
                "fn:{}",
                path.path
                    .segments
                    .iter()
                    .map(|segment| segment.ident.to_string())
                    .collect::<Vec<_>>()
                    .join("::")
            ))
        }
        syn::Expr::MethodCall(call) => Some(format!("method:{}", call.method)),
        _ => None,
    }
}

fn direct_terminal_expression(mut expression: &syn::Expr) -> Option<&syn::Expr> {
    loop {
        expression = match expression {
            syn::Expr::Try(expression) => &expression.expr,
            syn::Expr::Await(expression) => &expression.base,
            syn::Expr::Group(expression) => &expression.expr,
            syn::Expr::Paren(expression) => &expression.expr,
            syn::Expr::Field(expression) => &expression.base,
            syn::Expr::Call(call) if is_transparent_ok_call(call) => call.args.first()?,
            syn::Expr::Call(_) | syn::Expr::MethodCall(_) => return Some(expression),
            _ => return Some(expression),
        };
    }
}

fn is_transparent_ok_call(call: &syn::ExprCall) -> bool {
    matches!(
        call.func.as_ref(),
        syn::Expr::Path(path)
            if path.qself.is_none()
                && path.path.segments.len() == 1
                && path.path.is_ident("Ok")
                && call.args.len() == 1
    )
}

fn validate_protocol_core_structure(relative: &str, function: &syn::ItemFn) -> Result<(), String> {
    let direct_routes = direct_statement_call_routes(&function.block);
    validate_required_call_sequence(
        relative,
        "ingest_event_protocol_reconciliation_v1 direct body",
        &direct_routes,
        &[
            "fn:acquire_event_store_write_lock",
            "fn:validate_source_raw_authority",
            "fn:EventAdmission::for_profile",
            "fn:insert_raw_event",
            "fn:apply_raw_event_head",
        ],
    )?;

    let inserted_blocks = function
        .block
        .stmts
        .iter()
        .filter_map(|statement| match statement {
            syn::Stmt::Expr(syn::Expr::If(expression), _)
                if matches!(
                    expression.cond.as_ref(),
                    syn::Expr::Path(path)
                        if path.qself.is_none() && path.path.is_ident("inserted")
                ) =>
            {
                Some(&expression.then_branch)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let append_blocks = inserted_blocks
        .iter()
        .filter(|block| {
            let routes = direct_statement_call_routes(block);
            routes.iter().any(|route| route == "fn:insert_tags")
                || routes
                    .iter()
                    .any(|route| route == "fn:persist_event_coordinate_after_insert")
        })
        .collect::<Vec<_>>();
    let [append_block] = append_blocks.as_slice() else {
        return Err(format!(
            "{relative} protocol-v1 ingest must contain exactly one direct `if inserted` append block; found {}",
            append_blocks.len()
        ));
    };
    validate_required_call_sequence(
        relative,
        "protocol-v1 inserted append block",
        &direct_statement_call_routes(append_block),
        &["fn:insert_tags", "fn:persist_event_coordinate_after_insert"],
    )?;

    let synchronize_blocks = inserted_blocks
        .iter()
        .filter(|block| {
            direct_statement_call_routes(block)
                .iter()
                .any(|route| route == "fn:synchronize_after_insert")
        })
        .collect::<Vec<_>>();
    let [synchronize_block] = synchronize_blocks.as_slice() else {
        return Err(format!(
            "{relative} protocol-v1 ingest must contain exactly one direct `if inserted` synchronization block; found {}",
            synchronize_blocks.len()
        ));
    };
    validate_required_call_sequence(
        relative,
        "protocol-v1 inserted synchronization block",
        &direct_statement_call_routes(synchronize_block),
        &["fn:synchronize_after_insert"],
    )
}

fn exact_associated_function<'a>(
    relative: &str,
    file: &'a syn::File,
    owner: &str,
    name: &str,
) -> Result<&'a syn::ImplItemFn, String> {
    let matches = file
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Impl(item_impl)
                if item_impl.trait_.is_none()
                    && simple_type_name(&item_impl.self_ty).as_deref() == Some(owner) =>
            {
                Some(item_impl)
            }
            _ => None,
        })
        .flat_map(|item_impl| item_impl.items.iter())
        .filter_map(|item| match item {
            syn::ImplItem::Fn(function) if function.sig.ident == name => Some(function),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [function] = matches.as_slice() else {
        return Err(format!(
            "{relative} must define exactly one associated function `{owner}::{name}`; found {}",
            matches.len()
        ));
    };
    Ok(function)
}

fn local_call_sequence(
    relative: &str,
    item: &str,
    call_routes: &[String],
    free_functions: &BTreeMap<String, &syn::ItemFn>,
) -> Result<Vec<String>, String> {
    let mut locals = Vec::new();
    for route in call_routes {
        let Some(path) = route.strip_prefix("fn:") else {
            continue;
        };
        let segments = path.split("::").collect::<Vec<_>>();
        let Some(terminal) = segments.last().copied() else {
            continue;
        };
        if !free_functions.contains_key(terminal) {
            continue;
        }
        let allowed = matches!(
            segments.as_slice(),
            [_] | ["self", _] | ["crate", "store", _]
        );
        if !allowed {
            return Err(format!(
                "{relative} witnessed item `{item}` calls local function `{terminal}` through unsupported or ambiguous route `{path}`"
            ));
        }
        locals.push(terminal.to_owned());
    }
    Ok(locals)
}

#[derive(Debug)]
struct PostCoreExtensionBoundaryProjection {
    capability_struct_ast_sha256: String,
    capability_constructor_ast_sha256: String,
    capability_v1_method_ast_sha256: String,
    dispatcher_signature_sha256: String,
    dispatcher_v1_prefix_sha256: String,
}

fn describe_post_core_extension_boundary(
    workspace_root: &Path,
    require_v1_only: bool,
) -> Result<PostCoreExtensionBoundaryProjection, String> {
    let capabilities_bytes =
        read_regular_file(workspace_root, POST_CORE_CAPABILITIES_SOURCE_RELATIVE)?;
    let capabilities = parse_canonical_production_rust(
        POST_CORE_CAPABILITIES_SOURCE_RELATIVE,
        &capabilities_bytes,
    )?;
    validate_compiler_macro_inputs(POST_CORE_CAPABILITIES_SOURCE_RELATIVE, &capabilities, &[])?;
    let capability_uses = collect_top_level_use_routes(&capabilities)
        .into_iter()
        .map(|(_, route)| route)
        .collect::<BTreeSet<_>>();
    let mut expected_capability_uses = [
        "super::post_core_extensions_v1::apply_post_core_extensions_v1",
        "super::post_core_storage_v1::PostCoreStorageV1",
        "super::protocol_reconciliation_v1::ProtocolReconciliationV1IngestResult",
        "crate::error::RadrootsEventStoreError",
        "crate::model::RadrootsEventIngest",
        "sqlx::Sqlite",
        "sqlx::Transaction",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<BTreeSet<_>>();
    if !require_v1_only {
        expected_capability_uses.extend([
            "super::post_core_extensions_v2::apply_post_core_extensions_v2".to_owned(),
            "super::post_core_storage_v2::PostCoreStorageV2".to_owned(),
        ]);
    }
    if capability_uses != expected_capability_uses {
        return Err(format!(
            "{POST_CORE_CAPABILITIES_SOURCE_RELATIVE} imports drifted outside the exact authenticated capability boundary"
        ));
    }
    if capabilities.items.iter().any(|item| {
        !matches!(
            item,
            syn::Item::Use(_) | syn::Item::Struct(_) | syn::Item::Impl(_)
        )
    }) {
        return Err(format!(
            "{POST_CORE_CAPABILITIES_SOURCE_RELATIVE} may contain only imports, its capability struct, and its inherent impl"
        ));
    }
    let structs = capabilities
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Struct(item) if item.ident == "PostCoreExtensionCapabilities" => Some(item),
            _ => None,
        })
        .collect::<Vec<_>>();
    let all_struct_count = capabilities
        .items
        .iter()
        .filter(|item| matches!(item, syn::Item::Struct(_)))
        .count();
    let [capability_struct] = structs.as_slice() else {
        return Err(format!(
            "{POST_CORE_CAPABILITIES_SOURCE_RELATIVE} must define exactly one PostCoreExtensionCapabilities struct"
        ));
    };
    if all_struct_count != 1 {
        return Err(format!(
            "{POST_CORE_CAPABILITIES_SOURCE_RELATIVE} must not introduce helper structs with access to private transaction authority"
        ));
    }
    let expected_struct = compact_source_tokens(
        "pub(super) struct PostCoreExtensionCapabilities<'borrow, 'db> {
            tx: &'borrow mut Transaction<'db, Sqlite>,
        }",
    );
    if compact_tokens(capability_struct) != expected_struct {
        return Err(format!(
            "{POST_CORE_CAPABILITIES_SOURCE_RELATIVE} capability must retain exactly one private transaction field and no raw authority escape"
        ));
    }
    let impls = capabilities
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Impl(item)
                if item.trait_.is_none()
                    && simple_type_name(&item.self_ty).as_deref()
                        == Some("PostCoreExtensionCapabilities") =>
            {
                Some(item)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let all_impl_count = capabilities
        .items
        .iter()
        .filter(|item| matches!(item, syn::Item::Impl(_)))
        .count();
    let [capability_impl] = impls.as_slice() else {
        return Err(format!(
            "{POST_CORE_CAPABILITIES_SOURCE_RELATIVE} must define exactly one capability inherent impl"
        ));
    };
    if all_impl_count != 1 {
        return Err(format!(
            "{POST_CORE_CAPABILITIES_SOURCE_RELATIVE} must not introduce trait or helper impl authority"
        ));
    }
    let mut capability_header = (*capability_impl).clone();
    capability_header.items.clear();
    if compact_tokens(&capability_header)
        != compact_source_tokens(
            "impl<'borrow, 'db> PostCoreExtensionCapabilities<'borrow, 'db> {}",
        )
    {
        return Err(format!(
            "{POST_CORE_CAPABILITIES_SOURCE_RELATIVE} capability impl header drifted"
        ));
    }
    let methods = capability_impl
        .items
        .iter()
        .filter_map(|item| match item {
            syn::ImplItem::Fn(function) => Some((function.sig.ident.to_string(), function)),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    if methods.len() != capability_impl.items.len() {
        return Err(format!(
            "{POST_CORE_CAPABILITIES_SOURCE_RELATIVE} capability impl may contain only methods"
        ));
    }
    let expected_methods = if require_v1_only {
        vec!["apply_v1", "new"]
    } else {
        vec!["apply_v1", "apply_v2", "new"]
    };
    if methods.keys().map(String::as_str).collect::<Vec<_>>() != expected_methods {
        return Err(format!(
            "{POST_CORE_CAPABILITIES_SOURCE_RELATIVE} extension methods must match the exact migration-bound version inventory"
        ));
    }
    let constructor = methods.get("new").ok_or_else(|| {
        format!("{POST_CORE_CAPABILITIES_SOURCE_RELATIVE} must retain capability constructor `new`")
    })?;
    if compact_tokens(&constructor.sig)
        != compact_source_tokens("fn new(tx: &'borrow mut Transaction<'db, Sqlite>) -> Self")
        || compact_tokens(&constructor.block) != compact_source_tokens("{ Self { tx } }")
        || !is_pub_super(&constructor.vis)
        || !constructor.attrs.is_empty()
    {
        return Err(format!(
            "{POST_CORE_CAPABILITIES_SOURCE_RELATIVE} capability constructor drifted"
        ));
    }
    let apply_v1 = methods.get("apply_v1").ok_or_else(|| {
        format!("{POST_CORE_CAPABILITIES_SOURCE_RELATIVE} must retain capability method `apply_v1`")
    })?;
    if compact_tokens(&apply_v1.sig)
        != compact_source_tokens(
            "async fn apply_v1(
                &mut self,
                ingest: &RadrootsEventIngest,
                result: &ProtocolReconciliationV1IngestResult,
            ) -> Result<(), RadrootsEventStoreError>",
        )
        || compact_tokens(&apply_v1.block)
            != compact_source_tokens(
                "{
                    let mut storage = PostCoreStorageV1::new(self.tx);
                    apply_post_core_extensions_v1(&mut storage, ingest, result).await
                }",
            )
        || !is_pub_super(&apply_v1.vis)
        || !apply_v1.attrs.is_empty()
    {
        return Err(format!(
            "{POST_CORE_CAPABILITIES_SOURCE_RELATIVE} apply_v1 must retain the exact restricted storage-to-v1 extension route"
        ));
    }
    if !require_v1_only {
        let apply_v2 = methods.get("apply_v2").expect("exact v2 inventory checked");
        if compact_tokens(&apply_v2.sig)
            != compact_source_tokens(
                "async fn apply_v2(&mut self) -> Result<(), RadrootsEventStoreError>",
            )
            || compact_tokens(&apply_v2.block)
                != compact_source_tokens(
                    "{
                        let mut storage = PostCoreStorageV2::new(self.tx);
                        apply_post_core_extensions_v2(&mut storage).await
                    }",
                )
            || !is_pub_super(&apply_v2.vis)
            || !apply_v2.attrs.is_empty()
        {
            return Err(format!(
                "{POST_CORE_CAPABILITIES_SOURCE_RELATIVE} apply_v2 must retain the exact restricted storage-to-v2 extension route"
            ));
        }
    }

    let dispatcher_bytes = read_regular_file(workspace_root, POST_CORE_DISPATCHER_SOURCE_RELATIVE)?;
    let dispatcher =
        parse_canonical_production_rust(POST_CORE_DISPATCHER_SOURCE_RELATIVE, &dispatcher_bytes)?;
    validate_compiler_macro_inputs(POST_CORE_DISPATCHER_SOURCE_RELATIVE, &dispatcher, &[])?;
    let dispatcher_uses = collect_top_level_use_routes(&dispatcher)
        .into_iter()
        .map(|(_, route)| route)
        .collect::<BTreeSet<_>>();
    let expected_dispatcher_uses = [
        "super::post_core_extension_capabilities::PostCoreExtensionCapabilities",
        "super::protocol_reconciliation_v1::ProtocolReconciliationV1IngestResult",
        "crate::error::RadrootsEventStoreError",
        "crate::model::RadrootsEventIngest",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<BTreeSet<_>>();
    if dispatcher_uses != expected_dispatcher_uses {
        return Err(format!(
            "{POST_CORE_DISPATCHER_SOURCE_RELATIVE} imports drifted outside the authority-free dispatcher boundary"
        ));
    }
    if dispatcher
        .items
        .iter()
        .any(|item| !matches!(item, syn::Item::Use(_) | syn::Item::Fn(_)))
    {
        return Err(format!(
            "{POST_CORE_DISPATCHER_SOURCE_RELATIVE} may contain only imports and its dispatcher function"
        ));
    }
    let functions = dispatcher
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Fn(function) => Some(function),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [dispatcher_root] = functions.as_slice() else {
        return Err(format!(
            "{POST_CORE_DISPATCHER_SOURCE_RELATIVE} must define exactly one dispatcher function"
        ));
    };
    if dispatcher_root.sig.ident != "dispatch_post_core_extensions"
        || !is_pub_super(&dispatcher_root.vis)
        || !dispatcher_root.attrs.is_empty()
        || compact_tokens(&dispatcher_root.sig)
            != compact_source_tokens(
                "async fn dispatch_post_core_extensions(
                    capabilities: &mut PostCoreExtensionCapabilities<'_, '_>,
                    ingest: &RadrootsEventIngest,
                    result: &ProtocolReconciliationV1IngestResult,
                ) -> Result<(), RadrootsEventStoreError>",
            )
    {
        return Err(format!(
            "{POST_CORE_DISPATCHER_SOURCE_RELATIVE} dispatcher signature drifted"
        ));
    }
    let statements = &dispatcher_root.block.stmts;
    if statements.len() < 2
        || compact_tokens(&statements[0])
            != compact_source_tokens("capabilities.apply_v1(ingest, result).await?;")
        || compact_tokens(statements.last().expect("at least two statements"))
            != compact_source_tokens("Ok(())")
    {
        return Err(format!(
            "{POST_CORE_DISPATCHER_SOURCE_RELATIVE} dispatcher must begin with unconditional awaited apply_v1 error propagation and end with Ok(())"
        ));
    }
    let mut versions = Vec::new();
    for (index, statement) in statements[..statements.len() - 1].iter().enumerate() {
        let version = index + 1;
        let expected = match version {
            1 => "capabilities.apply_v1(ingest,result).await?;".to_owned(),
            2 if !require_v1_only => "capabilities.apply_v2().await?;".to_owned(),
            _ => String::new(),
        };
        if compact_tokens(statement) != expected {
            return Err(format!(
                "{POST_CORE_DISPATCHER_SOURCE_RELATIVE} extension call {version} must match its exact direct, contiguous, awaited, question-mark-propagated route"
            ));
        }
        versions.push(version);
    }
    let expected_versions = if require_v1_only {
        &[1][..]
    } else {
        &[1, 2][..]
    };
    if versions != expected_versions {
        return Err(format!(
            "{POST_CORE_DISPATCHER_SOURCE_RELATIVE} extension calls must match the exact migration-bound version inventory"
        ));
    }

    Ok(PostCoreExtensionBoundaryProjection {
        capability_struct_ast_sha256: sha256_hex(compact_tokens(capability_struct).as_bytes()),
        capability_constructor_ast_sha256: sha256_hex(compact_tokens(*constructor).as_bytes()),
        capability_v1_method_ast_sha256: sha256_hex(compact_tokens(*apply_v1).as_bytes()),
        dispatcher_signature_sha256: sha256_hex(compact_tokens(&dispatcher_root.sig).as_bytes()),
        dispatcher_v1_prefix_sha256: sha256_hex(compact_tokens(&statements[0]).as_bytes()),
    })
}

fn describe_post_core_sql_capability(
    workspace_root: &Path,
) -> Result<PostCoreSqlCapabilityDescriptor, String> {
    let boundary = describe_post_core_extension_boundary(workspace_root, false)?;
    let extension = read_regular_file(workspace_root, POST_CORE_EXTENSION_SOURCE_RELATIVE)?;
    validate_post_core_extension_source(POST_CORE_EXTENSION_SOURCE_RELATIVE, &extension)?;
    let extension_ast = canonical_rust_ast(
        POST_CORE_EXTENSION_SOURCE_RELATIVE,
        &extension,
        RustAstProfile::Production,
    )?;
    let storage = read_regular_file(workspace_root, POST_CORE_STORAGE_SOURCE_RELATIVE)?;
    let mut statements =
        validate_post_core_storage_source(POST_CORE_STORAGE_SOURCE_RELATIVE, &storage)?;
    let storage_ast = canonical_rust_ast(
        POST_CORE_STORAGE_SOURCE_RELATIVE,
        &storage,
        RustAstProfile::Production,
    )?;
    statements.sort();
    if statements.is_empty() {
        return Err(format!(
            "{POST_CORE_STORAGE_SOURCE_RELATIVE} post-core storage capability must contain authenticated SQL terminals"
        ));
    }
    let observed_capabilities = statements
        .iter()
        .flat_map(|statement| {
            statement.tables.iter().map(|table| {
                (
                    statement.operation.to_ascii_lowercase(),
                    table.to_ascii_lowercase(),
                )
            })
        })
        .collect::<BTreeSet<_>>();
    let allowed_capabilities = POST_CORE_SQL_ALLOWED_CAPABILITIES
        .iter()
        .map(|capability| (capability.operation.to_owned(), capability.table.to_owned()))
        .collect::<BTreeSet<_>>();
    if observed_capabilities != allowed_capabilities {
        let missing = allowed_capabilities
            .difference(&observed_capabilities)
            .cloned()
            .collect::<Vec<_>>();
        let unexpected = observed_capabilities
            .difference(&allowed_capabilities)
            .cloned()
            .collect::<Vec<_>>();
        return Err(format!(
            "{POST_CORE_STORAGE_SOURCE_RELATIVE} post-core SQL exercised capability set drifted; missing {missing:?}, unexpected {unexpected:?}"
        ));
    }

    Ok(PostCoreSqlCapabilityDescriptor {
        algorithm: POST_CORE_SQL_CAPABILITY_ALGORITHM.to_owned(),
        capabilities_path: POST_CORE_CAPABILITIES_SOURCE_RELATIVE.to_owned(),
        capability_type: "PostCoreExtensionCapabilities".to_owned(),
        capability_struct_ast_sha256: boundary.capability_struct_ast_sha256,
        capability_constructor_ast_sha256: boundary.capability_constructor_ast_sha256,
        capability_v1_method_ast_sha256: boundary.capability_v1_method_ast_sha256,
        dispatcher_path: POST_CORE_DISPATCHER_SOURCE_RELATIVE.to_owned(),
        dispatcher_root: "dispatch_post_core_extensions".to_owned(),
        dispatcher_signature_sha256: boundary.dispatcher_signature_sha256,
        dispatcher_v1_prefix_sha256: boundary.dispatcher_v1_prefix_sha256,
        extension_path: POST_CORE_EXTENSION_SOURCE_RELATIVE.to_owned(),
        extension_ast_sha256: sha256_hex(&extension_ast),
        storage_path: POST_CORE_STORAGE_SOURCE_RELATIVE.to_owned(),
        storage_ast_sha256: sha256_hex(&storage_ast),
        root: POST_CORE_EXTENSION_ROOT.to_owned(),
        storage_methods: POST_CORE_STORAGE_METHODS
            .iter()
            .map(|method| (*method).to_owned())
            .collect(),
        statements,
        allowed_capabilities: POST_CORE_SQL_ALLOWED_CAPABILITIES
            .iter()
            .map(|capability| PostCoreSqlOperationCapabilityDescriptor {
                operation: capability.operation.to_owned(),
                table: capability.table.to_owned(),
            })
            .collect(),
        forbidden_classes: POST_CORE_SQL_FORBIDDEN_CLASSES
            .iter()
            .map(|class| (*class).to_owned())
            .collect(),
    })
}

fn validate_post_core_extension_source(relative: &str, bytes: &[u8]) -> Result<(), String> {
    let file = parse_canonical_production_rust(relative, bytes)?;
    let use_routes = collect_top_level_use_routes(&file)
        .into_iter()
        .map(|(_, route)| route)
        .collect::<BTreeSet<_>>();
    for required in [
        "super::post_core_storage_v1::PostCoreStorageV1",
        "super::post_core_storage_v1::TradeProjectionWrite",
        "super::protocol_reconciliation_v1::ProtocolReconciliationV1IngestResult",
        "crate::error::RadrootsEventStoreError",
        "crate::model::RadrootsEventIngest",
    ] {
        if !use_routes.contains(required) {
            return Err(format!(
                "{relative} must import exact post-core extension route `{required}`"
            ));
        }
    }
    let expected_use_routes = [
        "super::post_core_storage_v1::PostCoreStorageV1",
        "super::post_core_storage_v1::TradeProjectionWrite",
        "super::protocol_reconciliation_v1::ProtocolReconciliationV1IngestResult",
        "crate::error::RadrootsEventStoreError",
        "crate::model::RadrootsEventIngest",
        "radroots_event::ids::RadrootsTradeCandidateId",
        "radroots_event::ids::RadrootsTradeMutationId",
        "radroots_event::trade::RADROOTS_TRADE_MUTATION_CONTRACT_IDS",
        "radroots_event::trade::RadrootsSellerReservationAssertionV1",
        "radroots_event::trade::RadrootsTradeDecisionV1",
        "radroots_event::trade::RadrootsTradeMutationBodyV1",
        "radroots_event::trade::RadrootsTradeMutationEnvelopeV1",
        "radroots_event::trade::trade_mutation_from_canonical_content",
        "sha2::Digest",
        "sha2::Sha256",
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    if use_routes
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>()
        != expected_use_routes
    {
        return Err(format!(
            "{relative} production extension imports must match the exact pure-domain capability allowlist"
        ));
    }
    for route in &use_routes {
        if route.ends_with("::*")
            || route.contains(" as ")
            || route.starts_with("sqlx::")
            || route.starts_with("std::")
            || route.starts_with("tokio::")
            || route.starts_with("async_std::")
            || route.starts_with("smol::")
            || route.starts_with("reqwest::")
            || route.starts_with("ureq::")
            || route.starts_with("crate::schema::")
            || route.starts_with("crate::nip09::")
            || (route.starts_with("super::protocol_reconciliation_v1::")
                && route
                    != "super::protocol_reconciliation_v1::ProtocolReconciliationV1IngestResult")
        {
            return Err(format!(
                "{relative} extension import `{route}` crosses the authority-free boundary"
            ));
        }
    }

    let mut functions = BTreeMap::new();
    for item in &file.items {
        match item {
            syn::Item::Use(_) => {}
            syn::Item::Fn(function) => {
                let name = function.sig.ident.to_string();
                if functions.insert(name.clone(), function).is_some() {
                    return Err(format!("{relative} contains duplicate function `{name}`"));
                }
            }
            _ => {
                return Err(format!(
                    "{relative} production extension source may contain only imports and free functions"
                ));
            }
        }
    }
    let root = functions.get(POST_CORE_EXTENSION_ROOT).ok_or_else(|| {
        format!("{relative} must define extension root `{POST_CORE_EXTENSION_ROOT}`")
    })?;
    let exposed = functions
        .values()
        .filter(|function| is_pub_super(&function.vis))
        .map(|function| function.sig.ident.to_string())
        .collect::<Vec<_>>();
    if exposed != [POST_CORE_EXTENSION_ROOT] {
        return Err(format!(
            "{relative} must expose only `{POST_CORE_EXTENSION_ROOT}`; found {exposed:?}"
        ));
    }
    if !root.attrs.is_empty()
        || compact_tokens(&root.sig)
            != compact_source_tokens(
                "async fn apply_post_core_extensions_v1(
                    storage: &mut PostCoreStorageV1<'_, '_>,
                    ingest: &RadrootsEventIngest,
                    result: &ProtocolReconciliationV1IngestResult,
                ) -> Result<(), RadrootsEventStoreError>",
            )
    {
        return Err(format!(
            "{relative} extension root signature or production attributes drifted"
        ));
    }
    if functions.iter().any(|(name, function)| {
        name != POST_CORE_EXTENSION_ROOT
            && (!is_inherited_visibility(&function.vis)
                || !function.attrs.is_empty()
                || function.sig.unsafety.is_some()
                || function.sig.abi.is_some()
                || function.sig.variadic.is_some())
    }) {
        return Err(format!(
            "{relative} extension helpers must remain untransformed, safe, Rust-ABI private functions"
        ));
    }

    let local_functions = functions.keys().cloned().collect::<BTreeSet<_>>();
    for (name, function) in functions {
        let mut audit = PostCoreExtensionAuthorityAudit {
            relative,
            function: name.as_str(),
            local_functions: &local_functions,
            error: None,
        };
        syn::visit::Visit::visit_item_fn(&mut audit, function);
        audit.finish()?;
    }
    Ok(())
}

fn validate_post_core_storage_source(
    relative: &str,
    bytes: &[u8],
) -> Result<Vec<PostCoreSqlStatementDescriptor>, String> {
    let file = parse_canonical_production_rust(relative, bytes)?;
    if file
        .items
        .iter()
        .any(|item| matches!(item, syn::Item::Impl(item) if item.trait_.is_some()))
    {
        return Err(format!(
            "{relative} storage capability must not implement traits that could leak, execute, dereference, convert, or drop transaction authority"
        ));
    }
    let use_routes = collect_top_level_use_routes(&file)
        .into_iter()
        .map(|(_, route)| route)
        .collect::<BTreeSet<_>>();
    for required in [
        "crate::error::RadrootsEventStoreError",
        "sqlx::Sqlite",
        "sqlx::Transaction",
    ] {
        if !use_routes.contains(required) {
            return Err(format!(
                "{relative} must import exact post-core storage route `{required}`"
            ));
        }
    }
    let expected_use_routes = [
        "crate::error::RadrootsEventStoreError",
        "crate::model::RadrootsTransportObservation",
        "radroots_event::envelope::RadrootsEventEnvelope",
        "radroots_event::ids::RadrootsTradeCandidateId",
        "radroots_event::ids::RadrootsTradeMutationId",
        "radroots_event::trade::RadrootsSellerReservationAssertionV1",
        "radroots_event::trade::RadrootsTradeMutationEnvelopeV1",
        "radroots_event::trade::RadrootsTradeMutationKindV1",
        "radroots_transport::RadrootsTransportKind",
        "sqlx::Sqlite",
        "sqlx::Transaction",
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    if use_routes
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>()
        != expected_use_routes
    {
        return Err(format!(
            "{relative} production storage imports must match the exact pure-data and SQL capability allowlist"
        ));
    }
    for route in &use_routes {
        if route.ends_with("::*")
            || route.contains(" as ")
            || route.starts_with("std::")
            || route.starts_with("tokio::")
            || route.starts_with("async_std::")
            || route.starts_with("smol::")
            || route.starts_with("reqwest::")
            || route.starts_with("ureq::")
            || route.starts_with("crate::schema::")
            || route.starts_with("crate::nip09::")
            || route.starts_with("super::protocol_reconciliation_v1::")
        {
            return Err(format!(
                "{relative} production storage import `{route}` crosses protocol or schema authority"
            ));
        }
    }

    let storage_structs = file
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Struct(item) if item.ident == "PostCoreStorageV1" => Some(item),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [storage_struct] = storage_structs.as_slice() else {
        return Err(format!(
            "{relative} must define exactly one `PostCoreStorageV1` capability"
        ));
    };
    if !is_pub_super(&storage_struct.vis)
        || !storage_struct.attrs.is_empty()
        || compact_tokens(&storage_struct.generics) != "<'borrow,'db>"
    {
        return Err(format!(
            "{relative} `PostCoreStorageV1` visibility, attributes, or lifetimes drifted"
        ));
    }
    let syn::Fields::Named(fields) = &storage_struct.fields else {
        return Err(format!(
            "{relative} `PostCoreStorageV1` must use one named private transaction field"
        ));
    };
    if fields.named.len() != 1 {
        return Err(format!(
            "{relative} `PostCoreStorageV1` must contain exactly one field"
        ));
    }
    let field = fields.named.first().expect("validated one storage field");
    if !is_inherited_visibility(&field.vis)
        || field.ident.as_ref().is_none_or(|ident| ident != "tx")
        || compact_tokens(&field.ty)
            != compact_source_tokens("&'borrow mut Transaction<'db, Sqlite>")
    {
        return Err(format!(
            "{relative} `PostCoreStorageV1` must contain only private `tx: &'borrow mut Transaction<'db, Sqlite>` authority"
        ));
    }

    let structs = file
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Struct(item) => Some((item.ident.to_string(), item)),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    if structs.keys().map(String::as_str).collect::<Vec<_>>()
        != ["PostCoreStorageV1", "TradeProjectionWrite"]
    {
        return Err(format!(
            "{relative} production storage may define only `PostCoreStorageV1` and `TradeProjectionWrite`"
        ));
    }
    let writer = structs
        .get("TradeProjectionWrite")
        .expect("validated writer struct");
    let syn::Fields::Named(writer_fields) = &writer.fields else {
        return Err(format!(
            "{relative} `TradeProjectionWrite` must use named private data fields"
        ));
    };
    if !is_pub_super(&writer.vis)
        || !writer.attrs.is_empty()
        || compact_tokens(&writer.generics) != "<'a>"
        || writer_fields.named.is_empty()
    {
        return Err(format!(
            "{relative} `TradeProjectionWrite` visibility, lifetime, attributes, or shape drifted"
        ));
    }
    for field in &writer_fields.named {
        let field_type = compact_tokens(&field.ty);
        if !is_inherited_visibility(&field.vis)
            || field.ident.is_none()
            || [
                "Transaction",
                "Sqlite",
                "PostCoreStorageV1",
                "dyn",
                "fn(",
                "impl",
                "&mut",
                "*const",
                "*mut",
            ]
            .iter()
            .any(|forbidden| field_type.contains(forbidden))
        {
            return Err(format!(
                "{relative} `TradeProjectionWrite` fields must remain private typed data without transaction, callback, pointer, or capability authority"
            ));
        }
    }

    let storage_impls = file
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Impl(item)
                if item.trait_.is_none()
                    && simple_type_name(&item.self_ty).as_deref() == Some("PostCoreStorageV1") =>
            {
                Some(item)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let [storage_impl] = storage_impls.as_slice() else {
        return Err(format!(
            "{relative} must define exactly one inherent `PostCoreStorageV1` implementation"
        ));
    };
    if !storage_impl.attrs.is_empty()
        || storage_impl.unsafety.is_some()
        || compact_tokens(&storage_impl.generics) != "<'borrow,'db>"
        || compact_tokens(&storage_impl.self_ty) != "PostCoreStorageV1<'borrow,'db>"
    {
        return Err(format!(
            "{relative} `PostCoreStorageV1` implementation header drifted"
        ));
    }
    let methods = storage_impl
        .items
        .iter()
        .map(|item| match item {
            syn::ImplItem::Fn(function) => Ok(function),
            _ => Err(format!(
                "{relative} `PostCoreStorageV1` implementation may contain only methods"
            )),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let exposed = methods
        .iter()
        .filter(|function| is_pub_super(&function.vis))
        .map(|function| function.sig.ident.to_string())
        .collect::<Vec<_>>();
    if exposed != POST_CORE_STORAGE_METHODS {
        return Err(format!(
            "{relative} exposed storage capability surface drifted: expected {:?}, found {exposed:?}",
            POST_CORE_STORAGE_METHODS
        ));
    }
    for method in &methods {
        let name = method.sig.ident.to_string();
        if !POST_CORE_STORAGE_METHODS.contains(&name.as_str())
            && !is_inherited_visibility(&method.vis)
        {
            return Err(format!(
                "{relative} private storage helper `{name}` must not be externally visible"
            ));
        }
        validate_post_core_storage_method_signature(relative, method)?;
    }

    for item in &file.items {
        match item {
            syn::Item::Use(_) | syn::Item::Struct(_) => {}
            syn::Item::Impl(item)
                if item.trait_.is_none()
                    && simple_type_name(&item.self_ty).as_deref() == Some("PostCoreStorageV1") => {}
            syn::Item::Impl(item)
                if item.trait_.is_none()
                    && simple_type_name(&item.self_ty).as_deref()
                        == Some("TradeProjectionWrite") =>
            {
                for impl_item in &item.items {
                    let syn::ImplItem::Fn(function) = impl_item else {
                        return Err(format!(
                            "{relative} `TradeProjectionWrite` implementation may contain only methods"
                        ));
                    };
                    if function.sig.ident != "new" || !is_pub_super(&function.vis) {
                        return Err(format!(
                            "{relative} `TradeProjectionWrite` may expose only its typed `new` constructor"
                        ));
                    }
                    if function.attrs.len() != 1
                        || !function.attrs.iter().all(|attribute| {
                            compact_tokens(attribute) == "#[allow(clippy::too_many_arguments)]"
                        })
                        || function.sig.unsafety.is_some()
                        || function.sig.abi.is_some()
                        || function.sig.variadic.is_some()
                        || storage_signature_contains_raw_authority(&function.sig)
                    {
                        return Err(format!(
                            "{relative} `TradeProjectionWrite::new` attributes or callable authority drifted"
                        ));
                    }
                    let name = function.sig.ident.to_string();
                    let statements =
                        collect_post_core_storage_sql(relative, &name, &function.block)?;
                    if !statements.is_empty() {
                        return Err(format!(
                            "{relative} `TradeProjectionWrite` must not own SQL authority"
                        ));
                    }
                }
            }
            syn::Item::Fn(function) => {
                if !is_inherited_visibility(&function.vis)
                    || !function.attrs.is_empty()
                    || storage_signature_contains_raw_authority(&function.sig)
                {
                    return Err(format!(
                        "{relative} free storage helpers must remain untransformed, private, callback-free, and transaction-free"
                    ));
                }
                let name = function.sig.ident.to_string();
                let statements = collect_post_core_storage_sql(relative, &name, &function.block)?;
                if !statements.is_empty() {
                    return Err(format!(
                        "{relative} free helper `{}` must not own SQL authority",
                        function.sig.ident
                    ));
                }
            }
            _ => {
                return Err(format!(
                    "{relative} production storage source contains an unsupported authority boundary"
                ));
            }
        }
    }

    let mut statements = Vec::new();
    for method in methods {
        let name = method.sig.ident.to_string();
        statements.extend(collect_post_core_storage_sql(
            relative,
            &name,
            &method.block,
        )?);
    }
    Ok(statements)
}

fn validate_post_core_storage_method_signature(
    relative: &str,
    method: &syn::ImplItemFn,
) -> Result<(), String> {
    let name = method.sig.ident.to_string();
    let expected = match name.as_str() {
        "new" => Some("fn new(tx:&'borrow mut Transaction<'db,Sqlite>)->Self"),
        "quarantine_trade" => Some(
            "async fn quarantine_trade(
                &mut self,
                trade_id: Option<&str>,
                mutation_id: Option<&str>,
                transport_event_id: Option<&str>,
                reason: &str,
                observed_at_ms: i64,
            ) -> Result<(), RadrootsEventStoreError>",
        ),
        "persist_trade_projection" => Some(
            "async fn persist_trade_projection(
                &mut self,
                write: TradeProjectionWrite<'_>,
            ) -> Result<(), RadrootsEventStoreError>",
        ),
        "upsert_transport_observation" => Some(
            "async fn upsert_transport_observation(
                &mut self,
                event_id: &str,
                observation: &RadrootsTransportObservation,
            ) -> Result<(), RadrootsEventStoreError>",
        ),
        _ => None,
    };
    if let Some(expected) = expected {
        if !method.attrs.is_empty()
            || compact_tokens(&method.sig) != compact_source_tokens(expected)
            || (name == "new" && compact_tokens(&method.block) != "{Self{tx}}")
        {
            return Err(format!(
                "{relative} exposed storage method `{name}` signature, attributes, or constructor body drifted"
            ));
        }
    } else if !method.attrs.is_empty() || storage_signature_contains_raw_authority(&method.sig) {
        return Err(format!(
            "{relative} private storage helper `{name}` must remain untransformed and must not receive or return raw transaction, connection, callback, future, or pointer authority"
        ));
    }
    Ok(())
}

fn storage_signature_contains_raw_authority(signature: &syn::Signature) -> bool {
    signature.unsafety.is_some()
        || signature.abi.is_some()
        || signature.variadic.is_some()
        || !signature.generics.params.is_empty()
        || signature.generics.where_clause.is_some()
        || [
            "Acquire",
            "Connection",
            "Executor",
            "Fn",
            "FnMut",
            "FnOnce",
            "Future",
            "PoolConnection",
            "Sqlite",
            "SqliteConnection",
            "SqlitePool",
            "Stream",
            "Transaction",
        ]
        .iter()
        .any(|ident| syntax_contains_ident(signature, ident))
        || {
            let compact = compact_tokens(signature);
            compact.contains("dyn")
                || compact.contains("fn(")
                || compact.contains("impl")
                || compact.contains("*const")
                || compact.contains("*mut")
        }
}

fn collect_post_core_storage_sql(
    relative: &str,
    function: &str,
    block: &syn::Block,
) -> Result<Vec<PostCoreSqlStatementDescriptor>, String> {
    let mut collector = PostCoreStorageSqlCollector {
        relative,
        function,
        statements: Vec::new(),
        query_constructors: 0,
        database_terminals: 0,
        transaction_field_uses: 0,
        approved_transaction_field_uses: 0,
        allow_direct_self_path: false,
        error: None,
    };
    syn::visit::Visit::visit_block(&mut collector, block);
    collector.finish()?;
    Ok(collector.statements)
}

fn is_pub_super(visibility: &syn::Visibility) -> bool {
    matches!(
        visibility,
        syn::Visibility::Restricted(restricted)
            if restricted.in_token.is_none() && restricted.path.is_ident("super")
    )
}

fn is_inherited_visibility(visibility: &syn::Visibility) -> bool {
    matches!(visibility, syn::Visibility::Inherited)
}

fn direct_expression_call_route(expression: &syn::ExprCall) -> Option<String> {
    let syn::Expr::Path(path) = expression.func.as_ref() else {
        return None;
    };
    if path.qself.is_some() {
        return None;
    }
    Some(
        path.path
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect::<Vec<_>>()
            .join("::"),
    )
}

struct PostCoreExtensionAuthorityAudit<'a> {
    relative: &'a str,
    function: &'a str,
    local_functions: &'a BTreeSet<String>,
    error: Option<String>,
}

impl PostCoreExtensionAuthorityAudit<'_> {
    fn finish(self) -> Result<(), String> {
        self.error.map_or(Ok(()), Err)
    }

    fn fail(&mut self, reason: impl Into<String>) {
        if self.error.is_none() {
            self.error = Some(format!(
                "{} post-core extension `{}` {}",
                self.relative,
                self.function,
                reason.into()
            ));
        }
    }
}

impl<'ast> syn::visit::Visit<'ast> for PostCoreExtensionAuthorityAudit<'_> {
    fn visit_stmt(&mut self, statement: &'ast syn::Stmt) {
        if matches!(statement, syn::Stmt::Item(_)) {
            self.fail("must not contain block-local item, import, alias, or macro declarations");
            return;
        }
        syn::visit::visit_stmt(self, statement);
    }

    fn visit_pat_ident(&mut self, pattern: &'ast syn::PatIdent) {
        let binding = pattern.ident.to_string();
        if self.local_functions.contains(&binding)
            || matches!(
                binding.as_str(),
                "Ok" | "Sha256"
                    | "Some"
                    | "TradeProjectionWrite"
                    | "hex"
                    | "trade_mutation_from_canonical_content"
            )
        {
            self.fail(format!(
                "must not shadow governed extension callee `{binding}`"
            ));
            return;
        }
        syn::visit::visit_pat_ident(self, pattern);
    }

    fn visit_expr_call(&mut self, expression: &'ast syn::ExprCall) {
        let route = direct_expression_call_route(expression);
        if let Some(route) = route.as_deref() {
            let terminal = route.rsplit("::").next().unwrap_or(route);
            let local = self.local_functions.contains(terminal)
                && matches!(
                    route.split("::").collect::<Vec<_>>().as_slice(),
                    [_] | ["self", _]
                );
            let pure = matches!(
                route,
                "Ok" | "Some"
                    | "Sha256::digest"
                    | "TradeProjectionWrite::new"
                    | "hex::encode"
                    | "trade_mutation_from_canonical_content"
            );
            if !local && !pure {
                self.fail(format!(
                    "calls route `{route}` outside the local/pure-domain allowlist"
                ));
            }
        } else {
            self.fail("must not use an indirect or qualified-self callable");
        }
        if expression
            .args
            .iter()
            .any(|argument| syntax_contains_ident(argument, "storage"))
        {
            let local = route
                .as_deref()
                .and_then(|route| route.rsplit("::").next())
                .is_some_and(|name| self.local_functions.contains(name));
            let exact_arguments = expression
                .args
                .iter()
                .filter(|argument| syntax_contains_ident(*argument, "storage"))
                .all(|argument| compact_tokens(argument) == "storage");
            if !local || !exact_arguments {
                self.fail(format!(
                    "must not pass storage capability through external or indirect call `{}`",
                    route.as_deref().unwrap_or("<indirect>")
                ));
            }
        }
        syn::visit::visit_expr_call(self, expression);
    }

    fn visit_expr_method_call(&mut self, expression: &'ast syn::ExprMethodCall) {
        let method = expression.method.to_string();
        if matches!(
            method.as_str(),
            "bind"
                | "connect"
                | "create"
                | "current_dir"
                | "kill"
                | "open"
                | "read"
                | "read_to_end"
                | "remove_var"
                | "set_current_dir"
                | "set_var"
                | "spawn"
                | "wait"
                | "write"
                | "write_all"
        ) {
            self.fail(format!(
                "calls forbidden ambient side-effect method `{method}`"
            ));
        }
        if syntax_contains_ident(&expression.receiver, "storage")
            && (compact_tokens(&expression.receiver) != "storage"
                || !POST_CORE_STORAGE_METHODS[1..].contains(&method.as_str()))
        {
            self.fail(format!(
                "calls storage through unsupported capability method `{method}`"
            ));
        }
        if expression
            .args
            .iter()
            .any(|argument| syntax_contains_ident(argument, "storage"))
        {
            self.fail(format!(
                "must not pass storage capability as an argument to method `{}`",
                expression.method
            ));
        }
        syn::visit::visit_expr_method_call(self, expression);
    }

    fn visit_expr_field(&mut self, expression: &'ast syn::ExprField) {
        if syntax_contains_ident(&expression.base, "storage") {
            self.fail("must not access storage capability fields");
        }
        syn::visit::visit_expr_field(self, expression);
    }

    fn visit_expr_path(&mut self, expression: &'ast syn::ExprPath) {
        let route = expression
            .path
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect::<Vec<_>>();
        let qualified_route_allowed = route.len() <= 1
            || route.first().is_some_and(|segment| {
                matches!(
                    segment.as_str(),
                    "RadrootsTradeDecisionV1"
                        | "RadrootsTradeMutationBodyV1"
                        | "Sha256"
                        | "TradeProjectionWrite"
                        | "hex"
                )
            });
        if !qualified_route_allowed
            || route.first().is_some_and(|segment| {
                matches!(
                    segment.as_str(),
                    "async_std" | "reqwest" | "smol" | "sqlx" | "std" | "tokio" | "ureq"
                )
            })
            || route.iter().any(|segment| {
                matches!(
                    segment.as_str(),
                    "Command"
                        | "File"
                        | "OpenOptions"
                        | "TcpListener"
                        | "TcpStream"
                        | "UdpSocket"
                        | "Sqlite"
                        | "SqliteConnection"
                        | "SqlitePool"
                        | "Transaction"
                        | "current_dir"
                        | "ingest_event_protocol_reconciliation_v1"
                        | "remove_var"
                        | "set_current_dir"
                        | "set_var"
                        | "spawn"
                        | "validate_protocol_post_extensions"
                        | "var"
                )
            })
        {
            self.fail(format!(
                "references forbidden SQL or protocol authority `{}`",
                route.join("::")
            ));
        }
        syn::visit::visit_expr_path(self, expression);
    }

    fn visit_macro(&mut self, item: &'ast syn::Macro) {
        self.fail(format!(
            "must not hide authority behind macro `{}`",
            compact_tokens(&item.path)
        ));
    }

    fn visit_expr_unsafe(&mut self, _expression: &'ast syn::ExprUnsafe) {
        self.fail("must not contain unsafe authority access");
    }
}

struct PostCoreStorageSqlCollector<'a> {
    relative: &'a str,
    function: &'a str,
    statements: Vec<PostCoreSqlStatementDescriptor>,
    query_constructors: usize,
    database_terminals: usize,
    transaction_field_uses: usize,
    approved_transaction_field_uses: usize,
    allow_direct_self_path: bool,
    error: Option<String>,
}

impl PostCoreStorageSqlCollector<'_> {
    fn finish(&self) -> Result<(), String> {
        if let Some(error) = &self.error {
            return Err(error.clone());
        }
        if self.query_constructors != self.database_terminals {
            return Err(format!(
                "{} post-core storage `{}` contains {} query constructors but {} authenticated database terminals",
                self.relative, self.function, self.query_constructors, self.database_terminals,
            ));
        }
        if self.transaction_field_uses != self.approved_transaction_field_uses {
            return Err(format!(
                "{} post-core storage `{}` contains {} transaction-field uses but only {} exact SQL-terminal uses",
                self.relative,
                self.function,
                self.transaction_field_uses,
                self.approved_transaction_field_uses,
            ));
        }
        Ok(())
    }

    fn fail(&mut self, message: impl Into<String>) {
        if self.error.is_none() {
            self.error = Some(format!(
                "{} post-core SQL capability `{}` {}",
                self.relative,
                self.function,
                message.into()
            ));
        }
    }
}

impl<'ast> syn::visit::Visit<'ast> for PostCoreStorageSqlCollector<'_> {
    fn visit_expr_call(&mut self, expression: &'ast syn::ExprCall) {
        if expression.args.iter().any(|argument| {
            expression_contains_storage_transaction(argument)
                || expression_contains_storage_self(argument)
        }) {
            self.fail("must not pass storage or transaction authority to a function or callback");
        }
        let Some(route) = direct_expression_call_route(expression) else {
            syn::visit::visit_expr_call(self, expression);
            return;
        };
        if route.contains("::")
            && !matches!(
                route.as_str(),
                "i64::from"
                    | "i64::try_from"
                    | "serde_json::to_string"
                    | "sqlx::query"
                    | "sqlx::query_as"
                    | "sqlx::query_scalar"
            )
        {
            self.fail(format!(
                "calls fully-qualified route `{route}` outside the pure-conversion and SQL allowlist"
            ));
        }
        if route.starts_with("sqlx::") {
            if !matches!(
                route.as_str(),
                "sqlx::query" | "sqlx::query_as" | "sqlx::query_scalar"
            ) {
                self.fail(format!(
                    "must not use unsupported sqlx constructor `{route}`"
                ));
            }
            self.query_constructors += 1;
        }
        syn::visit::visit_expr_call(self, expression);
    }

    fn visit_expr_path(&mut self, expression: &'ast syn::ExprPath) {
        let route = expression
            .path
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect::<Vec<_>>();
        if route.first().is_some_and(|segment| {
            matches!(
                segment.as_str(),
                "async_std" | "reqwest" | "smol" | "std" | "tokio" | "ureq"
            )
        }) || route.iter().any(|segment| {
            matches!(
                segment.as_str(),
                "Command"
                    | "File"
                    | "OpenOptions"
                    | "TcpListener"
                    | "TcpStream"
                    | "UdpSocket"
                    | "current_dir"
                    | "remove_var"
                    | "set_current_dir"
                    | "set_var"
                    | "spawn"
                    | "var"
            )
        }) {
            self.fail(format!(
                "references forbidden ambient side-effect authority `{}`",
                route.join("::")
            ));
        }
        if expression.qself.is_none()
            && expression.path.is_ident("self")
            && !self.allow_direct_self_path
        {
            self.fail("must not alias, move, return, or indirectly expose the storage capability");
        }
        syn::visit::visit_expr_path(self, expression);
    }

    fn visit_pat_struct(&mut self, pattern: &'ast syn::PatStruct) {
        if pattern
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "Self" || segment.ident == "PostCoreStorageV1")
        {
            self.fail("must not destructure the storage capability or transaction field");
            return;
        }
        syn::visit::visit_pat_struct(self, pattern);
    }

    fn visit_stmt(&mut self, statement: &'ast syn::Stmt) {
        if matches!(statement, syn::Stmt::Item(_)) {
            self.fail("must not contain block-local item or `use` declarations");
            return;
        }
        syn::visit::visit_stmt(self, statement);
    }

    fn visit_macro(&mut self, item: &'ast syn::Macro) {
        self.fail(format!(
            "must not contain macro expansion `{}`",
            compact_tokens(&item.path)
        ));
    }

    fn visit_expr_unsafe(&mut self, _expression: &'ast syn::ExprUnsafe) {
        self.fail("must not contain an unsafe block");
    }

    fn visit_expr_method_call(&mut self, expression: &'ast syn::ExprMethodCall) {
        let method = expression.method.to_string();
        let direct_self_receiver = matches!(
            expression.receiver.as_ref(),
            syn::Expr::Path(path) if path.qself.is_none() && path.path.is_ident("self")
        );
        if matches!(
            method.as_str(),
            "connect"
                | "create"
                | "current_dir"
                | "kill"
                | "open"
                | "read"
                | "read_to_end"
                | "remove_var"
                | "set_current_dir"
                | "set_var"
                | "spawn"
                | "wait"
                | "write"
                | "write_all"
        ) {
            self.fail(format!(
                "calls forbidden ambient side-effect method `{method}`"
            ));
        }
        let database_terminal = matches!(
            method.as_str(),
            "execute" | "fetch" | "fetch_all" | "fetch_many" | "fetch_one" | "fetch_optional"
        );
        if database_terminal {
            self.database_terminals += 1;
            match post_core_sql_from_terminal(expression, "&mut**self.tx") {
                Ok(terminal) => match describe_post_core_sql(
                    self.function,
                    &terminal.sql,
                    &terminal.terminal,
                    &terminal.bind_expressions,
                ) {
                    Ok(statement) => {
                        self.approved_transaction_field_uses += 1;
                        self.statements.push(statement);
                    }
                    Err(error) => self.fail(error),
                },
                Err(error) => self.fail(error),
            }
        } else if expression_contains_storage_transaction(&expression.receiver)
            || (!direct_self_receiver && expression_contains_storage_self(&expression.receiver))
            || expression.args.iter().any(|argument| {
                expression_contains_storage_transaction(argument)
                    || expression_contains_storage_self(argument)
            })
        {
            self.fail(format!(
                "must not pass storage or transaction authority through method `{method}` outside an authenticated literal SQL terminal"
            ));
        }
        if direct_self_receiver {
            for attribute in &expression.attrs {
                syn::visit::Visit::visit_attribute(self, attribute);
            }
            let previous = self.allow_direct_self_path;
            self.allow_direct_self_path = true;
            syn::visit::Visit::visit_expr(self, expression.receiver.as_ref());
            self.allow_direct_self_path = previous;
            if let Some(turbofish) = &expression.turbofish {
                syn::visit::Visit::visit_angle_bracketed_generic_arguments(self, turbofish);
            }
            for argument in &expression.args {
                syn::visit::Visit::visit_expr(self, argument);
            }
        } else {
            syn::visit::visit_expr_method_call(self, expression);
        }
    }

    fn visit_expr_field(&mut self, expression: &'ast syn::ExprField) {
        if matches!(&expression.member, syn::Member::Named(ident) if ident == "tx")
            && matches!(
                expression.base.as_ref(),
                syn::Expr::Path(path)
                    if path.qself.is_none() && path.path.is_ident("self")
            )
        {
            self.transaction_field_uses += 1;
            for attribute in &expression.attrs {
                syn::visit::Visit::visit_attribute(self, attribute);
            }
            let previous = self.allow_direct_self_path;
            self.allow_direct_self_path = true;
            syn::visit::Visit::visit_expr(self, expression.base.as_ref());
            self.allow_direct_self_path = previous;
            return;
        }
        syn::visit::visit_expr_field(self, expression);
    }
}

fn expression_contains_storage_self(expression: &syn::Expr) -> bool {
    struct StorageSelfUse {
        found: bool,
    }

    impl<'ast> syn::visit::Visit<'ast> for StorageSelfUse {
        fn visit_expr_path(&mut self, expression: &'ast syn::ExprPath) {
            if expression.qself.is_none() && expression.path.is_ident("self") {
                self.found = true;
            }
            syn::visit::visit_expr_path(self, expression);
        }
    }

    let mut audit = StorageSelfUse { found: false };
    syn::visit::Visit::visit_expr(&mut audit, expression);
    audit.found
}

fn expression_contains_storage_transaction(expression: &syn::Expr) -> bool {
    struct StorageTransactionUse {
        found: bool,
    }

    impl<'ast> syn::visit::Visit<'ast> for StorageTransactionUse {
        fn visit_expr_field(&mut self, expression: &'ast syn::ExprField) {
            if matches!(&expression.member, syn::Member::Named(ident) if ident == "tx")
                && matches!(
                    expression.base.as_ref(),
                    syn::Expr::Path(path)
                        if path.qself.is_none() && path.path.is_ident("self")
                )
            {
                self.found = true;
            }
            syn::visit::visit_expr_field(self, expression);
        }
    }

    let mut audit = StorageTransactionUse { found: false };
    syn::visit::Visit::visit_expr(&mut audit, expression);
    audit.found
}

fn post_core_sql_from_terminal(
    expression: &syn::ExprMethodCall,
    transaction_target: &str,
) -> Result<PostCoreSqlTerminal, String> {
    if expression.args.len() != 1
        || compact_tokens(
            expression
                .args
                .first()
                .expect("validated one terminal argument"),
        ) != transaction_target
    {
        return Err(format!(
            "database terminal `{}` must execute only on `{transaction_target}`",
            expression.method,
        ));
    }
    if !expression.attrs.is_empty() || expression.turbofish.is_some() {
        return Err(format!(
            "database terminal `{}` must not carry attributes or generic arguments",
            expression.method,
        ));
    }

    let mut receiver = expression.receiver.as_ref();
    let mut bind_expressions = Vec::new();
    loop {
        receiver = match peel_expression(receiver) {
            syn::Expr::MethodCall(call) if call.method == "bind" => {
                if !call.attrs.is_empty() || call.turbofish.is_some() || call.args.len() != 1 {
                    return Err(
                        "each database `.bind(...)` must contain exactly one direct, untransformed expression"
                            .to_owned(),
                    );
                }
                bind_expressions.push(compact_tokens(
                    call.args
                        .first()
                        .expect("validated one database bind argument"),
                ));
                call.receiver.as_ref()
            }
            syn::Expr::Call(call) => {
                let syn::Expr::Path(path) = call.func.as_ref() else {
                    return Err(
                        "database terminal must originate from a direct sqlx query constructor"
                            .to_owned(),
                    );
                };
                let route = path
                    .path
                    .segments
                    .iter()
                    .map(|segment| segment.ident.to_string())
                    .collect::<Vec<_>>()
                    .join("::");
                if path.qself.is_some()
                    || path.path.leading_colon.is_some()
                    || !call.attrs.is_empty()
                    || !matches!(
                        route.as_str(),
                        "sqlx::query" | "sqlx::query_as" | "sqlx::query_scalar"
                    )
                    || call.args.len() != 1
                {
                    return Err(format!(
                        "database terminal must originate from a one-literal sqlx query constructor; found `{route}`"
                    ));
                }
                let Some(syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Str(sql),
                    ..
                })) = call.args.first()
                else {
                    return Err("post-core SQL must be a compile-time string literal".to_owned());
                };
                bind_expressions.reverse();
                return Ok(PostCoreSqlTerminal {
                    sql: sql.value(),
                    terminal: expression.method.to_string(),
                    bind_expressions,
                });
            }
            _ => {
                return Err(
                    "database terminal receiver may contain only direct `.bind(...)` calls"
                        .to_owned(),
                );
            }
        };
    }
}

fn describe_post_core_sql(
    function: &str,
    sql: &str,
    terminal: &str,
    bind_expressions: &[String],
) -> Result<PostCoreSqlStatementDescriptor, String> {
    let tokens = lex_post_core_sql(sql)?;
    let Some(initial_operation) = tokens.first().and_then(PostCoreSqlToken::word) else {
        return Err("post-core SQL literal must not be empty".to_owned());
    };
    if !matches!(initial_operation, "DELETE" | "INSERT" | "SELECT") {
        return Err(format!(
            "post-core SQL operation `{initial_operation}` is outside the read/append capability"
        ));
    }
    for forbidden in [
        "ALTER",
        "ATTACH",
        "BEGIN",
        "COMMIT",
        "CREATE",
        "DETACH",
        "DROP",
        "EXCEPT",
        "INTERSECT",
        "PRAGMA",
        "REINDEX",
        "RELEASE",
        "REPLACE",
        "RETURNING",
        "ROLLBACK",
        "SAVEPOINT",
        "TRIGGER",
        "UNION",
        "VACUUM",
        "WITH",
    ] {
        if tokens.iter().any(|token| token.word() == Some(forbidden)) {
            return Err(format!(
                "post-core SQL must not contain forbidden keyword `{forbidden}`"
            ));
        }
    }
    let words = tokens
        .iter()
        .filter_map(PostCoreSqlToken::word)
        .collect::<Vec<_>>();
    for (index, token) in words.iter().enumerate() {
        if *token == "UPDATE"
            && !(index > 0
                && words[index - 1] == "DO"
                && words.get(index + 1).is_some_and(|next| *next == "SET"))
        {
            return Err("post-core SQL permits UPDATE only in INSERT ... DO UPDATE SET".to_owned());
        }
    }
    let operation = if initial_operation == "INSERT"
        && words
            .windows(3)
            .any(|window| window == ["DO", "UPDATE", "SET"])
    {
        "UPSERT"
    } else {
        initial_operation
    };

    let (table_index, table) = match initial_operation {
        "SELECT" => {
            if tokens.len() < 4
                || tokens.first().and_then(PostCoreSqlToken::word) != Some("SELECT")
                || !matches!(tokens.get(1), Some(PostCoreSqlToken::Number(value)) if value == "1")
                || tokens.get(2).and_then(PostCoreSqlToken::word) != Some("FROM")
                || words.iter().filter(|word| **word == "SELECT").count() != 1
                || words.iter().filter(|word| **word == "FROM").count() != 1
                || words.iter().any(|word| matches!(*word, "INTO" | "JOIN"))
                || tokens
                    .iter()
                    .any(|token| matches!(token, PostCoreSqlToken::Comma))
            {
                return Err(
                    "post-core SELECT must be the single-table `SELECT 1 FROM <table> ...` existence shape"
                        .to_owned(),
                );
            }
            let table = tokens
                .get(3)
                .and_then(PostCoreSqlToken::word)
                .ok_or_else(|| "post-core SELECT FROM must name one table".to_owned())?;
            (3, table)
        }
        "DELETE" => {
            if tokens.len() < 3
                || tokens.get(1).and_then(PostCoreSqlToken::word) != Some("FROM")
                || words.iter().filter(|word| **word == "DELETE").count() != 1
                || words.iter().filter(|word| **word == "FROM").count() != 1
                || words
                    .iter()
                    .any(|word| matches!(*word, "SELECT" | "INSERT" | "INTO" | "JOIN"))
                || tokens
                    .iter()
                    .any(|token| matches!(token, PostCoreSqlToken::Comma))
            {
                return Err(
                    "post-core DELETE must target exactly one direct table without subqueries or joins"
                        .to_owned(),
                );
            }
            let table = tokens
                .get(2)
                .and_then(PostCoreSqlToken::word)
                .ok_or_else(|| "post-core DELETE FROM must name one table".to_owned())?;
            (2, table)
        }
        "INSERT" => {
            let into_index = match (
                tokens.get(1).and_then(PostCoreSqlToken::word),
                tokens.get(2).and_then(PostCoreSqlToken::word),
                tokens.get(3).and_then(PostCoreSqlToken::word),
            ) {
                (Some("INTO"), _, _) => 1,
                (Some("OR"), Some("IGNORE"), Some("INTO")) => 3,
                _ => {
                    return Err(
                        "post-core INSERT permits only `INSERT INTO` or `INSERT OR IGNORE INTO`"
                            .to_owned(),
                    );
                }
            };
            if words.iter().filter(|word| **word == "INSERT").count() != 1
                || words.iter().filter(|word| **word == "INTO").count() != 1
                || words
                    .iter()
                    .any(|word| matches!(*word, "DELETE" | "FROM" | "JOIN" | "SELECT"))
            {
                return Err(
                    "post-core INSERT must target exactly one direct table without subqueries or joins"
                        .to_owned(),
                );
            }
            let table_index = into_index + 1;
            let table = tokens
                .get(table_index)
                .and_then(PostCoreSqlToken::word)
                .ok_or_else(|| "post-core INSERT INTO must name one table".to_owned())?;
            if !matches!(
                tokens.get(table_index + 1),
                Some(PostCoreSqlToken::LeftParen)
            ) {
                return Err(
                    "post-core INSERT must declare its target column list directly after the table"
                        .to_owned(),
                );
            }
            (table_index, table)
        }
        _ => unreachable!("validated initial post-core operation"),
    };
    if matches!(tokens.get(table_index + 1), Some(PostCoreSqlToken::Dot)) {
        return Err("post-core SQL must not use an attached or schema-qualified table".to_owned());
    }
    let table = table.to_ascii_lowercase();
    if !POST_CORE_SQL_ALLOWED_CAPABILITIES
        .iter()
        .any(|capability| capability.table == table)
    {
        return Err(format!(
            "post-core SQL table `{table}` is outside the explicit trade/observation capability"
        ));
    }
    if !POST_CORE_SQL_ALLOWED_CAPABILITIES.iter().any(|capability| {
        capability.operation.eq_ignore_ascii_case(operation) && capability.table == table
    }) {
        return Err(format!(
            "post-core SQL `{}` on table `{table}` is outside the explicit operation/table capability",
            operation.to_ascii_lowercase()
        ));
    }
    let placeholder_count = tokens
        .iter()
        .filter(|token| matches!(token, PostCoreSqlToken::Placeholder))
        .count();
    if placeholder_count != bind_expressions.len() {
        return Err(format!(
            "post-core SQL placeholder/bind cardinality drifted: found {placeholder_count} placeholders and {} ordered bind expressions",
            bind_expressions.len()
        ));
    }

    for (index, token) in tokens.iter().enumerate() {
        let Some(name) = token.word() else {
            continue;
        };
        if !matches!(tokens.get(index + 1), Some(PostCoreSqlToken::LeftParen)) {
            continue;
        }
        let structural = (initial_operation == "INSERT" && index == table_index)
            || name == "VALUES"
            || (name == "CONFLICT"
                && index > 0
                && tokens.get(index - 1).and_then(PostCoreSqlToken::word) == Some("ON"));
        let allowed_function = operation == "UPSERT" && matches!(name, "MIN" | "MAX");
        if !structural && !allowed_function {
            return Err(format!(
                "post-core SQL function or query constructor `{name}(...)` is outside the explicit grammar"
            ));
        }
    }

    Ok(PostCoreSqlStatementDescriptor {
        function: function.to_owned(),
        operation: operation.to_ascii_lowercase(),
        tables: vec![table],
        terminal: terminal.to_owned(),
        sql_sha256: sha256_hex(sql.as_bytes()),
        placeholder_count: u64::try_from(placeholder_count)
            .map_err(|_| "post-core SQL placeholder count does not fit u64".to_owned())?,
        bind_expressions: bind_expressions.to_vec(),
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum PostCoreSqlToken {
    Word(String),
    StringLiteral,
    Number(String),
    Placeholder,
    LeftParen,
    RightParen,
    Comma,
    Dot,
    Operator(String),
}

impl PostCoreSqlToken {
    fn word(&self) -> Option<&str> {
        match self {
            Self::Word(word) => Some(word),
            Self::StringLiteral
            | Self::Number(_)
            | Self::Placeholder
            | Self::LeftParen
            | Self::RightParen
            | Self::Comma
            | Self::Dot
            | Self::Operator(_) => None,
        }
    }
}

fn lex_post_core_sql(sql: &str) -> Result<Vec<PostCoreSqlToken>, String> {
    let bytes = sql.as_bytes();
    let mut tokens = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte.is_ascii_whitespace() {
            index += 1;
        } else if byte == b'(' {
            tokens.push(PostCoreSqlToken::LeftParen);
            index += 1;
        } else if byte == b')' {
            tokens.push(PostCoreSqlToken::RightParen);
            index += 1;
        } else if byte == b',' {
            tokens.push(PostCoreSqlToken::Comma);
            index += 1;
        } else if byte == b'.' {
            tokens.push(PostCoreSqlToken::Dot);
            index += 1;
        } else if byte == b'?' {
            tokens.push(PostCoreSqlToken::Placeholder);
            index += 1;
        } else if byte == b'\'' {
            index += 1;
            let mut closed = false;
            while index < bytes.len() {
                if bytes[index] == b'\'' {
                    if bytes.get(index + 1) == Some(&b'\'') {
                        index += 2;
                    } else {
                        index += 1;
                        closed = true;
                        break;
                    }
                } else {
                    index += 1;
                }
            }
            if !closed {
                return Err("post-core SQL contains an unterminated string literal".to_owned());
            }
            tokens.push(PostCoreSqlToken::StringLiteral);
        } else if byte == b';' {
            return Err("post-core SQL must contain exactly one statement".to_owned());
        } else if matches!(byte, b'"' | b'`' | b'[' | b']') {
            return Err(
                "post-core SQL must use unquoted identifiers so table capability checks are unambiguous"
                    .to_owned(),
            );
        } else if (byte == b'-' && bytes.get(index + 1) == Some(&b'-'))
            || (byte == b'/' && bytes.get(index + 1) == Some(&b'*'))
        {
            return Err("post-core SQL comments are forbidden".to_owned());
        } else if byte.is_ascii_alphabetic() || byte == b'_' {
            let start = index;
            index += 1;
            while index < bytes.len()
                && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_')
            {
                index += 1;
            }
            tokens.push(PostCoreSqlToken::Word(
                sql[start..index].to_ascii_uppercase(),
            ));
        } else if byte.is_ascii_digit()
            || (byte == b'-' && bytes.get(index + 1).is_some_and(u8::is_ascii_digit))
        {
            let start = index;
            index += 1;
            while index < bytes.len() && bytes[index].is_ascii_digit() {
                index += 1;
            }
            tokens.push(PostCoreSqlToken::Number(sql[start..index].to_owned()));
        } else if matches!(
            byte,
            b'=' | b'<' | b'>' | b'+' | b'-' | b'*' | b'!' | b'|' | b'&' | b'%' | b'/'
        ) {
            let start = index;
            index += 1;
            while index < bytes.len()
                && matches!(
                    bytes[index],
                    b'=' | b'<' | b'>' | b'+' | b'-' | b'*' | b'!' | b'|' | b'&' | b'%' | b'/'
                )
            {
                index += 1;
            }
            tokens.push(PostCoreSqlToken::Operator(sql[start..index].to_owned()));
        } else {
            return Err(format!(
                "post-core SQL contains unsupported byte `{}`",
                char::from(byte)
            ));
        }
    }
    Ok(tokens)
}

fn validate_required_call_sequence(
    relative: &str,
    item: &str,
    call_routes: &[String],
    required: &[&str],
) -> Result<(), String> {
    let required_set = required.iter().copied().collect::<BTreeSet<_>>();
    let actual = call_routes
        .iter()
        .filter(|route| required_set.contains(route.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    let expected = required
        .iter()
        .map(|route| (*route).to_owned())
        .collect::<Vec<_>>();
    if actual != expected {
        return Err(format!(
            "{relative} witnessed item `{item}` required call sequence drifted: expected {expected:?}, found {actual:?}"
        ));
    }
    Ok(())
}

fn validate_event_store_schema_name_matchers<'a>(
    relative: &str,
    file: &'a syn::File,
) -> Result<[&'a syn::ItemFn; 3], String> {
    const EXPECTED: [(&str, &str); 3] = [
        (
            "is_event_store_owned_table_name",
            r#"pub(crate) fn is_event_store_owned_table_name(
                registry: &[EventStoreMigration],
                name: &str,
            ) -> bool {
                sqlite_identifier_starts_with(name, EVENT_STORE_RESERVED_PREFIX)
                    || registry
                        .iter()
                        .flat_map(|migration| migration.owned_table_names)
                        .any(|owned| name.eq_ignore_ascii_case(owned))
            }"#,
        ),
        (
            "is_event_store_governed_schema_name",
            r#"pub(crate) fn is_event_store_governed_schema_name(
                registry: &[EventStoreMigration],
                name: &str,
            ) -> bool {
                name.eq_ignore_ascii_case(EVENT_STORE_LEDGER_NAME)
                    || is_event_store_owned_table_name(registry, name)
                    || registry
                        .iter()
                        .flat_map(|migration| migration.owned_object_names)
                        .any(|owned| name.eq_ignore_ascii_case(owned))
            }"#,
        ),
        (
            "sqlite_identifier_starts_with",
            r#"pub(crate) fn sqlite_identifier_starts_with(name: &str, prefix: &str) -> bool {
                name.get(..prefix.len())
                    .is_some_and(|candidate| candidate.eq_ignore_ascii_case(prefix))
            }"#,
        ),
    ];

    let functions = EXPECTED
        .map(|(name, expected)| -> Result<&syn::ItemFn, String> {
            let function = exact_top_level_function(relative, file, name)?;
            if compact_tokens(function) != compact_source_tokens(expected) {
                return Err(format!(
                    "{relative} authoritative schema-name matcher `{name}` signature or control flow drifted"
                ));
            }
            Ok(function)
        })
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    functions.try_into().map_err(|_| {
        format!("{relative} authoritative schema-name matcher set has invalid cardinality")
    })
}

fn describe_rust_fragment_witnesses(
    workspace_root: &Path,
) -> Result<Vec<RustFragmentWitnessDescriptor>, String> {
    let store_relative = "crates/event_store/src/store.rs";
    let store_bytes = read_regular_file(workspace_root, store_relative)?;
    let store = parse_canonical_production_rust(store_relative, &store_bytes)?;
    let migrations_bytes =
        read_regular_file(workspace_root, EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE)?;
    let migrations =
        parse_canonical_production_rust(EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE, &migrations_bytes)?;
    validate_migration_registry_reachability(EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE, &migrations)?;
    validate_manifest_validator_reachability(EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE, &migrations)?;
    let schema_name_matchers = validate_event_store_schema_name_matchers(
        EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
        &migrations,
    )?;
    let schema_name_matcher_fragment = quote::quote!(
        #(#schema_name_matchers)*
    );
    let schema_bytes = read_regular_file(workspace_root, EVENT_STORE_SCHEMA_SOURCE_RELATIVE)?;
    let schema =
        parse_canonical_production_rust(EVENT_STORE_SCHEMA_SOURCE_RELATIVE, &schema_bytes)?;
    let schema_runtime =
        validate_schema_runtime_reachability(EVENT_STORE_SCHEMA_SOURCE_RELATIVE, &schema)?;
    let schema_migration_application_functions = [
        "finish_schema_transaction",
        "preserve_primary_failure",
        "rollback_event_store_schema_offline",
        "rollback_event_store_schema_with_registry",
        "migrate_schema_on_connection",
        "rollback_schema_on_connection",
        "apply_migration_up",
        "apply_migration_down",
        "validate_catalog_delta",
        "create_ledger",
        "insert_ledger_row",
        "inspect_schema_on_connection",
        "apply_migration_hook",
        "validate_migration_hook_state",
        "validate_applied_migration_hooks",
        "read_catalog",
        "validate_ledger_catalog",
        "governed_catalog",
        "catalog_fingerprint",
        "read_history",
        "validate_history_against_registry",
        "validate_history_checksum",
        "validate_schema_fingerprint",
        "validate_database_integrity",
    ]
    .map(|name| exact_top_level_function(EVENT_STORE_SCHEMA_SOURCE_RELATIVE, &schema, name))
    .into_iter()
    .collect::<Result<Vec<_>, _>>()?;
    let schema_migration_application_fragment = quote::quote!(
        #(#schema_migration_application_functions)*
    );
    let migration_contract_constants = [
        "EVENT_STORE_LEDGER_NAME",
        "EVENT_STORE_RESERVED_PREFIX",
        "RADROOTS_EVENT_STORE_SCHEMA_VERSION_MIN",
    ]
    .map(|name| exact_executor_const(&migrations, EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE, name))
    .into_iter()
    .collect::<Result<Vec<_>, _>>()?;
    let migration_contract_type = exact_top_level_struct(
        EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
        &migrations,
        "EventStoreMigration",
    )?;
    let migration_contract_fragment = quote::quote!(
        #(#migration_contract_constants)*
        #migration_contract_type
    );
    let migration_catalog_constants = [
        "EVENT_STORE_LEDGER_DDL",
        "EVENT_STORE_LEDGER_CREATE_DDL",
        "EVENT_STORE_BASELINE_OBJECT_NAMES",
        "EVENT_STORE_BASELINE_TABLE_NAMES",
        "EVENT_STORE_BASELINE_FTS5_TABLE_NAMES",
        "EVENT_STORE_NIP09_OBJECT_NAMES",
        "EVENT_STORE_NIP09_TABLE_NAMES",
    ]
    .map(|name| exact_executor_const(&migrations, EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE, name))
    .into_iter()
    .collect::<Result<Vec<_>, _>>()?;
    let migration_catalog_fragment = quote::quote!(
        #(#migration_catalog_constants)*
    );
    let migration_runtime_functions = [
        "migration_for_version",
        "validate_owned_schema_name",
        "validate_embedded_migration_input",
        "validate_sha256_literal",
        "sha256_hex",
    ]
    .map(|name| exact_top_level_function(EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE, &migrations, name))
    .into_iter()
    .collect::<Result<Vec<_>, _>>()?;
    let migration_runtime_fragment = quote::quote!(
        #(#migration_runtime_functions)*
    );

    let begin_immediate = exact_associated_method_call_with_string(
        store_relative,
        &store,
        "RadrootsEventStore",
        "begin_write_transaction",
        "begin_with",
        "BEGIN IMMEDIATE",
    )?;
    let hook_variant = exact_enum_variant(
        EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
        &migrations,
        "EventStoreMigrationHook",
        "Nip09ReconciliationV1",
    )?;
    let hook_id_arm = exact_associated_match_arm(
        EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
        &migrations,
        "EventStoreMigrationHook",
        "id",
        "Nip09ReconciliationV1",
    )?;
    validate_exact_arm_expression(
        EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
        "EventStoreMigrationHook::id Nip09ReconciliationV1 arm",
        &hook_id_arm.body,
        "nip09_manifest::NIP09_RECONCILIATION_HOOK_ID",
    )?;
    let hook_manifest_arm = exact_associated_match_arm(
        EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
        &migrations,
        "EventStoreMigrationHook",
        "manifest_sha256",
        "Nip09ReconciliationV1",
    )?;
    validate_exact_arm_expression(
        EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
        "EventStoreMigrationHook::manifest_sha256 Nip09ReconciliationV1 arm",
        &hook_manifest_arm.body,
        "Some(nip09_manifest::NIP09_RECONCILIATION_MANIFEST_SHA256)",
    )?;
    let migration_v1 = exact_const_struct_array_element(
        EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
        &migrations,
        "EVENT_STORE_MIGRATIONS",
        "version",
        1,
    )?;
    let migration_v2 = exact_const_struct_array_element(
        EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
        &migrations,
        "EVENT_STORE_MIGRATIONS",
        "version",
        2,
    )?;
    let migration_validator_call = exact_direct_guarded_call(
        EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
        &migrations,
        "validate_migration_registry",
        "registry.iter().any(|migration|{migration.hook==EventStoreMigrationHook::Nip09ReconciliationV1})",
        "validate_generated_nip09_manifest_descriptor",
    )?;
    let migration_registry_loop = exact_direct_for_loop(
        EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
        &migrations,
        "validate_migration_registry",
        "(index,migration)",
        "registry.iter().enumerate()",
    )?;
    let migration_registry_function = exact_top_level_function(
        EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
        &migrations,
        "validate_migration_registry",
    )?;
    let migration_ledger_guard = migration_registry_function.block.stmts.first().ok_or_else(|| {
        format!(
            "{EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE} `validate_migration_registry` must begin with the authoritative ledger DDL guard"
        )
    })?;
    let migration_validator_arm = exact_direct_loop_match_arm(
        EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
        "validate_migration_registry",
        migration_registry_loop,
        &[
            "migration.hook",
            "migration.hook_manifest_sha256",
            "migration.event_contract_registry_version",
        ],
        "Nip09ReconciliationV1",
    )?;
    let manifest_bytes = exact_direct_local_initializer(
        EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
        &migrations,
        "validate_generated_nip09_manifest_descriptor",
        "bytes",
    )?;
    validate_exact_direct_method_call(
        EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
        "validate_generated_nip09_manifest_descriptor bytes initializer",
        manifest_bytes,
        "as_bytes",
        "nip09_manifest::NIP09_RECONCILIATION_MANIFEST_JSON",
    )?;
    let manifest_length_condition = exact_direct_fail_closed_if(
        EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
        &migrations,
        "validate_generated_nip09_manifest_descriptor",
        "bytes.len()!=nip09_manifest::NIP09_RECONCILIATION_MANIFEST_BYTE_LENGTH",
    )?;
    let manifest_hash_condition = exact_direct_fail_closed_if(
        EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
        &migrations,
        "validate_generated_nip09_manifest_descriptor",
        "sha256_hex(bytes)!=nip09_manifest::NIP09_RECONCILIATION_MANIFEST_SHA256",
    )?;
    let manifest_json_call = exact_direct_local_function_call(
        EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
        &migrations,
        "validate_generated_nip09_manifest_descriptor",
        "manifest",
        "serde_json::from_slice",
    )?;
    let expected_number_pointers = exact_direct_local_initializer(
        EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
        &migrations,
        "validate_generated_nip09_manifest_descriptor",
        "expected_numbers",
    )?;
    let expected_string_pointers = exact_direct_local_initializer(
        EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
        &migrations,
        "validate_generated_nip09_manifest_descriptor",
        "expected_strings",
    )?;
    let number_pointer_checks = exact_direct_local_initializer(
        EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
        &migrations,
        "validate_generated_nip09_manifest_descriptor",
        "numbers_match",
    )?;
    validate_manifest_pointer_check(
        EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
        "numbers_match",
        number_pointer_checks,
        "as_u64",
    )?;
    let string_pointer_checks = exact_direct_local_initializer(
        EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
        &migrations,
        "validate_generated_nip09_manifest_descriptor",
        "strings_match",
    )?;
    validate_manifest_pointer_check(
        EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
        "strings_match",
        string_pointer_checks,
        "as_str",
    )?;
    let manifest_metadata_condition = exact_direct_fail_closed_if(
        EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
        &migrations,
        "validate_generated_nip09_manifest_descriptor",
        "nip09_manifest::NIP09_RECONCILIATION_MANIFEST_SCHEMA_VERSION!=1||nip09_manifest::NIP09_RECONCILIATION_MIGRATION_VERSION!=2||nip09_manifest::NIP09_RECONCILIATION_VERSION<=0||nip09_manifest::NIP09_RECONCILIATION_ADDRESSABLE_FEED_VERSION<=0||!numbers_match||!strings_match",
    )?;
    let manifest_validator_function = exact_top_level_function(
        EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
        &migrations,
        "validate_generated_nip09_manifest_descriptor",
    )?;
    let embedded_registry_validator = exact_top_level_function(
        EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
        &migrations,
        "validate_embedded_migration_registry",
    )?;
    validate_exact_function_block(
        EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
        "validate_embedded_migration_registry",
        &embedded_registry_validator.block,
        "{validate_migration_registry(EVENT_STORE_MIGRATIONS,RADROOTS_EVENT_STORE_SCHEMA_VERSION_MIN,RADROOTS_EVENT_STORE_SCHEMA_VERSION_CURRENT,)}",
    )?;
    let apply_hook_arm = exact_tail_match_arm(
        EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
        &schema,
        "apply_migration_hook",
        "migration.hook",
        "Nip09ReconciliationV1",
    )?;
    validate_direct_arm_awaited_call(
        EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
        "apply_migration_hook Nip09ReconciliationV1 arm",
        &apply_hook_arm.body,
        "apply_reconciliation_hook",
    )?;
    let validate_hook_arm = exact_tail_match_arm(
        EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
        &schema,
        "validate_migration_hook_state",
        "migration.hook",
        "Nip09ReconciliationV1",
    )?;
    validate_direct_arm_awaited_call(
        EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
        "validate_migration_hook_state Nip09ReconciliationV1 arm",
        &validate_hook_arm.body,
        "validate_active_hook_state_fast",
    )?;
    let apply_hook_loop = exact_direct_for_loop(
        EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
        &schema,
        "migrate_schema_on_connection",
        "migration",
        "registry.iter().filter(|migration|migration.version>current_version)",
    )?;
    let apply_hook_call = exact_direct_loop_awaited_call(
        EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
        "migrate_schema_on_connection",
        apply_hook_loop,
        "apply_migration_hook",
    )?;
    let validate_hook_loop = exact_direct_for_loop(
        EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
        &schema,
        "validate_applied_migration_hooks",
        "migration",
        "registry.iter().filter(|migration|migration.version<=current)",
    )?;
    let validate_hook_call = exact_direct_loop_awaited_call(
        EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
        "validate_applied_migration_hooks",
        validate_hook_loop,
        "validate_migration_hook_state",
    )?;

    Ok(vec![
        rust_fragment_descriptor(
            "event_store_schema_inspection_entry_v1",
            EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
            "function:inspect_event_store_schema_status",
            schema_runtime[0],
        ),
        rust_fragment_descriptor(
            "event_store_schema_inspection_transaction_route_v1",
            EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
            "function:inspect_event_store_schema_status_with_registry",
            schema_runtime[1],
        ),
        rust_fragment_descriptor(
            "event_store_schema_migration_entry_v1",
            EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
            "function:migrate_event_store_schema",
            schema_runtime[2],
        ),
        rust_fragment_descriptor(
            "event_store_schema_generation_provider_route_v1",
            EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
            "function:migrate_event_store_schema_with_generation_provider",
            schema_runtime[3],
        ),
        rust_fragment_descriptor(
            "event_store_schema_manifest_guard_route_v1",
            EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
            "function:migrate_event_store_schema_with_generation_provider_and_limits_inner",
            schema_runtime[4],
        ),
        rust_fragment_descriptor(
            "event_store_schema_transaction_orchestration_v1",
            EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
            "function:migrate_event_store_schema_with_registry_and_generation_provider",
            schema_runtime[5],
        ),
        rust_fragment_descriptor(
            "event_store_temp_schema_guard_entry_v1",
            EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
            "function:validate_event_store_temp_schema",
            schema_runtime[6],
        ),
        rust_fragment_descriptor(
            "event_store_temp_schema_collision_validation_v1",
            EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
            "function:validate_event_store_temp_schema_with_registry",
            schema_runtime[7],
        ),
        rust_fragment_descriptor(
            "event_store_main_catalog_internal_filter_v1",
            EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
            "function:read_catalog",
            schema_runtime[8],
        ),
        rust_fragment_descriptor(
            "event_store_schema_migration_application_authority_v1",
            EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
            "function_set:finish_schema_transaction+preserve_primary_failure+rollback_event_store_schema_offline+rollback_event_store_schema_with_registry+migrate_schema_on_connection+rollback_schema_on_connection+apply_migration_up+apply_migration_down+validate_catalog_delta+create_ledger+insert_ledger_row+inspect_schema_on_connection+apply_migration_hook+validate_migration_hook_state+validate_applied_migration_hooks+read_catalog+validate_ledger_catalog+governed_catalog+catalog_fingerprint+read_history+validate_history_against_registry+validate_history_checksum+validate_schema_fingerprint+validate_database_integrity",
            &schema_migration_application_fragment,
        ),
        rust_fragment_descriptor(
            "event_store_schema_production_ast_authority_v1",
            EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
            "production_file_ast",
            &schema,
        ),
        rust_fragment_descriptor(
            "event_store_schema_name_matching_authority_v1",
            EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
            "function_set:is_event_store_owned_table_name+is_event_store_governed_schema_name+sqlite_identifier_starts_with",
            &schema_name_matcher_fragment,
        ),
        rust_fragment_descriptor(
            "event_store_migration_contract_shape_v1",
            EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
            "const_set:EVENT_STORE_LEDGER_NAME+EVENT_STORE_RESERVED_PREFIX+RADROOTS_EVENT_STORE_SCHEMA_VERSION_MIN;struct:EventStoreMigration",
            &migration_contract_fragment,
        ),
        rust_fragment_descriptor(
            "event_store_migration_catalog_authority_v1",
            EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
            "const_set:EVENT_STORE_LEDGER_DDL+EVENT_STORE_LEDGER_CREATE_DDL+EVENT_STORE_BASELINE_OBJECT_NAMES+EVENT_STORE_BASELINE_TABLE_NAMES+EVENT_STORE_BASELINE_FTS5_TABLE_NAMES+EVENT_STORE_NIP09_OBJECT_NAMES+EVENT_STORE_NIP09_TABLE_NAMES",
            &migration_catalog_fragment,
        ),
        rust_fragment_descriptor(
            "event_store_migration_runtime_helpers_v1",
            EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
            "function_set:migration_for_version+validate_owned_schema_name+validate_embedded_migration_input+validate_sha256_literal+sha256_hex",
            &migration_runtime_fragment,
        ),
        rust_fragment_descriptor(
            "event_store_begin_immediate_authority_v1",
            store_relative,
            "method_call:RadrootsEventStore::begin_write_transaction::begin_with(\"BEGIN IMMEDIATE\")",
            begin_immediate,
        ),
        rust_fragment_descriptor(
            "event_store_nip09_hook_variant_v1",
            EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
            "enum:EventStoreMigrationHook::Nip09ReconciliationV1",
            hook_variant,
        ),
        rust_fragment_descriptor(
            "event_store_nip09_hook_id_arm_v1",
            EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
            "match_arm:EventStoreMigrationHook::id::Nip09ReconciliationV1",
            hook_id_arm,
        ),
        rust_fragment_descriptor(
            "event_store_nip09_hook_manifest_arm_v1",
            EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
            "match_arm:EventStoreMigrationHook::manifest_sha256::Nip09ReconciliationV1",
            hook_manifest_arm,
        ),
        rust_fragment_descriptor(
            "event_store_baseline_migration_registry_entry_v1",
            EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
            "const_element:EVENT_STORE_MIGRATIONS[version=1]",
            migration_v1,
        ),
        rust_fragment_descriptor(
            "event_store_nip09_migration_registry_entry_v2",
            EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
            "const_element:EVENT_STORE_MIGRATIONS[version=2]",
            migration_v2,
        ),
        rust_fragment_descriptor(
            "event_store_nip09_manifest_validation_route_v1",
            EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
            "direct_guarded_call:validate_migration_registry::Nip09ReconciliationV1::validate_generated_nip09_manifest_descriptor",
            migration_validator_call,
        ),
        rust_fragment_descriptor(
            "event_store_nip09_registry_validation_arm_v1",
            EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
            "match_arm:validate_migration_registry::Nip09ReconciliationV1",
            migration_validator_arm,
        ),
        rust_fragment_descriptor(
            "event_store_migration_registry_validation_body_v1",
            EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
            "loop_body:validate_migration_registry::registry.iter().enumerate()",
            &migration_registry_loop.body,
        ),
        rust_fragment_descriptor(
            "event_store_migration_registry_validation_function_v1",
            EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
            "function:validate_migration_registry",
            migration_registry_function,
        ),
        rust_fragment_descriptor(
            "event_store_migration_ledger_guard_v1",
            EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
            "statement:validate_migration_registry::ledger_ddl_equivalence",
            migration_ledger_guard,
        ),
        rust_fragment_descriptor(
            "event_store_nip09_registry_validation_loop_v1",
            EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
            "direct_for_iterator:validate_migration_registry::registry.iter().enumerate()",
            &migration_registry_loop.expr,
        ),
        rust_fragment_descriptor(
            "event_store_nip09_manifest_bytes_authority_v1",
            EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
            "local_initializer:validate_generated_nip09_manifest_descriptor::bytes",
            manifest_bytes,
        ),
        rust_fragment_descriptor(
            "event_store_nip09_manifest_length_check_v1",
            EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
            "fail_closed_if:validate_generated_nip09_manifest_descriptor::manifest_byte_length",
            manifest_length_condition,
        ),
        rust_fragment_descriptor(
            "event_store_nip09_manifest_self_hash_check_v1",
            EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
            "fail_closed_if:validate_generated_nip09_manifest_descriptor::manifest_sha256",
            manifest_hash_condition,
        ),
        rust_fragment_descriptor(
            "event_store_nip09_manifest_json_parse_v1",
            EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
            "direct_local_call:validate_generated_nip09_manifest_descriptor::manifest::serde_json::from_slice",
            manifest_json_call,
        ),
        rust_fragment_descriptor(
            "event_store_nip09_manifest_number_pointer_table_v1",
            EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
            "local_initializer:validate_generated_nip09_manifest_descriptor::expected_numbers",
            expected_number_pointers,
        ),
        rust_fragment_descriptor(
            "event_store_nip09_manifest_string_pointer_table_v1",
            EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
            "local_initializer:validate_generated_nip09_manifest_descriptor::expected_strings",
            expected_string_pointers,
        ),
        rust_fragment_descriptor(
            "event_store_nip09_manifest_number_pointer_check_v1",
            EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
            "local_initializer:validate_generated_nip09_manifest_descriptor::numbers_match",
            number_pointer_checks,
        ),
        rust_fragment_descriptor(
            "event_store_nip09_manifest_string_pointer_check_v1",
            EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
            "local_initializer:validate_generated_nip09_manifest_descriptor::strings_match",
            string_pointer_checks,
        ),
        rust_fragment_descriptor(
            "event_store_nip09_manifest_metadata_guard_v1",
            EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
            "fail_closed_if:validate_generated_nip09_manifest_descriptor::pointer_matches",
            manifest_metadata_condition,
        ),
        rust_fragment_descriptor(
            "event_store_nip09_manifest_validator_function_v1",
            EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
            "function:validate_generated_nip09_manifest_descriptor",
            manifest_validator_function,
        ),
        rust_fragment_descriptor(
            "event_store_embedded_migration_registry_route_v1",
            EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
            "function_block:validate_embedded_migration_registry",
            &embedded_registry_validator.block,
        ),
        rust_fragment_descriptor(
            "event_store_nip09_apply_hook_arm_v1",
            EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
            "match_arm:apply_migration_hook::Nip09ReconciliationV1",
            apply_hook_arm,
        ),
        rust_fragment_descriptor(
            "event_store_nip09_validate_hook_arm_v1",
            EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
            "match_arm:validate_migration_hook_state::Nip09ReconciliationV1",
            validate_hook_arm,
        ),
        rust_fragment_descriptor(
            "event_store_nip09_apply_hook_call_v1",
            EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
            "awaited_call:migrate_schema_on_connection::apply_migration_hook",
            apply_hook_call,
        ),
        rust_fragment_descriptor(
            "event_store_nip09_apply_hook_loop_v1",
            EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
            "direct_for_iterator:migrate_schema_on_connection::pending_migrations",
            &apply_hook_loop.expr,
        ),
        rust_fragment_descriptor(
            "event_store_nip09_validate_hook_call_v1",
            EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
            "awaited_call:validate_applied_migration_hooks::validate_migration_hook_state",
            validate_hook_call,
        ),
        rust_fragment_descriptor(
            "event_store_nip09_validate_hook_loop_v1",
            EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
            "direct_for_iterator:validate_applied_migration_hooks::applied_migrations",
            &validate_hook_loop.expr,
        ),
    ])
}

fn rust_fragment_descriptor(
    role: &str,
    path: &str,
    selector: &str,
    fragment: &impl ToTokens,
) -> RustFragmentWitnessDescriptor {
    RustFragmentWitnessDescriptor {
        role: role.to_owned(),
        path: path.to_owned(),
        selector: selector.to_owned(),
        ast_sha256: sha256_hex(fragment.to_token_stream().to_string().as_bytes()),
    }
}

fn exact_associated_method_call_with_string<'a>(
    relative: &str,
    file: &'a syn::File,
    owner: &str,
    function: &str,
    method: &str,
    argument: &str,
) -> Result<&'a syn::ExprMethodCall, String> {
    let function_item = exact_associated_function(relative, file, owner, function)?;
    let Some(syn::Stmt::Expr(tail, None)) = function_item.block.stmts.last() else {
        return Err(format!(
            "{relative} `{owner}::{function}` must end in the authoritative `{method}` expression"
        ));
    };
    let Some(syn::Expr::MethodCall(call)) = direct_terminal_expression(tail) else {
        return Err(format!(
            "{relative} `{owner}::{function}` must directly return `{method}(\"{argument}\")`"
        ));
    };
    let matches_argument = call.args.len() == 1
        && matches!(
            call.args.first(),
            Some(syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(value),
                ..
            })) if value.value() == argument
        );
    if call.method != method || compact_tokens(&call.receiver) != "self.pool" || !matches_argument {
        return Err(format!(
            "{relative} `{owner}::{function}` must directly return `{method}(\"{argument}\")`"
        ));
    }
    Ok(call)
}

fn exact_direct_guarded_call<'a>(
    relative: &str,
    file: &'a syn::File,
    function: &str,
    expected_condition: &str,
    called: &str,
) -> Result<&'a syn::ExprIf, String> {
    let function_item = exact_top_level_function(relative, file, function)?;
    let matches = function_item
        .block
        .stmts
        .iter()
        .filter_map(|statement| match statement {
            syn::Stmt::Expr(syn::Expr::If(expression), _)
                if compact_tokens(&expression.cond)
                    == compact_source_tokens(expected_condition) =>
            {
                Some(expression)
            }
            _ => None,
        })
        .filter(|guard| {
            let [statement] = guard.then_branch.stmts.as_slice() else {
                return false;
            };
            guard.else_branch.is_none()
                && direct_statement_expression(statement)
                    .and_then(|expression| direct_try_function_call(expression, called))
                    .is_some()
        })
        .collect::<Vec<_>>();
    let [guard] = matches.as_slice() else {
        let actual_conditions = function_item
            .block
            .stmts
            .iter()
            .filter_map(|statement| match statement {
                syn::Stmt::Expr(syn::Expr::If(expression), _) => {
                    Some(compact_tokens(&expression.cond))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        return Err(format!(
            "{relative} `{function}` must contain exactly one direct `{expected_condition}` guard whose sole body statement calls `{called}`; found {}, actual direct conditions {actual_conditions:?}",
            matches.len(),
        ));
    };
    Ok(guard)
}

fn exact_direct_local_initializer<'a>(
    relative: &str,
    file: &'a syn::File,
    function: &str,
    binding: &str,
) -> Result<&'a syn::Expr, String> {
    let function_item = exact_top_level_function(relative, file, function)?;
    let matches = function_item
        .block
        .stmts
        .iter()
        .filter_map(|statement| match statement {
            syn::Stmt::Local(local)
                if local_pattern_ident(&local.pat).as_deref() == Some(binding) =>
            {
                local
                    .init
                    .as_ref()
                    .map(|initializer| initializer.expr.as_ref())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let [initializer] = matches.as_slice() else {
        return Err(format!(
            "{relative} `{function}` must contain exactly one direct `{binding}` initializer; found {}",
            matches.len()
        ));
    };
    Ok(initializer)
}

fn exact_direct_local_function_call<'a>(
    relative: &str,
    file: &'a syn::File,
    function: &str,
    binding: &str,
    called: &str,
) -> Result<&'a syn::ExprCall, String> {
    let initializer = exact_direct_local_initializer(relative, file, function, binding)?;
    direct_call_chain_function_call(initializer, called).ok_or_else(|| {
        format!(
            "{relative} `{function}` direct `{binding}` initializer must call `{called}` in its authoritative receiver chain"
        )
    })
}

fn exact_direct_fail_closed_if<'a>(
    relative: &str,
    file: &'a syn::File,
    function: &str,
    expected_condition: &str,
) -> Result<&'a syn::ExprIf, String> {
    let function_item = exact_top_level_function(relative, file, function)?;
    let matches = function_item
        .block
        .stmts
        .iter()
        .filter_map(|statement| match statement {
            syn::Stmt::Expr(syn::Expr::If(expression), _)
                if compact_tokens(&expression.cond)
                    == compact_source_tokens(expected_condition) =>
            {
                Some(expression)
            }
            _ => None,
        })
        .filter(|expression| {
            let [statement] = expression.then_branch.stmts.as_slice() else {
                return false;
            };
            let syn::Stmt::Expr(syn::Expr::Return(return_expression), _) = statement else {
                return false;
            };
            expression.else_branch.is_none()
                && return_expression
                    .expr
                    .as_deref()
                    .and_then(|expression| direct_function_call(expression, "Err"))
                    .is_some()
        })
        .collect::<Vec<_>>();
    let [expression] = matches.as_slice() else {
        return Err(format!(
            "{relative} `{function}` must contain exactly one fail-closed direct if for `{expected_condition}`; found {}",
            matches.len()
        ));
    };
    Ok(expression)
}

fn validate_exact_arm_expression(
    relative: &str,
    fragment: &str,
    expression: &syn::Expr,
    expected: &str,
) -> Result<(), String> {
    let expression = direct_arm_tail_expression(expression)
        .ok_or_else(|| format!("{relative} {fragment} must directly return `{expected}`"))?;
    if compact_tokens(expression) != compact_source_tokens(expected) {
        return Err(format!(
            "{relative} {fragment} must directly return `{expected}`"
        ));
    }
    Ok(())
}

fn validate_exact_function_block(
    relative: &str,
    function: &str,
    block: &syn::Block,
    expected: &str,
) -> Result<(), String> {
    let actual = compact_tokens(block);
    let expected = compact_source_tokens(expected);
    if actual != expected {
        return Err(format!(
            "{relative} `{function}` authoritative function body drifted: expected `{expected}`, found `{actual}`"
        ));
    }
    Ok(())
}

fn validate_migration_registry_reachability(
    relative: &str,
    file: &syn::File,
) -> Result<(), String> {
    let function = exact_top_level_function(relative, file, "validate_migration_registry")?;
    let statements = &function.block.stmts;
    let predecessor_manifest_guard = "if registry.iter().any(|migration|{migration.hook==EventStoreMigrationHook::Nip09ReconciliationV1}){validate_generated_nip09_manifest_descriptor()?;}";
    let food_manifest_guard = "if registry.iter().any(|migration|{migration.hook==EventStoreMigrationHook::FoodAvailabilityProjectionV1}){validate_generated_food_availability_projection_manifest_descriptor()?;}";
    let source_maintenance_manifest_guard = "if registry.iter().any(|migration|migration.hook==EventStoreMigrationHook::SourceMaintenanceV1){validate_generated_source_maintenance_manifest_descriptor()?;}";
    let range_guard = "if minimum==0||current<minimum||registry.is_empty(){return Err(RadrootsEventStoreError::MigrationRegistryDefect{reason:format!(\"migration version range {minimum}..={current} requires a non-empty positive registry\"),});}";
    let valid = statements.len() == 11
        && statements.first().is_some_and(|statement| {
            compact_tokens(statement) == compact_source_tokens(predecessor_manifest_guard)
        })
        && statements.get(1).is_some_and(|statement| {
            compact_tokens(statement) == compact_source_tokens(food_manifest_guard)
        })
        && statements.get(2).is_some_and(|statement| {
            compact_tokens(statement) == compact_source_tokens(source_maintenance_manifest_guard)
        })
        && statements.get(3).is_some_and(|statement| {
            compact_tokens(statement) == compact_source_tokens(range_guard)
        })
        && matches!(
            statements.get(4),
            Some(syn::Stmt::Local(local))
                if local_pattern_ident(&local.pat).as_deref() == Some("expected_version")
        )
        && matches!(
            statements.get(5),
            Some(syn::Stmt::Local(local))
                if local_pattern_ident(&local.pat).as_deref() == Some("owned_object_names")
        )
        && matches!(
            statements.get(6),
            Some(syn::Stmt::Local(local))
                if local_pattern_ident(&local.pat).as_deref() == Some("owned_table_names")
        )
        && matches!(
            statements.get(7),
            Some(syn::Stmt::Local(local))
                if local_pattern_ident(&local.pat).as_deref() == Some("migration_hook_ids")
        )
        && matches!(
            statements.get(8),
            Some(syn::Stmt::Expr(syn::Expr::ForLoop(_), _))
        )
        && matches!(
            statements.get(9),
            Some(syn::Stmt::Expr(syn::Expr::If(_), _))
        )
        && statements
            .get(10)
            .and_then(direct_statement_expression)
            .is_some_and(|expression| compact_tokens(expression) == "Ok(())");
    if !valid {
        return Err(format!(
            "{relative} `validate_migration_registry` authoritative top-level statement skeleton drifted: found {:?}",
            statements.iter().map(compact_tokens).collect::<Vec<_>>()
        ));
    }
    Ok(())
}

fn validate_manifest_validator_reachability(
    relative: &str,
    file: &syn::File,
) -> Result<(), String> {
    let function = exact_top_level_function(
        relative,
        file,
        "validate_generated_nip09_manifest_descriptor",
    )?;
    let statements = &function.block.stmts;
    let expected_locals = [
        (0, "bytes"),
        (1, "manifest"),
        (2, "up_byte_length"),
        (3, "down_byte_length"),
        (4, "reconciliation_version"),
        (5, "addressable_feed_version"),
    ];
    let valid = statements.len() == 9
        && expected_locals.iter().all(|(index, name)| {
            matches!(
                statements.get(*index),
                Some(syn::Stmt::Local(local))
                    if local_pattern_ident(&local.pat).as_deref() == Some(*name)
            )
        })
        && statements
            .get(1)
            .and_then(direct_statement_expression)
            .and_then(|expression| {
                direct_try_function_call(expression, "validate_generated_manifest_envelope")
            })
            .is_some()
        && [2, 3].iter().all(|index| {
            statements
                .get(*index)
                .and_then(direct_statement_expression)
                .and_then(|expression| {
                    direct_try_function_call(expression, "generated_manifest_u128_to_u64")
                })
                .is_some()
        })
        && [4, 5].iter().all(|index| {
            statements
                .get(*index)
                .and_then(direct_statement_expression)
                .and_then(|expression| {
                    direct_try_function_call(expression, "generated_manifest_i64_to_u64")
                })
                .is_some()
        })
        && statements
            .get(6)
            .and_then(direct_statement_expression)
            .and_then(|expression| {
                direct_try_function_call(expression, "validate_generated_manifest_metadata")
            })
            .is_some()
        && matches!(
            statements.get(7),
            Some(syn::Stmt::Expr(syn::Expr::ForLoop(_), _))
        )
        && statements
            .get(8)
            .and_then(direct_statement_expression)
            .is_some_and(|expression| compact_tokens(expression) == "Ok(())");
    if !valid {
        return Err(format!(
            "{relative} generated-manifest validator authoritative top-level statement skeleton drifted: found {:?}",
            statements.iter().map(compact_tokens).collect::<Vec<_>>()
        ));
    }
    let digest_loop = match statements.get(7) {
        Some(syn::Stmt::Expr(syn::Expr::ForLoop(expression), _)) => expression,
        _ => unreachable!("validated descriptor loop"),
    };
    if digest_loop.body.stmts.len() != 1
        || digest_loop
            .body
            .stmts
            .first()
            .and_then(direct_statement_expression)
            .and_then(|expression| direct_try_function_call(expression, "validate_sha256_literal"))
            .is_none()
    {
        return Err(format!(
            "{relative} generated-manifest descriptor digest loop must directly propagate validate_sha256_literal"
        ));
    }
    Ok(())
}

fn validate_source_maintenance_manifest_validator_reachability(
    relative: &str,
    file: &syn::File,
) -> Result<(), String> {
    let function = exact_top_level_function(
        relative,
        file,
        "validate_generated_source_maintenance_manifest_descriptor",
    )?;
    let statements = &function.block.stmts;
    let expected_locals = [(1, "bytes"), (2, "manifest")];
    let valid = statements.len() == 6
        && matches!(statements.first(), Some(syn::Stmt::Item(syn::Item::Use(_))))
        && expected_locals.iter().all(|(index, name)| {
            matches!(
                statements.get(*index),
                Some(syn::Stmt::Local(local))
                    if local_pattern_ident(&local.pat).as_deref() == Some(*name)
            )
        })
        && statements
            .get(2)
            .and_then(direct_statement_expression)
            .and_then(|expression| {
                direct_try_function_call(expression, "validate_generated_manifest_envelope")
            })
            .is_some()
        && statements
            .get(3)
            .and_then(direct_statement_expression)
            .and_then(|expression| {
                direct_try_function_call(expression, "validate_generated_manifest_metadata")
            })
            .is_some()
        && matches!(
            statements.get(4),
            Some(syn::Stmt::Expr(syn::Expr::ForLoop(_), _))
        )
        && statements
            .get(5)
            .and_then(direct_statement_expression)
            .is_some_and(|expression| compact_tokens(expression) == "Ok(())");
    if !valid {
        return Err(format!(
            "{relative} generated SourceMaintenance manifest validator authoritative statement skeleton drifted"
        ));
    }
    let digest_loop = match statements.get(4) {
        Some(syn::Stmt::Expr(syn::Expr::ForLoop(expression), _)) => expression,
        _ => unreachable!("validated descriptor loop"),
    };
    if digest_loop.body.stmts.len() != 1
        || digest_loop
            .body
            .stmts
            .first()
            .and_then(direct_statement_expression)
            .and_then(|expression| direct_try_function_call(expression, "validate_sha256_literal"))
            .is_none()
    {
        return Err(format!(
            "{relative} SourceMaintenance descriptor digest loop must directly propagate validate_sha256_literal"
        ));
    }
    Ok(())
}

fn validate_source_maintenance_migration_bindings(
    relative: &str,
    file: &syn::File,
) -> Result<(), String> {
    let entry = exact_const_struct_array_element(
        relative,
        file,
        "EVENT_STORE_MIGRATIONS",
        "version",
        u64::from(SOURCE_MAINTENANCE_MIGRATION_VERSION),
    )?;
    let expected_entry = r#"EventStoreMigration {
        version: 4,
        name: "source_maintenance",
        up_sql: include_str!("../migrations/0004_source_maintenance.up.sql"),
        down_sql: include_str!("../migrations/0004_source_maintenance.down.sql"),
        up_len: source_maintenance_manifest::SOURCE_MAINTENANCE_MIGRATION_UP_BYTE_LENGTH,
        down_len: source_maintenance_manifest::SOURCE_MAINTENANCE_MIGRATION_DOWN_BYTE_LENGTH,
        up_sha256: source_maintenance_manifest::SOURCE_MAINTENANCE_MIGRATION_UP_SHA256,
        down_sha256: source_maintenance_manifest::SOURCE_MAINTENANCE_MIGRATION_DOWN_SHA256,
        schema_sha256: source_maintenance_manifest::SOURCE_MAINTENANCE_SCHEMA_SHA256,
        owned_object_names: EVENT_STORE_SOURCE_MAINTENANCE_OBJECT_NAMES,
        replaced_object_names: EVENT_STORE_SOURCE_MAINTENANCE_REPLACED_OBJECT_NAMES,
        owned_table_names: EVENT_STORE_SOURCE_MAINTENANCE_TABLE_NAMES,
        fts5_table_names: &[],
        hook: EventStoreMigrationHook::SourceMaintenanceV1,
        hook_manifest_sha256: Some(source_maintenance_manifest::SOURCE_MAINTENANCE_MANIFEST_SHA256,),
        event_contract_registry_version: Some(
            source_maintenance_manifest::SOURCE_MAINTENANCE_EVENT_CONTRACT_REGISTRY_VERSION,
        ),
    }"#;
    let actual_entry = compact_tokens(entry);
    let expected_entry = compact_source_tokens(expected_entry);
    if actual_entry != expected_entry {
        return Err(format!(
            "{relative} SourceMaintenance v4 migration entry authority drifted: expected `{expected_entry}`, found `{actual_entry}`"
        ));
    }

    let loop_expression = exact_direct_for_loop(
        relative,
        file,
        "validate_migration_registry",
        "(index,migration)",
        "registry.iter().enumerate()",
    )?;
    let arm = exact_direct_loop_match_arm(
        relative,
        "validate_migration_registry",
        loop_expression,
        &[
            "migration.hook",
            "migration.hook_manifest_sha256",
            "migration.event_contract_registry_version",
        ],
        "SourceMaintenanceV1",
    )?;
    let expected_pattern = r#"(EventStoreMigrationHook::None, None, None)
        | (
            EventStoreMigrationHook::Nip09ReconciliationV1,
            Some(nip09_manifest::NIP09_RECONCILIATION_MANIFEST_SHA256),
            Some(nip09_manifest::NIP09_RECONCILIATION_EVENT_CONTRACT_REGISTRY_VERSION,),
        )
        | (
            EventStoreMigrationHook::FoodAvailabilityProjectionV1,
            Some(food_manifest::FOOD_AVAILABILITY_PROJECTION_MANIFEST_SHA256),
            Some(food_manifest::FOOD_AVAILABILITY_PROJECTION_EVENT_CONTRACT_REGISTRY_VERSION,),
        )
        | (
            EventStoreMigrationHook::SourceMaintenanceV1,
            Some(source_maintenance_manifest::SOURCE_MAINTENANCE_MANIFEST_SHA256),
            Some(source_maintenance_manifest::SOURCE_MAINTENANCE_EVENT_CONTRACT_REGISTRY_VERSION,),
        )"#;
    if compact_tokens(&arm.pat) != compact_source_tokens(expected_pattern)
        || arm.guard.is_some()
        || compact_tokens(&arm.body) != "{}"
    {
        return Err(format!(
            "{relative} SourceMaintenance registry tuple authority drifted: expected pattern `{}`, found pattern `{}`, guard {:?}, body `{}`",
            compact_source_tokens(expected_pattern),
            compact_tokens(&arm.pat),
            arm.guard
                .as_ref()
                .map(|(_, expression)| compact_tokens(expression)),
            compact_tokens(&arm.body),
        ));
    }
    Ok(())
}

fn validate_event_store_migration_support_authority(
    relative: &str,
    file: &syn::File,
) -> Result<(), String> {
    const EXPECTED: [(&str, &str); 10] = [
        (
            "event_store_ledger_ddl",
            "acb34d80155e42af0c3c53389963d7af0fb0810572fcc368228646943446171f",
        ),
        (
            "EVENT_STORE_LEDGER_DDL",
            "1e9616f21a61e3194fe087a08cb5c86d2cef99701e4d9c6ff2d838c0e6f52332",
        ),
        (
            "EVENT_STORE_LEDGER_CREATE_DDL",
            "86bc9677ae9a20a2bf0a14eab18e5dfd1e0c63f2712d764fc8c9ed73169c9fbd",
        ),
        (
            "EVENT_STORE_BASELINE_FTS5_TABLE_NAMES",
            "4ab01dfd843eb33e82fae3d9503000f9c0292ce4e6f61f18aeee33ebc15360d3",
        ),
        (
            "EventStoreMigration",
            "3552624482aa3c698ebcc88e5d3497e35d30d3646ea0605d2c192acc21006ee2",
        ),
        (
            "EVENT_STORE_MIGRATIONS[version=1]",
            "0f763874f3fb73f2a41701ec623bae3629464243d70de797d842cdde45ab847e",
        ),
        (
            "migration_for_version",
            "896f4fd8a67ba6dc17262f117fd74b0df74ee75f3cf0066bfddd90fb58d84a96",
        ),
        (
            "validate_embedded_migration_input",
            "526a7971d0736588d66cf95cca4063ebd3a4497c6f0c2c7ba96b39d09ee07259",
        ),
        (
            "validate_migration_registry",
            "1a4181d0a7b4792bd3f95555acfc31081dae5f7ccfaf6f05271bb08a81624080",
        ),
        (
            "validate_generated_nip09_manifest_descriptor",
            "c1f13e40a555ae73bb9ceb788af746baf0086919ecba70f01b014856c49a2e1b",
        ),
    ];

    let ledger_macros = file
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Macro(item)
                if item
                    .ident
                    .as_ref()
                    .is_some_and(|ident| ident == "event_store_ledger_ddl")
                    && item.mac.path.is_ident("macro_rules") =>
            {
                Some(item)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let [ledger_macro] = ledger_macros.as_slice() else {
        return Err(format!(
            "{relative} must contain exactly one `event_store_ledger_ddl` macro definition"
        ));
    };
    let tokens = [
        compact_tokens(*ledger_macro),
        compact_tokens(exact_executor_const(
            file,
            relative,
            "EVENT_STORE_LEDGER_DDL",
        )?),
        compact_tokens(exact_executor_const(
            file,
            relative,
            "EVENT_STORE_LEDGER_CREATE_DDL",
        )?),
        compact_tokens(exact_executor_const(
            file,
            relative,
            "EVENT_STORE_BASELINE_FTS5_TABLE_NAMES",
        )?),
        compact_tokens(exact_top_level_struct(
            relative,
            file,
            "EventStoreMigration",
        )?),
        compact_tokens(exact_const_struct_array_element(
            relative,
            file,
            "EVENT_STORE_MIGRATIONS",
            "version",
            1,
        )?),
        compact_tokens(exact_top_level_function(
            relative,
            file,
            "migration_for_version",
        )?),
        compact_tokens(exact_top_level_function(
            relative,
            file,
            "validate_embedded_migration_input",
        )?),
        compact_tokens(exact_top_level_function(
            relative,
            file,
            "validate_migration_registry",
        )?),
        compact_tokens(exact_top_level_function(
            relative,
            file,
            "validate_generated_nip09_manifest_descriptor",
        )?),
    ];
    let drift = EXPECTED
        .iter()
        .zip(tokens)
        .filter_map(|((label, expected), tokens)| {
            let actual = sha256_hex(tokens.as_bytes());
            (actual != *expected).then(|| format!("{label}={actual} (expected {expected})"))
        })
        .collect::<Vec<_>>();
    if !drift.is_empty() {
        return Err(format!(
            "{relative} active migration support token authority drifted: {}",
            drift.join(", ")
        ));
    }
    Ok(())
}

fn validate_source_maintenance_schema_dispatch(
    relative: &str,
    file: &syn::File,
) -> Result<(), String> {
    for (function, called) in [
        ("apply_migration_hook", "apply_source_maintenance_hook_v1"),
        (
            "validate_migration_hook_state",
            "validate_source_capacity_authority_full_v1",
        ),
    ] {
        let arm = exact_tail_match_arm(
            relative,
            file,
            function,
            "migration.hook",
            "SourceMaintenanceV1",
        )?;
        validate_direct_arm_awaited_call(
            relative,
            &format!("`{function}` SourceMaintenanceV1 arm"),
            &arm.body,
            called,
        )?;
        if arm.guard.is_some() {
            return Err(format!(
                "{relative} `{function}` SourceMaintenanceV1 arm must remain unguarded"
            ));
        }
    }
    Ok(())
}

fn validate_schema_migration_execution_authority(
    relative: &str,
    file: &syn::File,
) -> Result<(), String> {
    for (name, expected) in [
        (
            "apply_migration_up",
            r#"async fn apply_migration_up(
                connection: &mut SqliteConnection,
                registry: &[EventStoreMigration],
                migration: &EventStoreMigration,
            ) -> Result<(), RadrootsEventStoreError> {
                let before = read_catalog(connection).await?;
                sqlx::raw_sql(migration.up_sql)
                    .execute(&mut *connection)
                    .await?;
                let after = read_catalog(connection).await?;
                validate_catalog_delta(&before, &after, migration, "up")?;
                validate_schema_fingerprint(connection, registry, migration).await
            }"#,
        ),
        (
            "apply_migration_down",
            r#"async fn apply_migration_down(
                connection: &mut SqliteConnection,
                migration: &EventStoreMigration,
            ) -> Result<(), RadrootsEventStoreError> {
                let before = read_catalog(connection).await?;
                sqlx::raw_sql(migration.down_sql)
                    .execute(&mut *connection)
                    .await?;
                let after = read_catalog(connection).await?;
                validate_catalog_delta(&before, &after, migration, "down")
            }"#,
        ),
        (
            "validate_catalog_delta",
            r#"fn validate_catalog_delta(
                before: &[CatalogRow],
                after: &[CatalogRow],
                migration: &EventStoreMigration,
                direction: &'static str,
            ) -> Result<(), RadrootsEventStoreError> {
                let before = before
                    .iter()
                    .map(|row| (row.name.as_str(), row))
                    .collect::<BTreeMap<_, _>>();
                let after = after
                    .iter()
                    .map(|row| (row.name.as_str(), row))
                    .collect::<BTreeMap<_, _>>();
                let added = after
                    .keys()
                    .filter(|name| !before.contains_key(**name))
                    .copied()
                    .collect::<BTreeSet<_>>();
                let removed = before
                    .keys()
                    .filter(|name| !after.contains_key(**name))
                    .copied()
                    .collect::<BTreeSet<_>>();
                let changed = before
                    .iter()
                    .filter_map(|(name, row)| {
                        after
                            .get(name)
                            .filter(|after_row| *after_row != row)
                            .map(|_| *name)
                    })
                    .collect::<BTreeSet<_>>();
                let expected = migration
                    .owned_object_names
                    .iter()
                    .copied()
                    .collect::<BTreeSet<_>>();
                let expected_changed = migration
                    .replaced_object_names
                    .iter()
                    .copied()
                    .collect::<BTreeSet<_>>();

                let valid = match direction {
                    "up" => added == expected && removed.is_empty() && changed == expected_changed,
                    "down" => removed == expected && added.is_empty() && changed == expected_changed,
                    _ => false,
                };
                if !valid {
                    return Err(RadrootsEventStoreError::MigrationCatalogDeltaMismatch {
                        version: migration.version,
                        direction,
                        reason: format!(
                            "expected {} objects {expected:?} and changed replacement objects {expected_changed:?}; added {added:?}, removed {removed:?}, changed {changed:?}",
                            if direction == "up" {
                                "added"
                            } else {
                                "removed"
                            }
                        ),
                    });
                }
                Ok(())
            }"#,
        ),
        (
            "apply_migration_hook",
            r#"async fn apply_migration_hook(
                connection: &mut SqliteConnection,
                migration: &EventStoreMigration,
                generation_provider: &dyn SourceGenerationProvider,
                reconciliation_limits: ReconciliationCapacityLimits,
            ) -> Result<(), RadrootsEventStoreError> {
                match migration.hook {
                    EventStoreMigrationHook::None => Ok(()),
                    EventStoreMigrationHook::Nip09ReconciliationV1 => {
                        apply_reconciliation_hook(
                            connection,
                            generation_provider,
                            reconciliation_limits,
                        ).await
                    }
                    EventStoreMigrationHook::FoodAvailabilityProjectionV1 => {
                        apply_food_availability_projection_hook_v1(connection).await
                    }
                    EventStoreMigrationHook::SourceMaintenanceV1 => {
                        apply_source_maintenance_hook_v1(connection).await
                    }
                }
            }"#,
        ),
        (
            "validate_migration_hook_state",
            r#"async fn validate_migration_hook_state(
                connection: &mut SqliteConnection,
                migration: &EventStoreMigration,
            ) -> Result<(), RadrootsEventStoreError> {
                match migration.hook {
                    EventStoreMigrationHook::None => Ok(()),
                    EventStoreMigrationHook::Nip09ReconciliationV1 => {
                        validate_active_hook_state_fast(connection).await
                    }
                    EventStoreMigrationHook::FoodAvailabilityProjectionV1 => {
                        validate_food_availability_projection_hook_state_fast_v1(connection).await
                    }
                    EventStoreMigrationHook::SourceMaintenanceV1 => {
                        validate_source_capacity_authority_full_v1(connection).await
                    }
                }
            }"#,
        ),
    ] {
        let actual = exact_top_level_function(relative, file, name)?;
        let expected_file = parse_canonical_production_rust(
            &format!("authoritative {relative}:{name}"),
            expected.as_bytes(),
        )?;
        let expected = exact_top_level_function(relative, &expected_file, name)?;
        if compact_tokens(actual) != compact_tokens(expected) {
            return Err(format!(
                "{relative} authoritative schema migration execution function `{name}` signature or control flow drifted"
            ));
        }
    }
    Ok(())
}

fn validate_sqlite_encoding_preflight_authority(
    relative: &str,
    file: &syn::File,
) -> Result<(), String> {
    let configure_pool = exact_top_level_function(relative, file, "configure_pool")?;
    let expected_configure_pool = r#"
        async fn configure_pool(
            pool: &SqlitePool,
            file_backed: bool,
        ) -> Result<(), RadrootsEventStoreError> {
            let max_connections = pool.options().get_max_connections();
            if !file_backed && max_connections != 1 {
                return Err(RadrootsEventStoreError::UnsafeInMemoryPoolConnectionCount {
                    actual: max_connections,
                });
            }

            let mut connections = Vec::with_capacity(max_connections as usize);
            for _ in 0..max_connections {
                connections.push(pool.acquire().await?);
            }
            for connection in &mut connections {
                let main_filename = main_database_filename(connection).await?;
                let database_is_memory = main_filename.is_empty();
                if file_backed == database_is_memory {
                    return Err(RadrootsEventStoreError::SqlitePoolBackingMismatch {
                        file_backed,
                        filename: main_filename,
                    });
                }
                validate_main_database_encoding(connection).await?;
                crate::schema::validate_event_store_temp_schema(connection).await?;
                sqlx::query("PRAGMA foreign_keys = ON")
                    .execute(&mut **connection)
                    .await?;
                sqlx::query("PRAGMA busy_timeout = 5000")
                    .execute(&mut **connection)
                    .await?;
                if file_backed {
                    configure_file_journal_mode(connection).await?;
                }
            }
            let existing_options = pool.connect_options();
            let connect_options = existing_options
                .as_ref()
                .clone()
                .foreign_keys(true)
                .busy_timeout(Duration::from_millis(5_000));
            let connect_options = if file_backed {
                connect_options.journal_mode(SqliteJournalMode::Wal)
            } else {
                connect_options
            };
            pool.set_connect_options(connect_options);
            Ok(())
        }
    "#;
    if compact_tokens(configure_pool) != compact_source_tokens(expected_configure_pool) {
        return Err(format!(
            "{relative} `configure_pool` must validate every main database as UTF-8 after backing classification and before TEMP-schema, PRAGMA, journal, or connection-option mutation"
        ));
    }

    let validator = exact_top_level_function(relative, file, "validate_main_database_encoding")?;
    let expected_validator = r#"
        async fn validate_main_database_encoding(
            connection: &mut SqliteConnection,
        ) -> Result<(), RadrootsEventStoreError> {
            let actual: String = sqlx::query_scalar("PRAGMA main.encoding")
                .fetch_one(&mut *connection)
                .await?;
            if actual == "UTF-8" {
                return Ok(());
            }
            Err(RadrootsEventStoreError::SqliteMainDatabaseEncodingNotUtf8 { actual, })
        }
    "#;
    if compact_tokens(validator) != compact_source_tokens(expected_validator) {
        return Err(format!(
            "{relative} `validate_main_database_encoding` UTF-8 query or typed failure authority drifted: expected `{}`, found `{}`",
            compact_source_tokens(expected_validator),
            compact_tokens(validator),
        ));
    }
    Ok(())
}

fn validate_source_generation_rollback_authority(
    relative: &str,
    file: &syn::File,
) -> Result<(), String> {
    let policies = file
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Enum(item) if item.ident == "SourceGenerationHistoryRollbackPolicy" => {
                Some(item)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let [policy] = policies.as_slice() else {
        return Err(format!(
            "{relative} must define exactly one production `SourceGenerationHistoryRollbackPolicy`; found {}",
            policies.len()
        ));
    };
    let expected_policy = r#"
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        enum SourceGenerationHistoryRollbackPolicy {
            Preserve,
        }
    "#;
    if compact_tokens(policy) != compact_source_tokens(expected_policy) {
        return Err(format!(
            "{relative} production `SourceGenerationHistoryRollbackPolicy` must contain only `Preserve`"
        ));
    }

    let wrapper =
        exact_top_level_function(relative, file, "rollback_event_store_schema_with_registry")?;
    let expected_wrapper = r#"
        async fn rollback_event_store_schema_with_registry(
            pool: &SqlitePool,
            registry: &[EventStoreMigration],
            minimum: u32,
            supported_current: u32,
            target: u32,
        ) -> Result<(), RadrootsEventStoreError> {
            rollback_event_store_schema_with_registry_inner(
                pool,
                registry,
                minimum,
                supported_current,
                target,
                SourceGenerationHistoryRollbackPolicy::Preserve,
            )
            .await
        }
    "#;
    if compact_tokens(wrapper) != compact_source_tokens(expected_wrapper) {
        return Err(format!(
            "{relative} production rollback registry wrapper must directly select and await the `Preserve` policy"
        ));
    }

    let rollback = exact_top_level_function(relative, file, "rollback_schema_on_connection")?;
    let expected_signature = compact_source_tokens(
        r#"async fn rollback_schema_on_connection(
            connection: &mut SqliteConnection,
            registry: &[EventStoreMigration],
            supported_current: u32,
            target: u32,
            source_generation_history_policy: SourceGenerationHistoryRollbackPolicy,
        ) -> Result<(), RadrootsEventStoreError>"#,
    );
    if !rollback.attrs.is_empty()
        || !matches!(rollback.vis, syn::Visibility::Inherited)
        || compact_tokens(&rollback.sig) != expected_signature
        || rollback.block.stmts.len() != 6
    {
        return Err(format!(
            "{relative} `rollback_schema_on_connection` signature or six-statement authority skeleton drifted"
        ));
    }
    for (index, expected) in [
        r#"let RadrootsEventStoreSchemaStatus::Managed {
                version: current_version
            } = inspect_schema_on_connection(connection, registry, supported_current,).await?
            else {
                return Err(RadrootsEventStoreError::RollbackUnmanaged);
            };"#,
        r#"if target > current_version {
                return Err(RadrootsEventStoreError::RollbackAhead {
                    current: current_version,
                    target,
                });
            }"#,
        r#"if source_generation_history_policy == SourceGenerationHistoryRollbackPolicy::Preserve {
                validate_rollback_preserves_source_generation_history(
                    registry,
                    current_version,
                    target,
                )?;
            }"#,
    ]
    .into_iter()
    .enumerate()
    {
        if compact_tokens(&rollback.block.stmts[index]) != compact_source_tokens(expected) {
            return Err(format!(
                "{relative} `rollback_schema_on_connection` authoritative preflight statement {} drifted: expected `{}`, found `{}`",
                index + 1,
                compact_source_tokens(expected),
                compact_tokens(&rollback.block.stmts[index]),
            ));
        }
    }
    if !matches!(
        rollback.block.stmts.get(3),
        Some(syn::Stmt::Expr(syn::Expr::ForLoop(loop_expression), _))
            if compact_tokens(&loop_expression.pat) == "version"
                && compact_tokens(&loop_expression.expr)
                    == "((target+1)..=current_version).rev()"
    ) {
        return Err(format!(
            "{relative} source-generation rollback guard must remain immediately before the direct down-migration loop"
        ));
    }

    let validator = exact_top_level_function(
        relative,
        file,
        "validate_rollback_preserves_source_generation_history",
    )?;
    let expected_validator = r#"
        fn validate_rollback_preserves_source_generation_history(
            registry: &[EventStoreMigration],
            current: u32,
            target: u32,
        ) -> Result<(), RadrootsEventStoreError> {
            let Some(floor) = registry
                .iter()
                .find(|migration| {
                    migration.hook == EventStoreMigrationHook::Nip09ReconciliationV1
                })
                .map(|migration| migration.version)
            else {
                return Ok(());
            };
            if current < floor || target >= floor {
                return Ok(());
            }

            Err(RadrootsEventStoreError::RollbackWouldDiscardSourceGenerationHistory {
                current,
                target,
                floor,
            })
        }
    "#;
    if compact_tokens(validator) != compact_source_tokens(expected_validator) {
        return Err(format!(
            "{relative} source-generation rollback floor derivation or typed rejection authority drifted: expected `{}`, found `{}`",
            compact_source_tokens(expected_validator),
            compact_tokens(validator),
        ));
    }
    Ok(())
}

fn validate_source_maintenance_runtime_token_authority(
    workspace_root: &Path,
) -> Result<(), String> {
    const SOURCE_RUNTIME_AST_SHA256: &str =
        "c52b1acee9261d45590d8b443f10ad4fdd539fa8d6227e4ca8bc09c593c4c2b6";
    const FUNCTION_SPECS: [(&str, &str, &str); 4] = [
        (
            EVENT_STORE_PROTOCOL_RECONCILIATION_SOURCE_RELATIVE,
            "ingest_event_protocol_reconciliation_v1",
            "f8d26e1d4e1a362c7335f1ba58ad6f1bac2f119162b15ca1067391756149d1e3",
        ),
        (
            EVENT_STORE_PROTOCOL_RECONCILIATION_SOURCE_RELATIVE,
            "read_protocol_post_extension_authority_seal",
            "490e59d21fb84f3321c593ffb67a4d1ada1e5cc8373ed41e2c6834114f2a6ef9",
        ),
        (
            "crates/event_store/src/nip09/reconciliation_v1.rs",
            "apply_reconciliation_hook",
            "c73869559afe06b51c7df019f620509508bb574eaf51f2224493b3be28048682",
        ),
        (
            EVENT_STORE_STORE_SOURCE_RELATIVE,
            "associated:RadrootsEventStore::source_capacity_v1",
            "176c41b212e8d9d4ae3a53cf61dfade6d34b802f5bb649b18f66ffc769d5bade",
        ),
    ];

    let source_relative = "crates/event_store/src/source_maintenance_v1.rs";
    let source_bytes = read_regular_file(workspace_root, source_relative)?;
    let canonical_source =
        canonical_rust_ast(source_relative, &source_bytes, RustAstProfile::Production)?;
    let mut drift = Vec::new();
    let source_sha256 = sha256_hex(&canonical_source);
    if source_sha256 != SOURCE_RUNTIME_AST_SHA256 {
        drift.push(format!(
            "{source_relative}={source_sha256} (expected {SOURCE_RUNTIME_AST_SHA256})"
        ));
    }

    for (relative, function, expected_sha256) in FUNCTION_SPECS {
        let bytes = read_regular_file(workspace_root, relative)?;
        let file = parse_canonical_production_rust(relative, &bytes)?;
        let tokens = if let Some(method) = function.strip_prefix("associated:") {
            let (owner, method) = method.split_once("::").ok_or_else(|| {
                format!("invalid associated SourceMaintenance witness `{function}`")
            })?;
            compact_tokens(exact_associated_function(relative, &file, owner, method)?)
        } else {
            compact_tokens(exact_top_level_function(relative, &file, function)?)
        };
        let actual_sha256 = sha256_hex(tokens.as_bytes());
        if actual_sha256 != expected_sha256 {
            drift.push(format!(
                "{relative}:{function}={actual_sha256} (expected {expected_sha256})"
            ));
        }
    }
    if !drift.is_empty() {
        return Err(format!(
            "current SourceMaintenance runtime exact token authority drifted: {}",
            drift.join(", ")
        ));
    }
    Ok(())
}

fn validate_schema_runtime_reachability<'a>(
    relative: &str,
    file: &'a syn::File,
) -> Result<Vec<&'a syn::ItemFn>, String> {
    const EXPECTED: [(&str, &str); 9] = [
        (
            "inspect_event_store_schema_status",
            r#"pub async fn inspect_event_store_schema_status(
                pool: &SqlitePool,
            ) -> Result<RadrootsEventStoreSchemaStatus, RadrootsEventStoreError> {
                validate_embedded_migration_registry()?;
                inspect_event_store_schema_status_with_registry(
                    pool,
                    EVENT_STORE_MIGRATIONS,
                    RADROOTS_EVENT_STORE_SCHEMA_VERSION_CURRENT,
                ).await
            }"#,
        ),
        (
            "inspect_event_store_schema_status_with_registry",
            r#"async fn inspect_event_store_schema_status_with_registry(
                pool: &SqlitePool,
                registry: &[EventStoreMigration],
                supported_current: u32,
            ) -> Result<RadrootsEventStoreSchemaStatus, RadrootsEventStoreError> {
                let mut transaction = pool.begin().await?;
                let result = inspect_schema_on_connection(
                    &mut transaction,
                    registry,
                    supported_current,
                ).await;
                finish_schema_transaction(transaction, result).await
            }"#,
        ),
        (
            "migrate_event_store_schema",
            r#"pub(crate) async fn migrate_event_store_schema(
                pool: &SqlitePool,
            ) -> Result<(), RadrootsEventStoreError> {
                migrate_event_store_schema_with_generation_provider(
                    pool,
                    &OsSourceGenerationProvider,
                ).await
            }"#,
        ),
        (
            "migrate_event_store_schema_with_generation_provider",
            r#"pub(crate) async fn migrate_event_store_schema_with_generation_provider(
                pool: &SqlitePool,
                generation_provider: &dyn SourceGenerationProvider,
            ) -> Result<(), RadrootsEventStoreError> {
                migrate_event_store_schema_with_generation_provider_and_limits_inner(
                    pool,
                    generation_provider,
                    ReconciliationCapacityLimits::production(),
                ).await
            }"#,
        ),
        (
            "migrate_event_store_schema_with_generation_provider_and_limits_inner",
            r#"async fn migrate_event_store_schema_with_generation_provider_and_limits_inner(
                pool: &SqlitePool,
                generation_provider: &dyn SourceGenerationProvider,
                reconciliation_limits: ReconciliationCapacityLimits,
            ) -> Result<(), RadrootsEventStoreError> {
                validate_embedded_migration_registry()?;
                migrate_event_store_schema_with_registry_and_generation_provider(
                    pool,
                    EVENT_STORE_MIGRATIONS,
                    RADROOTS_EVENT_STORE_SCHEMA_VERSION_MIN,
                    RADROOTS_EVENT_STORE_SCHEMA_VERSION_CURRENT,
                    generation_provider,
                    reconciliation_limits,
                ).await
            }"#,
        ),
        (
            "migrate_event_store_schema_with_registry_and_generation_provider",
            r#"async fn migrate_event_store_schema_with_registry_and_generation_provider(
                pool: &SqlitePool,
                registry: &[EventStoreMigration],
                minimum: u32,
                supported_current: u32,
                generation_provider: &dyn SourceGenerationProvider,
                reconciliation_limits: ReconciliationCapacityLimits,
            ) -> Result<(), RadrootsEventStoreError> {
                validate_migration_registry(registry, minimum, supported_current)?;
                let status = inspect_event_store_schema_status_with_registry(
                    pool,
                    registry,
                    supported_current,
                ).await?;
                if status == (RadrootsEventStoreSchemaStatus::Managed {
                    version: supported_current,
                }) {
                    return Ok(());
                }
                if has_pending_source_capacity_hook(&status, registry) {
                    let mut connection = pool.acquire().await?;
                    validate_event_store_temp_schema_with_registry(
                        &mut connection,
                        registry
                    ).await?;
                    validate_reconciliation_capacity(
                        &mut connection,
                        reconciliation_limits
                    ).await?;
                    if has_pending_source_maintenance_hook(&status, registry) {
                        validate_no_persisted_ephemeral_raw_rows_v1(
                            &mut connection
                        ).await?;
                    }
                }
                let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
                let result = migrate_schema_on_connection(
                    &mut transaction,
                    registry,
                    supported_current,
                    generation_provider,
                    reconciliation_limits,
                ).await;
                finish_schema_transaction(transaction, result).await
            }"#,
        ),
        (
            "validate_event_store_temp_schema",
            r#"pub(crate) async fn validate_event_store_temp_schema(
                connection: &mut SqliteConnection,
            ) -> Result<(), RadrootsEventStoreError> {
                validate_event_store_temp_schema_with_registry(
                    connection,
                    EVENT_STORE_MIGRATIONS
                ).await
            }"#,
        ),
        (
            "validate_event_store_temp_schema_with_registry",
            r#"async fn validate_event_store_temp_schema_with_registry(
                connection: &mut SqliteConnection,
                registry: &[EventStoreMigration],
            ) -> Result<(), RadrootsEventStoreError> {
                let rows = sqlx::query(
                    "SELECT type, name, tbl_name FROM temp.sqlite_schema ORDER BY type, name, tbl_name",
                )
                .fetch_all(&mut *connection)
                .await?;
                for row in rows {
                    let object_type: String = row.try_get("type")?;
                    let name: String = row.try_get("name")?;
                    let table_name: String = row.try_get("tbl_name")?;
                    if matches!(object_type.as_str(), "trigger" | "view")
                        || is_event_store_governed_schema_name(registry, &name)
                        || is_event_store_governed_schema_name(registry, &table_name)
                    {
                        return Err(RadrootsEventStoreError::TemporarySchemaCollision {
                            object_type,
                            name,
                            table_name,
                        });
                    }
                }
                Ok(())
            }"#,
        ),
        (
            "read_catalog",
            r#"async fn read_catalog(
                connection: &mut SqliteConnection,
            ) -> Result<Vec<CatalogRow>, RadrootsEventStoreError> {
                let rows = sqlx::query(
                    "SELECT type, name, tbl_name, sql FROM main.sqlite_schema"
                )
                .fetch_all(&mut *connection)
                .await?;
                let catalog = rows
                    .into_iter()
                    .map(|row| {
                        Ok(CatalogRow {
                            object_type: row.try_get("type")?,
                            name: row.try_get("name")?,
                            table_name: row.try_get("tbl_name")?,
                            sql: row.try_get("sql")?,
                        })
                    })
                    .collect::<Result<Vec<_>, RadrootsEventStoreError>>()?;
                Ok(catalog
                    .into_iter()
                    .filter(|row| !sqlite_identifier_starts_with(&row.name, "sqlite_"))
                    .collect(),)
            }"#,
        ),
    ];

    let functions = EXPECTED
        .iter()
        .map(|(name, expected)| {
            let function = exact_top_level_function(relative, file, name)?;
            let actual = compact_tokens(function);
            let expected = compact_source_tokens(expected);
            if actual != expected {
                return Err(format!(
                    "{relative} authoritative schema runtime `{name}` signature or control flow drifted: expected `{expected}`, found `{actual}`"
                ));
            }
            Ok(function)
        })
        .collect::<Result<Vec<_>, String>>()?;
    let migrate = exact_top_level_function(relative, file, "migrate_schema_on_connection")?;
    let current_version = migrate
        .block
        .stmts
        .iter()
        .find(|statement| {
            matches!(
                statement,
                syn::Stmt::Local(local)
                    if local_pattern_ident(&local.pat).as_deref() == Some("current_version")
            )
        })
        .ok_or_else(|| {
            format!(
                "{relative} authoritative schema runtime is missing the direct `current_version` initializer"
            )
        })?;
    let expected_current_version = parse_canonical_production_rust(
        "authoritative migrate_schema_on_connection current_version initializer",
        br#"fn expected() {
            let current_version = match status {
                RadrootsEventStoreSchemaStatus::Uninitialized => {
                    apply_migration_up(connection, registry, &registry[0]).await?;
                    create_ledger(connection).await?;
                    insert_ledger_row(connection, &registry[0]).await?;
                    registry[0].version
                }
                RadrootsEventStoreSchemaStatus::UnledgeredBaseline => {
                    create_ledger(connection).await?;
                    insert_ledger_row(connection, &registry[0]).await?;
                    registry[0].version
                }
                RadrootsEventStoreSchemaStatus::Managed { version }
                    if version == supported_current =>
                {
                    return Ok(());
                }
                RadrootsEventStoreSchemaStatus::Managed { version } => version,
            };
        }"#,
    )?;
    let expected_current_version =
        exact_top_level_function(relative, &expected_current_version, "expected")?
            .block
            .stmts
            .first()
            .expect("authoritative current-version initializer");
    if compact_tokens(current_version) != compact_tokens(expected_current_version) {
        return Err(format!(
            "{relative} authoritative schema runtime `migrate_schema_on_connection` current-version control flow drifted"
        ));
    }
    validate_migration_hook_loop_reachability(relative, file)?;
    Ok(functions)
}

fn validate_migration_hook_loop_reachability(
    relative: &str,
    file: &syn::File,
) -> Result<(), String> {
    let apply_loop = exact_direct_for_loop(
        relative,
        file,
        "migrate_schema_on_connection",
        "migration",
        "registry.iter().filter(|migration|migration.version>current_version)",
    )?;
    let source_preflight = r#"if matches!(
        migration.hook,
        EventStoreMigrationHook::Nip09ReconciliationV1
            | EventStoreMigrationHook::FoodAvailabilityProjectionV1
            | EventStoreMigrationHook::SourceMaintenanceV1
    ) {
        validate_reconciliation_capacity(connection, reconciliation_limits).await?;
        if migration.hook == EventStoreMigrationHook::SourceMaintenanceV1 {
            validate_no_persisted_ephemeral_raw_rows_v1(connection).await?;
        }
    }"#;
    if apply_loop.body.stmts.first().is_none_or(|statement| {
        compact_tokens(statement) != compact_source_tokens(source_preflight)
    }) {
        return Err(format!(
            "{relative} `migrate_schema_on_connection` SourceMaintenance preflight or error propagation drifted"
        ));
    }
    for called in [
        "apply_migration_up",
        "apply_migration_hook",
        "validate_applied_migration_hooks",
        "insert_ledger_row",
    ] {
        exact_direct_loop_awaited_call(
            relative,
            "migrate_schema_on_connection",
            apply_loop,
            called,
        )?;
    }

    let validate_loop = exact_direct_for_loop(
        relative,
        file,
        "validate_applied_migration_hooks",
        "migration",
        "registry.iter().filter(|migration|migration.version<=current)",
    )?;
    exact_direct_loop_awaited_call(
        relative,
        "validate_applied_migration_hooks",
        validate_loop,
        "validate_migration_hook_state",
    )?;
    Ok(())
}

fn validate_no_diverging_control_flow(
    relative: &str,
    item: &str,
    statement: &syn::Stmt,
) -> Result<(), String> {
    use syn::visit::{self, Visit};

    #[derive(Default, Eq, PartialEq)]
    struct Divergence {
        returns: usize,
        breaks: usize,
        continues: usize,
    }
    impl<'ast> Visit<'ast> for Divergence {
        fn visit_expr_return(&mut self, expression: &'ast syn::ExprReturn) {
            self.returns += 1;
            visit::visit_expr_return(self, expression);
        }

        fn visit_expr_break(&mut self, expression: &'ast syn::ExprBreak) {
            self.breaks += 1;
            visit::visit_expr_break(self, expression);
        }

        fn visit_expr_continue(&mut self, expression: &'ast syn::ExprContinue) {
            self.continues += 1;
            visit::visit_expr_continue(self, expression);
        }
    }
    let mut divergence = Divergence::default();
    divergence.visit_stmt(statement);
    if divergence != Divergence::default() {
        return Err(format!(
            "{relative} `{item}` contains return/break/continue before its authoritative hook call"
        ));
    }
    Ok(())
}

fn validate_exact_direct_method_call(
    relative: &str,
    fragment: &str,
    expression: &syn::Expr,
    method: &str,
    receiver: &str,
) -> Result<(), String> {
    let Some(syn::Expr::MethodCall(call)) = direct_terminal_expression(expression) else {
        return Err(format!(
            "{relative} {fragment} must be a direct `{receiver}.{method}()` call"
        ));
    };
    if call.method != method
        || !call.args.is_empty()
        || compact_tokens(&call.receiver) != compact_source_tokens(receiver)
    {
        return Err(format!(
            "{relative} {fragment} must be a direct `{receiver}.{method}()` call"
        ));
    }
    Ok(())
}

fn validate_manifest_pointer_check(
    relative: &str,
    binding: &str,
    expression: &syn::Expr,
    value_accessor: &str,
) -> Result<(), String> {
    use syn::visit::Visit;

    let mut routes = RustCallRouteCollector { routes: Vec::new() };
    routes.visit_expr(expression);
    for route in [
        "method:iter".to_owned(),
        "method:all".to_owned(),
        "method:pointer".to_owned(),
        "method:and_then".to_owned(),
        format!("method:{value_accessor}"),
    ] {
        let count = routes
            .routes
            .iter()
            .filter(|candidate| **candidate == route)
            .count();
        if count != 1 {
            return Err(format!(
                "{relative} `{binding}` must contain exactly one `{route}` semantic route; found {count}"
            ));
        }
    }
    if !syntax_contains_ident(expression, "manifest")
        || !syntax_contains_ident(expression, "pointer")
        || !syntax_contains_ident(expression, "expected")
    {
        return Err(format!(
            "{relative} `{binding}` must compare manifest pointer values with the expected table"
        ));
    }
    Ok(())
}

fn direct_statement_expression(statement: &syn::Stmt) -> Option<&syn::Expr> {
    match statement {
        syn::Stmt::Local(local) => local
            .init
            .as_ref()
            .map(|initializer| initializer.expr.as_ref()),
        syn::Stmt::Expr(expression, _) => Some(expression),
        syn::Stmt::Item(_) | syn::Stmt::Macro(_) => None,
    }
}

fn direct_function_call<'a>(expression: &'a syn::Expr, called: &str) -> Option<&'a syn::ExprCall> {
    let syn::Expr::Call(call) = direct_terminal_expression(expression)? else {
        return None;
    };
    function_call_matches(call, called).then_some(call)
}

fn direct_try_function_call<'a>(
    expression: &'a syn::Expr,
    called: &str,
) -> Option<&'a syn::ExprCall> {
    let syn::Expr::Try(expression) = peel_group_or_paren(expression) else {
        return None;
    };
    let syn::Expr::Call(call) = peel_group_or_paren(&expression.expr) else {
        return None;
    };
    function_call_matches(call, called).then_some(call)
}

fn peel_group_or_paren(mut expression: &syn::Expr) -> &syn::Expr {
    loop {
        expression = match expression {
            syn::Expr::Group(expression) => &expression.expr,
            syn::Expr::Paren(expression) => &expression.expr,
            _ => return expression,
        };
    }
}

fn direct_call_chain_function_call<'a>(
    expression: &'a syn::Expr,
    called: &str,
) -> Option<&'a syn::ExprCall> {
    let mut expression = expression;
    loop {
        expression = match expression {
            syn::Expr::Try(expression) => &expression.expr,
            syn::Expr::Await(expression) => &expression.base,
            syn::Expr::Group(expression) => &expression.expr,
            syn::Expr::Paren(expression) => &expression.expr,
            syn::Expr::MethodCall(expression) => &expression.receiver,
            syn::Expr::Call(call) => {
                return function_call_matches(call, called).then_some(call);
            }
            _ => return None,
        };
    }
}

fn function_call_matches(call: &syn::ExprCall, called: &str) -> bool {
    let syn::Expr::Path(path) = call.func.as_ref() else {
        return false;
    };
    path.qself.is_none() && compact_tokens(&path.path) == compact_source_tokens(called)
}

fn local_pattern_ident(pattern: &syn::Pat) -> Option<String> {
    match pattern {
        syn::Pat::Ident(pattern) => Some(pattern.ident.to_string()),
        syn::Pat::Type(pattern) => local_pattern_ident(&pattern.pat),
        _ => None,
    }
}

fn compact_tokens(node: &impl ToTokens) -> String {
    compact_token_stream(node.to_token_stream())
}

fn compact_source_tokens(source: &str) -> String {
    compact_token_stream(
        source
            .parse()
            .expect("authoritative Rust source fragment must tokenize"),
    )
}

fn compact_token_stream(tokens: proc_macro2::TokenStream) -> String {
    use proc_macro2::{Delimiter, TokenTree};

    let mut compact = String::new();
    for token in tokens {
        match token {
            TokenTree::Group(group) => {
                let (open, close) = match group.delimiter() {
                    Delimiter::Parenthesis => ("(", ")"),
                    Delimiter::Brace => ("{", "}"),
                    Delimiter::Bracket => ("[", "]"),
                    Delimiter::None => ("", ""),
                };
                compact.push_str(open);
                compact.push_str(&compact_token_stream(group.stream()));
                compact.push_str(close);
            }
            TokenTree::Ident(ident) => compact.push_str(ident.to_string().as_str()),
            TokenTree::Punct(punctuation) => compact.push(punctuation.as_char()),
            TokenTree::Literal(literal) => compact.push_str(literal.to_string().as_str()),
        }
    }
    compact
}

fn exact_top_level_function<'a>(
    relative: &str,
    file: &'a syn::File,
    name: &str,
) -> Result<&'a syn::ItemFn, String> {
    let matches = file
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Fn(function) if function.sig.ident == name => Some(function),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [function] = matches.as_slice() else {
        return Err(format!(
            "{relative} must define exactly one function `{name}`; found {}",
            matches.len()
        ));
    };
    Ok(function)
}

fn exact_enum_variant<'a>(
    relative: &str,
    file: &'a syn::File,
    enum_name: &str,
    variant_name: &str,
) -> Result<&'a syn::Variant, String> {
    let enums = file
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Enum(item_enum) if item_enum.ident == enum_name => Some(item_enum),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [item_enum] = enums.as_slice() else {
        return Err(format!(
            "{relative} must define exactly one enum `{enum_name}`; found {}",
            enums.len()
        ));
    };
    let variants = item_enum
        .variants
        .iter()
        .filter(|variant| variant.ident == variant_name)
        .collect::<Vec<_>>();
    let [variant] = variants.as_slice() else {
        return Err(format!(
            "{relative} enum `{enum_name}` must define exactly one `{variant_name}` variant; found {}",
            variants.len()
        ));
    };
    Ok(variant)
}

fn exact_const_struct_array_element<'a>(
    relative: &str,
    file: &'a syn::File,
    const_name: &str,
    discriminant_field: &str,
    discriminant_value: u64,
) -> Result<&'a syn::ExprStruct, String> {
    let constants = file
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Const(constant) if constant.ident == const_name => Some(constant),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [constant] = constants.as_slice() else {
        return Err(format!(
            "{relative} must define exactly one constant `{const_name}`; found {}",
            constants.len()
        ));
    };
    let expression = peel_expression(&constant.expr);
    let syn::Expr::Array(array) = expression else {
        return Err(format!("{relative} `{const_name}` must contain an array"));
    };
    let matches = array
        .elems
        .iter()
        .filter_map(|element| match peel_expression(element) {
            syn::Expr::Struct(item) => Some(item),
            _ => None,
        })
        .filter(|item| {
            item.fields.iter().any(|field| {
                matches!(
                    (&field.member, peel_expression(&field.expr)),
                    (syn::Member::Named(name), syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Int(value),
                        ..
                    })) if name == discriminant_field
                        && value.base10_parse::<u64>().ok() == Some(discriminant_value)
                )
            })
        })
        .collect::<Vec<_>>();
    let [element] = matches.as_slice() else {
        return Err(format!(
            "{relative} `{const_name}` must contain exactly one {discriminant_field}={discriminant_value} struct element; found {}",
            matches.len()
        ));
    };
    Ok(element)
}

fn exact_top_level_struct<'a>(
    relative: &str,
    file: &'a syn::File,
    name: &str,
) -> Result<&'a syn::ItemStruct, String> {
    let structs = file
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Struct(item) if item.ident == name => Some(item),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [item] = structs.as_slice() else {
        return Err(format!(
            "{relative} must define exactly one top-level struct `{name}`; found {}",
            structs.len()
        ));
    };
    Ok(item)
}

fn peel_expression(mut expression: &syn::Expr) -> &syn::Expr {
    loop {
        expression = match expression {
            syn::Expr::Reference(reference) => &reference.expr,
            syn::Expr::Group(group) => &group.expr,
            syn::Expr::Paren(paren) => &paren.expr,
            _ => return expression,
        };
    }
}

fn exact_associated_match_arm<'a>(
    relative: &str,
    file: &'a syn::File,
    owner: &str,
    method: &str,
    variant: &str,
) -> Result<&'a syn::Arm, String> {
    let function = exact_associated_function(relative, file, owner, method)?;
    if function.block.stmts.len() != 1 {
        return Err(format!(
            "{relative} `{owner}::{method}` authoritative match must be its only statement"
        ));
    }
    let expression = direct_tail_match(&function.block).ok_or_else(|| {
        format!(
            "{relative} `{owner}::{method}` must directly return its authoritative `match self`"
        )
    })?;
    if compact_tokens(&expression.expr) != "self" {
        return Err(format!(
            "{relative} `{owner}::{method}` authoritative match must use `self` as its scrutinee"
        ));
    }
    exact_variant_arm(relative, &format!("{owner}::{method}"), expression, variant)
}

fn exact_tail_match_arm<'a>(
    relative: &str,
    file: &'a syn::File,
    function: &str,
    scrutinee: &str,
    variant: &str,
) -> Result<&'a syn::Arm, String> {
    let function_item = exact_top_level_function(relative, file, function)?;
    if function_item.block.stmts.len() != 1 {
        return Err(format!(
            "{relative} `{function}` authoritative match must be its only statement"
        ));
    }
    let expression = direct_tail_match(&function_item.block).ok_or_else(|| {
        format!(
            "{relative} `{function}` must directly return its authoritative `{scrutinee}` match"
        )
    })?;
    if compact_tokens(&expression.expr) != compact_source_tokens(scrutinee) {
        return Err(format!(
            "{relative} `{function}` authoritative match must use `{scrutinee}` as its scrutinee"
        ));
    }
    exact_variant_arm(relative, function, expression, variant)
}

fn exact_direct_for_loop<'a>(
    relative: &str,
    file: &'a syn::File,
    function: &str,
    pattern: &str,
    iterator: &str,
) -> Result<&'a syn::ExprForLoop, String> {
    let function_item = exact_top_level_function(relative, file, function)?;
    let matches = function_item
        .block
        .stmts
        .iter()
        .filter_map(|statement| match statement {
            syn::Stmt::Expr(syn::Expr::ForLoop(expression), _)
                if compact_tokens(&expression.pat) == compact_source_tokens(pattern)
                    && compact_tokens(&expression.expr) == compact_source_tokens(iterator) =>
            {
                Some(expression)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let [expression] = matches.as_slice() else {
        return Err(format!(
            "{relative} `{function}` must contain exactly one direct `for {pattern} in {iterator}` loop; found {}",
            matches.len()
        ));
    };
    let (expected_statement_count, expected_index) = match function {
        "validate_migration_registry" => (11, 8),
        "migrate_schema_on_connection" => (5, 2),
        "validate_applied_migration_hooks" => (2, 0),
        _ => {
            return Err(format!(
                "{relative} `{function}` is not an approved authoritative-loop witness"
            ));
        }
    };
    if function_item.block.stmts.len() != expected_statement_count
        || !matches!(
            function_item.block.stmts.get(expected_index),
            Some(syn::Stmt::Expr(syn::Expr::ForLoop(candidate), _))
                if std::ptr::eq(candidate, *expression)
        )
    {
        return Err(format!(
            "{relative} `{function}` authoritative loop must remain direct statement {} of {}",
            expected_index + 1,
            expected_statement_count
        ));
    }
    Ok(expression)
}

fn exact_direct_loop_match_arm<'a>(
    relative: &str,
    function: &str,
    loop_expression: &'a syn::ExprForLoop,
    scrutinee_fields: &[&str],
    variant: &str,
) -> Result<&'a syn::Arm, String> {
    if function == "validate_migration_registry"
        && (loop_expression.body.stmts.len() != 20
            || !matches!(
                loop_expression.body.stmts.get(18),
                Some(syn::Stmt::Expr(syn::Expr::Match(_), _))
            ))
    {
        return Err(format!(
            "{relative} `{function}` authoritative registry loop statement skeleton drifted"
        ));
    }
    let matches = loop_expression
        .body
        .stmts
        .iter()
        .filter_map(|statement| match statement {
            syn::Stmt::Expr(syn::Expr::Match(expression), _)
                if matches!(
                    peel_expression(&expression.expr),
                    syn::Expr::Tuple(tuple)
                        if tuple.elems.len() == scrutinee_fields.len()
                            && tuple.elems.iter().zip(scrutinee_fields).all(
                                |(actual, expected)| {
                                    compact_tokens(actual) == compact_source_tokens(expected)
                                },
                            )
                ) =>
            {
                Some(expression)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let [expression] = matches.as_slice() else {
        return Err(format!(
            "{relative} `{function}` authoritative loop must contain exactly one direct match over {scrutinee_fields:?}; found {}",
            matches.len()
        ));
    };
    exact_variant_arm(relative, function, expression, variant)
}

fn direct_tail_match(block: &syn::Block) -> Option<&syn::ExprMatch> {
    let syn::Stmt::Expr(expression, None) = block.stmts.last()? else {
        return None;
    };
    let syn::Expr::Match(expression) = peel_expression(expression) else {
        return None;
    };
    Some(expression)
}

fn exact_variant_arm<'a>(
    relative: &str,
    item: &str,
    expression: &'a syn::ExprMatch,
    variant: &str,
) -> Result<&'a syn::Arm, String> {
    let matches = expression
        .arms
        .iter()
        .filter(|arm| syntax_contains_ident(&arm.pat, variant))
        .collect::<Vec<_>>();
    let [arm] = matches.as_slice() else {
        return Err(format!(
            "{relative} `{item}` authoritative match must contain exactly one structured `{variant}` arm; found {}",
            matches.len()
        ));
    };
    Ok(arm)
}

fn exact_direct_loop_awaited_call<'a>(
    relative: &str,
    function: &str,
    loop_expression: &'a syn::ExprForLoop,
    called: &str,
) -> Result<&'a syn::ExprAwait, String> {
    let (expected_statement_count, expected_index) = match (function, called) {
        ("migrate_schema_on_connection", "apply_migration_up") => (5, 1),
        ("migrate_schema_on_connection", "apply_migration_hook") => (5, 2),
        ("migrate_schema_on_connection", "validate_applied_migration_hooks") => (5, 3),
        ("migrate_schema_on_connection", "insert_ledger_row") => (5, 4),
        ("validate_applied_migration_hooks", "validate_migration_hook_state") => (1, 0),
        _ => {
            return Err(format!(
                "{relative} `{function}` is not an approved awaited-loop witness"
            ));
        }
    };
    if loop_expression.body.stmts.len() != expected_statement_count {
        return Err(format!(
            "{relative} `{function}` authoritative loop must contain exactly {expected_statement_count} statements"
        ));
    }
    let matches = loop_expression
        .body
        .stmts
        .iter()
        .filter_map(direct_statement_expression)
        .filter_map(|expression| direct_try_awaited_function_call(expression, called))
        .collect::<Vec<_>>();
    let [expression] = matches.as_slice() else {
        return Err(format!(
            "{relative} `{function}` authoritative loop must contain exactly one direct awaited `{called}` call; found {}",
            matches.len()
        ));
    };
    if direct_statement_expression(
        loop_expression
            .body
            .stmts
            .get(expected_index)
            .expect("validated statement count"),
    )
    .and_then(|statement| direct_try_awaited_function_call(statement, called))
    .is_none_or(|candidate| !std::ptr::eq(candidate, *expression))
    {
        return Err(format!(
            "{relative} `{function}` awaited `{called}` call must remain direct loop statement {}",
            expected_index + 1
        ));
    }
    for statement in &loop_expression.body.stmts[..expected_index] {
        validate_no_diverging_control_flow(relative, function, statement)?;
    }
    Ok(expression)
}

fn validate_direct_arm_awaited_call(
    relative: &str,
    fragment: &str,
    expression: &syn::Expr,
    called: &str,
) -> Result<(), String> {
    let expression = direct_arm_tail_expression(expression)
        .ok_or_else(|| format!("{relative} {fragment} must directly return awaited `{called}`"))?;
    if direct_awaited_function_call(expression, called).is_none() {
        return Err(format!(
            "{relative} {fragment} must directly return awaited `{called}`"
        ));
    }
    Ok(())
}

fn direct_arm_tail_expression(mut expression: &syn::Expr) -> Option<&syn::Expr> {
    loop {
        expression = match expression {
            syn::Expr::Group(expression) => &expression.expr,
            syn::Expr::Paren(expression) => &expression.expr,
            syn::Expr::Block(expression) => {
                let syn::Stmt::Expr(tail, _) = expression.block.stmts.last()? else {
                    return None;
                };
                tail
            }
            _ => return Some(expression),
        };
    }
}

fn direct_awaited_function_call<'a>(
    expression: &'a syn::Expr,
    called: &str,
) -> Option<&'a syn::ExprAwait> {
    let mut expression = expression;
    loop {
        expression = match expression {
            syn::Expr::Try(expression) => &expression.expr,
            syn::Expr::Group(expression) => &expression.expr,
            syn::Expr::Paren(expression) => &expression.expr,
            syn::Expr::Await(expression) => {
                return direct_call_chain_function_call(&expression.base, called)
                    .is_some()
                    .then_some(expression);
            }
            _ => return None,
        };
    }
}

fn direct_try_awaited_function_call<'a>(
    expression: &'a syn::Expr,
    called: &str,
) -> Option<&'a syn::ExprAwait> {
    let syn::Expr::Try(expression) = peel_group_or_paren(expression) else {
        return None;
    };
    let syn::Expr::Await(expression) = peel_group_or_paren(&expression.expr) else {
        return None;
    };
    direct_call_chain_function_call(&expression.base, called)
        .is_some()
        .then_some(expression)
}

fn syntax_contains_ident(node: &impl ToTokens, ident: &str) -> bool {
    node.to_token_stream()
        .to_string()
        .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .any(|token| token == ident)
}

fn describe_result_vector(workspace_root: &Path) -> Result<MirroredFileDescriptor, String> {
    let canonical = read_regular_file(workspace_root, RESULT_VECTOR_CANONICAL_RELATIVE)?;
    let mirror = read_regular_file(workspace_root, RESULT_VECTOR_MIRROR_RELATIVE)?;
    if mirror != canonical {
        return Err(format!(
            "result vector mirror {RESULT_VECTOR_MIRROR_RELATIVE} must exactly match {RESULT_VECTOR_CANONICAL_RELATIVE}"
        ));
    }
    let vector: ReconciliationResultVector = serde_json::from_slice(&canonical)
        .map_err(|error| format!("parse {RESULT_VECTOR_CANONICAL_RELATIVE}: {error}"))?;
    validate_canonical_json(RESULT_VECTOR_CANONICAL_RELATIVE, &canonical, &vector)?;
    validate_result_vector(&vector)?;
    let executor = read_regular_file(workspace_root, RESULT_VECTOR_EXECUTOR_RELATIVE)?;
    let executor_source = std::str::from_utf8(&executor).map_err(|error| {
        format!("{RESULT_VECTOR_EXECUTOR_RELATIVE} must be UTF-8 Rust source: {error}")
    })?;
    validate_result_vector_executor_source(RESULT_VECTOR_EXECUTOR_RELATIVE, executor_source)?;
    let canonical_executor = canonical_rust_ast(
        RESULT_VECTOR_EXECUTOR_RELATIVE,
        &executor,
        RustAstProfile::Full,
    )?;
    Ok(MirroredFileDescriptor {
        canonical_path: RESULT_VECTOR_CANONICAL_RELATIVE.to_owned(),
        mirror_path: RESULT_VECTOR_MIRROR_RELATIVE.to_owned(),
        byte_length: byte_length(RESULT_VECTOR_CANONICAL_RELATIVE, &canonical)?,
        sha256: sha256_hex(&canonical),
        executor_id: RESULT_VECTOR_EXECUTOR_ID.to_owned(),
        executor_path: RESULT_VECTOR_EXECUTOR_RELATIVE.to_owned(),
        executor_test: RESULT_VECTOR_EXECUTOR_TEST.to_owned(),
        executor_hash_algorithm: RUST_FULL_AST_SHA256_ALGORITHM.to_owned(),
        executor_canonical_byte_length: byte_length(
            RESULT_VECTOR_EXECUTOR_RELATIVE,
            &canonical_executor,
        )?,
        executor_sha256: sha256_hex(&canonical_executor),
    })
}

fn validate_result_vector_executor_source(relative: &str, source: &str) -> Result<(), String> {
    use syn::parse::Parser;
    use syn::visit::{self, Visit};

    let file =
        syn::parse_file(source).map_err(|error| format!("parse {relative} as Rust: {error}"))?;
    let executor_id = exact_executor_const(&file, relative, "RESULT_VECTOR_EXECUTOR_ID")?;
    let syn::Expr::Lit(syn::ExprLit {
        lit: syn::Lit::Str(executor_id),
        ..
    }) = executor_id.expr.as_ref()
    else {
        return Err(format!(
            "{relative} RESULT_VECTOR_EXECUTOR_ID must be a string literal"
        ));
    };
    if executor_id.value() != RESULT_VECTOR_EXECUTOR_ID {
        return Err(format!(
            "{relative} RESULT_VECTOR_EXECUTOR_ID must equal {RESULT_VECTOR_EXECUTOR_ID}"
        ));
    }

    let name = "RESULT_VECTOR_BYTES";
    let expected_path = RESULT_VECTOR_INCLUDE_PATH;
    let item = exact_executor_const(&file, relative, name)?;
    let syn::Expr::Macro(expression) = item.expr.as_ref() else {
        return Err(format!("{relative} {name} must use include_bytes!"));
    };
    if !expression.mac.path.is_ident("include_bytes") {
        return Err(format!("{relative} {name} must use include_bytes!"));
    }
    let include_path = syn::parse2::<syn::LitStr>(expression.mac.tokens.clone())
        .map_err(|error| format!("parse {relative} {name} include_bytes! path: {error}"))?;
    if include_path.value() != expected_path {
        return Err(format!("{relative} {name} must include `{expected_path}`"));
    }

    let tests = file
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Fn(function) if function.sig.ident == RESULT_VECTOR_EXECUTOR_TEST => {
                Some(function)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let [test] = tests.as_slice() else {
        return Err(format!(
            "{relative} must contain exactly one function named {RESULT_VECTOR_EXECUTOR_TEST}; found {}",
            tests.len()
        ));
    };
    let has_tokio_test = test.attrs.iter().any(|attribute| {
        matches!(&attribute.meta, syn::Meta::Path(_))
            && attribute
                .path()
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .eq(["tokio", "test"].into_iter().map(str::to_owned))
    });
    if !has_tokio_test || test.sig.asyncness.is_none() || !test.sig.inputs.is_empty() {
        return Err(format!(
            "{relative} {RESULT_VECTOR_EXECUTOR_TEST} must be an argument-free async #[tokio::test]"
        ));
    }

    #[derive(Default)]
    struct ReferencedPaths {
        paths: BTreeSet<String>,
    }

    impl<'ast> Visit<'ast> for ReferencedPaths {
        fn visit_expr_path(&mut self, expression: &'ast syn::ExprPath) {
            self.paths.insert(
                expression
                    .path
                    .segments
                    .iter()
                    .map(|segment| segment.ident.to_string())
                    .collect::<Vec<_>>()
                    .join("::"),
            );
            visit::visit_expr_path(self, expression);
        }

        fn visit_macro(&mut self, item: &'ast syn::Macro) {
            let parser = syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated;
            if let Ok(expressions) = parser.parse2(item.tokens.clone()) {
                for expression in &expressions {
                    self.visit_expr(expression);
                }
            }
            visit::visit_macro(self, item);
        }
    }

    let mut references = ReferencedPaths::default();
    references.visit_block(&test.block);
    for required in [
        "RESULT_VECTOR_EXECUTOR_ID",
        "RESULT_VECTOR_BYTES",
        "nip09_manifest::NIP09_RECONCILIATION_RESULT_VECTOR_EXECUTOR_ID",
        "nip09_manifest::NIP09_RECONCILIATION_RESULT_VECTOR_SHA256",
        "nip09_manifest::NIP09_RECONCILIATION_HOOK_ID",
    ] {
        if !references.paths.contains(required) {
            return Err(format!(
                "{relative} {RESULT_VECTOR_EXECUTOR_TEST} must reference {required}"
            ));
        }
    }
    Ok(())
}

fn exact_executor_const<'a>(
    file: &'a syn::File,
    relative: &str,
    name: &str,
) -> Result<&'a syn::ItemConst, String> {
    let constants = file
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Const(item) if item.ident == name => Some(item),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [constant] = constants.as_slice() else {
        return Err(format!(
            "{relative} must contain exactly one constant named {name}; found {}",
            constants.len()
        ));
    };
    Ok(constant)
}

fn describe_file(workspace_root: &Path, relative: &str) -> Result<(u64, String), String> {
    let bytes = read_regular_file(workspace_root, relative)?;
    Ok((byte_length(relative, &bytes)?, sha256_hex(&bytes)))
}

fn describe_local_sqlite_source(
    workspace_root: &Path,
) -> Result<LocalRuntimeSourceDescriptor, String> {
    let workspace_manifest =
        parse_cargo_manifest(workspace_root, WORKSPACE_CARGO_MANIFEST_RELATIVE)?;
    let package_manifest_relative = format!("{LOCAL_SQLITE_SOURCE_RELATIVE}/Cargo.toml");
    let package_manifest = parse_cargo_manifest(workspace_root, &package_manifest_relative)?;
    let package = package_manifest
        .get("package")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| format!("{package_manifest_relative} must declare [package]"))?;
    let package_name = package
        .get("name")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| format!("{package_manifest_relative} must declare package.name"))?;
    let package_version = package
        .get("version")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| format!("{package_manifest_relative} must declare package.version"))?;
    if package_name != LOCAL_SQLITE_PACKAGE || package_version != LOCAL_SQLITE_VERSION {
        return Err(format!(
            "{package_manifest_relative} must identify {LOCAL_SQLITE_PACKAGE} {LOCAL_SQLITE_VERSION}"
        ));
    }
    if package.get("publish").and_then(toml::Value::as_bool) != Some(false) {
        return Err(format!(
            "{package_manifest_relative} governed patched source must set publish = false"
        ));
    }

    let patch = workspace_manifest
        .get("patch")
        .and_then(toml::Value::as_table)
        .and_then(|patch| patch.get("crates-io"))
        .and_then(toml::Value::as_table)
        .and_then(|registry| registry.get(LOCAL_SQLITE_PACKAGE))
        .and_then(toml::Value::as_table)
        .ok_or_else(|| {
            format!(
                "{WORKSPACE_CARGO_MANIFEST_RELATIVE} must declare [patch.crates-io].{LOCAL_SQLITE_PACKAGE}"
            )
        })?;
    if patch.get("path").and_then(toml::Value::as_str) != Some(LOCAL_SQLITE_SOURCE_RELATIVE) {
        return Err(format!(
            "{WORKSPACE_CARGO_MANIFEST_RELATIVE} patched {LOCAL_SQLITE_PACKAGE} path must be {LOCAL_SQLITE_SOURCE_RELATIVE}"
        ));
    }

    let feature_table = package_manifest
        .get("features")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| format!("{package_manifest_relative} must declare [features]"))?;
    let mut feature_definitions = feature_table
        .iter()
        .map(|(name, value)| {
            let mut enables = toml_string_array(
                &package_manifest_relative,
                Some(value),
                &format!("features.{name}"),
            )?;
            enables.sort();
            validate_unique_owned(
                &format!("{package_manifest_relative} features.{name}"),
                &enables,
            )?;
            Ok(CargoFeatureDefinitionDescriptor {
                name: name.clone(),
                enables,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    feature_definitions.sort();
    let bundled = feature_definitions
        .iter()
        .find(|feature| feature.name == "bundled")
        .ok_or_else(|| format!("{package_manifest_relative} must define feature bundled"))?;
    if bundled.enables != ["bundled_bindings".to_owned(), "cc".to_owned()] {
        return Err(format!(
            "{package_manifest_relative} feature bundled must enable cc and bundled_bindings"
        ));
    }
    if !feature_definitions
        .iter()
        .any(|feature| feature.name == "bundled_bindings" && feature.enables.is_empty())
    {
        return Err(format!(
            "{package_manifest_relative} must define the bundled_bindings feature"
        ));
    }

    let paths = governed_regular_file_inventory(workspace_root, LOCAL_SQLITE_SOURCE_RELATIVE)?;
    let mut expected_paths = LOCAL_SQLITE_REQUIRED_FILES
        .iter()
        .map(|relative| format!("{LOCAL_SQLITE_SOURCE_RELATIVE}/{relative}"))
        .collect::<Vec<_>>();
    expected_paths.sort();
    if paths != expected_paths {
        return Err(format!(
            "{LOCAL_SQLITE_SOURCE_RELATIVE} governed source inventory drifted: expected {expected_paths:?}, found {paths:?}"
        ));
    }
    let files = paths
        .iter()
        .map(|relative| {
            let bytes = read_regular_file(workspace_root, relative)?;
            Ok(FileDescriptor {
                path: relative.clone(),
                byte_length: byte_length(relative, &bytes)?,
                sha256: sha256_hex(&bytes),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    if files.is_empty() {
        return Err(format!(
            "{LOCAL_SQLITE_SOURCE_RELATIVE} governed source tree must contain files"
        ));
    }
    let tree_bytes = canonical_json_bytes(&files)?;

    Ok(LocalRuntimeSourceDescriptor {
        package: LOCAL_SQLITE_PACKAGE.to_owned(),
        version: LOCAL_SQLITE_VERSION.to_owned(),
        path: LOCAL_SQLITE_SOURCE_RELATIVE.to_owned(),
        patch_registry: "crates-io".to_owned(),
        patch_dependency: LOCAL_SQLITE_PACKAGE.to_owned(),
        activation_route: vec![
            "radroots_event_store/sqlite".to_owned(),
            "sqlx/sqlite-bundled".to_owned(),
            "libsqlite3-sys/bundled".to_owned(),
        ],
        feature_definitions,
        tree_algorithm: LOCAL_SOURCE_TREE_ALGORITHM.to_owned(),
        files,
        tree_sha256: sha256_hex(&tree_bytes),
    })
}

pub(super) fn governed_regular_file_inventory(
    workspace_root: &Path,
    relative_root: &str,
) -> Result<Vec<String>, String> {
    fn visit(
        workspace_root: &Path,
        path: &Path,
        inventory: &mut Vec<String>,
    ) -> Result<(), String> {
        let metadata = fs::symlink_metadata(path).map_err(|error| {
            format!(
                "inspect governed runtime source {}: {error}",
                path.display()
            )
        })?;
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "governed runtime source tree must not contain symlink {}",
                path.display()
            ));
        }
        if metadata.is_file() {
            let relative = path.strip_prefix(workspace_root).map_err(|error| {
                format!(
                    "governed runtime source {} escapes workspace: {error}",
                    path.display()
                )
            })?;
            let relative = relative.to_str().ok_or_else(|| {
                format!(
                    "governed runtime source path must be UTF-8: {}",
                    path.display()
                )
            })?;
            inventory.push(relative.replace('\\', "/"));
            return Ok(());
        }
        if !metadata.is_dir() {
            return Err(format!(
                "governed runtime source tree contains non-file entry {}",
                path.display()
            ));
        }
        let mut entries = fs::read_dir(path)
            .map_err(|error| format!("read governed runtime source {}: {error}", path.display()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("read governed runtime source {}: {error}", path.display()))?;
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries {
            visit(workspace_root, &entry.path(), inventory)?;
        }
        Ok(())
    }

    let root = workspace_root.join(relative_root);
    let mut inventory = Vec::new();
    visit(workspace_root, &root, &mut inventory)?;
    inventory.sort();
    validate_unique_owned("governed runtime source inventory", &inventory)?;
    Ok(inventory)
}

fn validate_governed_support_source_tree_baselines(workspace_root: &Path) -> Result<(), String> {
    for spec in GOVERNED_SUPPORT_SOURCE_TREE_BASELINES {
        let paths = governed_regular_file_inventory(workspace_root, spec.root)?;
        if let Some(relative) = paths.iter().find(|relative| !relative.ends_with(".rs")) {
            return Err(format!(
                "{} governed compiler source tree may contain only Rust source files; found {relative}",
                spec.root
            ));
        }
        let descriptors = paths
            .iter()
            .map(|relative| {
                let bytes = read_regular_file(workspace_root, relative)?;
                let file = parse_canonical_production_rust(relative, &bytes)?;
                validate_support_source_graph_authority(relative, &file)?;
                let canonical = canonical_rust_ast(relative, &bytes, RustAstProfile::Production)?;
                Ok(FileDescriptor {
                    path: relative.clone(),
                    byte_length: byte_length(relative, &canonical)?,
                    sha256: sha256_hex(&canonical),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let actual_sha256 = sha256_hex(&canonical_json_bytes(&descriptors)?);
        if actual_sha256 != spec.sha256 {
            return Err(format!(
                "{} governed production source-tree baseline drifted: expected {}, found {actual_sha256}",
                spec.root, spec.sha256
            ));
        }
    }
    Ok(())
}

fn validate_support_source_graph_authority(relative: &str, file: &syn::File) -> Result<(), String> {
    use syn::visit::Visit;

    struct Audit<'a> {
        relative: &'a str,
        module_paths: Vec<(String, String)>,
        item_macro_count: usize,
        error: Option<String>,
    }

    impl Audit<'_> {
        fn fail(&mut self, message: impl Into<String>) {
            if self.error.is_none() {
                self.error = Some(message.into());
            }
        }
    }

    impl<'ast> Visit<'ast> for Audit<'_> {
        fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
            for attribute in &item.attrs {
                if attribute.path().is_ident("path") {
                    let path = match &attribute.meta {
                        syn::Meta::NameValue(meta) => match &meta.value {
                            syn::Expr::Lit(syn::ExprLit {
                                lit: syn::Lit::Str(path),
                                ..
                            }) => path.value(),
                            _ => {
                                self.fail(format!(
                                    "{} production module `{}` has a non-literal path override",
                                    self.relative, item.ident
                                ));
                                continue;
                            }
                        },
                        _ => {
                            self.fail(format!(
                                "{} production module `{}` has a malformed path override",
                                self.relative, item.ident
                            ));
                            continue;
                        }
                    };
                    self.module_paths
                        .push((normalized_identifier(&item.ident), path));
                } else if attribute.path().is_ident("cfg_attr")
                    && syntax_contains_ident(attribute, "path")
                {
                    self.fail(format!(
                        "{} production module `{}` must not conditionally retarget its source path",
                        self.relative, item.ident
                    ));
                }
            }
            syn::visit::visit_item_mod(self, item);
        }

        fn visit_item_macro(&mut self, item: &'ast syn::ItemMacro) {
            self.item_macro_count += 1;
            syn::visit::visit_item_macro(self, item);
        }
    }

    let expected_module_paths = match relative {
        "crates/core/src/dto.rs" | "crates/event/src/dto.rs" => {
            vec![(
                "generated_roots".to_owned(),
                "generated/dto_roots.rs".to_owned(),
            )]
        }
        _ => Vec::new(),
    };
    let expected_compiler_macro_inputs = match relative {
        "crates/event/src/lib.rs" => vec![r#"env!("CARGO_PKG_VERSION")"#.to_owned()],
        "crates/event_codec/src/manifest.rs" => vec![
            r#"env!("CARGO_PKG_VERSION")"#.to_owned(),
            r#"env!("CARGO_PKG_VERSION")"#.to_owned(),
        ],
        "crates/event/src/wire/v1/tests.rs" => vec![
            r#"include_str!("../../../../../contracts/conformance/vectors/event/nip01_wire.v1.json")"#
                .to_owned(),
        ],
        _ => Vec::new(),
    };

    validate_compiler_macro_inputs(relative, file, &expected_compiler_macro_inputs)?;
    let mut audit = Audit {
        relative,
        module_paths: Vec::new(),
        item_macro_count: 0,
        error: None,
    };
    audit.visit_file(file);
    if let Some(error) = audit.error {
        return Err(error);
    }
    if audit.module_paths != expected_module_paths {
        return Err(format!(
            "{relative} production module source graph drifted: expected {expected_module_paths:?}, found {:?}",
            audit.module_paths
        ));
    }
    if audit.item_macro_count > 0 && !SUPPORT_ITEM_MACRO_SOURCE_ALLOWLIST.contains(&relative) {
        return Err(format!(
            "{relative} introduces unapproved production item-macro authority"
        ));
    }
    Ok(())
}

fn validate_governed_compiler_inputs(workspace_root: &Path) -> Result<(), String> {
    validate_governed_compiler_inputs_with_event_store_successor(workspace_root, None)
}

pub(super) fn validate_raw_source_rebuild_successor_compiler_inputs(
    workspace_root: &Path,
    event_store_compiler_tables_sha256: &str,
) -> Result<(), String> {
    validate_governed_compiler_inputs_with_event_store_successor(
        workspace_root,
        Some(event_store_compiler_tables_sha256),
    )
}

fn validate_governed_compiler_inputs_with_event_store_successor(
    workspace_root: &Path,
    event_store_successor_sha256: Option<&str>,
) -> Result<(), String> {
    let toolchain = parse_cargo_manifest(workspace_root, RUST_TOOLCHAIN_RELATIVE)?;
    let expected_toolchain: toml::Value = toml::from_str(
        r#"
[toolchain]
channel = "1.97.1"
components = ["clippy", "rust-analyzer", "rust-src", "rustfmt"]
targets = ["wasm32-unknown-unknown"]
"#,
    )
    .map_err(|error| format!("parse governed Rust toolchain expectation: {error}"))?;
    if toolchain != expected_toolchain {
        return Err(format!(
            "{RUST_TOOLCHAIN_RELATIVE} must remain the exact governed Rust 1.97.1 toolchain document"
        ));
    }
    for legacy_relative in ["rust-toolchain", ".cargo/config"] {
        let legacy = workspace_root.join(legacy_relative);
        match fs::symlink_metadata(&legacy) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Ok(_) => {
                return Err(format!(
                    "{legacy_relative} legacy compiler configuration must remain absent"
                ));
            }
            Err(error) => {
                return Err(format!(
                    "inspect legacy compiler configuration {}: {error}",
                    legacy.display()
                ));
            }
        }
    }
    let config = parse_cargo_manifest(workspace_root, CARGO_CONFIG_RELATIVE)?;
    let expected_config: toml::Value = toml::from_str(
        r#"
[alias]
xtask = "run -q -p xtask --"
"#,
    )
    .map_err(|error| format!("parse governed Cargo config expectation: {error}"))?;
    if config != expected_config {
        return Err(format!(
            "{CARGO_CONFIG_RELATIVE} must remain the exact benign xtask alias-only compiler configuration"
        ));
    }
    for (relative, expected_sha256) in [
        (MIGRATION_V1_UP_RELATIVE, MIGRATION_V1_UP_SHA256),
        (MIGRATION_V1_DOWN_RELATIVE, MIGRATION_V1_DOWN_SHA256),
    ] {
        let bytes = read_regular_file(workspace_root, relative)?;
        let actual_sha256 = sha256_hex(&bytes);
        if actual_sha256 != expected_sha256 {
            return Err(format!(
                "{relative} embedded compiler input drifted: expected {expected_sha256}, found {actual_sha256}"
            ));
        }
    }

    let governed_packages = [
        (
            CORE_CARGO_MANIFEST_RELATIVE,
            "radroots_core",
            "0.1.0-alpha",
            Some("radroots_core"),
        ),
        (
            EVENT_CARGO_MANIFEST_RELATIVE,
            "radroots_event",
            "0.1.0-alpha",
            Some("radroots_event"),
        ),
        (
            EVENT_CODEC_CARGO_MANIFEST_RELATIVE,
            "radroots_event_codec",
            "0.1.0-alpha",
            Some("radroots_event_codec"),
        ),
        (
            BLOSSOM_CARGO_MANIFEST_RELATIVE,
            "radroots_blossom",
            "0.1.0-alpha",
            Some("radroots_blossom"),
        ),
        (
            EVENT_STORE_CARGO_MANIFEST_RELATIVE,
            "radroots_event_store",
            "0.1.0-alpha",
            None,
        ),
        (
            TRANSPORT_CARGO_MANIFEST_RELATIVE,
            "radroots_transport",
            "0.1.0-alpha",
            Some("radroots_transport"),
        ),
    ];
    let mut actual_identities = Vec::new();
    for (relative, expected_package_name, expected_version, expected_crate_name) in
        governed_packages
    {
        let manifest = parse_cargo_manifest(workspace_root, relative)?;
        let package = manifest
            .get("package")
            .and_then(toml::Value::as_table)
            .ok_or_else(|| format!("{relative} must declare [package]"))?;
        if package.get("name").and_then(toml::Value::as_str) != Some(expected_package_name)
            || package.get("version").and_then(toml::Value::as_str) != Some(expected_version)
            || package
                .get("edition")
                .and_then(toml::Value::as_table)
                .and_then(|value| value.get("workspace"))
                .and_then(toml::Value::as_bool)
                != Some(true)
            || package
                .get("rust-version")
                .and_then(toml::Value::as_table)
                .and_then(|value| value.get("workspace"))
                .and_then(toml::Value::as_bool)
                != Some(true)
        {
            return Err(format!(
                "{relative} compiler package identity must remain {expected_package_name} {expected_version} with workspace edition and rust-version"
            ));
        }
        match (expected_crate_name, manifest.get("lib")) {
            (None, None) => {}
            (Some(expected), Some(lib))
                if lib.as_table().is_some_and(|lib| {
                    lib.len() == 1
                        && lib.get("name").and_then(toml::Value::as_str) == Some(expected)
                }) => {}
            _ => {
                return Err(format!(
                    "{relative} must preserve its exact Rust crate name without target-path authority"
                ));
            }
        }
        if [
            "build",
            "autolib",
            "autobins",
            "autoexamples",
            "autotests",
            "autobenches",
        ]
        .iter()
        .any(|key| package.contains_key(*key))
        {
            return Err(format!(
                "{relative} must not override Cargo target auto-discovery or build-script authority"
            ));
        }
        if [
            "build-dependencies",
            "target",
            "bin",
            "example",
            "test",
            "bench",
        ]
        .iter()
        .any(|key| manifest.get(*key).is_some())
        {
            return Err(format!(
                "{relative} must not introduce build, target-specific dependency, or target-path authority"
            ));
        }
        let package_root = workspace_root
            .join(relative)
            .parent()
            .expect("Cargo manifest has parent")
            .to_path_buf();
        for (label, path) in [
            ("build script", package_root.join("build.rs")),
            ("binary target", package_root.join("src/main.rs")),
            ("binary target directory", package_root.join("src/bin")),
        ] {
            match fs::symlink_metadata(&path) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Ok(_) => {
                    return Err(format!(
                        "{relative} must not have auto-discovered {label} {}",
                        path.display()
                    ));
                }
                Err(error) => {
                    return Err(format!(
                        "inspect governed {label} {}: {error}",
                        path.display()
                    ));
                }
            }
        }
        let mut compiler_tables = BTreeMap::new();
        for key in ["dependencies", "features"] {
            let value = manifest
                .get(key)
                .ok_or_else(|| format!("{relative} must declare [{key}]"))?;
            compiler_tables.insert(key.to_owned(), value.clone());
        }
        if relative == EVENT_STORE_CARGO_MANIFEST_RELATIVE {
            compiler_tables.insert(
                "dev-dependencies".to_owned(),
                manifest
                    .get("dev-dependencies")
                    .ok_or_else(|| {
                        format!(
                            "{EVENT_STORE_CARGO_MANIFEST_RELATIVE} must declare [dev-dependencies]"
                        )
                    })?
                    .clone(),
            );
        }
        actual_identities.push((
            relative.to_owned(),
            sha256_hex(&canonical_json_bytes(&compiler_tables)?),
        ));
    }

    let workspace_manifest =
        parse_cargo_manifest(workspace_root, WORKSPACE_CARGO_MANIFEST_RELATIVE)?;
    let workspace = workspace_manifest
        .get("workspace")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| format!("{WORKSPACE_CARGO_MANIFEST_RELATIVE} must declare [workspace]"))?;
    let workspace_package = workspace
        .get("package")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| {
            format!("{WORKSPACE_CARGO_MANIFEST_RELATIVE} must declare [workspace.package]")
        })?;
    let members = toml_string_array(
        WORKSPACE_CARGO_MANIFEST_RELATIVE,
        workspace.get("members"),
        "workspace.members",
    )?;
    validate_unique_owned("workspace members", &members)?;
    for required in [
        "crates/core",
        "crates/event",
        "crates/event_codec",
        "crates/blossom",
        "crates/event_store",
        "crates/transport",
        "tools/xtask",
    ] {
        if !members.iter().any(|member| member == required) {
            return Err(format!(
                "{WORKSPACE_CARGO_MANIFEST_RELATIVE} workspace.members must include governed package root {required}"
            ));
        }
    }
    let excluded = match workspace.get("exclude") {
        None => Vec::new(),
        value => toml_string_array(
            WORKSPACE_CARGO_MANIFEST_RELATIVE,
            value,
            "workspace.exclude",
        )?,
    };
    validate_unique_owned("workspace exclusions", &excluded)?;
    if excluded != ["fuzz".to_owned()] || workspace.contains_key("default-members") {
        return Err(format!(
            "{WORKSPACE_CARGO_MANIFEST_RELATIVE} must not exclude or narrow the default governed workspace member graph beyond the governed fuzz exclusion"
        ));
    }
    if workspace.get("resolver").and_then(toml::Value::as_str) != Some("3")
        || workspace_package
            .get("edition")
            .and_then(toml::Value::as_str)
            != Some("2024")
        || workspace_package
            .get("rust-version")
            .and_then(toml::Value::as_str)
            != Some("1.97.1")
    {
        return Err(format!(
            "{WORKSPACE_CARGO_MANIFEST_RELATIVE} must retain resolver 3, edition 2024, and rust-version 1.97.1 compiler authority"
        ));
    }
    if workspace_manifest.get("replace").is_some()
        || workspace_manifest.get("profile").is_some()
        || workspace_manifest.get("target").is_some()
    {
        return Err(format!(
            "{WORKSPACE_CARGO_MANIFEST_RELATIVE} must not introduce replacement, profile, or target-specific compiler authority"
        ));
    }
    let workspace_dependencies = workspace
        .get("dependencies")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| {
            format!("{WORKSPACE_CARGO_MANIFEST_RELATIVE} must declare [workspace.dependencies]")
        })?;
    let governed_workspace_dependencies = GOVERNED_WORKSPACE_DEPENDENCY_NAMES
        .iter()
        .map(|name| {
            workspace_dependencies
                .get(*name)
                .cloned()
                .map(|value| ((*name).to_owned(), value))
                .ok_or_else(|| {
                    format!(
                        "{WORKSPACE_CARGO_MANIFEST_RELATIVE} is missing governed workspace dependency {name}"
                    )
                })
        })
        .collect::<Result<BTreeMap<_, _>, String>>()?;
    actual_identities.push((
        "Cargo.toml#governed-workspace-dependencies".to_owned(),
        sha256_hex(&canonical_json_bytes(&governed_workspace_dependencies)?),
    ));
    if workspace_manifest.get("patch").is_some() {
        return Err(format!(
            "{WORKSPACE_CARGO_MANIFEST_RELATIVE} must not override registry sources with [patch]"
        ));
    }

    let expected_identities = GOVERNED_DEPENDENCY_TABLE_SHA256
        .iter()
        .map(|(relative, sha256)| {
            let sha256 = if *relative == EVENT_STORE_CARGO_MANIFEST_RELATIVE {
                event_store_successor_sha256.unwrap_or(sha256)
            } else {
                sha256
            };
            ((*relative).to_owned(), sha256.to_owned())
        })
        .collect::<Vec<_>>();
    if actual_identities != expected_identities {
        return Err(format!(
            "governed Cargo compiler dependency and feature tables drifted: expected {expected_identities:?}, found {actual_identities:?}"
        ));
    }
    Ok(())
}

fn validate_route_facade_baselines(workspace_root: &Path) -> Result<(), String> {
    let descriptors = ROUTE_FACADE_BASELINE_SOURCES
        .iter()
        .map(|relative| {
            let bytes = read_regular_file(workspace_root, relative)?;
            let canonical = canonical_rust_ast(relative, &bytes, RustAstProfile::Production)?;
            Ok(FileDescriptor {
                path: (*relative).to_owned(),
                byte_length: byte_length(relative, &canonical)?,
                sha256: sha256_hex(&canonical),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let actual_sha256 = sha256_hex(&canonical_json_bytes(&descriptors)?);
    if actual_sha256 != ROUTE_FACADE_BASELINE_SHA256 {
        return Err(format!(
            "governed route-facade production baseline drifted: expected {ROUTE_FACADE_BASELINE_SHA256}, found {actual_sha256}"
        ));
    }
    Ok(())
}

fn describe_cargo_feature_profile(
    workspace_root: &Path,
) -> Result<CargoFeatureProfileDescriptor, String> {
    let workspace_manifest =
        parse_cargo_manifest(workspace_root, WORKSPACE_CARGO_MANIFEST_RELATIVE)?;
    let event_store_manifest =
        parse_cargo_manifest(workspace_root, EVENT_STORE_CARGO_MANIFEST_RELATIVE)?;

    let packages = CARGO_PACKAGE_FEATURE_SPECS
        .iter()
        .map(|spec| describe_cargo_package_features(workspace_root, *spec))
        .collect::<Result<Vec<_>, _>>()?;
    let event_store_dependencies = EVENT_STORE_DEPENDENCY_FEATURE_SPECS
        .iter()
        .map(|spec| {
            describe_cargo_dependency_features(
                &workspace_manifest,
                &event_store_manifest,
                spec.name,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    validate_event_store_dependency_profile(&event_store_dependencies)?;
    Ok(CargoFeatureProfileDescriptor {
        packages,
        event_store_dependencies,
    })
}

fn parse_cargo_manifest(workspace_root: &Path, relative: &str) -> Result<toml::Value, String> {
    let bytes = read_regular_file(workspace_root, relative)?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|error| format!("{relative} must be UTF-8 TOML: {error}"))?;
    toml::from_str(text).map_err(|error| format!("parse {relative}: {error}"))
}

fn describe_cargo_package_features(
    workspace_root: &Path,
    spec: CargoPackageFeatureSpec,
) -> Result<CargoPackageFeatureDescriptor, String> {
    let manifest = parse_cargo_manifest(workspace_root, spec.manifest_path)?;
    let package_name = manifest
        .get("package")
        .and_then(toml::Value::as_table)
        .and_then(|package| package.get("name"))
        .and_then(toml::Value::as_str)
        .ok_or_else(|| format!("{} must declare package.name", spec.manifest_path))?;
    if package_name != spec.cargo_package_name {
        return Err(format!(
            "{} package.name must be {}",
            spec.manifest_path, spec.cargo_package_name
        ));
    }
    let feature_table = manifest
        .get("features")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| format!("{} must declare [features]", spec.manifest_path))?;
    let mut feature_definitions = spec
        .relevant_feature_definitions
        .iter()
        .map(|name| {
            let mut enables = toml_string_array(
                spec.manifest_path,
                feature_table.get(*name),
                &format!("features.{name}"),
            )?;
            enables.sort();
            validate_unique_owned(&format!("{} features.{name}", spec.manifest_path), &enables)?;
            validate_required_feature_enables(spec.package, name, &enables)?;
            Ok(CargoFeatureDefinitionDescriptor {
                name: (*name).to_owned(),
                enables,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    feature_definitions.sort();

    let mut selected_features = spec
        .selected_features
        .iter()
        .map(|feature| (*feature).to_owned())
        .collect::<Vec<_>>();
    selected_features.sort();
    validate_unique_owned(
        &format!("{} selected features", spec.manifest_path),
        &selected_features,
    )?;
    for selected in &selected_features {
        if !feature_table.contains_key(selected) {
            return Err(format!(
                "{} selected feature {selected} is not declared",
                spec.manifest_path
            ));
        }
    }

    Ok(CargoPackageFeatureDescriptor {
        package: spec.package.to_owned(),
        manifest_path: spec.manifest_path.to_owned(),
        default_features_enabled: spec.default_features_enabled,
        selected_features,
        feature_definitions,
    })
}

fn validate_required_feature_enables(
    package: &str,
    feature: &str,
    actual: &[String],
) -> Result<(), String> {
    let required: &[&str] = match (package, feature) {
        ("radroots_event_store", "default") => &["runtime-tokio", "sqlite"],
        ("radroots_event_store", "runtime-tokio") => &["sqlx/runtime-tokio"],
        ("radroots_event_store", "sqlite") => &["dep:getrandom", "dep:sqlx", "sqlx/sqlite-bundled"],
        ("radroots_event_codec", "nostr") => &["dep:nostr", "std"],
        ("radroots_event_codec", "serde") => {
            &["dep:serde", "radroots_core/serde", "radroots_event/serde"]
        }
        ("radroots_event_codec", "serde_json") => &["dep:serde_json", "serde"],
        ("radroots_event_codec", "std") => &[
            "radroots_blossom/std",
            "radroots_core/std",
            "radroots_event/std",
        ],
        ("radroots_event", "serde") => &["dep:serde", "radroots_core/serde"],
        ("radroots_event", "std") => {
            &["radroots_blossom/std", "radroots_core/std", "url_nostd/std"]
        }
        ("radroots_core", "default") => &["serde", "std"],
        ("radroots_core", "serde") => &["dep:serde", "rust_decimal/serde"],
        ("radroots_blossom", "std") => &["serde?/std", "sha2/std", "url_nostd/std"],
        _ => &[],
    };
    for required in required {
        if !actual.iter().any(|value| value == required) {
            return Err(format!(
                "{package} feature {feature} must enable {required} for the reconciliation profile"
            ));
        }
    }
    Ok(())
}

fn describe_cargo_dependency_features(
    workspace_manifest: &toml::Value,
    package_manifest: &toml::Value,
    name: &str,
) -> Result<CargoDependencyFeatureDescriptor, String> {
    let workspace_dependency = dependency_table(
        WORKSPACE_CARGO_MANIFEST_RELATIVE,
        workspace_manifest,
        "workspace.dependencies",
        name,
    )?;
    let package_dependency = dependency_table(
        EVENT_STORE_CARGO_MANIFEST_RELATIVE,
        package_manifest,
        "dependencies",
        name,
    )?;
    if package_dependency
        .get("workspace")
        .and_then(toml::Value::as_bool)
        != Some(true)
    {
        return Err(format!(
            "{EVENT_STORE_CARGO_MANIFEST_RELATIVE} dependency {name} must inherit from workspace dependencies"
        ));
    }

    let default_features = package_dependency
        .get("default-features")
        .and_then(toml::Value::as_bool)
        .or_else(|| {
            workspace_dependency
                .get("default-features")
                .and_then(toml::Value::as_bool)
        })
        .unwrap_or(true);
    let optional = package_dependency
        .get("optional")
        .and_then(toml::Value::as_bool)
        .unwrap_or(false);
    let mut features = BTreeSet::new();
    for (relative, table) in [
        (WORKSPACE_CARGO_MANIFEST_RELATIVE, workspace_dependency),
        (EVENT_STORE_CARGO_MANIFEST_RELATIVE, package_dependency),
    ] {
        for feature in toml_string_array(
            relative,
            table.get("features"),
            &format!("dependency {name} features"),
        )? {
            features.insert(feature);
        }
    }

    Ok(CargoDependencyFeatureDescriptor {
        name: name.to_owned(),
        default_features,
        optional,
        features: features.into_iter().collect(),
    })
}

fn dependency_table<'a>(
    relative: &str,
    manifest: &'a toml::Value,
    section: &str,
    name: &str,
) -> Result<&'a toml::value::Table, String> {
    let mut value = manifest;
    for component in section.split('.') {
        value = value
            .get(component)
            .ok_or_else(|| format!("{relative} must declare [{section}]"))?;
    }
    let dependency = value
        .as_table()
        .and_then(|table| table.get(name))
        .ok_or_else(|| format!("{relative} [{section}] must declare {name}"))?;
    dependency
        .as_table()
        .ok_or_else(|| format!("{relative} [{section}] dependency {name} must use table notation"))
}

fn toml_string_array(
    relative: &str,
    value: Option<&toml::Value>,
    label: &str,
) -> Result<Vec<String>, String> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    value
        .as_array()
        .ok_or_else(|| format!("{relative} {label} must be an array"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("{relative} {label} values must be strings"))
        })
        .collect()
}

fn validate_event_store_dependency_profile(
    dependencies: &[CargoDependencyFeatureDescriptor],
) -> Result<(), String> {
    let expected = [
        CargoDependencyFeatureDescriptor {
            name: "getrandom".to_owned(),
            default_features: false,
            optional: true,
            features: vec!["std".to_owned()],
        },
        CargoDependencyFeatureDescriptor {
            name: "radroots_event".to_owned(),
            default_features: false,
            optional: false,
            features: vec!["serde".to_owned(), "std".to_owned()],
        },
        CargoDependencyFeatureDescriptor {
            name: "radroots_event_codec".to_owned(),
            default_features: false,
            optional: false,
            features: vec![
                "nostr".to_owned(),
                "serde_json".to_owned(),
                "std".to_owned(),
            ],
        },
        CargoDependencyFeatureDescriptor {
            name: "sqlx".to_owned(),
            default_features: false,
            optional: true,
            features: vec!["derive".to_owned()],
        },
    ];
    if dependencies != expected {
        return Err(format!(
            "{EVENT_STORE_CARGO_MANIFEST_RELATIVE} reconciliation dependency feature profile has drifted: expected {expected:?}, found {dependencies:?}"
        ));
    }
    Ok(())
}

fn validate_unique_owned(label: &str, values: &[String]) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for value in values {
        if !seen.insert(value.as_str()) {
            return Err(format!("{label} contains duplicate value {value}"));
        }
    }
    Ok(())
}

fn byte_length(relative: &str, bytes: &[u8]) -> Result<u64, String> {
    u64::try_from(bytes.len()).map_err(|_| format!("{relative} byte length does not fit in u64"))
}

fn validate_manifest_shape(
    workspace_root: &Path,
    manifest: &Nip09ReconciliationManifest,
) -> Result<(), String> {
    if manifest.schema_version != SCHEMA_VERSION {
        return Err(format!(
            "{MANIFEST_RELATIVE} schema_version must be {SCHEMA_VERSION}"
        ));
    }
    if manifest.hook_id != HOOK_ID {
        return Err(format!("{MANIFEST_RELATIVE} hook_id must be {HOOK_ID}"));
    }
    let schema_bytes = canonical_json_bytes(&manifest_schema())?;
    let expected_manifest_schema = FileDescriptor {
        path: MANIFEST_SCHEMA_RELATIVE.to_owned(),
        byte_length: byte_length(MANIFEST_SCHEMA_RELATIVE, &schema_bytes)?,
        sha256: sha256_hex(&schema_bytes),
    };
    if manifest.manifest_schema != expected_manifest_schema {
        return Err(format!(
            "{MANIFEST_RELATIVE} manifest_schema must authenticate the exact generated Draft 2020-12 schema"
        ));
    }
    if manifest.migration.version != MIGRATION_VERSION
        || manifest.migration.name != MIGRATION_NAME
        || manifest.migration.schema_sha256 != SCHEMA_SHA256
    {
        return Err(format!(
            "{MANIFEST_RELATIVE} migration identity must match schema v{MIGRATION_VERSION} {MIGRATION_NAME}"
        ));
    }
    if manifest.profile.reconciliation_version != RECONCILIATION_VERSION
        || manifest.profile.addressable_feed_version != ADDRESSABLE_FEED_VERSION
        || manifest.profile.event_contract_registry_version != EVENT_CONTRACT_REGISTRY_VERSION
    {
        return Err(format!(
            "{MANIFEST_RELATIVE} profile versions must be reconciliation={RECONCILIATION_VERSION}, addressable-feed={ADDRESSABLE_FEED_VERSION}, registry={EVENT_CONTRACT_REGISTRY_VERSION}"
        ));
    }
    if manifest.migration.up_byte_length == 0
        || manifest.migration.down_byte_length == 0
        || manifest.manifest_schema.byte_length == 0
        || manifest.registry_inventory.byte_length == 0
        || manifest.result_vector.byte_length == 0
        || manifest.result_vector.executor_canonical_byte_length == 0
        || manifest.local_runtime_sources.is_empty()
        || manifest
            .local_runtime_sources
            .iter()
            .flat_map(|source| source.files.iter())
            .any(|file| file.byte_length == 0)
        || manifest
            .semantic_dependencies
            .iter()
            .any(|dependency| dependency.byte_length == 0)
        || manifest
            .frozen_sources
            .iter()
            .any(|source| source.canonical_byte_length == 0)
    {
        return Err(format!(
            "{MANIFEST_RELATIVE} byte and canonical AST lengths must be positive"
        ));
    }
    if manifest.result_vector.executor_hash_algorithm != RUST_FULL_AST_SHA256_ALGORITHM
        || manifest
            .frozen_sources
            .iter()
            .any(|source| source.hash_algorithm != RUST_PRODUCTION_AST_SHA256_ALGORITHM)
    {
        return Err(format!(
            "{MANIFEST_RELATIVE} Rust source identities must use their exact canonical AST algorithms"
        ));
    }
    for digest in [
        manifest.migration.up_sha256.as_str(),
        manifest.migration.down_sha256.as_str(),
        manifest.migration.schema_sha256.as_str(),
        manifest.manifest_schema.sha256.as_str(),
        manifest.registry_inventory.sha256.as_str(),
        manifest.result_vector.sha256.as_str(),
        manifest.result_vector.executor_sha256.as_str(),
    ]
    .into_iter()
    .chain(
        manifest
            .semantic_dependencies
            .iter()
            .map(|dependency| dependency.sha256.as_str()),
    )
    .chain(
        manifest
            .frozen_sources
            .iter()
            .map(|source| source.sha256.as_str()),
    )
    .chain(
        manifest
            .source_route_witnesses
            .iter()
            .map(|witness| witness.sha256.as_str()),
    )
    .chain(
        manifest
            .rust_item_witnesses
            .iter()
            .filter_map(|witness| witness.ast_sha256.as_deref()),
    )
    .chain(
        manifest
            .rust_fragment_witnesses
            .iter()
            .map(|witness| witness.ast_sha256.as_str()),
    )
    .chain(std::iter::once(
        manifest.impl_resolution_witness.sha256.as_str(),
    ))
    .chain(
        manifest
            .impl_resolution_witness
            .impls
            .iter()
            .map(|item| item.ast_sha256.as_str()),
    )
    .chain([
        manifest
            .post_core_sql_capability
            .extension_ast_sha256
            .as_str(),
        manifest
            .post_core_sql_capability
            .storage_ast_sha256
            .as_str(),
    ])
    .chain(
        manifest
            .post_core_sql_capability
            .statements
            .iter()
            .map(|statement| statement.sql_sha256.as_str()),
    )
    .chain(manifest.local_runtime_sources.iter().flat_map(|source| {
        std::iter::once(source.tree_sha256.as_str())
            .chain(source.files.iter().map(|file| file.sha256.as_str()))
    })) {
        validate_sha256("manifest digest", digest)?;
    }

    validate_cargo_feature_profile_shape(workspace_root, &manifest.cargo_feature_profile)?;

    validate_entry_point_sources(workspace_root)?;
    if manifest.entry_points != expected_entry_points() {
        return Err(format!(
            "{MANIFEST_RELATIVE} entry_points must match the exact frozen entry-point list"
        ));
    }
    validate_unique(
        "entry-point roles",
        manifest
            .entry_points
            .iter()
            .map(|entry| entry.role.as_str()),
    )?;
    validate_unique(
        "entry-point Rust paths",
        manifest
            .entry_points
            .iter()
            .map(|entry| entry.rust_path.as_str()),
    )?;
    if manifest
        .entry_points
        .iter()
        .any(|entry| entry.role.is_empty() || entry.rust_path.is_empty())
    {
        return Err(format!(
            "{MANIFEST_RELATIVE} entry-point roles and Rust paths must be non-empty"
        ));
    }

    validate_unique(
        "semantic dependency ids",
        manifest
            .semantic_dependencies
            .iter()
            .map(|dependency| dependency.id.as_str()),
    )?;
    validate_unique(
        "semantic dependency canonical paths",
        manifest
            .semantic_dependencies
            .iter()
            .map(|dependency| dependency.canonical_path.as_str()),
    )?;
    for dependency in &manifest.semantic_dependencies {
        validate_manifest_input_path(
            workspace_root,
            &dependency.canonical_path,
            "semantic dependency",
        )?;
        if let Some(mirror_path) = dependency.mirror_path.as_deref() {
            validate_manifest_input_path(
                workspace_root,
                mirror_path,
                "semantic dependency mirror",
            )?;
            if mirror_path == dependency.canonical_path {
                return Err(format!(
                    "{MANIFEST_RELATIVE} semantic dependency {} mirror must differ from its canonical path",
                    dependency.id
                ));
            }
        }
        if dependency.executors.is_empty() {
            return Err(format!(
                "{MANIFEST_RELATIVE} semantic dependency {} must name at least one executor",
                dependency.id
            ));
        }
        validate_unique(
            "semantic dependency executors",
            dependency.executors.iter().map(String::as_str),
        )?;
    }

    validate_unique(
        "frozen source roles",
        manifest
            .frozen_sources
            .iter()
            .map(|source| source.role.as_str()),
    )?;
    validate_unique(
        "frozen source paths",
        manifest
            .frozen_sources
            .iter()
            .map(|source| source.path.as_str()),
    )?;
    for source in &manifest.frozen_sources {
        if source.path.contains("/generated/")
            || source.path == RESULT_VECTOR_CANONICAL_RELATIVE
            || source.path == RESULT_VECTOR_MIRROR_RELATIVE
            || source.path == RESULT_VECTOR_EXECUTOR_RELATIVE
        {
            return Err(format!(
                "{MANIFEST_RELATIVE} frozen source {} must not reference a generated or self-describing output",
                source.path
            ));
        }
        validate_manifest_input_path(workspace_root, &source.path, "frozen source")?;
    }

    validate_source_route_witnesses(workspace_root, &manifest.source_route_witnesses)?;
    validate_rust_item_witnesses(workspace_root, &manifest.rust_item_witnesses)?;
    validate_rust_fragment_witnesses(workspace_root, &manifest.rust_fragment_witnesses)?;
    let expected_impl_resolution_witness = describe_impl_resolution_witness(workspace_root)?;
    if manifest.impl_resolution_witness != expected_impl_resolution_witness
        || manifest.impl_resolution_witness.algorithm != IMPL_RESOLUTION_WITNESS_ALGORITHM
        || manifest.impl_resolution_witness.impls.is_empty()
        || manifest.impl_resolution_witness.sha256
            != sha256_hex(&canonical_json_bytes(
                &manifest.impl_resolution_witness.impls,
            )?)
    {
        return Err(format!(
            "{MANIFEST_RELATIVE} impl-resolution witness must match the exact protected NIP-09 v1 type closure"
        ));
    }
    let expected_post_core_sql_capability = describe_post_core_sql_capability(workspace_root)?;
    if manifest.post_core_sql_capability != expected_post_core_sql_capability
        || manifest.post_core_sql_capability.algorithm != POST_CORE_SQL_CAPABILITY_ALGORITHM
    {
        return Err(format!(
            "{MANIFEST_RELATIVE} post-core SQL capability must match the exact authenticated transitive literal-SQL closure"
        ));
    }
    validate_local_runtime_sources(workspace_root, &manifest.local_runtime_sources)?;
    validate_manifest_input_path(
        workspace_root,
        &manifest.registry_inventory.path,
        "registry inventory",
    )?;
    validate_manifest_input_path(
        workspace_root,
        &manifest.result_vector.canonical_path,
        "result vector",
    )?;
    validate_manifest_input_path(
        workspace_root,
        &manifest.result_vector.mirror_path,
        "result vector mirror",
    )?;
    validate_manifest_input_path(
        workspace_root,
        &manifest.result_vector.executor_path,
        "result vector executor",
    )?;
    if manifest.result_vector.executor_id != RESULT_VECTOR_EXECUTOR_ID
        || manifest.result_vector.executor_path != RESULT_VECTOR_EXECUTOR_RELATIVE
        || manifest.result_vector.executor_test != RESULT_VECTOR_EXECUTOR_TEST
    {
        return Err(format!(
            "{MANIFEST_RELATIVE} result-vector executor identity must match the frozen v1 executor"
        ));
    }
    validate_runtime_dependency_profile(
        &manifest.runtime_dependency_policy,
        &manifest.runtime_dependencies,
    )?;
    Ok(())
}

fn validate_cargo_feature_profile_shape(
    workspace_root: &Path,
    profile: &CargoFeatureProfileDescriptor,
) -> Result<(), String> {
    if profile.packages.len() != CARGO_PACKAGE_FEATURE_SPECS.len() {
        return Err(format!(
            "{MANIFEST_RELATIVE} cargo feature profile must contain exactly {} packages",
            CARGO_PACKAGE_FEATURE_SPECS.len()
        ));
    }
    validate_unique(
        "Cargo feature-profile packages",
        profile
            .packages
            .iter()
            .map(|package| package.package.as_str()),
    )?;
    validate_unique(
        "Cargo feature-profile manifest paths",
        profile
            .packages
            .iter()
            .map(|package| package.manifest_path.as_str()),
    )?;
    for (package, spec) in profile.packages.iter().zip(CARGO_PACKAGE_FEATURE_SPECS) {
        let expected_selected = spec
            .selected_features
            .iter()
            .map(|feature| (*feature).to_owned())
            .collect::<BTreeSet<_>>();
        let actual_selected = package
            .selected_features
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let expected_definitions = spec
            .relevant_feature_definitions
            .iter()
            .map(|feature| (*feature).to_owned())
            .collect::<BTreeSet<_>>();
        let actual_definitions = package
            .feature_definitions
            .iter()
            .map(|definition| definition.name.clone())
            .collect::<BTreeSet<_>>();
        if package.package != spec.package
            || package.manifest_path != spec.manifest_path
            || package.default_features_enabled != spec.default_features_enabled
            || actual_selected != expected_selected
            || actual_definitions != expected_definitions
        {
            return Err(format!(
                "{MANIFEST_RELATIVE} Cargo feature profile for {} has drifted",
                spec.package
            ));
        }
        validate_manifest_input_path(
            workspace_root,
            &package.manifest_path,
            "Cargo feature-profile manifest",
        )?;
        if package
            .selected_features
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
            || package
                .feature_definitions
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
        {
            return Err(format!(
                "{MANIFEST_RELATIVE} Cargo feature profile lists must be strictly sorted"
            ));
        }
        for definition in &package.feature_definitions {
            if definition.name.is_empty()
                || definition.enables.iter().any(String::is_empty)
                || definition.enables.windows(2).any(|pair| pair[0] >= pair[1])
            {
                return Err(format!(
                    "{MANIFEST_RELATIVE} Cargo feature definitions must be named and contain non-empty, strictly sorted enables"
                ));
            }
            validate_required_feature_enables(
                &package.package,
                &definition.name,
                &definition.enables,
            )?;
        }
    }
    if profile
        .event_store_dependencies
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(format!(
            "{MANIFEST_RELATIVE} event-store dependency features must be strictly sorted"
        ));
    }
    validate_event_store_dependency_profile(&profile.event_store_dependencies)
}

fn validate_source_route_witnesses(
    workspace_root: &Path,
    witnesses: &[SourceRouteWitnessDescriptor],
) -> Result<(), String> {
    if witnesses.len() != SOURCE_ROUTE_WITNESS_SPECS.len() {
        return Err(format!(
            "{MANIFEST_RELATIVE} source route witness count has drifted"
        ));
    }
    validate_unique(
        "source route witness roles",
        witnesses.iter().map(|witness| witness.role.as_str()),
    )?;
    validate_unique(
        "source route witness paths",
        witnesses.iter().map(|witness| witness.path.as_str()),
    )?;
    for (witness, spec) in witnesses.iter().zip(SOURCE_ROUTE_WITNESS_SPECS) {
        let expected_routes = spec
            .modules
            .iter()
            .map(|module| format!("mod:{}:{}", module.visibility.label(), module.name))
            .chain(spec.uses.iter().map(|use_route| {
                format!("use:{}:{}", use_route.visibility.label(), use_route.path)
            }))
            .collect::<Vec<_>>();
        if witness.role != spec.role
            || witness.path != spec.path
            || witness.routes != expected_routes
        {
            return Err(format!(
                "{MANIFEST_RELATIVE} source route witness {} has drifted",
                spec.role
            ));
        }
        if witness.routes.is_empty() {
            return Err(format!(
                "{MANIFEST_RELATIVE} source route witness {} must contain routes",
                witness.role
            ));
        }
        validate_unique(
            "source route witness routes",
            witness.routes.iter().map(String::as_str),
        )?;
        validate_manifest_input_path(workspace_root, &witness.path, "source route witness")?;
    }
    Ok(())
}

fn validate_rust_item_witnesses(
    workspace_root: &Path,
    witnesses: &[RustItemWitnessDescriptor],
) -> Result<(), String> {
    let expected = describe_rust_item_witnesses(workspace_root)?;
    if witnesses != expected {
        return Err(format!(
            "{MANIFEST_RELATIVE} Rust item witnesses must match the exact protocol-v1 call graph"
        ));
    }
    if witnesses.is_empty() {
        return Err(format!(
            "{MANIFEST_RELATIVE} Rust item witnesses must not be empty"
        ));
    }
    validate_unique(
        "Rust item witness roles",
        witnesses.iter().map(|witness| witness.role.as_str()),
    )?;
    validate_unique(
        "Rust item witness items",
        witnesses.iter().map(|witness| witness.item.as_str()),
    )?;
    for witness in witnesses {
        if witness.role.is_empty() || witness.item.is_empty() {
            return Err(format!(
                "{MANIFEST_RELATIVE} Rust item witness {} has an invalid identity or call sequence",
                witness.role
            ));
        }
        match (witness.binding.as_str(), witness.root, &witness.ast_sha256) {
            ("self_ast", true, Some(_)) | ("ast_closure", _, Some(_)) => {}
            _ => {
                return Err(format!(
                    "{MANIFEST_RELATIVE} Rust item witness {} has an invalid binding/hash combination",
                    witness.role
                ));
            }
        }
        validate_manifest_input_path(workspace_root, &witness.path, "Rust item witness")?;
    }
    Ok(())
}

fn validate_rust_fragment_witnesses(
    workspace_root: &Path,
    witnesses: &[RustFragmentWitnessDescriptor],
) -> Result<(), String> {
    let expected = describe_rust_fragment_witnesses(workspace_root)?;
    if witnesses != expected {
        return Err(format!(
            "{MANIFEST_RELATIVE} Rust fragment witnesses must match the exact migration/runtime routes"
        ));
    }
    if witnesses.is_empty() {
        return Err(format!(
            "{MANIFEST_RELATIVE} Rust fragment witnesses must not be empty"
        ));
    }
    validate_unique(
        "Rust fragment witness roles",
        witnesses.iter().map(|witness| witness.role.as_str()),
    )?;
    validate_unique(
        "Rust fragment witness selectors",
        witnesses.iter().map(|witness| witness.selector.as_str()),
    )?;
    for witness in witnesses {
        if witness.role.is_empty() || witness.selector.is_empty() {
            return Err(format!(
                "{MANIFEST_RELATIVE} Rust fragment witness identities must be non-empty"
            ));
        }
        validate_manifest_input_path(workspace_root, &witness.path, "Rust fragment witness")?;
    }
    Ok(())
}

fn validate_local_runtime_sources(
    workspace_root: &Path,
    sources: &[LocalRuntimeSourceDescriptor],
) -> Result<(), String> {
    let expected = vec![describe_local_sqlite_source(workspace_root)?];
    if sources != expected {
        return Err(format!(
            "{MANIFEST_RELATIVE} local runtime sources must match the exact governed patched SQLite tree"
        ));
    }
    let [source] = sources else {
        return Err(format!(
            "{MANIFEST_RELATIVE} must contain exactly one governed local runtime source"
        ));
    };
    if source.package != LOCAL_SQLITE_PACKAGE
        || source.version != LOCAL_SQLITE_VERSION
        || source.path != LOCAL_SQLITE_SOURCE_RELATIVE
        || source.patch_registry != "crates-io"
        || source.patch_dependency != LOCAL_SQLITE_PACKAGE
        || source.tree_algorithm != LOCAL_SOURCE_TREE_ALGORITHM
        || source.activation_route
            != [
                "radroots_event_store/sqlite",
                "sqlx/sqlite-bundled",
                "libsqlite3-sys/bundled",
            ]
    {
        return Err(format!(
            "{MANIFEST_RELATIVE} governed local SQLite identity or activation route has drifted"
        ));
    }
    if source
        .feature_definitions
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(format!(
            "{MANIFEST_RELATIVE} local runtime source feature definitions must be strictly sorted"
        ));
    }
    if source
        .files
        .windows(2)
        .any(|pair| pair[0].path >= pair[1].path)
    {
        return Err(format!(
            "{MANIFEST_RELATIVE} local runtime source files must be strictly path-sorted"
        ));
    }
    let prefix = format!("{LOCAL_SQLITE_SOURCE_RELATIVE}/");
    for file in &source.files {
        if !file.path.starts_with(&prefix) {
            return Err(format!(
                "{MANIFEST_RELATIVE} local runtime source file {} escapes {}",
                file.path, source.path
            ));
        }
        validate_manifest_input_path(workspace_root, &file.path, "local runtime source file")?;
    }
    let tree_bytes = canonical_json_bytes(&source.files)?;
    if source.tree_sha256 != sha256_hex(&tree_bytes) {
        return Err(format!(
            "{MANIFEST_RELATIVE} local runtime source tree digest is inconsistent"
        ));
    }
    Ok(())
}

fn validate_runtime_dependency_profile(
    policy: &RuntimeDependencyPolicyDescriptor,
    dependencies: &[RuntimeDependencyDescriptor],
) -> Result<(), String> {
    if policy.algorithm != RUNTIME_DEPENDENCY_ALGORITHM {
        return Err(format!(
            "{MANIFEST_RELATIVE} runtime dependency algorithm must be {RUNTIME_DEPENDENCY_ALGORITHM}"
        ));
    }
    if dependencies.is_empty() || dependencies.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(format!(
            "{MANIFEST_RELATIVE} runtime dependencies must be non-empty and strictly sorted"
        ));
    }
    let mut identities = BTreeSet::new();
    for dependency in dependencies {
        Version::parse(&dependency.version).map_err(|error| {
            format!(
                "{MANIFEST_RELATIVE} runtime dependency {} has an invalid version: {error}",
                dependency.name
            )
        })?;
        validate_immutable_cargo_source(
            &dependency.name,
            &dependency.version,
            &dependency.source,
            dependency.checksum.as_deref(),
        )?;
        let identity = RuntimeDependencyIdentityDescriptor {
            name: dependency.name.clone(),
            version: dependency.version.clone(),
            source: dependency.source.clone(),
        };
        if !identities.insert(identity) {
            return Err(format!(
                "{MANIFEST_RELATIVE} runtime dependency identities must be unique"
            ));
        }
        if dependency
            .dependencies
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(format!(
                "{MANIFEST_RELATIVE} runtime dependency edges must be strictly sorted"
            ));
        }
    }
    for dependency in dependencies {
        for edge in &dependency.dependencies {
            if !identities.contains(edge) {
                return Err(format!(
                    "{MANIFEST_RELATIVE} runtime dependency edge {} -> {} leaves the recorded subgraph",
                    dependency.name, edge.name
                ));
            }
        }
    }

    if policy.roots.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(format!(
            "{MANIFEST_RELATIVE} runtime dependency roots must be strictly sorted"
        ));
    }
    let expected_roots = RUNTIME_DEPENDENCY_ROOTS
        .iter()
        .map(|root| (root.owner, root.name))
        .collect::<BTreeSet<_>>();
    let actual_roots = policy
        .roots
        .iter()
        .map(|root| (root.owner.as_str(), root.name.as_str()))
        .collect::<BTreeSet<_>>();
    if actual_roots != expected_roots {
        return Err(format!(
            "{MANIFEST_RELATIVE} runtime dependency roots have drifted"
        ));
    }
    for root in &policy.roots {
        let spec = RUNTIME_DEPENDENCY_ROOTS
            .iter()
            .find(|spec| spec.owner == root.owner && spec.name == root.name)
            .ok_or_else(|| {
                format!(
                    "{MANIFEST_RELATIVE} runtime dependency root {}/{} is not governed",
                    root.owner, root.name
                )
            })?;
        if spec
            .expected_version
            .is_some_and(|expected| root.version != expected)
        {
            return Err(format!(
                "{MANIFEST_RELATIVE} runtime dependency root {}/{} must use version {}",
                root.owner,
                root.name,
                spec.expected_version.expect("checked expected version")
            ));
        }
        let identity = RuntimeDependencyIdentityDescriptor {
            name: root.name.clone(),
            version: root.version.clone(),
            source: root.source.clone(),
        };
        if !identities.contains(&identity) {
            return Err(format!(
                "{MANIFEST_RELATIVE} runtime dependency root {} is absent from the subgraph",
                root.name
            ));
        }
    }

    if !policy.exclusions.is_empty() {
        return Err(format!(
            "{MANIFEST_RELATIVE} runtime dependency policy must not exclude direct reconciliation dependencies"
        ));
    }
    for required in RUNTIME_DEPENDENCY_ROOTS
        .iter()
        .map(|root| root.name)
        .chain(["libsqlite3-sys", "secp256k1", "secp256k1-sys"])
    {
        if !dependencies
            .iter()
            .any(|dependency| dependency.name == required)
        {
            return Err(format!(
                "{MANIFEST_RELATIVE} runtime dependency closure must include {required}"
            ));
        }
    }
    Ok(())
}

fn validate_manifest_input_path(
    workspace_root: &Path,
    relative: &str,
    label: &str,
) -> Result<(), String> {
    let generated_outputs = [
        MANIFEST_RELATIVE,
        MANIFEST_SCHEMA_RELATIVE,
        MANIFEST_SHA256_RELATIVE,
        GENERATED_DESCRIPTOR_RELATIVE,
    ];
    if relative.contains("/generated/") || generated_outputs.contains(&relative) {
        return Err(format!(
            "{MANIFEST_RELATIVE} {label} must not reference generated output {relative}"
        ));
    }
    validate_workspace_path(workspace_root, relative, false).map(|_| ())
}

fn validate_result_vector(vector: &ReconciliationResultVector) -> Result<(), String> {
    if vector.schema_version != SCHEMA_VERSION || vector.hook_id != HOOK_ID {
        return Err(format!(
            "{RESULT_VECTOR_CANONICAL_RELATIVE} must identify schema v{SCHEMA_VERSION} and hook {HOOK_ID}"
        ));
    }
    if vector.cases.is_empty() {
        return Err(format!(
            "{RESULT_VECTOR_CANONICAL_RELATIVE} must contain at least one case"
        ));
    }
    validate_unique(
        "result vector case ids",
        vector.cases.iter().map(|case| case.id.as_str()),
    )?;
    for case in &vector.cases {
        if case.id.is_empty() || case.input_events.is_empty() {
            return Err(format!(
                "{RESULT_VECTOR_CANONICAL_RELATIVE} cases require non-empty ids and input events"
            ));
        }
        validate_hex(
            "result vector source generation",
            &case.source_generation_hex,
            64,
        )?;
        if case.expected.raw_event_count
            != u64::try_from(case.input_events.len())
                .map_err(|_| "result-vector input count does not fit u64".to_owned())?
        {
            return Err(format!(
                "{RESULT_VECTOR_CANONICAL_RELATIVE} case {} raw_event_count must equal input event count",
                case.id
            ));
        }
        for (field, value) in [
            ("raw_event_count", case.expected.raw_event_count),
            ("coordinate_count", case.expected.coordinate_count),
            ("request_count", case.expected.request_count),
            ("event_target_count", case.expected.event_target_count),
            ("address_target_count", case.expected.address_target_count),
            ("transition_count", case.expected.transition_count),
        ] {
            if value > SQLITE_I64_MAX_U64 {
                return Err(format!(
                    "{RESULT_VECTOR_CANONICAL_RELATIVE} case {} expected {field} exceeds SQLite i64 range",
                    case.id
                ));
            }
        }
        for observed in &case.input_events {
            if observed.observed_at_ms < 0 {
                return Err(format!(
                    "{RESULT_VECTOR_CANONICAL_RELATIVE} case {} has a negative observation timestamp",
                    case.id
                ));
            }
            if observed.event.created_at > SQLITE_I64_MAX_U64 {
                return Err(format!(
                    "{RESULT_VECTOR_CANONICAL_RELATIVE} case {} event created_at exceeds SQLite i64 range",
                    case.id
                ));
            }
            validate_signed_event(&case.id, &observed.event)?;
        }
        let state = &case.expected.state;
        validate_hex("expected state pubkey", &state.pubkey, 64)?;
        validate_hex("expected state raw head id", &state.raw_head_event_id, 64)?;
        for value in [
            state.event_reference_request_id.0.as_deref(),
            state.address_reference_request_id.0.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            validate_hex("expected state deletion request id", value, 64)?;
        }
        if !matches!(
            state.admission_status.as_str(),
            "admitted" | "unsupported" | "invalid"
        ) {
            return Err(format!(
                "{RESULT_VECTOR_CANONICAL_RELATIVE} case {} has an invalid admission status",
                case.id
            ));
        }
        if state.contract_id.is_empty()
            || state.nip09_outcome.is_empty()
            || state.nip09_reason.is_empty()
        {
            return Err(format!(
                "{RESULT_VECTOR_CANONICAL_RELATIVE} case {} must assert contract and NIP-09 state",
                case.id
            ));
        }
        if state
            .address_reference_cutoff
            .0
            .is_some_and(|cutoff| cutoff > SQLITE_I64_MAX_U64)
        {
            return Err(format!(
                "{RESULT_VECTOR_CANONICAL_RELATIVE} case {} address deletion cutoff exceeds SQLite i64 range",
                case.id
            ));
        }
        if !matches!(state.visibility.as_str(), "visible" | "suppressed") {
            return Err(format!(
                "{RESULT_VECTOR_CANONICAL_RELATIVE} case {} has an invalid visibility",
                case.id
            ));
        }
    }
    Ok(())
}

fn validate_signed_event(case_id: &str, event: &SignedEvent) -> Result<(), String> {
    validate_hex("signed event id", &event.id, 64)?;
    validate_hex("signed event pubkey", &event.pubkey, 64)?;
    validate_hex("signed event signature", &event.sig, 128)?;
    if event.tags.iter().any(Vec::is_empty) {
        return Err(format!(
            "{RESULT_VECTOR_CANONICAL_RELATIVE} case {case_id} contains an empty NIP-01 tag"
        ));
    }
    Ok(())
}

fn runtime_dependencies_from_lock(
    lock_bytes: &[u8],
) -> Result<
    (
        RuntimeDependencyPolicyDescriptor,
        Vec<RuntimeDependencyDescriptor>,
    ),
    String,
> {
    let lock_text = std::str::from_utf8(lock_bytes)
        .map_err(|error| format!("{CARGO_LOCK_RELATIVE} must be UTF-8: {error}"))?;
    let lock: CargoLock = toml::from_str(lock_text)
        .map_err(|error| format!("parse {CARGO_LOCK_RELATIVE}: {error}"))?;
    if lock.package.is_empty() {
        return Err(format!("{CARGO_LOCK_RELATIVE} contains no packages"));
    }

    let mut identities = BTreeSet::new();
    let mut by_name: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (index, package) in lock.package.iter().enumerate() {
        let identity = (
            package.name.as_str(),
            package.version.as_str(),
            package.source.as_deref(),
        );
        if !identities.insert(identity) {
            return Err(format!(
                "{CARGO_LOCK_RELATIVE} contains duplicate package identity {} {} {:?}",
                package.name, package.version, package.source
            ));
        }
        by_name
            .entry(package.name.as_str())
            .or_default()
            .push(index);
    }

    let mut roots = Vec::new();
    let mut queue = VecDeque::new();
    for root in RUNTIME_DEPENDENCY_ROOTS {
        let index = resolve_direct_lock_dependency(&lock.package, &by_name, root.owner, root.name)?;
        let identity = runtime_dependency_identity(&lock.package[index])?;
        if root
            .expected_version
            .is_some_and(|expected| identity.version != expected)
        {
            return Err(format!(
                "{CARGO_LOCK_RELATIVE} direct semantic dependency {}/{} must resolve to version {}, found {}",
                root.owner,
                root.name,
                root.expected_version.expect("checked expected version"),
                identity.version
            ));
        }
        roots.push(RuntimeDependencyRootDescriptor {
            owner: root.owner.to_owned(),
            name: identity.name,
            version: identity.version,
            source: identity.source,
        });
        queue.push_back(index);
    }
    roots.sort();
    if roots.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(format!(
            "{CARGO_LOCK_RELATIVE} semantic dependency roots must be unique"
        ));
    }

    let exclusions = Vec::new();

    let mut visited = BTreeSet::new();
    let mut dependencies = Vec::new();
    while let Some(index) = queue.pop_front() {
        if !visited.insert(index) {
            continue;
        }
        let package = &lock.package[index];
        let identity = runtime_dependency_identity(package)?;
        let mut edges = BTreeSet::new();
        for raw_dependency in &package.dependencies {
            let dependency = parse_lock_dependency(raw_dependency)?;
            let dependency_index = resolve_lock_dependency(&lock.package, &by_name, &dependency)?;
            let dependency_identity = runtime_dependency_identity(&lock.package[dependency_index])?;
            edges.insert(dependency_identity);
            queue.push_back(dependency_index);
        }
        dependencies.push(RuntimeDependencyDescriptor {
            name: identity.name,
            version: identity.version,
            source: identity.source,
            checksum: package.checksum.clone(),
            dependencies: edges.into_iter().collect(),
        });
    }

    dependencies.sort();
    if dependencies.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(format!(
            "{CARGO_LOCK_RELATIVE} semantic dependency subgraph contains duplicate nodes"
        ));
    }
    for required in ["libsqlite3-sys", "secp256k1", "secp256k1-sys"] {
        if !dependencies
            .iter()
            .any(|dependency| dependency.name == required)
        {
            return Err(format!(
                "{CARGO_LOCK_RELATIVE} semantic dependency closure is missing {required}"
            ));
        }
    }
    Ok((
        RuntimeDependencyPolicyDescriptor {
            algorithm: RUNTIME_DEPENDENCY_ALGORITHM.to_owned(),
            roots,
            exclusions,
        },
        dependencies,
    ))
}

fn resolve_direct_lock_dependency(
    packages: &[CargoLockPackage],
    by_name: &BTreeMap<&str, Vec<usize>>,
    owner: &str,
    dependency_name: &str,
) -> Result<usize, String> {
    let owner_index = resolve_lock_dependency(
        packages,
        by_name,
        &CargoLockDependency {
            name: owner.to_owned(),
            version: None,
            source: None,
        },
    )?;
    if packages[owner_index].source.is_some() {
        return Err(format!(
            "{CARGO_LOCK_RELATIVE} semantic dependency owner {owner} must be a local package"
        ));
    }
    let matches = packages[owner_index]
        .dependencies
        .iter()
        .map(|raw| parse_lock_dependency(raw))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|dependency| dependency.name == dependency_name)
        .collect::<Vec<_>>();
    let [dependency] = matches.as_slice() else {
        return Err(format!(
            "{CARGO_LOCK_RELATIVE} local package {owner} must have exactly one direct dependency edge to {dependency_name}"
        ));
    };
    resolve_lock_dependency(packages, by_name, dependency)
}

fn runtime_dependency_identity(
    package: &CargoLockPackage,
) -> Result<RuntimeDependencyIdentityDescriptor, String> {
    Version::parse(&package.version).map_err(|error| {
        format!(
            "{CARGO_LOCK_RELATIVE} package {} has invalid version {}: {error}",
            package.name, package.version
        )
    })?;
    let source = match package.source.as_deref() {
        Some(source) => source,
        None if package.name == LOCAL_SQLITE_PACKAGE && package.version == LOCAL_SQLITE_VERSION => {
            LOCAL_SQLITE_LOCK_SOURCE
        }
        None => {
            return Err(format!(
                "{CARGO_LOCK_RELATIVE} semantic dependency subgraph contains ungoverned local package {} {}",
                package.name, package.version
            ));
        }
    };
    validate_immutable_cargo_source(
        &package.name,
        &package.version,
        source,
        package.checksum.as_deref(),
    )?;
    Ok(RuntimeDependencyIdentityDescriptor {
        name: package.name.clone(),
        version: package.version.clone(),
        source: source.to_owned(),
    })
}

fn validate_immutable_cargo_source(
    name: &str,
    version: &str,
    source: &str,
    checksum: Option<&str>,
) -> Result<(), String> {
    if source
        .chars()
        .any(|character| character.is_control() || character.is_whitespace())
    {
        return Err(format!(
            "{CARGO_LOCK_RELATIVE} package {name} {version} has an invalid source identity"
        ));
    }
    if source == LOCAL_SQLITE_LOCK_SOURCE {
        if name != LOCAL_SQLITE_PACKAGE || version != LOCAL_SQLITE_VERSION || checksum.is_some() {
            return Err(format!(
                "{CARGO_LOCK_RELATIVE} governed local runtime source identity must be {LOCAL_SQLITE_PACKAGE} {LOCAL_SQLITE_VERSION} without a registry checksum"
            ));
        }
        return Ok(());
    }
    if let Some(registry) = source.strip_prefix("registry+") {
        if !registry.starts_with("https://") {
            return Err(format!(
                "{CARGO_LOCK_RELATIVE} registry package {name} {version} must use an HTTPS registry identity"
            ));
        }
        let checksum = checksum.ok_or_else(|| {
            format!("{CARGO_LOCK_RELATIVE} registry package {name} {version} is missing a checksum")
        })?;
        validate_sha256("Cargo.lock registry checksum", checksum)?;
        return Ok(());
    }
    if let Some(git) = source.strip_prefix("git+") {
        let (repository, revision) = git.rsplit_once('#').ok_or_else(|| {
            format!("{CARGO_LOCK_RELATIVE} git package {name} {version} must pin an exact revision")
        })?;
        if !(repository.starts_with("https://") || repository.starts_with("ssh://"))
            || !matches!(revision.len(), 40 | 64)
            || !revision
                .as_bytes()
                .iter()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
        {
            return Err(format!(
                "{CARGO_LOCK_RELATIVE} git package {name} {version} has a non-immutable source identity"
            ));
        }
        return Ok(());
    }
    Err(format!(
        "{CARGO_LOCK_RELATIVE} package {name} {version} uses unsupported source identity {source}"
    ))
}

fn parse_lock_dependency(raw: &str) -> Result<CargoLockDependency, String> {
    let (identity, source) = if raw.ends_with(')') {
        let start = raw.rfind(" (").ok_or_else(|| {
            format!("{CARGO_LOCK_RELATIVE} dependency has malformed source: {raw}")
        })?;
        (
            &raw[..start],
            Some(raw[start + 2..raw.len() - 1].to_owned()),
        )
    } else {
        (raw, None)
    };
    let mut components = identity.split_whitespace();
    let name = components
        .next()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{CARGO_LOCK_RELATIVE} contains an empty dependency"))?;
    let version = components.next().map(str::to_owned);
    if components.next().is_some() {
        return Err(format!(
            "{CARGO_LOCK_RELATIVE} dependency has malformed identity: {raw}"
        ));
    }
    if let Some(version) = version.as_deref() {
        Version::parse(version).map_err(|error| {
            format!("{CARGO_LOCK_RELATIVE} dependency {raw} has invalid version: {error}")
        })?;
    }
    Ok(CargoLockDependency {
        name: name.to_owned(),
        version,
        source,
    })
}

fn resolve_lock_dependency(
    packages: &[CargoLockPackage],
    by_name: &BTreeMap<&str, Vec<usize>>,
    dependency: &CargoLockDependency,
) -> Result<usize, String> {
    let candidates = by_name
        .get(dependency.name.as_str())
        .into_iter()
        .flat_map(|indices| indices.iter().copied())
        .filter(|index| {
            let package = &packages[*index];
            dependency
                .version
                .as_deref()
                .is_none_or(|version| package.version == version)
                && dependency
                    .source
                    .as_deref()
                    .is_none_or(|source| package.source.as_deref() == Some(source))
        })
        .collect::<Vec<_>>();
    match candidates.as_slice() {
        [index] => Ok(*index),
        [] => Err(format!(
            "{CARGO_LOCK_RELATIVE} cannot resolve dependency {}{}{}",
            dependency.name,
            dependency
                .version
                .as_deref()
                .map(|version| format!(" {version}"))
                .unwrap_or_default(),
            dependency
                .source
                .as_deref()
                .map(|source| format!(" ({source})"))
                .unwrap_or_default()
        )),
        _ => Err(format!(
            "{CARGO_LOCK_RELATIVE} dependency {} is ambiguous; lockfile dependency identities must disambiguate version and source",
            dependency.name
        )),
    }
}

fn generated_descriptor(
    manifest: &Nip09ReconciliationManifest,
    manifest_bytes: &[u8],
    manifest_sha256: &str,
) -> String {
    let manifest_json = std::str::from_utf8(manifest_bytes)
        .expect("canonical JSON serialization always produces UTF-8");
    let manifest_literal = format!("{manifest_json:?}");
    format!(
        "// @generated by `cargo xtask contract nip09-reconciliation-manifest --write`; do not edit.\n\
#![allow(dead_code)]\n\n\
pub(crate) const NIP09_RECONCILIATION_MANIFEST_JSON: &str = {manifest_literal};\n\
pub(crate) const NIP09_RECONCILIATION_MANIFEST_BYTE_LENGTH: usize = {};\n\
pub(crate) const NIP09_RECONCILIATION_MANIFEST_SHA256: &str =\n    \"{manifest_sha256}\";\n\
pub(crate) const NIP09_RECONCILIATION_MANIFEST_SCHEMA_VERSION: u32 = {};\n\
pub(crate) const NIP09_RECONCILIATION_HOOK_ID: &str = \"{}\";\n\
pub(crate) const NIP09_RECONCILIATION_MIGRATION_VERSION: u32 = {};\n\
pub(crate) const NIP09_RECONCILIATION_MIGRATION_NAME: &str = \"{}\";\n\
pub(crate) const NIP09_RECONCILIATION_VERSION: i64 = {};\n\
pub(crate) const NIP09_RECONCILIATION_ADDRESSABLE_FEED_VERSION: i64 = {};\n\
pub(crate) const NIP09_RECONCILIATION_EVENT_CONTRACT_REGISTRY_VERSION: u32 = {};\n\
pub(crate) const NIP09_RECONCILIATION_MIGRATION_UP_BYTE_LENGTH: usize = {};\n\
pub(crate) const NIP09_RECONCILIATION_MIGRATION_UP_SHA256: &str =\n    \"{}\";\n\
pub(crate) const NIP09_RECONCILIATION_MIGRATION_DOWN_BYTE_LENGTH: usize = {};\n\
pub(crate) const NIP09_RECONCILIATION_MIGRATION_DOWN_SHA256: &str =\n    \"{}\";\n\
pub(crate) const NIP09_RECONCILIATION_SCHEMA_SHA256: &str =\n    \"{}\";\n\
pub(crate) const NIP09_RECONCILIATION_RESULT_VECTOR_SHA256: &str =\n    \"{}\";\n\
pub(crate) const NIP09_RECONCILIATION_RESULT_VECTOR_EXECUTOR_ID: &str =\n    \"{}\";\n\
pub(crate) const NIP09_RECONCILIATION_RESULT_VECTOR_EXECUTOR_SHA256: &str =\n    \"{}\";\n",
        manifest_bytes.len(),
        manifest.schema_version,
        manifest.hook_id,
        manifest.migration.version,
        manifest.migration.name,
        manifest.profile.reconciliation_version,
        manifest.profile.addressable_feed_version,
        manifest.profile.event_contract_registry_version,
        manifest.migration.up_byte_length,
        manifest.migration.up_sha256,
        manifest.migration.down_byte_length,
        manifest.migration.down_sha256,
        manifest.migration.schema_sha256,
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
    let runtime_identity = json!({
        "type": "object",
        "required": ["name", "version", "source"],
        "properties": {
            "name": {"type": "string", "minLength": 1},
            "version": {
                "type": "string",
                "pattern": "^[0-9]+\\.[0-9]+\\.[0-9]+(?:-[0-9A-Za-z.-]+)?(?:\\+[0-9A-Za-z.-]+)?$"
            },
            "source": {"type": "string", "minLength": 1}
        },
        "additionalProperties": false
    });
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://radroots.org/core/event-store/nip09-reconciliation-manifest-v1.schema.json",
        "title": "Radroots event-store NIP-09 reconciliation manifest v1",
        "type": "object",
        "required": [
            "schema_version",
            "hook_id",
            "manifest_schema",
            "migration",
            "profile",
            "cargo_feature_profile",
            "entry_points",
            "registry_inventory",
            "semantic_dependencies",
            "runtime_dependency_policy",
            "runtime_dependencies",
            "local_runtime_sources",
            "frozen_sources",
            "source_route_witnesses",
            "rust_item_witnesses",
            "rust_fragment_witnesses",
            "impl_resolution_witness",
            "post_core_sql_capability",
            "result_vector"
        ],
        "properties": {
            "schema_version": {"const": SCHEMA_VERSION},
            "hook_id": {"const": HOOK_ID},
            "manifest_schema": {
                "type": "object",
                "required": ["path", "byte_length", "sha256"],
                "properties": {
                    "path": {"const": MANIFEST_SCHEMA_RELATIVE},
                    "byte_length": {"type": "integer", "minimum": 1},
                    "sha256": hash.clone()
                },
                "additionalProperties": false
            },
            "migration": {
                "type": "object",
                "required": [
                    "version",
                    "name",
                    "up_byte_length",
                    "up_sha256",
                    "down_byte_length",
                    "down_sha256",
                    "schema_sha256"
                ],
                "properties": {
                    "version": {"const": MIGRATION_VERSION},
                    "name": {"const": MIGRATION_NAME},
                    "up_byte_length": {"type": "integer", "minimum": 1},
                    "up_sha256": hash.clone(),
                    "down_byte_length": {"type": "integer", "minimum": 1},
                    "down_sha256": hash.clone(),
                    "schema_sha256": {"const": SCHEMA_SHA256}
                },
                "additionalProperties": false
            },
            "profile": {
                "type": "object",
                "required": [
                    "reconciliation_version",
                    "addressable_feed_version",
                    "event_contract_registry_version"
                ],
                "properties": {
                    "reconciliation_version": {"const": RECONCILIATION_VERSION},
                    "addressable_feed_version": {"const": ADDRESSABLE_FEED_VERSION},
                    "event_contract_registry_version": {"const": EVENT_CONTRACT_REGISTRY_VERSION}
                },
                "additionalProperties": false
            },
            "cargo_feature_profile": {
                "type": "object",
                "required": ["packages", "event_store_dependencies"],
                "properties": {
                    "packages": {
                        "type": "array",
                        "minItems": 1,
                        "uniqueItems": true,
                        "items": {
                            "type": "object",
                            "required": [
                                "package",
                                "manifest_path",
                                "default_features_enabled",
                                "selected_features",
                                "feature_definitions"
                            ],
                            "properties": {
                                "package": {"type": "string", "minLength": 1},
                                "manifest_path": path.clone(),
                                "default_features_enabled": {"type": "boolean"},
                                "selected_features": {
                                    "type": "array",
                                    "minItems": 1,
                                    "uniqueItems": true,
                                    "items": {"type": "string", "minLength": 1}
                                },
                                "feature_definitions": {
                                    "type": "array",
                                    "minItems": 1,
                                    "uniqueItems": true,
                                    "items": {
                                        "type": "object",
                                        "required": ["name", "enables"],
                                        "properties": {
                                            "name": {"type": "string", "minLength": 1},
                                            "enables": {
                                                "type": "array",
                                                "uniqueItems": true,
                                                "items": {"type": "string", "minLength": 1}
                                            }
                                        },
                                        "additionalProperties": false
                                    }
                                }
                            },
                            "additionalProperties": false
                        }
                    },
                    "event_store_dependencies": {
                        "type": "array",
                        "minItems": 1,
                        "uniqueItems": true,
                        "items": {
                            "type": "object",
                            "required": ["name", "default_features", "optional", "features"],
                            "properties": {
                                "name": {"type": "string", "minLength": 1},
                                "default_features": {"type": "boolean"},
                                "optional": {"type": "boolean"},
                                "features": {
                                    "type": "array",
                                    "uniqueItems": true,
                                    "items": {"type": "string", "minLength": 1}
                                }
                            },
                            "additionalProperties": false
                        }
                    }
                },
                "additionalProperties": false
            },
            "entry_points": {
                "type": "array",
                "minItems": 1,
                "uniqueItems": true,
                "items": {
                    "type": "object",
                    "required": ["role", "rust_path"],
                    "properties": {
                        "role": {"type": "string", "minLength": 1},
                        "rust_path": {"type": "string", "minLength": 1}
                    },
                    "additionalProperties": false
                }
            },
            "registry_inventory": {
                "type": "object",
                "required": ["path", "byte_length", "sha256"],
                "properties": {
                    "path": path.clone(),
                    "byte_length": {"type": "integer", "minimum": 1},
                    "sha256": hash.clone()
                },
                "additionalProperties": false
            },
            "semantic_dependencies": {
                "type": "array",
                "minItems": 1,
                "uniqueItems": true,
                "items": {
                    "type": "object",
                    "required": [
                        "id",
                        "canonical_path",
                        "mirror_path",
                        "byte_length",
                        "sha256",
                        "executors"
                    ],
                    "properties": {
                        "id": {"type": "string", "minLength": 1},
                        "canonical_path": path.clone(),
                        "mirror_path": {
                            "anyOf": [
                                path.clone(),
                                {"type": "null"}
                            ]
                        },
                        "byte_length": {"type": "integer", "minimum": 1},
                        "sha256": hash.clone(),
                        "executors": {
                            "type": "array",
                            "minItems": 1,
                            "uniqueItems": true,
                            "items": {"type": "string", "minLength": 1}
                        }
                    },
                    "additionalProperties": false
                }
            },
            "runtime_dependency_policy": {
                "type": "object",
                "required": ["algorithm", "roots", "exclusions"],
                "properties": {
                    "algorithm": {"const": RUNTIME_DEPENDENCY_ALGORITHM},
                    "roots": {
                        "type": "array",
                        "minItems": 1,
                        "uniqueItems": true,
                        "items": {
                            "type": "object",
                            "required": ["owner", "name", "version", "source"],
                            "properties": {
                                "owner": {"type": "string", "minLength": 1},
                                "name": {"type": "string", "minLength": 1},
                                "version": {
                                    "type": "string",
                                    "pattern": "^[0-9]+\\.[0-9]+\\.[0-9]+(?:-[0-9A-Za-z.-]+)?(?:\\+[0-9A-Za-z.-]+)?$"
                                },
                                "source": {"type": "string", "minLength": 1}
                            },
                            "additionalProperties": false
                        }
                    },
                    "exclusions": {
                        "type": "array",
                        "uniqueItems": true,
                        "items": {
                            "type": "object",
                            "required": ["owner", "name", "reason", "bound_by"],
                            "properties": {
                                "owner": {"type": "string", "minLength": 1},
                                "name": {"type": "string", "minLength": 1},
                                "reason": {"type": "string", "minLength": 1},
                                "bound_by": {
                                    "type": "array",
                                    "minItems": 1,
                                    "uniqueItems": true,
                                    "items": {"type": "string", "minLength": 1}
                                }
                            },
                            "additionalProperties": false
                        }
                    }
                },
                "additionalProperties": false
            },
            "runtime_dependencies": {
                "type": "array",
                "minItems": 1,
                "uniqueItems": true,
                "items": {
                    "type": "object",
                    "required": ["name", "version", "source", "checksum", "dependencies"],
                    "properties": {
                        "name": {"type": "string", "minLength": 1},
                        "version": {
                            "type": "string",
                            "pattern": "^[0-9]+\\.[0-9]+\\.[0-9]+(?:-[0-9A-Za-z.-]+)?(?:\\+[0-9A-Za-z.-]+)?$"
                        },
                        "source": {"type": "string", "minLength": 1},
                        "checksum": {
                            "anyOf": [
                                hash.clone(),
                                {"type": "null"}
                            ]
                        },
                        "dependencies": {
                            "type": "array",
                            "uniqueItems": true,
                            "items": runtime_identity
                        }
                    },
                    "additionalProperties": false
                }
            },
            "local_runtime_sources": {
                "type": "array",
                "minItems": 1,
                "maxItems": 1,
                "uniqueItems": true,
                "items": {
                    "type": "object",
                    "required": [
                        "package",
                        "version",
                        "path",
                        "patch_registry",
                        "patch_dependency",
                        "activation_route",
                        "feature_definitions",
                        "tree_algorithm",
                        "files",
                        "tree_sha256"
                    ],
                    "properties": {
                        "package": {"const": LOCAL_SQLITE_PACKAGE},
                        "version": {"const": LOCAL_SQLITE_VERSION},
                        "path": {"const": LOCAL_SQLITE_SOURCE_RELATIVE},
                        "patch_registry": {"const": "crates-io"},
                        "patch_dependency": {"const": LOCAL_SQLITE_PACKAGE},
                        "activation_route": {
                            "type": "array",
                            "minItems": 3,
                            "maxItems": 3,
                            "uniqueItems": true,
                            "items": {"type": "string", "minLength": 1}
                        },
                        "feature_definitions": {
                            "type": "array",
                            "minItems": 1,
                            "uniqueItems": true,
                            "items": {
                                "type": "object",
                                "required": ["name", "enables"],
                                "properties": {
                                    "name": {"type": "string", "minLength": 1},
                                    "enables": {
                                        "type": "array",
                                        "uniqueItems": true,
                                        "items": {"type": "string", "minLength": 1}
                                    }
                                },
                                "additionalProperties": false
                            }
                        },
                        "tree_algorithm": {"const": LOCAL_SOURCE_TREE_ALGORITHM},
                        "files": {
                            "type": "array",
                            "minItems": LOCAL_SQLITE_REQUIRED_FILES.len(),
                            "maxItems": LOCAL_SQLITE_REQUIRED_FILES.len(),
                            "uniqueItems": true,
                            "items": {
                                "type": "object",
                                "required": ["path", "byte_length", "sha256"],
                                "properties": {
                                    "path": path.clone(),
                                    "byte_length": {"type": "integer", "minimum": 1},
                                    "sha256": hash.clone()
                                },
                                "additionalProperties": false
                            }
                        },
                        "tree_sha256": hash.clone()
                    },
                    "additionalProperties": false
                }
            },
            "frozen_sources": {
                "type": "array",
                "minItems": 1,
                "uniqueItems": true,
                "items": {
                    "type": "object",
                    "required": [
                        "role",
                        "path",
                        "hash_algorithm",
                        "canonical_byte_length",
                        "sha256"
                    ],
                    "properties": {
                        "role": {"type": "string", "minLength": 1},
                        "path": path.clone(),
                        "hash_algorithm": {
                            "const": RUST_PRODUCTION_AST_SHA256_ALGORITHM
                        },
                        "canonical_byte_length": {"type": "integer", "minimum": 1},
                        "sha256": hash.clone()
                    },
                    "additionalProperties": false
                }
            },
            "source_route_witnesses": {
                "type": "array",
                "minItems": 1,
                "uniqueItems": true,
                "items": {
                    "type": "object",
                    "required": ["role", "path", "routes", "sha256"],
                    "properties": {
                        "role": {"type": "string", "minLength": 1},
                        "path": path.clone(),
                        "routes": {
                            "type": "array",
                            "minItems": 1,
                            "uniqueItems": true,
                            "items": {"type": "string", "minLength": 1}
                        },
                        "sha256": hash.clone()
                    },
                    "additionalProperties": false
                }
            },
            "rust_item_witnesses": {
                "type": "array",
                "minItems": 1,
                "uniqueItems": true,
                "items": {
                    "type": "object",
                    "required": [
                        "role",
                        "path",
                        "item",
                        "root",
                        "binding",
                        "local_call_sequence",
                        "required_call_sequence",
                        "ast_sha256"
                    ],
                    "properties": {
                        "role": {"type": "string", "minLength": 1},
                        "path": path.clone(),
                        "item": {"type": "string", "minLength": 1},
                        "root": {"type": "boolean"},
                        "binding": {"enum": ["self_ast", "ast_closure"]},
                        "local_call_sequence": {
                            "type": "array",
                            "items": {"type": "string", "minLength": 1}
                        },
                        "required_call_sequence": {
                            "type": "array",
                            "items": {"type": "string", "minLength": 1}
                        },
                        "ast_sha256": {
                            "anyOf": [
                                hash.clone(),
                                {"type": "null"}
                            ]
                        }
                    },
                    "additionalProperties": false
                }
            },
            "rust_fragment_witnesses": {
                "type": "array",
                "minItems": 1,
                "uniqueItems": true,
                "items": {
                    "type": "object",
                    "required": ["role", "path", "selector", "ast_sha256"],
                    "properties": {
                        "role": {"type": "string", "minLength": 1},
                        "path": path.clone(),
                        "selector": {"type": "string", "minLength": 1},
                        "ast_sha256": hash.clone()
                    },
                    "additionalProperties": false
                }
            },
            "impl_resolution_witness": {
                "type": "object",
                "required": [
                    "algorithm",
                    "roots",
                    "protected_self_types",
                    "impls",
                    "sha256"
                ],
                "properties": {
                    "algorithm": {"const": IMPL_RESOLUTION_WITNESS_ALGORITHM},
                    "roots": {
                        "const": IMPL_RESOLUTION_SOURCE_ROOTS
                    },
                    "protected_self_types": {
                        "type": "array",
                        "minItems": 1,
                        "uniqueItems": true,
                        "items": {"type": "string", "minLength": 1}
                    },
                    "impls": {
                        "type": "array",
                        "minItems": 1,
                        "uniqueItems": true,
                        "items": {
                            "type": "object",
                            "required": [
                                "path",
                                "self_type",
                                "trait_path",
                                "member",
                                "impl_header_sha256",
                                "ast_sha256"
                            ],
                            "properties": {
                                "path": path.clone(),
                                "self_type": {"type": "string", "minLength": 1},
                                "trait_path": {
                                    "anyOf": [
                                        {"type": "string", "minLength": 1},
                                        {"type": "null"}
                                    ]
                                },
                                "member": {
                                    "anyOf": [
                                        {"type": "string", "minLength": 1},
                                        {"type": "null"}
                                    ]
                                },
                                "impl_header_sha256": hash.clone(),
                                "ast_sha256": hash.clone()
                            },
                            "additionalProperties": false
                        }
                    },
                    "sha256": hash.clone()
                },
                "additionalProperties": false
            },
            "post_core_sql_capability": {
                "type": "object",
                "required": [
                    "algorithm",
                    "capabilities_path",
                    "capability_type",
                    "capability_struct_ast_sha256",
                    "capability_constructor_ast_sha256",
                    "capability_v1_method_ast_sha256",
                    "dispatcher_path",
                    "dispatcher_root",
                    "dispatcher_signature_sha256",
                    "dispatcher_v1_prefix_sha256",
                    "extension_path",
                    "extension_ast_sha256",
                    "storage_path",
                    "storage_ast_sha256",
                    "root",
                    "storage_methods",
                    "statements",
                    "allowed_capabilities",
                    "forbidden_classes"
                ],
                "properties": {
                    "algorithm": {"const": POST_CORE_SQL_CAPABILITY_ALGORITHM},
                    "capabilities_path": {
                        "const": POST_CORE_CAPABILITIES_SOURCE_RELATIVE
                    },
                    "capability_type": {
                        "const": "PostCoreExtensionCapabilities"
                    },
                    "capability_struct_ast_sha256": hash.clone(),
                    "capability_constructor_ast_sha256": hash.clone(),
                    "capability_v1_method_ast_sha256": hash.clone(),
                    "dispatcher_path": {
                        "const": POST_CORE_DISPATCHER_SOURCE_RELATIVE
                    },
                    "dispatcher_root": {
                        "const": "dispatch_post_core_extensions"
                    },
                    "dispatcher_signature_sha256": hash.clone(),
                    "dispatcher_v1_prefix_sha256": hash.clone(),
                    "extension_path": {"const": POST_CORE_EXTENSION_SOURCE_RELATIVE},
                    "extension_ast_sha256": hash.clone(),
                    "storage_path": {"const": POST_CORE_STORAGE_SOURCE_RELATIVE},
                    "storage_ast_sha256": hash.clone(),
                    "root": {"const": POST_CORE_EXTENSION_ROOT},
                    "storage_methods": {"const": POST_CORE_STORAGE_METHODS},
                    "statements": {
                        "type": "array",
                        "minItems": 1,
                        "uniqueItems": true,
                        "items": {
                            "type": "object",
                            "required": [
                                "function",
                                "operation",
                                "tables",
                                "terminal",
                                "sql_sha256",
                                "placeholder_count",
                                "bind_expressions"
                            ],
                            "properties": {
                                "function": {"type": "string", "minLength": 1},
                                "operation": {
                                    "enum": ["delete", "insert", "select", "upsert"]
                                },
                                "tables": {
                                    "type": "array",
                                    "minItems": 1,
                                    "uniqueItems": true,
                                    "items": {"type": "string", "minLength": 1}
                                },
                                "terminal": {
                                    "enum": [
                                        "execute",
                                        "fetch",
                                        "fetch_all",
                                        "fetch_many",
                                        "fetch_one",
                                        "fetch_optional"
                                    ]
                                },
                                "sql_sha256": hash.clone(),
                                "placeholder_count": {
                                    "type": "integer",
                                    "minimum": 0
                                },
                                "bind_expressions": {
                                    "type": "array",
                                    "items": {"type": "string", "minLength": 1}
                                }
                            },
                            "additionalProperties": false
                        }
                    },
                    "allowed_capabilities": {
                        "type": "array",
                        "minItems": 1,
                        "uniqueItems": true,
                        "items": {
                            "type": "object",
                            "required": ["operation", "table"],
                            "properties": {
                                "operation": {
                                    "enum": ["delete", "insert", "select", "upsert"]
                                },
                                "table": {"type": "string", "minLength": 1}
                            },
                            "additionalProperties": false
                        }
                    },
                    "forbidden_classes": {
                        "type": "array",
                        "minItems": 1,
                        "uniqueItems": true,
                        "items": {"type": "string", "minLength": 1}
                    }
                },
                "additionalProperties": false
            },
            "result_vector": {
                "type": "object",
                "required": [
                    "canonical_path",
                    "mirror_path",
                    "byte_length",
                    "sha256",
                    "executor_id",
                    "executor_path",
                    "executor_test",
                    "executor_hash_algorithm",
                    "executor_canonical_byte_length",
                    "executor_sha256"
                ],
                "properties": {
                    "canonical_path": path.clone(),
                    "mirror_path": path.clone(),
                    "byte_length": {"type": "integer", "minimum": 1},
                    "sha256": hash.clone(),
                    "executor_id": {"const": RESULT_VECTOR_EXECUTOR_ID},
                    "executor_path": {"const": RESULT_VECTOR_EXECUTOR_RELATIVE},
                    "executor_test": {"const": RESULT_VECTOR_EXECUTOR_TEST},
                    "executor_hash_algorithm": {
                        "const": RUST_FULL_AST_SHA256_ALGORITHM
                    },
                    "executor_canonical_byte_length": {
                        "type": "integer",
                        "minimum": 1
                    },
                    "executor_sha256": hash
                },
                "additionalProperties": false
            }
        },
        "additionalProperties": false
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
    let expected = canonical_json_bytes(value)?;
    if actual != expected {
        return Err(format!(
            "{relative} must use canonical two-space JSON formatting and end with exactly one LF"
        ));
    }
    Ok(())
}

fn validate_unique<'a>(label: &str, values: impl Iterator<Item = &'a str>) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for value in values {
        if !seen.insert(value) {
            return Err(format!(
                "{MANIFEST_RELATIVE} contains duplicate {label}: {value}"
            ));
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

    const RAW_SOURCE_REBUILD_PREDECESSOR_SUPERSEDED_PATHS: [&str; 16] = [
        "Cargo.toml",
        "crates/blossom/Cargo.toml",
        "crates/blossom/src/error.rs",
        "crates/blossom/src/lib.rs",
        "crates/blossom/src/url.rs",
        "crates/event_store/Cargo.toml",
        "crates/event_store/src/error.rs",
        "crates/event_store/src/generated.rs",
        "crates/event_store/src/lib.rs",
        "crates/event_store/src/migrations.rs",
        "crates/event_store/src/model.rs",
        "crates/event_store/src/nip09/reconciliation_v1.rs",
        "crates/event_store/src/schema.rs",
        "crates/event_store/src/store.rs",
        "crates/event_store/src/store/food_availability_projection_v1.rs",
        "crates/event_store/src/store/protocol_reconciliation_v1.rs",
    ];

    fn repository_root() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("xtask manifest has a workspace root")
            .to_path_buf()
    }

    fn repository_source(relative: &str) -> String {
        fs::read_to_string(repository_root().join(relative)).expect("repository Rust source")
    }

    fn immutable_manifest() -> Nip09ReconciliationManifest {
        serde_json::from_slice(IMMUTABLE_MANIFEST_BYTES).expect("immutable NIP-09 manifest")
    }

    fn restore_predecessor_compiler_manifest(workspace_root: &Path) {
        let manifest_path = workspace_root.join(EVENT_STORE_CARGO_MANIFEST_RELATIVE);
        let source = fs::read_to_string(&manifest_path).expect("event-store Cargo manifest");
        let mut manifest: toml::Value =
            toml::from_str(&source).expect("parse event-store Cargo manifest");
        let dependencies = manifest
            .get_mut("dependencies")
            .and_then(toml::Value::as_table_mut)
            .expect("event-store dependencies");
        let futures = dependencies
            .remove("futures")
            .expect("RawSourceRebuild futures compiler edge must be present in the live fixture");
        let expected_futures: toml::Value =
            toml::from_str("dependency = { workspace = true, optional = true }")
                .expect("parse expected futures dependency");
        assert_eq!(
            futures,
            expected_futures
                .get("dependency")
                .expect("expected futures dependency")
                .clone(),
            "RawSourceRebuild futures compiler edge must retain its exact semantic shape"
        );
        let blossom = dependencies
            .remove("radroots_blossom")
            .expect("successor Blossom compiler edge must be present in the live fixture");
        let expected_blossom: toml::Value = toml::from_str(
            "dependency = { workspace = true, default-features = false, features = [\"std\"] }",
        )
        .expect("parse expected Blossom dependency");
        assert_eq!(
            blossom,
            expected_blossom
                .get("dependency")
                .expect("expected Blossom dependency")
                .clone(),
            "successor Blossom compiler edge must retain its exact semantic shape"
        );
        let sqlite_features = manifest
            .get_mut("features")
            .and_then(toml::Value::as_table_mut)
            .and_then(|features| features.get_mut("sqlite"))
            .and_then(toml::Value::as_array_mut)
            .expect("event-store sqlite features");
        let futures_index = sqlite_features
            .iter()
            .position(|feature| feature.as_str() == Some("dep:futures"))
            .expect("RawSourceRebuild futures feature edge must be present in the live fixture");
        sqlite_features.remove(futures_index);
        let tokio_features = manifest
            .get_mut("dev-dependencies")
            .and_then(toml::Value::as_table_mut)
            .and_then(|dependencies| dependencies.get_mut("tokio"))
            .and_then(toml::Value::as_table_mut)
            .and_then(|tokio| tokio.get_mut("features"))
            .and_then(toml::Value::as_array_mut)
            .expect("event-store Tokio development features");
        assert_eq!(
            tokio_features
                .iter()
                .map(toml::Value::as_str)
                .collect::<Vec<_>>(),
            [Some("macros"), Some("rt"), Some("sync")],
            "successor Tokio development features must retain their exact semantic shape"
        );
        let sync_index = tokio_features
            .iter()
            .position(|feature| feature.as_str() == Some("sync"))
            .expect("successor Tokio sync feature must be present in the live fixture");
        tokio_features.remove(sync_index);
        assert_eq!(
            tokio_features
                .iter()
                .map(toml::Value::as_str)
                .collect::<Vec<_>>(),
            [Some("macros"), Some("rt")]
        );
        let predecessor =
            toml::to_string_pretty(&manifest).expect("serialize predecessor Cargo manifest");
        fs::write(manifest_path, predecessor).expect("restore predecessor compiler manifest");

        let codec_manifest_path = workspace_root.join(EVENT_CODEC_CARGO_MANIFEST_RELATIVE);
        let codec_source =
            fs::read_to_string(&codec_manifest_path).expect("event-codec Cargo manifest");
        let mut codec_manifest: toml::Value =
            toml::from_str(&codec_source).expect("parse event-codec Cargo manifest");
        let serde_json_features = codec_manifest
            .get_mut("features")
            .and_then(toml::Value::as_table_mut)
            .and_then(|features| features.get_mut("serde_json"))
            .and_then(toml::Value::as_array_mut)
            .expect("event-codec serde_json feature");
        assert_eq!(
            serde_json_features
                .iter()
                .map(toml::Value::as_str)
                .collect::<Vec<_>>(),
            [
                Some("serde"),
                Some("dep:hex"),
                Some("dep:serde_json"),
                Some("dep:sha2"),
                Some("radroots_blossom/serde"),
            ],
            "publication successor feature edges must retain their exact semantic shape"
        );
        serde_json_features
            .retain(|feature| !matches!(feature.as_str(), Some("dep:hex" | "dep:sha2")));
        let codec_predecessor = toml::to_string_pretty(&codec_manifest)
            .expect("serialize predecessor event-codec Cargo manifest");
        fs::write(codec_manifest_path, codec_predecessor)
            .expect("restore predecessor event-codec compiler manifest");

        let blossom_path = workspace_root.join(BLOSSOM_CARGO_MANIFEST_RELATIVE);
        let blossom_source = fs::read_to_string(&blossom_path).expect("Blossom Cargo manifest");
        let mut blossom_manifest: toml::Value =
            toml::from_str(&blossom_source).expect("parse Blossom Cargo manifest");
        let raster_decode = blossom_manifest
            .get_mut("features")
            .and_then(toml::Value::as_table_mut)
            .and_then(|features| features.remove("raster-decode"))
            .expect("successor raster-decode feature must be present in the live fixture");
        assert_eq!(
            raster_decode
                .as_array()
                .expect("raster-decode feature array")
                .iter()
                .map(toml::Value::as_str)
                .collect::<Vec<_>>(),
            [
                Some("std"),
                Some("dep:image"),
                Some("dep:libwebp"),
                Some("dep:zune-core"),
                Some("dep:zune-jpeg")
            ],
            "successor raster-decode feature must retain its exact semantic shape"
        );
        let blossom_dependencies = blossom_manifest
            .get_mut("dependencies")
            .and_then(toml::Value::as_table_mut)
            .expect("Blossom dependencies");
        for dependency in ["image", "libwebp", "zune-core", "zune-jpeg"] {
            let removed = blossom_dependencies
                .remove(dependency)
                .unwrap_or_else(|| panic!("successor dependency {dependency} must be present"));
            let expected: toml::Value =
                toml::from_str("dependency = { workspace = true, optional = true }")
                    .expect("parse expected successor dependency");
            assert_eq!(
                removed,
                expected
                    .get("dependency")
                    .expect("expected successor dependency")
                    .clone(),
                "successor dependency {dependency} must retain its exact semantic shape"
            );
        }
        let blossom_predecessor = toml::to_string_pretty(&blossom_manifest)
            .expect("serialize predecessor Blossom Cargo manifest");
        fs::write(blossom_path, blossom_predecessor)
            .expect("restore predecessor Blossom compiler manifest");
    }

    fn strip_outer_try(statement: &mut syn::Stmt) {
        let expression = match statement {
            syn::Stmt::Expr(expression, _) => expression,
            syn::Stmt::Local(local) => local
                .init
                .as_mut()
                .map(|init| init.expr.as_mut())
                .expect("expected initialized local statement"),
            syn::Stmt::Item(_) | syn::Stmt::Macro(_) => {
                panic!("expected expression or initialized local statement")
            }
        };
        let syn::Expr::Try(try_expression) = expression else {
            panic!("expected try expression");
        };
        *expression = (*try_expression.expr).clone();
    }

    fn copy_file(source_root: &Path, destination_root: &Path, relative: &str) {
        let destination = destination_root.join(relative);
        fs::create_dir_all(destination.parent().expect("fixture parent"))
            .expect("create fixture parent");
        fs::copy(source_root.join(relative), destination).expect("copy fixture");
    }

    fn manifest_input_paths() -> Vec<&'static str> {
        let mut paths = vec![
            CARGO_CONFIG_RELATIVE,
            RUST_TOOLCHAIN_RELATIVE,
            CARGO_LOCK_RELATIVE,
            WORKSPACE_CARGO_MANIFEST_RELATIVE,
            EVENT_STORE_CARGO_MANIFEST_RELATIVE,
            EVENT_CODEC_CARGO_MANIFEST_RELATIVE,
            EVENT_CARGO_MANIFEST_RELATIVE,
            CORE_CARGO_MANIFEST_RELATIVE,
            BLOSSOM_CARGO_MANIFEST_RELATIVE,
            TRANSPORT_CARGO_MANIFEST_RELATIVE,
            EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
            EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
            EVENT_STORE_STORE_SOURCE_RELATIVE,
            POST_CORE_CAPABILITIES_SOURCE_RELATIVE,
            POST_CORE_DISPATCHER_SOURCE_RELATIVE,
            POST_CORE_EXTENSION_SOURCE_RELATIVE,
            POST_CORE_STORAGE_SOURCE_RELATIVE,
            MIGRATION_V1_UP_RELATIVE,
            MIGRATION_V1_DOWN_RELATIVE,
            MIGRATION_UP_RELATIVE,
            MIGRATION_DOWN_RELATIVE,
            "crates/event_store/migrations/0004_source_maintenance.up.sql",
            "crates/event_store/migrations/0004_source_maintenance.down.sql",
            REGISTRY_INVENTORY_RELATIVE,
            "contracts/event_store/event_contract_registry_v7.inventory.sha256",
            RESULT_VECTOR_CANONICAL_RELATIVE,
            RESULT_VECTOR_MIRROR_RELATIVE,
            RESULT_VECTOR_EXECUTOR_RELATIVE,
            MANIFEST_RELATIVE,
            MANIFEST_SCHEMA_RELATIVE,
            MANIFEST_SHA256_RELATIVE,
            GENERATED_DESCRIPTOR_RELATIVE,
            "contracts/conformance/vectors/event_store/source_maintenance.v1.json",
            "crates/event_store/tests/fixtures/source_maintenance.v1.json",
            "crates/event_store/tests/source_maintenance_v1_result_vector.rs",
            "crates/event_store/contracts/source_maintenance_v1.manifest.json",
            "crates/event_store/contracts/source_maintenance_v1.manifest.schema.json",
            "crates/event_store/contracts/source_maintenance_v1.manifest.sha256",
            "crates/event_store/src/generated/source_maintenance_manifest.rs",
        ];
        for dependency in SEMANTIC_DEPENDENCY_SPECS {
            paths.push(dependency.canonical_path);
            if let Some(mirror) = dependency.mirror_path {
                paths.push(mirror);
            }
        }
        paths.extend(FROZEN_SOURCE_SPECS.iter().map(|source| source.path));
        paths.extend(SOURCE_ROUTE_WITNESS_SPECS.iter().map(|source| source.path));
        paths.extend(SUCCESSOR_08C_EXCLUSIVE_SOURCE_PATHS);
        paths.extend(SUCCESSOR_08D_SOURCE_PATHS);
        paths.extend(SUCCESSOR_08D1_EXCLUSIVE_SOURCE_PATHS);
        paths.extend(super::super::source_maintenance::source_contract_fixture_source_paths());
        paths.sort_unstable();
        paths.dedup();
        paths
    }

    fn synthetic_workspace() -> tempfile::TempDir {
        let workspace = tempfile::TempDir::new().expect("workspace");
        let repository = repository_root();
        for relative in manifest_input_paths() {
            copy_file(&repository, workspace.path(), relative);
        }
        for spec in GOVERNED_SUPPORT_SOURCE_TREE_BASELINES {
            for relative in governed_regular_file_inventory(&repository, spec.root)
                .expect("governed support source inventory")
            {
                copy_file(&repository, workspace.path(), &relative);
            }
        }
        workspace
    }

    #[test]
    fn production_rust_identity_is_semantic_and_cfg_test_neutral() {
        let base = br#"
/// baseline documentation
fn stable(
    value: u32,
) -> u32 {
    value + 1
}

#[cfg(test)]
fn test_probe() -> u32 {
    1
}

#[cfg(any(test, feature = "sqlite"))]
fn mixed_probe() -> u32 {
    7
}
"#;
        let formatting_and_test_edit = br#"
// ordinary comment
/// revised documentation
fn stable(value: u32) -> u32 { value + 1 }

#[cfg(test)]
fn test_probe() -> u32 {
    999
}

#[cfg(feature = "sqlite")]
fn mixed_probe() -> u32 {
    7
}
"#;
        let base_production =
            canonical_rust_ast("base.rs", base, RustAstProfile::Production).expect("base AST");
        let edited_production = canonical_rust_ast(
            "edited.rs",
            formatting_and_test_edit,
            RustAstProfile::Production,
        )
        .expect("edited AST");
        assert_eq!(
            edited_production, base_production,
            "comments, docs, rustfmt trailing punctuation, cfg(test), and equivalent mixed cfg must not rotate production identity"
        );

        let production_edit = String::from_utf8(base.to_vec())
            .expect("UTF-8 fixture")
            .replacen("value + 1", "value + 2", 1);
        assert_ne!(
            canonical_rust_ast(
                "production_edit.rs",
                production_edit.as_bytes(),
                RustAstProfile::Production,
            )
            .expect("production edit AST"),
            base_production,
            "production behavior must rotate production identity"
        );

        let base_full =
            canonical_rust_ast("base.rs", base, RustAstProfile::Full).expect("base full AST");
        let full_test_edit = String::from_utf8(base.to_vec())
            .expect("UTF-8 fixture")
            .replacen("    1\n}", "    2\n}", 1);
        assert_ne!(
            canonical_rust_ast(
                "full_test_edit.rs",
                full_test_edit.as_bytes(),
                RustAstProfile::Full,
            )
            .expect("full test edit AST"),
            base_full,
            "full executor identity must rotate for cfg(test) behavior changes"
        );

        let opaque_test_cfg = b"fn f() { opaque!(#[cfg(test)] value); }\n";
        let error = canonical_rust_ast("opaque.rs", opaque_test_cfg, RustAstProfile::Production)
            .expect_err("opaque pure-test macro fragments must fail closed");
        assert!(error.contains("cfg") || error.contains("test"), "{error}");
    }

    #[test]
    fn raw_identifier_normalization_preserves_keywords_and_canonicalizes_nonkeywords() {
        let keyword = br#"
struct KeywordField {
    r#type: u32,
}
"#;
        let reformatted_keyword = br#"struct KeywordField { r#type: u32 }"#;
        let keyword_identity =
            canonical_rust_ast("keyword.rs", keyword, RustAstProfile::Production)
                .expect("raw-keyword identity");
        assert_eq!(
            canonical_rust_ast(
                "keyword_reformatted.rs",
                reformatted_keyword,
                RustAstProfile::Production,
            )
            .expect("reformatted raw-keyword identity"),
            keyword_identity,
        );
        assert!(
            std::str::from_utf8(&keyword_identity)
                .expect("canonical raw-keyword UTF-8")
                .contains("r#type"),
            "a raw keyword must remain parseable in the canonical AST"
        );

        let ordinary = br#"
fn hex() {}
macro_rules! route { ($name:ident) => {}; }
route!(hex);
"#;
        let raw_nonkeyword = br#"
fn r#hex() {}
macro_rules! route { ($name:ident) => {}; }
route!(r#hex);
"#;
        assert_eq!(
            canonical_rust_ast("ordinary.rs", ordinary, RustAstProfile::Full)
                .expect("ordinary identifier identity"),
            canonical_rust_ast("raw_nonkeyword.rs", raw_nonkeyword, RustAstProfile::Full)
                .expect("raw nonkeyword identity"),
            "raw syntax must not bypass semantic comparisons for nonkeyword identifiers"
        );
    }

    #[test]
    fn unchanged_predecessor_authority_drift_is_rejected() {
        let manifest: Nip09ReconciliationManifest =
            serde_json::from_slice(IMMUTABLE_MANIFEST_BYTES).expect("immutable manifest");
        let spec = FROZEN_SOURCE_SPECS
            .iter()
            .copied()
            .find(|spec| spec.path == "crates/event/src/deletion.rs")
            .expect("unchanged predecessor authority spec");
        let expected = manifest
            .frozen_sources
            .iter()
            .find(|source| source.path == spec.path)
            .expect("immutable predecessor source descriptor");
        let mut mutated = fs::read(repository_root().join(spec.path)).expect("authority source");
        mutated.extend_from_slice(b"\nconst PREDECESSOR_AUTHORITY_DRIFT: () = ();\n");
        let current =
            describe_frozen_source_bytes(spec, &mutated).expect("mutated production AST identity");
        let error = require_predecessor_frozen_source_match(expected, &current)
            .expect_err("production authority drift must fail");
        assert!(
            error.contains("unchanged predecessor frozen-source authority"),
            "{error}"
        );
    }

    #[test]
    fn privileged_store_authority_rejects_retargets_conditionals_and_bypass_calls() {
        let workspace = synthetic_workspace();
        validate_privileged_store_authority(workspace.path())
            .expect("baseline privileged store authority");
        let error_path = workspace.path().join("crates/event_store/src/error.rs");
        let error_source = fs::read_to_string(&error_path).expect("event-store error source");
        fs::write(
            &error_path,
            format!(
                "{error_source}\nimpl Drop for crate::error::RadrootsEventStoreError {{\n    fn drop(&mut self) {{ std::process::abort(); }}\n}}\n"
            ),
        )
        .expect("write existing-source implicit trait injection");
        let error = validate_privileged_store_authority(workspace.path())
            .expect_err("existing-source implicit trait injection must fail");
        assert!(
            error.contains("event-store trait impl authority drifted"),
            "{error}"
        );
        fs::write(&error_path, &error_source).expect("restore event-store error source");

        let migrations_path = workspace
            .path()
            .join(EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE);
        let migrations_source =
            fs::read_to_string(&migrations_path).expect("event-store migrations source");
        fs::write(
            &migrations_path,
            format!(
                "{migrations_source}\nimpl crate::store::RadrootsEventStore {{\n    pub async fn bypass(&self) {{\n        let _ = sqlx::query(\"DELETE FROM event_envelopes\").execute(self.pool()).await;\n    }}\n}}\n"
            ),
        )
        .expect("write migrations inherent authority injection");
        let error = validate_privileged_store_authority(workspace.path())
            .expect_err("migrations inherent authority injection must fail");
        assert!(
            error.contains("event-store inherent impl authority drifted"),
            "{error}"
        );
        fs::write(&migrations_path, migrations_source)
            .expect("restore event-store migrations source");

        let schema_path = workspace.path().join(EVENT_STORE_SCHEMA_SOURCE_RELATIVE);
        let schema_source = fs::read_to_string(&schema_path).expect("event-store schema source");
        fs::write(
            &schema_path,
            format!(
                "{schema_source}\nmacro_rules! inject_drop {{ () => {{\n    impl Drop for crate::model::RadrootsEventIngestReceipt {{\n        fn drop(&mut self) {{ std::process::abort(); }}\n    }}\n}} }}\ninject_drop!();\n"
            ),
        )
        .expect("write item-macro impl injection");
        let error = validate_privileged_store_authority(workspace.path())
            .expect_err("item-macro impl injection must fail");
        assert!(error.contains("item macro authority is closed"), "{error}");
        fs::write(&schema_path, &schema_source).expect("restore event-store schema source");

        let escaped_source_path = workspace.path().join("crates/event_store/evil.rs");
        fs::write(
            &escaped_source_path,
            "impl Drop for crate::model::RadrootsEventIngestReceipt {\n    fn drop(&mut self) { std::process::abort(); }\n}\n",
        )
        .expect("write escaped module source");
        fs::write(
            &schema_path,
            format!("{schema_source}\n#[path = \"../evil.rs\"]\nmod evil;\n"),
        )
        .expect("write escaped module route");
        let error = validate_privileged_store_authority(workspace.path())
            .expect_err("out-of-root module route must fail");
        assert!(
            error.contains("production module source graph is closed"),
            "{error}"
        );
        fs::write(&schema_path, &schema_source).expect("restore event-store schema source");
        fs::remove_file(escaped_source_path).expect("remove escaped module source");

        let store_path = workspace.path().join(EVENT_STORE_STORE_SOURCE_RELATIVE);
        let original = fs::read_to_string(&store_path).expect("store source");
        let mutations = [
            (
                "direct path retarget",
                original.replacen(
                    "mod post_core_extensions_v1;",
                    "#[path = \"store/protocol_reconciliation_v1.rs\"]\nmod post_core_extensions_v1;",
                    1,
                ),
                "outside the exact private governed module inventory",
            ),
            (
                "target-specific path retarget",
                original.replacen(
                    "mod post_core_extensions_v1;",
                    "#[cfg_attr(target_os = \"ios\", path = \"store/protocol_reconciliation_v1.rs\")]\nmod post_core_extensions_v1;",
                    1,
                ),
                "outside the exact private governed module inventory",
            ),
            (
                "residual module cfg",
                original.replacen(
                    "mod post_core_extensions_v1;",
                    "#[cfg(feature = \"sqlite\")]\nmod post_core_extensions_v1;",
                    1,
                ),
                "outside the exact private governed module inventory",
            ),
            (
                "residual privileged import cfg",
                original.replacen(
                    "use self::post_core_extension_dispatcher::dispatch_post_core_extensions;",
                    "#[cfg(feature = \"sqlite\")]\nuse self::post_core_extension_dispatcher::dispatch_post_core_extensions;",
                    1,
                ),
                "must be unconditional and private",
            ),
            (
                "disabled privileged import",
                original.replacen(
                    "use self::post_core_extension_dispatcher::dispatch_post_core_extensions;",
                    "#[cfg(any())]\nuse self::post_core_extension_dispatcher::dispatch_post_core_extensions;",
                    1,
                ),
                "SourceMaintenance privileged import authority drifted",
            ),
            (
                "extra associated core bypass call",
                format!(
                    "{original}\nimpl RadrootsEventStore {{\n    async fn bypass_protocol_seal(\n        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,\n        ingest: &RadrootsEventIngest,\n    ) -> Result<(), RadrootsEventStoreError> {{\n        let _ = ingest_event_protocol_reconciliation_v1(tx, ingest).await?;\n        Ok(())\n    }}\n}}\n"
                ),
                "privileged call-site cardinality or order drifted",
            ),
            (
                "block-local core import alias",
                format!(
                    "{original}\nasync fn local_import_bypass(\n    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,\n    ingest: &RadrootsEventIngest,\n) -> Result<(), RadrootsEventStoreError> {{\n    use self::protocol_reconciliation_v1::ingest_event_protocol_reconciliation_v1 as bypass;\n    let _ = bypass(tx, ingest).await?;\n    Ok(())\n}}\n"
                ),
                "must not be imported or aliased from a nested scope",
            ),
            (
                "function value alias",
                format!(
                    "{original}\nasync fn value_alias_bypass(\n    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,\n    ingest: &RadrootsEventIngest,\n) -> Result<(), RadrootsEventStoreError> {{\n    let bypass = ingest_event_protocol_reconciliation_v1;\n    let _ = bypass(tx, ingest).await?;\n    Ok(())\n}}\n"
                ),
                "takes or aliases privileged value",
            ),
            (
                "privileged parameter shadow",
                format!(
                    "{original}\nfn binding_shadow(validate_event_store_temp_schema: fn()) {{\n    validate_event_store_temp_schema();\n}}\n"
                ),
                "shadows privileged authority with binding",
            ),
            (
                "block-local storage alias",
                format!(
                    "{original}\nfn local_storage_alias<'borrow, 'db>(\n    tx: &'borrow mut sqlx::Transaction<'db, sqlx::Sqlite>,\n) {{\n    use self::post_core_storage_v1::PostCoreStorageV1 as Bypass;\n    let _ = Bypass::new(tx);\n}}\n"
                ),
                "must not be imported or aliased from a nested scope",
            ),
            (
                "storage type alias",
                format!(
                    "{original}\ntype BypassStorage<'borrow, 'db> = PostCoreStorageV1<'borrow, 'db>;\nfn type_alias_bypass<'borrow, 'db>(\n    tx: &'borrow mut sqlx::Transaction<'db, sqlx::Sqlite>,\n) {{\n    let _ = BypassStorage::new(tx);\n}}\n"
                ),
                "aliases privileged authority through type",
            ),
            (
                "storage associated type alias",
                format!(
                    "{original}\ntrait BypassStorageType<'borrow, 'db> {{\n    type Storage;\n}}\nimpl<'borrow, 'db> BypassStorageType<'borrow, 'db> for () {{\n    type Storage = PostCoreStorageV1<'borrow, 'db>;\n}}\nfn associated_type_bypass<'borrow, 'db>(\n    tx: &'borrow mut sqlx::Transaction<'db, sqlx::Sqlite>,\n) {{\n    let _ = <() as BypassStorageType<'borrow, 'db>>::Storage::new(tx);\n}}\n"
                ),
                "aliases `PostCoreStorageV1` through root-store type path",
            ),
            (
                "opaque macro injection",
                format!(
                    "{original}\nfn macro_bypass() {{\n    bypass_store_authority!();\n}}\n"
                ),
                "unsupported or non-builtin-resolved production macro",
            ),
            (
                "qualified allowed-name macro injection",
                format!(
                    "{original}\nfn qualified_macro_bypass() {{\n    crate::elsewhere::format!();\n}}\n"
                ),
                "unsupported or non-builtin-resolved production macro",
            ),
            (
                "nested format argument source injection",
                format!(
                    "{original}\nfn nested_format_bypass() {{\n    let _ = format!(\"{{}}\", {{ crate::elsewhere::inject(); \"\" }});\n}}\n"
                ),
                "side-effect-capable format! argument",
            ),
            (
                "matches guard source injection",
                format!(
                    "{original}\nfn matches_guard_bypass() {{\n    let _ = matches!(0, _ if {{ crate::elsewhere::inject(); true }});\n}}\n"
                ),
                "side-effect-capable matches! expression or guard",
            ),
            (
                "allowed-name macro import alias",
                format!(
                    "{original}\nuse crate::elsewhere::bypass_macro as format;\n"
                ),
                "privileged cross-module import routes drifted",
            ),
            (
                "derive import alias",
                format!(
                    "{original}\nuse crate::elsewhere::bypass_derive as Debug;\n#[derive(Debug)]\nstruct DeriveBypass;\n"
                ),
                "privileged cross-module import routes drifted",
            ),
            (
                "guard import alias",
                format!(
                    "{original}\nuse crate::elsewhere::bypass_guard as validate_event_store_temp_schema;\n"
                ),
                "privileged cross-module import routes drifted",
            ),
            (
                "guard reverse import alias",
                format!(
                    "{original}\nuse crate::schema::validate_event_store_temp_schema as bypass_guard;\n"
                ),
                "privileged terminal",
            ),
            (
                "raw guard import alias",
                format!(
                    "{original}\nuse crate::elsewhere::bypass_guard as r#validate_event_store_temp_schema;\n"
                ),
                "privileged",
            ),
            (
                "raw privileged call",
                format!(
                    "{original}\nasync fn raw_core_bypass(\n    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,\n    ingest: &RadrootsEventIngest,\n) -> Result<(), RadrootsEventStoreError> {{\n    let _ = r#ingest_event_protocol_reconciliation_v1(tx, ingest).await?;\n    Ok(())\n}}\n"
                ),
                "privileged terminal",
            ),
            (
                "raw qualified macro injection",
                format!(
                    "{original}\nfn raw_macro_bypass() {{\n    crate::elsewhere::r#format!();\n}}\n"
                ),
                "unsupported or non-builtin-resolved production macro",
            ),
            (
                "arbitrary glob macro import",
                format!("{original}\nuse crate::elsewhere::*;\n"),
                "privileged cross-module import routes drifted",
            ),
            (
                "include macro injection",
                format!("{original}\ninclude!(\"../bypass.inc\");\n"),
                "compiler macro inputs drifted",
            ),
            (
                "include string source injection",
                format!(
                    "{original}\nconst BYPASS_SOURCE: &str = include_str!(\"../bypass.inc\");\n"
                ),
                "compiler macro inputs drifted",
            ),
        ];
        for (label, mutation, expected_error) in mutations {
            assert_ne!(mutation, original, "{label} fixture must mutate");
            fs::write(&store_path, mutation).expect("write store mutation");
            let error = validate_privileged_store_authority(workspace.path())
                .expect_err("privileged authority mutation must fail");
            assert!(
                error.contains(expected_error)
                    || error.contains("production baseline outside audited extension modules")
                    || error.contains("event-store inherent impl authority drifted")
                    || error.contains("event-store trait impl authority drifted")
                    || error.contains("privileged terminal"),
                "{label} produced unexpected error: {error}"
            );
            fs::write(&store_path, &original).expect("restore store source");
        }

        for (relative, alias) in [
            (
                EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
                "pub(crate) use validate_event_store_temp_schema as chained_guard;\n",
            ),
            (
                EVENT_STORE_PROTOCOL_RECONCILIATION_SOURCE_RELATIVE,
                "pub(super) use ingest_event_protocol_reconciliation_v1 as chained_core;\n",
            ),
            (
                POST_CORE_STORAGE_SOURCE_RELATIVE,
                "pub(super) use PostCoreStorageV1 as ChainedStorage;\n",
            ),
        ] {
            let path = workspace.path().join(relative);
            let source = fs::read_to_string(&path).expect("privileged source");
            fs::write(&path, format!("{source}\n{alias}")).expect("write reverse alias");
            let error = validate_privileged_store_authority(workspace.path())
                .expect_err("privileged reverse alias must fail");
            assert!(
                error.contains("aliases or reexports")
                    || error.contains("privileged cross-module import routes drifted"),
                "{relative} produced unexpected reverse-alias error: {error}"
            );
            fs::write(&path, source).expect("restore privileged source");
        }

        let sibling_path = workspace
            .path()
            .join(EVENT_STORE_STORE_MODULE_ROOT_RELATIVE)
            .join("bypass.rs");
        fs::write(
            &sibling_path,
            "use super::protocol_reconciliation_v1::ingest_event_protocol_reconciliation_v1;\n",
        )
        .expect("write sibling bypass");
        fs::write(
            &store_path,
            original.replacen(
                "mod protocol_storage_v1;",
                "mod protocol_storage_v1;\npub(crate) mod bypass;",
                1,
            ),
        )
        .expect("route sibling bypass");
        let error = validate_privileged_store_authority(workspace.path())
            .expect_err("privileged sibling source must fail");
        assert!(
            error.contains("privileged cross-module import routes drifted")
                || error.contains("source inventory is closed")
                || error.contains("privileged terminal import"),
            "{error}"
        );

        fs::remove_file(&sibling_path).expect("remove sibling bypass");
        fs::write(&store_path, &original).expect("restore store source");

        let impl_bypass_path = workspace
            .path()
            .join(EVENT_STORE_STORE_MODULE_ROOT_RELATIVE)
            .join("impl_bypass.rs");
        let impl_bypass_cases = [
            (
                "same-name local type with qualified governed Drop impl",
                "struct ProtocolReconciliationV1IngestResult;\nimpl Drop for super::protocol_reconciliation_v1::ProtocolReconciliationV1IngestResult {\n    fn drop(&mut self) { std::process::abort(); }\n}\n",
                "event-store trait impl authority drifted",
            ),
            (
                "raw SQL and pool authority",
                "impl super::RadrootsEventStore {\n    pub async fn bypass(&self) {\n        let _ = sqlx::query(\"DROP TRIGGER radroots_event_store_guard\").execute(&self.pool).await;\n    }\n}\n",
                "event-store inherent impl authority drifted",
            ),
            (
                "trait impl on event store",
                "impl core::fmt::Debug for super::RadrootsEventStore {\n    fn fmt(&self, _: &mut core::fmt::Formatter<'_>) -> core::fmt::Result { Ok(()) }\n}\n",
                "event-store trait impl authority drifted",
            ),
            (
                "transaction accessor",
                "impl super::RadrootsEventStore {\n    pub async fn transaction(&self) { let _ = self.pool.begin().await; }\n}\n",
                "event-store inherent impl authority drifted",
            ),
            (
                "ambient process filesystem network thread and environment authority",
                "pub fn ambient() {\n    let _ = std::fs::read(\"x\");\n    let _ = std::net::TcpStream::connect(\"127.0.0.1:1\");\n    let _ = std::process::Command::new(\"true\");\n    let _ = std::thread::spawn(|| {});\n    let _ = std::env::var(\"HOME\");\n}\n",
                "source inventory is closed",
            ),
            (
                "callback and function-pointer authority",
                "pub fn callback_alias(callback: fn()) -> fn() { callback }\n",
                "source inventory is closed",
            ),
        ];
        for (label, source, expected_error) in impl_bypass_cases {
            fs::write(&impl_bypass_path, source).expect("write impl bypass child");
            fs::write(
                &store_path,
                original.replacen(
                    "mod protocol_storage_v1;",
                    "mod protocol_storage_v1;\npub(crate) mod impl_bypass;",
                    1,
                ),
            )
            .expect("route impl bypass child");
            let error = validate_privileged_store_authority(workspace.path())
                .expect_err("governed impl bypass must fail");
            assert!(
                error.contains(expected_error),
                "{label} produced unexpected error: {error}"
            );
        }
        fs::remove_file(&impl_bypass_path).expect("remove impl bypass child");
        fs::write(&store_path, &original).expect("restore store source");

        let benign_path = workspace
            .path()
            .join(EVENT_STORE_STORE_MODULE_ROOT_RELATIVE)
            .join("future_feed_projection_v1.rs");
        fs::write(&benign_path, "pub struct RadrootsFutureFeedProjection;\n")
            .expect("write benign forward-compatible sibling");
        let store_with_benign_child = original.replacen(
            "mod protocol_storage_v1;",
            "mod protocol_storage_v1;\npub(crate) mod future_feed_projection_v1;",
            1,
        );
        fs::write(&store_path, &store_with_benign_child).expect("route benign sibling");
        let error = validate_privileged_store_authority(workspace.path())
            .expect_err("even passive additional modules require explicit contract evolution");
        assert!(error.contains("source inventory is closed"), "{error}");

        let lib_path = workspace.path().join(EVENT_STORE_LIB_SOURCE_RELATIVE);
        let lib_original = fs::read_to_string(&lib_path).expect("event-store lib source");
        let lib_with_concrete_child_reexport = format!(
            "{lib_original}\n#[cfg(feature = \"sqlite\")]\npub use store::future_feed_projection_v1::RadrootsFutureFeedProjection;\n"
        );
        fs::write(&lib_path, &lib_with_concrete_child_reexport)
            .expect("write closed child concrete reexport");
        let error = validate_privileged_store_authority(workspace.path())
            .expect_err("additional public exports require explicit contract evolution");
        assert!(
            error.contains("public export inventory is closed"),
            "{error}"
        );
        fs::write(&lib_path, &lib_original).expect("restore event-store lib source");
        fs::remove_file(&benign_path).expect("remove benign sibling");
        fs::write(&store_path, &original).expect("restore store source");

        let shadow_path = workspace
            .path()
            .join(EVENT_STORE_STORE_MODULE_ROOT_RELATIVE)
            .join("hex.rs");
        fs::write(&shadow_path, "pub(super) fn encode() {}\n")
            .expect("write resolution-shadow sibling");
        for module in ["hex", "r#hex"] {
            fs::write(
                &store_path,
                original.replacen(
                    "mod protocol_storage_v1;",
                    &format!("mod protocol_storage_v1;\npub(crate) mod {module};"),
                    1,
                ),
            )
            .expect("route resolution-shadow sibling");
            let error = validate_privileged_store_authority(workspace.path())
                .expect_err("dependency-shadowing sibling must fail");
            assert!(error.contains("source inventory is closed"), "{error}");
        }
        fs::remove_file(&shadow_path).expect("remove resolution-shadow sibling");
        fs::write(&store_path, &original).expect("restore store source");

        fs::write(
            workspace
                .path()
                .join(EVENT_STORE_STORE_MODULE_ROOT_RELATIVE)
                .join("bypass.inc"),
            "unparsed production code injection",
        )
        .expect("write sibling include");
        let error = validate_privileged_store_authority(workspace.path())
            .expect_err("unexpected non-Rust sibling source must fail");
        assert!(
            error.contains("only auditable Rust source files"),
            "{error}"
        );

        fs::remove_file(
            workspace
                .path()
                .join(EVENT_STORE_STORE_MODULE_ROOT_RELATIVE)
                .join("bypass.inc"),
        )
        .expect("remove sibling include");
        let invalid_lib_extensions = [
            format!(
                "{lib_original}\n#[cfg(feature = \"sqlite\")]\nmod future_feed_projection_v1;\n#[cfg(feature = \"sqlite\")]\npub use model::RadrootsFutureFeedProjection;\n"
            ),
            format!(
                "{lib_original}\n#[cfg(feature = \"sqlite\")]\npub use model::RadrootsFutureFeedProjection;\n"
            ),
        ];
        for invalid_extension in invalid_lib_extensions {
            let invalid_lib = parse_canonical_production_rust(
                EVENT_STORE_LIB_SOURCE_RELATIVE,
                invalid_extension.as_bytes(),
            )
            .expect("parse invalid event-store lib extension");
            validate_event_store_lib_resolution_authority(
                EVENT_STORE_LIB_SOURCE_RELATIVE,
                &invalid_lib,
            )
            .expect_err("top-level module or model reexport extension must fail");
        }
        let lib_mutations = [
            format!(
                "{lib_original}\nmacro_rules! format {{ ($($token:tt)*) => {{ crate::elsewhere::inject() }} }}\n"
            ),
            format!("{lib_original}\n#[macro_use]\n#[cfg(feature = \"sqlite\")]\nmod injected;\n"),
            format!("{lib_original}\n#[prelude_import]\nuse crate::elsewhere::InjectedPrelude;\n"),
            format!("{lib_original}\nextern crate self as sqlx;\n"),
            lib_original.replacen(
                "#[cfg(feature = \"sqlite\")]\nmod store;",
                "#[cfg(not(target_os = \"ios\"))]\nmod store;",
                1,
            ),
            lib_original.replacen(
                "RadrootsEventStoreSourceGeneration, RadrootsEventStoreStatusSummary, RadrootsEventVisibility,",
                "RadrootsEventStoreSourceGeneration, RadrootsEventVisibility,",
                1,
            ),
        ];
        for mutation in lib_mutations {
            assert_ne!(mutation, lib_original, "ancestor fixture must mutate");
            fs::write(&lib_path, mutation).expect("write ancestor mutation");
            let error = validate_privileged_store_authority(workspace.path())
                .expect_err("ancestor namespace mutation must fail");
            assert!(
                error.contains(EVENT_STORE_LIB_SOURCE_RELATIVE),
                "unexpected ancestor error: {error}"
            );
            fs::write(&lib_path, &lib_original).expect("restore event-store lib source");
        }

        let food_path = workspace
            .path()
            .join("crates/event_store/src/store/food_availability_projection_v1.rs");
        let food_original = fs::read_to_string(&food_path).expect("08C Food store source");
        let food_mutations = [
            (
                "08C direct SourceMaintenance terminal call",
                format!(
                    "{food_original}\nasync fn source_terminal_bypass(connection: &mut sqlx::SqliteConnection) {{\n    let _ = crate::source_maintenance_v1::validate_source_capacity_authority_fast_v1(connection).await;\n}}\n"
                ),
            ),
            (
                "08C SourceMaintenance alias import",
                format!(
                    "{food_original}\nuse crate::source_maintenance_v1::validate_source_capacity_authority_fast_v1 as bypass;\n"
                ),
            ),
            (
                "08C SourceMaintenance glob import",
                format!("{food_original}\nuse crate::source_maintenance_v1::*;\n"),
            ),
            (
                "08C SourceMaintenance macro reference",
                format!(
                    "{food_original}\nfn source_terminal_macro_bypass() {{ stringify!(validate_source_capacity_authority_fast_v1); }}\n"
                ),
            ),
            (
                "08C SourceMaintenance associated-function shadow",
                format!(
                    "{food_original}\nstruct SourceTerminalShadow;\nimpl SourceTerminalShadow {{ fn validate_source_capacity_authority_fast_v1() {{}} }}\n"
                ),
            ),
            (
                "08C SourceMaintenance trait-function shadow",
                format!(
                    "{food_original}\ntrait SourceTerminalShadow {{ fn preflight_unique_raw_source_append_v1(); }}\n"
                ),
            ),
        ];
        for (label, mutation) in food_mutations {
            fs::write(&food_path, mutation).expect("write 08C terminal bypass");
            let error = validate_privileged_store_authority(workspace.path())
                .expect_err("08C SourceMaintenance terminal bypass must fail");
            assert!(
                error.contains("privileged terminal")
                    || error.contains("privileged authority")
                    || error.contains("glob import"),
                "{label} produced unexpected error: {error}"
            );
            fs::write(&food_path, &food_original).expect("restore 08C Food source");
        }

        let protocol_path = workspace
            .path()
            .join(EVENT_STORE_PROTOCOL_RECONCILIATION_SOURCE_RELATIVE);
        let protocol_original =
            fs::read_to_string(&protocol_path).expect("protocol reconciliation source");
        let mut protocol_shadow =
            syn::parse_file(&protocol_original).expect("protocol reconciliation AST");
        let ingest = protocol_shadow
            .items
            .iter_mut()
            .find_map(|item| match item {
                syn::Item::Fn(function)
                    if function.sig.ident == "ingest_event_protocol_reconciliation_v1" =>
                {
                    Some(function)
                }
                _ => None,
            })
            .expect("approved SourceMaintenance caller");
        ingest.block.stmts.insert(
            0,
            syn::parse_str("let validate_source_capacity_authority_fast_v1 = || ();")
                .expect("terminal shadow binding"),
        );
        fs::write(&protocol_path, prettyplease::unparse(&protocol_shadow))
            .expect("write approved-caller terminal shadow");
        let error = validate_privileged_store_authority(workspace.path())
            .expect_err("approved-caller terminal shadow must fail");
        assert!(error.contains("shadows privileged authority"), "{error}");
        fs::write(&protocol_path, protocol_original).expect("restore protocol source");
    }

    #[test]
    fn extracted_protocol_module_identity_is_test_neutral_and_production_sensitive() {
        for relative in [
            EVENT_STORE_PROTOCOL_RECONCILIATION_SOURCE_RELATIVE,
            EVENT_STORE_PROTOCOL_STORAGE_SOURCE_RELATIVE,
        ] {
            let source = repository_source(relative);
            let baseline =
                canonical_rust_ast(relative, source.as_bytes(), RustAstProfile::Production)
                    .expect("protocol module AST");
            let neutral_edit = format!(
                "/// test-neutral module documentation\n{source}\n#[cfg(test)]\nfn manifest_identity_probe() {{ panic!(\"test only\"); }}\n"
            );
            assert_eq!(
                canonical_rust_ast(
                    relative,
                    neutral_edit.as_bytes(),
                    RustAstProfile::Production,
                )
                .expect("test-neutral protocol module AST"),
                baseline,
                "{relative} documentation and test-only behavior must not rotate production identity"
            );
        }

        let authority_source =
            repository_source(EVENT_STORE_PROTOCOL_RECONCILIATION_SOURCE_RELATIVE);
        let authority_baseline = canonical_rust_ast(
            EVENT_STORE_PROTOCOL_RECONCILIATION_SOURCE_RELATIVE,
            authority_source.as_bytes(),
            RustAstProfile::Production,
        )
        .expect("authority module AST");
        let seal_type_drift =
            authority_source.replacen("main_schema_version: i64", "main_schema_version: u64", 1);
        assert_ne!(
            seal_type_drift, authority_source,
            "seal fixture must mutate"
        );
        assert_ne!(
            canonical_rust_ast(
                EVENT_STORE_PROTOCOL_RECONCILIATION_SOURCE_RELATIVE,
                seal_type_drift.as_bytes(),
                RustAstProfile::Production,
            )
            .expect("seal-drift module AST"),
            authority_baseline,
            "authority seal field/type drift must rotate production identity"
        );

        let mut no_op_validator =
            syn::parse_file(&authority_source).expect("authority module source");
        let validator = no_op_validator
            .items
            .iter_mut()
            .find_map(|item| match item {
                syn::Item::Fn(function)
                    if function.sig.ident == "validate_protocol_post_extensions" =>
                {
                    Some(function)
                }
                _ => None,
            })
            .expect("protocol post-extension validator");
        *validator.block = syn::parse_str("{ Ok(()) }").expect("no-op validator block");
        let no_op_validator = prettyplease::unparse(&no_op_validator);
        assert_ne!(
            canonical_rust_ast(
                EVENT_STORE_PROTOCOL_RECONCILIATION_SOURCE_RELATIVE,
                no_op_validator.as_bytes(),
                RustAstProfile::Production,
            )
            .expect("no-op validator AST"),
            authority_baseline,
            "replacing the authority validator with success must rotate production identity"
        );
    }

    #[test]
    fn token_compaction_preserves_literal_bytes() {
        let spaced: proc_macro2::TokenStream =
            r#"call("BEGIN IMMEDIATE")"#.parse().expect("spaced literal");
        let compact: proc_macro2::TokenStream =
            r#"call("BEGINIMMEDIATE")"#.parse().expect("compact literal");
        assert_ne!(
            compact_token_stream(spaced),
            compact_token_stream(compact),
            "literal whitespace is semantic and must not be discarded"
        );
    }

    #[test]
    fn migration_reachability_requires_guard_order_and_error_propagation() {
        let source = repository_source(EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE);
        let file = parse_canonical_production_rust(
            EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
            source.as_bytes(),
        )
        .expect("migration AST");
        validate_migration_registry_reachability(EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE, &file)
            .expect("authoritative registry reachability");
        validate_manifest_validator_reachability(EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE, &file)
            .expect("authoritative manifest-validator reachability");
        validate_source_maintenance_manifest_validator_reachability(
            EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
            &file,
        )
        .expect("authoritative SourceMaintenance descriptor reachability");
        validate_source_maintenance_migration_bindings(
            EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
            &file,
        )
        .expect("authoritative SourceMaintenance migration bindings");

        let mut early_return_file = file.clone();
        let early_return_registry = early_return_file
            .items
            .iter_mut()
            .find_map(|item| match item {
                syn::Item::Fn(function) if function.sig.ident == "validate_migration_registry" => {
                    Some(function)
                }
                _ => None,
            })
            .expect("registry validator");
        early_return_registry
            .block
            .stmts
            .insert(0, syn::parse_str("return Ok(());").expect("early return"));
        assert!(
            validate_migration_registry_reachability(
                EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
                &early_return_file,
            )
            .is_err(),
            "an early success return before the generated-manifest guard must fail"
        );

        let mut reordered_predecessor_guards = file.clone();
        let registry = reordered_predecessor_guards
            .items
            .iter_mut()
            .find_map(|item| match item {
                syn::Item::Fn(function) if function.sig.ident == "validate_migration_registry" => {
                    Some(function)
                }
                _ => None,
            })
            .expect("registry validator");
        registry.block.stmts.swap(0, 1);
        assert!(
            validate_migration_registry_reachability(
                EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
                &reordered_predecessor_guards,
            )
            .is_err(),
            "reordering the predecessor manifest guards must fail"
        );

        let mut missing_source_guard = file.clone();
        let registry = missing_source_guard
            .items
            .iter_mut()
            .find_map(|item| match item {
                syn::Item::Fn(function) if function.sig.ident == "validate_migration_registry" => {
                    Some(function)
                }
                _ => None,
            })
            .expect("registry validator");
        registry.block.stmts.remove(2);
        assert!(
            validate_migration_registry_reachability(
                EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
                &missing_source_guard,
            )
            .is_err(),
            "removing the SourceMaintenance descriptor guard must fail"
        );

        let mut reordered_source_guard = file.clone();
        let registry = reordered_source_guard
            .items
            .iter_mut()
            .find_map(|item| match item {
                syn::Item::Fn(function) if function.sig.ident == "validate_migration_registry" => {
                    Some(function)
                }
                _ => None,
            })
            .expect("registry validator");
        registry.block.stmts.swap(2, 3);
        assert!(
            validate_migration_registry_reachability(
                EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
                &reordered_source_guard,
            )
            .is_err(),
            "moving the SourceMaintenance guard behind the range guard must fail"
        );

        let mut discarded_source_guard_result = file.clone();
        let registry = discarded_source_guard_result
            .items
            .iter_mut()
            .find_map(|item| match item {
                syn::Item::Fn(function) if function.sig.ident == "validate_migration_registry" => {
                    Some(function)
                }
                _ => None,
            })
            .expect("registry validator");
        let syn::Stmt::Expr(syn::Expr::If(source_guard), _) = &mut registry.block.stmts[2] else {
            panic!("SourceMaintenance manifest guard");
        };
        strip_outer_try(&mut source_guard.then_branch.stmts[0]);
        assert!(
            validate_migration_registry_reachability(
                EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
                &discarded_source_guard_result,
            )
            .is_err(),
            "discarding the SourceMaintenance descriptor validator Result must fail"
        );

        let mut file = parse_canonical_production_rust(
            EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
            source.as_bytes(),
        )
        .expect("migration AST");
        let registry = file
            .items
            .iter_mut()
            .find_map(|item| match item {
                syn::Item::Fn(function) if function.sig.ident == "validate_migration_registry" => {
                    Some(function)
                }
                _ => None,
            })
            .expect("registry validator");
        let syn::Stmt::Expr(syn::Expr::If(manifest_guard), _) = &mut registry.block.stmts[0] else {
            panic!("manifest guard");
        };
        strip_outer_try(&mut manifest_guard.then_branch.stmts[0]);
        assert!(
            validate_migration_registry_reachability(
                EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
                &file,
            )
            .is_err(),
            "discarding the generated-manifest validator Result must fail"
        );

        let mut file = parse_canonical_production_rust(
            EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
            source.as_bytes(),
        )
        .expect("migration AST");
        let manifest = file
            .items
            .iter_mut()
            .find_map(|item| match item {
                syn::Item::Fn(function)
                    if function.sig.ident == "validate_generated_nip09_manifest_descriptor" =>
                {
                    Some(function)
                }
                _ => None,
            })
            .expect("manifest validator");
        strip_outer_try(&mut manifest.block.stmts[1]);
        assert!(
            validate_manifest_validator_reachability(
                EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
                &file,
            )
            .is_err(),
            "discarding manifest SHA validation must fail"
        );

        let mut source_descriptor_bypass = parse_canonical_production_rust(
            EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
            source.as_bytes(),
        )
        .expect("migration AST");
        let descriptor = source_descriptor_bypass
            .items
            .iter_mut()
            .find_map(|item| match item {
                syn::Item::Fn(function)
                    if function.sig.ident
                        == "validate_generated_source_maintenance_manifest_descriptor" =>
                {
                    Some(function)
                }
                _ => None,
            })
            .expect("SourceMaintenance descriptor validator");
        descriptor.block = syn::parse_str("{ Ok(()) }").expect("no-op descriptor body");
        assert!(
            validate_source_maintenance_manifest_validator_reachability(
                EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
                &source_descriptor_bypass,
            )
            .is_err(),
            "a no-op SourceMaintenance descriptor body must fail while its registry call remains"
        );
    }

    #[test]
    fn migration_hook_loops_require_awaited_question_mark_propagation() {
        let source = repository_source(EVENT_STORE_SCHEMA_SOURCE_RELATIVE);
        let file =
            parse_canonical_production_rust(EVENT_STORE_SCHEMA_SOURCE_RELATIVE, source.as_bytes())
                .expect("schema AST");
        validate_migration_hook_loop_reachability(EVENT_STORE_SCHEMA_SOURCE_RELATIVE, &file)
            .expect("authoritative hook propagation");

        for (function_name, loop_statement_index) in [
            ("migrate_schema_on_connection", 2usize),
            ("validate_applied_migration_hooks", 0usize),
        ] {
            let mut file = parse_canonical_production_rust(
                EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
                source.as_bytes(),
            )
            .expect("schema AST");
            let function = file
                .items
                .iter_mut()
                .find_map(|item| match item {
                    syn::Item::Fn(function) if function.sig.ident == function_name => {
                        Some(function)
                    }
                    _ => None,
                })
                .expect("hook function");
            let loop_expression = function
                .block
                .stmts
                .iter_mut()
                .find_map(|statement| match statement {
                    syn::Stmt::Expr(syn::Expr::ForLoop(expression), _) => Some(expression),
                    _ => None,
                })
                .expect("hook loop");
            strip_outer_try(&mut loop_expression.body.stmts[loop_statement_index]);
            assert!(
                validate_migration_hook_loop_reachability(
                    EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
                    &file,
                )
                .is_err(),
                "discarding `{function_name}` hook Result must fail"
            );
        }
    }

    #[test]
    fn schema_runtime_routes_reject_inspection_and_migration_bypasses() {
        let source = repository_source(EVENT_STORE_SCHEMA_SOURCE_RELATIVE);
        let file =
            parse_canonical_production_rust(EVENT_STORE_SCHEMA_SOURCE_RELATIVE, source.as_bytes())
                .expect("schema AST");
        validate_schema_runtime_reachability(EVENT_STORE_SCHEMA_SOURCE_RELATIVE, &file)
            .expect("authoritative schema runtime chain");

        for function_name in [
            "inspect_event_store_schema_status",
            "migrate_event_store_schema",
            "migrate_event_store_schema_with_generation_provider_and_limits_inner",
            "migrate_event_store_schema_with_registry_and_generation_provider",
        ] {
            let mut file = parse_canonical_production_rust(
                EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
                source.as_bytes(),
            )
            .expect("schema AST");
            let function = file
                .items
                .iter_mut()
                .find_map(|item| match item {
                    syn::Item::Fn(function) if function.sig.ident == function_name => {
                        Some(function)
                    }
                    _ => None,
                })
                .expect("schema route");
            function.block = syn::parse_str("{ Ok(()) }").expect("bypass block");
            assert!(
                validate_schema_runtime_reachability(EVENT_STORE_SCHEMA_SOURCE_RELATIVE, &file,)
                    .is_err(),
                "early-success bypass in `{function_name}` must fail"
            );
        }

        let source_preflight = r#"        if has_pending_source_maintenance_hook(&status, registry) {
            validate_no_persisted_ephemeral_raw_rows_v1(&mut connection).await?;
        }
"#;
        let begin_immediate =
            "    let mut transaction = pool.begin_with(\"BEGIN IMMEDIATE\").await?;\n";
        let preflight_mutations = [
            (
                "removed SourceMaintenance persisted-ephemeral preflight",
                source.replacen(source_preflight, "", 1),
            ),
            (
                "discarded SourceMaintenance persisted-ephemeral preflight Result",
                source.replacen(
                    "validate_no_persisted_ephemeral_raw_rows_v1(&mut connection).await?;",
                    "validate_no_persisted_ephemeral_raw_rows_v1(&mut connection).await;",
                    1,
                ),
            ),
            (
                "SourceMaintenance preflight moved after BEGIN IMMEDIATE",
                source.replacen(source_preflight, "", 1).replacen(
                    begin_immediate,
                    &format!("{begin_immediate}{source_preflight}"),
                    1,
                ),
            ),
            (
                "unguarded SourceMaintenance persisted-ephemeral preflight",
                source.replacen(
                    source_preflight,
                    "        validate_no_persisted_ephemeral_raw_rows_v1(&mut connection).await?;\n",
                    1,
                ),
            ),
        ];
        for (label, mutation) in preflight_mutations {
            assert_ne!(mutation, source, "{label} fixture must mutate");
            let file = parse_canonical_production_rust(
                EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
                mutation.as_bytes(),
            )
            .expect("mutated SourceMaintenance preflight AST");
            assert!(
                validate_schema_runtime_reachability(EVENT_STORE_SCHEMA_SOURCE_RELATIVE, &file)
                    .is_err(),
                "{label} must fail closed"
            );
        }

        for (label, mutation) in [
            (
                "TEMP LIKE wildcard filter",
                source.replacen(
                    "SELECT type, name, tbl_name FROM temp.sqlite_schema ORDER BY type, name, tbl_name",
                    "SELECT type, name, tbl_name FROM temp.sqlite_schema WHERE name NOT LIKE 'sqlite_%' ORDER BY type, name, tbl_name",
                    1,
                ),
            ),
            (
                "main catalog LIKE wildcard filter",
                source.replacen(
                    "SELECT type, name, tbl_name, sql FROM main.sqlite_schema",
                    "SELECT type, name, tbl_name, sql FROM main.sqlite_schema WHERE name NOT LIKE 'sqlite_%'",
                    1,
                ),
            ),
        ] {
            assert_ne!(mutation, source, "{label} fixture must mutate");
            let file = parse_canonical_production_rust(
                EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
                mutation.as_bytes(),
            )
            .expect("mutated schema AST");
            let error =
                validate_schema_runtime_reachability(EVENT_STORE_SCHEMA_SOURCE_RELATIVE, &file)
                    .err()
                    .expect("LIKE wildcard catalog filter must fail");
            assert!(
                error.contains("authoritative schema runtime"),
                "{label} produced unexpected error: {error}"
            );
        }
    }

    #[test]
    fn sqlite_encoding_preflight_rejects_ordering_and_propagation_bypasses() {
        let source = repository_source(EVENT_STORE_STORE_SOURCE_RELATIVE);
        let baseline =
            parse_canonical_production_rust(EVENT_STORE_STORE_SOURCE_RELATIVE, source.as_bytes())
                .expect("store AST");
        validate_sqlite_encoding_preflight_authority(EVENT_STORE_STORE_SOURCE_RELATIVE, &baseline)
            .expect("authoritative SQLite encoding preflight");

        for mutation in ["remove", "discard", "after_temp", "before_backing"] {
            let mut file = baseline.clone();
            let configure_pool = file
                .items
                .iter_mut()
                .find_map(|item| match item {
                    syn::Item::Fn(function) if function.sig.ident == "configure_pool" => {
                        Some(function)
                    }
                    _ => None,
                })
                .expect("configure_pool");
            let preflight = configure_pool
                .block
                .stmts
                .iter_mut()
                .find_map(|statement| match statement {
                    syn::Stmt::Expr(syn::Expr::ForLoop(preflight), _)
                        if compact_tokens(&preflight.body)
                            .contains("validate_main_database_encoding(connection).await?") =>
                    {
                        Some(preflight)
                    }
                    _ => None,
                })
                .expect("encoding preflight loop");
            let encoding_index = preflight
                .body
                .stmts
                .iter()
                .position(|statement| {
                    compact_tokens(statement)
                        == "validate_main_database_encoding(connection).await?;"
                })
                .expect("encoding preflight statement");
            let backing_index = preflight
                .body
                .stmts
                .iter()
                .position(|statement| {
                    compact_tokens(statement).starts_with("iffile_backed==database_is_memory")
                })
                .expect("backing classification statement");
            let temp_index = preflight
                .body
                .stmts
                .iter()
                .position(|statement| {
                    compact_tokens(statement)
                        .starts_with("crate::schema::validate_event_store_temp_schema")
                })
                .expect("TEMP-schema validation statement");
            match mutation {
                "remove" => {
                    preflight.body.stmts.remove(encoding_index);
                }
                "discard" => strip_outer_try(&mut preflight.body.stmts[encoding_index]),
                "after_temp" => preflight.body.stmts.swap(encoding_index, temp_index),
                "before_backing" => preflight.body.stmts.swap(backing_index, encoding_index),
                _ => unreachable!(),
            }
            assert!(
                validate_sqlite_encoding_preflight_authority(
                    EVENT_STORE_STORE_SOURCE_RELATIVE,
                    &file,
                )
                .is_err(),
                "SQLite encoding `{mutation}` bypass must fail closed"
            );
        }

        let mut no_op = baseline;
        let validator = no_op
            .items
            .iter_mut()
            .find_map(|item| match item {
                syn::Item::Fn(function)
                    if function.sig.ident == "validate_main_database_encoding" =>
                {
                    Some(function)
                }
                _ => None,
            })
            .expect("encoding validator");
        validator.block = syn::parse_str("{ Ok(()) }").expect("no-op encoding validator");
        assert!(
            validate_sqlite_encoding_preflight_authority(
                EVENT_STORE_STORE_SOURCE_RELATIVE,
                &no_op,
            )
            .is_err(),
            "a no-op encoding validator must fail closed"
        );
    }

    #[test]
    fn source_generation_rollback_guard_rejects_policy_and_ordering_bypasses() {
        let source = repository_source(EVENT_STORE_SCHEMA_SOURCE_RELATIVE);
        let baseline =
            parse_canonical_production_rust(EVENT_STORE_SCHEMA_SOURCE_RELATIVE, source.as_bytes())
                .expect("schema AST");
        validate_source_generation_rollback_authority(
            EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
            &baseline,
        )
        .expect("authoritative source-generation rollback guard");

        let mut wrapper_bypass = baseline.clone();
        let wrapper = wrapper_bypass
            .items
            .iter_mut()
            .find_map(|item| match item {
                syn::Item::Fn(function)
                    if function.sig.ident == "rollback_event_store_schema_with_registry" =>
                {
                    Some(function)
                }
                _ => None,
            })
            .expect("production rollback wrapper");
        wrapper.block = syn::parse_str("{ Ok(()) }").expect("rollback wrapper bypass");
        assert!(
            validate_source_generation_rollback_authority(
                EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
                &wrapper_bypass,
            )
            .is_err(),
            "a production rollback wrapper that omits `Preserve` must fail closed"
        );

        for mutation in ["remove", "discard", "after_down_loop"] {
            let mut file = baseline.clone();
            let rollback = file
                .items
                .iter_mut()
                .find_map(|item| match item {
                    syn::Item::Fn(function)
                        if function.sig.ident == "rollback_schema_on_connection" =>
                    {
                        Some(function)
                    }
                    _ => None,
                })
                .expect("rollback implementation");
            match mutation {
                "remove" => {
                    rollback.block.stmts.remove(2);
                }
                "discard" => {
                    let syn::Stmt::Expr(syn::Expr::If(guard), _) = &mut rollback.block.stmts[2]
                    else {
                        panic!("source-generation rollback guard");
                    };
                    strip_outer_try(&mut guard.then_branch.stmts[0]);
                }
                "after_down_loop" => rollback.block.stmts.swap(2, 3),
                _ => unreachable!(),
            }
            assert!(
                validate_source_generation_rollback_authority(
                    EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
                    &file,
                )
                .is_err(),
                "source-generation rollback `{mutation}` bypass must fail closed"
            );
        }

        let mut policy_bypass = baseline.clone();
        let policy = policy_bypass
            .items
            .iter_mut()
            .find_map(|item| match item {
                syn::Item::Enum(item) if item.ident == "SourceGenerationHistoryRollbackPolicy" => {
                    Some(item)
                }
                _ => None,
            })
            .expect("rollback policy enum");
        policy
            .variants
            .push(syn::parse_str("AllowDestructive").expect("production bypass variant"));
        assert!(
            validate_source_generation_rollback_authority(
                EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
                &policy_bypass,
            )
            .is_err(),
            "a second production rollback policy must fail closed"
        );

        let mut no_op = baseline;
        let validator = no_op
            .items
            .iter_mut()
            .find_map(|item| match item {
                syn::Item::Fn(function)
                    if function.sig.ident
                        == "validate_rollback_preserves_source_generation_history" =>
                {
                    Some(function)
                }
                _ => None,
            })
            .expect("rollback floor validator");
        validator.block = syn::parse_str("{ Ok(()) }").expect("no-op rollback validator");
        assert!(
            validate_source_generation_rollback_authority(
                EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
                &no_op,
            )
            .is_err(),
            "a no-op rollback floor validator must fail closed"
        );
    }

    #[test]
    fn standalone_source_contract_rejects_marker_preserving_schema_early_return() {
        let workspace = synthetic_workspace();
        super::super::source_maintenance::validate_schema_capacity_authority(workspace.path())
            .expect("baseline SourceMaintenance marker layer");

        let schema_path = workspace.path().join(EVENT_STORE_SCHEMA_SOURCE_RELATIVE);
        let source = fs::read_to_string(&schema_path).expect("schema source");
        let mut mutation = syn::parse_file(&source).expect("schema AST");
        let outer = mutation
            .items
            .iter_mut()
            .find_map(|item| match item {
                syn::Item::Fn(function)
                    if function.sig.ident
                        == "migrate_event_store_schema_with_registry_and_generation_provider" =>
                {
                    Some(function)
                }
                _ => None,
            })
            .expect("outer migration route");
        outer.block.stmts.insert(
            0,
            syn::parse_str("if std::hint::black_box(false) { return Ok(()); }")
                .expect("marker-preserving early return"),
        );
        fs::write(&schema_path, prettyplease::unparse(&mutation))
            .expect("write marker-preserving schema bypass");

        super::super::source_maintenance::validate_schema_capacity_authority(workspace.path())
            .expect("marker-only layer intentionally preserves all ordered witnesses");
        let error = super::super::source_maintenance::validate_source_contract(workspace.path())
            .expect_err("standalone SourceMaintenance authority must reject the early return");
        assert!(
            error.contains("authoritative schema runtime"),
            "unexpected active governance error: {error}"
        );
    }

    #[test]
    fn schema_migration_application_authority_rejects_hookless_and_call_path_bypasses() {
        let workspace = synthetic_workspace();
        let schema_path = workspace.path().join(EVENT_STORE_SCHEMA_SOURCE_RELATIVE);
        let source = fs::read_to_string(&schema_path).expect("schema authority source");
        let baseline_sha256 = sha256_hex(source.as_bytes());
        let baseline =
            parse_canonical_production_rust(EVENT_STORE_SCHEMA_SOURCE_RELATIVE, source.as_bytes())
                .expect("schema authority AST");
        validate_schema_runtime_reachability(EVENT_STORE_SCHEMA_SOURCE_RELATIVE, &baseline)
            .expect("current successor schema runtime authority");

        let mut hookless_mutation = syn::parse_file(&source).expect("schema authority AST");
        let hook_dispatch = hookless_mutation
            .items
            .iter_mut()
            .find_map(|item| match item {
                syn::Item::Fn(function) if function.sig.ident == "apply_migration_hook" => {
                    Some(function)
                }
                _ => None,
            })
            .expect("migration hook dispatcher");
        let hook_match = hook_dispatch
            .block
            .stmts
            .iter_mut()
            .find_map(|statement| match statement {
                syn::Stmt::Expr(syn::Expr::Match(expression), _) => Some(expression),
                _ => None,
            })
            .expect("migration hook match");
        let none_arm = hook_match
            .arms
            .iter_mut()
            .find(|arm| compact_tokens(&arm.pat) == "EventStoreMigrationHook::None")
            .expect("hookless migration arm");
        *none_arm.body = syn::parse_str(
            "{ sqlx::query(\"DELETE FROM event_envelopes\").execute(&mut *connection).await?; Ok(()) }",
        )
        .expect("malicious hookless arm");

        let mut sql_bypass = syn::parse_file(&source).expect("schema authority AST");
        let apply_up = sql_bypass
            .items
            .iter_mut()
            .find_map(|item| match item {
                syn::Item::Fn(function) if function.sig.ident == "apply_migration_up" => {
                    Some(function)
                }
                _ => None,
            })
            .expect("migration SQL application function");
        *apply_up.block = syn::parse_str("{ Ok(()) }").expect("no-op migration SQL application");

        let mut call_path_bypass = syn::parse_file(&source).expect("schema authority AST");
        let migrate = call_path_bypass
            .items
            .iter_mut()
            .find_map(|item| match item {
                syn::Item::Fn(function) if function.sig.ident == "migrate_schema_on_connection" => {
                    Some(function)
                }
                _ => None,
            })
            .expect("migration call path");
        let initializer = migrate
            .block
            .stmts
            .iter_mut()
            .find_map(|statement| match statement {
                syn::Stmt::Local(local)
                    if local_pattern_ident(&local.pat).as_deref() == Some("current_version") =>
                {
                    local.init.as_mut()
                }
                _ => None,
            })
            .expect("current-version initializer");
        let original_initializer = initializer.expr.as_ref().clone();
        *initializer.expr = syn::parse2(quote::quote!({ return Ok(()); #original_initializer }))
            .expect("early-return migration initializer");
        let import_rebind = source.replacen(
            "    apply_reconciliation_hook, validate_active_hook_state_fast, validate_reconciliation_capacity,",
            "    apply_reconciliation_hook, validate_reconciliation_capacity,",
            1,
        );
        assert_ne!(import_rebind, source, "import-rebind fixture must mutate");
        let import_rebind = format!(
            "{import_rebind}\nasync fn validate_active_hook_state_fast(\n    connection: &mut SqliteConnection,\n) -> Result<(), RadrootsEventStoreError> {{\n    sqlx::query(\"DELETE FROM event_envelopes\").execute(&mut *connection).await?;\n    Ok(())\n}}\n"
        );
        let catalog_delta_bypass = source.replacen(
            "changed == expected_changed",
            "changed.is_subset(&expected_changed)",
            1,
        );
        assert_ne!(
            catalog_delta_bypass, source,
            "catalog-delta bypass fixture must mutate"
        );

        for (label, mutation) in [
            (
                "malicious hookless migration arm",
                prettyplease::unparse(&hookless_mutation),
            ),
            (
                "bypassed migration SQL application",
                prettyplease::unparse(&sql_bypass),
            ),
            (
                "migration call-path early return",
                prettyplease::unparse(&call_path_bypass),
            ),
            ("import-rebound hook validator", import_rebind),
            ("widened replacement catalog delta", catalog_delta_bypass),
        ] {
            fs::write(&schema_path, mutation).expect("write schema authority mutation");
            let mutated_bytes = fs::read(&schema_path).expect("mutated schema bytes");
            assert_ne!(
                sha256_hex(&mutated_bytes),
                baseline_sha256,
                "{label} must rotate the successor's exact schema source descriptor"
            );
            let mutated =
                parse_canonical_production_rust(EVENT_STORE_SCHEMA_SOURCE_RELATIVE, &mutated_bytes)
                    .expect("mutated schema authority AST");
            let (structural_error, expected_error) = match label {
                "malicious hookless migration arm"
                | "bypassed migration SQL application"
                | "widened replacement catalog delta" => (
                    validate_schema_migration_execution_authority(
                        EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
                        &mutated,
                    )
                    .expect_err("migration execution mutation must fail closed"),
                    "authoritative schema migration execution",
                ),
                "migration call-path early return" => (
                    validate_schema_runtime_reachability(
                        EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
                        &mutated,
                    )
                    .err()
                    .expect("migration call-path mutation must fail closed"),
                    "authoritative schema runtime",
                ),
                "import-rebound hook validator" => (
                    validate_privileged_store_authority(workspace.path())
                        .expect_err("hook-validator rebind must fail closed"),
                    "privileged",
                ),
                _ => unreachable!(),
            };
            assert!(
                structural_error.contains(expected_error),
                "unexpected {label} structural error: {structural_error}"
            );
            let active_error =
                super::super::source_maintenance::validate_source_contract(workspace.path())
                    .expect_err("standalone SourceMaintenance contract must reject schema bypass");
            assert!(
                active_error.contains(expected_error),
                "unexpected active {label} error: {active_error}"
            );
            if label == "migration call-path early return" {
                let migrate = exact_top_level_function(
                    EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
                    &mutated,
                    "migrate_schema_on_connection",
                )
                .expect("mutated migration call path");
                let current_version = migrate
                    .block
                    .stmts
                    .iter()
                    .find(|statement| {
                        matches!(
                            statement,
                            syn::Stmt::Local(local)
                                if local_pattern_ident(&local.pat).as_deref()
                                    == Some("current_version")
                        )
                    })
                    .expect("mutated current-version statement");
                assert!(
                    validate_no_diverging_control_flow(
                        EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
                        "migrate_schema_on_connection current_version",
                        current_version,
                    )
                    .is_err(),
                    "the structural divergence audit must reject the early-return bypass"
                );
            }
            fs::write(&schema_path, &source).expect("restore schema authority source");
        }
    }

    #[test]
    fn successor_import_authority_rejects_direct_external_rebindings() {
        let workspace = synthetic_workspace();
        for (relative, label, needle, replacement) in [
            (
                EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
                "NIP-09 apply and validation routes",
                "use crate::nip09::reconciliation_v1::{",
                "use arbitrary_external::{",
            ),
            (
                EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
                "Food apply and validation routes",
                "use crate::store::food_availability_projection_v1::{",
                "use arbitrary_external::{",
            ),
            (
                EVENT_STORE_SCHEMA_SOURCE_RELATIVE,
                "migration helper routes",
                "use crate::migrations::{",
                "use arbitrary_external::{",
            ),
            (
                EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
                "NIP-09 generated manifest route",
                "use crate::generated::nip09_reconciliation_manifest as nip09_manifest;",
                "use arbitrary_external::nip09_reconciliation_manifest as nip09_manifest;",
            ),
            (
                EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
                "Food generated manifest route",
                "use crate::generated::food_availability_projection_manifest as food_manifest;",
                "use arbitrary_external::food_availability_projection_manifest as food_manifest;",
            ),
            (
                EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
                "SourceMaintenance generated manifest route",
                "use crate::generated::source_maintenance_manifest;",
                "use arbitrary_external::source_maintenance_manifest;",
            ),
        ] {
            let path = workspace.path().join(relative);
            let source = fs::read_to_string(&path).expect("successor import authority source");
            let mutation = source.replacen(needle, replacement, 1);
            assert_ne!(mutation, source, "{label} fixture must mutate");
            fs::write(&path, &mutation).expect("write external import rebind");
            let mutation = parse_canonical_production_rust(relative, mutation.as_bytes())
                .expect("external import rebind AST");
            let structural_error = if relative == EVENT_STORE_SCHEMA_SOURCE_RELATIVE {
                validate_event_store_schema_import_authority(relative, &mutation)
            } else {
                validate_event_store_migrations_import_authority(relative, &mutation)
            }
            .expect_err("external import rebind must fail exact route authority");
            assert!(
                structural_error.contains("production top-level import authority"),
                "unexpected {label} structural error: {structural_error}"
            );
            let active_error =
                super::super::source_maintenance::validate_source_contract(workspace.path())
                    .expect_err(
                        "standalone SourceMaintenance contract must reject external rebind",
                    );
            assert!(
                active_error.contains("production top-level import authority")
                    || active_error.contains("privileged terminal import"),
                "unexpected active {label} error: {active_error}"
            );
            fs::write(&path, source).expect("restore successor import authority source");
        }
    }

    #[test]
    fn schema_name_matcher_witness_rejects_case_and_prefix_regressions() {
        let source = repository_source(EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE);
        let file = parse_canonical_production_rust(
            EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
            source.as_bytes(),
        )
        .expect("migrations AST");
        validate_event_store_schema_name_matchers(EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE, &file)
            .expect("authoritative schema-name matchers");

        for mutation in [
            source.replacen(
                "name.eq_ignore_ascii_case(EVENT_STORE_LEDGER_NAME)",
                "name == EVENT_STORE_LEDGER_NAME",
                1,
            ),
            source.replacen(
                "candidate.eq_ignore_ascii_case(prefix)",
                "candidate == prefix",
                1,
            ),
            source.replacen(
                ".any(|owned| name.eq_ignore_ascii_case(owned))",
                ".any(|owned| name == owned)",
                1,
            ),
        ] {
            assert_ne!(mutation, source, "matcher fixture must mutate");
            let file = parse_canonical_production_rust(
                EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
                mutation.as_bytes(),
            )
            .expect("mutated migrations AST");
            assert!(
                validate_event_store_schema_name_matchers(
                    EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
                    &file,
                )
                .is_err(),
                "case/prefix matcher regression must fail"
            );
        }
    }

    #[test]
    fn public_store_schema_routes_reject_migration_bypasses() {
        let source = repository_source("crates/event_store/src/store.rs");
        let file =
            parse_canonical_production_rust("crates/event_store/src/store.rs", source.as_bytes())
                .expect("store AST");
        for name in [
            "open_memory",
            "open_file",
            "open_pool",
            "schema_status",
            "migrate_to_current_schema",
        ] {
            let function = exact_associated_function(
                "crates/event_store/src/store.rs",
                &file,
                "RadrootsEventStore",
                name,
            )
            .expect("public store schema route");
            validate_route_only_associated_function(
                "crates/event_store/src/store.rs",
                "RadrootsEventStore",
                name,
                function,
            )
            .expect("authoritative public store route");
        }

        let mut file =
            parse_canonical_production_rust("crates/event_store/src/store.rs", source.as_bytes())
                .expect("store AST");
        let open_memory = file
            .items
            .iter_mut()
            .filter_map(|item| match item {
                syn::Item::Impl(item) => Some(item),
                _ => None,
            })
            .flat_map(|item| item.items.iter_mut())
            .find_map(|item| match item {
                syn::ImplItem::Fn(function) if function.sig.ident == "open_memory" => {
                    Some(function)
                }
                _ => None,
            })
            .expect("open_memory");
        open_memory.block = syn::parse_str("{ loop {} }").expect("bypass block");
        assert!(
            validate_route_only_associated_function(
                "crates/event_store/src/store.rs",
                "RadrootsEventStore",
                "open_memory",
                open_memory,
            )
            .is_err(),
            "open_memory must not bypass schema migration"
        );
    }

    #[test]
    fn thin_ingest_wrapper_requires_lexical_capability_then_protocol_seal() {
        let relative = "crates/event_store/src/store.rs";
        let source = repository_source(relative);
        let file = parse_canonical_production_rust(relative, source.as_bytes()).expect("store AST");
        let function = exact_top_level_function(relative, &file, "ingest_event_in_transaction")
            .expect("thin ingest wrapper");
        validate_route_only_free_function(relative, "ingest_event_in_transaction", function)
            .expect("authoritative capability/seal wrapper");

        for mutation in [
            "remove_temp_guard",
            "swap_temp_guard_and_core",
            "remove_capability",
            "swap_capability_and_seal",
            "remove_seal",
        ] {
            let mut file =
                parse_canonical_production_rust(relative, source.as_bytes()).expect("store AST");
            let function = file
                .items
                .iter_mut()
                .find_map(|item| match item {
                    syn::Item::Fn(function)
                        if function.sig.ident == "ingest_event_in_transaction" =>
                    {
                        Some(function)
                    }
                    _ => None,
                })
                .expect("thin ingest wrapper");
            match mutation {
                "remove_temp_guard" => {
                    function.block.stmts.remove(0);
                }
                "swap_temp_guard_and_core" => {
                    function.block.stmts.swap(0, 1);
                }
                "remove_capability" => {
                    function.block.stmts.remove(2);
                }
                "swap_capability_and_seal" => {
                    function.block.stmts.swap(2, 3);
                }
                "remove_seal" => {
                    function.block.stmts.remove(3);
                }
                _ => unreachable!(),
            }
            assert!(
                validate_route_only_free_function(
                    relative,
                    "ingest_event_in_transaction",
                    function,
                )
                .is_err(),
                "thin wrapper mutation `{mutation}` must fail"
            );
        }
    }

    #[test]
    fn post_core_descriptor_attests_private_production_helpers() {
        let workspace = synthetic_workspace();
        let baseline =
            describe_post_core_sql_capability(workspace.path()).expect("baseline capability");

        let extension_path = workspace.path().join(POST_CORE_EXTENSION_SOURCE_RELATIVE);
        let mut extension = fs::read_to_string(&extension_path).expect("extension source");
        extension.push_str("\nfn helper_neutral_probe(value: u8) -> u8 { value }\n");
        fs::write(&extension_path, extension).expect("mutated extension source");

        let storage_path = workspace.path().join(POST_CORE_STORAGE_SOURCE_RELATIVE);
        let mut storage = fs::read_to_string(&storage_path).expect("storage source");
        storage.push_str("\nfn helper_neutral_probe(value: u8) -> u8 { value }\n");
        fs::write(&storage_path, storage).expect("mutated storage source");

        let edited =
            describe_post_core_sql_capability(workspace.path()).expect("edited capability");
        assert_ne!(
            edited, baseline,
            "private production helper behavior must rotate the attested capability descriptor"
        );
    }

    #[test]
    fn post_core_extension_boundary_is_v1_stable_and_append_only() {
        let workspace = synthetic_workspace();
        let immutable = immutable_manifest();
        let baseline = describe_post_core_extension_boundary(workspace.path(), false)
            .expect("migration-bound v2 capability boundary");
        assert_eq!(
            baseline.capability_struct_ast_sha256,
            immutable
                .post_core_sql_capability
                .capability_struct_ast_sha256
        );
        assert_eq!(
            baseline.capability_constructor_ast_sha256,
            immutable
                .post_core_sql_capability
                .capability_constructor_ast_sha256
        );
        assert_eq!(
            baseline.capability_v1_method_ast_sha256,
            immutable
                .post_core_sql_capability
                .capability_v1_method_ast_sha256
        );
        assert_eq!(
            baseline.dispatcher_signature_sha256,
            immutable
                .post_core_sql_capability
                .dispatcher_signature_sha256
        );
        assert_eq!(
            baseline.dispatcher_v1_prefix_sha256,
            immutable
                .post_core_sql_capability
                .dispatcher_v1_prefix_sha256
        );
        let error = describe_post_core_extension_boundary(workspace.path(), true)
            .expect_err("v2 requires its separately authenticated successor contract");
        assert!(
            error.contains("authenticated capability boundary"),
            "{error}"
        );

        let dispatcher_path = workspace.path().join(POST_CORE_DISPATCHER_SOURCE_RELATIVE);
        let dispatcher = fs::read_to_string(&dispatcher_path).expect("dispatcher source");
        for mutation in [
            dispatcher.replacen(
                "    capabilities.apply_v1(ingest, result).await?;\n    capabilities.apply_v2().await?;",
                "    capabilities.apply_v2().await?;\n    capabilities.apply_v1(ingest, result).await?;",
                1,
            ),
            dispatcher.replacen(
                "capabilities.apply_v1(ingest, result).await?;",
                "capabilities.apply_v1(ingest, result).await;",
                1,
            ),
            dispatcher.replacen(
                "    capabilities.apply_v1(ingest, result).await?;",
                "    if false { return Ok(()); }\n    capabilities.apply_v1(ingest, result).await?;",
                1,
            ),
            dispatcher.replacen(
                "capabilities.apply_v2().await?;",
                "capabilities.apply_v3().await?;",
                1,
            ),
            dispatcher.replacen(
                "    capabilities.apply_v2().await?;",
                "    capabilities.apply_v1(ingest, result).await?;\n    capabilities.apply_v2().await?;",
                1,
            ),
            dispatcher.replacen(
                "    capabilities.apply_v1(ingest, result).await?;",
                "    let v1 = capabilities.apply_v1(ingest, result);\n    v1.await?;",
                1,
            ),
        ] {
            assert_ne!(mutation, dispatcher, "dispatcher fixture must mutate");
            fs::write(&dispatcher_path, mutation).expect("write dispatcher mutation");
            assert!(
                describe_post_core_extension_boundary(workspace.path(), false).is_err(),
                "dispatcher must call direct extension methods contiguously in version order"
            );
        }
    }

    #[test]
    fn post_core_extension_capability_rejects_raw_authority_escape_shapes() {
        let workspace = synthetic_workspace();
        let capabilities_path = workspace
            .path()
            .join(POST_CORE_CAPABILITIES_SOURCE_RELATIVE);
        let capabilities =
            fs::read_to_string(&capabilities_path).expect("capability boundary source");
        let impl_end = capabilities
            .rfind("\n}")
            .expect("capability impl closing brace");
        let mutations = [
            format!("{capabilities}\nstruct ExtraTransactionHolder;\n"),
            format!(
                "{capabilities}\nimpl<'borrow, 'db> Drop for PostCoreExtensionCapabilities<'borrow, 'db> {{\n    fn drop(&mut self) {{}}\n}}\n"
            ),
            format!(
                "{}\n\n    pub(super) fn transaction(&mut self) -> &mut Transaction<'db, Sqlite> {{\n        self.tx\n    }}{}\n",
                &capabilities[..impl_end],
                capabilities[impl_end..].trim_end(),
            ),
            capabilities.replacen(
                "    tx: &'borrow mut Transaction<'db, Sqlite>,",
                "    tx: &'borrow mut Transaction<'db, Sqlite>,\n    escaped: bool,",
                1,
            ),
            capabilities.replacen(
                "        let mut storage = PostCoreStorageV1::new(self.tx);\n        apply_post_core_extensions_v1(&mut storage, ingest, result).await",
                "        let _ = (ingest, result);\n        Ok(())",
                1,
            ),
        ];
        for mutation in mutations {
            assert_ne!(mutation, capabilities, "capability fixture must mutate");
            fs::write(&capabilities_path, mutation).expect("write capability mutation");
            assert!(
                describe_post_core_extension_boundary(workspace.path(), false).is_err(),
                "raw transaction authority or v1 route escape must fail"
            );
        }
    }

    #[test]
    fn post_core_descriptor_attests_exact_ordered_observation_bindings() {
        let workspace = synthetic_workspace();
        let baseline =
            describe_post_core_sql_capability(workspace.path()).expect("baseline capability");
        let observation_statement = baseline
            .statements
            .iter()
            .find(|statement| {
                statement.function == "upsert_transport_observation"
                    && statement.tables == ["event_transport_observation"]
            })
            .expect("transport-observation SQL statement");
        assert_eq!(
            observation_statement.bind_expressions,
            [
                "event_id",
                "observation.transport_kind().canonical_label()",
                "observation.endpoint_uri().as_str()",
                "observation.endpoint_fingerprint().as_str()",
                "observation.observation_type().as_str()",
                "observation.observed_at_ms()",
                "observation.observed_at_ms()",
                "observation.caller_redacted_message()",
            ]
        );
        assert_eq!(observation_statement.placeholder_count, 8);

        let storage_path = workspace.path().join(POST_CORE_STORAGE_SOURCE_RELATIVE);
        let storage = fs::read_to_string(&storage_path).expect("storage source");
        let mutations = [
            (
                "constant replacement",
                storage.replacen(".bind(event_id)", ".bind(\"forged-event\")", 1),
            ),
            (
                "semantic substitution",
                storage.replacen(
                    ".bind(observation.transport_kind().canonical_label())",
                    ".bind(observation.endpoint_uri().as_str())",
                    1,
                ),
            ),
            (
                "URI and fingerprint swap",
                storage.replacen(
                    ".bind(observation.endpoint_uri().as_str())\n        .bind(observation.endpoint_fingerprint().as_str())",
                    ".bind(observation.endpoint_fingerprint().as_str())\n        .bind(observation.endpoint_uri().as_str())",
                    1,
                ),
            ),
            (
                "bind omission",
                storage.replacen(
                    "        .bind(observation.caller_redacted_message())\n",
                    "",
                    1,
                ),
            ),
        ];
        for (label, mutation) in mutations {
            assert_ne!(mutation, storage, "{label} fixture must mutate");
            fs::write(&storage_path, mutation).expect("write bind mutation");
            match describe_post_core_sql_capability(workspace.path()) {
                Ok(edited) => assert_ne!(
                    edited, baseline,
                    "{label} must rotate the post-core SQL capability descriptor"
                ),
                Err(error) => assert!(
                    label == "bind omission"
                        && error.contains("placeholder/bind cardinality drifted"),
                    "{label} produced unexpected extraction error: {error}"
                ),
            }
            let error = validate_route_facade_baselines(workspace.path())
                .expect_err("ordered bind mutation must fail immutable source authority");
            assert!(
                error.contains("route-facade production baseline"),
                "{error}"
            );
            fs::write(&storage_path, &storage).expect("restore storage source");
        }
    }

    #[test]
    fn post_core_sql_grammar_rejects_escalation_and_side_effects() {
        describe_post_core_sql(
            "probe",
            "SELECT 1 FROM trade_mutation WHERE mutation_id = ? LIMIT 1",
            "fetch_optional",
            &["mutation_id.as_str()".to_owned()],
        )
        .expect("authorized existence query");
        for sql in [
            "SELECT 1 FROM trade_mutation, event_envelopes",
            "SELECT 1 FROM trade_mutation.event_envelopes",
            "SELECT 1 FROM trade_mutation UNION SELECT 1 FROM trade_mutation",
            "SELECT load_extension(?) FROM trade_mutation",
            "INSERT INTO trade_mutation(mutation_id) VALUES(load_extension(?))",
            "DELETE FROM trade_missing_parent RETURNING writefile('x', 'y')",
        ] {
            let error = describe_post_core_sql("probe", sql, "execute", &[])
                .expect_err("SQL escalation must fail the restricted grammar");
            assert!(!error.is_empty());
        }
    }

    #[test]
    fn post_core_capability_rejects_ambient_and_transaction_authority_escapes() {
        let extension = repository_source(POST_CORE_EXTENSION_SOURCE_RELATIVE);
        validate_post_core_extension_source(
            POST_CORE_EXTENSION_SOURCE_RELATIVE,
            extension.as_bytes(),
        )
        .expect("authoritative extension boundary");
        for escape in [
            "fn ambient_escape() { let _ = std::fs::write(\"x\", b\"y\"); }",
            "fn ambient_escape() { let _ = std::net::TcpStream::connect(\"127.0.0.1:1\"); }",
            "fn ambient_escape() { let _ = std::process::Command::new(\"true\"); }",
            "fn ambient_escape() { tokio::spawn(async {}); }",
            "fn ambient_escape() { let _ = std::env::var(\"HOME\"); }",
            "fn ambient_escape() { unsafe { std::env::set_var(\"X\", \"Y\"); } }",
            "fn authority_escape() { let _ = sqlx::query(\"DELETE FROM trade_mutation\"); }",
            "fn authority_escape() { validate_protocol_post_extensions(); }",
            "fn authority_escape() { crate::other_authority(); }",
            "fn authority_escape() { arbitrary_external::execute(); }",
            "#[arbitrary_transform]\nfn transformed_helper() {}",
            "fn local_import_escape() { use crate::schema::ambient_side_effect as sha256_hex; sha256_hex(); }",
            "fn binding_shadow_escape() { let sha256_hex = arbitrary_external::execute; sha256_hex(); }",
        ] {
            let mutation = format!("{extension}\n{escape}\n");
            let error = validate_post_core_extension_source(
                POST_CORE_EXTENSION_SOURCE_RELATIVE,
                mutation.as_bytes(),
            )
            .expect_err("ambient or protocol authority escape must fail");
            assert!(!error.is_empty());
        }

        let storage = repository_source(POST_CORE_STORAGE_SOURCE_RELATIVE);
        validate_post_core_storage_source(POST_CORE_STORAGE_SOURCE_RELATIVE, storage.as_bytes())
            .expect("authoritative storage boundary");
        let mutations = [
            storage.replace(
                "pub(super) struct TradeProjectionWrite<'a> {",
                "pub(super) struct TradeProjectionWrite<'a> {\n    pub leaked: &'a str,",
            ),
            format!(
                "{storage}\nstruct ExtraTransactionHolder<'a> {{ tx: &'a mut Transaction<'a, Sqlite> }}\n"
            ),
            format!(
                "{storage}\nimpl<'borrow, 'db> AsMut<Transaction<'db, Sqlite>> for PostCoreStorageV1<'borrow, 'db> {{ fn as_mut(&mut self) -> &mut Transaction<'db, Sqlite> {{ self.tx }} }}\n"
            ),
            format!(
                "{storage}\nfn ambient_escape() {{ let _ = std::fs::write(\"x\", b\"y\"); }}\n"
            ),
            format!(
                "{storage}\nfn ambient_escape() {{ let _ = std::process::Command::new(\"true\"); }}\n"
            ),
            format!("{storage}\nfn authority_escape() {{ crate::other_authority(); }}\n"),
            storage.replacen(
                "        sqlx::query(",
                "        escape(&mut **self.tx);\n        sqlx::query(",
                1,
            ),
            storage.replacen(
                "        sqlx::query(",
                "        let leaked = &mut **self.tx;\n        let _ = leaked;\n        sqlx::query(",
                1,
            ),
            storage.replacen(
                "        sqlx::query(",
                "        let Self { tx } = self;\n        let connection = tx.as_mut();\n        escape(connection);\n        sqlx::query(",
                1,
            ),
            storage.replacen(
                "        sqlx::query(",
                "        let Self { tx: connection } = self;\n        escape(connection);\n        sqlx::query(",
                1,
            ),
            storage.replacen(
                "    async fn insert_trade_mutation_parents(",
                "    async fn insert_trade_mutation_parents<F: FnMut(&mut sqlx::SqliteConnection)>(",
                1,
            ),
            storage.replacen(
                "    async fn insert_trade_mutation_parents(",
                "    #[arbitrary_transform]\n    async fn insert_trade_mutation_parents(",
                1,
            ),
            storage
                .replacen(
                    "    pub(super) fn new(\n        event:",
                    "    pub(super) fn new<F: FnOnce()>(\n        callback: F,\n        event:",
                    1,
                )
                .replacen(
                    "    ) -> Self {\n        Self {",
                    "    ) -> Self {\n        callback();\n        Self {",
                    1,
                ),
            storage.replacen(
                "INSERT INTO trade_projection_quarantine",
                "INSERT INTO event_envelopes",
                1,
            ),
        ];
        for mutation in mutations {
            let error = validate_post_core_storage_source(
                POST_CORE_STORAGE_SOURCE_RELATIVE,
                mutation.as_bytes(),
            )
            .expect_err("storage capability escape must fail");
            assert!(!error.is_empty());
        }
    }

    #[test]
    fn write_and_validation_keep_the_generated_bundle_exact() {
        let workspace = synthetic_workspace();
        write_nip09_reconciliation_manifest(workspace.path()).expect("write manifest bundle");
        validate_nip09_reconciliation_manifest(workspace.path()).expect("validate manifest bundle");

        let manifest = fs::read(workspace.path().join(MANIFEST_RELATIVE)).expect("read manifest");
        let digest =
            fs::read(workspace.path().join(MANIFEST_SHA256_RELATIVE)).expect("read digest");
        assert_eq!(digest, format!("{}\n", sha256_hex(&manifest)).as_bytes());

        let parsed: Nip09ReconciliationManifest =
            serde_json::from_slice(&manifest).expect("parse manifest");
        assert!(
            parsed
                .runtime_dependencies
                .iter()
                .any(|dependency| dependency.name == "secp256k1")
        );
        assert!(
            parsed
                .runtime_dependencies
                .iter()
                .any(|dependency| dependency.name == "secp256k1-sys")
        );
        for direct in RUNTIME_DEPENDENCY_ROOTS {
            assert!(
                parsed
                    .runtime_dependencies
                    .iter()
                    .any(|dependency| dependency.name == direct.name)
            );
        }
    }

    #[test]
    fn hookless_post_v2_sql_rejects_transaction_catalog_and_maintenance_authority() {
        let workspace = synthetic_workspace();
        let future_up_path = workspace
            .path()
            .join("crates/event_store/migrations/0003_future_probe.up.sql");
        for malicious_sql in [
            "BEGIN IMMEDIATE;\n",
            "COMMIT;\n",
            "END;\n",
            "ROLLBACK;\n",
            "SAVEPOINT future_probe;\n",
            "RELEASE future_probe;\n",
            "ANALYZE radroots_event_store_future_probe;\n",
            "REINDEX radroots_event_store_future_probe;\n",
            "DELETE FROM sqlite_stat1;\n",
            "DELETE FROM sqlite_stat4;\n",
            "DELETE FROM sqlite_future_internal;\n",
        ] {
            fs::write(&future_up_path, malicious_sql).expect("write isolated-validator fixture");
            let error = validate_hookless_post_v2_migration_sql_isolated(
                workspace.path(),
                3,
                "up",
                "crates/event_store/migrations/0003_future_probe.up.sql",
            )
            .expect_err("hookless future migration must not acquire ambient SQL authority");
            assert!(
                error.contains("protected v1 object or ambient schema authority"),
                "{error}"
            );
        }
    }

    #[test]
    fn hookless_post_v2_sql_requires_exact_owned_object_ddl() {
        let owned = HooklessMigrationOwnedNames {
            objects: [
                "radroots_event_store_future_probe",
                "radroots_event_store_future_probe_value_idx",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            tables: ["radroots_event_store_future_probe"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
        };
        validate_hookless_migration_owned_ddl(
            "future.up.sql",
            3,
            "up",
            "CREATE TABLE main.radroots_event_store_future_probe (\n  id INTEGER PRIMARY KEY NOT NULL,\n  value TEXT NOT NULL CHECK (value <> '')\n) STRICT;\nCREATE UNIQUE INDEX radroots_event_store_future_probe_value_idx ON main.radroots_event_store_future_probe(value);\n",
            &owned,
        )
        .expect("exact owned-object up DDL");
        validate_hookless_migration_owned_ddl(
            "future.down.sql",
            3,
            "down",
            "DROP INDEX main.radroots_event_store_future_probe_value_idx;\nDROP TABLE radroots_event_store_future_probe;\n",
            &owned,
        )
        .expect("exact owned-object down DDL");

        for malicious_sql in [
            "INSERT INTO caller_alias(id) VALUES (1);",
            "UPDATE caller_alias SET id = 1;",
            "DELETE FROM caller_alias;",
            "REPLACE INTO caller_alias(id) VALUES (1);",
            "SELECT caller_side_effect();",
            "CREATE VIEW radroots_event_store_future_probe_value_idx AS SELECT * FROM caller_alias;",
            "CREATE TRIGGER radroots_event_store_future_probe_value_idx AFTER INSERT ON radroots_event_store_future_probe BEGIN DELETE FROM caller_alias; END;",
            "CREATE TABLE radroots_event_store_future_probe AS SELECT * FROM caller_alias;",
            "CREATE TABLE radroots_event_store_future_probe (id INTEGER REFERENCES caller_parent(id)) STRICT;\nCREATE INDEX radroots_event_store_future_probe_value_idx ON radroots_event_store_future_probe(id);",
            "CREATE TABLE radroots_event_store_future_probe (id INTEGER) STRICT;",
            "CREATE TABLE radroots_event_store_other (id INTEGER) STRICT;\nCREATE INDEX radroots_event_store_future_probe_value_idx ON radroots_event_store_other(id);",
            "CREATE TABLE radroots_event_store_future_probe (id INTEGER DEFAULT caller_side_effect()) STRICT;\nCREATE INDEX radroots_event_store_future_probe_value_idx ON radroots_event_store_future_probe(id);",
        ] {
            let error = validate_hookless_migration_owned_ddl(
                "future.up.sql",
                3,
                "up",
                malicious_sql,
                &owned,
            )
            .expect_err("non-owned or executable hookless migration SQL must fail");
            assert!(!error.is_empty());
        }
    }

    #[test]
    fn migration_runtime_authority_is_bound_without_binding_future_registry_growth() {
        let workspace = synthetic_workspace();
        let migrations_path = workspace
            .path()
            .join(EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE);
        let source = fs::read_to_string(&migrations_path).expect("migration authority source");
        let baseline_sha256 = sha256_hex(source.as_bytes());
        let baseline = parse_canonical_production_rust(
            EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
            source.as_bytes(),
        )
        .expect("migration authority AST");
        validate_migration_registry_reachability(EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE, &baseline)
            .expect("current successor registry reachability");
        validate_manifest_validator_reachability(EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE, &baseline)
            .expect("immutable predecessor descriptor reachability");
        validate_source_maintenance_manifest_validator_reachability(
            EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
            &baseline,
        )
        .expect("SourceMaintenance descriptor reachability");
        validate_source_maintenance_migration_bindings(
            EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
            &baseline,
        )
        .expect("SourceMaintenance v4 binding authority");
        validate_event_store_migrations_import_authority(
            EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
            &baseline,
        )
        .expect("migration import authority");
        validate_event_store_migration_support_authority(
            EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
            &baseline,
        )
        .expect("active migration support authority");
        expected_event_store_migration_compiler_inputs(workspace.path(), &baseline)
            .expect("current versioned migration compiler inputs");

        for (label, needle, replacement) in [
            (
                "SourceMaintenance v4 version",
                "        version: 4,\n        name: \"source_maintenance\",",
                "        version: 5,\n        name: \"source_maintenance\",",
            ),
            (
                "SourceMaintenance v4 hook",
                "        hook: EventStoreMigrationHook::SourceMaintenanceV1,",
                "        hook: EventStoreMigrationHook::None,",
            ),
            (
                "SourceMaintenance v4 manifest hash",
                "        hook_manifest_sha256: Some(source_maintenance_manifest::SOURCE_MAINTENANCE_MANIFEST_SHA256),",
                "        hook_manifest_sha256: Some(food_manifest::FOOD_AVAILABILITY_PROJECTION_MANIFEST_SHA256),",
            ),
            (
                "SourceMaintenance v4 registry version",
                "            source_maintenance_manifest::SOURCE_MAINTENANCE_EVENT_CONTRACT_REGISTRY_VERSION,",
                "            food_manifest::FOOD_AVAILABILITY_PROJECTION_EVENT_CONTRACT_REGISTRY_VERSION,",
            ),
            (
                "SourceMaintenance v4 replacement inventory binding",
                "        replaced_object_names: EVENT_STORE_SOURCE_MAINTENANCE_REPLACED_OBJECT_NAMES,",
                "        replaced_object_names: &[],",
            ),
        ] {
            let mutation = source.replacen(needle, replacement, 1);
            assert_ne!(mutation, source, "{label} fixture must mutate");
            fs::write(&migrations_path, &mutation).expect("write v4 authority mutation");
            let mutation = parse_canonical_production_rust(
                EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
                mutation.as_bytes(),
            )
            .expect("mutated SourceMaintenance v4 AST");
            assert!(
                expected_event_store_migration_compiler_inputs(workspace.path(), &mutation)
                    .is_err()
                    || validate_source_maintenance_migration_bindings(
                        EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
                        &mutation,
                    )
                    .is_err(),
                "{label} drift must fail closed"
            );
            super::super::source_maintenance::validate_source_contract(workspace.path())
                .expect_err("standalone SourceMaintenance contract must reject v4 binding drift");
            fs::write(&migrations_path, &source).expect("restore migration authority source");
        }

        for (label, needle, replacement) in [
            (
                "ledger DDL",
                ") STRICT, WITHOUT ROWID\")",
                ") STRICT, WITHOUT ROWID /* authority mutation */\")",
            ),
            (
                "baseline FTS inventory",
                "pub(crate) const EVENT_STORE_BASELINE_FTS5_TABLE_NAMES: &[&str] = &[\"listing_search_fts\"];",
                "pub(crate) const EVENT_STORE_BASELINE_FTS5_TABLE_NAMES: &[&str] = &[\"listing_search_fts\", \"future_search_fts\"];",
            ),
            (
                "migration contract type",
                "#[derive(Clone, Copy)]\npub(crate) struct EventStoreMigration",
                "#[derive(Clone, Copy, Debug)]\npub(crate) struct EventStoreMigration",
            ),
            (
                "frozen v1 registry entry",
                "        up_len: 10_712,",
                "        up_len: 10_713,",
            ),
            (
                "migration lookup helper",
                ".find(|migration| migration.version == version)",
                ".find(|migration| version == migration.version)",
            ),
            (
                "embedded migration checksum helper",
                "    if sql.len() != expected_len {",
                "    if sql.as_bytes().len() != expected_len {",
            ),
            (
                "generic registry validation body",
                "        if migration.name.is_empty() {",
                "        if migration.name.len() == 0 {",
            ),
            (
                "registry initializer early return",
                "    let mut expected_version = minimum;",
                "    let mut expected_version = { return Ok(()); minimum };",
            ),
            (
                "duplicate hook reuse guard",
                "            if !migration_hook_ids.insert(migration.hook.id()) {",
                "            if false && !migration_hook_ids.insert(migration.hook.id()) {",
            ),
            (
                "canonical hook migration binding",
                "            if migration.version != canonical_version || migration.name != canonical_name {",
                "            if false && (migration.version != canonical_version || migration.name != canonical_name) {",
            ),
            (
                "exact predecessor replacement ownership",
                "            if prior_owners.len() != 1 {",
                "            if prior_owners.is_empty() {",
            ),
            (
                "predecessor table replacement prohibition",
                "            if prior_owner.owned_table_names.contains(object_name)\n                || prior_owner.fts5_table_names.contains(object_name)",
                "            if prior_owner.owned_table_names.contains(object_name)\n                && prior_owner.fts5_table_names.contains(object_name)",
            ),
        ] {
            let mutation = source.replacen(needle, replacement, 1);
            assert_ne!(mutation, source, "{label} fixture must mutate");
            fs::write(&migrations_path, mutation).expect("write migration authority mutation");
            assert_ne!(
                sha256_hex(&fs::read(&migrations_path).expect("mutated migration bytes")),
                baseline_sha256,
                "{label} must rotate the successor's exact migration source descriptor"
            );
            let mutation = parse_canonical_production_rust(
                EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
                &fs::read(&migrations_path).expect("mutated migration bytes"),
            )
            .expect("mutated migration support AST");
            let structural_error = match validate_event_store_migration_support_authority(
                EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
                &mutation,
            ) {
                Ok(()) => panic!("{label} active migration support mutation must fail closed"),
                Err(error) => error,
            };
            assert!(
                structural_error.contains("active migration support token authority"),
                "unexpected {label} structural error: {structural_error}"
            );
            let active_error =
                super::super::source_maintenance::validate_source_contract(workspace.path())
                    .expect_err(
                        "standalone SourceMaintenance contract must reject migration support drift",
                    );
            assert!(
                active_error.contains("active migration support token authority"),
                "unexpected active {label} error: {active_error}"
            );
            fs::write(&migrations_path, &source).expect("restore migration authority source");
        }

        let mutation = source.replacen(
            r#"    let up_byte_length = generated_manifest_u128_to_u64(
        nip09_manifest::NIP09_RECONCILIATION_MIGRATION_UP_BYTE_LENGTH as u128,
        "generated NIP-09 migration up byte length is out of range",
    )?;"#,
            r#"    let up_byte_length = {
        return Ok(());
        0_u64
    };"#,
            1,
        );
        assert_ne!(
            mutation, source,
            "generated-manifest validator fixture must mutate only its initializer"
        );
        fs::write(&migrations_path, &mutation)
            .expect("write manifest-validator early-return mutation");
        assert_ne!(
            sha256_hex(mutation.as_bytes()),
            baseline_sha256,
            "generated-manifest validator bypass must rotate the successor source descriptor"
        );
        let mutation = parse_canonical_production_rust(
            EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
            mutation.as_bytes(),
        )
        .expect("manifest-validator early-return AST");
        let validator = exact_top_level_function(
            EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
            &mutation,
            "validate_generated_nip09_manifest_descriptor",
        )
        .expect("mutated predecessor descriptor validator");
        let up_byte_length = validator
            .block
            .stmts
            .iter()
            .find(|statement| {
                matches!(
                    statement,
                    syn::Stmt::Local(local)
                        if local_pattern_ident(&local.pat).as_deref()
                            == Some("up_byte_length")
                )
            })
            .expect("mutated up-byte-length statement");
        assert!(
            validate_no_diverging_control_flow(
                EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
                "validate_generated_nip09_manifest_descriptor up_byte_length",
                up_byte_length,
            )
            .is_err(),
            "the structural divergence audit must reject an early return"
        );
        let structural_error = validate_event_store_migration_support_authority(
            EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
            &mutation,
        )
        .expect_err("predecessor descriptor bypass must fail active support authority");
        assert!(structural_error.contains("active migration support token authority"));
        let active_error =
            super::super::source_maintenance::validate_source_contract(workspace.path())
                .expect_err("standalone SourceMaintenance contract must reject predecessor bypass");
        assert!(
            active_error.contains(
                "generated-manifest validator authoritative top-level statement skeleton drifted"
            ),
            "unexpected active predecessor descriptor error: {active_error}"
        );
        fs::write(&migrations_path, &source).expect("restore migration authority source");
    }

    #[test]
    fn source_replacement_inventory_and_sql_restoration_are_active_authority() {
        let workspace = synthetic_workspace();
        let migrations_path = workspace
            .path()
            .join(EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE);
        let migrations = fs::read_to_string(&migrations_path).expect("migration registry source");
        let replacement_mutation = migrations.replacen(
            "    \"radroots_event_store_food_availability_image_delete_guard\",\n",
            "",
            1,
        );
        assert_ne!(
            replacement_mutation, migrations,
            "replacement inventory fixture must mutate"
        );
        fs::write(&migrations_path, replacement_mutation)
            .expect("write replacement inventory mutation");
        let error = super::super::source_maintenance::validate_source_contract(workspace.path())
            .expect_err("replacement inventory drift must fail active SourceMaintenance authority");
        assert!(
            error.contains("migration catalog differs"),
            "unexpected replacement inventory error: {error}"
        );
        fs::write(&migrations_path, migrations).expect("restore migration registry source");

        for (relative, label, needle, replacement) in [
            (
                "crates/event_store/migrations/0004_source_maintenance.up.sql",
                "widened managed-v4 marker predicate",
                "      AND NEW.prior_last_transition_seq = state.last_transition_seq\n",
                "      AND NEW.prior_last_transition_seq >= state.last_transition_seq\n",
            ),
            (
                "crates/event_store/migrations/0004_source_maintenance.down.sql",
                "omitted exact v3 marker restoration predicate",
                "      AND NEW.transition_floor_seq = state.last_transition_seq\n",
                "",
            ),
        ] {
            let path = workspace.path().join(relative);
            let source = fs::read_to_string(&path).expect("replacement SQL source");
            let mutation = source.replacen(needle, replacement, 1);
            assert_ne!(mutation, source, "{label} fixture must mutate");
            fs::write(&path, mutation).expect("write replacement SQL mutation");
            let error =
                super::super::source_maintenance::validate_source_contract(workspace.path())
                    .expect_err("replacement SQL drift must fail exact migration identity");
            assert!(
                error.contains("reviewed v4 identity"),
                "unexpected {label} error: {error}"
            );
            fs::write(&path, source).expect("restore replacement SQL source");
        }
    }

    #[test]
    fn hookless_future_migration_does_not_churn_nip09_v1_artifacts() {
        let workspace = synthetic_workspace();
        let bundle_paths = [
            MANIFEST_RELATIVE,
            MANIFEST_SCHEMA_RELATIVE,
            MANIFEST_SHA256_RELATIVE,
            GENERATED_DESCRIPTOR_RELATIVE,
        ];
        let before = bundle_paths.map(|relative| {
            read_regular_file(workspace.path(), relative).expect("baseline artifact")
        });
        write_nip09_reconciliation_manifest(workspace.path())
            .expect("write baseline manifest bundle");

        let up_sql =
            b"CREATE TABLE radroots_event_store_future_probe (id INTEGER PRIMARY KEY NOT NULL) STRICT;\n";
        let down_sql = b"DROP TABLE radroots_event_store_future_probe;\n";
        fs::write(
            workspace
                .path()
                .join("crates/event_store/migrations/0005_future_probe.up.sql"),
            up_sql,
        )
        .expect("write future up migration");
        fs::write(
            workspace
                .path()
                .join("crates/event_store/migrations/0005_future_probe.down.sql"),
            down_sql,
        )
        .expect("write future down migration");

        let migrations_path = workspace
            .path()
            .join(EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE);
        let migrations = fs::read_to_string(&migrations_path).expect("migration registry source");
        let migrations = migrations.replacen(
            "pub const RADROOTS_EVENT_STORE_SCHEMA_VERSION_CURRENT: u32 = 4;",
            "pub const RADROOTS_EVENT_STORE_SCHEMA_VERSION_CURRENT: u32 = 5;",
            1,
        );
        let registry_tail = "    },\n];\n\npub(crate) fn migration_for_version";
        let future_entry = format!(
            r#"    }},
    EventStoreMigration {{
        version: 5,
        name: "future_probe",
        up_sql: include_str!("../migrations/0005_future_probe.up.sql"),
        down_sql: include_str!("../migrations/0005_future_probe.down.sql"),
        up_len: {},
        down_len: {},
        up_sha256: "{}",
        down_sha256: "{}",
        schema_sha256: "0000000000000000000000000000000000000000000000000000000000000000",
        owned_object_names: &["radroots_event_store_future_probe"],
        replaced_object_names: &[],
        owned_table_names: &["radroots_event_store_future_probe"],
        fts5_table_names: &[],
        hook: EventStoreMigrationHook::None,
        hook_manifest_sha256: None,
        event_contract_registry_version: None,
    }},
];

pub(crate) fn migration_for_version"#,
            up_sql.len(),
            down_sql.len(),
            sha256_hex(up_sql),
            sha256_hex(down_sql),
        );
        let migrations = migrations.replacen(registry_tail, &future_entry, 1);
        assert!(
            migrations.contains("version: 5")
                && migrations.contains("../migrations/0005_future_probe.up.sql"),
            "future migration fixture must extend the registry"
        );
        fs::write(&migrations_path, &migrations).expect("write future migration registry");

        let registry = parse_canonical_production_rust(
            EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
            &fs::read(&migrations_path).expect("future migration registry"),
        )
        .expect("future migration registry AST");
        expected_event_store_migration_compiler_inputs(workspace.path(), &registry)
            .expect("versioned successor and isolated future compiler inputs");

        for (field, needle, replacement) in [
            (
                "hook_manifest_sha256",
                "hook_manifest_sha256: None",
                "hook_manifest_sha256: Some(source_maintenance_manifest::SOURCE_MAINTENANCE_MANIFEST_SHA256)",
            ),
            (
                "event_contract_registry_version",
                "event_contract_registry_version: None",
                "event_contract_registry_version: Some(source_maintenance_manifest::SOURCE_MAINTENANCE_EVENT_CONTRACT_REGISTRY_VERSION)",
            ),
        ] {
            let mut invalid = migrations.clone();
            let index = invalid
                .rfind(needle)
                .expect("future hookless field must be the final matching field");
            invalid.replace_range(index..index + needle.len(), replacement);
            let invalid = parse_canonical_production_rust(
                EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
                invalid.as_bytes(),
            )
            .expect("invalid future hook authority AST");
            let error = expected_event_store_migration_compiler_inputs(workspace.path(), &invalid)
                .expect_err("hookless v5 migration must reject non-None authority");
            assert!(
                error.contains(field) && error.contains("versioned hook authority"),
                "unexpected v5 `{field}` error: {error}"
            );
        }

        let mut invalid_replacements = migrations.clone();
        let needle = "replaced_object_names: &[]";
        let index = invalid_replacements
            .rfind(needle)
            .expect("future hookless replacement field must be the final matching field");
        invalid_replacements.replace_range(
            index..index + needle.len(),
            "replaced_object_names: &[\"radroots_event_store_food_availability_projection_delete_guard\"]",
        );
        let invalid_replacements = parse_canonical_production_rust(
            EVENT_STORE_MIGRATIONS_SOURCE_RELATIVE,
            invalid_replacements.as_bytes(),
        )
        .expect("invalid future replacement authority AST");
        let error =
            expected_event_store_migration_compiler_inputs(workspace.path(), &invalid_replacements)
                .expect_err("hookless v5 migration must reject predecessor replacements");
        assert!(
            error.contains("hookless post-v2 migration 5")
                && error.contains("predecessor replacements")
                && error.contains("separately authenticated successor authority"),
            "unexpected v5 replacement authority error: {error}"
        );

        for (relative, before) in bundle_paths.into_iter().zip(before) {
            let after = read_regular_file(workspace.path(), relative).expect("future artifact");
            assert_eq!(
                after, before,
                "{} must remain byte-identical for a hookless post-v2 migration",
                relative
            );
        }
        let future_up_path = workspace
            .path()
            .join("crates/event_store/migrations/0005_future_probe.up.sql");
        for malicious_sql in [
            "INSERT INTO event_envelopes(raw_json) VALUES ('{}');\n",
            "INSERT INTO 'event_envelopes'(raw_json) VALUES ('{}');\n",
            "UPDATE 'event_envelopes' SET raw_json = '{}';\n",
            "DELETE FROM event_envelopes;\n",
            "DROP TABLE 'event_envelopes';\n",
            "CREATE TRIGGER future_probe AFTER INSERT ON trade_mutation BEGIN SELECT 1; END;\n",
            "CREATE TRIGGER future_probe AFTER INSERT ON 'trade_mutation' BEGIN SELECT 1; END;\n",
            "CREATE TRIGGER future_probe AFTER INSERT ON radroots_event_store_future_probe BEGIN UPDATE event_envelope_tags SET relay_indexed = 0; END;\n",
            "CREATE TABLE future_child (id INTEGER PRIMARY KEY, parent_id TEXT REFERENCES trade_missing_parent(missing_parent_mutation_id) ON DELETE RESTRICT) STRICT;\n",
            "CREATE UNIQUE INDEX future_probe_idx ON event_envelopes(event_id);\n",
            "ALTER TABLE radroots_event_store_nip09_request RENAME TO future_probe;\n",
            "DROP TRIGGER radroots_event_store_event_envelopes_append_guard;\n",
            "CREATE TRIGGER radroots_event_store_event_envelopes_append_guard AFTER INSERT ON radroots_event_store_future_probe BEGIN SELECT 1; END;\n",
            "DELETE FROM \"radroots_event_store_addressable_head_state\";\n",
            "DELETE FROM listing_search_fts_data;\n",
            "DELETE FROM radroots_event_store_schema_migrations;\n",
            "DROP TABLE 'radroots_event_store_schema_migrations';\n",
            "INSERT INTO caller_alias(id) VALUES (1);\n",
            "UPDATE caller_alias SET id = 1;\n",
            "DELETE FROM caller_alias;\n",
            "REPLACE INTO caller_alias(id) VALUES (1);\n",
            "CREATE TABLE radroots_event_store_future_probe AS SELECT * FROM caller_alias;\n",
            "CREATE VIEW radroots_event_store_future_probe AS SELECT * FROM caller_alias;\n",
            "UPDATE sqlite_sequence SET seq = 0 WHERE name = 'event_envelopes';\n",
            "DELETE FROM sqlite_stat1;\n",
            "DELETE FROM sqlite_stat4;\n",
            "ANALYZE radroots_event_store_future_probe;\n",
            "REINDEX radroots_event_store_future_probe;\n",
            "BEGIN IMMEDIATE;\n",
            "COMMIT;\n",
            "END;\n",
            "ROLLBACK;\n",
            "SAVEPOINT future_probe;\n",
            "RELEASE future_probe;\n",
            "SELECT load_extension('future_extension');\n",
            "PRAGMA writable_schema = ON;\n",
            "VACUUM INTO 'future-copy.sqlite3';\n",
        ] {
            fs::write(&future_up_path, malicious_sql).expect("write coupled future migration");
            let error = validate_hookless_post_v2_migration_sql_isolated(
                workspace.path(),
                5,
                "up",
                "crates/event_store/migrations/0005_future_probe.up.sql",
            )
            .expect_err("hookless future migration must not couple to v1 authority");
            assert!(
                error.contains("hookless post-v2 migration")
                    || error.contains("hookless CREATE")
                    || error.contains("hookless DROP"),
                "{error}"
            );
        }
        fs::write(&future_up_path, up_sql).expect("restore isolated future migration");
        validate_nip09_reconciliation_manifest(workspace.path())
            .expect("existing NIP-09 v1 artifacts remain valid");
    }

    #[test]
    fn unrelated_future_product_sources_do_not_churn_nip09_v1_manifest() {
        let workspace = synthetic_workspace();
        let before = read_regular_file(workspace.path(), MANIFEST_RELATIVE)
            .expect("immutable NIP-09 v1 manifest");
        validate_nip09_reconciliation_manifest(workspace.path())
            .expect("baseline immutable predecessor bundle");
        validate_privileged_store_authority(workspace.path())
            .expect("baseline current privileged store authority");

        let operational_listing_path = workspace
            .path()
            .join("crates/event/src/operational_listing.rs");
        let operational_listing =
            fs::read_to_string(&operational_listing_path).expect("operational-listing source");
        fs::write(
            &operational_listing_path,
            format!("pub mod future_food_availability_feed_v1;\n{operational_listing}"),
        )
        .expect("route future event source");
        fs::create_dir_all(
            workspace
                .path()
                .join("crates/event/src/operational_listing"),
        )
        .expect("create future event module directory");
        fs::write(
            workspace
                .path()
                .join(
                    "crates/event/src/operational_listing/future_food_availability_feed_v1.rs",
                ),
            "pub struct FutureFoodAvailabilityFeedItem {\n    pub event_id: String,\n    pub available_at_ms: i64,\n}\n\nimpl FutureFoodAvailabilityFeedItem {\n    pub fn is_available(&self) -> bool { self.available_at_ms >= 0 }\n}\n",
        )
        .expect("write future event source");

        let model_path = workspace.path().join("crates/event_store/src/model.rs");
        let model = fs::read_to_string(&model_path).expect("event-store model source");
        fs::write(
            &model_path,
            model.replacen(
                "pub(crate) mod reconciliation_v1;",
                "pub(crate) mod reconciliation_v1;\npub(crate) mod future_transition_feed_v1;",
                1,
            ),
        )
        .expect("route future event-store source");
        fs::write(
            workspace
                .path()
                .join("crates/event_store/src/model/future_transition_feed_v1.rs"),
            "pub struct FutureTransitionFeedRow {\n    pub event_seq: i64,\n    pub contract_id: String,\n}\n\nimpl FutureTransitionFeedRow {\n    pub fn next_sequence(&self) -> i64 { self.event_seq.saturating_add(1) }\n}\n",
        )
        .expect("write future event-store source");
        let model = fs::read_to_string(&model_path).expect("extended event-store model source");
        fs::write(
            &model_path,
            format!("{model}\npub use future_transition_feed_v1::FutureTransitionFeedRow;\n"),
        )
        .expect("reexport future event-store model");

        let store_path = workspace.path().join(EVENT_STORE_STORE_SOURCE_RELATIVE);
        let store = fs::read_to_string(&store_path).expect("event-store source");
        fs::write(
            &store_path,
            format!(
                "{store}\nimpl RadrootsEventStore {{\n    pub async fn query_future_transition_feed_v1(&self) -> Result<Vec<crate::model::FutureTransitionFeedRow>, RadrootsEventStoreError> {{\n        Ok(Vec::new())\n    }}\n}}\n"
            ),
        )
        .expect("add future event-store query method");
        let lib_path = workspace.path().join(EVENT_STORE_LIB_SOURCE_RELATIVE);
        let lib = fs::read_to_string(&lib_path).expect("event-store lib source");
        fs::write(
            &lib_path,
            format!(
                "{lib}\n#[cfg(feature = \"sqlite\")]\npub use model::FutureTransitionFeedRow;\n"
            ),
        )
        .expect("reexport future event-store query row");

        assert_eq!(
            read_regular_file(workspace.path(), MANIFEST_RELATIVE)
                .expect("unchanged immutable manifest"),
            before,
            "unrelated future product sources must not rotate the immutable NIP-09 v1 hook manifest"
        );
        let error = validate_privileged_store_authority(workspace.path())
            .expect_err("new public export requires explicit structural policy evolution");
        assert!(
            error.contains("public export inventory is closed"),
            "{error}"
        );
        fs::write(&lib_path, lib).expect("restore event-store lib source");
        let error = validate_privileged_store_authority(workspace.path())
            .expect_err("new event-store impl requires explicit structural policy evolution");
        assert!(
            error.contains("event-store inherent impl authority drifted"),
            "{error}"
        );
        let error = validate_governed_support_source_tree_baselines(workspace.path())
            .expect_err("new support source requires an explicit validator baseline review");
        assert!(error.contains("source-tree baseline drifted"), "{error}");
    }

    #[test]
    fn impl_resolution_witness_tracks_core_receivers_and_ignores_post_core_transport() {
        let workspace = synthetic_workspace();
        let baseline =
            describe_impl_resolution_witness(workspace.path()).expect("baseline impl witness");

        let unrelated_path = workspace
            .path()
            .join("crates/transport/src/future_unrelated_projection.rs");
        fs::write(
            &unrelated_path,
            "struct FutureUnrelatedProjection;\nimpl FutureUnrelatedProjection { fn value(&self) -> u8 { 1 } }\n",
        )
        .expect("write unrelated impl");
        assert_eq!(
            describe_impl_resolution_witness(workspace.path()).expect("unrelated impl witness"),
            baseline,
            "an impl on an unrelated future type must not rotate the v1 witness"
        );
        fs::remove_file(&unrelated_path).expect("remove unrelated impl");

        let unrelated_generic_path = workspace
            .path()
            .join("crates/transport/src/future_unrelated_generic_projection.rs");
        fs::write(
            &unrelated_generic_path,
            "struct FutureUnrelatedGenericProjection<T>(T);\nimpl<T> FutureUnrelatedGenericProjection<T> { fn value(&self) -> u8 { 1 } }\n",
        )
        .expect("write unrelated generic nominal impl");
        assert_eq!(
            describe_impl_resolution_witness(workspace.path())
                .expect("unrelated generic nominal impl witness"),
            baseline,
            "an impl on an unrelated local nominal generic type must not rotate the v1 witness"
        );
        fs::remove_file(&unrelated_generic_path).expect("remove unrelated generic nominal impl");

        let blanket_path = workspace
            .path()
            .join("crates/transport/src/future_blanket_projection.rs");
        fs::write(
            &blanket_path,
            "trait FutureBlanketProjection { fn value(&self) -> u8; }\nimpl<T> FutureBlanketProjection for T { fn value(&self) -> u8 { 1 } }\n",
        )
        .expect("write generic blanket impl");
        assert_eq!(
            describe_impl_resolution_witness(workspace.path())
                .expect("generic blanket impl witness"),
            baseline,
            "post-core transport blanket impls must not rotate the core v1 witness"
        );
        fs::remove_file(&blanket_path).expect("remove generic blanket impl");

        let projection_path = workspace
            .path()
            .join("crates/transport/src/future_projection_shadow.rs");
        fs::write(
            &projection_path,
            "trait FutureProjectionCarrier { type Target; }\nstruct FutureProjectionCarrierImpl;\nimpl FutureProjectionCarrier for FutureProjectionCarrierImpl { type Target = alloc::boxed::Box<crate::RadrootsTransportTarget>; }\ntrait FutureProjectionShadow { fn value(&self) -> u8; }\nimpl FutureProjectionShadow for <FutureProjectionCarrierImpl as FutureProjectionCarrier>::Target { fn value(&self) -> u8 { 1 } }\n",
        )
        .expect("write associated projection impl");
        let projection_witness = describe_impl_resolution_witness(workspace.path())
            .expect("associated projection impl witness");
        assert_eq!(
            projection_witness, baseline,
            "post-core transport projections must not rotate the core v1 witness"
        );
        let projection_source =
            fs::read_to_string(&projection_path).expect("associated projection source");
        fs::write(
            &projection_path,
            projection_source.replacen(
                "type Target = alloc::boxed::Box<crate::RadrootsTransportTarget>;",
                "type Target = &'static crate::RadrootsTransportTarget;",
                1,
            ),
        )
        .expect("mutate associated projection target");
        assert_eq!(
            describe_impl_resolution_witness(workspace.path())
                .expect("mutated associated projection impl witness"),
            baseline,
            "changing a post-core transport projection must not rotate the core v1 witness"
        );
        fs::remove_file(&projection_path).expect("remove associated projection impl");

        let alias_path = workspace
            .path()
            .join("crates/transport/src/future_import_alias_shadow.rs");
        fs::write(
            &alias_path,
            "use crate::RadrootsTransportTarget as ProtectedTargetAlias;\ntrait FutureImportAliasShadow { fn value(&self) -> u8; }\nimpl FutureImportAliasShadow for ProtectedTargetAlias { fn value(&self) -> u8 { 1 } }\n",
        )
        .expect("write protected import-alias impl");
        assert_eq!(
            describe_impl_resolution_witness(workspace.path())
                .expect("protected import-alias impl witness"),
            baseline,
            "post-core transport import aliases must not rotate the core v1 witness"
        );
        fs::remove_file(&alias_path).expect("remove protected import-alias impl");
        fs::write(
            &alias_path,
            "use crate::RadrootsTransportTarget as r#type;\ntrait FutureRawImportAliasShadow { fn value(&self) -> u8; }\nimpl FutureRawImportAliasShadow for r#type { fn value(&self) -> u8 { 1 } }\n",
        )
        .expect("write protected raw import-alias impl");
        assert_eq!(
            describe_impl_resolution_witness(workspace.path())
                .expect("protected raw import-alias impl witness"),
            baseline,
            "post-core transport raw import aliases must not rotate the core v1 witness"
        );
        fs::remove_file(&alias_path).expect("remove protected raw import-alias impl");

        let trait_alias_path = workspace
            .path()
            .join("crates/event_store/src/future_trait_alias_shadow.rs");
        fs::write(
            &trait_alias_path,
            "use crate::nip09::reconciliation_v1::SourceGenerationProvider as ProtectedProviderAlias;\nstruct FutureProvider;\nimpl ProtectedProviderAlias for FutureProvider { fn fill_generation(&self, _generation: &mut [u8; 32]) -> Result<(), crate::RadrootsEventStoreError> { unreachable!() } }\n",
        )
        .expect("write protected trait import-alias impl");
        assert_ne!(
            describe_impl_resolution_witness(workspace.path())
                .expect("protected trait import-alias impl witness"),
            baseline,
            "a use-alias impl of a protected trait must rotate the v1 witness"
        );
        fs::remove_file(&trait_alias_path).expect("remove protected trait import-alias impl");

        let target_path = workspace.path().join("crates/transport/src/target.rs");
        let target = fs::read_to_string(&target_path).expect("transport target source");
        let mutations = [
            format!(
                "{target}\nimpl RadrootsTransportTarget {{ pub fn ingest_event(&self) -> bool {{ true }} }}\n"
            ),
            format!(
                "{target}\ntrait ProtectedReferenceShadow {{ fn protected_shadow(&self); }}\nimpl ProtectedReferenceShadow for &RadrootsTransportTarget {{ fn protected_shadow(&self) {{}} }}\n"
            ),
            format!(
                "{target}\ntype ProtectedTargetAlias = RadrootsTransportTarget;\ntrait ProtectedAliasShadow {{ fn protected_shadow(&self); }}\nimpl ProtectedAliasShadow for &mut ProtectedTargetAlias {{ fn protected_shadow(&self) {{}} }}\n"
            ),
            format!(
                "{target}\ntrait ProtectedWrapperShadow {{ fn protected_shadow(&self); }}\nimpl ProtectedWrapperShadow for alloc::boxed::Box<RadrootsTransportTarget> {{ fn protected_shadow(&self) {{}} }}\n"
            ),
            format!(
                "{target}\ntrait ProtectedTraitTerminal {{ fn protected_shadow(&self); }}\nstruct UnrelatedProtectedTraitReceiver;\nimpl ProtectedTraitTerminal for UnrelatedProtectedTraitReceiver {{ fn protected_shadow(&self) {{}} }}\n"
            ),
        ];
        for mutation in mutations {
            fs::write(&target_path, mutation).expect("write protected impl mutation");
            assert_eq!(
                describe_impl_resolution_witness(workspace.path()).expect("protected impl witness"),
                baseline,
                "post-core transport impl changes must not rotate the core v1 witness"
            );
            fs::write(&target_path, &target).expect("restore transport target source");
        }

        let bounded = format!(
            "{target}\nstruct ProtectedGenericWrapper<T>(T);\ntrait ProtectedGenericBound {{}}\nimpl<T: ProtectedGenericBound> ProtectedGenericWrapper<T> {{ pub fn ingest_event(&self) -> bool {{ true }} }}\n"
        );
        fs::write(&target_path, &bounded).expect("write bounded protected inherent impl");
        assert_eq!(
            describe_impl_resolution_witness(workspace.path()).expect("bounded impl witness"),
            baseline,
            "post-core transport generic bounds must not rotate the core v1 witness"
        );
        fs::write(
            &target_path,
            bounded.replacen(
                "impl<T: ProtectedGenericBound> ProtectedGenericWrapper<T>",
                "impl<T> ProtectedGenericWrapper<T>",
                1,
            ),
        )
        .expect("relax protected inherent impl bound");
        assert_eq!(
            describe_impl_resolution_witness(workspace.path()).expect("relaxed impl witness"),
            baseline,
            "relaxing a post-core transport generic bound must not rotate the core v1 witness"
        );
        fs::write(&target_path, target).expect("restore transport target after bound mutation");
    }

    #[test]
    fn predecessor_impl_resolution_projection_is_fail_closed() {
        let workspace = synthetic_workspace();
        let manifest = immutable_manifest();
        let superseded_paths =
            super::super::source_maintenance::predecessor_superseded_source_paths()
                .iter()
                .copied()
                .chain(
                    RAW_SOURCE_REBUILD_PREDECESSOR_SUPERSEDED_PATHS
                        .iter()
                        .copied(),
                )
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
        validate_predecessor_impl_resolution_authority(
            workspace.path(),
            &manifest,
            &superseded_paths,
        )
        .expect("current successor must preserve predecessor impl authority");
        let unchanged_relative = "crates/event_codec/src/error.rs";
        let unchanged_path = workspace.path().join(unchanged_relative);
        let unchanged = fs::read_to_string(&unchanged_path).expect("unchanged predecessor source");

        fs::write(
            &unchanged_path,
            format!(
                "{unchanged}\ntrait UnexpectedPredecessorResolution {{}}\nimpl UnexpectedPredecessorResolution for EventEncodeError {{}}\n"
            ),
        )
        .expect("add unexpected predecessor impl authority");
        let error = validate_predecessor_impl_resolution_authority(
            workspace.path(),
            &manifest,
            &superseded_paths,
        )
        .expect_err("new predecessor-bound impl authority must fail closed");
        assert!(error.contains("unexpected ["), "{error}");
        assert!(error.contains("UnexpectedPredecessorResolution"), "{error}");

        let changed = unchanged.replacen(
            "failed to serialize JSON",
            "failed to serialize canonical JSON",
            1,
        );
        assert_ne!(changed, unchanged, "expected impl fixture must mutate");
        fs::write(&unchanged_path, changed).expect("change expected predecessor impl authority");
        let error = validate_predecessor_impl_resolution_authority(
            workspace.path(),
            &manifest,
            &superseded_paths,
        )
        .expect_err("changed predecessor-bound impl authority must fail closed");
        assert!(error.contains("missing ["), "{error}");
        assert!(error.contains("unexpected ["), "{error}");
        assert!(error.matches("EventEncodeError").count() >= 2, "{error}");
        fs::write(&unchanged_path, unchanged).expect("restore unchanged predecessor source");

        let successor_relative =
            "crates/event_store/src/nip09/reconciliation_v1/visibility_oracle_v1.rs";
        let successor_path = workspace.path().join(successor_relative);
        let successor = fs::read_to_string(&successor_path).expect("successor-only source");
        fs::write(
            successor_path,
            format!(
                "{successor}\ntrait SuccessorOnlyResolution {{}}\nimpl SuccessorOnlyResolution for RadrootsNip09SuppressionDecision {{}}\n"
            ),
        )
        .expect("change successor-only impl authority");
        validate_predecessor_impl_resolution_authority(
            workspace.path(),
            &manifest,
            &superseded_paths,
        )
        .expect("successor-only authority must not rotate predecessor projection");
    }

    #[test]
    fn nip09_v1_manifest_is_independent_of_post_core_transport_evolution() {
        let workspace = synthetic_workspace();
        let before = immutable_manifest();
        let before_bytes = read_regular_file(workspace.path(), MANIFEST_RELATIVE)
            .expect("immutable NIP-09 v1 manifest");

        assert!(
            before
                .cargo_feature_profile
                .packages
                .iter()
                .all(|package| package.package != "radroots_transport")
        );
        assert!(
            before
                .cargo_feature_profile
                .event_store_dependencies
                .iter()
                .all(|dependency| dependency.name != "radroots_transport")
        );
        assert!(
            before
                .runtime_dependency_policy
                .roots
                .iter()
                .all(|root| root.name != "radroots_transport")
        );
        assert!(
            before
                .runtime_dependencies
                .iter()
                .all(|dependency| dependency.name != "radroots_transport")
        );
        assert!(
            before
                .frozen_sources
                .iter()
                .all(|source| !source.path.starts_with("crates/transport/"))
        );
        assert!(
            before
                .impl_resolution_witness
                .roots
                .iter()
                .all(|root| !root.starts_with("crates/transport/"))
        );
        assert!(
            before
                .impl_resolution_witness
                .impls
                .iter()
                .all(|item| !item.path.starts_with("crates/transport/"))
        );

        let transport_manifest_path = workspace.path().join(TRANSPORT_CARGO_MANIFEST_RELATIVE);
        let transport_manifest =
            fs::read_to_string(&transport_manifest_path).expect("transport manifest");
        fs::write(
            &transport_manifest_path,
            transport_manifest.replacen(
                "serde = [\"dep:serde\"]",
                "serde = [\"dep:serde\"]\nphase-09b = []",
                1,
            ),
        )
        .expect("add future transport feature");

        let event_store_manifest_path = workspace.path().join(EVENT_STORE_CARGO_MANIFEST_RELATIVE);
        let event_store_manifest =
            fs::read_to_string(&event_store_manifest_path).expect("event-store manifest");
        fs::write(
            &event_store_manifest_path,
            event_store_manifest.replacen(
                "radroots_transport = { workspace = true, default-features = false }",
                "radroots_transport = { workspace = true, default-features = false, features = [\"phase-09b\"] }",
                1,
            ),
        )
        .expect("enable future transport feature");

        let delivery_path = workspace.path().join("crates/transport/src/delivery.rs");
        let delivery = fs::read_to_string(&delivery_path).expect("transport delivery source");
        fs::write(
            delivery_path,
            format!(
                "{delivery}\n#[derive(Clone, Debug, PartialEq, Eq)]\npub struct FuturePhase09bDelivery;\n"
            ),
        )
        .expect("extend future transport delivery source");

        let target_path = workspace.path().join("crates/transport/src/target.rs");
        let target = fs::read_to_string(&target_path).expect("transport target source");
        fs::write(
            target_path,
            format!(
                "{target}\nimpl RadrootsTransportTarget {{ pub fn future_phase_09b(&self) -> bool {{ true }} }}\n"
            ),
        )
        .expect("extend future transport target source");

        let lib_path = workspace.path().join("crates/transport/src/lib.rs");
        let lib = fs::read_to_string(&lib_path).expect("transport lib source");
        fs::write(
            lib_path,
            format!("{lib}\npub const RADROOTS_TRANSPORT_PHASE_09B: bool = true;\n"),
        )
        .expect("extend future transport facade");

        let cargo_lock_path = workspace.path().join(CARGO_LOCK_RELATIVE);
        let cargo_lock = fs::read_to_string(&cargo_lock_path).expect("Cargo.lock");
        let transport_lock_package = r#"[[package]]
name = "radroots_transport"
version = "0.1.0-alpha"
dependencies = [
 "futures",
 "serde",
 "serde_json",
 "sha2",
]
"#;
        let future_transport_lock_package = r#"[[package]]
name = "radroots_transport"
version = "0.1.0-alpha"
dependencies = [
 "futures",
 "serde",
 "serde_json",
 "sha2",
 "transport-phase-09b",
]

[[package]]
name = "transport-phase-09b"
version = "0.1.0"
"#;
        assert!(
            cargo_lock.contains(transport_lock_package),
            "synthetic Cargo.lock must contain the expected transport package"
        );
        fs::write(
            cargo_lock_path,
            cargo_lock.replacen(transport_lock_package, future_transport_lock_package, 1),
        )
        .expect("extend future transport lock subgraph");

        validate_nip09_reconciliation_manifest(workspace.path())
            .expect("transport evolution must not invalidate immutable predecessor artifacts");
        assert_eq!(
            read_regular_file(workspace.path(), MANIFEST_RELATIVE)
                .expect("unchanged immutable manifest"),
            before_bytes,
            "post-core transport source, feature, and lock evolution must not rotate NIP-09 v1"
        );
    }

    #[test]
    fn support_source_graph_authority_rejects_compiler_source_escapes() {
        let workspace = synthetic_workspace();
        for relative in [
            "crates/core/src/dto.rs",
            "crates/event/src/dto.rs",
            "crates/event/src/ids.rs",
            "crates/event/src/contract/registry_v7.rs",
            "crates/event/src/lib.rs",
            "crates/event_codec/src/manifest.rs",
            "crates/blossom/src/hash.rs",
        ] {
            let bytes =
                fs::read(workspace.path().join(relative)).expect("read governed support source");
            let file = parse_canonical_production_rust(relative, &bytes)
                .expect("parse governed support source");
            validate_support_source_graph_authority(relative, &file)
                .expect("baseline support source graph");
        }

        let relative = "crates/blossom/src/hash.rs";
        let path = workspace.path().join(relative);
        let original = fs::read_to_string(&path).expect("blossom hash source");
        let mutations = [
            (
                format!("{original}\n#[path = \"../escape.rs\"]\nmod escape;\n"),
                "production module source graph drifted",
            ),
            (
                format!(
                    "{original}\n#[cfg_attr(target_os = \"ios\", path = \"../escape.rs\")]\nmod escape;\n"
                ),
                "must not conditionally retarget its source path",
            ),
            (
                format!("{original}\ninclude!(\"../escape.rs\");\n"),
                "compiler macro inputs drifted",
            ),
            (
                format!(
                    "{original}\nmacro_rules! inject_impl {{ () => {{ impl Drop for Sha256 {{ fn drop(&mut self) {{}} }} }} }}\ninject_impl!();\n"
                ),
                "unapproved production item-macro authority",
            ),
        ];
        for (mutation, expected_error) in mutations {
            let file = parse_canonical_production_rust(relative, mutation.as_bytes())
                .expect("parse support source escape mutation");
            let error = validate_support_source_graph_authority(relative, &file)
                .expect_err("support source escape must fail");
            assert!(error.contains(expected_error), "{error}");
        }
    }

    #[test]
    fn validation_rejects_unknown_fields_and_noncanonical_digests() {
        let workspace = synthetic_workspace();
        write_nip09_reconciliation_manifest(workspace.path()).expect("write manifest bundle");

        let manifest_path = workspace.path().join(MANIFEST_RELATIVE);
        let mut manifest: Value =
            serde_json::from_slice(&fs::read(&manifest_path).expect("read manifest"))
                .expect("parse manifest value");
        manifest
            .as_object_mut()
            .expect("manifest object")
            .insert("unknown".to_owned(), Value::Bool(true));
        fs::write(
            &manifest_path,
            canonical_json_bytes(&manifest).expect("serialize invalid manifest"),
        )
        .expect("write invalid manifest");
        let unknown = validate_nip09_reconciliation_manifest(workspace.path())
            .expect_err("unknown manifest fields must fail");
        assert!(unknown.contains("unknown field"));

        write_nip09_reconciliation_manifest(workspace.path()).expect("restore manifest bundle");
        fs::write(
            workspace.path().join(MANIFEST_SHA256_RELATIVE),
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\n",
        )
        .expect("write uppercase digest");
        let uppercase = validate_nip09_reconciliation_manifest(workspace.path())
            .expect_err("uppercase digest must fail");
        assert!(uppercase.contains("stale") || uppercase.contains("lowercase hexadecimal"));
    }

    #[test]
    fn cargo_lock_closure_rejects_ambiguity_and_missing_registry_checksums() {
        let packages = vec![
            CargoLockPackage {
                name: "hex".to_owned(),
                version: "0.4.2".to_owned(),
                source: Some("registry+https://example.test/index".to_owned()),
                checksum: Some(
                    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
                ),
                dependencies: Vec::new(),
            },
            CargoLockPackage {
                name: "hex".to_owned(),
                version: "0.4.3".to_owned(),
                source: Some("registry+https://example.test/index".to_owned()),
                checksum: Some(
                    "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_owned(),
                ),
                dependencies: Vec::new(),
            },
        ];
        let by_name = BTreeMap::from([("hex", vec![0, 1])]);
        let error = resolve_lock_dependency(
            &packages,
            &by_name,
            &CargoLockDependency {
                name: "hex".to_owned(),
                version: None,
                source: None,
            },
        )
        .expect_err("ambiguous dependency must fail");
        assert!(error.contains("ambiguous"));

        let missing_checksum = CargoLockPackage {
            name: "hex".to_owned(),
            version: "0.4.3".to_owned(),
            source: Some("registry+https://example.test/index".to_owned()),
            checksum: None,
            dependencies: Vec::new(),
        };
        let error = runtime_dependency_identity(&missing_checksum)
            .expect_err("registry dependency without checksum must fail");
        assert!(error.contains("missing a checksum"));
    }

    #[test]
    fn result_vector_requires_strict_signed_event_fields() {
        let repository = repository_root();
        let bytes =
            fs::read(repository.join(RESULT_VECTOR_CANONICAL_RELATIVE)).expect("result vector");
        let mut vector: ReconciliationResultVector =
            serde_json::from_slice(&bytes).expect("parse result vector");
        validate_result_vector(&vector).expect("valid result vector");

        vector.cases[0].input_events[0].event.sig =
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
                .to_owned();
        let error = validate_result_vector(&vector).expect_err("uppercase signature must fail");
        assert!(error.contains("lowercase hexadecimal"));
    }

    #[test]
    fn result_vector_requires_complete_durable_state_shape() {
        let repository = repository_root();
        let bytes =
            fs::read(repository.join(RESULT_VECTOR_CANONICAL_RELATIVE)).expect("result vector");
        let vector: Value = serde_json::from_slice(&bytes).expect("result-vector JSON");

        let mut missing_state = vector.clone();
        missing_state["cases"][0]["expected"]
            .as_object_mut()
            .expect("expected object")
            .remove("state");
        assert!(
            serde_json::from_value::<ReconciliationResultVector>(missing_state).is_err(),
            "missing durable state must fail strict generator parsing"
        );

        for field in [
            "kind",
            "pubkey",
            "d_tag",
            "raw_head_event_id",
            "admission_status",
            "contract_id",
            "visibility",
            "nip09_outcome",
            "nip09_reason",
            "event_reference_request_id",
            "address_reference_request_id",
            "address_reference_cutoff",
        ] {
            let mut missing_field = vector.clone();
            missing_field["cases"][0]["expected"]["state"]
                .as_object_mut()
                .expect("state object")
                .remove(field);
            assert!(
                serde_json::from_value::<ReconciliationResultVector>(missing_field).is_err(),
                "missing durable state field `{field}` must fail strict generator parsing"
            );
        }
    }

    #[test]
    fn result_vector_rejects_values_outside_executor_i64_range() {
        let repository = repository_root();
        let bytes =
            fs::read(repository.join(RESULT_VECTOR_CANONICAL_RELATIVE)).expect("result vector");
        let original: Value = serde_json::from_slice(&bytes).expect("result-vector JSON");
        let out_of_range = SQLITE_I64_MAX_U64 + 1;

        for pointer in [
            "/cases/0/expected/coordinate_count",
            "/cases/0/input_events/0/event/created_at",
            "/cases/0/expected/state/address_reference_cutoff",
        ] {
            let mut value = original.clone();
            *value.pointer_mut(pointer).expect("fixture pointer") = Value::from(out_of_range);
            let vector: ReconciliationResultVector =
                serde_json::from_value(value).expect("u64-shaped vector");
            let error = validate_result_vector(&vector)
                .expect_err("executor-incompatible integer must fail generator validation");
            assert!(error.contains("i64 range"), "{error}");
        }
    }

    #[test]
    fn structured_route_witness_rejects_decoys_and_path_retargets() {
        const MODULES: &[ModuleRouteSpec] = &[ModuleRouteSpec {
            visibility: RouteVisibility::Public,
            name: "v1",
        }];
        const USES: &[UseRouteSpec] = &[UseRouteSpec {
            visibility: RouteVisibility::Public,
            path: "v1::*",
        }];
        let spec = SourceRouteWitnessSpec {
            role: "test_route",
            path: "test.rs",
            modules: MODULES,
            uses: USES,
        };

        let comment_decoy = "// pub mod v1;\n// pub use v1::*;\n";
        let error = validate_source_route_source("test.rs", comment_decoy, spec)
            .expect_err("comment decoys must fail");
        assert!(
            error.contains("module route") || error.contains("must not be empty"),
            "{error}"
        );

        let string_decoy = "const ROUTE: &str = \"pub mod v1; pub use v1::*;\";\n";
        let error = validate_source_route_source("test.rs", string_decoy, spec)
            .expect_err("string decoys must fail");
        assert!(error.contains("module route"), "{error}");

        let retarget = "#[path = \"other.rs\"]\npub mod v1;\npub use v1::*;\n";
        let error = validate_source_route_source("test.rs", retarget, spec)
            .expect_err("path retargeting must fail");
        assert!(error.contains("#[path]"));

        let conditional_retarget =
            "#[cfg_attr(target_os = \"ios\", path = \"other.rs\")]\npub mod v1;\npub use v1::*;\n";
        let error = validate_source_route_source("test.rs", conditional_retarget, spec)
            .expect_err("conditional path retargeting must fail");
        assert!(error.contains("#[path]"));

        let target_disabled = "#[cfg(not(target_os = \"ios\"))]\npub mod v1;\npub use v1::*;\n";
        let error = validate_source_route_source("test.rs", target_disabled, spec)
            .expect_err("target-disabled module route must fail");
        assert!(error.contains("attributes drifted"), "{error}");

        let conditional_alternative = "#[cfg(not(target_os = \"ios\"))]\npub mod v1;\n#[cfg(target_os = \"ios\")]\n#[path = \"other.rs\"]\npub mod v1;\npub use v1::*;\n";
        let error = validate_source_route_source("test.rs", conditional_alternative, spec)
            .expect_err("conditional alternative module route must fail");
        assert!(error.contains("exactly one module route"), "{error}");

        let target_disabled_use = "pub mod v1;\n#[cfg(not(target_os = \"ios\"))]\npub use v1::*;\n#[cfg(target_os = \"ios\")]\npub use other::*;\n";
        let error = validate_source_route_source("test.rs", target_disabled_use, spec)
            .expect_err("target-disabled use route must fail");
        assert!(error.contains("attributes drifted"), "{error}");

        let conditional_binding_override =
            "pub mod v1;\npub use v1::*;\n#[cfg(target_os = \"ios\")]\npub use evil::v1;\n";
        let error = validate_source_route_source("test.rs", conditional_binding_override, spec)
            .expect_err("conditional governed-binding override must fail");
        assert!(
            error.contains("shadows a governed route binding")
                || error.contains("must not introduce conditional resolution"),
            "{error}"
        );

        let inline = "pub mod v1 {}\npub use v1::*;\n";
        let error = validate_source_route_source("test.rs", inline, spec)
            .expect_err("inline route substitution must fail");
        assert!(error.contains("implicit external source"));

        let use_comment_decoy = "pub mod v1;\n// pub use v1::*;\n";
        let error = validate_source_route_source("test.rs", use_comment_decoy, spec)
            .expect_err("comment use decoy must fail");
        assert!(error.contains("use route"));

        let use_string_decoy = "pub mod v1;\nconst ROUTE: &str = \"pub use v1::*;\";\n";
        let error = validate_source_route_source("test.rs", use_string_decoy, spec)
            .expect_err("string use decoy must fail");
        assert!(error.contains("use route"));

        validate_source_route_source("test.rs", "pub mod v1;\npub use v1::*;\n", spec)
            .expect("structured module and use routes");
    }

    #[test]
    fn crate_root_routes_reject_unbound_extensions_and_source_injection() {
        for (relative, first_module) in [
            ("crates/event/src/lib.rs", "pub mod account;"),
            ("crates/event_codec/src/lib.rs", "pub mod d_tag;"),
        ] {
            let spec = *SOURCE_ROUTE_WITNESS_SPECS
                .iter()
                .find(|spec| spec.path == relative)
                .expect("crate-root route spec");
            let source = repository_source(relative);
            validate_source_route_source(relative, &source, spec)
                .expect("baseline crate-root routes");

            let benign = format!(
                "{source}\npub mod future_product_surface_v1;\npub use future_product_surface_v1::FutureSurface;\n"
            );
            validate_source_route_source(relative, &benign, spec)
                .expect_err("unbound public crate-root extension must fail closed");

            let mutations = [
                source.replacen(
                    first_module,
                    &format!(
                        "macro_rules! format {{ ($($token:tt)*) => {{ crate::inject() }} }}\n{first_module}"
                    ),
                    1,
                ),
                source.replacen(
                    first_module,
                    &format!("#[macro_use]\n{first_module}"),
                    1,
                ),
                format!("{source}\nextern crate self as core;\n"),
                source.replacen(
                    first_module,
                    &format!(
                        "#[prelude_import]\nuse crate::InjectedPrelude;\n{first_module}"
                    ),
                    1,
                ),
                source.replacen(
                    first_module,
                    &format!("#[evil::inject]\nstruct Injected;\n{first_module}"),
                    1,
                ),
                source.replacen(
                    first_module,
                    &format!("#[derive(evil::Inject)]\nstruct DerivedInjection;\n{first_module}"),
                    1,
                ),
                format!("{source}\npub mod radroots_blossom;\n"),
            ];
            for mutation in mutations {
                assert_ne!(mutation, source, "{relative} fixture must mutate");
                validate_source_route_source(relative, &mutation, spec)
                    .expect_err("crate-root source injection must fail");
            }
        }

        let admission = *SOURCE_ROUTE_WITNESS_SPECS
            .iter()
            .find(|spec| spec.path == "crates/event_codec/src/admission.rs")
            .expect("admission route spec");
        let source = repository_source(admission.path);
        let mutation = format!(
            "{source}\n#[cfg(target_os = \"ios\")]\npub use evil::admit_verified_event_registry_v7;\n"
        );
        let error = validate_source_route_source(admission.path, &mutation, admission)
            .expect_err("conditional admission override must fail");
        assert!(
            error.contains("shadows a governed route binding"),
            "{error}"
        );
        let mutation = format!("{source}\nmod alloc;\n");
        let error = validate_source_route_source(admission.path, &mutation, admission)
            .expect_err("dependency-shadowing facade module must fail");
        assert!(
            error.contains("collides with a governed resolution binding"),
            "{error}"
        );
    }

    #[test]
    fn result_vector_executor_requires_structured_identity_and_digest_bindings() {
        let valid = r#"
pub(super) const RESULT_VECTOR_EXECUTOR_ID: &str =
    "radroots_event_store.nip09_reconciliation_v1.result_vector_executor.v1";
const RESULT_VECTOR_BYTES: &[u8] =
    include_bytes!("../../../tests/fixtures/nip09_reconciliation.v1.json");

#[tokio::test]
async fn nip09_reconciliation_v1_result_vector() {
    assert_eq!(
        RESULT_VECTOR_EXECUTOR_ID,
        nip09_manifest::NIP09_RECONCILIATION_RESULT_VECTOR_EXECUTOR_ID
    );
    assert_eq!(
        sha256_hex(RESULT_VECTOR_BYTES),
        nip09_manifest::NIP09_RECONCILIATION_RESULT_VECTOR_SHA256
    );
    assert_eq!(
        vector.hook_id,
        nip09_manifest::NIP09_RECONCILIATION_HOOK_ID
    );
}
"#;
        validate_result_vector_executor_source("executor.rs", valid)
            .expect("structured executor source");

        let decoy = valid.replace(
            "#[tokio::test]\nasync fn nip09_reconciliation_v1_result_vector()",
            "const DECOY: &str = \"#[tokio::test] async fn nip09_reconciliation_v1_result_vector()\";\nasync fn other()",
        );
        let error = validate_result_vector_executor_source("executor.rs", &decoy)
            .expect_err("string decoy must not satisfy the executor test");
        assert!(error.contains("exactly one function"));

        let wrong_vector = valid.replace(
            "../../../tests/fixtures/nip09_reconciliation.v1.json",
            "../../../tests/fixtures/other.json",
        );
        let error = validate_result_vector_executor_source("executor.rs", &wrong_vector)
            .expect_err("retargeted vector include must fail");
        assert!(error.contains("RESULT_VECTOR_BYTES must include"));

        let generated_id_decoy = valid.replace(
            "nip09_manifest::NIP09_RECONCILIATION_RESULT_VECTOR_EXECUTOR_ID",
            "\"nip09_manifest::NIP09_RECONCILIATION_RESULT_VECTOR_EXECUTOR_ID\"",
        );
        let error = validate_result_vector_executor_source("executor.rs", &generated_id_decoy)
            .expect_err("string executor-id decoy must fail");
        assert!(error.contains("must reference"));
    }

    #[test]
    fn cargo_feature_profile_ignores_unrelated_dev_dependency_edits() {
        let workspace = synthetic_workspace();
        let before =
            describe_cargo_feature_profile(workspace.path()).expect("initial feature profile");
        let manifest_path = workspace.path().join(EVENT_STORE_CARGO_MANIFEST_RELATIVE);
        let mut manifest = fs::read_to_string(&manifest_path).expect("event-store manifest");
        manifest
            .push_str("\n[dev-dependencies.nip09_profile_unrelated_probe]\nversion = \"0.0.0\"\n");
        fs::write(manifest_path, manifest).expect("mutated event-store manifest");

        let after =
            describe_cargo_feature_profile(workspace.path()).expect("mutated feature profile");
        assert_eq!(
            after, before,
            "unrelated manifest edits must not churn the extracted feature profile"
        );
    }

    #[test]
    fn cargo_feature_profile_accepts_empty_markers_and_rejects_noncanonical_enables() {
        let workspace = synthetic_workspace();
        let profile =
            describe_cargo_feature_profile(workspace.path()).expect("governed feature profile");
        let core_std = profile
            .packages
            .iter()
            .find(|package| package.package == "radroots_core")
            .and_then(|package| {
                package
                    .feature_definitions
                    .iter()
                    .find(|definition| definition.name == "std")
            })
            .expect("radroots_core/std feature definition");
        assert!(
            core_std.enables.is_empty(),
            "radroots_core/std is a legitimate empty marker feature"
        );
        validate_cargo_feature_profile_shape(workspace.path(), &profile)
            .expect("empty marker feature shape");

        let mut manifest = immutable_manifest();
        manifest.cargo_feature_profile = profile.clone();
        validate_manifest_json_schema(
            &manifest_schema(),
            &serde_json::to_value(&manifest).expect("manifest value"),
        )
        .expect("empty marker feature schema");

        fn core_default_enables(profile: &mut CargoFeatureProfileDescriptor) -> &mut Vec<String> {
            &mut profile
                .packages
                .iter_mut()
                .find(|package| package.package == "radroots_core")
                .expect("radroots_core package")
                .feature_definitions
                .iter_mut()
                .find(|definition| definition.name == "default")
                .expect("radroots_core/default feature")
                .enables
        }

        let mut duplicate = profile.clone();
        core_default_enables(&mut duplicate).push("std".to_owned());
        let error = validate_cargo_feature_profile_shape(workspace.path(), &duplicate)
            .expect_err("duplicate feature enable must fail");
        assert!(error.contains("strictly sorted"), "{error}");

        let mut noncanonical = profile.clone();
        core_default_enables(&mut noncanonical).reverse();
        let error = validate_cargo_feature_profile_shape(workspace.path(), &noncanonical)
            .expect_err("noncanonical feature order must fail");
        assert!(error.contains("strictly sorted"), "{error}");

        let mut malformed = profile;
        core_default_enables(&mut malformed).insert(0, String::new());
        let error = validate_cargo_feature_profile_shape(workspace.path(), &malformed)
            .expect_err("empty feature enable must fail");
        assert!(error.contains("non-empty"), "{error}");
    }

    #[test]
    fn generated_descriptor_uses_rustfmt_stable_long_string_assignments() {
        let manifest = immutable_manifest();
        let manifest_bytes = IMMUTABLE_MANIFEST_BYTES.to_vec();
        let manifest_sha256 = sha256_hex(&manifest_bytes);
        let descriptor = generated_descriptor(&manifest, &manifest_bytes, &manifest_sha256);

        for (name, value) in [
            (
                "NIP09_RECONCILIATION_MANIFEST_SHA256",
                manifest_sha256.as_str(),
            ),
            (
                "NIP09_RECONCILIATION_MIGRATION_UP_SHA256",
                manifest.migration.up_sha256.as_str(),
            ),
            (
                "NIP09_RECONCILIATION_MIGRATION_DOWN_SHA256",
                manifest.migration.down_sha256.as_str(),
            ),
            (
                "NIP09_RECONCILIATION_SCHEMA_SHA256",
                manifest.migration.schema_sha256.as_str(),
            ),
            (
                "NIP09_RECONCILIATION_RESULT_VECTOR_SHA256",
                manifest.result_vector.sha256.as_str(),
            ),
            (
                "NIP09_RECONCILIATION_RESULT_VECTOR_EXECUTOR_ID",
                manifest.result_vector.executor_id.as_str(),
            ),
            (
                "NIP09_RECONCILIATION_RESULT_VECTOR_EXECUTOR_SHA256",
                manifest.result_vector.executor_sha256.as_str(),
            ),
        ] {
            let expected = format!("pub(crate) const {name}: &str =\n    {value:?};\n");
            assert!(
                descriptor.contains(&expected),
                "{name} must use the rustfmt-stable multiline assignment"
            );
        }
    }

    #[test]
    fn governed_compiler_inputs_reject_build_proc_macro_and_config_injection() {
        let workspace = synthetic_workspace();
        restore_predecessor_compiler_manifest(workspace.path());
        validate_governed_compiler_inputs(workspace.path())
            .expect("baseline governed compiler inputs");

        let toolchain_path = workspace.path().join(RUST_TOOLCHAIN_RELATIVE);
        let toolchain = fs::read_to_string(&toolchain_path).expect("Rust toolchain");
        fs::write(
            &toolchain_path,
            toolchain.replacen("channel = \"1.97.1\"", "channel = \"nightly\"", 1),
        )
        .expect("write toolchain mutation");
        let error = validate_governed_compiler_inputs(workspace.path())
            .expect_err("Rust toolchain mutation must fail");
        assert!(error.contains("exact governed Rust 1.97.1"), "{error}");
        fs::write(&toolchain_path, toolchain).expect("restore Rust toolchain");

        let legacy_toolchain_path = workspace.path().join("rust-toolchain");
        fs::write(&legacy_toolchain_path, "nightly\n").expect("write legacy Rust toolchain");
        let error = validate_governed_compiler_inputs(workspace.path())
            .expect_err("legacy Rust toolchain must fail");
        assert!(error.contains("must remain absent"), "{error}");
        fs::remove_file(legacy_toolchain_path).expect("remove legacy Rust toolchain");

        let build_script = workspace.path().join("crates/event_store/build.rs");
        fs::write(
            &build_script,
            "fn main() { println!(\"cargo:rustc-cfg=injected\"); }\n",
        )
        .expect("write build-script injection");
        let error = validate_governed_compiler_inputs(workspace.path())
            .expect_err("auto-discovered build script must fail");
        assert!(error.contains("auto-discovered build script"), "{error}");
        fs::remove_file(&build_script).expect("remove build-script injection");

        let binary_target = workspace.path().join("crates/event_store/src/main.rs");
        fs::write(&binary_target, "fn main() {}\n").expect("write auto-discovered binary target");
        let error = validate_governed_compiler_inputs(workspace.path())
            .expect_err("auto-discovered binary target must fail");
        assert!(error.contains("auto-discovered binary target"), "{error}");
        fs::remove_file(binary_target).expect("remove auto-discovered binary target");

        let binary_target_directory = workspace.path().join("crates/event_store/src/bin");
        fs::create_dir(&binary_target_directory)
            .expect("write auto-discovered binary target directory");
        let error = validate_governed_compiler_inputs(workspace.path())
            .expect_err("auto-discovered binary target directory must fail");
        assert!(
            error.contains("auto-discovered binary target directory"),
            "{error}"
        );
        fs::remove_dir(binary_target_directory)
            .expect("remove auto-discovered binary target directory");

        let manifest_path = workspace.path().join(EVENT_STORE_CARGO_MANIFEST_RELATIVE);
        let manifest = fs::read_to_string(&manifest_path).expect("event-store manifest");
        fs::write(
            &manifest_path,
            format!("{manifest}\n[build-dependencies.evil_macro]\nversion = \"1\"\n"),
        )
        .expect("write build dependency injection");
        let error = validate_governed_compiler_inputs(workspace.path())
            .expect_err("build dependency must fail");
        assert!(error.contains("target-specific dependency"), "{error}");
        fs::write(&manifest_path, &manifest).expect("restore event-store manifest");

        fs::write(
            &manifest_path,
            format!("{manifest}\n[dependencies.evil_proc_macro]\nversion = \"1\"\n"),
        )
        .expect("write production dependency injection");
        let error = validate_governed_compiler_inputs(workspace.path())
            .expect_err("unbound proc-macro dependency must fail");
        assert!(
            error.contains("compiler dependency and feature tables drifted"),
            "{error}"
        );
        fs::write(&manifest_path, &manifest).expect("restore event-store manifest");

        fs::write(
            &manifest_path,
            format!("{manifest}\n[lib]\npath = \"src/alternate.rs\"\n"),
        )
        .expect("write library target retarget");
        let error = validate_governed_compiler_inputs(workspace.path())
            .expect_err("library target retarget must fail");
        assert!(error.contains("target-path authority"), "{error}");
        fs::write(&manifest_path, &manifest).expect("restore event-store manifest");

        let workspace_manifest_path = workspace.path().join(WORKSPACE_CARGO_MANIFEST_RELATIVE);
        let workspace_manifest =
            fs::read_to_string(&workspace_manifest_path).expect("workspace manifest");
        let repointed_tokio = workspace_manifest.replacen(
            "tokio = { version = \"1\" }",
            "tokio = { path = \"crates/event\", package = \"radroots_event\" }",
            1,
        );
        assert_ne!(
            repointed_tokio, workspace_manifest,
            "tokio workspace fixture must mutate"
        );
        fs::write(&workspace_manifest_path, repointed_tokio)
            .expect("repoint tokio workspace dependency");
        let error = validate_governed_compiler_inputs(workspace.path())
            .expect_err("tokio workspace dependency repoint must fail");
        assert!(
            error.contains("compiler dependency and feature tables drifted"),
            "{error}"
        );
        fs::write(&workspace_manifest_path, workspace_manifest)
            .expect("restore workspace manifest");

        let config_path = workspace.path().join(CARGO_CONFIG_RELATIVE);
        let config = fs::read_to_string(&config_path).expect("Cargo config");
        fs::write(
            &config_path,
            format!("{config}\n[build]\nrustflags = [\"--cfg\", \"injected\"]\n"),
        )
        .expect("write compiler config injection");
        let error = validate_governed_compiler_inputs(workspace.path())
            .expect_err("compiler config injection must fail");
        assert!(error.contains("alias-only"), "{error}");
        fs::write(&config_path, &config).expect("restore Cargo config");

        let legacy_config_path = workspace.path().join(".cargo/config");
        fs::write(
            &legacy_config_path,
            "[build]\nrustflags = [\"--cfg\", \"legacy_injected\"]\n",
        )
        .expect("write legacy Cargo config");
        let error = validate_governed_compiler_inputs(workspace.path())
            .expect_err("legacy Cargo config must fail");
        assert!(error.contains("must remain absent"), "{error}");
        fs::remove_file(legacy_config_path).expect("remove legacy Cargo config");

        let workspace_manifest_path = workspace.path().join(WORKSPACE_CARGO_MANIFEST_RELATIVE);
        let workspace_manifest =
            fs::read_to_string(&workspace_manifest_path).expect("workspace manifest");
        for (label, mutation) in [
            (
                "resolver",
                workspace_manifest.replacen("resolver = \"3\"", "resolver = \"1\"", 1),
            ),
            (
                "edition",
                workspace_manifest.replacen("edition = \"2024\"", "edition = \"2021\"", 1),
            ),
        ] {
            assert_ne!(mutation, workspace_manifest, "{label} fixture must mutate");
            fs::write(&workspace_manifest_path, mutation)
                .expect("write workspace compiler authority mutation");
            let error = validate_governed_compiler_inputs(workspace.path())
                .expect_err("workspace compiler authority mutation must fail");
            assert!(
                error.contains("resolver 3, edition 2024, and rust-version 1.97.1"),
                "{label} produced unexpected error: {error}"
            );
            fs::write(&workspace_manifest_path, &workspace_manifest)
                .expect("restore workspace manifest");
        }

        for (label, mutation, expected_error) in [
            (
                "missing governed member",
                workspace_manifest.replacen("  \"crates/event_store\",\n", "", 1),
                "workspace.members must include governed package root",
            ),
            (
                "workspace exclusion",
                workspace_manifest.replacen(
                    "exclude = [\"fuzz\"]\n",
                    "exclude = [\"crates/event_store\"]\n",
                    1,
                ),
                "must not exclude or narrow",
            ),
            (
                "narrow default members",
                workspace_manifest.replacen(
                    "[workspace]\n",
                    "[workspace]\ndefault-members = [\"tools/xtask\"]\n",
                    1,
                ),
                "must not exclude or narrow",
            ),
        ] {
            assert_ne!(mutation, workspace_manifest, "{label} fixture must mutate");
            fs::write(&workspace_manifest_path, mutation)
                .expect("write workspace membership mutation");
            let error = validate_governed_compiler_inputs(workspace.path())
                .expect_err("workspace membership mutation must fail");
            assert!(
                error.contains(expected_error),
                "{label} produced unexpected error: {error}"
            );
            fs::write(&workspace_manifest_path, &workspace_manifest)
                .expect("restore workspace manifest");
        }
    }

    #[test]
    fn draft_2020_12_schema_is_executed_against_the_manifest() {
        let manifest = serde_json::to_value(immutable_manifest()).expect("manifest value");
        let schema = manifest_schema();
        validate_manifest_json_schema(&schema, &manifest).expect("valid schema instance");

        let mut zero_length = manifest.clone();
        zero_length["result_vector"]["byte_length"] = Value::from(0);
        let error = validate_manifest_json_schema(&schema, &zero_length)
            .expect_err("schema minimum must reject a zero byte length");
        assert!(error.contains("violates"));

        for (pointer, field) in [
            ("/frozen_sources/0", "hash_algorithm"),
            ("/frozen_sources/0", "canonical_byte_length"),
            ("/result_vector", "executor_hash_algorithm"),
            ("/result_vector", "executor_canonical_byte_length"),
        ] {
            let mut missing_identity_field = manifest.clone();
            missing_identity_field
                .pointer_mut(pointer)
                .and_then(Value::as_object_mut)
                .expect("semantic identity object")
                .remove(field);
            let error = validate_manifest_json_schema(&schema, &missing_identity_field)
                .expect_err("semantic identity fields must be required");
            assert!(error.contains("violates"), "{error}");
        }

        let mut wrong_version = manifest;
        wrong_version["profile"]["reconciliation_version"] = Value::from(2);
        let error = validate_manifest_json_schema(&schema, &wrong_version)
            .expect_err("schema const must reject a wrong profile version");
        assert!(error.contains("violates"));

        let mut invalid_schema = schema;
        invalid_schema["type"] = Value::from(7);
        let error = validate_manifest_json_schema(&invalid_schema, &wrong_version)
            .expect_err("invalid schema document must fail meta-schema validation");
        assert!(error.contains("not a valid JSON Schema Draft 2020-12"));
    }

    #[test]
    fn manifest_shape_rejects_generated_frozen_sources() {
        let workspace = synthetic_workspace();
        let mut manifest = immutable_manifest();
        manifest.frozen_sources[0].path = GENERATED_DESCRIPTOR_RELATIVE.to_owned();
        let error = validate_manifest_shape(workspace.path(), &manifest)
            .expect_err("generated source must not be frozen");
        assert!(error.contains("generated or self-describing"));
    }
}
