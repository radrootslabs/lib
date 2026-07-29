use super::protocol_storage_v1::{raw_head_snapshot_in_transaction, stored_raw_event_from_row};
use crate::error::RadrootsEventStoreError;
use crate::model::reconciliation_v1::{
    RadrootsEventAdmissionStatus, RadrootsEventIngest, RadrootsEventIngestReceipt,
    RadrootsEventPersistence, RadrootsEventStoreSourceGeneration, RadrootsRawHeadDecision,
    StoredEventClass, tag_semantic_name, tag_value_type_name,
};
use crate::nip09::reconciliation_v1::{
    EventAdmission, ReconciliationProfile, generation_from_blob,
    persist_event_coordinate_after_insert, synchronize_after_insert, validate_source_raw_authority,
};
use crate::source_maintenance_v1::{
    RawSourceCapacityDeltaV1, advance_source_capacity_after_insert_v1,
    preflight_unique_raw_source_append_v1, raw_source_capacity_delta_v1,
    validate_source_capacity_authority_fast_v1,
};
use radroots_event::contract::registry_v7::RadrootsEventContract;
use radroots_event::envelope::{RadrootsEventEnvelope, RadrootsEventKindClass};
use radroots_event::event_head::v1::{
    RadrootsCurrentEventHead, RadrootsEventHeadCandidate, RadrootsEventHeadCandidateResult,
    RadrootsEventHeadCoordinate, RadrootsEventHeadDecision,
    event_head_candidate_for_nip01_event_v1, select_event_head_v1,
};
use radroots_event::ids::RadrootsEventId;
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, Sqlite, SqliteConnection, Transaction};

pub(super) struct AppliedHead {
    pub(super) decision: RadrootsRawHeadDecision,
}

struct InsertRawEventResult {
    inserted: bool,
    seq: i64,
    admission_status: RadrootsEventAdmissionStatus,
    contract_id: Option<String>,
    valid_stream_eligible: bool,
}

pub(super) struct ProtocolReconciliationV1IngestResult {
    pub(super) receipt: RadrootsEventIngestReceipt,
    pub(super) inserted_seq: Option<i64>,
    pub(super) record_observation: bool,
    post_extension_authority_seal: ProtocolPostExtensionAuthoritySeal,
}

#[derive(Debug)]
struct ProtocolPostExtensionAuthoritySeal {
    source_generation: RadrootsEventStoreSourceGeneration,
    generation_ordinal: i64,
    reconciliation_version: i64,
    addressable_feed_version: i64,
    event_contract_registry_version: i64,
    hook_id: String,
    hook_manifest_sha256: String,
    transition_floor_seq: i64,
    baseline_raw_event_count: i64,
    baseline_raw_tag_count: i64,
    baseline_raw_high_water_seq: i64,
    raw_event_count: i64,
    raw_tag_count: i64,
    raw_event_bytes: u64,
    raw_tag_bytes: u64,
    raw_high_water_seq: i64,
    last_transition_seq: i64,
    retained_generation_count: u32,
    retained_generation_limit: u32,
    actual_raw_high_water_seq: i64,
    global_transition_min_seq: Option<i64>,
    global_transition_max_seq: Option<i64>,
    active_transition_min_seq: Option<i64>,
    active_transition_max_seq: Option<i64>,
    main_schema_version: i64,
    temp_schema_version: i64,
}

