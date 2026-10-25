//! First-dispatch linearization. Existing claims never create a new permit.
use super::super::{
    inventory::{
        Turn, acceptance,
        checkpoint::{self, EntryIdentity, IdentityTarget},
    },
    signing_plan,
};
use super::*;
use radroots_signing::{
    issuance::{IntentBinding, SigningIntent},
    provider::ProviderCall,
    request::CancellationSignal,
};
use std::{num::NonZeroU64, sync::Arc};
pub(in crate::workspace::stock) mod codec;
pub(in crate::workspace::stock) use codec::read_entry;

/// Captured selector data, without permission or provider capability evidence.
pub struct StockClaimRequest {
    pub(super) binding: WorkspaceBinding,
    pub(super) minimum: InventoryCheckpointRef,
    pub(super) operation: OperationId,
    pub(super) digest: CommandDigest,
    pub(super) recipient: PublicKey,
}
impl StockClaimRequest {
    pub const fn new(
        binding: WorkspaceBinding,
        minimum: InventoryCheckpointRef,
        operation: OperationId,
        digest: CommandDigest,
        recipient: PublicKey,
    ) -> Self {
        Self {
            binding,
            minimum,
            operation,
            digest,
            recipient,
        }
    }
    pub(in crate::workspace) fn attempt(&self) -> CommitAttempt {
        CommitAttempt::new(
            self.binding.input().workspace_id(),
            self.operation,
            self.digest,
            None,
        )
    }
}
impl std::fmt::Debug for StockClaimRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("StockClaimRequest(<redacted>)")
    }
}

/// An original durable claim. This report provides no fresh dispatch permit.
#[derive(Clone)]
pub struct StockSigningAttempt {
    bound: Arc<StockBoundSigningPlan>,
    recipient: PublicKey,
    entry: EntryIdentity,
    transaction: TransactionId,
    claimed_at: UnixSeconds,
    binding: IntentBinding,
}
impl StockSigningAttempt {
    pub fn attempt_id(&self) -> SigningAttemptId {
        SigningAttemptId::from_bytes(*self.entry.operation.as_bytes())
    }
    pub const fn transaction_id(&self) -> TransactionId {
        self.transaction
    }
    pub const fn claimed_at(&self) -> UnixSeconds {
        self.claimed_at
    }
    pub const fn recipient(&self) -> PublicKey {
        self.recipient
    }
    pub const fn binding(&self) -> IntentBinding {
        self.binding
    }
    pub fn bound_plan(&self) -> &StockBoundSigningPlan {
        &self.bound
    }
    pub fn attempt(&self) -> CommitAttempt {
        CommitAttempt::new(
            self.bound.attempt().workspace_id(),
            self.bound.plan().operation_id(),
            self.bound.plan().command_digest(),
            Some(self.transaction),
        )
    }
}
impl std::fmt::Debug for StockSigningAttempt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("StockSigningAttempt(<redacted>)")
    }
}

