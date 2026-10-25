use super::*;
#[derive(Clone, Copy)]
pub(in crate::workspace::stock) struct Projection {
    pub operation: OperationId,
}
impl Projection {
    pub(super) fn new(header: &ledger::Header, body: &Body) -> Result<Self, OperationError> {
        if header.related_operation != Some(body.operation) {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(Self {
            operation: body.operation,
        })
    }
    pub(in crate::workspace::stock) fn digest(
        self,
        actor: radroots_domain::PublicKey,
        entry: OperationId,
    ) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(b"radroots.seller_stock.abort_projection.v1\0");
        hash.update(actor.as_bytes());
        hash.update(self.operation.as_bytes());
        hash.update(entry.as_bytes());
        hash.finalize().into()
    }
    pub(super) async fn insert(
        self,
        connection: &mut SqliteConnection,
        actor: radroots_domain::PublicKey,
        entry: OperationId,
    ) -> Result<(), OperationError> {
        sqlx::query("INSERT INTO stock_aborts(actor,operation_id,entry_id) VALUES(?,?,?)")
            .bind(actor.as_bytes().as_slice())
            .bind(self.operation.as_bytes().as_slice())
            .bind(entry.as_bytes().as_slice())
            .execute(connection)
            .await
            .map_err(|_| OperationError::StorageFailure)?;
        Ok(())
    }
    pub(super) async fn confirm(
        self,
        connection: &mut SqliteConnection,
        actor: radroots_domain::PublicKey,
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
    let id:Vec<u8>=sqlx::query_scalar("SELECT operation_id FROM stock_aborts WHERE actor=? AND entry_id=? AND length(operation_id)=32")
        .bind(actor.as_slice()).bind(entry.as_slice()).fetch_optional(connection).await.map_err(|_|OperationError::RecoveryRequired)?
        .ok_or(OperationError::RecoveryRequired)?;
    let value = Projection {
        operation: OperationId::from_bytes(
            id.try_into()
                .map_err(|_| OperationError::RecoveryRequired)?,
        ),
    };
    Ok(value.digest(
        radroots_domain::PublicKey::from_bytes(*actor)
            .map_err(|_| OperationError::RecoveryRequired)?,
        OperationId::from_bytes(*entry),
    ))
}
