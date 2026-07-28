//! Frozen NIP-01 verification-v1 semantics.

#[cfg(not(feature = "std"))]
use alloc::string::String;

use core::fmt;
#[cfg(feature = "nostr")]
use core::str::FromStr;

use radroots_event::contract::registry_v7::{
    RadrootsContractValidationError, RadrootsEventContract,
    validate_event_contract_registry_v7 as validate_radroots_event_contract_registry_v7,
};
use radroots_event::envelope::RadrootsEventEnvelope;
use radroots_event::wire::v1::compute_canonical_nip01_event_id_v1;

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
    verify_event_id_v1(event)
}

/// Verifies the canonical identifier with reconciliation-v1 semantics.
pub fn verify_event_id_v1(
    event: RadrootsEventEnvelope,
) -> Result<RadrootsIdVerifiedEvent, RadrootsNip01VerificationError> {
    u16::try_from(event.kind_u32()).map_err(|_| {
        RadrootsNip01VerificationError::KindOutOfRange {
            kind: event.kind_u32(),
        }
    })?;
    let expected = compute_canonical_nip01_event_id_v1(
        &event.author().to_hex(),
        event.created_at_u64(),
        event.kind_u32(),
        &event.tags_as_vec(),
        event.content(),
    )
    .map_err(|_| RadrootsNip01VerificationError::MalformedEnvelope)?;
    if event.id() != &expected {
        return Err(RadrootsNip01VerificationError::IdMismatch {
            expected: expected.to_hex(),
            actual: event.id_hex(),
        });
    }
    Ok(RadrootsIdVerifiedEvent { event })
}

#[cfg(feature = "nostr")]
pub fn verify_event_signature(
    event: RadrootsIdVerifiedEvent,
) -> Result<RadrootsSignatureVerifiedEvent, RadrootsNip01VerificationError> {
    verify_event_signature_v1(event)
}

/// Verifies the Schnorr signature with reconciliation-v1 semantics.
#[cfg(feature = "nostr")]
pub fn verify_event_signature_v1(
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
    event: RadrootsIdVerifiedEvent,
) -> Result<RadrootsSignatureVerifiedEvent, RadrootsNip01VerificationError> {
    verify_event_signature_v1(event)
}

/// Reports unavailable signature verification with reconciliation-v1 semantics.
#[cfg(not(feature = "nostr"))]
pub fn verify_event_signature_v1(
    _event: RadrootsIdVerifiedEvent,
) -> Result<RadrootsSignatureVerifiedEvent, RadrootsNip01VerificationError> {
    Err(RadrootsNip01VerificationError::SignatureVerificationUnavailable)
}

/// Verifies the canonical NIP-01 identifier and Schnorr signature in order.
pub fn verify_nip01_event(
    event: RadrootsEventEnvelope,
) -> Result<RadrootsSignatureVerifiedEvent, RadrootsNip01VerificationError> {
    verify_nip01_event_v1(event)
}

/// Verifies a NIP-01 event with the behavior frozen for reconciliation v1.
pub fn verify_nip01_event_v1(
    event: RadrootsEventEnvelope,
) -> Result<RadrootsSignatureVerifiedEvent, RadrootsNip01VerificationError> {
    verify_event_signature_v1(verify_event_id_v1(event)?)
}

/// Applies full registry contract-shape validation to an already verified event.
pub fn validate_event_contract(
    event: RadrootsSignatureVerifiedEvent,
) -> Result<RadrootsContractValidatedEvent, RadrootsContractValidationError> {
    validate_event_contract_registry_v7(event)
}

/// Applies the immutable registry-v7 contract-shape validation boundary.
pub fn validate_event_contract_registry_v7(
    event: RadrootsSignatureVerifiedEvent,
) -> Result<RadrootsContractValidatedEvent, RadrootsContractValidationError> {
    let contract = validate_radroots_event_contract_registry_v7(event.event())?;
    Ok(RadrootsContractValidatedEvent {
        verified_event: event,
        contract,
    })
}

#[cfg(feature = "nostr")]
fn raw_event_from_radroots(
    event: &RadrootsEventEnvelope,
) -> Result<nostr::Event, RadrootsNip01VerificationError> {
    let event_id = event.id_hex();
    let id = nostr::EventId::from_hex(event_id.as_str())
        .map_err(|_| RadrootsNip01VerificationError::MalformedEnvelope)?;
    let public_key = nostr::secp256k1::XOnlyPublicKey::from_str(&event.author().to_hex())
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
    let signature = event.signature_hex();
    let sig = nostr::secp256k1::schnorr::Signature::from_str(signature.as_str())
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
mod tests;
