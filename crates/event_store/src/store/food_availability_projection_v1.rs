use super::addressable_transition_feed_v1::addressable_transition_page_in_transaction_v1;
use super::{
    RADROOTS_EVENT_STORE_QUERY_LIMIT_MAX, RadrootsEventStore, bool_from_i64, u64_from_i64,
};
use crate::RadrootsEventStoreError;
use crate::generated::food_availability_projection_manifest as food_manifest;
use crate::model::{
    RADROOTS_ADDRESSABLE_TRANSITION_FEED_VERSION_V1,
    RADROOTS_FOOD_AVAILABILITY_PROJECTION_APPLY_PAGE_LIMIT_V1,
    RADROOTS_FOOD_AVAILABILITY_PROJECTION_VERSION_V1, RadrootsAddressableTransitionCursorV1,
    RadrootsAddressableTransitionScopeV1, RadrootsAddressableTransitionV1,
    RadrootsEventAdmissionStatus, RadrootsEventIngest, RadrootsEventStoreSourceGeneration,
    RadrootsFoodAvailabilitySearchQueryV1, RadrootsFoodAvailabilityStatusFilterV1,
    RadrootsStoredFoodAvailabilityImageV1, RadrootsStoredFoodAvailabilityV1,
};
use crate::nip09::reconciliation_v1::{
    EventAdmission, ReconciliationProfile, generation_from_blob,
};
use radroots_event::food_availability::RadrootsFoodIdentifier;
use radroots_event::ids::RadrootsEventId;
use radroots_event_codec::food_availability::inbound::{
    RadrootsFoodAvailabilityImageDiagnostic, RadrootsFoodAvailabilityProjectionOutcome,
    project_verified_food_availability_event_registry_v7,
};
use radroots_identity::PublicKey;
use serde::Deserialize;
use sqlx::{Row, SqliteConnection};

#[cfg(test)]
use std::sync::Arc;
#[cfg(test)]
use tokio::sync::Notify;

#[cfg(test)]
tokio::task_local! {
    pub(super) static FOOD_AVAILABILITY_AUDIT_FTS_CHECKPOINT: (Arc<Notify>, Arc<Notify>);
}

const FOOD_AVAILABILITY_CONTRACT_ID: &str = "radroots.food.availability.v1";
pub(super) const FOOD_AVAILABILITY_POINT_QUERY_V1: &str = "SELECT projection.source_generation, projection.pubkey, projection.d_tag, projection.event_id, projection.event_seq, projection.created_at, projection.contract_id, projection.content, projection.title, projection.summary, projection.published_at, projection.location, projection.price_amount, projection.price_currency, projection.price_unit, projection.quantity_amount, projection.quantity_unit, projection.status, projection.diagnostic_codes_json, projection.source_transition_seq, projection.immutable_raw_json, projection.stored_images_json FROM radroots_event_store_food_availability_read_v1 AS projection JOIN radroots_event_store_source_state AS source ON source.singleton = 1 AND source.active_generation = projection.source_generation JOIN radroots_event_store_food_availability_cursor AS cursor ON cursor.singleton = 1 AND cursor.source_generation = projection.source_generation JOIN radroots_event_store_addressable_head_state AS head ON head.source_generation = projection.source_generation AND head.kind = 30402 AND head.pubkey = projection.pubkey AND head.d_tag = projection.d_tag AND head.raw_head_event_id = projection.event_id AND head.raw_head_event_seq = projection.event_seq AND head.raw_head_created_at = projection.created_at AND head.admission_status = 'admitted' AND head.admission_code IS NULL AND head.contract_id = projection.contract_id AND head.visibility = 'visible' AND head.nip09_outcome = 'visible' WHERE projection.pubkey = ? AND projection.d_tag = ?";
pub(super) const FOOD_AVAILABILITY_RECENT_QUERY_V1: &str = "SELECT projection.source_generation, projection.pubkey, projection.d_tag, projection.event_id, projection.event_seq, projection.created_at, projection.contract_id, projection.content, projection.title, projection.summary, projection.published_at, projection.location, projection.price_amount, projection.price_currency, projection.price_unit, projection.quantity_amount, projection.quantity_unit, projection.status, projection.diagnostic_codes_json, projection.source_transition_seq, projection.immutable_raw_json, projection.stored_images_json FROM radroots_event_store_source_state AS source CROSS JOIN radroots_event_store_food_availability_read_v1 AS projection ON source.singleton = 1 AND source.active_generation = projection.source_generation CROSS JOIN radroots_event_store_food_availability_cursor AS cursor ON cursor.singleton = 1 AND cursor.source_generation = projection.source_generation CROSS JOIN radroots_event_store_addressable_head_state AS head ON head.source_generation = projection.source_generation AND head.kind = 30402 AND head.pubkey = projection.pubkey AND head.d_tag = projection.d_tag AND head.raw_head_event_id = projection.event_id AND head.raw_head_event_seq = projection.event_seq AND head.raw_head_created_at = projection.created_at AND head.admission_status = 'admitted' AND head.admission_code IS NULL AND head.contract_id = projection.contract_id AND head.visibility = 'visible' AND head.nip09_outcome = 'visible' ORDER BY projection.published_at DESC, projection.event_id ASC LIMIT ?";
pub(super) const FOOD_AVAILABILITY_RECENT_STATUS_QUERY_V1: &str = "SELECT projection.source_generation, projection.pubkey, projection.d_tag, projection.event_id, projection.event_seq, projection.created_at, projection.contract_id, projection.content, projection.title, projection.summary, projection.published_at, projection.location, projection.price_amount, projection.price_currency, projection.price_unit, projection.quantity_amount, projection.quantity_unit, projection.status, projection.diagnostic_codes_json, projection.source_transition_seq, projection.immutable_raw_json, projection.stored_images_json FROM radroots_event_store_food_availability_read_v1 AS projection JOIN radroots_event_store_source_state AS source ON source.singleton = 1 AND source.active_generation = projection.source_generation JOIN radroots_event_store_food_availability_cursor AS cursor ON cursor.singleton = 1 AND cursor.source_generation = projection.source_generation JOIN radroots_event_store_addressable_head_state AS head ON head.source_generation = projection.source_generation AND head.kind = 30402 AND head.pubkey = projection.pubkey AND head.d_tag = projection.d_tag AND head.raw_head_event_id = projection.event_id AND head.raw_head_event_seq = projection.event_seq AND head.raw_head_created_at = projection.created_at AND head.admission_status = 'admitted' AND head.admission_code IS NULL AND head.contract_id = projection.contract_id AND head.visibility = 'visible' AND head.nip09_outcome = 'visible' WHERE projection.status = ? ORDER BY projection.published_at DESC, projection.event_id ASC LIMIT ?";
pub(super) const FOOD_AVAILABILITY_SEARCH_QUERY_V1: &str = "SELECT projection.source_generation, projection.pubkey, projection.d_tag, projection.event_id, projection.event_seq, projection.created_at, projection.contract_id, projection.content, projection.title, projection.summary, projection.published_at, projection.location, projection.price_amount, projection.price_currency, projection.price_unit, projection.quantity_amount, projection.quantity_unit, projection.status, projection.diagnostic_codes_json, projection.source_transition_seq, projection.immutable_raw_json, projection.stored_images_json FROM radroots_event_store_food_availability_search_fts JOIN radroots_event_store_food_availability_read_v1 AS projection ON projection.event_seq = radroots_event_store_food_availability_search_fts.rowid JOIN radroots_event_store_source_state AS source ON source.singleton = 1 AND source.active_generation = projection.source_generation JOIN radroots_event_store_food_availability_cursor AS cursor ON cursor.singleton = 1 AND cursor.source_generation = projection.source_generation JOIN radroots_event_store_addressable_head_state AS head ON head.source_generation = projection.source_generation AND head.kind = 30402 AND head.pubkey = projection.pubkey AND head.d_tag = projection.d_tag AND head.raw_head_event_id = projection.event_id AND head.raw_head_event_seq = projection.event_seq AND head.raw_head_created_at = projection.created_at AND head.admission_status = 'admitted' AND head.admission_code IS NULL AND head.contract_id = projection.contract_id AND head.visibility = 'visible' AND head.nip09_outcome = 'visible' WHERE radroots_event_store_food_availability_search_fts MATCH ? AND (? IS NULL OR projection.status = ?) ORDER BY projection.published_at DESC, projection.event_id ASC LIMIT ?";

