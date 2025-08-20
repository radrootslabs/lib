#![cfg_attr(not(feature = "serde"), allow(unused_attributes))]

#[cfg(not(feature = "std"))]
use alloc::string::String;

#[typeshare::typeshare]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
#[serde(rename_all = "snake_case", tag = "kind", content = "amount")]
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

#[typeshare::typeshare]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct TradeListingConveyanceRequest {
    pub accept_result_event_id: String,
    pub method: TradeListingConveyanceMethod,
}

#[typeshare::typeshare]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct TradeListingConveyanceResult {
    pub verified: bool,
    pub method: TradeListingConveyanceMethod,
    pub message: Option<String>,
}
