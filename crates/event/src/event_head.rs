#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::string::String;

use crate::contract::{
    RadrootsContractMatchError, RadrootsEventClass, RadrootsEventContract, identify_event_contract,
};
use crate::ids::{RadrootsDTag, RadrootsEventId, RadrootsIdParseError, RadrootsPublicKey};
use crate::tags::TAG_D;
use crate::{RadrootsEventEnvelope, RadrootsEventKindClass, RadrootsEventTag};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RadrootsEventHeadCoordinate {
    Replaceable {
        kind: u32,
        pubkey: RadrootsPublicKey,
    },
    Addressable {
        kind: u32,
        pubkey: RadrootsPublicKey,
        d_tag: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsEventHeadCandidate {
    pub coordinate: RadrootsEventHeadCoordinate,
    pub event_id: RadrootsEventId,
    pub created_at: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsCurrentEventHead {
    pub coordinate: RadrootsEventHeadCoordinate,
    pub event_id: RadrootsEventId,
    pub created_at: u64,
}

impl From<RadrootsEventHeadCandidate> for RadrootsCurrentEventHead {
    fn from(candidate: RadrootsEventHeadCandidate) -> Self {
        Self {
            coordinate: candidate.coordinate,
            event_id: candidate.event_id,
            created_at: candidate.created_at,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsEventHeadMalformed {
    InvalidEventId(RadrootsIdParseError),
    InvalidPubkey(RadrootsIdParseError),
    MissingDTag,
    InvalidDTag(RadrootsIdParseError),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsEventHeadCandidateResult {
    Candidate(RadrootsEventHeadCandidate),
    NotHeadSelected,
    NotPersisted,
    Malformed(RadrootsEventHeadMalformed),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsEventHeadDecision {
    Applied(RadrootsCurrentEventHead),
    SkippedDuplicate,
    SkippedOlder,
    SkippedSameTimestampHigherEventId,
    CoordinateMismatch,
}

pub fn event_head_candidate_for_class(
    event: &RadrootsEventEnvelope,
    class: RadrootsEventClass,
) -> RadrootsEventHeadCandidateResult {
    match class {
        RadrootsEventClass::Regular => RadrootsEventHeadCandidateResult::NotHeadSelected,
        RadrootsEventClass::Ephemeral => RadrootsEventHeadCandidateResult::NotPersisted,
        RadrootsEventClass::Replaceable | RadrootsEventClass::Addressable => {
            let event_id = event.id().clone();
            let pubkey = event.author().clone();
            let coordinate = if class == RadrootsEventClass::Replaceable {
                RadrootsEventHeadCoordinate::Replaceable {
                    kind: event.kind_u32(),
                    pubkey,
                }
            } else {
                let Some(d_tag) = first_tag_value(event.tag_slices(), TAG_D) else {
                    return RadrootsEventHeadCandidateResult::Malformed(
                        RadrootsEventHeadMalformed::MissingDTag,
                    );
                };
                let d_tag = match RadrootsDTag::parse(d_tag) {
                    Ok(d_tag) => d_tag,
                    Err(error) => {
                        return RadrootsEventHeadCandidateResult::Malformed(
                            RadrootsEventHeadMalformed::InvalidDTag(error),
                        );
                    }
                };
                RadrootsEventHeadCoordinate::Addressable {
                    kind: event.kind_u32(),
                    pubkey,
                    d_tag: d_tag.into_string(),
                }
            };
            RadrootsEventHeadCandidateResult::Candidate(RadrootsEventHeadCandidate {
                coordinate,
                event_id,
                created_at: event.created_at_u64(),
            })
        }
    }
}

/// Derives the raw NIP-01 head candidate from the numeric event-kind class.
///
/// This deliberately does not identify or validate a Radroots product
/// contract. Raw replacement ordering must include every signature-verified
/// replaceable or addressable event, including unsupported product shapes.
pub fn event_head_candidate_for_nip01_event(
    event: &RadrootsEventEnvelope,
) -> RadrootsEventHeadCandidateResult {
    let coordinate = match event.kind_class() {
        RadrootsEventKindClass::Regular => {
            return RadrootsEventHeadCandidateResult::NotHeadSelected;
        }
        RadrootsEventKindClass::Ephemeral => {
            return RadrootsEventHeadCandidateResult::NotPersisted;
        }
        RadrootsEventKindClass::Replaceable => RadrootsEventHeadCoordinate::Replaceable {
            kind: event.kind_u32(),
            pubkey: event.author().clone(),
        },
        RadrootsEventKindClass::Addressable => RadrootsEventHeadCoordinate::Addressable {
            kind: event.kind_u32(),
            pubkey: event.author().clone(),
            d_tag: String::from(first_tag_value(event.tag_slices(), TAG_D).unwrap_or("")),
        },
    };
    RadrootsEventHeadCandidateResult::Candidate(RadrootsEventHeadCandidate {
        coordinate,
        event_id: event.id().clone(),
        created_at: event.created_at_u64(),
    })
}

pub fn event_head_candidate_for_contract(
    event: &RadrootsEventEnvelope,
    contract: &RadrootsEventContract,
) -> RadrootsEventHeadCandidateResult {
    event_head_candidate_for_class(event, contract.class)
}

pub fn event_head_candidate_for_event(
    event: &RadrootsEventEnvelope,
) -> Result<RadrootsEventHeadCandidateResult, RadrootsContractMatchError> {
    let tags = event.tags_as_vec();
    let contract = identify_event_contract(event.kind_u32(), &tags, event.content())?;
    Ok(event_head_candidate_for_contract(event, contract))
}

pub fn select_event_head(
    candidate: RadrootsEventHeadCandidate,
    current: Option<&RadrootsCurrentEventHead>,
) -> RadrootsEventHeadDecision {
    let Some(current) = current else {
        return RadrootsEventHeadDecision::Applied(candidate.into());
    };
    if candidate.coordinate != current.coordinate {
        return RadrootsEventHeadDecision::CoordinateMismatch;
    }
    if candidate.event_id == current.event_id {
        return RadrootsEventHeadDecision::SkippedDuplicate;
    }
    if candidate.created_at > current.created_at {
        return RadrootsEventHeadDecision::Applied(candidate.into());
    }
    if candidate.created_at < current.created_at {
        return RadrootsEventHeadDecision::SkippedOlder;
    }
    if candidate.event_id < current.event_id {
        RadrootsEventHeadDecision::Applied(candidate.into())
    } else {
        RadrootsEventHeadDecision::SkippedSameTimestampHigherEventId
    }
}

fn first_tag_value<'a>(tags: &'a [RadrootsEventTag], name: &str) -> Option<&'a str> {
    tags.iter()
        .find(|tag| tag.as_slice().first().map(String::as_str) == Some(name))
        .and_then(|tag| tag.as_slice().get(1))
        .map(String::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RadrootsEventEnvelopeParts;
    use crate::contract::RadrootsContractMatchError;
    use crate::kinds::{
        KIND_FOLLOW, KIND_LIST_SET_GENERIC, KIND_POST, KIND_PROFILE, KIND_TRADE_PROPOSAL,
    };

    fn hex_64(character: char) -> String {
        core::iter::repeat_n(character, 64).collect()
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
                pubkey: RadrootsPublicKey::parse(hex_64('a')).unwrap()
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
                pubkey: RadrootsPublicKey::parse(hex_64('b')).unwrap(),
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
                pubkey: RadrootsPublicKey::parse(hex_64('a')).unwrap()
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
                pubkey: RadrootsPublicKey::parse(hex_64('a')).unwrap(),
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
                pubkey: RadrootsPublicKey::parse(hex_64('b')).unwrap(),
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
                    pubkey: RadrootsPublicKey::parse(hex_64('b')).unwrap(),
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
                pubkey: RadrootsPublicKey::parse(hex_64('b')).unwrap(),
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
                pubkey: RadrootsPublicKey::parse(hex_64('c')).unwrap()
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
}
