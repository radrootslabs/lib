//! Exact-intent signature facts with nullable observation and no durable claim.

use crate::provider::{ProviderCall, ProviderError};
#[cfg(not(feature = "std"))]
use alloc::string::String;
use core::{fmt, num::NonZeroU64};
use radroots_domain::OperationId;
use radroots_event::{
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

#[cfg(feature = "chat")]
mod dm_relays;
#[cfg(feature = "chat")]
pub use dm_relays::VerifiedDmRelayAdvertisement;

pub use radroots_protocol::intent::{ArtifactRole, IntentBinding};

/// Bounded exact preimage and immutable provenance. It is neither an
/// authenticated command context nor evidence of T0/T1 admission.
/// It has no automatic input-bearing diagnostics or serialization.
pub struct SigningIntent {
    binding: IntentBinding,
    plan: ExactPreimage,
    role: ArtifactRole,
    recipient: Option<PublicKey>,
    call: ProviderCall,
    signed_json_bytes: usize,
}
enum ExactPreimage {
    Authored(AuthoredEventPlan),
    #[cfg(feature = "artifacts")]
    Listing(radroots_protocol::listing::ListingEvent),
}
impl ExactPreimage {
    fn fields(&self) -> radroots_protocol::nip01::EventFields<'_> {
        match self {
            Self::Authored(plan) => radroots_protocol::nip01::EventFields {
                author: *plan.author(),
                created_at: plan.created_at(),
                kind: plan.body().kind(),
                tags: plan.body().tags(),
                content: plan.body().content(),
            },
            #[cfg(feature = "artifacts")]
            Self::Listing(event) => event.fields(),
        }
    }
    fn event_id(&self) -> radroots_event::id::EventId {
        match self {
            Self::Authored(plan) => *plan.expected_event_id(),
            #[cfg(feature = "artifacts")]
            Self::Listing(event) => radroots_event::id::EventId::from_bytes(*event.event_id()),
        }
    }
}
impl fmt::Debug for SigningIntent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SigningIntent(<redacted>)")
    }
}
impl SigningIntent {
    /// Converts one exact retained-plan artifact to the existing signing port.
    /// This verifies structural identity only; actual T1/first-claim commits,
    /// current grant, destination policy and provider support remain required.
    #[cfg(feature = "artifacts")]
    pub fn from_plan(
        plan: &radroots_protocol::plan::CapturedSigningPlan,
        target: radroots_protocol::plan::SigningTarget,
        call: ProviderCall,
    ) -> Result<Self, ProviderError> {
        use radroots_protocol::plan::SigningArtifact;
        match plan
            .artifact(target)
            .map_err(|_| ProviderError::InvalidInput)?
        {
            SigningArtifact::Seal(seal) => {
                Self::from_chat_seal(plan.operation_id(), plan.command_digest(), seal, call)
            }
            SigningArtifact::Public(event) => {
                let fields = event.fields();
                if fields.author != call.identity()
                    || fields.created_at > MAX_EVENT_UNIX_SECONDS
                    || call.deadline_unix_ms().get() > MAX_OBSERVATION_UNIX_MS
                {
                    return Err(ProviderError::InvalidInput);
                }
                let signed_json_bytes = signed_json_len(&fields, public_event_limits())?;
                let binding = IntentBinding::from_components(
                    plan.operation_id(),
                    *plan.command_digest().as_bytes(),
                    *plan.digest(),
                    ArtifactRole::PublicEvent,
                    None,
                    call.deadline_unix_ms(),
                )
                .map_err(|_| ProviderError::InvalidInput)?;
                Ok(Self {
                    binding,
                    plan: ExactPreimage::Listing(event.clone()),
                    role: ArtifactRole::PublicEvent,
                    recipient: None,
                    call,
                    signed_json_bytes,
                })
            }
        }
    }
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
            _ => return Err(ProviderError::UnsupportedVersion),
        };
        if !role_valid {
            return Err(ProviderError::InvalidInput);
        }
        let signed_json_bytes = signed_json_len(
            &radroots_protocol::nip01::EventFields {
                author: *plan.author(),
                created_at: plan.created_at(),
                kind: plan.body().kind(),
                tags: plan.body().tags(),
                content: plan.body().content(),
            },
            EventWireLimits::default(),
        )?;
        let binding = IntentBinding::from_components(
            operation_id,
            command_digest,
            *plan.digest().as_bytes(),
            role,
            recipient,
            call.deadline_unix_ms(),
        )
        .map_err(|_| ProviderError::InvalidInput)?;
        Ok(Self {
            binding,
            plan: ExactPreimage::Authored(plan),
            role,
            recipient,
            call,
            signed_json_bytes,
        })
    }
    /// Converts one bounded known-kind seal into the existing exact signer
    /// contract. This codec conversion grants no command, route or journal
    /// authority; the executor must retain the full plan before dispatch.
    #[cfg(any(feature = "chat", feature = "artifacts"))]
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
    pub fn author(&self) -> PublicKey {
        self.plan.fields().author
    }
    /// Returns the exact artifact role.
    pub const fn role(&self) -> ArtifactRole {
        self.role
    }
    /// Returns the exact recipient for a private carrier.
    pub const fn recipient(&self) -> Option<PublicKey> {
        self.recipient
    }
    /// Borrows exact NIP-01 fields and ID for one concrete conversion. The
    /// typed public profile does not pass through a predecessor generic registry.
    pub fn with_event<T>(
        &self,
        use_event: impl FnOnce(
            radroots_protocol::nip01::EventFields<'_>,
            radroots_event::id::EventId,
        ) -> T,
    ) -> T {
        use_event(self.plan.fields(), self.plan.event_id())
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

fn signed_json_len(
    fields: &radroots_protocol::nip01::EventFields<'_>,
    limits: EventWireLimits,
) -> Result<usize, ProviderError> {
    if fields.kind > u32::from(u16::MAX) {
        return Err(ProviderError::UnsupportedVersion);
    }
    let mut elements = 0usize;
    let mut tag_bytes = 0usize;
    if fields.tags.len() > limits.max_tag_count {
        return Err(ProviderError::LimitExceeded);
    }
    for tag in fields.tags {
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
    radroots_protocol::nip01::signed_json_len(fields, SIGNED_EVENT_MAX_BYTES)
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
        let limits = match &intent.plan {
            #[cfg(feature = "artifacts")]
            ExactPreimage::Listing(_) => public_event_limits(),
            _ => EventWireLimits {
                max_raw_json_bytes: SIGNED_EVENT_MAX_BYTES,
                max_extra_fields: 0,
                max_total_extra_json_bytes: 0,
                ..EventWireLimits::default()
            },
        };
        let Ok(wire) = Nip01EventWire::parse_json_exact_with_limits(&raw, limits) else {
            return reject(RejectionReason::InvalidArtifact);
        };
        let fields = intent.plan.fields();
        if wire.pubkey != fields.author.to_hex()
            || wire.created_at != fields.created_at
            || wire.kind != fields.kind
            || wire.tags != fields.tags
            || wire.content != fields.content
            || wire.id != intent.plan.event_id().to_hex()
        {
            return reject(RejectionReason::InvalidArtifact);
        }
        let Ok(envelope) = checked_envelope(wire, limits) else {
            return reject(RejectionReason::InvalidArtifact);
        };
        let Ok(verified_id) = verify::id(RawEvent::new(envelope)) else {
            return reject(RejectionReason::InvalidArtifact);
        };
        if verify::signature(verified_id, &Nip01SignatureVerifier).is_err() {
            return reject(RejectionReason::InvalidSignature);
        }
        Self {
            binding: intent.binding(),
            kind: OutcomeKind::Obtained(SignedArtifact {
                raw_json: raw,
                event_id: intent.plan.event_id(),
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

#[cfg(feature = "artifacts")]
fn public_event_limits() -> EventWireLimits {
    EventWireLimits {
        max_raw_json_bytes: SIGNED_EVENT_MAX_BYTES,
        max_content_bytes: SIGNED_EVENT_MAX_BYTES,
        max_tag_count: radroots_protocol::listing::LISTING_TAG_MAX_COUNT,
        max_total_tag_elements: radroots_protocol::listing::LISTING_TAG_TOTAL_ELEMENT_MAX_COUNT,
        max_tag_element_bytes: SIGNED_EVENT_MAX_BYTES,
        max_total_tag_bytes: SIGNED_EVENT_MAX_BYTES,
        max_extra_fields: 0,
        max_total_extra_json_bytes: 0,
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