pub(super) async fn ingest_event_protocol_reconciliation_v1(
    tx: &mut Transaction<'_, Sqlite>,
    ingest: &RadrootsEventIngest,
) -> Result<ProtocolReconciliationV1IngestResult, RadrootsEventStoreError> {
    acquire_event_store_write_lock(tx).await?;
    let profile = validate_source_raw_authority(tx).await?;
    validate_source_capacity_authority_fast_v1(tx).await?;
    let event = ingest.event();
    let admission = EventAdmission::for_profile(profile, ingest.verified_event())?;
    let kind_class = event.kind_class();
    let valid_stream_eligible = admission.valid_stream_eligible(kind_class);
    if kind_class == RadrootsEventKindClass::Ephemeral {
        let post_extension_authority_seal = read_protocol_post_extension_authority_seal(tx).await?;
        return Ok(ProtocolReconciliationV1IngestResult {
            receipt: RadrootsEventIngestReceipt {
                persistence: RadrootsEventPersistence::NotPersisted,
                event_id: event.id_str().to_owned(),
                admission_status: admission.status,
                admission_code: admission.code,
                contract_id: admission.contract.map(|contract| contract.id.to_owned()),
                valid_stream_eligible: false,
                raw_head_decision: RadrootsRawHeadDecision::NotPersisted,
            },
            inserted_seq: None,
            record_observation: false,
            post_extension_authority_seal,
        });
    }
    let tags = event.tags_as_vec();
    let tags_json = serde_json::to_string(&tags)?;
    let event_id = event.id_str().to_owned();
    let existing_raw_event: i64 =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM event_envelopes WHERE event_id = ?)")
            .bind(event_id.as_str())
            .fetch_one(&mut **tx)
            .await?;
    let capacity_delta = if existing_raw_event == 0 {
        let delta = raw_source_capacity_delta_v1(ingest, tags_json.as_str())?;
        preflight_unique_raw_source_append_v1(tx, delta).await?;
        Some(delta)
    } else {
        None
    };
    let insert = insert_raw_event(
        tx,
        ingest,
        &admission,
        valid_stream_eligible,
        ingest.raw_json(),
        tags_json.as_str(),
    )
    .await?;
    let inserted = insert.inserted;
    if inserted {
        insert_tags(tx, event, admission.contract).await?;
        persist_event_coordinate_after_insert(tx, ingest, &admission, insert.seq).await?;
    }
    let raw_head_decision = apply_raw_event_head(tx, profile, event, ingest.observed_at_ms())
        .await?
        .decision;
    if inserted {
        let capacity_delta =
            require_unique_raw_event_capacity_delta(event_id.as_str(), capacity_delta)?;
        synchronize_after_insert(
            tx,
            ingest,
            &admission,
            insert.seq,
            event_id.as_str(),
            tags.len(),
            &raw_head_decision,
        )
        .await?;
        advance_source_capacity_after_insert_v1(tx, capacity_delta, insert.seq).await?;
    }

    let post_extension_authority_seal = read_protocol_post_extension_authority_seal(tx).await?;
    Ok(ProtocolReconciliationV1IngestResult {
        receipt: RadrootsEventIngestReceipt {
            persistence: if inserted {
                RadrootsEventPersistence::Inserted { seq: insert.seq }
            } else {
                RadrootsEventPersistence::Duplicate { seq: insert.seq }
            },
            event_id,
            admission_status: insert.admission_status,
            admission_code: admission.code,
            contract_id: insert.contract_id,
            valid_stream_eligible: insert.valid_stream_eligible,
            raw_head_decision,
        },
        inserted_seq: inserted.then_some(insert.seq),
        record_observation: true,
        post_extension_authority_seal,
    })
}

