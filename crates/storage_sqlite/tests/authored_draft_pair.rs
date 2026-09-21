use radroots_storage::{
    Error,
    authored_draft::{
        AuthoredDraft, AuthoredDraftId, AuthoredDraftStage, AuthoredDraftStore,
        DraftAppendDisposition,
    },
    authored_draft_pair::AuthoredDraftPair,
};
fn draft(id: u8, payload: &[u8]) -> AuthoredDraft {
    AuthoredDraft::initial(
        AuthoredDraftId::new([id; 16]).unwrap(),
        [9; 32],
        "fixture.pair.v1",
        payload.to_vec(),
        AuthoredDraftStage::Draft,
        None,
        10,
    )
    .unwrap()
}
fn pair(a: AuthoredDraft, b: AuthoredDraft) -> AuthoredDraftPair {
    AuthoredDraftPair::new(a, None, b, None).unwrap()
}
async fn contract(store: &dyn AuthoredDraftStore) {
    let a = draft(1, b"claim");
    let b = draft(2, b"captured");
    let request = pair(a.clone(), b.clone());
    let receipt = store
        .append_authored_draft_pair(request.clone())
        .await
        .unwrap();
    assert_eq!(receipt[0].draft(), &a);
    assert_eq!(receipt[1].draft(), &b);
    assert!(
        receipt
            .iter()
            .all(|v| v.disposition() == DraftAppendDisposition::Inserted)
    );
    let replay = store
        .append_authored_draft_pair(request.clone())
        .await
        .unwrap();
    assert!(
        replay
            .iter()
            .all(|v| v.disposition() == DraftAppendDisposition::Replay)
    );
    // First-member conflict and second-member conflict must retain no fresh row.
    for candidate in [
        pair(draft(1, b"changed"), draft(3, b"new")),
        pair(draft(3, b"new"), draft(2, b"changed")),
        pair(a.clone(), draft(3, b"new")),
        pair(draft(3, b"new"), b.clone()),
    ] {
        assert_eq!(
            store.append_authored_draft_pair(candidate).await,
            Err(Error::DraftRevisionConflict)
        );
        assert!(
            store
                .authored_draft_head(draft(3, b"new").draft_id())
                .await
                .unwrap()
                .is_none()
        );
    }
    let a2 = a
        .successor(b"next claim".to_vec(), AuthoredDraftStage::Draft, None, 11)
        .unwrap();
    let b2 = b
        .successor(
            b"next capture".to_vec(),
            AuthoredDraftStage::Draft,
            None,
            11,
        )
        .unwrap();
    let incorrect =
        AuthoredDraftPair::new(a2.clone(), Some(a.revision()), b2.clone(), None).unwrap();
    assert_eq!(
        store.append_authored_draft_pair(incorrect).await,
        Err(Error::DraftRevisionConflict)
    );
    assert_eq!(
        store.authored_draft_head(a.draft_id()).await.unwrap(),
        Some(a.clone())
    );
    let incorrect =
        AuthoredDraftPair::new(a2.clone(), None, b2.clone(), Some(b.revision())).unwrap();
    assert_eq!(
        store.append_authored_draft_pair(incorrect).await,
        Err(Error::DraftRevisionConflict)
    );
    let next = AuthoredDraftPair::new(
        a2.clone(),
        Some(a.revision()),
        b2.clone(),
        Some(b.revision()),
    )
    .unwrap();
    store.append_authored_draft_pair(next).await.unwrap();
    // Replay returns immutable history and never rewinds the current head.
    let replay = store.append_authored_draft_pair(request).await.unwrap();
    assert_eq!(replay[0].draft(), &a);
    assert_eq!(
        store.authored_draft_head(a.draft_id()).await.unwrap(),
        Some(a2)
    );
    assert_eq!(
        store.authored_draft_head(b.draft_id()).await.unwrap(),
        Some(b2)
    );
    // The untouched single-row API retains its original behavior.
    let c = draft(4, b"single");
    store.append_authored_draft(c.clone(), None).await.unwrap();
    assert_eq!(
        store
            .append_authored_draft(c, None)
            .await
            .unwrap()
            .disposition(),
        DraftAppendDisposition::Replay
    );
}

use radroots_storage::event::SourceGeneration;
use radroots_storage_sqlite::{OpenMode, OpenOptions, Paths, SqliteStorage};
async fn open(root: &std::path::Path, mode: OpenMode) -> SqliteStorage {
    let options = OpenOptions::new(Paths::from_directory(root).unwrap(), mode);
    let options = if mode == OpenMode::Create {
        options
            .with_source_generation(SourceGeneration::new([7; 32]).unwrap(), 10)
            .unwrap()
    } else {
        options
    };
    SqliteStorage::open(options).await.unwrap()
}
#[tokio::test]
async fn sqlite_pair_contract_persists_across_reopen_and_read_only_refuses() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path(), OpenMode::Create).await;
    contract(&store).await;
    store.close().await.unwrap();
    let store = open(dir.path(), OpenMode::ReadOnly).await;
    for id in [1, 2] {
        assert_eq!(
            store
                .authored_draft_head(draft(id, b"lookup").draft_id())
                .await
                .unwrap()
                .unwrap()
                .revision()
                .get(),
            2
        );
    }
    assert_eq!(
        store
            .append_authored_draft_pair(pair(draft(5, b"a"), draft(6, b"b")))
            .await,
        Err(Error::BackendUnavailable)
    );
    store.close().await.unwrap();
    let store = open(dir.path(), OpenMode::ReadWriteExisting).await;
    let replay = store
        .append_authored_draft_pair(pair(draft(1, b"claim"), draft(2, b"captured")))
        .await
        .unwrap();
    assert!(
        replay
            .iter()
            .all(|v| v.disposition() == DraftAppendDisposition::Replay)
    );
    store.close().await.unwrap();
}
#[tokio::test]
async fn concurrent_sqlite_reservations_keep_exactly_one_complete_pair() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path(), OpenMode::Create).await;
    let (a, b) = tokio::join!(
        store.append_authored_draft_pair(pair(draft(1, b"a"), draft(2, b"capture a"))),
        store.append_authored_draft_pair(pair(draft(1, b"b"), draft(3, b"capture b")))
    );
    assert_ne!(a.is_ok(), b.is_ok());
    assert_eq!(
        store
            .authored_draft_head(draft(2, b"lookup").draft_id())
            .await
            .unwrap()
            .is_some(),
        a.is_ok()
    );
    assert_eq!(
        store
            .authored_draft_head(draft(3, b"lookup").draft_id())
            .await
            .unwrap()
            .is_some(),
        b.is_ok()
    );
    store.close().await.unwrap();
    let store = open(dir.path(), OpenMode::ReadWriteExisting).await;
    assert_eq!(
        store.authored_draft_heads([9; 32], 10).await.unwrap().len(),
        2
    );
    store.close().await.unwrap();
}
