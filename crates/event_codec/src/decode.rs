//! Deterministic event decoding boundary.
//!
//! Decoding validates the wire structure and returns a [`RawEvent`]. It does
//! not verify the event identifier, signature, or contract. Those transitions
//! remain explicit under [`crate::verify`].

use core::fmt;
use radroots_event::{envelope::EventEnvelopeError, wire::EventWireError};

// Domain-specific parsers live behind the canonical decoding namespace.
pub use crate::error::EventParseError;
pub use crate::parsed::{RadrootsParsedData, RadrootsParsedEvent};

pub mod event_ref {
    pub use crate::event_ref::*;
}

pub mod parsed {
    pub use crate::parsed::*;
}

pub mod wire {
    pub use crate::wire::*;
}

macro_rules! decode_domain {
    ($name:ident, $source:path) => {
        pub mod $name {
            pub use $source::*;
        }
    };
}

decode_domain!(app_data, crate::app_data::decode);
decode_domain!(article, crate::article::decode);
decode_domain!(calendar, crate::calendar::decode);
#[cfg(feature = "json")]
decode_domain!(coop, crate::coop::decode);
#[cfg(feature = "json")]
decode_domain!(document, crate::document::decode);
#[cfg(feature = "json")]
decode_domain!(farm_crdt, crate::farm_crdt::decode);
decode_domain!(farm_file, crate::farm_file::decode);
#[cfg(feature = "json")]
decode_domain!(farm_workspace, crate::farm_workspace::decode);
decode_domain!(file_metadata, crate::file_metadata::decode);
decode_domain!(follow, crate::follow::decode);
decode_domain!(geochat, crate::geochat::decode);
decode_domain!(gift_wrap, crate::gift_wrap::decode);
decode_domain!(group, crate::group::decode);
decode_domain!(http_auth, crate::http_auth::decode);
decode_domain!(list, crate::list::decode);
decode_domain!(list_set, crate::list_set::decode);
decode_domain!(message, crate::message::decode);
decode_domain!(message_file, crate::message_file::decode);
#[cfg(feature = "json")]
decode_domain!(plot, crate::plot::decode);
decode_domain!(reaction, crate::reaction::decode);
decode_domain!(relay_auth, crate::relay_auth::decode);
#[cfg(feature = "json")]
decode_domain!(relay_document, crate::relay_document::decode);
decode_domain!(report, crate::report::decode);
decode_domain!(repost, crate::repost::decode);
#[cfg(feature = "json")]
decode_domain!(resource_area, crate::resource_area::decode);
#[cfg(feature = "json")]
decode_domain!(resource_cap, crate::resource_cap::decode);
decode_domain!(seal, crate::seal::decode);

pub mod comment {
    pub use crate::comment::inbound::*;
}

pub mod deletion {
    pub use crate::deletion::inbound::*;
    pub use crate::deletion::reconciliation_v1::inbound as reconciliation_v1;
}

#[cfg(feature = "json")]
pub mod farm {
    pub use crate::farm::decode::*;
}

pub mod food_availability {
    pub use crate::food_availability::inbound::*;
}

pub mod job {
    pub use crate::job::error::*;
    pub use crate::job::traits::*;
    pub use crate::job::util::*;

    pub mod feedback {
        pub use crate::job::feedback::decode::*;
    }
    pub mod request {
        pub use crate::job::request::decode::*;
    }
    pub mod result {
        pub use crate::job::result::decode::*;
    }
}

#[cfg(feature = "knowledge")]
pub mod knowledge {
    pub use crate::knowledge::decode::*;
}

pub mod operational_listing {
    pub use crate::operational_listing::decode::*;
}

#[cfg(feature = "json")]
pub mod order {
    pub use crate::order::decode::*;
}

pub mod post {
    pub use crate::post::decode::*;
    pub use crate::post::inbound::*;
}

#[cfg(feature = "json")]
pub mod profile {
    pub use crate::profile::decode::*;
    pub use crate::profile::inbound::*;
    pub use crate::profile::{LegacyProfile, RadrootsProfileData};
}

pub mod reply {
    pub use crate::reply::inbound::*;
}

#[cfg(feature = "json")]
pub mod trade {
    pub use crate::trade::{RadrootsTradeMutationParseError, trade_mutation_from_event};
}

#[cfg(feature = "json")]
use radroots_event::{
    SignedEvent,
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

/// Decodes compact NIP-01 JSON into an ID-verified signed event.
#[cfg(feature = "json")]
pub fn signed_event(raw_json: &str) -> Result<SignedEvent, DecodeError> {
    let max = MAX_EVENT_JSON_BYTES;
    let actual = raw_json.len();
    if actual > max {
        return Err(DecodeError::InputTooLarge { max, actual });
    }
    let wire = Nip01EventWire::parse_json(raw_json).map_err(map_wire_error)?;
    SignedEvent::from_wire_verified_id(wire, raw_json).map_err(|error| match error {
        radroots_event::draft::SignedEventError::Envelope(error) => {
            DecodeError::InvalidEnvelope(error)
        }
        radroots_event::draft::SignedEventError::Wire(error)
        | radroots_event::draft::SignedEventError::RawJson(error) => map_wire_error(error),
        radroots_event::draft::SignedEventError::RawJsonMismatch => DecodeError::InvalidJson,
    })
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
