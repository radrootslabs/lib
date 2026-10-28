//! Initial T2 installation. Original preparation and issuance remain immutable.
use super::super::{issuance::evidence::selection, ledger, signing_plan};
use super::*;
use checkpoint::{EntryIdentity, IdentityTarget};
use radroots_domain::{OrderId, PublicKey, RecordId, stock::StockTicks};
mod codec;
mod projection;
pub(in crate::workspace::stock) use codec::{Retained, read_entry};
pub(in crate::workspace::stock) use projection::{Projection, raw_projection};

/// Captured original-operation selector; does not establish permission or evidence.
pub struct StockInstallationRequest {
    binding: WorkspaceBinding,
    minimum: radroots_storage::inventory::InventoryCheckpointRef,
    operation: OperationId,
    digest: CommandDigest,
}
impl StockInstallationRequest {
    pub const fn new(
        binding: WorkspaceBinding,
        minimum: radroots_storage::inventory::InventoryCheckpointRef,
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
    pub(in crate::workspace) fn attempt(&self) -> CommitAttempt {
        CommitAttempt::new(
            self.binding.input().workspace_id(),
            self.operation,
            self.digest,
            None,
        )
    }
}
impl std::fmt::Debug for StockInstallationRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("StockInstallationRequest(<redacted>)")
    }
}

/// Immutable recipient work. An absent artifact remains waiting for original
/// issuance evidence; installation does not claim wrapping or relay delivery.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct StockPublicationWork {
    recipient: PublicKey,
    seal: [u8; 32],
    evidence: Option<[u8; 32]>,
}
impl StockPublicationWork {
    pub const fn recipient(self) -> PublicKey {
        self.recipient
    }
    pub const fn seal_id(&self) -> &[u8; 32] {
        &self.seal
    }
    pub const fn evidence_digest(&self) -> Option<&[u8; 32]> {
        self.evidence.as_ref()
    }
}
impl std::fmt::Debug for StockPublicationWork {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("StockPublicationWork(<redacted>)")
    }
}

