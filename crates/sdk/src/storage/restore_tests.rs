use super::*;
use radroots_storage::backup::{BackupFormatVersion, BackupSecretPolicy};
#[cfg(feature = "memory")]
use radroots_storage::backup::{BackupMember, BackupMemberKind, MemberDigest};

#[cfg(feature = "memory")]
#[tokio::test]
async fn restore_metadata_is_not_a_successful_restore_capability() {
    let client = crate::ClientBuilder::memory_default().build().unwrap();
    let operations = client.storage_operations().unwrap();
    let manifest = BackupManifest::new(
        BackupFormatVersion::V1,
        BackupId::new([6; 16]).unwrap(),
        100,
        BackupSecretPolicy::ExcludeProtectedStorage,
        vec![
            BackupMember::new(
                "runtime/events.bin",
                BackupMemberKind::Runtime,
                1,
                MemberDigest::new([8; 32]),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let restore =
        RestorePlan::new(manifest, BackupSecretPolicy::ExcludeProtectedStorage, 200).unwrap();
    operations.begin_restore(restore.clone()).await.unwrap();
    assert_eq!(
        operations.stage_restore(restore.clone()).await,
        Err(RestoreCapabilityError::Unsupported)
    );
    assert_eq!(
        operations.finalize_restore(restore).await,
        Err(RestoreCapabilityError::Unsupported)
    );
    client.close().await.unwrap();
}

#[cfg(feature = "sqlite")]
#[tokio::test]
async fn sdk_restore_retains_historical_ids_and_requires_an_explicit_reopen() {
    use radroots_storage::{
        authored_draft::{AuthoredDraft, AuthoredDraftId, AuthoredDraftStage},
        event::SourceGeneration,
        status::ShutdownState,
    };
    let root = tempfile::tempdir().unwrap();
    let backup = tempfile::tempdir().unwrap();
    let paths = SqlitePaths::from_directory(root.path()).unwrap();
    let client = crate::ClientBuilder::sqlite(
        SqliteOptions::new(paths.clone(), SqliteOpenMode::Create)
            .with_source_generation(SourceGeneration::new([9; 32]).unwrap(), 100)
            .unwrap()
            .with_backup_root(backup.path())
            .unwrap(),
    )
    .await
    .unwrap()
    .build()
    .unwrap();
    let draft = AuthoredDraft::initial(
        AuthoredDraftId::new([7; 16]).unwrap(),
        [3; 32],
        "fixture.restore.v1",
        b"historical identity".to_vec(),
        AuthoredDraftStage::Draft,
        None,
        100,
    )
    .unwrap();
    let later = AuthoredDraft::initial(
        AuthoredDraftId::new([8; 16]).unwrap(),
        [3; 32],
        "fixture.restore.v1",
        b"later live state".to_vec(),
        AuthoredDraftStage::Draft,
        None,
        200,
    )
    .unwrap();
    client
        .storage()
        .unwrap()
        .append_authored_draft(draft.clone(), None)
        .await
        .unwrap();
    let operations = client.storage_operations().unwrap();
    let plan = BackupPlan::new(
        BackupId::new([6; 16]).unwrap(),
        BackupFormatVersion::V1,
        BackupSecretPolicy::IncludeProtectedStorage,
        100,
    )
    .unwrap();
    let manifest = operations.capture_backup(plan.clone()).await.unwrap();
    operations
        .finalize_backup(plan, manifest.clone())
        .await
        .unwrap();
    client
        .storage()
        .unwrap()
        .append_authored_draft(later.clone(), None)
        .await
        .unwrap();
    let restore =
        RestorePlan::new(manifest, BackupSecretPolicy::IncludeProtectedStorage, 200).unwrap();
    drop(operations.stage_restore(restore.clone()));
    drop(operations.finalize_restore(restore.clone()));
    assert_eq!(
        operations.status().await.unwrap().shutdown(),
        ShutdownState::Open
    );
    assert_eq!(
        operations
            .stage_restore(restore.clone())
            .await
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        client
            .storage()
            .unwrap()
            .authored_draft_head(later.draft_id())
            .await
            .unwrap(),
        Some(later.clone())
    );
    operations.finalize_restore(restore.clone()).await.unwrap();
    assert_eq!(
        operations.status().await.unwrap().shutdown(),
        ShutdownState::Closed
    );
    assert_eq!(
        operations.stage_restore(restore.clone()).await,
        Err(RestoreCapabilityError::Unavailable)
    );
    assert_eq!(
        operations.finalize_restore(restore).await,
        Err(RestoreCapabilityError::Unavailable)
    );
    client.close().await.unwrap();
    let reopened =
        crate::ClientBuilder::sqlite(SqliteOptions::new(paths, SqliteOpenMode::ReadWriteExisting))
            .await
            .unwrap()
            .build()
            .unwrap();
    assert_eq!(
        reopened
            .storage()
            .unwrap()
            .authored_draft_head(draft.draft_id())
            .await
            .unwrap(),
        Some(draft)
    );
    assert_eq!(
        reopened
            .storage()
            .unwrap()
            .authored_draft_head(later.draft_id())
            .await
            .unwrap(),
        None
    );
    reopened.close().await.unwrap();
}
