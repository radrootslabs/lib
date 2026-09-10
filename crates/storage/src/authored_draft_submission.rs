//! Atomic association of an immutable captured draft with an authored preparation.
//!
//! The application owns the complete semantic request in the intent payload.
//! Storage compares that payload and every preparation field on replay. A command
//! key is scoped by the stable author, never by a process generation or a digest.
use crate::{
    Error,
    atomic::AtomicCommitId,
    authored::{ArtifactOrigin, SigningState},
    authored_atomic::PrepareAuthoredOperation,
    authored_draft::{AuthoredDraft, AuthoredDraftId, AuthoredDraftRevision, AuthoredDraftStage},
    authored_draft_query::{AuthoredDraftQuery, AuthoredDraftScope},
};
use sha2::{Digest, Sha256};

/// Captured source metadata; contains no application payload or credentials.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "SourceWire", into = "SourceWire"))]
pub struct AuthoredDraftSource {
    draft_id: AuthoredDraftId,
    revision: AuthoredDraftRevision,
    author: [u8; 32],
    payload_schema: String,
    scope: Option<AuthoredDraftScope>,
    payload_sha256: [u8; 32],
    stage: AuthoredDraftStage,
    created_at_unix_ms: u64,
    updated_at_unix_ms: u64,
}
impl AuthoredDraftSource {
    pub fn capture(draft: &AuthoredDraft) -> Result<Self, Error> {
        draft.validate()?;
        let value = Self {
            draft_id: draft.draft_id(),
            revision: draft.revision(),
            author: *draft.author(),
            payload_schema: draft.payload_schema().to_owned(),
            scope: draft.scope(),
            payload_sha256: *draft.payload_sha256(),
            stage: draft.stage(),
            created_at_unix_ms: draft.created_at_unix_ms(),
            updated_at_unix_ms: draft.updated_at_unix_ms(),
        };
        value.validate()?;
        Ok(value)
    }
    fn validate(&self) -> Result<(), Error> {
        AuthoredDraftQuery::new(self.author, &self.payload_schema, self.scope, 1)?;
        if !matches!(
            self.stage,
            AuthoredDraftStage::Draft
                | AuthoredDraftStage::MediaPreparing
                | AuthoredDraftStage::MediaUploading
        ) || self.created_at_unix_ms == 0
            || self.updated_at_unix_ms < self.created_at_unix_ms
        {
            return Err(Error::InvalidAuthoredDraft);
        }
        Ok(())
    }
    pub fn matches(&self, draft: &AuthoredDraft) -> bool {
        Self::capture(draft).is_ok_and(|captured| captured == *self)
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
        &self.payload_schema
    }
    pub const fn scope(&self) -> Option<AuthoredDraftScope> {
        self.scope
    }
    pub const fn payload_sha256(&self) -> &[u8; 32] {
        &self.payload_sha256
    }
}

