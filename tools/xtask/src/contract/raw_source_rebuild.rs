use super::artifact_bundle::{GeneratedArtifact, read_regular_file};
use super::food_availability_projection::validate_food_availability_projection_predecessor_production_sources_under_lock;
use super::nip09_reconciliation::{
    canonical_production_rust_bytes, governed_regular_file_inventory,
    validate_current_event_store_successor_authority,
    validate_raw_source_rebuild_successor_compiler_inputs,
};
use super::source_maintenance::validate_source_maintenance_manifest_under_lock;
use quote::ToTokens;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use syn::{Item, UseTree};

const SCHEMA_VERSION: u32 = 1;
const CONTRACT_ID: &str = "radroots_event_store.raw_source_rebuild_v1";
const AUTHORITY_ID: &str = "raw_source_rebuild_v1";
const PREDECESSOR_CONTRACT_ID: &str = "radroots_event_store.source_maintenance_v1";
const PREDECESSOR_MANIFEST_RELATIVE: &str =
    "crates/event_store/contracts/source_maintenance_v1.manifest.json";
const PREDECESSOR_MANIFEST_BYTE_LENGTH: usize = 14_216;
const PREDECESSOR_MANIFEST_SHA256: &str =
    "e8911e6e5710278969cbd15557a5b856b1575dfd11a655711403598370b41221";
const EVENT_STORE_SCHEMA_VERSION: u32 = 4;
const EVENT_CONTRACT_REGISTRY_VERSION: u32 = 7;
const PROJECTION_CURSOR_COUNT_LIMIT: u32 = 4_096;
const PROJECTION_CURSOR_REJECTION_PROBE_LIMIT: u32 = PROJECTION_CURSOR_COUNT_LIMIT + 1;
const CALLER_MAIN_TABLE_COUNT_LIMIT: u32 = 4_096;
const CALLER_FOREIGN_KEY_ROW_COUNT_LIMIT: u32 = 4_096;
const CALLER_INBOUND_FOREIGN_KEY_POLICY: &str =
    "reject_all_rebuild_mutated_parent_dependencies_before_entropy_v1";
const CALLER_SCHEMA_PREFLIGHT_AST_SHA256: &str =
    "81396b4c375ea40c7f928ec5e4599de5b0d64aab51b7e08e84b79ab9dca6ab64";
const COLD_REPAIR_MODE: &str = "canonical_file_only_single_connection_lock_domain_probe_v1";
const TRANSACTION_MODE: &str = "begin_immediate_v1";
const DIGEST_ALGORITHM: &str = "sha256_domain_nul_typed_fields_v1";
const DIGEST_DOMAIN_TERMINATOR: &str = "nul_byte";
const RAW_DIGEST_DOMAIN_UTF8: &str = "radroots:event-store:immutable-raw-digest:v1";
const PRODUCT_DIGEST_DOMAIN_UTF8: &str = "radroots:event-store:active-product-state-digest:v1";
const VISIBILITY_ORACLE: &str = "pure_verified_raw_snapshot_direct_indexed_evidence_v1";
const VISIBILITY_ORACLE_EXPECTED_VISIBILITY_AST_SHA256: &str =
    "88155fb497668bc65d1adb107c8692483c44f8be1be7dea4663772aa6df23897";
const VISIBILITY_ORACLE_DECISION_AST_SHA256: &str =
    "be2034bc552829af7cb8f8f77ce7c0b97e2e23b94551994f6463877ef87c9404";
const RECONCILIATION_REQUEST_INDEX_INSERT_AST_SHA256: &str =
    "81d1e02d42dda1b34fd6ab30873765072d7030abf0bb42f7dc117b88eb206933";
const RECONCILIATION_REQUEST_INDEX_DECISION_AST_SHA256: &str =
    "2920383a13dc1f7f701039147cb3e5595797a5fe68217a7de6d26cb27715add1";
const RECONCILIATION_AFFECTED_COORDINATES_AST_SHA256: &str =
    "8b1aca89e5a20be8f5eb44e08e205e693ba6f2ea0cf4e3a5bd25910f5f4e25cf";
const EVENT_STORE_SUCCESSOR_COMPILER_TABLES_SHA256: &str =
    "10e6177bb51094e1775994e1cb3c7c72d01d71890ce45df6c3129ff3d7ee301c";
const SCOPED_INTEGRITY_MODE: &str = "event_store_owned_tables_and_indices_v1";
const SQLITE_SEQUENCE_SCOPE: &str = "target_first_after_single_shared_sequence_scan_v1";
const HASH_ALGORITHM: &str = "sha256_bytes_v1";
const PRODUCTION_AST_HASH_ALGORITHM: &str = "rust_production_ast_sha256_v1";
const WRITE_COMMAND: &str = "cargo xtask contract raw-source-rebuild-manifest --write";
const EVENT_STORE_PRODUCTION_SOURCES_RELATIVE: &str =
    "contracts/event_store_production_sources.toml";

const MANIFEST_RELATIVE: &str = "crates/event_store/contracts/raw_source_rebuild_v1.manifest.json";
const MANIFEST_SCHEMA_RELATIVE: &str =
    "crates/event_store/contracts/raw_source_rebuild_v1.manifest.schema.json";
const MANIFEST_SHA256_RELATIVE: &str =
    "crates/event_store/contracts/raw_source_rebuild_v1.manifest.sha256";
const GENERATED_DESCRIPTOR_RELATIVE: &str =
    "crates/event_store/src/generated/raw_source_rebuild_manifest.rs";
const RESULT_VECTOR_CANONICAL_RELATIVE: &str =
    "contracts/conformance/vectors/event_store/raw_source_rebuild.v1.json";
const RESULT_VECTOR_MIRROR_RELATIVE: &str =
    "crates/event_store/tests/fixtures/raw_source_rebuild.v1.json";
const RESULT_VECTOR_EXECUTOR_RELATIVE: &str =
    "crates/event_store/tests/raw_source_rebuild_v1_result_vector.rs";
const RESULT_VECTOR_EXECUTOR_ID: &str =
    "radroots_event_store.raw_source_rebuild_v1.result_vector_executor.v1";
const RESULT_VECTOR_EXECUTOR_TEST: &str = "raw_source_rebuild_v1_result_vector";
const REBUILD_RUNTIME_SOURCE_RELATIVE: &str =
    "crates/event_store/src/nip09/reconciliation_v1/raw_source_rebuild.rs";
const RECONCILIATION_RESULT_VECTOR_EXECUTOR_SOURCE_RELATIVE: &str =
    "crates/event_store/src/nip09/reconciliation_v1/result_vector_executor.rs";
const REBUILD_FAILPOINT_TEST_SOURCE_RELATIVE: &str =
    "crates/event_store/src/store/raw_source_rebuild_v1_tests.rs";
const REBUILD_FAILPOINT_TEST: &str = "raw_source_rebuild_failpoints_roll_back_every_stage_v1";
const RESULT_VECTOR_DELEGATED_SUITE_ID: &str =
    "radroots_event_store.raw_source_rebuild_v1.delegated_rust_test_suite.v1";
const RESULT_VECTOR_DELEGATED_SUITE_LANE: &str = "nix run .#contract";
const RESULT_VECTOR_DELEGATED_SUITE_PACKAGE: &str = "radroots_event_store";
const RESULT_VECTOR_BYTE_LENGTH: usize = 26_833;
const RESULT_VECTOR_SHA256: &str =
    "c37a2bf3714f53ab04fae8c5c9dbe2ad4b3f5310efa51f46bd8b116660f1fe15";
const RESULT_VECTOR_DIRECT_CASE_IDS: &[&str] = &[
    "empty_source_repeat_digest_parity",
    "signed_food_fixture_typed_digest_parity",
];
const WORKSPACE_MANIFEST_RELATIVE: &str = "Cargo.toml";
const FLAKE_SOURCE_RELATIVE: &str = "flake.nix";
const FLAKE_LOCK_RELATIVE: &str = "flake.lock";
const CONTRACT_APP_SOURCE_RELATIVE: &str = "build/nix/apps.nix";
const CONTRACT_LANE_SOURCE_RELATIVE: &str = "build/nix/common.nix";
const TOOLCHAIN_ROUTING_SOURCE_RELATIVE: &str = "build/nix/toolchains.nix";
const RUST_TOOLCHAIN_RELATIVE: &str = "rust-toolchain.toml";
const XTASK_MANIFEST_RELATIVE: &str = "tools/xtask/Cargo.toml";
const XTASK_REQUIRED_DISABLED_AUTO_TARGET_FLAGS: &[&str] = &[
    "build",
    "autolib",
    "autobins",
    "autotests",
    "autoexamples",
    "autobenches",
];
const XTASK_FORBIDDEN_AUTO_TARGET_PATHS: &[&str] = &[
    "tools/xtask/build.rs",
    "tools/xtask/src/lib.rs",
    "tools/xtask/src/bin.rs",
    "tools/xtask/src/bin",
    "tools/xtask/tests",
    "tools/xtask/examples",
    "tools/xtask/benches",
];
const CONTRACT_COMMAND_SOURCE_RELATIVE: &str = "tools/xtask/src/contract.rs";
const XTASK_MAIN_SOURCE_RELATIVE: &str = "tools/xtask/src/main.rs";
const RELEASE_RECORD_RELATIVE: &str = "contracts/releases/1.0.0-alpha.1.toml";
const CHANGELOG_RELATIVE: &str = "CHANGELOG.md";
const RELEASE_CHANGE_ID: &str = "event-store-raw-source-rebuild-authority";
const RELEASE_CHANGE_SUMMARY: &str = "Add an authenticated managed-v4 raw-source rebuild and file-only cold-repair authority with stable typed drift categories, serialized generation rotation, independent immutable-raw visibility audit, typed generation-normalized product-state digests, bounded generic projection cursors, a bounded caller-schema dependency preflight over every directly or indirectly mutated parent including the full Food FTS5 table family and sqlite_sequence, target-first transition sequence normalization, separately scoped integrity checks, exact rollback failpoints, a canonical-path SQLite lock-domain probe, deterministic crate-owned connection policy, and an executable successor contract while freezing the SourceMaintenance predecessor and migration inventory.";
const RELEASE_CHANGE_IMPACTS: &[&str] = &[
    "add_exported_type",
    "add_exported_function",
    "add_exported_constant",
    "add_exported_field",
    "add_enum_variant",
    "add_conformance_vector",
    "change_exported_enum_variant",
    "change_exported_algorithm_behavior",
];
const CHANGELOG_RELEASE_MARKER: &str =
    "<!-- release-change: event-store-raw-source-rebuild-authority -->";

const MIGRATION_RELATIVES: &[&str] = &[
    "crates/event_store/migrations/0001_event_store.down.sql",
    "crates/event_store/migrations/0001_event_store.up.sql",
    "crates/event_store/migrations/0002_nip09.down.sql",
    "crates/event_store/migrations/0002_nip09.up.sql",
    "crates/event_store/migrations/0003_food_availability_projection.down.sql",
    "crates/event_store/migrations/0003_food_availability_projection.up.sql",
    "crates/event_store/migrations/0004_source_maintenance.down.sql",
    "crates/event_store/migrations/0004_source_maintenance.up.sql",
];

const REBUILD_STAGES: &[&str] = &[
    "after_marker_open",
    "after_generation_rotation",
    "after_core_replay",
    "after_visibility_audit",
    "after_food_reset_replay",
    "after_food_audit",
    "after_marker_close",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RebuildFailpointSpec {
    id: &'static str,
    variant: &'static str,
    rollback_case_id: &'static str,
}

const REBUILD_FAILPOINTS: &[RebuildFailpointSpec] = &[
    RebuildFailpointSpec {
        id: "after_marker_open",
        variant: "AfterMarkerOpen",
        rollback_case_id: "rollback_after_marker_open",
    },
    RebuildFailpointSpec {
        id: "after_generation_rotation",
        variant: "AfterGenerationRotation",
        rollback_case_id: "rollback_after_generation_rotation",
    },
    RebuildFailpointSpec {
        id: "after_core_replay",
        variant: "AfterCoreReplay",
        rollback_case_id: "rollback_after_core_replay",
    },
    RebuildFailpointSpec {
        id: "after_visibility_audit",
        variant: "AfterVisibilityAudit",
        rollback_case_id: "rollback_after_visibility_audit",
    },
    RebuildFailpointSpec {
        id: "after_food_reset_replay",
        variant: "AfterFoodResetAndReplay",
        rollback_case_id: "rollback_after_food_reset_replay",
    },
    RebuildFailpointSpec {
        id: "after_food_audit",
        variant: "AfterFoodAudit",
        rollback_case_id: "rollback_after_food_audit",
    },
    RebuildFailpointSpec {
        id: "after_marker_close",
        variant: "AfterMarkerClose",
        rollback_case_id: "rollback_after_marker_close",
    },
];

const PRESERVED_AUTHORITIES: &[&str] = &[
    "legacy_listing",
    "trade",
    "transport_observation",
    "generic_projection_cursor",
    "unrelated_caller_state_without_dependencies_on_rebuild_owned_tables",
];

const PRODUCT_DIGEST_COMPONENTS: &[&str] = &[
    "logical_current_classifications",
    "raw_heads",
    "active_addressable_head_state",
    "active_nip09_facts",
    "current_visibility",
    "food_availability_rows",
    "food_availability_images",
    "logical_food_fts_rows",
    "stable_food_cursor_metadata",
];

const PRODUCT_DIGEST_EXCLUSIONS: &[&str] = &[
    "source_generation",
    "absolute_transition_sequence",
    "transition_history",
    "rebuild_origin",
    "rebuild_cause",
    "operational_timestamps",
    "generic_projection_cursors",
    "caller_owned_state",
];

const SCOPED_INTEGRITY_TABLES: &[&str] = &[
    "event_envelopes",
    "event_envelope_tags",
    "event_envelope_head",
    "radroots_event_store_source_generation",
    "radroots_event_store_source_rebuild_commit_barrier",
    "radroots_event_store_source_rebuild_marker",
    "radroots_event_store_source_state",
    "radroots_event_store_write_lock",
    "radroots_event_store_source_capacity_v1",
    "radroots_event_store_event_coordinate",
    "radroots_event_store_nip09_request",
    "radroots_event_store_nip09_event_target",
    "radroots_event_store_nip09_address_target",
    "radroots_event_store_addressable_head_state",
    "radroots_event_store_addressable_head_transition",
    "radroots_event_store_addressable_feed_integrity_v1",
    "radroots_event_store_food_availability_cursor",
    "radroots_event_store_food_availability_projection",
    "radroots_event_store_food_availability_image",
];

const CALLER_INBOUND_FOREIGN_KEY_PARENT_TABLES: &[&str] = &[
    "event_envelopes",
    "event_envelope_tags",
    "event_envelope_head",
    "radroots_event_store_source_generation",
    "radroots_event_store_source_rebuild_commit_barrier",
    "radroots_event_store_source_rebuild_marker",
    "radroots_event_store_source_state",
    "radroots_event_store_write_lock",
    "radroots_event_store_source_capacity_v1",
    "radroots_event_store_event_coordinate",
    "radroots_event_store_nip09_request",
    "radroots_event_store_nip09_event_target",
    "radroots_event_store_nip09_address_target",
    "radroots_event_store_addressable_head_state",
    "radroots_event_store_addressable_head_transition",
    "radroots_event_store_addressable_feed_integrity_v1",
    "radroots_event_store_food_availability_cursor",
    "radroots_event_store_food_availability_projection",
    "radroots_event_store_food_availability_image",
    "radroots_event_store_food_availability_search_fts",
    "radroots_event_store_food_availability_search_fts_config",
    "radroots_event_store_food_availability_search_fts_content",
    "radroots_event_store_food_availability_search_fts_data",
    "radroots_event_store_food_availability_search_fts_docsize",
    "radroots_event_store_food_availability_search_fts_idx",
    "sqlite_sequence",
];

const RAW_DIGEST_QUERY_SPECS: &[DigestQuerySpec] = &[
    DigestQuerySpec {
        section: "event_envelopes",
        sql: "SELECT seq, event_id, pubkey, created_at, kind, tags_json, content, sig, raw_json, inserted_at_ms FROM event_envelopes ORDER BY seq",
        fields: &[
            "seq",
            "event_id",
            "pubkey",
            "created_at",
            "kind",
            "tags_json",
            "content",
            "sig",
            "raw_json",
            "inserted_at_ms",
        ],
    },
    DigestQuerySpec {
        section: "event_envelope_tags",
        sql: "SELECT event.seq, tag.event_id, tag.tag_index, tag.tag_name, tag.tag_value, tag.tag_json FROM event_envelope_tags AS tag JOIN event_envelopes AS event ON event.event_id = tag.event_id ORDER BY event.seq, tag.tag_index",
        fields: &[
            "seq",
            "event_id",
            "tag_index",
            "tag_name",
            "tag_value",
            "tag_json",
        ],
    },
];

const PRODUCT_DIGEST_QUERY_SPECS: &[DigestQuerySpec] = &[
    DigestQuerySpec {
        section: "envelope_classification",
        sql: "SELECT event_id, verification_status, contract_status, contract_id, event_class, projection_eligible FROM event_envelopes ORDER BY event_id",
        fields: &[
            "event_id",
            "verification_status",
            "contract_status",
            "contract_id",
            "event_class",
            "projection_eligible",
        ],
    },
    DigestQuerySpec {
        section: "tag_classification",
        sql: "SELECT event_id, tag_index, contract_semantic, contract_value_type, relay_indexed FROM event_envelope_tags ORDER BY event_id, tag_index",
        fields: &[
            "event_id",
            "tag_index",
            "contract_semantic",
            "contract_value_type",
            "relay_indexed",
        ],
    },
    DigestQuerySpec {
        section: "raw_heads",
        sql: "SELECT coordinate_type, kind, pubkey, d_tag, event_id, created_at FROM event_envelope_head ORDER BY coordinate_type, kind, pubkey, d_tag",
        fields: &[
            "coordinate_type",
            "kind",
            "pubkey",
            "d_tag",
            "event_id",
            "created_at",
        ],
    },
    DigestQuerySpec {
        section: "event_coordinates",
        sql: "SELECT event_id, coordinate_type, kind, pubkey, created_at, admission_status, admission_code, contract_id, raw_d_tag, nip09_matchable, nip09_d_tag FROM radroots_event_store_event_coordinate WHERE source_generation = ? ORDER BY event_id",
        fields: &[
            "event_id",
            "coordinate_type",
            "kind",
            "pubkey",
            "created_at",
            "admission_status",
            "admission_code",
            "contract_id",
            "raw_d_tag",
            "nip09_matchable",
            "nip09_d_tag",
        ],
    },
    DigestQuerySpec {
        section: "nip09_requests",
        sql: "SELECT request_event_id, request_pubkey, request_created_at FROM radroots_event_store_nip09_request WHERE source_generation = ? ORDER BY request_event_id",
        fields: &["request_event_id", "request_pubkey", "request_created_at"],
    },
    DigestQuerySpec {
        section: "nip09_event_targets",
        sql: "SELECT request_event_id, target_event_id, source_tag_index, source_tag_value FROM radroots_event_store_nip09_event_target WHERE source_generation = ? ORDER BY request_event_id, target_event_id, source_tag_index",
        fields: &[
            "request_event_id",
            "target_event_id",
            "source_tag_index",
            "source_tag_value",
        ],
    },
    DigestQuerySpec {
        section: "nip09_address_targets",
        sql: "SELECT request_event_id, target_kind, target_pubkey, target_d_tag, inclusive_cutoff, source_tag_index, source_tag_value, source_kind_text, source_pubkey_text, source_d_tag FROM radroots_event_store_nip09_address_target WHERE source_generation = ? ORDER BY request_event_id, target_kind, target_pubkey, target_d_tag, source_tag_index",
        fields: &[
            "request_event_id",
            "target_kind",
            "target_pubkey",
            "target_d_tag",
            "inclusive_cutoff",
            "source_tag_index",
            "source_tag_value",
            "source_kind_text",
            "source_pubkey_text",
            "source_d_tag",
        ],
    },
    DigestQuerySpec {
        section: "addressable_heads",
        sql: "SELECT kind, pubkey, d_tag, raw_head_event_id, raw_head_created_at, admission_status, admission_code, contract_id, visibility, nip09_outcome, nip09_reason, event_reference_request_id, address_reference_request_id, address_reference_cutoff FROM radroots_event_store_addressable_head_state WHERE source_generation = ? ORDER BY kind, pubkey, d_tag",
        fields: &[
            "kind",
            "pubkey",
            "d_tag",
            "raw_head_event_id",
            "raw_head_created_at",
            "admission_status",
            "admission_code",
            "contract_id",
            "visibility",
            "nip09_outcome",
            "nip09_reason",
            "event_reference_request_id",
            "address_reference_request_id",
            "address_reference_cutoff",
        ],
    },
    DigestQuerySpec {
        section: "current_visibility",
        sql: "SELECT event_id, admission_status, contract_id, event_class, raw_d_tag, is_raw_head, raw_head_event_id, suppression_outcome, suppression_reason, event_reference_request_id, address_reference_request_id, address_reference_cutoff, current_visibility FROM radroots_event_store_current_visibility_v1 WHERE source_generation = ? ORDER BY event_id",
        fields: &[
            "event_id",
            "admission_status",
            "contract_id",
            "event_class",
            "raw_d_tag",
            "is_raw_head",
            "raw_head_event_id",
            "suppression_outcome",
            "suppression_reason",
            "event_reference_request_id",
            "address_reference_request_id",
            "address_reference_cutoff",
            "current_visibility",
        ],
    },
    DigestQuerySpec {
        section: "food_projection",
        sql: "SELECT kind, pubkey, d_tag, event_id, created_at, contract_id, content, title, summary, published_at, location, price_amount, price_currency, price_unit, quantity_amount, quantity_unit, status, diagnostic_codes_json FROM radroots_event_store_food_availability_projection WHERE source_generation = ? ORDER BY pubkey, d_tag",
        fields: &[
            "kind",
            "pubkey",
            "d_tag",
            "event_id",
            "created_at",
            "contract_id",
            "content",
            "title",
            "summary",
            "published_at",
            "location",
            "price_amount",
            "price_currency",
            "price_unit",
            "quantity_amount",
            "quantity_unit",
            "status",
            "diagnostic_codes_json",
        ],
    },
    DigestQuerySpec {
        section: "food_images",
        sql: "SELECT pubkey, d_tag, image_index, raw_tag_json, url, width, height, blossom_sha256, qualifies, diagnostic_codes_json FROM radroots_event_store_food_availability_image WHERE source_generation = ? ORDER BY pubkey, d_tag, image_index",
        fields: &[
            "pubkey",
            "d_tag",
            "image_index",
            "raw_tag_json",
            "url",
            "width",
            "height",
            "blossom_sha256",
            "qualifies",
            "diagnostic_codes_json",
        ],
    },
    DigestQuerySpec {
        section: "food_search",
        sql: "SELECT event_id, pubkey, d_tag, title, summary, content, location FROM radroots_event_store_food_availability_search_fts ORDER BY event_id",
        fields: &[
            "event_id", "pubkey", "d_tag", "title", "summary", "content", "location",
        ],
    },
    DigestQuerySpec {
        section: "food_cursor",
        sql: "SELECT feed_version, projection_version, scope_fingerprint, hook_manifest_sha256, projected_row_count FROM radroots_event_store_food_availability_cursor WHERE singleton = 1",
        fields: &[
            "feed_version",
            "projection_version",
            "scope_fingerprint",
            "hook_manifest_sha256",
            "projected_row_count",
        ],
    },
];

const ADDED_PUBLIC_SYMBOLS: &[&str] = &[
    "RADROOTS_EVENT_STORE_PROJECTION_CURSOR_COUNT_LIMIT_V1",
    "RadrootsEventStoreCallerInboundForeignKeyV1",
    "RadrootsEventStoreActiveProductStateDigestV1",
    "RadrootsEventStoreImmutableRawDigestV1",
    "RadrootsEventStoreRawSourceRebuildDriftV1",
    "RadrootsEventStoreRawSourceRebuildReportV1",
];

const PUBLIC_METHODS: &[&str] = &[
    "RadrootsEventStore::rebuild_from_raw_v1",
    "RadrootsEventStore::repair_file_from_raw_v1",
    "RadrootsEventStoreRawSourceRebuildReportV1::prior_source_generation",
    "RadrootsEventStoreRawSourceRebuildReportV1::new_source_generation",
    "RadrootsEventStoreRawSourceRebuildReportV1::source_capacity",
    "RadrootsEventStoreRawSourceRebuildReportV1::raw_high_water_seq",
    "RadrootsEventStoreRawSourceRebuildReportV1::immutable_raw_digest",
    "RadrootsEventStoreRawSourceRebuildReportV1::active_product_state_digest",
    "RadrootsEventStoreImmutableRawDigestV1::as_bytes",
    "RadrootsEventStoreActiveProductStateDigestV1::as_bytes",
    "RadrootsEventStoreRawSourceRebuildDriftV1::code",
];

const RAW_SOURCE_REBUILD_DRIFT_KINDS: &[(&str, &str)] = &[
    ("ManagedSchemaAuthority", "managed_schema_authority"),
    ("ImmutableRawAuthority", "immutable_raw_authority"),
    ("SourceGenerationLineage", "source_generation_lineage"),
    (
        "AddressableTransitionAuthority",
        "addressable_transition_authority",
    ),
    (
        "DerivedProductStateAuthority",
        "derived_product_state_authority",
    ),
    ("RebuildPostcondition", "rebuild_postcondition"),
];

const ERROR_VARIANTS: &[&str] = &[
    "ProjectionCursorCapacityExceeded",
    "RawSourceRepairDatabaseIdentityMismatch",
    "RawSourceRepairCanonicalPathLockDomainMismatch",
    "RawSourceRepairMainDatabaseCanonicalizationFailed",
    "RawSourceRebuildCallerForeignKeyCapacityExceeded",
    "RawSourceRebuildCallerInboundForeignKeyUnsupported",
    "RawSourceRebuildCallerTableCapacityExceeded",
    "RawSourceRebuildStateDrift",
    "RawSourceRebuildTransactionRollbackFailed",
];

const ENTRY_POINTS: &[(&str, &str)] = &[
    (
        "live_rebuild",
        "radroots_event_store::RadrootsEventStore::rebuild_from_raw_v1",
    ),
    (
        "cold_file_repair",
        "radroots_event_store::RadrootsEventStore::repair_file_from_raw_v1",
    ),
    (
        "projection_cursor_insert_preflight",
        "radroots_event_store::nip09::reconciliation_v1::preflight_projection_cursor_insert_v1",
    ),
    (
        "serialized_rebuild_runtime",
        "radroots_event_store::nip09::reconciliation_v1::raw_source_rebuild::rebuild_from_raw_v1_on_pool",
    ),
    (
        "independent_visibility_oracle",
        "radroots_event_store::nip09::reconciliation_v1::visibility_oracle_v1::audit_current_visibility_from_raw_v1",
    ),
    ("result_vector_executor", RESULT_VECTOR_EXECUTOR_TEST),
];

#[derive(Clone, Copy)]
struct SourceSpec {
    role: &'static str,
    path: &'static str,
}

#[derive(Clone, Copy)]
struct DigestQuerySpec {
    section: &'static str,
    sql: &'static str,
    fields: &'static [&'static str],
}

const DELEGATED_COMPILER_SOURCE_PINS: &[(&str, &str)] = &[
    (
        FLAKE_SOURCE_RELATIVE,
        "0251b26040cf5338c12dc777a4deaadb8f63eb4e88bc05929dcec67db88ff2bf",
    ),
    (
        FLAKE_LOCK_RELATIVE,
        "41b569739bfa0c488625326f4f0a874561601787951cdf7a3f171e60572fa20e",
    ),
    (
        CONTRACT_APP_SOURCE_RELATIVE,
        "41a185ac87379e24c1ede09c0f1aac820653dffc09f99cd803b145b44bed982c",
    ),
    (
        CONTRACT_LANE_SOURCE_RELATIVE,
        "b3340e1b4973e6a1e02899d164ca74842757f22b6b1a03f90461532fcd844df5",
    ),
    (
        TOOLCHAIN_ROUTING_SOURCE_RELATIVE,
        "cd664be945e28bf6c25c7758182ff8d01e03248832dfc2c045c01b4f4aff960f",
    ),
    (
        RUST_TOOLCHAIN_RELATIVE,
        "c33aa38292bab6513bf79ed2f69c1525b736dd738b15ca78af713b70b29265c9",
    ),
    (
        XTASK_MANIFEST_RELATIVE,
        "b915e0289bf7390d3c4194aaed1e748cf5e591426e0443c310775ddc7d7f63a5",
    ),
];

const REQUIRED_DELEGATED_COMPILER_SOURCES: &[(&str, &str)] = &[
    ("workspace_manifest_authority", WORKSPACE_MANIFEST_RELATIVE),
    ("workspace_lockfile_authority", "Cargo.lock"),
    ("nix_flake_app_export_authority", FLAKE_SOURCE_RELATIVE),
    ("nix_input_lock_authority", FLAKE_LOCK_RELATIVE),
    (
        "nix_contract_app_routing_authority",
        CONTRACT_APP_SOURCE_RELATIVE,
    ),
    (
        "nix_contract_test_lane_authority",
        CONTRACT_LANE_SOURCE_RELATIVE,
    ),
    (
        "nix_toolchain_routing_authority",
        TOOLCHAIN_ROUTING_SOURCE_RELATIVE,
    ),
    ("rust_toolchain_authority", RUST_TOOLCHAIN_RELATIVE),
    ("xtask_manifest_authority", XTASK_MANIFEST_RELATIVE),
];

const SOURCE_SPECS: &[SourceSpec] = &[
    SourceSpec {
        role: "workspace_manifest_authority",
        path: WORKSPACE_MANIFEST_RELATIVE,
    },
    SourceSpec {
        role: "workspace_lockfile_authority",
        path: "Cargo.lock",
    },
    SourceSpec {
        role: "nix_flake_app_export_authority",
        path: FLAKE_SOURCE_RELATIVE,
    },
    SourceSpec {
        role: "nix_input_lock_authority",
        path: FLAKE_LOCK_RELATIVE,
    },
    SourceSpec {
        role: "nix_contract_app_routing_authority",
        path: CONTRACT_APP_SOURCE_RELATIVE,
    },
    SourceSpec {
        role: "nix_contract_test_lane_authority",
        path: CONTRACT_LANE_SOURCE_RELATIVE,
    },
    SourceSpec {
        role: "nix_toolchain_routing_authority",
        path: TOOLCHAIN_ROUTING_SOURCE_RELATIVE,
    },
    SourceSpec {
        role: "rust_toolchain_authority",
        path: RUST_TOOLCHAIN_RELATIVE,
    },
    SourceSpec {
        role: "xtask_manifest_authority",
        path: XTASK_MANIFEST_RELATIVE,
    },
    SourceSpec {
        role: "event_store_dependency_feature_authority",
        path: "crates/event_store/Cargo.toml",
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
        role: "food_generated_descriptor_input",
        path: "crates/event_store/src/generated/food_availability_projection_manifest.rs",
    },
    SourceSpec {
        role: "nip09_generated_descriptor_input",
        path: "crates/event_store/src/generated/nip09_reconciliation_manifest.rs",
    },
    SourceSpec {
        role: "source_maintenance_generated_descriptor_input",
        path: "crates/event_store/src/generated/source_maintenance_manifest.rs",
    },
    SourceSpec {
        role: "public_surface",
        path: "crates/event_store/src/lib.rs",
    },
    SourceSpec {
        role: "migration_runtime_registry",
        path: "crates/event_store/src/migrations.rs",
    },
    SourceSpec {
        role: "model_registration",
        path: "crates/event_store/src/model.rs",
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
        role: "food_availability_projection_model",
        path: "crates/event_store/src/model/food_availability_projection_v1.rs",
    },
    SourceSpec {
        role: "ingest_reconciliation_model",
        path: "crates/event_store/src/model/ingest_reconciliation_v1.rs",
    },
    SourceSpec {
        role: "rebuild_report_and_digest_models",
        path: "crates/event_store/src/model/raw_source_rebuild_v1.rs",
    },
    SourceSpec {
        role: "reconciliation_model",
        path: "crates/event_store/src/model/reconciliation_v1.rs",
    },
    SourceSpec {
        role: "nip09_module_registration",
        path: "crates/event_store/src/nip09.rs",
    },
    SourceSpec {
        role: "reconciliation_runtime_registration",
        path: "crates/event_store/src/nip09/reconciliation_v1.rs",
    },
    SourceSpec {
        role: "serialized_raw_source_rebuild",
        path: REBUILD_RUNTIME_SOURCE_RELATIVE,
    },
    SourceSpec {
        role: "nip09_result_vector_executor_input",
        path: "crates/event_store/src/nip09/reconciliation_v1/result_vector_executor.rs",
    },
    SourceSpec {
        role: "independent_raw_visibility_oracle",
        path: "crates/event_store/src/nip09/reconciliation_v1/visibility_oracle_v1.rs",
    },
    SourceSpec {
        role: "managed_v4_validation_and_scoped_integrity",
        path: "crates/event_store/src/schema.rs",
    },
    SourceSpec {
        role: "source_capacity_rebuild_authority",
        path: "crates/event_store/src/source_maintenance_v1.rs",
    },
    SourceSpec {
        role: "public_rebuild_and_cold_repair_boundary",
        path: "crates/event_store/src/store.rs",
    },
    SourceSpec {
        role: "addressable_transition_feed_storage",
        path: "crates/event_store/src/store/addressable_transition_feed_v1.rs",
    },
    SourceSpec {
        role: "current_visibility_storage",
        path: "crates/event_store/src/store/current_visibility_v1.rs",
    },
    SourceSpec {
        role: "raw_source_rebuild_focused_tests",
        path: "crates/event_store/src/store/raw_source_rebuild_v1_tests.rs",
    },
    SourceSpec {
        role: "signed_food_digest_fixture",
        path: "crates/event_store/tests/fixtures/food_availability_projection.v1.json",
    },
    SourceSpec {
        role: "food_projection_reset_and_replay",
        path: "crates/event_store/src/store/food_availability_projection_v1.rs",
    },
    SourceSpec {
        role: "post_core_extension_capabilities",
        path: "crates/event_store/src/store/post_core_extension_capabilities.rs",
    },
    SourceSpec {
        role: "post_core_extension_dispatcher",
        path: "crates/event_store/src/store/post_core_extension_dispatcher.rs",
    },
    SourceSpec {
        role: "post_core_extensions_v1",
        path: "crates/event_store/src/store/post_core_extensions_v1.rs",
    },
    SourceSpec {
        role: "post_core_extensions_v2",
        path: "crates/event_store/src/store/post_core_extensions_v2.rs",
    },
    SourceSpec {
        role: "post_core_storage_v1",
        path: "crates/event_store/src/store/post_core_storage_v1.rs",
    },
    SourceSpec {
        role: "post_core_storage_v2",
        path: "crates/event_store/src/store/post_core_storage_v2.rs",
    },
    SourceSpec {
        role: "protocol_reconciliation_storage",
        path: "crates/event_store/src/store/protocol_reconciliation_v1.rs",
    },
    SourceSpec {
        role: "protocol_storage_boundary",
        path: "crates/event_store/src/store/protocol_storage_v1.rs",
    },
    SourceSpec {
        role: "event_store_package_readme",
        path: "crates/event_store/README",
    },
    SourceSpec {
        role: "signed_nip09_reconciliation_fixture",
        path: "crates/event_store/tests/fixtures/nip09_reconciliation.v1.json",
    },
    SourceSpec {
        role: "transitive_food_predecessor_governance",
        path: "tools/xtask/src/contract/food_availability_projection.rs",
    },
    SourceSpec {
        role: "immutable_predecessor_governance",
        path: "tools/xtask/src/contract/source_maintenance.rs",
    },
    SourceSpec {
        role: "transitive_nip09_predecessor_governance",
        path: "tools/xtask/src/contract/nip09_reconciliation.rs",
    },
    SourceSpec {
        role: "raw_source_rebuild_governance",
        path: "tools/xtask/src/contract/raw_source_rebuild.rs",
    },
    SourceSpec {
        role: "contract_command_authority",
        path: CONTRACT_COMMAND_SOURCE_RELATIVE,
    },
    SourceSpec {
        role: "xtask_dispatch_and_release_preflight",
        path: XTASK_MAIN_SOURCE_RELATIVE,
    },
    SourceSpec {
        role: "release_breaking_change_authority",
        path: RELEASE_RECORD_RELATIVE,
    },
    SourceSpec {
        role: "release_note_authority",
        path: CHANGELOG_RELATIVE,
    },
];

