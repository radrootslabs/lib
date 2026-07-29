use crate::error::RadrootsEventStoreError;
use crate::model::reconciliation_v1::{
    RadrootsEventAdmissionStatus, RadrootsStoredRawEvent, RadrootsStoredRawEventHead,
    StoredEventClass,
};
use radroots_event::envelope::RadrootsEventKind;
use radroots_event::envelope::event_head::v1::RadrootsEventHeadCoordinate;
use radroots_identity::PublicKey;
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, Sqlite, Transaction};

pub(super) struct RawHeadSnapshot {
    pub(super) raw_head: RadrootsStoredRawEventHead,
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub(super) fn stored_raw_event_from_row(
    row: SqliteRow,
) -> Result<RadrootsStoredRawEvent, RadrootsEventStoreError> {
    let kind = u32_from_i64("kind", row.try_get("kind")?)?;
    let created_at = u64_from_i64("created_at", row.try_get("created_at")?)?;
    let event_id: String = row.try_get("event_id")?;
    let verification_status: String = row.try_get("verification_status")?;
    if verification_status != "verified" {
        return Err(RadrootsEventStoreError::StoredRawEventNotVerified {
            event_id,
            status: verification_status,
        });
    }
    let contract_status: String = row.try_get("contract_status")?;
    if is_legacy_contract_status(contract_status.as_str()) {
        return Err(
            RadrootsEventStoreError::StoredRawEventRequiresReconciliation {
                event_id,
                contract_status,
            },
        );
    }
    let admission_status = RadrootsEventAdmissionStatus::parse(contract_status.as_str())?;
    let event_class = row
        .try_get::<Option<String>, _>("event_class")?
        .ok_or_else(|| RadrootsEventStoreError::StoredRawEventMissingClass {
            event_id: event_id.clone(),
        })
        .and_then(|value| StoredEventClass::parse(value.as_str()))?;
    let projection_eligible: i64 = row.try_get("projection_eligible")?;
    let valid_stream_eligible = match projection_eligible {
        0 => false,
        1 => true,
        _ => {
            return Err(
                RadrootsEventStoreError::StoredRawEventClassificationInconsistent { event_id },
            );
        }
    };
    let contract_id: Option<String> = row.try_get("contract_id")?;
    if kind > u32::from(u16::MAX) {
        return Err(RadrootsEventStoreError::StoredRawEventClassificationInconsistent { event_id });
    }
    let expected_class =
        StoredEventClass::from_event_kind_class(RadrootsEventKind::new(kind).class());
    if expected_class == StoredEventClass::Ephemeral {
        return Err(RadrootsEventStoreError::StoredRawEventClassificationInconsistent { event_id });
    }
    let expected_eligible = admission_status == RadrootsEventAdmissionStatus::Admitted
        && expected_class != StoredEventClass::Ephemeral;
    let contract_id_is_consistent =
        (admission_status == RadrootsEventAdmissionStatus::Admitted) == contract_id.is_some();
    if event_class != expected_class
        || valid_stream_eligible != expected_eligible
        || !contract_id_is_consistent
    {
        return Err(RadrootsEventStoreError::StoredRawEventClassificationInconsistent { event_id });
    }
    Ok(RadrootsStoredRawEvent {
        seq: row.try_get("seq")?,
        event_id,
        pubkey: row.try_get("pubkey")?,
        created_at,
        kind,
        tags_json: row.try_get("tags_json")?,
        content: row.try_get("content")?,
        sig: row.try_get("sig")?,
        raw_json: row.try_get("raw_json")?,
        admission_status,
        contract_id,
        event_class,
        valid_stream_eligible,
        inserted_at_ms: row.try_get("inserted_at_ms")?,
        updated_at_ms: row.try_get("updated_at_ms")?,
    })
}

pub(super) async fn raw_head_snapshot_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    coordinate: &RadrootsEventHeadCoordinate,
) -> Result<Option<RawHeadSnapshot>, RadrootsEventStoreError> {
    let row = match coordinate {
        RadrootsEventHeadCoordinate::Replaceable { kind, pubkey } => {
            sqlx::query(
                "SELECT event.seq, event.event_id, event.pubkey, event.created_at, event.kind, event.tags_json, event.content, event.sig, event.raw_json, event.verification_status, event.contract_status, event.contract_id, event.event_class, event.projection_eligible, event.inserted_at_ms, event.updated_at_ms, head.coordinate_type AS raw_head_coordinate_type, head.kind AS raw_head_kind, head.pubkey AS raw_head_pubkey, head.d_tag AS raw_head_d_tag, head.event_id AS raw_head_event_id, head.created_at AS raw_head_created_at, head.updated_at_ms AS raw_head_updated_at_ms FROM event_envelope_head AS head LEFT JOIN event_envelopes AS event ON event.event_id = head.event_id WHERE head.coordinate_type = 'replaceable' AND head.kind = ? AND head.pubkey = ? AND head.d_tag IS NULL",
            )
            .bind(i64::from(*kind))
            .bind(pubkey.to_hex())
            .fetch_optional(&mut **tx)
            .await?
        }
        RadrootsEventHeadCoordinate::Addressable {
            kind,
            pubkey,
            d_tag,
        } => {
            sqlx::query(
                "SELECT event.seq, event.event_id, event.pubkey, event.created_at, event.kind, event.tags_json, event.content, event.sig, event.raw_json, event.verification_status, event.contract_status, event.contract_id, event.event_class, event.projection_eligible, event.inserted_at_ms, event.updated_at_ms, head.coordinate_type AS raw_head_coordinate_type, head.kind AS raw_head_kind, head.pubkey AS raw_head_pubkey, head.d_tag AS raw_head_d_tag, head.event_id AS raw_head_event_id, head.created_at AS raw_head_created_at, head.updated_at_ms AS raw_head_updated_at_ms FROM event_envelope_head AS head LEFT JOIN event_envelopes AS event ON event.event_id = head.event_id WHERE head.coordinate_type = 'addressable' AND head.kind = ? AND head.pubkey = ? AND head.d_tag = ?",
            )
            .bind(i64::from(*kind))
            .bind(pubkey.to_hex())
            .bind(d_tag.as_str())
            .fetch_optional(&mut **tx)
            .await?
        }
    };
    row.map(|row| {
        let raw_head = stored_raw_head_from_joined_row(&row)?;
        if row.try_get::<Option<String>, _>("event_id")?.is_none() {
            return Err(RadrootsEventStoreError::StoredHeadInconsistent {
                event_id: raw_head.event_id,
            });
        }
        let raw_event = stored_raw_event_from_row(row)?;
        validate_raw_head_snapshot(coordinate, &raw_head, &raw_event)?;
        Ok(RawHeadSnapshot { raw_head })
    })
    .transpose()
}

