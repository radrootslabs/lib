#![forbid(unsafe_code)]

#[cfg(all(not(feature = "std"), not(test)))]
use alloc::{borrow::ToOwned, string::String, vec::Vec};

#[cfg(any(feature = "std", test))]
use std::{borrow::ToOwned, string::String, vec::Vec};

use crate::contract::registry_v7::{
    ContractValidationError, EventContract, RADROOTS_EVENT_CONTRACT_REGISTRY_VERSION,
    event_contract, validate_event_contract_parts,
};
use crate::envelope::{EventEnvelope, EventEnvelopeError, EventKind, EventTags, EventTimestamp};
use crate::id::{EventId, EventSignature, ParseError, parse_public_key};
use crate::wire::v1::{
    CanonicalEventIdError, EventWireError, Nip01EventWire, canonical_nip01_event_id_preimage,
    compute_canonical_nip01_event_id,
};
use core::fmt;
use radroots_identity::PublicKey;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DraftError {
    UnknownContract(String),
    ContractKindMismatch {
        contract_id: String,
        expected_kind: u32,
        actual_kind: u32,
    },
    ContractNotDraftAuthorable {
        contract_id: String,
    },
    ContractRegistryVersionMismatch {
        expected: u32,
        actual: u32,
    },
    DraftExpectedEventIdMismatch {
        expected_event_id: String,
        actual_event_id: String,
    },
    ContractShape {
        contract_id: String,
        error: ContractValidationError,
    },
    SignedEventPubkeyMismatch {
        expected_pubkey: String,
        actual_pubkey: String,
    },
    SignedEventIdMismatch {
        expected_event_id: String,
        actual_event_id: String,
    },
    SignedEventCreatedAtMismatch {
        expected_created_at: u64,
        actual_created_at: u64,
    },
    SignedEventKindMismatch {
        expected_kind: u32,
        actual_kind: u32,
    },
    SignedEventTagsMismatch {
        expected_len: usize,
        actual_len: usize,
    },
    SignedEventContentMismatch {
        expected_len: usize,
        actual_len: usize,
    },
    SignedEventComputedIdMismatch {
        expected_event_id: String,
        computed_event_id: String,
    },
    IdParse(ParseError),
    CanonicalEventId(CanonicalEventIdError),
    Envelope(EventEnvelopeError),
    SignedEvent(SignedEventError),
}

impl fmt::Display for DraftError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownContract(contract_id) => {
                write!(f, "unknown event contract `{contract_id}`")
            }
            Self::ContractKindMismatch {
                contract_id,
                expected_kind,
                actual_kind,
            } => write!(
                f,
                "event contract `{contract_id}` expects kind {expected_kind}, got {actual_kind}"
            ),
            Self::ContractNotDraftAuthorable { contract_id } => write!(
                f,
                "event contract `{contract_id}` is not authorable through generic frozen drafts"
            ),
            Self::ContractRegistryVersionMismatch { expected, actual } => write!(
                f,
                "event contract registry version mismatch: expected {expected}, got {actual}"
            ),
            Self::DraftExpectedEventIdMismatch {
                expected_event_id,
                actual_event_id,
            } => write!(
                f,
                "frozen draft event ID mismatch: expected {expected_event_id}, got {actual_event_id}"
            ),
            Self::ContractShape { contract_id, error } => write!(
                f,
                "event contract `{contract_id}` shape validation failed with code {}",
                error.code()
            ),
            Self::SignedEventPubkeyMismatch {
                expected_pubkey,
                actual_pubkey,
            } => write!(
                f,
                "signed event pubkey mismatch: expected {expected_pubkey}, got {actual_pubkey}"
            ),
            Self::SignedEventIdMismatch {
                expected_event_id,
                actual_event_id,
            } => write!(
                f,
                "signed event id mismatch: expected {expected_event_id}, got {actual_event_id}"
            ),
            Self::SignedEventCreatedAtMismatch {
                expected_created_at,
                actual_created_at,
            } => write!(
                f,
                "signed event created_at mismatch: expected {expected_created_at}, got {actual_created_at}"
            ),
            Self::SignedEventKindMismatch {
                expected_kind,
                actual_kind,
            } => write!(
                f,
                "signed event kind mismatch: expected {expected_kind}, got {actual_kind}"
            ),
            Self::SignedEventTagsMismatch {
                expected_len,
                actual_len,
            } => write!(
                f,
                "signed event tags mismatch: expected {expected_len} tags, got {actual_len} tags"
            ),
            Self::SignedEventContentMismatch {
                expected_len,
                actual_len,
            } => write!(
                f,
                "signed event content mismatch: expected {expected_len} bytes, got {actual_len} bytes"
            ),
            Self::SignedEventComputedIdMismatch {
                expected_event_id,
                computed_event_id,
            } => write!(
                f,
                "signed event computed id mismatch: expected {expected_event_id}, computed {computed_event_id}"
            ),
            Self::IdParse(error) => write!(f, "{error}"),
            Self::CanonicalEventId(error) => write!(f, "{error}"),
            Self::Envelope(error) => write!(f, "{error}"),
            Self::SignedEvent(error) => write!(f, "{error}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for DraftError {}

impl From<ParseError> for DraftError {
    fn from(value: ParseError) -> Self {
        Self::IdParse(value)
    }
}

impl From<CanonicalEventIdError> for DraftError {
    fn from(value: CanonicalEventIdError) -> Self {
        match value {
            CanonicalEventIdError::InvalidPubkey(error) => Self::IdParse(error),
            error => Self::CanonicalEventId(error),
        }
    }
}

impl From<EventEnvelopeError> for DraftError {
    fn from(value: EventEnvelopeError) -> Self {
        Self::Envelope(value)
    }
}

impl From<SignedEventError> for DraftError {
    fn from(value: SignedEventError) -> Self {
        Self::SignedEvent(value)
    }
}

#[cfg_attr(any(feature = "serde", test), derive(serde::Serialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventDraft {
    contract_id: String,
    contract_registry_version: u32,
    kind: EventKind,
    created_at: EventTimestamp,
    tags: EventTags,
    content: String,
    expected_pubkey: PublicKey,
    expected_event_id: EventId,
}

