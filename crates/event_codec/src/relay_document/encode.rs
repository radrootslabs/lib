#![cfg(feature = "json")]

#[cfg(not(feature = "std"))]
use alloc::string::String;

use radroots_event::social::relay_document::RelayDocument;

use crate::error::EventEncodeError;

pub fn to_json(doc: &RelayDocument) -> Result<String, EventEncodeError> {
    serde_json::to_string(doc).map_err(|_| EventEncodeError::Json)
}
