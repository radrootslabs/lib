#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

#[cfg(feature = "serde_json")]
use radroots_events::{RadrootsNostrEvent, tags::TAG_D};
use radroots_events::{RadrootsNostrEventPtr, kinds::KIND_PROFILE};
use radroots_events_codec::d_tag::is_d_tag_base64url;
#[cfg(feature = "serde_json")]
use radroots_events_codec::error::{EventEncodeError, EventParseError};
#[cfg(feature = "serde_json")]
use serde::de::DeserializeOwned;
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

use crate::listing::kinds::{
    KIND_TRADE_LISTING_ANSWER_RES, KIND_TRADE_LISTING_CANCEL_REQ,
    KIND_TRADE_LISTING_DISCOUNT_ACCEPT_REQ, KIND_TRADE_LISTING_DISCOUNT_DECLINE_REQ,
    KIND_TRADE_LISTING_DISCOUNT_OFFER_RES, KIND_TRADE_LISTING_DISCOUNT_REQ,
    KIND_TRADE_LISTING_FULFILLMENT_UPDATE_REQ, KIND_TRADE_LISTING_ORDER_REQ,
    KIND_TRADE_LISTING_ORDER_RES, KIND_TRADE_LISTING_ORDER_REVISION_REQ,
    KIND_TRADE_LISTING_ORDER_REVISION_RES, KIND_TRADE_LISTING_QUESTION_REQ,
    KIND_TRADE_LISTING_RECEIPT_REQ, KIND_TRADE_LISTING_VALIDATE_REQ,
    KIND_TRADE_LISTING_VALIDATE_RES, is_trade_listing_kind,
};
use crate::listing::order::{
    TradeAnswer, TradeDiscountDecision, TradeDiscountOffer, TradeDiscountRequest,
    TradeFulfillmentUpdate, TradeOrder, TradeOrderRevision, TradeQuestion, TradeReceipt,
};
#[cfg(feature = "serde_json")]
use crate::listing::tags::trade_listing_dvm_tags;
#[cfg(feature = "serde_json")]
use crate::listing::tags::{
    TAG_LISTING_EVENT, parse_trade_listing_counterparty_tag, parse_trade_listing_event_tag,
    parse_trade_listing_prev_tag, parse_trade_listing_root_tag,
};
use crate::listing::validation::TradeListingValidationError;

