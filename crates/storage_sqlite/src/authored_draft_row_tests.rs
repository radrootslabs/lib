use super::*;
use crate::authored_draft::{decode_row, query_tests::draft, tests::open_store};
use radroots_storage::{
    authored_draft::{AuthoredDraft, AuthoredDraftStore},
    authored_draft_query::{AuthoredDraftQuery, AuthoredDraftQueryRecord},
};
use sqlx::{Row, ValueRef};
use tempfile::TempDir;

// Fixture-only corruption, on one connection, with the exact migration guard
// restored before the owning reads run. No production schema is changed.
async fn corrupt_field(store: &crate::SqliteStorage, assignment: &'static str) {
    let guard: String = sqlx::query_scalar(
        "SELECT sql FROM sqlite_schema WHERE name = 'radroots_runtime_authored_draft_revisions_update_guard'",
    ).fetch_one(store.pool()).await.unwrap();
    let mut connection = store.pool().acquire().await.unwrap();
    sqlx::query("DROP TRIGGER radroots_runtime_authored_draft_revisions_update_guard")
        .execute(&mut *connection)
        .await
        .unwrap();
    sqlx::query("PRAGMA ignore_check_constraints = ON")
        .execute(&mut *connection)
        .await
        .unwrap();
    // Only closed test literals below supply this assignment.
    let statement = format!("UPDATE radroots_runtime_authored_draft_revisions SET {assignment}");
    sqlx::query(sqlx::AssertSqlSafe(statement.as_str()))
        .execute(&mut *connection)
        .await
        .unwrap();
    sqlx::query("PRAGMA ignore_check_constraints = OFF")
        .execute(&mut *connection)
        .await
        .unwrap();
    sqlx::query(sqlx::AssertSqlSafe(guard.as_str()))
        .execute(&mut *connection)
        .await
        .unwrap();
}

async fn rejects_point_reads_and_replay(store: &crate::SqliteStorage, value: &AuthoredDraft) {
    assert_eq!(
        store.authored_draft_head(value.draft_id()).await,
        Err(Error::CorruptAuthoredDraft)
    );
    assert_eq!(
        store
            .authored_draft_revision(value.draft_id(), value.revision())
            .await,
        Err(Error::CorruptAuthoredDraft)
    );
    assert_eq!(
        store.append_authored_draft(value.clone(), None).await,
        Err(Error::CorruptAuthoredDraft)
    );
    let successor = value.successor(vec![2], value.stage(), None, 11).unwrap();
    assert_eq!(
        store
            .append_authored_draft(successor, Some(value.revision()))
            .await,
        Err(Error::CorruptAuthoredDraft)
    );
    let mut transaction = store.pool().begin().await.unwrap();
    assert_eq!(
        crate::authored_draft::load_head_tx(&mut transaction, value.draft_id()).await,
        Err(Error::CorruptAuthoredDraft)
    );
    transaction.rollback().await.unwrap();
}

#[tokio::test]
async fn oversized_columns_are_bounded_in_sql_and_remain_corrupt() {
    for (assignment, column, sentinel) in [
        ("snapshot = zeroblob(16777217)", "snapshot", false),
        ("author = zeroblob(1048576)", "author", false),
        (
            "payload_sha256 = zeroblob(1048576)",
            "payload_sha256",
            false,
        ),
        ("operation_id = zeroblob(1048576)", "operation_id", true),
        ("payload_scope = zeroblob(1048576)", "payload_scope", true),
        (
            "payload_schema = replace(hex(zeroblob(65536)), '0', 'é')",
            "payload_schema",
            false,
        ),
    ] {
        let temp = TempDir::new().unwrap();
        let store = open_store(&temp).await;
        let value = draft(1, "known.v1", None, vec![1]);
        store
            .append_authored_draft(value.clone(), None)
            .await
            .unwrap();
        corrupt_field(&store, assignment).await;
        let row = load(
            store.pool(),
            value.draft_id().as_bytes(),
            Some(value.revision()),
        )
        .await
        .unwrap()
        .unwrap();
        if sentinel {
            assert_eq!(
                row.try_get::<Option<Vec<u8>>, _>(column).unwrap(),
                Some(vec![0])
            );
        } else {
            assert!(row.try_get_raw(column).unwrap().is_null(), "{column}");
        }
        assert_eq!(decode_row(&row), Err(Error::CorruptAuthoredDraft));
        rejects_point_reads_and_replay(&store, &value).await;
        // Foreign/corrupt authors remain outside this independently supplied
        // author selection; every other malformed head remains a locator.
        if column != "author" {
            let page = store
                .query_authored_drafts(
                    AuthoredDraftQuery::for_author_all_schemas([7; 32], 1).unwrap(),
                )
                .await
                .unwrap();
            assert!(matches!(
                page.records(),
                [AuthoredDraftQueryRecord::Corrupt { .. }]
            ));
        }
        store.close().await.unwrap();
    }
}

