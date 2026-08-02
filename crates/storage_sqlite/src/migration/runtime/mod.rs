//! Versioned schema authority for `runtime.sqlite`.
//!
//! The public descriptor surface exposes version and integrity metadata only.
//! Embedded SQL remains an implementation detail of this backend.

/// Lowest runtime schema version this package can recognize.
pub const MINIMUM_VERSION: u32 = 1;
/// Current runtime schema version created by this package.
pub const CURRENT_VERSION: u32 = 4;

#[allow(dead_code)] // Consumed by the migration executor introduced in its ordered RCL step.
const RUNTIME_V1_SQL: &str = include_str!("0001_runtime.up.sql");
#[allow(dead_code)] // Consumed by the migration executor introduced in its ordered RCL step.
const CANONICAL_EVENT_STORAGE_V2_SQL: &str = include_str!("0002_canonical_event_storage.up.sql");
#[allow(dead_code)] // Consumed by the migration executor introduced in its ordered RCL step.
const OPERATION_JOURNAL_V3_SQL: &str = include_str!("0003_operation_journal.up.sql");
#[allow(dead_code)] // Consumed by the migration executor introduced in its ordered RCL step.
const OUTBOX_DELIVERY_EVIDENCE_V4_SQL: &str = include_str!("0004_outbox_delivery_evidence.up.sql");

/// Stable, non-SQL description of one forward runtime migration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MigrationDescriptor {
    version: u32,
    name: &'static str,
    up_sha256: &'static str,
    owned_objects: &'static [&'static str],
}

impl MigrationDescriptor {
    /// Returns the positive, monotonically increasing migration version.
    pub const fn version(self) -> u32 {
        self.version
    }

    /// Returns the stable migration name.
    pub const fn name(self) -> &'static str {
        self.name
    }

    /// Returns the lowercase SHA-256 of the exact embedded migration bytes.
    pub const fn up_sha256(self) -> &'static str {
        self.up_sha256
    }

    /// Returns the complete, sorted SQLite catalog owned after this migration.
    pub const fn owned_objects(self) -> &'static [&'static str] {
        self.owned_objects
    }
}

const RUNTIME_V1_OBJECTS: &[&str] = &[
    "radroots_runtime_atomic_commits",
    "radroots_runtime_delivery_evidence",
    "radroots_runtime_delivery_evidence_item_idx",
    "radroots_runtime_event_index_checkpoints",
    "radroots_runtime_event_index_manifests",
    "radroots_runtime_event_index_shards",
    "radroots_runtime_event_provenance",
    "radroots_runtime_event_provenance_observed_idx",
    "radroots_runtime_events",
    "radroots_runtime_events_admission_idx",
    "radroots_runtime_events_delete_guard",
    "radroots_runtime_events_event_id_idx",
    "radroots_runtime_events_raw_update_guard",
    "radroots_runtime_journal_idempotency_idx",
    "radroots_runtime_journal_operations",
    "radroots_runtime_journal_recovery_idx",
    "radroots_runtime_outbox_items",
    "radroots_runtime_outbox_ready_idx",
    "radroots_runtime_outbox_targets",
    "radroots_runtime_projection_checkpoints",
    "radroots_runtime_projection_invalidations",
    "radroots_runtime_projection_rebuilds",
    "radroots_runtime_projection_rebuilds_stage_idx",
    "radroots_runtime_source_generations",
    "radroots_runtime_source_generations_delete_guard",
    "radroots_runtime_source_generations_identity_guard",
];

const RUNTIME_V2_OBJECTS: &[&str] = &[
    "radroots_runtime_atomic_commits",
    "radroots_runtime_delivery_evidence",
    "radroots_runtime_delivery_evidence_item_idx",
    "radroots_runtime_event_index_checkpoints",
    "radroots_runtime_event_index_manifests",
    "radroots_runtime_event_index_shards",
    "radroots_runtime_event_provenance",
    "radroots_runtime_event_provenance_observed_idx",
    "radroots_runtime_events",
    "radroots_runtime_events_admission_idx",
    "radroots_runtime_events_delete_guard",
    "radroots_runtime_events_event_id_idx",
    "radroots_runtime_events_raw_update_guard",
    "radroots_runtime_journal_idempotency_idx",
    "radroots_runtime_journal_operations",
    "radroots_runtime_journal_recovery_idx",
    "radroots_runtime_outbox_items",
    "radroots_runtime_outbox_ready_idx",
    "radroots_runtime_outbox_targets",
    "radroots_runtime_projection_checkpoints",
    "radroots_runtime_projection_invalidations",
    "radroots_runtime_projection_rebuilds",
    "radroots_runtime_projection_rebuilds_stage_idx",
    "radroots_runtime_source_generations",
    "radroots_runtime_source_generations_active_idx",
    "radroots_runtime_source_generations_delete_guard",
    "radroots_runtime_source_generations_identity_guard",
    "radroots_runtime_source_generations_sequence_guard",
];

