use super::visibility_oracle_v1::{VisibilityOracleFactV1, audit_current_visibility_from_raw_v1};
use super::{
    OsSourceGenerationProvider, ReconciledEvent, ReconciliationCapacityLimits,
    SourceGenerationProvider, SourceRebuildPlan, SourceState, TransitionOrigin,
    append_source_generation, close_source_rebuild_marker, generation_from_blob,
    load_reconciliation_snapshot, open_source_rebuild_marker, persist_event_coordinate_facts,
    persist_nip09_facts, read_source_state, rebuild_raw_heads, reconcile_raw_events,
    reconciliation_profile, rotate_source_state, synchronize_addressable_heads,
    update_source_authority, validate_active_hook_state_fast, validate_baseline_authority,
    validate_raw_source_rebuild_core_with_events_v1, validate_rebuild_marker_absent,
    validate_source_raw_authority_with_state,
};
use crate::migrations::{
    EVENT_STORE_LEDGER_NAME, EVENT_STORE_MIGRATIONS, EVENT_STORE_RESERVED_PREFIX,
};
use crate::model::{
    RadrootsEventStoreActiveProductStateDigestV1, RadrootsEventStoreImmutableRawDigestV1,
    RadrootsEventStoreRawSourceRebuildReportV1,
};
use crate::schema::validate_exact_managed_v4_for_raw_source_rebuild_v1;
use crate::source_maintenance_v1::{
    preflight_source_generation_append_v1, validate_source_capacity_authority_fast_v1,
    validate_source_capacity_authority_full_v1,
};
use crate::store::food_availability_projection_v1::{
    reset_and_replay_food_availability_from_raw_v1,
    validate_food_availability_projection_hook_state_fast_v1,
    validate_food_availability_projection_hook_v1,
};
use futures::TryStreamExt;
use sha2::{Digest, Sha256};
use sqlx::{Row, Sqlite, SqliteConnection, SqlitePool, Transaction};
use std::collections::BTreeSet;

use crate::{
    RadrootsEventStoreCallerInboundForeignKeyV1, RadrootsEventStoreError,
    RadrootsEventStoreRawSourceRebuildDriftV1, RadrootsEventStoreSourceGeneration,
};

const IMMUTABLE_RAW_DIGEST_DOMAIN_V1: &[u8] = b"radroots:event-store:immutable-raw-digest:v1\0";
const ACTIVE_PRODUCT_STATE_DIGEST_DOMAIN_V1: &[u8] =
    b"radroots:event-store:active-product-state-digest:v1\0";
const TRANSITION_SEQUENCE_NAME: &str = "radroots_event_store_addressable_head_transition";
const RAW_SOURCE_REBUILD_CALLER_MAIN_TABLE_COUNT_LIMIT_V1: u32 = 4_096;
const RAW_SOURCE_REBUILD_CALLER_FOREIGN_KEY_ROW_COUNT_LIMIT_V1: u32 = 4_096;
const REBUILD_OWNED_TABLES_V1: &[&str] = &[
    "event_envelopes",
    "event_envelope_tags",
    "event_envelope_head",
    "radroots_event_store_source_generation",
    "radroots_event_store_source_rebuild_commit_barrier",
    "radroots_event_store_source_rebuild_marker",
    "radroots_event_store_source_state",
    "radroots_event_store_write_lock",
    "radroots_event_store_source_capacity_v1",
    "radroots_event_store_event_coordinate",
    "radroots_event_store_nip09_request",
    "radroots_event_store_nip09_event_target",
    "radroots_event_store_nip09_address_target",
    "radroots_event_store_addressable_head_state",
    "radroots_event_store_addressable_head_transition",
    "radroots_event_store_addressable_feed_integrity_v1",
    "radroots_event_store_food_availability_cursor",
    "radroots_event_store_food_availability_projection",
    "radroots_event_store_food_availability_image",
];
const RAW_SOURCE_REBUILD_MUTATED_PARENT_TABLES_V1: &[&str] = &[
    "event_envelopes",
    "event_envelope_tags",
    "event_envelope_head",
    "radroots_event_store_source_generation",
    "radroots_event_store_source_rebuild_commit_barrier",
    "radroots_event_store_source_rebuild_marker",
    "radroots_event_store_source_state",
    "radroots_event_store_write_lock",
    "radroots_event_store_source_capacity_v1",
    "radroots_event_store_event_coordinate",
    "radroots_event_store_nip09_request",
    "radroots_event_store_nip09_event_target",
    "radroots_event_store_nip09_address_target",
    "radroots_event_store_addressable_head_state",
    "radroots_event_store_addressable_head_transition",
    "radroots_event_store_addressable_feed_integrity_v1",
    "radroots_event_store_food_availability_cursor",
    "radroots_event_store_food_availability_projection",
    "radroots_event_store_food_availability_image",
    "radroots_event_store_food_availability_search_fts",
    "radroots_event_store_food_availability_search_fts_config",
    "radroots_event_store_food_availability_search_fts_content",
    "radroots_event_store_food_availability_search_fts_data",
    "radroots_event_store_food_availability_search_fts_docsize",
    "radroots_event_store_food_availability_search_fts_idx",
    "sqlite_sequence",
];

#[derive(Clone, Copy)]
struct RawSourceRebuildCallerSchemaLimitsV1 {
    main_tables: u32,
    foreign_key_rows: u32,
}

impl RawSourceRebuildCallerSchemaLimitsV1 {
    const fn production() -> Self {
        Self {
            main_tables: RAW_SOURCE_REBUILD_CALLER_MAIN_TABLE_COUNT_LIMIT_V1,
            foreign_key_rows: RAW_SOURCE_REBUILD_CALLER_FOREIGN_KEY_ROW_COUNT_LIMIT_V1,
        }
    }
}

#[cfg(test)]
#[allow(clippy::enum_variant_names)] // Variants mirror the governed after_* failpoint IDs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RawSourceRebuildFailpointV1 {
    AfterMarkerOpen,
    AfterGenerationRotation,
    AfterCoreReplay,
    AfterVisibilityAudit,
    AfterFoodResetAndReplay,
    AfterFoodAudit,
    AfterMarkerClose,
}

#[cfg(not(test))]
type RawSourceRebuildFailpointV1 = ();

#[cfg(test)]
impl RawSourceRebuildFailpointV1 {
    const fn as_str(self) -> &'static str {
        match self {
            Self::AfterMarkerOpen => "after_marker_open",
            Self::AfterGenerationRotation => "after_generation_rotation",
            Self::AfterCoreReplay => "after_core_replay",
            Self::AfterVisibilityAudit => "after_visibility_audit",
            Self::AfterFoodResetAndReplay => "after_food_reset_replay",
            Self::AfterFoodAudit => "after_food_audit",
            Self::AfterMarkerClose => "after_marker_close",
        }
    }
}

pub(crate) async fn rebuild_from_raw_v1_on_pool(
    pool: &SqlitePool,
) -> Result<RadrootsEventStoreRawSourceRebuildReportV1, RadrootsEventStoreError> {
    rebuild_from_raw_v1_on_pool_inner(
        pool,
        &OsSourceGenerationProvider,
        None,
        RawSourceRebuildCallerSchemaLimitsV1::production(),
    )
    .await
}

#[cfg(test)]
pub(crate) async fn rebuild_from_raw_v1_on_pool_for_test(
    pool: &SqlitePool,
    generation_provider: &dyn SourceGenerationProvider,
    failpoint: Option<RawSourceRebuildFailpointV1>,
) -> Result<RadrootsEventStoreRawSourceRebuildReportV1, RadrootsEventStoreError> {
    rebuild_from_raw_v1_on_pool_inner(
        pool,
        generation_provider,
        failpoint,
        RawSourceRebuildCallerSchemaLimitsV1::production(),
    )
    .await
}

