#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::string::String;

#[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
#[cfg_attr(all(test, feature = "std"), dto(export))]
#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug)]
pub struct RadrootsGiftWrap {
    pub recipient: RadrootsGiftWrapRecipient,
    pub content: String,
    pub expiration: Option<u32>,
}

#[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
#[cfg_attr(all(test, feature = "std"), dto(export))]
#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug)]
pub struct RadrootsGiftWrapRecipient {
    pub public_key: String,
    pub relay_url: Option<String>,
}
