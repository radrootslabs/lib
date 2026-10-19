//! Actual exactly-once chat installation and append-only recipient work.
mod codec;
use super::journal::{OperationChange, Records as PreparationRecords, storage_error};
use super::{
    BrowserWorkspace, Mutation, State, WorkspaceError, address, crypto, owned, retained::Graph,
};
use crate::ReadSnapshot;
use radroots_domain::{CommandDigest, OperationId, TransactionId, UnixSeconds};
use radroots_secrets::context::{
    EnvelopeContext, EnvelopePurpose, EnvelopeSubject, PayloadSchemaId,
};
use radroots_storage::{
    command::{
        BoundSigningPlan, ChatInstallation, CommitAttempt, InstallationOutcome,
        InstallationRequest, OperationError, OperationSnapshot, PublicationWork,
    },
    workspace::{AttachmentProof, WorkspaceBinding},
};
use sha2::{Digest, Sha256};

fn context(
    state: &State,
    bound: &BoundSigningPlan,
    recipient: Option<u8>,
) -> Result<EnvelopeContext, OperationError> {
    let mut hash = Sha256::new()
        .chain_update(b"radroots.browser.local_outbox.installation.context.v2\0")
        .chain_update(state.selected.workspace_id().as_bytes())
        .chain_update(bound.prepared().identity().operation_id().as_bytes())
        .chain_update(bound.plan().digest());
    let schema = if let Some(index) = recipient {
        let seal = bound
            .plan()
            .seals()
            .get(index as usize)
            .ok_or(OperationError::RecoveryRequired)?;
        hash.update([1, index]);
        hash.update(seal.recipient().as_bytes());
        "radroots.browser.workspace.publication.v2"
    } else {
        hash.update([0]);
        "radroots.browser.workspace.message.v2"
    };
    let subject = hash
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    Ok(EnvelopeContext::new(
        EnvelopePurpose::parse("radroots.browser.workspace.installation")
            .map_err(|_| OperationError::InvalidCommand)?,
        EnvelopeSubject::parse("workspace_installation", subject)
            .map_err(|_| OperationError::InvalidCommand)?,
        PayloadSchemaId::parse(schema).map_err(|_| OperationError::InvalidCommand)?,
    ))
}
pub(super) struct Records {
    pub message: Option<ChatInstallation>,
    pub work: Vec<PublicationWork>,
}
impl Records {
    pub(super) async fn read(
        state: &State,
        view: &ReadSnapshot,
        graph: &Graph,
        operation: OperationId,
    ) -> Result<Self, OperationError> {
        let message = if let Some(raw) = state
            .read_view_key(view, address::key(operation, address::INSTALLATION, 0, 0))
            .await?
        {
            let bound = graph.bound().ok_or(OperationError::RecoveryRequired)?;
            let clear = crypto::open_bound(
                state.provider.as_ref(),
                &state.reference,
                context(state, bound, None)?,
                &raw,
            )
            .await?;
            Some(clear.expose_secret(|raw| codec::decode_message(raw, graph))?)
        } else {
            None
        };
        let mut work = Vec::with_capacity(2);
        if let Some(bound) = graph.bound() {
            for (index, seal) in bound.plan().seals().iter().enumerate() {
                if let Some(raw) = state
                    .read_view_key(
                        view,
                        address::key(operation, address::PUBLICATION, index as u8, 0),
                    )
                    .await?
                {
                    let installed = message.ok_or(OperationError::RecoveryRequired)?;
                    let clear = crypto::open_bound(
                        state.provider.as_ref(),
                        &state.reference,
                        context(state, bound, Some(index as u8))?,
                        &raw,
                    )
                    .await?;
                    work.push(clear.expose_secret(|raw| {
                        codec::decode_work(raw, graph, installed, seal.recipient())
                    })?);
                }
            }
        }
        if let Some(message) = message
            && !work.iter().any(|value| value.evidence() == message.basis())
        {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(Self { message, work })
    }
}
pub(super) fn work_commitment(message: ChatInstallation, work: PublicationWork) -> [u8; 32] {
    Sha256::digest(codec::work(message, work)).into()
}
impl State {
    /// A missing claim must not conceal installation/publication descendants.
    /// The complete witnessed prefix still fences all facts at actual commit.
    pub(super) async fn check_claim_absence(
        &self,
        view: &ReadSnapshot,
        operation: OperationId,
        digest: CommandDigest,
        t0: &PreparationRecords,
        observed: UnixSeconds,
    ) -> Result<(), OperationError> {
        self.delivery_facts(view, operation, Some(digest), t0, observed, None)
            .await?;
        Ok(())
    }
}
impl BrowserWorkspace {
    /// Installs the original logical message once and appends work for each
    /// actual obtained recipient. This performs no signing or delivery.
    pub async fn install_transition_and_outbox(
        &self,
        request: InstallationRequest,
        proof: AttachmentProof,
    ) -> Result<InstallationOutcome, OperationError> {
        let fallback = request.identity();
        let work = self.state.admit()?;
        let state = self.state.clone();
        let result = owned(work, async move {
            let access = state.operation_access(request.binding(), proof).await?;
            let operation = request.operation_id();
            let view = state.read_view(operation).await?;
            let t0 = state.operation_records(&view, operation).await?;
            let snapshot = state
                .delivery_facts(
                    &view,
                    operation,
                    Some(request.command_digest()),
                    &t0,
                    access.binding.observed_at(),
                    None,
                )
                .await?
                .snapshot
                .ok_or(OperationError::RecoveryRequired)?;
            let bound = snapshot
                .bound_plan()
                .ok_or(OperationError::RecoveryRequired)?;
            radroots_storage::command::protected::same_binding(
                bound.prepared().request().binding(),
                request.binding(),
            )?;
            if bound.plan().digest() != request.plan_digest() {
                return Err(OperationError::IdempotencyConflict);
            }
            let original = snapshot.installation();
            let missing = snapshot
                .recipients()
                .iter()
                .filter_map(|value| value.obtained())
                .filter(|evidence| {
                    !snapshot
                        .publications()
                        .iter()
                        .any(|value| value.evidence().recipient() == evidence.recipient())
                })
                .collect::<Vec<_>>();
            if original.is_none() && missing.is_empty() {
                return Err(OperationError::InvalidEvidence);
            }
            let fresh = original.is_none() || !missing.is_empty();
            let transaction = if fresh {
                TransactionId::from_bytes(*crypto::random::<32>()?)
            } else {
                original
                    .ok_or(OperationError::RecoveryRequired)?
                    .transaction_id()
            };
            let installed = if let Some(value) = original {
                value
            } else {
                ChatInstallation::from_committed_store(
                    bound,
                    *missing.first().ok_or(OperationError::InvalidEvidence)?,
                    transaction,
                    access.binding.observed_at(),
                )?
            };
            let mut extra = Vec::with_capacity(3);
            if original.is_none() {
                let raw = codec::message(bound, installed);
                let protected = crypto::seal_bound(
                    state.provider.as_ref(),
                    &state.reference,
                    context(&state, bound, None)?,
                    &raw,
                )
                .await?;
                extra.push(
                    Mutation::new(
                        address::key(operation, address::INSTALLATION, 0, 0),
                        None,
                        Some(protected),
                    )
                    .map_err(storage_error)?,
                );
            }
            for evidence in missing {
                let index = bound
                    .plan()
                    .seals()
                    .iter()
                    .position(|seal| seal.recipient() == evidence.recipient())
                    .ok_or(OperationError::RecoveryRequired)? as u8;
                let value = PublicationWork::from_committed_store(
                    bound,
                    installed,
                    evidence,
                    transaction,
                    access.binding.observed_at(),
                )?;
                let raw = codec::work(installed, value);
                let protected = crypto::seal_bound(
                    state.provider.as_ref(),
                    &state.reference,
                    context(&state, bound, Some(index))?,
                    &raw,
                )
                .await?;
                extra.push(
                    Mutation::new(
                        address::key(operation, address::PUBLICATION, index, 0),
                        None,
                        Some(protected),
                    )
                    .map_err(storage_error)?,
                );
            }
            let anchor = CommitAttempt::new(
                installed.workspace_id(),
                operation,
                installed.command_digest(),
                Some(transaction),
            );
            match state
                .check_operation(
                    access,
                    operation,
                    OperationChange {
                        before: t0,
                        after: None,
                        window: None,
                        delivery_deadline: None,
                        extra,
                    },
                    view,
                )
                .await
            {
                Ok(()) => Ok(InstallationOutcome::Committed(Box::new(installed))),
                Err(OperationError::Workspace(WorkspaceError::CommitUnknown)) => {
                    state.mark_unknown();
                    Ok(InstallationOutcome::Unknown(anchor))
                }
                Err(error) => Err(error),
            }
        })
        .await;
        match result {
            Err(OperationError::Workspace(WorkspaceError::CommitUnknown)) => {
                self.state.mark_unknown();
                Ok(InstallationOutcome::Unknown(fallback))
            }
            other => other,
        }
    }
    /// Confirms the original coherent T0/T1/S/T2 graph. Querying never dispatches
    /// a signer or implies a missing delivery acknowledgement.
    pub async fn operation_snapshot(
        &self,
        binding: WorkspaceBinding,
        operation: OperationId,
        digest: Option<CommandDigest>,
        proof: AttachmentProof,
    ) -> Result<Option<OperationSnapshot>, OperationError> {
        let work = self.state.admit()?;
        let state = self.state.clone();
        owned(work, async move {
            let access = state.operation_access(&binding, proof).await?;
            let observed = access.binding.observed_at();
            let view = state.read_view(operation).await?;
            let t0 = state.operation_records(&view, operation).await?;
            let snapshot = state
                .delivery_facts(&view, operation, digest, &t0, observed, None)
                .await?
                .snapshot;
            if let Some(snapshot) = &snapshot {
                radroots_storage::command::protected::same_binding(
                    snapshot.prepared().request().binding(),
                    &binding,
                )?;
            }
            state
                .check_operation(access, operation, OperationChange::observe(t0), view)
                .await?;
            Ok(snapshot)
        })
        .await
    }
}
