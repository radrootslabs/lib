use super::*;
use crate::{OpenMode, OpenOptions, Paths};
use radroots_storage::{
    authored_draft::{
        AuthoredDraft, AuthoredDraftId, AuthoredDraftRevision, AuthoredDraftStage,
        AuthoredDraftStore,
    },
    authored_draft_query::AuthoredDraftScope,
    authored_draft_submission::{AuthoredDraftSource, PrepareFromDraft},
    event::SourceGeneration,
    memory::MemoryStorage,
};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

async fn open(temp: &TempDir, mode: OpenMode) -> SqliteStorage {
    let options = OpenOptions::new(Paths::from_directory(temp.path()).unwrap(), mode);
    let options = if mode == OpenMode::Create {
        options
            .with_source_generation(SourceGeneration::new([9; 32]).unwrap(), 9)
            .unwrap()
    } else {
        options
    };
    SqliteStorage::open(options).await.unwrap()
}
fn fixture() -> (AuthoredDraft, PrepareFromDraft) {
    let (ordinary, plan) = super::tests::prepare();
    let AuthoredAtomicCommand::Prepare(preparation) = ordinary else {
        unreachable!()
    };
    let scope = AuthoredDraftScope::new([3; 32]).unwrap();
    let source = AuthoredDraft::initial(
        AuthoredDraftId::new([8; 16]).unwrap(),
        *plan.author().as_bytes(),
        "fixture.partial.v1",
        b"partial 0.".to_vec(),
        AuthoredDraftStage::Draft,
        None,
        9,
    )
    .unwrap()
    .with_scope(scope)
    .unwrap();
    let payload = b"complete captured request".to_vec();
    let intent = AuthoredDraft::reconstruct(
        AuthoredDraftId::new([7; 16]).unwrap(),
        AuthoredDraftRevision::INITIAL,
        *source.author(),
        "fixture.intent.v1",
        payload.clone(),
        Sha256::digest(&payload).into(),
        AuthoredDraftStage::Queued,
        Some(preparation.operation().operation_id()),
        10,
        10,
    )
    .unwrap()
    .with_scope(scope)
    .unwrap();
    let request = PrepareFromDraft::new(
        AtomicCommitId::new([4; 16]).unwrap(),
        AuthoredDraftSource::capture(&source).unwrap(),
        intent,
        preparation,
    )
    .unwrap();
    (source, request)
}
fn command(request: &PrepareFromDraft) -> AuthoredAtomicCommand {
    AuthoredAtomicCommand::PrepareFromDraft(Box::new(request.clone()))
}
async fn empty_submission(
    store: &SqliteStorage,
    source: &AuthoredDraft,
    request: &PrepareFromDraft,
) {
    for table in [
        "operations",
        "artifacts",
        "delivery_plans",
        "delivery_targets",
        "atomic_commits",
    ] {
        // Audited test-only identifiers come exclusively from the literal list above.
        let count: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT COUNT(*) FROM radroots_runtime_authored_{table}"
        )))
        .fetch_one(store.pool())
        .await
        .unwrap();
        assert_eq!(count, 0, "{table}");
    }
    assert_eq!(
        store
            .authored_draft_heads(*source.author(), 10)
            .await
            .unwrap(),
        std::slice::from_ref(source)
    );
    assert!(
        store
            .authored_receipt(request.commit_id())
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn submission_survives_lost_callback_reopen_and_matches_memory_after_later_save() {
    let temp = TempDir::new().unwrap();
    let store = open(&temp, OpenMode::Create).await;
    let memory = MemoryStorage::default();
    let (source, request) = fixture();
    store
        .append_authored_draft(source.clone(), None)
        .await
        .unwrap();
    memory
        .append_authored_draft(source.clone(), None)
        .await
        .unwrap();
    let expected = memory.execute_authored(command(&request)).await.unwrap();
    assert_eq!(
        store.execute_authored(command(&request)).await.unwrap(),
        expected
    );
    let newer = source
        .successor(b"later edit".to_vec(), AuthoredDraftStage::Draft, None, 11)
        .unwrap();
    for target in [&store as &dyn AuthoredDraftStore, &memory] {
        target
            .append_authored_draft(newer.clone(), Some(source.revision()))
            .await
            .unwrap();
    }
    // Discard the callback value and release the actual writer before recovery.
    store.close().await.unwrap();
    assert!(store.execute_authored(command(&request)).await.is_err());
    let store = open(&temp, OpenMode::ReadWriteExisting).await;
    let replay = store.execute_authored(command(&request)).await.unwrap();
    assert_eq!(
        replay,
        memory.execute_authored(command(&request)).await.unwrap()
    );
    assert_eq!(replay.disposition(), AtomicCommitDisposition::Replay);
    assert_eq!(replay.outcome(), expected.outcome());
    let ordinary = AuthoredAtomicCommand::Prepare(request.preparation().clone());
    assert_eq!(
        store.execute_authored(ordinary.clone()).await.unwrap(),
        memory.execute_authored(ordinary).await.unwrap()
    );
    // A valid changed context under the same author/command conflicts before CAS.
    let mut changed = serde_json::to_value(&request).unwrap();
    changed["source"]["scope"] = serde_json::json!([6; 32].to_vec());
    changed["intent"]["scope"] = serde_json::json!([6; 32].to_vec());
    let changed = serde_json::from_value::<PrepareFromDraft>(changed).unwrap();
    assert_eq!(command(&changed).digest(), command(&request).digest());
    assert_eq!(
        store.execute_authored(command(&changed)).await,
        Err(Error::AtomicCommitConflict)
    );
    let reader = open(&temp, OpenMode::ReadOnly).await;
    assert!(
        reader
            .authored_receipt(request.commit_id())
            .await
            .unwrap()
            .is_some()
    );
    assert!(reader.execute_authored(command(&request)).await.is_err());
    reader.close().await.unwrap();
    store.close().await.unwrap();
}

#[tokio::test]
async fn submission_rolls_back_before_and_after_every_record_and_receipt_insert() {
    let (source, request) = fixture();
    let ordinary = AuthoredAtomicCommand::Prepare(request.preparation().clone());
    let hex = |id: AtomicCommitId| {
        id.as_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    };
    let points = [
        ("operations", String::new()),
        ("artifacts", String::new()),
        ("delivery_plans", String::new()),
        ("delivery_targets", "WHEN NEW.ordinal = 0".into()),
        ("delivery_targets", "WHEN NEW.ordinal = 1".into()),
        (
            "draft_revisions",
            "WHEN NEW.draft_id != x'08080808080808080808080808080808'".into(),
        ),
        (
            "atomic_commits",
            format!("WHEN NEW.commit_id = x'{}'", hex(ordinary.commit_id())),
        ),
        (
            "atomic_commits",
            format!("WHEN NEW.commit_id = x'{}'", hex(request.commit_id())),
        ),
    ];
    for timing in ["BEFORE", "AFTER"] {
        for (table, condition) in &points {
            let temp = TempDir::new().unwrap();
            let store = open(&temp, OpenMode::Create).await;
            store
                .append_authored_draft(source.clone(), None)
                .await
                .unwrap();
            // Timing/table/condition are fixed test literals plus hex-encoded typed IDs.
            sqlx::query(sqlx::AssertSqlSafe(format!("CREATE TRIGGER submission_fault {timing} INSERT ON radroots_runtime_authored_{table} {condition} BEGIN SELECT RAISE(ABORT, 'injected submission failure'); END")))
                .execute(store.pool()).await.unwrap();
            assert!(
                store.execute_authored(command(&request)).await.is_err(),
                "{timing} {table} {condition}"
            );
            empty_submission(&store, &source, &request).await;
            sqlx::query("DROP TRIGGER submission_fault")
                .execute(store.pool())
                .await
                .unwrap();
            store.close().await.unwrap();
            let store = open(&temp, OpenMode::ReadWriteExisting).await;
            empty_submission(&store, &source, &request).await;
            assert_eq!(
                store
                    .execute_authored(command(&request))
                    .await
                    .unwrap()
                    .disposition(),
                AtomicCommitDisposition::Committed
            );
            store.close().await.unwrap();
        }
    }
}

#[tokio::test]
async fn abandoned_precommit_and_sqlite_full_preserve_source_without_success_association() {
    let temp = TempDir::new().unwrap();
    let store = open(&temp, OpenMode::Create).await;
    let (source, request) = fixture();
    store
        .append_authored_draft(source.clone(), None)
        .await
        .unwrap();
    let mut transaction = store.pool().begin_with("BEGIN IMMEDIATE").await.unwrap();
    let receipt = execute_transaction(&mut transaction, &command(&request))
        .await
        .unwrap();
    assert_eq!(receipt.disposition(), AtomicCommitDisposition::Committed);
    // This internal result has not crossed the public commit boundary.
    drop(transaction);
    empty_submission(&store, &source, &request).await;
    let mut transaction = store.pool().begin_with("BEGIN IMMEDIATE").await.unwrap();
    let page_count: i64 = sqlx::query_scalar("PRAGMA page_count")
        .fetch_one(&mut *transaction)
        .await
        .unwrap();
    // The PRAGMA accepts an integer, not a bind parameter; this is a typed i64.
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "PRAGMA max_page_count = {page_count}"
    )))
    .execute(&mut *transaction)
    .await
    .unwrap();
    let mut large = serde_json::to_value(&request).unwrap();
    let payload = vec![97u8; 100_000];
    large["intent"]["payload"] = serde_json::json!(payload);
    large["intent"]["payload_sha256"] = serde_json::json!(Sha256::digest(&payload).to_vec());
    let large: PrepareFromDraft = serde_json::from_value(large).unwrap();
    assert!(
        execute_transaction(&mut transaction, &command(&large))
            .await
            .is_err()
    );
    // SQLITE_FULL may already roll back the transaction at the engine boundary.
    let _ = transaction.rollback().await;
    empty_submission(&store, &source, &request).await;
    store.close().await.unwrap();
    let store = open(&temp, OpenMode::ReadWriteExisting).await;
    assert_eq!(
        store
            .execute_authored(command(&request))
            .await
            .unwrap()
            .disposition(),
        AtomicCommitDisposition::Committed
    );
    store.close().await.unwrap();
}

