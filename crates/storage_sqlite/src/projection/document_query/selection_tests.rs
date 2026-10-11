use super::*;
use crate::{OpenMode, OpenOptions, Paths};
use radroots_storage::{
    ProjectionStore,
    event::{EventSourceToken, SourceGeneration},
    memory::MemoryStorage,
    projection::{
        InvalidationReason, ProjectionCheckpoint, ProjectionId, ProjectionInvalidation,
        batch::{ProjectionDocumentBatch, ProjectionWriteScope},
        document_query::ProjectionDocumentKeyRange,
    },
};
use sqlx::Execute;
use tempfile::TempDir;

fn id() -> ProjectionId {
    ProjectionId::parse("fixture").unwrap()
}
fn generation(byte: u8) -> ProjectionGeneration {
    ProjectionGeneration::new([byte; 32]).unwrap()
}
fn range(first: &str, last: &str) -> ProjectionDocumentKeyRange {
    ProjectionDocumentKeyRange::new(first.into(), last.into()).unwrap()
}
fn query(limit: u16) -> ProjectionDocumentQuery {
    ProjectionDocumentQuery::new(
        id(),
        ProjectionDocumentGenerations::Exact(generation(1)),
        limit,
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
async fn promote(store: &dyn radroots_storage::Storage) -> EventSourceToken {
    let source = store.source_token().await.unwrap();
    let scope = ProjectionWriteScope::new(id(), generation(1), source, None).unwrap();
    let documents: Vec<_> = ["a", "m", "z"]
        .into_iter()
        .flat_map(|prefix| {
            (0..100).map(move |number| {
                ProjectionDocument::new(format!("{prefix}.{number:04}"), vec![1; 4]).unwrap()
            })
        })
        .collect();
    for chunk in documents.chunks(64) {
        store
            .put_projection_batch(
                ProjectionDocumentBatch::new(scope.clone(), chunk.to_vec()).unwrap(),
            )
            .await
            .unwrap();
    }
    store
        .promote_projection_batch(
            ProjectionDocumentBatch::new(
                scope,
                vec![ProjectionDocument::new("zz.end".into(), vec![1]).unwrap()],
            )
            .unwrap(),
            ProjectionCheckpoint::new(id(), generation(1), None, 301, 9).unwrap(),
        )
        .await
        .unwrap();
    source
}

#[tokio::test]
async fn selected_primary_key_pages_match_memory_and_portable_seek_after_reopen() {
    let temp = TempDir::new().unwrap();
    let store = open(&temp).await;
    let memory = MemoryStorage::new(SourceGeneration::new([9; 32]).unwrap());
    let source = promote(&store).await;
    assert_eq!(promote(&memory).await, source);
    let q = query(17)
        .with_key_range(range("m.0003", "m.0098"))
        .unwrap()
        .with_payload_budget(68)
        .unwrap();
    let mut selected = q.clone();
    let mut found = Vec::new();
    loop {
        let page = store
            .query_projection_documents_at(selected.clone(), source, generation(1))
            .await
            .unwrap();
        assert_eq!(
            page,
            memory
                .query_projection_documents_at(selected.clone(), source, generation(1))
                .await
                .unwrap()
        );
        assert!(page.records().len() <= 17);
        assert!(
            page.records()
                .iter()
                .map(|row| row.document().unwrap().value().len())
                .sum::<usize>()
                <= 68
        );
        found.extend(page.records().iter().map(|row| row.key().to_owned()));
        let Some(cursor) = page.next_cursor() else {
            break;
        };
        selected = q.clone().with_cursor(cursor).unwrap();
        assert_eq!(
            selected,
            q.clone()
                .with_after_key(found.last().unwrap().clone())
                .unwrap()
        );
    }
    assert_eq!(
        found,
        (3..=98)
            .map(|number| format!("m.{number:04}"))
            .collect::<Vec<_>>()
    );
    let resume = q.clone().with_after_key("m.0050".into()).unwrap();
    let expected = memory
        .query_projection_documents_at(resume.clone(), source, generation(1))
        .await
        .unwrap();
    store.close().await.unwrap();
    let store = SqliteStorage::open(OpenOptions::new(
        Paths::from_directory(temp.path()).unwrap(),
        OpenMode::ReadOnly,
    ))
    .await
    .unwrap();
    assert_eq!(
        store
            .query_projection_documents_at(resume, source, generation(1))
            .await
            .unwrap(),
        expected
    );
    assert!(
        store
            .query_projection_documents_at(
                q.with_after_key("m.0098".into()).unwrap(),
                source,
                generation(1)
            )
            .await
            .unwrap()
            .records()
            .is_empty()
    );
    let mut builder = selection(&selected);
    let mut actual = builder.build();
    let arguments = actual.take_arguments().unwrap().unwrap();
    let explanation = format!("EXPLAIN QUERY PLAN {}", actual.sql().as_ref());
    // Only a constant prefix decorates the actual bound-value selection SQL.
    let mut explanation =
        sqlx::QueryBuilder::<sqlx::Sqlite>::with_arguments(explanation, arguments);
    let rows = explanation.build().fetch_all(store.pool()).await.unwrap();
    let details: Vec<String> = rows.iter().map(|row| row.get("detail")).collect();
    assert!(
        details.iter().any(|value| value.contains("PRIMARY KEY")
            && value.contains("document_key>")
            && value.contains("document_key<")),
        "{details:?}"
    );
    assert!(
        !details.iter().any(|value| value.starts_with("SCAN ")),
        "{details:?}"
    );
    store.close().await.unwrap();
}

#[tokio::test]
async fn lower_payload_budget_preflights_valid_values_and_preserves_corruption() {
    let temp = TempDir::new().unwrap();
    let store = open(&temp).await;
    let memory = MemoryStorage::new(SourceGeneration::new([9; 32]).unwrap());
    for backend in [&store as &dyn ProjectionStore, &memory] {
        for (key, bytes) in [("a", 4), ("b", 6)] {
            backend
                .put_projection_document(
                    id(),
                    generation(1),
                    ProjectionDocument::new(key.into(), vec![7; bytes]).unwrap(),
                )
                .await
                .unwrap();
        }
        let q = query(2).with_payload_budget(4).unwrap();
        let first = backend.query_projection_documents(q.clone()).await.unwrap();
        assert_eq!(first.records().len(), 1);
        assert_eq!(first.records()[0].key(), "a");
        assert_eq!(
            backend
                .query_projection_documents(q.with_cursor(first.next_cursor().unwrap()).unwrap())
                .await,
            Err(Error::SpaceInsufficient)
        );
        assert_eq!(
            backend
                .query_projection_documents(
                    query(1)
                        .with_key_range(range("a", "a"))
                        .unwrap()
                        .with_payload_budget(3)
                        .unwrap()
                )
                .await,
            Err(Error::SpaceInsufficient)
        );
        let exact = backend
            .query_projection_documents(
                query(1)
                    .with_key_range(range("b", "b"))
                    .unwrap()
                    .with_payload_budget(6)
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(exact.records()[0].document().unwrap().value(), &[7; 6]);
        assert!(exact.next_cursor().is_none());
        assert_eq!(
            backend
                .projection_document(id(), generation(1), "a".into())
                .await
                .unwrap()
                .unwrap()
                .value(),
            &[7; 4]
        );
    }
    sqlx::query("UPDATE radroots_runtime_projection_documents SET value_sha256=zeroblob(32) WHERE document_key='b'").execute(store.pool()).await.unwrap();
    let b = query(1).with_key_range(range("b", "b")).unwrap();
    assert_eq!(
        store
            .query_projection_documents(b.clone().with_payload_budget(4).unwrap())
            .await,
        Err(Error::SpaceInsufficient)
    );
    assert!(
        store
            .query_projection_documents(b.with_payload_budget(6).unwrap())
            .await
            .unwrap()
            .records()[0]
            .document()
            .is_none()
    );
    store.close().await.unwrap();
}

#[tokio::test]
async fn key_selection_retains_source_active_and_ready_transaction_fences() {
    let temp = TempDir::new().unwrap();
    let store = open(&temp).await;
    let memory = MemoryStorage::new(SourceGeneration::new([9; 32]).unwrap());
    for backend in [&store as &dyn radroots_storage::Storage, &memory] {
        let source = promote(backend).await;
        let q = query(1).with_key_range(range("m.0000", "m.0000")).unwrap();
        assert_eq!(
            backend
                .query_projection_documents_at(q.clone(), source, generation(1))
                .await
                .unwrap()
                .records()[0]
                .key(),
            "m.0000"
        );
        let foreign = EventSourceToken::new(SourceGeneration::new([8; 32]).unwrap(), 0);
        assert_eq!(
            backend
                .query_projection_documents_at(q.clone(), foreign, generation(1))
                .await,
            Err(Error::SourceEpochChanged)
        );
        assert_eq!(
            backend
                .query_projection_documents_at(q.clone(), source, generation(2))
                .await,
            Err(Error::InvalidProjectionDocument)
        );
        backend
            .invalidate(
                ProjectionInvalidation::new(
                    id(),
                    generation(1),
                    generation(2),
                    InvalidationReason::ProjectionGenerationChanged,
                    10,
                )
                .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            backend
                .query_projection_documents_at(q, source, generation(1))
                .await,
            Err(Error::ProjectionRevisionConflict)
        );
        assert_eq!(
            backend
                .projection_document(id(), generation(1), "m.0000".into())
                .await
                .unwrap()
                .unwrap()
                .value(),
            &[1; 4]
        );
    }
    store.close().await.unwrap();
}
