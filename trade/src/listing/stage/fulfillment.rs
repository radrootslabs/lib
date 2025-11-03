#[cfg(not(feature = "std"))]
use alloc::string::String;

#[typeshare::typeshare]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct TradeListingFulfillmentRequest {
    pub payment_result_event_id: String,
}

#[typeshare::typeshare]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
#[cfg_attr(
    feature = "serde",
    serde(rename_all = "snake_case", tag = "kind", content = "amount")
)]
pub enum TradeListingFulfillmentState {
    Preparing,
    Shipped,
    ReadyForPickup,
    Delivered,
    Canceled,
}

#[typeshare::typeshare]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct TradeListingFulfillmentResult {
    pub state: TradeListingFulfillmentState,
    pub tracking: Option<String>,
    pub eta: Option<String>,
    pub notes: Option<String>,
}