#[tokio::test]
async fn concurrent_duplicates_and_save_have_one_atomic_order() {
    for _ in 0..4 {
        let temp = TempDir::new().unwrap();
        let store = open(&temp, OpenMode::Create).await;
        let (source, request) = fixture();
        store
            .append_authored_draft(source.clone(), None)
            .await
            .unwrap();
        let newer = source
            .successor(
                b"concurrent edit".to_vec(),
                AuthoredDraftStage::Draft,
                None,
                11,
            )
            .unwrap();
        let (first, second, save) = tokio::join!(
            store.execute_authored(command(&request)),
            store.execute_authored(command(&request)),
            store.append_authored_draft(newer.clone(), Some(source.revision()))
        );
        save.unwrap();
        match (first, second) {
            (Ok(a), Ok(b)) => {
                assert_eq!(a.outcome(), b.outcome());
                assert_ne!(a.disposition(), b.disposition());
                assert_eq!(
                    store
                        .execute_authored(command(&request))
                        .await
                        .unwrap()
                        .disposition(),
                    AtomicCommitDisposition::Replay
                );
                let operations: i64 =
                    sqlx::query_scalar("SELECT COUNT(*) FROM radroots_runtime_authored_operations")
                        .fetch_one(store.pool())
                        .await
                        .unwrap();
                assert_eq!(operations, 1);
            }
            (Err(Error::DraftRevisionConflict), Err(Error::DraftRevisionConflict)) => {
                assert!(
                    store
                        .authored_receipt(request.commit_id())
                        .await
                        .unwrap()
                        .is_none()
                );
                assert!(
                    store
                        .authored_draft_head(request.intent().draft_id())
                        .await
                        .unwrap()
                        .is_none()
                );
            }
            other => panic!("non-atomic submit/save result: {other:?}"),
        }
        assert_eq!(
            store.authored_draft_head(source.draft_id()).await.unwrap(),
            Some(newer)
        );
        store.close().await.unwrap();
    }
}

