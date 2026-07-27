#![forbid(unsafe_code)]

use crate::RadrootsOutboxError;
use crate::migrations::{
    OUTBOX_LEDGER_CREATE_DDL, OUTBOX_LEDGER_DDL, OUTBOX_LEDGER_NAME, OUTBOX_MIGRATIONS,
    OUTBOX_RESERVED_PREFIX, OutboxMigration, RADROOTS_OUTBOX_SCHEMA_VERSION_CURRENT,
    RADROOTS_OUTBOX_SCHEMA_VERSION_MIN, is_outbox_governed_schema_name, is_outbox_owned_table_name,
    migration_for_version, validate_embedded_migration_registry, validate_migration_registry,
};
use sha2::{Digest, Sha256};
use sqlx::{Connection, Row, Sqlite, SqliteConnection, SqlitePool, Transaction};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const SQLITE_IDENTIFIER_BYTES_MAX: usize = 255;
const SQLITE_CATALOG_SQL_BYTES_MAX: usize = 65_536;
pub(crate) const SQLITE_LEDGER_NAME_BYTES_MAX: usize = 128;
const SQLITE_LEDGER_DIGEST_BYTES_MAX: usize = 64;
const SQLITE_INTEGRITY_RESULT_ROWS_MAX: i64 = 2;

#[cfg(test)]
const EMPTY_SCHEMA_SHA256: &str =
    "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
