//! High-level all-or-nothing storage workflow contracts.
//!
//! [`AtomicStorage::commit`] is the local durable commit boundary. Dropping its
//! future before that boundary must leave no partial mutation. Cancellation
//! observed after a successful commit cannot claim rollback; replaying the same
//! commit identity and digest returns the original receipt.

use radroots_transport::BoxFuture;

use crate::{
    Error,
    event::{AdmissionReceipt, EventAdmission},
    projection::{ProjectionCheckpoint, ProjectionStatus},
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AtomicCommitId([u8; 16]);

impl AtomicCommitId {
    pub const fn new(bytes: [u8; 16]) -> Result<Self, Error> {
        if bytes_are_zero(&bytes) {
            return Err(Error::InvalidAtomicCommitId);
        }
        Ok(Self(bytes))
    }
    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

/// Domain-owner digest of the complete canonical workflow input.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AtomicCommitDigest([u8; 32]);

impl AtomicCommitDigest {
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AtomicWorkflowKind {
    Ingested,
}

/// Atomic inbound admission with an optional projection checkpoint advance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommitIngested {
    admission: EventAdmission,
    projection: Option<ProjectionCheckpoint>,
}

impl CommitIngested {
    pub const fn new(admission: EventAdmission, projection: Option<ProjectionCheckpoint>) -> Self {
        Self {
            admission,
            projection,
        }
    }
    pub const fn admission(&self) -> &EventAdmission {
        &self.admission
    }
    pub const fn projection(&self) -> Option<&ProjectionCheckpoint> {
        self.projection.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AtomicWorkflow {
    Ingested(Box<CommitIngested>),
}

impl AtomicWorkflow {
    pub const fn kind(&self) -> AtomicWorkflowKind {
        match self {
            Self::Ingested(_) => AtomicWorkflowKind::Ingested,
        }
    }
}

/// Idempotent request for one high-level durable workflow.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AtomicCommit {
    commit_id: AtomicCommitId,
    digest: AtomicCommitDigest,
    requested_at_unix_ms: u64,
    workflow: AtomicWorkflow,
}

impl AtomicCommit {
    pub fn new(
        commit_id: AtomicCommitId,
        digest: AtomicCommitDigest,
        requested_at_unix_ms: u64,
        workflow: AtomicWorkflow,
    ) -> Result<Self, Error> {
        if requested_at_unix_ms == 0 {
            return Err(Error::InvalidAtomicCommitTimestamp);
        }
        Ok(Self {
            commit_id,
            digest,
            requested_at_unix_ms,
            workflow,
        })
    }
    pub const fn commit_id(&self) -> AtomicCommitId {
        self.commit_id
    }
    pub const fn digest(&self) -> AtomicCommitDigest {
        self.digest
    }
    pub const fn requested_at_unix_ms(&self) -> u64 {
        self.requested_at_unix_ms
    }
    pub const fn workflow(&self) -> &AtomicWorkflow {
        &self.workflow
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AtomicCommitDisposition {
    Committed,
    Replay,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AtomicCommitOutcome {
    Ingested {
        admission: AdmissionReceipt,
        projection: Option<Box<ProjectionStatus>>,
    },
}

impl AtomicCommitOutcome {
    pub const fn kind(&self) -> AtomicWorkflowKind {
        match self {
            Self::Ingested { .. } => AtomicWorkflowKind::Ingested,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AtomicCommitReceipt {
    commit_id: AtomicCommitId,
    digest: AtomicCommitDigest,
    disposition: AtomicCommitDisposition,
    committed_at_unix_ms: u64,
    outcome: AtomicCommitOutcome,
}

impl AtomicCommitReceipt {
    pub fn new(
        request: &AtomicCommit,
        disposition: AtomicCommitDisposition,
        committed_at_unix_ms: u64,
        outcome: AtomicCommitOutcome,
    ) -> Result<Self, Error> {
        if [
            committed_at_unix_ms < request.requested_at_unix_ms(),
            outcome.kind() != request.workflow().kind(),
        ]
        .contains(&true)
        {
            return Err(Error::AtomicWorkflowMismatch);
        }
        Ok(Self {
            commit_id: request.commit_id(),
            digest: request.digest(),
            disposition,
            committed_at_unix_ms,
            outcome,
        })
    }

    /// Reconstructs and validates a receipt at a durable backend boundary.
    pub fn from_durable_parts(
        commit_id: AtomicCommitId,
        digest: AtomicCommitDigest,
        disposition: AtomicCommitDisposition,
        requested_at_unix_ms: u64,
        committed_at_unix_ms: u64,
        workflow_kind: AtomicWorkflowKind,
        outcome: AtomicCommitOutcome,
    ) -> Result<Self, Error> {
        if requested_at_unix_ms == 0
            || committed_at_unix_ms < requested_at_unix_ms
            || outcome.kind() != workflow_kind
        {
            return Err(Error::AtomicWorkflowMismatch);
        }
        Ok(Self {
            commit_id,
            digest,
            disposition,
            committed_at_unix_ms,
            outcome,
        })
    }
    pub const fn commit_id(&self) -> AtomicCommitId {
        self.commit_id
    }
    pub const fn digest(&self) -> AtomicCommitDigest {
        self.digest
    }
    pub const fn disposition(&self) -> AtomicCommitDisposition {
        self.disposition
    }
    pub const fn committed_at_unix_ms(&self) -> u64 {
        self.committed_at_unix_ms
    }
    pub const fn outcome(&self) -> &AtomicCommitOutcome {
        &self.outcome
    }
}

/// Backend-neutral all-or-nothing workflow commit SPI.
pub trait AtomicStorage: Send + Sync {
    /// Commits every mutation in the workflow or none of them. Exact replay
    /// returns the original outcome; identity reuse with another digest or kind
    /// is [`Error::AtomicCommitConflict`].
    fn commit(&self, request: AtomicCommit) -> BoxFuture<'_, Result<AtomicCommitReceipt, Error>>;
    fn receipt(
        &self,
        commit_id: AtomicCommitId,
    ) -> BoxFuture<'_, Result<Option<AtomicCommitReceipt>, Error>>;
}

const fn bytes_are_zero(bytes: &[u8; 16]) -> bool {
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != 0 {
            return false;
        }
        index += 1;
    }
    true
}
