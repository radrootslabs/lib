use crate::social::job::JobInputType;

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

#[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
#[cfg_attr(all(test, feature = "std"), dto(export))]
#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JobInput {
    pub data: String,
    pub input_type: JobInputType,
    pub relay: Option<String>,
    pub marker: Option<String>,
}

#[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
#[cfg_attr(all(test, feature = "std"), dto(export))]
#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JobParam {
    pub key: String,
    pub value: String,
}

#[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
#[cfg_attr(all(test, feature = "std"), dto(export))]
#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JobRequest {
    pub kind: u16,
    pub inputs: Vec<JobInput>,
    pub output: Option<String>,
    pub params: Vec<JobParam>,
    pub bid_sat: Option<u32>,
    pub relays: Vec<String>,
    pub providers: Vec<String>,
    pub topics: Vec<String>,
    pub encrypted: bool,
}
