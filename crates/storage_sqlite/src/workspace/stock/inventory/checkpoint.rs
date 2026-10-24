use super::*;

/// Constant-memory witness of the exact protected ancestry checked before IO
/// enters the write transaction. The transaction rereads every bound SQL row;
/// no key-provider callback or decryption runs while it owns the SQL lock.
pub(super) struct MinimumProof {
    lower: u64,
    upper: u64,
    digest: [u8; 32],
}
impl MinimumProof {
    pub(super) async fn confirm(
        &self,
        connection: &mut SqliteConnection,
    ) -> Result<(), OperationError> {
        let mut hash = history_hash();
        for revision in (self.lower..=self.upper).rev() {
            let row=sqlx::query("SELECT actor,grant_id,operation_id,command_digest,transaction_id,pool_id,chain_head,CASE WHEN length(protected) BETWEEN 1 AND 262144 THEN protected END FROM stock_operations WHERE revision=?")
                .bind(revision as i64).fetch_optional(&mut *connection).await.map_err(|_|OperationError::RecoveryRequired)?.ok_or(OperationError::RecoveryRequired)?;
            hash.update(revision.to_be_bytes());
            for column in 0..7 {
                let bytes: Vec<u8> = row
                    .try_get(column)
                    .map_err(|_| OperationError::RecoveryRequired)?;
                if bytes.len() != 32 {
                    return Err(OperationError::RecoveryRequired);
                }
                hash.update(bytes);
            }
            let protected: Vec<u8> = row
                .try_get::<Option<Vec<u8>>, _>(7)
                .map_err(|_| OperationError::RecoveryRequired)?
                .ok_or(OperationError::RecoveryRequired)?;
            hash.update((protected.len() as u64).to_be_bytes());
            hash.update(protected);
        }
        let actual: [u8; 32] = hash.finalize().into();
        if actual != self.digest {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(())
    }
}
fn history_hash() -> Sha256 {
    let mut hash = Sha256::new();
    hash.update(b"radroots.seller_stock.ancestry_snapshot.v1\0");
    hash
}
pub(super) async fn authenticate_minimum(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    binding: &WorkspaceBinding,
    current: InventoryCheckpoint,
    minimum: radroots_storage::inventory::InventoryCheckpointRef,
) -> Result<MinimumProof, OperationError> {
    if minimum.authority() != input.authority() {
        return Err(WorkspaceError::StaleAuthority.into());
    }
    if minimum.revision().get() > current.revision.get() {
        return Err(WorkspaceError::StaleCheckpoint.into());
    }
    let lower = minimum.revision().get().max(1);
    let upper = current.revision.get();
    let mut hash = history_hash();
    let mut expected = current.head;
    for revision in (lower..=upper).rev() {
        let id:Vec<u8>=sqlx::query_scalar("SELECT operation_id FROM stock_operations WHERE revision=? AND length(operation_id)=32")
            .bind(revision as i64).fetch_optional(&mut *connection).await.map_err(|_|OperationError::RecoveryRequired)?.ok_or(OperationError::RecoveryRequired)?;
        let operation = OperationId::from_bytes(
            id.try_into()
                .map_err(|_| OperationError::RecoveryRequired)?,
        );
        let retained = read_operation(connection, input, crypto, binding, operation)
            .await?
            .ok_or(OperationError::RecoveryRequired)?;
        let receipt = &retained.receipt;
        if receipt.checkpoint().revision().get() != revision
            || receipt.checkpoint().head() != expected
        {
            return Err(WorkspaceError::StaleCheckpoint.into());
        }
        if revision == minimum.revision().get() && expected != minimum.head() {
            return Err(WorkspaceError::StaleCheckpoint.into());
        }
        hash.update(revision.to_be_bytes());
        hash.update(binding.actor().participant().as_bytes());
        hash.update(binding.input().grant_id().as_bytes());
        hash.update(operation.as_bytes());
        hash.update(retained.digest.as_bytes());
        hash.update(
            receipt
                .transaction_id()
                .ok_or(OperationError::RecoveryRequired)?
                .as_bytes(),
        );
        hash.update(receipt.pool().id().as_bytes());
        hash.update(expected.ok_or(OperationError::RecoveryRequired)?.as_bytes());
        hash.update((retained.protected.len() as u64).to_be_bytes());
        hash.update(&retained.protected);
        expected = retained.previous_head;
    }
    if minimum.revision().get() == 0 && expected.is_some() {
        return Err(WorkspaceError::StaleCheckpoint.into());
    }
    Ok(MinimumProof {
        lower,
        upper,
        digest: hash.finalize().into(),
    })
}