async fn read_protocol_post_extension_authority_seal(
    tx: &mut Transaction<'_, Sqlite>,
) -> Result<ProtocolPostExtensionAuthoritySeal, RadrootsEventStoreError> {
    let source_capacity = validate_source_capacity_authority_fast_v1(tx).await?;
    let actual_raw_high_water_seq: i64 =
        sqlx::query_scalar("SELECT seq FROM event_envelopes ORDER BY seq DESC LIMIT 1")
            .fetch_optional(&mut **tx)
            .await?
            .unwrap_or(0);
    let main_schema_version: i64 = sqlx::query_scalar("PRAGMA main.schema_version")
        .fetch_one(&mut **tx)
        .await?;
    let temp_schema_version: i64 = sqlx::query_scalar("PRAGMA temp.schema_version")
        .fetch_one(&mut **tx)
        .await?;

    let rebuild_marker_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM radroots_event_store_source_rebuild_marker")
            .fetch_one(&mut **tx)
            .await?;
    if rebuild_marker_count != 0 {
        return protocol_post_extension_drift(format!(
            "source rebuild marker residue is present: {rebuild_marker_count} row(s)"
        ));
    }

    let source_row = sqlx::query(
        "SELECT state.active_generation, state.raw_event_count, state.raw_tag_count, state.raw_high_water_seq, state.last_transition_seq, generation.generation_ordinal, generation.reconciliation_version, generation.addressable_feed_version, generation.event_contract_registry_version, generation.hook_id, generation.hook_manifest_sha256, generation.transition_floor_seq, generation.baseline_raw_event_count, generation.baseline_raw_tag_count, generation.baseline_raw_high_water_seq FROM radroots_event_store_source_state AS state JOIN radroots_event_store_source_generation AS generation ON generation.source_generation = state.active_generation WHERE state.singleton = 1",
    )
    .fetch_one(&mut **tx)
    .await?;
    let source_generation = generation_from_blob(source_row.try_get("active_generation")?)?;
    let raw_event_count: i64 = source_row.try_get("raw_event_count")?;
    let raw_tag_count: i64 = source_row.try_get("raw_tag_count")?;
    let raw_high_water_seq: i64 = source_row.try_get("raw_high_water_seq")?;
    let last_transition_seq: i64 = source_row.try_get("last_transition_seq")?;
    let transition_floor_seq: i64 = source_row.try_get("transition_floor_seq")?;

    if raw_high_water_seq != actual_raw_high_water_seq {
        return protocol_post_extension_drift(format!(
            "raw high-water does not match source authority: expected={raw_high_water_seq}, actual={actual_raw_high_water_seq}"
        ));
    }

    let global_transition_min_seq: Option<i64> = sqlx::query_scalar(
        "SELECT transition_seq FROM radroots_event_store_addressable_head_transition ORDER BY transition_seq ASC LIMIT 1",
    )
    .fetch_optional(&mut **tx)
    .await?;
    let global_transition_max_seq: Option<i64> = sqlx::query_scalar(
        "SELECT transition_seq FROM radroots_event_store_addressable_head_transition ORDER BY transition_seq DESC LIMIT 1",
    )
    .fetch_optional(&mut **tx)
    .await?;
    let expected_global_min = (last_transition_seq > 0).then_some(1);
    let expected_global_max = (last_transition_seq > 0).then_some(last_transition_seq);
    if (
        last_transition_seq < 0,
        global_transition_min_seq,
        global_transition_max_seq,
    ) != (false, expected_global_min, expected_global_max)
    {
        return protocol_post_extension_drift(format!(
            "global transition bounds disagree with source state: min={global_transition_min_seq:?}, max={global_transition_max_seq:?}, last={last_transition_seq}"
        ));
    }

    let active_transition_min_seq: Option<i64> = sqlx::query_scalar(
        "SELECT transition_seq FROM radroots_event_store_addressable_head_transition WHERE source_generation = ? ORDER BY transition_seq ASC LIMIT 1",
    )
    .bind(source_generation.as_bytes().as_slice())
    .fetch_optional(&mut **tx)
    .await?;
    let active_transition_max_seq: Option<i64> = sqlx::query_scalar(
        "SELECT transition_seq FROM radroots_event_store_addressable_head_transition WHERE source_generation = ? ORDER BY transition_seq DESC LIMIT 1",
    )
    .bind(source_generation.as_bytes().as_slice())
    .fetch_optional(&mut **tx)
    .await?;
    // Both persisted values are nonnegative SQLite INTEGERs, so their
    // difference remains representable even when the authority is corrupt.
    let active_transition_span = last_transition_seq - transition_floor_seq;
    let expected_active_min = if active_transition_span <= 0 {
        None
    } else {
        Some(transition_floor_seq + 1)
    };
    let expected_active_max = (active_transition_span > 0).then_some(last_transition_seq);
    if (
        active_transition_span < 0,
        active_transition_min_seq,
        active_transition_max_seq,
    ) != (false, expected_active_min, expected_active_max)
    {
        return protocol_post_extension_drift(format!(
            "active transition bounds disagree with source state: floor={transition_floor_seq}, last={last_transition_seq}, min={active_transition_min_seq:?}, max={active_transition_max_seq:?}"
        ));
    }

    Ok(ProtocolPostExtensionAuthoritySeal {
        source_generation,
        generation_ordinal: source_row.try_get("generation_ordinal")?,
        reconciliation_version: source_row.try_get("reconciliation_version")?,
        addressable_feed_version: source_row.try_get("addressable_feed_version")?,
        event_contract_registry_version: source_row.try_get("event_contract_registry_version")?,
        hook_id: source_row.try_get("hook_id")?,
        hook_manifest_sha256: source_row.try_get("hook_manifest_sha256")?,
        transition_floor_seq,
        baseline_raw_event_count: source_row.try_get("baseline_raw_event_count")?,
        baseline_raw_tag_count: source_row.try_get("baseline_raw_tag_count")?,
        baseline_raw_high_water_seq: source_row.try_get("baseline_raw_high_water_seq")?,
        raw_event_count,
        raw_tag_count,
        raw_event_bytes: source_capacity.raw_event_text_bytes(),
        raw_tag_bytes: source_capacity.raw_tag_text_bytes(),
        raw_high_water_seq,
        last_transition_seq,
        retained_generation_count: source_capacity.retained_generation_count(),
        retained_generation_limit: source_capacity.retained_generation_limit(),
        actual_raw_high_water_seq,
        global_transition_min_seq,
        global_transition_max_seq,
        active_transition_min_seq,
        active_transition_max_seq,
        main_schema_version,
        temp_schema_version,
    })
}

