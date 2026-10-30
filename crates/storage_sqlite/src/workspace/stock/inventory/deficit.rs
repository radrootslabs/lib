//! Catalog19 owner-private losses against retained, installed original allocations.
//! The SQL index is only a projection. Full protected ancestry authenticates its
//! complete contents and is reread under the same final transaction fence.
use super::*;
use radroots_domain::{
    AgreementLineId, PublicKey,
    stock::{ObligationBacking, StockTicks},
};

pub(super) const MAX_LOSSES: usize = 32;

#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Loss {
    pub(super) acceptance: OperationId,
    pub(super) installation: OperationId,
    pub(super) line: AgreementLineId,
    pub(super) before: ObligationBacking,
    pub(super) after: ObligationBacking,
}
impl Loss {
    pub(super) fn quantity(&self) -> Result<StockTicks, OperationError> {
        let quantity = self
            .before
            .backed()
            .checked_sub(self.after.backed())
            .map_err(|_| OperationError::RecoveryRequired)?;
        if quantity == StockTicks::ZERO
            || self.before.lose_backing(quantity).ok() != Some(self.after)
        {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(quantity)
    }
    pub(super) fn metadata(&self, pool: PoolId) -> radroots_storage::inventory::StockDeficit {
        radroots_storage::inventory::StockDeficit::from_host_metadata(
            self.acceptance,
            self.installation,
            self.line,
            pool,
            self.after,
        )
    }
}

/// Available stock is corrected first. Any remainder selects still-backed
/// installed allocations by original installation revision then original line
/// order. Preparatory and quarantined claims are never silently depleted.
pub(super) async fn plan(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    command: &radroots_storage::inventory::CapturedInventoryCommand,
    before: Option<PoolSnapshot>,
    before_revision: u64,
) -> Result<Vec<Loss>, OperationError> {
    use radroots_domain::stock::AvailableStockCorrection;
    let radroots_storage::inventory::CapturedInventoryCommand::CorrectPool(command) = command
    else {
        return Ok(Vec::new());
    };
    if command.input().kind() == AvailableStockCorrection::Add {
        return Ok(Vec::new());
    }
    let pool = before.ok_or(OperationError::StaleState)?;
    if pool.id() != command.input().pool_id()
        || pool.version() != command.input().expected_version()
    {
        return Err(OperationError::StaleState);
    }
    let quantity = pool
        .unit()
        .ticks_for(
            radroots_domain::Quantity::new(command.input().amount().amount(), pool.unit().unit())
                .map_err(|_| OperationError::InvalidCommand)?,
        )
        .map_err(|_| OperationError::InvalidCommand)?;
    let physical = pool.balances().physical();
    if quantity <= physical.available {
        return Ok(Vec::new());
    }
    let mut remaining = quantity
        .checked_sub(physical.available)
        .map_err(|_| OperationError::InvalidCommand)?;
    if remaining > physical.committed_hold {
        return Err(OperationError::StaleState);
    }
    let mut cursor = super::allocations::InstalledCursor::new(pool.id(), before_revision)?;
    let mut losses = Vec::new();
    while remaining != StockTicks::ZERO {
        let allocation = Box::pin(cursor.next(connection, input, crypto))
            .await?
            .ok_or(OperationError::RecoveryRequired)?;
        let before = allocation
            .backing(connection, input, crypto, before_revision)
            .await?;
        let backed = before.backed();
        if backed == StockTicks::ZERO {
            continue;
        }
        if losses.len() == MAX_LOSSES {
            return Err(WorkspaceError::Capacity.into());
        }
        let quantity = remaining.min(backed);
        let after = before
            .lose_backing(quantity)
            .map_err(|_| OperationError::RecoveryRequired)?;
        losses.push(Loss {
            acceptance: allocation.acceptance,
            installation: allocation.installation,
            line: allocation.allocation.line_id(),
            before,
            after,
        });
        remaining = remaining
            .checked_sub(quantity)
            .map_err(|_| OperationError::RecoveryRequired)?;
    }
    Ok(losses)
}

pub(super) fn quantity(losses: &[Loss]) -> Result<StockTicks, OperationError> {
    if losses.len() > MAX_LOSSES {
        return Err(WorkspaceError::Capacity.into());
    }
    let mut quantity = StockTicks::ZERO;
    for (index, loss) in losses.iter().enumerate() {
        if losses[..index]
            .iter()
            .any(|prior| prior.acceptance == loss.acceptance && prior.line == loss.line)
        {
            return Err(OperationError::RecoveryRequired);
        }
        quantity = quantity
            .checked_add(loss.quantity()?)
            .map_err(|_| OperationError::RecoveryRequired)?;
    }
    Ok(quantity)
}

/// Index rows contain only identities. Actual prior quantities remain inside
/// bounded protected operation bodies; no plaintext numeric sum is authoritative.
#[expect(
    clippy::too_many_arguments,
    reason = "exact original allocation and revision boundary"
)]
pub(super) async fn prior_quantity(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    acceptance: OperationId,
    installation: OperationId,
    line: AgreementLineId,
    pool: PoolId,
    before_revision: u64,
) -> Result<StockTicks, OperationError> {
    let mut quantity = StockTicks::ZERO;
    let mut cursor = 0_u64;
    loop {
        let row = sqlx::query("SELECT o.revision,CASE WHEN length(o.operation_id)=32 THEN o.operation_id END FROM stock_deficits d JOIN stock_operations o ON o.actor=d.actor AND o.operation_id=d.operation_id WHERE d.actor=? AND d.acceptance_id=? AND d.installation_id=? AND d.line_id=? AND d.pool_id=? AND o.revision>? AND o.revision<? ORDER BY o.revision LIMIT 1")
            .bind(input.participant().as_bytes().as_slice()).bind(acceptance.as_bytes().as_slice())
            .bind(installation.as_bytes().as_slice()).bind(line.as_bytes().as_slice())
            .bind(pool.as_bytes().as_slice()).bind(cursor as i64).bind(before_revision as i64)
            .fetch_optional(&mut *connection).await.map_err(|_| OperationError::RecoveryRequired)?;
        let Some(row) = row else {
            return Ok(quantity);
        };
        let revision: i64 = row
            .try_get(0)
            .map_err(|_| OperationError::RecoveryRequired)?;
        let operation: Vec<u8> = row
            .try_get::<Option<Vec<u8>>, _>(1)
            .map_err(|_| OperationError::RecoveryRequired)?
            .ok_or(OperationError::RecoveryRequired)?;
        if revision <= 0 {
            return Err(OperationError::RecoveryRequired);
        }
        cursor = revision as u64;
        let operation = OperationId::from_bytes(
            operation
                .try_into()
                .map_err(|_| OperationError::RecoveryRequired)?,
        );
        let frame = super::super::ledger::read(connection, input, crypto, operation)
            .await?
            .ok_or(OperationError::RecoveryRequired)?;
        let (request, data) = super::codec::decode_operation(&frame.body, 19)?;
        if frame.header.family != super::super::ledger::Family::Inventory
            || frame.header.actor.participant() != input.participant()
            || frame.header.revision.get() != cursor
            || frame.header.operation != operation
            || request.command().context().operation_id() != operation
            || request.command().digest() != frame.header.digest
            || data.revision != frame.header.revision
            || data.head != frame.header.head
            || data.pool.id() != pool
        {
            return Err(OperationError::RecoveryRequired);
        }
        let mut matching = data.losses.iter().filter(|loss| {
            loss.acceptance == acceptance && loss.installation == installation && loss.line == line
        });
        let loss = matching.next().ok_or(OperationError::RecoveryRequired)?;
        if matching.next().is_some() {
            return Err(OperationError::RecoveryRequired);
        }
        quantity = quantity
            .checked_add(loss.quantity()?)
            .map_err(|_| OperationError::RecoveryRequired)?;
    }
}

