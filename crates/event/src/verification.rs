//! Native event verification typestates.
//!
//! The types in this module make the ordered NIP-01 validation stages explicit.
//! Their representations are private so a later stage can only be obtained by
//! consuming the immediately preceding stage and successfully enforcing its
//! invariant.

use core::fmt;

use crate::{
    contract::registry_v7::{
        RadrootsContractValidationError, RadrootsEventContract, validate_event_contract_registry_v7,
    },
    envelope::RadrootsEventEnvelope,
    id::RadrootsEventId,
    wire::compute_canonical_nip01_event_id,
};

/// A structurally valid event whose canonical identifier is not yet trusted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawEvent(RadrootsEventEnvelope);

impl RawEvent {
    #[must_use]
    pub const fn new(event: RadrootsEventEnvelope) -> Self {
        Self(event)
    }

    #[must_use]
    pub const fn event(&self) -> &RadrootsEventEnvelope {
        &self.0
    }

    #[must_use]
    pub fn into_event(self) -> RadrootsEventEnvelope {
        self.0
    }

    /// Recomputes and verifies the canonical NIP-01 event identifier.
    pub fn verify_id(self) -> Result<IdVerifiedEvent, Error> {
        let expected = compute_canonical_nip01_event_id(
            &self.0.author().to_hex(),
            self.0.created_at_u64(),
            self.0.kind_u32(),
            &self.0.tags_as_vec(),
            self.0.content(),
        )
        .map_err(|_| Error::MalformedEnvelope)?;
        let actual = *self.0.id();
        if actual != expected {
            return Err(Error::IdMismatch { expected, actual });
        }
        Ok(IdVerifiedEvent(self.0))
    }
}

/// An event whose declared identifier matches its canonical NIP-01 digest.
///
/// Its private representation rejects forged transitions:
///
/// ```compile_fail
/// use radroots_event::admission::{IdVerifiedEvent, RawEvent};
///
/// fn forge(raw: RawEvent) -> IdVerifiedEvent {
///     IdVerifiedEvent(raw.into_event())
/// }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdVerifiedEvent(RadrootsEventEnvelope);

impl IdVerifiedEvent {
    #[must_use]
    pub const fn event(&self) -> &RadrootsEventEnvelope {
        &self.0
    }

    #[must_use]
    pub fn into_event(self) -> RadrootsEventEnvelope {
        self.0
    }

    /// Verifies the BIP-340 signature over the already verified event ID.
    pub fn verify_signature(self) -> Result<SignatureVerifiedEvent, Error> {
        verify_signature(&self.0)?;
        Ok(SignatureVerifiedEvent(self.0))
    }
}

/// An event whose canonical identifier and BIP-340 signature are verified.
///
/// ```compile_fail
/// use radroots_event::admission::{IdVerifiedEvent, SignatureVerifiedEvent};
///
/// fn bypass_signature(event: IdVerifiedEvent) -> SignatureVerifiedEvent {
///     SignatureVerifiedEvent::new(event.into_event())
/// }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignatureVerifiedEvent(RadrootsEventEnvelope);

impl SignatureVerifiedEvent {
    #[must_use]
    pub const fn event(&self) -> &RadrootsEventEnvelope {
        &self.0
    }

    #[must_use]
    pub fn into_event(self) -> RadrootsEventEnvelope {
        self.0
    }

    /// Validates the event against the immutable registry-v7 contract set.
    pub fn validate_contract(self) -> Result<ContractValidatedEvent, Error> {
        let contract =
            validate_event_contract_registry_v7(&self.0).map_err(Error::ContractValidation)?;
        Ok(ContractValidatedEvent {
            event: self,
            contract,
        })
    }
}

/// A signature-verified event whose registry-selected contract is valid.
///
/// ```compile_fail
/// use radroots_event::admission::{ContractValidatedEvent, SignatureVerifiedEvent};
///
/// fn bypass_contract(event: SignatureVerifiedEvent) -> ContractValidatedEvent {
///     ContractValidatedEvent::from(event)
/// }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContractValidatedEvent {
    event: SignatureVerifiedEvent,
    contract: &'static RadrootsEventContract,
}

impl ContractValidatedEvent {
    #[must_use]
    pub const fn verified_event(&self) -> &SignatureVerifiedEvent {
        &self.event
    }

    #[must_use]
    pub const fn event(&self) -> &RadrootsEventEnvelope {
        self.event.event()
    }

