//! Exact attempt and independently verified obtained-seal data. Constructors
//! provide no proof that a backend retained a claim or authorized dispatch.
use super::{BoundSigningPlan, CommitAttempt, OperationError};
use crate::workspace::WorkspaceBinding;
use core::{fmt, num::NonZeroU64};
use radroots_domain::{
    CommandDigest, EnvelopeEventId, OperationId, PublicKey, SigningAttemptId, TransactionId,
    UnixSeconds, WorkspaceId,
};
use radroots_event::{
    GenericEventDraft, SignedEvent,
    wire::{EventWireLimits, Nip01EventWire},
};
use radroots_event_codec::{
    authoring::AuthoredEventPlan,
    verify::{self, Nip01SignatureVerifier, RawEvent},
};
use radroots_protocol::{
    chat::ChatSeal,
    intent::{ArtifactRole, IntentBinding},
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
    recipient: PublicKey,
}
impl ClaimAttemptRequest {
    pub const fn new(
        binding: WorkspaceBinding,
        operation_id: OperationId,
        command_digest: CommandDigest,
        plan_digest: [u8; 32],
        recipient: PublicKey,
    ) -> Self {
        Self {
            binding,
            operation_id,
            command_digest,
            plan_digest,
            recipient,
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
    pub const fn recipient(&self) -> PublicKey {
        self.recipient
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
    recipient_index: usize,
    attempt_id: SigningAttemptId,
    transaction_id: TransactionId,
    claimed_at: UnixSeconds,
    binding: IntentBinding,
    deadline: NonZeroU64,
}
impl SigningAttempt {
    pub fn from_committed_store(
        bound: BoundSigningPlan,
        recipient: PublicKey,
        attempt_id: SigningAttemptId,
        transaction_id: TransactionId,
        claimed_at: UnixSeconds,
    ) -> Result<Self, OperationError> {
        let window = bound.prepared().issuance_window();
        if claimed_at < bound.bound_at() || claimed_at >= window.first_issue_not_after() {
            return Err(OperationError::RecoveryRequired);
        }
        let recipient_index = bound
            .chat_plan()?
            .seals()
            .iter()
            .position(|seal| seal.recipient() == recipient)
            .ok_or(OperationError::InvalidCommand)?;
        let seal = &bound.chat_plan()?.seals()[recipient_index];
        let deadline = NonZeroU64::new(
            window
                .first_issue_not_after()
                .get()
                .checked_mul(1000)
                .ok_or(OperationError::RecoveryRequired)?,
        )
        .ok_or(OperationError::RecoveryRequired)?;
        let fields = seal.fields();
        // Internal reuse of the qualified v1 authored-plan hash. No predecessor
        // event model escapes this new API; downward parser retirement remains
        // separately governed by the shared target architecture.
        let draft = GenericEventDraft::new(
            "radroots.message.seal.v1",
            fields.kind,
            fields.created_at,
            Vec::new(),
            fields.content,
            fields.author.to_hex(),
        )
        .map_err(|_| OperationError::InvalidCommand)?;
        let plan =
            AuthoredEventPlan::from_generic(draft).map_err(|_| OperationError::InvalidCommand)?;
        if plan.expected_event_id().as_bytes() != &seal.event_id() {
            return Err(OperationError::RecoveryRequired);
        }
        let binding = IntentBinding::from_components(
            seal.operation_id(),
            *seal.command_digest().as_bytes(),
            *plan.digest().as_bytes(),
            ArtifactRole::BusinessSeal,
            Some(recipient),
            deadline,
        )
        .map_err(|_| OperationError::InvalidCommand)?;
        Ok(Self {
            bound,
            recipient_index,
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
    /// Borrows the exact retained seal. The private index is validated once
    /// against the immutable complete plan by the structural constructor.
    pub fn seal(&self) -> Result<&ChatSeal, OperationError> {
        self.bound
            .chat_plan()?
            .seals()
            .get(self.recipient_index)
            .ok_or(OperationError::RecoveryRequired)
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
pub struct SealEvidence {
    workspace_id: WorkspaceId,
    operation_id: OperationId,
    command_digest: CommandDigest,
    plan_digest: [u8; 32],
    recipient: PublicKey,
    attempt_id: SigningAttemptId,
    intent: IntentBinding,
    event_id: EnvelopeEventId,
    raw: Zeroizing<String>,
    digest: [u8; 32],
    observed_at: Option<NonZeroU64>,
}
impl SealEvidence {
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
        let limits = EventWireLimits {
            max_raw_json_bytes: 65536,
            max_extra_fields: 0,
            max_total_extra_json_bytes: 0,
            ..EventWireLimits::default()
        };
        let wire = Nip01EventWire::parse_json_exact_with_limits(raw, limits)
            .map_err(|_| OperationError::InvalidEvidence)?;
        let seal = attempt.seal()?;
        let fields = seal.fields();
        let event_id = EnvelopeEventId::from_bytes(seal.event_id());
        if wire.id != event_id.to_string()
            || wire.pubkey != fields.author.to_hex()
            || wire.kind != fields.kind
            || wire.created_at != fields.created_at
            || wire.tags != fields.tags
            || wire.content != fields.content
        {
            return Err(OperationError::InvalidEvidence);
        }
        let event = SignedEvent::from_wire_verified_id(wire, raw)
            .map_err(|_| OperationError::InvalidEvidence)?;
        let id = verify::id(RawEvent::new(event.envelope().clone()))
            .map_err(|_| OperationError::InvalidEvidence)?;
        verify::signature(id, &Nip01SignatureVerifier)
            .map_err(|_| OperationError::InvalidEvidence)?;
        let original = attempt.bound_plan().prepared().identity();
        Ok(Self {
            workspace_id: original.workspace_id(),
            operation_id: original.operation_id(),
            command_digest: original.command_digest(),
            plan_digest: *attempt.bound_plan().plan().digest(),
            recipient: seal.recipient(),
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
            recipient: self.recipient,
            attempt_id: self.attempt_id,
            intent: self.intent,
            event_id: self.event_id,
            raw: Zeroizing::new(self.raw.to_string()),
            digest: self.digest,
            observed_at: self.observed_at,
        }
    }
    pub fn matches_attempt(&self, attempt: &SigningAttempt) -> bool {
        let Ok(seal) = attempt.seal() else {
            return false;
        };
        self.workspace_id == attempt.bound_plan().prepared().identity().workspace_id()
            && self.attempt_id == attempt.attempt_id()
            && self.intent == attempt.binding()
            && self.operation_id == attempt.bound_plan().prepared().identity().operation_id()
            && self.command_digest == attempt.bound_plan().prepared().identity().command_digest()
            && &self.plan_digest == attempt.bound_plan().plan().digest()
            && self.recipient == seal.recipient()
            && self.event_id.as_bytes() == &seal.event_id()
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
    pub const fn recipient(&self) -> PublicKey {
        self.recipient
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
impl fmt::Debug for SealEvidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SealEvidence(<redacted>)")
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
    recipient: PublicKey,
    event_id: EnvelopeEventId,
    digest: [u8; 32],
    transaction_id: TransactionId,
    observed_at: Option<NonZeroU64>,
}
impl EvidenceReceipt {
    pub const fn from_committed_store(
        evidence: &SealEvidence,
        transaction_id: TransactionId,
    ) -> Self {
        Self {
            workspace_id: evidence.workspace_id,
            operation_id: evidence.operation_id,
            command_digest: evidence.command_digest,
            attempt_id: evidence.attempt_id,
            recipient: evidence.recipient,
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
    pub const fn recipient(&self) -> PublicKey {
        self.recipient
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
pub struct CapturedSeal {
    attempt: SigningAttempt,
    evidence: SealEvidence,
    receipt: EvidenceReceipt,
}
impl CapturedSeal {
    pub fn from_committed_store(
        attempt: SigningAttempt,
        evidence: SealEvidence,
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
    pub const fn evidence(&self) -> &SealEvidence {
        &self.evidence
    }
    pub const fn receipt(&self) -> EvidenceReceipt {
        self.receipt
    }
}
impl fmt::Debug for CapturedSeal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CapturedSeal(<redacted>)")
    }
}
