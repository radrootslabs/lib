//! Exact-intent signature facts with nullable observation and no durable claim.

use crate::provider::{ProviderCall, ProviderError};
#[cfg(not(feature = "std"))]
use alloc::string::String;
use core::{fmt, num::NonZeroU64};
use radroots_domain::OperationId;
use radroots_event::{
    SignedEvent,
    id::EventId,
    wire::{EventWireLimits, Nip01EventWire},
};
use radroots_event_codec::{
    authoring::AuthoredEventPlan,
    verify::{self, Nip01SignatureVerifier, RawEvent},
};
use radroots_identity::PublicKey;
use sha2::{Digest, Sha256};
#[cfg(feature = "std")]
use std::string::String;
use zeroize::Zeroizing;

/// Maximum complete serialized event, including keys, tags and signature.
pub const SIGNED_EVENT_MAX_BYTES: usize = 65_536;
/// Maximum supported authored timestamp in seconds (year 9999).
pub const MAX_EVENT_UNIX_SECONDS: u64 = 253_402_300_799;
const MAX_OBSERVATION_UNIX_MS: u64 = MAX_EVENT_UNIX_SECONDS * 1000 + 999;
const INTENT_DOMAIN: &[u8] = b"radroots.exact_signing_intent.v1\0";

#[cfg(feature = "chat")]
mod dm_relays;
#[cfg(feature = "chat")]
pub use dm_relays::VerifiedDmRelayAdvertisement;

/// Transport artifact role, separate from a participant's command permission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ArtifactRole {
    /// A public authored event, without private-carrier or rumor kinds.
    PublicEvent,
    /// An encrypted NIP-59 seal (kind 13) for one exact recipient.
    BusinessSeal,
    /// An outer NIP-59 gift wrap (kind 1059) for one exact recipient.
    GiftWrap,
}

/// Fixed provenance of one captured signing intent. This value grants nothing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IntentBinding {
    operation_id: OperationId,
    command_digest: [u8; 32],
    digest: [u8; 32],
}
impl IntentBinding {
    /// Returns the full canonical operation identity, never a truncated alias.
    pub const fn operation_id(self) -> OperationId {
        self.operation_id
    }
    /// Returns the captured command digest.
    pub const fn command_digest(&self) -> &[u8; 32] {
        &self.command_digest
    }
    /// Returns the domain-separated exact intent commitment.
    pub const fn digest(&self) -> &[u8; 32] {
        &self.digest
    }
}