#[cfg(test)]
pub(crate) async fn rebuild_from_raw_v1_on_pool_with_caller_schema_limits_for_test(
    pool: &SqlitePool,
    generation_provider: &dyn SourceGenerationProvider,
    main_table_limit: u32,
    foreign_key_row_limit: u32,
) -> Result<RadrootsEventStoreRawSourceRebuildReportV1, RadrootsEventStoreError> {
    rebuild_from_raw_v1_on_pool_inner(
        pool,
        generation_provider,
        None,
        RawSourceRebuildCallerSchemaLimitsV1 {
            main_tables: main_table_limit,
            foreign_key_rows: foreign_key_row_limit,
        },
    )
    .await
}

#[cfg(test)]
pub(crate) async fn rebuild_from_raw_v1_in_transaction_for_test(
    connection: &mut SqliteConnection,
    generation_provider: &dyn SourceGenerationProvider,
) -> Result<RadrootsEventStoreRawSourceRebuildReportV1, RadrootsEventStoreError> {
    rebuild_from_raw_v1_in_transaction_inner(
        connection,
        generation_provider,
        None,
        RawSourceRebuildCallerSchemaLimitsV1::production(),
    )
    .await
}

async fn rebuild_from_raw_v1_on_pool_inner(
    pool: &SqlitePool,
    generation_provider: &dyn SourceGenerationProvider,
    failpoint: Option<RawSourceRebuildFailpointV1>,
    caller_schema_limits: RawSourceRebuildCallerSchemaLimitsV1,
) -> Result<RadrootsEventStoreRawSourceRebuildReportV1, RadrootsEventStoreError> {
    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
    let result = rebuild_from_raw_v1_in_transaction_inner(
        &mut transaction,
        generation_provider,
        failpoint,
        caller_schema_limits,
    )
    .await;
    finish_raw_source_rebuild_transaction(transaction, result).await
}

pub(crate) async fn rebuild_from_raw_v1_in_existing_transaction(
    mut transaction: Transaction<'_, Sqlite>,
) -> Result<RadrootsEventStoreRawSourceRebuildReportV1, RadrootsEventStoreError> {
    let result = rebuild_from_raw_v1_in_transaction_inner(
        &mut transaction,
        &OsSourceGenerationProvider,
        None,
        RawSourceRebuildCallerSchemaLimitsV1::production(),
    )
    .await;
    finish_raw_source_rebuild_transaction(transaction, result).await
}

async fn rebuild_from_raw_v1_in_transaction_inner(
    connection: &mut SqliteConnection,
    generation_provider: &dyn SourceGenerationProvider,
    _failpoint: Option<RawSourceRebuildFailpointV1>,
    caller_schema_limits: RawSourceRebuildCallerSchemaLimitsV1,
) -> Result<RadrootsEventStoreRawSourceRebuildReportV1, RadrootsEventStoreError> {
    validate_exact_managed_v4_for_raw_source_rebuild_v1(connection).await?;
    preflight_caller_owned_schema_dependencies_v1(connection, caller_schema_limits).await?;
    validate_rebuild_marker_absent(connection).await?;
    validate_source_capacity_authority_full_v1(connection).await?;
    preflight_source_generation_append_v1(connection).await?;

    let snapshot =
        load_reconciliation_snapshot(connection, ReconciliationCapacityLimits::production())
            .await?;
    let transition_floor_seq = transition_high_water_v1(connection).await?;
    let prior =
        validate_source_lineage_for_rebuild_v1(connection, &snapshot.events, transition_floor_seq)
            .await?;
    let immutable_raw_digest = immutable_raw_digest_v1(connection).await?;
    if transition_floor_seq == i64::MAX {
        return rebuild_drift(
            RadrootsEventStoreRawSourceRebuildDriftV1::AddressableTransitionAuthority,
            "addressable transition sequence space is exhausted at SQLite INTEGER maximum",
        );
    }
    let mut generation_bytes = [0_u8; 32];
    generation_provider.fill_generation(&mut generation_bytes)?;
    let generation = RadrootsEventStoreSourceGeneration::from_bytes(generation_bytes);
    let generation_exists: i64 = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM radroots_event_store_source_generation WHERE source_generation = ?)",
    )
    .bind(generation.as_bytes().as_slice())
    .fetch_one(&mut *connection)
    .await?;
    if generation_exists != 0 {
        return rebuild_drift(
            RadrootsEventStoreRawSourceRebuildDriftV1::SourceGenerationLineage,
            "fresh source generation collided with retained lineage",
        );
    }

    let raw_event_count = i64::try_from(snapshot.capacity.raw_events).map_err(|_| {
        rebuild_state_error(
            RadrootsEventStoreRawSourceRebuildDriftV1::ImmutableRawAuthority,
            "raw event count exceeds SQLite integer range",
        )
    })?;
    let raw_tag_count = i64::try_from(snapshot.capacity.raw_tags).map_err(|_| {
        rebuild_state_error(
            RadrootsEventStoreRawSourceRebuildDriftV1::ImmutableRawAuthority,
            "raw tag count exceeds SQLite integer range",
        )
    })?;
    let raw_high_water_seq = snapshot.events.last().map(|event| event.seq).unwrap_or(0);
    let generation_ordinal: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(generation_ordinal), 0) + 1 FROM radroots_event_store_source_generation",
    )
    .fetch_one(&mut *connection)
    .await?;
    let plan = SourceRebuildPlan {
        generation,
        generation_ordinal,
        transition_floor_seq,
        raw_event_count,
        raw_tag_count,
        raw_high_water_seq,
        prior: Some(prior.clone()),
    };

    let marker = open_source_rebuild_marker(connection, &plan).await?;
    #[cfg(test)]
    inject_raw_source_rebuild_failpoint_v1(
        _failpoint,
        RawSourceRebuildFailpointV1::AfterMarkerOpen,
    )?;
    append_source_generation(connection, &plan).await?;
    rotate_source_state(connection, &plan).await?;
    #[cfg(test)]
    inject_raw_source_rebuild_failpoint_v1(
        _failpoint,
        RawSourceRebuildFailpointV1::AfterGenerationRotation,
    )?;
    let transition_sequence_rowid =
        prepare_transition_sqlite_sequence_v1(connection, transition_floor_seq).await?;

    reconcile_raw_events(connection, &snapshot.events).await?;
    persist_event_coordinate_facts(connection, generation, &snapshot.events).await?;
    rebuild_raw_heads(connection, &snapshot.events).await?;
    let requests = persist_nip09_facts(connection, generation, &snapshot.events).await?;
    synchronize_addressable_heads(
        connection,
        generation,
        &snapshot.events,
        &requests,
        TransitionOrigin::Baseline,
        None,
        "baseline_rebuild",
    )
    .await?;
    update_source_authority(
        connection,
        raw_event_count,
        raw_tag_count,
        raw_high_water_seq,
    )
    .await?;
    let replay_transition_high_water = transition_high_water_v1(connection).await?;
    validate_transition_sqlite_sequence_v1(
        connection,
        transition_sequence_rowid,
        replay_transition_high_water,
    )
    .await?;
    validate_raw_source_rebuild_core_with_events_v1(connection, generation, &snapshot.events)
        .await?;
    #[cfg(test)]
    inject_raw_source_rebuild_failpoint_v1(
        _failpoint,
        RawSourceRebuildFailpointV1::AfterCoreReplay,
    )?;

    let derived_visibility = load_derived_visibility_rows_v1(connection, generation).await?;
    audit_current_visibility_from_raw_v1(&snapshot.events, derived_visibility).await?;
    #[cfg(test)]
    inject_raw_source_rebuild_failpoint_v1(
        _failpoint,
        RawSourceRebuildFailpointV1::AfterVisibilityAudit,
    )?;

    crate::source_maintenance_v1::bind_source_capacity_to_generation_v1(connection, generation)
        .await?;
    reset_and_replay_food_availability_from_raw_v1(connection, generation).await?;
    #[cfg(test)]
    inject_raw_source_rebuild_failpoint_v1(
        _failpoint,
        RawSourceRebuildFailpointV1::AfterFoodResetAndReplay,
    )?;
    validate_food_availability_projection_hook_v1(connection).await?;
    #[cfg(test)]
    inject_raw_source_rebuild_failpoint_v1(
        _failpoint,
        RawSourceRebuildFailpointV1::AfterFoodAudit,
    )?;

    let final_raw_digest = immutable_raw_digest_v1(connection).await?;
    if final_raw_digest != immutable_raw_digest {
        return rebuild_drift(
            RadrootsEventStoreRawSourceRebuildDriftV1::ImmutableRawAuthority,
            "immutable raw digest changed during source rebuild",
        );
    }
    close_source_rebuild_marker(connection, marker).await?;
    #[cfg(test)]
    inject_raw_source_rebuild_failpoint_v1(
        _failpoint,
        RawSourceRebuildFailpointV1::AfterMarkerClose,
    )?;

    validate_active_hook_state_fast(connection).await?;
    let source_capacity = validate_source_capacity_authority_fast_v1(connection).await?;
    validate_food_availability_projection_hook_state_fast_v1(connection).await?;
    validate_scoped_integrity_v1(connection).await?;
    let active_product_state_digest =
        active_product_state_digest_v1(connection, generation).await?;
    if source_capacity.source_generation() != generation
        || source_capacity.raw_event_count() != snapshot.capacity.raw_events
        || source_capacity.raw_tag_count() != snapshot.capacity.raw_tags
        || source_capacity.raw_event_text_bytes() != snapshot.capacity.raw_event_bytes
        || source_capacity.raw_tag_text_bytes() != snapshot.capacity.raw_tag_bytes
        || source_capacity.raw_high_water_seq() != raw_high_water_seq
        || source_capacity.retained_generation_count()
            != u32::try_from(generation_ordinal).map_err(|_| {
                rebuild_state_error(
                    RadrootsEventStoreRawSourceRebuildDriftV1::SourceGenerationLineage,
                    "generation ordinal exceeds u32 range",
                )
            })?
        || prior.generation == generation
    {
        return rebuild_drift(
            RadrootsEventStoreRawSourceRebuildDriftV1::RebuildPostcondition,
            "committed rebuild report authority is inconsistent",
        );
    }
    Ok(RadrootsEventStoreRawSourceRebuildReportV1 {
        prior_source_generation: prior.generation,
        new_source_generation: generation,
        source_capacity,
        immutable_raw_digest,
        active_product_state_digest,
    })
}

