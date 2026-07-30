use super::current_visibility_v1::{parse_suppression_outcome, parse_suppression_reason};
use super::protocol_storage_v1::stored_raw_event_from_row;
use super::{RadrootsEventStore, u32_from_i64, u64_from_i64};
use crate::RadrootsEventStoreError;
use crate::model::{
    RADROOTS_ADDRESSABLE_TRANSITION_D_TAG_MAX_BYTES_V1,
    RADROOTS_ADDRESSABLE_TRANSITION_FEED_VERSION_V1,
    RADROOTS_ADDRESSABLE_TRANSITION_PAGE_LIMIT_MAX_V1,
    RADROOTS_ADDRESSABLE_TRANSITION_PAGE_RAW_JSON_MAX_BYTES_V1,
    RADROOTS_ADDRESSABLE_TRANSITION_PAGE_SCAN_MAX_V1, RadrootsAddressableTransitionCauseV1,
    RadrootsAddressableTransitionCoordinateV1, RadrootsAddressableTransitionCursorV1,
    RadrootsAddressableTransitionEventReferenceV1, RadrootsAddressableTransitionOriginV1,
    RadrootsAddressableTransitionPageV1, RadrootsAddressableTransitionRawHeadDecisionV1,
    RadrootsAddressableTransitionScopeV1, RadrootsAddressableTransitionV1,
    RadrootsAddressableTransitionVisibilityV1, RadrootsEventAdmissionStatus, RadrootsEventIngest,
    RadrootsEventStoreSourceGeneration, RadrootsNip09SuppressionEvidenceV1,
    RadrootsNip09SuppressionOutcome, RadrootsStoreProducedCanonicalEventV1, RadrootsStoredRawEvent,
    StoredEventClass,
};
use crate::nip09::reconciliation_v1::{
    EventAdmission, ReconciliationProfile, generation_from_blob,
};
use radroots_event::id::EventId;
use radroots_identity::PublicKey;
use sqlx::{QueryBuilder, Row, Sqlite, SqliteConnection};

impl RadrootsEventStore {
    pub async fn addressable_transition_page_v1(
        &self,
        scope: &RadrootsAddressableTransitionScopeV1,
        cursor: Option<&RadrootsAddressableTransitionCursorV1>,
        limit: u32,
    ) -> Result<RadrootsAddressableTransitionPageV1, RadrootsEventStoreError> {
        validate_limit(limit)?;
        let mut tx = self.pool.begin().await?;
        let page =
            addressable_transition_page_in_transaction_v1(&mut tx, scope, cursor, limit).await?;
        tx.commit().await?;
        Ok(page)
    }
}

