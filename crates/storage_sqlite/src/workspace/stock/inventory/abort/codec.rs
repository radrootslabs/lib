//! Decode original cleanup without recursively entering an authority/history read.
use super::*;
#[derive(Serialize, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct Movement {
    pub version: u8,
    pub operation: OperationId,
    pub digest: CommandDigest,
    pub original: OperationId,
    pub effect: Effect,
}
pub(in crate::workspace::stock) struct Retained {
    pub frame: ledger::Retained,
    pub receipt: StockAbortReceipt,
    projection: Projection,
    plan: Option<Plan>,
    movements: Vec<ledger::MovementWitness>,
    nonissuance: proof::Witness,
}
pub(super) fn head(
    header: &ledger::Header,
    body: &[u8],
    version: u8,
) -> Result<CheckpointDigest, OperationError> {
    let mut hash = Sha256::new();
    hash.update(match version {
        1 => b"radroots.seller_stock.abort_chain.v1\0".as_slice(),
        2 => b"radroots.seller_stock.abort_chain.v2\0".as_slice(),
        _ => return Err(OperationError::RecoveryRequired),
    });
    hash.update(header.namespace.workspace_id().as_bytes());
    hash.update(header.namespace.authority().executor_id().as_bytes());
    hash.update(
        header
            .namespace
            .authority()
            .generation()
            .get()
            .to_be_bytes(),
    );
    hash.update(header.namespace.route_digest().as_bytes());
    hash.update(header.actor.participant().as_bytes());
    hash.update(
        header
            .actor
            .grant_id()
            .map(|id| *id.as_bytes())
            .unwrap_or([0; 32]),
    );
    hash.update(header.operation.as_bytes());
    hash.update(header.digest.as_bytes());
    hash.update(header.transaction.as_bytes());
    hash.update(header.revision.get().to_be_bytes());
    hash.update([u8::from(header.previous.is_some())]);
    hash.update(header.previous.map(|id| *id.as_bytes()).unwrap_or([0; 32]));
    hash.update(Sha256::digest(body));
    Ok(CheckpointDigest::from_bytes(hash.finalize().into()))
}
pub(super) fn receipt(
    input: &AttachmentInput,
    header: &ledger::Header,
    body: &Body,
) -> Result<StockAbortReceipt, OperationError> {
    StockAbortReceipt::from_committed_store(
        input,
        CommitAttempt::new(
            input.workspace_id(),
            body.operation,
            header.digest,
            Some(header.transaction),
        ),
        header.order.ok_or(OperationError::RecoveryRequired)?,
        body.aborted_at,
        radroots_storage::inventory::InventoryCheckpointRef::from_host_metadata(
            input.authority(),
            header.revision,
            Some(header.head),
        )?
        .with_authorization(body.authorization),
        body.effects.iter().map(|effect| effect.after).collect(),
    )
}
async fn plan_selector(
    connection: &mut SqliteConnection,
    actor: radroots_domain::PublicKey,
    operation: OperationId,
    plan: Option<&Plan>,
) -> Result<(), OperationError> {
    let ids:Vec<(Vec<u8>,Vec<u8>)>=sqlx::query_as("SELECT operation_id,chain_head FROM stock_operations WHERE actor=? AND related_operation=? AND family=3 ORDER BY revision LIMIT 2")
        .bind(actor.as_bytes().as_slice()).bind(operation.as_bytes().as_slice()).fetch_all(connection).await.map_err(|_|OperationError::RecoveryRequired)?;
    let matches = match (plan, ids.as_slice()) {
        (None, []) => true,
        (Some(plan), [(entry, head)]) => {
            entry.as_slice() == plan.entry.as_bytes() && head.as_slice() == plan.head.as_bytes()
        }
        _ => false,
    };
    if !matches {
        return Err(OperationError::RecoveryRequired);
    }
    Ok(())
}
pub(in crate::workspace::stock) async fn read_entry(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    operation: OperationId,
) -> Result<Option<Retained>, OperationError> {
    let Some(frame) = ledger::read(connection, input, crypto, operation).await? else {
        return Ok(None);
    };
    let header = &frame.header;
    if header.family != ledger::Family::AbortedPreparation {
        return Err(OperationError::RecoveryRequired);
    }
    let body: Body = super::super::super::closed_record(&frame.body)
        .map_err(|_| OperationError::RecoveryRequired)?;
    if !matches!((body.version, body.proofs.legacy()), (1, true) | (2, false))
        || header.related_operation != Some(body.operation)
        || head(header, &frame.body, body.version)? != header.head
        || body.effects.len() > 64
    {
        return Err(OperationError::RecoveryRequired);
    }
    let original = Box::pin(acceptance::codec::read(
        connection,
        input,
        crypto,
        body.operation,
    ))
    .await?
    .ok_or(OperationError::RecoveryRequired)?;
    if original.frame.header.head != body.original_head
        || original.frame.header.digest != header.digest
        || original.frame.header.order != header.order
        || original.frame.header.revision >= header.revision
        || body.aborted_at < original.receipt.prepared_at()
    {
        return Err(OperationError::RecoveryRequired);
    }
    plan_selector(
        connection,
        input.participant(),
        body.operation,
        body.plan.as_ref(),
    )
    .await?;
    if let Some(plan) = &body.plan {
        let retained = Box::pin(signing_plan::read_entry(
            connection, input, crypto, plan.entry,
        ))
        .await?
        .ok_or(OperationError::RecoveryRequired)?;
        if retained.frame.header.head != plan.head
            || retained.frame.header.related_operation != Some(body.operation)
            || retained.frame.header.revision >= header.revision
            || body.aborted_at < retained.bound.bound_at()
        {
            return Err(OperationError::RecoveryRequired);
        }
    }
    let nonissuance = Box::pin(proof::read(
        connection,
        input,
        crypto,
        &body,
        header,
        original.receipt.rumor().recipients(),
    ))
    .await?;
    let quantities = original.held_allocations()?;
    if quantities.len() != body.effects.len() {
        return Err(OperationError::RecoveryRequired);
    }
    let mut movements = Vec::with_capacity(body.effects.len());
    for ((pool, quantity), effect) in quantities.into_iter().zip(&body.effects) {
        if pool != effect.before.id()
            || quantity != effect.quantity
            || effect.before.version().get() == 0
            || after(effect.before, quantity)? != effect.after
        {
            return Err(OperationError::RecoveryRequired);
        }
        let row=sqlx::query("SELECT actor,grant_id,operation_id,pool_id,CASE WHEN length(protected) BETWEEN 1 AND 262144 THEN protected END FROM stock_movements WHERE movement_id=?")
            .bind(effect.movement.as_slice()).fetch_optional(&mut *connection).await.map_err(|_|OperationError::RecoveryRequired)?
            .ok_or(OperationError::RecoveryRequired)?;
        let grant = header
            .actor
            .grant_id()
            .ok_or(OperationError::RecoveryRequired)?;
        for (index, wanted) in [
            *header.actor.participant().as_bytes(),
            *grant.as_bytes(),
            *header.operation.as_bytes(),
            *pool.as_bytes(),
        ]
        .into_iter()
        .enumerate()
        {
            let actual: Vec<u8> = row
                .try_get(index)
                .map_err(|_| OperationError::RecoveryRequired)?;
            if actual != wanted {
                return Err(OperationError::RecoveryRequired);
            }
        }
        let protected: Vec<u8> = row
            .try_get::<Option<Vec<u8>>, _>(4)
            .map_err(|_| OperationError::RecoveryRequired)?
            .ok_or(OperationError::RecoveryRequired)?;
        let clear = crypto
            .open_stock_record(input, "abort_movement", &effect.movement, &protected)
            .await?;
        let movement: Movement = clear
            .expose_secret(|bytes| super::super::super::closed_record(bytes))
            .map_err(|_| OperationError::RecoveryRequired)?;
        if movement
            != (Movement {
                version: 1,
                operation: header.operation,
                digest: header.digest,
                original: body.operation,
                effect: *effect,
            })
        {
            return Err(OperationError::RecoveryRequired);
        }
        movements.push(ledger::MovementWitness {
            id: effect.movement,
            pool,
            grant,
            protected,
        });
    }
    movements.sort_by_key(|value| value.id);
    if movements.windows(2).any(|pair| pair[0].id >= pair[1].id) {
        return Err(OperationError::RecoveryRequired);
    }
    let value = Retained {
        receipt: receipt(input, header, &body)?,
        projection: Projection::new(header, &body)?,
        frame,
        plan: body.plan,
        movements,
        nonissuance,
    };
    value.confirm(connection).await?;
    Ok(Some(value))
}
impl Retained {
    pub fn identity(&self) -> EntryIdentity {
        EntryIdentity {
            operation: self.frame.header.operation,
            revision: self.frame.header.revision,
            head: self.frame.header.head,
            protected_digest: Sha256::digest(&self.frame.protected).into(),
        }
    }
    pub async fn confirm(&self, connection: &mut SqliteConnection) -> Result<(), OperationError> {
        ledger::confirm_header(connection, &self.frame.header, &self.frame.protected).await?;
        ledger::confirm_parts(
            connection,
            self.frame.header.actor,
            self.frame.header.operation,
            self.frame.parts_witness,
            self.frame.header.parts,
        )
        .await?;
        self.projection
            .confirm(
                connection,
                self.frame.header.actor.participant(),
                self.frame.header.operation,
            )
            .await?;
        Box::pin(self.nonissuance.confirm(connection)).await?;
        plan_selector(
            connection,
            self.frame.header.actor.participant(),
            self.projection.operation,
            self.plan.as_ref(),
        )
        .await?;
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM stock_movements WHERE actor=? AND operation_id=?",
        )
        .bind(self.frame.header.actor.participant().as_bytes().as_slice())
        .bind(self.frame.header.operation.as_bytes().as_slice())
        .fetch_one(&mut *connection)
        .await
        .map_err(|_| OperationError::RecoveryRequired)?;
        if count != self.movements.len() as i64 {
            return Err(OperationError::RecoveryRequired);
        }
        for movement in &self.movements {
            let count:i64=sqlx::query_scalar("SELECT count(*) FROM stock_movements WHERE movement_id=? AND actor=? AND grant_id=? AND operation_id=? AND pool_id=? AND protected=?")
                .bind(movement.id.as_slice()).bind(self.frame.header.actor.participant().as_bytes().as_slice())
                .bind(movement.grant.as_bytes().as_slice()).bind(self.frame.header.operation.as_bytes().as_slice())
                .bind(movement.pool.as_bytes().as_slice()).bind(&movement.protected).fetch_one(&mut *connection).await.map_err(|_|OperationError::RecoveryRequired)?;
            if count != 1 {
                return Err(OperationError::RecoveryRequired);
            }
        }
        Ok(())
    }
    pub fn into_history(self) -> ledger::HistoryEntry {
        ledger::HistoryEntry {
            deficit: None,
            delivery: None,
            envelope: None,
            preparation: None,
            local_record: None,
            installation: None,
            signing: None,
            aborted: Some(self.projection),
            header: self.frame.header,
            protected: self.frame.protected,
            parts_witness: self.frame.parts_witness,
            pools: self.receipt.into_released_pools(),
            mapping: None,
            movements: self.movements,
        }
    }
}
