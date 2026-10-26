//! Owned ordinary public/private command execution over explicit host IO and protected storage.
#[path = "inquiry_routes.rs"]
mod routes;
pub use routes::{capture_dm_route, selected_dm_targets};

use super::{HostBound, HostFuture, Shared, SignerSession};
use alloc::{string::String, vec::Vec};
use core::num::NonZeroU64;
use radroots_domain::{PublicKey, UnixSeconds};
use radroots_protocol::chat::{ChatRumor, ChatSeal, ChatSigningPlan};
use radroots_protocol::nip59::CapturedDmRoute;
use radroots_protocol::{
    intent::SigningReplay,
    listing::PublicSigningPlan,
    plan::{CapturedSigningPlan, SigningTarget},
};
use radroots_signing::{
    issuance::{OutcomeKind, SigningIntent},
    provider::{PrivateCiphertext, PrivateText, ProviderCall},
    request::CancellationSignal,
};
use radroots_storage::{
    command::{
        ArtifactEvidence, BoundSigningPlan, ClaimAttemptOutcome, ClaimAttemptRequest,
        DeliveryAttemptOutcome, DeliveryAttemptRequest, DeliveryIntent, DeliveryObservation,
        EnvelopeCandidate, EvidenceCaptureOutcome, InstallationRequest, OperationError,
        OperationJournal, PlanBindingOutcome, PreparedOperation, PublicationArtifact,
        PublicationWork,
    },
    workspace::{AttachmentProof, WorkspaceBinding},
};
use radroots_transport::{DeliveryReceipt, SinkFailure, TargetSet};
use std::sync::Mutex;

/// Target IO, not command authority. Construction/start/query must not perform
/// network work or replay an outbox. Implementations must enforce their explicit
/// destination policy and bound every actual request.
pub trait CommandIo: HostBound {
    /// Whether explicit online startup permits this workflow's network work.
    fn enabled(&self) -> bool;
    /// Current positive supported time; never renews a first-issue window.
    fn now(&self) -> Result<UnixSeconds, OperationError>;
    /// Explicit configured public publication targets. An unsupported host must
    /// refuse; public work never borrows a private participant's route.
    fn public_targets(&self) -> Result<TargetSet, OperationError> {
        Err(OperationError::UnsupportedOperation)
    }
    /// CSPRNG-backed NIP-59 past time within the supported two-day interval.
    fn seal_time(&self, now: UnixSeconds) -> Result<UnixSeconds, OperationError>;
    /// Authenticates a bounded current kind-10050 head and captures only its
    /// explicitly approved write endpoints, preserving original signed bytes.
    fn route(
        &self,
        recipient: PublicKey,
        deadline: NonZeroU64,
    ) -> HostFuture<'_, Result<CapturedDmRoute, OperationError>>;
    /// Resolves only the original captured route against current write policy.
    fn targets(&self, route: &CapturedDmRoute) -> Result<TargetSet, OperationError>;
    /// Explicit relay acceptance criterion, frozen by the first retained D.
    fn publication_policy(&self) -> radroots_transport::policy::SatisfactionPolicy;
    /// Wraps the original obtained seal with a fresh temporary envelope key.
    fn envelope(
        &self,
        work: PublicationWork,
        raw_seal: &str,
    ) -> Result<EnvelopeCandidate, OperationError>;
    /// Fresh full-width transport-attempt identity from qualified entropy.
    fn delivery_id(&self) -> Result<String, OperationError>;
    /// Dispatches only an actual freshly committed original delivery claim.
    fn publish(
        &self,
        request: radroots_transport::sink::DeliveryRequest,
        selected: TargetSet,
    ) -> HostFuture<'_, Result<DeliveryReceipt, SinkFailure>>;
}

struct PendingCapture {
    journal: Shared<dyn OperationJournal>,
    binding: WorkspaceBinding,
    proof: AttachmentProof,
    evidence: ArtifactEvidence,
    // Keeps both the resource permit and conservative live-byte reservation.
    _lease: Shared<dyn HostBound>,
}
struct PendingObservation {
    journal: Shared<dyn OperationJournal>,
    binding: WorkspaceBinding,
    proof: AttachmentProof,
    observation: DeliveryObservation,
    _lease: Shared<dyn HostBound>,
}

