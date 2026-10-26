//! Durable per-recipient claims and original evidence on the stock journal.
use super::*;
use radroots_domain::{CommandDigest, OperationId, PublicKey, SigningAttemptId, UnixSeconds};
use radroots_storage::{
    command::{CommitAttempt, OperationError},
    inventory::InventoryCheckpointRef,
};
use sha2::{Digest, Sha256};
use sqlx::Row;
pub(in crate::workspace) mod claim;
pub(in crate::workspace) mod evidence;
mod projection;
pub(in crate::workspace) mod recovery;
pub(super) use claim::read_entry;
pub use claim::{StockClaimOutcome, StockClaimRequest, StockDispatchPermit, StockSigningAttempt};
pub use evidence::{
    StockCapturedEvidence, StockEvidenceCapture, StockEvidenceOutcome, StockEvidencePage,
    StockEvidenceReceipt,
};
pub(super) use projection::{Projection, raw_projection};
pub use recovery::StockExactReplayPermit;

pub(super) async fn check_counts(connection: &mut SqliteConnection) -> Result<(), WorkspaceError> {
    let catalog: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&mut *connection)
        .await
        .map_err(|_| WorkspaceError::RecoveryRequired)?;
    if catalog < 11 {
        return Ok(());
    }
    let (claims, entries): (i64,i64) = sqlx::query_as("SELECT (SELECT count(*) FROM stock_signing_claims),(SELECT count(*) FROM stock_operations WHERE family=4)").fetch_one(&mut *connection).await.map_err(|_|WorkspaceError::RecoveryRequired)?;
    let (evidence, captures): (i64,i64) = sqlx::query_as("SELECT (SELECT count(*) FROM stock_signing_evidence),(SELECT count(*) FROM stock_operations WHERE family=5)").fetch_one(&mut *connection).await.map_err(|_|WorkspaceError::RecoveryRequired)?;
    if claims != entries || evidence != captures {
        return Err(WorkspaceError::RecoveryRequired);
    }
    if catalog >= 12 {
        let (agreements, installs, work):(i64,i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM stock_agreements),(SELECT count(*) FROM stock_operations WHERE family=6),(SELECT count(*) FROM stock_publication_work)").fetch_one(&mut *connection).await.map_err(|_|WorkspaceError::RecoveryRequired)?;
        if agreements != installs
            || work != installs.checked_mul(2).ok_or(WorkspaceError::Capacity)?
        {
            return Err(WorkspaceError::RecoveryRequired);
        }
    }
    if catalog == 13 {
        let (aborts, entries, states, orders):(i64,i64,i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM stock_aborts),(SELECT count(*) FROM stock_operations WHERE family=7),(SELECT count(*) FROM stock_order_states),(SELECT count(DISTINCT order_id) FROM stock_orders)")
            .fetch_one(connection).await.map_err(|_|WorkspaceError::RecoveryRequired)?;
        if aborts != entries || states != orders {
            return Err(WorkspaceError::RecoveryRequired);
        }
    }
    Ok(())
}
