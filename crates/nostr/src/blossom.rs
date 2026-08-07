//! Signed BUD-11 HTTP authorization value adapters.
//!
//! This module signs, encodes, authenticates, and validates authorization
//! values only. HTTP clients, endpoint operations, uploads, downloads,
//! retries, and runtime ownership remain outside this crate.

use core::fmt;

use alloc::{string::String, vec::Vec};
use base64::{
    Engine as _, alphabet,
    engine::general_purpose::{GeneralPurpose, NO_PAD, URL_SAFE_NO_PAD},
};
use radroots_blossom::{
    AuthorizationClaim, Error,
    authorization::{
        AuthoredUploadClaim, AuthorizationValidation, RADROOTS_BLOSSOM_AUTHORIZATION_EVENT_KIND,
        ValidatedAuthorizationClaim,
    },
};

use crate::types::{
    RadrootsNostrEvent, RadrootsNostrEventBuilderUnchecked, RadrootsNostrEventId,
    RadrootsNostrKeys, RadrootsNostrKind, RadrootsNostrPublicKey, RadrootsNostrTag,
    RadrootsNostrTagKind, RadrootsNostrTimestamp,
};

const AUTHORIZATION_SCHEME: &str = "Nostr ";
const PERMISSIVE_URL_SAFE_NO_PAD: GeneralPurpose = GeneralPurpose::new(
    &alphabet::URL_SAFE,
    NO_PAD.with_decode_allow_trailing_bits(true),
);

/// A kind-24242 event minted from a strict authored Blossom upload claim.
#[derive(Clone, PartialEq, Eq)]
pub struct SignedAuthorization {
    event: RadrootsNostrEvent,
}

impl fmt::Debug for SignedAuthorization {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SignedAuthorization")
            .field("event_id", &self.event.id)
            .field("author", &self.event.pubkey)
            .field("created_at", &self.event.created_at)
            .finish_non_exhaustive()
    }
}

impl SignedAuthorization {
    /// Converts an already verified exact signer result into an HTTP-only
    /// authorization value, rejecting every non-BUD-11 event.
    pub fn from_signed_event(
        signed_event: &radroots_event::SignedEvent,
    ) -> Result<Self, AuthorizationError> {
        if signed_event.kind() != u32::from(RADROOTS_BLOSSOM_AUTHORIZATION_EVENT_KIND) {
            return Err(AuthorizationError::InvalidEventKind {
                actual: u64::from(signed_event.kind()),
            });
        }
        let event: RadrootsNostrEvent = serde_json::from_str(signed_event.raw_json())
            .map_err(|_| AuthorizationError::InvalidEventJson)?;
        // `SignedEvent` has already verified that its retained wire ID matches
        // the canonical event preimage. Repeating that invariant here would
        // create an unreachable failure branch; the signature remains an
        // intentionally separate verification stage on that type.
        if !event.verify_signature() {
            return Err(AuthorizationError::InvalidEventSignature);
        }
        Ok(Self { event })
    }

    pub fn event_id(&self) -> RadrootsNostrEventId {
        self.event.id
    }

    pub fn author(&self) -> RadrootsNostrPublicKey {
        self.event.pubkey
    }

    pub fn created_at(&self) -> RadrootsNostrTimestamp {
        self.event.created_at
    }
}

/// A canonical BUD-11 HTTP `Authorization` value.
#[derive(Clone, PartialEq, Eq)]
pub struct AuthorizationHeader(String);

impl AuthorizationHeader {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Debug for AuthorizationHeader {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("AuthorizationHeader")
            .field(&"[REDACTED]")
            .finish()
    }
}

impl AsRef<str> for AuthorizationHeader {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

/// A signature-verified BUD-11 event whose pure claim policy also passed.
#[derive(Clone, PartialEq, Eq)]
pub struct VerifiedAuthorization {
    event: RadrootsNostrEvent,
    claim: ValidatedAuthorizationClaim,
}

impl fmt::Debug for VerifiedAuthorization {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedAuthorization")
            .field("event_id", &self.event.id)
            .field("author", &self.event.pubkey)
            .field("created_at", &self.event.created_at)
            .finish_non_exhaustive()
    }
}

impl VerifiedAuthorization {
    pub fn event_id(&self) -> RadrootsNostrEventId {
        self.event.id
    }

    pub fn author(&self) -> RadrootsNostrPublicKey {
        self.event.pubkey
    }

    pub fn created_at(&self) -> RadrootsNostrTimestamp {
        self.event.created_at
    }

    pub fn claim(&self) -> &ValidatedAuthorizationClaim {
        &self.claim
    }
}

