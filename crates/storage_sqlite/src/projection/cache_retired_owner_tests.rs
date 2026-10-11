use super::*;
use crate::{OpenMode, OpenOptions, Paths};
use radroots_storage::{
    ProjectionStore, Storage,
    event::SourceGeneration,
    memory::MemoryStorage,
    projection::{
        InvalidationReason, ProjectionCheckpoint, ProjectionDocument, ProjectionHealth,
        ProjectionInvalidation, ProjectionSnapshot,
        batch::{ProjectionDocumentBatch, ProjectionWriteScope},
        cache::{ProjectionCacheLimits, ProjectionCacheScope, ProjectionCacheUsage},
        document_query::{ProjectionDocumentGenerations, ProjectionDocumentQuery},
    },
};
use sqlx::Row;
use tempfile::TempDir;

fn id() -> ProjectionId {
    ProjectionId::parse("retired_owner_fixture").unwrap()
}
fn foreign() -> ProjectionId {
    ProjectionId::parse("foreign_fixture").unwrap()
}
fn generation(value: u8) -> ProjectionGeneration {
    ProjectionGeneration::new([value; 32]).unwrap()
}
fn limits() -> ProjectionCacheLimits {
    ProjectionCacheLimits::new(67_108_864, 2, 32, 1_800_000).unwrap()
}
fn query() -> ProjectionDocumentQuery {
    ProjectionDocumentQuery::new(id(), ProjectionDocumentGenerations::All, 256).unwrap()
}
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
fn batch(
    source: EventSourceToken,
    active: Option<ProjectionGeneration>,
    target: u8,
    start: usize,
    count: usize,
) -> ProjectionDocumentBatch {
    ProjectionDocumentBatch::new(
        ProjectionWriteScope::new(id(), generation(target), source, active).unwrap(),
        (start..start + count)
            .map(|index| ProjectionDocument::new(format!("row.{index:03}"), vec![target]).unwrap())
            .collect(),
    )
    .unwrap()
}
async fn seed(backend: &dyn Storage) -> EventSourceToken {
    let source = backend.source_token().await.unwrap();
    backend
        .put_projection_snapshot(
            ProjectionSnapshot::new(id(), [7; 32], generation(1), 1, vec![7; 8]).unwrap(),
        )
        .await
        .unwrap();
    backend
        .configure_projection_cache(id(), limits())
        .await
        .unwrap();
    backend
        .promote_projection_batch(
            batch(source, None, 1, 0, 63),
            ProjectionCheckpoint::new(id(), generation(1), None, 63, 1).unwrap(),
        )
        .await
        .unwrap();
    backend
        .put_projection_batch(batch(source, Some(generation(1)), 2, 0, 64))
        .await
        .unwrap();
    backend
        .promote_projection_batch(
            batch(source, Some(generation(1)), 2, 64, 1),
            ProjectionCheckpoint::new(id(), generation(2), None, 65, 2).unwrap(),
        )
        .await
        .unwrap();
    backend
        .promote_projection_batch(
            batch(source, Some(generation(2)), 3, 0, 1),
            ProjectionCheckpoint::new(id(), generation(3), None, 1, 3).unwrap(),
        )
        .await
        .unwrap();
    backend
        .put_projection_batch(batch(source, Some(generation(3)), 4, 0, 1))
        .await
        .unwrap();
    backend
        .put_projection_document(
            foreign(),
            generation(5),
            ProjectionDocument::new("keep".into(), vec![5]).unwrap(),
        )
        .await
        .unwrap();
    source
}
async fn preserved(backend: &dyn Storage, source: EventSourceToken) {
    assert_eq!(backend.source_token().await.unwrap(), source);
    assert_eq!(
        ProjectionStore::status(backend, id())
            .await
            .unwrap()
            .unwrap()
            .generation(),
        generation(3)
    );
    for target in [3, 4] {
        assert_eq!(
            backend
                .projection_document(id(), generation(target), "row.000".into())
                .await
                .unwrap()
                .unwrap()
                .value(),
            &[target]
        );
    }
    assert_eq!(
        backend
            .projection_document(foreign(), generation(5), "keep".into())
            .await
            .unwrap()
            .unwrap()
            .value(),
        &[5]
    );
}

