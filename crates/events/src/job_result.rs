use crate::{RadrootsNostrEventPtr, job::JobPaymentRequest, job_request::RadrootsJobInput};

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
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
