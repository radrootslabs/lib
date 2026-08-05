#![forbid(unsafe_code)]

#[cfg(all(not(feature = "std"), not(test)))]
use alloc::{string::String, vec::Vec};

#[cfg(any(feature = "std", test))]
use std::{string::String, vec::Vec};

use radroots_identity::PublicKey;

use crate::contract::registry_v7::ContractValidationError;
use crate::envelope::{EventEnvelope, EventEnvelopeError};
use crate::id::{EventId, EventSignature, ParseError, parse_public_key};
use crate::wire::v1::{
    CanonicalEventIdError, EventWireError, Nip01EventWire, canonical_nip01_event_id_preimage,
    compute_canonical_nip01_event_id,
};
use core::fmt;

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
        crate::require_invariant(parsed == wire, &|| SignedEventError::RawJsonMismatch)?;
        let envelope = wire
            .clone()
            .into_unverified_envelope()
            .map_err(SignedEventError::Envelope)?;
        Ok(Self {
            envelope,
            wire,
            raw_json,
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
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use crate::envelope::kind::KIND_POST;

    fn hex_64(character: char) -> String {
        crate::test_valid_hex_64(character)
    }

    fn hex_128(character: char) -> String {
        core::iter::repeat_n(character, 128).collect()
    }

    fn verified_wire(content: &str) -> Nip01EventWire {
        let pubkey = hex_64('a');
        let id = compute_canonical_nip01_event_id(&pubkey, 1_700_000_000, KIND_POST, &[], content)
            .expect("event id")
            .into_string();
        Nip01EventWire {
            id,
            pubkey,
            created_at: 1_700_000_000,
            kind: KIND_POST,
            tags: Vec::new(),
            content: content.to_owned(),
            sig: hex_128('b'),
            extra: Default::default(),
        }
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
        .expect("raw JSON")
    }

    #[test]
    fn signed_event_preserves_verified_wire_and_typed_accessors() {
        let wire = verified_wire("hello");
        let raw_json = raw_json_for_wire(&wire);
        let signed = SignedEvent::from_wire_verified_id(wire.clone(), raw_json.clone())
            .expect("signed event");

        assert_eq!(signed.wire(), &wire);
        assert_eq!(signed.raw_json(), raw_json);
        assert_eq!(signed.envelope().id_hex(), signed.id_hex());
        assert_eq!(signed.id().to_hex(), signed.id_hex());
        assert_eq!(signed.pubkey().to_hex(), wire.pubkey);
        assert_eq!(signed.created_at(), wire.created_at);
        assert_eq!(signed.kind(), wire.kind);
        assert_eq!(signed.tags_as_vec(), wire.tags);
        assert_eq!(signed.content(), wire.content);
        assert_eq!(signed.sig().to_hex(), signed.signature_hex());
        assert_eq!(signed.sig_str(), wire.sig);
    }

    #[test]
    fn signed_event_rejects_noncanonical_or_mismatched_wire() {
        let wire = verified_wire("hello");
        let raw_json = raw_json_for_wire(&wire);
        let mut different_wire = verified_wire("different");
        let error =
            SignedEvent::from_wire_verified_id(wire.clone(), raw_json_for_wire(&different_wire))
                .expect_err("raw JSON mismatch");
        assert_eq!(error, SignedEventError::RawJsonMismatch);

        different_wire.id = hex_64('c');
        let error = SignedEvent::from_wire_verified_id(
            different_wire.clone(),
            raw_json_for_wire(&different_wire),
        )
        .expect_err("event id mismatch");
        assert!(matches!(error, SignedEventError::Wire(_)));

        let error = SignedEvent::new(SignedEventParts {
            id: "not-hex".to_owned(),
            pubkey: hex_64('a'),
            created_at: 1,
            kind: KIND_POST,
            tags: Vec::new(),
            content: String::new(),
            sig: hex_128('b'),
            raw_json,
        })
        .expect_err("invalid id");
        assert!(matches!(
            error,
            SignedEventError::Envelope(EventEnvelopeError::InvalidId(_))
        ));
    }

    #[test]
    fn event_id_helpers_are_canonical_and_reject_invalid_pubkeys() {
        let expected = "bb46df8e0d14e08773c7c6c88dfbb0925e6432048a2f2e82592afa415462d62a";
        let event_id = compute_nip01_event_id(&hex_64('a'), 1_700_000_000, KIND_POST, &[], "hello")
            .expect("event id");
        assert_eq!(event_id.to_hex(), expected);
        assert_eq!(
            nip01_event_id_preimage(&hex_64('a'), 1_700_000_000, KIND_POST, &[], "hello")
                .expect("preimage"),
            "[0,\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",1700000000,1,[],\"hello\"]"
        );

        assert!(matches!(
            compute_nip01_event_id("not-hex", 1, KIND_POST, &[], ""),
            Err(DraftError::IdParse(_))
        ));
        assert!(matches!(
            nip01_event_id_preimage("not-hex", 1, KIND_POST, &[], ""),
            Err(DraftError::IdParse(_))
        ));
    }

    #[test]
    fn errors_have_stable_nonempty_messages() {
        let errors = [
            DraftError::UnknownContract("missing".to_owned()),
            DraftError::ContractNotDraftAuthorable {
                contract_id: "typed-only".to_owned(),
            },
            DraftError::from(ParseError::Empty),
            DraftError::from(EventEnvelopeError::NonCanonicalId),
            DraftError::from(SignedEventError::RawJsonMismatch),
        ];
        for error in errors {
            assert!(!error.to_string().is_empty());
        }
    }
}
