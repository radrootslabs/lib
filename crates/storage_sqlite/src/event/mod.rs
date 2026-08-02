use radroots_event_codec::Codec;
use radroots_storage::{
    Error, EventStore,
    event::{
        AdmissionDisposition, AdmissionReceipt, AdmissionStage, BoxFuture, EventAdmission,
        EventCursor, EventId, EventPage, EventPosition, EventQuery, EventQueryBounds,
        EventSequence, SourceGeneration, StoredEventProvenance, StoredRawEvent,
        StoredVerifiedEvent, StoredVisibleEvent,
    },
    status::{EventStoreHealth, EventStoreMode, EventStoreStatus},
};
use sqlx::{QueryBuilder, Row, Sqlite, SqlitePool};

#[derive(Clone)]
pub struct SqliteStorage {
    pool: SqlitePool,
    generation: SourceGeneration,
    mode: EventStoreMode,
}

struct StoredEventRow {
    position: EventPosition,
    raw_json: String,
    stage: AdmissionStage,
}

impl SqliteStorage {
    #[allow(dead_code)] // Wired into the public open lifecycle in its ordered RCL checkpoint.
    pub(crate) const fn new(
        pool: SqlitePool,
        generation: SourceGeneration,
        mode: EventStoreMode,
    ) -> Self {
        Self {
            pool,
            generation,
            mode,
        }
    }

    pub(crate) const fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub(crate) const fn event_mode(&self) -> EventStoreMode {
        self.mode
    }

    async fn selected(
        &self,
        query: &EventQuery,
        minimum_stage: AdmissionStage,
    ) -> Result<(Vec<StoredEventRow>, Option<EventCursor>), Error> {
        self.validate_cursor(query)?;
        let after = query
            .bounds()
            .cursor()
            .map_or(0, |cursor| cursor.sequence().get());
        let fetch_limit = u64::from(query.bounds().limit()) + 1;
        let mut builder = QueryBuilder::<Sqlite>::new(
            "SELECT source_generation, source_sequence, signed_event, admission_stage \
             FROM radroots_runtime_events WHERE source_sequence > ",
        );
        builder.push_bind(i64_from_u64(after)?);
        if minimum_stage == AdmissionStage::Verified {
            builder.push(" AND admission_stage IN ('verified', 'visible')");
        } else if minimum_stage == AdmissionStage::Visible {
            builder.push(" AND admission_stage = 'visible'");
        }
        if !query.event_ids().is_empty() {
            builder.push(" AND event_id IN (");
            let mut separated = builder.separated(", ");
            for event_id in query.event_ids() {
                separated.push_bind(event_id.as_bytes().to_vec());
            }
            separated.push_unseparated(")");
        }
        builder.push(" ORDER BY source_sequence LIMIT ");
        builder.push_bind(i64_from_u64(fetch_limit)?);

        let rows = builder
            .build()
            .fetch_all(&self.pool)
            .await
            .map_err(map_backend)?;
        let mut decoded = rows
            .iter()
            .map(|row| self.decode_event_row(row))
            .collect::<Result<Vec<_>, _>>()?;
        let next = if decoded.len() > usize::from(query.bounds().limit()) {
            decoded.truncate(usize::from(query.bounds().limit()));
            decoded.last().map(|row| row.position)
        } else {
            None
        };
        Ok((decoded, next))
    }

    fn validate_cursor(&self, query: &EventQuery) -> Result<(), Error> {
        if query
            .bounds()
            .cursor()
            .is_some_and(|cursor| cursor.generation() != self.generation)
        {
            return Err(Error::SourceGenerationChanged);
        }
        Ok(())
    }

    fn decode_event_row(&self, row: &sqlx::sqlite::SqliteRow) -> Result<StoredEventRow, Error> {
        let generation = source_generation(row.try_get("source_generation").map_err(map_corrupt)?)?;
        if generation != self.generation {
            return Err(Error::CorruptStoredEvent);
        }
        let sequence = event_sequence(row.try_get("source_sequence").map_err(map_corrupt)?)?;
        let raw_json = String::from_utf8(row.try_get("signed_event").map_err(map_corrupt)?)
            .map_err(|_| Error::CorruptStoredEvent)?;
        Codec::decode_signed_event(raw_json.as_str()).map_err(|_| Error::CorruptStoredEvent)?;
        let stage = admission_stage(row.try_get("admission_stage").map_err(map_corrupt)?)?;
        Ok(StoredEventRow {
            position: EventPosition::new(generation, sequence),
            raw_json,
            stage,
        })
    }

