use super::*;
use radroots_storage::{event::VisibilityIndexBounds, memory::MemoryStorage};

fn query(limit: u16) -> EventQuery {
    EventQuery::all(EventQueryBounds::first(limit).unwrap())
}

fn admission(event: SignedEvent) -> EventAdmission {
    EventAdmission::visible(
        tests::observed(event.clone(), 10, None),
        tests::visible(&event),
    )
    .unwrap()
}

async fn finish(storage: &SqliteStorage) {
    for _ in 0..1000 {
        let progress = storage
            .maintain_visibility(VisibilityIndexBounds::new(64, 16 * 1024 * 1024).unwrap())
            .await
            .unwrap();
        assert!(progress.processed_rows() + progress.processed_keys() <= 64);
        assert!(progress.charged_bytes() <= 16 * 1024 * 1024);
        if progress.is_complete() {
            return;
        }
    }
    panic!("bounded fixture maintenance failed to complete");
}

#[tokio::test]
async fn indexed_visibility_refuses_stale_fences_without_full_source_decode() {
    let generation = SourceGeneration::new([91; 32]).unwrap();
    let storage = tests::store(generation).await;
    let token = storage.source_token().await.unwrap();
    let query = EventQuery::all(EventQueryBounds::first(5).unwrap());
    assert!(
        storage
            .query_visible_at(query.clone(), token)
            .await
            .unwrap()
            .items()
            .is_empty()
    );
    let event = tests::signed_event_with("retained", false, 10, 1, vec![]);
    let admission = EventAdmission::visible(
        tests::observed(event.clone(), 10, None),
        tests::visible(&event),
    )
    .unwrap();
    storage.admit(admission).await.unwrap();
    storage.work.take();
    assert_eq!(
        storage.query_visible_at(query.clone(), token).await,
        Err(Error::SourceEpochChanged)
    );
    let refused = storage.work.take();
    assert_eq!(refused.codec_inputs, 0);
    assert_eq!(refused.materialization_requests, 0);
    let current = storage.source_token().await.unwrap();
    assert_eq!(
        storage
            .query_visible_at(query, current)
            .await
            .unwrap()
            .items()
            .len(),
        1
    );
    let page = storage.work.take();
    assert_eq!(page.codec_inputs, 1);
    assert_eq!(page.visibility_inputs, 0);
    assert_eq!(page.materialization_requests, 0);
    assert_eq!(storage.status().await.unwrap().visible_events(), 1);
    assert_eq!(
        storage
            .query_visible_at(
                EventQuery::all(EventQueryBounds::first(5).unwrap()),
                EventSourceToken::new(SourceGeneration::new([90; 32]).unwrap(), 1)
            )
            .await,
        Err(Error::SourceEpochChanged)
    );
}

#[tokio::test]
async fn indexed_visibility_tiny_key_budget_preserves_pending_work_and_cannot_claim_false_ready() {
    let generation = SourceGeneration::new([98; 32]).unwrap();
    let storage = tests::store(generation).await;
    let memory = MemoryStorage::new(generation);
    let mut tags = Vec::new();
    for index in 0..65 {
        let event = tests::signed_event_with(&format!("target {index}"), false, index, 1, vec![]);
        tags.push(vec!["e".to_owned(), event.id().to_hex()]);
        let value = admission(event);
        storage.admit(value.clone()).await.unwrap();
        memory.admit(value).await.unwrap();
    }
    let deletion = admission(tests::signed_event_with("delete", false, 100, 5, tags));
    storage.admit(deletion.clone()).await.unwrap();
    memory.admit(deletion).await.unwrap();
    let tiny = VisibilityIndexBounds::new(1, 4096 + 128).unwrap();
    assert_eq!(
        storage.maintain_visibility(tiny).await,
        Err(Error::SpaceInsufficient)
    );
    assert_eq!(
        memory.maintain_visibility(tiny).await,
        Err(Error::SpaceInsufficient)
    );
    assert_eq!(
        storage.query_visible(query(5)).await,
        Err(Error::VisibilityRebuilding)
    );
    assert_eq!(memory.status().await, Err(Error::VisibilityRebuilding));
    let enough = VisibilityIndexBounds::new(1, 4096 + 128 + 4096 + 16 * 32).unwrap();
    let first = storage.maintain_visibility(enough).await.unwrap();
    assert_eq!(first, memory.maintain_visibility(enough).await.unwrap());
    assert_eq!(first.processed_keys(), 1);
    assert!(!first.is_complete());
    let done = storage.maintain_visibility(enough).await.unwrap();
    assert_eq!(done, memory.maintain_visibility(enough).await.unwrap());
    assert!(done.is_complete());
    assert_eq!(storage.status().await.unwrap().visible_events(), 1);
    sqlx::query("UPDATE radroots_runtime_source_generations SET sequence_head = sequence_head + 1 WHERE generation = ?")
        .bind(generation.as_bytes().as_slice()).execute(&storage.pool).await.unwrap();
    assert_eq!(
        storage
            .maintain_visibility(VisibilityIndexBounds::new(64, 16 * 1024 * 1024).unwrap())
            .await,
        Err(Error::CorruptStoredEvent)
    );
    assert_eq!(storage.status().await, Err(Error::CorruptStoredEvent));
}

