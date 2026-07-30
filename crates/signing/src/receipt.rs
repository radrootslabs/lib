//! Signing receipts.

use core::fmt;
use radroots_event::SignedEvent;
use radroots_protocol::runtime::v1::OperationId;

/// Successful signer output with portable operation provenance.
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, PartialEq, Eq)]
pub struct SignReceipt {
    operation_id: OperationId,
    signed_event: SignedEvent,
    completed_at_unix: u64,
}

impl fmt::Debug for SignReceipt {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SignReceipt")
            .field("operation_id", &self.operation_id)
            .field("signed_event_id", &self.signed_event.id_str())
            .field("completed_at_unix", &self.completed_at_unix)
            .finish()
    }
}

impl SignReceipt {
    /// Creates a receipt from an invariant-checked signed event.
    #[must_use]
    pub const fn new(
        operation_id: OperationId,
        signed_event: SignedEvent,
        completed_at_unix: u64,
    ) -> Self {
        Self {
            operation_id,
            signed_event,
            completed_at_unix,
        }
    }

    /// Returns the originating runtime operation identity.
    #[must_use]
    pub const fn operation_id(&self) -> OperationId {
        self.operation_id
    }

    /// Borrows the invariant-checked signed event.
    #[must_use]
    pub const fn signed_event(&self) -> &SignedEvent {
        &self.signed_event
    }

    /// Returns the host-supplied completion timestamp.
    #[must_use]
    pub const fn completed_at_unix(&self) -> u64 {
        self.completed_at_unix
    }
}