const EXPECTED_SOURCE_MAINTENANCE_DRIFT_PATHS: &[&str] = &[
    "crates/event_store/src/error.rs",
    "crates/event_store/src/generated.rs",
    "crates/event_store/src/lib.rs",
    "crates/event_store/src/model.rs",
    "crates/event_store/src/nip09/reconciliation_v1.rs",
    "crates/event_store/src/schema.rs",
    "crates/event_store/src/store.rs",
    "tools/xtask/src/contract/food_availability_projection.rs",
    "tools/xtask/src/contract/nip09_reconciliation.rs",
    "tools/xtask/src/contract/source_maintenance.rs",
    "tools/xtask/src/contract.rs",
    "tools/xtask/src/main.rs",
];

const TRANSITIVE_PREDECESSOR_SUPERSEDED_PATHS: &[&str] = &[
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
const BLOSSOM_READINESS_SUCCESSOR_TRANSITIVE_PATHS: &[&str] = &[
    "Cargo.toml",
    "crates/blossom/Cargo.toml",
    "crates/blossom/src/error.rs",
    "crates/blossom/src/lib.rs",
    "crates/blossom/src/url.rs",
];
#[cfg(test)]
const RASTER_DECODER_SECURITY_SUCCESSOR_DELEGATED_COMPILER_PATHS: &[&str] = &[
    CONTRACT_APP_SOURCE_RELATIVE,
    CONTRACT_LANE_SOURCE_RELATIVE,
    TOOLCHAIN_ROUTING_SOURCE_RELATIVE,
];

const GENERATED_ARTIFACT_PATHS: &[&str] = &[
    MANIFEST_RELATIVE,
    MANIFEST_SCHEMA_RELATIVE,
    MANIFEST_SHA256_RELATIVE,
    GENERATED_DESCRIPTOR_RELATIVE,
    RESULT_VECTOR_MIRROR_RELATIVE,
];

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct RawSourceRebuildManifest {
    schema_version: u32,
    contract_id: String,
    authority_id: String,
    manifest_schema: FileDescriptor,
    predecessor: PredecessorDescriptor,
    migration_inventory: Vec<FileDescriptor>,
    runtime: RuntimeDescriptor,
    entry_points: Vec<EntryPointDescriptor>,
    source_files: Vec<SourceFileDescriptor>,
    public_api: PublicApiDescriptor,
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
    contract_id: String,
    manifest: FileDescriptor,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct RuntimeDescriptor {
    event_store_schema_version: u32,
    event_contract_registry_version: u32,
    transaction_mode: String,
    projection_cursor_count_limit: u32,
    projection_cursor_rejection_probe_limit: u32,
    caller_main_table_count_limit: u32,
    caller_foreign_key_row_count_limit: u32,
    caller_inbound_foreign_key_policy: String,
    caller_inbound_foreign_key_parent_tables: Vec<String>,
    cold_repair_mode: String,
    immutable_raw_digest: DigestDescriptor,
    active_product_state_digest: ProductDigestDescriptor,
    visibility_oracle: String,
    scoped_integrity_mode: String,
    scoped_integrity_tables: Vec<String>,
    sqlite_sequence_scope: String,
    stages: Vec<String>,
    failpoints: Vec<String>,
    preserved_authorities: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct DigestDescriptor {
    algorithm: String,
    domain_utf8: String,
    domain_terminator: String,
    framing: DigestFramingDescriptor,
    output_bytes: u32,
    source_queries: Vec<DigestQueryDescriptor>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ProductDigestDescriptor {
    algorithm: String,
    domain_utf8: String,
    domain_terminator: String,
    framing: DigestFramingDescriptor,
    output_bytes: u32,
    components: Vec<String>,
    exclusions: Vec<String>,
    component_queries: Vec<DigestQueryDescriptor>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct DigestFramingDescriptor {
    section: String,
    row: String,
    signed_i64: String,
    boolean: String,
    optional: String,
    text: String,
    blob: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct DigestQueryDescriptor {
    section: String,
    sql: String,
    fields: Vec<DigestFieldDescriptor>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct DigestFieldDescriptor {
    name: String,
    framing: String,
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct EventStoreProductionSourceInventory {
    schema_version: u32,
    hash_algorithm: String,
    sources: Vec<EventStoreProductionSourceBaseline>,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(deny_unknown_fields)]
struct EventStoreProductionSourceBaseline {
    path: String,
    sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct PublicApiDescriptor {
    added_symbols: Vec<String>,
    methods: Vec<String>,
    error_variants: Vec<String>,
    drift_kinds: Vec<DriftKindDescriptor>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct DriftKindDescriptor {
    variant: String,
    code: String,
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct RawSourceRebuildVector {
    schema_version: u32,
    contract_id: String,
    delegated_suite: DelegatedSuite,
    cases: Vec<VectorCase>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct DelegatedSuite {
    id: String,
    lane: String,
    package: String,
    authorities: Vec<DelegatedAuthority>,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
struct DelegatedAuthority {
    authority: String,
    authority_path: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct VectorCase {
    id: String,
    execution: String,
    authority: String,
    authority_path: String,
    expected_outcome: String,
    expected_immutable_raw_digest: Option<String>,
    expected_active_product_state_digest: Option<String>,
}

pub(crate) fn write_raw_source_rebuild_manifest(workspace_root: &Path) -> Result<(), String> {
    validate_raw_source_rebuild_manifest(workspace_root)
}

pub(crate) fn validate_raw_source_rebuild_manifest(workspace_root: &Path) -> Result<(), String> {
    // Keep the frozen predecessor validator compiled for its governed mutation suite.
    let _immutable_predecessor_validator: fn(&Path) -> Result<(), String> =
        validate_raw_source_rebuild_manifest_under_lock;
    super::blossom_publication_readiness::validate_blossom_publication_readiness(workspace_root)
}

pub(super) fn validate_event_store_production_source_authority(
    workspace_root: &Path,
) -> Result<(), String> {
    let manifest_bytes = read_regular_file(workspace_root, MANIFEST_RELATIVE)?;
    let manifest: RawSourceRebuildManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| format!("parse {MANIFEST_RELATIVE}: {error}"))?;
    validate_manifest_shape(&manifest)?;
    validate_event_store_production_source_inventory(workspace_root, &manifest).map(|_| ())
}

pub(super) fn validate_raw_source_rebuild_predecessor_production_sources_under_lock(
    workspace_root: &Path,
    raw_superseded_paths: &[&str],
    transitive_superseded_paths: &[&str],
) -> Result<(), String> {
    let manifest_bytes = read_regular_file(workspace_root, MANIFEST_RELATIVE)?;
    let manifest: RawSourceRebuildManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| format!("parse {MANIFEST_RELATIVE}: {error}"))?;
    validate_manifest_shape(&manifest)?;
    let semantic_sources =
        validate_event_store_production_source_inventory(workspace_root, &manifest)?;

    let superseded = raw_superseded_paths
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if superseded.len() != raw_superseded_paths.len() {
        return Err("raw-source rebuild successor supersession paths must be unique".to_owned());
    }
    let predecessor_paths = manifest
        .source_files
        .iter()
        .map(|source| source.path.as_str())
        .collect::<BTreeSet<_>>();
    if let Some(path) = superseded
        .iter()
        .find(|path| !predecessor_paths.contains(**path))
    {
        return Err(format!(
            "raw-source rebuild successor supersession path `{path}` is not predecessor-bound"
        ));
    }

    for source in &manifest.source_files {
        if superseded.contains(source.path.as_str()) {
            continue;
        }
        if semantic_sources.contains(source.path.as_str()) {
            continue;
        }
        let current = read_regular_file(workspace_root, &source.path)?;
        if current.len() as u64 != source.byte_length || sha256_hex(&current) != source.sha256 {
            return Err(format!(
                "unchanged raw-source rebuild predecessor source `{}` drifted",
                source.path
            ));
        }
    }

    let transitive = TRANSITIVE_PREDECESSOR_SUPERSEDED_PATHS
        .iter()
        .copied()
        .chain(transitive_superseded_paths.iter().copied())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    validate_food_availability_projection_predecessor_production_sources_under_lock(
        workspace_root,
        &transitive,
    )
}

fn validate_event_store_production_source_inventory(
    workspace_root: &Path,
    manifest: &RawSourceRebuildManifest,
) -> Result<BTreeSet<String>, String> {
    let source_bytes = read_regular_file(workspace_root, EVENT_STORE_PRODUCTION_SOURCES_RELATIVE)?;
    let source = std::str::from_utf8(&source_bytes).map_err(|error| {
        format!("{EVENT_STORE_PRODUCTION_SOURCES_RELATIVE} must be UTF-8 TOML: {error}")
    })?;
    let inventory: EventStoreProductionSourceInventory = toml::from_str(&source)
        .map_err(|error| format!("parse {EVENT_STORE_PRODUCTION_SOURCES_RELATIVE}: {error}"))?;
    if inventory.schema_version != 1 || inventory.hash_algorithm != PRODUCTION_AST_HASH_ALGORITHM {
        return Err(format!(
            "{EVENT_STORE_PRODUCTION_SOURCES_RELATIVE} has unsupported identity"
        ));
    }
    if inventory
        .sources
        .windows(2)
        .any(|pair| pair[0].path >= pair[1].path)
    {
        return Err(format!(
            "{EVENT_STORE_PRODUCTION_SOURCES_RELATIVE} source paths must be strictly sorted and unique"
        ));
    }

    let expected_paths = manifest
        .source_files
        .iter()
        .filter(|source| is_semantic_event_store_production_source(&source.path))
        .map(|source| source.path.clone())
        .collect::<BTreeSet<_>>();
    let actual_paths = inventory
        .sources
        .iter()
        .map(|source| source.path.clone())
        .collect::<BTreeSet<_>>();
    if actual_paths != expected_paths {
        return Err(format!(
            "{EVENT_STORE_PRODUCTION_SOURCES_RELATIVE} source inventory drifted; expected {expected_paths:?}, found {actual_paths:?}"
        ));
    }

    for baseline in &inventory.sources {
        validate_sha256("event-store production source baseline", &baseline.sha256)?;
        let bytes = read_regular_file(workspace_root, &baseline.path)?;
        let canonical = canonical_production_rust_bytes(&baseline.path, &bytes)?;
        let actual_sha256 = sha256_hex(&canonical);
        if actual_sha256 != baseline.sha256 {
            return Err(format!(
                "{} production Rust authority drifted: expected {}, found {actual_sha256}",
                baseline.path, baseline.sha256
            ));
        }
    }
    Ok(actual_paths)
}

fn is_semantic_event_store_production_source(path: &str) -> bool {
    path.starts_with("crates/event_store/src/")
        && path.ends_with(".rs")
        && path != "crates/event_store/src/generated.rs"
        && !path.starts_with("crates/event_store/src/generated/")
        && path != RECONCILIATION_RESULT_VECTOR_EXECUTOR_SOURCE_RELATIVE
        && path != REBUILD_FAILPOINT_TEST_SOURCE_RELATIVE
}

fn validate_raw_source_rebuild_manifest_under_lock(workspace_root: &Path) -> Result<(), String> {
    validate_source_maintenance_manifest_under_lock(workspace_root)?;
    for artifact in expected_artifacts(workspace_root)? {
        let actual = read_regular_file(workspace_root, artifact.relative)?;
        if actual != artifact.contents {
            return Err(format!(
                "generated raw-source rebuild artifact {} is stale; run `{WRITE_COMMAND}`",
                artifact.relative
            ));
        }
    }

    let manifest_bytes = read_regular_file(workspace_root, MANIFEST_RELATIVE)?;
    let manifest_value: Value = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| format!("parse {MANIFEST_RELATIVE}: {error}"))?;
    let manifest: RawSourceRebuildManifest = serde_json::from_value(manifest_value.clone())
        .map_err(|error| format!("parse typed {MANIFEST_RELATIVE}: {error}"))?;
    validate_canonical_json(MANIFEST_RELATIVE, &manifest_bytes, &manifest)?;
    validate_manifest_shape(&manifest)?;

    let schema_bytes = read_regular_file(workspace_root, MANIFEST_SCHEMA_RELATIVE)?;
    let schema: Value = serde_json::from_slice(&schema_bytes)
        .map_err(|error| format!("parse {MANIFEST_SCHEMA_RELATIVE}: {error}"))?;
    validate_canonical_json(MANIFEST_SCHEMA_RELATIVE, &schema_bytes, &schema)?;
    validate_json_schema(&schema, &manifest_value)?;

    let sidecar = read_regular_file(workspace_root, MANIFEST_SHA256_RELATIVE)?;
    validate_digest_sidecar(MANIFEST_SHA256_RELATIVE, &sidecar)?;
    if sidecar != format!("{}\n", sha256_hex(&manifest_bytes)).as_bytes() {
        return Err(format!(
            "{MANIFEST_SHA256_RELATIVE} must match the checked-in manifest bytes"
        ));
    }

    let vector_bytes = read_regular_file(workspace_root, RESULT_VECTOR_CANONICAL_RELATIVE)?;
    validate_result_vector_identity(&vector_bytes)?;
    let mirror_bytes = read_regular_file(workspace_root, RESULT_VECTOR_MIRROR_RELATIVE)?;
    if vector_bytes != mirror_bytes {
        return Err(format!(
            "{RESULT_VECTOR_MIRROR_RELATIVE} must exactly mirror {RESULT_VECTOR_CANONICAL_RELATIVE}"
        ));
    }
    let vector: RawSourceRebuildVector = serde_json::from_slice(&vector_bytes)
        .map_err(|error| format!("parse {RESULT_VECTOR_CANONICAL_RELATIVE}: {error}"))?;
    validate_canonical_json(RESULT_VECTOR_CANONICAL_RELATIVE, &vector_bytes, &vector)?;
    validate_result_vector(workspace_root, &vector)?;
    validate_source_contract(workspace_root)
}

fn expected_artifacts(workspace_root: &Path) -> Result<Vec<GeneratedArtifact>, String> {
    let schema_bytes = canonical_json_bytes(&manifest_schema())?;
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
) -> Result<RawSourceRebuildManifest, String> {
    validate_source_maintenance_manifest_under_lock(workspace_root)?;
    validate_source_contract(workspace_root)?;

    let predecessor_bytes = read_regular_file(workspace_root, PREDECESSOR_MANIFEST_RELATIVE)?;
    validate_predecessor_identity(&predecessor_bytes)?;
    validate_predecessor_source_supersession(workspace_root, &predecessor_bytes)?;

    let vector_bytes = read_regular_file(workspace_root, RESULT_VECTOR_CANONICAL_RELATIVE)?;
    validate_result_vector_identity(&vector_bytes)?;
    let vector: RawSourceRebuildVector = serde_json::from_slice(&vector_bytes)
        .map_err(|error| format!("parse {RESULT_VECTOR_CANONICAL_RELATIVE}: {error}"))?;
    validate_canonical_json(RESULT_VECTOR_CANONICAL_RELATIVE, &vector_bytes, &vector)?;
    validate_result_vector(workspace_root, &vector)?;

    let source_files = SOURCE_SPECS
        .iter()
        .map(|spec| {
            let bytes = read_regular_file(workspace_root, spec.path)?;
            Ok(SourceFileDescriptor {
                role: spec.role.to_owned(),
                path: spec.path.to_owned(),
                byte_length: byte_length(spec.path, &bytes)?,
                sha256: sha256_hex(&bytes),
                hash_algorithm: HASH_ALGORITHM.to_owned(),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    let executor = descriptor_for_file(workspace_root, RESULT_VECTOR_EXECUTOR_RELATIVE)?;
    Ok(RawSourceRebuildManifest {
        schema_version: SCHEMA_VERSION,
        contract_id: CONTRACT_ID.to_owned(),
        authority_id: AUTHORITY_ID.to_owned(),
        manifest_schema: descriptor_for_bytes(MANIFEST_SCHEMA_RELATIVE, schema_bytes)?,
        predecessor: PredecessorDescriptor {
            contract_id: PREDECESSOR_CONTRACT_ID.to_owned(),
            manifest: descriptor_for_bytes(PREDECESSOR_MANIFEST_RELATIVE, &predecessor_bytes)?,
        },
        migration_inventory: MIGRATION_RELATIVES
            .iter()
            .map(|relative| descriptor_for_file(workspace_root, relative))
            .collect::<Result<Vec<_>, _>>()?,
        runtime: expected_runtime(),
        entry_points: ENTRY_POINTS
            .iter()
            .map(|(role, rust_path)| EntryPointDescriptor {
                role: (*role).to_owned(),
                rust_path: (*rust_path).to_owned(),
            })
            .collect(),
        source_files,
        public_api: PublicApiDescriptor {
            added_symbols: owned(ADDED_PUBLIC_SYMBOLS),
            methods: owned(PUBLIC_METHODS),
            error_variants: owned(ERROR_VARIANTS),
            drift_kinds: expected_drift_kinds(),
        },
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

fn expected_runtime() -> RuntimeDescriptor {
    RuntimeDescriptor {
        event_store_schema_version: EVENT_STORE_SCHEMA_VERSION,
        event_contract_registry_version: EVENT_CONTRACT_REGISTRY_VERSION,
        transaction_mode: TRANSACTION_MODE.to_owned(),
        projection_cursor_count_limit: PROJECTION_CURSOR_COUNT_LIMIT,
        projection_cursor_rejection_probe_limit: PROJECTION_CURSOR_REJECTION_PROBE_LIMIT,
        caller_main_table_count_limit: CALLER_MAIN_TABLE_COUNT_LIMIT,
        caller_foreign_key_row_count_limit: CALLER_FOREIGN_KEY_ROW_COUNT_LIMIT,
        caller_inbound_foreign_key_policy: CALLER_INBOUND_FOREIGN_KEY_POLICY.to_owned(),
        caller_inbound_foreign_key_parent_tables: owned(CALLER_INBOUND_FOREIGN_KEY_PARENT_TABLES),
        cold_repair_mode: COLD_REPAIR_MODE.to_owned(),
        immutable_raw_digest: DigestDescriptor {
            algorithm: DIGEST_ALGORITHM.to_owned(),
            domain_utf8: RAW_DIGEST_DOMAIN_UTF8.to_owned(),
            domain_terminator: DIGEST_DOMAIN_TERMINATOR.to_owned(),
            framing: expected_digest_framing(),
            output_bytes: 32,
            source_queries: digest_query_descriptors(RAW_DIGEST_QUERY_SPECS),
        },
        active_product_state_digest: ProductDigestDescriptor {
            algorithm: DIGEST_ALGORITHM.to_owned(),
            domain_utf8: PRODUCT_DIGEST_DOMAIN_UTF8.to_owned(),
            domain_terminator: DIGEST_DOMAIN_TERMINATOR.to_owned(),
            framing: expected_digest_framing(),
            output_bytes: 32,
            components: owned(PRODUCT_DIGEST_COMPONENTS),
            exclusions: owned(PRODUCT_DIGEST_EXCLUSIONS),
            component_queries: digest_query_descriptors(PRODUCT_DIGEST_QUERY_SPECS),
        },
        visibility_oracle: VISIBILITY_ORACLE.to_owned(),
        scoped_integrity_mode: SCOPED_INTEGRITY_MODE.to_owned(),
        scoped_integrity_tables: owned(SCOPED_INTEGRITY_TABLES),
        sqlite_sequence_scope: SQLITE_SEQUENCE_SCOPE.to_owned(),
        stages: owned(REBUILD_STAGES),
        failpoints: REBUILD_FAILPOINTS
            .iter()
            .map(|failpoint| failpoint.id.to_owned())
            .collect(),
        preserved_authorities: owned(PRESERVED_AUTHORITIES),
    }
}

fn expected_drift_kinds() -> Vec<DriftKindDescriptor> {
    RAW_SOURCE_REBUILD_DRIFT_KINDS
        .iter()
        .map(|(variant, code)| DriftKindDescriptor {
            variant: (*variant).to_owned(),
            code: (*code).to_owned(),
        })
        .collect()
}

fn expected_digest_framing() -> DigestFramingDescriptor {
    DigestFramingDescriptor {
        section: "S_then_N_then_u64be_length_then_utf8_name".to_owned(),
        row: "R".to_owned(),
        signed_i64: "I_then_i64be".to_owned(),
        boolean: "B_then_u8_0_or_1".to_owned(),
        optional: "O_then_presence_u8_then_nested_value_when_present".to_owned(),
        text: "T_then_u64be_length_then_utf8_bytes".to_owned(),
        blob: "X_then_u64be_length_then_bytes".to_owned(),
    }
}

fn digest_query_descriptors(specs: &[DigestQuerySpec]) -> Vec<DigestQueryDescriptor> {
    specs
        .iter()
        .map(|spec| {
            let framing = expected_digest_field_framing(spec.section);
            assert_eq!(
                spec.fields.len(),
                framing.len(),
                "governed digest field/framing inventory length"
            );
            DigestQueryDescriptor {
                section: spec.section.to_owned(),
                sql: spec.sql.to_owned(),
                fields: spec
                    .fields
                    .iter()
                    .zip(framing)
                    .map(|(name, framing)| DigestFieldDescriptor {
                        name: (*name).to_owned(),
                        framing: (*framing).to_owned(),
                    })
                    .collect(),
            }
        })
        .collect()
}

fn expected_digest_field_framing(section: &str) -> &'static [&'static str] {
    match section {
        "event_envelopes" => &[
            "i64", "text", "text", "i64", "i64", "text", "text", "text", "text", "i64",
        ],
        "event_envelope_tags" => &["i64", "text", "i64", "text", "optional_text", "text"],
        "envelope_classification" => &[
            "text",
            "text",
            "text",
            "optional_text",
            "optional_text",
            "boolean",
        ],
        "tag_classification" => &["text", "i64", "optional_text", "optional_text", "boolean"],
        "raw_heads" => &["text", "i64", "text", "optional_text", "text", "i64"],
        "event_coordinates" => &[
            "text",
            "text",
            "i64",
            "text",
            "i64",
            "text",
            "optional_text",
            "optional_text",
            "text",
            "boolean",
            "optional_text",
        ],
        "nip09_requests" => &["text", "text", "i64"],
        "nip09_event_targets" => &["text", "text", "i64", "text"],
        "nip09_address_targets" => &[
            "text", "i64", "text", "text", "i64", "i64", "text", "text", "text", "text",
        ],
        "addressable_heads" => &[
            "i64",
            "text",
            "text",
            "text",
            "i64",
            "text",
            "optional_text",
            "optional_text",
            "text",
            "optional_text",
            "optional_text",
            "optional_text",
            "optional_text",
            "optional_i64",
        ],
        "current_visibility" => &[
            "text",
            "text",
            "optional_text",
            "text",
            "optional_text",
            "boolean",
            "optional_text",
            "optional_text",
            "optional_text",
            "optional_text",
            "optional_text",
            "optional_i64",
            "text",
        ],
        "food_projection" => &[
            "i64",
            "text",
            "text",
            "text",
            "i64",
            "text",
            "text",
            "text",
            "text",
            "i64",
            "text",
            "text",
            "text",
            "text",
            "optional_text",
            "optional_text",
            "text",
            "text",
        ],
        "food_images" => &[
            "text",
            "text",
            "i64",
            "text",
            "optional_text",
            "optional_i64",
            "optional_i64",
            "optional_text",
            "boolean",
            "text",
        ],
        "food_search" => &["text", "text", "text", "text", "text", "text", "text"],
        "food_cursor" => &["i64", "i64", "blob", "text", "i64"],
        other => panic!("unrecognized governed digest section `{other}`"),
    }
}

fn validate_manifest_shape(manifest: &RawSourceRebuildManifest) -> Result<(), String> {
    let expected_entries = ENTRY_POINTS
        .iter()
        .map(|(role, rust_path)| EntryPointDescriptor {
            role: (*role).to_owned(),
            rust_path: (*rust_path).to_owned(),
        })
        .collect::<Vec<_>>();
    let expected_public_api = PublicApiDescriptor {
        added_symbols: owned(ADDED_PUBLIC_SYMBOLS),
        methods: owned(PUBLIC_METHODS),
        error_variants: owned(ERROR_VARIANTS),
        drift_kinds: expected_drift_kinds(),
    };
    if manifest.schema_version != SCHEMA_VERSION
        || manifest.contract_id != CONTRACT_ID
        || manifest.authority_id != AUTHORITY_ID
        || manifest.manifest_schema.path != MANIFEST_SCHEMA_RELATIVE
        || manifest.predecessor.contract_id != PREDECESSOR_CONTRACT_ID
        || manifest.predecessor.manifest.path != PREDECESSOR_MANIFEST_RELATIVE
        || manifest.predecessor.manifest.byte_length
            != u64::try_from(PREDECESSOR_MANIFEST_BYTE_LENGTH)
                .map_err(|_| "predecessor byte length does not fit u64".to_owned())?
        || manifest.predecessor.manifest.sha256 != PREDECESSOR_MANIFEST_SHA256
        || manifest.runtime != expected_runtime()
        || manifest.entry_points != expected_entries
        || manifest.public_api != expected_public_api
    {
        return Err(format!(
            "{MANIFEST_RELATIVE} has inconsistent raw-source rebuild identity or semantics"
        ));
    }
    let expected_sources = SOURCE_SPECS
        .iter()
        .map(|spec| (spec.role, spec.path))
        .collect::<Vec<_>>();
    let actual_sources = manifest
        .source_files
        .iter()
        .map(|source| (source.role.as_str(), source.path.as_str()))
        .collect::<Vec<_>>();
    if actual_sources != expected_sources {
        return Err(format!(
            "{MANIFEST_RELATIVE} source-file inventory is not exact"
        ));
    }
    let expected_migrations = MIGRATION_RELATIVES.to_vec();
    let actual_migrations = manifest
        .migration_inventory
        .iter()
        .map(|descriptor| descriptor.path.as_str())
        .collect::<Vec<_>>();
    if actual_migrations != expected_migrations {
        return Err(format!(
            "{MANIFEST_RELATIVE} migration inventory must remain exactly versions 0001 through 0004"
        ));
    }
    validate_unique(
        "raw-source rebuild source roles",
        manifest
            .source_files
            .iter()
            .map(|source| source.role.as_str()),
    )?;
    validate_unique(
        "raw-source rebuild source paths",
        manifest
            .source_files
            .iter()
            .map(|source| source.path.as_str()),
    )?;
    for source in &manifest.source_files {
        if GENERATED_ARTIFACT_PATHS.contains(&source.path.as_str()) {
            return Err(format!(
                "{MANIFEST_RELATIVE} recursively hashes generated artifact `{}`",
                source.path
            ));
        }
        validate_file_descriptor(
            source.path.as_str(),
            source.byte_length,
            &source.sha256,
            &source.hash_algorithm,
        )?;
    }
    for descriptor in manifest
        .migration_inventory
        .iter()
        .chain([&manifest.manifest_schema, &manifest.predecessor.manifest])
    {
        validate_file_descriptor(
            descriptor.path.as_str(),
            descriptor.byte_length,
            &descriptor.sha256,
            &descriptor.hash_algorithm,
        )?;
    }
    if manifest.result_vector.canonical_path != RESULT_VECTOR_CANONICAL_RELATIVE
        || manifest.result_vector.mirror_path != RESULT_VECTOR_MIRROR_RELATIVE
        || manifest.result_vector.hash_algorithm != HASH_ALGORITHM
        || manifest.result_vector.executor_id != RESULT_VECTOR_EXECUTOR_ID
        || manifest.result_vector.executor_path != RESULT_VECTOR_EXECUTOR_RELATIVE
        || manifest.result_vector.executor_test != RESULT_VECTOR_EXECUTOR_TEST
        || manifest.result_vector.executor_hash_algorithm != HASH_ALGORITHM
        || manifest.result_vector.byte_length == 0
        || manifest.result_vector.executor_byte_length == 0
    {
        return Err(format!(
            "{MANIFEST_RELATIVE} result-vector descriptor is invalid"
        ));
    }
    validate_sha256("result vector", &manifest.result_vector.sha256)?;
    validate_sha256(
        "result-vector executor",
        &manifest.result_vector.executor_sha256,
    )
}

fn validate_source_contract(workspace_root: &Path) -> Result<(), String> {
    validate_source_inventory()?;
    validate_complete_event_store_source_closure(workspace_root)?;
    validate_migration_inventory(workspace_root)?;
    validate_current_event_store_successor_authority(workspace_root)?;
    validate_raw_source_rebuild_successor_compiler_inputs(
        workspace_root,
        EVENT_STORE_SUCCESSOR_COMPILER_TABLES_SHA256,
    )?;
    validate_delegated_compiler_source_pins(workspace_root)?;
    validate_xtask_manifest_authority(workspace_root)?;
    validate_predecessor_source_supersession(
        workspace_root,
        &read_regular_file(workspace_root, PREDECESSOR_MANIFEST_RELATIVE)?,
    )?;
    validate_successor_compiler_input_authority(workspace_root)?;
    validate_public_api_authority(workspace_root)?;
    validate_error_authority(workspace_root)?;
    validate_runtime_authority(workspace_root)?;
    validate_release_authority(workspace_root)?;
    validate_command_reachability(workspace_root)
}

fn validate_source_inventory() -> Result<(), String> {
    validate_source_inventory_specs(SOURCE_SPECS)
}

fn validate_source_inventory_specs(source_specs: &[SourceSpec]) -> Result<(), String> {
    validate_unique(
        "raw-source rebuild source roles",
        source_specs.iter().map(|spec| spec.role),
    )?;
    validate_unique(
        "raw-source rebuild source paths",
        source_specs.iter().map(|spec| spec.path),
    )?;
    for (role, path) in REQUIRED_DELEGATED_COMPILER_SOURCES {
        let matches = source_specs
            .iter()
            .filter(|spec| spec.role == *role && spec.path == *path)
            .count();
        if matches != 1 {
            return Err(format!(
                "raw-source rebuild source inventory must bind delegated compiler input `{role}` at `{path}` exactly once; found {matches}"
            ));
        }
    }
    validate_unique("raw-source rebuild stages", REBUILD_STAGES.iter().copied())?;
    validate_unique(
        "raw-source rebuild failpoint IDs",
        REBUILD_FAILPOINTS.iter().map(|failpoint| failpoint.id),
    )?;
    validate_unique(
        "raw-source rebuild failpoint variants",
        REBUILD_FAILPOINTS.iter().map(|failpoint| failpoint.variant),
    )?;
    validate_unique(
        "raw-source rebuild rollback vector case IDs",
        REBUILD_FAILPOINTS
            .iter()
            .map(|failpoint| failpoint.rollback_case_id),
    )?;
    let sources = source_specs
        .iter()
        .map(|spec| spec.path)
        .collect::<BTreeSet<_>>();
    for generated in GENERATED_ARTIFACT_PATHS {
        if sources.contains(generated) {
            return Err(format!(
                "raw-source rebuild generated artifact `{generated}` must not participate in its own source hash graph"
            ));
        }
    }
    Ok(())
}

fn validate_complete_event_store_source_closure(workspace_root: &Path) -> Result<(), String> {
    let actual = governed_regular_file_inventory(workspace_root, "crates/event_store/src")?
        .into_iter()
        .filter(|relative| relative.ends_with(".rs"))
        .collect::<Vec<_>>();
    let mut expected = SOURCE_SPECS
        .iter()
        .map(|spec| spec.path)
        .filter(|relative| {
            relative.starts_with("crates/event_store/src/") && relative.ends_with(".rs")
        })
        .map(str::to_owned)
        .collect::<Vec<_>>();
    expected.push(GENERATED_DESCRIPTOR_RELATIVE.to_owned());
    expected.sort();

    if actual != expected {
        let actual_set = actual.iter().map(String::as_str).collect::<BTreeSet<_>>();
        let expected_set = expected.iter().map(String::as_str).collect::<BTreeSet<_>>();
        let missing = expected_set
            .difference(&actual_set)
            .copied()
            .collect::<Vec<_>>();
        let unexpected = actual_set
            .difference(&expected_set)
            .copied()
            .collect::<Vec<_>>();
        return Err(format!(
            "event-store Rust source closure drifted: missing {missing:?}, unexpected {unexpected:?}"
        ));
    }
    Ok(())
}

fn validate_delegated_compiler_source_pins(workspace_root: &Path) -> Result<(), String> {
    validate_delegated_compiler_source_pins_with_supersessions(workspace_root, &[])
}

fn validate_delegated_compiler_source_pins_with_supersessions(
    workspace_root: &Path,
    superseded_paths: &[&str],
) -> Result<(), String> {
    let superseded = superseded_paths.iter().copied().collect::<BTreeSet<_>>();
    if superseded.len() != superseded_paths.len() {
        return Err("delegated compiler source supersession paths must be unique".to_owned());
    }
    let pinned = DELEGATED_COMPILER_SOURCE_PINS
        .iter()
        .map(|(relative, _)| *relative)
        .collect::<BTreeSet<_>>();
    if let Some(relative) = superseded
        .iter()
        .find(|relative| !pinned.contains(**relative))
    {
        return Err(format!(
            "delegated compiler source supersession path `{relative}` is not predecessor-pinned"
        ));
    }
    for (relative, expected_sha256) in DELEGATED_COMPILER_SOURCE_PINS {
        if superseded.contains(relative) {
            continue;
        }
        let actual_sha256 = sha256_hex(&read_regular_file(workspace_root, relative)?);
        if actual_sha256 != *expected_sha256 {
            return Err(format!(
                "delegated compiler source `{relative}` drifted: expected {expected_sha256}, found {actual_sha256}"
            ));
        }
    }
    Ok(())
}

fn validate_xtask_manifest_authority(workspace_root: &Path) -> Result<(), String> {
    let source = regular_utf8_source(workspace_root, XTASK_MANIFEST_RELATIVE)?;
    let manifest: toml::Value = toml::from_str(&source)
        .map_err(|error| format!("parse {XTASK_MANIFEST_RELATIVE}: {error}"))?;
    let package = manifest
        .get("package")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| format!("{XTASK_MANIFEST_RELATIVE} must define one package table"))?;
    for flag in XTASK_REQUIRED_DISABLED_AUTO_TARGET_FLAGS {
        if package.get(*flag).and_then(toml::Value::as_bool) != Some(false) {
            return Err(format!(
                "{XTASK_MANIFEST_RELATIVE} package.{flag} must be exactly false so delegated compiler targets cannot be auto-discovered"
            ));
        }
    }
    let explicit_bins = manifest
        .get("bin")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| {
            format!("{XTASK_MANIFEST_RELATIVE} must declare exactly one explicit [[bin]] target")
        })?;
    let exact_main_bin = explicit_bins.len() == 1
        && explicit_bins[0].as_table().is_some_and(|target| {
            target.len() == 2
                && target.get("name").and_then(toml::Value::as_str) == Some("xtask")
                && target.get("path").and_then(toml::Value::as_str) == Some("src/main.rs")
        });
    if package.get("name").and_then(toml::Value::as_str) != Some("xtask")
        || package.get("publish").and_then(toml::Value::as_bool) != Some(false)
        || !exact_main_bin
        || ["lib", "example", "test", "bench"]
            .iter()
            .any(|target| manifest.get(*target).is_some())
    {
        return Err(format!(
            "{XTASK_MANIFEST_RELATIVE} must remain the unpublished xtask package with build and automatic target discovery disabled, exactly one explicit `xtask` binary at `src/main.rs`, and no additional Cargo targets"
        ));
    }
    for relative in XTASK_FORBIDDEN_AUTO_TARGET_PATHS {
        match fs::symlink_metadata(workspace_root.join(relative)) {
            Ok(_) => {
                return Err(format!(
                    "delegated xtask compiler authority forbids auto-target path `{relative}`"
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "inspect delegated compiler auto-target path `{relative}`: {error}"
                ));
            }
        }
    }
    Ok(())
}

fn validate_successor_compiler_input_authority(workspace_root: &Path) -> Result<(), String> {
    let mut sources = governed_regular_file_inventory(workspace_root, "crates/event_store/src")?
        .into_iter()
        .filter(|relative| relative.ends_with(".rs"))
        .collect::<Vec<_>>();
    sources.push(RESULT_VECTOR_EXECUTOR_RELATIVE.to_owned());
    for relative in sources {
        let file = rust_file(workspace_root, &relative)?;
        let expected_inputs = expected_successor_compiler_inputs(&relative);
        validate_exact_successor_compiler_inputs(&relative, &file, expected_inputs)?;
    }
    Ok(())
}

fn expected_successor_compiler_inputs(relative: &str) -> &'static [&'static str] {
    match relative {
        "crates/event_store/src/migrations.rs" => &[
            "include_str!(\"../migrations/0001_event_store.up.sql\")",
            "include_str!(\"../migrations/0001_event_store.down.sql\")",
            "include_str!(\"../migrations/0002_nip09.up.sql\")",
            "include_str!(\"../migrations/0002_nip09.down.sql\")",
            "include_str!(\"../migrations/0003_food_availability_projection.up.sql\")",
            "include_str!(\"../migrations/0003_food_availability_projection.down.sql\")",
            "include_str!(\"../migrations/0004_source_maintenance.up.sql\")",
            "include_str!(\"../migrations/0004_source_maintenance.down.sql\")",
            "env!(\"CARGO_MANIFEST_DIR\")",
        ],
        "crates/event_store/src/nip09/reconciliation_v1.rs" => &[
            "include_str!(\"../../migrations/0001_event_store.up.sql\")",
            "include_str!(\"../../migrations/0002_nip09.up.sql\")",
        ],
        "crates/event_store/src/nip09/reconciliation_v1/result_vector_executor.rs" => &[
            "include_bytes!(\"../../../tests/fixtures/nip09_reconciliation.v1.json\")",
            "include_str!(\"../../../migrations/0001_event_store.up.sql\")",
            "include_str!(\"../../../migrations/0002_nip09.up.sql\")",
        ],
        "crates/event_store/src/nip09/reconciliation_v1/visibility_oracle_v1.rs" => {
            &["include_bytes!(\"../../../tests/fixtures/food_availability_projection.v1.json\")"]
        }
        "crates/event_store/src/store/raw_source_rebuild_v1_tests.rs" => &[
            "include_bytes!(\"../../tests/fixtures/food_availability_projection.v1.json\")",
            "include_bytes!(\"../../tests/fixtures/nip09_reconciliation.v1.json\")",
        ],
        RESULT_VECTOR_EXECUTOR_RELATIVE => &[
            "include_bytes!(\"../../../contracts/conformance/vectors/event_store/raw_source_rebuild.v1.json\")",
            "include_bytes!(\"fixtures/food_availability_projection.v1.json\")",
        ],
        _ => &[],
    }
}

fn validate_exact_successor_compiler_inputs(
    relative: &str,
    file: &syn::File,
    expected_inputs: &[&str],
) -> Result<(), String> {
    struct Audit {
        inputs: Vec<String>,
        path_attributes: Vec<String>,
    }

    impl<'ast> syn::visit::Visit<'ast> for Audit {
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

        fn visit_attribute(&mut self, attribute: &'ast syn::Attribute) {
            if attribute.path().is_ident("path")
                || (attribute.path().is_ident("cfg_attr")
                    && token_stream_contains_ident(attribute.meta.to_token_stream(), "path"))
            {
                self.path_attributes.push(compact_tokens(attribute));
            }
            syn::visit::visit_attribute(self, attribute);
        }
    }

    use syn::visit::Visit;
    let mut audit = Audit {
        inputs: Vec::new(),
        path_attributes: Vec::new(),
    };
    audit.visit_file(file);
    let expected_inputs = expected_inputs
        .iter()
        .map(|input| (*input).to_owned())
        .collect::<Vec<_>>();
    if audit.inputs != expected_inputs || !audit.path_attributes.is_empty() {
        return Err(format!(
            "{relative} successor compiler-input authority drifted: expected {expected_inputs:?} and no path retargeting, found {:?} and {:?}",
            audit.inputs, audit.path_attributes
        ));
    }
    Ok(())
}

fn token_stream_contains_ident(tokens: proc_macro2::TokenStream, expected: &str) -> bool {
    tokens.into_iter().any(|token| match token {
        proc_macro2::TokenTree::Ident(ident) => ident == expected,
        proc_macro2::TokenTree::Group(group) => {
            token_stream_contains_ident(group.stream(), expected)
        }
        proc_macro2::TokenTree::Punct(_) | proc_macro2::TokenTree::Literal(_) => false,
    })
}

fn validate_predecessor_identity(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() != PREDECESSOR_MANIFEST_BYTE_LENGTH
        || sha256_hex(bytes) != PREDECESSOR_MANIFEST_SHA256
    {
        return Err(format!(
            "{PREDECESSOR_MANIFEST_RELATIVE} does not match the immutable SourceMaintenance predecessor identity"
        ));
    }
    Ok(())
}

fn validate_predecessor_source_supersession(
    workspace_root: &Path,
    predecessor_bytes: &[u8],
) -> Result<(), String> {
    validate_predecessor_identity(predecessor_bytes)?;
    let predecessor: Value = serde_json::from_slice(predecessor_bytes)
        .map_err(|error| format!("parse {PREDECESSOR_MANIFEST_RELATIVE}: {error}"))?;
    let descriptors = predecessor
        .get("source_files")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{PREDECESSOR_MANIFEST_RELATIVE} has no source_files array"))?;
    let successor_paths = SOURCE_SPECS
        .iter()
        .map(|spec| spec.path)
        .collect::<BTreeSet<_>>();
    let expected_drift = EXPECTED_SOURCE_MAINTENANCE_DRIFT_PATHS
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if expected_drift.len() != EXPECTED_SOURCE_MAINTENANCE_DRIFT_PATHS.len() {
        return Err(
            "raw-source rebuild expected predecessor drift paths must be unique".to_owned(),
        );
    }
    if let Some(path) = expected_drift
        .iter()
        .find(|path| !successor_paths.contains(**path))
    {
        return Err(format!(
            "raw-source rebuild successor does not current-byte-bind expected changed SourceMaintenance source `{path}`"
        ));
    }
    let mut predecessor_paths = BTreeSet::new();
    let mut actual_drift = BTreeSet::new();
    for descriptor in descriptors {
        let path = descriptor
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| "predecessor source descriptor has no path".to_owned())?;
        let predecessor_sha256 = descriptor
            .get("sha256")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("predecessor source descriptor `{path}` has no sha256"))?;
        if !predecessor_paths.insert(path) {
            return Err(format!(
                "{PREDECESSOR_MANIFEST_RELATIVE} contains duplicate source descriptor `{path}`"
            ));
        }
        let current = read_regular_file(workspace_root, path)?;
        if sha256_hex(&current) != predecessor_sha256 {
            actual_drift.insert(path);
        }
    }
    if actual_drift != expected_drift {
        return Err(format!(
            "raw-source rebuild SourceMaintenance drift inventory differs: expected {expected_drift:?}, found {actual_drift:?}"
        ));
    }

    let transitive = TRANSITIVE_PREDECESSOR_SUPERSEDED_PATHS
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if transitive.len() != TRANSITIVE_PREDECESSOR_SUPERSEDED_PATHS.len() {
        return Err(
            "raw-source rebuild transitive predecessor supersession paths must be unique"
                .to_owned(),
        );
    }
    if let Some(path) = transitive.iter().find(|path| {
        !successor_paths.contains(**path)
            && !predecessor_paths.contains(**path)
            && !BLOSSOM_READINESS_SUCCESSOR_TRANSITIVE_PATHS.contains(path)
    }) {
        return Err(format!(
            "raw-source rebuild transitive supersession path `{path}` is not bound by the current successor or immutable SourceMaintenance predecessor"
        ));
    }
    validate_food_availability_projection_predecessor_production_sources_under_lock(
        workspace_root,
        TRANSITIVE_PREDECESSOR_SUPERSEDED_PATHS,
    )?;
    Ok(())
}

fn validate_migration_inventory(workspace_root: &Path) -> Result<(), String> {
    let migration_root = workspace_root.join("crates/event_store/migrations");
    let mut actual = fs::read_dir(&migration_root)
        .map_err(|error| format!("read {}: {error}", migration_root.display()))?
        .map(|entry| {
            let entry = entry.map_err(|error| format!("read migration entry: {error}"))?;
            if !entry
                .file_type()
                .map_err(|error| format!("inspect {}: {error}", entry.path().display()))?
                .is_file()
            {
                return Err(format!(
                    "migration inventory entry {} must be a regular file",
                    entry.path().display()
                ));
            }
            entry
                .path()
                .strip_prefix(workspace_root)
                .map(|path| path.to_string_lossy().into_owned())
                .map_err(|error| format!("relativize migration path: {error}"))
        })
        .collect::<Result<Vec<_>, String>>()?;
    actual.sort();
    if actual != MIGRATION_RELATIVES {
        return Err(format!(
            "raw-source rebuild is runtime-only and requires the exact 0001-through-0004 migration inventory; found {actual:?}"
        ));
    }
    Ok(())
}

fn validate_public_api_authority(workspace_root: &Path) -> Result<(), String> {
    let model = rust_file(
        workspace_root,
        "crates/event_store/src/model/raw_source_rebuild_v1.rs",
    )?;
    let error = rust_file(workspace_root, "crates/event_store/src/error.rs")?;
    let lib = rust_file(workspace_root, "crates/event_store/src/lib.rs")?;
    let store = rust_file(workspace_root, "crates/event_store/src/store.rs")?;

    validate_digest_newtype(&model, "RadrootsEventStoreImmutableRawDigestV1")?;
    validate_digest_newtype(&model, "RadrootsEventStoreActiveProductStateDigestV1")?;
    validate_report_model(&model)?;
    validate_caller_inbound_foreign_key_model(&error)?;

    let routes = collect_top_level_public_use_routes(&lib);
    for symbol in ADDED_PUBLIC_SYMBOLS {
        let expected_module = match *symbol {
            "RADROOTS_EVENT_STORE_PROJECTION_CURSOR_COUNT_LIMIT_V1"
            | "RadrootsEventStoreCallerInboundForeignKeyV1"
            | "RadrootsEventStoreRawSourceRebuildDriftV1" => "error",
            _ => "model",
        };
        let matches = routes
            .iter()
            .filter(|route| route.exported_name == *symbol)
            .collect::<Vec<_>>();
        let [route] = matches.as_slice() else {
            return Err(format!(
                "crate root must export raw-source rebuild symbol `{symbol}` exactly once; found {}",
                matches.len()
            ));
        };
        if route.attributes != ["#[cfg(feature=\"sqlite\")]"].map(str::to_owned)
            || route.absolute
            || route.renamed
            || route.glob
            || route.segments.as_slice() != [expected_module, *symbol]
        {
            return Err(format!(
                "crate-root raw-source rebuild export `{symbol}` must be direct, non-renamed, and sqlite-gated from {expected_module}"
            ));
        }
    }

    validate_public_method_signatures(&store)?;
    validate_store_public_method_surface(&store)
}

fn validate_public_method_signatures(store: &syn::File) -> Result<(), String> {
    for (method, expected_signature) in [
        (
            "rebuild_from_raw_v1",
            "pub async fn rebuild_from_raw_v1(&self,) -> Result<RadrootsEventStoreRawSourceRebuildReportV1, RadrootsEventStoreError>",
        ),
        (
            "repair_file_from_raw_v1",
            "pub async fn repair_file_from_raw_v1(path: impl AsRef<Path>,) -> Result<(Self, RadrootsEventStoreRawSourceRebuildReportV1), RadrootsEventStoreError>",
        ),
    ] {
        let function = exact_associated_method(store, "RadrootsEventStore", method)?;
        let actual = compact_tokens(&function.sig);
        let expected = compact_signature(expected_signature)?;
        if actual != expected {
            return Err(format!(
                "RadrootsEventStore::{method} signature drifted: expected `{expected}`, found `{actual}`"
            ));
        }
    }
    Ok(())
}

fn validate_store_public_method_surface(file: &syn::File) -> Result<(), String> {
    const EXPECTED_SHA256: &str =
        "f96da738c7c24b9ebdc99f405035f7f9e0758d4e1d83f2a8de0b2877309eeb87";
    let signatures = file
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Impl(item)
                if item.trait_.is_none()
                    && compact_tokens(item.self_ty.as_ref()) == "RadrootsEventStore" =>
            {
                Some(item)
            }
            _ => None,
        })
        .flat_map(|item| &item.items)
        .filter_map(|item| match item {
            syn::ImplItem::Fn(function) if matches!(function.vis, syn::Visibility::Public(_)) => {
                Some(compact_tokens(&function.sig))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    validate_unique(
        "RadrootsEventStore public method signatures",
        signatures.iter().map(String::as_str),
    )?;
    let actual_sha256 = sha256_hex(signatures.join("\n").as_bytes());
    if actual_sha256 != EXPECTED_SHA256 {
        return Err(format!(
            "RadrootsEventStore complete public method surface drifted: expected {EXPECTED_SHA256}, found {actual_sha256}"
        ));
    }
    Ok(())
}

fn validate_digest_newtype(file: &syn::File, name: &str) -> Result<(), String> {
    let item = exact_struct(file, name)?;
    let mut item = item.clone();
    strip_doc_attributes(&mut item.attrs);
    for field in &mut item.fields {
        strip_doc_attributes(&mut field.attrs);
    }
    let expected = syn::parse_str::<syn::ItemStruct>(&format!(
        "#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)] pub struct {name}(pub(crate) [u8; 32]);"
    ))
    .map_err(|error| format!("parse authoritative digest model `{name}`: {error}"))?;
    if compact_tokens(&item) != compact_tokens(&expected) {
        return Err(format!(
            "{name} must remain an opaque, fixed-width, Copy SHA-256 newtype"
        ));
    }
    let inherent = exact_impl(file, name)?;
    let methods = inherent
        .items
        .iter()
        .filter_map(|item| match item {
            syn::ImplItem::Fn(function) => Some((function.sig.ident.to_string(), function)),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    if methods.keys().cloned().collect::<Vec<_>>() != ["as_bytes", "from_bytes"] {
        return Err(format!(
            "{name} must expose only public as_bytes plus crate-private from_bytes"
        ));
    }
    let from_bytes = methods["from_bytes"];
    let as_bytes = methods["as_bytes"];
    if compact_tokens(&from_bytes.sig)
        != compact_signature("pub(crate) const fn from_bytes(bytes: [u8; 32]) -> Self")?
        || compact_tokens(&as_bytes.sig)
            != compact_signature("pub const fn as_bytes(&self) -> &[u8; 32]")?
    {
        return Err(format!("{name} constructor or accessor signature drifted"));
    }
    Ok(())
}

fn validate_report_model(file: &syn::File) -> Result<(), String> {
    let item = exact_struct(file, "RadrootsEventStoreRawSourceRebuildReportV1")?;
    let mut item = item.clone();
    strip_doc_attributes(&mut item.attrs);
    for field in &mut item.fields {
        strip_doc_attributes(&mut field.attrs);
    }
    let expected = syn::parse_str::<syn::ItemStruct>(
        r#"#[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub struct RadrootsEventStoreRawSourceRebuildReportV1 {
            pub(crate) prior_source_generation: RadrootsEventStoreSourceGeneration,
            pub(crate) new_source_generation: RadrootsEventStoreSourceGeneration,
            pub(crate) source_capacity: RadrootsEventStoreSourceCapacityV1,
            pub(crate) immutable_raw_digest: RadrootsEventStoreImmutableRawDigestV1,
            pub(crate) active_product_state_digest: RadrootsEventStoreActiveProductStateDigestV1,
        }"#,
    )
    .map_err(|error| format!("parse authoritative rebuild report: {error}"))?;
    if compact_tokens(&item) != compact_tokens(&expected) {
        return Err(
            "RadrootsEventStoreRawSourceRebuildReportV1 field or visibility authority drifted"
                .to_owned(),
        );
    }
    let inherent = exact_impl(file, "RadrootsEventStoreRawSourceRebuildReportV1")?;
    let actual = inherent
        .items
        .iter()
        .filter_map(|item| match item {
            syn::ImplItem::Fn(function) => Some(function.sig.ident.to_string()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let expected = [
        "prior_source_generation",
        "new_source_generation",
        "source_capacity",
        "raw_high_water_seq",
        "immutable_raw_digest",
        "active_product_state_digest",
    ];
    if actual != expected {
        return Err(format!(
            "raw-source rebuild report accessor inventory differs: expected {expected:?}, found {actual:?}"
        ));
    }
    Ok(())
}

fn validate_caller_inbound_foreign_key_model(file: &syn::File) -> Result<(), String> {
    let item = exact_struct(file, "RadrootsEventStoreCallerInboundForeignKeyV1")?;
    let mut item = item.clone();
    strip_doc_attributes(&mut item.attrs);
    for field in &mut item.fields {
        strip_doc_attributes(&mut field.attrs);
    }
    let expected = syn::parse_str::<syn::ItemStruct>(
        r#"
        #[non_exhaustive]
        #[derive(Debug, PartialEq, Eq)]
        pub struct RadrootsEventStoreCallerInboundForeignKeyV1 {
            pub child_table: String,
            pub foreign_key_id: i64,
            pub foreign_key_sequence: i64,
            pub child_column: String,
            pub parent_table: String,
            pub parent_column: Option<String>,
            pub on_update: String,
            pub on_delete: String,
            pub match_clause: String,
        }
        "#,
    )
    .map_err(|error| format!("parse caller inbound foreign-key model: {error}"))?;
    if compact_tokens(&item) != compact_tokens(&expected) {
        return Err(
            "RadrootsEventStoreCallerInboundForeignKeyV1 field, visibility, or derive authority drifted"
                .to_owned(),
        );
    }

    let display = file
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Impl(item)
                if compact_tokens(item.self_ty.as_ref())
                    == "RadrootsEventStoreCallerInboundForeignKeyV1"
                    && item.trait_.as_ref().is_some_and(|(_, path, _)| {
                        compact_tokens(path) == "core::fmt::Display"
                    }) =>
            {
                Some(item)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let [display] = display.as_slice() else {
        return Err(format!(
            "RadrootsEventStoreCallerInboundForeignKeyV1 must define one Display authority; found {}",
            display.len()
        ));
    };
    let expected_display = syn::parse_str::<syn::ItemImpl>(
        r#"
        impl core::fmt::Display for RadrootsEventStoreCallerInboundForeignKeyV1 {
            fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                write!(
                    formatter,
                    "{}:{} on `{}` (`{}` -> `{}`.",
                    self.foreign_key_id,
                    self.foreign_key_sequence,
                    self.child_table,
                    self.child_column,
                    self.parent_table,
                )?;
                match self.parent_column.as_deref() {
                    Some(parent_column) => write!(formatter, "`{parent_column}`")?,
                    None => formatter.write_str("<implicit primary key>")?,
                }
                write!(
                    formatter,
                    ", on update {}, on delete {}, match {})",
                    self.on_update,
                    self.on_delete,
                    self.match_clause,
                )
            }
        }
        "#,
    )
    .map_err(|error| format!("parse caller inbound foreign-key Display authority: {error}"))?;
    if compact_tokens(*display) != compact_tokens(&expected_display) {
        return Err(
            "RadrootsEventStoreCallerInboundForeignKeyV1 Display authority drifted".to_owned(),
        );
    }
    Ok(())
}

fn validate_error_authority(workspace_root: &Path) -> Result<(), String> {
    let file = rust_file(workspace_root, "crates/event_store/src/error.rs")?;
    validate_raw_source_rebuild_drift_taxonomy(&file)?;
    let limit = file
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Const(item)
                if item.ident == "RADROOTS_EVENT_STORE_PROJECTION_CURSOR_COUNT_LIMIT_V1" =>
            {
                Some(item)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let [limit] = limit.as_slice() else {
        return Err(format!(
            "projection-cursor capacity authority must define its public limit exactly once; found {}",
            limit.len()
        ));
    };
    let mut limit = (*limit).clone();
    strip_doc_attributes(&mut limit.attrs);
    let expected_limit = syn::parse_str::<syn::ItemConst>(
        "pub const RADROOTS_EVENT_STORE_PROJECTION_CURSOR_COUNT_LIMIT_V1: u32 = 4_096;",
    )
    .map_err(|error| format!("parse projection-cursor limit authority: {error}"))?;
    if compact_tokens(&limit) != compact_tokens(&expected_limit) {
        return Err(
            "RADROOTS_EVENT_STORE_PROJECTION_CURSOR_COUNT_LIMIT_V1 must remain exactly 4,096"
                .to_owned(),
        );
    }
    let errors = exact_enum(&file, "RadrootsEventStoreError")?;
    const EXPECTED_ERROR_ENUM_SHA256: &str =
        "dcb9416ca05bda35845f8708fe73132df7137b0c0e002e8ba6d709989bc31939";
    let actual_error_enum_sha256 = sha256_hex(compact_tokens(errors).as_bytes());
    if actual_error_enum_sha256 != EXPECTED_ERROR_ENUM_SHA256 {
        return Err(format!(
            "RadrootsEventStoreError complete variant surface drifted: expected {EXPECTED_ERROR_ENUM_SHA256}, found {actual_error_enum_sha256}"
        ));
    }
    let variants = errors
        .variants
        .iter()
        .map(|variant| (variant.ident.to_string(), variant))
        .collect::<BTreeMap<_, _>>();
    for name in ERROR_VARIANTS {
        if !variants.contains_key(*name) {
            return Err(format!(
                "RadrootsEventStoreError must define raw-source rebuild variant `{name}`"
            ));
        }
    }
    let drift = variants["RawSourceRebuildStateDrift"];
    let rollback = variants["RawSourceRebuildTransactionRollbackFailed"];
    let cursor_capacity = variants["ProjectionCursorCapacityExceeded"];
    let repair_identity = variants["RawSourceRepairDatabaseIdentityMismatch"];
    let repair_lock_domain = variants["RawSourceRepairCanonicalPathLockDomainMismatch"];
    let repair_canonicalization = variants["RawSourceRepairMainDatabaseCanonicalizationFailed"];
    let caller_table_capacity = variants["RawSourceRebuildCallerTableCapacityExceeded"];
    let caller_foreign_key_capacity = variants["RawSourceRebuildCallerForeignKeyCapacityExceeded"];
    let caller_inbound_foreign_key = variants["RawSourceRebuildCallerInboundForeignKeyUnsupported"];
    let drift_fields = compact_tokens(&drift.fields);
    let rollback_fields = compact_tokens(&rollback.fields);
    if drift_fields != "{kind:RadrootsEventStoreRawSourceRebuildDriftV1,detail:String,}" {
        return Err(
            "RawSourceRebuildStateDrift must carry one stable drift kind and diagnostic detail"
                .to_owned(),
        );
    }
    if rollback_fields != "{#[source]primary:Box<RadrootsEventStoreError>,rollback:sqlx::Error,}" {
        return Err(format!(
            "RawSourceRebuildTransactionRollbackFailed must preserve typed primary and SQL rollback errors; found `{rollback_fields}`"
        ));
    }
    if compact_tokens(&cursor_capacity.fields) != "{current:u32,limit:u32}" {
        return Err(
            "ProjectionCursorCapacityExceeded must carry exact current and limit u32 fields"
                .to_owned(),
        );
    }
    if compact_tokens(&repair_identity.fields) != "{expected:String,actual:String}"
        || compact_tokens(&repair_lock_domain.fields) != "{canonical_path:String}"
        || compact_tokens(&repair_canonicalization.fields)
            != "{filename:String,#[source]source:std::io::Error,}"
    {
        return Err(
            "raw-source file repair errors must retain their exact typed fields".to_owned(),
        );
    }
    if compact_tokens(&caller_table_capacity.fields) != "{observed_at_least:u64,limit:u64}"
        || compact_tokens(&caller_foreign_key_capacity.fields)
            != "{observed_at_least:u64,limit:u64}"
        || compact_tokens(&caller_inbound_foreign_key.fields)
            != "{dependency:Box<RadrootsEventStoreCallerInboundForeignKeyV1>,}"
    {
        return Err(
            "raw-source rebuild caller-schema errors must retain their exact typed fields"
                .to_owned(),
        );
    }
    let drift_attrs = drift.attrs.iter().map(compact_tokens).collect::<Vec<_>>();
    let rollback_attrs = rollback
        .attrs
        .iter()
        .map(compact_tokens)
        .collect::<Vec<_>>();
    if !drift_attrs.iter().any(|attribute| {
        attribute.contains("event-storeraw-sourcerebuildauthorityisinconsistent({kind}):{detail}")
    }) || !rollback_attrs.iter().any(|attribute| {
        attribute
            .contains("raw-sourcerebuildfailed:{primary};transactionrollbackalsofailed:{rollback}")
    }) || !cursor_capacity
        .attrs
        .iter()
        .map(compact_tokens)
        .any(|attribute| {
            attribute.contains(
                "event-storegenericprojectioncursorcapacityexceeded:current{current},limit{limit}",
            )
        })
        || !repair_identity.attrs.iter().map(compact_tokens).any(|attribute| {
            attribute.contains(
                "raw-sourcerepairSQLitemaindatabaseidentitymismatch:expected`{expected}`,found`{actual}`",
            )
        })
        || !repair_lock_domain
            .attrs
            .iter()
            .map(compact_tokens)
            .any(|attribute| {
                attribute.contains(
                    "raw-sourcerepaircanonicalpath`{canonical_path}`doesnotsharethevalidatedSQLitemainlockdomain",
                )
            })
        || !repair_canonicalization
            .attrs
            .iter()
            .map(compact_tokens)
            .any(|attribute| {
                attribute.contains(
                    "raw-sourcerepaircouldnotcanonicalizeSQLitemaindatabase`{filename}`:{source}",
                )
            })
        || !caller_table_capacity
            .attrs
            .iter()
            .map(compact_tokens)
            .any(|attribute| {
                attribute.contains(
                    "event-storeraw-sourcerebuildcallermain-tableinventoryexceedsboundedpreflightcapacity:observedatleast{observed_at_least},limit{limit}",
                )
            })
        || !caller_foreign_key_capacity
            .attrs
            .iter()
            .map(compact_tokens)
            .any(|attribute| {
                attribute.contains(
                    "event-storeraw-sourcerebuildcallerforeign-keyinventoryexceedsboundedpreflightcapacity:observedatleast{observed_at_least}rows,limit{limit}",
                )
            })
        || !caller_inbound_foreign_key
            .attrs
            .iter()
            .map(compact_tokens)
            .any(|attribute| {
                attribute.contains(
                    "event-storeraw-sourcerebuilddoesnotsupportcaller-ownedforeignkey{dependency}",
                )
            })
    {
        return Err("raw-source rebuild typed error display contract drifted".to_owned());
    }
    Ok(())
}

fn validate_raw_source_rebuild_drift_taxonomy(file: &syn::File) -> Result<(), String> {
    let item = exact_enum(file, "RadrootsEventStoreRawSourceRebuildDriftV1")?;
    let mut item = item.clone();
    strip_doc_attributes(&mut item.attrs);
    for variant in &mut item.variants {
        strip_doc_attributes(&mut variant.attrs);
    }
    let expected = syn::parse_str::<syn::ItemEnum>(
        r#"
        #[non_exhaustive]
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum RadrootsEventStoreRawSourceRebuildDriftV1 {
            ManagedSchemaAuthority,
            ImmutableRawAuthority,
            SourceGenerationLineage,
            AddressableTransitionAuthority,
            DerivedProductStateAuthority,
            RebuildPostcondition,
        }
        "#,
    )
    .map_err(|error| format!("parse raw-source rebuild drift taxonomy: {error}"))?;
    if compact_tokens(&item) != compact_tokens(&expected) {
        return Err(
            "RadrootsEventStoreRawSourceRebuildDriftV1 variant or derive authority drifted"
                .to_owned(),
        );
    }

    let code = exact_associated_method(file, "RadrootsEventStoreRawSourceRebuildDriftV1", "code")?;
    let mut code = code.clone();
    strip_doc_attributes(&mut code.attrs);
    let expected_code = syn::parse_str::<syn::ImplItemFn>(
        r#"
        pub const fn code(self) -> &'static str {
            match self {
                Self::ManagedSchemaAuthority => "managed_schema_authority",
                Self::ImmutableRawAuthority => "immutable_raw_authority",
                Self::SourceGenerationLineage => "source_generation_lineage",
                Self::AddressableTransitionAuthority => "addressable_transition_authority",
                Self::DerivedProductStateAuthority => "derived_product_state_authority",
                Self::RebuildPostcondition => "rebuild_postcondition",
            }
        }
        "#,
    )
    .map_err(|error| format!("parse raw-source rebuild drift code authority: {error}"))?;
    if compact_tokens(&code) != compact_tokens(&expected_code) {
        return Err("RadrootsEventStoreRawSourceRebuildDriftV1::code mapping drifted".to_owned());
    }

    let display_impls = file
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Impl(item)
                if compact_tokens(item.self_ty.as_ref())
                    == "RadrootsEventStoreRawSourceRebuildDriftV1"
                    && item.trait_.as_ref().is_some_and(|(_, path, _)| {
                        compact_tokens(path) == "core::fmt::Display"
                    }) =>
            {
                Some(item)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let [display] = display_impls.as_slice() else {
        return Err(format!(
            "RadrootsEventStoreRawSourceRebuildDriftV1 must implement Display exactly once; found {}",
            display_impls.len()
        ));
    };
    let expected_display = syn::parse_str::<syn::ItemImpl>(
        r#"
        impl core::fmt::Display for RadrootsEventStoreRawSourceRebuildDriftV1 {
            fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                formatter.write_str(self.code())
            }
        }
        "#,
    )
    .map_err(|error| format!("parse raw-source rebuild drift Display authority: {error}"))?;
    if compact_tokens(*display) != compact_tokens(&expected_display) {
        return Err("RadrootsEventStoreRawSourceRebuildDriftV1 Display mapping drifted".to_owned());
    }
    Ok(())
}

fn validate_runtime_authority(workspace_root: &Path) -> Result<(), String> {
    let rebuild_relative = REBUILD_RUNTIME_SOURCE_RELATIVE;
    let rebuild = rust_source(workspace_root, rebuild_relative)?;
    let rebuild_file =
        syn::parse_file(&rebuild).map_err(|error| format!("parse {rebuild_relative}: {error}"))?;
    for (name, domain) in [
        ("IMMUTABLE_RAW_DIGEST_DOMAIN_V1", RAW_DIGEST_DOMAIN_UTF8),
        (
            "ACTIVE_PRODUCT_STATE_DIGEST_DOMAIN_V1",
            PRODUCT_DIGEST_DOMAIN_UTF8,
        ),
    ] {
        let mut expected = domain.as_bytes().to_vec();
        expected.push(0);
        if exact_byte_string_const(&rebuild_file, name)? != expected {
            return Err(format!(
                "{rebuild_relative} `{name}` must be exact UTF-8 domain bytes followed by one NUL terminator"
            ));
        }
    }
    validate_failpoint_authority(&rebuild_file, rebuild_relative)?;
    let failpoint_test_file = rust_file(workspace_root, REBUILD_FAILPOINT_TEST_SOURCE_RELATIVE)?;
    validate_failpoint_test_array_authority(
        &failpoint_test_file,
        REBUILD_FAILPOINT_TEST_SOURCE_RELATIVE,
    )?;
    let reconciliation_relative = "crates/event_store/src/nip09/reconciliation_v1.rs";
    let reconciliation_file = rust_file(workspace_root, reconciliation_relative)?;
    validate_rebuild_marker_token_authority(
        &reconciliation_file,
        reconciliation_relative,
        &rebuild_file,
        rebuild_relative,
    )?;
    validate_coordinator_authority(&rebuild_file, rebuild_relative)?;
    validate_caller_schema_dependency_authority(&rebuild_file, rebuild_relative)?;
    validate_transition_sequence_authority(&rebuild_file, rebuild_relative)?;
    validate_scoped_integrity_authority(&rebuild_file, rebuild_relative)?;
    validate_digest_query_authority(&rebuild_file, rebuild_relative)?;
    if rebuild.contains("json_array(")
        || rebuild.contains("Vec<String>")
        || rebuild.contains("update_product_rows")
        || rebuild.contains("update_active_product_rows")
    {
        return Err(
            "active product digest must hash typed binary fields, not SQLite JSON row serialization"
                .to_owned(),
        );
    }
    for helper in [
        "digest_section",
        "digest_row_start",
        "digest_i64",
        "digest_text",
        "digest_optional_text",
    ] {
        exact_free_function(&rebuild_file, helper).map_err(|error| {
            format!("typed digest framing helper `{helper}` is missing: {error}")
        })?;
    }

    let oracle_relative = "crates/event_store/src/nip09/reconciliation_v1/visibility_oracle_v1.rs";
    let oracle = rust_source(workspace_root, oracle_relative)?;
    let oracle_file =
        syn::parse_file(&oracle).map_err(|error| format!("parse {oracle_relative}: {error}"))?;
    for marker in [
        "audit_current_visibility_from_raw_v1",
        "ReconciledEvent",
        "StoredEventClass::Regular",
        "StoredEventClass::Replaceable",
        "StoredEventClass::Addressable",
        "kind_u32",
    ] {
        if !oracle.contains(marker) {
            return Err(format!(
                "{oracle_relative} is missing independent visibility-oracle witness `{marker}`"
            ));
        }
    }
    for forbidden in ["radroots_event_store_current_visibility_v1", "sqlx::query"] {
        if oracle.contains(forbidden) {
            return Err(format!(
                "independent raw-snapshot visibility oracle must not query derived visibility authority `{forbidden}`"
            ));
        }
    }
    validate_visibility_oracle_index_authority(&oracle_file, oracle_relative)?;

    let store_relative = "crates/event_store/src/store.rs";
    let store = rust_source(workspace_root, store_relative)?;
    let store_file =
        syn::parse_file(&store).map_err(|error| format!("parse {store_relative}: {error}"))?;
    validate_public_entry_point_authority(&store_file, store_relative)?;
    validate_streaming_dependency_authority(workspace_root)?;

    let reconciliation_relative = "crates/event_store/src/nip09/reconciliation_v1.rs";
    let reconciliation = rust_source(workspace_root, reconciliation_relative)?;
    let reconciliation_file = syn::parse_file(&reconciliation)
        .map_err(|error| format!("parse {reconciliation_relative}: {error}"))?;
    validate_reconciliation_direct_request_index_authority(
        &reconciliation_file,
        reconciliation_relative,
    )?;
    let validation = compact_tokens(exact_free_function(
        &reconciliation_file,
        "validate_projection_cursor_authority",
    )?);
    for marker in [
        "RADROOTS_EVENT_STORE_PROJECTION_CURSOR_COUNT_LIMIT_V1",
        "+1",
        "SELECT1FROMprojection_cursorLIMIT?",
        "SELECT1FROMradroots_event_store_projection_cursor_sourceLIMIT?",
        "validate_projection_cursor_cardinality_v1(cursor_probe.len())?",
        "validate_projection_cursor_cardinality_v1(identity_probe.len())?",
        "LIMIT1",
    ] {
        if !validation.contains(marker) {
            return Err(format!(
                "bounded generic projection-cursor validation is missing `{marker}`"
            ));
        }
    }
    if validation.contains("COUNT(") {
        return Err(
            "generic projection-cursor validation must use cap-plus-one probes, not unbounded counts"
                .to_owned(),
        );
    }
    let preflight = compact_tokens(exact_free_function(
        &reconciliation_file,
        "preflight_projection_cursor_insert_v1",
    )?);
    for marker in [
        "SELECT1FROMprojection_cursorLIMIT?",
        "RADROOTS_EVENT_STORE_PROJECTION_CURSOR_COUNT_LIMIT_V1",
        "ProjectionCursorCapacityExceeded",
    ] {
        if !preflight.contains(marker) {
            return Err(format!(
                "projection-cursor prospective insert preflight is missing `{marker}`"
            ));
        }
    }
    if rebuild.contains("validate_projection_cursor_authority")
        || rebuild.contains("preflight_projection_cursor_insert_v1")
        || oracle.contains("projection_cursor")
    {
        return Err(
            "public raw-source rebuild must not enumerate caller-owned generic projection cursors"
                .to_owned(),
        );
    }
    let schema_relative = "crates/event_store/src/schema.rs";
    let schema_source = rust_source(workspace_root, schema_relative)?;
    let schema_file = syn::parse_file(&schema_source)
        .map_err(|error| format!("parse {schema_relative}: {error}"))?;
    let schema_version = schema_file
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Const(item) if item.ident == "RAW_SOURCE_REBUILD_SCHEMA_VERSION_V1" => Some(item),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [schema_version] = schema_version.as_slice() else {
        return Err(format!(
            "{schema_relative} must define one dedicated raw-source rebuild schema version; found {}",
            schema_version.len()
        ));
    };
    if compact_tokens(schema_version) != "constRAW_SOURCE_REBUILD_SCHEMA_VERSION_V1:u32=4;" {
        return Err(
            "raw-source rebuild maintenance authority must remain pinned to literal schema v4"
                .to_owned(),
        );
    }
    let exact_v4 = compact_tokens(exact_free_function(
        &schema_file,
        "validate_exact_managed_v4_for_raw_source_rebuild_v1",
    )?);
    for marker in [
        "validate_embedded_migration_registry()?",
        "validate_repair_temp_schema_bounded_v1(connection,EVENT_STORE_MIGRATIONS).await?",
        "read_repair_catalog_bounded_v1(connection,EVENT_STORE_MIGRATIONS).await?",
        "validate_ledger_catalog(&catalog)?",
        "read_repair_history_bounded_v1(connection,RAW_SOURCE_REBUILD_SCHEMA_VERSION_V1)",
        "validate_history_against_registry(&history,EVENT_STORE_MIGRATIONS,RAW_SOURCE_REBUILD_SCHEMA_VERSION_V1,)?",
        "current!=RAW_SOURCE_REBUILD_SCHEMA_VERSION_V1",
        "catalog_fingerprint(&governed_catalog(&catalog,EVENT_STORE_MIGRATIONS))",
        "actual!=migration.schema_sha256",
    ] {
        if !exact_v4.contains(marker) {
            return Err(format!(
                "exact managed-v4 maintenance validator is missing `{marker}`"
            ));
        }
    }
    for forbidden in [
        "RADROOTS_EVENT_STORE_SCHEMA_VERSION_CURRENT",
        "validate_event_store_temp_schema_with_registry",
        "read_catalog(",
        "read_history(",
        "validate_active_hook_state",
        "validate_food_availability_projection_hook",
    ] {
        if exact_v4.contains(forbidden) {
            return Err(format!(
                "exact managed-v4 maintenance validator must not depend on `{forbidden}`"
            ));
        }
    }
    validate_bounded_repair_schema_authority(&schema_file, schema_relative)?;
    let store_tokens = compact_tokens(&store_file);
    if store_tokens
        .matches("preflight_projection_cursor_insert_v1")
        .count()
        < 3
    {
        return Err(
            "both supported generic projection-cursor insert paths must reach prospective capacity preflight"
                .to_owned(),
        );
    }
    Ok(())
}

fn validate_rebuild_marker_token_authority(
    reconciliation: &syn::File,
    reconciliation_relative: &str,
    rebuild: &syn::File,
    rebuild_relative: &str,
) -> Result<(), String> {
    let token = exact_struct(reconciliation, "SourceRebuildMarkerTokenV1")?;
    let mut token = token.clone();
    strip_doc_attributes(&mut token.attrs);
    for field in &mut token.fields {
        strip_doc_attributes(&mut field.attrs);
    }
    let expected_token = syn::parse_str::<syn::ItemStruct>(
        "struct SourceRebuildMarkerTokenV1 { generation: RadrootsEventStoreSourceGeneration, }",
    )
    .map_err(|error| format!("parse rebuild marker token authority: {error}"))?;
    if compact_tokens(&token) != compact_tokens(&expected_token) {
        return Err(format!(
            "{reconciliation_relative} rebuild marker token must remain private, single-field, and non-Clone/non-Copy"
        ));
    }

    let open = exact_free_function(reconciliation, "open_source_rebuild_marker")?;
    let close = exact_free_function(reconciliation, "close_source_rebuild_marker")?;
    if compact_tokens(&open.sig)
        != compact_signature(
            "async fn open_source_rebuild_marker(connection: &mut SqliteConnection, plan: &SourceRebuildPlan,) -> Result<SourceRebuildMarkerTokenV1, RadrootsEventStoreError>",
        )?
        || compact_tokens(&close.sig)
            != compact_signature(
                "async fn close_source_rebuild_marker(connection: &mut SqliteConnection, marker: SourceRebuildMarkerTokenV1,) -> Result<(), RadrootsEventStoreError>",
            )?
    {
        return Err(format!(
            "{reconciliation_relative} marker open/close signatures must create and consume the exact rebuild token"
        ));
    }
    let open_body = compact_tokens(&open.block);
    let close_body = compact_tokens(&close.block);
    if open_body
        .matches("SourceRebuildMarkerTokenV1{generation:plan.generation,}")
        .count()
        != 1
        || close_body.matches("marker.generation").count() != 1
    {
        return Err(format!(
            "{reconciliation_relative} marker token construction or generation-bound close authority drifted"
        ));
    }

    struct MarkerTokenConstructionCounter(usize);
    impl<'ast> syn::visit::Visit<'ast> for MarkerTokenConstructionCounter {
        fn visit_expr_struct(&mut self, expression: &'ast syn::ExprStruct) {
            if expression
                .path
                .segments
                .last()
                .is_some_and(|segment| segment.ident == "SourceRebuildMarkerTokenV1")
            {
                self.0 += 1;
            }
            syn::visit::visit_expr_struct(self, expression);
        }
    }
    use syn::visit::Visit;
    let mut constructions = MarkerTokenConstructionCounter(0);
    constructions.visit_file(reconciliation);
    constructions.visit_file(rebuild);
    if constructions.0 != 1 {
        return Err(format!(
            "governed rebuild sources must construct SourceRebuildMarkerTokenV1 exactly once inside marker open; found {}",
            constructions.0
        ));
    }

    for (relative, function) in [
        (
            reconciliation_relative,
            exact_free_function(reconciliation, "apply_reconciliation_hook")?,
        ),
        (
            rebuild_relative,
            exact_free_function(rebuild, "rebuild_from_raw_v1_in_transaction_inner")?,
        ),
    ] {
        let body = compact_tokens(&function.block);
        if body
            .matches("letmarker=open_source_rebuild_marker(connection,&plan).await?;")
            .count()
            != 1
            || body
                .matches("close_source_rebuild_marker(connection,marker).await?;")
                .count()
                != 1
            || body.contains("marker.clone()")
        {
            return Err(format!(
                "{relative}::{} must acquire and consume one non-cloned rebuild marker token",
                function.sig.ident
            ));
        }
    }
    Ok(())
}

fn validate_reconciliation_direct_request_index_authority(
    file: &syn::File,
    relative: &str,
) -> Result<(), String> {
    let request_index = exact_impl(file, "RequestIndex")?;
    let methods = request_index
        .items
        .iter()
        .filter_map(|item| match item {
            syn::ImplItem::Fn(function) => Some(function.sig.ident.to_string()),
            _ => None,
        })
        .collect::<Vec<_>>();
    if methods != ["new", "insert", "decision"] {
        return Err(format!(
            "{relative} RequestIndex method inventory must be exactly [new, insert, decision]; found {methods:?}"
        ));
    }

    let constructor = compact_tokens(exact_associated_method(file, "RequestIndex", "new")?);
    for marker in ["forrequestinrequests", "index.insert(request)"] {
        if !constructor.contains(marker) {
            return Err(format!(
                "{relative} RequestIndex::new is missing incremental construction authority `{marker}`"
            ));
        }
    }
    for forbidden in [".projection()", ".event_targets()", ".address_targets()"] {
        if constructor.contains(forbidden) {
            return Err(format!(
                "{relative} RequestIndex::new must delegate each request once, not scan `{forbidden}`"
            ));
        }
    }

    let insert_function = exact_associated_method(file, "RequestIndex", "insert")?;
    let insert = compact_tokens(insert_function);
    for (marker, count) in [
        (".projection()", 2),
        (".event_targets()", 1),
        (".address_targets()", 1),
    ] {
        if insert.matches(marker).count() != count {
            return Err(format!(
                "{relative} RequestIndex::insert must scan each admitted request target projection exactly once through `{marker}`"
            ));
        }
    }
    for marker in [
        "request_id<current.as_str()",
        "request_event.created_at_u64()>current.created_at",
        "request_event.created_at_u64()==current.created_at",
        "request_id<current.request_id.as_str()",
        "evidence.unauthorized=true",
    ] {
        if !insert.contains(marker) {
            return Err(format!(
                "{relative} RequestIndex::insert is missing canonical evidence reduction `{marker}`"
            ));
        }
    }
    let insert_sha256 = sha256_hex(insert.as_bytes());
    if insert_sha256 != RECONCILIATION_REQUEST_INDEX_INSERT_AST_SHA256 {
        return Err(format!(
            "{relative} RequestIndex::insert AST drifted: expected {RECONCILIATION_REQUEST_INDEX_INSERT_AST_SHA256}, found {insert_sha256}"
        ));
    }

    let decision_function = exact_associated_method(file, "RequestIndex", "decision")?;
    let decision = compact_tokens(decision_function);
    for marker in [
        "self.event_targets.get(event.id_str())",
        "by_author.get(event.author_str())",
        "self.address_targets.get(coordinate)",
        "RadrootsNip09SuppressionReason::DeletionRequestImmune",
        "RadrootsNip09SuppressionReason::NoAuthorizedReference",
        "RadrootsNip09SuppressionReason::RequestAuthorMismatch",
        "RadrootsNip09SuppressionReason::AddressCutoffPrecedesTarget",
        "RadrootsNip09SuppressionReason::EventIdReference",
        "RadrootsNip09SuppressionReason::AddressReferenceAtOrBeforeCutoff",
        "RadrootsNip09SuppressionReason::EventIdAndAddressReference",
    ] {
        if !decision.contains(marker) {
            return Err(format!(
                "{relative} RequestIndex::decision is missing direct evidence authority `{marker}`"
            ));
        }
    }
    for forbidden in [
        "matching(",
        "evaluate_nip09_suppression",
        ".projection()",
        ".event_targets()",
        ".address_targets()",
        ".iter()",
        ".into_iter()",
    ] {
        if decision.contains(forbidden) {
            return Err(format!(
                "{relative} RequestIndex::decision must not iterate or rescan through `{forbidden}`"
            ));
        }
    }
    let decision_sha256 = sha256_hex(decision.as_bytes());
    if decision_sha256 != RECONCILIATION_REQUEST_INDEX_DECISION_AST_SHA256 {
        return Err(format!(
            "{relative} RequestIndex::decision AST drifted: expected {RECONCILIATION_REQUEST_INDEX_DECISION_AST_SHA256}, found {decision_sha256}"
        ));
    }

    let affected = compact_tokens(exact_free_function(
        file,
        "request_affected_addressable_coordinates",
    )?);
    for marker in [
        "for target in request.projection().event_targets()",
        "event_by_id.get(target.event_id().as_str())",
        "winners.get(coordinate)",
        "for target in request.projection().address_targets()",
        "winners.contains_key(&coordinate)",
    ] {
        let marker = marker.replace(' ', "");
        if !affected.contains(&marker) {
            return Err(format!(
                "{relative} affected-coordinate reducer is missing `{marker}`"
            ));
        }
    }
    let affected_sha256 = sha256_hex(affected.as_bytes());
    if affected_sha256 != RECONCILIATION_AFFECTED_COORDINATES_AST_SHA256 {
        return Err(format!(
            "{relative} affected-coordinate reducer AST drifted: expected {RECONCILIATION_AFFECTED_COORDINATES_AST_SHA256}, found {affected_sha256}"
        ));
    }

    let desired = compact_tokens(exact_free_function(file, "desired_addressable_states")?);
    let history = compact_tokens(exact_free_function(file, "expected_transition_history")?);
    let state = compact_tokens(exact_free_function(file, "addressable_state_for_event")?);
    for (function, source, markers) in [
        (
            "desired_addressable_states",
            desired.as_str(),
            &["RequestIndex::new(requests)", "&request_index"][..],
        ),
        (
            "expected_transition_history",
            history.as_str(),
            &[
                "letmutrequest_index=RequestIndex::new(&requests)",
                "request_affected_addressable_coordinates(",
                "request_index.insert(&request)",
                "&request_index",
            ][..],
        ),
        (
            "addressable_state_for_event",
            state.as_str(),
            &["request_index.decision(event.verified_event.event())?"][..],
        ),
    ] {
        for marker in markers {
            if !source.contains(marker) {
                return Err(format!(
                    "{relative}::{function} is missing direct indexed authority `{marker}`"
                ));
            }
        }
        for forbidden in [
            "matching(",
            "request_references_event",
            "evaluate_nip09_suppression",
        ] {
            if source.contains(forbidden) {
                return Err(format!(
                    "{relative}::{function} contains projection-rescanning authority `{forbidden}`"
                ));
            }
        }
    }
    if history.matches("RequestIndex::new(&requests)").count() != 1 {
        return Err(format!(
            "{relative}::expected_transition_history must build its request index exactly once"
        ));
    }
    Ok(())
}

fn validate_visibility_oracle_index_authority(
    file: &syn::File,
    relative: &str,
) -> Result<(), String> {
    let expected_visibility = compact_tokens(exact_free_function(file, "expected_visibility")?);
    if expected_visibility
        .matches("request_index.decision(envelope)")
        .count()
        != 1
    {
        return Err(format!(
            "{relative} expected visibility must make exactly one direct indexed suppression decision per admitted event"
        ));
    }
    for forbidden in [
        "matching(",
        "evaluate_nip09_suppression",
        ".projection()",
        ".event_targets()",
        ".address_targets()",
    ] {
        if expected_visibility.contains(forbidden) {
            return Err(format!(
                "{relative} expected visibility must not use projection-rescanning authority `{forbidden}`"
            ));
        }
    }
    let expected_visibility_sha256 = sha256_hex(expected_visibility.as_bytes());
    if expected_visibility_sha256 != VISIBILITY_ORACLE_EXPECTED_VISIBILITY_AST_SHA256 {
        return Err(format!(
            "{relative} expected_visibility AST drifted: expected {VISIBILITY_ORACLE_EXPECTED_VISIBILITY_AST_SHA256}, found {expected_visibility_sha256}"
        ));
    }

    let request_index = exact_impl(file, "OracleRequestIndexV1<'a>")?;
    let methods = request_index
        .items
        .iter()
        .filter_map(|item| match item {
            syn::ImplItem::Fn(function) => Some(function.sig.ident.to_string()),
            _ => None,
        })
        .collect::<Vec<_>>();
    if methods != ["new", "decision"] {
        return Err(format!(
            "{relative} OracleRequestIndexV1 method inventory must be exactly [new, decision]; found {methods:?}"
        ));
    }
    let constructor = compact_tokens(exact_associated_method(
        file,
        "OracleRequestIndexV1<'a>",
        "new",
    )?);
    for (marker, count) in [
        (".projection()", 2),
        (".event_targets()", 1),
        (".address_targets()", 1),
    ] {
        if constructor.matches(marker).count() != count {
            return Err(format!(
                "{relative} OracleRequestIndexV1::new must contain exactly {count} indexed construction use(s) of `{marker}`"
            ));
        }
    }

    let decision_function = exact_associated_method(file, "OracleRequestIndexV1<'a>", "decision")?;
    let decision = compact_tokens(decision_function);
    for marker in [
        "self.event_targets.get(event.id_str())",
        "by_author.get(event.author_str())",
        "self.address_targets.get(coordinate)",
        "self.requests[index].event()",
        "RadrootsNip09SuppressionReason::DeletionRequestImmune",
        "RadrootsNip09SuppressionReason::NoAuthorizedReference",
        "RadrootsNip09SuppressionReason::RequestAuthorMismatch",
        "RadrootsNip09SuppressionReason::AddressCutoffPrecedesTarget",
        "RadrootsNip09SuppressionReason::EventIdReference",
        "RadrootsNip09SuppressionReason::AddressReferenceAtOrBeforeCutoff",
        "RadrootsNip09SuppressionReason::EventIdAndAddressReference",
        "event_reference_request_id",
        "address_reference_request_id",
        "address_reference_cutoff",
    ] {
        if !decision.contains(marker) {
            return Err(format!(
                "{relative} direct indexed suppression decision is missing `{marker}`"
            ));
        }
    }
    for forbidden in [
        "matching(",
        "evaluate_nip09_suppression",
        ".projection()",
        ".event_targets()",
        ".address_targets()",
        ".iter()",
        ".into_iter()",
    ] {
        if decision.contains(forbidden) {
            return Err(format!(
                "{relative} direct indexed suppression decision must not iterate or rescan through `{forbidden}`"
            ));
        }
    }
    #[derive(Default)]
    struct IterationAudit {
        for_loops: usize,
        while_loops: usize,
        loops: usize,
    }
    impl<'ast> syn::visit::Visit<'ast> for IterationAudit {
        fn visit_expr_for_loop(&mut self, expression: &'ast syn::ExprForLoop) {
            self.for_loops += 1;
            syn::visit::visit_expr_for_loop(self, expression);
        }

        fn visit_expr_while(&mut self, expression: &'ast syn::ExprWhile) {
            self.while_loops += 1;
            syn::visit::visit_expr_while(self, expression);
        }

        fn visit_expr_loop(&mut self, expression: &'ast syn::ExprLoop) {
            self.loops += 1;
            syn::visit::visit_expr_loop(self, expression);
        }
    }
    use syn::visit::Visit;
    let mut iteration_audit = IterationAudit::default();
    iteration_audit.visit_block(&decision_function.block);
    if iteration_audit.for_loops != 0
        || iteration_audit.while_loops != 0
        || iteration_audit.loops != 0
    {
        return Err(format!(
            "{relative} direct indexed suppression decision must not contain loops"
        ));
    }
    let decision_sha256 = sha256_hex(decision.as_bytes());
    if decision_sha256 != VISIBILITY_ORACLE_DECISION_AST_SHA256 {
        return Err(format!(
            "{relative} OracleRequestIndexV1::decision AST drifted: expected {VISIBILITY_ORACLE_DECISION_AST_SHA256}, found {decision_sha256}"
        ));
    }
    Ok(())
}

fn validate_public_entry_point_authority(file: &syn::File, relative: &str) -> Result<(), String> {
    validate_public_method_signatures(file)?;
    let expected_methods = [
        (
            "rebuild_from_raw_v1",
            r#"{
                crate::nip09::reconciliation_v1::rebuild_from_raw_v1_on_pool(&self.pool).await
            }"#,
        ),
        (
            "repair_file_from_raw_v1",
            r#"{
                let canonical_path = canonical_raw_source_repair_main_path_v1(path.as_ref())?;
                let options = SqliteConnectOptions::new()
                    .filename(&canonical_path)
                    .create_if_missing(false);
                let pool = SqlitePoolOptions::new()
                    .max_connections(1)
                    .connect_with(options)
                    .await?;
                pool.set_connect_options(raw_source_repair_connect_options_v1(&canonical_path));
                let mut connection = pool.acquire().await?;
                prepare_raw_source_repair_connection_v1(&mut connection, &canonical_path).await?;
                let transaction = connection.begin_with("BEGIN IMMEDIATE").await?;
                if let Err(primary) =
                    validate_raw_source_repair_canonical_lock_domain_v1(&canonical_path).await
                {
                    return preserve_raw_source_repair_probe_failure(primary, transaction.rollback().await);
                }
                let report =
                    crate::nip09::reconciliation_v1::rebuild_from_raw_v1_in_existing_transaction(
                        transaction,
                    )
                    .await?;
                drop(connection);
                Ok((Self { pool }, report))
            }"#,
        ),
    ];
    for (method, expected_body) in expected_methods {
        let actual = exact_associated_method(file, "RadrootsEventStore", method)?;
        let expected = syn::parse_str::<syn::Block>(expected_body)
            .map_err(|error| format!("parse governed {method} body: {error}"))?;
        if compact_tokens(&actual.block) != compact_tokens(&expected) {
            return Err(format!(
                "{relative}::RadrootsEventStore::{method} must retain its exact governed rebuild/cold-repair call path; expected `{}`, found `{}`",
                compact_tokens(&expected),
                compact_tokens(&actual.block),
            ));
        }
    }

    let full = compact_tokens(file);
    for forbidden in [
        "repair_pool_from_raw_v1",
        "RADROOTS_EVENT_STORE_RAW_SOURCE_REPAIR_POOL_CONNECTION_LIMIT_V1",
        "RawSourceRepairPoolConnectionLimitExceeded",
        "RawSourceRepairPoolDatabaseIdentityMismatch",
        "RawSourceRepairRequiresFileBackedDatabase",
        "PoolTempSchemaPolicy::RawSourceRepairV1",
    ] {
        if full.contains(forbidden) {
            return Err(format!(
                "{relative} file-only cold repair must not retain obsolete authority `{forbidden}`"
            ));
        }
    }

    let expected_functions = [
        (
            "prepare_raw_source_repair_connection_v1",
            "validated cold-repair connection preflight",
            r#"async fn prepare_raw_source_repair_connection_v1(
                connection: &mut SqliteConnection,
                canonical_path: &Path,
            ) -> Result<(), RadrootsEventStoreError> {
                let main_filename = main_database_filename(connection).await?;
                let actual = canonical_raw_source_repair_main_path_v1(Path::new(&main_filename))?;
                if actual != canonical_path {
                    return Err(
                        RadrootsEventStoreError::RawSourceRepairDatabaseIdentityMismatch {
                            expected: canonical_path.display().to_string(),
                            actual: actual.display().to_string(),
                        },
                    );
                }
                validate_main_database_encoding(connection).await?;
                crate::schema::validate_exact_managed_v4_for_raw_source_rebuild_v1(connection)
                    .await?;
                validate_file_journal_mode_is_wal(connection).await?;
                sqlx::query("PRAGMA foreign_keys = ON")
                    .execute(&mut *connection)
                    .await?;
                sqlx::query("PRAGMA busy_timeout = 5000")
                    .execute(&mut *connection)
                    .await?;
                Ok(())
            }"#,
        ),
        (
            "raw_source_repair_connect_options_v1",
            "sealed future cold-repair connection options",
            r#"fn raw_source_repair_connect_options_v1(canonical_path: &Path) -> SqliteConnectOptions {
                SqliteConnectOptions::new()
                    .filename(canonical_path)
                    .create_if_missing(false)
                    .journal_mode(SqliteJournalMode::Wal)
                    .foreign_keys(true)
                    .busy_timeout(Duration::from_millis(5_000))
            }"#,
        ),
        (
            "validate_raw_source_repair_canonical_lock_domain_v1",
            "canonical-path SQLite writer-lock-domain probe",
            r#"async fn validate_raw_source_repair_canonical_lock_domain_v1(
                canonical_path: &Path,
            ) -> Result<(), RadrootsEventStoreError> {
                let mut candidate = SqliteConnection::connect_with(
                    &SqliteConnectOptions::new()
                        .filename(canonical_path)
                        .create_if_missing(false)
                        .foreign_keys(true)
                        .busy_timeout(Duration::ZERO),
                )
                .await?;
                let candidate_filename = main_database_filename(&mut candidate).await?;
                let candidate_path =
                    canonical_raw_source_repair_main_path_v1(Path::new(&candidate_filename))?;
                if candidate_path != canonical_path {
                    return Err(
                        RadrootsEventStoreError::RawSourceRepairDatabaseIdentityMismatch {
                            expected: canonical_path.display().to_string(),
                            actual: candidate_path.display().to_string(),
                        },
                    );
                }
                validate_main_database_encoding(&mut candidate).await?;
                crate::schema::validate_exact_managed_v4_for_raw_source_rebuild_v1(&mut candidate)
                    .await?;
                validate_file_journal_mode_is_wal(&mut candidate).await?;

                let mut probe = candidate.begin().await?;
                let write = sqlx::query(
                    "UPDATE main.radroots_event_store_write_lock SET lock_version = lock_version WHERE singleton = 1",
                )
                .execute(&mut *probe)
                .await;
                let rollback = probe.rollback().await;
                match write {
                    Ok(_) => preserve_raw_source_repair_probe_failure(
                        RadrootsEventStoreError::RawSourceRepairCanonicalPathLockDomainMismatch {
                            canonical_path: canonical_path.display().to_string(),
                        },
                        rollback,
                    ),
                    Err(error) => {
                        if sqlite_error_is_busy_or_locked(&error) {
                            rollback?;
                            Ok(())
                        } else {
                            preserve_raw_source_repair_probe_failure(error.into(), rollback)
                        }
                    }
                }
            }"#,
        ),
        (
            "preserve_raw_source_repair_probe_failure",
            "cold-repair lock-probe rollback error preservation",
            r#"fn preserve_raw_source_repair_probe_failure<T>(
                primary: RadrootsEventStoreError,
                rollback: Result<(), sqlx::Error>,
            ) -> Result<T, RadrootsEventStoreError> {
                match rollback {
                    Ok(()) => Err(primary),
                    Err(rollback) => Err(
                        RadrootsEventStoreError::RawSourceRebuildTransactionRollbackFailed {
                            primary: Box::new(primary),
                            rollback,
                        },
                    ),
                }
            }"#,
        ),
        (
            "canonical_raw_source_repair_main_path_v1",
            "existing canonical cold-repair file identity",
            r#"fn canonical_raw_source_repair_main_path_v1(
                path: &Path,
            ) -> Result<PathBuf, RadrootsEventStoreError> {
                let filename = path.display().to_string();
                std::fs::canonicalize(path).map_err(|source| {
                    RadrootsEventStoreError::RawSourceRepairMainDatabaseCanonicalizationFailed {
                        filename,
                        source,
                    }
                })
            }"#,
        ),
        (
            "sqlite_error_is_busy_or_locked",
            "SQLite writer-lock error classification",
            r#"fn sqlite_error_is_busy_or_locked(error: &sqlx::Error) -> bool {
                let sqlx::Error::Database(error) = error else {
                    return false;
                };
                error
                    .code()
                    .and_then(|code| code.parse::<i32>().ok())
                    .is_some_and(|code| code & 0xff == 5 || code & 0xff == 6)
            }"#,
        ),
    ];
    for (function, authority, expected_source) in expected_functions {
        let actual = exact_free_function(file, function)?;
        let expected = syn::parse_str::<syn::ItemFn>(expected_source)
            .map_err(|error| format!("parse governed {function}: {error}"))?;
        if compact_tokens(actual) != compact_tokens(&expected) {
            return Err(format!(
                "{relative}::{function} must retain its exact {authority}; expected `{}`, found `{}`",
                compact_tokens(&expected),
                compact_tokens(actual),
            ));
        }
    }
    Ok(())
}

fn validate_streaming_dependency_authority(workspace_root: &Path) -> Result<(), String> {
    let relative = "crates/event_store/Cargo.toml";
    let bytes = read_regular_file(workspace_root, relative)?;
    let source = std::str::from_utf8(&bytes)
        .map_err(|error| format!("{relative} must be UTF-8 TOML: {error}"))?;
    let manifest: toml::Value =
        toml::from_str(source).map_err(|error| format!("parse {relative}: {error}"))?;
    let sqlite = manifest
        .get("features")
        .and_then(|features| features.get("sqlite"))
        .and_then(toml::Value::as_array)
        .ok_or_else(|| format!("{relative} must define the sqlite feature array"))?;
    if !sqlite
        .iter()
        .any(|feature| feature.as_str() == Some("dep:futures"))
    {
        return Err(format!(
            "{relative} sqlite feature must enable the optional futures streaming dependency"
        ));
    }
    let futures = manifest
        .get("dependencies")
        .and_then(|dependencies| dependencies.get("futures"))
        .and_then(toml::Value::as_table)
        .ok_or_else(|| format!("{relative} must define futures as a dependency table"))?;
    if futures.get("workspace").and_then(toml::Value::as_bool) != Some(true)
        || futures.get("optional").and_then(toml::Value::as_bool) != Some(true)
    {
        return Err(format!(
            "{relative} futures dependency must remain workspace-governed and optional"
        ));
    }
    Ok(())
}

fn validate_coordinator_authority(file: &syn::File, relative: &str) -> Result<(), String> {
    let serialized = compact_tokens(exact_free_function(
        file,
        "rebuild_from_raw_v1_on_pool_inner",
    )?);
    require_ordered_markers(
        relative,
        "serialized rebuild transaction",
        &serialized,
        &[
            "begin_with(\"BEGINIMMEDIATE\").await?",
            "rebuild_from_raw_v1_in_transaction_inner(",
            "finish_raw_source_rebuild_transaction(transaction,result).await",
        ],
    )?;

    let existing_transaction = compact_tokens(exact_free_function(
        file,
        "rebuild_from_raw_v1_in_existing_transaction",
    )?);
    require_ordered_markers(
        relative,
        "validated existing rebuild transaction",
        &existing_transaction,
        &[
            "rebuild_from_raw_v1_in_transaction_inner(",
            "&OsSourceGenerationProvider",
            "finish_raw_source_rebuild_transaction(transaction,result).await",
        ],
    )?;

    let coordinator = compact_tokens(exact_free_function(
        file,
        "rebuild_from_raw_v1_in_transaction_inner",
    )?);
    require_ordered_markers(
        relative,
        "raw-source rebuild coordinator",
        &coordinator,
        &[
            "validate_exact_managed_v4_for_raw_source_rebuild_v1(connection).await?",
            "preflight_caller_owned_schema_dependencies_v1(connection,caller_schema_limits).await?",
            "validate_rebuild_marker_absent(connection).await?",
            "validate_source_capacity_authority_full_v1(connection).await?",
            "preflight_source_generation_append_v1(connection).await?",
            "load_reconciliation_snapshot(",
            "transition_high_water_v1(connection).await?",
            "validate_source_lineage_for_rebuild_v1(connection,&snapshot.events,transition_floor_seq)",
            "immutable_raw_digest_v1(connection).await?",
            "generation_provider.fill_generation(&mutgeneration_bytes)?",
            "open_source_rebuild_marker(connection,&plan).await?",
            "RawSourceRebuildFailpointV1::AfterMarkerOpen",
            "append_source_generation(connection,&plan).await?",
            "rotate_source_state(connection,&plan).await?",
            "RawSourceRebuildFailpointV1::AfterGenerationRotation",
            "prepare_transition_sqlite_sequence_v1(connection,transition_floor_seq).await?",
            "reconcile_raw_events(connection,&snapshot.events).await?",
            "persist_event_coordinate_facts(connection,generation,&snapshot.events).await?",
            "rebuild_raw_heads(connection,&snapshot.events).await?",
            "persist_nip09_facts(connection,generation,&snapshot.events).await?",
            "synchronize_addressable_heads(",
            "update_source_authority(",
            "transition_high_water_v1(connection).await?",
            "validate_transition_sqlite_sequence_v1(connection,transition_sequence_rowid,replay_transition_high_water,).await?",
            "validate_raw_source_rebuild_core_with_events_v1(connection,generation,&snapshot.events)",
            "RawSourceRebuildFailpointV1::AfterCoreReplay",
            "load_derived_visibility_rows_v1(connection,generation).await?",
            "audit_current_visibility_from_raw_v1(&snapshot.events,derived_visibility).await?",
            "RawSourceRebuildFailpointV1::AfterVisibilityAudit",
            "bind_source_capacity_to_generation_v1(connection,generation)",
            "reset_and_replay_food_availability_from_raw_v1(connection,generation).await?",
            "RawSourceRebuildFailpointV1::AfterFoodResetAndReplay",
            "validate_food_availability_projection_hook_v1(connection).await?",
            "RawSourceRebuildFailpointV1::AfterFoodAudit",
            "immutable_raw_digest_v1(connection).await?",
            "final_raw_digest!=immutable_raw_digest",
            "close_source_rebuild_marker(connection,marker).await?",
            "RawSourceRebuildFailpointV1::AfterMarkerClose",
            "validate_active_hook_state_fast(connection).await?",
            "validate_source_capacity_authority_fast_v1(connection).await?",
            "validate_food_availability_projection_hook_state_fast_v1(connection).await?",
            "validate_scoped_integrity_v1(connection).await?",
            "active_product_state_digest_v1(connection,generation).await?",
            "RadrootsEventStoreRawSourceRebuildReportV1",
        ],
    )?;
    if coordinator
        .matches("preflight_caller_owned_schema_dependencies_v1(")
        .count()
        != 1
    {
        return Err(format!(
            "{relative} rebuild coordinator must run the caller-schema dependency preflight exactly once"
        ));
    }
    if coordinator
        .matches("inject_raw_source_rebuild_failpoint_v1(")
        .count()
        != REBUILD_FAILPOINTS.len()
    {
        return Err(format!(
            "{relative} rebuild coordinator must inject exactly one failpoint for each governed stage"
        ));
    }
    for variant in REBUILD_FAILPOINTS.iter().map(|failpoint| failpoint.variant) {
        if coordinator
            .matches(&format!("RawSourceRebuildFailpointV1::{variant}"))
            .count()
            != 1
        {
            return Err(format!(
                "{relative} rebuild coordinator must inject `{variant}` exactly once"
            ));
        }
    }
    if coordinator.contains("projection_cursor") {
        return Err(
            "raw-source rebuild coordinator must not enumerate or mutate generic projection cursors"
                .to_owned(),
        );
    }

    let finish = compact_tokens(exact_free_function(
        file,
        "finish_raw_source_rebuild_transaction",
    )?);
    for marker in [
        "transaction.commit().await?",
        "transaction.rollback().await",
        "preserve_raw_source_rebuild_primary_failure(primary,rollback)",
    ] {
        if !finish.contains(marker) {
            return Err(format!(
                "{relative} transaction finalizer is missing `{marker}`"
            ));
        }
    }
    Ok(())
}

fn validate_failpoint_authority(file: &syn::File, relative: &str) -> Result<(), String> {
    let mut actual_enum = exact_enum(file, "RawSourceRebuildFailpointV1")?.clone();
    strip_doc_attributes(&mut actual_enum.attrs);
    let variants = REBUILD_FAILPOINTS
        .iter()
        .map(|failpoint| format!("{},", failpoint.variant))
        .collect::<Vec<_>>()
        .join("\n");
    let expected_enum = syn::parse_str::<syn::ItemEnum>(&format!(
        r#"
        #[cfg(test)]
        #[allow(clippy::enum_variant_names)]
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub(crate) enum RawSourceRebuildFailpointV1 {{
            {variants}
        }}
        "#,
    ))
    .map_err(|error| format!("parse governed failpoint enum: {error}"))?;
    if compact_tokens(&actual_enum) != compact_tokens(&expected_enum) {
        return Err(format!(
            "{relative} must retain the exact governed test-only rebuild failpoint enum"
        ));
    }

    let actual_impl = exact_impl(file, "RawSourceRebuildFailpointV1")?;
    let match_arms = REBUILD_FAILPOINTS
        .iter()
        .map(|failpoint| format!("Self::{} => {:?},", failpoint.variant, failpoint.id))
        .collect::<Vec<_>>()
        .join("\n");
    let expected_impl = syn::parse_str::<syn::ItemImpl>(&format!(
        r#"
        #[cfg(test)]
        impl RawSourceRebuildFailpointV1 {{
            const fn as_str(self) -> &'static str {{
                match self {{
                    {match_arms}
                }}
            }}
        }}
        "#,
    ))
    .map_err(|error| format!("parse governed failpoint mapping: {error}"))?;
    if compact_tokens(actual_impl) != compact_tokens(&expected_impl) {
        return Err(format!(
            "{relative} must retain the exact failpoint-to-stage mapping"
        ));
    }

    let mut actual_injector =
        exact_free_function(file, "inject_raw_source_rebuild_failpoint_v1")?.clone();
    strip_doc_attributes(&mut actual_injector.attrs);
    let expected_injector = syn::parse_str::<syn::ItemFn>(
        r#"
        #[cfg(test)]
        fn inject_raw_source_rebuild_failpoint_v1(
            selected: Option<RawSourceRebuildFailpointV1>,
            stage: RawSourceRebuildFailpointV1,
        ) -> Result<(), RadrootsEventStoreError> {
            if selected == Some(stage) {
                return rebuild_drift(
                    RadrootsEventStoreRawSourceRebuildDriftV1::RebuildPostcondition,
                    format!("injected raw-source rebuild failure at {}", stage.as_str()),
                );
            }
            Ok(())
        }
        "#,
    )
    .map_err(|error| format!("parse governed failpoint injector: {error}"))?;
    if compact_tokens(&actual_injector) != compact_tokens(&expected_injector) {
        return Err(format!(
            "{relative} must retain the exact rollback failpoint injector"
        ));
    }
    validate_failpoint_injection_authority(file, relative)
}

fn validate_failpoint_injection_authority(file: &syn::File, relative: &str) -> Result<(), String> {
    #[derive(Default)]
    struct InjectionAudit {
        calls: Vec<String>,
        propagated_calls: Vec<String>,
    }

    impl<'ast> syn::visit::Visit<'ast> for InjectionAudit {
        fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
            if compact_tokens(call.func.as_ref()) == "inject_raw_source_rebuild_failpoint_v1" {
                self.calls.push(compact_tokens(call));
            }
            syn::visit::visit_expr_call(self, call);
        }

        fn visit_expr_try(&mut self, expression: &'ast syn::ExprTry) {
            if let syn::Expr::Call(call) = expression.expr.as_ref()
                && compact_tokens(call.func.as_ref()) == "inject_raw_source_rebuild_failpoint_v1"
            {
                self.propagated_calls.push(compact_tokens(call));
            }
            syn::visit::visit_expr_try(self, expression);
        }
    }

    let expected = REBUILD_FAILPOINTS
        .iter()
        .map(|failpoint| {
            format!(
                "inject_raw_source_rebuild_failpoint_v1(_failpoint,RawSourceRebuildFailpointV1::{},)",
                failpoint.variant
            )
        })
        .collect::<Vec<_>>();
    use syn::visit::Visit;
    let coordinator = exact_free_function(file, "rebuild_from_raw_v1_in_transaction_inner")?;
    let mut audit = InjectionAudit::default();
    audit.visit_block(&coordinator.block);
    if audit.calls != expected || audit.propagated_calls != expected {
        return Err(format!(
            "{relative} rebuild coordinator must contain exactly one ordered, error-propagating injection call for every governed failpoint: expected {expected:?}, calls {:?}, propagated {:?}",
            audit.calls, audit.propagated_calls,
        ));
    }
    Ok(())
}

fn validate_failpoint_test_array_authority(file: &syn::File, relative: &str) -> Result<(), String> {
    struct FailpointArrayAudit {
        arrays: Vec<Vec<String>>,
    }

    impl<'ast> syn::visit::Visit<'ast> for FailpointArrayAudit {
        fn visit_expr_for_loop(&mut self, expression: &'ast syn::ExprForLoop) {
            if compact_tokens(expression.pat.as_ref()) == "(index,failpoint)"
                && let syn::Expr::MethodCall(enumerate) = expression.expr.as_ref()
                && enumerate.method == "enumerate"
                && enumerate.args.is_empty()
                && let syn::Expr::MethodCall(into_iter) = enumerate.receiver.as_ref()
                && into_iter.method == "into_iter"
                && into_iter.args.is_empty()
                && let syn::Expr::Array(array) = into_iter.receiver.as_ref()
            {
                self.arrays
                    .push(array.elems.iter().map(compact_tokens).collect());
            }
            syn::visit::visit_expr_for_loop(self, expression);
        }
    }

    let expected = REBUILD_FAILPOINTS
        .iter()
        .map(|failpoint| format!("RawSourceRebuildFailpointV1::{}", failpoint.variant))
        .collect::<Vec<_>>();
    use syn::visit::Visit;
    let test = exact_top_level_function(file, REBUILD_FAILPOINT_TEST)?;
    let mut audit = FailpointArrayAudit { arrays: Vec::new() };
    audit.visit_block(&test.block);
    let [actual] = audit.arrays.as_slice() else {
        return Err(format!(
            "{relative}::{REBUILD_FAILPOINT_TEST} must contain exactly one governed failpoint array loop"
        ));
    };
    if actual != &expected {
        return Err(format!(
            "{relative}::{REBUILD_FAILPOINT_TEST} must enumerate the exact governed failpoint array once and in order"
        ));
    }
    Ok(())
}

fn validate_bounded_repair_schema_authority(
    file: &syn::File,
    relative: &str,
) -> Result<(), String> {
    let authority = compact_tokens(exact_free_function(
        file,
        "repair_governed_catalog_authority_v1",
    )?);
    for marker in [
        "owned_object_names.iter().copied()",
        "names.insert(EVENT_STORE_LEDGER_NAME)",
        "canonical_row_count.checked_add(1)",
    ] {
        if !authority.contains(marker) {
            return Err(format!(
                "{relative} repair catalog bound authority is missing `{marker}`"
            ));
        }
    }

    let catalog = compact_tokens(exact_free_function(file, "read_repair_catalog_bounded_v1")?);
    for marker in [
        "repair_governed_catalog_authority_v1(registry)?",
        "json_each(?)",
        "FROMmain.sqlite_schema",
        "lower(substr(name,1,7))!='sqlite_'",
        "nameCOLLATENOCASEIN(SELECTnameFROMgoverned)",
        "tbl_nameCOLLATENOCASEIN(SELECTnameFROMgoverned)",
        "EVENT_STORE_RESERVED_PREFIX",
        "LIMIT?",
        ".bind(row_limit)",
        ".fetch_all(&mut*connection)",
    ] {
        if !catalog.contains(marker) {
            return Err(format!(
                "{relative} bounded repair catalog reader is missing `{marker}`"
            ));
        }
    }

    let temp = compact_tokens(exact_free_function(
        file,
        "validate_repair_temp_schema_bounded_v1",
    )?);
    for marker in [
        "repair_governed_catalog_authority_v1(registry)?",
        "json_each(?)",
        "FROMtemp.sqlite_schema",
        "typeIN('trigger','view')",
        "nameCOLLATENOCASEIN(SELECTnameFROMgoverned)",
        "tbl_nameCOLLATENOCASEIN(SELECTnameFROMgoverned)",
        "EVENT_STORE_RESERVED_PREFIX",
        "LIMIT1",
        ".fetch_optional(&mut*connection)",
        "TemporarySchemaCollision",
    ] {
        if !temp.contains(marker) {
            return Err(format!(
                "{relative} bounded repair temp-collision probe is missing `{marker}`"
            ));
        }
    }

    let history = compact_tokens(exact_free_function(file, "read_repair_history_bounded_v1")?);
    for marker in [
        "i64::from(supported_current).checked_add(1)",
        "FROMmain.radroots_event_store_schema_migrations",
        "ORDERBYversion",
        "LIMIT?",
        ".bind(row_limit)",
        ".fetch_all(&mut*connection)",
    ] {
        if !history.contains(marker) {
            return Err(format!(
                "{relative} bounded repair migration-history reader is missing `{marker}`"
            ));
        }
    }
    Ok(())
}

fn validate_transition_sequence_authority(file: &syn::File, relative: &str) -> Result<(), String> {
    if exact_string_const(file, "TRANSITION_SEQUENCE_NAME")?
        != "radroots_event_store_addressable_head_transition"
    {
        return Err(format!(
            "{relative} transition sqlite_sequence target identity drifted"
        ));
    }
    let prepare = compact_tokens(exact_free_function(
        file,
        "prepare_transition_sqlite_sequence_v1",
    )?);
    require_ordered_markers(
        relative,
        "target-first sqlite_sequence preparation",
        &prepare,
        &[
            "transition_max<0||transition_max==i64::MAX",
            "SELECTrowid,name=?COLLATENOCASEFROMmain.sqlite_sequenceORDERBYrowidLIMIT1",
            ".bind(TRANSITION_SEQUENCE_NAME)",
            "None=>-1",
            "Some((rowid,Some(1)))=>rowid",
            "Some((i64::MIN,_))=>",
            "Some((rowid,_))=>rowid-1",
            "DELETEFROMmain.sqlite_sequenceWHEREnameCOLLATENOCASE=?",
            "INSERTINTOmain.sqlite_sequence(rowid,name,seq)VALUES(?,?,?)",
            ".bind(target_rowid)",
            ".bind(TRANSITION_SEQUENCE_NAME)",
            ".bind(transition_max)",
            "validate_transition_sqlite_sequence_v1(connection,target_rowid,transition_max).await?",
            "Ok(target_rowid)",
        ],
    )?;
    if prepare
        .matches("DELETEFROMmain.sqlite_sequenceWHEREnameCOLLATENOCASE=?")
        .count()
        != 1
    {
        return Err(format!(
            "{relative} target aliases must be removed by exactly one shared sqlite_sequence scan"
        ));
    }

    let validate = compact_tokens(exact_free_function(
        file,
        "validate_transition_sqlite_sequence_v1",
    )?);
    require_ordered_markers(
        relative,
        "target-first sqlite_sequence validation",
        &validate,
        &[
            "SELECTname,seqFROMmain.sqlite_sequenceWHERErowid=?",
            ".bind(target_rowid)",
            "SELECTrowidFROMmain.sqlite_sequenceORDERBYrowidLIMIT1",
            "first_rowid!=Some(target_rowid)",
            "AddressableTransitionAuthority",
        ],
    )?;
    let full_source = compact_tokens(file);
    for forbidden in [
        "normalize_transition_sqlite_sequence_v1",
        "TRANSITION_SEQUENCE_RESERVED_ROWID_V1",
        "unrelated_sqlite_sequence_snapshot_v1",
        "validate_unrelated_sqlite_sequences_v1",
        "quote(name)",
        "nameISNULLORname!=?",
    ] {
        if full_source.contains(forbidden) {
            return Err(format!(
                "{relative} must not scan or promote unrelated sqlite_sequence rows through `{forbidden}`"
            ));
        }
    }
    Ok(())
}

fn validate_caller_schema_dependency_authority(
    file: &syn::File,
    relative: &str,
) -> Result<(), String> {
    for name in [
        "RAW_SOURCE_REBUILD_CALLER_MAIN_TABLE_COUNT_LIMIT_V1",
        "RAW_SOURCE_REBUILD_CALLER_FOREIGN_KEY_ROW_COUNT_LIMIT_V1",
    ] {
        let matches = file
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Const(item) if item.ident == name => Some(item),
                _ => None,
            })
            .collect::<Vec<_>>();
        let [actual] = matches.as_slice() else {
            return Err(format!(
                "{relative} must define caller-schema limit `{name}` exactly once; found {}",
                matches.len()
            ));
        };
        let mut actual = (*actual).clone();
        strip_doc_attributes(&mut actual.attrs);
        let expected = syn::parse_str::<syn::ItemConst>(&format!("const {name}: u32 = 4_096;"))
            .map_err(|error| format!("parse caller-schema limit `{name}`: {error}"))?;
        if compact_tokens(&actual) != compact_tokens(&expected) {
            return Err(format!(
                "{relative} caller-schema limit `{name}` must remain exactly 4,096"
            ));
        }
    }

    let limits = exact_struct(file, "RawSourceRebuildCallerSchemaLimitsV1")?;
    let expected_limits = syn::parse_str::<syn::ItemStruct>(
        r#"
        #[derive(Clone, Copy)]
        struct RawSourceRebuildCallerSchemaLimitsV1 {
            main_tables: u32,
            foreign_key_rows: u32,
        }
        "#,
    )
    .map_err(|error| format!("parse caller-schema limits model: {error}"))?;
    if compact_tokens(limits) != compact_tokens(&expected_limits) {
        return Err(format!(
            "{relative} caller-schema limits model field or derive authority drifted"
        ));
    }
    let limits_impl = exact_impl(file, "RawSourceRebuildCallerSchemaLimitsV1")?;
    let expected_limits_impl = syn::parse_str::<syn::ItemImpl>(
        r#"
        impl RawSourceRebuildCallerSchemaLimitsV1 {
            const fn production() -> Self {
                Self {
                    main_tables: RAW_SOURCE_REBUILD_CALLER_MAIN_TABLE_COUNT_LIMIT_V1,
                    foreign_key_rows: RAW_SOURCE_REBUILD_CALLER_FOREIGN_KEY_ROW_COUNT_LIMIT_V1,
                }
            }
        }
        "#,
    )
    .map_err(|error| format!("parse caller-schema production limits authority: {error}"))?;
    if compact_tokens(limits_impl) != compact_tokens(&expected_limits_impl) {
        return Err(format!(
            "{relative} caller-schema production limits must route through both governed constants"
        ));
    }

    for (function, expected_source) in [
        (
            "governed_schema_names_json_v1",
            r#"fn governed_schema_names_json_v1() -> Result<String, RadrootsEventStoreError> {
                let mut names = EVENT_STORE_MIGRATIONS
                    .iter()
                    .flat_map(|migration| migration.owned_object_names.iter().copied())
                    .collect::<BTreeSet<_>>();
                names.insert(EVENT_STORE_LEDGER_NAME);
                Ok(serde_json::to_string(&names)?)
            }"#,
        ),
        (
            "caller_schema_count_v1",
            r#"fn caller_schema_count_v1(count: i64) -> Result<u64, RadrootsEventStoreError> {
                u64::try_from(count).map_err(|_| {
                    rebuild_state_error(
                        RadrootsEventStoreRawSourceRebuildDriftV1::ManagedSchemaAuthority,
                        "caller-owned schema inventory returned a negative row count",
                    )
                })
            }"#,
        ),
    ] {
        let actual = exact_free_function(file, function)?;
        let expected = syn::parse_str::<syn::ItemFn>(expected_source)
            .map_err(|error| format!("parse caller-schema helper `{function}`: {error}"))?;
        if compact_tokens(actual) != compact_tokens(&expected) {
            return Err(format!(
                "{relative}::{function} caller-schema authority drifted"
            ));
        }
    }

    let preflight_function =
        exact_free_function(file, "preflight_caller_owned_schema_dependencies_v1")?;
    let preflight = compact_tokens(preflight_function);
    let query_literals = sqlx_query_family_literals(preflight_function);
    if query_literals.len() != 3 {
        return Err(format!(
            "{relative} caller-schema preflight must contain exactly three literal sqlx queries; found {}",
            query_literals.len()
        ));
    }
    let query_authority = query_literals
        .iter()
        .map(|query| query.split_whitespace().collect::<String>())
        .collect::<Vec<_>>()
        .join("\0");
    require_ordered_markers(
        relative,
        "bounded caller-schema dependency preflight",
        &preflight,
        &[
            "governed_schema_names_json_v1()?",
            "serde_json::to_string(RAW_SOURCE_REBUILD_MUTATED_PARENT_TABLES_V1)?",
            "i64::from(limits.main_tables)+1",
            "RawSourceRebuildCallerTableCapacityExceeded",
            "i64::from(limits.foreign_key_rows)+1",
            "RawSourceRebuildCallerForeignKeyCapacityExceeded",
            "RawSourceRebuildCallerInboundForeignKeyUnsupported",
            "Box::new(RadrootsEventStoreCallerInboundForeignKeyV1",
        ],
    )?;
    require_ordered_markers(
        relative,
        "caller-schema dependency SQL",
        &query_authority,
        &[
            "FROMmain.sqlite_schemaASchild",
            "LIMIT?",
            "main.pragma_foreign_key_list(child.name,'main')ASforeign_key",
            "JOINrebuild_parentONforeign_key.\"table\"COLLATENOCASE=rebuild_parent.name",
            "ORDERBYchild.nameCOLLATENOCASE,child.name,foreign_key.id,foreign_key.seq",
            "LIMIT1",
        ],
    )?;
    for (marker, expected_count) in [
        ("FROMmain.sqlite_schemaASchild", 3),
        (
            "main.pragma_foreign_key_list(child.name,'main')ASforeign_key",
            2,
        ),
        ("LIMIT?", 2),
        ("LIMIT1", 1),
    ] {
        if query_authority.matches(marker).count() != expected_count {
            return Err(format!(
                "{relative} caller-schema preflight must contain `{marker}` exactly {expected_count} time(s)"
            ));
        }
    }
    for forbidden in [
        "temp.sqlite_schema",
        "temp.pragma_foreign_key_list",
        "foreign_key.on_delete=",
        "foreign_key.on_update=",
    ] {
        if query_authority.contains(forbidden) {
            return Err(format!(
                "{relative} caller-schema preflight must not narrow or redirect dependency discovery through `{forbidden}`"
            ));
        }
    }
    let actual_sha256 = sha256_hex(preflight.as_bytes());
    if actual_sha256 != CALLER_SCHEMA_PREFLIGHT_AST_SHA256 {
        return Err(format!(
            "{relative} caller-schema dependency preflight AST drifted: expected {CALLER_SCHEMA_PREFLIGHT_AST_SHA256}, found {actual_sha256}"
        ));
    }
    Ok(())
}

fn validate_scoped_integrity_authority(file: &syn::File, relative: &str) -> Result<(), String> {
    let actual_tables = exact_string_slice_const(file, "REBUILD_OWNED_TABLES_V1")?;
    let expected_tables = owned(SCOPED_INTEGRITY_TABLES);
    if actual_tables != expected_tables {
        return Err(format!(
            "{relative} scoped integrity table inventory differs: expected {expected_tables:?}, found {actual_tables:?}"
        ));
    }
    let actual_mutated_parents =
        exact_string_slice_const(file, "RAW_SOURCE_REBUILD_MUTATED_PARENT_TABLES_V1")?;
    let expected_mutated_parents = owned(CALLER_INBOUND_FOREIGN_KEY_PARENT_TABLES);
    if actual_mutated_parents != expected_mutated_parents {
        return Err(format!(
            "{relative} rebuild-mutated parent inventory differs: expected {expected_mutated_parents:?}, found {actual_mutated_parents:?}"
        ));
    }
    let integrity = compact_tokens(exact_free_function(file, "validate_scoped_integrity_v1")?);
    for marker in [
        "fortableinREBUILD_OWNED_TABLES_V1",
        "PRAGMAmain.integrity_check('{table}')",
        "sqlx::AssertSqlSafe(integrity_sql)",
        "detail!=\"ok\"",
        "PRAGMAmain.foreign_key_check('{table}')",
        "sqlx::AssertSqlSafe(foreign_key_sql)",
        ".fetch_optional(&mut*connection)",
        "ForeignKeyViolation",
        "radroots_event_store_food_availability_search_fts(radroots_event_store_food_availability_search_fts)VALUES('integrity-check')",
        "Fts5IntegrityCheckFailed",
    ] {
        if !integrity.contains(marker) {
            return Err(format!(
                "{relative} scoped integrity authority is missing `{marker}`"
            ));
        }
    }
    if integrity.matches(".fetch_all(&mut*connection)").count() != 1
        || integrity
            .matches(".fetch_optional(&mut*connection)")
            .count()
            != 1
    {
        return Err(format!(
            "{relative} scoped integrity must materialize only bounded integrity-check rows and fetch at most one foreign-key violation"
        ));
    }
    for forbidden in [
        "PRAGMAintegrity_check\"",
        "PRAGMAmain.integrity_check\"",
        "PRAGMAforeign_key_check\"",
        "PRAGMAmain.foreign_key_check\"",
        "quick_check",
    ] {
        if integrity.contains(forbidden) {
            return Err(format!(
                "{relative} scoped integrity authority contains forbidden global scan `{forbidden}`"
            ));
        }
    }
    Ok(())
}

fn validate_digest_query_authority(file: &syn::File, relative: &str) -> Result<(), String> {
    validate_one_digest_query_authority(
        file,
        relative,
        "immutable_raw_digest_v1",
        "IMMUTABLE_RAW_DIGEST_DOMAIN_V1",
        RAW_DIGEST_QUERY_SPECS,
    )?;
    validate_one_digest_query_authority(
        file,
        relative,
        "active_product_state_digest_v1",
        "ACTIVE_PRODUCT_STATE_DIGEST_DOMAIN_V1",
        PRODUCT_DIGEST_QUERY_SPECS,
    )?;
    validate_digest_framing_authority(file, relative)
}

fn validate_one_digest_query_authority(
    file: &syn::File,
    relative: &str,
    function_name: &str,
    domain_name: &str,
    specs: &[DigestQuerySpec],
) -> Result<(), String> {
    let function = exact_free_function(file, function_name)?;
    let compact = compact_tokens(function);
    if !compact.contains(&format!("digest.update({domain_name})")) {
        return Err(format!(
            "{relative}::{function_name} does not begin from governed domain `{domain_name}`"
        ));
    }
    let actual_queries = sqlx_query_literals(function);
    let expected_queries = specs.iter().map(|spec| spec.sql).collect::<Vec<_>>();
    if actual_queries != expected_queries {
        return Err(format!(
            "{relative}::{function_name} digest query inventory differs from the governed source/component queries"
        ));
    }
    let actual_sections = digest_section_literals(function);
    let expected_sections = specs.iter().map(|spec| spec.section).collect::<Vec<_>>();
    if actual_sections != expected_sections {
        return Err(format!(
            "{relative}::{function_name} digest section order differs: expected {expected_sections:?}, found {actual_sections:?}"
        ));
    }
    let expected_fields = specs
        .iter()
        .flat_map(|spec| {
            spec.fields
                .iter()
                .copied()
                .zip(expected_digest_field_framing(spec.section).iter().copied())
                .map(|(name, framing)| (name.to_owned(), framing.to_owned()))
        })
        .collect::<Vec<_>>();
    let actual_fields = digest_field_witnesses(function)?;
    if actual_fields != expected_fields {
        return Err(format!(
            "{relative}::{function_name} typed digest field witness order differs from its governed queries: expected {expected_fields:?}, found {actual_fields:?}"
        ));
    }
    validate_digest_streaming_authority(function, relative, function_name, specs)?;
    Ok(())
}

fn validate_digest_streaming_authority(
    function: &syn::ItemFn,
    relative: &str,
    function_name: &str,
    specs: &[DigestQuerySpec],
) -> Result<(), String> {
    let compact = compact_tokens(function);
    if compact.contains(".fetch_all(") {
        return Err(format!(
            "{relative}::{function_name} must stream digest rows and must not materialize them with fetch_all"
        ));
    }

    let statements = &function.block.stmts;
    let mut witnessed_queries = Vec::new();
    for (index, statement) in statements.iter().enumerate() {
        let syn::Stmt::Local(local) = statement else {
            continue;
        };
        let Some(initializer) = &local.init else {
            continue;
        };
        let query_literals = sqlx_query_literals_in_expr(&initializer.expr);
        if query_literals.is_empty() {
            continue;
        }
        let [query] = query_literals.as_slice() else {
            return Err(format!(
                "{relative}::{function_name} digest stream binding must contain exactly one SQL query"
            ));
        };
        let syn::Pat::Ident(binding) = &local.pat else {
            return Err(format!(
                "{relative}::{function_name} digest query `{query}` must bind one named mutable stream"
            ));
        };
        if binding.mutability.is_none() || binding.by_ref.is_some() || binding.subpat.is_some() {
            return Err(format!(
                "{relative}::{function_name} digest query `{query}` must bind one plain mutable stream"
            ));
        }
        let stream = binding.ident.to_string();
        let syn::Expr::MethodCall(fetch) = initializer.expr.as_ref() else {
            return Err(format!(
                "{relative}::{function_name} digest query `{query}` must terminate in fetch"
            ));
        };
        if fetch.method != "fetch"
            || fetch.args.len() != 1
            || compact_tokens(fetch.args.first().expect("one fetch argument")) != "&mut*connection"
        {
            return Err(format!(
                "{relative}::{function_name} digest query `{query}` must stream via fetch(&mut *connection)"
            ));
        }

        let Some(syn::Stmt::Expr(syn::Expr::While(row_loop), _)) = statements.get(index + 1) else {
            return Err(format!(
                "{relative}::{function_name} digest stream `{stream}` must be consumed immediately by while let"
            ));
        };
        let syn::Expr::Let(condition) = row_loop.cond.as_ref() else {
            return Err(format!(
                "{relative}::{function_name} digest stream `{stream}` must use while let Some(row)"
            ));
        };
        if compact_tokens(&condition.pat) != "Some(row)"
            || compact_tokens(&condition.expr) != format!("{stream}.try_next().await?")
        {
            return Err(format!(
                "{relative}::{function_name} digest stream `{stream}` must terminate through try_next().await?"
            ));
        }

        let top_level_row_starts = row_loop
            .body
            .stmts
            .iter()
            .enumerate()
            .filter_map(|(index, statement)| {
                is_digest_row_start_statement(statement).then_some(index)
            })
            .collect::<Vec<_>>();
        struct RowStartCounter(usize);
        impl<'ast> syn::visit::Visit<'ast> for RowStartCounter {
            fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
                if compact_tokens(call) == "digest_row_start(&mutdigest)" {
                    self.0 += 1;
                }
                syn::visit::visit_expr_call(self, call);
            }
        }
        use syn::visit::Visit;
        let mut row_start_calls = RowStartCounter(0);
        row_start_calls.visit_block(&row_loop.body);
        if top_level_row_starts != [0] || row_start_calls.0 != 1 {
            return Err(format!(
                "{relative}::{function_name} digest stream `{stream}` must begin every row with exactly one top-level digest_row_start marker"
            ));
        }

        let Some(syn::Stmt::Expr(drop_expression, _)) = statements.get(index + 2) else {
            return Err(format!(
                "{relative}::{function_name} digest stream `{stream}` must be dropped before the next query"
            ));
        };
        if compact_tokens(drop_expression) != format!("drop({stream})") {
            return Err(format!(
                "{relative}::{function_name} digest stream `{stream}` must be explicitly dropped after consumption"
            ));
        }
        witnessed_queries.push(query.clone());
    }

    let expected_queries = specs.iter().map(|spec| spec.sql).collect::<Vec<_>>();
    if witnessed_queries != expected_queries {
        return Err(format!(
            "{relative}::{function_name} must bind, consume, and drop exactly one streaming cursor for every governed digest query"
        ));
    }
    Ok(())
}

fn is_digest_row_start_statement(statement: &syn::Stmt) -> bool {
    let syn::Stmt::Expr(syn::Expr::Call(call), Some(_)) = statement else {
        return false;
    };
    compact_tokens(call) == "digest_row_start(&mutdigest)"
}

fn validate_digest_framing_authority(file: &syn::File, relative: &str) -> Result<(), String> {
    let expectations: &[(&str, &[&str])] = &[
        (
            "digest_section",
            &["digest.update(b\"S\")", "digest_bytes(digest,b'N',name)"],
        ),
        ("digest_row_start", &["digest.update(b\"R\")"]),
        (
            "digest_i64",
            &[
                "digest.update(b\"I\")",
                "digest.update(value.to_be_bytes())",
            ],
        ),
        (
            "digest_bytes",
            &[
                "u64::try_from(value.len())",
                "digest.update([marker])",
                "digest.update(length.to_be_bytes())",
                "digest.update(value)",
            ],
        ),
        ("digest_text", &["digest_bytes(digest,b'T',value)"]),
        (
            "digest_optional_text",
            &[
                "digest.update([b'O',1])",
                "digest_text(digest,value.as_bytes())",
                "digest.update([b'O',0])",
            ],
        ),
        (
            "digest_optional_i64",
            &[
                "digest.update([b'O',1])",
                "digest_i64(digest,value)",
                "digest.update([b'O',0])",
            ],
        ),
        (
            "digest_bool",
            &["digest.update([b'B',ifvalue==0{0}else{1}])"],
        ),
        ("digest_blob_field", &["digest_bytes(digest,b'X',&value)"]),
    ];
    for (name, markers) in expectations {
        let function = compact_tokens(exact_free_function(file, name)?);
        require_ordered_markers(relative, name, &function, markers)?;
    }
    Ok(())
}

fn require_ordered_markers(
    relative: &str,
    authority: &str,
    source: &str,
    markers: &[&str],
) -> Result<(), String> {
    let mut offset = 0_usize;
    for marker in markers {
        let Some(found) = source[offset..].find(marker) else {
            return Err(format!(
                "{relative} {authority} is missing ordered authority witness `{marker}`"
            ));
        };
        offset += found + marker.len();
    }
    Ok(())
}

fn validate_command_reachability(workspace_root: &Path) -> Result<(), String> {
    let contract = rust_source(workspace_root, CONTRACT_COMMAND_SOURCE_RELATIVE)?;
    let main = rust_source(workspace_root, XTASK_MAIN_SOURCE_RELATIVE)?;
    let main = syn::parse_file(&main)
        .map_err(|error| format!("parse {XTASK_MAIN_SOURCE_RELATIVE}: {error}"))?;
    let aggregate = compact_tokens(exact_top_level_function(
        &syn::parse_file(&contract)
            .map_err(|error| format!("parse {CONTRACT_COMMAND_SOURCE_RELATIVE}: {error}"))?,
        "validate_artifact_contracts",
    )?);
    let ordered = [
        "validate_source_maintenance_manifest(workspace_root)?",
        "phase1_publication_artifact::validate_immutable_raw_source_rebuild_predecessor(workspace_root)?",
        "validate_immutable_phase1_publication_artifact_predecessor(workspace_root)?",
        "validate_release_provenance_schema(workspace_root)?",
        "validate_phase1_publication_media_readiness_manifest(workspace_root)?",
        "validate_knowledge_contract_manifest(workspace_root)",
    ];
    require_ordered_markers(
        CONTRACT_COMMAND_SOURCE_RELATIVE,
        "aggregate contract authority",
        &aggregate,
        &ordered,
    )?;

    for (function_name, expected_call) in [
        (
            "validate_contract",
            "contract::validate_artifact_contracts(&root)",
        ),
        (
            "release_preflight_at",
            "contract::validate_artifact_contracts(root)",
        ),
    ] {
        let function = exact_top_level_function(&main, function_name)?;
        let calls = direct_call_tokens(function);
        if calls.iter().filter(|call| *call == expected_call).count() != 1 {
            return Err(format!(
                "{XTASK_MAIN_SOURCE_RELATIVE}::{function_name} must directly reach `{expected_call}` exactly once"
            ));
        }
    }

    let run_contract = exact_top_level_match(&main, "run_contract")?;
    let raw_arm = exact_match_arm(run_contract, "Some(\"raw-source-rebuild-manifest\")")?;
    let expected_raw_arm = syn::parse_str::<syn::Expr>(
        r#"match &args[1..] {
            [] => contract::validate_raw_source_rebuild_manifest(&workspace_root()),
            [flag] if flag == "--write" => {
                contract::write_raw_source_rebuild_manifest(&workspace_root())
            }
            _ => Err(
                "raw-source-rebuild-manifest accepts no arguments or exactly --write".to_string(),
            ),
        }"#,
    )
    .map_err(|error| format!("parse governed raw-source rebuild command arm: {error}"))?;
    if raw_arm.guard.is_some()
        || compact_tokens(raw_arm.body.as_ref()) != compact_tokens(&expected_raw_arm)
    {
        return Err(format!(
            "{XTASK_MAIN_SOURCE_RELATIVE} raw-source rebuild command arm drifted"
        ));
    }

    let run_release = exact_top_level_match(&main, "run_release")?;
    let preflight_arm = exact_match_arm(run_release, "Some(\"preflight\")")?;
    if preflight_arm.guard.is_some()
        || compact_tokens(preflight_arm.body.as_ref()) != "release_preflight()"
    {
        return Err(format!(
            "{XTASK_MAIN_SOURCE_RELATIVE} release preflight command arm drifted"
        ));
    }
    Ok(())
}

