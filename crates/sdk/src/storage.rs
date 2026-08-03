//! Canonical storage capability composition.

/// Backend-neutral storage status owned by `radroots_storage`.
pub type Status = radroots_storage::StorageStatus;

/// Backend-neutral integrity status owned by `radroots_storage`.
pub type IntegrityStatus = radroots_storage::status::IntegrityStatus;

/// Validated SQLite open configuration; this contains no connection or pool.
#[cfg(feature = "sqlite")]
pub type SqliteOptions = radroots_storage_sqlite::OpenOptions;

/// Explicit SQLite lifecycle mode.
#[cfg(feature = "sqlite")]
pub type SqliteOpenMode = radroots_storage_sqlite::OpenMode;

/// Validated SQLite-owned paths; this contains no backend handle.
#[cfg(feature = "sqlite")]
pub type SqlitePaths = radroots_storage_sqlite::Paths;

use radroots_storage::backup::{
    BackupId, BackupOperation, BackupPlan, BackupTransition, ReliabilityRevision, RestoreOperation,
    RestorePlan, RestoreTransition, StorageReliability,
};

/// Borrowed reliability operations over the canonical backend-neutral SPI.
#[derive(Clone, Copy)]
pub struct Operations<'a> {
    storage: &'a dyn radroots_storage::Storage,
}

impl<'a> Operations<'a> {
    pub(crate) const fn new(storage: &'a dyn radroots_storage::Storage) -> Self {
        Self { storage }
    }

    /// Begins or resumes one idempotent backup plan.
    pub async fn begin_backup(
        &self,
        plan: BackupPlan,
    ) -> Result<BackupOperation, radroots_storage::Error> {
        StorageReliability::begin_backup(self.storage, plan).await
    }

    /// Applies one optimistic backup transition.
    pub async fn transition_backup(
        &self,
        backup_id: BackupId,
        expected_revision: ReliabilityRevision,
        transition: BackupTransition,
        at_unix_ms: u64,
    ) -> Result<BackupOperation, radroots_storage::Error> {
        StorageReliability::transition_backup(
            self.storage,
            backup_id,
            expected_revision,
            transition,
            at_unix_ms,
        )
        .await
    }

    /// Begins or resumes one staged restore plan.
    pub async fn begin_restore(
        &self,
        plan: RestorePlan,
    ) -> Result<RestoreOperation, radroots_storage::Error> {
        StorageReliability::begin_restore(self.storage, plan).await
    }

    /// Applies one optimistic staged restore transition.
    pub async fn transition_restore(
        &self,
        backup_id: BackupId,
        expected_revision: ReliabilityRevision,
        transition: RestoreTransition,
        at_unix_ms: u64,
    ) -> Result<RestoreOperation, radroots_storage::Error> {
        StorageReliability::transition_restore(
            self.storage,
            backup_id,
            expected_revision,
            transition,
            at_unix_ms,
        )
        .await
    }

    /// Returns passive backend status without initiating recovery work.
    pub async fn status(&self) -> Result<Status, radroots_storage::Error> {
        StorageReliability::status(self.storage).await
    }

    /// Runs backend-owned integrity inspection.
    pub async fn integrity(&self) -> Result<IntegrityStatus, radroots_storage::Error> {
        StorageReliability::integrity(self.storage).await
    }
}

impl std::fmt::Debug for Operations<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Operations")
            .field("storage", &"<borrowed canonical storage>")
            .finish()
    }
}

#[cfg(all(test, feature = "memory"))]
mod tests {
    use std::sync::Arc;

    use radroots_storage::{
        backup::{
            BackupFormatVersion, BackupManifest, BackupMember, BackupMemberKind,
            BackupSecretPolicy, BackupStage, MemberDigest, MemberVerification, RestoreMemberStatus,
            RestoreStage,
        },
        event::SourceGeneration,
        memory::MemoryStorage,
        status::IntegrityHealth,
    };

    use crate::ClientBuilder;

    use super::*;

    fn manifest(backup_id: BackupId) -> BackupManifest {
        BackupManifest::new(
            BackupFormatVersion::V1,
            backup_id,
            1_800_000_000_100,
            BackupSecretPolicy::ExcludeProtectedStorage,
            vec![
                BackupMember::new(
                    "runtime/events.bin",
                    BackupMemberKind::Runtime,
                    16,
                    MemberDigest::new([3; 32]),
                )
                .expect("member"),
            ],
        )
        .expect("manifest")
    }

    #[tokio::test]
    async fn memory_reliability_preserves_staging_interruption_integrity_and_native_states() {
        let storage = Arc::new(MemoryStorage::new(
            SourceGeneration::new([8; 32]).expect("generation"),
        ));
        let client = ClientBuilder::new()
            .storage(storage)
            .build()
            .expect("client");
        let operations = client.storage_operations().expect("operations");
        let backup_id = BackupId::new([9; 16]).expect("backup id");
        let plan = BackupPlan::new(
            backup_id,
            BackupFormatVersion::V1,
            BackupSecretPolicy::ExcludeProtectedStorage,
            1_800_000_000_000,
        )
        .expect("plan");

        drop(operations.begin_backup(plan.clone()));
        let planned = operations.begin_backup(plan).await.expect("planned");
        assert_eq!(planned.stage(), BackupStage::Planned);
        let captured = operations
            .transition_backup(
                backup_id,
                planned.revision(),
                BackupTransition::Captured(manifest(backup_id)),
                1_800_000_000_200,
            )
            .await
            .expect("captured");
        let verified = operations
            .transition_backup(
                backup_id,
                captured.revision(),
                BackupTransition::Verified,
                1_800_000_000_300,
            )
            .await
            .expect("verified");
        let finalized = operations
            .transition_backup(
                backup_id,
                verified.revision(),
                BackupTransition::Finalize,
                1_800_000_000_400,
            )
            .await
            .expect("finalized");
        assert_eq!(finalized.stage(), BackupStage::Finalized);

        let restore_plan = RestorePlan::new(
            manifest(backup_id),
            BackupSecretPolicy::ExcludeProtectedStorage,
            1_800_000_001_000,
        )
        .expect("restore plan");
        let staging = operations
            .begin_restore(restore_plan.clone())
            .await
            .expect("staging");
        assert_eq!(staging.stage(), RestoreStage::Staging);
        let replayed = operations
            .begin_restore(restore_plan)
            .await
            .expect("resume staging");
        assert_eq!(replayed, staging);
        let verifying = operations
            .transition_restore(
                backup_id,
                staging.revision(),
                RestoreTransition::Staged,
                1_800_000_001_100,
            )
            .await
            .expect("verifying");
        let finalizing = operations
            .transition_restore(
                backup_id,
                verifying.revision(),
                RestoreTransition::Verified(vec![
                    RestoreMemberStatus::new("runtime/events.bin", MemberVerification::Verified)
                        .expect("member status"),
                ]),
                1_800_000_001_200,
            )
            .await
            .expect("finalizing");
        let restored = operations
            .transition_restore(
                backup_id,
                finalizing.revision(),
                RestoreTransition::Finalize,
                1_800_000_001_300,
            )
            .await
            .expect("restored");
        assert_eq!(restored.stage(), RestoreStage::Finalized);
        assert_eq!(
            operations.integrity().await.expect("integrity").health(),
            IntegrityHealth::Healthy
        );
        assert_eq!(
            operations
                .status()
                .await
                .expect("status")
                .integrity()
                .health(),
            IntegrityHealth::Healthy
        );
    }
}
