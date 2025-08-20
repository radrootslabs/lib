#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};
use radroots_core::RadrootsCoreQuantityPrice;
use radroots_events::listing::models::{RadrootsListingDiscount, RadrootsListingQuantity};

use crate::listing::model::{RadrootsTradeListingSubtotal, RadrootsTradeListingTotal};

#[typeshare::typeshare]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct TradeListingOrderRequestPayload {
    pub price: RadrootsCoreQuantityPrice,
    pub quantity: RadrootsListingQuantity,
}

#[typeshare::typeshare]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct TradeListingOrderRequest {
    pub event: radroots_events::RadrootsNostrEventPtr,
    pub payload: TradeListingOrderRequestPayload,
}

#[typeshare::typeshare]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct TradeListingOrderResult {
    pub quantity: RadrootsListingQuantity,
    pub price: RadrootsCoreQuantityPrice,
    pub discounts: Vec<RadrootsListingDiscount>,
    pub subtotal: RadrootsTradeListingSubtotal,
    pub total: RadrootsTradeListingTotal,
}
