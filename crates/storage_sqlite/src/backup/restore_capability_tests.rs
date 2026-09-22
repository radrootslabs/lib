use super::*;
use crate::OpenOptions;
use radroots_storage::{
    authored_draft::{AuthoredDraft, AuthoredDraftId, AuthoredDraftStage, AuthoredDraftStore},
    event::SourceGeneration,
    status::ShutdownState,
};

fn draft(id: u8) -> AuthoredDraft {
    AuthoredDraft::initial(
        AuthoredDraftId::new([id; 16]).unwrap(),
        [3; 32],
        "fixture.restore.v1",
        vec![id],
        AuthoredDraftStage::Draft,
        None,
        100,
    )
    .unwrap()
}

async fn fixture(root: &Path, backup: &Path) -> (SqliteStorage, RestorePlan) {
    let store = SqliteStorage::open(
        OpenOptions::new(
            crate::Paths::from_directory(root).unwrap(),
            OpenMode::Create,
        )
        .with_source_generation(SourceGeneration::new([7; 32]).unwrap(), 100)
        .unwrap()
        .with_backup_root(backup)
        .unwrap(),
    )
    .await
    .unwrap();
    store.append_authored_draft(draft(1), None).await.unwrap();
    let plan = BackupPlan::new(
        BackupId::new([6; 16]).unwrap(),
        BackupFormatVersion::V1,
        BackupSecretPolicy::IncludeProtectedStorage,
        100,
    )
    .unwrap();
    let owner: &dyn StorageReliability = &store;
    let manifest = owner.capture_backup(plan.clone()).await.unwrap();
    owner.finalize_backup(plan, manifest.clone()).await.unwrap();
    store.append_authored_draft(draft(2), None).await.unwrap();
    let restore =
        RestorePlan::new(manifest, BackupSecretPolicy::IncludeProtectedStorage, 200).unwrap();
    (store, restore)
}

