//! Canonical NIP-01 event representations.
//!
//! Canonicalization computes bytes and identifiers; it does not assert that an
//! envelope's declared identifier or signature is valid. Use [`crate::verify`]
//! for those explicit state transitions.

#[cfg(not(feature = "std"))]
use alloc::string::String;
#[cfg(feature = "std")]
use std::string::String;

use radroots_event::{
    envelope::EventEnvelope,
    id::EventId,
    wire::{
        CanonicalEventIdError, canonical_nip01_event_id_preimage, compute_canonical_nip01_event_id,
    },
};

/// A failure while producing a canonical NIP-01 representation.
pub type CanonicalError = CanonicalEventIdError;

/// Produces the canonical NIP-01 identifier preimage for an event.
///
/// This operation is deterministic and does not compare the computed
/// identifier with the envelope's declared identifier.
pub fn id_preimage(event: &EventEnvelope) -> Result<String, CanonicalError> {
    canonical_nip01_event_id_preimage(
        &event.author().to_hex(),
        event.created_at_u64(),
        event.kind_u32(),
        &event.tags_as_vec(),
        event.content(),
    )
}

/// Computes the canonical NIP-01 identifier for an event.
///
/// This operation is deterministic and does not advance the event's
/// verification state.
pub fn id(event: &EventEnvelope) -> Result<EventId, CanonicalError> {
    compute_canonical_nip01_event_id(
        &event.author().to_hex(),
        event.created_at_u64(),
        event.kind_u32(),
        &event.tags_as_vec(),
        event.content(),
    )
}