const RUNTIME_V4_OBJECTS: &[&str] = &[
    "radroots_runtime_atomic_commits",
    "radroots_runtime_delivery_evidence",
    "radroots_runtime_delivery_evidence_item_idx",
    "radroots_runtime_event_index_checkpoints",
    "radroots_runtime_event_index_manifests",
    "radroots_runtime_event_index_shards",
    "radroots_runtime_event_provenance",
    "radroots_runtime_event_provenance_observed_idx",
    "radroots_runtime_events",
    "radroots_runtime_events_admission_idx",
    "radroots_runtime_events_delete_guard",
    "radroots_runtime_events_event_id_idx",
    "radroots_runtime_events_raw_update_guard",
    "radroots_runtime_journal_idempotency_idx",
    "radroots_runtime_journal_operations",
    "radroots_runtime_journal_recovery_idx",
    "radroots_runtime_outbox_items",
    "radroots_runtime_outbox_operation_idx",
    "radroots_runtime_outbox_ready_idx",
    "radroots_runtime_outbox_targets",
    "radroots_runtime_projection_checkpoints",
    "radroots_runtime_projection_invalidations",
    "radroots_runtime_projection_rebuilds",
    "radroots_runtime_projection_rebuilds_stage_idx",
    "radroots_runtime_source_generations",
    "radroots_runtime_source_generations_active_idx",
    "radroots_runtime_source_generations_delete_guard",
    "radroots_runtime_source_generations_identity_guard",
    "radroots_runtime_source_generations_sequence_guard",
];

/// Ordered, immutable runtime migration plan.
pub const MIGRATIONS: &[MigrationDescriptor] = &[
    MigrationDescriptor {
        version: 1,
        name: "runtime_authority",
        up_sha256: "3b869122dd5bd58f4a15e7a71fd1377879640b36cd496d7b3f15278ef1e128c9",
        owned_objects: RUNTIME_V1_OBJECTS,
    },
    MigrationDescriptor {
        version: 2,
        name: "canonical_event_storage",
        up_sha256: "35b036ba84eff7135665c4ae42fa8232d8bacd8115805b03011a3eb76423f4b8",
        owned_objects: RUNTIME_V2_OBJECTS,
    },
    MigrationDescriptor {
        version: 3,
        name: "operation_journal",
        up_sha256: "4caa69a316777cabfc647e6d022007c1433a5793b2a55679629e8fac4d50a0f0",
        owned_objects: RUNTIME_V2_OBJECTS,
    },
    MigrationDescriptor {
        version: 4,
        name: "outbox_delivery_evidence",
        up_sha256: "435ad7590be7cd9d5b5ee3c9eb2d53419eba3ba12b0f0fc623f1787517524842",
        owned_objects: RUNTIME_V4_OBJECTS,
    },
];

#[allow(dead_code)] // Keeps raw SQL crate-private until the migration executor is installed.
pub(crate) const fn migration_sql(version: u32) -> Option<&'static str> {
    match version {
        1 => Some(RUNTIME_V1_SQL),
        2 => Some(CANONICAL_EVENT_STORAGE_V2_SQL),
        3 => Some(OPERATION_JOURNAL_V3_SQL),
        4 => Some(OUTBOX_DELIVERY_EVIDENCE_V4_SQL),
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
        include_str!("../../../../../contracts/storage/runtime_schema_v1.toml");

    #[derive(Debug, Deserialize)]
    struct PlanSnapshot {
        schema_version: u32,
        database: String,
        minimum_version: u32,
        current_version: u32,
        migration_name: String,
        migration_sha256: String,
        forward_only: bool,
        raw_sql_public: bool,
        authorities: Vec<String>,
        source_invariants: Vec<String>,
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
        assert_eq!(MINIMUM_VERSION, 1);
        assert_eq!(CURRENT_VERSION, 4);
        assert_eq!(MIGRATIONS.len(), 4);
        let migration = MIGRATIONS[3];
        assert_eq!(snapshot.schema_version, 1);
        assert_eq!(snapshot.database, "runtime.sqlite");
        assert_eq!(snapshot.minimum_version, MINIMUM_VERSION);
        assert_eq!(snapshot.current_version, CURRENT_VERSION);
        assert_eq!(snapshot.migration_name, migration.name());
        assert_eq!(snapshot.migration_sha256, migration.up_sha256());
        assert!(snapshot.forward_only);
        assert!(!snapshot.raw_sql_public);
        assert_eq!(snapshot.authorities.len(), 8);
        assert_eq!(snapshot.source_invariants.len(), 5);
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
        assert_eq!(migration_sql(5), None);
    }

    #[tokio::test]
    async fn fresh_database_has_exact_owned_schema() {
        let mut connection = SqliteConnection::connect("sqlite::memory:")
            .await
            .expect("open memory SQLite");
        for migration in MIGRATIONS {
            sqlx::raw_sql(migration_sql(migration.version()).expect("registered SQL"))
                .execute(&mut connection)
                .await
                .expect("apply runtime schema");
        }

        let rows = sqlx::query(
            "SELECT name FROM sqlite_schema \
             WHERE name LIKE 'radroots_runtime_%' \
             ORDER BY name",
        )
        .fetch_all(&mut connection)
        .await
        .expect("inspect runtime schema");
        let actual = rows
            .iter()
            .map(|row| row.get::<String, _>("name"))
            .collect::<Vec<_>>();
        assert_eq!(
            actual,
            MIGRATIONS
                .last()
                .expect("current migration")
                .owned_objects()
        );

        let foreign_key_violations = sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&mut connection)
            .await
            .expect("inspect foreign keys");
        assert!(foreign_key_violations.is_empty());
        let integrity = sqlx::query_scalar::<_, String>("PRAGMA integrity_check")
            .fetch_one(&mut connection)
            .await
            .expect("inspect integrity");
        assert_eq!(integrity, "ok");
    }
}
