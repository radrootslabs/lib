#![deny(unsafe_code)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

//! Reusable, service-neutral SQLite mechanics for Radroots services.

mod authority;
mod backup;
mod config;
mod connection;
mod error;
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod failpoint;
mod initialize;
mod integrity;
mod metadata;
mod migration;
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod native_metadata;
mod open;
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod persisted_value;
mod restore;
#[cfg(any(target_os = "linux", target_os = "macos"))]
#[allow(
    unsafe_code,
    reason = "the sealed SQLx-handle adapter owns the missing SQLite online-backup calls"
)]
mod sqlite_native_backup;
mod statement_policy;
mod status;
mod transaction_control;

pub(crate) fn all_constraints<const N: usize>(constraints: [bool; N]) -> bool {
    constraints.into_iter().all(core::convert::identity)
}

#[cfg(any(test, target_os = "linux", target_os = "macos"))]
pub(crate) fn require_condition(
    condition: bool,
    kind: ServiceSqliteErrorKind,
) -> Result<(), ServiceSqliteError> {
    if condition {
        Ok(())
    } else {
        Err(ServiceSqliteError::new(kind))
    }
}

pub use authority::WriterAuthority;
pub use backup::{
    BACKUP_MANIFEST_CANONICAL_MAX_BYTES, BACKUP_MANIFEST_SCHEMA, BACKUP_MANIFEST_SCHEMA_VERSION,
    BACKUP_STATE_MEMBER_NAME, BackupCreatedAtUnixMs, BackupManifestContractError,
    BackupManifestIntegrity, BackupManifestSha256, BackupMemberSha256, ServiceBackupManifest,
    ServiceBackupMember, VerifiedServiceBackup, verify_backup_bundle,
};
pub use config::{ServiceSqliteConnectionOptions, ServiceSqliteConnectionOptionsError};
pub use connection::{
    OpenedExistingServiceDatabase, ServiceSqliteHost, ServiceSqliteTransaction,
    ServiceSqliteTransactionError, ServiceSqliteTransactionErrorKind,
    ServiceSqliteTransactionFuture,
};
pub use error::{
    SafeServiceSqliteError, ServiceSqliteError, ServiceSqliteErrorCode, ServiceSqliteErrorKind,
};
pub use initialize::initialize_database;
pub use integrity::{
    IntegrityCheckOutcome, IntegrityCheckedAtUnixMs, IntegrityDiagnosticCode, SchemaCatalog,
    SchemaCatalogContractError, SchemaDigest, SchemaObject, SchemaObjectKind, SchemaVersionCatalog,
    ServiceSqliteIntegrityReport,
};
pub use metadata::{
    ExistingServiceDatabaseIntent, ServiceDatabaseIdentity, ServiceDatabaseMetadata,
    ServiceSqliteApplicationId, ServiceSqliteMetadataValueError,
};
pub use migration::{
    MigrationApplicationOutcome, MigrationAppliedAtUnixSeconds, MigrationBuildIdentity,
    MigrationCallback, MigrationCallbackBinding, MigrationCallbackFuture, MigrationCatalog,
    MigrationChecksum, MigrationContractError, MigrationDescriptor, MigrationEvidenceError,
    MigrationKind, MigrationName, MigrationTransactionExecutor,
};
pub use open::{OpenMode, ServiceSqlitePathError, ServiceSqlitePaths};
pub use restore::{StagedServiceRestore, finalize_staged_restore, stage_verified_restore};
pub use status::{
    MinimumFreeBytes, PlatformStateFilesystemCapacitySource, StateFilesystemCapacity,
    StateFilesystemCapacityError, StateFilesystemCapacityReadiness, StateFilesystemCapacitySource,
    StorageHealth, StorageIntegrity, StorageStatus, inspect_state_filesystem_capacity,
};

#[cfg(test)]
mod coverage_tests {
    use super::*;

    #[test]
    fn shared_condition_classifier_preserves_every_stable_error_kind() {
        for kind in [
            ServiceSqliteErrorKind::Authority,
            ServiceSqliteErrorKind::Open,
            ServiceSqliteErrorKind::Create,
            ServiceSqliteErrorKind::Pragma,
            ServiceSqliteErrorKind::Metadata,
            ServiceSqliteErrorKind::Migration,
            ServiceSqliteErrorKind::Backup,
            ServiceSqliteErrorKind::Restore,
            ServiceSqliteErrorKind::Integrity,
            ServiceSqliteErrorKind::Recovery,
        ] {
            assert!(require_condition(true, kind).is_ok());
            assert_eq!(
                require_condition(false, kind)
                    .expect_err("false condition")
                    .kind(),
                kind
            );
        }
    }
}