#[tokio::test]
async fn projection_retired_owner_collection_bounds_every_pass_and_preserves_active_staging_foreign()
 {
    let temp = TempDir::new().unwrap();
    let store = open(&temp, OpenMode::Create).await;
    let memory = MemoryStorage::new(SourceGeneration::new([9; 32]).unwrap());
    for backend in [&memory as &dyn Storage, &store as &dyn Storage] {
        let source = seed(backend).await;
        let before = backend
            .configure_projection_cache(id(), limits())
            .await
            .unwrap();
        assert_eq!(before.value_bytes, 139);
        assert_eq!(before.protected_generations, 2);
        assert_eq!(before.legacy_snapshots, 1);
        let mut freed = 0;
        for (index, removed) in [64, 1, 64, 2].into_iter().enumerate() {
            let result = backend
                .collect_retired_projection_cache(id(), source, Some(generation(3)))
                .await
                .unwrap();
            assert_eq!(result.removed_rows, removed);
            assert!(result.removed_rows <= 64);
            assert_eq!(result.more, index < 3);
            freed += result.removed_value_bytes;
            preserved(backend, source).await;
        }
        assert_eq!(freed, 136);
        let done = backend
            .collect_retired_projection_cache(id(), source, Some(generation(3)))
            .await
            .unwrap();
        assert_eq!(done.removed_rows, 0);
        assert_eq!(done.removed_value_bytes, 0);
        assert!(!done.more);
        assert_eq!(
            done.usage,
            ProjectionCacheUsage {
                value_bytes: 3,
                protected_generations: 2,
                legacy_snapshots: 0
            }
        );
        assert_eq!(
            backend
                .query_projection_documents(query())
                .await
                .unwrap()
                .records()
                .len(),
            2
        );
    }
    store.close().await.unwrap();
}

#[tokio::test]
async fn projection_retired_owner_collection_fences_no_work_and_refuses_before_effects() {
    let temp = TempDir::new().unwrap();
    let store = open(&temp, OpenMode::Create).await;
    let memory = MemoryStorage::new(SourceGeneration::new([9; 32]).unwrap());
    for backend in [&memory as &dyn Storage, &store as &dyn Storage] {
        let source = backend.source_token().await.unwrap();
        assert_eq!(
            backend
                .collect_retired_projection_cache(id(), source, None)
                .await,
            Err(Error::ProjectionRevisionConflict)
        );
        let empty_id = ProjectionId::parse("empty_fixture").unwrap();
        backend
            .configure_projection_cache(empty_id.clone(), limits())
            .await
            .unwrap();
        let empty = backend
            .collect_retired_projection_cache(empty_id.clone(), source, None)
            .await
            .unwrap();
        assert_eq!(empty.removed_rows, 0);
        assert!(!empty.more);
        let stale = EventSourceToken::new(source.generation(), source.epoch() + 1);
        assert_eq!(
            backend
                .collect_retired_projection_cache(empty_id.clone(), stale, None)
                .await,
            Err(Error::SourceEpochChanged)
        );
        assert_eq!(
            backend
                .collect_retired_projection_cache(empty_id, source, Some(generation(3)))
                .await,
            Err(Error::ProjectionRevisionConflict)
        );
        let source = seed(backend).await;
        let before = backend
            .query_projection_documents(query())
            .await
            .unwrap()
            .into_records();
        let usage = backend
            .configure_projection_cache(id(), limits())
            .await
            .unwrap();
        for changed in [
            stale,
            EventSourceToken::new(SourceGeneration::new([8; 32]).unwrap(), 0),
        ] {
            assert_eq!(
                backend
                    .collect_retired_projection_cache(id(), changed, Some(generation(3)))
                    .await,
                Err(Error::SourceEpochChanged)
            );
        }
        assert_eq!(
            backend
                .collect_retired_projection_cache(id(), source, Some(generation(2)))
                .await,
            Err(Error::ProjectionRevisionConflict)
        );
        assert_eq!(
            backend
                .collect_retired_projection_cache(foreign(), source, None)
                .await,
            Err(Error::ProjectionRevisionConflict)
        );
        assert_eq!(
            backend
                .query_projection_documents(query())
                .await
                .unwrap()
                .into_records(),
            before
        );
        assert_eq!(
            backend
                .configure_projection_cache(id(), limits())
                .await
                .unwrap(),
            usage
        );
        preserved(backend, source).await;
    }
    store.close().await.unwrap();
}

