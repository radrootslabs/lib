use serde::{Deserialize, Serialize};

use crate::{
    RadrootsNostrEvent, RadrootsNostrEventPtr,
    job::{JobPaymentRequest, request::models::RadrootsJobInput},
};

#[typeshare::typeshare]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsJobResultEventIndex {
    pub event: RadrootsNostrEvent,
    pub metadata: RadrootsJobResultEventMetadata,
}

#[typeshare::typeshare]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsJobResultEventMetadata {
    pub id: String,
    pub author: String,
    pub published_at: u32,
    pub kind: u32,
    pub job_result: RadrootsJobResult,
}

#[typeshare::typeshare]
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RadrootsJobResult {
    pub kind: u16,
    pub request_event: RadrootsNostrEventPtr,
    pub request_json: Option<String>,
    pub inputs: Vec<RadrootsJobInput>,
    pub customer_pubkey: Option<String>,
    pub payment: Option<JobPaymentRequest>,
    pub content: Option<String>,
    pub encrypted: bool,
}
