use futures_executor::block_on;
use radroots_storage::{
    Error, EventStore, ProjectionStore,
    event::{EventSourceToken, SourceGeneration},
    memory::MemoryStorage,
    projection::{
        ProjectionCheckpoint, ProjectionDocument, ProjectionGeneration, ProjectionId,
        batch::{PROJECTION_BATCH_BYTES_MAX, ProjectionDocumentBatch, ProjectionWriteScope},
        document_query::{ProjectionDocumentGenerations, ProjectionDocumentQuery},
    },
};
fn generation(byte: u8) -> ProjectionGeneration {
    ProjectionGeneration::new([byte; 32]).unwrap()
}
fn id() -> ProjectionId {
    ProjectionId::parse("fixture").unwrap()
}
fn document(key: &str, size: usize) -> ProjectionDocument {
    ProjectionDocument::new(key.into(), vec![7; size]).unwrap()
}
fn scope(
    source: EventSourceToken,
    active: Option<ProjectionGeneration>,
    target: u8,
) -> ProjectionWriteScope {
    ProjectionWriteScope::new(id(), generation(target), source, active).unwrap()
}
fn query(target: u8) -> ProjectionDocumentQuery {
    ProjectionDocumentQuery::new(
        id(),
        ProjectionDocumentGenerations::Exact(generation(target)),
        64,
    )
    .unwrap()
}

#[test]
fn projection_batch_bounds_duplicates_redaction_and_scope_fail_closed() {
    let source = EventSourceToken::new(SourceGeneration::new([9; 32]).unwrap(), 0);
    let scope = scope(source, None, 1);
    let docs: Vec<_> = (0..64)
        .map(|i| document(&format!("key.{i:02}"), 1))
        .collect();
    let batch = ProjectionDocumentBatch::new(scope.clone(), docs).unwrap();
    assert_eq!(batch.documents().len(), 64);
    assert!(!format!("{batch:?}").contains("key.00"));
    assert_eq!(
        ProjectionDocumentBatch::new(
            scope.clone(),
            (0..65).map(|i| document(&format!("k{i}"), 1)).collect()
        ),
        Err(Error::InvalidProjectionDocument)
    );
    assert_eq!(
        ProjectionDocumentBatch::new(
            scope.clone(),
            vec![document("same", 1), document("same", 2)]
        ),
        Err(Error::InvalidProjectionDocument)
    );
    let value_max = (PROJECTION_BATCH_BYTES_MAX - 4096 - 256 - 2) / 2;
    let exact =
        ProjectionDocumentBatch::new(scope.clone(), vec![document("k", value_max)]).unwrap();
    assert_eq!(exact.logical_bytes(), PROJECTION_BATCH_BYTES_MAX);
    assert_eq!(
        ProjectionDocumentBatch::new(scope, vec![document("k", value_max + 1)]),
        Err(Error::SpaceInsufficient)
    );
    assert_eq!(
        ProjectionWriteScope::new(id(), generation(1), source, Some(generation(1))),
        Err(Error::InvalidProjectionGeneration)
    );
    assert!(ProjectionDocument::new("\n".into(), vec![1]).is_err());
    assert!(ProjectionId::parse("wrong scope").is_err());
}

#[test]
fn projection_memory_atomic_generation_counts_fences_and_sealing_are_coherent() {
    let store = MemoryStorage::new(SourceGeneration::new([9; 32]).unwrap());
    let source = block_on(store.source_token()).unwrap();
    let first = scope(source, None, 1);
    block_on(
        store.put_projection_batch(
            ProjectionDocumentBatch::new(first.clone(), vec![document("a", 1), document("b", 2)])
                .unwrap(),
        ),
    )
    .unwrap();
    assert_eq!(
        block_on(ProjectionStore::status(&store, id())).unwrap(),
        None
    );
    let bad = ProjectionCheckpoint::new(id(), generation(1), None, 1, 1).unwrap();
    assert_eq!(
        block_on(store.promote_projection_batch(
            ProjectionDocumentBatch::new(first.clone(), vec![document("c", 3)]).unwrap(),
            bad
        )),
        Err(Error::ProjectionCheckpointMismatch)
    );
    assert_eq!(
        block_on(store.query_projection_documents(query(1)))
            .unwrap()
            .records()
            .len(),
        2
    );
    let good = ProjectionCheckpoint::new(id(), generation(1), None, 3, 1).unwrap();
    block_on(store.promote_projection_batch(
        ProjectionDocumentBatch::new(first, vec![document("c", 3)]).unwrap(),
        good,
    ))
    .unwrap();
    assert_eq!(
        block_on(store.query_projection_documents_at(query(1), source, generation(1)))
            .unwrap()
            .records()
            .len(),
        3
    );
    assert_eq!(
        block_on(store.put_projection_document(id(), generation(1), document("a", 9))),
        Err(Error::ProjectionRevisionConflict)
    );
    let second = scope(source, Some(generation(1)), 2);
    block_on(store.put_projection_batch(
        ProjectionDocumentBatch::new(second.clone(), vec![document("x", 1)]).unwrap(),
    ))
    .unwrap();
    assert_eq!(
        block_on(store.put_projection_batch(
            ProjectionDocumentBatch::new(scope(source, Some(generation(1)), 3), vec![]).unwrap()
        )),
        Err(Error::ProjectionRevisionConflict)
    );
    let stale = EventSourceToken::new(source.generation(), source.epoch() + 1);
    assert_eq!(
        block_on(store.query_projection_documents_at(query(1), stale, generation(1))),
        Err(Error::SourceEpochChanged)
    );
    assert_eq!(
        block_on(store.query_projection_documents_at(query(2), source, generation(1))),
        Err(Error::InvalidProjectionDocument)
    );
    block_on(store.promote_projection_batch(
        ProjectionDocumentBatch::new(second, vec![document("y", 2)]).unwrap(),
        ProjectionCheckpoint::new(id(), generation(2), None, 2, 2).unwrap(),
    ))
    .unwrap();
    assert_eq!(
        block_on(store.query_projection_documents_at(query(1), source, generation(1))),
        Err(Error::ProjectionRevisionConflict)
    );
    assert_eq!(
        block_on(store.query_projection_documents_at(query(2), source, generation(2)))
            .unwrap()
            .records()
            .len(),
        2
    );
    assert_eq!(
        block_on(store.put_projection_batch(
            ProjectionDocumentBatch::new(scope(source, None, 4), vec![]).unwrap()
        )),
        Err(Error::ProjectionRevisionConflict)
    );
}

