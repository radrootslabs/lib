//! Canonical event verification and admission orchestration.

use radroots_event::admission::{
    AdmissionPolicy as EventAdmissionPolicy, AdmittedEvent, ContractValidatedEvent, RawEvent,
    VisibilityPolicy,
};
use radroots_event_codec::verify::{self, Nip01SignatureVerifier};
use radroots_storage::{
    Error as StorageError,
    atomic::{
        AtomicCommit, AtomicCommitDigest, AtomicCommitDisposition, AtomicCommitId,
        AtomicCommitOutcome, AtomicWorkflow, CommitIngested,
    },
    event::{AdmissionReceipt, EventAdmission},
};
use radroots_transport::source::ObservedEvent;
use sha2::{Digest, Sha256};

use crate::{
    Engine,
    policy::{Error, OperationKind, SyncId},
};

/// Host decision applied after cryptographic and contract verification.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdmissionDecision {
    /// Reject the event without mutating canonical storage.
    Reject,
    /// Persist the signature-verified event without making it visible.
    Verified,
    /// Persist the event as admitted and visible.
    Visible,
}

/// Deterministic host policy for canonical admission and visibility.
///
/// Implementations must be side-effect free. The engine may evaluate a policy
/// once per observed event and owns all durable effects after that decision.
pub trait AdmissionPolicy: Send + Sync {
    /// Stable policy identity retained in event typestate evidence.
    fn policy_id(&self) -> &'static str;

    /// Decides whether a contract-valid event is rejected, verified-only, or visible.
    fn decide(&self, event: &ContractValidatedEvent) -> AdmissionDecision;
}

/// Registry-valid policy useful for hosts that do not add product authorization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegistryPolicy {
    decision: AdmissionDecision,
}

impl RegistryPolicy {
    /// Admits contract-valid events without authorizing visibility.
    pub const fn verified() -> Self {
        Self {
            decision: AdmissionDecision::Verified,
        }
    }

    /// Admits contract-valid events and authorizes visibility.
    pub const fn visible() -> Self {
        Self {
            decision: AdmissionDecision::Visible,
        }
    }
}

impl AdmissionPolicy for RegistryPolicy {
    fn policy_id(&self) -> &'static str {
        "radroots.registry_v7"
    }

    fn decide(&self, _event: &ContractValidatedEvent) -> AdmissionDecision {
        self.decision
    }
}

/// Normalized durable result for one observed event.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IngestReceipt {
    sync_id: SyncId,
    commit_disposition: AtomicCommitDisposition,
    admission: AdmissionReceipt,
    committed_at_unix_ms: u64,
}

impl IngestReceipt {
    pub const fn sync_id(&self) -> SyncId {
        self.sync_id
    }

    pub const fn commit_disposition(&self) -> AtomicCommitDisposition {
        self.commit_disposition
    }

    pub const fn admission(&self) -> &AdmissionReceipt {
        &self.admission
    }

    pub const fn committed_at_unix_ms(&self) -> u64 {
        self.committed_at_unix_ms
    }
}

/// Ordered independent outcomes for one bounded caller-supplied batch.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IngestBatchReceipt {
    outcomes: Vec<Result<IngestReceipt, Error>>,
}

impl IngestBatchReceipt {
    pub fn outcomes(&self) -> &[Result<IngestReceipt, Error>] {
        self.outcomes.as_slice()
    }

    pub fn accepted(&self) -> usize {
        self.outcomes
            .iter()
            .filter(|outcome| outcome.is_ok())
            .count()
    }

    pub fn rejected(&self) -> usize {
        self.outcomes.len() - self.accepted()
    }
}

