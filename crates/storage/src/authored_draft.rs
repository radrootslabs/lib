//! Immutable authored-draft revisions stored before outbound side effects.

use core::num::NonZeroU64;
use radroots_transport::BoxFuture;
use sha2::{Digest, Sha256};
use std::{string::String, vec::Vec};

use crate::{Error, journal::OperationInstanceId};

pub const AUTHORED_DRAFT_PAYLOAD_MAX_BYTES: usize = 4 * 1024 * 1024;
pub const AUTHORED_DRAFT_SCHEMA_MAX_BYTES: usize = 128;
pub const AUTHORED_DRAFT_QUERY_LIMIT_MAX: u16 = 256;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "[u8; 16]", into = "[u8; 16]"))]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AuthoredDraftId([u8; 16]);

impl AuthoredDraftId {
    pub const fn new(value: [u8; 16]) -> Result<Self, Error> {
        if bytes_are_zero(&value) {
            Err(Error::InvalidAuthoredDraft)
        } else {
            Ok(Self(value))
        }
    }

    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

impl TryFrom<[u8; 16]> for AuthoredDraftId {
    type Error = Error;

    fn try_from(value: [u8; 16]) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<AuthoredDraftId> for [u8; 16] {
    fn from(value: AuthoredDraftId) -> Self {
        value.0
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "u64", into = "u64"))]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AuthoredDraftRevision(NonZeroU64);

impl AuthoredDraftRevision {
    pub const INITIAL: Self = Self(NonZeroU64::MIN);

    pub const fn new(value: u64) -> Result<Self, Error> {
        match NonZeroU64::new(value) {
            Some(value) => Ok(Self(value)),
            None => Err(Error::InvalidAuthoredDraft),
        }
    }

    pub const fn get(self) -> u64 {
        self.0.get()
    }

    pub fn next(self) -> Result<Self, Error> {
        self.get()
            .checked_add(1)
            .ok_or(Error::InvalidAuthoredDraft)
            .and_then(Self::new)
    }
}

impl TryFrom<u64> for AuthoredDraftRevision {
    type Error = Error;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<AuthoredDraftRevision> for u64 {
    fn from(value: AuthoredDraftRevision) -> Self {
        value.get()
    }
}

/// Product-independent persistence phases for one local authored draft.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthoredDraftStage {
    Draft,
    MediaPreparing,
    MediaUploading,
    ReadyToSign,
    Queued,
    Cancelled,
}

impl AuthoredDraftStage {
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Cancelled)
    }
}

/// One immutable, integrity-bound draft revision.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(
        try_from = "AuthoredDraftRevisionWire",
        into = "AuthoredDraftRevisionWire"
    )
)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthoredDraft {
    draft_id: AuthoredDraftId,
    revision: AuthoredDraftRevision,
    author: [u8; 32],
    payload_schema: String,
    payload: Vec<u8>,
    payload_sha256: [u8; 32],
    stage: AuthoredDraftStage,
    operation_id: Option<OperationInstanceId>,
    created_at_unix_ms: u64,
    updated_at_unix_ms: u64,
}

#[cfg(feature = "serde")]
#[derive(serde::Serialize, serde::Deserialize)]
struct AuthoredDraftRevisionWire {
    draft_id: AuthoredDraftId,
    revision: AuthoredDraftRevision,
    author: [u8; 32],
    payload_schema: String,
    payload: Vec<u8>,
    payload_sha256: [u8; 32],
    stage: AuthoredDraftStage,
    operation_id: Option<OperationInstanceId>,
    created_at_unix_ms: u64,
    updated_at_unix_ms: u64,
}

#[cfg(feature = "serde")]
impl TryFrom<AuthoredDraftRevisionWire> for AuthoredDraft {
    type Error = Error;

    fn try_from(value: AuthoredDraftRevisionWire) -> Result<Self, Self::Error> {
        Self::reconstruct(
            value.draft_id,
            value.revision,
            value.author,
            value.payload_schema,
            value.payload,
            value.payload_sha256,
            value.stage,
            value.operation_id,
            value.created_at_unix_ms,
            value.updated_at_unix_ms,
        )
    }
}