/// Shared workflow and obtained-material owner. Its lifecycle host must call
/// `drain` before releasing work permits, callbacks or protected storage.
pub struct CommandRuntime {
    pub(super) io: Shared<dyn CommandIo>,
    pending: Mutex<Vec<Shared<PendingCapture>>>,
    observations: Mutex<Vec<Shared<PendingObservation>>>,
    draining: futures::lock::Mutex<()>,
}
pub(super) struct SigningResources {
    pub(super) signer: Option<SignerSession>,
    pub(super) lease: Shared<dyn HostBound>,
    pub(super) stock_captures: Shared<super::stock_evidence::Owner>,
}
pub(super) enum RunMode {
    Command,
    Resume,
}
impl CommandRuntime {
    /// Owns explicit IO without performing effects or starting a runtime.
    pub fn new(io: Shared<dyn CommandIo>) -> Self {
        Self {
            io,
            pending: Mutex::new(Vec::new()),
            observations: Mutex::new(Vec::new()),
            draining: futures::lock::Mutex::new(()),
        }
    }
    /// Retries only already obtained original evidence. Failure retains its
    /// exact bytes and resource ownership, including when this wait is dropped.
    pub async fn drain(&self) -> Result<(), OperationError> {
        let _serial = self.draining.lock().await;
        loop {
            let next = self
                .pending
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .first()
                .cloned();
            let Some(next) = next else {
                break;
            };
            match next
                .journal
                .reconcile_unknown(next.binding.input().clone(), &next.proof, &next.evidence)
                .await
                .map_err(|_| OperationError::RecoveryRequired)?
            {
                EvidenceCaptureOutcome::Committed(_) => self.remove(&next),
                EvidenceCaptureOutcome::Unknown(_) => return Err(OperationError::RecoveryRequired),
            }
        }
        loop {
            let next = self
                .observations
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .first()
                .cloned();
            let Some(next) = next else {
                return Ok(());
            };
            self.capture_observation(&next)
                .await
                .map_err(|_| OperationError::RecoveryRequired)?;
        }
    }
    async fn capture_observation(
        &self,
        entry: &Shared<PendingObservation>,
    ) -> Result<(), OperationError> {
        match entry
            .journal
            .record_delivery_observation(
                entry.binding.clone(),
                copy(&entry.proof)?,
                entry.observation.clone(),
            )
            .await?
        {
            radroots_storage::command::DeliveryObservationOutcome::Committed(_) => {
                let removed = {
                    let mut pending = self
                        .observations
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    pending
                        .iter()
                        .position(|value| Shared::ptr_eq(value, entry))
                        .map(|index| pending.remove(index))
                };
                drop(removed);
                Ok(())
            }
            radroots_storage::command::DeliveryObservationOutcome::Unknown(_) => {
                Err(OperationError::RecoveryRequired)
            }
        }
    }
    fn remove(&self, entry: &Shared<PendingCapture>) {
        let removed = {
            let mut pending = self
                .pending
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            pending
                .iter()
                .position(|value| Shared::ptr_eq(value, entry))
                .map(|index| pending.remove(index))
        };
        drop(removed);
    }
    pub(super) async fn run(
        &self,
        journal: Shared<dyn OperationJournal>,
        prepared: PreparedOperation,
        proof: &AttachmentProof,
        resources: SigningResources,
        mode: RunMode,
    ) -> Result<(), OperationError> {
        if !self.io.enabled() {
            return Ok(());
        }
        self.drain().await?;
        resources.stock_captures.ensure_ready()?;
        let signer = &resources.signer;
        let binding = prepared.request().binding().clone();
        let operation = prepared.identity().operation_id();
        let digest = prepared.identity().command_digest();
        let original = journal
            .signing_plan(binding.clone(), operation, digest, copy(proof)?)
            .await?;
        let bound = match original {
            Some(bound) => bound,
            None => {
                let signer = signer.as_ref().ok_or(OperationError::Unauthorized)?;
                let plan = self.plan(&prepared, signer).await?;
                match journal
                    .bind_signing_intents(binding.clone(), plan, copy(proof)?)
                    .await?
                {
                    PlanBindingOutcome::Committed(bound) => bound,
                    PlanBindingOutcome::Unknown(_) => return Err(OperationError::RecoveryRequired),
                }
            }
        };
        self.issue(
            journal.clone(),
            &bound,
            proof,
            signer.as_ref(),
            resources.lease.clone(),
            &resources.stock_captures,
        )
        .await?;
        let snapshot = journal
            .operation_snapshot(binding.clone(), operation, Some(digest), copy(proof)?)
            .await?
            .ok_or(OperationError::RecoveryRequired)?;
        if snapshot
            .artifacts()
            .iter()
            .any(|recipient| recipient.obtained().is_some())
        {
            journal
                .install_transition_and_outbox(
                    InstallationRequest::new(
                        binding.clone(),
                        operation,
                        digest,
                        *bound.plan().digest(),
                    ),
                    copy(proof)?,
                )
                .await?;
        }
        self.deliver(journal, &bound, proof, resources.lease, mode)
            .await
    }
    async fn plan(
        &self,
        prepared: &PreparedOperation,
        signer: &SignerSession,
    ) -> Result<CapturedSigningPlan, OperationError> {
        let now = self.io.now()?;
        let deadline = NonZeroU64::new(
            prepared
                .issuance_window()
                .first_issue_not_after()
                .get()
                .checked_mul(1000)
                .ok_or(OperationError::RecoveryRequired)?,
        )
        .ok_or(OperationError::RecoveryRequired)?;
        if now >= prepared.issuance_window().first_issue_not_after() {
            return Err(OperationError::DeadlineExceeded);
        }
        let observed = signer
            .observe()
            .await
            .map_err(|_| OperationError::Unauthorized)?;
        if observed.identity() != prepared.request().command().context().actor().participant() {
            return Err(OperationError::Unauthorized);
        }
        let replay = match observed
            .exact_issuance()
            .ok_or(OperationError::Unauthorized)?
        {
            radroots_signing::recovery::ReplayCapability::NonReplayable => {
                SigningReplay::NonReplayable
            }
            radroots_signing::recovery::ReplayCapability::LocalReplaySafe => {
                SigningReplay::LocalReplaySafe
            }
            radroots_signing::recovery::ReplayCapability::ExactReplayByRequestId => {
                SigningReplay::ExactReplayByRequestId
            }
            _ => return Err(OperationError::Unauthorized),
        };
        if let Some(command) = prepared.request().command().listing() {
            let targets = self.io.public_targets()?;
            let urls = targets
                .targets()
                .iter()
                .map(|target| target.uri().as_str().into())
                .collect::<Vec<String>>();
            return PublicSigningPlan::capture(command, now, &urls, replay)
                .map(CapturedSigningPlan::Listing)
                .map_err(|_| OperationError::InvalidCommand);
        }
        let command = prepared.request().chat_command()?;
        if observed.encryption() != radroots_signing::provider::PrivateCryptoCapability::Nip44V2
            || observed.decryption() != radroots_signing::provider::PrivateCryptoCapability::Nip44V2
        {
            return Err(OperationError::Unauthorized);
        }
        let rumor =
            ChatRumor::from_command(command, now).map_err(|_| OperationError::InvalidCommand)?;
        let mut routes = Vec::new();
        // Resolve every original route before provider encryption or T1.
        for recipient in rumor.recipients() {
            routes.push(self.io.route(*recipient, deadline).await?);
        }
        let mut seals = Vec::new();
        for recipient in rumor.recipients() {
            let call = || ProviderCall::new(rumor.author(), deadline, CancellationSignal::new());
            let text =
                PrivateText::new(rumor.json()).map_err(|_| OperationError::InvalidCommand)?;
            let encrypted = signer
                .encrypt_private(call(), *recipient, text)
                .await
                .map_err(|_| OperationError::Unauthorized)?;
            let check = PrivateCiphertext::parse(encrypted.as_str())
                .map_err(|_| OperationError::InvalidEvidence)?;
            let decrypted = signer
                .decrypt_private(call(), *recipient, check)
                .await
                .map_err(|_| OperationError::InvalidEvidence)?;
            if decrypted.expose() != rumor.json() {
                return Err(OperationError::InvalidEvidence);
            }
            seals.push(
                ChatSeal::new(
                    &rumor,
                    *recipient,
                    self.io.seal_time(now)?,
                    encrypted.as_str(),
                )
                .map_err(|_| OperationError::InvalidCommand)?,
            );
        }
        ChatSigningPlan::new(command, now, seals, routes, replay)
            .map(CapturedSigningPlan::Chat)
            .map_err(|_| OperationError::InvalidCommand)
    }
    async fn issue(
        &self,
        journal: Shared<dyn OperationJournal>,
        bound: &BoundSigningPlan,
        proof: &AttachmentProof,
        signer: Option<&SignerSession>,
        lease: Shared<dyn HostBound>,
        stock_captures: &super::stock_evidence::Owner,
    ) -> Result<(), OperationError> {
        let prepared = bound.prepared();
        let binding = prepared.request().binding();
        let targets = match bound.plan() {
            CapturedSigningPlan::Listing(_) => alloc::vec![SigningTarget::PublicEvent],
            CapturedSigningPlan::Chat(plan) => plan
                .seals()
                .iter()
                .map(|seal| SigningTarget::BusinessSeal(seal.recipient()))
                .collect(),
        };
        for target in targets {
            let select = || {
                ClaimAttemptRequest::for_artifact(
                    binding.clone(),
                    prepared.identity().operation_id(),
                    prepared.identity().command_digest(),
                    *bound.plan().digest(),
                    target,
                )
            };
            if journal
                .signing_attempt(select(), copy(proof)?)
                .await?
                .is_some()
            {
                continue;
            }
            let Some(signer) = signer else {
                continue;
            };
            if signer.state() != super::SessionState::Running
                || !self
                    .io
                    .now()
                    .is_ok_and(|now| now < prepared.issuance_window().first_issue_not_after())
            {
                continue;
            }
            stock_captures.ensure_ready()?;
            let attempt = match journal.claim_attempt(select(), copy(proof)?).await {
                Err(OperationError::DeadlineExceeded) => continue,
                Err(error) => return Err(error),
                Ok(outcome) => match outcome {
                    ClaimAttemptOutcome::Claimed(attempt) => attempt,
                    ClaimAttemptOutcome::Retained(_) | ClaimAttemptOutcome::Unknown(_) => continue,
                },
            };
            let intent = SigningIntent::from_plan(
                attempt.bound_plan().plan(),
                attempt.target(),
                ProviderCall::new(
                    attempt
                        .bound_plan()
                        .plan()
                        .artifact(attempt.target())
                        .map_err(|_| OperationError::RecoveryRequired)?
                        .fields()
                        .author,
                    attempt.deadline_unix_ms(),
                    CancellationSignal::new(),
                ),
            )
            .map_err(|_| OperationError::InvalidCommand)?;
            if intent.binding() != attempt.binding() {
                return Err(OperationError::RecoveryRequired);
            }
            stock_captures.ensure_ready()?;
            let outcome = signer.sign_exact(intent).await;
            let (_, kind) = outcome.into_parts();
            if let OutcomeKind::Obtained(artifact) = kind {
                let evidence = artifact.with_json(|raw| {
                    ArtifactEvidence::verify(&attempt, raw, artifact.observed_at_unix_ms())
                })?;
                let entry = Shared::new(PendingCapture {
                    journal: journal.clone(),
                    binding: binding.clone(),
                    proof: copy(proof)?,
                    evidence,
                    _lease: lease.clone(),
                });
                self.pending
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .push(entry.clone());
                match journal
                    .capture_evidence(binding.input().clone(), &entry.proof, &entry.evidence)
                    .await
                    .map_err(|_| OperationError::RecoveryRequired)?
                {
                    EvidenceCaptureOutcome::Committed(_) => self.remove(&entry),
                    EvidenceCaptureOutcome::Unknown(_) => {
                        return Err(OperationError::RecoveryRequired);
                    }
                }
            }
        }
        Ok(())
    }
    async fn deliver(
        &self,
        journal: Shared<dyn OperationJournal>,
        bound: &BoundSigningPlan,
        proof: &AttachmentProof,
        lease: Shared<dyn HostBound>,
        mode: RunMode,
    ) -> Result<(), OperationError> {
        let prepared = bound.prepared();
        let binding = prepared.request().binding();
        let snapshot = journal
            .operation_snapshot(
                binding.clone(),
                prepared.identity().operation_id(),
                Some(prepared.identity().command_digest()),
                copy(proof)?,
            )
            .await?
            .ok_or(OperationError::RecoveryRequired)?;
        let work = snapshot
            .publications()
            .iter()
            .copied()
            .map(PublicationArtifact::Private)
            .chain(
                snapshot
                    .public_publications()
                    .iter()
                    .copied()
                    .map(PublicationArtifact::Public),
            );
        for work in work {
            if snapshot.publication_accepted(work)? {
                continue;
            }
            // Replaying a command never dispatches a previously claimed attempt.
            let original = snapshot
                .delivery_attempts()
                .iter()
                .find(|attempt| attempt.work() == work);
            if original.is_some() && matches!(mode, RunMode::Command) {
                continue;
            }
            let (targets, envelope, event) = match work {
                PublicationArtifact::Private(private) => {
                    let recipient = private.recipient();
                    let route = bound
                        .chat_plan()?
                        .routes()
                        .iter()
                        .find(|route| route.participant() == recipient)
                        .ok_or(OperationError::RecoveryRequired)?;
                    let targets = self.io.targets(route)?;
                    let captured = journal
                        .obtained_evidence(
                            ClaimAttemptRequest::new(
                                binding.clone(),
                                prepared.identity().operation_id(),
                                prepared.identity().command_digest(),
                                *bound.plan().digest(),
                                recipient,
                            ),
                            copy(proof)?,
                        )
                        .await?
                        .ok_or(OperationError::RecoveryRequired)?;
                    let envelope = if original.is_none() {
                        Some(self.io.envelope(private, captured.evidence().raw_json())?)
                    } else {
                        None
                    };
                    let event = match original {
                        Some(v) => v.intent().event_id(),
                        None => envelope
                            .as_ref()
                            .ok_or(OperationError::RecoveryRequired)?
                            .event_id(),
                    };
                    (targets, envelope, event)
                }
                PublicationArtifact::Public(public) => {
                    let configured = self.io.public_targets()?;
                    let plan = bound
                        .plan()
                        .listing()
                        .ok_or(OperationError::RecoveryRequired)?;
                    let targets = TargetSet::new(
                        plan.selected_targets()
                            .iter()
                            .map(|uri| radroots_transport::Target::nostr_relay(uri.as_ref()))
                            .collect::<Result<Vec<_>, _>>()
                            .map_err(|_| OperationError::RecoveryRequired)?,
                    )
                    .map_err(|_| OperationError::RecoveryRequired)?;
                    if targets.targets().iter().any(|target| {
                        !configured
                            .targets()
                            .iter()
                            .any(|allowed| allowed.fingerprint() == target.fingerprint())
                    }) {
                        return Err(OperationError::UnverifiedApproval);
                    }
                    (targets, None, public.evidence().event_id())
                }
            };
            let deadline = NonZeroU64::new(
                self.io
                    .now()?
                    .get()
                    .checked_add(30)
                    .and_then(|time| time.checked_mul(1000))
                    .ok_or(OperationError::RecoveryRequired)?,
            )
            .ok_or(OperationError::RecoveryRequired)?;
            let (event, targets, policy) = match original {
                Some(original) => (
                    original.intent().event_id(),
                    original.intent().targets().clone(),
                    original.intent().policy().clone(),
                ),
                None => (event, targets, self.io.publication_policy()),
            };
            let selected = targets
                .targets()
                .iter()
                .filter(|target| {
                    !snapshot.delivery_observations().iter().any(|observation| {
                        observation.work() == work
                            && observation.target_receipt().target().fingerprint()
                                == target.fingerprint()
                            && observation.kind()
                                == radroots_storage::command::DeliveryObservationKind::Accepted
                    })
                })
                .cloned()
                .collect::<Vec<_>>();
            if selected.is_empty() {
                continue;
            }
            let selected =
                TargetSet::new(selected).map_err(|_| OperationError::RecoveryRequired)?;
            let request_id = self.io.delivery_id()?;
            let intent = match work {
                PublicationArtifact::Private(_) => {
                    DeliveryIntent::new(&request_id, event, &targets, &policy, deadline)
                }
                PublicationArtifact::Public(_) => {
                    DeliveryIntent::public(&request_id, event, &targets, &policy, deadline)
                }
            }?
            .with_selection(&selected)?;
            let request = DeliveryAttemptRequest::new(binding.clone(), work, intent)?;
            let attempt = match journal
                .prepare_delivery_attempt(request, copy(proof)?, envelope.as_ref())
                .await?
            {
                DeliveryAttemptOutcome::Claimed(attempt) => attempt,
                DeliveryAttemptOutcome::Retained(_) | DeliveryAttemptOutcome::Unknown { .. } => {
                    continue;
                }
            };
            let result = self
                .io
                .publish(
                    attempt.transport_request()?,
                    attempt.intent().selected_target_set()?,
                )
                .await;
            let observed = self
                .io
                .now()
                .ok()
                .and_then(|time| time.get().checked_mul(1000))
                .and_then(NonZeroU64::new);
            let reports = match &result {
                Ok(receipt) => receipt.target_receipts(),
                Err(failure) => failure.partial_evidence(),
            };
            let mut entries = Vec::new();
            for report in reports {
                let observation = match &result {
                    Ok(receipt) => DeliveryObservation::from_receipt(
                        &attempt,
                        receipt,
                        report.target().fingerprint(),
                        observed,
                    ),
                    Err(failure) => DeliveryObservation::from_failure(
                        &attempt,
                        failure,
                        report.target().fingerprint(),
                        observed,
                    ),
                }?;
                entries.push(Shared::new(PendingObservation {
                    journal: journal.clone(),
                    binding: binding.clone(),
                    proof: copy(proof)?,
                    observation,
                    _lease: lease.clone(),
                }));
            }
            self.observations
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .extend(entries.iter().cloned());
            for entry in entries {
                self.capture_observation(&entry).await?;
            }
        }
        Ok(())
    }
}
fn copy(proof: &AttachmentProof) -> Result<AttachmentProof, OperationError> {
    proof
        .with_secret(|secret| AttachmentProof::from_protected_input(secret))
        .map_err(Into::into)
}
