use super::{ClientSession, SessionHost, Shared};
use crate::session::{
    HostBound,
    command::{CommandRuntime, SigningResources},
};
use crate::{
    Error, OperationContext, OperationReceipt, OperationStatus,
    workspace::{CommandContext, WorkspaceSession},
};
use alloc::boxed::Box;
use radroots_domain::{ChatMessage, CommandDigest, OperationId, listing::Listing};
use radroots_protocol::{
    command::{CapturedChatCommand, CapturedCommand, CommandError, InquireInput},
    listing::{CapturedListingCommand, PublishInput},
};
use radroots_storage::command::{
    AdmissionOutcome, AdmissionRequest, OperationError, OperationJournal, OperationSnapshot,
};

type ReceiptProjection<T> = fn(&OperationSnapshot) -> Result<OperationReceipt<T>, OperationError>;

impl<H: SessionHost + 'static> ClientSession<H> {
    /// Explicitly resumes an original command under current attached access.
    /// Original claims are never signed again; delivery retries preserve exact
    /// retained outer bytes and the first publication criterion, selecting only
    /// targets without recorded acceptance. No first-issue deadline is renewed.
    pub async fn resume_command(
        &self,
        journal: Shared<dyn OperationJournal>,
        scope: &WorkspaceSession,
        operation: OperationId,
        expected: Option<CommandDigest>,
    ) -> crate::ExecutionResult<OperationStatus> {
        let proposed = OperationContext::proposed(Some(operation));
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
                return Err(Error::operation(OperationError::Unauthorized, proposed).into());
            }
        }
        let (binding, proof) = scope
            .captured_input()
            .map_err(|error| Error::operation(error.into(), proposed))?;
        let runtime = self
            .commands
            .clone()
            .ok_or_else(|| Error::operation(OperationError::Unauthorized, proposed))?;
        let host = self
            .task_host
            .as_ref()
            .ok_or_else(|| Error::operation(OperationError::Unauthorized, proposed))?;
        let signer = self.signer_session();
        let stock_captures = self.stock_captures.clone();
        let lease: Shared<dyn HostBound> = Shared::new((permit, budget));
        let (reply, observed) = futures::channel::oneshot::channel();
        host.submit(Box::pin(async move {
            let result = async {
                runtime
                    .drain()
                    .await
                    .map_err(|error| Error::operation(error, proposed))?;
                let prepared = journal
                    .operation(
                        binding.clone(),
                        operation,
                        expected,
                        proof
                            .with_secret(|secret| {
                                radroots_storage::workspace::AttachmentProof::from_protected_input(
                                    secret,
                                )
                            })
                            .map_err(|error| Error::operation(error.into(), proposed))?,
                    )
                    .await
                    .map_err(|error| Error::operation(error, proposed))?
                    .ok_or_else(|| Error::operation(OperationError::MissingHistory, proposed))?;
                let retained = OperationContext::retained(operation);
                let digest = prepared.identity().command_digest();
                runtime
                    .run(
                        journal.clone(),
                        prepared,
                        &proof,
                        SigningResources {
                            signer,
                            lease,
                            stock_captures,
                        },
                        crate::session::command::RunMode::Resume,
                    )
                    .await
                    .map_err(|error| error.into_failure(operation))?;
                let snapshot = journal
                    .operation_snapshot(binding, operation, Some(digest), proof)
                    .await
                    .map_err(|error| Error::operation(error, retained))?
                    .ok_or_else(|| Error::operation(OperationError::RecoveryRequired, retained))?;
                OperationStatus::try_from_snapshot(&snapshot)
                    .map_err(|error| Error::operation(error, retained).into())
            }
            .await;
            let _ = reply.send(result);
        }))
        .map_err(|error| error.with_context(proposed))?;
        observed.await.unwrap_or_else(|_| {
            Err(Error::operation(
                OperationError::RecoveryRequired,
                OperationContext::admission_unknown(operation),
            )
            .into())
        })
    }
    /// Captures and durably admits one inquiry through the configured journal.
    /// The concrete bounded worker owns accepted T0 work after observer loss.
    /// A pending receipt does not claim signing, installation or publication.
    /// The journal is an explicit host trust boundary, not adapter qualification.
    pub async fn inquire(
        &self,
        journal: Shared<dyn OperationJournal>,
        context: CommandContext,
        input: InquireInput,
    ) -> crate::ExecutionResult<OperationReceipt<ChatMessage>> {
        self.submit_command(
            journal,
            context,
            move |data| CapturedChatCommand::inquire(data, input).map(CapturedCommand::Chat),
            OperationReceipt::<ChatMessage>::try_from_snapshot,
        )
        .await
    }
    /// Captures an initial public listing under its actual attached seller
    /// scope. The same owner retains T0/T1/S/T2/D after observer loss.
    pub async fn publish_listing(
        &self,
        journal: Shared<dyn OperationJournal>,
        context: CommandContext,
        input: PublishInput,
    ) -> crate::ExecutionResult<OperationReceipt<Listing>> {
        self.submit_command(
            journal,
            context,
            move |data| CapturedListingCommand::capture(data, input).map(CapturedCommand::Listing),
            OperationReceipt::<Listing>::try_from_snapshot,
        )
        .await
    }
    async fn submit_command<T: HostBound + 'static, F>(
        &self,
        journal: Shared<dyn OperationJournal>,
        context: CommandContext,
        capture: F,
        project: ReceiptProjection<T>,
    ) -> crate::ExecutionResult<OperationReceipt<T>>
    where
        F: FnOnce(radroots_domain::CommandContextData) -> Result<CapturedCommand, CommandError>,
    {
        let operation = context.data().operation_id();
        let proposed = OperationContext::proposed(Some(operation));
        self.stock_captures
            .ensure_ready()
            .map_err(|error| Error::operation(error, proposed))?;
        let permit = self
            .session
            .admit()
            .map_err(|error| error.with_context(proposed))?;
        let budget = if self.task_host.is_some() || self.commands.is_some() {
            Some(
                self.task_budget
                    .reserve()
                    .map_err(|error| error.with_context(proposed))?,
            )
        } else {
            None
        };
        {
            let scopes = self
                .workspaces
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if !scopes.iter().any(|scope| scope.owns_context(&context)) {
                return Err(Error::operation(OperationError::Unauthorized, proposed).into());
            }
        }
        let (binding, proof) = context
            .captured_input()
            .map_err(|error| Error::operation(error.into(), proposed))?;
        let command = capture(context.data().clone())
            .map_err(|_| Error::operation(OperationError::InvalidCommand, proposed))?;
        let request = AdmissionRequest::new(binding, command)
            .map_err(|error| Error::operation(error, proposed))?;
        let runtime = self.commands.clone();
        let signer = self.signer_session();
        let stock_captures = self.stock_captures.clone();
        let lease: Shared<dyn HostBound> = Shared::new((permit, budget));
        if let Some(host) = &self.task_host {
            let (reply, observed) = futures::channel::oneshot::channel();
            let task = Box::pin(async move {
                let result = execute_command(
                    journal,
                    request,
                    proof,
                    runtime,
                    SigningResources {
                        signer,
                        lease,
                        stock_captures,
                    },
                    project,
                )
                .await;
                let _ = reply.send(result);
            });
            host.submit(task)
                .map_err(|error| error.with_context(proposed))?;
            observed
                .await
                .unwrap_or_else(|_| Err(Error::admission_uncertain(operation).into()))
        } else {
            execute_command(
                journal,
                request,
                proof,
                runtime,
                SigningResources {
                    signer,
                    lease,
                    stock_captures,
                },
                project,
            )
            .await
        }
    }
}

