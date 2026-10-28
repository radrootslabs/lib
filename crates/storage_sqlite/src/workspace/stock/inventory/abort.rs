//! Explicit release after authenticated original nonissuance and replay absence.
use super::super::{ledger, order_state, signing_plan};
use super::*;
use checkpoint::{EntryIdentity, IdentityTarget};
use radroots_domain::stock::StockTicks;
mod codec;
mod projection;
mod proof;
pub(in crate::workspace::stock) use codec::read_entry;
pub(in crate::workspace::stock) use projection::{Projection, raw_projection};

#[derive(Clone, Copy, Serialize, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct Effect {
    before: PoolSnapshot,
    after: PoolSnapshot,
    quantity: StockTicks,
    movement: [u8; 32],
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Plan {
    entry: OperationId,
    head: CheckpointDigest,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Body {
    version: u8,
    operation: OperationId,
    original_head: CheckpointDigest,
    #[serde(deserialize_with = "super::super::required_nullable")]
    plan: Option<Plan>,
    aborted_at: UnixSeconds,
    authorization: StockAuthorizationRef,
    effects: Vec<Effect>,
    #[serde(default, skip_serializing_if = "proof::Payload::legacy")]
    proofs: proof::Payload,
}
fn after(before: PoolSnapshot, quantity: StockTicks) -> Result<PoolSnapshot, OperationError> {
    let balances = before
        .balances()
        .release_preparatory(quantity)
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
async fn no_claims(
    connection: &mut SqliteConnection,
    actor: radroots_domain::PublicKey,
    operation: OperationId,
) -> Result<(), OperationError> {
    let count:i64=sqlx::query_scalar("SELECT (SELECT count(*) FROM stock_signing_claims WHERE actor=? AND operation_id=?)+(SELECT count(*) FROM stock_signing_evidence WHERE actor=? AND operation_id=?)+(SELECT count(*) FROM stock_agreements WHERE actor=? AND operation_id=?)+(SELECT count(*) FROM stock_operations WHERE actor=? AND related_operation=? AND family BETWEEN 4 AND 6)")
        .bind(actor.as_bytes().as_slice()).bind(operation.as_bytes().as_slice())
        .bind(actor.as_bytes().as_slice()).bind(operation.as_bytes().as_slice())
        .bind(actor.as_bytes().as_slice()).bind(operation.as_bytes().as_slice())
        .bind(actor.as_bytes().as_slice()).bind(operation.as_bytes().as_slice()).fetch_one(connection).await
        .map_err(|_|OperationError::RecoveryRequired)?;
    if count != 0 {
        return Err(OperationError::RecoveryRequired);
    }
    Ok(())
}
pub(in crate::workspace::stock) async fn existing(
    turn: &mut Turn<'_>,
    access: &Access,
    operation: OperationId,
) -> Result<(Option<codec::Retained>, MinimumProof), OperationError> {
    let id:Option<Vec<u8>>=sqlx::query_scalar("SELECT CASE WHEN length(entry_id)=32 THEN entry_id END FROM stock_aborts WHERE actor=? AND operation_id=?")
        .bind(turn.selected.participant().as_bytes().as_slice()).bind(operation.as_bytes().as_slice())
        .fetch_optional(&mut *turn.connection).await.map_err(|_|OperationError::RecoveryRequired)?;
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
    let proof = Box::pin(checkpoint::authenticate_identity(
        turn.connection,
        turn.selected,
        turn.crypto,
        &access.current,
        &access.authority,
        IdentityTarget::Abort {
            operation,
            expected: retained.as_ref().map(codec::Retained::identity),
        },
    ))
    .await?;
    Ok((retained, proof))
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
    let checked: Result<(), OperationError> = async {
        Box::pin(identity.confirm(&mut tx)).await?;
        if let Some(retained) = retained {
            retained.confirm(&mut tx).await?;
        }
        Box::pin(auth.confirm(&mut tx, turn.namespace, turn.clock, proof, false)).await?;
        Ok(())
    }
    .await;
    if tx.rollback().await.is_err() {
        *turn.uncertain = true;
        return Err(OperationError::RecoveryRequired);
    }
    checked
}
pub(in crate::workspace) async fn abort(
    turn: &mut Turn<'_>,
    request: StockAbortRequest,
    proof: AttachmentProof,
) -> Result<StockAbortOutcome, OperationError> {
    let auth = Box::pin(signing_plan::load(
        turn,
        request.binding(),
        request.minimum_checkpoint(),
        request.operation_id(),
        request.command_digest(),
        &proof,
    ))
    .await?;
    Box::pin(abort_authorized(turn, request, proof, auth)).await
}
async fn abort_authorized(
    turn: &mut Turn<'_>,
    request: StockAbortRequest,
    proof: AttachmentProof,
    auth: signing_plan::Authorized,
) -> Result<StockAbortOutcome, OperationError> {
    if !matches!(auth.access.authority.catalog, 13..=17) {
        return Err(OperationError::UnsupportedOperation);
    }
    let (old, identity) = Box::pin(existing(turn, &auth.access, request.operation_id())).await?;
    if let Some(old) = old {
        Box::pin(confirm_read(turn, &auth, &identity, Some(&old), &proof)).await?;
        return Ok(StockAbortOutcome::Aborted(Box::new(old.receipt)));
    }
    let at = auth.access.current.observed_at();
    let earliest = auth
        .plan
        .as_ref()
        .map(|plan| plan.bound.bound_at())
        .unwrap_or(auth.original.receipt.prepared_at());
    if at < earliest {
        return Err(WorkspaceError::Unavailable.into());
    }
    let current = Box::pin(checkpoint::authenticate_order_state(
        turn.connection,
        turn.selected,
        turn.crypto,
        &auth.access.current,
        &auth.access.authority,
        auth.original.receipt.order_id(),
    ))
    .await?;
    if current.witness.expected.is_none_or(|value| {
        value.phase != order_state::Phase::Prepared || value.operation != request.operation_id()
    }) {
        return Err(OperationError::StaleState);
    }
    let nonissuance = Box::pin(proof::prepare(turn, &auth, request.operation_id(), at)).await?;
    let backing = Box::pin(super::backing::read(
        turn,
        &auth.access,
        auth.original.receipt.held_pools(),
    ))
    .await?;
    let mut originals = Vec::new();
    let mut effects = Vec::new();
    for (pool, quantity) in auth.original.held_allocations()? {
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
        version: if nonissuance.payload.legacy() { 1 } else { 2 },
        operation: request.operation_id(),
        original_head: auth.original.frame.header.head,
        plan: auth.plan.as_ref().map(|plan| Plan {
            entry: plan.frame.header.operation,
            head: plan.frame.header.head,
        }),
        aborted_at: at,
        authorization: auth
            .access
            .authority
            .authorization
            .ok_or(OperationError::RecoveryRequired)?,
        effects,
        proofs: nonissuance.payload.clone(),
    };
    let clear = zeroize::Zeroizing::new(
        serde_json::to_vec(&body).map_err(|_| OperationError::StorageFailure)?,
    );
    if clear.len() > ledger::BODY_MAX_BYTES {
        return Err(WorkspaceError::Capacity.into());
    }
    let mut header = ledger::Header {
        version: 5,
        family: ledger::Family::AbortedPreparation,
        namespace: SellerAuthoritySelection::from_attachment(turn.selected),
        actor: auth.access.current.actor(),
        operation: entry,
        digest: request.command_digest(),
        transaction,
        revision,
        previous: before.head,
        head: CheckpointDigest::from_bytes([0; 32]),
        pool: None,
        order: Some(auth.original.receipt.order_id()),
        related_operation: Some(request.operation_id()),
        subject_grant: None,
        bytes: 0,
        parts: 0,
        body_digest: [0; 32],
    };
    header.head = codec::head(&header, &clear, body.version)?;
    let projection = Projection::new(&header, &body)?;
    let framed = ledger::prepare(turn.crypto, turn.selected, header, &clear).await?;
    let receipt = codec::receipt(turn.selected, &framed.header, &body)?;
    let mut pool_bytes = Vec::new();
    let mut movement_bytes = Vec::new();
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
                    "abort_movement",
                    &effect.movement,
                    &encode(&codec::Movement {
                        version: 1,
                        operation: entry,
                        digest: request.command_digest(),
                        original: request.operation_id(),
                        effect: *effect,
                    })?,
                )
                .await?,
        );
    }
    let mut next = auth.access.authority.clone();
    next.inventory = Some(InventoryCheckpoint {
        revision,
        head: Some(framed.header.head),
        movements: Sequence::new(
            before
                .movements
                .get()
                .checked_add(body.effects.len() as u64)
                .ok_or(WorkspaceError::Capacity)?,
        )
        .map_err(|_| WorkspaceError::Capacity)?,
        ..before
    });
    let root = turn
        .crypto
        .seal_stock_root(turn.selected, next.catalog, &next.encode()?)
        .await?;
    let anchor = receipt.attempt();
    let mut tx = turn
        .connection
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|_| OperationError::StorageFailure)?;
    let changed:Result<(),OperationError>=async {
        Box::pin(identity.confirm(&mut tx)).await?;
        Box::pin(nonissuance.confirm(&mut tx)).await?;
        let checked=Box::pin(auth.confirm(&mut tx,turn.namespace,turn.clock,&proof,false)).await?;
        if checked.observed_at()<at {return Err(WorkspaceError::Unavailable.into());}
        for pool in &backing {Box::pin(pool.confirm(&mut tx)).await?;}
        framed.insert_record(&mut tx).await?;projection.insert(&mut tx,turn.selected.participant(),entry).await?;
        for (index,(effect,original)) in body.effects.iter().zip(&originals).enumerate() {
            let changed=sqlx::query("UPDATE stock_pools SET revision=?,protected=? WHERE pool_id=? AND revision=? AND protected=?")
                .bind(effect.after.version().get() as i64).bind(&pool_bytes[index]).bind(effect.before.id().as_bytes().as_slice())
                .bind(effect.before.version().get() as i64).bind(&original.protected).execute(&mut *tx).await.map_err(|_|OperationError::StorageFailure)?;
            if changed.rows_affected()!=1 {return Err(OperationError::StaleState);}
            sqlx::query("INSERT INTO stock_movements(movement_id,actor,grant_id,operation_id,pool_id,protected) VALUES(?,?,?,?,?,?)")
                .bind(effect.movement.as_slice()).bind(turn.selected.participant().as_bytes().as_slice()).bind(checked.input().grant_id().as_bytes().as_slice())
                .bind(entry.as_bytes().as_slice()).bind(effect.before.id().as_bytes().as_slice()).bind(&movement_bytes[index])
                .execute(&mut *tx).await.map_err(|_|OperationError::StorageFailure)?;
        }
        current.advance(&mut tx,order_state::Marker::from_header(&framed.header,order_state::Phase::Aborted)?).await?;
        let changed=sqlx::query("UPDATE stock_authority SET protected=? WHERE singleton=1 AND protected=?")
            .bind(root).bind(&auth.access.root).execute(&mut *tx).await.map_err(|_|OperationError::StorageFailure)?;
        if changed.rows_affected()!=1 {return Err(OperationError::RecoveryRequired);}
        turn.namespace.validate()?;Ok(())
    }.await;
    if let Err(error) = changed {
        if tx.rollback().await.is_err() {
            *turn.uncertain = true;
            return Ok(StockAbortOutcome::Unknown(anchor));
        }
        return Err(error);
    }
    if tx.commit().await.is_err() {
        *turn.uncertain = true;
        return Ok(StockAbortOutcome::Unknown(anchor));
    }
    Ok(StockAbortOutcome::Aborted(Box::new(receipt)))
}
pub(in crate::workspace) async fn read(
    turn: &mut Turn<'_>,
    request: StockAbortRequest,
    proof: AttachmentProof,
) -> Result<Option<StockAbortReceipt>, OperationError> {
    let auth = Box::pin(signing_plan::load(
        turn,
        request.binding(),
        request.minimum_checkpoint(),
        request.operation_id(),
        request.command_digest(),
        &proof,
    ))
    .await?;
    if !matches!(auth.access.authority.catalog, 13..=17) {
        return Err(OperationError::UnsupportedOperation);
    }
    let (old, identity) = Box::pin(existing(turn, &auth.access, request.operation_id())).await?;
    Box::pin(confirm_read(turn, &auth, &identity, old.as_ref(), &proof)).await?;
    Ok(old.map(|value| value.receipt))
}

impl NativeWorkspace {
    pub async fn abort_proven_unissued(
        &self,
        request: StockAbortRequest,
        proof: &AttachmentProof,
    ) -> Result<StockAbortOutcome, OperationError> {
        self.worker
            .abort_proven_unissued(
                request,
                proof.with_secret(|bytes| AttachmentProof::from_protected_input(bytes))?,
            )
            .await
    }
    pub async fn stock_preparation_abort(
        &self,
        request: StockAbortRequest,
        proof: &AttachmentProof,
    ) -> Result<Option<StockAbortReceipt>, OperationError> {
        self.worker
            .stock_preparation_abort(
                request,
                proof.with_secret(|bytes| AttachmentProof::from_protected_input(bytes))?,
            )
            .await
    }
}
