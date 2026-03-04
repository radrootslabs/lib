#![forbid(unsafe_code)]

#[cfg(feature = "ts-rs")]
use ts_rs::TS;

#[cfg(not(feature = "std"))]
use alloc::string::String;


#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsGiftWrap {
    pub recipient: RadrootsGiftWrapRecipient,
    pub content: String,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "number | null"))]
    pub expiration: Option<u32>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsGiftWrapRecipient {
    pub public_key: String,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub relay_url: Option<String>,
}
