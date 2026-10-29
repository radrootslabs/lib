//! One authenticated observation of original preparation and per-recipient S.
use super::*;
use inventory::{Turn, checkpoint};
use issuance::{claim, evidence::selection};
use radroots_domain::{CommandDigest, OperationId, PublicKey, SigningAttemptId, UnixSeconds};
use radroots_signing::issuance::IntentBinding;
use radroots_storage::{command::OperationError, inventory::InventoryCheckpointRef};

/// Original-operation selector data; it grants no read or execution permission.
pub struct StockSnapshotRequest {
    binding: WorkspaceBinding,
    minimum: InventoryCheckpointRef,
    operation: OperationId,
    digest: CommandDigest,
}
impl StockSnapshotRequest {
    pub const fn new(
        binding: WorkspaceBinding,
        minimum: InventoryCheckpointRef,
        operation: OperationId,
        digest: CommandDigest,
    ) -> Self {
        Self {
            binding,
            minimum,
            operation,
            digest,
        }
    }
}
impl std::fmt::Debug for StockSnapshotRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("StockSnapshotRequest(<redacted>)")
    }
}

/// Recorded signing facts, independent of installation or delivery success.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StockSigningProgress {
    Unbound,
    Bound,
    Unknown,
    Obtained,
    OriginalUnissued,
}
/// Immutable claim identity. This reference cannot construct a dispatch permit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StockClaimReference {
    id: SigningAttemptId,
    transaction: TransactionId,
    at: UnixSeconds,
    binding: IntentBinding,
}
impl StockClaimReference {
    pub const fn attempt_id(&self) -> SigningAttemptId {
        self.id
    }
    pub const fn transaction_id(&self) -> TransactionId {
        self.transaction
    }
    pub const fn claimed_at(&self) -> UnixSeconds {
        self.at
    }
    pub const fn binding(&self) -> IntentBinding {
        self.binding
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StockRecipientSnapshot {
    recipient: PublicKey,
    expected: Option<[u8; 32]>,
    claim: Option<StockClaimReference>,
    count: Sequence,
    first: Option<StockEvidenceReceipt>,
    unissued: Option<super::StockUnissuedReceipt>,
    replay: Option<super::StockReplayAdmissionReceipt>,
}
impl StockRecipientSnapshot {
    pub const fn recipient(&self) -> PublicKey {
        self.recipient
    }
    pub const fn expected_event_id(&self) -> Option<[u8; 32]> {
        self.expected
    }
    pub const fn claim(&self) -> Option<StockClaimReference> {
        self.claim
    }
    /// Number of distinct retained original encodings, not new commitments.
    pub const fn evidence_count(&self) -> Sequence {
        self.count
    }
    pub const fn first_evidence(&self) -> Option<StockEvidenceReceipt> {
        self.first
    }
    /// Authenticated original-invocation fact, independent of later S.
    pub const fn original_unissued(&self) -> Option<super::StockUnissuedReceipt> {
        self.unissued
    }
    /// First durable current-catalog replay admission, not invocation or S.
    /// Retained catalogs without this family return None.
    pub const fn replay_admission(&self) -> Option<super::StockReplayAdmissionReceipt> {
        self.replay
    }
    pub const fn progress(&self) -> StockSigningProgress {
        if self.first.is_some() {
            StockSigningProgress::Obtained
        } else if self.unissued.is_some() {
            StockSigningProgress::OriginalUnissued
        } else if self.claim.is_some() {
            StockSigningProgress::Unknown
        } else if self.expected.is_some() {
            StockSigningProgress::Bound
        } else {
            StockSigningProgress::Unbound
        }
    }
}
/// Coherent original T0/T1/claim/obtained view. No installation, release,
/// reissuance, delivery or global history-completeness claim follows from it.
pub struct StockOperationSnapshot {
    preparation: StockAcceptanceReceipt,
    plan: Option<StockBoundSigningPlan>,
    checkpoint: InventoryCheckpointRef,
    recipients: [StockRecipientSnapshot; 2],
    installation: Option<super::StockInstallationReceipt>,
    aborted: Option<super::StockAbortReceipt>,
    delivery: Option<Box<[radroots_storage::inventory::StockDeliverySummary; 2]>>,
}
impl StockOperationSnapshot {
    /// Coherent original per-recipient delivery progress; retained catalogs
    /// without stock D expose no supported delivery view.
    pub fn delivery(&self) -> Option<&[radroots_storage::inventory::StockDeliverySummary; 2]> {
        self.delivery.as_deref()
    }
    pub fn aborted(&self) -> Option<&super::StockAbortReceipt> {
        self.aborted.as_ref()
    }
    pub fn installation(&self) -> Option<&super::StockInstallationReceipt> {
        self.installation.as_ref()
    }
    pub const fn preparation(&self) -> &StockAcceptanceReceipt {
        &self.preparation
    }
    pub const fn signing_plan(&self) -> Option<&StockBoundSigningPlan> {
        self.plan.as_ref()
    }
    pub const fn checkpoint(&self) -> InventoryCheckpointRef {
        self.checkpoint
    }
    pub const fn recipients(&self) -> &[StockRecipientSnapshot; 2] {
        &self.recipients
    }
}
impl std::fmt::Debug for StockOperationSnapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("StockOperationSnapshot(<redacted>)")
    }
}