fn direct_call_tokens(function: &syn::ItemFn) -> Vec<String> {
    struct Audit(Vec<String>);

    impl<'ast> syn::visit::Visit<'ast> for Audit {
        fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
            self.0.push(compact_tokens(call));
            syn::visit::visit_expr_call(self, call);
        }
    }

    use syn::visit::Visit;
    let mut audit = Audit(Vec::new());
    audit.visit_block(&function.block);
    audit.0
}

fn exact_top_level_match<'a>(
    file: &'a syn::File,
    function_name: &str,
) -> Result<&'a syn::ExprMatch, String> {
    let function = exact_top_level_function(file, function_name)?;
    let [syn::Stmt::Expr(syn::Expr::Match(expression), None)] = function.block.stmts.as_slice()
    else {
        return Err(format!(
            "{XTASK_MAIN_SOURCE_RELATIVE}::{function_name} must contain exactly one top-level match expression"
        ));
    };
    Ok(expression)
}

fn exact_match_arm<'a>(
    expression: &'a syn::ExprMatch,
    pattern: &str,
) -> Result<&'a syn::Arm, String> {
    let matching = expression
        .arms
        .iter()
        .filter(|arm| compact_tokens(&arm.pat) == pattern)
        .collect::<Vec<_>>();
    let [arm] = matching.as_slice() else {
        return Err(format!(
            "{XTASK_MAIN_SOURCE_RELATIVE} must contain exactly one command arm `{pattern}`; found {}",
            matching.len()
        ));
    };
    Ok(arm)
}

