//! Atomic T1 over the same protected stock ledger and connection owner as T0.
use super::*;
use inventory::{
    Access, Turn, acceptance,
    checkpoint::{self, EntryIdentity, IdentityTarget, MinimumProof},
};
use radroots_domain::{CheckpointDigest, CommandDigest, OperationId, UnixSeconds};
use radroots_protocol::commerce::CommerceSigningPlan;
use radroots_signing::commerce::VerifiedCommerceSigningPlan;
use radroots_storage::{
    command::{CommitAttempt, OperationError},
    inventory::InventoryCheckpointRef,
};
use sha2::{Digest, Sha256};
pub(in crate::workspace::stock) mod codec;
pub(super) use codec::read_entry;

/// A bounded candidate, with actual encryption and original route verification.
/// Only a confirmed host transaction can turn it into a bound plan.
pub struct StockSigningPlanRequest {
    binding: WorkspaceBinding,
    minimum: InventoryCheckpointRef,
    verified: VerifiedCommerceSigningPlan,
}
impl StockSigningPlanRequest {
    pub fn new(
        binding: WorkspaceBinding,
        minimum: InventoryCheckpointRef,
        verified: VerifiedCommerceSigningPlan,
    ) -> Result<Self, OperationError> {
        if verified.plan().protected_wire().len() > ledger::BODY_MAX_BYTES - codec::PREFIX_BYTES {
            return Err(WorkspaceError::Capacity.into());
        }
        Ok(Self {
            binding,
            minimum,
            verified,
        })
    }
    pub(in crate::workspace) fn attempt(&self) -> CommitAttempt {
        CommitAttempt::new(
            self.binding.input().workspace_id(),
            self.verified.plan().operation_id(),
            self.verified.plan().command_digest(),
            None,
        )
    }
}
impl std::fmt::Debug for StockSigningPlanRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("StockSigningPlanRequest(<redacted>)")
    }
}
/// Original complete plan read from a confirmed T1. This is not a dispatch
/// permission, a signer invocation, evidence capture or installed agreement.
pub struct StockBoundSigningPlan {
    attempt: CommitAttempt,
    preparation: StockAcceptanceReceipt,
    plan: CommerceSigningPlan,
    bound_at: UnixSeconds,
    checkpoint: InventoryCheckpointRef,
    intents: [[u8; 32]; 2],
}
impl StockBoundSigningPlan {
    /// Exact per-recipient signing intent commitments, in complete plan order.
    pub const fn intent_digests(&self) -> &[[u8; 32]; 2] {
        &self.intents
    }
    pub const fn attempt(&self) -> CommitAttempt {
        self.attempt
    }
    pub const fn preparation(&self) -> &StockAcceptanceReceipt {
        &self.preparation
    }
    pub const fn plan(&self) -> &CommerceSigningPlan {
        &self.plan
    }
    pub const fn bound_at(&self) -> UnixSeconds {
        self.bound_at
    }
    pub const fn checkpoint(&self) -> InventoryCheckpointRef {
        self.checkpoint
    }
}
impl std::fmt::Debug for StockBoundSigningPlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("StockBoundSigningPlan(<redacted>)")
    }
}
#[derive(Debug)]
pub enum StockSigningPlanOutcome {
    Bound(Box<StockBoundSigningPlan>),
    Unknown(CommitAttempt),
}

