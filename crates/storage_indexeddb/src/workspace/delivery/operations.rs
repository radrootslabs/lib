use super::{Selection, codec, graph, routes};
use crate::workspace::{
    BrowserWorkspace, WorkspaceError, crypto, installation::work_commitment,
    journal::OperationChange, owned,
};
use nostr::JsonUtil;
use radroots_domain::{CommandDigest, OperationId, PublicKey, TransactionId};
use radroots_storage::{
    command::{
        BoundSigningPlan, CommitAttempt, DeliveryAttempt, DeliveryAttemptOutcome,
        DeliveryAttemptRequest, DeliveryAttemptSummary, EnvelopeCandidate, OperationError,
        PublicationWork, SealEvidence,
    },
    workspace::{AttachmentProof, WorkspaceBinding},
};
use radroots_transport::sink::DeliveryRequestId;
use zeroize::Zeroizing;

fn contains_original(
    candidate: &EnvelopeCandidate,
    work: PublicationWork,
    bound: &BoundSigningPlan,
    source: &SealEvidence,
) -> Result<(), OperationError> {
    let actor = bound
        .prepared()
        .request()
        .command()
        .context()
        .actor()
        .participant();
    if candidate.work() != work
        || candidate.author() == actor
        || candidate.author() == bound.prepared().request().chat_command()?.peer()
        || candidate.author() == work.evidence().recipient()
        || source.digest() != work.evidence().digest()
    {
        return Err(OperationError::InvalidEvidence);
    }
    let event = nostr::Event::from_json(candidate.raw_json())
        .map_err(|_| OperationError::InvalidEvidence)?;
    let mut secp = nostr::secp256k1::Secp256k1::new();
    secp.seeded_randomize(&*crypto::random::<32>()?);
    event
        .verify_with_ctx(&secp)
        .map_err(|_| OperationError::InvalidEvidence)?;
    let recipient = nostr::PublicKey::from_slice(work.evidence().recipient().as_bytes())
        .map_err(|_| OperationError::InvalidEvidence)?;
    let opened = candidate.with_wrapping_key(|key| {
        let secret =
            nostr::SecretKey::from_slice(key).map_err(|_| OperationError::InvalidEvidence)?;
        let keys = nostr::Keys::new_with_ctx(&secp, secret);
        if keys.public_key().to_bytes() != *candidate.author().as_bytes() {
            return Err(OperationError::InvalidEvidence);
        }
        nostr::nips::nip44::decrypt(keys.secret_key(), &recipient, &event.content)
            .map(Zeroizing::new)
            .map_err(|_| OperationError::InvalidEvidence)
    })?;
    if opened.as_bytes() != source.raw_json().as_bytes() {
        return Err(OperationError::InvalidEvidence);
    }
    Ok(())
}
fn unknown(operation: CommitAttempt, attempt: DeliveryRequestId) -> DeliveryAttemptOutcome {
    DeliveryAttemptOutcome::Unknown { operation, attempt }
}
impl BrowserWorkspace {
    /// Retains actual original outer bytes and selected target intent before a
    /// potential invocation. Only a fresh actual commit yields `Claimed`.
    pub async fn prepare_delivery_attempt(
        &self,
        request: DeliveryAttemptRequest,
        proof: AttachmentProof,
        candidate: Option<&EnvelopeCandidate>,
    ) -> Result<DeliveryAttemptOutcome, OperationError> {
        let fallback = request.identity();
        let request_id = request.intent().request_id().clone();
        let work = self.state.admit()?;
        let candidate = candidate.map(EnvelopeCandidate::retained_copy);
        let state = self.state.clone();
        let result = owned(work, async move {
            let access = state.operation_access(request.binding(), proof).await?;
            let actual = request.work();
            let evidence = actual.evidence();
            let operation = evidence.operation_id();
            let view = state.read_view(operation).await?;
            let t0 = state.operation_records(&view, operation).await?;
            let facts = state
                .delivery_facts(
                    &view,
                    operation,
                    Some(evidence.command_digest()),
                    &t0,
                    access.binding.observed_at(),
                    Some(Selection {
                        recipient: evidence.recipient(),
                        request: request.intent().request_id(),
                        event: Some(request.intent().event_id()),
                        retain_seal: true,
                    }),
                )
                .await?;
            let snapshot = facts.snapshot.ok_or(OperationError::RecoveryRequired)?;
            radroots_storage::command::protected::same_binding(
                snapshot.prepared().request().binding(),
                request.binding(),
            )?;
            if !snapshot.publications().contains(&actual) {
                return Err(OperationError::InvalidEvidence);
            }
            let bound = snapshot
                .bound_plan()
                .ok_or(OperationError::RecoveryRequired)?;
            let recipient = bound
                .plan()
                .seals()
                .iter()
                .position(|seal| seal.recipient() == evidence.recipient())
                .ok_or(OperationError::InvalidEvidence)? as u8;
            let retained = facts
                .selected
                .as_ref()
                .filter(|value| value.intent().request_id() == request.intent().request_id());
            if let Some(retained) = retained
                && (retained.work() != actual
                    || retained.intent().digest() != request.intent().digest()
                    || candidate.as_ref().is_some_and(|value| {
                        value.work() != actual || value.raw_json() != retained.raw_json()
                    }))
            {
                return Err(OperationError::IdempotencyConflict);
            }
            let fresh = retained.is_none();
            let (report, extra) = if fresh {
                routes(bound, evidence.recipient(), request.intent())?;
                if snapshot.delivery_attempts().iter().any(|value| {
                    value.work() == actual
                        && value.intent().publication_policy_digest()
                            != request.intent().publication_policy_digest()
                }) {
                    return Err(OperationError::IdempotencyConflict);
                }
                let now = access.binding.observed_at();
                if now < actual.queued_at()
                    || snapshot
                        .delivery_attempts()
                        .iter()
                        .any(|value| value.retained_at() > now)
                {
                    return Err(WorkspaceError::Unavailable.into());
                }
                if now
                    .get()
                    .checked_mul(1000)
                    .is_none_or(|time| time >= request.intent().deadline_unix_ms().get())
                {
                    return Err(OperationError::DeadlineExceeded);
                }
                let mut ordinals = Vec::with_capacity(request.intent().selection().len());
                for target in request.intent().selected_targets() {
                    let count = snapshot
                        .delivery_attempts()
                        .iter()
                        .filter(|value| value.work() == actual)
                        .flat_map(|value| value.intent().selected_targets())
                        .filter(|old| old.uri() == target.uri())
                        .count();
                    if count >= 12 {
                        return Err(OperationError::DeliveryAttemptLimit);
                    }
                    ordinals.push(count as u8 + 1);
                }
                let raw = if let Some(candidate) = &candidate {
                    if candidate.event_id() != request.intent().event_id() {
                        return Err(OperationError::InvalidEvidence);
                    }
                    contains_original(
                        candidate,
                        actual,
                        bound,
                        facts
                            .original
                            .as_ref()
                            .ok_or(OperationError::RecoveryRequired)?,
                    )?;
                    Zeroizing::new(candidate.raw_json().to_owned())
                } else {
                    let original = facts
                        .selected
                        .as_ref()
                        .filter(|value| {
                            value.work() == actual
                                && value.intent().event_id() == request.intent().event_id()
                        })
                        .ok_or(OperationError::InvalidEvidence)?;
                    Zeroizing::new(original.raw_json().to_owned())
                };
                if facts.selected.as_ref().is_some_and(|value| {
                    value.intent().event_id() == request.intent().event_id()
                        && value.raw_json() != raw.as_str()
                }) {
                    return Err(OperationError::IdempotencyConflict);
                }
                if facts.slots[recipient as usize] >= 192 {
                    return Err(OperationError::DeliveryAttemptLimit);
                }
                let report = DeliveryAttempt::from_store_report(
                    actual,
                    request.intent().clone(),
                    &raw,
                    TransactionId::from_bytes(*crypto::random::<32>()?),
                    now,
                    ordinals,
                )?;
                let commitment = work_commitment(
                    snapshot
                        .installation()
                        .ok_or(OperationError::RecoveryRequired)?,
                    actual,
                );
                let mut summaries = snapshot.delivery_attempts().to_vec();
                summaries.push(DeliveryAttemptSummary::from_attempt(&report));
                snapshot.with_delivery(summaries, Vec::new())?;
                let extra = graph::protect(
                    &state,
                    operation,
                    recipient,
                    facts.slots[recipient as usize] + 1,
                    commitment,
                    &report,
                )
                .await?;
                (report, extra)
            } else {
                (
                    facts.selected.ok_or(OperationError::RecoveryRequired)?,
                    Vec::new(),
                )
            };
            let anchor = CommitAttempt::new(
                evidence.workspace_id(),
                operation,
                evidence.command_digest(),
                Some(report.transaction_id()),
            );
            let identity = report.intent().request_id().clone();
            let deadline = fresh.then_some(report.intent().deadline_unix_ms());
            match state
                .check_operation(
                    access,
                    operation,
                    OperationChange {
                        before: t0,
                        after: None,
                        window: None,
                        delivery_deadline: deadline,
                        extra,
                    },
                    view,
                )
                .await
            {
                Ok(()) => Ok(if fresh {
                    DeliveryAttemptOutcome::Claimed(Box::new(report))
                } else {
                    DeliveryAttemptOutcome::Retained(Box::new(report))
                }),
                Err(OperationError::Workspace(WorkspaceError::CommitUnknown)) => {
                    state.mark_unknown();
                    Ok(unknown(anchor, identity))
                }
                Err(error) => Err(error),
            }
        })
        .await;
        match result {
            Err(OperationError::Workspace(WorkspaceError::CommitUnknown)) => {
                self.state.mark_unknown();
                Ok(unknown(fallback, request_id))
            }
            other => other,
        }
    }
    /// Reads the exact original attempt without dispatch or safe-reissue rights.
    pub async fn delivery_attempt(
        &self,
        binding: WorkspaceBinding,
        operation: OperationId,
        digest: CommandDigest,
        recipient: PublicKey,
        request: DeliveryRequestId,
        proof: AttachmentProof,
    ) -> Result<Option<DeliveryAttempt>, OperationError> {
        if !codec::valid_request(request.as_str()) {
            return Err(OperationError::InvalidCommand);
        }
        let work = self.state.admit()?;
        let state = self.state.clone();
        owned(work, async move {
            let access = state.operation_access(&binding, proof).await?;
            let view = state.read_view(operation).await?;
            let t0 = state.operation_records(&view, operation).await?;
            let facts = state
                .delivery_facts(
                    &view,
                    operation,
                    Some(digest),
                    &t0,
                    access.binding.observed_at(),
                    Some(Selection {
                        recipient,
                        request: &request,
                        event: None,
                        retain_seal: false,
                    }),
                )
                .await?;
            if let Some(snapshot) = &facts.snapshot {
                radroots_storage::command::protected::same_binding(
                    snapshot.prepared().request().binding(),
                    &binding,
                )?;
            }
            state
                .check_operation(access, operation, OperationChange::observe(t0), view)
                .await?;
            Ok(facts.selected)
        })
        .await
    }
}
