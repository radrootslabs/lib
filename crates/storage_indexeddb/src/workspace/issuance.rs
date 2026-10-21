//! Real S claims and append-only obtained output; no signer runs under storage.
use super::journal::{OperationChange, Records, mutations, storage_error};
use super::{BrowserWorkspace, Mutation, State, WorkspaceError, address, crypto, owned};
use crate::ReadSnapshot;
use radroots_domain::{CommandDigest, OperationId, SigningAttemptId, TransactionId};
use radroots_secrets::context::{
    EnvelopeContext, EnvelopePurpose, EnvelopeSubject, PayloadSchemaId,
};
use radroots_storage::{
    command::{
        ArtifactEvidence, BoundSigningPlan, CapturedArtifact, ClaimAttemptOutcome,
        ClaimAttemptRequest, CommitAttempt, EvidenceCaptureOutcome, EvidenceReceipt,
        OperationError, SigningAttempt, SigningTarget,
        protected::{
            self,
            issuance::{ClaimRecord, EvidenceManifest},
            plan::PART_BYTES,
        },
    },
    workspace::{AttachmentInput, AttachmentProof},
};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

pub(super) enum Claim {
    Absent(BoundSigningPlan, u8),
    Present(Box<SigningAttempt>),
}
pub(super) fn position(
    bound: &BoundSigningPlan,
    target: SigningTarget,
) -> Result<Option<u8>, OperationError> {
    if target == SigningTarget::PublicEvent {
        return Ok(bound.plan().listing().map(|_| 2));
    }
    let Some(chat) = bound.plan().chat() else {
        return Ok(None);
    };
    Ok(chat
        .seals()
        .iter()
        .position(|seal| Some(seal.recipient()) == target.recipient())
        .and_then(|i| u8::try_from(i).ok()))
}
pub(super) fn target(bound: &BoundSigningPlan, index: u8) -> Result<SigningTarget, OperationError> {
    if index == 2 && bound.plan().listing().is_some() {
        return Ok(SigningTarget::PublicEvent);
    }
    let chat = bound.chat_plan()?;
    chat.seals()
        .get(index as usize)
        .map(|seal| SigningTarget::BusinessSeal(seal.recipient()))
        .ok_or(OperationError::RecoveryRequired)
}
fn base(state: &State, bound: &BoundSigningPlan, index: u8) -> Result<Sha256, OperationError> {
    let identity = bound.prepared().identity();
    let target = target(bound, index)?;
    if target == SigningTarget::PublicEvent {
        return Ok(Sha256::new()
            .chain_update(b"radroots.browser.local_outbox.public_issuance.context.v1\0")
            .chain_update(state.selected.workspace_id().as_bytes())
            .chain_update(identity.operation_id().as_bytes())
            .chain_update(bound.plan().digest())
            .chain_update([index]));
    }
    Ok(Sha256::new()
        .chain_update(b"radroots.browser.local_outbox.issuance.context.v2\0")
        .chain_update(state.selected.workspace_id().as_bytes())
        .chain_update(identity.operation_id().as_bytes())
        .chain_update(bound.plan().digest())
        .chain_update(
            target
                .recipient()
                .ok_or(OperationError::RecoveryRequired)?
                .as_bytes(),
        )
        .chain_update([index]))
}
fn finish(hash: Sha256, schema: &str) -> Result<EnvelopeContext, OperationError> {
    let subject = hash
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    Ok(EnvelopeContext::new(
        EnvelopePurpose::parse("radroots.browser.workspace.issuance")
            .map_err(|_| OperationError::InvalidCommand)?,
        EnvelopeSubject::parse("workspace_issuance", subject)
            .map_err(|_| OperationError::InvalidCommand)?,
        PayloadSchemaId::parse(schema).map_err(|_| OperationError::InvalidCommand)?,
    ))
}
fn claim_context(
    state: &State,
    bound: &BoundSigningPlan,
    index: u8,
) -> Result<EnvelopeContext, OperationError> {
    finish(
        base(state, bound, index)?.chain_update(b"claim\0"),
        "radroots.browser.workspace.claim.v2",
    )
}
fn evidence_context(
    state: &State,
    attempt: &SigningAttempt,
    index: u8,
    part: u8,
    digest: Option<&[u8; 32]>,
) -> Result<EnvelopeContext, OperationError> {
    let mut hash = base(state, attempt.bound_plan(), index)?
        .chain_update(b"evidence\0")
        .chain_update(attempt.attempt_id().as_bytes())
        .chain_update([part]);
    if let Some(digest) = digest {
        hash.update(digest);
    }
    finish(hash, "radroots.browser.workspace.evidence.v2")
}
impl State {
    async fn bound(
        &self,
        view: &ReadSnapshot,
        operation: OperationId,
        digest: CommandDigest,
        t0: &Records,
    ) -> Result<Option<BoundSigningPlan>, OperationError> {
        match super::retained::header(self, view, operation, Some(digest), t0).await? {
            Some(radroots_storage::command::RetainedOperation::Planned(bound)) => Ok(Some(*bound)),
            _ => Ok(None),
        }
    }
    pub(super) async fn claim(
        &self,
        view: &ReadSnapshot,
        bound: BoundSigningPlan,
        recipient: SigningTarget,
        index: u8,
    ) -> Result<Claim, OperationError> {
        let operation = bound.prepared().identity().operation_id();
        let key = address::key(operation, address::CLAIM, index, 0);
        let Some(protected) = self.read_view_key(view, key).await? else {
            if view.keys().any(|key| matches!(address::fields(key), Ok((address::EVIDENCE, found, _)) if found == index)) { return Err(OperationError::RecoveryRequired); }
            return Ok(Claim::Absent(bound, index));
        };
        let clear = crypto::open_bound(
            self.provider.as_ref(),
            &self.reference,
            claim_context(self, &bound, index)?,
            &protected,
        )
        .await?;
        let attempt = clear.expose_secret(|raw| ClaimRecord::decode(raw, bound, recipient))?;
        Ok(Claim::Present(Box::new(attempt)))
    }
    pub(super) async fn evidence(
        &self,
        view: &ReadSnapshot,
        attempt: &SigningAttempt,
        index: u8,
    ) -> Result<Option<(ArtifactEvidence, EvidenceReceipt)>, OperationError> {
        let operation = attempt.bound_plan().prepared().identity().operation_id();
        let mut records = Vec::with_capacity(5);
        for part in 0..=4 {
            records.push(
                self.read_view_key(
                    view,
                    address::key(operation, address::EVIDENCE, index, part),
                )
                .await?,
            );
        }
        let Some(manifest) = &records[0] else {
            if records.iter().any(Option::is_some) {
                return Err(OperationError::RecoveryRequired);
            }
            return Ok(None);
        };
        let raw = crypto::open_bound(
            self.provider.as_ref(),
            &self.reference,
            evidence_context(self, attempt, index, 0, None)?,
            manifest,
        )
        .await?;
        let manifest = raw.expose_secret(|raw| EvidenceManifest::decode(raw, attempt))?;
        let mut clear = Zeroizing::new(Vec::with_capacity(manifest.byte_count()));
        for (part, bytes) in records.iter().enumerate().skip(1) {
            if part > manifest.part_count() {
                if bytes.is_some() {
                    return Err(OperationError::RecoveryRequired);
                }
                continue;
            }
            let bytes = bytes.as_ref().ok_or(OperationError::RecoveryRequired)?;
            let raw = crypto::open_bound(
                self.provider.as_ref(),
                &self.reference,
                evidence_context(
                    self,
                    attempt,
                    index,
                    part as u8,
                    Some(manifest.evidence_digest()),
                )?,
                bytes,
            )
            .await?;
            let expected = (manifest.byte_count() - (part - 1) * PART_BYTES).min(PART_BYTES);
            raw.expose_secret(|raw| {
                if raw.len() != expected {
                    return Err(OperationError::RecoveryRequired);
                }
                clear.extend_from_slice(raw);
                Ok(())
            })?;
        }
        let evidence = manifest.verify(
            attempt,
            core::str::from_utf8(&clear).map_err(|_| OperationError::RecoveryRequired)?,
        )?;
        let receipt = EvidenceReceipt::from_committed_store(&evidence, manifest.transaction_id());
        Ok(Some((evidence, receipt)))
    }
    async fn protect_evidence(
        &self,
        attempt: &SigningAttempt,
        index: u8,
        supplied: &ArtifactEvidence,
        transaction: TransactionId,
    ) -> Result<Vec<Mutation>, OperationError> {
        let manifest = EvidenceManifest::capture(attempt, supplied, transaction)?;
        let operation = supplied.operation_id();
        let encoded = manifest.encode()?;
        let protected = crypto::seal_bound(
            self.provider.as_ref(),
            &self.reference,
            evidence_context(self, attempt, index, 0, None)?,
            &encoded,
        )
        .await?;
        let mut changes = vec![
            Mutation::new(
                address::key(operation, address::EVIDENCE, index, 0),
                None,
                Some(protected),
            )
            .map_err(storage_error)?,
        ];
        for (part, raw) in supplied
            .raw_json()
            .as_bytes()
            .chunks(PART_BYTES)
            .enumerate()
        {
            let part = part as u8 + 1;
            let protected = crypto::seal_bound(
                self.provider.as_ref(),
                &self.reference,
                evidence_context(self, attempt, index, part, Some(supplied.digest()))?,
                raw,
            )
            .await?;
            changes.push(
                Mutation::new(
                    address::key(operation, address::EVIDENCE, index, part),
                    None,
                    Some(protected),
                )
                .map_err(storage_error)?,
            );
        }
        Ok(changes)
    }
}
impl BrowserWorkspace {
    /// Only an actual newly committed claim reports Claimed. Retained claims
    /// are potential prior issuance and must not be dispatched again.
    pub async fn claim_attempt(
        &self,
        request: ClaimAttemptRequest,
        proof: AttachmentProof,
    ) -> Result<ClaimAttemptOutcome, OperationError> {
        let fallback = request.identity();
        let work = self.state.admit()?;
        let state = self.state.clone();
        let result = owned(work, async move {
            let access = state.operation_access(request.binding(), proof).await?;
            let operation = request.operation_id();
            let view = state.read_view(operation).await?;
            let t0 = state.operation_records(&view, operation).await?;
            let bound = state
                .bound(&view, operation, request.command_digest(), &t0)
                .await?
                .ok_or(OperationError::RecoveryRequired)?;
            protected::same_binding(bound.prepared().request().binding(), request.binding())?;
            if bound.plan().digest() != request.plan_digest() {
                return Err(OperationError::IdempotencyConflict);
            }
            let index =
                position(&bound, request.target())?.ok_or(OperationError::InvalidCommand)?;
            let (attempt, fresh, extra) =
                match state.claim(&view, bound, request.target(), index).await? {
                    Claim::Present(attempt) => (*attempt, false, Vec::new()),
                    Claim::Absent(bound, index) => {
                        state
                            .check_claim_absence(
                                &view,
                                operation,
                                request.command_digest(),
                                &t0,
                                access.binding.observed_at(),
                            )
                            .await?;
                        let now = access.binding.observed_at();
                        if now < bound.bound_at() {
                            return Err(WorkspaceError::Unavailable.into());
                        }
                        if now >= bound.prepared().issuance_window().first_issue_not_after() {
                            return Err(OperationError::DeadlineExceeded);
                        }
                        let attempt = SigningAttempt::from_committed_store(
                            bound,
                            request.target(),
                            SigningAttemptId::from_bytes(*crypto::random::<32>()?),
                            TransactionId::from_bytes(*crypto::random::<32>()?),
                            now,
                        )?;
                        let encoded = ClaimRecord::capture(&attempt)?.encode()?;
                        let protected = crypto::seal_bound(
                            state.provider.as_ref(),
                            &state.reference,
                            claim_context(&state, attempt.bound_plan(), index)?,
                            &encoded,
                        )
                        .await?;
                        let extra = vec![
                            Mutation::new(
                                address::key(operation, address::CLAIM, index, 0),
                                None,
                                Some(protected),
                            )
                            .map_err(storage_error)?,
                        ];
                        (attempt, true, extra)
                    }
                };
            let anchor = attempt.identity();
            let change = OperationChange {
                before: t0,
                after: None,
                window: fresh.then(|| attempt.bound_plan().prepared().issuance_window()),
                delivery_deadline: None,
                extra,
            };
            match state.check_operation(access, operation, change, view).await {
                Ok(()) => Ok(if fresh {
                    ClaimAttemptOutcome::Claimed(attempt)
                } else {
                    ClaimAttemptOutcome::Retained(attempt)
                }),
                Err(OperationError::Workspace(WorkspaceError::CommitUnknown)) => {
                    state.mark_unknown();
                    Ok(ClaimAttemptOutcome::Unknown(anchor))
                }
                Err(error) => Err(error),
            }
        })
        .await;
        match result {
            Err(OperationError::Workspace(WorkspaceError::CommitUnknown)) => {
                self.state.mark_unknown();
                Ok(ClaimAttemptOutcome::Unknown(fallback))
            }
            other => other,
        }
    }
    /// Current-access original claim query; finding a claim authorizes no replay.
    pub async fn signing_attempt(
        &self,
        request: ClaimAttemptRequest,
        proof: AttachmentProof,
    ) -> Result<Option<SigningAttempt>, OperationError> {
        let work = self.state.admit()?;
        let state = self.state.clone();
        owned(work, async move {
            let access = state.operation_access(request.binding(), proof).await?;
            let operation = request.operation_id();
            let view = state.read_view(operation).await?;
            let t0 = state.operation_records(&view, operation).await?;
            let result = if let Some(bound) = state
                .bound(&view, operation, request.command_digest(), &t0)
                .await?
            {
                protected::same_binding(bound.prepared().request().binding(), request.binding())?;
                if bound.plan().digest() != request.plan_digest() {
                    return Err(OperationError::IdempotencyConflict);
                }
                if let Some(index) = position(&bound, request.target())? {
                    match state.claim(&view, bound, request.target(), index).await? {
                        Claim::Present(attempt) => Some(*attempt),
                        Claim::Absent(_, _) => {
                            state
                                .check_claim_absence(
                                    &view,
                                    operation,
                                    request.command_digest(),
                                    &t0,
                                    access.binding.observed_at(),
                                )
                                .await?;
                            None
                        }
                    }
                } else {
                    None
                }
            } else {
                None
            };
            state
                .check_operation(access, operation, OperationChange::observe(t0), view)
                .await?;
            Ok(result)
        })
        .await
    }
    /// Appends already obtained exact output to an actual original claim.
    /// No current clock or grant revocation can discard that retained fact.
    /// Borrowed original input remains caller-owned through error or lost wait;
    /// an accepted driver retains only its bounded protected-work copy.
    pub async fn capture_evidence(
        &self,
        input: AttachmentInput,
        proof: &AttachmentProof,
        supplied: &ArtifactEvidence,
    ) -> Result<EvidenceCaptureOutcome, OperationError> {
        let capture_identity = super::recovery::CaptureIdentity::artifact(supplied);
        let owned_identity = capture_identity.clone();
        let fallback = CommitAttempt::new(
            supplied.workspace_id(),
            supplied.operation_id(),
            supplied.command_digest(),
            None,
        );
        let work = self.state.admit_capture()?;
        let state = self.state.clone();
        let proof = proof.with_secret(|raw| AttachmentProof::from_protected_input(raw))?;
        let supplied = supplied.retained_copy();
        let result = owned(work, async move {
            let (grant, protected) = state.capture_retained(&input, &proof).await?;
            if supplied.workspace_id() != state.selected.workspace_id() {
                return Err(OperationError::InvalidEvidence);
            }
            let operation = supplied.operation_id();
            let view = state.read_view(operation).await?;
            let t0 = state.operation_records(&view, operation).await?;
            let bound = state
                .bound(&view, operation, supplied.command_digest(), &t0)
                .await?
                .ok_or(OperationError::RecoveryRequired)?;
            let index =
                position(&bound, supplied.target())?.ok_or(OperationError::InvalidEvidence)?;
            let attempt = match state.claim(&view, bound, supplied.target(), index).await? {
                Claim::Present(attempt) => attempt,
                Claim::Absent(_, _) => return Err(OperationError::RecoveryRequired),
            };
            if !supplied.matches_attempt(&attempt) {
                return Err(OperationError::InvalidEvidence);
            }
            let checked = ArtifactEvidence::verify(
                &attempt,
                supplied.raw_json(),
                supplied.observed_at_unix_ms(),
            )?;
            drop(supplied);
            let coordinate = state
                .coordinate_guard(attempt.bound_plan().prepared())
                .await?;
            let (receipt, mut extra) = match state.evidence(&view, &attempt, index).await? {
                Some((original, receipt)) => {
                    if original.raw_json() != checked.raw_json()
                        || original.digest() != checked.digest()
                    {
                        return Err(OperationError::IdempotencyConflict);
                    }
                    (receipt, Vec::new())
                }
                None => {
                    let transaction = TransactionId::from_bytes(*crypto::random::<32>()?);
                    let extra = state
                        .protect_evidence(&attempt, index, &checked, transaction)
                        .await?;
                    (
                        EvidenceReceipt::from_committed_store(&checked, transaction),
                        extra,
                    )
                }
            };
            let anchor = CommitAttempt::new(
                receipt.workspace_id(),
                operation,
                receipt.command_digest(),
                Some(receipt.transaction_id()),
            );
            extra.extend(coordinate);
            let batch = mutations(protected, operation, t0, None, extra)?;
            let active = state.active()?;
            match state
                .checked_batch(batch, view, move || {
                    if !active.get() {
                        return Err(WorkspaceError::Closed.into());
                    }
                    grant.authenticate_retained_capture(&input, &proof)?;
                    Ok(())
                })
                .await
            {
                Ok(()) => {
                    state.capture_confirmed(&owned_identity);
                    Ok(EvidenceCaptureOutcome::Committed(receipt))
                }
                Err(OperationError::Workspace(WorkspaceError::CommitUnknown)) => {
                    state.mark_capture_unknown(owned_identity);
                    Ok(EvidenceCaptureOutcome::Unknown(anchor))
                }
                Err(error) => Err(error),
            }
        })
        .await;
        match result {
            Err(OperationError::Workspace(WorkspaceError::CommitUnknown)) => {
                self.state.mark_capture_unknown(capture_identity);
                Ok(EvidenceCaptureOutcome::Unknown(fallback))
            }
            other => other,
        }
    }
    /// Reads original verified output under current access; no issuance occurs.
    pub async fn obtained_evidence(
        &self,
        request: ClaimAttemptRequest,
        proof: AttachmentProof,
    ) -> Result<Option<CapturedArtifact>, OperationError> {
        let work = self.state.admit()?;
        let state = self.state.clone();
        owned(work, async move {
            let access = state.operation_access(request.binding(), proof).await?;
            let operation = request.operation_id();
            let view = state.read_view(operation).await?;
            let t0 = state.operation_records(&view, operation).await?;
            let result = if let Some(bound) = state
                .bound(&view, operation, request.command_digest(), &t0)
                .await?
            {
                protected::same_binding(bound.prepared().request().binding(), request.binding())?;
                if bound.plan().digest() != request.plan_digest() {
                    return Err(OperationError::IdempotencyConflict);
                }
                if let Some(index) = position(&bound, request.target())? {
                    match state.claim(&view, bound, request.target(), index).await? {
                        Claim::Present(attempt) => {
                            let obtained = state.evidence(&view, &attempt, index).await?;
                            if obtained.is_none() {
                                state
                                    .check_claim_absence(
                                        &view,
                                        operation,
                                        request.command_digest(),
                                        &t0,
                                        access.binding.observed_at(),
                                    )
                                    .await?;
                            }
                            obtained
                                .map(|(evidence, receipt)| {
                                    CapturedArtifact::from_committed_store(
                                        *attempt,
                                        evidence,
                                        receipt.transaction_id(),
                                    )
                                })
                                .transpose()?
                        }
                        Claim::Absent(_, _) => {
                            state
                                .check_claim_absence(
                                    &view,
                                    operation,
                                    request.command_digest(),
                                    &t0,
                                    access.binding.observed_at(),
                                )
                                .await?;
                            None
                        }
                    }
                } else {
                    None
                }
            } else {
                None
            };
            state
                .check_operation(access, operation, OperationChange::observe(t0), view)
                .await?;
            Ok(result)
        })
        .await
    }
}
