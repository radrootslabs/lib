use super::*;
use crate::{OpenMode, OpenOptions, Paths};
use radroots_storage::{
    EventStore, ProjectionStore,
    event::{EventSourceToken, SourceGeneration},
    memory::MemoryStorage,
    projection::{
        ProjectionCheckpoint, ProjectionDocument, ProjectionGeneration, ProjectionSnapshot,
        batch::{ProjectionDocumentBatch, ProjectionWriteScope},
    },
};
use tempfile::TempDir;
fn id() -> ProjectionId {
    ProjectionId::parse("cache_fixture").unwrap()
}
fn generation(value: u8) -> ProjectionGeneration {
    ProjectionGeneration::new([value; 32]).unwrap()
}
fn limits() -> ProjectionCacheLimits {
    ProjectionCacheLimits::new(67_108_864, 2, 32, 1_800_000).unwrap()
}
fn batch(
    source: EventSourceToken,
    active: Option<ProjectionGeneration>,
    target: u8,
    keys: &[&str],
) -> ProjectionDocumentBatch {
    ProjectionDocumentBatch::new(
        ProjectionWriteScope::new(id(), generation(target), source, active).unwrap(),
        keys.iter()
            .map(|key| ProjectionDocument::new((*key).into(), vec![7]).unwrap())
            .collect(),
    )
    .unwrap()
}
fn scope(
    source: EventSourceToken,
    active: Option<ProjectionGeneration>,
    target: u8,
) -> ProjectionCacheScope {
    ProjectionCacheScope::new(id(), generation(target), source, active).unwrap()
}
fn checkpoint(target: u8, count: u64) -> ProjectionCheckpoint {
    ProjectionCheckpoint::new(id(), generation(target), None, count, 1).unwrap()
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

#[tokio::test]
async fn projection_cache_protection_and_retirement_match_memory_with_bounded_collection() {
    let temp = TempDir::new().unwrap();
    let store = open(&temp).await;
    let memory = MemoryStorage::new(SourceGeneration::new([9; 32]).unwrap());
    let source = store.source_token().await.unwrap();
    for backend in [&memory as &dyn ProjectionStore, &store] {
        assert_eq!(
            backend
                .configure_projection_cache(id(), limits())
                .await
                .unwrap(),
            ProjectionCacheUsage::default()
        );
        backend
            .promote_projection_batch(batch(source, None, 1, &["active"]), checkpoint(1, 1))
            .await
            .unwrap();
        backend
            .put_projection_batch(batch(source, Some(generation(1)), 2, &["staging"]))
            .await
            .unwrap();
        assert_eq!(
            backend
                .configure_projection_cache(id(), limits())
                .await
                .unwrap(),
            ProjectionCacheUsage {
                value_bytes: 2,
                protected_generations: 2,
                legacy_snapshots: 0
            }
        );
        assert_eq!(
            backend
                .retire_projection_generation(scope(source, Some(generation(1)), 1))
                .await,
            Err(Error::ProjectionRevisionConflict)
        );
        assert_eq!(
            backend
                .collect_projection_cache(scope(source, Some(generation(1)), 1))
                .await
                .unwrap()
                .removed_rows,
            0
        );
        assert_eq!(
            backend
                .collect_projection_cache(scope(source, Some(generation(1)), 2))
                .await
                .unwrap()
                .removed_rows,
            0
        );
        backend
            .retire_projection_generation(scope(source, Some(generation(1)), 2))
            .await
            .unwrap();
        let collected = backend
            .collect_projection_cache(scope(source, Some(generation(1)), 2))
            .await
            .unwrap();
        assert_eq!(collected.removed_rows, 2);
        assert_eq!(collected.removed_value_bytes, 1);
        assert!(!collected.more);
        assert_eq!(
            collected.usage,
            ProjectionCacheUsage {
                value_bytes: 1,
                protected_generations: 1,
                legacy_snapshots: 0
            }
        );
        assert_eq!(
            backend
                .collect_projection_cache(scope(
                    EventSourceToken::new(source.generation(), 1),
                    Some(generation(1)),
                    2
                ))
                .await,
            Err(Error::SourceEpochChanged)
        );
        assert_eq!(
            backend
                .collect_projection_cache(scope(source, None, 2))
                .await,
            Err(Error::ProjectionRevisionConflict)
        );
        assert_eq!(
            backend
                .put_projection_document(
                    id(),
                    generation(9),
                    ProjectionDocument::new("legacy".into(), vec![7]).unwrap()
                )
                .await,
            Err(Error::ProjectionRevisionConflict)
        );
        let keys: Vec<_> = (0..64).map(|number| format!("key.{number:02}")).collect();
        let keys: Vec<_> = keys.iter().map(String::as_str).collect();
        backend
            .put_projection_batch(batch(source, Some(generation(1)), 3, &keys))
            .await
            .unwrap();
        backend
            .put_projection_batch(batch(source, Some(generation(1)), 3, &["key.64"]))
            .await
            .unwrap();
        backend
            .retire_projection_generation(scope(source, Some(generation(1)), 3))
            .await
            .unwrap();
        let first = backend
            .collect_projection_cache(scope(source, Some(generation(1)), 3))
            .await
            .unwrap();
        assert_eq!(first.removed_rows, 64);
        assert!(first.more);
        let last = backend
            .collect_projection_cache(scope(source, Some(generation(1)), 3))
            .await
            .unwrap();
        assert_eq!(last.removed_rows, 2);
        assert!(!last.more);
        assert_eq!(last.usage.value_bytes, 1);
    }
    store.close().await.unwrap();
}

fn value_batch(
    source: EventSourceToken,
    active: Option<ProjectionGeneration>,
    target: u8,
    key: &str,
    size: usize,
) -> ProjectionDocumentBatch {
    ProjectionDocumentBatch::new(
        ProjectionWriteScope::new(id(), generation(target), source, active).unwrap(),
        vec![ProjectionDocument::new(key.into(), vec![7; size]).unwrap()],
    )
    .unwrap()
}
fn query(target: u8) -> radroots_storage::projection::document_query::ProjectionDocumentQuery {
    use radroots_storage::projection::document_query::{
        ProjectionDocumentGenerations, ProjectionDocumentQuery,
    };
    ProjectionDocumentQuery::new(
        id(),
        ProjectionDocumentGenerations::Exact(generation(target)),
        64,
    )
    .unwrap()
}
fn snapshot(number: u8, target: u8, created: u64, size: usize) -> ProjectionSnapshot {
    ProjectionSnapshot::new(
        id(),
        [number; 32],
        generation(target),
        created,
        vec![8; size],
    )
    .unwrap()
}

#[tokio::test]
async fn projection_cache_actual_64mib_includes_legacy_active_and_staging_and_rolls_back_overflow()
{
    let temp = TempDir::new().unwrap();
    let store = open(&temp).await;
    let memory = MemoryStorage::new(SourceGeneration::new([9; 32]).unwrap());
    let source = store.source_token().await.unwrap();
    for backend in [&memory as &dyn ProjectionStore, &store] {
        backend
            .put_projection_snapshot(snapshot(1, 9, 1, 16_777_216))
            .await
            .unwrap();
        backend
            .configure_projection_cache(id(), limits())
            .await
            .unwrap();
        for number in 0..4 {
            backend
                .put_projection_batch(value_batch(
                    source,
                    None,
                    1,
                    &format!("active.{number}"),
                    4_194_304,
                ))
                .await
                .unwrap();
        }
        backend
            .promote_projection_batch(batch(source, None, 1, &[]), checkpoint(1, 4))
            .await
            .unwrap();
        for number in 0..8 {
            backend
                .put_projection_batch(value_batch(
                    source,
                    Some(generation(1)),
                    2,
                    &format!("staging.{number}"),
                    4_194_304,
                ))
                .await
                .unwrap();
        }
        let full = backend
            .configure_projection_cache(id(), limits())
            .await
            .unwrap();
        assert_eq!(
            full,
            ProjectionCacheUsage {
                value_bytes: 67_108_864,
                protected_generations: 2,
                legacy_snapshots: 1
            }
        );
        assert_eq!(
            backend
                .put_projection_batch(value_batch(source, Some(generation(1)), 2, "one_over", 1))
                .await,
            Err(Error::SpaceInsufficient)
        );
        assert_eq!(
            backend
                .configure_projection_cache(id(), limits())
                .await
                .unwrap(),
            full
        );
        assert_eq!(
            backend
                .query_projection_documents(query(2))
                .await
                .unwrap()
                .records()
                .len(),
            4
        ); // existing page ceiling selects four4MiB values
        backend
            .collect_projection_cache(scope(source, Some(generation(1)), 9))
            .await
            .unwrap();
        backend
            .promote_projection_batch(batch(source, Some(generation(1)), 2, &[]), checkpoint(2, 8))
            .await
            .unwrap();
        let collected = backend
            .collect_projection_cache(scope(source, Some(generation(2)), 1))
            .await
            .unwrap();
        assert_eq!(collected.removed_value_bytes, 16_777_216);
        assert_eq!(
            collected.usage,
            ProjectionCacheUsage {
                value_bytes: 33_554_432,
                protected_generations: 1,
                legacy_snapshots: 0
            }
        );
    }
    let mut transaction = store.pool().begin().await.unwrap();
    let logical = usage(&mut transaction).await.unwrap();
    transaction.rollback().await.unwrap();
    let page_count: i64 = sqlx::query_scalar("PRAGMA page_count")
        .fetch_one(store.pool())
        .await
        .unwrap();
    let page_size: i64 = sqlx::query_scalar("PRAGMA page_size")
        .fetch_one(store.pool())
        .await
        .unwrap();
    let wal = temp.path().join("runtime.sqlite-wal");
    assert!(page_count * page_size > logical.value_bytes as i64);
    assert!(std::fs::metadata(wal).unwrap().len() > 0);
    store.close().await.unwrap();
}

#[tokio::test]
async fn projection_cache_quota_and_third_protected_wal_races_preserve_transactional_counts() {
    let temp = TempDir::new().unwrap();
    let store = open(&temp).await;
    let source = store.source_token().await.unwrap();
    let small = ProjectionCacheLimits::new(3, 2, 32, 1_800_000).unwrap();
    store.configure_projection_cache(id(), small).await.unwrap();
    store
        .promote_projection_batch(batch(source, None, 1, &["active"]), checkpoint(1, 1))
        .await
        .unwrap();
    let (left, right) = tokio::join!(
        store.put_projection_batch(value_batch(source, Some(generation(1)), 2, "left", 2)),
        store.put_projection_batch(value_batch(source, Some(generation(1)), 2, "right", 2))
    );
    assert!(matches!(
        (&left, &right),
        (Ok(()), Err(Error::SpaceInsufficient)) | (Err(Error::SpaceInsufficient), Ok(()))
    ));
    assert_eq!(
        store.configure_projection_cache(id(), small).await.unwrap(),
        ProjectionCacheUsage {
            value_bytes: 3,
            protected_generations: 2,
            legacy_snapshots: 0
        }
    );
    assert_eq!(
        store
            .query_projection_documents(query(2))
            .await
            .unwrap()
            .records()
            .len(),
        1
    );
    store
        .retire_projection_generation(scope(source, Some(generation(1)), 2))
        .await
        .unwrap();
    store
        .collect_projection_cache(scope(source, Some(generation(1)), 2))
        .await
        .unwrap();
    let other = ProjectionId::parse("other_cache").unwrap();
    store
        .configure_projection_cache(other.clone(), small)
        .await
        .unwrap();
    let second = ProjectionDocumentBatch::new(
        ProjectionWriteScope::new(other.clone(), generation(3), source, None).unwrap(),
        vec![ProjectionDocument::new("other".into(), vec![7]).unwrap()],
    )
    .unwrap();
    let (left, right) = tokio::join!(
        store.put_projection_batch(batch(source, Some(generation(1)), 2, &["new"])),
        store.put_projection_batch(second)
    );
    assert!(matches!(
        (&left, &right),
        (Ok(()), Err(Error::SpaceInsufficient)) | (Err(Error::SpaceInsufficient), Ok(()))
    ));
    assert_eq!(
        store
            .configure_projection_cache(id(), small)
            .await
            .unwrap()
            .protected_generations,
        2
    );
    store.close().await.unwrap();
}

#[tokio::test]
async fn projection_cache_legacy_32_33_pressure_no_new_snapshots_and_no_sliding_replay() {
    let temp = TempDir::new().unwrap();
    let store = open(&temp).await;
    let memory = MemoryStorage::new(SourceGeneration::new([9; 32]).unwrap());
    let source = store.source_token().await.unwrap();
    let created = now().unwrap();
    for backend in [&memory as &dyn ProjectionStore, &store] {
        backend.checkpoint(checkpoint(1, 0)).await.unwrap();
        for number in 1..=33 {
            backend
                .put_projection_snapshot(snapshot(number, 1, created, 1))
                .await
                .unwrap();
        }
        assert_eq!(
            backend
                .configure_projection_cache(id(), limits())
                .await
                .unwrap()
                .legacy_snapshots,
            33
        );
        let replay = snapshot(2, 1, created, 1);
        backend
            .put_projection_snapshot(replay.clone())
            .await
            .unwrap();
        assert_eq!(
            backend.projection_snapshot(id(), [2; 32]).await.unwrap(),
            Some(replay)
        );
        assert_eq!(
            backend
                .put_projection_snapshot(snapshot(34, 1, created, 1))
                .await,
            Err(Error::ProjectionRevisionConflict)
        );
        assert_eq!(
            backend
                .put_projection_snapshot(snapshot(2, 1, created + 1, 1))
                .await,
            Err(Error::CorruptProjectionDocument)
        );
        let result = backend
            .collect_projection_cache(scope(source, Some(generation(1)), 1))
            .await
            .unwrap();
        assert_eq!(result.removed_rows, 1);
        assert!(!result.more);
        assert_eq!(result.usage.legacy_snapshots, 32);
        let foreign = ProjectionId::parse("legacy_other").unwrap();
        let extra =
            ProjectionSnapshot::new(foreign, [1; 32], generation(1), created, vec![8]).unwrap();
        assert_eq!(
            backend.put_projection_snapshot(extra).await,
            Err(Error::SpaceInsufficient)
        );
        assert_eq!(
            backend
                .put_projection_snapshot(snapshot(1, 1, created, 1))
                .await,
            Err(Error::ProjectionRevisionConflict)
        );
        assert_eq!(
            backend
                .configure_projection_cache(
                    id(),
                    ProjectionCacheLimits::new(67_108_865, 2, 32, 1_800_000).unwrap()
                )
                .await,
            Err(Error::ProjectionRevisionConflict)
        );
    }
    store.close().await.unwrap();
}

#[tokio::test]
async fn projection_cache_stale_and_invalidated_active_retirement_allows_successor_without_canonical_loss()
 {
    use radroots_storage::projection::{InvalidationReason, ProjectionInvalidation};
    let temp = TempDir::new().unwrap();
    let store = open(&temp).await;
    let memory = MemoryStorage::new(SourceGeneration::new([9; 32]).unwrap());
    for backend in [&memory as &dyn ProjectionStore, &store] {
        let source = EventSourceToken::new(SourceGeneration::new([9; 32]).unwrap(), 0);
        backend
            .configure_projection_cache(id(), limits())
            .await
            .unwrap();
        backend
            .promote_projection_batch(batch(source, None, 1, &["active"]), checkpoint(1, 1))
            .await
            .unwrap();
        backend
            .invalidate(
                ProjectionInvalidation::new(
                    id(),
                    generation(1),
                    generation(2),
                    InvalidationReason::OperatorRequested,
                    1,
                )
                .unwrap(),
            )
            .await
            .unwrap();
        let retired = backend
            .retire_projection_generation(scope(source, Some(generation(1)), 1))
            .await
            .unwrap();
        assert_eq!(retired.protected_generations, 0);
        assert_eq!(backend.status(id()).await.unwrap(), None);
        backend
            .promote_projection_batch(batch(source, None, 2, &["successor"]), checkpoint(2, 1))
            .await
            .unwrap();
        backend
            .collect_projection_cache(scope(source, Some(generation(2)), 1))
            .await
            .unwrap();
        assert_eq!(
            backend
                .query_projection_documents_at(query(2), source, generation(2))
                .await
                .unwrap()
                .records()
                .len(),
            1
        );
    }
    sqlx::query("UPDATE radroots_runtime_source_generations SET mutation_epoch=mutation_epoch+1 WHERE state='active'").execute(store.pool()).await.unwrap();
    let current = store.source_token().await.unwrap();
    assert_eq!(
        store
            .retire_projection_generation(scope(
                EventSourceToken::new(current.generation(), 0),
                Some(generation(2)),
                2
            ))
            .await,
        Err(Error::SourceEpochChanged)
    );
    store
        .retire_projection_generation(scope(current, Some(generation(2)), 2))
        .await
        .unwrap();
    store
        .promote_projection_batch(batch(current, None, 3, &["fresh"]), checkpoint(3, 1))
        .await
        .unwrap();
    store
        .collect_projection_cache(scope(current, Some(generation(3)), 2))
        .await
        .unwrap();
    assert_eq!(store.source_token().await.unwrap(), current);
    store.close().await.unwrap();
}

#[tokio::test]
async fn projection_cache_gc_cancellation_rollback_read_race_reopen_and_readonly_preserve_active() {
    let temp = TempDir::new().unwrap();
    let store = open(&temp).await;
    let source = store.source_token().await.unwrap();
    store
        .configure_projection_cache(id(), limits())
        .await
        .unwrap();
    store
        .promote_projection_batch(batch(source, None, 1, &["active"]), checkpoint(1, 1))
        .await
        .unwrap();
    store
        .put_projection_batch(batch(
            source,
            Some(generation(1)),
            2,
            &["obsolete.a", "obsolete.b"],
        ))
        .await
        .unwrap();
    store
        .retire_projection_generation(scope(source, Some(generation(1)), 2))
        .await
        .unwrap();
    let mut connections = Vec::new();
    for _ in 0..4 {
        connections.push(store.pool().acquire().await.unwrap());
    }
    let mut pending = store.collect_projection_cache(scope(source, Some(generation(1)), 2));
    assert!(matches!(
        pending
            .as_mut()
            .poll(&mut std::task::Context::from_waker(std::task::Waker::noop())),
        std::task::Poll::Pending
    ));
    drop(pending);
    drop(connections);
    assert_eq!(
        store
            .query_projection_documents(query(2))
            .await
            .unwrap()
            .records()
            .len(),
        2
    );
    sqlx::raw_sql("CREATE TRIGGER fixture_cache_delete BEFORE DELETE ON radroots_runtime_projection_documents WHEN OLD.document_key='obsolete.b' BEGIN SELECT RAISE(ABORT,'fixture'); END;").execute(store.pool()).await.unwrap();
    let before = store
        .configure_projection_cache(id(), limits())
        .await
        .unwrap();
    assert!(
        store
            .collect_projection_cache(scope(source, Some(generation(1)), 2))
            .await
            .is_err()
    );
    assert_eq!(
        store
            .configure_projection_cache(id(), limits())
            .await
            .unwrap(),
        before
    );
    assert_eq!(
        store
            .query_projection_documents(query(2))
            .await
            .unwrap()
            .records()
            .len(),
        2
    );
    sqlx::raw_sql("DROP TRIGGER fixture_cache_delete")
        .execute(store.pool())
        .await
        .unwrap();
    let (read, collected) = tokio::join!(
        store.query_projection_documents_at(query(1), source, generation(1)),
        store.collect_projection_cache(scope(source, Some(generation(1)), 2))
    );
    assert_eq!(read.unwrap().records().len(), 1);
    assert_eq!(collected.unwrap().removed_rows, 3);
    store.close().await.unwrap();
    let reopened = SqliteStorage::open(OpenOptions::new(
        Paths::from_directory(temp.path()).unwrap(),
        OpenMode::ReadWriteExisting,
    ))
    .await
    .unwrap();
    assert_eq!(
        reopened
            .configure_projection_cache(id(), limits())
            .await
            .unwrap(),
        ProjectionCacheUsage {
            value_bytes: 1,
            protected_generations: 1,
            legacy_snapshots: 0
        }
    );
    reopened.close().await.unwrap();
    let readonly = SqliteStorage::open(OpenOptions::new(
        Paths::from_directory(temp.path()).unwrap(),
        OpenMode::ReadOnly,
    ))
    .await
    .unwrap();
    assert_eq!(
        readonly
            .collect_projection_cache(scope(source, Some(generation(1)), 2))
            .await,
        Err(Error::BackendUnavailable)
    );
    assert_eq!(
        readonly
            .retire_projection_generation(scope(source, Some(generation(1)), 1))
            .await,
        Err(Error::BackendUnavailable)
    );
    assert_eq!(
        readonly.configure_projection_cache(id(), limits()).await,
        Err(Error::BackendUnavailable)
    );
    assert_eq!(
        readonly
            .query_projection_documents_at(query(1), source, generation(1))
            .await
            .unwrap()
            .records()
            .len(),
        1
    );
    readonly.close().await.unwrap();
}

#[tokio::test]
async fn projection_cache_owner_time_highwater_expires_future_and_rollback_without_resurrection() {
    let temp = TempDir::new().unwrap();
    let store = open(&temp).await;
    let created = now().unwrap();
    let source = store.source_token().await.unwrap();
    for (number, time) in [(1, 1), (2, created), (3, created + 3_600_000)] {
        store
            .put_projection_snapshot(snapshot(number, 1, time, 1))
            .await
            .unwrap();
    }
    store.checkpoint(checkpoint(1, 0)).await.unwrap();
    store
        .configure_projection_cache(id(), limits())
        .await
        .unwrap();
    assert!(
        store
            .projection_snapshot(id(), [2; 32])
            .await
            .unwrap()
            .is_some()
    );
    assert_eq!(
        store.projection_snapshot(id(), [3; 32]).await.unwrap(),
        None
    );
    let first = store
        .collect_projection_cache(scope(source, Some(generation(1)), 1))
        .await
        .unwrap();
    assert_eq!(first.removed_rows, 2);
    assert!(!first.more);
    let mut transaction = store.pool().begin_with("BEGIN IMMEDIATE").await.unwrap();
    assert_eq!(
        time(&mut transaction, created + 1_800_000).await.unwrap(),
        created + 1_800_000
    );
    assert_eq!(
        time(&mut transaction, created - 1).await.unwrap(),
        created + 1_800_000
    );
    transaction.commit().await.unwrap();
    assert_eq!(
        store.projection_snapshot(id(), [2; 32]).await.unwrap(),
        None
    );
    store
        .put_projection_snapshot(snapshot(2, 1, created, 1))
        .await
        .unwrap(); // exact replay cannot renew time
    assert_eq!(
        store.projection_snapshot(id(), [2; 32]).await.unwrap(),
        None
    );
    store.close().await.unwrap();
    let reopened = SqliteStorage::open(OpenOptions::new(
        Paths::from_directory(temp.path()).unwrap(),
        OpenMode::ReadWriteExisting,
    ))
    .await
    .unwrap();
    assert_eq!(
        reopened.projection_snapshot(id(), [2; 32]).await.unwrap(),
        None
    );
    assert_eq!(
        reopened
            .collect_projection_cache(scope(source, Some(generation(1)), 1))
            .await
            .unwrap()
            .removed_rows,
        1
    );
    assert_eq!(
        reopened
            .put_projection_snapshot(snapshot(2, 1, created, 1))
            .await,
        Err(Error::ProjectionRevisionConflict)
    );
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn projection_cache_snapshot_scalar_length_and_digest_admission_precedes_payload_hydration() {
    let temp = TempDir::new().unwrap();
    let store = open(&temp).await;
    for (number, size) in [(1, 0), (2, 16_777_217)] {
        let mut connection = store.pool().acquire().await.unwrap();
        sqlx::raw_sql("PRAGMA ignore_check_constraints=ON")
            .execute(&mut *connection)
            .await
            .unwrap();
        sqlx::query("INSERT INTO radroots_runtime_projection_snapshots(projection_id,snapshot_id,generation,created_at_unix_ms,value,value_sha256) VALUES(?,?,?,1,zeroblob(?),zeroblob(32))").bind(id().as_str()).bind([number;32].as_slice()).bind(generation(1).as_bytes().as_slice()).bind(size).execute(&mut *connection).await.unwrap();
        sqlx::raw_sql("PRAGMA ignore_check_constraints=OFF")
            .execute(&mut *connection)
            .await
            .unwrap();
        drop(connection);
        assert_eq!(
            store.projection_snapshot(id(), [number; 32]).await,
            Err(Error::CorruptProjectionDocument)
        );
    }
    sqlx::query("INSERT INTO radroots_runtime_projection_snapshots(projection_id,snapshot_id,generation,created_at_unix_ms,value,value_sha256) VALUES(?,?,?,1,x'01',zeroblob(32))").bind(id().as_str()).bind([3;32].as_slice()).bind(generation(1).as_bytes().as_slice()).execute(store.pool()).await.unwrap();
    assert_eq!(
        store.projection_snapshot(id(), [3; 32]).await,
        Err(Error::CorruptProjectionDocument)
    );
    store
        .configure_projection_cache(id(), limits())
        .await
        .unwrap();
    assert_eq!(
        store.projection_snapshot(id(), [2; 32]).await.unwrap(),
        None
    ); // expired oversized row is rejected before value fetch
    store.close().await.unwrap();
}

fn review_admission(verified: bool) -> radroots_storage::event::EventAdmission {
    use radroots_event::{SignedEvent, admission::RawEvent, wire::Nip01EventWire};
    use radroots_storage::event::EventAdmission;
    use radroots_transport::{
        Target, TransportId,
        source::{EventProvenance, ObservedEvent},
    };
    struct FixtureVerifier;
    impl radroots_event::admission::SignatureVerifier for FixtureVerifier {
        fn verify_signature(
            &self,
            _event: &radroots_event::Event,
        ) -> Result<(), radroots_event::Error> {
            Ok(())
        }
    }
    let mut wire = Nip01EventWire {
        id: "0".repeat(64),
        pubkey: "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df".into(),
        created_at: 17,
        kind: 0,
        tags: vec![],
        content: r#"{"name":"review"}"#.into(),
        sig: "42".repeat(64),
        extra: Default::default(),
    };
    wire.id = wire.computed_event_id().unwrap().to_hex();
    let raw = serde_json::to_string(&wire).unwrap();
    let event = SignedEvent::from_wire_verified_id(wire, raw).unwrap();
    let target = Target::new(TransportId::NOSTR, "wss://fixture.example").unwrap();
    let observed = ObservedEvent::new(
        event.clone(),
        EventProvenance::new(TransportId::NOSTR, target.fingerprint().clone(), 1).unwrap(),
    );
    if verified {
        let verified = RawEvent::new(event.envelope().clone())
            .verify_id()
            .unwrap()
            .verify_signature(&FixtureVerifier)
            .unwrap();
        EventAdmission::verified(observed, verified).unwrap()
    } else {
        EventAdmission::raw(observed)
    }
}

#[tokio::test]
async fn projection_cache_collection_preserves_canonical_authored_intent_receipt_custody_and_foreign_projection()
 {
    use radroots_storage::journal::OperationId;
    use radroots_storage::{
        Storage,
        atomic::{
            AtomicCommit, AtomicCommitDigest, AtomicCommitId, AtomicWorkflow, CommitIngested,
        },
        authored_draft::{AuthoredDraft, AuthoredDraftId, AuthoredDraftStage},
        journal::{IdempotencyDigest, IdempotencyKey, OperationInstanceId, PrepareOperation},
        private_artifact::{
            ArtifactCommitment, ArtifactKind, ArtifactSchemaId, DurableSecretReference,
            PrivateArtifactId, PrivateArtifactMetadata, RetentionPolicy,
        },
    };
    let temp = TempDir::new().unwrap();
    let store = open(&temp).await;
    let memory = MemoryStorage::new(SourceGeneration::new([9; 32]).unwrap());
    for backend in [&memory as &dyn Storage, &store] {
        let request = AtomicCommit::new(
            AtomicCommitId::new([5; 16]).unwrap(),
            AtomicCommitDigest::new([5; 32]),
            1,
            AtomicWorkflow::Ingested(Box::new(CommitIngested::new(review_admission(false), None))),
        )
        .unwrap();
        let receipt = backend.commit(request.clone()).await.unwrap();
        let source = backend.source_token().await.unwrap();
        let draft = AuthoredDraft::initial(
            AuthoredDraftId::new([1; 16]).unwrap(),
            [3; 32],
            "fixture.preserved.v1",
            b"authored intent".to_vec(),
            AuthoredDraftStage::Draft,
            None,
            1,
        )
        .unwrap();
        backend
            .append_authored_draft(draft.clone(), None)
            .await
            .unwrap();
        let intent = PrepareOperation::new(
            OperationInstanceId::new([1; 16]).unwrap(),
            OperationId::SyncPush,
            IdempotencyKey::parse("cache-preserve").unwrap(),
            IdempotencyDigest::new([1; 32]),
            1,
        )
        .unwrap();
        backend.prepare(intent.clone()).await.unwrap();
        let custody = PrivateArtifactMetadata::new(
            PrivateArtifactId::new([1; 16]).unwrap(),
            ArtifactKind::parse("fixture.custody").unwrap(),
            ArtifactSchemaId::parse("fixture.custody.v1").unwrap(),
            ArtifactCommitment::new([1; 32]),
            1,
            DurableSecretReference::new("memory", "fixture", 1).unwrap(),
            RetentionPolicy::indefinite(),
            1,
        )
        .unwrap();
        backend.put_metadata(custody.clone()).await.unwrap();
        let foreign = ProjectionId::parse("settings_and_recovery").unwrap();
        backend
            .put_projection_document(
                foreign.clone(),
                generation(1),
                ProjectionDocument::new("keep".into(), vec![7]).unwrap(),
            )
            .await
            .unwrap();
        backend
            .configure_projection_cache(id(), limits())
            .await
            .unwrap();
        backend
            .put_projection_batch(batch(source, None, 1, &["retire"]))
            .await
            .unwrap();
        backend
            .retire_projection_generation(scope(source, None, 1))
            .await
            .unwrap();
        assert_eq!(
            backend
                .collect_projection_cache(
                    ProjectionCacheScope::new(foreign.clone(), generation(1), source, None)
                        .unwrap()
                )
                .await,
            Err(Error::ProjectionRevisionConflict)
        );
        assert_eq!(
            backend
                .collect_projection_cache(scope(source, None, 1))
                .await
                .unwrap()
                .removed_rows,
            2
        );
        assert_eq!(backend.source_token().await.unwrap(), source);
        assert_eq!(
            backend.receipt(request.commit_id()).await.unwrap(),
            Some(receipt)
        );
        assert_eq!(
            backend.authored_draft_head(draft.draft_id()).await.unwrap(),
            Some(draft)
        );
        assert!(
            backend
                .by_idempotency_key(OperationId::SyncPush, intent.idempotency_key().clone())
                .await
                .unwrap()
                .is_some()
        );
        assert_eq!(
            backend.metadata(custody.artifact_id()).await.unwrap(),
            Some(custody)
        );
        assert_eq!(
            backend
                .projection_document(foreign, generation(1), "keep".into())
                .await
                .unwrap()
                .unwrap()
                .value(),
            &[7]
        );
    }
    store.close().await.unwrap();
}

#[tokio::test]
async fn projection_cache_63_64_registration_boundary_reports_only_actual_remaining_work() {
    let temp = TempDir::new().unwrap();
    let store = open(&temp).await;
    let memory = MemoryStorage::new(SourceGeneration::new([9; 32]).unwrap());
    let source = store.source_token().await.unwrap();
    for backend in [&memory as &dyn ProjectionStore, &store] {
        backend
            .configure_projection_cache(id(), limits())
            .await
            .unwrap();
        for (target, count) in [(1, 63), (2, 64), (3, 0)] {
            let keys: Vec<_> = (0..count)
                .map(|number| format!("key.{number:02}"))
                .collect();
            let keys: Vec<_> = keys.iter().map(String::as_str).collect();
            backend
                .put_projection_batch(batch(source, None, target, &keys))
                .await
                .unwrap();
            backend
                .retire_projection_generation(scope(source, None, target))
                .await
                .unwrap();
            let first = backend
                .collect_projection_cache(scope(source, None, target))
                .await
                .unwrap();
            assert_eq!(first.removed_rows, (count + 1).min(64));
            assert_eq!(first.more, count == 64);
            if first.more {
                let second = backend
                    .collect_projection_cache(scope(source, None, target))
                    .await
                    .unwrap();
                assert_eq!(second.removed_rows, 1);
                assert!(!second.more);
            }
            assert_eq!(
                backend
                    .configure_projection_cache(id(), limits())
                    .await
                    .unwrap(),
                ProjectionCacheUsage::default()
            );
        }
    }
    store.close().await.unwrap();
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

#[tokio::test]
async fn projection_cache_legacy_completion_preserves_registry_capacity_and_atomic_refusal_in_both_providers()
 {
    for (registered, capacity) in [(true, 2), (true, 3), (false, 2)] {
        let temp = TempDir::new().unwrap();
        let store = open(&temp).await;
        let memory = MemoryStorage::new(SourceGeneration::new([9; 32]).unwrap());
        for backend in [&memory as &dyn ProjectionStore, &store] {
            legacy_cache_completion(backend, registered, capacity).await;
        }
        store.close().await.unwrap();
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

#[tokio::test]
async fn projection_cache_batch_replacement_has_one_aggregate_policy_in_both_orders_and_providers()
{
    for (quota, initial, replacement) in [
        (4, [2, 2], [3, 1]),
        (4, [2, 2], [2, 1]),
        (6, [4, 4], [5, 2]),
        (6, [4, 4], [5, 4]),
        (6, [1, 1], [2, 2]),
    ] {
        for reverse in [false, true] {
            let temp = TempDir::new().unwrap();
            let store = open(&temp).await;
            let memory = MemoryStorage::new(SourceGeneration::new([9; 32]).unwrap());
            for backend in [&memory as &dyn ProjectionStore, &store] {
                cache_batch_replacement(backend, quota, initial, replacement, reverse).await;
            }
            store.close().await.unwrap();
        }
    }
}

#[tokio::test]
async fn projection_cache_batch_reservation_faults_roll_back_rows_counters_registration_and_status()
{
    for corrupt_reservation in [false, true] {
        let temp = TempDir::new().unwrap();
        let store = open(&temp).await;
        let source = store.source_token().await.unwrap();
        let batch = |sizes: [usize; 2]| {
            ProjectionDocumentBatch::new(
                ProjectionWriteScope::new(id(), generation(1), source, None).unwrap(),
                vec![
                    ProjectionDocument::new("a".into(), vec![1; sizes[0]]).unwrap(),
                    ProjectionDocument::new("b".into(), vec![2; sizes[1]]).unwrap(),
                ],
            )
            .unwrap()
        };
        store.put_projection_batch(batch([2, 2])).await.unwrap();
        let limits = ProjectionCacheLimits::new(4, 2, 32, 1_800_000).unwrap();
        let before = store
            .configure_projection_cache(id(), limits)
            .await
            .unwrap();
        let trigger = if corrupt_reservation {
            "CREATE TRIGGER fixture_batch_fault AFTER UPDATE OF value ON radroots_runtime_projection_documents WHEN NEW.document_key='a' BEGIN UPDATE radroots_runtime_projection_cache_budget SET batch_value_ceiling=batch_value_ceiling+1 WHERE singleton=1; END;"
        } else {
            "CREATE TRIGGER fixture_batch_fault BEFORE UPDATE OF value ON radroots_runtime_projection_documents WHEN NEW.document_key='a' BEGIN SELECT RAISE(ABORT,'fixture batch abort'); END;"
        };
        sqlx::raw_sql(trigger).execute(store.pool()).await.unwrap();
        let result = store
            .promote_projection_batch(batch([3, 1]), checkpoint(1, 2))
            .await;
        if corrupt_reservation {
            assert_eq!(result, Err(Error::CorruptProjectionRecord));
        } else {
            assert!(result.is_err());
        }
        assert_eq!(ProjectionStore::status(&store, id()).await.unwrap(), None);
        assert_eq!(
            store
                .configure_projection_cache(id(), limits)
                .await
                .unwrap(),
            before
        );
        for (key, value) in [("a", 1), ("b", 2)] {
            assert_eq!(
                store
                    .projection_document(id(), generation(1), key.into())
                    .await
                    .unwrap()
                    .unwrap()
                    .value(),
                vec![value; 2]
            );
        }
        let row = sqlx::query("SELECT document_rows,value_bytes,phase FROM radroots_runtime_projection_generations WHERE projection_id=? AND generation=?")
            .bind(id().as_str()).bind(generation(1).as_bytes().as_slice()).fetch_one(store.pool()).await.unwrap();
        assert_eq!(row.get::<i64, _>("document_rows"), 2);
        assert_eq!(row.get::<i64, _>("value_bytes"), 4);
        assert_eq!(row.get::<String, _>("phase"), "staging");
        let ceiling: Option<i64> = sqlx::query_scalar("SELECT batch_value_ceiling FROM radroots_runtime_projection_cache_budget WHERE singleton=1").fetch_one(store.pool()).await.unwrap();
        assert_eq!(ceiling, None);
        sqlx::raw_sql("DROP TRIGGER fixture_batch_fault")
            .execute(store.pool())
            .await
            .unwrap();
        sqlx::query("UPDATE radroots_runtime_projection_cache_budget SET batch_value_ceiling=4 WHERE singleton=1").execute(store.pool()).await.unwrap();
        assert_eq!(
            store
                .promote_projection_batch(batch([3, 1]), checkpoint(1, 2))
                .await,
            Err(Error::CorruptProjectionRecord)
        );
        assert_eq!(ProjectionStore::status(&store, id()).await.unwrap(), None);
        assert_eq!(
            store
                .configure_projection_cache(id(), limits)
                .await
                .unwrap(),
            before
        );
        sqlx::query("UPDATE radroots_runtime_projection_cache_budget SET batch_value_ceiling=NULL WHERE singleton=1").execute(store.pool()).await.unwrap();
        store
            .promote_projection_batch(batch([3, 1]), checkpoint(1, 2))
            .await
            .unwrap();
        let ceiling: Option<i64> = sqlx::query_scalar("SELECT batch_value_ceiling FROM radroots_runtime_projection_cache_budget WHERE singleton=1").fetch_one(store.pool()).await.unwrap();
        assert_eq!(ceiling, None);
        assert_eq!(
            store
                .configure_projection_cache(id(), limits)
                .await
                .unwrap(),
            before
        );
        store.close().await.unwrap();
        let reopened = open(&temp).await;
        assert_eq!(
            reopened
                .configure_projection_cache(id(), limits)
                .await
                .unwrap(),
            before
        );
        assert_eq!(
            reopened
                .projection_document(id(), generation(1), "a".into())
                .await
                .unwrap()
                .unwrap()
                .value(),
            vec![1; 3]
        );
        assert_eq!(
            ProjectionStore::status(&reopened, id())
                .await
                .unwrap()
                .unwrap()
                .generation(),
            generation(1)
        );
        let ceiling: Option<i64> = sqlx::query_scalar("SELECT batch_value_ceiling FROM radroots_runtime_projection_cache_budget WHERE singleton=1").fetch_one(reopened.pool()).await.unwrap();
        assert_eq!(ceiling, None);
        reopened.close().await.unwrap();
    }
}