pub(super) async fn addressable_transition_page_in_transaction_v1(
    connection: &mut SqliteConnection,
    scope: &RadrootsAddressableTransitionScopeV1,
    cursor: Option<&RadrootsAddressableTransitionCursorV1>,
    limit: u32,
) -> Result<RadrootsAddressableTransitionPageV1, RadrootsEventStoreError> {
    validate_limit(limit)?;
    let source = read_and_validate_source_authority(connection).await?;
    let start = validate_or_create_cursor(connection, scope, cursor, &source).await?;
    let fetch_limit = i64::from(RADROOTS_ADDRESSABLE_TRANSITION_PAGE_SCAN_MAX_V1) + 1;
    let mut query = QueryBuilder::<Sqlite>::new(
        "SELECT transition_seq, source_generation, origin, kind, pubkey, d_tag, raw_head_event_id, raw_head_event_seq, raw_head_created_at, visible_event_id, visible_event_seq, retracted_event_id, retracted_event_seq, admission_status, admission_code, contract_id, visibility, nip09_outcome, nip09_reason, event_reference_request_id, address_reference_request_id, address_reference_cutoff, cause_event_seq, cause_event_id, raw_head_decision FROM radroots_event_store_addressable_head_transition WHERE source_generation = ",
    );
    query.push_bind(source.generation.as_bytes().as_slice());
    query.push(" AND transition_seq > ");
    query.push_bind(start);
    query.push(" AND transition_seq <= ");
    query.push_bind(source.high_water);
    query.push(" ORDER BY transition_seq LIMIT ");
    query.push_bind(fetch_limit);

    let mut rows = query.build().fetch_all(&mut *connection).await?;
    let scan_limited = rows.len()
        > usize::try_from(RADROOTS_ADDRESSABLE_TRANSITION_PAGE_SCAN_MAX_V1)
            .expect("u32 fits usize");
    if scan_limited {
        rows.pop();
    }
    let mut transitions = Vec::with_capacity(rows.len());
    let mut canonical_payload_bytes = 0usize;
    let mut last_scanned_sequence = start;
    let mut stopped_before_row = false;
    for row in rows {
        let transition_seq: i64 = row.try_get("transition_seq").map_err(|error| {
            corruption(format!("transition sequence cannot be decoded: {error}"))
        })?;
        let expected_sequence = last_scanned_sequence
            .checked_add(1)
            .ok_or_else(|| corruption("transition sequence overflow"))?;
        if transition_seq != expected_sequence {
            return Err(RadrootsEventStoreError::AddressableTransitionSequenceGap {
                reason: format!(
                    "expected transition sequence {expected_sequence}, found {transition_seq}"
                ),
            });
        }
        let kind = u32_from_i64(
            "transition.kind",
            row.try_get("kind").map_err(|error| {
                corruption(format!("transition kind cannot be decoded: {error}"))
            })?,
        )
        .map_err(|error| corruption(error.to_string()))?;
        if !scope.kinds().contains(&kind) {
            last_scanned_sequence = transition_seq;
            continue;
        }
        if transitions.len() == usize::try_from(limit).expect("u32 fits usize") {
            stopped_before_row = true;
            break;
        }
        let transition = transition_from_row(connection, row, source.generation).await?;
        let transition_payload_bytes = transition
            .visible_event()
            .map_or(0, |event| event.raw_json().len());
        let next_payload_bytes = canonical_payload_bytes
            .checked_add(transition_payload_bytes)
            .ok_or(
                RadrootsEventStoreError::AddressableTransitionPagePayloadTooLarge {
                    max: RADROOTS_ADDRESSABLE_TRANSITION_PAGE_RAW_JSON_MAX_BYTES_V1,
                    actual: usize::MAX,
                },
            )?;
        if next_payload_bytes > RADROOTS_ADDRESSABLE_TRANSITION_PAGE_RAW_JSON_MAX_BYTES_V1 {
            if transitions.is_empty() {
                return Err(
                    RadrootsEventStoreError::AddressableTransitionPagePayloadTooLarge {
                        max: RADROOTS_ADDRESSABLE_TRANSITION_PAGE_RAW_JSON_MAX_BYTES_V1,
                        actual: next_payload_bytes,
                    },
                );
            }
            stopped_before_row = true;
            break;
        }
        canonical_payload_bytes = next_payload_bytes;
        last_scanned_sequence = transition_seq;
        transitions.push(transition);
    }
    if !scan_limited && !stopped_before_row && last_scanned_sequence < source.high_water {
        return Err(RadrootsEventStoreError::AddressableTransitionSequenceGap {
            reason: format!(
                "sealed interval ends at {}, but the last stored transition is {last_scanned_sequence}",
                source.high_water
            ),
        });
    }
    let has_more = last_scanned_sequence < source.high_water;
    Ok(RadrootsAddressableTransitionPageV1 {
        source_high_water: source.high_water,
        transitions,
        next_cursor: RadrootsAddressableTransitionCursorV1::new(
            source.generation,
            scope.fingerprint(),
            last_scanned_sequence,
        )?,
        has_more,
    })
}

fn validate_limit(limit: u32) -> Result<(), RadrootsEventStoreError> {
    if !(1..=RADROOTS_ADDRESSABLE_TRANSITION_PAGE_LIMIT_MAX_V1).contains(&limit) {
        return Err(RadrootsEventStoreError::QueryLimitOutOfRange {
            min: 1,
            max: RADROOTS_ADDRESSABLE_TRANSITION_PAGE_LIMIT_MAX_V1,
            actual: limit,
        });
    }
    Ok(())
}

struct FeedSourceAuthority {
    generation: RadrootsEventStoreSourceGeneration,
    feed_version: u32,
    floor: i64,
    high_water: i64,
}

