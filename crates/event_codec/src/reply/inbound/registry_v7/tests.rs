use super::*;

fn h(character: char) -> String {
    crate::test_fixtures::fixture_public_key_hex(character)
}

#[test]
fn tolerant_inbound_accepts_blank_content_and_absent_participants() {
    let projection = project_nip10_reply_parts(
        KIND_POST,
        &[vec![
            "e".to_string(),
            h('a'),
            String::new(),
            "root".to_string(),
        ]],
        "",
        10,
    )
    .expect("optional participants and blank content do not erase a Reply");

    assert!(projection.is_direct());
    assert!(projection.participants().is_empty());
    assert_eq!(
        projection
            .diagnostics()
            .iter()
            .map(RadrootsNip10ReplyDiagnostic::code)
            .collect::<Vec<_>>(),
        vec!["reply_author_missing_ignored"]
    );
    assert_eq!(projection.diagnostics()[0].tag_index(), None);
    assert_eq!(projection.diagnostics()[0].raw_tag(), None);
}

#[test]
fn tolerant_inbound_retains_raw_optional_metadata_and_orders_diagnostics() {
    let root_id = h('a');
    let parent_id = h('d');
    let participant = h('b');
    let parent_author_hint = h('c');
    let tags = vec![
        vec![
            "e".to_string(),
            root_id.clone(),
            "https://relay.example".to_string(),
            "root".to_string(),
            "not-a-pubkey".to_string(),
        ],
        vec![
            "e".to_string(),
            parent_id.clone(),
            String::new(),
            "reply".to_string(),
            parent_author_hint,
        ],
        vec!["p".to_string()],
        vec!["p".to_string(), "not-a-pubkey".to_string()],
        vec![
            "p".to_string(),
            participant.clone(),
            "https://relay.example".to_string(),
        ],
        vec!["p".to_string(), participant.clone()],
    ];

    let projection = project_nip10_reply_parts(KIND_POST, &tags, "Reply", 10)
        .expect("malformed optional metadata must not erase a Reply");

    assert!(!projection.is_direct());
    assert_eq!(projection.root().tag_index(), 0);
    assert_eq!(projection.root().raw_tag(), tags[0]);
    assert_eq!(projection.root().event_id().to_hex(), root_id);
    assert!(projection.root().relay().is_none());
    assert!(projection.root().author_hint().is_none());
    let parent = projection.reply_reference().expect("reply reference");
    assert_eq!(parent.tag_index(), 1);
    assert_eq!(parent.raw_tag(), tags[1]);
    assert_eq!(parent.event_id().to_hex(), parent_id);
    assert_eq!(projection.participants().len(), 1);
    assert_eq!(projection.participants()[0].tag_index(), 4);
    assert_eq!(projection.participants()[0].raw_tag(), tags[4]);
    assert_eq!(projection.participants()[0].pubkey().to_hex(), participant);
    assert!(projection.participants()[0].relay().is_none());

    assert_eq!(
        projection
            .diagnostics()
            .iter()
            .map(RadrootsNip10ReplyDiagnostic::code)
            .collect::<Vec<_>>(),
        vec![
            "reply_reference_relay_ignored",
            "reply_reference_author_ignored",
            "reply_author_shape_ignored",
            "reply_author_invalid_ignored",
            "reply_author_relay_ignored",
            "reply_author_duplicate_ignored",
            "reply_author_mismatch_ignored",
        ]
    );
    assert_eq!(
        projection
            .diagnostics()
            .iter()
            .map(RadrootsNip10ReplyDiagnostic::tag_index)
            .collect::<Vec<_>>(),
        vec![
            Some(0),
            Some(0),
            Some(2),
            Some(3),
            Some(4),
            Some(5),
            Some(1),
        ]
    );
    for diagnostic in projection.diagnostics() {
        let Some(tag_index) = diagnostic.tag_index() else {
            continue;
        };
        assert_eq!(diagnostic.raw_tag().expect("source tag"), tags[tag_index]);
    }
}

