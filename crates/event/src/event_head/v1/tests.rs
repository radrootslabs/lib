use super::*;
use crate::contract::RadrootsContractMatchError;
use crate::envelope::RadrootsEventEnvelopeParts;
use crate::envelope::kind::{
    KIND_FOLLOW, KIND_LIST_SET_GENERIC, KIND_POST, KIND_PROFILE, KIND_TRADE_PROPOSAL,
};
use crate::id::parse_public_key;

fn hex_64(character: char) -> String {
    crate::test_valid_hex_64(character)
}

fn hex_128(character: char) -> String {
    core::iter::repeat_n(character, 128).collect()
}

fn event(
    kind: u32,
    id: &str,
    author: &str,
    created_at: u64,
    tags: Vec<Vec<String>>,
) -> RadrootsEventEnvelope {
    RadrootsEventEnvelope::new(RadrootsEventEnvelopeParts {
        id: id.to_string(),
        author: author.to_string(),
        created_at,
        kind,
        tags,
        content: String::new(),
        sig: hex_128('f'),
    })
    .expect("event envelope")
}

fn event_with_content(
    kind: u32,
    id: &str,
    author: &str,
    created_at: u64,
    tags: Vec<Vec<String>>,
    content: &str,
) -> RadrootsEventEnvelope {
    RadrootsEventEnvelope::new(RadrootsEventEnvelopeParts {
        id: id.to_string(),
        author: author.to_string(),
        created_at,
        kind,
        tags,
        content: content.to_string(),
        sig: hex_128('f'),
    })
    .expect("event envelope")
}

fn candidate(id: char, created_at: u64) -> RadrootsEventHeadCandidate {
    expect_candidate(event_head_candidate_for_class(
        &event(10002, &hex_64(id), &hex_64('a'), created_at, Vec::new()),
        RadrootsEventClass::Replaceable,
    ))
}

fn expect_candidate(result: RadrootsEventHeadCandidateResult) -> RadrootsEventHeadCandidate {
    match result {
        RadrootsEventHeadCandidateResult::Candidate(candidate) => candidate,
        other => panic!("expected candidate: {other:?}"),
    }
}

#[test]
fn regular_and_ephemeral_events_do_not_create_heads() {
    let event = event(1, &hex_64('1'), &hex_64('a'), 1, Vec::new());
    assert_eq!(
        event_head_candidate_for_class(&event, RadrootsEventClass::Regular),
        RadrootsEventHeadCandidateResult::NotHeadSelected
    );
    assert_eq!(
        event_head_candidate_for_class(&event, RadrootsEventClass::Ephemeral),
        RadrootsEventHeadCandidateResult::NotPersisted
    );
}

#[test]
fn replaceable_events_use_kind_and_pubkey_coordinates() {
    let event = event(10002, &hex_64('1'), &hex_64('a'), 5, Vec::new());
    let candidate = expect_candidate(event_head_candidate_for_class(
        &event,
        RadrootsEventClass::Replaceable,
    ));
    assert_eq!(
        candidate.coordinate,
        RadrootsEventHeadCoordinate::Replaceable {
            kind: 10002,
            pubkey: parse_public_key(hex_64('a')).unwrap()
        }
    );
    assert_eq!(candidate.created_at, 5);
}

#[test]
fn addressable_events_use_kind_pubkey_and_d_tag_coordinates() {
    let event = event(
        30023,
        &hex_64('2'),
        &hex_64('b'),
        7,
        vec![vec![TAG_D.to_string(), "article-1".to_string()]],
    );
    let candidate = expect_candidate(event_head_candidate_for_class(
        &event,
        RadrootsEventClass::Addressable,
    ));
    assert_eq!(
        candidate.coordinate,
        RadrootsEventHeadCoordinate::Addressable {
            kind: 30023,
            pubkey: parse_public_key(hex_64('b')).unwrap(),
            d_tag: "article-1".to_owned()
        }
    );
}

#[test]
fn addressable_events_require_valid_d_tags() {
    let missing = event(30023, &hex_64('2'), &hex_64('b'), 7, Vec::new());
    assert_eq!(
        event_head_candidate_for_class(&missing, RadrootsEventClass::Addressable),
        RadrootsEventHeadCandidateResult::Malformed(RadrootsEventHeadMalformed::MissingDTag)
    );

    let invalid = event(
        30023,
        &hex_64('2'),
        &hex_64('b'),
        7,
        vec![vec![TAG_D.to_string(), "bad d".to_string()]],
    );
    assert!(matches!(
        event_head_candidate_for_class(&invalid, RadrootsEventClass::Addressable),
        RadrootsEventHeadCandidateResult::Malformed(RadrootsEventHeadMalformed::InvalidDTag(_))
    ));
}

