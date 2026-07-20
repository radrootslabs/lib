use crate::RadrootsEventStoreError;
use crate::migrations::{
    EVENT_STORE_LEDGER_DDL, EVENT_STORE_LEDGER_NAME, EVENT_STORE_MIGRATIONS,
    EVENT_STORE_RESERVED_PREFIX, EventStoreMigration, RADROOTS_EVENT_STORE_SCHEMA_VERSION_CURRENT,
    RADROOTS_EVENT_STORE_SCHEMA_VERSION_MIN, migration_for_version,
    validate_embedded_migration_registry, validate_migration_registry,
};
use sha2::{Digest, Sha256};
use sqlx::{Row, Sqlite, SqliteConnection, SqlitePool, Transaction};
use std::collections::{BTreeMap, BTreeSet};

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
    validate_embedded_migration_registry()?;
    migrate_event_store_schema_with_registry(
        pool,
        EVENT_STORE_MIGRATIONS,
        RADROOTS_EVENT_STORE_SCHEMA_VERSION_MIN,
        RADROOTS_EVENT_STORE_SCHEMA_VERSION_CURRENT,
    )
    .await
}

async fn migrate_event_store_schema_with_registry(
    pool: &SqlitePool,
    registry: &[EventStoreMigration],
    minimum: u32,
    supported_current: u32,
) -> Result<(), RadrootsEventStoreError> {
    validate_migration_registry(registry, minimum, supported_current)?;
    if inspect_event_store_schema_status_with_registry(pool, registry, supported_current).await?
        == (RadrootsEventStoreSchemaStatus::Managed {
            version: supported_current,
        })
    {
        return Ok(());
    }

    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
    let result = migrate_schema_on_connection(&mut transaction, registry, supported_current).await;
    finish_schema_transaction(transaction, result).await
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

async fn rollback_event_store_schema_with_registry(
    pool: &SqlitePool,
    registry: &[EventStoreMigration],
    minimum: u32,
    supported_current: u32,
    target: u32,
) -> Result<(), RadrootsEventStoreError> {
    if target < minimum {
        return Err(RadrootsEventStoreError::RollbackBelowVersionFloor {
            floor: minimum,
            target,
        });
    }
    validate_migration_registry(registry, minimum, supported_current)?;
    let mut transaction = pool.begin_with("BEGIN EXCLUSIVE").await?;
    let result =
        rollback_schema_on_connection(&mut transaction, registry, supported_current, target).await;
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
        apply_migration_up(connection, registry, migration).await?;
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
        let deleted =
            sqlx::query("DELETE FROM radroots_event_store_schema_migrations WHERE version = ?")
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
    }

    validate_database_integrity(connection, registry).await?;
    match inspect_schema_on_connection(connection, registry, supported_current).await? {
        RadrootsEventStoreSchemaStatus::Managed { version } if version == target => Ok(()),
        status => Err(RadrootsEventStoreError::MigrationLedgerDrift {
            reason: format!("rollback completed in unexpected state {status:?}"),
        }),
    }
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
                    "DELETE FROM radroots_event_store_schema_migrations WHERE version = ?",
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
            sqlx::query("DROP TABLE radroots_event_store_schema_migrations")
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

    let valid = match direction {
        "up" => added == expected && removed.is_empty() && changed.is_empty(),
        "down" => removed == expected && added.is_empty() && changed.is_empty(),
        _ => false,
    };
    if !valid {
        return Err(RadrootsEventStoreError::MigrationCatalogDeltaMismatch {
            version: migration.version,
            direction,
            reason: format!(
                "expected {} objects {expected:?}; added {added:?}, removed {removed:?}, changed {changed:?}",
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
    sqlx::query(EVENT_STORE_LEDGER_DDL)
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
        "INSERT INTO radroots_event_store_schema_migrations(version, name, up_sha256, down_sha256, schema_sha256) VALUES (?, ?, ?, ?, ?)",
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
    Ok(RadrootsEventStoreSchemaStatus::Managed { version: current })
}

async fn read_catalog(
    connection: &mut SqliteConnection,
) -> Result<Vec<CatalogRow>, RadrootsEventStoreError> {
    let rows = sqlx::query(
        "SELECT type, name, tbl_name, sql FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%'",
    )
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

fn validate_ledger_catalog(catalog: &[CatalogRow]) -> Result<bool, RadrootsEventStoreError> {
    let rows = catalog
        .iter()
        .filter(|row| {
            row.name == EVENT_STORE_LEDGER_NAME || row.table_name == EVENT_STORE_LEDGER_NAME
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
        .filter(|row| row.name != EVENT_STORE_LEDGER_NAME)
        .filter(|row| {
            registry.iter().any(|migration| {
                migration.owned_object_names.contains(&row.name.as_str())
                    || migration
                        .owned_table_names
                        .contains(&row.table_name.as_str())
            }) || row.name.starts_with(EVENT_STORE_RESERVED_PREFIX)
                || row.table_name.starts_with(EVENT_STORE_RESERVED_PREFIX)
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
        "SELECT version, name, up_sha256, down_sha256, schema_sha256 FROM radroots_event_store_schema_migrations ORDER BY version",
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
                .any(|row| row.object_type == "table" && row.name == *table)
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
        if registry
            .iter()
            .any(|migration| migration.owned_table_names.contains(&table.as_str()))
            || table.starts_with(EVENT_STORE_RESERVED_PREFIX)
        {
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
    use std::time::{Duration, Instant};

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
            owned_table_names: SYNTHETIC_V2_TABLE_NAMES,
            fts5_table_names: NO_FTS5_TABLES,
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
            RadrootsEventStoreSchemaStatus::Managed { version: 1 }
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
            assert_eq!(history_count, 1);
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
            RadrootsEventStoreSchemaStatus::Managed { version: 1 }
        );
    }

    #[tokio::test]
    async fn exact_legacy_adoption_preserves_rows() {
        let pool = memory_pool().await;
        install_unledgered_baseline(&pool).await;
        sqlx::query(
            "INSERT INTO event_envelopes(event_id, pubkey, created_at, kind, tags_json, content, sig, raw_json, verification_status, contract_status, contract_id, event_class, projection_eligible, inserted_at_ms, updated_at_ms) VALUES ('legacy', 'author', 1, 1, '[]', 'content', 'sig', '{}', 'verified', 'unsupported', NULL, 'regular', 0, 1, 1)",
        )
        .execute(&pool)
        .await
        .expect("legacy row");

        migrate_event_store_schema(&pool)
            .await
            .expect("legacy adoption");
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM event_envelopes WHERE event_id = 'legacy'")
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

        let reserved = memory_pool().await;
        sqlx::query(
            "CREATE TABLE radroots_event_store_shared_collision (id INTEGER PRIMARY KEY) STRICT",
        )
        .execute(&reserved)
        .await
        .expect("reserved namespace collision");
        assert!(matches!(
            migrate_event_store_schema(&reserved).await,
            Err(RadrootsEventStoreError::UnmanagedSchema { .. })
        ));

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
            Err(RadrootsEventStoreError::MigrationLedgerDrift { reason })
                if reason.contains("without migration history")
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
        sqlx::query("UPDATE radroots_event_store_schema_migrations SET name = 'counterfeit'")
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
                current: 1,
                database: i64::MAX
            })
        ));
    }

    #[test]
    fn history_validator_rejects_gaps_and_unknown_versions() {
        let row = |version| AppliedMigration {
            version,
            name: "event_store".to_owned(),
            up_sha256: EVENT_STORE_MIGRATIONS[0].up_sha256.to_owned(),
            down_sha256: EVENT_STORE_MIGRATIONS[0].down_sha256.to_owned(),
            schema_sha256: EVENT_STORE_MIGRATIONS[0].schema_sha256.to_owned(),
        };
        assert!(matches!(
            validate_history_against_registry(&[row(2)], EVENT_STORE_MIGRATIONS, 2),
            Err(RadrootsEventStoreError::MigrationHistoryGap {
                expected: 1,
                actual: Some(2)
            })
        ));

        assert!(matches!(
            validate_history_against_registry(&[row(1), row(2)], EVENT_STORE_MIGRATIONS, 2),
            Err(RadrootsEventStoreError::UnknownMigration { version: 2 })
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
    async fn rollback_rejects_below_floor_ahead_and_unmanaged_targets() {
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
        assert!(matches!(
            rollback_event_store_schema_offline(&managed, 2).await,
            Err(RadrootsEventStoreError::RollbackAhead {
                current: 1,
                target: 2
            })
        ));
        rollback_event_store_schema_offline(&managed, 1)
            .await
            .expect("idempotent rollback");
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
            owned_table_names: OBJECTS,
            fts5_table_names: NO_FTS5_TABLES,
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
            Err(RadrootsEventStoreError::ForeignKeyViolation { .. })
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
            RadrootsEventStoreSchemaStatus::Managed { version: 1 }
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
            owned_table_names: OBJECTS,
            fts5_table_names: NO_FTS5_TABLES,
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
                .busy_timeout(Duration::from_millis(100))
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
        let started = Instant::now();
        migrate_event_store_schema(&second)
            .await
            .expect("read-only fast path");
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "current-schema migration attempted to wait for a writer lock"
        );
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
        assert_eq!(history_count, 1);
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
            RadrootsEventStoreSchemaStatus::Managed { version: 1 }
        );
        assert_eq!(
            second.schema_status().await.expect("second status"),
            RadrootsEventStoreSchemaStatus::Managed { version: 1 }
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
