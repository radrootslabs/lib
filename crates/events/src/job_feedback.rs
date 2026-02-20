#[cfg(feature = "ts-rs")]
use ts_rs::TS;

use crate::{
    RadrootsNostrEvent, RadrootsNostrEventPtr,
    job::{JobFeedbackStatus, JobPaymentRequest},
};

#[cfg(not(feature = "std"))]
use alloc::string::String;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsJobFeedbackEventIndex {
    pub event: RadrootsNostrEvent,
    pub metadata: RadrootsJobFeedbackEventMetadata,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsJobFeedbackEventMetadata {
    pub id: String,
    pub author: String,
    pub published_at: u32,
    pub kind: u32,
    pub job_feedback: RadrootsJobFeedback,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsJobFeedback {
    pub kind: u16,
    pub status: JobFeedbackStatus,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub extra_info: Option<String>,
    pub request_event: RadrootsNostrEventPtr,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub customer_pubkey: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "JobPaymentRequest | null"))]
    pub payment: Option<JobPaymentRequest>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub content: Option<String>,
    pub encrypted: bool,
}
