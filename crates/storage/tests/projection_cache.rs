use futures_executor::block_on;
use radroots_storage::{
    Error, EventStore, ProjectionStore,
    event::{EventSourceToken, SourceGeneration},
    memory::MemoryStorage,
    projection::{
        ProjectionDocument, ProjectionGeneration, ProjectionId, ProjectionSnapshot,
        cache::{ProjectionCacheLimits, ProjectionCacheScope},
    },
};
fn id(value: &str) -> ProjectionId {
    ProjectionId::parse(value).unwrap()
}
fn generation(value: u8) -> ProjectionGeneration {
    ProjectionGeneration::new([value; 32]).unwrap()
}

#[test]
fn projection_cache_limits_and_exact_scope_validate_before_effects() {
    for values in [
        [0, 2, 32, 1],
        [1, 0, 32, 1],
        [1, 2, 0, 1],
        [1, 2, 32, 0],
        [i64::MAX as u64 + 1, 2, 32, 1],
    ] {
        assert_eq!(
            ProjectionCacheLimits::new(values[0], values[1], values[2], values[3]),
            Err(Error::InvalidProjectionCacheLimits)
        );
    }
    let maximum = ProjectionCacheLimits::new(
        i64::MAX as u64,
        i64::MAX as u64,
        i64::MAX as u64,
        i64::MAX as u64,
    )
    .unwrap();
    assert_eq!(
        [
            maximum.value_bytes(),
            maximum.protected_generations(),
            maximum.legacy_snapshots(),
            maximum.legacy_age_ms()
        ],
        [i64::MAX as u64; 4]
    );
    let source = EventSourceToken::new(SourceGeneration::new([9; 32]).unwrap(), 0);
    let scope =
        ProjectionCacheScope::new(id("fixture"), generation(1), source, Some(generation(1)))
            .unwrap();
    assert_eq!(scope.projection_id(), &id("fixture"));
    assert_eq!(scope.generation(), generation(1));
    assert_eq!(scope.source(), source);
    assert_eq!(scope.expected_active(), Some(generation(1)));
    assert!(!Error::InvalidProjectionCacheLimits.to_string().is_empty());
}

#[test]
fn projection_cache_preserves_other_projection_state_and_expires_legacy_by_owner_time() {
    let store = MemoryStorage::new(SourceGeneration::new([9; 32]).unwrap());
    block_on(store.put_projection_document(
        id("settings"),
        generation(1),
        ProjectionDocument::new("owned_work".into(), vec![7]).unwrap(),
    ))
    .unwrap();
    block_on(store.put_projection_snapshot(
        ProjectionSnapshot::new(id("cache"), [1; 32], generation(1), 1, vec![8]).unwrap(),
    ))
    .unwrap();
    let limits = ProjectionCacheLimits::new(67_108_864, 2, 32, 1_800_000).unwrap();
    let usage = block_on(store.configure_projection_cache(id("cache"), limits)).unwrap();
    assert_eq!(usage.value_bytes, 2);
    assert_eq!(usage.legacy_snapshots, 1);
    assert_eq!(
        block_on(store.projection_snapshot(id("cache"), [1; 32])).unwrap(),
        None
    );
    let source = block_on(store.source_token()).unwrap();
    let foreign = ProjectionCacheScope::new(id("settings"), generation(1), source, None).unwrap();
    assert_eq!(
        block_on(store.collect_projection_cache(foreign)),
        Err(Error::ProjectionRevisionConflict)
    );
    let exact = ProjectionCacheScope::new(id("cache"), generation(1), source, None).unwrap();
    let result = block_on(store.collect_projection_cache(exact)).unwrap();
    assert_eq!(result.removed_rows, 1);
    assert_eq!(result.removed_value_bytes, 1);
    assert_eq!(result.usage.value_bytes, 1);
    assert_eq!(
        block_on(store.projection_document(id("settings"), generation(1), "owned_work".into()))
            .unwrap()
            .unwrap()
            .value(),
        &[7]
    );
    assert_eq!(
        block_on(store.put_projection_snapshot(
            ProjectionSnapshot::new(id("cache"), [1; 32], generation(1), 1, vec![8]).unwrap()
        )),
        Err(Error::ProjectionRevisionConflict)
    );
    assert_eq!(block_on(store.source_token()).unwrap(), source);
    assert_eq!(
        block_on(store.configure_projection_cache(
            id("cache"),
            ProjectionCacheLimits::new(1, 2, 32, 1_800_000).unwrap()
        )),
        Err(Error::ProjectionRevisionConflict)
    );
}

