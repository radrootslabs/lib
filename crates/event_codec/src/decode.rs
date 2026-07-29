//! Deterministic event decoding boundary.
//!
//! Decoding validates the wire structure and returns a [`RawEvent`]. It does
//! not verify the event identifier, signature, or contract. Those transitions
//! remain explicit under [`crate::verify`].

use core::fmt;
use radroots_event::envelope::EventEnvelopeError;

#[cfg(feature = "json")]
use radroots_event::{
    admission::RawEvent,
    envelope::{EventEnvelope, EventEnvelopeParts},
    wire::{EventWireLimits, Nip01EventWire},
};

/// A failure while decoding an untrusted event representation.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DecodeError {
    /// The encoded event exceeds the public input budget.
    InputTooLarge { max: usize, actual: usize },
    /// The input is not a structurally valid event JSON object.
    InvalidJson,
    /// A decoded field violates the native event-envelope contract.
    InvalidEnvelope(EventEnvelopeError),
}

impl DecodeError {
    /// Returns a stable machine-readable error code.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InputTooLarge { .. } => "input_too_large",
            Self::InvalidJson => "invalid_json",
            Self::InvalidEnvelope(_) => "invalid_envelope",
        }
    }
}

impl fmt::Display for DecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InputTooLarge { max, actual } => {
                write!(formatter, "event JSON size {actual} exceeds {max} bytes")
            }
            Self::InvalidJson => formatter.write_str("event JSON is invalid"),
            Self::InvalidEnvelope(error) => write!(formatter, "event envelope is invalid: {error}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for DecodeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidEnvelope(error) => Some(error),
            Self::InputTooLarge { .. } | Self::InvalidJson => None,
        }
    }
}

/// Decodes compact NIP-01 event JSON without hiding later validation stages.
///
/// The returned [`RawEvent`] must still pass [`crate::verify::id`],
/// [`crate::verify::signature`], and [`crate::verify::contract`] before it is
/// treated as verified or contract-valid.
///
/// ```no_run
/// # #[cfg(feature = "json")]
/// # {
/// let raw_json = r#"{"id":"00","pubkey":"00","created_at":0,"kind":1,"tags":[],"content":"","sig":"00"}"#;
/// let raw = radroots_event_codec::decode::event(raw_json)?;
/// let id_verified = radroots_event_codec::verify::id(raw)?;
/// # let _ = id_verified;
/// # }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[cfg(feature = "json")]
pub fn event(raw_json: &str) -> Result<RawEvent, DecodeError> {
    let max = EventWireLimits::default().max_raw_json_bytes;
    let actual = raw_json.len();
    if actual > max {
        return Err(DecodeError::InputTooLarge { max, actual });
    }

    let wire =
        serde_json::from_str::<Nip01EventWire>(raw_json).map_err(|_| DecodeError::InvalidJson)?;
    let envelope = EventEnvelope::new(EventEnvelopeParts {
        id: wire.id,
        author: wire.pubkey,
        created_at: wire.created_at,
        kind: wire.kind,
        tags: wire.tags,
        content: wire.content,
        sig: wire.sig,
    })
    .map_err(DecodeError::InvalidEnvelope)?;
    Ok(RawEvent::new(envelope))
}
