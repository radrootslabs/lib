use super::*;
use crate::{OpenMode, OpenOptions, Paths};
use radroots_storage::{authored_draft_pair::AuthoredDraftPair, event::SourceGeneration};
use std::{future::poll_fn, task::Poll};
fn draft(id: u8, schema: &str) -> AuthoredDraft {
    AuthoredDraft::initial(
        AuthoredDraftId::new([id; 16]).unwrap(),
        [9; 32],
        schema,
        b"fixture".to_vec(),
        AuthoredDraftStage::Draft,
        None,
        10,
    )
    .unwrap()
}
async fn open(path: &std::path::Path, mode: OpenMode) -> SqliteStorage {
    let options = OpenOptions::new(Paths::from_directory(path).unwrap(), mode);
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
async fn second_sql_insert_failure_rolls_back_first_revision() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path(), OpenMode::Create).await;
    sqlx::query("CREATE TRIGGER fixture_pair_abort BEFORE INSERT ON radroots_runtime_authored_draft_revisions WHEN NEW.payload_schema='fixture.reject.v1' BEGIN SELECT RAISE(ABORT,'fixture pair rejection'); END").execute(store.pool()).await.unwrap();
    let request = AuthoredDraftPair::new(
        draft(1, "fixture.pair.v1"),
        None,
        draft(2, "fixture.reject.v1"),
        None,
    )
    .unwrap();
    assert!(store.append_authored_draft_pair(request).await.is_err());
    assert!(
        store
            .authored_draft_heads([9; 32], 10)
            .await
            .unwrap()
            .is_empty()
    );
    sqlx::query("DROP TRIGGER fixture_pair_abort")
        .execute(store.pool())
        .await
        .unwrap();
    store.close().await.unwrap();
    let store = open(dir.path(), OpenMode::ReadWriteExisting).await;
    assert!(
        store
            .authored_draft_heads([9; 32], 10)
            .await
            .unwrap()
            .is_empty()
    );
    store.close().await.unwrap();
}
#[tokio::test]
async fn cancelling_real_pair_futures_never_leaves_one_member_after_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path(), OpenMode::Create).await;
    let mut cancelled = 0;
    let mut completed = 0;
    let mut durable = Vec::new();
    // Cancel at successive real Pending boundaries, including caller loss near
    // commit. No test hook or surrogate transaction replaces the public method.
    for boundary in 1..=64u8 {
        let a = draft(boundary * 2, "fixture.pair.v1");
        let b = draft(boundary * 2 + 1, "fixture.pair.v1");
        let mut future = store.append_authored_draft_pair(
            AuthoredDraftPair::new(a.clone(), None, b.clone(), None).unwrap(),
        );
        let mut pending = 0;
        let acknowledged = poll_fn(|cx| match future.as_mut().poll(cx) {
            Poll::Ready(result) => {
                result.unwrap();
                Poll::Ready(true)
            }
            Poll::Pending => {
                pending += 1;
                if pending == boundary {
                    Poll::Ready(false)
                } else {
                    Poll::Pending
                }
            }
        })
        .await;
        drop(future);
        if acknowledged {
            completed += 1;
        } else {
            cancelled += 1;
        }
        // Drain any queued commit/rollback before observing both heads.
        store
            .pool()
            .begin_with("BEGIN IMMEDIATE")
            .await
            .unwrap()
            .rollback()
            .await
            .unwrap();
        let first = store.authored_draft_head(a.draft_id()).await.unwrap();
        let second = store.authored_draft_head(b.draft_id()).await.unwrap();
        assert_eq!(first.is_some(), second.is_some(), "boundary {boundary}");
        if acknowledged {
            assert_eq!(first, Some(a.clone()));
        }
        durable.push((a, b, first.is_some()));
    }
    assert!(
        cancelled > 0 && completed > 0,
        "cancelled={cancelled}, completed={completed}"
    );
    store.close().await.unwrap();
    let store = open(dir.path(), OpenMode::ReadWriteExisting).await;
    for (a, b, present) in durable {
        assert_eq!(
            store
                .authored_draft_head(a.draft_id())
                .await
                .unwrap()
                .is_some(),
            present
        );
        assert_eq!(
            store
                .authored_draft_head(b.draft_id())
                .await
                .unwrap()
                .is_some(),
            present
        );
        if present {
            let receipts = store
                .append_authored_draft_pair(AuthoredDraftPair::new(a, None, b, None).unwrap())
                .await
                .unwrap();
            assert!(
                receipts
                    .iter()
                    .all(|r| r.disposition() == DraftAppendDisposition::Replay)
            );
        }
    }
    store.close().await.unwrap();
}
