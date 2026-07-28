#![forbid(unsafe_code)]

//! Frozen addressable-feed-v1 head-selection semantics.

#[cfg(not(feature = "std"))]
use alloc::string::String;

use crate::contract::registry_v7::{
    RadrootsContractMatchError, RadrootsEventClass, RadrootsEventContract, identify_event_contract,
};
use crate::envelope::{RadrootsEventEnvelope, RadrootsEventKindClass, RadrootsEventTag};
use crate::ids::{PublicKey, RadrootsDTag, RadrootsEventId, RadrootsIdParseError};
use crate::tags::TAG_D;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RadrootsEventHeadCoordinate {
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
            let event_id = *event.id();
            let pubkey = *event.author();
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
    event_head_candidate_for_nip01_event_v1(event)
}

/// Derives a raw head candidate with addressable-feed-v1 semantics.
pub fn event_head_candidate_for_nip01_event_v1(
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
            pubkey: *event.author(),
        },
        RadrootsEventKindClass::Addressable => RadrootsEventHeadCoordinate::Addressable {
            kind: event.kind_u32(),
            pubkey: *event.author(),
            d_tag: String::from(first_tag_value(event.tag_slices(), TAG_D).unwrap_or("")),
        },
    };
    RadrootsEventHeadCandidateResult::Candidate(RadrootsEventHeadCandidate {
        coordinate,
        event_id: *event.id(),
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
    select_event_head_v1(candidate, current)
}

/// Selects a raw head with addressable-feed-v1 ordering semantics.
pub fn select_event_head_v1(
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
mod tests;
