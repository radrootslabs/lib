use crate::{
    social::{job::JobPaymentRequest, job_request::RadrootsJobInput},
    tag::RadrootsEventPtr,
};

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

#[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
#[cfg_attr(all(test, feature = "std"), dto(export))]
#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsJobResult {
    pub kind: u16,
    pub request_event: RadrootsEventPtr,
    pub request_json: Option<String>,
    pub inputs: Vec<RadrootsJobInput>,
    pub customer_pubkey: Option<String>,
    pub payment: Option<JobPaymentRequest>,
    pub content: Option<String>,
    pub encrypted: bool,
}
