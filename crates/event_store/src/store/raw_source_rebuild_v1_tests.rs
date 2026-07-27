#![cfg(test)]

use super::RadrootsEventStore;
use crate::model::{RadrootsEventIngest, RadrootsProjectionCursor};
use crate::nip09::reconciliation_v1::{
    RawSourceRebuildFailpointV1, SourceGenerationProvider,
    preserve_raw_source_rebuild_primary_failure_for_test,
    rebuild_from_raw_v1_in_transaction_for_test, rebuild_from_raw_v1_on_pool_for_test,
    rebuild_from_raw_v1_on_pool_with_caller_schema_limits_for_test,
};
use crate::schema::rollback_event_store_schema_offline_destructive_for_migration_test;
use crate::{
    RADROOTS_EVENT_STORE_PROJECTION_CURSOR_COUNT_LIMIT_V1,
    RADROOTS_EVENT_STORE_RETAINED_SOURCE_GENERATION_LIMIT_V1, RadrootsEventStoreError,
    RadrootsEventStoreRawSourceRebuildDriftV1,
};
use serde_json::Value;
use sqlx::Connection;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{SqliteConnection, SqlitePool};
use std::path::Path;

const FOOD_FIXTURE: &[u8] =
    include_bytes!("../../tests/fixtures/food_availability_projection.v1.json");
const NIP09_FIXTURE: &[u8] = include_bytes!("../../tests/fixtures/nip09_reconciliation.v1.json");
const TRANSITION_SEQUENCE_NAME: &str = "radroots_event_store_addressable_head_transition";

struct FixedGeneration(u8);

impl SourceGenerationProvider for FixedGeneration {
    fn fill_generation(&self, generation: &mut [u8; 32]) -> Result<(), RadrootsEventStoreError> {
        generation.fill(self.0);
        Ok(())
    }
}

struct PanickingGeneration;

impl SourceGenerationProvider for PanickingGeneration {
    fn fill_generation(&self, _generation: &mut [u8; 32]) -> Result<(), RadrootsEventStoreError> {
        panic!("generation entropy was requested after the retained-history preflight")
    }
}

struct FailingGeneration;

impl SourceGenerationProvider for FailingGeneration {
    fn fill_generation(&self, _generation: &mut [u8; 32]) -> Result<(), RadrootsEventStoreError> {
        Err(RadrootsEventStoreError::SourceGenerationEntropyUnavailable)
    }
}

#[derive(Debug, PartialEq, Eq)]
struct RebuildAuthoritySnapshot {
    source_state: Vec<String>,
    source_capacity: Vec<String>,
    migration_history: Vec<String>,
    commit_barrier: Vec<String>,
    write_lock: Vec<String>,
    feed_integrity: Vec<String>,
    generations: Vec<String>,
    markers: Vec<String>,
    envelopes: Vec<String>,
    tags: Vec<String>,
    raw_heads: Vec<String>,
    coordinates: Vec<String>,
    nip09_requests: Vec<String>,
    nip09_event_targets: Vec<String>,
    nip09_address_targets: Vec<String>,
    addressable_heads: Vec<String>,
    transitions: Vec<String>,
    food_cursor: Vec<String>,
    food_projection: Vec<String>,
    food_images: Vec<String>,
    food_search: Vec<String>,
    sqlite_sequences: Vec<String>,
}

fn food_fixture_ingest() -> RadrootsEventIngest {
    fixture_case_ingests(
        FOOD_FIXTURE,
        "visible_food_availability_projects_and_searches",
        "events",
    )
    .into_iter()
    .next()
    .expect("Food fixture event")
}

fn fixture_case_ingests(
    bytes: &[u8],
    case_id: &str,
    events_field: &str,
) -> Vec<RadrootsEventIngest> {
    let fixture: Value = serde_json::from_slice(bytes).expect("parse event fixture");
    let case = fixture["cases"]
        .as_array()
        .expect("fixture cases")
        .iter()
        .find(|case| case["id"].as_str() == Some(case_id))
        .unwrap_or_else(|| panic!("missing fixture case {case_id}"));
    case[events_field]
        .as_array()
        .expect("fixture events")
        .iter()
        .map(|observed| {
            let raw_json =
                serde_json::to_string(&observed["event"]).expect("serialize fixture event");
            let observed_at_ms = observed["observed_at_ms"]
                .as_i64()
                .expect("fixture observed_at_ms");
            RadrootsEventIngest::from_raw_json(raw_json, observed_at_ms)
                .expect("verify fixture event")
        })
        .collect()
}

async fn seed_food_fixture(store: &RadrootsEventStore) {
    let receipt = store
        .ingest_event(food_fixture_ingest())
        .await
        .expect("ingest Food fixture");
    assert!(receipt.persistence.is_inserted());
}

async fn seed_fixture_case(
    store: &RadrootsEventStore,
    bytes: &[u8],
    case_id: &str,
    events_field: &str,
) {
    for ingest in fixture_case_ingests(bytes, case_id, events_field) {
        store
            .ingest_event(ingest)
            .await
            .unwrap_or_else(|error| panic!("ingest fixture case {case_id}: {error}"));
    }
}

async fn query_string_rows(pool: &SqlitePool, sql: &'static str) -> Vec<String> {
    sqlx::query_scalar(sql)
        .fetch_all(pool)
        .await
        .expect("snapshot query")
}

async fn rebuild_authority_snapshot(store: &RadrootsEventStore) -> RebuildAuthoritySnapshot {
    RebuildAuthoritySnapshot {
        source_state: query_string_rows(
            store.pool(),
            "SELECT printf('%s|%d|%d|%d|%d', hex(active_generation), raw_event_count, raw_tag_count, raw_high_water_seq, last_transition_seq) FROM radroots_event_store_source_state ORDER BY singleton",
        )
        .await,
        source_capacity: query_string_rows(
            store.pool(),
            "SELECT printf('%s|%d|%d|%d|%d|%d|%d|%d', hex(source_generation), raw_event_count, raw_tag_count, raw_event_bytes, raw_tag_bytes, raw_high_water_seq, retained_generation_count, retained_generation_limit) FROM radroots_event_store_source_capacity_v1 ORDER BY singleton",
        )
        .await,
        migration_history: query_string_rows(
            store.pool(),
            "SELECT printf('%d|%s|%s|%s|%s', version, name, up_sha256, down_sha256, schema_sha256) FROM radroots_event_store_schema_migrations ORDER BY version",
        )
        .await,
        commit_barrier: query_string_rows(
            store.pool(),
            "SELECT printf('%d', barrier_key) FROM radroots_event_store_source_rebuild_commit_barrier ORDER BY barrier_key",
        )
        .await,
        write_lock: query_string_rows(
            store.pool(),
            "SELECT printf('%d|%d', singleton, lock_version) FROM radroots_event_store_write_lock ORDER BY singleton",
        )
        .await,
        feed_integrity: query_string_rows(
            store.pool(),
            "SELECT printf('%s|%d|%d|%d', hex(source_generation), transition_floor_seq, last_transition_seq, transition_count) FROM radroots_event_store_addressable_feed_integrity_v1 ORDER BY hex(source_generation)",
        )
        .await,
        generations: query_string_rows(
            store.pool(),
            "SELECT printf('%s|%d|%d|%d|%d|%s|%s|%d|%d|%d|%d', hex(source_generation), generation_ordinal, reconciliation_version, addressable_feed_version, event_contract_registry_version, hook_id, hook_manifest_sha256, transition_floor_seq, baseline_raw_event_count, baseline_raw_tag_count, baseline_raw_high_water_seq) FROM radroots_event_store_source_generation ORDER BY generation_ordinal",
        )
        .await,
        markers: query_string_rows(
            store.pool(),
            "SELECT printf('%d|%s|%d', singleton, hex(target_generation), target_generation_ordinal) FROM radroots_event_store_source_rebuild_marker ORDER BY singleton",
        )
        .await,
        envelopes: query_string_rows(
            store.pool(),
            "SELECT printf('%d|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%d|%d|%d', seq, quote(event_id), quote(pubkey), quote(tags_json), quote(content), quote(sig), quote(raw_json), quote(verification_status), quote(contract_status), quote(contract_id), quote(event_class), quote(created_at), quote(kind), projection_eligible, inserted_at_ms, updated_at_ms) FROM event_envelopes ORDER BY seq",
        )
        .await,
        tags: query_string_rows(
            store.pool(),
            "SELECT printf('%s|%d|%s|%s|%s|%s|%s|%d', quote(event_id), tag_index, quote(tag_name), quote(tag_value), quote(tag_json), quote(contract_semantic), quote(contract_value_type), relay_indexed) FROM event_envelope_tags ORDER BY event_id, tag_index",
        )
        .await,
        raw_heads: query_string_rows(
            store.pool(),
            "SELECT printf('%s|%d|%s|%s|%s|%d|%d', coordinate_type, kind, pubkey, quote(d_tag), event_id, created_at, updated_at_ms) FROM event_envelope_head ORDER BY coordinate_type, kind, pubkey, d_tag",
        )
        .await,
        coordinates: query_string_rows(
            store.pool(),
            "SELECT printf('%s|%s|%d|%s|%s|%s', hex(source_generation), event_id, event_seq, coordinate_type, admission_status, quote(nip09_d_tag)) FROM radroots_event_store_event_coordinate ORDER BY hex(source_generation), event_id",
        )
        .await,
        nip09_requests: query_string_rows(
            store.pool(),
            "SELECT printf('%s|%s|%d|%d', request_event_id, request_pubkey, request_created_at, request_event_seq) FROM radroots_event_store_nip09_request ORDER BY hex(source_generation), request_event_id",
        )
        .await,
        nip09_event_targets: query_string_rows(
            store.pool(),
            "SELECT printf('%s|%s|%d|%s', request_event_id, target_event_id, source_tag_index, source_tag_value) FROM radroots_event_store_nip09_event_target ORDER BY hex(source_generation), request_event_id, target_event_id, source_tag_index",
        )
        .await,
        nip09_address_targets: query_string_rows(
            store.pool(),
            "SELECT printf('%s|%d|%s|%s|%d|%d', request_event_id, target_kind, target_pubkey, target_d_tag, inclusive_cutoff, source_tag_index) FROM radroots_event_store_nip09_address_target ORDER BY hex(source_generation), request_event_id, target_kind, target_pubkey, target_d_tag, source_tag_index",
        )
        .await,
        addressable_heads: query_string_rows(
            store.pool(),
            "SELECT printf('%s|%d|%s|%s|%s|%s|%s', hex(source_generation), kind, pubkey, d_tag, raw_head_event_id, visibility, quote(nip09_reason)) FROM radroots_event_store_addressable_head_state ORDER BY hex(source_generation), kind, pubkey, d_tag",
        )
        .await,
        transitions: query_string_rows(
            store.pool(),
            "SELECT printf('%d|%s|%s|%d|%s|%s|%s|%s', transition_seq, hex(source_generation), origin, kind, pubkey, d_tag, raw_head_event_id, visibility) FROM radroots_event_store_addressable_head_transition ORDER BY transition_seq",
        )
        .await,
        food_cursor: query_string_rows(
            store.pool(),
            "SELECT printf('%s|%d|%d|%s|%s|%d', hex(source_generation), feed_version, projection_version, hex(scope_fingerprint), hook_manifest_sha256, projected_row_count) FROM radroots_event_store_food_availability_cursor ORDER BY singleton",
        )
        .await,
        food_projection: query_string_rows(
            store.pool(),
            "SELECT printf('%s|%d|%s|%s|%s|%s|%s|%s', hex(source_generation), kind, pubkey, d_tag, event_id, title, location, status) FROM radroots_event_store_food_availability_projection ORDER BY pubkey, d_tag",
        )
        .await,
        food_images: query_string_rows(
            store.pool(),
            "SELECT printf('%s|%s|%s|%d|%s|%d', hex(source_generation), pubkey, d_tag, image_index, quote(url), qualifies) FROM radroots_event_store_food_availability_image ORDER BY pubkey, d_tag, image_index",
        )
        .await,
        food_search: query_string_rows(
            store.pool(),
            "SELECT printf('%s|%s|%s|%s|%s|%s|%s', event_id, pubkey, d_tag, title, summary, content, location) FROM radroots_event_store_food_availability_search_fts ORDER BY event_id",
        )
        .await,
        sqlite_sequences: query_string_rows(
            store.pool(),
            "SELECT printf('%d|%s|%s', rowid, quote(name), quote(seq)) FROM main.sqlite_sequence ORDER BY rowid",
        )
        .await,
    }
}