async fn read_and_validate_source_authority(
    connection: &mut SqliteConnection,
) -> Result<FeedSourceAuthority, RadrootsEventStoreError> {
    let row = sqlx::query(
        "SELECT source.active_generation, source.last_transition_seq, generation.addressable_feed_version, generation.transition_floor_seq, integrity.transition_floor_seq AS sealed_floor_seq, integrity.last_transition_seq AS sealed_last_transition_seq, integrity.transition_count AS sealed_transition_count FROM radroots_event_store_source_state AS source JOIN radroots_event_store_source_generation AS generation ON generation.source_generation = source.active_generation JOIN radroots_event_store_addressable_feed_integrity_v1 AS integrity ON integrity.source_generation = source.active_generation WHERE source.singleton = 1",
    )
    .fetch_optional(&mut *connection)
    .await?
    .ok_or_else(|| corruption("active source authority is missing"))?;
    let authority = FeedSourceAuthority {
        generation: generation_from_blob(row.try_get("active_generation")?)
            .map_err(|error| corruption(format!("active generation is invalid: {error}")))?,
        feed_version: u32_from_i64(
            "addressable_feed_version",
            row.try_get("addressable_feed_version")?,
        )
        .map_err(|error| corruption(error.to_string()))?,
        floor: row.try_get("transition_floor_seq")?,
        high_water: row.try_get("last_transition_seq")?,
    };
    if authority.feed_version != RADROOTS_ADDRESSABLE_TRANSITION_FEED_VERSION_V1 {
        return Err(
            RadrootsEventStoreError::AddressableTransitionFeedVersionMismatch {
                expected: RADROOTS_ADDRESSABLE_TRANSITION_FEED_VERSION_V1,
                actual: authority.feed_version,
            },
        );
    }
    if authority.floor < 0 || authority.high_water < authority.floor {
        return Err(corruption(format!(
            "active transition interval has floor={} and high-water={}",
            authority.floor, authority.high_water
        )));
    }
    let expected_count = authority.high_water - authority.floor;
    let sealed_floor: i64 = row.try_get("sealed_floor_seq")?;
    let sealed_high_water: i64 = row.try_get("sealed_last_transition_seq")?;
    let sealed_count: i64 = row.try_get("sealed_transition_count")?;
    if sealed_floor != authority.floor
        || sealed_high_water != authority.high_water
        || sealed_count != expected_count
    {
        return Err(RadrootsEventStoreError::AddressableTransitionSequenceGap {
            reason: format!(
                "active interval floor={} high-water={} disagrees with seal floor={sealed_floor}, high-water={sealed_high_water}, count={sealed_count}",
                authority.floor, authority.high_water,
            ),
        });
    }
    if authority.high_water > authority.floor {
        let first_sequence = authority
            .floor
            .checked_add(1)
            .ok_or_else(|| corruption("active transition interval floor overflow"))?;
        let boundary_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM radroots_event_store_addressable_head_transition WHERE source_generation = ? AND transition_seq IN (?, ?)",
        )
        .bind(authority.generation.as_bytes().as_slice())
        .bind(first_sequence)
        .bind(authority.high_water)
        .fetch_one(&mut *connection)
        .await?;
        let expected_boundary_count = if first_sequence == authority.high_water {
            1
        } else {
            2
        };
        if boundary_count != expected_boundary_count {
            return Err(RadrootsEventStoreError::AddressableTransitionSequenceGap {
                reason: format!(
                    "sealed interval {}..={} is missing a boundary transition",
                    first_sequence, authority.high_water
                ),
            });
        }
    }
    Ok(authority)
}

async fn validate_or_create_cursor(
    connection: &mut SqliteConnection,
    scope: &RadrootsAddressableTransitionScopeV1,
    cursor: Option<&RadrootsAddressableTransitionCursorV1>,
    source: &FeedSourceAuthority,
) -> Result<i64, RadrootsEventStoreError> {
    let Some(cursor) = cursor else {
        return Ok(source.floor);
    };
    if cursor.feed_version() != RADROOTS_ADDRESSABLE_TRANSITION_FEED_VERSION_V1 {
        return Err(
            RadrootsEventStoreError::AddressableTransitionFeedVersionMismatch {
                expected: RADROOTS_ADDRESSABLE_TRANSITION_FEED_VERSION_V1,
                actual: cursor.feed_version(),
            },
        );
    }
    if cursor.scope_fingerprint() != scope.fingerprint() {
        return Err(RadrootsEventStoreError::AddressableTransitionScopeMismatch);
    }
    if cursor.source_generation() != source.generation {
        return Err(RadrootsEventStoreError::AddressableTransitionSourceGenerationMismatch);
    }
    if cursor.last_transition_seq() < source.floor {
        return Err(
            RadrootsEventStoreError::AddressableTransitionCursorExpired {
                cursor: cursor.last_transition_seq(),
                floor: source.floor,
            },
        );
    }
    if cursor.last_transition_seq() > source.high_water {
        return Err(RadrootsEventStoreError::AddressableTransitionCursorAhead {
            cursor: cursor.last_transition_seq(),
            high_water: source.high_water,
        });
    }
    if cursor.last_transition_seq() != source.floor
        && cursor.last_transition_seq() != source.high_water
    {
        let exists: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM radroots_event_store_addressable_head_transition WHERE source_generation = ? AND transition_seq = ?",
        )
        .bind(source.generation.as_bytes().as_slice())
        .bind(cursor.last_transition_seq())
        .fetch_one(&mut *connection)
        .await?;
        if exists != 1 {
            return Err(RadrootsEventStoreError::AddressableTransitionSequenceGap {
                reason: format!(
                    "cursor sequence {} is absent from the sealed active interval",
                    cursor.last_transition_seq()
                ),
            });
        }
    }
    Ok(cursor.last_transition_seq())
}