fn validate_release_authority(workspace_root: &Path) -> Result<(), String> {
    let release_bytes = read_regular_file(workspace_root, RELEASE_RECORD_RELATIVE)?;
    let release_source = std::str::from_utf8(&release_bytes)
        .map_err(|error| format!("{RELEASE_RECORD_RELATIVE} must be UTF-8: {error}"))?;
    let release: toml::Value = toml::from_str(release_source)
        .map_err(|error| format!("parse {RELEASE_RECORD_RELATIVE}: {error}"))?;
    let changes = release
        .get("changes")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| format!("{RELEASE_RECORD_RELATIVE} must define changes"))?;
    let matching = changes
        .iter()
        .filter(|change| change.get("id").and_then(toml::Value::as_str) == Some(RELEASE_CHANGE_ID))
        .collect::<Vec<_>>();
    let [change] = matching.as_slice() else {
        return Err(format!(
            "{RELEASE_RECORD_RELATIVE} must define exactly one `{RELEASE_CHANGE_ID}` change; found {}",
            matching.len()
        ));
    };
    let impacts = change
        .get("semver_impacts")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| format!("release change `{RELEASE_CHANGE_ID}` has no semver impacts"))?
        .iter()
        .map(|impact| {
            impact.as_str().ok_or_else(|| {
                format!("release change `{RELEASE_CHANGE_ID}` semver impacts must be strings")
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if change.get("classification").and_then(toml::Value::as_str) != Some("breaking")
        || impacts != RELEASE_CHANGE_IMPACTS
        || change.get("summary").and_then(toml::Value::as_str) != Some(RELEASE_CHANGE_SUMMARY)
    {
        return Err(format!(
            "release change `{RELEASE_CHANGE_ID}` must retain its exact breaking classification, semver impacts, and summary"
        ));
    }

    let changelog_bytes = read_regular_file(workspace_root, CHANGELOG_RELATIVE)?;
    let changelog = std::str::from_utf8(&changelog_bytes)
        .map_err(|error| format!("{CHANGELOG_RELATIVE} must be UTF-8: {error}"))?;
    if changelog.matches(CHANGELOG_RELEASE_MARKER).count() != 1 {
        return Err(format!(
            "{CHANGELOG_RELATIVE} must contain exactly one `{CHANGELOG_RELEASE_MARKER}` marker"
        ));
    }
    let current_start = changelog
        .find("## [1.0.0-alpha.1]")
        .ok_or_else(|| format!("{CHANGELOG_RELATIVE} has no current release section"))?;
    let current = &changelog[current_start..];
    let current_end = current["## [1.0.0-alpha.1]".len()..]
        .find("\n## [")
        .map_or(current.len(), |offset| offset + "## [1.0.0-alpha.1]".len());
    let current = &current[..current_end];
    if !current.contains(&format!(
        "{CHANGELOG_RELEASE_MARKER}\n- Event-store schema v4 now exposes a versioned raw-source rebuild operation"
    )) {
        return Err(format!(
            "{CHANGELOG_RELATIVE} current release must bind the raw-source rebuild note to `{RELEASE_CHANGE_ID}`"
        ));
    }
    Ok(())
}

fn validate_result_vector(
    workspace_root: &Path,
    vector: &RawSourceRebuildVector,
) -> Result<(), String> {
    if vector.schema_version != SCHEMA_VERSION || vector.contract_id != CONTRACT_ID {
        return Err(format!(
            "{RESULT_VECTOR_CANONICAL_RELATIVE} has inconsistent identity"
        ));
    }
    if vector.cases.is_empty() {
        return Err(format!(
            "{RESULT_VECTOR_CANONICAL_RELATIVE} must contain executable cases"
        ));
    }
    validate_delegated_suite_inventory(vector)?;
    validate_unique(
        "raw-source rebuild vector case IDs",
        vector.cases.iter().map(|case| case.id.as_str()),
    )?;
    validate_failpoint_result_vector_cases(vector)?;
    let mut direct_count = 0_usize;
    for case in &vector.cases {
        if case.expected_outcome.trim().is_empty() {
            return Err(format!("vector case `{}` has no expected outcome", case.id));
        }
        match case.execution.as_str() {
            "direct_executor" => {
                direct_count += 1;
                if case.authority != RESULT_VECTOR_EXECUTOR_TEST
                    || case.authority_path != RESULT_VECTOR_EXECUTOR_RELATIVE
                    || !RESULT_VECTOR_DIRECT_CASE_IDS.contains(&case.id.as_str())
                {
                    return Err(format!(
                        "direct vector case `{}` must bind the canonical executor",
                        case.id
                    ));
                }
                if case.expected_immutable_raw_digest.is_none()
                    || case.expected_active_product_state_digest.is_none()
                {
                    return Err(format!(
                        "direct vector case `{}` must freeze exact immutable-raw and active-product digest bytes",
                        case.id
                    ));
                }
            }
            "delegated_rust_test" => {}
            other => {
                return Err(format!(
                    "vector case `{}` has unsupported execution mode `{other}`",
                    case.id
                ));
            }
        }
        for digest in [
            case.expected_immutable_raw_digest.as_deref(),
            case.expected_active_product_state_digest.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            validate_vector_expected_digest(digest)?;
        }
    }
    if direct_count != RESULT_VECTOR_DIRECT_CASE_IDS.len() {
        return Err(format!(
            "raw-source rebuild vector requires exactly {} direct executor cases; found {direct_count}",
            RESULT_VECTOR_DIRECT_CASE_IDS.len()
        ));
    }
    for authority in &vector.delegated_suite.authorities {
        validate_executable_test(
            workspace_root,
            &authority.authority_path,
            &authority.authority,
        )?;
    }
    validate_executable_test(
        workspace_root,
        RESULT_VECTOR_EXECUTOR_RELATIVE,
        RESULT_VECTOR_EXECUTOR_TEST,
    )?;
    validate_direct_executor_authority(workspace_root)?;
    validate_delegated_suite_contract_lane(workspace_root)
}

fn validate_failpoint_result_vector_cases(vector: &RawSourceRebuildVector) -> Result<(), String> {
    let expected_ids = REBUILD_FAILPOINTS
        .iter()
        .map(|failpoint| failpoint.rollback_case_id)
        .collect::<BTreeSet<_>>();
    let actual_cases = vector
        .cases
        .iter()
        .filter(|case| {
            case.authority == REBUILD_FAILPOINT_TEST || case.id.starts_with("rollback_after_")
        })
        .collect::<Vec<_>>();
    let actual_ids = actual_cases
        .iter()
        .map(|case| case.id.as_str())
        .collect::<BTreeSet<_>>();
    if actual_ids != expected_ids {
        return Err(format!(
            "raw-source rebuild vector rollback failpoint case IDs drifted: expected {expected_ids:?}, found {actual_ids:?}"
        ));
    }
    for failpoint in REBUILD_FAILPOINTS {
        let matching = actual_cases
            .iter()
            .filter(|case| case.id == failpoint.rollback_case_id)
            .copied()
            .collect::<Vec<_>>();
        let [case] = matching.as_slice() else {
            return Err(format!(
                "raw-source rebuild vector must bind rollback case `{}` exactly once",
                failpoint.rollback_case_id
            ));
        };
        if case.execution != "delegated_rust_test"
            || case.authority != REBUILD_FAILPOINT_TEST
            || case.authority_path != REBUILD_FAILPOINT_TEST_SOURCE_RELATIVE
        {
            return Err(format!(
                "raw-source rebuild rollback case `{}` must bind the governed failpoint test authority",
                failpoint.rollback_case_id
            ));
        }
    }
    Ok(())
}

fn validate_delegated_suite_inventory(vector: &RawSourceRebuildVector) -> Result<(), String> {
    let suite = &vector.delegated_suite;
    if suite.id != RESULT_VECTOR_DELEGATED_SUITE_ID
        || suite.lane != RESULT_VECTOR_DELEGATED_SUITE_LANE
        || suite.package != RESULT_VECTOR_DELEGATED_SUITE_PACKAGE
    {
        return Err(format!(
            "{RESULT_VECTOR_CANONICAL_RELATIVE} delegated suite identity, lane, or package drifted"
        ));
    }
    if suite.authorities.is_empty() {
        return Err(format!(
            "{RESULT_VECTOR_CANONICAL_RELATIVE} delegated suite must contain exact test authorities"
        ));
    }
    let suite_authorities = suite
        .authorities
        .iter()
        .map(|authority| {
            (
                authority.authority_path.as_str(),
                authority.authority.as_str(),
            )
        })
        .collect::<Vec<_>>();
    let suite_authority_keys = suite_authorities
        .iter()
        .map(|(path, authority)| format!("{path}::{authority}"))
        .collect::<Vec<_>>();
    validate_unique(
        "raw-source rebuild delegated suite authorities",
        suite_authority_keys.iter().map(String::as_str),
    )?;
    if suite_authorities
        .iter()
        .any(|(path, authority)| path.trim().is_empty() || authority.trim().is_empty())
    {
        return Err(
            "raw-source rebuild delegated suite authorities must have nonempty paths and names"
                .to_owned(),
        );
    }

    let suite_authorities = suite_authorities.into_iter().collect::<BTreeSet<_>>();
    let delegated_case_authorities = vector
        .cases
        .iter()
        .filter(|case| case.execution == "delegated_rust_test")
        .map(|case| (case.authority_path.as_str(), case.authority.as_str()))
        .collect::<BTreeSet<_>>();
    if suite_authorities != delegated_case_authorities {
        let missing = delegated_case_authorities
            .difference(&suite_authorities)
            .map(|(path, authority)| format!("{path}::{authority}"))
            .collect::<Vec<_>>();
        let unrepresented = suite_authorities
            .difference(&delegated_case_authorities)
            .map(|(path, authority)| format!("{path}::{authority}"))
            .collect::<Vec<_>>();
        return Err(format!(
            "raw-source rebuild delegated suite must exactly cover delegated vector cases; missing {missing:?}; unrepresented {unrepresented:?}"
        ));
    }
    Ok(())
}

fn validate_delegated_suite_contract_lane(workspace_root: &Path) -> Result<(), String> {
    let flake = regular_utf8_source(workspace_root, FLAKE_SOURCE_RELATIVE)?;
    let apps = regular_utf8_source(workspace_root, CONTRACT_APP_SOURCE_RELATIVE)?;
    let common = regular_utf8_source(workspace_root, CONTRACT_LANE_SOURCE_RELATIVE)?;
    let toolchains = regular_utf8_source(workspace_root, TOOLCHAIN_ROUTING_SOURCE_RELATIVE)?;
    validate_delegated_suite_contract_lane_sources(&flake, &apps, &common, &toolchains)?;
    validate_flake_lock_authority(workspace_root)?;
    validate_delegated_suite_test_targets(workspace_root)
}

fn validate_delegated_suite_test_targets(workspace_root: &Path) -> Result<(), String> {
    let cargo_relative = "crates/event_store/Cargo.toml";
    let cargo = regular_utf8_source(workspace_root, cargo_relative)?;
    let cargo: toml::Value =
        toml::from_str(&cargo).map_err(|error| format!("parse {cargo_relative}: {error}"))?;
    let package = cargo
        .get("package")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| format!("{cargo_relative} must define one package"))?;
    if package.get("name").and_then(toml::Value::as_str)
        != Some(RESULT_VECTOR_DELEGATED_SUITE_PACKAGE)
        || package.get("autotests").and_then(toml::Value::as_bool) == Some(false)
        || cargo
            .get("lib")
            .and_then(|lib| lib.get("test"))
            .and_then(toml::Value::as_bool)
            == Some(false)
    {
        return Err(format!(
            "{cargo_relative} must leave the delegated library tests and direct integration executor enabled for `{RESULT_VECTOR_DELEGATED_SUITE_PACKAGE}`"
        ));
    }

    for (relative, module_name, expected_attributes) in [
        (
            "crates/event_store/src/store.rs",
            "raw_source_rebuild_v1_tests",
            &["#[cfg(test)]"][..],
        ),
        (
            "crates/event_store/src/nip09/reconciliation_v1.rs",
            "visibility_oracle_v1",
            &[][..],
        ),
    ] {
        let file = rust_file(workspace_root, relative)?;
        let modules = file
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Mod(module) if module.ident == module_name => Some(module),
                _ => None,
            })
            .collect::<Vec<_>>();
        let [module] = modules.as_slice() else {
            return Err(format!(
                "{relative} must register delegated test module `{module_name}` exactly once; found {}",
                modules.len()
            ));
        };
        let attributes = module.attrs.iter().map(compact_tokens).collect::<Vec<_>>();
        if !matches!(module.vis, syn::Visibility::Inherited)
            || module.content.is_some()
            || attributes != expected_attributes
        {
            return Err(format!(
                "{relative} delegated test module `{module_name}` visibility, source routing, or attributes drifted"
            ));
        }
    }
    Ok(())
}