async fn cold_file_authority_snapshot(path: &Path) -> (RebuildAuthoritySnapshot, String) {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(false),
        )
        .await
        .expect("open cold snapshot pool");
    let store = RadrootsEventStore { pool };
    let snapshot = rebuild_authority_snapshot(&store).await;
    let journal_mode = sqlx::query_scalar("PRAGMA main.journal_mode")
        .fetch_one(store.pool())
        .await
        .expect("cold snapshot journal mode");
    store.pool().close().await;
    (snapshot, journal_mode)
}

async fn logical_product_snapshot(store: &RadrootsEventStore) -> Vec<Vec<String>> {
    vec![
        query_string_rows(
            store.pool(),
            "SELECT printf('%s|%s|%s|%s|%d', event_id, contract_status, quote(contract_id), quote(event_class), projection_eligible) FROM event_envelopes ORDER BY event_id",
        )
        .await,
        query_string_rows(
            store.pool(),
            "SELECT printf('%s|%d|%s|%s|%d', event_id, tag_index, quote(contract_semantic), quote(contract_value_type), relay_indexed) FROM event_envelope_tags ORDER BY event_id, tag_index",
        )
        .await,
        query_string_rows(
            store.pool(),
            "SELECT printf('%s|%d|%s|%s|%s', coordinate_type, kind, pubkey, quote(d_tag), event_id) FROM event_envelope_head ORDER BY coordinate_type, kind, pubkey, d_tag",
        )
        .await,
        query_string_rows(
            store.pool(),
            "SELECT printf('%s|%d|%s|%d|%s|%s|%s|%s|%d|%s', event_id, event_seq, coordinate_type, kind, pubkey, admission_status, quote(admission_code), quote(contract_id), nip09_matchable, quote(nip09_d_tag)) FROM radroots_event_store_event_coordinate WHERE source_generation = (SELECT active_generation FROM radroots_event_store_source_state WHERE singleton = 1) ORDER BY event_id",
        )
        .await,
        query_string_rows(
            store.pool(),
            "SELECT printf('%d|%s|%s|%s|%d|%s|%s|%s|%s', kind, pubkey, d_tag, raw_head_event_id, raw_head_created_at, admission_status, quote(admission_code), quote(contract_id), visibility) FROM radroots_event_store_addressable_head_state WHERE source_generation = (SELECT active_generation FROM radroots_event_store_source_state WHERE singleton = 1) ORDER BY kind, pubkey, d_tag",
        )
        .await,
        query_string_rows(
            store.pool(),
            "SELECT printf('%s|%s|%s|%s|%s', event_id, admission_status, quote(contract_id), current_visibility, quote(suppression_reason)) FROM radroots_event_store_current_visibility_v1 ORDER BY event_id",
        )
        .await,
        query_string_rows(
            store.pool(),
            "SELECT printf('%s|%s|%d', request_event_id, request_pubkey, request_created_at) FROM radroots_event_store_nip09_request WHERE source_generation = (SELECT active_generation FROM radroots_event_store_source_state WHERE singleton = 1) ORDER BY request_event_id",
        )
        .await,
        query_string_rows(
            store.pool(),
            "SELECT printf('%s|%s|%d|%s', request_event_id, target_event_id, source_tag_index, source_tag_value) FROM radroots_event_store_nip09_event_target WHERE source_generation = (SELECT active_generation FROM radroots_event_store_source_state WHERE singleton = 1) ORDER BY request_event_id, target_event_id, source_tag_index",
        )
        .await,
        query_string_rows(
            store.pool(),
            "SELECT printf('%s|%d|%s|%s|%d|%d', request_event_id, target_kind, target_pubkey, target_d_tag, inclusive_cutoff, source_tag_index) FROM radroots_event_store_nip09_address_target WHERE source_generation = (SELECT active_generation FROM radroots_event_store_source_state WHERE singleton = 1) ORDER BY request_event_id, target_kind, target_pubkey, target_d_tag, source_tag_index",
        )
        .await,
        query_string_rows(
            store.pool(),
            "SELECT printf('%d|%s|%s|%s|%s|%s|%s', kind, pubkey, d_tag, event_id, title, location, status) FROM radroots_event_store_food_availability_projection ORDER BY pubkey, d_tag",
        )
        .await,
        query_string_rows(
            store.pool(),
            "SELECT printf('%s|%s|%d|%s|%s|%s|%s|%d', pubkey, d_tag, image_index, raw_tag_json, quote(url), quote(width), quote(height), qualifies) FROM radroots_event_store_food_availability_image ORDER BY pubkey, d_tag, image_index",
        )
        .await,
        query_string_rows(
            store.pool(),
            "SELECT printf('%s|%s|%s|%s|%s|%s|%s', event_id, pubkey, d_tag, title, summary, content, location) FROM radroots_event_store_food_availability_search_fts ORDER BY event_id",
        )
        .await,
    ]
}

async fn set_trigger_guarded_drift(
    store: &RadrootsEventStore,
    trigger: &'static str,
    mutation: &'static str,
) {
    let trigger_sql: String = sqlx::query_scalar(
        "SELECT sql FROM main.sqlite_schema WHERE type = 'trigger' AND name = ?",
    )
    .bind(trigger)
    .fetch_one(store.pool())
    .await
    .expect("load guard SQL");
    sqlx::query(sqlx::AssertSqlSafe(format!("DROP TRIGGER main.{trigger}")))
        .execute(store.pool())
        .await
        .expect("drop guard");
    sqlx::query(mutation)
        .execute(store.pool())
        .await
        .expect("forge drift");
    sqlx::query(sqlx::AssertSqlSafe(trigger_sql))
        .execute(store.pool())
        .await
        .expect("restore guard");
}

async fn transition_high_water(store: &RadrootsEventStore) -> i64 {
    sqlx::query_scalar(
        "SELECT COALESCE(MAX(transition_seq), 0) FROM radroots_event_store_addressable_head_transition",
    )
    .fetch_one(store.pool())
    .await
    .expect("transition high-water")
}

async fn assert_nonempty_transition_high_water_drift_is_repaired(
    drift_sql: &'static str,
    pristine_generation: u8,
    repair_generation: u8,
    repeat_generation: u8,
) {
    let store = RadrootsEventStore::open_memory().await.expect("open");
    seed_food_fixture(&store).await;
    let pristine = rebuild_from_raw_v1_on_pool_for_test(
        store.pool(),
        &FixedGeneration(pristine_generation),
        None,
    )
    .await
    .expect("establish pristine rebuild");
    let pristine_product = logical_product_snapshot(&store).await;
    let prior_transition_high_water = transition_high_water(&store).await;
    assert!(prior_transition_high_water > 0);

    set_trigger_guarded_drift(
        &store,
        "radroots_event_store_source_state_authority_update_guard",
        drift_sql,
    )
    .await;
    let drifted_high_water: i64 = sqlx::query_scalar(
        "SELECT last_transition_seq FROM radroots_event_store_source_state WHERE singleton = 1",
    )
    .fetch_one(store.pool())
    .await
    .expect("drifted source-state high-water");
    assert_ne!(drifted_high_water, prior_transition_high_water);

    let repaired = rebuild_from_raw_v1_on_pool_for_test(
        store.pool(),
        &FixedGeneration(repair_generation),
        None,
    )
    .await
    .expect("repair active transition high-water drift");
    assert_eq!(
        repaired.immutable_raw_digest(),
        pristine.immutable_raw_digest()
    );
    assert_eq!(
        repaired.active_product_state_digest(),
        pristine.active_product_state_digest()
    );
    assert_eq!(logical_product_snapshot(&store).await, pristine_product);

    let repaired_generation = repaired.new_source_generation();
    let repaired_floor: i64 = sqlx::query_scalar(
        "SELECT transition_floor_seq FROM radroots_event_store_source_generation WHERE source_generation = ?",
    )
    .bind(repaired_generation.as_bytes().as_slice())
    .fetch_one(store.pool())
    .await
    .expect("repaired generation transition floor");
    assert_eq!(repaired_floor, prior_transition_high_water);
    let repaired_state_high_water: i64 = sqlx::query_scalar(
        "SELECT last_transition_seq FROM radroots_event_store_source_state WHERE singleton = 1",
    )
    .fetch_one(store.pool())
    .await
    .expect("repaired source-state high-water");
    assert_eq!(
        repaired_state_high_water,
        transition_high_water(&store).await
    );

    store
        .migrate_to_current_schema()
        .await
        .expect("ordinary reopen validation after repair");
    let repeated = rebuild_from_raw_v1_on_pool_for_test(
        store.pool(),
        &FixedGeneration(repeat_generation),
        None,
    )
    .await
    .expect("repeat rebuild after repair");
    assert_eq!(
        repeated.immutable_raw_digest(),
        repaired.immutable_raw_digest()
    );
    assert_eq!(
        repeated.active_product_state_digest(),
        repaired.active_product_state_digest()
    );
}

async fn insert_projection_cursor_capacity(store: &RadrootsEventStore) {
    sqlx::query(
        "WITH digits(n) AS (VALUES (0),(1),(2),(3),(4),(5),(6),(7),(8),(9),(10),(11),(12),(13),(14),(15)) INSERT INTO projection_cursor(projection_id, projection_version, last_event_seq, updated_at_ms) SELECT printf('projection-%04d', high.n * 256 + middle.n * 16 + low.n), 1, 0, 1 FROM digits AS high CROSS JOIN digits AS middle CROSS JOIN digits AS low",
    )
    .execute(store.pool())
    .await
    .expect("fill governed cursor capacity");
}

#[tokio::test]
async fn raw_source_rebuild_incremental_reopen_and_repeat_parity_v1() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let path = tempdir.path().join("raw-rebuild-parity.sqlite");
    let store = RadrootsEventStore::open_file(&path)
        .await
        .expect("open file");
    seed_fixture_case(
        &store,
        FOOD_FIXTURE,
        "post_cutoff_replacement_restores_projection",
        "events",
    )
    .await;
    let incremental = logical_product_snapshot(&store).await;

    let first = rebuild_from_raw_v1_on_pool_for_test(store.pool(), &FixedGeneration(0x21), None)
        .await
        .expect("first rebuild");
    assert_eq!(logical_product_snapshot(&store).await, incremental);
    store.pool().close().await;

    let reopened = RadrootsEventStore::open_file(&path)
        .await
        .expect("strict reopen");
    assert_eq!(logical_product_snapshot(&reopened).await, incremental);
    let second =
        rebuild_from_raw_v1_on_pool_for_test(reopened.pool(), &FixedGeneration(0x22), None)
            .await
            .expect("second rebuild");
    let third = rebuild_from_raw_v1_on_pool_for_test(reopened.pool(), &FixedGeneration(0x23), None)
        .await
        .expect("repeat rebuild");
    assert_eq!(logical_product_snapshot(&reopened).await, incremental);
    assert_eq!(first.immutable_raw_digest(), second.immutable_raw_digest());
    assert_eq!(second.immutable_raw_digest(), third.immutable_raw_digest());
    assert_eq!(
        first.active_product_state_digest(),
        second.active_product_state_digest()
    );
    assert_eq!(
        second.active_product_state_digest(),
        third.active_product_state_digest()
    );

    for (index, (bytes, case_id, events_field)) in [
        (
            FOOD_FIXTURE,
            "invalid_same_timestamp_winner_retracts_projection",
            "events",
        ),
        (
            FOOD_FIXTURE,
            "blossom_digest_and_image_diagnostics_are_preserved",
            "events",
        ),
        (
            FOOD_FIXTURE,
            "wrong_author_address_deletion_preserves_projection",
            "events",
        ),
        (
            FOOD_FIXTURE,
            "operational_listing_head_retracts_food_projection",
            "events",
        ),
        (
            NIP09_FIXTURE,
            "maximum_address_cutoff_is_order_independent",
            "input_events",
        ),
        (
            NIP09_FIXTURE,
            "later_revision_survives_maximum_address_cutoff",
            "input_events",
        ),
        (
            NIP09_FIXTURE,
            "unauthorized_exact_reference_does_not_override_authorized_stale_cutoff",
            "input_events",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let case_store = RadrootsEventStore::open_memory()
            .await
            .expect("open case store");
        seed_fixture_case(&case_store, bytes, case_id, events_field).await;
        let incremental = logical_product_snapshot(&case_store).await;
        let generation = u8::try_from(0x80 + index).expect("fixture generation");
        let rebuilt = rebuild_from_raw_v1_on_pool_for_test(
            case_store.pool(),
            &FixedGeneration(generation),
            None,
        )
        .await
        .unwrap_or_else(|error| panic!("rebuild fixture case {case_id}: {error}"));
        assert_eq!(
            logical_product_snapshot(&case_store).await,
            incremental,
            "fixture case {case_id}"
        );
        let repeated = rebuild_from_raw_v1_on_pool_for_test(
            case_store.pool(),
            &FixedGeneration(generation + 0x10),
            None,
        )
        .await
        .unwrap_or_else(|error| panic!("repeat fixture case {case_id}: {error}"));
        assert_eq!(
            rebuilt.immutable_raw_digest(),
            repeated.immutable_raw_digest(),
            "fixture case {case_id} raw digest"
        );
        assert_eq!(
            rebuilt.active_product_state_digest(),
            repeated.active_product_state_digest(),
            "fixture case {case_id} product digest"
        );
        let deletion_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM event_envelopes WHERE kind = 5")
                .fetch_one(case_store.pool())
                .await
                .expect("deletion count");
        let visible_deletion_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM radroots_event_store_current_visibility_v1 AS visibility JOIN event_envelopes AS event USING (event_id) WHERE event.kind = 5 AND visibility.current_visibility = 'visible'",
        )
        .fetch_one(case_store.pool())
        .await
        .expect("visible deletion count");
        assert_eq!(visible_deletion_count, deletion_count, "kind-5 immunity");
    }
}

