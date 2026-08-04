#![allow(dead_code)]

use super::artifact_bundle::{
    GeneratedArtifact, read_regular_file, with_artifact_bundle_transaction,
};
use super::food_availability_projection::validate_food_availability_projection_predecessor_production_sources_under_lock;
use super::nip09_reconciliation::validate_current_event_store_successor_authority;
use quote::ToTokens;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use syn::{Expr, Item, UseTree};

const SCHEMA_VERSION: u32 = 1;
const CONTRACT_ID: &str = "radroots_event_store.source_maintenance_v1";
const HOOK_ID: &str = "source_maintenance_v1";
const MIGRATION_VERSION: u32 = 4;
const MIGRATION_NAME: &str = "source_maintenance";
const CAPACITY_VERSION: u32 = 1;
const EVENT_CONTRACT_REGISTRY_VERSION: u32 = 7;
const CAPACITY_AUTHORITY_ID: &str = "radroots_event_store_source_capacity_v1";
const ACCOUNTING_ALGORITHM: &str = "sqlite_cast_blob_octet_sum_v1";
const REOPEN_VALIDATION_MODE: &str = "bounded_full_raw_recount_v1";
const GENERATION_HISTORY_VALIDATION: &str = "bounded_count_plus_active_ordinal_v1";
const RAW_EVENT_COUNT_LIMIT: u64 = 25_000;
const RAW_TAG_COUNT_LIMIT: u64 = 250_000;
const RAW_EVENT_TEXT_BYTES_LIMIT: u64 = 67_108_864;
const RAW_TAG_TEXT_BYTES_LIMIT: u64 = 33_554_432;
const RETAINED_SOURCE_GENERATION_LIMIT: u32 = 8;
const RAW_EVENT_REJECTION_SCAN_BOUND: u64 = RAW_EVENT_COUNT_LIMIT + 1;
const RAW_TAG_REJECTION_SCAN_BOUND: u64 = RAW_TAG_COUNT_LIMIT + 1;
const RETAINED_GENERATION_REJECTION_SCAN_BOUND: u32 = RETAINED_SOURCE_GENERATION_LIMIT + 1;
const SCHEMA_SHA256: &str = "074f85b663444ac150239ecd8441ea4a96ad83a798a55e22d2e5e2f7ee943a8c";
const HASH_ALGORITHM: &str = "sha256_bytes_v1";
const WRITE_COMMAND: &str = "cargo xtask contract source-maintenance-manifest --write";

const PREDECESSOR_HOOK_ID: &str = "food_availability_projection_v1";
const PREDECESSOR_MANIFEST_RELATIVE: &str =
    "crates/event_store/contracts/food_availability_projection_v1.manifest.json";
const PREDECESSOR_MANIFEST_BYTE_LENGTH: usize = 17_455;
const PREDECESSOR_MANIFEST_SHA256: &str =
    "02dfe1b450fbdac16e718888215b4dd5c85d8975440fa21e8f439fb24c2b2990";
const NIP09_HOOK_ID: &str = "nip09_reconciliation_v1";
const NIP09_MANIFEST_SHA256: &str =
    "74af832420ffbaa9805e89df3c0b34f126a443e1598f757e3372f407f9003b77";
const FOOD_SCOPE_FINGERPRINT_SHA256: &str =
    "8b63c5ddc48a2cc7db69295238b96d5f814dba50427c80b4d0079f061e6d3de0";
const ACTIVE_GENERATION_AUTHORITY: &str = "radroots_event_store_source_state";
const MARKER_CLOSE_AUTHORITY: &str = "radroots_event_store_source_capacity_marker_close_guard";

const MANIFEST_RELATIVE: &str = "crates/event_store/contracts/source_maintenance_v1.manifest.json";
const MANIFEST_SCHEMA_RELATIVE: &str =
    "crates/event_store/contracts/source_maintenance_v1.manifest.schema.json";
const MANIFEST_SHA256_RELATIVE: &str =
    "crates/event_store/contracts/source_maintenance_v1.manifest.sha256";
const GENERATED_DESCRIPTOR_RELATIVE: &str =
    "crates/event_store/src/generated/source_maintenance_manifest.rs";
const MIGRATIONS_SOURCE_RELATIVE: &str = "crates/event_store/src/migrations.rs";
const MIGRATION_UP_RELATIVE: &str = "crates/event_store/migrations/0004_source_maintenance.up.sql";
const MIGRATION_DOWN_RELATIVE: &str =
    "crates/event_store/migrations/0004_source_maintenance.down.sql";
const RESULT_VECTOR_CANONICAL_RELATIVE: &str =
    "contracts/conformance/vectors/event_store/source_maintenance.v1.json";
const RESULT_VECTOR_MIRROR_RELATIVE: &str =
    "crates/event_store/tests/fixtures/source_maintenance.v1.json";
const RESULT_VECTOR_EXECUTOR_RELATIVE: &str =
    "crates/event_store/tests/source_maintenance_v1_result_vector.rs";
const RESULT_VECTOR_EXECUTOR_ID: &str =
    "radroots_event_store.source_maintenance_v1.result_vector_executor.v1";
const RESULT_VECTOR_EXECUTOR_TEST: &str = "source_maintenance_v1_result_vector";
const FOOD_PREDECESSOR_RESULT_VECTOR_EXECUTOR_RELATIVE: &str =
    "crates/event_store/tests/food_availability_projection_v1_result_vector.rs";
const NIP09_SUCCESSOR_RESULT_VECTOR_EXECUTOR_RELATIVE: &str =
    "crates/event_store/tests/support/nip09_reconciliation_v1_result_vector_v2.rs";
const NIP09_SUCCESSOR_RESULT_VECTOR_EXECUTOR_ID: &str =
    "radroots_event_store.nip09_reconciliation_v1.result_vector_executor.v2";
const CONTRACT_COMMAND_SOURCE_RELATIVE: &str = "tools/xtask/src/contract.rs";
const XTASK_MAIN_SOURCE_RELATIVE: &str = "tools/xtask/src/main.rs";
const XTASK_MAIN_FULL_AST_SHA256: &str =
    "ef32f8973e24dba1cc4727152d2998cfebe79b43cda889d9e99d9541054a3f3f";

const RAW_EVENT_COLUMNS: &[&str] = &[
    "event_id",
    "pubkey",
    "tags_json",
    "content",
    "sig",
    "raw_json",
];
const RAW_TAG_COLUMNS: &[&str] = &["event_id", "tag_name", "tag_value", "tag_json"];
const NULLABLE_RAW_TAG_COLUMNS: &[&str] = &["tag_value"];

const EXPECTED_CATALOG_OBJECTS: &[&str] = &[
    "radroots_event_store_source_capacity_delete_guard",
    "radroots_event_store_source_capacity_insert_guard",
    "radroots_event_store_source_capacity_marker_close_guard",
    "radroots_event_store_source_capacity_update_guard",
    "radroots_event_store_source_capacity_v1",
    "radroots_event_store_source_generation_capacity_advance",
    "radroots_event_store_source_generation_capacity_guard",
];
const EXPECTED_CATALOG_TABLES: &[&str] = &["radroots_event_store_source_capacity_v1"];
const EXPECTED_REPLACED_CATALOG_OBJECTS: &[&str] = &[
    "radroots_event_store_food_availability_image_delete_guard",
    "radroots_event_store_food_availability_projection_delete_guard",
    "radroots_event_store_source_rebuild_marker_insert_guard",
];

const INHERITED_PUBLIC_API: &[&str] = &[
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

const ADDED_PUBLIC_API: &[&str] = &[
    "RADROOTS_EVENT_STORE_RAW_EVENT_COUNT_LIMIT_V1",
    "RADROOTS_EVENT_STORE_RAW_EVENT_TEXT_BYTES_LIMIT_V1",
    "RADROOTS_EVENT_STORE_RAW_TAG_COUNT_LIMIT_V1",
    "RADROOTS_EVENT_STORE_RAW_TAG_TEXT_BYTES_LIMIT_V1",
    "RADROOTS_EVENT_STORE_RETAINED_SOURCE_GENERATION_LIMIT_V1",
    "RadrootsEventStoreSourceCapacityResourceV1",
    "RadrootsEventStoreSourceCapacityV1",
];

const PUBLIC_METHODS: &[&str] = &[
    "RadrootsEventStore::source_capacity_v1",
    "RadrootsEventStoreSourceCapacityResourceV1::as_str",
    "RadrootsEventStoreSourceCapacityV1::source_generation",
    "RadrootsEventStoreSourceCapacityV1::raw_event_count",
    "RadrootsEventStoreSourceCapacityV1::raw_tag_count",
    "RadrootsEventStoreSourceCapacityV1::raw_event_text_bytes",
    "RadrootsEventStoreSourceCapacityV1::raw_tag_text_bytes",
    "RadrootsEventStoreSourceCapacityV1::raw_high_water_seq",
    "RadrootsEventStoreSourceCapacityV1::retained_generation_count",
    "RadrootsEventStoreSourceCapacityV1::retained_generation_limit",
];
const ERROR_VARIANTS: &[&str] = &[
    "SourceCapacityExceeded",
    "SourceGenerationHistoryLimitReached",
    "PersistedEphemeralRawEvent",
    "SourceCapacityStateDrift",
    "SqliteMainDatabaseEncodingNotUtf8",
    "RollbackWouldDiscardSourceGenerationHistory",
];
const REMOVED_PUBLIC_API: &[&str] = &[
    "RadrootsEventStoreReconciliationResource",
    "RadrootsEventStoreError::ReconciliationCapacityExceeded",
];
const BREAKING_PUBLIC_API_REPLACEMENTS: &[(&str, &str)] = &[
    (
        "RadrootsEventStoreReconciliationResource",
        "RadrootsEventStoreSourceCapacityResourceV1",
    ),
    (
        "RadrootsEventStoreError::ReconciliationCapacityExceeded",
        "RadrootsEventStoreError::SourceCapacityExceeded",
    ),
];

const GOVERNED_MODEL_MODULES: &[&str] = &[
    "addressable_transition_feed_v1",
    "current_visibility_v1",
    "food_availability_projection_v1",
];

const ENTRY_POINTS: &[(&str, &str)] = &[
    (
        "migration_registry",
        "radroots_event_store::migrations::EVENT_STORE_MIGRATIONS[3]",
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
        "capacity_query",
        "radroots_event_store::RadrootsEventStore::source_capacity_v1",
    ),
    (
        "raw_append_preflight",
        "radroots_event_store::source_maintenance_v1::preflight_unique_raw_source_append_v1",
    ),
    (
        "raw_append_advance",
        "radroots_event_store::source_maintenance_v1::advance_source_capacity_after_insert_v1",
    ),
    (
        "generation_append_preflight",
        "radroots_event_store::source_maintenance_v1::preflight_source_generation_append_v1",
    ),
    (
        "generation_rebuild_bind",
        "radroots_event_store::source_maintenance_v1::bind_source_capacity_to_generation_v1",
    ),
    (
        "sqlite_encoding_preflight",
        "radroots_event_store::store::validate_main_database_encoding",
    ),
    (
        "source_generation_history_rollback_guard",
        "radroots_event_store::schema::validate_rollback_preserves_source_generation_history",
    ),
    ("result_vector_executor", RESULT_VECTOR_EXECUTOR_TEST),
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
        role: "core_currency_value_authority",
        path: "crates/core/src/currency.rs",
    },
    SourceSpec {
        role: "core_decimal_value_authority",
        path: "crates/core/src/decimal.rs",
    },
    SourceSpec {
        role: "core_money_value_authority",
        path: "crates/core/src/money.rs",
    },
    SourceSpec {
        role: "core_percent_value_authority",
        path: "crates/core/src/percent.rs",
    },
    SourceSpec {
        role: "core_quantity_value_authority",
        path: "crates/core/src/quantity.rs",
    },
    SourceSpec {
        role: "core_quantity_price_value_authority",
        path: "crates/core/src/quantity_price.rs",
    },
    SourceSpec {
        role: "core_unit_value_authority",
        path: "crates/core/src/unit.rs",
    },
    SourceSpec {
        role: "blossom_public_surface",
        path: "crates/blossom/src/lib.rs",
    },
    SourceSpec {
        role: "blossom_authorization_authority",
        path: "crates/blossom/src/authorization.rs",
    },
    SourceSpec {
        role: "blossom_descriptor_authority",
        path: "crates/blossom/src/descriptor.rs",
    },
    SourceSpec {
        role: "blossom_error_authority",
        path: "crates/blossom/src/error.rs",
    },
    SourceSpec {
        role: "blossom_hash_authority",
        path: "crates/blossom/src/hash.rs",
    },
    SourceSpec {
        role: "blossom_media_type_authority",
        path: "crates/blossom/src/media_type.rs",
    },
    SourceSpec {
        role: "blossom_url_authority",
        path: "crates/blossom/src/url.rs",
    },
    SourceSpec {
        role: "event_public_surface",
        path: "crates/event/src/lib.rs",
    },
    SourceSpec {
        role: "event_contract_facade",
        path: "crates/event/src/contract.rs",
    },
    SourceSpec {
        role: "event_contract_registry_v7_authority",
        path: "crates/event/src/contract/registry_v7.rs",
    },
    SourceSpec {
        role: "event_envelope_authority",
        path: "crates/event/src/envelope.rs",
    },
    SourceSpec {
        role: "event_verification_typestate_authority",
        path: "crates/event/src/verification.rs",
    },
    SourceSpec {
        role: "event_admission_typestate_authority",
        path: "crates/event/src/admission.rs",
    },
    SourceSpec {
        role: "event_head_facade",
        path: "crates/event/src/event_head.rs",
    },
    SourceSpec {
        role: "event_head_v1_authority",
        path: "crates/event/src/event_head/v1.rs",
    },
    SourceSpec {
        role: "event_ids_authority",
        path: "crates/event/src/id.rs",
    },
    SourceSpec {
        role: "event_trade_authority",
        path: "crates/event/src/trade.rs",
    },
    SourceSpec {
        role: "event_kinds_authority",
        path: "crates/event/src/kinds.rs",
    },
    SourceSpec {
        role: "event_tags_authority",
        path: "crates/event/src/tags.rs",
    },
    SourceSpec {
        role: "event_draft_authority",
        path: "crates/event/src/draft.rs",
    },
    SourceSpec {
        role: "event_calendar_authority",
        path: "crates/event/src/calendar.rs",
    },
    SourceSpec {
        role: "event_classified_listing_authority",
        path: "crates/event/src/classified_listing.rs",
    },
    SourceSpec {
        role: "event_profile_authority",
        path: "crates/event/src/profile.rs",
    },
    SourceSpec {
        role: "event_post_authority",
        path: "crates/event/src/post.rs",
    },
    SourceSpec {
        role: "event_comment_authority",
        path: "crates/event/src/comment.rs",
    },
    SourceSpec {
        role: "event_food_availability_authority",
        path: "crates/event/src/food_availability.rs",
    },
    SourceSpec {
        role: "event_deletion_authority",
        path: "crates/event/src/deletion.rs",
    },
    SourceSpec {
        role: "event_dto_authority",
        path: "crates/event/src/dto.rs",
    },
    SourceSpec {
        role: "event_farm_crdt_authority",
        path: "crates/event/src/farm_crdt.rs",
    },
    SourceSpec {
        role: "event_knowledge_authority",
        path: "crates/event/src/knowledge.rs",
    },
    SourceSpec {
        role: "event_operational_listing_authority",
        path: "crates/event/src/operational_listing.rs",
    },
    SourceSpec {
        role: "event_order_authority",
        path: "crates/event/src/order.rs",
    },
    SourceSpec {
        role: "event_reply_authority",
        path: "crates/event/src/reply.rs",
    },
    SourceSpec {
        role: "event_trade_validation_authority",
        path: "crates/event/src/trade_validation.rs",
    },
    SourceSpec {
        role: "event_relay_hint_authority",
        path: "crates/event/src/relay_hint.rs",
    },
    SourceSpec {
        role: "event_media_authority",
        path: "crates/event/src/media.rs",
    },
    SourceSpec {
        role: "event_social_authority",
        path: "crates/event/src/social.rs",
    },
    SourceSpec {
        role: "event_wire_facade",
        path: "crates/event/src/wire.rs",
    },
    SourceSpec {
        role: "event_wire_v1_authority",
        path: "crates/event/src/wire/v1.rs",
    },
    SourceSpec {
        role: "event_codec_public_surface",
        path: "crates/event_codec/src/lib.rs",
    },
    SourceSpec {
        role: "event_codec_verification_facade",
        path: "crates/event_codec/src/verification.rs",
    },
    SourceSpec {
        role: "event_codec_verification_v1_authority",
        path: "crates/event_codec/src/verification/v1.rs",
    },
    SourceSpec {
        role: "event_codec_registry_v7_admission_authority",
        path: "crates/event_codec/src/admission/registry_v7.rs",
    },
    SourceSpec {
        role: "event_codec_admission_facade",
        path: "crates/event_codec/src/admission.rs",
    },
    SourceSpec {
        role: "event_codec_profile_inbound_facade",
        path: "crates/event_codec/src/profile/inbound.rs",
    },
    SourceSpec {
        role: "event_codec_profile_registry_v7_authority",
        path: "crates/event_codec/src/profile/inbound/registry_v7.rs",
    },
    SourceSpec {
        role: "event_codec_post_inbound_facade",
        path: "crates/event_codec/src/post/inbound.rs",
    },
    SourceSpec {
        role: "event_codec_post_registry_v7_authority",
        path: "crates/event_codec/src/post/inbound/registry_v7.rs",
    },
    SourceSpec {
        role: "event_codec_reply_inbound_facade",
        path: "crates/event_codec/src/reply/inbound.rs",
    },
    SourceSpec {
        role: "event_codec_reply_registry_v7_authority",
        path: "crates/event_codec/src/reply/inbound/registry_v7.rs",
    },
    SourceSpec {
        role: "event_codec_comment_inbound_facade",
        path: "crates/event_codec/src/comment/inbound.rs",
    },
    SourceSpec {
        role: "event_codec_comment_registry_v7_authority",
        path: "crates/event_codec/src/comment/inbound/registry_v7.rs",
    },
    SourceSpec {
        role: "event_codec_deletion_facade",
        path: "crates/event_codec/src/deletion/mod.rs",
    },
    SourceSpec {
        role: "event_codec_deletion_reconciliation_v1_authority",
        path: "crates/event_codec/src/deletion/reconciliation_v1.rs",
    },
    SourceSpec {
        role: "event_codec_error_authority",
        path: "crates/event_codec/src/error.rs",
    },
    SourceSpec {
        role: "event_codec_food_admission_authority",
        path: "crates/event_codec/src/food_availability/admission.rs",
    },
    SourceSpec {
        role: "event_codec_food_authored_authority",
        path: "crates/event_codec/src/food_availability/authored.rs",
    },
    SourceSpec {
        role: "event_codec_food_inbound_facade",
        path: "crates/event_codec/src/food_availability/inbound.rs",
    },
    SourceSpec {
        role: "event_codec_food_registry_v7_authority",
        path: "crates/event_codec/src/food_availability/inbound/registry_v7.rs",
    },
    SourceSpec {
        role: "event_codec_job_traits_authority",
        path: "crates/event_codec/src/job/traits.rs",
    },
    SourceSpec {
        role: "event_codec_job_encode_authority",
        path: "crates/event_codec/src/job/encode.rs",
    },
    SourceSpec {
        role: "event_codec_knowledge_verification_authority",
        path: "crates/event_codec/src/knowledge/verification.rs",
    },
    SourceSpec {
        role: "event_codec_operational_listing_tags_authority",
        path: "crates/event_codec/src/operational_listing/tags.rs",
    },
    SourceSpec {
        role: "event_codec_order_decode_authority",
        path: "crates/event_codec/src/order/decode.rs",
    },
    SourceSpec {
        role: "event_codec_profile_facade",
        path: "crates/event_codec/src/profile/mod.rs",
    },
    SourceSpec {
        role: "event_codec_tag_builders_authority",
        path: "crates/event_codec/src/tag_builders.rs",
    },
    SourceSpec {
        role: "event_codec_trade_facade",
        path: "crates/event_codec/src/trade/mod.rs",
    },
    SourceSpec {
        role: "event_store_dependency_authority",
        path: "crates/event_store/Cargo.toml",
    },
    SourceSpec {
        role: "event_store_error_and_limits",
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
        role: "predecessor_model_public_surface",
        path: "crates/event_store/src/model.rs",
    },
    SourceSpec {
        role: "event_store_reconciliation_v1_model",
        path: "crates/event_store/src/model/reconciliation_v1.rs",
    },
    SourceSpec {
        role: "event_store_reconciliation_v1_ingest_model",
        path: "crates/event_store/src/model/ingest_reconciliation_v1.rs",
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
        role: "source_generation_rebuild_authority",
        path: "crates/event_store/src/nip09/reconciliation_v1.rs",
    },
    SourceSpec {
        role: "nip09_successor_result_vector_executor",
        path: NIP09_SUCCESSOR_RESULT_VECTOR_EXECUTOR_RELATIVE,
    },
    SourceSpec {
        role: "schema_migration_and_reopen_authority",
        path: "crates/event_store/src/schema.rs",
    },
    SourceSpec {
        role: "public_store_and_transaction_authority",
        path: "crates/event_store/src/store.rs",
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
        role: "predecessor_post_core_v1_extension",
        path: "crates/event_store/src/store/post_core_extensions_v1.rs",
    },
    SourceSpec {
        role: "predecessor_post_core_v1_storage",
        path: "crates/event_store/src/store/post_core_storage_v1.rs",
    },
    SourceSpec {
        role: "raw_ingest_capacity_authority",
        path: "crates/event_store/src/store/protocol_reconciliation_v1.rs",
    },
    SourceSpec {
        role: "predecessor_protocol_storage",
        path: "crates/event_store/src/store/protocol_storage_v1.rs",
    },
    SourceSpec {
        role: "source_maintenance_runtime",
        path: "crates/event_store/src/source_maintenance_v1.rs",
    },
    SourceSpec {
        role: "predecessor_food_result_vector_executor",
        path: FOOD_PREDECESSOR_RESULT_VECTOR_EXECUTOR_RELATIVE,
    },
    SourceSpec {
        role: "artifact_transaction_authority",
        path: "tools/xtask/src/contract/artifact_bundle.rs",
    },
    SourceSpec {
        role: "predecessor_successor_governance",
        path: "tools/xtask/src/contract/food_availability_projection.rs",
    },
    SourceSpec {
        role: "transitive_predecessor_membership_governance",
        path: "tools/xtask/src/contract/nip09_reconciliation.rs",
    },
    SourceSpec {
        role: "source_maintenance_governance",
        path: "tools/xtask/src/contract/source_maintenance.rs",
    },
    SourceSpec {
        role: "contract_command_authority",
        path: "tools/xtask/src/contract.rs",
    },
    SourceSpec {
        role: "dto_root_generation_authority",
        path: "tools/xtask/src/dto_roots.rs",
    },
    SourceSpec {
        role: "xtask_dispatch_and_release_preflight",
        path: "tools/xtask/src/main.rs",
    },
];