    async fn store_provenance(
        transaction: &mut sqlx::Transaction<'_, Sqlite>,
        admission: &EventAdmission,
    ) -> Result<(), Error> {
        let provenance = admission.provenance();
        let cursor = provenance.cursor().map_or("", |cursor| cursor.as_str());
        sqlx::query(
            "INSERT OR IGNORE INTO radroots_runtime_event_provenance (
               event_id, transport_id, target_fingerprint, observed_at_unix_ms, cursor
             ) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(admission.event_id().as_bytes().as_slice())
        .bind(provenance.transport_id().as_str())
        .bind(provenance.target().as_str())
        .bind(i64_from_u64(provenance.observed_at_unix_ms())?)
        .bind(cursor)
        .execute(&mut **transaction)
        .await
        .map_err(map_backend)?;
        Ok(())
    }

    pub(crate) async fn admit_transaction(
        &self,
        transaction: &mut sqlx::Transaction<'_, Sqlite>,
        admission: EventAdmission,
    ) -> Result<AdmissionReceipt, Error> {
        let existing = sqlx::query(
            "SELECT source_generation, source_sequence, signed_event, admission_stage
             FROM radroots_runtime_events WHERE event_id = ?",
        )
        .bind(admission.event_id().as_bytes().as_slice())
        .fetch_optional(&mut **transaction)
        .await
        .map_err(map_backend)?;

        let (position, disposition) = if let Some(row) = existing {
            let stored = self.decode_event_row(&row)?;
            if stored.raw_json.as_bytes() != admission.event().raw_json().as_bytes() {
                return Err(Error::EventConflict);
            }
            if admission.stage() < stored.stage {
                return Err(Error::AdmissionRegression);
            }
            let disposition = if admission.stage() == stored.stage {
                AdmissionDisposition::Duplicate
            } else {
                sqlx::query(
                    "UPDATE radroots_runtime_events
                     SET admission_stage = ?, updated_at_unix_ms = MAX(updated_at_unix_ms, ?)
                     WHERE event_id = ?",
                )
                .bind(stage_name(admission.stage()))
                .bind(i64_from_u64(admission.provenance().observed_at_unix_ms())?)
                .bind(admission.event_id().as_bytes().as_slice())
                .execute(&mut **transaction)
                .await
                .map_err(map_backend)?;
                AdmissionDisposition::Advanced
            };
            (stored.position, disposition)
        } else {
            let next = sqlx::query_scalar::<_, i64>(
                "UPDATE radroots_runtime_source_generations
                 SET sequence_head = sequence_head + 1
                 WHERE generation = ? AND state = 'active'
                 RETURNING sequence_head",
            )
            .bind(self.generation.as_bytes().as_slice())
            .fetch_optional(&mut **transaction)
            .await
            .map_err(map_backend)?
            .ok_or(Error::SourceGenerationChanged)?;
            let sequence = event_sequence(next)?;
            let observed_at = i64_from_u64(admission.provenance().observed_at_unix_ms())?;
            sqlx::query(
                "INSERT INTO radroots_runtime_events (
                   source_generation, source_sequence, event_id, admission_stage,
                   signed_event, admitted_at_unix_ms, updated_at_unix_ms
                 ) VALUES (?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(self.generation.as_bytes().as_slice())
            .bind(next)
            .bind(admission.event_id().as_bytes().as_slice())
            .bind(stage_name(admission.stage()))
            .bind(admission.event().raw_json().as_bytes())
            .bind(observed_at)
            .bind(observed_at)
            .execute(&mut **transaction)
            .await
            .map_err(map_backend)?;
            (
                EventPosition::new(self.generation, sequence),
                AdmissionDisposition::Inserted,
            )
        };
        Self::store_provenance(transaction, &admission).await?;
        Ok(AdmissionReceipt::new(
            *admission.event_id(),
            position,
            admission.stage(),
            disposition,
        ))
    }
}

impl EventStore for SqliteStorage {
    fn status(&self) -> BoxFuture<'_, Result<EventStoreStatus, Error>> {
        Box::pin(async move {
            let row = sqlx::query(
                "SELECT
                   COUNT(*) AS raw_events,
                   COALESCE(SUM(CASE WHEN admission_stage IN ('verified', 'visible') THEN 1 ELSE 0 END), 0)
                     AS verified_events,
                   COALESCE(SUM(CASE WHEN admission_stage = 'visible' THEN 1 ELSE 0 END), 0)
                     AS visible_events
                 FROM radroots_runtime_events WHERE source_generation = ?",
            )
            .bind(self.generation.as_bytes().as_slice())
            .fetch_one(&self.pool)
            .await
            .map_err(map_backend)?;
            EventStoreStatus::new(
                self.generation,
                self.mode,
                EventStoreHealth::Available,
                u64_from_i64(row.try_get("raw_events").map_err(map_corrupt)?)?,
                u64_from_i64(row.try_get("verified_events").map_err(map_corrupt)?)?,
                u64_from_i64(row.try_get("visible_events").map_err(map_corrupt)?)?,
            )
        })
    }

