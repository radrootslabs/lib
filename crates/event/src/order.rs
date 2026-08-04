#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::{
    string::{String, ToString},
    vec::Vec,
};

use crate::envelope::kind::*;
#[cfg(test)]
use crate::id::OrderQuoteId;
use crate::id::{ClassifiedListingAddress, InventoryBinId, OrderId};
#[cfg(test)]
use crate::listing::operational::OperationalListingParseError;
pub use crate::trade::order_economics::*;
#[cfg(test)]
use crate::trade::validation::OperationalListingValidationError;
use radroots_core::{Currency, Decimal, Money};
use radroots_identity::PublicKey;

pub const RADROOTS_COMMERCIAL_LISTING_DOMAIN: &str = "trade:listing";
pub const RADROOTS_ORDER_ENVELOPE_VERSION: u16 = 1;

impl OrderEconomics {
    pub fn canonicalize(&mut self) {
        self.items
            .sort_by(|left, right| left.bin_id.cmp(&right.bin_id));
        self.discounts.sort_by(|left, right| left.id.cmp(&right.id));
        self.adjustments
            .sort_by(|left, right| left.id.cmp(&right.id));
        if let Ok(totals) = self.derived_totals() {
            self.subtotal = totals.subtotal;
            self.discount_total = totals.discount_total;
            self.adjustment_total = totals.adjustment_total;
            self.total = totals.total;
        }
    }

    pub fn canonicalized(&self) -> Self {
        let mut economics = self.clone();
        economics.canonicalize();
        economics
    }

    pub fn derived_totals(&self) -> Result<OrderEconomicTotals, OrderPayloadError> {
        if self.items.is_empty() {
            return Err(OrderPayloadError::MissingEconomicItems);
        }

        let mut subtotal = Money::zero(self.currency);
        for (index, item) in self.items.iter().enumerate() {
            let line_subtotal = validate_economic_item(item, self.currency, index)?;
            subtotal = checked_money_add(&subtotal, &line_subtotal, "subtotal")?;
        }

        let mut discount_total = Money::zero(self.currency);
        for (index, line) in self.discounts.iter().enumerate() {
            validate_economic_line(line, self.currency, "discounts", index)?;
            if line.kind != OrderEconomicLineKind::ListingDiscount {
                return Err(OrderPayloadError::InvalidEconomicLineKind {
                    field: "discounts",
                    index,
                });
            }
            if line.effect != OrderEconomicEffect::Decrease {
                return Err(OrderPayloadError::InvalidEconomicLineEffect {
                    field: "discounts",
                    index,
                });
            }
            discount_total = checked_money_add(&discount_total, &line.amount, "discount_total")?;
        }

        let mut adjustment_total = Money::zero(self.currency);
        let mut total = checked_money_sub_non_negative(&subtotal, &discount_total, "total")?;
        for (index, line) in self.adjustments.iter().enumerate() {
            validate_economic_line(line, self.currency, "adjustments", index)?;
            if line.kind == OrderEconomicLineKind::ListingDiscount {
                return Err(OrderPayloadError::InvalidEconomicLineKind {
                    field: "adjustments",
                    index,
                });
            }
            adjustment_total =
                checked_money_add(&adjustment_total, &line.amount, "adjustment_total")?;
            total = match line.effect {
                OrderEconomicEffect::Increase => checked_money_add(&total, &line.amount, "total")?,
                OrderEconomicEffect::Decrease => {
                    checked_money_sub_non_negative(&total, &line.amount, "total")?
                }
            };
        }

        Ok(OrderEconomicTotals {
            subtotal,
            discount_total,
            adjustment_total,
            total,
        })
    }

    pub fn validate(&self) -> Result<(), OrderPayloadError> {
        validate_required_field(self.quote_id.as_str(), "quote_id")?;
        if self.quote_version == 0 {
            return Err(OrderPayloadError::InvalidQuoteVersion);
        }

        let totals = self.derived_totals()?;
        validate_economic_item_order(&self.items)?;
        validate_economic_line_order(&self.discounts, "discounts")?;
        validate_economic_line_order(&self.adjustments, "adjustments")?;
        validate_total_money(&self.subtotal, self.currency, "subtotal")?;
        validate_total_money(&self.discount_total, self.currency, "discount_total")?;
        validate_total_money(&self.adjustment_total, self.currency, "adjustment_total")?;
        validate_total_money(&self.total, self.currency, "total")?;
        validate_total_matches(&self.subtotal, &totals.subtotal, "subtotal")?;
        validate_total_matches(
            &self.discount_total,
            &totals.discount_total,
            "discount_total",
        )?;
        validate_total_matches(
            &self.adjustment_total,
            &totals.adjustment_total,
            "adjustment_total",
        )?;
        validate_total_matches(&self.total, &totals.total, "total")
    }
}

#[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
#[cfg_attr(all(test, feature = "std"), dto(export))]
#[cfg_attr(all(test, feature = "std"), dto(ts(name = "OrderRequest")))]
#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderRequest {
    pub order_id: OrderId,
    pub listing_addr: ClassifiedListingAddress,
    #[cfg_attr(all(test, feature = "std"), dto(as = "string"))]
    pub buyer_pubkey: PublicKey,
    #[cfg_attr(all(test, feature = "std"), dto(as = "string"))]
    pub seller_pubkey: PublicKey,
    pub items: Vec<OrderItem>,
    pub economics: OrderEconomics,
}

impl OrderRequest {
    pub fn validate(&self) -> Result<(), OrderPayloadError> {
        validate_required_field(self.order_id.as_str(), "order_id")?;
        validate_required_field(self.listing_addr.as_str(), "listing_addr")?;
        validate_order_items(&self.items)?;
        self.economics.validate()?;
        validate_order_economics_binding(&self.items, &self.economics)
    }
}

#[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
#[cfg_attr(all(test, feature = "std"), dto(export))]
#[cfg_attr(all(test, feature = "std"), dto(ts(name = "OrderInventoryCommitment")))]
#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderInventoryCommitment {
    pub bin_id: InventoryBinId,
    pub bin_count: u32,
}

#[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
#[cfg_attr(all(test, feature = "std"), dto(export))]
#[cfg_attr(all(test, feature = "std"), dto(ts(name = "OrderDecisionOutcome")))]
#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    any(feature = "serde", test),
    serde(rename_all = "snake_case", tag = "decision")
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OrderDecisionOutcome {
    #[cfg_attr(any(feature = "serde", test), serde(rename = "accepted"))]
    Accepted {
        inventory_commitments: Vec<OrderInventoryCommitment>,
    },
    #[cfg_attr(any(feature = "serde", test), serde(rename = "declined"))]
    Declined { reason: String },
}

impl OrderDecisionOutcome {
    pub fn validate(&self) -> Result<(), OrderPayloadError> {
        match self {
            Self::Accepted {
                inventory_commitments,
            } => validate_inventory_commitments(inventory_commitments),
            Self::Declined { reason } => validate_required_field(reason, "reason"),
        }
    }
}