#[test]
fn event_head_selection_uses_nip01_time_and_lowest_id_rules() {
    let current: RadrootsCurrentEventHead = candidate('3', 10).into();

    assert!(matches!(
        select_event_head(candidate('1', 1), None),
        RadrootsEventHeadDecision::Applied(_)
    ));
    assert!(matches!(
        select_event_head(candidate('4', 11), Some(&current)),
        RadrootsEventHeadDecision::Applied(_)
    ));
    assert_eq!(
        select_event_head(candidate('2', 9), Some(&current)),
        RadrootsEventHeadDecision::SkippedOlder
    );
    assert_eq!(
        select_event_head(candidate('3', 10), Some(&current)),
        RadrootsEventHeadDecision::SkippedDuplicate
    );
    assert!(matches!(
        select_event_head(candidate('2', 10), Some(&current)),
        RadrootsEventHeadDecision::Applied(_)
    ));
    assert_eq!(
        select_event_head(candidate('4', 10), Some(&current)),
        RadrootsEventHeadDecision::SkippedSameTimestampHigherEventId
    );
}

#[test]
fn event_head_selection_rejects_coordinate_mismatch() {
    let current: RadrootsCurrentEventHead = candidate('3', 10).into();
    let other = event_head_candidate_for_class(
        &event(
            30023,
            &hex_64('2'),
            &hex_64('a'),
            11,
            vec![vec![TAG_D.to_string(), "article".to_string()]],
        ),
        RadrootsEventClass::Addressable,
    );
    let other = expect_candidate(other);
    assert_eq!(
        select_event_head(other, Some(&current)),
        RadrootsEventHeadDecision::CoordinateMismatch
    );
}

#[test]
fn contract_bridge_uses_replaceable_event_classes() {
    let event = event(KIND_FOLLOW, &hex_64('1'), &hex_64('a'), 1, Vec::new());
    let candidate = expect_candidate(event_head_candidate_for_event(&event).expect("contract"));
    assert_eq!(
        candidate.coordinate,
        RadrootsEventHeadCoordinate::Replaceable {
            kind: KIND_FOLLOW,
            pubkey: parse_public_key(hex_64('a')).unwrap()
        }
    );
}

#[test]
fn raw_nip01_bridge_uses_numeric_kind_classes_without_contract_identification() {
    let replaceable = event(19_999, &hex_64('1'), &hex_64('a'), 1, Vec::new());
    let replaceable = expect_candidate(event_head_candidate_for_nip01_event(&replaceable));
    assert_eq!(
        replaceable.coordinate,
        RadrootsEventHeadCoordinate::Replaceable {
            kind: 19_999,
            pubkey: parse_public_key(hex_64('a')).unwrap(),
        }
    );

    let addressable = event(
        39_999,
        &hex_64('2'),
        &hex_64('b'),
        2,
        vec![vec![TAG_D.to_string(), "unsupported".to_string()]],
    );
    let addressable = expect_candidate(event_head_candidate_for_nip01_event(&addressable));
    assert_eq!(
        addressable.coordinate,
        RadrootsEventHeadCoordinate::Addressable {
            kind: 39_999,
            pubkey: parse_public_key(hex_64('b')).unwrap(),
            d_tag: "unsupported".to_owned(),
        }
    );

    let regular = event(40_000, &hex_64('3'), &hex_64('c'), 3, Vec::new());
    assert_eq!(
        event_head_candidate_for_nip01_event(&regular),
        RadrootsEventHeadCandidateResult::NotHeadSelected
    );
}

#[test]
fn raw_nip01_addressable_coordinates_treat_d_as_opaque_protocol_data() {
    for (tags, expected) in [
        (Vec::new(), ""),
        (vec![vec![TAG_D.to_owned(), String::new()]], ""),
        (
            vec![
                vec![TAG_D.to_owned()],
                vec![TAG_D.to_owned(), "ignored".to_owned()],
            ],
            "",
        ),
        (
            vec![vec![TAG_D.to_owned(), "not a product d".to_owned()]],
            "not a product d",
        ),
        (
            vec![vec![TAG_D.to_owned(), "line\nbreak".to_owned()]],
            "line\nbreak",
        ),
        (
            vec![
                vec![TAG_D.to_owned(), "first value".to_owned()],
                vec![TAG_D.to_owned(), "second-value".to_owned()],
            ],
            "first value",
        ),
    ] {
        let event = event(39_999, &hex_64('2'), &hex_64('b'), 2, tags);
        let candidate = expect_candidate(event_head_candidate_for_nip01_event(&event));
        assert_eq!(
            candidate.coordinate,
            RadrootsEventHeadCoordinate::Addressable {
                kind: 39_999,
                pubkey: parse_public_key(hex_64('b')).unwrap(),
                d_tag: expected.to_owned(),
            }
        );
    }
}