#[test]
fn projection_cache_memory_third_protected_and_foreign_bytes_refuse_without_partial_rows() {
    use radroots_storage::projection::{
        ProjectionCheckpoint,
        batch::{ProjectionDocumentBatch, ProjectionWriteScope},
        document_query::{ProjectionDocumentGenerations, ProjectionDocumentQuery},
    };
    let store = MemoryStorage::new(SourceGeneration::new([9; 32]).unwrap());
    let source = block_on(store.source_token()).unwrap();
    let limits = ProjectionCacheLimits::new(3, 2, 32, 1_800_000).unwrap();
    block_on(store.configure_projection_cache(id("cache"), limits)).unwrap();
    let first = ProjectionDocumentBatch::new(
        ProjectionWriteScope::new(id("cache"), generation(1), source, None).unwrap(),
        vec![ProjectionDocument::new("a".into(), vec![7]).unwrap()],
    )
    .unwrap();
    block_on(store.promote_projection_batch(
        first,
        ProjectionCheckpoint::new(id("cache"), generation(1), None, 1, 1).unwrap(),
    ))
    .unwrap();
    block_on(store.configure_projection_cache(id("second"), limits)).unwrap();
    let staged = ProjectionDocumentBatch::new(
        ProjectionWriteScope::new(id("second"), generation(2), source, None).unwrap(),
        vec![ProjectionDocument::new("b".into(), vec![7]).unwrap()],
    )
    .unwrap();
    block_on(store.put_projection_batch(staged)).unwrap();
    let third = ProjectionDocumentBatch::new(
        ProjectionWriteScope::new(id("cache"), generation(3), source, Some(generation(1))).unwrap(),
        vec![ProjectionDocument::new("c".into(), vec![7]).unwrap()],
    )
    .unwrap();
    assert_eq!(
        block_on(store.put_projection_batch(third)),
        Err(Error::SpaceInsufficient)
    );
    let query = ProjectionDocumentQuery::new(
        id("cache"),
        ProjectionDocumentGenerations::Exact(generation(3)),
        1,
    )
    .unwrap();
    assert!(
        block_on(store.query_projection_documents(query))
            .unwrap()
            .records()
            .is_empty()
    );
    assert_eq!(
        block_on(store.configure_projection_cache(id("cache"), limits))
            .unwrap()
            .value_bytes,
        2
    );
    assert_eq!(
        block_on(store.put_projection_document(
            id("foreign"),
            generation(1),
            ProjectionDocument::new("too_big".into(), vec![7; 2]).unwrap()
        )),
        Err(Error::SpaceInsufficient)
    );
    assert_eq!(
        block_on(store.projection_document(id("foreign"), generation(1), "too_big".into()))
            .unwrap(),
        None
    );
    assert_eq!(
        block_on(store.configure_projection_cache(id("cache"), limits))
            .unwrap()
            .protected_generations,
        2
    );
}

