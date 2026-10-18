use super::{ClientSession, SessionHost, Shared};
use crate::session::{HostBound, inquiry::InquiryRuntime};
use crate::{
    Error, OperationContext, OperationReceipt,
    workspace::{CommandContext, WorkspaceSession},
};
use alloc::boxed::Box;
use radroots_domain::{ChatMessage, CommandDigest, OperationId};
use radroots_protocol::command::{CapturedChatCommand, InquireInput};
use radroots_storage::command::{
    AdmissionOutcome, AdmissionRequest, OperationError, OperationJournal,
};

impl<H: SessionHost + 'static> ClientSession<H> {
    /// Explicitly resumes an original inquiry under current attached access.
    /// Original claims are never signed again; delivery retries preserve exact
    /// retained outer bytes and the first publication criterion, selecting only
    /// targets without recorded acceptance. No first-issue deadline is renewed.
    pub async fn resume_inquiry(
        &self,
        journal: Shared<dyn OperationJournal>,
        scope: &WorkspaceSession,
        operation: OperationId,
        expected: Option<CommandDigest>,
    ) -> crate::Result<OperationReceipt<ChatMessage>> {
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
                return Err(Error::operation(OperationError::Unauthorized, proposed));
            }
        }
        let (binding, proof) = scope
            .captured_input()
            .map_err(|error| Error::operation(error.into(), proposed))?;
        let runtime = self
            .inquiry
            .clone()
            .ok_or_else(|| Error::operation(OperationError::Unauthorized, proposed))?;
        let host = self
            .task_host
            .as_ref()
            .ok_or_else(|| Error::operation(OperationError::Unauthorized, proposed))?;
        let signer = self.signer_session();
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
                        signer,
                        lease,
                        crate::session::inquiry::RunMode::Resume,
                    )
                    .await
                    .map_err(|error| Error::operation(error, retained))?;
                let snapshot = journal
                    .operation_snapshot(binding, operation, Some(digest), proof)
                    .await
                    .map_err(|error| Error::operation(error, retained))?
                    .ok_or_else(|| Error::operation(OperationError::RecoveryRequired, retained))?;
                Ok::<_, Error>(OperationReceipt::from_snapshot(&snapshot))
            }
            .await;
            let _ = reply.send(result);
        }))
        .map_err(|error| error.with_context(proposed))?;
        observed.await.unwrap_or_else(|_| {
            Err(Error::operation(
                OperationError::RecoveryRequired,
                OperationContext::admission_unknown(operation),
            ))
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
    ) -> crate::Result<OperationReceipt<ChatMessage>> {
        let operation = context.data().operation_id();
        let proposed = OperationContext::proposed(Some(operation));
        let permit = self
            .session
            .admit()
            .map_err(|error| error.with_context(proposed))?;
        let budget = if self.task_host.is_some() || self.inquiry.is_some() {
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
                return Err(Error::operation(OperationError::Unauthorized, proposed));
            }
        }
        let (binding, proof) = context
            .captured_input()
            .map_err(|error| Error::operation(error.into(), proposed))?;
        let command = CapturedChatCommand::inquire(context.data().clone(), input)
            .map_err(|_| Error::operation(OperationError::InvalidCommand, proposed))?;
        let request = AdmissionRequest::new(binding, command)
            .map_err(|error| Error::operation(error, proposed))?;
        let runtime = self.inquiry.clone();
        let signer = self.signer_session();
        let lease: Shared<dyn HostBound> = Shared::new((permit, budget));
        if let Some(host) = &self.task_host {
            let (reply, observed) = futures::channel::oneshot::channel();
            let task = Box::pin(async move {
                let result = execute_inquiry(journal, request, proof, runtime, signer, lease).await;
                let _ = reply.send(result);
            });
            host.submit(task)
                .map_err(|error| error.with_context(proposed))?;
            observed
                .await
                .unwrap_or_else(|_| Err(Error::admission_uncertain(operation)))
        } else {
            execute_inquiry(journal, request, proof, runtime, signer, lease).await
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
    ) -> crate::Result<Option<OperationReceipt<ChatMessage>>> {
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
        journal
            .operation_snapshot(binding, operation, expected_digest, proof)
            .await
            .map(|snapshot| snapshot.as_ref().map(OperationReceipt::from_snapshot))
            .map_err(|error| Error::operation(error, proposed))
    }
}

async fn execute_inquiry(
    journal: Shared<dyn OperationJournal>,
    request: AdmissionRequest,
    proof: radroots_storage::workspace::AttachmentProof,
    runtime: Option<Shared<InquiryRuntime>>,
    signer: Option<super::SignerSession>,
    lease: Shared<dyn HostBound>,
) -> crate::Result<OperationReceipt<ChatMessage>> {
    let operation = request.command().context().operation_id();
    let proposed = OperationContext::proposed(Some(operation));
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
        AdmissionOutcome::Unknown(_) => Err(Error::admission_uncertain(operation)),
        AdmissionOutcome::Committed(prepared) => {
            let retained = OperationContext::retained(operation);
            if let Some(runtime) = runtime {
                runtime
                    .run(
                        journal.clone(),
                        prepared,
                        &query_proof,
                        signer,
                        lease,
                        crate::session::inquiry::RunMode::Command,
                    )
                    .await
                    .map_err(|error| Error::operation(error, retained))?;
            }
            let snapshot = journal
                .operation_snapshot(binding, operation, Some(digest), query_proof)
                .await
                .map_err(|error| Error::operation(error, retained))?
                .ok_or_else(|| Error::operation(OperationError::RecoveryRequired, retained))?;
            Ok(OperationReceipt::from_snapshot(&snapshot))
        }
    }
}