fn validate_delegated_suite_contract_lane_sources(
    flake: &str,
    apps: &str,
    common: &str,
    toolchains: &str,
) -> Result<(), String> {
    let flake_toolchains =
        nix_code_occurrences(flake, "toolchains = import ./build/nix/toolchains.nix {");
    let all_flake_toolchains = nix_code_occurrences(flake, "toolchains =");
    if flake_toolchains.len() != 1 || all_flake_toolchains != flake_toolchains {
        return Err(format!(
            "{FLAKE_SOURCE_RELATIVE} must bind exactly one toolchain authority from ./build/nix/toolchains.nix"
        ));
    }
    let flake_common = nix_code_occurrences(flake, "common = import ./build/nix/common.nix {");
    let all_flake_common = nix_code_occurrences(flake, "common =");
    if flake_common.len() != 1 || all_flake_common != flake_common {
        return Err(format!(
            "{FLAKE_SOURCE_RELATIVE} must bind exactly one common authority from ./build/nix/common.nix"
        ));
    }
    let flake_apps = nix_code_occurrences(flake, "apps = import ./build/nix/apps.nix {");
    let all_flake_apps = nix_code_occurrences(flake, "apps =");
    if flake_apps.len() != 1 || all_flake_apps != flake_apps {
        return Err(format!(
            "{FLAKE_SOURCE_RELATIVE} must export exactly one per-system apps authority from ./build/nix/apps.nix"
        ));
    }
    let per_system = nix_code_occurrences(flake, "perSystem =");
    if per_system.len() != 1
        || per_system[0] >= flake_toolchains[0]
        || flake_toolchains[0] >= flake_common[0]
        || flake_common[0] >= flake_apps[0]
    {
        return Err(format!(
            "{FLAKE_SOURCE_RELATIVE} must bind governed toolchains, common, and apps imports in order through perSystem"
        ));
    }
    for (label, assignment_start, source, expected) in [
        (
            "toolchains",
            flake_toolchains[0],
            nix_balanced_slice(flake, flake_toolchains[0], b'{', b'}')?,
            "{inheritpkgs;}",
        ),
        (
            "common",
            flake_common[0],
            nix_balanced_slice(flake, flake_common[0], b'{', b'}')?,
            "{crane=inputs.crane;inheritlibpkgstoolchains;}",
        ),
        (
            "apps",
            flake_apps[0],
            nix_balanced_slice(flake, flake_apps[0], b'{', b'}')?,
            "{inheritcommonconfiglibpkgstoolchains;}",
        ),
    ] {
        let actual = source.split_whitespace().collect::<String>();
        if actual != expected {
            return Err(format!(
                "{FLAKE_SOURCE_RELATIVE} `{label}` import arguments drifted from the exact delegated contract-lane authority"
            ));
        }
        validate_nix_attrset_assignment_terminator(flake, assignment_start).map_err(|error| {
            format!(
                "{FLAKE_SOURCE_RELATIVE} `{label}` import must end immediately after its governed arguments: {error}"
            )
        })?;
    }

    let stable_assignments = nix_code_occurrences(toolchains, "stable =");
    let stable_authority = nix_code_occurrences(
        toolchains,
        "stable = pkgs.rust-bin.fromRustupToolchainFile ../../rust-toolchain.toml;",
    );
    if stable_assignments.len() != 1 || stable_authority != stable_assignments {
        return Err(format!(
            "{TOOLCHAIN_ROUTING_SOURCE_RELATIVE} must route the stable toolchain exactly through ../../{RUST_TOOLCHAIN_RELATIVE}"
        ));
    }

    let cargo_source_assignments =
        nix_code_occurrences(common, "cargoSource = lib.fileset.toSource {");
    if cargo_source_assignments.len() != 1 {
        return Err(format!(
            "{CONTRACT_LANE_SOURCE_RELATIVE} must define exactly one cargoSource fileset"
        ));
    }
    let cargo_source = nix_balanced_slice(common, cargo_source_assignments[0], b'{', b'}')?;
    for required in [
        "../../Cargo.toml",
        "../../Cargo.lock",
        "../../flake.nix",
        "../../flake.lock",
        "../../build/nix/apps.nix",
        "../../build/nix/common.nix",
        "../../build/nix/toolchains.nix",
        "../../rust-toolchain.toml",
        "../../tools",
    ] {
        if nix_code_occurrences(cargo_source, required).len() != 1 {
            return Err(format!(
                "{CONTRACT_LANE_SOURCE_RELATIVE} cargoSource must include governed input `{required}` exactly once"
            ));
        }
    }

    let contract_apps = nix_code_occurrences(apps, "contract = mkRepoApp {");
    if contract_apps.len() != 1 {
        return Err(format!(
            "{CONTRACT_APP_SOURCE_RELATIVE} must define exactly one executable contract app"
        ));
    }
    let contract_app = nix_balanced_slice(apps, contract_apps[0], b'{', b'}')?;
    let contract_app_authority = contract_app.split_whitespace().collect::<String>();
    let expected_contract_app_authority = concat!(
        "{",
        "name=\"contract\";",
        "description=\"Runthecore-librarycontractlane\";",
        "runtimeInputs=common.runtimeInputs.stable;",
        "command=common.contractCommand;",
        "}"
    );
    if contract_app_authority != expected_contract_app_authority {
        return Err(format!(
            "{CONTRACT_APP_SOURCE_RELATIVE} contract app must bind the exact stable runtime, default path, environment, and command authority"
        ));
    }

    let crate_list_assignments = nix_code_occurrences(common, "coreContractCrates = [");
    if crate_list_assignments.len() != 1 {
        return Err(format!(
            "{CONTRACT_LANE_SOURCE_RELATIVE} must define one literal coreContractCrates list"
        ));
    }
    let crate_list = nix_balanced_slice(common, crate_list_assignments[0], b'[', b']')?;
    let crates = nix_literal_string_array(crate_list)?;
    validate_unique(
        "Nix core contract crates",
        crates.iter().map(String::as_str),
    )?;
    if crates
        .iter()
        .filter(|package| package.as_str() == RESULT_VECTOR_DELEGATED_SUITE_PACKAGE)
        .count()
        != 1
    {
        return Err(format!(
            "{CONTRACT_LANE_SOURCE_RELATIVE} coreContractCrates must contain `{RESULT_VECTOR_DELEGATED_SUITE_PACKAGE}` exactly once"
        ));
    }

    let cargo_args = nix_assignment_through_semicolon(common, "coreContractCargoArgs")?;
    let cargo_arg_lines = cargo_args
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    if cargo_arg_lines.len() != 3
        || cargo_arg_lines[0] != "coreContractCargoArgs ="
        || cargo_arg_lines[1]
            != "lib.concatStringsSep \" \" (map (crate: \"-p ${crate}\") coreContractCrates)"
        || cargo_arg_lines[2]
            != "+ \" --features radroots_blossom/raster-decode,radroots_event_codec/serde_json,radroots_event_codec/nostr,radroots_nostr/blossom,radroots_nostr/client,radroots_nostr/codec,radroots_nostr/events\";"
    {
        return Err(format!(
            "{CONTRACT_LANE_SOURCE_RELATIVE} coreContractCargoArgs must map every literal core contract crate to an unfiltered `-p` package selection"
        ));
    }

    let contract_command = nix_indented_string_assignment(common, "contractCommand")?;
    let contract_commands = contract_command
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    let expected_commands = [
        "cargo run -q -p xtask -- hygiene forbidden-identifiers",
        "cargo check -q ${coreContractCargoArgs}",
        "cargo test -q ${coreContractCargoArgs}",
        "cargo run -q -p xtask -- contract validate",
    ];
    if contract_commands != expected_commands {
        return Err(format!(
            "{CONTRACT_LANE_SOURCE_RELATIVE} contractCommand must run the exact unfiltered core package test lane before contract validation"
        ));
    }
    Ok(())
}

