//! Shared explicit cleanup selectors and reports, without backend qualification.
use super::*;
use crate::workspace::AttachmentInput;
use radroots_domain::{OrderId, UnixSeconds};

/// Original-operation selectors. These values supply no nonissuance proof.
pub struct StockAbortRequest {
    binding: WorkspaceBinding,
    minimum: InventoryCheckpointRef,
    operation: OperationId,
    digest: CommandDigest,
}
impl StockAbortRequest {
    pub const fn new(
        binding: WorkspaceBinding,
        minimum: InventoryCheckpointRef,
        operation: OperationId,
        digest: CommandDigest,
    ) -> Self {
        Self {
            binding,
            minimum,
            operation,
            digest,
        }
    }
    pub const fn binding(&self) -> &WorkspaceBinding {
        &self.binding
    }
    pub const fn minimum_checkpoint(&self) -> InventoryCheckpointRef {
        self.minimum
    }
    pub const fn operation_id(&self) -> OperationId {
        self.operation
    }
    pub const fn command_digest(&self) -> CommandDigest {
        self.digest
    }
    /// Original recovery metadata, never evidence of admission or completion.
    pub fn attempt(&self) -> CommitAttempt {
        CommitAttempt::new(
            self.binding.input().workspace_id(),
            self.operation,
            self.digest,
            None,
        )
    }
}
impl core::fmt::Debug for StockAbortRequest {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("StockAbortRequest(<redacted>)")
    }
}

/// Reported original cleanup, distinct from cancellation or delivery.
pub struct StockAbortReceipt {
    attempt: CommitAttempt,
    order: OrderId,
    at: UnixSeconds,
    checkpoint: InventoryCheckpointRef,
    pools: Vec<PoolSnapshot>,
}
impl StockAbortReceipt {
    /// Checks bounded report metadata only. A qualified store must authenticate
    /// the original absence and effects and confirm its actual atomic commit.
    pub fn from_committed_store(
        namespace: &AttachmentInput,
        attempt: CommitAttempt,
        order: OrderId,
        at: UnixSeconds,
        checkpoint: InventoryCheckpointRef,
        pools: Vec<PoolSnapshot>,
    ) -> Result<Self, OperationError> {
        if attempt.workspace_id() != namespace.workspace_id()
            || attempt.transaction_id().is_none()
            || checkpoint.authority() != namespace.authority()
            || checkpoint.revision().get() == 0
            || checkpoint.authorization().is_none()
            || pools.len() > 64
            || pools.iter().any(|pool| pool.version().get() < 3)
            || pools.windows(2).any(|pair| pair[0].id() >= pair[1].id())
        {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(Self {
            attempt,
            order,
            at,
            checkpoint,
            pools,
        })
    }
    pub const fn attempt(&self) -> CommitAttempt {
        self.attempt
    }
    pub const fn order_id(&self) -> OrderId {
        self.order
    }
    pub const fn aborted_at(&self) -> UnixSeconds {
        self.at
    }
    pub const fn checkpoint(&self) -> InventoryCheckpointRef {
        self.checkpoint
    }
    pub fn released_pools(&self) -> &[PoolSnapshot] {
        &self.pools
    }
    /// Transfers the original reported after-images without another copy.
    pub fn into_released_pools(self) -> Vec<PoolSnapshot> {
        self.pools
    }
}
impl core::fmt::Debug for StockAbortReceipt {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("StockAbortReceipt(<redacted>)")
    }
}
#[derive(Debug)]
pub enum StockAbortOutcome {
    Aborted(Box<StockAbortReceipt>),
    Unknown(CommitAttempt),
}