    #[must_use]
    pub const fn contract(&self) -> &'static RadrootsEventContract {
        self.contract
    }

    #[must_use]
    pub const fn contract_id(&self) -> &'static str {
        self.contract.id
    }

    #[must_use]
    pub fn into_verified_event(self) -> SignatureVerifiedEvent {
        self.event
    }

    #[must_use]
    pub fn into_event(self) -> RadrootsEventEnvelope {
        self.event.into_event()
    }
}

/// A failure while advancing an event verification typestate.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    MalformedEnvelope,
    IdMismatch {
        expected: RadrootsEventId,
        actual: RadrootsEventId,
    },
    SignatureInvalid,
    SignatureVerificationUnavailable,
    ContractValidation(RadrootsContractValidationError),
}

impl Error {
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::MalformedEnvelope => "malformed_envelope",
            Self::IdMismatch { .. } => "id_mismatch",
            Self::SignatureInvalid => "signature_invalid",
            Self::SignatureVerificationUnavailable => "signature_verification_unavailable",
            Self::ContractValidation(_) => "contract_validation",
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedEnvelope => formatter.write_str("malformed NIP-01 event envelope"),
            Self::IdMismatch { expected, actual } => write!(
                formatter,
                "NIP-01 event id mismatch: expected {}, got {}",
                expected.to_hex(),
                actual.to_hex()
            ),
            Self::SignatureInvalid => formatter.write_str("invalid NIP-01 event signature"),
            Self::SignatureVerificationUnavailable => {
                formatter.write_str("NIP-01 signature verification is unavailable")
            }
            Self::ContractValidation(error) => {
                write!(
                    formatter,
                    "event contract validation failed: {}",
                    error.code()
                )
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}

#[cfg(feature = "signature")]
fn verify_signature(event: &RadrootsEventEnvelope) -> Result<(), Error> {
    use secp256k1::{Message, Secp256k1, XOnlyPublicKey, schnorr::Signature};

    let message = Message::from_digest(*event.id().as_bytes());
    let public_key = XOnlyPublicKey::from_slice(&event.author().into_bytes())
        .map_err(|_| Error::MalformedEnvelope)?;
    let signature =
        Signature::from_slice(event.sig().as_bytes()).map_err(|_| Error::SignatureInvalid)?;
    Secp256k1::verification_only()
        .verify_schnorr(&signature, &message, &public_key)
        .map_err(|_| Error::SignatureInvalid)
}

#[cfg(not(feature = "signature"))]
fn verify_signature(_event: &RadrootsEventEnvelope) -> Result<(), Error> {
    Err(Error::SignatureVerificationUnavailable)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::envelope::RadrootsEventEnvelopeParts;

    fn valid_profile_event() -> RadrootsEventEnvelope {
        RadrootsEventEnvelope::new(RadrootsEventEnvelopeParts {
            id: "762bee187e9e645b81ec26ade05a69b5e8398caf527be8de0d9a45311ed0c7a0"
                .to_owned(),
            author: "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df"
                .to_owned(),
            created_at: 1_800_000_100,
            kind: 0,
            tags: vec![],
            content: "{\"display_name\":\"Moss Street Farm\",\"bot\":false,\"website\":\"https://mossstreet.example\",\"picture\":42}".to_owned(),
            sig: "4290da0bb6422986647bc8cd5f63bd52d49f41e7b665d3b47105b8109183e8d596f322c531d4061df53e1d2b70fda12d5d1c14f3720d7a56d9d0a03746af5109".to_owned(),
        })
        .expect("valid profile event")
    }

    #[test]
    fn raw_event_advances_only_after_id_verification() {
        let verified = RawEvent::new(valid_profile_event())
            .verify_id()
            .expect("verified id");
        assert_eq!(
            verified.event().id().to_hex(),
            "762bee187e9e645b81ec26ade05a69b5e8398caf527be8de0d9a45311ed0c7a0"
        );
    }

    #[cfg(feature = "signature")]
    #[test]
    fn positive_vector_reaches_contract_validated_state() {
        let validated = RawEvent::new(valid_profile_event())
            .verify_id()
            .expect("verified id")
            .verify_signature()
            .expect("verified signature")
            .validate_contract()
            .expect("validated contract");
        assert_eq!(validated.contract_id(), "radroots.profile.metadata.v1");
    }

    #[cfg(not(feature = "signature"))]
    #[test]
    fn signature_transition_fails_closed_when_capability_is_disabled() {
        let error = RawEvent::new(valid_profile_event())
            .verify_id()
            .expect("verified id")
            .verify_signature()
            .expect_err("signature capability must be unavailable");
        assert_eq!(error, Error::SignatureVerificationUnavailable);
    }
}