fn cache_batch(
    source: EventSourceToken,
    active: Option<ProjectionGeneration>,
    target: u8,
    keys: &[String],
) -> radroots_storage::projection::batch::ProjectionDocumentBatch {
    use radroots_storage::projection::batch::{ProjectionDocumentBatch, ProjectionWriteScope};
    ProjectionDocumentBatch::new(
        ProjectionWriteScope::new(id("cache"), generation(target), source, active).unwrap(),
        keys.iter()
            .map(|key| ProjectionDocument::new(key.clone(), vec![7]).unwrap())
            .collect(),
    )
    .unwrap()
}
fn cache_scope(
    source: EventSourceToken,
    active: Option<ProjectionGeneration>,
    target: u8,
) -> ProjectionCacheScope {
    ProjectionCacheScope::new(id("cache"), generation(target), source, active).unwrap()
}
fn cache_checkpoint(target: u8, count: u64) -> radroots_storage::projection::ProjectionCheckpoint {
    radroots_storage::projection::ProjectionCheckpoint::new(
        id("cache"),
        generation(target),
        None,
        count,
        1,
    )
    .unwrap()
}
fn cache_limits() -> ProjectionCacheLimits {
    ProjectionCacheLimits::new(67_108_864, 2, 32, 1_800_000).unwrap()
}
#[test]
fn projection_cache_memory_live_staging_and_63_64_65_retirement_preserve_exact_continuation() {
    use radroots_storage::projection::{InvalidationReason, ProjectionInvalidation};
    let store = MemoryStorage::new(SourceGeneration::new([9; 32]).unwrap());
    let source = block_on(store.source_token()).unwrap();
    block_on(store.put_projection_document(
        id("z_foreign"),
        generation(9),
        ProjectionDocument::new("keep".into(), vec![8]).unwrap(),
    ))
    .unwrap();
    block_on(store.configure_projection_cache(id("cache"), cache_limits())).unwrap();
    block_on(store.promote_projection_batch(
        cache_batch(source, None, 1, &["active".into()]),
        cache_checkpoint(1, 1),
    ))
    .unwrap();
    assert_eq!(
        block_on(store.retire_projection_generation(cache_scope(source, Some(generation(1)), 1))),
        Err(Error::ProjectionRevisionConflict)
    );
    assert_eq!(
        block_on(store.collect_projection_cache(cache_scope(source, Some(generation(1)), 1)))
            .unwrap()
            .removed_rows,
        0
    );
    assert_eq!(
        block_on(store.collect_projection_cache(cache_scope(
            EventSourceToken::new(source.generation(), 1),
            Some(generation(1)),
            1
        ))),
        Err(Error::SourceEpochChanged)
    );
    assert_eq!(
        block_on(store.collect_projection_cache(cache_scope(source, None, 1))),
        Err(Error::ProjectionRevisionConflict)
    );
    for (target, count) in [(2, 63), (3, 64), (4, 65), (5, 0)] {
        let keys: Vec<_> = (0..count.min(64))
            .map(|number| format!("key.{number:02}"))
            .collect();
        block_on(store.put_projection_batch(cache_batch(
            source,
            Some(generation(1)),
            target,
            &keys,
        )))
        .unwrap();
        if count == 65 {
            block_on(store.put_projection_batch(cache_batch(
                source,
                Some(generation(1)),
                target,
                &["key.64".into()],
            )))
            .unwrap();
        }
        assert_eq!(
            block_on(store.collect_projection_cache(cache_scope(
                source,
                Some(generation(1)),
                target
            )))
            .unwrap()
            .removed_rows,
            0
        );
        block_on(store.retire_projection_generation(cache_scope(
            source,
            Some(generation(1)),
            target,
        )))
        .unwrap();
        let first = block_on(store.collect_projection_cache(cache_scope(
            source,
            Some(generation(1)),
            target,
        )))
        .unwrap();
        assert_eq!(first.removed_rows, (count + 1).min(64));
        assert_eq!(first.more, count >= 64);
        if first.more {
            let second = block_on(store.collect_projection_cache(cache_scope(
                source,
                Some(generation(1)),
                target,
            )))
            .unwrap();
            assert_eq!(second.removed_rows, count - 63);
            assert!(!second.more);
        }
        assert_eq!(
            block_on(store.configure_projection_cache(id("cache"), cache_limits()))
                .unwrap()
                .value_bytes,
            2
        );
    }
    block_on(
        store.invalidate(
            ProjectionInvalidation::new(
                id("cache"),
                generation(1),
                generation(6),
                InvalidationReason::OperatorRequested,
                1,
            )
            .unwrap(),
        ),
    )
    .unwrap();
    block_on(store.retire_projection_generation(cache_scope(source, Some(generation(1)), 1)))
        .unwrap();
    assert_eq!(
        block_on(ProjectionStore::status(&store, id("cache"))).unwrap(),
        None
    );
    block_on(store.promote_projection_batch(
        cache_batch(source, None, 6, &["successor".into()]),
        cache_checkpoint(6, 1),
    ))
    .unwrap();
    block_on(store.collect_projection_cache(cache_scope(source, Some(generation(6)), 1))).unwrap();
    assert_eq!(
        block_on(store.projection_document(id("z_foreign"), generation(9), "keep".into()))
            .unwrap()
            .unwrap()
            .value(),
        &[8]
    );
}

