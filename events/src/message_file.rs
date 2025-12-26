#![forbid(unsafe_code)]

use crate::{RadrootsNostrEvent, RadrootsNostrEventPtr};
use crate::message::RadrootsMessageRecipient;
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsMessageFileEventIndex {
    pub event: RadrootsNostrEvent,
    pub metadata: RadrootsMessageFileEventMetadata,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsMessageFileEventMetadata {
    pub id: String,
    pub author: String,
    pub published_at: u32,
    pub kind: u32,
    pub message_file: RadrootsMessageFile,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsMessageFile {
    pub recipients: Vec<RadrootsMessageRecipient>,
    pub file_url: String,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "RadrootsNostrEventPtr | null"))]
    pub reply_to: Option<RadrootsNostrEventPtr>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub subject: Option<String>,
    pub file_type: String,
    pub encryption_algorithm: String,
    pub decryption_key: String,
    pub decryption_nonce: String,
    pub encrypted_hash: String,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub original_hash: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "number | null"))]
    pub size: Option<u64>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "RadrootsMessageFileDimensions | null"))]
    pub dimensions: Option<RadrootsMessageFileDimensions>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub blurhash: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub thumb: Option<String>,
    pub fallbacks: Vec<String>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RadrootsMessageFileDimensions {
    pub w: u32,
    pub h: u32,
}
