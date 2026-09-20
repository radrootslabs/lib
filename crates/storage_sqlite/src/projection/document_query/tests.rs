use super::*;
use crate::{OpenMode, OpenOptions, Paths};
use radroots_storage::{
    ProjectionStore, event::SourceGeneration, memory::MemoryStorage, projection::ProjectionId,
};
use tempfile::TempDir;

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
fn generation(byte: u8) -> ProjectionGeneration {
    ProjectionGeneration::new([byte; 32]).unwrap()
}
fn query(selection: ProjectionDocumentGenerations, limit: u16) -> ProjectionDocumentQuery {
    ProjectionDocumentQuery::new(ProjectionId::parse("fixture").unwrap(), selection, limit).unwrap()
}
async fn put(store: &dyn ProjectionStore, generation: u8, key: &str, value: Vec<u8>) {
    store
        .put_projection_document(
            ProjectionId::parse("fixture").unwrap(),
            self::generation(generation),
            ProjectionDocument::new(key.into(), value).unwrap(),
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn inventory_thousand_records_matches_memory_across_generations_updates_and_reopen() {
    let temp = TempDir::new().unwrap();
    let store = open(&temp).await;
    let memory = MemoryStorage::new(SourceGeneration::new([9; 32]).unwrap());
    for i in (0..1000).rev() {
        for backend in [&store as &dyn ProjectionStore, &memory] {
            put(backend, 1 + (i % 2) as u8, &format!("key.{i:04}"), vec![1]).await;
        }
    }
    store
        .put_projection_document(
            ProjectionId::parse("foreign").unwrap(),
            generation(1),
            ProjectionDocument::new("key.0000".into(), vec![3]).unwrap(),
        )
        .await
        .unwrap();
    let q = query(ProjectionDocumentGenerations::All, 37);
    let first = store.query_projection_documents(q.clone()).await.unwrap();
    assert_eq!(
        first,
        memory.query_projection_documents(q.clone()).await.unwrap()
    );
    for backend in [&store as &dyn ProjectionStore, &memory] {
        put(backend, 1, "key.0000", vec![2]).await;
        put(backend, 2, "key.0999", vec![2]).await;
    }
    store.close().await.unwrap();
    let store = open(&temp).await;
    let mut count = first.records().len();
    let mut cursor = first.next_cursor().cloned();
    while let Some(current) = cursor {
        let query = q.clone().with_cursor(&current).unwrap();
        let page = store
            .query_projection_documents(query.clone())
            .await
            .unwrap();
        assert_eq!(
            page,
            memory.query_projection_documents(query).await.unwrap()
        );
        count += page.records().len();
        cursor = page.next_cursor().cloned();
    }
    assert_eq!(count, 1000);
    for byte in [1, 2, 3] {
        let q = query(ProjectionDocumentGenerations::Exact(generation(byte)), 256);
        let page = store.query_projection_documents(q.clone()).await.unwrap();
        assert_eq!(
            page,
            memory.query_projection_documents(q.clone()).await.unwrap()
        );
        assert!(
            page.records()
                .iter()
                .all(|r| r.generation() == generation(byte))
        );
        if byte == 3 {
            assert!(page.records().is_empty());
        } else {
            let last = store
                .query_projection_documents(q.with_cursor(page.next_cursor().unwrap()).unwrap())
                .await
                .unwrap();
            assert_eq!(last.records().len() + page.records().len(), 500);
            assert!(last.next_cursor().is_none());
        }
    }
    store.close().await.unwrap();
    assert_eq!(
        store.query_projection_documents(q).await,
        Err(Error::BackendUnavailable)
    );
}

#[tokio::test]
async fn inventory_readonly_preserves_identical_keys_and_byte_order() {
    let temp = TempDir::new().unwrap();
    let store = open(&temp).await;
    for byte in [1, 2] {
        for key in ["same", "z", "é"] {
            put(&store, byte, key, vec![byte]).await;
        }
    }
    store.close().await.unwrap();
    let store = SqliteStorage::open(OpenOptions::new(
        Paths::from_directory(temp.path()).unwrap(),
        OpenMode::ReadOnly,
    ))
    .await
    .unwrap();
    let q = query(ProjectionDocumentGenerations::All, 1);
    let mut current = q.clone();
    let mut found = Vec::new();
    loop {
        let page = store.query_projection_documents(current).await.unwrap();
        found.extend(
            page.records()
                .iter()
                .map(|r| (r.generation(), r.key().to_owned())),
        );
        let Some(cursor) = page.next_cursor() else {
            break;
        };
        current = q.clone().with_cursor(cursor).unwrap();
    }
    assert_eq!(
        found,
        [1, 2]
            .into_iter()
            .flat_map(|byte| ["same", "z", "é"].map(|key| (generation(byte), key.to_owned())))
            .collect::<Vec<_>>()
    );
    let exact = query(ProjectionDocumentGenerations::Exact(generation(1)), 1);
    let page = store.query_projection_documents(exact).await.unwrap();
    assert!(
        query(ProjectionDocumentGenerations::Exact(generation(2)), 1)
            .with_cursor(page.next_cursor().unwrap())
            .is_err()
    );
    store.close().await.unwrap();
}

#[tokio::test]
async fn inventory_bounds_bytes_including_corrupt_payloads_without_losing_continuation() {
    let temp = TempDir::new().unwrap();
    let store = open(&temp).await;
    put(&store, 1, "a", vec![7; PROJECTION_DOCUMENT_PAGE_BYTES_MAX]).await;
    put(&store, 1, "b", vec![8]).await;
    let q = query(ProjectionDocumentGenerations::All, 256);
    for corrupt in [false, true] {
        if corrupt {
            sqlx::query("UPDATE radroots_runtime_projection_documents SET value_sha256 = zeroblob(32) WHERE document_key = 'a'")
                .execute(store.pool()).await.unwrap();
        }
        let first = store.query_projection_documents(q.clone()).await.unwrap();
        assert_eq!(first.records().len(), 1);
        assert_eq!(first.records()[0].document().is_none(), corrupt);
        let next = store
            .query_projection_documents(
                q.clone().with_cursor(first.next_cursor().unwrap()).unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(next.records()[0].key(), "b");
        assert!(next.next_cursor().is_none());
    }
    store.close().await.unwrap();
}

#[tokio::test]
async fn inventory_exposes_bounded_corruption_and_rejects_unpageable_keys() {
    let temp = TempDir::new().unwrap();
    let store = open(&temp).await;
    // One held connection makes disabled-check injection local to this fixture.
    let mut connection = store.pool().acquire().await.unwrap();
    sqlx::query("PRAGMA ignore_check_constraints = ON")
        .execute(&mut *connection)
        .await
        .unwrap();
    for (key, size, digest) in [("a", 0, 32), ("b", 16777217, 32), ("c", 1, 33)] {
        sqlx::query("INSERT INTO radroots_runtime_projection_documents VALUES ('fixture', ?, ?, zeroblob(?), zeroblob(?))")
            .bind(generation(1).as_bytes().as_slice()).bind(key).bind(size).bind(digest)
            .execute(&mut *connection).await.unwrap();
    }
    sqlx::query("PRAGMA ignore_check_constraints = OFF")
        .execute(&mut *connection)
        .await
        .unwrap();
    drop(connection);
    put(&store, 1, "d", vec![1]).await;
    let q = query(ProjectionDocumentGenerations::All, 2);
    let page = store.query_projection_documents(q.clone()).await.unwrap();
    assert!(page.records().iter().all(|r| r.document().is_none()));
    let next = store
        .query_projection_documents(q.with_cursor(page.next_cursor().unwrap()).unwrap())
        .await
        .unwrap();
    assert!(next.records()[0].document().is_none());
    assert!(next.records()[1].document().is_some());
    assert!(next.next_cursor().is_none());
    for key in ["\n".to_owned(), "é".repeat(512)] {
        sqlx::query("INSERT INTO radroots_runtime_projection_documents VALUES ('fixture', ?, ?, x'01', zeroblob(32))")
            .bind(generation(2).as_bytes().as_slice()).bind(&key).execute(store.pool()).await.unwrap();
        assert_eq!(
            store
                .query_projection_documents(query(
                    ProjectionDocumentGenerations::Exact(generation(2)),
                    1
                ))
                .await,
            Err(Error::CorruptProjectionDocument)
        );
        sqlx::query("DELETE FROM radroots_runtime_projection_documents WHERE generation = ?")
            .bind(generation(2).as_bytes().as_slice())
            .execute(store.pool())
            .await
            .unwrap();
    }
    sqlx::query("INSERT INTO radroots_runtime_projection_documents VALUES ('fixture', zeroblob(32), 'zero', x'01', zeroblob(32))")
        .execute(store.pool()).await.unwrap();
    assert_eq!(
        store
            .query_projection_documents(query(ProjectionDocumentGenerations::All, 1))
            .await,
        Err(Error::CorruptProjectionDocument)
    );
    store.close().await.unwrap();
}
