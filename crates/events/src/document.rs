#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsDocumentSubject {
    pub pubkey: String,
    pub address: Option<String>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsDocument {
    pub d_tag: String,
    pub doc_type: String,
    pub title: String,
    pub version: String,
    pub summary: Option<String>,
    pub effective_at: Option<u32>,
    pub body_markdown: Option<String>,
    pub subject: RadrootsDocumentSubject,
    pub tags: Option<Vec<String>>,
}
