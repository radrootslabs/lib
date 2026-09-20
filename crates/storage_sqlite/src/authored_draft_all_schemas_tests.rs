use super::{
    query_tests::{corrupt, draft},
    tests::open_store,
};
use radroots_storage::{
    authored_draft::{AuthoredDraft, AuthoredDraftId, AuthoredDraftStage, AuthoredDraftStore},
    authored_draft_query::{
        AUTHORED_DRAFT_PAGE_SNAPSHOT_MAX_BYTES, AuthoredDraftQuery, AuthoredDraftQueryRecord,
        AuthoredDraftScope,
    },
};
use tempfile::TempDir;

#[tokio::test]
async fn all_schema_sqlite_pages_include_unknown_and_corrupt_owners_across_reopen() {
    let temp = TempDir::new().unwrap();
    let store = open_store(&temp).await;
    let scope = AuthoredDraftScope::new([3; 32]).unwrap();
    for id in 1_u128..=1001 {
        let value = AuthoredDraft::initial(
            AuthoredDraftId::new(id.to_be_bytes()).unwrap(),
            [7; 32],
            if id % 3 == 0 {
                "future.unknown.v999"
            } else {
                "known.v1"
            },
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
    corrupt(&store, 0, 7, "future.unknown.v999", Some(scope)).await;
    corrupt(&store, 2, 8, "future.unknown.v999", None).await;
    corrupt(&store, 3, 7, "", None).await;
    corrupt(&store, 4, 7, "future.oversized.v999", None).await;
    let update_guard: String = sqlx::query_scalar(
        "SELECT sql FROM sqlite_schema WHERE type = 'trigger' AND name = 'radroots_runtime_authored_draft_revisions_update_guard'"
    ).fetch_one(store.pool()).await.unwrap();
    sqlx::query("DROP TRIGGER radroots_runtime_authored_draft_revisions_update_guard")
        .execute(store.pool())
        .await
        .unwrap();
    // The production CHECK rejects oversized writes. Simulate a corrupted file
    // on one held fixture connection, then restore enforcement before querying.
    let mut connection = store.pool().acquire().await.unwrap();
    sqlx::query("PRAGMA ignore_check_constraints = ON")
        .execute(&mut *connection)
        .await
        .unwrap();
    sqlx::query("UPDATE radroots_runtime_authored_draft_revisions SET snapshot = zeroblob(?) WHERE draft_id = ?")
        .bind((AUTHORED_DRAFT_PAGE_SNAPSHOT_MAX_BYTES + 1) as i64).bind([4_u8; 16].as_slice()).execute(&mut *connection).await.unwrap();
    sqlx::query("PRAGMA ignore_check_constraints = OFF")
        .execute(&mut *connection)
        .await
        .unwrap();
    // Exact SQL captured from this fresh fixture's governed migration, with no
    // external input or interpolation. Restore its immutable catalog verbatim.
    sqlx::query(sqlx::AssertSqlSafe(update_guard.as_str()))
        .execute(&mut *connection)
        .await
        .unwrap();
    drop(connection);
    let q = AuthoredDraftQuery::for_author_all_schemas([7; 32], 37).unwrap();
    let first = store.query_authored_drafts(q.clone()).await.unwrap();
    assert_eq!(first.records().len(), 37);
    assert!(matches!(
        first.records()[0],
        AuthoredDraftQueryRecord::Corrupt { .. }
    ));
    assert!(first.records()[0].draft_id().is_err());
    let mut keys: Vec<_> = first
        .records()
        .iter()
        .map(AuthoredDraftQueryRecord::draft_key)
        .collect();
    let mut next = first.next_cursor().cloned();
    for id in [1_u128, 1001] {
        let old = store
            .authored_draft_head(AuthoredDraftId::new(id.to_be_bytes()).unwrap())
            .await
            .unwrap()
            .unwrap();
        let revised = old
            .successor(vec![2], AuthoredDraftStage::Draft, None, 11)
            .unwrap();
        store
            .append_authored_draft(revised, Some(old.revision()))
            .await
            .unwrap();
    }
    store.close().await.unwrap();
    let store = open_store(&temp).await;
    while let Some(cursor) = next {
        let cursor = serde_json::from_slice(&serde_json::to_vec(&cursor).unwrap()).unwrap();
        let page = store
            .query_authored_drafts(q.clone().with_cursor(&cursor).unwrap())
            .await
            .unwrap();
        assert!(page.records().len() <= 37);
        for row in page.records() {
            keys.push(row.draft_key());
            if row.draft_key() == 1001_u128.to_be_bytes() {
                assert_eq!(row.revision().get(), 2);
            }
            if row.draft_key() == [3; 16] || row.draft_key() == [4; 16] {
                assert!(matches!(row, AuthoredDraftQueryRecord::Corrupt { .. }));
            }
        }
        next = page.next_cursor().cloned();
    }
    let mut expected = vec![[0; 16]];
    expected.extend((1_u128..=1001).map(u128::to_be_bytes));
    expected.extend([[3; 16], [4; 16]]);
    assert_eq!(keys, expected);
    let fresh = store.query_authored_drafts(q).await.unwrap();
    assert_eq!(fresh.records()[1].revision().get(), 2);
    let exact = store
        .query_authored_drafts(AuthoredDraftQuery::for_author([7; 32], "known.v1", 37).unwrap())
        .await
        .unwrap();
    assert!(exact.records().iter().all(
        |r| matches!(r, AuthoredDraftQueryRecord::Draft(d) if d.payload_schema() == "known.v1")
    ));
    store.close().await.unwrap();
}

#[tokio::test]
async fn all_schema_sqlite_continuation_preserves_snapshot_and_payload_budgets() {
    for byte in [0, 99] {
        let temp = TempDir::new().unwrap();
        let store = open_store(&temp).await;
        for (id, schema, scope) in [
            (1, "known.v1", None),
            (
                2,
                "future.v999",
                Some(AuthoredDraftScope::new([3; 32]).unwrap()),
            ),
        ] {
            store
                .append_authored_draft(draft(id, schema, scope, vec![byte; 3 * 1024 * 1024]), None)
                .await
                .unwrap();
        }
        let q = AuthoredDraftQuery::for_author_all_schemas([7; 32], 256).unwrap();
        let first = store.query_authored_drafts(q.clone()).await.unwrap();
        assert_eq!(first.records().len(), 1);
        assert!(
            matches!(&first.records()[0], AuthoredDraftQueryRecord::Draft(d) if d.payload_schema() == "known.v1")
        );
        let next = store
            .query_authored_drafts(q.with_cursor(first.next_cursor().unwrap()).unwrap())
            .await
            .unwrap();
        assert_eq!(next.records().len(), 1);
        assert!(
            matches!(&next.records()[0], AuthoredDraftQueryRecord::Draft(d) if d.payload_schema() == "future.v999")
        );
        assert!(next.next_cursor().is_none());
        store.close().await.unwrap();
    }
}
