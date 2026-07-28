use super::*;

fn h(character: char) -> String {
    crate::test_fixtures::fixture_public_key_hex(character)
}

fn top_event_tags() -> Vec<Vec<String>> {
    vec![
        vec!["p".to_string(), h('b')],
        vec!["q".to_string(), h('f')],
        vec!["K".to_string(), "30402".to_string()],
        vec!["P".to_string(), h('b')],
        vec!["k".to_string(), "30402".to_string()],
        vec!["E".to_string(), h('a'), String::new(), h('b')],
        vec!["e".to_string(), h('a'), String::new(), h('b')],
    ]
}

#[test]
fn projects_unordered_top_event_and_preserves_supplemental_tags() {
    let tags = top_event_tags();
    let projection =
        project_nip22_comment_parts(KIND_COMMENT, &tags, "Comment", 10).expect("projection");
    assert!(matches!(
        projection.position(),
        RadrootsInboundNip22CommentPosition::TopLevelEvent { reference }
            if reference.author_hint().is_some()
    ));
    assert_eq!(projection.raw_tags(), tags);
    assert!(projection.diagnostics().is_empty());
}

#[test]
fn projects_top_address_with_distinct_optional_relay_hints() {
    let coordinate = format!("31922:{}:market", h('b'));
    let tags = vec![
        vec![
            "A".to_string(),
            coordinate.clone(),
            "wss://root.example".to_string(),
        ],
        vec!["K".to_string(), "31922".to_string()],
        vec![
            "P".to_string(),
            h('b'),
            "wss://participant.example".to_string(),
        ],
        vec!["a".to_string(), coordinate],
        vec![
            "e".to_string(),
            h('e'),
            "wss://revision.example".to_string(),
        ],
        vec!["k".to_string(), "31922".to_string()],
        vec!["p".to_string(), h('b')],
    ];
    let projection =
        project_nip22_comment_parts(KIND_COMMENT, &tags, "Comment", 10).expect("projection");
    let RadrootsInboundNip22CommentPosition::TopLevelAddress {
        reference,
        current_revision,
    } = projection.position()
    else {
        panic!("top-level address");
    };
    assert_eq!(current_revision.event_id().as_str(), h('e'));
    assert!(reference.relay().is_none());
    assert_eq!(
        current_revision.relay().expect("relay").as_str(),
        "wss://revision.example"
    );
}

#[test]
fn direct_address_revision_cardinality_precedes_parent_coordinate_parsing() {
    let coordinate = format!("31922:{}:market", h('b'));
    let tags = vec![
        vec!["A".to_string(), coordinate, String::new()],
        vec!["K".to_string(), "31922".to_string()],
        vec!["P".to_string(), h('b')],
        vec![
            "a".to_string(),
            format!("031922:{}:market", h('b')),
            String::new(),
        ],
        vec!["k".to_string(), "31922".to_string()],
        vec!["p".to_string(), h('b')],
    ];

    assert_eq!(
        project_nip22_comment_parts(KIND_COMMENT, &tags, "Comment", 10).unwrap_err(),
        RadrootsNip22CommentProjectionError::RevisionMissing { actual: 0 }
    );
}

#[test]
fn nested_parent_uses_author_hint_and_keeps_mentions() {
    let mut tags = top_event_tags();
    tags[4][1] = KIND_COMMENT.to_string();
    tags[6] = vec![
        "e".to_string(),
        h('c'),
        "wss://parent.example".to_string(),
        h('d'),
    ];
    tags[0][1] = h('d');
    tags.push(vec!["p".to_string(), h('e')]);
    let projection =
        project_nip22_comment_parts(KIND_COMMENT, &tags, "Reply", 10).expect("projection");
    let RadrootsInboundNip22CommentPosition::Nested { parent } = projection.position() else {
        panic!("nested");
    };
    assert_eq!(parent.event_id().as_str(), h('c'));
    assert_eq!(parent.author().pubkey().to_hex(), h('d'));
    assert_eq!(projection.mentions().len(), 1);
    assert_eq!(projection.mentions()[0].pubkey().to_hex(), h('e'));
}

#[test]
fn missing_parent_hint_with_multiple_participants_is_ambiguous() {
    let mut tags = top_event_tags();
    tags[4][1] = KIND_COMMENT.to_string();
    tags[6] = vec!["e".to_string(), h('c')];
    tags.push(vec!["p".to_string(), h('e')]);
    assert_eq!(
        project_nip22_comment_parts(KIND_COMMENT, &tags, "Reply", 10).unwrap_err(),
        RadrootsNip22CommentProjectionError::ParentAuthorAmbiguous
    );
}

