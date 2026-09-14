use super::{SqliteStorage, tests::open_store};
use radroots_storage::{
    Error,
    authored_draft::{AuthoredDraft, AuthoredDraftId, AuthoredDraftStage, AuthoredDraftStore},
    authored_draft_query::{AuthoredDraftQuery, AuthoredDraftQueryRecord, AuthoredDraftScope},
};
use tempfile::TempDir;

fn draft(
    id: u8,
    schema: &str,
    scope: Option<AuthoredDraftScope>,
    payload: Vec<u8>,
) -> AuthoredDraft {
    let draft = AuthoredDraft::initial(
        AuthoredDraftId::new([id; 16]).unwrap(),
        [7; 32],
        schema,
        payload,
        AuthoredDraftStage::Draft,
        None,
        10,
    )
    .unwrap();
    match scope {
        Some(scope) => draft.with_scope(scope).unwrap(),
        None => draft,
    }
}
fn query(scope: Option<AuthoredDraftScope>, limit: u16) -> AuthoredDraftQuery {
    AuthoredDraftQuery::new([7; 32], "fixture.composer.v1", scope, limit).unwrap()
}

#[tokio::test]
async fn author_wide_sqlite_pages_preserve_scope_isolation_corruption_and_restart() {
    let temp = TempDir::new().unwrap();
    let store = open_store(&temp).await;
    let scope = AuthoredDraftScope::new([9; 32]).unwrap();
    for id in 1_u128..=1000 {
        let value = AuthoredDraft::initial(
            AuthoredDraftId::new(id.to_be_bytes()).unwrap(),
            [7; 32],
            "fixture.composer.v1",
            vec![1],
            AuthoredDraftStage::Draft,
            None,
            10,
        )
        .unwrap();
        let value = if id % 2 == 0 {
            value.with_scope(scope).unwrap()
        } else {
            value
        };
        store.append_authored_draft(value, None).await.unwrap();
    }
    corrupt(&store, 0, 7, "fixture.composer.v1", Some(scope)).await;
    corrupt(&store, 2, 8, "fixture.composer.v1", Some(scope)).await;
    corrupt(&store, 3, 7, "fixture.other.v1", Some(scope)).await;
    let q = AuthoredDraftQuery::for_author([7; 32], "fixture.composer.v1", 37).unwrap();
    let first = store.query_authored_drafts(q.clone()).await.unwrap();
    assert_eq!(first.records().len(), 37);
    assert!(matches!(
        first.records()[0],
        AuthoredDraftQueryRecord::Corrupt {
            draft_key: [0, ..],
            ..
        }
    ));
    for (index, record) in first.records().iter().enumerate().skip(1) {
        assert_eq!(record.draft_key(), (index as u128).to_be_bytes());
    }
    let mut cursor = first.next_cursor().cloned();
    // Revisions of both a visited and an unvisited ID do not move position.
    for id in [1_u128, 999] {
        let original = store
            .authored_draft_head(AuthoredDraftId::new(id.to_be_bytes()).unwrap())
            .await
            .unwrap()
            .unwrap();
        let next = original
            .successor(vec![2], AuthoredDraftStage::Draft, None, 11)
            .unwrap();
        store
            .append_authored_draft(next, Some(original.revision()))
            .await
            .unwrap();
    }
    store.close().await.unwrap();
    let store = open_store(&temp).await;
    let mut expected = 37_u128;
    while let Some(current) = cursor {
        let bytes = serde_json::to_vec(&current).unwrap();
        let decoded = serde_json::from_slice(&bytes).unwrap();
        let page = store
            .query_authored_drafts(q.clone().with_cursor(&decoded).unwrap())
            .await
            .unwrap();
        assert!(page.records().len() <= 37);
        for record in page.records() {
            assert_eq!(record.draft_key(), expected.to_be_bytes());
            if expected == 999 {
                assert_eq!(record.revision().get(), 2);
            }
            expected += 1;
        }
        cursor = page.next_cursor().cloned();
    }
    assert_eq!(expected, 1001);
    let unscoped = store.query_authored_drafts(query(None, 256)).await.unwrap();
    assert!(
        unscoped
            .records()
            .iter()
            .all(|record| u128::from_be_bytes(record.draft_key()) % 2 == 1)
    );
    let fresh = store.query_authored_drafts(q).await.unwrap();
    assert_eq!(fresh.records()[1].revision().get(), 2);
    store.close().await.unwrap();
}

