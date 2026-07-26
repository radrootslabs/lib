#![forbid(unsafe_code)]

use radroots_event_store::{RadrootsEventIngest, RadrootsEventStore};
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeSet;

const RESULT_VECTOR_EXECUTOR_ID: &str =
    "radroots_event_store.raw_source_rebuild_v1.result_vector_executor.v1";
const RESULT_VECTOR_BYTES: &[u8] =
    include_bytes!("../../../contracts/conformance/vectors/event_store/raw_source_rebuild.v1.json");
const FOOD_FIXTURE_BYTES: &[u8] = include_bytes!("fixtures/food_availability_projection.v1.json");

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSourceRebuildVector {
    schema_version: u32,
    contract_id: String,
    delegated_suite: DelegatedSuite,
    cases: Vec<VectorCase>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DelegatedSuite {
    id: String,
    lane: String,
    package: String,
    authorities: Vec<DelegatedAuthority>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DelegatedAuthority {
    authority: String,
    authority_path: String,
}

#[derive(Debug, Deserialize)]
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

#[derive(Clone, Copy)]
struct ExpectedCase {
    id: &'static str,
    execution: &'static str,
    authority: &'static str,
    authority_path: &'static str,
}

#[derive(Clone, Copy)]
struct ExpectedDelegatedAuthority {
    authority: &'static str,
    authority_path: &'static str,
}

const DIRECT_EXECUTOR: &str = "direct_executor";
const DELEGATED_RUST_TEST: &str = "delegated_rust_test";
const EXECUTOR_TEST: &str = "raw_source_rebuild_v1_result_vector";
const EXECUTOR_PATH: &str = "crates/event_store/tests/raw_source_rebuild_v1_result_vector.rs";
const REBUILD_TEST_PATH: &str = "crates/event_store/src/store/raw_source_rebuild_v1_tests.rs";
const ORACLE_TEST_PATH: &str =
    "crates/event_store/src/nip09/reconciliation_v1/visibility_oracle_v1.rs";
const DELEGATED_SUITE_ID: &str =
    "radroots_event_store.raw_source_rebuild_v1.delegated_rust_test_suite.v1";
const DELEGATED_SUITE_LANE: &str = "nix run .#contract";
const DELEGATED_SUITE_PACKAGE: &str = "radroots_event_store";

const EXPECTED_DELEGATED_AUTHORITIES: &[ExpectedDelegatedAuthority] = &[
    ExpectedDelegatedAuthority {
        authority: "raw_source_rebuild_incremental_reopen_and_repeat_parity_v1",
        authority_path: REBUILD_TEST_PATH,
    },
    ExpectedDelegatedAuthority {
        authority: "projection_cursor_capacity_accepts_exact_and_rejects_one_over_v1",
        authority_path: REBUILD_TEST_PATH,
    },
    ExpectedDelegatedAuthority {
        authority: "raw_source_rebuild_invalidates_generic_cursors_without_enumerating_or_mutating_them_v1",
        authority_path: REBUILD_TEST_PATH,
    },
    ExpectedDelegatedAuthority {
        authority: "raw_source_rebuild_normalizes_only_transition_sqlite_sequence_v1",
        authority_path: REBUILD_TEST_PATH,
    },
    ExpectedDelegatedAuthority {
        authority: "raw_source_rebuild_rejects_unrelated_minimum_transition_sequence_rowid_v1",
        authority_path: REBUILD_TEST_PATH,
    },
    ExpectedDelegatedAuthority {
        authority: "raw_source_rebuild_reuses_target_alias_at_minimum_sequence_rowid_v1",
        authority_path: REBUILD_TEST_PATH,
    },
    ExpectedDelegatedAuthority {
        authority: "raw_source_rebuild_repairs_active_transition_high_water_metadata_drift_v1",
        authority_path: REBUILD_TEST_PATH,
    },
    ExpectedDelegatedAuthority {
        authority: "raw_source_rebuild_repairs_empty_transition_high_water_metadata_drift_v1",
        authority_path: REBUILD_TEST_PATH,
    },
    ExpectedDelegatedAuthority {
        authority: "raw_source_rebuild_repairs_derived_drift_and_refuses_raw_drift_atomically_v1",
        authority_path: REBUILD_TEST_PATH,
    },
    ExpectedDelegatedAuthority {
        authority: "raw_source_rebuild_refuses_transition_history_gap_atomically_v1",
        authority_path: REBUILD_TEST_PATH,
    },
    ExpectedDelegatedAuthority {
        authority: "raw_source_rebuild_refuses_historical_generation_lineage_corruption_v1",
        authority_path: REBUILD_TEST_PATH,
    },
    ExpectedDelegatedAuthority {
        authority: "raw_source_rebuild_failpoints_roll_back_every_stage_v1",
        authority_path: REBUILD_TEST_PATH,
    },
    ExpectedDelegatedAuthority {
        authority: "raw_source_rebuild_rollback_failure_preserves_primary_and_rollback_errors_v1",
        authority_path: REBUILD_TEST_PATH,
    },
    ExpectedDelegatedAuthority {
        authority: "raw_source_rebuild_wal_readers_observe_only_committed_generation_v1",
        authority_path: REBUILD_TEST_PATH,
    },
    ExpectedDelegatedAuthority {
        authority: "raw_source_rebuild_rejects_caller_inbound_foreign_keys_atomically_v1",
        authority_path: REBUILD_TEST_PATH,
    },
    ExpectedDelegatedAuthority {
        authority: "raw_source_rebuild_caller_schema_inventory_limits_are_typed_and_atomic_v1",
        authority_path: REBUILD_TEST_PATH,
    },
    ExpectedDelegatedAuthority {
        authority: "raw_source_rebuild_scoped_integrity_preserves_caller_state_v1",
        authority_path: REBUILD_TEST_PATH,
    },
    ExpectedDelegatedAuthority {
        authority: "raw_source_rebuild_generation_exhaustion_precedes_entropy_and_mutation_v1",
        authority_path: REBUILD_TEST_PATH,
    },
    ExpectedDelegatedAuthority {
        authority: "raw_source_rebuild_entropy_failure_is_atomic_v1",
        authority_path: REBUILD_TEST_PATH,
    },
    ExpectedDelegatedAuthority {
        authority: "raw_source_rebuild_empty_source_without_transitions_is_deterministic_v1",
        authority_path: REBUILD_TEST_PATH,
    },
    ExpectedDelegatedAuthority {
        authority: "raw_source_rebuild_cold_file_repair_v1",
        authority_path: REBUILD_TEST_PATH,
    },
    ExpectedDelegatedAuthority {
        authority: "raw_source_repair_preflights_reject_bounded_authority_drift_v1",
        authority_path: REBUILD_TEST_PATH,
    },
    ExpectedDelegatedAuthority {
        authority: "raw_source_repair_rejects_delete_mode_exact_v4_without_mutation_v1",
        authority_path: REBUILD_TEST_PATH,
    },
    ExpectedDelegatedAuthority {
        authority: "raw_source_repair_rejects_canonical_path_lock_domain_mismatch_v1",
        authority_path: REBUILD_TEST_PATH,
    },
    ExpectedDelegatedAuthority {
        authority: "raw_source_repair_post_preflight_failures_preserve_wal_and_state_v1",
        authority_path: REBUILD_TEST_PATH,
    },
    ExpectedDelegatedAuthority {
        authority: "raw_snapshot_visibility_oracle_covers_regular_replaceable_addressable_and_deletion_v1",
        authority_path: ORACLE_TEST_PATH,
    },
    ExpectedDelegatedAuthority {
        authority: "raw_snapshot_visibility_oracle_matches_wide_event_and_address_requests_v1",
        authority_path: ORACLE_TEST_PATH,
    },
    ExpectedDelegatedAuthority {
        authority: "raw_snapshot_visibility_oracle_matches_all_protocol_decision_branches_v1",
        authority_path: ORACLE_TEST_PATH,
    },
    ExpectedDelegatedAuthority {
        authority: "raw_snapshot_visibility_oracle_is_order_and_repeat_invariant_v1",
        authority_path: ORACLE_TEST_PATH,
    },
];

const EXPECTED_CASES: &[ExpectedCase] = &[
    ExpectedCase {
        id: "empty_source_repeat_digest_parity",
        execution: DIRECT_EXECUTOR,
        authority: EXECUTOR_TEST,
        authority_path: EXECUTOR_PATH,
    },
    ExpectedCase {
        id: "signed_food_fixture_typed_digest_parity",
        execution: DIRECT_EXECUTOR,
        authority: EXECUTOR_TEST,
        authority_path: EXECUTOR_PATH,
    },
    ExpectedCase {
        id: "incremental_reopen_repeat_product_parity",
        execution: DELEGATED_RUST_TEST,
        authority: "raw_source_rebuild_incremental_reopen_and_repeat_parity_v1",
        authority_path: REBUILD_TEST_PATH,
    },
    ExpectedCase {
        id: "projection_cursor_capacity_exact_and_one_over",
        execution: DELEGATED_RUST_TEST,
        authority: "projection_cursor_capacity_accepts_exact_and_rejects_one_over_v1",
        authority_path: REBUILD_TEST_PATH,
    },
    ExpectedCase {
        id: "generic_cursor_lazy_generation_invalidation",
        execution: DELEGATED_RUST_TEST,
        authority: "raw_source_rebuild_invalidates_generic_cursors_without_enumerating_or_mutating_them_v1",
        authority_path: REBUILD_TEST_PATH,
    },
    ExpectedCase {
        id: "target_first_transition_sequence_normalization",
        execution: DELEGATED_RUST_TEST,
        authority: "raw_source_rebuild_normalizes_only_transition_sqlite_sequence_v1",
        authority_path: REBUILD_TEST_PATH,
    },
    ExpectedCase {
        id: "unrelated_minimum_transition_sequence_rowid_refusal",
        execution: DELEGATED_RUST_TEST,
        authority: "raw_source_rebuild_rejects_unrelated_minimum_transition_sequence_rowid_v1",
        authority_path: REBUILD_TEST_PATH,
    },
    ExpectedCase {
        id: "minimum_target_alias_sequence_reuse",
        execution: DELEGATED_RUST_TEST,
        authority: "raw_source_rebuild_reuses_target_alias_at_minimum_sequence_rowid_v1",
        authority_path: REBUILD_TEST_PATH,
    },
    ExpectedCase {
        id: "active_transition_high_water_metadata_repair",
        execution: DELEGATED_RUST_TEST,
        authority: "raw_source_rebuild_repairs_active_transition_high_water_metadata_drift_v1",
        authority_path: REBUILD_TEST_PATH,
    },
    ExpectedCase {
        id: "empty_transition_high_water_metadata_repair",
        execution: DELEGATED_RUST_TEST,
        authority: "raw_source_rebuild_repairs_empty_transition_high_water_metadata_drift_v1",
        authority_path: REBUILD_TEST_PATH,
    },
    ExpectedCase {
        id: "derived_repair_and_raw_refusal_atomicity",
        execution: DELEGATED_RUST_TEST,
        authority: "raw_source_rebuild_repairs_derived_drift_and_refuses_raw_drift_atomically_v1",
        authority_path: REBUILD_TEST_PATH,
    },
    ExpectedCase {
        id: "transition_history_gap_refusal_atomicity",
        execution: DELEGATED_RUST_TEST,
        authority: "raw_source_rebuild_refuses_transition_history_gap_atomically_v1",
        authority_path: REBUILD_TEST_PATH,
    },
    ExpectedCase {
        id: "historical_generation_lineage_corruption_refusal",
        execution: DELEGATED_RUST_TEST,
        authority: "raw_source_rebuild_refuses_historical_generation_lineage_corruption_v1",
        authority_path: REBUILD_TEST_PATH,
    },
    ExpectedCase {
        id: "rollback_after_marker_open",
        execution: DELEGATED_RUST_TEST,
        authority: "raw_source_rebuild_failpoints_roll_back_every_stage_v1",
        authority_path: REBUILD_TEST_PATH,
    },
    ExpectedCase {
        id: "rollback_after_generation_rotation",
        execution: DELEGATED_RUST_TEST,
        authority: "raw_source_rebuild_failpoints_roll_back_every_stage_v1",
        authority_path: REBUILD_TEST_PATH,
    },
    ExpectedCase {
        id: "rollback_after_core_replay",
        execution: DELEGATED_RUST_TEST,
        authority: "raw_source_rebuild_failpoints_roll_back_every_stage_v1",
        authority_path: REBUILD_TEST_PATH,
    },
    ExpectedCase {
        id: "rollback_after_visibility_audit",
        execution: DELEGATED_RUST_TEST,
        authority: "raw_source_rebuild_failpoints_roll_back_every_stage_v1",
        authority_path: REBUILD_TEST_PATH,
    },
    ExpectedCase {
        id: "rollback_after_food_reset_replay",
        execution: DELEGATED_RUST_TEST,
        authority: "raw_source_rebuild_failpoints_roll_back_every_stage_v1",
        authority_path: REBUILD_TEST_PATH,
    },
    ExpectedCase {
        id: "rollback_after_food_audit",
        execution: DELEGATED_RUST_TEST,
        authority: "raw_source_rebuild_failpoints_roll_back_every_stage_v1",
        authority_path: REBUILD_TEST_PATH,
    },
    ExpectedCase {
        id: "rollback_after_marker_close",
        execution: DELEGATED_RUST_TEST,
        authority: "raw_source_rebuild_failpoints_roll_back_every_stage_v1",
        authority_path: REBUILD_TEST_PATH,
    },
    ExpectedCase {
        id: "rollback_failure_preserves_both_errors",
        execution: DELEGATED_RUST_TEST,
        authority: "raw_source_rebuild_rollback_failure_preserves_primary_and_rollback_errors_v1",
        authority_path: REBUILD_TEST_PATH,
    },
    ExpectedCase {
        id: "wal_reader_commit_visibility",
        execution: DELEGATED_RUST_TEST,
        authority: "raw_source_rebuild_wal_readers_observe_only_committed_generation_v1",
        authority_path: REBUILD_TEST_PATH,
    },
    ExpectedCase {
        id: "caller_inbound_foreign_key_refusal_atomicity",
        execution: DELEGATED_RUST_TEST,
        authority: "raw_source_rebuild_rejects_caller_inbound_foreign_keys_atomically_v1",
        authority_path: REBUILD_TEST_PATH,
    },
    ExpectedCase {
        id: "caller_schema_inventory_capacity_exact_and_one_over",
        execution: DELEGATED_RUST_TEST,
        authority: "raw_source_rebuild_caller_schema_inventory_limits_are_typed_and_atomic_v1",
        authority_path: REBUILD_TEST_PATH,
    },
    ExpectedCase {
        id: "scoped_integrity_preserves_caller_state",
        execution: DELEGATED_RUST_TEST,
        authority: "raw_source_rebuild_scoped_integrity_preserves_caller_state_v1",
        authority_path: REBUILD_TEST_PATH,
    },
    ExpectedCase {
        id: "generation_exhaustion_preflight",
        execution: DELEGATED_RUST_TEST,
        authority: "raw_source_rebuild_generation_exhaustion_precedes_entropy_and_mutation_v1",
        authority_path: REBUILD_TEST_PATH,
    },
    ExpectedCase {
        id: "generation_entropy_failure_atomicity",
        execution: DELEGATED_RUST_TEST,
        authority: "raw_source_rebuild_entropy_failure_is_atomic_v1",
        authority_path: REBUILD_TEST_PATH,
    },
    ExpectedCase {
        id: "empty_source_without_transitions",
        execution: DELEGATED_RUST_TEST,
        authority: "raw_source_rebuild_empty_source_without_transitions_is_deterministic_v1",
        authority_path: REBUILD_TEST_PATH,
    },
    ExpectedCase {
        id: "cold_file_repair",
        execution: DELEGATED_RUST_TEST,
        authority: "raw_source_rebuild_cold_file_repair_v1",
        authority_path: REBUILD_TEST_PATH,
    },
    ExpectedCase {
        id: "cold_bounded_preflight_authority_drift_refusal",
        execution: DELEGATED_RUST_TEST,
        authority: "raw_source_repair_preflights_reject_bounded_authority_drift_v1",
        authority_path: REBUILD_TEST_PATH,
    },
    ExpectedCase {
        id: "cold_non_wal_file_refusal_atomicity",
        execution: DELEGATED_RUST_TEST,
        authority: "raw_source_repair_rejects_delete_mode_exact_v4_without_mutation_v1",
        authority_path: REBUILD_TEST_PATH,
    },
    ExpectedCase {
        id: "cold_canonical_path_lock_domain_refusal_atomicity",
        execution: DELEGATED_RUST_TEST,
        authority: "raw_source_repair_rejects_canonical_path_lock_domain_mismatch_v1",
        authority_path: REBUILD_TEST_PATH,
    },
    ExpectedCase {
        id: "cold_post_preflight_failure_wal_state_atomicity",
        execution: DELEGATED_RUST_TEST,
        authority: "raw_source_repair_post_preflight_failures_preserve_wal_and_state_v1",
        authority_path: REBUILD_TEST_PATH,
    },
    ExpectedCase {
        id: "pure_raw_snapshot_visibility_oracle",
        execution: DELEGATED_RUST_TEST,
        authority: "raw_snapshot_visibility_oracle_covers_regular_replaceable_addressable_and_deletion_v1",
        authority_path: ORACLE_TEST_PATH,
    },
    ExpectedCase {
        id: "direct_indexed_visibility_oracle_wide_targets",
        execution: DELEGATED_RUST_TEST,
        authority: "raw_snapshot_visibility_oracle_matches_wide_event_and_address_requests_v1",
        authority_path: ORACLE_TEST_PATH,
    },
    ExpectedCase {
        id: "direct_indexed_visibility_oracle_protocol_matrix",
        execution: DELEGATED_RUST_TEST,
        authority: "raw_snapshot_visibility_oracle_matches_all_protocol_decision_branches_v1",
        authority_path: ORACLE_TEST_PATH,
    },
    ExpectedCase {
        id: "direct_indexed_visibility_oracle_order_repeat_invariance",
        execution: DELEGATED_RUST_TEST,
        authority: "raw_snapshot_visibility_oracle_is_order_and_repeat_invariant_v1",
        authority_path: ORACLE_TEST_PATH,
    },
];

fn decode_digest(value: &str) -> [u8; 32] {
    let mut bytes = [0_u8; 32];
    hex::decode_to_slice(value, &mut bytes).expect("decode governed lowercase SHA-256 digest");
    bytes
}

fn signed_food_fixture_ingest() -> RadrootsEventIngest {
    let fixture: Value = serde_json::from_slice(FOOD_FIXTURE_BYTES).expect("parse Food fixture");
    let observed = fixture["cases"]
        .as_array()
        .expect("Food fixture cases")
        .iter()
        .find(|case| case["id"].as_str() == Some("visible_food_availability_projects_and_searches"))
        .and_then(|case| case["events"].as_array())
        .and_then(|events| events.first())
        .expect("signed Food fixture event");
    let raw_json = serde_json::to_string(&observed["event"]).expect("serialize Food fixture event");
    let observed_at_ms = observed["observed_at_ms"]
        .as_i64()
        .expect("Food fixture observation time");
    RadrootsEventIngest::from_raw_json(raw_json, observed_at_ms)
        .expect("verify signed Food fixture event")
}

#[tokio::test]
async fn raw_source_rebuild_v1_result_vector() {
    assert_eq!(
        RESULT_VECTOR_EXECUTOR_ID,
        "radroots_event_store.raw_source_rebuild_v1.result_vector_executor.v1"
    );
    let vector: RawSourceRebuildVector =
        serde_json::from_slice(RESULT_VECTOR_BYTES).expect("parse raw-source rebuild vector");
    assert_eq!(vector.schema_version, 1);
    assert_eq!(
        vector.contract_id,
        "radroots_event_store.raw_source_rebuild_v1"
    );
    assert_eq!(vector.delegated_suite.id, DELEGATED_SUITE_ID);
    assert_eq!(vector.delegated_suite.lane, DELEGATED_SUITE_LANE);
    assert_eq!(vector.delegated_suite.package, DELEGATED_SUITE_PACKAGE);
    assert_eq!(vector.cases.len(), EXPECTED_CASES.len());

    let mut case_ids = BTreeSet::new();
    for (case, expected) in vector.cases.iter().zip(EXPECTED_CASES) {
        assert!(
            case_ids.insert(case.id.as_str()),
            "duplicate case {}",
            case.id
        );
        assert_eq!(case.id, expected.id);
        assert_eq!(case.execution, expected.execution);
        assert_eq!(case.authority, expected.authority);
        assert_eq!(case.authority_path, expected.authority_path);
        assert!(!case.expected_outcome.is_empty());
        for digest in [
            case.expected_immutable_raw_digest.as_deref(),
            case.expected_active_product_state_digest.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            assert_eq!(digest.len(), 64);
            assert!(
                digest
                    .as_bytes()
                    .iter()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
            );
        }
    }

    assert_eq!(
        vector.delegated_suite.authorities.len(),
        EXPECTED_DELEGATED_AUTHORITIES.len()
    );
    let mut delegated_suite_authorities = BTreeSet::new();
    for (actual, expected) in vector
        .delegated_suite
        .authorities
        .iter()
        .zip(EXPECTED_DELEGATED_AUTHORITIES)
    {
        assert_eq!(actual.authority, expected.authority);
        assert_eq!(actual.authority_path, expected.authority_path);
        assert!(
            delegated_suite_authorities
                .insert((actual.authority_path.as_str(), actual.authority.as_str(),)),
            "duplicate delegated suite authority {}::{}",
            actual.authority_path,
            actual.authority
        );
    }
    let delegated_case_authorities = vector
        .cases
        .iter()
        .filter(|case| case.execution == DELEGATED_RUST_TEST)
        .map(|case| (case.authority_path.as_str(), case.authority.as_str()))
        .collect::<BTreeSet<_>>();
    assert_eq!(delegated_suite_authorities, delegated_case_authorities);

    let direct_case = &vector.cases[0];
    assert_eq!(direct_case.id, "empty_source_repeat_digest_parity");
    let expected_immutable_raw_digest = decode_digest(
        direct_case
            .expected_immutable_raw_digest
            .as_deref()
            .expect("direct case immutable-raw digest"),
    );
    let expected_active_product_state_digest = decode_digest(
        direct_case
            .expected_active_product_state_digest
            .as_deref()
            .expect("direct case active-product-state digest"),
    );

    let store = RadrootsEventStore::open_memory()
        .await
        .expect("open managed-v4 in-memory store");
    let initial_generation = store
        .source_generation()
        .await
        .expect("initial source generation");
    let initial_capacity = store
        .source_capacity_v1()
        .await
        .expect("initial source capacity");
    assert_eq!(initial_capacity.raw_event_count(), 0);
    assert_eq!(initial_capacity.raw_tag_count(), 0);
    assert_eq!(initial_capacity.raw_event_text_bytes(), 0);
    assert_eq!(initial_capacity.raw_tag_text_bytes(), 0);
    assert_eq!(initial_capacity.raw_high_water_seq(), 0);

    let first = store
        .rebuild_from_raw_v1()
        .await
        .expect("first empty-source rebuild");
    assert_eq!(first.prior_source_generation(), initial_generation);
    assert_ne!(first.new_source_generation(), initial_generation);
    assert_eq!(first.source_capacity().raw_event_count(), 0);
    assert_eq!(first.source_capacity().raw_tag_count(), 0);
    assert_eq!(first.source_capacity().raw_event_text_bytes(), 0);
    assert_eq!(first.source_capacity().raw_tag_text_bytes(), 0);
    assert_eq!(first.raw_high_water_seq(), 0);

    let second = store
        .rebuild_from_raw_v1()
        .await
        .expect("second empty-source rebuild");
    assert_eq!(
        second.prior_source_generation(),
        first.new_source_generation()
    );
    assert_ne!(
        second.new_source_generation(),
        first.new_source_generation()
    );
    assert_eq!(second.source_capacity().raw_event_count(), 0);
    assert_eq!(second.source_capacity().raw_tag_count(), 0);
    assert_eq!(second.source_capacity().raw_event_text_bytes(), 0);
    assert_eq!(second.source_capacity().raw_tag_text_bytes(), 0);
    assert_eq!(second.raw_high_water_seq(), 0);
    assert_eq!(second.immutable_raw_digest(), first.immutable_raw_digest());
    assert_eq!(
        second.active_product_state_digest(),
        first.active_product_state_digest()
    );
    assert_eq!(
        first.immutable_raw_digest().as_bytes(),
        &expected_immutable_raw_digest,
        "actual empty immutable-raw digest: {}",
        hex::encode(first.immutable_raw_digest().as_bytes())
    );
    assert_eq!(
        first.active_product_state_digest().as_bytes(),
        &expected_active_product_state_digest,
        "actual empty active-product digest: {}",
        hex::encode(first.active_product_state_digest().as_bytes())
    );

    let food_case = &vector.cases[1];
    assert_eq!(food_case.id, "signed_food_fixture_typed_digest_parity");
    let expected_food_raw_digest = decode_digest(
        food_case
            .expected_immutable_raw_digest
            .as_deref()
            .expect("Food case immutable-raw digest"),
    );
    let expected_food_product_digest = decode_digest(
        food_case
            .expected_active_product_state_digest
            .as_deref()
            .expect("Food case active-product-state digest"),
    );
    let food_store = RadrootsEventStore::open_memory()
        .await
        .expect("open Food digest store");
    food_store
        .ingest_event(signed_food_fixture_ingest())
        .await
        .expect("ingest signed Food fixture");
    let food_first = food_store
        .rebuild_from_raw_v1()
        .await
        .expect("first Food fixture rebuild");
    assert_eq!(food_first.source_capacity().raw_event_count(), 1);
    assert!(food_first.source_capacity().raw_tag_count() > 0);
    assert_eq!(food_first.raw_high_water_seq(), 1);
    assert_eq!(
        food_first.immutable_raw_digest().as_bytes(),
        &expected_food_raw_digest,
        "actual Food immutable-raw digest: {}",
        hex::encode(food_first.immutable_raw_digest().as_bytes())
    );
    assert_eq!(
        food_first.active_product_state_digest().as_bytes(),
        &expected_food_product_digest,
        "actual Food active-product digest: {}",
        hex::encode(food_first.active_product_state_digest().as_bytes())
    );
    let food_second = food_store
        .rebuild_from_raw_v1()
        .await
        .expect("second Food fixture rebuild");
    assert_ne!(
        food_second.new_source_generation(),
        food_first.new_source_generation()
    );
    assert_eq!(
        food_second.immutable_raw_digest(),
        food_first.immutable_raw_digest()
    );
    assert_eq!(
        food_second.active_product_state_digest(),
        food_first.active_product_state_digest()
    );
}
