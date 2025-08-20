#[cfg(not(feature = "std"))]
use alloc::string::String;

#[typeshare::typeshare]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct TradeListingInvoiceRequest {
    pub accept_result_event_id: String,
}

#[typeshare::typeshare]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct TradeListingInvoiceResult {
    pub total_sat: u32,
    pub bolt11: Option<String>,
    pub note: Option<String>,
    pub expires_at: Option<u32>,
}