/// Failures at the signed Blossom HTTP authorization boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum AuthorizationError {
    InvalidHeaderWhitespace,
    InvalidHeaderScheme,
    EmptyHeaderPayload,
    HeaderPaddingForbidden,
    InvalidHeaderBase64,
    NonCanonicalHeaderBase64,
    InvalidHeaderUtf8,
    InvalidEventJson,
    InvalidEventKind { actual: u64 },
    InvalidEventId,
    InvalidEventSignature,
    EventSigning,
    BlossomClaim(Error),
}

impl AuthorizationError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidHeaderWhitespace => "invalid_header_whitespace",
            Self::InvalidHeaderScheme => "invalid_header_scheme",
            Self::EmptyHeaderPayload => "empty_header_payload",
            Self::HeaderPaddingForbidden => "header_padding_forbidden",
            Self::InvalidHeaderBase64 => "invalid_header_base64",
            Self::NonCanonicalHeaderBase64 => "noncanonical_header_base64",
            Self::InvalidHeaderUtf8 => "invalid_header_utf8",
            Self::InvalidEventJson => "invalid_event_json",
            Self::InvalidEventKind { .. } => "invalid_event_kind",
            Self::InvalidEventId => "invalid_event_id",
            Self::InvalidEventSignature => "invalid_event_signature",
            Self::EventSigning => "event_signing",
            Self::BlossomClaim(error) => error.code(),
        }
    }

    pub fn blossom_claim_error(&self) -> Option<&Error> {
        match self {
            Self::BlossomClaim(error) => Some(error),
            _ => None,
        }
    }
}