async fn preflight_caller_owned_schema_dependencies_v1(
    connection: &mut SqliteConnection,
    limits: RawSourceRebuildCallerSchemaLimitsV1,
) -> Result<(), RadrootsEventStoreError> {
    let governed_names_json = governed_schema_names_json_v1()?;
    let mutated_parent_tables_json =
        serde_json::to_string(RAW_SOURCE_REBUILD_MUTATED_PARENT_TABLES_V1)?;

    let caller_main_table_count: i64 = sqlx::query_scalar(
        "WITH governed(name) AS (
           SELECT CAST(value AS TEXT) COLLATE NOCASE FROM main.json_each(?)
         )
         SELECT COUNT(*)
         FROM (
           SELECT 1
           FROM main.sqlite_schema AS child
           WHERE child.type = 'table'
             AND lower(substr(child.name, 1, 7)) != 'sqlite_'
             AND child.name COLLATE NOCASE NOT IN (SELECT name FROM governed)
             AND lower(substr(child.name, 1, length(?))) != lower(?)
           LIMIT ?
         )",
    )
    .bind(&governed_names_json)
    .bind(EVENT_STORE_RESERVED_PREFIX)
    .bind(EVENT_STORE_RESERVED_PREFIX)
    .bind(i64::from(limits.main_tables) + 1)
    .fetch_one(&mut *connection)
    .await?;
    let caller_main_table_count = caller_schema_count_v1(caller_main_table_count)?;
    if caller_main_table_count > u64::from(limits.main_tables) {
        return Err(
            RadrootsEventStoreError::RawSourceRebuildCallerTableCapacityExceeded {
                observed_at_least: caller_main_table_count,
                limit: u64::from(limits.main_tables),
            },
        );
    }

    let caller_foreign_key_row_count: i64 = sqlx::query_scalar(
        "WITH governed(name) AS (
           SELECT CAST(value AS TEXT) COLLATE NOCASE FROM main.json_each(?)
         )
         SELECT COUNT(*)
         FROM (
           SELECT 1
           FROM main.sqlite_schema AS child
           JOIN main.pragma_foreign_key_list(child.name, 'main') AS foreign_key
           WHERE child.type = 'table'
             AND lower(substr(child.name, 1, 7)) != 'sqlite_'
             AND child.name COLLATE NOCASE NOT IN (SELECT name FROM governed)
             AND lower(substr(child.name, 1, length(?))) != lower(?)
           LIMIT ?
         )",
    )
    .bind(&governed_names_json)
    .bind(EVENT_STORE_RESERVED_PREFIX)
    .bind(EVENT_STORE_RESERVED_PREFIX)
    .bind(i64::from(limits.foreign_key_rows) + 1)
    .fetch_one(&mut *connection)
    .await?;
    let caller_foreign_key_row_count = caller_schema_count_v1(caller_foreign_key_row_count)?;
    if caller_foreign_key_row_count > u64::from(limits.foreign_key_rows) {
        return Err(
            RadrootsEventStoreError::RawSourceRebuildCallerForeignKeyCapacityExceeded {
                observed_at_least: caller_foreign_key_row_count,
                limit: u64::from(limits.foreign_key_rows),
            },
        );
    }

    let dependency = sqlx::query(
        "WITH governed(name) AS (
           SELECT CAST(value AS TEXT) COLLATE NOCASE FROM main.json_each(?)
         ), rebuild_parent(name) AS (
           SELECT CAST(value AS TEXT) COLLATE NOCASE FROM main.json_each(?)
         )
         SELECT
           child.name AS child_table,
           foreign_key.id AS foreign_key_id,
           foreign_key.seq AS foreign_key_sequence,
           foreign_key.\"from\" AS child_column,
           rebuild_parent.name AS parent_table,
           foreign_key.\"to\" AS parent_column,
           foreign_key.on_update AS on_update,
           foreign_key.on_delete AS on_delete,
           foreign_key.\"match\" AS match_clause
         FROM main.sqlite_schema AS child
         JOIN main.pragma_foreign_key_list(child.name, 'main') AS foreign_key
         JOIN rebuild_parent
           ON foreign_key.\"table\" COLLATE NOCASE = rebuild_parent.name
         WHERE child.type = 'table'
           AND lower(substr(child.name, 1, 7)) != 'sqlite_'
           AND child.name COLLATE NOCASE NOT IN (SELECT name FROM governed)
           AND lower(substr(child.name, 1, length(?))) != lower(?)
         ORDER BY child.name COLLATE NOCASE, child.name, foreign_key.id, foreign_key.seq
         LIMIT 1",
    )
    .bind(&governed_names_json)
    .bind(&mutated_parent_tables_json)
    .bind(EVENT_STORE_RESERVED_PREFIX)
    .bind(EVENT_STORE_RESERVED_PREFIX)
    .fetch_optional(&mut *connection)
    .await?;
    if let Some(dependency) = dependency {
        return Err(
            RadrootsEventStoreError::RawSourceRebuildCallerInboundForeignKeyUnsupported {
                dependency: Box::new(RadrootsEventStoreCallerInboundForeignKeyV1 {
                    child_table: dependency.try_get("child_table")?,
                    foreign_key_id: dependency.try_get("foreign_key_id")?,
                    foreign_key_sequence: dependency.try_get("foreign_key_sequence")?,
                    child_column: dependency.try_get("child_column")?,
                    parent_table: dependency.try_get("parent_table")?,
                    parent_column: dependency.try_get("parent_column")?,
                    on_update: dependency.try_get("on_update")?,
                    on_delete: dependency.try_get("on_delete")?,
                    match_clause: dependency.try_get("match_clause")?,
                }),
            },
        );
    }
    Ok(())
}