#[cfg(test)]
pub(super) fn source_contract_fixture_source_paths() -> Vec<&'static str> {
    SOURCE_SPECS.iter().map(|source| source.path).collect()
}

const PREDECESSOR_SUPERSEDED_SOURCE_PATHS: &[&str] = &[
    "Cargo.toml",
    "crates/blossom/src/authorization.rs",
    "crates/blossom/src/descriptor.rs",
    "crates/blossom/src/error.rs",
    "crates/blossom/src/hash.rs",
    "crates/blossom/src/lib.rs",
    "crates/blossom/src/url.rs",
    "crates/core/src/currency.rs",
    "crates/core/src/decimal.rs",
    "crates/core/src/money.rs",
    "crates/core/src/percent.rs",
    "crates/core/src/quantity.rs",
    "crates/core/src/quantity_price.rs",
    "crates/core/src/unit.rs",
    "crates/event/src/lib.rs",
    "crates/event/src/calendar.rs",
    "crates/event/src/classified_listing.rs",
    "crates/event/src/comment.rs",
    "crates/event/src/contract.rs",
    "crates/event/src/contract/registry_v7.rs",
    "crates/event/src/deletion.rs",
    "crates/event/src/dto.rs",
    "crates/event/src/draft.rs",
    "crates/event/src/envelope.rs",
    "crates/event/src/event_head.rs",
    "crates/event/src/event_head/v1.rs",
    "crates/event/src/farm_crdt.rs",
    "crates/event/src/food_availability.rs",
    "crates/event/src/ids.rs",
    "crates/event/src/knowledge.rs",
    "crates/event/src/kinds.rs",
    "crates/event/src/media.rs",
    "crates/event/src/operational_listing.rs",
    "crates/event/src/order.rs",
    "crates/event/src/post.rs",
    "crates/event/src/profile.rs",
    "crates/event/src/relay_hint.rs",
    "crates/event/src/reply.rs",
    "crates/event/src/social.rs",
    "crates/event/src/tags.rs",
    "crates/event/src/trade.rs",
    "crates/event/src/trade_validation.rs",
    "crates/event/src/wire.rs",
    "crates/event/src/wire/v1.rs",
    "crates/event_codec/src/admission.rs",
    "crates/event_codec/src/admission/registry_v7.rs",
    "crates/event_codec/src/comment/inbound.rs",
    "crates/event_codec/src/comment/inbound/registry_v7.rs",
    "crates/event_codec/src/deletion/mod.rs",
    "crates/event_codec/src/deletion/reconciliation_v1.rs",
    "crates/event_codec/src/error.rs",
    "crates/event_codec/src/food_availability/admission.rs",
    "crates/event_codec/src/food_availability/authored.rs",
    "crates/event_codec/src/food_availability/inbound.rs",
    "crates/event_codec/src/food_availability/inbound/registry_v7.rs",
    "crates/event_codec/src/job/encode.rs",
    "crates/event_codec/src/job/traits.rs",
    "crates/event_codec/src/knowledge/verification.rs",
    "crates/event_codec/src/lib.rs",
    "crates/event_codec/src/operational_listing/tags.rs",
    "crates/event_codec/src/order/decode.rs",
    "crates/event_codec/src/post/inbound.rs",
    "crates/event_codec/src/post/inbound/registry_v7.rs",
    "crates/event_codec/src/profile/mod.rs",
    "crates/event_codec/src/profile/inbound.rs",
    "crates/event_codec/src/profile/inbound/registry_v7.rs",
    "crates/event_codec/src/reply/inbound.rs",
    "crates/event_codec/src/reply/inbound/registry_v7.rs",
    "crates/event_codec/src/tag_builders.rs",
    "crates/event_codec/src/trade/mod.rs",
    "crates/event_codec/src/verification.rs",
    "crates/event_codec/src/verification/v1.rs",
    "crates/event_store/Cargo.toml",
    "crates/event_store/src/error.rs",
    "crates/event_store/src/generated.rs",
    "crates/event_store/src/lib.rs",
    "crates/event_store/src/migrations.rs",
    "crates/event_store/src/model.rs",
    "crates/event_store/src/model/addressable_transition_feed_v1.rs",
    "crates/event_store/src/model/current_visibility_v1.rs",
    "crates/event_store/src/model/food_availability_projection_v1.rs",
    "crates/event_store/src/model/ingest_reconciliation_v1.rs",
    "crates/event_store/src/model/reconciliation_v1.rs",
    "crates/event_store/src/nip09/reconciliation_v1.rs",
    "crates/event_store/src/schema.rs",
    "crates/event_store/src/store.rs",
    "crates/event_store/src/store/addressable_transition_feed_v1.rs",
    "crates/event_store/src/store/current_visibility_v1.rs",
    "crates/event_store/src/store/food_availability_projection_v1.rs",
    "crates/event_store/src/store/post_core_extensions_v1.rs",
    "crates/event_store/src/store/post_core_storage_v1.rs",
    "crates/event_store/src/store/protocol_reconciliation_v1.rs",
    "crates/event_store/src/store/protocol_storage_v1.rs",
];

const PREDECESSOR_SUPERSESSION_REPLACEMENTS: &[(&str, &str)] =
    &[("crates/event/src/ids.rs", "crates/event/src/id.rs")];

const PREDECESSOR_SUPERSEDED_ARTIFACT_PATHS: &[&str] =
    &[FOOD_PREDECESSOR_RESULT_VECTOR_EXECUTOR_RELATIVE];

#[cfg(test)]
pub(super) fn predecessor_superseded_source_paths() -> &'static [&'static str] {
    PREDECESSOR_SUPERSEDED_SOURCE_PATHS
}

const GENERATED_ARTIFACT_PATHS: &[&str] = &[
    MANIFEST_RELATIVE,
    MANIFEST_SCHEMA_RELATIVE,
    MANIFEST_SHA256_RELATIVE,
    GENERATED_DESCRIPTOR_RELATIVE,
    RESULT_VECTOR_MIRROR_RELATIVE,
];

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct SourceMaintenanceManifest {
    schema_version: u32,
    contract_id: String,
    hook_id: String,
    manifest_schema: FileDescriptor,
    predecessor: PredecessorDescriptor,
    migration: MigrationDescriptor,
    source_maintenance: SourceMaintenanceDescriptor,
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
    replaced_objects: Vec<String>,
    tables: Vec<String>,
    fts5_tables: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct SourceMaintenanceDescriptor {
    version: u32,
    event_contract_registry_version: u32,
    capacity_authority_id: String,
    accounting: AccountingDescriptor,
    limits: LimitDescriptor,
    reopen_validation: ReopenValidationDescriptor,
    rebuild_seal: RebuildSealDescriptor,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct AccountingDescriptor {
    algorithm: String,
    raw_event_columns: Vec<String>,
    raw_tag_columns: Vec<String>,
    nullable_raw_tag_columns: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct LimitDescriptor {
    raw_events: u64,
    raw_tags: u64,
    raw_event_text_bytes: u64,
    raw_tag_text_bytes: u64,
    retained_source_generations: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ReopenValidationDescriptor {
    mode: String,
    raw_event_rejection_scan_bound: u64,
    raw_tag_rejection_scan_bound: u64,
    generation_history_validation: String,
    retained_generation_rejection_scan_bound: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct RebuildSealDescriptor {
    nip09_hook_id: String,
    nip09_manifest_sha256: String,
    food_hook_id: String,
    food_manifest_sha256: String,
    food_scope_fingerprint_sha256: String,
    active_generation_authority: String,
    marker_close_authority: String,
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
struct PublicApiDescriptor {
    inherited_predecessor_symbols: Vec<String>,
    added_symbols: Vec<String>,
    methods: Vec<String>,
    error_variants: Vec<String>,
    removed_symbols: Vec<String>,
    breaking_replacements: Vec<PublicApiReplacementDescriptor>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct PublicApiReplacementDescriptor {
    removed: String,
    replacement: String,
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
struct SourceMaintenanceVector {
    schema_version: u32,
    contract_id: String,
    capacity_version: u32,
    limits: LimitDescriptor,
    accounting: AccountingDescriptor,
    cases: Vec<VectorCase>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct VectorCase {
    id: String,
    execution: String,
    authority: String,
    authority_path: String,
    resource: Option<String>,
    boundary: Option<String>,
    expected_outcome: String,
    error_domain: Option<String>,
    expected_error: Option<String>,
}

pub(crate) fn write_source_maintenance_manifest(workspace_root: &Path) -> Result<(), String> {
    with_artifact_bundle_transaction(workspace_root, |transaction| {
        let artifacts = expected_artifacts(workspace_root)?;
        transaction.write(artifacts)?;
        validate_source_maintenance_manifest_under_lock(workspace_root)
    })
}

pub(crate) fn validate_source_maintenance_manifest(workspace_root: &Path) -> Result<(), String> {
    with_artifact_bundle_transaction(workspace_root, |_| {
        validate_source_maintenance_manifest_under_lock(workspace_root)
    })
}

pub(super) fn validate_source_maintenance_manifest_under_lock(
    workspace_root: &Path,
) -> Result<(), String> {
    let expected = expected_artifacts(workspace_root)?;
    for artifact in expected {
        let actual = read_regular_file(workspace_root, artifact.relative)?;
        if actual != artifact.contents {
            return Err(stale_error(artifact.relative));
        }
    }

    let manifest_bytes = read_regular_file(workspace_root, MANIFEST_RELATIVE)?;
    let manifest_value: Value = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| format!("parse {MANIFEST_RELATIVE}: {error}"))?;
    let manifest: SourceMaintenanceManifest = serde_json::from_value(manifest_value.clone())
        .map_err(|error| format!("parse typed {MANIFEST_RELATIVE}: {error}"))?;
    validate_canonical_json(MANIFEST_RELATIVE, &manifest_bytes, &manifest)?;
    validate_manifest_shape(&manifest)?;

    let schema_bytes = read_regular_file(workspace_root, MANIFEST_SCHEMA_RELATIVE)?;
    let schema: Value = serde_json::from_slice(&schema_bytes)
        .map_err(|error| format!("parse {MANIFEST_SCHEMA_RELATIVE}: {error}"))?;
    validate_canonical_json(MANIFEST_SCHEMA_RELATIVE, &schema_bytes, &schema)?;
    validate_manifest_json_schema(&schema, &manifest_value)?;

    let digest = read_regular_file(workspace_root, MANIFEST_SHA256_RELATIVE)?;
    validate_digest_sidecar(MANIFEST_SHA256_RELATIVE, &digest)?;
    if digest != format!("{}\n", sha256_hex(&manifest_bytes)).as_bytes() {
        return Err(format!(
            "{MANIFEST_SHA256_RELATIVE} must match the checked-in manifest bytes"
        ));
    }

    let vector_bytes = read_regular_file(workspace_root, RESULT_VECTOR_CANONICAL_RELATIVE)?;
    let mirror_bytes = read_regular_file(workspace_root, RESULT_VECTOR_MIRROR_RELATIVE)?;
    if vector_bytes != mirror_bytes {
        return Err(format!(
            "{RESULT_VECTOR_MIRROR_RELATIVE} must exactly mirror {RESULT_VECTOR_CANONICAL_RELATIVE}"
        ));
    }
    let vector: SourceMaintenanceVector = serde_json::from_slice(&vector_bytes)
        .map_err(|error| format!("parse {RESULT_VECTOR_CANONICAL_RELATIVE}: {error}"))?;
    validate_canonical_json(RESULT_VECTOR_CANONICAL_RELATIVE, &vector_bytes, &vector)?;
    validate_result_vector(workspace_root, &vector)?;
    Ok(())
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
) -> Result<SourceMaintenanceManifest, String> {
    validate_source_contract(workspace_root)?;
    validate_predecessor_production_source_coverage(workspace_root)?;

    let predecessor_bytes = read_regular_file(workspace_root, PREDECESSOR_MANIFEST_RELATIVE)?;
    if predecessor_bytes.len() != PREDECESSOR_MANIFEST_BYTE_LENGTH
        || sha256_hex(&predecessor_bytes) != PREDECESSOR_MANIFEST_SHA256
    {
        return Err(format!(
            "{PREDECESSOR_MANIFEST_RELATIVE} does not match the immutable predecessor identity"
        ));
    }
    validate_predecessor_public_api(&predecessor_bytes)?;

    let vector_bytes = read_regular_file(workspace_root, RESULT_VECTOR_CANONICAL_RELATIVE)?;
    let vector: SourceMaintenanceVector = serde_json::from_slice(&vector_bytes)
        .map_err(|error| format!("parse {RESULT_VECTOR_CANONICAL_RELATIVE}: {error}"))?;
    validate_canonical_json(RESULT_VECTOR_CANONICAL_RELATIVE, &vector_bytes, &vector)?;
    validate_result_vector(workspace_root, &vector)?;

    let migration_source = read_regular_file(workspace_root, MIGRATIONS_SOURCE_RELATIVE)?;
    let catalog = catalog_from_migration_source(&migration_source)?;
    validate_catalog(&catalog)?;
    let executor = descriptor_for_file(workspace_root, RESULT_VECTOR_EXECUTOR_RELATIVE)?;
    let migration_up = descriptor_for_file(workspace_root, MIGRATION_UP_RELATIVE)?;
    let migration_down = descriptor_for_file(workspace_root, MIGRATION_DOWN_RELATIVE)?;
    validate_migration_identity(&migration_up, &migration_down)?;

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

    Ok(SourceMaintenanceManifest {
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
            up: migration_up,
            down: migration_down,
            schema_sha256: SCHEMA_SHA256.to_owned(),
            catalog,
        },
        source_maintenance: SourceMaintenanceDescriptor {
            version: CAPACITY_VERSION,
            event_contract_registry_version: EVENT_CONTRACT_REGISTRY_VERSION,
            capacity_authority_id: CAPACITY_AUTHORITY_ID.to_owned(),
            accounting: AccountingDescriptor {
                algorithm: ACCOUNTING_ALGORITHM.to_owned(),
                raw_event_columns: owned(RAW_EVENT_COLUMNS),
                raw_tag_columns: owned(RAW_TAG_COLUMNS),
                nullable_raw_tag_columns: owned(NULLABLE_RAW_TAG_COLUMNS),
            },
            limits: expected_limits(),
            reopen_validation: ReopenValidationDescriptor {
                mode: REOPEN_VALIDATION_MODE.to_owned(),
                raw_event_rejection_scan_bound: RAW_EVENT_REJECTION_SCAN_BOUND,
                raw_tag_rejection_scan_bound: RAW_TAG_REJECTION_SCAN_BOUND,
                generation_history_validation: GENERATION_HISTORY_VALIDATION.to_owned(),
                retained_generation_rejection_scan_bound: RETAINED_GENERATION_REJECTION_SCAN_BOUND,
            },
            rebuild_seal: RebuildSealDescriptor {
                nip09_hook_id: NIP09_HOOK_ID.to_owned(),
                nip09_manifest_sha256: NIP09_MANIFEST_SHA256.to_owned(),
                food_hook_id: PREDECESSOR_HOOK_ID.to_owned(),
                food_manifest_sha256: PREDECESSOR_MANIFEST_SHA256.to_owned(),
                food_scope_fingerprint_sha256: FOOD_SCOPE_FINGERPRINT_SHA256.to_owned(),
                active_generation_authority: ACTIVE_GENERATION_AUTHORITY.to_owned(),
                marker_close_authority: MARKER_CLOSE_AUTHORITY.to_owned(),
            },
        },
        entry_points: ENTRY_POINTS
            .iter()
            .map(|(role, rust_path)| EntryPointDescriptor {
                role: (*role).to_owned(),
                rust_path: (*rust_path).to_owned(),
            })
            .collect(),
        source_files,
        public_api: expected_public_api(),
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

fn expected_limits() -> LimitDescriptor {
    LimitDescriptor {
        raw_events: RAW_EVENT_COUNT_LIMIT,
        raw_tags: RAW_TAG_COUNT_LIMIT,
        raw_event_text_bytes: RAW_EVENT_TEXT_BYTES_LIMIT,
        raw_tag_text_bytes: RAW_TAG_TEXT_BYTES_LIMIT,
        retained_source_generations: RETAINED_SOURCE_GENERATION_LIMIT,
    }
}

fn expected_public_api() -> PublicApiDescriptor {
    PublicApiDescriptor {
        inherited_predecessor_symbols: owned(INHERITED_PUBLIC_API),
        added_symbols: owned(ADDED_PUBLIC_API),
        methods: owned(PUBLIC_METHODS),
        error_variants: owned(ERROR_VARIANTS),
        removed_symbols: owned(REMOVED_PUBLIC_API),
        breaking_replacements: BREAKING_PUBLIC_API_REPLACEMENTS
            .iter()
            .map(|(removed, replacement)| PublicApiReplacementDescriptor {
                removed: (*removed).to_owned(),
                replacement: (*replacement).to_owned(),
            })
            .collect(),
    }
}

fn owned(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

fn validate_predecessor_production_source_coverage(workspace_root: &Path) -> Result<(), String> {
    let source_paths = SOURCE_SPECS
        .iter()
        .map(|source| source.path)
        .collect::<Vec<_>>();
    let unique_source_paths = source_paths.iter().copied().collect::<BTreeSet<_>>();
    if unique_source_paths.len() != source_paths.len() {
        return Err("SourceMaintenance SOURCE_SPECS paths must be unique".to_owned());
    }
    let replaced_predecessors = PREDECESSOR_SUPERSESSION_REPLACEMENTS
        .iter()
        .map(|(predecessor, _)| *predecessor)
        .collect::<BTreeSet<_>>();
    let replacement_paths = PREDECESSOR_SUPERSESSION_REPLACEMENTS
        .iter()
        .map(|(_, replacement)| *replacement)
        .collect::<BTreeSet<_>>();
    if replaced_predecessors.len() != PREDECESSOR_SUPERSESSION_REPLACEMENTS.len()
        || replacement_paths.len() != PREDECESSOR_SUPERSESSION_REPLACEMENTS.len()
    {
        return Err(
            "SourceMaintenance predecessor supersession replacements must be one-to-one".to_owned(),
        );
    }
    for (predecessor, replacement) in PREDECESSOR_SUPERSESSION_REPLACEMENTS {
        if predecessor == replacement || !PREDECESSOR_SUPERSEDED_SOURCE_PATHS.contains(predecessor)
        {
            return Err(format!(
                "SourceMaintenance replacement `{predecessor}` -> `{replacement}` must rename an explicitly superseded predecessor"
            ));
        }
        if workspace_root.join(predecessor).exists() {
            return Err(format!(
                "SourceMaintenance renamed predecessor path `{predecessor}` must be absent"
            ));
        }
    }
    for path in PREDECESSOR_SUPERSEDED_SOURCE_PATHS {
        let current_path = PREDECESSOR_SUPERSESSION_REPLACEMENTS
            .iter()
            .find_map(|(predecessor, replacement)| (*predecessor == *path).then_some(*replacement))
            .unwrap_or(path);
        let count = source_paths
            .iter()
            .filter(|candidate| **candidate == current_path)
            .count();
        if count != 1 {
            return Err(format!(
                "SourceMaintenance successor must current-byte-bind superseded predecessor path `{path}` through `{current_path}` exactly once; found {count}"
            ));
        }
    }
    let superseded = PREDECESSOR_SUPERSEDED_SOURCE_PATHS
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if superseded.len() != PREDECESSOR_SUPERSEDED_SOURCE_PATHS.len() {
        return Err("SourceMaintenance predecessor supersession paths must be unique".to_owned());
    }
    let superseded_artifacts = PREDECESSOR_SUPERSEDED_ARTIFACT_PATHS
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if superseded_artifacts.len() != PREDECESSOR_SUPERSEDED_ARTIFACT_PATHS.len() {
        return Err(
            "SourceMaintenance predecessor artifact supersession paths must be unique".to_owned(),
        );
    }
    for path in PREDECESSOR_SUPERSEDED_ARTIFACT_PATHS {
        let count = source_paths
            .iter()
            .filter(|candidate| **candidate == *path)
            .count();
        if count != 1 {
            return Err(format!(
                "SourceMaintenance successor must current-byte-bind superseded predecessor artifact `{path}` exactly once; found {count}"
            ));
        }
    }
    validate_food_availability_projection_predecessor_production_sources_under_lock(
        workspace_root,
        PREDECESSOR_SUPERSEDED_SOURCE_PATHS,
        PREDECESSOR_SUPERSEDED_ARTIFACT_PATHS,
    )
}

fn validate_predecessor_public_api(predecessor_bytes: &[u8]) -> Result<(), String> {
    let predecessor: Value = serde_json::from_slice(predecessor_bytes)
        .map_err(|error| format!("parse {PREDECESSOR_MANIFEST_RELATIVE}: {error}"))?;
    let actual = predecessor
        .pointer("/public_api")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{PREDECESSOR_MANIFEST_RELATIVE} has no public_api array"))?
        .iter()
        .map(|value| {
            value.as_str().map(str::to_owned).ok_or_else(|| {
                format!("{PREDECESSOR_MANIFEST_RELATIVE} public_api values must be strings")
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    if actual != owned(INHERITED_PUBLIC_API) {
        return Err(
            "SourceMaintenance inherited public API must exactly equal the immutable FoodAvailability public API"
                .to_owned(),
        );
    }
    Ok(())
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

fn byte_length(relative: &str, bytes: &[u8]) -> Result<u64, String> {
    u64::try_from(bytes.len()).map_err(|_| format!("{relative} byte length does not fit u64"))
}

fn catalog_from_migration_source(bytes: &[u8]) -> Result<CatalogDescriptor, String> {
    let source = std::str::from_utf8(bytes)
        .map_err(|error| format!("{MIGRATIONS_SOURCE_RELATIVE} must be UTF-8: {error}"))?;
    let syntax = syn::parse_file(source)
        .map_err(|error| format!("parse {MIGRATIONS_SOURCE_RELATIVE}: {error}"))?;
    Ok(CatalogDescriptor {
        objects: extract_string_array_const(
            &syntax,
            "EVENT_STORE_SOURCE_MAINTENANCE_OBJECT_NAMES",
        )?,
        replaced_objects: extract_string_array_const(
            &syntax,
            "EVENT_STORE_SOURCE_MAINTENANCE_REPLACED_OBJECT_NAMES",
        )?,
        tables: extract_string_array_const(&syntax, "EVENT_STORE_SOURCE_MAINTENANCE_TABLE_NAMES")?,
        fts5_tables: Vec::new(),
    })
}

fn extract_string_array_const(syntax: &syn::File, name: &str) -> Result<Vec<String>, String> {
    let expression = syntax
        .items
        .iter()
        .find_map(|item| match item {
            Item::Const(item) if item.ident == name => Some(item.expr.as_ref()),
            _ => None,
        })
        .ok_or_else(|| format!("{MIGRATIONS_SOURCE_RELATIVE} must define `{name}`"))?;
    let Expr::Array(array) = strip_expression_wrappers(expression) else {
        return Err(format!(
            "{MIGRATIONS_SOURCE_RELATIVE} `{name}` must be a literal array"
        ));
    };
    array
        .elems
        .iter()
        .map(|element| match strip_expression_wrappers(element) {
            Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(value),
                ..
            }) => Ok(value.value()),
            _ => Err(format!(
                "{MIGRATIONS_SOURCE_RELATIVE} `{name}` values must be string literals"
            )),
        })
        .collect()
}

fn strip_expression_wrappers(mut expression: &Expr) -> &Expr {
    loop {
        match expression {
            Expr::Reference(reference) => expression = &reference.expr,
            Expr::Group(group) => expression = &group.expr,
            Expr::Paren(paren) => expression = &paren.expr,
            _ => return expression,
        }
    }
}

fn validate_catalog(catalog: &CatalogDescriptor) -> Result<(), String> {
    if catalog.objects != owned(EXPECTED_CATALOG_OBJECTS)
        || catalog.replaced_objects != owned(EXPECTED_REPLACED_CATALOG_OBJECTS)
        || catalog.tables != owned(EXPECTED_CATALOG_TABLES)
        || !catalog.fts5_tables.is_empty()
    {
        return Err(format!(
            "SourceMaintenance migration catalog differs: expected objects {:?}, replacements {:?}, tables {:?}, no FTS5; found {catalog:?}",
            EXPECTED_CATALOG_OBJECTS, EXPECTED_REPLACED_CATALOG_OBJECTS, EXPECTED_CATALOG_TABLES,
        ));
    }
    validate_unique(
        "SourceMaintenance catalog objects",
        catalog.objects.iter().map(String::as_str),
    )?;
    validate_unique(
        "SourceMaintenance replaced catalog objects",
        catalog.replaced_objects.iter().map(String::as_str),
    )?;
    validate_unique(
        "SourceMaintenance catalog tables",
        catalog.tables.iter().map(String::as_str),
    )?;
    if catalog
        .replaced_objects
        .iter()
        .any(|name| catalog.objects.contains(name) || catalog.tables.contains(name))
    {
        return Err(
            "SourceMaintenance replaced catalog objects must be disjoint from newly owned objects and tables"
                .to_owned(),
        );
    }
    Ok(())
}

fn validate_migration_identity(up: &FileDescriptor, down: &FileDescriptor) -> Result<(), String> {
    const UP_BYTE_LENGTH: u64 = 19_841;
    const UP_SHA256: &str = "ab2724188f8d08c897eebea2533a635e7c74282a25e84e4c0c37e78b08837a43";
    const DOWN_BYTE_LENGTH: u64 = 5_172;
    const DOWN_SHA256: &str = "fe44fd53c51545c08ea479b385e6781079dab70fc63da2a3c205d727a00ce860";
    if up.path != MIGRATION_UP_RELATIVE
        || up.byte_length != UP_BYTE_LENGTH
        || up.sha256 != UP_SHA256
        || down.path != MIGRATION_DOWN_RELATIVE
        || down.byte_length != DOWN_BYTE_LENGTH
        || down.sha256 != DOWN_SHA256
    {
        return Err(
            "SourceMaintenance migration bytes do not match the reviewed v4 identity".to_owned(),
        );
    }
    Ok(())
}

pub(super) fn validate_source_contract(workspace_root: &Path) -> Result<(), String> {
    validate_source_inventory()?;
    let migration_up = descriptor_for_file(workspace_root, MIGRATION_UP_RELATIVE)?;
    let migration_down = descriptor_for_file(workspace_root, MIGRATION_DOWN_RELATIVE)?;
    validate_migration_identity(&migration_up, &migration_down)?;
    validate_public_api_authority(workspace_root)?;
    validate_error_and_limit_authority(workspace_root)?;
    validate_migration_registry_authority(workspace_root)?;
    validate_capacity_runtime_authority(workspace_root)?;
    validate_ingest_capacity_authority(workspace_root)?;
    validate_schema_capacity_authority(workspace_root)?;
    validate_generation_rebuild_authority(workspace_root)?;
    validate_nip09_successor_result_vector_executor(workspace_root)?;
    validate_sql_capacity_authority(workspace_root)?;
    validate_contract_command_reachability_authority(workspace_root)?;
    validate_current_event_store_successor_authority(workspace_root)
}

fn validate_contract_command_reachability_authority(workspace_root: &Path) -> Result<(), String> {
    let contract = rust_source(workspace_root, CONTRACT_COMMAND_SOURCE_RELATIVE)?;
    let main = rust_source(workspace_root, XTASK_MAIN_SOURCE_RELATIVE)?;
    validate_contract_command_reachability_sources(&contract, &main)
}

fn validate_contract_command_reachability_sources(
    contract_source: &str,
    main_source: &str,
) -> Result<(), String> {
    let contract = syn::parse_file(contract_source)
        .map_err(|error| format!("parse {CONTRACT_COMMAND_SOURCE_RELATIVE}: {error}"))?;
    let main = syn::parse_file(main_source)
        .map_err(|error| format!("parse {XTASK_MAIN_SOURCE_RELATIVE}: {error}"))?;
    let main_ast_sha256 = sha256_hex(compact_tokens(&main).as_bytes());
    if main_ast_sha256 != XTASK_MAIN_FULL_AST_SHA256 {
        return Err(format!(
            "{XTASK_MAIN_SOURCE_RELATIVE} full dispatch AST authority drifted: expected {XTASK_MAIN_FULL_AST_SHA256}, found {main_ast_sha256}"
        ));
    }
    for (relative, file, name, expected) in [
        (
            CONTRACT_COMMAND_SOURCE_RELATIVE,
            &contract,
            "validate_artifact_contracts",
            r#"pub(crate) fn validate_artifact_contracts(
                workspace_root: &Path
            ) -> Result<(), String> {
                validate_event_contract_registry_v7_inventory(workspace_root)?;
                validate_nip09_reconciliation_manifest(workspace_root)?;
                validate_source_maintenance_manifest(workspace_root)?;
                validate_knowledge_contract_manifest(workspace_root)
            }"#,
        ),
        (
            XTASK_MAIN_SOURCE_RELATIVE,
            &main,
            "validate_protocol_contracts",
            r#"fn validate_protocol_contracts() -> Result<(), String> {
                use radroots_protocol::{capability, event, runtime, schema};

                capability::v1::validate_catalog(capability::v1::CATALOG)
                    .map_err(|error| error.to_string())?;
                event::v1::validate_catalog(event::v1::CATALOG)
                    .map_err(|error| error.to_string())?;
                event::v1::validate_trade_state_vocabulary(event::v1::TRADE_STATE_VOCABULARY)
                    .map_err(|error| error.to_string())?;
                runtime::v1::validate_catalog(runtime::v1::CATALOG)
                    .map_err(|error| error.to_string())?;
                schema::protocol_v1_registry()
                    .map(|_| ())
                    .map_err(|error| error.to_string())
            }"#,
        ),
        (
            XTASK_MAIN_SOURCE_RELATIVE,
            &main,
            "validate_contract",
            r#"fn validate_contract() -> Result<(), String> {
                validate_protocol_contracts()?;
                let root = workspace_root();
                dto_roots::check(&root)?;
                generate::protocol::check(&root)?;
                contract::load_contract_bundle(&root)
                    .and_then(|bundle| contract::validate_contract_bundle(&bundle))
                    .and_then(|_| contract::validate_canonical_event_boundary(&root))
                    .and_then(|_| contract::validate_artifact_contracts(&root))
            }"#,
        ),
        (
            XTASK_MAIN_SOURCE_RELATIVE,
            &main,
            "release_preflight_at",
            r#"fn release_preflight_at(root: &Path) -> Result<(), String> {
                dto_roots::check(root)?;
                generate::protocol::check(root)?;
                contract::validate_artifact_contracts(root)?;
                contract::validate_release_preflight(root)
            }"#,
        ),
    ] {
        let actual = compact_tokens(exact_top_level_function(file, name)?);
        let expected_file = syn::parse_file(expected)
            .map_err(|error| format!("parse authoritative `{name}` function: {error}"))?;
        let expected = compact_tokens(exact_top_level_function(&expected_file, name)?);
        if actual != expected {
            return Err(format!(
                "{relative} `{name}` SourceMaintenance validation call-path authority drifted: expected `{expected}`, found `{actual}`"
            ));
        }
    }
    Ok(())
}

fn validate_source_inventory() -> Result<(), String> {
    validate_unique(
        "SourceMaintenance source roles",
        SOURCE_SPECS.iter().map(|spec| spec.role),
    )?;
    validate_unique(
        "SourceMaintenance source paths",
        SOURCE_SPECS.iter().map(|spec| spec.path),
    )?;
    let source_paths = SOURCE_SPECS
        .iter()
        .map(|spec| spec.path)
        .collect::<BTreeSet<_>>();
    for path in GENERATED_ARTIFACT_PATHS {
        if source_paths.contains(path) {
            return Err(format!(
                "SourceMaintenance generated artifact `{path}` must not participate in its own source hash graph"
            ));
        }
    }
    if source_paths.contains(RESULT_VECTOR_MIRROR_RELATIVE)
        || source_paths.contains(MANIFEST_RELATIVE)
        || source_paths.contains(MANIFEST_SCHEMA_RELATIVE)
    {
        return Err("SourceMaintenance source inventory contains a self-hashed output".to_owned());
    }
    Ok(())
}

fn validate_error_and_limit_authority(workspace_root: &Path) -> Result<(), String> {
    let source = rust_source(workspace_root, "crates/event_store/src/error.rs")?;
    validate_error_and_limit_source(&source)
}

fn validate_error_and_limit_source(source: &str) -> Result<(), String> {
    let syntax = syn::parse_file(source)
        .map_err(|error| format!("parse crates/event_store/src/error.rs: {error}"))?;
    let public_items = top_level_public_item_names(&syntax);
    for symbol in &ADDED_PUBLIC_API[..6] {
        if !public_items.contains(*symbol) {
            return Err(format!(
                "crates/event_store/src/error.rs must publicly define `{symbol}`"
            ));
        }
    }
    validate_source_capacity_resource_authority(&syntax)?;
    validate_source_capacity_limit_authority(&syntax)?;
    let error_enum = exact_top_level_enum(&syntax, "RadrootsEventStoreError")?;
    if error_enum
        .attrs
        .iter()
        .any(|attribute| attribute.path().is_ident("cfg") || attribute.path().is_ident("cfg_attr"))
    {
        return Err(
            "top-level enum `RadrootsEventStoreError` must not have conditional attributes"
                .to_owned(),
        );
    }
    let variants = error_enum
        .variants
        .iter()
        .map(|variant| variant.ident.to_string())
        .collect::<BTreeSet<_>>();
    for variant in ERROR_VARIANTS {
        if !variants.contains(*variant) {
            return Err(format!(
                "RadrootsEventStoreError must define SourceMaintenance variant `{variant}`"
            ));
        }
    }
    if variants.contains("ReconciliationCapacityExceeded") {
        return Err(
            "removed error variant `RadrootsEventStoreError::ReconciliationCapacityExceeded` must remain absent"
                .to_owned(),
        );
    }
    for (name, expected) in [
        (
            "SourceCapacityExceeded",
            r#"#[error(
                "event-store retained source {resource} capacity exceeded: current {current}, requested additional {requested}, limit {limit}; durable append refused, retain a bounded source set in a new disposable cache"
            )]
            SourceCapacityExceeded {
                resource: RadrootsEventStoreSourceCapacityResourceV1,
                current: u64,
                requested: u64,
                limit: u64,
            }"#,
        ),
        (
            "SourceGenerationHistoryLimitReached",
            r#"#[error(
                "event-store retained source generation limit reached: current {current}, limit {limit}; replace and resync into a fresh store"
            )]
            SourceGenerationHistoryLimitReached { current: u32, limit: u32 }"#,
        ),
        (
            "PersistedEphemeralRawEvent",
            r#"#[error(
                "event-store retained source contains ephemeral event `{event_id}` of kind {kind}; ephemeral events must be discarded"
            )]
            PersistedEphemeralRawEvent { event_id: String, kind: i64 }"#,
        ),
        (
            "SourceCapacityStateDrift",
            r#"#[error("event-store retained source capacity authority is inconsistent: {reason}")]
            SourceCapacityStateDrift { reason: String }"#,
        ),
        (
            "SqliteMainDatabaseEncodingNotUtf8",
            r#"#[error(
                "event-store SQLite main database must use UTF-8 encoding; reported `{actual}`"
            )]
            SqliteMainDatabaseEncodingNotUtf8 { actual: String }"#,
        ),
        (
            "RollbackWouldDiscardSourceGenerationHistory",
            r#"#[error(
                "event-store rollback from version {current} to {target} would discard retained source-generation history; minimum retained-history schema version is {floor}"
            )]
            RollbackWouldDiscardSourceGenerationHistory {
                current: u32,
                target: u32,
                floor: u32,
            }"#,
        ),
    ] {
        let actual = error_enum
            .variants
            .iter()
            .find(|variant| variant.ident == name)
            .ok_or_else(|| format!("RadrootsEventStoreError must define `{name}`"))?;
        let mut actual = actual.clone();
        actual
            .attrs
            .retain(|attribute| !attribute.path().is_ident("doc"));
        let expected = syn::parse_str::<syn::ItemEnum>(&format!("enum Expected {{ {expected} }}"))
            .map_err(|error| format!("parse authoritative `{name}` variant: {error}"))?;
        let expected = expected
            .variants
            .first()
            .expect("authoritative error enum contains one variant");
        if compact_tokens(&actual) != compact_tokens(expected) {
            return Err(format!(
                "RadrootsEventStoreError::{name} typed fields or display contract drifted"
            ));
        }
    }
    Ok(())
}

