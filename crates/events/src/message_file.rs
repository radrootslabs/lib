#![forbid(unsafe_code)]

use crate::RadrootsNostrEventPtr;
use crate::message::RadrootsMessageRecipient;

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsMessageFile {
    pub recipients: Vec<RadrootsMessageRecipient>,
    pub file_url: String,
    pub reply_to: Option<RadrootsNostrEventPtr>,
    pub subject: Option<String>,
    pub file_type: String,
    pub encryption_algorithm: String,
    pub decryption_key: String,
    pub decryption_nonce: String,
    pub encrypted_hash: String,
    pub original_hash: Option<String>,
    pub size: Option<u64>,
    pub dimensions: Option<RadrootsMessageFileDimensions>,
    pub blurhash: Option<String>,
    pub thumb: Option<String>,
    pub fallbacks: Vec<String>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RadrootsMessageFileDimensions {
    pub w: u32,
    pub h: u32,
}