pub(super) async fn validate_protocol_post_extensions(
    tx: &mut Transaction<'_, Sqlite>,
    result: &ProtocolReconciliationV1IngestResult,
) -> Result<(), RadrootsEventStoreError> {
    let actual = read_protocol_post_extension_authority_seal(tx).await?;
    if !protocol_post_extension_authority_matches(&result.post_extension_authority_seal, &actual) {
        return protocol_post_extension_drift(format!(
            "post-core extensions changed protocol-owned authority: expected {:?}, found {actual:?}",
            result.post_extension_authority_seal
        ));
    }
    Ok(())
}

fn protocol_post_extension_authority_matches(
    expected: &ProtocolPostExtensionAuthoritySeal,
    actual: &ProtocolPostExtensionAuthoritySeal,
) -> bool {
    (
        (
            &expected.source_generation,
            expected.generation_ordinal,
            expected.reconciliation_version,
            expected.addressable_feed_version,
            expected.event_contract_registry_version,
            &expected.hook_id,
            &expected.hook_manifest_sha256,
            expected.transition_floor_seq,
            expected.baseline_raw_event_count,
        ),
        (
            expected.baseline_raw_tag_count,
            expected.baseline_raw_high_water_seq,
            expected.raw_event_count,
            expected.raw_tag_count,
            expected.raw_event_bytes,
            expected.raw_tag_bytes,
            expected.raw_high_water_seq,
            expected.last_transition_seq,
            expected.retained_generation_count,
        ),
        (
            expected.retained_generation_limit,
            expected.actual_raw_high_water_seq,
            expected.global_transition_min_seq,
            expected.global_transition_max_seq,
            expected.active_transition_min_seq,
            expected.active_transition_max_seq,
            expected.main_schema_version,
            expected.temp_schema_version,
        ),
    ) == (
        (
            &actual.source_generation,
            actual.generation_ordinal,
            actual.reconciliation_version,
            actual.addressable_feed_version,
            actual.event_contract_registry_version,
            &actual.hook_id,
            &actual.hook_manifest_sha256,
            actual.transition_floor_seq,
            actual.baseline_raw_event_count,
        ),
        (
            actual.baseline_raw_tag_count,
            actual.baseline_raw_high_water_seq,
            actual.raw_event_count,
            actual.raw_tag_count,
            actual.raw_event_bytes,
            actual.raw_tag_bytes,
            actual.raw_high_water_seq,
            actual.last_transition_seq,
            actual.retained_generation_count,
        ),
        (
            actual.retained_generation_limit,
            actual.actual_raw_high_water_seq,
            actual.global_transition_min_seq,
            actual.global_transition_max_seq,
            actual.active_transition_min_seq,
            actual.active_transition_max_seq,
            actual.main_schema_version,
            actual.temp_schema_version,
        ),
    )
}

fn protocol_post_extension_drift<T>(reason: String) -> Result<T, RadrootsEventStoreError> {
    Err(RadrootsEventStoreError::MigrationHookStateDrift {
        hook_id: "nip09_reconciliation_v1",
        reason,
    })
}

async fn acquire_event_store_write_lock(
    connection: &mut SqliteConnection,
) -> Result<(), RadrootsEventStoreError> {
    let acquired = sqlx::query(
        "UPDATE radroots_event_store_write_lock SET lock_version = lock_version WHERE singleton = 1",
    )
    .execute(connection)
    .await?;
    if acquired.rows_affected() != 1 {
        return Err(RadrootsEventStoreError::MigrationHookStateDrift {
            hook_id: "nip09_reconciliation_v1",
            reason: "event-store write lock authority is missing".to_owned(),
        });
    }
    Ok(())
}

