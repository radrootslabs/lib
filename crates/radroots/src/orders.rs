//! Ordinary captured order acceptance and original recovery through Core.
use crate::{Client, CommandContext, Result, WorkspaceSession};
use radroots_domain::{CommandDigest, OperationId};
pub use radroots_execution::session::commerce::{
    AcceptanceRecipient, AcceptanceStatus, AcceptanceUnissuedReceipt, InstalledAcceptance,
};
pub use radroots_protocol::command::AcceptOrderInput;
use radroots_storage::{
    inventory::InventoryCheckpointRef,
    workspace::seller_stock::{StockListingCheckpointRef, StockSourceCheckpointRef},
};

/// Borrowed order workflows for one explicitly selected client-owned scope.
pub struct Orders<'a> {
    client: &'a Client,
    scope: WorkspaceSession,
}
impl<'a> Orders<'a> {
    pub(crate) const fn new(client: &'a Client, scope: WorkspaceSession) -> Self {
        Self { client, scope }
    }
    /// Owns captured acceptance through actual native T0/T1/S and initial T2.
    /// Inspect independent recipient/installation facts; this performs no gift
    /// wrapping or relay publication. Available command IO determines whether
    /// preparation can proceed to signing; a pending report is not acceptance.
    pub async fn accept(
        &self,
        context: CommandContext,
        minimum: InventoryCheckpointRef,
        sources: StockSourceCheckpointRef,
        listings: StockListingCheckpointRef,
        input: AcceptOrderInput,
    ) -> crate::ExecutionResult<AcceptanceStatus> {
        self.client
            .session
            .accept_stock_order(
                self.client.inventory()?,
                self.client.acceptance_store()?,
                &self.scope,
                context,
                minimum,
                sources,
                listings,
                input,
            )
            .await
    }
    /// Explicitly resumes captured original work. Existing non-replayable
    /// claims never sign again; qualified replay uses the original host permit.
    pub async fn resume_acceptance(
        &self,
        minimum: InventoryCheckpointRef,
        operation: OperationId,
        digest: CommandDigest,
    ) -> crate::ExecutionResult<AcceptanceStatus> {
        self.client
            .session
            .resume_stock_acceptance(
                self.client.inventory()?,
                self.client.acceptance_store()?,
                &self.scope,
                minimum,
                operation,
                digest,
            )
            .await
    }
    /// Reads coherent original acceptance, independent signing and installation
    /// facts without issuing, replaying or publishing any artifact.
    pub async fn acceptance_status(
        &self,
        minimum: InventoryCheckpointRef,
        operation: OperationId,
        digest: CommandDigest,
    ) -> Result<AcceptanceStatus> {
        self.client
            .session
            .stock_acceptance_status(
                self.client.acceptance_store()?,
                &self.scope,
                minimum,
                operation,
                digest,
            )
            .await
    }
}
