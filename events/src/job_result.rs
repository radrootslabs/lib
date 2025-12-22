#[cfg(feature = "ts-rs")]
use ts_rs::TS;

use crate::{
    RadrootsNostrEvent, RadrootsNostrEventPtr, job::JobPaymentRequest,
    job_request::RadrootsJobInput,
};

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsJobResultEventIndex {
    pub event: RadrootsNostrEvent,
    pub metadata: RadrootsJobResultEventMetadata,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsJobResultEventMetadata {
    pub id: String,
    pub author: String,
    pub published_at: u32,
    pub kind: u32,
    pub job_result: RadrootsJobResult,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsJobResult {
    pub kind: u16,
    pub request_event: RadrootsNostrEventPtr,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub request_json: Option<String>,
    pub inputs: Vec<RadrootsJobInput>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub customer_pubkey: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "JobPaymentRequest | null"))]
    pub payment: Option<JobPaymentRequest>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub content: Option<String>,
    pub encrypted: bool,
}