/// Authenticated lifecycle state of the governed outbox schema.
pub enum RadrootsOutboxSchemaStatus {
    /// No governed outbox schema objects exist.
    Uninitialized,
    /// The exact frozen baseline exists without its migration ledger.
    UnledgeredBaseline,
    /// The schema and ledger match a supported managed version.
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

/// Inspects and authenticates schema state without adopting or migrating it.
pub async fn inspect_outbox_schema_status(
    pool: &SqlitePool,
) -> Result<RadrootsOutboxSchemaStatus, RadrootsOutboxError> {
    validate_embedded_migration_registry()?;
    let mut transaction = pool.begin().await?;
    let result = inspect_schema_on_connection(
        &mut transaction,
        OUTBOX_MIGRATIONS,
        RADROOTS_OUTBOX_SCHEMA_VERSION_CURRENT,
    )
    .await;
    finish_schema_transaction(transaction, result).await
}

pub(crate) async fn inspect_outbox_schema_on_connection(
    connection: &mut SqliteConnection,
) -> Result<RadrootsOutboxSchemaStatus, RadrootsOutboxError> {
    validate_embedded_migration_registry()?;
    let mut transaction = connection.begin().await?;
    let result = inspect_schema_on_connection(
        &mut transaction,
        OUTBOX_MIGRATIONS,
        RADROOTS_OUTBOX_SCHEMA_VERSION_CURRENT,
    )
    .await;
    finish_schema_transaction(transaction, result).await
}

#[cfg(test)]
pub(crate) async fn migrate_outbox_schema(pool: &SqlitePool) -> Result<(), RadrootsOutboxError> {
    validate_embedded_migration_registry()?;
    migrate_outbox_schema_with_registry(
        pool,
        OUTBOX_MIGRATIONS,
        RADROOTS_OUTBOX_SCHEMA_VERSION_MIN,
        RADROOTS_OUTBOX_SCHEMA_VERSION_CURRENT,
    )
    .await
}

pub(crate) async fn migrate_outbox_schema_on_connection(
    connection: &mut SqliteConnection,
) -> Result<(), RadrootsOutboxError> {
    validate_embedded_migration_registry()?;
    validate_migration_registry(
        OUTBOX_MIGRATIONS,
        RADROOTS_OUTBOX_SCHEMA_VERSION_MIN,
        RADROOTS_OUTBOX_SCHEMA_VERSION_CURRENT,
    )?;
    let mut transaction = connection.begin_with("BEGIN IMMEDIATE").await?;
    let result = migrate_schema_on_connection(
        &mut transaction,
        OUTBOX_MIGRATIONS,
        RADROOTS_OUTBOX_SCHEMA_VERSION_CURRENT,
    )
    .await;
    finish_schema_transaction(transaction, result).await
}

#[cfg(test)]
async fn migrate_outbox_schema_with_registry(
    pool: &SqlitePool,
    registry: &[OutboxMigration],
    minimum: u32,
    supported_current: u32,
) -> Result<(), RadrootsOutboxError> {
    validate_migration_registry(registry, minimum, supported_current)?;
    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
    let result = migrate_schema_on_connection(&mut transaction, registry, supported_current).await;
    finish_schema_transaction(transaction, result).await
}

pub(crate) async fn rollback_outbox_schema_on_connection(
    connection: &mut SqliteConnection,
    target: u32,
) -> Result<(), RadrootsOutboxError> {
    if target < RADROOTS_OUTBOX_SCHEMA_VERSION_MIN {
        return Err(RadrootsOutboxError::RollbackBelowVersionFloor {
            floor: RADROOTS_OUTBOX_SCHEMA_VERSION_MIN,
            target,
        });
    }
    validate_embedded_migration_registry()?;
    let mut transaction = connection.begin_with("BEGIN EXCLUSIVE").await?;
    let result = rollback_schema_on_connection(
        &mut transaction,
        OUTBOX_MIGRATIONS,
        RADROOTS_OUTBOX_SCHEMA_VERSION_CURRENT,
        target,
    )
    .await;
    finish_schema_transaction(transaction, result).await
}

#[cfg(test)]
pub(crate) async fn destroy_outbox_schema_for_migration_test(
    pool: &SqlitePool,
) -> Result<(), RadrootsOutboxError> {
    validate_embedded_migration_registry()?;
    let mut transaction = pool.begin_with("BEGIN EXCLUSIVE").await?;
    let result = destroy_schema_on_connection(&mut transaction).await;
    finish_schema_transaction(transaction, result).await
}

async fn finish_schema_transaction<T>(
    transaction: Transaction<'_, Sqlite>,
    result: Result<T, RadrootsOutboxError>,
) -> Result<T, RadrootsOutboxError> {
    match result {
        Ok(value) => {
            transaction.commit().await?;
            Ok(value)
        }
        Err(primary) => match transaction.rollback().await {
            Ok(()) => Err(primary),
            Err(rollback) => Err(RadrootsOutboxError::MigrationTransactionRollbackFailed {
                primary: Box::new(primary),
                rollback,
            }),
        },
    }
}

async fn migrate_schema_on_connection(
    connection: &mut SqliteConnection,
    registry: &[OutboxMigration],
    supported_current: u32,
) -> Result<(), RadrootsOutboxError> {
    let status = inspect_schema_on_connection(connection, registry, supported_current).await?;
    let current_version = match status {
        RadrootsOutboxSchemaStatus::Uninitialized => {
            apply_migration_up(connection, registry, &registry[0]).await?;
            create_ledger(connection, registry).await?;
            insert_ledger_row(connection, &registry[0]).await?;
            registry[0].version
        }
        RadrootsOutboxSchemaStatus::UnledgeredBaseline => {
            create_ledger(connection, registry).await?;
            insert_ledger_row(connection, &registry[0]).await?;
            registry[0].version
        }
        RadrootsOutboxSchemaStatus::Managed { version } if version == supported_current => version,
        RadrootsOutboxSchemaStatus::Managed { version } => version,
    };

    for migration in registry
        .iter()
        .filter(|migration| migration.version > current_version)
    {
        apply_migration_up(connection, registry, migration).await?;
        insert_ledger_row(connection, migration).await?;
    }

    match inspect_schema_on_connection(connection, registry, supported_current).await? {
        RadrootsOutboxSchemaStatus::Managed { version } if version == supported_current => Ok(()),
        status => Err(RadrootsOutboxError::MigrationRegistryDefect {
            reason: format!("migration completed in unexpected state {status:?}"),
        }),
    }
}

async fn rollback_schema_on_connection(
    connection: &mut SqliteConnection,
    registry: &[OutboxMigration],
    supported_current: u32,
    target: u32,
) -> Result<(), RadrootsOutboxError> {
    let RadrootsOutboxSchemaStatus::Managed {
        version: current_version,
    } = inspect_schema_on_connection(connection, registry, supported_current).await?
    else {
        return Err(RadrootsOutboxError::RollbackUnmanaged);
    };
    if target > current_version {
        return Err(RadrootsOutboxError::RollbackAhead {
            current: current_version,
            target,
        });
    }

    for version in ((target + 1)..=current_version).rev() {
        let migration = migration_for_version(registry, version)
            .ok_or(RadrootsOutboxError::UnknownMigration { version })?;
        apply_migration_down(connection, registry, migration).await?;
        let prior = migration_for_version(registry, version - 1).ok_or(
            RadrootsOutboxError::MigrationHistoryGap {
                expected: version - 1,
                actual: None,
            },
        )?;
        validate_schema_fingerprint(connection, registry, prior).await?;
        let deleted =
            sqlx::query("DELETE FROM main.radroots_outbox_schema_migrations WHERE version = ?")
                .bind(i64::from(version))
                .execute(&mut *connection)
                .await?;
        if deleted.rows_affected() != 1 {
            return Err(RadrootsOutboxError::MigrationLedgerDrift {
                reason: format!(
                    "rollback expected one ledger row for version {version}, deleted {}",
                    deleted.rows_affected()
                ),
            });
        }
    }

    match inspect_schema_on_connection(connection, registry, supported_current).await? {
        RadrootsOutboxSchemaStatus::Managed { version } if version == target => Ok(()),
        status => Err(RadrootsOutboxError::MigrationLedgerDrift {
            reason: format!("rollback completed in unexpected state {status:?}"),
        }),
    }
}

#[cfg(test)]
async fn destroy_schema_on_connection(
    connection: &mut SqliteConnection,
) -> Result<(), RadrootsOutboxError> {
    match inspect_schema_on_connection(
        connection,
        OUTBOX_MIGRATIONS,
        RADROOTS_OUTBOX_SCHEMA_VERSION_CURRENT,
    )
    .await?
    {
        RadrootsOutboxSchemaStatus::Managed { version } => {
            for version in (RADROOTS_OUTBOX_SCHEMA_VERSION_MIN..=version).rev() {
                let migration = migration_for_version(OUTBOX_MIGRATIONS, version)
                    .ok_or(RadrootsOutboxError::UnknownMigration { version })?;
                apply_migration_down(connection, OUTBOX_MIGRATIONS, migration).await?;
                let deleted = sqlx::query(
                    "DELETE FROM main.radroots_outbox_schema_migrations WHERE version = ?",
                )
                .bind(i64::from(version))
                .execute(&mut *connection)
                .await?;
                if deleted.rows_affected() != 1 {
                    return Err(RadrootsOutboxError::MigrationLedgerDrift {
                        reason: format!(
                            "test destruction expected one ledger row for version {version}, deleted {}",
                            deleted.rows_affected()
                        ),
                    });
                }
            }
            validate_empty_governed_catalog(connection, OUTBOX_MIGRATIONS).await?;
            sqlx::query("DROP TABLE main.radroots_outbox_schema_migrations")
                .execute(&mut *connection)
                .await?;
        }
        RadrootsOutboxSchemaStatus::UnledgeredBaseline => {
            apply_migration_down(connection, OUTBOX_MIGRATIONS, &OUTBOX_MIGRATIONS[0]).await?;
            validate_empty_governed_catalog(connection, OUTBOX_MIGRATIONS).await?;
        }
        RadrootsOutboxSchemaStatus::Uninitialized => {}
    }
    Ok(())
}

async fn apply_migration_up(
    connection: &mut SqliteConnection,
    registry: &[OutboxMigration],
    migration: &OutboxMigration,
) -> Result<(), RadrootsOutboxError> {
    let before = read_catalog_bounded(connection, registry).await?;
    sqlx::raw_sql(migration.up_sql)
        .execute(&mut *connection)
        .await?;
    let after = read_catalog_bounded(connection, registry).await?;
    validate_catalog_delta(&before, &after, migration, "up")?;
    validate_schema_fingerprint(connection, registry, migration).await
}

async fn apply_migration_down(
    connection: &mut SqliteConnection,
    registry: &[OutboxMigration],
    migration: &OutboxMigration,
) -> Result<(), RadrootsOutboxError> {
    let before = read_catalog_bounded(connection, registry).await?;
    sqlx::raw_sql(migration.down_sql)
        .execute(&mut *connection)
        .await?;
    let after = read_catalog_bounded(connection, registry).await?;
    validate_catalog_delta(&before, &after, migration, "down")
}

fn validate_catalog_delta(
    before: &[CatalogRow],
    after: &[CatalogRow],
    migration: &OutboxMigration,
    direction: &'static str,
) -> Result<(), RadrootsOutboxError> {
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
    let valid = match direction {
        "up" => added == expected && removed.is_empty() && changed.is_empty(),
        "down" => removed == expected && added.is_empty() && changed.is_empty(),
        _ => false,
    };
    if !valid {
        return Err(RadrootsOutboxError::MigrationCatalogDeltaMismatch {
            version: migration.version,
            direction,
            reason: format!(
                "expected {expected:?}; added {added:?}, removed {removed:?}, changed {changed:?}"
            ),
        });
    }
    Ok(())
}

async fn create_ledger(
    connection: &mut SqliteConnection,
    registry: &[OutboxMigration],
) -> Result<(), RadrootsOutboxError> {
    sqlx::query(OUTBOX_LEDGER_CREATE_DDL)
        .execute(&mut *connection)
        .await?;
    validate_ledger_catalog(&read_catalog_bounded(connection, registry).await?)?;
    Ok(())
}

async fn insert_ledger_row(
    connection: &mut SqliteConnection,
    migration: &OutboxMigration,
) -> Result<(), RadrootsOutboxError> {
    sqlx::query(
        "INSERT INTO main.radroots_outbox_schema_migrations(version, name, up_sha256, down_sha256, schema_sha256) VALUES (?, ?, ?, ?, ?)",
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
    registry: &[OutboxMigration],
    supported_current: u32,
) -> Result<RadrootsOutboxSchemaStatus, RadrootsOutboxError> {
    validate_outbox_temp_schema_with_registry(connection, registry).await?;
    let catalog = read_catalog_bounded(connection, registry).await?;
    let has_ledger = validate_ledger_catalog(&catalog)?;
    let governed = governed_catalog(&catalog, registry);
    let actual_schema_sha256 = catalog_fingerprint(&governed);

    if !has_ledger {
        if governed.is_empty() {
            return Ok(RadrootsOutboxSchemaStatus::Uninitialized);
        }
        let baseline = &registry[0];
        if governed.len() == baseline.owned_object_names.len()
            && actual_schema_sha256 == baseline.schema_sha256
        {
            return Ok(RadrootsOutboxSchemaStatus::UnledgeredBaseline);
        }
        return Err(RadrootsOutboxError::UnmanagedSchema {
            actual_schema_sha256,
        });
    }

    let history = read_history_bounded(connection, supported_current).await?;
    let current = validate_history_against_registry(&history, registry, supported_current)?;
    let expected = migration_for_version(registry, current)
        .ok_or(RadrootsOutboxError::UnknownMigration { version: current })?;
    if actual_schema_sha256 != expected.schema_sha256 {
        return Err(RadrootsOutboxError::SchemaFingerprintMismatch {
            version: current,
            expected: expected.schema_sha256,
            actual: actual_schema_sha256,
        });
    }
    Ok(RadrootsOutboxSchemaStatus::Managed { version: current })
}

async fn validate_outbox_temp_schema_with_registry(
    connection: &mut SqliteConnection,
    registry: &[OutboxMigration],
) -> Result<(), RadrootsOutboxError> {
    let collision = sqlx::query(
        "SELECT
            length(CAST(type AS BLOB)) AS type_bytes,
            CASE WHEN length(CAST(type AS BLOB)) <= ? THEN type END AS bounded_type,
            length(CAST(name AS BLOB)) AS name_bytes,
            CASE WHEN length(CAST(name AS BLOB)) <= ? THEN name END AS bounded_name,
            length(CAST(tbl_name AS BLOB)) AS table_name_bytes,
            CASE WHEN length(CAST(tbl_name AS BLOB)) <= ? THEN tbl_name END AS bounded_table_name
         FROM temp.sqlite_schema
         WHERE type IN ('trigger', 'view')
            OR lower(substr(name, 1, length(?))) = lower(?)
            OR lower(substr(tbl_name, 1, length(?))) = lower(?)
            OR name = ? COLLATE NOCASE
            OR tbl_name = ? COLLATE NOCASE
         ORDER BY type, name, tbl_name LIMIT 1",
    )
    .bind(i64::try_from(SQLITE_IDENTIFIER_BYTES_MAX).unwrap_or(i64::MAX))
    .bind(i64::try_from(SQLITE_IDENTIFIER_BYTES_MAX).unwrap_or(i64::MAX))
    .bind(i64::try_from(SQLITE_IDENTIFIER_BYTES_MAX).unwrap_or(i64::MAX))
    .bind(OUTBOX_RESERVED_PREFIX)
    .bind(OUTBOX_RESERVED_PREFIX)
    .bind(OUTBOX_RESERVED_PREFIX)
    .bind(OUTBOX_RESERVED_PREFIX)
    .bind(OUTBOX_LEDGER_NAME)
    .bind(OUTBOX_LEDGER_NAME)
    .fetch_optional(&mut *connection)
    .await?;
    if let Some(row) = collision {
        let object_type = bounded_required_text(
            &row,
            "bounded_type",
            "type_bytes",
            "temporary catalog object type",
            SQLITE_IDENTIFIER_BYTES_MAX,
        )?;
        let name = bounded_required_text(
            &row,
            "bounded_name",
            "name_bytes",
            "temporary catalog object name",
            SQLITE_IDENTIFIER_BYTES_MAX,
        )?;
        let table_name = bounded_required_text(
            &row,
            "bounded_table_name",
            "table_name_bytes",
            "temporary catalog table name",
            SQLITE_IDENTIFIER_BYTES_MAX,
        )?;
        if matches!(object_type.as_str(), "trigger" | "view")
            || is_outbox_governed_schema_name(registry, &name)
            || is_outbox_governed_schema_name(registry, &table_name)
        {
            return Err(RadrootsOutboxError::TemporarySchemaCollision {
                object_type,
                name,
                table_name,
            });
        }
    }
    Ok(())
}

fn catalog_row_limit(registry: &[OutboxMigration]) -> Result<i64, RadrootsOutboxError> {
    let max = registry
        .iter()
        .flat_map(|migration| migration.owned_object_names.iter().copied())
        .collect::<BTreeSet<_>>()
        .len()
        .checked_add(1)
        .ok_or_else(|| RadrootsOutboxError::MigrationRegistryDefect {
            reason: "governed catalog object limit overflow".to_owned(),
        })?;
    i64::try_from(max.checked_add(1).ok_or_else(|| {
        RadrootsOutboxError::MigrationRegistryDefect {
            reason: "governed catalog collision limit overflow".to_owned(),
        }
    })?)
    .map_err(|_| RadrootsOutboxError::MigrationRegistryDefect {
        reason: "governed catalog object limit is outside SQLite range".to_owned(),
    })
}

async fn read_catalog_bounded(
    connection: &mut SqliteConnection,
    registry: &[OutboxMigration],
) -> Result<Vec<CatalogRow>, RadrootsOutboxError> {
    let row_limit = catalog_row_limit(registry)?;
    let rows = sqlx::query(
        "SELECT
            length(CAST(type AS BLOB)) AS type_bytes,
            CASE WHEN length(CAST(type AS BLOB)) <= ? THEN type END AS bounded_type,
            length(CAST(name AS BLOB)) AS name_bytes,
            CASE WHEN length(CAST(name AS BLOB)) <= ? THEN name END AS bounded_name,
            length(CAST(tbl_name AS BLOB)) AS table_name_bytes,
            CASE WHEN length(CAST(tbl_name AS BLOB)) <= ? THEN tbl_name END AS bounded_table_name,
            length(CAST(sql AS BLOB)) AS sql_bytes,
            CASE WHEN sql IS NULL OR length(CAST(sql AS BLOB)) <= ? THEN sql END AS bounded_sql
         FROM main.sqlite_schema
         WHERE lower(substr(name, 1, 7)) != 'sqlite_'
           AND (lower(substr(name, 1, length(?))) = lower(?)
             OR lower(substr(tbl_name, 1, length(?))) = lower(?)
             OR name = ? COLLATE NOCASE
             OR tbl_name = ? COLLATE NOCASE)
         ORDER BY type, name, tbl_name LIMIT ?",
    )
    .bind(i64::try_from(SQLITE_IDENTIFIER_BYTES_MAX).unwrap_or(i64::MAX))
    .bind(i64::try_from(SQLITE_IDENTIFIER_BYTES_MAX).unwrap_or(i64::MAX))
    .bind(i64::try_from(SQLITE_IDENTIFIER_BYTES_MAX).unwrap_or(i64::MAX))
    .bind(i64::try_from(SQLITE_CATALOG_SQL_BYTES_MAX).unwrap_or(i64::MAX))
    .bind(OUTBOX_RESERVED_PREFIX)
    .bind(OUTBOX_RESERVED_PREFIX)
    .bind(OUTBOX_RESERVED_PREFIX)
    .bind(OUTBOX_RESERVED_PREFIX)
    .bind(OUTBOX_LEDGER_NAME)
    .bind(OUTBOX_LEDGER_NAME)
    .bind(row_limit)
    .fetch_all(&mut *connection)
    .await?;
    if i64::try_from(rows.len()).ok() == Some(row_limit) {
        return Err(RadrootsOutboxError::GovernedCatalogCapacityExceeded {
            max: usize::try_from(row_limit - 1).unwrap_or(usize::MAX),
        });
    }
    rows.into_iter()
        .map(|row| {
            Ok(CatalogRow {
                object_type: bounded_required_text(
                    &row,
                    "bounded_type",
                    "type_bytes",
                    "catalog object type",
                    SQLITE_IDENTIFIER_BYTES_MAX,
                )?,
                name: bounded_required_text(
                    &row,
                    "bounded_name",
                    "name_bytes",
                    "catalog object name",
                    SQLITE_IDENTIFIER_BYTES_MAX,
                )?,
                table_name: bounded_required_text(
                    &row,
                    "bounded_table_name",
                    "table_name_bytes",
                    "catalog table name",
                    SQLITE_IDENTIFIER_BYTES_MAX,
                )?,
                sql: bounded_optional_text(
                    &row,
                    "bounded_sql",
                    "sql_bytes",
                    "catalog SQL",
                    SQLITE_CATALOG_SQL_BYTES_MAX,
                )?,
            })
        })
        .collect()
}

fn bounded_required_text(
    row: &sqlx::sqlite::SqliteRow,
    value_column: &'static str,
    length_column: &'static str,
    field: &'static str,
    max: usize,
) -> Result<String, RadrootsOutboxError> {
    let actual = bounded_text_length(row.try_get(length_column)?, field, max)?;
    if actual > max {
        return Err(RadrootsOutboxError::SqliteTextLimitExceeded { field, max, actual });
    }
    row.try_get::<Option<String>, _>(value_column)?.ok_or(
        RadrootsOutboxError::SqliteLifecycleFailure {
            stage: "bounded SQLite text decode",
        },
    )
}

fn bounded_optional_text(
    row: &sqlx::sqlite::SqliteRow,
    value_column: &'static str,
    length_column: &'static str,
    field: &'static str,
    max: usize,
) -> Result<Option<String>, RadrootsOutboxError> {
    let Some(length) = row.try_get::<Option<i64>, _>(length_column)? else {
        return Ok(None);
    };
    let actual = bounded_text_length(length, field, max)?;
    if actual > max {
        return Err(RadrootsOutboxError::SqliteTextLimitExceeded { field, max, actual });
    }
    row.try_get(value_column).map_err(Into::into)
}

fn bounded_text_length(
    length: i64,
    field: &'static str,
    max: usize,
) -> Result<usize, RadrootsOutboxError> {
    usize::try_from(length).map_err(|_| RadrootsOutboxError::SqliteTextLimitExceeded {
        field,
        max,
        actual: usize::MAX,
    })
}

fn validate_ledger_catalog(catalog: &[CatalogRow]) -> Result<bool, RadrootsOutboxError> {
    let rows = catalog
        .iter()
        .filter(|row| {
            row.name.eq_ignore_ascii_case(OUTBOX_LEDGER_NAME)
                || row.table_name.eq_ignore_ascii_case(OUTBOX_LEDGER_NAME)
        })
        .collect::<Vec<_>>();
    if rows.is_empty() {
        return Ok(false);
    }
    if rows.len() != 1 {
        return Err(RadrootsOutboxError::MigrationLedgerDrift {
            reason: format!(
                "expected exactly one non-internal ledger catalog object, found {}",
                rows.len()
            ),
        });
    }
    let row = rows[0];
    if row.object_type != "table"
        || row.name != OUTBOX_LEDGER_NAME
        || row.table_name != OUTBOX_LEDGER_NAME
        || row.sql.as_deref() != Some(OUTBOX_LEDGER_DDL)
    {
        return Err(RadrootsOutboxError::MigrationLedgerDrift {
            reason: "ledger table definition does not match canonical catalog SQL".to_owned(),
        });
    }
    Ok(true)
}

fn governed_catalog(catalog: &[CatalogRow], registry: &[OutboxMigration]) -> Vec<CatalogRow> {
    catalog
        .iter()
        .filter(|row| !row.name.eq_ignore_ascii_case(OUTBOX_LEDGER_NAME))
        .filter(|row| {
            is_outbox_governed_schema_name(registry, &row.name)
                || is_outbox_governed_schema_name(registry, &row.table_name)
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

async fn read_history_bounded(
    connection: &mut SqliteConnection,
    supported_current: u32,
) -> Result<Vec<AppliedMigration>, RadrootsOutboxError> {
    let row_limit = i64::from(supported_current).checked_add(1).ok_or_else(|| {
        RadrootsOutboxError::MigrationRegistryDefect {
            reason: "migration history row limit overflow".to_owned(),
        }
    })?;
    let rows = sqlx::query(
        "SELECT version,
            length(CAST(name AS BLOB)) AS name_bytes,
            CASE WHEN length(CAST(name AS BLOB)) <= ? THEN name END AS bounded_name,
            length(CAST(up_sha256 AS BLOB)) AS up_sha256_bytes,
            CASE WHEN length(CAST(up_sha256 AS BLOB)) <= ? THEN up_sha256 END AS bounded_up_sha256,
            length(CAST(down_sha256 AS BLOB)) AS down_sha256_bytes,
            CASE WHEN length(CAST(down_sha256 AS BLOB)) <= ? THEN down_sha256 END AS bounded_down_sha256,
            length(CAST(schema_sha256 AS BLOB)) AS schema_sha256_bytes,
            CASE WHEN length(CAST(schema_sha256 AS BLOB)) <= ? THEN schema_sha256 END AS bounded_schema_sha256
         FROM main.radroots_outbox_schema_migrations
         ORDER BY version LIMIT ?",
    )
    .bind(i64::try_from(SQLITE_LEDGER_NAME_BYTES_MAX).unwrap_or(i64::MAX))
    .bind(i64::try_from(SQLITE_LEDGER_DIGEST_BYTES_MAX).unwrap_or(i64::MAX))
    .bind(i64::try_from(SQLITE_LEDGER_DIGEST_BYTES_MAX).unwrap_or(i64::MAX))
    .bind(i64::try_from(SQLITE_LEDGER_DIGEST_BYTES_MAX).unwrap_or(i64::MAX))
    .bind(row_limit)
    .fetch_all(&mut *connection)
    .await?;
    rows.into_iter()
        .map(|row| {
            Ok(AppliedMigration {
                version: row.try_get("version")?,
                name: bounded_required_text(
                    &row,
                    "bounded_name",
                    "name_bytes",
                    "migration ledger name",
                    SQLITE_LEDGER_NAME_BYTES_MAX,
                )?,
                up_sha256: bounded_required_text(
                    &row,
                    "bounded_up_sha256",
                    "up_sha256_bytes",
                    "migration ledger up checksum",
                    SQLITE_LEDGER_DIGEST_BYTES_MAX,
                )?,
                down_sha256: bounded_required_text(
                    &row,
                    "bounded_down_sha256",
                    "down_sha256_bytes",
                    "migration ledger down checksum",
                    SQLITE_LEDGER_DIGEST_BYTES_MAX,
                )?,
                schema_sha256: bounded_required_text(
                    &row,
                    "bounded_schema_sha256",
                    "schema_sha256_bytes",
                    "migration ledger schema checksum",
                    SQLITE_LEDGER_DIGEST_BYTES_MAX,
                )?,
            })
        })
        .collect()
}

fn validate_history_against_registry(
    history: &[AppliedMigration],
    registry: &[OutboxMigration],
    supported_current: u32,
) -> Result<u32, RadrootsOutboxError> {
    if history.is_empty() {
        return Err(RadrootsOutboxError::MigrationLedgerDrift {
            reason: "ledger exists without migration history".to_owned(),
        });
    }
    let database_version = history
        .iter()
        .map(|row| row.version)
        .max()
        .unwrap_or_default();
    if database_version > i64::from(supported_current) {
        return Err(RadrootsOutboxError::SchemaTooNew {
            current: supported_current,
            database: database_version,
        });
    }
    let mut expected_version = registry[0].version;
    for row in history {
        let version =
            u32::try_from(row.version).map_err(|_| RadrootsOutboxError::MigrationLedgerDrift {
                reason: format!(
                    "ledger version {} is outside the positive range",
                    row.version
                ),
            })?;
        if version != expected_version {
            return Err(RadrootsOutboxError::MigrationHistoryGap {
                expected: expected_version,
                actual: Some(version),
            });
        }
        let migration = migration_for_version(registry, version)
            .ok_or(RadrootsOutboxError::UnknownMigration { version })?;
        if row.name != migration.name {
            return Err(RadrootsOutboxError::MigrationHistoryNameDrift {
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
        expected_version = expected_version.checked_add(1).ok_or_else(|| {
            RadrootsOutboxError::MigrationLedgerDrift {
                reason: "migration history version overflow".to_owned(),
            }
        })?;
    }
    Ok(expected_version - 1)
}

fn validate_history_checksum(
    version: u32,
    field: &'static str,
    actual: &str,
    expected: &'static str,
) -> Result<(), RadrootsOutboxError> {
    if actual != expected {
        return Err(RadrootsOutboxError::MigrationHistoryChecksumDrift {
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
    registry: &[OutboxMigration],
    migration: &OutboxMigration,
) -> Result<(), RadrootsOutboxError> {
    let catalog = read_catalog_bounded(connection, registry).await?;
    let actual = catalog_fingerprint(&governed_catalog(&catalog, registry));
    if actual != migration.schema_sha256 {
        return Err(RadrootsOutboxError::SchemaFingerprintMismatch {
            version: migration.version,
            expected: migration.schema_sha256,
            actual,
        });
    }
    Ok(())
}

pub(crate) async fn validate_outbox_owned_integrity(
    connection: &mut SqliteConnection,
) -> Result<(), RadrootsOutboxError> {
    validate_embedded_migration_registry()?;
    let tables = OUTBOX_MIGRATIONS
        .iter()
        .flat_map(|migration| migration.owned_table_names.iter().copied())
        .collect::<BTreeSet<_>>();
    for table in tables {
        validate_owned_table_foreign_keys(connection, table).await?;
        validate_integrity_results(connection, Some(table)).await?;
    }
    Ok(())
}

pub(crate) async fn validate_full_database_integrity(
    connection: &mut SqliteConnection,
) -> Result<(), RadrootsOutboxError> {
    validate_integrity_results(connection, None).await
}

async fn validate_owned_table_foreign_keys(
    connection: &mut SqliteConnection,
    table: &'static str,
) -> Result<(), RadrootsOutboxError> {
    if !is_outbox_owned_table_name(OUTBOX_MIGRATIONS, table) {
        return Err(RadrootsOutboxError::MigrationRegistryDefect {
            reason: "scoped integrity table is outside outbox authority".to_owned(),
        });
    }
    let row = sqlx::query(
        "SELECT
            length(CAST(\"table\" AS BLOB)) AS table_bytes,
            CASE WHEN length(CAST(\"table\" AS BLOB)) <= ? THEN \"table\" END AS bounded_table,
            rowid,
            length(CAST(parent AS BLOB)) AS parent_bytes,
            CASE WHEN length(CAST(parent AS BLOB)) <= ? THEN parent END AS bounded_parent,
            fkid
         FROM pragma_foreign_key_check(?) LIMIT 1",
    )
    .bind(i64::try_from(SQLITE_IDENTIFIER_BYTES_MAX).unwrap_or(i64::MAX))
    .bind(i64::try_from(SQLITE_IDENTIFIER_BYTES_MAX).unwrap_or(i64::MAX))
    .bind(table)
    .fetch_optional(&mut *connection)
    .await?;
    if let Some(row) = row {
        return Err(RadrootsOutboxError::ForeignKeyViolation {
            table: bounded_required_text(
                &row,
                "bounded_table",
                "table_bytes",
                "foreign-key table name",
                SQLITE_IDENTIFIER_BYTES_MAX,
            )?,
            rowid: row.try_get("rowid")?,
            parent: bounded_required_text(
                &row,
                "bounded_parent",
                "parent_bytes",
                "foreign-key parent name",
                SQLITE_IDENTIFIER_BYTES_MAX,
            )?,
            foreign_key_id: row.try_get("fkid")?,
        });
    }
    Ok(())
}

async fn validate_integrity_results(
    connection: &mut SqliteConnection,
    table: Option<&'static str>,
) -> Result<(), RadrootsOutboxError> {
    let max = i64::try_from(crate::RADROOTS_OUTBOX_DIAGNOSTIC_BYTES_MAX).unwrap_or(i64::MAX);
    let rows = if let Some(table) = table {
        sqlx::query(
            "SELECT
                length(CAST(integrity_check AS BLOB)) AS detail_bytes,
                CASE WHEN length(CAST(integrity_check AS BLOB)) <= ? THEN integrity_check END AS bounded_detail
             FROM pragma_integrity_check(?) LIMIT ?",
        )
        .bind(max)
        .bind(table)
        .bind(SQLITE_INTEGRITY_RESULT_ROWS_MAX)
        .fetch_all(&mut *connection)
        .await?
    } else {
        sqlx::query(
            "SELECT
                length(CAST(integrity_check AS BLOB)) AS detail_bytes,
                CASE WHEN length(CAST(integrity_check AS BLOB)) <= ? THEN integrity_check END AS bounded_detail
             FROM pragma_integrity_check LIMIT ?",
        )
        .bind(max)
        .bind(SQLITE_INTEGRITY_RESULT_ROWS_MAX)
        .fetch_all(&mut *connection)
        .await?
    };
    if rows.len() == 1 {
        let detail = bounded_required_text(
            &rows[0],
            "bounded_detail",
            "detail_bytes",
            "integrity diagnostic",
            crate::RADROOTS_OUTBOX_DIAGNOSTIC_BYTES_MAX,
        )?;
        if detail == "ok" {
            return Ok(());
        }
        return Err(RadrootsOutboxError::IntegrityCheckFailed { detail });
    }
    Err(RadrootsOutboxError::IntegrityCheckFailed {
        detail: "integrity check returned an invalid result cardinality".to_owned(),
    })
}

#[cfg(test)]
async fn validate_empty_governed_catalog(
    connection: &mut SqliteConnection,
    registry: &[OutboxMigration],
) -> Result<(), RadrootsOutboxError> {
    let catalog = read_catalog_bounded(connection, registry).await?;
    let actual = catalog_fingerprint(&governed_catalog(&catalog, registry));
    if actual != EMPTY_SCHEMA_SHA256 {
        return Err(RadrootsOutboxError::SchemaFingerprintMismatch {
            version: 0,
            expected: EMPTY_SCHEMA_SHA256,
            actual,
        });
    }
    Ok(())
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::str::FromStr;

    async fn memory_pool() -> SqlitePool {
        SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(SqliteConnectOptions::from_str("sqlite::memory:").expect("options"))
            .await
            .expect("pool")
    }

    #[tokio::test]
    async fn fresh_migration_reaches_the_authenticated_schema_fingerprint() {
        let pool = memory_pool().await;
        migrate_outbox_schema(&pool).await.expect("fresh migration");
        assert_eq!(
            inspect_outbox_schema_status(&pool)
                .await
                .expect("managed schema"),
            RadrootsOutboxSchemaStatus::Managed { version: 1 }
        );
    }

    async fn apply_unledgered_baseline(pool: &SqlitePool) {
        sqlx::raw_sql(OUTBOX_MIGRATIONS[0].up_sql)
            .execute(pool)
            .await
            .expect("unledgered baseline");
    }

    async fn ledger_count(pool: &SqlitePool) -> i64 {
        sqlx::query_scalar(
            "SELECT COUNT(*) FROM main.sqlite_schema WHERE type = 'table' AND name = ?",
        )
        .bind(OUTBOX_LEDGER_NAME)
        .fetch_one(pool)
        .await
        .expect("ledger count")
    }

    async fn synthetic_v2_registry(pool: &SqlitePool) -> [OutboxMigration; 2] {
        const UP_SQL: &str = "CREATE TABLE outbox_future (value TEXT NOT NULL) STRICT;\n";
        const DOWN_SQL: &str = "DROP TABLE outbox_future;\n";

        migrate_outbox_schema(pool).await.expect("version 1");
        sqlx::raw_sql(UP_SQL)
            .execute(pool)
            .await
            .expect("synthetic version 2 schema");
        let catalog = sqlx::query(
            "SELECT type, name, tbl_name, sql FROM main.sqlite_schema
             WHERE lower(substr(name, 1, 7)) != 'sqlite_'
               AND (lower(substr(name, 1, length(?))) = lower(?)
                 OR lower(substr(tbl_name, 1, length(?))) = lower(?))",
        )
        .bind(OUTBOX_RESERVED_PREFIX)
        .bind(OUTBOX_RESERVED_PREFIX)
        .bind(OUTBOX_RESERVED_PREFIX)
        .bind(OUTBOX_RESERVED_PREFIX)
        .fetch_all(pool)
        .await
        .expect("synthetic version 2 catalog")
        .into_iter()
        .map(|row| CatalogRow {
            object_type: row.try_get("type").expect("catalog type"),
            name: row.try_get("name").expect("catalog name"),
            table_name: row.try_get("tbl_name").expect("catalog table name"),
            sql: row.try_get("sql").expect("catalog SQL"),
        })
        .collect::<Vec<_>>();
        let schema_sha256 = Box::leak(catalog_fingerprint(&catalog).into_boxed_str());
        sqlx::raw_sql(DOWN_SQL)
            .execute(pool)
            .await
            .expect("remove synthetic version 2 schema");

        let up_sha256 =
            Box::leak(crate::migrations::sha256_hex(UP_SQL.as_bytes()).into_boxed_str());
        let down_sha256 =
            Box::leak(crate::migrations::sha256_hex(DOWN_SQL.as_bytes()).into_boxed_str());
        [
            OUTBOX_MIGRATIONS[0],
            OutboxMigration {
                version: 2,
                name: "future",
                up_sql: UP_SQL,
                down_sql: DOWN_SQL,
                up_len: UP_SQL.len(),
                down_len: DOWN_SQL.len(),
                up_sha256,
                down_sha256,
                schema_sha256,
                owned_object_names: &["outbox_future"],
                owned_table_names: &["outbox_future"],
            },
        ]
    }

    async fn inspect_with_registry(
        pool: &SqlitePool,
        registry: &[OutboxMigration],
        supported_current: u32,
    ) -> Result<RadrootsOutboxSchemaStatus, RadrootsOutboxError> {
        let mut transaction = pool.begin().await?;
        let result =
            inspect_schema_on_connection(&mut transaction, registry, supported_current).await;
        finish_schema_transaction(transaction, result).await
    }

    #[tokio::test]
    async fn additive_future_migration_advances_and_rolls_back_to_an_explicit_target() {
        let pool = memory_pool().await;
        sqlx::raw_sql(
            "CREATE TABLE caller_state (value TEXT NOT NULL);
             INSERT INTO caller_state(value) VALUES ('preserved');",
        )
        .execute(&pool)
        .await
        .expect("caller state");
        let registry = synthetic_v2_registry(&pool).await;

        migrate_outbox_schema_with_registry(&pool, &registry, 1, 2)
            .await
            .expect("advance to version 2");
        assert_eq!(
            inspect_with_registry(&pool, &registry, 2)
                .await
                .expect("managed version 2"),
            RadrootsOutboxSchemaStatus::Managed { version: 2 }
        );
        let future_objects: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM main.sqlite_schema WHERE name = 'outbox_future'",
        )
        .fetch_one(&pool)
        .await
        .expect("future object count");
        assert_eq!(future_objects, 1);

        let mut transaction = pool
            .begin_with("BEGIN EXCLUSIVE")
            .await
            .expect("rollback transaction");
        let result = rollback_schema_on_connection(&mut transaction, &registry, 2, 1).await;
        finish_schema_transaction(transaction, result)
            .await
            .expect("rollback to version 1");
        assert_eq!(
            inspect_with_registry(&pool, &registry, 2)
                .await
                .expect("managed version 1"),
            RadrootsOutboxSchemaStatus::Managed { version: 1 }
        );
        let future_objects: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM main.sqlite_schema WHERE name = 'outbox_future'",
        )
        .fetch_one(&pool)
        .await
        .expect("rolled-back object count");
        assert_eq!(future_objects, 0);
        let caller_value: String = sqlx::query_scalar("SELECT value FROM caller_state")
            .fetch_one(&pool)
            .await
            .expect("preserved caller state");
        assert_eq!(caller_value, "preserved");

        let mut transaction = pool
            .begin_with("BEGIN EXCLUSIVE")
            .await
            .expect("ahead transaction");
        let result = rollback_schema_on_connection(&mut transaction, &registry, 2, 2).await;
        assert!(matches!(
            result,
            Err(RadrootsOutboxError::RollbackAhead { .. })
        ));
        transaction
            .rollback()
            .await
            .expect("rollback ahead fixture");

        let unmanaged = memory_pool().await;
        let mut transaction = unmanaged
            .begin_with("BEGIN EXCLUSIVE")
            .await
            .expect("unmanaged transaction");
        let result = rollback_schema_on_connection(&mut transaction, &registry, 2, 1).await;
        assert!(matches!(
            result,
            Err(RadrootsOutboxError::RollbackUnmanaged)
        ));
        transaction
            .rollback()
            .await
            .expect("rollback unmanaged fixture");
    }

    #[tokio::test]
    async fn migration_post_up_authority_failure_rolls_back_catalog_ledger_and_caller_state() {
        let pool = memory_pool().await;
        sqlx::raw_sql(
            "CREATE TABLE caller_state (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             INSERT INTO caller_state(key, value) VALUES ('victoria', 'preserved');",
        )
        .execute(&pool)
        .await
        .expect("caller state");
        let mut registry = synthetic_v2_registry(&pool).await;

        let mut connection = pool.acquire().await.expect("catalog connection");
        let before_catalog = read_catalog_bounded(&mut connection, &registry)
            .await
            .expect("before catalog");
        let before_fingerprint = catalog_fingerprint(&governed_catalog(&before_catalog, &registry));
        let before_history = read_history_bounded(&mut connection, 2)
            .await
            .expect("before history");
        drop(connection);

        registry[1].owned_object_names = &["outbox_expected"];
        registry[1].owned_table_names = &["outbox_expected"];
        let error = migrate_outbox_schema_with_registry(&pool, &registry, 1, 2)
            .await
            .expect_err("post-UP catalog delta must fail");
        assert!(matches!(
            error,
            RadrootsOutboxError::MigrationCatalogDeltaMismatch {
                version: 2,
                direction: "up",
                ..
            }
        ));

        let mut connection = pool.acquire().await.expect("verification connection");
        let after_catalog = read_catalog_bounded(&mut connection, &registry)
            .await
            .expect("after catalog");
        let after_fingerprint = catalog_fingerprint(&governed_catalog(&after_catalog, &registry));
        let after_history = read_history_bounded(&mut connection, 2)
            .await
            .expect("after history");
        drop(connection);
        assert_eq!(after_fingerprint, before_fingerprint);
        assert_eq!(after_history, before_history);
        assert_eq!(
            inspect_outbox_schema_status(&pool)
                .await
                .expect("restored managed schema"),
            RadrootsOutboxSchemaStatus::Managed { version: 1 }
        );
        let future_objects: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM main.sqlite_schema WHERE name = 'outbox_future'",
        )
        .fetch_one(&pool)
        .await
        .expect("rolled-back future object count");
        assert_eq!(future_objects, 0);
        let caller_value: String =
            sqlx::query_scalar("SELECT value FROM caller_state WHERE key = 'victoria'")
                .fetch_one(&pool)
                .await
                .expect("preserved caller row");
        assert_eq!(caller_value, "preserved");
    }

    #[tokio::test]
    async fn migration_exact_unledgered_baseline_is_adopted_without_replaying_or_losing_caller_state()
     {
        let pool = memory_pool().await;
        sqlx::query("CREATE TABLE caller_state (key TEXT PRIMARY KEY, value TEXT NOT NULL)")
            .execute(&pool)
            .await
            .expect("caller table");
        sqlx::query("INSERT INTO caller_state(key, value) VALUES ('victoria', 'preserved')")
            .execute(&pool)
            .await
            .expect("caller row");
        apply_unledgered_baseline(&pool).await;
        sqlx::query(
            "INSERT INTO outbox_operations(operation_kind, expected_pubkey, semantic_scope, trade_id, mutation_id, canonical_payload_sha256, idempotency_key, operation_idempotency_digest, status, created_at_ms, updated_at_ms)
             VALUES ('post', 'author', 'generic_event', NULL, NULL, NULL, NULL, ?, 'queued', 1, 1)",
        )
        .bind("a".repeat(64))
        .execute(&pool)
        .await
        .expect("legacy row");

        assert_eq!(
            inspect_outbox_schema_status(&pool)
                .await
                .expect("unledgered status"),
            RadrootsOutboxSchemaStatus::UnledgeredBaseline
        );
        migrate_outbox_schema(&pool).await.expect("adoption");
        assert_eq!(ledger_count(&pool).await, 1);
        let caller: String =
            sqlx::query_scalar("SELECT value FROM caller_state WHERE key = 'victoria'")
                .fetch_one(&pool)
                .await
                .expect("caller row preserved");
        assert_eq!(caller, "preserved");
        let operations: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM outbox_operations")
            .fetch_one(&pool)
            .await
            .expect("legacy row count");
        assert_eq!(operations, 1);
    }

    #[tokio::test]
    async fn migration_test_only_destruction_handles_unledgered_and_uninitialized_schemas() {
        let pool = memory_pool().await;
        apply_unledgered_baseline(&pool).await;
        destroy_outbox_schema_for_migration_test(&pool)
            .await
            .expect("destroy unledgered baseline");
        assert_eq!(
            inspect_outbox_schema_status(&pool)
                .await
                .expect("uninitialized after destruction"),
            RadrootsOutboxSchemaStatus::Uninitialized
        );
        destroy_outbox_schema_for_migration_test(&pool)
            .await
            .expect("destroy uninitialized schema");
    }

    #[tokio::test]
    async fn migration_fresh_initialization_preserves_unrelated_caller_schema_and_rows() {
        let pool = memory_pool().await;
        sqlx::raw_sql(
            "CREATE TABLE caller_table (value TEXT NOT NULL);
             INSERT INTO caller_table(value) VALUES ('keep');
             CREATE INDEX caller_table_value_idx ON caller_table(value);",
        )
        .execute(&pool)
        .await
        .expect("caller schema");
        migrate_outbox_schema(&pool).await.expect("migration");
        let value: String = sqlx::query_scalar("SELECT value FROM caller_table")
            .fetch_one(&pool)
            .await
            .expect("caller row");
        assert_eq!(value, "keep");
        let index_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM main.sqlite_schema WHERE name = 'caller_table_value_idx'",
        )
        .fetch_one(&pool)
        .await
        .expect("caller index");
        assert_eq!(index_count, 1);
    }

    #[tokio::test]
    async fn migration_partial_changed_and_unknown_unledgered_catalogs_fail_before_adoption() {
        for mutation in [
            "DROP INDEX outbox_event_event_id_idx",
            "DROP INDEX outbox_event_event_id_idx; CREATE INDEX outbox_event_event_id_idx ON outbox_event(expected_pubkey)",
            "CREATE TABLE outbox_counterfeit (value TEXT NOT NULL)",
        ] {
            let pool = memory_pool().await;
            apply_unledgered_baseline(&pool).await;
            sqlx::raw_sql(mutation)
                .execute(&pool)
                .await
                .expect("catalog mutation");
            assert!(matches!(
                migrate_outbox_schema(&pool).await,
                Err(RadrootsOutboxError::UnmanagedSchema { .. })
            ));
            assert_eq!(ledger_count(&pool).await, 0);
        }
    }

    #[tokio::test]
    async fn migration_counterfeit_ledger_shape_fails_before_any_schema_mutation() {
        let pool = memory_pool().await;
        sqlx::query("CREATE TABLE radroots_outbox_schema_migrations (version INTEGER)")
            .execute(&pool)
            .await
            .expect("counterfeit ledger");
        assert!(matches!(
            migrate_outbox_schema(&pool).await,
            Err(RadrootsOutboxError::MigrationLedgerDrift { .. })
        ));
        let outbox_objects: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM main.sqlite_schema WHERE lower(substr(name, 1, 7)) = 'outbox_'",
        )
        .fetch_one(&pool)
        .await
        .expect("outbox object count");
        assert_eq!(outbox_objects, 0);
    }

    #[tokio::test]
    async fn migration_ledger_name_checksum_and_catalog_mutations_fail_closed() {
        for statement in [
            "UPDATE radroots_outbox_schema_migrations SET name = 'counterfeit' WHERE version = 1",
            "UPDATE radroots_outbox_schema_migrations SET up_sha256 = 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb' WHERE version = 1",
            "UPDATE radroots_outbox_schema_migrations SET schema_sha256 = 'cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc' WHERE version = 1",
            "DROP INDEX outbox_event_event_id_idx; CREATE INDEX outbox_event_event_id_idx ON outbox_event(expected_pubkey)",
        ] {
            let pool = memory_pool().await;
            migrate_outbox_schema(&pool).await.expect("migration");
            sqlx::raw_sql(statement)
                .execute(&pool)
                .await
                .expect("managed mutation");
            assert!(migrate_outbox_schema(&pool).await.is_err(), "{statement}");
            let rows: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM radroots_outbox_schema_migrations")
                    .fetch_one(&pool)
                    .await
                    .expect("ledger rows");
            assert_eq!(rows, 1);
        }
    }

    #[tokio::test]
    async fn migration_newer_history_and_governed_catalog_overflow_are_bounded_and_rejected() {
        let newer = memory_pool().await;
        migrate_outbox_schema(&newer).await.expect("migration");
        sqlx::query(
            "INSERT INTO radroots_outbox_schema_migrations(version, name, up_sha256, down_sha256, schema_sha256)
             VALUES (2, 'future', ?, ?, ?)",
        )
        .bind("a".repeat(64))
        .bind("b".repeat(64))
        .bind("c".repeat(64))
        .execute(&newer)
        .await
        .expect("future history");
        assert!(matches!(
            inspect_outbox_schema_status(&newer).await,
            Err(RadrootsOutboxError::SchemaTooNew {
                current: 1,
                database: 2
            })
        ));

        let overflow = memory_pool().await;
        migrate_outbox_schema(&overflow).await.expect("migration");
        sqlx::query("CREATE TABLE outbox_unknown (value TEXT)")
            .execute(&overflow)
            .await
            .expect("unknown governed object");
        assert!(matches!(
            inspect_outbox_schema_status(&overflow).await,
            Err(RadrootsOutboxError::GovernedCatalogCapacityExceeded { max: 14 })
        ));
    }

    #[test]
    fn migration_history_validation_rejects_gaps_and_unknown_versions() {
        let row = |version, migration: &OutboxMigration| AppliedMigration {
            version,
            name: migration.name.to_owned(),
            up_sha256: migration.up_sha256.to_owned(),
            down_sha256: migration.down_sha256.to_owned(),
            schema_sha256: migration.schema_sha256.to_owned(),
        };
        assert!(matches!(
            validate_history_against_registry(
                &[row(2, &OUTBOX_MIGRATIONS[0])],
                OUTBOX_MIGRATIONS,
                3
            ),
            Err(RadrootsOutboxError::MigrationHistoryGap {
                expected: 1,
                actual: Some(2)
            })
        ));
        assert!(matches!(
            validate_history_against_registry(
                &[row(1, &OUTBOX_MIGRATIONS[0]), row(2, &OUTBOX_MIGRATIONS[0])],
                OUTBOX_MIGRATIONS,
                3,
            ),
            Err(RadrootsOutboxError::UnknownMigration { version: 2 })
        ));

        assert!(matches!(
            validate_history_against_registry(&[], OUTBOX_MIGRATIONS, 1),
            Err(RadrootsOutboxError::MigrationLedgerDrift { .. })
        ));
        assert!(matches!(
            validate_history_against_registry(
                &[row(-1, &OUTBOX_MIGRATIONS[0])],
                OUTBOX_MIGRATIONS,
                1
            ),
            Err(RadrootsOutboxError::MigrationLedgerDrift { .. })
        ));
        assert_eq!(
            validate_history_against_registry(
                &[row(1, &OUTBOX_MIGRATIONS[0])],
                OUTBOX_MIGRATIONS,
                1
            )
            .expect("canonical history"),
            1
        );

        let mut name_drift = row(1, &OUTBOX_MIGRATIONS[0]);
        name_drift.name = "counterfeit".to_owned();
        assert!(matches!(
            validate_history_against_registry(&[name_drift], OUTBOX_MIGRATIONS, 1),
            Err(RadrootsOutboxError::MigrationHistoryNameDrift { .. })
        ));
        for field in ["up_sha256", "down_sha256", "schema_sha256"] {
            let mut checksum_drift = row(1, &OUTBOX_MIGRATIONS[0]);
            match field {
                "up_sha256" => checksum_drift.up_sha256 = "a".repeat(64),
                "down_sha256" => checksum_drift.down_sha256 = "a".repeat(64),
                "schema_sha256" => checksum_drift.schema_sha256 = "a".repeat(64),
                _ => unreachable!(),
            }
            assert!(matches!(
                validate_history_against_registry(&[checksum_drift], OUTBOX_MIGRATIONS, 1),
                Err(RadrootsOutboxError::MigrationHistoryChecksumDrift {
                    field: actual_field,
                    ..
                }) if actual_field == field
            ));
        }
    }

    fn catalog_row(
        object_type: &str,
        name: &str,
        table_name: &str,
        sql: Option<&str>,
    ) -> CatalogRow {
        CatalogRow {
            object_type: object_type.to_owned(),
            name: name.to_owned(),
            table_name: table_name.to_owned(),
            sql: sql.map(str::to_owned),
        }
    }

    #[test]
    fn migration_catalog_delta_and_ledger_validators_fail_closed() {
        let mut migration = OUTBOX_MIGRATIONS[0];
        migration.version = 2;
        migration.owned_object_names = &["outbox_future"];
        migration.owned_table_names = &["outbox_future"];
        let future = catalog_row(
            "table",
            "outbox_future",
            "outbox_future",
            Some("CREATE TABLE outbox_future (value TEXT)"),
        );
        validate_catalog_delta(&[], std::slice::from_ref(&future), &migration, "up")
            .expect("additive delta");
        validate_catalog_delta(std::slice::from_ref(&future), &[], &migration, "down")
            .expect("rollback delta");
        assert!(matches!(
            validate_catalog_delta(&[], std::slice::from_ref(&future), &migration, "sideways"),
            Err(RadrootsOutboxError::MigrationCatalogDeltaMismatch { .. })
        ));
        let changed = catalog_row(
            "table",
            "outbox_future",
            "outbox_future",
            Some("CREATE TABLE outbox_future (changed TEXT)"),
        );
        assert!(matches!(
            validate_catalog_delta(
                std::slice::from_ref(&future),
                std::slice::from_ref(&changed),
                &migration,
                "up"
            ),
            Err(RadrootsOutboxError::MigrationCatalogDeltaMismatch { .. })
        ));

        assert!(!validate_ledger_catalog(&[]).expect("absent ledger"));
        let canonical = catalog_row(
            "table",
            OUTBOX_LEDGER_NAME,
            OUTBOX_LEDGER_NAME,
            Some(OUTBOX_LEDGER_DDL),
        );
        assert!(validate_ledger_catalog(std::slice::from_ref(&canonical)).expect("ledger"));
        assert!(matches!(
            validate_ledger_catalog(&[canonical.clone(), canonical.clone()]),
            Err(RadrootsOutboxError::MigrationLedgerDrift { .. })
        ));
        for counterfeit in [
            catalog_row(
                "view",
                OUTBOX_LEDGER_NAME,
                OUTBOX_LEDGER_NAME,
                Some(OUTBOX_LEDGER_DDL),
            ),
            catalog_row(
                "table",
                "counterfeit",
                OUTBOX_LEDGER_NAME,
                Some(OUTBOX_LEDGER_DDL),
            ),
            catalog_row(
                "table",
                OUTBOX_LEDGER_NAME,
                "counterfeit",
                Some(OUTBOX_LEDGER_DDL),
            ),
            catalog_row(
                "table",
                OUTBOX_LEDGER_NAME,
                OUTBOX_LEDGER_NAME,
                Some("counterfeit"),
            ),
        ] {
            assert!(matches!(
                validate_ledger_catalog(&[counterfeit]),
                Err(RadrootsOutboxError::MigrationLedgerDrift { .. })
            ));
        }
    }

    #[tokio::test]
    async fn temporary_authority_collisions_are_rejected_before_migration() {
        for sql in [
            "CREATE TEMP TABLE outbox_event (value TEXT)",
            "CREATE TEMP TABLE radroots_outbox_schema_migrations (value TEXT)",
            "CREATE TEMP VIEW caller_view AS SELECT 1 AS value",
        ] {
            let pool = memory_pool().await;
            sqlx::raw_sql(sql)
                .execute(&pool)
                .await
                .expect("temp fixture");
            assert!(matches!(
                migrate_outbox_schema(&pool).await,
                Err(RadrootsOutboxError::TemporarySchemaCollision { .. })
            ));
            assert_eq!(ledger_count(&pool).await, 0);
        }
    }

    #[tokio::test]
    async fn migration_current_schema_reopen_is_idempotent_and_does_not_rewrite_history() {
        let pool = memory_pool().await;
        migrate_outbox_schema(&pool).await.expect("first migration");
        let before_changes: i64 = sqlx::query_scalar("SELECT total_changes()")
            .fetch_one(&pool)
            .await
            .expect("before total changes");
        migrate_outbox_schema(&pool)
            .await
            .expect("current migration");
        let after_changes: i64 = sqlx::query_scalar("SELECT total_changes()")
            .fetch_one(&pool)
            .await
            .expect("after total changes");
        assert_eq!(before_changes, after_changes);
        assert_eq!(ledger_count(&pool).await, 1);
    }

    #[tokio::test]
    async fn migration_rollback_wrapper_rejects_exactly_below_the_registry_floor() {
        let pool = memory_pool().await;
        let mut connection = pool.acquire().await.expect("connection");
        assert!(matches!(
            rollback_outbox_schema_on_connection(
                &mut connection,
                RADROOTS_OUTBOX_SCHEMA_VERSION_MIN - 1,
            )
            .await,
            Err(RadrootsOutboxError::RollbackBelowVersionFloor {
                floor: RADROOTS_OUTBOX_SCHEMA_VERSION_MIN,
                target: 0,
            })
        ));
    }
}