fn validate_source_capacity_limit_authority(file: &syn::File) -> Result<(), String> {
    for (name, expected) in [
        (
            "RADROOTS_EVENT_STORE_RAW_EVENT_COUNT_LIMIT_V1",
            "pub const RADROOTS_EVENT_STORE_RAW_EVENT_COUNT_LIMIT_V1: u64 = 25_000;",
        ),
        (
            "RADROOTS_EVENT_STORE_RAW_TAG_COUNT_LIMIT_V1",
            "pub const RADROOTS_EVENT_STORE_RAW_TAG_COUNT_LIMIT_V1: u64 = 250_000;",
        ),
        (
            "RADROOTS_EVENT_STORE_RAW_EVENT_TEXT_BYTES_LIMIT_V1",
            "pub const RADROOTS_EVENT_STORE_RAW_EVENT_TEXT_BYTES_LIMIT_V1: u64 = 64 * 1024 * 1024;",
        ),
        (
            "RADROOTS_EVENT_STORE_RAW_TAG_TEXT_BYTES_LIMIT_V1",
            "pub const RADROOTS_EVENT_STORE_RAW_TAG_TEXT_BYTES_LIMIT_V1: u64 = 32 * 1024 * 1024;",
        ),
        (
            "RADROOTS_EVENT_STORE_RETAINED_SOURCE_GENERATION_LIMIT_V1",
            "pub const RADROOTS_EVENT_STORE_RETAINED_SOURCE_GENERATION_LIMIT_V1: u32 = 8;",
        ),
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
                "event-store capacity limit authority must define top-level `{name}` exactly once; found {}",
                matches.len()
            ));
        };
        let mut actual = (*actual).clone();
        actual
            .attrs
            .retain(|attribute| !attribute.path().is_ident("doc"));
        let expected = syn::parse_str::<syn::ItemConst>(expected)
            .map_err(|error| format!("parse authoritative capacity limit `{name}`: {error}"))?;
        if compact_tokens(&actual) != compact_tokens(&expected) {
            return Err(format!(
                "event-store capacity limit authority `{name}` visibility, attributes, type, or value drifted"
            ));
        }
    }
    Ok(())
}

fn validate_source_capacity_resource_authority(file: &syn::File) -> Result<(), String> {
    let resources = file
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Enum(item) if item.ident == "RadrootsEventStoreSourceCapacityResourceV1" => {
                Some(item)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let [resource] = resources.as_slice() else {
        return Err(format!(
            "crates/event_store/src/error.rs must define `RadrootsEventStoreSourceCapacityResourceV1` exactly once; found {}",
            resources.len()
        ));
    };
    let mut resource = (*resource).clone();
    resource
        .attrs
        .retain(|attribute| !attribute.path().is_ident("doc"));
    for variant in &mut resource.variants {
        variant
            .attrs
            .retain(|attribute| !attribute.path().is_ident("doc"));
    }
    let expected = syn::parse_str::<syn::ItemEnum>(
        r#"#[derive(Clone, Copy, Debug, PartialEq, Eq)]
        #[non_exhaustive]
        pub enum RadrootsEventStoreSourceCapacityResourceV1 {
            RawEvents,
            RawTags,
            RawEventBytes,
            RawTagBytes,
        }"#,
    )
    .map_err(|error| format!("parse source-capacity resource authority: {error}"))?;
    if compact_tokens(&resource) != compact_tokens(&expected) {
        return Err(
            "RadrootsEventStoreSourceCapacityResourceV1 variants, visibility, or attributes drifted"
                .to_owned(),
        );
    }

    let inherent = exact_top_level_impl(file, None, "RadrootsEventStoreSourceCapacityResourceV1")?;
    let mut inherent = inherent.clone();
    strip_doc_attributes_from_impl(&mut inherent);
    let expected = syn::parse_str::<syn::ItemImpl>(
        r#"impl RadrootsEventStoreSourceCapacityResourceV1 {
            pub const fn as_str(self) -> &'static str {
                match self {
                    Self::RawEvents => "raw event count",
                    Self::RawTags => "raw tag count",
                    Self::RawEventBytes => "total retained raw-source event row text bytes",
                    Self::RawTagBytes => "total retained raw-source tag row text bytes",
                }
            }
        }"#,
    )
    .map_err(|error| format!("parse source-capacity label authority: {error}"))?;
    if compact_tokens(&inherent) != compact_tokens(&expected) {
        return Err(
            "RadrootsEventStoreSourceCapacityResourceV1::as_str label authority drifted".to_owned(),
        );
    }

    let display = exact_top_level_impl(
        file,
        Some("core::fmt::Display"),
        "RadrootsEventStoreSourceCapacityResourceV1",
    )?;
    let mut display = display.clone();
    strip_doc_attributes_from_impl(&mut display);
    let expected = syn::parse_str::<syn::ItemImpl>(
        r#"impl core::fmt::Display for RadrootsEventStoreSourceCapacityResourceV1 {
            fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                formatter.write_str(self.as_str())
            }
        }"#,
    )
    .map_err(|error| format!("parse source-capacity Display authority: {error}"))?;
    if compact_tokens(&display) != compact_tokens(&expected) {
        return Err(
            "RadrootsEventStoreSourceCapacityResourceV1 Display authority drifted".to_owned(),
        );
    }
    Ok(())
}

fn validate_source_capacity_snapshot_authority(file: &syn::File) -> Result<(), String> {
    let snapshots = file
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Struct(item) if item.ident == "RadrootsEventStoreSourceCapacityV1" => Some(item),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [snapshot] = snapshots.as_slice() else {
        return Err(format!(
            "crates/event_store/src/source_maintenance_v1.rs must define `RadrootsEventStoreSourceCapacityV1` exactly once; found {}",
            snapshots.len()
        ));
    };
    let mut snapshot = (*snapshot).clone();
    snapshot
        .attrs
        .retain(|attribute| !attribute.path().is_ident("doc"));
    for field in &mut snapshot.fields {
        field
            .attrs
            .retain(|attribute| !attribute.path().is_ident("doc"));
    }
    let expected = syn::parse_str::<syn::ItemStruct>(
        r#"#[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub struct RadrootsEventStoreSourceCapacityV1 {
            source_generation: RadrootsEventStoreSourceGeneration,
            capacity: ReconciliationCapacity,
            raw_high_water_seq: i64,
            retained_generation_count: u32,
            retained_generation_limit: u32,
        }"#,
    )
    .map_err(|error| format!("parse source-capacity snapshot authority: {error}"))?;
    if compact_tokens(&snapshot) != compact_tokens(&expected) {
        return Err(
            "RadrootsEventStoreSourceCapacityV1 derives, visibility, or private field authority drifted"
                .to_owned(),
        );
    }

    let inherent = exact_top_level_impl(file, None, "RadrootsEventStoreSourceCapacityV1")?;
    let mut inherent = inherent.clone();
    strip_doc_attributes_from_impl(&mut inherent);
    let expected = syn::parse_str::<syn::ItemImpl>(
        r#"impl RadrootsEventStoreSourceCapacityV1 {
            pub const fn source_generation(&self) -> RadrootsEventStoreSourceGeneration {
                self.source_generation
            }

            pub const fn raw_event_count(&self) -> u64 {
                self.capacity.raw_events
            }

            pub const fn raw_tag_count(&self) -> u64 {
                self.capacity.raw_tags
            }

            pub const fn raw_event_text_bytes(&self) -> u64 {
                self.capacity.raw_event_bytes
            }

            pub const fn raw_tag_text_bytes(&self) -> u64 {
                self.capacity.raw_tag_bytes
            }

            pub const fn raw_high_water_seq(&self) -> i64 {
                self.raw_high_water_seq
            }

            pub const fn retained_generation_count(&self) -> u32 {
                self.retained_generation_count
            }

            pub const fn retained_generation_limit(&self) -> u32 {
                self.retained_generation_limit
            }
        }"#,
    )
    .map_err(|error| format!("parse source-capacity snapshot accessor authority: {error}"))?;
    if compact_tokens(&inherent) != compact_tokens(&expected) {
        return Err(
            "RadrootsEventStoreSourceCapacityV1 public accessor authority drifted".to_owned(),
        );
    }
    Ok(())
}

