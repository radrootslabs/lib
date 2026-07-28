#![forbid(unsafe_code)]

use crate::model::{RadrootsEventIngest, RadrootsEventStoreSourceGeneration};
use crate::nip09::reconciliation_v1::{
    ReconciliationCapacity, ReconciliationCapacityLimits, measure_reconciliation_capacity_bounded,
};
use crate::{
    RADROOTS_EVENT_STORE_RETAINED_SOURCE_GENERATION_LIMIT_V1, RadrootsEventStoreError,
    RadrootsEventStoreSourceCapacityResourceV1,
};
use sqlx::{Row, SqliteConnection};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RawSourceCapacityDeltaV1 {
    raw_events: u64,
    raw_tags: u64,
    raw_event_bytes: u64,
    raw_tag_bytes: u64,
}

/// Persisted retained raw-source capacity sealed to one database snapshot.
///
/// This is the constant-cost authority used by ordinary reads and writes. It
/// does not rescan raw rows; migrations and database reopen perform the full
/// raw-source recount that authenticates this seal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RadrootsEventStoreSourceCapacityV1 {
    source_generation: RadrootsEventStoreSourceGeneration,
    capacity: ReconciliationCapacity,
    raw_high_water_seq: i64,
    retained_generation_count: u32,
    retained_generation_limit: u32,
}

impl RadrootsEventStoreSourceCapacityV1 {
    /// Returns the active source generation sealed by this snapshot.
    pub const fn source_generation(&self) -> RadrootsEventStoreSourceGeneration {
        self.source_generation
    }

    /// Returns the retained raw event-row count.
    pub const fn raw_event_count(&self) -> u64 {
        self.capacity.raw_events
    }

    /// Returns the retained raw tag-row count.
    pub const fn raw_tag_count(&self) -> u64 {
        self.capacity.raw_tags
    }

    /// Returns governed UTF-8 bytes across retained raw event text fields.
    pub const fn raw_event_text_bytes(&self) -> u64 {
        self.capacity.raw_event_bytes
    }

    /// Returns governed UTF-8 bytes across retained raw tag text fields.
    pub const fn raw_tag_text_bytes(&self) -> u64 {
        self.capacity.raw_tag_bytes
    }

    /// Returns the greatest retained raw event sequence.
    pub const fn raw_high_water_seq(&self) -> i64 {
        self.raw_high_water_seq
    }

    /// Returns the append-only source generations currently retained.
    pub const fn retained_generation_count(&self) -> u32 {
        self.retained_generation_count
    }

    /// Returns the maximum append-only source generations this store retains.
    pub const fn retained_generation_limit(&self) -> u32 {
        self.retained_generation_limit
    }
}

pub(crate) fn raw_source_capacity_delta_v1(
    ingest: &RadrootsEventIngest,
    tags_json: &str,
) -> Result<RawSourceCapacityDeltaV1, RadrootsEventStoreError> {
    let event = ingest.event();
    let event_id = event.id_hex();
    let signature = event.signature_hex();
    let raw_event_bytes = raw_event_row_bytes_v1(
        event_id.as_str(),
        &event.author().to_hex(),
        tags_json,
        event.content(),
        signature.as_str(),
        ingest.raw_json(),
    )?;
    let mut raw_tags = 0_u64;
    let mut raw_tag_bytes = 0_u64;
    for tag in event.tag_slices() {
        let values = tag.as_slice();
        let tag_name = values.first().map(String::as_str).unwrap_or("");
        let tag_value = values.get(1).map(String::as_str);
        let tag_json = serde_json::to_string(values)?;
        raw_tags = checked_capacity_add(
            RadrootsEventStoreSourceCapacityResourceV1::RawTags,
            raw_tags,
            1,
        )?;
        raw_tag_bytes = checked_capacity_add(
            RadrootsEventStoreSourceCapacityResourceV1::RawTagBytes,
            raw_tag_bytes,
            raw_tag_row_bytes_v1(event_id.as_str(), tag_name, tag_value, tag_json.as_str())?,
        )?;
    }
    Ok(RawSourceCapacityDeltaV1 {
        raw_events: 1,
        raw_tags,
        raw_event_bytes,
        raw_tag_bytes,
    })
}

pub(crate) async fn preflight_unique_raw_source_append_v1(
    connection: &mut SqliteConnection,
    delta: RawSourceCapacityDeltaV1,
) -> Result<(), RadrootsEventStoreError> {
    let current = validate_source_capacity_authority_fast_v1(connection).await?;
    validate_prospective_capacity(current.capacity, delta)
}

pub(crate) async fn advance_source_capacity_after_insert_v1(
    connection: &mut SqliteConnection,
    delta: RawSourceCapacityDeltaV1,
    inserted_seq: i64,
) -> Result<(), RadrootsEventStoreError> {
    let current = read_source_capacity_v1(connection).await?;
    validate_prospective_capacity(current.capacity, delta)?;
    let next = ReconciliationCapacity {
        raw_events: checked_capacity_add(
            RadrootsEventStoreSourceCapacityResourceV1::RawEvents,
            current.capacity.raw_events,
            delta.raw_events,
        )?,
        raw_tags: checked_capacity_add(
            RadrootsEventStoreSourceCapacityResourceV1::RawTags,
            current.capacity.raw_tags,
            delta.raw_tags,
        )?,
        raw_event_bytes: checked_capacity_add(
            RadrootsEventStoreSourceCapacityResourceV1::RawEventBytes,
            current.capacity.raw_event_bytes,
            delta.raw_event_bytes,
        )?,
        raw_tag_bytes: checked_capacity_add(
            RadrootsEventStoreSourceCapacityResourceV1::RawTagBytes,
            current.capacity.raw_tag_bytes,
            delta.raw_tag_bytes,
        )?,
    };
    let updated = sqlx::query(
        "UPDATE radroots_event_store_source_capacity_v1 SET raw_event_count = ?, raw_tag_count = ?, raw_event_bytes = ?, raw_tag_bytes = ?, raw_high_water_seq = ? WHERE singleton = 1 AND source_generation = ? AND raw_event_count = ? AND raw_tag_count = ? AND raw_event_bytes = ? AND raw_tag_bytes = ? AND raw_high_water_seq = ? AND retained_generation_count = ? AND retained_generation_limit = ?",
    )
    .bind(sqlite_capacity_value(next.raw_events, "raw_event_count")?)
    .bind(sqlite_capacity_value(next.raw_tags, "raw_tag_count")?)
    .bind(sqlite_capacity_value(next.raw_event_bytes, "raw_event_bytes")?)
    .bind(sqlite_capacity_value(next.raw_tag_bytes, "raw_tag_bytes")?)
    .bind(inserted_seq)
    .bind(current.source_generation.as_bytes().as_slice())
    .bind(sqlite_capacity_value(
        current.capacity.raw_events,
        "raw_event_count",
    )?)
    .bind(sqlite_capacity_value(
        current.capacity.raw_tags,
        "raw_tag_count",
    )?)
    .bind(sqlite_capacity_value(
        current.capacity.raw_event_bytes,
        "raw_event_bytes",
    )?)
    .bind(sqlite_capacity_value(
        current.capacity.raw_tag_bytes,
        "raw_tag_bytes",
    )?)
    .bind(current.raw_high_water_seq)
    .bind(i64::from(current.retained_generation_count))
    .bind(i64::from(current.retained_generation_limit))
    .execute(&mut *connection)
    .await?;
    if updated.rows_affected() != 1 {
        return source_capacity_drift(format!(
            "append authority compare-and-swap affected {} rows",
            updated.rows_affected()
        ));
    }
    validate_source_capacity_authority_fast_v1(connection)
        .await
        .map(|_| ())
}

