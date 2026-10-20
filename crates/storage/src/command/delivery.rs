//! Bounded private delivery data. Constructors authenticate carriers only;
//! actual protected work, containment, authorization and dispatch belong to
//! qualified store/execution owners, never to a report or enum variant.

use super::{CommitAttempt, OperationError, PublicationWork};
use crate::workspace::WorkspaceBinding;
use core::{fmt, num::NonZeroU64};
use radroots_domain::{EnvelopeEventId, PublicKey, TransactionId, UnixSeconds};
use radroots_event::{
    SignedEvent,
    wire::{EventWireLimits, Nip01EventWire},
};
use radroots_event_codec::verify::{self, Nip01SignatureVerifier, RawEvent};
use radroots_transport::{
    TargetSet, TransportId,
    policy::{SatisfactionClass, SatisfactionPolicy},
    sink::{DeliveryPayload, DeliveryRequest, DeliveryRequestId},
};
use sha2::{Digest, Sha256};
use std::{boxed::Box, collections::BTreeSet, string::String, vec, vec::Vec};
use zeroize::Zeroizing;

const SIGNED_MAX_BYTES: usize = 65_536;
const MAX_TIME: u64 = 253_402_300_799;

fn outer(raw: &str, recipient: PublicKey) -> Result<SignedEvent, OperationError> {
    if raw.len() > SIGNED_MAX_BYTES {
        return Err(OperationError::InvalidEvidence);
    }
    let limits = EventWireLimits {
        max_raw_json_bytes: SIGNED_MAX_BYTES,
        max_extra_fields: 0,
        max_total_extra_json_bytes: 0,
        ..Default::default()
    };
    let wire = Nip01EventWire::parse_json_exact_with_limits(raw, limits)
        .map_err(|_| OperationError::InvalidEvidence)?;
    if wire.kind != 1059
        || wire.created_at == 0
        || wire.created_at > MAX_TIME
        || wire.tags != [vec![String::from("p"), recipient.to_hex()]]
    {
        return Err(OperationError::InvalidEvidence);
    }
    let event = SignedEvent::from_wire_verified_id(wire, raw)
        .map_err(|_| OperationError::InvalidEvidence)?;
    let checked = verify::id(RawEvent::new(event.envelope().clone()))
        .map_err(|_| OperationError::InvalidEvidence)?;
    verify::signature(checked, &Nip01SignatureVerifier)
        .map_err(|_| OperationError::InvalidEvidence)?;
    Ok(event)
}

/// One obtained outer awaiting protected native verification and retention.
/// The key is temporary wrapping material, not participant custody. A matching
/// signature proves neither original-seal containment nor an actual store link.
pub struct EnvelopeCandidate {
    work: PublicationWork,
    event: EnvelopeEventId,
    author: PublicKey,
    created_at: UnixSeconds,
    digest: [u8; 32],
    raw: Zeroizing<String>,
    key: Zeroizing<[u8; 32]>,
}
impl EnvelopeCandidate {
    /// Bounds before copying and verifies the exact one-recipient outer carrier.
    /// Native capture must independently derive the key and decrypt its content
    /// against actual original retained seal bytes before any transport right.
    pub fn from_obtained(
        work: PublicationWork,
        raw: &str,
        wrapping_key: &[u8; 32],
    ) -> Result<Self, OperationError> {
        let carrier = outer(raw, work.recipient())?;
        Ok(Self {
            work,
            event: EnvelopeEventId::from_bytes(*carrier.envelope().id().as_bytes()),
            author: *carrier.envelope().author(),
            created_at: UnixSeconds::new(carrier.envelope().created_at_u64())
                .map_err(|_| OperationError::InvalidEvidence)?,
            digest: Sha256::digest(raw.as_bytes()).into(),
            raw: Zeroizing::new(String::from(raw)),
            key: Zeroizing::new(*wrapping_key),
        })
    }
    /// Reported original work; a getter does not authenticate backend state.
    pub const fn work(&self) -> PublicationWork {
        self.work
    }
    /// Actual signed outer event identity, separate from the original seal.
    pub const fn event_id(&self) -> EnvelopeEventId {
        self.event
    }
    /// Actual outer author; native capture must prove this temporary key.
    pub const fn author(&self) -> PublicKey {
        self.author
    }
    /// Signed randomized outer time, not a grant or trusted clock observation.
    pub const fn created_at(&self) -> UnixSeconds {
        self.created_at
    }
    /// SHA-256 of the exact original outer representation.
    pub const fn digest(&self) -> &[u8; 32] {
        &self.digest
    }
    /// Exact obtained bytes for protected handling; never diagnostic output.
    pub fn raw_json(&self) -> &str {
        &self.raw
    }
    /// Scoped borrow for native containment verification only.
    pub fn with_wrapping_key<T>(&self, verify: impl FnOnce(&[u8; 32]) -> T) -> T {
        verify(&self.key)
    }
    /// Explicit bounded owner copy; the caller retains its original until the
    /// actual outcome is reconciled. This creates no persistence fact.
    pub fn retained_copy(&self) -> Self {
        Self {
            work: self.work,
            event: self.event,
            author: self.author,
            created_at: self.created_at,
            digest: self.digest,
            raw: Zeroizing::new(String::from(self.raw_json())),
            key: Zeroizing::new(*self.key),
        }
    }
}
impl fmt::Debug for EnvelopeCandidate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("EnvelopeCandidate(<redacted>)")
    }
}