#[cfg(feature = "serde")]
impl From<AuthoredDraft> for AuthoredDraftRevisionWire {
    fn from(value: AuthoredDraft) -> Self {
        Self {
            draft_id: value.draft_id,
            revision: value.revision,
            author: value.author,
            payload_schema: value.payload_schema,
            payload: value.payload,
            payload_sha256: value.payload_sha256,
            stage: value.stage,
            operation_id: value.operation_id,
            created_at_unix_ms: value.created_at_unix_ms,
            updated_at_unix_ms: value.updated_at_unix_ms,
        }
    }
}

impl AuthoredDraft {
    #[allow(clippy::too_many_arguments)]
    pub fn initial(
        draft_id: AuthoredDraftId,
        author: [u8; 32],
        payload_schema: impl Into<String>,
        payload: Vec<u8>,
        stage: AuthoredDraftStage,
        operation_id: Option<OperationInstanceId>,
        created_at_unix_ms: u64,
    ) -> Result<Self, Error> {
        if matches!(
            stage,
            AuthoredDraftStage::ReadyToSign
                | AuthoredDraftStage::Queued
                | AuthoredDraftStage::Cancelled
        ) {
            return Err(Error::InvalidAuthoredDraft);
        }
        let payload_sha256 = Sha256::digest(payload.as_slice()).into();
        Self::reconstruct(
            draft_id,
            AuthoredDraftRevision::INITIAL,
            author,
            payload_schema.into(),
            payload,
            payload_sha256,
            stage,
            operation_id,
            created_at_unix_ms,
            created_at_unix_ms,
        )
    }

