//! Client-owned explicit carrier work reuses commerce admission and shutdown.
use super::{ClientSession, SessionHost, Shared};
use crate::{
    Error, OperationContext,
    session::{command::RunMode, commerce::*},
    workspace::WorkspaceSession,
};
use alloc::boxed::Box;
use radroots_domain::{CommandDigest, OperationId, PublicKey};
use radroots_storage::{
    command::OperationError,
    inventory::{InventoryCheckpointRef, InventoryStore},
};

impl<H: SessionHost + 'static> ClientSession<H> {
    /// Requests one explicit additional outer for an actual original F10/S/T2.
    /// Only a newly confirmed admission can create its key. Repeating the same
    /// identity observes/resumes known bytes and never regenerates an unknown.
    #[allow(clippy::too_many_arguments)]
    pub async fn rewrap_stock_acceptance(
        &self,
        inventory: Shared<dyn InventoryStore>,
        store: Shared<dyn AcceptanceStore>,
        scope: &WorkspaceSession,
        minimum: InventoryCheckpointRef,
        operation: OperationId,
        digest: CommandDigest,
        recipient: PublicKey,
        id: OperationId,
    ) -> crate::ExecutionResult<AcceptanceRewrapStatus> {
        self.submit_rewrap(
            inventory,
            store,
            scope,
            minimum,
            operation,
            digest,
            recipient,
            id,
            RunMode::Command,
        )
        .await
    }
    /// Resumes a known exact outer using current scoped consent. No wrapping
    /// admission, temporary key or business signature is created by recovery.
    #[allow(clippy::too_many_arguments)]
    pub async fn resume_stock_rewrap(
        &self,
        inventory: Shared<dyn InventoryStore>,
        store: Shared<dyn AcceptanceStore>,
        scope: &WorkspaceSession,
        minimum: InventoryCheckpointRef,
        operation: OperationId,
        digest: CommandDigest,
        recipient: PublicKey,
        id: OperationId,
    ) -> crate::ExecutionResult<AcceptanceRewrapStatus> {
        self.submit_rewrap(
            inventory,
            store,
            scope,
            minimum,
            operation,
            digest,
            recipient,
            id,
            RunMode::Resume,
        )
        .await
    }
    #[allow(clippy::too_many_arguments)]
    async fn submit_rewrap(
        &self,
        inventory: Shared<dyn InventoryStore>,
        store: Shared<dyn AcceptanceStore>,
        scope: &WorkspaceSession,
        minimum: InventoryCheckpointRef,
        operation: OperationId,
        digest: CommandDigest,
        recipient: PublicKey,
        id: OperationId,
        mode: RunMode,
    ) -> crate::ExecutionResult<AcceptanceRewrapStatus> {
        self.submit_stock_commerce(
            scope,
            operation,
            mode,
            move |binding| {
                Ok(AcceptanceSelection {
                    binding,
                    minimum,
                    operation,
                    digest,
                })
            },
            move |selection, proof, runtime, resources| {
                Box::pin(async move {
                    let runtime = runtime.ok_or_else(|| {
                        Error::operation(
                            OperationError::UnsupportedOperation,
                            OperationContext::retained(operation),
                        )
                    })?;
                    runtime
                        .run_stock_rewrap(
                            store, inventory, selection, &proof, resources, recipient, id, mode,
                        )
                        .await
                })
            },
        )
        .await
    }
    /// Reads independent admission/capture/delivery facts with current access.
    /// An absent capture grants neither a fresh wrapping nor dispatch right.
    #[allow(clippy::too_many_arguments)]
    pub async fn stock_acceptance_rewrap_status(
        &self,
        inventory: Shared<dyn InventoryStore>,
        scope: &WorkspaceSession,
        minimum: InventoryCheckpointRef,
        operation: OperationId,
        digest: CommandDigest,
        recipient: PublicKey,
        id: OperationId,
    ) -> crate::Result<AcceptanceRewrapStatus> {
        self.run_stock(
            scope,
            OperationContext::retained(operation),
            move |binding, proof| {
                Box::pin(async move {
                    rewrap_status(
                        &inventory,
                        &AcceptanceSelection {
                            binding,
                            minimum,
                            operation,
                            digest,
                        },
                        recipient,
                        id,
                        &proof,
                    )
                    .await
                })
            },
        )
        .await
    }
}
