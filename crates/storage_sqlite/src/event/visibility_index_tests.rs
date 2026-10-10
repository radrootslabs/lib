use super::tests::{observed, signed_event_with, unprepared_store as store, verified, visible};
use super::*;
use radroots_storage::event::VisibilityIndexBounds;

fn generation() -> SourceGeneration {
    SourceGeneration::new([81; 32]).unwrap()
}
fn bounds(rows: u16) -> VisibilityIndexBounds {
    VisibilityIndexBounds::new(rows, 16 * 1024 * 1024).unwrap()
}
fn profile(name: &str, time: u64) -> SignedEvent {
    signed_event_with(&format!(r#"{{"name":"{name}"}}"#), false, time, 0, vec![])
}
fn raw(event: SignedEvent) -> EventAdmission {
    EventAdmission::raw(observed(event, 100, None))
}
fn admitted(event: SignedEvent) -> EventAdmission {
    EventAdmission::visible(observed(event.clone(), 100, None), visible(&event)).unwrap()
}

async fn finish(storage: &SqliteStorage) {
    for _ in 0..100 {
        if storage
            .maintain_visibility(bounds(2))
            .await
            .unwrap()
            .is_complete()
        {
            return;
        }
    }
    panic!("bounded fixture index did not finish");
}

async fn indexed_visible(storage: &SqliteStorage) -> Vec<EventId> {
    let values: Vec<Vec<u8>> = sqlx::query_scalar(
        "SELECT candidate.event_id FROM radroots_runtime_visibility_events AS candidate
         WHERE candidate.source_generation = ? AND candidate.admission_stage = 'visible'
         AND candidate.ephemeral = 0
         AND (candidate.head_coordinate IS NULL OR candidate.event_id = (
           SELECT head.event_id FROM radroots_runtime_visibility_events AS head
           WHERE head.source_generation = candidate.source_generation
           AND head.head_coordinate = candidate.head_coordinate
           ORDER BY head.created_at DESC, head.event_id ASC LIMIT 1))
         AND (candidate.deletion_request = 1 OR (
           NOT EXISTS (SELECT 1 FROM radroots_runtime_visibility_deletion_events AS deletion
             WHERE deletion.source_generation = candidate.source_generation
             AND deletion.target_event = candidate.event_id AND deletion.author = candidate.author)
           AND NOT EXISTS (SELECT 1 FROM radroots_runtime_visibility_deletion_addresses AS deletion
             WHERE deletion.source_generation = candidate.source_generation
             AND deletion.target_coordinate = candidate.deletion_coordinate
             AND deletion.author = candidate.author AND deletion.created_at >= candidate.created_at)))
         ORDER BY candidate.event_id",
    ).bind(storage.generation.as_bytes().as_slice()).fetch_all(&storage.pool).await.unwrap();
    let ids = values
        .into_iter()
        .map(|value| EventId::from_bytes(value.try_into().unwrap()))
        .collect::<Vec<_>>();
    let mut public_ids = storage
        .query_visible(EventQuery::all(EventQueryBounds::first(1000).unwrap()))
        .await
        .unwrap()
        .items()
        .iter()
        .map(|item| *item.event().id())
        .collect::<Vec<_>>();
    public_ids.sort();
    assert_eq!(public_ids, ids);
    ids
}

#[tokio::test]
async fn visibility_index_backfill_and_changed_replay_survive_provider_reopen_without_restart() {
    let storage = store(generation()).await;
    let first = profile("first", 10);
    for event in [first.clone(), profile("second", 20), profile("third", 30)] {
        storage.admit(raw(event)).await.unwrap();
    }
    let before = storage.maintain_visibility(bounds(1)).await.unwrap();
    assert_eq!(before.source_sequence(), 1);
    assert_eq!(before.processed_rows(), 1);
    assert!(!before.is_complete());
    storage.admit(admitted(first.clone())).await.unwrap();
    storage.admit(raw(profile("fourth", 40))).await.unwrap();
    let reopened = SqliteStorage::new(
        storage.pool.clone(),
        generation(),
        EventStoreMode::ReadWrite,
    );
    let after = reopened.maintain_visibility(bounds(1)).await.unwrap();
    assert_eq!(after.source_sequence(), 2);
    assert!(!after.is_complete());
    let third = reopened.maintain_visibility(bounds(1)).await.unwrap();
    assert_eq!(third.source_sequence(), 3);
    assert!(!third.is_complete());
    finish(&reopened).await;
    let token = reopened.source_token().await.unwrap();
    let current = reopened.maintain_visibility(bounds(1)).await.unwrap();
    assert!(current.is_complete());
    assert_eq!(current.processed_rows(), 0);
    assert_eq!(current.source_sequence(), 3);
    assert_eq!(current.token(), token);
    let row = sqlx::query("SELECT admission_stage, last_mutation_epoch FROM radroots_runtime_visibility_events WHERE event_id = ?")
        .bind(first.id().as_bytes().as_slice()).fetch_one(&reopened.pool).await.unwrap();
    assert_eq!(row.get::<String, _>("admission_stage"), "visible");
    assert_eq!(row.get::<i64, _>("last_mutation_epoch"), 4);
    assert_eq!(indexed_visible(&reopened).await, vec![*first.id()]);
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM radroots_runtime_visibility_events")
        .fetch_one(&reopened.pool)
        .await
        .unwrap();
    assert_eq!(count, 4);
}

#[tokio::test]
async fn visibility_index_matches_reference_winners_deletion_before_target_and_unsupported_head() {
    let storage = store(generation()).await;
    let old = profile("old", 10);
    let current = profile("current", 20);
    let deletion = signed_event_with(
        "delete",
        false,
        30,
        5,
        vec![vec!["e".to_owned(), current.id().to_hex()]],
    );
    for event in [deletion, old, current] {
        storage.admit(admitted(event)).await.unwrap();
    }
    finish(&storage).await;
    assert_eq!(
        indexed_visible(&storage).await,
        storage
            .rebuild_visibility()
            .await
            .unwrap()
            .visible_event_ids()
    );
    let newer = profile("unsupported", 40);
    storage
        .admit(
            EventAdmission::verified(observed(newer.clone(), 100, None), verified(&newer)).unwrap(),
        )
        .await
        .unwrap();
    assert!(
        sqlx::query_scalar::<_, bool>("SELECT ready FROM radroots_runtime_visibility_progress")
            .fetch_one(&storage.pool)
            .await
            .unwrap()
    );
    finish(&storage).await;
    assert_eq!(
        indexed_visible(&storage).await,
        storage
            .rebuild_visibility()
            .await
            .unwrap()
            .visible_event_ids()
    );
    let same_a = profile("same-a", 50);
    let same_b = profile("same-b", 50);
    storage.admit(admitted(same_a)).await.unwrap();
    storage.admit(admitted(same_b)).await.unwrap();
    storage
        .admit(raw(profile("forged raw later", 60)))
        .await
        .unwrap();
    let address_deletion = signed_event_with(
        "address delete",
        false,
        55,
        5,
        vec![vec![
            "a".to_owned(),
            "0:585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df:".to_owned(),
        ]],
    );
    storage.admit(admitted(address_deletion)).await.unwrap();
    finish(&storage).await;
    assert_eq!(
        indexed_visible(&storage).await,
        storage
            .rebuild_visibility()
            .await
            .unwrap()
            .visible_event_ids()
    );
}

#[tokio::test]
async fn visibility_index_bounds_charge_before_payload_decode_and_commit_partial_progress() {
    let storage = store(generation()).await;
    let first = profile("first", 10);
    let size = first.raw_json().len() as u64;
    storage.admit(raw(first)).await.unwrap();
    storage
        .admit(raw(profile(&"x".repeat(1000), 20)))
        .await
        .unwrap();
    let one = 4096 + 2 * 128 + VisibilityIndexBounds::charge(size).unwrap();
    storage.work.take();
    let progress = storage
        .maintain_visibility(VisibilityIndexBounds::new(2, one).unwrap())
        .await
        .unwrap();
    assert_eq!(progress.charged_bytes(), one);
    assert_eq!(progress.processed_rows(), 1);
    assert_eq!(storage.work.take().codec_inputs, 1);
    assert!(!progress.is_complete());
    assert_eq!(
        storage
            .maintain_visibility(VisibilityIndexBounds::new(2, one).unwrap())
            .await,
        Err(Error::SpaceInsufficient)
    );
    assert_eq!(storage.work.take().codec_inputs, 0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT backfill_sequence FROM radroots_runtime_visibility_progress"
        )
        .fetch_one(&storage.pool)
        .await
        .unwrap(),
        1
    );
    finish(&storage).await;
    storage.work.take();
    assert!(
        storage
            .maintain_visibility(bounds(64))
            .await
            .unwrap()
            .is_complete()
    );
    assert_eq!(storage.work.take().codec_inputs, 0);
}

#[tokio::test]
async fn visibility_index_fault_rolls_back_all_metadata_and_progress_then_resumes() {
    let storage = store(generation()).await;
    storage.admit(raw(profile("first", 10))).await.unwrap();
    sqlx::raw_sql("CREATE TEMP TRIGGER visibility_test_fault AFTER INSERT ON radroots_runtime_visibility_events BEGIN SELECT RAISE(ABORT, 'index test fault'); END;")
        .execute(&storage.pool).await.unwrap();
    assert_eq!(
        storage.maintain_visibility(bounds(1)).await,
        Err(Error::BackendUnavailable)
    );
    for query in [
        "SELECT COUNT(*) FROM radroots_runtime_visibility_progress",
        "SELECT COUNT(*) FROM radroots_runtime_visibility_events",
    ] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(query)
                .fetch_one(&storage.pool)
                .await
                .unwrap(),
            0
        );
    }
    sqlx::query("DROP TRIGGER visibility_test_fault")
        .execute(&storage.pool)
        .await
        .unwrap();
    assert!(
        storage
            .maintain_visibility(bounds(1))
            .await
            .unwrap()
            .is_complete()
    );
}