/// Bounded exact preimage and immutable provenance. It is neither an
/// authenticated command context nor evidence of T0/T1 admission.
/// It has no automatic input-bearing diagnostics or serialization.
pub struct SigningIntent {
    binding: IntentBinding,
    plan: AuthoredEventPlan,
    role: ArtifactRole,
    recipient: Option<PublicKey>,
    call: ProviderCall,
    signed_json_bytes: usize,
}
impl fmt::Debug for SigningIntent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SigningIntent(<redacted>)")
    }
}
impl SigningIntent {
    /// Freezes an already validated authored preimage and its captured context.
    /// Complete-event and parser budgets are checked without serializing or
    /// cloning the plan. A tighter business profile remains executor-owned.
    pub fn new(
        operation_id: OperationId,
        command_digest: [u8; 32],
        plan: AuthoredEventPlan,
        role: ArtifactRole,
        recipient: Option<PublicKey>,
        call: ProviderCall,
    ) -> Result<Self, ProviderError> {
        if plan.author() != &call.identity()
            || plan.created_at() > MAX_EVENT_UNIX_SECONDS
            || call.deadline_unix_ms().get() > MAX_OBSERVATION_UNIX_MS
        {
            return Err(ProviderError::InvalidInput);
        }
        let kind = plan.body().kind();
        let role_valid = match role {
            ArtifactRole::PublicEvent => recipient.is_none() && !matches!(kind, 13 | 14 | 1059),
            ArtifactRole::BusinessSeal => {
                kind == 13 && recipient.is_some() && plan.body().tags().is_empty()
            }
            ArtifactRole::GiftWrap => {
                kind == 1059
                    && recipient.is_some_and(|peer| {
                        let tags = plan.body().tags();
                        let mut recipients = tags
                            .iter()
                            .filter(|tag| tag.first().is_some_and(|field| field == "p"));
                        recipients
                            .next()
                            .is_some_and(|tag| tag.len() == 2 && tag[1] == peer.to_hex())
                            && recipients.next().is_none()
                    })
            }
        };
        if !role_valid {
            return Err(ProviderError::InvalidInput);
        }
        let signed_json_bytes = signed_json_len(&plan)?;
        let mut digest = Sha256::new();
        digest.update(INTENT_DOMAIN);
        digest.update(operation_id.as_bytes());
        digest.update(command_digest);
        digest.update(plan.digest().as_bytes());
        digest.update([match role {
            ArtifactRole::PublicEvent => 0,
            ArtifactRole::BusinessSeal => 1,
            ArtifactRole::GiftWrap => 2,
        }]);
        match recipient {
            Some(peer) => {
                digest.update([1]);
                digest.update(peer.as_bytes());
            }
            None => digest.update([0]),
        }
        digest.update(call.deadline_unix_ms().get().to_be_bytes());
        let binding = IntentBinding {
            operation_id,
            command_digest,
            digest: digest.finalize().into(),
        };
        Ok(Self {
            binding,
            plan,
            role,
            recipient,
            call,
            signed_json_bytes,
        })
    }
    /// Converts one bounded known-kind seal into the existing exact signer
    /// contract. This codec conversion grants no command, route or journal
    /// authority; the executor must retain the full plan before dispatch.
    #[cfg(feature = "chat")]
    pub fn from_chat_seal(
        operation_id: OperationId,
        command_digest: radroots_domain::CommandDigest,
        seal: &radroots_protocol::chat::ChatSeal,
        call: ProviderCall,
    ) -> Result<Self, ProviderError> {
        if seal.operation_id() != operation_id || seal.command_digest() != command_digest {
            return Err(ProviderError::InvalidInput);
        }
        let fields = seal.fields();
        let draft = radroots_event::GenericEventDraft::new(
            "radroots.message.seal.v1",
            fields.kind,
            fields.created_at,
            core::iter::empty().collect(),
            fields.content,
            fields.author.to_hex(),
        )
        .map_err(|_| ProviderError::InvalidInput)?;
        let plan =
            AuthoredEventPlan::from_generic(draft).map_err(|_| ProviderError::InvalidInput)?;
        if plan.expected_event_id().as_bytes() != &seal.event_id() {
            return Err(ProviderError::InvalidInput);
        }
        Self::new(
            operation_id,
            *command_digest.as_bytes(),
            plan,
            ArtifactRole::BusinessSeal,
            Some(seal.recipient()),
            call,
        )
    }
    /// Returns the original immutable binding.
    pub const fn binding(&self) -> IntentBinding {
        self.binding
    }
    /// Returns the exact authored-event byte count under the supported serializer.
    pub const fn signed_json_bytes(&self) -> usize {
        self.signed_json_bytes
    }
    /// Returns the expected author, independently of any business actor claim.
    pub const fn author(&self) -> PublicKey {
        *self.plan.author()
    }
    /// Returns the exact artifact role.
    pub const fn role(&self) -> ArtifactRole {
        self.role
    }
    /// Returns the exact recipient for a private carrier.
    pub const fn recipient(&self) -> Option<PublicKey> {
        self.recipient
    }
    /// Exposes the bounded preimage only within an explicit host conversion.
    pub fn with_plan<T>(&self, use_plan: impl FnOnce(&AuthoredEventPlan) -> T) -> T {
        use_plan(&self.plan)
    }
    /// Checks scheduling before effects. Never repeat this after obtaining bytes.
    pub fn check_before_effect(
        &self,
        identity: PublicKey,
        now_unix_ms: u64,
    ) -> Result<(), ProviderError> {
        self.call.check_before_effect(identity, now_unix_ms)
    }
}

