use futures_executor::block_on;
use radroots_storage::{
    Error,
    authored_draft::{AuthoredDraft, AuthoredDraftId, AuthoredDraftStage, AuthoredDraftStore},
    authored_draft_query::{
        AUTHORED_DRAFT_PAGE_PAYLOAD_MAX_BYTES, AuthoredDraftCursor, AuthoredDraftPage,
        AuthoredDraftQuery, AuthoredDraftQueryRecord, AuthoredDraftScope,
    },
    memory::MemoryStorage,
};

fn draft(
    id: u8,
    schema: &str,
    scope: Option<AuthoredDraftScope>,
    payload: Vec<u8>,
) -> AuthoredDraft {
    let draft = AuthoredDraft::initial(
        AuthoredDraftId::new([id; 16]).unwrap(),
        [9; 32],
        schema,
        payload,
        AuthoredDraftStage::Draft,
        None,
        10,
    )
    .unwrap();
    scope.map_or_else(
        || draft.clone(),
        |scope| draft.clone().with_scope(scope).unwrap(),
    )
}
fn query(scope: Option<AuthoredDraftScope>, limit: u16) -> AuthoredDraftQuery {
    AuthoredDraftQuery::new([9; 32], "fixture.composer.v1", scope, limit).unwrap()
}

#[test]
fn author_wide_cursor_requires_explicit_selection_and_independent_authority() {
    let q = AuthoredDraftQuery::for_author([9; 32], "fixture.composer.v1", 256).unwrap();
    assert!(q.is_author_wide());
    assert_eq!(q.scope(), None);
    assert!(!query(None, 1).is_author_wide());
    assert!(AuthoredDraftQuery::for_author([0; 32], "fixture.composer.v1", 1).is_err());
    let cursor = q.cursor_after([3; 16]);
    let bytes = serde_json::to_vec(&cursor).unwrap();
    let decoded: AuthoredDraftCursor = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(decoded, cursor);
    assert_eq!(
        q.clone().with_cursor(&decoded).unwrap().after(),
        Some([3; 16])
    );
    assert_eq!(serde_json::to_vec(&decoded).unwrap(), bytes);
    let wire = serde_json::to_value(&cursor).unwrap();
    assert_eq!(wire["schema_version"], 2);
    assert_eq!(wire["selection"], "author_schema");
    assert!(wire["scope"].is_null());
    for changed in [
        query(None, 1),
        query(Some(AuthoredDraftScope::new([7; 32]).unwrap()), 1),
        AuthoredDraftQuery::for_author([8; 32], q.payload_schema(), 1).unwrap(),
        AuthoredDraftQuery::for_author([9; 32], "fixture.other.v1", 1).unwrap(),
    ] {
        assert!(changed.with_cursor(&decoded).is_err());
    }
    assert!(
        q.with_cursor(&query(None, 1).cursor_after([3; 16]))
            .is_err()
    );
    for (field, value) in [
        ("schema_version", serde_json::json!(1)),
        ("schema_version", serde_json::json!(3)),
        ("selection", serde_json::Value::Null),
        ("selection", serde_json::json!("other")),
        ("scope", serde_json::json!([7; 32].to_vec())),
        ("author", serde_json::json!([0; 32].to_vec())),
        ("payload_schema", serde_json::json!("")),
        ("unknown", serde_json::json!(true)),
    ] {
        let mut forged = wire.clone();
        forged[field] = value;
        assert!(
            serde_json::from_value::<AuthoredDraftCursor>(forged).is_err(),
            "{field}"
        );
    }
    let mut absent = wire;
    let mut absent_scope = absent.clone();
    absent_scope.as_object_mut().unwrap().remove("scope");
    assert!(serde_json::from_value::<AuthoredDraftCursor>(absent_scope).is_err());
    absent.as_object_mut().unwrap().remove("selection");
    assert!(serde_json::from_value::<AuthoredDraftCursor>(absent).is_err());
    let old = query(None, 1).cursor_after([3; 16]);
    let mut forbidden = serde_json::to_value(&old).unwrap();
    forbidden["selection"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<AuthoredDraftCursor>(forbidden).is_err());
    let expected = format!(
        "{{\"schema_version\":1,\"author\":{},\"payload_schema\":\"fixture.composer.v1\",\"scope\":null,\"after_id\":{}}}",
        serde_json::to_string(&[9; 32]).unwrap(),
        serde_json::to_string(&[3; 16]).unwrap()
    );
    assert_eq!(serde_json::to_string(&old).unwrap(), expected);
}

