use super::*;
use crate::{OpenMode, OpenOptions, Paths};
use radroots_storage::event::SourceGeneration;
use std::path::Path;
use tempfile::TempDir;

#[path = "authored_durability_crash_tests.rs"]
mod crash;

async fn open(directory: &Path, mode: OpenMode) -> SqliteStorage {
    let mut options = OpenOptions::new(Paths::from_directory(directory).unwrap(), mode);
    if matches!(mode, OpenMode::Create) {
        options = options
            .with_source_generation(SourceGeneration::new([91; 32]).unwrap(), 9)
            .unwrap();
    }
    SqliteStorage::open(options).await.unwrap()
}

fn first() -> AuthoredDraft {
    AuthoredDraft::initial(
        AuthoredDraftId::new([41; 16]).unwrap(),
        [7; 32],
        "radroots.durability-fixture.v1",
        b"acknowledged baseline".to_vec(),
        AuthoredDraftStage::Draft,
        None,
        10,
    )
    .unwrap()
}

fn next(previous: &AuthoredDraft, payload: Vec<u8>) -> AuthoredDraft {
    previous
        .successor(payload, AuthoredDraftStage::Draft, None, 11)
        .unwrap()
}

async fn assert_head(store: &SqliteStorage, expected: &AuthoredDraft) {
    assert_eq!(
        store
            .authored_draft_head(expected.draft_id())
            .await
            .unwrap(),
        Some(expected.clone())
    );
}

