//! Versioned schema authority for `runtime.sqlite`.
//!
//! The public descriptor surface exposes version and integrity metadata only.
//! Embedded SQL remains an implementation detail of this backend.

/// Lowest runtime schema version this package can recognize.
pub const MINIMUM_VERSION: u32 = 1;
/// Current runtime schema version created by this package.
pub const CURRENT_VERSION: u32 = 11;

const RUNTIME_V1_SQL: &str = include_str!("0001_runtime.up.sql");
const CANONICAL_EVENT_STORAGE_V2_SQL: &str = include_str!("0002_canonical_event_storage.up.sql");
const OPERATION_JOURNAL_V3_SQL: &str = include_str!("0003_operation_journal.up.sql");
const OUTBOX_DELIVERY_EVIDENCE_V4_SQL: &str = include_str!("0004_outbox_delivery_evidence.up.sql");
const PROJECTION_METADATA_V5_SQL: &str = include_str!("0005_projection_metadata.up.sql");
const LEGACY_IMPORT_JOURNAL_V6_SQL: &str = include_str!("0006_legacy_import_journal.up.sql");
const LEGACY_EVENT_STAGING_V7_SQL: &str = include_str!("0007_legacy_event_staging.up.sql");
const LEGACY_OUTBOX_STAGING_V8_SQL: &str = include_str!("0008_legacy_outbox_staging.up.sql");
const LEGACY_IMPORT_COMMITS_V9_SQL: &str = include_str!("0009_legacy_import_commits.up.sql");
const PROJECTION_REBUILD_SOURCE_BINDING_V10_SQL: &str =
    include_str!("0010_projection_rebuild_source_binding.up.sql");
const AUTHORED_OPERATIONS_V11_SQL: &str = include_str!("0011_authored_operations.up.sql");

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