/// Immutable one-attempt transport intent. Every target is explicit; neither
/// policy nor target construction proves an original route approval or lease.
#[derive(Clone, Eq, PartialEq)]
pub struct DeliveryIntent {
    request: DeliveryRequestId,
    event: EnvelopeEventId,
    targets: TargetSet,
    policy: SatisfactionPolicy,
    selection: Box<[radroots_transport::target::TargetFingerprint]>,
    publication_policy: [u8; 32],
    deadline: NonZeroU64,
    digest: [u8; 32],
}
impl DeliveryIntent {
    /// Captures a full 256-bit lowercase-hex request identity, at most sixteen
    /// Nostr targets and explicit relay-acceptance policy. Relay acceptance
    /// cannot prove recipient delivery. Owned copies shed caller spare capacity.
    pub fn new(
        request_id: &str,
        event: EnvelopeEventId,
        targets: &TargetSet,
        policy: &SatisfactionPolicy,
        deadline: NonZeroU64,
    ) -> Result<Self, OperationError> {
        if request_id.len() != 64
            || !request_id
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || targets.len() > 16
            || targets.is_empty()
            || deadline.get() > MAX_TIME * 1000 + 999
            || policy.class() != SatisfactionClass::Accepted
            || targets
                .targets()
                .iter()
                .any(|t| t.kind() != &TransportId::NOSTR || t.uri().as_str().len() > 2048)
        {
            return Err(OperationError::InvalidCommand);
        }
        policy
            .validate_for(targets)
            .map_err(|_| OperationError::InvalidCommand)?;
        let mut endpoints = BTreeSet::new();
        if targets
            .targets()
            .iter()
            .any(|target| !endpoints.insert(target.uri().as_str()))
        {
            return Err(OperationError::InvalidCommand);
        }
        let request =
            DeliveryRequestId::parse(request_id).map_err(|_| OperationError::InvalidCommand)?;
        let mut hash = Sha256::new();
        hash.update(b"radroots.private.delivery.publication_policy.v1\0");
        hash.update((targets.len() as u16).to_be_bytes());
        for target in targets.targets() {
            hash.update(target.fingerprint().as_str().as_bytes());
        }
        let selected = policy.targets();
        if selected.is_any() {
            hash.update([0]);
        } else if selected.is_all() {
            hash.update([1]);
        } else if let Some(quorum) = selected.quorum_threshold() {
            hash.update([2]);
            hash.update(quorum.to_be_bytes());
        } else if let Some(required) = selected.required_targets() {
            hash.update([3]);
            hash.update((required.len() as u16).to_be_bytes());
            for target in required {
                hash.update(target.as_str().as_bytes());
            }
        } else {
            return Err(OperationError::InvalidCommand);
        }
        let mut value = Self {
            request,
            event,
            targets: targets.clone(),
            policy: policy.clone(),
            selection: targets
                .targets()
                .iter()
                .map(|target| target.fingerprint().clone())
                .collect(),
            publication_policy: hash.finalize().into(),
            deadline,
            digest: [0; 32],
        };
        value.refresh_digest();
        Ok(value)
    }
    fn refresh_digest(&mut self) {
        let mut hash = Sha256::new();
        hash.update(b"radroots.private.delivery.intent.v1\0");
        hash.update(self.request.as_str().as_bytes());
        hash.update(self.event.as_bytes());
        hash.update(self.publication_policy);
        hash.update(self.deadline.get().to_be_bytes());
        hash.update((self.selection.len() as u16).to_be_bytes());
        for target in &self.selection {
            hash.update(target.as_str().as_bytes());
        }
        self.digest = hash.finalize().into();
    }
    /// Selects a nonempty exact subset for this potential invocation, retaining
    /// the full original target set, completion policy and deadline. Only selected
    /// endpoints spend attempt budgets. Data construction grants no dispatch right.
    pub fn with_selection(mut self, selected: &TargetSet) -> Result<Self, OperationError> {
        if selected.is_empty()
            || selected
                .targets()
                .iter()
                .any(|target| !self.targets.targets().contains(target))
        {
            return Err(OperationError::InvalidCommand);
        }
        self.selection = self
            .targets
            .targets()
            .iter()
            .filter(|target| selected.contains(target.fingerprint()))
            .map(|target| target.fingerprint().clone())
            .collect();
        self.refresh_digest();
        Ok(self)
    }
    /// Complete original publication criterion, excluding attempt ID, envelope
    /// selection and per-attempt deadline. The native owner freezes this
    /// criterion for the original recipient work.
    pub const fn publication_policy_digest(&self) -> &[u8; 32] {
        &self.publication_policy
    }
    /// Selected target identities in original full-set order.
    pub fn selection(&self) -> &[radroots_transport::target::TargetFingerprint] {
        &self.selection
    }
    /// Exact selected descriptors in original full-set order.
    pub fn selected_targets(&self) -> impl Iterator<Item = &radroots_transport::Target> {
        self.targets
            .targets()
            .iter()
            .filter(|target| self.selection.contains(target.fingerprint()))
    }
    /// Builds the bounded exact subset for `EventSink::deliver_selected`.
    /// The full original transport request must still be validated separately.
    pub fn selected_target_set(&self) -> Result<TargetSet, OperationError> {
        TargetSet::new(self.selected_targets().cloned().collect())
            .map_err(|_| OperationError::InvalidCommand)
    }
    /// Exact full attempt identity, not diagnostic correlation.
    pub const fn request_id(&self) -> &DeliveryRequestId {
        &self.request
    }
    /// Actual selected outer identity.
    pub const fn event_id(&self) -> EnvelopeEventId {
        self.event
    }
    /// Original selected target descriptors in request order.
    pub const fn targets(&self) -> &TargetSet {
        &self.targets
    }
    /// Explicit relay-acceptance law, independent of business installation.
    pub const fn policy(&self) -> &SatisfactionPolicy {
        &self.policy
    }
    /// Original absolute deadline; replay grants no fresh time window.
    pub const fn deadline_unix_ms(&self) -> NonZeroU64 {
        self.deadline
    }
    /// Commitment to identity, envelope, deadline, ordered target identities and policy.
    /// Human target labels are diagnostic and excluded.
    pub const fn digest(&self) -> &[u8; 32] {
        &self.digest
    }
}
impl fmt::Debug for DeliveryIntent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DeliveryIntent(<redacted>)")
    }
}

