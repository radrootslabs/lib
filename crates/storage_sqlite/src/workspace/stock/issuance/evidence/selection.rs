//! Bind every returned selector and absence fact through final SQL observation.
use super::*;
#[derive(Clone, Copy, Eq, PartialEq)]
pub(in crate::workspace::stock) struct Selected {
    pub digest: [u8; 32],
    pub entry: OperationId,
    pub revision: Sequence,
}
pub(in crate::workspace::stock) async fn page(
    connection: &mut SqliteConnection,
    actor: PublicKey,
    operation: OperationId,
    recipient: PublicKey,
    after: Option<Sequence>,
) -> Result<Vec<Selected>, OperationError> {
    let rows=sqlx::query("SELECT CASE WHEN length(e.evidence_digest)=32 THEN e.evidence_digest END,CASE WHEN length(e.entry_id)=32 THEN e.entry_id END,o.revision FROM stock_signing_evidence e JOIN stock_operations o ON o.actor=e.actor AND o.operation_id=e.entry_id WHERE e.actor=? AND e.operation_id=? AND e.recipient=? AND (? IS NULL OR o.revision>?) ORDER BY o.revision LIMIT 33")
        .bind(actor.as_bytes().as_slice()).bind(operation.as_bytes().as_slice()).bind(recipient.as_bytes().as_slice()).bind(after.map(|v|v.get() as i64)).bind(after.map(|v|v.get() as i64)).fetch_all(connection).await.map_err(|_|OperationError::RecoveryRequired)?;
    let mut selected = Vec::with_capacity(rows.len());
    let mut previous = after.map(Sequence::get).unwrap_or(0);
    for row in rows {
        let field = |index| {
            row.try_get::<Option<Vec<u8>>, _>(index)
                .map_err(|_| OperationError::RecoveryRequired)?
                .ok_or(OperationError::RecoveryRequired)?
                .try_into()
                .map_err(|_| OperationError::RecoveryRequired)
        };
        let revision: i64 = row
            .try_get(2)
            .map_err(|_| OperationError::RecoveryRequired)?;
        if revision <= 0 || revision as u64 <= previous {
            return Err(OperationError::RecoveryRequired);
        }
        let revision =
            Sequence::new(revision as u64).map_err(|_| OperationError::RecoveryRequired)?;
        previous = revision.get();
        selected.push(Selected {
            digest: field(0)?,
            entry: OperationId::from_bytes(field(1)?),
            revision,
        });
    }
    Ok(selected)
}
pub(super) async fn entry(
    connection: &mut SqliteConnection,
    actor: PublicKey,
    operation: OperationId,
    recipient: PublicKey,
    digest: [u8; 32],
) -> Result<Option<OperationId>, OperationError> {
    let value:Option<Vec<u8>>=sqlx::query_scalar("SELECT CASE WHEN length(entry_id)=32 THEN entry_id END FROM stock_signing_evidence WHERE actor=? AND operation_id=? AND recipient=? AND evidence_digest=?").bind(actor.as_bytes().as_slice()).bind(operation.as_bytes().as_slice()).bind(recipient.as_bytes().as_slice()).bind(digest.as_slice()).fetch_optional(connection).await.map_err(|_|OperationError::RecoveryRequired)?;
    value
        .map(|id| {
            Ok(OperationId::from_bytes(
                id.try_into()
                    .map_err(|_| OperationError::RecoveryRequired)?,
            ))
        })
        .transpose()
}
pub(in crate::workspace::stock) async fn open(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    operation: OperationId,
    recipient: PublicKey,
    selected: Selected,
) -> Result<codec::Retained, OperationError> {
    let retained = read_entry(connection, input, crypto, selected.entry)
        .await?
        .ok_or(OperationError::RecoveryRequired)?;
    if retained.frame.header.revision != selected.revision
        || retained.value.receipt.attempt().operation_id() != operation
        || retained.value.receipt.recipient() != recipient
        || retained.value.receipt.digest() != &selected.digest
    {
        return Err(OperationError::RecoveryRequired);
    }
    Ok(retained)
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub(in crate::workspace::stock) struct Summary {
    pub count: Sequence,
    pub first: Option<Selected>,
}
impl Summary {
    pub(in crate::workspace::stock) async fn read(
        connection: &mut SqliteConnection,
        actor: PublicKey,
        operation: OperationId,
        recipient: PublicKey,
    ) -> Result<Self, OperationError> {
        let count:i64=sqlx::query_scalar("SELECT count(*) FROM stock_signing_evidence WHERE actor=? AND operation_id=? AND recipient=?").bind(actor.as_bytes().as_slice()).bind(operation.as_bytes().as_slice()).bind(recipient.as_bytes().as_slice()).fetch_one(&mut *connection).await.map_err(|_|OperationError::RecoveryRequired)?;
        if count < 0 {
            return Err(OperationError::RecoveryRequired);
        }
        let count = Sequence::new(count as u64).map_err(|_| OperationError::RecoveryRequired)?;
        let first = page(connection, actor, operation, recipient, None)
            .await?
            .first()
            .copied();
        if (count.get() == 0) != first.is_none() {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(Self { count, first })
    }
    pub(in crate::workspace::stock) async fn confirm(
        &self,
        connection: &mut SqliteConnection,
        actor: PublicKey,
        operation: OperationId,
        recipient: PublicKey,
    ) -> Result<(), OperationError> {
        if Self::read(connection, actor, operation, recipient).await? != *self {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(())
    }
}