fn governed_schema_names_json_v1() -> Result<String, RadrootsEventStoreError> {
    let mut names = EVENT_STORE_MIGRATIONS
        .iter()
        .flat_map(|migration| migration.owned_object_names.iter().copied())
        .collect::<BTreeSet<_>>();
    names.insert(EVENT_STORE_LEDGER_NAME);
    Ok(serde_json::to_string(&names)?)
}

fn caller_schema_count_v1(count: i64) -> Result<u64, RadrootsEventStoreError> {
    u64::try_from(count).map_err(|_| {
        rebuild_state_error(
            RadrootsEventStoreRawSourceRebuildDriftV1::ManagedSchemaAuthority,
            "caller-owned schema inventory returned a negative row count",
        )
    })
}

async fn load_derived_visibility_rows_v1(
    connection: &mut SqliteConnection,
    generation: RadrootsEventStoreSourceGeneration,
) -> Result<Vec<VisibilityOracleFactV1>, RadrootsEventStoreError> {
    let rows = sqlx::query(
        "SELECT event.event_id, event.contract_status AS admission_status, event.contract_id, event.event_class, visibility.raw_d_tag, visibility.is_raw_head, visibility.raw_head_event_id, visibility.suppression_outcome, visibility.suppression_reason, visibility.event_reference_request_id, visibility.address_reference_request_id, visibility.address_reference_cutoff, visibility.current_visibility, visibility.source_generation FROM event_envelopes AS event JOIN radroots_event_store_current_visibility_v1 AS visibility ON visibility.event_id = event.event_id WHERE visibility.source_generation = ? ORDER BY event.seq",
    )
    .bind(generation.as_bytes().as_slice())
    .fetch_all(&mut *connection)
    .await?;
    let mut actual = Vec::with_capacity(rows.len());
    for row in rows {
        let stored_generation: Vec<u8> = row.try_get("source_generation")?;
        if stored_generation.as_slice() != generation.as_bytes().as_slice() {
            return rebuild_drift(
                RadrootsEventStoreRawSourceRebuildDriftV1::DerivedProductStateAuthority,
                "current visibility exposed a foreign source generation",
            );
        }
        actual.push(VisibilityOracleFactV1 {
            event_id: row.try_get("event_id")?,
            admission_status: row.try_get("admission_status")?,
            contract_id: row.try_get("contract_id")?,
            event_class: row.try_get("event_class")?,
            raw_d_tag: row.try_get("raw_d_tag")?,
            is_raw_head: row.try_get("is_raw_head")?,
            raw_head_event_id: row.try_get("raw_head_event_id")?,
            suppression_outcome: row.try_get("suppression_outcome")?,
            suppression_reason: row.try_get("suppression_reason")?,
            event_reference_request_id: row.try_get("event_reference_request_id")?,
            address_reference_request_id: row.try_get("address_reference_request_id")?,
            address_reference_cutoff: row.try_get("address_reference_cutoff")?,
            current_visibility: row.try_get("current_visibility")?,
        });
    }
    Ok(actual)
}

async fn validate_source_lineage_for_rebuild_v1(
    connection: &mut SqliteConnection,
    events: &[ReconciledEvent],
    terminal_transition_high_water: i64,
) -> Result<SourceState, RadrootsEventStoreError> {
    let state = read_source_state(connection).await?;
    validate_source_raw_authority_with_state(connection, &state).await?;
    validate_baseline_authority(connection, &state, events).await?;
    let rows = sqlx::query(
        "SELECT source_generation, generation_ordinal, reconciliation_version, addressable_feed_version, event_contract_registry_version, hook_id, hook_manifest_sha256, transition_floor_seq, baseline_raw_event_count, baseline_raw_tag_count, baseline_raw_high_water_seq FROM radroots_event_store_source_generation ORDER BY generation_ordinal",
    )
    .fetch_all(&mut *connection)
    .await?;
    let capacity = validate_source_capacity_authority_fast_v1(connection).await?;
    if rows.len() != usize::try_from(capacity.retained_generation_count()).unwrap_or(usize::MAX) {
        return rebuild_drift(
            RadrootsEventStoreRawSourceRebuildDriftV1::SourceGenerationLineage,
            "retained source-generation count does not match lineage rows",
        );
    }
    if rows.is_empty() {
        return rebuild_drift(
            RadrootsEventStoreRawSourceRebuildDriftV1::SourceGenerationLineage,
            "retained source-generation lineage is empty",
        );
    }
    let mut prior_baseline = (0_i64, 0_i64, 0_i64);
    for (index, row) in rows.iter().enumerate() {
        let ordinal: i64 = row.try_get("generation_ordinal")?;
        let expected_ordinal = i64::try_from(index + 1).map_err(|_| {
            rebuild_state_error(
                RadrootsEventStoreRawSourceRebuildDriftV1::SourceGenerationLineage,
                "generation lineage exceeds SQLite range",
            )
        })?;
        let generation = generation_from_blob(row.try_get("source_generation")?)?;
        let hook_id: String = row.try_get("hook_id")?;
        let hook_manifest_sha256: String = row.try_get("hook_manifest_sha256")?;
        reconciliation_profile(
            row.try_get("reconciliation_version")?,
            row.try_get("addressable_feed_version")?,
            row.try_get("event_contract_registry_version")?,
            hook_id.as_str(),
            hook_manifest_sha256.as_str(),
        )?;
        let floor: i64 = row.try_get("transition_floor_seq")?;
        let baseline_events: i64 = row.try_get("baseline_raw_event_count")?;
        let baseline_tags: i64 = row.try_get("baseline_raw_tag_count")?;
        let baseline_high_water: i64 = row.try_get("baseline_raw_high_water_seq")?;
        let expected_baseline_events =
            i64::try_from(events.partition_point(|event| event.seq <= baseline_high_water))
                .map_err(|_| {
                    rebuild_state_error(
                        RadrootsEventStoreRawSourceRebuildDriftV1::SourceGenerationLineage,
                        "historical raw baseline exceeds SQLite range",
                    )
                })?;
        let expected_baseline_high_water = if expected_baseline_events == 0 {
            0
        } else {
            let final_index = usize::try_from(expected_baseline_events - 1).map_err(|_| {
                rebuild_state_error(
                    RadrootsEventStoreRawSourceRebuildDriftV1::SourceGenerationLineage,
                    "historical raw baseline is negative",
                )
            })?;
            events
                .get(final_index)
                .map(|event| event.seq)
                .ok_or_else(|| {
                    rebuild_state_error(
                        RadrootsEventStoreRawSourceRebuildDriftV1::SourceGenerationLineage,
                        "historical raw baseline is out of range",
                    )
                })?
        };
        let expected_baseline_tags: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM event_envelope_tags AS tag JOIN event_envelopes AS event ON event.event_id = tag.event_id WHERE event.seq <= ?",
        )
        .bind(baseline_high_water)
        .fetch_one(&mut *connection)
        .await?;
        let transition_end: i64 = if let Some(next) = rows.get(index + 1) {
            next.try_get("transition_floor_seq")?
        } else {
            terminal_transition_high_water
        };
        let transition_bounds = sqlx::query(
            "SELECT COUNT(*) AS transition_count, MIN(transition_seq) AS first_transition_seq, MAX(transition_seq) AS last_transition_seq FROM radroots_event_store_addressable_head_transition WHERE source_generation = ?",
        )
        .bind(generation.as_bytes().as_slice())
        .fetch_one(&mut *connection)
        .await?;
        let transition_count: i64 = transition_bounds.try_get("transition_count")?;
        let first_transition_seq: Option<i64> =
            transition_bounds.try_get("first_transition_seq")?;
        let last_transition_seq: Option<i64> = transition_bounds.try_get("last_transition_seq")?;
        let expected_transition_count = transition_end.checked_sub(floor).ok_or_else(|| {
            rebuild_state_error(
                RadrootsEventStoreRawSourceRebuildDriftV1::AddressableTransitionAuthority,
                format!(
                    "source-generation lineage row {expected_ordinal} has an inverted transition interval"
                ),
            )
        })?;
        let expected_first_transition = if expected_transition_count == 0 {
            None
        } else {
            Some(floor.checked_add(1).ok_or_else(|| {
                rebuild_state_error(
                    RadrootsEventStoreRawSourceRebuildDriftV1::AddressableTransitionAuthority,
                    "historical transition sequence overflow",
                )
            })?)
        };
        let expected_last_transition = (expected_transition_count != 0).then_some(transition_end);
        if ordinal != expected_ordinal
            || baseline_events < 0
            || baseline_tags < 0
            || baseline_high_water < 0
            || baseline_events > state.raw_event_count
            || baseline_tags > state.raw_tag_count
            || baseline_high_water > state.raw_high_water_seq
            || baseline_events < prior_baseline.0
            || baseline_tags < prior_baseline.1
            || baseline_high_water < prior_baseline.2
            || baseline_events != expected_baseline_events
            || baseline_tags != expected_baseline_tags
            || baseline_high_water != expected_baseline_high_water
        {
            return rebuild_drift(
                RadrootsEventStoreRawSourceRebuildDriftV1::SourceGenerationLineage,
                format!("source-generation lineage row {expected_ordinal} is inconsistent"),
            );
        }
        if (index == 0 && floor != 0)
            || transition_count != expected_transition_count
            || first_transition_seq != expected_first_transition
            || last_transition_seq != expected_last_transition
        {
            return rebuild_drift(
                RadrootsEventStoreRawSourceRebuildDriftV1::AddressableTransitionAuthority,
                format!(
                    "source-generation lineage row {expected_ordinal} has inconsistent addressable transition authority"
                ),
            );
        }
        if index + 1 == rows.len() && generation != state.generation {
            return rebuild_drift(
                RadrootsEventStoreRawSourceRebuildDriftV1::SourceGenerationLineage,
                "active source generation is not the terminal lineage row",
            );
        }
        prior_baseline = (baseline_events, baseline_tags, baseline_high_water);
    }
    let transition_summary = sqlx::query(
        "SELECT COUNT(*) AS transition_count, MIN(transition_seq) AS first_transition_seq, MAX(transition_seq) AS last_transition_seq FROM radroots_event_store_addressable_head_transition",
    )
    .fetch_one(&mut *connection)
    .await?;
    let transition_count: i64 = transition_summary.try_get("transition_count")?;
    let first_transition_seq: Option<i64> = transition_summary.try_get("first_transition_seq")?;
    let last_transition_seq: Option<i64> = transition_summary.try_get("last_transition_seq")?;
    let expected_first_transition = (terminal_transition_high_water != 0).then_some(1);
    let expected_last_transition =
        (terminal_transition_high_water != 0).then_some(terminal_transition_high_water);
    if transition_count != terminal_transition_high_water
        || first_transition_seq != expected_first_transition
        || last_transition_seq != expected_last_transition
    {
        return rebuild_drift(
            RadrootsEventStoreRawSourceRebuildDriftV1::AddressableTransitionAuthority,
            "retained source-generation transition lineage has gaps or foreign rows",
        );
    }
    Ok(state)
}