fn validate_flake_lock_authority(workspace_root: &Path) -> Result<(), String> {
    let source = regular_utf8_source(workspace_root, FLAKE_LOCK_RELATIVE)?;
    let lock: Value = serde_json::from_str(&source)
        .map_err(|error| format!("parse {FLAKE_LOCK_RELATIVE}: {error}"))?;
    if lock.get("version").and_then(Value::as_u64) != Some(7)
        || lock.get("root").and_then(Value::as_str) != Some("root")
    {
        return Err(format!(
            "{FLAKE_LOCK_RELATIVE} must remain a version-7 lock with the root node named `root`"
        ));
    }
    let nodes = lock
        .get("nodes")
        .and_then(Value::as_object)
        .ok_or_else(|| format!("{FLAKE_LOCK_RELATIVE} must define a nodes object"))?;
    let root_inputs = nodes
        .get("root")
        .and_then(|root| root.get("inputs"))
        .and_then(Value::as_object)
        .ok_or_else(|| format!("{FLAKE_LOCK_RELATIVE} root node must define direct inputs"))?;
    let expected_inputs = [
        ("crane", "crane"),
        ("flake-parts", "flake-parts"),
        ("nixpkgs", "nixpkgs"),
        ("rust-overlay", "rust-overlay"),
        ("treefmt-nix", "treefmt-nix"),
    ]
    .into_iter()
    .map(|(name, node)| (name.to_owned(), Value::String(node.to_owned())))
    .collect::<serde_json::Map<_, _>>();
    if root_inputs != &expected_inputs {
        return Err(format!(
            "{FLAKE_LOCK_RELATIVE} root inputs must exactly lock crane, flake-parts, nixpkgs, rust-overlay, and treefmt-nix"
        ));
    }
    for node in expected_inputs.values().filter_map(Value::as_str) {
        let locked = nodes
            .get(node)
            .and_then(|node| node.get("locked"))
            .and_then(Value::as_object)
            .ok_or_else(|| {
                format!("{FLAKE_LOCK_RELATIVE} direct input node `{node}` must be locked")
            })?;
        for field in ["narHash", "rev"] {
            if locked
                .get(field)
                .and_then(Value::as_str)
                .is_none_or(str::is_empty)
            {
                return Err(format!(
                    "{FLAKE_LOCK_RELATIVE} direct input node `{node}` must bind nonempty `{field}`"
                ));
            }
        }
    }
    Ok(())
}

fn regular_utf8_source(workspace_root: &Path, relative: &str) -> Result<String, String> {
    let bytes = read_regular_file(workspace_root, relative)?;
    String::from_utf8(bytes).map_err(|error| format!("{relative} must be UTF-8: {error}"))
}

fn nix_code_mask(source: &str) -> Vec<bool> {
    #[derive(Clone, Copy)]
    enum State {
        Code,
        LineComment,
        BlockComment,
        DoubleString,
        IndentedString,
    }

    let bytes = source.as_bytes();
    let mut mask = vec![false; bytes.len()];
    let mut state = State::Code;
    let mut index = 0_usize;
    while index < bytes.len() {
        match state {
            State::Code => {
                if bytes[index] == b'#' {
                    state = State::LineComment;
                    index += 1;
                } else if bytes[index..].starts_with(b"/*") {
                    state = State::BlockComment;
                    index += 2;
                } else if bytes[index] == b'"' {
                    mask[index] = true;
                    state = State::DoubleString;
                    index += 1;
                } else if bytes[index..].starts_with(b"''") {
                    mask[index] = true;
                    state = State::IndentedString;
                    index += 2;
                } else {
                    mask[index] = true;
                    index += 1;
                }
            }
            State::LineComment => {
                if bytes[index] == b'\n' {
                    state = State::Code;
                } else {
                    index += 1;
                }
            }
            State::BlockComment => {
                if bytes[index..].starts_with(b"*/") {
                    state = State::Code;
                    index += 2;
                } else {
                    index += 1;
                }
            }
            State::DoubleString => {
                if bytes[index] == b'\\' {
                    index = (index + 2).min(bytes.len());
                } else if bytes[index] == b'"' {
                    state = State::Code;
                    index += 1;
                } else {
                    index += 1;
                }
            }
            State::IndentedString => {
                if bytes[index..].starts_with(b"''") {
                    match bytes.get(index + 2) {
                        Some(b'$' | b'\'' | b'\\') => index += 3,
                        _ => {
                            state = State::Code;
                            index += 2;
                        }
                    }
                } else {
                    index += 1;
                }
            }
        }
    }
    mask
}

fn nix_code_occurrences(source: &str, needle: &str) -> Vec<usize> {
    let mask = nix_code_mask(source);
    source
        .match_indices(needle)
        .filter_map(|(index, _)| mask.get(index).copied().unwrap_or(false).then_some(index))
        .collect()
}

fn nix_balanced_slice(
    source: &str,
    search_start: usize,
    open: u8,
    close: u8,
) -> Result<&str, String> {
    let mask = nix_code_mask(source);
    let bytes = source.as_bytes();
    let start = (search_start..bytes.len())
        .find(|index| mask[*index] && bytes[*index] == open)
        .ok_or_else(|| {
            format!(
                "governed Nix authority has no `{}` opener",
                char::from(open)
            )
        })?;
    let mut depth = 0_usize;
    for index in start..bytes.len() {
        if !mask[index] {
            continue;
        }
        if bytes[index] == open {
            depth += 1;
        } else if bytes[index] == close {
            depth = depth
                .checked_sub(1)
                .ok_or_else(|| "governed Nix authority has unbalanced delimiters".to_owned())?;
            if depth == 0 {
                return Ok(&source[start..=index]);
            }
        }
    }
    Err("governed Nix authority has an unterminated delimiter".to_owned())
}

fn validate_nix_attrset_assignment_terminator(
    source: &str,
    assignment_start: usize,
) -> Result<(), String> {
    let attrset = nix_balanced_slice(source, assignment_start, b'{', b'}')?;
    let attrset_start = attrset.as_ptr() as usize - source.as_ptr() as usize;
    let after_attrset = &source[attrset_start + attrset.len()..];
    let first = after_attrset
        .bytes()
        .find(|byte| !byte.is_ascii_whitespace());
    if first != Some(b';') {
        return Err("postfix merge or expression detected before assignment terminator".to_owned());
    }
    Ok(())
}

fn nix_literal_string_array(source: &str) -> Result<Vec<String>, String> {
    let mask = nix_code_mask(source);
    let bytes = source.as_bytes();
    let mut values = Vec::new();
    let mut index = 0_usize;
    while index < bytes.len() {
        if !mask[index] {
            index += 1;
            continue;
        }
        match bytes[index] {
            b'[' | b']' | b' ' | b'\t' | b'\r' | b'\n' => index += 1,
            b'"' => {
                let start = index + 1;
                index = start;
                while index < bytes.len() && bytes[index] != b'"' {
                    if bytes[index] == b'\\' || bytes[index..].starts_with(b"${") {
                        return Err(
                            "governed Nix package list must use plain literal strings".to_owned()
                        );
                    }
                    index += 1;
                }
                if index == bytes.len() {
                    return Err("governed Nix package list has an unterminated string".to_owned());
                }
                values.push(source[start..index].to_owned());
                index += 1;
            }
            _ => {
                return Err(
                    "governed Nix package list must contain only literal strings".to_owned(),
                );
            }
        }
    }
    Ok(values)
}

fn nix_assignment_through_semicolon<'a>(source: &'a str, name: &str) -> Result<&'a str, String> {
    let needle = format!("{name} =");
    let starts = nix_code_occurrences(source, &needle);
    let [start] = starts.as_slice() else {
        return Err(format!(
            "governed Nix source must define `{name}` exactly once; found {}",
            starts.len()
        ));
    };
    let mask = nix_code_mask(source);
    let end = (*start..source.len())
        .find(|index| mask[*index] && source.as_bytes()[*index] == b';')
        .ok_or_else(|| format!("governed Nix assignment `{name}` has no terminator"))?;
    Ok(&source[*start..=end])
}

fn nix_indented_string_assignment<'a>(source: &'a str, name: &str) -> Result<&'a str, String> {
    let needle = format!("{name} = ''");
    let starts = nix_code_occurrences(source, &needle);
    let [start] = starts.as_slice() else {
        return Err(format!(
            "governed Nix source must define indented string `{name}` exactly once; found {}",
            starts.len()
        ));
    };
    let content_start = *start + needle.len();
    let bytes = source.as_bytes();
    let mut index = content_start;
    while index < bytes.len() {
        if bytes[index..].starts_with(b"''") {
            match bytes.get(index + 2) {
                Some(b'$' | b'\'' | b'\\') => index += 3,
                _ => {
                    let content = &source[content_start..index];
                    index += 2;
                    while bytes.get(index).is_some_and(u8::is_ascii_whitespace) {
                        index += 1;
                    }
                    if bytes.get(index) != Some(&b';') {
                        return Err(format!(
                            "governed Nix indented string `{name}` must end with one semicolon"
                        ));
                    }
                    return Ok(content);
                }
            }
        } else {
            index += 1;
        }
    }
    Err(format!(
        "governed Nix indented string `{name}` is unterminated"
    ))
}

fn validate_result_vector_identity(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() != RESULT_VECTOR_BYTE_LENGTH || sha256_hex(bytes) != RESULT_VECTOR_SHA256 {
        return Err(format!(
            "{RESULT_VECTOR_CANONICAL_RELATIVE} does not match the immutable executable case inventory"
        ));
    }
    Ok(())
}

fn validate_direct_executor_authority(workspace_root: &Path) -> Result<(), String> {
    let file = rust_file(workspace_root, RESULT_VECTOR_EXECUTOR_RELATIVE)?;
    let full_source = compact_tokens(&file);
    if !full_source.contains("include_bytes!(\"fixtures/food_availability_projection.v1.json\")") {
        return Err(
            "direct raw-source rebuild vector executor must byte-bind the signed Food fixture"
                .to_owned(),
        );
    }
    let function = compact_tokens(exact_free_function(&file, RESULT_VECTOR_EXECUTOR_TEST)?);
    for marker in [
        "decode_digest(",
        "expected_immutable_raw_digest.as_deref().expect(",
        "expected_active_product_state_digest.as_deref().expect(",
        "first.immutable_raw_digest().as_bytes(),&expected_immutable_raw_digest",
        "first.active_product_state_digest().as_bytes(),&expected_active_product_state_digest",
        "signed_food_fixture_ingest()",
        "food_first.immutable_raw_digest().as_bytes(),&expected_food_raw_digest",
        "food_first.active_product_state_digest().as_bytes(),&expected_food_product_digest",
        "food_second.immutable_raw_digest(),food_first.immutable_raw_digest()",
        "food_second.active_product_state_digest(),food_first.active_product_state_digest()",
    ] {
        if !function.contains(marker) {
            return Err(format!(
                "direct raw-source rebuild vector executor is missing exact digest authority `{marker}`"
            ));
        }
    }
    for case_id in RESULT_VECTOR_DIRECT_CASE_IDS {
        if !function.contains(case_id) {
            return Err(format!(
                "direct raw-source rebuild vector executor does not execute case `{case_id}`"
            ));
        }
    }
    Ok(())
}

fn validate_executable_test(
    workspace_root: &Path,
    relative: &str,
    name: &str,
) -> Result<(), String> {
    let file = rust_file(workspace_root, relative)?;
    #[derive(Clone)]
    struct ModuleContext {
        name: String,
        attributes: Vec<String>,
        private: bool,
    }
    struct Match<'a> {
        function: &'a syn::ItemFn,
        modules: Vec<ModuleContext>,
    }
    fn collect<'a>(
        items: &'a [Item],
        name: &str,
        modules: &mut Vec<ModuleContext>,
        matches: &mut Vec<Match<'a>>,
    ) {
        for item in items {
            match item {
                Item::Fn(function) if function.sig.ident == name => matches.push(Match {
                    function,
                    modules: modules.clone(),
                }),
                Item::Mod(module) => {
                    if let Some((_, items)) = &module.content {
                        modules.push(ModuleContext {
                            name: module.ident.to_string(),
                            attributes: module.attrs.iter().map(compact_tokens).collect(),
                            private: matches!(module.vis, syn::Visibility::Inherited),
                        });
                        collect(items, name, modules, matches);
                        modules.pop();
                    }
                }
                _ => {}
            }
        }
    }
    let mut matches = Vec::new();
    collect(&file.items, name, &mut Vec::new(), &mut matches);
    let [matched] = matches.as_slice() else {
        return Err(format!(
            "executable raw-source rebuild authority {relative}::{name} must exist exactly once; found {}",
            matches.len()
        ));
    };
    let expected_test_module = (relative
        == "crates/event_store/src/nip09/reconciliation_v1/visibility_oracle_v1.rs")
        .then_some("tests");
    match (expected_test_module, matched.modules.as_slice()) {
        (None, []) => {}
        (Some(expected), [module])
            if module.name == expected
                && module.private
                && module.attributes == ["#[cfg(test)]"] => {}
        _ => {
            let actual = matched
                .modules
                .iter()
                .map(|module| {
                    format!(
                        "{}:{:?}:private={}",
                        module.name, module.attributes, module.private
                    )
                })
                .collect::<Vec<_>>();
            return Err(format!(
                "executable raw-source rebuild authority {relative}::{name} has unsupported module ancestry {actual:?}"
            ));
        }
    }
    let function = matched.function;
    let attrs = function
        .attrs
        .iter()
        .map(compact_tokens)
        .collect::<Vec<_>>();
    if attrs.as_slice() != ["#[test]"] && attrs.as_slice() != ["#[tokio::test]"] {
        return Err(format!(
            "executable raw-source rebuild authority {relative}::{name} must have exactly one unconditional test attribute"
        ));
    }
    struct ReturnCounter(usize);
    impl<'ast> syn::visit::Visit<'ast> for ReturnCounter {
        fn visit_expr_return(&mut self, expression: &'ast syn::ExprReturn) {
            self.0 += 1;
            syn::visit::visit_expr_return(self, expression);
        }
    }
    use syn::visit::Visit;
    let mut returns = ReturnCounter(0);
    returns.visit_block(&function.block);
    if returns.0 != 0 {
        return Err(format!(
            "executable raw-source rebuild authority {relative}::{name} must not contain early return control flow"
        ));
    }
    Ok(())
}

fn generated_descriptor(
    manifest: &RawSourceRebuildManifest,
    manifest_bytes: &[u8],
    manifest_sha256: &str,
) -> String {
    let manifest_json = std::str::from_utf8(manifest_bytes).expect("canonical manifest UTF-8");
    format!(
        "// @generated by `{WRITE_COMMAND}`; do not edit.\n\
#![allow(dead_code)]\n\
\n\
pub(crate) const RAW_SOURCE_REBUILD_MANIFEST_JSON: &str = {manifest_json:?};\n\
pub(crate) const RAW_SOURCE_REBUILD_MANIFEST_BYTE_LENGTH: usize = {};\n\
pub(crate) const RAW_SOURCE_REBUILD_MANIFEST_SHA256: &str =\n    \"{manifest_sha256}\";\n\
pub(crate) const RAW_SOURCE_REBUILD_CONTRACT_ID: &str =\n    \"{CONTRACT_ID}\";\n\
pub(crate) const RAW_SOURCE_REBUILD_AUTHORITY_ID: &str = \"{AUTHORITY_ID}\";\n\
pub(crate) const RAW_SOURCE_REBUILD_PREDECESSOR_MANIFEST_SHA256: &str =\n    \"{PREDECESSOR_MANIFEST_SHA256}\";\n\
pub(crate) const RAW_SOURCE_REBUILD_EVENT_STORE_SCHEMA_VERSION: u32 = {EVENT_STORE_SCHEMA_VERSION};\n\
pub(crate) const RAW_SOURCE_REBUILD_EVENT_CONTRACT_REGISTRY_VERSION: u32 = {EVENT_CONTRACT_REGISTRY_VERSION};\n\
pub(crate) const RAW_SOURCE_REBUILD_RESULT_VECTOR_SHA256: &str =\n    \"{}\";\n\
pub(crate) const RAW_SOURCE_REBUILD_RESULT_VECTOR_EXECUTOR_SHA256: &str =\n    \"{}\";\n",
        manifest_bytes.len(),
        manifest.result_vector.sha256,
        manifest.result_vector.executor_sha256,
    )
}