#[tokio::test]
async fn projection_cursor_capacity_accepts_exact_and_rejects_one_over_v1() {
    let store = RadrootsEventStore::open_memory().await.expect("open");
    rollback_event_store_schema_offline_destructive_for_migration_test(store.pool(), 1)
        .await
        .expect("rollback exact-cap store to v1");
    insert_projection_cursor_capacity(&store).await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM projection_cursor")
            .fetch_one(store.pool())
            .await
            .expect("cursor count"),
        i64::from(RADROOTS_EVENT_STORE_PROJECTION_CURSOR_COUNT_LIMIT_V1)
    );
    store
        .migrate_to_current_schema()
        .await
        .expect("v1 migration accepts exact cursor capacity");

    let generation = store.source_generation().await.expect("generation");
    let existing_ticket = store
        .prepare_projection_cursor_rebuild("projection-0000", 1)
        .await
        .expect("prepare existing cursor binding at exact capacity");
    store
        .reset_projection_cursor_after_rebuild(existing_ticket, 2)
        .await
        .expect("bind existing cursor at exact capacity");
    let existing = RadrootsProjectionCursor::new("projection-0000", 1, generation, 0, 3)
        .expect("existing cursor update");
    store
        .compare_and_swap_projection_cursor(&existing, Some(0))
        .await
        .expect("existing cursor remains updateable at exact capacity");
    assert_eq!(
        store
            .projection_cursor("projection-0000", 1)
            .await
            .expect("read existing cursor")
            .expect("existing cursor")
            .updated_at_ms(),
        3
    );
    let overflow = RadrootsProjectionCursor::new("projection-overflow", 1, generation, 0, 2)
        .expect("overflow cursor");
    assert!(matches!(
        store
            .compare_and_swap_projection_cursor(&overflow, None)
            .await,
        Err(RadrootsEventStoreError::ProjectionCursorCapacityExceeded { current, limit })
            if current == limit && limit == RADROOTS_EVENT_STORE_PROJECTION_CURSOR_COUNT_LIMIT_V1
    ));
    let ticket = store
        .prepare_projection_cursor_rebuild("projection-reset-overflow", 1)
        .await
        .expect("missing cursor ticket");
    assert!(matches!(
        store.reset_projection_cursor_after_rebuild(ticket, 3).await,
        Err(RadrootsEventStoreError::ProjectionCursorCapacityExceeded { current, limit })
            if current == limit && limit == RADROOTS_EVENT_STORE_PROJECTION_CURSOR_COUNT_LIMIT_V1
    ));

    let one_over = RadrootsEventStore::open_memory()
        .await
        .expect("open one-over store");
    rollback_event_store_schema_offline_destructive_for_migration_test(one_over.pool(), 1)
        .await
        .expect("rollback one-over store to v1");
    insert_projection_cursor_capacity(&one_over).await;
    sqlx::query(
        "INSERT INTO projection_cursor(projection_id, projection_version, last_event_seq, updated_at_ms) VALUES ('projection-direct-overflow', 1, 0, 4)",
    )
    .execute(one_over.pool())
    .await
    .expect("forge one-over cursor inventory");
    assert!(matches!(
        one_over.migrate_to_current_schema().await,
        Err(RadrootsEventStoreError::ProjectionCursorCapacityExceeded { current, limit })
            if current == limit + 1
                && limit == RADROOTS_EVENT_STORE_PROJECTION_CURSOR_COUNT_LIMIT_V1
    ));
}

#[tokio::test]
async fn raw_source_rebuild_invalidates_generic_cursors_without_enumerating_or_mutating_them_v1() {
    let store = RadrootsEventStore::open_memory().await.expect("open");
    let generation = store.source_generation().await.expect("generation");
    let cursor =
        RadrootsProjectionCursor::new("generic", 1, generation, 0, 10).expect("generic cursor");
    store
        .compare_and_swap_projection_cursor(&cursor, None)
        .await
        .expect("insert cursor");
    let before = query_string_rows(
        store.pool(),
        "SELECT printf('%s|%d|%d|%d|%s|%d', cursor.projection_id, cursor.projection_version, cursor.last_event_seq, cursor.updated_at_ms, hex(source.source_generation), source.source_revision) FROM projection_cursor AS cursor JOIN radroots_event_store_projection_cursor_source AS source USING (projection_id) ORDER BY cursor.projection_id",
    )
    .await;

    store.rebuild_from_raw_v1().await.expect("rebuild");
    let after = query_string_rows(
        store.pool(),
        "SELECT printf('%s|%d|%d|%d|%s|%d', cursor.projection_id, cursor.projection_version, cursor.last_event_seq, cursor.updated_at_ms, hex(source.source_generation), source.source_revision) FROM projection_cursor AS cursor JOIN radroots_event_store_projection_cursor_source AS source USING (projection_id) ORDER BY cursor.projection_id",
    )
    .await;
    assert_eq!(after, before);
    assert!(matches!(
        store.projection_cursor("generic", 1).await,
        Err(RadrootsEventStoreError::ProjectionSourceGenerationMismatch { projection_id })
            if projection_id == "generic"
    ));
}

#[tokio::test]
async fn raw_source_rebuild_normalizes_only_transition_sqlite_sequence_v1() {
    for (index, corruption) in ["missing", "low", "high", "duplicate", "case_alias"]
        .into_iter()
        .enumerate()
    {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        seed_food_fixture(&store).await;
        for caller_index in 0..64 {
            let create = format!(
                "CREATE TABLE caller_autoincrement_{caller_index}(id INTEGER PRIMARY KEY AUTOINCREMENT, value TEXT NOT NULL)"
            );
            sqlx::query(sqlx::AssertSqlSafe(create))
                .execute(store.pool())
                .await
                .expect("caller table");
            let insert = format!(
                "INSERT INTO caller_autoincrement_{caller_index}(value) VALUES ('preserve')"
            );
            sqlx::query(sqlx::AssertSqlSafe(insert))
                .execute(store.pool())
                .await
                .expect("caller row");
        }
        match corruption {
            "missing" => {
                sqlx::query("DELETE FROM main.sqlite_sequence WHERE name = ?")
                    .bind(TRANSITION_SEQUENCE_NAME)
                    .execute(store.pool())
                    .await
                    .expect("remove target sequence");
            }
            "low" => {
                sqlx::query("UPDATE main.sqlite_sequence SET seq = 0 WHERE name = ?")
                    .bind(TRANSITION_SEQUENCE_NAME)
                    .execute(store.pool())
                    .await
                    .expect("lower target sequence");
            }
            "high" => {
                sqlx::query("UPDATE main.sqlite_sequence SET seq = 99 WHERE name = ?")
                    .bind(TRANSITION_SEQUENCE_NAME)
                    .execute(store.pool())
                    .await
                    .expect("raise target sequence");
            }
            "duplicate" => {
                sqlx::query("INSERT INTO main.sqlite_sequence(name, seq) VALUES (?, 99)")
                    .bind(TRANSITION_SEQUENCE_NAME)
                    .execute(store.pool())
                    .await
                    .expect("duplicate target sequence");
            }
            "case_alias" => {
                sqlx::query("UPDATE main.sqlite_sequence SET name = upper(name) WHERE name = ?")
                    .bind(TRANSITION_SEQUENCE_NAME)
                    .execute(store.pool())
                    .await
                    .expect("retarget sequence name casing");
            }
            _ => unreachable!("closed sequence corruption matrix"),
        }
        let unrelated_before = query_string_rows(
            store.pool(),
            "SELECT printf('%d|%s|%s', rowid, quote(name), quote(seq)) FROM main.sqlite_sequence WHERE name IS NULL OR name COLLATE NOCASE != 'radroots_event_store_addressable_head_transition' ORDER BY rowid",
        )
        .await;

        rebuild_from_raw_v1_on_pool_for_test(
            store.pool(),
            &FixedGeneration(u8::try_from(0x31 + index).expect("test generation")),
            None,
        )
        .await
        .unwrap_or_else(|error| panic!("rebuild {corruption} sequence: {error}"));
        let target_sequences: Vec<(i64, String, Option<i64>)> = sqlx::query_as(
            "SELECT rowid, name, seq FROM main.sqlite_sequence WHERE name COLLATE NOCASE = ? ORDER BY rowid",
        )
        .bind(TRANSITION_SEQUENCE_NAME)
        .fetch_all(store.pool())
        .await
        .expect("target sequence rows");
        assert_eq!(target_sequences.len(), 1, "{corruption}");
        let target = &target_sequences[0];
        assert_eq!(target.1, TRANSITION_SEQUENCE_NAME, "{corruption}");
        assert_eq!(target.2, Some(2), "{corruption}");
        assert_eq!(
            sqlx::query_as::<_, (i64, String)>(
                "SELECT rowid, name FROM main.sqlite_sequence ORDER BY rowid LIMIT 1",
            )
            .fetch_one(store.pool())
            .await
            .expect("first sequence row"),
            (target.0, TRANSITION_SEQUENCE_NAME.to_owned()),
            "{corruption}"
        );
        let unrelated_after = query_string_rows(
            store.pool(),
            "SELECT printf('%d|%s|%s', rowid, quote(name), quote(seq)) FROM main.sqlite_sequence WHERE name IS NULL OR name COLLATE NOCASE != 'radroots_event_store_addressable_head_transition' ORDER BY rowid",
        )
        .await;
        assert_eq!(unrelated_after, unrelated_before, "{corruption}");
    }
}

#[tokio::test]
async fn raw_source_rebuild_rejects_unrelated_minimum_transition_sequence_rowid_v1() {
    let store = RadrootsEventStore::open_memory().await.expect("open");
    seed_food_fixture(&store).await;
    sqlx::query("INSERT INTO main.sqlite_sequence(rowid, name, seq) VALUES (?, ?, 0)")
        .bind(i64::MIN)
        .bind("caller_minimum_sequence")
        .execute(store.pool())
        .await
        .expect("occupy reserved sequence rowid");
    let before = rebuild_authority_snapshot(&store).await;

    assert!(matches!(
        rebuild_from_raw_v1_on_pool_for_test(store.pool(), &FixedGeneration(0x3f), None,).await,
        Err(RadrootsEventStoreError::RawSourceRebuildStateDrift {
            kind: RadrootsEventStoreRawSourceRebuildDriftV1::AddressableTransitionAuthority,
            ..
        })
    ));
    assert_eq!(rebuild_authority_snapshot(&store).await, before);
}

#[tokio::test]
async fn raw_source_rebuild_reuses_target_alias_at_minimum_sequence_rowid_v1() {
    let store = RadrootsEventStore::open_memory().await.expect("open");
    seed_food_fixture(&store).await;
    sqlx::query("DELETE FROM main.sqlite_sequence WHERE name COLLATE NOCASE = ?")
        .bind(TRANSITION_SEQUENCE_NAME)
        .execute(store.pool())
        .await
        .expect("remove canonical target row");
    sqlx::query("INSERT INTO main.sqlite_sequence(rowid, name, seq) VALUES (?, upper(?), 99)")
        .bind(i64::MIN)
        .bind(TRANSITION_SEQUENCE_NAME)
        .execute(store.pool())
        .await
        .expect("insert minimum target alias");

    rebuild_from_raw_v1_on_pool_for_test(store.pool(), &FixedGeneration(0x40), None)
        .await
        .expect("rebuild target alias");
    assert_eq!(
        sqlx::query_as::<_, (i64, String, i64)>(
            "SELECT rowid, name, seq FROM main.sqlite_sequence ORDER BY rowid LIMIT 1",
        )
        .fetch_one(store.pool())
        .await
        .expect("canonical minimum target row"),
        (i64::MIN, TRANSITION_SEQUENCE_NAME.to_owned(), 2)
    );
}