#[derive(Clone, Debug)]
struct FoodAvailabilityProjectionCursorState {
    feed_cursor: RadrootsAddressableTransitionCursorV1,
    projected_row_count: i64,
}

impl RadrootsEventStore {
    pub async fn food_availability_v1(
        &self,
        pubkey: &PublicKey,
        identifier: &RadrootsFoodIdentifier,
    ) -> Result<Option<RadrootsStoredFoodAvailabilityV1>, RadrootsEventStoreError> {
        let mut tx = self.pool.begin().await?;
        validate_food_availability_projection_hook_state_fast_v1(&mut tx).await?;
        let row = sqlx::query(FOOD_AVAILABILITY_POINT_QUERY_V1)
            .bind(pubkey.to_hex())
            .bind(identifier.as_str())
            .fetch_optional(&mut *tx)
            .await?;
        let result = match row {
            Some(row) => Some(load_and_validate_projection_row(row)?),
            None => None,
        };
        tx.commit().await?;
        Ok(result)
    }

    pub async fn recent_food_availability_v1(
        &self,
        status: RadrootsFoodAvailabilityStatusFilterV1,
        limit: u32,
    ) -> Result<Vec<RadrootsStoredFoodAvailabilityV1>, RadrootsEventStoreError> {
        validate_query_limit(limit)?;
        let mut tx = self.pool.begin().await?;
        validate_food_availability_projection_hook_state_fast_v1(&mut tx).await?;
        let rows = match status.storage_value() {
            None => {
                sqlx::query(FOOD_AVAILABILITY_RECENT_QUERY_V1)
                    .bind(i64::from(limit))
                    .fetch_all(&mut *tx)
                    .await?
            }
            Some(status) => {
                sqlx::query(FOOD_AVAILABILITY_RECENT_STATUS_QUERY_V1)
                    .bind(status)
                    .bind(i64::from(limit))
                    .fetch_all(&mut *tx)
                    .await?
            }
        };
        let result = load_and_validate_projection_rows(rows)?;
        tx.commit().await?;
        Ok(result)
    }

    pub async fn search_food_availability_v1(
        &self,
        query: &RadrootsFoodAvailabilitySearchQueryV1,
        status: RadrootsFoodAvailabilityStatusFilterV1,
        limit: u32,
    ) -> Result<Vec<RadrootsStoredFoodAvailabilityV1>, RadrootsEventStoreError> {
        validate_query_limit(limit)?;
        let mut tx = self.pool.begin().await?;
        validate_food_availability_projection_hook_state_fast_v1(&mut tx).await?;
        let rows = sqlx::query(FOOD_AVAILABILITY_SEARCH_QUERY_V1)
            .bind(query.fts5_match_expression())
            .bind(status.storage_value())
            .bind(status.storage_value())
            .bind(i64::from(limit))
            .fetch_all(&mut *tx)
            .await?;
        let result = load_and_validate_projection_rows(rows)?;
        tx.commit().await?;
        Ok(result)
    }

    /// Runs the exhaustive FoodAvailability projection and FTS integrity audit.
    ///
    /// Normal reads use the bounded seal check. Maintenance, rebuild, and
    /// conformance paths should call this after all typed writes are complete.
    pub async fn audit_food_availability_projection_v1(
        &self,
    ) -> Result<(), RadrootsEventStoreError> {
        let mut tx = self.begin_write_transaction().await?;
        validate_food_availability_projection_hook_v1(&mut tx).await?;
        tx.commit().await?;
        Ok(())
    }
}

pub(crate) async fn apply_food_availability_projection_hook_v1(
    connection: &mut SqliteConnection,
) -> Result<(), RadrootsEventStoreError> {
    apply_pending_food_availability_transitions_v1(connection).await?;
    validate_food_availability_projection_hook_v1(connection).await
}

