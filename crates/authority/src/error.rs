#![forbid(unsafe_code)]

use thiserror::Error;

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};
#[cfg(feature = "std")]
use std::{string::String, vec::Vec};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RadrootsAuthorityError {
    #[error("invalid actor public key")]
    InvalidActorPubkey,

    #[error("invalid actor account id: empty")]
    InvalidActorAccountIdEmpty,

    #[error("invalid actor account id: contains leading or trailing whitespace")]
    InvalidActorAccountIdUntrimmed,

    #[error("invalid actor account id: contains a control character")]
    InvalidActorAccountIdControlCharacter,

    #[error("invalid actor account id: longer than {max_len} characters")]
    InvalidActorAccountIdTooLong { max_len: usize },

    #[error("invalid signer public key")]
    InvalidSignerPubkey,

    #[error("unknown event contract `{contract_id}`")]
    UnknownContract { contract_id: String },

    #[error("event contract `{contract_id}` expects kind {expected_kind}, got {actual_kind}")]
    DraftKindMismatch {
        contract_id: String,
        expected_kind: u32,
        actual_kind: u32,
    },

    #[error("actor does not satisfy role {required_role:?} for contract `{contract_id}`")]
    ActorRoleUnsatisfied {
        contract_id: String,
        required_role: radroots_events::contract::RadrootsActorRole,
    },

    #[error("actor pubkey mismatch: expected {expected_pubkey}, got {actor_pubkey}")]
    ActorPubkeyMismatch {
        expected_pubkey: String,
        actor_pubkey: String,
    },

    #[error("signer pubkey mismatch: expected {expected_pubkey}, got {signer_pubkey}")]
    SignerPubkeyMismatch {
        expected_pubkey: String,
        signer_pubkey: String,
    },

    #[error("signed event pubkey mismatch: expected {expected_pubkey}, got {actual_pubkey}")]
    SignedEventPubkeyMismatch {
        expected_pubkey: String,
        actual_pubkey: String,
    },

    #[error("signed event id mismatch: expected {expected_event_id}, got {actual_event_id}")]
    SignedEventIdMismatch {
        expected_event_id: String,
        actual_event_id: String,
    },

    #[error(
        "signed event created_at mismatch: expected {expected_created_at}, got {actual_created_at}"
    )]
    SignedEventCreatedAtMismatch {
        expected_created_at: u32,
        actual_created_at: u32,
    },

    #[error("signed event kind mismatch: expected {expected_kind}, got {actual_kind}")]
    SignedEventKindMismatch {
        expected_kind: u32,
        actual_kind: u32,
    },

    #[error("signed event tags mismatch: expected {expected_tags:?}, got {actual_tags:?}")]
    SignedEventTagsMismatch {
        expected_tags: Vec<Vec<String>>,
        actual_tags: Vec<Vec<String>>,
    },

    #[error("signed event content mismatch")]
    SignedEventContentMismatch {
        expected_content: String,
        actual_content: String,
    },

    #[error("signed event computed id could not be derived: {message}")]
    SignedEventComputedIdInvalid { message: String },

    #[error(
        "signed event computed id mismatch: expected {expected_event_id}, computed {computed_event_id}"
    )]
    SignedEventComputedIdMismatch {
        expected_event_id: String,
        computed_event_id: String,
    },

    #[error("signer error: {0}")]
    Signer(#[from] RadrootsSignerError),
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RadrootsSignerError {
    #[error("signer unavailable")]
    Unavailable,

    #[error("signer rejected draft")]
    Rejected,

    #[error("signing failed: {message}")]
    SigningFailed { message: String },
}
