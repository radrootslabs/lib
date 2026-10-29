//! One shared T1/S/T2/D algorithm; target hosts implement transactions only.
use super::*;
use crate::{
    Error, OperationContext,
    session::{
        Shared, SignerSession,
        command::{CommandRuntime, RunMode, SigningResources},
    },
};
use alloc::vec::Vec;
use core::num::NonZeroU64;
use radroots_domain::UnixSeconds;
use radroots_protocol::{
    commerce::{CommerceSeal, EncodedCommerceRumor},
    intent::SigningReplay,
};
use radroots_signing::{
    issuance::{ExactRecoveryCapability, ExactRecoveryKind, SignedArtifact, SigningIntent},
    provider::PrivateText,
    recovery::ReplayCapability,
};
use radroots_storage::inventory::{InventoryStore, StockEvidenceOutcome};
#[path = "commerce_delivery.rs"]
mod delivery;

fn same(attempt: CommitAttempt, selection: &AcceptanceSelection) -> bool {
    attempt.workspace_id() == selection.binding.input().workspace_id()
        && attempt.operation_id() == selection.operation
        && attempt.command_digest() == selection.digest
}
pub(crate) fn observe(
    selection: &AcceptanceSelection,
    snapshot: &dyn AcceptanceSnapshot,
) -> Result<AcceptanceStatus, OperationError> {
    let original = snapshot.preparation();
    let recipients = snapshot.recipients();
    if !same(original.attempt(), selection)
        || original.attempt().transaction_id().is_none()
        || recipients
            .iter()
            .map(|v| v.recipient)
            .ne(original.rumor().recipients().iter().copied())
    {
        return Err(OperationError::RecoveryRequired);
    }
    if let Some(plan) = snapshot.plan() {
        CommerceSigningPlan::parse(
            selection.operation,
            selection.digest,
            original.rumor(),
            plan.protected_wire(),
        )
        .map_err(|_| OperationError::RecoveryRequired)?;
    } else if recipients
        .iter()
        .any(|v| v.claim.is_some() || v.evidence.is_some() || v.original_unissued.is_some())
    {
        return Err(OperationError::RecoveryRequired);
    }
    for recipient in &recipients {
        if let Some(claim) = recipient.claim {
            let plan = snapshot.plan().ok_or(OperationError::RecoveryRequired)?;
            let seal = plan
                .seals()
                .iter()
                .find(|v| v.recipient() == recipient.recipient)
                .ok_or(OperationError::RecoveryRequired)?;
            if claim.binding
                != SigningIntent::from_commerce_seal(seal, call(original)?)
                    .map_err(|_| OperationError::RecoveryRequired)?
                    .binding()
            {
                return Err(OperationError::RecoveryRequired);
            }
        }
        if let Some(evidence) = recipient.evidence
            && (!same(evidence.attempt(), selection)
                || evidence.attempt().transaction_id().is_none()
                || evidence.recipient() != recipient.recipient
                || recipient
                    .claim
                    .is_none_or(|v| v.id != evidence.signing_attempt_id()))
        {
            return Err(OperationError::RecoveryRequired);
        }
    }
    for recipient in &recipients {
        if let Some(value) = recipient.original_unissued
            && (!same(value.attempt, selection)
                || value.attempt.transaction_id().is_none()
                || value.recipient != recipient.recipient
                || recipient.claim.is_none_or(|v| v.id != value.claim)
                || value.checkpoint.authority() != selection.minimum.authority())
        {
            return Err(OperationError::RecoveryRequired);
        }
    }
    let installation = snapshot.installation();
    if snapshot.aborted()
        && recipients
            .iter()
            .any(|v| v.evidence.is_some() || (v.claim.is_some() && v.original_unissued.is_none()))
    {
        return Err(OperationError::RecoveryRequired);
    }
    if let Some(installed) = installation {
        let peer = recipients
            .iter()
            .find(|v| v.recipient != original.rumor().author())
            .ok_or(OperationError::RecoveryRequired)?;
        if !same(installed.attempt, selection)
            || installed.attempt.transaction_id().is_none()
            || installed.order != original.order_id()
            || installed.record != original.rumor().record_id()
            || peer.evidence.is_none()
            || snapshot.aborted()
        {
            return Err(OperationError::RecoveryRequired);
        }
    }
    Ok(AcceptanceStatus {
        delivery: match snapshot.delivery() {
            Some(values) => {
                for (value, recipient) in values.iter().zip(&recipients) {
                    if value.checkpoint() != snapshot.checkpoint()
                        || value.source().is_some_and(|source| {
                            !same(source.attempt(), selection)
                                || source.recipient() != recipient.recipient
                                || recipient.evidence != Some(source.evidence())
                                || installation.is_none_or(|installed| {
                                    Some(source.installation_transaction_id())
                                        != installed.attempt.transaction_id()
                                })
                        })
                    {
                        return Err(OperationError::RecoveryRequired);
                    }
                }
                Some(alloc::boxed::Box::new(values.clone()))
            }
            None => None,
        },
        attempt: original.attempt(),
        record: original.rumor().record_id(),
        checkpoint: snapshot.checkpoint(),
        recipients,
        plan_bound: snapshot.plan().is_some(),
        installation,
        aborted: snapshot.aborted(),
    })
}
fn call(original: &StockAcceptanceReceipt) -> Result<ProviderCall, OperationError> {
    let deadline = original
        .first_issue_deadline()
        .get()
        .checked_mul(1000)
        .and_then(NonZeroU64::new)
        .ok_or(OperationError::RecoveryRequired)?;
    Ok(ProviderCall::new(
        original.rumor().author(),
        deadline,
        CancellationSignal::new(),
    ))
}
fn copy(proof: &AttachmentProof) -> Result<AttachmentProof, OperationError> {
    proof
        .with_secret(|bytes| AttachmentProof::from_protected_input(bytes))
        .map_err(Into::into)
}
impl CommandRuntime {
    pub(in crate::session) async fn run_acceptance(
        &self,
        store: Shared<dyn AcceptanceStore>,
        inventory: Shared<dyn InventoryStore>,
        mut selection: AcceptanceSelection,
        proof: &AttachmentProof,
        resources: SigningResources,
        mode: RunMode,
    ) -> crate::ExecutionResult<AcceptanceStatus> {
        let retained = OperationContext::retained(selection.operation);
        let fail = |e| crate::ExecutionFailure::from(Error::operation(e, retained));
        let mut snapshot = store
            .snapshot(selection.clone(), proof)
            .await
            .map_err(fail)?;
        let mut status = observe(&selection, snapshot.as_ref()).map_err(fail)?;
        if !snapshot.aborted()
            && snapshot.installation().is_none()
            && snapshot.recipients().iter().any(|v| {
                v.recipient != snapshot.preparation().rumor().author() && v.evidence.is_some()
            })
        {
            selection.minimum = snapshot.checkpoint();
            match store
                .install(selection.clone(), proof)
                .await
                .map_err(fail)?
            {
                AcceptanceInstallationOutcome::Installed(value)
                    if same(value.attempt, &selection)
                        && value.attempt.transaction_id().is_some() => {}
                _ => return Err(fail(OperationError::RecoveryRequired)),
            }
            snapshot = store
                .snapshot(selection.clone(), proof)
                .await
                .map_err(fail)?;
            status = observe(&selection, snapshot.as_ref()).map_err(fail)?;
        }
        if snapshot.aborted() || !self.io.enabled() {
            return Ok(status);
        }
        self.drain().await?;
        resources.stock_captures.ensure_ready().map_err(fail)?;
        if snapshot.plan().is_none() {
            let Some(signer) = resources.signer.as_ref() else {
                return Ok(status);
            };
            let plan = self
                .acceptance_plan(snapshot.preparation(), signer)
                .await
                .map_err(fail)?;
            selection.minimum = snapshot.checkpoint();
            match store
                .bind(selection.clone(), plan, proof)
                .await
                .map_err(fail)?
            {
                AcceptancePlanOutcome::Bound(anchor)
                    if same(anchor, &selection) && anchor.transaction_id().is_some() => {}
                _ => return Err(fail(OperationError::RecoveryRequired)),
            }
        }
        let author = snapshot.preparation().rumor().author();
        let peer = *snapshot
            .preparation()
            .rumor()
            .recipients()
            .iter()
            .find(|v| **v != author)
            .ok_or_else(|| fail(OperationError::RecoveryRequired))?;
        drop(snapshot);
        // A peer commitment is installed before self-history work can fail.
        for recipient in [peer, author] {
            let current = store
                .snapshot(selection.clone(), proof)
                .await
                .map_err(fail)?;
            observe(&selection, current.as_ref()).map_err(fail)?;
            if current.aborted() {
                return observe(&selection, current.as_ref()).map_err(fail);
            }
            selection.minimum = current.checkpoint();
            let facts = current
                .recipients()
                .into_iter()
                .find(|v| v.recipient == recipient)
                .ok_or_else(|| fail(OperationError::RecoveryRequired))?;
            if facts.evidence.is_none() {
                let Some(signer) = resources.signer.as_ref() else {
                    continue;
                };
                resources.stock_captures.ensure_ready().map_err(fail)?;
                let plan = current
                    .plan()
                    .ok_or_else(|| fail(OperationError::RecoveryRequired))?;
                if facts.claim.is_some()
                    && (!matches!(&mode, RunMode::Resume)
                        || plan.replay() != SigningReplay::LocalReplaySafe)
                {
                    continue;
                }
                if facts.claim.is_none()
                    && self.io.now().map_err(fail)? >= current.preparation().first_issue_deadline()
                {
                    continue;
                }
                // Recheck every originally selected target and the actual scoped
                // provider before acquiring any fresh first-dispatch claim.
                for route in plan.routes() {
                    self.io.targets(route).map_err(fail)?;
                }
                let original = current.preparation();
                let plan_copy = CommerceSigningPlan::parse(
                    selection.operation,
                    selection.digest,
                    original.rumor(),
                    plan.protected_wire(),
                )
                .map_err(|_| fail(OperationError::RecoveryRequired))?;
                signer
                    .verify_commerce_plan(
                        call(original).map_err(fail)?,
                        original.rumor(),
                        plan_copy,
                    )
                    .await
                    .map_err(|_| fail(OperationError::Unauthorized))?;
                let capture_proof = copy(proof).map_err(fail)?;
                let capture_selection = selection.clone();
                let obtained: Option<(SignedArtifact, AcceptanceClaim)> = if let Some(claim) =
                    facts.claim
                {
                    if !matches!(&mode, RunMode::Resume)
                        || plan.replay() != SigningReplay::LocalReplaySafe
                    {
                        None
                    } else {
                        let (request, report) = store
                            .exact_replay(
                                selection.clone(),
                                recipient,
                                call(original).map_err(fail)?,
                                proof,
                            )
                            .await
                            .map_err(fail)?;
                        if report != claim || request.original().binding() != claim.binding {
                            return Err(fail(OperationError::RecoveryRequired));
                        }
                        self.ensure_ready()?;
                        match signer.recover_exact(request).await.into_parts().1 {
                            ExactRecoveryKind::Obtained(artifact) => Some((artifact, report)),
                            ExactRecoveryKind::Rejected(rejected) => {
                                return Err(crate::ExecutionFailure::rejected(
                                    selection.operation,
                                    rejected,
                                ));
                            }
                            _ => None,
                        }
                    }
                } else {
                    if self.io.now().map_err(fail)? >= original.first_issue_deadline() {
                        continue;
                    }
                    match store
                        .claim(selection.clone(), recipient, proof)
                        .await
                        .map_err(fail)?
                    {
                        AcceptanceClaimOutcome::Fresh(permit) => {
                            let report = permit.claim();
                            let seal = plan
                                .seals()
                                .iter()
                                .find(|v| v.recipient() == recipient)
                                .ok_or_else(|| fail(OperationError::RecoveryRequired))?;
                            let expected = SigningIntent::from_commerce_seal(
                                seal,
                                call(original).map_err(fail)?,
                            )
                            .map_err(|_| fail(OperationError::RecoveryRequired))?
                            .binding();
                            if report.binding != expected {
                                return Err(fail(OperationError::RecoveryRequired));
                            }
                            self.ensure_ready()?;
                            resources.stock_captures.ensure_ready().map_err(fail)?;
                            match permit
                                .dispatch(signer, CancellationSignal::new())
                                .await
                                .map_err(fail)?
                            {
                                AcceptanceDispatchOutcome::Obtained(actual, artifact) => {
                                    if actual != report {
                                        resources.stock_captures.retain_obtained(
                                            crate::session::stock_evidence::ObtainedContext {
                                                selection: capture_selection,
                                                claim: report,
                                                recipient,
                                                store: inventory.clone(),
                                                proof: capture_proof,
                                                lease: resources.lease.clone(),
                                            },
                                            artifact,
                                        );
                                        return Err(fail(OperationError::RecoveryRequired));
                                    }
                                    Some((artifact, report))
                                }
                                AcceptanceDispatchOutcome::Unissued(observation) => {
                                    let pending = resources.stock_captures.retain_unissued(
                                        capture_selection.clone(),
                                        report,
                                        recipient,
                                        observation,
                                        resources.lease.clone(),
                                    );
                                    if !matches!(
                                        resources.stock_captures.capture_unissued(&pending).await?,
                                        AcceptanceUnissuedOutcome::Committed(_)
                                    ) {
                                        return Err(fail(OperationError::RecoveryRequired));
                                    }
                                    None
                                }
                                AcceptanceDispatchOutcome::Rejected(actual, rejected)
                                    if actual == report =>
                                {
                                    return Err(crate::ExecutionFailure::rejected(
                                        selection.operation,
                                        rejected,
                                    ));
                                }
                                AcceptanceDispatchOutcome::Unknown(actual, _)
                                    if actual == report =>
                                {
                                    None
                                }
                                _ => return Err(fail(OperationError::RecoveryRequired)),
                            }
                        }
                        AcceptanceClaimOutcome::Existing(_) => None,
                        AcceptanceClaimOutcome::Unknown(_) => {
                            return Err(fail(OperationError::RecoveryRequired));
                        }
                    }
                };
                if let Some((artifact, claim)) = obtained {
                    let pending = resources.stock_captures.retain_obtained(
                        crate::session::stock_evidence::ObtainedContext {
                            selection: capture_selection,
                            claim,
                            recipient,
                            store: inventory.clone(),
                            proof: capture_proof,
                            lease: resources.lease.clone(),
                        },
                        artifact,
                    );
                    if !matches!(
                        resources
                            .stock_captures
                            .capture_obtained(&pending)
                            .await
                            .map_err(|e| e.with_context(retained))?,
                        StockEvidenceOutcome::Committed(_)
                    ) {
                        return Err(fail(OperationError::RecoveryRequired));
                    }
                }
            }
            if recipient == peer {
                let captured = store
                    .snapshot(selection.clone(), proof)
                    .await
                    .map_err(fail)?;
                observe(&selection, captured.as_ref()).map_err(fail)?;
                let peer_ready = captured
                    .recipients()
                    .iter()
                    .any(|v| v.recipient == peer && v.evidence.is_some());
                if peer_ready && captured.installation().is_none() {
                    selection.minimum = captured.checkpoint();
                    match store
                        .install(selection.clone(), proof)
                        .await
                        .map_err(fail)?
                    {
                        AcceptanceInstallationOutcome::Installed(value)
                            if same(value.attempt, &selection)
                                && value.attempt.transaction_id().is_some() => {}
                        _ => return Err(fail(OperationError::RecoveryRequired)),
                    }
                }
            }
            self.deliver_acceptance(
                &store, &inventory, &selection, proof, &resources, recipient, &mode,
            )
            .await?;
        }
        let final_view = store
            .snapshot(selection.clone(), proof)
            .await
            .map_err(fail)?;
        observe(&selection, final_view.as_ref()).map_err(fail)
    }
    async fn acceptance_plan(
        &self,
        original: &StockAcceptanceReceipt,
        signer: &SignerSession,
    ) -> Result<VerifiedCommerceSigningPlan, OperationError> {
        let rumor: &EncodedCommerceRumor = original.rumor();
        let now: UnixSeconds = self.io.now()?;
        if now < original.prepared_at() || now >= original.first_issue_deadline() {
            return Err(OperationError::DeadlineExceeded);
        }
        let state = signer
            .observe()
            .await
            .map_err(|_| OperationError::Unauthorized)?;
        if state.identity() != rumor.author() {
            return Err(OperationError::Unauthorized);
        }
        let replay = match state.exact_issuance() {
            Some(_) if state.exact_recovery() == ExactRecoveryCapability::LocalExactReplay => {
                SigningReplay::LocalReplaySafe
            }
            Some(ReplayCapability::NonReplayable) => SigningReplay::NonReplayable,
            Some(ReplayCapability::ExactReplayByRequestId) => SigningReplay::ExactReplayByRequestId,
            _ => return Err(OperationError::UnsupportedOperation),
        };
        let mut routes = Vec::with_capacity(2);
        for recipient in rumor.recipients() {
            let route = self
                .io
                .route(*recipient, call(original)?.deadline_unix_ms())
                .await?;
            self.io.targets(&route)?;
            routes.push(route);
        }
        let mut seals = Vec::with_capacity(2);
        for recipient in rumor.recipients() {
            let plaintext = PrivateText::new(rumor.protected_wire())
                .map_err(|_| OperationError::InvalidCommand)?;
            let ciphertext = signer
                .encrypt_private(call(original)?, *recipient, plaintext)
                .await
                .map_err(|_| OperationError::Unauthorized)?;
            seals.push(
                CommerceSeal::new(
                    original.attempt().operation_id(),
                    original.attempt().command_digest(),
                    rumor,
                    *recipient,
                    self.io.seal_time(now)?,
                    ciphertext.as_str(),
                )
                .map_err(|_| OperationError::InvalidEvidence)?,
            );
        }
        let plan = CommerceSigningPlan::new(
            original.attempt().operation_id(),
            original.attempt().command_digest(),
            rumor,
            seals
                .try_into()
                .map_err(|_| OperationError::RecoveryRequired)?,
            routes
                .try_into()
                .map_err(|_| OperationError::RecoveryRequired)?,
            replay,
        )
        .map_err(|_| OperationError::InvalidEvidence)?;
        signer
            .verify_commerce_plan(call(original)?, rumor, plan)
            .await
            .map_err(|_| OperationError::InvalidEvidence)
    }
}
