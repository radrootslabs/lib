#![forbid(unsafe_code)]

//! Frozen addressable-feed-v1 head-selection semantics.

#[cfg(not(feature = "std"))]
use alloc::string::String;

use crate::contract::registry_v7::{
    ContractMatchError, EventClass, EventContract, identify_event_contract,
};
use crate::envelope::{EventEnvelope, EventKindClass, EventTag};
use crate::id::{DTag, EventId, ParseError};
use crate::tag::name::TAG_D;
use radroots_identity::PublicKey;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EventHeadCoordinate {
    Replaceable {
        kind: u32,
        pubkey: PublicKey,
    },
    Addressable {
        kind: u32,
        pubkey: PublicKey,
        d_tag: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventHeadCandidate {
    pub coordinate: EventHeadCoordinate,
    pub event_id: EventId,
    pub created_at: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CurrentEventHead {
    pub coordinate: EventHeadCoordinate,
    pub event_id: EventId,
    pub created_at: u64,
}

impl From<EventHeadCandidate> for CurrentEventHead {
    fn from(candidate: EventHeadCandidate) -> Self {
        Self {
            coordinate: candidate.coordinate,
            event_id: candidate.event_id,
            created_at: candidate.created_at,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EventHeadMalformed {
    InvalidEventId(ParseError),
    InvalidPubkey(ParseError),
    MissingDTag,
    InvalidDTag(ParseError),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EventHeadCandidateResult {
    Candidate(EventHeadCandidate),
    NotHeadSelected,
    NotPersisted,
    Malformed(EventHeadMalformed),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EventHeadDecision {
    Applied(CurrentEventHead),
    SkippedDuplicate,
    SkippedOlder,
    SkippedSameTimestampHigherEventId,
    CoordinateMismatch,
}

pub fn event_head_candidate_for_class(
    event: &EventEnvelope,
    class: EventClass,
) -> EventHeadCandidateResult {
    match class {
        EventClass::Regular => EventHeadCandidateResult::NotHeadSelected,
        EventClass::Ephemeral => EventHeadCandidateResult::NotPersisted,
        EventClass::Replaceable | EventClass::Addressable => {
            let event_id = *event.id();
            let pubkey = *event.author();
            let coordinate = if class == EventClass::Replaceable {
                EventHeadCoordinate::Replaceable {
                    kind: event.kind_u32(),
                    pubkey,
                }
            } else {
                let Some(d_tag) = first_tag_value(event.tag_slices(), TAG_D) else {
                    return EventHeadCandidateResult::Malformed(EventHeadMalformed::MissingDTag);
                };
                let d_tag = match DTag::parse(d_tag) {
                    Ok(d_tag) => d_tag,
                    Err(error) => {
                        return EventHeadCandidateResult::Malformed(
                            EventHeadMalformed::InvalidDTag(error),
                        );
                    }
                };
                EventHeadCoordinate::Addressable {
                    kind: event.kind_u32(),
                    pubkey,
                    d_tag: d_tag.into_string(),
                }
            };
            EventHeadCandidateResult::Candidate(EventHeadCandidate {
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
pub fn event_head_candidate_for_nip01_event(event: &EventEnvelope) -> EventHeadCandidateResult {
    event_head_candidate_for_nip01_event_v1(event)
}

/// Derives a raw head candidate with addressable-feed-v1 semantics.
pub fn event_head_candidate_for_nip01_event_v1(event: &EventEnvelope) -> EventHeadCandidateResult {
    let coordinate = match event.kind_class() {
        EventKindClass::Regular => {
            return EventHeadCandidateResult::NotHeadSelected;
        }
        EventKindClass::Ephemeral => {
            return EventHeadCandidateResult::NotPersisted;
        }
        EventKindClass::Replaceable => EventHeadCoordinate::Replaceable {
            kind: event.kind_u32(),
            pubkey: *event.author(),
        },
        EventKindClass::Addressable => EventHeadCoordinate::Addressable {
            kind: event.kind_u32(),
            pubkey: *event.author(),
            d_tag: String::from(first_tag_value(event.tag_slices(), TAG_D).unwrap_or("")),
        },
    };
    EventHeadCandidateResult::Candidate(EventHeadCandidate {
        coordinate,
        event_id: *event.id(),
        created_at: event.created_at_u64(),
    })
}

pub fn event_head_candidate_for_contract(
    event: &EventEnvelope,
    contract: &EventContract,
) -> EventHeadCandidateResult {
    event_head_candidate_for_class(event, contract.class)
}

pub fn event_head_candidate_for_event(
    event: &EventEnvelope,
) -> Result<EventHeadCandidateResult, ContractMatchError> {
    let tags = event.tags_as_vec();
    let contract = identify_event_contract(event.kind_u32(), &tags, event.content())?;
    Ok(event_head_candidate_for_contract(event, contract))
}

pub fn select_event_head(
    candidate: EventHeadCandidate,
    current: Option<&CurrentEventHead>,
) -> EventHeadDecision {
    select_event_head_v1(candidate, current)
}

/// Selects a raw head with addressable-feed-v1 ordering semantics.
pub fn select_event_head_v1(
    candidate: EventHeadCandidate,
    current: Option<&CurrentEventHead>,
) -> EventHeadDecision {
    let Some(current) = current else {
        return EventHeadDecision::Applied(candidate.into());
    };
    if candidate.coordinate != current.coordinate {
        return EventHeadDecision::CoordinateMismatch;
    }
    if candidate.event_id == current.event_id {
        return EventHeadDecision::SkippedDuplicate;
    }
    if candidate.created_at > current.created_at {
        return EventHeadDecision::Applied(candidate.into());
    }
    if candidate.created_at < current.created_at {
        return EventHeadDecision::SkippedOlder;
    }
    if candidate.event_id < current.event_id {
        EventHeadDecision::Applied(candidate.into())
    } else {
        EventHeadDecision::SkippedSameTimestampHigherEventId
    }
}

fn first_tag_value<'a>(tags: &'a [EventTag], name: &str) -> Option<&'a str> {
    tags.iter()
        .find(|tag| tag.as_slice().first().map(String::as_str) == Some(name))
        .and_then(|tag| tag.as_slice().get(1))
        .map(String::as_str)
}

#[cfg(test)]
#[path = "v1/tests.rs"]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests;
