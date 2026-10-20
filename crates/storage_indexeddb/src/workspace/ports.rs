//! Shared journal dispatch uses the actual browser owner, never another engine.
use super::BrowserWorkspace;
use radroots_domain::{CommandDigest, OperationId, PublicKey};
use radroots_storage::{
    command::{
        AdmissionOutcome, AdmissionRequest, BoundSigningPlan, CapturedSeal, CapturedSigningPlan,
        ClaimAttemptOutcome, ClaimAttemptRequest, DeliveryAttempt, DeliveryAttemptOutcome,
        DeliveryAttemptRequest, DeliveryObservation, DeliveryObservationOutcome, EnvelopeCandidate,
        EvidenceCaptureOutcome, InstallationOutcome, InstallationRequest, OperationError,
        OperationJournal, OperationSnapshot, PlanBindingOutcome, PreparedOperation, SealEvidence,
        SigningAttempt,
    },
    workspace::{AttachmentInput, AttachmentProof, WorkspaceBinding},
};
use radroots_transport::{BoxFuture, sink::DeliveryRequestId};

impl OperationJournal for BrowserWorkspace {
    fn reconcile_unknown<'a>(
        &'a self,
        input: AttachmentInput,
        proof: &'a AttachmentProof,
        evidence: &'a SealEvidence,
    ) -> BoxFuture<'a, Result<EvidenceCaptureOutcome, OperationError>> {
        Box::pin(BrowserWorkspace::capture_evidence(
            self, input, proof, evidence,
        ))
    }
    fn record_delivery_observation(
        &self,
        binding: WorkspaceBinding,
        proof: AttachmentProof,
        observation: DeliveryObservation,
    ) -> BoxFuture<'_, Result<DeliveryObservationOutcome, OperationError>> {
        Box::pin(BrowserWorkspace::record_delivery_observation(
            self,
            binding,
            proof,
            observation,
        ))
    }
    fn delivery_observations(
        &self,
        binding: WorkspaceBinding,
        operation: OperationId,
        digest: CommandDigest,
        recipient: PublicKey,
        request: DeliveryRequestId,
        proof: AttachmentProof,
    ) -> BoxFuture<'_, Result<Vec<DeliveryObservation>, OperationError>> {
        Box::pin(BrowserWorkspace::delivery_observations(
            self, binding, operation, digest, recipient, request, proof,
        ))
    }
    fn prepare_delivery_attempt<'a>(
        &'a self,
        request: DeliveryAttemptRequest,
        proof: AttachmentProof,
        candidate: Option<&'a EnvelopeCandidate>,
    ) -> BoxFuture<'a, Result<DeliveryAttemptOutcome, OperationError>> {
        Box::pin(BrowserWorkspace::prepare_delivery_attempt(
            self, request, proof, candidate,
        ))
    }
    fn delivery_attempt(
        &self,
        binding: WorkspaceBinding,
        operation: OperationId,
        digest: CommandDigest,
        recipient: PublicKey,
        request: DeliveryRequestId,
        proof: AttachmentProof,
    ) -> BoxFuture<'_, Result<Option<DeliveryAttempt>, OperationError>> {
        Box::pin(BrowserWorkspace::delivery_attempt(
            self, binding, operation, digest, recipient, request, proof,
        ))
    }
    fn install_transition_and_outbox(
        &self,
        request: InstallationRequest,
        proof: AttachmentProof,
    ) -> BoxFuture<'_, Result<InstallationOutcome, OperationError>> {
        Box::pin(BrowserWorkspace::install_transition_and_outbox(
            self, request, proof,
        ))
    }
    fn operation_snapshot(
        &self,
        binding: WorkspaceBinding,
        operation: OperationId,
        digest: Option<CommandDigest>,
        proof: AttachmentProof,
    ) -> BoxFuture<'_, Result<Option<OperationSnapshot>, OperationError>> {
        Box::pin(BrowserWorkspace::operation_snapshot(
            self, binding, operation, digest, proof,
        ))
    }
    fn claim_attempt(
        &self,
        request: ClaimAttemptRequest,
        proof: AttachmentProof,
    ) -> BoxFuture<'_, Result<ClaimAttemptOutcome, OperationError>> {
        Box::pin(BrowserWorkspace::claim_attempt(self, request, proof))
    }
    fn signing_attempt(
        &self,
        request: ClaimAttemptRequest,
        proof: AttachmentProof,
    ) -> BoxFuture<'_, Result<Option<SigningAttempt>, OperationError>> {
        Box::pin(BrowserWorkspace::signing_attempt(self, request, proof))
    }
    fn capture_evidence<'a>(
        &'a self,
        input: AttachmentInput,
        proof: &'a AttachmentProof,
        evidence: &'a SealEvidence,
    ) -> BoxFuture<'a, Result<EvidenceCaptureOutcome, OperationError>> {
        Box::pin(BrowserWorkspace::capture_evidence(
            self, input, proof, evidence,
        ))
    }
    fn obtained_evidence(
        &self,
        request: ClaimAttemptRequest,
        proof: AttachmentProof,
    ) -> BoxFuture<'_, Result<Option<CapturedSeal>, OperationError>> {
        Box::pin(BrowserWorkspace::obtained_evidence(self, request, proof))
    }
    fn bind_signing_intents(
        &self,
        binding: WorkspaceBinding,
        plan: CapturedSigningPlan,
        proof: AttachmentProof,
    ) -> BoxFuture<'_, Result<PlanBindingOutcome, OperationError>> {
        Box::pin(BrowserWorkspace::bind_signing_intents(
            self, binding, plan, proof,
        ))
    }
    fn signing_plan(
        &self,
        binding: WorkspaceBinding,
        operation: OperationId,
        digest: CommandDigest,
        proof: AttachmentProof,
    ) -> BoxFuture<'_, Result<Option<BoundSigningPlan>, OperationError>> {
        Box::pin(BrowserWorkspace::signing_plan(
            self, binding, operation, digest, proof,
        ))
    }
    fn admit_command_and_hold(
        &self,
        request: AdmissionRequest,
        proof: AttachmentProof,
    ) -> BoxFuture<'_, Result<AdmissionOutcome, OperationError>> {
        Box::pin(BrowserWorkspace::admit_command_and_hold(
            self, request, proof,
        ))
    }
    fn operation(
        &self,
        binding: WorkspaceBinding,
        operation: OperationId,
        digest: Option<CommandDigest>,
        proof: AttachmentProof,
    ) -> BoxFuture<'_, Result<Option<PreparedOperation>, OperationError>> {
        Box::pin(BrowserWorkspace::operation(
            self, binding, operation, digest, proof,
        ))
    }
}