pub(crate) async fn apply_pending_food_availability_transitions_v1(
    connection: &mut SqliteConnection,
) -> Result<(), RadrootsEventStoreError> {
    let scope = RadrootsAddressableTransitionScopeV1::food_availability();
    let mut state = ensure_projection_cursor(connection, &scope).await?;
    loop {
        let page = addressable_transition_page_in_transaction_v1(
            connection,
            &scope,
            Some(&state.feed_cursor),
            RADROOTS_FOOD_AVAILABILITY_PROJECTION_APPLY_PAGE_LIMIT_V1,
        )
        .await?;
        let mut projected_row_delta = 0_i64;
        for transition in page.transitions() {
            projected_row_delta = projected_row_delta
                .checked_add(apply_transition(connection, transition).await?)
                .ok_or_else(|| projection_drift("projection row-count delta overflowed"))?;
        }
        let next = page.next_cursor().clone();
        state = advance_projection_cursor(connection, &state, projected_row_delta, next).await?;
        if !page.has_more() {
            break;
        }
    }
    Ok(())
}

async fn ensure_projection_cursor(
    connection: &mut SqliteConnection,
    scope: &RadrootsAddressableTransitionScopeV1,
) -> Result<FoodAvailabilityProjectionCursorState, RadrootsEventStoreError> {
    let source = sqlx::query(
        "SELECT source.active_generation, source.last_transition_seq, generation.transition_floor_seq, generation.addressable_feed_version FROM radroots_event_store_source_state AS source JOIN radroots_event_store_source_generation AS generation ON generation.source_generation = source.active_generation JOIN radroots_event_store_addressable_feed_integrity_v1 AS integrity ON integrity.source_generation = source.active_generation AND integrity.transition_floor_seq = generation.transition_floor_seq AND integrity.last_transition_seq = source.last_transition_seq WHERE source.singleton = 1",
    )
    .fetch_optional(&mut *connection)
    .await?
    .ok_or_else(|| projection_drift("active source/feed authority is missing"))?;
    let generation = projection_generation_from_blob(
        source.try_get("active_generation")?,
        "active source generation is invalid",
    )?;
    let floor: i64 = source.try_get("transition_floor_seq")?;
    let feed_version: i64 = source.try_get("addressable_feed_version")?;
    if feed_version != i64::from(RADROOTS_ADDRESSABLE_TRANSITION_FEED_VERSION_V1) {
        return Err(projection_drift(format!(
            "addressable feed version is {feed_version}"
        )));
    }

    let existing = sqlx::query(
        "SELECT source_generation, feed_version, projection_version, scope_fingerprint, hook_manifest_sha256, last_transition_seq, projected_row_count FROM radroots_event_store_food_availability_cursor WHERE singleton = 1",
    )
    .fetch_optional(&mut *connection)
    .await?;
    if let Some(existing) = existing.as_ref() {
        let existing_generation = projection_generation_from_blob(
            existing.try_get("source_generation")?,
            "stored cursor generation is invalid",
        )?;
        if existing_generation != generation {
            sqlx::query("DELETE FROM radroots_event_store_food_availability_projection")
                .execute(&mut *connection)
                .await?;
            let deleted = sqlx::query(
                "DELETE FROM radroots_event_store_food_availability_cursor WHERE singleton = 1",
            )
            .execute(&mut *connection)
            .await?;
            if deleted.rows_affected() != 1 {
                return Err(projection_drift(
                    "generation reset did not delete exactly one projection cursor",
                ));
            }
        }
    }

    let existing = sqlx::query(
        "SELECT source_generation, feed_version, projection_version, scope_fingerprint, hook_manifest_sha256, last_transition_seq, projected_row_count FROM radroots_event_store_food_availability_cursor WHERE singleton = 1",
    )
    .fetch_optional(&mut *connection)
    .await?;
    if existing.is_none() {
        let inserted = sqlx::query(
            "INSERT INTO radroots_event_store_food_availability_cursor(singleton, source_generation, feed_version, projection_version, scope_fingerprint, hook_manifest_sha256, last_transition_seq, projected_row_count) VALUES (1, ?, ?, ?, ?, ?, ?, 0)",
        )
        .bind(generation.as_bytes().as_slice())
        .bind(i64::from(RADROOTS_ADDRESSABLE_TRANSITION_FEED_VERSION_V1))
        .bind(i64::from(RADROOTS_FOOD_AVAILABILITY_PROJECTION_VERSION_V1))
        .bind(scope.fingerprint().as_bytes().as_slice())
        .bind(food_manifest::FOOD_AVAILABILITY_PROJECTION_MANIFEST_SHA256)
        .bind(floor)
        .execute(&mut *connection)
        .await?;
        if inserted.rows_affected() != 1 {
            return Err(projection_drift(
                "projection cursor initialization did not insert one row",
            ));
        }
    }

    let row = sqlx::query(
        "SELECT source_generation, feed_version, projection_version, scope_fingerprint, hook_manifest_sha256, last_transition_seq, projected_row_count FROM radroots_event_store_food_availability_cursor WHERE singleton = 1",
    )
    .fetch_one(&mut *connection)
    .await?;
    validate_cursor_identity(&row, generation, scope)?;
    let projected_row_count = row.try_get("projected_row_count")?;
    validate_projected_row_count(projected_row_count)?;
    Ok(FoodAvailabilityProjectionCursorState {
        feed_cursor: RadrootsAddressableTransitionCursorV1::new(
            generation,
            scope.fingerprint(),
            row.try_get("last_transition_seq")?,
        )?,
        projected_row_count,
    })
}

