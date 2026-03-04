#![forbid(unsafe_code)]

use crate::{kinds::KIND_APP_DATA as KIND_APP_DATA_EVENT};
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

#[cfg(not(feature = "std"))]
use alloc::string::String;

pub const KIND_APP_DATA: u32 = KIND_APP_DATA_EVENT;


#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsAppData {
    pub d_tag: String,
    pub content: String,
}
