#![forbid(unsafe_code)]

use nostr::{EventBuilder, Keys, Kind, SecretKey, Tag, TagKind, Timestamp};
use radroots_event_store::{
    RADROOTS_EVENT_STORE_RAW_EVENT_COUNT_LIMIT_V1,
    RADROOTS_EVENT_STORE_RAW_EVENT_TEXT_BYTES_LIMIT_V1,
    RADROOTS_EVENT_STORE_RAW_TAG_COUNT_LIMIT_V1, RADROOTS_EVENT_STORE_RAW_TAG_TEXT_BYTES_LIMIT_V1,
    RADROOTS_EVENT_STORE_RETAINED_SOURCE_GENERATION_LIMIT_V1, RadrootsEventIngest,
    RadrootsEventStore,
};
use serde::Deserialize;
use std::collections::BTreeSet;

const RESULT_VECTOR_EXECUTOR_ID: &str =
    "radroots_event_store.source_maintenance_v1.result_vector_executor.v1";
const RESULT_VECTOR_BYTES: &[u8] =
    include_bytes!("../../../contracts/conformance/vectors/event_store/source_maintenance.v1.json");
const FIXTURE_SECRET_KEY_HEX: &str =
    "10c5304d6c9ae3a1a16f7860f1cc8f5e3a76225a2663b3a989a0d775919b7df5";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceMaintenanceVector {
    schema_version: u32,
    contract_id: String,
    capacity_version: u32,
    limits: CapacityLimits,
    accounting: CapacityAccounting,
    cases: Vec<VectorCase>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CapacityLimits {
    raw_events: u64,
    raw_tags: u64,
    raw_event_text_bytes: u64,
    raw_tag_text_bytes: u64,
    retained_source_generations: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CapacityAccounting {
    algorithm: String,
    raw_event_columns: Vec<String>,
    raw_tag_columns: Vec<String>,
    nullable_raw_tag_columns: Vec<String>,
}

#[derive(Debug, Deserialize)]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CapacityDelta {
    raw_events: u64,
    raw_tags: u64,
    raw_event_text_bytes: u64,
    raw_tag_text_bytes: u64,
}

#[derive(Clone, Copy)]
struct ExpectedCase {
    id: &'static str,
    execution: &'static str,
    authority: &'static str,
    authority_path: &'static str,
    error_domain: Option<&'static str>,
}

const DIRECT_EXECUTOR: &str = "direct_executor";
const RESULT_VECTOR_EXECUTOR_TEST: &str = "source_maintenance_v1_result_vector";
const RESULT_VECTOR_EXECUTOR_PATH: &str =
    "crates/event_store/tests/source_maintenance_v1_result_vector.rs";

const EXPECTED_CASES: &[ExpectedCase] = &[
    ExpectedCase {
        id: "fresh_store_zero_authority",
        execution: DIRECT_EXECUTOR,
        authority: RESULT_VECTOR_EXECUTOR_TEST,
        authority_path: RESULT_VECTOR_EXECUTOR_PATH,
        error_domain: None,
    },
    ExpectedCase {
        id: "durable_unique_append_updates_all_dimensions",
        execution: DIRECT_EXECUTOR,
        authority: RESULT_VECTOR_EXECUTOR_TEST,
        authority_path: RESULT_VECTOR_EXECUTOR_PATH,
        error_domain: None,
    },
    ExpectedCase {
        id: "duplicate_at_exact_boundary_is_idempotent",
        execution: "delegated_rust_test",
        authority: "exact_capacity_boundary_allows_duplicate_observation_and_ephemeral_noop",
        authority_path: "crates/event_store/src/store.rs",
        error_domain: None,
    },
    ExpectedCase {
        id: "ephemeral_consumes_no_capacity",
        execution: DIRECT_EXECUTOR,
        authority: RESULT_VECTOR_EXECUTOR_TEST,
        authority_path: RESULT_VECTOR_EXECUTOR_PATH,
        error_domain: None,
    },
    ExpectedCase {
        id: "raw_event_count_exact",
        execution: "delegated_rust_test",
        authority: "every_prospective_capacity_dimension_accepts_exact_and_rejects_one_over",
        authority_path: "crates/event_store/src/source_maintenance_v1.rs",
        error_domain: None,
    },
    ExpectedCase {
        id: "raw_event_count_one_over",
        execution: "delegated_rust_test",
        authority: "every_prospective_capacity_dimension_accepts_exact_and_rejects_one_over",
        authority_path: "crates/event_store/src/source_maintenance_v1.rs",
        error_domain: Some("typed"),
    },
    ExpectedCase {
        id: "raw_tag_count_exact",
        execution: "delegated_rust_test",
        authority: "every_prospective_capacity_dimension_accepts_exact_and_rejects_one_over",
        authority_path: "crates/event_store/src/source_maintenance_v1.rs",
        error_domain: None,
    },
    ExpectedCase {
        id: "raw_tag_count_one_over",
        execution: "delegated_rust_test",
        authority: "every_prospective_capacity_dimension_accepts_exact_and_rejects_one_over",
        authority_path: "crates/event_store/src/source_maintenance_v1.rs",
        error_domain: Some("typed"),
    },
    ExpectedCase {
        id: "raw_event_text_bytes_exact",
        execution: "delegated_rust_test",
        authority: "every_prospective_capacity_dimension_accepts_exact_and_rejects_one_over",
        authority_path: "crates/event_store/src/source_maintenance_v1.rs",
        error_domain: None,
    },
    ExpectedCase {
        id: "raw_event_text_bytes_one_over",
        execution: "delegated_rust_test",
        authority: "every_prospective_capacity_dimension_accepts_exact_and_rejects_one_over",
        authority_path: "crates/event_store/src/source_maintenance_v1.rs",
        error_domain: Some("typed"),
    },
    ExpectedCase {
        id: "raw_tag_text_bytes_exact",
        execution: "delegated_rust_test",
        authority: "every_prospective_capacity_dimension_accepts_exact_and_rejects_one_over",
        authority_path: "crates/event_store/src/source_maintenance_v1.rs",
        error_domain: None,
    },
    ExpectedCase {
        id: "raw_tag_text_bytes_one_over",
        execution: "delegated_rust_test",
        authority: "every_prospective_capacity_dimension_accepts_exact_and_rejects_one_over",
        authority_path: "crates/event_store/src/source_maintenance_v1.rs",
        error_domain: Some("typed"),
    },
    ExpectedCase {
        id: "outer_transaction_rollback_restores_capacity",
        execution: DIRECT_EXECUTOR,
        authority: RESULT_VECTOR_EXECUTOR_TEST,
        authority_path: RESULT_VECTOR_EXECUTOR_PATH,
        error_domain: None,
    },
    ExpectedCase {
        id: "failed_nested_ingest_rolls_back_savepoint_only",
        execution: "delegated_rust_test",
        authority: "borrowed_ingest_savepoint_rolls_back_post_core_authority_forge",
        authority_path: "crates/event_store/src/store.rs",
        error_domain: Some("typed"),
    },
    ExpectedCase {
        id: "v3_to_v4_under_limit_succeeds",
        execution: "delegated_rust_test",
        authority: "v3_to_v4_under_limit_backfills_exact_capacity_and_preserves_source",
        authority_path: "crates/event_store/src/schema.rs",
        error_domain: None,
    },
    ExpectedCase {
        id: "v3_to_v4_prior_transition_drift_is_atomic",
        execution: "delegated_rust_test",
        authority: "v3_to_v4_rejects_prior_transition_drift_atomically",
        authority_path: "crates/event_store/src/schema.rs",
        error_domain: Some("typed"),
    },
    ExpectedCase {
        id: "v3_to_v4_one_over_is_atomic",
        execution: "delegated_rust_test",
        authority: "source_capacity_is_rechecked_for_every_rebuild_bound_migration",
        authority_path: "crates/event_store/src/schema.rs",
        error_domain: Some("typed"),
    },
    ExpectedCase {
        id: "v3_to_v4_persisted_ephemeral_is_atomic",
        execution: "delegated_rust_test",
        authority: "v4_rejects_persisted_legacy_ephemeral_rows_atomically",
        authority_path: "crates/event_store/src/schema.rs",
        error_domain: Some("typed"),
    },
    ExpectedCase {
        id: "reopen_rejects_incoherent_capacity_authority",
        execution: "delegated_rust_test",
        authority: "reopen_full_measure_detects_every_persisted_capacity_dimension",
        authority_path: "crates/event_store/src/source_maintenance_v1.rs",
        error_domain: Some("typed"),
    },
    ExpectedCase {
        id: "reopen_stops_at_first_raw_event_one_over",
        execution: "delegated_rust_test",
        authority: "reopen_stops_at_the_first_raw_event_one_over_before_ephemeral_probe",
        authority_path: "crates/event_store/src/source_maintenance_v1.rs",
        error_domain: Some("typed"),
    },
    ExpectedCase {
        id: "retained_generation_rebuild_exact",
        execution: "delegated_rust_test",
        authority: "ninth_current_v4_rebuild_is_typed_and_preflight_atomic",
        authority_path: "crates/event_store/src/store.rs",
        error_domain: None,
    },
    ExpectedCase {
        id: "ninth_rebuild_is_typed_and_atomic",
        execution: "delegated_rust_test",
        authority: "ninth_current_v4_rebuild_is_typed_and_preflight_atomic",
        authority_path: "crates/event_store/src/store.rs",
        error_domain: Some("typed"),
    },
    ExpectedCase {
        id: "retained_generation_sql_backstop_one_over",
        execution: "delegated_sql_test",
        authority: "generation_sql_backstop_allows_exact_append_and_is_conflict_safe_one_over",
        authority_path: "crates/event_store/src/source_maintenance_v1.rs",
        error_domain: Some("sqlite_database"),
    },
    ExpectedCase {
        id: "rebuild_marker_accepts_consistent_seals",
        execution: "delegated_rust_test",
        authority: "current_v4_rebuild_rotates_capacity_and_food_authority_end_to_end",
        authority_path: "crates/event_store/src/store.rs",
        error_domain: None,
    },
    ExpectedCase {
        id: "rebuild_marker_rejects_incoherent_seals",
        execution: "delegated_sql_test",
        authority: "marker_close_sql_backstop_rejects_each_required_seal_drift",
        authority_path: "crates/event_store/src/source_maintenance_v1.rs",
        error_domain: Some("sqlite_database"),
    },
    ExpectedCase {
        id: "v4_marker_repair_binds_exact_prior_and_floor",
        execution: "delegated_rust_test",
        authority: "v4_marker_open_allows_repairing_prior_transition_high_water_drift",
        authority_path: "crates/event_store/src/schema.rs",
        error_domain: Some("sqlite_database"),
    },
    ExpectedCase {
        id: "v4_food_reset_requires_target_rotation",
        execution: "delegated_rust_test",
        authority: "v4_food_reset_requires_marker_rotation_and_preserves_target_rows",
        authority_path: "crates/event_store/src/schema.rs",
        error_domain: None,
    },
    ExpectedCase {
        id: "v4_down_restores_predecessor_triggers",
        execution: "delegated_rust_test",
        authority: "v4_down_restores_exact_predecessor_trigger_sql_and_fingerprint",
        authority_path: "crates/event_store/src/schema.rs",
        error_domain: None,
    },
    ExpectedCase {
        id: "utf16_open_file_rejected_before_mutation",
        execution: "delegated_rust_test",
        authority: "open_file_rejects_utf16_main_database_before_schema_or_journal_mutation",
        authority_path: "crates/event_store/src/store.rs",
        error_domain: Some("typed"),
    },
    ExpectedCase {
        id: "utf16_open_pool_rejected_before_mutation",
        execution: "delegated_rust_test",
        authority: "open_pool_rejects_utf16_main_database_before_schema_or_journal_mutation",
        authority_path: "crates/event_store/src/store.rs",
        error_domain: Some("typed"),
    },
    ExpectedCase {
        id: "utf8_non_ascii_nul_reopen_accounting",
        execution: "delegated_rust_test",
        authority: "utf8_file_reopen_preserves_non_ascii_and_nul_capacity_accounting",
        authority_path: "crates/event_store/src/store.rs",
        error_domain: None,
    },
    ExpectedCase {
        id: "generation_destructive_rollback_rejected",
        execution: "delegated_rust_test",
        authority: "rollback_rejects_below_floor_ahead_unmanaged_and_generation_destructive_targets",
        authority_path: "crates/event_store/src/schema.rs",
        error_domain: Some("typed"),
    },
    ExpectedCase {
        id: "generation_destructive_two_step_rollback_rejected",
        execution: "delegated_rust_test",
        authority: "rollback_cannot_bypass_generation_history_guard_through_version_three",
        authority_path: "crates/event_store/src/schema.rs",
        error_domain: Some("typed"),
    },
    ExpectedCase {
        id: "independent_pool_last_byte_slot_race",
        execution: "delegated_rust_test",
        authority: "independent_file_pools_serialize_the_last_raw_event_byte_capacity_slot",
        authority_path: "crates/event_store/src/store.rs",
        error_domain: Some("typed"),
    },
];

#[tokio::test]
async fn source_maintenance_v1_result_vector() {
    assert_eq!(
        RESULT_VECTOR_EXECUTOR_ID,
        "radroots_event_store.source_maintenance_v1.result_vector_executor.v1"
    );
    let vector: SourceMaintenanceVector =
        serde_json::from_slice(RESULT_VECTOR_BYTES).expect("strict SourceMaintenance vector");
    validate_vector_header(&vector);
    validate_case_inventory(&vector.cases);
    let mut executed_direct_cases = BTreeSet::new();

    let store = RadrootsEventStore::open_memory()
        .await
        .expect("open current in-memory event store");
    let fresh = store
        .source_capacity_v1()
        .await
        .expect("fresh source capacity");
    assert_eq!(fresh.raw_event_count(), 0);
    assert_eq!(fresh.raw_tag_count(), 0);
    assert_eq!(fresh.raw_event_text_bytes(), 0);
    assert_eq!(fresh.raw_tag_text_bytes(), 0);
    assert_eq!(fresh.raw_high_water_seq(), 0);
    assert_eq!(fresh.retained_generation_count(), 1);
    assert_eq!(
        fresh.retained_generation_limit(),
        RADROOTS_EVENT_STORE_RETAINED_SOURCE_GENERATION_LIMIT_V1
    );
    mark_direct_case(&mut executed_direct_cases, "fresh_store_zero_authority");

    let durable_raw = signed_raw_json(
        1,
        1_750_000_000,
        vec![vec!["t".to_owned(), "soil".to_owned()]],
        "Victoria field note",
    );
    let expected_delta = capacity_delta(&durable_raw);
    let first = store
        .ingest_event(
            RadrootsEventIngest::from_raw_json(durable_raw.clone(), 1_750_000_001)
                .expect("verified durable fixture"),
        )
        .await
        .expect("unique durable ingest");
    assert!(first.persistence.is_inserted());
    let after_first = store
        .source_capacity_v1()
        .await
        .expect("capacity after unique durable ingest");
    assert_eq!(after_first.raw_event_count(), expected_delta.raw_events);
    assert_eq!(after_first.raw_tag_count(), expected_delta.raw_tags);
    assert_eq!(
        after_first.raw_event_text_bytes(),
        expected_delta.raw_event_text_bytes
    );
    assert_eq!(
        after_first.raw_tag_text_bytes(),
        expected_delta.raw_tag_text_bytes
    );
    assert_eq!(after_first.raw_high_water_seq(), 1);
    mark_direct_case(
        &mut executed_direct_cases,
        "durable_unique_append_updates_all_dimensions",
    );

    let duplicate = store
        .ingest_event(
            RadrootsEventIngest::from_raw_json(durable_raw, 1_750_000_002)
                .expect("verified duplicate fixture"),
        )
        .await
        .expect("duplicate durable ingest");
    assert!(duplicate.persistence.is_duplicate());
    assert_eq!(
        store
            .source_capacity_v1()
            .await
            .expect("capacity after duplicate"),
        after_first
    );

    let ephemeral_raw = signed_raw_json(20_001, 1_750_000_003, Vec::new(), "relay-only");
    let ephemeral = store
        .ingest_event(
            RadrootsEventIngest::from_raw_json(ephemeral_raw, 1_750_000_004)
                .expect("verified ephemeral fixture"),
        )
        .await
        .expect("ephemeral ingest outcome");
    assert!(!ephemeral.persistence.is_inserted());
    assert_eq!(
        store
            .source_capacity_v1()
            .await
            .expect("capacity after ephemeral"),
        after_first
    );
    mark_direct_case(&mut executed_direct_cases, "ephemeral_consumes_no_capacity");

    let rolled_back_raw = signed_raw_json(1, 1_750_000_005, Vec::new(), "rolled back");
    let rolled_back_id = serde_json::from_str::<serde_json::Value>(&rolled_back_raw)
        .expect("rolled-back JSON")
        .get("id")
        .and_then(serde_json::Value::as_str)
        .expect("rolled-back event id")
        .to_owned();
    let mut transaction = store
        .begin_write_transaction()
        .await
        .expect("begin composed write");
    let receipt = store
        .ingest_event_in_transaction(
            &mut transaction,
            RadrootsEventIngest::from_raw_json(rolled_back_raw, 1_750_000_006)
                .expect("verified rollback fixture"),
        )
        .await
        .expect("nested ingest before outer rollback");
    assert!(receipt.persistence.is_inserted());
    transaction.rollback().await.expect("rollback outer write");
    assert_eq!(
        store
            .source_capacity_v1()
            .await
            .expect("capacity after outer rollback"),
        after_first
    );
    assert!(
        store
            .raw_event(&rolled_back_id)
            .await
            .expect("rolled-back raw lookup")
            .is_none()
    );
    mark_direct_case(
        &mut executed_direct_cases,
        "outer_transaction_rollback_restores_capacity",
    );

    assert_direct_cases_executed(&vector.cases, &executed_direct_cases);
}

fn validate_vector_header(vector: &SourceMaintenanceVector) {
    assert_eq!(vector.schema_version, 1);
    assert_eq!(
        vector.contract_id,
        "radroots_event_store.source_maintenance_v1"
    );
    assert_eq!(vector.capacity_version, 1);
    assert_eq!(
        vector.limits.raw_events,
        RADROOTS_EVENT_STORE_RAW_EVENT_COUNT_LIMIT_V1
    );
    assert_eq!(
        vector.limits.raw_tags,
        RADROOTS_EVENT_STORE_RAW_TAG_COUNT_LIMIT_V1
    );
    assert_eq!(
        vector.limits.raw_event_text_bytes,
        RADROOTS_EVENT_STORE_RAW_EVENT_TEXT_BYTES_LIMIT_V1
    );
    assert_eq!(
        vector.limits.raw_tag_text_bytes,
        RADROOTS_EVENT_STORE_RAW_TAG_TEXT_BYTES_LIMIT_V1
    );
    assert_eq!(
        vector.limits.retained_source_generations,
        RADROOTS_EVENT_STORE_RETAINED_SOURCE_GENERATION_LIMIT_V1
    );
    assert_eq!(vector.accounting.algorithm, "sqlite_cast_blob_octet_sum_v1");
    assert_eq!(
        vector.accounting.raw_event_columns,
        [
            "event_id",
            "pubkey",
            "tags_json",
            "content",
            "sig",
            "raw_json"
        ]
    );
    assert_eq!(
        vector.accounting.raw_tag_columns,
        ["event_id", "tag_name", "tag_value", "tag_json"]
    );
    assert_eq!(vector.accounting.nullable_raw_tag_columns, ["tag_value"]);
}

fn validate_case_inventory(cases: &[VectorCase]) {
    assert_eq!(cases.len(), EXPECTED_CASES.len());
    for (case, expected) in cases.iter().zip(EXPECTED_CASES) {
        assert_eq!(case.id, expected.id);
        assert_eq!(case.execution, expected.execution, "{}", case.id);
        assert_eq!(case.authority, expected.authority, "{}", case.id);
        assert_eq!(case.authority_path, expected.authority_path, "{}", case.id);
        assert_eq!(
            case.error_domain.as_deref(),
            expected.error_domain,
            "{}",
            case.id
        );
        assert!(!case.expected_outcome.is_empty(), "{}", case.id);
        match case.boundary.as_deref() {
            Some(
                "corrupt_managed_v3" | "exact" | "managed_v4_rebuild" | "one_over" | "under_limit"
                | "v4_to_v3",
            )
            | None => {}
            other => panic!("{}: invalid boundary {other:?}", case.id),
        }
        match (case.error_domain.as_deref(), case.expected_error.as_deref()) {
            (None, None) => {}
            (Some("typed" | "sqlite_database"), Some(error)) if !error.is_empty() => {}
            other => panic!("{}: inconsistent error metadata {other:?}", case.id),
        }
        if case.execution == DIRECT_EXECUTOR {
            assert!(case.resource.is_none(), "{}", case.id);
            assert!(case.boundary.is_none(), "{}", case.id);
        }
    }
}

fn mark_direct_case(executed: &mut BTreeSet<String>, id: &str) {
    assert!(
        executed.insert(id.to_owned()),
        "direct case executed twice: {id}"
    );
}

fn assert_direct_cases_executed(cases: &[VectorCase], executed: &BTreeSet<String>) {
    let expected = cases
        .iter()
        .filter(|case| case.execution == DIRECT_EXECUTOR)
        .map(|case| case.id.clone())
        .collect::<BTreeSet<_>>();
    assert_eq!(executed, &expected);
}

fn signed_raw_json(kind: u16, created_at: u64, tags: Vec<Vec<String>>, content: &str) -> String {
    let secret_key = SecretKey::from_hex(FIXTURE_SECRET_KEY_HEX).expect("fixture secret key");
    let keys = Keys::new(secret_key);
    let tags = tags
        .into_iter()
        .map(|mut values| {
            let name = values.remove(0);
            Tag::custom(TagKind::Custom(name.into()), values)
        })
        .collect::<Vec<_>>();
    let event = EventBuilder::new(Kind::Custom(kind), content)
        .tags(tags)
        .custom_created_at(Timestamp::from_secs(created_at))
        .sign_with_keys(&keys)
        .expect("signed fixture event");
    serde_json::to_string(&event).expect("fixture event JSON")
}

fn capacity_delta(raw_json: &str) -> CapacityDelta {
    let event: serde_json::Value = serde_json::from_str(raw_json).expect("signed fixture JSON");
    let event_id = text_field(&event, "id");
    let pubkey = text_field(&event, "pubkey");
    let content = text_field(&event, "content");
    let sig = text_field(&event, "sig");
    let tags = event
        .get("tags")
        .and_then(serde_json::Value::as_array)
        .expect("fixture tags");
    let tags_json = serde_json::to_string(tags).expect("canonical tags JSON");
    let raw_event_text_bytes = [event_id, pubkey, tags_json.as_str(), content, sig, raw_json]
        .into_iter()
        .map(str::len)
        .sum::<usize>();
    let raw_tag_text_bytes = tags
        .iter()
        .map(|tag| {
            let values = tag.as_array().expect("tag array");
            let name = values
                .first()
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            let value = values
                .get(1)
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            event_id.len()
                + name.len()
                + value.len()
                + serde_json::to_string(values).expect("tag JSON").len()
        })
        .sum::<usize>();
    CapacityDelta {
        raw_events: 1,
        raw_tags: u64::try_from(tags.len()).expect("tag count fits u64"),
        raw_event_text_bytes: u64::try_from(raw_event_text_bytes)
            .expect("event byte count fits u64"),
        raw_tag_text_bytes: u64::try_from(raw_tag_text_bytes).expect("tag byte count fits u64"),
    }
}

fn text_field<'a>(event: &'a serde_json::Value, name: &str) -> &'a str {
    event
        .get(name)
        .and_then(serde_json::Value::as_str)
        .unwrap_or_else(|| panic!("fixture event missing {name}"))
}
