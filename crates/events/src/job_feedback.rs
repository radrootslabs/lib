use crate::{
    RadrootsNostrEventPtr,
    job::{JobFeedbackStatus, JobPaymentRequest},
};

#[cfg(not(feature = "std"))]
use alloc::string::String;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsJobFeedback {
    pub kind: u16,
    pub status: JobFeedbackStatus,
    pub extra_info: Option<String>,
    pub request_event: RadrootsNostrEventPtr,
    pub customer_pubkey: Option<String>,
    pub payment: Option<JobPaymentRequest>,
    pub content: Option<String>,
    pub encrypted: bool,
}