#[test]
fn diagnoses_same_tag_advisory_values_in_stable_order() {
    let mut tags = top_event_tags();
    tags[5] = vec![
        "E".to_string(),
        h('a'),
        "WSS://bad.example".to_string(),
        "not-an-author".to_string(),
    ];
    let raw_tag = tags[5].clone();
    let projection =
        project_nip22_comment_parts(KIND_COMMENT, &tags, "Comment", 10).expect("projection");
    assert_eq!(
        projection
            .diagnostics()
            .iter()
            .map(|diagnostic| (
                diagnostic.code(),
                diagnostic.tag_index(),
                diagnostic.raw_tag()
            ))
            .collect::<Vec<_>>(),
        vec![
            ("comment_root_relay_ignored", 5, raw_tag.as_slice()),
            ("comment_root_author_hint_ignored", 5, raw_tag.as_slice()),
        ]
    );
}

#[test]
fn hard_gates_mixed_duplicate_and_inconsistent_authority() {
    let mut tags = top_event_tags();
    tags.push(vec!["A".to_string(), format!("30402:{}:listing", h('b'))]);
    assert_eq!(
        project_nip22_comment_parts(KIND_COMMENT, &tags, "Comment", 10).unwrap_err(),
        RadrootsNip22CommentProjectionError::RootCardinality { actual: 2 }
    );

    let mut tags = top_event_tags();
    tags.push(vec!["K".to_string(), "30402".to_string()]);
    assert!(matches!(
        project_nip22_comment_parts(KIND_COMMENT, &tags, "Comment", 10),
        Err(RadrootsNip22CommentProjectionError::RootKindCardinality { actual: 2 })
    ));

    let mut tags = top_event_tags();
    tags[3][1] = h('c');
    assert!(matches!(
        project_nip22_comment_parts(KIND_COMMENT, &tags, "Comment", 10),
        Err(RadrootsNip22CommentProjectionError::RootAuthorMismatch { .. })
    ));
}

#[test]
fn rejects_reserved_external_root_and_parent_authority_forms() {
    let mut external_root = top_event_tags();
    external_root.push(vec![
        "I".to_string(),
        "https://example.com/root".to_string(),
    ]);
    assert!(matches!(
        project_nip22_comment_parts(KIND_COMMENT, &external_root, "Comment", 10),
        Err(RadrootsNip22CommentProjectionError::RootFormUnsupported { .. })
    ));

    let mut external_parent = top_event_tags();
    external_parent.push(vec![
        "i".to_string(),
        "https://example.com/parent".to_string(),
    ]);
    assert!(matches!(
        project_nip22_comment_parts(KIND_COMMENT, &external_parent, "Comment", 10),
        Err(RadrootsNip22CommentProjectionError::ParentFormUnsupported { .. })
    ));
}

#[test]
fn hard_gates_noncanonical_kind_tokens_and_valid_hint_mismatches() {
    for value in ["030402", "+30402"] {
        let mut tags = top_event_tags();
        tags[2][1] = value.to_string();
        assert!(matches!(
            project_nip22_comment_parts(KIND_COMMENT, &tags, "Comment", 10),
            Err(RadrootsNip22CommentProjectionError::RootKindUnsupported { .. })
        ));
    }

    let mut tags = top_event_tags();
    tags[5][3] = h('c');
    assert!(matches!(
        project_nip22_comment_parts(KIND_COMMENT, &tags, "Comment", 10),
        Err(RadrootsNip22CommentProjectionError::RootAuthorMismatch { tag_index: 5 })
    ));
}

#[test]
fn uses_unicode_white_space_for_blank_content() {
    let tags = top_event_tags();
    project_nip22_comment_parts(KIND_COMMENT, &tags, "\u{001c}", 10)
        .expect("U+001C is not Unicode White_Space");
    assert_eq!(
        project_nip22_comment_parts(KIND_COMMENT, &tags, "\u{00a0}", 10).unwrap_err(),
        RadrootsNip22CommentProjectionError::ContentMissing
    );
}