/// Single-use confirmed first-dispatch boundary, distinct from actual provider
/// capability and destination policy. Those remain caller obligations before
/// invoking the signer. An original/recovered claim cannot construct this type.
pub struct StockDispatchPermit {
    report: StockSigningAttempt,
}
impl StockDispatchPermit {
    pub const fn report(&self) -> &StockSigningAttempt {
        &self.report
    }
    /// Consumes this first-dispatch permit to expose the unchanged exact intent.
    /// The returned report anchors later capture; it cannot reissue a permit.
    pub fn into_signing_intent(
        self,
        cancellation: CancellationSignal,
    ) -> Result<(SigningIntent, StockSigningAttempt), OperationError> {
        let intent = intent(&self.report.bound, self.report.recipient, cancellation)?;
        if intent.binding() != self.report.binding {
            return Err(OperationError::RecoveryRequired);
        }
        Ok((intent, self.report))
    }
}
impl std::fmt::Debug for StockDispatchPermit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("StockDispatchPermit(<redacted>)")
    }
}
#[derive(Debug)]
pub enum StockClaimOutcome {
    Fresh(StockDispatchPermit),
    Existing(StockSigningAttempt),
    Unknown(CommitAttempt),
}
pub(in crate::workspace::stock) fn intent(
    bound: &StockBoundSigningPlan,
    recipient: PublicKey,
    cancellation: CancellationSignal,
) -> Result<SigningIntent, OperationError> {
    let seal = bound
        .plan()
        .seals()
        .iter()
        .find(|seal| seal.recipient() == recipient)
        .ok_or(OperationError::InvalidCommand)?;
    let deadline = NonZeroU64::new(
        bound
            .preparation()
            .first_issue_deadline()
            .get()
            .checked_mul(1000)
            .ok_or(OperationError::RecoveryRequired)?,
    )
    .ok_or(OperationError::RecoveryRequired)?;
    SigningIntent::from_commerce_seal(
        seal,
        ProviderCall::new(seal.fields().author, deadline, cancellation),
    )
    .map_err(|_| OperationError::InvalidEvidence)
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Body {
    version: u8,
    operation: OperationId,
    plan_entry: OperationId,
    plan_transaction: TransactionId,
    plan_head: radroots_domain::CheckpointDigest,
    plan_digest: [u8; 32],
    recipient: PublicKey,
    intent: [u8; 32],
    deadline: UnixSeconds,
    claimed_at: UnixSeconds,
}

pub(in crate::workspace::stock) async fn existing(
    turn: &mut Turn<'_>,
    auth: &signing_plan::Authorized,
    operation: OperationId,
    recipient: PublicKey,
) -> Result<(Option<codec::Retained>, checkpoint::MinimumProof), OperationError> {
    let id:Option<Vec<u8>>=sqlx::query_scalar("SELECT CASE WHEN length(entry_id)=32 THEN entry_id END FROM stock_signing_claims WHERE actor=? AND operation_id=? AND recipient=?")
        .bind(turn.selected.participant().as_bytes().as_slice()).bind(operation.as_bytes().as_slice()).bind(recipient.as_bytes().as_slice()).fetch_optional(&mut *turn.connection).await.map_err(|_|OperationError::RecoveryRequired)?;
    let retained = if let Some(id) = id {
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
    let witness = checkpoint::authenticate_identity(
        turn.connection,
        turn.selected,
        turn.crypto,
        &auth.access.current,
        &auth.access.authority,
        IdentityTarget::SigningClaim {
            operation,
            recipient,
            expected: retained.as_ref().map(codec::Retained::identity),
        },
    )
    .await?;
    Ok((retained, witness))
}
async fn confirm_read(
    turn: &mut Turn<'_>,
    auth: &signing_plan::Authorized,
    witness: &checkpoint::MinimumProof,
    retained: Option<&codec::Retained>,
    proof: &AttachmentProof,
) -> Result<(), OperationError> {
    let mut tx = turn
        .connection
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|_| OperationError::StorageFailure)?;
    let checked: Result<(), OperationError> = async {
        witness.confirm(&mut tx).await?;
        if let Some(value) = retained {
            value.confirm(&mut tx).await?;
        }
        auth.confirm(&mut tx, turn.namespace, turn.clock, proof, false)
            .await?;
        Ok(())
    }
    .await;
    if tx.rollback().await.is_err() {
        *turn.uncertain = true;
        return Err(OperationError::RecoveryRequired);
    }
    checked
}
pub(in crate::workspace) async fn claim(
    turn: &mut Turn<'_>,
    request: StockClaimRequest,
    proof: AttachmentProof,
) -> Result<StockClaimOutcome, OperationError> {
    let mut auth = signing_plan::load(
        turn,
        &request.binding,
        request.minimum,
        request.operation,
        request.digest,
        &proof,
    )
    .await?;
    if auth.access.authority.catalog != 11 {
        return Err(OperationError::UnsupportedOperation);
    }
    let plan = auth.plan.as_ref().ok_or(OperationError::StaleState)?;
    let selected = intent(&plan.bound, request.recipient, CancellationSignal::new())?;
    let (retained, witness) = existing(turn, &auth, request.operation, request.recipient).await?;
    if let Some(retained) = retained {
        confirm_read(turn, &auth, &witness, Some(&retained), &proof).await?;
        return Ok(StockClaimOutcome::Existing(retained.report));
    }
    let at = auth.access.current.observed_at();
    acceptance::check_consent(&auth.access, &auth.consent, &auth.command, true, at)?;
    if at < plan.bound.bound_at() || at >= plan.bound.preparation().first_issue_deadline() {
        return Err(OperationError::DeadlineExceeded);
    }
    let backing = Box::pin(super::super::inventory::backing::read(
        turn,
        &auth.access,
        auth.original.receipt.held_pools(),
    ))
    .await?;
    let entry = OperationId::from_bytes(super::super::super::random()?);
    let transaction = TransactionId::from_bytes(super::super::super::random()?);
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
    let body = Body {
        version: 1,
        operation: request.operation,
        plan_entry: plan.frame.header.operation,
        plan_transaction: plan
            .bound
            .attempt()
            .transaction_id()
            .ok_or(OperationError::RecoveryRequired)?,
        plan_head: plan.frame.header.head,
        plan_digest: *plan.bound.plan().digest(),
        recipient: request.recipient,
        intent: *selected.binding().digest(),
        deadline: plan.bound.preparation().first_issue_deadline(),
        claimed_at: at,
    };
    let clear = zeroize::Zeroizing::new(
        serde_json::to_vec(&body).map_err(|_| OperationError::StorageFailure)?,
    );
    if clear.len() > 16 * 1024 {
        return Err(WorkspaceError::Capacity.into());
    }
    let mut header = ledger::Header {
        version: 5,
        family: ledger::Family::SigningClaim,
        namespace: SellerAuthoritySelection::from_attachment(turn.selected),
        actor: auth.access.current.actor(),
        operation: entry,
        digest: request.digest,
        transaction,
        revision,
        previous: before.head,
        head: radroots_domain::CheckpointDigest::from_bytes([0; 32]),
        pool: None,
        order: Some(plan.bound.preparation().order_id()),
        related_operation: Some(request.operation),
        subject_grant: None,
        bytes: 0,
        parts: 0,
        body_digest: [0; 32],
    };
    header.head = codec::head(&header, &clear);
    let head = header.head;
    let framed = ledger::prepare(turn.crypto, turn.selected, header, &clear).await?;
    let entry_identity = EntryIdentity {
        operation: entry,
        revision,
        head,
        protected_digest: Sha256::digest(&framed.protected).into(),
    };
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
    let anchor = CommitAttempt::new(
        turn.selected.workspace_id(),
        request.operation,
        request.digest,
        Some(transaction),
    );
    let mut tx = turn
        .connection
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|_| OperationError::StorageFailure)?;
    let changed:Result<(),OperationError>=async {
        witness.confirm(&mut tx).await?;
        let current=auth.confirm(&mut tx,turn.namespace,turn.clock,&proof,true).await?;
        for pool in &backing { pool.confirm(&mut tx).await?; }
        if current.observed_at()<at {return Err(WorkspaceError::Unavailable.into());}
        framed.insert_record(&mut tx).await?;
        sqlx::query("INSERT INTO stock_signing_claims(actor,operation_id,recipient,entry_id) VALUES(?,?,?,?)").bind(turn.selected.participant().as_bytes().as_slice()).bind(request.operation.as_bytes().as_slice()).bind(request.recipient.as_bytes().as_slice()).bind(entry.as_bytes().as_slice()).execute(&mut *tx).await.map_err(|_|OperationError::StorageFailure)?;
        let updated=sqlx::query("UPDATE stock_authority SET protected=? WHERE singleton=1 AND protected=?").bind(root).bind(&auth.access.root).execute(&mut *tx).await.map_err(|_|OperationError::StorageFailure)?;
        if updated.rows_affected()!=1 {return Err(OperationError::RecoveryRequired);} turn.namespace.validate()?;Ok(())
    }.await;
    if let Err(error) = changed {
        if tx.rollback().await.is_err() {
            *turn.uncertain = true;
            return Ok(StockClaimOutcome::Unknown(anchor));
        }
        return Err(error);
    }
    if tx.commit().await.is_err() {
        *turn.uncertain = true;
        return Ok(StockClaimOutcome::Unknown(anchor));
    }
    let bound = Arc::new(
        auth.plan
            .take()
            .ok_or(OperationError::RecoveryRequired)?
            .bound,
    );
    Ok(StockClaimOutcome::Fresh(StockDispatchPermit {
        report: StockSigningAttempt {
            bound,
            recipient: request.recipient,
            entry: entry_identity,
            transaction,
            claimed_at: at,
            binding: selected.binding(),
        },
    }))
}
pub(in crate::workspace) async fn read(
    turn: &mut Turn<'_>,
    request: StockClaimRequest,
    proof: AttachmentProof,
) -> Result<Option<StockSigningAttempt>, OperationError> {
    let auth = signing_plan::load(
        turn,
        &request.binding,
        request.minimum,
        request.operation,
        request.digest,
        &proof,
    )
    .await?;
    if auth.access.authority.catalog != 11 {
        return Err(OperationError::UnsupportedOperation);
    }
    let plan = auth.plan.as_ref().ok_or(OperationError::StaleState)?;
    intent(&plan.bound, request.recipient, CancellationSignal::new())?;
    let (retained, witness) = existing(turn, &auth, request.operation, request.recipient).await?;
    confirm_read(turn, &auth, &witness, retained.as_ref(), &proof).await?;
    Ok(retained.map(|value| value.report))
}
impl NativeWorkspace {
    pub async fn claim_stock_signing_attempt(
        &self,
        request: StockClaimRequest,
        proof: &AttachmentProof,
    ) -> Result<StockClaimOutcome, OperationError> {
        self.worker
            .claim_stock_signing_attempt(
                request,
                proof.with_secret(|bytes| AttachmentProof::from_protected_input(bytes))?,
            )
            .await
    }
    pub async fn stock_signing_attempt(
        &self,
        request: StockClaimRequest,
        proof: &AttachmentProof,
    ) -> Result<Option<StockSigningAttempt>, OperationError> {
        self.worker
            .stock_signing_attempt(
                request,
                proof.with_secret(|bytes| AttachmentProof::from_protected_input(bytes))?,
            )
            .await
    }
}
