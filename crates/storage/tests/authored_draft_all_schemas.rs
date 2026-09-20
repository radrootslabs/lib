use futures_executor::block_on;
use radroots_storage::{
    authored_draft::{AuthoredDraft, AuthoredDraftId, AuthoredDraftStage, AuthoredDraftStore},
    authored_draft_query::{
        AuthoredDraftCursor, AuthoredDraftPage, AuthoredDraftQuery, AuthoredDraftQueryRecord,
        AuthoredDraftScope,
    },
    memory::MemoryStorage,
};

fn draft(id: u128, author: u8, schema: &str, payload: Vec<u8>) -> AuthoredDraft {
    let value = AuthoredDraft::initial(
        AuthoredDraftId::new(id.to_be_bytes()).unwrap(),
        [author; 32],
        schema,
        payload,
        AuthoredDraftStage::Draft,
        None,
        10,
    )
    .unwrap();
    if id.is_multiple_of(2) {
        value
            .with_scope(AuthoredDraftScope::new([3; 32]).unwrap())
            .unwrap()
    } else {
        value
    }
}

#[test]
fn all_schema_cursor_has_explicit_independent_authority_and_no_wildcard_schema() {
    let q = AuthoredDraftQuery::for_author_all_schemas([7; 32], 37).unwrap();
    assert!(q.is_author_wide());
    assert_eq!(q.scope(), None);
    assert_eq!(q.payload_schema(), None);
    for (author, limit) in [([0; 32], 1), ([7; 32], 0), ([7; 32], 257)] {
        assert!(AuthoredDraftQuery::for_author_all_schemas(author, limit).is_err());
    }
    let cursor = q.cursor_after([0; 16]);
    let encoded = serde_json::to_string(&cursor).unwrap();
    let expected = format!(
        "{{\"schema_version\":3,\"author\":{},\"payload_schema\":null,\"scope\":null,\"after_id\":{},\"selection\":\"author_all_schemas\"}}",
        serde_json::to_string(&[7; 32]).unwrap(),
        serde_json::to_string(&[0; 16]).unwrap()
    );
    assert_eq!(encoded, expected);
    let decoded: AuthoredDraftCursor = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, cursor);
    assert_eq!(
        q.clone().with_cursor(&decoded).unwrap().after(),
        Some([0; 16])
    );
    assert_eq!(serde_json::to_string(&decoded).unwrap(), encoded);
    for other in [
        AuthoredDraftQuery::new([7; 32], "known.v1", None, 1).unwrap(),
        AuthoredDraftQuery::for_author([7; 32], "known.v1", 1).unwrap(),
        AuthoredDraftQuery::for_author_all_schemas([8; 32], 1).unwrap(),
    ] {
        assert!(q.clone().with_cursor(&other.cursor_after([0; 16])).is_err());
        assert!(other.with_cursor(&cursor).is_err());
    }
    let wire = serde_json::to_value(&cursor).unwrap();
    for (field, value) in [
        ("schema_version", serde_json::json!(1)),
        ("schema_version", serde_json::json!(2)),
        ("schema_version", serde_json::json!(4)),
        ("selection", serde_json::json!("author_schema")),
        ("selection", serde_json::Value::Null),
        ("selection", serde_json::json!("future")),
        ("payload_schema", serde_json::json!("known.v1")),
        ("payload_schema", serde_json::json!("")),
        ("scope", serde_json::json!([3; 32].to_vec())),
        ("author", serde_json::json!([0; 32].to_vec())),
        ("after_id", serde_json::json!([0; 15].to_vec())),
        ("unexpected", serde_json::json!(true)),
    ] {
        let mut changed = wire.clone();
        changed[field] = value;
        assert!(
            serde_json::from_value::<AuthoredDraftCursor>(changed).is_err(),
            "{field}"
        );
    }
    for field in [
        "schema_version",
        "author",
        "payload_schema",
        "scope",
        "after_id",
        "selection",
    ] {
        let mut changed = wire.clone();
        changed.as_object_mut().unwrap().remove(field);
        assert!(
            serde_json::from_value::<AuthoredDraftCursor>(changed).is_err(),
            "missing {field}"
        );
    }
    let duplicate = encoded.replace("\"scope\":null", "\"scope\":null,\"scope\":null");
    assert!(serde_json::from_str::<AuthoredDraftCursor>(&duplicate).is_err());
    let exact = AuthoredDraftQuery::for_author([7; 32], "known.v1", 1).unwrap();
    let mut wrong_marker = serde_json::to_value(exact.cursor_after([0; 16])).unwrap();
    wrong_marker["selection"] = serde_json::json!("author_all_schemas");
    assert!(serde_json::from_value::<AuthoredDraftCursor>(wrong_marker).is_err());
    assert_eq!(exact.payload_schema(), Some("known.v1"));
    assert!(AuthoredDraftQuery::new([7; 32], "", None, 1).is_err());
}

