use super::{InventoryCheckpointRef, OperationError};
use radroots_domain::stock::{ObligationBacking, PoolView};

/// Current selected-executor physical stock and its pool-backed installed
/// obligations at one confirmed inventory checkpoint. Preparatory claims and
/// never-stock-backed future promises are not installed obligation totals.
/// This metadata grants no authority, completeness beyond this pool, or effect.
#[derive(Clone, Eq, PartialEq)]
pub struct StockSnapshot {
    pool: PoolView,
    installed: ObligationBacking,
    checkpoint: InventoryCheckpointRef,
}
impl StockSnapshot {
    /// Reconstructs checked metadata only. The qualified store must authenticate
    /// original allocations, current backing, permissions and the final fence.
    pub fn from_host_metadata(
        pool: PoolView,
        installed: ObligationBacking,
        checkpoint: InventoryCheckpointRef,
    ) -> Result<Self, OperationError> {
        let physical = pool.snapshot().balances().physical();
        let held = physical
            .committed_hold
            .checked_add(physical.physical_quarantine)
            .map_err(|_| OperationError::RecoveryRequired)?;
        if pool.snapshot().version().get() == 0
            || checkpoint.revision().get() == 0
            || installed.remaining() > pool.snapshot().balances().introduced()
            || installed.backed() > held
        {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(Self {
            pool,
            installed,
            checkpoint,
        })
    }
    pub const fn pool(&self) -> &PoolView {
        &self.pool
    }
    /// Remaining = backed + deficit for known pool-backed installed obligations.
    /// Pending issuance risk remains represented by the physical held categories.
    pub const fn installed_obligations(&self) -> ObligationBacking {
        self.installed
    }
    pub const fn checkpoint(&self) -> InventoryCheckpointRef {
        self.checkpoint
    }
}
impl core::fmt::Debug for StockSnapshot {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("StockSnapshot(<private current metadata>)")
    }
}
