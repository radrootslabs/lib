//! Exact attempt and independently verified obtained-artifact data. Constructors
//! provide no proof that a backend retained a claim or authorized dispatch.
use super::{BoundSigningPlan, CommitAttempt, OperationError};
use crate::workspace::WorkspaceBinding;
use core::{fmt, num::NonZeroU64};
use radroots_domain::{
    CommandDigest, EnvelopeEventId, OperationId, PublicKey, SigningAttemptId, TransactionId,
    UnixSeconds, WorkspaceId,
};
use radroots_event::{
    GenericEventDraft,
    wire::{EventWireLimits, Nip01EventWire},
};
use radroots_event_codec::{
    authoring::AuthoredEventPlan,
    verify::{self, Nip01SignatureVerifier, RawEvent},
};
use radroots_protocol::{
    chat::ChatSeal,
    intent::IntentBinding,
    plan::{SigningArtifact, SigningTarget},
};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

/// Frozen selectors for claiming one retained recipient intent. No constructor
/// authenticates a principal, approves a route or permits a signer invocation.
pub struct ClaimAttemptRequest {
    binding: WorkspaceBinding,
    operation_id: OperationId,
    command_digest: CommandDigest,
    plan_digest: [u8; 32],
    target: SigningTarget,
}
impl ClaimAttemptRequest {
    pub const fn new(
        binding: WorkspaceBinding,
        operation_id: OperationId,
        command_digest: CommandDigest,
        plan_digest: [u8; 32],
        recipient: PublicKey,
    ) -> Self {
        Self::for_artifact(
            binding,
            operation_id,
            command_digest,
            plan_digest,
            SigningTarget::BusinessSeal(recipient),
        )
    }
    pub const fn for_artifact(
        binding: WorkspaceBinding,
        operation_id: OperationId,
        command_digest: CommandDigest,
        plan_digest: [u8; 32],
        target: SigningTarget,
    ) -> Self {
        Self {
            binding,
            operation_id,
            command_digest,
            plan_digest,
            target,
        }
    }
    pub const fn binding(&self) -> &WorkspaceBinding {
        &self.binding
    }
    pub const fn operation_id(&self) -> OperationId {
        self.operation_id
    }
    pub const fn command_digest(&self) -> CommandDigest {
        self.command_digest
    }
    pub const fn plan_digest(&self) -> &[u8; 32] {
        &self.plan_digest
    }
    pub const fn target(&self) -> SigningTarget {
        self.target
    }
    /// Private-only projection; a public artifact has no recipient.
    pub fn recipient(&self) -> Result<PublicKey, OperationError> {
        self.target
            .recipient()
            .ok_or(OperationError::UnsupportedOperation)
    }
    pub fn identity(&self) -> CommitAttempt {
        CommitAttempt::new(
            self.binding.input().workspace_id(),
            self.operation_id,
            self.command_digest,
            None,
        )
    }
}
impl fmt::Debug for ClaimAttemptRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ClaimAttemptRequest(<redacted>)")
    }
}