async fn transition_high_water_v1(
    connection: &mut SqliteConnection,
) -> Result<i64, RadrootsEventStoreError> {
    Ok(sqlx::query_scalar(
        "SELECT COALESCE(MAX(transition_seq), 0) FROM radroots_event_store_addressable_head_transition",
    )
    .fetch_one(&mut *connection)
    .await?)
}

async fn prepare_transition_sqlite_sequence_v1(
    connection: &mut SqliteConnection,
    transition_max: i64,
) -> Result<i64, RadrootsEventStoreError> {
    if transition_max < 0 || transition_max == i64::MAX {
        return rebuild_drift(
            RadrootsEventStoreRawSourceRebuildDriftV1::AddressableTransitionAuthority,
            format!(
                "addressable transition sequence high-water {transition_max} cannot be normalized"
            ),
        );
    }
    let first: Option<(i64, Option<i64>)> = sqlx::query_as(
        "SELECT rowid, name = ? COLLATE NOCASE FROM main.sqlite_sequence ORDER BY rowid LIMIT 1",
    )
    .bind(TRANSITION_SEQUENCE_NAME)
    .fetch_optional(&mut *connection)
    .await?;
    let target_rowid = match first {
        None => -1,
        Some((rowid, Some(1))) => rowid,
        Some((i64::MIN, _)) => {
            return rebuild_drift(
                RadrootsEventStoreRawSourceRebuildDriftV1::AddressableTransitionAuthority,
                "unrelated sqlite_sequence authority exhausts target-first rowid space",
            );
        }
        Some((rowid, _)) => rowid - 1,
    };
    sqlx::query("DELETE FROM main.sqlite_sequence WHERE name COLLATE NOCASE = ?")
        .bind(TRANSITION_SEQUENCE_NAME)
        .execute(&mut *connection)
        .await?;
    sqlx::query("INSERT INTO main.sqlite_sequence(rowid, name, seq) VALUES (?, ?, ?)")
        .bind(target_rowid)
        .bind(TRANSITION_SEQUENCE_NAME)
        .bind(transition_max)
        .execute(&mut *connection)
        .await?;
    validate_transition_sqlite_sequence_v1(connection, target_rowid, transition_max).await?;
    Ok(target_rowid)
}

async fn validate_transition_sqlite_sequence_v1(
    connection: &mut SqliteConnection,
    target_rowid: i64,
    transition_max: i64,
) -> Result<(), RadrootsEventStoreError> {
    let normalized: Option<(String, Option<i64>)> =
        sqlx::query_as("SELECT name, seq FROM main.sqlite_sequence WHERE rowid = ?")
            .bind(target_rowid)
            .fetch_optional(&mut *connection)
            .await?;
    let first_rowid: Option<i64> =
        sqlx::query_scalar("SELECT rowid FROM main.sqlite_sequence ORDER BY rowid LIMIT 1")
            .fetch_optional(&mut *connection)
            .await?;
    if normalized != Some((TRANSITION_SEQUENCE_NAME.to_owned(), Some(transition_max)))
        || first_rowid != Some(target_rowid)
    {
        return rebuild_drift(
            RadrootsEventStoreRawSourceRebuildDriftV1::AddressableTransitionAuthority,
            "addressable transition sqlite_sequence target-first authority is inconsistent",
        );
    }
    Ok(())
}

async fn immutable_raw_digest_v1(
    connection: &mut SqliteConnection,
) -> Result<RadrootsEventStoreImmutableRawDigestV1, RadrootsEventStoreError> {
    let mut digest = Sha256::new();
    digest.update(IMMUTABLE_RAW_DIGEST_DOMAIN_V1);
    digest_section(&mut digest, b"event_envelopes")?;
    let mut events = sqlx::query(
        "SELECT seq, event_id, pubkey, created_at, kind, tags_json, content, sig, raw_json, inserted_at_ms FROM event_envelopes ORDER BY seq",
    )
    .fetch(&mut *connection);
    while let Some(row) = events.try_next().await? {
        digest_row_start(&mut digest);
        digest_i64(&mut digest, row.try_get("seq")?);
        digest_text(
            &mut digest,
            row.try_get::<String, _>("event_id")?.as_bytes(),
        )?;
        digest_text(&mut digest, row.try_get::<String, _>("pubkey")?.as_bytes())?;
        digest_i64(&mut digest, row.try_get("created_at")?);
        digest_i64(&mut digest, row.try_get("kind")?);
        digest_text(
            &mut digest,
            row.try_get::<String, _>("tags_json")?.as_bytes(),
        )?;
        digest_text(&mut digest, row.try_get::<String, _>("content")?.as_bytes())?;
        digest_text(&mut digest, row.try_get::<String, _>("sig")?.as_bytes())?;
        digest_text(
            &mut digest,
            row.try_get::<String, _>("raw_json")?.as_bytes(),
        )?;
        digest_i64(&mut digest, row.try_get("inserted_at_ms")?);
    }
    drop(events);
    digest_section(&mut digest, b"event_envelope_tags")?;
    let mut tags = sqlx::query(
        "SELECT event.seq, tag.event_id, tag.tag_index, tag.tag_name, tag.tag_value, tag.tag_json FROM event_envelope_tags AS tag JOIN event_envelopes AS event ON event.event_id = tag.event_id ORDER BY event.seq, tag.tag_index",
    )
    .fetch(&mut *connection);
    while let Some(row) = tags.try_next().await? {
        digest_row_start(&mut digest);
        digest_i64(&mut digest, row.try_get("seq")?);
        digest_text(
            &mut digest,
            row.try_get::<String, _>("event_id")?.as_bytes(),
        )?;
        digest_i64(&mut digest, row.try_get("tag_index")?);
        digest_text(
            &mut digest,
            row.try_get::<String, _>("tag_name")?.as_bytes(),
        )?;
        digest_optional_text(
            &mut digest,
            row.try_get::<Option<String>, _>("tag_value")?.as_deref(),
        )?;
        digest_text(
            &mut digest,
            row.try_get::<String, _>("tag_json")?.as_bytes(),
        )?;
    }
    drop(tags);
    Ok(RadrootsEventStoreImmutableRawDigestV1::from_bytes(
        digest.finalize().into(),
    ))
}

