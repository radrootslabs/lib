#[cfg(feature = "ts-rs")]
use ts_rs::TS;

use crate::{RadrootsNostrEvent, job::JobInputType};

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsJobRequestEventIndex {
    pub event: RadrootsNostrEvent,
    pub metadata: RadrootsJobRequestEventMetadata,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsJobRequestEventMetadata {
    pub id: String,
    pub author: String,
    pub published_at: u32,
    pub kind: u32,
    pub job_request: RadrootsJobRequest,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsJobInput {
    pub data: String,
    pub input_type: JobInputType,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub relay: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub marker: Option<String>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsJobParam {
    pub key: String,
    pub value: String,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsJobRequest {
    pub kind: u16,
    pub inputs: Vec<RadrootsJobInput>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub output: Option<String>,
    pub params: Vec<RadrootsJobParam>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "number | null"))]
    pub bid_sat: Option<u32>,
    pub relays: Vec<String>,
    pub providers: Vec<String>,
    pub topics: Vec<String>,
    pub encrypted: bool,
}