async fn transition_from_row(
    connection: &mut SqliteConnection,
    row: sqlx::sqlite::SqliteRow,
    expected_generation: RadrootsEventStoreSourceGeneration,
) -> Result<RadrootsAddressableTransitionV1, RadrootsEventStoreError> {
    let transition_seq: i64 = row.try_get("transition_seq")?;
    if transition_seq <= 0 {
        return Err(corruption(format!(
            "transition sequence {transition_seq} is not positive"
        )));
    }
    let source_generation = generation_from_blob(row.try_get("source_generation")?)
        .map_err(|error| corruption(format!("transition generation is invalid: {error}")))?;
    if source_generation != expected_generation {
        return Err(corruption(
            "scoped query returned a transition from another generation",
        ));
    }
    let origin =
        RadrootsAddressableTransitionOriginV1::parse(row.try_get::<String, _>("origin")?.as_str())
            .map_err(|error| corruption(error.to_string()))?;
    let kind = u32_from_i64("transition.kind", row.try_get("kind")?)
        .map_err(|error| corruption(error.to_string()))?;
    let pubkey: String = row.try_get("pubkey")?;
    let d_tag: String = row.try_get("d_tag")?;
    if !(30_000..=39_999).contains(&kind)
        || d_tag.len() > RADROOTS_ADDRESSABLE_TRANSITION_D_TAG_MAX_BYTES_V1
    {
        return Err(corruption("transition coordinate is outside wire bounds"));
    }
    let coordinate = RadrootsAddressableTransitionCoordinateV1 {
        kind,
        pubkey: PublicKey::from_hex(pubkey.as_str())
            .map_err(|error| corruption(format!("transition pubkey is invalid: {error}")))?,
        d_tag,
    };
    let raw_head = required_reference(
        "raw_head",
        row.try_get("raw_head_event_id")?,
        row.try_get("raw_head_event_seq")?,
    )?;
    let raw_head_created_at = u64_from_i64(
        "transition.raw_head_created_at",
        row.try_get("raw_head_created_at")?,
    )
    .map_err(|error| corruption(error.to_string()))?;
    let visible_reference = optional_reference(
        "visible_event",
        row.try_get("visible_event_id")?,
        row.try_get("visible_event_seq")?,
    )?;
    let retracted_event = optional_reference(
        "retracted_event",
        row.try_get("retracted_event_id")?,
        row.try_get("retracted_event_seq")?,
    )?;
    let admission_status =
        RadrootsEventAdmissionStatus::parse(row.try_get::<String, _>("admission_status")?.as_str())
            .map_err(|error| corruption(error.to_string()))?;
    let admission_code: Option<String> = row.try_get("admission_code")?;
    let contract_id: Option<String> = row.try_get("contract_id")?;
    let visibility = RadrootsAddressableTransitionVisibilityV1::parse(
        row.try_get::<String, _>("visibility")?.as_str(),
    )
    .map_err(|error| corruption(error.to_string()))?;
    let suppression = suppression_evidence_from_transition_row(&row)?;
    let cause_reference = optional_reference(
        "cause_event",
        row.try_get("cause_event_id")?,
        row.try_get("cause_event_seq")?,
    )?;
    let raw_head_decision = RadrootsAddressableTransitionRawHeadDecisionV1::parse(
        row.try_get::<String, _>("raw_head_decision")?.as_str(),
    )
    .map_err(|error| corruption(error.to_string()))?;

    validate_transition_shape(
        origin,
        &raw_head,
        visible_reference.as_ref(),
        retracted_event.as_ref(),
        admission_status,
        admission_code.as_deref(),
        contract_id.as_deref(),
        visibility,
        suppression.as_ref(),
        cause_reference.as_ref(),
        raw_head_decision,
        raw_head_created_at,
    )?;

    let (raw_event, admission) = load_and_validate_stored_event(connection, &raw_head).await?;
    validate_addressable_reference(
        connection,
        source_generation,
        &coordinate,
        &raw_head,
        &raw_event,
    )
    .await?;
    if raw_event.created_at != raw_head_created_at
        || admission.status != admission_status
        || admission.code.as_deref() != admission_code.as_deref()
        || admission.contract.map(|contract| contract.id) != contract_id.as_deref()
    {
        return Err(corruption(format!(
            "transition {transition_seq} disagrees with its raw-head event"
        )));
    }

    let visible_event = if let Some(reference) = visible_reference.as_ref() {
        if reference != &raw_head {
            return Err(corruption(format!(
                "transition {transition_seq} visible event is not the raw head"
            )));
        }
        Some(RadrootsStoreProducedCanonicalEventV1 {
            event_id: *raw_head.event_id(),
            pubkey: *coordinate.pubkey(),
            created_at: raw_event.created_at,
            kind: coordinate.kind(),
            raw_json: raw_event.raw_json.clone(),
        })
    } else {
        None
    };

    if let Some(reference) = retracted_event.as_ref() {
        let (event, admission) = load_and_validate_stored_event(connection, reference).await?;
        validate_addressable_reference(
            connection,
            source_generation,
            &coordinate,
            reference,
            &event,
        )
        .await?;
        if admission.status != RadrootsEventAdmissionStatus::Admitted {
            return Err(corruption(format!(
                "transition {transition_seq} retracts an event that is not admitted"
            )));
        }
    }
    let cause = if let Some(reference) = cause_reference.as_ref() {
        if reference == &raw_head {
            Some((raw_event.clone(), admission.clone()))
        } else {
            Some(load_and_validate_stored_event(connection, reference).await?)
        }
    } else {
        None
    };
    validate_incremental_cause(
        connection,
        source_generation,
        origin,
        &coordinate,
        &raw_head,
        cause_reference.as_ref(),
        cause.as_ref(),
        suppression.as_ref(),
        raw_head_decision,
    )
    .await?;
    let current_state = TransitionStateSnapshot {
        raw_head: raw_head.clone(),
        raw_head_created_at,
        admission_status,
        admission_code: admission_code.clone(),
        contract_id: contract_id.clone(),
        visibility,
        suppression: suppression.clone(),
    };
    validate_retraction_lineage(
        connection,
        source_generation,
        transition_seq,
        origin,
        &coordinate,
        &current_state,
        retracted_event.as_ref(),
    )
    .await?;
    let cause_event = cause
        .map(|(event, admission)| {
            let event_reference = cause_reference
                .clone()
                .ok_or_else(|| corruption("loaded transition cause has no reference"))?;
            let pubkey = PublicKey::from_hex(event.pubkey.as_str()).map_err(|error| {
                corruption(format!("transition cause pubkey is invalid: {error}"))
            })?;
            Ok::<RadrootsAddressableTransitionCauseV1, RadrootsEventStoreError>(
                RadrootsAddressableTransitionCauseV1 {
                    event: event_reference,
                    pubkey,
                    created_at: event.created_at,
                    kind: event.kind,
                    admission_status: admission.status,
                    admission_code: admission.code,
                    contract_id: admission.contract.map(|contract| contract.id.to_owned()),
                },
            )
        })
        .transpose()?;

    Ok(RadrootsAddressableTransitionV1 {
        transition_seq,
        source_generation,
        origin,
        coordinate,
        raw_head,
        raw_head_created_at,
        visible_event,
        retracted_event,
        admission_status,
        admission_code,
        contract_id,
        visibility,
        suppression,
        cause_event,
        raw_head_decision,
    })
}