#[tokio::test]
async fn indexed_visibility_large_mixed_histories_have_bounded_next_pages_and_truthful_cached_counts()
 {
    for size in [10_000_i64, 16_384_i64] {
        let generation = SourceGeneration::new([92; 32]).unwrap();
        let storage = tests::store(generation).await;
        let mut transaction = storage.pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
        sqlx::query(
            "UPDATE radroots_runtime_source_generations SET sequence_head = ? WHERE generation = ?",
        )
        .bind(size)
        .bind(generation.as_bytes().as_slice())
        .execute(&mut *transaction)
        .await
        .unwrap();
        for index in 0..size {
            let profile = index % 4 == 0;
            let unsupported = index % 4 == 3;
            let content = if profile {
                format!(r#"{{"name":"Farm {index}"}}"#)
            } else {
                format!("retained {index}")
            };
            let event = tests::signed_event_with(
                &content,
                false,
                index as u64 + 100,
                if profile || unsupported { 0 } else { 1 },
                vec![],
            );
            let stage = match index % 4 {
                2 => "raw",
                3 => "verified",
                _ => "visible",
            };
            let contract = if stage == "visible" {
                Some(
                    tests::visible(&event)
                        .admitted_event()
                        .validated_event()
                        .contract_id(),
                )
            } else {
                None
            };
            sqlx::query("INSERT INTO radroots_runtime_events
                (source_generation, source_sequence, event_id, admission_stage, signed_event,
                 admitted_at_unix_ms, updated_at_unix_ms, admitted_contract_id, admitted_registry_version)
                VALUES (?, ?, ?, ?, ?, 10, 10, ?, ?)")
                .bind(generation.as_bytes().as_slice()).bind(index + 1).bind(event.id().as_bytes().as_slice())
                .bind(stage).bind(event.raw_json().as_bytes()).bind(contract)
                .bind(contract.map(|_| i64::from(radroots_event::contract::RegistryVersion::CURRENT.get())))
                .execute(&mut *transaction).await.unwrap();
        }
        transaction.commit().await.unwrap();
        assert_eq!(storage.status().await, Err(Error::VisibilityRebuilding));
        assert_eq!(
            storage.query_visible(query(5)).await,
            Err(Error::VisibilityRebuilding)
        );
        finish(&storage).await;
        let oracle = storage.rebuild_visibility().await.unwrap();
        let status = storage.status().await.unwrap();
        assert_eq!(status.raw_events(), size as u64);
        assert_eq!(status.verified_events(), (size * 3 / 4) as u64);
        assert_eq!(
            status.visible_events(),
            oracle.visible_event_ids().len() as u64
        );
        storage.work.take();
        let token = storage.source_token().await.unwrap();
        let mut bounds = EventQueryBounds::first(1000).unwrap();
        let mut returned = std::collections::BTreeSet::new();
        loop {
            let page = storage
                .query_visible_at(EventQuery::all(bounds), token)
                .await
                .unwrap();
            let counts = storage.work.take();
            assert_eq!(counts.materialization_requests, 0);
            assert_eq!(counts.visibility_inputs, 0);
            assert!(counts.key_rows_returned <= 1001);
            assert_eq!(counts.codec_inputs, page.items().len() as u64);
            for item in page.items() {
                assert!(returned.insert(*item.event().id()));
            }
            match page.next_cursor() {
                Some(cursor) => bounds = EventQueryBounds::first(1000).unwrap().after(cursor),
                None => break,
            }
        }
        assert_eq!(
            returned,
            oracle.visible_event_ids().iter().copied().collect()
        );
        let first = storage.query_visible(query(5)).await.unwrap();
        storage.work.take();
        let next = storage
            .query_visible(EventQuery::all(
                EventQueryBounds::first(5)
                    .unwrap()
                    .after(first.next_cursor().unwrap()),
            ))
            .await
            .unwrap();
        let counts = storage.work.take();
        assert_eq!(next.items().len(), 5);
        assert_eq!(counts.key_rows_returned, 6);
        assert_eq!(counts.codec_inputs, 5);
        assert_eq!(counts.materialization_requests, 0);
        let sql = storage.work.template(work::Read::Keys);
        let mut builder = QueryBuilder::<Sqlite>::new("EXPLAIN QUERY PLAN ");
        builder.push(sql);
        let plans = builder
            .build()
            .bind(generation.as_bytes().as_slice())
            .bind(i64_from_u64(first.next_cursor().unwrap().sequence().get()).unwrap())
            .bind(6_i64)
            .fetch_all(&storage.pool)
            .await
            .unwrap();
        assert!(plans.iter().any(|row| {
            row.get::<String, _>("detail")
                .contains("visibility_members_page_idx")
        }));
        assert!(
            !plans
                .iter()
                .any(|row| row.get::<String, _>("detail").contains("USE TEMP B-TREE"))
        );
        storage.work.take();
        assert_eq!(storage.status().await.unwrap(), status);
        let cached = storage.work.take();
        assert_eq!(cached.aggregate_rows_returned, 1);
        assert_eq!(cached.codec_inputs, 0);
        storage.close().await.unwrap();
    }
}

#[tokio::test]
async fn indexed_visibility_deletion_queue_is_bounded_restartable_and_matches_memory() {
    let generation = SourceGeneration::new([93; 32]).unwrap();
    let storage = tests::store(generation).await;
    let memory = MemoryStorage::new(generation);
    let mut tags = Vec::new();
    for index in 0..133 {
        let event = tests::signed_event_with(&format!("target {index}"), false, index, 1, vec![]);
        tags.push(vec!["e".to_owned(), event.id().to_hex()]);
        let value = admission(event);
        storage.admit(value.clone()).await.unwrap();
        memory.admit(value).await.unwrap();
    }
    let deletion = admission(tests::signed_event_with("delete", false, 200, 5, tags));
    storage.admit(deletion.clone()).await.unwrap();
    memory.admit(deletion).await.unwrap();
    assert_eq!(storage.status().await, Err(Error::VisibilityRebuilding));
    assert_eq!(
        memory.query_visible(query(5)).await,
        Err(Error::VisibilityRebuilding)
    );
    let reopened = SqliteStorage::new(storage.pool.clone(), generation, EventStoreMode::ReadWrite);
    let bounds = VisibilityIndexBounds::new(64, 16 * 1024 * 1024).unwrap();
    let first = reopened.maintain_visibility(bounds).await.unwrap();
    assert_eq!(first, memory.maintain_visibility(bounds).await.unwrap());
    assert_eq!(first.processed_rows(), 0);
    assert_eq!(first.processed_keys(), 64);
    assert!(!first.is_complete());
    // Admission while target work is unfinished must retain both cursors.
    let later = admission(tests::signed_event_with("later", false, 300, 1, vec![]));
    reopened.admit(later.clone()).await.unwrap();
    memory.admit(later).await.unwrap();
    for _ in 0..4 {
        let progress = reopened.maintain_visibility(bounds).await.unwrap();
        assert_eq!(progress, memory.maintain_visibility(bounds).await.unwrap());
        assert!(progress.processed_rows() + progress.processed_keys() <= 64);
        if progress.is_complete() {
            break;
        }
    }
    assert_eq!(
        reopened.status().await.unwrap(),
        memory.status().await.unwrap()
    );
    assert_eq!(reopened.status().await.unwrap().visible_events(), 2);
    assert_eq!(
        reopened.query_visible(query(5)).await.unwrap(),
        memory.query_visible(query(5)).await.unwrap()
    );
    assert_eq!(
        reopened.rebuild_visibility().await.unwrap(),
        memory.rebuild_visibility().await.unwrap()
    );
}

#[tokio::test]
async fn indexed_visibility_partial_byte_pages_preserve_continuation_and_id_navigation() {
    let generation = SourceGeneration::new([94; 32]).unwrap();
    let storage = tests::store(generation).await;
    let memory = MemoryStorage::new(generation);
    let mut ids = Vec::new();
    for index in 0..12 {
        let event = tests::signed_event_with(
            &format!("{index}{}", "x".repeat(120_000)),
            false,
            index,
            1,
            vec![],
        );
        ids.push(*event.id());
        let value = admission(event);
        storage.admit(value.clone()).await.unwrap();
        memory.admit(value).await.unwrap();
    }
    let first = storage.query_visible(query(12)).await.unwrap();
    assert_eq!(first, memory.query_visible(query(12)).await.unwrap());
    assert!(first.items().len() < 12 && !first.items().is_empty());
    let next_bounds = EventQueryBounds::first(12)
        .unwrap()
        .after(first.next_cursor().unwrap());
    let next = storage
        .query_visible(EventQuery::all(next_bounds))
        .await
        .unwrap();
    assert_eq!(first.items().len() + next.items().len(), 12);
    assert!(next.next_cursor().is_none());
    storage.work.take();
    let selected =
        EventQuery::for_ids(EventQueryBounds::first(5).unwrap(), vec![ids[0], ids[11]]).unwrap();
    let page = storage.query_visible(selected.clone()).await.unwrap();
    assert_eq!(page, memory.query_visible(selected).await.unwrap());
    let counts = storage.work.take();
    assert_eq!(counts.key_rows_returned, 2);
    assert_eq!(counts.codec_inputs, 2);
    assert!(
        storage
            .work
            .template(work::Read::Keys)
            .contains("INDEXED BY radroots_runtime_visibility_members_event_idx")
    );
}

#[tokio::test]
async fn indexed_visibility_fault_after_membership_rolls_back_source_counts_and_readiness() {
    let storage = tests::store(SourceGeneration::new([95; 32]).unwrap()).await;
    let token = storage.source_token().await.unwrap();
    let before = storage.status().await.unwrap();
    sqlx::raw_sql(
        "CREATE TEMP TRIGGER fail_membership AFTER INSERT ON radroots_runtime_visibility_members
        BEGIN SELECT RAISE(ABORT, 'fixture membership fault'); END",
    )
    .execute(&storage.pool)
    .await
    .unwrap();
    let event = tests::signed_event_with("failure", false, 10, 1, vec![]);
    assert_eq!(
        storage.admit(admission(event.clone())).await,
        Err(Error::BackendUnavailable)
    );
    assert_eq!(storage.source_token().await.unwrap(), token);
    assert_eq!(storage.status().await.unwrap(), before);
    assert!(
        storage
            .query_raw(query(5))
            .await
            .unwrap()
            .items()
            .is_empty()
    );
    sqlx::query("DROP TRIGGER fail_membership")
        .execute(&storage.pool)
        .await
        .unwrap();
    storage.admit(admission(event)).await.unwrap();
    assert_eq!(storage.status().await.unwrap().visible_events(), 1);
}

#[tokio::test]
async fn indexed_visibility_cancelled_pool_wait_has_no_payload_or_maintenance_effects() {
    use std::task::{Context, Poll, Waker};
    let storage = tests::store(SourceGeneration::new([96; 32]).unwrap()).await;
    storage
        .admit(admission(tests::signed_event_with(
            "retained",
            false,
            10,
            1,
            vec![],
        )))
        .await
        .unwrap();
    let token = storage.source_token().await.unwrap();
    storage.work.take();
    let held = storage.pool.acquire().await.unwrap();
    let mut future = storage.query_visible_at(query(5), token);
    assert!(matches!(
        future
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop())),
        Poll::Pending
    ));
    drop(future);
    drop(held);
    assert_eq!(storage.work.take(), work::Counts::default());
    assert_eq!(storage.source_token().await.unwrap(), token);
    assert_eq!(
        storage
            .query_visible_at(query(5), token)
            .await
            .unwrap()
            .items()
            .len(),
        1
    );
}

