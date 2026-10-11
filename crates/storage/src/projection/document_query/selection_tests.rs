use super::*;

fn generation(byte: u8) -> ProjectionGeneration {
    ProjectionGeneration::new([byte; 32]).unwrap()
}
fn query() -> ProjectionDocumentQuery {
    ProjectionDocumentQuery::new(
        ProjectionId::parse("fixture").unwrap(),
        ProjectionDocumentGenerations::Exact(generation(1)),
        2,
    )
    .unwrap()
}
fn range(first: &str, last: &str) -> ProjectionDocumentKeyRange {
    ProjectionDocumentKeyRange::new(first.into(), last.into()).unwrap()
}
fn record(key: &str, bytes: usize) -> ProjectionDocumentRecord {
    ProjectionDocumentRecord::new(
        generation(1),
        ProjectionDocument::new(key.into(), vec![1; bytes]).unwrap(),
    )
    .unwrap()
}

#[test]
fn selection_validates_key_and_payload_boundaries_before_provider_work() {
    for key in ["", " bad", "bad\n", &"x".repeat(513), &"é".repeat(257)] {
        assert!(ProjectionDocumentKeyRange::new(key.into(), key.into()).is_err());
        assert!(query().with_after_key(key.into()).is_err());
    }
    let maximum = "é".repeat(256);
    assert_eq!(range(&maximum, &maximum).first().len(), 512);
    assert!(query().with_after_key(maximum).is_ok());
    assert!(ProjectionDocumentKeyRange::new("z".into(), "a".into()).is_err());
    for bytes in [0, PROJECTION_DOCUMENT_PAGE_BYTES_MAX + 1, usize::MAX] {
        assert!(query().with_payload_budget(bytes).is_err());
    }
    for bytes in [1, PROJECTION_DOCUMENT_PAGE_BYTES_MAX] {
        assert_eq!(
            query().with_payload_budget(bytes).unwrap().payload_budget(),
            bytes
        );
    }
    let all = ProjectionDocumentQuery::new(
        query().projection_id().clone(),
        ProjectionDocumentGenerations::All,
        1,
    )
    .unwrap();
    assert!(all.clone().with_key_range(range("a", "z")).is_err());
    assert!(all.with_after_key("a".into()).is_err());
    for key in ["a", "z"] {
        assert!(
            query()
                .with_key_range(range("b", "y"))
                .unwrap()
                .with_after_key(key.into())
                .is_err()
        );
    }
    assert!(
        query()
            .with_after_key("c".into())
            .unwrap()
            .with_key_range(range("a", "z"))
            .is_err()
    );
}

#[test]
fn selected_continuations_cannot_widen_range_or_cross_generation() {
    let selected = query().with_key_range(range("b", "é")).unwrap();
    let page =
        ProjectionDocumentPage::new(&selected, vec![record("b", 1), record("z", 1)], true).unwrap();
    let cursor = page.next_cursor().unwrap();
    let resumed = selected.clone().with_cursor(cursor).unwrap();
    let sought = selected.clone().with_after_key("z".into()).unwrap();
    assert_eq!(resumed, sought);
    assert!(ProjectionDocumentPage::new(&sought, vec![record("é", 1)], false).is_ok());
    for changed in [
        query(),
        query().with_key_range(range("a", "é")).unwrap(),
        ProjectionDocumentQuery::new(
            query().projection_id().clone(),
            ProjectionDocumentGenerations::Exact(generation(2)),
            2,
        )
        .unwrap()
        .with_key_range(range("b", "é"))
        .unwrap(),
    ] {
        assert!(changed.with_cursor(cursor).is_err());
    }
    for key in ["a", "ê"] {
        assert!(ProjectionDocumentPage::new(&selected, vec![record(key, 1)], false).is_err());
    }
    let exact = query().with_key_range(range("é", "é")).unwrap();
    assert_eq!(
        ProjectionDocumentPage::new(&exact, vec![record("é", 1)], false)
            .unwrap()
            .records()
            .len(),
        1
    );
    let budget = selected.with_payload_budget(7).unwrap();
    assert!(
        ProjectionDocumentPage::new(&budget, vec![record("b", 4), record("c", 4)], false).is_err()
    );
}
