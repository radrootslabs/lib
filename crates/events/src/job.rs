#[cfg(not(feature = "std"))]
use alloc::string::String;

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Debug, PartialEq, Eq, Copy)]
pub enum JobInputType {
    #[cfg_attr(feature = "serde", serde(rename = "url"))]
    Url,
    #[cfg_attr(feature = "serde", serde(rename = "event"))]
    Event,
    #[cfg_attr(feature = "serde", serde(rename = "job"))]
    Job,
    #[cfg_attr(feature = "serde", serde(rename = "text"))]
    Text,
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Debug, PartialEq, Eq, Copy)]
pub enum JobFeedbackStatus {
    #[cfg_attr(feature = "serde", serde(rename = "payment_required"))]
    PaymentRequired,
    #[cfg_attr(feature = "serde", serde(rename = "processing"))]
    Processing,
    #[cfg_attr(feature = "serde", serde(rename = "error"))]
    Error,
    #[cfg_attr(feature = "serde", serde(rename = "success"))]
    Success,
    #[cfg_attr(feature = "serde", serde(rename = "partial"))]
    Partial,
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JobPaymentRequest {
    pub amount_sat: u32,
    pub bolt11: Option<String>,
}
