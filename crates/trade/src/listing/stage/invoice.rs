#[cfg(not(feature = "std"))]
use alloc::string::String;
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct TradeListingInvoiceRequest {
    pub accept_result_event_id: String,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct TradeListingInvoiceResult {
    pub total_sat: u32,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub bolt11: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub note: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "number | null"))]
    pub expires_at: Option<u32>,
}