#[tokio::test]
async fn raw_source_rebuild_repairs_derived_drift_and_refuses_raw_drift_atomically_v1() {
    let store = RadrootsEventStore::open_memory().await.expect("open");
    seed_fixture_case(
        &store,
        FOOD_FIXTURE,
        "post_cutoff_replacement_restores_projection",
        "events",
    )
    .await;
    let pristine = logical_product_snapshot(&store).await;
    set_trigger_guarded_drift(
        &store,
        "radroots_event_store_event_envelopes_derived_update_guard",
        "UPDATE event_envelopes SET contract_status = 'invalid', contract_id = NULL, event_class = NULL, projection_eligible = 0",
    )
    .await;
    set_trigger_guarded_drift(
        &store,
        "radroots_event_store_nip09_request_update_guard",
        "UPDATE radroots_event_store_nip09_request SET request_created_at = request_created_at + 1",
    )
    .await;
    set_trigger_guarded_drift(
        &store,
        "radroots_event_store_food_availability_cursor_update_guard",
        "UPDATE radroots_event_store_food_availability_cursor SET projected_row_count = 0",
    )
    .await;
    set_trigger_guarded_drift(
        &store,
        "radroots_event_store_food_availability_projection_delete_guard",
        "DELETE FROM radroots_event_store_food_availability_projection",
    )
    .await;
    set_trigger_guarded_drift(
        &store,
        "radroots_event_store_addressable_state_delete_guard",
        "DELETE FROM radroots_event_store_addressable_head_state",
    )
    .await;
    set_trigger_guarded_drift(
        &store,
        "radroots_event_store_event_head_delete_guard",
        "DELETE FROM event_envelope_head",
    )
    .await;
    sqlx::query("DELETE FROM radroots_event_store_food_availability_search_fts")
        .execute(store.pool())
        .await
        .expect("forge Food search drift");
    rebuild_from_raw_v1_on_pool_for_test(store.pool(), &FixedGeneration(0x32), None)
        .await
        .expect("repair derived drift");
    assert_eq!(logical_product_snapshot(&store).await, pristine);

    set_trigger_guarded_drift(
        &store,
        "radroots_event_store_event_envelopes_raw_update_guard",
        "UPDATE event_envelopes SET content = 'Parsnip available this week.' WHERE content = 'Carrots available this week.'",
    )
    .await;
    let before = rebuild_authority_snapshot(&store).await;
    assert!(matches!(
        rebuild_from_raw_v1_on_pool_for_test(store.pool(), &FixedGeneration(0x33), None).await,
        Err(RadrootsEventStoreError::RawEventReconciliationMismatch { field, .. })
            if field == "content"
    ));
    assert_eq!(rebuild_authority_snapshot(&store).await, before);

    let capacity_store = RadrootsEventStore::open_memory()
        .await
        .expect("open capacity-drift store");
    seed_food_fixture(&capacity_store).await;
    set_trigger_guarded_drift(
        &capacity_store,
        "radroots_event_store_source_capacity_update_guard",
        "UPDATE radroots_event_store_source_capacity_v1 SET raw_event_count = raw_event_count + 1",
    )
    .await;
    let capacity_before = rebuild_authority_snapshot(&capacity_store).await;
    assert!(matches!(
        rebuild_from_raw_v1_on_pool_for_test(capacity_store.pool(), &FixedGeneration(0x34), None,)
            .await,
        Err(RadrootsEventStoreError::SourceCapacityStateDrift { reason })
            if reason == "capacity seal does not match active source state and generation history"
    ));
    assert_eq!(
        rebuild_authority_snapshot(&capacity_store).await,
        capacity_before
    );

    let catalog_store = RadrootsEventStore::open_memory()
        .await
        .expect("open catalog-drift store");
    seed_food_fixture(&catalog_store).await;
    sqlx::query("DROP INDEX event_envelope_kind_created_idx")
        .execute(catalog_store.pool())
        .await
        .expect("forge governed catalog drift");
    let catalog_before = rebuild_authority_snapshot(&catalog_store).await;
    assert!(matches!(
        rebuild_from_raw_v1_on_pool_for_test(catalog_store.pool(), &FixedGeneration(0x35), None,)
            .await,
        Err(RadrootsEventStoreError::SchemaFingerprintMismatch { .. })
    ));
    assert_eq!(
        rebuild_authority_snapshot(&catalog_store).await,
        catalog_before
    );

    let ledger_store = RadrootsEventStore::open_memory()
        .await
        .expect("open ledger-drift store");
    seed_food_fixture(&ledger_store).await;
    sqlx::query(
        "UPDATE radroots_event_store_schema_migrations SET up_sha256 = ? WHERE version = 4",
    )
    .bind("0".repeat(64))
    .execute(ledger_store.pool())
    .await
    .expect("forge migration-ledger drift");
    let ledger_before = rebuild_authority_snapshot(&ledger_store).await;
    assert!(matches!(
        rebuild_from_raw_v1_on_pool_for_test(ledger_store.pool(), &FixedGeneration(0x36), None,)
            .await,
        Err(RadrootsEventStoreError::MigrationHistoryChecksumDrift {
            version: 4,
            field: "up_sha256",
            ..
        })
    ));
    assert_eq!(
        rebuild_authority_snapshot(&ledger_store).await,
        ledger_before
    );
}

#[tokio::test]
async fn raw_source_rebuild_failpoints_roll_back_every_stage_v1() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    for (index, failpoint) in [
        RawSourceRebuildFailpointV1::AfterMarkerOpen,
        RawSourceRebuildFailpointV1::AfterGenerationRotation,
        RawSourceRebuildFailpointV1::AfterCoreReplay,
        RawSourceRebuildFailpointV1::AfterVisibilityAudit,
        RawSourceRebuildFailpointV1::AfterFoodResetAndReplay,
        RawSourceRebuildFailpointV1::AfterFoodAudit,
        RawSourceRebuildFailpointV1::AfterMarkerClose,
    ]
    .into_iter()
    .enumerate()
    {
        let path = tempdir.path().join(format!("failpoint-{index}.sqlite"));
        let store = RadrootsEventStore::open_file(&path).await.expect("open");
        seed_fixture_case(
            &store,
            FOOD_FIXTURE,
            "authorized_address_deletion_retracts_projection",
            "events",
        )
        .await;
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM radroots_event_store_nip09_request",
            )
            .fetch_one(store.pool())
            .await
            .expect("NIP-09 request count"),
            1
        );
        let before = rebuild_authority_snapshot(&store).await;
        let error = rebuild_from_raw_v1_on_pool_for_test(
            store.pool(),
            &FixedGeneration(0x41),
            Some(failpoint),
        )
        .await
        .expect_err("injected rebuild must fail");
        assert!(matches!(
            error,
            RadrootsEventStoreError::RawSourceRebuildStateDrift {
                kind: RadrootsEventStoreRawSourceRebuildDriftV1::RebuildPostcondition,
                ..
            }
        ));
        assert_eq!(rebuild_authority_snapshot(&store).await, before);
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM radroots_event_store_source_rebuild_marker",
            )
            .fetch_one(store.pool())
            .await
            .expect("marker count"),
            0
        );
        store.pool().close().await;
        let reopened = RadrootsEventStore::open_file(&path)
            .await
            .expect("strict reopen after rollback");
        assert_eq!(rebuild_authority_snapshot(&reopened).await, before);
    }
}

#[test]
fn raw_source_rebuild_rollback_failure_preserves_primary_and_rollback_errors_v1() {
    let primary = RadrootsEventStoreError::RawSourceRebuildStateDrift {
        kind: RadrootsEventStoreRawSourceRebuildDriftV1::RebuildPostcondition,
        detail: "primary".to_owned(),
    };
    let rollback = sqlx::Error::Protocol("rollback".to_owned());
    assert!(matches!(
        preserve_raw_source_rebuild_primary_failure_for_test::<()>(primary, Err(rollback)),
        Err(RadrootsEventStoreError::RawSourceRebuildTransactionRollbackFailed {
            primary,
            rollback: sqlx::Error::Protocol(_),
        }) if matches!(
            *primary,
            RadrootsEventStoreError::RawSourceRebuildStateDrift {
                kind: RadrootsEventStoreRawSourceRebuildDriftV1::RebuildPostcondition,
                ..
            }
        )
    ));
}

#[test]
fn raw_source_rebuild_drift_kind_codes_and_display_are_stable_v1() {
    for (kind, expected) in [
        (
            RadrootsEventStoreRawSourceRebuildDriftV1::ManagedSchemaAuthority,
            "managed_schema_authority",
        ),
        (
            RadrootsEventStoreRawSourceRebuildDriftV1::ImmutableRawAuthority,
            "immutable_raw_authority",
        ),
        (
            RadrootsEventStoreRawSourceRebuildDriftV1::SourceGenerationLineage,
            "source_generation_lineage",
        ),
        (
            RadrootsEventStoreRawSourceRebuildDriftV1::AddressableTransitionAuthority,
            "addressable_transition_authority",
        ),
        (
            RadrootsEventStoreRawSourceRebuildDriftV1::DerivedProductStateAuthority,
            "derived_product_state_authority",
        ),
        (
            RadrootsEventStoreRawSourceRebuildDriftV1::RebuildPostcondition,
            "rebuild_postcondition",
        ),
    ] {
        assert_eq!(kind.code(), expected);
        assert_eq!(kind.to_string(), expected);
    }

    let error = RadrootsEventStoreError::RawSourceRebuildStateDrift {
        kind: RadrootsEventStoreRawSourceRebuildDriftV1::ImmutableRawAuthority,
        detail: "diagnostic context".to_owned(),
    };
    assert_eq!(
        error.to_string(),
        "event-store raw-source rebuild authority is inconsistent (immutable_raw_authority): diagnostic context"
    );
}

#[tokio::test]
async fn raw_source_rebuild_wal_readers_observe_only_committed_generation_v1() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let path = tempdir.path().join("raw-rebuild-wal.sqlite");
    let writer = RadrootsEventStore::open_file(&path).await.expect("writer");
    seed_food_fixture(&writer).await;
    let reader = RadrootsEventStore::open_file(&path).await.expect("reader");
    assert_eq!(writer.pragma_journal_mode().await.expect("WAL"), "wal");
    let prior_generation = reader.source_generation().await.expect("prior generation");

    let mut transaction = writer.begin_write_transaction().await.expect("writer tx");
    let report =
        rebuild_from_raw_v1_in_transaction_for_test(&mut transaction, &FixedGeneration(0x51))
            .await
            .expect("uncommitted rebuild");
    assert_eq!(
        reader.source_generation().await.expect("reader snapshot"),
        prior_generation
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM radroots_event_store_source_rebuild_marker",
        )
        .fetch_one(reader.pool())
        .await
        .expect("reader marker count"),
        0
    );
    transaction.commit().await.expect("commit rebuild");
    assert_eq!(
        reader
            .source_generation()
            .await
            .expect("committed generation"),
        report.new_source_generation()
    );
}

