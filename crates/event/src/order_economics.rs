#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use radroots_core::{Currency, Decimal, Money, Unit};

use crate::ids::{RadrootsInventoryBinId, RadrootsOrderQuoteId};

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "dto-bindgen", dto(ts(name = "RadrootsOrderItem")))]
#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOrderItem {
    pub bin_id: RadrootsInventoryBinId,
    pub bin_count: u32,
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "dto-bindgen", dto(ts(name = "RadrootsOrderPricingBasis")))]
#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(any(feature = "serde", test), serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsOrderPricingBasis {
    #[cfg_attr(any(feature = "serde", test), serde(rename = "listing_event"))]
    ListingEvent,
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(
    feature = "dto-bindgen",
    dto(ts(name = "RadrootsOrderEconomicLineKind"))
)]
#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(any(feature = "serde", test), serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsOrderEconomicLineKind {
    #[cfg_attr(any(feature = "serde", test), serde(rename = "listing_discount"))]
    ListingDiscount,
    #[cfg_attr(any(feature = "serde", test), serde(rename = "basket_adjustment"))]
    BasketAdjustment,
    #[cfg_attr(any(feature = "serde", test), serde(rename = "revision_adjustment"))]
    RevisionAdjustment,
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "dto-bindgen", dto(ts(name = "RadrootsOrderEconomicActor")))]
#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(any(feature = "serde", test), serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsOrderEconomicActor {
    #[cfg_attr(any(feature = "serde", test), serde(rename = "buyer"))]
    Buyer,
    #[cfg_attr(any(feature = "serde", test), serde(rename = "seller"))]
    Seller,
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "dto-bindgen", dto(ts(name = "RadrootsOrderEconomicEffect")))]
#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(any(feature = "serde", test), serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsOrderEconomicEffect {
    #[cfg_attr(any(feature = "serde", test), serde(rename = "increase"))]
    Increase,
    #[cfg_attr(any(feature = "serde", test), serde(rename = "decrease"))]
    Decrease,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOrderEconomicItem {
    pub bin_id: RadrootsInventoryBinId,
    pub bin_count: u32,
    #[cfg_attr(feature = "dto-bindgen", dto(ts(type = "Decimal")))]
    pub quantity_amount: Decimal,
    #[cfg_attr(feature = "dto-bindgen", dto(ts(type = "Unit")))]
    pub quantity_unit: Unit,
    #[cfg_attr(feature = "dto-bindgen", dto(ts(type = "Decimal")))]
    pub unit_price_amount: Decimal,
    #[cfg_attr(feature = "dto-bindgen", dto(ts(type = "Currency")))]
    pub unit_price_currency: Currency,
    #[cfg_attr(feature = "dto-bindgen", dto(ts(type = "Money")))]
    pub line_subtotal: Money,
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "dto-bindgen", dto(ts(name = "RadrootsOrderEconomicLine")))]
#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOrderEconomicLine {
    pub id: String,
    pub kind: RadrootsOrderEconomicLineKind,
    pub actor: RadrootsOrderEconomicActor,
    pub effect: RadrootsOrderEconomicEffect,
    #[cfg_attr(feature = "dto-bindgen", dto(ts(type = "Money")))]
    pub amount: Money,
    pub reason: String,
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "dto-bindgen", dto(ts(name = "RadrootsOrderEconomicTotals")))]
#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOrderEconomicTotals {
    #[cfg_attr(feature = "dto-bindgen", dto(ts(type = "Money")))]
    pub subtotal: Money,
    #[cfg_attr(feature = "dto-bindgen", dto(ts(type = "Money")))]
    pub discount_total: Money,
    #[cfg_attr(feature = "dto-bindgen", dto(ts(type = "Money")))]
    pub adjustment_total: Money,
    #[cfg_attr(feature = "dto-bindgen", dto(ts(type = "Money")))]
    pub total: Money,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOrderEconomics {
    pub quote_id: RadrootsOrderQuoteId,
    pub quote_version: u32,
    pub pricing_basis: RadrootsOrderPricingBasis,
    #[cfg_attr(feature = "dto-bindgen", dto(ts(type = "Currency")))]
    pub currency: Currency,
    pub items: Vec<RadrootsOrderEconomicItem>,
    pub discounts: Vec<RadrootsOrderEconomicLine>,
    pub adjustments: Vec<RadrootsOrderEconomicLine>,
    #[cfg_attr(feature = "dto-bindgen", dto(ts(type = "Money")))]
    pub subtotal: Money,
    #[cfg_attr(feature = "dto-bindgen", dto(ts(type = "Money")))]
    pub discount_total: Money,
    #[cfg_attr(feature = "dto-bindgen", dto(ts(type = "Money")))]
    pub adjustment_total: Money,
    #[cfg_attr(feature = "dto-bindgen", dto(ts(type = "Money")))]
    pub total: Money,
}
