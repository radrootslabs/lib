#[cfg(not(feature = "std"))]
use alloc::string::String;

#[typeshare::typeshare]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct TradeListingAcceptRequest {
    pub order_result_event_id: String,
    pub listing_event_id: String,
}

#[typeshare::typeshare]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct TradeListingAcceptResult {
    pub listing_event_id: String,
    pub order_result_event_id: String,
    pub accepted_by: String,
}
