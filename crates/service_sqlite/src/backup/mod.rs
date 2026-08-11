//! Stable service backup manifest identity.

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod capture;
mod manifest;

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) use capture::capture_online_backup;
#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
pub(crate) use capture::{
    TEST_CAPTURE_PHASE_BACKUP_STEPPED, TEST_CAPTURE_PHASE_BEFORE_CREATE,
    TEST_CAPTURE_PHASE_JOIN_AWAITED, TEST_CAPTURE_PHASE_METADATA_AWAITED,
    TEST_CAPTURE_PHASE_POST_COPY, TEST_CAPTURE_PHASE_PRE_FINAL_SYNC,
    TEST_CAPTURE_PHASE_STAGING_CREATED, TestCaptureSyncFailure, test_capture_block_phase,
    test_capture_inject_metadata_failure, test_capture_online_backup_with_sync_failure,
    test_capture_panic_worker, test_capture_phase, test_capture_reset,
};

pub use manifest::{
    BACKUP_MANIFEST_CANONICAL_MAX_BYTES, BACKUP_MANIFEST_SCHEMA, BACKUP_MANIFEST_SCHEMA_VERSION,
    BACKUP_STATE_MEMBER_NAME, BackupCreatedAtUnixMs, BackupManifestContractError,
    BackupManifestIntegrity, BackupManifestSha256, BackupMemberSha256, ServiceBackupManifest,
    ServiceBackupMember,
};