fn exact_top_level_impl<'a>(
    file: &'a syn::File,
    trait_path: Option<&str>,
    self_type: &str,
) -> Result<&'a syn::ItemImpl, String> {
    let matches = file
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Impl(item)
                if compact_tokens(item.self_ty.as_ref()) == self_type
                    && item
                        .trait_
                        .as_ref()
                        .map(|(_, path, _)| compact_tokens(path))
                        .as_deref()
                        == trait_path =>
            {
                Some(item)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let [item] = matches.as_slice() else {
        return Err(format!(
            "governed Rust source must define impl `{}` for `{self_type}` exactly once; found {}",
            trait_path.unwrap_or("inherent"),
            matches.len()
        ));
    };
    Ok(item)
}

fn strip_doc_attributes_from_impl(item: &mut syn::ItemImpl) {
    item.attrs
        .retain(|attribute| !attribute.path().is_ident("doc"));
    for member in &mut item.items {
        if let syn::ImplItem::Fn(function) = member {
            function
                .attrs
                .retain(|attribute| !attribute.path().is_ident("doc"));
        }
    }
}

fn validate_migration_registry_authority(workspace_root: &Path) -> Result<(), String> {
    let source = rust_source(workspace_root, MIGRATIONS_SOURCE_RELATIVE)?;
    let compact = compact_rust(&source, MIGRATIONS_SOURCE_RELATIVE)?;
    for marker in [
        "pubconstRADROOTS_EVENT_STORE_SCHEMA_VERSION_CURRENT:u32=4",
        "SourceMaintenanceV1",
        "version:4",
        "name:\"source_maintenance\"",
        "up_len:source_maintenance_manifest::SOURCE_MAINTENANCE_MIGRATION_UP_BYTE_LENGTH",
        "down_len:source_maintenance_manifest::SOURCE_MAINTENANCE_MIGRATION_DOWN_BYTE_LENGTH",
        "up_sha256:source_maintenance_manifest::SOURCE_MAINTENANCE_MIGRATION_UP_SHA256",
        "down_sha256:source_maintenance_manifest::SOURCE_MAINTENANCE_MIGRATION_DOWN_SHA256",
        "schema_sha256:source_maintenance_manifest::SOURCE_MAINTENANCE_SCHEMA_SHA256",
        "replaced_object_names:EVENT_STORE_SOURCE_MAINTENANCE_REPLACED_OBJECT_NAMES",
        "hook_manifest_sha256:Some(source_maintenance_manifest::SOURCE_MAINTENANCE_MANIFEST_SHA256)",
        "event_contract_registry_version:Some(source_maintenance_manifest::SOURCE_MAINTENANCE_EVENT_CONTRACT_REGISTRY_VERSION,)",
    ] {
        require_marker("SourceMaintenance migration registry", &compact, marker)?;
    }
    let catalog = catalog_from_migration_source(source.as_bytes())?;
    validate_catalog(&catalog)
}

fn validate_capacity_runtime_authority(workspace_root: &Path) -> Result<(), String> {
    let relative = "crates/event_store/src/source_maintenance_v1.rs";
    let source = rust_source(workspace_root, relative)?;
    let syntax = syn::parse_file(&source).map_err(|error| format!("parse {relative}: {error}"))?;
    validate_source_capacity_snapshot_authority(&syntax)?;

    let full = exact_free_function_tokens(&syntax, "validate_source_capacity_authority_full_v1")?;
    require_ordered_markers(
        "full SourceMaintenance reopen authority",
        &full,
        &[
            "validate_source_capacity_authority_fast_v1(connection).await?",
            "measure_reconciliation_capacity_bounded(connection,ReconciliationCapacityLimits::production(),).await?",
            "validate_measured_capacity(measured)?",
            "validate_no_persisted_ephemeral_raw_rows_v1(connection).await?",
            "require_invariant(measured==persisted.capacity",
        ],
    )?;

    let fast = exact_free_function_tokens(&syntax, "validate_source_capacity_authority_fast_v1")?;
    require_marker(
        "bounded generation-history validation",
        &fast,
        "(SELECTCOUNT(*)FROM(SELECT1FROMradroots_event_store_source_generationLIMIT9))ASretained_generation_count",
    )?;
    require_marker(
        "active generation ordinal validation",
        &fast,
        "generation.generation_ordinal",
    )?;
    if fast.contains("fetch_all") || fast.contains("Vec<") {
        return Err(
            "fast SourceMaintenance validation must not materialize generation history".to_owned(),
        );
    }

    for function in [
        "raw_source_capacity_delta_v1",
        "preflight_unique_raw_source_append_v1",
        "advance_source_capacity_after_insert_v1",
        "apply_source_maintenance_hook_v1",
        "validate_source_capacity_authority_fast_v1",
        "validate_source_capacity_authority_full_v1",
        "validate_no_persisted_ephemeral_raw_rows_v1",
        "preflight_source_generation_append_v1",
        "bind_source_capacity_to_generation_v1",
    ] {
        exact_free_function_tokens(&syntax, function)?;
    }
    Ok(())
}

fn validate_ingest_capacity_authority(workspace_root: &Path) -> Result<(), String> {
    let relative = "crates/event_store/src/store/protocol_reconciliation_v1.rs";
    let source = rust_source(workspace_root, relative)?;
    let syntax = syn::parse_file(&source).map_err(|error| format!("parse {relative}: {error}"))?;
    let ingest = exact_free_function_tokens(&syntax, "ingest_event_protocol_reconciliation_v1")?;
    require_ordered_markers(
        "SourceMaintenance ingest authority",
        &ingest,
        &[
            "acquire_event_store_write_lock(tx).await?",
            "validate_source_raw_authority(tx).await?",
            "validate_source_capacity_authority_fast_v1(tx).await?",
            "ifkind_class==EventKindClass::Ephemeral",
            "SELECTEXISTS(SELECT1FROMevent_envelopesWHEREevent_id=?)",
            "raw_source_capacity_delta_v1(ingest,tags_json.as_str())?",
            "preflight_unique_raw_source_append_v1(tx,delta).await?",
            "insert_raw_event",
            "synchronize_after_insert",
            "advance_source_capacity_after_insert_v1(tx,capacity_delta,insert.seq).await?",
            "read_protocol_post_extension_authority_seal(tx).await?",
        ],
    )
}

pub(super) fn validate_schema_capacity_authority(workspace_root: &Path) -> Result<(), String> {
    let relative = "crates/event_store/src/schema.rs";
    let source = rust_source(workspace_root, relative)?;
    let syntax = syn::parse_file(&source).map_err(|error| format!("parse {relative}: {error}"))?;
    let outer = exact_free_function_tokens(
        &syntax,
        "migrate_event_store_schema_with_registry_and_generation_provider",
    )?;
    require_ordered_markers(
        "SourceMaintenance outer migration preflight",
        &outer,
        &[
            "inspect_event_store_schema_status_with_registry",
            "ifhas_pending_source_capacity_hook",
            "validate_event_store_temp_schema_with_registry",
            "validate_reconciliation_capacity",
            "ifhas_pending_source_maintenance_hook",
            "validate_no_persisted_ephemeral_raw_rows_v1",
            "begin_with(\"BEGINIMMEDIATE\")",
        ],
    )?;
    let inner = exact_free_function_tokens(&syntax, "migrate_schema_on_connection")?;
    require_ordered_markers(
        "SourceMaintenance in-transaction migration recheck",
        &inner,
        &[
            "EventStoreMigrationHook::SourceMaintenanceV1",
            "validate_reconciliation_capacity(connection,reconciliation_limits).await?",
            "validate_no_persisted_ephemeral_raw_rows_v1(connection).await?",
            "apply_migration_up(connection,registry,migration).await?",
            "apply_migration_hook",
            "validate_applied_migration_hooks",
            "insert_ledger_row",
        ],
    )?;
    let inspect = exact_free_function_tokens(&syntax, "inspect_schema_on_connection")?;
    require_ordered_markers(
        "managed-store reopen validation",
        &inspect,
        &[
            "validate_history_against_registry",
            "ifactual_schema_sha256!=expected.schema_sha256",
            "validate_applied_migration_hooks(connection,registry,current).await?",
            "RadrootsEventStoreSchemaStatus::Managed",
        ],
    )?;
    let hook = exact_free_function_tokens(&syntax, "validate_migration_hook_state")?;
    require_ordered_markers(
        "SourceMaintenance hook validation dispatch",
        &hook,
        &[
            "EventStoreMigrationHook::SourceMaintenanceV1",
            "validate_source_capacity_authority_full_v1(connection).await",
        ],
    )
}

fn validate_generation_rebuild_authority(workspace_root: &Path) -> Result<(), String> {
    let relative = "crates/event_store/src/nip09/reconciliation_v1.rs";
    let source = rust_source(workspace_root, relative)?;
    let syntax = syn::parse_file(&source).map_err(|error| format!("parse {relative}: {error}"))?;
    let rebuild = exact_free_function_tokens(&syntax, "apply_reconciliation_hook")?;
    require_ordered_markers(
        "SourceMaintenance source-generation rebuild authority",
        &rebuild,
        &[
            "preflight_source_generation_append_v1(connection).await?",
            "open_source_rebuild_marker",
            "append_source_generation",
            "bind_source_capacity_to_generation_v1",
            "apply_food_availability_projection_hook_v1",
            "close_source_rebuild_marker",
            "validate_sqlite_integrity_after_rebuild",
            "validate_active_hook_state_fast",
        ],
    )?;

    let measure = exact_free_function_tokens(&syntax, "measure_reconciliation_capacity_bounded")?;
    require_ordered_markers(
        "SourceMaintenance bounded raw-source recount",
        &measure,
        &[
            "bounded_capacity_page_len(capacity.raw_events,limits.raw_events)",
            ".bind(page_size)",
            "ifrow_count<page_len",
            "bounded_capacity_page_len(capacity.raw_tags,limits.raw_tags)",
            ".bind(page_size)",
            "ifrow_count<page_len",
        ],
    )?;
    let page_len = exact_free_function_tokens(&syntax, "bounded_capacity_page_len")?;
    require_ordered_markers(
        "SourceMaintenance rejection-probe page bound",
        &page_len,
        &[
            "limit.saturating_sub(current)",
            ".saturating_add(1)",
            ".min(RECONCILIATION_SNAPSHOT_BATCH_COUNT)",
            "i64::try_from(page_count).unwrap_or(RECONCILIATION_SNAPSHOT_BATCH_SIZE)",
            "usize::try_from(page_count).unwrap_or(RECONCILIATION_SNAPSHOT_BATCH_LEN)",
        ],
    )
}

fn validate_nip09_successor_result_vector_executor(workspace_root: &Path) -> Result<(), String> {
    let module_relative = "crates/event_store/src/nip09/reconciliation_v1.rs";
    let module = compact_rust(
        &rust_source(workspace_root, module_relative)?,
        module_relative,
    )?;
    require_ordered_markers(
        "SourceMaintenance NIP-09 successor module selection",
        &module,
        &[
            "#[path=\"../../tests/support/nip09_reconciliation_v1_result_vector_v2.rs\"]modresult_vector_executor;",
            "include_bytes!(\"../../tests/fixtures/nip09_reconciliation.v1.json\")",
            "include_str!(\"../../migrations/0001_event_store.up.sql\")",
            "include_str!(\"../../migrations/0002_nip09.up.sql\")",
            "implSourceGenerationProviderforFixedResultVectorGeneration",
        ],
    )?;

    let relative = NIP09_SUCCESSOR_RESULT_VECTOR_EXECUTOR_RELATIVE;
    let source = rust_source(workspace_root, relative)?;
    let compact = compact_rust(&source, relative)?;
    require_ordered_markers(
        "SourceMaintenance NIP-09 successor result-vector executor",
        &compact,
        &[
            &format!(
                "constRESULT_VECTOR_EXECUTOR_ID:&str=\"{NIP09_SUCCESSOR_RESULT_VECTOR_EXECUTOR_ID}\""
            ),
            "sha256_hex(NIP09_RESULT_VECTOR_BYTES)",
            "sqlx::raw_sql(NIP09_EVENT_STORE_V1_UP_SQL)",
            ".bind(event.id_hex())",
            ".bind(event.signature_hex())",
            ".bind(event.id_hex())",
            "sqlx::raw_sql(NIP09_V1_UP_SQL)",
        ],
    )?;
    if compact.contains(".id_str()") || compact.contains(".sig_str()") {
        return Err(format!(
            "{relative} must use explicit canonical hexadecimal boundary encoders"
        ));
    }
    Ok(())
}

fn validate_sql_capacity_authority(workspace_root: &Path) -> Result<(), String> {
    let sql = read_regular_file(workspace_root, MIGRATION_UP_RELATIVE)?;
    let sql = std::str::from_utf8(&sql)
        .map_err(|error| format!("{MIGRATION_UP_RELATIVE} must be UTF-8 SQL: {error}"))?;
    for marker in [
        "DROP TRIGGER radroots_event_store_source_rebuild_marker_insert_guard",
        "CREATE TRIGGER radroots_event_store_source_rebuild_marker_insert_guard",
        "DROP TRIGGER radroots_event_store_food_availability_projection_delete_guard",
        "CREATE TRIGGER radroots_event_store_food_availability_projection_delete_guard",
        "DROP TRIGGER radroots_event_store_food_availability_image_delete_guard",
        "CREATE TRIGGER radroots_event_store_food_availability_image_delete_guard",
        "source.active_generation = marker.target_generation",
        "OLD.source_generation != source.active_generation",
        "raw_event_count >= 0 AND raw_event_count <= 25000",
        "raw_tag_count >= 0 AND raw_tag_count <= 250000",
        "raw_event_bytes >= 0 AND raw_event_bytes <= 67108864",
        "raw_tag_bytes >= 0 AND raw_tag_bytes <= 33554432",
        "retained_generation_count >= 1 AND retained_generation_count <= 8",
        "retained_generation_limit = 8",
        "CREATE TRIGGER radroots_event_store_source_generation_capacity_guard",
        "retained_generation_count >= retained_generation_limit",
        "CREATE TRIGGER radroots_event_store_source_generation_capacity_advance",
        "CREATE TRIGGER radroots_event_store_source_capacity_marker_close_guard",
        "02dfe1b450fbdac16e718888215b4dd5c85d8975440fa21e8f439fb24c2b2990",
        "8B63C5DDC48A2CC7DB69295238B96D5F814DBA50427C80B4D0079F061E6D3DE0",
        "capacity.retained_generation_count = (\n      SELECT COUNT(*)\n      FROM (\n        SELECT 1\n        FROM radroots_event_store_source_generation\n        LIMIT 9\n      )\n    )",
        "generation.generation_ordinal = capacity.retained_generation_count",
    ] {
        require_marker("SourceMaintenance SQL capacity authority", sql, marker)?;
    }
    if sql.contains("AND NEW.transition_floor_seq = state.last_transition_seq") {
        return Err(
            "SourceMaintenance v4 marker replacement must derive the transition floor from retained transitions rather than stale source-state high-water"
                .to_owned(),
        );
    }

    let down = read_regular_file(workspace_root, MIGRATION_DOWN_RELATIVE)?;
    let down = std::str::from_utf8(&down)
        .map_err(|error| format!("{MIGRATION_DOWN_RELATIVE} must be UTF-8 SQL: {error}"))?;
    for marker in [
        "DROP TRIGGER radroots_event_store_food_availability_image_delete_guard",
        "CREATE TRIGGER radroots_event_store_food_availability_image_delete_guard",
        "DROP TRIGGER radroots_event_store_food_availability_projection_delete_guard",
        "CREATE TRIGGER radroots_event_store_food_availability_projection_delete_guard",
        "DROP TRIGGER radroots_event_store_source_rebuild_marker_insert_guard",
        "CREATE TRIGGER radroots_event_store_source_rebuild_marker_insert_guard",
        "AND NEW.transition_floor_seq = state.last_transition_seq",
        "event-store FoodAvailability image delete is not backed by a pending retraction",
        "event-store FoodAvailability projection delete is not backed by a pending retraction",
    ] {
        require_marker(
            "SourceMaintenance exact v3 SQL restoration authority",
            down,
            marker,
        )?;
    }
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
    let model_source = rust_source(workspace_root, "crates/event_store/src/model.rs")?;
    let lib_source = rust_source(workspace_root, "crates/event_store/src/lib.rs")?;
    let error_source = rust_source(workspace_root, "crates/event_store/src/error.rs")?;
    let maintenance_source = rust_source(
        workspace_root,
        "crates/event_store/src/source_maintenance_v1.rs",
    )?;
    let store_source = rust_source(workspace_root, "crates/event_store/src/store.rs")?;
    validate_public_api_sources(
        &model_source,
        &lib_source,
        &error_source,
        &maintenance_source,
        &store_source,
    )
}

fn validate_public_api_sources(
    model_source: &str,
    lib_source: &str,
    error_source: &str,
    maintenance_source: &str,
    store_source: &str,
) -> Result<(), String> {
    let model = syn::parse_file(model_source)
        .map_err(|error| format!("parse crates/event_store/src/model.rs: {error}"))?;
    let lib = syn::parse_file(lib_source)
        .map_err(|error| format!("parse crates/event_store/src/lib.rs: {error}"))?;
    let error = syn::parse_file(error_source)
        .map_err(|error| format!("parse crates/event_store/src/error.rs: {error}"))?;
    let maintenance = syn::parse_file(maintenance_source).map_err(|error| {
        format!("parse crates/event_store/src/source_maintenance_v1.rs: {error}")
    })?;
    let store = syn::parse_file(store_source)
        .map_err(|error| format!("parse crates/event_store/src/store.rs: {error}"))?;

    let governed_modules = GOVERNED_MODEL_MODULES
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let mut model_exports = BTreeSet::new();
    let mut represented_modules = BTreeSet::new();
    for route in collect_top_level_public_use_routes(&model) {
        let Some(module) = route.segments.first().map(String::as_str) else {
            continue;
        };
        if !governed_modules.contains(module) {
            continue;
        }
        if route.absolute || route.renamed || route.glob || route.segments.len() != 2 {
            return Err(format!(
                "model predecessor export `{}` must be a direct, non-renamed public re-export",
                route.segments.join("::")
            ));
        }
        represented_modules.insert(module.to_owned());
        if !model_exports.insert(route.exported_name.clone()) {
            return Err(format!(
                "model predecessor symbol `{}` is exported more than once",
                route.exported_name
            ));
        }
    }
    let expected_modules = GOVERNED_MODEL_MODULES
        .iter()
        .map(|value| (*value).to_owned())
        .collect::<BTreeSet<_>>();
    let expected_inherited = INHERITED_PUBLIC_API
        .iter()
        .map(|value| (*value).to_owned())
        .collect::<BTreeSet<_>>();
    if represented_modules != expected_modules || model_exports != expected_inherited {
        return Err(format!(
            "model inherited FoodAvailability export authority differs: modules={represented_modules:?}, symbols={model_exports:?}"
        ));
    }

    let sqlite_cfg = "#[cfg(feature=\"sqlite\")]";
    let routes = collect_top_level_public_use_routes(&lib);
    if routes
        .iter()
        .any(|route| route.exported_name == "RadrootsEventStoreReconciliationResource")
    {
        return Err(
            "removed public symbol `RadrootsEventStoreReconciliationResource` must remain absent from the crate root"
                .to_owned(),
        );
    }
    let mut expected_root_routes = BTreeMap::new();
    for symbol in INHERITED_PUBLIC_API {
        expected_root_routes.insert((*symbol).to_owned(), "model");
    }
    for symbol in &ADDED_PUBLIC_API[..6] {
        expected_root_routes.insert((*symbol).to_owned(), "error");
    }
    expected_root_routes.insert(
        "RadrootsEventStoreSourceCapacityV1".to_owned(),
        "source_maintenance_v1",
    );
    for (symbol, expected_module) in expected_root_routes {
        let matches = routes
            .iter()
            .filter(|route| route.exported_name == symbol)
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err(format!(
                "crate root must export governed symbol `{symbol}` exactly once; found {}",
                matches.len()
            ));
        }
        let route = matches[0];
        if route.attributes.as_slice() != [sqlite_cfg]
            || route.absolute
            || route.renamed
            || route.glob
            || route.segments.len() != 2
            || route.segments[0] != expected_module
            || route.segments[1] != symbol
        {
            return Err(format!(
                "crate-root governed export `{symbol}` must be direct, non-renamed, sqlite-gated, and sourced from `{expected_module}`"
            ));
        }
    }

    let error_public = top_level_public_item_names(&error);
    if error_public.contains("RadrootsEventStoreReconciliationResource") {
        return Err(
            "removed public symbol `RadrootsEventStoreReconciliationResource` must remain absent from the error module"
                .to_owned(),
        );
    }
    for symbol in &ADDED_PUBLIC_API[..6] {
        if !error_public.contains(*symbol) {
            return Err(format!("error module does not publicly define `{symbol}`"));
        }
    }
    validate_source_capacity_snapshot_authority(&maintenance)?;

    let methods = associated_method_tokens(&store, "RadrootsEventStore", "source_capacity_v1")?;
    if methods.len() != 1 {
        return Err(format!(
            "RadrootsEventStore must define source_capacity_v1 exactly once; found {}",
            methods.len()
        ));
    }
    let expected = syn::parse_str::<syn::ImplItemFn>(
        r#"pub async fn source_capacity_v1(
            &self,
        ) -> Result<crate::RadrootsEventStoreSourceCapacityV1, RadrootsEventStoreError> {
            let mut tx = self.pool.begin().await?;
            let capacity =
                crate::source_maintenance_v1::validate_source_capacity_authority_fast_v1(
                    &mut tx
                ).await?;
            tx.commit().await?;
            Ok(capacity)
        }"#,
    )
    .map_err(|error| format!("parse authoritative source_capacity_v1 method: {error}"))?;
    let expected = compact_tokens(&expected);
    if methods[0] != expected {
        return Err(format!(
            "public capacity query signature or four-statement transaction authority drifted: expected `{expected}`, found `{}`",
            methods[0]
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

fn top_level_public_item_names(file: &syn::File) -> BTreeSet<String> {
    file.items
        .iter()
        .filter_map(|item| match item {
            Item::Const(item) if matches!(item.vis, syn::Visibility::Public(_)) => {
                Some(item.ident.to_string())
            }
            Item::Enum(item) if matches!(item.vis, syn::Visibility::Public(_)) => {
                Some(item.ident.to_string())
            }
            Item::Fn(item) if matches!(item.vis, syn::Visibility::Public(_)) => {
                Some(item.sig.ident.to_string())
            }
            Item::Struct(item) if matches!(item.vis, syn::Visibility::Public(_)) => {
                Some(item.ident.to_string())
            }
            Item::Type(item) if matches!(item.vis, syn::Visibility::Public(_)) => {
                Some(item.ident.to_string())
            }
            _ => None,
        })
        .collect()
}

fn exact_top_level_enum<'a>(file: &'a syn::File, name: &str) -> Result<&'a syn::ItemEnum, String> {
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
            "Rust source must define top-level enum `{name}` exactly once; found {}",
            matches.len()
        ));
    };
    Ok(item)
}