/// Selects actual retained publication work for one potential transport attempt.
pub struct DeliveryAttemptRequest {
    binding: WorkspaceBinding,
    work: PublicationWork,
    intent: DeliveryIntent,
}
impl DeliveryAttemptRequest {
    /// Structural scope comparison only; a native owner must authenticate the
    /// original complete graph, current grant and exact selected routes.
    pub fn new(
        binding: WorkspaceBinding,
        work: PublicationWork,
        intent: DeliveryIntent,
    ) -> Result<Self, OperationError> {
        if binding.input().workspace_id() != work.evidence().workspace_id() {
            return Err(OperationError::InvalidCommand);
        }
        Ok(Self {
            binding,
            work,
            intent,
        })
    }
    /// Reported authenticated binding, subject to current native revalidation.
    pub const fn binding(&self) -> &WorkspaceBinding {
        &self.binding
    }
    /// Immutable reported work; native capture compares its actual protected row.
    pub const fn work(&self) -> PublicationWork {
        self.work
    }
    /// Exact requested transport intent.
    pub const fn intent(&self) -> &DeliveryIntent {
        &self.intent
    }
    /// Original operation anchor; no outcome is inferred from construction.
    pub fn identity(&self) -> CommitAttempt {
        let e = self.work.evidence();
        CommitAttempt::new(e.workspace_id(), e.operation_id(), e.command_digest(), None)
    }
}
impl fmt::Debug for DeliveryAttemptRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DeliveryAttemptRequest(<redacted>)")
    }
}

