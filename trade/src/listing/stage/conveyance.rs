#[cfg(not(feature = "std"))]
use alloc::string::String;
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
#[cfg_attr(
    feature = "serde",
    serde(rename_all = "snake_case", tag = "kind", content = "amount")
)]
pub enum TradeListingConveyanceMethod {
    SellerDelivery {
        window: Option<String>,
        notes: Option<String>,
    },
    BuyerPickup {
        location_hint: Option<String>,
        by_when: Option<String>,
    },
    ThirdParty {
        provider: String,
        ref_id: Option<String>,
        notes: Option<String>,
    },
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct TradeListingConveyanceRequest {
    pub accept_result_event_id: String,
    pub method: TradeListingConveyanceMethod,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct TradeListingConveyanceResult {
    pub verified: bool,
    pub method: TradeListingConveyanceMethod,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub message: Option<String>,
}