fn projection_hash() -> Sha256 {
    let mut hash = Sha256::new();
    hash.update(b"radroots.seller_stock.deficit_projection.v1\0");
    hash
}
pub(super) fn digest(pool: PoolId, losses: &[Loss]) -> Result<[u8; 32], OperationError> {
    let mut ordered: Vec<_> = losses.iter().collect();
    ordered.sort_by_key(|loss| (loss.acceptance, loss.line));
    let mut hash = projection_hash();
    hash.update((ordered.len() as u64).to_be_bytes());
    for loss in ordered {
        hash.update(loss.acceptance.as_bytes());
        hash.update(loss.installation.as_bytes());
        hash.update(loss.line.as_bytes());
        hash.update(pool.as_bytes());
    }
    Ok(hash.finalize().into())
}
pub(in crate::workspace::stock) async fn raw_projection(
    connection: &mut SqliteConnection,
    actor: &[u8; 32],
    operation: &[u8; 32],
) -> Result<[u8; 32], OperationError> {
    let rows = sqlx::query("SELECT CASE WHEN length(acceptance_id)=32 THEN acceptance_id END,CASE WHEN length(installation_id)=32 THEN installation_id END,CASE WHEN length(line_id)=32 THEN line_id END,CASE WHEN length(pool_id)=32 THEN pool_id END FROM stock_deficits WHERE actor=? AND operation_id=? ORDER BY acceptance_id,line_id LIMIT 33")
        .bind(actor.as_slice()).bind(operation.as_slice()).fetch_all(connection).await
        .map_err(|_| OperationError::RecoveryRequired)?;
    if rows.len() > MAX_LOSSES {
        return Err(OperationError::RecoveryRequired);
    }
    let mut hash = projection_hash();
    hash.update((rows.len() as u64).to_be_bytes());
    for row in rows {
        for column in 0..4 {
            let value: Vec<u8> = row
                .try_get::<Option<Vec<u8>>, _>(column)
                .map_err(|_| OperationError::RecoveryRequired)?
                .ok_or(OperationError::RecoveryRequired)?;
            hash.update(value);
        }
    }
    Ok(hash.finalize().into())
}
pub(super) async fn insert(
    connection: &mut SqliteConnection,
    actor: PublicKey,
    operation: OperationId,
    pool: PoolId,
    losses: &[Loss],
) -> Result<(), OperationError> {
    for loss in losses {
        sqlx::query("INSERT INTO stock_deficits(actor,operation_id,acceptance_id,installation_id,line_id,pool_id) VALUES(?,?,?,?,?,?)")
            .bind(actor.as_bytes().as_slice()).bind(operation.as_bytes().as_slice())
            .bind(loss.acceptance.as_bytes().as_slice()).bind(loss.installation.as_bytes().as_slice())
            .bind(loss.line.as_bytes().as_slice()).bind(pool.as_bytes().as_slice())
            .execute(&mut *connection).await.map_err(|_| OperationError::StorageFailure)?;
    }
    if raw_projection(connection, actor.as_bytes(), operation.as_bytes()).await?
        != digest(pool, losses)?
    {
        return Err(OperationError::RecoveryRequired);
    }
    Ok(())
}