#[test]
fn projection_memory_updates_empty_generations_and_checkpoint_scopes_are_atomic() {
    let store = MemoryStorage::new(SourceGeneration::new([9; 32]).unwrap());
    let source = block_on(store.source_token()).unwrap();
    block_on(store.put_projection_document(id(), generation(1), document("legacy", 1))).unwrap();
    assert_eq!(
        block_on(store.put_projection_batch(
            ProjectionDocumentBatch::new(scope(source, None, 1), vec![]).unwrap()
        )),
        Err(Error::ProjectionRevisionConflict)
    );
    assert_eq!(
        block_on(store.projection_document(id(), generation(1), "bad\nkey".into())),
        Err(Error::InvalidProjectionDocument)
    );
    let next = scope(source, None, 2);
    let rows: Vec<_> = (0..64).map(|i| document(&format!("k{i:02}"), 1)).collect();
    block_on(store.put_projection_batch(ProjectionDocumentBatch::new(next.clone(), rows).unwrap()))
        .unwrap();
    block_on(store.put_projection_batch(
        ProjectionDocumentBatch::new(next.clone(), vec![document("k00", 3)]).unwrap(),
    ))
    .unwrap();
    assert_eq!(
        block_on(store.projection_document(id(), generation(2), "k00".into()))
            .unwrap()
            .unwrap()
            .value()
            .len(),
        3
    );
    for checkpoint in [
        ProjectionCheckpoint::new(
            ProjectionId::parse("foreign").unwrap(),
            generation(2),
            None,
            64,
            1,
        )
        .unwrap(),
        ProjectionCheckpoint::new(id(), generation(3), None, 64, 1).unwrap(),
        ProjectionCheckpoint::new(
            id(),
            generation(2),
            Some(radroots_storage::event::EventPosition::new(
                source.generation(),
                radroots_storage::event::EventSequence::new(1).unwrap(),
            )),
            64,
            1,
        )
        .unwrap(),
    ] {
        assert_eq!(
            block_on(store.promote_projection_batch(
                ProjectionDocumentBatch::new(next.clone(), vec![document("k00", 4)]).unwrap(),
                checkpoint
            )),
            Err(Error::ProjectionCheckpointMismatch)
        );
        assert_eq!(
            block_on(store.projection_document(id(), generation(2), "k00".into()))
                .unwrap()
                .unwrap()
                .value()
                .len(),
            3
        );
    }
    block_on(store.promote_projection_batch(
        ProjectionDocumentBatch::new(next, vec![]).unwrap(),
        ProjectionCheckpoint::new(id(), generation(2), None, 64, 1).unwrap(),
    ))
    .unwrap();
    let empty = scope(source, Some(generation(2)), 3);
    block_on(store.promote_projection_batch(
        ProjectionDocumentBatch::new(empty, vec![]).unwrap(),
        ProjectionCheckpoint::new(id(), generation(3), None, 0, 2).unwrap(),
    ))
    .unwrap();
    assert!(
        block_on(store.query_projection_documents_at(query(3), source, generation(3)))
            .unwrap()
            .records()
            .is_empty()
    );
    let foreign = EventSourceToken::new(SourceGeneration::new([8; 32]).unwrap(), 0);
    assert_eq!(
        block_on(store.put_projection_batch(
            ProjectionDocumentBatch::new(scope(foreign, Some(generation(3)), 4), vec![]).unwrap()
        )),
        Err(Error::SourceEpochChanged)
    );
}
