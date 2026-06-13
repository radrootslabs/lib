#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use crate::RadrootsNostrEvent;
use crate::contract::{
    RadrootsContractMatchError, RadrootsEventClass, RadrootsEventContract, identify_event_contract,
};
use crate::ids::{RadrootsDTag, RadrootsEventId, RadrootsIdParseError, RadrootsPublicKey};
use crate::tags::TAG_D;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RadrootsEventHeadCoordinate {
    Replaceable {
        kind: u32,
        pubkey: RadrootsPublicKey,
    },
    Addressable {
        kind: u32,
        pubkey: RadrootsPublicKey,
        d_tag: RadrootsDTag,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsEventHeadCandidate {
    pub coordinate: RadrootsEventHeadCoordinate,
    pub event_id: RadrootsEventId,
    pub created_at: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsCurrentEventHead {
    pub coordinate: RadrootsEventHeadCoordinate,
    pub event_id: RadrootsEventId,
    pub created_at: u32,
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
    event: &RadrootsNostrEvent,
    class: RadrootsEventClass,
) -> RadrootsEventHeadCandidateResult {
    match class {
        RadrootsEventClass::Regular => RadrootsEventHeadCandidateResult::NotHeadSelected,
        RadrootsEventClass::Ephemeral => RadrootsEventHeadCandidateResult::NotPersisted,
        RadrootsEventClass::Replaceable | RadrootsEventClass::Addressable => {
            let event_id = match RadrootsEventId::parse(&event.id) {
                Ok(event_id) => event_id,
                Err(error) => {
                    return RadrootsEventHeadCandidateResult::Malformed(
                        RadrootsEventHeadMalformed::InvalidEventId(error),
                    );
                }
            };
            let pubkey = match RadrootsPublicKey::parse(&event.author) {
                Ok(pubkey) => pubkey,
                Err(error) => {
                    return RadrootsEventHeadCandidateResult::Malformed(
                        RadrootsEventHeadMalformed::InvalidPubkey(error),
                    );
                }
            };
            let coordinate = match class {
                RadrootsEventClass::Replaceable => RadrootsEventHeadCoordinate::Replaceable {
                    kind: event.kind,
                    pubkey,
                },
                RadrootsEventClass::Addressable => {
                    let Some(d_tag) = first_tag_value(&event.tags, TAG_D) else {
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
                        kind: event.kind,
                        pubkey,
                        d_tag,
                    }
                }
                RadrootsEventClass::Regular | RadrootsEventClass::Ephemeral => unreachable!(),
            };
            RadrootsEventHeadCandidateResult::Candidate(RadrootsEventHeadCandidate {
                coordinate,
                event_id,
                created_at: event.created_at,
            })
        }
    }
}

pub fn event_head_candidate_for_contract(
    event: &RadrootsNostrEvent,
    contract: &RadrootsEventContract,
) -> RadrootsEventHeadCandidateResult {
    event_head_candidate_for_class(event, contract.class)
}

pub fn event_head_candidate_for_event(
    event: &RadrootsNostrEvent,
) -> Result<RadrootsEventHeadCandidateResult, RadrootsContractMatchError> {
    let contract = identify_event_contract(event.kind, &event.tags, &event.content)?;
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

fn first_tag_value<'a>(tags: &'a [Vec<String>], name: &str) -> Option<&'a str> {
    tags.iter()
        .find(|tag| tag.first().map(String::as_str) == Some(name))
        .and_then(|tag| tag.get(1))
        .map(String::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::RadrootsContractMatchError;
    use crate::kinds::{
        KIND_FOLLOW, KIND_LIST_SET_GENERIC, KIND_ORDER_REQUEST, KIND_POST, KIND_PROFILE,
    };

    fn hex_64(character: char) -> String {
        core::iter::repeat_n(character, 64).collect()
    }

    fn event(
        kind: u32,
        id: &str,
        author: &str,
        created_at: u32,
        tags: Vec<Vec<String>>,
    ) -> RadrootsNostrEvent {
        RadrootsNostrEvent {
            id: id.to_string(),
            author: author.to_string(),
            created_at,
            kind,
            tags,
            content: String::new(),
            sig: String::new(),
        }
    }

    fn event_with_content(
        kind: u32,
        id: &str,
        author: &str,
        created_at: u32,
        tags: Vec<Vec<String>>,
        content: &str,
    ) -> RadrootsNostrEvent {
        let mut event = event(kind, id, author, created_at, tags);
        event.content = content.to_string();
        event
    }

    fn candidate(id: char, created_at: u32) -> RadrootsEventHeadCandidate {
        match event_head_candidate_for_class(
            &event(10002, &hex_64(id), &hex_64('a'), created_at, Vec::new()),
            RadrootsEventClass::Replaceable,
        ) {
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
        let RadrootsEventHeadCandidateResult::Candidate(candidate) =
            event_head_candidate_for_class(&event, RadrootsEventClass::Replaceable)
        else {
            panic!("expected candidate")
        };
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
        let RadrootsEventHeadCandidateResult::Candidate(candidate) =
            event_head_candidate_for_class(&event, RadrootsEventClass::Addressable)
        else {
            panic!("expected candidate")
        };
        assert_eq!(
            candidate.coordinate,
            RadrootsEventHeadCoordinate::Addressable {
                kind: 30023,
                pubkey: RadrootsPublicKey::parse(hex_64('b')).unwrap(),
                d_tag: RadrootsDTag::parse("article-1").unwrap()
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
    fn malformed_candidates_report_invalid_event_ids_and_pubkeys() {
        let bad_event_id = event(10002, "not-hex", &hex_64('a'), 1, Vec::new());
        assert!(matches!(
            event_head_candidate_for_class(&bad_event_id, RadrootsEventClass::Replaceable),
            RadrootsEventHeadCandidateResult::Malformed(
                RadrootsEventHeadMalformed::InvalidEventId(_)
            )
        ));

        let bad_pubkey = event(10002, &hex_64('1'), "not-hex", 1, Vec::new());
        assert!(matches!(
            event_head_candidate_for_class(&bad_pubkey, RadrootsEventClass::Replaceable),
            RadrootsEventHeadCandidateResult::Malformed(RadrootsEventHeadMalformed::InvalidPubkey(
                _
            ))
        ));
    }

    #[test]
    fn event_head_selection_uses_nip01_time_and_lowest_id_rules() {
        let current: RadrootsCurrentEventHead = candidate('3', 10).into();

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
        let RadrootsEventHeadCandidateResult::Candidate(other) = other else {
            panic!("expected candidate")
        };
        assert_eq!(
            select_event_head(other, Some(&current)),
            RadrootsEventHeadDecision::CoordinateMismatch
        );
    }

    #[test]
    fn contract_bridge_uses_replaceable_event_classes() {
        let event = event(KIND_FOLLOW, &hex_64('1'), &hex_64('a'), 1, Vec::new());
        let RadrootsEventHeadCandidateResult::Candidate(candidate) =
            event_head_candidate_for_event(&event).expect("contract")
        else {
            panic!("expected candidate")
        };
        assert_eq!(
            candidate.coordinate,
            RadrootsEventHeadCoordinate::Replaceable {
                kind: KIND_FOLLOW,
                pubkey: RadrootsPublicKey::parse(hex_64('a')).unwrap()
            }
        );
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
        let RadrootsEventHeadCandidateResult::Candidate(candidate) =
            event_head_candidate_for_event(&event).expect("contract")
        else {
            panic!("expected candidate")
        };
        assert_eq!(
            candidate.coordinate,
            RadrootsEventHeadCoordinate::Addressable {
                kind: KIND_LIST_SET_GENERIC,
                pubkey: RadrootsPublicKey::parse(hex_64('b')).unwrap(),
                d_tag: RadrootsDTag::parse("member_of.farms").unwrap()
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
        let RadrootsEventHeadCandidateResult::Candidate(candidate) =
            event_head_candidate_for_event(&profile).expect("profile contract")
        else {
            panic!("expected candidate")
        };
        assert_eq!(
            candidate.coordinate,
            RadrootsEventHeadCoordinate::Replaceable {
                kind: KIND_PROFILE,
                pubkey: RadrootsPublicKey::parse(hex_64('c')).unwrap()
            }
        );
    }

    #[test]
    fn contract_bridge_keeps_order_events_out_of_head_selection() {
        let order = event_with_content(
            KIND_ORDER_REQUEST,
            &hex_64('4'),
            &hex_64('d'),
            1,
            vec![
                vec!["p".to_string(), hex_64('e')],
                vec!["a".to_string(), format!("30402:{}:listing-1", hex_64('f'))],
                vec![TAG_D.to_string(), "order-1".to_string()],
            ],
            "{}",
        );
        assert_eq!(
            event_head_candidate_for_event(&order).expect("order contract"),
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
}
