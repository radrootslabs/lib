//! Explicit protected native outbox and fresh seller stock attachment.
//!
//! Fresh stock enrollment is separate from outboxes. Neither can attach a remote endpoint,
//! continue an existing order or restore state from a participant key. Its
//! single connection and persistent writer lock belong to one bounded worker.
//! Losing an observer never drops that lock ahead of the SQLite connection.

mod admission;
mod attempt;
mod crypto;
mod delivery;
mod evidence;
mod fs;
mod installation;
mod issuance;
mod listing;
mod listing_installation;
mod operation;
mod plan;
mod record;
mod recovery;
mod schema;
mod snapshot;
mod stock;
pub use stock::{
    SellerStockConfig, StockAcceptanceOutcome, StockAcceptanceReceipt, StockAcceptanceRequest,
    StockAuthorityReceipt, StockConsentCapture, StockConsentMode, StockConsentOutcome,
    StockConsentReceipt, StockEnrollment, StockGrantEnrollment, StockGrantOutcome,
    StockGrantReceipt, StockHistorySnapshot, StockListingCapture, StockListingOutcome,
    StockListingReceipt, StockListingSnapshot, StockRequestSnapshot, StockSourceCapture,
    StockSourceOutcome, StockSourceProfile, StockSourceReceipt, StockSourceSnapshot,
};
mod worker;

#[cfg(test)]
mod tests;

pub use crypto::WorkspaceCrypto;
pub use radroots_storage::workspace::{
    AttachmentInput, AttachmentProof, WorkspaceBinding, WorkspaceClock, WorkspaceError,
    WorkspaceScope,
};

use radroots_domain::{ExecutorId, GrantId, WorkspaceId};
use radroots_storage::workspace::WorkspaceStore;
use radroots_transport::BoxFuture;
use std::{path::Path, sync::Arc};

pub use radroots_storage::workspace::local_outbox::LocalOutboxConfig;

