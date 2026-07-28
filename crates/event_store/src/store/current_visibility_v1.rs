use super::protocol_storage_v1::stored_raw_event_from_row;
use super::{RadrootsEventStore, bool_from_i64};
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
            .is_none_or(|raw_head_event_id| raw_head_event_id.to_hex() != event_id)
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
            visibility.event.admission_status
                == crate::model::RadrootsEventAdmissionStatus::Admitted
                && visibility.is_raw_head
                && evidence
                    .is_some_and(|value| value.outcome == RadrootsNip09SuppressionOutcome::Visible)
        }
        RadrootsCurrentVisibilityDecisionV1::NotAdmitted => {
            visibility.event.admission_status
                != crate::model::RadrootsEventAdmissionStatus::Admitted
                && evidence.is_none()
        }
        RadrootsCurrentVisibilityDecisionV1::NotCurrent => {
            visibility.event.admission_status
                == crate::model::RadrootsEventAdmissionStatus::Admitted
                && !visibility.is_raw_head
                && visibility.raw_head_event_id.is_some()
                && evidence.is_some()
        }
        RadrootsCurrentVisibilityDecisionV1::Suppressed => {
            visibility.event.admission_status
                == crate::model::RadrootsEventAdmissionStatus::Admitted
                && visibility.is_raw_head
                && evidence.is_some_and(|value| {
                    value.outcome == RadrootsNip09SuppressionOutcome::Suppressed
                })
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
    if row.try_get::<String, _>("raw_head_event_id")? != visibility.event.event_id
        || row.try_get::<i64, _>("raw_head_event_seq")? != visibility.event.seq
        || row.try_get::<i64, _>("raw_head_created_at")?
            != i64::try_from(visibility.event.created_at).map_err(|_| {
                RadrootsEventStoreError::CurrentVisibilityDrift {
                    reason: format!(
                        "addressable event `{}` timestamp is outside SQLite range",
                        visibility.event.event_id
                    ),
                }
            })?
        || row.try_get::<String, _>("admission_status")?
            != visibility.event.admission_status.as_str()
        || row.try_get::<Option<String>, _>("admission_code")?
            != row.try_get::<Option<String>, _>("coordinate_admission_code")?
        || row.try_get::<Option<String>, _>("contract_id")? != visibility.event.contract_id
        || row.try_get::<String, _>("visibility")? != visibility.decision.as_str()
        || row
            .try_get::<Option<String>, _>("nip09_outcome")?
            .as_deref()
            != evidence.map(|value| value.outcome.code())
        || row.try_get::<Option<String>, _>("nip09_reason")?.as_deref()
            != evidence.map(|value| value.reason.code())
        || row.try_get::<Option<String>, _>("event_reference_request_id")?
            != evidence
                .and_then(|value| value.event_reference_request_id.as_ref())
                .map(RadrootsEventId::to_hex)
        || row.try_get::<Option<String>, _>("address_reference_request_id")?
            != evidence
                .and_then(|value| value.address_reference_request_id.as_ref())
                .map(RadrootsEventId::to_hex)
        || stored_cutoff != evidence.and_then(|value| value.address_reference_cutoff)
    {
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