#[tokio::test]
async fn projection_retired_owner_collection_rolls_back_and_recovers_across_read_race_and_reopen() {
    let temp = TempDir::new().unwrap();
    let store = open(&temp, OpenMode::Create).await;
    let source = seed(&store).await;
    let before = store
        .query_projection_documents(query())
        .await
        .unwrap()
        .into_records();
    let usage = store
        .configure_projection_cache(id(), limits())
        .await
        .unwrap();
    sqlx::raw_sql("CREATE TRIGGER fixture_retired_delete BEFORE DELETE ON radroots_runtime_projection_documents WHEN OLD.document_key='row.010' BEGIN SELECT RAISE(ABORT,'fixture'); END;").execute(store.pool()).await.unwrap();
    assert!(
        store
            .collect_retired_projection_cache(id(), source, Some(generation(3)))
            .await
            .is_err()
    );
    assert_eq!(
        store
            .query_projection_documents(query())
            .await
            .unwrap()
            .into_records(),
        before
    );
    assert_eq!(
        store
            .configure_projection_cache(id(), limits())
            .await
            .unwrap(),
        usage
    );
    sqlx::raw_sql("DROP TRIGGER fixture_retired_delete")
        .execute(store.pool())
        .await
        .unwrap();
    let current = ProjectionDocumentQuery::new(
        id(),
        ProjectionDocumentGenerations::Exact(generation(3)),
        20,
    )
    .unwrap();
    let (read, collected) = tokio::join!(
        store.query_projection_documents_at(current, source, generation(3)),
        store.collect_retired_projection_cache(id(), source, Some(generation(3))),
    );
    assert_eq!(read.unwrap().records().len(), 1);
    assert_eq!(collected.unwrap().removed_rows, 64);
    store.close().await.unwrap();
    let reopened = open(&temp, OpenMode::ReadWriteExisting).await;
    for (removed, more) in [(1, true), (64, true), (2, false)] {
        let result = reopened
            .collect_retired_projection_cache(id(), source, Some(generation(3)))
            .await
            .unwrap();
        assert_eq!((result.removed_rows, result.more), (removed, more));
    }
    preserved(&reopened, source).await;
    reopened.close().await.unwrap();
    let readonly = open(&temp, OpenMode::ReadOnly).await;
    assert_eq!(
        readonly
            .collect_retired_projection_cache(id(), source, Some(generation(3)))
            .await,
        Err(Error::BackendUnavailable)
    );
    preserved(&readonly, source).await;
    readonly.close().await.unwrap();
}

#[tokio::test]
async fn projection_retired_owner_selection_uses_partial_index_without_payload_scan() {
    let temp = TempDir::new().unwrap();
    let store = open(&temp, OpenMode::Create).await;
    seed(&store).await;
    let mut explain = sqlx::QueryBuilder::<sqlx::Sqlite>::new("EXPLAIN QUERY PLAN ");
    explain.push(SELECTION);
    let details = explain
        .build()
        .bind(id().as_str())
        .bind(generation(3).as_bytes().as_slice())
        .bind(generation(3).as_bytes().as_slice())
        .fetch_all(store.pool())
        .await
        .unwrap();
    let details: Vec<String> = details
        .iter()
        .map(|row| row.try_get("detail").unwrap())
        .collect();
    assert!(
        details.iter().any(|detail| detail.contains("SEARCH")
            && detail.contains("radroots_runtime_projection_retired_generation_idx")),
        "{details:?}"
    );
    assert!(
        !details
            .iter()
            .any(|detail| detail.contains("SCAN") || detail.contains("TEMP B-TREE")),
        "{details:?}"
    );
    assert_eq!(
        store
            .projection_document(id(), generation(1), "row.000".into())
            .await
            .unwrap()
            .unwrap()
            .value(),
        &[1]
    );
    store.close().await.unwrap();
}