/// A captured request and its distinct immutable intent, installed without effects.
#[derive(Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(try_from = "SubmissionWire", into = "SubmissionWire")
)]
pub struct PrepareFromDraft {
    command_id: AtomicCommitId,
    source: AuthoredDraftSource,
    intent: AuthoredDraft,
    preparation: PrepareAuthoredOperation,
}
impl PrepareFromDraft {
    pub fn new(
        command_id: AtomicCommitId,
        source: AuthoredDraftSource,
        intent: AuthoredDraft,
        preparation: PrepareAuthoredOperation,
    ) -> Result<Self, Error> {
        let value = Self {
            command_id,
            source,
            intent,
            preparation,
        };
        value.validate()?;
        Ok(value)
    }
    pub fn validate(&self) -> Result<(), Error> {
        AtomicCommitId::new(*self.command_id.as_bytes())?;
        self.source.validate()?;
        self.intent.validate()?;
        let operation = self.preparation.operation();
        let at = self.preparation.requested_at_unix_ms();
        if self.intent.draft_id() == self.source.draft_id
            || self.intent.author() != &self.source.author
            || self.intent.scope() != self.source.scope
            || self.intent.revision() != AuthoredDraftRevision::INITIAL
            || !matches!(
                self.intent.stage(),
                AuthoredDraftStage::ReadyToSign | AuthoredDraftStage::Queued
            )
            || self.intent.operation_id() != Some(operation.operation_id())
            || self.intent.created_at_unix_ms() != at
            || self.intent.updated_at_unix_ms() != at
            || at < self.source.updated_at_unix_ms
            || operation.created_at_unix_ms() != at
            || operation.updated_at_unix_ms() != at
            || operation.revision().get() != 1
        {
            return Err(Error::AtomicWorkflowMismatch);
        }
        for artifact in self.preparation.artifacts() {
            if artifact.origin() != ArtifactOrigin::Planned
                || artifact.signing_state() != SigningState::Planned
                || artifact.revision().get() != 1
                || artifact.created_at_unix_ms() != at
                || artifact.updated_at_unix_ms() != at
                || artifact
                    .plan()
                    .ok_or(Error::AtomicWorkflowMismatch)?
                    .decode()?
                    .plan()
                    .author()
                    .as_bytes()
                    != &self.source.author
            {
                return Err(Error::AtomicWorkflowMismatch);
            }
        }
        if self.preparation.delivery_plans().iter().any(|plan| {
            plan.revision().get() != 1
                || plan.created_at_unix_ms() != at
                || plan.updated_at_unix_ms() != at
        }) {
            return Err(Error::AtomicWorkflowMismatch);
        }
        Ok(())
    }
    /// Stable lookup key, available before persistence and after restart.
    pub fn commit_id_for(author: &[u8; 32], command_id: AtomicCommitId) -> AtomicCommitId {
        let mut hash = Sha256::new();
        hash.update(b"radroots.authored.draft.submission.id.v1\0");
        hash.update(author);
        hash.update(command_id.as_bytes());
        let digest = hash.finalize();
        let mut id = [0; 16];
        id.copy_from_slice(&digest[..16]);
        AtomicCommitId::new(id).expect("SHA-256 derived submission identity is nonzero")
    }
    pub fn commit_id(&self) -> AtomicCommitId {
        Self::commit_id_for(&self.source.author, self.command_id)
    }
    pub const fn command_id(&self) -> AtomicCommitId {
        self.command_id
    }
    pub const fn source(&self) -> &AuthoredDraftSource {
        &self.source
    }
    pub const fn intent(&self) -> &AuthoredDraft {
        &self.intent
    }
    pub const fn preparation(&self) -> &PrepareAuthoredOperation {
        &self.preparation
    }
}
impl core::fmt::Debug for PrepareFromDraft {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("PrepareFromDraft")
            .field("command_id", &self.command_id)
            .field("source", &self.source)
            .field("intent_id", &self.intent.draft_id())
            .field("operation_id", &self.preparation.operation().operation_id())
            .finish_non_exhaustive()
    }
}

#[cfg(feature = "serde")]
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceWire {
    draft_id: AuthoredDraftId,
    revision: AuthoredDraftRevision,
    author: [u8; 32],
    payload_schema: String,
    scope: Option<AuthoredDraftScope>,
    payload_sha256: [u8; 32],
    stage: AuthoredDraftStage,
    created_at_unix_ms: u64,
    updated_at_unix_ms: u64,
}
#[cfg(feature = "serde")]
impl TryFrom<SourceWire> for AuthoredDraftSource {
    type Error = Error;
    fn try_from(v: SourceWire) -> Result<Self, Error> {
        let value = Self {
            draft_id: v.draft_id,
            revision: v.revision,
            author: v.author,
            payload_schema: v.payload_schema,
            scope: v.scope,
            payload_sha256: v.payload_sha256,
            stage: v.stage,
            created_at_unix_ms: v.created_at_unix_ms,
            updated_at_unix_ms: v.updated_at_unix_ms,
        };
        value.validate()?;
        Ok(value)
    }
}
#[cfg(feature = "serde")]
impl From<AuthoredDraftSource> for SourceWire {
    fn from(v: AuthoredDraftSource) -> Self {
        Self {
            draft_id: v.draft_id,
            revision: v.revision,
            author: v.author,
            payload_schema: v.payload_schema,
            scope: v.scope,
            payload_sha256: v.payload_sha256,
            stage: v.stage,
            created_at_unix_ms: v.created_at_unix_ms,
            updated_at_unix_ms: v.updated_at_unix_ms,
        }
    }
}
#[cfg(feature = "serde")]
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SubmissionWire {
    command_id: AtomicCommitId,
    source: AuthoredDraftSource,
    intent: AuthoredDraft,
    preparation: PrepareAuthoredOperation,
}
#[cfg(feature = "serde")]
impl TryFrom<SubmissionWire> for PrepareFromDraft {
    type Error = Error;
    fn try_from(v: SubmissionWire) -> Result<Self, Error> {
        Self::new(v.command_id, v.source, v.intent, v.preparation)
    }
}
#[cfg(feature = "serde")]
impl From<PrepareFromDraft> for SubmissionWire {
    fn from(v: PrepareFromDraft) -> Self {
        Self {
            command_id: v.command_id,
            source: v.source,
            intent: v.intent,
            preparation: v.preparation,
        }
    }
}
