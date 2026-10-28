//! Mechanical views of the same selected native stock owner for Core workflows.
use radroots_domain::PublicKey;
use radroots_execution::session::{
    HostFuture,
    commerce::{
        AcceptanceClaim, AcceptanceClaimOutcome, AcceptanceDispatch, AcceptanceDispatchOutcome,
        AcceptanceInstallationOutcome, AcceptancePlanOutcome, AcceptanceRecipient,
        AcceptanceSelection, AcceptanceSnapshot, AcceptanceStore, AcceptanceUnissued,
        AcceptanceUnissuedOutcome, AcceptanceUnissuedReceipt, InstalledAcceptance,
    },
};
use radroots_signing::{
    host::Shared,
    issuance::{ExactIssuer, ExactRecoveryRequest},
    provider::ProviderCall,
    request::CancellationSignal,
};
use radroots_storage::{
    command::OperationError,
    inventory::{InventoryCheckpointRef, StockAcceptanceReceipt},
    workspace::AttachmentProof,
};
use radroots_storage_sqlite::workspace::{
    NativeWorkspace, StockClaimOutcome, StockClaimRequest, StockDispatchOutcome,
    StockDispatchPermit, StockInstallationOutcome, StockInstallationReceipt,
    StockInstallationRequest, StockOperationSnapshot, StockSigningAttempt, StockSigningPlanOutcome,
    StockSigningPlanRequest, StockSnapshotRequest, StockUnissuedInvocation, StockUnissuedOutcome,
    StockUnissuedReceipt,
};

