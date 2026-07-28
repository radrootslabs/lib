use crate::migrations::{
    EVENT_STORE_LEDGER_CREATE_DDL, EVENT_STORE_LEDGER_DDL, EVENT_STORE_LEDGER_NAME,
    EVENT_STORE_MIGRATIONS, EventStoreMigration, EventStoreMigrationHook,
    RADROOTS_EVENT_STORE_SCHEMA_VERSION_CURRENT, RADROOTS_EVENT_STORE_SCHEMA_VERSION_MIN,
    is_event_store_governed_schema_name, is_event_store_owned_table_name, migration_for_version,
    sqlite_identifier_starts_with, validate_embedded_migration_registry,
    validate_migration_registry,
};
use crate::{RadrootsEventStoreError, RadrootsEventStoreRawSourceRebuildDriftV1};
use sha2::{Digest, Sha256};
use sqlx::{Row, Sqlite, SqliteConnection, SqlitePool, Transaction};
use std::collections::{BTreeMap, BTreeSet};

use crate::nip09::reconciliation_v1::{
    OsSourceGenerationProvider, ReconciliationCapacityLimits, SourceGenerationProvider,
    apply_reconciliation_hook, validate_active_hook_state_fast, validate_reconciliation_capacity,
};
use crate::source_maintenance_v1::{
    apply_source_maintenance_hook_v1, validate_no_persisted_ephemeral_raw_rows_v1,
    validate_source_capacity_authority_full_v1,
};
use crate::store::food_availability_projection_v1::{
    apply_food_availability_projection_hook_v1,
    validate_food_availability_projection_hook_state_fast_v1,
};

const RAW_SOURCE_REBUILD_SCHEMA_VERSION_V1: u32 = 4;