impl EventDraft {
    pub fn new(
        contract_id: impl Into<String>,
        kind: u32,
        created_at: u64,
        tags: Vec<Vec<String>>,
        content: impl Into<String>,
        expected_pubkey: impl AsRef<str>,
    ) -> Result<Self, DraftError> {
        let contract_id = contract_id.into();
        let contract = match event_contract(&contract_id) {
            Some(contract) => contract,
            None => return Err(DraftError::UnknownContract(contract_id.clone())),
        };
        if contract.kind != kind {
            return Err(DraftError::ContractKindMismatch {
                contract_id,
                expected_kind: contract.kind,
                actual_kind: kind,
            });
        }
        ensure_generic_draft_authorable(contract)?;
        let expected_pubkey = parse_public_key(expected_pubkey.as_ref())?;
        let content = content.into();
        validate_event_contract_parts(kind, &tags, content.as_str(), contract.id).map_err(
            |error| DraftError::ContractShape {
                contract_id: contract.id.to_owned(),
                error,
            },
        )?;
        let typed_tags = EventTags::new(tags)?;
        let expected_pubkey_hex = expected_pubkey.to_hex();
        let expected_event_id = compute_nip01_event_id_for_valid_pubkey(
            expected_pubkey_hex.as_str(),
            created_at,
            kind,
            &typed_tags.to_vec(),
            &content,
        );
        Ok(Self {
            contract_id: contract.id.to_owned(),
            contract_registry_version: RADROOTS_EVENT_CONTRACT_REGISTRY_VERSION,
            kind: EventKind::new(kind),
            created_at: EventTimestamp::new(created_at),
            tags: typed_tags,
            content,
            expected_pubkey,
            expected_event_id,
        })
    }

    pub fn nip01_preimage(&self) -> Result<String, DraftError> {
        let expected_pubkey = self.expected_pubkey.to_hex();
        Ok(nip01_event_id_preimage_for_valid_pubkey(
            expected_pubkey.as_str(),
            self.created_at.as_u64(),
            self.kind.as_u32(),
            &self.tags.to_vec(),
            self.content.as_str(),
        ))
    }

    /// Revalidates registry policy, contract shape, and the deterministic ID.
    ///
    /// Signing boundaries must call this even for a previously validated draft
    /// so persisted data cannot bypass current registry authority.
    pub fn validate_for_signing(&self) -> Result<(), DraftError> {
        if self.contract_registry_version != RADROOTS_EVENT_CONTRACT_REGISTRY_VERSION {
            return Err(DraftError::ContractRegistryVersionMismatch {
                expected: RADROOTS_EVENT_CONTRACT_REGISTRY_VERSION,
                actual: self.contract_registry_version,
            });
        }
        let contract = event_contract(self.contract_id())
            .ok_or_else(|| DraftError::UnknownContract(self.contract_id().to_owned()))?;
        if contract.kind != self.kind_u32() {
            return Err(DraftError::ContractKindMismatch {
                contract_id: contract.id.to_owned(),
                expected_kind: contract.kind,
                actual_kind: self.kind_u32(),
            });
        }
        ensure_generic_draft_authorable(contract)?;
        validate_event_contract_parts(
            self.kind_u32(),
            &self.tags_as_vec(),
            self.content(),
            contract.id,
        )
        .map_err(|error| DraftError::ContractShape {
            contract_id: contract.id.to_owned(),
            error,
        })?;
        let expected_pubkey = self.expected_pubkey.to_hex();
        let actual_event_id = compute_nip01_event_id_for_valid_pubkey(
            expected_pubkey.as_str(),
            self.created_at_u64(),
            self.kind_u32(),
            &self.tags_as_vec(),
            self.content(),
        );
        if actual_event_id != self.expected_event_id {
            return Err(DraftError::DraftExpectedEventIdMismatch {
                expected_event_id: actual_event_id.to_hex(),
                actual_event_id: self.expected_event_id.to_hex(),
            });
        }
        Ok(())
    }

    #[inline]
    pub fn contract_id(&self) -> &str {
        self.contract_id.as_str()
    }

    #[inline]
    pub fn contract_registry_version(&self) -> u32 {
        self.contract_registry_version
    }

    #[inline]
    pub fn kind(&self) -> EventKind {
        self.kind
    }

    #[inline]
    pub fn kind_u32(&self) -> u32 {
        self.kind.as_u32()
    }

    #[inline]
    pub fn created_at(&self) -> EventTimestamp {
        self.created_at
    }

    #[inline]
    pub fn created_at_u64(&self) -> u64 {
        self.created_at.as_u64()
    }

    #[inline]
    pub fn tags(&self) -> &EventTags {
        &self.tags
    }

    pub fn tags_as_vec(&self) -> Vec<Vec<String>> {
        self.tags.to_vec()
    }

    #[inline]
    pub fn content(&self) -> &str {
        self.content.as_str()
    }

    #[inline]
    pub fn expected_pubkey(&self) -> &PublicKey {
        &self.expected_pubkey
    }

    #[inline]
    pub fn expected_event_id(&self) -> &EventId {
        &self.expected_event_id
    }

    #[inline]
    pub fn expected_event_id_hex(&self) -> String {
        self.expected_event_id.to_hex()
    }
}

fn ensure_generic_draft_authorable(contract: &EventContract) -> Result<(), DraftError> {
    if !contract.authoring_policy().permits_generic_draft() {
        return Err(DraftError::ContractNotDraftAuthorable {
            contract_id: contract.id.to_owned(),
        });
    }
    Ok(())
}

#[cfg(any(feature = "serde", test))]
impl<'de> serde::Deserialize<'de> for EventDraft {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct DraftSerde {
            contract_id: String,
            contract_registry_version: u32,
            kind: EventKind,
            created_at: EventTimestamp,
            tags: EventTags,
            content: String,
            expected_pubkey: PublicKey,
            expected_event_id: EventId,
        }