#[tokio::test]
async fn raw_source_rebuild_rejects_caller_inbound_foreign_keys_atomically_v1() {
    for (suffix, on_delete) in [
        ("cascade", "CASCADE"),
        ("set_null", "SET NULL"),
        ("set_default", "SET DEFAULT"),
        ("restrict", "RESTRICT"),
        ("no_action", "NO ACTION"),
    ] {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        seed_food_fixture(&store).await;
        let child_table = format!("caller_inbound_{suffix}");
        sqlx::query(
            "CREATE TABLE caller_rebuild_side_effect(id INTEGER PRIMARY KEY AUTOINCREMENT, action TEXT NOT NULL)",
        )
        .execute(store.pool())
        .await
        .expect("caller side-effect table");
        let parent_table = if suffix == "cascade" {
            "RADROOTS_EVENT_STORE_FOOD_AVAILABILITY_CURSOR"
        } else {
            "radroots_event_store_food_availability_cursor"
        };
        let create_child = format!(
            "CREATE TABLE {child_table}(id INTEGER PRIMARY KEY, parent_singleton INTEGER DEFAULT 1 REFERENCES {parent_table}(singleton) ON DELETE {on_delete}, note TEXT NOT NULL)"
        );
        sqlx::query(sqlx::AssertSqlSafe(create_child))
            .execute(store.pool())
            .await
            .expect("caller inbound-FK table");
        for (trigger_suffix, trigger_event) in [
            ("delete", "AFTER DELETE"),
            ("update", "AFTER UPDATE OF parent_singleton"),
        ] {
            let create_trigger = format!(
                "CREATE TRIGGER {child_table}_{trigger_suffix}_side_effect {trigger_event} ON {child_table} BEGIN INSERT INTO caller_rebuild_side_effect(action) VALUES ('{trigger_suffix}'); END"
            );
            sqlx::query(sqlx::AssertSqlSafe(create_trigger))
                .execute(store.pool())
                .await
                .expect("caller child side-effect trigger");
        }
        let insert_child = format!(
            "INSERT INTO {child_table}(id, parent_singleton, note) VALUES (1, 1, 'preserve')"
        );
        sqlx::query(sqlx::AssertSqlSafe(insert_child))
            .execute(store.pool())
            .await
            .expect("caller dependent row");

        if suffix == "cascade" {
            let mut connection = store.pool().acquire().await.expect("connection");
            sqlx::query("CREATE TEMP TABLE pragma_foreign_key_list(value TEXT)")
                .execute(&mut *connection)
                .await
                .expect("temporary pragma decoy");
        }

        let authority_before = rebuild_authority_snapshot(&store).await;
        let child_before = sqlx::query_scalar::<_, String>(sqlx::AssertSqlSafe(format!(
            "SELECT printf('%d|%s|%s', id, quote(parent_singleton), note) FROM {child_table} ORDER BY id"
        )))
        .fetch_all(store.pool())
        .await
        .expect("caller child snapshot");
        let schema_before = query_string_rows(
            store.pool(),
            "SELECT printf('%s|%s|%s|%s', type, name, tbl_name, sql) FROM main.sqlite_schema WHERE name LIKE 'caller_%' ORDER BY type, name",
        )
        .await;
        let side_effect_before = query_string_rows(
            store.pool(),
            "SELECT printf('%d|%s', id, action) FROM caller_rebuild_side_effect ORDER BY id",
        )
        .await;

        let error = rebuild_from_raw_v1_on_pool_for_test(store.pool(), &PanickingGeneration, None)
            .await
            .expect_err("caller inbound FK must be rejected before entropy");
        match error {
            RadrootsEventStoreError::RawSourceRebuildCallerInboundForeignKeyUnsupported {
                dependency,
            } => {
                assert_eq!(dependency.child_table, child_table);
                assert_eq!(dependency.foreign_key_id, 0);
                assert_eq!(dependency.foreign_key_sequence, 0);
                assert_eq!(dependency.child_column, "parent_singleton");
                assert_eq!(
                    dependency.parent_table,
                    "radroots_event_store_food_availability_cursor"
                );
                assert_eq!(dependency.parent_column.as_deref(), Some("singleton"));
                assert_eq!(dependency.on_update, "NO ACTION");
                assert_eq!(dependency.on_delete, on_delete);
                assert_eq!(dependency.match_clause, "NONE");
            }
            other => panic!("unexpected inbound-FK refusal: {other:?}"),
        }

        assert_eq!(rebuild_authority_snapshot(&store).await, authority_before);
        assert_eq!(
            sqlx::query_scalar::<_, String>(sqlx::AssertSqlSafe(format!(
                "SELECT printf('%d|%s|%s', id, quote(parent_singleton), note) FROM {child_table} ORDER BY id"
            )))
            .fetch_all(store.pool())
            .await
            .expect("caller child after refusal"),
            child_before
        );
        assert_eq!(
            query_string_rows(
                store.pool(),
                "SELECT printf('%s|%s|%s|%s', type, name, tbl_name, sql) FROM main.sqlite_schema WHERE name LIKE 'caller_%' ORDER BY type, name",
            )
            .await,
            schema_before
        );
        assert_eq!(
            query_string_rows(
                store.pool(),
                "SELECT printf('%d|%s', id, action) FROM caller_rebuild_side_effect ORDER BY id",
            )
            .await,
            side_effect_before
        );
    }

    for (suffix, parent_table, parent_column) in [
        (
            "virtual",
            "radroots_event_store_food_availability_search_fts",
            "rowid",
        ),
        (
            "config",
            "radroots_event_store_food_availability_search_fts_config",
            "k",
        ),
        (
            "content",
            "radroots_event_store_food_availability_search_fts_content",
            "id",
        ),
        (
            "data",
            "radroots_event_store_food_availability_search_fts_data",
            "id",
        ),
        (
            "docsize",
            "radroots_event_store_food_availability_search_fts_docsize",
            "id",
        ),
        (
            "idx",
            "radroots_event_store_food_availability_search_fts_idx",
            "segid",
        ),
        ("sqlite_sequence", "sqlite_sequence", "rowid"),
    ] {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        seed_food_fixture(&store).await;
        let child_table = format!("caller_mutation_parent_{suffix}");
        let mut connection = store.pool().acquire().await.expect("connection");
        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(&mut *connection)
            .await
            .expect("disable FK checks for mutation-parent fixture setup");
        sqlx::query(
            "CREATE TABLE caller_mutation_parent_side_effect(id INTEGER PRIMARY KEY AUTOINCREMENT, action TEXT NOT NULL)",
        )
        .execute(&mut *connection)
        .await
        .expect("caller mutation-parent side-effect table");
        let create_child = format!(
            "CREATE TABLE {child_table}(id INTEGER PRIMARY KEY, parent_key, note TEXT NOT NULL, FOREIGN KEY(parent_key) REFERENCES {parent_table}({parent_column}) ON UPDATE SET NULL ON DELETE CASCADE)"
        );
        sqlx::query(sqlx::AssertSqlSafe(create_child))
            .execute(&mut *connection)
            .await
            .expect("caller mutation-parent inbound-FK table");
        for (trigger_suffix, trigger_event) in [
            ("delete", "AFTER DELETE"),
            ("update", "AFTER UPDATE OF parent_key"),
        ] {
            let create_trigger = format!(
                "CREATE TRIGGER {child_table}_{trigger_suffix}_side_effect {trigger_event} ON {child_table} BEGIN INSERT INTO caller_mutation_parent_side_effect(action) VALUES ('{trigger_suffix}'); END"
            );
            sqlx::query(sqlx::AssertSqlSafe(create_trigger))
                .execute(&mut *connection)
                .await
                .expect("caller mutation-parent child side-effect trigger");
        }
        let insert_child = format!(
            "INSERT INTO {child_table}(id, parent_key, note) SELECT 1, {parent_column}, 'preserve' FROM {parent_table} LIMIT 1"
        );
        let inserted = sqlx::query(sqlx::AssertSqlSafe(insert_child))
            .execute(&mut *connection)
            .await
            .expect("caller mutation-parent dependent row");
        assert_eq!(
            inserted.rows_affected(),
            1,
            "empty rebuild mutation parent {parent_table}"
        );
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&mut *connection)
            .await
            .expect("restore FK checks");
        drop(connection);

        let authority_before = rebuild_authority_snapshot(&store).await;
        let child_before = sqlx::query_scalar::<_, String>(sqlx::AssertSqlSafe(format!(
            "SELECT printf('%d|%s|%s', id, quote(parent_key), note) FROM {child_table} ORDER BY id"
        )))
        .fetch_all(store.pool())
        .await
        .expect("caller mutation-parent child snapshot");
        let schema_before = query_string_rows(
            store.pool(),
            "SELECT printf('%s|%s|%s|%s', type, name, tbl_name, sql) FROM main.sqlite_schema WHERE name LIKE 'caller_mutation_parent_%' ORDER BY type, name",
        )
        .await;
        let side_effect_before = query_string_rows(
            store.pool(),
            "SELECT printf('%d|%s', id, action) FROM caller_mutation_parent_side_effect ORDER BY id",
        )
        .await;

        let error = rebuild_from_raw_v1_on_pool_for_test(store.pool(), &PanickingGeneration, None)
            .await
            .expect_err("caller mutation-parent inbound FK must be rejected before entropy");
        match error {
            RadrootsEventStoreError::RawSourceRebuildCallerInboundForeignKeyUnsupported {
                dependency,
            } => {
                assert_eq!(dependency.child_table, child_table);
                assert_eq!(dependency.foreign_key_id, 0);
                assert_eq!(dependency.foreign_key_sequence, 0);
                assert_eq!(dependency.child_column, "parent_key");
                assert_eq!(dependency.parent_table, parent_table);
                assert_eq!(dependency.parent_column.as_deref(), Some(parent_column));
                assert_eq!(dependency.on_update, "SET NULL");
                assert_eq!(dependency.on_delete, "CASCADE");
                assert_eq!(dependency.match_clause, "NONE");
            }
            other => panic!("unexpected mutation-parent inbound-FK refusal: {other:?}"),
        }

        assert_eq!(rebuild_authority_snapshot(&store).await, authority_before);
        assert_eq!(
            sqlx::query_scalar::<_, String>(sqlx::AssertSqlSafe(format!(
                "SELECT printf('%d|%s|%s', id, quote(parent_key), note) FROM {child_table} ORDER BY id"
            )))
            .fetch_all(store.pool())
            .await
            .expect("caller mutation-parent child after refusal"),
            child_before
        );
        assert_eq!(
            query_string_rows(
                store.pool(),
                "SELECT printf('%s|%s|%s|%s', type, name, tbl_name, sql) FROM main.sqlite_schema WHERE name LIKE 'caller_mutation_parent_%' ORDER BY type, name",
            )
            .await,
            schema_before
        );
        assert_eq!(
            query_string_rows(
                store.pool(),
                "SELECT printf('%d|%s', id, action) FROM caller_mutation_parent_side_effect ORDER BY id",
            )
            .await,
            side_effect_before
        );
    }
}

#[tokio::test]
async fn raw_source_rebuild_caller_schema_inventory_limits_are_typed_and_atomic_v1() {
    let table_store = RadrootsEventStore::open_memory().await.expect("open");
    for table in ["caller_inventory_a", "caller_inventory_b"] {
        let create = format!("CREATE TABLE {table}(id INTEGER PRIMARY KEY, value TEXT NOT NULL)");
        sqlx::query(sqlx::AssertSqlSafe(create))
            .execute(table_store.pool())
            .await
            .expect("caller inventory table");
        let insert = format!("INSERT INTO {table}(id, value) VALUES (1, 'preserve')");
        sqlx::query(sqlx::AssertSqlSafe(insert))
            .execute(table_store.pool())
            .await
            .expect("caller inventory row");
    }
    let table_authority_before = rebuild_authority_snapshot(&table_store).await;
    let table_rows_before = query_string_rows(
        table_store.pool(),
        "SELECT (SELECT value FROM caller_inventory_a WHERE id = 1) || '|' || (SELECT value FROM caller_inventory_b WHERE id = 1)",
    )
    .await;
    assert!(matches!(
        rebuild_from_raw_v1_on_pool_with_caller_schema_limits_for_test(
            table_store.pool(),
            &PanickingGeneration,
            1,
            4_096,
        )
        .await,
        Err(
            RadrootsEventStoreError::RawSourceRebuildCallerTableCapacityExceeded {
                observed_at_least: 2,
                limit: 1,
            }
        )
    ));
    assert_eq!(
        rebuild_authority_snapshot(&table_store).await,
        table_authority_before
    );
    assert_eq!(
        query_string_rows(
            table_store.pool(),
            "SELECT (SELECT value FROM caller_inventory_a WHERE id = 1) || '|' || (SELECT value FROM caller_inventory_b WHERE id = 1)",
        )
        .await,
        table_rows_before
    );
    rebuild_from_raw_v1_on_pool_with_caller_schema_limits_for_test(
        table_store.pool(),
        &FixedGeneration(0x91),
        2,
        4_096,
    )
    .await
    .expect("exact caller-table capacity remains rebuildable");
    assert_eq!(
        query_string_rows(
            table_store.pool(),
            "SELECT (SELECT value FROM caller_inventory_a WHERE id = 1) || '|' || (SELECT value FROM caller_inventory_b WHERE id = 1)",
        )
        .await,
        table_rows_before
    );

    let foreign_key_store = RadrootsEventStore::open_memory().await.expect("open");
    sqlx::query("CREATE TABLE caller_fk_parent(id INTEGER PRIMARY KEY)")
        .execute(foreign_key_store.pool())
        .await
        .expect("caller FK parent");
    sqlx::query(
        "CREATE TABLE caller_fk_child(id INTEGER PRIMARY KEY, caller_parent_id INTEGER REFERENCES caller_fk_parent(id), managed_singleton INTEGER REFERENCES radroots_event_store_food_availability_cursor(singleton) ON DELETE CASCADE)",
    )
    .execute(foreign_key_store.pool())
    .await
    .expect("caller FK child");
    sqlx::query("INSERT INTO caller_fk_parent(id) VALUES (1)")
        .execute(foreign_key_store.pool())
        .await
        .expect("caller FK parent row");
    sqlx::query(
        "INSERT INTO caller_fk_child(id, caller_parent_id, managed_singleton) VALUES (1, 1, 1)",
    )
    .execute(foreign_key_store.pool())
    .await
    .expect("caller FK child row");
    let foreign_key_authority_before = rebuild_authority_snapshot(&foreign_key_store).await;
    let foreign_key_rows_before = query_string_rows(
        foreign_key_store.pool(),
        "SELECT printf('%d|%d|%d', id, caller_parent_id, managed_singleton) FROM caller_fk_child ORDER BY id",
    )
    .await;
    assert!(matches!(
        rebuild_from_raw_v1_on_pool_with_caller_schema_limits_for_test(
            foreign_key_store.pool(),
            &PanickingGeneration,
            2,
            1,
        )
        .await,
        Err(
            RadrootsEventStoreError::RawSourceRebuildCallerForeignKeyCapacityExceeded {
                observed_at_least: 2,
                limit: 1,
            }
        )
    ));
    assert_eq!(
        rebuild_authority_snapshot(&foreign_key_store).await,
        foreign_key_authority_before
    );
    assert_eq!(
        query_string_rows(
            foreign_key_store.pool(),
            "SELECT printf('%d|%d|%d', id, caller_parent_id, managed_singleton) FROM caller_fk_child ORDER BY id",
        )
        .await,
        foreign_key_rows_before
    );
    assert!(matches!(
        rebuild_from_raw_v1_on_pool_with_caller_schema_limits_for_test(
            foreign_key_store.pool(),
            &PanickingGeneration,
            2,
            2,
        )
        .await,
        Err(RadrootsEventStoreError::RawSourceRebuildCallerInboundForeignKeyUnsupported {
            dependency,
        }) if dependency.child_table == "caller_fk_child"
            && dependency.parent_table == "radroots_event_store_food_availability_cursor"
    ));
    assert_eq!(
        rebuild_authority_snapshot(&foreign_key_store).await,
        foreign_key_authority_before
    );
    assert_eq!(
        query_string_rows(
            foreign_key_store.pool(),
            "SELECT printf('%d|%d|%d', id, caller_parent_id, managed_singleton) FROM caller_fk_child ORDER BY id",
        )
        .await,
        foreign_key_rows_before
    );
}

