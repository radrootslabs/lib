use crate::error::RadrootsEventStoreError;
use crate::model::reconciliation_v1::{
    RadrootsEventAdmissionStatus, RadrootsStoredRawEvent, RadrootsStoredRawEventHead,
    StoredEventClass,
};
use radroots_event::envelope::RadrootsEventKind;
use radroots_event::event_head::v1::RadrootsEventHeadCoordinate;
use radroots_event::ids::RadrootsPublicKey;
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
            .bind(pubkey.as_str())
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
            .bind(pubkey.as_str())
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
    let pubkey = RadrootsPublicKey::parse(event.pubkey.clone()).map_err(|_| inconsistent())?;
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
                pubkey: RadrootsPublicKey::parse(raw_head.pubkey.clone()).map_err(|_| {
                    RadrootsEventStoreError::StoredHeadInconsistent {
                        event_id: raw_head.event_id.clone(),
                    }
                })?,
            }
        }
        StoredEventClass::Addressable => RadrootsEventHeadCoordinate::Addressable {
            kind: raw_head.kind,
            pubkey: RadrootsPublicKey::parse(raw_head.pubkey.clone()).map_err(|_| {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn raw_event(
        kind: u32,
        event_class: StoredEventClass,
        tags_json: &str,
    ) -> RadrootsStoredRawEvent {
        RadrootsStoredRawEvent {
            seq: 1,
            event_id: "b".repeat(64),
            pubkey: "a".repeat(64),
            created_at: 7,
            kind,
            tags_json: tags_json.to_owned(),
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

    fn raw_head(event: &RadrootsStoredRawEvent) -> RadrootsStoredRawEventHead {
        RadrootsStoredRawEventHead {
            coordinate_type: event.event_class,
            kind: event.kind,
            pubkey: event.pubkey.clone(),
            d_tag: None,
            event_id: event.event_id.clone(),
            created_at: event.created_at,
            updated_at_ms: event.updated_at_ms,
        }
    }

    #[test]
    fn stored_event_coordinates_fail_closed_for_every_class_and_encoding() {
        let replaceable = raw_event(10_001, StoredEventClass::Replaceable, "[]");
        let coordinate =
            raw_head_coordinate_for_stored_event(&replaceable).expect("replaceable coordinate");
        assert!(matches!(
            coordinate,
            RadrootsEventHeadCoordinate::Replaceable { kind: 10_001, .. }
        ));

        let addressable = raw_event(
            30_001,
            StoredEventClass::Addressable,
            r#"[["d","opaque"],["d","ignored"]]"#,
        );
        assert!(matches!(
            raw_head_coordinate_for_stored_event(&addressable).expect("addressable coordinate"),
            RadrootsEventHeadCoordinate::Addressable { kind: 30_001, d_tag, .. }
                if d_tag == "opaque"
        ));
        let no_identifier = raw_event(30_001, StoredEventClass::Addressable, "[]");
        assert!(matches!(
            raw_head_coordinate_for_stored_event(&no_identifier).expect("empty d coordinate"),
            RadrootsEventHeadCoordinate::Addressable { d_tag, .. } if d_tag.is_empty()
        ));

        let mut invalid = addressable.clone();
        invalid.pubkey = "invalid".to_owned();
        assert!(raw_head_coordinate_for_stored_event(&invalid).is_err());
        invalid = addressable.clone();
        invalid.tags_json = "not JSON".to_owned();
        assert!(raw_head_coordinate_for_stored_event(&invalid).is_err());
        for event_class in [StoredEventClass::Regular, StoredEventClass::Ephemeral] {
            invalid = addressable.clone();
            invalid.event_class = event_class;
            assert!(raw_head_coordinate_for_stored_event(&invalid).is_err());
        }
    }

    #[test]
    fn raw_head_snapshot_validation_rejects_each_identity_drift() {
        let event = raw_event(10_001, StoredEventClass::Replaceable, "[]");
        let requested = raw_head_coordinate_for_stored_event(&event).expect("coordinate");
        let head = raw_head(&event);
        validate_raw_head_snapshot(&requested, &head, &event).expect("valid snapshot");

        let wrong_requested = RadrootsEventHeadCoordinate::Replaceable {
            kind: 10_002,
            pubkey: RadrootsPublicKey::parse(event.pubkey.clone()).expect("pubkey"),
        };
        assert!(validate_raw_head_snapshot(&wrong_requested, &head, &event).is_err());

        let mut mutated = head.clone();
        mutated.kind += 1;
        assert!(validate_raw_head_snapshot(&requested, &mutated, &event).is_err());
        mutated = head.clone();
        mutated.event_id = "d".repeat(64);
        assert!(validate_raw_head_snapshot(&requested, &mutated, &event).is_err());
        mutated = head.clone();
        mutated.created_at += 1;
        assert!(validate_raw_head_snapshot(&requested, &mutated, &event).is_err());
        mutated = head.clone();
        mutated.pubkey = "invalid".to_owned();
        assert!(validate_raw_head_snapshot(&requested, &mutated, &event).is_err());
        mutated = head.clone();
        mutated.d_tag = Some("unexpected".to_owned());
        assert!(validate_raw_head_snapshot(&requested, &mutated, &event).is_err());

        let addressable = raw_event(30_001, StoredEventClass::Addressable, r#"[["d","opaque"]]"#);
        let requested = raw_head_coordinate_for_stored_event(&addressable).expect("coordinate");
        let mut addressable_head = raw_head(&addressable);
        addressable_head.d_tag = Some("opaque".to_owned());
        validate_raw_head_snapshot(&requested, &addressable_head, &addressable)
            .expect("valid addressable snapshot");
        addressable_head.d_tag = None;
        assert!(validate_raw_head_snapshot(&requested, &addressable_head, &addressable).is_err());
        addressable_head.d_tag = Some("opaque".to_owned());
        addressable_head.pubkey = "invalid".to_owned();
        assert!(validate_raw_head_snapshot(&requested, &addressable_head, &addressable).is_err());

        mutated = head;
        mutated.coordinate_type = StoredEventClass::Regular;
        assert!(validate_raw_head_snapshot(&requested, &mutated, &addressable).is_err());
    }

    #[test]
    fn storage_integer_and_legacy_status_helpers_are_exact() {
        for legacy in [
            "supported",
            "unsupported_kind",
            "unsupported_shape",
            "ambiguous_shape",
        ] {
            assert!(is_legacy_contract_status(legacy));
        }
        assert!(!is_legacy_contract_status("admitted"));
        assert_eq!(u32_from_i64("fixture", 7).expect("u32"), 7);
        assert!(u32_from_i64("fixture", -1).is_err());
        assert!(u32_from_i64("fixture", i64::from(u32::MAX) + 1).is_err());
        assert_eq!(u64_from_i64("fixture", 7).expect("u64"), 7);
        assert!(u64_from_i64("fixture", -1).is_err());
    }

    #[tokio::test]
    async fn raw_head_join_rejects_a_missing_referenced_event() {
        let store = crate::RadrootsEventStore::open_memory()
            .await
            .expect("open store");
        let pubkey = "a".repeat(64);
        let event_id = "b".repeat(64);
        let mut connection = store.pool().acquire().await.expect("trusted connection");
        sqlx::query("DROP TRIGGER radroots_event_store_event_head_insert_guard")
            .execute(&mut *connection)
            .await
            .expect("trusted head guard removal");
        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(&mut *connection)
            .await
            .expect("disable trusted foreign-key enforcement");
        sqlx::query(
            "INSERT INTO event_envelope_head(coordinate_type, kind, pubkey, d_tag, event_id, created_at, updated_at_ms) VALUES ('replaceable', 10001, ?, NULL, ?, 7, 8)",
        )
        .bind(pubkey.as_str())
        .bind(event_id.as_str())
        .execute(&mut *connection)
        .await
        .expect("trusted orphan head insertion");
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&mut *connection)
            .await
            .expect("restore foreign-key enforcement");
        drop(connection);

        let coordinate = RadrootsEventHeadCoordinate::Replaceable {
            kind: 10_001,
            pubkey: RadrootsPublicKey::parse(pubkey).expect("pubkey"),
        };
        let mut transaction = store.pool().begin().await.expect("transaction");
        assert!(matches!(
            raw_head_snapshot_in_transaction(&mut transaction, &coordinate).await,
            Err(RadrootsEventStoreError::StoredHeadInconsistent {
                event_id: stored_event_id,
            }) if stored_event_id == event_id
        ));
    }

    #[tokio::test]
    async fn stored_raw_head_rejects_an_unknown_coordinate_type() {
        let store = crate::RadrootsEventStore::open_memory()
            .await
            .expect("open store");
        let row = sqlx::query("SELECT 'unknown' AS raw_head_coordinate_type")
            .fetch_one(store.pool())
            .await
            .expect("fixture row");

        assert!(matches!(
            stored_raw_head_from_joined_row(&row),
            Err(RadrootsEventStoreError::InvalidStoredEnum {
                field: "event_class",
                value,
            }) if value == "unknown"
        ));
    }
}
