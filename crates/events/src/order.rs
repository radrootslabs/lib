#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::{
    string::{String, ToString},
    vec::Vec,
};

use crate::ids::{
    RadrootsEconomicsDigest, RadrootsInventoryBinId, RadrootsListingAddress, RadrootsOrderId,
    RadrootsOrderQuoteId, RadrootsOrderRevisionId,
};
use crate::kinds::*;
pub use crate::order_economics::*;
#[cfg(test)]
use crate::trade_validation::RadrootsTradeValidationListingError;
use radroots_core::{RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney};

pub const RADROOTS_COMMERCIAL_LISTING_DOMAIN: &str = "trade:listing";
pub const RADROOTS_ORDER_ENVELOPE_VERSION: u16 = 1;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsListingParseError {
    InvalidKind(u32),
    MissingTag(String),
    InvalidTag(String),
    InvalidNumber(String),
    InvalidUnit,
    InvalidCurrency,
    InvalidJson(String),
    InvalidDiscount(String),
}

impl core::fmt::Display for RadrootsListingParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidKind(kind) => write!(f, "invalid listing kind: {kind}"),
            Self::MissingTag(tag) => write!(f, "missing required tag: {tag}"),
            Self::InvalidTag(tag) => write!(f, "invalid tag: {tag}"),
            Self::InvalidNumber(field) => write!(f, "invalid number: {field}"),
            Self::InvalidUnit => write!(f, "invalid unit"),
            Self::InvalidCurrency => write!(f, "invalid currency"),
            Self::InvalidJson(field) => write!(f, "invalid json: {field}"),
            Self::InvalidDiscount(kind) => write!(f, "invalid discount data for {kind}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsListingParseError {}

impl RadrootsOrderEconomics {
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

    pub fn derived_totals(&self) -> Result<RadrootsOrderEconomicTotals, RadrootsOrderPayloadError> {
        if self.items.is_empty() {
            return Err(RadrootsOrderPayloadError::MissingEconomicItems);
        }

        let mut subtotal = RadrootsCoreMoney::zero(self.currency);
        for (index, item) in self.items.iter().enumerate() {
            let line_subtotal = validate_economic_item(item, self.currency, index)?;
            subtotal = checked_money_add(&subtotal, &line_subtotal, "subtotal")?;
        }

        let mut discount_total = RadrootsCoreMoney::zero(self.currency);
        for (index, line) in self.discounts.iter().enumerate() {
            validate_economic_line(line, self.currency, "discounts", index)?;
            if line.kind != RadrootsOrderEconomicLineKind::ListingDiscount {
                return Err(RadrootsOrderPayloadError::InvalidEconomicLineKind {
                    field: "discounts",
                    index,
                });
            }
            if line.effect != RadrootsOrderEconomicEffect::Decrease {
                return Err(RadrootsOrderPayloadError::InvalidEconomicLineEffect {
                    field: "discounts",
                    index,
                });
            }
            discount_total = checked_money_add(&discount_total, &line.amount, "discount_total")?;
        }

        let mut adjustment_total = RadrootsCoreMoney::zero(self.currency);
        let mut total = checked_money_sub_non_negative(&subtotal, &discount_total, "total")?;
        for (index, line) in self.adjustments.iter().enumerate() {
            validate_economic_line(line, self.currency, "adjustments", index)?;
            if line.kind == RadrootsOrderEconomicLineKind::ListingDiscount {
                return Err(RadrootsOrderPayloadError::InvalidEconomicLineKind {
                    field: "adjustments",
                    index,
                });
            }
            adjustment_total =
                checked_money_add(&adjustment_total, &line.amount, "adjustment_total")?;
            total = match line.effect {
                RadrootsOrderEconomicEffect::Increase => {
                    checked_money_add(&total, &line.amount, "total")?
                }
                RadrootsOrderEconomicEffect::Decrease => {
                    checked_money_sub_non_negative(&total, &line.amount, "total")?
                }
            };
        }

        Ok(RadrootsOrderEconomicTotals {
            subtotal,
            discount_total,
            adjustment_total,
            total,
        })
    }

    pub fn validate(&self) -> Result<(), RadrootsOrderPayloadError> {
        validate_required_field(&self.quote_id, "quote_id")?;
        if self.quote_version == 0 {
            return Err(RadrootsOrderPayloadError::InvalidQuoteVersion);
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

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOrderRequest {
    pub order_id: RadrootsOrderId,
    pub listing_addr: RadrootsListingAddress,
    pub buyer_pubkey: String,
    pub seller_pubkey: String,
    pub items: Vec<RadrootsOrderItem>,
    pub economics: RadrootsOrderEconomics,
}

impl RadrootsOrderRequest {
    pub fn validate(&self) -> Result<(), RadrootsOrderPayloadError> {
        validate_required_field(&self.order_id, "order_id")?;
        validate_required_field(&self.listing_addr, "listing_addr")?;
        validate_required_field(&self.buyer_pubkey, "buyer_pubkey")?;
        validate_required_field(&self.seller_pubkey, "seller_pubkey")?;
        validate_order_items(&self.items)?;
        self.economics.validate()?;
        validate_order_economics_binding(&self.items, &self.economics)
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOrderRevisionProposal {
    pub revision_id: RadrootsOrderRevisionId,
    pub order_id: RadrootsOrderId,
    pub listing_addr: RadrootsListingAddress,
    pub buyer_pubkey: String,
    pub seller_pubkey: String,
    pub root_event_id: String,
    pub prev_event_id: String,
    pub items: Vec<RadrootsOrderItem>,
    pub economics: RadrootsOrderEconomics,
    pub reason: String,
}

impl RadrootsOrderRevisionProposal {
    pub fn validate(&self) -> Result<(), RadrootsOrderPayloadError> {
        validate_required_field(&self.revision_id, "revision_id")?;
        validate_required_field(&self.order_id, "order_id")?;
        validate_required_field(&self.listing_addr, "listing_addr")?;
        validate_required_field(&self.buyer_pubkey, "buyer_pubkey")?;
        validate_required_field(&self.seller_pubkey, "seller_pubkey")?;
        validate_required_field(&self.root_event_id, "root_event_id")?;
        validate_required_field(&self.prev_event_id, "prev_event_id")?;
        validate_required_field(&self.reason, "reason")?;
        validate_order_items(&self.items)?;
        self.economics.validate()?;
        validate_order_economics_binding(&self.items, &self.economics)
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case", tag = "decision"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsOrderRevisionOutcome {
    Accepted,
    Declined { reason: String },
}

impl RadrootsOrderRevisionOutcome {
    pub fn validate(&self) -> Result<(), RadrootsOrderPayloadError> {
        match self {
            Self::Accepted => Ok(()),
            Self::Declined { reason } => validate_required_field(reason, "reason"),
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOrderRevisionDecision {
    pub revision_id: RadrootsOrderRevisionId,
    pub order_id: RadrootsOrderId,
    pub listing_addr: RadrootsListingAddress,
    pub buyer_pubkey: String,
    pub seller_pubkey: String,
    pub root_event_id: String,
    pub prev_event_id: String,
    pub decision: RadrootsOrderRevisionOutcome,
}

impl RadrootsOrderRevisionDecision {
    pub fn validate(&self) -> Result<(), RadrootsOrderPayloadError> {
        validate_required_field(&self.revision_id, "revision_id")?;
        validate_required_field(&self.order_id, "order_id")?;
        validate_required_field(&self.listing_addr, "listing_addr")?;
        validate_required_field(&self.buyer_pubkey, "buyer_pubkey")?;
        validate_required_field(&self.seller_pubkey, "seller_pubkey")?;
        validate_required_field(&self.root_event_id, "root_event_id")?;
        validate_required_field(&self.prev_event_id, "prev_event_id")?;
        self.decision.validate()
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOrderInventoryCommitment {
    pub bin_id: RadrootsInventoryBinId,
    pub bin_count: u32,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case", tag = "decision"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsOrderDecisionOutcome {
    Accepted {
        inventory_commitments: Vec<RadrootsOrderInventoryCommitment>,
    },
    Declined {
        reason: String,
    },
}

impl RadrootsOrderDecisionOutcome {
    pub fn validate(&self) -> Result<(), RadrootsOrderPayloadError> {
        match self {
            Self::Accepted {
                inventory_commitments,
            } => validate_inventory_commitments(inventory_commitments),
            Self::Declined { reason } => validate_required_field(reason, "reason"),
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOrderDecision {
    pub order_id: RadrootsOrderId,
    pub listing_addr: RadrootsListingAddress,
    pub buyer_pubkey: String,
    pub seller_pubkey: String,
    pub decision: RadrootsOrderDecisionOutcome,
}

impl RadrootsOrderDecision {
    pub fn validate(&self) -> Result<(), RadrootsOrderPayloadError> {
        validate_required_field(&self.order_id, "order_id")?;
        validate_required_field(&self.listing_addr, "listing_addr")?;
        validate_required_field(&self.buyer_pubkey, "buyer_pubkey")?;
        validate_required_field(&self.seller_pubkey, "seller_pubkey")?;
        self.decision.validate()
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsOrderFulfillmentState {
    AcceptedNotFulfilled,
    Preparing,
    ReadyForPickup,
    OutForDelivery,
    Delivered,
    SellerCancelled,
}

impl RadrootsOrderFulfillmentState {
    #[inline]
    pub const fn is_publishable_update(self) -> bool {
        !matches!(self, Self::AcceptedNotFulfilled)
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOrderFulfillmentUpdate {
    pub order_id: RadrootsOrderId,
    pub listing_addr: RadrootsListingAddress,
    pub buyer_pubkey: String,
    pub seller_pubkey: String,
    pub status: RadrootsOrderFulfillmentState,
}

impl RadrootsOrderFulfillmentUpdate {
    pub fn validate(&self) -> Result<(), RadrootsOrderPayloadError> {
        validate_required_field(&self.order_id, "order_id")?;
        validate_required_field(&self.listing_addr, "listing_addr")?;
        validate_required_field(&self.buyer_pubkey, "buyer_pubkey")?;
        validate_required_field(&self.seller_pubkey, "seller_pubkey")?;
        if self.status.is_publishable_update() {
            Ok(())
        } else {
            Err(RadrootsOrderPayloadError::InvalidFulfillmentStatus)
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOrderCancellation {
    pub order_id: RadrootsOrderId,
    pub listing_addr: RadrootsListingAddress,
    pub buyer_pubkey: String,
    pub seller_pubkey: String,
    pub reason: String,
}

impl RadrootsOrderCancellation {
    pub fn validate(&self) -> Result<(), RadrootsOrderPayloadError> {
        validate_required_field(&self.order_id, "order_id")?;
        validate_required_field(&self.listing_addr, "listing_addr")?;
        validate_required_field(&self.buyer_pubkey, "buyer_pubkey")?;
        validate_required_field(&self.seller_pubkey, "seller_pubkey")?;
        validate_required_field(&self.reason, "reason")
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOrderReceipt {
    pub order_id: RadrootsOrderId,
    pub listing_addr: RadrootsListingAddress,
    pub buyer_pubkey: String,
    pub seller_pubkey: String,
    pub received: bool,
    pub issue: Option<String>,
    pub received_at: u64,
}

impl RadrootsOrderReceipt {
    pub fn validate(&self) -> Result<(), RadrootsOrderPayloadError> {
        validate_required_field(&self.order_id, "order_id")?;
        validate_required_field(&self.listing_addr, "listing_addr")?;
        validate_required_field(&self.buyer_pubkey, "buyer_pubkey")?;
        validate_required_field(&self.seller_pubkey, "seller_pubkey")?;
        if self.received {
            if self.issue.is_some() {
                return Err(RadrootsOrderPayloadError::UnexpectedReceiptIssue);
            }
        } else {
            match self.issue.as_deref() {
                Some(issue) => validate_required_field(issue, "issue")?,
                None => return Err(RadrootsOrderPayloadError::MissingReceiptIssue),
            }
        }
        Ok(())
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsOrderPaymentMethod {
    Cash,
    ManualTransfer,
    Other,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOrderPaymentRecord {
    pub order_id: RadrootsOrderId,
    pub listing_addr: RadrootsListingAddress,
    pub buyer_pubkey: String,
    pub seller_pubkey: String,
    pub root_event_id: String,
    pub previous_event_id: String,
    pub agreement_event_id: String,
    pub quote_id: RadrootsOrderQuoteId,
    pub quote_version: u32,
    pub economics_digest: RadrootsEconomicsDigest,
    pub amount: RadrootsCoreDecimal,
    pub currency: RadrootsCoreCurrency,
    pub method: RadrootsOrderPaymentMethod,
    pub reference: Option<String>,
    pub paid_at: Option<u64>,
}

impl RadrootsOrderPaymentRecord {
    pub fn validate(&self) -> Result<(), RadrootsOrderPayloadError> {
        validate_required_field(&self.order_id, "order_id")?;
        validate_required_field(&self.listing_addr, "listing_addr")?;
        validate_required_field(&self.buyer_pubkey, "buyer_pubkey")?;
        validate_required_field(&self.seller_pubkey, "seller_pubkey")?;
        validate_required_field(&self.root_event_id, "root_event_id")?;
        validate_required_field(&self.previous_event_id, "previous_event_id")?;
        validate_required_field(&self.agreement_event_id, "agreement_event_id")?;
        validate_required_field(&self.quote_id, "quote_id")?;
        validate_required_field(&self.economics_digest, "economics_digest")?;
        if self.quote_version == 0 {
            return Err(RadrootsOrderPayloadError::InvalidQuoteVersion);
        }
        if self.amount.is_zero() || self.amount.is_sign_negative() {
            return Err(RadrootsOrderPayloadError::InvalidPaymentAmount);
        }
        if let Some(reference) = self.reference.as_deref() {
            validate_required_field(reference, "reference")?;
        }
        Ok(())
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsOrderSettlementOutcome {
    Accepted,
    Rejected,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOrderSettlementDecision {
    pub order_id: RadrootsOrderId,
    pub listing_addr: RadrootsListingAddress,
    pub seller_pubkey: String,
    pub buyer_pubkey: String,
    pub root_event_id: String,
    pub previous_event_id: String,
    pub agreement_event_id: String,
    pub payment_event_id: String,
    pub quote_id: RadrootsOrderQuoteId,
    pub quote_version: u32,
    pub economics_digest: RadrootsEconomicsDigest,
    pub amount: RadrootsCoreDecimal,
    pub currency: RadrootsCoreCurrency,
    pub decision: RadrootsOrderSettlementOutcome,
    pub reason: Option<String>,
}

impl RadrootsOrderSettlementDecision {
    pub fn validate(&self) -> Result<(), RadrootsOrderPayloadError> {
        validate_required_field(&self.order_id, "order_id")?;
        validate_required_field(&self.listing_addr, "listing_addr")?;
        validate_required_field(&self.seller_pubkey, "seller_pubkey")?;
        validate_required_field(&self.buyer_pubkey, "buyer_pubkey")?;
        validate_required_field(&self.root_event_id, "root_event_id")?;
        validate_required_field(&self.previous_event_id, "previous_event_id")?;
        validate_required_field(&self.agreement_event_id, "agreement_event_id")?;
        validate_required_field(&self.payment_event_id, "payment_event_id")?;
        validate_required_field(&self.quote_id, "quote_id")?;
        validate_required_field(&self.economics_digest, "economics_digest")?;
        if self.quote_version == 0 {
            return Err(RadrootsOrderPayloadError::InvalidQuoteVersion);
        }
        if self.amount.is_zero() || self.amount.is_sign_negative() {
            return Err(RadrootsOrderPayloadError::InvalidPaymentAmount);
        }
        match self.decision {
            RadrootsOrderSettlementOutcome::Accepted => {
                if self.reason.is_some() {
                    return Err(RadrootsOrderPayloadError::UnexpectedSettlementReason);
                }
            }
            RadrootsOrderSettlementOutcome::Rejected => match self.reason.as_deref() {
                Some(reason) => validate_required_field(reason, "reason")?,
                None => return Err(RadrootsOrderPayloadError::MissingSettlementReason),
            },
        }
        Ok(())
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsCommercialDomain {
    #[cfg_attr(feature = "serde", serde(rename = "trade:listing"))]
    Listing,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsOrderEventType {
    #[cfg_attr(feature = "serde", serde(rename = "TradeOrderRequested"))]
    OrderRequested,
    #[cfg_attr(feature = "serde", serde(rename = "TradeOrderDecision"))]
    OrderDecision,
    #[cfg_attr(feature = "serde", serde(rename = "TradeOrderRevisionProposed"))]
    OrderRevisionProposed,
    #[cfg_attr(feature = "serde", serde(rename = "TradeOrderRevisionDecision"))]
    OrderRevisionDecision,
    #[cfg_attr(feature = "serde", serde(rename = "TradeOrderCancelled"))]
    OrderCancelled,
    #[cfg_attr(feature = "serde", serde(rename = "TradeFulfillmentUpdated"))]
    FulfillmentUpdated,
    #[cfg_attr(feature = "serde", serde(rename = "TradeBuyerReceipt"))]
    BuyerReceipt,
    #[cfg_attr(feature = "serde", serde(rename = "TradePaymentRecorded"))]
    PaymentRecorded,
    #[cfg_attr(feature = "serde", serde(rename = "TradeSettlementDecision"))]
    SettlementDecision,
}

impl RadrootsOrderEventType {
    #[inline]
    pub const fn from_kind(kind: u32) -> Option<Self> {
        match kind {
            KIND_ORDER_REQUEST => Some(Self::OrderRequested),
            KIND_ORDER_DECISION => Some(Self::OrderDecision),
            KIND_ORDER_REVISION_PROPOSAL => Some(Self::OrderRevisionProposed),
            KIND_ORDER_REVISION_DECISION => Some(Self::OrderRevisionDecision),
            KIND_ORDER_CANCELLATION => Some(Self::OrderCancelled),
            KIND_ORDER_FULFILLMENT_UPDATE => Some(Self::FulfillmentUpdated),
            KIND_ORDER_RECEIPT => Some(Self::BuyerReceipt),
            KIND_ORDER_PAYMENT_RECORD => Some(Self::PaymentRecorded),
            KIND_ORDER_SETTLEMENT_DECISION => Some(Self::SettlementDecision),
            _ => None,
        }
    }

    #[inline]
    pub const fn kind(self) -> u32 {
        match self {
            Self::OrderRequested => KIND_ORDER_REQUEST,
            Self::OrderDecision => KIND_ORDER_DECISION,
            Self::OrderRevisionProposed => KIND_ORDER_REVISION_PROPOSAL,
            Self::OrderRevisionDecision => KIND_ORDER_REVISION_DECISION,
            Self::OrderCancelled => KIND_ORDER_CANCELLATION,
            Self::FulfillmentUpdated => KIND_ORDER_FULFILLMENT_UPDATE,
            Self::BuyerReceipt => KIND_ORDER_RECEIPT,
            Self::PaymentRecorded => KIND_ORDER_PAYMENT_RECORD,
            Self::SettlementDecision => KIND_ORDER_SETTLEMENT_DECISION,
        }
    }

    #[inline]
    pub const fn name(self) -> &'static str {
        match self {
            Self::OrderRequested => "TradeOrderRequested",
            Self::OrderDecision => "TradeOrderDecision",
            Self::OrderRevisionProposed => "TradeOrderRevisionProposed",
            Self::OrderRevisionDecision => "TradeOrderRevisionDecision",
            Self::OrderCancelled => "TradeOrderCancelled",
            Self::FulfillmentUpdated => "TradeFulfillmentUpdated",
            Self::BuyerReceipt => "TradeBuyerReceipt",
            Self::PaymentRecorded => "TradePaymentRecorded",
            Self::SettlementDecision => "TradeSettlementDecision",
        }
    }

    #[inline]
    pub const fn requires_listing_snapshot(self) -> bool {
        matches!(self, Self::OrderRequested)
    }

    #[inline]
    pub const fn requires_order_chain(self) -> bool {
        matches!(
            self,
            Self::OrderDecision
                | Self::OrderRevisionProposed
                | Self::OrderRevisionDecision
                | Self::OrderCancelled
                | Self::FulfillmentUpdated
                | Self::BuyerReceipt
                | Self::PaymentRecorded
                | Self::SettlementDecision
        )
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOrderEnvelope<T> {
    pub version: u16,
    pub domain: RadrootsCommercialDomain,
    #[cfg_attr(feature = "serde", serde(rename = "type"))]
    pub message_type: RadrootsOrderEventType,
    pub order_id: String,
    pub listing_addr: String,
    pub payload: T,
}

impl<T> RadrootsOrderEnvelope<T> {
    #[inline]
    pub fn new(
        message_type: RadrootsOrderEventType,
        listing_addr: impl Into<String>,
        order_id: impl Into<String>,
        payload: T,
    ) -> Self {
        Self {
            version: RADROOTS_ORDER_ENVELOPE_VERSION,
            domain: RadrootsCommercialDomain::Listing,
            message_type,
            order_id: order_id.into(),
            listing_addr: listing_addr.into(),
            payload,
        }
    }

    pub fn validate(&self) -> Result<(), RadrootsOrderEnvelopeError> {
        if self.version != RADROOTS_ORDER_ENVELOPE_VERSION {
            return Err(RadrootsOrderEnvelopeError::InvalidVersion {
                expected: RADROOTS_ORDER_ENVELOPE_VERSION,
                got: self.version,
            });
        }
        if self.order_id.trim().is_empty() {
            return Err(RadrootsOrderEnvelopeError::MissingOrderId);
        }
        if self.listing_addr.trim().is_empty() {
            return Err(RadrootsOrderEnvelopeError::MissingListingAddr);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadrootsOrderEnvelopeError {
    InvalidVersion { expected: u16, got: u16 },
    MissingOrderId,
    MissingListingAddr,
}

impl core::fmt::Display for RadrootsOrderEnvelopeError {
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
impl std::error::Error for RadrootsOrderEnvelopeError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadrootsOrderPayloadError {
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
    InvalidFulfillmentStatus,
    MissingReceiptIssue,
    UnexpectedReceiptIssue,
    InvalidPaymentAmount,
    MissingSettlementReason,
    UnexpectedSettlementReason,
}

impl core::fmt::Display for RadrootsOrderPayloadError {
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
            Self::InvalidFulfillmentStatus => {
                write!(f, "fulfillment status is not publishable")
            }
            Self::MissingReceiptIssue => {
                write!(f, "receipt issue is required when received is false")
            }
            Self::UnexpectedReceiptIssue => {
                write!(f, "receipt issue must be absent when received is true")
            }
            Self::InvalidPaymentAmount => {
                write!(f, "payment amount must be greater than zero")
            }
            Self::MissingSettlementReason => {
                write!(f, "settlement reason is required when decision is rejected")
            }
            Self::UnexpectedSettlementReason => {
                write!(
                    f,
                    "settlement reason must be absent when decision is accepted"
                )
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsOrderPayloadError {}

fn validate_required_field(
    value: &str,
    field: &'static str,
) -> Result<(), RadrootsOrderPayloadError> {
    if value.trim().is_empty() {
        Err(RadrootsOrderPayloadError::EmptyField(field))
    } else {
        Ok(())
    }
}

fn validate_order_items(items: &[RadrootsOrderItem]) -> Result<(), RadrootsOrderPayloadError> {
    if items.is_empty() {
        return Err(RadrootsOrderPayloadError::MissingItems);
    }
    for (index, item) in items.iter().enumerate() {
        validate_required_field(&item.bin_id, "bin_id")?;
        if item.bin_count == 0 {
            return Err(RadrootsOrderPayloadError::InvalidItemBinCount { index });
        }
    }
    Ok(())
}

fn validate_economic_item(
    item: &RadrootsOrderEconomicItem,
    expected_currency: RadrootsCoreCurrency,
    index: usize,
) -> Result<RadrootsCoreMoney, RadrootsOrderPayloadError> {
    validate_required_field(&item.bin_id, "economics.items.bin_id")?;
    if item.bin_count == 0 {
        return Err(RadrootsOrderPayloadError::InvalidEconomicItemBinCount { index });
    }
    if item.quantity_amount.is_zero() || item.quantity_amount.is_sign_negative() {
        return Err(RadrootsOrderPayloadError::InvalidEconomicItemQuantity { index });
    }
    if item.unit_price_amount.is_sign_negative() {
        return Err(RadrootsOrderPayloadError::InvalidEconomicItemPrice { index });
    }
    if item.unit_price_currency != expected_currency {
        return Err(RadrootsOrderPayloadError::InvalidEconomicCurrency {
            field: "items.unit_price_currency",
        });
    }
    validate_total_money(
        &item.line_subtotal,
        expected_currency,
        "items.line_subtotal",
    )?;

    let quantity_total = checked_decimal_mul(
        item.quantity_amount,
        RadrootsCoreDecimal::from(item.bin_count),
    )
    .ok_or(RadrootsOrderPayloadError::InvalidEconomicItemSubtotal { index })?;
    let expected_subtotal = checked_decimal_mul(item.unit_price_amount, quantity_total)
        .ok_or(RadrootsOrderPayloadError::InvalidEconomicItemSubtotal { index })?;
    if item.line_subtotal.amount != expected_subtotal {
        return Err(RadrootsOrderPayloadError::InvalidEconomicItemSubtotal { index });
    }
    Ok(item.line_subtotal.clone())
}

fn validate_order_economics_binding(
    items: &[RadrootsOrderItem],
    economics: &RadrootsOrderEconomics,
) -> Result<(), RadrootsOrderPayloadError> {
    let order_items = normalized_order_item_counts(items).ok_or(
        RadrootsOrderPayloadError::InvalidOrderEconomicsBinding {
            field: "items.bin_count",
        },
    )?;
    if order_items.len() != economics.items.len() {
        return Err(RadrootsOrderPayloadError::InvalidOrderEconomicsBinding { field: "items" });
    }
    for (item, economic_item) in order_items.iter().zip(economics.items.iter()) {
        if item.bin_id != economic_item.bin_id {
            return Err(RadrootsOrderPayloadError::InvalidOrderEconomicsBinding {
                field: "items.bin_id",
            });
        }
        if item.bin_count != u64::from(economic_item.bin_count) {
            return Err(RadrootsOrderPayloadError::InvalidOrderEconomicsBinding {
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

fn normalized_order_item_counts(
    items: &[RadrootsOrderItem],
) -> Option<Vec<NormalizedOrderItemCount>> {
    let mut counts: Vec<NormalizedOrderItemCount> = Vec::new();
    for item in items {
        let bin_id = item.bin_id.trim();
        if bin_id.is_empty() || item.bin_count == 0 {
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
    line: &RadrootsOrderEconomicLine,
    expected_currency: RadrootsCoreCurrency,
    field: &'static str,
    index: usize,
) -> Result<(), RadrootsOrderPayloadError> {
    validate_required_field(&line.id, "economics.line.id")?;
    validate_required_field(&line.reason, "economics.line.reason")?;
    if line.amount.currency != expected_currency {
        return Err(RadrootsOrderPayloadError::InvalidEconomicCurrency { field });
    }
    if line.amount.amount.is_zero() || line.amount.amount.is_sign_negative() {
        return Err(RadrootsOrderPayloadError::InvalidEconomicLineAmount { field, index });
    }
    Ok(())
}

fn validate_economic_item_order(
    items: &[RadrootsOrderEconomicItem],
) -> Result<(), RadrootsOrderPayloadError> {
    for pair in items.windows(2) {
        if pair[0].bin_id >= pair[1].bin_id {
            return Err(RadrootsOrderPayloadError::InvalidEconomicOrdering {
                field: "items.bin_id",
            });
        }
    }
    Ok(())
}

fn validate_economic_line_order(
    lines: &[RadrootsOrderEconomicLine],
    field: &'static str,
) -> Result<(), RadrootsOrderPayloadError> {
    for pair in lines.windows(2) {
        if pair[0].id >= pair[1].id {
            return Err(RadrootsOrderPayloadError::InvalidEconomicOrdering { field });
        }
    }
    Ok(())
}

fn validate_total_money(
    money: &RadrootsCoreMoney,
    expected_currency: RadrootsCoreCurrency,
    field: &'static str,
) -> Result<(), RadrootsOrderPayloadError> {
    if money.currency != expected_currency {
        return Err(RadrootsOrderPayloadError::InvalidEconomicCurrency { field });
    }
    if money.amount.is_sign_negative() {
        return Err(RadrootsOrderPayloadError::InvalidEconomicTotal { field });
    }
    Ok(())
}

fn validate_total_matches(
    actual: &RadrootsCoreMoney,
    expected: &RadrootsCoreMoney,
    field: &'static str,
) -> Result<(), RadrootsOrderPayloadError> {
    if actual.currency != expected.currency {
        return Err(RadrootsOrderPayloadError::InvalidEconomicCurrency { field });
    }
    if actual.amount != expected.amount {
        return Err(RadrootsOrderPayloadError::InvalidEconomicTotal { field });
    }
    Ok(())
}

fn checked_decimal_add(
    left: RadrootsCoreDecimal,
    right: RadrootsCoreDecimal,
) -> Option<RadrootsCoreDecimal> {
    left.0.checked_add(right.0).map(RadrootsCoreDecimal)
}

fn checked_decimal_sub(
    left: RadrootsCoreDecimal,
    right: RadrootsCoreDecimal,
) -> Option<RadrootsCoreDecimal> {
    left.0.checked_sub(right.0).map(RadrootsCoreDecimal)
}

fn checked_decimal_mul(
    left: RadrootsCoreDecimal,
    right: RadrootsCoreDecimal,
) -> Option<RadrootsCoreDecimal> {
    left.0.checked_mul(right.0).map(RadrootsCoreDecimal)
}

fn checked_money_add(
    left: &RadrootsCoreMoney,
    right: &RadrootsCoreMoney,
    field: &'static str,
) -> Result<RadrootsCoreMoney, RadrootsOrderPayloadError> {
    if left.currency != right.currency {
        return Err(RadrootsOrderPayloadError::InvalidEconomicCurrency { field });
    }
    let amount = checked_decimal_add(left.amount, right.amount)
        .ok_or(RadrootsOrderPayloadError::InvalidEconomicTotal { field })?;
    Ok(RadrootsCoreMoney::new(amount, left.currency))
}

fn checked_money_sub_non_negative(
    left: &RadrootsCoreMoney,
    right: &RadrootsCoreMoney,
    field: &'static str,
) -> Result<RadrootsCoreMoney, RadrootsOrderPayloadError> {
    if left.currency != right.currency {
        return Err(RadrootsOrderPayloadError::InvalidEconomicCurrency { field });
    }
    let amount = checked_decimal_sub(left.amount, right.amount)
        .ok_or(RadrootsOrderPayloadError::InvalidEconomicTotal { field })?;
    if amount.is_sign_negative() {
        return Err(RadrootsOrderPayloadError::InvalidEconomicTotal { field });
    }
    Ok(RadrootsCoreMoney::new(amount, left.currency))
}

fn validate_inventory_commitments(
    commitments: &[RadrootsOrderInventoryCommitment],
) -> Result<(), RadrootsOrderPayloadError> {
    if commitments.is_empty() {
        return Err(RadrootsOrderPayloadError::MissingInventoryCommitments);
    }
    for (index, commitment) in commitments.iter().enumerate() {
        validate_required_field(&commitment.bin_id, "bin_id")?;
        if commitment.bin_count == 0 {
            return Err(RadrootsOrderPayloadError::InvalidInventoryCommitmentCount { index });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use radroots_core::{
        RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreUnit,
    };

    fn sample_pubkey() -> String {
        "0".repeat(64)
    }

    fn sample_listing_addr() -> RadrootsListingAddress {
        format!("30402:{}:AAAAAAAAAAAAAAAAAAAAAg", sample_pubkey())
            .parse()
            .unwrap()
    }

    fn order_id(raw: &str) -> RadrootsOrderId {
        raw.parse().unwrap()
    }

    fn revision_id(raw: &str) -> RadrootsOrderRevisionId {
        raw.parse().unwrap()
    }

    fn quote_id(raw: &str) -> RadrootsOrderQuoteId {
        raw.parse().unwrap()
    }

    fn bin_id(raw: &str) -> RadrootsInventoryBinId {
        raw.parse().unwrap()
    }

    fn digest(raw: &str) -> RadrootsEconomicsDigest {
        raw.parse().unwrap()
    }

    fn sample_order_request() -> RadrootsOrderRequest {
        RadrootsOrderRequest {
            order_id: order_id("order-1"),
            listing_addr: sample_listing_addr(),
            buyer_pubkey: "buyer".into(),
            seller_pubkey: "seller".into(),
            items: vec![RadrootsOrderItem {
                bin_id: bin_id("bin-1"),
                bin_count: 2,
            }],
            economics: sample_bound_order_economics(),
        }
    }

    fn decimal(raw: &str) -> RadrootsCoreDecimal {
        raw.parse().unwrap()
    }

    fn usd(raw: &str) -> RadrootsCoreMoney {
        RadrootsCoreMoney::new(decimal(raw), RadrootsCoreCurrency::USD)
    }

    fn sample_order_economics() -> RadrootsOrderEconomics {
        RadrootsOrderEconomics {
            quote_id: quote_id("quote-1"),
            quote_version: 1,
            pricing_basis: RadrootsOrderPricingBasis::ListingEvent,
            currency: RadrootsCoreCurrency::USD,
            items: vec![
                RadrootsOrderEconomicItem {
                    bin_id: bin_id("bin-a"),
                    bin_count: 2,
                    quantity_amount: decimal("1.5"),
                    quantity_unit: RadrootsCoreUnit::Each,
                    unit_price_amount: decimal("4"),
                    unit_price_currency: RadrootsCoreCurrency::USD,
                    line_subtotal: usd("12"),
                },
                RadrootsOrderEconomicItem {
                    bin_id: bin_id("bin-b"),
                    bin_count: 1,
                    quantity_amount: decimal("2"),
                    quantity_unit: RadrootsCoreUnit::Each,
                    unit_price_amount: decimal("3"),
                    unit_price_currency: RadrootsCoreCurrency::USD,
                    line_subtotal: usd("6"),
                },
            ],
            discounts: vec![RadrootsOrderEconomicLine {
                id: "discount-a".into(),
                kind: RadrootsOrderEconomicLineKind::ListingDiscount,
                actor: RadrootsOrderEconomicActor::Seller,
                effect: RadrootsOrderEconomicEffect::Decrease,
                amount: usd("3"),
                reason: "farmstand pickup".into(),
            }],
            adjustments: vec![
                RadrootsOrderEconomicLine {
                    id: "adjustment-a".into(),
                    kind: RadrootsOrderEconomicLineKind::BasketAdjustment,
                    actor: RadrootsOrderEconomicActor::Buyer,
                    effect: RadrootsOrderEconomicEffect::Increase,
                    amount: usd("2"),
                    reason: "special handling".into(),
                },
                RadrootsOrderEconomicLine {
                    id: "adjustment-b".into(),
                    kind: RadrootsOrderEconomicLineKind::BasketAdjustment,
                    actor: RadrootsOrderEconomicActor::Buyer,
                    effect: RadrootsOrderEconomicEffect::Decrease,
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

    fn sample_bound_order_economics() -> RadrootsOrderEconomics {
        RadrootsOrderEconomics {
            quote_id: quote_id("quote-bound-1"),
            quote_version: 1,
            pricing_basis: RadrootsOrderPricingBasis::ListingEvent,
            currency: RadrootsCoreCurrency::USD,
            items: vec![RadrootsOrderEconomicItem {
                bin_id: bin_id("bin-1"),
                bin_count: 2,
                quantity_amount: decimal("1"),
                quantity_unit: RadrootsCoreUnit::Each,
                unit_price_amount: decimal("5"),
                unit_price_currency: RadrootsCoreCurrency::USD,
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

    fn sample_inventory_commitment() -> RadrootsOrderInventoryCommitment {
        RadrootsOrderInventoryCommitment {
            bin_id: bin_id("bin-1"),
            bin_count: 2,
        }
    }

    fn sample_order_decision() -> RadrootsOrderDecision {
        RadrootsOrderDecision {
            order_id: order_id("order-1"),
            listing_addr: sample_listing_addr(),
            buyer_pubkey: "buyer".into(),
            seller_pubkey: "seller".into(),
            decision: RadrootsOrderDecisionOutcome::Accepted {
                inventory_commitments: vec![sample_inventory_commitment()],
            },
        }
    }

    fn sample_order_fulfillment_update() -> RadrootsOrderFulfillmentUpdate {
        RadrootsOrderFulfillmentUpdate {
            order_id: order_id("order-1"),
            listing_addr: sample_listing_addr(),
            buyer_pubkey: "buyer".into(),
            seller_pubkey: "seller".into(),
            status: RadrootsOrderFulfillmentState::ReadyForPickup,
        }
    }

    fn sample_order_cancellation() -> RadrootsOrderCancellation {
        RadrootsOrderCancellation {
            order_id: order_id("order-1"),
            listing_addr: sample_listing_addr(),
            buyer_pubkey: "buyer".into(),
            seller_pubkey: "seller".into(),
            reason: "changed plans".into(),
        }
    }

    fn sample_order_buyer_receipt(received: bool) -> RadrootsOrderReceipt {
        RadrootsOrderReceipt {
            order_id: order_id("order-1"),
            listing_addr: sample_listing_addr(),
            buyer_pubkey: "buyer".into(),
            seller_pubkey: "seller".into(),
            received,
            issue: (!received).then(|| "damaged items".into()),
            received_at: 1_777_665_600,
        }
    }

    fn sample_order_revision_proposal() -> RadrootsOrderRevisionProposal {
        RadrootsOrderRevisionProposal {
            revision_id: revision_id("rev-1"),
            order_id: order_id("order-1"),
            listing_addr: sample_listing_addr(),
            buyer_pubkey: "buyer".into(),
            seller_pubkey: "seller".into(),
            root_event_id: "root-event".into(),
            prev_event_id: "previous-event".into(),
            items: vec![RadrootsOrderItem {
                bin_id: bin_id("bin-1"),
                bin_count: 2,
            }],
            economics: sample_bound_order_economics(),
            reason: "update quantity".into(),
        }
    }

    fn sample_order_revision_decision(
        decision: RadrootsOrderRevisionOutcome,
    ) -> RadrootsOrderRevisionDecision {
        RadrootsOrderRevisionDecision {
            revision_id: revision_id("rev-1"),
            order_id: order_id("order-1"),
            listing_addr: sample_listing_addr(),
            buyer_pubkey: "buyer".into(),
            seller_pubkey: "seller".into(),
            root_event_id: "root-event".into(),
            prev_event_id: "previous-event".into(),
            decision,
        }
    }

    fn sample_payment_recorded() -> RadrootsOrderPaymentRecord {
        RadrootsOrderPaymentRecord {
            order_id: order_id("order-1"),
            listing_addr: sample_listing_addr(),
            buyer_pubkey: "buyer".into(),
            seller_pubkey: "seller".into(),
            root_event_id: "root-event".into(),
            previous_event_id: "previous-event".into(),
            agreement_event_id: "agreement-event".into(),
            quote_id: quote_id("quote-1"),
            quote_version: 1,
            economics_digest: digest("economics-digest"),
            amount: decimal("16"),
            currency: RadrootsCoreCurrency::USD,
            method: RadrootsOrderPaymentMethod::ManualTransfer,
            reference: Some("bank-ref".into()),
            paid_at: Some(1_777_665_600),
        }
    }

    fn sample_settlement_decision(
        decision: RadrootsOrderSettlementOutcome,
        reason: Option<&str>,
    ) -> RadrootsOrderSettlementDecision {
        RadrootsOrderSettlementDecision {
            order_id: order_id("order-1"),
            listing_addr: sample_listing_addr(),
            seller_pubkey: "seller".into(),
            buyer_pubkey: "buyer".into(),
            root_event_id: "root-event".into(),
            previous_event_id: "previous-event".into(),
            agreement_event_id: "agreement-event".into(),
            payment_event_id: "payment-event".into(),
            quote_id: quote_id("quote-1"),
            quote_version: 1,
            economics_digest: digest("economics-digest"),
            amount: decimal("16"),
            currency: RadrootsCoreCurrency::USD,
            decision,
            reason: reason.map(Into::into),
        }
    }

    #[test]
    fn order_message_type_uses_canonical_names_and_kinds() {
        assert_eq!(
            RadrootsOrderEventType::from_kind(KIND_ORDER_REQUEST),
            Some(RadrootsOrderEventType::OrderRequested)
        );
        assert_eq!(
            RadrootsOrderEventType::from_kind(KIND_ORDER_DECISION),
            Some(RadrootsOrderEventType::OrderDecision)
        );
        assert_eq!(
            RadrootsOrderEventType::from_kind(KIND_ORDER_REVISION_PROPOSAL),
            Some(RadrootsOrderEventType::OrderRevisionProposed)
        );
        assert_eq!(
            RadrootsOrderEventType::from_kind(KIND_ORDER_REVISION_DECISION),
            Some(RadrootsOrderEventType::OrderRevisionDecision)
        );
        assert_eq!(
            RadrootsOrderEventType::from_kind(KIND_ORDER_FULFILLMENT_UPDATE),
            Some(RadrootsOrderEventType::FulfillmentUpdated)
        );
        assert_eq!(
            RadrootsOrderEventType::from_kind(KIND_ORDER_CANCELLATION),
            Some(RadrootsOrderEventType::OrderCancelled)
        );
        assert_eq!(
            RadrootsOrderEventType::from_kind(KIND_ORDER_RECEIPT),
            Some(RadrootsOrderEventType::BuyerReceipt)
        );
        assert_eq!(
            RadrootsOrderEventType::from_kind(KIND_ORDER_PAYMENT_RECORD),
            Some(RadrootsOrderEventType::PaymentRecorded)
        );
        assert_eq!(
            RadrootsOrderEventType::from_kind(KIND_ORDER_SETTLEMENT_DECISION),
            Some(RadrootsOrderEventType::SettlementDecision)
        );
        assert_eq!(RadrootsOrderEventType::from_kind(3431), None);
        assert_eq!(
            RadrootsOrderEventType::OrderRequested.kind(),
            KIND_ORDER_REQUEST
        );
        assert_eq!(
            RadrootsOrderEventType::OrderDecision.kind(),
            KIND_ORDER_DECISION
        );
        assert_eq!(
            RadrootsOrderEventType::OrderRevisionProposed.kind(),
            KIND_ORDER_REVISION_PROPOSAL
        );
        assert_eq!(
            RadrootsOrderEventType::OrderRevisionDecision.kind(),
            KIND_ORDER_REVISION_DECISION
        );
        assert_eq!(
            RadrootsOrderEventType::FulfillmentUpdated.kind(),
            KIND_ORDER_FULFILLMENT_UPDATE
        );
        assert_eq!(
            RadrootsOrderEventType::OrderCancelled.kind(),
            KIND_ORDER_CANCELLATION
        );
        assert_eq!(
            RadrootsOrderEventType::BuyerReceipt.kind(),
            KIND_ORDER_RECEIPT
        );
        assert_eq!(
            RadrootsOrderEventType::PaymentRecorded.kind(),
            KIND_ORDER_PAYMENT_RECORD
        );
        assert_eq!(
            RadrootsOrderEventType::SettlementDecision.kind(),
            KIND_ORDER_SETTLEMENT_DECISION
        );
        assert_eq!(
            RadrootsOrderEventType::OrderRequested.name(),
            "TradeOrderRequested"
        );
        assert_eq!(
            RadrootsOrderEventType::OrderDecision.name(),
            "TradeOrderDecision"
        );
        assert_eq!(
            RadrootsOrderEventType::OrderRevisionProposed.name(),
            "TradeOrderRevisionProposed"
        );
        assert_eq!(
            RadrootsOrderEventType::OrderRevisionDecision.name(),
            "TradeOrderRevisionDecision"
        );
        assert_eq!(
            RadrootsOrderEventType::FulfillmentUpdated.name(),
            "TradeFulfillmentUpdated"
        );
        assert_eq!(
            RadrootsOrderEventType::OrderCancelled.name(),
            "TradeOrderCancelled"
        );
        assert_eq!(
            RadrootsOrderEventType::BuyerReceipt.name(),
            "TradeBuyerReceipt"
        );
        assert_eq!(
            RadrootsOrderEventType::PaymentRecorded.name(),
            "TradePaymentRecorded"
        );
        assert_eq!(
            RadrootsOrderEventType::SettlementDecision.name(),
            "TradeSettlementDecision"
        );
        assert!(RadrootsOrderEventType::OrderRequested.requires_listing_snapshot());
        assert!(RadrootsOrderEventType::OrderDecision.requires_order_chain());
        assert!(RadrootsOrderEventType::OrderRevisionProposed.requires_order_chain());
        assert!(RadrootsOrderEventType::OrderRevisionDecision.requires_order_chain());
        assert!(RadrootsOrderEventType::FulfillmentUpdated.requires_order_chain());
        assert!(RadrootsOrderEventType::OrderCancelled.requires_order_chain());
        assert!(RadrootsOrderEventType::BuyerReceipt.requires_order_chain());
        assert!(RadrootsOrderEventType::PaymentRecorded.requires_order_chain());
        assert!(RadrootsOrderEventType::SettlementDecision.requires_order_chain());
        assert!(!RadrootsOrderEventType::OrderRequested.requires_order_chain());
        assert!(!RadrootsOrderEventType::PaymentRecorded.requires_listing_snapshot());

        let request_name = serde_json::to_value(RadrootsOrderEventType::OrderRequested).unwrap();
        let decision_name = serde_json::to_value(RadrootsOrderEventType::OrderDecision).unwrap();
        let revision_proposed_name =
            serde_json::to_value(RadrootsOrderEventType::OrderRevisionProposed).unwrap();
        let revision_decision_name =
            serde_json::to_value(RadrootsOrderEventType::OrderRevisionDecision).unwrap();
        let fulfillment_name =
            serde_json::to_value(RadrootsOrderEventType::FulfillmentUpdated).unwrap();
        let cancellation_name =
            serde_json::to_value(RadrootsOrderEventType::OrderCancelled).unwrap();
        let receipt_name = serde_json::to_value(RadrootsOrderEventType::BuyerReceipt).unwrap();
        let payment_name = serde_json::to_value(RadrootsOrderEventType::PaymentRecorded).unwrap();
        let settlement_name =
            serde_json::to_value(RadrootsOrderEventType::SettlementDecision).unwrap();
        assert_eq!(request_name, serde_json::json!("TradeOrderRequested"));
        assert_eq!(decision_name, serde_json::json!("TradeOrderDecision"));
        assert_eq!(
            revision_proposed_name,
            serde_json::json!("TradeOrderRevisionProposed")
        );
        assert_eq!(
            revision_decision_name,
            serde_json::json!("TradeOrderRevisionDecision")
        );
        assert_eq!(
            fulfillment_name,
            serde_json::json!("TradeFulfillmentUpdated")
        );
        assert_eq!(cancellation_name, serde_json::json!("TradeOrderCancelled"));
        assert_eq!(receipt_name, serde_json::json!("TradeBuyerReceipt"));
        assert_eq!(payment_name, serde_json::json!("TradePaymentRecorded"));
        assert_eq!(
            settlement_name,
            serde_json::json!("TradeSettlementDecision")
        );
    }

    #[test]
    fn order_request_validation_rejects_invalid_fields() {
        assert_eq!(sample_order_request().validate(), Ok(()));

        let mut missing_buyer_pubkey = sample_order_request();
        missing_buyer_pubkey.buyer_pubkey = " ".into();
        assert_eq!(
            missing_buyer_pubkey.validate().unwrap_err(),
            RadrootsOrderPayloadError::EmptyField("buyer_pubkey")
        );

        let mut missing_items = sample_order_request();
        missing_items.items.clear();
        assert_eq!(
            missing_items.validate().unwrap_err(),
            RadrootsOrderPayloadError::MissingItems
        );

        let mut invalid_count = sample_order_request();
        invalid_count.items[0].bin_count = 0;
        assert_eq!(
            invalid_count.validate().unwrap_err(),
            RadrootsOrderPayloadError::InvalidItemBinCount { index: 0 }
        );

        let mut mismatched_economic_item = sample_order_request();
        mismatched_economic_item.economics.items[0].bin_id = bin_id("bin-other");
        assert_eq!(
            mismatched_economic_item.validate().unwrap_err(),
            RadrootsOrderPayloadError::InvalidOrderEconomicsBinding {
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
            RadrootsOrderPayloadError::InvalidOrderEconomicsBinding {
                field: "items.bin_count"
            }
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
        economics.discounts.push(RadrootsOrderEconomicLine {
            id: "discount-b".into(),
            kind: RadrootsOrderEconomicLineKind::ListingDiscount,
            actor: RadrootsOrderEconomicActor::Seller,
            effect: RadrootsOrderEconomicEffect::Decrease,
            amount: usd("1"),
            reason: "market credit".into(),
        });
        economics.discounts.reverse();
        economics.subtotal = usd("19");
        economics.total = usd("17");
        assert_eq!(
            economics.validate().unwrap_err(),
            RadrootsOrderPayloadError::InvalidEconomicOrdering {
                field: "items.bin_id"
            }
        );

        let canonical = economics.canonicalized();
        assert_eq!(canonical.items[0].bin_id, "bin-a");
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
        economics.items[0].unit_price_currency = RadrootsCoreCurrency::EUR;
        assert_eq!(
            economics.validate().unwrap_err(),
            RadrootsOrderPayloadError::InvalidEconomicCurrency {
                field: "items.unit_price_currency"
            }
        );

        let mut economics = sample_order_economics();
        economics.adjustments[0].amount =
            RadrootsCoreMoney::new(decimal("2"), RadrootsCoreCurrency::EUR);
        assert_eq!(
            economics.validate().unwrap_err(),
            RadrootsOrderPayloadError::InvalidEconomicCurrency {
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
            RadrootsOrderPayloadError::InvalidEconomicItemBinCount { index: 0 }
        );

        let mut economics = sample_order_economics();
        economics.items[0].line_subtotal = usd("11.99");
        assert_eq!(
            economics.validate().unwrap_err(),
            RadrootsOrderPayloadError::InvalidEconomicItemSubtotal { index: 0 }
        );

        let mut economics = sample_order_economics();
        economics.items[0].line_subtotal =
            RadrootsCoreMoney::new(decimal("12"), RadrootsCoreCurrency::EUR);
        assert_eq!(
            economics.validate().unwrap_err(),
            RadrootsOrderPayloadError::InvalidEconomicCurrency {
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
            RadrootsOrderPayloadError::MissingEconomicItems
        );

        let mut economics = sample_order_economics();
        economics.quote_version = 0;
        assert_eq!(
            economics.validate().unwrap_err(),
            RadrootsOrderPayloadError::InvalidQuoteVersion
        );

        let mut economics = sample_order_economics();
        economics.items[0].quantity_amount = decimal("0");
        assert_eq!(
            economics.validate().unwrap_err(),
            RadrootsOrderPayloadError::InvalidEconomicItemQuantity { index: 0 }
        );

        let mut economics = sample_order_economics();
        economics.items[0].quantity_amount = decimal("-1");
        assert_eq!(
            economics.validate().unwrap_err(),
            RadrootsOrderPayloadError::InvalidEconomicItemQuantity { index: 0 }
        );

        let mut economics = sample_order_economics();
        economics.items[0].unit_price_amount = decimal("-1");
        assert_eq!(
            economics.validate().unwrap_err(),
            RadrootsOrderPayloadError::InvalidEconomicItemPrice { index: 0 }
        );

        let mut economics = sample_order_economics();
        economics.discounts[0].kind = RadrootsOrderEconomicLineKind::BasketAdjustment;
        assert_eq!(
            economics.validate().unwrap_err(),
            RadrootsOrderPayloadError::InvalidEconomicLineKind {
                field: "discounts",
                index: 0
            }
        );

        let mut economics = sample_order_economics();
        economics.subtotal = RadrootsCoreMoney::new(decimal("18"), RadrootsCoreCurrency::EUR);
        assert_eq!(
            economics.validate().unwrap_err(),
            RadrootsOrderPayloadError::InvalidEconomicCurrency { field: "subtotal" }
        );

        let mut economics = sample_order_economics();
        economics.subtotal = usd("-1");
        assert_eq!(
            economics.validate().unwrap_err(),
            RadrootsOrderPayloadError::InvalidEconomicTotal { field: "subtotal" }
        );

        let mut economics = sample_order_economics();
        economics.discount_total = usd("4");
        assert_eq!(
            economics.validate().unwrap_err(),
            RadrootsOrderPayloadError::InvalidEconomicTotal {
                field: "discount_total"
            }
        );

        let mut economics = sample_order_economics();
        economics.adjustment_total = usd("4");
        assert_eq!(
            economics.validate().unwrap_err(),
            RadrootsOrderPayloadError::InvalidEconomicTotal {
                field: "adjustment_total"
            }
        );

        let economics = sample_bound_order_economics();
        assert_eq!(
            validate_order_economics_binding(&[], &economics).unwrap_err(),
            RadrootsOrderPayloadError::InvalidOrderEconomicsBinding { field: "items" }
        );

        let invalid_order_items = [RadrootsOrderItem {
            bin_id: bin_id("bin-1"),
            bin_count: 0,
        }];
        assert_eq!(
            validate_order_economics_binding(&invalid_order_items, &economics).unwrap_err(),
            RadrootsOrderPayloadError::InvalidOrderEconomicsBinding {
                field: "items.bin_count"
            }
        );

        let duplicate_counts = normalized_order_item_counts(&[
            RadrootsOrderItem {
                bin_id: bin_id("bin-1"),
                bin_count: 1,
            },
            RadrootsOrderItem {
                bin_id: bin_id("bin-1"),
                bin_count: 2,
            },
        ])
        .unwrap();
        assert_eq!(duplicate_counts[0].bin_count, 3);

        assert!(
            normalized_order_item_counts(&[RadrootsOrderItem {
                bin_id: bin_id("bin-1"),
                bin_count: 0,
            }])
            .is_none()
        );
        let sorted_counts = normalized_order_item_counts(&[
            RadrootsOrderItem {
                bin_id: bin_id("bin-b"),
                bin_count: 1,
            },
            RadrootsOrderItem {
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
        economics.discounts[0].effect = RadrootsOrderEconomicEffect::Increase;
        assert_eq!(
            economics.validate().unwrap_err(),
            RadrootsOrderPayloadError::InvalidEconomicLineEffect {
                field: "discounts",
                index: 0
            }
        );

        let mut economics = sample_order_economics();
        economics.adjustments[0].kind = RadrootsOrderEconomicLineKind::ListingDiscount;
        assert_eq!(
            economics.validate().unwrap_err(),
            RadrootsOrderPayloadError::InvalidEconomicLineKind {
                field: "adjustments",
                index: 0
            }
        );

        let mut economics = sample_order_economics();
        economics.adjustments[0].amount = usd("0");
        assert_eq!(
            economics.validate().unwrap_err(),
            RadrootsOrderPayloadError::InvalidEconomicLineAmount {
                field: "adjustments",
                index: 0
            }
        );

        let mut economics = sample_order_economics();
        economics.adjustments[0].amount = usd("-1");
        assert_eq!(
            economics.validate().unwrap_err(),
            RadrootsOrderPayloadError::InvalidEconomicLineAmount {
                field: "adjustments",
                index: 0
            }
        );
    }

    #[test]
    fn order_economics_helpers_cover_currency_error_paths() {
        assert_eq!(
            validate_total_money(&usd("-1"), RadrootsCoreCurrency::USD, "subtotal").unwrap_err(),
            RadrootsOrderPayloadError::InvalidEconomicTotal { field: "subtotal" }
        );
        assert_eq!(
            validate_total_matches(
                &usd("1"),
                &RadrootsCoreMoney::new(decimal("1"), RadrootsCoreCurrency::EUR),
                "total"
            )
            .unwrap_err(),
            RadrootsOrderPayloadError::InvalidEconomicCurrency { field: "total" }
        );
        assert_eq!(
            checked_money_add(
                &usd("1"),
                &RadrootsCoreMoney::new(decimal("1"), RadrootsCoreCurrency::EUR),
                "subtotal"
            )
            .unwrap_err(),
            RadrootsOrderPayloadError::InvalidEconomicCurrency { field: "subtotal" }
        );
        assert_eq!(
            checked_money_sub_non_negative(
                &usd("1"),
                &RadrootsCoreMoney::new(decimal("1"), RadrootsCoreCurrency::EUR),
                "total"
            )
            .unwrap_err(),
            RadrootsOrderPayloadError::InvalidEconomicCurrency { field: "total" }
        );
    }

    #[test]
    fn order_economics_validation_rejects_duplicate_line_ids() {
        let mut economics = sample_order_economics();
        economics.adjustments[1].id = "adjustment-a".into();
        assert_eq!(
            economics.validate().unwrap_err(),
            RadrootsOrderPayloadError::InvalidEconomicOrdering {
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
            RadrootsOrderPayloadError::InvalidEconomicTotal { field: "total" }
        );
    }

    #[test]
    fn order_decision_validation_enforces_commitment_invariants() {
        assert_eq!(sample_order_decision().validate(), Ok(()));

        let declined = RadrootsOrderDecision {
            decision: RadrootsOrderDecisionOutcome::Declined {
                reason: "out_of_stock".into(),
            },
            ..sample_order_decision()
        };
        assert_eq!(declined.validate(), Ok(()));

        let accepted_without_commitments = RadrootsOrderDecision {
            decision: RadrootsOrderDecisionOutcome::Accepted {
                inventory_commitments: Vec::new(),
            },
            ..sample_order_decision()
        };
        assert_eq!(
            accepted_without_commitments.validate().unwrap_err(),
            RadrootsOrderPayloadError::MissingInventoryCommitments
        );

        let accepted_with_zero_count = RadrootsOrderDecision {
            decision: RadrootsOrderDecisionOutcome::Accepted {
                inventory_commitments: vec![RadrootsOrderInventoryCommitment {
                    bin_id: bin_id("bin-1"),
                    bin_count: 0,
                }],
            },
            ..sample_order_decision()
        };
        assert_eq!(
            accepted_with_zero_count.validate().unwrap_err(),
            RadrootsOrderPayloadError::InvalidInventoryCommitmentCount { index: 0 }
        );

        let declined_without_reason = RadrootsOrderDecision {
            decision: RadrootsOrderDecisionOutcome::Declined { reason: " ".into() },
            ..sample_order_decision()
        };
        assert_eq!(
            declined_without_reason.validate().unwrap_err(),
            RadrootsOrderPayloadError::EmptyField("reason")
        );
    }

    #[test]
    fn order_revision_validation_covers_proposed_and_decision_paths() {
        assert_eq!(sample_order_revision_proposal().validate(), Ok(()));

        assert_eq!(
            sample_order_revision_decision(RadrootsOrderRevisionOutcome::Accepted).validate(),
            Ok(())
        );
        assert_eq!(
            sample_order_revision_decision(RadrootsOrderRevisionOutcome::Declined {
                reason: "out of stock".into(),
            })
            .validate(),
            Ok(())
        );

        let declined_without_reason =
            sample_order_revision_decision(RadrootsOrderRevisionOutcome::Declined {
                reason: " ".into(),
            });
        assert_eq!(
            declined_without_reason.validate().unwrap_err(),
            RadrootsOrderPayloadError::EmptyField("reason")
        );
    }

    #[test]
    fn order_fulfillment_update_validation_rejects_derived_state() {
        assert_eq!(sample_order_fulfillment_update().validate(), Ok(()));

        let derived = RadrootsOrderFulfillmentUpdate {
            status: RadrootsOrderFulfillmentState::AcceptedNotFulfilled,
            ..sample_order_fulfillment_update()
        };
        assert_eq!(
            derived.validate().unwrap_err(),
            RadrootsOrderPayloadError::InvalidFulfillmentStatus
        );

        let missing_seller = RadrootsOrderFulfillmentUpdate {
            seller_pubkey: " ".into(),
            ..sample_order_fulfillment_update()
        };
        assert_eq!(
            missing_seller.validate().unwrap_err(),
            RadrootsOrderPayloadError::EmptyField("seller_pubkey")
        );
    }

    #[test]
    fn order_cancellation_validation_requires_buyer_bindings_and_reason() {
        assert_eq!(sample_order_cancellation().validate(), Ok(()));

        let missing_reason = RadrootsOrderCancellation {
            reason: " ".into(),
            ..sample_order_cancellation()
        };
        assert_eq!(
            missing_reason.validate().unwrap_err(),
            RadrootsOrderPayloadError::EmptyField("reason")
        );

        let missing_buyer = RadrootsOrderCancellation {
            buyer_pubkey: " ".into(),
            ..sample_order_cancellation()
        };
        assert_eq!(
            missing_buyer.validate().unwrap_err(),
            RadrootsOrderPayloadError::EmptyField("buyer_pubkey")
        );
    }

    #[test]
    fn order_buyer_receipt_validation_requires_consistent_received_and_issue() {
        assert_eq!(sample_order_buyer_receipt(true).validate(), Ok(()));
        assert_eq!(sample_order_buyer_receipt(false).validate(), Ok(()));

        let received_with_issue = RadrootsOrderReceipt {
            issue: Some("damaged".into()),
            ..sample_order_buyer_receipt(true)
        };
        assert_eq!(
            received_with_issue.validate().unwrap_err(),
            RadrootsOrderPayloadError::UnexpectedReceiptIssue
        );

        let not_received_without_issue = RadrootsOrderReceipt {
            issue: None,
            ..sample_order_buyer_receipt(false)
        };
        assert_eq!(
            not_received_without_issue.validate().unwrap_err(),
            RadrootsOrderPayloadError::MissingReceiptIssue
        );

        let not_received_blank_issue = RadrootsOrderReceipt {
            issue: Some(" ".into()),
            ..sample_order_buyer_receipt(false)
        };
        assert_eq!(
            not_received_blank_issue.validate().unwrap_err(),
            RadrootsOrderPayloadError::EmptyField("issue")
        );
    }

    #[test]
    fn order_payment_and_settlement_validation_covers_amount_and_reason_paths() {
        assert_eq!(sample_payment_recorded().validate(), Ok(()));

        let unreferenced_payment = RadrootsOrderPaymentRecord {
            reference: None,
            ..sample_payment_recorded()
        };
        assert_eq!(unreferenced_payment.validate(), Ok(()));

        let invalid_quote_version = RadrootsOrderPaymentRecord {
            quote_version: 0,
            ..sample_payment_recorded()
        };
        assert_eq!(
            invalid_quote_version.validate().unwrap_err(),
            RadrootsOrderPayloadError::InvalidQuoteVersion
        );

        let invalid_amount = RadrootsOrderPaymentRecord {
            amount: decimal("0"),
            ..sample_payment_recorded()
        };
        assert_eq!(
            invalid_amount.validate().unwrap_err(),
            RadrootsOrderPayloadError::InvalidPaymentAmount
        );

        let negative_amount = RadrootsOrderPaymentRecord {
            amount: decimal("-1"),
            ..sample_payment_recorded()
        };
        assert_eq!(
            negative_amount.validate().unwrap_err(),
            RadrootsOrderPayloadError::InvalidPaymentAmount
        );

        let blank_reference = RadrootsOrderPaymentRecord {
            reference: Some(" ".into()),
            ..sample_payment_recorded()
        };
        assert_eq!(
            blank_reference.validate().unwrap_err(),
            RadrootsOrderPayloadError::EmptyField("reference")
        );

        assert_eq!(
            sample_settlement_decision(RadrootsOrderSettlementOutcome::Accepted, None).validate(),
            Ok(())
        );
        assert_eq!(
            sample_settlement_decision(RadrootsOrderSettlementOutcome::Rejected, Some("damaged"))
                .validate(),
            Ok(())
        );

        let accepted_with_reason =
            sample_settlement_decision(RadrootsOrderSettlementOutcome::Accepted, Some("extra"));
        assert_eq!(
            accepted_with_reason.validate().unwrap_err(),
            RadrootsOrderPayloadError::UnexpectedSettlementReason
        );

        let rejected_without_reason =
            sample_settlement_decision(RadrootsOrderSettlementOutcome::Rejected, None);
        assert_eq!(
            rejected_without_reason.validate().unwrap_err(),
            RadrootsOrderPayloadError::MissingSettlementReason
        );

        let rejected_blank_reason =
            sample_settlement_decision(RadrootsOrderSettlementOutcome::Rejected, Some(" "));
        assert_eq!(
            rejected_blank_reason.validate().unwrap_err(),
            RadrootsOrderPayloadError::EmptyField("reason")
        );

        let invalid_quote_version = RadrootsOrderSettlementDecision {
            quote_version: 0,
            ..sample_settlement_decision(RadrootsOrderSettlementOutcome::Accepted, None)
        };
        assert_eq!(
            invalid_quote_version.validate().unwrap_err(),
            RadrootsOrderPayloadError::InvalidQuoteVersion
        );

        let zero_amount = RadrootsOrderSettlementDecision {
            amount: decimal("0"),
            ..sample_settlement_decision(RadrootsOrderSettlementOutcome::Accepted, None)
        };
        assert_eq!(
            zero_amount.validate().unwrap_err(),
            RadrootsOrderPayloadError::InvalidPaymentAmount
        );

        let invalid_amount = RadrootsOrderSettlementDecision {
            amount: decimal("-1"),
            ..sample_settlement_decision(RadrootsOrderSettlementOutcome::Accepted, None)
        };
        assert_eq!(
            invalid_amount.validate().unwrap_err(),
            RadrootsOrderPayloadError::InvalidPaymentAmount
        );
    }

    #[test]
    fn order_envelope_serializes_canonical_type_name() {
        let envelope = RadrootsOrderEnvelope::new(
            RadrootsOrderEventType::OrderRequested,
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
        let invalid_version = RadrootsOrderEnvelope {
            version: RADROOTS_ORDER_ENVELOPE_VERSION + 1,
            domain: RadrootsCommercialDomain::Listing,
            message_type: RadrootsOrderEventType::OrderRequested,
            order_id: "order-1".into(),
            listing_addr: sample_listing_addr().into_string(),
            payload: sample_order_request(),
        };
        let invalid_version_err = invalid_version.validate().unwrap_err();
        assert_eq!(
            invalid_version_err,
            RadrootsOrderEnvelopeError::InvalidVersion {
                expected: RADROOTS_ORDER_ENVELOPE_VERSION,
                got: RADROOTS_ORDER_ENVELOPE_VERSION + 1,
            }
        );
        assert_eq!(
            invalid_version_err.to_string(),
            "invalid order envelope version: expected 1, got 2"
        );

        let missing_order = RadrootsOrderEnvelope::new(
            RadrootsOrderEventType::OrderRequested,
            sample_listing_addr(),
            " ",
            sample_order_request(),
        );
        let missing_order_err = missing_order.validate().unwrap_err();
        assert_eq!(
            missing_order_err,
            RadrootsOrderEnvelopeError::MissingOrderId
        );
        assert_eq!(
            missing_order_err.to_string(),
            "missing order_id for order message"
        );

        let missing_listing = RadrootsOrderEnvelope::new(
            RadrootsOrderEventType::OrderRequested,
            " ",
            "order-1",
            sample_order_request(),
        );
        let missing_listing_err = missing_listing.validate().unwrap_err();
        assert_eq!(
            missing_listing_err,
            RadrootsOrderEnvelopeError::MissingListingAddr
        );
        assert_eq!(missing_listing_err.to_string(), "missing listing_addr");
    }

    #[test]
    fn listing_parse_error_display_variants() {
        assert_eq!(
            RadrootsListingParseError::InvalidKind(KIND_PROFILE).to_string(),
            "invalid listing kind: 0"
        );
        assert_eq!(
            RadrootsListingParseError::MissingTag("price".into()).to_string(),
            "missing required tag: price"
        );
        assert_eq!(
            RadrootsListingParseError::InvalidTag("farm".into()).to_string(),
            "invalid tag: farm"
        );
        assert_eq!(
            RadrootsListingParseError::InvalidNumber("inventory".into()).to_string(),
            "invalid number: inventory"
        );
        assert_eq!(
            RadrootsListingParseError::InvalidUnit.to_string(),
            "invalid unit"
        );
        assert_eq!(
            RadrootsListingParseError::InvalidCurrency.to_string(),
            "invalid currency"
        );
        assert_eq!(
            RadrootsListingParseError::InvalidJson("bins".into()).to_string(),
            "invalid json: bins"
        );
        assert_eq!(
            RadrootsListingParseError::InvalidDiscount("offer".into()).to_string(),
            "invalid discount data for offer"
        );
    }

    #[test]
    fn listing_validation_error_display_variants() {
        assert_eq!(
            (RadrootsTradeValidationListingError::InvalidKind { kind: KIND_PROFILE }).to_string(),
            "invalid listing kind: 0"
        );
        assert_eq!(
            RadrootsTradeValidationListingError::MissingListingId.to_string(),
            "missing listing id"
        );
        assert_eq!(
            RadrootsTradeValidationListingError::ListingEventNotFound {
                listing_addr: "listing-1".into(),
            }
            .to_string(),
            "listing event not found: listing-1"
        );
        assert_eq!(
            RadrootsTradeValidationListingError::ListingEventFetchFailed {
                listing_addr: "listing-2".into(),
            }
            .to_string(),
            "listing event fetch failed: listing-2"
        );
        assert_eq!(
            RadrootsTradeValidationListingError::ParseError {
                error: RadrootsListingParseError::InvalidJson("payload".into()),
            }
            .to_string(),
            "invalid listing data: invalid json: payload"
        );
        assert_eq!(
            RadrootsTradeValidationListingError::InvalidSeller.to_string(),
            "listing author does not match farm pubkey"
        );
        assert_eq!(
            RadrootsTradeValidationListingError::MissingFarmProfile.to_string(),
            "missing farm profile"
        );
        assert_eq!(
            RadrootsTradeValidationListingError::MissingFarmRecord.to_string(),
            "missing farm record"
        );
        assert_eq!(
            RadrootsTradeValidationListingError::MissingTitle.to_string(),
            "missing listing title"
        );
        assert_eq!(
            RadrootsTradeValidationListingError::MissingDescription.to_string(),
            "missing listing description"
        );
        assert_eq!(
            RadrootsTradeValidationListingError::MissingProductType.to_string(),
            "missing listing product type"
        );
        assert_eq!(
            RadrootsTradeValidationListingError::MissingBins.to_string(),
            "missing listing bins"
        );
        assert_eq!(
            RadrootsTradeValidationListingError::MissingPrimaryBin.to_string(),
            "missing primary listing bin"
        );
        assert_eq!(
            RadrootsTradeValidationListingError::InvalidBin.to_string(),
            "invalid listing bin"
        );
        assert_eq!(
            RadrootsTradeValidationListingError::MissingPrice.to_string(),
            "missing listing price"
        );
        assert_eq!(
            RadrootsTradeValidationListingError::InvalidPrice.to_string(),
            "invalid listing price"
        );
        assert_eq!(
            RadrootsTradeValidationListingError::MissingInventory.to_string(),
            "missing listing inventory"
        );
        assert_eq!(
            RadrootsTradeValidationListingError::InvalidInventory.to_string(),
            "invalid listing inventory"
        );
        assert_eq!(
            RadrootsTradeValidationListingError::MissingAvailability.to_string(),
            "missing listing availability"
        );
        assert_eq!(
            RadrootsTradeValidationListingError::MissingLocation.to_string(),
            "missing listing location"
        );
        assert_eq!(
            RadrootsTradeValidationListingError::MissingDeliveryMethod.to_string(),
            "missing listing delivery method"
        );
    }

    #[test]
    fn order_payload_error_display_variants_cover_all_messages() {
        let cases = [
            (
                RadrootsOrderPayloadError::EmptyField("field"),
                "field cannot be empty",
            ),
            (
                RadrootsOrderPayloadError::MissingItems,
                "items must contain at least one item",
            ),
            (
                RadrootsOrderPayloadError::InvalidItemBinCount { index: 2 },
                "items[2].bin_count must be greater than zero",
            ),
            (
                RadrootsOrderPayloadError::MissingEconomicItems,
                "economics.items must contain at least one item",
            ),
            (
                RadrootsOrderPayloadError::InvalidEconomicItemBinCount { index: 3 },
                "economics.items[3].bin_count must be greater than zero",
            ),
            (
                RadrootsOrderPayloadError::InvalidEconomicItemQuantity { index: 4 },
                "economics.items[4].quantity_amount must be greater than zero",
            ),
            (
                RadrootsOrderPayloadError::InvalidEconomicItemPrice { index: 5 },
                "economics.items[5].unit_price_amount must not be negative",
            ),
            (
                RadrootsOrderPayloadError::InvalidEconomicItemSubtotal { index: 6 },
                "economics.items[6].line_subtotal is invalid",
            ),
            (
                RadrootsOrderPayloadError::InvalidEconomicLineAmount {
                    field: "adjustments",
                    index: 7,
                },
                "economics.adjustments[7].amount must be greater than zero",
            ),
            (
                RadrootsOrderPayloadError::InvalidEconomicLineKind {
                    field: "discounts",
                    index: 8,
                },
                "economics.discounts[8].kind is invalid",
            ),
            (
                RadrootsOrderPayloadError::InvalidEconomicLineEffect {
                    field: "discounts",
                    index: 9,
                },
                "economics.discounts[9].effect is invalid",
            ),
            (
                RadrootsOrderPayloadError::InvalidEconomicCurrency { field: "total" },
                "economics.total currency is invalid",
            ),
            (
                RadrootsOrderPayloadError::InvalidEconomicOrdering { field: "items" },
                "economics.items is not in canonical order",
            ),
            (
                RadrootsOrderPayloadError::InvalidEconomicTotal { field: "subtotal" },
                "economics.subtotal total is invalid",
            ),
            (
                RadrootsOrderPayloadError::InvalidOrderEconomicsBinding { field: "items" },
                "order items does not match economics",
            ),
            (
                RadrootsOrderPayloadError::InvalidQuoteVersion,
                "economics.quote_version must be greater than zero",
            ),
            (
                RadrootsOrderPayloadError::MissingInventoryCommitments,
                "accepted decisions must contain at least one inventory commitment",
            ),
            (
                RadrootsOrderPayloadError::InvalidInventoryCommitmentCount { index: 1 },
                "inventory_commitments[1].bin_count must be greater than zero",
            ),
            (
                RadrootsOrderPayloadError::InvalidFulfillmentStatus,
                "fulfillment status is not publishable",
            ),
            (
                RadrootsOrderPayloadError::MissingReceiptIssue,
                "receipt issue is required when received is false",
            ),
            (
                RadrootsOrderPayloadError::UnexpectedReceiptIssue,
                "receipt issue must be absent when received is true",
            ),
            (
                RadrootsOrderPayloadError::InvalidPaymentAmount,
                "payment amount must be greater than zero",
            ),
            (
                RadrootsOrderPayloadError::MissingSettlementReason,
                "settlement reason is required when decision is rejected",
            ),
            (
                RadrootsOrderPayloadError::UnexpectedSettlementReason,
                "settlement reason must be absent when decision is accepted",
            ),
        ];

        for (error, expected) in cases {
            assert_eq!(error.to_string(), expected);
        }
    }
}
