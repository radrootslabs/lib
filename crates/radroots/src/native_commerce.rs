//! Mechanical views of the same selected native stock owner for Core workflows.
use radroots_domain::PublicKey;
use radroots_execution::session::{
    HostFuture,
    commerce::{
        AcceptanceClaim, AcceptanceClaimOutcome, AcceptanceDispatch, AcceptanceInstallationOutcome,
        AcceptancePlanOutcome, AcceptanceRecipient, AcceptanceSelection, AcceptanceSnapshot,
        AcceptanceStore, InstalledAcceptance,
    },
};
use radroots_signing::{
    host::Shared,
    issuance::{ExactRecoveryRequest, SigningIntent},
    provider::ProviderCall,
    request::CancellationSignal,
};
use radroots_storage::{
    command::OperationError,
    inventory::{InventoryCheckpointRef, StockAcceptanceReceipt},
    workspace::AttachmentProof,
};
use radroots_storage_sqlite::workspace::{
    NativeWorkspace, StockClaimOutcome, StockClaimRequest, StockDispatchPermit,
    StockInstallationOutcome, StockInstallationReceipt, StockInstallationRequest,
    StockOperationSnapshot, StockSigningAttempt, StockSigningPlanOutcome, StockSigningPlanRequest,
    StockSnapshotRequest,
};

pub(crate) struct NativeCommerce(pub(crate) Shared<NativeWorkspace>);
struct Snapshot(StockOperationSnapshot);
struct Dispatch(StockDispatchPermit);
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
        })
    }
    fn installation(&self) -> Option<InstalledAcceptance> {
        self.0.installation().map(installation)
    }
    fn aborted(&self) -> bool {
        self.0.aborted().is_some()
    }
}
impl AcceptanceDispatch for Dispatch {
    fn into_intent(
        self: Box<Self>,
        cancel: CancellationSignal,
    ) -> Result<(SigningIntent, AcceptanceClaim), OperationError> {
        let (intent, report) = self.0.into_signing_intent(cancel)?;
        Ok((intent, claim(&report)))
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
            match self
                .0
                .claim_stock_signing_attempt(select(value, recipient), proof)
                .await?
            {
                StockClaimOutcome::Fresh(value) => {
                    Ok(AcceptanceClaimOutcome::Fresh(Box::new(Dispatch(value))))
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