pub(in crate::workspace) async fn read(
    turn: &mut Turn<'_>,
    request: StockSnapshotRequest,
    proof: AttachmentProof,
) -> Result<StockOperationSnapshot, OperationError> {
    let auth = Box::pin(signing_plan::load(
        turn,
        &request.binding,
        request.minimum,
        request.operation,
        request.digest,
        &proof,
    ))
    .await?;
    Box::pin(read_authorized(turn, request, proof, auth)).await
}

// Keep authorization loading outside the projection's larger polling frame.
async fn read_authorized(
    turn: &mut Turn<'_>,
    request: StockSnapshotRequest,
    proof: AttachmentProof,
    auth: signing_plan::Authorized,
) -> Result<StockOperationSnapshot, OperationError> {
    if auth.access.authority.catalog < 11 {
        return Err(OperationError::UnsupportedOperation);
    }
    let mut reports = Vec::with_capacity(2);
    let mut claims = Vec::with_capacity(2);
    let mut refusals = Vec::with_capacity(2);
    let mut replays = Vec::with_capacity(2);
    let mut summaries = Vec::with_capacity(2);
    for recipient in auth.original.receipt.rumor().recipients() {
        let (retained, claim_proof) =
            Box::pin(claim::existing(turn, &auth, request.operation, *recipient)).await?;
        let reference = retained.as_ref().map(|value| StockClaimReference {
            id: value.report.attempt_id(),
            transaction: value.report.transaction_id(),
            at: value.report.claimed_at(),
            binding: value.report.binding(),
        });
        let summary = selection::Summary::read(
            turn.connection,
            turn.selected.participant(),
            request.operation,
            *recipient,
        )
        .await?;
        let first = if let Some(selected) = summary.first {
            Some(
                Box::pin(selection::open(
                    turn.connection,
                    turn.selected,
                    turn.crypto,
                    request.operation,
                    *recipient,
                    selected,
                ))
                .await?
                .value
                .receipt(),
            )
        } else {
            None
        };
        let expected = auth.plan.as_ref().and_then(|plan| {
            plan.bound
                .plan()
                .seals()
                .iter()
                .find(|seal| seal.recipient() == *recipient)
                .map(|seal| seal.event_id())
        });
        if reference.is_some() && expected.is_none()
            || first.is_some_and(|value| {
                reference.is_none_or(|claim| claim.id != value.signing_attempt_id())
            })
        {
            return Err(OperationError::RecoveryRequired);
        }
        let unissued = if auth.access.authority.catalog >= 14 {
            Box::pin(issuance::unissued::lookup(
                turn.connection,
                turn.selected,
                turn.crypto,
                request.operation,
                *recipient,
            ))
            .await?
        } else {
            None
        };
        if unissued
            .as_ref()
            .is_some_and(|v| reference.is_none_or(|c| c.id != v.receipt().signing_attempt_id()))
        {
            return Err(OperationError::RecoveryRequired);
        }
        let replay = if auth.access.authority.catalog >= 15 {
            Box::pin(issuance::replay_admission::lookup(
                turn.connection,
                turn.selected,
                turn.crypto,
                request.operation,
                *recipient,
            ))
            .await?
        } else {
            None
        };
        if replay
            .as_ref()
            .is_some_and(|v| reference.is_none_or(|c| c.id != v.receipt().signing_attempt_id()))
        {
            return Err(OperationError::RecoveryRequired);
        }
        reports.push(StockRecipientSnapshot {
            recipient: *recipient,
            expected,
            claim: reference,
            count: summary.count,
            first,
            unissued: unissued.as_ref().map(|v| v.receipt()),
            replay: replay.as_ref().map(|v| v.receipt()),
        });
        refusals.push(unissued);
        replays.push(replay);
        claims.push(claim_proof);
        summaries.push(summary);
    }
    let installation = if auth.access.authority.catalog >= 12 {
        Some(
            Box::pin(super::inventory::installation::existing(
                turn,
                &auth,
                request.operation,
            ))
            .await?,
        )
    } else {
        None
    };
    let aborted = if auth.access.authority.catalog >= 13 {
        Some(
            Box::pin(super::inventory::abort::existing(
                turn,
                &auth.access,
                request.operation,
            ))
            .await?,
        )
    } else {
        None
    };
    if aborted.as_ref().is_some_and(|(value, _)| value.is_some())
        && installation
            .as_ref()
            .is_some_and(|(value, _)| value.is_some())
    {
        return Err(OperationError::RecoveryRequired);
    }
    let current_order = if auth.access.authority.catalog >= 13 {
        Some(
            Box::pin(super::inventory::checkpoint::authenticate_order_state(
                turn.connection,
                turn.selected,
                turn.crypto,
                &auth.access.current,
                &auth.access.authority,
                auth.original.receipt.order_id(),
            ))
            .await?,
        )
    } else {
        None
    };
    let before = auth
        .access
        .authority
        .inventory
        .ok_or(OperationError::RecoveryRequired)?;
    let delivery = if matches!(auth.access.authority.catalog, 17..=19) {
        Some(Box::new(
            Box::pin(super::delivery::snapshot(
                turn,
                radroots_storage::inventory::StockEnvelopeRequest::new(
                    request.binding.clone(),
                    request.minimum,
                    request.operation,
                    request.digest,
                    auth.original.receipt.rumor().recipients()[0],
                    None,
                )?,
                &auth.access.authority,
            ))
            .await?,
        ))
    } else {
        None
    };
    let minimum = InventoryCheckpointRef::from_host_metadata(
        turn.selected.authority(),
        Sequence::new(0).map_err(|_| WorkspaceError::InvalidInput)?,
        None,
    )?;
    let witness = if current_order.is_none() {
        Some(
            Box::pin(checkpoint::authenticate_minimum(
                turn.connection,
                turn.selected,
                turn.crypto,
                &auth.access.current,
                &auth.access.authority,
                minimum,
            ))
            .await?,
        )
    } else {
        None
    };
    let mut tx = turn
        .connection
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|_| OperationError::StorageFailure)?;
    let checked: Result<(), OperationError> = async {
        if let Some(witness) = &witness {
            witness.confirm(&mut tx).await?;
        }
        if let Some(current_order) = &current_order {
            current_order.confirm(&mut tx).await?;
        }
        if let Some((retained, identity)) = &aborted {
            identity.confirm(&mut tx).await?;
            if let Some(retained) = retained {
                retained.confirm(&mut tx).await?;
            }
        }
        if let Some((retained, identity)) = &installation {
            identity.confirm(&mut tx).await?;
            if let Some(retained) = retained {
                retained.confirm(&mut tx).await?;
            }
        }
        for (index, report) in reports.iter().enumerate() {
            claims[index].confirm(&mut tx).await?;
            if let Some(value) = &replays[index] {
                value.confirm(&mut tx).await?;
            }
            if let Some(value) = &refusals[index] {
                value.confirm(&mut tx).await?;
            }
            summaries[index]
                .confirm(
                    &mut tx,
                    turn.selected.participant(),
                    request.operation,
                    report.recipient,
                )
                .await?;
        }
        Box::pin(auth.confirm(&mut tx, turn.namespace, turn.clock, &proof, false)).await?;
        Ok(())
    }
    .await;
    if tx.rollback().await.is_err() {
        *turn.uncertain = true;
        return Err(OperationError::RecoveryRequired);
    }
    checked?;
    let checkpoint = InventoryCheckpointRef::from_host_metadata(
        turn.selected.authority(),
        before.revision,
        before.head,
    )?
    .with_authorization(
        auth.access
            .authority
            .authorization
            .ok_or(OperationError::RecoveryRequired)?,
    );
    Ok(StockOperationSnapshot {
        delivery,
        aborted: aborted.and_then(|(retained, _)| retained.map(|value| value.receipt)),
        installation: installation.and_then(|(retained, _)| retained.map(|value| value.receipt)),
        preparation: auth.original.receipt,
        plan: auth.plan.map(|value| value.bound),
        checkpoint,
        recipients: reports
            .try_into()
            .map_err(|_| OperationError::RecoveryRequired)?,
    })
}
impl NativeWorkspace {
    pub async fn stock_operation_snapshot(
        &self,
        request: StockSnapshotRequest,
        proof: &AttachmentProof,
    ) -> Result<StockOperationSnapshot, OperationError> {
        self.worker
            .stock_operation_snapshot(
                request,
                proof.with_secret(|bytes| AttachmentProof::from_protected_input(bytes))?,
            )
            .await
    }
}
