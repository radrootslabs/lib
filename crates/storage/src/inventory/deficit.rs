use radroots_domain::{AgreementLineId, OperationId, PoolId, stock::ObligationBacking};

/// A retained correction's original-allocation deficit at that transaction.
/// This is historical effect metadata, not a current backing or authority proof.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct StockDeficit {
    acceptance: OperationId,
    installation: OperationId,
    line: AgreementLineId,
    pool: PoolId,
    backing: ObligationBacking,
}
impl StockDeficit {
    /// Reconstructs metadata only. The store must independently authenticate
    /// the original allocation, correction, transaction and current access.
    pub const fn from_host_metadata(
        acceptance: OperationId,
        installation: OperationId,
        line: AgreementLineId,
        pool: PoolId,
        backing: ObligationBacking,
    ) -> Self {
        Self {
            acceptance,
            installation,
            line,
            pool,
            backing,
        }
    }
    pub const fn acceptance(self) -> OperationId {
        self.acceptance
    }
    pub const fn installation(self) -> OperationId {
        self.installation
    }
    pub const fn line(self) -> AgreementLineId {
        self.line
    }
    pub const fn pool(self) -> PoolId {
        self.pool
    }
    pub const fn backing(self) -> ObligationBacking {
        self.backing
    }
}
impl core::fmt::Debug for StockDeficit {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("StockDeficit(<private effect metadata>)")
    }
}
