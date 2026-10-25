//! Fresh issuance checks current backing without freezing unrelated pool changes.
use super::*;
pub(in crate::workspace::stock) struct BackingProof {
    id: PoolId,
    version: Sequence,
    protected_digest: [u8; 32],
    ancestry: MinimumProof,
}
pub(in crate::workspace::stock) async fn read(
    turn: &mut Turn<'_>,
    access: &Access,
    pools: &[PoolSnapshot],
) -> Result<Vec<BackingProof>, OperationError> {
    if pools.len() > 96 {
        return Err(WorkspaceError::Capacity.into());
    }
    let mut proofs = Vec::with_capacity(pools.len());
    for held in pools {
        let current = read_pool(turn.connection, turn.selected, turn.crypto, held.id())
            .await?
            .ok_or(OperationError::RecoveryRequired)?;
        let ancestry = authenticate_projection(
            turn.connection,
            turn.selected,
            turn.crypto,
            &access.current,
            &access.authority,
            held.id(),
            Some(current.pool),
        )
        .await?;
        proofs.push(BackingProof {
            id: held.id(),
            version: current.pool.version(),
            protected_digest: Sha256::digest(&current.protected).into(),
            ancestry,
        });
    }
    Ok(proofs)
}
impl BackingProof {
    pub(in crate::workspace::stock) async fn confirm(
        &self,
        connection: &mut SqliteConnection,
    ) -> Result<(), OperationError> {
        self.ancestry.confirm(connection).await?;
        let row=sqlx::query("SELECT revision,CASE WHEN length(protected) BETWEEN 1 AND 262144 THEN protected END FROM stock_pools WHERE pool_id=?").bind(self.id.as_bytes().as_slice()).fetch_optional(connection).await.map_err(|_|OperationError::RecoveryRequired)?.ok_or(OperationError::RecoveryRequired)?;
        let version: i64 = row
            .try_get(0)
            .map_err(|_| OperationError::RecoveryRequired)?;
        let protected: Vec<u8> = row
            .try_get::<Option<Vec<u8>>, _>(1)
            .map_err(|_| OperationError::RecoveryRequired)?
            .ok_or(OperationError::RecoveryRequired)?;
        if version <= 0
            || version as u64 != self.version.get()
            || <[u8; 32]>::from(Sha256::digest(protected)) != self.protected_digest
        {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(())
    }
}