impl Engine {
    /// Verifies and atomically persists one exact transport observation.
    pub async fn ingest(
        &self,
        observed: ObservedEvent,
        policy: &dyn AdmissionPolicy,
    ) -> Result<IngestReceipt, Error> {
        let verified = verify::signature(
            verify::id(RawEvent::new(observed.event().envelope().clone()))
                .map_err(|_| Error::VerificationFailed)?,
            &Nip01SignatureVerifier,
        )
        .map_err(|_| Error::VerificationFailed)?;
        let validated =
            verify::contract(verified.clone()).map_err(|_| Error::VerificationFailed)?;
        let decision = policy.decide(&validated);
        if decision == AdmissionDecision::Reject {
            return Err(Error::PolicyRejected);
        }

        let admission = match decision {
            AdmissionDecision::Verified => EventAdmission::verified(observed.clone(), verified),
            AdmissionDecision::Visible => {
                let evidence = DecisionEvidence {
                    policy_id: policy.policy_id(),
                };
                let visible = validated
                    .admit_with(&evidence)
                    .and_then(|event| event.make_visible_with(&evidence))
                    .map_err(|never| match never {})?;
                EventAdmission::visible(observed.clone(), visible)
            }
            AdmissionDecision::Reject => unreachable!("rejection returned before admission"),
        }
        .map_err(map_storage_error)?;

        let sync_id = self.ids.next_id(OperationKind::Ingest)?;
        let requested_at_unix_ms = self.clock.now_unix_ms()?;
        let digest = ingest_digest(&observed, policy.policy_id(), decision);
        let request = AtomicCommit::new(
            AtomicCommitId::new(*sync_id.as_bytes()).map_err(map_storage_error)?,
            digest,
            requested_at_unix_ms,
            AtomicWorkflow::Ingested(Box::new(CommitIngested::new(admission, None))),
        )
        .map_err(map_storage_error)?;
        let receipt = self
            .storage
            .commit(request)
            .await
            .map_err(map_storage_error)?;
        let AtomicCommitOutcome::Ingested { admission, .. } = receipt.outcome() else {
            return Err(Error::InvalidIngestReceipt);
        };
        if admission.event_id() != observed.event().id() {
            return Err(Error::InvalidIngestReceipt);
        }
        Ok(IngestReceipt {
            sync_id,
            commit_disposition: receipt.disposition(),
            admission: admission.clone(),
            committed_at_unix_ms: receipt.committed_at_unix_ms(),
        })
    }

    /// Ingests each supplied observation independently and preserves input order.
    ///
    /// A rejected or failed item does not suppress later items and no hidden
    /// retry, polling, or scheduling loop is created.
    pub async fn ingest_batch(
        &self,
        observed: Vec<ObservedEvent>,
        policy: &dyn AdmissionPolicy,
    ) -> IngestBatchReceipt {
        let mut outcomes = Vec::with_capacity(observed.len());
        for event in observed {
            outcomes.push(self.ingest(event, policy).await);
        }
        IngestBatchReceipt { outcomes }
    }
}

struct DecisionEvidence {
    policy_id: &'static str,
}

impl EventAdmissionPolicy for DecisionEvidence {
    type Error = core::convert::Infallible;

    fn policy_id(&self) -> &'static str {
        self.policy_id
    }

    fn admit(&self, _event: &ContractValidatedEvent) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl VisibilityPolicy for DecisionEvidence {
    type Error = core::convert::Infallible;

    fn policy_id(&self) -> &'static str {
        self.policy_id
    }

    fn make_visible(&self, _event: &AdmittedEvent) -> Result<(), Self::Error> {
        Ok(())
    }
}

fn ingest_digest(
    observed: &ObservedEvent,
    policy_id: &str,
    decision: AdmissionDecision,
) -> AtomicCommitDigest {
    let provenance = observed.provenance();
    let mut hasher = Sha256::new();
    hash_field(&mut hasher, b"radroots.sync.ingest.v1");
    hash_field(&mut hasher, observed.event().raw_json().as_bytes());
    hash_field(&mut hasher, provenance.transport_id().as_str().as_bytes());
    hash_field(&mut hasher, provenance.target().as_str().as_bytes());
    hash_field(
        &mut hasher,
        provenance.observed_at_unix_ms().to_be_bytes().as_slice(),
    );
    hash_field(
        &mut hasher,
        provenance
            .cursor()
            .map_or(&[][..], |cursor| cursor.as_str().as_bytes()),
    );
    hash_field(&mut hasher, policy_id.as_bytes());
    hasher.update([match decision {
        AdmissionDecision::Reject => 0,
        AdmissionDecision::Verified => 1,
        AdmissionDecision::Visible => 2,
    }]);
    AtomicCommitDigest::new(hasher.finalize().into())
}

fn hash_field(hasher: &mut Sha256, value: &[u8]) {
    hasher.update(u64::try_from(value.len()).unwrap_or(u64::MAX).to_be_bytes());
    hasher.update(value);
}

fn map_storage_error(error: StorageError) -> Error {
    match error {
        StorageError::EventConflict | StorageError::AtomicCommitConflict => Error::StorageConflict,
        _ => Error::StorageFailed,
    }
}
