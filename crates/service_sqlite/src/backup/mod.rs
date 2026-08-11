//! Stable service backup manifest identity.

mod manifest;

pub use manifest::{
    BACKUP_MANIFEST_CANONICAL_MAX_BYTES, BACKUP_MANIFEST_SCHEMA, BACKUP_MANIFEST_SCHEMA_VERSION,
    BACKUP_STATE_MEMBER_NAME, BackupCreatedAtUnixMs, BackupManifestContractError,
    BackupManifestIntegrity, BackupManifestSha256, BackupMemberSha256, ServiceBackupManifest,
    ServiceBackupMember,
};
