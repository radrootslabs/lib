//! Actual protected T1 storage; no participant issuance or network effects.
use super::journal::{OperationChange, Records, storage_error};
use super::{BrowserWorkspace, Mutation, RecordKey, State, WorkspaceError, crypto, owned};
use crate::ReadSnapshot;
use radroots_domain::{CommandDigest, OperationId, TransactionId};
use radroots_secrets::context::{
    EnvelopeContext, EnvelopePurpose, EnvelopeSubject, PayloadSchemaId,
};
use radroots_storage::{
    command::{
        BoundSigningPlan, ChatSigningPlan, CommitAttempt, OperationError, PlanBindingOutcome,
        PreparedOperation,
        protected::{
            self,
            plan::{PART_BYTES, PARTS_MAX, PlanManifest},
        },
    },
    workspace::{AttachmentProof, WorkspaceBinding},
};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

const SLOTS: usize = PARTS_MAX + 1;
fn key(operation: OperationId, slot: usize) -> RecordKey {
    super::address::key(operation, super::address::PLAN, 0, slot as u8)
}

fn context(
    state: &State,
    operation: OperationId,
    part: Option<(usize, CommandDigest)>,
) -> Result<EnvelopeContext, WorkspaceError> {
    let mut h = Sha256::new();
    h.update(b"radroots.browser.local_outbox.plan.context.v2\0");
    h.update(state.selected.workspace_id().as_bytes());
    h.update(operation.as_bytes());
    if let Some((index, digest)) = part {
        h.update(b"part\0");
        h.update((index as u32).to_be_bytes());
        h.update(digest.as_bytes());
    } else {
        h.update(b"manifest\0");
    }
    let subject = h
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    Ok(EnvelopeContext::new(
        EnvelopePurpose::parse("radroots.browser.workspace.plan")
            .map_err(|_| WorkspaceError::InvalidInput)?,
        EnvelopeSubject::parse("workspace_plan", subject)
            .map_err(|_| WorkspaceError::InvalidInput)?,
        PayloadSchemaId::parse("radroots.browser.workspace.plan.v2")
            .map_err(|_| WorkspaceError::InvalidInput)?,
    ))
}
fn comparisons(
    operation: OperationId,
    before: Records,
    after: Option<Records>,
) -> Result<Vec<Mutation>, OperationError> {
    before
        .into_iter()
        .enumerate()
        .map(|(slot, old)| {
            let new = after
                .as_ref()
                .map_or_else(|| old.clone(), |values| values[slot].clone());
            Mutation::new(key(operation, slot), old, new).map_err(storage_error)
        })
        .collect()
}
impl State {
    pub(super) async fn plan_records(
        &self,
        view: &ReadSnapshot,
        operation: OperationId,
    ) -> Result<Records, OperationError> {
        let mut records = Vec::with_capacity(SLOTS);
        for slot in 0..SLOTS {
            let value = self.read_view_key(view, key(operation, slot)).await?;
            if value
                .as_ref()
                .is_some_and(|v| v.len() > crypto::PROTECTED_MAX)
            {
                return Err(OperationError::RecoveryRequired);
            }
            records.push(value);
        }
        Ok(records)
    }
    pub(super) async fn absent_plans(
        &self,
        view: &ReadSnapshot,
        operation: OperationId,
    ) -> Result<Vec<Mutation>, OperationError> {
        let before = self.plan_records(view, operation).await?;
        if before.iter().any(Option::is_some)
            || view.keys().any(|key| {
                !matches!(
                    super::address::fields(key),
                    Ok((super::address::PREPARED, _, _))
                )
            })
        {
            return Err(OperationError::RecoveryRequired);
        }
        comparisons(operation, before, None)
    }
    pub(super) async fn retained_plan(
        &self,
        prepared: &PreparedOperation,
        records: &Records,
    ) -> Result<Option<(PlanManifest, ChatSigningPlan)>, OperationError> {
        let Some(manifest) = &records[0] else {
            if records.iter().any(Option::is_some) {
                return Err(OperationError::RecoveryRequired);
            }
            return Ok(None);
        };
        let operation = prepared.identity().operation_id();
        let clear = crypto::open_bound(
            self.provider.as_ref(),
            &self.reference,
            context(self, operation, None)?,
            manifest,
        )
        .await?;
        let manifest = clear.expose_secret(|bytes| PlanManifest::decode(bytes, prepared))?;
        let mut wire = Zeroizing::new(Vec::with_capacity(manifest.byte_count()));
        for index in 0..PARTS_MAX {
            if index >= manifest.part_count() {
                if records[index + 1].is_some() {
                    return Err(OperationError::RecoveryRequired);
                }
                continue;
            }
            let protected = records[index + 1]
                .as_ref()
                .ok_or(OperationError::RecoveryRequired)?;
            let bytes = crypto::open_bound(
                self.provider.as_ref(),
                &self.reference,
                context(self, operation, Some((index + 1, manifest.plan_digest())))?,
                protected,
            )
            .await?;
            let expected = (manifest.byte_count() - index * PART_BYTES).min(PART_BYTES);
            bytes.expose_secret(|bytes| {
                if bytes.len() != expected {
                    return Err(OperationError::RecoveryRequired);
                }
                wire.extend_from_slice(bytes);
                Ok(())
            })?;
        }
        let plan = ChatSigningPlan::parse(prepared.request().chat_command()?, &wire)
            .map_err(|_| OperationError::RecoveryRequired)?;
        if plan.digest() != manifest.plan_digest().as_bytes() {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(Some((manifest, plan)))
    }
    async fn protect_plan(
        &self,
        operation: OperationId,
        manifest: &PlanManifest,
        plan: &ChatSigningPlan,
    ) -> Result<Records, OperationError> {
        let mut records = Vec::with_capacity(SLOTS);
        let bytes = manifest.encode()?;
        records.push(Some(
            crypto::seal_bound(
                self.provider.as_ref(),
                &self.reference,
                context(self, operation, None)?,
                &bytes,
            )
            .await?,
        ));
        for (index, part) in plan.protected_wire().chunks(PART_BYTES).enumerate() {
            records.push(Some(
                crypto::seal_bound(
                    self.provider.as_ref(),
                    &self.reference,
                    context(self, operation, Some((index + 1, manifest.plan_digest())))?,
                    part,
                )
                .await?,
            ));
        }
        records.resize_with(SLOTS, || None);
        Ok(records)
    }
}
impl BrowserWorkspace {
    /// Atomically binds a complete original plan before any participant issuance.
    /// Exact retained replay preserves its transaction/observation after expiry.
    pub async fn bind_signing_intents(
        &self,
        binding: WorkspaceBinding,
        supplied: ChatSigningPlan,
        proof: AttachmentProof,
    ) -> Result<PlanBindingOutcome, OperationError> {
        let fallback = CommitAttempt::new(
            binding.input().workspace_id(),
            supplied.operation_id(),
            supplied.command_digest(),
            None,
        );
        let work = self.state.admit()?;
        let state = self.state.clone();
        let result = owned(work, async move {
            let access = state.operation_access(&binding, proof).await?;
            let operation = supplied.operation_id();
            let view = state.read_view(operation).await?;
            let t0 = state.operation_records(&view, operation).await?;
            let prepared = state
                .preparation(operation, &t0)
                .await?
                .ok_or(OperationError::RecoveryRequired)?;
            protected::check_digest(&prepared, binding.actor(), Some(supplied.command_digest()))?;
            protected::same_binding(prepared.request().binding(), &binding)?;
            let before = state.plan_records(&view, operation).await?;
            let (manifest, plan, after, fresh) = if let Some((manifest, plan)) =
                state.retained_plan(&prepared, &before).await?
            {
                if plan.protected_wire() != supplied.protected_wire() {
                    return Err(OperationError::IdempotencyConflict);
                }
                (manifest, plan, None, false)
            } else {
                if view.keys().any(|key| {
                    !matches!(
                        super::address::fields(key),
                        Ok((super::address::PREPARED, _, _))
                    )
                }) {
                    return Err(OperationError::RecoveryRequired);
                }
                let window = prepared.issuance_window();
                let observed = access.binding.observed_at();
                if observed < window.prepared_at() {
                    return Err(WorkspaceError::Unavailable.into());
                }
                if observed >= window.first_issue_not_after() {
                    return Err(OperationError::DeadlineExceeded);
                }
                let checked = ChatSigningPlan::parse(
                    prepared.request().chat_command()?,
                    supplied.protected_wire(),
                )
                .map_err(|_| OperationError::InvalidCommand)?;
                drop(supplied);
                let transaction = TransactionId::from_bytes(*crypto::random::<32>()?);
                let manifest = PlanManifest::capture(&prepared, &checked, transaction, observed)?;
                let records = state.protect_plan(operation, &manifest, &checked).await?;
                (manifest, checked, Some(records), true)
            };
            let attempt = CommitAttempt::new(
                prepared.identity().workspace_id(),
                operation,
                prepared.identity().command_digest(),
                Some(manifest.transaction_id()),
            );
            let change = OperationChange {
                before: t0,
                after: None,
                window: fresh.then(|| prepared.issuance_window()),
                delivery_deadline: None,
                extra: comparisons(operation, before, after)?,
            };
            match state.check_operation(access, operation, change, view).await {
                Ok(()) => Ok(PlanBindingOutcome::Committed(
                    manifest.bind(prepared, plan)?,
                )),
                Err(OperationError::Workspace(WorkspaceError::CommitUnknown)) => {
                    state.mark_unknown();
                    Ok(PlanBindingOutcome::Unknown(attempt))
                }
                Err(error) => Err(error),
            }
        })
        .await;
        match result {
            Err(OperationError::Workspace(WorkspaceError::CommitUnknown)) => {
                self.state.mark_unknown();
                Ok(PlanBindingOutcome::Unknown(fallback))
            }
            other => other,
        }
    }
    /// Authenticates and confirms the original plan without issuance or replay.
    pub async fn signing_plan(
        &self,
        binding: WorkspaceBinding,
        operation: OperationId,
        digest: CommandDigest,
        proof: AttachmentProof,
    ) -> Result<Option<BoundSigningPlan>, OperationError> {
        let work = self.state.admit()?;
        let state = self.state.clone();
        owned(work, async move {
            let access = state.operation_access(&binding, proof).await?;
            let view = state.read_view(operation).await?;
            let t0 = state.operation_records(&view, operation).await?;
            let prepared = state.preparation(operation, &t0).await?;
            let records = state.plan_records(&view, operation).await?;
            let retained = if let Some(prepared) = prepared {
                protected::check_digest(&prepared, binding.actor(), Some(digest))?;
                protected::same_binding(prepared.request().binding(), &binding)?;
                let plan = state.retained_plan(&prepared, &records).await?;
                if plan.is_none() {
                    state.absent_plans(&view, operation).await?;
                }
                Some((prepared, plan))
            } else {
                if !view.is_empty() {
                    return Err(OperationError::RecoveryRequired);
                }
                None
            };
            let change = OperationChange {
                before: t0,
                after: None,
                window: None,
                delivery_deadline: None,
                extra: comparisons(operation, records, None)?,
            };
            state
                .check_operation(access, operation, change, view)
                .await?;
            match retained {
                Some((prepared, Some((manifest, plan)))) => manifest.bind(prepared, plan).map(Some),
                _ => Ok(None),
            }
        })
        .await
    }
}
