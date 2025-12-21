#![cfg(feature = "serde_json")]

#[cfg(not(feature = "std"))]
use alloc::string::String;

use radroots_events::relay_document::RadrootsRelayDocument;

use crate::error::EventEncodeError;

pub fn to_json(doc: &RadrootsRelayDocument) -> Result<String, EventEncodeError> {
    serde_json::to_string(doc).map_err(|_| EventEncodeError::Json)
}