#[cfg(test)]
const EMPTY_SCHEMA_SHA256: &str =
    "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum RadrootsEventStoreSchemaStatus {
    Uninitialized,
    UnledgeredBaseline,
    Managed { version: u32 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CatalogRow {
    object_type: String,
    name: String,
    table_name: String,
    sql: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AppliedMigration {
    version: i64,
    name: String,
    up_sha256: String,
    down_sha256: String,
    schema_sha256: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SourceGenerationHistoryRollbackPolicy {
    Preserve,
    #[cfg(test)]
    AllowDestructiveForMigrationTest,
}

pub async fn inspect_event_store_schema_status(
    pool: &SqlitePool,
) -> Result<RadrootsEventStoreSchemaStatus, RadrootsEventStoreError> {
    validate_embedded_migration_registry()?;
    inspect_event_store_schema_status_with_registry(
        pool,
        EVENT_STORE_MIGRATIONS,
        RADROOTS_EVENT_STORE_SCHEMA_VERSION_CURRENT,
    )
    .await
}

async fn inspect_event_store_schema_status_with_registry(
    pool: &SqlitePool,
    registry: &[EventStoreMigration],
    supported_current: u32,
) -> Result<RadrootsEventStoreSchemaStatus, RadrootsEventStoreError> {
    let mut transaction = pool.begin().await?;
    let result = inspect_schema_on_connection(&mut transaction, registry, supported_current).await;
    finish_schema_transaction(transaction, result).await
}

pub(crate) async fn migrate_event_store_schema(
    pool: &SqlitePool,
) -> Result<(), RadrootsEventStoreError> {
    migrate_event_store_schema_with_generation_provider(pool, &OsSourceGenerationProvider).await
}

/// Validates the exact current managed catalog and ledger without consulting
/// derived hook state. Raw-source repair uses this before it starts replacing
/// derived authority; ordinary open continues through the stricter hook path.
pub(crate) async fn validate_exact_managed_v4_for_raw_source_rebuild_v1(
    connection: &mut SqliteConnection,
) -> Result<(), RadrootsEventStoreError> {
    validate_embedded_migration_registry()?;
    validate_repair_temp_schema_bounded_v1(connection, EVENT_STORE_MIGRATIONS).await?;
    let catalog = read_repair_catalog_bounded_v1(connection, EVENT_STORE_MIGRATIONS).await?;
    if !validate_ledger_catalog(&catalog)? {
        return Err(RadrootsEventStoreError::RawSourceRebuildStateDrift {
            kind: RadrootsEventStoreRawSourceRebuildDriftV1::ManagedSchemaAuthority,
            detail: "maintenance repair requires an exact managed-v4 migration ledger".to_owned(),
        });
    }
    let history =
        read_repair_history_bounded_v1(connection, RAW_SOURCE_REBUILD_SCHEMA_VERSION_V1).await?;
    let current = validate_history_against_registry(
        &history,
        EVENT_STORE_MIGRATIONS,
        RAW_SOURCE_REBUILD_SCHEMA_VERSION_V1,
    )?;
    if current != RAW_SOURCE_REBUILD_SCHEMA_VERSION_V1 {
        return Err(RadrootsEventStoreError::RawSourceRebuildStateDrift {
            kind: RadrootsEventStoreRawSourceRebuildDriftV1::ManagedSchemaAuthority,
            detail: format!(
                "maintenance repair requires managed schema version {}, found {current}",
                RAW_SOURCE_REBUILD_SCHEMA_VERSION_V1
            ),
        });
    }
    let migration =
        migration_for_version(EVENT_STORE_MIGRATIONS, RAW_SOURCE_REBUILD_SCHEMA_VERSION_V1).ok_or(
            RadrootsEventStoreError::UnknownMigration {
                version: RAW_SOURCE_REBUILD_SCHEMA_VERSION_V1,
            },
        )?;
    let actual = catalog_fingerprint(&governed_catalog(&catalog, EVENT_STORE_MIGRATIONS));
    if actual != migration.schema_sha256 {
        return Err(RadrootsEventStoreError::SchemaFingerprintMismatch {
            version: migration.version,
            expected: migration.schema_sha256,
            actual,
        });
    }
    Ok(())
}

pub(crate) async fn migrate_event_store_schema_with_generation_provider(
    pool: &SqlitePool,
    generation_provider: &dyn SourceGenerationProvider,
) -> Result<(), RadrootsEventStoreError> {
    migrate_event_store_schema_with_generation_provider_and_limits_inner(
        pool,
        generation_provider,
        ReconciliationCapacityLimits::production(),
    )
    .await
}

#[cfg(test)]
pub(crate) async fn migrate_event_store_schema_with_generation_provider_and_limits(
    pool: &SqlitePool,
    generation_provider: &dyn SourceGenerationProvider,
    reconciliation_limits: ReconciliationCapacityLimits,
) -> Result<(), RadrootsEventStoreError> {
    migrate_event_store_schema_with_generation_provider_and_limits_inner(
        pool,
        generation_provider,
        reconciliation_limits,
    )
    .await
}

async fn migrate_event_store_schema_with_generation_provider_and_limits_inner(
    pool: &SqlitePool,
    generation_provider: &dyn SourceGenerationProvider,
    reconciliation_limits: ReconciliationCapacityLimits,
) -> Result<(), RadrootsEventStoreError> {
    validate_embedded_migration_registry()?;
    migrate_event_store_schema_with_registry_and_generation_provider(
        pool,
        EVENT_STORE_MIGRATIONS,
        RADROOTS_EVENT_STORE_SCHEMA_VERSION_MIN,
        RADROOTS_EVENT_STORE_SCHEMA_VERSION_CURRENT,
        generation_provider,
        reconciliation_limits,
    )
    .await
}

#[cfg(test)]
async fn migrate_event_store_schema_with_registry(
    pool: &SqlitePool,
    registry: &[EventStoreMigration],
    minimum: u32,
    supported_current: u32,
) -> Result<(), RadrootsEventStoreError> {
    migrate_event_store_schema_with_registry_and_generation_provider(
        pool,
        registry,
        minimum,
        supported_current,
        &OsSourceGenerationProvider,
        ReconciliationCapacityLimits::production(),
    )
    .await
}

async fn migrate_event_store_schema_with_registry_and_generation_provider(
    pool: &SqlitePool,
    registry: &[EventStoreMigration],
    minimum: u32,
    supported_current: u32,
    generation_provider: &dyn SourceGenerationProvider,
    reconciliation_limits: ReconciliationCapacityLimits,
) -> Result<(), RadrootsEventStoreError> {
    validate_migration_registry(registry, minimum, supported_current)?;
    let status =
        inspect_event_store_schema_status_with_registry(pool, registry, supported_current).await?;
    if status
        == (RadrootsEventStoreSchemaStatus::Managed {
            version: supported_current,
        })
    {
        return Ok(());
    }
    if has_pending_source_capacity_hook(&status, registry) {
        let mut connection = pool.acquire().await?;
        validate_event_store_temp_schema_with_registry(&mut connection, registry).await?;
        validate_reconciliation_capacity(&mut connection, reconciliation_limits).await?;
        if has_pending_source_maintenance_hook(&status, registry) {
            validate_no_persisted_ephemeral_raw_rows_v1(&mut connection).await?;
        }
    }

    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
    let result = migrate_schema_on_connection(
        &mut transaction,
        registry,
        supported_current,
        generation_provider,
        reconciliation_limits,
    )
    .await;
    finish_schema_transaction(transaction, result).await
}

fn has_pending_source_capacity_hook(
    status: &RadrootsEventStoreSchemaStatus,
    registry: &[EventStoreMigration],
) -> bool {
    let current_version = match status {
        RadrootsEventStoreSchemaStatus::Uninitialized => return false,
        RadrootsEventStoreSchemaStatus::UnledgeredBaseline => registry[0].version,
        RadrootsEventStoreSchemaStatus::Managed { version } => *version,
    };
    registry.iter().any(|migration| {
        migration.version > current_version
            && matches!(
                migration.hook,
                EventStoreMigrationHook::Nip09ReconciliationV1
                    | EventStoreMigrationHook::FoodAvailabilityProjectionV1
                    | EventStoreMigrationHook::SourceMaintenanceV1
            )
    })
}

fn has_pending_source_maintenance_hook(
    status: &RadrootsEventStoreSchemaStatus,
    registry: &[EventStoreMigration],
) -> bool {
    let current_version = match status {
        RadrootsEventStoreSchemaStatus::Uninitialized => return false,
        RadrootsEventStoreSchemaStatus::UnledgeredBaseline => registry[0].version,
        RadrootsEventStoreSchemaStatus::Managed { version } => *version,
    };
    registry.iter().any(|migration| {
        migration.version > current_version
            && migration.hook == EventStoreMigrationHook::SourceMaintenanceV1
    })
}

pub(crate) async fn rollback_event_store_schema_offline(
    pool: &SqlitePool,
    target: u32,
) -> Result<(), RadrootsEventStoreError> {
    rollback_event_store_schema_with_registry(
        pool,
        EVENT_STORE_MIGRATIONS,
        RADROOTS_EVENT_STORE_SCHEMA_VERSION_MIN,
        RADROOTS_EVENT_STORE_SCHEMA_VERSION_CURRENT,
        target,
    )
    .await
}

#[cfg(test)]
pub(crate) async fn rollback_event_store_schema_offline_destructive_for_migration_test(
    pool: &SqlitePool,
    target: u32,
) -> Result<(), RadrootsEventStoreError> {
    rollback_event_store_schema_with_registry_inner(
        pool,
        EVENT_STORE_MIGRATIONS,
        RADROOTS_EVENT_STORE_SCHEMA_VERSION_MIN,
        RADROOTS_EVENT_STORE_SCHEMA_VERSION_CURRENT,
        target,
        SourceGenerationHistoryRollbackPolicy::AllowDestructiveForMigrationTest,
    )
    .await
}

async fn rollback_event_store_schema_with_registry(
    pool: &SqlitePool,
    registry: &[EventStoreMigration],
    minimum: u32,
    supported_current: u32,
    target: u32,
) -> Result<(), RadrootsEventStoreError> {
    rollback_event_store_schema_with_registry_inner(
        pool,
        registry,
        minimum,
        supported_current,
        target,
        SourceGenerationHistoryRollbackPolicy::Preserve,
    )
    .await
}

async fn rollback_event_store_schema_with_registry_inner(
    pool: &SqlitePool,
    registry: &[EventStoreMigration],
    minimum: u32,
    supported_current: u32,
    target: u32,
    source_generation_history_policy: SourceGenerationHistoryRollbackPolicy,
) -> Result<(), RadrootsEventStoreError> {
    if target < minimum {
        return Err(RadrootsEventStoreError::RollbackBelowVersionFloor {
            floor: minimum,
            target,
        });
    }
    validate_migration_registry(registry, minimum, supported_current)?;
    let mut transaction = pool.begin_with("BEGIN EXCLUSIVE").await?;
    let result = rollback_schema_on_connection(
        &mut transaction,
        registry,
        supported_current,
        target,
        source_generation_history_policy,
    )
    .await;
    finish_schema_transaction(transaction, result).await
}

#[cfg(test)]
pub(crate) async fn destroy_event_store_schema_for_test(
    pool: &SqlitePool,
) -> Result<(), RadrootsEventStoreError> {
    validate_embedded_migration_registry()?;
    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
    let result = destroy_schema_on_connection(&mut transaction).await;
    finish_schema_transaction(transaction, result).await
}

async fn finish_schema_transaction<T>(
    transaction: Transaction<'static, Sqlite>,
    result: Result<T, RadrootsEventStoreError>,
) -> Result<T, RadrootsEventStoreError> {
    match result {
        Ok(value) => {
            transaction.commit().await?;
            Ok(value)
        }
        Err(error) => {
            let rollback = transaction.rollback().await;
            preserve_primary_failure(error, rollback)
        }
    }
}

fn preserve_primary_failure<T>(
    primary: RadrootsEventStoreError,
    rollback: Result<(), sqlx::Error>,
) -> Result<T, RadrootsEventStoreError> {
    match rollback {
        Ok(()) => Err(primary),
        Err(rollback) => Err(
            RadrootsEventStoreError::MigrationTransactionRollbackFailed {
                primary: Box::new(primary),
                rollback,
            },
        ),
    }
}

async fn migrate_schema_on_connection(
    connection: &mut SqliteConnection,
    registry: &[EventStoreMigration],
    supported_current: u32,
    generation_provider: &dyn SourceGenerationProvider,
    reconciliation_limits: ReconciliationCapacityLimits,
) -> Result<(), RadrootsEventStoreError> {
    let status = inspect_schema_on_connection(connection, registry, supported_current).await?;
    let current_version = match status {
        RadrootsEventStoreSchemaStatus::Uninitialized => {
            apply_migration_up(connection, registry, &registry[0]).await?;
            create_ledger(connection).await?;
            insert_ledger_row(connection, &registry[0]).await?;
            registry[0].version
        }
        RadrootsEventStoreSchemaStatus::UnledgeredBaseline => {
            create_ledger(connection).await?;
            insert_ledger_row(connection, &registry[0]).await?;
            registry[0].version
        }
        RadrootsEventStoreSchemaStatus::Managed { version } if version == supported_current => {
            return Ok(());
        }
        RadrootsEventStoreSchemaStatus::Managed { version } => version,
    };

    for migration in registry
        .iter()
        .filter(|migration| migration.version > current_version)
    {
        if matches!(
            migration.hook,
            EventStoreMigrationHook::Nip09ReconciliationV1
                | EventStoreMigrationHook::FoodAvailabilityProjectionV1
                | EventStoreMigrationHook::SourceMaintenanceV1
        ) {
            validate_reconciliation_capacity(connection, reconciliation_limits).await?;
            if migration.hook == EventStoreMigrationHook::SourceMaintenanceV1 {
                validate_no_persisted_ephemeral_raw_rows_v1(connection).await?;
            }
        }
        apply_migration_up(connection, registry, migration).await?;
        apply_migration_hook(
            connection,
            migration,
            generation_provider,
            reconciliation_limits,
        )
        .await?;
        validate_applied_migration_hooks(connection, registry, migration.version).await?;
        insert_ledger_row(connection, migration).await?;
    }

    validate_database_integrity(connection, registry).await?;
    match inspect_schema_on_connection(connection, registry, supported_current).await? {
        RadrootsEventStoreSchemaStatus::Managed { version } if version == supported_current => {
            Ok(())
        }
        status => Err(RadrootsEventStoreError::MigrationRegistryDefect {
            reason: format!("migration completed in unexpected state {status:?}"),
        }),
    }
}

async fn rollback_schema_on_connection(
    connection: &mut SqliteConnection,
    registry: &[EventStoreMigration],
    supported_current: u32,
    target: u32,
    source_generation_history_policy: SourceGenerationHistoryRollbackPolicy,
) -> Result<(), RadrootsEventStoreError> {
    let RadrootsEventStoreSchemaStatus::Managed {
        version: current_version,
    } = inspect_schema_on_connection(connection, registry, supported_current).await?
    else {
        return Err(RadrootsEventStoreError::RollbackUnmanaged);
    };
    if target > current_version {
        return Err(RadrootsEventStoreError::RollbackAhead {
            current: current_version,
            target,
        });
    }
    if source_generation_history_policy == SourceGenerationHistoryRollbackPolicy::Preserve {
        validate_rollback_preserves_source_generation_history(registry, current_version, target)?;
    }

    for version in ((target + 1)..=current_version).rev() {
        let migration = migration_for_version(registry, version)
            .ok_or(RadrootsEventStoreError::UnknownMigration { version })?;
        apply_migration_down(connection, migration).await?;
        let prior = migration_for_version(registry, version - 1).ok_or(
            RadrootsEventStoreError::MigrationHistoryGap {
                expected: version - 1,
                actual: None,
            },
        )?;
        validate_schema_fingerprint(connection, registry, prior).await?;
        let deleted = sqlx::query(
            "DELETE FROM main.radroots_event_store_schema_migrations WHERE version = ?",
        )
        .bind(i64::from(version))
        .execute(&mut *connection)
        .await?;
        if deleted.rows_affected() != 1 {
            return Err(RadrootsEventStoreError::MigrationLedgerDrift {
                reason: format!(
                    "rollback expected one ledger row for version {version}, deleted {}",
                    deleted.rows_affected()
                ),
            });
        }
        validate_applied_migration_hooks(connection, registry, prior.version).await?;
    }

    validate_database_integrity(connection, registry).await?;
    match inspect_schema_on_connection(connection, registry, supported_current).await? {
        RadrootsEventStoreSchemaStatus::Managed { version } if version == target => Ok(()),
        status => Err(RadrootsEventStoreError::MigrationLedgerDrift {
            reason: format!("rollback completed in unexpected state {status:?}"),
        }),
    }
}

fn validate_rollback_preserves_source_generation_history(
    registry: &[EventStoreMigration],
    current: u32,
    target: u32,
) -> Result<(), RadrootsEventStoreError> {
    let Some(floor) = registry
        .iter()
        .find(|migration| migration.hook == EventStoreMigrationHook::Nip09ReconciliationV1)
        .map(|migration| migration.version)
    else {
        return Ok(());
    };
    if current < floor || target >= floor {
        return Ok(());
    }

    Err(
        RadrootsEventStoreError::RollbackWouldDiscardSourceGenerationHistory {
            current,
            target,
            floor,
        },
    )
}

#[cfg(test)]
async fn destroy_schema_on_connection(
    connection: &mut SqliteConnection,
) -> Result<(), RadrootsEventStoreError> {
    match inspect_schema_on_connection(
        connection,
        EVENT_STORE_MIGRATIONS,
        RADROOTS_EVENT_STORE_SCHEMA_VERSION_CURRENT,
    )
    .await?
    {
        RadrootsEventStoreSchemaStatus::Managed { version } => {
            for version in (RADROOTS_EVENT_STORE_SCHEMA_VERSION_MIN..=version).rev() {
                let migration = migration_for_version(EVENT_STORE_MIGRATIONS, version)
                    .ok_or(RadrootsEventStoreError::UnknownMigration { version })?;
                apply_migration_down(connection, migration).await?;
                let deleted = sqlx::query(
                    "DELETE FROM main.radroots_event_store_schema_migrations WHERE version = ?",
                )
                .bind(i64::from(version))
                .execute(&mut *connection)
                .await?;
                if deleted.rows_affected() != 1 {
                    return Err(RadrootsEventStoreError::MigrationLedgerDrift {
                        reason: format!(
                            "destruction expected one ledger row for version {version}, deleted {}",
                            deleted.rows_affected()
                        ),
                    });
                }
            }
            validate_empty_governed_catalog(connection, EVENT_STORE_MIGRATIONS).await?;
            sqlx::query("DROP TABLE main.radroots_event_store_schema_migrations")
                .execute(&mut *connection)
                .await?;
        }
        RadrootsEventStoreSchemaStatus::UnledgeredBaseline => {
            apply_migration_down(connection, &EVENT_STORE_MIGRATIONS[0]).await?;
            validate_empty_governed_catalog(connection, EVENT_STORE_MIGRATIONS).await?;
        }
        RadrootsEventStoreSchemaStatus::Uninitialized => {}
    }
    validate_database_integrity(connection, EVENT_STORE_MIGRATIONS).await
}

async fn apply_migration_up(
    connection: &mut SqliteConnection,
    registry: &[EventStoreMigration],
    migration: &EventStoreMigration,
) -> Result<(), RadrootsEventStoreError> {
    let before = read_catalog(connection).await?;
    sqlx::raw_sql(migration.up_sql)
        .execute(&mut *connection)
        .await?;
    let after = read_catalog(connection).await?;
    validate_catalog_delta(&before, &after, migration, "up")?;
    validate_schema_fingerprint(connection, registry, migration).await
}

async fn apply_migration_down(
    connection: &mut SqliteConnection,
    migration: &EventStoreMigration,
) -> Result<(), RadrootsEventStoreError> {
    let before = read_catalog(connection).await?;
    sqlx::raw_sql(migration.down_sql)
        .execute(&mut *connection)
        .await?;
    let after = read_catalog(connection).await?;
    validate_catalog_delta(&before, &after, migration, "down")
}

fn validate_catalog_delta(
    before: &[CatalogRow],
    after: &[CatalogRow],
    migration: &EventStoreMigration,
    direction: &'static str,
) -> Result<(), RadrootsEventStoreError> {
    let before = before
        .iter()
        .map(|row| (row.name.as_str(), row))
        .collect::<BTreeMap<_, _>>();
    let after = after
        .iter()
        .map(|row| (row.name.as_str(), row))
        .collect::<BTreeMap<_, _>>();
    let added = after
        .keys()
        .filter(|name| !before.contains_key(**name))
        .copied()
        .collect::<BTreeSet<_>>();
    let removed = before
        .keys()
        .filter(|name| !after.contains_key(**name))
        .copied()
        .collect::<BTreeSet<_>>();
    let changed = before
        .iter()
        .filter_map(|(name, row)| {
            after
                .get(name)
                .filter(|after_row| *after_row != row)
                .map(|_| *name)
        })
        .collect::<BTreeSet<_>>();
    let expected = migration
        .owned_object_names
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let expected_changed = migration
        .replaced_object_names
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();

    let valid = match direction {
        "up" => added == expected && removed.is_empty() && changed == expected_changed,
        "down" => removed == expected && added.is_empty() && changed == expected_changed,
        _ => false,
    };
    if !valid {
        return Err(RadrootsEventStoreError::MigrationCatalogDeltaMismatch {
            version: migration.version,
            direction,
            reason: format!(
                "expected {} objects {expected:?} and changed replacement objects {expected_changed:?}; added {added:?}, removed {removed:?}, changed {changed:?}",
                if direction == "up" {
                    "added"
                } else {
                    "removed"
                }
            ),
        });
    }
    Ok(())
}

async fn create_ledger(connection: &mut SqliteConnection) -> Result<(), RadrootsEventStoreError> {
    sqlx::query(EVENT_STORE_LEDGER_CREATE_DDL)
        .execute(&mut *connection)
        .await?;
    validate_ledger_catalog(&read_catalog(connection).await?)?;
    Ok(())
}

async fn insert_ledger_row(
    connection: &mut SqliteConnection,
    migration: &EventStoreMigration,
) -> Result<(), RadrootsEventStoreError> {
    sqlx::query(
        "INSERT INTO main.radroots_event_store_schema_migrations(version, name, up_sha256, down_sha256, schema_sha256) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(i64::from(migration.version))
    .bind(migration.name)
    .bind(migration.up_sha256)
    .bind(migration.down_sha256)
    .bind(migration.schema_sha256)
    .execute(&mut *connection)
    .await?;
    Ok(())
}

async fn inspect_schema_on_connection(
    connection: &mut SqliteConnection,
    registry: &[EventStoreMigration],
    supported_current: u32,
) -> Result<RadrootsEventStoreSchemaStatus, RadrootsEventStoreError> {
    validate_event_store_temp_schema_with_registry(connection, registry).await?;
    let catalog = read_catalog(connection).await?;
    let has_ledger = validate_ledger_catalog(&catalog)?;
    let governed = governed_catalog(&catalog, registry);
    let actual_schema_sha256 = catalog_fingerprint(&governed);

    if !has_ledger {
        if governed.is_empty() {
            return Ok(RadrootsEventStoreSchemaStatus::Uninitialized);
        }
        let baseline = &registry[0];
        if governed.len() == baseline.owned_object_names.len()
            && actual_schema_sha256 == baseline.schema_sha256
        {
            return Ok(RadrootsEventStoreSchemaStatus::UnledgeredBaseline);
        }
        return Err(RadrootsEventStoreError::UnmanagedSchema {
            actual_schema_sha256,
        });
    }

    let history = read_history(connection).await?;
    let current = validate_history_against_registry(&history, registry, supported_current)?;
    let expected = migration_for_version(registry, current)
        .ok_or(RadrootsEventStoreError::UnknownMigration { version: current })?;
    if actual_schema_sha256 != expected.schema_sha256 {
        return Err(RadrootsEventStoreError::SchemaFingerprintMismatch {
            version: current,
            expected: expected.schema_sha256,
            actual: actual_schema_sha256,
        });
    }
    validate_applied_migration_hooks(connection, registry, current).await?;
    Ok(RadrootsEventStoreSchemaStatus::Managed { version: current })
}

pub(crate) async fn validate_event_store_temp_schema(
    connection: &mut SqliteConnection,
) -> Result<(), RadrootsEventStoreError> {
    validate_event_store_temp_schema_with_registry(connection, EVENT_STORE_MIGRATIONS).await
}

fn repair_governed_catalog_authority_v1(
    registry: &[EventStoreMigration],
) -> Result<(String, i64), RadrootsEventStoreError> {
    let mut names = registry
        .iter()
        .flat_map(|migration| migration.owned_object_names.iter().copied())
        .collect::<BTreeSet<_>>();
    names.insert(EVENT_STORE_LEDGER_NAME);
    let canonical_row_count = i64::try_from(names.len()).map_err(|_| {
        RadrootsEventStoreError::RawSourceRebuildStateDrift {
            kind: RadrootsEventStoreRawSourceRebuildDriftV1::ManagedSchemaAuthority,
            detail: "managed catalog authority exceeds the SQLite row-count range".to_owned(),
        }
    })?;
    let row_limit = canonical_row_count.checked_add(1).ok_or_else(|| {
        RadrootsEventStoreError::RawSourceRebuildStateDrift {
            kind: RadrootsEventStoreRawSourceRebuildDriftV1::ManagedSchemaAuthority,
            detail: "managed catalog authority cannot reserve a collision row".to_owned(),
        }
    })?;
    Ok((serde_json::to_string(&names)?, row_limit))
}

async fn read_repair_catalog_bounded_v1(
    connection: &mut SqliteConnection,
    registry: &[EventStoreMigration],
) -> Result<Vec<CatalogRow>, RadrootsEventStoreError> {
    let (governed_names_json, row_limit) = repair_governed_catalog_authority_v1(registry)?;
    let rows = sqlx::query(
        "WITH governed(name) AS (
           SELECT CAST(value AS TEXT) COLLATE NOCASE FROM json_each(?)
         )
         SELECT type, name, tbl_name, sql
         FROM main.sqlite_schema
         WHERE lower(substr(name, 1, 7)) != 'sqlite_'
           AND (
             name COLLATE NOCASE IN (SELECT name FROM governed)
             OR tbl_name COLLATE NOCASE IN (SELECT name FROM governed)
             OR lower(substr(name, 1, length(?))) = lower(?)
             OR lower(substr(tbl_name, 1, length(?))) = lower(?)
           )
         ORDER BY type, name, tbl_name
         LIMIT ?",
    )
    .bind(&governed_names_json)
    .bind(crate::migrations::EVENT_STORE_RESERVED_PREFIX)
    .bind(crate::migrations::EVENT_STORE_RESERVED_PREFIX)
    .bind(crate::migrations::EVENT_STORE_RESERVED_PREFIX)
    .bind(crate::migrations::EVENT_STORE_RESERVED_PREFIX)
    .bind(row_limit)
    .fetch_all(&mut *connection)
    .await?;
    rows.into_iter()
        .map(|row| {
            Ok(CatalogRow {
                object_type: row.try_get("type")?,
                name: row.try_get("name")?,
                table_name: row.try_get("tbl_name")?,
                sql: row.try_get("sql")?,
            })
        })
        .collect()
}

pub(crate) async fn validate_repair_temp_schema_bounded_v1(
    connection: &mut SqliteConnection,
    registry: &[EventStoreMigration],
) -> Result<(), RadrootsEventStoreError> {
    let (governed_names_json, _) = repair_governed_catalog_authority_v1(registry)?;
    let collision = sqlx::query(
        "WITH governed(name) AS (
           SELECT CAST(value AS TEXT) COLLATE NOCASE FROM json_each(?)
         )
         SELECT type, name, tbl_name
         FROM temp.sqlite_schema
         WHERE type IN ('trigger', 'view')
            OR name COLLATE NOCASE IN (SELECT name FROM governed)
            OR tbl_name COLLATE NOCASE IN (SELECT name FROM governed)
            OR lower(substr(name, 1, length(?))) = lower(?)
            OR lower(substr(tbl_name, 1, length(?))) = lower(?)
         ORDER BY type, name, tbl_name
         LIMIT 1",
    )
    .bind(&governed_names_json)
    .bind(crate::migrations::EVENT_STORE_RESERVED_PREFIX)
    .bind(crate::migrations::EVENT_STORE_RESERVED_PREFIX)
    .bind(crate::migrations::EVENT_STORE_RESERVED_PREFIX)
    .bind(crate::migrations::EVENT_STORE_RESERVED_PREFIX)
    .fetch_optional(&mut *connection)
    .await?;
    if let Some(row) = collision {
        return Err(RadrootsEventStoreError::TemporarySchemaCollision {
            object_type: row.try_get("type")?,
            name: row.try_get("name")?,
            table_name: row.try_get("tbl_name")?,
        });
    }
    Ok(())
}

async fn validate_event_store_temp_schema_with_registry(
    connection: &mut SqliteConnection,
    registry: &[EventStoreMigration],
) -> Result<(), RadrootsEventStoreError> {
    let rows = sqlx::query(
        "SELECT type, name, tbl_name FROM temp.sqlite_schema ORDER BY type, name, tbl_name",
    )
    .fetch_all(&mut *connection)
    .await?;
    for row in rows {
        let object_type: String = row.try_get("type")?;
        let name: String = row.try_get("name")?;
        let table_name: String = row.try_get("tbl_name")?;
        if matches!(object_type.as_str(), "trigger" | "view")
            || is_event_store_governed_schema_name(registry, &name)
            || is_event_store_governed_schema_name(registry, &table_name)
        {
            return Err(RadrootsEventStoreError::TemporarySchemaCollision {
                object_type,
                name,
                table_name,
            });
        }
    }
    Ok(())
}

async fn apply_migration_hook(
    connection: &mut SqliteConnection,
    migration: &EventStoreMigration,
    generation_provider: &dyn SourceGenerationProvider,
    reconciliation_limits: ReconciliationCapacityLimits,
) -> Result<(), RadrootsEventStoreError> {
    match migration.hook {
        EventStoreMigrationHook::None => Ok(()),
        EventStoreMigrationHook::Nip09ReconciliationV1 => {
            apply_reconciliation_hook(connection, generation_provider, reconciliation_limits).await
        }
        EventStoreMigrationHook::FoodAvailabilityProjectionV1 => {
            apply_food_availability_projection_hook_v1(connection).await
        }
        EventStoreMigrationHook::SourceMaintenanceV1 => {
            apply_source_maintenance_hook_v1(connection).await
        }
    }
}

async fn validate_migration_hook_state(
    connection: &mut SqliteConnection,
    migration: &EventStoreMigration,
) -> Result<(), RadrootsEventStoreError> {
    match migration.hook {
        EventStoreMigrationHook::None => Ok(()),
        EventStoreMigrationHook::Nip09ReconciliationV1 => {
            validate_active_hook_state_fast(connection).await
        }
        EventStoreMigrationHook::FoodAvailabilityProjectionV1 => {
            validate_food_availability_projection_hook_state_fast_v1(connection).await
        }
        EventStoreMigrationHook::SourceMaintenanceV1 => {
            validate_source_capacity_authority_full_v1(connection).await
        }
    }
}

async fn validate_applied_migration_hooks(
    connection: &mut SqliteConnection,
    registry: &[EventStoreMigration],
    current: u32,
) -> Result<(), RadrootsEventStoreError> {
    for migration in registry
        .iter()
        .filter(|migration| migration.version <= current)
    {
        validate_migration_hook_state(connection, migration).await?;
    }
    Ok(())
}

async fn read_catalog(
    connection: &mut SqliteConnection,
) -> Result<Vec<CatalogRow>, RadrootsEventStoreError> {
    let rows = sqlx::query("SELECT type, name, tbl_name, sql FROM main.sqlite_schema")
        .fetch_all(&mut *connection)
        .await?;
    let catalog = rows
        .into_iter()
        .map(|row| {
            Ok(CatalogRow {
                object_type: row.try_get("type")?,
                name: row.try_get("name")?,
                table_name: row.try_get("tbl_name")?,
                sql: row.try_get("sql")?,
            })
        })
        .collect::<Result<Vec<_>, RadrootsEventStoreError>>()?;
    Ok(catalog
        .into_iter()
        .filter(|row| !sqlite_identifier_starts_with(&row.name, "sqlite_"))
        .collect())
}

fn validate_ledger_catalog(catalog: &[CatalogRow]) -> Result<bool, RadrootsEventStoreError> {
    let rows = catalog
        .iter()
        .filter(|row| {
            row.name.eq_ignore_ascii_case(EVENT_STORE_LEDGER_NAME)
                || row.table_name.eq_ignore_ascii_case(EVENT_STORE_LEDGER_NAME)
        })
        .collect::<Vec<_>>();
    if rows.is_empty() {
        return Ok(false);
    }
    if rows.len() != 1 {
        return Err(RadrootsEventStoreError::MigrationLedgerDrift {
            reason: format!(
                "expected exactly one non-internal ledger catalog object, found {}",
                rows.len()
            ),
        });
    }
    let row = rows[0];
    if row.object_type != "table"
        || row.name != EVENT_STORE_LEDGER_NAME
        || row.table_name != EVENT_STORE_LEDGER_NAME
        || row.sql.as_deref() != Some(EVENT_STORE_LEDGER_DDL)
    {
        return Err(RadrootsEventStoreError::MigrationLedgerDrift {
            reason: "ledger table definition does not match the canonical catalog SQL".to_owned(),
        });
    }
    Ok(true)
}

fn governed_catalog(catalog: &[CatalogRow], registry: &[EventStoreMigration]) -> Vec<CatalogRow> {
    catalog
        .iter()
        .filter(|row| !row.name.eq_ignore_ascii_case(EVENT_STORE_LEDGER_NAME))
        .filter(|row| {
            is_event_store_governed_schema_name(registry, &row.name)
                || is_event_store_governed_schema_name(registry, &row.table_name)
        })
        .cloned()
        .collect()
}

fn catalog_fingerprint(catalog: &[CatalogRow]) -> String {
    let mut rows = catalog.to_vec();
    rows.sort_by(|left, right| {
        (
            left.object_type.as_bytes(),
            left.name.as_bytes(),
            left.table_name.as_bytes(),
            left.sql.as_deref().unwrap_or("").as_bytes(),
        )
            .cmp(&(
                right.object_type.as_bytes(),
                right.name.as_bytes(),
                right.table_name.as_bytes(),
                right.sql.as_deref().unwrap_or("").as_bytes(),
            ))
    });

    let mut digest = Sha256::new();
    for row in rows {
        for field in [
            row.object_type.as_str(),
            row.name.as_str(),
            row.table_name.as_str(),
            row.sql.as_deref().unwrap_or(""),
        ] {
            digest.update(field.as_bytes());
            digest.update([0]);
        }
    }
    hex::encode(digest.finalize())
}

async fn read_history(
    connection: &mut SqliteConnection,
) -> Result<Vec<AppliedMigration>, RadrootsEventStoreError> {
    let rows = sqlx::query(
        "SELECT version, name, up_sha256, down_sha256, schema_sha256 FROM main.radroots_event_store_schema_migrations ORDER BY version",
    )
    .fetch_all(&mut *connection)
    .await?;
    rows.into_iter()
        .map(|row| {
            Ok(AppliedMigration {
                version: row.try_get("version")?,
                name: row.try_get("name")?,
                up_sha256: row.try_get("up_sha256")?,
                down_sha256: row.try_get("down_sha256")?,
                schema_sha256: row.try_get("schema_sha256")?,
            })
        })
        .collect()
}

async fn read_repair_history_bounded_v1(
    connection: &mut SqliteConnection,
    supported_current: u32,
) -> Result<Vec<AppliedMigration>, RadrootsEventStoreError> {
    let row_limit = i64::from(supported_current).checked_add(1).ok_or_else(|| {
        RadrootsEventStoreError::RawSourceRebuildStateDrift {
            kind: RadrootsEventStoreRawSourceRebuildDriftV1::ManagedSchemaAuthority,
            detail: "managed migration-history authority cannot reserve a drift row".to_owned(),
        }
    })?;
    let rows = sqlx::query(
        "SELECT version, name, up_sha256, down_sha256, schema_sha256
         FROM main.radroots_event_store_schema_migrations
         ORDER BY version
         LIMIT ?",
    )
    .bind(row_limit)
    .fetch_all(&mut *connection)
    .await?;
    rows.into_iter()
        .map(|row| {
            Ok(AppliedMigration {
                version: row.try_get("version")?,
                name: row.try_get("name")?,
                up_sha256: row.try_get("up_sha256")?,
                down_sha256: row.try_get("down_sha256")?,
                schema_sha256: row.try_get("schema_sha256")?,
            })
        })
        .collect()
}

fn validate_history_against_registry(
    history: &[AppliedMigration],
    registry: &[EventStoreMigration],
    supported_current: u32,
) -> Result<u32, RadrootsEventStoreError> {
    if history.is_empty() {
        return Err(RadrootsEventStoreError::MigrationLedgerDrift {
            reason: "ledger exists without migration history".to_owned(),
        });
    }
    let database_version = history
        .iter()
        .map(|row| row.version)
        .max()
        .unwrap_or_default();
    if database_version > i64::from(supported_current) {
        return Err(RadrootsEventStoreError::SchemaTooNew {
            current: supported_current,
            database: database_version,
        });
    }

    let mut expected_version = RADROOTS_EVENT_STORE_SCHEMA_VERSION_MIN;
    for row in history {
        let version = u32::try_from(row.version).map_err(|_| {
            RadrootsEventStoreError::MigrationLedgerDrift {
                reason: format!(
                    "ledger version {} is outside the supported positive range",
                    row.version
                ),
            }
        })?;
        if version != expected_version {
            return Err(RadrootsEventStoreError::MigrationHistoryGap {
                expected: expected_version,
                actual: Some(version),
            });
        }
        let migration = registry
            .iter()
            .find(|migration| migration.version == version)
            .ok_or(RadrootsEventStoreError::UnknownMigration { version })?;
        if row.name != migration.name {
            return Err(RadrootsEventStoreError::MigrationHistoryNameDrift {
                version,
                expected: migration.name,
                actual: row.name.clone(),
            });
        }
        validate_history_checksum(version, "up_sha256", &row.up_sha256, migration.up_sha256)?;
        validate_history_checksum(
            version,
            "down_sha256",
            &row.down_sha256,
            migration.down_sha256,
        )?;
        validate_history_checksum(
            version,
            "schema_sha256",
            &row.schema_sha256,
            migration.schema_sha256,
        )?;
        expected_version += 1;
    }
    Ok(expected_version - 1)
}

fn validate_history_checksum(
    version: u32,
    field: &'static str,
    actual: &str,
    expected: &'static str,
) -> Result<(), RadrootsEventStoreError> {
    if actual != expected {
        return Err(RadrootsEventStoreError::MigrationHistoryChecksumDrift {
            version,
            field,
            expected,
            actual: actual.to_owned(),
        });
    }
    Ok(())
}

async fn validate_schema_fingerprint(
    connection: &mut SqliteConnection,
    registry: &[EventStoreMigration],
    migration: &EventStoreMigration,
) -> Result<(), RadrootsEventStoreError> {
    let catalog = read_catalog(connection).await?;
    validate_ledger_catalog(&catalog)?;
    let actual = catalog_fingerprint(&governed_catalog(&catalog, registry));
    if actual != migration.schema_sha256 {
        return Err(RadrootsEventStoreError::SchemaFingerprintMismatch {
            version: migration.version,
            expected: migration.schema_sha256,
            actual,
        });
    }
    Ok(())
}

#[cfg(test)]
async fn validate_empty_governed_catalog(
    connection: &mut SqliteConnection,
    registry: &[EventStoreMigration],
) -> Result<(), RadrootsEventStoreError> {
    let catalog = read_catalog(connection).await?;
    let actual = catalog_fingerprint(&governed_catalog(&catalog, registry));
    if actual != EMPTY_SCHEMA_SHA256 {
        return Err(RadrootsEventStoreError::SchemaFingerprintMismatch {
            version: 0,
            expected: EMPTY_SCHEMA_SHA256,
            actual,
        });
    }
    Ok(())
}

async fn validate_database_integrity(
    connection: &mut SqliteConnection,
    registry: &[EventStoreMigration],
) -> Result<(), RadrootsEventStoreError> {
    let integrity_rows = sqlx::query("PRAGMA integrity_check")
        .fetch_all(&mut *connection)
        .await?;
    for row in integrity_rows {
        let detail: String = row.try_get(0)?;
        if detail != "ok" {
            return Err(RadrootsEventStoreError::IntegrityCheckFailed { detail });
        }
    }

    let catalog = read_catalog(connection).await?;
    for table in registry
        .iter()
        .flat_map(|migration| migration.fts5_table_names.iter().copied())
        .filter(|table| {
            catalog
                .iter()
                .any(|row| row.object_type == "table" && row.name.eq_ignore_ascii_case(table))
        })
    {
        let statement = format!("INSERT INTO {table}({table}) VALUES('integrity-check')");
        sqlx::query(sqlx::AssertSqlSafe(statement))
            .execute(&mut *connection)
            .await
            .map_err(|source| RadrootsEventStoreError::Fts5IntegrityCheckFailed {
                table,
                source,
            })?;
    }

    let foreign_key_rows = sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(&mut *connection)
        .await?;
    for row in foreign_key_rows {
        let table: String = row.try_get("table")?;
        if is_event_store_owned_table_name(registry, &table) {
            return Err(RadrootsEventStoreError::ForeignKeyViolation {
                table,
                rowid: row.try_get("rowid")?,
                parent: row.try_get("parent")?,
                foreign_key_index: row.try_get("fkid")?,
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod migration_framework {
    use super::*;
    use crate::RadrootsEventStore;
    use crate::migrations::sha256_hex;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::str::FromStr;
    use std::time::Duration;

    #[test]
    fn pending_capacity_hooks_follow_schema_progress() {
        let uninitialized = RadrootsEventStoreSchemaStatus::Uninitialized;
        assert!(!has_pending_source_capacity_hook(
            &uninitialized,
            EVENT_STORE_MIGRATIONS,
        ));
        assert!(!has_pending_source_maintenance_hook(
            &uninitialized,
            EVENT_STORE_MIGRATIONS,
        ));

        let baseline = RadrootsEventStoreSchemaStatus::UnledgeredBaseline;
        assert!(has_pending_source_capacity_hook(
            &baseline,
            EVENT_STORE_MIGRATIONS,
        ));
        assert!(has_pending_source_maintenance_hook(
            &baseline,
            EVENT_STORE_MIGRATIONS,
        ));

        let current = RadrootsEventStoreSchemaStatus::Managed {
            version: RADROOTS_EVENT_STORE_SCHEMA_VERSION_CURRENT,
        };
        assert!(!has_pending_source_capacity_hook(
            &current,
            EVENT_STORE_MIGRATIONS,
        ));
        assert!(!has_pending_source_maintenance_hook(
            &current,
            EVENT_STORE_MIGRATIONS,
        ));
    }

    const SYNTHETIC_V2_UP: &str = "CREATE TABLE radroots_event_store_v2_parent (
  id INTEGER PRIMARY KEY NOT NULL
) STRICT;
CREATE TABLE radroots_event_store_v2_child (
  id INTEGER PRIMARY KEY NOT NULL,
  parent_id INTEGER NOT NULL REFERENCES radroots_event_store_v2_parent(id)
) STRICT;
CREATE INDEX radroots_event_store_v2_child_parent_idx
ON radroots_event_store_v2_child(parent_id);";
    const SYNTHETIC_V2_DOWN: &str = "DROP INDEX radroots_event_store_v2_child_parent_idx;
DROP TABLE radroots_event_store_v2_child;
DROP TABLE radroots_event_store_v2_parent;";
    const SYNTHETIC_V2_OBJECT_NAMES: &[&str] = &[
        "radroots_event_store_v2_child",
        "radroots_event_store_v2_child_parent_idx",
        "radroots_event_store_v2_parent",
    ];
    const SYNTHETIC_V2_TABLE_NAMES: &[&str] = &[
        "radroots_event_store_v2_child",
        "radroots_event_store_v2_parent",
    ];
    const NO_FTS5_TABLES: &[&str] = &[];
    const ZERO_SHA256: &str = "0000000000000000000000000000000000000000000000000000000000000000";

    async fn memory_pool() -> SqlitePool {
        SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::from_str("sqlite::memory:").expect("memory options"),
            )
            .await
            .expect("memory pool")
    }

    fn leaked_sha256(value: &str) -> &'static str {
        Box::leak(sha256_hex(value.as_bytes()).into_boxed_str())
    }

    fn synthetic_v2_descriptor(schema_sha256: &'static str) -> EventStoreMigration {
        EventStoreMigration {
            version: 2,
            name: "synthetic_v2",
            up_sql: SYNTHETIC_V2_UP,
            down_sql: SYNTHETIC_V2_DOWN,
            up_len: SYNTHETIC_V2_UP.len(),
            down_len: SYNTHETIC_V2_DOWN.len(),
            up_sha256: leaked_sha256(SYNTHETIC_V2_UP),
            down_sha256: leaked_sha256(SYNTHETIC_V2_DOWN),
            schema_sha256,
            owned_object_names: SYNTHETIC_V2_OBJECT_NAMES,
            replaced_object_names: &[],
            owned_table_names: SYNTHETIC_V2_TABLE_NAMES,
            fts5_table_names: NO_FTS5_TABLES,
            hook: crate::migrations::EventStoreMigrationHook::None,
            hook_manifest_sha256: None,
            event_contract_registry_version: None,
        }
    }

    async fn synthetic_v2_registry() -> Vec<EventStoreMigration> {
        let provisional = [
            EVENT_STORE_MIGRATIONS[0],
            synthetic_v2_descriptor(ZERO_SHA256),
        ];
        let pool = memory_pool().await;
        sqlx::raw_sql(EVENT_STORE_MIGRATIONS[0].up_sql)
            .execute(&pool)
            .await
            .expect("v1 schema");
        sqlx::raw_sql(SYNTHETIC_V2_UP)
            .execute(&pool)
            .await
            .expect("v2 schema");
        let mut connection = pool.acquire().await.expect("connection");
        let catalog = read_catalog(&mut connection).await.expect("catalog");
        let schema_sha256 = Box::leak(
            catalog_fingerprint(&governed_catalog(&catalog, &provisional)).into_boxed_str(),
        );
        vec![
            EVENT_STORE_MIGRATIONS[0],
            synthetic_v2_descriptor(schema_sha256),
        ]
    }

    async fn install_unledgered_baseline(pool: &SqlitePool) {
        sqlx::raw_sql(EVENT_STORE_MIGRATIONS[0].up_sql)
            .execute(pool)
            .await
            .expect("baseline schema");
    }

    async fn schema_object_sql(pool: &SqlitePool, name: &str) -> String {
        sqlx::query_scalar("SELECT sql FROM main.sqlite_schema WHERE name = ?")
            .bind(name)
            .fetch_one(pool)
            .await
            .expect("schema object SQL")
    }

    async fn insert_test_rebuild_marker(
        connection: &mut SqliteConnection,
        target_generation: &[u8; 32],
        transition_floor_seq: i64,
        prior_last_transition_seq: i64,
    ) -> Result<sqlx::sqlite::SqliteQueryResult, sqlx::Error> {
        sqlx::query(
            "INSERT INTO radroots_event_store_source_rebuild_marker(singleton, barrier_key, target_generation, target_generation_ordinal, reconciliation_version, addressable_feed_version, event_contract_registry_version, hook_id, hook_manifest_sha256, transition_floor_seq, baseline_raw_event_count, baseline_raw_tag_count, baseline_raw_high_water_seq, prior_active_generation, prior_raw_event_count, prior_raw_tag_count, prior_raw_high_water_seq, prior_last_transition_seq) SELECT 1, 1, ?, generation.generation_ordinal + 1, generation.reconciliation_version, generation.addressable_feed_version, generation.event_contract_registry_version, generation.hook_id, generation.hook_manifest_sha256, ?, state.raw_event_count, state.raw_tag_count, state.raw_high_water_seq, state.active_generation, state.raw_event_count, state.raw_tag_count, state.raw_high_water_seq, ? FROM radroots_event_store_source_state AS state JOIN radroots_event_store_source_generation AS generation ON generation.source_generation = state.active_generation WHERE state.singleton = 1",
        )
        .bind(target_generation.as_slice())
        .bind(transition_floor_seq)
        .bind(prior_last_transition_seq)
        .execute(connection)
        .await
    }

    #[tokio::test]
    async fn later_hookless_migrations_do_not_disable_prior_hook_validation() {
        let store = RadrootsEventStore::open_memory().await.expect("v2 store");
        let mut hookless_v3 = EVENT_STORE_MIGRATIONS[0];
        hookless_v3.version = 3;
        hookless_v3.name = "hookless_v3_probe";
        let registry = [
            EVENT_STORE_MIGRATIONS[0],
            EVENT_STORE_MIGRATIONS[1],
            hookless_v3,
        ];

        let mut connection = store.pool().acquire().await.expect("connection");
        validate_applied_migration_hooks(&mut connection, &registry, 3)
            .await
            .expect("v2 hook remains valid under a hookless v3");

        sqlx::query("DROP TRIGGER radroots_event_store_source_state_delete_guard")
            .execute(&mut *connection)
            .await
            .expect("remove guard for corruption fixture");
        sqlx::query("DELETE FROM radroots_event_store_source_state")
            .execute(&mut *connection)
            .await
            .expect("corrupt v2 hook state");

        assert!(matches!(
            validate_applied_migration_hooks(&mut connection, &registry, 3).await,
            Err(RadrootsEventStoreError::MigrationHookStateDrift {
                hook_id: "nip09_reconciliation_v1",
                ..
            })
        ));
    }

    #[tokio::test]
    async fn source_capacity_is_rechecked_for_every_rebuild_bound_migration() {
        let pool = memory_pool().await;
        install_unledgered_baseline(&pool).await;
        sqlx::query(
            "INSERT INTO event_envelopes(event_id, pubkey, created_at, kind, tags_json, content, sig, raw_json, verification_status, contract_status, contract_id, event_class, projection_eligible, inserted_at_ms, updated_at_ms) VALUES (?, ?, 1, 1, '[]', '', ?, '{}', 'verified', 'unsupported', NULL, 'regular', 0, 1, 1)",
        )
        .bind("a".repeat(64))
        .bind("b".repeat(64))
        .bind("c".repeat(128))
        .execute(&pool)
        .await
        .expect("legacy raw event");

        let limits = ReconciliationCapacityLimits {
            raw_events: 0,
            ..ReconciliationCapacityLimits::production()
        };
        let mut transaction = pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .expect("migration transaction");
        let result = migrate_schema_on_connection(
            &mut transaction,
            EVENT_STORE_MIGRATIONS,
            RADROOTS_EVENT_STORE_SCHEMA_VERSION_CURRENT,
            &OsSourceGenerationProvider,
            limits,
        )
        .await;
        let error = finish_schema_transaction(transaction, result)
            .await
            .expect_err("reconciliation capacity excess must fail");
        assert!(
            matches!(
                error,
                RadrootsEventStoreError::SourceCapacityExceeded {
                    resource: crate::RadrootsEventStoreSourceCapacityResourceV1::RawEvents,
                    current: 0,
                    requested: 1,
                    limit: 0,
                }
            ),
            "unexpected reconciliation capacity failure: {error:?}"
        );
        assert_eq!(
            inspect_event_store_schema_status(&pool)
                .await
                .expect("status after rollback"),
            RadrootsEventStoreSchemaStatus::UnledgeredBaseline
        );
        let v2_object_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_schema WHERE name = 'radroots_event_store_source_state'",
        )
        .fetch_one(&pool)
        .await
        .expect("v2 object count");
        assert_eq!(v2_object_count, 0);

        let pool = memory_pool().await;
        migrate_event_store_schema_with_registry(
            &pool,
            &EVENT_STORE_MIGRATIONS[..2],
            RADROOTS_EVENT_STORE_SCHEMA_VERSION_MIN,
            2,
        )
        .await
        .expect("install v2 schema");
        sqlx::query(
            "INSERT INTO event_envelopes(event_id, pubkey, created_at, kind, tags_json, content, sig, raw_json, verification_status, contract_status, contract_id, event_class, projection_eligible, inserted_at_ms, updated_at_ms) VALUES (?, ?, 1, 1, '[]', '', ?, '{}', 'verified', 'unsupported', NULL, 'regular', 0, 1, 1)",
        )
        .bind("d".repeat(64))
        .bind("e".repeat(64))
        .bind("f".repeat(128))
        .execute(&pool)
        .await
        .expect("post-v2 raw event");
        sqlx::query(
            "UPDATE radroots_event_store_source_state SET raw_event_count = 1, raw_high_water_seq = (SELECT MAX(seq) FROM event_envelopes) WHERE singleton = 1",
        )
        .execute(&pool)
        .await
        .expect("advance post-v2 source authority");

        let mut transaction = pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .expect("v3 migration transaction");
        let result = migrate_schema_on_connection(
            &mut transaction,
            EVENT_STORE_MIGRATIONS,
            RADROOTS_EVENT_STORE_SCHEMA_VERSION_CURRENT,
            &OsSourceGenerationProvider,
            limits,
        )
        .await;
        let error = finish_schema_transaction(transaction, result)
            .await
            .expect_err("v3 capacity excess must fail");
        assert!(
            matches!(
                error,
                RadrootsEventStoreError::SourceCapacityExceeded {
                    resource: crate::RadrootsEventStoreSourceCapacityResourceV1::RawEvents,
                    current: 0,
                    requested: 1,
                    limit: 0,
                }
            ),
            "unexpected v3 capacity failure: {error:?}"
        );
        assert_eq!(
            inspect_event_store_schema_status(&pool)
                .await
                .expect("v2 status after rejected v3 migration"),
            RadrootsEventStoreSchemaStatus::Managed { version: 2 }
        );
        let v3_object_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_schema WHERE name = 'radroots_event_store_food_availability_projection'",
        )
        .fetch_one(&pool)
        .await
        .expect("v3 object count");
        assert_eq!(v3_object_count, 0);
        let v3_ledger_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM radroots_event_store_schema_migrations WHERE version = 3",
        )
        .fetch_one(&pool)
        .await
        .expect("v3 ledger count");
        assert_eq!(v3_ledger_count, 0);

        let pool = memory_pool().await;
        migrate_event_store_schema_with_registry(
            &pool,
            &EVENT_STORE_MIGRATIONS[..3],
            RADROOTS_EVENT_STORE_SCHEMA_VERSION_MIN,
            3,
        )
        .await
        .expect("install v3 schema");
        sqlx::query(
            "INSERT INTO event_envelopes(event_id, pubkey, created_at, kind, tags_json, content, sig, raw_json, verification_status, contract_status, contract_id, event_class, projection_eligible, inserted_at_ms, updated_at_ms) VALUES (?, ?, 1, 1, '[]', '', ?, '{}', 'verified', 'unsupported', NULL, 'regular', 0, 1, 1)",
        )
        .bind("1".repeat(64))
        .bind("2".repeat(64))
        .bind("3".repeat(128))
        .execute(&pool)
        .await
        .expect("post-v3 raw event");
        sqlx::query(
            "UPDATE radroots_event_store_source_state SET raw_event_count = 1, raw_high_water_seq = (SELECT MAX(seq) FROM event_envelopes) WHERE singleton = 1",
        )
        .execute(&pool)
        .await
        .expect("advance post-v3 source authority");

        let mut transaction = pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .expect("v4 migration transaction");
        let result = migrate_schema_on_connection(
            &mut transaction,
            EVENT_STORE_MIGRATIONS,
            RADROOTS_EVENT_STORE_SCHEMA_VERSION_CURRENT,
            &OsSourceGenerationProvider,
            limits,
        )
        .await;
        let error = finish_schema_transaction(transaction, result)
            .await
            .expect_err("v4 capacity excess must fail");
        assert!(
            matches!(
                error,
                RadrootsEventStoreError::SourceCapacityExceeded {
                    resource: crate::RadrootsEventStoreSourceCapacityResourceV1::RawEvents,
                    current: 0,
                    requested: 1,
                    limit: 0,
                }
            ),
            "unexpected v4 capacity failure: {error:?}"
        );
        assert_eq!(
            inspect_event_store_schema_status(&pool)
                .await
                .expect("v3 status after rejected v4 migration"),
            RadrootsEventStoreSchemaStatus::Managed { version: 3 }
        );
        let v4_object_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_schema WHERE name = 'radroots_event_store_source_capacity_v1'",
        )
        .fetch_one(&pool)
        .await
        .expect("v4 object count");
        assert_eq!(v4_object_count, 0);
        let v4_ledger_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM radroots_event_store_schema_migrations WHERE version = 4",
        )
        .fetch_one(&pool)
        .await
        .expect("v4 ledger count");
        assert_eq!(v4_ledger_count, 0);
    }

    #[tokio::test]
    async fn v3_to_v4_under_limit_backfills_exact_capacity_and_preserves_source() {
        let pool = memory_pool().await;
        migrate_event_store_schema_with_registry(
            &pool,
            &EVENT_STORE_MIGRATIONS[..3],
            RADROOTS_EVENT_STORE_SCHEMA_VERSION_MIN,
            3,
        )
        .await
        .expect("install v3 schema");
        sqlx::query("CREATE TABLE caller_state(id INTEGER PRIMARY KEY, value TEXT NOT NULL)")
            .execute(&pool)
            .await
            .expect("create unrelated caller table");
        sqlx::query("INSERT INTO caller_state(id, value) VALUES (1, 'preserve')")
            .execute(&pool)
            .await
            .expect("seed unrelated caller row");
        let event_id = "7".repeat(64);
        sqlx::query(
            "INSERT INTO event_envelopes(event_id, pubkey, created_at, kind, tags_json, content, sig, raw_json, verification_status, contract_status, contract_id, event_class, projection_eligible, inserted_at_ms, updated_at_ms) VALUES (?, ?, 1, 1, '[]', 'under-limit', ?, '{}', 'verified', 'unsupported', NULL, 'regular', 0, 1, 1)",
        )
        .bind(event_id.as_str())
        .bind("8".repeat(64))
        .bind("9".repeat(128))
        .execute(&pool)
        .await
        .expect("post-v3 raw event");
        sqlx::query(
            "UPDATE radroots_event_store_source_state SET raw_event_count = 1, raw_high_water_seq = (SELECT MAX(seq) FROM event_envelopes) WHERE singleton = 1",
        )
        .execute(&pool)
        .await
        .expect("advance post-v3 source authority");
        let generation_before: Vec<u8> = sqlx::query_scalar(
            "SELECT active_generation FROM radroots_event_store_source_state WHERE singleton = 1",
        )
        .fetch_one(&pool)
        .await
        .expect("v3 source generation");
        let event_bytes: i64 = sqlx::query_scalar(
            "SELECT length(CAST(event_id AS BLOB)) + length(CAST(pubkey AS BLOB)) + length(CAST(tags_json AS BLOB)) + length(CAST(content AS BLOB)) + length(CAST(sig AS BLOB)) + length(CAST(raw_json AS BLOB)) FROM event_envelopes WHERE event_id = ?",
        )
        .bind(event_id.as_str())
        .fetch_one(&pool)
        .await
        .expect("raw event byte authority");

        migrate_event_store_schema_with_registry(
            &pool,
            EVENT_STORE_MIGRATIONS,
            RADROOTS_EVENT_STORE_SCHEMA_VERSION_MIN,
            RADROOTS_EVENT_STORE_SCHEMA_VERSION_CURRENT,
        )
        .await
        .expect("migrate under-limit v3 source to v4");

        assert_eq!(
            inspect_event_store_schema_status(&pool)
                .await
                .expect("v4 status"),
            RadrootsEventStoreSchemaStatus::Managed { version: 4 }
        );
        let capacity: (Vec<u8>, i64, i64, i64, i64, i64, i64) = sqlx::query_as(
            "SELECT source_generation, raw_event_count, raw_tag_count, raw_event_bytes, raw_tag_bytes, retained_generation_count, retained_generation_limit FROM radroots_event_store_source_capacity_v1 WHERE singleton = 1",
        )
        .fetch_one(&pool)
        .await
        .expect("v4 capacity authority");
        assert_eq!(capacity, (generation_before, 1, 0, event_bytes, 0, 1, 8));
        let preserved: (i64, String) = sqlx::query_as(
            "SELECT (SELECT COUNT(*) FROM event_envelopes WHERE event_id = ?), (SELECT value FROM caller_state WHERE id = 1)",
        )
        .bind(event_id)
        .fetch_one(&pool)
        .await
        .expect("preserved source and caller state");
        assert_eq!(preserved, (1, "preserve".to_owned()));
    }

    #[tokio::test]
    async fn v4_rejects_persisted_legacy_ephemeral_rows_atomically() {
        let pool = memory_pool().await;
        migrate_event_store_schema_with_registry(
            &pool,
            &EVENT_STORE_MIGRATIONS[..3],
            RADROOTS_EVENT_STORE_SCHEMA_VERSION_MIN,
            3,
        )
        .await
        .expect("install v3 schema");
        let event_id = "4".repeat(64);
        sqlx::query(
            "INSERT INTO event_envelopes(event_id, pubkey, created_at, kind, tags_json, content, sig, raw_json, verification_status, contract_status, contract_id, event_class, projection_eligible, inserted_at_ms, updated_at_ms) VALUES (?, ?, 1, 20000, '[]', '', ?, '{}', 'verified', 'unsupported', NULL, 'ephemeral', 0, 1, 1)",
        )
        .bind(event_id.as_str())
        .bind("5".repeat(64))
        .bind("6".repeat(128))
        .execute(&pool)
        .await
        .expect("legacy persisted ephemeral row");
        sqlx::query(
            "UPDATE radroots_event_store_source_state SET raw_event_count = 1, raw_high_water_seq = (SELECT MAX(seq) FROM event_envelopes) WHERE singleton = 1",
        )
        .execute(&pool)
        .await
        .expect("advance legacy source authority");

        let mut transaction = pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .expect("v4 migration transaction");
        let result = migrate_schema_on_connection(
            &mut transaction,
            EVENT_STORE_MIGRATIONS,
            RADROOTS_EVENT_STORE_SCHEMA_VERSION_CURRENT,
            &OsSourceGenerationProvider,
            ReconciliationCapacityLimits::production(),
        )
        .await;
        let error = finish_schema_transaction(transaction, result)
            .await
            .expect_err("persisted ephemeral source must reject v4");
        assert!(matches!(
            error,
            RadrootsEventStoreError::PersistedEphemeralRawEvent {
                ref event_id,
                kind: 20_000,
            } if event_id == &"4".repeat(64)
        ));
        assert_eq!(
            inspect_event_store_schema_status(&pool)
                .await
                .expect("v3 remains valid after rejected v4 migration"),
            RadrootsEventStoreSchemaStatus::Managed { version: 3 }
        );
        let v4_object_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_schema WHERE name = 'radroots_event_store_source_capacity_v1'",
        )
        .fetch_one(&pool)
        .await
        .expect("v4 object count");
        assert_eq!(v4_object_count, 0);
        let v4_ledger_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM radroots_event_store_schema_migrations WHERE version = 4",
        )
        .fetch_one(&pool)
        .await
        .expect("v4 ledger count");
        assert_eq!(v4_ledger_count, 0);
        let raw_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM event_envelopes WHERE event_id = ?")
                .bind(event_id)
                .fetch_one(&pool)
                .await
                .expect("legacy raw row remains after rollback");
        assert_eq!(raw_count, 1);
    }

    #[tokio::test]
    async fn read_only_inspection_classifies_empty_baseline_and_managed_schemas() {
        let pool = memory_pool().await;
        assert_eq!(
            inspect_event_store_schema_status(&pool)
                .await
                .expect("empty status"),
            RadrootsEventStoreSchemaStatus::Uninitialized
        );

        install_unledgered_baseline(&pool).await;
        assert_eq!(
            inspect_event_store_schema_status(&pool)
                .await
                .expect("baseline status"),
            RadrootsEventStoreSchemaStatus::UnledgeredBaseline
        );

        migrate_event_store_schema(&pool)
            .await
            .expect("adopt baseline");
        assert_eq!(
            inspect_event_store_schema_status(&pool)
                .await
                .expect("managed status"),
            RadrootsEventStoreSchemaStatus::Managed {
                version: RADROOTS_EVENT_STORE_SCHEMA_VERSION_CURRENT,
            }
        );
    }

    #[tokio::test]
    async fn direct_schema_inspection_and_migration_reject_governed_temp_objects() {
        for (ddl, expected_name) in [
            (
                "CREATE TEMP TABLE radroots_event_store_schema_migrations (version INTEGER)",
                "radroots_event_store_schema_migrations",
            ),
            (
                "CREATE TEMP TABLE event_envelopes (event_id TEXT)",
                "event_envelopes",
            ),
            (
                "CREATE TEMP TABLE \"EvEnT_EnVeLoPeS\" (event_id TEXT)",
                "EvEnT_EnVeLoPeS",
            ),
            (
                "CREATE TEMP TABLE \"RaDrOoTs_EvEnT_StOrE_CaLlEr\" (value TEXT)",
                "RaDrOoTs_EvEnT_StOrE_CaLlEr",
            ),
            (
                "CREATE TEMP VIEW event_envelope_head AS SELECT 1 AS event_id",
                "event_envelope_head",
            ),
        ] {
            let pool = memory_pool().await;
            let registry = &EVENT_STORE_MIGRATIONS[..1];
            sqlx::query(ddl)
                .execute(&pool)
                .await
                .expect("temporary collision");

            let inspection =
                inspect_event_store_schema_status_with_registry(&pool, registry, 1).await;
            assert!(
                matches!(
                    &inspection,
                    Err(RadrootsEventStoreError::TemporarySchemaCollision {
                        name,
                        ..
                    }) if name == expected_name
                ),
                "unexpected inspection result for `{ddl}`: {inspection:?}"
            );
            let migration = migrate_event_store_schema_with_registry(&pool, registry, 1, 1).await;
            assert!(
                matches!(
                    &migration,
                    Err(RadrootsEventStoreError::TemporarySchemaCollision {
                        name,
                        ..
                    }) if name == expected_name
                ),
                "unexpected migration result for `{ddl}`: {migration:?}"
            );
            let main_object_count: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM main.sqlite_schema WHERE NOT (substr(name, 1, 7) = 'sqlite_' COLLATE NOCASE)",
            )
            .fetch_one(&pool)
            .await
            .expect("main catalog");
            assert_eq!(main_object_count, 0);
        }
    }

    #[tokio::test]
    async fn direct_schema_paths_reject_temp_trigger_targets_and_preserve_unrelated_temp_state() {
        for (trigger_name, target, expected_table_name) in [
            ("caller_probe", "event_envelopes", "event_envelopes"),
            (
                "caller_probe_mixed",
                "\"EVENT_ENVELOPES\"",
                "EVENT_ENVELOPES",
            ),
            ("sqliteX_event_guard", "event_envelopes", "event_envelopes"),
        ] {
            let collision = memory_pool().await;
            install_unledgered_baseline(&collision).await;
            let statement = format!(
                "CREATE TEMP TRIGGER {trigger_name} AFTER INSERT ON main.{target} BEGIN SELECT 1; END"
            );
            sqlx::query(sqlx::AssertSqlSafe(statement))
                .execute(&collision)
                .await
                .expect("temporary trigger");

            assert!(matches!(
                inspect_event_store_schema_status_with_registry(
                    &collision,
                    &EVENT_STORE_MIGRATIONS[..1],
                    1,
                )
                .await,
                Err(RadrootsEventStoreError::TemporarySchemaCollision {
                    name,
                    table_name,
                    ..
                }) if name == trigger_name && table_name == expected_table_name
            ));
            assert!(matches!(
                migrate_event_store_schema_with_registry(
                    &collision,
                    &EVENT_STORE_MIGRATIONS[..1],
                    1,
                    1,
                )
                .await,
                Err(RadrootsEventStoreError::TemporarySchemaCollision {
                    name,
                    table_name,
                    ..
                }) if name == trigger_name && table_name == expected_table_name
            ));
            let ledger_count: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM main.sqlite_schema WHERE name = 'radroots_event_store_schema_migrations'",
            )
            .fetch_one(&collision)
            .await
            .expect("main ledger catalog");
            assert_eq!(ledger_count, 0);
        }

        let allowed = memory_pool().await;
        sqlx::query("CREATE TEMP TABLE caller_cache (value TEXT NOT NULL)")
            .execute(&allowed)
            .await
            .expect("unrelated temporary table");
        sqlx::query("INSERT INTO caller_cache(value) VALUES ('preserved')")
            .execute(&allowed)
            .await
            .expect("unrelated temporary row");
        migrate_event_store_schema_with_registry(&allowed, &EVENT_STORE_MIGRATIONS[..1], 1, 1)
            .await
            .expect("unrelated temporary state is allowed");
        let value: String = sqlx::query_scalar("SELECT value FROM temp.caller_cache")
            .fetch_one(&allowed)
            .await
            .expect("preserved temporary row");
        assert_eq!(value, "preserved");
    }

    #[tokio::test]
    async fn direct_schema_paths_reject_ambient_temp_views_and_triggers() {
        for ddl in [
            "CREATE TEMP VIEW caller_alias AS SELECT 1 AS value",
            "CREATE TEMP TABLE caller_cache (value TEXT);
             CREATE TEMP TRIGGER caller_probe
             AFTER INSERT ON caller_cache
             BEGIN
               SELECT 1;
             END",
        ] {
            let pool = memory_pool().await;
            sqlx::raw_sql(ddl)
                .execute(&pool)
                .await
                .expect("ambient temporary object");

            assert!(matches!(
                migrate_event_store_schema_with_registry(
                    &pool,
                    &EVENT_STORE_MIGRATIONS[..1],
                    1,
                    1,
                )
                .await,
                Err(RadrootsEventStoreError::TemporarySchemaCollision {
                    object_type,
                    ..
                }) if matches!(object_type.as_str(), "trigger" | "view")
            ));
            let main_object_count: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM main.sqlite_schema WHERE NOT (substr(name, 1, 7) = 'sqlite_' COLLATE NOCASE)",
            )
            .fetch_one(&pool)
            .await
            .expect("main catalog");
            assert_eq!(main_object_count, 0);
        }
    }

    #[tokio::test]
    async fn direct_schema_catalog_does_not_treat_sqlite_like_user_names_as_internal() {
        const USER_TRIGGER_NAME: &str = "sqliteX_event_guard";
        const DELTA_UP: &str = "CREATE TABLE event_envelopes (event_id TEXT);
CREATE TRIGGER sqliteX_event_guard
BEFORE INSERT ON event_envelopes
BEGIN
  SELECT 1;
END;";
        const DELTA_DOWN: &str = "DROP TRIGGER sqliteX_event_guard;
DROP TABLE event_envelopes;";
        const DELTA_OBJECT_NAMES: &[&str] = &["event_envelopes"];
        const DELTA_TABLE_NAMES: &[&str] = &["event_envelopes"];

        let managed = memory_pool().await;
        install_unledgered_baseline(&managed).await;
        migrate_event_store_schema_with_registry(&managed, &EVENT_STORE_MIGRATIONS[..1], 1, 1)
            .await
            .expect("managed baseline");
        sqlx::raw_sql(
            "CREATE TRIGGER sqliteX_event_guard
             BEFORE INSERT ON main.event_envelopes
             BEGIN
               SELECT 1;
             END;",
        )
        .execute(&managed)
        .await
        .expect("user trigger with sqlite-like name");

        assert!(matches!(
            inspect_event_store_schema_status_with_registry(
                &managed,
                &EVENT_STORE_MIGRATIONS[..1],
                1,
            )
            .await,
            Err(RadrootsEventStoreError::SchemaFingerprintMismatch { version: 1, .. })
        ));
        let mut managed_connection = managed.acquire().await.expect("managed connection");
        let managed_catalog = read_catalog(&mut managed_connection)
            .await
            .expect("managed catalog");
        assert!(
            managed_catalog
                .iter()
                .any(|row| row.name == USER_TRIGGER_NAME)
        );

        let migration = EventStoreMigration {
            version: 1,
            name: "sqlite_like_delta",
            up_sql: DELTA_UP,
            down_sql: DELTA_DOWN,
            up_len: DELTA_UP.len(),
            down_len: DELTA_DOWN.len(),
            up_sha256: ZERO_SHA256,
            down_sha256: ZERO_SHA256,
            schema_sha256: ZERO_SHA256,
            owned_object_names: DELTA_OBJECT_NAMES,
            replaced_object_names: &[],
            owned_table_names: DELTA_TABLE_NAMES,
            fts5_table_names: NO_FTS5_TABLES,
            hook: EventStoreMigrationHook::None,
            hook_manifest_sha256: None,
            event_contract_registry_version: None,
        };
        let delta = memory_pool().await;
        let mut delta_connection = delta.acquire().await.expect("delta connection");
        let before = read_catalog(&mut delta_connection)
            .await
            .expect("before catalog");
        sqlx::raw_sql(DELTA_UP)
            .execute(&mut *delta_connection)
            .await
            .expect("migration delta");
        let after = read_catalog(&mut delta_connection)
            .await
            .expect("after catalog");
        assert!(
            after.iter().any(|row| row.name == USER_TRIGGER_NAME),
            "sqlite-like user objects must remain visible to catalog delta checks"
        );
        assert!(matches!(
            validate_catalog_delta(&before, &after, &migration, "up"),
            Err(RadrootsEventStoreError::MigrationCatalogDeltaMismatch {
                reason,
                ..
            }) if reason.contains(USER_TRIGGER_NAME)
        ));
    }

    #[tokio::test]
    async fn direct_schema_rollback_rejects_temp_collisions_before_mutation() {
        let pool = memory_pool().await;
        let registry = synthetic_v2_registry().await;
        migrate_event_store_schema_with_registry(&pool, &registry, 1, 2)
            .await
            .expect("managed schema");
        sqlx::query("CREATE TEMP TABLE event_envelope_tags (event_id TEXT)")
            .execute(&pool)
            .await
            .expect("temporary collision");

        assert!(matches!(
            rollback_event_store_schema_with_registry(&pool, &registry, 1, 2, 1).await,
            Err(RadrootsEventStoreError::TemporarySchemaCollision {
                name,
                ..
            }) if name == "event_envelope_tags"
        ));
        sqlx::query("DROP TABLE temp.event_envelope_tags")
            .execute(&pool)
            .await
            .expect("remove collision");
        assert_eq!(
            inspect_event_store_schema_status_with_registry(&pool, &registry, 2)
                .await
                .expect("schema remains current"),
            RadrootsEventStoreSchemaStatus::Managed { version: 2 }
        );
    }

    #[tokio::test]
    async fn baseline_catalog_has_exact_object_count_and_fingerprint() {
        let pool = memory_pool().await;
        install_unledgered_baseline(&pool).await;
        let mut connection = pool.acquire().await.expect("connection");
        let catalog = governed_catalog(
            &read_catalog(&mut connection).await.expect("catalog"),
            EVENT_STORE_MIGRATIONS,
        );

        assert_eq!(catalog.len(), 46);
        assert_eq!(
            catalog_fingerprint(&catalog),
            "5b1f92779640f1a2dbd75e37a96996bda6c8be58883190f69eb3eced22a48f03"
        );
    }

    #[tokio::test]
    async fn nip09_catalog_matches_the_declared_schema_fingerprint() {
        let predecessor_registry = &EVENT_STORE_MIGRATIONS[..2];
        let pool = memory_pool().await;
        sqlx::raw_sql(predecessor_registry[0].up_sql)
            .execute(&pool)
            .await
            .expect("v1 schema");
        sqlx::raw_sql(predecessor_registry[1].up_sql)
            .execute(&pool)
            .await
            .expect("v2 schema");

        let mut connection = pool.acquire().await.expect("connection");
        let catalog = governed_catalog(
            &read_catalog(&mut connection).await.expect("catalog"),
            predecessor_registry,
        );
        let declared_object_count = predecessor_registry
            .iter()
            .map(|migration| migration.owned_object_names.len())
            .sum::<usize>();

        assert_eq!(catalog.len(), declared_object_count);
        assert_eq!(
            catalog_fingerprint(&catalog),
            predecessor_registry[1].schema_sha256
        );
    }

    #[tokio::test]
    async fn current_catalog_matches_the_declared_schema_fingerprint() {
        let pool = memory_pool().await;
        for migration in EVENT_STORE_MIGRATIONS {
            sqlx::raw_sql(migration.up_sql)
                .execute(&pool)
                .await
                .expect("migration schema");
        }

        let mut connection = pool.acquire().await.expect("connection");
        let catalog = governed_catalog(
            &read_catalog(&mut connection).await.expect("catalog"),
            EVENT_STORE_MIGRATIONS,
        );
        let declared_object_count = EVENT_STORE_MIGRATIONS
            .iter()
            .map(|migration| migration.owned_object_names.len())
            .sum::<usize>();

        assert_eq!(catalog.len(), declared_object_count);
        assert_eq!(
            catalog_fingerprint(&catalog),
            EVENT_STORE_MIGRATIONS
                .last()
                .expect("current migration")
                .schema_sha256
        );
    }

    #[tokio::test]
    async fn fresh_memory_and_file_migrations_are_idempotent_and_install_a_strict_ledger() {
        let memory = memory_pool().await;
        let tempdir = tempfile::tempdir().expect("tempdir");
        let file = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(tempdir.path().join("event-store.sqlite"))
                    .create_if_missing(true),
            )
            .await
            .expect("file pool");

        for pool in [&memory, &file] {
            migrate_event_store_schema(pool)
                .await
                .expect("first migration");
            migrate_event_store_schema(pool)
                .await
                .expect("idempotent migration");
            let history_count: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM radroots_event_store_schema_migrations")
                    .fetch_one(pool)
                    .await
                    .expect("history count");
            assert_eq!(
                history_count,
                i64::from(RADROOTS_EVENT_STORE_SCHEMA_VERSION_CURRENT)
            );
            let row = sqlx::query(
                "SELECT wr, strict FROM pragma_table_list WHERE name = 'radroots_event_store_schema_migrations'",
            )
            .fetch_one(pool)
            .await
            .expect("ledger table metadata");
            assert_eq!(row.try_get::<i64, _>("wr").expect("without rowid"), 1);
            assert_eq!(row.try_get::<i64, _>("strict").expect("strict"), 1);
        }
    }

    #[tokio::test]
    async fn unrelated_shared_schema_is_ignored_and_survives_destruction() {
        let pool = memory_pool().await;
        sqlx::query("CREATE TABLE sdk_state (id INTEGER PRIMARY KEY, value TEXT NOT NULL)")
            .execute(&pool)
            .await
            .expect("shared table");
        sqlx::query("INSERT INTO sdk_state(id, value) VALUES (1, 'preserved')")
            .execute(&pool)
            .await
            .expect("shared row");

        migrate_event_store_schema(&pool).await.expect("migration");
        destroy_event_store_schema_for_test(&pool)
            .await
            .expect("destruction");

        let value: String = sqlx::query_scalar("SELECT value FROM sdk_state WHERE id = 1")
            .fetch_one(&pool)
            .await
            .expect("preserved shared row");
        assert_eq!(value, "preserved");
        assert_eq!(
            inspect_event_store_schema_status(&pool)
                .await
                .expect("destroyed status"),
            RadrootsEventStoreSchemaStatus::Uninitialized
        );

        migrate_event_store_schema(&pool).await.expect("recreate");
        assert_eq!(
            inspect_event_store_schema_status(&pool)
                .await
                .expect("recreated status"),
            RadrootsEventStoreSchemaStatus::Managed {
                version: RADROOTS_EVENT_STORE_SCHEMA_VERSION_CURRENT,
            }
        );
    }

    #[tokio::test]
    async fn exact_legacy_adoption_preserves_rows() {
        let pool = memory_pool().await;
        install_unledgered_baseline(&pool).await;
        sqlx::query(
            "INSERT INTO projection_cursor(projection_id, projection_version, last_event_seq, updated_at_ms) VALUES ('legacy', 1, 0, 1)",
        )
        .execute(&pool)
        .await
        .expect("legacy row");

        migrate_event_store_schema(&pool)
            .await
            .expect("legacy adoption");
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM projection_cursor WHERE projection_id = 'legacy'",
        )
        .fetch_one(&pool)
        .await
        .expect("legacy row count");
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn partial_and_attached_schema_objects_fail_closed() {
        let partial = memory_pool().await;
        sqlx::query("CREATE TABLE event_envelopes (seq INTEGER PRIMARY KEY)")
            .execute(&partial)
            .await
            .expect("partial table");
        assert!(matches!(
            migrate_event_store_schema(&partial).await,
            Err(RadrootsEventStoreError::UnmanagedSchema { .. })
        ));

        let extra_index = memory_pool().await;
        install_unledgered_baseline(&extra_index).await;
        sqlx::query("CREATE INDEX unexpected_event_index ON event_envelopes(pubkey)")
            .execute(&extra_index)
            .await
            .expect("attached index");
        assert!(matches!(
            migrate_event_store_schema(&extra_index).await,
            Err(RadrootsEventStoreError::UnmanagedSchema { .. })
        ));

        let extra_trigger = memory_pool().await;
        install_unledgered_baseline(&extra_trigger).await;
        sqlx::query(
            "CREATE TRIGGER unexpected_event_trigger AFTER INSERT ON event_envelopes BEGIN SELECT 1; END",
        )
        .execute(&extra_trigger)
        .await
        .expect("attached trigger");
        assert!(matches!(
            migrate_event_store_schema(&extra_trigger).await,
            Err(RadrootsEventStoreError::UnmanagedSchema { .. })
        ));
    }

    #[tokio::test]
    async fn missing_extra_and_reserved_namespace_objects_fail_closed() {
        let missing = memory_pool().await;
        install_unledgered_baseline(&missing).await;
        sqlx::query("DROP INDEX event_envelope_contract_idx")
            .execute(&missing)
            .await
            .expect("remove governed object");
        assert!(matches!(
            inspect_event_store_schema_status(&missing).await,
            Err(RadrootsEventStoreError::UnmanagedSchema { .. })
        ));

        for name in [
            "radroots_event_store_shared_collision",
            "RADROOTS_EVENT_STORE_SHARED_COLLISION",
        ] {
            let reserved = memory_pool().await;
            let statement = format!("CREATE TABLE {name} (id INTEGER PRIMARY KEY) STRICT");
            sqlx::query(sqlx::AssertSqlSafe(statement))
                .execute(&reserved)
                .await
                .expect("reserved namespace collision");
            assert!(matches!(
                migrate_event_store_schema(&reserved).await,
                Err(RadrootsEventStoreError::UnmanagedSchema { .. })
            ));
        }

        let extra = memory_pool().await;
        install_unledgered_baseline(&extra).await;
        sqlx::query("CREATE INDEX extra_owned_attachment ON listing_projection(seller_pubkey)")
            .execute(&extra)
            .await
            .expect("extra governed attachment");
        assert!(matches!(
            inspect_event_store_schema_status(&extra).await,
            Err(RadrootsEventStoreError::UnmanagedSchema { .. })
        ));
    }

    #[tokio::test]
    async fn fts5_logical_integrity_is_validated() {
        let pool = memory_pool().await;
        migrate_event_store_schema(&pool).await.expect("migration");
        sqlx::query(
            "INSERT INTO listing_search_fts(listing_addr, title, description, product_type, locality, seller_pubkey) VALUES ('listing:1', 'carrots', 'fresh carrots', 'vegetable', 'Victoria', 'seller')",
        )
        .execute(&pool)
        .await
        .expect("FTS row");
        let mut connection = pool.acquire().await.expect("connection");
        validate_database_integrity(&mut connection, EVENT_STORE_MIGRATIONS)
            .await
            .expect("healthy FTS index");
        sqlx::query(
            "UPDATE listing_search_fts_data SET block = X'00' WHERE id = (SELECT MAX(id) FROM listing_search_fts_data)",
        )
            .execute(&mut *connection)
            .await
            .expect("corrupt FTS index");
        let error = validate_database_integrity(&mut connection, EVENT_STORE_MIGRATIONS)
            .await
            .expect_err("corrupt FTS index");
        match error {
            RadrootsEventStoreError::Fts5IntegrityCheckFailed {
                table: "listing_search_fts",
                ..
            } => {}
            RadrootsEventStoreError::IntegrityCheckFailed { detail }
                if detail.to_ascii_lowercase().contains("fts5") => {}
            other => panic!("unexpected FTS integrity error: {other:?}"),
        }
    }

    #[tokio::test]
    async fn counterfeit_and_empty_ledgers_fail_closed() {
        let counterfeit = memory_pool().await;
        install_unledgered_baseline(&counterfeit).await;
        sqlx::query(
            "CREATE TABLE radroots_event_store_schema_migrations (version INTEGER PRIMARY KEY)",
        )
        .execute(&counterfeit)
        .await
        .expect("counterfeit ledger");
        assert!(matches!(
            migrate_event_store_schema(&counterfeit).await,
            Err(RadrootsEventStoreError::MigrationLedgerDrift { .. })
        ));

        let case_variant = memory_pool().await;
        install_unledgered_baseline(&case_variant).await;
        sqlx::query(
            "CREATE TABLE \"RaDrOoTs_EvEnT_StOrE_ScHeMa_MiGrAtIoNs\" (version INTEGER PRIMARY KEY)",
        )
        .execute(&case_variant)
        .await
        .expect("case-variant ledger");
        assert!(matches!(
            inspect_event_store_schema_status(&case_variant).await,
            Err(RadrootsEventStoreError::MigrationLedgerDrift { .. })
        ));
        assert!(matches!(
            migrate_event_store_schema(&case_variant).await,
            Err(RadrootsEventStoreError::MigrationLedgerDrift { .. })
        ));

        let empty = memory_pool().await;
        install_unledgered_baseline(&empty).await;
        sqlx::query(EVENT_STORE_LEDGER_DDL)
            .execute(&empty)
            .await
            .expect("empty ledger");
        assert!(matches!(
            migrate_event_store_schema(&empty).await,
            Err(RadrootsEventStoreError::MigrationLedgerDrift { .. })
        ));

        let attached = memory_pool().await;
        migrate_event_store_schema(&attached)
            .await
            .expect("managed schema");
        sqlx::query(
            "CREATE INDEX unexpected_ledger_index ON radroots_event_store_schema_migrations(name)",
        )
        .execute(&attached)
        .await
        .expect("attached ledger index");
        assert!(matches!(
            inspect_event_store_schema_status(&attached).await,
            Err(RadrootsEventStoreError::MigrationLedgerDrift { .. })
        ));
    }

    #[tokio::test]
    async fn deleted_ledger_rows_and_history_gaps_fail_closed() {
        let deleted = memory_pool().await;
        migrate_event_store_schema(&deleted)
            .await
            .expect("managed schema");
        sqlx::query("DELETE FROM radroots_event_store_schema_migrations WHERE version = 1")
            .execute(&deleted)
            .await
            .expect("delete ledger row");
        assert!(matches!(
            inspect_event_store_schema_status(&deleted).await,
            Err(RadrootsEventStoreError::MigrationHistoryGap {
                expected: 1,
                actual: Some(2)
            })
        ));

        let registry = synthetic_v2_registry().await;
        let gap = memory_pool().await;
        migrate_event_store_schema_with_registry(&gap, &registry, 1, 2)
            .await
            .expect("managed v2 schema");
        sqlx::query("DELETE FROM radroots_event_store_schema_migrations WHERE version = 1")
            .execute(&gap)
            .await
            .expect("delete first ledger row");
        assert!(matches!(
            inspect_event_store_schema_status_with_registry(&gap, &registry, 2).await,
            Err(RadrootsEventStoreError::MigrationHistoryGap {
                expected: 1,
                actual: Some(2)
            })
        ));
    }

    #[tokio::test]
    async fn ledger_name_and_checksum_drift_fail_closed() {
        for (column, expected_field) in [
            ("up_sha256", "up_sha256"),
            ("down_sha256", "down_sha256"),
            ("schema_sha256", "schema_sha256"),
        ] {
            let pool = memory_pool().await;
            migrate_event_store_schema(&pool).await.expect("migration");
            let statement = format!(
                "UPDATE radroots_event_store_schema_migrations SET {column} = '{}'",
                "0".repeat(64)
            );
            sqlx::query(sqlx::AssertSqlSafe(statement))
                .execute(&pool)
                .await
                .expect("checksum drift");
            assert!(matches!(
                inspect_event_store_schema_status(&pool).await,
                Err(RadrootsEventStoreError::MigrationHistoryChecksumDrift {
                    field,
                    ..
                }) if field == expected_field
            ));
        }

        let pool = memory_pool().await;
        migrate_event_store_schema(&pool).await.expect("migration");
        sqlx::query(
            "UPDATE radroots_event_store_schema_migrations SET name = 'counterfeit' WHERE version = 1",
        )
            .execute(&pool)
            .await
            .expect("name drift");
        assert!(matches!(
            inspect_event_store_schema_status(&pool).await,
            Err(RadrootsEventStoreError::MigrationHistoryNameDrift { .. })
        ));
    }

    #[tokio::test]
    async fn managed_schema_drift_fails_closed() {
        let pool = memory_pool().await;
        migrate_event_store_schema(&pool).await.expect("migration");
        sqlx::query("CREATE INDEX unexpected_event_index ON event_envelopes(pubkey)")
            .execute(&pool)
            .await
            .expect("schema drift");

        assert!(matches!(
            inspect_event_store_schema_status(&pool).await,
            Err(RadrootsEventStoreError::SchemaFingerprintMismatch { .. })
        ));
    }

    #[tokio::test]
    async fn too_new_history_precedes_other_history_and_schema_drift() {
        let pool = memory_pool().await;
        migrate_event_store_schema(&pool).await.expect("migration");
        sqlx::query(
            "UPDATE radroots_event_store_schema_migrations SET name = 'counterfeit' WHERE version = 1",
        )
        .execute(&pool)
        .await
        .expect("name drift");
        sqlx::query(
            "INSERT INTO radroots_event_store_schema_migrations(version, name, up_sha256, down_sha256, schema_sha256) VALUES (?, 'future', ?, ?, ?)",
        )
        .bind(i64::MAX)
        .bind("0".repeat(64))
        .bind("1".repeat(64))
        .bind("2".repeat(64))
        .execute(&pool)
        .await
        .expect("future history");
        sqlx::query("CREATE INDEX unexpected_event_index ON event_envelopes(pubkey)")
            .execute(&pool)
            .await
            .expect("schema drift");

        assert!(matches!(
            inspect_event_store_schema_status(&pool).await,
            Err(RadrootsEventStoreError::SchemaTooNew {
                current: RADROOTS_EVENT_STORE_SCHEMA_VERSION_CURRENT,
                database: i64::MAX
            })
        ));
    }

    #[test]
    fn history_validator_rejects_gaps_and_unknown_versions() {
        let row = |migration: &EventStoreMigration| AppliedMigration {
            version: i64::from(migration.version),
            name: migration.name.to_owned(),
            up_sha256: migration.up_sha256.to_owned(),
            down_sha256: migration.down_sha256.to_owned(),
            schema_sha256: migration.schema_sha256.to_owned(),
        };
        assert!(matches!(
            validate_history_against_registry(
                &[AppliedMigration {
                    version: 2,
                    name: "nip09".to_owned(),
                    up_sha256: EVENT_STORE_MIGRATIONS[1].up_sha256.to_owned(),
                    down_sha256: EVENT_STORE_MIGRATIONS[1].down_sha256.to_owned(),
                    schema_sha256: EVENT_STORE_MIGRATIONS[1].schema_sha256.to_owned(),
                }],
                EVENT_STORE_MIGRATIONS,
                2
            ),
            Err(RadrootsEventStoreError::MigrationHistoryGap {
                expected: 1,
                actual: Some(2)
            })
        ));

        let unknown = AppliedMigration {
            version: 5,
            name: "future".to_owned(),
            up_sha256: "0".repeat(64),
            down_sha256: "1".repeat(64),
            schema_sha256: "2".repeat(64),
        };
        assert!(matches!(
            validate_history_against_registry(
                &[
                    row(&EVENT_STORE_MIGRATIONS[0]),
                    row(&EVENT_STORE_MIGRATIONS[1]),
                    row(&EVENT_STORE_MIGRATIONS[2]),
                    row(&EVENT_STORE_MIGRATIONS[3]),
                    unknown
                ],
                EVENT_STORE_MIGRATIONS,
                5
            ),
            Err(RadrootsEventStoreError::UnknownMigration { version: 5 })
        ));
    }

    #[test]
    fn rollback_failure_preserves_the_primary_schema_error() {
        let primary = RadrootsEventStoreError::RollbackUnmanaged;
        let combined = preserve_primary_failure::<()>(primary, Err(sqlx::Error::PoolClosed))
            .expect_err("combined failure");
        assert!(matches!(
            combined,
            RadrootsEventStoreError::MigrationTransactionRollbackFailed {
                primary,
                rollback: sqlx::Error::PoolClosed,
            } if matches!(*primary, RadrootsEventStoreError::RollbackUnmanaged)
        ));

        let primary_only =
            preserve_primary_failure::<()>(RadrootsEventStoreError::RollbackUnmanaged, Ok(()))
                .expect_err("primary failure");
        assert!(matches!(
            primary_only,
            RadrootsEventStoreError::RollbackUnmanaged
        ));
    }

    #[tokio::test]
    async fn rollback_rejects_below_floor_ahead_unmanaged_and_generation_destructive_targets() {
        let unmanaged = memory_pool().await;
        assert!(matches!(
            rollback_event_store_schema_offline(&unmanaged, 1).await,
            Err(RadrootsEventStoreError::RollbackUnmanaged)
        ));

        let managed = memory_pool().await;
        migrate_event_store_schema(&managed)
            .await
            .expect("migration");
        assert!(matches!(
            rollback_event_store_schema_offline(&managed, 0).await,
            Err(RadrootsEventStoreError::RollbackBelowVersionFloor {
                floor: 1,
                target: 0
            })
        ));
        let ahead = RADROOTS_EVENT_STORE_SCHEMA_VERSION_CURRENT + 1;
        assert!(matches!(
            rollback_event_store_schema_offline(&managed, ahead).await,
            Err(RadrootsEventStoreError::RollbackAhead {
                current: RADROOTS_EVENT_STORE_SCHEMA_VERSION_CURRENT,
                target
            }) if target == ahead
        ));
        let history_before: Vec<(Vec<u8>, i64)> = sqlx::query_as(
            "SELECT source_generation, generation_ordinal FROM radroots_event_store_source_generation ORDER BY generation_ordinal",
        )
        .fetch_all(&managed)
        .await
        .expect("source-generation history before rejected rollback");
        assert!(matches!(
            rollback_event_store_schema_offline(&managed, 1).await,
            Err(
                RadrootsEventStoreError::RollbackWouldDiscardSourceGenerationHistory {
                    current: RADROOTS_EVENT_STORE_SCHEMA_VERSION_CURRENT,
                    target: 1,
                    floor: 2,
                }
            )
        ));
        assert_eq!(
            inspect_event_store_schema_status(&managed)
                .await
                .expect("current status after rejected rollback"),
            RadrootsEventStoreSchemaStatus::Managed {
                version: RADROOTS_EVENT_STORE_SCHEMA_VERSION_CURRENT,
            }
        );
        assert_eq!(
            sqlx::query_as::<_, (Vec<u8>, i64)>(
                "SELECT source_generation, generation_ordinal FROM radroots_event_store_source_generation ORDER BY generation_ordinal",
            )
            .fetch_all(&managed)
            .await
            .expect("source-generation history after rejected rollback"),
            history_before
        );

        rollback_event_store_schema_offline_destructive_for_migration_test(&managed, 1)
            .await
            .expect("test-only destructive rollback to v1");
        rollback_event_store_schema_offline(&managed, 1)
            .await
            .expect("v1 to v1 idempotent rollback");
    }

    #[tokio::test]
    async fn rollback_cannot_bypass_generation_history_guard_through_version_three() {
        let managed = memory_pool().await;
        migrate_event_store_schema(&managed)
            .await
            .expect("migration");
        rollback_event_store_schema_offline(&managed, 3)
            .await
            .expect("rollback to history-preserving v3");
        let history_before: Vec<(Vec<u8>, i64)> = sqlx::query_as(
            "SELECT source_generation, generation_ordinal FROM radroots_event_store_source_generation ORDER BY generation_ordinal",
        )
        .fetch_all(&managed)
        .await
        .expect("v3 source-generation history");

        assert!(matches!(
            rollback_event_store_schema_offline(&managed, 1).await,
            Err(
                RadrootsEventStoreError::RollbackWouldDiscardSourceGenerationHistory {
                    current: 3,
                    target: 1,
                    floor: 2,
                }
            )
        ));
        assert_eq!(
            inspect_event_store_schema_status(&managed)
                .await
                .expect("v3 status after rejected bypass"),
            RadrootsEventStoreSchemaStatus::Managed { version: 3 }
        );
        assert_eq!(
            sqlx::query_as::<_, (Vec<u8>, i64)>(
                "SELECT source_generation, generation_ordinal FROM radroots_event_store_source_generation ORDER BY generation_ordinal",
            )
            .fetch_all(&managed)
            .await
            .expect("v3 source-generation history after rejected bypass"),
            history_before
        );
    }

    #[tokio::test]
    async fn synthetic_v2_rolls_back_to_v1() {
        let registry = synthetic_v2_registry().await;
        let pool = memory_pool().await;
        migrate_event_store_schema_with_registry(&pool, &registry, 1, 2)
            .await
            .expect("migrate to v2");

        rollback_event_store_schema_with_registry(&pool, &registry, 1, 2, 1)
            .await
            .expect("rollback to v1");
        assert_eq!(
            inspect_event_store_schema_status_with_registry(&pool, &registry, 2)
                .await
                .expect("v1 status"),
            RadrootsEventStoreSchemaStatus::Managed { version: 1 }
        );
        let v2_objects: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_schema WHERE name LIKE 'radroots_event_store_v2_%'",
        )
        .fetch_one(&pool)
        .await
        .expect("v2 object count");
        assert_eq!(v2_objects, 0);
    }

    #[tokio::test]
    async fn failed_v2_down_sql_rolls_back_atomically() {
        const BAD_DOWN: &str = "DROP TABLE radroots_event_store_missing_v2;";
        let registry = synthetic_v2_registry().await;
        let pool = memory_pool().await;
        migrate_event_store_schema_with_registry(&pool, &registry, 1, 2)
            .await
            .expect("migrate to v2");

        let mut bad_registry = registry.clone();
        bad_registry[1].down_sql = BAD_DOWN;
        bad_registry[1].down_len = BAD_DOWN.len();
        bad_registry[1].down_sha256 = leaked_sha256(BAD_DOWN);
        sqlx::query(
            "UPDATE radroots_event_store_schema_migrations SET down_sha256 = ? WHERE version = 2",
        )
        .bind(bad_registry[1].down_sha256)
        .execute(&pool)
        .await
        .expect("align synthetic ledger");

        assert!(matches!(
            rollback_event_store_schema_with_registry(&pool, &bad_registry, 1, 2, 1).await,
            Err(RadrootsEventStoreError::Sqlx(_))
        ));
        assert_eq!(
            inspect_event_store_schema_status_with_registry(&pool, &bad_registry, 2)
                .await
                .expect("v2 preserved"),
            RadrootsEventStoreSchemaStatus::Managed { version: 2 }
        );
    }

    #[tokio::test]
    async fn wrong_post_down_fingerprint_rolls_back_atomically() {
        let registry = synthetic_v2_registry().await;
        let pool = memory_pool().await;
        migrate_event_store_schema_with_registry(&pool, &registry, 1, 2)
            .await
            .expect("migrate to v2");

        let mut wrong_registry = registry.clone();
        wrong_registry[0].schema_sha256 = ZERO_SHA256;
        sqlx::query(
            "UPDATE radroots_event_store_schema_migrations SET schema_sha256 = ? WHERE version = 1",
        )
        .bind(ZERO_SHA256)
        .execute(&pool)
        .await
        .expect("align synthetic ledger");

        assert!(matches!(
            rollback_event_store_schema_with_registry(&pool, &wrong_registry, 1, 2, 1).await,
            Err(RadrootsEventStoreError::SchemaFingerprintMismatch { version: 1, .. })
        ));
        assert_eq!(
            inspect_event_store_schema_status_with_registry(&pool, &wrong_registry, 2)
                .await
                .expect("v2 preserved"),
            RadrootsEventStoreSchemaStatus::Managed { version: 2 }
        );
    }

    #[tokio::test]
    async fn failing_v2_up_rolls_fresh_install_back_to_uninitialized() {
        const FAILING_UP: &str = "CREATE TABLE radroots_event_store_failing_v2 (
  id INTEGER PRIMARY KEY NOT NULL
) STRICT;
INSERT INTO radroots_event_store_missing_v2(id) VALUES (1);";
        const DOWN: &str = "DROP TABLE radroots_event_store_failing_v2;";
        const OBJECTS: &[&str] = &["radroots_event_store_failing_v2"];
        let v2 = EventStoreMigration {
            version: 2,
            name: "failing_v2",
            up_sql: FAILING_UP,
            down_sql: DOWN,
            up_len: FAILING_UP.len(),
            down_len: DOWN.len(),
            up_sha256: leaked_sha256(FAILING_UP),
            down_sha256: leaked_sha256(DOWN),
            schema_sha256: ZERO_SHA256,
            owned_object_names: OBJECTS,
            replaced_object_names: &[],
            owned_table_names: OBJECTS,
            fts5_table_names: NO_FTS5_TABLES,
            hook: crate::migrations::EventStoreMigrationHook::None,
            hook_manifest_sha256: None,
            event_contract_registry_version: None,
        };
        let registry = [EVENT_STORE_MIGRATIONS[0], v2];
        let pool = memory_pool().await;
        sqlx::query("CREATE TABLE sdk_state (id INTEGER PRIMARY KEY, value TEXT NOT NULL)")
            .execute(&pool)
            .await
            .expect("shared table");

        assert!(matches!(
            migrate_event_store_schema_with_registry(&pool, &registry, 1, 2).await,
            Err(RadrootsEventStoreError::Sqlx(_))
        ));
        assert_eq!(
            inspect_event_store_schema_status(&pool)
                .await
                .expect("rolled-back status"),
            RadrootsEventStoreSchemaStatus::Uninitialized
        );
        let shared_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sdk_state")
            .fetch_one(&pool)
            .await
            .expect("shared table survives");
        assert_eq!(shared_count, 0);
    }

    #[tokio::test]
    async fn failed_adoption_rolls_back_ledger_creation() {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")
            .expect("memory options")
            .foreign_keys(false);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .expect("memory pool");
        install_unledgered_baseline(&pool).await;
        sqlx::query(
            "INSERT INTO event_envelope_tags(event_id, tag_index, tag_name, tag_json, relay_indexed) VALUES ('missing', 0, 'd', '[\"d\"]', 0)",
        )
        .execute(&pool)
        .await
        .expect("orphan row");
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .expect("foreign keys");

        assert!(matches!(
            migrate_event_store_schema(&pool).await,
            Err(RadrootsEventStoreError::RawEventReconciliationMismatch {
                event_id,
                field: "tag_rows",
            }) if event_id == "missing"
        ));
        let ledger_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_schema WHERE name = 'radroots_event_store_schema_migrations'",
        )
        .fetch_one(&pool)
        .await
        .expect("ledger catalog");
        assert_eq!(ledger_count, 0);
    }

    #[tokio::test]
    async fn unrelated_shared_foreign_key_violation_does_not_block_real_migration() {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")
            .expect("memory options")
            .foreign_keys(false);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .expect("memory pool");
        sqlx::raw_sql(
            "CREATE TABLE caller_parent (id INTEGER PRIMARY KEY) STRICT;
CREATE TABLE caller_child (
  id INTEGER PRIMARY KEY,
  parent_id INTEGER NOT NULL REFERENCES caller_parent(id)
) STRICT;
INSERT INTO caller_child(id, parent_id) VALUES (1, 999);",
        )
        .execute(&pool)
        .await
        .expect("unrelated orphan");
        install_unledgered_baseline(&pool).await;
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .expect("foreign keys");

        migrate_event_store_schema(&pool)
            .await
            .expect("unrelated violation is outside event-store ownership");
        let rows = sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&pool)
            .await
            .expect("full foreign-key report");
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].try_get::<String, _>("table").expect("child table"),
            "caller_child"
        );
    }

    #[tokio::test]
    async fn independent_file_pools_serialize_schema_initialization() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let path = tempdir.path().join("concurrent.sqlite");
        let pool = || async {
            SqlitePoolOptions::new()
                .max_connections(1)
                .connect_with(
                    SqliteConnectOptions::new()
                        .filename(&path)
                        .create_if_missing(true)
                        .busy_timeout(std::time::Duration::from_secs(5)),
                )
                .await
                .expect("file pool")
        };
        let (first, second) = tokio::join!(pool(), pool());
        let (first_result, second_result) = tokio::join!(
            migrate_event_store_schema(&first),
            migrate_event_store_schema(&second)
        );
        first_result.expect("first migration");
        second_result.expect("second migration");
        assert_eq!(
            inspect_event_store_schema_status(&first)
                .await
                .expect("managed status"),
            RadrootsEventStoreSchemaStatus::Managed {
                version: RADROOTS_EVENT_STORE_SCHEMA_VERSION_CURRENT,
            }
        );
    }

    #[test]
    fn registry_rejects_unreserved_post_baseline_ownership() {
        const ORDINARY_OBJECTS: &[&str] = &["future_table"];
        let mut v2 = synthetic_v2_descriptor(ZERO_SHA256);
        v2.owned_object_names = ORDINARY_OBJECTS;
        v2.owned_table_names = ORDINARY_OBJECTS;
        let registry = [EVENT_STORE_MIGRATIONS[0], v2];

        assert!(matches!(
            validate_migration_registry(&registry, 1, 2),
            Err(RadrootsEventStoreError::MigrationRegistryDefect { reason })
                if reason.contains("outside the reserved")
        ));
    }

    #[test]
    fn registry_rejects_invalid_predecessor_replacement_declarations() {
        const BASELINE_REPLACEMENT: &[&str] = &["event_envelope_kind_created_idx"];
        const DUPLICATE_REPLACEMENTS: &[&str] = &[
            "radroots_event_store_source_rebuild_marker_insert_guard",
            "radroots_event_store_source_rebuild_marker_insert_guard",
        ];
        const MISSING_REPLACEMENT: &[&str] = &["radroots_event_store_missing_guard"];
        const CURRENT_OWNED_REPLACEMENT: &[&str] =
            &["radroots_event_store_source_capacity_insert_guard"];
        const TABLE_REPLACEMENT: &[&str] = &["radroots_event_store_source_state"];

        let mut baseline = EVENT_STORE_MIGRATIONS[0];
        baseline.replaced_object_names = BASELINE_REPLACEMENT;
        assert!(matches!(
            validate_migration_registry(&[baseline], 1, 1),
            Err(RadrootsEventStoreError::MigrationRegistryDefect { reason })
                if reason.contains("baseline migration cannot replace")
        ));

        let mut hookless = synthetic_v2_descriptor(ZERO_SHA256);
        hookless.replaced_object_names = BASELINE_REPLACEMENT;
        assert!(matches!(
            validate_migration_registry(&[EVENT_STORE_MIGRATIONS[0], hookless], 1, 2),
            Err(RadrootsEventStoreError::MigrationRegistryDefect { reason })
                if reason.contains("without an authenticated successor hook")
        ));

        for (replacements, expected_reason) in [
            (DUPLICATE_REPLACEMENTS, "declared more than once"),
            (MISSING_REPLACEMENT, "exactly one prior migration"),
            (CURRENT_OWNED_REPLACEMENT, "also newly owned"),
            (TABLE_REPLACEMENT, "only non-table schema objects"),
        ] {
            let mut v4 = EVENT_STORE_MIGRATIONS[3];
            v4.replaced_object_names = replacements;
            let registry = [
                EVENT_STORE_MIGRATIONS[0],
                EVENT_STORE_MIGRATIONS[1],
                EVENT_STORE_MIGRATIONS[2],
                v4,
            ];
            assert!(matches!(
                validate_migration_registry(&registry, 1, 4),
                Err(RadrootsEventStoreError::MigrationRegistryDefect { reason })
                    if reason.contains(expected_reason)
            ));
        }
    }

    #[test]
    fn registry_binds_authenticated_hooks_to_one_canonical_migration() {
        let mut duplicate = EVENT_STORE_MIGRATIONS[1];
        duplicate.version = 3;
        duplicate.name = "duplicate_nip09";
        assert!(matches!(
            validate_migration_registry(
                &[EVENT_STORE_MIGRATIONS[0], EVENT_STORE_MIGRATIONS[1], duplicate],
                1,
                3,
            ),
            Err(RadrootsEventStoreError::MigrationRegistryDefect { reason })
                if reason.contains("migration hook `nip09_reconciliation_v1` is declared more than once")
        ));

        let mut misbound = EVENT_STORE_MIGRATIONS[3];
        misbound.version = 2;
        misbound.name = "future_replacement";
        assert!(matches!(
            validate_migration_registry(&[EVENT_STORE_MIGRATIONS[0], misbound], 1, 2),
            Err(RadrootsEventStoreError::MigrationRegistryDefect { reason })
                if reason.contains("bound to canonical migration 4 `source_maintenance`")
        ));
    }

    #[tokio::test]
    async fn migration_catalog_delta_requires_exact_symmetric_replacements() {
        const ADDED_OBJECTS: &[&str] = &["radroots_event_store_replacement_probe"];
        const REPLACED_OBJECTS: &[&str] = &["event_envelope_kind_created_idx"];
        const EXTRA_REPLACEMENTS: &[&str] = &[
            "event_envelope_kind_created_idx",
            "event_envelope_projection_idx",
        ];
        let pool = memory_pool().await;
        install_unledgered_baseline(&pool).await;
        let original_index_sql = schema_object_sql(&pool, REPLACED_OBJECTS[0]).await;
        let mut connection = pool.acquire().await.expect("connection");
        let before = read_catalog(&mut connection).await.expect("before catalog");
        sqlx::raw_sql(
            "DROP INDEX event_envelope_kind_created_idx;
             CREATE INDEX event_envelope_kind_created_idx
             ON event_envelopes(kind, event_id);
             CREATE TABLE radroots_event_store_replacement_probe (
               id INTEGER PRIMARY KEY NOT NULL
             ) STRICT;",
        )
        .execute(&mut *connection)
        .await
        .expect("replacement up delta");
        let changed = read_catalog(&mut connection)
            .await
            .expect("changed catalog");
        let mut migration = synthetic_v2_descriptor(ZERO_SHA256);
        migration.owned_object_names = ADDED_OBJECTS;
        migration.owned_table_names = ADDED_OBJECTS;
        migration.replaced_object_names = REPLACED_OBJECTS;
        validate_catalog_delta(&before, &changed, &migration, "up")
            .expect("exact up replacement delta");

        let mut undeclared = migration;
        undeclared.replaced_object_names = &[];
        assert!(matches!(
            validate_catalog_delta(&before, &changed, &undeclared, "up"),
            Err(RadrootsEventStoreError::MigrationCatalogDeltaMismatch { .. })
        ));
        let mut missing = migration;
        missing.replaced_object_names = EXTRA_REPLACEMENTS;
        assert!(matches!(
            validate_catalog_delta(&before, &changed, &missing, "up"),
            Err(RadrootsEventStoreError::MigrationCatalogDeltaMismatch { .. })
        ));
        let mut add_remove_masquerade = changed.clone();
        add_remove_masquerade.retain(|row| row.name != REPLACED_OBJECTS[0]);
        assert!(matches!(
            validate_catalog_delta(&before, &add_remove_masquerade, &migration, "up"),
            Err(RadrootsEventStoreError::MigrationCatalogDeltaMismatch { .. })
        ));

        sqlx::raw_sql("DROP TABLE radroots_event_store_replacement_probe;")
            .execute(&mut *connection)
            .await
            .expect("remove added object");
        sqlx::raw_sql("DROP INDEX event_envelope_kind_created_idx;")
            .execute(&mut *connection)
            .await
            .expect("remove replacement index");
        sqlx::query(sqlx::AssertSqlSafe(original_index_sql.clone()))
            .execute(&mut *connection)
            .await
            .expect("restore predecessor index");
        let restored = read_catalog(&mut connection)
            .await
            .expect("restored catalog");
        validate_catalog_delta(&changed, &restored, &migration, "down")
            .expect("exact down replacement delta");
        assert_eq!(
            restored
                .iter()
                .find(|row| row.name == REPLACED_OBJECTS[0])
                .and_then(|row| row.sql.as_deref()),
            Some(original_index_sql.as_str())
        );
    }

    #[tokio::test]
    async fn v4_marker_open_allows_repairing_prior_transition_high_water_drift() {
        let pool = memory_pool().await;
        migrate_event_store_schema_with_registry(
            &pool,
            EVENT_STORE_MIGRATIONS,
            RADROOTS_EVENT_STORE_SCHEMA_VERSION_MIN,
            RADROOTS_EVENT_STORE_SCHEMA_VERSION_CURRENT,
        )
        .await
        .expect("install v4 schema");

        let authority_guard = schema_object_sql(
            &pool,
            "radroots_event_store_source_state_authority_update_guard",
        )
        .await;
        sqlx::query("DROP TRIGGER radroots_event_store_source_state_authority_update_guard")
            .execute(&pool)
            .await
            .expect("temporarily remove state authority guard");
        sqlx::query(
            "UPDATE radroots_event_store_source_state SET last_transition_seq = 7 WHERE singleton = 1",
        )
        .execute(&pool)
        .await
        .expect("drift derived transition high-water");
        sqlx::query(sqlx::AssertSqlSafe(authority_guard))
            .execute(&pool)
            .await
            .expect("restore state authority guard");
        let mut connection = pool.acquire().await.expect("fingerprint connection");
        validate_schema_fingerprint(
            &mut connection,
            EVENT_STORE_MIGRATIONS,
            &EVENT_STORE_MIGRATIONS[3],
        )
        .await
        .expect("exact v4 catalog after drift fixture");
        drop(connection);

        let target_generation = [0x91; 32];
        let mut transaction = pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .expect("marker transaction");
        let wrong_prior = insert_test_rebuild_marker(&mut transaction, &target_generation, 0, 6)
            .await
            .expect_err("marker must still bind the exact prior transition high-water");
        assert!(wrong_prior.as_database_error().is_some_and(|error| {
            error
                .message()
                .contains("exact raw and prior source authority")
        }));
        let wrong_floor = insert_test_rebuild_marker(&mut transaction, &target_generation, 1, 7)
            .await
            .expect_err("marker must bind the actual retained transition maximum");
        assert!(wrong_floor.as_database_error().is_some_and(|error| {
            error
                .message()
                .contains("exact raw and prior source authority")
        }));
        let inserted = insert_test_rebuild_marker(&mut transaction, &target_generation, 0, 7)
            .await
            .expect("derived transition drift is repairable under v4");
        assert_eq!(inserted.rows_affected(), 1);
        let marker_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM radroots_event_store_source_rebuild_marker")
                .fetch_one(&mut *transaction)
                .await
                .expect("marker count");
        assert_eq!(marker_count, 1);

        let appended = sqlx::query(
            "INSERT INTO radroots_event_store_source_generation(source_generation, generation_ordinal, reconciliation_version, addressable_feed_version, event_contract_registry_version, hook_id, hook_manifest_sha256, transition_floor_seq, baseline_raw_event_count, baseline_raw_tag_count, baseline_raw_high_water_seq) SELECT target_generation, target_generation_ordinal, reconciliation_version, addressable_feed_version, event_contract_registry_version, hook_id, hook_manifest_sha256, transition_floor_seq, baseline_raw_event_count, baseline_raw_tag_count, baseline_raw_high_water_seq FROM radroots_event_store_source_rebuild_marker WHERE singleton = 1",
        )
        .execute(&mut *transaction)
        .await
        .expect("append repair generation");
        assert_eq!(appended.rows_affected(), 1);
        let rotated = sqlx::query(
            "UPDATE radroots_event_store_source_state SET active_generation = ?, raw_event_count = 0, raw_tag_count = 0, raw_high_water_seq = 0, last_transition_seq = 0 WHERE singleton = 1 AND active_generation = (SELECT prior_active_generation FROM radroots_event_store_source_rebuild_marker WHERE singleton = 1) AND raw_event_count = (SELECT prior_raw_event_count FROM radroots_event_store_source_rebuild_marker WHERE singleton = 1) AND raw_tag_count = (SELECT prior_raw_tag_count FROM radroots_event_store_source_rebuild_marker WHERE singleton = 1) AND raw_high_water_seq = (SELECT prior_raw_high_water_seq FROM radroots_event_store_source_rebuild_marker WHERE singleton = 1) AND last_transition_seq = (SELECT prior_last_transition_seq FROM radroots_event_store_source_rebuild_marker WHERE singleton = 1)",
        )
        .bind(target_generation.as_slice())
        .execute(&mut *transaction)
        .await
        .expect("rotate source state through exact marker CAS");
        assert_eq!(rotated.rows_affected(), 1);
        let repaired: (Vec<u8>, i64, i64) = sqlx::query_as(
            "SELECT state.active_generation, state.last_transition_seq, COALESCE(MAX(transition.transition_seq), 0) FROM radroots_event_store_source_state AS state LEFT JOIN radroots_event_store_addressable_head_transition AS transition ON transition.source_generation = state.active_generation WHERE state.singleton = 1 GROUP BY state.active_generation, state.last_transition_seq",
        )
        .fetch_one(&mut *transaction)
        .await
        .expect("repaired transition authority");
        assert_eq!(repaired.0.as_slice(), target_generation.as_slice());
        assert_eq!(repaired.1, repaired.2);
        assert_eq!(repaired.1, 0);
        transaction
            .rollback()
            .await
            .expect("rollback marker fixture");
    }

    #[tokio::test]
    async fn v3_to_v4_rejects_prior_transition_drift_atomically() {
        let pool = memory_pool().await;
        migrate_event_store_schema_with_registry(
            &pool,
            &EVENT_STORE_MIGRATIONS[..3],
            RADROOTS_EVENT_STORE_SCHEMA_VERSION_MIN,
            3,
        )
        .await
        .expect("install managed v3 schema");
        let healthy_state: (Vec<u8>, i64, i64, i64, i64) = sqlx::query_as(
            "SELECT active_generation, raw_event_count, raw_tag_count, raw_high_water_seq, last_transition_seq FROM radroots_event_store_source_state WHERE singleton = 1",
        )
        .fetch_one(&pool)
        .await
        .expect("healthy v3 source state");
        let authority_guard = schema_object_sql(
            &pool,
            "radroots_event_store_source_state_authority_update_guard",
        )
        .await;
        let mut predecessor_trigger_sql = BTreeMap::new();
        for name in crate::migrations::EVENT_STORE_SOURCE_MAINTENANCE_REPLACED_OBJECT_NAMES {
            predecessor_trigger_sql.insert(*name, schema_object_sql(&pool, name).await);
        }
        let ledger_before: Vec<(i64, String, String, String, String)> = sqlx::query_as(
            "SELECT version, name, up_sha256, down_sha256, schema_sha256 FROM radroots_event_store_schema_migrations ORDER BY version",
        )
        .fetch_all(&pool)
        .await
        .expect("v3 ledger before drift");

        let mut corruption = pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .expect("v3 corruption fixture transaction");
        sqlx::query("DROP TRIGGER radroots_event_store_source_state_authority_update_guard")
            .execute(&mut *corruption)
            .await
            .expect("temporarily remove state authority guard");
        sqlx::query(
            "UPDATE radroots_event_store_source_state SET last_transition_seq = 7 WHERE singleton = 1",
        )
        .execute(&mut *corruption)
        .await
        .expect("drift managed v3 transition high-water");
        sqlx::query(sqlx::AssertSqlSafe(authority_guard.clone()))
            .execute(&mut *corruption)
            .await
            .expect("restore exact v3 state authority guard");
        corruption
            .commit()
            .await
            .expect("commit corrupt managed-v3 fixture");
        let corrupt_state: (Vec<u8>, i64, i64, i64, i64) = sqlx::query_as(
            "SELECT active_generation, raw_event_count, raw_tag_count, raw_high_water_seq, last_transition_seq FROM radroots_event_store_source_state WHERE singleton = 1",
        )
        .fetch_one(&pool)
        .await
        .expect("committed corrupt v3 source state");
        assert_eq!(corrupt_state.4, 7);
        assert_ne!(corrupt_state, healthy_state);

        let error = migrate_event_store_schema_with_registry(
            &pool,
            EVENT_STORE_MIGRATIONS,
            RADROOTS_EVENT_STORE_SCHEMA_VERSION_MIN,
            RADROOTS_EVENT_STORE_SCHEMA_VERSION_CURRENT,
        )
        .await
        .expect_err("v4 upgrade must not repair corrupt managed-v3 hook state");
        assert!(
            matches!(
                error,
                RadrootsEventStoreError::MigrationHookStateDrift {
                    hook_id: "nip09_reconciliation_v1",
                    ..
                }
            ),
            "unexpected managed-v3 drift failure: {error:?}"
        );

        assert_eq!(
            sqlx::query_as::<_, (Vec<u8>, i64, i64, i64, i64)>(
                "SELECT active_generation, raw_event_count, raw_tag_count, raw_high_water_seq, last_transition_seq FROM radroots_event_store_source_state WHERE singleton = 1",
            )
            .fetch_one(&pool)
            .await
            .expect("corrupt state after rejected upgrade"),
            corrupt_state
        );
        let ledger_after: Vec<(i64, String, String, String, String)> = sqlx::query_as(
            "SELECT version, name, up_sha256, down_sha256, schema_sha256 FROM radroots_event_store_schema_migrations ORDER BY version",
        )
        .fetch_all(&pool)
        .await
        .expect("ledger after rejected upgrade");
        assert_eq!(ledger_after, ledger_before);
        assert_eq!(
            ledger_after.iter().map(|row| row.0).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        let v4_objects: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM main.sqlite_schema WHERE name = 'radroots_event_store_source_capacity_v1'",
        )
        .fetch_one(&pool)
        .await
        .expect("v4 object count after failed upgrade");
        assert_eq!(v4_objects, 0);
        let v4_ledger_rows: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM radroots_event_store_schema_migrations WHERE version = 4",
        )
        .fetch_one(&pool)
        .await
        .expect("v4 ledger row count after failed upgrade");
        assert_eq!(v4_ledger_rows, 0);
        for (name, sql) in &predecessor_trigger_sql {
            assert_eq!(schema_object_sql(&pool, name).await, *sql);
        }

        let mut repair = pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .expect("v3 fixture repair transaction");
        sqlx::query("DROP TRIGGER radroots_event_store_source_state_authority_update_guard")
            .execute(&mut *repair)
            .await
            .expect("temporarily remove state authority guard for repair");
        sqlx::query(
            "UPDATE radroots_event_store_source_state SET last_transition_seq = ? WHERE singleton = 1",
        )
        .bind(healthy_state.4)
        .execute(&mut *repair)
        .await
        .expect("repair v3 transition high-water fixture");
        sqlx::query(sqlx::AssertSqlSafe(authority_guard))
            .execute(&mut *repair)
            .await
            .expect("restore exact v3 state authority guard after repair");
        repair.commit().await.expect("commit v3 fixture repair");
        assert_eq!(
            inspect_event_store_schema_status_with_registry(
                &pool,
                EVENT_STORE_MIGRATIONS,
                RADROOTS_EVENT_STORE_SCHEMA_VERSION_CURRENT,
            )
            .await
            .expect("managed v3 status after explicit fixture repair"),
            RadrootsEventStoreSchemaStatus::Managed { version: 3 }
        );
    }

    #[tokio::test]
    async fn v4_food_reset_requires_marker_rotation_and_preserves_target_rows() {
        const PROJECTION_INSERT_GUARD: &str =
            "radroots_event_store_food_availability_projection_insert_guard";
        const IMAGE_INSERT_GUARD: &str =
            "radroots_event_store_food_availability_image_insert_guard";
        const CURSOR_DELETE_GUARD: &str =
            "radroots_event_store_food_availability_cursor_delete_guard";
        let pool = memory_pool().await;
        migrate_event_store_schema_with_registry(
            &pool,
            EVENT_STORE_MIGRATIONS,
            RADROOTS_EVENT_STORE_SCHEMA_VERSION_MIN,
            RADROOTS_EVENT_STORE_SCHEMA_VERSION_CURRENT,
        )
        .await
        .expect("install v4 schema");
        let active_generation: Vec<u8> = sqlx::query_scalar(
            "SELECT active_generation FROM radroots_event_store_source_state WHERE singleton = 1",
        )
        .fetch_one(&pool)
        .await
        .expect("active generation");
        let target_generation = [0x92; 32];
        assert_ne!(active_generation.as_slice(), target_generation.as_slice());

        let mut connection = pool.acquire().await.expect("fixture connection");
        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(&mut *connection)
            .await
            .expect("disable fixture foreign keys");
        let mut guard_definitions = Vec::new();
        for guard in [
            PROJECTION_INSERT_GUARD,
            IMAGE_INSERT_GUARD,
            CURSOR_DELETE_GUARD,
        ] {
            let definition: String =
                sqlx::query_scalar("SELECT sql FROM main.sqlite_schema WHERE name = ?")
                    .bind(guard)
                    .fetch_one(&mut *connection)
                    .await
                    .expect("fixture guard definition");
            let drop_statement = format!("DROP TRIGGER {guard}");
            sqlx::query(sqlx::AssertSqlSafe(drop_statement))
                .execute(&mut *connection)
                .await
                .expect("drop fixture guard");
            guard_definitions.push(definition);
        }
        sqlx::query("DELETE FROM radroots_event_store_food_availability_cursor")
            .execute(&mut *connection)
            .await
            .expect("remove Food cursor fixture");
        let author = "a".repeat(64);
        for (generation, event_id, event_seq, d_tag) in [
            (
                active_generation.as_slice(),
                "b".repeat(64),
                1_i64,
                "historical",
            ),
            (
                target_generation.as_slice(),
                "c".repeat(64),
                2_i64,
                "target",
            ),
        ] {
            sqlx::query(
                "INSERT INTO radroots_event_store_food_availability_projection(source_generation, kind, pubkey, d_tag, event_id, event_seq, created_at, contract_id, content, title, summary, published_at, location, price_amount, price_currency, price_unit, quantity_amount, quantity_unit, status, diagnostic_codes_json, source_transition_seq) VALUES (?, 30402, ?, ?, ?, ?, 10, 'radroots.food.availability.v1', 'fixture', 'Fixture', 'Fixture summary', 10, 'Victoria, BC', '3', 'CAD', 'lb', NULL, NULL, 'active', '[]', 1)",
            )
            .bind(generation)
            .bind(author.as_str())
            .bind(d_tag)
            .bind(event_id)
            .bind(event_seq)
            .execute(&mut *connection)
            .await
            .expect("insert Food projection fixture");
            sqlx::query(
                "INSERT INTO radroots_event_store_food_availability_image(source_generation, pubkey, d_tag, image_index, raw_tag_json, url, width, height, blossom_sha256, qualifies, diagnostic_codes_json) VALUES (?, ?, ?, 0, '[\"image\",\"https://media.example/fixture.webp\"]', NULL, NULL, NULL, NULL, 0, '[]')",
            )
            .bind(generation)
            .bind(author.as_str())
            .bind(d_tag)
            .execute(&mut *connection)
            .await
            .expect("insert Food image fixture");
        }
        for definition in guard_definitions {
            sqlx::query(sqlx::AssertSqlSafe(definition))
                .execute(&mut *connection)
                .await
                .expect("restore fixture guard");
        }
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&mut *connection)
            .await
            .expect("restore fixture foreign keys");
        drop(connection);

        for table in [
            "radroots_event_store_food_availability_image",
            "radroots_event_store_food_availability_projection",
        ] {
            let statement = format!("DELETE FROM {table} WHERE source_generation = ?");
            let error = sqlx::query(sqlx::AssertSqlSafe(statement))
                .bind(active_generation.as_slice())
                .execute(&pool)
                .await
                .expect_err("marker-free Food reset must fail");
            assert!(error.as_database_error().is_some());
        }

        let mut transaction = pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .expect("Food reset transaction");
        insert_test_rebuild_marker(&mut transaction, &target_generation, 0, 0)
            .await
            .expect("open rebuild marker");
        let pre_rotation = sqlx::query(
            "DELETE FROM radroots_event_store_food_availability_image WHERE source_generation = ?",
        )
        .bind(active_generation.as_slice())
        .execute(&mut *transaction)
        .await
        .expect_err("marker alone must not authorize Food reset");
        assert!(pre_rotation.as_database_error().is_some());

        let appended = sqlx::query(
            "INSERT INTO radroots_event_store_source_generation(source_generation, generation_ordinal, reconciliation_version, addressable_feed_version, event_contract_registry_version, hook_id, hook_manifest_sha256, transition_floor_seq, baseline_raw_event_count, baseline_raw_tag_count, baseline_raw_high_water_seq) SELECT target_generation, target_generation_ordinal, reconciliation_version, addressable_feed_version, event_contract_registry_version, hook_id, hook_manifest_sha256, transition_floor_seq, baseline_raw_event_count, baseline_raw_tag_count, baseline_raw_high_water_seq FROM radroots_event_store_source_rebuild_marker WHERE singleton = 1",
        )
        .execute(&mut *transaction)
        .await
        .expect("append target generation");
        assert_eq!(appended.rows_affected(), 1);
        let rotated = sqlx::query(
            "UPDATE radroots_event_store_source_state SET active_generation = ?, raw_event_count = 0, raw_tag_count = 0, raw_high_water_seq = 0, last_transition_seq = 0 WHERE singleton = 1",
        )
        .bind(target_generation.as_slice())
        .execute(&mut *transaction)
        .await
        .expect("rotate source state");
        assert_eq!(rotated.rows_affected(), 1);

        for table in [
            "radroots_event_store_food_availability_image",
            "radroots_event_store_food_availability_projection",
        ] {
            let statement = format!("DELETE FROM {table} WHERE source_generation = ?");
            let deleted = sqlx::query(sqlx::AssertSqlSafe(statement))
                .bind(active_generation.as_slice())
                .execute(&mut *transaction)
                .await
                .expect("post-rotation historical Food reset");
            assert_eq!(deleted.rows_affected(), 1);
        }
        for table in [
            "radroots_event_store_food_availability_image",
            "radroots_event_store_food_availability_projection",
        ] {
            let statement = format!("DELETE FROM {table} WHERE source_generation = ?");
            let error = sqlx::query(sqlx::AssertSqlSafe(statement))
                .bind(target_generation.as_slice())
                .execute(&mut *transaction)
                .await
                .expect_err("active target-generation Food rows must remain guarded");
            assert!(error.as_database_error().is_some());
        }
        let remaining: (i64, i64) = sqlx::query_as(
            "SELECT (SELECT COUNT(*) FROM radroots_event_store_food_availability_projection WHERE source_generation = ?), (SELECT COUNT(*) FROM radroots_event_store_food_availability_image WHERE source_generation = ?)",
        )
        .bind(target_generation.as_slice())
        .bind(target_generation.as_slice())
        .fetch_one(&mut *transaction)
        .await
        .expect("target Food rows");
        assert_eq!(remaining, (1, 1));
        transaction
            .rollback()
            .await
            .expect("rollback Food reset fixture");
    }

    #[tokio::test]
    async fn v4_down_restores_exact_predecessor_trigger_sql_and_fingerprint() {
        const REPLACED: &[&str] = &[
            "radroots_event_store_food_availability_image_delete_guard",
            "radroots_event_store_food_availability_projection_delete_guard",
            "radroots_event_store_source_rebuild_marker_insert_guard",
        ];
        let pool = memory_pool().await;
        migrate_event_store_schema_with_registry(
            &pool,
            &EVENT_STORE_MIGRATIONS[..3],
            RADROOTS_EVENT_STORE_SCHEMA_VERSION_MIN,
            3,
        )
        .await
        .expect("install v3 schema");
        let mut predecessor_sql = BTreeMap::new();
        for name in REPLACED {
            predecessor_sql.insert(*name, schema_object_sql(&pool, name).await);
        }

        migrate_event_store_schema_with_registry(
            &pool,
            EVENT_STORE_MIGRATIONS,
            RADROOTS_EVENT_STORE_SCHEMA_VERSION_MIN,
            RADROOTS_EVENT_STORE_SCHEMA_VERSION_CURRENT,
        )
        .await
        .expect("upgrade to v4");
        for name in REPLACED {
            assert_ne!(schema_object_sql(&pool, name).await, predecessor_sql[*name]);
        }

        rollback_event_store_schema_with_registry(
            &pool,
            EVENT_STORE_MIGRATIONS,
            RADROOTS_EVENT_STORE_SCHEMA_VERSION_MIN,
            RADROOTS_EVENT_STORE_SCHEMA_VERSION_CURRENT,
            3,
        )
        .await
        .expect("rollback v4 to v3");
        for name in REPLACED {
            assert_eq!(schema_object_sql(&pool, name).await, predecessor_sql[*name]);
        }
        assert_eq!(
            inspect_event_store_schema_status_with_registry(
                &pool,
                EVENT_STORE_MIGRATIONS,
                RADROOTS_EVENT_STORE_SCHEMA_VERSION_CURRENT,
            )
            .await
            .expect("restored v3 status"),
            RadrootsEventStoreSchemaStatus::Managed { version: 3 }
        );
    }

    #[tokio::test]
    async fn synthetic_v2_owned_objects_are_fingerprinted_and_foreign_keys_are_checked() {
        let registry = synthetic_v2_registry().await;
        validate_migration_registry(&registry, 1, 2).expect("synthetic registry");

        let fingerprint_pool = memory_pool().await;
        migrate_event_store_schema_with_registry(&fingerprint_pool, &registry, 1, 2)
            .await
            .expect("migrate to v2");
        assert_eq!(
            inspect_event_store_schema_status_with_registry(&fingerprint_pool, &registry, 2)
                .await
                .expect("v2 status"),
            RadrootsEventStoreSchemaStatus::Managed { version: 2 }
        );
        sqlx::query("DROP INDEX radroots_event_store_v2_child_parent_idx")
            .execute(&fingerprint_pool)
            .await
            .expect("remove v2 owned object");
        assert!(matches!(
            inspect_event_store_schema_status_with_registry(&fingerprint_pool, &registry, 2).await,
            Err(RadrootsEventStoreError::SchemaFingerprintMismatch { version: 2, .. })
        ));

        let foreign_key_options = SqliteConnectOptions::from_str("sqlite::memory:")
            .expect("memory options")
            .foreign_keys(false);
        let foreign_key_pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(foreign_key_options)
            .await
            .expect("foreign-key pool");
        migrate_event_store_schema_with_registry(&foreign_key_pool, &registry, 1, 2)
            .await
            .expect("migrate foreign-key pool");
        sqlx::query("INSERT INTO radroots_event_store_v2_child(id, parent_id) VALUES (1, 999)")
            .execute(&foreign_key_pool)
            .await
            .expect("orphan v2 row");
        let mut connection = foreign_key_pool.acquire().await.expect("connection");
        assert!(matches!(
            validate_database_integrity(&mut connection, &registry).await,
            Err(RadrootsEventStoreError::ForeignKeyViolation { table, .. })
                if table == "radroots_event_store_v2_child"
        ));
    }

    #[tokio::test]
    async fn reserved_namespace_rejects_stale_v2_objects_after_ledger_loss() {
        let registry = synthetic_v2_registry().await;
        let pool = memory_pool().await;
        migrate_event_store_schema_with_registry(&pool, &registry, 1, 2)
            .await
            .expect("migrate to v2");
        sqlx::query("DROP TABLE radroots_event_store_schema_migrations")
            .execute(&pool)
            .await
            .expect("remove ledger");

        assert!(matches!(
            inspect_event_store_schema_status(&pool).await,
            Err(RadrootsEventStoreError::UnmanagedSchema { .. })
        ));
    }

    #[tokio::test]
    async fn migration_catalog_delta_rejects_undeclared_objects_atomically() {
        const UP: &str = "CREATE TABLE radroots_event_store_declared_v2 (
  id INTEGER PRIMARY KEY NOT NULL
) STRICT;
CREATE TABLE forgotten_v2_table (
  id INTEGER PRIMARY KEY NOT NULL
) STRICT;";
        const DOWN: &str = "DROP TABLE radroots_event_store_declared_v2;";
        const OBJECTS: &[&str] = &["radroots_event_store_declared_v2"];
        let v2 = EventStoreMigration {
            version: 2,
            name: "undeclared_delta",
            up_sql: UP,
            down_sql: DOWN,
            up_len: UP.len(),
            down_len: DOWN.len(),
            up_sha256: leaked_sha256(UP),
            down_sha256: leaked_sha256(DOWN),
            schema_sha256: ZERO_SHA256,
            owned_object_names: OBJECTS,
            replaced_object_names: &[],
            owned_table_names: OBJECTS,
            fts5_table_names: NO_FTS5_TABLES,
            hook: crate::migrations::EventStoreMigrationHook::None,
            hook_manifest_sha256: None,
            event_contract_registry_version: None,
        };
        let registry = [EVENT_STORE_MIGRATIONS[0], v2];
        let pool = memory_pool().await;

        assert!(matches!(
            migrate_event_store_schema_with_registry(&pool, &registry, 1, 2).await,
            Err(RadrootsEventStoreError::MigrationCatalogDeltaMismatch {
                version: 2,
                direction: "up",
                ..
            })
        ));
        assert_eq!(
            inspect_event_store_schema_status(&pool)
                .await
                .expect("rolled-back status"),
            RadrootsEventStoreSchemaStatus::Uninitialized
        );
        let leaked_objects: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_schema WHERE name IN ('radroots_event_store_declared_v2', 'forgotten_v2_table', 'radroots_event_store_schema_migrations')",
        )
        .fetch_one(&pool)
        .await
        .expect("leaked object count");
        assert_eq!(leaked_objects, 0);
    }

    #[tokio::test]
    async fn current_schema_fast_path_does_not_wait_for_a_writer_lock() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let path = tempdir.path().join("fast-path.sqlite");
        let options = || {
            SqliteConnectOptions::new()
                .filename(&path)
                .create_if_missing(true)
                .busy_timeout(Duration::ZERO)
        };
        let first = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options())
            .await
            .expect("first pool");
        let second = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options())
            .await
            .expect("second pool");
        migrate_event_store_schema(&first)
            .await
            .expect("initial migration");

        let writer = first
            .begin_with("BEGIN IMMEDIATE")
            .await
            .expect("writer transaction");
        migrate_event_store_schema(&second)
            .await
            .expect("read-only fast path");
        writer.rollback().await.expect("release writer");
    }

    #[tokio::test]
    async fn independent_pools_serialize_unledgered_baseline_adoption() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let path = tempdir.path().join("legacy-adoption.sqlite");
        let options = || {
            SqliteConnectOptions::new()
                .filename(&path)
                .create_if_missing(true)
                .busy_timeout(Duration::from_secs(5))
        };
        let first = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options())
            .await
            .expect("first pool");
        install_unledgered_baseline(&first).await;
        let second = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options())
            .await
            .expect("second pool");

        let (first_result, second_result) = tokio::join!(
            migrate_event_store_schema(&first),
            migrate_event_store_schema(&second)
        );
        first_result.expect("first adoption");
        second_result.expect("second adoption");
        let history_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM radroots_event_store_schema_migrations")
                .fetch_one(&first)
                .await
                .expect("history count");
        assert_eq!(
            history_count,
            i64::from(RADROOTS_EVENT_STORE_SCHEMA_VERSION_CURRENT)
        );
    }

    #[tokio::test]
    async fn public_open_file_serializes_concurrent_initialization() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let path = tempdir.path().join("public-open.sqlite");
        let (first, second) = tokio::join!(
            RadrootsEventStore::open_file(&path),
            RadrootsEventStore::open_file(&path)
        );
        let first = first.expect("first public open");
        let second = second.expect("second public open");
        assert_eq!(
            first.schema_status().await.expect("first status"),
            RadrootsEventStoreSchemaStatus::Managed {
                version: RADROOTS_EVENT_STORE_SCHEMA_VERSION_CURRENT,
            }
        );
        assert_eq!(
            second.schema_status().await.expect("second status"),
            RadrootsEventStoreSchemaStatus::Managed {
                version: RADROOTS_EVENT_STORE_SCHEMA_VERSION_CURRENT,
            }
        );
    }

    #[tokio::test]
    async fn migration_and_rollback_preserve_pragma_user_version() {
        let registry = synthetic_v2_registry().await;
        let pool = memory_pool().await;
        sqlx::query("PRAGMA user_version = 73")
            .execute(&pool)
            .await
            .expect("set user version");

        migrate_event_store_schema_with_registry(&pool, &registry, 1, 2)
            .await
            .expect("migrate to v2");
        rollback_event_store_schema_with_registry(&pool, &registry, 1, 2, 1)
            .await
            .expect("rollback to v1");
        let user_version: i64 = sqlx::query_scalar("PRAGMA user_version")
            .fetch_one(&pool)
            .await
            .expect("user version");
        assert_eq!(user_version, 73);
    }

    #[tokio::test]
    async fn representative_pre_freeze_schema_fails_closed() {
        let pool = memory_pool().await;
        install_unledgered_baseline(&pool).await;
        sqlx::raw_sql(
            "DROP INDEX trade_projection_checkpoint_actor_idx;
DROP INDEX trade_projection_checkpoint_agreement_idx;
DROP TABLE trade_projection_checkpoint;
CREATE TABLE trade_projection (
  order_id TEXT NOT NULL,
  root_event_id TEXT NOT NULL,
  projection_version INTEGER NOT NULL,
  PRIMARY KEY(order_id, root_event_id, projection_version)
);",
        )
        .execute(&pool)
        .await
        .expect("representative pre-freeze trade projection");

        assert!(matches!(
            migrate_event_store_schema(&pool).await,
            Err(RadrootsEventStoreError::UnmanagedSchema { .. })
        ));
    }
}
