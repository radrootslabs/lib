//! Client-owned acceptance tasks; caller observation is not execution ownership.
use super::{ClientSession, SessionHost, Shared};
use crate::{
    Error, OperationContext,
    session::{
        HostBound,
        command::{RunMode, SigningResources},
        commerce::*,
    },
    workspace::{CommandContext, WorkspaceSession},
};
use alloc::boxed::Box;
use radroots_domain::{CommandDigest, OperationId};
use radroots_protocol::command::{AcceptOrderInput, CapturedOrderAcceptance};
use radroots_storage::{
    command::OperationError,
    inventory::{
        InventoryCheckpointRef, InventoryStore, StockAcceptanceOutcome, StockAcceptanceRequest,
    },
    workspace::{
        WorkspaceBinding,
        seller_stock::{StockListingCheckpointRef, StockSourceCheckpointRef},
    },
};

impl<H: SessionHost + 'static> ClientSession<H> {
    /// Owns original acceptance through T0/T1/S/T2 and bounded original delivery.
    /// Inspect independent signing, installation and per-recipient relay facts;
    /// relay acceptance never establishes buyer receipt or stock release.
    /// Missing IO leaves actual preparation pending without signing.
    #[allow(clippy::too_many_arguments)]
    pub async fn accept_stock_order(
        &self,
        inventory: Shared<dyn InventoryStore>,
        store: Shared<dyn AcceptanceStore>,
        scope: &WorkspaceSession,
        context: CommandContext,
        minimum: InventoryCheckpointRef,
        sources: StockSourceCheckpointRef,
        listings: StockListingCheckpointRef,
        input: AcceptOrderInput,
    ) -> crate::ExecutionResult<AcceptanceStatus> {
        let operation = context.data().operation_id();
        if !scope.owns_context(&context) {
            return Err(Error::operation(
                OperationError::Unauthorized,
                OperationContext::proposed(Some(operation)),
            )
            .into());
        }
        self.submit_acceptance(
            inventory,
            store,
            scope,
            operation,
            RunMode::Command,
            move |binding| {
                let command = CapturedOrderAcceptance::new(context.data().clone(), input)
                    .map_err(|_| OperationError::InvalidCommand)?;
                let digest = command.digest();
                let request = StockAcceptanceRequest::capture(
                    binding.clone(),
                    minimum,
                    sources,
                    listings,
                    &command,
                )?;
                Ok((
                    AcceptanceSelection {
                        binding,
                        minimum,
                        operation,
                        digest,
                    },
                    Some(request),
                ))
            },
        )
        .await
    }
    /// Resumes only original retained inputs. Non-replayable existing claims
    /// remain unknown; a qualified original local replay requires its actual
    /// current host permit and original deadline. This never releases stock.
    #[allow(clippy::too_many_arguments)]
    pub async fn resume_stock_acceptance(
        &self,
        inventory: Shared<dyn InventoryStore>,
        store: Shared<dyn AcceptanceStore>,
        scope: &WorkspaceSession,
        minimum: InventoryCheckpointRef,
        operation: OperationId,
        digest: CommandDigest,
    ) -> crate::ExecutionResult<AcceptanceStatus> {
        self.submit_acceptance(
            inventory,
            store,
            scope,
            operation,
            RunMode::Resume,
            move |binding| {
                Ok((
                    AcceptanceSelection {
                        binding,
                        minimum,
                        operation,
                        digest,
                    },
                    None,
                ))
            },
        )
        .await
    }
    /// Reads original stock acceptance facts without signing or replaying work.
    pub async fn stock_acceptance_status(
        &self,
        store: Shared<dyn AcceptanceStore>,
        scope: &WorkspaceSession,
        minimum: InventoryCheckpointRef,
        operation: OperationId,
        digest: CommandDigest,
    ) -> crate::Result<AcceptanceStatus> {
        self.run_stock(
            scope,
            OperationContext::retained(operation),
            move |binding, proof| {
                Box::pin(async move {
                    let selection = AcceptanceSelection {
                        binding,
                        minimum,
                        operation,
                        digest,
                    };
                    let view = store.snapshot(selection.clone(), &proof).await?;
                    crate::session::commerce::observe(&selection, view.as_ref())
                })
            },
        )
        .await
    }
    async fn submit_acceptance(
        &self,
        inventory: Shared<dyn InventoryStore>,
        store: Shared<dyn AcceptanceStore>,
        scope: &WorkspaceSession,
        operation: OperationId,
        mode: RunMode,
        capture: impl FnOnce(
            WorkspaceBinding,
        ) -> Result<
            (AcceptanceSelection, Option<StockAcceptanceRequest>),
            OperationError,
        >,
    ) -> crate::ExecutionResult<AcceptanceStatus> {
        let proposed = OperationContext::proposed(Some(operation));
        if matches!(&mode, RunMode::Command) {
            self.stock_captures
                .ensure_ready()
                .map_err(|e| Error::operation(e, proposed))?;
        }
        let permit = self.session.admit().map_err(|e| e.with_context(proposed))?;
        let budget = self
            .task_budget
            .reserve()
            .map_err(|e| e.with_context(proposed))?;
        {
            let scopes = self
                .workspaces
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if !scopes.iter().any(|v| v.same_scope(scope)) {
                return Err(Error::operation(OperationError::Unauthorized, proposed).into());
            }
        }
        let host = self
            .task_host
            .as_ref()
            .ok_or_else(|| Error::operation(OperationError::UnsupportedOperation, proposed))?;
        let (binding, proof) = scope
            .captured_input()
            .map_err(|e| Error::operation(e.into(), proposed))?;
        let (selection, request) = capture(binding).map_err(|e| Error::operation(e, proposed))?;
        let runtime = self.commands.clone();
        let resources = SigningResources {
            signer: self.signer_session(),
            lease: Shared::new((permit, budget)) as Shared<dyn HostBound>,
            stock_captures: self.stock_captures.clone(),
        };
        let (reply, observed) = futures::channel::oneshot::channel();
        host.submit(Box::pin(async move {
            let result = async {
                if matches!(&mode, RunMode::Resume) {
                    resources.stock_captures.drain().await?;
                } else {
                    resources
                        .stock_captures
                        .ensure_ready()
                        .map_err(|e| Error::operation(e, proposed))?;
                }
                if let Some(request) = request {
                    let credential = proof
                        .with_secret(|bytes| {
                            radroots_storage::workspace::AttachmentProof::from_protected_input(
                                bytes,
                            )
                        })
                        .map_err(|e| Error::operation(e.into(), proposed))?;
                    match inventory
                        .prepare_stock_acceptance(request, credential)
                        .await
                        .map_err(|e| Error::operation(e, proposed))?
                    {
                        StockAcceptanceOutcome::Unknown(_) => {
                            return Err(Error::admission_uncertain(operation).into());
                        }
                        StockAcceptanceOutcome::Prepared(value)
                            if value.attempt().workspace_id()
                                == selection.binding.input().workspace_id()
                                && value.attempt().operation_id() == operation
                                && value.attempt().command_digest() == selection.digest
                                && value.attempt().transaction_id().is_some() => {}
                        StockAcceptanceOutcome::Aborted(value)
                            if value.attempt().workspace_id()
                                == selection.binding.input().workspace_id()
                                && value.attempt().operation_id() == operation
                                && value.attempt().command_digest() == selection.digest
                                && value.attempt().transaction_id().is_some() => {}
                        _ => {
                            return Err(Error::operation(
                                OperationError::RecoveryRequired,
                                OperationContext::admission_unknown(operation),
                            )
                            .into());
                        }
                    }
                }
                if let Some(runtime) = runtime {
                    runtime
                        .run_acceptance(store, inventory, selection, &proof, resources, mode)
                        .await
                } else {
                    let retained = OperationContext::retained(operation);
                    let view = store
                        .snapshot(selection.clone(), &proof)
                        .await
                        .map_err(|e| Error::operation(e, retained))?;
                    crate::session::commerce::observe(&selection, view.as_ref())
                        .map_err(|e| Error::operation(e, retained).into())
                }
            }
            .await;
            let _ = reply.send(result);
        }))
        .map_err(|e| e.with_context(proposed))?;
        observed
            .await
            .unwrap_or_else(|_| Err(Error::admission_uncertain(operation).into()))
    }
}
