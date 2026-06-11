#![forbid(unsafe_code)]

use crate::RadrootsNostrEventPtr;

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsMessage {
    pub recipients: Vec<RadrootsMessageRecipient>,
    pub content: String,
    pub reply_to: Option<RadrootsNostrEventPtr>,
    pub subject: Option<String>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsMessageRecipient {
    pub public_key: String,
    pub relay_url: Option<String>,
}