async fn active_product_state_digest_v1(
    connection: &mut SqliteConnection,
    generation: RadrootsEventStoreSourceGeneration,
) -> Result<RadrootsEventStoreActiveProductStateDigestV1, RadrootsEventStoreError> {
    let mut digest = Sha256::new();
    digest.update(ACTIVE_PRODUCT_STATE_DIGEST_DOMAIN_V1);
    digest_section(&mut digest, b"envelope_classification")?;
    let mut envelope_classification = sqlx::query(
        "SELECT event_id, verification_status, contract_status, contract_id, event_class, projection_eligible FROM event_envelopes ORDER BY event_id",
    )
    .fetch(&mut *connection);
    while let Some(row) = envelope_classification.try_next().await? {
        digest_row_start(&mut digest);
        digest_text_field(&mut digest, &row, "event_id")?;
        digest_text_field(&mut digest, &row, "verification_status")?;
        digest_text_field(&mut digest, &row, "contract_status")?;
        digest_optional_text_field(&mut digest, &row, "contract_id")?;
        digest_optional_text_field(&mut digest, &row, "event_class")?;
        digest_bool_field(&mut digest, &row, "projection_eligible")?;
    }
    drop(envelope_classification);

    digest_section(&mut digest, b"tag_classification")?;
    let mut tag_classification = sqlx::query(
        "SELECT event_id, tag_index, contract_semantic, contract_value_type, relay_indexed FROM event_envelope_tags ORDER BY event_id, tag_index",
    )
    .fetch(&mut *connection);
    while let Some(row) = tag_classification.try_next().await? {
        digest_row_start(&mut digest);
        digest_text_field(&mut digest, &row, "event_id")?;
        digest_i64_field(&mut digest, &row, "tag_index")?;
        digest_optional_text_field(&mut digest, &row, "contract_semantic")?;
        digest_optional_text_field(&mut digest, &row, "contract_value_type")?;
        digest_bool_field(&mut digest, &row, "relay_indexed")?;
    }
    drop(tag_classification);

    digest_section(&mut digest, b"raw_heads")?;
    let mut raw_heads = sqlx::query(
        "SELECT coordinate_type, kind, pubkey, d_tag, event_id, created_at FROM event_envelope_head ORDER BY coordinate_type, kind, pubkey, d_tag",
    )
    .fetch(&mut *connection);
    while let Some(row) = raw_heads.try_next().await? {
        digest_row_start(&mut digest);
        digest_text_field(&mut digest, &row, "coordinate_type")?;
        digest_i64_field(&mut digest, &row, "kind")?;
        digest_text_field(&mut digest, &row, "pubkey")?;
        digest_optional_text_field(&mut digest, &row, "d_tag")?;
        digest_text_field(&mut digest, &row, "event_id")?;
        digest_i64_field(&mut digest, &row, "created_at")?;
    }
    drop(raw_heads);

    digest_section(&mut digest, b"event_coordinates")?;
    let mut event_coordinates = sqlx::query(
        "SELECT event_id, coordinate_type, kind, pubkey, created_at, admission_status, admission_code, contract_id, raw_d_tag, nip09_matchable, nip09_d_tag FROM radroots_event_store_event_coordinate WHERE source_generation = ? ORDER BY event_id",
    )
    .bind(generation.as_bytes().as_slice())
    .fetch(&mut *connection);
    while let Some(row) = event_coordinates.try_next().await? {
        digest_row_start(&mut digest);
        digest_text_field(&mut digest, &row, "event_id")?;
        digest_text_field(&mut digest, &row, "coordinate_type")?;
        digest_i64_field(&mut digest, &row, "kind")?;
        digest_text_field(&mut digest, &row, "pubkey")?;
        digest_i64_field(&mut digest, &row, "created_at")?;
        digest_text_field(&mut digest, &row, "admission_status")?;
        digest_optional_text_field(&mut digest, &row, "admission_code")?;
        digest_optional_text_field(&mut digest, &row, "contract_id")?;
        digest_text_field(&mut digest, &row, "raw_d_tag")?;
        digest_bool_field(&mut digest, &row, "nip09_matchable")?;
        digest_optional_text_field(&mut digest, &row, "nip09_d_tag")?;
    }
    drop(event_coordinates);

    digest_section(&mut digest, b"nip09_requests")?;
    let mut nip09_requests = sqlx::query(
        "SELECT request_event_id, request_pubkey, request_created_at FROM radroots_event_store_nip09_request WHERE source_generation = ? ORDER BY request_event_id",
    )
    .bind(generation.as_bytes().as_slice())
    .fetch(&mut *connection);
    while let Some(row) = nip09_requests.try_next().await? {
        digest_row_start(&mut digest);
        digest_text_field(&mut digest, &row, "request_event_id")?;
        digest_text_field(&mut digest, &row, "request_pubkey")?;
        digest_i64_field(&mut digest, &row, "request_created_at")?;
    }
    drop(nip09_requests);

    digest_section(&mut digest, b"nip09_event_targets")?;
    let mut nip09_event_targets = sqlx::query(
        "SELECT request_event_id, target_event_id, source_tag_index, source_tag_value FROM radroots_event_store_nip09_event_target WHERE source_generation = ? ORDER BY request_event_id, target_event_id, source_tag_index",
    )
    .bind(generation.as_bytes().as_slice())
    .fetch(&mut *connection);
    while let Some(row) = nip09_event_targets.try_next().await? {
        digest_row_start(&mut digest);
        digest_text_field(&mut digest, &row, "request_event_id")?;
        digest_text_field(&mut digest, &row, "target_event_id")?;
        digest_i64_field(&mut digest, &row, "source_tag_index")?;
        digest_text_field(&mut digest, &row, "source_tag_value")?;
    }
    drop(nip09_event_targets);

    digest_section(&mut digest, b"nip09_address_targets")?;
    let mut nip09_address_targets = sqlx::query(
        "SELECT request_event_id, target_kind, target_pubkey, target_d_tag, inclusive_cutoff, source_tag_index, source_tag_value, source_kind_text, source_pubkey_text, source_d_tag FROM radroots_event_store_nip09_address_target WHERE source_generation = ? ORDER BY request_event_id, target_kind, target_pubkey, target_d_tag, source_tag_index",
    )
    .bind(generation.as_bytes().as_slice())
    .fetch(&mut *connection);
    while let Some(row) = nip09_address_targets.try_next().await? {
        digest_row_start(&mut digest);
        digest_text_field(&mut digest, &row, "request_event_id")?;
        digest_i64_field(&mut digest, &row, "target_kind")?;
        digest_text_field(&mut digest, &row, "target_pubkey")?;
        digest_text_field(&mut digest, &row, "target_d_tag")?;
        digest_i64_field(&mut digest, &row, "inclusive_cutoff")?;
        digest_i64_field(&mut digest, &row, "source_tag_index")?;
        digest_text_field(&mut digest, &row, "source_tag_value")?;
        digest_text_field(&mut digest, &row, "source_kind_text")?;
        digest_text_field(&mut digest, &row, "source_pubkey_text")?;
        digest_text_field(&mut digest, &row, "source_d_tag")?;
    }
    drop(nip09_address_targets);

    digest_section(&mut digest, b"addressable_heads")?;
    let mut addressable_heads = sqlx::query(
        "SELECT kind, pubkey, d_tag, raw_head_event_id, raw_head_created_at, admission_status, admission_code, contract_id, visibility, nip09_outcome, nip09_reason, event_reference_request_id, address_reference_request_id, address_reference_cutoff FROM radroots_event_store_addressable_head_state WHERE source_generation = ? ORDER BY kind, pubkey, d_tag",
    )
    .bind(generation.as_bytes().as_slice())
    .fetch(&mut *connection);
    while let Some(row) = addressable_heads.try_next().await? {
        digest_row_start(&mut digest);
        digest_i64_field(&mut digest, &row, "kind")?;
        digest_text_field(&mut digest, &row, "pubkey")?;
        digest_text_field(&mut digest, &row, "d_tag")?;
        digest_text_field(&mut digest, &row, "raw_head_event_id")?;
        digest_i64_field(&mut digest, &row, "raw_head_created_at")?;
        digest_text_field(&mut digest, &row, "admission_status")?;
        digest_optional_text_field(&mut digest, &row, "admission_code")?;
        digest_optional_text_field(&mut digest, &row, "contract_id")?;
        digest_text_field(&mut digest, &row, "visibility")?;
        digest_optional_text_field(&mut digest, &row, "nip09_outcome")?;
        digest_optional_text_field(&mut digest, &row, "nip09_reason")?;
        digest_optional_text_field(&mut digest, &row, "event_reference_request_id")?;
        digest_optional_text_field(&mut digest, &row, "address_reference_request_id")?;
        digest_optional_i64_field(&mut digest, &row, "address_reference_cutoff")?;
    }
    drop(addressable_heads);

    digest_section(&mut digest, b"current_visibility")?;
    let mut current_visibility = sqlx::query(
        "SELECT event_id, admission_status, contract_id, event_class, raw_d_tag, is_raw_head, raw_head_event_id, suppression_outcome, suppression_reason, event_reference_request_id, address_reference_request_id, address_reference_cutoff, current_visibility FROM radroots_event_store_current_visibility_v1 WHERE source_generation = ? ORDER BY event_id",
    )
    .bind(generation.as_bytes().as_slice())
    .fetch(&mut *connection);
    while let Some(row) = current_visibility.try_next().await? {
        digest_row_start(&mut digest);
        digest_text_field(&mut digest, &row, "event_id")?;
        digest_text_field(&mut digest, &row, "admission_status")?;
        digest_optional_text_field(&mut digest, &row, "contract_id")?;
        digest_text_field(&mut digest, &row, "event_class")?;
        digest_optional_text_field(&mut digest, &row, "raw_d_tag")?;
        digest_bool_field(&mut digest, &row, "is_raw_head")?;
        digest_optional_text_field(&mut digest, &row, "raw_head_event_id")?;
        digest_optional_text_field(&mut digest, &row, "suppression_outcome")?;
        digest_optional_text_field(&mut digest, &row, "suppression_reason")?;
        digest_optional_text_field(&mut digest, &row, "event_reference_request_id")?;
        digest_optional_text_field(&mut digest, &row, "address_reference_request_id")?;
        digest_optional_i64_field(&mut digest, &row, "address_reference_cutoff")?;
        digest_text_field(&mut digest, &row, "current_visibility")?;
    }
    drop(current_visibility);

    digest_section(&mut digest, b"food_projection")?;
    let mut food_projection = sqlx::query(
        "SELECT kind, pubkey, d_tag, event_id, created_at, contract_id, content, title, summary, published_at, location, price_amount, price_currency, price_unit, quantity_amount, quantity_unit, status, diagnostic_codes_json FROM radroots_event_store_food_availability_projection WHERE source_generation = ? ORDER BY pubkey, d_tag",
    )
    .bind(generation.as_bytes().as_slice())
    .fetch(&mut *connection);
    while let Some(row) = food_projection.try_next().await? {
        digest_row_start(&mut digest);
        digest_i64_field(&mut digest, &row, "kind")?;
        for field in ["pubkey", "d_tag", "event_id"] {
            digest_text_field(&mut digest, &row, field)?;
        }
        digest_i64_field(&mut digest, &row, "created_at")?;
        for field in ["contract_id", "content", "title", "summary"] {
            digest_text_field(&mut digest, &row, field)?;
        }
        digest_i64_field(&mut digest, &row, "published_at")?;
        for field in ["location", "price_amount", "price_currency", "price_unit"] {
            digest_text_field(&mut digest, &row, field)?;
        }
        digest_optional_text_field(&mut digest, &row, "quantity_amount")?;
        digest_optional_text_field(&mut digest, &row, "quantity_unit")?;
        digest_text_field(&mut digest, &row, "status")?;
        digest_text_field(&mut digest, &row, "diagnostic_codes_json")?;
    }
    drop(food_projection);

    digest_section(&mut digest, b"food_images")?;
    let mut food_images = sqlx::query(
        "SELECT pubkey, d_tag, image_index, raw_tag_json, url, width, height, blossom_sha256, qualifies, diagnostic_codes_json FROM radroots_event_store_food_availability_image WHERE source_generation = ? ORDER BY pubkey, d_tag, image_index",
    )
    .bind(generation.as_bytes().as_slice())
    .fetch(&mut *connection);
    while let Some(row) = food_images.try_next().await? {
        digest_row_start(&mut digest);
        digest_text_field(&mut digest, &row, "pubkey")?;
        digest_text_field(&mut digest, &row, "d_tag")?;
        digest_i64_field(&mut digest, &row, "image_index")?;
        digest_text_field(&mut digest, &row, "raw_tag_json")?;
        digest_optional_text_field(&mut digest, &row, "url")?;
        digest_optional_i64_field(&mut digest, &row, "width")?;
        digest_optional_i64_field(&mut digest, &row, "height")?;
        digest_optional_text_field(&mut digest, &row, "blossom_sha256")?;
        digest_bool_field(&mut digest, &row, "qualifies")?;
        digest_text_field(&mut digest, &row, "diagnostic_codes_json")?;
    }
    drop(food_images);

    digest_section(&mut digest, b"food_search")?;
    let mut food_search = sqlx::query(
        "SELECT event_id, pubkey, d_tag, title, summary, content, location FROM radroots_event_store_food_availability_search_fts ORDER BY event_id",
    )
    .fetch(&mut *connection);
    while let Some(row) = food_search.try_next().await? {
        digest_row_start(&mut digest);
        for field in [
            "event_id", "pubkey", "d_tag", "title", "summary", "content", "location",
        ] {
            digest_text_field(&mut digest, &row, field)?;
        }
    }
    drop(food_search);

    digest_section(&mut digest, b"food_cursor")?;
    let mut food_cursor = sqlx::query(
        "SELECT feed_version, projection_version, scope_fingerprint, hook_manifest_sha256, projected_row_count FROM radroots_event_store_food_availability_cursor WHERE singleton = 1",
    )
    .fetch(&mut *connection);
    while let Some(row) = food_cursor.try_next().await? {
        digest_row_start(&mut digest);
        digest_i64_field(&mut digest, &row, "feed_version")?;
        digest_i64_field(&mut digest, &row, "projection_version")?;
        digest_blob_field(&mut digest, &row, "scope_fingerprint")?;
        digest_text_field(&mut digest, &row, "hook_manifest_sha256")?;
        digest_i64_field(&mut digest, &row, "projected_row_count")?;
    }
    drop(food_cursor);
    Ok(RadrootsEventStoreActiveProductStateDigestV1::from_bytes(
        digest.finalize().into(),
    ))
}