const RUNTIME_V6_OBJECTS: &[&str] = &[
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
    "radroots_runtime_legacy_import_delete_guard",
    "radroots_runtime_legacy_import_identity_guard",
    "radroots_runtime_legacy_import_member_delete_guard",
    "radroots_runtime_legacy_import_member_identity_guard",
    "radroots_runtime_legacy_import_member_state_guard",
    "radroots_runtime_legacy_import_members",
    "radroots_runtime_legacy_import_state_guard",
    "radroots_runtime_legacy_import_state_idx",
    "radroots_runtime_legacy_imports",
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

const RUNTIME_V7_OBJECTS: &[&str] = &[
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
    "radroots_runtime_legacy_event_staging",
    "radroots_runtime_legacy_event_staging_delete_guard",
    "radroots_runtime_legacy_event_staging_insert_guard",
    "radroots_runtime_legacy_event_staging_update_guard",
    "radroots_runtime_legacy_import_delete_guard",
    "radroots_runtime_legacy_import_identity_guard",
    "radroots_runtime_legacy_import_member_delete_guard",
    "radroots_runtime_legacy_import_member_identity_guard",
    "radroots_runtime_legacy_import_member_state_guard",
    "radroots_runtime_legacy_import_members",
    "radroots_runtime_legacy_import_state_guard",
    "radroots_runtime_legacy_import_state_idx",
    "radroots_runtime_legacy_imports",
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

const RUNTIME_V8_OBJECTS: &[&str] = &[
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
    "radroots_runtime_legacy_event_staging",
    "radroots_runtime_legacy_event_staging_delete_guard",
    "radroots_runtime_legacy_event_staging_insert_guard",
    "radroots_runtime_legacy_event_staging_update_guard",
    "radroots_runtime_legacy_import_delete_guard",
    "radroots_runtime_legacy_import_identity_guard",
    "radroots_runtime_legacy_import_member_delete_guard",
    "radroots_runtime_legacy_import_member_identity_guard",
    "radroots_runtime_legacy_import_member_state_guard",
    "radroots_runtime_legacy_import_members",
    "radroots_runtime_legacy_import_state_guard",
    "radroots_runtime_legacy_import_state_idx",
    "radroots_runtime_legacy_imports",
    "radroots_runtime_legacy_outbox_staging",
    "radroots_runtime_legacy_outbox_staging_delete_guard",
    "radroots_runtime_legacy_outbox_staging_insert_guard",
    "radroots_runtime_legacy_outbox_staging_parent_idx",
    "radroots_runtime_legacy_outbox_staging_update_guard",
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

const RUNTIME_V9_OBJECTS: &[&str] = &[
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
    "radroots_runtime_legacy_event_staging",
    "radroots_runtime_legacy_event_staging_delete_guard",
    "radroots_runtime_legacy_event_staging_insert_guard",
    "radroots_runtime_legacy_event_staging_update_guard",
    "radroots_runtime_legacy_import_commit_delete_guard",
    "radroots_runtime_legacy_import_commit_update_guard",
    "radroots_runtime_legacy_import_commits",
    "radroots_runtime_legacy_import_delete_guard",
    "radroots_runtime_legacy_import_identity_guard",
    "radroots_runtime_legacy_import_member_delete_guard",
    "radroots_runtime_legacy_import_member_identity_guard",
    "radroots_runtime_legacy_import_member_state_guard",
    "radroots_runtime_legacy_import_members",
    "radroots_runtime_legacy_import_state_guard",
    "radroots_runtime_legacy_import_state_idx",
    "radroots_runtime_legacy_imports",
    "radroots_runtime_legacy_outbox_staging",
    "radroots_runtime_legacy_outbox_staging_delete_guard",
    "radroots_runtime_legacy_outbox_staging_insert_guard",
    "radroots_runtime_legacy_outbox_staging_parent_idx",
    "radroots_runtime_legacy_outbox_staging_update_guard",
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

const RUNTIME_V11_OBJECTS: &[&str] = &[
    "radroots_runtime_atomic_commits",
    "radroots_runtime_authored_artifacts",
    "radroots_runtime_authored_artifacts_admission_ready_idx",
    "radroots_runtime_authored_artifacts_signing_ready_idx",
    "radroots_runtime_authored_atomic_commits",
    "radroots_runtime_authored_atomic_commits_delete_guard",
    "radroots_runtime_authored_atomic_commits_update_guard",
    "radroots_runtime_authored_delivery_attempts",
    "radroots_runtime_authored_delivery_plans",
    "radroots_runtime_authored_delivery_ready_idx",
    "radroots_runtime_authored_delivery_targets",
    "radroots_runtime_authored_operations",
    "radroots_runtime_delivery_evidence",
    "radroots_runtime_delivery_evidence_item_idx",
    "radroots_runtime_event_index_checkpoints",
    "radroots_runtime_event_index_manifests",
    "radroots_runtime_event_index_shards",
    "radroots_runtime_event_provenance",
    "radroots_runtime_event_provenance_observed_idx",
    "radroots_runtime_events",
    "radroots_runtime_events_admission_idx",
    "radroots_runtime_events_contract_metadata_guard",
    "radroots_runtime_events_contract_metadata_insert_guard",
    "radroots_runtime_events_delete_guard",
    "radroots_runtime_events_event_id_idx",
    "radroots_runtime_events_raw_update_guard",
    "radroots_runtime_journal_idempotency_idx",
    "radroots_runtime_journal_operations",
    "radroots_runtime_journal_recovery_idx",
    "radroots_runtime_legacy_event_staging",
    "radroots_runtime_legacy_event_staging_delete_guard",
    "radroots_runtime_legacy_event_staging_insert_guard",
    "radroots_runtime_legacy_event_staging_update_guard",
    "radroots_runtime_legacy_import_commit_delete_guard",
    "radroots_runtime_legacy_import_commit_update_guard",
    "radroots_runtime_legacy_import_commits",
    "radroots_runtime_legacy_import_delete_guard",
    "radroots_runtime_legacy_import_identity_guard",
    "radroots_runtime_legacy_import_member_delete_guard",
    "radroots_runtime_legacy_import_member_identity_guard",
    "radroots_runtime_legacy_import_member_state_guard",
    "radroots_runtime_legacy_import_members",
    "radroots_runtime_legacy_import_state_guard",
    "radroots_runtime_legacy_import_state_idx",
    "radroots_runtime_legacy_imports",
    "radroots_runtime_legacy_outbox_staging",
    "radroots_runtime_legacy_outbox_staging_delete_guard",
    "radroots_runtime_legacy_outbox_staging_insert_guard",
    "radroots_runtime_legacy_outbox_staging_parent_idx",
    "radroots_runtime_legacy_outbox_staging_update_guard",
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
    MigrationDescriptor {
        version: 5,
        name: "projection_metadata",
        up_sha256: "ef162e3591057414b5704ff07fc93e57127ad7996ab3d431541418fd9c285e51",
        owned_objects: RUNTIME_V4_OBJECTS,
    },
    MigrationDescriptor {
        version: 6,
        name: "legacy_import_journal",
        up_sha256: "5858336c25b048d6cdae1beedc8bf0bdfb530b858c3c50f501621c3314d23d4a",
        owned_objects: RUNTIME_V6_OBJECTS,
    },
    MigrationDescriptor {
        version: 7,
        name: "legacy_event_staging",
        up_sha256: "cb10766ab1a98fa13e27f36fdab869c72efefda98ea2331da32e90b3528c63aa",
        owned_objects: RUNTIME_V7_OBJECTS,
    },
    MigrationDescriptor {
        version: 8,
        name: "legacy_outbox_staging",
        up_sha256: "b2ad0ee5bf7ac9e56584623c641e5208f5446dd0ff5ced9f235cd1756781be34",
        owned_objects: RUNTIME_V8_OBJECTS,
    },
    MigrationDescriptor {
        version: 9,
        name: "legacy_import_commits",
        up_sha256: "f0807eecd652a26844c3502d81386a9d54480cb178abe1b71035e0601916afb7",
        owned_objects: RUNTIME_V9_OBJECTS,
    },
    MigrationDescriptor {
        version: 10,
        name: "projection_rebuild_source_binding",
        up_sha256: "8dfe0f83058f51e3edf9bdac16b408c6abdc88dd84a53f8e893aaf06fe89f7c7",
        owned_objects: RUNTIME_V9_OBJECTS,
    },
    MigrationDescriptor {
        version: 11,
        name: "authored_operations",
        up_sha256: "fa461e3977594a364850d8639539ca902a93b715f8ce6629182dbc536266499f",
        owned_objects: RUNTIME_V11_OBJECTS,
    },
];

pub(crate) const fn migration_sql(version: u32) -> Option<&'static str> {
    match version {
        1 => Some(RUNTIME_V1_SQL),
        2 => Some(CANONICAL_EVENT_STORAGE_V2_SQL),
        3 => Some(OPERATION_JOURNAL_V3_SQL),
        4 => Some(OUTBOX_DELIVERY_EVIDENCE_V4_SQL),
        5 => Some(PROJECTION_METADATA_V5_SQL),
        6 => Some(LEGACY_IMPORT_JOURNAL_V6_SQL),
        7 => Some(LEGACY_EVENT_STAGING_V7_SQL),
        8 => Some(LEGACY_OUTBOX_STAGING_V8_SQL),
        9 => Some(LEGACY_IMPORT_COMMITS_V9_SQL),
        10 => Some(PROJECTION_REBUILD_SOURCE_BINDING_V10_SQL),
        11 => Some(AUTHORED_OPERATIONS_V11_SQL),
        _ => None,
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
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
        application_id: u32,
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
        assert_eq!(CURRENT_VERSION, 11);
        assert_eq!(MIGRATIONS.len(), 11);
        let migration = MIGRATIONS[8];
        assert_eq!(snapshot.schema_version, 1);
        assert_eq!(snapshot.database, "runtime.sqlite");
        assert_eq!(snapshot.application_id, 1_380_209_236);
        assert_eq!(snapshot.minimum_version, MINIMUM_VERSION);
        assert_eq!(snapshot.current_version, 9);
        assert_eq!(snapshot.migration_name, migration.name());
        assert_eq!(snapshot.migration_sha256, migration.up_sha256());
        assert!(snapshot.forward_only);
        assert!(!snapshot.raw_sql_public);
        assert_eq!(snapshot.authorities.len(), 12);
        assert_eq!(snapshot.source_invariants.len(), 5);
        assert_eq!(snapshot.migrations.len(), 9);
        for (expected, actual) in snapshot.migrations.iter().zip(&MIGRATIONS[..9]) {
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
        assert_eq!(migration_sql(12), None);
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