fn associated_method_tokens(
    file: &syn::File,
    owner: &str,
    method: &str,
) -> Result<Vec<String>, String> {
    let mut matches = Vec::new();
    for item in &file.items {
        let Item::Impl(item_impl) = item else {
            continue;
        };
        if compact_tokens(item_impl.self_ty.as_ref()) != owner {
            continue;
        }
        for item in &item_impl.items {
            let syn::ImplItem::Fn(function) = item else {
                continue;
            };
            if function.sig.ident == method {
                let mut function = function.clone();
                function.attrs.clear();
                matches.push(compact_tokens(&function));
            }
        }
    }
    Ok(matches)
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
    if collector.matches.len() != 1 {
        return Err(format!(
            "governed Rust source must define free function `{name}` exactly once; found {}",
            collector.matches.len()
        ));
    }
    Ok(collector.matches.pop().expect("one function"))
}

fn exact_free_function_tokens(file: &syn::File, name: &str) -> Result<String, String> {
    Ok(compact_tokens(exact_free_function(file, name)?))
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

fn rust_source(workspace_root: &Path, relative: &str) -> Result<String, String> {
    let bytes = read_regular_file(workspace_root, relative)?;
    std::str::from_utf8(&bytes)
        .map(str::to_owned)
        .map_err(|error| format!("{relative} must be UTF-8 Rust: {error}"))
}

fn compact_rust(source: &str, relative: &str) -> Result<String, String> {
    let syntax = syn::parse_file(source).map_err(|error| format!("parse {relative}: {error}"))?;
    Ok(compact_tokens(&syntax))
}

fn compact_tokens(tokens: &impl ToTokens) -> String {
    tokens.to_token_stream().to_string().replace(' ', "")
}

fn require_marker(label: &str, source: &str, marker: &str) -> Result<(), String> {
    if !source.contains(marker) {
        return Err(format!("{label} is missing exact witness `{marker}`"));
    }
    Ok(())
}

fn require_ordered_markers(label: &str, source: &str, markers: &[&str]) -> Result<(), String> {
    let mut offset = 0;
    for marker in markers {
        let Some(found) = source[offset..].find(marker) else {
            return Err(format!(
                "{label} is missing ordered witness `{marker}` after byte {offset}"
            ));
        };
        offset += found + marker.len();
    }
    Ok(())
}

fn validate_manifest_shape(manifest: &SourceMaintenanceManifest) -> Result<(), String> {
    if manifest.schema_version != SCHEMA_VERSION
        || manifest.contract_id != CONTRACT_ID
        || manifest.hook_id != HOOK_ID
        || manifest.manifest_schema.path != MANIFEST_SCHEMA_RELATIVE
        || manifest.predecessor.hook_id != PREDECESSOR_HOOK_ID
        || manifest.predecessor.manifest.path != PREDECESSOR_MANIFEST_RELATIVE
        || manifest.predecessor.manifest.byte_length
            != u64::try_from(PREDECESSOR_MANIFEST_BYTE_LENGTH)
                .map_err(|_| "predecessor length does not fit u64".to_owned())?
        || manifest.predecessor.manifest.sha256 != PREDECESSOR_MANIFEST_SHA256
        || manifest.migration.version != MIGRATION_VERSION
        || manifest.migration.name != MIGRATION_NAME
        || manifest.migration.up.path != MIGRATION_UP_RELATIVE
        || manifest.migration.down.path != MIGRATION_DOWN_RELATIVE
        || manifest.migration.schema_sha256 != SCHEMA_SHA256
        || manifest.source_maintenance
            != (SourceMaintenanceDescriptor {
                version: CAPACITY_VERSION,
                event_contract_registry_version: EVENT_CONTRACT_REGISTRY_VERSION,
                capacity_authority_id: CAPACITY_AUTHORITY_ID.to_owned(),
                accounting: AccountingDescriptor {
                    algorithm: ACCOUNTING_ALGORITHM.to_owned(),
                    raw_event_columns: owned(RAW_EVENT_COLUMNS),
                    raw_tag_columns: owned(RAW_TAG_COLUMNS),
                    nullable_raw_tag_columns: owned(NULLABLE_RAW_TAG_COLUMNS),
                },
                limits: expected_limits(),
                reopen_validation: ReopenValidationDescriptor {
                    mode: REOPEN_VALIDATION_MODE.to_owned(),
                    raw_event_rejection_scan_bound: RAW_EVENT_REJECTION_SCAN_BOUND,
                    raw_tag_rejection_scan_bound: RAW_TAG_REJECTION_SCAN_BOUND,
                    generation_history_validation: GENERATION_HISTORY_VALIDATION.to_owned(),
                    retained_generation_rejection_scan_bound:
                        RETAINED_GENERATION_REJECTION_SCAN_BOUND,
                },
                rebuild_seal: RebuildSealDescriptor {
                    nip09_hook_id: NIP09_HOOK_ID.to_owned(),
                    nip09_manifest_sha256: NIP09_MANIFEST_SHA256.to_owned(),
                    food_hook_id: PREDECESSOR_HOOK_ID.to_owned(),
                    food_manifest_sha256: PREDECESSOR_MANIFEST_SHA256.to_owned(),
                    food_scope_fingerprint_sha256: FOOD_SCOPE_FINGERPRINT_SHA256.to_owned(),
                    active_generation_authority: ACTIVE_GENERATION_AUTHORITY.to_owned(),
                    marker_close_authority: MARKER_CLOSE_AUTHORITY.to_owned(),
                },
            })
        || manifest.public_api != expected_public_api()
    {
        return Err(format!(
            "{MANIFEST_RELATIVE} has inconsistent SourceMaintenance identity or semantics"
        ));
    }
    validate_catalog(&manifest.migration.catalog)?;
    validate_migration_identity(&manifest.migration.up, &manifest.migration.down)?;

    let expected_entry_points = ENTRY_POINTS
        .iter()
        .map(|(role, rust_path)| EntryPointDescriptor {
            role: (*role).to_owned(),
            rust_path: (*rust_path).to_owned(),
        })
        .collect::<Vec<_>>();
    if manifest.entry_points != expected_entry_points {
        return Err(format!(
            "{MANIFEST_RELATIVE} entry-point inventory is not exact"
        ));
    }
    let expected_source_identity = SOURCE_SPECS
        .iter()
        .map(|spec| (spec.role, spec.path))
        .collect::<Vec<_>>();
    let actual_source_identity = manifest
        .source_files
        .iter()
        .map(|source| (source.role.as_str(), source.path.as_str()))
        .collect::<Vec<_>>();
    if actual_source_identity != expected_source_identity {
        return Err(format!(
            "{MANIFEST_RELATIVE} source-file inventory is not exact"
        ));
    }
    validate_unique(
        "SourceMaintenance manifest source roles",
        manifest
            .source_files
            .iter()
            .map(|source| source.role.as_str()),
    )?;
    validate_unique(
        "SourceMaintenance manifest source paths",
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
        validate_sha256(source.path.as_str(), source.sha256.as_str())?;
        if source.hash_algorithm != HASH_ALGORITHM || source.byte_length == 0 {
            return Err(format!(
                "{MANIFEST_RELATIVE} source descriptor `{}` is invalid",
                source.path
            ));
        }
    }
    for descriptor in [
        &manifest.manifest_schema,
        &manifest.predecessor.manifest,
        &manifest.migration.up,
        &manifest.migration.down,
    ] {
        validate_sha256(descriptor.path.as_str(), descriptor.sha256.as_str())?;
        if descriptor.hash_algorithm != HASH_ALGORITHM || descriptor.byte_length == 0 {
            return Err(format!(
                "{MANIFEST_RELATIVE} file descriptor `{}` is invalid",
                descriptor.path
            ));
        }
    }
    validate_sha256(
        "migration schema",
        manifest.migration.schema_sha256.as_str(),
    )?;
    validate_sha256("result vector", manifest.result_vector.sha256.as_str())?;
    validate_sha256(
        "result-vector executor",
        manifest.result_vector.executor_sha256.as_str(),
    )?;
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
    Ok(())
}

fn generated_descriptor(
    manifest: &SourceMaintenanceManifest,
    manifest_bytes: &[u8],
    manifest_sha256: &str,
) -> String {
    let manifest_json = std::str::from_utf8(manifest_bytes).expect("canonical manifest is UTF-8");
    let manifest_literal = format!("{manifest_json:?}");
    format!(
        "// @generated by `cargo xtask contract source-maintenance-manifest --write`; do not edit.\n\
pub(crate) const SOURCE_MAINTENANCE_MANIFEST_JSON: &str = {manifest_literal};\n\
pub(crate) const SOURCE_MAINTENANCE_MANIFEST_BYTE_LENGTH: usize = {};\n\
pub(crate) const SOURCE_MAINTENANCE_MANIFEST_SHA256: &str =\n    \"{manifest_sha256}\";\n\
pub(crate) const SOURCE_MAINTENANCE_MANIFEST_SCHEMA_VERSION: u32 = {SCHEMA_VERSION};\n\
pub(crate) const SOURCE_MAINTENANCE_CONTRACT_ID: &str =\n    \"{CONTRACT_ID}\";\n\
pub(crate) const SOURCE_MAINTENANCE_HOOK_ID: &str = \"{HOOK_ID}\";\n\
pub(crate) const SOURCE_MAINTENANCE_MIGRATION_VERSION: u32 = {MIGRATION_VERSION};\n\
pub(crate) const SOURCE_MAINTENANCE_MIGRATION_NAME: &str = \"{MIGRATION_NAME}\";\n\
pub(crate) const SOURCE_MAINTENANCE_MIGRATION_UP_BYTE_LENGTH: usize = {};\n\
pub(crate) const SOURCE_MAINTENANCE_MIGRATION_UP_SHA256: &str =\n    \"{}\";\n\
pub(crate) const SOURCE_MAINTENANCE_MIGRATION_DOWN_BYTE_LENGTH: usize = {};\n\
pub(crate) const SOURCE_MAINTENANCE_MIGRATION_DOWN_SHA256: &str =\n    \"{}\";\n\
pub(crate) const SOURCE_MAINTENANCE_SCHEMA_SHA256: &str =\n    \"{SCHEMA_SHA256}\";\n\
pub(crate) const SOURCE_MAINTENANCE_EVENT_CONTRACT_REGISTRY_VERSION: u32 = {EVENT_CONTRACT_REGISTRY_VERSION};\n\
pub(crate) const SOURCE_MAINTENANCE_CAPACITY_VERSION: u32 = {CAPACITY_VERSION};\n\
pub(crate) const SOURCE_MAINTENANCE_CAPACITY_AUTHORITY_ID: &str =\n    \"{CAPACITY_AUTHORITY_ID}\";\n\
pub(crate) const SOURCE_MAINTENANCE_ACCOUNTING_ALGORITHM: &str = \"{ACCOUNTING_ALGORITHM}\";\n\
pub(crate) const SOURCE_MAINTENANCE_RAW_EVENT_COLUMNS: &[&str] = &{};\n\
pub(crate) const SOURCE_MAINTENANCE_RAW_TAG_COLUMNS: &[&str] =\n    &{};\n\
pub(crate) const SOURCE_MAINTENANCE_NULLABLE_RAW_TAG_COLUMNS: &[&str] = &{};\n\
pub(crate) const SOURCE_MAINTENANCE_REPLACED_OBJECT_NAMES: &[&str] = &{};\n\
pub(crate) const SOURCE_MAINTENANCE_RAW_EVENT_COUNT_LIMIT: u64 = {RAW_EVENT_COUNT_LIMIT};\n\
pub(crate) const SOURCE_MAINTENANCE_RAW_TAG_COUNT_LIMIT: u64 = {RAW_TAG_COUNT_LIMIT};\n\
pub(crate) const SOURCE_MAINTENANCE_RAW_EVENT_TEXT_BYTES_LIMIT: u64 = {RAW_EVENT_TEXT_BYTES_LIMIT};\n\
pub(crate) const SOURCE_MAINTENANCE_RAW_TAG_TEXT_BYTES_LIMIT: u64 = {RAW_TAG_TEXT_BYTES_LIMIT};\n\
pub(crate) const SOURCE_MAINTENANCE_RETAINED_SOURCE_GENERATION_LIMIT: u32 = {RETAINED_SOURCE_GENERATION_LIMIT};\n\
pub(crate) const SOURCE_MAINTENANCE_PREDECESSOR_HOOK_ID: &str = \"{PREDECESSOR_HOOK_ID}\";\n\
pub(crate) const SOURCE_MAINTENANCE_PREDECESSOR_MANIFEST_SHA256: &str =\n    \"{PREDECESSOR_MANIFEST_SHA256}\";\n\
pub(crate) const SOURCE_MAINTENANCE_RESULT_VECTOR_SHA256: &str =\n    \"{}\";\n\
pub(crate) const SOURCE_MAINTENANCE_RESULT_VECTOR_EXECUTOR_ID: &str =\n    \"{RESULT_VECTOR_EXECUTOR_ID}\";\n\
pub(crate) const SOURCE_MAINTENANCE_RESULT_VECTOR_EXECUTOR_SHA256: &str =\n    \"{}\";\n",
        manifest_bytes.len(),
        usize::try_from(manifest.migration.up.byte_length).expect("up length fits usize"),
        manifest.migration.up.sha256,
        usize::try_from(manifest.migration.down.byte_length).expect("down length fits usize"),
        manifest.migration.down.sha256,
        rust_multiline_string_slice(RAW_EVENT_COLUMNS),
        rust_string_slice(RAW_TAG_COLUMNS),
        rust_string_slice(NULLABLE_RAW_TAG_COLUMNS),
        rust_multiline_string_slice(EXPECTED_REPLACED_CATALOG_OBJECTS),
        manifest.result_vector.sha256,
        manifest.result_vector.executor_sha256,
    )
}

fn rust_string_slice(values: &[&str]) -> String {
    let values = values
        .iter()
        .map(|value| format!("{value:?}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{values}]")
}

fn rust_multiline_string_slice(values: &[&str]) -> String {
    let values = values
        .iter()
        .map(|value| format!("    {value:?},"))
        .collect::<Vec<_>>()
        .join("\n");
    format!("[\n{values}\n]")
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
        "items": {"type": "string", "minLength": 1}
    });
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://radroots.org/contracts/event-store/source-maintenance-v1-manifest.schema.json",
        "title": "Radroots event-store SourceMaintenance v1 manifest",
        "type": "object",
        "required": [
            "schema_version", "contract_id", "hook_id", "manifest_schema", "predecessor",
            "migration", "source_maintenance", "entry_points", "source_files", "public_api",
            "result_vector"
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
                        "required": ["objects", "replaced_objects", "tables", "fts5_tables"],
                        "properties": {
                            "objects": string_array.clone(),
                            "replaced_objects": string_array.clone(),
                            "tables": string_array.clone(),
                            "fts5_tables": string_array.clone()
                        },
                        "additionalProperties": false
                    }
                },
                "additionalProperties": false
            },
            "source_maintenance": {
                "type": "object",
                "required": [
                    "version", "event_contract_registry_version", "capacity_authority_id",
                    "accounting", "limits", "reopen_validation", "rebuild_seal"
                ],
                "properties": {
                    "version": {"const": CAPACITY_VERSION},
                    "event_contract_registry_version": {"const": EVENT_CONTRACT_REGISTRY_VERSION},
                    "capacity_authority_id": {"const": CAPACITY_AUTHORITY_ID},
                    "accounting": {"$ref": "#/$defs/accounting"},
                    "limits": {"$ref": "#/$defs/limits"},
                    "reopen_validation": {
                        "type": "object",
                        "required": [
                            "mode", "raw_event_rejection_scan_bound",
                            "raw_tag_rejection_scan_bound", "generation_history_validation",
                            "retained_generation_rejection_scan_bound"
                        ],
                        "properties": {
                            "mode": {"const": REOPEN_VALIDATION_MODE},
                            "raw_event_rejection_scan_bound": {
                                "const": RAW_EVENT_REJECTION_SCAN_BOUND
                            },
                            "raw_tag_rejection_scan_bound": {
                                "const": RAW_TAG_REJECTION_SCAN_BOUND
                            },
                            "generation_history_validation": {"const": GENERATION_HISTORY_VALIDATION},
                            "retained_generation_rejection_scan_bound": {
                                "const": RETAINED_GENERATION_REJECTION_SCAN_BOUND
                            }
                        },
                        "additionalProperties": false
                    },
                    "rebuild_seal": {
                        "type": "object",
                        "required": [
                            "nip09_hook_id", "nip09_manifest_sha256", "food_hook_id",
                            "food_manifest_sha256", "food_scope_fingerprint_sha256",
                            "active_generation_authority", "marker_close_authority"
                        ],
                        "properties": {
                            "nip09_hook_id": {"const": NIP09_HOOK_ID},
                            "nip09_manifest_sha256": {"const": NIP09_MANIFEST_SHA256},
                            "food_hook_id": {"const": PREDECESSOR_HOOK_ID},
                            "food_manifest_sha256": {"const": PREDECESSOR_MANIFEST_SHA256},
                            "food_scope_fingerprint_sha256": {"const": FOOD_SCOPE_FINGERPRINT_SHA256},
                            "active_generation_authority": {"const": ACTIVE_GENERATION_AUTHORITY},
                            "marker_close_authority": {"const": MARKER_CLOSE_AUTHORITY}
                        },
                        "additionalProperties": false
                    }
                },
                "additionalProperties": false
            },
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
            "public_api": {
                "type": "object",
                "required": [
                    "inherited_predecessor_symbols", "added_symbols", "methods", "error_variants",
                    "removed_symbols", "breaking_replacements"
                ],
                "properties": {
                    "inherited_predecessor_symbols": string_array.clone(),
                    "added_symbols": string_array.clone(),
                    "methods": string_array.clone(),
                    "error_variants": string_array.clone(),
                    "removed_symbols": string_array.clone(),
                    "breaking_replacements": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "required": ["removed", "replacement"],
                            "properties": {
                                "removed": {"type": "string", "minLength": 1},
                                "replacement": {"type": "string", "minLength": 1}
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
                    "path": {"type": "string", "pattern": path_pattern},
                    "byte_length": {"type": "integer", "minimum": 1},
                    "sha256": {"type": "string", "pattern": "^[0-9a-f]{64}$"},
                    "hash_algorithm": {"const": HASH_ALGORITHM}
                },
                "additionalProperties": false
            },
            "accounting": {
                "type": "object",
                "required": [
                    "algorithm", "raw_event_columns", "raw_tag_columns", "nullable_raw_tag_columns"
                ],
                "properties": {
                    "algorithm": {"const": ACCOUNTING_ALGORITHM},
                    "raw_event_columns": string_array.clone(),
                    "raw_tag_columns": string_array.clone(),
                    "nullable_raw_tag_columns": string_array.clone()
                },
                "additionalProperties": false
            },
            "limits": {
                "type": "object",
                "required": [
                    "raw_events", "raw_tags", "raw_event_text_bytes", "raw_tag_text_bytes",
                    "retained_source_generations"
                ],
                "properties": {
                    "raw_events": {"const": RAW_EVENT_COUNT_LIMIT},
                    "raw_tags": {"const": RAW_TAG_COUNT_LIMIT},
                    "raw_event_text_bytes": {"const": RAW_EVENT_TEXT_BYTES_LIMIT},
                    "raw_tag_text_bytes": {"const": RAW_TAG_TEXT_BYTES_LIMIT},
                    "retained_source_generations": {"const": RETAINED_SOURCE_GENERATION_LIMIT}
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
    let canonical = canonical_json_bytes(value)?;
    if actual != canonical {
        return Err(format!(
            "{relative} must use canonical pretty JSON with one trailing newline"
        ));
    }
    Ok(())
}

fn validate_unique<'a>(label: &str, values: impl Iterator<Item = &'a str>) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for value in values {
        if !seen.insert(value) {
            return Err(format!("{label} contains duplicate `{value}`"));
        }
    }
    Ok(())
}

fn validate_digest_sidecar(relative: &str, bytes: &[u8]) -> Result<(), String> {
    let value =
        std::str::from_utf8(bytes).map_err(|error| format!("{relative} must be UTF-8: {error}"))?;
    let Some(digest) = value.strip_suffix('\n') else {
        return Err(format!("{relative} must end in exactly one newline"));
    };
    if digest.contains('\n') || digest.contains('\r') {
        return Err(format!("{relative} must contain one SHA-256 digest line"));
    }
    validate_sha256(relative, digest)
}