fn manifest_schema() -> Value {
    let path_pattern = "^[A-Za-z0-9_-][A-Za-z0-9._-]*(?:/[A-Za-z0-9_-][A-Za-z0-9._-]*)*$";
    let file = json!({
        "type": "object",
        "required": ["path", "byte_length", "sha256", "hash_algorithm"],
        "properties": {
            "path": {"type": "string", "pattern": path_pattern},
            "byte_length": {"type": "integer", "minimum": 1},
            "sha256": {"type": "string", "pattern": "^[0-9a-f]{64}$"},
            "hash_algorithm": {"const": HASH_ALGORITHM}
        },
        "additionalProperties": false
    });
    let string_array = json!({
        "type": "array",
        "items": {"type": "string", "minLength": 1},
        "uniqueItems": true
    });
    let digest_field = json!({
        "type": "object",
        "required": ["name", "framing"],
        "properties": {
            "name": {"type": "string", "minLength": 1},
            "framing": {"enum": ["i64", "boolean", "optional_i64", "text", "optional_text", "blob"]}
        },
        "additionalProperties": false
    });
    let digest_query = json!({
        "type": "object",
        "required": ["section", "sql", "fields"],
        "properties": {
            "section": {"type": "string", "minLength": 1},
            "sql": {"type": "string", "minLength": 1},
            "fields": {"type": "array", "minItems": 1, "items": digest_field}
        },
        "additionalProperties": false
    });
    let digest_framing = json!({
        "type": "object",
        "required": ["section", "row", "signed_i64", "boolean", "optional", "text", "blob"],
        "properties": {
            "section": {"const": "S_then_N_then_u64be_length_then_utf8_name"},
            "row": {"const": "R"},
            "signed_i64": {"const": "I_then_i64be"},
            "boolean": {"const": "B_then_u8_0_or_1"},
            "optional": {"const": "O_then_presence_u8_then_nested_value_when_present"},
            "text": {"const": "T_then_u64be_length_then_utf8_bytes"},
            "blob": {"const": "X_then_u64be_length_then_bytes"}
        },
        "additionalProperties": false
    });
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://radroots.org/contracts/event-store/raw-source-rebuild-v1-manifest.schema.json",
        "title": "Radroots event-store raw-source rebuild v1 manifest",
        "type": "object",
        "required": [
            "schema_version", "contract_id", "authority_id", "manifest_schema", "predecessor",
            "migration_inventory", "runtime", "entry_points", "source_files", "public_api",
            "result_vector"
        ],
        "properties": {
            "schema_version": {"const": SCHEMA_VERSION},
            "contract_id": {"const": CONTRACT_ID},
            "authority_id": {"const": AUTHORITY_ID},
            "manifest_schema": file.clone(),
            "predecessor": {
                "type": "object",
                "required": ["contract_id", "manifest"],
                "properties": {
                    "contract_id": {"const": PREDECESSOR_CONTRACT_ID},
                    "manifest": file.clone()
                },
                "additionalProperties": false
            },
            "migration_inventory": {
                "type": "array", "minItems": 8, "maxItems": 8, "items": file.clone()
            },
            "runtime": {
                "type": "object",
                "required": [
                    "event_store_schema_version", "event_contract_registry_version",
                    "transaction_mode", "projection_cursor_count_limit",
                    "projection_cursor_rejection_probe_limit", "caller_main_table_count_limit",
                    "caller_foreign_key_row_count_limit", "caller_inbound_foreign_key_policy",
                    "caller_inbound_foreign_key_parent_tables",
                    "cold_repair_mode",
                    "immutable_raw_digest", "active_product_state_digest",
                    "visibility_oracle", "scoped_integrity_mode", "scoped_integrity_tables", "sqlite_sequence_scope", "stages", "failpoints",
                    "preserved_authorities"
                ],
                "properties": {
                    "event_store_schema_version": {"const": EVENT_STORE_SCHEMA_VERSION},
                    "event_contract_registry_version": {"const": EVENT_CONTRACT_REGISTRY_VERSION},
                    "transaction_mode": {"const": TRANSACTION_MODE},
                    "projection_cursor_count_limit": {"const": PROJECTION_CURSOR_COUNT_LIMIT},
                    "projection_cursor_rejection_probe_limit": {"const": PROJECTION_CURSOR_REJECTION_PROBE_LIMIT},
                    "caller_main_table_count_limit": {"const": CALLER_MAIN_TABLE_COUNT_LIMIT},
                    "caller_foreign_key_row_count_limit": {"const": CALLER_FOREIGN_KEY_ROW_COUNT_LIMIT},
                    "caller_inbound_foreign_key_policy": {"const": CALLER_INBOUND_FOREIGN_KEY_POLICY},
                    "caller_inbound_foreign_key_parent_tables": string_array.clone(),
                    "cold_repair_mode": {"const": COLD_REPAIR_MODE},
                    "immutable_raw_digest": {
                        "type": "object",
                        "required": ["algorithm", "domain_utf8", "domain_terminator", "framing", "output_bytes", "source_queries"],
                        "properties": {
                            "algorithm": {"const": DIGEST_ALGORITHM},
                            "domain_utf8": {"const": RAW_DIGEST_DOMAIN_UTF8},
                            "domain_terminator": {"const": DIGEST_DOMAIN_TERMINATOR},
                            "framing": digest_framing.clone(),
                            "output_bytes": {"const": 32},
                            "source_queries": {"type": "array", "minItems": 2, "maxItems": 2, "items": digest_query.clone()}
                        },
                        "additionalProperties": false
                    },
                    "active_product_state_digest": {
                        "type": "object",
                        "required": ["algorithm", "domain_utf8", "domain_terminator", "framing", "output_bytes", "components", "exclusions", "component_queries"],
                        "properties": {
                            "algorithm": {"const": DIGEST_ALGORITHM},
                            "domain_utf8": {"const": PRODUCT_DIGEST_DOMAIN_UTF8},
                            "domain_terminator": {"const": DIGEST_DOMAIN_TERMINATOR},
                            "framing": digest_framing,
                            "output_bytes": {"const": 32},
                            "components": string_array.clone(),
                            "exclusions": string_array.clone(),
                            "component_queries": {"type": "array", "minItems": 13, "maxItems": 13, "items": digest_query.clone()}
                        },
                        "additionalProperties": false
                    },
                    "visibility_oracle": {"const": VISIBILITY_ORACLE},
                    "scoped_integrity_mode": {"const": SCOPED_INTEGRITY_MODE},
                    "scoped_integrity_tables": string_array.clone(),
                    "sqlite_sequence_scope": {"const": SQLITE_SEQUENCE_SCOPE},
                    "stages": string_array.clone(),
                    "failpoints": string_array.clone(),
                    "preserved_authorities": string_array.clone()
                },
                "additionalProperties": false
            },
            "entry_points": {
                "type": "array",
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
            "source_files": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["role", "path", "byte_length", "sha256", "hash_algorithm"],
                    "properties": {
                        "role": {"type": "string", "minLength": 1},
                        "path": {"type": "string", "pattern": path_pattern},
                        "byte_length": {"type": "integer", "minimum": 1},
                        "sha256": {"type": "string", "pattern": "^[0-9a-f]{64}$"},
                        "hash_algorithm": {"const": HASH_ALGORITHM}
                    },
                    "additionalProperties": false
                }
            },
            "public_api": {
                "type": "object",
                "required": ["added_symbols", "methods", "error_variants", "drift_kinds"],
                "properties": {
                    "added_symbols": string_array.clone(),
                    "methods": string_array.clone(),
                    "error_variants": string_array.clone(),
                    "drift_kinds": {
                        "type": "array",
                        "minItems": 6,
                        "maxItems": 6,
                        "items": {
                            "type": "object",
                            "required": ["variant", "code"],
                            "properties": {
                                "variant": {"type": "string", "minLength": 1},
                                "code": {"type": "string", "pattern": "^[a-z][a-z0-9_]*$"}
                            },
                            "additionalProperties": false
                        }
                    }
                },
                "additionalProperties": false
            },
            "result_vector": {
                "type": "object",
                "required": [
                    "canonical_path", "mirror_path", "byte_length", "sha256", "hash_algorithm",
                    "executor_id", "executor_path", "executor_test", "executor_byte_length",
                    "executor_sha256", "executor_hash_algorithm"
                ],
                "properties": {
                    "canonical_path": {"type": "string", "pattern": path_pattern},
                    "mirror_path": {"type": "string", "pattern": path_pattern},
                    "byte_length": {"type": "integer", "minimum": 1},
                    "sha256": {"type": "string", "pattern": "^[0-9a-f]{64}$"},
                    "hash_algorithm": {"const": HASH_ALGORITHM},
                    "executor_id": {"type": "string", "minLength": 1},
                    "executor_path": {"type": "string", "pattern": path_pattern},
                    "executor_test": {"type": "string", "minLength": 1},
                    "executor_byte_length": {"type": "integer", "minimum": 1},
                    "executor_sha256": {"type": "string", "pattern": "^[0-9a-f]{64}$"},
                    "executor_hash_algorithm": {"const": HASH_ALGORITHM}
                },
                "additionalProperties": false
            }
        },
        "additionalProperties": false
    })
}

#[derive(Clone, Debug)]
struct PublicUseRoute {
    segments: Vec<String>,
    exported_name: String,
    renamed: bool,
    glob: bool,
    absolute: bool,
    attributes: Vec<String>,
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
        flatten_public_use_tree(
            &item_use.tree,
            &mut Vec::new(),
            item_use.leading_colon.is_some(),
            &attributes,
            &mut routes,
        );
    }
    routes
}

fn flatten_public_use_tree(
    tree: &UseTree,
    prefix: &mut Vec<String>,
    absolute: bool,
    attributes: &[String],
    routes: &mut Vec<PublicUseRoute>,
) {
    match tree {
        UseTree::Path(path) => {
            prefix.push(path.ident.to_string());
            flatten_public_use_tree(&path.tree, prefix, absolute, attributes, routes);
            prefix.pop();
        }
        UseTree::Name(name) => {
            let mut segments = prefix.clone();
            segments.push(name.ident.to_string());
            routes.push(PublicUseRoute {
                exported_name: name.ident.to_string(),
                segments,
                renamed: false,
                glob: false,
                absolute,
                attributes: attributes.to_vec(),
            });
        }
        UseTree::Rename(rename) => {
            let mut segments = prefix.clone();
            segments.push(rename.ident.to_string());
            routes.push(PublicUseRoute {
                exported_name: rename.rename.to_string(),
                segments,
                renamed: true,
                glob: false,
                absolute,
                attributes: attributes.to_vec(),
            });
        }
        UseTree::Glob(_) => routes.push(PublicUseRoute {
            exported_name: "*".to_owned(),
            segments: prefix.clone(),
            renamed: false,
            glob: true,
            absolute,
            attributes: attributes.to_vec(),
        }),
        UseTree::Group(group) => {
            for item in &group.items {
                flatten_public_use_tree(item, prefix, absolute, attributes, routes);
            }
        }
    }
}

fn exact_struct<'a>(file: &'a syn::File, name: &str) -> Result<&'a syn::ItemStruct, String> {
    let matches = file
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Struct(item) if item.ident == name => Some(item),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [item] = matches.as_slice() else {
        return Err(format!(
            "governed Rust source must define struct `{name}` exactly once; found {}",
            matches.len()
        ));
    };
    Ok(item)
}

fn exact_enum<'a>(file: &'a syn::File, name: &str) -> Result<&'a syn::ItemEnum, String> {
    let matches = file
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Enum(item) if item.ident == name => Some(item),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [item] = matches.as_slice() else {
        return Err(format!(
            "governed Rust source must define enum `{name}` exactly once; found {}",
            matches.len()
        ));
    };
    Ok(item)
}

fn exact_byte_string_const(file: &syn::File, name: &str) -> Result<Vec<u8>, String> {
    let matches = file
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Const(item) if item.ident == name => Some(item),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [item] = matches.as_slice() else {
        return Err(format!(
            "governed Rust source must define byte-string const `{name}` exactly once; found {}",
            matches.len()
        ));
    };
    let syn::Expr::Lit(syn::ExprLit {
        lit: syn::Lit::ByteStr(value),
        ..
    }) = item.expr.as_ref()
    else {
        return Err(format!("`{name}` must be one literal byte string"));
    };
    Ok(value.value())
}

fn exact_string_const(file: &syn::File, name: &str) -> Result<String, String> {
    let matches = file
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Const(item) if item.ident == name => Some(item),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [item] = matches.as_slice() else {
        return Err(format!(
            "governed Rust source must define string const `{name}` exactly once; found {}",
            matches.len()
        ));
    };
    let syn::Expr::Lit(syn::ExprLit {
        lit: syn::Lit::Str(value),
        ..
    }) = item.expr.as_ref()
    else {
        return Err(format!("`{name}` must be one literal string"));
    };
    Ok(value.value())
}

fn exact_string_slice_const(file: &syn::File, name: &str) -> Result<Vec<String>, String> {
    let matches = file
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Const(item) if item.ident == name => Some(item),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [item] = matches.as_slice() else {
        return Err(format!(
            "governed Rust source must define string-slice const `{name}` exactly once; found {}",
            matches.len()
        ));
    };
    let syn::Expr::Reference(reference) = item.expr.as_ref() else {
        return Err(format!("`{name}` must be a reference to one literal array"));
    };
    let syn::Expr::Array(array) = reference.expr.as_ref() else {
        return Err(format!("`{name}` must be a reference to one literal array"));
    };
    array
        .elems
        .iter()
        .map(|element| match element {
            syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(value),
                ..
            }) => Ok(value.value()),
            _ => Err(format!("`{name}` must contain only literal strings")),
        })
        .collect()
}

fn sqlx_query_literals(function: &syn::ItemFn) -> Vec<String> {
    struct Collector(Vec<String>);
    impl<'ast> syn::visit::Visit<'ast> for Collector {
        fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
            if compact_tokens(call.func.as_ref()) == "sqlx::query"
                && let Some(syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Str(value),
                    ..
                })) = call.args.first()
            {
                self.0.push(value.value());
            }
            syn::visit::visit_expr_call(self, call);
        }
    }
    use syn::visit::Visit;
    let mut collector = Collector(Vec::new());
    collector.visit_block(&function.block);
    collector.0
}

fn sqlx_query_family_literals(function: &syn::ItemFn) -> Vec<String> {
    struct Collector(Vec<String>);
    impl<'ast> syn::visit::Visit<'ast> for Collector {
        fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
            let function = compact_tokens(call.func.as_ref());
            if matches!(function.as_str(), "sqlx::query" | "sqlx::query_scalar")
                && let Some(syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Str(value),
                    ..
                })) = call.args.first()
            {
                self.0.push(value.value());
            }
            syn::visit::visit_expr_call(self, call);
        }
    }
    use syn::visit::Visit;
    let mut collector = Collector(Vec::new());
    collector.visit_block(&function.block);
    collector.0
}

fn sqlx_query_literals_in_expr(expression: &syn::Expr) -> Vec<String> {
    struct Collector(Vec<String>);
    impl<'ast> syn::visit::Visit<'ast> for Collector {
        fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
            if compact_tokens(call.func.as_ref()) == "sqlx::query"
                && let Some(syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Str(value),
                    ..
                })) = call.args.first()
            {
                self.0.push(value.value());
            }
            syn::visit::visit_expr_call(self, call);
        }
    }
    use syn::visit::Visit;
    let mut collector = Collector(Vec::new());
    collector.visit_expr(expression);
    collector.0
}

fn digest_section_literals(function: &syn::ItemFn) -> Vec<String> {
    struct Collector(Vec<String>);
    impl<'ast> syn::visit::Visit<'ast> for Collector {
        fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
            if compact_tokens(call.func.as_ref()) == "digest_section"
                && let Some(syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::ByteStr(value),
                    ..
                })) = call.args.iter().nth(1)
            {
                self.0
                    .push(String::from_utf8_lossy(&value.value()).into_owned());
            }
            syn::visit::visit_expr_call(self, call);
        }
    }
    use syn::visit::Visit;
    let mut collector = Collector(Vec::new());
    collector.visit_block(&function.block);
    collector.0
}

fn digest_field_witnesses(function: &syn::ItemFn) -> Result<Vec<(String, String)>, String> {
    struct Collector {
        fields: Vec<(String, String)>,
    }
    impl<'ast> syn::visit::Visit<'ast> for Collector {
        fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
            let function_name = compact_tokens(call.func.as_ref());
            if let Some(framing) = digest_field_call_framing(&function_name) {
                if let Some(syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Str(field),
                    ..
                })) = call.args.last()
                {
                    self.fields.push((field.value(), framing.to_owned()));
                }
            } else if let Some(framing) = direct_digest_call_framing(&function_name)
                && let Some(value) = call.args.iter().nth(1)
            {
                let literals = expression_string_literals(value);
                if let [field] = literals.as_slice() {
                    self.fields.push((field.clone(), framing.to_owned()));
                }
            }
            syn::visit::visit_expr_call(self, call);
        }

        fn visit_expr_for_loop(&mut self, expression: &'ast syn::ExprForLoop) {
            let syn::Pat::Ident(binding) = expression.pat.as_ref() else {
                syn::visit::visit_expr_for_loop(self, expression);
                return;
            };
            let syn::Expr::Array(array) = expression.expr.as_ref() else {
                syn::visit::visit_expr_for_loop(self, expression);
                return;
            };
            let fields = array
                .elems
                .iter()
                .filter_map(|element| match element {
                    syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Str(value),
                        ..
                    }) => Some(value.value()),
                    _ => None,
                })
                .collect::<Vec<_>>();
            if fields.len() == array.elems.len()
                && let Some(framing) =
                    loop_digest_field_framing(&expression.body, binding.ident.to_string().as_str())
            {
                self.fields
                    .extend(fields.into_iter().map(|field| (field, framing.to_owned())));
            }
            syn::visit::visit_expr_for_loop(self, expression);
        }
    }
    use syn::visit::Visit;
    let mut collector = Collector { fields: Vec::new() };
    collector.visit_block(&function.block);
    Ok(collector.fields)
}

fn digest_field_call_framing(function_name: &str) -> Option<&'static str> {
    match function_name {
        "digest_i64_field" => Some("i64"),
        "digest_optional_i64_field" => Some("optional_i64"),
        "digest_text_field" => Some("text"),
        "digest_optional_text_field" => Some("optional_text"),
        "digest_bool_field" => Some("boolean"),
        "digest_blob_field" => Some("blob"),
        _ => None,
    }
}

fn direct_digest_call_framing(function_name: &str) -> Option<&'static str> {
    match function_name {
        "digest_i64" => Some("i64"),
        "digest_text" => Some("text"),
        "digest_optional_text" => Some("optional_text"),
        _ => None,
    }
}

fn expression_string_literals(expression: &syn::Expr) -> Vec<String> {
    struct Collector(Vec<String>);
    impl<'ast> syn::visit::Visit<'ast> for Collector {
        fn visit_lit_str(&mut self, literal: &'ast syn::LitStr) {
            self.0.push(literal.value());
        }
    }
    use syn::visit::Visit;
    let mut collector = Collector(Vec::new());
    collector.visit_expr(expression);
    collector.0
}

fn loop_digest_field_framing(block: &syn::Block, binding: &str) -> Option<&'static str> {
    struct Collector<'a> {
        binding: &'a str,
        framings: Vec<&'static str>,
    }
    impl<'ast> syn::visit::Visit<'ast> for Collector<'_> {
        fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
            let function_name = compact_tokens(call.func.as_ref());
            if let Some(framing) = digest_field_call_framing(&function_name)
                && call
                    .args
                    .last()
                    .is_some_and(|argument| compact_tokens(argument) == self.binding)
            {
                self.framings.push(framing);
            }
            syn::visit::visit_expr_call(self, call);
        }
    }
    use syn::visit::Visit;
    let mut collector = Collector {
        binding,
        framings: Vec::new(),
    };
    collector.visit_block(block);
    match collector.framings.as_slice() {
        [framing] => Some(*framing),
        _ => None,
    }
}

fn exact_impl<'a>(file: &'a syn::File, name: &str) -> Result<&'a syn::ItemImpl, String> {
    let matches = file
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Impl(item)
                if item.trait_.is_none() && compact_tokens(item.self_ty.as_ref()) == name =>
            {
                Some(item)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let [item] = matches.as_slice() else {
        return Err(format!(
            "governed Rust source must define one inherent impl for `{name}`; found {}",
            matches.len()
        ));
    };
    Ok(item)
}

fn exact_associated_method<'a>(
    file: &'a syn::File,
    owner: &str,
    method: &str,
) -> Result<&'a syn::ImplItemFn, String> {
    let mut matches = Vec::new();
    for item in &file.items {
        let Item::Impl(item) = item else {
            continue;
        };
        if compact_tokens(item.self_ty.as_ref()) != owner {
            continue;
        }
        for member in &item.items {
            if let syn::ImplItem::Fn(function) = member
                && function.sig.ident == method
            {
                matches.push(function);
            }
        }
    }
    let [function] = matches.as_slice() else {
        return Err(format!(
            "{owner} must define `{method}` exactly once; found {}",
            matches.len()
        ));
    };
    Ok(function)
}

struct FreeFunctionCollector<'name, 'ast> {
    name: &'name str,
    matches: Vec<&'ast syn::ItemFn>,
}

impl<'ast> syn::visit::Visit<'ast> for FreeFunctionCollector<'_, 'ast> {
    fn visit_item_fn(&mut self, function: &'ast syn::ItemFn) {
        if function.sig.ident == self.name {
            self.matches.push(function);
        }
        syn::visit::visit_item_fn(self, function);
    }
}

fn exact_free_function<'a>(file: &'a syn::File, name: &str) -> Result<&'a syn::ItemFn, String> {
    use syn::visit::Visit;
    let mut collector = FreeFunctionCollector {
        name,
        matches: Vec::new(),
    };
    collector.visit_file(file);
    let [function] = collector.matches.as_slice() else {
        return Err(format!(
            "governed Rust source must define free function `{name}` exactly once; found {}",
            collector.matches.len()
        ));
    };
    Ok(function)
}

fn exact_top_level_function<'a>(
    file: &'a syn::File,
    name: &str,
) -> Result<&'a syn::ItemFn, String> {
    let matches = file
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Fn(function) if function.sig.ident == name => Some(function),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [function] = matches.as_slice() else {
        return Err(format!(
            "governed Rust source must define top-level function `{name}` exactly once; found {}",
            matches.len()
        ));
    };
    Ok(function)
}

fn compact_signature(source: &str) -> Result<String, String> {
    let function = syn::parse_str::<syn::ImplItemFn>(&format!("{source} {{ unreachable!() }}"))
        .map_err(|error| format!("parse authoritative signature `{source}`: {error}"))?;
    Ok(compact_tokens(&function.sig))
}

fn strip_doc_attributes(attributes: &mut Vec<syn::Attribute>) {
    attributes.retain(|attribute| !attribute.path().is_ident("doc"));
}

fn descriptor_for_file(workspace_root: &Path, relative: &str) -> Result<FileDescriptor, String> {
    descriptor_for_bytes(relative, &read_regular_file(workspace_root, relative)?)
}

fn descriptor_for_bytes(relative: &str, bytes: &[u8]) -> Result<FileDescriptor, String> {
    Ok(FileDescriptor {
        path: relative.to_owned(),
        byte_length: byte_length(relative, bytes)?,
        sha256: sha256_hex(bytes),
        hash_algorithm: HASH_ALGORITHM.to_owned(),
    })
}

fn validate_file_descriptor(
    path: &str,
    byte_length: u64,
    sha256: &str,
    hash_algorithm: &str,
) -> Result<(), String> {
    if byte_length == 0 || hash_algorithm != HASH_ALGORITHM {
        return Err(format!("file descriptor `{path}` is invalid"));
    }
    validate_sha256(path, sha256)
}

fn byte_length(relative: &str, bytes: &[u8]) -> Result<u64, String> {
    u64::try_from(bytes.len()).map_err(|_| format!("{relative} byte length does not fit u64"))
}

fn rust_file(workspace_root: &Path, relative: &str) -> Result<syn::File, String> {
    let source = rust_source(workspace_root, relative)?;
    syn::parse_file(&source).map_err(|error| format!("parse {relative}: {error}"))
}

fn rust_source(workspace_root: &Path, relative: &str) -> Result<String, String> {
    let bytes = read_regular_file(workspace_root, relative)?;
    std::str::from_utf8(&bytes)
        .map(str::to_owned)
        .map_err(|error| format!("{relative} must be UTF-8 Rust: {error}"))
}

fn compact_tokens(tokens: &impl ToTokens) -> String {
    tokens.to_token_stream().to_string().replace(' ', "")
}

fn owned(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

fn validate_unique<'a>(
    label: &str,
    values: impl IntoIterator<Item = &'a str>,
) -> Result<(), String> {
    let values = values.into_iter().collect::<Vec<_>>();
    let unique = values.iter().copied().collect::<BTreeSet<_>>();
    if unique.len() != values.len() {
        return Err(format!("{label} must contain no duplicate values"));
    }
    Ok(())
}

fn canonical_json_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, String> {
    let mut bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("serialize canonical JSON: {error}"))?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn validate_canonical_json<T: Serialize>(
    relative: &str,
    bytes: &[u8],
    value: &T,
) -> Result<(), String> {
    let expected = canonical_json_bytes(value)?;
    if bytes != expected {
        return Err(format!(
            "{relative} must use canonical pretty JSON with one trailing newline"
        ));
    }
    Ok(())
}

fn validate_json_schema(schema: &Value, manifest: &Value) -> Result<(), String> {
    let validator = jsonschema::validator_for(schema)
        .map_err(|error| format!("compile {MANIFEST_SCHEMA_RELATIVE}: {error}"))?;
    let errors = validator
        .iter_errors(manifest)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{MANIFEST_RELATIVE} violates {MANIFEST_SCHEMA_RELATIVE}: {}",
            errors.join("; ")
        ))
    }
}

fn validate_digest_sidecar(relative: &str, bytes: &[u8]) -> Result<(), String> {
    let value =
        std::str::from_utf8(bytes).map_err(|error| format!("{relative} must be UTF-8: {error}"))?;
    let Some(digest) = value.strip_suffix('\n') else {
        return Err(format!("{relative} must end with one newline"));
    };
    if digest.contains('\n') {
        return Err(format!("{relative} must contain one digest line"));
    }
    validate_sha256(relative, digest)
}

fn validate_sha256(label: &str, value: &str) -> Result<(), String> {
    if value.len() != 64
        || !value
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(format!("{label} must be canonical lowercase SHA-256 hex"));
    }
    Ok(())
}

