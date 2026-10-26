//! Owned calls into the explicitly selected private inventory store.
use super::{ClientSession, SessionHost, Shared};
use crate::{
    Error, OperationContext,
    session::{HostBound, HostFuture},
    workspace::{CommandContext, WorkspaceSession},
};
use alloc::boxed::Box;
use radroots_domain::{
    CommandDigest, OperationId, PoolId,
    stock::{OptionMappingKey, OptionPoolMapping, PoolView},
};
use radroots_protocol::command::{
    AcceptOrderInput, CapturedInventoryCommand, CapturedOptionMapping, CapturedOrderAcceptance,
    CapturedPoolCorrection, CapturedPoolCreation, CreatePoolInput, MapOptionInput,
    PoolCorrectionInput,
};
use radroots_storage::{
    command::OperationError,
    inventory::{
        InventoryCheckpointRef, InventoryOutcome, InventoryReceipt, InventoryRequest,
        InventoryStore, StockAbortOutcome, StockAbortReceipt, StockAbortRequest,
        StockAcceptanceOutcome, StockAcceptanceRequest,
    },
    workspace::{AttachmentProof, WorkspaceBinding},
};

impl<H: SessionHost + 'static> ClientSession<H> {
    /// Captures original acceptance and owns its T0 admission until settlement.
    /// A preparation report claims no signature, installed agreement or delivery.
    /// Concrete store authentication/consent and actual atomic effects remain
    /// authoritative; supplied minima and command metadata grant nothing.
    #[allow(clippy::too_many_arguments)]
    pub async fn prepare_stock_acceptance(
        &self,
        store: Shared<dyn InventoryStore>,
        scope: &WorkspaceSession,
        context: CommandContext,
        minimum: InventoryCheckpointRef,
        sources: radroots_storage::workspace::seller_stock::StockSourceCheckpointRef,
        listings: radroots_storage::workspace::seller_stock::StockListingCheckpointRef,
        input: AcceptOrderInput,
    ) -> crate::Result<StockAcceptanceOutcome> {
        let operation = context.data().operation_id();
        let proposed = OperationContext::proposed(Some(operation));
        if !scope.owns_context(&context) {
            return Err(Error::operation(OperationError::Unauthorized, proposed));
        }
        self.run_stock_with_loss(
            scope,
            proposed,
            Error::admission_uncertain(operation),
            move |binding, proof| {
                // Capture only after reserving work and byte capacity, before
                // submission. No store, signer or transport runs in capture.
                let request = CapturedOrderAcceptance::new(context.data().clone(), input)
                    .map_err(|_| OperationError::InvalidCommand)
                    .and_then(|command| {
                        StockAcceptanceRequest::capture(
                            binding, minimum, sources, listings, &command,
                        )
                    });
                Box::pin(async move { store.prepare_stock_acceptance(request?, proof).await })
            },
        )
        .await
    }

    /// Captures pool creation from this client's own selected attachment.
    /// Actual authority, conservation and commit remain the store's obligation.
    pub async fn create_stock_pool(
        &self,
        store: Shared<dyn InventoryStore>,
        scope: &WorkspaceSession,
        context: CommandContext,
        minimum: InventoryCheckpointRef,
        input: CreatePoolInput,
    ) -> crate::Result<InventoryOutcome> {
        self.submit_inventory(store, scope, context, minimum, |data| {
            CapturedPoolCreation::new(data, input).map(Into::into)
        })
        .await
    }

    /// Captures an exact available-stock correction or spoilage command.
    pub async fn correct_stock_pool(
        &self,
        store: Shared<dyn InventoryStore>,
        scope: &WorkspaceSession,
        context: CommandContext,
        minimum: InventoryCheckpointRef,
        input: PoolCorrectionInput,
    ) -> crate::Result<InventoryOutcome> {
        self.submit_inventory(store, scope, context, minimum, |data| {
            CapturedPoolCorrection::new(data, input).map(Into::into)
        })
        .await
    }

    /// Captures a private option-to-pool mapping without signing or transport.
    pub async fn map_stock_option(
        &self,
        store: Shared<dyn InventoryStore>,
        scope: &WorkspaceSession,
        context: CommandContext,
        minimum: InventoryCheckpointRef,
        input: MapOptionInput,
    ) -> crate::Result<InventoryOutcome> {
        self.submit_inventory(store, scope, context, minimum, |data| {
            CapturedOptionMapping::new(data, input).map(Into::into)
        })
        .await
    }

    async fn submit_inventory(
        &self,
        store: Shared<dyn InventoryStore>,
        scope: &WorkspaceSession,
        context: CommandContext,
        minimum: InventoryCheckpointRef,
        capture: impl FnOnce(
            radroots_domain::CommandContextData,
        ) -> Result<
            CapturedInventoryCommand,
            radroots_protocol::command::CommandError,
        >,
    ) -> crate::Result<InventoryOutcome> {
        let proposed = OperationContext::proposed(Some(context.data().operation_id()));
        if !scope.owns_context(&context) {
            return Err(Error::operation(OperationError::Unauthorized, proposed));
        }
        self.run_stock_with_loss(
            scope,
            proposed,
            Error::admission_uncertain(context.data().operation_id()),
            move |binding, proof| {
                // Capture only after resource and byte reservations. This closure
                // performs no store, signer or transport call before submission.
                let request = capture(context.data().clone())
                    .map_err(|_| OperationError::InvalidCommand)
                    .and_then(|command| InventoryRequest::new(binding, minimum, command));
                Box::pin(async move { store.execute_inventory(request?, proof).await })
            },
        )
        .await
    }

    /// Explicitly asks the selected store to prove original claim absence and
    /// atomically release only that preparation. Selectors are never proof.
    pub async fn abort_proven_unissued(
        &self,
        store: Shared<dyn InventoryStore>,
        scope: &WorkspaceSession,
        minimum: InventoryCheckpointRef,
        operation: OperationId,
        digest: CommandDigest,
    ) -> crate::Result<StockAbortOutcome> {
        self.run_stock(
            scope,
            OperationContext::proposed(Some(operation)),
            move |binding, proof| {
                let request = StockAbortRequest::new(binding, minimum, operation, digest);
                Box::pin(async move { store.abort_proven_unissued(request, proof).await })
            },
        )
        .await
    }

    /// Reads an original abort without inferring safe nonissuance from absence.
    pub async fn stock_preparation_abort(
        &self,
        store: Shared<dyn InventoryStore>,
        scope: &WorkspaceSession,
        minimum: InventoryCheckpointRef,
        operation: OperationId,
        digest: CommandDigest,
    ) -> crate::Result<Option<StockAbortReceipt>> {
        self.run_stock(
            scope,
            OperationContext::proposed(Some(operation)),
            move |binding, proof| {
                let request = StockAbortRequest::new(binding, minimum, operation, digest);
                Box::pin(async move { store.stock_preparation_abort(request, proof).await })
            },
        )
        .await
    }

    /// Reads the original private inventory effect under an independent minimum.
    pub async fn inventory_operation(
        &self,
        store: Shared<dyn InventoryStore>,
        scope: &WorkspaceSession,
        minimum: InventoryCheckpointRef,
        operation: OperationId,
        digest: Option<CommandDigest>,
    ) -> crate::Result<Option<InventoryReceipt>> {
        self.run_stock(
            scope,
            OperationContext::proposed(Some(operation)),
            move |binding, proof| {
                Box::pin(async move {
                    store
                        .inventory_operation(binding, minimum, operation, digest, proof)
                        .await
                })
            },
        )
        .await
    }

    /// Reads a private pool; a missing row never constructs stock or authority.
    pub async fn stock_pool(
        &self,
        store: Shared<dyn InventoryStore>,
        scope: &WorkspaceSession,
        minimum: InventoryCheckpointRef,
        pool: PoolId,
    ) -> crate::Result<Option<PoolView>> {
        self.run_stock(
            scope,
            OperationContext::proposed(None),
            move |binding, proof| {
                Box::pin(async move { store.pool(binding, minimum, pool, proof).await })
            },
        )
        .await
    }

    /// Reads the actual private mapping under current owner access.
    pub async fn stock_option_mapping(
        &self,
        store: Shared<dyn InventoryStore>,
        scope: &WorkspaceSession,
        minimum: InventoryCheckpointRef,
        key: OptionMappingKey,
    ) -> crate::Result<Option<OptionPoolMapping>> {
        self.run_stock(
            scope,
            OperationContext::proposed(None),
            move |binding, proof| {
                Box::pin(async move { store.option_mapping(binding, minimum, key, proof).await })
            },
        )
        .await
    }

    async fn run_stock<T: HostBound + 'static>(
        &self,
        scope: &WorkspaceSession,
        proposed: OperationContext,
        work: impl FnOnce(
            WorkspaceBinding,
            AttachmentProof,
        ) -> HostFuture<'static, Result<T, OperationError>>,
    ) -> crate::Result<T> {
        self.run_stock_with_loss(
            scope,
            proposed,
            Error::operation(OperationError::RecoveryRequired, proposed),
            work,
        )
        .await
    }

    async fn run_stock_with_loss<T: HostBound + 'static>(
        &self,
        scope: &WorkspaceSession,
        proposed: OperationContext,
        lost_observation: Error,
        work: impl FnOnce(
            WorkspaceBinding,
            AttachmentProof,
        ) -> HostFuture<'static, Result<T, OperationError>>,
    ) -> crate::Result<T> {
        let permit = self
            .session
            .admit()
            .map_err(|error| error.with_context(proposed))?;
        let budget = self
            .task_budget
            .reserve()
            .map_err(|error| error.with_context(proposed))?;
        {
            let scopes = self
                .workspaces
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if !scopes.iter().any(|owned| owned.same_scope(scope)) {
                return Err(Error::operation(OperationError::Unauthorized, proposed));
            }
        }
        let host = self
            .task_host
            .as_ref()
            .ok_or_else(|| Error::operation(OperationError::UnsupportedOperation, proposed))?;
        let (binding, proof) = scope
            .captured_input()
            .map_err(|error| Error::operation(error.into(), proposed))?;
        let future = work(binding, proof);
        let lease: Shared<dyn HostBound> = Shared::new((permit, budget));
        let (reply, observed) = futures::channel::oneshot::channel();
        host.submit(Box::pin(async move {
            let result = future
                .await
                .map_err(|error| Error::operation(error, proposed));
            let _ = reply.send(result);
            // Observer loss cannot drop the permit before the store settles.
            drop(lease);
        }))
        .map_err(|error| error.with_context(proposed))?;
        // An accepted admission task lost before its reply has unknown T0;
        // reads/recovery cannot manufacture a confirmed business phase either.
        observed.await.unwrap_or(Err(lost_observation))
    }
}
