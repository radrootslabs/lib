//! Protected immutable T0 records. No signer, network or business installation.
use super::{
    Batch, BrowserWorkspace, Error, Mutation, RecordKey, State, WorkspaceError, crypto, grant_key,
    owned,
};
use radroots_domain::{CommandDigest, IssuanceWindow, OperationId, Sequence, TransactionId};
use radroots_secrets::context::{
    EnvelopeContext, EnvelopePurpose, EnvelopeSubject, PayloadSchemaId,
};
use radroots_storage::{
    command::{
        AdmissionOutcome, AdmissionRequest, CommitAttempt, OperationError, PreparedOperation,
        protected,
    },
    workspace::{AttachmentProof, WorkspaceBinding, local_outbox::LocalOutboxGrant},
};
use sha2::{Digest, Sha256};
use std::{cell::Cell, rc::Rc};
use zeroize::Zeroizing;

const PART_BYTES: usize = 16 * 1024;
const PARTS: usize = protected::PREPARED_BYTES_MAX / PART_BYTES;
const SLOTS: usize = PARTS + 1;
pub(super) type Records = Vec<Option<Vec<u8>>>;

pub(super) struct OperationChange {
    pub before: Records,
    pub after: Option<Records>,
    pub window: Option<IssuanceWindow>,
    pub extra: Vec<Mutation>,
}
impl OperationChange {
    pub fn observe(before: Records) -> Self {
        Self {
            before,
            after: None,
            window: None,
            extra: Vec::new(),
        }
    }
}