fn validate_cursor_identity(
    row: &sqlx::sqlite::SqliteRow,
    generation: RadrootsEventStoreSourceGeneration,
    scope: &RadrootsAddressableTransitionScopeV1,
) -> Result<(), RadrootsEventStoreError> {
    let stored_generation = projection_generation_from_blob(
        row.try_get("source_generation")?,
        "stored cursor generation is invalid",
    )?;
    let scope_fingerprint: Vec<u8> = row.try_get("scope_fingerprint")?;
    if stored_generation != generation
        || row.try_get::<i64, _>("feed_version")?
            != i64::from(RADROOTS_ADDRESSABLE_TRANSITION_FEED_VERSION_V1)
        || row.try_get::<i64, _>("projection_version")?
            != i64::from(RADROOTS_FOOD_AVAILABILITY_PROJECTION_VERSION_V1)
        || scope_fingerprint.as_slice() != scope.fingerprint().as_bytes().as_slice()
        || row.try_get::<String, _>("hook_manifest_sha256")?
            != food_manifest::FOOD_AVAILABILITY_PROJECTION_MANIFEST_SHA256
    {
        return Err(projection_drift(
            "projection cursor identity is inconsistent",
        ));
    }
    validate_projected_row_count(row.try_get("projected_row_count")?)?;
    Ok(())
}

async fn advance_projection_cursor(
    connection: &mut SqliteConnection,
    expected: &FoodAvailabilityProjectionCursorState,
    projected_row_delta: i64,
    next: RadrootsAddressableTransitionCursorV1,
) -> Result<FoodAvailabilityProjectionCursorState, RadrootsEventStoreError> {
    let next_projected_row_count = expected
        .projected_row_count
        .checked_add(projected_row_delta)
        .ok_or_else(|| projection_drift("projection row count overflowed"))?;
    validate_projected_row_count(next_projected_row_count)?;
    if next.last_transition_seq() == expected.feed_cursor.last_transition_seq() {
        if projected_row_delta != 0 {
            return Err(projection_drift(
                "projection row count changed without a feed transition",
            ));
        }
        return Ok(expected.clone());
    }
    let updated = sqlx::query(
        "UPDATE radroots_event_store_food_availability_cursor SET last_transition_seq = ?, projected_row_count = ? WHERE singleton = 1 AND source_generation = ? AND last_transition_seq = ? AND projected_row_count = ?",
    )
    .bind(next.last_transition_seq())
    .bind(next_projected_row_count)
    .bind(next.source_generation().as_bytes().as_slice())
    .bind(expected.feed_cursor.last_transition_seq())
    .bind(expected.projected_row_count)
    .execute(&mut *connection)
    .await?;
    if updated.rows_affected() != 1 {
        return Err(projection_drift(format!(
            "projection cursor compare-and-swap expected sequence {} and row count {}",
            expected.feed_cursor.last_transition_seq(),
            expected.projected_row_count,
        )));
    }
    Ok(FoodAvailabilityProjectionCursorState {
        feed_cursor: next,
        projected_row_count: next_projected_row_count,
    })
}

async fn apply_transition(
    connection: &mut SqliteConnection,
    transition: &RadrootsAddressableTransitionV1,
) -> Result<i64, RadrootsEventStoreError> {
    let coordinate = transition.coordinate();
    let existing_event_id: Option<String> = sqlx::query_scalar(
        "SELECT event_id FROM radroots_event_store_food_availability_projection WHERE source_generation = ? AND pubkey = ? AND d_tag = ?",
    )
    .bind(transition.source_generation().as_bytes().as_slice())
    .bind(coordinate.pubkey().to_hex())
    .bind(coordinate.d_tag())
    .fetch_optional(&mut *connection)
    .await?;
    let visible_event_id = transition
        .visible_event()
        .map(|event| event.event_id().as_str());
    let mut projected_row_delta = 0_i64;
    match (existing_event_id.as_deref(), transition.retracted_event()) {
        (Some(existing), Some(retracted)) if existing == retracted.event_id().as_str() => {
            let deleted = sqlx::query(
                "DELETE FROM radroots_event_store_food_availability_projection WHERE source_generation = ? AND pubkey = ? AND d_tag = ? AND event_id = ?",
            )
            .bind(transition.source_generation().as_bytes().as_slice())
            .bind(coordinate.pubkey().to_hex())
            .bind(coordinate.d_tag())
            .bind(retracted.event_id().as_str())
            .execute(&mut *connection)
            .await?;
            if deleted.rows_affected() != 1 {
                return Err(projection_drift(
                    "pending FoodAvailability retraction did not delete one row",
                ));
            }
            projected_row_delta = -1;
        }
        (Some(existing), None) if visible_event_id == Some(existing) => {
            if transition.contract_id() != Some(FOOD_AVAILABILITY_CONTRACT_ID) {
                return Err(projection_drift(
                    "unchanged visible FoodAvailability event lost its contract admission",
                ));
            }
            return Ok(0);
        }
        (Some(_), _) => {
            return Err(projection_drift(
                "pending transition does not retract the stored coordinate projection",
            ));
        }
        (None, _) => {}
    }

    let Some(canonical) = transition.visible_event() else {
        return Ok(projected_row_delta);
    };
    if transition.contract_id() != Some(FOOD_AVAILABILITY_CONTRACT_ID) {
        return Ok(projected_row_delta);
    }
    let ingest = RadrootsEventIngest::from_raw_json(canonical.raw_json().to_owned(), 0)
        .map_err(|error| projection_drift(format!("canonical event reverify failed: {error}")))?;
    let projection =
        match project_verified_food_availability_event_registry_v7(ingest.verified_event())
            .map_err(|error| {
                projection_drift(format!("FoodAvailability projection failed: {error}"))
            })? {
            RadrootsFoodAvailabilityProjectionOutcome::Focused(projection) => projection,
            RadrootsFoodAvailabilityProjectionOutcome::Excluded(_) => {
                return Err(projection_drift(
                    "FoodAvailability-admitted transition was excluded by registry-v7 projection",
                ));
            }
            _ => {
                return Err(projection_drift(
                    "FoodAvailability projection returned an unsupported outcome",
                ));
            }
        };
    let event = ingest.event();
    if canonical.event_id().as_str() != event.id_str()
        || canonical.pubkey() != event.author()
        || canonical.created_at() != event.created_at_u64()
        || canonical.kind() != event.kind_u32()
    {
        return Err(projection_drift(
            "canonical event identity disagrees with its verified raw JSON",
        ));
    }
    let stored = RadrootsStoredFoodAvailabilityV1::from_projection(
        transition.source_generation(),
        *canonical.pubkey(),
        canonical.event_id().clone(),
        transition.raw_head().event_seq(),
        canonical.created_at(),
        transition.transition_seq(),
        &projection,
    )?;
    persist_projection(connection, &stored).await?;
    projected_row_delta
        .checked_add(1)
        .ok_or_else(|| projection_drift("projection row-count delta overflowed"))
}

