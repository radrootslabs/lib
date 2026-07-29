use super::*;

fn h(character: char) -> String {
    crate::test_fixtures::fixture_public_key_hex(character)
}

fn valid_event_tags() -> Vec<Vec<String>> {
    vec![
        vec!["e".to_string(), h('a')],
        vec!["k".to_string(), "1".to_string()],
    ]
}

#[test]
fn projects_raw_mixed_duplicates_trailing_and_unknown_tags() {
    let coordinate_b = format!("30402:{}:produce", h('b'));
    let coordinate_c = format!("31923:{}:market", h('c'));
    let tags = vec![
        vec!["x".to_string(), "unknown".to_string()],
        vec!["e".to_string(), h('f'), "relay".to_string()],
        vec![
            "a".to_string(),
            coordinate_c.clone(),
            "trailing".to_string(),
        ],
        vec!["e".to_string(), h('a')],
        vec!["e".to_string(), h('F'), "duplicate".to_string()],
        vec!["a".to_string(), coordinate_b.clone()],
        vec![
            "a".to_string(),
            format!("030402:{}:produce", h('B')),
            "duplicate".to_string(),
        ],
        vec!["k".to_string(), "31923".to_string(), "trailing".to_string()],
        vec!["k".to_string(), "1".to_string()],
    ];
    let projection = project_nip09_deletion_request_parts(KIND_DELETION_REQUEST, &tags, "", 1)
        .expect("projection");

    assert_eq!(projection.raw_tags(), tags);
    assert_eq!(
        projection
            .event_targets()
            .iter()
            .map(|target| (target.event_id().to_hex(), target.tag_index()))
            .collect::<Vec<_>>(),
        vec![(h('a'), 3), (h('f'), 1)]
    );
    assert_eq!(
        projection
            .address_targets()
            .iter()
            .map(|target| (target.coordinate().as_str(), target.tag_index()))
            .collect::<Vec<_>>(),
        vec![(coordinate_b.as_str(), 5), (coordinate_c.as_str(), 2)]
    );
    assert_eq!(
        projection
            .kind_advisories()
            .iter()
            .map(|advisory| (advisory.kind(), advisory.tag_index()))
            .collect::<Vec<_>>(),
        vec![(1, 8), (31_923, 7)]
    );
    assert_eq!(projection.event_targets()[1].raw_tag(), tags[1]);
    assert_eq!(projection.address_targets()[1].raw_tag(), tags[2]);
    assert!(projection.diagnostics().is_empty());
}

#[test]
fn emits_advisory_diagnostics_in_source_order() {
    let coordinate = format!("30402:{}:produce", h('b'));
    let tags = vec![
        vec!["k".to_string()],
        vec!["a".to_string(), coordinate],
        vec!["k".to_string(), "+30402".to_string()],
        vec!["k".to_string(), "30402".to_string()],
        vec![
            "k".to_string(),
            "30402".to_string(),
            "duplicate".to_string(),
        ],
        vec!["k".to_string(), "31923".to_string()],
        vec!["k".to_string(), "65536".to_string()],
    ];
    let projection = project_nip09_deletion_request_parts(KIND_DELETION_REQUEST, &tags, "", 1)
        .expect("projection");

    assert_eq!(
        projection
            .diagnostics()
            .iter()
            .map(|diagnostic| (diagnostic.code(), diagnostic.tag_index()))
            .collect::<Vec<_>>(),
        vec![
            ("deletion_kind_advisory_shape_ignored", 0),
            ("deletion_kind_advisory_invalid_ignored", 2),
            ("deletion_kind_advisory_duplicate_ignored", 4),
            ("deletion_kind_advisory_conflict_ignored", 5),
            ("deletion_kind_advisory_invalid_ignored", 6),
        ]
    );
    assert_eq!(
        projection
            .kind_advisories()
            .iter()
            .map(RadrootsInboundNip09DeletionKindAdvisory::kind)
            .collect::<Vec<_>>(),
        vec![30_402, 31_923]
    );
    for diagnostic in projection.diagnostics() {
        assert_eq!(diagnostic.raw_tag(), tags[diagnostic.tag_index()]);
    }
}