    fn admit(&self, admission: EventAdmission) -> BoxFuture<'_, Result<AdmissionReceipt, Error>> {
        Box::pin(async move {
            if self.mode == EventStoreMode::ReadOnly {
                return Err(Error::BackendUnavailable);
            }
            let mut transaction = self
                .pool
                .begin_with("BEGIN IMMEDIATE")
                .await
                .map_err(map_backend)?;
            let receipt = self.admit_transaction(&mut transaction, admission).await?;
            transaction.commit().await.map_err(map_backend)?;
            Ok(receipt)
        })
    }

    fn query_raw(
        &self,
        query: EventQuery,
    ) -> BoxFuture<'_, Result<EventPage<StoredRawEvent>, Error>> {
        Box::pin(async move {
            let (rows, next) = self.selected(&query, AdmissionStage::Raw).await?;
            let items = rows
                .into_iter()
                .map(|row| {
                    let event = Codec::decode_signed_event(row.raw_json.as_str())
                        .map_err(|_| Error::CorruptStoredEvent)?;
                    Ok(StoredRawEvent::new(row.position, event, row.stage))
                })
                .collect::<Result<Vec<_>, Error>>()?;
            EventPage::new(self.generation, items, next, query.bounds())
        })
    }

    fn query_verified(
        &self,
        query: EventQuery,
    ) -> BoxFuture<'_, Result<EventPage<StoredVerifiedEvent>, Error>> {
        Box::pin(async move {
            let (rows, next) = self.selected(&query, AdmissionStage::Verified).await?;
            let items = rows
                .into_iter()
                .map(|row| {
                    let event = Codec::decode_signed_event(row.raw_json.as_str())
                        .map_err(|_| Error::CorruptStoredEvent)?;
                    Ok(StoredVerifiedEvent::new(row.position, event))
                })
                .collect::<Result<Vec<_>, Error>>()?;
            EventPage::new(self.generation, items, next, query.bounds())
        })
    }

    fn query_visible(
        &self,
        query: EventQuery,
    ) -> BoxFuture<'_, Result<EventPage<StoredVisibleEvent>, Error>> {
        Box::pin(async move {
            let (rows, next) = self.selected(&query, AdmissionStage::Visible).await?;
            let items = rows
                .into_iter()
                .map(|row| {
                    let event = Codec::decode_signed_event(row.raw_json.as_str())
                        .map_err(|_| Error::CorruptStoredEvent)?;
                    Ok(StoredVisibleEvent::new(row.position, event))
                })
                .collect::<Result<Vec<_>, Error>>()?;
            EventPage::new(self.generation, items, next, query.bounds())
        })
    }

    fn query_provenance(
        &self,
        event_id: EventId,
        bounds: EventQueryBounds,
    ) -> BoxFuture<'_, Result<EventPage<StoredEventProvenance>, Error>> {
        Box::pin(async move {
            if bounds
                .cursor()
                .is_some_and(|cursor| cursor.generation() != self.generation)
            {
                return Err(Error::SourceGenerationChanged);
            }
            let event_row = sqlx::query(
                "SELECT source_generation, source_sequence
                 FROM radroots_runtime_events WHERE event_id = ?",
            )
            .bind(event_id.as_bytes().as_slice())
            .fetch_optional(&self.pool)
            .await
            .map_err(map_backend)?
            .ok_or(Error::EventNotFound)?;
            let generation = source_generation(
                event_row
                    .try_get("source_generation")
                    .map_err(map_corrupt)?,
            )?;
            if generation != self.generation {
                return Err(Error::CorruptStoredEvent);
            }
            let position = EventPosition::new(
                generation,
                event_sequence(event_row.try_get("source_sequence").map_err(map_corrupt)?)?,
            );
            let after = bounds.cursor().map_or(0, |cursor| cursor.sequence().get());
            let items = if position.sequence().get() <= after {
                Vec::new()
            } else {
                let rows = sqlx::query(
                    "SELECT transport_id, target_fingerprint, observed_at_unix_ms, cursor
                     FROM radroots_runtime_event_provenance
                     WHERE event_id = ?
                     ORDER BY observed_at_unix_ms, transport_id, target_fingerprint, cursor
                     LIMIT ?",
                )
                .bind(event_id.as_bytes().as_slice())
                .bind(i64::from(bounds.limit()))
                .fetch_all(&self.pool)
                .await
                .map_err(map_backend)?;
                rows.iter()
                    .map(|row| {
                        let cursor = row.try_get::<String, _>("cursor").map_err(map_corrupt)?;
                        StoredEventProvenance::from_stored_parts(
                            position,
                            row.try_get::<String, _>("transport_id")
                                .map_err(map_corrupt)?
                                .as_str(),
                            row.try_get::<String, _>("target_fingerprint")
                                .map_err(map_corrupt)?
                                .as_str(),
                            u64_from_i64(row.try_get("observed_at_unix_ms").map_err(map_corrupt)?)?,
                            (!cursor.is_empty()).then_some(cursor.as_str()),
                        )
                    })
                    .collect::<Result<Vec<_>, Error>>()?
            };
            EventPage::new(self.generation, items, None, bounds)
        })
    }
}

