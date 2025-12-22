#[cfg(not(feature = "std"))]
use alloc::string::String;

#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct TradeListingReceiptRequest {
    pub fulfillment_result_event_id: String,
    pub note: Option<String>,
}

#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct TradeListingReceiptResult {
    pub acknowledged: bool,
    pub at: u32,
}