fn signed_json_len(plan: &AuthoredEventPlan) -> Result<usize, ProviderError> {
    if plan.body().kind() > u32::from(u16::MAX) {
        return Err(ProviderError::UnsupportedVersion);
    }
    let limits = EventWireLimits::default();
    let mut elements = 0usize;
    let mut tag_bytes = 0usize;
    if plan.body().tags().len() > limits.max_tag_count {
        return Err(ProviderError::LimitExceeded);
    }
    for tag in plan.body().tags() {
        elements = elements
            .checked_add(tag.len())
            .filter(|n| *n <= limits.max_total_tag_elements)
            .ok_or(ProviderError::LimitExceeded)?;
        for field in tag {
            if field.len() > limits.max_tag_element_bytes {
                return Err(ProviderError::LimitExceeded);
            }
            tag_bytes = tag_bytes
                .checked_add(field.len())
                .filter(|n| *n <= limits.max_total_tag_bytes)
                .ok_or(ProviderError::LimitExceeded)?;
        }
    }
    radroots_protocol::nip01::signed_json_len(
        &radroots_protocol::nip01::EventFields {
            author: *plan.author(),
            created_at: plan.created_at(),
            kind: plan.body().kind(),
            tags: plan.body().tags(),
            content: plan.body().content(),
        },
        SIGNED_EVENT_MAX_BYTES,
    )
    .map_err(|_| ProviderError::LimitExceeded)
}

