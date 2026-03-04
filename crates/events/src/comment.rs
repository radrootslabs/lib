#[cfg(feature = "ts-rs")]
use ts_rs::TS;

use crate::{RadrootsNostrEventRef};

#[cfg(not(feature = "std"))]
use alloc::string::String;


#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsComment {
    pub root: RadrootsNostrEventRef,
    pub parent: RadrootsNostrEventRef,
    pub content: String,
}