/// Actual store report of an immutable first claim. Construction remains
/// structural. Only an actual newly committed claim may be dispatched once;
/// retrieving this snapshot never proves a previous invocation did not begin.
pub struct SigningAttempt {
    bound: BoundSigningPlan,
    target: SigningTarget,
    attempt_id: SigningAttemptId,
    transaction_id: TransactionId,
    claimed_at: UnixSeconds,
    binding: IntentBinding,
    deadline: NonZeroU64,
}
impl SigningAttempt {
    pub fn from_committed_store(
        bound: BoundSigningPlan,
        target: impl Into<SigningTarget>,
        attempt_id: SigningAttemptId,
        transaction_id: TransactionId,
        claimed_at: UnixSeconds,
    ) -> Result<Self, OperationError> {
        let window = bound.prepared().issuance_window();
        if claimed_at < bound.bound_at() || claimed_at >= window.first_issue_not_after() {
            return Err(OperationError::RecoveryRequired);
        }
        let target = target.into();
        if matches!(target, SigningTarget::PublicEvent) && bound.plan().listing().is_none()
            || matches!(target, SigningTarget::BusinessSeal(_)) && bound.plan().chat().is_none()
        {
            return Err(OperationError::UnsupportedOperation);
        }
        let artifact = bound
            .plan()
            .artifact(target)
            .map_err(|_| OperationError::InvalidCommand)?;
        let deadline = NonZeroU64::new(
            window
                .first_issue_not_after()
                .get()
                .checked_mul(1000)
                .ok_or(OperationError::RecoveryRequired)?,
        )
        .ok_or(OperationError::RecoveryRequired)?;
        let preimage_digest = match target {
            SigningTarget::PublicEvent => *bound.plan().digest(),
            SigningTarget::BusinessSeal(_) => {
                let fields = artifact.fields();
                // Internal reuse of the qualified v1 authored-plan hash. No predecessor
                // event model escapes this new API; downward parser retirement remains
                // separately governed by the shared target architecture.
                let draft = GenericEventDraft::new(
                    "radroots.message.seal.v1",
                    fields.kind,
                    fields.created_at,
                    fields.tags.to_vec(),
                    fields.content,
                    fields.author.to_hex(),
                )
                .map_err(|_| OperationError::InvalidCommand)?;
                let plan = AuthoredEventPlan::from_generic(draft)
                    .map_err(|_| OperationError::InvalidCommand)?;
                if plan.expected_event_id().as_bytes() != &artifact.event_id() {
                    return Err(OperationError::RecoveryRequired);
                }
                *plan.digest().as_bytes()
            }
        };
        let binding = IntentBinding::from_components(
            bound.plan().operation_id(),
            *bound.plan().command_digest().as_bytes(),
            preimage_digest,
            target.role(),
            target.recipient(),
            deadline,
        )
        .map_err(|_| OperationError::InvalidCommand)?;
        Ok(Self {
            bound,
            target,
            attempt_id,
            transaction_id,
            claimed_at,
            binding,
            deadline,
        })
    }
    /// Transfers the immutable retained plan without creating another claim.
    pub fn into_bound_plan(self) -> BoundSigningPlan {
        self.bound
    }
    pub const fn bound_plan(&self) -> &BoundSigningPlan {
        &self.bound
    }
    /// Exact retained artifact, with no private projection for public events.
    pub fn artifact(&self) -> Result<SigningArtifact<'_>, OperationError> {
        self.bound
            .plan()
            .artifact(self.target)
            .map_err(|_| OperationError::RecoveryRequired)
    }
    pub const fn target(&self) -> SigningTarget {
        self.target
    }
    /// Borrows a private seal only when this attempt actually selected one.
    pub fn seal(&self) -> Result<&ChatSeal, OperationError> {
        match self.artifact()? {
            SigningArtifact::Seal(seal) => Ok(seal),
            _ => Err(OperationError::UnsupportedOperation),
        }
    }
    pub const fn attempt_id(&self) -> SigningAttemptId {
        self.attempt_id
    }
    pub const fn transaction_id(&self) -> TransactionId {
        self.transaction_id
    }
    pub const fn claimed_at(&self) -> UnixSeconds {
        self.claimed_at
    }
    pub const fn binding(&self) -> IntentBinding {
        self.binding
    }
    pub const fn deadline_unix_ms(&self) -> NonZeroU64 {
        self.deadline
    }
    pub fn identity(&self) -> CommitAttempt {
        let original = self.bound.prepared().identity();
        CommitAttempt::new(
            original.workspace_id(),
            original.operation_id(),
            original.command_digest(),
            Some(self.transaction_id),
        )
    }
}
impl fmt::Debug for SigningAttempt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SigningAttempt(<redacted>)")
    }
}

/// Confirmed new claim, retained potential issuance, or anchored uncertainty.
/// A data variant alone cannot qualify a custom adapter's commit or dispatch.
pub enum ClaimAttemptOutcome {
    /// Only this actual newly committed store result permits its owning
    /// executor to proceed once after route/provider/current-access checks.
    Claimed(SigningAttempt),
    /// Never dispatch again merely because this original record was found.
    Retained(SigningAttempt),
    /// Reconcile the original identity; never infer rollback/nonissuance.
    Unknown(CommitAttempt),
}
impl fmt::Debug for ClaimAttemptOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Claimed(_) => "ClaimAttemptOutcome::Claimed(<redacted>)",
            Self::Retained(_) => "ClaimAttemptOutcome::Retained(<redacted>)",
            Self::Unknown(_) => "ClaimAttemptOutcome::Unknown(<retained>)",
        })
    }
}

