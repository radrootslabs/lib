#![cfg(feature = "json")]

use radroots_event::social::relay_document::RelayDocument;

use crate::error::EventParseError;

pub fn from_json(content: &str) -> Result<RelayDocument, EventParseError> {
    serde_json::from_str(content).map_err(|_| EventParseError::InvalidJson("relay_document"))
}