async fn insert_raw_event(
    tx: &mut Transaction<'_, Sqlite>,
    ingest: &RadrootsEventIngest,
    admission: &EventAdmission,
    valid_stream_eligible: bool,
    raw_json: &str,
    tags_json: &str,
) -> Result<InsertRawEventResult, RadrootsEventStoreError> {
    let event = ingest.event();
    let contract_id = admission.contract.map(|contract| contract.id);
    let event_class = StoredEventClass::from_event_kind_class(event.kind_class()).as_str();
    let result = sqlx::query(
        "INSERT OR IGNORE INTO event_envelopes(event_id, pubkey, created_at, kind, tags_json, content, sig, raw_json, verification_status, contract_status, contract_id, event_class, projection_eligible, inserted_at_ms, updated_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(event.id_str())
    .bind(event.author().to_hex())
    .bind(i64_from_u64("created_at", event.created_at_u64())?)
    .bind(i64::from(event.kind_u32()))
    .bind(tags_json)
    .bind(event.content())
    .bind(event.sig_str())
    .bind(raw_json)
    .bind("verified")
    .bind(admission.status.as_str())
    .bind(contract_id)
    .bind(event_class)
    .bind(bool_i64(valid_stream_eligible))
    .bind(ingest.observed_at_ms())
    .bind(ingest.observed_at_ms())
    .execute(&mut **tx)
    .await?;
    let inserted = result.rows_affected() > 0;
    let seq = event_seq(tx, event.id_str()).await?;
    if inserted {
        return Ok(InsertRawEventResult {
            inserted: true,
            seq,
            admission_status: admission.status,
            contract_id: contract_id.map(str::to_owned),
            valid_stream_eligible,
        });
    }

    let existing = stored_raw_event_row_in_transaction(tx, event.id_str()).await?;
    let stored = stored_raw_event_from_row(existing)?;
    Ok(InsertRawEventResult {
        inserted: false,
        seq: stored.seq,
        admission_status: stored.admission_status,
        contract_id: stored.contract_id,
        valid_stream_eligible: stored.valid_stream_eligible,
    })
}

async fn stored_raw_event_row_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    event_id: &str,
) -> Result<SqliteRow, RadrootsEventStoreError> {
    sqlx::query(
        "SELECT seq, event_id, pubkey, created_at, kind, tags_json, content, sig, raw_json, verification_status, contract_status, contract_id, event_class, projection_eligible, inserted_at_ms, updated_at_ms FROM event_envelopes WHERE event_id = ?",
    )
    .bind(event_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(Into::into)
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn event_seq(
    tx: &mut Transaction<'_, Sqlite>,
    event_id: &str,
) -> Result<i64, RadrootsEventStoreError> {
    let row = sqlx::query("SELECT seq FROM event_envelopes WHERE event_id = ?")
        .bind(event_id)
        .fetch_one(&mut **tx)
        .await?;
    row.try_get("seq").map_err(Into::into)
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn insert_tags(
    tx: &mut Transaction<'_, Sqlite>,
    event: &RadrootsEventEnvelope,
    contract: Option<&'static RadrootsEventContract>,
) -> Result<(), RadrootsEventStoreError> {
    for (index, tag) in event.tag_slices().iter().enumerate() {
        let tag_values = tag.as_slice();
        let tag_name = tag_values.first().map(String::as_str).unwrap_or("");
        let tag_value = tag_values.get(1).map(String::as_str);
        let tag_json = serde_json::to_string(tag_values)?;
        let tag_contract = contract.and_then(|contract| {
            contract
                .tags
                .iter()
                .find(|candidate| candidate.name == tag_name)
        });
        let contract_semantic = tag_contract.map(|tag| tag_semantic_name(tag.semantic));
        let contract_value_type = tag_contract.map(|tag| tag_value_type_name(tag.value_type));
        let relay_indexed = tag_contract.map(|tag| tag.relay_indexed).unwrap_or(false);
        sqlx::query(
            "INSERT INTO event_envelope_tags(event_id, tag_index, tag_name, tag_value, tag_json, contract_semantic, contract_value_type, relay_indexed) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(event.id_str())
        .bind(i64::try_from(index).map_err(|_| RadrootsEventStoreError::IntegerRange {
            field: "tag_index",
            value: i64::MAX,
        })?)
        .bind(tag_name)
        .bind(tag_value)
        .bind(tag_json.as_str())
        .bind(contract_semantic)
        .bind(contract_value_type)
        .bind(bool_i64(relay_indexed))
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

pub(super) async fn apply_raw_event_head(
    tx: &mut Transaction<'_, Sqlite>,
    profile: ReconciliationProfile,
    event: &RadrootsEventEnvelope,
    updated_at_ms: i64,
) -> Result<AppliedHead, RadrootsEventStoreError> {
    let candidate = match profile {
        ReconciliationProfile::Nip09V1RegistryV7 => event_head_candidate_for_nip01_event_v1(event),
    };
    let candidate = match raw_head_candidate(candidate) {
        Ok(candidate) => candidate,
        Err(decision) => return Ok(AppliedHead { decision }),
    };
    let current = current_event_head(tx, &candidate.coordinate).await?;
    let protocol_decision = match profile {
        ReconciliationProfile::Nip09V1RegistryV7 => {
            select_event_head_v1(candidate.clone(), current.as_ref())
        }
    };
    if let RadrootsEventHeadDecision::Applied(head) = &protocol_decision {
        upsert_head(tx, &candidate, head, updated_at_ms).await?;
    }
    Ok(AppliedHead {
        decision: RadrootsRawHeadDecision::from_protocol(&protocol_decision),
    })
}

fn raw_head_candidate(
    candidate: RadrootsEventHeadCandidateResult,
) -> Result<RadrootsEventHeadCandidate, RadrootsRawHeadDecision> {
    match candidate {
        RadrootsEventHeadCandidateResult::Candidate(candidate) => Ok(candidate),
        RadrootsEventHeadCandidateResult::NotHeadSelected => {
            Err(RadrootsRawHeadDecision::NotHeadSelected)
        }
        RadrootsEventHeadCandidateResult::NotPersisted => {
            Err(RadrootsRawHeadDecision::NotPersisted)
        }
        RadrootsEventHeadCandidateResult::Malformed(_) => {
            Err(RadrootsRawHeadDecision::MalformedCoordinate)
        }
    }
}

async fn current_event_head(
    tx: &mut Transaction<'_, Sqlite>,
    coordinate: &RadrootsEventHeadCoordinate,
) -> Result<Option<RadrootsCurrentEventHead>, RadrootsEventStoreError> {
    let snapshot = raw_head_snapshot_in_transaction(tx, coordinate).await?;
    snapshot
        .map(|snapshot| {
            Ok(RadrootsCurrentEventHead {
                coordinate: coordinate.clone(),
                event_id: RadrootsEventId::parse(snapshot.raw_head.event_id)?,
                created_at: snapshot.raw_head.created_at,
            })
        })
        .transpose()
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn upsert_head(
    tx: &mut Transaction<'_, Sqlite>,
    candidate: &RadrootsEventHeadCandidate,
    head: &RadrootsCurrentEventHead,
    updated_at_ms: i64,
) -> Result<(), RadrootsEventStoreError> {
    match &head.coordinate {
        RadrootsEventHeadCoordinate::Replaceable { kind, pubkey } => {
            sqlx::query(
                "DELETE FROM event_envelope_head WHERE coordinate_type = 'replaceable' AND kind = ? AND pubkey = ? AND d_tag IS NULL",
            )
            .bind(i64::from(*kind))
            .bind(pubkey.to_hex())
            .execute(&mut **tx)
            .await?;
            sqlx::query(
                "INSERT INTO event_envelope_head(coordinate_type, kind, pubkey, d_tag, event_id, created_at, updated_at_ms) VALUES ('replaceable', ?, ?, NULL, ?, ?, ?)",
            )
            .bind(i64::from(*kind))
            .bind(pubkey.to_hex())
            .bind(candidate.event_id.as_str())
            .bind(i64_from_u64("created_at", candidate.created_at)?)
            .bind(updated_at_ms)
            .execute(&mut **tx)
            .await?;
        }
        RadrootsEventHeadCoordinate::Addressable {
            kind,
            pubkey,
            d_tag,
        } => {
            sqlx::query(
                "DELETE FROM event_envelope_head WHERE coordinate_type = 'addressable' AND kind = ? AND pubkey = ? AND d_tag = ?",
            )
            .bind(i64::from(*kind))
            .bind(pubkey.to_hex())
            .bind(d_tag.as_str())
            .execute(&mut **tx)
            .await?;
            sqlx::query(
                "INSERT INTO event_envelope_head(coordinate_type, kind, pubkey, d_tag, event_id, created_at, updated_at_ms) VALUES ('addressable', ?, ?, ?, ?, ?, ?)",
            )
            .bind(i64::from(*kind))
            .bind(pubkey.to_hex())
            .bind(d_tag.as_str())
            .bind(candidate.event_id.as_str())
            .bind(i64_from_u64("created_at", candidate.created_at)?)
            .bind(updated_at_ms)
            .execute(&mut **tx)
            .await?;
        }
    }
    Ok(())
}

fn require_unique_raw_event_capacity_delta(
    event_id: &str,
    capacity_delta: Option<RawSourceCapacityDeltaV1>,
) -> Result<RawSourceCapacityDeltaV1, RadrootsEventStoreError> {
    capacity_delta.ok_or_else(|| RadrootsEventStoreError::SourceCapacityStateDrift {
        reason: format!("unique raw event `{event_id}` was inserted after duplicate preflight"),
    })
}

fn i64_from_u64(field: &'static str, value: u64) -> Result<i64, RadrootsEventStoreError> {
    match i64::try_from(value) {
        Ok(value) => Ok(value),
        Err(_) => Err(RadrootsEventStoreError::UnsignedIntegerRange { field, value }),
    }
}

const fn bool_i64(value: bool) -> i64 {
    if value { 1 } else { 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn protocol_seal_error_after_corruption(
        statements: &[&'static str],
    ) -> RadrootsEventStoreError {
        let store = crate::RadrootsEventStore::open_memory()
            .await
            .expect("open store");
        let mut transaction = store.pool().begin().await.expect("transaction");
        for statement in statements {
            sqlx::query(*statement)
                .execute(&mut *transaction)
                .await
                .expect("apply trusted corruption");
        }
        let error = read_protocol_post_extension_authority_seal(&mut transaction)
            .await
            .expect_err("protocol seal must reject corruption");
        transaction.rollback().await.expect("rollback corruption");
        error
    }

    fn authority_seal() -> ProtocolPostExtensionAuthoritySeal {
        ProtocolPostExtensionAuthoritySeal {
            source_generation: RadrootsEventStoreSourceGeneration::from_bytes([0x11; 32]),
            generation_ordinal: 1,
            reconciliation_version: 1,
            addressable_feed_version: 1,
            event_contract_registry_version: 7,
            hook_id: "nip09_reconciliation_v1".to_owned(),
            hook_manifest_sha256: "a".repeat(64),
            transition_floor_seq: 0,
            baseline_raw_event_count: 0,
            baseline_raw_tag_count: 0,
            baseline_raw_high_water_seq: 0,
            raw_event_count: 1,
            raw_tag_count: 2,
            raw_event_bytes: 3,
            raw_tag_bytes: 4,
            raw_high_water_seq: 5,
            last_transition_seq: 6,
            retained_generation_count: 1,
            retained_generation_limit: 8,
            actual_raw_high_water_seq: 5,
            global_transition_min_seq: Some(1),
            global_transition_max_seq: Some(6),
            active_transition_min_seq: Some(1),
            active_transition_max_seq: Some(6),
            main_schema_version: 7,
            temp_schema_version: 0,
        }
    }

    #[test]
    fn post_extension_authority_match_rejects_every_field_drift() {
        assert!(protocol_post_extension_authority_matches(
            &authority_seal(),
            &authority_seal()
        ));

        macro_rules! rejects {
            ($field:ident, $value:expr) => {{
                let expected = authority_seal();
                let mut actual = authority_seal();
                actual.$field = $value;
                assert!(
                    !protocol_post_extension_authority_matches(&expected, &actual),
                    "accepted drift in {}",
                    stringify!($field)
                );
            }};
        }

        rejects!(
            source_generation,
            RadrootsEventStoreSourceGeneration::from_bytes([0x22; 32])
        );
        rejects!(generation_ordinal, 2);
        rejects!(reconciliation_version, 2);
        rejects!(addressable_feed_version, 2);
        rejects!(event_contract_registry_version, 8);
        rejects!(hook_id, "other".to_owned());
        rejects!(hook_manifest_sha256, "b".repeat(64));
        rejects!(transition_floor_seq, 1);
        rejects!(baseline_raw_event_count, 1);
        rejects!(baseline_raw_tag_count, 1);
        rejects!(baseline_raw_high_water_seq, 1);
        rejects!(raw_event_count, 2);
        rejects!(raw_tag_count, 3);
        rejects!(raw_event_bytes, 4);
        rejects!(raw_tag_bytes, 5);
        rejects!(raw_high_water_seq, 6);
        rejects!(last_transition_seq, 7);
        rejects!(retained_generation_count, 2);
        rejects!(retained_generation_limit, 9);
        rejects!(actual_raw_high_water_seq, 6);
        rejects!(global_transition_min_seq, None);
        rejects!(global_transition_max_seq, None);
        rejects!(active_transition_min_seq, None);
        rejects!(active_transition_max_seq, None);
        rejects!(main_schema_version, 8);
        rejects!(temp_schema_version, 1);
    }

    #[test]
    fn protocol_scalar_helpers_fail_closed() {
        assert_eq!(bool_i64(false), 0);
        assert_eq!(bool_i64(true), 1);
        assert_eq!(i64_from_u64("fixture", 7).expect("i64"), 7);
        assert!(matches!(
            i64_from_u64("fixture", u64::MAX),
            Err(RadrootsEventStoreError::UnsignedIntegerRange {
                field: "fixture",
                value: u64::MAX,
            })
        ));
        assert!(matches!(
            protocol_post_extension_drift::<()>("fixture".to_owned()),
            Err(RadrootsEventStoreError::MigrationHookStateDrift {
                hook_id: "nip09_reconciliation_v1",
                reason,
            }) if reason == "fixture"
        ));
        assert_eq!(
            raw_head_candidate(RadrootsEventHeadCandidateResult::Malformed(
                radroots_event::event_head::v1::RadrootsEventHeadMalformed::MissingDTag,
            ))
            .expect_err("malformed coordinate"),
            RadrootsRawHeadDecision::MalformedCoordinate
        );
        assert!(matches!(
            require_unique_raw_event_capacity_delta("event-id", None),
            Err(RadrootsEventStoreError::SourceCapacityStateDrift { reason })
                if reason == "unique raw event `event-id` was inserted after duplicate preflight"
        ));
    }

    #[tokio::test]
    async fn protocol_authority_seal_rejects_isolated_storage_drift() {
        let marker = protocol_seal_error_after_corruption(&[
            "DROP TRIGGER radroots_event_store_source_rebuild_marker_insert_guard",
            "INSERT INTO radroots_event_store_source_rebuild_marker(singleton, barrier_key, target_generation, target_generation_ordinal, reconciliation_version, addressable_feed_version, event_contract_registry_version, hook_id, hook_manifest_sha256, transition_floor_seq, baseline_raw_event_count, baseline_raw_tag_count, baseline_raw_high_water_seq, prior_active_generation, prior_raw_event_count, prior_raw_tag_count, prior_raw_high_water_seq, prior_last_transition_seq) SELECT 1, 1, generation.source_generation, generation.generation_ordinal, generation.reconciliation_version, generation.addressable_feed_version, generation.event_contract_registry_version, generation.hook_id, generation.hook_manifest_sha256, generation.transition_floor_seq, state.raw_event_count, state.raw_tag_count, state.raw_high_water_seq, state.active_generation, state.raw_event_count, state.raw_tag_count, state.raw_high_water_seq, state.last_transition_seq FROM radroots_event_store_source_generation AS generation JOIN radroots_event_store_source_state AS state ON state.active_generation = generation.source_generation WHERE state.singleton = 1",
        ])
        .await;
        assert!(matches!(
            marker,
            RadrootsEventStoreError::MigrationHookStateDrift { reason, .. }
                if reason.contains("source rebuild marker residue")
        ));

        let raw_high_water = protocol_seal_error_after_corruption(&[
            "DROP TRIGGER radroots_event_store_source_capacity_update_guard",
            "DROP TRIGGER radroots_event_store_source_state_authority_update_guard",
            "UPDATE radroots_event_store_source_capacity_v1 SET raw_event_count = 1, raw_high_water_seq = 1 WHERE singleton = 1",
            "UPDATE radroots_event_store_source_state SET raw_event_count = 1, raw_high_water_seq = 1 WHERE singleton = 1",
        ])
        .await;
        assert!(matches!(
            raw_high_water,
            RadrootsEventStoreError::MigrationHookStateDrift { reason, .. }
                if reason.contains("raw high-water does not match source authority")
        ));

        let global_bounds = protocol_seal_error_after_corruption(&[
            "DROP TRIGGER radroots_event_store_source_state_authority_update_guard",
            "UPDATE radroots_event_store_source_state SET last_transition_seq = 1 WHERE singleton = 1",
        ])
        .await;
        assert!(matches!(
            global_bounds,
            RadrootsEventStoreError::MigrationHookStateDrift { reason, .. }
                if reason.contains("global transition bounds disagree")
        ));

        let active_bounds = protocol_seal_error_after_corruption(&[
            "DROP TRIGGER radroots_event_store_source_generation_update_guard",
            "UPDATE radroots_event_store_source_generation SET transition_floor_seq = 1 WHERE source_generation = (SELECT active_generation FROM radroots_event_store_source_state WHERE singleton = 1)",
        ])
        .await;
        assert!(matches!(
            active_bounds,
            RadrootsEventStoreError::MigrationHookStateDrift { reason, .. }
                if reason.contains("active transition bounds disagree")
        ));
    }

    #[tokio::test]
    async fn protocol_write_lock_requires_singleton_authority() {
        let store = crate::RadrootsEventStore::open_memory()
            .await
            .expect("open store");
        let mut transaction = store.pool().begin().await.expect("transaction");
        acquire_event_store_write_lock(&mut transaction)
            .await
            .expect("singleton write lock");
        transaction.rollback().await.expect("rollback");

        sqlx::query("DROP TRIGGER radroots_event_store_write_lock_delete_guard")
            .execute(store.pool())
            .await
            .expect("trusted write-lock guard removal");
        sqlx::query("DELETE FROM radroots_event_store_write_lock")
            .execute(store.pool())
            .await
            .expect("trusted write-lock corruption");

        let mut transaction = store.pool().begin().await.expect("transaction");
        assert!(matches!(
            acquire_event_store_write_lock(&mut transaction).await,
            Err(RadrootsEventStoreError::MigrationHookStateDrift {
                hook_id: "nip09_reconciliation_v1",
                reason,
            }) if reason == "event-store write lock authority is missing"
        ));
    }
}
