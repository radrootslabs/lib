//! Body-derived immutable agreement and complete recipient-work index witness.
use super::*;
#[derive(Clone, Copy)]
pub(in crate::workspace::stock) struct Projection {
    pub operation: OperationId,
    order: OrderId,
    record: RecordId,
    recipients: [PublicKey; 2],
    sequence: Sequence,
    changes: Option<[u8; 32]>,
}
impl Projection {
    pub(super) fn new(header: &ledger::Header, body: &Body) -> Result<Self, OperationError> {
        Ok(Self {
            operation: body.operation,
            order: header.order.ok_or(OperationError::RecoveryRequired)?,
            record: body.record,
            recipients: body.work.map(|value| value.recipient),
            sequence: body
                .commit_sequence
                .unwrap_or(Sequence::new(1).map_err(|_| OperationError::RecoveryRequired)?),
            changes: body
                .changes
                .as_deref()
                .map(super::super::changes::digest)
                .transpose()?,
        })
    }
    pub(in crate::workspace::stock) fn digest(
        self,
        actor: PublicKey,
        entry: OperationId,
    ) -> [u8; 32] {
        let mut h = Sha256::new();
        h.update(b"radroots.seller_stock.installation_projection.v1\0");
        h.update(actor.as_bytes());
        h.update(entry.as_bytes());
        h.update(self.operation.as_bytes());
        h.update(self.order.as_bytes());
        h.update(self.sequence.get().to_be_bytes());
        h.update(self.record.as_bytes());
        for recipient in self.recipients {
            h.update(recipient.as_bytes());
        }
        if let Some(changes) = self.changes {
            h.update(b"obligation_changes.v1\0");
            h.update(changes);
        }
        h.finalize().into()
    }
    pub(super) async fn confirm(
        self,
        connection: &mut SqliteConnection,
        actor: PublicKey,
        entry: OperationId,
    ) -> Result<(), OperationError> {
        if raw_projection(connection, actor.as_bytes(), entry.as_bytes()).await?
            != self.digest(actor, entry)
        {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(())
    }
    pub(super) async fn insert(
        self,
        connection: &mut SqliteConnection,
        actor: PublicKey,
        entry: OperationId,
    ) -> Result<(), OperationError> {
        sqlx::query("INSERT INTO stock_agreements(order_id,commit_sequence,actor,operation_id,entry_id,record_id) VALUES(?,?,?,?,?,?)").bind(self.order.as_bytes().as_slice()).bind(self.sequence.get() as i64).bind(actor.as_bytes().as_slice()).bind(self.operation.as_bytes().as_slice()).bind(entry.as_bytes().as_slice()).bind(self.record.as_bytes().as_slice()).execute(&mut *connection).await.map_err(|_|OperationError::StorageFailure)?;
        for recipient in self.recipients {
            sqlx::query("INSERT INTO stock_publication_work(actor,operation_id,recipient,installation_entry) VALUES(?,?,?,?)").bind(actor.as_bytes().as_slice()).bind(self.operation.as_bytes().as_slice()).bind(recipient.as_bytes().as_slice()).bind(entry.as_bytes().as_slice()).execute(&mut *connection).await.map_err(|_|OperationError::StorageFailure)?;
        }
        Ok(())
    }
}
pub(in crate::workspace::stock) async fn raw_projection(
    connection: &mut SqliteConnection,
    actor: &[u8; 32],
    entry: &[u8; 32],
) -> Result<[u8; 32], OperationError> {
    let row=sqlx::query("SELECT order_id,commit_sequence,operation_id,record_id FROM stock_agreements WHERE actor=? AND entry_id=?").bind(actor.as_slice()).bind(entry.as_slice()).fetch_optional(&mut *connection).await.map_err(|_|OperationError::RecoveryRequired)?.ok_or(OperationError::RecoveryRequired)?;
    let field = |index| {
        let value: Vec<u8> = row
            .try_get(index)
            .map_err(|_| OperationError::RecoveryRequired)?;
        value
            .try_into()
            .map_err(|_| OperationError::RecoveryRequired)
    };
    let sequence: i64 = row
        .try_get(1)
        .map_err(|_| OperationError::RecoveryRequired)?;
    if sequence <= 0 {
        return Err(OperationError::RecoveryRequired);
    }
    let operation = OperationId::from_bytes(field(2)?);
    let rows=sqlx::query("SELECT operation_id,recipient FROM stock_publication_work WHERE actor=? AND installation_entry=? ORDER BY recipient LIMIT 3").bind(actor.as_slice()).bind(entry.as_slice()).fetch_all(&mut *connection).await.map_err(|_|OperationError::RecoveryRequired)?;
    if rows.len() != 2 {
        return Err(OperationError::RecoveryRequired);
    }
    let mut recipients = Vec::with_capacity(2);
    for row in rows {
        let value: Vec<u8> = row
            .try_get(0)
            .map_err(|_| OperationError::RecoveryRequired)?;
        if value != operation.as_bytes() {
            return Err(OperationError::RecoveryRequired);
        }
        let value: Vec<u8> = row
            .try_get(1)
            .map_err(|_| OperationError::RecoveryRequired)?;
        recipients.push(
            PublicKey::from_bytes(
                value
                    .try_into()
                    .map_err(|_| OperationError::RecoveryRequired)?,
            )
            .map_err(|_| OperationError::RecoveryRequired)?,
        );
    }
    let actor_key = PublicKey::from_bytes(*actor).map_err(|_| OperationError::RecoveryRequired)?;
    let entry_key = OperationId::from_bytes(*entry);
    let catalog: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&mut *connection)
        .await
        .map_err(|_| OperationError::RecoveryRequired)?;
    let changes = if catalog >= 20 {
        let raw = super::super::changes::raw_digest(connection, actor_key, entry_key).await?;
        if sequence != 1 || raw != super::super::changes::digest(&[])? {
            Some(raw)
        } else {
            None
        }
    } else {
        None
    };
    let projection = Projection {
        operation,
        order: OrderId::from_bytes(field(0)?),
        record: RecordId::from_bytes(field(3)?),
        recipients: recipients
            .try_into()
            .map_err(|_| OperationError::RecoveryRequired)?,
        sequence: Sequence::new(sequence as u64).map_err(|_| OperationError::RecoveryRequired)?,
        changes,
    };
    Ok(projection.digest(
        PublicKey::from_bytes(*actor).map_err(|_| OperationError::RecoveryRequired)?,
        OperationId::from_bytes(*entry),
    ))
}
