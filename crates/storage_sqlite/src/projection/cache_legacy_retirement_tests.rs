use super::*;
use crate::{OpenMode, OpenOptions, Paths};
use radroots_storage::{
    EventStore, ProjectionStore, Storage,
    event::{EventSourceToken, SourceGeneration},
    memory::MemoryStorage,
    projection::{
        InvalidationReason, ProjectionCheckpoint, ProjectionDocument, ProjectionGeneration,
        ProjectionInvalidation, RawSourceDigest, RebuildTicket, RebuildTicketId, RebuildTransition,
        batch::{ProjectionDocumentBatch, ProjectionWriteScope},
    },
};
use tempfile::TempDir;

fn legacy() -> ProjectionId {
    ProjectionId::parse("legacy_coordination").unwrap()
}
fn rows() -> ProjectionId {
    ProjectionId::parse("derived_rows").unwrap()
}
fn generation(value: u8) -> ProjectionGeneration {
    ProjectionGeneration::new([value; 32]).unwrap()
}
fn limits() -> ProjectionCacheLimits {
    ProjectionCacheLimits::new(67_108_864, 2, 32, 1_800_000).unwrap()
}
fn exact(source: EventSourceToken) -> ProjectionCacheScope {
    ProjectionCacheScope::new(legacy(), generation(9), source, Some(generation(9))).unwrap()
}
fn invalidation() -> ProjectionInvalidation {
    ProjectionInvalidation::new(
        legacy(),
        generation(9),
        generation(10),
        InvalidationReason::ProjectionGenerationChanged,
        1,
    )
    .unwrap()
}
async fn open(temp: &TempDir) -> SqliteStorage {
    SqliteStorage::open(
        OpenOptions::new(
            Paths::from_directory(temp.path()).unwrap(),
            OpenMode::Create,
        )
        .with_source_generation(SourceGeneration::new([9; 32]).unwrap(), 9)
        .unwrap(),
    )
    .await
    .unwrap()
}
async fn seed(backend: &dyn Storage) {
    backend
        .put_projection_document(
            legacy(),
            generation(9),
            ProjectionDocument::new("original".into(), b"legacy".to_vec()).unwrap(),
        )
        .await
        .unwrap();
    backend
        .put_projection_snapshot(
            ProjectionSnapshot::new(legacy(), [3; 32], generation(9), 1, b"snapshot".to_vec())
                .unwrap(),
        )
        .await
        .unwrap();
    backend
        .checkpoint(ProjectionCheckpoint::new(legacy(), generation(9), None, 1, 1).unwrap())
        .await
        .unwrap();
}
fn batch(
    source: EventSourceToken,
    target: u8,
    active: Option<ProjectionGeneration>,
) -> ProjectionDocumentBatch {
    ProjectionDocumentBatch::new(
        ProjectionWriteScope::new(rows(), generation(target), source, active).unwrap(),
        vec![ProjectionDocument::new("row".into(), vec![7]).unwrap()],
    )
    .unwrap()
}

#[tokio::test]
async fn projection_cache_legacy_retirement_releases_only_metadata_with_memory_sqlite_parity() {
    let temp = TempDir::new().unwrap();
    let store = open(&temp).await;
    let memory = MemoryStorage::new(SourceGeneration::new([9; 32]).unwrap());
    for backend in [&memory as &dyn Storage, &store as &dyn Storage] {
        seed(backend).await;
        let source = EventStore::source_token(backend).await.unwrap();
        backend
            .configure_projection_cache(rows(), limits())
            .await
            .unwrap();
        assert_eq!(
            backend.retire_projection_generation(exact(source)).await,
            Err(Error::ProjectionRevisionConflict)
        );
        assert_eq!(
            backend
                .retire_projection_generation(exact(EventSourceToken::new(source.generation(), 1)))
                .await,
            Err(Error::SourceEpochChanged)
        );
        assert_eq!(
            backend
                .retire_projection_generation(
                    ProjectionCacheScope::new(legacy(), generation(9), source, None).unwrap()
                )
                .await,
            Err(Error::ProjectionRevisionConflict)
        );
        backend
            .promote_projection_batch(
                batch(source, 1, None),
                ProjectionCheckpoint::new(rows(), generation(1), None, 1, 2).unwrap(),
            )
            .await
            .unwrap();
        let before = backend
            .configure_projection_cache(rows(), limits())
            .await
            .unwrap();
        assert_eq!(before.protected_generations, 2);
        assert_eq!(
            backend
                .put_projection_batch(batch(source, 2, Some(generation(1))))
                .await,
            Err(Error::SpaceInsufficient)
        );
        let status = backend.invalidate(invalidation()).await.unwrap();
        let retired = backend
            .retire_projection_generation(exact(source))
            .await
            .unwrap();
        assert_eq!(retired.protected_generations, 1);
        assert_eq!(retired.value_bytes, before.value_bytes);
        assert_eq!(
            ProjectionStore::status(backend, legacy()).await.unwrap(),
            Some(status.clone())
        );
        assert_eq!(status.health(), ProjectionHealth::Invalidated);
        assert_eq!(
            backend
                .retire_projection_generation(exact(source))
                .await
                .unwrap(),
            retired
        );
        assert_eq!(
            backend.collect_projection_cache(exact(source)).await,
            Err(Error::ProjectionRevisionConflict)
        );
        assert_eq!(
            backend
                .projection_document(legacy(), generation(9), "original".into())
                .await
                .unwrap()
                .unwrap()
                .value(),
            b"legacy"
        );
        assert_eq!(
            backend
                .projection_snapshot(legacy(), [3; 32])
                .await
                .unwrap()
                .unwrap()
                .value(),
            b"snapshot"
        );
        assert_eq!(
            backend
                .checkpoint(ProjectionCheckpoint::new(legacy(), generation(9), None, 1, 3).unwrap())
                .await,
            Err(Error::ProjectionRevisionConflict)
        );
        backend
            .put_projection_batch(batch(source, 2, Some(generation(1))))
            .await
            .unwrap();
        assert_eq!(
            backend
                .configure_projection_cache(rows(), limits())
                .await
                .unwrap()
                .protected_generations,
            2
        );
    }
}