fn digest_section(digest: &mut Sha256, name: &[u8]) -> Result<(), RadrootsEventStoreError> {
    digest.update(b"S");
    digest_bytes(digest, b'N', name)
}

fn digest_row_start(digest: &mut Sha256) {
    digest.update(b"R");
}

fn digest_i64(digest: &mut Sha256, value: i64) {
    digest.update(b"I");
    digest.update(value.to_be_bytes());
}

fn digest_bytes(
    digest: &mut Sha256,
    marker: u8,
    value: &[u8],
) -> Result<(), RadrootsEventStoreError> {
    let length = u64::try_from(value.len()).map_err(|_| {
        rebuild_state_error(
            RadrootsEventStoreRawSourceRebuildDriftV1::RebuildPostcondition,
            "digest field length exceeds u64",
        )
    })?;
    digest.update([marker]);
    digest.update(length.to_be_bytes());
    digest.update(value);
    Ok(())
}

fn digest_text(digest: &mut Sha256, value: &[u8]) -> Result<(), RadrootsEventStoreError> {
    digest_bytes(digest, b'T', value)
}

fn digest_optional_text(
    digest: &mut Sha256,
    value: Option<&str>,
) -> Result<(), RadrootsEventStoreError> {
    match value {
        Some(value) => {
            digest.update([b'O', 1]);
            digest_text(digest, value.as_bytes())
        }
        None => {
            digest.update([b'O', 0]);
            Ok(())
        }
    }
}

