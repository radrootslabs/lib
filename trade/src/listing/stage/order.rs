#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
use radroots_core::{RadrootsCoreDiscount, RadrootsCoreQuantityPrice};
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

use crate::listing::model::{RadrootsTradeListingSubtotal, RadrootsTradeListingTotal};

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct TradeListingOrderRequestPayload {
    pub bin_id: String,
    pub bin_count: u32,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct TradeListingOrderRequest {
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsNostrEventPtr"))]
    pub event: radroots_events::RadrootsNostrEventPtr,
    pub payload: TradeListingOrderRequestPayload,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct TradeListingOrderResult {
    pub bin_id: String,
    pub bin_count: u32,
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsCoreQuantityPrice"))]
    pub price: RadrootsCoreQuantityPrice,
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsCoreDiscount[]"))]
    pub discounts: Vec<RadrootsCoreDiscount>,
    pub subtotal: RadrootsTradeListingSubtotal,
    pub total: RadrootsTradeListingTotal,
}
