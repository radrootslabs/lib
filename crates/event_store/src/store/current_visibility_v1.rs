use super::protocol_storage_v1::stored_raw_event_from_row;
use super::{RadrootsEventStore, bool_from_i64, u64_from_i64};
use crate::RadrootsEventStoreError;
use crate::model::{
    RadrootsCurrentEventVisibilityV1, RadrootsCurrentVisibilityDecisionV1,
    RadrootsNip09SuppressionEvidenceV1, RadrootsNip09SuppressionOutcome,
    RadrootsNip09SuppressionReason, StoredEventClass,
};
use crate::nip09::reconciliation_v1::generation_from_blob;
use radroots_event::ids::RadrootsEventId;
use sqlx::{Row, Sqlite, Transaction};

impl RadrootsEventStore {
    pub async fn current_event_visibility_v1(
        &self,
        event_id: &str,
    ) -> Result<Option<RadrootsCurrentEventVisibilityV1>, RadrootsEventStoreError> {
        let mut tx = self.pool.begin().await?;
        let result = current_visibility_in_transaction(&mut tx, event_id).await?;
        tx.commit().await?;
        Ok(result)
    }
}

pub(super) async fn current_visibility_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    event_id: &str,
) -> Result<Option<RadrootsCurrentEventVisibilityV1>, RadrootsEventStoreError> {
    let event_exists: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM event_envelopes WHERE event_id = ?")
            .bind(event_id)
            .fetch_one(&mut **tx)
            .await?;
    if event_exists == 0 {
        return Ok(None);
    }
    let row = sqlx::query(
        "SELECT event.seq, event.event_id, event.pubkey, event.created_at, event.kind, event.tags_json, event.content, event.sig, event.raw_json, event.verification_status, event.contract_status, event.contract_id, event.event_class, event.projection_eligible, event.inserted_at_ms, event.updated_at_ms, visibility.source_generation, visibility.raw_d_tag, visibility.is_raw_head, visibility.raw_head_event_id, visibility.suppression_outcome, visibility.suppression_reason, visibility.event_reference_request_id, visibility.address_reference_request_id, visibility.address_reference_cutoff, visibility.current_visibility FROM radroots_event_store_current_visibility_v1 AS visibility JOIN event_envelopes AS event ON event.event_id = visibility.event_id WHERE visibility.event_id = ?",
    )
    .bind(event_id)
    .fetch_optional(&mut **tx)
    .await?;
    let row = row.ok_or_else(|| RadrootsEventStoreError::CurrentVisibilityDrift {
        reason: format!("stored event `{event_id}` has no current-visibility authority"),
    })?;

    let source_generation = generation_from_blob(row.try_get("source_generation")?)
        .map_err(|error| visibility_authority_error("source generation", error))?;
    let raw_d_tag: Option<String> = row.try_get("raw_d_tag")?;
    let is_raw_head = bool_from_i64(
        "current_visibility.is_raw_head",
        row.try_get("is_raw_head")?,
    )
    .map_err(|error| visibility_authority_error("raw-head marker", error))?;
    let raw_head_event_id = row
        .try_get::<Option<String>, _>("raw_head_event_id")?
        .map(RadrootsEventId::parse)
        .transpose()
        .map_err(|error| visibility_authority_error("raw-head event id", error))?;
    let decision = RadrootsCurrentVisibilityDecisionV1::parse(
        row.try_get::<String, _>("current_visibility")?.as_str(),
    )
    .map_err(|error| visibility_authority_error("visibility decision", error))?;
    let suppression = suppression_evidence_from_row(&row)
        .map_err(|error| visibility_authority_error("suppression evidence", error))?;
    let event = stored_raw_event_from_row(row)
        .map_err(|error| visibility_authority_error("stored raw event", error))?;
    let visibility = RadrootsCurrentEventVisibilityV1 {
        source_generation,
        event,
        is_raw_head,
        raw_head_event_id,
        suppression,
        decision,
    };
    validate_visibility_shape(&visibility)?;
    validate_addressable_head_projection(tx, &visibility, raw_d_tag.as_deref()).await?;
    Ok(Some(visibility))
}