#[test]
fn event_target_prevents_unprovable_kind_conflict() {
    let tags = vec![
        vec!["a".to_string(), format!("30402:{}:produce", h('b'))],
        vec!["e".to_string(), h('a')],
        vec!["k".to_string(), "31923".to_string()],
    ];
    let projection = project_nip09_deletion_request_parts(KIND_DELETION_REQUEST, &tags, "", 1)
        .expect("mixed projection");
    assert_eq!(projection.kind_advisories()[0].kind(), 31_923);
    assert!(projection.diagnostics().is_empty());
}

#[test]
fn accepts_empty_content_and_trailing_target_fields() {
    let tags = vec![
        vec!["e".to_string(), h('a'), String::new(), "extra".to_string()],
        vec![
            "a".to_string(),
            format!("30000:{}:", h('b')),
            "extra".to_string(),
        ],
    ];
    project_nip09_deletion_request_parts(KIND_DELETION_REQUEST, &tags, "", 1)
        .expect("trailing fields");
}

#[test]
fn first_malformed_target_in_source_order_is_a_hard_error() {
    let tags = vec![
        vec!["a".to_string(), format!("30000:{}:", h('b'))],
        vec!["e".to_string()],
        vec!["a".to_string(), "bad".to_string()],
    ];
    assert_eq!(
        project_nip09_deletion_request_parts(KIND_DELETION_REQUEST, &tags, "", 1).unwrap_err(),
        RadrootsNip09DeletionProjectionError::EventTargetShape { tag_index: 1 }
    );

    let tags = vec![
        vec!["e".to_string(), h('a')],
        vec!["a".to_string(), "bad".to_string()],
        vec!["e".to_string(), "bad".to_string()],
    ];
    assert!(matches!(
        project_nip09_deletion_request_parts(KIND_DELETION_REQUEST, &tags, "", 1),
        Err(RadrootsNip09DeletionProjectionError::AddressTargetInvalid { tag_index: 1, .. })
    ));
}

#[test]
fn missing_target_union_follows_target_shape_and_validity() {
    assert_eq!(
        project_nip09_deletion_request_parts(
            KIND_DELETION_REQUEST,
            &[vec!["x".to_string()], vec!["k".to_string()]],
            "",
            1,
        )
        .unwrap_err(),
        RadrootsNip09DeletionProjectionError::TargetMissing
    );
    assert_eq!(
        project_nip09_deletion_request_parts(
            KIND_DELETION_REQUEST,
            &[vec!["e".to_string()]],
            "",
            1,
        )
        .unwrap_err(),
        RadrootsNip09DeletionProjectionError::EventTargetShape { tag_index: 0 }
    );
}

#[test]
fn enforces_exact_error_precedence_before_target_semantics() {
    let oversized_content = "x".repeat(RADROOTS_NIP09_DELETION_CONTENT_MAX_BYTES + 1);
    let oversized_tags = vec![vec!["e".to_string()]; RADROOTS_NIP09_DELETION_TAG_MAX_COUNT + 1];
    assert!(matches!(
        project_nip09_deletion_request_parts(1, &oversized_tags, &oversized_content, 1),
        Err(RadrootsNip09DeletionProjectionError::UnsupportedKind { actual: 1 })
    ));
    assert!(matches!(
        project_nip09_deletion_request_parts(
            KIND_DELETION_REQUEST,
            &oversized_tags,
            &oversized_content,
            1
        ),
        Err(RadrootsNip09DeletionProjectionError::ContentTooLarge { .. })
    ));
    assert!(matches!(
        project_nip09_deletion_request_parts(KIND_DELETION_REQUEST, &oversized_tags, "", 1),
        Err(RadrootsNip09DeletionProjectionError::TagCountExceeded { .. })
    ));

    let too_many_elements = vec![
        vec!["x".to_string(); RADROOTS_NIP09_DELETION_TAG_TOTAL_ELEMENT_MAX_COUNT],
        vec!["e".to_string()],
    ];
    assert_eq!(
        project_nip09_deletion_request_parts(KIND_DELETION_REQUEST, &too_many_elements, "", 1)
            .unwrap_err(),
        RadrootsNip09DeletionProjectionError::TagElementCountExceeded {
            max: RADROOTS_NIP09_DELETION_TAG_TOTAL_ELEMENT_MAX_COUNT,
            actual: RADROOTS_NIP09_DELETION_TAG_TOTAL_ELEMENT_MAX_COUNT + 1,
        }
    );

    let oversized_element = vec![
        vec![
            "x".to_string(),
            "x".repeat(RADROOTS_NIP09_DELETION_TAG_ELEMENT_MAX_BYTES + 1),
        ],
        vec!["e".to_string()],
    ];
    assert_eq!(
        project_nip09_deletion_request_parts(KIND_DELETION_REQUEST, &oversized_element, "", 1)
            .unwrap_err(),
        RadrootsNip09DeletionProjectionError::TagElementTooLarge {
            max: RADROOTS_NIP09_DELETION_TAG_ELEMENT_MAX_BYTES,
            actual: RADROOTS_NIP09_DELETION_TAG_ELEMENT_MAX_BYTES + 1,
            tag_index: 0,
            element_index: 1,
        }
    );
}