/// Independently authenticated exact obtained bytes, bound to reported claim
/// data but not proof of the backend's actual retained attempt. Concrete capture
/// must compare its own protected claim and plan before appending this evidence.
pub struct ArtifactEvidence {
    workspace_id: WorkspaceId,
    operation_id: OperationId,
    command_digest: CommandDigest,
    plan_digest: [u8; 32],
    target: SigningTarget,
    attempt_id: SigningAttemptId,
    intent: IntentBinding,
    event_id: EnvelopeEventId,
    raw: Zeroizing<String>,
    digest: [u8; 32],
    observed_at: Option<NonZeroU64>,
}
impl ArtifactEvidence {
    /// Bounds before copying and verifies ID/signature/every signed field.
    /// No clock, grant-expiry or cancellation lookup may discard obtained bytes.
    pub fn verify(
        attempt: &SigningAttempt,
        raw: &str,
        observed_at: Option<NonZeroU64>,
    ) -> Result<Self, OperationError> {
        if raw.len() > 65536 {
            return Err(OperationError::InvalidEvidence);
        }
        let limits = if attempt.target() == SigningTarget::PublicEvent {
            EventWireLimits {
                max_raw_json_bytes: 65536,
                max_content_bytes: 65536,
                max_tag_count: radroots_protocol::listing::LISTING_TAG_MAX_COUNT,
                max_total_tag_elements:
                    radroots_protocol::listing::LISTING_TAG_TOTAL_ELEMENT_MAX_COUNT,
                max_tag_element_bytes: 65536,
                max_total_tag_bytes: 65536,
                max_extra_fields: 0,
                max_total_extra_json_bytes: 0,
            }
        } else {
            EventWireLimits {
                max_raw_json_bytes: 65536,
                max_extra_fields: 0,
                max_total_extra_json_bytes: 0,
                ..EventWireLimits::default()
            }
        };
        let wire = Nip01EventWire::parse_json_exact_with_limits(raw, limits)
            .map_err(|_| OperationError::InvalidEvidence)?;
        let artifact = attempt.artifact()?;
        let fields = artifact.fields();
        let event_id = EnvelopeEventId::from_bytes(artifact.event_id());
        if wire.id != event_id.to_string()
            || wire.pubkey != fields.author.to_hex()
            || wire.kind != fields.kind
            || wire.created_at != fields.created_at
            || wire.tags != fields.tags
            || wire.content != fields.content
        {
            return Err(OperationError::InvalidEvidence);
        }
        let envelope =
            checked_envelope(wire, limits).map_err(|_| OperationError::InvalidEvidence)?;
        let id =
            verify::id(RawEvent::new(envelope)).map_err(|_| OperationError::InvalidEvidence)?;
        verify::signature(id, &Nip01SignatureVerifier)
            .map_err(|_| OperationError::InvalidEvidence)?;
        let original = attempt.bound_plan().prepared().identity();
        Ok(Self {
            workspace_id: original.workspace_id(),
            operation_id: original.operation_id(),
            command_digest: original.command_digest(),
            plan_digest: *attempt.bound_plan().plan().digest(),
            target: attempt.target(),
            attempt_id: attempt.attempt_id(),
            intent: attempt.binding(),
            event_id,
            raw: Zeroizing::new(String::from(raw)),
            digest: Sha256::digest(raw.as_bytes()).into(),
            observed_at: observed_at.filter(|time| time.get() <= 253_402_300_799_999),
        })
    }
    /// Explicitly copies a bounded already-verified value for the store owner;
    /// the caller keeps its sole original through actual durable completion.
    pub fn retained_copy(&self) -> Self {
        Self {
            workspace_id: self.workspace_id,
            operation_id: self.operation_id,
            command_digest: self.command_digest,
            plan_digest: self.plan_digest,
            target: self.target,
            attempt_id: self.attempt_id,
            intent: self.intent,
            event_id: self.event_id,
            raw: Zeroizing::new(self.raw.to_string()),
            digest: self.digest,
            observed_at: self.observed_at,
        }
    }
    pub fn matches_attempt(&self, attempt: &SigningAttempt) -> bool {
        let Ok(artifact) = attempt.artifact() else {
            return false;
        };
        self.workspace_id == attempt.bound_plan().prepared().identity().workspace_id()
            && self.attempt_id == attempt.attempt_id()
            && self.intent == attempt.binding()
            && self.operation_id == attempt.bound_plan().prepared().identity().operation_id()
            && self.command_digest == attempt.bound_plan().prepared().identity().command_digest()
            && &self.plan_digest == attempt.bound_plan().plan().digest()
            && self.target == attempt.target()
            && self.event_id.as_bytes() == &artifact.event_id()
    }
    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }
    pub const fn operation_id(&self) -> OperationId {
        self.operation_id
    }
    pub const fn command_digest(&self) -> CommandDigest {
        self.command_digest
    }
    pub const fn plan_digest(&self) -> &[u8; 32] {
        &self.plan_digest
    }
    pub const fn target(&self) -> SigningTarget {
        self.target
    }
    /// Private-only projection; a public artifact has no recipient.
    pub fn recipient(&self) -> Result<PublicKey, OperationError> {
        self.target
            .recipient()
            .ok_or(OperationError::UnsupportedOperation)
    }
    pub const fn attempt_id(&self) -> SigningAttemptId {
        self.attempt_id
    }
    pub const fn intent(&self) -> IntentBinding {
        self.intent
    }
    pub const fn event_id(&self) -> EnvelopeEventId {
        self.event_id
    }
    pub fn raw_json(&self) -> &str {
        &self.raw
    }
    pub const fn digest(&self) -> &[u8; 32] {
        &self.digest
    }
    pub const fn observed_at_unix_ms(&self) -> Option<NonZeroU64> {
        self.observed_at
    }
}
impl fmt::Debug for ArtifactEvidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ArtifactEvidence(<redacted>)")
    }
}