#[test]
fn inbound_projection_uses_the_canonical_relay_hint_profile() {
    let root_id = h('a');
    let root_author = h('b');
    for relay in [
        "wss://%65xample.com",
        "wss://127.1",
        "wss://relay.example:01",
        "wss://[2001:0db8::1]",
        "wss://relay.example/%2f",
    ] {
        let root_tag = vec![
            "e".to_string(),
            root_id.clone(),
            relay.to_string(),
            "root".to_string(),
        ];
        let tags = vec![root_tag.clone(), vec!["p".to_string(), root_author.clone()]];
        let projection = project_nip10_reply_parts(KIND_POST, &tags, "Reply", 10)
            .unwrap_or_else(|error| panic!("{relay} must remain advisory: {error}"));

        assert!(projection.root().relay().is_none(), "{relay}");
        assert_eq!(projection.root().raw_tag(), root_tag);
        assert_eq!(
            projection
                .diagnostics()
                .iter()
                .map(RadrootsNip10ReplyDiagnostic::code)
                .collect::<Vec<_>>(),
            vec!["reply_reference_relay_ignored"],
            "{relay}"
        );
        assert_eq!(
            projection.diagnostics()[0].raw_tag(),
            Some(root_tag.as_slice()),
            "{relay}"
        );
    }

    let tags = vec![
        vec![
            "e".to_string(),
            root_id,
            "wss://[2001:db8::1]:65535/nostr?region=ca-bc".to_string(),
            "root".to_string(),
        ],
        vec![
            "p".to_string(),
            root_author,
            "ws://127.0.0.1:21003".to_string(),
        ],
    ];
    let projection = project_nip10_reply_parts(KIND_POST, &tags, "Reply", 10)
        .expect("canonical reference and participant relays");
    assert_eq!(
        projection.root().relay().expect("root relay").as_str(),
        "wss://[2001:db8::1]:65535/nostr?region=ca-bc"
    );
    assert_eq!(
        projection.participants()[0]
            .relay()
            .expect("participant relay")
            .as_str(),
        "ws://127.0.0.1:21003"
    );
    assert!(projection.diagnostics().is_empty());
}

#[test]
fn inbound_relay_syntax_and_tag_element_budgets_remain_separate() {
    let prefix = "wss://relay.example/";
    let relay = format!(
        "{prefix}{}",
        "a".repeat(RADROOTS_POST_TAG_ELEMENT_MAX_BYTES + 1 - prefix.len())
    );
    assert_eq!(relay.len(), RADROOTS_POST_TAG_ELEMENT_MAX_BYTES + 1);
    NostrRelayHint::parse(&relay).expect("relay syntax has no Reply wire budget");

    let tags = vec![
        vec!["e".to_string(), h('a'), relay, "root".to_string()],
        vec!["p".to_string(), h('b')],
    ];
    assert!(matches!(
        project_nip10_reply_parts(KIND_POST, &tags, "Reply", 10),
        Err(RadrootsNip10ReplyProjectionError::TagElementTooLarge {
            max: RADROOTS_POST_TAG_ELEMENT_MAX_BYTES,
            actual,
            tag_index: 0,
            element_index: 2,
        }) if actual == RADROOTS_POST_TAG_ELEMENT_MAX_BYTES + 1
    ));
}

#[test]
fn tolerant_inbound_keeps_reference_ids_and_markers_as_hard_gates() {
    let author = h('b');
    for (tag, expected) in [
        (
            vec![
                "e".to_string(),
                "not-an-event-id".to_string(),
                String::new(),
                "root".to_string(),
            ],
            "reply_event_id_invalid",
        ),
        (
            vec![
                "e".to_string(),
                h('a'),
                String::new(),
                "mention".to_string(),
            ],
            "reply_marker_missing",
        ),
    ] {
        let error = project_nip10_reply_parts(
            KIND_POST,
            &[tag, vec!["p".to_string(), author.clone()]],
            "Reply",
            10,
        )
        .unwrap_err();
        assert_eq!(error.code(), expected);
    }
}