pub(super) fn suppression_evidence_from_row(
    row: &sqlx::sqlite::SqliteRow,
) -> Result<Option<RadrootsNip09SuppressionEvidenceV1>, RadrootsEventStoreError> {
    let outcome: Option<String> = row.try_get("suppression_outcome")?;
    let reason: Option<String> = row.try_get("suppression_reason")?;
    let event_reference_request_id = row
        .try_get::<Option<String>, _>("event_reference_request_id")?
        .map(RadrootsEventId::parse)
        .transpose()
        .map_err(|error| visibility_authority_error("event deletion request id", error))?;
    let address_reference_request_id = row
        .try_get::<Option<String>, _>("address_reference_request_id")?
        .map(RadrootsEventId::parse)
        .transpose()
        .map_err(|error| visibility_authority_error("address deletion request id", error))?;
    let address_reference_cutoff = row
        .try_get::<Option<i64>, _>("address_reference_cutoff")?
        .map(|value| {
            u64::try_from(value).map_err(|_| RadrootsEventStoreError::IntegerRange {
                field: "current_visibility.address_reference_cutoff",
                value,
            })
        })
        .transpose()
        .map_err(|error| visibility_authority_error("address deletion cutoff", error))?;
    match (outcome, reason) {
        (Some(outcome), Some(reason)) => Ok(Some(RadrootsNip09SuppressionEvidenceV1 {
            outcome: parse_suppression_outcome(outcome.as_str())
                .map_err(|error| visibility_authority_error("suppression outcome", error))?,
            reason: parse_suppression_reason(reason.as_str())
                .map_err(|error| visibility_authority_error("suppression reason", error))?,
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
        _ => current_visibility_drift("suppression evidence is incomplete"),
    }
}

pub(super) fn parse_suppression_outcome(
    value: &str,
) -> Result<RadrootsNip09SuppressionOutcome, RadrootsEventStoreError> {
    match value {
        "visible" => Ok(RadrootsNip09SuppressionOutcome::Visible),
        "suppressed" => Ok(RadrootsNip09SuppressionOutcome::Suppressed),
        _ => Err(RadrootsEventStoreError::InvalidStoredEnum {
            field: "nip09.suppression_outcome",
            value: value.to_owned(),
        }),
    }
}

pub(super) fn parse_suppression_reason(
    value: &str,
) -> Result<RadrootsNip09SuppressionReason, RadrootsEventStoreError> {
    match value {
        "deletion_request_immune" => Ok(RadrootsNip09SuppressionReason::DeletionRequestImmune),
        "deletion_no_authorized_reference" => {
            Ok(RadrootsNip09SuppressionReason::NoAuthorizedReference)
        }
        "deletion_request_author_mismatch" => {
            Ok(RadrootsNip09SuppressionReason::RequestAuthorMismatch)
        }
        "deletion_address_cutoff_precedes_target" => {
            Ok(RadrootsNip09SuppressionReason::AddressCutoffPrecedesTarget)
        }
        "deletion_event_id_reference" => Ok(RadrootsNip09SuppressionReason::EventIdReference),
        "deletion_address_reference" => {
            Ok(RadrootsNip09SuppressionReason::AddressReferenceAtOrBeforeCutoff)
        }
        "deletion_event_id_and_address_reference" => {
            Ok(RadrootsNip09SuppressionReason::EventIdAndAddressReference)
        }
        _ => Err(RadrootsEventStoreError::InvalidStoredEnum {
            field: "nip09.suppression_reason",
            value: value.to_owned(),
        }),
    }
}

fn validate_visibility_shape(
    visibility: &RadrootsCurrentEventVisibilityV1,
) -> Result<(), RadrootsEventStoreError> {
    let event_id = visibility.event.event_id.as_str();
    if visibility.event.event_class == StoredEventClass::Ephemeral {
        return current_visibility_drift(format!(
            "persisted ephemeral event `{event_id}` entered current visibility"
        ));
    }
    let regular = visibility.event.event_class == StoredEventClass::Regular;
    if regular && (visibility.raw_head_event_id.is_some() || !visibility.is_raw_head) {
        return current_visibility_drift(format!(
            "event `{event_id}` has inconsistent raw-head identity"
        ));
    }
    if !regular
        && visibility.is_raw_head
        && visibility
            .raw_head_event_id
            .as_ref()
            .is_none_or(|raw_head_event_id| raw_head_event_id.as_str() != event_id)
    {
        return current_visibility_drift(format!(
            "event `{event_id}` is marked as the raw head without matching head identity"
        ));
    }
    if !regular
        && visibility.raw_head_event_id.is_none()
        && visibility.decision != RadrootsCurrentVisibilityDecisionV1::NotAdmitted
    {
        return current_visibility_drift(format!(
            "event `{event_id}` has no coordinate head but is not classified as not admitted"
        ));
    }
    let evidence = visibility.suppression.as_ref();
    if evidence.is_some_and(|value| {
        !value.is_coherent_for_event(visibility.event.kind, visibility.event.created_at)
    }) {
        return current_visibility_drift(format!(
            "event `{event_id}` has incoherent suppression evidence"
        ));
    }
    let valid = match visibility.decision {
        RadrootsCurrentVisibilityDecisionV1::Visible => {
            (
                visibility.event.admission_status,
                visibility.is_raw_head,
                evidence.map(|value| value.outcome),
            ) == (
                crate::model::RadrootsEventAdmissionStatus::Admitted,
                true,
                Some(RadrootsNip09SuppressionOutcome::Visible),
            )
        }
        RadrootsCurrentVisibilityDecisionV1::NotAdmitted => {
            (
                visibility.event.admission_status
                    == crate::model::RadrootsEventAdmissionStatus::Admitted,
                evidence.is_none(),
            ) == (false, true)
        }
        RadrootsCurrentVisibilityDecisionV1::NotCurrent => {
            (
                visibility.event.admission_status,
                visibility.is_raw_head,
                visibility.raw_head_event_id.is_some(),
                evidence.is_some(),
            ) == (
                crate::model::RadrootsEventAdmissionStatus::Admitted,
                false,
                true,
                true,
            )
        }
        RadrootsCurrentVisibilityDecisionV1::Suppressed => {
            (
                visibility.event.admission_status,
                visibility.is_raw_head,
                evidence.map(|value| value.outcome),
            ) == (
                crate::model::RadrootsEventAdmissionStatus::Admitted,
                true,
                Some(RadrootsNip09SuppressionOutcome::Suppressed),
            )
        }
    };
    if !valid {
        return current_visibility_drift(format!(
            "event `{event_id}` has an incoherent visibility decision"
        ));
    }
    Ok(())
}

async fn validate_addressable_head_projection(
    tx: &mut Transaction<'_, Sqlite>,
    visibility: &RadrootsCurrentEventVisibilityV1,
    raw_d_tag: Option<&str>,
) -> Result<(), RadrootsEventStoreError> {
    if visibility.event.event_class != StoredEventClass::Addressable || !visibility.is_raw_head {
        return Ok(());
    }
    let raw_d_tag = raw_d_tag.ok_or_else(|| RadrootsEventStoreError::CurrentVisibilityDrift {
        reason: format!(
            "addressable event `{}` has no raw d tag",
            visibility.event.event_id
        ),
    })?;
    let row = sqlx::query(
        "SELECT state.raw_head_event_id, state.raw_head_event_seq, state.raw_head_created_at, state.admission_status, state.admission_code, coordinate.admission_code AS coordinate_admission_code, state.contract_id, state.visibility, state.nip09_outcome, state.nip09_reason, state.event_reference_request_id, state.address_reference_request_id, state.address_reference_cutoff FROM radroots_event_store_addressable_head_state AS state JOIN radroots_event_store_event_coordinate AS coordinate ON coordinate.source_generation = state.source_generation AND coordinate.event_seq = state.raw_head_event_seq AND coordinate.event_id = state.raw_head_event_id AND coordinate.coordinate_type = 'addressable' AND coordinate.kind = state.kind AND coordinate.pubkey = state.pubkey AND coordinate.raw_d_tag = state.d_tag WHERE state.source_generation = ? AND state.kind = ? AND state.pubkey = ? AND state.d_tag = ?",
    )
    .bind(visibility.source_generation.as_bytes().as_slice())
    .bind(i64::from(visibility.event.kind))
    .bind(visibility.event.pubkey.as_str())
    .bind(raw_d_tag)
    .fetch_optional(&mut **tx)
    .await?;
    let Some(row) = row else {
        return current_visibility_drift(format!(
            "addressable head state is missing for `{}`",
            visibility.event.event_id
        ));
    };
    let evidence = visibility.suppression.as_ref();
    let stored_created_at = u64_from_i64(
        "addressable_head_state.raw_head_created_at",
        row.try_get("raw_head_created_at")?,
    )
    .map_err(|error| visibility_authority_error("stored raw-head created-at", error))?;
    let stored_cutoff = row
        .try_get::<Option<i64>, _>("address_reference_cutoff")?
        .map(|value| {
            u64::try_from(value).map_err(|_| RadrootsEventStoreError::IntegerRange {
                field: "addressable_head_state.address_reference_cutoff",
                value,
            })
        })
        .transpose()
        .map_err(|error| visibility_authority_error("stored address deletion cutoff", error))?;
    let stored_raw_head_event_id: String = row.try_get("raw_head_event_id")?;
    let stored_raw_head_event_seq: i64 = row.try_get("raw_head_event_seq")?;
    let stored_admission_status: String = row.try_get("admission_status")?;
    let stored_admission_code: Option<String> = row.try_get("admission_code")?;
    let coordinate_admission_code: Option<String> = row.try_get("coordinate_admission_code")?;
    let stored_contract_id: Option<String> = row.try_get("contract_id")?;
    let stored_visibility: String = row.try_get("visibility")?;
    let stored_outcome: Option<String> = row.try_get("nip09_outcome")?;
    let stored_reason: Option<String> = row.try_get("nip09_reason")?;
    let stored_event_reference_request_id: Option<String> =
        row.try_get("event_reference_request_id")?;
    let stored_address_reference_request_id: Option<String> =
        row.try_get("address_reference_request_id")?;
    if (
        stored_raw_head_event_id.as_str(),
        stored_raw_head_event_seq,
        stored_created_at,
        stored_admission_status.as_str(),
        stored_admission_code.as_deref(),
        stored_contract_id.as_deref(),
        stored_visibility.as_str(),
        stored_outcome.as_deref(),
        stored_reason.as_deref(),
        stored_event_reference_request_id.as_deref(),
        stored_address_reference_request_id.as_deref(),
        stored_cutoff,
    ) != (
        visibility.event.event_id.as_str(),
        visibility.event.seq,
        visibility.event.created_at,
        visibility.event.admission_status.as_str(),
        coordinate_admission_code.as_deref(),
        visibility.event.contract_id.as_deref(),
        visibility.decision.as_str(),
        evidence.map(|value| value.outcome.code()),
        evidence.map(|value| value.reason.code()),
        evidence
            .and_then(|value| value.event_reference_request_id.as_ref())
            .map(RadrootsEventId::as_str),
        evidence
            .and_then(|value| value.address_reference_request_id.as_ref())
            .map(RadrootsEventId::as_str),
        evidence.and_then(|value| value.address_reference_cutoff),
    ) {
        return current_visibility_drift(format!(
            "central visibility disagrees with addressable head state for `{}`",
            visibility.event.event_id
        ));
    }
    Ok(())
}

fn current_visibility_drift<T>(reason: impl Into<String>) -> Result<T, RadrootsEventStoreError> {
    Err(RadrootsEventStoreError::CurrentVisibilityDrift {
        reason: reason.into(),
    })
}

fn visibility_authority_error(
    context: &'static str,
    error: impl core::fmt::Display,
) -> RadrootsEventStoreError {
    RadrootsEventStoreError::CurrentVisibilityDrift {
        reason: format!("{context} is invalid: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        RadrootsEventAdmissionStatus, RadrootsEventStoreSourceGeneration, RadrootsStoredRawEvent,
    };
    use sqlx::{Connection, SqliteConnection};

    async fn suppression_evidence_row(
        connection: &mut SqliteConnection,
        outcome: Option<&str>,
        reason: Option<&str>,
        event_reference_request_id: Option<&str>,
        address_reference_request_id: Option<&str>,
        address_reference_cutoff: Option<i64>,
    ) -> sqlx::sqlite::SqliteRow {
        sqlx::query(
            "SELECT ? AS suppression_outcome, ? AS suppression_reason, ? AS event_reference_request_id, ? AS address_reference_request_id, ? AS address_reference_cutoff",
        )
        .bind(outcome)
        .bind(reason)
        .bind(event_reference_request_id)
        .bind(address_reference_request_id)
        .bind(address_reference_cutoff)
        .fetch_one(connection)
        .await
        .expect("suppression evidence row")
    }

    fn event(event_class: StoredEventClass) -> RadrootsStoredRawEvent {
        RadrootsStoredRawEvent {
            seq: 1,
            event_id: "a".repeat(64),
            pubkey: "b".repeat(64),
            created_at: 7,
            kind: match event_class {
                StoredEventClass::Regular => 1,
                StoredEventClass::Replaceable => 10_001,
                StoredEventClass::Addressable => 30_001,
                StoredEventClass::Ephemeral => 20_001,
            },
            tags_json: "[]".to_owned(),
            content: String::new(),
            sig: "c".repeat(128),
            raw_json: "{}".to_owned(),
            admission_status: RadrootsEventAdmissionStatus::Admitted,
            contract_id: Some("fixture".to_owned()),
            event_class,
            valid_stream_eligible: true,
            inserted_at_ms: 8,
            updated_at_ms: 9,
        }
    }

    fn visible_evidence() -> RadrootsNip09SuppressionEvidenceV1 {
        RadrootsNip09SuppressionEvidenceV1 {
            outcome: RadrootsNip09SuppressionOutcome::Visible,
            reason: RadrootsNip09SuppressionReason::NoAuthorizedReference,
            event_reference_request_id: None,
            address_reference_request_id: None,
            address_reference_cutoff: None,
        }
    }

    fn suppressed_evidence() -> RadrootsNip09SuppressionEvidenceV1 {
        RadrootsNip09SuppressionEvidenceV1 {
            outcome: RadrootsNip09SuppressionOutcome::Suppressed,
            reason: RadrootsNip09SuppressionReason::EventIdReference,
            event_reference_request_id: Some(
                RadrootsEventId::parse("d".repeat(64)).expect("request id"),
            ),
            address_reference_request_id: None,
            address_reference_cutoff: None,
        }
    }

    fn visibility(
        event_class: StoredEventClass,
        decision: RadrootsCurrentVisibilityDecisionV1,
        is_raw_head: bool,
        raw_head_event_id: Option<&str>,
        suppression: Option<RadrootsNip09SuppressionEvidenceV1>,
    ) -> RadrootsCurrentEventVisibilityV1 {
        RadrootsCurrentEventVisibilityV1 {
            source_generation: RadrootsEventStoreSourceGeneration::from_bytes([0x11; 32]),
            event: event(event_class),
            is_raw_head,
            raw_head_event_id: raw_head_event_id
                .map(|value| RadrootsEventId::parse(value.repeat(64)).expect("raw-head event id")),
            suppression,
            decision,
        }
    }

    #[test]
    fn suppression_storage_enums_round_trip_and_reject_unknown_values() {
        for (value, expected) in [
            ("visible", RadrootsNip09SuppressionOutcome::Visible),
            ("suppressed", RadrootsNip09SuppressionOutcome::Suppressed),
        ] {
            assert_eq!(parse_suppression_outcome(value).expect("outcome"), expected);
        }
        assert!(parse_suppression_outcome("unknown").is_err());

        for (value, expected) in [
            (
                "deletion_request_immune",
                RadrootsNip09SuppressionReason::DeletionRequestImmune,
            ),
            (
                "deletion_no_authorized_reference",
                RadrootsNip09SuppressionReason::NoAuthorizedReference,
            ),
            (
                "deletion_request_author_mismatch",
                RadrootsNip09SuppressionReason::RequestAuthorMismatch,
            ),
            (
                "deletion_address_cutoff_precedes_target",
                RadrootsNip09SuppressionReason::AddressCutoffPrecedesTarget,
            ),
            (
                "deletion_event_id_reference",
                RadrootsNip09SuppressionReason::EventIdReference,
            ),
            (
                "deletion_address_reference",
                RadrootsNip09SuppressionReason::AddressReferenceAtOrBeforeCutoff,
            ),
            (
                "deletion_event_id_and_address_reference",
                RadrootsNip09SuppressionReason::EventIdAndAddressReference,
            ),
        ] {
            assert_eq!(parse_suppression_reason(value).expect("reason"), expected);
        }
        assert!(parse_suppression_reason("unknown").is_err());
    }

    #[tokio::test]
    async fn suppression_row_decoder_rejects_each_malformed_authority() {
        let mut connection = SqliteConnection::connect("sqlite::memory:")
            .await
            .expect("in-memory SQLite connection");
        let event_request_id = "a".repeat(64);
        let address_request_id = "b".repeat(64);

        let empty = suppression_evidence_row(&mut connection, None, None, None, None, None).await;
        assert_eq!(
            suppression_evidence_from_row(&empty).expect("empty evidence"),
            None
        );

        let populated = suppression_evidence_row(
            &mut connection,
            Some("visible"),
            Some("deletion_no_authorized_reference"),
            Some(event_request_id.as_str()),
            Some(address_request_id.as_str()),
            Some(7),
        )
        .await;
        let evidence = suppression_evidence_from_row(&populated)
            .expect("populated evidence")
            .expect("suppression evidence");
        assert_eq!(evidence.outcome, RadrootsNip09SuppressionOutcome::Visible);
        assert_eq!(
            evidence.reason,
            RadrootsNip09SuppressionReason::NoAuthorizedReference
        );
        assert_eq!(
            evidence
                .event_reference_request_id
                .as_ref()
                .map(RadrootsEventId::as_str),
            Some(event_request_id.as_str()),
        );
        assert_eq!(
            evidence
                .address_reference_request_id
                .as_ref()
                .map(RadrootsEventId::as_str),
            Some(address_request_id.as_str()),
        );
        assert_eq!(evidence.address_reference_cutoff, Some(7));

        for (label, row, expected_context) in [
            (
                "event request id",
                suppression_evidence_row(
                    &mut connection,
                    Some("visible"),
                    Some("deletion_no_authorized_reference"),
                    Some("invalid"),
                    None,
                    None,
                )
                .await,
                "event deletion request id",
            ),
            (
                "address request id",
                suppression_evidence_row(
                    &mut connection,
                    Some("visible"),
                    Some("deletion_no_authorized_reference"),
                    None,
                    Some("invalid"),
                    None,
                )
                .await,
                "address deletion request id",
            ),
            (
                "address cutoff",
                suppression_evidence_row(
                    &mut connection,
                    Some("visible"),
                    Some("deletion_no_authorized_reference"),
                    None,
                    None,
                    Some(-1),
                )
                .await,
                "address deletion cutoff",
            ),
            (
                "outcome",
                suppression_evidence_row(
                    &mut connection,
                    Some("invalid"),
                    Some("deletion_no_authorized_reference"),
                    None,
                    None,
                    None,
                )
                .await,
                "suppression outcome",
            ),
            (
                "reason",
                suppression_evidence_row(
                    &mut connection,
                    Some("visible"),
                    Some("invalid"),
                    None,
                    None,
                    None,
                )
                .await,
                "suppression reason",
            ),
        ] {
            assert!(
                matches!(
                    suppression_evidence_from_row(&row),
                    Err(RadrootsEventStoreError::CurrentVisibilityDrift { ref reason })
                        if reason.contains(expected_context)
                ),
                "{label} corruption was accepted",
            );
        }

        for (label, row) in [
            (
                "missing reason",
                suppression_evidence_row(&mut connection, Some("visible"), None, None, None, None)
                    .await,
            ),
            (
                "orphan event request",
                suppression_evidence_row(
                    &mut connection,
                    None,
                    None,
                    Some(event_request_id.as_str()),
                    None,
                    None,
                )
                .await,
            ),
            (
                "orphan address request",
                suppression_evidence_row(
                    &mut connection,
                    None,
                    None,
                    None,
                    Some(address_request_id.as_str()),
                    None,
                )
                .await,
            ),
            (
                "orphan address cutoff",
                suppression_evidence_row(&mut connection, None, None, None, None, Some(7)).await,
            ),
        ] {
            assert!(
                matches!(
                    suppression_evidence_from_row(&row),
                    Err(RadrootsEventStoreError::CurrentVisibilityDrift { ref reason })
                        if reason == "suppression evidence is incomplete"
                ),
                "{label} authority was accepted",
            );
        }
    }

    #[tokio::test]
    async fn addressable_head_validator_requires_coordinate_and_state_authority() {
        let store = RadrootsEventStore::open_memory().await.expect("open store");
        let mut transaction = store.pool().begin().await.expect("transaction");
        let addressable = visibility(
            StoredEventClass::Addressable,
            RadrootsCurrentVisibilityDecisionV1::Visible,
            true,
            Some("a"),
            Some(visible_evidence()),
        );

        assert!(matches!(
            validate_addressable_head_projection(&mut transaction, &addressable, None).await,
            Err(RadrootsEventStoreError::CurrentVisibilityDrift { reason })
                if reason.contains("has no raw d tag")
        ));
        assert!(matches!(
            validate_addressable_head_projection(
                &mut transaction,
                &addressable,
                Some("missing-coordinate"),
            )
            .await,
            Err(RadrootsEventStoreError::CurrentVisibilityDrift { reason })
                if reason.contains("addressable head state is missing")
        ));
    }

    #[test]
    fn visibility_shape_accepts_each_decision_and_rejects_each_incoherence() {
        let regular_id = "a".repeat(64);
        validate_visibility_shape(&visibility(
            StoredEventClass::Regular,
            RadrootsCurrentVisibilityDecisionV1::Visible,
            true,
            None,
            Some(visible_evidence()),
        ))
        .expect("visible");

        let mut not_admitted = visibility(
            StoredEventClass::Regular,
            RadrootsCurrentVisibilityDecisionV1::NotAdmitted,
            true,
            None,
            None,
        );
        not_admitted.event.admission_status = RadrootsEventAdmissionStatus::Unsupported;
        not_admitted.event.contract_id = None;
        not_admitted.event.valid_stream_eligible = false;
        validate_visibility_shape(&not_admitted).expect("not admitted");
        let mut nonregular_not_admitted = visibility(
            StoredEventClass::Replaceable,
            RadrootsCurrentVisibilityDecisionV1::NotAdmitted,
            false,
            None,
            None,
        );
        nonregular_not_admitted.event.admission_status = RadrootsEventAdmissionStatus::Unsupported;
        nonregular_not_admitted.event.contract_id = None;
        nonregular_not_admitted.event.valid_stream_eligible = false;
        validate_visibility_shape(&nonregular_not_admitted).expect("nonregular not admitted");

        validate_visibility_shape(&visibility(
            StoredEventClass::Replaceable,
            RadrootsCurrentVisibilityDecisionV1::NotCurrent,
            false,
            Some("e"),
            Some(visible_evidence()),
        ))
        .expect("not current");
        validate_visibility_shape(&visibility(
            StoredEventClass::Regular,
            RadrootsCurrentVisibilityDecisionV1::Suppressed,
            true,
            None,
            Some(suppressed_evidence()),
        ))
        .expect("suppressed");

        assert!(
            validate_visibility_shape(&visibility(
                StoredEventClass::Ephemeral,
                RadrootsCurrentVisibilityDecisionV1::Visible,
                true,
                None,
                Some(visible_evidence()),
            ))
            .is_err()
        );
        for (is_raw_head, raw_head_event_id) in [(false, None), (true, Some("e"))] {
            assert!(
                validate_visibility_shape(&visibility(
                    StoredEventClass::Regular,
                    RadrootsCurrentVisibilityDecisionV1::Visible,
                    is_raw_head,
                    raw_head_event_id,
                    Some(visible_evidence()),
                ))
                .is_err()
            );
        }
        assert!(
            validate_visibility_shape(&visibility(
                StoredEventClass::Replaceable,
                RadrootsCurrentVisibilityDecisionV1::Visible,
                true,
                None,
                Some(visible_evidence()),
            ))
            .is_err()
        );
        assert!(
            validate_visibility_shape(&visibility(
                StoredEventClass::Replaceable,
                RadrootsCurrentVisibilityDecisionV1::Visible,
                false,
                None,
                Some(visible_evidence()),
            ))
            .is_err()
        );

        let incoherent = RadrootsNip09SuppressionEvidenceV1 {
            outcome: RadrootsNip09SuppressionOutcome::Visible,
            reason: RadrootsNip09SuppressionReason::EventIdReference,
            event_reference_request_id: None,
            address_reference_request_id: None,
            address_reference_cutoff: None,
        };
        assert!(
            validate_visibility_shape(&visibility(
                StoredEventClass::Regular,
                RadrootsCurrentVisibilityDecisionV1::Visible,
                true,
                None,
                Some(incoherent),
            ))
            .is_err()
        );

        let mut invalid = visibility(
            StoredEventClass::Regular,
            RadrootsCurrentVisibilityDecisionV1::Visible,
            true,
            None,
            None,
        );
        assert!(validate_visibility_shape(&invalid).is_err());
        invalid = not_admitted.clone();
        invalid.event.admission_status = RadrootsEventAdmissionStatus::Admitted;
        assert!(validate_visibility_shape(&invalid).is_err());
        invalid = visibility(
            StoredEventClass::Replaceable,
            RadrootsCurrentVisibilityDecisionV1::NotCurrent,
            false,
            Some("e"),
            None,
        );
        assert!(validate_visibility_shape(&invalid).is_err());
        invalid = visibility(
            StoredEventClass::Regular,
            RadrootsCurrentVisibilityDecisionV1::Suppressed,
            true,
            None,
            Some(visible_evidence()),
        );
        assert!(validate_visibility_shape(&invalid).is_err());

        assert_eq!(regular_id, invalid.event.event_id);
    }

    #[test]
    fn visibility_errors_preserve_context() {
        assert!(matches!(
            current_visibility_drift::<()>("fixture"),
            Err(RadrootsEventStoreError::CurrentVisibilityDrift { reason })
                if reason == "fixture"
        ));
        assert!(matches!(
            visibility_authority_error("fixture", "bad"),
            RadrootsEventStoreError::CurrentVisibilityDrift { reason }
                if reason == "fixture is invalid: bad"
        ));
    }
}
