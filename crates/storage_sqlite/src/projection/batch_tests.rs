use super::*;
use crate::{OpenMode, OpenOptions, Paths};
use radroots_storage::{
    EventStore, ProjectionStore,
    event::SourceGeneration,
    memory::MemoryStorage,
    projection::{
        batch::ProjectionWriteScope,
        document_query::{ProjectionDocumentGenerations, ProjectionDocumentQuery},
    },
};
use tempfile::TempDir;

fn id() -> ProjectionId {
    ProjectionId::parse("fixture").unwrap()
}
fn generation(byte: u8) -> ProjectionGeneration {
    ProjectionGeneration::new([byte; 32]).unwrap()
}
fn document(key: &str) -> ProjectionDocument {
    ProjectionDocument::new(key.into(), key.as_bytes().to_vec()).unwrap()
}
fn scope(
    source: EventSourceToken,
    active: Option<ProjectionGeneration>,
    target: u8,
) -> ProjectionWriteScope {
    ProjectionWriteScope::new(id(), generation(target), source, active).unwrap()
}
fn batch(scope: ProjectionWriteScope, keys: &[&str]) -> ProjectionDocumentBatch {
    ProjectionDocumentBatch::new(scope, keys.iter().map(|key| document(key)).collect()).unwrap()
}
fn checkpoint(target: u8, count: u64) -> ProjectionCheckpoint {
    ProjectionCheckpoint::new(id(), generation(target), None, count, target as u64).unwrap()
}
fn query(target: u8) -> ProjectionDocumentQuery {
    ProjectionDocumentQuery::new(
        id(),
        ProjectionDocumentGenerations::Exact(generation(target)),
        64,
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

#[tokio::test]
async fn projection_atomic_batch_and_promotion_match_memory_and_survive_reopen() {
    let temp = TempDir::new().unwrap();
    let store = open(&temp).await;
    let memory = MemoryStorage::new(SourceGeneration::new([9; 32]).unwrap());
    let source = store.source_token().await.unwrap();
    for backend in [&store as &dyn ProjectionStore, &memory] {
        let first = scope(source, None, 1);
        backend
            .put_projection_batch(batch(first.clone(), &["a", "b"]))
            .await
            .unwrap();
        assert_eq!(backend.status(id()).await.unwrap(), None);
        assert_eq!(
            backend
                .promote_projection_batch(batch(first.clone(), &["c"]), checkpoint(1, 2))
                .await,
            Err(Error::ProjectionCheckpointMismatch)
        );
        assert_eq!(
            backend
                .query_projection_documents(query(1))
                .await
                .unwrap()
                .records()
                .len(),
            2
        );
        backend
            .promote_projection_batch(batch(first, &["c"]), checkpoint(1, 3))
            .await
            .unwrap();
        assert_eq!(
            backend
                .put_projection_document(id(), generation(1), document("a"))
                .await,
            Err(Error::ProjectionRevisionConflict)
        );
        let second = scope(source, Some(generation(1)), 2);
        backend
            .put_projection_batch(batch(second.clone(), &["x"]))
            .await
            .unwrap();
        assert_eq!(
            backend
                .put_projection_batch(batch(scope(source, Some(generation(1)), 3), &[]))
                .await,
            Err(Error::ProjectionRevisionConflict)
        );
        assert_eq!(
            backend
                .query_projection_documents_at(query(1), source, generation(1))
                .await
                .unwrap()
                .records()
                .len(),
            3
        );
    }
    store.close().await.unwrap();
    let store = open(&temp).await;
    assert_eq!(
        store
            .query_projection_documents_at(query(1), source, generation(1))
            .await
            .unwrap(),
        memory
            .query_projection_documents_at(query(1), source, generation(1))
            .await
            .unwrap()
    );
    for backend in [&store as &dyn ProjectionStore, &memory] {
        backend
            .promote_projection_batch(
                batch(scope(source, Some(generation(1)), 2), &["y"]),
                checkpoint(2, 2),
            )
            .await
            .unwrap();
        assert_eq!(
            backend
                .query_projection_documents_at(query(1), source, generation(1))
                .await,
            Err(Error::ProjectionRevisionConflict)
        );
        assert_eq!(
            backend
                .query_projection_documents_at(query(2), source, generation(2))
                .await
                .unwrap()
                .records()
                .len(),
            2
        );
    }
    assert_eq!(
        store
            .query_projection_documents_at(query(2), source, generation(2))
            .await
            .unwrap(),
        memory
            .query_projection_documents_at(query(2), source, generation(2))
            .await
            .unwrap()
    );
    store.close().await.unwrap();
    let readonly = SqliteStorage::open(OpenOptions::new(
        Paths::from_directory(temp.path()).unwrap(),
        OpenMode::ReadOnly,
    ))
    .await
    .unwrap();
    assert_eq!(
        readonly
            .query_projection_documents_at(query(2), source, generation(2))
            .await
            .unwrap()
            .records()
            .len(),
        2
    );
    assert_eq!(
        readonly
            .put_projection_batch(batch(scope(source, Some(generation(2)), 3), &["z"]))
            .await,
        Err(Error::BackendUnavailable)
    );
    readonly.close().await.unwrap();
}

#[tokio::test]
async fn projection_final_batch_fault_rolls_back_every_row_fence_and_status() {
    let temp = TempDir::new().unwrap();
    let store = open(&temp).await;
    let source = store.source_token().await.unwrap();
    store
        .promote_projection_batch(batch(scope(source, None, 1), &["old"]), checkpoint(1, 1))
        .await
        .unwrap();
    sqlx::raw_sql("CREATE TRIGGER fixture_projection_abort BEFORE INSERT ON radroots_runtime_projection_documents WHEN NEW.document_key = 'fail' BEGIN SELECT RAISE(ABORT,'fixture'); END;")
        .execute(store.pool()).await.unwrap();
    let next = scope(source, Some(generation(1)), 2);
    assert!(
        store
            .promote_projection_batch(batch(next.clone(), &["new", "fail"]), checkpoint(2, 2))
            .await
            .is_err()
    );
    assert!(
        store
            .query_projection_documents(query(2))
            .await
            .unwrap()
            .records()
            .is_empty()
    );
    assert_eq!(
        ProjectionStore::status(&store, id())
            .await
            .unwrap()
            .unwrap()
            .generation(),
        generation(1)
    );
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM radroots_runtime_projection_generations WHERE phase='staging'",
    )
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(count, 0);
    sqlx::raw_sql("DROP TRIGGER fixture_projection_abort")
        .execute(store.pool())
        .await
        .unwrap();
    store
        .promote_projection_batch(batch(next, &["new", "fail"]), checkpoint(2, 2))
        .await
        .unwrap();
    assert_eq!(
        store
            .query_projection_documents_at(query(2), source, generation(2))
            .await
            .unwrap()
            .records()
            .len(),
        2
    );
    store.close().await.unwrap();
}

#[tokio::test]
async fn projection_stale_source_epoch_and_cancelled_wait_cannot_publish() {
    let temp = TempDir::new().unwrap();
    let store = open(&temp).await;
    let source = store.source_token().await.unwrap();
    let mut all = Vec::new();
    for _ in 0..4 {
        all.push(store.pool().acquire().await.unwrap());
    }
    let next = scope(source, None, 1);
    let mut future = store.promote_projection_batch(batch(next.clone(), &["a"]), checkpoint(1, 1));
    assert!(matches!(
        future
            .as_mut()
            .poll(&mut std::task::Context::from_waker(std::task::Waker::noop())),
        std::task::Poll::Pending
    ));
    drop(future);
    drop(all);
    assert_eq!(ProjectionStore::status(&store, id()).await.unwrap(), None);
    assert!(
        store
            .query_projection_documents(query(1))
            .await
            .unwrap()
            .records()
            .is_empty()
    );
    store
        .put_projection_batch(batch(next.clone(), &["a"]))
        .await
        .unwrap();
    sqlx::query("UPDATE radroots_runtime_source_generations SET mutation_epoch = mutation_epoch + 1 WHERE generation = ?")
        .bind(source.generation().as_bytes().as_slice()).execute(store.pool()).await.unwrap();
    assert_eq!(
        store
            .promote_projection_batch(batch(next, &["b"]), checkpoint(1, 2))
            .await,
        Err(Error::SourceEpochChanged)
    );
    let changed = store.source_token().await.unwrap();
    assert_eq!(
        store
            .put_projection_batch(batch(scope(changed, None, 1), &["b"]))
            .await,
        Err(Error::ProjectionRevisionConflict)
    );
    assert_eq!(
        store
            .query_projection_documents(query(1))
            .await
            .unwrap()
            .records()
            .len(),
        1
    );
    assert_eq!(ProjectionStore::status(&store, id()).await.unwrap(), None);
    store.close().await.unwrap();
}

#[tokio::test]
async fn projection_wal_readers_observe_complete_old_new_or_typed_stale() {
    let temp = TempDir::new().unwrap();
    let store = open(&temp).await;
    let source = store.source_token().await.unwrap();
    store
        .promote_projection_batch(
            batch(scope(source, None, 1), &["old.a", "old.b"]),
            checkpoint(1, 2),
        )
        .await
        .unwrap();
    for next in 2..18u8 {
        let old = next - 1;
        let (read, promoted) = tokio::join!(
            store.query_projection_documents_at(query(old), source, generation(old)),
            store.promote_projection_batch(
                batch(
                    scope(source, Some(generation(old)), next),
                    &["new.a", "new.b"]
                ),
                checkpoint(next, 2)
            )
        );
        promoted.unwrap();
        match read {
            Ok(page) => assert_eq!(page.records().len(), 2),
            Err(Error::ProjectionRevisionConflict) => {}
            other => panic!("unexpected read {other:?}"),
        }
        assert_eq!(
            store
                .query_projection_documents_at(query(next), source, generation(next))
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
async fn projection_real_same_count_admission_change_invalidates_durable_staging() {
    use radroots_event::{SignedEvent, admission::RawEvent, wire::Nip01EventWire};
    use radroots_storage::{Storage, event::EventAdmission};
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
        created_at: 1,
        kind: 0,
        tags: vec![],
        content: r#"{"name":"fixture"}"#.into(),
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
    let verified = RawEvent::new(event.envelope().clone())
        .verify_id()
        .unwrap()
        .verify_signature(&FixtureVerifier)
        .unwrap();
    let temp = TempDir::new().unwrap();
    let store = open(&temp).await;
    let memory = MemoryStorage::new(SourceGeneration::new([9; 32]).unwrap());
    for backend in [&store as &dyn Storage, &memory] {
        let empty = backend.source_token().await.unwrap();
        let receipt = backend
            .admit(EventAdmission::raw(observed.clone()))
            .await
            .unwrap();
        assert_eq!(
            backend
                .put_projection_batch(batch(scope(empty, None, 1), &["a"]))
                .await,
            Err(Error::SourceEpochChanged)
        );
        let source = backend.source_token().await.unwrap();
        let next = scope(source, None, 1);
        backend
            .put_projection_batch(batch(next.clone(), &["a"]))
            .await
            .unwrap();
        backend
            .admit(EventAdmission::raw(observed.clone()))
            .await
            .unwrap();
        assert_eq!(backend.source_token().await.unwrap(), source);
        backend
            .admit(EventAdmission::verified(observed.clone(), verified.clone()).unwrap())
            .await
            .unwrap();
        let changed = backend.source_token().await.unwrap();
        assert_eq!(changed.epoch(), source.epoch() + 1);
        let checkpoint =
            ProjectionCheckpoint::new(id(), generation(1), Some(receipt.position()), 2, 1).unwrap();
        assert_eq!(
            backend
                .promote_projection_batch(batch(next, &["b"]), checkpoint)
                .await,
            Err(Error::SourceEpochChanged)
        );
        assert_eq!(ProjectionStore::status(backend, id()).await.unwrap(), None);
        assert_eq!(
            backend
                .query_projection_documents(query(1))
                .await
                .unwrap()
                .records()
                .len(),
            1
        );
    }
    store.close().await.unwrap();
}

#[tokio::test]
async fn projection_max_batch_updates_and_exact_lookup_validate_before_hydration() {
    let temp = TempDir::new().unwrap();
    let store = open(&temp).await;
    let source = store.source_token().await.unwrap();
    let next = scope(source, None, 1);
    let documents: Vec<_> = (0..64).map(|i| document(&format!("k{i:02}"))).collect();
    store
        .put_projection_batch(ProjectionDocumentBatch::new(next.clone(), documents).unwrap())
        .await
        .unwrap();
    store
        .promote_projection_batch(batch(next, &["k00"]), checkpoint(1, 64))
        .await
        .unwrap();
    assert_eq!(
        store
            .query_projection_documents_at(query(1), source, generation(1))
            .await
            .unwrap()
            .records()
            .len(),
        64
    );
    assert_eq!(
        store
            .projection_document(id(), generation(1), "k00".into())
            .await
            .unwrap()
            .unwrap()
            .value(),
        b"k00"
    );
    assert_eq!(
        store
            .projection_document(id(), generation(1), "missing".into())
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        store
            .projection_document(id(), generation(1), "bad\nkey".into())
            .await,
        Err(Error::InvalidProjectionDocument)
    );
    sqlx::query("UPDATE radroots_runtime_projection_documents SET value_sha256 = zeroblob(32) WHERE document_key='k00'").execute(store.pool()).await.unwrap();
    assert_eq!(
        store
            .projection_document(id(), generation(1), "k00".into())
            .await,
        Err(Error::CorruptProjectionDocument)
    );
    let plan=sqlx::query("EXPLAIN QUERY PLAN SELECT document_key FROM radroots_runtime_projection_documents WHERE projection_id=? AND generation=? AND document_key>? ORDER BY document_key LIMIT 65")
        .bind(id().as_str()).bind(generation(1).as_bytes().as_slice()).bind("k00").fetch_all(store.pool()).await.unwrap();
    let details: Vec<String> = plan.iter().map(|row| row.get("detail")).collect();
    assert!(
        details
            .iter()
            .any(|line| line.contains("PRIMARY KEY") && line.contains("document_key>"))
    );
    assert!(!details.iter().any(|line| line.contains("TEMP B-TREE")));
    store.close().await.unwrap();
}

#[tokio::test]
async fn projection_foreign_sources_legacy_rows_and_invalidated_headers_refuse_without_writes() {
    let temp = TempDir::new().unwrap();
    let store = open(&temp).await;
    let source = store.source_token().await.unwrap();
    let foreign = EventSourceToken::new(SourceGeneration::new([8; 32]).unwrap(), 0);
    assert_eq!(
        store
            .put_projection_batch(batch(scope(foreign, None, 1), &["foreign"]))
            .await,
        Err(Error::SourceEpochChanged)
    );
    store
        .put_projection_document(id(), generation(1), document("legacy"))
        .await
        .unwrap();
    assert_eq!(
        store
            .put_projection_batch(batch(scope(source, None, 1), &[]))
            .await,
        Err(Error::ProjectionRevisionConflict)
    );
    store
        .promote_projection_batch(batch(scope(source, None, 2), &["active"]), checkpoint(2, 1))
        .await
        .unwrap();
    assert_eq!(
        store
            .query_projection_documents_at(query(1), source, generation(2))
            .await,
        Err(Error::InvalidProjectionDocument)
    );
    store
        .invalidate(
            radroots_storage::projection::ProjectionInvalidation::new(
                id(),
                generation(2),
                generation(3),
                radroots_storage::projection::InvalidationReason::OperatorRequested,
                3,
            )
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        store
            .put_projection_batch(batch(scope(source, Some(generation(2)), 3), &["refused"]))
            .await,
        Err(Error::ProjectionRevisionConflict)
    );
    assert_eq!(
        store
            .query_projection_documents_at(query(2), source, generation(2))
            .await,
        Err(Error::ProjectionRevisionConflict)
    );
    assert!(
        store
            .query_projection_documents(query(3))
            .await
            .unwrap()
            .records()
            .is_empty()
    );
    store.close().await.unwrap();
}

#[tokio::test]
async fn projection_retired_or_corrupt_staging_fences_cannot_be_rebound() {
    let temp = TempDir::new().unwrap();
    let store = open(&temp).await;
    let source = store.source_token().await.unwrap();
    let next = scope(source, None, 1);
    store
        .put_projection_batch(batch(next.clone(), &["retained"]))
        .await
        .unwrap();
    for statement in [
        "UPDATE radroots_runtime_projection_generations SET source_generation=zeroblob(32)",
        "UPDATE radroots_runtime_projection_generations SET source_epoch=1",
        "UPDATE radroots_runtime_projection_generations SET expected_active=zeroblob(32)",
        "UPDATE radroots_runtime_projection_generations SET phase='retired'",
    ] {
        sqlx::query(statement).execute(store.pool()).await.unwrap();
        assert_eq!(
            store
                .put_projection_batch(batch(next.clone(), &["refused"]))
                .await,
            Err(Error::ProjectionRevisionConflict)
        );
        assert_eq!(
            store
                .query_projection_documents(query(1))
                .await
                .unwrap()
                .records()
                .len(),
            1
        );
        sqlx::query("UPDATE radroots_runtime_projection_generations SET source_generation=?,source_epoch=0,expected_active=NULL,phase='staging'")
            .bind(source.generation().as_bytes().as_slice()).execute(store.pool()).await.unwrap();
    }
    store
        .promote_projection_batch(batch(next, &[]), checkpoint(1, 1))
        .await
        .unwrap();
    store.close().await.unwrap();
}

#[tokio::test]
async fn projection_nonempty_checkpoints_bind_exact_identity_generation_and_source_position() {
    use radroots_event::{SignedEvent, wire::Nip01EventWire};
    use radroots_storage::event::{EventAdmission, EventPosition, EventSequence};
    use radroots_transport::{
        Target, TransportId,
        source::{EventProvenance, ObservedEvent},
    };
    let temp = TempDir::new().unwrap();
    let store = open(&temp).await;
    let mut wire = Nip01EventWire {
        id: "0".repeat(64),
        pubkey: "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df".into(),
        created_at: 1,
        kind: 0,
        tags: vec![],
        content: r#"{"name":"fixture"}"#.into(),
        sig: "42".repeat(64),
        extra: Default::default(),
    };
    wire.id = wire.computed_event_id().unwrap().to_hex();
    let raw = serde_json::to_string(&wire).unwrap();
    let event = SignedEvent::from_wire_verified_id(wire, raw).unwrap();
    let target = Target::new(TransportId::NOSTR, "wss://fixture.example").unwrap();
    let observed = ObservedEvent::new(
        event,
        EventProvenance::new(TransportId::NOSTR, target.fingerprint().clone(), 1).unwrap(),
    );
    let receipt = store.admit(EventAdmission::raw(observed)).await.unwrap();
    let source = store.source_token().await.unwrap();
    let next = scope(source, None, 1);
    store
        .put_projection_batch(batch(next.clone(), &["a"]))
        .await
        .unwrap();
    for checkpoint in [
        ProjectionCheckpoint::new(
            ProjectionId::parse("foreign").unwrap(),
            generation(1),
            Some(receipt.position()),
            2,
            1,
        )
        .unwrap(),
        ProjectionCheckpoint::new(id(), generation(2), Some(receipt.position()), 2, 1).unwrap(),
        ProjectionCheckpoint::new(id(), generation(1), None, 2, 1).unwrap(),
        ProjectionCheckpoint::new(
            id(),
            generation(1),
            Some(EventPosition::new(
                SourceGeneration::new([8; 32]).unwrap(),
                EventSequence::new(1).unwrap(),
            )),
            2,
            1,
        )
        .unwrap(),
        ProjectionCheckpoint::new(
            id(),
            generation(1),
            Some(EventPosition::new(
                source.generation(),
                EventSequence::new(2).unwrap(),
            )),
            2,
            1,
        )
        .unwrap(),
    ] {
        assert_eq!(
            store
                .promote_projection_batch(batch(next.clone(), &["b"]), checkpoint)
                .await,
            Err(Error::ProjectionCheckpointMismatch)
        );
        assert_eq!(
            store
                .query_projection_documents(query(1))
                .await
                .unwrap()
                .records()
                .len(),
            1
        );
        assert_eq!(ProjectionStore::status(&store, id()).await.unwrap(), None);
    }
    store
        .promote_projection_batch(
            batch(next, &["b"]),
            ProjectionCheckpoint::new(id(), generation(1), Some(receipt.position()), 2, 1).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        store
            .query_projection_documents_at(query(1), source, generation(1))
            .await
            .unwrap()
            .records()
            .len(),
        2
    );
    store.close().await.unwrap();
}

#[tokio::test]
async fn projection_exact_lookup_refuses_zero_and_oversized_stored_values_before_hydration() {
    let temp = TempDir::new().unwrap();
    let store = open(&temp).await;
    let source = store.source_token().await.unwrap();
    store
        .put_projection_document(id(), generation(1), document("retained"))
        .await
        .unwrap();
    for size in [0, 16_777_217] {
        let mut connection = store.pool().acquire().await.unwrap();
        sqlx::raw_sql("PRAGMA ignore_check_constraints=ON")
            .execute(&mut *connection)
            .await
            .unwrap();
        sqlx::query("UPDATE radroots_runtime_projection_documents SET value=zeroblob(?) WHERE document_key='retained'")
            .bind(size).execute(&mut *connection).await.unwrap();
        sqlx::raw_sql("PRAGMA ignore_check_constraints=OFF")
            .execute(&mut *connection)
            .await
            .unwrap();
        drop(connection);
        assert_eq!(
            store
                .projection_document(id(), generation(1), "retained".into())
                .await,
            Err(Error::CorruptProjectionDocument)
        );
        assert_eq!(store.source_token().await.unwrap(), source);
    }
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
async fn projection_promoted_source_binding_survives_real_insert_and_same_count_change() {
    use radroots_storage::Storage;
    let temp = TempDir::new().unwrap();
    let store = open(&temp).await;
    let memory = MemoryStorage::new(SourceGeneration::new([9; 32]).unwrap());
    for backend in [&memory as &dyn Storage, &store] {
        let empty = backend.source_token().await.unwrap();
        backend
            .promote_projection_batch(batch(scope(empty, None, 1), &["empty"]), checkpoint(1, 1))
            .await
            .unwrap();
        let receipt = backend.admit(review_admission(false)).await.unwrap();
        let raw = backend.source_token().await.unwrap();
        assert_eq!(
            backend
                .query_projection_documents_at(query(1), empty, generation(1))
                .await,
            Err(Error::SourceEpochChanged)
        );
        assert_eq!(
            backend
                .query_projection_documents_at(query(1), raw, generation(1))
                .await,
            Err(Error::SourceEpochChanged)
        );
        backend
            .promote_projection_batch(
                batch(scope(raw, Some(generation(1)), 2), &["raw"]),
                ProjectionCheckpoint::new(id(), generation(2), Some(receipt.position()), 1, 2)
                    .unwrap(),
            )
            .await
            .unwrap();
        backend.admit(review_admission(true)).await.unwrap();
        let verified = backend.source_token().await.unwrap();
        assert_eq!(verified.epoch(), raw.epoch() + 1);
        assert_eq!(
            backend
                .query_projection_documents_at(query(2), raw, generation(2))
                .await,
            Err(Error::SourceEpochChanged)
        );
        assert_eq!(
            backend
                .query_projection_documents_at(query(2), verified, generation(2))
                .await,
            Err(Error::SourceEpochChanged)
        );
        assert_eq!(
            backend
                .query_projection_documents(query(2))
                .await
                .unwrap()
                .records()
                .len(),
            1
        );
        backend
            .promote_projection_batch(
                batch(scope(verified, Some(generation(2)), 3), &["verified"]),
                ProjectionCheckpoint::new(id(), generation(3), Some(receipt.position()), 1, 3)
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            backend
                .query_projection_documents_at(query(3), verified, generation(3))
                .await
                .unwrap()
                .records()
                .len(),
            1
        );
    }
    store.close().await.unwrap();
}

#[tokio::test]
async fn projection_registered_checkpoint_and_atomic_admission_cannot_bypass_promotion() {
    use radroots_storage::{
        Storage,
        atomic::{
            AtomicCommit, AtomicCommitDigest, AtomicCommitId, AtomicWorkflow, CommitIngested,
        },
    };
    let temp = TempDir::new().unwrap();
    let store = open(&temp).await;
    let memory = MemoryStorage::new(SourceGeneration::new([9; 32]).unwrap());
    for backend in [&memory as &dyn Storage, &store] {
        let source = backend.source_token().await.unwrap();
        let staged = scope(source, None, 1);
        backend
            .put_projection_batch(batch(staged.clone(), &["partial"]))
            .await
            .unwrap();
        assert_eq!(
            backend.checkpoint(checkpoint(1, 999)).await,
            Err(Error::ProjectionRevisionConflict)
        );
        assert_eq!(ProjectionStore::status(backend, id()).await.unwrap(), None);
        let request = AtomicCommit::new(
            AtomicCommitId::new([1; 16]).unwrap(),
            AtomicCommitDigest::new([1; 32]),
            1,
            AtomicWorkflow::Ingested(Box::new(CommitIngested::new(
                review_admission(false),
                Some(checkpoint(1, 999)),
            ))),
        )
        .unwrap();
        assert_eq!(
            backend.commit(request.clone()).await,
            Err(Error::ProjectionRevisionConflict)
        );
        assert_eq!(backend.receipt(request.commit_id()).await.unwrap(), None);
        assert_eq!(backend.source_token().await.unwrap(), source);
        assert_eq!(ProjectionStore::status(backend, id()).await.unwrap(), None);
        backend
            .promote_projection_batch(batch(staged, &[]), checkpoint(1, 1))
            .await
            .unwrap();
        let prior = ProjectionStore::status(backend, id()).await.unwrap();
        assert_eq!(
            backend.checkpoint(checkpoint(1, 999)).await,
            Err(Error::ProjectionRevisionConflict)
        );
        assert_eq!(ProjectionStore::status(backend, id()).await.unwrap(), prior);
        assert_eq!(
            backend
                .query_projection_documents_at(query(1), source, generation(1))
                .await
                .unwrap()
                .records()
                .len(),
            1
        );
        backend
            .promote_projection_batch(
                batch(scope(source, Some(generation(1)), 2), &["new"]),
                checkpoint(2, 1),
            )
            .await
            .unwrap();
        assert_eq!(
            backend.checkpoint(checkpoint(1, 999)).await,
            Err(Error::ProjectionRevisionConflict)
        );
        let legacy = ProjectionId::parse("legacy").unwrap();
        backend
            .put_projection_document(legacy.clone(), generation(9), document("legacy"))
            .await
            .unwrap();
        backend
            .checkpoint(
                ProjectionCheckpoint::new(legacy.clone(), generation(9), None, 7, 9).unwrap(),
            )
            .await
            .unwrap();
        let legacy_query = ProjectionDocumentQuery::new(
            legacy,
            ProjectionDocumentGenerations::Exact(generation(9)),
            1,
        )
        .unwrap();
        assert_eq!(
            backend
                .query_projection_documents_at(legacy_query.clone(), source, generation(9))
                .await,
            Err(Error::ProjectionRevisionConflict)
        );
        assert_eq!(
            backend
                .query_projection_documents(legacy_query)
                .await
                .unwrap()
                .records()
                .len(),
            1
        );
    }
    store.close().await.unwrap();
}

#[tokio::test]
async fn projection_legacy_rebuild_cannot_publish_registered_staging_or_retired_generation() {
    use radroots_storage::projection::{
        InvalidationReason, ProjectionInvalidation, RawSourceDigest, RebuildTicket,
        RebuildTicketId, RebuildTransition,
    };
    for retired in [false, true] {
        let temp = TempDir::new().unwrap();
        let store = open(&temp).await;
        let memory = MemoryStorage::new(SourceGeneration::new([9; 32]).unwrap());
        for backend in [&memory as &dyn ProjectionStore, &store] {
            let source = store.source_token().await.unwrap();
            backend
                .promote_projection_batch(
                    batch(scope(source, None, 1), &["first"]),
                    checkpoint(1, 1),
                )
                .await
                .unwrap();
            let second = batch(scope(source, Some(generation(1)), 2), &["second"]);
            let (active, target) = if retired {
                backend
                    .promote_projection_batch(second, checkpoint(2, 1))
                    .await
                    .unwrap();
                (2, 1)
            } else {
                backend.put_projection_batch(second).await.unwrap();
                (1, 2)
            };
            let invalidation = ProjectionInvalidation::new(
                id(),
                generation(active),
                generation(target),
                InvalidationReason::OperatorRequested,
                10,
            )
            .unwrap();
            backend.invalidate(invalidation.clone()).await.unwrap();
            let ticket = RebuildTicket::requested(
                RebuildTicketId::new([target; 16]).unwrap(),
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
            let status = backend.status(id()).await.unwrap();
            assert_eq!(
                backend
                    .transition_rebuild(RebuildTransition::complete(
                        running.ticket_id(),
                        running.revision(),
                        12,
                        ProjectionCheckpoint::new(id(), generation(target), None, 999, 12).unwrap()
                    ))
                    .await,
                Err(Error::ProjectionRevisionConflict)
            );
            assert_eq!(backend.status(id()).await.unwrap(), status);
            assert_eq!(
                backend.rebuild(ticket.ticket_id()).await.unwrap(),
                Some(running)
            );
            assert_eq!(
                backend
                    .query_projection_documents(query(target))
                    .await
                    .unwrap()
                    .records()
                    .len(),
                1
            );
            assert_eq!(
                backend
                    .query_projection_documents_at(query(target), source, generation(target))
                    .await,
                Err(Error::ProjectionRevisionConflict)
            );
        }
        store.close().await.unwrap();
    }
}

#[tokio::test]
async fn projection_source_mutation_wal_race_reopen_and_read_only_never_relabel_old_rows() {
    for _ in 0..8 {
        let temp = TempDir::new().unwrap();
        let store = open(&temp).await;
        let old = store.source_token().await.unwrap();
        store
            .promote_projection_batch(batch(scope(old, None, 1), &["old"]), checkpoint(1, 1))
            .await
            .unwrap();
        let (mutation, read) = tokio::join!(
            store.admit(review_admission(false)),
            store.query_projection_documents_at(query(1), old, generation(1))
        );
        let receipt = mutation.unwrap();
        match read {
            Ok(page) => assert_eq!(page.records().len(), 1),
            Err(error) => assert_eq!(error, Error::SourceEpochChanged),
        }
        let current = store.source_token().await.unwrap();
        assert_ne!(current, old);
        for token in [old, current] {
            assert_eq!(
                store
                    .query_projection_documents_at(query(1), token, generation(1))
                    .await,
                Err(Error::SourceEpochChanged)
            );
        }
        store.close().await.unwrap();
        let readonly = SqliteStorage::open(OpenOptions::new(
            Paths::from_directory(temp.path()).unwrap(),
            OpenMode::ReadOnly,
        ))
        .await
        .unwrap();
        for token in [old, current] {
            assert_eq!(
                readonly
                    .query_projection_documents_at(query(1), token, generation(1))
                    .await,
                Err(Error::SourceEpochChanged)
            );
        }
        assert_eq!(
            readonly
                .query_projection_documents(query(1))
                .await
                .unwrap()
                .records()
                .len(),
            1
        );
        readonly.close().await.unwrap();
        let reopened = open(&temp).await;
        assert_eq!(
            reopened
                .query_projection_documents_at(query(1), current, generation(1))
                .await,
            Err(Error::SourceEpochChanged)
        );
        reopened
            .promote_projection_batch(
                batch(scope(current, Some(generation(1)), 2), &["new"]),
                ProjectionCheckpoint::new(id(), generation(2), Some(receipt.position()), 1, 2)
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            reopened
                .query_projection_documents_at(query(2), current, generation(2))
                .await
                .unwrap()
                .records()
                .len(),
            1
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
                .query_projection_documents_at(query(2), current, generation(2))
                .await
                .unwrap()
                .records()
                .len(),
            1
        );
        readonly.close().await.unwrap();
    }
}