/// Scope of a forensic digest. A prefix digest never claims full coverage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArtifactFingerprint {
    sha256: [u8; 32],
    covered_bytes: usize,
    total_bytes: usize,
}
impl ArtifactFingerprint {
    fn of(raw: &[u8]) -> Self {
        let covered_bytes = raw.len().min(SIGNED_EVENT_MAX_BYTES);
        Self {
            sha256: Sha256::digest(&raw[..covered_bytes]).into(),
            covered_bytes,
            total_bytes: raw.len(),
        }
    }
    /// Returns the SHA-256 of exactly the reported covered prefix.
    pub const fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }
    /// Returns the number of bytes included in the digest.
    pub const fn covered_bytes(self) -> usize {
        self.covered_bytes
    }
    /// Returns the original response size, without retaining its payload.
    pub const fn total_bytes(self) -> usize {
        self.total_bytes
    }
}
/// A fixed classification of untrusted signer output.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RejectionReason {
    /// The response exceeds the complete-event budget.
    TooLarge,
    /// Structural decoding, exact fields or the supported profile do not match.
    InvalidArtifact,
    /// Cryptographic verification failed.
    InvalidSignature,
    /// The returned provenance belongs to another captured intent.
    BindingMismatch,
}
/// Input-free forensic metadata. This supplies no authenticated business fact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RejectedArtifact {
    reason: RejectionReason,
    fingerprint: Option<ArtifactFingerprint>,
}
impl RejectedArtifact {
    /// Returns the fixed classification.
    pub const fn reason(self) -> RejectionReason {
        self.reason
    }
    /// Returns exact digest scope when response bytes were available.
    pub const fn fingerprint(self) -> Option<ArtifactFingerprint> {
        self.fingerprint
    }
}
/// Authenticated exact bytes, without a durable-admission or business claim.
/// No automatic serialization, cloning or payload-bearing diagnostics exists.
pub struct SignedArtifact {
    raw_json: Zeroizing<String>,
    event_id: EventId,
    observed_at_unix_ms: Option<NonZeroU64>,
}
impl fmt::Debug for SignedArtifact {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SignedArtifact(<redacted>)")
    }
}
impl SignedArtifact {
    /// Returns the exact authenticated event identity.
    pub const fn event_id(&self) -> EventId {
        self.event_id
    }
    /// Returns a checked positive observation, or explicit unknown time.
    pub const fn observed_at_unix_ms(&self) -> Option<NonZeroU64> {
        self.observed_at_unix_ms
    }
    /// Exposes original bytes only for explicit capture/transport work.
    pub fn with_json<T>(&self, use_json: impl FnOnce(&str) -> T) -> T {
        use_json(&self.raw_json)
    }
}
/// What a qualified provider reports, separately from durable capture.
#[derive(Debug)]
#[non_exhaustive]
pub enum OutcomeKind {
    /// This provider proves its invocation did not dispatch signing.
    NotDispatched(ProviderError),
    /// Exact authenticated bytes were obtained; scheduling may have stopped.
    Obtained(SignedArtifact),
    /// No conclusive issuance result is available; retain uncertainty.
    Unknown(ProviderError),
    /// Untrusted output is not a business success or proof of no issuance.
    Rejected(RejectedArtifact),
}
/// One result bound to the original captured intent. It is not an operation receipt.
#[derive(Debug)]
pub struct SigningOutcome {
    binding: IntentBinding,
    kind: OutcomeKind,
}
impl SigningOutcome {
    /// Reports a qualified pre-dispatch refusal, without effects or legacy fallback.
    pub const fn not_dispatched(binding: IntentBinding, reason: ProviderError) -> Self {
        Self {
            binding,
            kind: OutcomeKind::NotDispatched(reason),
        }
    }
    /// Reports uncertainty, which never permits blind reissue or hold release.
    pub const fn unknown(binding: IntentBinding, reason: ProviderError) -> Self {
        Self {
            binding,
            kind: OutcomeKind::Unknown(reason),
        }
    }
    /// Verifies an obtained response without consulting scheduling or clocks.
    /// Malformed output retains only bounded forensic metadata. Valid bytes are
    /// preserved even with unavailable, expired or cancelled observation state.
    pub fn capture(
        intent: &SigningIntent,
        raw_json: String,
        observed_at_unix_ms: Option<NonZeroU64>,
    ) -> Self {
        let raw = Zeroizing::new(raw_json);
        let fingerprint = ArtifactFingerprint::of(raw.as_bytes());
        let reject = |reason| Self {
            binding: intent.binding(),
            kind: OutcomeKind::Rejected(RejectedArtifact {
                reason,
                fingerprint: Some(fingerprint),
            }),
        };
        if raw.len() > SIGNED_EVENT_MAX_BYTES {
            return reject(RejectionReason::TooLarge);
        }
        let limits = EventWireLimits {
            max_raw_json_bytes: SIGNED_EVENT_MAX_BYTES,
            max_extra_fields: 0,
            max_total_extra_json_bytes: 0,
            ..EventWireLimits::default()
        };
        let Ok(wire) = Nip01EventWire::parse_json_exact_with_limits(&raw, limits) else {
            return reject(RejectionReason::InvalidArtifact);
        };
        let plan = &intent.plan;
        if wire.pubkey != plan.author().to_hex()
            || wire.created_at != plan.created_at()
            || wire.kind != plan.body().kind()
            || wire.tags != plan.body().tags()
            || wire.content != plan.body().content()
            || wire.id != plan.expected_event_id().to_hex()
        {
            return reject(RejectionReason::InvalidArtifact);
        }
        let Ok(event) = SignedEvent::from_wire_verified_id(wire, raw.as_str()) else {
            return reject(RejectionReason::InvalidArtifact);
        };
        let Ok(verified_id) = verify::id(RawEvent::new(event.envelope().clone())) else {
            return reject(RejectionReason::InvalidArtifact);
        };
        if verify::signature(verified_id, &Nip01SignatureVerifier).is_err() {
            return reject(RejectionReason::InvalidSignature);
        }
        Self {
            binding: intent.binding(),
            kind: OutcomeKind::Obtained(SignedArtifact {
                raw_json: raw,
                event_id: *plan.expected_event_id(),
                observed_at_unix_ms: observed_at_unix_ms
                    .filter(|at| at.get() <= MAX_OBSERVATION_UNIX_MS),
            }),
        }
    }
    /// Returns the exact captured provenance.
    pub const fn binding(&self) -> IntentBinding {
        self.binding
    }
    /// Borrows the classified result without cloning private bytes.
    pub const fn kind(&self) -> &OutcomeKind {
        &self.kind
    }
    /// Transfers the original binding and classified owned result.
    pub fn into_parts(self) -> (IntentBinding, OutcomeKind) {
        (self.binding, self.kind)
    }
    /// Refuses cross-intent callback results without promoting them to success
    /// or proof of no issuance. Any available bytes retain scoped forensics.
    pub fn enforce_binding(self, expected: IntentBinding) -> Self {
        if self.binding == expected {
            return self;
        }
        let fingerprint = match &self.kind {
            OutcomeKind::Obtained(artifact) => {
                Some(artifact.with_json(|raw| ArtifactFingerprint::of(raw.as_bytes())))
            }
            OutcomeKind::Rejected(rejected) => rejected.fingerprint,
            _ => None,
        };
        Self {
            binding: expected,
            kind: OutcomeKind::Rejected(RejectedArtifact {
                reason: RejectionReason::BindingMismatch,
                fingerprint,
            }),
        }
    }
}