async fn persist_projection(
    connection: &mut SqliteConnection,
    projection: &RadrootsStoredFoodAvailabilityV1,
) -> Result<(), RadrootsEventStoreError> {
    let diagnostic_codes_json = diagnostic_codes_json(projection.diagnostics())?;
    let quantity_amount = projection.quantity().map(|quantity| quantity.amount());
    let quantity_unit = projection
        .quantity()
        .map(|quantity| quantity.unit().as_str());
    let inserted = sqlx::query(
        "INSERT INTO radroots_event_store_food_availability_projection(source_generation, kind, pubkey, d_tag, event_id, event_seq, created_at, contract_id, content, title, summary, published_at, location, price_amount, price_currency, price_unit, quantity_amount, quantity_unit, status, diagnostic_codes_json, source_transition_seq) VALUES (?, 30402, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(projection.source_generation().as_bytes().as_slice())
    .bind(projection.pubkey().to_hex())
    .bind(projection.identifier().as_str())
    .bind(projection.event_id().as_str())
    .bind(projection.event_seq())
    .bind(i64_from_u64("food.created_at", projection.created_at())?)
    .bind(FOOD_AVAILABILITY_CONTRACT_ID)
    .bind(projection.content().as_str())
    .bind(projection.title().as_str())
    .bind(projection.summary().as_str())
    .bind(i64_from_u64(
        "food.published_at",
        projection.published_at().as_u64(),
    )?)
    .bind(projection.location().as_str())
    .bind(projection.price().amount())
    .bind(projection.price().currency().as_str())
    .bind(projection.price().unit().as_str())
    .bind(quantity_amount)
    .bind(quantity_unit)
    .bind(projection.status().as_str())
    .bind(diagnostic_codes_json)
    .bind(projection.source_transition_seq())
    .execute(&mut *connection)
    .await?;
    if inserted.rows_affected() != 1 {
        return Err(projection_drift(
            "FoodAvailability projection insert did not affect one row",
        ));
    }
    for image in projection.images() {
        persist_image(connection, projection, image).await?;
    }
    Ok(())
}

async fn persist_image(
    connection: &mut SqliteConnection,
    projection: &RadrootsStoredFoodAvailabilityV1,
    image: &RadrootsStoredFoodAvailabilityImageV1,
) -> Result<(), RadrootsEventStoreError> {
    let raw_tag_json = serde_json::to_string(image.raw_tag())?;
    let diagnostics_json = diagnostic_codes_json(image.diagnostics())?;
    let dimensions = image.dimensions();
    let inserted = sqlx::query(
        "INSERT INTO radroots_event_store_food_availability_image(source_generation, pubkey, d_tag, image_index, raw_tag_json, url, width, height, blossom_sha256, qualifies, diagnostic_codes_json) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(projection.source_generation().as_bytes().as_slice())
    .bind(projection.pubkey().to_hex())
    .bind(projection.identifier().as_str())
    .bind(i64::from(image.image_index()))
    .bind(raw_tag_json)
    .bind(image.url())
    .bind(dimensions.map(|value| i64::from(value.width())))
    .bind(dimensions.map(|value| i64::from(value.height())))
    .bind(image.blossom_sha256().map(|digest| digest.to_string()))
    .bind(i64::from(image.qualifies()))
    .bind(diagnostics_json)
    .execute(&mut *connection)
    .await?;
    if inserted.rows_affected() != 1 {
        return Err(projection_drift(
            "FoodAvailability image insert did not affect one row",
        ));
    }
    Ok(())
}

pub(crate) async fn validate_food_availability_projection_hook_v1(
    connection: &mut SqliteConnection,
) -> Result<(), RadrootsEventStoreError> {
    let state = food_availability_projection_cursor_state_fast_v1(connection).await?;
    let generation = state.feed_cursor.source_generation();

    let rows = sqlx::query(
        "SELECT source_generation, pubkey, d_tag, event_id, event_seq, created_at, contract_id, content, title, summary, published_at, location, price_amount, price_currency, price_unit, quantity_amount, quantity_unit, status, diagnostic_codes_json, source_transition_seq, immutable_raw_json, stored_images_json FROM radroots_event_store_food_availability_read_v1 WHERE source_generation = ? ORDER BY pubkey, d_tag",
    )
    .bind(generation.as_bytes().as_slice())
    .fetch_all(&mut *connection)
    .await?;
    let mut actual_coordinates = Vec::with_capacity(rows.len());
    for row in rows {
        let projection = load_and_validate_projection_row(row)?;
        validate_projection_source_transition(connection, &projection).await?;
        validate_fts_row(connection, &projection).await?;
        actual_coordinates.push((
            projection.pubkey().to_hex(),
            projection.identifier().as_str().to_owned(),
            projection.event_id().as_str().to_owned(),
            projection.event_seq(),
            i64_from_u64("food.created_at", projection.created_at())?,
        ));
    }
    let actual_row_count = i64::try_from(actual_coordinates.len())
        .map_err(|_| projection_drift("projection row count exceeds i64"))?;
    if actual_row_count != state.projected_row_count {
        return Err(projection_drift(format!(
            "projection row count {} differs from sealed count {}",
            actual_row_count, state.projected_row_count,
        )));
    }
    let expected_coordinates = sqlx::query(
        "SELECT pubkey, d_tag, raw_head_event_id, raw_head_event_seq, raw_head_created_at FROM radroots_event_store_addressable_head_state WHERE source_generation = ? AND kind = 30402 AND admission_status = 'admitted' AND admission_code IS NULL AND contract_id = ? AND visibility = 'visible' AND nip09_outcome = 'visible' ORDER BY pubkey, d_tag",
    )
    .bind(generation.as_bytes().as_slice())
    .bind(FOOD_AVAILABILITY_CONTRACT_ID)
    .fetch_all(&mut *connection)
    .await?
    .into_iter()
    .map(|row| {
        Ok::<_, sqlx::Error>((
            row.try_get::<String, _>("pubkey")?,
            row.try_get::<String, _>("d_tag")?,
            row.try_get::<String, _>("raw_head_event_id")?,
            row.try_get::<i64, _>("raw_head_event_seq")?,
            row.try_get::<i64, _>("raw_head_created_at")?,
        ))
    })
    .collect::<Result<Vec<_>, _>>()?;
    if actual_coordinates != expected_coordinates {
        return Err(projection_drift(
            "projection coordinate witnesses do not equal the current admitted, visible FoodAvailability heads",
        ));
    }
    let fts_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM radroots_event_store_food_availability_search_fts",
    )
    .fetch_one(&mut *connection)
    .await?;
    if fts_count != state.projected_row_count {
        return Err(projection_drift(format!(
            "FoodAvailability FTS row count {fts_count} differs from sealed count {}",
            state.projected_row_count,
        )));
    }
    #[cfg(test)]
    wait_at_food_availability_audit_fts_checkpoint().await;
    sqlx::query(
        "INSERT INTO radroots_event_store_food_availability_search_fts(radroots_event_store_food_availability_search_fts) VALUES('integrity-check')",
    )
    .execute(&mut *connection)
    .await
    .map_err(|error| projection_drift(format!("FoodAvailability FTS integrity check failed: {error}")))?;
    Ok(())
}

async fn validate_projection_source_transition(
    connection: &mut SqliteConnection,
    projection: &RadrootsStoredFoodAvailabilityV1,
) -> Result<(), RadrootsEventStoreError> {
    let authoritative: i64 = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM radroots_event_store_addressable_head_transition AS transition WHERE transition.transition_seq = ? AND transition.source_generation = ? AND transition.source_generation = (SELECT active_generation FROM radroots_event_store_source_state WHERE singleton = 1) AND transition.kind = 30402 AND transition.pubkey = ? AND transition.d_tag = ? AND transition.raw_head_event_id = ? AND transition.raw_head_event_seq = ? AND transition.raw_head_created_at = ? AND transition.visible_event_id = ? AND transition.visible_event_seq = ? AND transition.admission_status = 'admitted' AND transition.admission_code IS NULL AND transition.contract_id = ? AND transition.visibility = 'visible' AND transition.nip09_outcome = 'visible' AND transition.raw_head_decision IN ('baseline_rebuild', 'applied') AND transition.transition_seq = (SELECT MAX(candidate.transition_seq) FROM radroots_event_store_addressable_head_transition AS candidate WHERE candidate.source_generation = transition.source_generation AND candidate.kind = transition.kind AND candidate.pubkey = transition.pubkey AND candidate.d_tag = transition.d_tag AND candidate.raw_head_decision IN ('baseline_rebuild', 'applied')))",
    )
    .bind(projection.source_transition_seq())
    .bind(projection.source_generation().as_bytes().as_slice())
    .bind(projection.pubkey().to_hex())
    .bind(projection.identifier().as_str())
    .bind(projection.event_id().as_str())
    .bind(projection.event_seq())
    .bind(i64_from_u64("food.created_at", projection.created_at())?)
    .bind(projection.event_id().as_str())
    .bind(projection.event_seq())
    .bind(FOOD_AVAILABILITY_CONTRACT_ID)
    .fetch_one(&mut *connection)
    .await?;
    if authoritative != 1 {
        return Err(projection_drift(
            "stored FoodAvailability source transition is not authoritative for its projection",
        ));
    }
    Ok(())
}