#[tokio::test]
async fn restore_spi_stages_without_live_changes_then_closes_and_reopens_exact_history() {
    let root = tempfile::tempdir().unwrap();
    let backup = tempfile::tempdir().unwrap();
    let (store, restore) = fixture(root.path(), backup.path()).await;
    let owner: &dyn StorageReliability = &store;
    let paths = crate::Paths::from_directory(root.path()).unwrap();
    let staging = RestoreStaging::new(&paths, restore.manifest()).unwrap();
    drop(owner.stage_restore(restore.clone()));
    drop(owner.finalize_restore(restore.clone()));
    assert!(!staging.runtime.exists() && !staging.private.exists());
    assert_eq!(
        store.storage_status().await.unwrap().shutdown(),
        ShutdownState::Open
    );
    let statuses = owner.stage_restore(restore.clone()).await.unwrap();
    assert_eq!(statuses.len(), 2);
    assert!(
        statuses
            .iter()
            .all(|v| v.verification() == MemberVerification::Verified)
    );
    assert_eq!(
        store
            .authored_draft_head(draft(2).draft_id())
            .await
            .unwrap(),
        Some(draft(2))
    );
    assert_eq!(
        owner.stage_restore(restore.clone()).await,
        Err(RestoreCapabilityError::Conflict)
    );
    owner.finalize_restore(restore.clone()).await.unwrap();
    assert_eq!(
        store.storage_status().await.unwrap().shutdown(),
        ShutdownState::Closed
    );
    assert_eq!(
        owner.stage_restore(restore.clone()).await,
        Err(RestoreCapabilityError::Unavailable)
    );
    assert_eq!(
        owner.finalize_restore(restore).await,
        Err(RestoreCapabilityError::Unavailable)
    );
    let reopened = SqliteStorage::open(OpenOptions::new(paths, OpenMode::ReadWriteExisting))
        .await
        .unwrap();
    assert_eq!(
        reopened
            .authored_draft_head(draft(1).draft_id())
            .await
            .unwrap(),
        Some(draft(1))
    );
    assert_eq!(
        reopened
            .authored_draft_head(draft(2).draft_id())
            .await
            .unwrap(),
        None
    );
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn restore_spi_rejects_configuration_version_and_tampering_without_live_changes() {
    let root = tempfile::tempdir().unwrap();
    let backup = tempfile::tempdir().unwrap();
    let (mut store, restore) = fixture(root.path(), backup.path()).await;
    let backup_root = store.backup_root.take();
    assert_eq!(
        StorageReliability::stage_restore(&store, restore.clone()).await,
        Err(RestoreCapabilityError::InvalidConfiguration)
    );
    store.backup_root = Some(std::sync::Arc::new(backup.path().join("absent")));
    assert_eq!(
        StorageReliability::stage_restore(&store, restore.clone()).await,
        Err(RestoreCapabilityError::InvalidConfiguration)
    );
    store.backup_root = backup_root;
    let original = restore.manifest();
    for future in [false, true] {
        let mut members = original.members().to_vec();
        if !future {
            let member = &members[0];
            members[0] = BackupMember::new(
                member.relative_path(),
                member.kind(),
                member.byte_length() + 1,
                member.sha256(),
            )
            .unwrap();
        }
        let manifest = BackupManifest::new(
            if future {
                BackupFormatVersion::new(2).unwrap()
            } else {
                BackupFormatVersion::V1
            },
            original.backup_id(),
            original.created_at_unix_ms(),
            original.secret_policy(),
            members,
        )
        .unwrap();
        let bad = RestorePlan::new(manifest, original.secret_policy(), 200).unwrap();
        assert_eq!(
            StorageReliability::stage_restore(&store, bad.clone()).await,
            Err(RestoreCapabilityError::VerificationFailed)
        );
        let finalization = StorageReliability::finalize_restore(&store, bad).await;
        if future {
            assert_eq!(
                finalization,
                Err(RestoreCapabilityError::UnsupportedVersion)
            );
        } else {
            assert_eq!(finalization, Err(RestoreCapabilityError::Failed));
        }
        assert_eq!(
            store.storage_status().await.unwrap().shutdown(),
            ShutdownState::Open
        );
    }
    assert_eq!(
        store
            .authored_draft_head(draft(2).draft_id())
            .await
            .unwrap(),
        Some(draft(2))
    );
    store.close().await.unwrap();
    let reader = SqliteStorage::open(
        OpenOptions::new(
            crate::Paths::from_directory(root.path()).unwrap(),
            OpenMode::ReadOnly,
        )
        .with_backup_root(backup.path())
        .unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(
        StorageReliability::stage_restore(&reader, restore.clone()).await,
        Err(RestoreCapabilityError::Unavailable)
    );
    assert_eq!(
        StorageReliability::finalize_restore(&reader, restore).await,
        Err(RestoreCapabilityError::Unavailable)
    );
    reader.close().await.unwrap();
}

#[tokio::test]
async fn cancelled_restore_close_can_be_drained_without_releasing_a_live_writer() {
    let root = tempfile::tempdir().unwrap();
    let backup = tempfile::tempdir().unwrap();
    let (store, restore) = fixture(root.path(), backup.path()).await;
    StorageReliability::stage_restore(&store, restore.clone())
        .await
        .unwrap();
    let held = store.pool.acquire().await.unwrap();
    let held_private = store.private_pool.acquire().await.unwrap();
    let mut finalization = Box::pin(StorageReliability::finalize_restore(&store, restore));
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            std::future::poll_fn(|context| {
                assert!(finalization.as_mut().poll(context).is_pending());
                std::task::Poll::Ready(())
            })
            .await;
            if store.storage_status().await.unwrap().shutdown() == ShutdownState::Closing {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    drop(finalization);
    let paths = crate::Paths::from_directory(root.path()).unwrap();
    assert!(matches!(
        SqliteStorage::open(OpenOptions::new(paths.clone(), OpenMode::ReadWriteExisting)).await,
        Err(Error::WriterAlreadyActive { .. })
    ));
    drop(held);
    let mut close = Box::pin(store.close());
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            std::future::poll_fn(|context| {
                assert!(close.as_mut().poll(context).is_pending());
                std::task::Poll::Ready(())
            })
            .await;
            if store.private_pool.is_closed() {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(matches!(
        SqliteStorage::open(OpenOptions::new(paths.clone(), OpenMode::ReadWriteExisting)).await,
        Err(Error::WriterAlreadyActive { .. })
    ));
    drop(held_private);
    assert_eq!(close.await.unwrap().shutdown(), ShutdownState::Closed);
    let reopened = SqliteStorage::open(OpenOptions::new(paths, OpenMode::ReadWriteExisting))
        .await
        .unwrap();
    assert_eq!(
        reopened
            .authored_draft_head(draft(2).draft_id())
            .await
            .unwrap(),
        Some(draft(2))
    );
    reopened.close().await.unwrap();
}

#[test]
fn restore_spi_errors_do_not_expose_filesystem_evidence_or_nested_causes() {
    for raw in [
        Error::RestoreFilesystem {
            operation: "/private/canary",
            source: std::io::Error::other("credential-canary"),
        },
        Error::RestoreMarkerCorrupt(PathBuf::from("/private/canary")),
        Error::RestoreRecoveryConflict(PathBuf::from("/private/canary")),
        Error::RestoreStagingFailed {
            member: "credential-canary",
        },
    ] {
        let bounded = capability::map_restore_error(raw);
        let report = format!("{bounded:?} {bounded}");
        assert!(!report.contains("canary") && !report.contains('/'));
        assert!(std::error::Error::source(&bounded).is_none());
    }
}
