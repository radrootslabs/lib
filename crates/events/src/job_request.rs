use crate::job::JobInputType;

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsJobInput {
    pub data: String,
    pub input_type: JobInputType,
    pub relay: Option<String>,
    pub marker: Option<String>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsJobParam {
    pub key: String,
    pub value: String,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsJobRequest {
    pub kind: u16,
    pub inputs: Vec<RadrootsJobInput>,
    pub output: Option<String>,
    pub params: Vec<RadrootsJobParam>,
    pub bid_sat: Option<u32>,
    pub relays: Vec<String>,
    pub providers: Vec<String>,
    pub topics: Vec<String>,
    pub encrypted: bool,
}
