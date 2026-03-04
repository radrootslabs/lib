#[cfg(feature = "ts-rs")]
use ts_rs::TS;

use crate::{
    RadrootsNostrEventPtr,
    job::{JobFeedbackStatus, JobPaymentRequest},
};

#[cfg(not(feature = "std"))]
use alloc::string::String;

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