#[tokio::test]
async fn commit_failure_returns_no_success_and_rolls_back_every_authored_write() {
    let temp = TempDir::new().unwrap();
    let store = open(&temp, OpenMode::Create).await;
    let (source, request) = fixture();
    store
        .append_authored_draft(source.clone(), None)
        .await
        .unwrap();
    sqlx::query("CREATE TABLE submission_commit_fault (parent BLOB REFERENCES radroots_runtime_authored_operations(operation_id) DEFERRABLE INITIALLY DEFERRED)")
        .execute(store.pool()).await.unwrap();
    // The last receipt succeeds; only the actual transaction COMMIT fails.
    sqlx::query("CREATE TRIGGER submission_commit_fault_trigger AFTER INSERT ON radroots_runtime_authored_atomic_commits BEGIN INSERT INTO submission_commit_fault VALUES (x'99999999999999999999999999999999'); END")
        .execute(store.pool()).await.unwrap();
    assert!(store.execute_authored(command(&request)).await.is_err());
    empty_submission(&store, &source, &request).await;
    sqlx::query("DROP TRIGGER submission_commit_fault_trigger")
        .execute(store.pool())
        .await
        .unwrap();
    sqlx::query("DROP TABLE submission_commit_fault")
        .execute(store.pool())
        .await
        .unwrap();
    assert_eq!(
        store
            .execute_authored(command(&request))
            .await
            .unwrap()
            .disposition(),
        AtomicCommitDisposition::Committed
    );
    store.close().await.unwrap();
}