#[test]
fn enforces_inbound_structural_tag_limits_exactly() {
    let mut exact_tag_count = top_event_tags();
    exact_tag_count.extend(
        (exact_tag_count.len()..RADROOTS_NIP22_COMMENT_TAG_MAX_COUNT)
            .map(|_| vec!["x".to_string()]),
    );
    project_nip22_comment_parts(KIND_COMMENT, &exact_tag_count, "Comment", 10)
        .expect("exact tag-count limit");

    let mut overflow_tag_count = exact_tag_count;
    overflow_tag_count.push(vec!["x".to_string()]);
    assert_eq!(
        project_nip22_comment_parts(KIND_COMMENT, &overflow_tag_count, "Comment", 10).unwrap_err(),
        RadrootsNip22CommentProjectionError::TagCountExceeded {
            max: RADROOTS_NIP22_COMMENT_TAG_MAX_COUNT,
            actual: RADROOTS_NIP22_COMMENT_TAG_MAX_COUNT + 1,
        }
    );

    let mut exact_element_count = top_event_tags();
    let authority_element_count = exact_element_count.iter().map(Vec::len).sum::<usize>();
    let supplemental_element_count =
        RADROOTS_NIP22_COMMENT_TAG_TOTAL_ELEMENT_MAX_COUNT - authority_element_count;
    let supplemental_tag_count = RADROOTS_NIP22_COMMENT_TAG_MAX_COUNT - exact_element_count.len();
    let four_element_tags = supplemental_tag_count - 10;
    exact_element_count.extend((0..four_element_tags).map(|_| {
        vec![
            "x".to_string(),
            "x".to_string(),
            "x".to_string(),
            "x".to_string(),
        ]
    }));
    exact_element_count.extend((0..10).map(|_| {
        vec![
            "x".to_string(),
            "x".to_string(),
            "x".to_string(),
            "x".to_string(),
            "x".to_string(),
        ]
    }));
    assert_eq!(
        exact_element_count.iter().map(Vec::len).sum::<usize>(),
        supplemental_element_count + authority_element_count
    );
    project_nip22_comment_parts(KIND_COMMENT, &exact_element_count, "Comment", 10)
        .expect("exact total tag-element limit");

    let mut overflow_element_count = exact_element_count;
    overflow_element_count[0].push("x".to_string());
    assert_eq!(
        project_nip22_comment_parts(KIND_COMMENT, &overflow_element_count, "Comment", 10)
            .unwrap_err(),
        RadrootsNip22CommentProjectionError::TagElementCountExceeded {
            max: RADROOTS_NIP22_COMMENT_TAG_TOTAL_ELEMENT_MAX_COUNT,
            actual: RADROOTS_NIP22_COMMENT_TAG_TOTAL_ELEMENT_MAX_COUNT + 1,
        }
    );
}

#[test]
fn enforces_inbound_aggregate_tag_budget_exactly() {
    let mut tags = top_event_tags();
    let current = tags
        .iter()
        .flat_map(|tag| tag.iter())
        .map(String::len)
        .sum::<usize>();
    let remaining = RADROOTS_NIP22_COMMENT_TAG_TOTAL_MAX_BYTES - current;
    while tags
        .iter()
        .flat_map(|tag| tag.iter())
        .map(String::len)
        .sum::<usize>()
        + RADROOTS_NIP22_COMMENT_TAG_ELEMENT_MAX_BYTES
        <= RADROOTS_NIP22_COMMENT_TAG_TOTAL_MAX_BYTES
    {
        tags.push(vec![
            String::new(),
            "x".repeat(RADROOTS_NIP22_COMMENT_TAG_ELEMENT_MAX_BYTES),
        ]);
    }
    let now = tags
        .iter()
        .flat_map(|tag| tag.iter())
        .map(String::len)
        .sum::<usize>();
    tags.push(vec![String::new(), "x".repeat(remaining - (now - current))]);
    validate_tag_and_wire_budgets(&tags, "Comment", 10).expect("exact aggregate tag budget");
    tags.last_mut().expect("last tag")[1].push('x');
    assert!(matches!(
        validate_tag_and_wire_budgets(&tags, "Comment", 10),
        Err(RadrootsNip22CommentProjectionError::TagBytesExceeded { max, actual })
            if max == RADROOTS_NIP22_COMMENT_TAG_TOTAL_MAX_BYTES && actual == max + 1
    ));
}

#[test]
fn verified_projection_api_cannot_accept_an_unverified_envelope() {
    fn project(
        event: &RadrootsSignatureVerifiedEvent,
    ) -> Result<RadrootsInboundNip22CommentProjection, RadrootsNip22CommentProjectionError> {
        project_verified_nip22_comment_event(event)
    }
    let _ = project;
}
