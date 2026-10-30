//! One currently authenticated pool snapshot, with no signer or store effect.
use super::*;
use radroots_domain::stock::{ObligationBacking, StockTicks};
use radroots_storage::inventory::StockSnapshot;

impl Turn<'_> {
    pub async fn inspect(
        &mut self,
        binding: WorkspaceBinding,
        minimum: radroots_storage::inventory::InventoryCheckpointRef,
        id: PoolId,
        proof: AttachmentProof,
    ) -> Result<Option<StockSnapshot>, OperationError> {
        let mut access = self.access(&binding, minimum, &proof).await?;
        if access.authority.catalog != 19 {
            return Err(OperationError::UnsupportedOperation);
        }
        access.record.require(StockAction::ReadPools, id)?;
        access.record.require(StockAction::ReadOperations, id)?;
        let retained = read_pool(self.connection, self.selected, self.crypto, id).await?;
        access.projection_proof = Some(
            authenticate_projection(
                self.connection,
                self.selected,
                self.crypto,
                &access.current,
                &access.authority,
                id,
                retained.as_ref().map(|value| value.pool),
            )
            .await?,
        );
        let snapshot = if let Some(pool) = &retained {
            let current = access
                .authority
                .inventory
                .ok_or(OperationError::RecoveryRequired)?;
            let before_revision = current
                .revision
                .get()
                .checked_add(1)
                .ok_or(WorkspaceError::Capacity)?;
            let mut cursor = super::allocations::InstalledCursor::new(id, before_revision)?;
            let mut remaining = StockTicks::ZERO;
            let mut backed = StockTicks::ZERO;
            let mut deficit = StockTicks::ZERO;
            while let Some(allocation) =
                Box::pin(cursor.next(self.connection, self.selected, self.crypto)).await?
            {
                let value = allocation
                    .backing(self.connection, self.selected, self.crypto, before_revision)
                    .await?;
                remaining = remaining
                    .checked_add(value.remaining())
                    .map_err(|_| OperationError::RecoveryRequired)?;
                backed = backed
                    .checked_add(value.backed())
                    .map_err(|_| OperationError::RecoveryRequired)?;
                deficit = deficit
                    .checked_add(value.deficit())
                    .map_err(|_| OperationError::RecoveryRequired)?;
            }
            // Catalog19 has no quarantine/consumption/release producer. A
            // current report must match its actual installed physical backing.
            if backed != pool.pool.balances().physical().committed_hold
                || pool.pool.balances().physical().physical_quarantine != StockTicks::ZERO
            {
                return Err(OperationError::RecoveryRequired);
            }
            let checkpoint =
                radroots_storage::inventory::InventoryCheckpointRef::from_host_metadata(
                    self.selected.authority(),
                    current.revision,
                    current.head,
                )?
                .with_optional_authorization(access.authority.authorization);
            Some(StockSnapshot::from_host_metadata(
                pool.view.clone(),
                ObligationBacking::new(remaining, backed, deficit)
                    .map_err(|_| OperationError::RecoveryRequired)?,
                checkpoint,
            )?)
        } else {
            None
        };
        self.confirm(
            &access,
            &proof,
            None,
            retained.as_ref(),
            None,
            retained.is_none().then_some(id),
        )
        .await?;
        Ok(snapshot)
    }
}
