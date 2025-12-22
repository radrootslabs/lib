#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
use radroots_core::RadrootsCoreQuantityPrice;
use radroots_events::listing::{RadrootsListingDiscount, RadrootsListingQuantity};
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

use crate::listing::model::{RadrootsTradeListingSubtotal, RadrootsTradeListingTotal};

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct TradeListingOrderRequestPayload {
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsCoreQuantityPrice"))]
    pub price: RadrootsCoreQuantityPrice,
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsListingQuantity"))]
    pub quantity: RadrootsListingQuantity,
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
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsListingQuantity"))]
    pub quantity: RadrootsListingQuantity,
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsCoreQuantityPrice"))]
    pub price: RadrootsCoreQuantityPrice,
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsListingDiscount[]"))]
    pub discounts: Vec<RadrootsListingDiscount>,
    pub subtotal: RadrootsTradeListingSubtotal,
    pub total: RadrootsTradeListingTotal,
}
