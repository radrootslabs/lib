//! Ordinary captured order acceptance and original recovery through Core.
use crate::{Client, CommandContext, Result, WorkspaceSession};
use radroots_domain::{CommandDigest, OperationId};
pub use radroots_execution::session::commerce::{
    AcceptanceRecipient, AcceptanceRewrapStatus, AcceptanceStatus, AcceptanceUnissuedReceipt,
    InstalledAcceptance,
};
pub use radroots_protocol::command::{AcceptOrderInput, InstallTermsInput};
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
    /// Owns captured acceptance through native T0/T1/S/T2 and original delivery.
    /// Inspect independent signing, installation and relay observations. An
    /// installed peer can reach delivery even if later self signing fails.
    /// Available command IO controls progress; pending preparation is not acceptance.
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
    /// Owns mutually authorized complete replacement installation through the
    /// same executor as acceptance. Positive holds precede issuance; old
    /// obligations remain pending and negative deltas release only at T2.
    pub async fn install_terms(
        &self,
        context: CommandContext,
        minimum: InventoryCheckpointRef,
        sources: StockSourceCheckpointRef,
        listings: StockListingCheckpointRef,
        input: InstallTermsInput,
    ) -> crate::ExecutionResult<AcceptanceStatus> {
        self.client
            .session
            .install_stock_terms(
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
    /// Creates one explicitly requested additional carrier for the same original
    /// commitment. Existing/unknown admissions never generate another key.
    pub async fn rewrap_acceptance(
        &self,
        minimum: InventoryCheckpointRef,
        operation: OperationId,
        digest: CommandDigest,
        recipient: radroots_domain::PublicKey,
        rewrap_id: OperationId,
    ) -> crate::ExecutionResult<AcceptanceRewrapStatus> {
        self.client
            .session
            .rewrap_stock_acceptance(
                self.client.inventory()?,
                self.client.acceptance_store()?,
                &self.scope,
                minimum,
                operation,
                digest,
                recipient,
                rewrap_id,
            )
            .await
    }
    /// Explicitly retries only a known captured carrier within its original
    /// policy and cumulative invocation budget. Unknown bytes never regenerate.
    pub async fn resume_acceptance_rewrap(
        &self,
        minimum: InventoryCheckpointRef,
        operation: OperationId,
        digest: CommandDigest,
        recipient: radroots_domain::PublicKey,
        rewrap_id: OperationId,
    ) -> crate::ExecutionResult<AcceptanceRewrapStatus> {
        self.client
            .session
            .resume_stock_rewrap(
                self.client.inventory()?,
                self.client.acceptance_store()?,
                &self.scope,
                minimum,
                operation,
                digest,
                recipient,
                rewrap_id,
            )
            .await
    }
    /// Reads retained carrier facts without wrapping, signing or dispatch.
    pub async fn acceptance_rewrap_status(
        &self,
        minimum: InventoryCheckpointRef,
        operation: OperationId,
        digest: CommandDigest,
        recipient: radroots_domain::PublicKey,
        rewrap_id: OperationId,
    ) -> Result<AcceptanceRewrapStatus> {
        self.client
            .session
            .stock_acceptance_rewrap_status(
                self.client.inventory()?,
                &self.scope,
                minimum,
                operation,
                digest,
                recipient,
                rewrap_id,
            )
            .await
    }
}