/// Complete original attempt report and exact bytes. Constructing or querying
/// this value never permits replay of a potential invocation.
pub struct DeliveryAttempt {
    work: PublicationWork,
    intent: DeliveryIntent,
    raw: Zeroizing<String>,
    digest: [u8; 32],
    transaction: TransactionId,
    retained_at: UnixSeconds,
    ordinals: Box<[u8]>,
}
impl DeliveryAttempt {
    /// Checks scoped data and carrier authenticity only. Native backends must
    /// independently verify original protected state and execute retention.
    pub fn from_store_report(
        work: PublicationWork,
        intent: DeliveryIntent,
        raw: &str,
        transaction: TransactionId,
        retained_at: UnixSeconds,
        ordinals: Vec<u8>,
    ) -> Result<Self, OperationError> {
        let event = outer(raw, work.recipient())?;
        if event.envelope().id().as_bytes() != intent.event_id().as_bytes()
            || retained_at < work.queued_at()
            || event.envelope().created_at_u64() > retained_at.get()
            || retained_at
                .get()
                .checked_mul(1000)
                .is_none_or(|now| now >= intent.deadline_unix_ms().get())
            || ordinals.len() != intent.selection().len()
            || ordinals.iter().any(|n| !(1..=12).contains(n))
        {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(Self {
            work,
            intent,
            raw: Zeroizing::new(String::from(raw)),
            digest: Sha256::digest(raw.as_bytes()).into(),
            transaction,
            retained_at,
            ordinals: ordinals.into_boxed_slice(),
        })
    }
    /// Original protected publication reference.
    pub const fn work(&self) -> PublicationWork {
        self.work
    }
    /// Original complete transport intent.
    pub const fn intent(&self) -> &DeliveryIntent {
        &self.intent
    }
    /// Exact originally retained outer representation.
    pub fn raw_json(&self) -> &str {
        &self.raw
    }
    /// Digest of the exact retained representation.
    pub const fn envelope_digest(&self) -> &[u8; 32] {
        &self.digest
    }
    /// Original actual store transaction.
    pub const fn transaction_id(&self) -> TransactionId {
        self.transaction
    }
    /// Original retention time; never a renewed dispatch deadline.
    pub const fn retained_at(&self) -> UnixSeconds {
        self.retained_at
    }
    /// Per-target potential invocation numbers, in exact target order.
    pub fn target_ordinals(&self) -> &[u8] {
        &self.ordinals
    }
    /// Reconstructs the full original request. A qualified owner must use
    /// `EventSink::deliver_selected` with `intent().selected_target_set()`;
    /// unselected endpoints have no retained invocation. This data conversion
    /// grants no dispatch permission or automatic retry.
    pub fn transport_request(&self) -> Result<DeliveryRequest, OperationError> {
        let event = outer(self.raw_json(), self.work.recipient())?;
        DeliveryRequest::new(
            self.intent.request_id().as_str(),
            DeliveryPayload::new(event),
            self.intent.targets().clone(),
            self.intent.policy().clone(),
            self.intent.deadline_unix_ms().get(),
        )
        .map_err(|_| OperationError::RecoveryRequired)
    }
}
impl fmt::Debug for DeliveryAttempt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DeliveryAttempt(<redacted>)")
    }
}

/// Only an actual freshly committed native claim can permit its qualified
/// owner to proceed once after current route/provider/access checks.
pub enum DeliveryAttemptOutcome {
    /// Actual new protected attempt and exact bytes committed before transport.
    Claimed(Box<DeliveryAttempt>),
    /// Existing potential invocation, never permission to dispatch again.
    Retained(Box<DeliveryAttempt>),
    /// Reconcile this exact original identity before any further scheduling.
    Unknown {
        /// Original workspace/operation/command and optional transaction anchor.
        operation: CommitAttempt,
        /// Full original transport attempt identity.
        attempt: DeliveryRequestId,
    },
}
impl fmt::Debug for DeliveryAttemptOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DeliveryAttemptOutcome(<redacted>)")
    }
}
