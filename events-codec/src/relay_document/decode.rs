#![cfg(feature = "serde_json")]

use radroots_events::relay_document::RadrootsRelayDocument;

use crate::error::EventParseError;

pub fn from_json(content: &str) -> Result<RadrootsRelayDocument, EventParseError> {
    serde_json::from_str(content).map_err(|_| EventParseError::InvalidJson("relay_document"))
}