pub(crate) struct NativeCommerce(pub(crate) Shared<NativeWorkspace>);
struct Snapshot(StockOperationSnapshot);
struct Dispatch {
    permit: StockDispatchPermit,
    store: Shared<NativeWorkspace>,
    binding: radroots_storage::workspace::WorkspaceBinding,
    proof: AttachmentProof,
}
struct Unissued {
    observation: StockUnissuedInvocation,
    store: Shared<NativeWorkspace>,
    binding: radroots_storage::workspace::WorkspaceBinding,
    proof: AttachmentProof,
}
fn refusal(value: &StockUnissuedReceipt) -> AcceptanceUnissuedReceipt {
    AcceptanceUnissuedReceipt {
        attempt: value.attempt(),
        claim: value.signing_attempt_id(),
        recipient: value.recipient(),
        reason: value.reason(),
        checkpoint: value.checkpoint(),
    }
}
fn refusal_outcome(value: StockUnissuedOutcome) -> AcceptanceUnissuedOutcome {
    match value {
        StockUnissuedOutcome::Committed(value) => {
            AcceptanceUnissuedOutcome::Committed(Box::new(refusal(&value)))
        }
        StockUnissuedOutcome::Unknown(value) => AcceptanceUnissuedOutcome::Unknown(value),
    }
}
impl AcceptanceUnissued for Unissued {
    fn claim(&self) -> AcceptanceClaim {
        claim(self.observation.attempt())
    }
    fn reason(&self) -> radroots_signing::provider::ProviderError {
        self.observation.reason()
    }
    fn capture(&self) -> HostFuture<'_, Result<AcceptanceUnissuedOutcome, OperationError>> {
        Box::pin(async move {
            self.store
                .capture_stock_original_unissued(
                    self.binding.clone(),
                    &self.observation,
                    &self.proof,
                )
                .await
                .map(refusal_outcome)
        })
    }
    fn reconcile(&self) -> HostFuture<'_, Result<AcceptanceUnissuedOutcome, OperationError>> {
        Box::pin(async move {
            self.store
                .reconcile_stock_original_unissued(
                    self.binding.clone(),
                    &self.observation,
                    &self.proof,
                )
                .await
                .map(refusal_outcome)
        })
    }
}
fn claim(value: &StockSigningAttempt) -> AcceptanceClaim {
    AcceptanceClaim {
        id: value.attempt_id(),
        binding: value.binding(),
    }
}
fn installation(value: &StockInstallationReceipt) -> InstalledAcceptance {
    InstalledAcceptance {
        attempt: value.attempt(),
        order: value.order_id(),
        record: value.record_id(),
        checkpoint: value.checkpoint(),
    }
}
impl AcceptanceSnapshot for Snapshot {
    fn preparation(&self) -> &StockAcceptanceReceipt {
        self.0.preparation()
    }
    fn plan(&self) -> Option<&radroots_protocol::commerce::CommerceSigningPlan> {
        self.0.signing_plan().map(|v| v.plan())
    }
    fn checkpoint(&self) -> InventoryCheckpointRef {
        self.0.checkpoint()
    }
    fn recipients(&self) -> [AcceptanceRecipient; 2] {
        self.0.recipients().each_ref().map(|v| AcceptanceRecipient {
            recipient: v.recipient(),
            claim: v.claim().map(|c| AcceptanceClaim {
                id: c.attempt_id(),
                binding: c.binding(),
            }),
            evidence: v.first_evidence(),
            original_unissued: v.original_unissued().as_ref().map(refusal),
        })
    }
    fn installation(&self) -> Option<InstalledAcceptance> {
        self.0.installation().map(installation)
    }
    fn aborted(&self) -> bool {
        self.0.aborted().is_some()
    }
    fn delivery(&self) -> Option<&[radroots_storage::inventory::StockDeliverySummary; 2]> {
        self.0.delivery()
    }
}
impl AcceptanceDispatch for Dispatch {
    fn claim(&self) -> AcceptanceClaim {
        claim(self.permit.report())
    }
    fn dispatch<'a>(
        self: Box<Self>,
        issuer: &'a dyn ExactIssuer,
        cancel: CancellationSignal,
    ) -> HostFuture<'a, Result<AcceptanceDispatchOutcome, OperationError>> {
        Box::pin(async move {
            let Self {
                permit,
                store,
                binding,
                proof,
            } = *self;
            Ok(match permit.dispatch(issuer, cancel).await? {
                StockDispatchOutcome::Obtained { attempt, artifact } => {
                    AcceptanceDispatchOutcome::Obtained(claim(&attempt), artifact)
                }
                StockDispatchOutcome::Unissued(observation) => {
                    AcceptanceDispatchOutcome::Unissued(Box::new(Unissued {
                        observation,
                        store,
                        binding,
                        proof,
                    }))
                }
                StockDispatchOutcome::Unknown { attempt, reason } => {
                    AcceptanceDispatchOutcome::Unknown(claim(&attempt), reason)
                }
                StockDispatchOutcome::Rejected { attempt, rejection } => {
                    AcceptanceDispatchOutcome::Rejected(claim(&attempt), rejection)
                }
            })
        })
    }
}
fn select(value: AcceptanceSelection, recipient: PublicKey) -> StockClaimRequest {
    StockClaimRequest::new(
        value.binding,
        value.minimum,
        value.operation,
        value.digest,
        recipient,
    )
}
impl AcceptanceStore for NativeCommerce {
    fn signing_evidence<'a>(
        &'a self,
        selection: AcceptanceSelection,
        receipt: radroots_storage::inventory::StockEvidenceReceipt,
        proof: &'a AttachmentProof,
    ) -> HostFuture<
        'a,
        Result<Option<radroots_storage::inventory::StockEvidenceSource>, OperationError>,
    > {
        Box::pin(async move {
            let source = self
                .0
                .stock_signing_evidence(
                    select(selection, receipt.recipient()),
                    *receipt.digest(),
                    proof,
                )
                .await?;
            source
                .map(|source| {
                    if source.receipt() != receipt {
                        return Err(OperationError::RecoveryRequired);
                    }
                    source.with_json(|raw| {
                        radroots_storage::inventory::StockEvidenceSource::from_retained_wire(
                            receipt, raw,
                        )
                    })
                })
                .transpose()
        })
    }
    fn snapshot<'a>(
        &'a self,
        value: AcceptanceSelection,
        proof: &'a AttachmentProof,
    ) -> HostFuture<'a, Result<Box<dyn AcceptanceSnapshot>, OperationError>> {
        Box::pin(async move {
            let value = self
                .0
                .stock_operation_snapshot(
                    StockSnapshotRequest::new(
                        value.binding,
                        value.minimum,
                        value.operation,
                        value.digest,
                    ),
                    proof,
                )
                .await?;
            Ok(Box::new(Snapshot(value)) as Box<dyn AcceptanceSnapshot>)
        })
    }
    fn bind<'a>(
        &'a self,
        value: AcceptanceSelection,
        plan: radroots_signing::commerce::VerifiedCommerceSigningPlan,
        proof: &'a AttachmentProof,
    ) -> HostFuture<'a, Result<AcceptancePlanOutcome, OperationError>> {
        Box::pin(async move {
            match self
                .0
                .bind_stock_signing_intents(
                    StockSigningPlanRequest::new(value.binding, value.minimum, plan)?,
                    proof,
                )
                .await?
            {
                StockSigningPlanOutcome::Bound(value) => {
                    Ok(AcceptancePlanOutcome::Bound(value.attempt()))
                }
                StockSigningPlanOutcome::Unknown(value) => {
                    Ok(AcceptancePlanOutcome::Unknown(value))
                }
            }
        })
    }
    fn claim<'a>(
        &'a self,
        value: AcceptanceSelection,
        recipient: PublicKey,
        proof: &'a AttachmentProof,
    ) -> HostFuture<'a, Result<AcceptanceClaimOutcome, OperationError>> {
        Box::pin(async move {
            // Retain capture material before admission and before the issuer runs.
            let binding = value.binding.clone();
            let retained_proof =
                proof.with_secret(|bytes| AttachmentProof::from_protected_input(bytes))?;
            match self
                .0
                .claim_stock_signing_attempt(select(value, recipient), proof)
                .await?
            {
                StockClaimOutcome::Fresh(value) => {
                    Ok(AcceptanceClaimOutcome::Fresh(Box::new(Dispatch {
                        permit: value,
                        store: self.0.clone(),
                        binding,
                        proof: retained_proof,
                    })))
                }
                StockClaimOutcome::Existing(value) => {
                    Ok(AcceptanceClaimOutcome::Existing(claim(&value)))
                }
                StockClaimOutcome::Unknown(value) => Ok(AcceptanceClaimOutcome::Unknown(value)),
            }
        })
    }
    fn exact_replay<'a>(
        &'a self,
        value: AcceptanceSelection,
        recipient: PublicKey,
        call: ProviderCall,
        proof: &'a AttachmentProof,
    ) -> HostFuture<'a, Result<(ExactRecoveryRequest, AcceptanceClaim), OperationError>> {
        Box::pin(async move {
            let permit = self
                .0
                .authorize_stock_exact_replay(select(value, recipient), proof)
                .await?;
            let (request, report) = permit.into_recovery_request(call)?;
            Ok((request, claim(&report)))
        })
    }
    fn install<'a>(
        &'a self,
        value: AcceptanceSelection,
        proof: &'a AttachmentProof,
    ) -> HostFuture<'a, Result<AcceptanceInstallationOutcome, OperationError>> {
        Box::pin(async move {
            match self
                .0
                .install_stock_agreement(
                    StockInstallationRequest::new(
                        value.binding,
                        value.minimum,
                        value.operation,
                        value.digest,
                    ),
                    proof,
                )
                .await?
            {
                StockInstallationOutcome::Installed(value) => Ok(
                    AcceptanceInstallationOutcome::Installed(Box::new(installation(&value))),
                ),
                StockInstallationOutcome::Unknown(value) => {
                    Ok(AcceptanceInstallationOutcome::Unknown(value))
                }
            }
        })
    }
}