#[tokio::test]
async fn projection_cache_legacy_retirement_preserves_refusal_and_counters_on_sqlite_reopen() {
    let temp = TempDir::new().unwrap();
    let store = open(&temp).await;
    seed(&store).await;
    let source = store.source_token().await.unwrap();
    store
        .configure_projection_cache(rows(), limits())
        .await
        .unwrap();
    let expected = store.invalidate(invalidation()).await.unwrap();
    let usage = store
        .retire_projection_generation(exact(source))
        .await
        .unwrap();
    assert_eq!(usage.protected_generations, 0);
    drop(store);
    let reopened = open(&temp).await;
    assert_eq!(
        ProjectionStore::status(&reopened, legacy()).await.unwrap(),
        Some(expected)
    );
    assert_eq!(
        reopened
            .configure_projection_cache(rows(), limits())
            .await
            .unwrap(),
        usage
    );
    assert_eq!(
        reopened
            .retire_projection_generation(exact(source))
            .await
            .unwrap(),
        usage
    );
    assert_eq!(
        reopened.collect_projection_cache(exact(source)).await,
        Err(Error::ProjectionRevisionConflict)
    );
    assert_eq!(
        reopened
            .projection_document(legacy(), generation(9), "original".into())
            .await
            .unwrap()
            .unwrap()
            .value(),
        b"legacy"
    );
    reopened
        .promote_projection_batch(
            batch(source, 1, None),
            ProjectionCheckpoint::new(rows(), generation(1), None, 1, 2).unwrap(),
        )
        .await
        .unwrap();
    reopened
        .put_projection_batch(batch(source, 2, Some(generation(1))))
        .await
        .unwrap();
    assert_eq!(
        reopened
            .configure_projection_cache(rows(), limits())
            .await
            .unwrap()
            .protected_generations,
        2
    );
}

#[tokio::test]
async fn projection_cache_legacy_retirement_refuses_requested_and_running_rebuilds() {
    let temp = TempDir::new().unwrap();
    let store = open(&temp).await;
    let memory = MemoryStorage::new(SourceGeneration::new([9; 32]).unwrap());
    for backend in [&memory as &dyn Storage, &store as &dyn Storage] {
        seed(backend).await;
        let source = EventStore::source_token(backend).await.unwrap();
        backend.invalidate(invalidation()).await.unwrap();
        let requested = backend
            .request_rebuild(
                RebuildTicket::requested(
                    RebuildTicketId::new([4; 16]).unwrap(),
                    invalidation(),
                    source.generation(),
                    None,
                    RawSourceDigest::new([1; 32]),
                )
                .unwrap(),
            )
            .await
            .unwrap();
        let before = ProjectionStore::status(backend, legacy()).await.unwrap();
        assert_eq!(
            backend.retire_projection_generation(exact(source)).await,
            Err(Error::ProjectionRevisionConflict)
        );
        assert_eq!(
            ProjectionStore::status(backend, legacy()).await.unwrap(),
            before
        );
        backend
            .transition_rebuild(RebuildTransition::start(
                requested.ticket_id(),
                requested.revision(),
                2,
            ))
            .await
            .unwrap();
        let before = ProjectionStore::status(backend, legacy()).await.unwrap();
        assert_eq!(
            backend.retire_projection_generation(exact(source)).await,
            Err(Error::ProjectionRevisionConflict)
        );
        assert_eq!(
            ProjectionStore::status(backend, legacy()).await.unwrap(),
            before
        );
    }
}

#[tokio::test]
async fn projection_cache_legacy_retirement_refuses_live_unconfigured_batch_registration() {
    let temp = TempDir::new().unwrap();
    let store = open(&temp).await;
    let memory = MemoryStorage::new(SourceGeneration::new([9; 32]).unwrap());
    for backend in [&memory as &dyn Storage, &store as &dyn Storage] {
        let source = EventStore::source_token(backend).await.unwrap();
        backend
            .promote_projection_batch(
                batch(source, 1, None),
                ProjectionCheckpoint::new(rows(), generation(1), None, 1, 2).unwrap(),
            )
            .await
            .unwrap();
        backend
            .invalidate(
                ProjectionInvalidation::new(
                    rows(),
                    generation(1),
                    generation(2),
                    InvalidationReason::OperatorRequested,
                    3,
                )
                .unwrap(),
            )
            .await
            .unwrap();
        let scope =
            ProjectionCacheScope::new(rows(), generation(1), source, Some(generation(1))).unwrap();
        assert_eq!(
            backend.retire_projection_generation(scope).await,
            Err(Error::ProjectionRevisionConflict)
        );
        assert_eq!(
            backend
                .projection_document(rows(), generation(1), "row".into())
                .await
                .unwrap()
                .unwrap()
                .value(),
            &[7]
        );
    }
}
