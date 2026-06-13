use crate::RadrootsEventStoreError;
use crate::migrations::{EVENT_STORE_MIGRATION_DOWN, EVENT_STORE_MIGRATION_UP};
use crate::model::{
    RadrootsEventContractStatus, RadrootsEventHeadStoreDecision, RadrootsEventIngest,
    RadrootsEventIngestReceipt, RadrootsEventVerificationStatus, RadrootsProjectionCursor,
    RadrootsRelayObservation, RadrootsStoredEvent, RadrootsStoredEventHead, RadrootsStoredEventTag,
    StoredEventClass, tag_semantic_name, tag_value_type_name,
};
use radroots_events::RadrootsNostrEvent;
use radroots_events::contract::{
    RadrootsEventClass, RadrootsEventContract, identify_event_contract,
};
use radroots_events::event_head::{
    RadrootsCurrentEventHead, RadrootsEventHeadCandidate, RadrootsEventHeadCandidateResult,
    RadrootsEventHeadCoordinate, RadrootsEventHeadDecision, event_head_candidate_for_contract,
    select_event_head,
};
use radroots_events::ids::{RadrootsEventId, RadrootsEventSignature, RadrootsPublicKey};
use radroots_nostr::prelude::{RadrootsNostrEventVerification, radroots_nostr_verify_event};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use std::path::Path;
use std::str::FromStr;

#[derive(Clone)]
pub struct RadrootsEventStore {
    pool: SqlitePool,
}

impl RadrootsEventStore {
    pub async fn open_memory() -> Result<Self, RadrootsEventStoreError> {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")?;
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        configure_connection(&pool, false).await?;
        apply_up(&pool).await?;
        Ok(Self { pool })
    }

    pub async fn open_file(path: impl AsRef<Path>) -> Result<Self, RadrootsEventStoreError> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        configure_connection(&pool, true).await?;
        apply_up(&pool).await?;
        Ok(Self { pool })
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn migrate_down(&self) -> Result<(), RadrootsEventStoreError> {
        apply_down(&self.pool).await
    }

    pub async fn pragma_foreign_keys(&self) -> Result<i64, RadrootsEventStoreError> {
        query_i64(&self.pool, "PRAGMA foreign_keys").await
    }

    pub async fn pragma_busy_timeout(&self) -> Result<i64, RadrootsEventStoreError> {
        query_i64(&self.pool, "PRAGMA busy_timeout").await
    }

    pub async fn pragma_journal_mode(&self) -> Result<String, RadrootsEventStoreError> {
        query_string(&self.pool, "PRAGMA journal_mode").await
    }

    pub async fn ingest_event(
        &self,
        ingest: RadrootsEventIngest,
    ) -> Result<RadrootsEventIngestReceipt, RadrootsEventStoreError> {
        validate_event_identity(&ingest.event)?;
        let verification_status = verify_event(&ingest.event);
        let classification = classify_event(&ingest.event);
        let raw_json = ingest
            .raw_json
            .clone()
            .map(Ok)
            .unwrap_or_else(|| serde_json::to_string(&ingest.event))?;
        let tags_json = serde_json::to_string(&ingest.event.tags)?;
        let mut tx = self.pool.begin().await?;
        let insert = insert_raw_event(
            &mut tx,
            &ingest,
            &classification,
            verification_status,
            raw_json.as_str(),
            tags_json.as_str(),
        )
        .await?;
        let inserted = insert.inserted;
        let mut head_decision = RadrootsEventHeadStoreDecision::Unsupported;
        let mut projection_eligible = classification.base_projection_eligible(verification_status);

        if inserted {
            insert_tags(&mut tx, &ingest.event, classification.contract).await?;
            if let Some(contract) = classification.contract {
                let head =
                    apply_event_head(&mut tx, &ingest.event, contract, ingest.observed_at_ms)
                        .await?;
                projection_eligible = projection_eligible && head.projection_eligible;
                head_decision = head.decision;
                sqlx::query(
                    "UPDATE nostr_event SET projection_eligible = ?, updated_at_ms = ? WHERE event_id = ?",
                )
                .bind(bool_i64(projection_eligible))
                .bind(ingest.observed_at_ms)
                .bind(ingest.event.id.as_str())
                .execute(&mut *tx)
                .await?;
            }
        } else if classification.contract.is_some() {
            head_decision = RadrootsEventHeadStoreDecision::SkippedDuplicate;
            projection_eligible = false;
        }

        if let Some(observation) = ingest.relay_observation.as_ref() {
            upsert_observation(&mut tx, ingest.event.id.as_str(), observation).await?;
        }

        tx.commit().await?;

        Ok(RadrootsEventIngestReceipt {
            seq: insert.seq,
            event_id: ingest.event.id,
            inserted,
            verification_status,
            contract_status: classification.contract_status,
            contract_id: classification
                .contract
                .map(|contract| contract.id.to_owned()),
            projection_eligible,
            head_decision,
        })
    }

