use super::*;

fn generation() -> ProjectionGeneration {
    ProjectionGeneration::new([1; 32]).unwrap()
}
fn query(limit: u16) -> ProjectionDocumentQuery {
    ProjectionDocumentQuery::new(
        ProjectionId::parse("fixture").unwrap(),
        ProjectionDocumentGenerations::All,
        limit,
    )
    .unwrap()
}
fn record(key: &str, size: usize) -> ProjectionDocumentRecord {
    ProjectionDocumentRecord::new(
        generation(),
        ProjectionDocument::new(key.into(), vec![123; size]).unwrap(),
    )
    .unwrap()
}

#[test]
fn independent_scope_and_bounded_page_construction() {
    let q = query(2);
    for limit in [0, 257] {
        assert!(
            ProjectionDocumentQuery::new(q.projection_id().clone(), q.generations(), limit)
                .is_err()
        );
    }
    for key in ["", " bad", "bad\n", &"x".repeat(513)] {
        assert!(ProjectionDocumentRecord::corrupt(generation(), key.into()).is_err());
    }
    let corrupt = ProjectionDocumentRecord::corrupt(generation(), "b".into()).unwrap();
    let page =
        ProjectionDocumentPage::new(&q, vec![record("a", 1), corrupt.clone()], true).unwrap();
    let cursor = page.next_cursor().unwrap();
    assert!(corrupt.document().is_none());
    assert_eq!(corrupt.generation(), generation());
    assert_eq!(corrupt.key(), "b");
    assert_eq!(page.clone().into_records(), page.records());
    assert!(!format!("{:?}", record("redacted", 100)).contains("123"));
    assert_eq!(
        q.clone().with_cursor(cursor).unwrap().after(),
        Some((generation(), "b"))
    );
    for changed in [
        ProjectionDocumentQuery::new(ProjectionId::parse("other").unwrap(), q.generations(), 1)
            .unwrap(),
        ProjectionDocumentQuery::new(
            q.projection_id().clone(),
            ProjectionDocumentGenerations::Exact(generation()),
            1,
        )
        .unwrap(),
    ] {
        assert!(changed.with_cursor(cursor).is_err());
    }
    assert!(ProjectionDocumentPage::new(&q, vec![], true).is_err());
    assert!(ProjectionDocumentPage::new(&q, vec![record("a", 1), record("a", 1)], false).is_err());
    assert!(
        ProjectionDocumentPage::new(&query(1), vec![record("a", 1), record("b", 1)], false)
            .is_err()
    );
    let resumed = q.clone().with_cursor(cursor).unwrap();
    assert!(ProjectionDocumentPage::new(&resumed, vec![record("b", 1)], false).is_err());
    assert!(
        ProjectionDocumentPage::new(
            &q,
            vec![
                record("a", PROJECTION_DOCUMENT_PAGE_BYTES_MAX),
                record("b", 1)
            ],
            false
        )
        .is_err()
    );
    let exact = ProjectionDocumentQuery::new(
        q.projection_id().clone(),
        ProjectionDocumentGenerations::Exact(ProjectionGeneration::new([2; 32]).unwrap()),
        2,
    )
    .unwrap();
    assert!(ProjectionDocumentPage::new(&exact, vec![record("a", 1)], false).is_err());
    assert!(
        ProjectionDocumentPage::new(&q, vec![], false)
            .unwrap()
            .next_cursor()
            .is_none()
    );
}

#[cfg(feature = "serde")]
#[test]
fn unchecked_legacy_serde_values_cannot_enter_queries_or_records() {
    let invalid_id: ProjectionId = serde_json::from_str("\"bad id\"").unwrap();
    let zero: ProjectionGeneration =
        serde_json::from_str(&serde_json::to_string(&[0; 32]).unwrap()).unwrap();
    assert!(
        ProjectionDocumentQuery::new(invalid_id, ProjectionDocumentGenerations::All, 1).is_err()
    );
    assert!(
        ProjectionDocumentQuery::new(
            query(1).projection_id().clone(),
            ProjectionDocumentGenerations::Exact(zero),
            1
        )
        .is_err()
    );
    assert!(ProjectionDocumentRecord::corrupt(zero, "key".into()).is_err());
    assert!(
        ProjectionDocumentRecord::new(
            zero,
            ProjectionDocument::new("key".into(), vec![1]).unwrap()
        )
        .is_err()
    );
}

#[cfg(feature = "memory")]
#[test]
fn memory_inventory_continues_after_byte_exhaustion_and_reports_closed_store() {
    use crate::{ProjectionStore, event::SourceGeneration, memory::MemoryStorage};
    futures_executor::block_on(async {
        let store = MemoryStorage::new(SourceGeneration::new([1; 32]).unwrap());
        let q = query(256);
        for (key, size) in [("a", PROJECTION_DOCUMENT_PAGE_BYTES_MAX), ("b", 1)] {
            store
                .put_projection_document(
                    q.projection_id().clone(),
                    generation(),
                    ProjectionDocument::new(key.into(), vec![1; size]).unwrap(),
                )
                .await
                .unwrap();
        }
        let first = store.query_projection_documents(q.clone()).await.unwrap();
        assert_eq!(first.records().len(), 1);
        let second = store
            .query_projection_documents(q.with_cursor(first.next_cursor().unwrap()).unwrap())
            .await
            .unwrap();
        assert_eq!(second.records()[0].key(), "b");
        assert!(second.next_cursor().is_none());
        crate::backup::StorageReliability::close(&store)
            .await
            .unwrap();
        assert_eq!(
            store.query_projection_documents(query(1)).await,
            Err(Error::BackendUnavailable)
        );
    });
}
