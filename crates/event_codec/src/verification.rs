#[cfg(not(feature = "std"))]
use alloc::string::{String, ToString};

use core::fmt;
#[cfg(feature = "nostr")]
use core::str::FromStr;

use radroots_event::RadrootsEventEnvelope;
use radroots_event::contract::{
    RadrootsContractValidationError, RadrootsEventContract,
    validate_event_contract as validate_radroots_event_contract,
};
use radroots_event::ids::RadrootsEventId;
use radroots_event::wire::compute_canonical_nip01_event_id;

#[cfg(feature = "knowledge")]
pub use crate::knowledge::verification::{
    RadrootsDecodeError, RadrootsDecodedEvent, decode_validated_event,
    verify_and_decode_radroots_event,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsIdVerifiedEvent {
    event: RadrootsEventEnvelope,
}

impl RadrootsIdVerifiedEvent {
    pub fn event(&self) -> &RadrootsEventEnvelope {
        &self.event
    }

    pub fn into_event(self) -> RadrootsEventEnvelope {
        self.event
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsSignatureVerifiedEvent {
    event: RadrootsEventEnvelope,
}

impl RadrootsSignatureVerifiedEvent {
    pub fn event(&self) -> &RadrootsEventEnvelope {
        &self.event
    }

    pub fn into_event(self) -> RadrootsEventEnvelope {
        self.event
    }
}

/// A NIP-01 verified event whose registry-selected contract shape is valid.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsContractValidatedEvent {
    verified_event: RadrootsSignatureVerifiedEvent,
    contract: &'static RadrootsEventContract,
}

impl RadrootsContractValidatedEvent {
    pub fn verified_event(&self) -> &RadrootsSignatureVerifiedEvent {
        &self.verified_event
    }

    pub fn event(&self) -> &RadrootsEventEnvelope {
        self.verified_event.event()
    }

    pub fn contract(&self) -> &'static RadrootsEventContract {
        self.contract
    }

    pub fn contract_id(&self) -> &'static str {
        self.contract.id
    }

    pub fn into_verified_event(self) -> RadrootsSignatureVerifiedEvent {
        self.verified_event
    }

    pub fn into_event(self) -> RadrootsEventEnvelope {
        self.verified_event.into_event()
    }
}

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsNip01VerificationError {
    MalformedEnvelope,
    KindOutOfRange { kind: u32 },
    IdMismatch { expected: String, actual: String },
    SignatureInvalid,
    SignatureVerificationUnavailable,
}

impl RadrootsNip01VerificationError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::MalformedEnvelope => "malformed_envelope",
            Self::KindOutOfRange { .. } => "kind_out_of_range",
            Self::IdMismatch { .. } => "id_mismatch",
            Self::SignatureInvalid => "signature_invalid",
            Self::SignatureVerificationUnavailable => "signature_verification_unavailable",
        }
    }
}