#[tokio::test]
async fn indexed_visibility_concurrent_wal_admission_returns_one_fenced_head_or_typed_stale() {
    let directory = tempfile::tempdir().unwrap();
    let paths = crate::Paths::from_directory(directory.path()).unwrap();
    let storage = SqliteStorage::open(
        crate::OpenOptions::new(paths, crate::OpenMode::Create)
            .with_source_generation(SourceGeneration::new([97; 32]).unwrap(), 100)
            .unwrap(),
    )
    .await
    .unwrap();
    let mut current = tests::signed_event_with(r#"{"name":"initial"}"#, false, 1, 0, vec![]);
    storage.admit(admission(current.clone())).await.unwrap();
    for index in 2..18 {
        let next = tests::signed_event_with(
            &format!(r#"{{"name":"Farm {index}"}}"#),
            false,
            index,
            0,
            vec![],
        );
        let token = storage.source_token().await.unwrap();
        let (read, write) = tokio::join!(
            storage.query_visible_at(query(5), token),
            storage.admit(admission(next.clone()))
        );
        write.unwrap();
        match read {
            Ok(page) => {
                assert_eq!(page.items().len(), 1);
                assert_eq!(page.items()[0].event().id(), current.id());
            }
            Err(Error::SourceEpochChanged) => {}
            Err(error) => panic!("unexpected fenced read failure: {error}"),
        }
        let page = storage
            .query_visible_at(query(5), storage.source_token().await.unwrap())
            .await
            .unwrap();
        assert_eq!(page.items().len(), 1);
        assert_eq!(page.items()[0].event().id(), next.id());
        current = next;
    }
    storage.close().await.unwrap();
}