#[test]
fn projection_cache_memory_protected_legacy_pressure_future_and_expiry_collect_without_replay() {
    let store = MemoryStorage::new(SourceGeneration::new([9; 32]).unwrap());
    let source = block_on(store.source_token()).unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    block_on(store.checkpoint(cache_checkpoint(1, 0))).unwrap();
    for number in 1..=33 {
        block_on(
            store.put_projection_snapshot(
                ProjectionSnapshot::new(id("cache"), [number; 32], generation(1), now, vec![8])
                    .unwrap(),
            ),
        )
        .unwrap();
    }
    block_on(store.put_projection_snapshot(
        ProjectionSnapshot::new(id("z_foreign"), [1; 32], generation(9), now, vec![8]).unwrap(),
    ))
    .unwrap();
    block_on(store.configure_projection_cache(id("cache"), cache_limits())).unwrap();
    let first =
        block_on(store.collect_projection_cache(cache_scope(source, Some(generation(1)), 1)))
            .unwrap();
    assert_eq!(first.removed_rows, 2);
    assert!(!first.more);
    assert_eq!(first.usage.legacy_snapshots, 32);
    assert_eq!(
        block_on(store.projection_snapshot(id("cache"), [1; 32])).unwrap(),
        None
    );
    assert!(
        block_on(store.projection_snapshot(id("cache"), [3; 32]))
            .unwrap()
            .is_some()
    );
    let future_store = MemoryStorage::new(source.generation());
    block_on(future_store.checkpoint(cache_checkpoint(1, 0))).unwrap();
    for number in 1..=65 {
        block_on(
            future_store.put_projection_snapshot(
                ProjectionSnapshot::new(
                    id("cache"),
                    [number; 32],
                    generation(1),
                    now + 3_600_000,
                    vec![8],
                )
                .unwrap(),
            ),
        )
        .unwrap();
    }
    block_on(
        future_store.put_projection_snapshot(
            ProjectionSnapshot::new(
                id("z_foreign"),
                [1; 32],
                generation(9),
                now + 3_600_000,
                vec![8],
            )
            .unwrap(),
        ),
    )
    .unwrap();
    block_on(future_store.configure_projection_cache(id("cache"), cache_limits())).unwrap();
    let first = block_on(future_store.collect_projection_cache(cache_scope(
        source,
        Some(generation(1)),
        1,
    )))
    .unwrap();
    assert_eq!(first.removed_rows, 64);
    assert!(first.more);
    let second = block_on(future_store.collect_projection_cache(cache_scope(
        source,
        Some(generation(1)),
        1,
    )))
    .unwrap();
    assert_eq!(second.removed_rows, 1);
    assert!(!second.more);
    assert_eq!(second.usage.legacy_snapshots, 1);
    assert_eq!(
        block_on(future_store.put_projection_snapshot(
            ProjectionSnapshot::new(id("cache"), [1; 32], generation(1), now, vec![8]).unwrap()
        )),
        Err(Error::ProjectionRevisionConflict)
    );
    assert!(
        block_on(future_store.projection_snapshot(id("z_foreign"), [1; 32]))
            .unwrap()
            .is_some()
    );
}

