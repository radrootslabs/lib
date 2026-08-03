//! Versioned schema authority for `private.sqlite`.
//!
//! The public descriptor surface exposes version and integrity metadata only.
//! Embedded SQL remains an implementation detail of this backend.

/// Lowest private schema version this package can recognize.
pub const MINIMUM_VERSION: u32 = 1;
/// Current private schema version created by this package.
pub const CURRENT_VERSION: u32 = 3;

const PRIVATE_V1_SQL: &str = include_str!("0001_private.up.sql");
const LEGACY_PRIVATE_STAGING_V2_SQL: &str = include_str!("0002_legacy_private_staging.up.sql");
const LEGACY_IMPORT_COMMITS_V3_SQL: &str = include_str!("0003_legacy_import_commits.up.sql");

/// Stable, non-SQL description of one forward private migration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MigrationDescriptor {
    version: u32,
    name: &'static str,
    up_sha256: &'static str,
    owned_objects: &'static [&'static str],
}

impl MigrationDescriptor {
    pub const fn version(self) -> u32 {
        self.version
    }

    pub const fn name(self) -> &'static str {
        self.name
    }

    pub const fn up_sha256(self) -> &'static str {
        self.up_sha256
    }

    pub const fn owned_objects(self) -> &'static [&'static str] {
        self.owned_objects
    }
}

const PRIVATE_V1_OBJECTS: &[&str] = &[
    "radroots_private_artifacts",
    "radroots_private_artifacts_delete_guard",
    "radroots_private_artifacts_envelope_guard",
    "radroots_private_artifacts_expiry_idx",
    "radroots_private_artifacts_identity_guard",
    "radroots_private_artifacts_key_version_idx",
    "radroots_private_artifacts_kind_idx",
];

const PRIVATE_V2_OBJECTS: &[&str] = &[
    "radroots_private_artifacts",
    "radroots_private_artifacts_delete_guard",
    "radroots_private_artifacts_envelope_guard",
    "radroots_private_artifacts_expiry_idx",
    "radroots_private_artifacts_identity_guard",
    "radroots_private_artifacts_key_version_idx",
    "radroots_private_artifacts_kind_idx",
    "radroots_private_legacy_import_staging",
    "radroots_private_legacy_import_staging_delete_guard",
    "radroots_private_legacy_import_staging_insert_guard",
    "radroots_private_legacy_import_staging_parent_idx",
    "radroots_private_legacy_import_staging_update_guard",
];

const PRIVATE_V3_OBJECTS: &[&str] = &[
    "radroots_private_artifacts",
    "radroots_private_artifacts_delete_guard",
    "radroots_private_artifacts_envelope_guard",
    "radroots_private_artifacts_expiry_idx",
    "radroots_private_artifacts_identity_guard",
    "radroots_private_artifacts_key_version_idx",
    "radroots_private_artifacts_kind_idx",
    "radroots_private_legacy_import_commit_delete_guard",
    "radroots_private_legacy_import_commit_update_guard",
    "radroots_private_legacy_import_commits",
    "radroots_private_legacy_import_staging",
    "radroots_private_legacy_import_staging_delete_guard",
    "radroots_private_legacy_import_staging_insert_guard",
    "radroots_private_legacy_import_staging_parent_idx",
    "radroots_private_legacy_import_staging_update_guard",
];

/// Ordered, immutable private migration plan.
pub const MIGRATIONS: &[MigrationDescriptor] = &[
    MigrationDescriptor {
        version: 1,
        name: "private_artifacts",
        up_sha256: "07050386292ff8ce9ec0e756c9ac88e458a249d53e8654e1102caa2e361f11ab",
        owned_objects: PRIVATE_V1_OBJECTS,
    },
    MigrationDescriptor {
        version: 2,
        name: "legacy_private_staging",
        up_sha256: "299ec0c476b2f5ab995f245d36603969af345827c9ecdf9490cfe0b0dbe4b9f9",
        owned_objects: PRIVATE_V2_OBJECTS,
    },
    MigrationDescriptor {
        version: 3,
        name: "legacy_import_commits",
        up_sha256: "9377f0af8f070d977a5237e2a1294e6977f5b704e7a8434d97a3dc5f4ae75e86",
        owned_objects: PRIVATE_V3_OBJECTS,
    },
];

