#![forbid(unsafe_code)]

use super::{
    EventAdmission, FixedResultVectorGeneration, NIP09_EVENT_STORE_V1_UP_SQL,
    NIP09_RESULT_VECTOR_BYTES, NIP09_V1_UP_SQL, ReconciliationCapacityLimits,
    ReconciliationProfile, apply_reconciliation_hook, validate_applied_hook_state,
};
use crate::generated::nip09_reconciliation_manifest as nip09_manifest;
use crate::model::reconciliation_v1::{
    RadrootsEventAdmissionStatus, RadrootsEventIngest, StoredEventClass, tag_semantic_name,
    tag_value_type_name,
};
use radroots_event::envelope::kind::KIND_LIST_SET_FOLLOW;
use radroots_event::wire::v1::Nip01EventWire;
use sha2::{Digest, Sha256};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqliteConnection, SqlitePool};
use std::str::FromStr;

pub(super) const RESULT_VECTOR_EXECUTOR_ID: &str =
    "radroots_event_store.nip09_reconciliation_v1.result_vector_executor.v2";

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Nip09ReconciliationVector {
    schema_version: u32,
    hook_id: String,
    cases: Vec<Nip09ReconciliationVectorCase>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Nip09ReconciliationVectorCase {
    id: String,
    source_generation_hex: String,
    input_events: Vec<Nip09ReconciliationVectorInput>,
    expected: Nip09ReconciliationVectorExpected,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Nip09ReconciliationVectorInput {
    observed_at_ms: i64,
    event: Nip09ReconciliationVectorEvent,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Nip09ReconciliationVectorEvent {
    id: String,
    pubkey: String,
    created_at: u64,
    kind: u32,
    tags: Vec<Vec<String>>,
    content: String,
    sig: String,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Nip09ReconciliationVectorExpected {
    raw_event_count: i64,
    coordinate_count: i64,
    request_count: i64,
    event_target_count: i64,
    address_target_count: i64,
    transition_count: i64,
    state: Nip09ReconciliationVectorState,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Nip09ReconciliationVectorState {
    kind: i64,
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
    address_reference_cutoff: RequiredNullable<i64>,
}

#[derive(serde::Deserialize)]
#[serde(transparent)]
struct RequiredNullable<T>(Option<T>);

fn deserialize_required_nullable<'de, D, T>(
    deserializer: D,
) -> Result<RequiredNullable<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    <Option<T> as serde::Deserialize>::deserialize(deserializer).map(RequiredNullable)
}

#[tokio::test]
async fn nip09_reconciliation_v1_result_vector() {
    assert_eq!(
        RESULT_VECTOR_EXECUTOR_ID,
        "radroots_event_store.nip09_reconciliation_v1.result_vector_executor.v2"
    );
    assert_ne!(
        RESULT_VECTOR_EXECUTOR_ID,
        nip09_manifest::NIP09_RECONCILIATION_RESULT_VECTOR_EXECUTOR_ID,
        "the successor executor must remain distinct from the authenticated predecessor"
    );
    assert_eq!(
        sha256_hex(NIP09_RESULT_VECTOR_BYTES),
        nip09_manifest::NIP09_RECONCILIATION_RESULT_VECTOR_SHA256
    );
    let vector: Nip09ReconciliationVector = serde_json::from_slice(NIP09_RESULT_VECTOR_BYTES)
        .expect("strict NIP-09 reconciliation vector");
    assert_eq!(vector.schema_version, 1);
    assert_eq!(vector.hook_id, nip09_manifest::NIP09_RECONCILIATION_HOOK_ID);
    assert!(!vector.cases.is_empty());

    for case in vector.cases {
        execute_case(case).await;
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

async fn execute_case(case: Nip09ReconciliationVectorCase) {
    let pool = open_v1_store(case.id.as_str()).await;
    {
        let mut connection = pool
            .acquire()
            .await
            .unwrap_or_else(|error| panic!("{}: acquire v1 store: {error}", case.id));
        for input in &case.input_events {
            seed_v1_raw_event(&mut connection, case.id.as_str(), input).await;
        }
    }

    let generation: [u8; 32] = hex::decode(&case.source_generation_hex)
        .unwrap_or_else(|error| panic!("{}: source generation hex: {error}", case.id))
        .try_into()
        .unwrap_or_else(|bytes: Vec<u8>| {
            panic!("{}: source generation length {}", case.id, bytes.len())
        });
    apply_v1_reconciliation(&pool, case.id.as_str(), generation).await;
    assert_results(&pool, case.id.as_str(), generation, case.expected).await;
}

async fn open_v1_store(case_id: &str) -> SqlitePool {
    let options = SqliteConnectOptions::from_str("sqlite::memory:")
        .unwrap_or_else(|error| panic!("{case_id}: memory options: {error}"))
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .unwrap_or_else(|error| panic!("{case_id}: open v1 store: {error}"));
    sqlx::raw_sql(NIP09_EVENT_STORE_V1_UP_SQL)
        .execute(&pool)
        .await
        .unwrap_or_else(|error| panic!("{case_id}: install v1 schema: {error}"));
    pool
}

async fn seed_v1_raw_event(
    connection: &mut SqliteConnection,
    case_id: &str,
    input: &Nip09ReconciliationVectorInput,
) {
    let wire = Nip01EventWire {
        id: input.event.id.clone(),
        pubkey: input.event.pubkey.clone(),
        created_at: input.event.created_at,
        kind: input.event.kind,
        tags: input.event.tags.clone(),
        content: input.event.content.clone(),
        sig: input.event.sig.clone(),
        extra: Default::default(),
    };
    let raw_json = serde_json::to_string(&wire)
        .unwrap_or_else(|error| panic!("{case_id}: event JSON: {error}"));
    let ingest =
        RadrootsEventIngest::from_raw_json_reconciliation_v1(raw_json, input.observed_at_ms)
            .unwrap_or_else(|error| panic!("{case_id}: vector event verification: {error}"));
    let event = ingest.event();
    let admission = EventAdmission::for_profile(
        ReconciliationProfile::Nip09V1RegistryV7,
        ingest.verified_event(),
    )
    .unwrap_or_else(|error| panic!("{case_id}: registry-v7 admission: {error}"));
    if input.event.kind == KIND_LIST_SET_FOLLOW {
        assert_eq!(
            admission.status,
            RadrootsEventAdmissionStatus::Admitted,
            "{case_id}: {:?}",
            admission.code
        );
        assert_eq!(
            admission.contract.map(|contract| contract.id),
            Some("radroots.list_set.follow.v1"),
            "{case_id}"
        );
    }

    let tags = event.tags_as_vec();
    let tags_json = serde_json::to_string(&tags)
        .unwrap_or_else(|error| panic!("{case_id}: tags JSON: {error}"));
    let event_class = StoredEventClass::from_event_kind_class(event.kind_class());
    let valid_stream_eligible = admission.valid_stream_eligible(event.kind_class());
    let inserted = sqlx::query(
        "INSERT INTO event_envelopes(event_id, pubkey, created_at, kind, tags_json, content, sig, raw_json, verification_status, contract_status, contract_id, event_class, projection_eligible, inserted_at_ms, updated_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'verified', ?, ?, ?, ?, ?, ?)",
    )
    .bind(event.id_hex())
    .bind(event.author().to_hex())
    .bind(
        i64::try_from(event.created_at_u64())
            .unwrap_or_else(|_| panic!("{case_id}: created_at exceeds SQLite range")),
    )
    .bind(i64::from(event.kind_u32()))
    .bind(tags_json)
    .bind(event.content())
    .bind(event.signature_hex())
    .bind(ingest.raw_json())
    .bind(admission.status.as_str())
    .bind(admission.contract.map(|contract| contract.id))
    .bind(event_class.as_str())
    .bind(i64::from(valid_stream_eligible))
    .bind(input.observed_at_ms)
    .bind(input.observed_at_ms)
    .execute(&mut *connection)
    .await
    .unwrap_or_else(|error| panic!("{case_id}: seed raw event: {error}"));
    assert_eq!(inserted.rows_affected(), 1, "{case_id}: raw event insert");

    for (index, tag) in tags.iter().enumerate() {
        let tag_name = tag.first().map(String::as_str).unwrap_or("");
        let tag_value = tag.get(1).map(String::as_str);
        let tag_json = serde_json::to_string(tag)
            .unwrap_or_else(|error| panic!("{case_id}: tag JSON: {error}"));
        let contract_tag = admission.contract.and_then(|contract| {
            contract
                .tags
                .iter()
                .find(|candidate| candidate.name == tag_name)
        });
        let inserted = sqlx::query(
            "INSERT INTO event_envelope_tags(event_id, tag_index, tag_name, tag_value, tag_json, contract_semantic, contract_value_type, relay_indexed) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(event.id_hex())
        .bind(
            i64::try_from(index)
                .unwrap_or_else(|_| panic!("{case_id}: tag index exceeds SQLite range")),
        )
        .bind(tag_name)
        .bind(tag_value)
        .bind(tag_json)
        .bind(contract_tag.map(|tag| tag_semantic_name(tag.semantic)))
        .bind(contract_tag.map(|tag| tag_value_type_name(tag.value_type)))
        .bind(i64::from(
            contract_tag.map(|tag| tag.relay_indexed).unwrap_or(false),
        ))
        .execute(&mut *connection)
        .await
        .unwrap_or_else(|error| panic!("{case_id}: seed raw tag: {error}"));
        assert_eq!(inserted.rows_affected(), 1, "{case_id}: raw tag insert");
    }
}

async fn apply_v1_reconciliation(pool: &SqlitePool, case_id: &str, generation: [u8; 32]) {
    let mut transaction = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .unwrap_or_else(|error| panic!("{case_id}: reconciliation transaction: {error}"));
    sqlx::raw_sql(NIP09_V1_UP_SQL)
        .execute(&mut *transaction)
        .await
        .unwrap_or_else(|error| panic!("{case_id}: install NIP-09 v1 schema: {error}"));
    apply_reconciliation_hook(
        &mut transaction,
        &FixedResultVectorGeneration(generation),
        ReconciliationCapacityLimits::production(),
    )
    .await
    .unwrap_or_else(|error| panic!("{case_id}: NIP-09 v1 reconciliation: {error}"));
    validate_applied_hook_state(&mut transaction)
        .await
        .unwrap_or_else(|error| panic!("{case_id}: NIP-09 v1 deep validation: {error}"));
    transaction
        .commit()
        .await
        .unwrap_or_else(|error| panic!("{case_id}: reconciliation commit: {error}"));
}

async fn assert_results(
    pool: &SqlitePool,
    case_id: &str,
    generation: [u8; 32],
    expected: Nip09ReconciliationVectorExpected,
) {
    let stored_generation: Vec<u8> = sqlx::query_scalar(
        "SELECT active_generation FROM radroots_event_store_source_state WHERE singleton = 1",
    )
    .fetch_one(pool)
    .await
    .unwrap_or_else(|error| panic!("{case_id}: source generation: {error}"));
    assert_eq!(stored_generation.as_slice(), generation, "{case_id}");

    let source_raw_event_count: i64 = sqlx::query_scalar(
        "SELECT raw_event_count FROM radroots_event_store_source_state WHERE singleton = 1",
    )
    .fetch_one(pool)
    .await
    .unwrap_or_else(|error| panic!("{case_id}: source raw count: {error}"));
    let raw_event_count = row_count(pool, case_id, "event_envelopes").await;
    let coordinate_count = row_count(pool, case_id, "radroots_event_store_event_coordinate").await;
    let request_count = row_count(pool, case_id, "radroots_event_store_nip09_request").await;
    let event_target_count =
        row_count(pool, case_id, "radroots_event_store_nip09_event_target").await;
    let address_target_count =
        row_count(pool, case_id, "radroots_event_store_nip09_address_target").await;
    let transition_count = row_count(
        pool,
        case_id,
        "radroots_event_store_addressable_head_transition",
    )
    .await;
    assert_eq!(
        (
            source_raw_event_count,
            raw_event_count,
            coordinate_count,
            request_count,
            event_target_count,
            address_target_count,
            transition_count,
        ),
        (
            expected.raw_event_count,
            expected.raw_event_count,
            expected.coordinate_count,
            expected.request_count,
            expected.event_target_count,
            expected.address_target_count,
            expected.transition_count,
        ),
        "{case_id}"
    );

    let state = sqlx::query(
        "SELECT kind, pubkey, d_tag, raw_head_event_id, admission_status, contract_id, visibility, nip09_outcome, nip09_reason, event_reference_request_id, address_reference_request_id, address_reference_cutoff FROM radroots_event_store_addressable_head_state",
    )
    .fetch_one(pool)
    .await
    .unwrap_or_else(|error| panic!("{case_id}: state: {error}"));
    assert_eq!(
        state.try_get::<i64, _>("kind").expect("kind"),
        expected.state.kind,
        "{case_id}"
    );
    assert_eq!(
        state.try_get::<String, _>("pubkey").expect("pubkey"),
        expected.state.pubkey,
        "{case_id}"
    );
    assert_eq!(
        state.try_get::<String, _>("d_tag").expect("d_tag"),
        expected.state.d_tag,
        "{case_id}"
    );
    assert_eq!(
        state
            .try_get::<String, _>("raw_head_event_id")
            .expect("raw_head_event_id"),
        expected.state.raw_head_event_id,
        "{case_id}"
    );
    assert_eq!(
        state
            .try_get::<String, _>("admission_status")
            .expect("admission_status"),
        expected.state.admission_status,
        "{case_id}"
    );
    assert_eq!(
        state
            .try_get::<String, _>("contract_id")
            .expect("contract_id"),
        expected.state.contract_id,
        "{case_id}"
    );
    assert_eq!(
        state
            .try_get::<String, _>("visibility")
            .expect("visibility"),
        expected.state.visibility,
        "{case_id}"
    );
    assert_eq!(
        state
            .try_get::<String, _>("nip09_outcome")
            .expect("nip09_outcome"),
        expected.state.nip09_outcome,
        "{case_id}"
    );
    assert_eq!(
        state
            .try_get::<String, _>("nip09_reason")
            .expect("nip09_reason"),
        expected.state.nip09_reason,
        "{case_id}"
    );
    assert_eq!(
        state
            .try_get::<Option<String>, _>("event_reference_request_id")
            .expect("event_reference_request_id"),
        expected.state.event_reference_request_id.0,
        "{case_id}"
    );
    assert_eq!(
        state
            .try_get::<Option<String>, _>("address_reference_request_id")
            .expect("address_reference_request_id"),
        expected.state.address_reference_request_id.0,
        "{case_id}"
    );
    assert_eq!(
        state
            .try_get::<Option<i64>, _>("address_reference_cutoff")
            .expect("address_reference_cutoff"),
        expected.state.address_reference_cutoff.0,
        "{case_id}"
    );
    assert_contiguous_transition_sequence(pool, case_id).await;
}

async fn row_count(pool: &SqlitePool, case_id: &str, table: &'static str) -> i64 {
    let statement = format!("SELECT COUNT(*) FROM {table}");
    sqlx::query_scalar(sqlx::AssertSqlSafe(statement))
        .fetch_one(pool)
        .await
        .unwrap_or_else(|error| panic!("{case_id}: {table} count: {error}"))
}

async fn assert_contiguous_transition_sequence(pool: &SqlitePool, case_id: &str) {
    let sequences = sqlx::query_scalar::<_, i64>(
        "SELECT transition_seq FROM radroots_event_store_addressable_head_transition ORDER BY transition_seq",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_else(|error| panic!("{case_id}: transition sequence: {error}"));
    for (index, sequence) in sequences.iter().enumerate() {
        assert_eq!(
            *sequence,
            i64::try_from(index + 1).expect("sequence"),
            "{case_id}"
        );
    }
    let last_transition: i64 = sqlx::query_scalar(
        "SELECT last_transition_seq FROM radroots_event_store_source_state WHERE singleton = 1",
    )
    .fetch_one(pool)
    .await
    .unwrap_or_else(|error| panic!("{case_id}: last transition: {error}"));
    assert_eq!(
        last_transition,
        sequences.last().copied().unwrap_or_default(),
        "{case_id}"
    );
}

#[test]
fn nip09_reconciliation_v1_result_vector_requires_complete_state() {
    let vector: serde_json::Value =
        serde_json::from_slice(NIP09_RESULT_VECTOR_BYTES).expect("result-vector JSON");

    let mut missing_state = vector.clone();
    missing_state["cases"][0]["expected"]
        .as_object_mut()
        .expect("expected object")
        .remove("state");
    assert!(
        serde_json::from_value::<Nip09ReconciliationVector>(missing_state).is_err(),
        "missing durable state must fail strict executor parsing"
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
            serde_json::from_value::<Nip09ReconciliationVector>(missing_field).is_err(),
            "missing durable state field `{field}` must fail strict executor parsing"
        );
    }
}
