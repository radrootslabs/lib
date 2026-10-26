//! Typed private stock commands through one selected authenticated attachment.
use crate::{Client, CommandContext, OperationContext, Result, WorkspaceSession};
use radroots_domain::{
    CommandDigest, OperationId, PoolId,
    stock::{AvailableStockCorrection, OptionMappingKey, OptionPoolMapping, PoolView},
};
pub use radroots_protocol::command::{CreatePoolInput, MapOptionInput, PoolCorrectionInput};
pub use radroots_storage::inventory::{InventoryCheckpointRef, InventoryOutcome, InventoryReceipt};
pub use radroots_storage::inventory::{
    StockEvidenceOutcome, StockEvidenceReceipt, StockEvidenceRequest,
};

/// Borrowed access to this client's explicitly selected private stock owner.
pub struct Stock<'a> {
    client: &'a Client,
    scope: WorkspaceSession,
}
impl<'a> Stock<'a> {
    pub(crate) const fn new(client: &'a Client, scope: WorkspaceSession) -> Self {
        Self { client, scope }
    }
    /// Retains already obtained original evidence through actual append and
    /// caller loss. Failed or uncertain capture remains owned for explicit retry
    /// or close draining. This supplies no signing or installation permission.
    pub async fn capture_evidence(
        &self,
        request: &StockEvidenceRequest,
    ) -> Result<StockEvidenceOutcome> {
        self.client
            .session
            .capture_stock_evidence(self.client.inventory()?, &self.scope, request)
            .await
    }
    /// Retries this client's retained original captures without obtaining new
    /// signatures, changing captured bytes or renewing their original windows.
    pub async fn drain_evidence(&self) -> Result<()> {
        self.client.session.drain_stock_evidence().await
    }
    /// Creates a private pool with actual atomic conservation and idempotency.
    /// The report claims neither Nostr signing nor transport delivery.
    pub async fn create_pool(
        &self,
        context: CommandContext,
        minimum: InventoryCheckpointRef,
        input: CreatePoolInput,
    ) -> Result<InventoryOutcome> {
        self.client
            .session
            .create_stock_pool(
                self.client.inventory()?,
                &self.scope,
                context,
                minimum,
                input,
            )
            .await
    }
    /// Applies an explicit positive addition or available-stock removal.
    pub async fn adjust(
        &self,
        context: CommandContext,
        minimum: InventoryCheckpointRef,
        input: PoolCorrectionInput,
    ) -> Result<InventoryOutcome> {
        if input.kind() == AvailableStockCorrection::Spoil {
            return Err(invalid(&context));
        }
        self.correct(context, minimum, input).await
    }
    /// Records physical spoilage without silently spending held quantities.
    pub async fn spoil(
        &self,
        context: CommandContext,
        minimum: InventoryCheckpointRef,
        input: PoolCorrectionInput,
    ) -> Result<InventoryOutcome> {
        if input.kind() != AvailableStockCorrection::Spoil {
            return Err(invalid(&context));
        }
        self.correct(context, minimum, input).await
    }
    async fn correct(
        &self,
        context: CommandContext,
        minimum: InventoryCheckpointRef,
        input: PoolCorrectionInput,
    ) -> Result<InventoryOutcome> {
        self.client
            .session
            .correct_stock_pool(
                self.client.inventory()?,
                &self.scope,
                context,
                minimum,
                input,
            )
            .await
    }
    /// Updates an owner-private option mapping with exact expected progress.
    pub async fn map_option(
        &self,
        context: CommandContext,
        minimum: InventoryCheckpointRef,
        input: MapOptionInput,
    ) -> Result<InventoryOutcome> {
        self.client
            .session
            .map_stock_option(
                self.client.inventory()?,
                &self.scope,
                context,
                minimum,
                input,
            )
            .await
    }
    /// Reads the actual protected pool under an independently retained minimum.
    pub async fn pool(
        &self,
        minimum: InventoryCheckpointRef,
        pool: PoolId,
    ) -> Result<Option<PoolView>> {
        self.client
            .session
            .stock_pool(self.client.inventory()?, &self.scope, minimum, pool)
            .await
    }
    /// Reads a private mapping; absence supplies no default allocation.
    pub async fn option_mapping(
        &self,
        minimum: InventoryCheckpointRef,
        key: OptionMappingKey,
    ) -> Result<Option<OptionPoolMapping>> {
        self.client
            .session
            .stock_option_mapping(self.client.inventory()?, &self.scope, minimum, key)
            .await
    }
    /// Recovers an original private effect, without executing a fresh command.
    pub async fn operation(
        &self,
        minimum: InventoryCheckpointRef,
        operation: OperationId,
        digest: Option<CommandDigest>,
    ) -> Result<Option<InventoryReceipt>> {
        self.client
            .session
            .inventory_operation(
                self.client.inventory()?,
                &self.scope,
                minimum,
                operation,
                digest,
            )
            .await
    }
}
fn invalid(context: &CommandContext) -> crate::Error {
    crate::Error::operation(
        radroots_storage::command::OperationError::InvalidCommand,
        OperationContext::proposed(Some(context.data().operation_id())),
    )
}