#[tokio::test]
async fn authored_durability_commit_fault_never_acknowledges_and_exact_retry_recovers() {
    let temp = TempDir::new().unwrap();
    let store = open(temp.path(), OpenMode::Create).await;
    let baseline = first();
    let pending = next(&baseline, b"pending complete revision".to_vec());
    store
        .append_authored_draft(baseline.clone(), None)
        .await
        .unwrap();
    sqlx::query("CREATE TABLE authored_commit_parent (id INTEGER PRIMARY KEY)")
        .execute(store.pool())
        .await
        .unwrap();
    sqlx::query("CREATE TABLE authored_commit_fault (id INTEGER REFERENCES authored_commit_parent(id) DEFERRABLE INITIALLY DEFERRED)")
        .execute(store.pool()).await.unwrap();
    sqlx::query("CREATE TRIGGER authored_commit_fault_trigger AFTER INSERT ON radroots_runtime_authored_draft_revisions BEGIN INSERT INTO authored_commit_fault VALUES (99); END")
        .execute(store.pool()).await.unwrap();

    // Establish that the insertion succeeds and the actual COMMIT is the fault.
    let mut transaction = store.pool().begin_with("BEGIN IMMEDIATE").await.unwrap();
    insert_draft_tx(&mut transaction, &pending).await.unwrap();
    let failure = transaction.commit().await.unwrap_err();
    assert_eq!(
        failure.as_database_error().unwrap().kind(),
        sqlx::error::ErrorKind::ForeignKeyViolation
    );
    assert_eq!(
        store
            .append_authored_draft(pending.clone(), Some(baseline.revision()))
            .await,
        Err(Error::BackendUnavailable)
    );
    assert_head(&store, &baseline).await;
    assert!(
        store
            .authored_draft_revision(pending.draft_id(), pending.revision())
            .await
            .unwrap()
            .is_none()
    );

    for statement in [
        "DROP TRIGGER authored_commit_fault_trigger",
        "DROP TABLE authored_commit_fault",
        "DROP TABLE authored_commit_parent",
    ] {
        sqlx::query(statement).execute(store.pool()).await.unwrap();
    }
    let receipt = store
        .append_authored_draft(pending.clone(), Some(baseline.revision()))
        .await
        .unwrap();
    assert_eq!(receipt.disposition(), DraftAppendDisposition::Inserted);
    store.close().await.unwrap();
    let reopened = open(temp.path(), OpenMode::ReadWriteExisting).await;
    assert_head(&reopened, &pending).await;
    assert_eq!(
        reopened
            .append_authored_draft(pending, Some(baseline.revision()))
            .await
            .unwrap()
            .disposition(),
        DraftAppendDisposition::Replay
    );
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn authored_durability_sqlite_capacity_failure_preserves_the_acknowledged_head() {
    let temp = TempDir::new().unwrap();
    let store = open(temp.path(), OpenMode::Create).await;
    let baseline = first();
    store
        .append_authored_draft(baseline.clone(), None)
        .await
        .unwrap();
    let mut connections = Vec::new();
    for _ in 0..4 {
        connections.push(store.pool().acquire().await.unwrap());
    }
    let mut original_limits = Vec::new();
    for connection in &mut connections {
        original_limits.push(
            sqlx::query_scalar::<_, i64>("PRAGMA max_page_count")
                .fetch_one(&mut **connection)
                .await
                .unwrap(),
        );
        let pages = sqlx::query_scalar::<_, i64>("PRAGMA page_count")
            .fetch_one(&mut **connection)
            .await
            .unwrap();
        // PRAGMA assignments do not accept bind parameters; only an i64 is interpolated.
        let limit = sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(format!(
            "PRAGMA max_page_count = {}",
            pages + 2
        )))
        .fetch_one(&mut **connection)
        .await
        .unwrap();
        assert_eq!(limit, pages + 2);
    }
    drop(connections);
    // This bounded allocation proves SQLITE_FULL without filling the host disk.
    let failure =
        sqlx::query("CREATE TABLE authored_capacity_probe AS SELECT zeroblob(1048576) AS data")
            .execute(store.pool())
            .await
            .unwrap_err();
    assert_eq!(
        failure.as_database_error().unwrap().code().as_deref(),
        Some("13")
    );
    let pending = next(&baseline, vec![42; 256 * 1024]);
    assert_eq!(
        store
            .append_authored_draft(pending.clone(), Some(baseline.revision()))
            .await,
        Err(Error::BackendUnavailable)
    );
    assert_head(&store, &baseline).await;

    let mut connections = Vec::new();
    for _ in 0..4 {
        connections.push(store.pool().acquire().await.unwrap());
    }
    for (connection, limit) in connections.iter_mut().zip(original_limits) {
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "PRAGMA max_page_count = {limit}"
        )))
        .execute(&mut **connection)
        .await
        .unwrap();
    }
    drop(connections);
    store.close().await.unwrap();
    let reopened = open(temp.path(), OpenMode::ReadWriteExisting).await;
    assert_head(&reopened, &baseline).await;
    assert_eq!(
        reopened
            .append_authored_draft(pending.clone(), Some(baseline.revision()))
            .await
            .unwrap()
            .disposition(),
        DraftAppendDisposition::Inserted
    );
    assert_head(&reopened, &pending).await;
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn authored_durability_denied_writes_have_no_receipt_or_head_advance() {
    let temp = TempDir::new().unwrap();
    let store = open(temp.path(), OpenMode::Create).await;
    let baseline = first();
    let pending = next(&baseline, b"denied revision".to_vec());
    store
        .append_authored_draft(baseline.clone(), None)
        .await
        .unwrap();
    let mut connections = Vec::new();
    for _ in 0..4 {
        connections.push(store.pool().acquire().await.unwrap());
    }
    for connection in &mut connections {
        sqlx::query("PRAGMA query_only = ON")
            .execute(&mut **connection)
            .await
            .unwrap();
    }
    drop(connections);
    assert_eq!(
        store
            .append_authored_draft(pending, Some(baseline.revision()))
            .await,
        Err(Error::BackendUnavailable)
    );
    assert_head(&store, &baseline).await;
    store.close().await.unwrap();
    let reopened = open(temp.path(), OpenMode::ReadWriteExisting).await;
    assert_head(&reopened, &baseline).await;
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn authored_durability_busy_writer_never_returns_a_success_receipt() {
    let temp = TempDir::new().unwrap();
    let store = SqliteStorage::open(
        OpenOptions::new(
            Paths::from_directory(temp.path()).unwrap(),
            OpenMode::Create,
        )
        .with_source_generation(SourceGeneration::new([91; 32]).unwrap(), 9)
        .unwrap()
        .with_busy_timeout(std::time::Duration::from_millis(10))
        .unwrap(),
    )
    .await
    .unwrap();
    let baseline = first();
    let pending = next(&baseline, b"blocked complete revision".to_vec());
    store
        .append_authored_draft(baseline.clone(), None)
        .await
        .unwrap();
    let transaction = store.pool().begin_with("BEGIN IMMEDIATE").await.unwrap();
    assert_eq!(
        store
            .append_authored_draft(pending.clone(), Some(baseline.revision()))
            .await,
        Err(Error::BackendUnavailable)
    );
    assert_head(&store, &baseline).await;
    transaction.rollback().await.unwrap();
    assert_eq!(
        store
            .append_authored_draft(pending.clone(), Some(baseline.revision()))
            .await
            .unwrap()
            .disposition(),
        DraftAppendDisposition::Inserted
    );
    assert_head(&store, &pending).await;
    store.close().await.unwrap();
}

#[tokio::test]
async fn authored_durability_read_only_and_closed_stores_cannot_acknowledge() {
    let temp = TempDir::new().unwrap();
    let store = open(temp.path(), OpenMode::Create).await;
    let baseline = first();
    let pending = next(&baseline, b"not writable".to_vec());
    store
        .append_authored_draft(baseline.clone(), None)
        .await
        .unwrap();
    store.close().await.unwrap();
    assert_eq!(
        store
            .append_authored_draft(pending.clone(), Some(baseline.revision()))
            .await,
        Err(Error::BackendUnavailable)
    );
    let read_only = open(temp.path(), OpenMode::ReadOnly).await;
    assert_head(&read_only, &baseline).await;
    assert_eq!(
        read_only
            .append_authored_draft(pending, Some(baseline.revision()))
            .await,
        Err(Error::BackendUnavailable)
    );
    read_only.close().await.unwrap();
}

#[test]
fn authored_durability_contract_distinguishes_crash_and_power_loss() {
    let policy: toml::Value = toml::from_str(include_str!(
        "../../../contracts/storage/failure_injection_policy_v1.toml"
    ))
    .unwrap();
    let authored = &policy["authored_write"];
    assert_eq!(
        authored["acknowledgment"].as_str(),
        Some("only_after_successful_commit_or_exact_committed_replay")
    );
    assert_eq!(
        authored["commit_fault"].as_str(),
        Some("deferred_foreign_key_at_actual_commit")
    );
    assert_eq!(
        authored["capacity_fault"].as_str(),
        Some("bounded_sqlite_max_page_count")
    );
    assert_eq!(
        authored["write_denied_fault"].as_str(),
        Some("owned_connection_query_only")
    );
    assert_eq!(
        authored["busy_fault"].as_str(),
        Some("owned_begin_immediate_with_bounded_timeout")
    );
    assert_eq!(
        authored["process_termination_points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap())
            .collect::<Vec<_>>(),
        ["after_acknowledgment", "during_uncommitted_insert"]
    );
    assert_eq!(authored["power_loss_qualified"].as_bool(), Some(false));
    assert_eq!(
        authored["protected_data_policy_owner"].as_str(),
        Some("native_host")
    );
}