fn digest_optional_i64(digest: &mut Sha256, value: Option<i64>) {
    match value {
        Some(value) => {
            digest.update([b'O', 1]);
            digest_i64(digest, value);
        }
        None => digest.update([b'O', 0]),
    }
}

fn digest_bool(digest: &mut Sha256, value: i64) -> Result<(), RadrootsEventStoreError> {
    match value {
        0 | 1 => {
            digest.update([b'B', if value == 0 { 0 } else { 1 }]);
            Ok(())
        }
        _ => rebuild_drift(
            RadrootsEventStoreRawSourceRebuildDriftV1::DerivedProductStateAuthority,
            format!("digest boolean field has invalid value {value}"),
        ),
    }
}

fn digest_text_field(
    digest: &mut Sha256,
    row: &sqlx::sqlite::SqliteRow,
    field: &'static str,
) -> Result<(), RadrootsEventStoreError> {
    let value: String = row.try_get(field)?;
    digest_text(digest, value.as_bytes())
}

fn digest_optional_text_field(
    digest: &mut Sha256,
    row: &sqlx::sqlite::SqliteRow,
    field: &'static str,
) -> Result<(), RadrootsEventStoreError> {
    let value: Option<String> = row.try_get(field)?;
    digest_optional_text(digest, value.as_deref())
}

fn digest_i64_field(
    digest: &mut Sha256,
    row: &sqlx::sqlite::SqliteRow,
    field: &'static str,
) -> Result<(), RadrootsEventStoreError> {
    digest_i64(digest, row.try_get(field)?);
    Ok(())
}

fn digest_optional_i64_field(
    digest: &mut Sha256,
    row: &sqlx::sqlite::SqliteRow,
    field: &'static str,
) -> Result<(), RadrootsEventStoreError> {
    digest_optional_i64(digest, row.try_get(field)?);
    Ok(())
}

fn digest_bool_field(
    digest: &mut Sha256,
    row: &sqlx::sqlite::SqliteRow,
    field: &'static str,
) -> Result<(), RadrootsEventStoreError> {
    digest_bool(digest, row.try_get(field)?)
}

fn digest_blob_field(
    digest: &mut Sha256,
    row: &sqlx::sqlite::SqliteRow,
    field: &'static str,
) -> Result<(), RadrootsEventStoreError> {
    let value: Vec<u8> = row.try_get(field)?;
    digest_bytes(digest, b'X', &value)
}

async fn validate_scoped_integrity_v1(
    connection: &mut SqliteConnection,
) -> Result<(), RadrootsEventStoreError> {
    for table in REBUILD_OWNED_TABLES_V1 {
        let integrity_sql = format!("PRAGMA main.integrity_check('{table}')");
        let rows = sqlx::query(sqlx::AssertSqlSafe(integrity_sql))
            .fetch_all(&mut *connection)
            .await?;
        for row in rows {
            let detail: String = row.try_get(0)?;
            if detail != "ok" {
                return Err(RadrootsEventStoreError::IntegrityCheckFailed { detail });
            }
        }

        let foreign_key_sql = format!("PRAGMA main.foreign_key_check('{table}')");
        if let Some(row) = sqlx::query(sqlx::AssertSqlSafe(foreign_key_sql))
            .fetch_optional(&mut *connection)
            .await?
        {
            return Err(RadrootsEventStoreError::ForeignKeyViolation {
                table: row.try_get("table")?,
                rowid: row.try_get("rowid")?,
                parent: row.try_get("parent")?,
                foreign_key_index: row.try_get("fkid")?,
            });
        }
    }
    sqlx::query(
        "INSERT INTO radroots_event_store_food_availability_search_fts(radroots_event_store_food_availability_search_fts) VALUES('integrity-check')",
    )
    .execute(&mut *connection)
    .await
    .map_err(|source| RadrootsEventStoreError::Fts5IntegrityCheckFailed {
        table: "radroots_event_store_food_availability_search_fts",
        source,
    })?;
    Ok(())
}

async fn finish_raw_source_rebuild_transaction(
    transaction: Transaction<'_, Sqlite>,
    result: Result<RadrootsEventStoreRawSourceRebuildReportV1, RadrootsEventStoreError>,
) -> Result<RadrootsEventStoreRawSourceRebuildReportV1, RadrootsEventStoreError> {
    match result {
        Ok(report) => {
            transaction.commit().await?;
            Ok(report)
        }
        Err(primary) => {
            let rollback = transaction.rollback().await;
            preserve_raw_source_rebuild_primary_failure(primary, rollback)
        }
    }
}

fn preserve_raw_source_rebuild_primary_failure<T>(
    primary: RadrootsEventStoreError,
    rollback: Result<(), sqlx::Error>,
) -> Result<T, RadrootsEventStoreError> {
    match rollback {
        Ok(()) => Err(primary),
        Err(rollback) => Err(
            RadrootsEventStoreError::RawSourceRebuildTransactionRollbackFailed {
                primary: Box::new(primary),
                rollback,
            },
        ),
    }
}

#[cfg(test)]
fn inject_raw_source_rebuild_failpoint_v1(
    selected: Option<RawSourceRebuildFailpointV1>,
    stage: RawSourceRebuildFailpointV1,
) -> Result<(), RadrootsEventStoreError> {
    if selected == Some(stage) {
        return rebuild_drift(
            RadrootsEventStoreRawSourceRebuildDriftV1::RebuildPostcondition,
            format!("injected raw-source rebuild failure at {}", stage.as_str()),
        );
    }
    Ok(())
}

fn rebuild_state_error(
    kind: RadrootsEventStoreRawSourceRebuildDriftV1,
    detail: impl Into<String>,
) -> RadrootsEventStoreError {
    RadrootsEventStoreError::RawSourceRebuildStateDrift {
        kind,
        detail: detail.into(),
    }
}

fn rebuild_drift<T>(
    kind: RadrootsEventStoreRawSourceRebuildDriftV1,
    detail: impl Into<String>,
) -> Result<T, RadrootsEventStoreError> {
    Err(rebuild_state_error(kind, detail))
}

#[cfg(test)]
pub(crate) fn preserve_raw_source_rebuild_primary_failure_for_test<T>(
    primary: RadrootsEventStoreError,
    rollback: Result<(), sqlx::Error>,
) -> Result<T, RadrootsEventStoreError> {
    preserve_raw_source_rebuild_primary_failure(primary, rollback)
}