#[cfg(test)]
async fn wait_at_food_availability_audit_fts_checkpoint() {
    let release = FOOD_AVAILABILITY_AUDIT_FTS_CHECKPOINT
        .try_with(|(reached, release)| {
            reached.notify_one();
            Arc::clone(release)
        })
        .ok();
    if let Some(release) = release {
        release.notified().await;
    }
}

pub(crate) async fn validate_food_availability_projection_hook_state_fast_v1(
    connection: &mut SqliteConnection,
) -> Result<(), RadrootsEventStoreError> {
    food_availability_projection_cursor_state_fast_v1(connection)
        .await
        .map(|_| ())
}

async fn food_availability_projection_cursor_state_fast_v1(
    connection: &mut SqliteConnection,
) -> Result<FoodAvailabilityProjectionCursorState, RadrootsEventStoreError> {
    let scope = RadrootsAddressableTransitionScopeV1::food_availability();
    let row = sqlx::query(
        "SELECT source.active_generation, source.last_transition_seq AS source_high_water, generation.transition_floor_seq AS generation_floor, generation.addressable_feed_version, integrity.transition_floor_seq AS integrity_floor, integrity.last_transition_seq AS integrity_high_water, integrity.transition_count, cursor.source_generation, cursor.feed_version, cursor.projection_version, cursor.scope_fingerprint, cursor.hook_manifest_sha256, cursor.last_transition_seq, cursor.projected_row_count FROM radroots_event_store_source_state AS source JOIN radroots_event_store_source_generation AS generation ON generation.source_generation = source.active_generation JOIN radroots_event_store_addressable_feed_integrity_v1 AS integrity ON integrity.source_generation = source.active_generation JOIN radroots_event_store_food_availability_cursor AS cursor ON cursor.singleton = 1 WHERE source.singleton = 1",
    )
    .fetch_optional(&mut *connection)
    .await?
    .ok_or_else(|| projection_drift("active source, feed, or projection seal is missing"))?;
    let generation = projection_generation_from_blob(
        row.try_get("active_generation")?,
        "active source generation is invalid",
    )?;
    validate_cursor_identity(&row, generation, &scope)?;
    let source_high_water: i64 = row.try_get("source_high_water")?;
    let generation_floor: i64 = row.try_get("generation_floor")?;
    let integrity_floor: i64 = row.try_get("integrity_floor")?;
    let integrity_high_water: i64 = row.try_get("integrity_high_water")?;
    let transition_count: i64 = row.try_get("transition_count")?;
    let cursor_high_water: i64 = row.try_get("last_transition_seq")?;
    let expected_transition_count = source_high_water
        .checked_sub(generation_floor)
        .filter(|count| *count >= 0)
        .ok_or_else(|| projection_drift("source high-water precedes its transition floor"))?;
    if row.try_get::<i64, _>("addressable_feed_version")?
        != i64::from(RADROOTS_ADDRESSABLE_TRANSITION_FEED_VERSION_V1)
        || integrity_floor != generation_floor
        || integrity_high_water != source_high_water
        || transition_count != expected_transition_count
    {
        return Err(projection_drift(
            "active addressable feed integrity seal is inconsistent",
        ));
    }
    if cursor_high_water != source_high_water {
        return Err(projection_drift(
            "projection cursor is not at the source high-water",
        ));
    }
    let projected_row_count: i64 = row.try_get("projected_row_count")?;
    validate_projected_row_count(projected_row_count)?;
    Ok(FoodAvailabilityProjectionCursorState {
        feed_cursor: RadrootsAddressableTransitionCursorV1::new(
            generation,
            scope.fingerprint(),
            cursor_high_water,
        )?,
        projected_row_count,
    })
}

