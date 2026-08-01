//! High-level all-or-nothing storage workflow contracts.
//!
//! [`AtomicStorage::commit`] is the local durable commit boundary. Dropping its
//! future before that boundary must leave no partial mutation. Cancellation
//! observed after a successful commit cannot claim rollback; replaying the same
//! commit identity and digest returns the original receipt.

use radroots_event::{EventId, SignedEvent};
use radroots_transport::BoxFuture;

use crate::{
    Error,
    event::{AdmissionReceipt, EventAdmission},
    journal::{JournalRevision, OperationInstanceId, OperationRecord, PrepareOperation},
    outbox::{DeliveryAttemptEvidence, EnqueueOutboxItem, OutboxRecord},
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
    Prepared,
    Signed,
    Enqueued,
    Delivered,
    Ingested,
}

/// Journal transition inputs for a newly signed operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommitSigned {
    instance_id: OperationInstanceId,
    expected_revision: JournalRevision,
    event: SignedEvent,
}

impl CommitSigned {
    pub fn new(
        instance_id: OperationInstanceId,
        expected_revision: JournalRevision,
        event: SignedEvent,
    ) -> Self {
        Self {
            instance_id,
            expected_revision,
            event,
        }
    }
    pub const fn instance_id(&self) -> OperationInstanceId {
        self.instance_id
    }
    pub const fn expected_revision(&self) -> JournalRevision {
        self.expected_revision
    }
    pub const fn event(&self) -> &SignedEvent {
        &self.event
    }
}

/// Atomic canonical admission, journal commit, and delivery-plan enqueue.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommitEnqueued {
    instance_id: OperationInstanceId,
    expected_revision: JournalRevision,
    admission: EventAdmission,
    outbox: EnqueueOutboxItem,
    committed_at_unix_ms: u64,
}

impl CommitEnqueued {
    pub fn new(
        instance_id: OperationInstanceId,
        expected_revision: JournalRevision,
        admission: EventAdmission,
        outbox: EnqueueOutboxItem,
        committed_at_unix_ms: u64,
    ) -> Result<Self, Error> {
        if committed_at_unix_ms == 0
            || admission.event_id() != outbox.request().payload().event().id()
            || outbox.operation_instance_id() != instance_id
        {
            return Err(Error::AtomicWorkflowMismatch);
        }
        Ok(Self {
            instance_id,
            expected_revision,
            admission,
            outbox,
            committed_at_unix_ms,
        })
    }
    pub const fn instance_id(&self) -> OperationInstanceId {
        self.instance_id
    }
    pub const fn expected_revision(&self) -> JournalRevision {
        self.expected_revision
    }
    pub const fn admission(&self) -> &EventAdmission {
        &self.admission
    }
    pub const fn outbox(&self) -> &EnqueueOutboxItem {
        &self.outbox
    }
    pub const fn committed_at_unix_ms(&self) -> u64 {
        self.committed_at_unix_ms
    }
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
    Prepared(PrepareOperation),
    Signed(Box<CommitSigned>),
    Enqueued(Box<CommitEnqueued>),
    Delivered(Box<DeliveryAttemptEvidence>),
    Ingested(Box<CommitIngested>),
}

impl AtomicWorkflow {
    pub const fn kind(&self) -> AtomicWorkflowKind {
        match self {
            Self::Prepared(_) => AtomicWorkflowKind::Prepared,
            Self::Signed(_) => AtomicWorkflowKind::Signed,
            Self::Enqueued(_) => AtomicWorkflowKind::Enqueued,
            Self::Delivered(_) => AtomicWorkflowKind::Delivered,
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
    Prepared {
        journal: OperationRecord,
    },
    Signed {
        journal: OperationRecord,
        event_id: EventId,
    },
    Enqueued {
        journal: OperationRecord,
        admission: AdmissionReceipt,
        outbox: Box<OutboxRecord>,
    },
    Delivered {
        outbox: Box<OutboxRecord>,
    },
    Ingested {
        admission: AdmissionReceipt,
        projection: Option<Box<ProjectionStatus>>,
    },
}

impl AtomicCommitOutcome {
    pub const fn kind(&self) -> AtomicWorkflowKind {
        match self {
            Self::Prepared { .. } => AtomicWorkflowKind::Prepared,
            Self::Signed { .. } => AtomicWorkflowKind::Signed,
            Self::Enqueued { .. } => AtomicWorkflowKind::Enqueued,
            Self::Delivered { .. } => AtomicWorkflowKind::Delivered,
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
        if committed_at_unix_ms < request.requested_at_unix_ms()
            || outcome.kind() != request.workflow().kind()
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