const fn stage_name(stage: AdmissionStage) -> &'static str {
    match stage {
        AdmissionStage::Raw => "raw",
        AdmissionStage::Verified => "verified",
        AdmissionStage::Visible => "visible",
    }
}

fn admission_stage(value: String) -> Result<AdmissionStage, Error> {
    match value.as_str() {
        "raw" => Ok(AdmissionStage::Raw),
        "verified" => Ok(AdmissionStage::Verified),
        "visible" => Ok(AdmissionStage::Visible),
        _ => Err(Error::CorruptStoredEvent),
    }
}

fn source_generation(value: Vec<u8>) -> Result<SourceGeneration, Error> {
    SourceGeneration::new(value.try_into().map_err(|_| Error::CorruptStoredEvent)?)
        .map_err(|_| Error::CorruptStoredEvent)
}

fn event_sequence(value: i64) -> Result<EventSequence, Error> {
    EventSequence::new(u64_from_i64(value)?).map_err(|_| Error::CorruptStoredEvent)
}

fn i64_from_u64(value: u64) -> Result<i64, Error> {
    i64::try_from(value).map_err(|_| Error::CorruptStoredEvent)
}

fn u64_from_i64(value: i64) -> Result<u64, Error> {
    u64::try_from(value).map_err(|_| Error::CorruptStoredEvent)
}

fn map_backend(_: sqlx::Error) -> Error {
    Error::BackendUnavailable
}