fn load_and_validate_projection_rows(
    rows: Vec<sqlx::sqlite::SqliteRow>,
) -> Result<Vec<RadrootsStoredFoodAvailabilityV1>, RadrootsEventStoreError> {
    let mut result = Vec::with_capacity(rows.len());
    for row in rows {
        result.push(load_and_validate_projection_row(row)?);
    }
    Ok(result)
}

fn load_and_validate_projection_row(
    row: sqlx::sqlite::SqliteRow,
) -> Result<RadrootsStoredFoodAvailabilityV1, RadrootsEventStoreError> {
    let generation = projection_generation_from_blob(
        row.try_get("source_generation")?,
        "stored projection generation is invalid",
    )?;
    let pubkey = PublicKey::from_hex(row.try_get::<String, _>("pubkey")?.as_str())
        .map_err(|error| projection_drift(format!("stored pubkey is invalid: {error}")))?;
    let event_id = RadrootsEventId::parse(row.try_get::<String, _>("event_id")?.as_str())
        .map_err(|error| projection_drift(format!("stored event id is invalid: {error}")))?;
    let event_seq: i64 = row.try_get("event_seq")?;
    let created_at = u64_from_i64("food.created_at", row.try_get("created_at")?)
        .map_err(|error| projection_drift(error.to_string()))?;
    let transition_seq: i64 = row.try_get("source_transition_seq")?;
    let raw_json: String = row.try_get("immutable_raw_json")?;
    let ingest = RadrootsEventIngest::from_raw_json(raw_json, 0)
        .map_err(|error| projection_drift(format!("projected event reverify failed: {error}")))?;
    if ingest.event().id_str() != event_id.as_str()
        || ingest.event().author() != &pubkey
        || ingest.event().created_at_u64() != created_at
        || ingest.event().kind_u32() != 30_402
    {
        return Err(projection_drift(
            "projection identity disagrees with immutable signed event",
        ));
    }
    let admission = EventAdmission::for_profile(
        ReconciliationProfile::Nip09V1RegistryV7,
        ingest.verified_event(),
    )
    .map_err(|error| projection_drift(format!("stored admission is invalid: {error}")))?;
    if admission.status != RadrootsEventAdmissionStatus::Admitted
        || admission.contract.map(|contract| contract.id) != Some(FOOD_AVAILABILITY_CONTRACT_ID)
    {
        return Err(projection_drift(
            "projected event is not registry-v7 FoodAvailability",
        ));
    }
    let focused =
        match project_verified_food_availability_event_registry_v7(ingest.verified_event())
            .map_err(|error| projection_drift(format!("stored projection failed: {error}")))?
        {
            RadrootsFoodAvailabilityProjectionOutcome::Focused(projection) => projection,
            RadrootsFoodAvailabilityProjectionOutcome::Excluded(_) => {
                return Err(projection_drift(
                    "stored FoodAvailability event is excluded",
                ));
            }
            _ => {
                return Err(projection_drift(
                    "stored FoodAvailability event returned an unsupported projection outcome",
                ));
            }
        };
    let expected = RadrootsStoredFoodAvailabilityV1::from_projection(
        generation,
        pubkey,
        event_id,
        event_seq,
        created_at,
        transition_seq,
        &focused,
    )?;
    validate_projection_columns(&row, &expected)?;
    validate_image_rows(row.try_get("stored_images_json")?, &expected)?;
    Ok(expected)
}

