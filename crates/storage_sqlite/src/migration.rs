//! Governed SQLite schema migration boundary.

use crate::{Error, OpenMode};
use sqlx::{Connection, Row, SqliteConnection, sqlite::SqliteConnectOptions};

mod authored_v10;
pub use authored_v10::AuthoredV10Preflight;

/// Inspects an exact V10 runtime database without mutating it.
#[cfg_attr(coverage_nightly, coverage(off))]
pub async fn preflight_authored_v10(paths: &crate::Paths) -> Result<AuthoredV10Preflight, Error> {
    paths.validate_filesystem(OpenMode::ReadOnly)?;
    let mut connection = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(paths.runtime())
            .read_only(true),
    )
    .await
    .map_err(|_| Error::DatabaseOpenFailed {
        database: RUNTIME_DATABASE,
    })?;
    sqlx::raw_sql("PRAGMA query_only = ON")
        .execute(&mut connection)
        .await
        .map_err(|_| Error::SchemaMetadataUnavailable {
            database: RUNTIME_DATABASE,
        })?;
    let current = metadata(&mut connection, RUNTIME_DATABASE).await?;
    if current.application_id != RUNTIME_APPLICATION_ID {
        return Err(Error::SchemaIdentityMismatch {
            database: RUNTIME_DATABASE,
            expected: RUNTIME_APPLICATION_ID,
            actual: current.application_id,
        });
    }
    if current.version < 10 {
        return Err(Error::SchemaMigrationRequired {
            database: RUNTIME_DATABASE,
            current: 10,
            actual: current.version,
        });
    }
    if current.version > 10 {
        return Err(Error::SchemaTooNew {
            database: RUNTIME_DATABASE,
            supported: 10,
            actual: current.version,
        });
    }
    validate_exact_catalog(
        &mut connection,
        RUNTIME_DATABASE,
        10,
        runtime::MIGRATIONS[9].owned_objects(),
    )
    .await?;
    let report = authored_v10::inspect(&mut connection).await?.report;
    connection
        .close()
        .await
        .map_err(|_| Error::DatabaseCloseFailed {
            database: RUNTIME_DATABASE,
        })?;
    Ok(report)
}

/// Versioned schema authority for `private.sqlite`.
pub mod private;
/// Versioned schema authority for `runtime.sqlite`.
pub mod runtime;

const RUNTIME_DATABASE: &str = "runtime.sqlite";
const PRIVATE_DATABASE: &str = "private.sqlite";
const RUNTIME_APPLICATION_ID: u32 = 1_380_209_236;
const PRIVATE_APPLICATION_ID: u32 = 1_380_208_722;

const SET_RUNTIME_APPLICATION_ID: &str = "PRAGMA application_id = 1380209236";
const SET_PRIVATE_APPLICATION_ID: &str = "PRAGMA application_id = 1380208722";

#[derive(Clone, Copy)]
struct MigrationStep {
    version: u32,
    sql: &'static str,
    owned_objects: &'static [&'static str],
}

struct MigrationPlan {
    database: &'static str,
    application_id: u32,
    set_application_id_sql: &'static str,
    minimum_version: u32,
    current_version: u32,
    steps: Vec<MigrationStep>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MigrationReport {
    initial_version: u32,
    final_version: u32,
    applied: u32,
}

impl MigrationReport {
    #[allow(dead_code)] // Read by the public open lifecycle in its ordered RCL checkpoint.
    pub(crate) const fn initial_version(self) -> u32 {
        self.initial_version
    }

    #[allow(dead_code)] // Read by the public open lifecycle in its ordered RCL checkpoint.
    pub(crate) const fn final_version(self) -> u32 {
        self.final_version
    }