#[tokio::test]
async fn raw_source_rebuild_scoped_integrity_preserves_caller_state_v1() {
    let store = RadrootsEventStore::open_memory().await.expect("open");
    seed_food_fixture(&store).await;
    let (event_seq, event_id, pubkey, created_at): (i64, String, String, i64) = sqlx::query_as(
        "SELECT seq, event_id, pubkey, created_at FROM event_envelopes ORDER BY seq LIMIT 1",
    )
    .fetch_one(store.pool())
    .await
    .expect("raw source row");
    sqlx::query(
        "INSERT INTO event_transport_observation(event_id, transport_kind, endpoint_uri, endpoint_fingerprint, observation_type, first_observed_at_ms, last_observed_at_ms, observation_count, redacted_message) VALUES (?, 'nostr', 'wss://relay.example', 'caller-endpoint', 'received', 1, 2, 2, 'preserve')",
    )
    .bind(&event_id)
    .execute(store.pool())
    .await
    .expect("legacy transport observation");
    sqlx::query(
        "INSERT INTO listing_projection(listing_addr, listing_event_id, seller_pubkey, farm_pubkey, farm_d_tag, listing_d_tag, title, description, product_type, primary_bin_id, quantity_amount, quantity_unit, price_amount, price_currency, inventory_available, availability_status, delivery_method, locality_primary, locality_city, locality_region, locality_country, geohash5, listing_json, source_event_seq, created_at, updated_at_ms) VALUES ('caller-listing', ?, ?, ?, 'farm', 'listing', 'Carrots', 'Fresh carrots', 'vegetable', 'bin-1', '12', 'kg', '5.00', 'CAD', '12', 'available', 'pickup', 'Victoria, BC', 'Victoria', 'BC', 'CA', 'c28', '{}', ?, ?, 3)",
    )
    .bind(&event_id)
    .bind(&pubkey)
    .bind(&pubkey)
    .bind(event_seq)
    .bind(created_at)
    .execute(store.pool())
    .await
    .expect("legacy listing projection");
    sqlx::query(
        "INSERT INTO listing_search_fts(listing_addr, title, description, product_type, locality, seller_pubkey) VALUES ('caller-listing', 'Carrots', 'Fresh carrots', 'vegetable', 'Victoria, BC', ?)",
    )
    .bind(&pubkey)
    .execute(store.pool())
    .await
    .expect("legacy listing search row");
    sqlx::query(
        "INSERT INTO trade_mutation(mutation_id, trade_id, root_mutation_id, contract_id, mutation_kind, schema_version, candidate_id, proposal_mutation_id, target_claim_mutation_id, author_pubkey, counterparty_pubkey, buyer_pubkey, seller_pubkey, farm_id, authored_at_unix_s, canonical_payload_bytes, payload_sha256, first_event_seq, first_transport_event_id, inserted_at_ms) VALUES ('caller-mutation', 'caller-trade', NULL, 'radroots.trade.v1', 'proposal', 1, 'candidate-1', NULL, NULL, ?, ?, ?, ?, 'farm-1', ?, X'7B7D', ?, ?, ?, 4)",
    )
    .bind(&pubkey)
    .bind(&pubkey)
    .bind(&pubkey)
    .bind(&pubkey)
    .bind(created_at)
    .bind("a".repeat(64))
    .bind(event_seq)
    .bind(&event_id)
    .execute(store.pool())
    .await
    .expect("legacy trade mutation");
    sqlx::query(
        "INSERT INTO trade_transport_envelope(transport_event_id, mutation_id, trade_id, transport_kind, pubkey, created_at, event_seq, payload_sha256, observed_at_ms) VALUES (?, 'caller-mutation', 'caller-trade', 'nostr', ?, ?, ?, ?, 5)",
    )
    .bind(&event_id)
    .bind(&pubkey)
    .bind(created_at)
    .bind(event_seq)
    .bind("a".repeat(64))
    .execute(store.pool())
    .await
    .expect("legacy trade transport envelope");
    let mut connection = store.pool().acquire().await.expect("connection");
    sqlx::query("PRAGMA foreign_keys = OFF")
        .execute(&mut *connection)
        .await
        .expect("disable caller FK checks");
    sqlx::query("CREATE TABLE caller_parent(id INTEGER PRIMARY KEY)")
        .execute(&mut *connection)
        .await
        .expect("caller parent");
    sqlx::query(
        "CREATE TABLE caller_child(id INTEGER PRIMARY KEY AUTOINCREMENT, parent_id INTEGER NOT NULL REFERENCES caller_parent(id), note TEXT NOT NULL)",
    )
    .execute(&mut *connection)
    .await
    .expect("caller child");
    sqlx::query("CREATE INDEX caller_child_note_idx ON caller_child(note)")
        .execute(&mut *connection)
        .await
        .expect("caller index");
    sqlx::query("INSERT INTO caller_child(parent_id, note) VALUES (999, 'preserve')")
        .execute(&mut *connection)
        .await
        .expect("caller FK violation");
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&mut *connection)
        .await
        .expect("restore FK checks");
    drop(connection);
    let caller_before = query_string_rows(
        store.pool(),
        "SELECT printf('%d|%d|%s', id, parent_id, note) FROM caller_child ORDER BY id",
    )
    .await;
    let sequence_before = query_string_rows(
        store.pool(),
        "SELECT printf('%d|%s|%s', rowid, quote(name), quote(seq)) FROM sqlite_sequence WHERE name = 'caller_child'",
    )
    .await;
    let caller_index_before = query_string_rows(
        store.pool(),
        "SELECT sql FROM sqlite_schema WHERE type = 'index' AND name = 'caller_child_note_idx'",
    )
    .await;
    let legacy_before = vec![
        query_string_rows(
            store.pool(),
            "SELECT printf('%s|%s|%s|%s|%d|%d|%d|%s', event_id, transport_kind, endpoint_fingerprint, observation_type, first_observed_at_ms, last_observed_at_ms, observation_count, quote(redacted_message)) FROM event_transport_observation ORDER BY event_id, transport_kind, endpoint_fingerprint, observation_type",
        )
        .await,
        query_string_rows(
            store.pool(),
            "SELECT printf('%s|%s|%s|%s|%s|%s|%d|%d', listing_addr, listing_event_id, seller_pubkey, title, locality_primary, listing_json, source_event_seq, updated_at_ms) FROM listing_projection ORDER BY listing_addr",
        )
        .await,
        query_string_rows(
            store.pool(),
            "SELECT printf('%s|%s|%s|%s|%s|%s', listing_addr, title, description, product_type, locality, seller_pubkey) FROM listing_search_fts ORDER BY listing_addr",
        )
        .await,
        query_string_rows(
            store.pool(),
            "SELECT printf('%s|%s|%s|%s|%d|%s|%s|%d|%s', mutation_id, trade_id, contract_id, mutation_kind, schema_version, hex(canonical_payload_bytes), payload_sha256, first_event_seq, first_transport_event_id) FROM trade_mutation ORDER BY mutation_id",
        )
        .await,
        query_string_rows(
            store.pool(),
            "SELECT printf('%s|%s|%s|%s|%s|%d|%d|%s|%d', transport_event_id, mutation_id, trade_id, transport_kind, pubkey, created_at, event_seq, payload_sha256, observed_at_ms) FROM trade_transport_envelope ORDER BY transport_event_id",
        )
        .await,
    ];

    store
        .rebuild_from_raw_v1()
        .await
        .expect("scoped rebuild ignores caller violation");
    assert_eq!(
        query_string_rows(
            store.pool(),
            "SELECT printf('%d|%d|%s', id, parent_id, note) FROM caller_child ORDER BY id",
        )
        .await,
        caller_before
    );
    assert_eq!(
        query_string_rows(
            store.pool(),
            "SELECT printf('%d|%s|%s', rowid, quote(name), quote(seq)) FROM sqlite_sequence WHERE name = 'caller_child'",
        )
        .await,
        sequence_before
    );
    assert_eq!(
        query_string_rows(
            store.pool(),
            "SELECT sql FROM sqlite_schema WHERE type = 'index' AND name = 'caller_child_note_idx'",
        )
        .await,
        caller_index_before
    );
    let legacy_after = vec![
        query_string_rows(
            store.pool(),
            "SELECT printf('%s|%s|%s|%s|%d|%d|%d|%s', event_id, transport_kind, endpoint_fingerprint, observation_type, first_observed_at_ms, last_observed_at_ms, observation_count, quote(redacted_message)) FROM event_transport_observation ORDER BY event_id, transport_kind, endpoint_fingerprint, observation_type",
        )
        .await,
        query_string_rows(
            store.pool(),
            "SELECT printf('%s|%s|%s|%s|%s|%s|%d|%d', listing_addr, listing_event_id, seller_pubkey, title, locality_primary, listing_json, source_event_seq, updated_at_ms) FROM listing_projection ORDER BY listing_addr",
        )
        .await,
        query_string_rows(
            store.pool(),
            "SELECT printf('%s|%s|%s|%s|%s|%s', listing_addr, title, description, product_type, locality, seller_pubkey) FROM listing_search_fts ORDER BY listing_addr",
        )
        .await,
        query_string_rows(
            store.pool(),
            "SELECT printf('%s|%s|%s|%s|%d|%s|%s|%d|%s', mutation_id, trade_id, contract_id, mutation_kind, schema_version, hex(canonical_payload_bytes), payload_sha256, first_event_seq, first_transport_event_id) FROM trade_mutation ORDER BY mutation_id",
        )
        .await,
        query_string_rows(
            store.pool(),
            "SELECT printf('%s|%s|%s|%s|%s|%d|%d|%s|%d', transport_event_id, mutation_id, trade_id, transport_kind, pubkey, created_at, event_seq, payload_sha256, observed_at_ms) FROM trade_transport_envelope ORDER BY transport_event_id",
        )
        .await,
    ];
    assert_eq!(legacy_after, legacy_before);
    assert_eq!(
        sqlx::query("PRAGMA foreign_key_check('caller_child')")
            .fetch_all(store.pool())
            .await
            .expect("caller FK audit")
            .len(),
        1
    );
}

