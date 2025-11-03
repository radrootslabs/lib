use serde::{Deserialize, Serialize};

use crate::{
    RadrootsNostrEvent, RadrootsNostrEventPtr,
    job::{JobFeedbackStatus, JobPaymentRequest},
};

#[typeshare::typeshare]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsJobFeedbackEventIndex {
    pub event: RadrootsNostrEvent,
    pub metadata: RadrootsJobFeedbackEventMetadata,
}

#[typeshare::typeshare]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsJobFeedbackEventMetadata {
    pub id: String,
    pub author: String,
    pub published_at: u32,
    pub kind: u32,
    pub job_feedback: RadrootsJobFeedback,
}

#[typeshare::typeshare]
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
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