/// One native local-outbox connection, with an owned bounded request worker.
/// Clones refer to the same admission, writer authority and close completion.
#[derive(Clone)]
pub struct NativeWorkspace {
    worker: Arc<worker::Handle>,
}
impl NativeWorkspace {
    /// Creates a new owner-only namespace and persisted protected grant. All
    /// workspace/executor/grant IDs and the returned credential use OS entropy.
    /// The caller must retain both returned values through protected handling.
    pub async fn initialize_local_outbox(
        path: &Path,
        config: LocalOutboxConfig,
        crypto: WorkspaceCrypto,
        clock: Arc<dyn WorkspaceClock>,
    ) -> Result<(Self, AttachmentInput, AttachmentProof), WorkspaceError> {
        let now = clock.now()?;
        config.validate_at(now)?;
        let secret = zeroize::Zeroizing::new(random()?);
        let proof = AttachmentProof::from_protected_input(secret.as_ref())?;
        let (input, proof, record) = config.enroll(
            now,
            WorkspaceId::from_bytes(random()?),
            ExecutorId::from_bytes(random()?),
            GrantId::from_bytes(random()?),
            proof,
        )?;
        let worker = worker::Handle::launch(
            path.to_path_buf(),
            input.clone(),
            Some(record),
            crypto,
            clock,
        )
        .await?;
        Ok((
            Self {
                worker: Arc::new(worker),
            },
            input,
            proof,
        ))
    }
    /// Opens only an existing exact namespace. The independently retained
    /// selection supplies the expected envelope context; no grant is created.
    pub async fn open_existing(
        path: &Path,
        expected: AttachmentInput,
        crypto: WorkspaceCrypto,
        clock: Arc<dyn WorkspaceClock>,
    ) -> Result<Self, WorkspaceError> {
        let worker =
            worker::Handle::launch(path.to_path_buf(), expected, None, crypto, clock).await?;
        Ok(Self {
            worker: Arc::new(worker),
        })
    }
    /// Authenticates and revokes this exact local grant. A cancelled observer
    /// must query its retained identity; it cannot presume the write aborted.
    pub async fn revoke(
        &self,
        input: AttachmentInput,
        proof: &AttachmentProof,
    ) -> Result<(), WorkspaceError> {
        self.worker.revoke(input, proof).await
    }
    /// Copies an explicitly borrowed protected credential into one owned
    /// bounded authentication request. The caller retains its original input.
    pub async fn authenticate(
        &self,
        input: AttachmentInput,
        proof: &AttachmentProof,
    ) -> Result<WorkspaceBinding, WorkspaceError> {
        let proof = proof.with_secret(|secret| AttachmentProof::from_protected_input(secret))?;
        self.worker.authenticate(input, proof).await
    }
    /// Permanently closes admission, drains admitted requests and explicitly
    /// closes SQLite before releasing the persistent writer authority.
    pub async fn close(&self) -> Result<(), WorkspaceError> {
        self.worker.close().await
    }
}
impl std::fmt::Debug for NativeWorkspace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("NativeWorkspace(<redacted>)")
    }
}
impl WorkspaceStore for NativeWorkspace {
    fn authenticate(
        &self,
        input: AttachmentInput,
        proof: AttachmentProof,
    ) -> BoxFuture<'_, Result<WorkspaceBinding, WorkspaceError>> {
        Box::pin(self.worker.authenticate(input, proof))
    }
    fn close_workspace(&self) -> BoxFuture<'_, Result<(), WorkspaceError>> {
        Box::pin(self.close())
    }
}
impl radroots_storage::command::OperationJournal for NativeWorkspace {
    fn reconcile_unknown<'a>(
        &'a self,
        input: AttachmentInput,
        proof: &'a AttachmentProof,
        evidence: &'a radroots_storage::command::ArtifactEvidence,
    ) -> BoxFuture<
        'a,
        Result<
            radroots_storage::command::EvidenceCaptureOutcome,
            radroots_storage::command::OperationError,
        >,
    > {
        Box::pin(self.worker.reconcile_unknown(input, proof, evidence))
    }
    fn record_delivery_observation(
        &self,
        binding: WorkspaceBinding,
        proof: AttachmentProof,
        observation: radroots_storage::command::DeliveryObservation,
    ) -> BoxFuture<
        '_,
        Result<
            radroots_storage::command::DeliveryObservationOutcome,
            radroots_storage::command::OperationError,
        >,
    > {
        Box::pin(
            self.worker
                .capture_delivery_observation(binding, proof, observation),
        )
    }
    fn delivery_observations(
        &self,
        binding: WorkspaceBinding,
        operation: radroots_domain::OperationId,
        digest: radroots_domain::CommandDigest,
        target: radroots_storage::command::SigningTarget,
        request: radroots_transport::sink::DeliveryRequestId,
        proof: AttachmentProof,
    ) -> BoxFuture<
        '_,
        Result<
            Vec<radroots_storage::command::DeliveryObservation>,
            radroots_storage::command::OperationError,
        >,
    > {
        Box::pin(
            self.worker
                .query_delivery_observations(binding, operation, digest, target, request, proof),
        )
    }
    fn prepare_delivery_attempt<'a>(
        &'a self,
        request: radroots_storage::command::DeliveryAttemptRequest,
        proof: AttachmentProof,
        candidate: Option<&'a radroots_storage::command::EnvelopeCandidate>,
    ) -> BoxFuture<
        'a,
        Result<
            radroots_storage::command::DeliveryAttemptOutcome,
            radroots_storage::command::OperationError,
        >,
    > {
        Box::pin(self.worker.prepare_delivery(request, proof, candidate))
    }
    fn delivery_attempt(
        &self,
        binding: WorkspaceBinding,
        operation: radroots_domain::OperationId,
        digest: radroots_domain::CommandDigest,
        target: radroots_storage::command::SigningTarget,
        request: radroots_transport::sink::DeliveryRequestId,
        proof: AttachmentProof,
    ) -> BoxFuture<
        '_,
        Result<
            Option<radroots_storage::command::DeliveryAttempt>,
            radroots_storage::command::OperationError,
        >,
    > {
        Box::pin(
            self.worker
                .query_delivery(binding, operation, digest, target, request, proof),
        )
    }
    fn install_transition_and_outbox(
        &self,
        request: radroots_storage::command::InstallationRequest,
        proof: AttachmentProof,
    ) -> BoxFuture<
        '_,
        Result<
            radroots_storage::command::InstallationOutcome,
            radroots_storage::command::OperationError,
        >,
    > {
        Box::pin(self.worker.install_chat(request, proof))
    }
    fn operation_snapshot(
        &self,
        binding: WorkspaceBinding,
        operation_id: radroots_domain::OperationId,
        expected_digest: Option<radroots_domain::CommandDigest>,
        proof: AttachmentProof,
    ) -> BoxFuture<
        '_,
        Result<
            Option<radroots_storage::command::OperationSnapshot>,
            radroots_storage::command::OperationError,
        >,
    > {
        Box::pin(
            self.worker
                .operation_snapshot(binding, operation_id, expected_digest, proof),
        )
    }
    fn claim_attempt(
        &self,
        request: radroots_storage::command::ClaimAttemptRequest,
        proof: AttachmentProof,
    ) -> BoxFuture<
        '_,
        Result<
            radroots_storage::command::ClaimAttemptOutcome,
            radroots_storage::command::OperationError,
        >,
    > {
        Box::pin(self.worker.claim_attempt(request, proof))
    }
    fn signing_attempt(
        &self,
        request: radroots_storage::command::ClaimAttemptRequest,
        proof: AttachmentProof,
    ) -> BoxFuture<
        '_,
        Result<
            Option<radroots_storage::command::SigningAttempt>,
            radroots_storage::command::OperationError,
        >,
    > {
        Box::pin(self.worker.signing_attempt(request, proof))
    }
    fn capture_evidence<'a>(
        &'a self,
        input: AttachmentInput,
        proof: &'a AttachmentProof,
        evidence: &'a radroots_storage::command::ArtifactEvidence,
    ) -> BoxFuture<
        'a,
        Result<
            radroots_storage::command::EvidenceCaptureOutcome,
            radroots_storage::command::OperationError,
        >,
    > {
        Box::pin(self.worker.capture_evidence(input, proof, evidence))
    }
    fn obtained_evidence(
        &self,
        request: radroots_storage::command::ClaimAttemptRequest,
        proof: AttachmentProof,
    ) -> BoxFuture<
        '_,
        Result<
            Option<radroots_storage::command::CapturedArtifact>,
            radroots_storage::command::OperationError,
        >,
    > {
        Box::pin(self.worker.obtained_evidence(request, proof))
    }

    fn bind_signing_intents(
        &self,
        binding: WorkspaceBinding,
        plan: radroots_storage::command::CapturedSigningPlan,
        proof: AttachmentProof,
    ) -> BoxFuture<
        '_,
        Result<
            radroots_storage::command::PlanBindingOutcome,
            radroots_storage::command::OperationError,
        >,
    > {
        Box::pin(self.worker.bind_plan(binding, plan, proof))
    }
    fn signing_plan(
        &self,
        binding: WorkspaceBinding,
        operation_id: radroots_domain::OperationId,
        digest: radroots_domain::CommandDigest,
        proof: AttachmentProof,
    ) -> BoxFuture<
        '_,
        Result<
            Option<radroots_storage::command::BoundSigningPlan>,
            radroots_storage::command::OperationError,
        >,
    > {
        Box::pin(
            self.worker
                .signing_plan(binding, operation_id, digest, proof),
        )
    }
    fn admit_command_and_hold(
        &self,
        request: radroots_storage::command::AdmissionRequest,
        proof: AttachmentProof,
    ) -> BoxFuture<
        '_,
        Result<
            radroots_storage::command::AdmissionOutcome,
            radroots_storage::command::OperationError,
        >,
    > {
        Box::pin(self.worker.admit(request, proof))
    }
    fn operation(
        &self,
        binding: WorkspaceBinding,
        operation_id: radroots_domain::OperationId,
        digest: Option<radroots_domain::CommandDigest>,
        proof: AttachmentProof,
    ) -> BoxFuture<
        '_,
        Result<
            Option<radroots_storage::command::PreparedOperation>,
            radroots_storage::command::OperationError,
        >,
    > {
        Box::pin(self.worker.operation(binding, operation_id, digest, proof))
    }
}
fn random() -> Result<[u8; 32], WorkspaceError> {
    let mut value = [0; 32];
    getrandom::getrandom(&mut value).map_err(|_| WorkspaceError::Unavailable)?;
    Ok(value)
}
