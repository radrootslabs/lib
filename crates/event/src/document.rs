#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

#[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
#[cfg_attr(all(test, feature = "std"), dto(export))]
#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug)]
pub struct DocumentSubject {
    pub pubkey: String,
    pub address: Option<String>,
}

#[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
#[cfg_attr(all(test, feature = "std"), dto(export))]
#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug)]
pub struct Document {
    pub d_tag: String,
    pub doc_type: String,
    pub title: String,
    pub version: String,
    pub summary: Option<String>,
    pub effective_at: Option<u32>,
    pub body_markdown: Option<String>,
    pub subject: DocumentSubject,
    pub tags: Option<Vec<String>>,
}