/// Fixed reference to exact retained obtained bytes. Data construction alone
/// supplies no proof of capture, business installation or delivery.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EvidenceReceipt {
    workspace_id: WorkspaceId,
    operation_id: OperationId,
    command_digest: CommandDigest,
    attempt_id: SigningAttemptId,
    target: SigningTarget,
    event_id: EnvelopeEventId,
    digest: [u8; 32],
    transaction_id: TransactionId,
    observed_at: Option<NonZeroU64>,
}
impl EvidenceReceipt {
    pub const fn from_committed_store(
        evidence: &ArtifactEvidence,
        transaction_id: TransactionId,
    ) -> Self {
        Self {
            workspace_id: evidence.workspace_id,
            operation_id: evidence.operation_id,
            command_digest: evidence.command_digest,
            attempt_id: evidence.attempt_id,
            target: evidence.target,
            event_id: evidence.event_id,
            digest: evidence.digest,
            transaction_id,
            observed_at: evidence.observed_at,
        }
    }
    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }
    pub const fn operation_id(&self) -> OperationId {
        self.operation_id
    }
    pub const fn command_digest(&self) -> CommandDigest {
        self.command_digest
    }
    pub const fn attempt_id(&self) -> SigningAttemptId {
        self.attempt_id
    }
    pub const fn target(&self) -> SigningTarget {
        self.target
    }
    /// Private-only projection; a public artifact has no recipient.
    pub fn recipient(&self) -> Result<PublicKey, OperationError> {
        self.target
            .recipient()
            .ok_or(OperationError::UnsupportedOperation)
    }
    pub const fn event_id(&self) -> EnvelopeEventId {
        self.event_id
    }
    pub const fn digest(&self) -> &[u8; 32] {
        &self.digest
    }
    pub const fn transaction_id(&self) -> TransactionId {
        self.transaction_id
    }
    pub const fn observed_at_unix_ms(&self) -> Option<NonZeroU64> {
        self.observed_at
    }
}
/// Actual capture completion, separate from installing or sending a record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EvidenceCaptureOutcome {
    Committed(EvidenceReceipt),
    Unknown(CommitAttempt),
}
/// An original protected obtained record read under current workspace access.
/// Advanced adapter construction remains a structural report, not qualification.
pub struct CapturedArtifact {
    attempt: SigningAttempt,
    evidence: ArtifactEvidence,
    receipt: EvidenceReceipt,
}
impl CapturedArtifact {
    pub fn from_committed_store(
        attempt: SigningAttempt,
        evidence: ArtifactEvidence,
        transaction_id: TransactionId,
    ) -> Result<Self, OperationError> {
        if !evidence.matches_attempt(&attempt) {
            return Err(OperationError::RecoveryRequired);
        }
        let receipt = EvidenceReceipt::from_committed_store(&evidence, transaction_id);
        Ok(Self {
            attempt,
            evidence,
            receipt,
        })
    }
    pub const fn attempt(&self) -> &SigningAttempt {
        &self.attempt
    }
    pub const fn evidence(&self) -> &ArtifactEvidence {
        &self.evidence
    }
    pub const fn receipt(&self) -> EvidenceReceipt {
        self.receipt
    }
}
impl fmt::Debug for CapturedArtifact {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CapturedArtifact(<redacted>)")
    }
}

fn checked_envelope(
    wire: Nip01EventWire,
    limits: EventWireLimits,
) -> Result<radroots_event::envelope::EventEnvelope, radroots_event::envelope::EventEnvelopeError> {
    use radroots_event::envelope::{EventEnvelope, EventEnvelopeLimits, EventEnvelopeParts};
    EventEnvelope::new_with_limits(
        EventEnvelopeParts {
            id: wire.id,
            author: wire.pubkey,
            created_at: wire.created_at,
            kind: wire.kind,
            tags: wire.tags,
            content: wire.content,
            sig: wire.sig,
        },
        EventEnvelopeLimits {
            max_content_bytes: limits.max_content_bytes,
            max_tag_count: limits.max_tag_count,
            max_total_tag_elements: limits.max_total_tag_elements,
            max_tag_element_bytes: limits.max_tag_element_bytes,
            max_total_tag_bytes: limits.max_total_tag_bytes,
        },
    )
}