#[test]
fn accepts_exact_shared_resource_boundaries() {
    let mut tag_count = valid_event_tags();
    tag_count.extend(
        (tag_count.len()..RADROOTS_NIP09_DELETION_TAG_MAX_COUNT).map(|_| vec!["x".to_string()]),
    );
    project_nip09_deletion_request_parts(KIND_DELETION_REQUEST, &tag_count, "", 1)
        .expect("exact tag-count boundary");

    let mut element_count = valid_event_tags();
    element_count.push(vec!["x".to_string(); 8]);
    element_count.extend(
        (element_count.len()..RADROOTS_NIP09_DELETION_TAG_MAX_COUNT)
            .map(|_| vec!["x".to_string(); 4]),
    );
    assert_eq!(
        element_count.iter().map(Vec::len).sum::<usize>(),
        RADROOTS_NIP09_DELETION_TAG_TOTAL_ELEMENT_MAX_COUNT
    );
    project_nip09_deletion_request_parts(KIND_DELETION_REQUEST, &element_count, "", 1)
        .expect("exact tag-element boundary");

    let exact_element = vec![
        vec!["e".to_string(), h('a')],
        vec![
            "x".to_string(),
            "x".repeat(RADROOTS_NIP09_DELETION_TAG_ELEMENT_MAX_BYTES),
        ],
    ];
    project_nip09_deletion_request_parts(KIND_DELETION_REQUEST, &exact_element, "", 1)
        .expect("exact individual tag-element boundary");

    let mut tag_bytes = valid_event_tags();
    tag_bytes.extend((0..31).map(|_| {
        vec![
            String::new(),
            "x".repeat(RADROOTS_NIP09_DELETION_TAG_ELEMENT_MAX_BYTES),
        ]
    }));
    tag_bytes.push(vec![String::new(), "x".repeat(4_029)]);
    assert_eq!(
        tag_bytes
            .iter()
            .flat_map(|tag| tag.iter())
            .map(String::len)
            .sum::<usize>(),
        RADROOTS_NIP09_DELETION_TAG_TOTAL_MAX_BYTES
    );
    project_nip09_deletion_request_parts(KIND_DELETION_REQUEST, &tag_bytes, "", 1)
        .expect("exact aggregate tag-byte boundary");

    project_nip09_deletion_request_parts(
        KIND_DELETION_REQUEST,
        &valid_event_tags(),
        &"x".repeat(RADROOTS_NIP09_DELETION_CONTENT_MAX_BYTES),
        1,
    )
    .expect("exact content boundary");
}