pub(crate) async fn apply_source_maintenance_hook_v1(
    connection: &mut SqliteConnection,
) -> Result<(), RadrootsEventStoreError> {
    let capacity = measure_reconciliation_capacity_bounded(
        connection,
        ReconciliationCapacityLimits::production(),
    )
    .await?;
    validate_measured_capacity(capacity)?;
    validate_no_persisted_ephemeral_raw_rows_v1(connection).await?;
    let row = sqlx::query(
        "SELECT state.active_generation, state.raw_event_count, state.raw_tag_count, state.raw_high_water_seq, (SELECT COUNT(*) FROM (SELECT 1 FROM radroots_event_store_source_generation LIMIT 9)) AS retained_generation_count FROM radroots_event_store_source_state AS state WHERE state.singleton = 1",
    )
    .fetch_one(&mut *connection)
    .await?;
    let source_generation = source_generation_bytes(row.try_get("active_generation")?)?;
    let raw_event_count: i64 = row.try_get("raw_event_count")?;
    let raw_tag_count: i64 = row.try_get("raw_tag_count")?;
    let raw_high_water_seq: i64 = row.try_get("raw_high_water_seq")?;
    let retained_generation_count = generation_count(row.try_get("retained_generation_count")?)?;
    if retained_generation_count > RADROOTS_EVENT_STORE_RETAINED_SOURCE_GENERATION_LIMIT_V1 {
        return Err(
            RadrootsEventStoreError::SourceGenerationHistoryLimitReached {
                current: retained_generation_count,
                limit: RADROOTS_EVENT_STORE_RETAINED_SOURCE_GENERATION_LIMIT_V1,
            },
        );
    }
    if raw_event_count != sqlite_capacity_value(capacity.raw_events, "raw_event_count")?
        || raw_tag_count != sqlite_capacity_value(capacity.raw_tags, "raw_tag_count")?
    {
        return source_capacity_drift(
            "measured raw row counts disagree with active source state".to_owned(),
        );
    }
    let inserted = sqlx::query(
        "INSERT INTO radroots_event_store_source_capacity_v1(singleton, source_generation, raw_event_count, raw_tag_count, raw_event_bytes, raw_tag_bytes, raw_high_water_seq, retained_generation_count, retained_generation_limit) VALUES (1, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(source_generation.as_slice())
    .bind(raw_event_count)
    .bind(raw_tag_count)
    .bind(sqlite_capacity_value(
        capacity.raw_event_bytes,
        "raw_event_bytes",
    )?)
    .bind(sqlite_capacity_value(
        capacity.raw_tag_bytes,
        "raw_tag_bytes",
    )?)
    .bind(raw_high_water_seq)
    .bind(i64::from(retained_generation_count))
    .bind(i64::from(
        RADROOTS_EVENT_STORE_RETAINED_SOURCE_GENERATION_LIMIT_V1,
    ))
    .execute(&mut *connection)
    .await?;
    if inserted.rows_affected() != 1 {
        return source_capacity_drift(format!(
            "source capacity initialization affected {} rows",
            inserted.rows_affected()
        ));
    }
    validate_source_capacity_authority_full_v1(connection).await
}

pub(crate) async fn validate_source_capacity_authority_fast_v1(
    connection: &mut SqliteConnection,
) -> Result<RadrootsEventStoreSourceCapacityV1, RadrootsEventStoreError> {
    let capacity = read_source_capacity_v1(connection).await?;
    validate_measured_capacity(capacity.capacity)?;
    if capacity.retained_generation_limit
        != RADROOTS_EVENT_STORE_RETAINED_SOURCE_GENERATION_LIMIT_V1
    {
        return source_capacity_drift(format!(
            "retained generation limit is {}, expected {}",
            capacity.retained_generation_limit,
            RADROOTS_EVENT_STORE_RETAINED_SOURCE_GENERATION_LIMIT_V1
        ));
    }
    let row = sqlx::query(
        "SELECT state.active_generation, state.raw_event_count, state.raw_tag_count, state.raw_high_water_seq, generation.generation_ordinal, (SELECT COUNT(*) FROM (SELECT 1 FROM radroots_event_store_source_generation LIMIT 9)) AS retained_generation_count FROM radroots_event_store_source_state AS state JOIN radroots_event_store_source_generation AS generation ON generation.source_generation = state.active_generation WHERE state.singleton = 1",
    )
    .fetch_one(&mut *connection)
    .await?;
    let active_generation = RadrootsEventStoreSourceGeneration::from_bytes(
        source_generation_bytes(row.try_get("active_generation")?)?,
    );
    let raw_event_count = sqlite_nonnegative_capacity(
        RadrootsEventStoreSourceCapacityResourceV1::RawEvents,
        row.try_get("raw_event_count")?,
    )?;
    let raw_tag_count = sqlite_nonnegative_capacity(
        RadrootsEventStoreSourceCapacityResourceV1::RawTags,
        row.try_get("raw_tag_count")?,
    )?;
    let raw_high_water_seq: i64 = row.try_get("raw_high_water_seq")?;
    let generation_ordinal = generation_count(row.try_get("generation_ordinal")?)?;
    let retained_generation_count = generation_count(row.try_get("retained_generation_count")?)?;
    if active_generation != capacity.source_generation
        || raw_event_count != capacity.capacity.raw_events
        || raw_tag_count != capacity.capacity.raw_tags
        || raw_high_water_seq != capacity.raw_high_water_seq
        || generation_ordinal != retained_generation_count
        || retained_generation_count != capacity.retained_generation_count
        || retained_generation_count > capacity.retained_generation_limit
    {
        return source_capacity_drift(
            "capacity seal does not match active source state and generation history".to_owned(),
        );
    }
    Ok(capacity)
}

pub(crate) async fn validate_source_capacity_authority_full_v1(
    connection: &mut SqliteConnection,
) -> Result<(), RadrootsEventStoreError> {
    let persisted = validate_source_capacity_authority_fast_v1(connection).await?;
    let measured = measure_reconciliation_capacity_bounded(
        connection,
        ReconciliationCapacityLimits::production(),
    )
    .await?;
    validate_measured_capacity(measured)?;
    validate_no_persisted_ephemeral_raw_rows_v1(connection).await?;
    if measured != persisted.capacity {
        return source_capacity_drift(format!(
            "persisted capacity {:?} differs from measured raw authority {measured:?}",
            persisted.capacity
        ));
    }
    Ok(())
}

pub(crate) async fn validate_no_persisted_ephemeral_raw_rows_v1(
    connection: &mut SqliteConnection,
) -> Result<(), RadrootsEventStoreError> {
    let row = sqlx::query(
        "SELECT event_id, kind FROM event_envelopes WHERE kind BETWEEN 20000 AND 29999 ORDER BY seq LIMIT 1",
    )
    .fetch_optional(&mut *connection)
    .await?;
    if let Some(row) = row {
        return Err(RadrootsEventStoreError::PersistedEphemeralRawEvent {
            event_id: row.try_get("event_id")?,
            kind: row.try_get("kind")?,
        });
    }
    Ok(())
}

pub(crate) async fn preflight_source_generation_append_v1(
    connection: &mut SqliteConnection,
) -> Result<(), RadrootsEventStoreError> {
    let capacity_authority_exists: i64 = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM main.sqlite_schema WHERE type = 'table' AND name = 'radroots_event_store_source_capacity_v1')",
    )
    .fetch_one(&mut *connection)
    .await?;
    if capacity_authority_exists == 0 {
        return Ok(());
    }
    let capacity = validate_source_capacity_authority_fast_v1(connection).await?;
    validate_source_generation_append_available_v1(
        capacity.retained_generation_count,
        capacity.retained_generation_limit,
    )
}