fn key(operation: OperationId, slot: usize) -> RecordKey {
    let mut h = Sha256::new();
    h.update(b"radroots.browser.local_outbox.prepared.key.v1\0");
    h.update(operation.as_bytes());
    h.update((slot as u32).to_be_bytes());
    RecordKey::new(h.finalize().into())
}
fn context(
    state: &State,
    operation: OperationId,
    part: Option<(usize, &[u8])>,
) -> Result<EnvelopeContext, WorkspaceError> {
    let mut h = Sha256::new();
    h.update(b"radroots.browser.local_outbox.prepared.context.v1\0");
    h.update(state.selected.workspace_id().as_bytes());
    h.update(operation.as_bytes());
    if let Some((index, digest)) = part {
        h.update(b"part\0");
        h.update((index as u32).to_be_bytes());
        h.update(digest);
    } else {
        h.update(b"manifest\0");
    }
    let subject = h
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    Ok(EnvelopeContext::new(
        EnvelopePurpose::parse("radroots.browser.workspace.prepared")
            .map_err(|_| WorkspaceError::InvalidInput)?,
        EnvelopeSubject::parse("workspace_prepared", subject)
            .map_err(|_| WorkspaceError::InvalidInput)?,
        PayloadSchemaId::parse("radroots.browser.workspace.prepared.v1")
            .map_err(|_| WorkspaceError::InvalidInput)?,
    ))
}
impl State {
    pub(super) async fn operation_records(
        &self,
        operation: OperationId,
    ) -> Result<Records, OperationError> {
        let mut records = Vec::with_capacity(SLOTS);
        for slot in 0..SLOTS {
            let value = self
                .db
                .get(key(operation, slot))
                .await
                .map_err(storage_error)?;
            if value
                .as_ref()
                .is_some_and(|bytes| bytes.len() > crypto::PROTECTED_MAX)
            {
                return Err(OperationError::RecoveryRequired);
            }
            records.push(value);
        }
        Ok(records)
    }
    pub(super) async fn preparation(
        &self,
        operation: OperationId,
        records: &Records,
    ) -> Result<Option<PreparedOperation>, OperationError> {
        let Some(manifest) = &records[0] else {
            if records.iter().any(Option::is_some) {
                return Err(OperationError::RecoveryRequired);
            }
            return Ok(None);
        };
        let manifest = crypto::open_bound(
            self.provider.as_ref(),
            &self.reference,
            context(self, operation, None)?,
            manifest,
        )
        .await?;
        let (length, digest) = manifest.expose_secret(|bytes| {
            if bytes.len() != 40 || &bytes[..4] != b"RRP1" {
                return Err(OperationError::RecoveryRequired);
            }
            let length = u32::from_be_bytes(
                bytes[4..8]
                    .try_into()
                    .map_err(|_| OperationError::RecoveryRequired)?,
            ) as usize;
            if length == 0 || length > protected::PREPARED_BYTES_MAX {
                return Err(OperationError::RecoveryRequired);
            }
            let digest: [u8; 32] = bytes[8..]
                .try_into()
                .map_err(|_| OperationError::RecoveryRequired)?;
            Ok((length, digest))
        })?;
        let count = length.div_ceil(PART_BYTES);
        let mut plaintext = Zeroizing::new(Vec::with_capacity(length));
        for index in 0..PARTS {
            if index >= count {
                if records[index + 1].is_some() {
                    return Err(OperationError::RecoveryRequired);
                }
                continue;
            }
            let part = records[index + 1]
                .as_ref()
                .ok_or(OperationError::RecoveryRequired)?;
            let opened = crypto::open_bound(
                self.provider.as_ref(),
                &self.reference,
                context(self, operation, Some((index, &digest)))?,
                part,
            )
            .await?;
            let expected = (length - index * PART_BYTES).min(PART_BYTES);
            opened.expose_secret(|bytes| {
                if bytes.len() != expected {
                    return Err(OperationError::RecoveryRequired);
                }
                plaintext.extend_from_slice(bytes);
                Ok(())
            })?;
        }
        if Sha256::digest(plaintext.as_slice()).as_slice() != digest {
            return Err(OperationError::RecoveryRequired);
        }
        protected::decode(&plaintext, &self.selected, operation).map(Some)
    }
    async fn protect_preparation(
        &self,
        request: &AdmissionRequest,
        transaction: TransactionId,
        window: IssuanceWindow,
    ) -> Result<Records, OperationError> {
        let plaintext = protected::encode(request, transaction, window)?;
        let digest = Sha256::digest(plaintext.as_slice());
        let mut manifest = Zeroizing::new(Vec::with_capacity(40));
        manifest.extend_from_slice(b"RRP1");
        manifest.extend_from_slice(&(plaintext.len() as u32).to_be_bytes());
        manifest.extend_from_slice(&digest);
        let operation = request.command().context().operation_id();
        let mut records = Vec::with_capacity(SLOTS);
        records.push(Some(
            crypto::seal_bound(
                self.provider.as_ref(),
                &self.reference,
                context(self, operation, None)?,
                &manifest,
            )
            .await?,
        ));
        for (index, bytes) in plaintext.chunks(PART_BYTES).enumerate() {
            records.push(Some(
                crypto::seal_bound(
                    self.provider.as_ref(),
                    &self.reference,
                    context(self, operation, Some((index, &digest)))?,
                    bytes,
                )
                .await?,
            ));
        }
        records.resize_with(SLOTS, || None);
        Ok(records)
    }
}
pub(super) fn storage_error(error: Error) -> OperationError {
    match error {
        Error::Capacity => WorkspaceError::Capacity.into(),
        Error::Closed => WorkspaceError::Closed.into(),
        Error::CommitUnknown => WorkspaceError::CommitUnknown.into(),
        Error::Aborted | Error::Unavailable => OperationError::StorageFailure,
        _ => OperationError::RecoveryRequired,
    }
}
pub(super) struct Access {
    grant: LocalOutboxGrant,
    protected: Vec<u8>,
    pub binding: WorkspaceBinding,
    proof: AttachmentProof,
}
impl State {
    pub(super) async fn operation_access(
        &self,
        binding: &WorkspaceBinding,
        proof: AttachmentProof,
    ) -> Result<Access, OperationError> {
        let (grant, protected, current) = self.access(binding.input(), &proof).await?;
        protected::same_binding(binding, &current)?;
        Ok(Access {
            grant,
            protected,
            binding: current,
            proof,
        })
    }
    pub(super) async fn check_operation(
        &self,
        access: Access,
        operation: OperationId,
        change: OperationChange,
    ) -> Result<(), OperationError> {
        let OperationChange {
            before,
            after,
            window,
            extra,
        } = change;
        let mut mutations = Vec::with_capacity(SLOTS + 1 + extra.len());
        mutations.push(
            Mutation::new(
                grant_key(),
                Some(access.protected.clone()),
                Some(access.protected),
            )
            .map_err(storage_error)?,
        );
        for (slot, old) in before.into_iter().enumerate() {
            let new = after
                .as_ref()
                .map_or_else(|| old.clone(), |r| r[slot].clone());
            mutations.push(Mutation::new(key(operation, slot), old, new).map_err(storage_error)?);
        }
        mutations.extend(extra);
        let batch = Batch::new(mutations).map_err(storage_error)?;
        let active = self.active()?;
        let clock = self.clock.clone();
        let failure = Rc::new(Cell::new(None));
        let refused = failure.clone();
        let result = self
            .db
            .compare_exchange_checked(batch, move || {
                let eligible = (|| {
                    if !active.get() {
                        return Err(WorkspaceError::Closed.into());
                    }
                    let now = clock.now()?;
                    if now.get() == 0 || now < access.binding.observed_at() {
                        return Err(WorkspaceError::Unavailable.into());
                    }
                    let current =
                        access
                            .grant
                            .authenticate(access.binding.input(), &access.proof, now)?;
                    protected::same_binding(&access.binding, &current)?;
                    if let Some(window) = window {
                        if now < window.prepared_at() {
                            return Err(WorkspaceError::Unavailable.into());
                        }
                        if now >= window.first_issue_not_after() {
                            return Err(OperationError::DeadlineExceeded);
                        }
                    }
                    Ok(())
                })();
                if let Err(error) = eligible {
                    refused.set(Some(error));
                    false
                } else {
                    true
                }
            })
            .await;
        if matches!(result, Err(Error::GuardRefused)) {
            return Err(failure.get().unwrap_or(OperationError::RecoveryRequired));
        }
        match result {
            Ok(_) => Ok(()),
            Err(Error::CommitUnknown) => Err(WorkspaceError::CommitUnknown.into()),
            Err(error) => Err(storage_error(error)),
        }
    }
}
impl BrowserWorkspace {
    /// Atomically retains an authenticated immutable local buyer preparation.
    /// Only actual commit/re-read yields Prepared; no signing or delivery occurs.
    pub async fn admit_command_and_hold(
        &self,
        request: AdmissionRequest,
        proof: AttachmentProof,
    ) -> Result<AdmissionOutcome, OperationError> {
        let context = request.command().context();
        let fallback = CommitAttempt::new(
            context.workspace_id(),
            context.operation_id(),
            request.command().digest(),
            None,
        );
        let work = self.state.admit()?;
        let state = self.state.clone();
        let result = owned(work, async move {
            let access = state.operation_access(request.binding(), proof).await?;
            let context = request.command().context();
            let operation = context.operation_id();
            let before = state.operation_records(operation).await?;
            let extra = if before.iter().all(Option::is_none) {
                state.absent_plans(operation).await?
            } else {
                Vec::new()
            };
            let (prepared, after, fresh) =
                if let Some(existing) = state.preparation(operation, &before).await? {
                    protected::check_digest(
                        &existing,
                        context.actor(),
                        Some(request.command().digest()),
                    )?;
                    (existing, None, false)
                } else {
                    if request.command().parent().is_some() || context.expected().is_some() {
                        return Err(OperationError::MissingHistory);
                    }
                    if context.approval().is_some() {
                        return Err(OperationError::UnverifiedApproval);
                    }
                    if access.binding.observed_at() < request.binding().observed_at() {
                        return Err(WorkspaceError::Unavailable.into());
                    }
                    if context.not_after() > access.binding.expires_at() {
                        return Err(OperationError::DeadlineExceeded);
                    }
                    let window =
                        IssuanceWindow::capture(access.binding.observed_at(), context.not_after())
                            .map_err(|_| OperationError::DeadlineExceeded)?;
                    let transaction = TransactionId::from_bytes(*crypto::random::<32>()?);
                    let records = state
                        .protect_preparation(&request, transaction, window)
                        .await?;
                    let prepared = PreparedOperation::from_committed_store(
                        request,
                        transaction,
                        Sequence::new(1).map_err(|_| OperationError::RecoveryRequired)?,
                        window,
                    )?;
                    (prepared, Some(records), true)
                };
            let attempt = prepared.identity();
            match state
                .check_operation(
                    access,
                    operation,
                    OperationChange {
                        before,
                        after,
                        window: fresh.then(|| prepared.issuance_window()),
                        extra,
                    },
                )
                .await
            {
                Ok(()) => Ok(AdmissionOutcome::Committed(prepared)),
                Err(OperationError::Workspace(WorkspaceError::CommitUnknown)) => {
                    state.stop();
                    Ok(AdmissionOutcome::Unknown(attempt))
                }
                Err(error) => Err(error),
            }
        })
        .await;
        match result {
            Err(OperationError::Workspace(WorkspaceError::CommitUnknown)) => {
                self.state.stop();
                Ok(AdmissionOutcome::Unknown(fallback))
            }
            other => other,
        }
    }
    /// Reads the original preparation under current access without replaying it.
    /// Absent results are confirmed atomically against every fixed record slot.
    pub async fn query_operation(
        &self,
        binding: WorkspaceBinding,
        operation: OperationId,
        digest: Option<CommandDigest>,
        proof: AttachmentProof,
    ) -> Result<Option<PreparedOperation>, OperationError> {
        let work = self.state.admit()?;
        let state = self.state.clone();
        owned(work, async move {
            let access = state.operation_access(&binding, proof).await?;
            let records = state.operation_records(operation).await?;
            let result = state.preparation(operation, &records).await?;
            if let Some(prepared) = &result {
                protected::check_digest(prepared, binding.actor(), digest)?;
            }
            let mut change = OperationChange::observe(records);
            if result.is_none() {
                change.extra = state.absent_plans(operation).await?;
            }
            state.check_operation(access, operation, change).await?;
            Ok(result)
        })
        .await
    }
}