    pub fn successor(
        &self,
        payload: Vec<u8>,
        stage: AuthoredDraftStage,
        operation_id: Option<OperationInstanceId>,
        updated_at_unix_ms: u64,
    ) -> Result<Self, Error> {
        let payload_sha256 = Sha256::digest(payload.as_slice()).into();
        let next = Self::reconstruct(
            self.draft_id,
            self.revision.next()?,
            self.author,
            self.payload_schema.clone(),
            payload,
            payload_sha256,
            stage,
            operation_id,
            self.created_at_unix_ms,
            updated_at_unix_ms,
        )?;
        next.validate_successor_of(self)?;
        Ok(next)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn reconstruct(
        draft_id: AuthoredDraftId,
        revision: AuthoredDraftRevision,
        author: [u8; 32],
        payload_schema: impl Into<String>,
        payload: Vec<u8>,
        payload_sha256: [u8; 32],
        stage: AuthoredDraftStage,
        operation_id: Option<OperationInstanceId>,
        created_at_unix_ms: u64,
        updated_at_unix_ms: u64,
    ) -> Result<Self, Error> {
        let value = Self {
            draft_id,
            revision,
            author,
            payload_schema: payload_schema.into(),
            payload,
            payload_sha256,
            stage,
            operation_id,
            created_at_unix_ms,
            updated_at_unix_ms,
        };
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<(), Error> {
        let schema = self.payload_schema.as_str();
        let requires_operation = matches!(
            self.stage,
            AuthoredDraftStage::ReadyToSign | AuthoredDraftStage::Queued
        );
        if bytes_are_zero(&self.author)
            || schema.is_empty()
            || schema.len() > AUTHORED_DRAFT_SCHEMA_MAX_BYTES
            || schema != schema.trim()
            || schema.chars().any(char::is_control)
            || self.payload.is_empty()
            || self.payload.len() > AUTHORED_DRAFT_PAYLOAD_MAX_BYTES
            || Sha256::digest(self.payload.as_slice()).as_slice() != self.payload_sha256
            || self.created_at_unix_ms == 0
            || self.updated_at_unix_ms < self.created_at_unix_ms
            || (requires_operation && self.operation_id.is_none())
            || (!requires_operation
                && self.stage != AuthoredDraftStage::Cancelled
                && self.operation_id.is_some())
        {
            return Err(Error::InvalidAuthoredDraft);
        }
        Ok(())
    }

    pub fn validate_successor_of(&self, previous: &Self) -> Result<(), Error> {
        let identity_matches = self.draft_id == previous.draft_id
            && self.author == previous.author
            && self.payload_schema == previous.payload_schema
            && self.created_at_unix_ms == previous.created_at_unix_ms
            && self.revision == previous.revision.next()?
            && self.updated_at_unix_ms >= previous.updated_at_unix_ms;
        let operation_matches = match (previous.operation_id, self.operation_id) {
            (Some(previous), Some(next)) => previous == next,
            (None, _) => true,
            (Some(_), None) => false,
        };
        let frozen_queue_payload = !matches!(
            (previous.stage, self.stage),
            (
                AuthoredDraftStage::ReadyToSign,
                AuthoredDraftStage::ReadyToSign | AuthoredDraftStage::Queued
            ) | (AuthoredDraftStage::Queued, AuthoredDraftStage::Queued)
        ) || self.payload_sha256 == previous.payload_sha256;
        let stage_allowed = match previous.stage {
            AuthoredDraftStage::Draft => matches!(
                self.stage,
                AuthoredDraftStage::Draft
                    | AuthoredDraftStage::MediaPreparing
                    | AuthoredDraftStage::ReadyToSign
                    | AuthoredDraftStage::Cancelled
            ),
            AuthoredDraftStage::MediaPreparing => matches!(
                self.stage,
                AuthoredDraftStage::MediaPreparing
                    | AuthoredDraftStage::MediaUploading
                    | AuthoredDraftStage::ReadyToSign
                    | AuthoredDraftStage::Cancelled
            ),
            AuthoredDraftStage::MediaUploading => matches!(
                self.stage,
                AuthoredDraftStage::MediaPreparing
                    | AuthoredDraftStage::MediaUploading
                    | AuthoredDraftStage::ReadyToSign
                    | AuthoredDraftStage::Cancelled
            ),
            AuthoredDraftStage::ReadyToSign => matches!(
                self.stage,
                AuthoredDraftStage::ReadyToSign
                    | AuthoredDraftStage::Queued
                    | AuthoredDraftStage::Cancelled
            ),
            AuthoredDraftStage::Queued => matches!(
                self.stage,
                AuthoredDraftStage::Queued | AuthoredDraftStage::Cancelled
            ),
            AuthoredDraftStage::Cancelled => false,
        };
        if !identity_matches || !operation_matches || !frozen_queue_payload || !stage_allowed {
            return Err(Error::DraftRevisionConflict);
        }
        Ok(())
    }

    pub const fn draft_id(&self) -> AuthoredDraftId {
        self.draft_id
    }
    pub const fn revision(&self) -> AuthoredDraftRevision {
        self.revision
    }
    pub const fn author(&self) -> &[u8; 32] {
        &self.author
    }
    pub fn payload_schema(&self) -> &str {
        self.payload_schema.as_str()
    }
    pub fn payload(&self) -> &[u8] {
        self.payload.as_slice()
    }
    pub const fn payload_sha256(&self) -> &[u8; 32] {
        &self.payload_sha256
    }
    pub const fn stage(&self) -> AuthoredDraftStage {
        self.stage
    }
    pub const fn operation_id(&self) -> Option<OperationInstanceId> {
        self.operation_id
    }
    pub const fn created_at_unix_ms(&self) -> u64 {
        self.created_at_unix_ms
    }
    pub const fn updated_at_unix_ms(&self) -> u64 {
        self.updated_at_unix_ms
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DraftAppendDisposition {
    Inserted,
    Replay,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DraftAppendReceipt {
    draft: AuthoredDraft,
    disposition: DraftAppendDisposition,
}

impl DraftAppendReceipt {
    pub const fn new(draft: AuthoredDraft, disposition: DraftAppendDisposition) -> Self {
        Self { draft, disposition }
    }
    pub const fn draft(&self) -> &AuthoredDraft {
        &self.draft
    }
    pub const fn disposition(&self) -> DraftAppendDisposition {
        self.disposition
    }
}

pub trait AuthoredDraftStore: Send + Sync {
    fn append_authored_draft(
        &self,
        draft: AuthoredDraft,
        expected_head: Option<AuthoredDraftRevision>,
    ) -> BoxFuture<'_, Result<DraftAppendReceipt, Error>>;

    fn authored_draft_head(
        &self,
        draft_id: AuthoredDraftId,
    ) -> BoxFuture<'_, Result<Option<AuthoredDraft>, Error>>;

    fn authored_draft_revision(
        &self,
        draft_id: AuthoredDraftId,
        revision: AuthoredDraftRevision,
    ) -> BoxFuture<'_, Result<Option<AuthoredDraft>, Error>>;

    fn authored_draft_heads(
        &self,
        author: [u8; 32],
        limit: u16,
    ) -> BoxFuture<'_, Result<Vec<AuthoredDraft>, Error>>;
}

const fn bytes_are_zero<const N: usize>(bytes: &[u8; N]) -> bool {
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != 0 {
            return false;
        }
        index += 1;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft() -> AuthoredDraft {
        AuthoredDraft::initial(
            AuthoredDraftId::new([1; 16]).unwrap(),
            [2; 32],
            "radroots.phase1-draft.v1",
            b"draft".to_vec(),
            AuthoredDraftStage::Draft,
            None,
            10,
        )
        .unwrap()
    }

    #[test]
    fn revisions_are_immutable_and_phase_ordered() {
        let first = draft();
        let media = first
            .successor(
                b"media".to_vec(),
                AuthoredDraftStage::MediaPreparing,
                None,
                11,
            )
            .unwrap();
        let operation = OperationInstanceId::new([3; 16]).unwrap();
        let ready = media
            .successor(
                b"ready".to_vec(),
                AuthoredDraftStage::ReadyToSign,
                Some(operation),
                12,
            )
            .unwrap();
        assert!(
            ready
                .successor(
                    b"changed".to_vec(),
                    AuthoredDraftStage::Queued,
                    Some(operation),
                    13,
                )
                .is_err()
        );
        let queued = ready
            .successor(
                b"ready".to_vec(),
                AuthoredDraftStage::Queued,
                Some(operation),
                13,
            )
            .unwrap();
        assert_eq!(queued.revision().get(), 4);
        assert!(
            queued
                .successor(b"x".to_vec(), AuthoredDraftStage::Draft, None, 14)
                .is_err()
        );
    }

    #[test]
    fn reconstruction_rejects_tampering_and_invalid_contracts() {
        let value = draft();
        assert!(AuthoredDraftId::new([0; 16]).is_err());
        assert!(AuthoredDraftRevision::new(0).is_err());
        assert!(
            AuthoredDraft::reconstruct(
                value.draft_id(),
                value.revision(),
                *value.author(),
                value.payload_schema(),
                value.payload().to_vec(),
                [9; 32],
                value.stage(),
                None,
                value.created_at_unix_ms(),
                value.updated_at_unix_ms(),
            )
            .is_err()
        );
        assert!(
            AuthoredDraft::initial(
                value.draft_id(),
                [0; 32],
                "schema",
                Vec::new(),
                AuthoredDraftStage::ReadyToSign,
                None,
                0,
            )
            .is_err()
        );
    }

    #[test]
    fn validation_rejects_every_independent_invalid_field() {
        let value = draft();
        let digest = |payload: &[u8]| -> [u8; 32] { Sha256::digest(payload).into() };
        let reconstruct = |author: [u8; 32],
                           schema: String,
                           payload: Vec<u8>,
                           payload_sha256: [u8; 32],
                           stage: AuthoredDraftStage,
                           operation_id: Option<OperationInstanceId>,
                           created_at_unix_ms: u64,
                           updated_at_unix_ms: u64| {
            AuthoredDraft::reconstruct(
                value.draft_id(),
                value.revision(),
                author,
                schema,
                payload,
                payload_sha256,
                stage,
                operation_id,
                created_at_unix_ms,
                updated_at_unix_ms,
            )
        };

        let operation = OperationInstanceId::new([3; 16]).unwrap();
        let valid_payload = b"draft".to_vec();
        let valid_digest = digest(&valid_payload);
        let invalid = [
            reconstruct(
                [0; 32],
                "schema".into(),
                valid_payload.clone(),
                valid_digest,
                AuthoredDraftStage::Draft,
                None,
                10,
                10,
            ),
            reconstruct(
                [2; 32],
                String::new(),
                valid_payload.clone(),
                valid_digest,
                AuthoredDraftStage::Draft,
                None,
                10,
                10,
            ),
            reconstruct(
                [2; 32],
                "x".repeat(AUTHORED_DRAFT_SCHEMA_MAX_BYTES + 1),
                valid_payload.clone(),
                valid_digest,
                AuthoredDraftStage::Draft,
                None,
                10,
                10,
            ),
            reconstruct(
                [2; 32],
                " schema".into(),
                valid_payload.clone(),
                valid_digest,
                AuthoredDraftStage::Draft,
                None,
                10,
                10,
            ),
            reconstruct(
                [2; 32],
                "bad\nschema".into(),
                valid_payload.clone(),
                valid_digest,
                AuthoredDraftStage::Draft,
                None,
                10,
                10,
            ),
            reconstruct(
                [2; 32],
                "schema".into(),
                Vec::new(),
                digest(&[]),
                AuthoredDraftStage::Draft,
                None,
                10,
                10,
            ),
            reconstruct(
                [2; 32],
                "schema".into(),
                valid_payload.clone(),
                [9; 32],
                AuthoredDraftStage::Draft,
                None,
                10,
                10,
            ),
            reconstruct(
                [2; 32],
                "schema".into(),
                valid_payload.clone(),
                valid_digest,
                AuthoredDraftStage::Draft,
                None,
                0,
                10,
            ),
            reconstruct(
                [2; 32],
                "schema".into(),
                valid_payload.clone(),
                valid_digest,
                AuthoredDraftStage::Draft,
                None,
                10,
                9,
            ),
            reconstruct(
                [2; 32],
                "schema".into(),
                valid_payload.clone(),
                valid_digest,
                AuthoredDraftStage::ReadyToSign,
                None,
                10,
                10,
            ),
            reconstruct(
                [2; 32],
                "schema".into(),
                valid_payload,
                valid_digest,
                AuthoredDraftStage::Draft,
                Some(operation),
                10,
                10,
            ),
        ];
        assert!(invalid.into_iter().all(|result| result.is_err()));

        let oversized = vec![0; AUTHORED_DRAFT_PAYLOAD_MAX_BYTES + 1];
        assert!(
            reconstruct(
                [2; 32],
                "schema".into(),
                oversized.clone(),
                digest(&oversized),
                AuthoredDraftStage::Draft,
                None,
                10,
                10,
            )
            .is_err()
        );
        assert!(
            AuthoredDraftRevision::new(u64::MAX)
                .unwrap()
                .next()
                .is_err()
        );
        assert!(AuthoredDraftStage::Cancelled.is_terminal());
        assert!(!AuthoredDraftStage::Draft.is_terminal());
        for forbidden_initial_stage in [
            AuthoredDraftStage::ReadyToSign,
            AuthoredDraftStage::Queued,
            AuthoredDraftStage::Cancelled,
        ] {
            assert!(
                AuthoredDraft::initial(
                    AuthoredDraftId::new([4; 16]).unwrap(),
                    [5; 32],
                    "schema",
                    b"payload".to_vec(),
                    forbidden_initial_stage,
                    None,
                    10,
                )
                .is_err()
            );
        }
    }

    #[test]
    fn every_stage_pair_obeys_the_transition_matrix() {
        let stages = [
            AuthoredDraftStage::Draft,
            AuthoredDraftStage::MediaPreparing,
            AuthoredDraftStage::MediaUploading,
            AuthoredDraftStage::ReadyToSign,
            AuthoredDraftStage::Queued,
            AuthoredDraftStage::Cancelled,
        ];
        let operation = OperationInstanceId::new([3; 16]).unwrap();

        for previous_stage in stages {
            for next_stage in stages {
                let previous_operation = matches!(
                    previous_stage,
                    AuthoredDraftStage::ReadyToSign | AuthoredDraftStage::Queued
                )
                .then_some(operation);
                let previous = AuthoredDraft::reconstruct(
                    AuthoredDraftId::new([1; 16]).unwrap(),
                    AuthoredDraftRevision::INITIAL,
                    [2; 32],
                    "schema",
                    b"payload".to_vec(),
                    Sha256::digest(b"payload").into(),
                    previous_stage,
                    previous_operation,
                    10,
                    10,
                )
                .unwrap();
                let next_operation = match next_stage {
                    AuthoredDraftStage::ReadyToSign | AuthoredDraftStage::Queued => Some(operation),
                    AuthoredDraftStage::Cancelled => previous_operation,
                    _ => None,
                };
                let payload = if matches!(
                    (previous_stage, next_stage),
                    (
                        AuthoredDraftStage::ReadyToSign,
                        AuthoredDraftStage::ReadyToSign | AuthoredDraftStage::Queued
                    ) | (AuthoredDraftStage::Queued, AuthoredDraftStage::Queued)
                ) {
                    b"payload".to_vec()
                } else {
                    b"next".to_vec()
                };
                let allowed = match previous_stage {
                    AuthoredDraftStage::Draft => matches!(
                        next_stage,
                        AuthoredDraftStage::Draft
                            | AuthoredDraftStage::MediaPreparing
                            | AuthoredDraftStage::ReadyToSign
                            | AuthoredDraftStage::Cancelled
                    ),
                    AuthoredDraftStage::MediaPreparing => matches!(
                        next_stage,
                        AuthoredDraftStage::MediaPreparing
                            | AuthoredDraftStage::MediaUploading
                            | AuthoredDraftStage::ReadyToSign
                            | AuthoredDraftStage::Cancelled
                    ),
                    AuthoredDraftStage::MediaUploading => matches!(
                        next_stage,
                        AuthoredDraftStage::MediaPreparing
                            | AuthoredDraftStage::MediaUploading
                            | AuthoredDraftStage::ReadyToSign
                            | AuthoredDraftStage::Cancelled
                    ),
                    AuthoredDraftStage::ReadyToSign => matches!(
                        next_stage,
                        AuthoredDraftStage::ReadyToSign
                            | AuthoredDraftStage::Queued
                            | AuthoredDraftStage::Cancelled
                    ),
                    AuthoredDraftStage::Queued => matches!(
                        next_stage,
                        AuthoredDraftStage::Queued | AuthoredDraftStage::Cancelled
                    ),
                    AuthoredDraftStage::Cancelled => false,
                };
                assert_eq!(
                    previous
                        .successor(payload, next_stage, next_operation, 11)
                        .is_ok(),
                    allowed,
                    "{previous_stage:?} -> {next_stage:?}"
                );
            }
        }

        let previous = AuthoredDraft::reconstruct(
            AuthoredDraftId::new([1; 16]).unwrap(),
            AuthoredDraftRevision::INITIAL,
            [2; 32],
            "schema",
            b"payload".to_vec(),
            Sha256::digest(b"payload").into(),
            AuthoredDraftStage::ReadyToSign,
            Some(operation),
            10,
            10,
        )
        .unwrap();
        assert!(
            previous
                .successor(
                    b"payload".to_vec(),
                    AuthoredDraftStage::ReadyToSign,
                    Some(OperationInstanceId::new([4; 16]).unwrap()),
                    11,
                )
                .is_err()
        );
        assert!(
            previous
                .successor(b"payload".to_vec(), AuthoredDraftStage::Cancelled, None, 11,)
                .is_err()
        );
        let rebound_identity = AuthoredDraft::reconstruct(
            AuthoredDraftId::new([9; 16]).unwrap(),
            previous.revision().next().unwrap(),
            *previous.author(),
            previous.payload_schema(),
            previous.payload().to_vec(),
            *previous.payload_sha256(),
            AuthoredDraftStage::ReadyToSign,
            previous.operation_id(),
            previous.created_at_unix_ms(),
            11,
        )
        .unwrap();
        assert_eq!(
            rebound_identity.validate_successor_of(&previous),
            Err(Error::DraftRevisionConflict)
        );
    }
}