#[test]
fn enforces_aggregate_tag_and_compact_wire_budgets_before_targets() {
    let mut tag_bytes = vec![
        vec![
            String::new(),
            "x".repeat(RADROOTS_NIP09_DELETION_TAG_ELEMENT_MAX_BYTES),
        ];
        RADROOTS_NIP09_DELETION_TAG_TOTAL_MAX_BYTES
            / RADROOTS_NIP09_DELETION_TAG_ELEMENT_MAX_BYTES
    ];
    tag_bytes.push(vec!["e".to_string()]);
    assert_eq!(
        project_nip09_deletion_request_parts(KIND_DELETION_REQUEST, &tag_bytes, "", 1).unwrap_err(),
        RadrootsNip09DeletionProjectionError::TagBytesExceeded {
            max: RADROOTS_NIP09_DELETION_TAG_TOTAL_MAX_BYTES,
            actual: RADROOTS_NIP09_DELETION_TAG_TOTAL_MAX_BYTES + 1,
        }
    );

    let escaped_content = "\u{0001}".repeat(50_000);
    assert!(matches!(
        project_nip09_deletion_request_parts(
            KIND_DELETION_REQUEST,
            &[vec!["e".to_string()]],
            &escaped_content,
            1
        ),
        Err(RadrootsNip09DeletionProjectionError::EventWireTooLarge { .. })
    ));
}

#[test]
fn error_codes_and_messages_are_stable() {
    let errors = [
        RadrootsNip09DeletionProjectionError::UnsupportedKind { actual: 1 },
        RadrootsNip09DeletionProjectionError::ContentTooLarge { max: 1, actual: 2 },
        RadrootsNip09DeletionProjectionError::TagCountExceeded { max: 1, actual: 2 },
        RadrootsNip09DeletionProjectionError::TagElementCountExceeded { max: 1, actual: 2 },
        RadrootsNip09DeletionProjectionError::TagElementTooLarge {
            max: 1,
            actual: 2,
            tag_index: 3,
            element_index: 4,
        },
        RadrootsNip09DeletionProjectionError::TagBytesExceeded { max: 1, actual: 2 },
        RadrootsNip09DeletionProjectionError::EventWireTooLarge { max: 1, actual: 2 },
        RadrootsNip09DeletionProjectionError::EventTargetShape { tag_index: 0 },
        RadrootsNip09DeletionProjectionError::EventTargetInvalid {
            tag_index: 0,
            error: ParseError::InvalidFormat,
        },
        RadrootsNip09DeletionProjectionError::AddressTargetShape { tag_index: 0 },
        RadrootsNip09DeletionProjectionError::AddressTargetInvalid {
            tag_index: 0,
            error: Nip01CoordinateParseError::InvalidFormat,
        },
        RadrootsNip09DeletionProjectionError::TargetMissing,
    ];
    let expected = [
        "unsupported_kind",
        "deletion_content_too_large",
        "deletion_tag_count_exceeded",
        "deletion_tag_element_count_exceeded",
        "deletion_tag_element_too_large",
        "deletion_tag_bytes_exceeded",
        "deletion_event_wire_too_large",
        "deletion_event_target_shape",
        "deletion_event_target_invalid",
        "deletion_address_target_shape",
        "deletion_address_target_invalid",
        "deletion_target_missing",
    ];
    for (error, expected) in errors.into_iter().zip(expected) {
        assert_eq!(error.code(), expected);
        assert!(!error.to_string().is_empty());
    }
}

#[test]
fn verified_projection_api_cannot_accept_an_unverified_envelope() {
    fn project(
        event: &RadrootsSignatureVerifiedEvent,
    ) -> Result<RadrootsInboundNip09DeletionProjection, RadrootsNip09DeletionProjectionError> {
        project_verified_nip09_deletion_request_event(event)
    }
    let _ = project;
}

#[test]
fn wire_estimator_uses_actual_created_at_width() {
    let tags = valid_event_tags();
    let mut content = "\u{0001}".repeat(43_600);
    while project_nip09_deletion_request_parts(KIND_DELETION_REQUEST, &tags, &content, u64::MAX)
        .is_ok()
    {
        content.push('\u{0001}');
    }
    assert!(matches!(
        project_nip09_deletion_request_parts(KIND_DELETION_REQUEST, &tags, &content, u64::MAX),
        Err(RadrootsNip09DeletionProjectionError::EventWireTooLarge { .. })
    ));
    project_nip09_deletion_request_parts(KIND_DELETION_REQUEST, &tags, &content, 1)
        .expect("short created_at width remains within the wire budget");
}