#[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
#[cfg_attr(all(test, feature = "std"), dto(export))]
#[cfg_attr(all(test, feature = "std"), dto(ts(name = "OrderDecision")))]
#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderDecision {
    pub order_id: OrderId,
    pub listing_addr: ClassifiedListingAddress,
    #[cfg_attr(all(test, feature = "std"), dto(as = "string"))]
    pub buyer_pubkey: PublicKey,
    #[cfg_attr(all(test, feature = "std"), dto(as = "string"))]
    pub seller_pubkey: PublicKey,
    pub decision: OrderDecisionOutcome,
}

impl OrderDecision {
    pub fn validate(&self) -> Result<(), OrderPayloadError> {
        validate_required_field(self.order_id.as_str(), "order_id")?;
        validate_required_field(self.listing_addr.as_str(), "listing_addr")?;
        self.decision.validate()
    }
}

#[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
#[cfg_attr(all(test, feature = "std"), dto(export))]
#[cfg_attr(all(test, feature = "std"), dto(ts(name = "OrderCancellation")))]
#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderCancellation {
    pub order_id: OrderId,
    pub listing_addr: ClassifiedListingAddress,
    #[cfg_attr(all(test, feature = "std"), dto(as = "string"))]
    pub buyer_pubkey: PublicKey,
    #[cfg_attr(all(test, feature = "std"), dto(as = "string"))]
    pub seller_pubkey: PublicKey,
    pub reason: String,
}

impl OrderCancellation {
    pub fn validate(&self) -> Result<(), OrderPayloadError> {
        validate_required_field(self.order_id.as_str(), "order_id")?;
        validate_required_field(self.listing_addr.as_str(), "listing_addr")?;
        validate_required_field(&self.reason, "reason")
    }
}

#[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
#[cfg_attr(all(test, feature = "std"), dto(export))]
#[cfg_attr(all(test, feature = "std"), dto(ts(name = "CommercialDomain")))]
#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(any(feature = "serde", test), serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommercialDomain {
    #[cfg_attr(any(feature = "serde", test), serde(rename = "trade:listing"))]
    Listing,
}

#[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
#[cfg_attr(all(test, feature = "std"), dto(export))]
#[cfg_attr(all(test, feature = "std"), dto(ts(name = "OrderEventType")))]
#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrderEventType {
    #[cfg_attr(any(feature = "serde", test), serde(rename = "TradeOrderRequested"))]
    OrderRequested,
    #[cfg_attr(any(feature = "serde", test), serde(rename = "TradeOrderDecision"))]
    OrderDecision,
    #[cfg_attr(any(feature = "serde", test), serde(rename = "TradeOrderCancelled"))]
    OrderCancelled,
}

impl OrderEventType {
    #[inline]
    pub const fn from_kind(kind: u32) -> Option<Self> {
        match kind {
            KIND_ORDER_REQUEST => Some(Self::OrderRequested),
            KIND_ORDER_DECISION => Some(Self::OrderDecision),
            KIND_ORDER_CANCELLATION => Some(Self::OrderCancelled),
            _ => None,
        }
    }

    #[inline]
    pub const fn kind(self) -> u32 {
        match self {
            Self::OrderRequested => KIND_ORDER_REQUEST,
            Self::OrderDecision => KIND_ORDER_DECISION,
            Self::OrderCancelled => KIND_ORDER_CANCELLATION,
        }
    }

    #[inline]
    pub const fn name(self) -> &'static str {
        match self {
            Self::OrderRequested => "TradeOrderRequested",
            Self::OrderDecision => "TradeOrderDecision",
            Self::OrderCancelled => "TradeOrderCancelled",
        }
    }

    #[inline]
    pub const fn requires_listing_snapshot(self) -> bool {
        matches!(self, Self::OrderRequested)
    }

    #[inline]
    pub const fn requires_order_chain(self) -> bool {
        matches!(self, Self::OrderDecision | Self::OrderCancelled)
    }
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderEnvelope<T> {
    pub version: u16,
    pub domain: CommercialDomain,
    #[cfg_attr(any(feature = "serde", test), serde(rename = "type"))]
    pub message_type: OrderEventType,
    pub order_id: String,
    pub listing_addr: String,
    pub payload: T,
}

impl<T> OrderEnvelope<T> {
    #[inline]
    pub fn new(
        message_type: OrderEventType,
        listing_addr: impl Into<String>,
        order_id: impl Into<String>,
        payload: T,
    ) -> Self {
        Self {
            version: RADROOTS_ORDER_ENVELOPE_VERSION,
            domain: CommercialDomain::Listing,
            message_type,
            order_id: order_id.into(),
            listing_addr: listing_addr.into(),
            payload,
        }
    }

