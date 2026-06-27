#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use radroots_core::{
    RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreUnit,
};

use crate::ids::{RadrootsInventoryBinId, RadrootsOrderQuoteId};

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "dto-bindgen", dto(ts(name = "RadrootsOrderItem")))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOrderItem {
    pub bin_id: RadrootsInventoryBinId,
    pub bin_count: u32,
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "dto-bindgen", dto(ts(name = "RadrootsOrderPricingBasis")))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsOrderPricingBasis {
    #[cfg_attr(feature = "serde", serde(rename = "listing_event"))]
    ListingEvent,
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(
    feature = "dto-bindgen",
    dto(ts(name = "RadrootsOrderEconomicLineKind"))
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsOrderEconomicLineKind {
    #[cfg_attr(feature = "serde", serde(rename = "listing_discount"))]
    ListingDiscount,
    #[cfg_attr(feature = "serde", serde(rename = "basket_adjustment"))]
    BasketAdjustment,
    #[cfg_attr(feature = "serde", serde(rename = "revision_adjustment"))]
    RevisionAdjustment,
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "dto-bindgen", dto(ts(name = "RadrootsOrderEconomicActor")))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsOrderEconomicActor {
    #[cfg_attr(feature = "serde", serde(rename = "buyer"))]
    Buyer,
    #[cfg_attr(feature = "serde", serde(rename = "seller"))]
    Seller,
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "dto-bindgen", dto(ts(name = "RadrootsOrderEconomicEffect")))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsOrderEconomicEffect {
    #[cfg_attr(feature = "serde", serde(rename = "increase"))]
    Increase,
    #[cfg_attr(feature = "serde", serde(rename = "decrease"))]
    Decrease,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOrderEconomicItem {
    pub bin_id: RadrootsInventoryBinId,
    pub bin_count: u32,
    pub quantity_amount: RadrootsCoreDecimal,
    pub quantity_unit: RadrootsCoreUnit,
    pub unit_price_amount: RadrootsCoreDecimal,
    pub unit_price_currency: RadrootsCoreCurrency,
    pub line_subtotal: RadrootsCoreMoney,
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "dto-bindgen", dto(ts(name = "RadrootsOrderEconomicLine")))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOrderEconomicLine {
    pub id: String,
    pub kind: RadrootsOrderEconomicLineKind,
    pub actor: RadrootsOrderEconomicActor,
    pub effect: RadrootsOrderEconomicEffect,
    pub amount: RadrootsCoreMoney,
    pub reason: String,
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "dto-bindgen", dto(ts(name = "RadrootsOrderEconomicTotals")))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOrderEconomicTotals {
    pub subtotal: RadrootsCoreMoney,
    pub discount_total: RadrootsCoreMoney,
    pub adjustment_total: RadrootsCoreMoney,
    pub total: RadrootsCoreMoney,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOrderEconomics {
    pub quote_id: RadrootsOrderQuoteId,
    pub quote_version: u32,
    pub pricing_basis: RadrootsOrderPricingBasis,
    pub currency: RadrootsCoreCurrency,
    pub items: Vec<RadrootsOrderEconomicItem>,
    pub discounts: Vec<RadrootsOrderEconomicLine>,
    pub adjustments: Vec<RadrootsOrderEconomicLine>,
    pub subtotal: RadrootsCoreMoney,
    pub discount_total: RadrootsCoreMoney,
    pub adjustment_total: RadrootsCoreMoney,
    pub total: RadrootsCoreMoney,
}