async fn legacy_cache_completion(backend: &dyn ProjectionStore, registered: bool, capacity: u64) {
    use radroots_storage::projection::{
        InvalidationReason, ProjectionCheckpoint, ProjectionHealth, ProjectionInvalidation,
        RawSourceDigest, RebuildTicket, RebuildTicketId, RebuildTransition,
        batch::{ProjectionDocumentBatch, ProjectionWriteScope},
    };
    let id = ProjectionId::parse("legacy_cache_completion").unwrap();
    let source = EventSourceToken::new(SourceGeneration::new([9; 32]).unwrap(), 0);
    let limits = ProjectionCacheLimits::new(67_108_864, capacity, 32, 1_800_000).unwrap();
    let checkpoint =
        |target| ProjectionCheckpoint::new(id.clone(), generation(target), None, 1, 1).unwrap();
    let batch = |target, active| {
        ProjectionDocumentBatch::new(
            ProjectionWriteScope::new(id.clone(), generation(target), source, active).unwrap(),
            vec![ProjectionDocument::new("kept".into(), vec![7]).unwrap()],
        )
        .unwrap()
    };
    let scope = |target, active| {
        ProjectionCacheScope::new(
            id.clone(),
            generation(target),
            source,
            Some(generation(active)),
        )
        .unwrap()
    };
    backend
        .configure_projection_cache(id.clone(), limits)
        .await
        .unwrap();
    if registered {
        backend
            .promote_projection_batch(batch(1, None), checkpoint(1))
            .await
            .unwrap();
    } else {
        backend.checkpoint(checkpoint(1)).await.unwrap();
    }
    backend
        .put_projection_batch(batch(2, Some(generation(1))))
        .await
        .unwrap();
    let invalidation = ProjectionInvalidation::new(
        id.clone(),
        generation(1),
        generation(3),
        InvalidationReason::OperatorRequested,
        10,
    )
    .unwrap();
    backend.invalidate(invalidation.clone()).await.unwrap();
    let ticket = RebuildTicket::requested(
        RebuildTicketId::new([3; 16]).unwrap(),
        invalidation,
        source.generation(),
        None,
        RawSourceDigest::new([1; 32]),
    )
    .unwrap();
    backend.request_rebuild(ticket.clone()).await.unwrap();
    let running = backend
        .transition_rebuild(RebuildTransition::start(
            ticket.ticket_id(),
            ticket.revision(),
            11,
        ))
        .await
        .unwrap();
    let before_status = backend.status(id.clone()).await.unwrap();
    let before_usage = backend
        .configure_projection_cache(id.clone(), limits)
        .await
        .unwrap();
    assert_eq!(before_usage.protected_generations, 2);
    let complete =
        RebuildTransition::complete(running.ticket_id(), running.revision(), 12, checkpoint(3));
    if registered && capacity == 2 {
        assert_eq!(
            backend.transition_rebuild(complete.clone()).await,
            Err(Error::SpaceInsufficient)
        );
        assert_eq!(backend.status(id.clone()).await.unwrap(), before_status);
        assert_eq!(
            backend.rebuild(ticket.ticket_id()).await.unwrap(),
            Some(running.clone())
        );
        assert_eq!(
            backend
                .configure_projection_cache(id.clone(), limits)
                .await
                .unwrap(),
            before_usage
        );
        assert_eq!(
            backend
                .collect_projection_cache(scope(1, 1))
                .await
                .unwrap()
                .removed_rows,
            0
        );
        assert_eq!(
            backend
                .collect_projection_cache(scope(2, 1))
                .await
                .unwrap()
                .removed_rows,
            0
        );
        assert_eq!(
            backend
                .retire_projection_generation(scope(2, 1))
                .await
                .unwrap()
                .protected_generations,
            1
        );
    }
    let completed = backend.transition_rebuild(complete).await.unwrap();
    assert_eq!(
        backend.rebuild(ticket.ticket_id()).await.unwrap(),
        Some(completed)
    );
    let status = backend.status(id.clone()).await.unwrap().unwrap();
    assert_eq!(status.generation(), generation(3));
    assert_eq!(status.health(), ProjectionHealth::Ready);
    assert_eq!(status.active_rebuild(), None);
    assert_eq!(status.checkpoint(), Some(&checkpoint(3)));
    let expected = if registered && capacity == 3 { 3 } else { 2 };
    assert_eq!(
        backend
            .configure_projection_cache(id.clone(), limits)
            .await
            .unwrap()
            .protected_generations,
        expected
    );
    assert_eq!(
        backend
            .collect_projection_cache(scope(3, 3))
            .await
            .unwrap()
            .removed_rows,
        0
    );
    if registered {
        assert_eq!(
            backend
                .collect_projection_cache(scope(1, 3))
                .await
                .unwrap()
                .removed_rows,
            0
        );
        assert_eq!(
            backend
                .retire_projection_generation(scope(1, 3))
                .await
                .unwrap()
                .protected_generations,
            expected - 1
        );
        let collected = backend.collect_projection_cache(scope(1, 3)).await.unwrap();
        assert_eq!(collected.removed_rows, 2);
        assert_eq!(collected.removed_value_bytes, 1);
        assert!(!collected.more);
        assert_eq!(collected.usage.protected_generations, expected - 1);
    }
    if !registered || capacity == 3 {
        assert_eq!(
            backend
                .collect_projection_cache(scope(2, 3))
                .await
                .unwrap()
                .removed_rows,
            0
        );
        backend
            .retire_projection_generation(scope(2, 3))
            .await
            .unwrap();
    }
    let collected = backend.collect_projection_cache(scope(2, 3)).await.unwrap();
    assert_eq!(collected.removed_rows, 2);
    assert_eq!(collected.removed_value_bytes, 1);
    assert!(!collected.more);
    assert_eq!(collected.usage.protected_generations, 1);
    assert_eq!(backend.status(id.clone()).await.unwrap(), Some(status));
    assert_eq!(
        backend
            .collect_projection_cache(scope(3, 3))
            .await
            .unwrap()
            .removed_rows,
        0
    );
}

