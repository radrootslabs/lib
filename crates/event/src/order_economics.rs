#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use radroots_core::{Currency, Decimal, Money, Unit};

use crate::id::{InventoryBinId, OrderQuoteId};

#[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
#[cfg_attr(all(test, feature = "std"), dto(export))]
#[cfg_attr(all(test, feature = "std"), dto(ts(name = "OrderItem")))]
#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderItem {
    pub bin_id: InventoryBinId,
    pub bin_count: u32,
}

#[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
#[cfg_attr(all(test, feature = "std"), dto(export))]
#[cfg_attr(all(test, feature = "std"), dto(ts(name = "OrderPricingBasis")))]
#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(any(feature = "serde", test), serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrderPricingBasis {
    #[cfg_attr(any(feature = "serde", test), serde(rename = "listing_event"))]
    ListingEvent,
}

#[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
#[cfg_attr(all(test, feature = "std"), dto(export))]
#[cfg_attr(all(test, feature = "std"), dto(ts(name = "OrderEconomicLineKind")))]
#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(any(feature = "serde", test), serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrderEconomicLineKind {
    #[cfg_attr(any(feature = "serde", test), serde(rename = "listing_discount"))]
    ListingDiscount,
    #[cfg_attr(any(feature = "serde", test), serde(rename = "basket_adjustment"))]
    BasketAdjustment,
    #[cfg_attr(any(feature = "serde", test), serde(rename = "revision_adjustment"))]
    RevisionAdjustment,
}

#[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
#[cfg_attr(all(test, feature = "std"), dto(export))]
#[cfg_attr(all(test, feature = "std"), dto(ts(name = "OrderEconomicActor")))]
#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(any(feature = "serde", test), serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrderEconomicActor {
    #[cfg_attr(any(feature = "serde", test), serde(rename = "buyer"))]
    Buyer,
    #[cfg_attr(any(feature = "serde", test), serde(rename = "seller"))]
    Seller,
}

#[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
#[cfg_attr(all(test, feature = "std"), dto(export))]
#[cfg_attr(all(test, feature = "std"), dto(ts(name = "OrderEconomicEffect")))]
#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(any(feature = "serde", test), serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrderEconomicEffect {
    #[cfg_attr(any(feature = "serde", test), serde(rename = "increase"))]
    Increase,
    #[cfg_attr(any(feature = "serde", test), serde(rename = "decrease"))]
    Decrease,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
#[cfg_attr(all(test, feature = "std"), dto(export))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderEconomicItem {
    pub bin_id: InventoryBinId,
    pub bin_count: u32,
    #[cfg_attr(all(test, feature = "std"), dto(ts(type = "Decimal")))]
    pub quantity_amount: Decimal,
    #[cfg_attr(all(test, feature = "std"), dto(ts(type = "Unit")))]
    pub quantity_unit: Unit,
    #[cfg_attr(all(test, feature = "std"), dto(ts(type = "Decimal")))]
    pub unit_price_amount: Decimal,
    #[cfg_attr(all(test, feature = "std"), dto(ts(type = "Currency")))]
    pub unit_price_currency: Currency,
    #[cfg_attr(all(test, feature = "std"), dto(ts(type = "Money")))]
    pub line_subtotal: Money,
}

#[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
#[cfg_attr(all(test, feature = "std"), dto(export))]
#[cfg_attr(all(test, feature = "std"), dto(ts(name = "OrderEconomicLine")))]
#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderEconomicLine {
    pub id: String,
    pub kind: OrderEconomicLineKind,
    pub actor: OrderEconomicActor,
    pub effect: OrderEconomicEffect,
    #[cfg_attr(all(test, feature = "std"), dto(ts(type = "Money")))]
    pub amount: Money,
    pub reason: String,
}

#[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
#[cfg_attr(all(test, feature = "std"), dto(export))]
#[cfg_attr(all(test, feature = "std"), dto(ts(name = "OrderEconomicTotals")))]
#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderEconomicTotals {
    #[cfg_attr(all(test, feature = "std"), dto(ts(type = "Money")))]
    pub subtotal: Money,
    #[cfg_attr(all(test, feature = "std"), dto(ts(type = "Money")))]
    pub discount_total: Money,
    #[cfg_attr(all(test, feature = "std"), dto(ts(type = "Money")))]
    pub adjustment_total: Money,
    #[cfg_attr(all(test, feature = "std"), dto(ts(type = "Money")))]
    pub total: Money,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
#[cfg_attr(all(test, feature = "std"), dto(export))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderEconomics {
    pub quote_id: OrderQuoteId,
    pub quote_version: u32,
    pub pricing_basis: OrderPricingBasis,
    #[cfg_attr(all(test, feature = "std"), dto(ts(type = "Currency")))]
    pub currency: Currency,
    pub items: Vec<OrderEconomicItem>,
    pub discounts: Vec<OrderEconomicLine>,
    pub adjustments: Vec<OrderEconomicLine>,
    #[cfg_attr(all(test, feature = "std"), dto(ts(type = "Money")))]
    pub subtotal: Money,
    #[cfg_attr(all(test, feature = "std"), dto(ts(type = "Money")))]
    pub discount_total: Money,
    #[cfg_attr(all(test, feature = "std"), dto(ts(type = "Money")))]
    pub adjustment_total: Money,
    #[cfg_attr(all(test, feature = "std"), dto(ts(type = "Money")))]
    pub total: Money,
}