#[test]
fn all_schema_memory_inventory_covers_unknown_schemas_and_scopes_beyond_one_thousand() {
    let store = MemoryStorage::default();
    for id in (1..=1001).rev() {
        let schema = if id % 3 == 0 {
            "future.unknown.v999"
        } else {
            "known.v1"
        };
        block_on(store.append_authored_draft(draft(id, 7, schema, vec![1]), None)).unwrap();
    }
    block_on(store.append_authored_draft(draft(1002, 8, "future.unknown.v999", vec![1]), None))
        .unwrap();
    let q = AuthoredDraftQuery::for_author_all_schemas([7; 32], 37).unwrap();
    let mut next = None;
    let mut found = Vec::new();
    loop {
        let query = next.as_ref().map_or_else(
            || q.clone(),
            |cursor| q.clone().with_cursor(cursor).unwrap(),
        );
        let page = block_on(store.query_authored_drafts(query)).unwrap();
        assert!(page.records().len() <= 37);
        for record in page.records() {
            let AuthoredDraftQueryRecord::Draft(value) = record else {
                panic!("valid fixture")
            };
            let id = u128::from_be_bytes(record.draft_key());
            found.push(id);
            assert_eq!(
                value.payload_schema(),
                if id % 3 == 0 {
                    "future.unknown.v999"
                } else {
                    "known.v1"
                }
            );
            if id == 1001 {
                assert_eq!(value.revision().get(), 2);
            }
        }
        next = page.next_cursor().cloned();
        if found.len() == 37 {
            for id in [1_u128, 1001] {
                let value = block_on(
                    store.authored_draft_head(AuthoredDraftId::new(id.to_be_bytes()).unwrap()),
                )
                .unwrap()
                .unwrap();
                let revised = value
                    .successor(vec![2], AuthoredDraftStage::Draft, None, 11)
                    .unwrap();
                block_on(store.append_authored_draft(revised, Some(value.revision()))).unwrap();
            }
        }
        if next.is_none() {
            break;
        }
    }
    assert_eq!(found, (1..=1001).collect::<Vec<_>>());
    let fresh = block_on(store.query_authored_drafts(q)).unwrap();
    assert_eq!(fresh.records()[0].revision().get(), 2);
}

#[test]
fn all_schema_memory_pages_keep_payload_bounds_and_validate_authority() {
    let store = MemoryStorage::default();
    let first = draft(1, 7, "known.v1", vec![1; 3 * 1024 * 1024]);
    let second = draft(2, 7, "future.v999", vec![2; 3 * 1024 * 1024]);
    for value in [first.clone(), second.clone()] {
        block_on(store.append_authored_draft(value, None)).unwrap();
    }
    let q = AuthoredDraftQuery::for_author_all_schemas([7; 32], 256).unwrap();
    let page = block_on(store.query_authored_drafts(q.clone())).unwrap();
    assert_eq!(
        page.records(),
        [AuthoredDraftQueryRecord::Draft(first.clone())]
    );
    let next = block_on(
        store.query_authored_drafts(q.clone().with_cursor(page.next_cursor().unwrap()).unwrap()),
    )
    .unwrap();
    assert_eq!(
        next.records(),
        [AuthoredDraftQueryRecord::Draft(second.clone())]
    );
    assert!(next.next_cursor().is_none());
    assert!(
        AuthoredDraftPage::new(
            &q,
            vec![
                AuthoredDraftQueryRecord::Draft(first),
                AuthoredDraftQueryRecord::Draft(second)
            ],
            false
        )
        .is_err()
    );
    let foreign = AuthoredDraftQueryRecord::Draft(draft(3, 8, "known.v1", vec![1]));
    assert!(AuthoredDraftPage::new(&q, vec![foreign], false).is_err());
}
