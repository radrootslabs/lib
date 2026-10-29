//! Outstanding obligations reconcile backing and deficits without creating stock.
use super::{StockError, StockTicks};
use core::fmt;

/// A checked remaining obligation in one fixed pool unit. Deficit is unbacked
/// obligation, never physical stock. Construction proves no original allocation,
/// permission, generation, causality or durable effect.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ObligationBacking {
    remaining: StockTicks,
    backed: StockTicks,
    deficit: StockTicks,
}

impl ObligationBacking {
    /// Requires exact remaining = backed + deficit within the stock tick bound.
    /// Zero remaining is allowed only with zero backing and zero deficit.
    pub fn new(
        remaining: StockTicks,
        backed: StockTicks,
        deficit: StockTicks,
    ) -> Result<Self, StockError> {
        if backed.checked_add(deficit)? != remaining {
            return Err(StockError::ObligationMismatch);
        }
        Ok(Self {
            remaining,
            backed,
            deficit,
        })
    }

    /// Exact outstanding quantity, independently of physical availability.
    pub const fn remaining(self) -> StockTicks {
        self.remaining
    }

    /// Outstanding quantity claimed to have retained physical backing.
    pub const fn backed(self) -> StockTicks {
        self.backed
    }

    /// Outstanding quantity whose physical backing is absent.
    pub const fn deficit(self) -> StockTicks {
        self.deficit
    }

    /// Proposes loss of original physical backing while retaining the entire
    /// obligation. The native executor must record the physical depletion and
    /// this deficit atomically against the same original allocation/version.
    pub fn lose_backing(self, quantity: StockTicks) -> Result<Self, StockError> {
        positive(quantity)?;
        Self::new(
            self.remaining,
            self.backed.checked_sub(quantity)?,
            self.deficit.checked_add(quantity)?,
        )
    }

    /// Proposes replacement backing for a known deficit. The executor must
    /// actually hold that stock; this value cannot acquire physical capacity.
    pub fn restore_backing(self, quantity: StockTicks) -> Result<Self, StockError> {
        positive(quantity)?;
        let deficit = self.deficit.checked_sub(quantity)?;
        Self::new(self.remaining, self.backed.checked_add(quantity)?, deficit)
    }

    /// Proposes fulfillment of backed residual only. The authoritative handover
    /// transaction consumes its physical backing once. A buyer receipt changes
    /// neither this obligation nor the physical balances a second time.
    pub fn consume_backed(self, quantity: StockTicks) -> Result<Self, StockError> {
        positive(quantity)?;
        Self::new(
            self.remaining.checked_sub(quantity)?,
            self.backed.checked_sub(quantity)?,
            self.deficit,
        )
    }

    /// Proposes a nonempty authorized residual release with explicit backed and
    /// deficit portions. Only the backed portion can return physical stock to
    /// available. Bilateral authority, exact residual and durable identity are
    /// checked by the executor, never inferred from this arithmetic.
    pub fn release_residual(
        self,
        backed: StockTicks,
        deficit: StockTicks,
    ) -> Result<Self, StockError> {
        let quantity = backed.checked_add(deficit)?;
        positive(quantity)?;
        Self::new(
            self.remaining.checked_sub(quantity)?,
            self.backed.checked_sub(backed)?,
            self.deficit.checked_sub(deficit)?,
        )
    }
}

fn positive(quantity: StockTicks) -> Result<(), StockError> {
    if quantity == StockTicks::ZERO {
        Err(StockError::ZeroMovement)
    } else {
        Ok(())
    }
}

impl fmt::Debug for ObligationBacking {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ObligationBacking(<private proposal>)")
    }
}