#[test]
fn projection_cache_memory_legacy_completion_preserves_registry_capacity_and_atomic_refusal() {
    for (registered, capacity) in [(true, 2), (true, 3), (false, 2)] {
        let memory = MemoryStorage::new(SourceGeneration::new([9; 32]).unwrap());
        block_on(legacy_cache_completion(&memory, registered, capacity));
    }
}

async fn cache_batch_replacement(
    backend: &dyn ProjectionStore,
    quota: u64,
    initial: [usize; 2],
    replacement: [usize; 2],
    reverse: bool,
) {
    use radroots_storage::projection::batch::{ProjectionDocumentBatch, ProjectionWriteScope};
    let id = ProjectionId::parse("batch_replacement").unwrap();
    let source = EventSourceToken::new(SourceGeneration::new([9; 32]).unwrap(), 0);
    let batch = |sizes: [usize; 2], target, reverse| {
        let mut documents = vec![
            ProjectionDocument::new("a".into(), vec![1; sizes[0]]).unwrap(),
            ProjectionDocument::new("b".into(), vec![2; sizes[1]]).unwrap(),
        ];
        if reverse {
            documents.reverse();
        }
        ProjectionDocumentBatch::new(
            ProjectionWriteScope::new(id.clone(), generation(target), source, None).unwrap(),
            documents,
        )
        .unwrap()
    };
    backend
        .put_projection_batch(batch(initial, 1, false))
        .await
        .unwrap();
    let limits = ProjectionCacheLimits::new(quota, 2, 32, 1_800_000).unwrap();
    let before = backend
        .configure_projection_cache(id.clone(), limits)
        .await
        .unwrap();
    let status = backend.status(id.clone()).await.unwrap();
    let previous_bytes = initial.iter().sum::<usize>() as u64;
    let next_bytes = replacement.iter().sum::<usize>() as u64;
    let result = backend
        .put_projection_batch(batch(replacement, 1, reverse))
        .await;
    let refused = next_bytes > previous_bytes && next_bytes > quota;
    let expected = if refused {
        assert_eq!(result, Err(Error::SpaceInsufficient));
        assert_eq!(
            backend
                .configure_projection_cache(id.clone(), limits)
                .await
                .unwrap(),
            before
        );
        initial
    } else {
        result.unwrap();
        let current = backend
            .configure_projection_cache(id.clone(), limits)
            .await
            .unwrap();
        assert_eq!(current.value_bytes, next_bytes);
        assert_eq!(current.protected_generations, 1);
        replacement
    };
    assert_eq!(backend.status(id.clone()).await.unwrap(), status);
    for (key, size, value) in [("a", expected[0], 1), ("b", expected[1], 2)] {
        assert_eq!(
            backend
                .projection_document(id.clone(), generation(1), key.into())
                .await
                .unwrap()
                .unwrap()
                .value(),
            vec![value; size]
        );
    }
    let current = backend
        .configure_projection_cache(id.clone(), limits)
        .await
        .unwrap();
    let over = quota.saturating_sub(current.value_bytes) + 1;
    let foreign = ProjectionId::parse("direct_writer").unwrap();
    assert_eq!(
        backend
            .put_projection_document(
                foreign.clone(),
                generation(9),
                ProjectionDocument::new("overflow".into(), vec![9; over as usize]).unwrap()
            )
            .await,
        Err(Error::SpaceInsufficient)
    );
    assert_eq!(
        backend
            .projection_document(foreign, generation(9), "overflow".into())
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        backend
            .configure_projection_cache(id.clone(), limits)
            .await
            .unwrap(),
        current
    );
    assert_eq!(
        backend
            .put_projection_batch(
                ProjectionDocumentBatch::new(
                    ProjectionWriteScope::new(
                        ProjectionId::parse("new_admission").unwrap(),
                        generation(2),
                        source,
                        None
                    )
                    .unwrap(),
                    vec![ProjectionDocument::new("a".into(), vec![7; over as usize]).unwrap()]
                )
                .unwrap()
            )
            .await,
        Err(Error::SpaceInsufficient)
    );
    assert_eq!(
        backend
            .configure_projection_cache(id.clone(), limits)
            .await
            .unwrap(),
        current
    );
    assert_eq!(
        backend
            .projection_document(
                ProjectionId::parse("new_admission").unwrap(),
                generation(2),
                "a".into()
            )
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        backend
            .collect_projection_cache(
                ProjectionCacheScope::new(id.clone(), generation(1), source, None).unwrap()
            )
            .await
            .unwrap()
            .removed_rows,
        0
    );
    backend
        .retire_projection_generation(
            ProjectionCacheScope::new(id.clone(), generation(1), source, None).unwrap(),
        )
        .await
        .unwrap();
    let collected = backend
        .collect_projection_cache(
            ProjectionCacheScope::new(id, generation(1), source, None).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(collected.removed_rows, 3);
    assert!(!collected.more);
    assert_eq!(collected.usage.value_bytes, 0);
    assert_eq!(collected.usage.protected_generations, 0);
}

#[test]
fn projection_cache_memory_batch_replacement_has_one_aggregate_policy_in_both_orders() {
    for (quota, initial, replacement) in [
        (4, [2, 2], [3, 1]),
        (4, [2, 2], [2, 1]),
        (6, [4, 4], [5, 2]),
        (6, [4, 4], [5, 4]),
        (6, [1, 1], [2, 2]),
    ] {
        for reverse in [false, true] {
            let memory = MemoryStorage::new(SourceGeneration::new([9; 32]).unwrap());
            block_on(cache_batch_replacement(
                &memory,
                quota,
                initial,
                replacement,
                reverse,
            ));
        }
    }
}
