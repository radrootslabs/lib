//! Original-allocation residual transitions; the SQL projection has identities
//! only. Quantities stay in the authenticated installation journal body.
use super::*;
use radroots_domain::{
    AgreementLineId, PublicKey,
    stock::{ObligationBacking, StockTicks},
};

#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::workspace::stock) struct Release {
    pub acceptance: OperationId,
    pub installation: OperationId,
    pub line: AgreementLineId,
    pub pool: PoolId,
    pub before: ObligationBacking,
    pub after: ObligationBacking,
    pub backed: StockTicks,
    pub deficit: StockTicks,
}
impl Release {
    pub fn validate(&self) -> Result<StockTicks, OperationError> {
        let quantity = self
            .backed
            .checked_add(self.deficit)
            .map_err(|_| OperationError::RecoveryRequired)?;
        if self.before.release_residual(self.backed, self.deficit).ok() != Some(self.after) {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(quantity)
    }
    fn key(&self) -> ChangeIdentity {
        ChangeIdentity {
            acceptance: self.acceptance,
            line: self.line,
            pool: self.pool,
            installation: self.installation,
        }
    }
}
#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
struct ChangeIdentity {
    acceptance: OperationId,
    line: AgreementLineId,
    pool: PoolId,
    installation: OperationId,
}
fn index_digest(values: &mut [ChangeIdentity]) -> Result<[u8; 32], OperationError> {
    if values.len() > 64 {
        return Err(OperationError::RecoveryRequired);
    }
    values.sort();
    if values.windows(2).any(|pair| {
        pair[0].acceptance == pair[1].acceptance
            && pair[0].line == pair[1].line
            && pair[0].pool == pair[1].pool
    }) {
        return Err(OperationError::RecoveryRequired);
    }
    let mut hash = Sha256::new();
    hash.update(b"radroots.seller_stock.obligation_changes_projection.v1\0");
    hash.update((values.len() as u64).to_be_bytes());
    for value in values {
        hash.update(value.acceptance.as_bytes());
        hash.update(value.line.as_bytes());
        hash.update(value.pool.as_bytes());
        hash.update(value.installation.as_bytes());
    }
    Ok(hash.finalize().into())
}
pub(super) fn digest(changes: &[Release]) -> Result<[u8; 32], OperationError> {
    for change in changes {
        change.validate()?;
    }
    index_digest(&mut changes.iter().map(Release::key).collect::<Vec<_>>())
}
pub(super) async fn raw_digest(
    connection: &mut SqliteConnection,
    actor: PublicKey,
    entry: OperationId,
) -> Result<[u8; 32], OperationError> {
    let rows=sqlx::query("SELECT acceptance_id,line_id,pool_id,installation_id FROM stock_obligation_changes WHERE actor=? AND operation_id=? ORDER BY acceptance_id,line_id,pool_id LIMIT 65")
        .bind(actor.as_bytes().as_slice()).bind(entry.as_bytes().as_slice()).fetch_all(&mut *connection).await.map_err(|_|OperationError::RecoveryRequired)?;
    let mut values = Vec::with_capacity(rows.len());
    for row in rows {
        let field = |index| -> Result<[u8; 32], OperationError> {
            row.try_get::<Vec<u8>, _>(index)
                .map_err(|_| OperationError::RecoveryRequired)?
                .try_into()
                .map_err(|_| OperationError::RecoveryRequired)
        };
        values.push(ChangeIdentity {
            acceptance: OperationId::from_bytes(field(0)?),
            line: AgreementLineId::from_bytes(field(1)?),
            pool: PoolId::from_bytes(field(2)?),
            installation: OperationId::from_bytes(field(3)?),
        });
    }
    index_digest(&mut values)
}
pub(super) async fn insert(
    connection: &mut SqliteConnection,
    actor: PublicKey,
    entry: OperationId,
    changes: &[Release],
) -> Result<(), OperationError> {
    digest(changes)?;
    for change in changes {
        sqlx::query("INSERT INTO stock_obligation_changes(actor,operation_id,acceptance_id,installation_id,line_id,pool_id) VALUES(?,?,?,?,?,?)")
            .bind(actor.as_bytes().as_slice()).bind(entry.as_bytes().as_slice()).bind(change.acceptance.as_bytes().as_slice())
            .bind(change.installation.as_bytes().as_slice()).bind(change.line.as_bytes().as_slice()).bind(change.pool.as_bytes().as_slice())
            .execute(&mut *connection).await.map_err(|_|OperationError::StorageFailure)?;
    }
    Ok(())
}

/// Replays each protected loss/release in original revision order. Summing
/// historical losses alone would resurrect released obligations or deficits.
pub(super) async fn current(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    original: &super::allocations::InstalledAllocation,
    before_revision: u64,
) -> Result<ObligationBacking, OperationError> {
    let mut state = ObligationBacking::new(
        original.allocation.ticks(),
        original.allocation.ticks(),
        StockTicks::ZERO,
    )
    .map_err(|_| OperationError::RecoveryRequired)?;
    let mut cursor = 0_u64;
    loop {
        let row=sqlx::query("WITH c AS (SELECT actor,operation_id,acceptance_id,installation_id,line_id,pool_id,0 AS kind FROM stock_deficits UNION ALL SELECT actor,operation_id,acceptance_id,installation_id,line_id,pool_id,1 AS kind FROM stock_obligation_changes) SELECT o.revision,o.operation_id,c.kind FROM c JOIN stock_operations o ON o.actor=c.actor AND o.operation_id=c.operation_id WHERE c.actor=? AND c.acceptance_id=? AND c.installation_id=? AND c.line_id=? AND c.pool_id=? AND o.revision>? AND o.revision<? ORDER BY o.revision LIMIT 1")
            .bind(input.participant().as_bytes().as_slice()).bind(original.acceptance.as_bytes().as_slice()).bind(original.installation.as_bytes().as_slice())
            .bind(original.allocation.line_id().as_bytes().as_slice()).bind(original.allocation.pool_id().as_bytes().as_slice())
            .bind(cursor as i64).bind(before_revision as i64).fetch_optional(&mut *connection).await.map_err(|_|OperationError::RecoveryRequired)?;
        let Some(row) = row else {
            return Ok(state);
        };
        let revision: i64 = row
            .try_get(0)
            .map_err(|_| OperationError::RecoveryRequired)?;
        let operation = OperationId::from_bytes(
            row.try_get::<Vec<u8>, _>(1)
                .map_err(|_| OperationError::RecoveryRequired)?
                .try_into()
                .map_err(|_| OperationError::RecoveryRequired)?,
        );
        let kind: i64 = row
            .try_get(2)
            .map_err(|_| OperationError::RecoveryRequired)?;
        if revision <= 0 {
            return Err(OperationError::RecoveryRequired);
        }
        cursor = revision as u64;
        let frame = super::super::ledger::read(connection, input, crypto, operation)
            .await?
            .ok_or(OperationError::RecoveryRequired)?;
        if frame.header.revision.get() != cursor
            || frame.header.actor.participant() != input.participant()
        {
            return Err(OperationError::RecoveryRequired);
        }
        if kind == 0 {
            let (request, data) = decode_operation(&frame.body, 20)?;
            if frame.header.family != super::super::ledger::Family::Inventory
                || request.command().digest() != frame.header.digest
                || data.revision != frame.header.revision
                || data.head != frame.header.head
                || data.pool.id() != original.allocation.pool_id()
            {
                return Err(OperationError::RecoveryRequired);
            }
            let mut matches = data.losses.iter().filter(|loss| {
                loss.acceptance == original.acceptance
                    && loss.installation == original.installation
                    && loss.line == original.allocation.line_id()
            });
            let change = matches.next().ok_or(OperationError::RecoveryRequired)?;
            if matches.next().is_some() || change.before != state {
                return Err(OperationError::RecoveryRequired);
            }
            change.quantity()?;
            state = change.after;
        } else if kind == 1 {
            let changes = super::installation::read_changes(&frame)?;
            let mut matches = changes.iter().filter(|change| {
                change.acceptance == original.acceptance
                    && change.installation == original.installation
                    && change.line == original.allocation.line_id()
                    && change.pool == original.allocation.pool_id()
            });
            let change = matches.next().ok_or(OperationError::RecoveryRequired)?;
            if matches.next().is_some() || change.before != state {
                return Err(OperationError::RecoveryRequired);
            }
            change.validate()?;
            state = change.after;
        } else {
            return Err(OperationError::RecoveryRequired);
        }
    }
}
