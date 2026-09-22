use super::*;
use crate::OpenOptions;
use radroots_storage::authored_draft::{
    AuthoredDraft, AuthoredDraftId, AuthoredDraftStage, AuthoredDraftStore,
};
use radroots_storage::event::SourceGeneration;

fn plan(id: u8) -> BackupPlan {
    BackupPlan::new(
        BackupId::new([id; 16]).unwrap(),
        BackupFormatVersion::V1,
        BackupSecretPolicy::IncludeProtectedStorage,
        100,
    )
    .unwrap()
}

async fn fixture(root: &Path, backup: &Path) -> SqliteStorage {
    SqliteStorage::open(
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
    .unwrap()
}

#[tokio::test]
async fn canonical_capability_captures_wal_and_finalizes_exact_members() {
    let root = tempfile::tempdir().unwrap();
    let backup = tempfile::tempdir().unwrap();
    let store = fixture(root.path(), backup.path()).await;
    let draft = AuthoredDraft::initial(
        AuthoredDraftId::new([8; 16]).unwrap(),
        [3; 32],
        "fixture.backup.v1",
        b"latest committed WAL work".to_vec(),
        AuthoredDraftStage::Draft,
        None,
        7777,
    )
    .unwrap();
    store
        .append_authored_draft(draft.clone(), None)
        .await
        .unwrap();
    assert!(
        fs::metadata(root.path().join("runtime.sqlite-wal"))
            .unwrap()
            .len()
            > 0
    );
    let plan = plan(1);
    let owner: &dyn StorageReliability = &store;
    drop(owner.capture_backup(plan.clone()));
    assert_eq!(fs::read_dir(backup.path()).unwrap().count(), 0);
    let manifest = owner.capture_backup(plan.clone()).await.unwrap();
    assert_eq!(manifest.members().len(), 2);
    let layout = BackupLayout::new(backup.path(), &plan);
    let mut connection = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(&layout.runtime_file)
            .read_only(true),
    )
    .await
    .unwrap();
    let value: Vec<u8> =
        sqlx::query_scalar("SELECT snapshot FROM radroots_runtime_authored_draft_revisions")
            .fetch_one(&mut connection)
            .await
            .unwrap();
    assert_eq!(
        serde_json::from_slice::<AuthoredDraft>(&value).unwrap(),
        draft
    );
    connection.close().await.unwrap();
    owner
        .verify_backup(plan.clone(), manifest.clone())
        .await
        .unwrap();
    for _ in 0..2 {
        owner
            .finalize_backup(plan.clone(), manifest.clone())
            .await
            .unwrap();
    }
    assert!(!layout.staging.exists());
    assert!(layout.finalized.is_dir());
    assert_eq!(
        owner.capture_backup(plan.clone()).await,
        Err(BackupCapabilityError::Conflict)
    );
    store.close().await.unwrap();
    assert_eq!(
        owner.capture_backup(plan.clone()).await,
        Err(BackupCapabilityError::Unavailable)
    );
    assert_eq!(
        owner.verify_backup(plan.clone(), manifest.clone()).await,
        Err(BackupCapabilityError::Unavailable)
    );
    assert_eq!(
        owner.finalize_backup(plan, manifest).await,
        Err(BackupCapabilityError::Unavailable)
    );
}

#[tokio::test]
async fn canonical_capability_refuses_tampered_inventory_and_retains_staging() {
    let root = tempfile::tempdir().unwrap();
    let backup = tempfile::tempdir().unwrap();
    let store = fixture(root.path(), backup.path()).await;
    let owner: &dyn StorageReliability = &store;
    let plan = plan(2);
    let manifest = owner.capture_backup(plan.clone()).await.unwrap();
    let layout = BackupLayout::new(backup.path(), &plan);
    let bytes = fs::read(&layout.runtime_file).unwrap();
    for bad_size in [false, true] {
        let mut members = manifest.members().to_vec();
        let original = &members[0];
        members[0] = BackupMember::new(
            original.relative_path(),
            original.kind(),
            original.byte_length() + u64::from(bad_size),
            if bad_size {
                original.sha256()
            } else {
                MemberDigest::new([0; 32])
            },
        )
        .unwrap();
        let bad = BackupManifest::new(
            manifest.format_version(),
            manifest.backup_id(),
            manifest.created_at_unix_ms(),
            manifest.secret_policy(),
            members,
        )
        .unwrap();
        assert_eq!(
            owner.verify_backup(plan.clone(), bad.clone()).await,
            Err(BackupCapabilityError::VerificationFailed)
        );
        assert_eq!(
            owner.finalize_backup(plan.clone(), bad).await,
            Err(BackupCapabilityError::VerificationFailed)
        );
    }
    let wrong_plan = BackupPlan::new(
        plan.backup_id(),
        plan.format_version(),
        plan.secret_policy(),
        101,
    )
    .unwrap();
    assert_eq!(
        owner.verify_backup(wrong_plan, manifest.clone()).await,
        Err(BackupCapabilityError::VerificationFailed)
    );
    fs::write(layout.staging.join("unexpected"), b"retained").unwrap();
    assert_eq!(
        owner.verify_backup(plan.clone(), manifest.clone()).await,
        Err(BackupCapabilityError::VerificationFailed)
    );
    assert_eq!(fs::read(&layout.runtime_file).unwrap(), bytes);
    assert!(!layout.finalized.exists());
    assert_eq!(
        owner.capture_backup(plan.clone()).await,
        Err(BackupCapabilityError::Conflict)
    );
    let missing = self::plan(3);
    assert_eq!(
        owner.verify_backup(missing, manifest).await,
        Err(BackupCapabilityError::VerificationFailed)
    );
    store.close().await.unwrap();
}

#[tokio::test]
async fn canonical_capability_classifies_configuration_version_and_io_without_paths() {
    let root = tempfile::tempdir().unwrap();
    let backup = tempfile::tempdir().unwrap();
    let mut store = fixture(root.path(), backup.path()).await;
    let future = BackupPlan::new(
        BackupId::new([4; 16]).unwrap(),
        BackupFormatVersion::new(2).unwrap(),
        BackupSecretPolicy::IncludeProtectedStorage,
        100,
    )
    .unwrap();
    assert_eq!(
        StorageReliability::capture_backup(&store, future).await,
        Err(BackupCapabilityError::UnsupportedVersion)
    );
    store.backup_root = None;
    assert_eq!(
        StorageReliability::capture_backup(&store, plan(4)).await,
        Err(BackupCapabilityError::InvalidConfiguration)
    );
    store.backup_root = Some(std::sync::Arc::new(backup.path().join("absent")));
    assert_eq!(
        StorageReliability::capture_backup(&store, plan(4)).await,
        Err(BackupCapabilityError::InvalidConfiguration)
    );
    // The public SPI must never forward backend paths or nested I/O details.
    let error = capability::map_error(Error::BackupFilesystem {
        operation: "capture /private/canary",
        source: std::io::Error::other("credential-canary"),
    });
    assert_eq!(error, BackupCapabilityError::Failed);
    assert!(!format!("{error:?} {error}").contains("canary"));
    store.close().await.unwrap();
}