        let value = DraftSerde::deserialize(deserializer)?;
        if value.contract_registry_version != RADROOTS_EVENT_CONTRACT_REGISTRY_VERSION {
            return Err(serde::de::Error::custom(
                DraftError::ContractRegistryVersionMismatch {
                    expected: RADROOTS_EVENT_CONTRACT_REGISTRY_VERSION,
                    actual: value.contract_registry_version,
                },
            ));
        }
        let draft = Self::new(
            value.contract_id,
            value.kind.as_u32(),
            value.created_at.as_u64(),
            value.tags.to_vec(),
            value.content,
            value.expected_pubkey.to_hex(),
        )
        .map_err(serde::de::Error::custom)?;
        if draft.expected_event_id != value.expected_event_id {
            return Err(serde::de::Error::custom(
                DraftError::DraftExpectedEventIdMismatch {
                    expected_event_id: draft.expected_event_id.to_hex(),
                    actual_event_id: value.expected_event_id.to_hex(),
                },
            ));
        }
        Ok(draft)
    }
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignedEventParts {
    pub id: String,
    pub pubkey: String,
    pub created_at: u64,
    pub kind: u32,
    pub tags: Vec<Vec<String>>,
    pub content: String,
    pub sig: String,
    pub raw_json: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignedEvent {
    envelope: EventEnvelope,
    wire: Nip01EventWire,
    raw_json: String,
}

#[cfg(any(feature = "serde", test))]
impl serde::Serialize for SignedEvent {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;

        let mut state = serializer.serialize_struct("SignedEvent", 2)?;
        state.serialize_field("wire", &self.wire)?;
        state.serialize_field("raw_json", &self.raw_json)?;
        state.end()
    }
}

#[cfg(any(feature = "serde", test))]
impl<'de> serde::Deserialize<'de> for SignedEvent {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        struct SignedEventSerde {
            wire: Nip01EventWire,
            raw_json: String,
        }