#[derive(PartialEq, Eq)]
struct TransitionStateSnapshot {
    raw_head: RadrootsAddressableTransitionEventReferenceV1,
    raw_head_created_at: u64,
    admission_status: RadrootsEventAdmissionStatus,
    admission_code: Option<String>,
    contract_id: Option<String>,
    visibility: RadrootsAddressableTransitionVisibilityV1,
    suppression: Option<RadrootsNip09SuppressionEvidenceV1>,
}

#[allow(clippy::too_many_arguments)]
async fn validate_incremental_cause(
    connection: &mut SqliteConnection,
    generation: RadrootsEventStoreSourceGeneration,
    origin: RadrootsAddressableTransitionOriginV1,
    coordinate: &RadrootsAddressableTransitionCoordinateV1,
    raw_head: &RadrootsAddressableTransitionEventReferenceV1,
    cause_reference: Option<&RadrootsAddressableTransitionEventReferenceV1>,
    cause: Option<&(RadrootsStoredRawEvent, EventAdmission)>,
    suppression: Option<&RadrootsNip09SuppressionEvidenceV1>,
    decision: RadrootsAddressableTransitionRawHeadDecisionV1,
) -> Result<(), RadrootsEventStoreError> {
    if origin == RadrootsAddressableTransitionOriginV1::Baseline {
        return Ok(());
    }
    let cause_reference = cause_reference
        .ok_or_else(|| corruption("incremental transition has no cause reference"))?;
    let (cause_event, cause_admission) =
        cause.ok_or_else(|| corruption("incremental transition cause could not be loaded"))?;
    match decision {
        RadrootsAddressableTransitionRawHeadDecisionV1::Applied => {
            if cause_reference != raw_head {
                return Err(corruption(
                    "applied incremental transition cause is not its new raw head",
                ));
            }
        }
        RadrootsAddressableTransitionRawHeadDecisionV1::NotHeadSelected => {
            if cause_event.kind != 5
                || cause_admission.status != RadrootsEventAdmissionStatus::Admitted
            {
                return Err(corruption(
                    "non-head incremental transition was not caused by an admitted deletion request",
                ));
            }
            let author_matches = cause_event.pubkey == coordinate.pubkey().to_hex();
            let records_author_mismatch = suppression.is_some_and(|evidence| {
                evidence.reason()
                    == crate::model::RadrootsNip09SuppressionReason::RequestAuthorMismatch
            });
            if author_matches == records_author_mismatch {
                return Err(corruption(
                    "deletion cause author does not agree with suppression evidence",
                ));
            }
            let targeted: i64 = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM radroots_event_store_nip09_event_target WHERE source_generation = ? AND request_event_id = ? AND target_event_id = ?) OR EXISTS(SELECT 1 FROM radroots_event_store_nip09_address_target WHERE source_generation = ? AND request_event_id = ? AND target_kind = ? AND target_pubkey = ? AND target_d_tag = ?)",
            )
            .bind(generation.as_bytes().as_slice())
            .bind(cause_reference.event_id().to_hex())
            .bind(raw_head.event_id().to_hex())
            .bind(generation.as_bytes().as_slice())
            .bind(cause_reference.event_id().to_hex())
            .bind(i64::from(coordinate.kind()))
            .bind(coordinate.pubkey().to_hex())
            .bind(coordinate.d_tag())
            .fetch_one(&mut *connection)
            .await?;
            if targeted != 1 {
                return Err(corruption(
                    "deletion cause does not target the transitioned coordinate",
                ));
            }
        }
        RadrootsAddressableTransitionRawHeadDecisionV1::BaselineRebuild
        | RadrootsAddressableTransitionRawHeadDecisionV1::SkippedOlder
        | RadrootsAddressableTransitionRawHeadDecisionV1::SkippedSameTimestampHigherEventId
        | RadrootsAddressableTransitionRawHeadDecisionV1::MalformedCoordinate => {
            return Err(corruption(format!(
                "raw-head decision `{}` cannot emit an incremental transition",
                decision.as_str()
            )));
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn validate_retraction_lineage(
    connection: &mut SqliteConnection,
    generation: RadrootsEventStoreSourceGeneration,
    transition_seq: i64,
    origin: RadrootsAddressableTransitionOriginV1,
    coordinate: &RadrootsAddressableTransitionCoordinateV1,
    current: &TransitionStateSnapshot,
    retracted: Option<&RadrootsAddressableTransitionEventReferenceV1>,
) -> Result<(), RadrootsEventStoreError> {
    let prior = sqlx::query(
        "SELECT raw_head_event_id, raw_head_event_seq, raw_head_created_at, admission_status, admission_code, contract_id, visibility, nip09_outcome, nip09_reason, event_reference_request_id, address_reference_request_id, address_reference_cutoff FROM radroots_event_store_addressable_head_transition WHERE source_generation = ? AND kind = ? AND pubkey = ? AND d_tag = ? AND transition_seq < ? ORDER BY transition_seq DESC LIMIT 1",
    )
    .bind(generation.as_bytes().as_slice())
    .bind(i64::from(coordinate.kind()))
    .bind(coordinate.pubkey().to_hex())
    .bind(coordinate.d_tag())
    .bind(transition_seq)
    .fetch_optional(&mut *connection)
    .await?;
    let expected = if let Some(prior) = prior {
        if origin == RadrootsAddressableTransitionOriginV1::Baseline {
            return Err(corruption(
                "baseline transition follows existing coordinate state",
            ));
        }
        let prior_state = TransitionStateSnapshot {
            raw_head: required_reference(
                "prior_raw_head",
                prior.try_get("raw_head_event_id")?,
                prior.try_get("raw_head_event_seq")?,
            )?,
            raw_head_created_at: u64_from_i64(
                "prior_transition.raw_head_created_at",
                prior.try_get("raw_head_created_at")?,
            )
            .map_err(|error| corruption(error.to_string()))?,
            admission_status: RadrootsEventAdmissionStatus::parse(
                prior.try_get::<String, _>("admission_status")?.as_str(),
            )
            .map_err(|error| corruption(error.to_string()))?,
            admission_code: prior.try_get("admission_code")?,
            contract_id: prior.try_get("contract_id")?,
            visibility: RadrootsAddressableTransitionVisibilityV1::parse(
                prior.try_get::<String, _>("visibility")?.as_str(),
            )
            .map_err(|error| corruption(error.to_string()))?,
            suppression: suppression_evidence_from_transition_row(&prior)?,
        };
        if prior_state == *current {
            return Err(corruption(
                "incremental transition repeats the complete prior state",
            ));
        }
        (prior_state.visibility == RadrootsAddressableTransitionVisibilityV1::Visible
            && (current.visibility != RadrootsAddressableTransitionVisibilityV1::Visible
                || prior_state.raw_head != current.raw_head))
            .then_some(prior_state.raw_head)
    } else {
        if origin == RadrootsAddressableTransitionOriginV1::Baseline && retracted.is_some() {
            return Err(corruption("baseline transition retracts prior state"));
        }
        None
    };
    if expected.as_ref() != retracted {
        return Err(corruption(
            "transition retraction does not match the immediately preceding visible state",
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_transition_shape(
    origin: RadrootsAddressableTransitionOriginV1,
    raw_head: &RadrootsAddressableTransitionEventReferenceV1,
    visible_event: Option<&RadrootsAddressableTransitionEventReferenceV1>,
    retracted_event: Option<&RadrootsAddressableTransitionEventReferenceV1>,
    admission_status: RadrootsEventAdmissionStatus,
    admission_code: Option<&str>,
    contract_id: Option<&str>,
    visibility: RadrootsAddressableTransitionVisibilityV1,
    suppression: Option<&RadrootsNip09SuppressionEvidenceV1>,
    cause_event: Option<&RadrootsAddressableTransitionEventReferenceV1>,
    raw_head_decision: RadrootsAddressableTransitionRawHeadDecisionV1,
    raw_head_created_at: u64,
) -> Result<(), RadrootsEventStoreError> {
    let origin_valid = match origin {
        RadrootsAddressableTransitionOriginV1::Baseline => {
            cause_event.is_none()
                && retracted_event.is_none()
                && raw_head_decision
                    == RadrootsAddressableTransitionRawHeadDecisionV1::BaselineRebuild
        }
        RadrootsAddressableTransitionOriginV1::Incremental => {
            cause_event.is_some()
                && raw_head_decision
                    != RadrootsAddressableTransitionRawHeadDecisionV1::BaselineRebuild
        }
    };
    let admission_valid = match admission_status {
        RadrootsEventAdmissionStatus::Admitted => admission_code.is_none() && contract_id.is_some(),
        RadrootsEventAdmissionStatus::Unsupported | RadrootsEventAdmissionStatus::Invalid => {
            admission_code.is_some() && contract_id.is_none()
        }
    };
    let visibility_valid = match visibility {
        RadrootsAddressableTransitionVisibilityV1::Visible => {
            admission_status == RadrootsEventAdmissionStatus::Admitted
                && visible_event == Some(raw_head)
                && suppression.is_some_and(|evidence| {
                    evidence.outcome() == RadrootsNip09SuppressionOutcome::Visible
                })
        }
        RadrootsAddressableTransitionVisibilityV1::NotAdmitted => {
            admission_status != RadrootsEventAdmissionStatus::Admitted
                && visible_event.is_none()
                && suppression.is_none()
        }
        RadrootsAddressableTransitionVisibilityV1::Suppressed => {
            admission_status == RadrootsEventAdmissionStatus::Admitted
                && visible_event.is_none()
                && suppression.is_some_and(|evidence| {
                    evidence.outcome() == RadrootsNip09SuppressionOutcome::Suppressed
                })
        }
    };
    if !origin_valid || !admission_valid || !visibility_valid {
        return Err(corruption(
            "transition fields have an incoherent decision shape",
        ));
    }
    if let (Some(visible), Some(retracted)) = (visible_event, retracted_event)
        && visible == retracted
    {
        return Err(corruption("transition retracts the event it makes visible"));
    }
    if let Some(evidence) = suppression {
        validate_suppression_shape(evidence, raw_head_created_at)?;
    }
    Ok(())
}

fn validate_suppression_shape(
    evidence: &RadrootsNip09SuppressionEvidenceV1,
    raw_head_created_at: u64,
) -> Result<(), RadrootsEventStoreError> {
    if !evidence.is_coherent_for_event(30_000, raw_head_created_at) {
        return Err(corruption(
            "suppression evidence is internally inconsistent",
        ));
    }
    Ok(())
}

fn suppression_evidence_from_transition_row(
    row: &sqlx::sqlite::SqliteRow,
) -> Result<Option<RadrootsNip09SuppressionEvidenceV1>, RadrootsEventStoreError> {
    let outcome: Option<String> = row.try_get("nip09_outcome")?;
    let reason: Option<String> = row.try_get("nip09_reason")?;
    let event_reference_request_id = optional_event_id(
        "event_reference_request_id",
        row.try_get("event_reference_request_id")?,
    )?;
    let address_reference_request_id = optional_event_id(
        "address_reference_request_id",
        row.try_get("address_reference_request_id")?,
    )?;
    let address_reference_cutoff = row
        .try_get::<Option<i64>, _>("address_reference_cutoff")?
        .map(|value| u64_from_i64("transition.address_reference_cutoff", value))
        .transpose()
        .map_err(|error| corruption(error.to_string()))?;
    match (outcome, reason) {
        (Some(outcome), Some(reason)) => Ok(Some(RadrootsNip09SuppressionEvidenceV1 {
            outcome: parse_suppression_outcome(outcome.as_str())
                .map_err(|error| corruption(error.to_string()))?,
            reason: parse_suppression_reason(reason.as_str())
                .map_err(|error| corruption(error.to_string()))?,
            event_reference_request_id,
            address_reference_request_id,
            address_reference_cutoff,
        })),
        (None, None)
            if event_reference_request_id.is_none()
                && address_reference_request_id.is_none()
                && address_reference_cutoff.is_none() =>
        {
            Ok(None)
        }
        _ => Err(corruption("transition has incomplete suppression evidence")),
    }
}

fn required_reference(
    field: &'static str,
    event_id: String,
    event_seq: i64,
) -> Result<RadrootsAddressableTransitionEventReferenceV1, RadrootsEventStoreError> {
    if event_seq <= 0 {
        return Err(corruption(format!("{field} sequence is not positive")));
    }
    Ok(RadrootsAddressableTransitionEventReferenceV1 {
        event_id: parse_event_id(field, event_id)?,
        event_seq,
    })
}

fn optional_reference(
    field: &'static str,
    event_id: Option<String>,
    event_seq: Option<i64>,
) -> Result<Option<RadrootsAddressableTransitionEventReferenceV1>, RadrootsEventStoreError> {
    match (event_id, event_seq) {
        (Some(event_id), Some(event_seq)) => {
            required_reference(field, event_id, event_seq).map(Some)
        }
        (None, None) => Ok(None),
        _ => Err(corruption(format!("{field} identity is partial"))),
    }
}

fn optional_event_id(
    field: &'static str,
    value: Option<String>,
) -> Result<Option<EventId>, RadrootsEventStoreError> {
    value.map(|value| parse_event_id(field, value)).transpose()
}

fn parse_event_id(field: &'static str, value: String) -> Result<EventId, RadrootsEventStoreError> {
    EventId::parse(value.as_str())
        .map_err(|error| corruption(format!("{field} event id is invalid: {error}")))
}

async fn load_and_validate_stored_event(
    connection: &mut SqliteConnection,
    reference: &RadrootsAddressableTransitionEventReferenceV1,
) -> Result<(RadrootsStoredRawEvent, EventAdmission), RadrootsEventStoreError> {
    let row = sqlx::query(
        "SELECT seq, event_id, pubkey, created_at, kind, tags_json, content, sig, raw_json, verification_status, contract_status, contract_id, event_class, projection_eligible, inserted_at_ms, updated_at_ms FROM event_envelopes WHERE seq = ? AND event_id = ?",
    )
    .bind(reference.event_seq())
    .bind(reference.event_id().to_hex())
    .fetch_optional(&mut *connection)
    .await?
    .ok_or_else(|| {
        corruption(format!(
            "referenced event `{}` at sequence {} is missing",
            reference.event_id(),
            reference.event_seq()
        ))
    })?;
    let stored = stored_raw_event_from_row(row)
        .map_err(|error| corruption(format!("stored raw event row is invalid: {error}")))?;
    let reconstructed = RadrootsEventIngest::from_raw_json(stored.raw_json.clone(), 0)
        .map_err(|error| corruption(format!("stored raw event cannot be reverified: {error}")))?;
    let event = reconstructed.event();
    let tags_json = serde_json::to_string(&event.tags_as_vec()).map_err(|error| {
        corruption(format!(
            "stored raw event tags cannot be canonicalized: {error}"
        ))
    })?;
    if stored.event_id != event.id_hex()
        || stored.pubkey != event.author().to_hex()
        || stored.created_at != event.created_at_u64()
        || stored.kind != event.kind_u32()
        || stored.tags_json != tags_json
        || stored.content != event.content()
        || stored.sig != event.signature_hex()
    {
        return Err(corruption(format!(
            "stored event `{}` disagrees with its signed raw JSON",
            reference.event_id()
        )));
    }
    let admission = EventAdmission::for_profile(
        ReconciliationProfile::Nip09V1RegistryV7,
        reconstructed.verified_event(),
    )
    .map_err(|error| corruption(format!("stored raw event cannot be admitted: {error}")))?;
    if admission.status != stored.admission_status
        || admission.contract.map(|contract| contract.id) != stored.contract_id.as_deref()
        || admission.valid_stream_eligible(event.kind_class()) != stored.valid_stream_eligible
    {
        return Err(corruption(format!(
            "stored event `{}` disagrees with registry-v7 admission",
            reference.event_id()
        )));
    }
    Ok((stored, admission))
}

async fn validate_addressable_reference(
    connection: &mut SqliteConnection,
    generation: RadrootsEventStoreSourceGeneration,
    coordinate: &RadrootsAddressableTransitionCoordinateV1,
    reference: &RadrootsAddressableTransitionEventReferenceV1,
    event: &RadrootsStoredRawEvent,
) -> Result<(), RadrootsEventStoreError> {
    if event.event_class != StoredEventClass::Addressable
        || event.kind != coordinate.kind()
        || event.pubkey != coordinate.pubkey().to_hex()
    {
        return Err(corruption(format!(
            "event `{}` does not match transition coordinate `{}:{}:{}`",
            reference.event_id(),
            coordinate.kind(),
            coordinate.pubkey(),
            coordinate.d_tag()
        )));
    }
    let exists: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM radroots_event_store_event_coordinate WHERE source_generation = ? AND event_seq = ? AND event_id = ? AND coordinate_type = 'addressable' AND kind = ? AND pubkey = ? AND raw_d_tag = ?",
    )
    .bind(generation.as_bytes().as_slice())
    .bind(reference.event_seq())
    .bind(reference.event_id().to_hex())
    .bind(i64::from(coordinate.kind()))
    .bind(coordinate.pubkey().to_hex())
    .bind(coordinate.d_tag())
    .fetch_one(&mut *connection)
    .await?;
    if exists != 1 {
        return Err(corruption(format!(
            "event `{}` has no matching addressable coordinate authority",
            reference.event_id()
        )));
    }
    Ok(())
}

fn corruption(reason: impl Into<String>) -> RadrootsEventStoreError {
    RadrootsEventStoreError::AddressableTransitionCorruption {
        reason: reason.into(),
    }
}
