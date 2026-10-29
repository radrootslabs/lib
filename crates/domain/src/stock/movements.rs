//! Pure movement proposals; durable admission and original allocation proof are external.
use super::{PoolBalances, StockError, StockTicks};

/// Physical backing held for an original operation or installed obligation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HeldStockCategory {
    /// Original preparation whose issuance is not resolved.
    Preparatory,
    /// Remaining backing of an installed commitment.
    Committed,
    /// Backing retained under unresolved signature or business risk.
    Quarantined,
}

/// Depletion of genuinely present held stock, without deleting its obligation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HeldStockLoss {
    /// Move the physical quantity into cumulative waste.
    Spoilage,
    /// Move the physical quantity into cumulative removed stock.
    Removal,
}

enum Destination {
    Available,
    Committed,
    Quarantined,
    Consumed,
    Waste,
    Removed,
}

impl PoolBalances {
    /// Proposes release of authorized unconsumed committed backing. The executor
    /// must prove the exact residual and bilateral authority and atomically
    /// retain the release movement; a timeout or missing ACK proves neither.
    pub fn release_committed(self, quantity: StockTicks) -> Result<Self, StockError> {
        self.transfer_held(
            HeldStockCategory::Committed,
            Destination::Available,
            quantity,
        )
    }

    /// Proposes physical consumption of original committed backing. Exactly-once
    /// handover identity, authority, residual and version checks belong to the
    /// atomic executor. Buyer receipts must never call this movement again.
    pub fn consume_committed(self, quantity: StockTicks) -> Result<Self, StockError> {
        self.transfer_held(
            HeldStockCategory::Committed,
            Destination::Consumed,
            quantity,
        )
    }

    /// Proposes retention of preparatory backing under unresolved issuance risk.
    /// Expiry cannot release it or establish nonissuance.
    pub fn quarantine_preparatory(self, quantity: StockTicks) -> Result<Self, StockError> {
        self.transfer_held(
            HeldStockCategory::Preparatory,
            Destination::Quarantined,
            quantity,
        )
    }

    /// Proposes retention of installed backing under unresolved risk. Original
    /// allocation and obligation linkage must survive the actual transaction.
    pub fn quarantine_committed(self, quantity: StockTicks) -> Result<Self, StockError> {
        self.transfer_held(
            HeldStockCategory::Committed,
            Destination::Quarantined,
            quantity,
        )
    }

    /// Proposes restoration to committed backing after qualified reconciliation.
    /// This value neither resolves unknown issuance nor authorizes a retry.
    pub fn restore_quarantined(self, quantity: StockTicks) -> Result<Self, StockError> {
        self.transfer_held(
            HeldStockCategory::Quarantined,
            Destination::Committed,
            quantity,
        )
    }

    /// Proposes a release after original nonissuance or authorized residual
    /// resolution is proven. An expired lease, query or absent record is no proof.
    pub fn release_quarantined(self, quantity: StockTicks) -> Result<Self, StockError> {
        self.transfer_held(
            HeldStockCategory::Quarantined,
            Destination::Available,
            quantity,
        )
    }

    /// Proposes consumption after original handover evidence is reconciled. The
    /// executor must install the same handover once, retaining its original claim.
    pub fn consume_quarantined(self, quantity: StockTicks) -> Result<Self, StockError> {
        self.transfer_held(
            HeldStockCategory::Quarantined,
            Destination::Consumed,
            quantity,
        )
    }

    /// Proposes depletion of actually present backing. It preserves lifetime
    /// introduced stock and never releases capacity. The executor must retain
    /// affected original claims and reconcile any outstanding obligation to a
    /// deficit in the same transaction; this physical value cannot delete it.
    pub fn deplete_held(
        self,
        category: HeldStockCategory,
        loss: HeldStockLoss,
        quantity: StockTicks,
    ) -> Result<Self, StockError> {
        let destination = match loss {
            HeldStockLoss::Spoilage => Destination::Waste,
            HeldStockLoss::Removal => Destination::Removed,
        };
        self.transfer_held(category, destination, quantity)
    }

    fn transfer_held(
        self,
        category: HeldStockCategory,
        destination: Destination,
        quantity: StockTicks,
    ) -> Result<Self, StockError> {
        if quantity == StockTicks::ZERO {
            return Err(StockError::ZeroMovement);
        }
        let mut physical = self.physical();
        let source = match category {
            HeldStockCategory::Preparatory => &mut physical.preparatory_hold,
            HeldStockCategory::Committed => &mut physical.committed_hold,
            HeldStockCategory::Quarantined => &mut physical.physical_quarantine,
        };
        *source = source.checked_sub(quantity)?;
        let target = match destination {
            Destination::Available => &mut physical.available,
            Destination::Committed => &mut physical.committed_hold,
            Destination::Quarantined => &mut physical.physical_quarantine,
            Destination::Consumed => &mut physical.consumed,
            Destination::Waste => &mut physical.waste,
            Destination::Removed => &mut physical.removed,
        };
        *target = target.checked_add(quantity)?;
        Self::new(self.introduced(), physical)
    }
}