impl fmt::Display for AuthorizationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidHeaderWhitespace => {
                formatter.write_str("invalid whitespace in Blossom authorization header")
            }
            Self::InvalidHeaderScheme => {
                formatter.write_str("Blossom authorization header must use the Nostr scheme")
            }
            Self::EmptyHeaderPayload => {
                formatter.write_str("Blossom authorization header payload is empty")
            }
            Self::HeaderPaddingForbidden => {
                formatter.write_str("Blossom authorization header Base64url padding is forbidden")
            }
            Self::InvalidHeaderBase64 => {
                formatter.write_str("invalid Blossom authorization header Base64url")
            }
            Self::NonCanonicalHeaderBase64 => {
                formatter.write_str("noncanonical Blossom authorization header Base64url")
            }
            Self::InvalidHeaderUtf8 => {
                formatter.write_str("Blossom authorization event JSON is not UTF-8")
            }
            Self::InvalidEventJson => {
                formatter.write_str("invalid Blossom authorization event JSON")
            }
            Self::InvalidEventKind { actual } => write!(
                formatter,
                "invalid Blossom authorization event kind {actual}"
            ),
            Self::InvalidEventId => formatter.write_str("invalid Blossom authorization event id"),
            Self::InvalidEventSignature => {
                formatter.write_str("invalid Blossom authorization event signature")
            }
            Self::EventSigning => formatter.write_str("failed to sign Blossom authorization event"),
            Self::BlossomClaim(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for AuthorizationError {}

impl From<Error> for AuthorizationError {
    fn from(error: Error) -> Self {
        Self::BlossomClaim(error)
    }
}

/// Sign a direct kind-24242 event from a strict authored BUD-11 upload claim.
pub fn sign_authorization(
    keys: &RadrootsNostrKeys,
    claim: &AuthoredUploadClaim,
) -> Result<SignedAuthorization, AuthorizationError> {
    let wire = claim.wire_parts();
    let tags = wire
        .tags()
        .iter()
        .map(|tag| {
            let (kind, values) = tag
                .split_first()
                .expect("authored Blossom wire tags always contain a kind");
            RadrootsNostrTag::custom(
                RadrootsNostrTagKind::custom(kind.as_str()),
                values.iter().cloned(),
            )
        })
        .collect::<Vec<_>>();
    finish_signed_event(
        RadrootsNostrEventBuilderUnchecked::new(
            RadrootsNostrKind::Custom(wire.kind()),
            wire.content(),
        )
        .tags(tags)
        .custom_created_at(RadrootsNostrTimestamp::from_secs(wire.created_at()))
        .sign_with_keys(keys),
    )
}

fn finish_signed_event<E>(
    result: Result<RadrootsNostrEvent, E>,
) -> Result<SignedAuthorization, AuthorizationError> {
    result
        .map(|event| SignedAuthorization { event })
        .map_err(|_| AuthorizationError::EventSigning)
}

/// Encode a signed BUD-11 event as a canonical `Nostr` authorization value.
pub fn encode_authorization_header(authorization: &SignedAuthorization) -> AuthorizationHeader {
    // `nostr::Event` contains no fallible serializer fields or non-string map keys.
    let json = serde_json::to_vec(&authorization.event)
        .expect("Nostr event JSON serialization is infallible");
    let payload = URL_SAFE_NO_PAD.encode(json);
    AuthorizationHeader(format!("{AUTHORIZATION_SCHEME}{payload}"))
}

/// Converts one verified generic signer receipt into a canonical BUD-11 HTTP
/// authorization value without exposing or accepting secret key material.
pub fn encode_signed_event_authorization_header(
    signed_event: &radroots_event::SignedEvent,
) -> Result<AuthorizationHeader, AuthorizationError> {
    SignedAuthorization::from_signed_event(signed_event)
        .map(|authorization| encode_authorization_header(&authorization))
}

/// Decode, authenticate, parse, and validate a BUD-11 authorization value.
pub fn decode_verify_authorization_header(
    header: &str,
    validation: &AuthorizationValidation,
) -> Result<VerifiedAuthorization, AuthorizationError> {
    if header.trim_start() != header {
        return Err(AuthorizationError::InvalidHeaderWhitespace);
    }
    let bytes = header.as_bytes();
    let Some(first_space) = bytes.iter().position(|byte| *byte == b' ') else {
        return Err(AuthorizationError::InvalidHeaderScheme);
    };
    if !bytes[..first_space].eq_ignore_ascii_case(b"Nostr") {
        return Err(AuthorizationError::InvalidHeaderScheme);
    }
    let payload_start = bytes[first_space..]
        .iter()
        .position(|byte| *byte != b' ')
        .map_or(bytes.len(), |offset| first_space + offset);
    let payload = &header[payload_start..];
    if payload.is_empty() {
        return Err(AuthorizationError::EmptyHeaderPayload);
    }
    if payload.chars().any(char::is_whitespace) {
        return Err(AuthorizationError::InvalidHeaderWhitespace);
    }
    if payload.contains('=') {
        return Err(AuthorizationError::HeaderPaddingForbidden);
    }
    let decoded = PERMISSIVE_URL_SAFE_NO_PAD
        .decode(payload)
        .map_err(|_| AuthorizationError::InvalidHeaderBase64)?;
    if URL_SAFE_NO_PAD.encode(&decoded) != payload {
        return Err(AuthorizationError::NonCanonicalHeaderBase64);
    }
    let json = String::from_utf8(decoded).map_err(|_| AuthorizationError::InvalidHeaderUtf8)?;
    validate_raw_event_json(&json)?;
    let event: RadrootsNostrEvent =
        serde_json::from_str(&json).map_err(|_| AuthorizationError::InvalidEventJson)?;
    if !event.verify_id() {
        return Err(AuthorizationError::InvalidEventId);
    }
    if !event.verify_signature() {
        return Err(AuthorizationError::InvalidEventSignature);
    }

    let tags: Vec<Vec<String>> = event
        .tags
        .iter()
        .map(|tag| tag.as_slice().to_vec())
        .collect();
    let claim = AuthorizationClaim::parse(&event.content, event.created_at.as_secs(), &tags)?
        .validate(validation)?;

    Ok(VerifiedAuthorization { event, claim })
}

fn validate_raw_event_json(json: &str) -> Result<(), AuthorizationError> {
    const EVENT_FIELDS: [&str; 7] = [
        "id",
        "pubkey",
        "created_at",
        "kind",
        "tags",
        "content",
        "sig",
    ];

    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|_| AuthorizationError::InvalidEventJson)?;
    let object = value
        .as_object()
        .ok_or(AuthorizationError::InvalidEventJson)?;
    if object.len() != EVENT_FIELDS.len()
        || !EVENT_FIELDS.iter().all(|field| object.contains_key(*field))
    {
        return Err(AuthorizationError::InvalidEventJson);
    }

    let actual_kind = object
        .get("kind")
        .and_then(serde_json::Value::as_u64)
        .ok_or(AuthorizationError::InvalidEventJson)?;
    if actual_kind != u64::from(RADROOTS_BLOSSOM_AUTHORIZATION_EVENT_KIND) {
        return Err(AuthorizationError::InvalidEventKind {
            actual: actual_kind,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blossom_signing_failure_maps_to_typed_adapter_error() {
        let result = finish_signed_event(Err::<RadrootsNostrEvent, ()>(()));
        assert_eq!(result, Err(AuthorizationError::EventSigning));
    }
}