fn validate_sha256(label: &str, value: &str) -> Result<(), String> {
    if value.len() != 64
        || !value
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(format!("{label} must be a lowercase 64-hex SHA-256 digest"));
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn stale_error(relative: &str) -> String {
    format!("{relative} is stale; run `{WRITE_COMMAND}`")
}

#[derive(Clone, Copy)]
struct ExpectedVectorCase {
    id: &'static str,
    execution: &'static str,
    authority: &'static str,
    authority_path: &'static str,
    resource: Option<&'static str>,
    boundary: Option<&'static str>,
    expected_outcome: &'static str,
    error_domain: Option<&'static str>,
    expected_error: Option<&'static str>,
}

const DIRECT_EXECUTOR: &str = "direct_executor";
const DELEGATED_RUST_TEST: &str = "delegated_rust_test";
const DELEGATED_SQL_TEST: &str = "delegated_sql_test";

const EXPECTED_VECTOR_CASES: &[ExpectedVectorCase] = &[
    ExpectedVectorCase {
        id: "fresh_store_zero_authority",
        execution: DIRECT_EXECUTOR,
        authority: RESULT_VECTOR_EXECUTOR_TEST,
        authority_path: RESULT_VECTOR_EXECUTOR_RELATIVE,
        resource: None,
        boundary: None,
        expected_outcome: "accepted",
        error_domain: None,
        expected_error: None,
    },
    ExpectedVectorCase {
        id: "durable_unique_append_updates_all_dimensions",
        execution: DIRECT_EXECUTOR,
        authority: RESULT_VECTOR_EXECUTOR_TEST,
        authority_path: RESULT_VECTOR_EXECUTOR_RELATIVE,
        resource: None,
        boundary: None,
        expected_outcome: "accepted",
        error_domain: None,
        expected_error: None,
    },
    ExpectedVectorCase {
        id: "duplicate_at_exact_boundary_is_idempotent",
        execution: DELEGATED_RUST_TEST,
        authority: "exact_capacity_boundary_allows_duplicate_observation_and_ephemeral_noop",
        authority_path: "crates/event_store/src/store.rs",
        resource: Some("raw_events"),
        boundary: Some("exact"),
        expected_outcome: "accepted_without_capacity_delta",
        error_domain: None,
        expected_error: None,
    },
    ExpectedVectorCase {
        id: "ephemeral_consumes_no_capacity",
        execution: DIRECT_EXECUTOR,
        authority: RESULT_VECTOR_EXECUTOR_TEST,
        authority_path: RESULT_VECTOR_EXECUTOR_RELATIVE,
        resource: None,
        boundary: None,
        expected_outcome: "accepted_without_capacity_delta",
        error_domain: None,
        expected_error: None,
    },
    ExpectedVectorCase {
        id: "raw_event_count_exact",
        execution: DELEGATED_RUST_TEST,
        authority: "every_prospective_capacity_dimension_accepts_exact_and_rejects_one_over",
        authority_path: "crates/event_store/src/source_maintenance_v1.rs",
        resource: Some("raw_events"),
        boundary: Some("exact"),
        expected_outcome: "accepted",
        error_domain: None,
        expected_error: None,
    },
    ExpectedVectorCase {
        id: "raw_event_count_one_over",
        execution: DELEGATED_RUST_TEST,
        authority: "every_prospective_capacity_dimension_accepts_exact_and_rejects_one_over",
        authority_path: "crates/event_store/src/source_maintenance_v1.rs",
        resource: Some("raw_events"),
        boundary: Some("one_over"),
        expected_outcome: "rejected_before_mutation",
        error_domain: Some("typed"),
        expected_error: Some("SourceCapacityExceeded"),
    },
    ExpectedVectorCase {
        id: "raw_tag_count_exact",
        execution: DELEGATED_RUST_TEST,
        authority: "every_prospective_capacity_dimension_accepts_exact_and_rejects_one_over",
        authority_path: "crates/event_store/src/source_maintenance_v1.rs",
        resource: Some("raw_tags"),
        boundary: Some("exact"),
        expected_outcome: "accepted",
        error_domain: None,
        expected_error: None,
    },
    ExpectedVectorCase {
        id: "raw_tag_count_one_over",
        execution: DELEGATED_RUST_TEST,
        authority: "every_prospective_capacity_dimension_accepts_exact_and_rejects_one_over",
        authority_path: "crates/event_store/src/source_maintenance_v1.rs",
        resource: Some("raw_tags"),
        boundary: Some("one_over"),
        expected_outcome: "rejected_before_mutation",
        error_domain: Some("typed"),
        expected_error: Some("SourceCapacityExceeded"),
    },
    ExpectedVectorCase {
        id: "raw_event_text_bytes_exact",
        execution: DELEGATED_RUST_TEST,
        authority: "every_prospective_capacity_dimension_accepts_exact_and_rejects_one_over",
        authority_path: "crates/event_store/src/source_maintenance_v1.rs",
        resource: Some("raw_event_text_bytes"),
        boundary: Some("exact"),
        expected_outcome: "accepted",
        error_domain: None,
        expected_error: None,
    },
    ExpectedVectorCase {
        id: "raw_event_text_bytes_one_over",
        execution: DELEGATED_RUST_TEST,
        authority: "every_prospective_capacity_dimension_accepts_exact_and_rejects_one_over",
        authority_path: "crates/event_store/src/source_maintenance_v1.rs",
        resource: Some("raw_event_text_bytes"),
        boundary: Some("one_over"),
        expected_outcome: "rejected_before_mutation",
        error_domain: Some("typed"),
        expected_error: Some("SourceCapacityExceeded"),
    },
    ExpectedVectorCase {
        id: "raw_tag_text_bytes_exact",
        execution: DELEGATED_RUST_TEST,
        authority: "every_prospective_capacity_dimension_accepts_exact_and_rejects_one_over",
        authority_path: "crates/event_store/src/source_maintenance_v1.rs",
        resource: Some("raw_tag_text_bytes"),
        boundary: Some("exact"),
        expected_outcome: "accepted",
        error_domain: None,
        expected_error: None,
    },
    ExpectedVectorCase {
        id: "raw_tag_text_bytes_one_over",
        execution: DELEGATED_RUST_TEST,
        authority: "every_prospective_capacity_dimension_accepts_exact_and_rejects_one_over",
        authority_path: "crates/event_store/src/source_maintenance_v1.rs",
        resource: Some("raw_tag_text_bytes"),
        boundary: Some("one_over"),
        expected_outcome: "rejected_before_mutation",
        error_domain: Some("typed"),
        expected_error: Some("SourceCapacityExceeded"),
    },
    ExpectedVectorCase {
        id: "outer_transaction_rollback_restores_capacity",
        execution: DIRECT_EXECUTOR,
        authority: RESULT_VECTOR_EXECUTOR_TEST,
        authority_path: RESULT_VECTOR_EXECUTOR_RELATIVE,
        resource: None,
        boundary: None,
        expected_outcome: "rolled_back_without_capacity_delta",
        error_domain: None,
        expected_error: None,
    },
    ExpectedVectorCase {
        id: "failed_nested_ingest_rolls_back_savepoint_only",
        execution: DELEGATED_RUST_TEST,
        authority: "borrowed_ingest_savepoint_rolls_back_post_core_authority_forge",
        authority_path: "crates/event_store/src/store.rs",
        resource: None,
        boundary: None,
        expected_outcome: "failed_ingest_rolled_back_and_prior_caller_work_preserved",
        error_domain: Some("typed"),
        expected_error: Some("MigrationHookStateDrift"),
    },
    ExpectedVectorCase {
        id: "v3_to_v4_under_limit_succeeds",
        execution: DELEGATED_RUST_TEST,
        authority: "v3_to_v4_under_limit_backfills_exact_capacity_and_preserves_source",
        authority_path: "crates/event_store/src/schema.rs",
        resource: None,
        boundary: Some("under_limit"),
        expected_outcome: "accepted",
        error_domain: None,
        expected_error: None,
    },
    ExpectedVectorCase {
        id: "v3_to_v4_prior_transition_drift_is_atomic",
        execution: DELEGATED_RUST_TEST,
        authority: "v3_to_v4_rejects_prior_transition_drift_atomically",
        authority_path: "crates/event_store/src/schema.rs",
        resource: None,
        boundary: Some("corrupt_managed_v3"),
        expected_outcome: "rejected_before_v4_schema_ledger_or_predecessor_trigger_mutation",
        error_domain: Some("typed"),
        expected_error: Some("MigrationHookStateDrift"),
    },
    ExpectedVectorCase {
        id: "v3_to_v4_one_over_is_atomic",
        execution: DELEGATED_RUST_TEST,
        authority: "source_capacity_is_rechecked_for_every_rebuild_bound_migration",
        authority_path: "crates/event_store/src/schema.rs",
        resource: Some("raw_events"),
        boundary: Some("one_over"),
        expected_outcome: "rejected_before_mutation",
        error_domain: Some("typed"),
        expected_error: Some("SourceCapacityExceeded"),
    },
    ExpectedVectorCase {
        id: "v3_to_v4_persisted_ephemeral_is_atomic",
        execution: DELEGATED_RUST_TEST,
        authority: "v4_rejects_persisted_legacy_ephemeral_rows_atomically",
        authority_path: "crates/event_store/src/schema.rs",
        resource: None,
        boundary: None,
        expected_outcome: "rejected_before_mutation",
        error_domain: Some("typed"),
        expected_error: Some("PersistedEphemeralRawEvent"),
    },
    ExpectedVectorCase {
        id: "reopen_rejects_incoherent_capacity_authority",
        execution: DELEGATED_RUST_TEST,
        authority: "reopen_full_measure_detects_every_persisted_capacity_dimension",
        authority_path: "crates/event_store/src/source_maintenance_v1.rs",
        resource: None,
        boundary: None,
        expected_outcome: "rejected_on_reopen",
        error_domain: Some("typed"),
        expected_error: Some("SourceCapacityStateDrift"),
    },
    ExpectedVectorCase {
        id: "reopen_stops_at_first_raw_event_one_over",
        execution: DELEGATED_RUST_TEST,
        authority: "reopen_stops_at_the_first_raw_event_one_over_before_ephemeral_probe",
        authority_path: "crates/event_store/src/source_maintenance_v1.rs",
        resource: Some("raw_events"),
        boundary: Some("one_over"),
        expected_outcome: "rejected_at_scan_bound",
        error_domain: Some("typed"),
        expected_error: Some("SourceCapacityExceeded"),
    },
    ExpectedVectorCase {
        id: "retained_generation_rebuild_exact",
        execution: DELEGATED_RUST_TEST,
        authority: "ninth_current_v4_rebuild_is_typed_and_preflight_atomic",
        authority_path: "crates/event_store/src/store.rs",
        resource: Some("retained_source_generations"),
        boundary: Some("exact"),
        expected_outcome: "accepted",
        error_domain: None,
        expected_error: None,
    },
    ExpectedVectorCase {
        id: "ninth_rebuild_is_typed_and_atomic",
        execution: DELEGATED_RUST_TEST,
        authority: "ninth_current_v4_rebuild_is_typed_and_preflight_atomic",
        authority_path: "crates/event_store/src/store.rs",
        resource: Some("retained_source_generations"),
        boundary: Some("one_over"),
        expected_outcome: "rejected_before_entropy_or_mutation",
        error_domain: Some("typed"),
        expected_error: Some("SourceGenerationHistoryLimitReached"),
    },
    ExpectedVectorCase {
        id: "retained_generation_sql_backstop_one_over",
        execution: DELEGATED_SQL_TEST,
        authority: "generation_sql_backstop_allows_exact_append_and_is_conflict_safe_one_over",
        authority_path: "crates/event_store/src/source_maintenance_v1.rs",
        resource: Some("retained_source_generations"),
        boundary: Some("one_over"),
        expected_outcome: "rejected_by_sql_backstop",
        error_domain: Some("sqlite_database"),
        expected_error: Some(
            "event-store retained source generation limit reached; replace and resync into a fresh store",
        ),
    },
    ExpectedVectorCase {
        id: "rebuild_marker_accepts_consistent_seals",
        execution: DELEGATED_RUST_TEST,
        authority: "current_v4_rebuild_rotates_capacity_and_food_authority_end_to_end",
        authority_path: "crates/event_store/src/store.rs",
        resource: None,
        boundary: None,
        expected_outcome: "accepted",
        error_domain: None,
        expected_error: None,
    },
    ExpectedVectorCase {
        id: "rebuild_marker_rejects_incoherent_seals",
        execution: DELEGATED_SQL_TEST,
        authority: "marker_close_sql_backstop_rejects_each_required_seal_drift",
        authority_path: "crates/event_store/src/source_maintenance_v1.rs",
        resource: None,
        boundary: None,
        expected_outcome: "rejected_by_sql_backstop",
        error_domain: Some("sqlite_database"),
        expected_error: Some(
            "event-store rebuild marker cannot close before capacity, NIP-09, and FoodAvailability seals agree",
        ),
    },
    ExpectedVectorCase {
        id: "v4_marker_repair_binds_exact_prior_and_floor",
        execution: DELEGATED_RUST_TEST,
        authority: "v4_marker_open_allows_repairing_prior_transition_high_water_drift",
        authority_path: "crates/event_store/src/schema.rs",
        resource: None,
        boundary: Some("managed_v4_rebuild"),
        expected_outcome: "accepts_derived_high_water_repair_and_rejects_wrong_prior_or_floor",
        error_domain: Some("sqlite_database"),
        expected_error: Some("exact raw and prior source authority"),
    },
    ExpectedVectorCase {
        id: "v4_food_reset_requires_target_rotation",
        execution: DELEGATED_RUST_TEST,
        authority: "v4_food_reset_requires_marker_rotation_and_preserves_target_rows",
        authority_path: "crates/event_store/src/schema.rs",
        resource: None,
        boundary: Some("managed_v4_rebuild"),
        expected_outcome: "historical_rows_deleted_only_after_rotation_and_target_rows_preserved",
        error_domain: None,
        expected_error: None,
    },
    ExpectedVectorCase {
        id: "v4_down_restores_predecessor_triggers",
        execution: DELEGATED_RUST_TEST,
        authority: "v4_down_restores_exact_predecessor_trigger_sql_and_fingerprint",
        authority_path: "crates/event_store/src/schema.rs",
        resource: None,
        boundary: Some("v4_to_v3"),
        expected_outcome: "restored_exact_predecessor_trigger_sql_and_v3_fingerprint",
        error_domain: None,
        expected_error: None,
    },
    ExpectedVectorCase {
        id: "utf16_open_file_rejected_before_mutation",
        execution: DELEGATED_RUST_TEST,
        authority: "open_file_rejects_utf16_main_database_before_schema_or_journal_mutation",
        authority_path: "crates/event_store/src/store.rs",
        resource: None,
        boundary: None,
        expected_outcome: "rejected_before_schema_or_journal_mutation",
        error_domain: Some("typed"),
        expected_error: Some("SqliteMainDatabaseEncodingNotUtf8"),
    },
    ExpectedVectorCase {
        id: "utf16_open_pool_rejected_before_mutation",
        execution: DELEGATED_RUST_TEST,
        authority: "open_pool_rejects_utf16_main_database_before_schema_or_journal_mutation",
        authority_path: "crates/event_store/src/store.rs",
        resource: None,
        boundary: None,
        expected_outcome: "rejected_before_schema_or_journal_mutation",
        error_domain: Some("typed"),
        expected_error: Some("SqliteMainDatabaseEncodingNotUtf8"),
    },
    ExpectedVectorCase {
        id: "utf8_non_ascii_nul_reopen_accounting",
        execution: DELEGATED_RUST_TEST,
        authority: "utf8_file_reopen_preserves_non_ascii_and_nul_capacity_accounting",
        authority_path: "crates/event_store/src/store.rs",
        resource: None,
        boundary: None,
        expected_outcome: "accepted_with_exact_capacity_after_reopen",
        error_domain: None,
        expected_error: None,
    },
    ExpectedVectorCase {
        id: "generation_destructive_rollback_rejected",
        execution: DELEGATED_RUST_TEST,
        authority: "rollback_rejects_below_floor_ahead_unmanaged_and_generation_destructive_targets",
        authority_path: "crates/event_store/src/schema.rs",
        resource: Some("retained_source_generations"),
        boundary: None,
        expected_outcome: "rejected_before_mutation_with_status_and_history_preserved",
        error_domain: Some("typed"),
        expected_error: Some("RollbackWouldDiscardSourceGenerationHistory"),
    },
    ExpectedVectorCase {
        id: "generation_destructive_two_step_rollback_rejected",
        execution: DELEGATED_RUST_TEST,
        authority: "rollback_cannot_bypass_generation_history_guard_through_version_three",
        authority_path: "crates/event_store/src/schema.rs",
        resource: Some("retained_source_generations"),
        boundary: None,
        expected_outcome: "rejected_before_mutation_after_history_preserving_intermediate_rollback",
        error_domain: Some("typed"),
        expected_error: Some("RollbackWouldDiscardSourceGenerationHistory"),
    },
    ExpectedVectorCase {
        id: "independent_pool_last_byte_slot_race",
        execution: DELEGATED_RUST_TEST,
        authority: "independent_file_pools_serialize_the_last_raw_event_byte_capacity_slot",
        authority_path: "crates/event_store/src/store.rs",
        resource: Some("raw_event_text_bytes"),
        boundary: Some("exact"),
        expected_outcome: "exactly_one_accepted_one_typed_rejection_and_clean_reopen",
        error_domain: Some("typed"),
        expected_error: Some("SourceCapacityExceeded"),
    },
];

fn validate_result_vector(
    workspace_root: &Path,
    vector: &SourceMaintenanceVector,
) -> Result<(), String> {
    if vector.schema_version != SCHEMA_VERSION
        || vector.contract_id != CONTRACT_ID
        || vector.capacity_version != CAPACITY_VERSION
        || vector.limits != expected_limits()
        || vector.accounting
            != (AccountingDescriptor {
                algorithm: ACCOUNTING_ALGORITHM.to_owned(),
                raw_event_columns: owned(RAW_EVENT_COLUMNS),
                raw_tag_columns: owned(RAW_TAG_COLUMNS),
                nullable_raw_tag_columns: owned(NULLABLE_RAW_TAG_COLUMNS),
            })
    {
        return Err(format!(
            "{RESULT_VECTOR_CANONICAL_RELATIVE} header, limits, or accounting authority is invalid"
        ));
    }
    if vector.cases.len() != EXPECTED_VECTOR_CASES.len() {
        return Err(format!(
            "{RESULT_VECTOR_CANONICAL_RELATIVE} must contain exactly {} cases; found {}",
            EXPECTED_VECTOR_CASES.len(),
            vector.cases.len()
        ));
    }
    validate_unique(
        "SourceMaintenance result-vector case IDs",
        vector.cases.iter().map(|case| case.id.as_str()),
    )?;
    for (actual, expected) in vector.cases.iter().zip(EXPECTED_VECTOR_CASES) {
        if actual.id != expected.id
            || actual.execution != expected.execution
            || actual.authority != expected.authority
            || actual.authority_path != expected.authority_path
            || actual.resource.as_deref() != expected.resource
            || actual.boundary.as_deref() != expected.boundary
            || actual.expected_outcome != expected.expected_outcome
            || actual.error_domain.as_deref() != expected.error_domain
            || actual.expected_error.as_deref() != expected.expected_error
        {
            return Err(format!(
                "{RESULT_VECTOR_CANONICAL_RELATIVE} case `{}` does not match its exact governed expectation",
                expected.id
            ));
        }
        match (
            actual.error_domain.as_deref(),
            actual.expected_error.as_deref(),
        ) {
            (None, None) => {}
            (Some("typed" | "sqlite_database"), Some(error)) if !error.is_empty() => {}
            _ => {
                return Err(format!(
                    "{RESULT_VECTOR_CANONICAL_RELATIVE} case `{}` has inconsistent error domain",
                    actual.id
                ));
            }
        }
    }
    validate_delegated_test_authorities(workspace_root, vector)
}

#[derive(Clone, Copy)]
struct DelegatedAuthoritySpec {
    path: &'static str,
    test: &'static str,
    ordered_markers: &'static [&'static str],
}

const EXECUTABLE_AUTHORITY_AST_SHA256: &str =
    "b1a7658f47b4561ad816ef65dab47cf68c75de3c459383371319e96e6051d435";
const BOUND_AUTHORITY_SOURCE_AST_SHA256: &str =
    "ad05785d1e9ed452038080f3b95f3bc516a88ad659efe0353342468afb28fce3";

#[derive(Clone, Debug, Serialize)]
struct ExecutableAuthorityIdentity {
    path: String,
    test: String,
    tokens: String,
}

#[derive(Clone, Debug, Serialize)]
struct BoundAuthoritySourceIdentity {
    path: String,
    tokens: String,
}

