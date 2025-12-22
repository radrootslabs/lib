#[cfg(feature = "ts-rs")]
use ts_rs::TS;

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsRelayDocument {
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub name: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub description: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub pubkey: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub contact: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "number[] | null"))]
    pub supported_nips: Option<Vec<u16>>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub software: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub version: Option<String>,
}