        let value = SignedEventSerde::deserialize(deserializer)?;
        SignedEvent::from_wire_verified_id(value.wire, value.raw_json)
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SignedEventError {
    Wire(EventWireError),
    RawJson(EventWireError),
    RawJsonMismatch,
    Envelope(EventEnvelopeError),
}

impl fmt::Display for SignedEventError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Wire(error) => write!(f, "signed event wire is invalid: {error}"),
            Self::RawJson(error) => write!(f, "signed event raw JSON is invalid: {error}"),
            Self::RawJsonMismatch => {
                write!(
                    f,
                    "signed event raw JSON does not match the provided wire event"
                )
            }
            Self::Envelope(error) => write!(f, "signed event envelope is invalid: {error}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for SignedEventError {}

impl From<EventEnvelopeError> for SignedEventError {
    fn from(value: EventEnvelopeError) -> Self {
        Self::Envelope(value)
    }
}

impl SignedEvent {
    pub fn new(parts: SignedEventParts) -> Result<Self, SignedEventError> {
        let id = EventId::parse(parts.id)
            .map_err(EventEnvelopeError::InvalidId)?
            .into_string();
        let pubkey = parse_public_key(parts.pubkey)
            .map_err(EventEnvelopeError::InvalidAuthor)?
            .to_hex();
        let sig = EventSignature::parse(parts.sig)
            .map_err(EventEnvelopeError::InvalidSignature)?
            .into_string();
        let wire = Nip01EventWire {
            id,
            pubkey,
            created_at: parts.created_at,
            kind: parts.kind,
            tags: parts.tags,
            content: parts.content,
            sig,
            extra: Default::default(),
        };
        Self::from_wire_verified_id(wire, parts.raw_json)
    }

    pub fn from_wire_verified_id(
        wire: Nip01EventWire,
        raw_json: impl Into<String>,
    ) -> Result<Self, SignedEventError> {
        wire.verify_id().map_err(SignedEventError::Wire)?;
        let raw_json = raw_json.into();
        let parsed =
            Nip01EventWire::parse_json(raw_json.as_str()).map_err(SignedEventError::RawJson)?;
        if parsed != wire {
            return Err(SignedEventError::RawJsonMismatch);
        }
        let envelope = wire
            .clone()
            .into_envelope_unchecked_id()
            .map_err(SignedEventError::Envelope)?;
        Ok(Self {
            envelope,
            wire,
            raw_json,
        })
    }

    #[cfg(test)]
    fn from_wire_unchecked(
        wire: Nip01EventWire,
        raw_json: impl Into<String>,
    ) -> Result<Self, SignedEventError> {
        let envelope = wire
            .clone()
            .into_envelope_unchecked_id()
            .map_err(SignedEventError::Envelope)?;
        Ok(Self {
            envelope,
            wire,
            raw_json: raw_json.into(),
        })
    }

    #[inline]
    pub fn envelope(&self) -> &EventEnvelope {
        &self.envelope
    }

    #[inline]
    pub fn wire(&self) -> &Nip01EventWire {
        &self.wire
    }

    #[inline]
    pub fn raw_json(&self) -> &str {
        self.raw_json.as_str()
    }

    #[inline]
    pub fn id(&self) -> &EventId {
        self.envelope.id()
    }

    /// Returns the canonical NIP-01 event-id encoding retained by the wire boundary.
    #[inline]
    pub fn id_str(&self) -> &str {
        self.wire.id.as_str()
    }

    #[inline]
    pub fn id_hex(&self) -> String {
        self.envelope.id().to_hex()
    }

    #[inline]
    pub fn pubkey(&self) -> &PublicKey {
        self.envelope.author()
    }

    #[inline]
    pub fn created_at(&self) -> u64 {
        self.envelope.created_at_u64()
    }

    #[inline]
    pub fn kind(&self) -> u32 {
        self.envelope.kind_u32()
    }

    pub fn tags_as_vec(&self) -> Vec<Vec<String>> {
        self.envelope.tags_as_vec()
    }

    #[inline]
    pub fn content(&self) -> &str {
        self.envelope.content()
    }

    #[inline]
    pub fn sig(&self) -> &EventSignature {
        self.envelope.sig()
    }

    /// Returns the canonical NIP-01 signature encoding retained by the wire boundary.
    #[inline]
    pub fn sig_str(&self) -> &str {
        self.wire.sig.as_str()
    }

    #[inline]
    pub fn signature_hex(&self) -> String {
        self.envelope.sig().to_hex()
    }
}

pub fn validate_signed_nostr_event_matches_draft(
    signed_event: &SignedEvent,
    draft: &EventDraft,
) -> Result<(), DraftError> {
    draft.validate_for_signing()?;
    if signed_event.pubkey() != draft.expected_pubkey() {
        return Err(DraftError::SignedEventPubkeyMismatch {
            expected_pubkey: draft.expected_pubkey().to_hex(),
            actual_pubkey: signed_event.pubkey().to_hex(),
        });
    }
    if signed_event.created_at() != draft.created_at_u64() {
        return Err(DraftError::SignedEventCreatedAtMismatch {
            expected_created_at: draft.created_at_u64(),
            actual_created_at: signed_event.created_at(),
        });
    }
    if signed_event.kind() != draft.kind_u32() {
        return Err(DraftError::SignedEventKindMismatch {
            expected_kind: draft.kind_u32(),
            actual_kind: signed_event.kind(),
        });
    }
    let signed_tags = signed_event.tags_as_vec();
    let draft_tags = draft.tags_as_vec();
    if signed_tags != draft_tags {
        return Err(DraftError::SignedEventTagsMismatch {
            expected_len: draft_tags.len(),
            actual_len: signed_tags.len(),
        });
    }
    if signed_event.content() != draft.content() {
        return Err(DraftError::SignedEventContentMismatch {
            expected_len: draft.content().len(),
            actual_len: signed_event.content().len(),
        });
    }
    if signed_event.id() != draft.expected_event_id() {
        return Err(DraftError::SignedEventIdMismatch {
            expected_event_id: draft.expected_event_id.to_hex(),
            actual_event_id: signed_event.id().to_hex(),
        });
    }
    let signed_pubkey = signed_event.pubkey().to_hex();
    let computed_event_id = compute_nip01_event_id_for_valid_pubkey(
        signed_pubkey.as_str(),
        draft.created_at_u64(),
        signed_event.kind(),
        &signed_tags,
        signed_event.content(),
    );
    if computed_event_id != *signed_event.id() {
        return Err(DraftError::SignedEventComputedIdMismatch {
            expected_event_id: signed_event.id().to_hex(),
            computed_event_id: computed_event_id.to_hex(),
        });
    }
    Ok(())
}

pub fn compute_nip01_event_id(
    pubkey: &str,
    created_at: u64,
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<EventId, DraftError> {
    parse_public_key(pubkey)?;
    Ok(compute_nip01_event_id_for_valid_pubkey(
        pubkey, created_at, kind, tags, content,
    ))
}

pub fn nip01_event_id_preimage(
    pubkey: &str,
    created_at: u64,
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<String, DraftError> {
    parse_public_key(pubkey)?;
    Ok(nip01_event_id_preimage_for_valid_pubkey(
        pubkey, created_at, kind, tags, content,
    ))
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn compute_nip01_event_id_for_valid_pubkey(
    pubkey: &str,
    created_at: u64,
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> EventId {
    compute_canonical_nip01_event_id(pubkey, created_at, kind, tags, content)
        .expect("a validated public key always produces a canonical event id")
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn nip01_event_id_preimage_for_valid_pubkey(
    pubkey: &str,
    created_at: u64,
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> String {
    canonical_nip01_event_id_preimage(pubkey, created_at, kind, tags, content)
        .expect("a validated public key always produces a canonical preimage")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::envelope::kind::{
        KIND_COMMENT, KIND_DELETION_REQUEST, KIND_FARM_CRDT_CHANGE, KIND_GEOCHAT,
        KIND_KNOWLEDGE_CLAIM, KIND_KNOWLEDGE_SOURCE, KIND_POST, KIND_PROFILE,
    };

    fn hex_64(character: char) -> String {
        crate::test_valid_hex_64(character)
    }

    fn hex_128(character: char) -> String {
        core::iter::repeat_n(character, 128).collect()
    }

    fn raw_json_for_wire(wire: &Nip01EventWire) -> String {
        serde_json::to_string(&serde_json::json!({
            "id": wire.id,
            "pubkey": wire.pubkey,
            "created_at": wire.created_at,
            "kind": wire.kind,
            "tags": wire.tags,
            "content": wire.content,
            "sig": wire.sig,
        }))
        .expect("raw json")
    }

    fn verified_wire(
        pubkey: String,
        created_at: u64,
        kind: u32,
        tags: Vec<Vec<String>>,
        content: String,
        sig: String,
    ) -> Nip01EventWire {
        let id = compute_canonical_nip01_event_id(
            pubkey.as_str(),
            created_at,
            kind,
            &tags,
            content.as_str(),
        )
        .expect("event id")
        .into_string();
        Nip01EventWire {
            id,
            pubkey,
            created_at,
            kind,
            tags,
            content,
            sig,
            extra: Default::default(),
        }
    }

    fn unchecked_wire(
        id: String,
        pubkey: String,
        created_at: u64,
        kind: u32,
        tags: Vec<Vec<String>>,
        content: String,
        sig: String,
    ) -> Nip01EventWire {
        Nip01EventWire {
            id,
            pubkey,
            created_at,
            kind,
            tags,
            content,
            sig,
            extra: Default::default(),
        }
    }

    fn signed_event_for_draft(draft: &EventDraft) -> SignedEvent {
        let wire = verified_wire(
            draft.expected_pubkey().to_hex(),
            draft.created_at_u64(),
            draft.kind_u32(),
            draft.tags_as_vec(),
            draft.content().to_string(),
            hex_128('b'),
        );
        let raw_json = raw_json_for_wire(&wire);
        SignedEvent::from_wire_verified_id(wire, raw_json).expect("signed event")
    }

    fn generic_draft() -> EventDraft {
        EventDraft::new(
            "radroots.social.geochat.v1",
            KIND_GEOCHAT,
            1_700_000_000,
            Vec::new(),
            "hello",
            "a".repeat(64),
        )
        .expect("draft")
    }

    fn claim_content() -> &'static str {
        r#"{"schema":"radroots.knowledge.claim.v1","schema_version":1}"#
    }

    #[test]
    fn draft_computes_expected_event_id() {
        let draft = EventDraft::new(
            "radroots.social.geochat.v1",
            KIND_GEOCHAT,
            1_700_000_000,
            Vec::new(),
            "hello",
            hex_64('a'),
        )
        .expect("draft");

        assert_eq!(
            draft.nip01_preimage().expect("preimage"),
            "[0,\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",1700000000,20000,[],\"hello\"]"
        );
        assert_eq!(
            draft.expected_event_id_hex(),
            "07643222c33091b20114d5baf1a32288e808177eac7d87acb2c4f610363a2a7d"
        );
    }

    #[test]
    fn deterministic_event_id_changes_when_preimage_changes() {
        let tags = vec![vec!["t".to_owned(), "soil".to_owned()]];
        let base = compute_nip01_event_id(hex_64('a').as_str(), 1, KIND_POST, &tags, "hello")
            .expect("base");
        let pubkey_changed =
            compute_nip01_event_id(hex_64('b').as_str(), 1, KIND_POST, &tags, "hello")
                .expect("pubkey");
        let time_changed =
            compute_nip01_event_id(hex_64('a').as_str(), 2, KIND_POST, &tags, "hello")
                .expect("time");
        let kind_changed =
            compute_nip01_event_id(hex_64('a').as_str(), 1, KIND_PROFILE, &tags, "hello")
                .expect("kind");
        let tag_order_changed = compute_nip01_event_id(
            hex_64('a').as_str(),
            1,
            KIND_POST,
            &[
                vec!["p".to_owned(), hex_64('c')],
                vec!["t".to_owned(), "soil".to_owned()],
            ],
            "hello",
        )
        .expect("tag order");
        let content_changed =
            compute_nip01_event_id(hex_64('a').as_str(), 1, KIND_POST, &tags, "hello!")
                .expect("content");

        assert_ne!(base, pubkey_changed);
        assert_ne!(base, time_changed);
        assert_ne!(base, kind_changed);
        assert_ne!(base, tag_order_changed);
        assert_ne!(base, content_changed);
    }

    #[test]
    fn profile_golden_event_id_is_stable() {
        let event_id = compute_nip01_event_id(hex_64('c').as_str(), 1_700_000_001, 0, &[], "{}")
            .expect("event id");

        assert_eq!(
            event_id.to_hex(),
            "679ee570a933961d4f5ee95ed60cbc34d85c9c24a8da5e92f6036462ee0fc852"
        );
    }

    #[test]
    fn draft_constructor_rejects_unknown_contract_and_kind_mismatch() {
        let unknown = EventDraft::new("missing", KIND_POST, 1, Vec::new(), "", hex_64('a'))
            .expect_err("unknown contract");
        assert!(matches!(unknown, DraftError::UnknownContract(_)));

        let mismatch = EventDraft::new(
            "radroots.social.post.v1",
            KIND_PROFILE,
            1,
            Vec::new(),
            "",
            hex_64('a'),
        )
        .expect_err("kind mismatch");
        assert!(matches!(mismatch, DraftError::ContractKindMismatch { .. }));

        let invalid_pubkey = EventDraft::new(
            "radroots.social.geochat.v1",
            KIND_GEOCHAT,
            1,
            Vec::new(),
            "",
            "not-hex",
        )
        .expect_err("invalid pubkey");
        assert!(matches!(invalid_pubkey, DraftError::IdParse(_)));
    }

    #[test]
    fn draft_constructor_rejects_read_only_and_typed_only_contracts() {
        for (contract_id, kind) in [
            ("radroots.profile.metadata.v1", KIND_PROFILE),
            ("radroots.social.post.v1", KIND_POST),
            ("radroots.social.update.v1", KIND_POST),
            ("radroots.social.photo_update.v1", KIND_POST),
            ("radroots.social.ask.v1", KIND_POST),
            ("radroots.social.reply.v1", KIND_POST),
            ("radroots.social.deletion_request.v1", KIND_DELETION_REQUEST),
            ("radroots.social.comment.v1", KIND_COMMENT),
        ] {
            let error = EventDraft::new(contract_id, kind, 1, Vec::new(), "hello", hex_64('a'))
                .expect_err("governed typed or read-only contract must reject generic drafts");
            assert_eq!(
                error,
                DraftError::ContractNotDraftAuthorable {
                    contract_id: contract_id.to_owned(),
                }
            );
        }
    }

    #[test]
    fn draft_deserialization_revalidates_registry_policy_shape_and_event_id() {
        let draft = generic_draft();
        let value = serde_json::to_value(&draft).expect("draft json");
        let decoded: EventDraft =
            serde_json::from_value(value.clone()).expect("validated roundtrip");
        assert_eq!(decoded, draft);

        for stale_version in 1..RADROOTS_EVENT_CONTRACT_REGISTRY_VERSION {
            let mut tampered = value.clone();
            tampered["contract_registry_version"] = serde_json::json!(stale_version);
            let error = serde_json::from_value::<EventDraft>(tampered)
                .expect_err("stale registry generation must fail");
            assert!(error.to_string().contains("registry version mismatch"));
        }

        let mut tampered = value.clone();
        tampered["expected_event_id"] = serde_json::Value::String(hex_64('f'));
        let error = serde_json::from_value::<EventDraft>(tampered)
            .expect_err("tampered deterministic ID must fail");
        assert!(error.to_string().contains("frozen draft event ID mismatch"));

        let mut tampered = value.clone();
        tampered["contract_id"] = serde_json::Value::String("missing".to_owned());
        let error = serde_json::from_value::<EventDraft>(tampered)
            .expect_err("unknown persisted contract must fail");
        assert!(error.to_string().contains("unknown event contract"));

        let mut tampered = value.clone();
        tampered["kind"] = serde_json::json!(KIND_PROFILE);
        let error = serde_json::from_value::<EventDraft>(tampered)
            .expect_err("persisted contract kind mismatch must fail");
        assert!(error.to_string().contains("expects kind"));

        let mut tampered = value.clone();
        tampered["contract_id"] = serde_json::Value::String("radroots.social.post.v1".to_owned());
        tampered["kind"] = serde_json::json!(KIND_POST);
        let error = serde_json::from_value::<EventDraft>(tampered)
            .expect_err("read-only persisted contract must fail");
        assert!(error.to_string().contains("not authorable"));

        let json_draft = EventDraft::new(
            "radroots.farm.crdt_change.v1",
            KIND_FARM_CRDT_CHANGE,
            1,
            Vec::new(),
            "{}",
            hex_64('a'),
        )
        .expect("generic JSON draft");
        let mut tampered = serde_json::to_value(json_draft).expect("JSON draft value");
        tampered["content"] = serde_json::Value::String("not-json".to_owned());
        let error = serde_json::from_value::<EventDraft>(tampered)
            .expect_err("persisted contract shape mismatch must fail");
        assert!(error.to_string().contains("shape validation failed"));

        let mut tampered = value;
        tampered["unexpected"] = serde_json::json!(true);
        let error = serde_json::from_value::<EventDraft>(tampered)
            .expect_err("unknown persisted draft field must fail");
        assert!(error.to_string().contains("unknown field"));
    }

    #[test]
    fn draft_constructor_rejects_contract_shape_errors() {
        let missing_contract = EventDraft::new(
            "radroots.knowledge.claim.v1",
            KIND_KNOWLEDGE_CLAIM,
            1,
            Vec::new(),
            claim_content(),
            hex_64('a'),
        )
        .expect_err("missing contract tag");
        assert!(matches!(
            missing_contract,
            DraftError::ContractShape {
                error: ContractValidationError::MissingTag {
                    name: "contract",
                    ..
                },
                ..
            }
        ));

        let invalid_event_pointer = EventDraft::new(
            "radroots.knowledge.claim.v1",
            KIND_KNOWLEDGE_CLAIM,
            1,
            vec![
                vec![
                    "contract".to_owned(),
                    "radroots.knowledge.claim.v1".to_owned(),
                ],
                vec![
                    "source".to_owned(),
                    "not-hex".to_owned(),
                    hex_64('a'),
                    KIND_KNOWLEDGE_SOURCE.to_string(),
                    String::new(),
                ],
            ],
            claim_content(),
            hex_64('a'),
        )
        .expect_err("invalid event pointer");
        assert!(matches!(
            invalid_event_pointer,
            DraftError::ContractShape {
                error: ContractValidationError::TagValueMismatch { name: "source", .. },
                ..
            }
        ));

        let invalid_relay = EventDraft::new(
            "radroots.knowledge.claim.v1",
            KIND_KNOWLEDGE_CLAIM,
            1,
            vec![
                vec![
                    "contract".to_owned(),
                    "radroots.knowledge.claim.v1".to_owned(),
                ],
                vec![
                    "source".to_owned(),
                    hex_64('b'),
                    hex_64('a'),
                    KIND_KNOWLEDGE_SOURCE.to_string(),
                    String::new(),
                    "http://relay.radroots.example".to_owned(),
                ],
            ],
            claim_content(),
            hex_64('a'),
        )
        .expect_err("invalid event pointer relay");
        assert!(matches!(
            invalid_relay,
            DraftError::ContractShape {
                error: ContractValidationError::TagValueMismatch { name: "source", .. },
                ..
            }
        ));

        let invalid_json = EventDraft::new(
            "radroots.knowledge.claim.v1",
            KIND_KNOWLEDGE_CLAIM,
            1,
            vec![vec![
                "contract".to_owned(),
                "radroots.knowledge.claim.v1".to_owned(),
            ]],
            "not-json",
            hex_64('a'),
        )
        .expect_err("invalid json");
        assert!(matches!(
            invalid_json,
            DraftError::ContractShape {
                error: ContractValidationError::InvalidJsonContent { .. },
                ..
            }
        ));
    }

    #[test]
    fn signed_event_validates_ids_and_roundtrips_with_serde() {
        let wire = verified_wire(
            hex_64('e'),
            10,
            KIND_POST,
            vec![vec!["t".to_owned(), "soil".to_owned()]],
            "hello".to_owned(),
            hex_128('f'),
        );
        let raw_json = raw_json_for_wire(&wire);
        let signed = SignedEvent::new(SignedEventParts {
            id: wire.id.clone(),
            pubkey: wire.pubkey.clone(),
            created_at: wire.created_at,
            kind: wire.kind,
            tags: wire.tags.clone(),
            content: wire.content.clone(),
            sig: wire.sig.clone(),
            raw_json: raw_json.clone(),
        })
        .expect("signed event");
        let json = serde_json::to_string(&signed).expect("serialize");
        let decoded: SignedEvent = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(decoded, signed);
        assert_eq!(decoded.envelope().id_hex(), decoded.id_hex());
        assert_eq!(decoded.wire().id, decoded.id_hex());
        assert_eq!(decoded.id().to_hex(), decoded.id_hex());
        assert_eq!(decoded.pubkey().to_hex(), hex_64('e'));
        assert_eq!(decoded.created_at(), 10);
        assert_eq!(decoded.kind(), KIND_POST);
        assert_eq!(decoded.tags_as_vec(), wire.tags);
        assert_eq!(decoded.content(), "hello");
        assert_eq!(decoded.sig().to_hex(), decoded.signature_hex());
        assert_eq!(decoded.raw_json(), raw_json);
    }

    #[test]
    fn signed_event_constructors_validate_wire_and_raw_json() {
        let wire = verified_wire(
            hex_64('2'),
            42,
            KIND_POST,
            vec![vec!["t".to_owned(), "soil".to_owned()]],
            "hello".to_owned(),
            hex_128('3'),
        );
        let raw_json = raw_json_for_wire(&wire);
        let signed = SignedEvent::from_wire_verified_id(wire.clone(), raw_json.clone())
            .expect("signed event");

        assert_eq!(signed.id_hex(), wire.id);
        assert_eq!(signed.pubkey().to_hex(), hex_64('2'));
        assert_eq!(signed.signature_hex(), hex_128('3'));
        assert_eq!(signed.raw_json(), raw_json);

        let invalid = SignedEvent::new(SignedEventParts {
            id: "not-hex".to_owned(),
            pubkey: hex_64('e'),
            created_at: 10,
            kind: KIND_POST,
            tags: Vec::new(),
            content: String::new(),
            sig: "f".repeat(128),
            raw_json: "{}".to_owned(),
        })
        .expect_err("invalid id");
        assert!(matches!(
            invalid,
            SignedEventError::Envelope(EventEnvelopeError::InvalidId(_))
        ));

        let invalid = SignedEvent::new(SignedEventParts {
            id: hex_64('d'),
            pubkey: "not-hex".to_owned(),
            created_at: 10,
            kind: KIND_POST,
            tags: Vec::new(),
            content: String::new(),
            sig: "f".repeat(128),
            raw_json: "{}".to_owned(),
        })
        .expect_err("invalid pubkey");
        assert!(matches!(
            invalid,
            SignedEventError::Envelope(EventEnvelopeError::InvalidAuthor(_))
        ));

        let invalid = SignedEvent::new(SignedEventParts {
            id: hex_64('d'),
            pubkey: hex_64('e'),
            created_at: 10,
            kind: KIND_POST,
            tags: Vec::new(),
            content: String::new(),
            sig: "not-hex".to_owned(),
            raw_json: "{}".to_owned(),
        })
        .expect_err("invalid sig");
        assert!(matches!(
            invalid,
            SignedEventError::Envelope(EventEnvelopeError::InvalidSignature(_))
        ));

        let mismatched_raw =
            SignedEvent::from_wire_verified_id(wire, "{}").expect_err("raw mismatch");
        assert!(matches!(mismatched_raw, SignedEventError::RawJson(_)));
    }

    #[test]
    fn signed_event_validation_accepts_exact_draft_match() {
        let draft = generic_draft();
        let signed = signed_event_for_draft(&draft);

        validate_signed_nostr_event_matches_draft(&signed, &draft).expect("valid signed event");
    }

    #[test]
    fn signed_event_validation_rejects_draft_mismatches() {
        let draft = generic_draft();

        let signed = SignedEvent::from_wire_unchecked(
            unchecked_wire(
                draft.expected_event_id_hex(),
                hex_64('c'),
                draft.created_at_u64(),
                draft.kind_u32(),
                draft.tags_as_vec(),
                draft.content().to_string(),
                hex_128('b'),
            ),
            "{}",
        )
        .expect("unchecked signed event");
        let error =
            validate_signed_nostr_event_matches_draft(&signed, &draft).expect_err("mismatch");
        assert!(matches!(
            error,
            DraftError::SignedEventPubkeyMismatch { .. }
        ));

        let signed = SignedEvent::from_wire_unchecked(
            unchecked_wire(
                hex_64('d'),
                draft.expected_pubkey().to_hex(),
                draft.created_at_u64(),
                draft.kind_u32(),
                draft.tags_as_vec(),
                draft.content().to_string(),
                hex_128('b'),
            ),
            "{}",
        )
        .expect("unchecked signed event");
        let error =
            validate_signed_nostr_event_matches_draft(&signed, &draft).expect_err("mismatch");
        assert!(matches!(error, DraftError::SignedEventIdMismatch { .. }));

        let signed = SignedEvent::from_wire_unchecked(
            unchecked_wire(
                draft.expected_event_id_hex(),
                draft.expected_pubkey().to_hex(),
                draft.created_at_u64() + 1,
                draft.kind_u32(),
                draft.tags_as_vec(),
                draft.content().to_string(),
                hex_128('b'),
            ),
            "{}",
        )
        .expect("unchecked signed event");
        let error =
            validate_signed_nostr_event_matches_draft(&signed, &draft).expect_err("mismatch");
        assert!(matches!(
            error,
            DraftError::SignedEventCreatedAtMismatch { .. }
        ));

        let signed = SignedEvent::from_wire_unchecked(
            unchecked_wire(
                draft.expected_event_id_hex(),
                draft.expected_pubkey().to_hex(),
                draft.created_at_u64(),
                KIND_PROFILE,
                draft.tags_as_vec(),
                draft.content().to_string(),
                hex_128('b'),
            ),
            "{}",
        )
        .expect("unchecked signed event");
        let error =
            validate_signed_nostr_event_matches_draft(&signed, &draft).expect_err("mismatch");
        assert!(matches!(error, DraftError::SignedEventKindMismatch { .. }));

        let mut tags = draft.tags_as_vec();
        tags.push(vec!["p".to_owned(), hex_64('e')]);
        let signed = SignedEvent::from_wire_unchecked(
            unchecked_wire(
                draft.expected_event_id_hex(),
                draft.expected_pubkey().to_hex(),
                draft.created_at_u64(),
                draft.kind_u32(),
                tags,
                draft.content().to_string(),
                hex_128('b'),
            ),
            "{}",
        )
        .expect("unchecked signed event");
        let error =
            validate_signed_nostr_event_matches_draft(&signed, &draft).expect_err("mismatch");
        assert!(matches!(error, DraftError::SignedEventTagsMismatch { .. }));

        let signed = SignedEvent::from_wire_unchecked(
            unchecked_wire(
                draft.expected_event_id_hex(),
                draft.expected_pubkey().to_hex(),
                draft.created_at_u64(),
                draft.kind_u32(),
                draft.tags_as_vec(),
                "changed".to_owned(),
                hex_128('b'),
            ),
            "{}",
        )
        .expect("unchecked signed event");
        let error =
            validate_signed_nostr_event_matches_draft(&signed, &draft).expect_err("mismatch");
        assert!(matches!(
            error,
            DraftError::SignedEventContentMismatch { .. }
        ));
    }

    #[test]
    fn draft_errors_format_all_variants() {
        let errors = [
            DraftError::UnknownContract("missing".to_owned()),
            DraftError::ContractKindMismatch {
                contract_id: "radroots.social.post.v1".to_owned(),
                expected_kind: KIND_POST,
                actual_kind: KIND_PROFILE,
            },
            DraftError::ContractNotDraftAuthorable {
                contract_id: "radroots.social.post.v1".to_owned(),
            },
            DraftError::ContractRegistryVersionMismatch {
                expected: RADROOTS_EVENT_CONTRACT_REGISTRY_VERSION,
                actual: 1,
            },
            DraftError::DraftExpectedEventIdMismatch {
                expected_event_id: hex_64('a'),
                actual_event_id: hex_64('b'),
            },
            DraftError::ContractShape {
                contract_id: "radroots.knowledge.claim.v1".to_owned(),
                error: ContractValidationError::MissingTag {
                    contract_id: "radroots.knowledge.claim.v1",
                    name: "contract",
                },
            },
            DraftError::SignedEventPubkeyMismatch {
                expected_pubkey: hex_64('a'),
                actual_pubkey: hex_64('b'),
            },
            DraftError::SignedEventIdMismatch {
                expected_event_id: hex_64('c'),
                actual_event_id: hex_64('d'),
            },
            DraftError::SignedEventCreatedAtMismatch {
                expected_created_at: 1,
                actual_created_at: 2,
            },
            DraftError::SignedEventKindMismatch {
                expected_kind: KIND_POST,
                actual_kind: KIND_PROFILE,
            },
            DraftError::SignedEventTagsMismatch {
                expected_len: 1,
                actual_len: 2,
            },
            DraftError::SignedEventContentMismatch {
                expected_len: 5,
                actual_len: 7,
            },
            DraftError::SignedEventComputedIdMismatch {
                expected_event_id: hex_64('e'),
                computed_event_id: hex_64('f'),
            },
            DraftError::from(ParseError::Empty),
            DraftError::CanonicalEventId(CanonicalEventIdError::InvalidComputedEventId(
                ParseError::InvalidFormat,
            )),
            DraftError::from(EventEnvelopeError::NonCanonicalId),
            DraftError::from(SignedEventError::RawJsonMismatch),
        ];

        for error in errors {
            assert!(!error.to_string().is_empty());
        }

        assert!(
            DraftError::CanonicalEventId(CanonicalEventIdError::InvalidComputedEventId(
                ParseError::InvalidFormat,
            ),)
            .to_string()
            .contains("canonical event id digest")
        );

        assert!(matches!(
            DraftError::from(CanonicalEventIdError::InvalidPubkey(
                ParseError::InvalidFormat,
            )),
            DraftError::IdParse(_)
        ));
        assert!(matches!(
            DraftError::from(CanonicalEventIdError::InvalidComputedEventId(
                ParseError::InvalidFormat,
            )),
            DraftError::CanonicalEventId(_)
        ));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn draft_and_signed_event_accessors_expose_typed_state() {
        let draft = generic_draft();
        assert_eq!(draft.contract_id(), "radroots.social.geochat.v1");
        assert_eq!(
            draft.contract_registry_version(),
            RADROOTS_EVENT_CONTRACT_REGISTRY_VERSION
        );
        assert_eq!(draft.kind().as_u32(), draft.kind_u32());
        assert_eq!(draft.created_at().as_u64(), draft.created_at_u64());
        assert_eq!(draft.tags().to_vec(), draft.tags_as_vec());
        assert_eq!(draft.expected_pubkey().to_hex(), hex_64('a'));
        assert_eq!(
            draft.expected_event_id().to_hex(),
            draft.expected_event_id_hex()
        );

        let signed = signed_event_for_draft(&draft);
        assert_eq!(signed.envelope().id_hex(), signed.id_hex());
        assert_eq!(signed.wire().id, signed.id_hex());
        assert_eq!(signed.id().to_hex(), signed.id_hex());
        assert_eq!(signed.pubkey(), draft.expected_pubkey());
        assert_eq!(signed.sig().to_hex(), signed.signature_hex());

        for error in [
            SignedEventError::Wire(EventWireError::NonCanonicalIdentifier { field: "id" }),
            SignedEventError::RawJson(EventWireError::NonCanonicalIdentifier { field: "id" }),
            SignedEventError::RawJsonMismatch,
            SignedEventError::from(EventEnvelopeError::NonCanonicalId),
        ] {
            assert!(!error.to_string().is_empty());
        }

        let wire = signed.wire().clone();
        let mut different_wire = wire.clone();
        different_wire.content = "different".to_owned();
        different_wire.id = compute_canonical_nip01_event_id(
            different_wire.pubkey.as_str(),
            different_wire.created_at,
            different_wire.kind,
            &different_wire.tags,
            different_wire.content.as_str(),
        )
        .expect("different id")
        .into_string();
        let error = SignedEvent::from_wire_verified_id(wire, raw_json_for_wire(&different_wire))
            .expect_err("raw JSON mismatch");
        assert_eq!(error, SignedEventError::RawJsonMismatch);
    }

    #[test]
    fn event_id_computation_rejects_invalid_pubkeys() {
        let error =
            compute_nip01_event_id("not-hex", 1, KIND_POST, &[], "").expect_err("invalid pubkey");
        assert!(matches!(error, DraftError::IdParse(_)));
        let error = nip01_event_id_preimage("not-hex", 1, KIND_POST, &[], "")
            .expect_err("invalid preimage pubkey");
        assert!(matches!(error, DraftError::IdParse(_)));
    }
}
