use futures_executor::block_on;
use radroots_storage::{
    Error,
    authored_draft::{
        AUTHORED_DRAFT_QUERY_LIMIT_MAX, AuthoredDraft, AuthoredDraftId, AuthoredDraftRevision,
        AuthoredDraftStage, AuthoredDraftStore, DraftAppendDisposition,
    },
    memory::MemoryStorage,
};

fn draft(id: u8, author: u8, created_at_unix_ms: u64) -> AuthoredDraft {
    AuthoredDraft::initial(
        AuthoredDraftId::new([id; 16]).expect("draft ID"),
        [author; 32],
        "radroots.phase1-draft.v1",
        vec![id],
        AuthoredDraftStage::Draft,
        None,
        created_at_unix_ms,
    )
    .expect("draft")
}

#[test]
fn memory_draft_store_replays_conflicts_and_queries_exact_heads() {
    fn accepts_dyn(_: &dyn AuthoredDraftStore) {}

    let store = MemoryStorage::default();
    accepts_dyn(&store);
    let first = draft(1, 9, 10);
    assert_eq!(
        block_on(store.append_authored_draft(first.clone(), None))
            .expect("insert")
            .disposition(),
        DraftAppendDisposition::Inserted
    );
    let replay = block_on(store.append_authored_draft(first.clone(), None)).expect("replay");
    assert_eq!(replay.disposition(), DraftAppendDisposition::Replay);
    assert_eq!(replay.draft(), &first);

    let conflicting = AuthoredDraft::initial(
        first.draft_id(),
        *first.author(),
        first.payload_schema(),
        b"conflict".to_vec(),
        AuthoredDraftStage::Draft,
        None,
        first.created_at_unix_ms(),
    )
    .expect("conflicting revision");
    assert_eq!(
        block_on(store.append_authored_draft(conflicting, None)),
        Err(Error::DraftRevisionConflict)
    );

    let second = first
        .successor(
            b"second".to_vec(),
            AuthoredDraftStage::MediaPreparing,
            None,
            11,
        )
        .expect("successor");
    assert_eq!(
        block_on(store.append_authored_draft(second.clone(), None)),
        Err(Error::DraftRevisionConflict)
    );
    assert_eq!(
        block_on(store.append_authored_draft(
            second.clone(),
            Some(AuthoredDraftRevision::new(2).expect("revision")),
        )),
        Err(Error::DraftRevisionConflict)
    );
    block_on(store.append_authored_draft(second.clone(), Some(AuthoredDraftRevision::INITIAL)))
        .expect("successor insert");

    assert_eq!(
        block_on(store.authored_draft_head(first.draft_id()))
            .expect("head")
            .expect("stored head"),
        second
    );
    assert_eq!(
        block_on(store.authored_draft_revision(first.draft_id(), first.revision()))
            .expect("revision")
            .expect("stored revision"),
        first
    );
    assert!(
        block_on(store.authored_draft_revision(
            AuthoredDraftId::new([8; 16]).expect("missing ID"),
            AuthoredDraftRevision::INITIAL,
        ))
        .expect("missing revision query")
        .is_none()
    );
}

#[test]
fn memory_draft_head_query_is_bounded_filtered_sorted_and_truncated() {
    let store = MemoryStorage::default();
    let first = draft(1, 9, 10);
    let second = draft(2, 9, 20);
    let other_author = draft(3, 8, 30);
    for value in [&first, &second, &other_author] {
        block_on(store.append_authored_draft(value.clone(), None)).expect("insert");
    }
    let first_head = first
        .successor(
            b"new head".to_vec(),
            AuthoredDraftStage::MediaPreparing,
            None,
            40,
        )
        .expect("new head");
    block_on(store.append_authored_draft(first_head.clone(), Some(first.revision())))
        .expect("head insert");

    let heads = block_on(store.authored_draft_heads([9; 32], 2)).expect("heads");
    assert_eq!(heads, vec![first_head.clone(), second]);
    assert_eq!(
        block_on(store.authored_draft_heads([9; 32], 1)).expect("truncated heads"),
        vec![first_head]
    );
    assert!(
        block_on(store.authored_draft_heads([7; 32], 2))
            .expect("unmatched author")
            .is_empty()
    );
    assert_eq!(
        block_on(store.authored_draft_heads([0; 32], 1)),
        Err(Error::InvalidAuthoredDraft)
    );
    assert_eq!(
        block_on(store.authored_draft_heads([9; 32], 0)),
        Err(Error::InvalidAuthoredDraft)
    );
    assert_eq!(
        block_on(store.authored_draft_heads([9; 32], AUTHORED_DRAFT_QUERY_LIMIT_MAX + 1,)),
        Err(Error::InvalidAuthoredDraft)
    );
}