    #[allow(dead_code)] // Read by the public open lifecycle in its ordered RCL checkpoint.
    pub(crate) const fn applied(self) -> u32 {
        self.applied
    }
}

#[allow(dead_code)] // Wired into the public open lifecycle in its ordered RCL checkpoint.
#[cfg_attr(coverage_nightly, coverage(off))]
pub(crate) async fn migrate_runtime(
    connection: &mut SqliteConnection,
    mode: OpenMode,
) -> Result<MigrationReport, Error> {
    let steps = runtime::MIGRATIONS
        .iter()
        .map(|migration| {
            Ok(MigrationStep {
                version: migration.version(),
                sql: runtime::migration_sql(migration.version()).ok_or(
                    Error::SchemaMetadataUnavailable {
                        database: RUNTIME_DATABASE,
                    },
                )?,
                owned_objects: migration.owned_objects(),
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;
    migrate(
        connection,
        mode,
        &MigrationPlan {
            database: RUNTIME_DATABASE,
            application_id: RUNTIME_APPLICATION_ID,
            set_application_id_sql: SET_RUNTIME_APPLICATION_ID,
            minimum_version: runtime::MINIMUM_VERSION,
            current_version: runtime::CURRENT_VERSION,
            steps,
        },
    )
    .await
}

#[allow(dead_code)] // Wired into the public open lifecycle in its ordered RCL checkpoint.
#[cfg_attr(coverage_nightly, coverage(off))]
pub(crate) async fn migrate_private(
    connection: &mut SqliteConnection,
    mode: OpenMode,
) -> Result<MigrationReport, Error> {
    let steps = private::MIGRATIONS
        .iter()
        .map(|migration| {
            Ok(MigrationStep {
                version: migration.version(),
                sql: private::migration_sql(migration.version()).ok_or(
                    Error::SchemaMetadataUnavailable {
                        database: PRIVATE_DATABASE,
                    },
                )?,
                owned_objects: migration.owned_objects(),
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;
    migrate(
        connection,
        mode,
        &MigrationPlan {
            database: PRIVATE_DATABASE,
            application_id: PRIVATE_APPLICATION_ID,
            set_application_id_sql: SET_PRIVATE_APPLICATION_ID,
            minimum_version: private::MINIMUM_VERSION,
            current_version: private::CURRENT_VERSION,
            steps,
        },
    )
    .await
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn migrate(
    connection: &mut SqliteConnection,
    mode: OpenMode,
    plan: &MigrationPlan,
) -> Result<MigrationReport, Error> {
    validate_plan(plan)?;
    let initial = metadata(connection, plan.database).await?;
    validate_metadata(plan, initial)?;
    validate_catalog(connection, plan, initial.version).await?;

    if initial.version == plan.current_version {
        return Ok(MigrationReport {
            initial_version: initial.version,
            final_version: initial.version,
            applied: 0,
        });
    }
    if !mode.is_writable() {
        if plan.database == RUNTIME_DATABASE && initial.version == 10 {
            let inspected = authored_v10::inspect(connection).await?;
            if !inspected.report.is_eligible() {
                return Err(inspected.report.blocked_error());
            }
        }
        return Err(Error::SchemaMigrationRequired {
            database: plan.database,
            current: plan.current_version,
            actual: initial.version,
        });
    }

    let mut transaction = connection
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|_| Error::SchemaMigrationFailed {
            database: plan.database,
            target_version: initial.version.saturating_add(1),
        })?;
    let transactional = match metadata(&mut transaction, plan.database).await {
        Ok(metadata) => metadata,
        Err(error) => {
            let _rollback = transaction.rollback().await;
            return Err(error);
        }
    };
    if transactional != initial {
        let _rollback = transaction.rollback().await;
        return Err(Error::SchemaMetadataUnavailable {
            database: plan.database,
        });
    }

    if initial.version == 0
        && sqlx::raw_sql(plan.set_application_id_sql)
            .execute(&mut *transaction)
            .await
            .is_err()
    {
        let error = Error::SchemaMigrationFailed {
            database: plan.database,
            target_version: 1,
        };
        let _rollback = transaction.rollback().await;
        return Err(error);
    }

    let mut applied = 0_u32;
    for step in plan
        .steps
        .iter()
        .filter(|step| step.version > initial.version)
    {
        let inspected_v10 = if plan.database == RUNTIME_DATABASE && step.version == 11 {
            match authored_v10::inspect(&mut transaction).await {
                Ok(inspected) if inspected.report.is_eligible() => Some(inspected),
                Ok(inspected) => {
                    let error = inspected.report.blocked_error();
                    let _rollback = transaction.rollback().await;
                    return Err(error);
                }
                Err(error) => {
                    let _rollback = transaction.rollback().await;
                    return Err(error);
                }
            }
        } else {
            None
        };
        let version_sql =
            set_user_version_sql(step.version).ok_or(Error::SchemaMigrationFailed {
                database: plan.database,
                target_version: step.version,
            })?;
        if sqlx::raw_sql(step.sql)
            .execute(&mut *transaction)
            .await
            .is_err()
        {
            let error = Error::SchemaMigrationFailed {
                database: plan.database,
                target_version: step.version,
            };
            let _rollback = transaction.rollback().await;
            return Err(error);
        }
        if let Some(inspected) = inspected_v10.as_ref()
            && let Err(error) = authored_v10::apply(&mut transaction, inspected).await
        {
            let _rollback = transaction.rollback().await;
            return Err(error);
        }
        if sqlx::raw_sql(version_sql)
            .execute(&mut *transaction)
            .await
            .is_err()
        {
            let error = Error::SchemaMigrationFailed {
                database: plan.database,
                target_version: step.version,
            };
            let _rollback = transaction.rollback().await;
            return Err(error);
        }
        if validate_exact_catalog(
            &mut transaction,
            plan.database,
            step.version,
            step.owned_objects,
        )
        .await
        .is_err()
        {
            let error = Error::SchemaMigrationFailed {
                database: plan.database,
                target_version: step.version,
            };
            let _rollback = transaction.rollback().await;
            return Err(error);
        }
        applied = applied.saturating_add(1);
    }
    transaction
        .commit()
        .await
        .map_err(|_| Error::SchemaMigrationFailed {
            database: plan.database,
            target_version: plan.current_version,
        })?;
    Ok(MigrationReport {
        initial_version: initial.version,
        final_version: plan.current_version,
        applied,
    })
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct SchemaMetadata {
    application_id: u32,
    version: u32,
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn metadata(
    connection: &mut SqliteConnection,
    database: &'static str,
) -> Result<SchemaMetadata, Error> {
    let application_id = sqlx::query_scalar::<_, i64>("PRAGMA application_id")
        .fetch_one(&mut *connection)
        .await
        .map_err(|_| Error::SchemaMetadataUnavailable { database })?;
    let version = sqlx::query_scalar::<_, i64>("PRAGMA user_version")
        .fetch_one(&mut *connection)
        .await
        .map_err(|_| Error::SchemaMetadataUnavailable { database })?;
    Ok(SchemaMetadata {
        application_id: u32::try_from(application_id)
            .map_err(|_| Error::SchemaMetadataUnavailable { database })?,
        version: u32::try_from(version)
            .map_err(|_| Error::SchemaMetadataUnavailable { database })?,
    })
}

fn validate_plan(plan: &MigrationPlan) -> Result<(), Error> {
    let valid = plan.minimum_version > 0
        && plan.minimum_version <= plan.current_version
        && plan.current_version <= 11
        && plan.steps.len() == usize::try_from(plan.current_version).unwrap_or(usize::MAX)
        && plan
            .steps
            .iter()
            .enumerate()
            .all(|(index, step)| step.version == u32::try_from(index + 1).unwrap_or(u32::MAX));
    if valid {
        Ok(())
    } else {
        Err(Error::SchemaMetadataUnavailable {
            database: plan.database,
        })
    }
}

fn validate_metadata(plan: &MigrationPlan, metadata: SchemaMetadata) -> Result<(), Error> {
    if metadata.version > plan.current_version {
        return Err(Error::SchemaTooNew {
            database: plan.database,
            supported: plan.current_version,
            actual: metadata.version,
        });
    }
    if metadata.version > 0 && metadata.version < plan.minimum_version {
        return Err(Error::SchemaTooOld {
            database: plan.database,
            minimum: plan.minimum_version,
            actual: metadata.version,
        });
    }
    let expected_application_id = if metadata.version == 0 {
        0
    } else {
        plan.application_id
    };
    if metadata.application_id != expected_application_id {
        return Err(Error::SchemaIdentityMismatch {
            database: plan.database,
            expected: expected_application_id,
            actual: metadata.application_id,
        });
    }
    Ok(())
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn validate_catalog(
    connection: &mut SqliteConnection,
    plan: &MigrationPlan,
    version: u32,
) -> Result<(), Error> {
    let expected = if version == 0 {
        &[][..]
    } else {
        plan.steps
            .get(
                usize::try_from(version - 1).map_err(|_| Error::SchemaCatalogMismatch {
                    database: plan.database,
                    version,
                })?,
            )
            .ok_or(Error::SchemaCatalogMismatch {
                database: plan.database,
                version,
            })?
            .owned_objects
    };
    validate_exact_catalog(connection, plan.database, version, expected).await
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn validate_exact_catalog(
    connection: &mut SqliteConnection,
    database: &'static str,
    version: u32,
    expected: &[&str],
) -> Result<(), Error> {
    let rows = sqlx::query(
        "SELECT name FROM sqlite_schema
         WHERE name NOT LIKE 'sqlite_%'
         ORDER BY name",
    )
    .fetch_all(&mut *connection)
    .await
    .map_err(|_| Error::SchemaCatalogMismatch { database, version })?;
    let actual = rows
        .iter()
        .map(|row| row.get::<String, _>("name"))
        .collect::<Vec<_>>();
    if actual
        .iter()
        .map(String::as_str)
        .eq(expected.iter().copied())
    {
        Ok(())
    } else if version == 0 {
        Err(Error::UnrecognizedSchema { database })
    } else {
        Err(Error::SchemaCatalogMismatch { database, version })
    }
}

const fn set_user_version_sql(version: u32) -> Option<&'static str> {
    match version {
        1 => Some("PRAGMA user_version = 1"),
        2 => Some("PRAGMA user_version = 2"),
        3 => Some("PRAGMA user_version = 3"),
        4 => Some("PRAGMA user_version = 4"),
        5 => Some("PRAGMA user_version = 5"),
        6 => Some("PRAGMA user_version = 6"),
        7 => Some("PRAGMA user_version = 7"),
        8 => Some("PRAGMA user_version = 8"),
        9 => Some("PRAGMA user_version = 9"),
        10 => Some("PRAGMA user_version = 10"),
        11 => Some("PRAGMA user_version = 11"),
        _ => None,
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use sqlx::sqlite::SqliteConnectOptions;

    use super::*;

    const TEST_V1_OBJECTS: &[&str] = &["radroots_test_one"];
    const TEST_V2_OBJECTS: &[&str] = &["radroots_test_one", "radroots_test_two"];

    async fn connection() -> SqliteConnection {
        SqliteConnection::connect("sqlite::memory:")
            .await
            .expect("memory SQLite")
    }

    async fn pragma(connection: &mut SqliteConnection, name: &str) -> i64 {
        let sql = match name {
            "application_id" => "PRAGMA application_id",
            "user_version" => "PRAGMA user_version",
            _ => panic!("unsupported test pragma"),
        };
        sqlx::query_scalar(sql)
            .fetch_one(connection)
            .await
            .expect("pragma")
    }

    async fn establish_runtime_version(connection: &mut SqliteConnection, version: u32) {
        for migration_version in 1..=version {
            sqlx::raw_sql(
                runtime::migration_sql(migration_version).expect("registered runtime SQL"),
            )
            .execute(&mut *connection)
            .await
            .expect("runtime migration");
        }
        sqlx::raw_sql(SET_RUNTIME_APPLICATION_ID)
            .execute(&mut *connection)
            .await
            .expect("runtime application id");
        sqlx::raw_sql(set_user_version_sql(version).expect("version pragma"))
            .execute(&mut *connection)
            .await
            .expect("runtime user version");
    }

    async fn establish_private_version(connection: &mut SqliteConnection, version: u32) {
        for migration_version in 1..=version {
            sqlx::raw_sql(
                private::migration_sql(migration_version).expect("registered private SQL"),
            )
            .execute(&mut *connection)
            .await
            .expect("private migration");
        }
        sqlx::raw_sql(SET_PRIVATE_APPLICATION_ID)
            .execute(&mut *connection)
            .await
            .expect("private application id");
        sqlx::raw_sql(set_user_version_sql(version).expect("version pragma"))
            .execute(&mut *connection)
            .await
            .expect("private user version");
    }

    #[tokio::test]
    async fn fresh_runtime_and_private_schemas_migrate_to_exact_current_versions() {
        let mut runtime_connection = connection().await;
        let runtime_report = migrate_runtime(&mut runtime_connection, OpenMode::Create)
            .await
            .expect("runtime migrations");
        assert_eq!(runtime_report.initial_version(), 0);
        assert_eq!(runtime_report.final_version(), runtime::CURRENT_VERSION);
        assert_eq!(runtime_report.applied(), runtime::CURRENT_VERSION);
        assert_eq!(
            pragma(&mut runtime_connection, "application_id").await,
            i64::from(RUNTIME_APPLICATION_ID)
        );
        assert_eq!(
            pragma(&mut runtime_connection, "user_version").await,
            i64::from(runtime::CURRENT_VERSION)
        );
        assert_eq!(
            migrate_runtime(&mut runtime_connection, OpenMode::ReadOnly)
                .await
                .expect("current read-only runtime"),
            MigrationReport {
                initial_version: runtime::CURRENT_VERSION,
                final_version: runtime::CURRENT_VERSION,
                applied: 0,
            }
        );

        let mut private_connection = connection().await;
        let private_report = migrate_private(&mut private_connection, OpenMode::Create)
            .await
            .expect("private migrations");
        assert_eq!(private_report.initial_version(), 0);
        assert_eq!(private_report.final_version(), private::CURRENT_VERSION);
        assert_eq!(private_report.applied(), private::CURRENT_VERSION);
        assert_eq!(
            pragma(&mut private_connection, "application_id").await,
            i64::from(PRIVATE_APPLICATION_ID)
        );
        assert_eq!(
            pragma(&mut private_connection, "user_version").await,
            i64::from(private::CURRENT_VERSION)
        );
    }

    #[tokio::test]
    async fn recognized_runtime_schema_upgrades_forward_and_preserves_data() {
        let mut connection = connection().await;
        establish_runtime_version(&mut connection, 1).await;
        sqlx::query(
            "INSERT INTO radroots_runtime_source_generations (
               generation, state, created_at_unix_ms
             ) VALUES (?, 'active', 10)",
        )
        .bind([7_u8; 32].as_slice())
        .execute(&mut connection)
        .await
        .expect("v1 data");

        let report = migrate_runtime(&mut connection, OpenMode::ReadWriteExisting)
            .await
            .expect("forward migration");
        assert_eq!(report.initial_version(), 1);
        assert_eq!(report.final_version(), runtime::CURRENT_VERSION);
        assert_eq!(report.applied(), runtime::CURRENT_VERSION - 1);
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM radroots_runtime_source_generations",
            )
            .fetch_one(&mut connection)
            .await
            .expect("preserved data"),
            1
        );
    }

    #[tokio::test]
    async fn committed_migration_reopens_as_current_after_a_lost_success_response() {
        let directory = tempfile::tempdir().expect("database directory");
        let path = directory.path().join(RUNTIME_DATABASE);
        let mut connection = SqliteConnection::connect_with(
            &SqliteConnectOptions::new()
                .filename(&path)
                .create_if_missing(true),
        )
        .await
        .expect("open migration database");
        establish_runtime_version(&mut connection, 1).await;
        let _lost_response = migrate_runtime(&mut connection, OpenMode::ReadWriteExisting)
            .await
            .expect("commit pending migration");
        connection.close().await.expect("simulate process exit");

        let mut reopened = SqliteConnection::connect_with(
            &SqliteConnectOptions::new().filename(&path).read_only(true),
        )
        .await
        .expect("reopen migrated database");
        let report = migrate_runtime(&mut reopened, OpenMode::ReadOnly)
            .await
            .expect("recognize committed migration");
        assert_eq!(report.initial_version(), runtime::CURRENT_VERSION);
        assert_eq!(report.final_version(), runtime::CURRENT_VERSION);
        assert_eq!(report.applied(), 0);
    }

    #[tokio::test]
    async fn every_recognized_runtime_version_applies_exactly_the_pending_suffix() {
        for initial_version in 1..=runtime::CURRENT_VERSION {
            let mut connection = connection().await;
            establish_runtime_version(&mut connection, initial_version).await;
            let report = migrate_runtime(&mut connection, OpenMode::ReadWriteExisting)
                .await
                .expect("recognized forward migration");
            assert_eq!(report.initial_version(), initial_version);
            assert_eq!(report.final_version(), runtime::CURRENT_VERSION);
            assert_eq!(report.applied(), runtime::CURRENT_VERSION - initial_version);
        }
    }

    #[tokio::test]
    async fn every_recognized_private_version_applies_exactly_the_pending_suffix() {
        for initial_version in 1..=private::CURRENT_VERSION {
            let mut connection = connection().await;
            establish_private_version(&mut connection, initial_version).await;
            let report = migrate_private(&mut connection, OpenMode::ReadWriteExisting)
                .await
                .expect("recognized private forward migration");
            assert_eq!(report.initial_version(), initial_version);
            assert_eq!(report.final_version(), private::CURRENT_VERSION);
            assert_eq!(report.applied(), private::CURRENT_VERSION - initial_version);
        }
    }

    #[tokio::test]
    async fn read_only_old_schema_requires_migration_without_mutating() {
        let mut connection = connection().await;
        establish_runtime_version(&mut connection, 1).await;
        assert!(matches!(
            migrate_runtime(&mut connection, OpenMode::ReadOnly).await,
            Err(Error::SchemaMigrationRequired {
                database: RUNTIME_DATABASE,
                current: runtime::CURRENT_VERSION,
                actual: 1,
            })
        ));
        assert_eq!(pragma(&mut connection, "user_version").await, 1);
    }

    #[tokio::test]
    async fn newer_wrong_identity_and_unversioned_nonempty_schemas_fail_closed() {
        let mut newer = connection().await;
        sqlx::raw_sql(SET_RUNTIME_APPLICATION_ID)
            .execute(&mut newer)
            .await
            .expect("application id");
        sqlx::raw_sql("PRAGMA user_version = 12")
            .execute(&mut newer)
            .await
            .expect("newer version");
        assert!(matches!(
            migrate_runtime(&mut newer, OpenMode::ReadWriteExisting).await,
            Err(Error::SchemaTooNew {
                database: RUNTIME_DATABASE,
                supported: runtime::CURRENT_VERSION,
                actual: 12,
            })
        ));
        assert_eq!(pragma(&mut newer, "user_version").await, 12);

        let mut wrong_identity = connection().await;
        establish_runtime_version(&mut wrong_identity, 1).await;
        assert!(matches!(
            migrate_private(&mut wrong_identity, OpenMode::ReadWriteExisting).await,
            Err(Error::SchemaIdentityMismatch {
                database: PRIVATE_DATABASE,
                expected: PRIVATE_APPLICATION_ID,
                actual: RUNTIME_APPLICATION_ID,
            })
        ));

        let mut unknown = connection().await;
        sqlx::query("CREATE TABLE unrelated(value INTEGER)")
            .execute(&mut unknown)
            .await
            .expect("unknown table");
        assert!(matches!(
            migrate_runtime(&mut unknown, OpenMode::Create).await,
            Err(Error::UnrecognizedSchema {
                database: RUNTIME_DATABASE,
            })
        ));
        assert_eq!(pragma(&mut unknown, "application_id").await, 0);
        assert_eq!(pragma(&mut unknown, "user_version").await, 0);
    }

    #[tokio::test]
    async fn current_version_with_incomplete_catalog_is_rejected() {
        let mut connection = connection().await;
        sqlx::raw_sql(SET_PRIVATE_APPLICATION_ID)
            .execute(&mut connection)
            .await
            .expect("private application id");
        sqlx::raw_sql("PRAGMA user_version = 1")
            .execute(&mut connection)
            .await
            .expect("private version");
        assert!(matches!(
            migrate_private(&mut connection, OpenMode::ReadOnly).await,
            Err(Error::SchemaCatalogMismatch {
                database: PRIVATE_DATABASE,
                version: 1,
            })
        ));
    }

    #[tokio::test]
    async fn any_failed_step_rolls_back_the_entire_pending_plan() {
        let mut connection = connection().await;
        let plan = MigrationPlan {
            database: "test.sqlite",
            application_id: 4_242,
            set_application_id_sql: "PRAGMA application_id = 4242",
            minimum_version: 1,
            current_version: 2,
            steps: vec![
                MigrationStep {
                    version: 1,
                    sql: "CREATE TABLE radroots_test_one(value INTEGER)",
                    owned_objects: TEST_V1_OBJECTS,
                },
                MigrationStep {
                    version: 2,
                    sql: "CREATE TABLE radroots_test_two(value INTEGER);
                          INSERT INTO radroots_missing VALUES (1)",
                    owned_objects: TEST_V2_OBJECTS,
                },
            ],
        };
        assert!(matches!(
            migrate(&mut connection, OpenMode::Create, &plan).await,
            Err(Error::SchemaMigrationFailed {
                database: "test.sqlite",
                target_version: 2,
            })
        ));
        assert_eq!(pragma(&mut connection, "application_id").await, 0);
        assert_eq!(pragma(&mut connection, "user_version").await, 0);
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%'",
            )
            .fetch_one(&mut connection)
            .await
            .expect("rolled-back catalog"),
            0
        );
    }

    #[test]
    fn migration_plan_validation_rejects_every_invalid_shape() {
        fn plan(minimum_version: u32, current_version: u32, versions: &[u32]) -> MigrationPlan {
            MigrationPlan {
                database: "test.sqlite",
                application_id: 4_242,
                set_application_id_sql: "PRAGMA application_id = 4242",
                minimum_version,
                current_version,
                steps: versions
                    .iter()
                    .copied()
                    .map(|version| MigrationStep {
                        version,
                        sql: "SELECT 1",
                        owned_objects: &[],
                    })
                    .collect(),
            }
        }

        assert!(validate_plan(&plan(1, 2, &[1, 2])).is_ok());
        for invalid in [
            plan(0, 2, &[1, 2]),
            plan(3, 2, &[1, 2]),
            plan(1, 10, &[1, 2]),
            plan(1, 2, &[1]),
            plan(1, 2, &[1, 3]),
        ] {
            assert!(matches!(
                validate_plan(&invalid),
                Err(Error::SchemaMetadataUnavailable {
                    database: "test.sqlite"
                })
            ));
        }
    }
}