#[tokio::test]
async fn snapshot_limit_is_inclusive_and_empty_snapshot_is_rejected_before_decode() {
    for (assignment, expected) in [
        ("snapshot = zeroblob(16777216)", Some(16777216)),
        ("snapshot = X''", None),
    ] {
        let temp = TempDir::new().unwrap();
        let store = open_store(&temp).await;
        let value = draft(1, "known.v1", None, vec![1]);
        store
            .append_authored_draft(value.clone(), None)
            .await
            .unwrap();
        corrupt_field(&store, assignment).await;
        let row = load(store.pool(), value.draft_id().as_bytes(), None)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            row.try_get::<Option<Vec<u8>>, _>("snapshot")
                .unwrap()
                .map(|bytes| bytes.len()),
            expected
        );
        assert_eq!(decode_row(&row), Err(Error::CorruptAuthoredDraft));
        store.close().await.unwrap();
    }
}

#[tokio::test]
async fn malformed_key_fails_inventory_instead_of_becoming_absence() {
    let temp = TempDir::new().unwrap();
    let store = open_store(&temp).await;
    store
        .append_authored_draft(draft(1, "known.v1", None, vec![1]), None)
        .await
        .unwrap();
    corrupt_field(&store, "draft_id = zeroblob(1048576)").await;
    assert!(
        store
            .query_authored_drafts(AuthoredDraftQuery::for_author_all_schemas([7; 32], 1).unwrap())
            .await
            .is_err()
    );
    store.close().await.unwrap();
}

#[tokio::test]
async fn bounded_load_preserves_history_absence_and_compound_index() {
    let temp = TempDir::new().unwrap();
    let store = open_store(&temp).await;
    let first = draft(1, "known.v1", None, vec![1]);
    let second = first.successor(vec![2], first.stage(), None, 11).unwrap();
    store
        .append_authored_draft(first.clone(), None)
        .await
        .unwrap();
    store
        .append_authored_draft(second.clone(), Some(first.revision()))
        .await
        .unwrap();
    for (revision, expected) in [(None, &second), (Some(first.revision()), &first)] {
        let row = load(store.pool(), first.draft_id().as_bytes(), revision)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(&decode_row(&row).unwrap(), expected);
        assert!(
            row.try_get::<Option<Vec<u8>>, _>("operation_id")
                .unwrap()
                .is_none()
        );
        assert!(
            row.try_get::<Option<Vec<u8>>, _>("payload_scope")
                .unwrap()
                .is_none()
        );
    }
    assert!(load(store.pool(), &[2; 16], None).await.unwrap().is_none());
    assert!(
        load(
            store.pool(),
            first.draft_id().as_bytes(),
            Some(AuthoredDraftRevision::new(3).unwrap())
        )
        .await
        .unwrap()
        .is_none()
    );
    assert!(matches!(
        load(
            store.pool(),
            &[1; 16],
            Some(AuthoredDraftRevision::new(u64::MAX).unwrap())
        )
        .await,
        Err(Error::InvalidAuthoredDraft)
    ));
    for statement in [HEAD, REVISION] {
        let explain = format!("EXPLAIN QUERY PLAN {statement}");
        let rows = sqlx::query(sqlx::AssertSqlSafe(explain.as_str()))
            .bind([1_u8; 16].as_slice())
            .bind(1_i64)
            .fetch_all(store.pool())
            .await
            .unwrap();
        let details: Vec<String> = rows.iter().map(|row| row.get("detail")).collect();
        assert!(
            details.iter().any(|detail| detail.contains("SEARCH")
                && detail.contains("PRIMARY KEY")
                && detail.contains("draft_id=?")),
            "{details:?}"
        );
        assert!(
            !details
                .iter()
                .any(|detail| detail.contains("SCAN ") || detail.contains("TEMP B-TREE")),
            "{details:?}"
        );
    }
    store.close().await.unwrap();
    assert!(matches!(
        load(store.pool(), &[1; 16], None).await,
        Err(Error::BackendUnavailable)
    ));
}
