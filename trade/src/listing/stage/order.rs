#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
use radroots_core::RadrootsCoreQuantityPrice;
use radroots_events::listing::{RadrootsListingDiscount, RadrootsListingQuantity};

use crate::listing::model::{RadrootsTradeListingSubtotal, RadrootsTradeListingTotal};

#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct TradeListingOrderRequestPayload {
    pub price: RadrootsCoreQuantityPrice,
    pub quantity: RadrootsListingQuantity,
}

#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct TradeListingOrderRequest {
    pub event: radroots_events::RadrootsNostrEventPtr,
    pub payload: TradeListingOrderRequestPayload,
}

#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct TradeListingOrderResult {
    pub quantity: RadrootsListingQuantity,
    pub price: RadrootsCoreQuantityPrice,
    pub discounts: Vec<RadrootsListingDiscount>,
    pub subtotal: RadrootsTradeListingSubtotal,
    pub total: RadrootsTradeListingTotal,
}