#[test]
fn author_wide_memory_pages_cover_a_thousand_scopes_and_preserve_bounds() {
    let store = MemoryStorage::default();
    let scope = AuthoredDraftScope::new([7; 32]).unwrap();
    for id in 1_u128..=1000 {
        let value = AuthoredDraft::initial(
            AuthoredDraftId::new(id.to_be_bytes()).unwrap(),
            [9; 32],
            "fixture.composer.v1",
            vec![1],
            AuthoredDraftStage::Draft,
            None,
            10,
        )
        .unwrap();
        let value = if id % 2 == 0 {
            value.with_scope(scope).unwrap()
        } else {
            value
        };
        block_on(store.append_authored_draft(value, None)).unwrap();
    }
    for (id, author, schema) in [
        (1001_u128, [8; 32], "fixture.composer.v1"),
        (1002, [9; 32], "fixture.other.v1"),
    ] {
        let value = AuthoredDraft::initial(
            AuthoredDraftId::new(id.to_be_bytes()).unwrap(),
            author,
            schema,
            vec![1],
            AuthoredDraftStage::Draft,
            None,
            10,
        )
        .unwrap();
        block_on(store.append_authored_draft(value, None)).unwrap();
    }
    let q = AuthoredDraftQuery::for_author([9; 32], "fixture.composer.v1", 37).unwrap();
    let mut cursor = None;
    let mut expected = 1_u128;
    loop {
        let query = cursor.as_ref().map_or_else(
            || q.clone(),
            |cursor| q.clone().with_cursor(cursor).unwrap(),
        );
        let page = block_on(store.query_authored_drafts(query)).unwrap();
        assert!(page.records().len() <= 37);
        for record in page.records() {
            assert_eq!(record.draft_key(), expected.to_be_bytes());
            expected += 1;
        }
        cursor = page.next_cursor().cloned();
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(expected, 1001);
    for id in 1003_u128..=1005 {
        let value = AuthoredDraft::initial(
            AuthoredDraftId::new(id.to_be_bytes()).unwrap(),
            [9; 32],
            "fixture.large.v1",
            vec![1; 2 * 1024 * 1024],
            AuthoredDraftStage::Draft,
            None,
            10,
        )
        .unwrap();
        let value = if id % 2 == 0 {
            value.with_scope(scope).unwrap()
        } else {
            value
        };
        block_on(store.append_authored_draft(value, None)).unwrap();
    }
    let q = AuthoredDraftQuery::for_author([9; 32], "fixture.large.v1", 256).unwrap();
    let first = block_on(store.query_authored_drafts(q.clone())).unwrap();
    assert_eq!(first.records().len(), 2);
    let second =
        block_on(store.query_authored_drafts(q.with_cursor(first.next_cursor().unwrap()).unwrap()))
            .unwrap();
    assert_eq!(second.records().len(), 1);
    assert!(second.next_cursor().is_none());
}

#[test]
fn scoped_pages_preserve_revisions_and_do_not_mix_other_schemas_or_scopes() {
    let store = MemoryStorage::default();
    let scope = AuthoredDraftScope::new([7; 32]).unwrap();
    let first = draft(
        2,
        "fixture.composer.v1",
        Some(scope),
        b"unfinished 1.".to_vec(),
    );
    let second = draft(
        4,
        "fixture.composer.v1",
        Some(scope),
        b"partial date 2026-".to_vec(),
    );
    for value in [
        first.clone(),
        second.clone(),
        draft(1, "fixture.profile.v1", Some(scope), b"profile".to_vec()),
        draft(3, "fixture.composer.v1", None, b"other context".to_vec()),
    ] {
        block_on(store.append_authored_draft(value, None)).unwrap();
    }
    let page = block_on(store.query_authored_drafts(query(Some(scope), 1))).unwrap();
    assert_eq!(
        page.records(),
        [AuthoredDraftQueryRecord::Draft(first.clone())]
    );
    assert_eq!(page.records()[0].draft_id().unwrap(), first.draft_id());
    assert_eq!(page.records()[0].revision(), first.revision());
    let cursor = page.next_cursor().unwrap();
    let changed = first
        .successor(b"later edit".to_vec(), AuthoredDraftStage::Draft, None, 11)
        .unwrap();
    assert_eq!(changed.scope(), Some(scope));
    block_on(store.append_authored_draft(changed.clone(), Some(first.revision()))).unwrap();
    let next =
        block_on(store.query_authored_drafts(query(Some(scope), 2).with_cursor(cursor).unwrap()))
            .unwrap();
    assert_eq!(next.records(), [AuthoredDraftQueryRecord::Draft(second)]);
    assert!(next.next_cursor().is_none());
    let fresh = block_on(store.query_authored_drafts(query(Some(scope), 1))).unwrap();
    assert_eq!(
        fresh.records(),
        [AuthoredDraftQueryRecord::Draft(changed.clone())]
    );
    assert!(changed.with_scope(scope).is_err());
    assert!(first.clone().with_scope(scope).is_err());
    let mut forged = serde_json::to_value(
        first
            .successor(b"next".to_vec(), AuthoredDraftStage::Draft, None, 11)
            .unwrap(),
    )
    .unwrap();
    forged["scope"] = serde_json::json!([8; 32].to_vec());
    let forged: AuthoredDraft = serde_json::from_value(forged).unwrap();
    assert_eq!(
        forged.validate_successor_of(&first),
        Err(Error::DraftRevisionConflict)
    );
}

#[test]
fn query_and_cursor_reject_invalid_or_changed_authority() {
    assert!(AuthoredDraftScope::new([0; 32]).is_err());
    for schema in ["", " x", "x\n", &"x".repeat(129)] {
        assert!(AuthoredDraftQuery::new([9; 32], schema, None, 1).is_err());
    }
    for (author, limit) in [([0; 32], 1), ([9; 32], 0), ([9; 32], 257)] {
        assert!(AuthoredDraftQuery::new(author, "fixture.composer.v1", None, limit).is_err());
    }
    let q = query(None, 256);
    let cursor = q.cursor_after([0; 16]);
    for changed in [
        AuthoredDraftQuery::new([8; 32], q.payload_schema(), None, 1).unwrap(),
        AuthoredDraftQuery::new([9; 32], "fixture.profile.v1", None, 1).unwrap(),
        query(Some(AuthoredDraftScope::new([7; 32]).unwrap()), 1),
    ] {
        assert!(changed.with_cursor(&cursor).is_err());
    }
    let bytes = serde_json::to_vec(&cursor).unwrap();
    let decoded: AuthoredDraftCursor = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(decoded, cursor);
    assert_eq!(
        q.clone().with_cursor(&decoded).unwrap().after(),
        Some([0; 16])
    );
    for (field, value) in [
        ("schema_version", serde_json::json!(2)),
        ("payload_schema", serde_json::json!("")),
        ("scope", serde_json::json!([0; 32].to_vec())),
        ("unknown", serde_json::json!(true)),
    ] {
        let mut wire = serde_json::to_value(&cursor).unwrap();
        wire[field] = value;
        assert!(serde_json::from_value::<AuthoredDraftCursor>(wire).is_err());
    }
}

#[test]
fn legacy_unscoped_wire_and_scoped_serde_keep_exact_payload_bytes() {
    let legacy = draft(1, "fixture.composer.v1", None, b"0.\n2026-".to_vec());
    let bytes = serde_json::to_vec(&legacy).unwrap();
    assert!(
        !String::from_utf8(bytes.clone())
            .unwrap()
            .contains("\"scope\"")
    );
    let restored: AuthoredDraft = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(restored, legacy);
    assert_eq!(serde_json::to_vec(&restored).unwrap(), bytes);
    let scope = AuthoredDraftScope::new([7; 32]).unwrap();
    assert_eq!(
        AuthoredDraftScope::try_from(<[u8; 32]>::from(scope)).unwrap(),
        scope
    );
    let scoped = legacy.with_scope(scope).unwrap();
    let scoped_bytes = serde_json::to_vec(&scoped).unwrap();
    assert_eq!(
        serde_json::from_slice::<AuthoredDraft>(&scoped_bytes).unwrap(),
        scoped
    );
    let mut malformed = serde_json::to_value(&scoped).unwrap();
    malformed["scope"] = serde_json::json!([0; 32].to_vec());
    assert!(serde_json::from_value::<AuthoredDraft>(malformed).is_err());
}

#[test]
fn page_payload_budget_advances_without_losing_the_next_large_record() {
    let store = MemoryStorage::default();
    for id in [1, 2] {
        block_on(store.append_authored_draft(
            draft(id, "fixture.composer.v1", None, vec![id; 3 * 1024 * 1024]),
            None,
        ))
        .unwrap();
    }
    let q = query(None, 256);
    let first = block_on(store.query_authored_drafts(q.clone())).unwrap();
    assert_eq!(first.records().len(), 1);
    let next = block_on(
        store.query_authored_drafts(q.clone().with_cursor(first.next_cursor().unwrap()).unwrap()),
    )
    .unwrap();
    assert_eq!(next.records()[0].draft_key(), [2; 16]);
    assert!(next.next_cursor().is_none());
    let mut records = first.into_records();
    records.extend(next.into_records());
    let total_payload: usize = records
        .iter()
        .map(|record| match record {
            AuthoredDraftQueryRecord::Draft(draft) => draft.payload().len(),
            AuthoredDraftQueryRecord::Corrupt { .. } => 0,
        })
        .sum();
    assert!(total_payload > AUTHORED_DRAFT_PAGE_PAYLOAD_MAX_BYTES);
    assert!(AuthoredDraftPage::new(&q, records, false).is_err());
    assert!(AuthoredDraftPage::new(&q, vec![], true).is_err());
    let record = AuthoredDraftQueryRecord::Draft(draft(1, "fixture.profile.v1", None, vec![1]));
    assert!(AuthoredDraftPage::new(&q, vec![record], false).is_err());
    let record = AuthoredDraftQueryRecord::Draft(draft(1, "fixture.composer.v1", None, vec![1]));
    assert!(AuthoredDraftPage::new(&q, vec![record.clone(), record.clone()], false).is_err());
    assert!(AuthoredDraftPage::new(&query(None, 1), vec![record.clone(), record], false).is_err());
}

#[test]
fn bounded_reference_pages_scan_a_thousand_reversed_heads_and_preserve_newer_revisions() {
    let store = MemoryStorage::default();
    for number in (1u16..=1000).rev() {
        let mut id = [0; 16];
        id[14..].copy_from_slice(&number.to_be_bytes());
        let value = AuthoredDraft::initial(
            radroots_storage::authored_draft::AuthoredDraftId::new(id).unwrap(),
            [9; 32],
            "fixture.composer.v1",
            number.to_be_bytes().to_vec(),
            AuthoredDraftStage::Draft,
            None,
            10,
        )
        .unwrap();
        block_on(store.append_authored_draft(value, None)).unwrap();
    }
    let q = query(None, 127);
    let mut cursor = None;
    let mut found = Vec::new();
    loop {
        let current = cursor.as_ref().map_or_else(
            || q.clone(),
            |cursor| q.clone().with_cursor(cursor).unwrap(),
        );
        let page = block_on(store.query_authored_drafts(current)).unwrap();
        assert!(page.records().len() <= usize::from(q.limit()));
        for record in page.records() {
            let key = record.draft_key();
            found.push(u16::from_be_bytes([key[14], key[15]]));
        }
        cursor = page.next_cursor().cloned();
        if found.len() == 127 {
            let AuthoredDraftQueryRecord::Draft(first) = &page.records()[0] else {
                panic!("valid fixture")
            };
            let edited = first
                .successor(
                    b"newer source".to_vec(),
                    AuthoredDraftStage::Draft,
                    None,
                    11,
                )
                .unwrap();
            block_on(store.append_authored_draft(edited, Some(first.revision()))).unwrap();
        }
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(found, (1u16..=1000).collect::<Vec<_>>());
    let fresh = block_on(store.query_authored_drafts(query(None, 1))).unwrap();
    let AuthoredDraftQueryRecord::Draft(first) = &fresh.records()[0] else {
        panic!("valid fixture")
    };
    assert_eq!(first.payload(), b"newer source");
}
