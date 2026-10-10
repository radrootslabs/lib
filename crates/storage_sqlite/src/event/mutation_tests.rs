use super::tests::{observed, signed_event_with, store, verified, visible};
use super::*;
use radroots_storage::atomic::{
    AtomicCommit, AtomicCommitDigest, AtomicCommitDisposition, AtomicCommitId, AtomicStorage,
    AtomicWorkflow, CommitIngested,
};

fn event(content: &str) -> SignedEvent {
    signed_event_with(content, false, 1_800_000_100, 0, vec![])
}

fn generation() -> SourceGeneration {
    SourceGeneration::new([77; 32]).unwrap()
}

fn atomic_request(event: SignedEvent, id: u8) -> AtomicCommit {
    AtomicCommit::new(
        AtomicCommitId::new([id; 16]).unwrap(),
        AtomicCommitDigest::new([id; 32]),
        100,
        AtomicWorkflow::Ingested(Box::new(CommitIngested::new(
            EventAdmission::raw(observed(event, 100, None)),
            None,
        ))),
    )
    .unwrap()
}

#[tokio::test]
async fn epoch_tracks_semantic_changes_without_duplicate_or_provenance_churn() {
    let storage = store(generation()).await;
    assert_eq!(
        storage.source_token().await.unwrap(),
        EventSourceToken::new(generation(), 0)
    );
    let event = event(r#"{"name":"Farm"}"#);
    let receipt = storage
        .admit(EventAdmission::raw(observed(event.clone(), 10, None)))
        .await
        .unwrap();
    assert_eq!(storage.source_token().await.unwrap().epoch(), 1);
    for at in [10, 20] {
        assert_eq!(
            storage
                .admit(EventAdmission::raw(observed(event.clone(), at, None)))
                .await
                .unwrap()
                .disposition(),
            AdmissionDisposition::Duplicate
        );
        assert_eq!(storage.source_token().await.unwrap().epoch(), 1);
    }
    let advanced = storage
        .admit(
            EventAdmission::verified(observed(event.clone(), 30, None), verified(&event)).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(advanced.position(), receipt.position());
    assert_eq!(storage.source_token().await.unwrap().epoch(), 2);
    storage
        .admit(EventAdmission::visible(observed(event.clone(), 40, None), visible(&event)).unwrap())
        .await
        .unwrap();
    assert_eq!(storage.source_token().await.unwrap().epoch(), 3);
    storage
        .admit(EventAdmission::visible(observed(event.clone(), 50, None), visible(&event)).unwrap())
        .await
        .unwrap();
    assert_eq!(storage.source_token().await.unwrap().epoch(), 3);
    assert_eq!(storage.status().await.unwrap().raw_events(), 1);
    assert_eq!(
        storage
            .admit(EventAdmission::raw(observed(event, 60, None)))
            .await,
        Err(Error::AdmissionRegression)
    );
    assert_eq!(storage.source_token().await.unwrap().epoch(), 3);
}

#[tokio::test]
async fn sqlite_epoch_maximum_allows_replay_but_rolls_back_insert_and_promotion() {
    let storage = store(generation()).await;
    sqlx::query("UPDATE radroots_runtime_source_generations SET mutation_epoch = ?")
        .bind(i64::MAX - 1)
        .execute(&storage.pool)
        .await
        .unwrap();
    let original = event(r#"{"name":"Original"}"#);
    storage
        .admit(EventAdmission::raw(observed(original.clone(), 10, None)))
        .await
        .unwrap();
    let maximum = EventSourceToken::new(generation(), i64::MAX as u64);
    assert_eq!(storage.source_token().await.unwrap(), maximum);
    storage
        .admit(EventAdmission::raw(observed(original.clone(), 20, None)))
        .await
        .unwrap();
    let before_provenance = storage
        .query_provenance(*original.id(), EventQueryBounds::first(10).unwrap())
        .await
        .unwrap();
    assert_eq!(
        storage
            .admit(
                EventAdmission::visible(observed(original.clone(), 30, None), visible(&original))
                    .unwrap()
            )
            .await,
        Err(Error::EventMutationOverflow)
    );
    assert_eq!(
        storage
            .admit(EventAdmission::raw(observed(
                event(r#"{"name":"New"}"#),
                40,
                None
            )))
            .await,
        Err(Error::EventMutationOverflow)
    );
    assert_eq!(storage.source_token().await.unwrap(), maximum);
    let sequence: i64 =
        sqlx::query_scalar("SELECT sequence_head FROM radroots_runtime_source_generations")
            .fetch_one(&storage.pool)
            .await
            .unwrap();
    assert_eq!(sequence, 1);
    assert_eq!(
        storage
            .query_provenance(*original.id(), EventQueryBounds::first(10).unwrap())
            .await
            .unwrap(),
        before_provenance
    );
    let status = storage.status().await.unwrap();
    assert_eq!(status.raw_events(), 1);
    assert_eq!(status.verified_events(), 0);
    let error = Error::EventMutationOverflow;
    assert_eq!(error.to_string(), "storage event mutation epoch overflowed");
}

#[tokio::test]
async fn actual_sql_writer_and_same_value_updates_obey_the_epoch_trigger() {
    let storage = store(generation()).await;
    let event = event(r#"{"name":"Farm"}"#);
    storage
        .admit(EventAdmission::raw(observed(event, 10, None)))
        .await
        .unwrap();
    sqlx::query("UPDATE radroots_runtime_events SET admission_stage = 'verified'")
        .execute(&storage.pool)
        .await
        .unwrap();
    assert_eq!(storage.source_token().await.unwrap().epoch(), 2);
    sqlx::query("UPDATE radroots_runtime_events SET admission_stage = 'visible', admitted_contract_id = 'radroots.profile.metadata.v1', admitted_registry_version = 7").execute(&storage.pool).await.unwrap();
    assert_eq!(storage.source_token().await.unwrap().epoch(), 3);
    sqlx::query("UPDATE radroots_runtime_events SET admission_stage = admission_stage, admitted_contract_id = admitted_contract_id, admitted_registry_version = admitted_registry_version, updated_at_unix_ms = 20").execute(&storage.pool).await.unwrap();
    assert_eq!(storage.source_token().await.unwrap().epoch(), 3);
    assert!(
        sqlx::query("UPDATE radroots_runtime_source_generations SET mutation_epoch = 1")
            .execute(&storage.pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("DELETE FROM radroots_runtime_events")
            .execute(&storage.pool)
            .await
            .is_err()
    );
    assert_eq!(storage.source_token().await.unwrap().epoch(), 3);
}

#[tokio::test]
async fn dropped_transaction_rolls_back_event_sequence_and_epoch_together() {
    let storage = store(generation()).await;
    let mut transaction = storage.pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
    storage
        .admit_transaction(
            &mut transaction,
            EventAdmission::raw(observed(event("{}"), 10, None)),
        )
        .await
        .unwrap();
    let epoch: i64 =
        sqlx::query_scalar("SELECT mutation_epoch FROM radroots_runtime_source_generations")
            .fetch_one(&mut *transaction)
            .await
            .unwrap();
    assert_eq!(epoch, 1);
    drop(transaction);
    assert_eq!(storage.source_token().await.unwrap().epoch(), 0);
    assert_eq!(storage.status().await.unwrap().raw_events(), 0);
    let sequence: i64 =
        sqlx::query_scalar("SELECT sequence_head FROM radroots_runtime_source_generations")
            .fetch_one(&storage.pool)
            .await
            .unwrap();
    assert_eq!(sequence, 0);
}

#[tokio::test]
async fn lost_atomic_receipt_replay_does_not_advance_the_committed_epoch() {
    let storage = store(generation()).await;
    let request = atomic_request(event("{}"), 1);
    let commit_id = request.commit_id();
    drop(storage.commit(request.clone()).await.unwrap());
    let token = storage.source_token().await.unwrap();
    assert_eq!(token.epoch(), 1);
    assert!(storage.receipt(commit_id).await.unwrap().is_some());
    assert_eq!(
        storage.commit(request).await.unwrap().disposition(),
        AtomicCommitDisposition::Replay
    );
    assert_eq!(storage.source_token().await.unwrap(), token);
}

#[tokio::test]
async fn failure_after_event_write_rolls_back_epoch_and_atomic_receipt() {
    let storage = store(generation()).await;
    sqlx::raw_sql("CREATE TEMP TRIGGER test_receipt_fault BEFORE INSERT ON radroots_runtime_atomic_commits BEGIN SELECT RAISE(ABORT, 'injected receipt failure'); END;")
        .execute(&storage.pool).await.unwrap();
    let request = atomic_request(event("{}"), 2);
    let id = request.commit_id();
    assert!(storage.commit(request.clone()).await.is_err());
    assert_eq!(storage.source_token().await.unwrap().epoch(), 0);
    assert_eq!(storage.status().await.unwrap().raw_events(), 0);
    assert_eq!(storage.receipt(id).await.unwrap(), None);
    sqlx::query("DROP TRIGGER test_receipt_fault")
        .execute(&storage.pool)
        .await
        .unwrap();
    storage.commit(request).await.unwrap();
    assert_eq!(storage.source_token().await.unwrap().epoch(), 1);
}

#[tokio::test]
async fn token_read_never_decodes_event_history_and_refuses_retired_sources() {
    let storage = store(generation()).await;
    storage
        .admit(EventAdmission::raw(observed(event("{}"), 10, None)))
        .await
        .unwrap();
    storage.work.take();
    sqlx::query("DROP TRIGGER radroots_runtime_events_raw_update_guard")
        .execute(&storage.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE radroots_runtime_events SET signed_event = X'7b'")
        .execute(&storage.pool)
        .await
        .unwrap();
    assert_eq!(storage.source_token().await.unwrap().epoch(), 1);
    let counts = storage.work.take();
    assert_eq!(counts.event_rows_returned, 0);
    assert_eq!(counts.codec_inputs, 0);
    assert!(
        storage
            .query_raw(EventQuery::all(EventQueryBounds::first(1).unwrap()))
            .await
            .is_err()
    );
    sqlx::query(
        "UPDATE radroots_runtime_source_generations SET state = 'retired', retired_at_unix_ms = 20",
    )
    .execute(&storage.pool)
    .await
    .unwrap();
    assert_eq!(
        storage.source_token().await,
        Err(Error::SourceGenerationChanged)
    );
}

#[tokio::test]
async fn forward_migration_bootstraps_retained_and_empty_sources_without_decoding_history() {
    use crate::migration::runtime::{MIGRATIONS, migration_sql};
    use sqlx::sqlite::SqlitePoolOptions;
    for retained in [false, true] {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        for migration in &MIGRATIONS[..17] {
            sqlx::raw_sql(migration_sql(migration.version()).unwrap())
                .execute(&pool)
                .await
                .unwrap();
        }
        sqlx::query("INSERT INTO radroots_runtime_source_generations (generation, sequence_head, state, created_at_unix_ms) VALUES (?, 0, 'active', 1)").bind(generation().as_bytes().as_slice()).execute(&pool).await.unwrap();
        if retained {
            let event = event("{}");
            sqlx::query("UPDATE radroots_runtime_source_generations SET sequence_head = 1")
                .execute(&pool)
                .await
                .unwrap();
            sqlx::query("INSERT INTO radroots_runtime_events (source_generation, source_sequence, event_id, admission_stage, signed_event, admitted_at_unix_ms, updated_at_unix_ms) VALUES (?, 1, ?, 'raw', X'7b', 10, 10)")
                .bind(generation().as_bytes().as_slice()).bind(event.id().as_bytes().as_slice()).execute(&pool).await.unwrap();
        }
        let adapter = SqliteStorage::new(pool.clone(), generation(), EventStoreMode::ReadWrite);
        assert_eq!(adapter.source_token().await, Err(Error::BackendUnavailable));
        sqlx::raw_sql(migration_sql(18).unwrap())
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(
            adapter.source_token().await.unwrap().epoch(),
            u64::from(retained)
        );
        let reopened = SqliteStorage::new(pool, generation(), EventStoreMode::ReadOnly);
        assert_eq!(
            reopened.source_token().await.unwrap(),
            adapter.source_token().await.unwrap()
        );
        if retained {
            assert!(
                reopened
                    .query_raw(EventQuery::all(EventQueryBounds::first(1).unwrap()))
                    .await
                    .is_err()
            );
        }
    }
}