pub(crate) const fn migration_sql(version: u32) -> Option<&'static str> {
    match version {
        1 => Some(PRIVATE_V1_SQL),
        2 => Some(LEGACY_PRIVATE_STAGING_V2_SQL),
        3 => Some(LEGACY_IMPORT_COMMITS_V3_SQL),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{CURRENT_VERSION, MIGRATIONS, MINIMUM_VERSION, migration_sql};
    use serde::Deserialize;
    use sha2::{Digest, Sha256};
    use sqlx::{Connection, Row, SqliteConnection};

    const PLAN_SNAPSHOT: &str =
        include_str!("../../../../../contracts/storage/private_schema_v1.toml");

    #[derive(Debug, Deserialize)]
    struct PlanSnapshot {
        schema_version: u32,
        database: String,
        application_id: u32,
        minimum_version: u32,
        current_version: u32,
        migration_name: String,
        migration_sha256: String,
        forward_only: bool,
        raw_sql_public: bool,
        encrypted_envelopes: bool,
        authorities: Vec<String>,
        forbidden_tables: Vec<String>,
        migrations: Vec<MigrationSnapshot>,
    }

    #[derive(Debug, Deserialize)]
    struct MigrationSnapshot {
        version: u32,
        name: String,
        sha256: String,
        owned_objects: Vec<String>,
    }

    #[test]
    fn migration_plan_matches_governed_snapshot() {
        let snapshot = toml::from_str::<PlanSnapshot>(PLAN_SNAPSHOT).expect("valid snapshot");
        let migration = MIGRATIONS[2];
        assert_eq!(snapshot.schema_version, 1);
        assert_eq!(snapshot.database, "private.sqlite");
        assert_eq!(snapshot.application_id, 1_380_208_722);
        assert_eq!(snapshot.minimum_version, MINIMUM_VERSION);
        assert_eq!(snapshot.current_version, CURRENT_VERSION);
        assert_eq!(snapshot.migration_name, migration.name());
        assert_eq!(snapshot.migration_sha256, migration.up_sha256());
        assert!(snapshot.forward_only);
        assert!(!snapshot.raw_sql_public);
        assert!(snapshot.encrypted_envelopes);
        assert_eq!(snapshot.authorities.len(), 6);
        assert_eq!(snapshot.forbidden_tables, ["studio", "ui_state"]);
        assert_eq!(snapshot.migrations.len(), MIGRATIONS.len());
        for (expected, actual) in snapshot.migrations.iter().zip(MIGRATIONS) {
            assert_eq!(expected.version, actual.version());
            assert_eq!(expected.name, actual.name());
            assert_eq!(expected.sha256, actual.up_sha256());
            assert_eq!(expected.owned_objects, actual.owned_objects());
        }
    }

    #[test]
    fn embedded_migration_checksum_is_pinned() {
        for migration in MIGRATIONS {
            let sql = migration_sql(migration.version()).expect("registered SQL");
            assert_eq!(format!("{:x}", Sha256::digest(sql)), migration.up_sha256());
        }
        assert_eq!(migration_sql(4), None);
    }

    #[tokio::test]
    async fn fresh_database_has_exact_private_schema_and_no_studio_authority() {
        let mut connection = SqliteConnection::connect("sqlite::memory:")
            .await
            .expect("open memory SQLite");
        for migration in MIGRATIONS {
            sqlx::raw_sql(migration_sql(migration.version()).expect("registered SQL"))
                .execute(&mut connection)
                .await
                .expect("apply private schema");
        }
        let rows = sqlx::query(
            "SELECT name FROM sqlite_schema
             WHERE name LIKE 'radroots_private_%'
             ORDER BY name",
        )
        .fetch_all(&mut connection)
        .await
        .expect("inspect private schema");
        let actual = rows
            .iter()
            .map(|row| row.get::<String, _>("name"))
            .collect::<Vec<_>>();
        assert_eq!(actual, MIGRATIONS[2].owned_objects());
        let forbidden = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM sqlite_schema
             WHERE lower(name) LIKE '%studio%' OR lower(name) LIKE '%ui_state%'",
        )
        .fetch_one(&mut connection)
        .await
        .expect("inspect forbidden tables");
        assert_eq!(forbidden, 0);
        assert!(
            sqlx::query("DELETE FROM radroots_private_artifacts")
                .execute(&mut connection)
                .await
                .is_ok()
        );
        let integrity = sqlx::query_scalar::<_, String>("PRAGMA integrity_check")
            .fetch_one(&mut connection)
            .await
            .expect("inspect integrity");
        assert_eq!(integrity, "ok");
    }
}
