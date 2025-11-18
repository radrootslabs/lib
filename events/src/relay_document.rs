use serde::{Deserialize, Serialize};
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsRelayDocument {
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    name: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    description: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pubkey: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    contact: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "number[] | null"))]
    supported_nips: Option<Vec<u16>>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    software: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    version: Option<String>,
}