fn map_corrupt(_: sqlx::Error) -> Error {
    Error::CorruptStoredEvent
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migration::runtime::{MIGRATIONS, migration_sql};
    use radroots_event::{
        SignedEvent,
        admission::{AdmissionPolicy, RawEvent, VisibilityPolicy, VisibleEvent},
        wire::Nip01EventWire,
    };
    use radroots_storage::event::EventQueryBounds;
    use radroots_transport::{
        Target, TransportId,
        source::{EventProvenance, FetchCursor, ObservedEvent},
    };
    use sqlx::sqlite::SqlitePoolOptions;

    struct Allow;

    impl radroots_event::admission::SignatureVerifier for Allow {
        fn verify_signature(
            &self,
            _event: &radroots_event::Event,
        ) -> Result<(), radroots_event::Error> {
            Ok(())
        }
    }

    impl AdmissionPolicy for Allow {
        type Error = core::convert::Infallible;

        fn policy_id(&self) -> &'static str {
            "test.storage-sqlite.admission.v1"
        }

        fn admit(
            &self,
            _event: &radroots_event::admission::ContractValidatedEvent,
        ) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    impl VisibilityPolicy for Allow {
        type Error = core::convert::Infallible;

        fn policy_id(&self) -> &'static str {
            "test.storage-sqlite.visibility.v1"
        }

        fn make_visible(
            &self,
            _event: &radroots_event::admission::AdmittedEvent,
        ) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    async fn store(generation: SourceGeneration) -> SqliteStorage {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("memory SQLite");
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .expect("foreign keys");
        for migration in MIGRATIONS {
            sqlx::raw_sql(migration_sql(migration.version()).expect("registered SQL"))
                .execute(&pool)
                .await
                .expect("runtime migration");
        }
        sqlx::query(
            "INSERT INTO radroots_runtime_source_generations (
               generation, sequence_head, state, created_at_unix_ms, retired_at_unix_ms
             ) VALUES (?, 0, 'active', 1, NULL)",
        )
        .bind(generation.as_bytes().as_slice())
        .execute(&pool)
        .await
        .expect("source generation");
        SqliteStorage::new(pool, generation, EventStoreMode::ReadWrite)
    }

    fn signed_event(content: &str, pretty: bool) -> SignedEvent {
        let mut wire = Nip01EventWire {
            id: "0".repeat(64),
            pubkey: "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df".to_owned(),
            created_at: 1_800_000_100,
            kind: 0,
            tags: vec![],
            content: content.to_owned(),
            sig: "42".repeat(64),
            extra: Default::default(),
        };
        wire.id = wire
            .computed_event_id()
            .expect("canonical event id")
            .to_hex();
        let value = serde_json::json!({
            "id": &wire.id,
            "pubkey": &wire.pubkey,
            "created_at": wire.created_at,
            "kind": wire.kind,
            "tags": &wire.tags,
            "content": &wire.content,
            "sig": &wire.sig,
        });
        let raw_json = if pretty {
            serde_json::to_string_pretty(&value).expect("pretty event JSON")
        } else {
            value.to_string()
        };
        SignedEvent::from_wire_verified_id(wire, raw_json).expect("signed event")
    }

    fn observed(event: SignedEvent, at: u64, cursor: Option<&str>) -> ObservedEvent {
        let target = Target::new(TransportId::NOSTR, "wss://relay.example").expect("target");
        let mut provenance =
            EventProvenance::new(TransportId::NOSTR, target.fingerprint().clone(), at)
                .expect("provenance");
        if let Some(cursor) = cursor {
            provenance = provenance.with_cursor(FetchCursor::parse(cursor).expect("cursor"));
        }
        ObservedEvent::new(event, provenance)
    }

    fn verified(event: &SignedEvent) -> radroots_event::VerifiedEvent {
        RawEvent::new(event.envelope().clone())
            .verify_id()
            .expect("event id")
            .verify_signature(&Allow)
            .expect("signature")
    }

    fn visible(event: &SignedEvent) -> VisibleEvent {
        verified(event)
            .validate_contract()
            .expect("contract")
            .admit_with(&Allow)
            .expect("admission")
            .make_visible_with(&Allow)
            .expect("visibility")
    }

    #[tokio::test]
    async fn admission_is_idempotent_monotonic_and_conflict_safe() {
        let generation = SourceGeneration::new([7; 32]).expect("generation");
        let store = store(generation).await;
        let event = signed_event(
            "{\"display_name\":\"Moss Street Farm\",\"bot\":false}",
            false,
        );

        let inserted = store
            .admit(EventAdmission::raw(observed(event.clone(), 10, None)))
            .await
            .expect("insert raw");
        assert_eq!(inserted.disposition(), AdmissionDisposition::Inserted);
        assert_eq!(inserted.position().sequence().get(), 1);

        let advanced = store
            .admit(
                EventAdmission::visible(
                    observed(event.clone(), 11, Some("relay-page-1")),
                    visible(&event),
                )
                .expect("visible admission"),
            )
            .await
            .expect("advance visible");
        assert_eq!(advanced.disposition(), AdmissionDisposition::Advanced);
        assert_eq!(advanced.position(), inserted.position());

        let duplicate = store
            .admit(
                EventAdmission::visible(
                    observed(event.clone(), 11, Some("relay-page-1")),
                    visible(&event),
                )
                .expect("visible admission"),
            )
            .await
            .expect("duplicate visible");
        assert_eq!(duplicate.disposition(), AdmissionDisposition::Duplicate);
        assert_eq!(
            store
                .admit(EventAdmission::raw(observed(event.clone(), 12, None)))
                .await,
            Err(Error::AdmissionRegression)
        );

        let same_id_different_bytes = signed_event(
            "{\"display_name\":\"Moss Street Farm\",\"bot\":false}",
            true,
        );
        assert_eq!(same_id_different_bytes.id(), event.id());
        assert_eq!(
            store
                .admit(EventAdmission::raw(observed(
                    same_id_different_bytes,
                    13,
                    None,
                )))
                .await,
            Err(Error::EventConflict)
        );
    }

    #[tokio::test]
    async fn queries_preserve_stage_bounds_cursors_and_exact_provenance() {
        let generation = SourceGeneration::new([8; 32]).expect("generation");
        let store = store(generation).await;
        let empty_status = store.status().await.expect("empty status");
        assert_eq!(empty_status.raw_events(), 0);
        assert_eq!(empty_status.verified_events(), 0);
        assert_eq!(empty_status.visible_events(), 0);
        let raw_event = signed_event("raw", false);
        let visible_event =
            signed_event("{\"display_name\":\"Visible Farm\",\"bot\":false}", false);
        store
            .admit(EventAdmission::raw(observed(raw_event.clone(), 20, None)))
            .await
            .expect("raw event");
        store
            .admit(
                EventAdmission::visible(
                    observed(visible_event.clone(), 21, Some("relay-page-2")),
                    visible(&visible_event),
                )
                .expect("visible admission"),
            )
            .await
            .expect("visible event");

        let first = store
            .query_raw(EventQuery::all(EventQueryBounds::first(1).expect("bounds")))
            .await
            .expect("first page");
        assert_eq!(first.items().len(), 1);
        assert_eq!(first.items()[0].event(), &raw_event);
        let next = first.next_cursor().expect("continuation cursor");
        let second = store
            .query_raw(EventQuery::all(
                EventQueryBounds::first(1).expect("bounds").after(next),
            ))
            .await
            .expect("second page");
        assert_eq!(second.items()[0].event(), &visible_event);
        assert!(second.next_cursor().is_none());

        let verified_page = store
            .query_verified(EventQuery::all(
                EventQueryBounds::first(10).expect("bounds"),
            ))
            .await
            .expect("verified page");
        assert_eq!(verified_page.items().len(), 1);
        assert_eq!(verified_page.items()[0].event(), &visible_event);
        let visible_page = store
            .query_visible(
                EventQuery::for_ids(
                    EventQueryBounds::first(10).expect("bounds"),
                    vec![*visible_event.id()],
                )
                .expect("id query"),
            )
            .await
            .expect("visible page");
        assert_eq!(visible_page.items().len(), 1);

        let provenance = store
            .query_provenance(
                *visible_event.id(),
                EventQueryBounds::first(10).expect("bounds"),
            )
            .await
            .expect("provenance");
        assert_eq!(provenance.items().len(), 1);
        assert_eq!(provenance.items()[0].provenance().observed_at_unix_ms(), 21);
        assert_eq!(
            provenance.items()[0]
                .provenance()
                .cursor()
                .expect("cursor")
                .as_str(),
            "relay-page-2"
        );

        let status = store.status().await.expect("status");
        assert_eq!(status.raw_events(), 2);
        assert_eq!(status.verified_events(), 1);
        assert_eq!(status.visible_events(), 1);
        let foreign_cursor = EventPosition::new(
            SourceGeneration::new([9; 32]).expect("foreign generation"),
            EventSequence::new(1).expect("sequence"),
        );
        assert_eq!(
            store
                .query_raw(EventQuery::all(
                    EventQueryBounds::first(1)
                        .expect("bounds")
                        .after(foreign_cursor),
                ))
                .await,
            Err(Error::SourceGenerationChanged)
        );
    }

    #[tokio::test]
    async fn corrupt_rows_fail_closed_and_source_history_is_immutable() {
        let generation = SourceGeneration::new([10; 32]).expect("generation");
        let store = store(generation).await;
        let event = signed_event("corruption", false);
        store
            .admit(EventAdmission::raw(observed(event, 30, None)))
            .await
            .expect("event");

        assert!(
            sqlx::query("DELETE FROM radroots_runtime_events")
                .execute(&store.pool)
                .await
                .is_err()
        );
        assert!(
            sqlx::query("DELETE FROM radroots_runtime_source_generations")
                .execute(&store.pool)
                .await
                .is_err()
        );
        sqlx::query("PRAGMA ignore_check_constraints = ON")
            .execute(&store.pool)
            .await
            .expect("disable checks for corruption probe");
        sqlx::query("UPDATE radroots_runtime_events SET admission_stage = 'corrupt'")
            .execute(&store.pool)
            .await
            .expect("forge corrupt stage");
        assert_eq!(
            store
                .query_raw(EventQuery::all(EventQueryBounds::first(1).expect("bounds"),))
                .await,
            Err(Error::CorruptStoredEvent)
        );
    }
}
