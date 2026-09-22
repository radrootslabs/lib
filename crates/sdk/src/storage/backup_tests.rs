use super::*;
use radroots_storage::backup::{BackupFormatVersion, BackupSecretPolicy};

fn plan() -> BackupPlan {
    BackupPlan::new(
        BackupId::new([6; 16]).unwrap(),
        BackupFormatVersion::V1,
        BackupSecretPolicy::IncludeProtectedStorage,
        100,
    )
    .unwrap()
}

#[cfg(feature = "memory")]
#[tokio::test]
async fn reliability_metadata_never_substitutes_for_actual_backup_capability() {
    use radroots_storage::backup::{BackupMember, BackupMemberKind, MemberDigest};
    let client = crate::ClientBuilder::memory_default().build().unwrap();
    let operations = client.storage_operations().unwrap();
    assert_eq!(
        operations.settle_backup_writes().await,
        Err(BackupCapabilityError::Unsupported)
    );
    let plan = plan();
    operations.begin_backup(plan.clone()).await.unwrap();
    let manifest = BackupManifest::new(
        plan.format_version(),
        plan.backup_id(),
        plan.requested_at_unix_ms(),
        plan.secret_policy(),
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
    assert_eq!(
        operations.capture_backup(plan.clone()).await,
        Err(BackupCapabilityError::Unsupported)
    );
    assert_eq!(
        operations
            .verify_backup(plan.clone(), manifest.clone())
            .await,
        Err(BackupCapabilityError::Unsupported)
    );
    assert_eq!(
        operations.finalize_backup(plan, manifest).await,
        Err(BackupCapabilityError::Unsupported)
    );
    client.close().await.unwrap();
}

#[cfg(feature = "sqlite")]
#[tokio::test]
async fn sdk_delegates_real_backup_and_close_to_one_canonical_owner() {
    use radroots_storage::{
        authored_draft::{AuthoredDraft, AuthoredDraftId, AuthoredDraftStage},
        event::SourceGeneration,
    };
    let root = tempfile::tempdir().unwrap();
    let backup = tempfile::tempdir().unwrap();
    let options = SqliteOptions::new(
        SqlitePaths::from_directory(root.path()).unwrap(),
        SqliteOpenMode::Create,
    )
    .with_source_generation(SourceGeneration::new([9; 32]).unwrap(), 100)
    .unwrap()
    .with_backup_root(backup.path())
    .unwrap();
    let client = crate::ClientBuilder::sqlite(options)
        .await
        .unwrap()
        .build()
        .unwrap();
    let draft = AuthoredDraft::initial(
        AuthoredDraftId::new([7; 16]).unwrap(),
        [3; 32],
        "fixture.backup.v1",
        b"retained authored work".to_vec(),
        AuthoredDraftStage::Draft,
        None,
        100,
    )
    .unwrap();
    client
        .storage()
        .unwrap()
        .append_authored_draft(draft.clone(), None)
        .await
        .unwrap();
    let operations = client.storage_operations().unwrap();
    let plan = plan();
    drop(operations.capture_backup(plan.clone()));
    operations.settle_backup_writes().await.unwrap();
    assert_eq!(std::fs::read_dir(backup.path()).unwrap().count(), 0);
    let manifest = operations.capture_backup(plan.clone()).await.unwrap();
    assert_eq!(manifest.backup_id(), plan.backup_id());
    assert_eq!(manifest.members().len(), 2);
    assert!(
        manifest
            .members()
            .iter()
            .all(|member| member.byte_length() > 0)
    );
    operations
        .verify_backup(plan.clone(), manifest.clone())
        .await
        .unwrap();
    for _ in 0..2 {
        operations
            .finalize_backup(plan.clone(), manifest.clone())
            .await
            .unwrap();
    }
    assert_eq!(std::fs::read_dir(backup.path()).unwrap().count(), 1);
    assert_eq!(
        client
            .storage()
            .unwrap()
            .authored_draft_head(draft.draft_id())
            .await
            .unwrap(),
        Some(draft)
    );
    client.close().await.unwrap();
    assert_eq!(
        operations.capture_backup(plan.clone()).await,
        Err(BackupCapabilityError::Unavailable)
    );
    assert_eq!(
        operations
            .verify_backup(plan.clone(), manifest.clone())
            .await,
        Err(BackupCapabilityError::Unavailable)
    );
    assert_eq!(
        operations.finalize_backup(plan, manifest).await,
        Err(BackupCapabilityError::Unavailable)
    );
}
