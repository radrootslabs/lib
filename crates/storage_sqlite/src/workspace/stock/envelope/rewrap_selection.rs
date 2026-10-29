//! Bounded natural-index witness, including rows outside the protected chain.
use super::*;
#[derive(Eq, PartialEq)]
pub(in crate::workspace::stock::envelope) struct SelectionWitness {
    pub count: usize,
    digest: [u8; 32],
}
impl SelectionWitness {
    pub(in crate::workspace::stock::envelope) async fn read(
        connection: &mut SqliteConnection,
        request: &StockEnvelopeRequest,
    ) -> Result<Self, OperationError> {
        let admissions = sqlx::query("SELECT CASE WHEN length(r.request_id)=32 THEN r.request_id END,CASE WHEN length(r.entry_id)=32 THEN r.entry_id END,o.family,CASE WHEN length(o.related_operation)=32 THEN o.related_operation END FROM stock_rewrap_admissions r LEFT JOIN stock_operations o ON o.actor=r.actor AND o.operation_id=r.entry_id WHERE r.actor=? AND r.operation_id=? AND r.recipient=? ORDER BY r.request_id,r.entry_id LIMIT 193")
            .bind(request.binding().input().participant().as_bytes().as_slice())
            .bind(request.operation_id().as_bytes().as_slice()).bind(request.recipient().as_bytes().as_slice())
            .fetch_all(&mut *connection).await.map_err(|_| OperationError::RecoveryRequired)?;
        let captures = sqlx::query("SELECT CASE WHEN length(r.event_id)=32 THEN r.event_id END,CASE WHEN length(r.entry_id)=32 THEN r.entry_id END,o.family,CASE WHEN length(o.related_operation)=32 THEN o.related_operation END,CASE WHEN length(r.admission_entry)=32 THEN r.admission_entry END,CASE WHEN length(a.entry_id)=32 THEN a.entry_id END FROM stock_rewraps r LEFT JOIN stock_operations o ON o.actor=r.actor AND o.operation_id=r.entry_id LEFT JOIN stock_rewrap_admissions a ON a.actor=r.actor AND a.operation_id=r.operation_id AND a.recipient=r.recipient AND a.entry_id=r.admission_entry WHERE r.actor=? AND r.operation_id=? AND r.recipient=? ORDER BY r.event_id,r.entry_id LIMIT 193")
            .bind(request.binding().input().participant().as_bytes().as_slice())
            .bind(request.operation_id().as_bytes().as_slice()).bind(request.recipient().as_bytes().as_slice())
            .fetch_all(connection).await.map_err(|_| OperationError::RecoveryRequired)?;
        if admissions.len() > COUNT_MAX as usize || captures.len() > admissions.len() {
            return Err(OperationError::RecoveryRequired);
        }
        let mut hash = Sha256::new();
        hash.update(b"radroots.seller_stock.rewrap_selection.v1\0");
        for (rows, family) in [(&admissions, 13), (&captures, 14)] {
            hash.update([family as u8]);
            hash.update((rows.len() as u16).to_be_bytes());
            for row in rows {
                if row
                    .try_get::<Option<i64>, _>(2)
                    .map_err(|_| OperationError::RecoveryRequired)?
                    != Some(family)
                    || row
                        .try_get::<Option<Vec<u8>>, _>(3)
                        .map_err(|_| OperationError::RecoveryRequired)?
                        .as_deref()
                        != Some(request.operation_id().as_bytes().as_slice())
                {
                    return Err(OperationError::RecoveryRequired);
                }
                for index in 0..2 {
                    let bytes: Vec<u8> = row
                        .try_get(index)
                        .map_err(|_| OperationError::RecoveryRequired)?;
                    if bytes.len() != 32 {
                        return Err(OperationError::RecoveryRequired);
                    }
                    hash.update(bytes);
                }
                if family == 14 {
                    let admission: Vec<u8> = row
                        .try_get(4)
                        .map_err(|_| OperationError::RecoveryRequired)?;
                    let matched: Option<Vec<u8>> = row
                        .try_get(5)
                        .map_err(|_| OperationError::RecoveryRequired)?;
                    if admission.len() != 32 || matched.as_deref() != Some(admission.as_slice()) {
                        return Err(OperationError::RecoveryRequired);
                    }
                    hash.update(admission);
                }
            }
        }
        Ok(Self {
            count: admissions.len(),
            digest: hash.finalize().into(),
        })
    }
    pub(in crate::workspace::stock::envelope) async fn confirm(
        &self,
        connection: &mut SqliteConnection,
        request: &StockEnvelopeRequest,
    ) -> Result<(), OperationError> {
        if Self::read(connection, request).await? != *self {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(())
    }
}