#[tokio::test]
async fn raw_source_rebuild_generation_exhaustion_precedes_entropy_and_mutation_v1() {
    let store = RadrootsEventStore::open_memory().await.expect("open");
    for generation in 1..RADROOTS_EVENT_STORE_RETAINED_SOURCE_GENERATION_LIMIT_V1 {
        rebuild_from_raw_v1_on_pool_for_test(
            store.pool(),
            &FixedGeneration(u8::try_from(generation).expect("test generation")),
            None,
        )
        .await
        .expect("fill retained generation history");
    }
    let before = rebuild_authority_snapshot(&store).await;
    assert!(matches!(
        rebuild_from_raw_v1_on_pool_for_test(store.pool(), &PanickingGeneration, None).await,
        Err(RadrootsEventStoreError::SourceGenerationHistoryLimitReached { current, limit })
            if current == limit && limit == RADROOTS_EVENT_STORE_RETAINED_SOURCE_GENERATION_LIMIT_V1
    ));
    assert_eq!(rebuild_authority_snapshot(&store).await, before);
}

#[tokio::test]
async fn raw_source_rebuild_entropy_failure_is_atomic_v1() {
    let store = RadrootsEventStore::open_memory().await.expect("open");
    seed_food_fixture(&store).await;
    let before = rebuild_authority_snapshot(&store).await;

    assert!(matches!(
        rebuild_from_raw_v1_on_pool_for_test(store.pool(), &FailingGeneration, None).await,
        Err(RadrootsEventStoreError::SourceGenerationEntropyUnavailable)
    ));
    assert_eq!(rebuild_authority_snapshot(&store).await, before);
}

#[tokio::test]
async fn raw_source_rebuild_empty_source_without_transitions_is_deterministic_v1() {
    let store = RadrootsEventStore::open_memory().await.expect("open");
    let first = rebuild_from_raw_v1_on_pool_for_test(store.pool(), &FixedGeneration(0x61), None)
        .await
        .expect("first empty rebuild");
    let second = rebuild_from_raw_v1_on_pool_for_test(store.pool(), &FixedGeneration(0x62), None)
        .await
        .expect("second empty rebuild");
    assert_eq!(first.raw_high_water_seq(), 0);
    assert_eq!(second.raw_high_water_seq(), 0);
    assert_eq!(first.immutable_raw_digest(), second.immutable_raw_digest());
    assert_eq!(
        first.active_product_state_digest(),
        second.active_product_state_digest()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM radroots_event_store_addressable_head_transition",
        )
        .fetch_one(store.pool())
        .await
        .expect("transition count"),
        0
    );
    let sequence = sqlx::query_as::<_, (i64, String, i64)>(
        "SELECT rowid, name, seq FROM sqlite_sequence ORDER BY rowid LIMIT 1",
    )
    .fetch_one(store.pool())
    .await
    .expect("transition sequence");
    assert_eq!(sequence.1, TRANSITION_SEQUENCE_NAME);
    assert_eq!(sequence.2, 0);
}

#[tokio::test]
async fn raw_source_repair_preflights_reject_bounded_authority_drift_v1() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let missing_canonical_path = tempdir.path().join("missing-canonical.sqlite");
    let missing_canonical_filename = missing_canonical_path.display().to_string();
    assert!(matches!(
        super::canonical_raw_source_repair_main_path_v1(&missing_canonical_path),
        Err(RadrootsEventStoreError::RawSourceRepairMainDatabaseCanonicalizationFailed {
            filename,
            source,
        }) if filename == missing_canonical_filename
            && source.kind() == std::io::ErrorKind::NotFound
    ));

    let catalog_path = tempdir.path().join("repair-catalog-drift.sqlite");
    let catalog_store = RadrootsEventStore::open_file(&catalog_path)
        .await
        .expect("open catalog fixture");
    catalog_store.pool().close().await;
    let mut catalog_connection =
        SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(&catalog_path))
            .await
            .expect("catalog drift connection");
    sqlx::query("CREATE TABLE radroots_event_store_future_authority(value TEXT NOT NULL)")
        .execute(&mut catalog_connection)
        .await
        .expect("add one reserved catalog object beyond managed authority");
    catalog_connection
        .close()
        .await
        .expect("close catalog drift connection");
    assert!(matches!(
        RadrootsEventStore::repair_file_from_raw_v1(&catalog_path).await,
        Err(RadrootsEventStoreError::SchemaFingerprintMismatch { version: 4, .. })
    ));
    let mut catalog_verifier =
        SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(&catalog_path))
            .await
            .expect("catalog verifier");
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM main.sqlite_schema WHERE type = 'table' AND name = 'radroots_event_store_future_authority'",
        )
        .fetch_one(&mut catalog_verifier)
        .await
        .expect("reserved catalog object count"),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM radroots_event_store_source_generation",
        )
        .fetch_one(&mut catalog_verifier)
        .await
        .expect("catalog rejection generation count"),
        1
    );
    catalog_verifier
        .close()
        .await
        .expect("close catalog verifier");

    let history_path = tempdir.path().join("repair-history-drift.sqlite");
    let history_store = RadrootsEventStore::open_file(&history_path)
        .await
        .expect("open history fixture");
    history_store.pool().close().await;
    let mut history_connection =
        SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(&history_path))
            .await
            .expect("history drift connection");
    sqlx::query(
        "INSERT INTO radroots_event_store_schema_migrations(version, name, up_sha256, down_sha256, schema_sha256) VALUES (5, 'future_migration', ?, ?, ?)",
    )
    .bind("0".repeat(64))
    .bind("1".repeat(64))
    .bind("2".repeat(64))
    .execute(&mut history_connection)
    .await
    .expect("add the bounded fifth history row");
    history_connection
        .close()
        .await
        .expect("close history drift connection");
    assert!(matches!(
        RadrootsEventStore::repair_file_from_raw_v1(&history_path).await,
        Err(RadrootsEventStoreError::SchemaTooNew {
            current: 4,
            database: 5,
        })
    ));
    let mut history_verifier =
        SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(&history_path))
            .await
            .expect("history verifier");
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM radroots_event_store_schema_migrations",
        )
        .fetch_one(&mut history_verifier)
        .await
        .expect("history row count"),
        5
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM radroots_event_store_source_generation",
        )
        .fetch_one(&mut history_verifier)
        .await
        .expect("history rejection generation count"),
        1
    );
    history_verifier
        .close()
        .await
        .expect("close history verifier");

    let encoding_path = tempdir.path().join("repair-utf16.sqlite");
    let mut encoding_connection = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(&encoding_path)
            .create_if_missing(true),
    )
    .await
    .expect("UTF-16 fixture connection");
    sqlx::query("PRAGMA main.encoding = 'UTF-16le'")
        .execute(&mut encoding_connection)
        .await
        .expect("set UTF-16LE encoding");
    sqlx::query("CREATE TABLE encoding_anchor(value TEXT NOT NULL)")
        .execute(&mut encoding_connection)
        .await
        .expect("materialize UTF-16LE database");
    sqlx::query("DROP TABLE encoding_anchor")
        .execute(&mut encoding_connection)
        .await
        .expect("restore empty UTF-16LE catalog");
    encoding_connection
        .close()
        .await
        .expect("close UTF-16 fixture");
    assert!(matches!(
        RadrootsEventStore::repair_file_from_raw_v1(&encoding_path).await,
        Err(RadrootsEventStoreError::SqliteMainDatabaseEncodingNotUtf8 { actual })
            if actual == "UTF-16le"
    ));
    let mut encoding_verifier =
        SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(&encoding_path))
            .await
            .expect("UTF-16 verifier");
    assert_eq!(
        sqlx::query_scalar::<_, String>("PRAGMA main.encoding")
            .fetch_one(&mut encoding_verifier)
            .await
            .expect("UTF-16 encoding after rejection"),
        "UTF-16le"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("PRAGMA main.journal_mode")
            .fetch_one(&mut encoding_verifier)
            .await
            .expect("UTF-16 journal mode after rejection"),
        "delete"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM main.sqlite_schema WHERE name = 'radroots_event_store_schema_migrations' OR name = 'event_envelopes' OR name LIKE 'radroots_event_store_%'",
        )
        .fetch_one(&mut encoding_verifier)
        .await
        .expect("UTF-16 event-store catalog after rejection"),
        0
    );
    encoding_verifier
        .close()
        .await
        .expect("close UTF-16 verifier");
}

#[tokio::test]
async fn raw_source_rebuild_cold_file_repair_v1() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let missing = tempdir.path().join("missing.sqlite");
    assert!(matches!(
        RadrootsEventStore::repair_file_from_raw_v1(&missing).await,
        Err(RadrootsEventStoreError::RawSourceRepairMainDatabaseCanonicalizationFailed {
            source,
            ..
        }) if source.kind() == std::io::ErrorKind::NotFound
    ));
    assert!(
        !missing.exists(),
        "cold repair must not create a missing file"
    );

    let unmanaged = tempdir.path().join("unmanaged.sqlite");
    let mut unmanaged_connection = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(&unmanaged)
            .create_if_missing(true),
    )
    .await
    .expect("unmanaged connection");
    sqlx::query("CREATE TABLE caller_only(value TEXT NOT NULL)")
        .execute(&mut unmanaged_connection)
        .await
        .expect("unmanaged caller table");
    assert_eq!(
        sqlx::query_scalar::<_, String>("PRAGMA main.journal_mode = DELETE")
            .fetch_one(&mut unmanaged_connection)
            .await
            .expect("set unmanaged journal mode"),
        "delete"
    );
    unmanaged_connection.close().await.expect("close unmanaged");
    assert!(matches!(
        RadrootsEventStore::repair_file_from_raw_v1(&unmanaged).await,
        Err(RadrootsEventStoreError::RawSourceRebuildStateDrift {
            kind: RadrootsEventStoreRawSourceRebuildDriftV1::ManagedSchemaAuthority,
            ..
        })
    ));
    let mut unmanaged_verifier =
        SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(&unmanaged))
            .await
            .expect("unmanaged verifier");
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM sqlite_schema WHERE name = 'radroots_event_store_schema_migrations'",
        )
        .fetch_one(&mut unmanaged_verifier)
        .await
        .expect("unmanaged ledger absence"),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM sqlite_schema WHERE type = 'table' AND name = 'caller_only'",
        )
        .fetch_one(&mut unmanaged_verifier)
        .await
        .expect("caller-owned table preservation"),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("PRAGMA main.journal_mode")
            .fetch_one(&mut unmanaged_verifier)
            .await
            .expect("unmanaged journal mode after rejected repair"),
        "delete",
        "rejected cold repair must not persistently configure WAL"
    );
    unmanaged_verifier.close().await.expect("close verifier");

    let v3_path = tempdir.path().join("managed-v3.sqlite");
    let v3_store = RadrootsEventStore::open_file(&v3_path)
        .await
        .expect("open v3 fixture");
    rollback_event_store_schema_offline_destructive_for_migration_test(v3_store.pool(), 3)
        .await
        .expect("rollback to v3");
    v3_store.pool().close().await;
    assert!(matches!(
        RadrootsEventStore::repair_file_from_raw_v1(&v3_path).await,
        Err(RadrootsEventStoreError::RawSourceRebuildStateDrift {
            kind: RadrootsEventStoreRawSourceRebuildDriftV1::ManagedSchemaAuthority,
            ..
        })
    ));
    let mut v3_verifier =
        SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(&v3_path))
            .await
            .expect("v3 verifier");
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT MAX(version) FROM radroots_event_store_schema_migrations",
        )
        .fetch_one(&mut v3_verifier)
        .await
        .expect("v3 ledger"),
        3
    );
    v3_verifier.close().await.expect("close v3 verifier");

    let path = tempdir.path().join("cold-raw-repair.sqlite");
    let store = RadrootsEventStore::open_file(&path).await.expect("open");
    seed_food_fixture(&store).await;
    let pristine_product = logical_product_snapshot(&store).await;
    set_trigger_guarded_drift(
        &store,
        "radroots_event_store_event_envelopes_derived_update_guard",
        "UPDATE event_envelopes SET contract_status = 'invalid', contract_id = NULL, event_class = NULL, projection_eligible = 0",
    )
    .await;
    set_trigger_guarded_drift(
        &store,
        "radroots_event_store_food_availability_cursor_update_guard",
        "UPDATE radroots_event_store_food_availability_cursor SET hook_manifest_sha256 = '0000000000000000000000000000000000000000000000000000000000000000'",
    )
    .await;
    store.pool().close().await;
    let ordinary_open_error = match RadrootsEventStore::open_file(&path).await {
        Ok(_) => panic!("ordinary open must reject drifted derived authority"),
        Err(error) => error,
    };
    assert!(matches!(
        ordinary_open_error,
        RadrootsEventStoreError::FoodAvailabilityProjectionDrift { reason }
            if reason == "projection cursor identity is inconsistent"
    ));

    let (repaired, first_report) = RadrootsEventStore::repair_file_from_raw_v1(&path)
        .await
        .expect("cold file repair");
    assert_eq!(first_report.source_capacity().raw_event_count(), 1);
    assert_eq!(logical_product_snapshot(&repaired).await, pristine_product);

    set_trigger_guarded_drift(
        &repaired,
        "radroots_event_store_food_availability_cursor_update_guard",
        "UPDATE radroots_event_store_food_availability_cursor SET hook_manifest_sha256 = '0000000000000000000000000000000000000000000000000000000000000000'",
    )
    .await;
    repaired.pool().close().await;
    let (repaired_from_file, second_report) = RadrootsEventStore::repair_file_from_raw_v1(&path)
        .await
        .expect("second cold file repair");
    assert_eq!(
        first_report.immutable_raw_digest(),
        second_report.immutable_raw_digest()
    );
    assert_eq!(
        first_report.active_product_state_digest(),
        second_report.active_product_state_digest()
    );
    assert_eq!(
        logical_product_snapshot(&repaired_from_file).await,
        pristine_product
    );
}