#[tokio::test]
async fn projection_obsolete_snapshot_collection_preserves_legacy_custody_and_requires_safe_scope()
{
    let temp = TempDir::new().unwrap();
    let store = open(&temp, OpenMode::Create).await;
    let memory = MemoryStorage::new(SourceGeneration::new([9; 32]).unwrap());
    let legacy = ProjectionId::parse("legacy_custody_fixture").unwrap();
    for backend in [&memory as &dyn Storage, &store as &dyn Storage] {
        let source = backend.source_token().await.unwrap();
        for target in [9, 10] {
            backend
                .put_projection_document(
                    legacy.clone(),
                    generation(target),
                    ProjectionDocument::new("custody".into(), vec![target; 16]).unwrap(),
                )
                .await
                .unwrap();
        }
        backend
            .checkpoint(
                ProjectionCheckpoint::new(legacy.clone(), generation(10), None, 1, 1).unwrap(),
            )
            .await
            .unwrap();
        for index in 1..=65 {
            backend
                .put_projection_snapshot(
                    ProjectionSnapshot::new(
                        legacy.clone(),
                        [index; 32],
                        generation(9),
                        1,
                        vec![7; 2],
                    )
                    .unwrap(),
                )
                .await
                .unwrap();
        }
        backend
            .put_projection_snapshot(
                ProjectionSnapshot::new(legacy.clone(), [200; 32], generation(10), 1, vec![8; 2])
                    .unwrap(),
            )
            .await
            .unwrap();
        let old = || {
            ProjectionCacheScope::new(legacy.clone(), generation(9), source, Some(generation(10)))
                .unwrap()
        };
        assert_eq!(
            backend.collect_obsolete_projection_snapshots(old()).await,
            Err(Error::ProjectionRevisionConflict)
        );
        let before = backend
            .configure_projection_cache(legacy.clone(), limits())
            .await
            .unwrap();
        assert_eq!(before.legacy_snapshots, 66);
        let current = || {
            ProjectionCacheScope::new(legacy.clone(), generation(10), source, Some(generation(10)))
                .unwrap()
        };
        assert_eq!(
            backend
                .collect_obsolete_projection_snapshots(current())
                .await,
            Err(Error::ProjectionRevisionConflict)
        );
        assert_eq!(
            backend
                .collect_obsolete_projection_snapshots(
                    ProjectionCacheScope::new(
                        legacy.clone(),
                        generation(9),
                        EventSourceToken::new(source.generation(), 1),
                        Some(generation(10))
                    )
                    .unwrap()
                )
                .await,
            Err(Error::SourceEpochChanged)
        );
        assert_eq!(
            backend
                .collect_obsolete_projection_snapshots(
                    ProjectionCacheScope::new(legacy.clone(), generation(9), source, None).unwrap()
                )
                .await,
            Err(Error::ProjectionRevisionConflict)
        );
        backend
            .put_projection_batch(
                ProjectionDocumentBatch::new(
                    ProjectionWriteScope::new(
                        legacy.clone(),
                        generation(11),
                        source,
                        Some(generation(10)),
                    )
                    .unwrap(),
                    vec![ProjectionDocument::new("staging".into(), vec![11]).unwrap()],
                )
                .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            backend
                .collect_obsolete_projection_snapshots(
                    ProjectionCacheScope::new(
                        legacy.clone(),
                        generation(11),
                        source,
                        Some(generation(10))
                    )
                    .unwrap()
                )
                .await,
            Err(Error::ProjectionRevisionConflict)
        );
        for (removed, more) in [(64, true), (1, false)] {
            let result = backend
                .collect_obsolete_projection_snapshots(old())
                .await
                .unwrap();
            assert_eq!((result.removed_rows, result.more), (removed, more));
            assert_eq!(result.removed_value_bytes, removed * 2);
        }
        let invalidated = backend
            .invalidate(
                ProjectionInvalidation::new(
                    legacy.clone(),
                    generation(10),
                    generation(12),
                    InvalidationReason::ProjectionGenerationChanged,
                    2,
                )
                .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(invalidated.health(), ProjectionHealth::Invalidated);
        let result = backend
            .collect_obsolete_projection_snapshots(current())
            .await
            .unwrap();
        assert_eq!(result.removed_rows, 1);
        assert_eq!(result.removed_value_bytes, 2);
        assert!(!result.more);
        assert_eq!(result.usage.legacy_snapshots, 0);
        assert_eq!(result.usage.protected_generations, 2);
        assert_eq!(backend.source_token().await.unwrap(), source);
        assert_eq!(
            ProjectionStore::status(backend, legacy.clone())
                .await
                .unwrap(),
            Some(invalidated)
        );
        for target in [9, 10] {
            assert_eq!(
                backend
                    .projection_document(legacy.clone(), generation(target), "custody".into())
                    .await
                    .unwrap()
                    .unwrap()
                    .value(),
                &[target; 16]
            );
        }
        assert_eq!(
            backend
                .projection_document(legacy.clone(), generation(11), "staging".into())
                .await
                .unwrap()
                .unwrap()
                .value(),
            &[11]
        );
    }
    store.close().await.unwrap();
    let readonly = open(&temp, OpenMode::ReadOnly).await;
    let source = radroots_storage::EventStore::source_token(&readonly)
        .await
        .unwrap();
    assert_eq!(
        readonly
            .collect_obsolete_projection_snapshots(
                ProjectionCacheScope::new(
                    legacy.clone(),
                    generation(9),
                    source,
                    Some(generation(10))
                )
                .unwrap()
            )
            .await,
        Err(Error::BackendUnavailable)
    );
    assert_eq!(
        readonly
            .projection_document(legacy, generation(9), "custody".into())
            .await
            .unwrap()
            .unwrap()
            .value(),
        &[9; 16]
    );
    readonly.close().await.unwrap();
}
