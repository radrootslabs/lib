//! Qualified local replay authorization over an existing protected claim.
use super::super::{
    inventory::{Turn, acceptance},
    order_state,
};
use super::{
    StockClaimRequest, StockReplayAdmissionReceipt, StockSigningAttempt, claim, replay_admission,
    signing_plan,
};
use crate::workspace::{AttachmentProof, NativeWorkspace, WorkspaceError};
use radroots_domain::UnixSeconds;
use radroots_protocol::intent::SigningReplay;
use radroots_signing::{
    issuance::{ExactRecoveryRequest, SigningIntent},
    provider::ProviderCall,
    request::CancellationSignal,
};
use radroots_storage::command::OperationError;
use sqlx::Connection;

/// Single-use current authorization for qualified exact local replay only.
/// This is never a new first-dispatch claim, proof of issuance or nonissuance.
pub struct StockExactReplayPermit {
    report: StockSigningAttempt,
    at: UnixSeconds,
    admission: Option<StockReplayAdmissionReceipt>,
}
impl StockExactReplayPermit {
    /// Confirmed first replay admission in tracked catalogs15–16. Retained13/14
    /// permits are explicitly untracked and return None; no report authorizes
    /// stock release, a fresh first dispatch or another replay invocation.
    pub const fn admission(&self) -> Option<StockReplayAdmissionReceipt> {
        self.admission
    }

    /// Borrows the original protected attempt; this report cannot mint a permit.
    pub const fn report(&self) -> &StockSigningAttempt {
        &self.report
    }
    /// Returns the final checked workspace authorization observation.
    pub const fn authorized_at(&self) -> UnixSeconds {
        self.at
    }
    /// Transfers exact original intent data to the separately qualified provider.
    /// The owner must retain obtained S through durable capture after observer
    /// loss. The actual local adapter checks both original and call windows.
    pub fn into_recovery_request(
        self,
        call: ProviderCall,
    ) -> Result<(ExactRecoveryRequest, StockSigningAttempt), OperationError> {
        let intent: SigningIntent = claim::intent(
            self.report.bound_plan(),
            self.report.recipient(),
            CancellationSignal::new(),
        )?;
        if intent.binding() != self.report.binding() {
            return Err(OperationError::RecoveryRequired);
        }
        let request =
            ExactRecoveryRequest::new(intent, call).map_err(|_| OperationError::InvalidCommand)?;
        Ok((request, self.report))
    }
}
impl core::fmt::Debug for StockExactReplayPermit {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("StockExactReplayPermit(<redacted>)")
    }
}

pub(in crate::workspace) async fn authorize(
    turn: &mut Turn<'_>,
    request: StockClaimRequest,
    proof: AttachmentProof,
) -> Result<StockExactReplayPermit, OperationError> {
    let auth = Box::pin(signing_plan::load(
        turn,
        &request.binding,
        request.minimum,
        request.operation,
        request.digest,
        &proof,
    ))
    .await?;
    if !matches!(auth.access.authority.catalog, 13..=17) {
        return Err(OperationError::UnsupportedOperation);
    }
    let plan = auth.plan.as_ref().ok_or(OperationError::StaleState)?;
    // Later provider capabilities cannot upgrade the retained T1 policy.
    if plan.bound.plan().replay() != SigningReplay::LocalReplaySafe {
        return Err(OperationError::UnsupportedOperation);
    }
    let intent = claim::intent(&plan.bound, request.recipient, CancellationSignal::new())?;
    let (retained, witness) = Box::pin(claim::existing(
        turn,
        &auth,
        request.operation,
        request.recipient,
    ))
    .await?;
    let retained = retained.ok_or(OperationError::StaleState)?;
    if intent.binding() != retained.report.binding() {
        return Err(OperationError::RecoveryRequired);
    }
    let at = auth.access.current.observed_at();
    acceptance::check_consent(&auth.access, &auth.consent, &auth.command, true, at)?;
    if at < plan.bound.bound_at()
        || at < retained.report.claimed_at()
        || at >= plan.bound.preparation().first_issue_deadline()
    {
        return Err(OperationError::DeadlineExceeded);
    }
    let order = Box::pin(order_state::signing_state(
        turn,
        &auth.access,
        auth.original.receipt.order_id(),
        request.operation,
        true,
    ))
    .await?;
    let backing = Box::pin(super::super::inventory::backing::read(
        turn,
        &auth.access,
        auth.original.receipt.held_pools(),
    ))
    .await?;
    let pending = if auth.access.authority.catalog >= 15 {
        Some(Box::pin(replay_admission::prepare(turn, &auth, &retained, at)).await?)
    } else {
        None
    };
    let mut tx = turn
        .connection
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|_| OperationError::StorageFailure)?;
    let checked: Result<UnixSeconds, OperationError> = async {
        Box::pin(witness.confirm(&mut tx)).await?;
        retained.confirm(&mut tx).await?;
        if let Some(order) = &order {
            order.confirm(&mut tx).await?;
        }
        for pool in &backing {
            pool.confirm(&mut tx).await?;
        }
        let current =
            Box::pin(auth.confirm(&mut tx, turn.namespace, turn.clock, &proof, true)).await?;
        if current.observed_at() < at || current.observed_at() < retained.report.claimed_at() {
            return Err(WorkspaceError::Unavailable.into());
        }
        if let Some(pending) = &pending {
            pending.apply(&mut tx, turn.selected, &auth).await?;
        }
        turn.namespace.validate()?;
        Ok(current.observed_at())
    }
    .await;
    let authorized_at = match checked {
        Ok(value) => value,
        Err(error) => {
            if tx.rollback().await.is_err() {
                *turn.uncertain = true;
                return Err(OperationError::RecoveryRequired);
            }
            return Err(error);
        }
    };
    // A current fresh marker must be committed before a permit can escape.
    // Existing markers and retained13/14 authorization require no new write.
    let done = if pending.as_ref().is_some_and(|v| v.writes()) {
        tx.commit().await
    } else {
        tx.rollback().await
    };
    if done.is_err() {
        *turn.uncertain = true;
        return Err(OperationError::RecoveryRequired);
    }
    Ok(StockExactReplayPermit {
        report: retained.report,
        at: authorized_at,
        admission: pending.as_ref().map(|v| v.receipt()),
    })
}

impl NativeWorkspace {
    /// Authenticates current first-capable local replay rights for an existing
    /// original claim. Unsupported/expired/recovery-only state dispatches nothing.
    pub async fn authorize_stock_exact_replay(
        &self,
        request: StockClaimRequest,
        proof: &AttachmentProof,
    ) -> Result<StockExactReplayPermit, OperationError> {
        self.worker
            .authorize_stock_exact_replay(
                request,
                proof.with_secret(|bytes| AttachmentProof::from_protected_input(bytes))?,
            )
            .await
    }
}
