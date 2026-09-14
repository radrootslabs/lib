//! Cryptographically verified exact-plan signing receipts.

use core::fmt;
use radroots_event::SignedEvent;
use radroots_event_codec::verify::{self, Nip01SignatureVerifier, RawEvent};
use radroots_protocol::runtime::v1::OperationId;

use crate::{Error, SignRequest, SignerRequestId, SigningIntentId, error::Kind};

/// Successful signer output with exact request and artifact provenance.
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, PartialEq, Eq)]
pub struct SignReceipt {
    operation_kind: OperationId,
    intent_id: SigningIntentId,
    signer_request_id: SignerRequestId,
    signed_event: SignedEvent,
    completed_at_unix_ms: u64,
}

impl fmt::Debug for SignReceipt {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SignReceipt")
            .field("operation_kind", &self.operation_kind)
            .field("intent_id", &self.intent_id)
            .field("signer_request_id", &self.signer_request_id)
            .field("signed_event_id", &self.signed_event.id_str())
            .field("completed_at_unix_ms", &self.completed_at_unix_ms)
            .finish()
    }
}

impl SignReceipt {
    /// Verifies exact plan fields, raw/wire coherence, event ID, signature,
    /// deadline, cancellation, and request identity before success exists.
    pub fn from_signed_event(
        request: &SignRequest,
        signed_event: SignedEvent,
        completed_at_unix_ms: u64,
    ) -> Result<Self, Error> {
        request.ensure_active(completed_at_unix_ms)?;
        verify_signed_event(&signed_event, request)?;
        Ok(Self {
            operation_kind: request.operation_kind(),
            intent_id: request.intent_id(),
            signer_request_id: request.signer_request_id(),
            signed_event,
            completed_at_unix_ms,
        })
    }

    #[must_use]
    pub const fn operation_kind(&self) -> OperationId {
        self.operation_kind
    }

    #[must_use]
    pub const fn intent_id(&self) -> SigningIntentId {
        self.intent_id
    }

    #[must_use]
    pub const fn signer_request_id(&self) -> SignerRequestId {
        self.signer_request_id
    }

    #[must_use]
    pub const fn signed_event(&self) -> &SignedEvent {
        &self.signed_event
    }

    #[must_use]
    pub const fn completed_at_unix_ms(&self) -> u64 {
        self.completed_at_unix_ms
    }
}

fn verify_exact_plan(event: &SignedEvent, request: &SignRequest) -> Result<(), Error> {
    if event.pubkey() != request.expected_author()
        || event.created_at() != request.created_at()
        || event.kind() != request.kind()
        || event.tags_as_vec() != request.tags()
        || event.content() != request.content()
        || event.id() != request.expected_event_id()
    {
        return Err(Error::new(Kind::SignerOutputInvalid));
    }
    // `SignedEvent` construction proves its retained raw JSON parses to this
    // exact wire value; the checks above bind that wire to the request plan.
    Ok(())
}

/// Verified authored signature facts, independent of whether a caller is waiting.
///
/// This is neither an active success receipt nor authority to sign, admit or
/// deliver an event. It cannot represent an expiring Blossom credential.
/// Persistence callers must use [`Self::revalidate`] with their retained request
/// and locally observed time before recording the evidence.
///
/// Evidence cannot be deserialized without cryptographic verification:
/// ```compile_fail
/// fn decode<'a, T: serde::Deserialize<'a>>() {}
/// decode::<radroots_signing::AuthoredSignEvidence>();
/// ```
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, PartialEq, Eq)]
pub struct AuthoredSignEvidence {
    operation_kind: OperationId,
    intent_id: SigningIntentId,
    signer_request_id: SignerRequestId,
    signed_event: SignedEvent,
    observed_at_unix_ms: u64,
}

impl AuthoredSignEvidence {
    /// Verifies an already-created authored event against its originating request.
    ///
    /// Deadline and cancellation affect scheduling, not the truth of retained
    /// signature facts. Observation time must be positive and is not proof of
    /// when the signature was created. This constructor performs no signing.
    pub fn from_signed_event(
        request: &SignRequest,
        signed_event: SignedEvent,
        observed_at_unix_ms: u64,
    ) -> Result<Self, Error> {
        if request.authored_plan().is_none() || observed_at_unix_ms == 0 {
            return Err(Error::new(Kind::InvalidArgument));
        }
        verify_signed_event(&signed_event, request)?;
        Ok(Self {
            operation_kind: request.operation_kind(),
            intent_id: request.intent_id(),
            signer_request_id: request.signer_request_id(),
            signed_event,
            observed_at_unix_ms,
        })
    }

    /// Rebinds verified evidence to a caller's exact retained request and clock.
    ///
    /// A signature over the same event under a different operation or artifact
    /// is not evidence for this request. The original raw event bytes survive.
    pub fn revalidate(
        &self,
        request: &SignRequest,
        observed_at_unix_ms: u64,
    ) -> Result<Self, Error> {
        verify_identity(
            self.operation_kind,
            self.intent_id,
            self.signer_request_id,
            request,
        )?;
        Self::from_signed_event(request, self.signed_event.clone(), observed_at_unix_ms)
    }

    #[must_use]
    pub const fn operation_kind(&self) -> OperationId {
        self.operation_kind
    }

    #[must_use]
    pub const fn intent_id(&self) -> SigningIntentId {
        self.intent_id
    }

    #[must_use]
    pub const fn signer_request_id(&self) -> SignerRequestId {
        self.signer_request_id
    }

    #[must_use]
    pub const fn signed_event(&self) -> &SignedEvent {
        &self.signed_event
    }

    #[must_use]
    pub const fn observed_at_unix_ms(&self) -> u64 {
        self.observed_at_unix_ms
    }
}

impl fmt::Debug for AuthoredSignEvidence {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthoredSignEvidence")
            .field("operation_kind", &self.operation_kind)
            .field("intent_id", &self.intent_id)
            .field("signer_request_id", &self.signer_request_id)
            .field("signed_event_id", &self.signed_event.id_str())
            .field("observed_at_unix_ms", &self.observed_at_unix_ms)
            .finish()
    }
}

pub(crate) fn verify_identity(
    operation_kind: OperationId,
    intent_id: SigningIntentId,
    signer_request_id: SignerRequestId,
    request: &SignRequest,
) -> Result<(), Error> {
    if operation_kind != request.operation_kind()
        || intent_id != request.intent_id()
        || signer_request_id != request.signer_request_id()
    {
        return Err(Error::new(Kind::SignerOutputInvalid));
    }
    Ok(())
}

fn verify_signed_event(event: &SignedEvent, request: &SignRequest) -> Result<(), Error> {
    verify_exact_plan(event, request)?;
    let id_verified = verify::id(RawEvent::new(event.envelope().clone()))
        .map_err(|_| Error::new(Kind::SignerOutputInvalid))?;
    verify::signature(id_verified, &Nip01SignatureVerifier)
        .map_err(|_| Error::new(Kind::SignerOutputInvalid))?;
    Ok(())
}