#[test]
fn product_addressable_coordinates_retain_strict_d_validation() {
    for tags in [
        Vec::new(),
        vec![vec![TAG_D.to_owned(), String::new()]],
        vec![vec![TAG_D.to_owned(), "not a product d".to_owned()]],
    ] {
        let event = event(30_023, &hex_64('2'), &hex_64('b'), 2, tags);
        assert!(matches!(
            event_head_candidate_for_class(&event, RadrootsEventClass::Addressable),
            RadrootsEventHeadCandidateResult::Malformed(_)
        ));
    }
}

#[test]
fn contract_bridge_uses_addressable_event_classes() {
    let event = event(
        KIND_LIST_SET_GENERIC,
        &hex_64('2'),
        &hex_64('b'),
        1,
        vec![vec![TAG_D.to_string(), "member_of.farms".to_string()]],
    );
    let candidate = expect_candidate(event_head_candidate_for_event(&event).expect("contract"));
    assert_eq!(
        candidate.coordinate,
        RadrootsEventHeadCoordinate::Addressable {
            kind: KIND_LIST_SET_GENERIC,
            pubkey: parse_public_key(hex_64('b')).unwrap(),
            d_tag: "member_of.farms".to_owned()
        }
    );
}

#[test]
fn contract_bridge_uses_profile_replaceable_heads() {
    let profile = event_with_content(
        KIND_PROFILE,
        &hex_64('3'),
        &hex_64('c'),
        1,
        Vec::new(),
        r#"{"name":"Alice"}"#,
    );
    let candidate =
        expect_candidate(event_head_candidate_for_event(&profile).expect("profile contract"));
    assert_eq!(
        candidate.coordinate,
        RadrootsEventHeadCoordinate::Replaceable {
            kind: KIND_PROFILE,
            pubkey: parse_public_key(hex_64('c')).unwrap()
        }
    );
}

#[test]
fn contract_bridge_keeps_trade_mutations_out_of_head_selection() {
    let trade = event_with_content(
        KIND_TRADE_PROPOSAL,
        &hex_64('4'),
        &hex_64('d'),
        1,
        vec![
            vec![
                "contract".to_string(),
                "radroots.trade.proposal.v1".to_string(),
            ],
            vec!["p".to_string(), hex_64('e')],
            vec![
                TAG_D.to_string(),
                "11111111111111111111111111111111".to_string(),
            ],
        ],
        r#"{"contract_id":"radroots.trade.proposal.v1"}"#,
    );
    assert_eq!(
        event_head_candidate_for_event(&trade).expect("trade contract"),
        RadrootsEventHeadCandidateResult::NotHeadSelected
    );
}

#[test]
fn contract_bridge_reports_unsupported_and_malformed_shapes() {
    let unsupported = event(999_999, &hex_64('5'), &hex_64('a'), 1, Vec::new());
    assert_eq!(
        event_head_candidate_for_event(&unsupported),
        Err(RadrootsContractMatchError::UnsupportedKind(999_999))
    );

    let malformed_addressable = event(
        KIND_LIST_SET_GENERIC,
        &hex_64('6'),
        &hex_64('a'),
        1,
        Vec::new(),
    );
    assert_eq!(
        event_head_candidate_for_event(&malformed_addressable),
        Err(RadrootsContractMatchError::UnsupportedShape(
            KIND_LIST_SET_GENERIC
        ))
    );

    let regular_with_d_tag = event(
        KIND_POST,
        &hex_64('7'),
        &hex_64('a'),
        1,
        vec![vec![TAG_D.to_string(), "not-a-head".to_string()]],
    );
    assert_eq!(
        event_head_candidate_for_event(&regular_with_d_tag).expect("post contract"),
        RadrootsEventHeadCandidateResult::NotHeadSelected
    );
}

#[test]
fn expect_candidate_reports_non_candidate_inputs() {
    let result = std::panic::catch_unwind(|| {
        expect_candidate(RadrootsEventHeadCandidateResult::NotHeadSelected);
    });

    assert!(result.is_err());
}