impl<H: SessionHost> ClientSession<H> {
    /// Reads the original complete operation through a scope attached to this
    /// client. Query time never renews the original first-issue deadline and
    /// absence never proves an uncertain invocation was safely unissued.
    pub async fn operation(
        &self,
        journal: Shared<dyn OperationJournal>,
        scope: &WorkspaceSession,
        operation: OperationId,
        expected_digest: Option<CommandDigest>,
    ) -> crate::Result<Option<OperationStatus>> {
        let proposed = OperationContext::proposed(Some(operation));
        let _permit = self
            .session
            .admit()
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
        let (binding, proof) = scope
            .captured_input()
            .map_err(|error| Error::operation(error.into(), proposed))?;
        let snapshot = journal
            .operation_snapshot(binding, operation, expected_digest, proof)
            .await
            .map_err(|error| Error::operation(error, proposed))?;
        snapshot
            .as_ref()
            .map(OperationStatus::try_from_snapshot)
            .transpose()
            .map_err(|error| Error::operation(error, OperationContext::retained(operation)))
    }
}

async fn execute_command<T: HostBound + 'static>(
    journal: Shared<dyn OperationJournal>,
    request: AdmissionRequest,
    proof: radroots_storage::workspace::AttachmentProof,
    runtime: Option<Shared<CommandRuntime>>,
    resources: SigningResources,
    project: ReceiptProjection<T>,
) -> crate::ExecutionResult<OperationReceipt<T>> {
    let operation = request.command().context().operation_id();
    let proposed = OperationContext::proposed(Some(operation));
    resources
        .stock_captures
        .ensure_ready()
        .map_err(|error| Error::operation(error, proposed))?;
    let binding = request.binding().clone();
    let digest = request.command().digest();
    // Original lookup precedes fresh deadline/planning. Authentication of the
    // actual transaction remains native; a captured descriptor grants nothing.
    let query_proof = proof
        .with_secret(|secret| {
            radroots_storage::workspace::AttachmentProof::from_protected_input(secret)
        })
        .map_err(|error| Error::operation(error.into(), proposed))?;
    let result = journal
        .admit_command_and_hold(request, proof)
        .await
        .map_err(|error| Error::operation(error, proposed))?;
    match result {
        AdmissionOutcome::Unknown(_) => Err(Error::admission_uncertain(operation).into()),
        AdmissionOutcome::Committed(prepared) => {
            let retained = OperationContext::retained(operation);
            if let Some(runtime) = runtime {
                runtime
                    .run(
                        journal.clone(),
                        prepared,
                        &query_proof,
                        resources,
                        crate::session::command::RunMode::Command,
                    )
                    .await
                    .map_err(|error| error.into_failure(operation))?;
            }
            let snapshot = journal
                .operation_snapshot(binding, operation, Some(digest), query_proof)
                .await
                .map_err(|error| Error::operation(error, retained))?
                .ok_or_else(|| Error::operation(OperationError::RecoveryRequired, retained))?;
            project(&snapshot).map_err(|error| Error::operation(error, retained).into())
        }
    }
}