#[tokio::test]
async fn raw_source_repair_rejects_delete_mode_exact_v4_without_mutation_v1() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let path = tempdir.path().join("repair-delete-mode.sqlite");
    let store = RadrootsEventStore::open_file(&path)
        .await
        .expect("open exact-v4 fixture");
    seed_food_fixture(&store).await;
    store.pool().close().await;

    let mut connection =
        SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(&path))
            .await
            .expect("DELETE-mode connection");
    assert_eq!(
        sqlx::query_scalar::<_, String>("PRAGMA main.journal_mode = DELETE")
            .fetch_one(&mut connection)
            .await
            .expect("set DELETE mode"),
        "delete"
    );
    connection.close().await.expect("close DELETE-mode fixture");
    let (before, before_journal_mode) = cold_file_authority_snapshot(&path).await;
    assert_eq!(before_journal_mode, "delete");

    assert!(matches!(
        RadrootsEventStore::repair_file_from_raw_v1(&path).await,
        Err(RadrootsEventStoreError::SqliteFileJournalModeNotWal { actual })
            if actual == "delete"
    ));

    let (after, after_journal_mode) = cold_file_authority_snapshot(&path).await;
    assert_eq!(after, before);
    assert_eq!(after_journal_mode, "delete");
}

#[tokio::test]
async fn raw_source_repair_rejects_canonical_path_lock_domain_mismatch_v1() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let primary_path = tempdir.path().join("repair-primary.sqlite");
    let candidate_path = tempdir.path().join("repair-candidate.sqlite");
    let primary_store = RadrootsEventStore::open_file(&primary_path)
        .await
        .expect("open primary fixture");
    seed_food_fixture(&primary_store).await;
    primary_store.pool().close().await;
    let candidate_store = RadrootsEventStore::open_file(&candidate_path)
        .await
        .expect("open candidate fixture");
    candidate_store.pool().close().await;
    let primary_before = cold_file_authority_snapshot(&primary_path).await;
    let candidate_before = cold_file_authority_snapshot(&candidate_path).await;
    let canonical_candidate =
        std::fs::canonicalize(&candidate_path).expect("canonical candidate path");

    let mut primary = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(&primary_path)
            .create_if_missing(false),
    )
    .await
    .expect("open primary connection");
    let transaction = primary
        .begin_with("BEGIN IMMEDIATE")
        .await
        .expect("hold primary write domain");
    assert!(matches!(
        super::validate_raw_source_repair_canonical_lock_domain_v1(&canonical_candidate).await,
        Err(RadrootsEventStoreError::RawSourceRepairCanonicalPathLockDomainMismatch {
            canonical_path,
        }) if canonical_path == canonical_candidate.display().to_string()
    ));
    transaction
        .rollback()
        .await
        .expect("rollback primary write domain");
    primary.close().await.expect("close primary connection");
    assert_eq!(
        cold_file_authority_snapshot(&primary_path).await,
        primary_before
    );
    assert_eq!(
        cold_file_authority_snapshot(&candidate_path).await,
        candidate_before
    );
}

#[tokio::test]
async fn raw_source_repair_post_preflight_failures_preserve_wal_and_state_v1() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    for (case, trigger, mutation) in [
        (
            "raw",
            "radroots_event_store_event_envelopes_raw_update_guard",
            "UPDATE event_envelopes SET content = 'Parsnip available this week.' WHERE content = 'Carrots available this week.'",
        ),
        (
            "source",
            "radroots_event_store_source_capacity_update_guard",
            "UPDATE radroots_event_store_source_capacity_v1 SET raw_event_count = raw_event_count + 1",
        ),
    ] {
        let path = tempdir.path().join(format!("repair-{case}-drift.sqlite"));
        let store = RadrootsEventStore::open_file(&path)
            .await
            .expect("open drift fixture");
        seed_food_fixture(&store).await;
        set_trigger_guarded_drift(&store, trigger, mutation).await;
        let before = rebuild_authority_snapshot(&store).await;
        store.pool().close().await;

        let error = match RadrootsEventStore::repair_file_from_raw_v1(&path).await {
            Ok(_) => panic!("post-preflight authority drift must fail"),
            Err(error) => error,
        };
        match case {
            "raw" => assert!(matches!(
                error,
                RadrootsEventStoreError::RawEventReconciliationMismatch {
                    field: "content",
                    ..
                }
            )),
            "source" => assert!(matches!(
                error,
                RadrootsEventStoreError::SourceCapacityStateDrift { reason }
                    if reason
                        == "capacity seal does not match active source state and generation history"
            )),
            _ => unreachable!(),
        }

        let (after, journal_mode) = cold_file_authority_snapshot(&path).await;
        assert_eq!(after, before, "{case} drift");
        assert_eq!(journal_mode, "wal", "{case} drift");
    }
}

#[tokio::test]
async fn raw_source_rebuild_repairs_active_transition_high_water_metadata_drift_v1() {
    assert_nonempty_transition_high_water_drift_is_repaired(
        "UPDATE radroots_event_store_source_state SET last_transition_seq = 0 WHERE singleton = 1",
        0x81,
        0x82,
        0x83,
    )
    .await;
    assert_nonempty_transition_high_water_drift_is_repaired(
        "UPDATE radroots_event_store_source_state SET last_transition_seq = (SELECT COALESCE(MAX(transition_seq), 0) + 7 FROM radroots_event_store_addressable_head_transition) WHERE singleton = 1",
        0x84,
        0x85,
        0x86,
    )
    .await;
}

#[tokio::test]
async fn raw_source_rebuild_repairs_empty_transition_high_water_metadata_drift_v1() {
    let store = RadrootsEventStore::open_memory().await.expect("open");
    let pristine = rebuild_from_raw_v1_on_pool_for_test(store.pool(), &FixedGeneration(0x87), None)
        .await
        .expect("establish empty pristine rebuild");
    let pristine_product = logical_product_snapshot(&store).await;
    assert_eq!(transition_high_water(&store).await, 0);
    set_trigger_guarded_drift(
        &store,
        "radroots_event_store_source_state_authority_update_guard",
        "UPDATE radroots_event_store_source_state SET last_transition_seq = 7 WHERE singleton = 1",
    )
    .await;

    let repaired = rebuild_from_raw_v1_on_pool_for_test(store.pool(), &FixedGeneration(0x88), None)
        .await
        .expect("repair empty transition high-water drift");
    assert_eq!(
        repaired.immutable_raw_digest(),
        pristine.immutable_raw_digest()
    );
    assert_eq!(
        repaired.active_product_state_digest(),
        pristine.active_product_state_digest()
    );
    assert_eq!(logical_product_snapshot(&store).await, pristine_product);
    let repaired_generation = repaired.new_source_generation();
    let repaired_floor: i64 = sqlx::query_scalar(
        "SELECT transition_floor_seq FROM radroots_event_store_source_generation WHERE source_generation = ?",
    )
    .bind(repaired_generation.as_bytes().as_slice())
    .fetch_one(store.pool())
    .await
    .expect("empty repaired generation transition floor");
    assert_eq!(repaired_floor, 0);
    assert_eq!(transition_high_water(&store).await, 0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT last_transition_seq FROM radroots_event_store_source_state WHERE singleton = 1",
        )
        .fetch_one(store.pool())
        .await
        .expect("empty repaired source-state high-water"),
        0
    );

    store
        .migrate_to_current_schema()
        .await
        .expect("ordinary reopen validation after empty repair");
    let repeated = rebuild_from_raw_v1_on_pool_for_test(store.pool(), &FixedGeneration(0x89), None)
        .await
        .expect("repeat empty rebuild after repair");
    assert_eq!(
        repeated.immutable_raw_digest(),
        repaired.immutable_raw_digest()
    );
    assert_eq!(
        repeated.active_product_state_digest(),
        repaired.active_product_state_digest()
    );
}

#[tokio::test]
async fn raw_source_rebuild_refuses_transition_history_gap_atomically_v1() {
    let store = RadrootsEventStore::open_memory().await.expect("open");
    seed_fixture_case(
        &store,
        FOOD_FIXTURE,
        "post_cutoff_replacement_restores_projection",
        "events",
    )
    .await;
    let transition_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM radroots_event_store_addressable_head_transition")
            .fetch_one(store.pool())
            .await
            .expect("transition count before gap");
    assert!(transition_count >= 2);
    set_trigger_guarded_drift(
        &store,
        "radroots_event_store_addressable_transition_delete_guard",
        "DELETE FROM radroots_event_store_addressable_head_transition WHERE transition_seq = (SELECT MIN(transition_seq) FROM radroots_event_store_addressable_head_transition)",
    )
    .await;
    let before = rebuild_authority_snapshot(&store).await;

    assert!(matches!(
        rebuild_from_raw_v1_on_pool_for_test(store.pool(), &FixedGeneration(0x8a), None).await,
        Err(RadrootsEventStoreError::RawSourceRebuildStateDrift {
            kind: RadrootsEventStoreRawSourceRebuildDriftV1::AddressableTransitionAuthority,
            detail,
        }) if detail.contains("lineage row") || detail.contains("gaps or foreign rows")
    ));
    assert_eq!(rebuild_authority_snapshot(&store).await, before);
}

#[tokio::test]
async fn raw_source_rebuild_refuses_historical_generation_lineage_corruption_v1() {
    let store = RadrootsEventStore::open_memory().await.expect("open");
    seed_food_fixture(&store).await;
    rebuild_from_raw_v1_on_pool_for_test(store.pool(), &FixedGeneration(0x71), None)
        .await
        .expect("first rebuild");
    rebuild_from_raw_v1_on_pool_for_test(store.pool(), &FixedGeneration(0x72), None)
        .await
        .expect("second rebuild");
    set_trigger_guarded_drift(
        &store,
        "radroots_event_store_source_generation_update_guard",
        "UPDATE radroots_event_store_source_generation SET baseline_raw_event_count = 0 WHERE generation_ordinal = 2",
    )
    .await;
    let before = rebuild_authority_snapshot(&store).await;
    assert!(matches!(
        rebuild_from_raw_v1_on_pool_for_test(store.pool(), &FixedGeneration(0x73), None).await,
        Err(RadrootsEventStoreError::RawSourceRebuildStateDrift {
            kind: RadrootsEventStoreRawSourceRebuildDriftV1::SourceGenerationLineage,
            detail,
        }) if detail.contains("lineage row 2")
    ));
    assert_eq!(rebuild_authority_snapshot(&store).await, before);

    let transition_store = RadrootsEventStore::open_memory().await.expect("open");
    seed_food_fixture(&transition_store).await;
    rebuild_from_raw_v1_on_pool_for_test(transition_store.pool(), &FixedGeneration(0x74), None)
        .await
        .expect("first rebuild");
    rebuild_from_raw_v1_on_pool_for_test(transition_store.pool(), &FixedGeneration(0x75), None)
        .await
        .expect("second rebuild");
    set_trigger_guarded_drift(
        &transition_store,
        "radroots_event_store_addressable_transition_update_guard",
        "UPDATE radroots_event_store_addressable_head_transition SET source_generation = (SELECT source_generation FROM radroots_event_store_source_generation WHERE generation_ordinal = 2) WHERE transition_seq = 1",
    )
    .await;
    let transition_before = rebuild_authority_snapshot(&transition_store).await;
    assert!(matches!(
        rebuild_from_raw_v1_on_pool_for_test(
            transition_store.pool(),
            &FixedGeneration(0x76),
            None,
        )
        .await,
        Err(RadrootsEventStoreError::RawSourceRebuildStateDrift {
            kind: RadrootsEventStoreRawSourceRebuildDriftV1::AddressableTransitionAuthority,
            detail,
        }) if detail.contains("lineage row 1")
    ));
    assert_eq!(
        rebuild_authority_snapshot(&transition_store).await,
        transition_before
    );
}