    pub fn validate(&self) -> Result<(), OrderEnvelopeError> {
        if self.version != RADROOTS_ORDER_ENVELOPE_VERSION {
            return Err(OrderEnvelopeError::InvalidVersion {
                expected: RADROOTS_ORDER_ENVELOPE_VERSION,
                got: self.version,
            });
        }
        if self.order_id.trim().is_empty() {
            return Err(OrderEnvelopeError::MissingOrderId);
        }
        if self.listing_addr.trim().is_empty() {
            return Err(OrderEnvelopeError::MissingListingAddr);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrderEnvelopeError {
    InvalidVersion { expected: u16, got: u16 },
    MissingOrderId,
    MissingListingAddr,
}

impl core::fmt::Display for OrderEnvelopeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidVersion { expected, got } => {
                write!(
                    f,
                    "invalid order envelope version: expected {expected}, got {got}"
                )
            }
            Self::MissingOrderId => write!(f, "missing order_id for order message"),
            Self::MissingListingAddr => write!(f, "missing listing_addr"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for OrderEnvelopeError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrderPayloadError {
    EmptyField(&'static str),
    MissingItems,
    InvalidItemBinCount { index: usize },
    MissingEconomicItems,
    InvalidEconomicItemBinCount { index: usize },
    InvalidEconomicItemQuantity { index: usize },
    InvalidEconomicItemPrice { index: usize },
    InvalidEconomicItemSubtotal { index: usize },
    InvalidEconomicLineAmount { field: &'static str, index: usize },
    InvalidEconomicLineKind { field: &'static str, index: usize },
    InvalidEconomicLineEffect { field: &'static str, index: usize },
    InvalidEconomicCurrency { field: &'static str },
    InvalidEconomicOrdering { field: &'static str },
    InvalidEconomicTotal { field: &'static str },
    InvalidOrderEconomicsBinding { field: &'static str },
    InvalidQuoteVersion,
    MissingInventoryCommitments,
    InvalidInventoryCommitmentCount { index: usize },
}

impl core::fmt::Display for OrderPayloadError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::EmptyField(field) => write!(f, "{field} cannot be empty"),
            Self::MissingItems => write!(f, "items must contain at least one item"),
            Self::InvalidItemBinCount { index } => {
                write!(f, "items[{index}].bin_count must be greater than zero")
            }
            Self::MissingEconomicItems => {
                write!(f, "economics.items must contain at least one item")
            }
            Self::InvalidEconomicItemBinCount { index } => write!(
                f,
                "economics.items[{index}].bin_count must be greater than zero"
            ),
            Self::InvalidEconomicItemQuantity { index } => write!(
                f,
                "economics.items[{index}].quantity_amount must be greater than zero"
            ),
            Self::InvalidEconomicItemPrice { index } => write!(
                f,
                "economics.items[{index}].unit_price_amount must not be negative"
            ),
            Self::InvalidEconomicItemSubtotal { index } => {
                write!(f, "economics.items[{index}].line_subtotal is invalid")
            }
            Self::InvalidEconomicLineAmount { field, index } => {
                write!(
                    f,
                    "economics.{field}[{index}].amount must be greater than zero"
                )
            }
            Self::InvalidEconomicLineKind { field, index } => {
                write!(f, "economics.{field}[{index}].kind is invalid")
            }
            Self::InvalidEconomicLineEffect { field, index } => {
                write!(f, "economics.{field}[{index}].effect is invalid")
            }
            Self::InvalidEconomicCurrency { field } => {
                write!(f, "economics.{field} currency is invalid")
            }
            Self::InvalidEconomicOrdering { field } => {
                write!(f, "economics.{field} is not in canonical order")
            }
            Self::InvalidEconomicTotal { field } => {
                write!(f, "economics.{field} total is invalid")
            }
            Self::InvalidOrderEconomicsBinding { field } => {
                write!(f, "order {field} does not match economics")
            }
            Self::InvalidQuoteVersion => {
                write!(f, "economics.quote_version must be greater than zero")
            }
            Self::MissingInventoryCommitments => {
                write!(
                    f,
                    "accepted decisions must contain at least one inventory commitment"
                )
            }
            Self::InvalidInventoryCommitmentCount { index } => write!(
                f,
                "inventory_commitments[{index}].bin_count must be greater than zero"
            ),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for OrderPayloadError {}

fn validate_required_field(value: &str, field: &'static str) -> Result<(), OrderPayloadError> {
    if value.trim().is_empty() {
        Err(OrderPayloadError::EmptyField(field))
    } else {
        Ok(())
    }
}

fn validate_order_items(items: &[OrderItem]) -> Result<(), OrderPayloadError> {
    if items.is_empty() {
        return Err(OrderPayloadError::MissingItems);
    }
    for (index, item) in items.iter().enumerate() {
        validate_required_field(item.bin_id.as_str(), "bin_id")?;
        if item.bin_count == 0 {
            return Err(OrderPayloadError::InvalidItemBinCount { index });
        }
    }
    Ok(())
}

fn validate_economic_item(
    item: &OrderEconomicItem,
    expected_currency: Currency,
    index: usize,
) -> Result<Money, OrderPayloadError> {
    validate_required_field(item.bin_id.as_str(), "economics.items.bin_id")?;
    if item.bin_count == 0 {
        return Err(OrderPayloadError::InvalidEconomicItemBinCount { index });
    }
    if item.quantity_amount.is_zero() || item.quantity_amount.is_sign_negative() {
        return Err(OrderPayloadError::InvalidEconomicItemQuantity { index });
    }
    if item.unit_price_amount.is_sign_negative() {
        return Err(OrderPayloadError::InvalidEconomicItemPrice { index });
    }
    if item.unit_price_currency != expected_currency {
        return Err(OrderPayloadError::InvalidEconomicCurrency {
            field: "items.unit_price_currency",
        });
    }
    validate_total_money(
        &item.line_subtotal,
        expected_currency,
        "items.line_subtotal",
    )?;

    let quantity_total = checked_decimal_mul(item.quantity_amount, Decimal::from(item.bin_count))
        .ok_or(OrderPayloadError::InvalidEconomicItemSubtotal { index })?;
    let expected_subtotal = checked_decimal_mul(item.unit_price_amount, quantity_total)
        .ok_or(OrderPayloadError::InvalidEconomicItemSubtotal { index })?;
    if item.line_subtotal.amount() != expected_subtotal {
        return Err(OrderPayloadError::InvalidEconomicItemSubtotal { index });
    }
    Ok(item.line_subtotal.clone())
}

fn validate_order_economics_binding(
    items: &[OrderItem],
    economics: &OrderEconomics,
) -> Result<(), OrderPayloadError> {
    let order_items = normalized_order_item_counts(items).ok_or(
        OrderPayloadError::InvalidOrderEconomicsBinding {
            field: "items.bin_count",
        },
    )?;
    if order_items.len() != economics.items.len() {
        return Err(OrderPayloadError::InvalidOrderEconomicsBinding { field: "items" });
    }
    for (item, economic_item) in order_items.iter().zip(economics.items.iter()) {
        if item.bin_id != economic_item.bin_id.as_str() {
            return Err(OrderPayloadError::InvalidOrderEconomicsBinding {
                field: "items.bin_id",
            });
        }
        if item.bin_count != u64::from(economic_item.bin_count) {
            return Err(OrderPayloadError::InvalidOrderEconomicsBinding {
                field: "items.bin_count",
            });
        }
    }
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
struct NormalizedOrderItemCount {
    bin_id: String,
    bin_count: u64,
}

fn normalized_order_item_counts(items: &[OrderItem]) -> Option<Vec<NormalizedOrderItemCount>> {
    let mut counts: Vec<NormalizedOrderItemCount> = Vec::new();
    for item in items {
        let bin_id = item.bin_id.as_str();
        if item.bin_count == 0 {
            return None;
        }
        if let Some(existing) = counts.iter_mut().find(|count| count.bin_id == bin_id) {
            existing.bin_count = existing.bin_count.checked_add(u64::from(item.bin_count))?;
        } else {
            counts.push(NormalizedOrderItemCount {
                bin_id: bin_id.to_string(),
                bin_count: u64::from(item.bin_count),
            });
        }
    }
    counts.sort_by(|left, right| left.bin_id.cmp(&right.bin_id));
    Some(counts)
}

fn validate_economic_line(
    line: &OrderEconomicLine,
    expected_currency: Currency,
    field: &'static str,
    index: usize,
) -> Result<(), OrderPayloadError> {
    validate_required_field(&line.id, "economics.line.id")?;
    validate_required_field(&line.reason, "economics.line.reason")?;
    if line.amount.currency() != expected_currency {
        return Err(OrderPayloadError::InvalidEconomicCurrency { field });
    }
    if line.amount.amount().is_zero() || line.amount.amount().is_sign_negative() {
        return Err(OrderPayloadError::InvalidEconomicLineAmount { field, index });
    }
    Ok(())
}

fn validate_economic_item_order(items: &[OrderEconomicItem]) -> Result<(), OrderPayloadError> {
    for pair in items.windows(2) {
        if pair[0].bin_id >= pair[1].bin_id {
            return Err(OrderPayloadError::InvalidEconomicOrdering {
                field: "items.bin_id",
            });
        }
    }
    Ok(())
}

fn validate_economic_line_order(
    lines: &[OrderEconomicLine],
    field: &'static str,
) -> Result<(), OrderPayloadError> {
    for pair in lines.windows(2) {
        if pair[0].id >= pair[1].id {
            return Err(OrderPayloadError::InvalidEconomicOrdering { field });
        }
    }
    Ok(())
}

fn validate_total_money(
    money: &Money,
    expected_currency: Currency,
    field: &'static str,
) -> Result<(), OrderPayloadError> {
    if money.currency() != expected_currency {
        return Err(OrderPayloadError::InvalidEconomicCurrency { field });
    }
    if money.amount().is_sign_negative() {
        return Err(OrderPayloadError::InvalidEconomicTotal { field });
    }
    Ok(())
}

fn validate_total_matches(
    actual: &Money,
    expected: &Money,
    field: &'static str,
) -> Result<(), OrderPayloadError> {
    if actual.currency() != expected.currency() {
        return Err(OrderPayloadError::InvalidEconomicCurrency { field });
    }
    if actual.amount() != expected.amount() {
        return Err(OrderPayloadError::InvalidEconomicTotal { field });
    }
    Ok(())
}

fn checked_decimal_add(left: Decimal, right: Decimal) -> Option<Decimal> {
    left.checked_add(right).ok()
}

fn checked_decimal_sub(left: Decimal, right: Decimal) -> Option<Decimal> {
    left.checked_sub(right).ok()
}

fn checked_decimal_mul(left: Decimal, right: Decimal) -> Option<Decimal> {
    left.checked_mul(right).ok()
}

fn checked_money_add(
    left: &Money,
    right: &Money,
    field: &'static str,
) -> Result<Money, OrderPayloadError> {
    if left.currency() != right.currency() {
        return Err(OrderPayloadError::InvalidEconomicCurrency { field });
    }
    let amount = checked_decimal_add(left.amount(), right.amount())
        .ok_or(OrderPayloadError::InvalidEconomicTotal { field })?;
    Money::try_new(amount, left.currency())
        .map_err(|_| OrderPayloadError::InvalidEconomicTotal { field })
}

fn checked_money_sub_non_negative(
    left: &Money,
    right: &Money,
    field: &'static str,
) -> Result<Money, OrderPayloadError> {
    if left.currency() != right.currency() {
        return Err(OrderPayloadError::InvalidEconomicCurrency { field });
    }
    let amount = checked_decimal_sub(left.amount(), right.amount())
        .ok_or(OrderPayloadError::InvalidEconomicTotal { field })?;
    if amount.is_sign_negative() {
        return Err(OrderPayloadError::InvalidEconomicTotal { field });
    }
    Money::try_new(amount, left.currency())
        .map_err(|_| OrderPayloadError::InvalidEconomicTotal { field })
}

fn validate_inventory_commitments(
    commitments: &[OrderInventoryCommitment],
) -> Result<(), OrderPayloadError> {
    if commitments.is_empty() {
        return Err(OrderPayloadError::MissingInventoryCommitments);
    }
    for (index, commitment) in commitments.iter().enumerate() {
        validate_required_field(commitment.bin_id.as_str(), "bin_id")?;
        if commitment.bin_count == 0 {
            return Err(OrderPayloadError::InvalidInventoryCommitmentCount { index });
        }
    }
    Ok(())
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use radroots_core::{Currency, Decimal, Money, Unit};

    fn pubkey(character: char) -> PublicKey {
        crate::test_valid_hex_64(character).parse().unwrap()
    }

    fn buyer_pubkey() -> PublicKey {
        pubkey('b')
    }

    fn seller_pubkey() -> PublicKey {
        pubkey('a')
    }

    fn sample_listing_addr() -> ClassifiedListingAddress {
        format!("30402:{}:AAAAAAAAAAAAAAAAAAAAAg", seller_pubkey())
            .parse()
            .unwrap()
    }

    fn order_id(raw: &str) -> OrderId {
        raw.parse().unwrap()
    }

    fn quote_id(raw: &str) -> OrderQuoteId {
        raw.parse().unwrap()
    }

    fn bin_id(raw: &str) -> InventoryBinId {
        raw.parse().unwrap()
    }

    fn sample_order_request() -> OrderRequest {
        OrderRequest {
            order_id: order_id("order-1"),
            listing_addr: sample_listing_addr(),
            buyer_pubkey: buyer_pubkey(),
            seller_pubkey: seller_pubkey(),
            items: vec![OrderItem {
                bin_id: bin_id("bin-1"),
                bin_count: 2,
            }],
            economics: sample_bound_order_economics(),
        }
    }

    fn decimal(raw: &str) -> Decimal {
        raw.parse().unwrap()
    }

    fn usd(raw: &str) -> Money {
        money(raw, Currency::USD)
    }

    fn money(raw: &str, currency: Currency) -> Money {
        Money::try_new(decimal(raw), currency).unwrap()
    }

    fn sample_order_economics() -> OrderEconomics {
        OrderEconomics {
            quote_id: quote_id("quote-1"),
            quote_version: 1,
            pricing_basis: OrderPricingBasis::ListingEvent,
            currency: Currency::USD,
            items: vec![
                OrderEconomicItem {
                    bin_id: bin_id("bin-a"),
                    bin_count: 2,
                    quantity_amount: decimal("1.5"),
                    quantity_unit: Unit::Each,
                    unit_price_amount: decimal("4"),
                    unit_price_currency: Currency::USD,
                    line_subtotal: usd("12"),
                },
                OrderEconomicItem {
                    bin_id: bin_id("bin-b"),
                    bin_count: 1,
                    quantity_amount: decimal("2"),
                    quantity_unit: Unit::Each,
                    unit_price_amount: decimal("3"),
                    unit_price_currency: Currency::USD,
                    line_subtotal: usd("6"),
                },
            ],
            discounts: vec![OrderEconomicLine {
                id: "discount-a".into(),
                kind: OrderEconomicLineKind::ListingDiscount,
                actor: OrderEconomicActor::Seller,
                effect: OrderEconomicEffect::Decrease,
                amount: usd("3"),
                reason: "farmstand pickup".into(),
            }],
            adjustments: vec![
                OrderEconomicLine {
                    id: "adjustment-a".into(),
                    kind: OrderEconomicLineKind::BasketAdjustment,
                    actor: OrderEconomicActor::Buyer,
                    effect: OrderEconomicEffect::Increase,
                    amount: usd("2"),
                    reason: "special handling".into(),
                },
                OrderEconomicLine {
                    id: "adjustment-b".into(),
                    kind: OrderEconomicLineKind::BasketAdjustment,
                    actor: OrderEconomicActor::Buyer,
                    effect: OrderEconomicEffect::Decrease,
                    amount: usd("1"),
                    reason: "local pickup credit".into(),
                },
            ],
            subtotal: usd("18"),
            discount_total: usd("3"),
            adjustment_total: usd("3"),
            total: usd("16"),
        }
    }

    fn sample_bound_order_economics() -> OrderEconomics {
        OrderEconomics {
            quote_id: quote_id("quote-bound-1"),
            quote_version: 1,
            pricing_basis: OrderPricingBasis::ListingEvent,
            currency: Currency::USD,
            items: vec![OrderEconomicItem {
                bin_id: bin_id("bin-1"),
                bin_count: 2,
                quantity_amount: decimal("1"),
                quantity_unit: Unit::Each,
                unit_price_amount: decimal("5"),
                unit_price_currency: Currency::USD,
                line_subtotal: usd("10"),
            }],
            discounts: Vec::new(),
            adjustments: Vec::new(),
            subtotal: usd("10"),
            discount_total: usd("0"),
            adjustment_total: usd("0"),
            total: usd("10"),
        }
    }

    fn sample_inventory_commitment() -> OrderInventoryCommitment {
        OrderInventoryCommitment {
            bin_id: bin_id("bin-1"),
            bin_count: 2,
        }
    }

    fn sample_order_decision() -> OrderDecision {
        OrderDecision {
            order_id: order_id("order-1"),
            listing_addr: sample_listing_addr(),
            buyer_pubkey: buyer_pubkey(),
            seller_pubkey: seller_pubkey(),
            decision: OrderDecisionOutcome::Accepted {
                inventory_commitments: vec![sample_inventory_commitment()],
            },
        }
    }

    fn sample_order_cancellation() -> OrderCancellation {
        OrderCancellation {
            order_id: order_id("order-1"),
            listing_addr: sample_listing_addr(),
            buyer_pubkey: buyer_pubkey(),
            seller_pubkey: seller_pubkey(),
            reason: "changed plans".into(),
        }
    }

    #[test]
    fn order_message_type_uses_canonical_names_and_kinds() {
        assert_eq!(
            OrderEventType::from_kind(KIND_ORDER_REQUEST),
            Some(OrderEventType::OrderRequested)
        );
        assert_eq!(
            OrderEventType::from_kind(KIND_ORDER_DECISION),
            Some(OrderEventType::OrderDecision)
        );
        assert_eq!(OrderEventType::from_kind(3424), None);
        assert_eq!(OrderEventType::from_kind(3425), None);
        assert_eq!(
            OrderEventType::from_kind(KIND_ORDER_CANCELLATION),
            Some(OrderEventType::OrderCancelled)
        );
        assert_eq!(OrderEventType::from_kind(3433), None);
        assert_eq!(OrderEventType::from_kind(3434), None);
        assert_eq!(OrderEventType::from_kind(3435), None);
        assert_eq!(OrderEventType::from_kind(3436), None);
        assert_eq!(OrderEventType::from_kind(3431), None);
        assert_eq!(OrderEventType::OrderRequested.kind(), KIND_ORDER_REQUEST);
        assert_eq!(OrderEventType::OrderDecision.kind(), KIND_ORDER_DECISION);
        assert_eq!(
            OrderEventType::OrderCancelled.kind(),
            KIND_ORDER_CANCELLATION
        );
        assert_eq!(OrderEventType::OrderRequested.name(), "TradeOrderRequested");
        assert_eq!(OrderEventType::OrderDecision.name(), "TradeOrderDecision");
        assert_eq!(OrderEventType::OrderCancelled.name(), "TradeOrderCancelled");
        assert!(OrderEventType::OrderRequested.requires_listing_snapshot());
        assert!(OrderEventType::OrderDecision.requires_order_chain());
        assert!(OrderEventType::OrderCancelled.requires_order_chain());
        assert!(!OrderEventType::OrderRequested.requires_order_chain());

        let request_name = serde_json::to_value(OrderEventType::OrderRequested).unwrap();
        let decision_name = serde_json::to_value(OrderEventType::OrderDecision).unwrap();
        let cancellation_name = serde_json::to_value(OrderEventType::OrderCancelled).unwrap();
        assert_eq!(request_name, serde_json::json!("TradeOrderRequested"));
        assert_eq!(decision_name, serde_json::json!("TradeOrderDecision"));
        assert_eq!(cancellation_name, serde_json::json!("TradeOrderCancelled"));
    }

    #[test]
    fn order_request_validation_rejects_invalid_fields() {
        assert_eq!(sample_order_request().validate(), Ok(()));

        let mut missing_items = sample_order_request();
        missing_items.items.clear();
        assert_eq!(
            missing_items.validate().unwrap_err(),
            OrderPayloadError::MissingItems
        );

        let mut invalid_count = sample_order_request();
        invalid_count.items[0].bin_count = 0;
        assert_eq!(
            invalid_count.validate().unwrap_err(),
            OrderPayloadError::InvalidItemBinCount { index: 0 }
        );

        let mut mismatched_economic_item = sample_order_request();
        mismatched_economic_item.economics.items[0].bin_id = bin_id("bin-other");
        assert_eq!(
            mismatched_economic_item.validate().unwrap_err(),
            OrderPayloadError::InvalidOrderEconomicsBinding {
                field: "items.bin_id"
            }
        );

        let mut mismatched_economic_count = sample_order_request();
        mismatched_economic_count.economics.items[0].bin_count = 3;
        mismatched_economic_count.economics.items[0].line_subtotal = usd("15");
        mismatched_economic_count.economics.subtotal = usd("15");
        mismatched_economic_count.economics.total = usd("15");
        assert_eq!(
            mismatched_economic_count.validate().unwrap_err(),
            OrderPayloadError::InvalidOrderEconomicsBinding {
                field: "items.bin_count"
            }
        );
    }

    #[test]
    fn order_payload_json_rejects_invalid_protocol_identifiers() {
        let mut request = serde_json::to_value(sample_order_request()).unwrap();
        request["buyer_pubkey"] = serde_json::json!("not-a-pubkey");
        assert!(serde_json::from_value::<OrderRequest>(request).is_err());
    }

    #[test]
    fn listing_parse_error_json_preserves_external_tagged_shape() {
        assert_eq!(
            serde_json::to_value(OperationalListingParseError::InvalidKind(KIND_PROFILE)).unwrap(),
            serde_json::json!({ "InvalidKind": KIND_PROFILE })
        );
        assert_eq!(
            serde_json::to_value(OperationalListingParseError::MissingTag("price".into())).unwrap(),
            serde_json::json!({ "MissingTag": "price" })
        );
        assert_eq!(
            serde_json::to_value(OperationalListingParseError::InvalidUnit).unwrap(),
            serde_json::json!("InvalidUnit")
        );
        assert_eq!(
            serde_json::from_value::<OperationalListingParseError>(serde_json::json!({
                "InvalidJson": "bins"
            }))
            .unwrap(),
            OperationalListingParseError::InvalidJson("bins".into())
        );
    }

    #[test]
    fn order_economics_validation_accepts_canonical_totals() {
        let economics = sample_order_economics();
        assert_eq!(economics.validate(), Ok(()));

        let totals = economics.derived_totals().unwrap();
        assert_eq!(totals.subtotal, usd("18"));
        assert_eq!(totals.discount_total, usd("3"));
        assert_eq!(totals.adjustment_total, usd("3"));
        assert_eq!(totals.total, usd("16"));

        let json = serde_json::to_value(&economics).unwrap();
        assert_eq!(json["pricing_basis"], serde_json::json!("listing_event"));
        assert_eq!(
            json["discounts"][0]["kind"],
            serde_json::json!("listing_discount")
        );
        assert_eq!(
            json["adjustments"][0]["effect"],
            serde_json::json!("increase")
        );
    }

    #[test]
    fn order_economics_canonicalized_sorts_items_and_lines() {
        let mut economics = sample_order_economics();
        economics.items.reverse();
        economics.adjustments.reverse();
        economics.discounts.push(OrderEconomicLine {
            id: "discount-b".into(),
            kind: OrderEconomicLineKind::ListingDiscount,
            actor: OrderEconomicActor::Seller,
            effect: OrderEconomicEffect::Decrease,
            amount: usd("1"),
            reason: "market credit".into(),
        });
        economics.discounts.reverse();
        economics.subtotal = usd("19");
        economics.total = usd("17");
        assert_eq!(
            economics.validate().unwrap_err(),
            OrderPayloadError::InvalidEconomicOrdering {
                field: "items.bin_id"
            }
        );

        let canonical = economics.canonicalized();
        assert_eq!(canonical.items[0].bin_id.as_str(), "bin-a");
        assert_eq!(canonical.discounts[0].id, "discount-a");
        assert_eq!(canonical.adjustments[0].id, "adjustment-a");
        assert_eq!(canonical.subtotal, usd("18"));
        assert_eq!(canonical.discount_total, usd("4"));
        assert_eq!(canonical.total, usd("15"));
        assert_eq!(canonical.validate(), Ok(()));

        let mut uncanonicalizable = sample_order_economics();
        uncanonicalizable.items.clear();
        uncanonicalizable.subtotal = usd("88");
        uncanonicalizable.canonicalize();
        assert_eq!(uncanonicalizable.subtotal, usd("88"));
    }

    #[test]
    fn order_economics_validation_rejects_mixed_currency() {
        let mut economics = sample_order_economics();
        economics.items[0].unit_price_currency = Currency::EUR;
        assert_eq!(
            economics.validate().unwrap_err(),
            OrderPayloadError::InvalidEconomicCurrency {
                field: "items.unit_price_currency"
            }
        );

        let mut economics = sample_order_economics();
        economics.adjustments[0].amount = money("2", Currency::EUR);
        assert_eq!(
            economics.validate().unwrap_err(),
            OrderPayloadError::InvalidEconomicCurrency {
                field: "adjustments"
            }
        );
    }

    #[test]
    fn order_economics_validation_rejects_bad_subtotal() {
        let mut economics = sample_order_economics();
        economics.items[0].bin_count = 0;
        assert_eq!(
            economics.validate().unwrap_err(),
            OrderPayloadError::InvalidEconomicItemBinCount { index: 0 }
        );

        let mut economics = sample_order_economics();
        economics.items[0].line_subtotal = usd("11.99");
        assert_eq!(
            economics.validate().unwrap_err(),
            OrderPayloadError::InvalidEconomicItemSubtotal { index: 0 }
        );

        let mut economics = sample_order_economics();
        economics.items[0].line_subtotal = money("12", Currency::EUR);
        assert_eq!(
            economics.validate().unwrap_err(),
            OrderPayloadError::InvalidEconomicCurrency {
                field: "items.line_subtotal"
            }
        );
    }

    #[test]
    fn order_economics_validation_covers_remaining_error_paths() {
        let mut economics = sample_order_economics();
        economics.items.clear();
        assert_eq!(
            economics.derived_totals().unwrap_err(),
            OrderPayloadError::MissingEconomicItems
        );

        let mut economics = sample_order_economics();
        economics.quote_version = 0;
        assert_eq!(
            economics.validate().unwrap_err(),
            OrderPayloadError::InvalidQuoteVersion
        );

        let mut economics = sample_order_economics();
        economics.items[0].quantity_amount = decimal("0");
        assert_eq!(
            economics.validate().unwrap_err(),
            OrderPayloadError::InvalidEconomicItemQuantity { index: 0 }
        );

        let mut economics = sample_order_economics();
        economics.items[0].quantity_amount = decimal("-1");
        assert_eq!(
            economics.validate().unwrap_err(),
            OrderPayloadError::InvalidEconomicItemQuantity { index: 0 }
        );

        let mut economics = sample_order_economics();
        economics.items[0].unit_price_amount = decimal("-1");
        assert_eq!(
            economics.validate().unwrap_err(),
            OrderPayloadError::InvalidEconomicItemPrice { index: 0 }
        );

        let mut economics = sample_order_economics();
        economics.discounts[0].kind = OrderEconomicLineKind::BasketAdjustment;
        assert_eq!(
            economics.validate().unwrap_err(),
            OrderPayloadError::InvalidEconomicLineKind {
                field: "discounts",
                index: 0
            }
        );

        let mut economics = sample_order_economics();
        economics.subtotal = money("18", Currency::EUR);
        assert_eq!(
            economics.validate().unwrap_err(),
            OrderPayloadError::InvalidEconomicCurrency { field: "subtotal" }
        );

        let mut economics = sample_order_economics();
        economics.discount_total = usd("4");
        assert_eq!(
            economics.validate().unwrap_err(),
            OrderPayloadError::InvalidEconomicTotal {
                field: "discount_total"
            }
        );

        let mut economics = sample_order_economics();
        economics.adjustment_total = usd("4");
        assert_eq!(
            economics.validate().unwrap_err(),
            OrderPayloadError::InvalidEconomicTotal {
                field: "adjustment_total"
            }
        );

        let economics = sample_bound_order_economics();
        assert_eq!(
            validate_order_economics_binding(&[], &economics).unwrap_err(),
            OrderPayloadError::InvalidOrderEconomicsBinding { field: "items" }
        );

        let invalid_order_items = [OrderItem {
            bin_id: bin_id("bin-1"),
            bin_count: 0,
        }];
        assert_eq!(
            validate_order_economics_binding(&invalid_order_items, &economics).unwrap_err(),
            OrderPayloadError::InvalidOrderEconomicsBinding {
                field: "items.bin_count"
            }
        );

        let duplicate_counts = normalized_order_item_counts(&[
            OrderItem {
                bin_id: bin_id("bin-1"),
                bin_count: 1,
            },
            OrderItem {
                bin_id: bin_id("bin-1"),
                bin_count: 2,
            },
        ])
        .unwrap();
        assert_eq!(duplicate_counts[0].bin_count, 3);

        assert!(
            normalized_order_item_counts(&[OrderItem {
                bin_id: bin_id("bin-1"),
                bin_count: 0,
            }])
            .is_none()
        );
        let sorted_counts = normalized_order_item_counts(&[
            OrderItem {
                bin_id: bin_id("bin-b"),
                bin_count: 1,
            },
            OrderItem {
                bin_id: bin_id("bin-a"),
                bin_count: 1,
            },
        ])
        .unwrap();
        assert_eq!(sorted_counts[0].bin_id, "bin-a");
    }

    #[test]
    fn order_economics_validation_rejects_bad_line_semantics() {
        let mut economics = sample_order_economics();
        economics.discounts[0].effect = OrderEconomicEffect::Increase;
        assert_eq!(
            economics.validate().unwrap_err(),
            OrderPayloadError::InvalidEconomicLineEffect {
                field: "discounts",
                index: 0
            }
        );

        let mut economics = sample_order_economics();
        economics.adjustments[0].kind = OrderEconomicLineKind::ListingDiscount;
        assert_eq!(
            economics.validate().unwrap_err(),
            OrderPayloadError::InvalidEconomicLineKind {
                field: "adjustments",
                index: 0
            }
        );

        let mut economics = sample_order_economics();
        economics.adjustments[0].amount = usd("0");
        assert_eq!(
            economics.validate().unwrap_err(),
            OrderPayloadError::InvalidEconomicLineAmount {
                field: "adjustments",
                index: 0
            }
        );
    }

    #[test]
    fn order_economics_helpers_cover_currency_error_paths() {
        assert_eq!(
            validate_total_matches(&usd("1"), &money("1", Currency::EUR), "total").unwrap_err(),
            OrderPayloadError::InvalidEconomicCurrency { field: "total" }
        );
        assert_eq!(
            checked_money_add(&usd("1"), &money("1", Currency::EUR), "subtotal").unwrap_err(),
            OrderPayloadError::InvalidEconomicCurrency { field: "subtotal" }
        );
        assert_eq!(
            checked_money_sub_non_negative(&usd("1"), &money("1", Currency::EUR), "total")
                .unwrap_err(),
            OrderPayloadError::InvalidEconomicCurrency { field: "total" }
        );
    }

    #[test]
    fn order_economics_validation_rejects_duplicate_line_ids() {
        let mut economics = sample_order_economics();
        economics.adjustments[1].id = "adjustment-a".into();
        assert_eq!(
            economics.validate().unwrap_err(),
            OrderPayloadError::InvalidEconomicOrdering {
                field: "adjustments"
            }
        );
    }

    #[test]
    fn order_economics_validation_rejects_negative_derived_total() {
        let mut economics = sample_order_economics();
        economics.adjustments[1].amount = usd("20");
        economics.adjustment_total = usd("22");
        economics.total = usd("0");
        assert_eq!(
            economics.validate().unwrap_err(),
            OrderPayloadError::InvalidEconomicTotal { field: "total" }
        );
    }

    #[test]
    fn order_decision_validation_enforces_commitment_invariants() {
        assert_eq!(sample_order_decision().validate(), Ok(()));

        let declined = OrderDecision {
            decision: OrderDecisionOutcome::Declined {
                reason: "out_of_stock".into(),
            },
            ..sample_order_decision()
        };
        assert_eq!(declined.validate(), Ok(()));

        let accepted_without_commitments = OrderDecision {
            decision: OrderDecisionOutcome::Accepted {
                inventory_commitments: Vec::new(),
            },
            ..sample_order_decision()
        };
        assert_eq!(
            accepted_without_commitments.validate().unwrap_err(),
            OrderPayloadError::MissingInventoryCommitments
        );

        let accepted_with_zero_count = OrderDecision {
            decision: OrderDecisionOutcome::Accepted {
                inventory_commitments: vec![OrderInventoryCommitment {
                    bin_id: bin_id("bin-1"),
                    bin_count: 0,
                }],
            },
            ..sample_order_decision()
        };
        assert_eq!(
            accepted_with_zero_count.validate().unwrap_err(),
            OrderPayloadError::InvalidInventoryCommitmentCount { index: 0 }
        );

        let declined_without_reason = OrderDecision {
            decision: OrderDecisionOutcome::Declined { reason: " ".into() },
            ..sample_order_decision()
        };
        assert_eq!(
            declined_without_reason.validate().unwrap_err(),
            OrderPayloadError::EmptyField("reason")
        );
    }

    #[test]
    fn order_cancellation_validation_requires_buyer_bindings_and_reason() {
        assert_eq!(sample_order_cancellation().validate(), Ok(()));

        let missing_reason = OrderCancellation {
            reason: " ".into(),
            ..sample_order_cancellation()
        };
        assert_eq!(
            missing_reason.validate().unwrap_err(),
            OrderPayloadError::EmptyField("reason")
        );
    }

    #[test]
    fn order_envelope_serializes_canonical_type_name() {
        let envelope = OrderEnvelope::new(
            OrderEventType::OrderRequested,
            sample_listing_addr(),
            "order-1",
            sample_order_request(),
        );
        assert_eq!(envelope.validate(), Ok(()));

        let json = serde_json::to_value(&envelope).unwrap();
        assert_eq!(json["type"], serde_json::json!("TradeOrderRequested"));
        assert_eq!(json["order_id"], serde_json::json!("order-1"));
        assert_eq!(
            json["listing_addr"],
            serde_json::json!(sample_listing_addr().as_str())
        );
        assert_eq!(json["payload"]["items"][0]["bin_id"], "bin-1");
    }

    #[test]
    fn order_envelope_validation_and_display_cover_error_paths() {
        let invalid_version = OrderEnvelope {
            version: RADROOTS_ORDER_ENVELOPE_VERSION + 1,
            domain: CommercialDomain::Listing,
            message_type: OrderEventType::OrderRequested,
            order_id: "order-1".into(),
            listing_addr: sample_listing_addr().into_string(),
            payload: sample_order_request(),
        };
        let invalid_version_err = invalid_version.validate().unwrap_err();
        assert_eq!(
            invalid_version_err,
            OrderEnvelopeError::InvalidVersion {
                expected: RADROOTS_ORDER_ENVELOPE_VERSION,
                got: RADROOTS_ORDER_ENVELOPE_VERSION + 1,
            }
        );
        assert_eq!(
            invalid_version_err.to_string(),
            "invalid order envelope version: expected 1, got 2"
        );

        let missing_order = OrderEnvelope::new(
            OrderEventType::OrderRequested,
            sample_listing_addr(),
            " ",
            sample_order_request(),
        );
        let missing_order_err = missing_order.validate().unwrap_err();
        assert_eq!(missing_order_err, OrderEnvelopeError::MissingOrderId);
        assert_eq!(
            missing_order_err.to_string(),
            "missing order_id for order message"
        );

        let missing_listing = OrderEnvelope::new(
            OrderEventType::OrderRequested,
            " ",
            "order-1",
            sample_order_request(),
        );
        let missing_listing_err = missing_listing.validate().unwrap_err();
        assert_eq!(missing_listing_err, OrderEnvelopeError::MissingListingAddr);
        assert_eq!(missing_listing_err.to_string(), "missing listing_addr");
    }

    #[test]
    fn listing_parse_error_display_variants() {
        assert_eq!(
            OperationalListingParseError::InvalidKind(KIND_PROFILE).to_string(),
            "invalid operational listing kind: 0"
        );
        assert_eq!(
            OperationalListingParseError::MissingTag("price".into()).to_string(),
            "missing required tag: price"
        );
        assert_eq!(
            OperationalListingParseError::InvalidTag("farm".into()).to_string(),
            "invalid tag: farm"
        );
        assert_eq!(
            OperationalListingParseError::InvalidNumber("inventory".into()).to_string(),
            "invalid number: inventory"
        );
        assert_eq!(
            OperationalListingParseError::InvalidUnit.to_string(),
            "invalid unit"
        );
        assert_eq!(
            OperationalListingParseError::InvalidCurrency.to_string(),
            "invalid currency"
        );
        assert_eq!(
            OperationalListingParseError::InvalidJson("bins".into()).to_string(),
            "invalid json: bins"
        );
        assert_eq!(
            OperationalListingParseError::InvalidDiscount("offer".into()).to_string(),
            "invalid discount data for offer"
        );
    }

    #[test]
    fn listing_validation_error_display_variants() {
        assert_eq!(
            (OperationalListingValidationError::InvalidKind { kind: KIND_PROFILE }).to_string(),
            "invalid listing kind: 0"
        );
        assert_eq!(
            OperationalListingValidationError::InvalidProfile.to_string(),
            "classified listing is not an Operational Listing profile"
        );
        assert_eq!(
            OperationalListingValidationError::MissingListingId.to_string(),
            "missing listing id"
        );
        assert_eq!(
            OperationalListingValidationError::ListingEventNotFound {
                listing_addr: "listing-1".into(),
            }
            .to_string(),
            "listing event not found: listing-1"
        );
        assert_eq!(
            OperationalListingValidationError::ListingEventFetchFailed {
                listing_addr: "listing-2".into(),
            }
            .to_string(),
            "listing event fetch failed: listing-2"
        );
        assert_eq!(
            OperationalListingValidationError::ParseError {
                error: OperationalListingParseError::InvalidJson("payload".into()),
            }
            .to_string(),
            "invalid listing data: invalid json: payload"
        );
        assert_eq!(
            OperationalListingValidationError::InvalidSeller.to_string(),
            "listing author does not match farm pubkey"
        );
        assert_eq!(
            OperationalListingValidationError::MissingFarmProfile.to_string(),
            "missing farm profile"
        );
        assert_eq!(
            OperationalListingValidationError::MissingFarmRecord.to_string(),
            "missing farm record"
        );
        assert_eq!(
            OperationalListingValidationError::MissingTitle.to_string(),
            "missing listing title"
        );
        assert_eq!(
            OperationalListingValidationError::MissingDescription.to_string(),
            "missing listing description"
        );
        assert_eq!(
            OperationalListingValidationError::MissingProductType.to_string(),
            "missing listing product type"
        );
        assert_eq!(
            OperationalListingValidationError::MissingBins.to_string(),
            "missing listing bins"
        );
        assert_eq!(
            OperationalListingValidationError::MissingPrimaryBin.to_string(),
            "missing primary listing bin"
        );
        assert_eq!(
            OperationalListingValidationError::InvalidBin.to_string(),
            "invalid listing bin"
        );
        assert_eq!(
            OperationalListingValidationError::MissingPrice.to_string(),
            "missing listing price"
        );
        assert_eq!(
            OperationalListingValidationError::InvalidPrice.to_string(),
            "invalid listing price"
        );
        assert_eq!(
            OperationalListingValidationError::MissingInventory.to_string(),
            "missing listing inventory"
        );
        assert_eq!(
            OperationalListingValidationError::InvalidInventory.to_string(),
            "invalid listing inventory"
        );
        assert_eq!(
            OperationalListingValidationError::MissingAvailability.to_string(),
            "missing listing availability"
        );
        assert_eq!(
            OperationalListingValidationError::MissingLocation.to_string(),
            "missing listing location"
        );
        assert_eq!(
            OperationalListingValidationError::MissingDeliveryMethod.to_string(),
            "missing listing delivery method"
        );
    }

    #[test]
    fn order_payload_error_display_variants_cover_all_messages() {
        let cases = [
            (
                OrderPayloadError::EmptyField("field"),
                "field cannot be empty",
            ),
            (
                OrderPayloadError::MissingItems,
                "items must contain at least one item",
            ),
            (
                OrderPayloadError::InvalidItemBinCount { index: 2 },
                "items[2].bin_count must be greater than zero",
            ),
            (
                OrderPayloadError::MissingEconomicItems,
                "economics.items must contain at least one item",
            ),
            (
                OrderPayloadError::InvalidEconomicItemBinCount { index: 3 },
                "economics.items[3].bin_count must be greater than zero",
            ),
            (
                OrderPayloadError::InvalidEconomicItemQuantity { index: 4 },
                "economics.items[4].quantity_amount must be greater than zero",
            ),
            (
                OrderPayloadError::InvalidEconomicItemPrice { index: 5 },
                "economics.items[5].unit_price_amount must not be negative",
            ),
            (
                OrderPayloadError::InvalidEconomicItemSubtotal { index: 6 },
                "economics.items[6].line_subtotal is invalid",
            ),
            (
                OrderPayloadError::InvalidEconomicLineAmount {
                    field: "adjustments",
                    index: 7,
                },
                "economics.adjustments[7].amount must be greater than zero",
            ),
            (
                OrderPayloadError::InvalidEconomicLineKind {
                    field: "discounts",
                    index: 8,
                },
                "economics.discounts[8].kind is invalid",
            ),
            (
                OrderPayloadError::InvalidEconomicLineEffect {
                    field: "discounts",
                    index: 9,
                },
                "economics.discounts[9].effect is invalid",
            ),
            (
                OrderPayloadError::InvalidEconomicCurrency { field: "total" },
                "economics.total currency is invalid",
            ),
            (
                OrderPayloadError::InvalidEconomicOrdering { field: "items" },
                "economics.items is not in canonical order",
            ),
            (
                OrderPayloadError::InvalidEconomicTotal { field: "subtotal" },
                "economics.subtotal total is invalid",
            ),
            (
                OrderPayloadError::InvalidOrderEconomicsBinding { field: "items" },
                "order items does not match economics",
            ),
            (
                OrderPayloadError::InvalidQuoteVersion,
                "economics.quote_version must be greater than zero",
            ),
            (
                OrderPayloadError::MissingInventoryCommitments,
                "accepted decisions must contain at least one inventory commitment",
            ),
            (
                OrderPayloadError::InvalidInventoryCommitmentCount { index: 1 },
                "inventory_commitments[1].bin_count must be greater than zero",
            ),
        ];

        for (error, expected) in cases {
            assert_eq!(error.to_string(), expected);
        }
    }
}
