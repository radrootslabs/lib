//! Stateless convenience entrypoints for the canonical codec stages.

#[cfg(feature = "serde_json")]
use crate::{DecodeError, EncodeError, decode, encode};
use crate::{VerificationError, verify};
#[cfg(all(not(feature = "std"), feature = "serde_json"))]
use alloc::string::String;
#[cfg(all(feature = "std", feature = "serde_json"))]
use std::string::String;

/// Stateless convenience entrypoints for the canonical codec stages.
///
/// The methods preserve the same explicit typestate transitions as their
/// module-level counterparts; using `Codec` never combines or skips stages.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Codec;

impl Codec {
    /// Decodes compact NIP-01 JSON into an unverified event.
    #[cfg(feature = "serde_json")]
    pub fn decode_event(raw_json: &str) -> Result<verify::RawEvent, DecodeError> {
        decode::event(raw_json)
    }

    /// Encodes a native event envelope as compact NIP-01 JSON.
    #[cfg(feature = "serde_json")]
    pub fn encode_event(
        event: &radroots_event::envelope::EventEnvelope,
    ) -> Result<String, EncodeError> {
        encode::event(event)
    }

    /// Verifies an event's canonical identifier.
    pub fn verify_id(
        event: verify::RawEvent,
    ) -> Result<verify::IdVerifiedEvent, VerificationError> {
        verify::id(event)
    }

    /// Verifies an event signature through an explicit verifier.
    pub fn verify_signature<V>(
        event: verify::IdVerifiedEvent,
        verifier: &V,
    ) -> Result<verify::SignatureVerifiedEvent, VerificationError>
    where
        V: verify::SignatureVerifier + ?Sized,
    {
        verify::signature(event, verifier)
    }

    /// Validates an already signature-verified event contract.
    pub fn verify_contract(
        event: verify::SignatureVerifiedEvent,
    ) -> Result<verify::ContractValidatedEvent, VerificationError> {
        verify::contract(event)
    }
}