#[tokio::test]
async fn visibility_index_direct_writer_invalidates_atomically_and_rollback_preserves_ready_state()
{
    let storage = store(generation()).await;
    let event = profile("first", 10);
    storage.admit(raw(event)).await.unwrap();
    finish(&storage).await;
    let mut transaction = storage.pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
    sqlx::query("UPDATE radroots_runtime_events SET admission_stage = 'verified'")
        .execute(&mut *transaction)
        .await
        .unwrap();
    let dirty: bool = sqlx::query_scalar("SELECT ready FROM radroots_runtime_visibility_progress")
        .fetch_one(&mut *transaction)
        .await
        .unwrap();
    assert!(!dirty);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT last_mutation_epoch FROM radroots_runtime_events")
            .fetch_one(&mut *transaction)
            .await
            .unwrap(),
        2
    );
    transaction.rollback().await.unwrap();
    assert!(
        sqlx::query_scalar::<_, bool>("SELECT ready FROM radroots_runtime_visibility_progress")
            .fetch_one(&storage.pool)
            .await
            .unwrap()
    );
    sqlx::query("UPDATE radroots_runtime_events SET admission_stage = 'verified'")
        .execute(&storage.pool)
        .await
        .unwrap();
    assert!(
        !sqlx::query_scalar::<_, bool>("SELECT ready FROM radroots_runtime_visibility_progress")
            .fetch_one(&storage.pool)
            .await
            .unwrap()
    );
    let current = storage.maintain_visibility(bounds(1)).await.unwrap();
    assert!(current.is_complete());
    assert_eq!(current.processed_rows(), 1);
    sqlx::query("UPDATE radroots_runtime_events SET admission_stage = admission_stage")
        .execute(&storage.pool)
        .await
        .unwrap();
    assert_eq!(
        storage
            .maintain_visibility(bounds(1))
            .await
            .unwrap()
            .processed_rows(),
        0
    );
}