    pub async fn get_event(
        &self,
        event_id: &str,
    ) -> Result<Option<RadrootsStoredEvent>, RadrootsEventStoreError> {
        let row = sqlx::query(
            "SELECT seq, event_id, pubkey, created_at, kind, tags_json, content, sig, raw_json, verification_status, contract_status, contract_id, event_class, projection_eligible, inserted_at_ms, updated_at_ms FROM nostr_event WHERE event_id = ?",
        )
        .bind(event_id)
        .fetch_optional(&self.pool)
        .await?;
        row.map(stored_event_from_row).transpose()
    }

    pub async fn tags_for_event(
        &self,
        event_id: &str,
    ) -> Result<Vec<RadrootsStoredEventTag>, RadrootsEventStoreError> {
        let rows = sqlx::query(
            "SELECT event_id, tag_index, tag_name, tag_value, tag_json, contract_semantic, contract_value_type, relay_indexed FROM nostr_event_tag WHERE event_id = ? ORDER BY tag_index",
        )
        .bind(event_id)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(stored_tag_from_row).collect()
    }

    pub async fn observations_for_event(
        &self,
        event_id: &str,
    ) -> Result<Vec<RadrootsRelayObservationRow>, RadrootsEventStoreError> {
        let rows = sqlx::query(
            "SELECT event_id, relay_url, observation_type, first_seen_at_ms, last_seen_at_ms, observation_count, last_message FROM relay_event_seen WHERE event_id = ? ORDER BY relay_url, observation_type",
        )
        .bind(event_id)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(relay_observation_from_row).collect()
    }

    pub async fn event_head(
        &self,
        coordinate: &RadrootsEventHeadCoordinate,
    ) -> Result<Option<RadrootsStoredEventHead>, RadrootsEventStoreError> {
        let row = match coordinate {
            RadrootsEventHeadCoordinate::Replaceable { kind, pubkey } => {
                sqlx::query(
                    "SELECT coordinate_type, kind, pubkey, d_tag, event_id, created_at, updated_at_ms FROM nostr_event_head WHERE coordinate_type = 'replaceable' AND kind = ? AND pubkey = ? AND d_tag IS NULL",
                )
                .bind(i64::from(*kind))
                .bind(pubkey.as_str())
                .fetch_optional(&self.pool)
                .await?
            }
            RadrootsEventHeadCoordinate::Addressable {
                kind,
                pubkey,
                d_tag,
            } => {
                sqlx::query(
                    "SELECT coordinate_type, kind, pubkey, d_tag, event_id, created_at, updated_at_ms FROM nostr_event_head WHERE coordinate_type = 'addressable' AND kind = ? AND pubkey = ? AND d_tag = ?",
                )
                .bind(i64::from(*kind))
                .bind(pubkey.as_str())
                .bind(d_tag.as_str())
                .fetch_optional(&self.pool)
                .await?
            }
        };
        row.map(stored_head_from_row).transpose()
    }

    pub async fn get_projection_cursor(
        &self,
        projection_id: &str,
    ) -> Result<Option<RadrootsProjectionCursor>, RadrootsEventStoreError> {
        let row = sqlx::query(
            "SELECT projection_id, projection_version, last_event_seq, updated_at_ms FROM projection_cursor WHERE projection_id = ?",
        )
        .bind(projection_id)
        .fetch_optional(&self.pool)
        .await?;
        row.map(projection_cursor_from_row).transpose()
    }

