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
        verify_exact_plan(&signed_event, request)?;
        let id_verified = verify::id(RawEvent::new(signed_event.envelope().clone()))
            .map_err(|_| Error::new(Kind::SignerOutputInvalid))?;
        verify::signature(id_verified, &Nip01SignatureVerifier)
            .map_err(|_| Error::new(Kind::SignerOutputInvalid))?;
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