pub(crate) async fn bind_source_capacity_to_generation_v1(
    connection: &mut SqliteConnection,
    target_generation: RadrootsEventStoreSourceGeneration,
) -> Result<bool, RadrootsEventStoreError> {
    let capacity_authority_exists: i64 = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM main.sqlite_schema WHERE type = 'table' AND name = 'radroots_event_store_source_capacity_v1')",
    )
    .fetch_one(&mut *connection)
    .await?;
    if capacity_authority_exists == 0 {
        return Ok(false);
    }
    let current = read_source_capacity_v1(connection).await?;
    if current.source_generation == target_generation {
        return source_capacity_drift(
            "source rebuild target already owns the persisted capacity seal".to_owned(),
        );
    }
    let updated = sqlx::query(
        "UPDATE radroots_event_store_source_capacity_v1 SET source_generation = ? WHERE singleton = 1 AND source_generation = ? AND raw_event_count = ? AND raw_tag_count = ? AND raw_event_bytes = ? AND raw_tag_bytes = ? AND raw_high_water_seq = ? AND retained_generation_count = ? AND retained_generation_limit = ?",
    )
    .bind(target_generation.as_bytes().as_slice())
    .bind(current.source_generation.as_bytes().as_slice())
    .bind(sqlite_capacity_value(
        current.capacity.raw_events,
        "raw_event_count",
    )?)
    .bind(sqlite_capacity_value(
        current.capacity.raw_tags,
        "raw_tag_count",
    )?)
    .bind(sqlite_capacity_value(
        current.capacity.raw_event_bytes,
        "raw_event_bytes",
    )?)
    .bind(sqlite_capacity_value(
        current.capacity.raw_tag_bytes,
        "raw_tag_bytes",
    )?)
    .bind(current.raw_high_water_seq)
    .bind(i64::from(current.retained_generation_count))
    .bind(i64::from(current.retained_generation_limit))
    .execute(&mut *connection)
    .await?;
    if updated.rows_affected() != 1 {
        return source_capacity_drift(format!(
            "source rebuild capacity bind affected {} rows",
            updated.rows_affected()
        ));
    }
    let rebound = validate_source_capacity_authority_fast_v1(connection).await?;
    if rebound.source_generation != target_generation {
        return source_capacity_drift(
            "source rebuild capacity bind did not select its target generation".to_owned(),
        );
    }
    Ok(true)
}

fn validate_source_generation_append_available_v1(
    current: u32,
    limit: u32,
) -> Result<(), RadrootsEventStoreError> {
    if current >= limit {
        return Err(
            RadrootsEventStoreError::SourceGenerationHistoryLimitReached { current, limit },
        );
    }
    Ok(())
}

