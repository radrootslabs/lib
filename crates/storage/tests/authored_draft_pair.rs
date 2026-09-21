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

#[test]
fn memory_atomic_pair_conflict_replay_and_successor_contract() {
    futures_executor::block_on(contract(&radroots_storage::memory::MemoryStorage::default()));
}
#[test]
fn pair_rejects_duplicate_keys_and_foreign_author() {
    let a = draft(1, b"a");
    assert_eq!(
        AuthoredDraftPair::new(a.clone(), None, a.clone(), None),
        Err(Error::InvalidAuthoredDraft)
    );
    let foreign = AuthoredDraft::initial(
        AuthoredDraftId::new([2; 16]).unwrap(),
        [8; 32],
        "fixture.pair.v1",
        b"foreign".to_vec(),
        AuthoredDraftStage::Draft,
        None,
        10,
    )
    .unwrap();
    assert_eq!(
        AuthoredDraftPair::new(a, None, foreign, None),
        Err(Error::InvalidAuthoredDraft)
    );
}
#[test]
fn concurrent_memory_reservations_commit_one_complete_pair() {
    use std::sync::{Arc, Barrier};
    let store = Arc::new(radroots_storage::memory::MemoryStorage::default());
    let barrier = Arc::new(Barrier::new(2));
    let tasks: Vec<_> =
        (2..=3)
            .map(|id| {
                let store = store.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    (
                        id,
                        futures_executor::block_on(store.append_authored_draft_pair(pair(
                            draft(1, &[id]),
                            draft(id, b"capture"),
                        ))),
                    )
                })
            })
            .collect();
    let results: Vec<_> = tasks.into_iter().map(|t| t.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|(_, v)| v.is_ok()).count(), 1);
    for (id, result) in results {
        assert_eq!(
            futures_executor::block_on(store.authored_draft_head(draft(id, b"lookup").draft_id()))
                .unwrap()
                .is_some(),
            result.is_ok()
        );
    }
}

struct Unsupported(radroots_storage::memory::MemoryStorage);
impl AuthoredDraftStore for Unsupported {
    fn query_authored_drafts(
        &self,
        q: radroots_storage::authored_draft_query::AuthoredDraftQuery,
    ) -> radroots_transport::BoxFuture<
        '_,
        Result<radroots_storage::authored_draft_query::AuthoredDraftPage, Error>,
    > {
        self.0.query_authored_drafts(q)
    }
    fn append_authored_draft(
        &self,
        d: AuthoredDraft,
        e: Option<radroots_storage::authored_draft::AuthoredDraftRevision>,
    ) -> radroots_transport::BoxFuture<
        '_,
        Result<radroots_storage::authored_draft::DraftAppendReceipt, Error>,
    > {
        self.0.append_authored_draft(d, e)
    }
    fn authored_draft_head(
        &self,
        id: AuthoredDraftId,
    ) -> radroots_transport::BoxFuture<'_, Result<Option<AuthoredDraft>, Error>> {
        self.0.authored_draft_head(id)
    }
    fn authored_draft_revision(
        &self,
        id: AuthoredDraftId,
        r: radroots_storage::authored_draft::AuthoredDraftRevision,
    ) -> radroots_transport::BoxFuture<'_, Result<Option<AuthoredDraft>, Error>> {
        self.0.authored_draft_revision(id, r)
    }
    fn authored_draft_heads(
        &self,
        a: [u8; 32],
        n: u16,
    ) -> radroots_transport::BoxFuture<'_, Result<Vec<AuthoredDraft>, Error>> {
        self.0.authored_draft_heads(a, n)
    }
}
#[test]
fn unsupported_backend_never_falls_back_to_separate_writes() {
    let store = Unsupported(radroots_storage::memory::MemoryStorage::default());
    assert_eq!(
        futures_executor::block_on(
            store.append_authored_draft_pair(pair(draft(1, b"a"), draft(2, b"b")))
        ),
        Err(Error::BackendUnavailable)
    );
    assert!(
        futures_executor::block_on(store.authored_draft_heads([9; 32], 10))
            .unwrap()
            .is_empty()
    );
}