fn validate_projection_columns(
    row: &sqlx::sqlite::SqliteRow,
    expected: &RadrootsStoredFoodAvailabilityV1,
) -> Result<(), RadrootsEventStoreError> {
    let expected_diagnostics = diagnostic_codes_json(expected.diagnostics())?;
    let quantity_amount = expected.quantity().map(|quantity| quantity.amount());
    let quantity_unit = expected.quantity().map(|quantity| quantity.unit().as_str());
    if row.try_get::<String, _>("d_tag")? != expected.identifier().as_str()
        || row.try_get::<String, _>("contract_id")? != FOOD_AVAILABILITY_CONTRACT_ID
        || row.try_get::<String, _>("content")? != expected.content().as_str()
        || row.try_get::<String, _>("title")? != expected.title().as_str()
        || row.try_get::<String, _>("summary")? != expected.summary().as_str()
        || u64_from_i64("food.published_at", row.try_get("published_at")?)
            .map_err(|error| projection_drift(error.to_string()))?
            != expected.published_at().as_u64()
        || row.try_get::<String, _>("location")? != expected.location().as_str()
        || row.try_get::<String, _>("price_amount")? != expected.price().amount()
        || row.try_get::<String, _>("price_currency")? != expected.price().currency().as_str()
        || row.try_get::<String, _>("price_unit")? != expected.price().unit().as_str()
        || row
            .try_get::<Option<String>, _>("quantity_amount")?
            .as_deref()
            != quantity_amount
        || row
            .try_get::<Option<String>, _>("quantity_unit")?
            .as_deref()
            != quantity_unit
        || row.try_get::<String, _>("status")? != expected.status().as_str()
        || row.try_get::<String, _>("diagnostic_codes_json")? != expected_diagnostics
    {
        return Err(projection_drift(
            "stored FoodAvailability columns differ from registry-v7 reprojection",
        ));
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredFoodAvailabilityImageRowV1 {
    image_index: i64,
    raw_tag_json: String,
    url: Option<String>,
    width: Option<i64>,
    height: Option<i64>,
    blossom_sha256: Option<String>,
    qualifies: i64,
    diagnostic_codes_json: String,
}

fn validate_image_rows(
    stored_images_json: String,
    expected: &RadrootsStoredFoodAvailabilityV1,
) -> Result<(), RadrootsEventStoreError> {
    let rows: Vec<StoredFoodAvailabilityImageRowV1> =
        serde_json::from_str(stored_images_json.as_str())
            .map_err(|error| projection_drift(format!("stored image rows are invalid: {error}")))?;
    if rows.len() != expected.images().len() {
        return Err(projection_drift(
            "stored FoodAvailability image count differs",
        ));
    }
    for (row, image) in rows.into_iter().zip(expected.images()) {
        let dimensions = image.dimensions();
        let stored_blossom_sha256 = row
            .blossom_sha256
            .map(|value| {
                radroots_blossom::RadrootsBlossomSha256::from_hex(value.as_str()).map_err(|error| {
                    projection_drift(format!("stored Blossom digest is invalid: {error}"))
                })
            })
            .transpose()?;
        if row.image_index != i64::from(image.image_index())
            || row.raw_tag_json
                != serde_json::to_string(image.raw_tag()).map_err(|error| {
                    projection_drift(format!("expected image tag is not serializable: {error}"))
                })?
            || row.url.as_deref() != image.url()
            || row.width != dimensions.map(|value| i64::from(value.width()))
            || row.height != dimensions.map(|value| i64::from(value.height()))
            || stored_blossom_sha256 != image.blossom_sha256()
            || bool_from_i64("food.image.qualifies", row.qualifies)
                .map_err(|error| projection_drift(error.to_string()))?
                != image.qualifies()
            || row.diagnostic_codes_json != diagnostic_codes_json(image.diagnostics())?
        {
            return Err(projection_drift(
                "stored FoodAvailability image differs from registry-v7 reprojection",
            ));
        }
    }
    Ok(())
}

async fn validate_fts_row(
    connection: &mut SqliteConnection,
    projection: &RadrootsStoredFoodAvailabilityV1,
) -> Result<(), RadrootsEventStoreError> {
    let row = sqlx::query(
        "SELECT event_id, pubkey, d_tag, title, summary, content, location FROM radroots_event_store_food_availability_search_fts WHERE rowid = ?",
    )
    .bind(projection.event_seq())
    .fetch_optional(&mut *connection)
    .await?
    .ok_or_else(|| projection_drift("FoodAvailability FTS row is missing"))?;
    if row.try_get::<String, _>("event_id")? != projection.event_id().as_str()
        || row.try_get::<String, _>("pubkey")? != projection.pubkey().to_hex()
        || row.try_get::<String, _>("d_tag")? != projection.identifier().as_str()
        || row.try_get::<String, _>("title")? != projection.title().as_str()
        || row.try_get::<String, _>("summary")? != projection.summary().as_str()
        || row.try_get::<String, _>("content")? != projection.content().as_str()
        || row.try_get::<String, _>("location")? != projection.location().as_str()
    {
        return Err(projection_drift(
            "FoodAvailability FTS row differs from projection",
        ));
    }
    Ok(())
}

fn diagnostic_codes_json(
    diagnostics: &[RadrootsFoodAvailabilityImageDiagnostic],
) -> Result<String, RadrootsEventStoreError> {
    serde_json::to_string(
        &diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code())
            .collect::<Vec<_>>(),
    )
    .map_err(|error| projection_drift(format!("diagnostics are not serializable: {error}")))
}

fn validate_query_limit(limit: u32) -> Result<(), RadrootsEventStoreError> {
    if !(1..=RADROOTS_EVENT_STORE_QUERY_LIMIT_MAX).contains(&limit) {
        return Err(RadrootsEventStoreError::QueryLimitOutOfRange {
            min: 1,
            max: RADROOTS_EVENT_STORE_QUERY_LIMIT_MAX,
            actual: limit,
        });
    }
    Ok(())
}

fn projection_drift(reason: impl Into<String>) -> RadrootsEventStoreError {
    RadrootsEventStoreError::FoodAvailabilityProjectionDrift {
        reason: reason.into(),
    }
}

fn validate_projected_row_count(value: i64) -> Result<(), RadrootsEventStoreError> {
    if value < 0 {
        return Err(projection_drift(format!(
            "projection cursor has negative row count {value}",
        )));
    }
    Ok(())
}

fn projection_generation_from_blob(
    value: Vec<u8>,
    context: &'static str,
) -> Result<RadrootsEventStoreSourceGeneration, RadrootsEventStoreError> {
    generation_from_blob(value).map_err(|error| projection_drift(format!("{context}: {error}")))
}

fn i64_from_u64(field: &'static str, value: u64) -> Result<i64, RadrootsEventStoreError> {
    i64::try_from(value).map_err(|_| RadrootsEventStoreError::UnsignedIntegerRange { field, value })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_generation_corruption_uses_the_projection_error_surface() {
        assert!(matches!(
            projection_generation_from_blob(
                vec![0_u8; 31],
                "stored projection generation is invalid",
            ),
            Err(RadrootsEventStoreError::FoodAvailabilityProjectionDrift { reason })
                if reason.contains("stored projection generation is invalid")
                    && reason.contains("31 bytes instead of 32")
        ));
    }
}