#[tokio::test]
async fn author_wide_sqlite_pages_preserve_snapshot_and_payload_budgets() {
    for byte in [0, 99] {
        let temp = TempDir::new().unwrap();
        let store = open_store(&temp).await;
        let scope = AuthoredDraftScope::new([9; 32]).unwrap();
        for (id, scope) in [(1, None), (2, Some(scope))] {
            store
                .append_authored_draft(
                    draft(
                        id,
                        "fixture.composer.v1",
                        scope,
                        vec![byte; 3 * 1024 * 1024],
                    ),
                    None,
                )
                .await
                .unwrap();
        }
        let q = AuthoredDraftQuery::for_author([7; 32], "fixture.composer.v1", 256).unwrap();
        let first = store.query_authored_drafts(q.clone()).await.unwrap();
        assert_eq!(first.records().len(), 1);
        let next = store
            .query_authored_drafts(q.with_cursor(first.next_cursor().unwrap()).unwrap())
            .await
            .unwrap();
        assert_eq!(next.records().len(), 1);
        assert_eq!(next.records()[0].draft_key(), [2; 16]);
        assert!(matches!(
            next.records()[0],
            AuthoredDraftQueryRecord::Draft(_)
        ));
        assert!(next.next_cursor().is_none());
        store.close().await.unwrap();
    }
}
async fn corrupt(
    store: &SqliteStorage,
    id: u8,
    author: u8,
    schema: &str,
    scope: Option<AuthoredDraftScope>,
) {
    sqlx::query("INSERT INTO radroots_runtime_authored_draft_revisions
        (draft_id, revision, author, stage, payload_sha256, created_at_unix_ms, updated_at_unix_ms, snapshot, payload_schema, payload_scope)
        VALUES (?, 1, ?, 0, ?, 10, 10, ?, ?, ?)")
        .bind([id; 16].as_slice()).bind([author; 32].as_slice()).bind([3; 32].as_slice())
        .bind(b"{malformed private payload".as_slice()).bind(schema).bind(scope.map(|scope| scope.as_bytes().to_vec()))
        .execute(store.pool()).await.unwrap();
}

#[tokio::test]
async fn scoped_sqlite_pages_isolate_corruption_foreign_schemas_and_contexts() {
    let temp = TempDir::new().unwrap();
    let store = open_store(&temp).await;
    let scope = AuthoredDraftScope::new([9; 32]).unwrap();
    let first = draft(
        2,
        "fixture.composer.v1",
        Some(scope),
        b"incomplete 0.".to_vec(),
    );
    let second = draft(6, "fixture.composer.v1", Some(scope), b"2026-".to_vec());
    for value in [
        first.clone(),
        second.clone(),
        draft(1, "fixture.profile.v1", Some(scope), b"profile".to_vec()),
        draft(3, "fixture.composer.v1", None, b"unscoped".to_vec()),
    ] {
        store.append_authored_draft(value, None).await.unwrap();
    }
    corrupt(&store, 4, 7, "fixture.composer.v1", Some(scope)).await;
    // Unknown historical schema yields only an author-bound corruption locator.
    corrupt(&store, 5, 7, "", None).await;
    corrupt(&store, 7, 8, "", None).await;
    corrupt(&store, 8, 7, "fixture.profile.v1", Some(scope)).await;
    let q = query(Some(scope), 1);
    let first_page = store.query_authored_drafts(q.clone()).await.unwrap();
    assert_eq!(
        first_page.records(),
        [AuthoredDraftQueryRecord::Draft(first.clone())]
    );
    let changed = first
        .successor(
            b"later saved edit".to_vec(),
            AuthoredDraftStage::Draft,
            None,
            11,
        )
        .unwrap();
    store
        .append_authored_draft(changed, Some(first.revision()))
        .await
        .unwrap();
    let mut cursor = first_page.next_cursor().cloned();
    let mut records = first_page.into_records();
    while let Some(next) = cursor {
        let page = store
            .query_authored_drafts(q.clone().with_cursor(&next).unwrap())
            .await
            .unwrap();
        cursor = page.next_cursor().cloned();
        records.extend(page.into_records());
    }
    assert_eq!(
        records
            .iter()
            .map(AuthoredDraftQueryRecord::draft_key)
            .collect::<Vec<_>>(),
        [[2; 16], [4; 16], [5; 16], [6; 16]]
    );
    assert!(matches!(
        records[1],
        AuthoredDraftQueryRecord::Corrupt { .. }
    ));
    assert!(matches!(
        records[2],
        AuthoredDraftQueryRecord::Corrupt { .. }
    ));
    assert_eq!(records[3], AuthoredDraftQueryRecord::Draft(second));
    assert!(!format!("{records:?}").contains("private payload"));
    assert_eq!(
        store.authored_draft_heads([7; 32], 10).await,
        Err(Error::CorruptAuthoredDraft)
    );
    store.close().await.unwrap();
    assert!(store.query_authored_drafts(q.clone()).await.is_err());
    let reopened = open_store(&temp).await;
    let page = reopened
        .query_authored_drafts(query(Some(scope), 10))
        .await
        .unwrap();
    assert_eq!(page.records().len(), 4);
    assert!(page.next_cursor().is_none());
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn sqlite_snapshot_budget_keeps_large_valid_records_on_later_pages() {
    // Small numeric bytes exhaust decoded payload first; larger ones exhaust serialized snapshots first.
    for payload_byte in [0, 99] {
        let temp = TempDir::new().unwrap();
        let store = open_store(&temp).await;
        for id in [1, 2] {
            store
                .append_authored_draft(
                    draft(
                        id,
                        "fixture.composer.v1",
                        None,
                        vec![payload_byte; 3 * 1024 * 1024],
                    ),
                    None,
                )
                .await
                .unwrap();
        }
        let q = query(None, 256);
        let first = store.query_authored_drafts(q.clone()).await.unwrap();
        assert_eq!(first.records().len(), 1);
        let next = store
            .query_authored_drafts(q.with_cursor(first.next_cursor().unwrap()).unwrap())
            .await
            .unwrap();
        assert_eq!(next.records()[0].draft_key(), [2; 16]);
        assert!(matches!(
            next.records()[0],
            AuthoredDraftQueryRecord::Draft(_)
        ));
        assert!(next.next_cursor().is_none());
        store.close().await.unwrap();
    }
}

#[tokio::test]
async fn zero_domain_identity_is_an_isolated_repair_position() {
    let temp = TempDir::new().unwrap();
    let store = open_store(&temp).await;
    corrupt(&store, 0, 7, "", None).await;
    let good = draft(1, "fixture.composer.v1", None, b"partial".to_vec());
    store
        .append_authored_draft(good.clone(), None)
        .await
        .unwrap();
    let q = query(None, 1);
    let page = store.query_authored_drafts(q.clone()).await.unwrap();
    assert!(page.records()[0].draft_id().is_err());
    let next = store
        .query_authored_drafts(q.with_cursor(page.next_cursor().unwrap()).unwrap())
        .await
        .unwrap();
    assert_eq!(next.records(), [AuthoredDraftQueryRecord::Draft(good)]);
    store.close().await.unwrap();
}

#[tokio::test]
async fn inconsistent_query_metadata_is_isolated_and_never_returns_foreign_payload() {
    for corrupt_scope in [false, true] {
        let temp = TempDir::new().unwrap();
        let store = open_store(&temp).await;
        let value = draft(1, "fixture.composer.v1", None, b"private original".to_vec());
        store
            .append_authored_draft(value.clone(), None)
            .await
            .unwrap();
        sqlx::query("DROP TRIGGER radroots_runtime_authored_draft_revisions_update_guard")
            .execute(store.pool())
            .await
            .unwrap();
        let (schema, scope) = if corrupt_scope {
            (
                "fixture.composer.v1",
                Some(AuthoredDraftScope::new([4; 32]).unwrap()),
            )
        } else {
            ("fixture.other.v1", None)
        };
        sqlx::query("UPDATE radroots_runtime_authored_draft_revisions SET payload_schema = ?, payload_scope = ?")
            .bind(schema).bind(scope.map(|v|v.as_bytes().to_vec())).execute(store.pool()).await.unwrap();
        assert_eq!(
            store.authored_draft_head(value.draft_id()).await,
            Err(Error::CorruptAuthoredDraft)
        );
        let page = store
            .query_authored_drafts(AuthoredDraftQuery::new([7; 32], schema, scope, 1).unwrap())
            .await
            .unwrap();
        assert_eq!(
            page.records(),
            [AuthoredDraftQueryRecord::Corrupt {
                draft_key: *value.draft_id().as_bytes(),
                revision: value.revision()
            }]
        );
        assert!(!format!("{page:?}").contains("private original"));
        store.close().await.unwrap();
    }
}
