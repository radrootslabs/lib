//! Exact body-derived cache witness. It cannot authorize capture or transport.
use super::*;
#[derive(Clone, Copy)]
pub(in crate::workspace::stock) struct Projection {
    pub operation: OperationId,
    pub recipient: PublicKey,
}
impl Projection {
    pub(in crate::workspace::stock) fn digest(
        self,
        actor: PublicKey,
        entry: OperationId,
    ) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(b"radroots.seller_stock.envelope_projection.v1\0");
        hash.update(actor.as_bytes());
        hash.update(entry.as_bytes());
        hash.update(self.operation.as_bytes());
        hash.update(self.recipient.as_bytes());
        hash.finalize().into()
    }
    pub(super) async fn insert(
        self,
        connection: &mut SqliteConnection,
        actor: PublicKey,
        entry: OperationId,
    ) -> Result<(), OperationError> {
        sqlx::query(
            "INSERT INTO stock_envelopes(actor,operation_id,recipient,entry_id) VALUES(?,?,?,?)",
        )
        .bind(actor.as_bytes().as_slice())
        .bind(self.operation.as_bytes().as_slice())
        .bind(self.recipient.as_bytes().as_slice())
        .bind(entry.as_bytes().as_slice())
        .execute(connection)
        .await
        .map_err(|_| OperationError::StorageFailure)?;
        Ok(())
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
}
pub(in crate::workspace::stock) async fn raw_projection(
    connection: &mut SqliteConnection,
    actor: &[u8; 32],
    entry: &[u8; 32],
) -> Result<[u8; 32], OperationError> {
    let row = sqlx::query(
        "SELECT operation_id,recipient FROM stock_envelopes WHERE actor=? AND entry_id=?",
    )
    .bind(actor.as_slice())
    .bind(entry.as_slice())
    .fetch_optional(connection)
    .await
    .map_err(|_| OperationError::RecoveryRequired)?
    .ok_or(OperationError::RecoveryRequired)?;
    let field = |index| {
        row.try_get::<Vec<u8>, _>(index)
            .map_err(|_| OperationError::RecoveryRequired)?
            .try_into()
            .map_err(|_| OperationError::RecoveryRequired)
    };
    Ok(Projection {
        operation: OperationId::from_bytes(field(0)?),
        recipient: PublicKey::from_bytes(field(1)?)
            .map_err(|_| OperationError::RecoveryRequired)?,
    }
    .digest(
        PublicKey::from_bytes(*actor).map_err(|_| OperationError::RecoveryRequired)?,
        OperationId::from_bytes(*entry),
    ))
}
