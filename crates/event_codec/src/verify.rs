//! Explicit event identifier, signature, and contract verification stages.
//!
//! Each function consumes one typestate and returns the next. The API cannot
//! silently skip identifier, signature, or contract validation.

pub use radroots_event::admission::Error as VerificationError;
pub use radroots_event::admission::{
    ContractValidatedEvent, IdVerifiedEvent, RawEvent, SignatureVerifiedEvent, SignatureVerifier,
};

// Preserve the mature verification profiles under the final verification
// namespace while consumers move away from the superseded crate-root exports.
pub use crate::verification::*;

/// Verifies that an event's declared identifier matches its canonical bytes.
pub fn id(event: RawEvent) -> Result<IdVerifiedEvent, VerificationError> {
    event.verify_id()
}

/// Verifies an event signature using an explicit deterministic capability.
///
/// ```no_run
/// use radroots_event::admission::{Error, SignatureVerifier};
/// use radroots_event::envelope::EventEnvelope;
///
/// struct Verifier;
///
/// impl SignatureVerifier for Verifier {
///     fn verify_signature(&self, _event: &EventEnvelope) -> Result<(), Error> {
///         Ok(())
///     }
/// }
///
/// # fn advance(
/// #     event: radroots_event_codec::verify::IdVerifiedEvent,
/// # ) -> Result<(), radroots_event_codec::VerificationError> {
/// let signature_verified =
///     radroots_event_codec::verify::signature(event, &Verifier)?;
/// let contract_validated =
///     radroots_event_codec::verify::contract(signature_verified)?;
/// # let _ = contract_validated;
/// # Ok(())
/// # }
/// ```
pub fn signature<V>(
    event: IdVerifiedEvent,
    verifier: &V,
) -> Result<SignatureVerifiedEvent, VerificationError>
where
    V: SignatureVerifier + ?Sized,
{
    event.verify_signature(verifier)
}

/// Validates an already signature-verified event against the contract registry.
pub fn contract(
    event: SignatureVerifiedEvent,
) -> Result<ContractValidatedEvent, VerificationError> {
    event.validate_contract()
}
