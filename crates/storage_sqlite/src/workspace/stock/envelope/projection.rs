//! Exact body-derived original, wrapping-admission and capture indexes.
use super::*;
#[derive(Clone, Copy)]
pub(in crate::workspace::stock) enum Projection {
    Original {
        operation: OperationId,
        recipient: PublicKey,
    },
    Admission {
        operation: OperationId,
        recipient: PublicKey,
        request: OperationId,
    },
    Rewrap {
        operation: OperationId,
        recipient: PublicKey,
        admission: OperationId,
        event: EnvelopeEventId,
    },
}
impl Projection {
    pub(in crate::workspace::stock) fn digest(
        self,
        actor: PublicKey,
        entry: OperationId,
    ) -> [u8; 32] {
        let mut hash = Sha256::new();
        let (operation, recipient) = match self {
            Self::Original {
                operation,
                recipient,
            } => {
                hash.update(b"radroots.seller_stock.envelope_projection.v1\0");
                (operation, recipient)
            }
            Self::Admission {
                operation,
                recipient,
                ..
            } => {
                hash.update(b"radroots.seller_stock.rewrap_admission_projection.v1\0");
                (operation, recipient)
            }
            Self::Rewrap {
                operation,
                recipient,
                ..
            } => {
                hash.update(b"radroots.seller_stock.rewrap_projection.v1\0");
                (operation, recipient)
            }
        };
        hash.update(actor.as_bytes());
        hash.update(entry.as_bytes());
        hash.update(operation.as_bytes());
        hash.update(recipient.as_bytes());
        match self {
            Self::Original { .. } => {}
            Self::Admission { request, .. } => hash.update(request.as_bytes()),
            Self::Rewrap {
                admission, event, ..
            } => {
                hash.update(admission.as_bytes());
                hash.update(event.as_bytes());
            }
        }
        hash.finalize().into()
    }
    pub(super) async fn insert(
        self,
        connection: &mut SqliteConnection,
        actor: PublicKey,
        entry: OperationId,
    ) -> Result<(), OperationError> {
        match self {
            Self::Original {
                operation,
                recipient,
            } => {
                sqlx::query("INSERT INTO stock_envelopes(actor,operation_id,recipient,entry_id) VALUES(?,?,?,?)")
                    .bind(actor.as_bytes().as_slice()).bind(operation.as_bytes().as_slice()).bind(recipient.as_bytes().as_slice()).bind(entry.as_bytes().as_slice())
                    .execute(connection).await.map_err(|_| OperationError::StorageFailure)?;
            }
            Self::Admission {
                operation,
                recipient,
                request,
            } => {
                sqlx::query("INSERT INTO stock_rewrap_admissions(actor,operation_id,recipient,request_id,entry_id) VALUES(?,?,?,?,?)")
                    .bind(actor.as_bytes().as_slice()).bind(operation.as_bytes().as_slice()).bind(recipient.as_bytes().as_slice())
                    .bind(request.as_bytes().as_slice()).bind(entry.as_bytes().as_slice()).execute(connection).await.map_err(|_| OperationError::StorageFailure)?;
            }
            Self::Rewrap {
                operation,
                recipient,
                admission,
                event,
            } => {
                sqlx::query("INSERT INTO stock_rewraps(actor,operation_id,recipient,admission_entry,event_id,entry_id) VALUES(?,?,?,?,?,?)")
                    .bind(actor.as_bytes().as_slice()).bind(operation.as_bytes().as_slice()).bind(recipient.as_bytes().as_slice())
                    .bind(admission.as_bytes().as_slice()).bind(event.as_bytes().as_slice()).bind(entry.as_bytes().as_slice()).execute(connection).await.map_err(|_| OperationError::StorageFailure)?;
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
        let found = match self {
            Self::Original { .. } => {
                raw_projection(connection, actor.as_bytes(), entry.as_bytes()).await?
            }
            Self::Admission { .. } => {
                raw_admission_projection(connection, actor.as_bytes(), entry.as_bytes()).await?
            }
            Self::Rewrap { .. } => {
                raw_rewrap_projection(connection, actor.as_bytes(), entry.as_bytes()).await?
            }
        };
        if found != self.digest(actor, entry) {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(())
    }
}
fn bytes(row: &sqlx::sqlite::SqliteRow, index: usize) -> Result<[u8; 32], OperationError> {
    row.try_get::<Vec<u8>, _>(index)
        .map_err(|_| OperationError::RecoveryRequired)?
        .try_into()
        .map_err(|_| OperationError::RecoveryRequired)
}
pub(in crate::workspace::stock) async fn raw_projection(
    connection: &mut SqliteConnection,
    actor: &[u8; 32],
    entry: &[u8; 32],
) -> Result<[u8; 32], OperationError> {
    let row = sqlx::query(
        "SELECT CASE WHEN length(operation_id)=32 THEN operation_id END,CASE WHEN length(recipient)=32 THEN recipient END FROM stock_envelopes WHERE actor=? AND entry_id=?",
    )
    .bind(actor.as_slice())
    .bind(entry.as_slice())
    .fetch_optional(connection)
    .await
    .map_err(|_| OperationError::RecoveryRequired)?
    .ok_or(OperationError::RecoveryRequired)?;
    Ok(Projection::Original {
        operation: OperationId::from_bytes(bytes(&row, 0)?),
        recipient: PublicKey::from_bytes(bytes(&row, 1)?)
            .map_err(|_| OperationError::RecoveryRequired)?,
    }
    .digest(
        PublicKey::from_bytes(*actor).map_err(|_| OperationError::RecoveryRequired)?,
        OperationId::from_bytes(*entry),
    ))
}
pub(in crate::workspace::stock) async fn raw_admission_projection(
    connection: &mut SqliteConnection,
    actor: &[u8; 32],
    entry: &[u8; 32],
) -> Result<[u8; 32], OperationError> {
    let row = sqlx::query("SELECT CASE WHEN length(operation_id)=32 THEN operation_id END,CASE WHEN length(recipient)=32 THEN recipient END,CASE WHEN length(request_id)=32 THEN request_id END FROM stock_rewrap_admissions WHERE actor=? AND entry_id=?")
        .bind(actor.as_slice()).bind(entry.as_slice()).fetch_optional(connection).await.map_err(|_| OperationError::RecoveryRequired)?.ok_or(OperationError::RecoveryRequired)?;
    Ok(Projection::Admission {
        operation: OperationId::from_bytes(bytes(&row, 0)?),
        recipient: PublicKey::from_bytes(bytes(&row, 1)?)
            .map_err(|_| OperationError::RecoveryRequired)?,
        request: OperationId::from_bytes(bytes(&row, 2)?),
    }
    .digest(
        PublicKey::from_bytes(*actor).map_err(|_| OperationError::RecoveryRequired)?,
        OperationId::from_bytes(*entry),
    ))
}
pub(in crate::workspace::stock) async fn raw_rewrap_projection(
    connection: &mut SqliteConnection,
    actor: &[u8; 32],
    entry: &[u8; 32],
) -> Result<[u8; 32], OperationError> {
    let row = sqlx::query("SELECT CASE WHEN length(operation_id)=32 THEN operation_id END,CASE WHEN length(recipient)=32 THEN recipient END,CASE WHEN length(admission_entry)=32 THEN admission_entry END,CASE WHEN length(event_id)=32 THEN event_id END FROM stock_rewraps WHERE actor=? AND entry_id=?")
        .bind(actor.as_slice()).bind(entry.as_slice()).fetch_optional(connection).await.map_err(|_| OperationError::RecoveryRequired)?.ok_or(OperationError::RecoveryRequired)?;
    Ok(Projection::Rewrap {
        operation: OperationId::from_bytes(bytes(&row, 0)?),
        recipient: PublicKey::from_bytes(bytes(&row, 1)?)
            .map_err(|_| OperationError::RecoveryRequired)?,
        admission: OperationId::from_bytes(bytes(&row, 2)?),
        event: EnvelopeEventId::from_bytes(bytes(&row, 3)?),
    }
    .digest(
        PublicKey::from_bytes(*actor).map_err(|_| OperationError::RecoveryRequired)?,
        OperationId::from_bytes(*entry),
    ))
}
