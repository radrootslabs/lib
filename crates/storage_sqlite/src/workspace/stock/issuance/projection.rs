use super::*;
/// Immutable body-derived selector, independently compared with its SQL index.
#[derive(Clone, Copy)]
pub(in crate::workspace::stock) struct Projection {
    pub operation: OperationId,
    pub recipient: PublicKey,
    pub evidence: Option<[u8; 32]>,
}
impl Projection {
    pub fn digest(self, actor: PublicKey, entry: OperationId) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(b"radroots.seller_stock.issuance_projection.v1\0");
        hash.update(actor.as_bytes());
        hash.update(entry.as_bytes());
        hash.update(self.operation.as_bytes());
        hash.update(self.recipient.as_bytes());
        hash.update([u8::from(self.evidence.is_some())]);
        if let Some(value) = self.evidence {
            hash.update(value);
        }
        hash.finalize().into()
    }
    pub async fn confirm(
        self,
        connection: &mut SqliteConnection,
        actor: PublicKey,
        entry: OperationId,
    ) -> Result<(), OperationError> {
        let actual = raw_projection(
            connection,
            actor.as_bytes(),
            entry.as_bytes(),
            if self.evidence.is_some() { 5 } else { 4 },
        )
        .await?;
        if actual != self.digest(actor, entry) {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(())
    }
}
pub(in crate::workspace::stock) async fn raw_projection(
    connection: &mut SqliteConnection,
    actor: &[u8; 32],
    entry: &[u8; 32],
    family: i64,
) -> Result<[u8; 32], OperationError> {
    let row = match family {
        4 => sqlx::query("SELECT CASE WHEN length(operation_id)=32 THEN operation_id END,CASE WHEN length(recipient)=32 THEN recipient END,NULL FROM stock_signing_claims WHERE actor=? AND entry_id=?").bind(actor.as_slice()).bind(entry.as_slice()).fetch_optional(connection).await,
        5 => sqlx::query("SELECT CASE WHEN length(operation_id)=32 THEN operation_id END,CASE WHEN length(recipient)=32 THEN recipient END,CASE WHEN length(evidence_digest)=32 THEN evidence_digest END FROM stock_signing_evidence WHERE actor=? AND entry_id=?").bind(actor.as_slice()).bind(entry.as_slice()).fetch_optional(connection).await,
        _ => return Err(OperationError::RecoveryRequired),
    }.map_err(|_|OperationError::RecoveryRequired)?.ok_or(OperationError::RecoveryRequired)?;
    let field = |index| {
        row.try_get::<Option<Vec<u8>>, _>(index)
            .map_err(|_| OperationError::RecoveryRequired)?
            .ok_or(OperationError::RecoveryRequired)?
            .try_into()
            .map_err(|_| OperationError::RecoveryRequired)
    };
    let projection = Projection {
        operation: OperationId::from_bytes(field(0)?),
        recipient: PublicKey::from_bytes(field(1)?)
            .map_err(|_| OperationError::RecoveryRequired)?,
        evidence: if family == 5 { Some(field(2)?) } else { None },
    };
    Ok(projection.digest(
        PublicKey::from_bytes(*actor).map_err(|_| OperationError::RecoveryRequired)?,
        OperationId::from_bytes(*entry),
    ))
}