pub const TRADE_LISTING_DOMAIN: &str = "trade:listing";
pub const TRADE_LISTING_ENVELOPE_VERSION: u16 = 1;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TradeListingDomain {
    #[cfg_attr(feature = "serde", serde(rename = "trade:listing"))]
    TradeListing,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TradeListingMessageType {
    ListingValidateRequest,
    ListingValidateResult,
    OrderRequest,
    OrderResponse,
    OrderRevision,
    OrderRevisionAccept,
    OrderRevisionDecline,
    Question,
    Answer,
    DiscountRequest,
    DiscountOffer,
    DiscountAccept,
    DiscountDecline,
    Cancel,
    FulfillmentUpdate,
    Receipt,
}

impl TradeListingMessageType {
    #[inline]
    pub const fn from_kind(kind: u16) -> Option<Self> {
        match kind {
            KIND_TRADE_LISTING_VALIDATE_REQ => {
                Some(TradeListingMessageType::ListingValidateRequest)
            }
            KIND_TRADE_LISTING_VALIDATE_RES => Some(TradeListingMessageType::ListingValidateResult),
            KIND_TRADE_LISTING_ORDER_REQ => Some(TradeListingMessageType::OrderRequest),
            KIND_TRADE_LISTING_ORDER_RES => Some(TradeListingMessageType::OrderResponse),
            KIND_TRADE_LISTING_ORDER_REVISION_REQ => Some(TradeListingMessageType::OrderRevision),
            KIND_TRADE_LISTING_ORDER_REVISION_RES => None,
            KIND_TRADE_LISTING_QUESTION_REQ => Some(TradeListingMessageType::Question),
            KIND_TRADE_LISTING_ANSWER_RES => Some(TradeListingMessageType::Answer),
            KIND_TRADE_LISTING_DISCOUNT_REQ => Some(TradeListingMessageType::DiscountRequest),
            KIND_TRADE_LISTING_DISCOUNT_OFFER_RES => Some(TradeListingMessageType::DiscountOffer),
            KIND_TRADE_LISTING_DISCOUNT_ACCEPT_REQ => Some(TradeListingMessageType::DiscountAccept),
            KIND_TRADE_LISTING_DISCOUNT_DECLINE_REQ => {
                Some(TradeListingMessageType::DiscountDecline)
            }
            KIND_TRADE_LISTING_CANCEL_REQ => Some(TradeListingMessageType::Cancel),
            KIND_TRADE_LISTING_FULFILLMENT_UPDATE_REQ => {
                Some(TradeListingMessageType::FulfillmentUpdate)
            }
            KIND_TRADE_LISTING_RECEIPT_REQ => Some(TradeListingMessageType::Receipt),
            _ => None,
        }
    }

    #[inline]
    pub const fn kind(self) -> u16 {
        match self {
            TradeListingMessageType::ListingValidateRequest => KIND_TRADE_LISTING_VALIDATE_REQ,
            TradeListingMessageType::ListingValidateResult => KIND_TRADE_LISTING_VALIDATE_RES,
            TradeListingMessageType::OrderRequest => KIND_TRADE_LISTING_ORDER_REQ,
            TradeListingMessageType::OrderResponse => KIND_TRADE_LISTING_ORDER_RES,
            TradeListingMessageType::OrderRevision => KIND_TRADE_LISTING_ORDER_REVISION_REQ,
            TradeListingMessageType::OrderRevisionAccept => KIND_TRADE_LISTING_ORDER_REVISION_RES,
            TradeListingMessageType::OrderRevisionDecline => KIND_TRADE_LISTING_ORDER_REVISION_RES,
            TradeListingMessageType::Question => KIND_TRADE_LISTING_QUESTION_REQ,
            TradeListingMessageType::Answer => KIND_TRADE_LISTING_ANSWER_RES,
            TradeListingMessageType::DiscountRequest => KIND_TRADE_LISTING_DISCOUNT_REQ,
            TradeListingMessageType::DiscountOffer => KIND_TRADE_LISTING_DISCOUNT_OFFER_RES,
            TradeListingMessageType::DiscountAccept => KIND_TRADE_LISTING_DISCOUNT_ACCEPT_REQ,
            TradeListingMessageType::DiscountDecline => KIND_TRADE_LISTING_DISCOUNT_DECLINE_REQ,
            TradeListingMessageType::Cancel => KIND_TRADE_LISTING_CANCEL_REQ,
            TradeListingMessageType::FulfillmentUpdate => KIND_TRADE_LISTING_FULFILLMENT_UPDATE_REQ,
            TradeListingMessageType::Receipt => KIND_TRADE_LISTING_RECEIPT_REQ,
        }
    }

    #[inline]
    pub const fn requires_order_id(self) -> bool {
        !matches!(
            self,
            TradeListingMessageType::ListingValidateRequest
                | TradeListingMessageType::ListingValidateResult
        )
    }

    #[inline]
    pub const fn requires_listing_snapshot(self) -> bool {
        matches!(
            self,
            TradeListingMessageType::OrderRequest
                | TradeListingMessageType::OrderRevision
                | TradeListingMessageType::DiscountRequest
                | TradeListingMessageType::DiscountOffer
        )
    }

    #[inline]
    pub const fn requires_trade_chain(self) -> bool {
        !matches!(
            self,
            TradeListingMessageType::ListingValidateRequest
                | TradeListingMessageType::ListingValidateResult
                | TradeListingMessageType::OrderRequest
        )
    }

    #[inline]
    pub const fn is_request(self) -> bool {
        matches!(
            self,
            TradeListingMessageType::ListingValidateRequest
                | TradeListingMessageType::OrderRequest
                | TradeListingMessageType::OrderRevision
                | TradeListingMessageType::Question
                | TradeListingMessageType::DiscountRequest
                | TradeListingMessageType::DiscountAccept
                | TradeListingMessageType::DiscountDecline
                | TradeListingMessageType::Cancel
                | TradeListingMessageType::FulfillmentUpdate
                | TradeListingMessageType::Receipt
        )
    }

    #[inline]
    pub const fn is_result(self) -> bool {
        !self.is_request()
    }
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TradeListingEnvelope<T> {
    pub version: u16,
    pub domain: TradeListingDomain,
    #[cfg_attr(feature = "serde", serde(rename = "type"))]
    pub message_type: TradeListingMessageType,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub order_id: Option<String>,
    pub listing_addr: String,
    pub payload: T,
}

impl<T> TradeListingEnvelope<T> {
    #[inline]
    pub fn new(
        message_type: TradeListingMessageType,
        listing_addr: impl Into<String>,
        order_id: Option<String>,
        payload: T,
    ) -> Self {
        Self {
            version: TRADE_LISTING_ENVELOPE_VERSION,
            domain: TradeListingDomain::TradeListing,
            message_type,
            order_id,
            listing_addr: listing_addr.into(),
            payload,
        }
    }

    pub fn validate(&self) -> Result<(), TradeListingEnvelopeError> {
        if self.version != TRADE_LISTING_ENVELOPE_VERSION {
            return Err(TradeListingEnvelopeError::InvalidVersion {
                expected: TRADE_LISTING_ENVELOPE_VERSION,
                got: self.version,
            });
        }
        if self.listing_addr.trim().is_empty() {
            return Err(TradeListingEnvelopeError::MissingListingAddr);
        }
        if self.message_type.requires_order_id() {
            match self.order_id.as_deref() {
                Some(id) if !id.trim().is_empty() => {}
                _ => return Err(TradeListingEnvelopeError::MissingOrderId),
            }
        }
        Ok(())
    }
}

#[cfg(feature = "serde_json")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TradeListingEnvelopeEvent {
    pub kind: u16,
    pub content: String,
    pub tags: Vec<Vec<String>>,
}

#[cfg(feature = "serde_json")]
fn map_envelope_error(error: TradeListingEnvelopeError) -> EventEncodeError {
    match error {
        TradeListingEnvelopeError::MissingOrderId => {
            EventEncodeError::EmptyRequiredField("order_id")
        }
        TradeListingEnvelopeError::MissingListingAddr => {
            EventEncodeError::EmptyRequiredField("listing_addr")
        }
        TradeListingEnvelopeError::InvalidVersion { .. } => {
            EventEncodeError::InvalidField("version")
        }
    }
}

#[cfg(feature = "serde_json")]
pub fn trade_listing_envelope_event_build(
    recipient_pubkey: impl Into<String>,
    message_type: TradeListingMessageType,
    listing_addr: impl Into<String>,
    order_id: Option<String>,
    listing_event: Option<&RadrootsNostrEventPtr>,
    root_event_id: Option<&str>,
    prev_event_id: Option<&str>,
    payload: &TradeListingMessagePayload,
) -> Result<TradeListingEnvelopeEvent, EventEncodeError> {
    if payload.message_type() != message_type {
        return Err(EventEncodeError::InvalidField("payload"));
    }
    if message_type.requires_listing_snapshot() && listing_event.is_none() {
        return Err(EventEncodeError::EmptyRequiredField("listing_event.id"));
    }
    if message_type.requires_trade_chain() {
        if root_event_id.is_none() {
            return Err(EventEncodeError::EmptyRequiredField("root_event_id"));
        }
        if prev_event_id.is_none() {
            return Err(EventEncodeError::EmptyRequiredField("prev_event_id"));
        }
    }
    let listing_addr = listing_addr.into();
    let envelope = TradeListingEnvelope::new(
        message_type,
        listing_addr.clone(),
        order_id.clone(),
        payload.clone(),
    );
    envelope.validate().map_err(map_envelope_error)?;
    let content = serde_json::to_string(&envelope).map_err(|_| EventEncodeError::Json)?;
    let tags = trade_listing_dvm_tags(
        recipient_pubkey,
        &listing_addr,
        order_id.as_deref(),
        listing_event,
        root_event_id,
        prev_event_id,
    )?;
    Ok(TradeListingEnvelopeEvent {
        kind: message_type.kind(),
        content,
        tags,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TradeListingEnvelopeError {
    InvalidVersion { expected: u16, got: u16 },
    MissingOrderId,
    MissingListingAddr,
}

impl core::fmt::Display for TradeListingEnvelopeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            TradeListingEnvelopeError::InvalidVersion { expected, got } => {
                write!(
                    f,
                    "invalid envelope version: expected {expected}, got {got}"
                )
            }
            TradeListingEnvelopeError::MissingOrderId => {
                write!(f, "missing order_id for order-scoped message")
            }
            TradeListingEnvelopeError::MissingListingAddr => {
                write!(f, "missing listing_addr")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for TradeListingEnvelopeError {}

#[cfg(feature = "serde_json")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TradeListingEnvelopeParseError {
    InvalidKind(u32),
    InvalidJson,
    InvalidEnvelope(TradeListingEnvelopeError),
    MessageTypeKindMismatch {
        event_kind: u32,
        message_type: TradeListingMessageType,
    },
    MissingTag(&'static str),
    InvalidTag(&'static str),
    ListingAddrTagMismatch,
    OrderIdTagMismatch,
    InvalidListingAddr(TradeListingAddressError),
}

#[cfg(feature = "serde_json")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TradeListingEventContext {
    pub counterparty_pubkey: String,
    pub listing_event: Option<RadrootsNostrEventPtr>,
    pub root_event_id: Option<String>,
    pub prev_event_id: Option<String>,
}

#[cfg(feature = "serde_json")]
impl core::fmt::Display for TradeListingEnvelopeParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            TradeListingEnvelopeParseError::InvalidKind(kind) => {
                write!(f, "invalid trade listing event kind: {kind}")
            }
            TradeListingEnvelopeParseError::InvalidJson => {
                write!(f, "invalid trade listing envelope json")
            }
            TradeListingEnvelopeParseError::InvalidEnvelope(error) => write!(f, "{error}"),
            TradeListingEnvelopeParseError::MessageTypeKindMismatch {
                event_kind,
                message_type,
            } => write!(
                f,
                "trade listing envelope type {message_type:?} does not match event kind {event_kind}"
            ),
            TradeListingEnvelopeParseError::MissingTag(tag) => {
                write!(f, "missing required trade listing tag: {tag}")
            }
            TradeListingEnvelopeParseError::InvalidTag(tag) => {
                write!(f, "invalid trade listing tag: {tag}")
            }
            TradeListingEnvelopeParseError::ListingAddrTagMismatch => {
                write!(f, "trade listing address tag does not match envelope")
            }
            TradeListingEnvelopeParseError::OrderIdTagMismatch => {
                write!(f, "trade order id tag does not match envelope")
            }
            TradeListingEnvelopeParseError::InvalidListingAddr(error) => write!(f, "{error}"),
        }
    }
}

#[cfg(feature = "std")]
#[cfg(feature = "serde_json")]
impl std::error::Error for TradeListingEnvelopeParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            TradeListingEnvelopeParseError::InvalidEnvelope(error) => Some(error),
            TradeListingEnvelopeParseError::InvalidListingAddr(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TradeListingAddress {
    pub kind: u16,
    pub seller_pubkey: String,
    pub listing_id: String,
}

impl TradeListingAddress {
    pub fn parse(addr: &str) -> Result<Self, TradeListingAddressError> {
        let (kind_raw, seller_and_listing) = addr
            .split_once(':')
            .ok_or(TradeListingAddressError::InvalidFormat)?;
        let (seller_pubkey_raw, listing_id_raw) = seller_and_listing
            .split_once(':')
            .ok_or(TradeListingAddressError::InvalidFormat)?;
        if listing_id_raw.contains(':') {
            return Err(TradeListingAddressError::InvalidFormat);
        }
        let kind = kind_raw
            .parse::<u16>()
            .map_err(|_| TradeListingAddressError::InvalidFormat)?;
        let seller_pubkey = seller_pubkey_raw.to_string();
        let listing_id = listing_id_raw.to_string();
        if kind == KIND_PROFILE as u16
            || seller_pubkey.trim().is_empty()
            || listing_id.trim().is_empty()
            || !is_d_tag_base64url(&listing_id)
        {
            return Err(TradeListingAddressError::InvalidFormat);
        }
        Ok(Self {
            kind,
            seller_pubkey,
            listing_id,
        })
    }

    #[inline]
    pub fn as_str(&self) -> String {
        format!("{}:{}:{}", self.kind, self.seller_pubkey, self.listing_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TradeListingAddressError {
    InvalidFormat,
}

impl core::fmt::Display for TradeListingAddressError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            TradeListingAddressError::InvalidFormat => {
                write!(f, "invalid listing address format")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for TradeListingAddressError {}

#[cfg(feature = "serde_json")]
fn required_tag_value<'a>(
    tags: &'a [Vec<String>],
    key: &'static str,
) -> Result<&'a str, TradeListingEnvelopeParseError> {
    let tag = tags
        .iter()
        .find(|tag| tag.first().map(|value| value.as_str()) == Some(key))
        .ok_or(TradeListingEnvelopeParseError::MissingTag(key))?;
    let value = tag
        .get(1)
        .map(|value| value.as_str())
        .ok_or(TradeListingEnvelopeParseError::InvalidTag(key))?;
    if value.trim().is_empty() {
        return Err(TradeListingEnvelopeParseError::InvalidTag(key));
    }
    Ok(value)
}

#[cfg(feature = "serde_json")]
impl<T> TradeListingEnvelope<T>
where
    T: DeserializeOwned,
{
    pub fn from_event(event: &RadrootsNostrEvent) -> Result<Self, TradeListingEnvelopeParseError> {
        let event_kind = u16::try_from(event.kind)
            .map_err(|_| TradeListingEnvelopeParseError::InvalidKind(event.kind))?;
        if !is_trade_listing_kind(event_kind) {
            return Err(TradeListingEnvelopeParseError::InvalidKind(event.kind));
        }
        let envelope = serde_json::from_str::<Self>(&event.content)
            .map_err(|_| TradeListingEnvelopeParseError::InvalidJson)?;
        envelope
            .validate()
            .map_err(TradeListingEnvelopeParseError::InvalidEnvelope)?;
        if envelope.message_type.kind() != event_kind {
            return Err(TradeListingEnvelopeParseError::MessageTypeKindMismatch {
                event_kind: event.kind,
                message_type: envelope.message_type,
            });
        }

        let listing_addr = required_tag_value(&event.tags, "a")?;
        if envelope.listing_addr != listing_addr {
            return Err(TradeListingEnvelopeParseError::ListingAddrTagMismatch);
        }
        TradeListingAddress::parse(&envelope.listing_addr)
            .map_err(TradeListingEnvelopeParseError::InvalidListingAddr)?;

        if let Some(order_id) = envelope.order_id.as_deref() {
            let tag_order_id = required_tag_value(&event.tags, TAG_D)?;
            if tag_order_id != order_id {
                return Err(TradeListingEnvelopeParseError::OrderIdTagMismatch);
            }
        }

        trade_listing_event_context_from_tags(envelope.message_type, &event.tags)?;

        Ok(envelope)
    }
}

#[cfg(feature = "serde_json")]
pub fn trade_listing_event_context_from_tags(
    message_type: TradeListingMessageType,
    tags: &[Vec<String>],
) -> Result<TradeListingEventContext, TradeListingEnvelopeParseError> {
    let counterparty_pubkey =
        parse_trade_listing_counterparty_tag(tags).map_err(map_event_parse_error)?;
    let listing_event = parse_trade_listing_event_tag(tags).map_err(map_event_parse_error)?;
    let root_event_id = parse_trade_listing_root_tag(tags).map_err(map_event_parse_error)?;
    let prev_event_id = parse_trade_listing_prev_tag(tags).map_err(map_event_parse_error)?;
    if message_type.requires_listing_snapshot() && listing_event.is_none() {
        return Err(TradeListingEnvelopeParseError::MissingTag(
            TAG_LISTING_EVENT,
        ));
    }
    if message_type.requires_trade_chain() {
        if root_event_id.is_none() {
            return Err(TradeListingEnvelopeParseError::MissingTag(
                radroots_events::tags::TAG_E_ROOT,
            ));
        }
        if prev_event_id.is_none() {
            return Err(TradeListingEnvelopeParseError::MissingTag(
                radroots_events::tags::TAG_E_PREV,
            ));
        }
    }
    Ok(TradeListingEventContext {
        counterparty_pubkey,
        listing_event,
        root_event_id,
        prev_event_id,
    })
}

#[cfg(feature = "serde_json")]
fn map_event_parse_error(error: EventParseError) -> TradeListingEnvelopeParseError {
    match error {
        EventParseError::MissingTag(tag) => TradeListingEnvelopeParseError::MissingTag(tag),
        EventParseError::InvalidTag(tag) => TradeListingEnvelopeParseError::InvalidTag(tag),
        EventParseError::InvalidKind { expected: _, got } => {
            TradeListingEnvelopeParseError::InvalidKind(got)
        }
        EventParseError::InvalidNumber(tag, _) | EventParseError::InvalidJson(tag) => {
            TradeListingEnvelopeParseError::InvalidTag(tag)
        }
    }
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TradeListingValidateRequest {
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "RadrootsNostrEventPtr | null"))]
    pub listing_event: Option<RadrootsNostrEventPtr>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TradeListingValidateResult {
    pub valid: bool,
    #[cfg_attr(feature = "ts-rs", ts(type = "TradeListingValidationError[]"))]
    pub errors: Vec<TradeListingValidationError>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TradeOrderResponse {
    pub accepted: bool,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub reason: Option<String>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TradeOrderRevisionResponse {
    pub accepted: bool,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub reason: Option<String>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TradeListingCancel {
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub reason: Option<String>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(rename_all = "snake_case", tag = "kind", content = "amount")
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TradeListingMessagePayload {
    ListingValidateRequest(TradeListingValidateRequest),
    ListingValidateResult(TradeListingValidateResult),
    OrderRequest(TradeOrder),
    OrderResponse(TradeOrderResponse),
    OrderRevision(TradeOrderRevision),
    OrderRevisionAccept(TradeOrderRevisionResponse),
    OrderRevisionDecline(TradeOrderRevisionResponse),
    Question(TradeQuestion),
    Answer(TradeAnswer),
    DiscountRequest(TradeDiscountRequest),
    DiscountOffer(TradeDiscountOffer),
    DiscountAccept(TradeDiscountDecision),
    DiscountDecline(TradeDiscountDecision),
    Cancel(TradeListingCancel),
    FulfillmentUpdate(TradeFulfillmentUpdate),
    Receipt(TradeReceipt),
}

impl TradeListingMessagePayload {
    pub const fn message_type(&self) -> TradeListingMessageType {
        match self {
            TradeListingMessagePayload::ListingValidateRequest(_) => {
                TradeListingMessageType::ListingValidateRequest
            }
            TradeListingMessagePayload::ListingValidateResult(_) => {
                TradeListingMessageType::ListingValidateResult
            }
            TradeListingMessagePayload::OrderRequest(_) => TradeListingMessageType::OrderRequest,
            TradeListingMessagePayload::OrderResponse(_) => TradeListingMessageType::OrderResponse,
            TradeListingMessagePayload::OrderRevision(_) => TradeListingMessageType::OrderRevision,
            TradeListingMessagePayload::OrderRevisionAccept(_) => {
                TradeListingMessageType::OrderRevisionAccept
            }
            TradeListingMessagePayload::OrderRevisionDecline(_) => {
                TradeListingMessageType::OrderRevisionDecline
            }
            TradeListingMessagePayload::Question(_) => TradeListingMessageType::Question,
            TradeListingMessagePayload::Answer(_) => TradeListingMessageType::Answer,
            TradeListingMessagePayload::DiscountRequest(_) => {
                TradeListingMessageType::DiscountRequest
            }
            TradeListingMessagePayload::DiscountOffer(_) => TradeListingMessageType::DiscountOffer,
            TradeListingMessagePayload::DiscountAccept(_) => {
                TradeListingMessageType::DiscountAccept
            }
            TradeListingMessagePayload::DiscountDecline(_) => {
                TradeListingMessageType::DiscountDecline
            }
            TradeListingMessagePayload::Cancel(_) => TradeListingMessageType::Cancel,
            TradeListingMessagePayload::FulfillmentUpdate(_) => {
                TradeListingMessageType::FulfillmentUpdate
            }
            TradeListingMessagePayload::Receipt(_) => TradeListingMessageType::Receipt,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        TradeListingAddress, TradeListingAddressError, TradeListingEnvelope,
        TradeListingEnvelopeError, TradeListingEnvelopeParseError, TradeListingMessagePayload,
        TradeListingMessageType, TradeListingValidateRequest, TradeOrderResponse,
        trade_listing_envelope_event_build,
    };
    use radroots_events::kinds::KIND_LISTING;
    #[cfg(feature = "serde_json")]
    use radroots_events::{RadrootsNostrEvent, RadrootsNostrEventPtr};
    #[cfg(feature = "serde_json")]
    use radroots_events_codec::error::EventEncodeError;

    #[cfg(feature = "serde_json")]
    use crate::listing::order::{TradeOrder, TradeOrderItem};

    #[test]
    fn envelope_requires_listing_addr() {
        let env = TradeListingEnvelope::new(
            TradeListingMessageType::ListingValidateRequest,
            "",
            None,
            TradeListingValidateRequest {
                listing_event: None,
            },
        );
        assert_eq!(
            env.validate().unwrap_err(),
            TradeListingEnvelopeError::MissingListingAddr
        );
    }

    #[test]
    fn envelope_requires_order_id_for_order_scoped() {
        let env = TradeListingEnvelope::new(
            TradeListingMessageType::OrderRequest,
            format!("{KIND_LISTING}:pubkey:AAAAAAAAAAAAAAAAAAAAAg"),
            None,
            TradeListingValidateRequest {
                listing_event: None,
            },
        );
        assert_eq!(
            env.validate().unwrap_err(),
            TradeListingEnvelopeError::MissingOrderId
        );
    }

    #[test]
    fn envelope_accepts_non_empty_order_id_for_order_scoped() {
        let env = TradeListingEnvelope::new(
            TradeListingMessageType::OrderRequest,
            format!("{KIND_LISTING}:pubkey:AAAAAAAAAAAAAAAAAAAAAg"),
            Some("order-1".to_string()),
            TradeListingValidateRequest {
                listing_event: None,
            },
        );
        assert!(env.validate().is_ok());
    }

    #[test]
    fn envelope_rejects_blank_order_id_for_order_scoped() {
        let env = TradeListingEnvelope::new(
            TradeListingMessageType::OrderRequest,
            format!("{KIND_LISTING}:pubkey:AAAAAAAAAAAAAAAAAAAAAg"),
            Some(" ".to_string()),
            TradeListingValidateRequest {
                listing_event: None,
            },
        );
        assert_eq!(
            env.validate().unwrap_err(),
            TradeListingEnvelopeError::MissingOrderId
        );
    }

    #[test]
    fn envelope_accepts_non_order_message_without_order_id() {
        let env = TradeListingEnvelope::new(
            TradeListingMessageType::ListingValidateResult,
            format!("{KIND_LISTING}:pubkey:AAAAAAAAAAAAAAAAAAAAAg"),
            None,
            TradeListingValidateRequest {
                listing_event: None,
            },
        );
        assert!(env.validate().is_ok());
    }

    #[test]
    fn message_type_kind_and_request_flags_cover_all_variants() {
        let expected_kinds = crate::listing::kinds::TRADE_LISTING_KINDS;
        let assert_case =
            |message_type: TradeListingMessageType, is_request: bool, is_result: bool| {
                assert_eq!(message_type.is_request(), is_request);
                assert_eq!(message_type.is_result(), is_result);
                assert!(expected_kinds.contains(&message_type.kind()));
            };

        assert_case(TradeListingMessageType::ListingValidateRequest, true, false);
        assert_case(TradeListingMessageType::ListingValidateResult, false, true);
        assert_case(TradeListingMessageType::OrderRequest, true, false);
        assert_case(TradeListingMessageType::OrderResponse, false, true);
        assert_case(TradeListingMessageType::OrderRevision, true, false);
        assert_case(TradeListingMessageType::OrderRevisionAccept, false, true);
        assert_case(TradeListingMessageType::OrderRevisionDecline, false, true);
        assert_case(TradeListingMessageType::Question, true, false);
        assert_case(TradeListingMessageType::Answer, false, true);
        assert_case(TradeListingMessageType::DiscountRequest, true, false);
        assert_case(TradeListingMessageType::DiscountOffer, false, true);
        assert_case(TradeListingMessageType::DiscountAccept, true, false);
        assert_case(TradeListingMessageType::DiscountDecline, true, false);
        assert_case(TradeListingMessageType::Cancel, true, false);
        assert_case(TradeListingMessageType::FulfillmentUpdate, true, false);
        assert_case(TradeListingMessageType::Receipt, true, false);
    }

    #[test]
    fn message_type_from_kind_roundtrips_supported_variants() {
        for message_type in [
            TradeListingMessageType::ListingValidateRequest,
            TradeListingMessageType::ListingValidateResult,
            TradeListingMessageType::OrderRequest,
            TradeListingMessageType::OrderResponse,
            TradeListingMessageType::OrderRevision,
            TradeListingMessageType::Question,
            TradeListingMessageType::Answer,
            TradeListingMessageType::DiscountRequest,
            TradeListingMessageType::DiscountOffer,
            TradeListingMessageType::DiscountAccept,
            TradeListingMessageType::DiscountDecline,
            TradeListingMessageType::Cancel,
            TradeListingMessageType::FulfillmentUpdate,
            TradeListingMessageType::Receipt,
        ] {
            assert_eq!(
                TradeListingMessageType::from_kind(message_type.kind()),
                Some(message_type)
            );
        }
        assert_eq!(
            TradeListingMessageType::from_kind(super::KIND_TRADE_LISTING_ORDER_REVISION_RES),
            None
        );
        assert_eq!(TradeListingMessageType::from_kind(5000), None);
    }

    #[test]
    fn envelope_validate_rejects_invalid_version() {
        let mut env = TradeListingEnvelope::new(
            TradeListingMessageType::ListingValidateRequest,
            format!("{KIND_LISTING}:pubkey:AAAAAAAAAAAAAAAAAAAAAg"),
            None,
            TradeListingValidateRequest {
                listing_event: None,
            },
        );
        env.version = 9;
        assert_eq!(
            env.validate().unwrap_err(),
            TradeListingEnvelopeError::InvalidVersion {
                expected: super::TRADE_LISTING_ENVELOPE_VERSION,
                got: 9
            }
        );
    }

    #[test]
    fn envelope_error_display_messages_are_stable() {
        assert_eq!(
            TradeListingEnvelopeError::MissingOrderId.to_string(),
            "missing order_id for order-scoped message"
        );
        assert_eq!(
            TradeListingEnvelopeError::MissingListingAddr.to_string(),
            "missing listing_addr"
        );
        assert!(
            TradeListingEnvelopeError::InvalidVersion {
                expected: 1,
                got: 2
            }
            .to_string()
            .contains("expected 1, got 2")
        );
    }

    #[test]
    fn trade_listing_address_parse_and_render_roundtrip() {
        let addr_raw = format!("{KIND_LISTING}:seller:AAAAAAAAAAAAAAAAAAAAAg");
        let parsed = TradeListingAddress::parse(&addr_raw).expect("valid address");
        assert_eq!(parsed.kind, KIND_LISTING as u16);
        assert_eq!(parsed.seller_pubkey, "seller");
        assert_eq!(parsed.listing_id, "AAAAAAAAAAAAAAAAAAAAAg");
        assert_eq!(parsed.as_str(), addr_raw);
    }

    #[test]
    fn trade_listing_address_parse_rejects_invalid_shapes() {
        assert_eq!(
            TradeListingAddress::parse("not-a-kind:seller:AAAAAAAAAAAAAAAAAAAAAg").unwrap_err(),
            TradeListingAddressError::InvalidFormat
        );
        assert_eq!(
            TradeListingAddress::parse("30340").unwrap_err(),
            TradeListingAddressError::InvalidFormat
        );
        assert_eq!(
            TradeListingAddress::parse("30340:seller").unwrap_err(),
            TradeListingAddressError::InvalidFormat
        );
        assert_eq!(
            TradeListingAddress::parse("30340:seller:AAAAAAAAAAAAAAAAAAAAAg:extra").unwrap_err(),
            TradeListingAddressError::InvalidFormat
        );
        assert_eq!(
            TradeListingAddress::parse("0:seller:AAAAAAAAAAAAAAAAAAAAAg").unwrap_err(),
            TradeListingAddressError::InvalidFormat
        );
        assert_eq!(
            TradeListingAddress::parse("30340: :AAAAAAAAAAAAAAAAAAAAAg").unwrap_err(),
            TradeListingAddressError::InvalidFormat
        );
        assert_eq!(
            TradeListingAddress::parse("30340:seller: ").unwrap_err(),
            TradeListingAddressError::InvalidFormat
        );
        assert_eq!(
            TradeListingAddress::parse("30340:seller:not-base64").unwrap_err(),
            TradeListingAddressError::InvalidFormat
        );
    }

    #[test]
    fn trade_listing_address_error_display_message_is_stable() {
        assert_eq!(
            TradeListingAddressError::InvalidFormat.to_string(),
            "invalid listing address format"
        );
    }

    #[cfg(feature = "serde_json")]
    fn base_order() -> TradeOrder {
        TradeOrder {
            order_id: "order-1".into(),
            listing_addr: format!("{KIND_LISTING}:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg"),
            buyer_pubkey: "buyer-pubkey".into(),
            seller_pubkey: "seller-pubkey".into(),
            items: vec![TradeOrderItem {
                bin_id: "bin-1".into(),
                bin_count: 2,
            }],
            discounts: None,
        }
    }

    #[cfg(feature = "serde_json")]
    fn listing_snapshot() -> RadrootsNostrEventPtr {
        RadrootsNostrEventPtr {
            id: "listing-event-id".into(),
            relays: None,
        }
    }

    #[cfg(feature = "serde_json")]
    fn base_event(
        actor_pubkey: &str,
        recipient_pubkey: &str,
        message_type: TradeListingMessageType,
        listing_addr: &str,
        order_id: Option<&str>,
        payload: &TradeListingMessagePayload,
    ) -> RadrootsNostrEvent {
        let message_type = payload.message_type();
        let listing_event = message_type
            .requires_listing_snapshot()
            .then(listing_snapshot);
        let built = trade_listing_envelope_event_build(
            recipient_pubkey,
            message_type,
            listing_addr.to_string(),
            order_id.map(str::to_string),
            listing_event.as_ref(),
            None,
            None,
            payload,
        )
        .expect("canonical envelope event");
        RadrootsNostrEvent {
            id: "event-id".into(),
            author: actor_pubkey.into(),
            created_at: 1_700_000_000,
            kind: u32::from(built.kind),
            tags: built.tags,
            content: built.content,
            sig: "sig".into(),
        }
    }

    #[cfg(feature = "serde_json")]
    #[test]
    fn envelope_event_build_includes_order_and_snapshot_tags() {
        let listing_addr = format!("{KIND_LISTING}:pubkey:AAAAAAAAAAAAAAAAAAAAAg");
        let payload = TradeListingMessagePayload::OrderRequest(base_order());
        let built = super::trade_listing_envelope_event_build(
            "pubkey",
            TradeListingMessageType::OrderRequest,
            listing_addr.clone(),
            Some(String::from("order-1")),
            Some(&listing_snapshot()),
            None,
            None,
            &payload,
        )
        .unwrap();

        assert_eq!(built.kind, TradeListingMessageType::OrderRequest.kind());

        let envelope: TradeListingEnvelope<serde_json::Value> =
            serde_json::from_str(&built.content).unwrap();
        assert_eq!(envelope.listing_addr, listing_addr.clone());
        assert_eq!(envelope.order_id.as_deref(), Some("order-1"));
        assert_eq!(built.tags.len(), 4);
    }

    #[cfg(feature = "serde_json")]
    #[test]
    fn envelope_event_build_omits_order_tag_when_missing() {
        let listing_addr = format!("{KIND_LISTING}:pubkey:AAAAAAAAAAAAAAAAAAAAAg");
        let payload =
            TradeListingMessagePayload::ListingValidateRequest(TradeListingValidateRequest {
                listing_event: None,
            });
        let built = super::trade_listing_envelope_event_build(
            "pubkey",
            TradeListingMessageType::ListingValidateRequest,
            listing_addr.clone(),
            None,
            None,
            None,
            None,
            &payload,
        )
        .unwrap();

        assert_eq!(
            built.kind,
            TradeListingMessageType::ListingValidateRequest.kind()
        );

        let envelope: TradeListingEnvelope<serde_json::Value> =
            serde_json::from_str(&built.content).unwrap();
        assert_eq!(envelope.listing_addr, listing_addr);
        assert!(envelope.order_id.is_none());
        assert_eq!(built.tags.len(), 2);
    }

    #[cfg(feature = "serde_json")]
    #[test]
    fn envelope_event_build_requires_snapshot_for_order_request() {
        let listing_addr = format!("{KIND_LISTING}:pubkey:AAAAAAAAAAAAAAAAAAAAAg");
        let payload = TradeListingMessagePayload::OrderRequest(base_order());
        let err = super::trade_listing_envelope_event_build(
            "pubkey",
            TradeListingMessageType::OrderRequest,
            listing_addr,
            Some(String::from("order-1")),
            None,
            None,
            None,
            &payload,
        )
        .unwrap_err();
        assert_eq!(
            err,
            EventEncodeError::EmptyRequiredField("listing_event.id")
        );
    }

    #[cfg(feature = "serde_json")]
    #[test]
    fn envelope_event_build_requires_chain_tags_for_order_response() {
        let listing_addr = format!("{KIND_LISTING}:pubkey:AAAAAAAAAAAAAAAAAAAAAg");
        let payload = TradeListingMessagePayload::OrderResponse(TradeOrderResponse {
            accepted: true,
            reason: None,
        });
        let err = super::trade_listing_envelope_event_build(
            "buyer-pubkey",
            TradeListingMessageType::OrderResponse,
            listing_addr,
            Some(String::from("order-1")),
            None,
            None,
            None,
            &payload,
        )
        .unwrap_err();
        assert_eq!(err, EventEncodeError::EmptyRequiredField("root_event_id"));
    }

    #[cfg(feature = "serde_json")]
    #[test]
    fn envelope_from_event_parses_canonical_order_request() {
        let payload = TradeListingMessagePayload::OrderRequest(base_order());
        let event = base_event(
            "buyer-pubkey",
            "seller-pubkey",
            TradeListingMessageType::OrderRequest,
            &format!("{KIND_LISTING}:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg"),
            Some("order-1"),
            &payload,
        );

        let envelope =
            TradeListingEnvelope::<TradeListingMessagePayload>::from_event(&event).unwrap();
        assert_eq!(envelope.message_type, TradeListingMessageType::OrderRequest);
        assert_eq!(envelope.order_id.as_deref(), Some("order-1"));
        assert_eq!(envelope.payload, payload);
    }

    #[cfg(feature = "serde_json")]
    #[test]
    fn envelope_from_event_rejects_kind_mismatch() {
        let payload = TradeListingMessagePayload::OrderRequest(base_order());
        let mut event = base_event(
            "buyer-pubkey",
            "seller-pubkey",
            TradeListingMessageType::OrderRequest,
            &format!("{KIND_LISTING}:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg"),
            Some("order-1"),
            &payload,
        );
        event.kind = u32::from(TradeListingMessageType::OrderResponse.kind());

        let err = TradeListingEnvelope::<TradeListingMessagePayload>::from_event(&event)
            .expect_err("kind mismatch should fail");
        assert_eq!(
            err,
            TradeListingEnvelopeParseError::MessageTypeKindMismatch {
                event_kind: u32::from(TradeListingMessageType::OrderResponse.kind()),
                message_type: TradeListingMessageType::OrderRequest,
            }
        );
    }

    #[cfg(feature = "serde_json")]
    #[test]
    fn envelope_from_event_rejects_listing_addr_tag_mismatch() {
        let payload = TradeListingMessagePayload::OrderRequest(base_order());
        let mut event = base_event(
            "buyer-pubkey",
            "seller-pubkey",
            TradeListingMessageType::OrderRequest,
            &format!("{KIND_LISTING}:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg"),
            Some("order-1"),
            &payload,
        );
        event.tags[1][1] = format!("{KIND_LISTING}:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAw");

        let err = TradeListingEnvelope::<TradeListingMessagePayload>::from_event(&event)
            .expect_err("listing addr mismatch should fail");
        assert_eq!(err, TradeListingEnvelopeParseError::ListingAddrTagMismatch);
    }

    #[cfg(feature = "serde_json")]
    #[test]
    fn envelope_from_event_rejects_order_id_tag_mismatch() {
        let payload = TradeListingMessagePayload::OrderRequest(base_order());
        let mut event = base_event(
            "buyer-pubkey",
            "seller-pubkey",
            TradeListingMessageType::OrderRequest,
            &format!("{KIND_LISTING}:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg"),
            Some("order-1"),
            &payload,
        );
        event.tags[2][1] = "order-2".into();

        let err = TradeListingEnvelope::<TradeListingMessagePayload>::from_event(&event)
            .expect_err("order id mismatch should fail");
        assert_eq!(err, TradeListingEnvelopeParseError::OrderIdTagMismatch);
    }
}
