//! Shared T0 data and reported effects; concrete stores authenticate and commit.
use super::{CommitAttempt, InventoryCheckpointRef, OperationError};
use crate::workspace::{
    AttachmentInput, WorkspaceBinding,
    seller_stock::{StockListingCheckpointRef, StockSourceCheckpointRef},
};
use radroots_domain::{
    IssuanceWindow, OrderId, RecordId, TermsDigest, UnixSeconds, stock::PoolSnapshot,
};
use radroots_protocol::{
    command::{CapturedOrderAcceptance, CapturedStockCommitment, CapturedTermsInstallation},
    commerce::EncodedCommerceRumor,
};
use std::sync::Arc;

/// Captured inputs and independently retained minima, never authority evidence.
pub struct StockAcceptanceRequest {
    binding: WorkspaceBinding,
    minimum: InventoryCheckpointRef,
    sources: StockSourceCheckpointRef,
    listings: StockListingCheckpointRef,
    command: CapturedStockCommitment,
}
impl StockAcceptanceRequest {
    /// Copies bounded protected command bytes. Current consent, source policy,
    /// exact allocation, authority and actual T0 effects belong to the store.
    pub fn capture(
        binding: WorkspaceBinding,
        minimum: InventoryCheckpointRef,
        sources: StockSourceCheckpointRef,
        listings: StockListingCheckpointRef,
        command: &CapturedOrderAcceptance,
    ) -> Result<Self, OperationError> {
        Self::capture_wire(
            binding,
            minimum,
            sources,
            listings,
            command.protected_wire(),
        )
    }
    /// Copies exact terms-installation data into the same native commitment
    /// request. This performs no admission, allocation, signing or stock effect.
    pub fn capture_terms(
        binding: WorkspaceBinding,
        minimum: InventoryCheckpointRef,
        sources: StockSourceCheckpointRef,
        listings: StockListingCheckpointRef,
        command: &CapturedTermsInstallation,
    ) -> Result<Self, OperationError> {
        Self::capture_wire(
            binding,
            minimum,
            sources,
            listings,
            command.protected_wire(),
        )
    }
    fn capture_wire(
        binding: WorkspaceBinding,
        minimum: InventoryCheckpointRef,
        sources: StockSourceCheckpointRef,
        listings: StockListingCheckpointRef,
        raw: &[u8],
    ) -> Result<Self, OperationError> {
        let command =
            CapturedStockCommitment::parse(raw).map_err(|_| OperationError::InvalidCommand)?;
        Ok(Self {
            binding,
            minimum,
            sources,
            listings,
            command,
        })
    }
    pub const fn binding(&self) -> &WorkspaceBinding {
        &self.binding
    }
    pub const fn minimum_checkpoint(&self) -> InventoryCheckpointRef {
        self.minimum
    }
    pub const fn source_checkpoint(&self) -> StockSourceCheckpointRef {
        self.sources
    }
    pub const fn listing_checkpoint(&self) -> StockListingCheckpointRef {
        self.listings
    }
    pub const fn command(&self) -> &CapturedStockCommitment {
        &self.command
    }
    /// Original identity anchor; not proof of a confirmed commit.
    pub fn attempt(&self) -> CommitAttempt {
        CommitAttempt::new(
            self.binding.input().workspace_id(),
            self.command.context().operation_id(),
            self.command.digest(),
            None,
        )
    }
}
impl core::fmt::Debug for StockAcceptanceRequest {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("StockAcceptanceRequest(<private command>)")
    }
}

/// A store's original T0 preparation report, distinct from S or installed T2.
/// Constructing or copying this metadata supplies no grant or signing permit.
#[derive(Clone)]
pub struct StockAcceptanceReceipt {
    attempt: CommitAttempt,
    order: OrderId,
    authorization: RecordId,
    terms: TermsDigest,
    checkpoint: InventoryCheckpointRef,
    pools: Vec<PoolSnapshot>,
    allocations: usize,
    window: IssuanceWindow,
    rumor: Arc<EncodedCommerceRumor>,
}
impl StockAcceptanceReceipt {
    /// Checks bounded host report consistency. The actual store must separately
    /// authenticate protected original inputs/effects and confirm the commit.
    #[allow(clippy::too_many_arguments)]
    pub fn from_committed_store(
        namespace: &AttachmentInput,
        attempt: CommitAttempt,
        order: OrderId,
        authorization: RecordId,
        terms: TermsDigest,
        checkpoint: InventoryCheckpointRef,
        pools: Vec<PoolSnapshot>,
        allocations: usize,
        window: IssuanceWindow,
        rumor: EncodedCommerceRumor,
    ) -> Result<Self, OperationError> {
        if attempt.workspace_id() != namespace.workspace_id()
            || attempt.transaction_id().is_none()
            || checkpoint.authority() != namespace.authority()
            || checkpoint.revision().get() == 0
            || checkpoint.authorization().is_none()
            || rumor.created_at() != window.prepared_at()
            || allocations > 64
            || pools.len() > allocations
            || pools.iter().any(|pool| pool.version().get() < 2)
            || pools.windows(2).any(|pair| pair[0].id() >= pair[1].id())
        {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(Self {
            attempt,
            order,
            authorization,
            terms,
            checkpoint,
            pools,
            allocations,
            window,
            rumor: Arc::new(rumor),
        })
    }
    pub const fn attempt(&self) -> CommitAttempt {
        self.attempt
    }
    pub const fn order_id(&self) -> OrderId {
        self.order
    }
    /// Original selected buyer request or terms authorization. This metadata
    /// alone proves neither participant consent nor a confirmed preparation.
    pub const fn authorization(&self) -> RecordId {
        self.authorization
    }
    pub const fn terms_digest(&self) -> TermsDigest {
        self.terms
    }
    pub const fn checkpoint(&self) -> InventoryCheckpointRef {
        self.checkpoint
    }
    /// Original after-images in pool identity order, not current free stock.
    pub fn held_pools(&self) -> &[PoolSnapshot] {
        &self.pools
    }
    /// Transfers reported after-images without duplicating private history.
    pub fn into_held_pools(self) -> Vec<PoolSnapshot> {
        self.pools
    }
    pub const fn allocation_count(&self) -> usize {
        self.allocations
    }
    pub const fn prepared_at(&self) -> UnixSeconds {
        self.window.prepared_at()
    }
    pub const fn first_issue_deadline(&self) -> UnixSeconds {
        self.window.first_issue_not_after()
    }
    /// Exact unsigned original record; no first-dispatch or replay permission.
    pub fn rumor(&self) -> &EncodedCommerceRumor {
        &self.rumor
    }
}
impl core::fmt::Debug for StockAcceptanceReceipt {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("StockAcceptanceReceipt(<private preparation>)")
    }
}
#[derive(Debug)]
pub enum StockAcceptanceOutcome {
    Prepared(Box<StockAcceptanceReceipt>),
    Aborted(Box<super::StockAbortReceipt>),
    Unknown(CommitAttempt),
}