const DELEGATED_AUTHORITIES: &[DelegatedAuthoritySpec] = &[
    DelegatedAuthoritySpec {
        path: "crates/event_store/src/store.rs",
        test: "exact_capacity_boundary_allows_duplicate_observation_and_ephemeral_noop",
        ordered_markers: &[
            "RADROOTS_EVENT_STORE_RAW_EVENT_COUNT_LIMIT_V1",
            "validate_source_capacity_authority_fast_v1",
            "duplicate.persistence.is_duplicate()",
            "SourceCapacityExceeded",
            "RadrootsEventPersistence::NotPersisted",
            "assert_eq!(ephemeral_observation_count,0)",
            "transaction.rollback().await",
        ],
    },
    DelegatedAuthoritySpec {
        path: "crates/event_store/src/store.rs",
        test: "borrowed_ingest_savepoint_rolls_back_post_core_authority_forge",
        ordered_markers: &[
            "priorcallerwork",
            "capacity_after_prior",
            "expect_err(\"post-corerawauthoritymutationmustfail\")",
            "MigrationHookStateDrift",
            "capacity_after_rollback,capacity_after_prior",
            "callermaycommitpriorworkafterfailedingest",
            "raw_event(&prior_event.id_hex())",
            "raw_event(&trigger_event.id_hex())",
            "raw_event(&forged_event.id_hex())",
            "trade_mutation_count,0",
            "transition_count,0",
            "capacity_after_prior",
        ],
    },
    DelegatedAuthoritySpec {
        path: "crates/event_store/src/source_maintenance_v1.rs",
        test: "every_prospective_capacity_dimension_accepts_exact_and_rejects_one_over",
        ordered_markers: &[
            "RadrootsEventStoreSourceCapacityResourceV1::RawEvents",
            "RadrootsEventStoreSourceCapacityResourceV1::RawTags",
            "RadrootsEventStoreSourceCapacityResourceV1::RawEventBytes",
            "RadrootsEventStoreSourceCapacityResourceV1::RawTagBytes",
            "capacity_with(resource,limit-1)",
            "delta_with(resource,1)",
            "capacity_with(resource,limit)",
            "SourceCapacityExceeded",
            "requested:1",
            "capacity_with(resource,limit+1)",
            "requested:0",
        ],
    },
    DelegatedAuthoritySpec {
        path: "crates/event_store/src/schema.rs",
        test: "v3_to_v4_under_limit_backfills_exact_capacity_and_preserves_source",
        ordered_markers: &[
            "&EVENT_STORE_MIGRATIONS[..3]",
            "event_envelopes",
            "active_generation",
            "RADROOTS_EVENT_STORE_SCHEMA_VERSION_CURRENT",
            "Managed{version:4}",
            "radroots_event_store_source_capacity_v1",
            "assert_eq!(capacity,(generation_before,1,0,event_bytes,0,1,8))",
            "preserved",
        ],
    },
    DelegatedAuthoritySpec {
        path: "crates/event_store/src/schema.rs",
        test: "v3_to_v4_rejects_prior_transition_drift_atomically",
        ordered_markers: &[
            "&EVENT_STORE_MIGRATIONS[..3]",
            "predecessor_trigger_sql",
            "corruption.commit().await",
            "expect_err(\"v4upgrademustnotrepaircorruptmanaged-v3hookstate\")",
            "MigrationHookStateDrift{hook_id:\"nip09_reconciliation_v1\"",
            "ledger_after,ledger_before",
            "v4_objects,0",
            "v4_ledger_rows,0",
            "schema_object_sql(&pool,name).await,*sql",
            "Managed{version:3}",
        ],
    },
    DelegatedAuthoritySpec {
        path: "crates/event_store/src/schema.rs",
        test: "source_capacity_is_rechecked_for_every_rebuild_bound_migration",
        ordered_markers: &[
            "raw_events:0",
            "UnledgeredBaseline",
            "&EVENT_STORE_MIGRATIONS[..3]",
            "expect_err(\"v4capacityexcessmustfail\")",
            "SourceCapacityExceeded",
            "Managed{version:3}",
            "radroots_event_store_source_capacity_v1",
            "assert_eq!(v4_object_count,0)",
            "assert_eq!(v4_ledger_count,0)",
        ],
    },
    DelegatedAuthoritySpec {
        path: "crates/event_store/src/schema.rs",
        test: "v4_rejects_persisted_legacy_ephemeral_rows_atomically",
        ordered_markers: &[
            "&EVENT_STORE_MIGRATIONS[..3]",
            "kind,tags_json,content",
            "20000",
            "begin_with(\"BEGINIMMEDIATE\")",
            "expect_err(\"persistedephemeralsourcemustrejectv4\")",
            "PersistedEphemeralRawEvent",
            "Managed{version:3}",
            "radroots_event_store_source_capacity_v1",
            "assert_eq!(v4_object_count,0)",
            "assert_eq!(v4_ledger_count,0)",
            "assert_eq!(raw_count,1)",
        ],
    },
    DelegatedAuthoritySpec {
        path: "crates/event_store/src/source_maintenance_v1.rs",
        test: "reopen_full_measure_detects_every_persisted_capacity_dimension",
        ordered_markers: &[
            "raw_event_count=1",
            "raw_tag_count=1",
            "raw_event_bytes=1",
            "raw_tag_bytes=1",
            "RadrootsEventStore::open_file(&path).await",
            "SourceCapacityStateDrift",
        ],
    },
    DelegatedAuthoritySpec {
        path: "crates/event_store/src/source_maintenance_v1.rs",
        test: "reopen_stops_at_the_first_raw_event_one_over_before_ephemeral_probe",
        ordered_markers: &[
            "value<25001",
            "CASEWHENvalue=25001THEN20000ELSE1END",
            "RadrootsEventStore::open_file(&path).await",
            "SourceCapacityExceeded",
            "current:25_000",
            "requested:1",
            "limit:25_000",
        ],
    },
    DelegatedAuthoritySpec {
        path: "crates/event_store/src/store.rs",
        test: "ninth_current_v4_rebuild_is_typed_and_preflight_atomic",
        ordered_markers: &[
            "forordinalin2_u8..=8",
            "assert_eq!(capacity_before.retained_generation_count(),8)",
            "PanickingGeneration",
            "expect_err(\"ninthrebuildmustfailbeforeentropyormutation\")",
            "SourceGenerationHistoryLimitReached{current:8,limit:8,}",
            "assert_eq!(source_authority_snapshot(&store).await,source_before)",
            "assert_eq!(raw_authority_digest(&store).await,raw_before)",
            "assert_eq!(normalized_nip09_snapshot(&store).await,nip09_before)",
            "assert_eq!(food_after,food_before)",
            "assert_eq!(derived_after,(derived_before.0,derived_before.1,derived_before.2,0))",
        ],
    },
    DelegatedAuthoritySpec {
        path: "crates/event_store/src/source_maintenance_v1.rs",
        test: "generation_sql_backstop_allows_exact_append_and_is_conflict_safe_one_over",
        ordered_markers: &[
            "retained_generation_count=retained_generation_limit-1",
            "exactgenerationboundarymustappend",
            "assert_eq!(retained_count,8)",
            "sourcegenerationalreadyexists",
            "uniquegenerationappendmusthittheSQLcapacitybackstop",
            "sqlx::Error::Database",
            "retainedsourcegenerationlimitreached",
            "transaction.rollback().await",
        ],
    },
    DelegatedAuthoritySpec {
        path: "crates/event_store/src/store.rs",
        test: "current_v4_rebuild_rotates_capacity_and_food_authority_end_to_end",
        ordered_markers: &[
            "apply_reconciliation_hook",
            "after.retained_generation_count()",
            "before.retained_generation_count()+1",
            "assert_eq!(marker_count,0)",
            "audit_food_availability_projection_v1",
        ],
    },
    DelegatedAuthoritySpec {
        path: "crates/event_store/src/source_maintenance_v1.rs",
        test: "marker_close_sql_backstop_rejects_each_required_seal_drift",
        ordered_markers: &[
            "rebuildmarkercannotclosebeforecapacity,NIP-09,andFoodAvailabilitysealsagree",
            "fordriftin[\"capacity\",\"nip09\",\"food\",\"fts\"]",
            "radroots_event_store_source_capacity_update_guard",
            "raw_event_bytes=raw_event_bytes+1",
            "radroots_event_store_addressable_feed_integrity_v1",
            "last_transition_seq=last_transition_seq+1",
            "radroots_event_store_food_availability_cursor_update_guard",
            "projected_row_count=projected_row_count+1",
            "radroots_event_store_food_availability_search_fts",
            "DELETEFROMradroots_event_store_source_rebuild_marker",
            "sqlx::Error::Database",
            "database.message()==MARKER_CLOSE_ERROR",
            "transaction.rollback().await",
        ],
    },
    DelegatedAuthoritySpec {
        path: "crates/event_store/src/schema.rs",
        test: "v4_marker_open_allows_repairing_prior_transition_high_water_drift",
        ordered_markers: &[
            "last_transition_seq=7",
            "validate_schema_fingerprint",
            "wrong_prior",
            "wrong_floor",
            "expect(\"derivedtransitiondriftisrepairableunderv4\")",
            "repaired.0.as_slice(),target_generation.as_slice()",
            "repaired.1,repaired.2",
            "repaired.1,0",
        ],
    },
    DelegatedAuthoritySpec {
        path: "crates/event_store/src/schema.rs",
        test: "v4_food_reset_requires_marker_rotation_and_preserves_target_rows",
        ordered_markers: &[
            "expect_err(\"marker-freeFoodresetmustfail\")",
            "expect_err(\"markeralonemustnotauthorizeFoodreset\")",
            "expect(\"rotatesourcestate\")",
            "expect(\"post-rotationhistoricalFoodreset\")",
            "expect_err(\"activetarget-generationFoodrowsmustremainguarded\")",
            "remaining,(1,1)",
        ],
    },
    DelegatedAuthoritySpec {
        path: "crates/event_store/src/schema.rs",
        test: "v4_down_restores_exact_predecessor_trigger_sql_and_fingerprint",
        ordered_markers: &[
            "&EVENT_STORE_MIGRATIONS[..3]",
            "predecessor_sql",
            "assert_ne!(schema_object_sql(&pool,name).await,predecessor_sql[*name])",
            "rollback_event_store_schema_with_registry",
            "assert_eq!(schema_object_sql(&pool,name).await,predecessor_sql[*name])",
            "Managed{version:3}",
        ],
    },
    DelegatedAuthoritySpec {
        path: "crates/event_store/src/store.rs",
        test: "open_file_rejects_utf16_main_database_before_schema_or_journal_mutation",
        ordered_markers: &[
            "initialize_utf16le_database(&path).await",
            "RadrootsEventStore::open_file(&path).await",
            "SqliteMainDatabaseEncodingNotUtf8{actual}",
            "actual==\"UTF-16le\"",
            "assert_utf16le_database_was_not_mutated(&path).await",
        ],
    },
    DelegatedAuthoritySpec {
        path: "crates/event_store/src/store.rs",
        test: "open_pool_rejects_utf16_main_database_before_schema_or_journal_mutation",
        ordered_markers: &[
            "initialize_utf16le_database(&path).await",
            "max_connections(2)",
            "RadrootsEventStore::open_pool(pool,true).await",
            "SqliteMainDatabaseEncodingNotUtf8{actual}",
            "actual==\"UTF-16le\"",
            "assert_utf16le_database_was_not_mutated(&path).await",
        ],
    },
    DelegatedAuthoritySpec {
        path: "crates/event_store/src/store.rs",
        test: "utf8_file_reopen_preserves_non_ascii_and_nul_capacity_accounting",
        ordered_markers: &[
            "letexpected_tag_count=",
            "raw_source_text_bytes(&event)",
            "expect(\"non-ASCIIandNULingest\")",
            "before_reopen.raw_event_count(),1",
            "before_reopen.raw_tag_count(),expected_tag_count",
            "before_reopen.raw_event_text_bytes(),expected_event_bytes",
            "before_reopen.raw_tag_text_bytes(),expected_tag_bytes",
            "store.pool().close().await",
            "RadrootsEventStore::open_file(&path).await",
            "source_capacity_v1()",
            "before_reopen",
            "raw_event(&event_id)",
            "tags_for_event(&event_id)",
            "assert_eq!(tags.len(),2)",
        ],
    },
    DelegatedAuthoritySpec {
        path: "crates/event_store/src/schema.rs",
        test: "rollback_rejects_below_floor_ahead_unmanaged_and_generation_destructive_targets",
        ordered_markers: &[
            "lethistory_before:Vec<(Vec<u8>,i64)>=",
            "rollback_event_store_schema_offline(&managed,1).await",
            "RollbackWouldDiscardSourceGenerationHistory{current:RADROOTS_EVENT_STORE_SCHEMA_VERSION_CURRENT,target:1,floor:2,}",
            "inspect_event_store_schema_status(&managed).await",
            "Managed{version:RADROOTS_EVENT_STORE_SCHEMA_VERSION_CURRENT,}",
            "source-generationhistoryafterrejectedrollback",
            "history_before",
        ],
    },
    DelegatedAuthoritySpec {
        path: "crates/event_store/src/schema.rs",
        test: "rollback_cannot_bypass_generation_history_guard_through_version_three",
        ordered_markers: &[
            "rollback_event_store_schema_offline(&managed,3).await",
            "lethistory_before:Vec<(Vec<u8>,i64)>=",
            "rollback_event_store_schema_offline(&managed,1).await",
            "RollbackWouldDiscardSourceGenerationHistory{current:3,target:1,floor:2,}",
            "inspect_event_store_schema_status(&managed).await",
            "Managed{version:3}",
            "v3source-generationhistoryafterrejectedbypass",
            "history_before",
        ],
    },
    DelegatedAuthoritySpec {
        path: "crates/event_store/src/store.rs",
        test: "independent_file_pools_serialize_the_last_raw_event_byte_capacity_slot",
        ordered_markers: &[
            "RADROOTS_EVENT_STORE_RAW_EVENT_TEXT_BYTES_LIMIT_V1",
            "before_race.raw_event_text_bytes(),filler_target",
            "Barrier::new(3)",
            "tokio::spawn",
            "tokio::spawn",
            "tokio::join!",
            "(Ok(accepted),Err(rejected))|(Err(rejected),Ok(accepted))",
            "accepted.persistence.is_inserted()",
            "SourceCapacityExceeded{resource:crate::RadrootsEventStoreSourceCapacityResourceV1::RawEventBytes",
            "requested==contender_bytes",
            "cleanfullreopenafterlast-slotrace",
            "after_race.raw_event_count(),filler_count+1",
            "after_race.raw_event_text_bytes(),crate::RADROOTS_EVENT_STORE_RAW_EVENT_TEXT_BYTES_LIMIT_V1",
            "assert_eq!(retained_contenders,1)",
        ],
    },
];

const MANDATORY_BOUND_AUTHORITIES: &[DelegatedAuthoritySpec] = &[
    DelegatedAuthoritySpec {
        path: "crates/event_store/src/nip09/reconciliation_v1.rs",
        test: "bounded_capacity_page_len_caps_gross_source_probe_at_one_over",
        ordered_markers: &[
            "forlimitin[25_000_u64,250_000_u64]",
            "bounded_capacity_page_len(current,limit)",
            "(1..=RECONCILIATION_SNAPSHOT_BATCH_LEN).contains(&fetched_len)",
            "assert_eq!(fetched,limit+1)",
            "bounded_capacity_page_len(24_576,25_000),(425,425)",
            "bounded_capacity_page_len(249_856,250_000),(145,145)",
            "bounded_capacity_page_len(25_000,25_000),(1,1)",
        ],
    },
    DelegatedAuthoritySpec {
        path: "crates/event_store/src/source_maintenance_v1.rs",
        test: "generation_append_limit_returns_the_typed_current_and_limit",
        ordered_markers: &[
            "validate_source_generation_append_available_v1(7,8)",
            "validate_source_generation_append_available_v1(8,8)",
            "SourceGenerationHistoryLimitReached{current:8,limit:8,}",
        ],
    },
    DelegatedAuthoritySpec {
        path: "crates/event_store/src/source_maintenance_v1.rs",
        test: "retained_generation_nip09_logical_rows_have_an_audited_upper_bound",
        ordered_markers: &[
            "RADROOTS_EVENT_STORE_RAW_EVENT_COUNT_LIMIT_V1",
            "RADROOTS_EVENT_STORE_RAW_TAG_COUNT_LIMIT_V1",
            "letper_generation=",
            "4*events+2*tags+2",
            "RADROOTS_EVENT_STORE_RETAINED_SOURCE_GENERATION_LIMIT_V1",
            "4_800_016",
            "EVENT_STORE_MIGRATIONS",
        ],
    },
];

const MANDATORY_BOUND_HELPER_AUTHORITIES: &[DelegatedAuthoritySpec] = &[DelegatedAuthoritySpec {
    path: "crates/event_store/src/store.rs",
    test: "assert_utf16le_database_was_not_mutated",
    ordered_markers: &[
        "PRAGMAmain.encoding",
        "PRAGMAmain.journal_mode",
        "SELECTCOUNT(*)FROMmain.sqlite_schemaWHEREname='radroots_event_store_schema_migrations'ORname='event_envelopes'ORnameLIKE'radroots_event_store_%'",
        "assert_eq!(encoding,\"UTF-16le\")",
        "assert_eq!(journal_mode,\"delete\")",
        "assert_eq!(event_store_objects,0)",
    ],
}];

fn validate_delegated_test_authorities(
    workspace_root: &Path,
    vector: &SourceMaintenanceVector,
) -> Result<(), String> {
    let delegated = vector
        .cases
        .iter()
        .filter(|case| case.execution != DIRECT_EXECUTOR)
        .map(|case| (case.authority_path.as_str(), case.authority.as_str()))
        .collect::<BTreeSet<_>>();
    let expected = DELEGATED_AUTHORITIES
        .iter()
        .map(|spec| (spec.path, spec.test))
        .collect::<BTreeSet<_>>();
    if delegated != expected {
        return Err(format!(
            "SourceMaintenance delegated-test inventory differs: expected {expected:?}, found {delegated:?}"
        ));
    }
    let mut executable_identities = Vec::new();
    for spec in DELEGATED_AUTHORITIES
        .iter()
        .chain(MANDATORY_BOUND_AUTHORITIES)
    {
        executable_identities.push(ExecutableAuthorityIdentity {
            path: spec.path.to_owned(),
            test: spec.test.to_owned(),
            tokens: validate_delegated_authority(workspace_root, *spec)?,
        });
    }
    for spec in MANDATORY_BOUND_HELPER_AUTHORITIES {
        executable_identities.push(ExecutableAuthorityIdentity {
            path: spec.path.to_owned(),
            test: spec.test.to_owned(),
            tokens: validate_bound_function_authority(workspace_root, *spec)?,
        });
    }

    let executor_source = rust_source(workspace_root, RESULT_VECTOR_EXECUTOR_RELATIVE)?;
    let executor = syn::parse_file(&executor_source)
        .map_err(|error| format!("parse {RESULT_VECTOR_EXECUTOR_RELATIVE}: {error}"))?;
    let executor = exact_free_function(&executor, RESULT_VECTOR_EXECUTOR_TEST)?;
    validate_executable_test_authority(
        RESULT_VECTOR_EXECUTOR_RELATIVE,
        RESULT_VECTOR_EXECUTOR_TEST,
        executor,
    )?;
    let executor = compact_tokens(executor);
    for case in vector
        .cases
        .iter()
        .filter(|case| case.execution == DIRECT_EXECUTOR)
    {
        require_marker(
            "SourceMaintenance direct result-vector executor",
            &executor,
            case.id.as_str(),
        )?;
    }
    require_marker(
        "SourceMaintenance direct result-vector executor",
        &executor,
        "assert_direct_cases_executed",
    )?;
    executable_identities.push(ExecutableAuthorityIdentity {
        path: RESULT_VECTOR_EXECUTOR_RELATIVE.to_owned(),
        test: RESULT_VECTOR_EXECUTOR_TEST.to_owned(),
        tokens: executor,
    });
    validate_executable_authority_identities(&executable_identities)?;
    let source_identities = bound_authority_source_identities(workspace_root)?;
    validate_bound_authority_source_identities(&source_identities)?;
    Ok(())
}

fn validate_delegated_authority(
    workspace_root: &Path,
    spec: DelegatedAuthoritySpec,
) -> Result<String, String> {
    let source = rust_source(workspace_root, spec.path)?;
    let syntax = syn::parse_file(&source)
        .map_err(|error| format!("parse delegated authority {}: {error}", spec.path))?;
    let function = exact_free_function(&syntax, spec.test)?;
    validate_executable_test_authority(spec.path, spec.test, function)?;
    let function = compact_tokens(function);
    require_ordered_markers(
        &format!("delegated authority {}::{}", spec.path, spec.test),
        &function,
        spec.ordered_markers,
    )?;
    Ok(function)
}

fn validate_executable_authority_identities(
    identities: &[ExecutableAuthorityIdentity],
) -> Result<(), String> {
    let bytes = canonical_json_bytes(&identities)?;
    let actual = sha256_hex(&bytes);
    if actual != EXECUTABLE_AUTHORITY_AST_SHA256 {
        return Err(format!(
            "SourceMaintenance executable direct/delegated authority AST identity drifted: expected {EXECUTABLE_AUTHORITY_AST_SHA256}, found {actual}"
        ));
    }
    Ok(())
}

fn bound_authority_source_identities(
    workspace_root: &Path,
) -> Result<Vec<BoundAuthoritySourceIdentity>, String> {
    let paths = DELEGATED_AUTHORITIES
        .iter()
        .chain(MANDATORY_BOUND_AUTHORITIES)
        .chain(MANDATORY_BOUND_HELPER_AUTHORITIES)
        .map(|spec| spec.path)
        .chain([RESULT_VECTOR_EXECUTOR_RELATIVE])
        .collect::<BTreeSet<_>>();
    paths
        .into_iter()
        .map(|path| {
            let source = rust_source(workspace_root, path)?;
            let file = syn::parse_file(&source)
                .map_err(|error| format!("parse bound authority source {path}: {error}"))?;
            Ok(BoundAuthoritySourceIdentity {
                path: path.to_owned(),
                tokens: compact_tokens(&file),
            })
        })
        .collect()
}

fn validate_bound_authority_source_identities(
    identities: &[BoundAuthoritySourceIdentity],
) -> Result<(), String> {
    let bytes = canonical_json_bytes(&identities)?;
    let actual = sha256_hex(&bytes);
    if actual != BOUND_AUTHORITY_SOURCE_AST_SHA256 {
        return Err(format!(
            "SourceMaintenance bound executor/test-module/helper source AST identity drifted: expected {BOUND_AUTHORITY_SOURCE_AST_SHA256}, found {actual}"
        ));
    }
    Ok(())
}

#[derive(Default)]
struct EarlyReturnAudit {
    count: usize,
}

impl<'ast> syn::visit::Visit<'ast> for EarlyReturnAudit {
    fn visit_expr_return(&mut self, expression: &'ast syn::ExprReturn) {
        self.count += 1;
        syn::visit::visit_expr_return(self, expression);
    }
}