    pub async fn update_projection_cursor(
        &self,
        cursor: &RadrootsProjectionCursor,
    ) -> Result<(), RadrootsEventStoreError> {
        sqlx::query(
            "INSERT INTO projection_cursor(projection_id, projection_version, last_event_seq, updated_at_ms) VALUES (?, ?, ?, ?) ON CONFLICT(projection_id) DO UPDATE SET projection_version = excluded.projection_version, last_event_seq = excluded.last_event_seq, updated_at_ms = excluded.updated_at_ms",
        )
        .bind(cursor.projection_id.as_str())
        .bind(i64::from(cursor.projection_version))
        .bind(cursor.last_event_seq)
        .bind(cursor.updated_at_ms)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn events_since_cursor(
        &self,
        projection_id: &str,
        limit: u32,
    ) -> Result<Vec<RadrootsStoredEvent>, RadrootsEventStoreError> {
        let cursor = self.get_projection_cursor(projection_id).await?;
        let last_event_seq = cursor
            .as_ref()
            .map(|cursor| cursor.last_event_seq)
            .unwrap_or(0);
        let rows = sqlx::query(
            "SELECT seq, event_id, pubkey, created_at, kind, tags_json, content, sig, raw_json, verification_status, contract_status, contract_id, event_class, projection_eligible, inserted_at_ms, updated_at_ms FROM nostr_event WHERE projection_eligible = 1 AND seq > ? ORDER BY seq ASC LIMIT ?",
        )
        .bind(last_event_seq)
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(stored_event_from_row).collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsRelayObservationRow {
    pub event_id: String,
    pub relay_url: String,
    pub observation_type: String,
    pub first_seen_at_ms: i64,
    pub last_seen_at_ms: i64,
    pub observation_count: i64,
    pub last_message: Option<String>,
}

struct EventClassification {
    contract_status: RadrootsEventContractStatus,
    contract: Option<&'static RadrootsEventContract>,
}

impl EventClassification {
    fn base_projection_eligible(&self, verification: RadrootsEventVerificationStatus) -> bool {
        verification == RadrootsEventVerificationStatus::Verified
            && self
                .contract
                .map(|contract| contract.class != RadrootsEventClass::Ephemeral)
                .unwrap_or(false)
    }
}

struct AppliedHead {
    decision: RadrootsEventHeadStoreDecision,
    projection_eligible: bool,
}

struct InsertRawEventResult {
    inserted: bool,
    seq: i64,
}

async fn configure_connection(
    pool: &SqlitePool,
    file_backed: bool,
) -> Result<(), RadrootsEventStoreError> {
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(pool)
        .await?;
    sqlx::query("PRAGMA busy_timeout = 5000")
        .execute(pool)
        .await?;
    if file_backed {
        sqlx::query("PRAGMA journal_mode = WAL")
            .execute(pool)
            .await?;
    }
    Ok(())
}

async fn apply_up(pool: &SqlitePool) -> Result<(), RadrootsEventStoreError> {
    sqlx::raw_sql(EVENT_STORE_MIGRATION_UP)
        .execute(pool)
        .await?;
    Ok(())
}

async fn apply_down(pool: &SqlitePool) -> Result<(), RadrootsEventStoreError> {
    sqlx::raw_sql(EVENT_STORE_MIGRATION_DOWN)
        .execute(pool)
        .await?;
    Ok(())
}

async fn query_i64(pool: &SqlitePool, sql: &str) -> Result<i64, RadrootsEventStoreError> {
    let row = sqlx::query(sql).fetch_one(pool).await?;
    Ok(row.try_get(0)?)
}

async fn query_string(pool: &SqlitePool, sql: &str) -> Result<String, RadrootsEventStoreError> {
    let row = sqlx::query(sql).fetch_one(pool).await?;
    Ok(row.try_get(0)?)
}

fn validate_event_identity(event: &RadrootsNostrEvent) -> Result<(), RadrootsEventStoreError> {
    RadrootsEventId::parse(event.id.as_str())?;
    RadrootsPublicKey::parse(event.author.as_str())?;
    RadrootsEventSignature::parse(event.sig.as_str())?;
    Ok(())
}

fn classify_event(event: &RadrootsNostrEvent) -> EventClassification {
    match identify_event_contract(event.kind, &event.tags, &event.content) {
        Ok(contract) => EventClassification {
            contract_status: RadrootsEventContractStatus::Supported,
            contract: Some(contract),
        },
        Err(error) => EventClassification {
            contract_status: RadrootsEventContractStatus::from_match_error(error),
            contract: None,
        },
    }
}

fn verify_event(event: &RadrootsNostrEvent) -> RadrootsEventVerificationStatus {
    match radroots_nostr_verify_event(event) {
        RadrootsNostrEventVerification::Verified => RadrootsEventVerificationStatus::Verified,
        RadrootsNostrEventVerification::IdVerified => RadrootsEventVerificationStatus::IdVerified,
        RadrootsNostrEventVerification::IdMismatch => RadrootsEventVerificationStatus::IdMismatch,
        RadrootsNostrEventVerification::SignatureInvalid => {
            RadrootsEventVerificationStatus::SignatureInvalid
        }
        RadrootsNostrEventVerification::MalformedEnvelope => {
            RadrootsEventVerificationStatus::MalformedEnvelope
        }
    }
}

async fn insert_raw_event(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    ingest: &RadrootsEventIngest,
    classification: &EventClassification,
    verification_status: RadrootsEventVerificationStatus,
    raw_json: &str,
    tags_json: &str,
) -> Result<InsertRawEventResult, RadrootsEventStoreError> {
    let event = &ingest.event;
    let contract_id = classification.contract.map(|contract| contract.id);
    let event_class = classification
        .contract
        .map(|contract| StoredEventClass::from_event_class(contract.class).as_str());
    let projection_eligible = classification.base_projection_eligible(verification_status);
    let result = sqlx::query(
        "INSERT OR IGNORE INTO nostr_event(event_id, pubkey, created_at, kind, tags_json, content, sig, raw_json, verification_status, contract_status, contract_id, event_class, projection_eligible, inserted_at_ms, updated_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(event.id.as_str())
    .bind(event.author.as_str())
    .bind(i64::from(event.created_at))
    .bind(i64::from(event.kind))
    .bind(tags_json)
    .bind(event.content.as_str())
    .bind(event.sig.as_str())
    .bind(raw_json)
    .bind(verification_status.as_str())
    .bind(classification.contract_status.as_str())
    .bind(contract_id)
    .bind(event_class)
    .bind(bool_i64(projection_eligible))
    .bind(ingest.observed_at_ms)
    .bind(ingest.observed_at_ms)
    .execute(&mut **tx)
    .await?;
    let inserted = result.rows_affected() > 0;
    let seq = event_seq(tx, event.id.as_str()).await?;
    Ok(InsertRawEventResult { inserted, seq })
}

async fn event_seq(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    event_id: &str,
) -> Result<i64, RadrootsEventStoreError> {
    let row = sqlx::query("SELECT seq FROM nostr_event WHERE event_id = ?")
        .bind(event_id)
        .fetch_one(&mut **tx)
        .await?;
    row.try_get("seq").map_err(Into::into)
}

async fn insert_tags(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    event: &RadrootsNostrEvent,
    contract: Option<&'static RadrootsEventContract>,
) -> Result<(), RadrootsEventStoreError> {
    for (index, tag) in event.tags.iter().enumerate() {
        let tag_name = tag.first().map(String::as_str).unwrap_or("");
        let tag_value = tag.get(1).map(String::as_str);
        let tag_json = serde_json::to_string(tag)?;
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
            "INSERT INTO nostr_event_tag(event_id, tag_index, tag_name, tag_value, tag_json, contract_semantic, contract_value_type, relay_indexed) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(event.id.as_str())
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

async fn upsert_observation(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    event_id: &str,
    observation: &RadrootsRelayObservation,
) -> Result<(), RadrootsEventStoreError> {
    sqlx::query(
        "INSERT INTO relay_event_seen(event_id, relay_url, observation_type, first_seen_at_ms, last_seen_at_ms, observation_count, last_message) VALUES (?, ?, ?, ?, ?, 1, ?) ON CONFLICT(event_id, relay_url, observation_type) DO UPDATE SET last_seen_at_ms = excluded.last_seen_at_ms, observation_count = relay_event_seen.observation_count + 1, last_message = excluded.last_message",
    )
    .bind(event_id)
    .bind(observation.relay_url.as_str())
    .bind(observation.observation_type.as_str())
    .bind(observation.observed_at_ms)
    .bind(observation.observed_at_ms)
    .bind(observation.message.as_deref())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn apply_event_head(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    event: &RadrootsNostrEvent,
    contract: &RadrootsEventContract,
    updated_at_ms: i64,
) -> Result<AppliedHead, RadrootsEventStoreError> {
    let candidate = match event_head_candidate_for_contract(event, contract) {
        RadrootsEventHeadCandidateResult::Candidate(candidate) => candidate,
        RadrootsEventHeadCandidateResult::NotHeadSelected => {
            return Ok(AppliedHead {
                decision: RadrootsEventHeadStoreDecision::NotHeadSelected,
                projection_eligible: true,
            });
        }
        RadrootsEventHeadCandidateResult::NotPersisted => {
            return Ok(AppliedHead {
                decision: RadrootsEventHeadStoreDecision::NotPersisted,
                projection_eligible: false,
            });
        }
        RadrootsEventHeadCandidateResult::Malformed(_) => {
            return Ok(AppliedHead {
                decision: RadrootsEventHeadStoreDecision::Malformed,
                projection_eligible: false,
            });
        }
    };
    let current = current_event_head(tx, &candidate.coordinate).await?;
    let protocol_decision = select_event_head(candidate.clone(), current.as_ref());
    if let RadrootsEventHeadDecision::Applied(head) = &protocol_decision {
        upsert_head(tx, &candidate, head, updated_at_ms).await?;
    }
    let projection_eligible = matches!(protocol_decision, RadrootsEventHeadDecision::Applied(_));
    Ok(AppliedHead {
        decision: RadrootsEventHeadStoreDecision::from_protocol(&protocol_decision),
        projection_eligible,
    })
}

async fn current_event_head(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    coordinate: &RadrootsEventHeadCoordinate,
) -> Result<Option<RadrootsCurrentEventHead>, RadrootsEventStoreError> {
    let row = match coordinate {
        RadrootsEventHeadCoordinate::Replaceable { kind, pubkey } => {
            sqlx::query(
                "SELECT event_id, created_at FROM nostr_event_head WHERE coordinate_type = 'replaceable' AND kind = ? AND pubkey = ? AND d_tag IS NULL",
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
                "SELECT event_id, created_at FROM nostr_event_head WHERE coordinate_type = 'addressable' AND kind = ? AND pubkey = ? AND d_tag = ?",
            )
            .bind(i64::from(*kind))
            .bind(pubkey.as_str())
            .bind(d_tag.as_str())
            .fetch_optional(&mut **tx)
            .await?
        }
    };
    row.map(|row| {
        let event_id: String = row.try_get("event_id")?;
        let created_at: i64 = row.try_get("created_at")?;
        Ok(RadrootsCurrentEventHead {
            coordinate: coordinate.clone(),
            event_id: RadrootsEventId::parse(event_id)?,
            created_at: u32_from_i64("created_at", created_at)?,
        })
    })
    .transpose()
}

async fn upsert_head(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    candidate: &RadrootsEventHeadCandidate,
    head: &RadrootsCurrentEventHead,
    updated_at_ms: i64,
) -> Result<(), RadrootsEventStoreError> {
    match &head.coordinate {
        RadrootsEventHeadCoordinate::Replaceable { kind, pubkey } => {
            sqlx::query(
                "DELETE FROM nostr_event_head WHERE coordinate_type = 'replaceable' AND kind = ? AND pubkey = ? AND d_tag IS NULL",
            )
            .bind(i64::from(*kind))
            .bind(pubkey.as_str())
            .execute(&mut **tx)
            .await?;
            sqlx::query(
                "INSERT INTO nostr_event_head(coordinate_type, kind, pubkey, d_tag, event_id, created_at, updated_at_ms) VALUES ('replaceable', ?, ?, NULL, ?, ?, ?)",
            )
            .bind(i64::from(*kind))
            .bind(pubkey.as_str())
            .bind(candidate.event_id.as_str())
            .bind(i64::from(candidate.created_at))
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
                "DELETE FROM nostr_event_head WHERE coordinate_type = 'addressable' AND kind = ? AND pubkey = ? AND d_tag = ?",
            )
            .bind(i64::from(*kind))
            .bind(pubkey.as_str())
            .bind(d_tag.as_str())
            .execute(&mut **tx)
            .await?;
            sqlx::query(
                "INSERT INTO nostr_event_head(coordinate_type, kind, pubkey, d_tag, event_id, created_at, updated_at_ms) VALUES ('addressable', ?, ?, ?, ?, ?, ?)",
            )
            .bind(i64::from(*kind))
            .bind(pubkey.as_str())
            .bind(d_tag.as_str())
            .bind(candidate.event_id.as_str())
            .bind(i64::from(candidate.created_at))
            .bind(updated_at_ms)
            .execute(&mut **tx)
            .await?;
        }
    }
    Ok(())
}

fn stored_event_from_row(
    row: sqlx::sqlite::SqliteRow,
) -> Result<RadrootsStoredEvent, RadrootsEventStoreError> {
    let kind = u32_from_i64("kind", row.try_get("kind")?)?;
    let created_at = u32_from_i64("created_at", row.try_get("created_at")?)?;
    let verification_status =
        RadrootsEventVerificationStatus::parse(row.try_get("verification_status")?)?;
    let contract_status =
        RadrootsEventContractStatus::parse(row.try_get("contract_status")?, kind)?;
    let event_class = row
        .try_get::<Option<String>, _>("event_class")?
        .map(|value| StoredEventClass::parse(value.as_str()))
        .transpose()?;
    let projection_eligible = row.try_get::<i64, _>("projection_eligible")? != 0;
    Ok(RadrootsStoredEvent {
        seq: row.try_get("seq")?,
        event_id: row.try_get("event_id")?,
        pubkey: row.try_get("pubkey")?,
        created_at,
        kind,
        tags_json: row.try_get("tags_json")?,
        content: row.try_get("content")?,
        sig: row.try_get("sig")?,
        raw_json: row.try_get("raw_json")?,
        verification_status,
        contract_status,
        contract_id: row.try_get("contract_id")?,
        event_class,
        projection_eligible,
        inserted_at_ms: row.try_get("inserted_at_ms")?,
        updated_at_ms: row.try_get("updated_at_ms")?,
    })
}

fn stored_tag_from_row(
    row: sqlx::sqlite::SqliteRow,
) -> Result<RadrootsStoredEventTag, RadrootsEventStoreError> {
    Ok(RadrootsStoredEventTag {
        event_id: row.try_get("event_id")?,
        tag_index: u32_from_i64("tag_index", row.try_get("tag_index")?)?,
        tag_name: row.try_get("tag_name")?,
        tag_value: row.try_get("tag_value")?,
        tag_json: row.try_get("tag_json")?,
        contract_semantic: row.try_get("contract_semantic")?,
        contract_value_type: row.try_get("contract_value_type")?,
        relay_indexed: row.try_get::<i64, _>("relay_indexed")? != 0,
    })
}

fn stored_head_from_row(
    row: sqlx::sqlite::SqliteRow,
) -> Result<RadrootsStoredEventHead, RadrootsEventStoreError> {
    Ok(RadrootsStoredEventHead {
        coordinate_type: StoredEventClass::parse(row.try_get("coordinate_type")?)?,
        kind: u32_from_i64("kind", row.try_get("kind")?)?,
        pubkey: row.try_get("pubkey")?,
        d_tag: row.try_get("d_tag")?,
        event_id: row.try_get("event_id")?,
        created_at: u32_from_i64("created_at", row.try_get("created_at")?)?,
        updated_at_ms: row.try_get("updated_at_ms")?,
    })
}

fn projection_cursor_from_row(
    row: sqlx::sqlite::SqliteRow,
) -> Result<RadrootsProjectionCursor, RadrootsEventStoreError> {
    Ok(RadrootsProjectionCursor {
        projection_id: row.try_get("projection_id")?,
        projection_version: u32_from_i64("projection_version", row.try_get("projection_version")?)?,
        last_event_seq: row.try_get("last_event_seq")?,
        updated_at_ms: row.try_get("updated_at_ms")?,
    })
}

fn relay_observation_from_row(
    row: sqlx::sqlite::SqliteRow,
) -> Result<RadrootsRelayObservationRow, RadrootsEventStoreError> {
    Ok(RadrootsRelayObservationRow {
        event_id: row.try_get("event_id")?,
        relay_url: row.try_get("relay_url")?,
        observation_type: row.try_get("observation_type")?,
        first_seen_at_ms: row.try_get("first_seen_at_ms")?,
        last_seen_at_ms: row.try_get("last_seen_at_ms")?,
        observation_count: row.try_get("observation_count")?,
        last_message: row.try_get("last_message")?,
    })
}

fn u32_from_i64(field: &'static str, value: i64) -> Result<u32, RadrootsEventStoreError> {
    u32::try_from(value).map_err(|_| RadrootsEventStoreError::IntegerRange { field, value })
}

fn bool_i64(value: bool) -> i64 {
    if value { 1 } else { 0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use radroots_events::event_head::event_head_candidate_for_event;
    use radroots_events::kinds::{KIND_LISTING, KIND_ORDER_REQUEST, KIND_POST, KIND_PROFILE};
    use radroots_nostr::prelude::{
        RadrootsNostrKeys, RadrootsNostrSecretKey, RadrootsNostrTimestamp,
        radroots_event_from_nostr, radroots_nostr_build_event,
    };

    const FIXTURE_ALICE_SECRET_KEY_HEX: &str =
        "10c5304d6c9ae3a1a16f7860f1cc8f5e3a76225a2663b3a989a0d775919b7df5";
    const FIXTURE_ALICE_PUBLIC_KEY_HEX: &str =
        "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df";

    fn fixture_keys() -> RadrootsNostrKeys {
        let secret_key =
            RadrootsNostrSecretKey::from_hex(FIXTURE_ALICE_SECRET_KEY_HEX).expect("secret key");
        RadrootsNostrKeys::new(secret_key)
    }

    fn event_id(character: char) -> String {
        core::iter::repeat_n(character, 64).collect()
    }

    fn signed_event(
        kind: u32,
        created_at: u32,
        tags: Vec<Vec<String>>,
        content: &str,
    ) -> RadrootsNostrEvent {
        let raw_event = radroots_nostr_build_event(kind, content, tags)
            .expect("builder")
            .custom_created_at(RadrootsNostrTimestamp::from_secs(u64::from(created_at)))
            .sign_with_keys(&fixture_keys())
            .expect("signed event");
        radroots_event_from_nostr(&raw_event)
    }

    fn tamper_signature(event: &mut RadrootsNostrEvent) {
        let replacement = if event.sig.starts_with('0') { "1" } else { "0" };
        event.sig.replace_range(0..1, replacement);
    }

    #[test]
    fn verification_status_values_round_trip() {
        for status in [
            RadrootsEventVerificationStatus::NotChecked,
            RadrootsEventVerificationStatus::IdVerified,
            RadrootsEventVerificationStatus::Verified,
            RadrootsEventVerificationStatus::IdMismatch,
            RadrootsEventVerificationStatus::SignatureInvalid,
            RadrootsEventVerificationStatus::MalformedEnvelope,
        ] {
            assert_eq!(
                RadrootsEventVerificationStatus::parse(status.as_str()).expect("status"),
                status
            );
        }
        assert!(RadrootsEventVerificationStatus::parse("invalid").is_err());
    }

    #[tokio::test]
    async fn constructor_enforces_sqlite_pragmas() {
        let store = RadrootsEventStore::open_memory().await.expect("open");

        assert_eq!(store.pragma_foreign_keys().await.expect("foreign_keys"), 1);
        assert_eq!(
            store.pragma_busy_timeout().await.expect("busy_timeout"),
            5000
        );
    }

    #[tokio::test]
    async fn migration_can_run_down() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        store.migrate_down().await.expect("down");

        let missing = match sqlx::query("SELECT COUNT(*) FROM nostr_event")
            .fetch_one(store.pool())
            .await
        {
            Ok(_) => panic!("table should be removed"),
            Err(error) => error,
        };
        assert!(missing.to_string().contains("nostr_event"));
    }

    #[tokio::test]
    async fn ingest_retains_raw_event_and_ignores_duplicate_rows() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let event = signed_event(
            KIND_POST,
            10,
            vec![vec!["t".to_owned(), "soil".to_owned()]],
            "hello",
        );
        let ingest =
            RadrootsEventIngest::new(event.clone(), 1_000).with_raw_json("{\"fixture\":true}");

        let first = store
            .ingest_event(ingest.clone())
            .await
            .expect("first ingest");
        let second = store.ingest_event(ingest).await.expect("second ingest");
        let stored = store
            .get_event(event.id.as_str())
            .await
            .expect("get")
            .expect("stored");

        assert!(first.inserted);
        assert!(!second.inserted);
        assert_eq!(first.seq, second.seq);
        assert_eq!(
            first.verification_status,
            RadrootsEventVerificationStatus::Verified
        );
        assert_eq!(stored.seq, first.seq);
        assert_eq!(stored.raw_json, "{\"fixture\":true}");
        assert_eq!(stored.content, "hello");
        assert_eq!(stored.tags_json, "[[\"t\",\"soil\"]]");
        assert_eq!(
            stored.contract_status,
            RadrootsEventContractStatus::Supported
        );
        assert!(stored.projection_eligible);
        assert_eq!(
            store
                .tags_for_event(event.id.as_str())
                .await
                .expect("tags")
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn unsupported_verified_events_are_stored_but_not_projected() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let event = signed_event(999, 11, Vec::new(), "unsupported");
        let receipt = store
            .ingest_event(RadrootsEventIngest::new(event.clone(), 2_000))
            .await
            .expect("ingest");
        let stored = store
            .get_event(event.id.as_str())
            .await
            .expect("get")
            .expect("stored");

        assert_eq!(
            receipt.contract_status,
            RadrootsEventContractStatus::UnsupportedKind(999)
        );
        assert_eq!(
            stored.verification_status,
            RadrootsEventVerificationStatus::Verified
        );
        assert!(!stored.projection_eligible);
    }

    #[tokio::test]
    async fn id_mismatch_events_are_stored_but_not_projected() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let mut event = signed_event(KIND_POST, 12, Vec::new(), "hello");
        event.content = "tampered".to_owned();
        let receipt = store
            .ingest_event(RadrootsEventIngest::new(event.clone(), 2_100))
            .await
            .expect("ingest");
        let stored = store
            .get_event(event.id.as_str())
            .await
            .expect("get")
            .expect("stored");

        assert_eq!(
            receipt.contract_status,
            RadrootsEventContractStatus::Supported
        );
        assert_eq!(
            receipt.verification_status,
            RadrootsEventVerificationStatus::IdMismatch
        );
        assert_eq!(
            stored.verification_status,
            RadrootsEventVerificationStatus::IdMismatch
        );
        assert!(!stored.projection_eligible);
        assert!(
            store
                .events_since_cursor("social", 10)
                .await
                .expect("events")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn signature_invalid_events_are_stored_but_not_projected() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let mut event = signed_event(KIND_POST, 13, Vec::new(), "hello");
        tamper_signature(&mut event);
        let receipt = store
            .ingest_event(RadrootsEventIngest::new(event.clone(), 2_200))
            .await
            .expect("ingest");
        let stored = store
            .get_event(event.id.as_str())
            .await
            .expect("get")
            .expect("stored");

        assert_eq!(
            receipt.verification_status,
            RadrootsEventVerificationStatus::SignatureInvalid
        );
        assert_eq!(
            stored.verification_status,
            RadrootsEventVerificationStatus::SignatureInvalid
        );
        assert!(!stored.projection_eligible);
        assert!(
            store
                .events_since_cursor("social", 10)
                .await
                .expect("events")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn tag_rows_preserve_order_and_contract_metadata() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let event = signed_event(
            KIND_PROFILE,
            14,
            vec![
                vec!["p".to_owned(), FIXTURE_ALICE_PUBLIC_KEY_HEX.to_owned()],
                vec!["t".to_owned(), "harvest".to_owned()],
            ],
            "{}",
        );

        store
            .ingest_event(RadrootsEventIngest::new(event.clone(), 3_000))
            .await
            .expect("ingest");
        let tags = store.tags_for_event(event.id.as_str()).await.expect("tags");

        assert_eq!(tags[0].tag_index, 0);
        assert_eq!(tags[0].tag_name, "p");
        assert_eq!(tags[0].contract_value_type.as_deref(), Some("public_key"));
        assert!(tags[0].relay_indexed);
        assert_eq!(tags[1].tag_index, 1);
        assert_eq!(tags[1].tag_json, "[\"t\",\"harvest\"]");
    }

    #[tokio::test]
    async fn listing_event_tag_persists_event_pointer_contract_metadata() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let listing_event_id = event_id('f');
        let event = signed_event(
            KIND_ORDER_REQUEST,
            16,
            vec![
                vec!["d".to_owned(), "order-1".to_owned()],
                vec!["p".to_owned(), FIXTURE_ALICE_PUBLIC_KEY_HEX.to_owned()],
                vec![
                    "a".to_owned(),
                    format!(
                        "{KIND_LISTING}:{}:AAAAAAAAAAAAAAAAAAAAAg",
                        FIXTURE_ALICE_PUBLIC_KEY_HEX
                    ),
                ],
                vec![
                    "listing_event".to_owned(),
                    listing_event_id.clone(),
                    "wss://relay.example.com".to_owned(),
                ],
            ],
            "{}",
        );

        store
            .ingest_event(RadrootsEventIngest::new(event.clone(), 3_100))
            .await
            .expect("ingest");
        let tags = store.tags_for_event(event.id.as_str()).await.expect("tags");
        let listing_tag = tags
            .iter()
            .find(|tag| tag.tag_name == "listing_event")
            .expect("listing event tag");

        assert_eq!(
            listing_tag.tag_value.as_deref(),
            Some(listing_event_id.as_str())
        );
        assert_eq!(
            listing_tag.contract_semantic.as_deref(),
            Some("listing_snapshot")
        );
        assert_eq!(
            listing_tag.contract_value_type.as_deref(),
            Some("event_pointer")
        );
        assert!(!listing_tag.relay_indexed);
    }

    #[tokio::test]
    async fn relay_observations_upsert_separately_from_event_identity() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let event = signed_event(KIND_POST, 15, Vec::new(), "hello");
        let observation = RadrootsRelayObservation::new(
            "wss://relay.local",
            crate::RadrootsRelayObservationType::Subscription,
            4_000,
        );
        let ingest = RadrootsEventIngest::new(event.clone(), 4_000).with_observation(observation);
        store.ingest_event(ingest).await.expect("first");
        let observation = RadrootsRelayObservation::new(
            "wss://relay.local",
            crate::RadrootsRelayObservationType::Subscription,
            4_100,
        )
        .with_message("duplicate accepted");
        let ingest = RadrootsEventIngest::new(event.clone(), 4_100).with_observation(observation);
        store.ingest_event(ingest).await.expect("second");

        let observations = store
            .observations_for_event(event.id.as_str())
            .await
            .expect("observations");
        assert_eq!(observations.len(), 1);
        assert_eq!(observations[0].observation_count, 2);
        assert_eq!(observations[0].last_seen_at_ms, 4_100);
        assert_eq!(
            observations[0].last_message.as_deref(),
            Some("duplicate accepted")
        );
    }

    #[tokio::test]
    async fn event_heads_use_protocol_tie_breaks() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let high = signed_event(KIND_PROFILE, 20, Vec::new(), "{\"name\":\"high\"}");
        let low = signed_event(KIND_PROFILE, 20, Vec::new(), "{\"name\":\"low\"}");

        let first = store
            .ingest_event(RadrootsEventIngest::new(high.clone(), 5_000))
            .await
            .expect("first");
        let second = store
            .ingest_event(RadrootsEventIngest::new(low.clone(), 5_100))
            .await
            .expect("second");
        let RadrootsEventHeadCandidateResult::Candidate(candidate) =
            event_head_candidate_for_event(&low).expect("candidate")
        else {
            panic!("profile should select a head");
        };
        let head = store
            .event_head(&candidate.coordinate)
            .await
            .expect("head")
            .expect("stored head");

        assert_eq!(first.head_decision, RadrootsEventHeadStoreDecision::Applied);
        let expected_id = if low.id < high.id { &low.id } else { &high.id };
        let expected_second_decision = if low.id < high.id {
            RadrootsEventHeadStoreDecision::Applied
        } else {
            RadrootsEventHeadStoreDecision::SkippedSameTimestampHigherEventId
        };
        assert_eq!(second.head_decision, expected_second_decision);
        assert_eq!(&head.event_id, expected_id);
    }

    #[tokio::test]
    async fn projection_cursors_replay_by_store_sequence() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let first = signed_event(KIND_POST, 30, Vec::new(), "one");
        let second = signed_event(KIND_POST, 30, Vec::new(), "two");
        let first_receipt = store
            .ingest_event(RadrootsEventIngest::new(first.clone(), 6_000))
            .await
            .expect("first");
        let second_receipt = store
            .ingest_event(RadrootsEventIngest::new(second.clone(), 6_100))
            .await
            .expect("second");
        assert!(first_receipt.seq < second_receipt.seq);

        let replay = store
            .events_since_cursor("social", 10)
            .await
            .expect("initial replay");
        assert_eq!(replay.len(), 2);
        assert_eq!(replay[0].event_id, first.id);
        assert_eq!(replay[1].event_id, second.id);
        store
            .update_projection_cursor(&RadrootsProjectionCursor {
                projection_id: "social".to_owned(),
                projection_version: 1,
                last_event_seq: first_receipt.seq,
                updated_at_ms: 6_200,
            })
            .await
            .expect("cursor");
        let replay = store
            .events_since_cursor("social", 10)
            .await
            .expect("next replay");
        assert_eq!(replay.len(), 1);
        assert_eq!(replay[0].event_id, second.id);
    }
}