pub(super) fn raw_head_coordinate_for_stored_event(
    event: &RadrootsStoredRawEvent,
) -> Result<RadrootsEventHeadCoordinate, RadrootsEventStoreError> {
    let inconsistent = || RadrootsEventStoreError::StoredHeadInconsistent {
        event_id: event.event_id.clone(),
    };
    let pubkey = PublicKey::from_hex(&event.pubkey).map_err(|_| inconsistent())?;
    match event.event_class {
        StoredEventClass::Replaceable => Ok(RadrootsEventHeadCoordinate::Replaceable {
            kind: event.kind,
            pubkey,
        }),
        StoredEventClass::Addressable => {
            let tags: Vec<Vec<String>> =
                serde_json::from_str(event.tags_json.as_str()).map_err(|_| inconsistent())?;
            let d_tag = tags
                .iter()
                .find(|tag| tag.first().map(String::as_str) == Some("d"))
                .and_then(|tag| tag.get(1))
                .cloned()
                .unwrap_or_default();
            Ok(RadrootsEventHeadCoordinate::Addressable {
                kind: event.kind,
                pubkey,
                d_tag,
            })
        }
        StoredEventClass::Regular | StoredEventClass::Ephemeral => Err(inconsistent()),
    }
}

fn stored_raw_head_from_joined_row(
    row: &SqliteRow,
) -> Result<RadrootsStoredRawEventHead, RadrootsEventStoreError> {
    Ok(RadrootsStoredRawEventHead {
        coordinate_type: StoredEventClass::parse(
            row.try_get::<String, _>("raw_head_coordinate_type")?
                .as_str(),
        )?,
        kind: u32_from_i64("kind", row.try_get("raw_head_kind")?)?,
        pubkey: row.try_get("raw_head_pubkey")?,
        d_tag: row.try_get("raw_head_d_tag")?,
        event_id: row.try_get("raw_head_event_id")?,
        created_at: u64_from_i64("created_at", row.try_get("raw_head_created_at")?)?,
        updated_at_ms: row.try_get("raw_head_updated_at_ms")?,
    })
}

fn validate_raw_head_snapshot(
    requested_coordinate: &RadrootsEventHeadCoordinate,
    raw_head: &RadrootsStoredRawEventHead,
    raw_event: &RadrootsStoredRawEvent,
) -> Result<(), RadrootsEventStoreError> {
    let expected_coordinate = raw_head_coordinate_for_stored_event(raw_event)?;
    let stored_coordinate = match raw_head.coordinate_type {
        StoredEventClass::Replaceable if raw_head.d_tag.is_none() => {
            RadrootsEventHeadCoordinate::Replaceable {
                kind: raw_head.kind,
                pubkey: PublicKey::from_hex(&raw_head.pubkey).map_err(|_| {
                    RadrootsEventStoreError::StoredHeadInconsistent {
                        event_id: raw_head.event_id.clone(),
                    }
                })?,
            }
        }
        StoredEventClass::Addressable => RadrootsEventHeadCoordinate::Addressable {
            kind: raw_head.kind,
            pubkey: PublicKey::from_hex(&raw_head.pubkey).map_err(|_| {
                RadrootsEventStoreError::StoredHeadInconsistent {
                    event_id: raw_head.event_id.clone(),
                }
            })?,
            d_tag: raw_head.d_tag.clone().ok_or_else(|| {
                RadrootsEventStoreError::StoredHeadInconsistent {
                    event_id: raw_head.event_id.clone(),
                }
            })?,
        },
        _ => {
            return Err(RadrootsEventStoreError::StoredHeadInconsistent {
                event_id: raw_head.event_id.clone(),
            });
        }
    };
    if &stored_coordinate != requested_coordinate
        || stored_coordinate != expected_coordinate
        || raw_head.event_id != raw_event.event_id
        || raw_head.created_at != raw_event.created_at
    {
        return Err(RadrootsEventStoreError::StoredHeadInconsistent {
            event_id: raw_head.event_id.clone(),
        });
    }
    Ok(())
}

fn is_legacy_contract_status(value: &str) -> bool {
    matches!(
        value,
        "supported" | "unsupported_kind" | "unsupported_shape" | "ambiguous_shape"
    )
}

fn u32_from_i64(field: &'static str, value: i64) -> Result<u32, RadrootsEventStoreError> {
    u32::try_from(value).map_err(|_| RadrootsEventStoreError::IntegerRange { field, value })
}

fn u64_from_i64(field: &'static str, value: i64) -> Result<u64, RadrootsEventStoreError> {
    u64::try_from(value).map_err(|_| RadrootsEventStoreError::IntegerRange { field, value })
}