/// Confirmed initial agreement and exact per-recipient work. This is business
/// installation evidence, independent of signing dispatch and delivery status.
#[derive(Clone)]
pub struct StockInstallationReceipt {
    attempt: CommitAttempt,
    entry: EntryIdentity,
    order: OrderId,
    record: RecordId,
    at: UnixSeconds,
    checkpoint: radroots_storage::inventory::InventoryCheckpointRef,
    pools: Vec<PoolSnapshot>,
    work: [StockPublicationWork; 2],
}
impl StockInstallationReceipt {
    pub const fn attempt(&self) -> CommitAttempt {
        self.attempt
    }
    pub const fn order_id(&self) -> OrderId {
        self.order
    }
    pub const fn record_id(&self) -> RecordId {
        self.record
    }
    pub const fn installed_at(&self) -> UnixSeconds {
        self.at
    }
    pub const fn checkpoint(&self) -> radroots_storage::inventory::InventoryCheckpointRef {
        self.checkpoint
    }
    pub fn installed_pools(&self) -> &[PoolSnapshot] {
        &self.pools
    }
    pub const fn publication_work(&self) -> &[StockPublicationWork; 2] {
        &self.work
    }
}
impl std::fmt::Debug for StockInstallationReceipt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("StockInstallationReceipt(<redacted>)")
    }
}
#[derive(Debug)]
pub enum StockInstallationOutcome {
    Installed(Box<StockInstallationReceipt>),
    Unknown(CommitAttempt),
}
#[derive(Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EvidenceRef {
    entry: OperationId,
    revision: Sequence,
    digest: [u8; 32],
}
#[derive(Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Work {
    recipient: PublicKey,
    intent: [u8; 32],
    seal: [u8; 32],
    #[serde(deserialize_with = "super::required_nullable")]
    evidence: Option<EvidenceRef>,
}
#[derive(Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Effect {
    before: PoolSnapshot,
    after: PoolSnapshot,
    quantity: StockTicks,
    movement: [u8; 32],
}
fn after(before: PoolSnapshot, quantity: StockTicks) -> Result<PoolSnapshot, OperationError> {
    let balances = before
        .balances()
        .install_preparatory(quantity)
        .map_err(|_| OperationError::RecoveryRequired)?;
    let version = Sequence::new(
        before
            .version()
            .get()
            .checked_add(1)
            .ok_or(WorkspaceError::Capacity)?,
    )
    .map_err(|_| WorkspaceError::Capacity)?;
    Ok(PoolSnapshot::new(
        before.id(),
        before.unit(),
        balances,
        version,
    ))
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Body {
    version: u8,
    operation: OperationId,
    plan_entry: OperationId,
    plan_head: CheckpointDigest,
    record: RecordId,
    installed_at: UnixSeconds,
    authorization: radroots_storage::workspace::seller_stock::StockAuthorizationRef,
    effects: Vec<Effect>,
    work: [Work; 2],
}
pub(in crate::workspace::stock) async fn existing(
    turn: &mut Turn<'_>,
    auth: &signing_plan::Authorized,
    operation: OperationId,
) -> Result<(Option<codec::Retained>, MinimumProof), OperationError> {
    let id: Option<Vec<u8>>=sqlx::query_scalar("SELECT CASE WHEN length(entry_id)=32 THEN entry_id END FROM stock_agreements WHERE actor=? AND operation_id=?").bind(turn.selected.participant().as_bytes().as_slice()).bind(operation.as_bytes().as_slice()).fetch_optional(&mut *turn.connection).await.map_err(|_|OperationError::RecoveryRequired)?;
    let retained = match id {
        Some(id) => Some(
            Box::pin(read_entry(
                turn.connection,
                turn.selected,
                turn.crypto,
                OperationId::from_bytes(
                    id.try_into()
                        .map_err(|_| OperationError::RecoveryRequired)?,
                ),
            ))
            .await?
            .ok_or(OperationError::RecoveryRequired)?,
        ),
        None => None,
    };
    let proof = checkpoint::authenticate_identity(
        turn.connection,
        turn.selected,
        turn.crypto,
        &auth.access.current,
        &auth.access.authority,
        IdentityTarget::Installation {
            operation,
            expected: retained.as_ref().map(|value| value.receipt.entry),
        },
    )
    .await?;
    Ok((retained, proof))
}
pub(in crate::workspace) async fn install(
    turn: &mut Turn<'_>,
    request: StockInstallationRequest,
    proof: AttachmentProof,
) -> Result<StockInstallationOutcome, OperationError> {
    let auth = Box::pin(signing_plan::load(
        turn,
        &request.binding,
        request.minimum,
        request.operation,
        request.digest,
        &proof,
    ))
    .await?;
    if !matches!(auth.access.authority.catalog, 12..=16) {
        return Err(OperationError::UnsupportedOperation);
    }
    let (old, identity) = Box::pin(existing(turn, &auth, request.operation)).await?;
    if let Some(old) = old {
        Box::pin(confirm_read(turn, &auth, &identity, Some(&old), &proof)).await?;
        return Ok(StockInstallationOutcome::Installed(Box::new(old.receipt)));
    }
    let plan = auth.plan.as_ref().ok_or(OperationError::StaleState)?;
    if auth.access.current.observed_at() < plan.bound.bound_at() {
        return Err(WorkspaceError::Unavailable.into());
    }
    let current_order = if auth.access.authority.catalog >= 13 {
        let checked = Box::pin(super::checkpoint::authenticate_order_state(
            turn.connection,
            turn.selected,
            turn.crypto,
            &auth.access.current,
            &auth.access.authority,
            plan.bound.preparation().order_id(),
        ))
        .await?;
        if checked.witness.expected.is_none_or(|value| {
            value.phase != super::super::order_state::Phase::Prepared
                || value.operation != request.operation
        }) {
            return Err(OperationError::StaleState);
        }
        Some(checked)
    } else {
        None
    };
    let recipients = *plan.bound.preparation().rumor().recipients();
    let mut selections = Vec::with_capacity(2);
    let mut work = Vec::with_capacity(2);
    let mut peer = false;
    for (index, recipient) in recipients.into_iter().enumerate() {
        let summary = selection::Summary::read(
            turn.connection,
            turn.selected.participant(),
            request.operation,
            recipient,
        )
        .await?;
        let selected = if let Some(selected) = summary.first {
            let obtained = Box::pin(selection::open(
                turn.connection,
                turn.selected,
                turn.crypto,
                request.operation,
                recipient,
                selected,
            ))
            .await?;
            if recipient != turn.selected.participant() {
                peer = true;
            }
            Some(EvidenceRef {
                entry: obtained.frame.header.operation,
                revision: obtained.frame.header.revision,
                digest: selected.digest,
            })
        } else {
            None
        };
        work.push(Work {
            recipient,
            intent: plan.bound.intent_digests()[index],
            seal: plan.bound.plan().seals()[index].event_id(),
            evidence: selected,
        });
        selections.push((recipient, summary));
    }
    if !peer {
        return Err(OperationError::InvalidEvidence);
    }
    let backing = Box::pin(super::backing::read(
        turn,
        &auth.access,
        auth.original.receipt.held_pools(),
    ))
    .await?;
    let allocations = auth.original.held_allocations()?;
    let mut originals = Vec::with_capacity(allocations.len());
    let mut effects = Vec::with_capacity(allocations.len());
    for (pool, quantity) in allocations {
        let original = read_pool(turn.connection, turn.selected, turn.crypto, pool)
            .await?
            .ok_or(OperationError::RecoveryRequired)?;
        effects.push(Effect {
            before: original.pool,
            after: after(original.pool, quantity)?,
            quantity,
            movement: super::super::super::random()?,
        });
        originals.push(original);
    }
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
    let transaction = TransactionId::from_bytes(super::super::super::random()?);
    let entry = OperationId::from_bytes(super::super::super::random()?);
    let body = Body {
        version: 1,
        operation: request.operation,
        plan_entry: plan.frame.header.operation,
        plan_head: plan.frame.header.head,
        record: plan.bound.preparation().rumor().record_id(),
        installed_at: auth.access.current.observed_at(),
        authorization: auth
            .access
            .authority
            .authorization
            .ok_or(OperationError::RecoveryRequired)?,
        effects,
        work: work
            .try_into()
            .map_err(|_| OperationError::RecoveryRequired)?,
    };
    let clear = encode(&body)?;
    let mut header = ledger::Header {
        version: 5,
        family: ledger::Family::Installation,
        namespace: SellerAuthoritySelection::from_attachment(turn.selected),
        actor: auth.access.current.actor(),
        operation: entry,
        digest: request.digest,
        transaction,
        revision,
        previous: before.head,
        head: CheckpointDigest::from_bytes([0; 32]),
        pool: None,
        order: Some(auth.original.receipt.order_id()),
        related_operation: Some(request.operation),
        subject_grant: None,
        bytes: 0,
        parts: 0,
        body_digest: [0; 32],
    };
    header.head = codec::head(&header, &clear);
    let projection = Projection::new(&header, &body)?;
    let head = header.head;
    let framed = ledger::prepare(turn.crypto, turn.selected, header, &clear).await?;
    let receipt = codec::receipt(turn.selected, &framed.header, &body, &framed.protected)?;
    let mut pool_bytes = Vec::with_capacity(body.effects.len());
    let mut movement_bytes = Vec::with_capacity(body.effects.len());
    for (effect, original) in body.effects.iter().zip(&originals) {
        pool_bytes.push(
            turn.crypto
                .seal_stock_record(
                    turn.selected,
                    "pool",
                    effect.after.id().as_bytes(),
                    &encode(&PoolWire {
                        version: 1,
                        label: original.view.label(),
                        pool: effect.after,
                    })?,
                )
                .await?,
        );
        movement_bytes.push(
            turn.crypto
                .seal_stock_record(
                    turn.selected,
                    "installation_movement",
                    &effect.movement,
                    &encode(&codec::Movement {
                        version: 1,
                        operation: entry,
                        digest: request.digest,
                        record: body.record,
                        effect: *effect,
                    })?,
                )
                .await?,
        );
    }
    let movements = Sequence::new(
        before
            .movements
            .get()
            .checked_add(body.effects.len() as u64)
            .ok_or(WorkspaceError::Capacity)?,
    )
    .map_err(|_| WorkspaceError::Capacity)?;
    let mut next = auth.access.authority.clone();
    next.inventory = Some(InventoryCheckpoint {
        revision,
        head: Some(head),
        movements,
        ..before
    });
    let root = turn
        .crypto
        .seal_stock_root(turn.selected, next.catalog, &next.encode()?)
        .await?;
    let anchor = receipt.attempt;
    let mut tx = turn
        .connection
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|_| OperationError::StorageFailure)?;
    let changed:Result<(),OperationError>=async {
        identity.confirm(&mut tx).await?;
        let current=auth.confirm(&mut tx,turn.namespace,turn.clock,&proof,false).await?;
        if current.observed_at()<body.installed_at {return Err(WorkspaceError::Unavailable.into());}
        for backing in &backing {backing.confirm(&mut tx).await?;}
        for (recipient,summary) in &selections {summary.confirm(&mut tx,turn.selected.participant(),request.operation,*recipient).await?;}
        framed.insert_record(&mut tx).await?;
        projection.insert(&mut tx,turn.selected.participant(),entry).await?;
        for (index,(effect,original)) in body.effects.iter().zip(&originals).enumerate() {
            let changed=sqlx::query("UPDATE stock_pools SET revision=?,protected=? WHERE pool_id=? AND revision=? AND protected=?").bind(effect.after.version().get() as i64).bind(&pool_bytes[index]).bind(effect.before.id().as_bytes().as_slice()).bind(effect.before.version().get() as i64).bind(&original.protected).execute(&mut *tx).await.map_err(|_|OperationError::StorageFailure)?;
            if changed.rows_affected()!=1 {return Err(OperationError::StaleState);}
            sqlx::query("INSERT INTO stock_movements(movement_id,actor,grant_id,operation_id,pool_id,protected) VALUES(?,?,?,?,?,?)").bind(effect.movement.as_slice()).bind(turn.selected.participant().as_bytes().as_slice()).bind(current.input().grant_id().as_bytes().as_slice()).bind(entry.as_bytes().as_slice()).bind(effect.before.id().as_bytes().as_slice()).bind(&movement_bytes[index]).execute(&mut *tx).await.map_err(|_|OperationError::StorageFailure)?;
        }
        if let Some(current_order) = &current_order {
            current_order.advance(&mut tx, super::super::order_state::Marker::from_header(&framed.header, super::super::order_state::Phase::Installed)?).await?;
        }
        let changed=sqlx::query("UPDATE stock_authority SET protected=? WHERE singleton=1 AND protected=?").bind(root).bind(&auth.access.root).execute(&mut *tx).await.map_err(|_|OperationError::StorageFailure)?;
        if changed.rows_affected()!=1 {return Err(OperationError::RecoveryRequired);}turn.namespace.validate()?;Ok(())
    }.await;
    if let Err(error) = changed {
        if tx.rollback().await.is_err() {
            *turn.uncertain = true;
            return Ok(StockInstallationOutcome::Unknown(anchor));
        }
        return Err(error);
    }
    if tx.commit().await.is_err() {
        *turn.uncertain = true;
        return Ok(StockInstallationOutcome::Unknown(anchor));
    }
    Ok(StockInstallationOutcome::Installed(Box::new(receipt)))
}
pub(in crate::workspace) async fn read(
    turn: &mut Turn<'_>,
    request: StockInstallationRequest,
    proof: AttachmentProof,
) -> Result<Option<StockInstallationReceipt>, OperationError> {
    let auth = Box::pin(signing_plan::load(
        turn,
        &request.binding,
        request.minimum,
        request.operation,
        request.digest,
        &proof,
    ))
    .await?;
    if !matches!(auth.access.authority.catalog, 12..=16) {
        return Err(OperationError::UnsupportedOperation);
    }
    let (retained, identity) = Box::pin(existing(turn, &auth, request.operation)).await?;
    Box::pin(confirm_read(
        turn,
        &auth,
        &identity,
        retained.as_ref(),
        &proof,
    ))
    .await?;
    Ok(retained.map(|value| value.receipt))
}
async fn confirm_read(
    turn: &mut Turn<'_>,
    auth: &signing_plan::Authorized,
    identity: &MinimumProof,
    retained: Option<&codec::Retained>,
    proof: &AttachmentProof,
) -> Result<(), OperationError> {
    let mut tx = turn
        .connection
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|_| OperationError::StorageFailure)?;
    let result: Result<(), OperationError> = async {
        identity.confirm(&mut tx).await?;
        auth.confirm(&mut tx, turn.namespace, turn.clock, proof, false)
            .await?;
        if let Some(value) = retained {
            value.confirm(&mut tx).await?;
        }
        Ok(())
    }
    .await;
    if tx.rollback().await.is_err() {
        *turn.uncertain = true;
        return Err(OperationError::RecoveryRequired);
    }
    result
}
impl NativeWorkspace {
    pub async fn install_stock_agreement(
        &self,
        request: StockInstallationRequest,
        proof: &AttachmentProof,
    ) -> Result<StockInstallationOutcome, OperationError> {
        self.worker
            .install_stock_agreement(
                request,
                proof.with_secret(|bytes| AttachmentProof::from_protected_input(bytes))?,
            )
            .await
    }
    pub async fn stock_agreement_installation(
        &self,
        request: StockInstallationRequest,
        proof: &AttachmentProof,
    ) -> Result<Option<StockInstallationReceipt>, OperationError> {
        self.worker
            .stock_agreement_installation(
                request,
                proof.with_secret(|bytes| AttachmentProof::from_protected_input(bytes))?,
            )
            .await
    }
}