#[test]
fn marked_inbound_retains_citations_and_ignores_malformed_supplements() {
    let root_id = h('a');
    let citation_id = h('c');
    let tags = vec![
        vec!["e".to_string()],
        vec![
            "e".to_string(),
            citation_id.clone(),
            "wss://relay.example".to_string(),
            String::new(),
            h('b'),
        ],
        vec!["e".to_string(), "not-an-event-id".to_string()],
        vec![
            "e".to_string(),
            h('d'),
            String::new(),
            "mention".to_string(),
        ],
        vec![
            "e".to_string(),
            root_id.clone(),
            String::new(),
            "root".to_string(),
        ],
    ];

    let projection = project_nip10_reply_parts(KIND_POST, &tags, "Reply", 10)
        .expect("supplemental references must not erase a marked Reply");

    assert_eq!(projection.root().event_id().to_hex(), root_id);
    assert_eq!(projection.citations().len(), 1);
    assert_eq!(projection.citations()[0].tag_index(), 1);
    assert_eq!(projection.citations()[0].raw_tag(), tags[1]);
    assert_eq!(projection.citations()[0].event_id().to_hex(), citation_id);
    assert_eq!(
        projection.citations()[0]
            .relay()
            .expect("citation relay")
            .as_str(),
        "wss://relay.example"
    );
    assert_eq!(
        projection.citations()[0]
            .author_hint()
            .expect("citation author")
            .to_hex(),
        h('b')
    );
    assert_eq!(
        projection
            .diagnostics()
            .iter()
            .map(RadrootsNip10ReplyDiagnostic::code)
            .collect::<Vec<_>>(),
        vec![
            "reply_citation_shape_ignored",
            "reply_citation_event_id_ignored",
            "reply_citation_marker_ignored",
            "reply_author_missing_ignored",
            "reply_author_mismatch_ignored",
        ]
    );
    assert_eq!(
        projection
            .diagnostics()
            .iter()
            .map(RadrootsNip10ReplyDiagnostic::tag_index)
            .collect::<Vec<_>>(),
        vec![Some(0), Some(2), Some(3), None, Some(1)]
    );
}

#[test]
fn positional_inbound_accepts_empty_markers_and_tolerates_middle_citations() {
    let root_id = h('a');
    let root_author = h('b');
    let direct_tags = vec![
        vec![
            "e".to_string(),
            root_id.clone(),
            "wss://root.relay.example".to_string(),
            String::new(),
            root_author.clone(),
        ],
        vec!["p".to_string(), root_author.clone()],
    ];
    let direct = project_nip10_reply_parts(KIND_POST, &direct_tags, "Direct", 10)
        .expect("empty marker and fifth author are valid positional input");
    assert_eq!(direct.style(), RadrootsNip10ReplyStyle::LegacyPositional);
    assert!(direct.is_direct());
    assert_eq!(
        direct
            .root()
            .author_hint()
            .expect("root author hint")
            .to_hex(),
        root_author
    );
    assert!(direct.diagnostics().is_empty());

    let parent_id = h('c');
    let parent_author = h('d');
    let citation_id = h('e');
    let citation_author = h('f');
    let nested_tags = vec![
        vec![
            "e".to_string(),
            root_id,
            String::new(),
            String::new(),
            root_author.clone(),
        ],
        vec![
            "e".to_string(),
            "1".repeat(64),
            String::new(),
            "mention".to_string(),
        ],
        vec![
            "e".to_string(),
            citation_id.clone(),
            String::new(),
            String::new(),
            citation_author.clone(),
        ],
        vec![
            "e".to_string(),
            parent_id.clone(),
            "https://parent.relay.example".to_string(),
            String::new(),
            parent_author.clone(),
        ],
        vec!["p".to_string(), root_author],
        vec!["p".to_string(), parent_author],
        vec!["p".to_string(), citation_author],
    ];
    let nested = project_nip10_reply_parts(KIND_POST, &nested_tags, "Nested", 10)
        .expect("malformed middle citations must not erase positional anchors");
    assert_eq!(nested.style(), RadrootsNip10ReplyStyle::LegacyPositional);
    assert_eq!(nested.parent().event_id().to_hex(), parent_id);
    assert_eq!(nested.citations().len(), 1);
    assert_eq!(nested.citations()[0].tag_index(), 2);
    assert_eq!(nested.citations()[0].event_id().to_hex(), citation_id);
    assert_eq!(
        nested
            .diagnostics()
            .iter()
            .map(RadrootsNip10ReplyDiagnostic::code)
            .collect::<Vec<_>>(),
        vec![
            "reply_citation_marker_ignored",
            "reply_reference_relay_ignored",
        ]
    );
    assert_eq!(nested.diagnostics()[0].tag_index(), Some(1));
    assert_eq!(
        nested.diagnostics()[0].raw_tag(),
        Some(nested_tags[1].as_slice())
    );
    assert_eq!(nested.diagnostics()[1].tag_index(), Some(3));
    assert_eq!(
        nested.diagnostics()[1].raw_tag(),
        Some(nested_tags[3].as_slice())
    );
}
