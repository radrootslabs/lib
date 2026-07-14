#![forbid(unsafe_code)]

#[cfg(all(not(feature = "std"), not(test)))]
use alloc::{borrow::ToOwned, string::String, vec::Vec};

#[cfg(any(feature = "std", test))]
use std::{borrow::ToOwned, string::String, vec::Vec};

use crate::contract::{
    RADROOTS_EVENT_CONTRACT_REGISTRY_VERSION, RadrootsContractValidationError, event_contract,
    validate_event_contract_parts,
};
use crate::ids::{
    RadrootsEventId, RadrootsEventSignature, RadrootsIdParseError, RadrootsPublicKey,
};
use crate::wire::{
    RadrootsCanonicalEventIdError, RadrootsEventWireError, RadrootsNip01EventWire,
    canonical_nip01_event_id_preimage, compute_canonical_nip01_event_id,
};
use crate::{
    RadrootsEventEnvelope, RadrootsEventEnvelopeError, RadrootsEventKind, RadrootsEventTags,
    RadrootsEventTimestamp,
};
use core::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsDraftError {
    UnknownContract(String),
    ContractKindMismatch {
        contract_id: String,
        expected_kind: u32,
        actual_kind: u32,
    },
    ContractShape {
        contract_id: String,
        error: RadrootsContractValidationError,
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
    IdParse(RadrootsIdParseError),
    CanonicalEventId(RadrootsCanonicalEventIdError),
    Envelope(RadrootsEventEnvelopeError),
    SignedEvent(RadrootsSignedEventError),
}

impl fmt::Display for RadrootsDraftError {
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
impl std::error::Error for RadrootsDraftError {}

impl From<RadrootsIdParseError> for RadrootsDraftError {
    fn from(value: RadrootsIdParseError) -> Self {
        Self::IdParse(value)
    }
}

impl From<RadrootsCanonicalEventIdError> for RadrootsDraftError {
    fn from(value: RadrootsCanonicalEventIdError) -> Self {
        match value {
            RadrootsCanonicalEventIdError::InvalidPubkey(error) => Self::IdParse(error),
            error => Self::CanonicalEventId(error),
        }
    }
}

impl From<RadrootsEventEnvelopeError> for RadrootsDraftError {
    fn from(value: RadrootsEventEnvelopeError) -> Self {
        Self::Envelope(value)
    }
}

impl From<RadrootsSignedEventError> for RadrootsDraftError {
    fn from(value: RadrootsSignedEventError) -> Self {
        Self::SignedEvent(value)
    }
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsEventDraft {
    contract_id: String,
    contract_registry_version: u32,
    kind: RadrootsEventKind,
    created_at: RadrootsEventTimestamp,
    tags: RadrootsEventTags,
    content: String,
    expected_pubkey: RadrootsPublicKey,
    expected_event_id: RadrootsEventId,
}

impl RadrootsEventDraft {
    pub fn new(
        contract_id: impl Into<String>,
        kind: u32,
        created_at: u64,
        tags: Vec<Vec<String>>,
        content: impl Into<String>,
        expected_pubkey: impl AsRef<str>,
    ) -> Result<Self, RadrootsDraftError> {
        let contract_id = contract_id.into();
        let contract = match event_contract(&contract_id) {
            Some(contract) => contract,
            None => return Err(RadrootsDraftError::UnknownContract(contract_id.clone())),
        };
        if contract.kind != kind {
            return Err(RadrootsDraftError::ContractKindMismatch {
                contract_id,
                expected_kind: contract.kind,
                actual_kind: kind,
            });
        }
        let expected_pubkey = RadrootsPublicKey::parse(expected_pubkey.as_ref())?;
        let content = content.into();
        validate_event_contract_parts(kind, &tags, content.as_str(), contract.id).map_err(
            |error| RadrootsDraftError::ContractShape {
                contract_id: contract.id.to_owned(),
                error,
            },
        )?;
        let typed_tags = RadrootsEventTags::new(tags)?;
        let expected_event_id = compute_nip01_event_id(
            expected_pubkey.as_str(),
            created_at,
            kind,
            &typed_tags.to_vec(),
            &content,
        )?;
        Ok(Self {
            contract_id: contract.id.to_owned(),
            contract_registry_version: RADROOTS_EVENT_CONTRACT_REGISTRY_VERSION,
            kind: RadrootsEventKind::new(kind),
            created_at: RadrootsEventTimestamp::new(created_at),
            tags: typed_tags,
            content,
            expected_pubkey,
            expected_event_id,
        })
    }

    pub fn nip01_preimage(&self) -> Result<String, RadrootsDraftError> {
        nip01_event_id_preimage(
            self.expected_pubkey.as_str(),
            self.created_at.as_u64(),
            self.kind.as_u32(),
            &self.tags.to_vec(),
            self.content.as_str(),
        )
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
    pub fn kind(&self) -> RadrootsEventKind {
        self.kind
    }

    #[inline]
    pub fn kind_u32(&self) -> u32 {
        self.kind.as_u32()
    }

    #[inline]
    pub fn created_at(&self) -> RadrootsEventTimestamp {
        self.created_at
    }

    #[inline]
    pub fn created_at_u64(&self) -> u64 {
        self.created_at.as_u64()
    }

    #[inline]
    pub fn tags(&self) -> &RadrootsEventTags {
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
    pub fn expected_pubkey(&self) -> &RadrootsPublicKey {
        &self.expected_pubkey
    }

    #[inline]
    pub fn expected_pubkey_str(&self) -> &str {
        self.expected_pubkey.as_str()
    }

    #[inline]
    pub fn expected_event_id(&self) -> &RadrootsEventId {
        &self.expected_event_id
    }

    #[inline]
    pub fn expected_event_id_str(&self) -> &str {
        self.expected_event_id.as_str()
    }
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsSignedEventParts {
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
pub struct RadrootsSignedEvent {
    envelope: RadrootsEventEnvelope,
    wire: RadrootsNip01EventWire,
    raw_json: String,
}

#[cfg(any(feature = "serde", test))]
impl serde::Serialize for RadrootsSignedEvent {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;

        let mut state = serializer.serialize_struct("RadrootsSignedEvent", 2)?;
        state.serialize_field("wire", &self.wire)?;
        state.serialize_field("raw_json", &self.raw_json)?;
        state.end()
    }
}

#[cfg(any(feature = "serde", test))]
impl<'de> serde::Deserialize<'de> for RadrootsSignedEvent {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        struct SignedEventSerde {
            wire: RadrootsNip01EventWire,
            raw_json: String,
        }

        let value = SignedEventSerde::deserialize(deserializer)?;
        RadrootsSignedEvent::from_wire_verified_id(value.wire, value.raw_json)
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsSignedEventError {
    Wire(RadrootsEventWireError),
    RawJson(RadrootsEventWireError),
    RawJsonMismatch,
    Envelope(RadrootsEventEnvelopeError),
}

impl fmt::Display for RadrootsSignedEventError {
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
impl std::error::Error for RadrootsSignedEventError {}

impl From<RadrootsEventEnvelopeError> for RadrootsSignedEventError {
    fn from(value: RadrootsEventEnvelopeError) -> Self {
        Self::Envelope(value)
    }
}

#[cfg(feature = "signature")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsSignatureVerificationError {
    InvalidEventId,
    InvalidPubkey,
    InvalidSignature,
    VerificationFailed,
}

#[cfg(feature = "signature")]
impl fmt::Display for RadrootsSignatureVerificationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidEventId => write!(
                f,
                "signed event id cannot be decoded for signature verification"
            ),
            Self::InvalidPubkey => write!(
                f,
                "signed event pubkey cannot be decoded for signature verification"
            ),
            Self::InvalidSignature => write!(
                f,
                "signed event signature cannot be decoded for verification"
            ),
            Self::VerificationFailed => write!(f, "signed event signature verification failed"),
        }
    }
}

#[cfg(all(feature = "signature", feature = "std"))]
impl std::error::Error for RadrootsSignatureVerificationError {}

#[cfg(feature = "signature")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsVerifiedSignedEvent {
    signed_event: RadrootsSignedEvent,
}

impl RadrootsSignedEvent {
    pub fn new(parts: RadrootsSignedEventParts) -> Result<Self, RadrootsSignedEventError> {
        let id = RadrootsEventId::parse(parts.id)
            .map_err(RadrootsEventEnvelopeError::InvalidId)?
            .into_string();
        let pubkey = RadrootsPublicKey::parse(parts.pubkey)
            .map_err(RadrootsEventEnvelopeError::InvalidAuthor)?
            .into_string();
        let sig = RadrootsEventSignature::parse(parts.sig)
            .map_err(RadrootsEventEnvelopeError::InvalidSignature)?
            .into_string();
        let wire = RadrootsNip01EventWire {
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
        wire: RadrootsNip01EventWire,
        raw_json: impl Into<String>,
    ) -> Result<Self, RadrootsSignedEventError> {
        wire.verify_id().map_err(RadrootsSignedEventError::Wire)?;
        let raw_json = raw_json.into();
        let parsed = RadrootsNip01EventWire::parse_json(raw_json.as_str())
            .map_err(RadrootsSignedEventError::RawJson)?;
        if parsed != wire {
            return Err(RadrootsSignedEventError::RawJsonMismatch);
        }
        let envelope = wire
            .clone()
            .into_envelope_unchecked_id()
            .map_err(RadrootsSignedEventError::Envelope)?;
        Ok(Self {
            envelope,
            wire,
            raw_json,
        })
    }

    #[cfg(test)]
    fn from_wire_unchecked(
        wire: RadrootsNip01EventWire,
        raw_json: impl Into<String>,
    ) -> Result<Self, RadrootsSignedEventError> {
        let envelope = wire
            .clone()
            .into_envelope_unchecked_id()
            .map_err(RadrootsSignedEventError::Envelope)?;
        Ok(Self {
            envelope,
            wire,
            raw_json: raw_json.into(),
        })
    }

    #[inline]
    pub fn envelope(&self) -> &RadrootsEventEnvelope {
        &self.envelope
    }

    #[inline]
    pub fn wire(&self) -> &RadrootsNip01EventWire {
        &self.wire
    }

    #[inline]
    pub fn raw_json(&self) -> &str {
        self.raw_json.as_str()
    }

    #[inline]
    pub fn id(&self) -> &RadrootsEventId {
        self.envelope.id()
    }

    #[inline]
    pub fn id_str(&self) -> &str {
        self.envelope.id_str()
    }

    #[inline]
    pub fn pubkey(&self) -> &RadrootsPublicKey {
        self.envelope.author()
    }

    #[inline]
    pub fn pubkey_str(&self) -> &str {
        self.envelope.author_str()
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
    pub fn sig(&self) -> &RadrootsEventSignature {
        self.envelope.sig()
    }

    #[inline]
    pub fn sig_str(&self) -> &str {
        self.envelope.sig_str()
    }

    #[cfg(feature = "signature")]
    pub fn verify_signature(
        self,
    ) -> Result<RadrootsVerifiedSignedEvent, RadrootsSignatureVerificationError> {
        verify_bip340_signature(&self)?;
        Ok(RadrootsVerifiedSignedEvent { signed_event: self })
    }
}

#[cfg(feature = "signature")]
impl RadrootsVerifiedSignedEvent {
    #[inline]
    pub fn signed_event(&self) -> &RadrootsSignedEvent {
        &self.signed_event
    }

    #[inline]
    pub fn into_signed_event(self) -> RadrootsSignedEvent {
        self.signed_event
    }
}

pub fn validate_signed_nostr_event_matches_draft(
    signed_event: &RadrootsSignedEvent,
    draft: &RadrootsEventDraft,
) -> Result<(), RadrootsDraftError> {
    if signed_event.pubkey_str() != draft.expected_pubkey_str() {
        return Err(RadrootsDraftError::SignedEventPubkeyMismatch {
            expected_pubkey: draft.expected_pubkey_str().to_owned(),
            actual_pubkey: signed_event.pubkey_str().to_owned(),
        });
    }
    if signed_event.created_at() != draft.created_at_u64() {
        return Err(RadrootsDraftError::SignedEventCreatedAtMismatch {
            expected_created_at: draft.created_at_u64(),
            actual_created_at: signed_event.created_at(),
        });
    }
    if signed_event.kind() != draft.kind_u32() {
        return Err(RadrootsDraftError::SignedEventKindMismatch {
            expected_kind: draft.kind_u32(),
            actual_kind: signed_event.kind(),
        });
    }
    let signed_tags = signed_event.tags_as_vec();
    let draft_tags = draft.tags_as_vec();
    if signed_tags != draft_tags {
        return Err(RadrootsDraftError::SignedEventTagsMismatch {
            expected_len: draft_tags.len(),
            actual_len: signed_tags.len(),
        });
    }
    if signed_event.content() != draft.content() {
        return Err(RadrootsDraftError::SignedEventContentMismatch {
            expected_len: draft.content().len(),
            actual_len: signed_event.content().len(),
        });
    }
    if signed_event.id_str() != draft.expected_event_id_str() {
        return Err(RadrootsDraftError::SignedEventIdMismatch {
            expected_event_id: draft.expected_event_id_str().to_owned(),
            actual_event_id: signed_event.id_str().to_owned(),
        });
    }
    let computed_event_id = compute_nip01_event_id(
        signed_event.pubkey_str(),
        draft.created_at_u64(),
        signed_event.kind(),
        &signed_tags,
        signed_event.content(),
    )?
    .into_string();
    if computed_event_id.as_str() != signed_event.id_str() {
        return Err(RadrootsDraftError::SignedEventComputedIdMismatch {
            expected_event_id: signed_event.id_str().to_owned(),
            computed_event_id,
        });
    }
    Ok(())
}

#[cfg(feature = "signature")]
fn verify_bip340_signature(
    signed_event: &RadrootsSignedEvent,
) -> Result<(), RadrootsSignatureVerificationError> {
    use secp256k1::{Message, Secp256k1, XOnlyPublicKey, schnorr::Signature};

    let mut event_id = [0u8; 32];
    hex::decode_to_slice(signed_event.id_str(), &mut event_id)
        .map_err(|_| RadrootsSignatureVerificationError::InvalidEventId)?;
    let mut pubkey = [0u8; 32];
    hex::decode_to_slice(signed_event.pubkey_str(), &mut pubkey)
        .map_err(|_| RadrootsSignatureVerificationError::InvalidPubkey)?;
    let mut sig = [0u8; 64];
    hex::decode_to_slice(signed_event.sig_str(), &mut sig)
        .map_err(|_| RadrootsSignatureVerificationError::InvalidSignature)?;
    let message = Message::from_digest(event_id);
    let pubkey = XOnlyPublicKey::from_slice(&pubkey)
        .map_err(|_| RadrootsSignatureVerificationError::InvalidPubkey)?;
    let sig = Signature::from_slice(&sig)
        .map_err(|_| RadrootsSignatureVerificationError::InvalidSignature)?;
    Secp256k1::verification_only()
        .verify_schnorr(&sig, &message, &pubkey)
        .map_err(|_| RadrootsSignatureVerificationError::VerificationFailed)
}

pub fn compute_nip01_event_id(
    pubkey: &str,
    created_at: u64,
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<RadrootsEventId, RadrootsDraftError> {
    RadrootsPublicKey::parse(pubkey)?;
    Ok(compute_canonical_nip01_event_id(
        pubkey, created_at, kind, tags, content,
    )?)
}

pub fn nip01_event_id_preimage(
    pubkey: &str,
    created_at: u64,
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<String, RadrootsDraftError> {
    Ok(canonical_nip01_event_id_preimage(
        pubkey, created_at, kind, tags, content,
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kinds::{KIND_KNOWLEDGE_CLAIM, KIND_KNOWLEDGE_SOURCE, KIND_POST, KIND_PROFILE};

    fn hex_64(character: char) -> String {
        core::iter::repeat_n(character, 64).collect()
    }

    fn hex_128(character: char) -> String {
        core::iter::repeat_n(character, 128).collect()
    }

    fn raw_json_for_wire(wire: &RadrootsNip01EventWire) -> String {
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
    ) -> RadrootsNip01EventWire {
        let id = compute_canonical_nip01_event_id(
            pubkey.as_str(),
            created_at,
            kind,
            &tags,
            content.as_str(),
        )
        .expect("event id")
        .into_string();
        RadrootsNip01EventWire {
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
    ) -> RadrootsNip01EventWire {
        RadrootsNip01EventWire {
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

    fn signed_event_for_draft(draft: &RadrootsEventDraft) -> RadrootsSignedEvent {
        let wire = verified_wire(
            draft.expected_pubkey_str().to_string(),
            draft.created_at_u64(),
            draft.kind_u32(),
            draft.tags_as_vec(),
            draft.content().to_string(),
            hex_128('b'),
        );
        let raw_json = raw_json_for_wire(&wire);
        RadrootsSignedEvent::from_wire_verified_id(wire, raw_json).expect("signed event")
    }

    fn post_draft() -> RadrootsEventDraft {
        RadrootsEventDraft::new(
            "radroots.social.post.v1",
            KIND_POST,
            1_700_000_000,
            vec![vec!["t".to_owned(), "soil".to_owned()]],
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
        let draft = RadrootsEventDraft::new(
            "radroots.social.post.v1",
            KIND_POST,
            1_700_000_000,
            vec![
                vec!["t".to_owned(), "soil".to_owned()],
                vec!["p".to_owned(), hex_64('b')],
            ],
            "hello",
            hex_64('a'),
        )
        .expect("draft");

        assert_eq!(
            draft.nip01_preimage().expect("preimage"),
            "[0,\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",1700000000,1,[[\"t\",\"soil\"],[\"p\",\"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\"]],\"hello\"]"
        );
        assert_eq!(
            draft.expected_event_id_str(),
            "59d2486ef5557e0e317127de55005f2863361ad4041277ae523a869f2294cf9c"
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
            event_id.as_str(),
            "2a15e33622a155ae231b28bebe390869e67a0e228f77ecfcd652b1ce180a9dde"
        );
    }

    #[test]
    fn draft_constructor_rejects_unknown_contract_and_kind_mismatch() {
        let unknown = RadrootsEventDraft::new("missing", KIND_POST, 1, Vec::new(), "", hex_64('a'))
            .expect_err("unknown contract");
        assert!(matches!(unknown, RadrootsDraftError::UnknownContract(_)));

        let mismatch = RadrootsEventDraft::new(
            "radroots.social.post.v1",
            KIND_PROFILE,
            1,
            Vec::new(),
            "",
            hex_64('a'),
        )
        .expect_err("kind mismatch");
        assert!(matches!(
            mismatch,
            RadrootsDraftError::ContractKindMismatch { .. }
        ));

        let invalid_pubkey = RadrootsEventDraft::new(
            "radroots.social.post.v1",
            KIND_POST,
            1,
            Vec::new(),
            "",
            "not-hex",
        )
        .expect_err("invalid pubkey");
        assert!(matches!(invalid_pubkey, RadrootsDraftError::IdParse(_)));
    }

    #[test]
    fn draft_constructor_rejects_contract_shape_errors() {
        let missing_contract = RadrootsEventDraft::new(
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
            RadrootsDraftError::ContractShape {
                error: RadrootsContractValidationError::MissingTag {
                    name: "contract",
                    ..
                },
                ..
            }
        ));

        let invalid_event_pointer = RadrootsEventDraft::new(
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
            RadrootsDraftError::ContractShape {
                error: RadrootsContractValidationError::TagValueMismatch { name: "source", .. },
                ..
            }
        ));

        let invalid_relay = RadrootsEventDraft::new(
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
            RadrootsDraftError::ContractShape {
                error: RadrootsContractValidationError::TagValueMismatch { name: "source", .. },
                ..
            }
        ));

        let invalid_json = RadrootsEventDraft::new(
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
            RadrootsDraftError::ContractShape {
                error: RadrootsContractValidationError::InvalidJsonContent { .. },
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
        let signed = RadrootsSignedEvent::new(RadrootsSignedEventParts {
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
        let decoded: RadrootsSignedEvent = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(decoded, signed);
        assert_eq!(decoded.pubkey_str(), hex_64('e'));
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
        let signed = RadrootsSignedEvent::from_wire_verified_id(wire.clone(), raw_json.clone())
            .expect("signed event");

        assert_eq!(signed.id_str(), wire.id);
        assert_eq!(signed.pubkey_str(), hex_64('2'));
        assert_eq!(signed.sig_str(), hex_128('3'));
        assert_eq!(signed.raw_json(), raw_json);

        let invalid = RadrootsSignedEvent::new(RadrootsSignedEventParts {
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
            RadrootsSignedEventError::Envelope(RadrootsEventEnvelopeError::InvalidId(_))
        ));

        let invalid = RadrootsSignedEvent::new(RadrootsSignedEventParts {
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
            RadrootsSignedEventError::Envelope(RadrootsEventEnvelopeError::InvalidAuthor(_))
        ));

        let invalid = RadrootsSignedEvent::new(RadrootsSignedEventParts {
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
            RadrootsSignedEventError::Envelope(RadrootsEventEnvelopeError::InvalidSignature(_))
        ));

        let mismatched_raw =
            RadrootsSignedEvent::from_wire_verified_id(wire, "{}").expect_err("raw mismatch");
        assert!(matches!(
            mismatched_raw,
            RadrootsSignedEventError::RawJson(_)
        ));
    }

    #[test]
    fn signed_event_validation_accepts_exact_draft_match() {
        let draft = post_draft();
        let signed = signed_event_for_draft(&draft);

        validate_signed_nostr_event_matches_draft(&signed, &draft).expect("valid signed event");
    }

    #[test]
    fn signed_event_validation_rejects_draft_mismatches() {
        let draft = post_draft();

        let signed = RadrootsSignedEvent::from_wire_unchecked(
            unchecked_wire(
                draft.expected_event_id_str().to_string(),
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
            RadrootsDraftError::SignedEventPubkeyMismatch { .. }
        ));

        let signed = RadrootsSignedEvent::from_wire_unchecked(
            unchecked_wire(
                hex_64('d'),
                draft.expected_pubkey_str().to_string(),
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
            RadrootsDraftError::SignedEventIdMismatch { .. }
        ));

        let signed = RadrootsSignedEvent::from_wire_unchecked(
            unchecked_wire(
                draft.expected_event_id_str().to_string(),
                draft.expected_pubkey_str().to_string(),
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
            RadrootsDraftError::SignedEventCreatedAtMismatch { .. }
        ));

        let signed = RadrootsSignedEvent::from_wire_unchecked(
            unchecked_wire(
                draft.expected_event_id_str().to_string(),
                draft.expected_pubkey_str().to_string(),
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
        assert!(matches!(
            error,
            RadrootsDraftError::SignedEventKindMismatch { .. }
        ));

        let mut tags = draft.tags_as_vec();
        tags.push(vec!["p".to_owned(), hex_64('e')]);
        let signed = RadrootsSignedEvent::from_wire_unchecked(
            unchecked_wire(
                draft.expected_event_id_str().to_string(),
                draft.expected_pubkey_str().to_string(),
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
        assert!(matches!(
            error,
            RadrootsDraftError::SignedEventTagsMismatch { .. }
        ));

        let signed = RadrootsSignedEvent::from_wire_unchecked(
            unchecked_wire(
                draft.expected_event_id_str().to_string(),
                draft.expected_pubkey_str().to_string(),
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
            RadrootsDraftError::SignedEventContentMismatch { .. }
        ));

        let mut draft_value = serde_json::to_value(post_draft()).expect("draft json");
        draft_value["expected_event_id"] = serde_json::Value::String(hex_64('f'));
        let draft: RadrootsEventDraft =
            serde_json::from_value(draft_value).expect("tampered draft");
        let signed = RadrootsSignedEvent::from_wire_unchecked(
            unchecked_wire(
                draft.expected_event_id_str().to_string(),
                draft.expected_pubkey_str().to_string(),
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
            RadrootsDraftError::SignedEventComputedIdMismatch { .. }
        ));
    }

    #[test]
    fn draft_errors_format_all_variants() {
        let errors = [
            RadrootsDraftError::UnknownContract("missing".to_owned()),
            RadrootsDraftError::ContractKindMismatch {
                contract_id: "radroots.social.post.v1".to_owned(),
                expected_kind: KIND_POST,
                actual_kind: KIND_PROFILE,
            },
            RadrootsDraftError::ContractShape {
                contract_id: "radroots.knowledge.claim.v1".to_owned(),
                error: RadrootsContractValidationError::MissingTag {
                    contract_id: "radroots.knowledge.claim.v1",
                    name: "contract",
                },
            },
            RadrootsDraftError::SignedEventPubkeyMismatch {
                expected_pubkey: hex_64('a'),
                actual_pubkey: hex_64('b'),
            },
            RadrootsDraftError::SignedEventIdMismatch {
                expected_event_id: hex_64('c'),
                actual_event_id: hex_64('d'),
            },
            RadrootsDraftError::SignedEventCreatedAtMismatch {
                expected_created_at: 1,
                actual_created_at: 2,
            },
            RadrootsDraftError::SignedEventKindMismatch {
                expected_kind: KIND_POST,
                actual_kind: KIND_PROFILE,
            },
            RadrootsDraftError::SignedEventTagsMismatch {
                expected_len: 1,
                actual_len: 2,
            },
            RadrootsDraftError::SignedEventContentMismatch {
                expected_len: 5,
                actual_len: 7,
            },
            RadrootsDraftError::SignedEventComputedIdMismatch {
                expected_event_id: hex_64('e'),
                computed_event_id: hex_64('f'),
            },
            RadrootsDraftError::from(RadrootsIdParseError::Empty),
            RadrootsDraftError::CanonicalEventId(
                RadrootsCanonicalEventIdError::InvalidComputedEventId(
                    RadrootsIdParseError::InvalidFormat,
                ),
            ),
        ];

        for error in errors {
            assert!(!error.to_string().is_empty());
        }

        assert!(
            RadrootsDraftError::CanonicalEventId(
                RadrootsCanonicalEventIdError::InvalidComputedEventId(
                    RadrootsIdParseError::InvalidFormat,
                ),
            )
            .to_string()
            .contains("canonical event id digest")
        );
    }

    #[test]
    fn event_id_computation_rejects_invalid_pubkeys() {
        let error =
            compute_nip01_event_id("not-hex", 1, KIND_POST, &[], "").expect_err("invalid pubkey");
        assert!(matches!(error, RadrootsDraftError::IdParse(_)));
    }

    #[cfg(feature = "signature")]
    #[test]
    fn verified_signed_event_accepts_valid_bip340_signature() {
        use secp256k1::{Keypair, Message, Secp256k1, SecretKey};

        let secp = Secp256k1::new();
        let secret_key = SecretKey::from_slice(&[3u8; 32]).expect("secret key");
        let keypair = Keypair::from_secret_key(&secp, &secret_key);
        let (pubkey, _) = keypair.x_only_public_key();
        let pubkey = pubkey.to_string();
        let tags = vec![vec!["t".to_owned(), "soil".to_owned()]];
        let content = "hello".to_owned();
        let event_id = compute_canonical_nip01_event_id(
            pubkey.as_str(),
            1_700_000_000,
            KIND_POST,
            &tags,
            content.as_str(),
        )
        .expect("event id")
        .into_string();
        let mut event_id_bytes = [0u8; 32];
        hex::decode_to_slice(event_id.as_str(), &mut event_id_bytes).expect("event id bytes");
        let message = Message::from_digest(event_id_bytes);
        let sig = secp
            .sign_schnorr_no_aux_rand(&message, &keypair)
            .to_string();
        let wire = unchecked_wire(
            event_id,
            pubkey,
            1_700_000_000,
            KIND_POST,
            tags,
            content,
            sig,
        );
        let raw_json = raw_json_for_wire(&wire);
        let signed =
            RadrootsSignedEvent::from_wire_verified_id(wire, raw_json).expect("signed event");
        let verified = signed.verify_signature().expect("verified event");

        assert_eq!(verified.signed_event().kind(), KIND_POST);
    }

    #[cfg(feature = "signature")]
    #[test]
    fn verified_signed_event_rejects_invalid_bip340_signature() {
        use secp256k1::{Keypair, Secp256k1, SecretKey};

        let secp = Secp256k1::new();
        let secret_key = SecretKey::from_slice(&[3u8; 32]).expect("secret key");
        let keypair = Keypair::from_secret_key(&secp, &secret_key);
        let (pubkey, _) = keypair.x_only_public_key();
        let wire = verified_wire(
            pubkey.to_string(),
            1_700_000_000,
            KIND_POST,
            Vec::new(),
            "hello".to_owned(),
            hex_128('b'),
        );
        let raw_json = raw_json_for_wire(&wire);
        let signed =
            RadrootsSignedEvent::from_wire_verified_id(wire, raw_json).expect("signed event");
        let error = signed.verify_signature().expect_err("invalid signature");

        assert_eq!(
            error,
            RadrootsSignatureVerificationError::VerificationFailed
        );
    }
}
