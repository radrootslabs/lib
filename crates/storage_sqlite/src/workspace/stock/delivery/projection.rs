//! Exact body-derived indexes; projections cannot supply dispatch permission.
use super::*;

#[derive(Clone, Copy)]
pub(in crate::workspace::stock) enum Projection {
    Attempt {
        operation: OperationId,
        recipient: PublicKey,
        request: [u8; 32],
    },
    Observation {
        operation: OperationId,
        recipient: PublicKey,
        request: [u8; 32],
        target: [u8; 32],
        kind: u8,
    },
}
impl Projection {
    pub(in crate::workspace::stock) fn digest(
        self,
        actor: PublicKey,
        entry: OperationId,
    ) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(b"radroots.seller_stock.delivery_projection.v1\0");
        hash.update(actor.as_bytes());
        hash.update(entry.as_bytes());
        let (operation, recipient, request) = match self {
            Self::Attempt {
                operation,
                recipient,
                request,
            } => {
                hash.update([11]);
                (operation, recipient, request)
            }
            Self::Observation {
                operation,
                recipient,
                request,
                target,
                kind,
            } => {
                hash.update([12]);
                hash.update(target);
                hash.update([kind]);
                (operation, recipient, request)
            }
        };
        hash.update(operation.as_bytes());
        hash.update(recipient.as_bytes());
        hash.update(request);
        hash.finalize().into()
    }
    pub(super) async fn insert(
        self,
        connection: &mut SqliteConnection,
        actor: PublicKey,
        entry: OperationId,
    ) -> Result<(), OperationError> {
        match self {
            Self::Attempt {
                operation,
                recipient,
                request,
            } => {
                sqlx::query("INSERT INTO stock_delivery_attempts(actor,operation_id,recipient,request_key,entry_id) VALUES(?,?,?,?,?)")
                    .bind(actor.as_bytes().as_slice()).bind(operation.as_bytes().as_slice())
                    .bind(recipient.as_bytes().as_slice()).bind(request.as_slice())
                    .bind(entry.as_bytes().as_slice()).execute(connection).await
                    .map_err(|_|OperationError::StorageFailure)?;
            }
            Self::Observation {
                operation,
                recipient,
                request,
                target,
                kind,
            } => {
                sqlx::query("INSERT INTO stock_delivery_observations(actor,operation_id,recipient,request_key,target_key,kind,entry_id) VALUES(?,?,?,?,?,?,?)")
                    .bind(actor.as_bytes().as_slice()).bind(operation.as_bytes().as_slice())
                    .bind(recipient.as_bytes().as_slice()).bind(request.as_slice()).bind(target.as_slice())
                    .bind(i64::from(kind)).bind(entry.as_bytes().as_slice()).execute(connection).await
                    .map_err(|_|OperationError::StorageFailure)?;
            }
        }
        Ok(())
    }
    pub(super) async fn confirm(
        self,
        connection: &mut SqliteConnection,
        actor: PublicKey,
        entry: OperationId,
    ) -> Result<(), OperationError> {
        let family = if matches!(self, Self::Attempt { .. }) {
            11
        } else {
            12
        };
        if raw_projection(connection, actor.as_bytes(), entry.as_bytes(), family).await?
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
    family: i64,
) -> Result<[u8; 32], OperationError> {
    let sql = match family {
        11 => {
            "SELECT operation_id,recipient,request_key,NULL,NULL FROM stock_delivery_attempts WHERE actor=? AND entry_id=?"
        }
        12 => {
            "SELECT operation_id,recipient,request_key,target_key,kind FROM stock_delivery_observations WHERE actor=? AND entry_id=?"
        }
        _ => return Err(OperationError::RecoveryRequired),
    };
    let row = sqlx::query(sql)
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
    let operation = OperationId::from_bytes(field(0)?);
    let recipient =
        PublicKey::from_bytes(field(1)?).map_err(|_| OperationError::RecoveryRequired)?;
    let request = field(2)?;
    let projection = if family == 11 {
        Projection::Attempt {
            operation,
            recipient,
            request,
        }
    } else {
        let kind: i64 = row
            .try_get(4)
            .map_err(|_| OperationError::RecoveryRequired)?;
        if !(1..=6).contains(&kind) {
            return Err(OperationError::RecoveryRequired);
        }
        Projection::Observation {
            operation,
            recipient,
            request,
            target: field(3)?,
            kind: kind as u8,
        }
    };
    Ok(projection.digest(
        PublicKey::from_bytes(*actor).map_err(|_| OperationError::RecoveryRequired)?,
        OperationId::from_bytes(*entry),
    ))
}
