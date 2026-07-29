//! Deterministic event encoding boundary.

#[cfg(all(not(feature = "std"), feature = "serde_json"))]
use alloc::string::String;
use core::fmt;
#[cfg(all(feature = "std", feature = "serde_json"))]
use std::string::String;

#[cfg(feature = "serde_json")]
use radroots_event::envelope::EventEnvelope;

/// A failure while encoding an event.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EncodeError {
    /// The event could not be represented as canonical compact JSON.
    Json,
}

impl EncodeError {
    /// Returns a stable machine-readable error code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Json => "json",
        }
    }
}

impl fmt::Display for EncodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("failed to encode event JSON")
    }
}

#[cfg(feature = "std")]
impl std::error::Error for EncodeError {}

/// Encodes an event as deterministic compact NIP-01 JSON.
///
/// This operation preserves the supplied envelope; it does not imply that the
/// identifier, signature, or contract has been verified.
///
/// ```no_run
/// # #[cfg(feature = "serde_json")]
/// # {
/// # let raw_json = r#"{"id":"00","pubkey":"00","created_at":0,"kind":1,"tags":[],"content":"","sig":"00"}"#;
/// let raw = radroots_event_codec::decode::event(raw_json)?;
/// let encoded = radroots_event_codec::encode::event(raw.event())?;
/// # let _ = encoded;
/// # }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[cfg(feature = "serde_json")]
pub fn event(event: &EventEnvelope) -> Result<String, EncodeError> {
    serde_json::to_string(&event.to_nip01_wire()).map_err(|_| EncodeError::Json)
}