async fn read_source_capacity_v1(
    connection: &mut SqliteConnection,
) -> Result<RadrootsEventStoreSourceCapacityV1, RadrootsEventStoreError> {
    let rows = sqlx::query(
        "SELECT source_generation, raw_event_count, raw_tag_count, raw_event_bytes, raw_tag_bytes, raw_high_water_seq, retained_generation_count, retained_generation_limit FROM radroots_event_store_source_capacity_v1 WHERE singleton = 1",
    )
    .fetch_all(&mut *connection)
    .await?;
    if rows.len() != 1 {
        return source_capacity_drift(format!(
            "expected one source capacity row, found {}",
            rows.len()
        ));
    }
    let row = &rows[0];
    Ok(RadrootsEventStoreSourceCapacityV1 {
        source_generation: RadrootsEventStoreSourceGeneration::from_bytes(source_generation_bytes(
            row.try_get("source_generation")?,
        )?),
        capacity: ReconciliationCapacity {
            raw_events: sqlite_nonnegative_capacity(
                RadrootsEventStoreSourceCapacityResourceV1::RawEvents,
                row.try_get("raw_event_count")?,
            )?,
            raw_tags: sqlite_nonnegative_capacity(
                RadrootsEventStoreSourceCapacityResourceV1::RawTags,
                row.try_get("raw_tag_count")?,
            )?,
            raw_event_bytes: sqlite_nonnegative_capacity(
                RadrootsEventStoreSourceCapacityResourceV1::RawEventBytes,
                row.try_get("raw_event_bytes")?,
            )?,
            raw_tag_bytes: sqlite_nonnegative_capacity(
                RadrootsEventStoreSourceCapacityResourceV1::RawTagBytes,
                row.try_get("raw_tag_bytes")?,
            )?,
        },
        raw_high_water_seq: row.try_get("raw_high_water_seq")?,
        retained_generation_count: generation_count(row.try_get("retained_generation_count")?)?,
        retained_generation_limit: generation_count(row.try_get("retained_generation_limit")?)?,
    })
}

fn validate_prospective_capacity(
    current: ReconciliationCapacity,
    delta: RawSourceCapacityDeltaV1,
) -> Result<(), RadrootsEventStoreError> {
    let limits = ReconciliationCapacityLimits::production();
    for (resource, requested) in [
        (
            RadrootsEventStoreSourceCapacityResourceV1::RawEvents,
            delta.raw_events,
        ),
        (
            RadrootsEventStoreSourceCapacityResourceV1::RawTags,
            delta.raw_tags,
        ),
        (
            RadrootsEventStoreSourceCapacityResourceV1::RawEventBytes,
            delta.raw_event_bytes,
        ),
        (
            RadrootsEventStoreSourceCapacityResourceV1::RawTagBytes,
            delta.raw_tag_bytes,
        ),
    ] {
        let current_value = current.value(resource);
        let limit = limits.limit(resource);
        if current_value
            .checked_add(requested)
            .is_none_or(|next| next > limit)
        {
            return Err(RadrootsEventStoreError::SourceCapacityExceeded {
                resource,
                current: current_value,
                requested,
                limit,
            });
        }
    }
    Ok(())
}

fn validate_measured_capacity(
    capacity: ReconciliationCapacity,
) -> Result<(), RadrootsEventStoreError> {
    validate_prospective_capacity(
        capacity,
        RawSourceCapacityDeltaV1 {
            raw_events: 0,
            raw_tags: 0,
            raw_event_bytes: 0,
            raw_tag_bytes: 0,
        },
    )
}

fn raw_event_row_bytes_v1(
    event_id: &str,
    pubkey: &str,
    tags_json: &str,
    content: &str,
    sig: &str,
    raw_json: &str,
) -> Result<u64, RadrootsEventStoreError> {
    checked_text_byte_sum(
        RadrootsEventStoreSourceCapacityResourceV1::RawEventBytes,
        [event_id, pubkey, tags_json, content, sig, raw_json],
    )
}

fn raw_tag_row_bytes_v1(
    event_id: &str,
    tag_name: &str,
    tag_value: Option<&str>,
    tag_json: &str,
) -> Result<u64, RadrootsEventStoreError> {
    let required = checked_text_byte_sum(
        RadrootsEventStoreSourceCapacityResourceV1::RawTagBytes,
        [event_id, tag_name, tag_json],
    )?;
    checked_capacity_add(
        RadrootsEventStoreSourceCapacityResourceV1::RawTagBytes,
        required,
        u64::try_from(tag_value.map_or(0, str::len)).map_err(|_| {
            RadrootsEventStoreError::SourceCapacityExceeded {
                resource: RadrootsEventStoreSourceCapacityResourceV1::RawTagBytes,
                current: required,
                requested: u64::MAX,
                limit: ReconciliationCapacityLimits::production()
                    .limit(RadrootsEventStoreSourceCapacityResourceV1::RawTagBytes),
            }
        })?,
    )
}

fn checked_text_byte_sum<const N: usize>(
    resource: RadrootsEventStoreSourceCapacityResourceV1,
    values: [&str; N],
) -> Result<u64, RadrootsEventStoreError> {
    values.iter().try_fold(0_u64, |current, value| {
        let requested = u64::try_from(value.len()).map_err(|_| {
            RadrootsEventStoreError::SourceCapacityExceeded {
                resource,
                current,
                requested: u64::MAX,
                limit: ReconciliationCapacityLimits::production().limit(resource),
            }
        })?;
        checked_capacity_add(resource, current, requested)
    })
}

fn checked_capacity_add(
    resource: RadrootsEventStoreSourceCapacityResourceV1,
    current: u64,
    requested: u64,
) -> Result<u64, RadrootsEventStoreError> {
    current
        .checked_add(requested)
        .ok_or_else(|| RadrootsEventStoreError::SourceCapacityExceeded {
            resource,
            current,
            requested,
            limit: ReconciliationCapacityLimits::production().limit(resource),
        })
}

fn sqlite_nonnegative_capacity(
    resource: RadrootsEventStoreSourceCapacityResourceV1,
    value: i64,
) -> Result<u64, RadrootsEventStoreError> {
    u64::try_from(value).map_err(|_| RadrootsEventStoreError::SourceCapacityStateDrift {
        reason: format!("persisted {resource} is negative or outside the unsigned range: {value}"),
    })
}

fn sqlite_capacity_value(value: u64, field: &'static str) -> Result<i64, RadrootsEventStoreError> {
    i64::try_from(value).map_err(|_| RadrootsEventStoreError::SourceCapacityStateDrift {
        reason: format!("{field} exceeds the SQLite integer range: {value}"),
    })
}

fn generation_count(value: i64) -> Result<u32, RadrootsEventStoreError> {
    u32::try_from(value)
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| RadrootsEventStoreError::SourceCapacityStateDrift {
            reason: format!("retained generation count is outside the positive u32 range: {value}"),
        })
}

fn source_generation_bytes(value: Vec<u8>) -> Result<[u8; 32], RadrootsEventStoreError> {
    value.try_into().map_err(
        |value: Vec<u8>| RadrootsEventStoreError::SourceCapacityStateDrift {
            reason: format!(
                "source capacity generation has {} bytes instead of 32",
                value.len()
            ),
        },
    )
}

