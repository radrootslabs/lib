//! Deterministic event decoding boundary.
//!
//! Decoding validates the wire structure and returns a [`RawEvent`]. It does
//! not verify the event identifier, signature, or contract. Those transitions
//! remain explicit under [`crate::verify`].

use core::fmt;
use radroots_event::{envelope::EventEnvelopeError, wire::EventWireError};

#[cfg(feature = "json")]
use radroots_event::{
    admission::RawEvent,
    wire::{DEFAULT_RAW_JSON_MAX_BYTES, Nip01EventWire},
};

/// Maximum compact NIP-01 event JSON size accepted by [`event`].
#[cfg(feature = "json")]
pub const MAX_EVENT_JSON_BYTES: usize = DEFAULT_RAW_JSON_MAX_BYTES;

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
    /// A decoded field violates the bounded NIP-01 wire contract.
    InvalidWire(EventWireError),
}

impl DecodeError {
    /// Returns a stable machine-readable error code.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InputTooLarge { .. } => "input_too_large",
            Self::InvalidJson => "invalid_json",
            Self::InvalidEnvelope(_) => "invalid_envelope",
            Self::InvalidWire(_) => "invalid_wire",
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
            Self::InvalidWire(error) => write!(formatter, "event wire is invalid: {error}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for DecodeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidEnvelope(error) => Some(error),
            Self::InvalidWire(error) => Some(error),
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
    let max = MAX_EVENT_JSON_BYTES;
    let actual = raw_json.len();
    if actual > max {
        return Err(DecodeError::InputTooLarge { max, actual });
    }

    let wire = Nip01EventWire::parse_json_unverified(raw_json).map_err(map_wire_error)?;
    let envelope = wire
        .into_unverified_envelope()
        .map_err(DecodeError::InvalidEnvelope)?;
    Ok(RawEvent::new(envelope))
}

#[cfg(feature = "json")]
fn map_wire_error(error: EventWireError) -> DecodeError {
    match error {
        EventWireError::RawJsonTooLarge { max, actual } => {
            DecodeError::InputTooLarge { max, actual }
        }
        EventWireError::Json(_)
        | EventWireError::RootNotObject
        | EventWireError::MissingField(_)
        | EventWireError::InvalidField(_) => DecodeError::InvalidJson,
        EventWireError::Envelope(error) => DecodeError::InvalidEnvelope(error),
        error => DecodeError::InvalidWire(error),
    }
}