fn validate_executable_test_authority(
    relative: &str,
    name: &str,
    function: &syn::ItemFn,
) -> Result<(), String> {
    let expected_attribute = if function.sig.asyncness.is_some() {
        "#[tokio::test]"
    } else {
        "#[test]"
    };
    let attributes = function
        .attrs
        .iter()
        .map(compact_tokens)
        .collect::<Vec<_>>();
    if attributes != [expected_attribute] {
        return Err(format!(
            "executable test authority {relative}::{name} must have exactly `{expected_attribute}` and no disabling or conditional attributes; found {attributes:?}"
        ));
    }
    if !matches!(function.vis, syn::Visibility::Inherited)
        || function.sig.constness.is_some()
        || function.sig.unsafety.is_some()
        || function.sig.abi.is_some()
        || !function.sig.inputs.is_empty()
        || !function.sig.generics.params.is_empty()
        || function.sig.generics.where_clause.is_some()
        || !matches!(function.sig.output, syn::ReturnType::Default)
        || function.sig.variadic.is_some()
    {
        return Err(format!(
            "executable test authority {relative}::{name} must remain a private zero-argument test with no generic, ABI, unsafe, variadic, or return-type escape"
        ));
    }

    use syn::visit::Visit;
    let mut returns = EarlyReturnAudit::default();
    returns.visit_block(&function.block);
    if returns.count != 0 {
        return Err(format!(
            "executable test authority {relative}::{name} must not contain early return control flow; found {} return expression(s)",
            returns.count
        ));
    }
    Ok(())
}

fn validate_bound_function_authority(
    workspace_root: &Path,
    spec: DelegatedAuthoritySpec,
) -> Result<String, String> {
    let source = rust_source(workspace_root, spec.path)?;
    let syntax = syn::parse_file(&source)
        .map_err(|error| format!("parse bound authority {}: {error}", spec.path))?;
    let function = exact_free_function_tokens(&syntax, spec.test)?;
    require_ordered_markers(
        &format!("bound authority {}::{}", spec.path, spec.test),
        &function,
        spec.ordered_markers,
    )?;
    Ok(function)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn workspace_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("xtask lives under tools/ in the workspace")
            .to_path_buf()
    }

    fn public_api_sources(root: &Path) -> (String, String, String, String, String) {
        (
            rust_source(root, "crates/event_store/src/model.rs").expect("model source"),
            rust_source(root, "crates/event_store/src/lib.rs").expect("lib source"),
            rust_source(root, "crates/event_store/src/error.rs").expect("error source"),
            rust_source(root, "crates/event_store/src/source_maintenance_v1.rs")
                .expect("maintenance source"),
            rust_source(root, "crates/event_store/src/store.rs").expect("store source"),
        )
    }

    #[test]
    fn source_inventory_excludes_outputs_and_exactly_supersedes_predecessors() {
        let root = workspace_root();
        validate_source_inventory().expect("source inventory");
        validate_predecessor_production_source_coverage(&root)
            .expect("exact predecessor supersession");

        let source_paths = SOURCE_SPECS
            .iter()
            .map(|spec| spec.path)
            .collect::<BTreeSet<_>>();
        for generated in GENERATED_ARTIFACT_PATHS {
            assert!(!source_paths.contains(generated));
        }
        assert_eq!(
            PREDECESSOR_SUPERSEDED_SOURCE_PATHS
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
                .len(),
            PREDECESSOR_SUPERSEDED_SOURCE_PATHS.len()
        );
    }

    #[test]
    fn public_surface_rejects_renames_and_retired_api_reintroduction() {
        let root = workspace_root();
        let (model, lib, error, maintenance, store) = public_api_sources(&root);
        validate_public_api_sources(&model, &lib, &error, &maintenance, &store)
            .expect("current public API");

        let renamed_model = model.replacen(
            "RadrootsAddressableTransitionCauseV1,",
            "RadrootsAddressableTransitionCauseV1 as RenamedCause,",
            1,
        );
        let renamed_error =
            validate_public_api_sources(&renamed_model, &lib, &error, &maintenance, &store)
                .expect_err("renamed inherited symbol must fail");
        assert!(renamed_error.contains("direct, non-renamed"));

        let retired_type =
            format!("{error}\npub struct RadrootsEventStoreReconciliationResource;\n");
        let retired_type_error =
            validate_public_api_sources(&model, &lib, &retired_type, &maintenance, &store)
                .expect_err("retired public type must fail");
        assert!(retired_type_error.contains("must remain absent"));

        let retired_variant = error.replacen(
            "pub enum RadrootsEventStoreError {",
            "pub enum RadrootsEventStoreError {\n    ReconciliationCapacityExceeded,",
            1,
        );
        let retired_variant_error =
            validate_error_and_limit_source(&retired_variant).expect_err("retired variant");
        assert!(retired_variant_error.contains("must remain absent"));
    }

    #[test]
    fn manifest_schema_rejects_unknown_fields() {
        let root = workspace_root();
        let schema = manifest_schema();
        let manifest_bytes =
            read_regular_file(&root, MANIFEST_RELATIVE).expect("generated manifest bytes");
        let mut manifest: Value =
            serde_json::from_slice(&manifest_bytes).expect("generated manifest JSON");
        validate_manifest_json_schema(&schema, &manifest).expect("current manifest schema");

        manifest
            .pointer_mut("/source_maintenance/reopen_validation")
            .and_then(Value::as_object_mut)
            .expect("reopen validation object")
            .insert("unbounded_scan".to_owned(), Value::Bool(true));
        let error = validate_manifest_json_schema(&schema, &manifest)
            .expect_err("unknown reopen field must fail");
        assert!(error.contains("violates"));
    }

    #[test]
    fn generated_bundle_render_is_rerunnable_without_byte_drift() {
        let root = workspace_root();
        let before = expected_artifacts(&root)
            .expect("first in-memory generated bundle render")
            .into_iter()
            .map(|artifact| (artifact.relative, artifact.contents))
            .collect::<Vec<_>>();
        let after = expected_artifacts(&root)
            .expect("second in-memory generated bundle render")
            .into_iter()
            .map(|artifact| (artifact.relative, artifact.contents))
            .collect::<Vec<_>>();
        assert_eq!(before, after);
    }

    fn current_executable_authority_identities(root: &Path) -> Vec<ExecutableAuthorityIdentity> {
        let mut identities = DELEGATED_AUTHORITIES
            .iter()
            .chain(MANDATORY_BOUND_AUTHORITIES)
            .map(|spec| ExecutableAuthorityIdentity {
                path: spec.path.to_owned(),
                test: spec.test.to_owned(),
                tokens: validate_delegated_authority(root, *spec)
                    .expect("current delegated executable authority"),
            })
            .collect::<Vec<_>>();
        identities.extend(MANDATORY_BOUND_HELPER_AUTHORITIES.iter().map(|spec| {
            ExecutableAuthorityIdentity {
                path: spec.path.to_owned(),
                test: spec.test.to_owned(),
                tokens: validate_bound_function_authority(root, *spec)
                    .expect("current bound helper authority"),
            }
        }));
        let source = rust_source(root, RESULT_VECTOR_EXECUTOR_RELATIVE)
            .expect("direct result-vector executor source");
        let file = syn::parse_file(&source).expect("direct result-vector executor AST");
        let function = exact_free_function(&file, RESULT_VECTOR_EXECUTOR_TEST)
            .expect("direct result-vector executor function");
        validate_executable_test_authority(
            RESULT_VECTOR_EXECUTOR_RELATIVE,
            RESULT_VECTOR_EXECUTOR_TEST,
            function,
        )
        .expect("current direct executable authority");
        identities.push(ExecutableAuthorityIdentity {
            path: RESULT_VECTOR_EXECUTOR_RELATIVE.to_owned(),
            test: RESULT_VECTOR_EXECUTOR_TEST.to_owned(),
            tokens: compact_tokens(function),
        });
        identities
    }

    fn assert_bound_source_mutation_rejected(
        baseline: &[BoundAuthoritySourceIdentity],
        path: &str,
        source: &str,
        label: &str,
    ) {
        let mut identities = baseline.to_vec();
        let identity = identities
            .iter_mut()
            .find(|identity| identity.path == path)
            .unwrap_or_else(|| panic!("missing bound source identity for {path}"));
        let file = syn::parse_file(source)
            .unwrap_or_else(|error| panic!("parse {label} mutation for {path}: {error}"));
        let tokens = compact_tokens(&file);
        assert_ne!(tokens, identity.tokens, "{label} fixture must mutate");
        identity.tokens = tokens;
        let error = validate_bound_authority_source_identities(&identities)
            .expect_err("bound authority source-context drift must fail closed");
        assert!(
            error.contains("bound executor/test-module/helper source AST identity drifted"),
            "unexpected {label} error: {error}"
        );
    }

    #[test]
    fn bound_executor_and_test_module_sources_are_exact_ast_authority() {
        let root = workspace_root();
        let baseline =
            bound_authority_source_identities(&root).expect("current bound source identities");
        validate_bound_authority_source_identities(&baseline)
            .expect("current bound source aggregate identity");

        let executor = rust_source(&root, RESULT_VECTOR_EXECUTOR_RELATIVE)
            .expect("direct result-vector executor source");
        let crate_disabled = executor.replacen(
            "#![forbid(unsafe_code)]",
            "#![forbid(unsafe_code)]\n#![cfg(any())]",
            1,
        );
        assert_bound_source_mutation_rejected(
            &baseline,
            RESULT_VECTOR_EXECUTOR_RELATIVE,
            &crate_disabled,
            "crate-disabled direct executor",
        );

        let delegated_path = "crates/event_store/src/source_maintenance_v1.rs";
        let delegated =
            rust_source(&root, delegated_path).expect("delegated authority module source");
        let module_disabled = delegated.replacen(
            "#[cfg(test)]\nmod tests {",
            "#[cfg(all(test, any()))]\nmod tests {",
            1,
        );
        assert_bound_source_mutation_rejected(
            &baseline,
            delegated_path,
            &module_disabled,
            "disabled delegated test module",
        );

        let macro_shadowed = executor.replacen(
            "#![forbid(unsafe_code)]",
            "#![forbid(unsafe_code)]\nmacro_rules! assert_eq { ($($tokens:tt)*) => {}; }",
            1,
        );
        assert_bound_source_mutation_rejected(
            &baseline,
            RESULT_VECTOR_EXECUTOR_RELATIVE,
            &macro_shadowed,
            "shadowing direct-executor assertion macro",
        );
    }

    #[test]
    fn executable_authority_rejects_disabled_missing_and_unreachable_tests() {
        let root = workspace_root();
        let source = rust_source(&root, RESULT_VECTOR_EXECUTOR_RELATIVE)
            .expect("direct result-vector executor source");
        for (label, mutation) in [
            (
                "ignored direct executor",
                source.replacen("#[tokio::test]", "#[ignore]\n#[tokio::test]", 1),
            ),
            (
                "missing direct executor attribute",
                source.replacen("#[tokio::test]\n", "", 1),
            ),
            (
                "early-returning direct executor",
                source.replacen(
                    "async fn source_maintenance_v1_result_vector() {",
                    "async fn source_maintenance_v1_result_vector() {\n    return;",
                    1,
                ),
            ),
        ] {
            assert_ne!(mutation, source, "{label} fixture must mutate");
            let file = syn::parse_file(&mutation).expect("mutated direct executor AST");
            let function = exact_free_function(&file, RESULT_VECTOR_EXECUTOR_TEST)
                .expect("mutated direct executor function");
            let error = validate_executable_test_authority(
                RESULT_VECTOR_EXECUTOR_RELATIVE,
                RESULT_VECTOR_EXECUTOR_TEST,
                function,
            )
            .expect_err("disabled or early-returning direct executor must fail closed");
            assert!(
                error.contains("executable test authority"),
                "unexpected {label} error: {error}"
            );
        }

        let mut identities = current_executable_authority_identities(&root);
        validate_executable_authority_identities(&identities)
            .expect("current aggregate executable identity");
        let direct = identities
            .last_mut()
            .expect("direct executable identity is terminal");
        let file = syn::parse_file(&source).expect("direct executor AST");
        let function = exact_free_function(&file, RESULT_VECTOR_EXECUTOR_TEST)
            .expect("direct executor function");
        let mut function = function.clone();
        let body = function.block.clone();
        *function.block = syn::parse_quote!({ if false #body; });
        direct.tokens = compact_tokens(&function);
        let error = validate_executable_authority_identities(&identities)
            .expect_err("non-returning unreachable test body must fail exact AST authority");
        assert!(error.contains("executable direct/delegated authority AST identity drifted"));

        let mut identities = current_executable_authority_identities(&root);
        let helper_spec = MANDATORY_BOUND_HELPER_AUTHORITIES[0];
        let helper_source = rust_source(&root, helper_spec.path).expect("bound helper source");
        let helper_file = syn::parse_file(&helper_source).expect("bound helper AST");
        let helper = exact_free_function(&helper_file, helper_spec.test).expect("bound helper");
        let mut bypass = helper.clone();
        let body = bypass.block.clone();
        *bypass.block = syn::parse_quote!({ if false #body; });
        let identity = identities
            .iter_mut()
            .find(|identity| identity.path == helper_spec.path && identity.test == helper_spec.test)
            .expect("bound helper identity");
        identity.tokens = compact_tokens(&bypass);
        validate_executable_authority_identities(&identities)
            .expect_err("unreachable bound helper body must fail exact AST authority");
    }

    #[test]
    fn command_and_release_validation_reachability_is_exact() {
        let root = workspace_root();
        let contract =
            rust_source(&root, CONTRACT_COMMAND_SOURCE_RELATIVE).expect("contract command source");
        let main = rust_source(&root, XTASK_MAIN_SOURCE_RELATIVE).expect("xtask main source");
        validate_contract_command_reachability_sources(&contract, &main)
            .expect("current aggregate and release validation reachability");

        let mutations = [
            (
                "aggregate removal",
                contract.replacen(
                    "    validate_source_maintenance_manifest(workspace_root)?;\n",
                    "",
                    1,
                ),
                main.clone(),
            ),
            (
                "aggregate discarded result",
                contract.replacen(
                    "    validate_source_maintenance_manifest(workspace_root)?;",
                    "    let _ = validate_source_maintenance_manifest(workspace_root);",
                    1,
                ),
                main.clone(),
            ),
            (
                "aggregate reordering",
                contract.replacen(
                    "    validate_nip09_reconciliation_manifest(workspace_root)?;\n    validate_source_maintenance_manifest(workspace_root)?;",
                    "    validate_source_maintenance_manifest(workspace_root)?;\n    validate_nip09_reconciliation_manifest(workspace_root)?;",
                    1,
                ),
                main.clone(),
            ),
            (
                "contract validation removal",
                contract.clone(),
                main.replacen(
                    "        .and_then(|_| contract::validate_artifact_contracts(&root))",
                    "        .map(|_| ())",
                    1,
                ),
            ),
            (
                "contract protocol freshness removal",
                contract.clone(),
                main.replacen("    generate::protocol::check(&root)?;\n", "", 1),
            ),
            (
                "contract protocol freshness discarded result",
                contract.clone(),
                main.replacen(
                    "    generate::protocol::check(&root)?;",
                    "    let _ = generate::protocol::check(&root);",
                    1,
                ),
            ),
            (
                "contract protocol freshness reordering",
                contract.clone(),
                main.replacen(
                    "    dto_roots::check(&root)?;\n    generate::protocol::check(&root)?;",
                    "    generate::protocol::check(&root)?;\n    dto_roots::check(&root)?;",
                    1,
                ),
            ),
            (
                "contract validate dispatch bypass",
                contract.clone(),
                main.replacen(
                    "        Some(\"validate\") => validate_contract(),",
                    "        Some(\"validate\") => Ok(()),",
                    1,
                ),
            ),
            (
                "release preflight dispatch bypass",
                contract.clone(),
                main.replacen(
                    "        Some(\"preflight\") => release_preflight(),",
                    "        Some(\"preflight\") => Ok(()),",
                    1,
                ),
            ),
            (
                "release validation removal",
                contract.clone(),
                main.replacen("    contract::validate_artifact_contracts(root)?;\n", "", 1),
            ),
            (
                "release protocol freshness removal",
                contract.clone(),
                main.replacen("    generate::protocol::check(root)?;\n", "", 1),
            ),
            (
                "release protocol freshness discarded result",
                contract.clone(),
                main.replacen(
                    "    generate::protocol::check(root)?;",
                    "    let _ = generate::protocol::check(root);",
                    1,
                ),
            ),
            (
                "release protocol freshness reordering",
                contract.clone(),
                main.replacen(
                    "    dto_roots::check(root)?;\n    generate::protocol::check(root)?;",
                    "    generate::protocol::check(root)?;\n    dto_roots::check(root)?;",
                    1,
                ),
            ),
            (
                "release validation discarded result",
                contract.clone(),
                main.replacen(
                    "    contract::validate_artifact_contracts(root)?;",
                    "    let _ = contract::validate_artifact_contracts(root);",
                    1,
                ),
            ),
            (
                "release validation reordering",
                contract.clone(),
                main.replacen(
                    "    generate::protocol::check(root)?;\n    contract::validate_artifact_contracts(root)?;",
                    "    contract::validate_artifact_contracts(root)?;\n    generate::protocol::check(root)?;",
                    1,
                ),
            ),
        ];
        for (label, contract_mutation, main_mutation) in mutations {
            assert!(
                contract_mutation != contract || main_mutation != main,
                "{label} fixture must mutate"
            );
            let error =
                validate_contract_command_reachability_sources(&contract_mutation, &main_mutation)
                    .expect_err("validation reachability drift must fail closed");
            assert!(
                error.contains("validation call-path authority drifted")
                    || error.contains("full dispatch AST authority drifted"),
                "unexpected {label} error: {error}"
            );
        }
    }

    #[test]
    fn capacity_snapshot_struct_and_public_accessors_are_exact_authority() {
        let root = workspace_root();
        let source = rust_source(&root, "crates/event_store/src/source_maintenance_v1.rs")
            .expect("source-capacity snapshot authority");
        let baseline = syn::parse_file(&source).expect("source-capacity snapshot AST");
        validate_source_capacity_snapshot_authority(&baseline)
            .expect("current source-capacity snapshot authority");

        for (label, needle, replacement) in [
            (
                "snapshot derive",
                "#[derive(Clone, Copy, Debug, PartialEq, Eq)]\npub struct RadrootsEventStoreSourceCapacityV1",
                "#[derive(Clone, Debug, PartialEq, Eq)]\npub struct RadrootsEventStoreSourceCapacityV1",
            ),
            (
                "private field type",
                "    raw_high_water_seq: i64,\n    retained_generation_count: u32,",
                "    raw_high_water_seq: u64,\n    retained_generation_count: u32,",
            ),
            (
                "public accessor signature",
                "    pub const fn raw_event_count(&self) -> u64 {",
                "    pub fn raw_event_count(&mut self) -> u64 {",
            ),
            (
                "public accessor body",
                "    pub const fn raw_event_count(&self) -> u64 {\n        self.capacity.raw_events\n    }",
                "    pub const fn raw_event_count(&self) -> u64 {\n        self.capacity.raw_tags\n    }",
            ),
        ] {
            let mutation = source.replacen(needle, replacement, 1);
            assert_ne!(mutation, source, "{label} fixture must mutate");
            let file = syn::parse_file(&mutation).expect("mutated source-capacity snapshot AST");
            let error = validate_source_capacity_snapshot_authority(&file)
                .expect_err("source-capacity snapshot drift must fail closed");
            assert!(
                error.contains("RadrootsEventStoreSourceCapacityV1"),
                "unexpected {label} error: {error}"
            );
        }
    }

    #[test]
    fn capacity_resource_and_all_typed_errors_are_exact_authority() {
        let root = workspace_root();
        let source = rust_source(&root, "crates/event_store/src/error.rs")
            .expect("event-store error authority");
        validate_error_and_limit_source(&source).expect("current capacity/error authority");
        for (label, needle, replacement) in [
            (
                "conditional duplicate capacity constant",
                "pub const RADROOTS_EVENT_STORE_RAW_EVENT_COUNT_LIMIT_V1: u64 = 25_000;",
                "#[cfg(any())]\npub const RADROOTS_EVENT_STORE_RAW_EVENT_COUNT_LIMIT_V1: u64 = 25_000;\n#[cfg(not(any()))]\npub const RADROOTS_EVENT_STORE_RAW_EVENT_COUNT_LIMIT_V1: u64 = 1;",
            ),
            (
                "conditional duplicate error enum",
                "#[derive(Debug, thiserror::Error)]\npub enum RadrootsEventStoreError {",
                "#[cfg(any())]\npub enum RadrootsEventStoreError {}\n\n#[derive(Debug, thiserror::Error)]\npub enum RadrootsEventStoreError {",
            ),
            (
                "capacity constant value",
                "pub const RADROOTS_EVENT_STORE_RAW_TAG_COUNT_LIMIT_V1: u64 = 250_000;",
                "pub const RADROOTS_EVENT_STORE_RAW_TAG_COUNT_LIMIT_V1: u64 = 249_999;",
            ),
            ("resource non-exhaustive", "#[non_exhaustive]", ""),
            ("resource variant", "    RawTagBytes,", "    TagBytes,"),
            ("resource label", "\"raw tag count\"", "\"raw tags\""),
            (
                "capacity requested type",
                "        requested: u64,\n        limit: u64,",
                "        requested: u32,\n        limit: u64,",
            ),
            (
                "generation history type",
                "    SourceGenerationHistoryLimitReached { current: u32, limit: u32 },",
                "    SourceGenerationHistoryLimitReached { current: u64, limit: u32 },",
            ),
            (
                "ephemeral kind type",
                "    PersistedEphemeralRawEvent { event_id: String, kind: i64 },",
                "    PersistedEphemeralRawEvent { event_id: String, kind: u64 },",
            ),
            (
                "capacity drift reason type",
                "    SourceCapacityStateDrift { reason: String },",
                "    SourceCapacityStateDrift { reason: &'static str },",
            ),
            (
                "UTF-8 diagnostic type",
                "    SqliteMainDatabaseEncodingNotUtf8 { actual: String },",
                "    SqliteMainDatabaseEncodingNotUtf8 { actual: &'static str },",
            ),
            (
                "rollback floor type",
                "        floor: u32,\n    },\n    #[error(\"event-store rollback requires a managed schema\")]",
                "        floor: u64,\n    },\n    #[error(\"event-store rollback requires a managed schema\")]",
            ),
            (
                "capacity error display",
                "requested additional {requested}",
                "requested {requested}",
            ),
        ] {
            let mutation = source.replacen(needle, replacement, 1);
            assert_ne!(mutation, source, "{label} fixture must mutate");
            let error = validate_error_and_limit_source(&mutation)
                .expect_err("typed capacity/error authority drift must fail closed");
            assert!(!error.is_empty(), "{label} must return a diagnostic");
        }
    }
}