#[test]
fn storage_submission_machine_policy_matches_native_bounds() {
    let policy: serde_json::Value = serde_json::from_str(include_str!(
        "../../../contracts/architecture/decisions/authored_draft_submission.v1.json"
    ))
    .unwrap();
    let bounds = &policy["bounds"];
    assert_eq!(
        bounds["page_records"],
        radroots_storage::authored_draft::AUTHORED_DRAFT_QUERY_LIMIT_MAX
    );
    assert_eq!(
        bounds["decoded_page_payload_bytes"],
        radroots_storage::authored_draft_query::AUTHORED_DRAFT_PAGE_PAYLOAD_MAX_BYTES
    );
    assert_eq!(
        bounds["serialized_page_snapshot_bytes"],
        radroots_storage::authored_draft_query::AUTHORED_DRAFT_PAGE_SNAPSHOT_MAX_BYTES
    );
    assert_eq!(
        bounds["existing_atomic_receipt_snapshot_bytes"],
        super::SNAPSHOT_MAX_BYTES
    );
    assert_eq!(
        policy["owners"],
        serde_json::json!([
            "radroots_storage",
            "radroots_storage_sqlite",
            "radroots_sync"
        ])
    );
}

#[tokio::test]
async fn existing_intent_and_partial_graph_id_collisions_cannot_create_associations() {
    let (source, request) = fixture();
    let temp = TempDir::new().unwrap();
    let store = open(&temp, OpenMode::Create).await;
    store
        .append_authored_draft(source.clone(), None)
        .await
        .unwrap();
    store
        .append_authored_draft(request.intent().clone(), None)
        .await
        .unwrap();
    assert_eq!(
        store.execute_authored(command(&request)).await,
        Err(Error::DraftRevisionConflict)
    );
    assert!(
        store
            .authored_receipt(request.commit_id())
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        store
            .authored_operation(request.preparation().operation().operation_id())
            .await
            .unwrap()
            .is_none()
    );
    store.close().await.unwrap();
    let temp = TempDir::new().unwrap();
    let store = open(&temp, OpenMode::Create).await;
    store
        .execute_authored(AuthoredAtomicCommand::Prepare(
            request.preparation().clone(),
        ))
        .await
        .unwrap();
    for artifact_collision in [true, false] {
        let base = request.preparation();
        let operation_id = OperationInstanceId::new([20; 16]).unwrap();
        let artifact_id = if artifact_collision {
            base.artifacts()[0].artifact_id()
        } else {
            AuthoredArtifactId::new([21; 16]).unwrap()
        };
        let plan_id = if artifact_collision {
            AuthoredDeliveryPlanId::new([22; 16]).unwrap()
        } else {
            base.delivery_plans()[0].plan_id()
        };
        let artifact = AuthoredArtifact::planned(
            artifact_id,
            operation_id,
            0,
            base.artifacts()[0].plan().unwrap().decode().unwrap().plan(),
            10,
        )
        .unwrap();
        let preparation = radroots_storage::authored_atomic::PrepareAuthoredOperation::new(
            AuthoredOperation::new(operation_id, vec![artifact_id], 10).unwrap(),
            vec![artifact],
            vec![
                AuthoredDeliveryPlan::new(
                    plan_id,
                    artifact_id,
                    base.delivery_plans()[0].intent().clone(),
                    10,
                )
                .unwrap(),
            ],
            base.input_digest(),
            10,
        )
        .unwrap();
        assert_eq!(
            store
                .execute_authored(AuthoredAtomicCommand::Prepare(preparation))
                .await,
            Err(Error::AtomicCommitConflict)
        );
        assert!(
            store
                .authored_operation(operation_id)
                .await
                .unwrap()
                .is_none()
        );
    }
    store.close().await.unwrap();
}

#[tokio::test]
async fn submission_receipt_shadow_times_must_match_the_captured_request() {
    let temp = TempDir::new().unwrap();
    let store = open(&temp, OpenMode::Create).await;
    let (source, request) = fixture();
    store.append_authored_draft(source, None).await.unwrap();
    store.execute_authored(command(&request)).await.unwrap();
    for requested in [9i64, 11] {
        let row = sqlx::query("SELECT commit_id, commit_digest, ? AS requested_at_unix_ms, committed_at_unix_ms, receipt FROM radroots_runtime_authored_atomic_commits WHERE commit_id = ?")
            .bind(requested).bind(request.commit_id().as_bytes().as_slice()).fetch_one(store.pool()).await.unwrap();
        assert_eq!(decode_receipt_row(&row), Err(Error::AtomicCommitFailed));
    }
    assert_eq!(
        store
            .execute_authored(command(&request))
            .await
            .unwrap()
            .disposition(),
        AtomicCommitDisposition::Replay
    );
    store.close().await.unwrap();
}
