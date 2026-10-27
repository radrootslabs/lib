use super::*;
/// Immutable body-derived selector, independently compared with its SQL index.
#[derive(Clone, Copy)]
pub(in crate::workspace::stock) struct Projection {
    pub operation: OperationId,
    pub recipient: PublicKey,
    pub kind: ProjectionKind,
}
#[derive(Clone, Copy)]
pub(in crate::workspace::stock) enum ProjectionKind {
    Claim,
    Obtained([u8; 32]),
    OriginalUnissued(u8),
    ReplayAdmission,
}
impl ProjectionKind {
    fn family(self) -> i64 {
        match self {
            Self::Claim => 4,
            Self::Obtained(_) => 5,
            Self::OriginalUnissued(_) => 8,
            Self::ReplayAdmission => 9,
        }
    }
}
impl Projection {
    pub fn digest(self, actor: PublicKey, entry: OperationId) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(b"radroots.seller_stock.issuance_projection.v1\0");
        hash.update(actor.as_bytes());
        hash.update(entry.as_bytes());
        hash.update(self.operation.as_bytes());
        hash.update(self.recipient.as_bytes());
        match self.kind {
            ProjectionKind::Claim => hash.update([0]),
            ProjectionKind::ReplayAdmission => hash.update([3]),
            ProjectionKind::Obtained(value) => {
                hash.update([1]);
                hash.update(value);
            }
            ProjectionKind::OriginalUnissued(reason) => {
                hash.update([2]);
                hash.update([reason]);
            }
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
            self.kind.family(),
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
        8 => sqlx::query("SELECT CASE WHEN length(operation_id)=32 THEN operation_id END,CASE WHEN length(recipient)=32 THEN recipient END,reason FROM stock_original_unissued WHERE actor=? AND entry_id=?").bind(actor.as_slice()).bind(entry.as_slice()).fetch_optional(connection).await,
        9 => sqlx::query("SELECT CASE WHEN length(operation_id)=32 THEN operation_id END,CASE WHEN length(recipient)=32 THEN recipient END,NULL FROM stock_replay_admissions WHERE actor=? AND entry_id=?").bind(actor.as_slice()).bind(entry.as_slice()).fetch_optional(connection).await,
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
        kind: match family {
            4 => ProjectionKind::Claim,
            9 => ProjectionKind::ReplayAdmission,
            5 => ProjectionKind::Obtained(field(2)?),
            8 => {
                let reason: u8 = row
                    .try_get::<i64, _>(2)
                    .ok()
                    .and_then(|v| v.try_into().ok())
                    .filter(|v| (1..=9).contains(v))
                    .ok_or(OperationError::RecoveryRequired)?;
                ProjectionKind::OriginalUnissued(reason)
            }
            _ => return Err(OperationError::RecoveryRequired),
        },
    };
    Ok(projection.digest(
        PublicKey::from_bytes(*actor).map_err(|_| OperationError::RecoveryRequired)?,
        OperationId::from_bytes(*entry),
    ))
}
