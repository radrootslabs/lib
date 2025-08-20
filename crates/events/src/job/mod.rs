pub mod feedback {
    pub mod models;
}

pub mod request {
    pub mod models;
}

pub mod result {
    pub mod models;
}

use serde::{Deserialize, Serialize};

#[typeshare::typeshare]
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Copy)]
#[serde(rename_all = "snake_case")]
pub enum JobInputType {
    Url,
    Event,
    Job,
    Text,
}

#[typeshare::typeshare]
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Copy)]
#[serde(rename_all = "snake_case")]
pub enum JobFeedbackStatus {
    PaymentRequired,
    Processing,
    Error,
    Success,
    Partial,
}

#[typeshare::typeshare]
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct JobPaymentRequest {
    pub amount_sat: u32,
    pub bolt11: Option<String>,
}
