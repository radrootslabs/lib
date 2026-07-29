use crate::{
    social::job::{JobFeedbackStatus, JobPaymentRequest},
    tag::EventPtr,
};

#[cfg(not(feature = "std"))]
use alloc::string::String;

#[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
#[cfg_attr(all(test, feature = "std"), dto(export))]
#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JobFeedback {
    pub kind: u16,
    pub status: JobFeedbackStatus,
    pub extra_info: Option<String>,
    pub request_event: EventPtr,
    pub customer_pubkey: Option<String>,
    pub payment: Option<JobPaymentRequest>,
    pub content: Option<String>,
    pub encrypted: bool,
}
