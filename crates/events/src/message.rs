#![forbid(unsafe_code)]

use crate::RadrootsNostrEventPtr;
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsMessage {
    pub recipients: Vec<RadrootsMessageRecipient>,
    pub content: String,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "RadrootsNostrEventPtr | null"))]
    pub reply_to: Option<RadrootsNostrEventPtr>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub subject: Option<String>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsMessageRecipient {
    pub public_key: String,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub relay_url: Option<String>,
}