fn source_capacity_drift<T>(reason: String) -> Result<T, RadrootsEventStoreError> {
    Err(RadrootsEventStoreError::SourceCapacityStateDrift { reason })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RadrootsEventStore;
    use sqlx::Connection;

    fn capacity_with(
        resource: RadrootsEventStoreSourceCapacityResourceV1,
        value: u64,
    ) -> ReconciliationCapacity {
        let mut capacity = ReconciliationCapacity::default();
        match resource {
            RadrootsEventStoreSourceCapacityResourceV1::RawEvents => {
                capacity.raw_events = value;
            }
            RadrootsEventStoreSourceCapacityResourceV1::RawTags => {
                capacity.raw_tags = value;
            }
            RadrootsEventStoreSourceCapacityResourceV1::RawEventBytes => {
                capacity.raw_event_bytes = value;
            }
            RadrootsEventStoreSourceCapacityResourceV1::RawTagBytes => {
                capacity.raw_tag_bytes = value;
            }
        }
        capacity
    }

    fn delta_with(
        resource: RadrootsEventStoreSourceCapacityResourceV1,
        value: u64,
    ) -> RawSourceCapacityDeltaV1 {
        let mut delta = RawSourceCapacityDeltaV1 {
            raw_events: 0,
            raw_tags: 0,
            raw_event_bytes: 0,
            raw_tag_bytes: 0,
        };
        match resource {
            RadrootsEventStoreSourceCapacityResourceV1::RawEvents => delta.raw_events = value,
            RadrootsEventStoreSourceCapacityResourceV1::RawTags => delta.raw_tags = value,
            RadrootsEventStoreSourceCapacityResourceV1::RawEventBytes => {
                delta.raw_event_bytes = value;
            }
            RadrootsEventStoreSourceCapacityResourceV1::RawTagBytes => {
                delta.raw_tag_bytes = value;
            }
        }
        delta
    }

    #[tokio::test]
    async fn rust_utf8_accounting_matches_sqlite_blob_lengths() {
        let mut connection = SqliteConnection::connect("sqlite::memory:")
            .await
            .expect("memory SQLite");
        let event_fields = [
            "event\0id",
            "publíckey",
            "[[\"t\",\"野菜\u{0000}\"]]",
            "café 🥕\0",
            "signature",
            "{\"content\":\"café 🥕\\u0000\"}",
        ];
        let rust_event = raw_event_row_bytes_v1(
            event_fields[0],
            event_fields[1],
            event_fields[2],
            event_fields[3],
            event_fields[4],
            event_fields[5],
        )
        .expect("Rust event byte count");
        let sqlite_event: i64 = sqlx::query_scalar(
            "SELECT length(CAST(? AS BLOB)) + length(CAST(? AS BLOB)) + length(CAST(? AS BLOB)) + length(CAST(? AS BLOB)) + length(CAST(? AS BLOB)) + length(CAST(? AS BLOB))",
        )
        .bind(event_fields[0])
        .bind(event_fields[1])
        .bind(event_fields[2])
        .bind(event_fields[3])
        .bind(event_fields[4])
        .bind(event_fields[5])
        .fetch_one(&mut connection)
        .await
        .expect("SQLite event byte count");
        assert_eq!(rust_event, u64::try_from(sqlite_event).expect("positive"));

        let rust_tag = raw_tag_row_bytes_v1(
            event_fields[0],
            "t\0",
            Some("野菜🥕"),
            "[\"t\\u0000\",\"野菜🥕\"]",
        )
        .expect("Rust tag byte count");
        let sqlite_tag: i64 = sqlx::query_scalar(
            "SELECT length(CAST(? AS BLOB)) + length(CAST(? AS BLOB)) + COALESCE(length(CAST(? AS BLOB)), 0) + length(CAST(? AS BLOB))",
        )
        .bind(event_fields[0])
        .bind("t\0")
        .bind("野菜🥕")
        .bind("[\"t\\u0000\",\"野菜🥕\"]")
        .fetch_one(&mut connection)
        .await
        .expect("SQLite tag byte count");
        assert_eq!(rust_tag, u64::try_from(sqlite_tag).expect("positive"));
    }

    #[test]
    fn every_prospective_capacity_dimension_accepts_exact_and_rejects_one_over() {
        let limits = ReconciliationCapacityLimits::production();
        for resource in [
            RadrootsEventStoreSourceCapacityResourceV1::RawEvents,
            RadrootsEventStoreSourceCapacityResourceV1::RawTags,
            RadrootsEventStoreSourceCapacityResourceV1::RawEventBytes,
            RadrootsEventStoreSourceCapacityResourceV1::RawTagBytes,
        ] {
            let limit = limits.limit(resource);
            validate_prospective_capacity(
                capacity_with(resource, limit - 1),
                delta_with(resource, 1),
            )
            .expect("exact prospective capacity boundary");
            let error = validate_prospective_capacity(
                capacity_with(resource, limit),
                delta_with(resource, 1),
            )
            .expect_err("one-over prospective capacity boundary");
            assert!(matches!(
                error,
                RadrootsEventStoreError::SourceCapacityExceeded {
                    resource: actual_resource,
                    current,
                    requested: 1,
                    limit: actual_limit,
                } if actual_resource == resource && current == limit && actual_limit == limit
            ));

            validate_measured_capacity(capacity_with(resource, limit))
                .expect("exact measured capacity boundary");
            let error = validate_measured_capacity(capacity_with(resource, limit + 1))
                .expect_err("one-over measured capacity boundary");
            assert!(matches!(
                error,
                RadrootsEventStoreError::SourceCapacityExceeded {
                    resource: actual_resource,
                    current,
                    requested: 0,
                    limit: actual_limit,
                } if actual_resource == resource
                    && current == limit + 1
                    && actual_limit == limit
            ));
        }
    }

    #[test]
    fn retained_generation_nip09_logical_rows_have_an_audited_upper_bound() {
        let events = crate::RADROOTS_EVENT_STORE_RAW_EVENT_COUNT_LIMIT_V1;
        let tags = crate::RADROOTS_EVENT_STORE_RAW_TAG_COUNT_LIMIT_V1;

        // Per generation: one generation row, at most E coordinates, E
        // deletion requests, T combined event/address targets, E head states,
        // E + T transitions, and one feed-integrity row.
        let generation = 1_u64;
        let coordinates = events;
        let requests = events;
        let combined_targets = tags;
        let head_states = events;
        let transitions = events + tags;
        let feed_integrity = 1_u64;
        let per_generation = generation
            + coordinates
            + requests
            + combined_targets
            + head_states
            + transitions
            + feed_integrity;
        assert_eq!(per_generation, 4 * events + 2 * tags + 2);
        assert_eq!(per_generation, 600_002);
        assert_eq!(
            per_generation * u64::from(RADROOTS_EVENT_STORE_RETAINED_SOURCE_GENERATION_LIMIT_V1),
            4_800_016
        );

        for table in [
            "radroots_event_store_source_generation",
            "radroots_event_store_event_coordinate",
            "radroots_event_store_nip09_request",
            "radroots_event_store_nip09_event_target",
            "radroots_event_store_nip09_address_target",
            "radroots_event_store_addressable_head_state",
            "radroots_event_store_addressable_head_transition",
            "radroots_event_store_addressable_feed_integrity_v1",
        ] {
            assert!(
                crate::migrations::EVENT_STORE_MIGRATIONS
                    .iter()
                    .any(|migration| migration.owned_table_names.contains(&table)),
                "logical-bound table is absent from the governed migration catalog: {table}"
            );
        }
    }

    #[test]
    fn generation_append_limit_returns_the_typed_current_and_limit() {
        validate_source_generation_append_available_v1(7, 8)
            .expect("one retained generation slot remains");
        assert!(matches!(
            validate_source_generation_append_available_v1(8, 8),
            Err(
                RadrootsEventStoreError::SourceGenerationHistoryLimitReached {
                    current: 8,
                    limit: 8,
                }
            )
        ));
    }

    #[tokio::test]
    async fn generation_sql_backstop_allows_exact_append_and_is_conflict_safe_one_over() {
        let store = RadrootsEventStore::open_memory().await.expect("open store");
        let mut transaction = store
            .begin_write_transaction()
            .await
            .expect("maintenance fixture transaction");
        sqlx::query("DROP TRIGGER radroots_event_store_source_capacity_update_guard")
            .execute(&mut *transaction)
            .await
            .expect("remove capacity update guard in rolled-back fixture");
        sqlx::query(
            "UPDATE radroots_event_store_source_capacity_v1 SET retained_generation_count = retained_generation_limit - 1 WHERE singleton = 1",
        )
        .execute(&mut *transaction)
        .await
        .expect("place fixture one below retained generation limit");
        sqlx::query("DROP TRIGGER radroots_event_store_source_generation_append_guard")
            .execute(&mut *transaction)
            .await
            .expect("isolate the v4 generation-capacity backstop");

        let exact = sqlx::query(
            "INSERT INTO radroots_event_store_source_generation(source_generation, generation_ordinal, reconciliation_version, addressable_feed_version, event_contract_registry_version, hook_id, hook_manifest_sha256, transition_floor_seq, baseline_raw_event_count, baseline_raw_tag_count, baseline_raw_high_water_seq) SELECT ?, generation_ordinal + 1, reconciliation_version, addressable_feed_version, event_contract_registry_version, hook_id, hook_manifest_sha256, transition_floor_seq, baseline_raw_event_count, baseline_raw_tag_count, baseline_raw_high_water_seq FROM radroots_event_store_source_generation ORDER BY generation_ordinal DESC LIMIT 1",
        )
        .bind(vec![0xa4_u8; 32])
        .execute(&mut *transaction)
        .await
        .expect("exact generation boundary must append");
        assert_eq!(exact.rows_affected(), 1);
        let retained_count: i64 = sqlx::query_scalar(
            "SELECT retained_generation_count FROM radroots_event_store_source_capacity_v1 WHERE singleton = 1",
        )
        .fetch_one(&mut *transaction)
        .await
        .expect("advanced retained generation count");
        assert_eq!(retained_count, 8);

        let duplicate_error = sqlx::query(
            "INSERT INTO radroots_event_store_source_generation(source_generation, generation_ordinal, reconciliation_version, addressable_feed_version, event_contract_registry_version, hook_id, hook_manifest_sha256, transition_floor_seq, baseline_raw_event_count, baseline_raw_tag_count, baseline_raw_high_water_seq) SELECT source_generation, generation_ordinal, reconciliation_version, addressable_feed_version, event_contract_registry_version, hook_id, hook_manifest_sha256, transition_floor_seq, baseline_raw_event_count, baseline_raw_tag_count, baseline_raw_high_water_seq FROM radroots_event_store_source_generation ORDER BY generation_ordinal DESC LIMIT 1",
        )
        .execute(&mut *transaction)
        .await
        .expect_err("duplicate generation remains a v2 conflict at the limit");
        assert!(matches!(
            duplicate_error,
            sqlx::Error::Database(ref database)
                if database.message().contains("source generation already exists")
                    && !database.message().contains("generation limit reached")
        ));

        let error = sqlx::query(
            "INSERT INTO radroots_event_store_source_generation(source_generation, generation_ordinal, reconciliation_version, addressable_feed_version, event_contract_registry_version, hook_id, hook_manifest_sha256, transition_floor_seq, baseline_raw_event_count, baseline_raw_tag_count, baseline_raw_high_water_seq) SELECT ?, generation_ordinal + 1, reconciliation_version, addressable_feed_version, event_contract_registry_version, hook_id, hook_manifest_sha256, transition_floor_seq, baseline_raw_event_count, baseline_raw_tag_count, baseline_raw_high_water_seq FROM radroots_event_store_source_generation ORDER BY generation_ordinal DESC LIMIT 1",
        )
        .bind(vec![0xa5_u8; 32])
        .execute(&mut *transaction)
        .await
        .expect_err("unique generation append must hit the SQL capacity backstop");
        assert!(matches!(
            error,
            sqlx::Error::Database(ref database)
                if database.message().contains("retained source generation limit reached")
        ));
        transaction
            .rollback()
            .await
            .expect("roll back SQL backstop fixture");
    }

    #[tokio::test]
    async fn marker_close_sql_backstop_rejects_each_required_seal_drift() {
        const MARKER_CLOSE_ERROR: &str = "event-store rebuild marker cannot close before capacity, NIP-09, and FoodAvailability seals agree";

        for drift in ["capacity", "nip09", "food", "fts"] {
            let store = RadrootsEventStore::open_memory().await.expect("open store");
            let capacity_before = store
                .source_capacity_v1()
                .await
                .expect("capacity before marker fixture");
            let derived_seals_before: (i64, i64, i64) = sqlx::query_as(
                "SELECT integrity.last_transition_seq, integrity.transition_count, cursor.projected_row_count FROM radroots_event_store_addressable_feed_integrity_v1 AS integrity JOIN radroots_event_store_source_state AS source ON source.active_generation = integrity.source_generation JOIN radroots_event_store_food_availability_cursor AS cursor ON cursor.source_generation = source.active_generation WHERE source.singleton = 1 AND cursor.singleton = 1",
            )
            .fetch_one(store.pool())
            .await
            .expect("derived seals before marker fixture");
            let mut transaction = store
                .begin_write_transaction()
                .await
                .expect("marker fixture transaction");
            sqlx::query("DROP TRIGGER radroots_event_store_source_rebuild_marker_insert_guard")
                .execute(&mut *transaction)
                .await
                .expect("remove marker insert guard in rolled-back fixture");
            sqlx::query(
                "INSERT INTO radroots_event_store_source_rebuild_marker(singleton, barrier_key, target_generation, target_generation_ordinal, reconciliation_version, addressable_feed_version, event_contract_registry_version, hook_id, hook_manifest_sha256, transition_floor_seq, baseline_raw_event_count, baseline_raw_tag_count, baseline_raw_high_water_seq, prior_active_generation, prior_raw_event_count, prior_raw_tag_count, prior_raw_high_water_seq, prior_last_transition_seq) SELECT 1, 1, generation.source_generation, generation.generation_ordinal, generation.reconciliation_version, generation.addressable_feed_version, generation.event_contract_registry_version, generation.hook_id, generation.hook_manifest_sha256, generation.transition_floor_seq, state.raw_event_count, state.raw_tag_count, state.raw_high_water_seq, state.active_generation, state.raw_event_count, state.raw_tag_count, state.raw_high_water_seq, state.last_transition_seq FROM radroots_event_store_source_generation AS generation JOIN radroots_event_store_source_state AS state ON state.active_generation = generation.source_generation WHERE state.singleton = 1",
            )
            .execute(&mut *transaction)
            .await
            .expect("install completed synthetic marker");
            match drift {
                "capacity" => {
                    sqlx::query("DROP TRIGGER radroots_event_store_source_capacity_update_guard")
                        .execute(&mut *transaction)
                        .await
                        .expect("remove capacity guard in rolled-back fixture");
                    sqlx::query(
                        "UPDATE radroots_event_store_source_capacity_v1 SET raw_event_bytes = raw_event_bytes + 1 WHERE singleton = 1",
                    )
                    .execute(&mut *transaction)
                    .await
                    .expect("corrupt capacity byte seal");
                }
                "nip09" => {
                    sqlx::query(
                        "UPDATE radroots_event_store_addressable_feed_integrity_v1 SET last_transition_seq = last_transition_seq + 1, transition_count = transition_count + 1",
                    )
                    .execute(&mut *transaction)
                    .await
                    .expect("corrupt NIP-09 feed-integrity seal");
                }
                "food" => {
                    sqlx::query(
                        "DROP TRIGGER radroots_event_store_food_availability_cursor_update_guard",
                    )
                    .execute(&mut *transaction)
                    .await
                    .expect("remove Food cursor guard in rolled-back fixture");
                    sqlx::query(
                        "UPDATE radroots_event_store_food_availability_cursor SET projected_row_count = projected_row_count + 1 WHERE singleton = 1",
                    )
                    .execute(&mut *transaction)
                    .await
                    .expect("corrupt Food cursor/projection seal");
                }
                "fts" => {
                    sqlx::query(
                        "INSERT INTO radroots_event_store_food_availability_search_fts(rowid, event_id, pubkey, d_tag, title, summary, content, location) VALUES (1, 'fixture', 'fixture', 'fixture', 'fixture', '', '', '')",
                    )
                    .execute(&mut *transaction)
                    .await
                    .expect("corrupt FTS seal");
                }
                _ => unreachable!("bounded drift fixture"),
            }

            let error = sqlx::query(
                "DELETE FROM radroots_event_store_source_rebuild_marker WHERE singleton = 1",
            )
            .execute(&mut *transaction)
            .await
            .expect_err("marker close must reject inconsistent seals");
            assert!(matches!(
                error,
                sqlx::Error::Database(ref database) if database.message() == MARKER_CLOSE_ERROR
            ));
            transaction
                .rollback()
                .await
                .expect("roll back marker corruption fixture");

            assert_eq!(
                store
                    .source_capacity_v1()
                    .await
                    .expect("capacity after rollback"),
                capacity_before
            );
            let residue: (i64, i64) = sqlx::query_as(
                "SELECT (SELECT COUNT(*) FROM radroots_event_store_source_rebuild_marker), (SELECT COUNT(*) FROM radroots_event_store_food_availability_search_fts)",
            )
            .fetch_one(store.pool())
            .await
            .expect("marker and FTS residue counts");
            assert_eq!(residue, (0, 0));
            let derived_seals_after: (i64, i64, i64) = sqlx::query_as(
                "SELECT integrity.last_transition_seq, integrity.transition_count, cursor.projected_row_count FROM radroots_event_store_addressable_feed_integrity_v1 AS integrity JOIN radroots_event_store_source_state AS source ON source.active_generation = integrity.source_generation JOIN radroots_event_store_food_availability_cursor AS cursor ON cursor.source_generation = source.active_generation WHERE source.singleton = 1 AND cursor.singleton = 1",
            )
            .fetch_one(store.pool())
            .await
            .expect("derived seals after marker rollback");
            assert_eq!(derived_seals_after, derived_seals_before);
        }
    }

    #[tokio::test]
    async fn reopen_full_measure_detects_every_persisted_capacity_dimension() {
        for (index, capacity_assignment, source_assignment) in [
            (0_u8, "raw_event_count = 1", Some("raw_event_count = 1")),
            (1, "raw_tag_count = 1", Some("raw_tag_count = 1")),
            (2, "raw_event_bytes = 1", None),
            (3, "raw_tag_bytes = 1", None),
        ] {
            let directory = tempfile::tempdir().expect("temporary directory");
            let path = directory.path().join(format!("capacity-{index}.sqlite"));
            let store = RadrootsEventStore::open_file(&path)
                .await
                .expect("open file store");
            let capacity_guard: String = sqlx::query_scalar(
                "SELECT sql FROM main.sqlite_schema WHERE type = 'trigger' AND name = 'radroots_event_store_source_capacity_update_guard'",
            )
            .fetch_one(store.pool())
            .await
            .expect("capacity update guard SQL");
            let source_guard: String = sqlx::query_scalar(
                "SELECT sql FROM main.sqlite_schema WHERE type = 'trigger' AND name = 'radroots_event_store_source_state_authority_update_guard'",
            )
            .fetch_one(store.pool())
            .await
            .expect("source-state update guard SQL");

            let mut transaction = store
                .begin_write_transaction()
                .await
                .expect("corruption fixture transaction");
            sqlx::query("DROP TRIGGER radroots_event_store_source_capacity_update_guard")
                .execute(&mut *transaction)
                .await
                .expect("remove capacity guard");
            sqlx::query("DROP TRIGGER radroots_event_store_source_state_authority_update_guard")
                .execute(&mut *transaction)
                .await
                .expect("remove source-state guard");
            if let Some(source_assignment) = source_assignment {
                sqlx::query(sqlx::AssertSqlSafe(format!(
                    "UPDATE radroots_event_store_source_state SET {source_assignment} WHERE singleton = 1"
                )))
                .execute(&mut *transaction)
                .await
                .expect("corrupt source-state count seal");
            }
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "UPDATE radroots_event_store_source_capacity_v1 SET {capacity_assignment} WHERE singleton = 1"
            )))
            .execute(&mut *transaction)
            .await
            .expect("corrupt persisted capacity seal");
            sqlx::raw_sql(sqlx::AssertSqlSafe(source_guard))
                .execute(&mut *transaction)
                .await
                .expect("restore exact source-state guard");
            sqlx::raw_sql(sqlx::AssertSqlSafe(capacity_guard))
                .execute(&mut *transaction)
                .await
                .expect("restore exact capacity guard");
            transaction
                .commit()
                .await
                .expect("commit corruption fixture");
            store.pool().close().await;
            drop(store);

            assert!(matches!(
                RadrootsEventStore::open_file(&path).await,
                Err(RadrootsEventStoreError::SourceCapacityStateDrift { .. })
            ));
        }
    }

    #[tokio::test]
    async fn reopen_stops_at_the_first_raw_event_one_over_before_ephemeral_probe() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("bounded-one-over.sqlite");
        let store = RadrootsEventStore::open_file(&path)
            .await
            .expect("open file store");
        let capacity_guard: String = sqlx::query_scalar(
            "SELECT sql FROM main.sqlite_schema WHERE type = 'trigger' AND name = 'radroots_event_store_source_capacity_update_guard'",
        )
        .fetch_one(store.pool())
        .await
        .expect("capacity update guard SQL");
        let source_guard: String = sqlx::query_scalar(
            "SELECT sql FROM main.sqlite_schema WHERE type = 'trigger' AND name = 'radroots_event_store_source_state_authority_update_guard'",
        )
        .fetch_one(store.pool())
        .await
        .expect("source-state update guard SQL");
        let mut transaction = store
            .begin_write_transaction()
            .await
            .expect("corruption fixture transaction");
        sqlx::query("DROP TRIGGER radroots_event_store_source_capacity_update_guard")
            .execute(&mut *transaction)
            .await
            .expect("remove capacity guard");
        sqlx::query("DROP TRIGGER radroots_event_store_source_state_authority_update_guard")
            .execute(&mut *transaction)
            .await
            .expect("remove source-state guard");
        sqlx::query(
            "WITH RECURSIVE fixture(value) AS (VALUES(1) UNION ALL SELECT value + 1 FROM fixture WHERE value < 25001) INSERT INTO event_envelopes(seq, event_id, pubkey, created_at, kind, tags_json, content, sig, raw_json, verification_status, contract_status, contract_id, event_class, projection_eligible, inserted_at_ms, updated_at_ms) SELECT value, printf('%064x', value), printf('%064x', 0), value, CASE WHEN value = 25001 THEN 20000 ELSE 1 END, '[]', '', printf('%0128x', value), '{}', 'verified', 'unsupported', NULL, CASE WHEN value = 25001 THEN 'ephemeral' ELSE 'regular' END, 0, value, value FROM fixture",
        )
        .execute(&mut *transaction)
        .await
        .expect("install one-over raw authority with a trailing ephemeral row");
        sqlx::query(
            "UPDATE radroots_event_store_source_state SET raw_event_count = 25000, raw_high_water_seq = 25001 WHERE singleton = 1",
        )
        .execute(&mut *transaction)
        .await
        .expect("seal source state at the accepted count");
        sqlx::query(
            "UPDATE radroots_event_store_source_capacity_v1 SET raw_event_count = 25000, raw_high_water_seq = 25001 WHERE singleton = 1",
        )
        .execute(&mut *transaction)
        .await
        .expect("seal capacity at the accepted count");
        sqlx::raw_sql(sqlx::AssertSqlSafe(source_guard))
            .execute(&mut *transaction)
            .await
            .expect("restore exact source-state guard");
        sqlx::raw_sql(sqlx::AssertSqlSafe(capacity_guard))
            .execute(&mut *transaction)
            .await
            .expect("restore exact capacity guard");
        transaction
            .commit()
            .await
            .expect("commit corrupt one-over fixture");
        store.pool().close().await;
        drop(store);

        let error = match RadrootsEventStore::open_file(&path).await {
            Ok(_) => panic!("bounded reopen accepted the first row over capacity"),
            Err(error) => error,
        };
        assert!(
            matches!(
                &error,
                RadrootsEventStoreError::SourceCapacityExceeded {
                    resource: RadrootsEventStoreSourceCapacityResourceV1::RawEvents,
                    current: 25_000,
                    requested: 1,
                    limit: 25_000,
                }
            ),
            "unexpected bounded reopen error: {error:?}"
        );
    }
}
