//! Exact signature facts bound to an already committed signing attempt.

use radroots_event::SignedEvent;
use radroots_event_codec::verify::{self, Nip01SignatureVerifier, RawEvent};

use super::{
    AuthoredAtomicCommand, AuthoredAtomicOutcome, AuthoredAtomicReceipt, ClaimAuthoredTarget,
    ClaimAuthoredWork,
};
use crate::{
    Error,
    authored::{AuthoredArtifact, AuthoredArtifactId, WorkClaim},
    journal::OperationInstanceId,
};

/// Records an existing authored signature without granting scheduling authority.
///
/// The backend must retrieve its own immutable receipt for [`Self::claim_command`]
/// inside the recording transaction. A caller-supplied receipt is not durable
/// attempt authority. The original claim may have expired or been superseded;
/// new signing, admission and delivery still require their ordinary fences.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordSignedArtifact {
    operation_id: OperationInstanceId,
    artifact_id: AuthoredArtifactId,
    claim: WorkClaim,
    event: SignedEvent,
    observed_at_unix_ms: u64,
}

impl RecordSignedArtifact {
    pub fn new(
        operation_id: OperationInstanceId,
        artifact_id: AuthoredArtifactId,
        claim: WorkClaim,
        event: SignedEvent,
        observed_at_unix_ms: u64,
    ) -> Result<Self, Error> {
        claim.validate()?;
        if observed_at_unix_ms < claim.acquired_at_unix_ms() {
            return Err(Error::AtomicWorkflowMismatch);
        }
        let id_verified = verify::id(RawEvent::new(event.envelope().clone()))
            .map_err(|_| Error::InvalidAuthoredArtifact)?;
        verify::signature(id_verified, &Nip01SignatureVerifier)
            .map_err(|_| Error::InvalidAuthoredArtifact)?;
        Ok(Self {
            operation_id,
            artifact_id,
            claim,
            event,
            observed_at_unix_ms,
        })
    }

    pub const fn operation_id(&self) -> OperationInstanceId {
        self.operation_id
    }
    pub const fn artifact_id(&self) -> AuthoredArtifactId {
        self.artifact_id
    }
    pub const fn claim(&self) -> &WorkClaim {
        &self.claim
    }
    pub const fn event(&self) -> &SignedEvent {
        &self.event
    }
    pub const fn observed_at_unix_ms(&self) -> u64 {
        self.observed_at_unix_ms
    }

    pub fn claim_command(&self) -> AuthoredAtomicCommand {
        AuthoredAtomicCommand::Claim(ClaimAuthoredWork::new(
            ClaimAuthoredTarget::ArtifactSigning(self.artifact_id),
            self.claim.clone(),
        ))
    }

    /// Applies facts only after exact backend-owned attempt provenance is checked.
    ///
    /// The first signed bytes are immutable. An identical result changes nothing;
    /// a different result conflicts. Cancellation and terminal failure survive.
    pub fn apply_to(
        &self,
        artifact: &mut AuthoredArtifact,
        original_claim: &AuthoredAtomicReceipt,
    ) -> Result<(), Error> {
        let AuthoredAtomicOutcome::Artifact(original) = original_claim.outcome() else {
            return Err(Error::AtomicWorkflowMismatch);
        };
        original.validate()?;
        artifact.validate()?;
        if !original_claim.matches_command(&self.claim_command())
            || original_claim.committed_at_unix_ms() != self.claim.acquired_at_unix_ms()
            || original.signing_claim() != Some(&self.claim)
            || original.operation_id() != self.operation_id
            || artifact.operation_id() != self.operation_id
            || original.artifact_id() != self.artifact_id
            || artifact.artifact_id() != self.artifact_id
            || original.plan() != artifact.plan()
            || original.origin() != artifact.origin()
            || original.ordinal() != artifact.ordinal()
            || original.created_at_unix_ms() != artifact.created_at_unix_ms()
            || original.revision() > artifact.revision()
            || original.updated_at_unix_ms() > artifact.updated_at_unix_ms()
        {
            return Err(Error::AtomicWorkflowMismatch);
        }
        artifact.record_signed_fact(self.event.clone(), self.observed_at_unix_ms)
    }
}
