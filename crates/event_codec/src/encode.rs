//! Deterministic event encoding boundary.

#[cfg(all(not(feature = "std"), feature = "json"))]
use alloc::string::String;
use core::fmt;
#[cfg(all(feature = "std", feature = "json"))]
use std::string::String;

#[cfg(feature = "json")]
use radroots_event::envelope::EventEnvelope;

// Domain-specific algorithms live behind the canonical encoding namespace.
// Their implementation modules remain crate-private so the public API has one
// stable route for each operation.
pub use crate::error::{EventEncodeError, RadrootsEncodeError};

macro_rules! encode_domain {
    ($name:ident, $source:path) => {
        pub mod $name {
            pub use $source::*;
        }
    };
}

encode_domain!(app_data, crate::app_data::encode);
encode_domain!(article, crate::article::encode);
encode_domain!(calendar, crate::calendar::encode);
pub mod coop {
    pub use crate::coop::encode::*;
    pub use crate::coop::list_sets::*;
}
encode_domain!(document, crate::document::encode);
encode_domain!(farm_crdt, crate::farm_crdt::encode);
encode_domain!(farm_file, crate::farm_file::encode);
encode_domain!(farm_workspace, crate::farm_workspace::encode);
encode_domain!(file_metadata, crate::file_metadata::encode);
encode_domain!(follow, crate::follow::encode);
encode_domain!(geochat, crate::geochat::encode);
encode_domain!(gift_wrap, crate::gift_wrap::encode);
encode_domain!(group, crate::group::encode);
encode_domain!(http_auth, crate::http_auth::encode);
encode_domain!(list, crate::list::encode);
encode_domain!(list_set, crate::list_set::encode);
encode_domain!(message, crate::message::encode);
encode_domain!(message_file, crate::message_file::encode);
encode_domain!(plot, crate::plot::encode);
encode_domain!(reaction, crate::reaction::encode);
encode_domain!(relay_auth, crate::relay_auth::encode);
#[cfg(feature = "json")]
encode_domain!(relay_document, crate::relay_document::encode);
encode_domain!(report, crate::report::encode);
encode_domain!(repost, crate::repost::encode);
pub mod resource_area {
    pub use crate::resource_area::encode::*;
    pub use crate::resource_area::list_sets::*;
}
encode_domain!(resource_cap, crate::resource_cap::encode);
encode_domain!(seal, crate::seal::encode);

pub mod comment {
    pub use crate::comment::authored::*;
}

pub mod deletion {
    pub use crate::deletion::authored::*;
}

pub mod farm {
    pub use crate::farm::encode::*;
    pub use crate::farm::list_sets::*;
}

pub mod food_availability {
    pub use crate::food_availability::authored::*;
}

pub mod job {
    pub use crate::job::encode::*;

    pub mod feedback {
        pub use crate::job::feedback::encode::*;
    }
    pub mod request {
        pub use crate::job::request::encode::*;
    }
    pub mod result {
        pub use crate::job::result::encode::*;
    }
}

pub mod tag_builders {
    pub use crate::tag_builders::*;
}

#[cfg(feature = "knowledge")]
pub mod knowledge {
    pub use crate::knowledge::encode::*;
}

pub mod operational_listing {
    pub use crate::operational_listing::encode::*;
    pub use crate::operational_listing::tags::*;
}

pub mod order {
    #[cfg(feature = "json")]
    pub use crate::order::encode::*;
    pub use crate::order::tags::*;
}

pub mod post {
    pub use crate::post::authored::*;
}

#[cfg(feature = "json")]
pub mod profile {
    pub use crate::profile::authored::*;
}

pub mod reply {
    pub use crate::reply::authored::*;
}

#[cfg(feature = "json")]
pub mod trade {
    pub use crate::trade::{
        RadrootsTradeMutationError, trade_mutation_event_build,
        trade_mutation_event_build_with_extra_tags, trade_mutation_tags,
    };
}

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
/// # #[cfg(feature = "json")]
/// # {
/// # let raw_json = r#"{"id":"00","pubkey":"00","created_at":0,"kind":1,"tags":[],"content":"","sig":"00"}"#;
/// let raw = radroots_event_codec::decode::event(raw_json)?;
/// let encoded = radroots_event_codec::encode::event(raw.event())?;
/// # let _ = encoded;
/// # }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[cfg(feature = "json")]
pub fn event(event: &EventEnvelope) -> Result<String, EncodeError> {
    serde_json::to_string(&event.to_nip01_wire()).map_err(|_| EncodeError::Json)
}
