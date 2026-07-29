#![forbid(unsafe_code)]

use crate::tag::RadrootsEventPtr;

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug)]
pub struct RadrootsMessage {
    pub recipients: Vec<RadrootsMessageRecipient>,
    pub content: String,
    pub reply_to: Option<RadrootsEventPtr>,
    pub subject: Option<String>,
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug)]
pub struct RadrootsMessageRecipient {
    pub public_key: String,
    pub relay_url: Option<String>,
}