fn validate_vector_expected_digest(value: &str) -> Result<(), String> {
    validate_sha256("vector expected digest", value)?;
    if value.as_bytes().iter().all(|byte| *byte == b'0') {
        return Err("vector expected digest must not be an all-zero bootstrap value".to_owned());
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace_root() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("xtask lives under tools in the workspace")
            .to_path_buf()
    }

    #[test]
    fn source_inventory_is_unique_complete_and_excludes_generated_outputs() {
        validate_source_inventory().expect("raw-source rebuild source inventory");
        let mut missing_compiler_input = SOURCE_SPECS.to_vec();
        missing_compiler_input.retain(|spec| spec.path != FLAKE_LOCK_RELATIVE);
        let error = validate_source_inventory_specs(&missing_compiler_input)
            .expect_err("delegated compiler input omission must fail closed");
        assert!(error.contains("nix_input_lock_authority"), "{error}");
        let root = workspace_root();
        validate_complete_event_store_source_closure(&root)
            .expect("complete event-store Rust source closure");
        validate_successor_compiler_input_authority(&root)
            .expect("complete event-store compiler-input authority");
        validate_delegated_compiler_source_pins_with_supersessions(
            &root,
            RASTER_DECODER_SECURITY_SUCCESSOR_DELEGATED_COMPILER_PATHS,
        )
        .expect("delegated compiler source pins outside the active Blossom raster decoder security successor");
        validate_xtask_manifest_authority(&root).expect("xtask compiler authority");
    }

    #[test]
    fn event_store_production_source_inventory_is_test_neutral_and_fail_closed() {
        let root = workspace_root();
        let workspace = tempfile::tempdir().expect("production source workspace");
        let manifest: RawSourceRebuildManifest =
            serde_json::from_slice(&read_regular_file(&root, MANIFEST_RELATIVE).expect("manifest"))
                .expect("typed manifest");
        for relative in std::iter::once(MANIFEST_RELATIVE)
            .chain(std::iter::once(EVENT_STORE_PRODUCTION_SOURCES_RELATIVE))
            .chain(
                manifest
                    .source_files
                    .iter()
                    .map(|source| source.path.as_str())
                    .filter(|path| is_semantic_event_store_production_source(path)),
            )
        {
            let destination = workspace.path().join(relative);
            fs::create_dir_all(destination.parent().expect("source parent"))
                .expect("create source parent");
            fs::copy(root.join(relative), destination).expect("copy production source authority");
        }
        validate_event_store_production_source_authority(workspace.path())
            .expect("current production source authority");

        let error_relative = "crates/event_store/src/error.rs";
        let error_path = workspace.path().join(error_relative);
        let original = fs::read_to_string(&error_path).expect("read error source");
        fs::write(
            &error_path,
            format!("{original}\n#[cfg(test)]\nfn coverage_probe() {{}}\n"),
        )
        .expect("write test-only source addition");
        validate_event_store_production_source_authority(workspace.path())
            .expect("test-only source addition must preserve production identity");

        fs::write(
            &error_path,
            format!("{original}\nfn production_authority_drift() {{}}\n"),
        )
        .expect("write production source drift");
        let error = validate_event_store_production_source_authority(workspace.path())
            .expect_err("production source drift must fail closed");
        assert!(
            error.contains("production Rust authority drifted"),
            "{error}"
        );

        fs::write(&error_path, original).expect("restore error source");
        let inventory_path = workspace
            .path()
            .join(EVENT_STORE_PRODUCTION_SOURCES_RELATIVE);
        let inventory = fs::read_to_string(&inventory_path).expect("read production inventory");
        fs::write(
            &inventory_path,
            inventory.replacen(
                "sha256 = \"e218754814e195a76fcdfa99c4c4abeaa3b045b4c799b56685ed8acfe5edb90b\"",
                "sha256 = \"0000000000000000000000000000000000000000000000000000000000000000\"",
                1,
            ),
        )
        .expect("write production baseline drift");
        let error = validate_event_store_production_source_authority(workspace.path())
            .expect_err("production baseline drift must fail closed");
        assert!(
            error.contains("production Rust authority drifted"),
            "{error}"
        );
    }

    #[test]
    fn delegated_compiler_sources_and_xtask_targets_fail_closed() {
        let root = workspace_root();
        let pinned_workspace = tempfile::tempdir().expect("compiler pin workspace");
        for (relative, _) in DELEGATED_COMPILER_SOURCE_PINS {
            let destination = pinned_workspace.path().join(relative);
            fs::create_dir_all(destination.parent().expect("compiler pin parent"))
                .expect("create compiler pin parent");
            fs::copy(root.join(relative), destination).expect("copy compiler pin source");
        }
        validate_delegated_compiler_source_pins_with_supersessions(
            pinned_workspace.path(),
            RASTER_DECODER_SECURITY_SUCCESSOR_DELEGATED_COMPILER_PATHS,
        )
        .expect("current compiler source pins outside the active Blossom raster decoder security successor");
        fs::write(pinned_workspace.path().join(FLAKE_LOCK_RELATIVE), "{}\n")
            .expect("mutate pinned flake lock");
        validate_delegated_compiler_source_pins_with_supersessions(
            pinned_workspace.path(),
            RASTER_DECODER_SECURITY_SUCCESSOR_DELEGATED_COMPILER_PATHS,
        )
        .expect_err("compiler source mutation must fail closed");

        let unknown = ["build/nix/not-predecessor-pinned.nix"];
        let error = validate_delegated_compiler_source_pins_with_supersessions(
            pinned_workspace.path(),
            &unknown,
        )
        .expect_err("unknown compiler source supersession must fail closed");
        assert!(error.contains("not predecessor-pinned"), "{error}");

        let manifest_workspace = tempfile::tempdir().expect("xtask manifest workspace");
        let manifest_path = manifest_workspace.path().join(XTASK_MANIFEST_RELATIVE);
        fs::create_dir_all(manifest_path.parent().expect("xtask manifest parent"))
            .expect("create xtask manifest parent");
        fs::copy(root.join(XTASK_MANIFEST_RELATIVE), &manifest_path).expect("copy xtask manifest");
        validate_xtask_manifest_authority(manifest_workspace.path())
            .expect("current xtask manifest authority");
        fs::write(
            manifest_workspace.path().join("tools/xtask/build.rs"),
            "fn main() {}\n",
        )
        .expect("write injected build script");
        validate_xtask_manifest_authority(manifest_workspace.path())
            .expect_err("xtask build-script injection must fail closed");
    }

    #[test]
    fn xtask_auto_target_flags_reject_omission_and_retargeting() {
        let root = workspace_root();
        let manifest =
            regular_utf8_source(&root, XTASK_MANIFEST_RELATIVE).expect("current xtask manifest");

        for flag in XTASK_REQUIRED_DISABLED_AUTO_TARGET_FLAGS {
            let assignment = format!("{flag} = false\n");
            let omitted = manifest.replacen(&assignment, "", 1);
            assert_ne!(omitted, manifest, "{flag} omission fixture must mutate");
            let retargeted = manifest.replacen(&assignment, &format!("{flag} = true\n"), 1);
            assert_ne!(retargeted, manifest, "{flag} retarget fixture must mutate");

            for (mutation, label) in [(omitted, "omission"), (retargeted, "retarget")] {
                let workspace = tempfile::tempdir().expect("xtask flag fixture workspace");
                let path = workspace.path().join(XTASK_MANIFEST_RELATIVE);
                fs::create_dir_all(path.parent().expect("xtask manifest fixture parent"))
                    .expect("create xtask manifest fixture parent");
                fs::write(&path, mutation).expect("write xtask manifest mutation");
                let error = validate_xtask_manifest_authority(workspace.path())
                    .expect_err("xtask auto-target flag mutation must fail closed");
                assert!(
                    error.contains(flag),
                    "{flag} {label} error must identify the exact flag: {error}"
                );
            }
        }
    }

    #[test]
    fn xtask_explicit_main_binary_rejects_omission_retargeting_and_additions() {
        let root = workspace_root();
        let manifest =
            regular_utf8_source(&root, XTASK_MANIFEST_RELATIVE).expect("current xtask manifest");
        let exact_target = "[[bin]]\nname = \"xtask\"\npath = \"src/main.rs\"\n";
        assert!(
            manifest.contains(exact_target),
            "explicit xtask binary fixture must match the governed target"
        );

        for (mutation, label) in [
            (manifest.replacen(exact_target, "", 1), "omission"),
            (
                manifest.replacen(
                    exact_target,
                    "[[bin]]\nname = \"injected\"\npath = \"src/main.rs\"\n",
                    1,
                ),
                "name retarget",
            ),
            (
                manifest.replacen(
                    "path = \"src/main.rs\"\n",
                    "path = \"src/injected.rs\"\n",
                    1,
                ),
                "path retarget",
            ),
            (
                format!("{manifest}\n[[bin]]\nname = \"injected\"\npath = \"src/injected.rs\"\n"),
                "additional target",
            ),
            (
                manifest.replacen(
                    "path = \"src/main.rs\"\n",
                    "path = \"src/main.rs\"\ntest = false\n",
                    1,
                ),
                "additional target field",
            ),
        ] {
            assert_ne!(mutation, manifest, "{label} fixture must mutate");
            let workspace = tempfile::tempdir().expect("xtask binary fixture workspace");
            let path = workspace.path().join(XTASK_MANIFEST_RELATIVE);
            fs::create_dir_all(path.parent().expect("xtask manifest fixture parent"))
                .expect("create xtask manifest fixture parent");
            fs::write(&path, mutation).expect("write xtask binary mutation");
            let error = validate_xtask_manifest_authority(workspace.path())
                .expect_err("xtask binary target mutation must fail closed");
            assert!(
                error.contains("exactly one explicit")
                    || error.contains("exactly one explicit `xtask` binary"),
                "{label} error must identify the exact binary authority: {error}"
            );
        }
    }

    #[test]
    fn xtask_auto_target_source_paths_are_forbidden() {
        let root = workspace_root();
        for (forbidden, injected) in [
            ("tools/xtask/build.rs", "tools/xtask/build.rs"),
            ("tools/xtask/src/lib.rs", "tools/xtask/src/lib.rs"),
            ("tools/xtask/src/bin.rs", "tools/xtask/src/bin.rs"),
            ("tools/xtask/src/bin", "tools/xtask/src/bin/injected.rs"),
            ("tools/xtask/tests", "tools/xtask/tests/injected.rs"),
            ("tools/xtask/examples", "tools/xtask/examples/injected.rs"),
            ("tools/xtask/benches", "tools/xtask/benches/injected.rs"),
        ] {
            let workspace = tempfile::tempdir().expect("xtask path fixture workspace");
            let manifest_path = workspace.path().join(XTASK_MANIFEST_RELATIVE);
            fs::create_dir_all(
                manifest_path
                    .parent()
                    .expect("xtask manifest fixture parent"),
            )
            .expect("create xtask manifest fixture parent");
            fs::copy(root.join(XTASK_MANIFEST_RELATIVE), &manifest_path)
                .expect("copy xtask manifest fixture");

            let injected_path = workspace.path().join(injected);
            fs::create_dir_all(injected_path.parent().expect("injected auto-target parent"))
                .expect("create injected auto-target parent");
            fs::write(&injected_path, "fn main() {}\n").expect("write injected auto-target source");

            let error = validate_xtask_manifest_authority(workspace.path())
                .expect_err("xtask auto-target source path must fail closed");
            assert!(
                error.contains(forbidden),
                "auto-target error must identify `{forbidden}`: {error}"
            );
        }
    }

    #[test]
    fn rebuild_marker_token_is_non_cloneable_and_consumed_by_both_routes() {
        let root = workspace_root();
        let reconciliation_relative = "crates/event_store/src/nip09/reconciliation_v1.rs";
        let reconciliation =
            rust_source(&root, reconciliation_relative).expect("reconciliation marker authority");
        let rebuild = rust_source(&root, REBUILD_RUNTIME_SOURCE_RELATIVE)
            .expect("raw rebuild marker authority");
        let reconciliation_file =
            syn::parse_file(&reconciliation).expect("parse reconciliation marker authority");
        let rebuild_file = syn::parse_file(&rebuild).expect("parse raw rebuild marker authority");
        validate_rebuild_marker_token_authority(
            &reconciliation_file,
            reconciliation_relative,
            &rebuild_file,
            REBUILD_RUNTIME_SOURCE_RELATIVE,
        )
        .expect("current rebuild marker token authority");

        let cloneable = reconciliation.replacen(
            "struct SourceRebuildMarkerTokenV1 {",
            "#[derive(Clone)]\nstruct SourceRebuildMarkerTokenV1 {",
            1,
        );
        assert_ne!(cloneable, reconciliation, "cloneable fixture must mutate");
        let cloneable = syn::parse_file(&cloneable).expect("parse cloneable marker fixture");
        validate_rebuild_marker_token_authority(
            &cloneable,
            reconciliation_relative,
            &rebuild_file,
            REBUILD_RUNTIME_SOURCE_RELATIVE,
        )
        .expect_err("cloneable marker token must fail closed");

        let non_consuming = rebuild.replacen(
            "close_source_rebuild_marker(connection, marker).await?;",
            "close_source_rebuild_marker(connection, marker.clone()).await?;",
            1,
        );
        assert_ne!(
            non_consuming, rebuild,
            "non-consuming marker fixture must mutate"
        );
        let non_consuming =
            syn::parse_file(&non_consuming).expect("parse non-consuming marker fixture");
        validate_rebuild_marker_token_authority(
            &reconciliation_file,
            reconciliation_relative,
            &non_consuming,
            REBUILD_RUNTIME_SOURCE_RELATIVE,
        )
        .expect_err("cloned marker consumption must fail closed");

        let forged = rebuild.replacen(
            "    close_source_rebuild_marker(connection, marker).await?;",
            "    let marker = super::SourceRebuildMarkerTokenV1 { generation: plan.generation };\n    close_source_rebuild_marker(connection, marker).await?;",
            1,
        );
        assert_ne!(forged, rebuild, "forged marker fixture must mutate");
        let forged = syn::parse_file(&forged).expect("parse forged marker fixture");
        validate_rebuild_marker_token_authority(
            &reconciliation_file,
            reconciliation_relative,
            &forged,
            REBUILD_RUNTIME_SOURCE_RELATIVE,
        )
        .expect_err("forged marker shadow must fail closed");
    }

    #[test]
    fn compiler_input_authority_rejects_unapproved_inputs_and_path_retargeting() {
        for source in [
            "#[path = \"escape.rs\"]\nmod escape;",
            "#[cfg_attr(target_os = \"ios\", path = \"escape.rs\")]\nmod escape;",
        ] {
            let file = syn::parse_file(source).expect("path-retargeting probe parses");
            let error = validate_exact_successor_compiler_inputs("probe.rs", &file, &[])
                .expect_err("path retargeting must fail closed");
            assert!(error.contains("no path retargeting"), "{error}");
        }

        let file = syn::parse_file("const ESCAPE: &str = include_str!(\"escape.rs\");")
            .expect("compiler-input probe parses");
        let error = validate_exact_successor_compiler_inputs("probe.rs", &file, &[])
            .expect_err("unapproved compiler input must fail closed");
        assert!(error.contains("include_str!"), "{error}");
    }

    #[test]
    fn executable_authority_rejects_should_panic_and_extra_attributes() {
        let workspace = tempfile::tempdir().expect("executable authority workspace");
        let relative = "probe.rs";
        fs::write(
            workspace.path().join(relative),
            "#[test]\n#[should_panic]\nfn governed_test() { panic!(\"bypass\"); }\n",
        )
        .expect("write should-panic probe");
        let error = validate_executable_test(workspace.path(), relative, "governed_test")
            .expect_err("should-panic authority must fail closed");
        assert!(error.contains("exactly one unconditional test attribute"));
    }

    #[test]
    fn command_reachability_rejects_string_literal_witnesses() {
        let root = workspace_root();
        let workspace = tempfile::tempdir().expect("command authority workspace");
        for relative in [CONTRACT_COMMAND_SOURCE_RELATIVE, XTASK_MAIN_SOURCE_RELATIVE] {
            let destination = workspace.path().join(relative);
            fs::create_dir_all(destination.parent().expect("command source parent"))
                .expect("create command source parent");
            fs::copy(root.join(relative), destination).expect("copy command source");
        }
        let main_path = workspace.path().join(XTASK_MAIN_SOURCE_RELATIVE);
        let main = fs::read_to_string(&main_path).expect("xtask main source");
        let bypass = main.replacen(
            "[] => contract::validate_raw_source_rebuild_manifest(&workspace_root()),",
            "[] => { let _ = \"contract::validate_raw_source_rebuild_manifest(&workspace_root())\"; Ok(()) },",
            1,
        );
        assert_ne!(bypass, main, "command bypass fixture must mutate");
        fs::write(main_path, bypass).expect("write command bypass");
        let error = validate_command_reachability(workspace.path())
            .expect_err("string literal must not satisfy command reachability");
        assert!(error.contains("command arm drifted"), "{error}");
    }

    #[test]
    fn delegated_suite_inventory_rejects_missing_and_unrepresented_authorities() {
        let root = workspace_root();
        let bytes = read_regular_file(&root, RESULT_VECTOR_CANONICAL_RELATIVE)
            .expect("raw-source rebuild vector");
        let vector: RawSourceRebuildVector =
            serde_json::from_slice(&bytes).expect("typed raw-source rebuild vector");
        validate_delegated_suite_inventory(&vector).expect("current delegated suite inventory");

        let mut missing = vector.clone();
        missing
            .delegated_suite
            .authorities
            .pop()
            .expect("nonempty suite");
        let error = validate_delegated_suite_inventory(&missing)
            .expect_err("missing delegated authority must fail closed");
        assert!(error.contains("missing"), "{error}");

        let mut unrepresented = vector.clone();
        unrepresented
            .delegated_suite
            .authorities
            .push(DelegatedAuthority {
                authority: "unrepresented_test_v1".to_owned(),
                authority_path: REBUILD_FAILPOINT_TEST_SOURCE_RELATIVE.to_owned(),
            });
        let error = validate_delegated_suite_inventory(&unrepresented)
            .expect_err("unrepresented delegated authority must fail closed");
        assert!(error.contains("unrepresented"), "{error}");

        let mut duplicate = vector.clone();
        duplicate
            .delegated_suite
            .authorities
            .push(duplicate.delegated_suite.authorities[0].clone());
        let error = validate_delegated_suite_inventory(&duplicate)
            .expect_err("duplicate delegated authority must fail closed");
        assert!(error.contains("duplicate"), "{error}");
    }

    #[test]
    fn delegated_suite_contract_lane_rejects_filters_and_lexical_decoys() {
        let root = workspace_root();
        let flake = regular_utf8_source(&root, FLAKE_SOURCE_RELATIVE).expect("flake authority");
        let apps = regular_utf8_source(&root, CONTRACT_APP_SOURCE_RELATIVE)
            .expect("contract app authority");
        let common = regular_utf8_source(&root, CONTRACT_LANE_SOURCE_RELATIVE)
            .expect("contract lane authority");
        let toolchains = regular_utf8_source(&root, TOOLCHAIN_ROUTING_SOURCE_RELATIVE)
            .expect("toolchain routing authority");
        validate_delegated_suite_contract_lane_sources(&flake, &apps, &common, &toolchains)
            .expect("current delegated suite contract lane");

        let flake_bypass = flake
            .replacen(
                "apps = import ./build/nix/apps.nix {",
                "apps = import ./build/nix/apps-bypass.nix {",
                1,
            )
            .replacen(
                "description = \"Radroots Core Libraries\";",
                "description = \"apps = import ./build/nix/apps.nix {\";",
                1,
            );
        assert_ne!(flake_bypass, flake, "flake bypass fixture must mutate");
        validate_delegated_suite_contract_lane_sources(&flake_bypass, &apps, &common, &toolchains)
            .expect_err("string decoy must not satisfy flake app routing");

        let mut common_import_bypass = flake
            .replacen(
                "common = import ./build/nix/common.nix {",
                "common = import ./build/nix/common-bypass.nix {",
                1,
            )
            .replacen(
                "description = \"Radroots Core Libraries\";",
                "description = \"common = import ./build/nix/common.nix {\";",
                1,
            );
        common_import_bypass.push_str("\n# common = import ./build/nix/common.nix {\n");
        assert_ne!(
            common_import_bypass, flake,
            "common import bypass fixture must mutate"
        );
        validate_delegated_suite_contract_lane_sources(
            &common_import_bypass,
            &apps,
            &common,
            &toolchains,
        )
        .expect_err("string and comment decoys must not satisfy flake common routing");

        let common_argument_bypass = flake.replacen(
            "            inherit lib pkgs toolchains;",
            "            inherit lib pkgs;\n            toolchains = import ./build/nix/toolchains-bypass.nix { inherit pkgs; };",
            1,
        );
        assert_ne!(
            common_argument_bypass, flake,
            "common argument bypass fixture must mutate"
        );
        validate_delegated_suite_contract_lane_sources(
            &common_argument_bypass,
            &apps,
            &common,
            &toolchains,
        )
        .expect_err("common toolchain argument substitution must fail closed");

        let toolchain_import_bypass = flake
            .replacen(
                "toolchains = import ./build/nix/toolchains.nix {",
                "toolchains = import ./build/nix/toolchains-bypass.nix {",
                1,
            )
            .replacen(
                "description = \"Radroots Core Libraries\";",
                "description = \"toolchains = import ./build/nix/toolchains.nix {\";",
                1,
            );
        assert_ne!(
            toolchain_import_bypass, flake,
            "toolchain import bypass fixture must mutate"
        );
        validate_delegated_suite_contract_lane_sources(
            &toolchain_import_bypass,
            &apps,
            &common,
            &toolchains,
        )
        .expect_err("string decoy must not satisfy flake toolchain routing");

        let toolchain_postfix_bypass = flake.replacen(
            "toolchains = import ./build/nix/toolchains.nix { inherit pkgs; };",
            "toolchains = import ./build/nix/toolchains.nix { inherit pkgs; } // { stable = null; };",
            1,
        );
        assert_ne!(
            toolchain_postfix_bypass, flake,
            "toolchain postfix bypass fixture must mutate"
        );
        validate_delegated_suite_contract_lane_sources(
            &toolchain_postfix_bypass,
            &apps,
            &common,
            &toolchains,
        )
        .expect_err("toolchain postfix override must fail closed");

        let common_postfix_bypass = flake.replacen(
            "            inherit lib pkgs toolchains;\n          };",
            "            inherit lib pkgs toolchains;\n          } // { contractCommand = \"cargo test -q -p xtask\"; };",
            1,
        );
        assert_ne!(
            common_postfix_bypass, flake,
            "common postfix bypass fixture must mutate"
        );
        validate_delegated_suite_contract_lane_sources(
            &common_postfix_bypass,
            &apps,
            &common,
            &toolchains,
        )
        .expect_err("common postfix override must fail closed");

        let apps_postfix_bypass = flake.replacen(
            "              ;\n          };\n\n          checks =",
            "              ;\n          } // { contract = { type = \"app\"; program = \"/bin/false\"; }; };\n\n          checks =",
            1,
        );
        assert_ne!(
            apps_postfix_bypass, flake,
            "apps postfix bypass fixture must mutate"
        );
        validate_delegated_suite_contract_lane_sources(
            &apps_postfix_bypass,
            &apps,
            &common,
            &toolchains,
        )
        .expect_err("apps postfix override must fail closed");

        let apps_bypass = apps
            .replacen(
                "command = common.contractCommand;",
                "command = \"cargo test -q -p xtask\";",
                1,
            )
            .replacen(
                "description = \"Run the core-library contract lane\";",
                "description = \"command = common.contractCommand;\";",
                1,
            );
        assert_ne!(apps_bypass, apps, "apps bypass fixture must mutate");
        validate_delegated_suite_contract_lane_sources(&flake, &apps_bypass, &common, &toolchains)
            .expect_err("string decoy must not satisfy contract app command routing");

        let contract_path_bypass = apps.replacen(
            "    command = common.contractCommand;\n  };",
            "    command = common.contractCommand;\n    pathPrefix = \"\";\n  };",
            1,
        );
        assert_ne!(
            contract_path_bypass, apps,
            "contract path bypass fixture must mutate"
        );
        validate_delegated_suite_contract_lane_sources(
            &flake,
            &contract_path_bypass,
            &common,
            &toolchains,
        )
        .expect_err("contract path override must fail closed");

        let package_bypass = common.replacen(
            "    \"radroots_event_store\"\n",
            "    # \"radroots_event_store\"\n",
            1,
        );
        assert_ne!(package_bypass, common, "package bypass fixture must mutate");
        validate_delegated_suite_contract_lane_sources(&flake, &apps, &package_bypass, &toolchains)
            .expect_err("commented package must not satisfy literal package inventory");

        let cargo_source_bypass = common.replacen(
            "        ../../build/nix/toolchains.nix\n",
            "        # ../../build/nix/toolchains.nix\n",
            1,
        );
        assert_ne!(
            cargo_source_bypass, common,
            "cargo source bypass fixture must mutate"
        );
        validate_delegated_suite_contract_lane_sources(
            &flake,
            &apps,
            &cargo_source_bypass,
            &toolchains,
        )
        .expect_err("commented source path must not satisfy cargoSource closure");

        let toolchain_bypass = toolchains.replacen(
            "stable = pkgs.rust-bin.fromRustupToolchainFile ../../rust-toolchain.toml;",
            "stable = pkgs.rust-bin.fromRustupToolchainFile ../../rust-toolchain-bypass.toml;",
            1,
        );
        assert_ne!(
            toolchain_bypass, toolchains,
            "stable toolchain bypass fixture must mutate"
        );
        validate_delegated_suite_contract_lane_sources(&flake, &apps, &common, &toolchain_bypass)
            .expect_err("stable toolchain bypass must fail closed");

        let command_bypass = common.replacen(
            "cargo test -q ${coreContractCargoArgs}",
            "cargo test -q -p xtask\n    # cargo test -q ${coreContractCargoArgs}",
            1,
        );
        assert_ne!(
            command_bypass, common,
            "contract command bypass fixture must mutate"
        );
        validate_delegated_suite_contract_lane_sources(&flake, &apps, &command_bypass, &toolchains)
            .expect_err("shell-comment decoy must not satisfy unfiltered package tests");

        let selector_bypass = common.replacen(
            "radroots_nostr/events\";",
            "radroots_nostr/events --lib\";",
            1,
        );
        assert_ne!(
            selector_bypass, common,
            "cargo selector fixture must mutate"
        );
        validate_delegated_suite_contract_lane_sources(
            &flake,
            &apps,
            &selector_bypass,
            &toolchains,
        )
        .expect_err("Cargo target selector must not skip the integration executor");
    }

    #[test]
    fn delegated_suite_flake_lock_rejects_unlocked_or_redirected_direct_inputs() {
        let root = workspace_root();
        validate_flake_lock_authority(&root).expect("current flake lock authority");
        let workspace = tempfile::tempdir().expect("flake lock test workspace");
        let lock_path = workspace.path().join(FLAKE_LOCK_RELATIVE);
        let lock = regular_utf8_source(&root, FLAKE_LOCK_RELATIVE).expect("flake lock source");

        fs::write(
            &lock_path,
            lock.replacen("\"crane\": \"crane\"", "\"crane\": \"nixpkgs\"", 1),
        )
        .expect("write redirected flake lock");
        validate_flake_lock_authority(workspace.path())
            .expect_err("redirected direct input must fail closed");

        fs::write(
            &lock_path,
            lock.replacen("\"narHash\":", "\"untrustedNarHash\":", 1),
        )
        .expect("write unlocked flake lock");
        validate_flake_lock_authority(workspace.path())
            .expect_err("missing direct-input narHash must fail closed");
    }

    #[test]
    fn delegated_suite_test_targets_reject_disabled_or_detached_tests() {
        let root = workspace_root();
        let workspace = tempfile::tempdir().expect("delegated test-target workspace");
        for relative in [
            "crates/event_store/Cargo.toml",
            "crates/event_store/src/store.rs",
            "crates/event_store/src/nip09/reconciliation_v1.rs",
        ] {
            let destination = workspace.path().join(relative);
            fs::create_dir_all(destination.parent().expect("test-target source parent"))
                .expect("create test-target source parent");
            fs::copy(root.join(relative), destination).expect("copy test-target source");
        }
        validate_delegated_suite_test_targets(workspace.path())
            .expect("current delegated suite test targets");

        let cargo_path = workspace.path().join("crates/event_store/Cargo.toml");
        let cargo = fs::read_to_string(&cargo_path).expect("event-store Cargo manifest");
        fs::write(&cargo_path, format!("{cargo}\n[lib]\ntest = false\n"))
            .expect("disable library tests");
        validate_delegated_suite_test_targets(workspace.path())
            .expect_err("disabled delegated library tests must fail closed");
        fs::write(&cargo_path, cargo).expect("restore Cargo manifest");

        let store_path = workspace.path().join("crates/event_store/src/store.rs");
        let store = fs::read_to_string(&store_path).expect("event-store source");
        let detached = store.replacen(
            "#[cfg(test)]\nmod raw_source_rebuild_v1_tests;",
            "#[cfg(any())]\nmod raw_source_rebuild_v1_tests;",
            1,
        );
        assert_ne!(detached, store, "detached test module fixture must mutate");
        fs::write(store_path, detached).expect("detach delegated test module");
        validate_delegated_suite_test_targets(workspace.path())
            .expect_err("detached delegated test module must fail closed");
    }

    #[test]
    fn drift_taxonomy_rejects_variant_and_code_retargeting() {
        let root = workspace_root();
        let source = rust_source(&root, "crates/event_store/src/error.rs")
            .expect("event-store error source");
        let baseline = syn::parse_file(&source).expect("event-store error AST");
        validate_raw_source_rebuild_drift_taxonomy(&baseline)
            .expect("current raw-source rebuild drift taxonomy");

        for mutation in [
            source.replacen(
                "    RebuildPostcondition,",
                "    RebuildPostconditionRetargeted,",
                1,
            ),
            source.replacen(
                "\"source_generation_lineage\"",
                "\"addressable_transition_authority\"",
                1,
            ),
        ] {
            assert_ne!(mutation, source, "taxonomy fixture must mutate");
            let file = syn::parse_file(&mutation).expect("mutated taxonomy AST");
            validate_raw_source_rebuild_drift_taxonomy(&file)
                .expect_err("taxonomy mutation must fail closed");
        }
    }

    #[test]
    fn failpoint_authority_rejects_mapping_retargeting_and_injection_drift() {
        let root = workspace_root();
        let relative = REBUILD_RUNTIME_SOURCE_RELATIVE;
        let source = rust_source(&root, relative).expect("raw-source rebuild runtime");
        let baseline = syn::parse_file(&source).expect("raw-source rebuild AST");
        validate_failpoint_authority(&baseline, relative).expect("current failpoint authority");
        validate_coordinator_authority(&baseline, relative).expect("current coordinator authority");

        let retargeted = source.replacen(
            "Self::AfterFoodAudit => \"after_food_audit\"",
            "Self::AfterFoodAudit => \"after_food_reset_replay\"",
            1,
        );
        assert_ne!(retargeted, source, "retargeted fixture must mutate");
        let retargeted = syn::parse_file(&retargeted).expect("retargeted failpoint AST");
        validate_failpoint_authority(&retargeted, relative)
            .expect_err("retargeted failpoint stage must fail closed");

        let extra = source.replacen(
            "append_source_generation(connection, &plan).await?;",
            "inject_raw_source_rebuild_failpoint_v1(\n            _failpoint,\n            RawSourceRebuildFailpointV1::AfterMarkerOpen,\n        )?;\n        append_source_generation(connection, &plan).await?;",
            1,
        );
        assert_ne!(extra, source, "extra-injection fixture must mutate");
        let extra = syn::parse_file(&extra).expect("extra-injection AST");
        validate_failpoint_authority(&extra, relative)
            .expect_err("extra rollback injection must fail closed");

        let omitted = source.replacen(
            "    #[cfg(test)]\n    inject_raw_source_rebuild_failpoint_v1(\n        _failpoint,\n        RawSourceRebuildFailpointV1::AfterFoodAudit,\n    )?;\n",
            "",
            1,
        );
        assert_ne!(omitted, source, "omitted-injection fixture must mutate");
        let omitted = syn::parse_file(&omitted).expect("omitted-injection AST");
        validate_failpoint_authority(&omitted, relative)
            .expect_err("omitted rollback injection must fail closed");
    }

    #[test]
    fn failpoint_test_array_rejects_member_omission() {
        let root = workspace_root();
        let source = rust_source(&root, REBUILD_FAILPOINT_TEST_SOURCE_RELATIVE)
            .expect("raw-source rebuild failpoint tests");
        let baseline = syn::parse_file(&source).expect("raw-source rebuild failpoint test AST");
        validate_failpoint_test_array_authority(&baseline, REBUILD_FAILPOINT_TEST_SOURCE_RELATIVE)
            .expect("current failpoint test array authority");

        let omitted = source.replacen(
            "        RawSourceRebuildFailpointV1::AfterVisibilityAudit,\n",
            "",
            1,
        );
        assert_ne!(omitted, source, "test-array omission fixture must mutate");
        let omitted = syn::parse_file(&omitted).expect("omitted failpoint test array AST");
        validate_failpoint_test_array_authority(&omitted, REBUILD_FAILPOINT_TEST_SOURCE_RELATIVE)
            .expect_err("omitted failpoint test array member must fail closed");
    }

    #[test]
    fn digest_streaming_authority_rejects_duplicate_and_missing_row_markers() {
        let root = workspace_root();
        let relative = REBUILD_RUNTIME_SOURCE_RELATIVE;
        let source = rust_source(&root, relative).expect("raw-source rebuild runtime");
        for mutation in [
            source.replacen(
                "        digest_row_start(&mut digest);",
                "        digest_row_start(&mut digest);\n        digest_row_start(&mut digest);",
                1,
            ),
            source.replacen("        digest_row_start(&mut digest);\n", "", 1),
        ] {
            assert_ne!(mutation, source, "row-marker fixture must mutate");
            let file = syn::parse_file(&mutation).expect("mutated digest runtime AST");
            let function = exact_free_function(&file, "immutable_raw_digest_v1")
                .expect("immutable raw digest authority");
            let error = validate_digest_streaming_authority(
                function,
                relative,
                "immutable_raw_digest_v1",
                RAW_DIGEST_QUERY_SPECS,
            )
            .expect_err("row-marker framing mutation must fail closed");
            assert!(error.contains("exactly one top-level"), "{error}");
        }
    }

    #[test]
    fn migration_inventory_remains_runtime_only_v4() {
        validate_migration_inventory(&workspace_root()).expect("exact migration inventory");
    }

    #[test]
    fn source_maintenance_predecessor_identity_is_frozen() {
        let root = workspace_root();
        let bytes = read_regular_file(&root, PREDECESSOR_MANIFEST_RELATIVE)
            .expect("SourceMaintenance predecessor manifest");
        validate_predecessor_identity(&bytes).expect("frozen predecessor identity");
        validate_predecessor_source_supersession(&root, &bytes)
            .expect("changed predecessor sources are superseded");
    }

    #[test]
    fn generated_bundle_render_is_deterministic() {
        let root = workspace_root();
        crate::contract::phase1_publication_artifact::validate_immutable_raw_source_rebuild_predecessor(
            &root,
        )
        .expect("first immutable predecessor validation");
        crate::contract::phase1_publication_artifact::validate_immutable_raw_source_rebuild_predecessor(
            &root,
        )
        .expect("second immutable predecessor validation");

        let checked_in = read_regular_file(&root, MANIFEST_RELATIVE).expect("immutable manifest");
        let manifest: RawSourceRebuildManifest =
            serde_json::from_slice(&checked_in).expect("typed immutable manifest");
        let first = canonical_json_bytes(&manifest).expect("first immutable render");
        let second = canonical_json_bytes(&manifest).expect("second immutable render");
        assert_eq!(first, second);
        assert_eq!(first, checked_in);
    }

    #[test]
    fn direct_vector_digest_rejects_bootstrap_placeholder() {
        let error = validate_vector_expected_digest(&"0".repeat(64))
            .expect_err("all-zero bootstrap digest must fail closed");
        assert!(error.contains("bootstrap"), "{error}");
    }

    #[test]
    fn executable_vector_case_inventory_is_frozen() {
        let root = workspace_root();
        let bytes = read_regular_file(&root, RESULT_VECTOR_CANONICAL_RELATIVE)
            .expect("raw-source rebuild vector");
        validate_result_vector_identity(&bytes).expect("immutable vector identity");

        let mut reduced = bytes;
        reduced.pop();
        let error = validate_result_vector_identity(&reduced)
            .expect_err("reduced vector inventory must fail closed");
        assert!(
            error.contains("immutable executable case inventory"),
            "{error}"
        );
    }

    #[test]
    fn rollback_vector_requires_exact_failpoint_case_ids() {
        let root = workspace_root();
        let bytes = read_regular_file(&root, RESULT_VECTOR_CANONICAL_RELATIVE)
            .expect("raw-source rebuild vector");
        let mut vector = serde_json::from_slice::<RawSourceRebuildVector>(&bytes)
            .expect("raw-source rebuild vector schema");
        validate_failpoint_result_vector_cases(&vector)
            .expect("current rollback failpoint vector cases");

        let case = vector
            .cases
            .iter_mut()
            .find(|case| case.id == "rollback_after_marker_open")
            .expect("marker-open rollback case");
        case.id = "rollback_after_marker_open_retargeted".to_owned();
        validate_failpoint_result_vector_cases(&vector)
            .expect_err("retargeted rollback failpoint case ID must fail closed");
    }

    #[test]
    fn public_error_runtime_and_command_authorities_are_active() {
        let root = workspace_root();
        validate_public_api_authority(&root).expect("public API authority");
        validate_error_authority(&root).expect("typed error authority");
        validate_runtime_authority(&root).expect("runtime authority");
        validate_command_reachability(&root).expect("command and release reachability");
    }

    #[test]
    fn caller_schema_dependency_authority_rejects_limits_narrowing_and_bypass() {
        let root = workspace_root();
        let relative = REBUILD_RUNTIME_SOURCE_RELATIVE;
        let source = rust_source(&root, relative).expect("raw-source rebuild runtime");
        let baseline = syn::parse_file(&source).expect("raw-source rebuild AST");
        validate_caller_schema_dependency_authority(&baseline, relative)
            .expect("current caller-schema dependency authority");
        validate_scoped_integrity_authority(&baseline, relative)
            .expect("current mutated-parent inventory authority");
        validate_coordinator_authority(&baseline, relative)
            .expect("current rebuild coordinator authority");

        let missing_mutated_parent = source.replacen("    \"sqlite_sequence\",\n", "", 1);
        assert_ne!(
            missing_mutated_parent, source,
            "mutated-parent omission fixture must mutate"
        );
        let missing_mutated_parent =
            syn::parse_file(&missing_mutated_parent).expect("parse mutated-parent omission AST");
        validate_scoped_integrity_authority(&missing_mutated_parent, relative)
            .expect_err("mutated-parent omission must fail closed");

        for (label, mutation) in [
            (
                "limit-retarget",
                source.replacen(
                    "const RAW_SOURCE_REBUILD_CALLER_MAIN_TABLE_COUNT_LIMIT_V1: u32 = 4_096;",
                    "const RAW_SOURCE_REBUILD_CALLER_MAIN_TABLE_COUNT_LIMIT_V1: u32 = 4_097;",
                    1,
                ),
            ),
            (
                "unqualified-foreign-key-inventory",
                source.replacen(
                    "JOIN main.pragma_foreign_key_list(child.name, 'main') AS foreign_key",
                    "JOIN pragma_foreign_key_list(child.name) AS foreign_key",
                    1,
                ),
            ),
            (
                "action-filter",
                source.replacen(
                    r#"ON foreign_key.\"table\" COLLATE NOCASE = rebuild_parent.name"#,
                    r#"ON foreign_key.\"table\" COLLATE NOCASE = rebuild_parent.name
           AND foreign_key.on_delete = 'CASCADE'"#,
                    1,
                ),
            ),
            (
                "temporary-schema-redirection",
                source.replacen(
                    "FROM main.sqlite_schema AS child",
                    "FROM temp.sqlite_schema AS child",
                    1,
                ),
            ),
        ] {
            assert_ne!(mutation, source, "{label} fixture must mutate");
            let mutation = syn::parse_file(&mutation)
                .unwrap_or_else(|error| panic!("parse {label} caller-schema AST: {error}"));
            assert!(
                validate_caller_schema_dependency_authority(&mutation, relative).is_err(),
                "{label} caller-schema bypass must fail closed"
            );
        }

        let coordinator_bypass = source.replacen(
            "preflight_caller_owned_schema_dependencies_v1(connection, caller_schema_limits).await?;",
            "let _ = caller_schema_limits;",
            1,
        );
        assert_ne!(
            coordinator_bypass, source,
            "coordinator bypass fixture must mutate"
        );
        let coordinator_bypass =
            syn::parse_file(&coordinator_bypass).expect("parse coordinator bypass AST");
        validate_coordinator_authority(&coordinator_bypass, relative)
            .expect_err("caller-schema preflight bypass must fail closed");
    }

    #[test]
    fn cold_repair_authority_rejects_caller_mode_and_route_bypasses() {
        let root = workspace_root();
        let relative = "crates/event_store/src/store.rs";
        let source = rust_source(&root, relative).expect("event-store source");
        let baseline = syn::parse_file(&source).expect("event-store AST");
        validate_public_entry_point_authority(&baseline, relative)
            .expect("current cold-repair authority");

        let caller_mode = source.replacen(
            "pub async fn repair_file_from_raw_v1(\n        path: impl AsRef<Path>,\n    )",
            "pub async fn repair_file_from_raw_v1(\n        path: impl AsRef<Path>,\n        pool: SqlitePool,\n    )",
            1,
        );
        assert_ne!(caller_mode, source, "caller-mode fixture must mutate");
        let caller_mode = syn::parse_file(&caller_mode).expect("caller-mode AST");
        let error = validate_public_entry_point_authority(&caller_mode, relative)
            .expect_err("caller-supplied backing mode must fail closed");
        assert!(error.contains("signature drifted"), "{error}");

        for (label, mutation) in [
            (
                "multi-connection",
                source.replacen(
                    ".max_connections(1)\n            .connect_with(options)\n            .await?;\n        pool.set_connect_options(raw_source_repair_connect_options_v1(&canonical_path));",
                    ".max_connections(2)\n            .connect_with(options)\n            .await?;\n        pool.set_connect_options(raw_source_repair_connect_options_v1(&canonical_path));",
                    1,
                ),
            ),
            (
                "lock-domain-bypass",
                source.replacen(
                    "validate_raw_source_repair_canonical_lock_domain_v1(&canonical_path).await",
                    "Ok(())",
                    1,
                ),
            ),
            (
                "identity-bypass",
                source.replacen(
                    "    if actual != canonical_path {",
                    "    if false && actual != canonical_path {",
                    1,
                ),
            ),
            (
                "unqualified-lock-probe",
                source.replacen(
                    "UPDATE main.radroots_event_store_write_lock",
                    "UPDATE radroots_event_store_write_lock",
                    1,
                ),
            ),
            (
                "create-missing-file",
                source.replacen(".create_if_missing(false)", ".create_if_missing(true)", 1),
            ),
        ] {
            assert_ne!(mutation, source, "{label} fixture must mutate");
            let mutation = syn::parse_file(&mutation)
                .unwrap_or_else(|error| panic!("parse {label} cold-repair bypass AST: {error}"));
            assert!(
                validate_public_entry_point_authority(&mutation, relative).is_err(),
                "{label} cold-repair bypass must fail closed"
            );
        }
    }

    #[test]
    fn schema_rejects_unknown_runtime_fields() {
        let root = workspace_root();
        let schema: Value = serde_json::from_slice(
            &read_regular_file(&root, MANIFEST_SCHEMA_RELATIVE).expect("immutable schema bytes"),
        )
        .expect("immutable schema");
        let mut manifest: Value = serde_json::from_slice(
            &read_regular_file(&root, MANIFEST_RELATIVE).expect("immutable manifest bytes"),
        )
        .expect("immutable manifest");
        manifest
            .pointer_mut("/runtime")
            .and_then(Value::as_object_mut)
            .expect("runtime object")
            .insert("unbounded_scan".to_owned(), Value::Bool(true));
        let error = validate_json_schema(&schema, &manifest)
            .expect_err("unknown runtime field must fail closed");
        assert!(error.contains("violates"), "{error}");
    }
}