pub(in crate::workspace::stock) struct Authorized {
    pub(in crate::workspace::stock) access: Access,
    pub(in crate::workspace::stock) original: acceptance::codec::Retained,
    pub(in crate::workspace::stock) command: radroots_protocol::command::CapturedOrderAcceptance,
    pub(in crate::workspace::stock) original_proof: MinimumProof,
    pub(in crate::workspace::stock) consent: consent::Retained,
    pub(in crate::workspace::stock) consent_proof: MinimumProof,
    pub(in crate::workspace::stock) plan: Option<codec::Retained>,
    pub(in crate::workspace::stock) plan_proof: MinimumProof,
}
impl Authorized {
    pub(in crate::workspace::stock) async fn confirm(
        &self,
        connection: &mut SqliteConnection,
        namespace: &super::super::fs::Namespace,
        clock: &dyn WorkspaceClock,
        proof: &AttachmentProof,
        fresh: bool,
    ) -> Result<WorkspaceBinding, OperationError> {
        self.original_proof.confirm(connection).await?;
        self.consent_proof.confirm(connection).await?;
        self.plan_proof.confirm(connection).await?;
        self.original.confirm(connection).await?;
        if let Some(plan) = &self.plan {
            plan.confirm(connection).await?;
        }
        let current = self
            .access
            .confirm(connection, namespace, clock, proof)
            .await?;
        acceptance::check_consent(
            &self.access,
            &self.consent,
            &self.command,
            fresh,
            current.observed_at(),
        )?;
        if fresh
            && (current.observed_at() < self.original.receipt.prepared_at()
                || current.observed_at() >= self.original.receipt.first_issue_deadline())
        {
            return Err(OperationError::DeadlineExceeded);
        }
        Ok(current)
    }
}
pub(in crate::workspace::stock) async fn load(
    turn: &mut Turn<'_>,
    binding: &WorkspaceBinding,
    minimum: InventoryCheckpointRef,
    operation: OperationId,
    digest: CommandDigest,
    proof: &AttachmentProof,
) -> Result<Authorized, OperationError> {
    let access = Box::pin(turn.access(binding, minimum, proof)).await?;
    if !matches!(access.authority.catalog, 10..=19) {
        return Err(OperationError::UnsupportedOperation);
    }
    let original = Box::pin(acceptance::codec::read(
        turn.connection,
        turn.selected,
        turn.crypto,
        operation,
    ))
    .await?
    .ok_or(OperationError::StaleState)?;
    if original.frame.header.digest != digest {
        return Err(OperationError::IdempotencyConflict);
    }
    let original_proof = Box::pin(checkpoint::authenticate_identity(
        turn.connection,
        turn.selected,
        turn.crypto,
        &access.current,
        &access.authority,
        IdentityTarget::Operation(operation, Some(original.identity())),
    ))
    .await?;
    let command = original.command()?;
    let (consent, consent_proof) =
        Box::pin(acceptance::current_consent(turn, &access, &command)).await?;
    acceptance::check_consent(
        &access,
        &consent,
        &command,
        false,
        access.current.observed_at(),
    )?;
    for pool in original.receipt.held_pools() {
        if !consent.resources.contains(pool.id()) {
            return Err(OperationError::Unauthorized);
        }
        access
            .record
            .require(StockAction::ReadOperations, pool.id())?;
    }
    let id: Option<Vec<u8>> = sqlx::query_scalar("SELECT operation_id FROM stock_operations WHERE actor=? AND related_operation=? AND family=3 AND length(operation_id)=32")
        .bind(turn.selected.participant().as_bytes().as_slice()).bind(operation.as_bytes().as_slice()).fetch_optional(&mut *turn.connection).await.map_err(|_| OperationError::RecoveryRequired)?;
    let plan = if let Some(id) = id {
        Some(
            read_entry(
                turn.connection,
                turn.selected,
                turn.crypto,
                OperationId::from_bytes(
                    id.try_into()
                        .map_err(|_| OperationError::RecoveryRequired)?,
                ),
            )
            .await?
            .ok_or(OperationError::RecoveryRequired)?,
        )
    } else {
        None
    };
    let plan_proof = Box::pin(checkpoint::authenticate_identity(
        turn.connection,
        turn.selected,
        turn.crypto,
        &access.current,
        &access.authority,
        IdentityTarget::SigningPlan {
            operation,
            expected: plan.as_ref().map(codec::Retained::identity),
        },
    ))
    .await?;
    Ok(Authorized {
        access,
        original,
        command,
        original_proof,
        consent,
        consent_proof,
        plan,
        plan_proof,
    })
}
pub(in crate::workspace) async fn bind(
    turn: &mut Turn<'_>,
    request: StockSigningPlanRequest,
    proof: AttachmentProof,
) -> Result<StockSigningPlanOutcome, OperationError> {
    let plan = request.verified.plan();
    let auth = load(
        turn,
        &request.binding,
        request.minimum,
        plan.operation_id(),
        plan.command_digest(),
        &proof,
    )
    .await?;
    if let Some(original) = &auth.plan {
        if original.bound.plan.protected_wire() != plan.protected_wire() {
            return Err(OperationError::IdempotencyConflict);
        }
    } else {
        CommerceSigningPlan::parse(
            plan.operation_id(),
            plan.command_digest(),
            auth.original.receipt.rumor(),
            plan.protected_wire(),
        )
        .map_err(|_| OperationError::InvalidEvidence)?;
        if request.verified.deadline_unix_ms()
            != auth
                .original
                .receipt
                .first_issue_deadline()
                .get()
                .checked_mul(1000)
                .ok_or(OperationError::RecoveryRequired)?
        {
            return Err(OperationError::InvalidCommand);
        }
        acceptance::check_consent(
            &auth.access,
            &auth.consent,
            &auth.command,
            true,
            auth.access.current.observed_at(),
        )?;
    }
    if auth.plan.is_some() {
        confirm_read(turn, &auth, &proof).await?;
        return Ok(StockSigningPlanOutcome::Bound(Box::new(
            auth.plan.ok_or(OperationError::RecoveryRequired)?.bound,
        )));
    }
    let current_order = Box::pin(super::order_state::signing_state(
        turn,
        &auth.access,
        auth.original.receipt.order_id(),
        plan.operation_id(),
        false,
    ))
    .await?;
    let backing = Box::pin(inventory::backing::read(
        turn,
        &auth.access,
        auth.original.receipt.held_pools(),
    ))
    .await?;
    let before = auth
        .access
        .authority
        .inventory
        .ok_or(OperationError::RecoveryRequired)?;
    let revision = Sequence::new(
        before
            .revision
            .get()
            .checked_add(1)
            .ok_or(WorkspaceError::Capacity)?,
    )
    .map_err(|_| WorkspaceError::Capacity)?;
    let bound_at = auth.access.current.observed_at();
    let transaction = TransactionId::from_bytes(super::super::random()?);
    let entry = OperationId::from_bytes(super::super::random()?);
    let plan = request.verified.into_plan();
    let intents = codec::intents(&plan, auth.original.receipt.first_issue_deadline())?;
    let body = codec::encode(&auth.original, &plan, bound_at)?;
    let mut header = ledger::Header {
        version: 5,
        family: ledger::Family::SigningPlan,
        namespace: SellerAuthoritySelection::from_attachment(turn.selected),
        actor: auth.access.current.actor(),
        operation: entry,
        digest: plan.command_digest(),
        transaction,
        revision,
        previous: before.head,
        head: CheckpointDigest::from_bytes([0; 32]),
        pool: None,
        order: Some(auth.original.receipt.order_id()),
        related_operation: Some(plan.operation_id()),
        subject_grant: None,
        bytes: 0,
        parts: 0,
        body_digest: [0; 32],
    };
    header.head = codec::head(&header, &body);
    let checkpoint = InventoryCheckpointRef::from_host_metadata(
        turn.selected.authority(),
        revision,
        Some(header.head),
    )?
    .with_optional_authorization(auth.access.authority.authorization);
    let attempt = CommitAttempt::new(
        turn.selected.workspace_id(),
        plan.operation_id(),
        plan.command_digest(),
        Some(transaction),
    );
    let head = header.head;
    let framed = ledger::prepare(turn.crypto, turn.selected, header, &body).await?;
    let mut next = auth.access.authority.clone();
    next.inventory = Some(InventoryCheckpoint {
        revision,
        head: Some(head),
        ..before
    });
    let root = turn
        .crypto
        .seal_stock_root(turn.selected, next.catalog, &next.encode()?)
        .await?;
    let mut tx = turn
        .connection
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|_| OperationError::StorageFailure)?;
    // Do not borrow the connection again while the transaction is live.
    let changed: Result<(), OperationError> = async {
        auth.original_proof.confirm(&mut tx).await?; auth.consent_proof.confirm(&mut tx).await?; auth.plan_proof.confirm(&mut tx).await?; auth.original.confirm(&mut tx).await?;
        let current = auth.access.confirm(&mut tx, turn.namespace, turn.clock, &proof).await?;
        for pool in &backing { pool.confirm(&mut tx).await?; }
        if let Some(current_order) = &current_order {current_order.confirm(&mut tx).await?;}
        acceptance::check_consent(&auth.access, &auth.consent, &auth.command, true, current.observed_at())?;
        if current.observed_at() < bound_at || current.observed_at() >= auth.original.receipt.first_issue_deadline() { return Err(OperationError::DeadlineExceeded); }
        sqlx::query("INSERT INTO stock_operations(actor,grant_id,operation_id,command_digest,transaction_id,pool_id,revision,chain_head,protected,family,order_id,related_operation,subject_grant) VALUES(?,?,?,?,?,NULL,?,?,?,3,?,?,NULL)")
            .bind(auth.access.current.actor().participant().as_bytes().as_slice()).bind(auth.access.current.input().grant_id().as_bytes().as_slice()).bind(entry.as_bytes().as_slice()).bind(plan.command_digest().as_bytes().as_slice()).bind(transaction.as_bytes().as_slice()).bind(revision.get() as i64).bind(head.as_bytes().as_slice()).bind(&framed.protected).bind(auth.original.receipt.order_id().as_bytes().as_slice()).bind(plan.operation_id().as_bytes().as_slice()).execute(&mut *tx).await.map_err(|_| OperationError::StorageFailure)?;
        framed.insert_parts(&mut tx, auth.access.current.actor(), entry).await?;
        let updated = sqlx::query("UPDATE stock_authority SET protected=? WHERE singleton=1 AND protected=?").bind(root).bind(&auth.access.root).execute(&mut *tx).await.map_err(|_| OperationError::StorageFailure)?;
        if updated.rows_affected() != 1 { return Err(OperationError::RecoveryRequired); }
        turn.namespace.validate()?; Ok(())
    }.await;
    if let Err(error) = changed {
        if tx.rollback().await.is_err() {
            *turn.uncertain = true;
            return Ok(StockSigningPlanOutcome::Unknown(attempt));
        }
        return Err(error);
    }
    if tx.commit().await.is_err() {
        *turn.uncertain = true;
        return Ok(StockSigningPlanOutcome::Unknown(attempt));
    }
    Ok(StockSigningPlanOutcome::Bound(Box::new(
        StockBoundSigningPlan {
            attempt,
            intents,
            preparation: auth.original.receipt,
            plan,
            bound_at,
            checkpoint,
        },
    )))
}
async fn confirm_read(
    turn: &mut Turn<'_>,
    auth: &Authorized,
    proof: &AttachmentProof,
) -> Result<(), OperationError> {
    let mut tx = turn
        .connection
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|_| OperationError::StorageFailure)?;
    let checked: Result<(), OperationError> = async {
        auth.original_proof.confirm(&mut tx).await?;
        auth.consent_proof.confirm(&mut tx).await?;
        auth.plan_proof.confirm(&mut tx).await?;
        auth.original.confirm(&mut tx).await?;
        if let Some(plan) = &auth.plan {
            plan.confirm(&mut tx).await?;
        }
        let current = auth
            .access
            .confirm(&mut tx, turn.namespace, turn.clock, proof)
            .await?;
        acceptance::check_consent(
            &auth.access,
            &auth.consent,
            &auth.command,
            false,
            current.observed_at(),
        )?;
        Ok(())
    }
    .await;
    if tx.rollback().await.is_err() {
        *turn.uncertain = true;
        return Err(OperationError::RecoveryRequired);
    }
    checked
}
pub(in crate::workspace) async fn read(
    turn: &mut Turn<'_>,
    binding: WorkspaceBinding,
    minimum: InventoryCheckpointRef,
    operation: OperationId,
    digest: CommandDigest,
    proof: AttachmentProof,
) -> Result<Option<StockBoundSigningPlan>, OperationError> {
    let auth = load(turn, &binding, minimum, operation, digest, &proof).await?;
    confirm_read(turn, &auth, &proof).await?;
    Ok(auth.plan.map(|value| value.bound))
}
impl NativeWorkspace {
    pub async fn bind_stock_signing_intents(
        &self,
        request: StockSigningPlanRequest,
        proof: &AttachmentProof,
    ) -> Result<StockSigningPlanOutcome, OperationError> {
        self.worker
            .bind_stock_signing_intents(
                request,
                proof.with_secret(|bytes| AttachmentProof::from_protected_input(bytes))?,
            )
            .await
    }
    pub async fn stock_signing_plan(
        &self,
        binding: WorkspaceBinding,
        minimum: InventoryCheckpointRef,
        operation: OperationId,
        digest: CommandDigest,
        proof: &AttachmentProof,
    ) -> Result<Option<StockBoundSigningPlan>, OperationError> {
        self.worker
            .stock_signing_plan(
                binding,
                minimum,
                operation,
                digest,
                proof.with_secret(|bytes| AttachmentProof::from_protected_input(bytes))?,
            )
            .await
    }
}