#[tokio::test]
async fn visibility_index_corruption_and_retired_or_readonly_sources_refuse_completion() {
    let storage = store(generation()).await;
    storage.admit(raw(profile("first", 10))).await.unwrap();
    sqlx::query("DROP TRIGGER radroots_runtime_events_raw_update_guard")
        .execute(&storage.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE radroots_runtime_events SET signed_event = X'7b'")
        .execute(&storage.pool)
        .await
        .unwrap();
    assert_eq!(
        storage.maintain_visibility(bounds(1)).await,
        Err(Error::CorruptStoredEvent)
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM radroots_runtime_visibility_progress")
            .fetch_one(&storage.pool)
            .await
            .unwrap(),
        0
    );
    let readonly = SqliteStorage::new(storage.pool.clone(), generation(), EventStoreMode::ReadOnly);
    assert_eq!(
        readonly.maintain_visibility(bounds(1)).await,
        Err(Error::BackendUnavailable)
    );
    sqlx::query(
        "UPDATE radroots_runtime_source_generations SET state = 'retired', retired_at_unix_ms = 2",
    )
    .execute(&storage.pool)
    .await
    .unwrap();
    assert_eq!(
        storage.maintain_visibility(bounds(1)).await,
        Err(Error::SourceGenerationChanged)
    );
}

#[tokio::test]
async fn visibility_index_empty_source_is_complete_and_last_change_replay_uses_index_navigation() {
    let storage = store(generation()).await;
    let empty = storage.maintain_visibility(bounds(1)).await.unwrap();
    assert!(empty.is_complete());
    assert_eq!(empty.token().epoch(), 0);
    storage
        .admit(raw(profile("first", u64::MAX)))
        .await
        .unwrap();
    let progress = storage.maintain_visibility(bounds(1)).await.unwrap();
    assert!(progress.is_complete());
    assert_eq!(progress.source_sequence(), 0);
    let time: Vec<u8> =
        sqlx::query_scalar("SELECT created_at FROM radroots_runtime_visibility_events")
            .fetch_one(&storage.pool)
            .await
            .unwrap();
    assert_eq!(time, u64::MAX.to_be_bytes());
    let plan = sqlx::query("EXPLAIN QUERY PLAN SELECT source_sequence, last_mutation_epoch, length(signed_event) FROM radroots_runtime_events WHERE source_generation = ? AND (last_mutation_epoch, source_sequence) > (?, ?) ORDER BY last_mutation_epoch, source_sequence LIMIT ?")
        .bind(generation().as_bytes().as_slice()).bind(0i64).bind(0i64).bind(1i64)
        .fetch_all(&storage.pool).await.unwrap();
    assert!(plan.iter().any(|row| {
        row.get::<String, _>("detail")
            .contains("radroots_runtime_events_mutation_idx")
    }));
}

#[tokio::test]
async fn visibility_index_real_database_close_and_reopen_preserves_bounded_progress() {
    let directory = tempfile::tempdir().unwrap();
    let paths = crate::Paths::from_directory(directory.path()).unwrap();
    let storage = SqliteStorage::open(
        crate::OpenOptions::new(paths.clone(), crate::OpenMode::Create)
            .with_source_generation(generation(), 100)
            .unwrap(),
    )
    .await
    .unwrap();
    // Retain a real file-backed unprepared-source fixture; fresh empty sources
    // otherwise initialize a ready index under forward20.
    sqlx::query("DELETE FROM radroots_runtime_visibility_progress WHERE source_generation = ?")
        .bind(generation().as_bytes().as_slice())
        .execute(&storage.pool)
        .await
        .unwrap();
    for event in [profile("first", 10), profile("second", 20)] {
        storage.admit(admitted(event)).await.unwrap();
    }
    let first = storage.maintain_visibility(bounds(1)).await.unwrap();
    assert_eq!(first.source_sequence(), 1);
    assert!(!first.is_complete());
    storage.close().await.unwrap();
    assert_eq!(
        storage.maintain_visibility(bounds(1)).await,
        Err(Error::BackendUnavailable)
    );
    let reopened = SqliteStorage::open(crate::OpenOptions::new(
        paths,
        crate::OpenMode::ReadWriteExisting,
    ))
    .await
    .unwrap();
    let next = reopened.maintain_visibility(bounds(1)).await.unwrap();
    assert_eq!(next.source_sequence(), 2);
    assert_eq!(next.processed_rows(), 1);
    assert!(next.is_complete());
    assert_eq!(
        indexed_visible(&reopened).await,
        reopened
            .rebuild_visibility()
            .await
            .unwrap()
            .visible_event_ids()
    );
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn visibility_index_forward_migration_retains_legacy_truth_and_refuses_corrupt_payloads() {
    use crate::migration::runtime::{MIGRATIONS, migration_sql};
    use sqlx::sqlite::SqlitePoolOptions;
    for state in [0, 1, 2] {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        for migration in &MIGRATIONS[..18] {
            sqlx::raw_sql(migration_sql(migration.version()).unwrap())
                .execute(&pool)
                .await
                .unwrap();
        }
        sqlx::query("INSERT INTO radroots_runtime_source_generations (generation, sequence_head, state, created_at_unix_ms) VALUES (?, 0, 'active', 1)")
            .bind(generation().as_bytes().as_slice()).execute(&pool).await.unwrap();
        let storage = SqliteStorage::new(pool.clone(), generation(), EventStoreMode::ReadWrite);
        let event = profile("legacy", 10);
        if state != 0 {
            sqlx::query("UPDATE radroots_runtime_source_generations SET sequence_head = 1 WHERE generation = ?")
                .bind(generation().as_bytes().as_slice()).execute(&pool).await.unwrap();
            sqlx::query(
                "INSERT INTO radroots_runtime_events (source_generation, source_sequence, event_id,
                admission_stage, signed_event, admitted_at_unix_ms, updated_at_unix_ms)
                VALUES (?, 1, ?, 'raw', ?, 100, 100)",
            )
            .bind(generation().as_bytes().as_slice())
            .bind(event.id().as_bytes().as_slice())
            .bind(event.raw_json().as_bytes())
            .execute(&pool)
            .await
            .unwrap();
            if state == 2 {
                sqlx::query("DROP TRIGGER radroots_runtime_events_raw_update_guard")
                    .execute(&pool)
                    .await
                    .unwrap();
                sqlx::query("UPDATE radroots_runtime_events SET signed_event = X'7b'")
                    .execute(&pool)
                    .await
                    .unwrap();
            }
        }
        let token = storage.source_token().await.unwrap();
        assert_eq!(
            storage.maintain_visibility(bounds(1)).await,
            Err(Error::BackendUnavailable)
        );
        storage.work.take();
        sqlx::raw_sql(migration_sql(19).unwrap())
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(migration_sql(20).unwrap())
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(storage.source_token().await.unwrap(), token);
        assert_eq!(storage.work.take().codec_inputs, 0);
        if state == 2 {
            assert_eq!(
                storage.maintain_visibility(bounds(1)).await,
                Err(Error::CorruptStoredEvent)
            );
        } else {
            let result = storage.maintain_visibility(bounds(1)).await.unwrap();
            assert!(result.is_complete());
            assert_eq!(result.processed_rows(), u16::from(state == 1));
            if state == 1 {
                assert_eq!(
                    sqlx::query_scalar::<_, Vec<u8>>(
                        "SELECT signed_event FROM radroots_runtime_events"
                    )
                    .fetch_one(&pool)
                    .await
                    .unwrap(),
                    event.raw_json().as_bytes()
                );
            }
        }
    }
}

#[tokio::test]
async fn visibility_index_cancellation_before_admission_leaves_no_progress_and_allows_retry() {
    let storage = store(generation()).await;
    storage.admit(raw(profile("first", 10))).await.unwrap();
    let held = storage.pool.acquire().await.unwrap();
    let mut future = storage.maintain_visibility(bounds(1));
    std::future::poll_fn(|context| {
        assert!(future.as_mut().poll(context).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
    drop(future);
    drop(held);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM radroots_runtime_visibility_progress")
            .fetch_one(&storage.pool)
            .await
            .unwrap(),
        0
    );
    assert!(
        storage
            .maintain_visibility(bounds(1))
            .await
            .unwrap()
            .is_complete()
    );
}

#[tokio::test]
async fn visibility_index_malformed_progress_or_missing_source_tail_cannot_claim_complete() {
    let storage = store(generation()).await;
    storage.admit(raw(profile("first", 10))).await.unwrap();
    finish(&storage).await;
    sqlx::query("PRAGMA ignore_check_constraints = ON")
        .execute(&storage.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE radroots_runtime_visibility_progress SET schema_version = 2")
        .execute(&storage.pool)
        .await
        .unwrap();
    assert_eq!(
        storage.maintain_visibility(bounds(1)).await,
        Err(Error::CorruptStoredEvent)
    );
    sqlx::query("UPDATE radroots_runtime_visibility_progress SET schema_version = 1")
        .execute(&storage.pool)
        .await
        .unwrap();
    // Clear only derived membership before deliberately restarting this fixture.
    for table in [
        "radroots_runtime_visibility_heads",
        "radroots_runtime_visibility_members",
        "radroots_runtime_visibility_pending",
    ] {
        let sql = match table {
            "radroots_runtime_visibility_heads" => "DELETE FROM radroots_runtime_visibility_heads",
            "radroots_runtime_visibility_members" => {
                "DELETE FROM radroots_runtime_visibility_members"
            }
            _ => "DELETE FROM radroots_runtime_visibility_pending",
        };
        sqlx::query(sql).execute(&storage.pool).await.unwrap();
    }
    sqlx::query("DELETE FROM radroots_runtime_visibility_progress")
        .execute(&storage.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE radroots_runtime_source_generations SET sequence_head = 2")
        .execute(&storage.pool)
        .await
        .unwrap();
    assert!(
        !storage
            .maintain_visibility(bounds(1))
            .await
            .unwrap()
            .is_complete()
    );
    assert_eq!(
        storage.maintain_visibility(bounds(1)).await,
        Err(Error::CorruptStoredEvent)
    );
}

#[tokio::test]
async fn visibility_index_memory_and_sqlite_progress_agree_across_same_count_promotion() {
    let storage = super::tests::store(generation()).await;
    let memory = radroots_storage::memory::MemoryStorage::new(generation());
    let first = profile("first", 10);
    let initial = [raw(first.clone()), raw(profile("large", 20))];
    for admission in initial {
        storage.admit(admission.clone()).await.unwrap();
        memory.admit(admission).await.unwrap();
    }
    for budget in [
        VisibilityIndexBounds::new(
            2,
            4096 + 2 * 128 + VisibilityIndexBounds::charge(first.raw_json().len() as u64).unwrap(),
        )
        .unwrap(),
        bounds(1),
    ] {
        assert_eq!(
            storage.maintain_visibility(budget).await.unwrap(),
            memory.maintain_visibility(budget).await.unwrap()
        );
    }
    for admission in [admitted(first.clone()), admitted(profile("later", 30))] {
        storage.admit(admission.clone()).await.unwrap();
        memory.admit(admission).await.unwrap();
    }
    for _ in 0..3 {
        let sql = storage.maintain_visibility(bounds(1)).await.unwrap();
        let reference = memory.maintain_visibility(bounds(1)).await.unwrap();
        assert_eq!(sql, reference);
        if sql.is_complete() {
            break;
        }
    }
    assert!(
        storage
            .maintain_visibility(bounds(1))
            .await
            .unwrap()
            .is_complete()
    );
    assert_eq!(
        storage.rebuild_visibility().await.unwrap(),
        memory.rebuild_visibility().await.unwrap()
    );
    assert_eq!(
        indexed_visible(&storage).await,
        memory
            .rebuild_visibility()
            .await
            .unwrap()
            .visible_event_ids()
    );
}