impl fmt::Display for RadrootsNip01VerificationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedEnvelope => formatter.write_str("malformed NIP-01 event envelope"),
            Self::KindOutOfRange { kind } => {
                write!(formatter, "NIP-01 event kind {kind} exceeds {}", u16::MAX)
            }
            Self::IdMismatch { expected, actual } => {
                write!(
                    formatter,
                    "NIP-01 event id mismatch: expected {expected}, got {actual}"
                )
            }
            Self::SignatureInvalid => formatter.write_str("invalid NIP-01 event signature"),
            Self::SignatureVerificationUnavailable => {
                formatter.write_str("NIP-01 signature verification requires the nostr feature")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsNip01VerificationError {}

pub fn verify_event_id(
    event: RadrootsEventEnvelope,
) -> Result<RadrootsIdVerifiedEvent, RadrootsNip01VerificationError> {
    u16::try_from(event.kind_u32()).map_err(|_| {
        RadrootsNip01VerificationError::KindOutOfRange {
            kind: event.kind_u32(),
        }
    })?;
    RadrootsEventId::parse(event.id_str())
        .map_err(|_| RadrootsNip01VerificationError::MalformedEnvelope)?;
    let expected = compute_canonical_nip01_event_id(
        event.author_str(),
        event.created_at_u64(),
        event.kind_u32(),
        &event.tags_as_vec(),
        event.content(),
    )
    .map_err(|_| RadrootsNip01VerificationError::MalformedEnvelope)?
    .into_string();
    if event.id_str() != expected {
        return Err(RadrootsNip01VerificationError::IdMismatch {
            expected,
            actual: event.id_str().to_string(),
        });
    }
    Ok(RadrootsIdVerifiedEvent { event })
}

#[cfg(feature = "nostr")]
pub fn verify_event_signature(
    event: RadrootsIdVerifiedEvent,
) -> Result<RadrootsSignatureVerifiedEvent, RadrootsNip01VerificationError> {
    let raw_event = raw_event_from_radroots(&event.event)?;
    if raw_event.verify_signature() {
        Ok(RadrootsSignatureVerifiedEvent { event: event.event })
    } else {
        Err(RadrootsNip01VerificationError::SignatureInvalid)
    }
}

#[cfg(not(feature = "nostr"))]
pub fn verify_event_signature(
    _event: RadrootsIdVerifiedEvent,
) -> Result<RadrootsSignatureVerifiedEvent, RadrootsNip01VerificationError> {
    Err(RadrootsNip01VerificationError::SignatureVerificationUnavailable)
}

/// Verifies the canonical NIP-01 identifier and Schnorr signature in order.
pub fn verify_nip01_event(
    event: RadrootsEventEnvelope,
) -> Result<RadrootsSignatureVerifiedEvent, RadrootsNip01VerificationError> {
    verify_event_signature(verify_event_id(event)?)
}

/// Applies full registry contract-shape validation to an already verified event.
pub fn validate_event_contract(
    event: RadrootsSignatureVerifiedEvent,
) -> Result<RadrootsContractValidatedEvent, RadrootsContractValidationError> {
    let contract = validate_radroots_event_contract(event.event())?;
    Ok(RadrootsContractValidatedEvent {
        verified_event: event,
        contract,
    })
}

#[cfg(feature = "nostr")]
fn raw_event_from_radroots(
    event: &RadrootsEventEnvelope,
) -> Result<nostr::Event, RadrootsNip01VerificationError> {
    let id = nostr::EventId::from_hex(event.id_str())
        .map_err(|_| RadrootsNip01VerificationError::MalformedEnvelope)?;
    let public_key = nostr::secp256k1::XOnlyPublicKey::from_str(event.author_str())
        .map(nostr::PublicKey::from)
        .map_err(|_| RadrootsNip01VerificationError::MalformedEnvelope)?;
    let kind = u16::try_from(event.kind_u32()).map_err(|_| {
        RadrootsNip01VerificationError::KindOutOfRange {
            kind: event.kind_u32(),
        }
    })?;
    let tags_vec = event.tags_as_vec();
    let mut tags = Vec::with_capacity(tags_vec.len());
    for tag in tags_vec {
        tags.push(
            nostr::Tag::parse(tag)
                .map_err(|_| RadrootsNip01VerificationError::MalformedEnvelope)?,
        );
    }
    let sig = nostr::secp256k1::schnorr::Signature::from_str(event.sig_str())
        .map_err(|_| RadrootsNip01VerificationError::MalformedEnvelope)?;
    Ok(nostr::Event::new(
        id,
        public_key,
        nostr::Timestamp::from_secs(event.created_at_u64()),
        nostr::Kind::Custom(kind),
        tags,
        event.content().to_string(),
        sig,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use radroots_event::RadrootsEventEnvelopeParts;

    #[test]
    fn id_verification_returns_the_exact_envelope() {
        let event = signed_max_kind_event();
        let verified = verify_event_id(event.clone()).expect("canonical event id");

        assert_eq!(verified.event(), &event);
        assert_eq!(verified.into_event(), event);
    }

    #[cfg(feature = "nostr")]
    #[test]
    fn signature_verification_returns_the_exact_envelope() {
        let event = signed_max_kind_event();
        let verified = verify_nip01_event(event.clone()).expect("valid Schnorr signature");

        assert_eq!(verified.event(), &event);
        assert_eq!(verified.into_event(), event);
    }

    #[cfg(not(feature = "nostr"))]
    #[test]
    fn signature_verification_reports_unavailable_without_nostr() {
        let event = verify_event_id(signed_max_kind_event()).expect("canonical event id");

        assert_eq!(
            verify_event_signature(event),
            Err(RadrootsNip01VerificationError::SignatureVerificationUnavailable)
        );
    }

    #[test]
    fn verification_error_codes_are_stable() {
        let errors = [
            (
                RadrootsNip01VerificationError::MalformedEnvelope,
                "malformed_envelope",
            ),
            (
                RadrootsNip01VerificationError::KindOutOfRange { kind: 65_536 },
                "kind_out_of_range",
            ),
            (
                RadrootsNip01VerificationError::IdMismatch {
                    expected: "expected".to_string(),
                    actual: "actual".to_string(),
                },
                "id_mismatch",
            ),
            (
                RadrootsNip01VerificationError::SignatureInvalid,
                "signature_invalid",
            ),
            (
                RadrootsNip01VerificationError::SignatureVerificationUnavailable,
                "signature_verification_unavailable",
            ),
        ];

        for (error, expected) in errors {
            assert_eq!(error.code(), expected);
            assert!(!error.to_string().is_empty());
        }
    }

    #[test]
    fn id_verification_rejects_an_out_of_range_kind_before_hashing() {
        let original = signed_max_kind_event();
        let kind = u32::from(u16::MAX) + 1;
        let id = compute_canonical_nip01_event_id(
            original.author_str(),
            original.created_at_u64(),
            kind,
            &original.tags_as_vec(),
            original.content(),
        )
        .expect("canonical hash remains mechanically computable")
        .into_string();
        let event = RadrootsEventEnvelope::new(RadrootsEventEnvelopeParts {
            id,
            author: original.author_str().to_owned(),
            created_at: original.created_at_u64(),
            kind,
            tags: original.tags_as_vec(),
            content: original.content().to_owned(),
            sig: original.sig_str().to_owned(),
        })
        .expect("base envelope permits the wider internal kind representation");

        assert_eq!(
            verify_event_id(event),
            Err(RadrootsNip01VerificationError::KindOutOfRange { kind })
        );
    }

    fn signed_max_kind_event() -> RadrootsEventEnvelope {
        RadrootsEventEnvelope::new(RadrootsEventEnvelopeParts {
            id: "a07878757d705d3cd848b9264791d699069068a5f0a575112f351367b0987958"
                .to_string(),
            author: "1b84c5567b126440995d3ed5aaba0565d71e1834604819ff9c17f5e9d5dd078f"
                .to_string(),
            created_at: 1_800_000_104,
            kind: u32::from(u16::MAX),
            tags: Vec::new(),
            content: "maximum-kind".to_string(),
            sig: "d79b19843a0bfd769c02c73866d44a3a06f7b11e107a5257971b60e700aa25565802fd3a7eed4042fe8db7d709a465e5f61478eb8291178831bf48f6b0980671"
                .to_string(),
        })
        .expect("valid event envelope")
    }
}
