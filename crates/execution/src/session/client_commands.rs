use super::{ClientSession, SessionHost, Shared};
use crate::{
    Error, OperationContext, OperationReceipt,
    workspace::{CommandContext, WorkspaceSession},
};
use radroots_domain::{ChatMessage, CommandDigest, OperationId};
use radroots_protocol::command::{CapturedChatCommand, InquireInput};
use radroots_storage::command::{
    AdmissionOutcome, AdmissionRequest, OperationError, OperationJournal,
};

impl<H: SessionHost> ClientSession<H> {
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
        let _permit = self
            .session
            .admit()
            .map_err(|error| error.with_context(proposed))?;
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
        let digest = command.digest();
        let request = AdmissionRequest::new(binding.clone(), command)
            .map_err(|error| Error::operation(error, proposed))?;
        // The actual admission checks the original identity before fresh
        // deadline/planning; context.revalidate would reject retained replays.
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
            AdmissionOutcome::Committed(_) => {
                let retained = OperationContext::retained(operation);
                let snapshot = journal
                    .operation_snapshot(binding, operation, Some(digest), query_proof)
                    .await
                    .map_err(|error| Error::operation(error, retained))?
                    .ok_or_else(|| Error::operation(OperationError::RecoveryRequired, retained))?;
                Ok(OperationReceipt::from_snapshot(&snapshot))
            }
        }
    }

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
