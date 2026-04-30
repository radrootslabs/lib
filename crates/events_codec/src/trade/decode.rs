#[cfg(all(not(feature = "std"), feature = "serde_json"))]
use alloc::{borrow::ToOwned, format, string::String, vec::Vec};

#[cfg(feature = "serde_json")]
use radroots_events::{
    RadrootsNostrEvent, RadrootsNostrEventPtr,
    kinds::{KIND_PROFILE, is_active_trade_public_kind, is_trade_kind},
    tags::{TAG_D, TAG_E_PREV, TAG_E_ROOT},
    trade::{
        RadrootsActiveTradeEnvelope, RadrootsActiveTradeEnvelopeError,
        RadrootsActiveTradeMessageType, RadrootsActiveTradePayloadError, RadrootsTradeBuyerReceipt,
        RadrootsTradeEnvelope, RadrootsTradeEnvelopeError, RadrootsTradeFulfillmentUpdated,
        RadrootsTradeMessageType, RadrootsTradeOrderCancelled, RadrootsTradeOrderDecisionEvent,
        RadrootsTradeOrderRequested,
    },
};
#[cfg(feature = "serde_json")]
use serde::de::DeserializeOwned;

#[cfg(feature = "serde_json")]
use crate::d_tag::is_d_tag_base64url;
#[cfg(feature = "serde_json")]
use crate::trade::tags::{
    TAG_LISTING_EVENT, parse_trade_counterparty_tag, parse_trade_listing_event_tag,
    parse_trade_prev_tag, parse_trade_root_tag,
};

#[cfg(feature = "serde_json")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsTradeEnvelopeParseError {
    InvalidKind(u32),
    InvalidJson,
    InvalidEnvelope(RadrootsTradeEnvelopeError),
    MessageTypeKindMismatch {
        event_kind: u32,
        message_type: RadrootsTradeMessageType,
    },
    MissingTag(&'static str),
    InvalidTag(&'static str),
    ListingAddrTagMismatch,
    OrderIdTagMismatch,
    InvalidListingAddr(RadrootsTradeListingAddressError),
}

#[cfg(feature = "serde_json")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsActiveTradeEnvelopeParseError {
    InvalidKind(u32),
    InvalidJson,
    InvalidEnvelope(RadrootsActiveTradeEnvelopeError),
    InvalidPayload(RadrootsActiveTradePayloadError),
    MessageTypeKindMismatch {
        event_kind: u32,
        message_type: RadrootsActiveTradeMessageType,
    },
    MissingTag(&'static str),
    InvalidTag(&'static str),
    ListingAddrTagMismatch,
    OrderIdTagMismatch,
    PayloadBindingMismatch(&'static str),
    AuthorMismatch,
    CounterpartyTagMismatch,
    InvalidListingAddr(RadrootsTradeListingAddressError),
}

#[cfg(feature = "serde_json")]
impl core::fmt::Display for RadrootsTradeEnvelopeParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidKind(kind) => write!(f, "invalid trade event kind: {kind}"),
            Self::InvalidJson => write!(f, "invalid trade envelope json"),
            Self::InvalidEnvelope(error) => write!(f, "{error}"),
            Self::MessageTypeKindMismatch {
                event_kind,
                message_type,
            } => write!(
                f,
                "trade envelope type {message_type:?} does not match event kind {event_kind}"
            ),
            Self::MissingTag(tag) => write!(f, "missing required trade tag: {tag}"),
            Self::InvalidTag(tag) => write!(f, "invalid trade tag: {tag}"),
            Self::ListingAddrTagMismatch => {
                write!(f, "trade listing address tag does not match envelope")
            }
            Self::OrderIdTagMismatch => write!(f, "trade order id tag does not match envelope"),
            Self::InvalidListingAddr(error) => write!(f, "{error}"),
        }
    }
}

#[cfg(feature = "serde_json")]
impl core::fmt::Display for RadrootsActiveTradeEnvelopeParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidKind(kind) => write!(f, "invalid active trade event kind: {kind}"),
            Self::InvalidJson => write!(f, "invalid active trade envelope json"),
            Self::InvalidEnvelope(error) => write!(f, "{error}"),
            Self::InvalidPayload(error) => write!(f, "{error}"),
            Self::MessageTypeKindMismatch {
                event_kind,
                message_type,
            } => write!(
                f,
                "active trade envelope type {message_type:?} does not match event kind {event_kind}"
            ),
            Self::MissingTag(tag) => write!(f, "missing required active trade tag: {tag}"),
            Self::InvalidTag(tag) => write!(f, "invalid active trade tag: {tag}"),
            Self::ListingAddrTagMismatch => {
                write!(
                    f,
                    "active trade listing address tag does not match envelope"
                )
            }
            Self::OrderIdTagMismatch => {
                write!(f, "active trade order id tag does not match envelope")
            }
            Self::PayloadBindingMismatch(field) => {
                write!(f, "active trade payload {field} does not match envelope")
            }
            Self::AuthorMismatch => write!(f, "active trade event author does not match payload"),
            Self::CounterpartyTagMismatch => {
                write!(f, "active trade counterparty tag does not match payload")
            }
            Self::InvalidListingAddr(error) => write!(f, "{error}"),
        }
    }
}

#[cfg(all(feature = "std", feature = "serde_json"))]
impl std::error::Error for RadrootsTradeEnvelopeParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidEnvelope(error) => Some(error),
            Self::InvalidListingAddr(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(all(feature = "std", feature = "serde_json"))]
impl std::error::Error for RadrootsActiveTradeEnvelopeParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidEnvelope(error) => Some(error),
            Self::InvalidPayload(error) => Some(error),
            Self::InvalidListingAddr(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(feature = "serde_json")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeEventContext {
    pub counterparty_pubkey: String,
    pub listing_event: Option<RadrootsNostrEventPtr>,
    pub root_event_id: Option<String>,
    pub prev_event_id: Option<String>,
}

#[cfg(feature = "serde_json")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeListingAddress {
    pub kind: u32,
    pub seller_pubkey: String,
    pub listing_id: String,
}

#[cfg(feature = "serde_json")]
impl RadrootsTradeListingAddress {
    pub fn parse(addr: &str) -> Result<Self, RadrootsTradeListingAddressError> {
        let (kind_raw, seller_and_listing) = addr
            .split_once(':')
            .ok_or(RadrootsTradeListingAddressError::InvalidFormat)?;
        let (seller_pubkey_raw, listing_id_raw) = seller_and_listing
            .split_once(':')
            .ok_or(RadrootsTradeListingAddressError::InvalidFormat)?;
        if listing_id_raw.contains(':') {
            return Err(RadrootsTradeListingAddressError::InvalidFormat);
        }
        let kind = kind_raw
            .parse::<u32>()
            .map_err(|_| RadrootsTradeListingAddressError::InvalidFormat)?;
        let seller_pubkey = seller_pubkey_raw.to_owned();
        let listing_id = listing_id_raw.to_owned();
        if kind == KIND_PROFILE
            || seller_pubkey.trim().is_empty()
            || listing_id.trim().is_empty()
            || !is_d_tag_base64url(&listing_id)
        {
            return Err(RadrootsTradeListingAddressError::InvalidFormat);
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

#[cfg(feature = "serde_json")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadrootsTradeListingAddressError {
    InvalidFormat,
}

#[cfg(feature = "serde_json")]
impl core::fmt::Display for RadrootsTradeListingAddressError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidFormat => write!(f, "invalid listing address format"),
        }
    }
}

#[cfg(all(feature = "std", feature = "serde_json"))]
impl std::error::Error for RadrootsTradeListingAddressError {}

#[cfg(feature = "serde_json")]
fn required_tag_value<'a>(
    tags: &'a [Vec<String>],
    key: &'static str,
) -> Result<&'a str, RadrootsTradeEnvelopeParseError> {
    let tag = tags
        .iter()
        .find(|tag| tag.first().map(|value| value.as_str()) == Some(key))
        .ok_or(RadrootsTradeEnvelopeParseError::MissingTag(key))?;
    let value = tag
        .get(1)
        .map(|value| value.as_str())
        .ok_or(RadrootsTradeEnvelopeParseError::InvalidTag(key))?;
    if value.trim().is_empty() {
        return Err(RadrootsTradeEnvelopeParseError::InvalidTag(key));
    }
    Ok(value)
}

#[cfg(feature = "serde_json")]
pub fn trade_envelope_from_event<T: DeserializeOwned>(
    event: &RadrootsNostrEvent,
) -> Result<RadrootsTradeEnvelope<T>, RadrootsTradeEnvelopeParseError> {
    if !is_trade_kind(event.kind) {
        return Err(RadrootsTradeEnvelopeParseError::InvalidKind(event.kind));
    }
    let envelope = serde_json::from_str::<RadrootsTradeEnvelope<T>>(&event.content)
        .map_err(|_| RadrootsTradeEnvelopeParseError::InvalidJson)?;
    envelope
        .validate()
        .map_err(RadrootsTradeEnvelopeParseError::InvalidEnvelope)?;
    if envelope.message_type.kind() != event.kind {
        return Err(RadrootsTradeEnvelopeParseError::MessageTypeKindMismatch {
            event_kind: event.kind,
            message_type: envelope.message_type,
        });
    }

    let listing_addr = required_tag_value(&event.tags, "a")?;
    if envelope.listing_addr != listing_addr {
        return Err(RadrootsTradeEnvelopeParseError::ListingAddrTagMismatch);
    }
    RadrootsTradeListingAddress::parse(&envelope.listing_addr)
        .map_err(RadrootsTradeEnvelopeParseError::InvalidListingAddr)?;

    if let Some(order_id) = envelope.order_id.as_deref() {
        let tag_order_id = required_tag_value(&event.tags, TAG_D)?;
        if tag_order_id != order_id {
            return Err(RadrootsTradeEnvelopeParseError::OrderIdTagMismatch);
        }
    }

    let message_type = envelope.message_type;
    trade_event_context_from_tags(message_type, &event.tags)?;

    Ok(envelope)
}

#[cfg(feature = "serde_json")]
pub fn active_trade_envelope_from_event<T: DeserializeOwned>(
    event: &RadrootsNostrEvent,
) -> Result<RadrootsActiveTradeEnvelope<T>, RadrootsActiveTradeEnvelopeParseError> {
    if !is_active_trade_public_kind(event.kind) {
        return Err(RadrootsActiveTradeEnvelopeParseError::InvalidKind(
            event.kind,
        ));
    }
    let envelope = serde_json::from_str::<RadrootsActiveTradeEnvelope<T>>(&event.content)
        .map_err(|_| RadrootsActiveTradeEnvelopeParseError::InvalidJson)?;
    envelope
        .validate()
        .map_err(RadrootsActiveTradeEnvelopeParseError::InvalidEnvelope)?;
    if envelope.message_type.kind() != event.kind {
        return Err(
            RadrootsActiveTradeEnvelopeParseError::MessageTypeKindMismatch {
                event_kind: event.kind,
                message_type: envelope.message_type,
            },
        );
    }

    let listing_addr = required_active_tag_value(&event.tags, "a")?;
    if envelope.listing_addr != listing_addr {
        return Err(RadrootsActiveTradeEnvelopeParseError::ListingAddrTagMismatch);
    }
    RadrootsTradeListingAddress::parse(&envelope.listing_addr)
        .map_err(RadrootsActiveTradeEnvelopeParseError::InvalidListingAddr)?;

    let tag_order_id = required_active_tag_value(&event.tags, TAG_D)?;
    if tag_order_id != envelope.order_id {
        return Err(RadrootsActiveTradeEnvelopeParseError::OrderIdTagMismatch);
    }

    active_trade_event_context_from_tags(envelope.message_type, &event.tags)?;
    Ok(envelope)
}

#[cfg(feature = "serde_json")]
pub fn active_trade_order_request_from_event(
    event: &RadrootsNostrEvent,
) -> Result<
    RadrootsActiveTradeEnvelope<RadrootsTradeOrderRequested>,
    RadrootsActiveTradeEnvelopeParseError,
> {
    let envelope = active_trade_envelope_from_event::<RadrootsTradeOrderRequested>(event)?;
    if envelope.message_type != RadrootsActiveTradeMessageType::TradeOrderRequested {
        return Err(
            RadrootsActiveTradeEnvelopeParseError::MessageTypeKindMismatch {
                event_kind: event.kind,
                message_type: envelope.message_type,
            },
        );
    }
    envelope
        .payload
        .validate()
        .map_err(RadrootsActiveTradeEnvelopeParseError::InvalidPayload)?;
    validate_active_order_binding(
        event,
        &envelope,
        &envelope.payload.order_id,
        &envelope.payload.listing_addr,
        &envelope.payload.buyer_pubkey,
        &envelope.payload.seller_pubkey,
    )?;
    Ok(envelope)
}

#[cfg(feature = "serde_json")]
pub fn active_trade_order_decision_from_event(
    event: &RadrootsNostrEvent,
) -> Result<
    RadrootsActiveTradeEnvelope<RadrootsTradeOrderDecisionEvent>,
    RadrootsActiveTradeEnvelopeParseError,
> {
    let envelope = active_trade_envelope_from_event::<RadrootsTradeOrderDecisionEvent>(event)?;
    if envelope.message_type != RadrootsActiveTradeMessageType::TradeOrderDecision {
        return Err(
            RadrootsActiveTradeEnvelopeParseError::MessageTypeKindMismatch {
                event_kind: event.kind,
                message_type: envelope.message_type,
            },
        );
    }
    envelope
        .payload
        .validate()
        .map_err(RadrootsActiveTradeEnvelopeParseError::InvalidPayload)?;
    validate_active_order_binding(
        event,
        &envelope,
        &envelope.payload.order_id,
        &envelope.payload.listing_addr,
        &envelope.payload.seller_pubkey,
        &envelope.payload.buyer_pubkey,
    )?;
    Ok(envelope)
}

#[cfg(feature = "serde_json")]
pub fn active_trade_fulfillment_update_from_event(
    event: &RadrootsNostrEvent,
) -> Result<
    RadrootsActiveTradeEnvelope<RadrootsTradeFulfillmentUpdated>,
    RadrootsActiveTradeEnvelopeParseError,
> {
    let envelope = active_trade_envelope_from_event::<RadrootsTradeFulfillmentUpdated>(event)?;
    if envelope.message_type != RadrootsActiveTradeMessageType::TradeFulfillmentUpdated {
        return Err(
            RadrootsActiveTradeEnvelopeParseError::MessageTypeKindMismatch {
                event_kind: event.kind,
                message_type: envelope.message_type,
            },
        );
    }
    envelope
        .payload
        .validate()
        .map_err(RadrootsActiveTradeEnvelopeParseError::InvalidPayload)?;
    validate_active_order_binding(
        event,
        &envelope,
        &envelope.payload.order_id,
        &envelope.payload.listing_addr,
        &envelope.payload.seller_pubkey,
        &envelope.payload.buyer_pubkey,
    )?;
    Ok(envelope)
}

#[cfg(feature = "serde_json")]
pub fn active_trade_order_cancel_from_event(
    event: &RadrootsNostrEvent,
) -> Result<
    RadrootsActiveTradeEnvelope<RadrootsTradeOrderCancelled>,
    RadrootsActiveTradeEnvelopeParseError,
> {
    let envelope = active_trade_envelope_from_event::<RadrootsTradeOrderCancelled>(event)?;
    if envelope.message_type != RadrootsActiveTradeMessageType::TradeOrderCancelled {
        return Err(
            RadrootsActiveTradeEnvelopeParseError::MessageTypeKindMismatch {
                event_kind: event.kind,
                message_type: envelope.message_type,
            },
        );
    }
    envelope
        .payload
        .validate()
        .map_err(RadrootsActiveTradeEnvelopeParseError::InvalidPayload)?;
    validate_active_order_binding(
        event,
        &envelope,
        &envelope.payload.order_id,
        &envelope.payload.listing_addr,
        &envelope.payload.buyer_pubkey,
        &envelope.payload.seller_pubkey,
    )?;
    Ok(envelope)
}

#[cfg(feature = "serde_json")]
pub fn active_trade_buyer_receipt_from_event(
    event: &RadrootsNostrEvent,
) -> Result<
    RadrootsActiveTradeEnvelope<RadrootsTradeBuyerReceipt>,
    RadrootsActiveTradeEnvelopeParseError,
> {
    let envelope = active_trade_envelope_from_event::<RadrootsTradeBuyerReceipt>(event)?;
    if envelope.message_type != RadrootsActiveTradeMessageType::TradeBuyerReceipt {
        return Err(
            RadrootsActiveTradeEnvelopeParseError::MessageTypeKindMismatch {
                event_kind: event.kind,
                message_type: envelope.message_type,
            },
        );
    }
    envelope
        .payload
        .validate()
        .map_err(RadrootsActiveTradeEnvelopeParseError::InvalidPayload)?;
    validate_active_order_binding(
        event,
        &envelope,
        &envelope.payload.order_id,
        &envelope.payload.listing_addr,
        &envelope.payload.buyer_pubkey,
        &envelope.payload.seller_pubkey,
    )?;
    Ok(envelope)
}

#[cfg(feature = "serde_json")]
pub fn trade_event_context_from_tags(
    message_type: RadrootsTradeMessageType,
    tags: &[Vec<String>],
) -> Result<RadrootsTradeEventContext, RadrootsTradeEnvelopeParseError> {
    let counterparty_pubkey =
        parse_trade_counterparty_tag(tags).map_err(map_tag_parse_error_for_trade_envelope)?;
    let listing_event =
        parse_trade_listing_event_tag(tags).map_err(map_tag_parse_error_for_trade_envelope)?;
    let root_event_id =
        parse_trade_root_tag(tags).map_err(map_tag_parse_error_for_trade_envelope)?;
    let prev_event_id =
        parse_trade_prev_tag(tags).map_err(map_tag_parse_error_for_trade_envelope)?;

    if message_type.requires_listing_snapshot() && listing_event.is_none() {
        return Err(RadrootsTradeEnvelopeParseError::MissingTag(
            TAG_LISTING_EVENT,
        ));
    }
    if message_type.requires_trade_chain() {
        if root_event_id.is_none() {
            return Err(RadrootsTradeEnvelopeParseError::MissingTag(TAG_E_ROOT));
        }
        if prev_event_id.is_none() {
            return Err(RadrootsTradeEnvelopeParseError::MissingTag(TAG_E_PREV));
        }
    }

    Ok(RadrootsTradeEventContext {
        counterparty_pubkey,
        listing_event,
        root_event_id,
        prev_event_id,
    })
}

#[cfg(feature = "serde_json")]
pub fn active_trade_event_context_from_tags(
    message_type: RadrootsActiveTradeMessageType,
    tags: &[Vec<String>],
) -> Result<RadrootsTradeEventContext, RadrootsActiveTradeEnvelopeParseError> {
    let counterparty_pubkey = parse_trade_counterparty_tag(tags)
        .map_err(map_tag_parse_error_for_active_trade_envelope)?;
    let listing_event = parse_trade_listing_event_tag(tags)
        .map_err(map_tag_parse_error_for_active_trade_envelope)?;
    let root_event_id =
        parse_trade_root_tag(tags).map_err(map_tag_parse_error_for_active_trade_envelope)?;
    let prev_event_id =
        parse_trade_prev_tag(tags).map_err(map_tag_parse_error_for_active_trade_envelope)?;

    if message_type.requires_listing_snapshot() && listing_event.is_none() {
        return Err(RadrootsActiveTradeEnvelopeParseError::MissingTag(
            TAG_LISTING_EVENT,
        ));
    }
    if message_type.requires_trade_chain() {
        if root_event_id.is_none() {
            return Err(RadrootsActiveTradeEnvelopeParseError::MissingTag(
                TAG_E_ROOT,
            ));
        }
        if prev_event_id.is_none() {
            return Err(RadrootsActiveTradeEnvelopeParseError::MissingTag(
                TAG_E_PREV,
            ));
        }
    }

    Ok(RadrootsTradeEventContext {
        counterparty_pubkey,
        listing_event,
        root_event_id,
        prev_event_id,
    })
}

#[cfg(feature = "serde_json")]
fn map_tag_parse_error_for_trade_envelope(
    error: crate::error::EventParseError,
) -> RadrootsTradeEnvelopeParseError {
    match error {
        crate::error::EventParseError::MissingTag(tag) => {
            RadrootsTradeEnvelopeParseError::MissingTag(tag)
        }
        crate::error::EventParseError::InvalidTag(tag) => {
            RadrootsTradeEnvelopeParseError::InvalidTag(tag)
        }
        crate::error::EventParseError::InvalidKind { expected: _, got } => {
            RadrootsTradeEnvelopeParseError::InvalidKind(got)
        }
        crate::error::EventParseError::InvalidNumber(tag, _)
        | crate::error::EventParseError::InvalidJson(tag) => {
            RadrootsTradeEnvelopeParseError::InvalidTag(tag)
        }
    }
}

#[cfg(feature = "serde_json")]
fn required_active_tag_value<'a>(
    tags: &'a [Vec<String>],
    key: &'static str,
) -> Result<&'a str, RadrootsActiveTradeEnvelopeParseError> {
    let tag = tags
        .iter()
        .find(|tag| tag.first().map(|value| value.as_str()) == Some(key))
        .ok_or(RadrootsActiveTradeEnvelopeParseError::MissingTag(key))?;
    let value = tag
        .get(1)
        .map(|value| value.as_str())
        .ok_or(RadrootsActiveTradeEnvelopeParseError::InvalidTag(key))?;
    if value.trim().is_empty() {
        return Err(RadrootsActiveTradeEnvelopeParseError::InvalidTag(key));
    }
    Ok(value)
}

#[cfg(feature = "serde_json")]
fn map_tag_parse_error_for_active_trade_envelope(
    error: crate::error::EventParseError,
) -> RadrootsActiveTradeEnvelopeParseError {
    match error {
        crate::error::EventParseError::MissingTag(tag) => {
            RadrootsActiveTradeEnvelopeParseError::MissingTag(tag)
        }
        crate::error::EventParseError::InvalidTag(tag) => {
            RadrootsActiveTradeEnvelopeParseError::InvalidTag(tag)
        }
        crate::error::EventParseError::InvalidKind { expected: _, got } => {
            RadrootsActiveTradeEnvelopeParseError::InvalidKind(got)
        }
        crate::error::EventParseError::InvalidNumber(tag, _)
        | crate::error::EventParseError::InvalidJson(tag) => {
            RadrootsActiveTradeEnvelopeParseError::InvalidTag(tag)
        }
    }
}

#[cfg(feature = "serde_json")]
fn validate_active_order_binding<T>(
    event: &RadrootsNostrEvent,
    envelope: &RadrootsActiveTradeEnvelope<T>,
    payload_order_id: &str,
    payload_listing_addr: &str,
    expected_author: &str,
    expected_counterparty: &str,
) -> Result<(), RadrootsActiveTradeEnvelopeParseError> {
    if envelope.order_id != payload_order_id {
        return Err(RadrootsActiveTradeEnvelopeParseError::PayloadBindingMismatch("order_id"));
    }
    if envelope.listing_addr != payload_listing_addr {
        return Err(RadrootsActiveTradeEnvelopeParseError::PayloadBindingMismatch("listing_addr"));
    }
    if event.author != expected_author {
        return Err(RadrootsActiveTradeEnvelopeParseError::AuthorMismatch);
    }
    let counterparty = parse_trade_counterparty_tag(&event.tags)
        .map_err(map_tag_parse_error_for_active_trade_envelope)?;
    if counterparty != expected_counterparty {
        return Err(RadrootsActiveTradeEnvelopeParseError::CounterpartyTagMismatch);
    }
    Ok(())
}

#[cfg(all(test, feature = "serde_json"))]
mod tests {
    use super::{
        RadrootsActiveTradeEnvelopeParseError, RadrootsTradeEnvelopeParseError,
        RadrootsTradeListingAddress, active_trade_buyer_receipt_from_event,
        active_trade_envelope_from_event, active_trade_fulfillment_update_from_event,
        active_trade_order_cancel_from_event, active_trade_order_decision_from_event,
        active_trade_order_request_from_event, trade_envelope_from_event,
        trade_event_context_from_tags,
    };
    use crate::trade::encode::{
        active_trade_buyer_receipt_event_build, active_trade_fulfillment_update_event_build,
        active_trade_order_cancel_event_build, active_trade_order_decision_event_build,
        active_trade_order_request_event_build, trade_envelope_event_build,
    };
    use crate::trade::tags::TAG_LISTING_EVENT;
    use radroots_events::{
        RadrootsNostrEvent, RadrootsNostrEventPtr,
        kinds::{
            KIND_TRADE_CANCEL, KIND_TRADE_FULFILLMENT_UPDATE, KIND_TRADE_ORDER_DECISION,
            KIND_TRADE_ORDER_REQUEST, KIND_TRADE_RECEIPT,
        },
        tags::{TAG_D, TAG_E_PREV, TAG_E_ROOT},
        trade::{
            RadrootsActiveTradeEnvelope, RadrootsActiveTradeFulfillmentState,
            RadrootsActiveTradeMessageType, RadrootsTradeBuyerReceipt, RadrootsTradeEnvelope,
            RadrootsTradeFulfillmentUpdated, RadrootsTradeInventoryCommitment,
            RadrootsTradeMessagePayload, RadrootsTradeMessageType, RadrootsTradeOrder,
            RadrootsTradeOrderCancelled, RadrootsTradeOrderDecision,
            RadrootsTradeOrderDecisionEvent, RadrootsTradeOrderItem, RadrootsTradeOrderRequested,
        },
    };

    fn base_order() -> RadrootsTradeOrder {
        RadrootsTradeOrder {
            order_id: "order-1".into(),
            listing_addr: "30402:seller:AAAAAAAAAAAAAAAAAAAAAg".into(),
            buyer_pubkey: "buyer".into(),
            seller_pubkey: "seller".into(),
            items: vec![RadrootsTradeOrderItem {
                bin_id: "lb".into(),
                bin_count: 3,
            }],
            discounts: None,
        }
    }

    fn active_order_request() -> RadrootsTradeOrderRequested {
        RadrootsTradeOrderRequested {
            order_id: "order-1".into(),
            listing_addr: "30402:seller:AAAAAAAAAAAAAAAAAAAAAg".into(),
            buyer_pubkey: "buyer".into(),
            seller_pubkey: "seller".into(),
            items: vec![RadrootsTradeOrderItem {
                bin_id: "lb".into(),
                bin_count: 3,
            }],
        }
    }

    fn active_order_decision() -> RadrootsTradeOrderDecisionEvent {
        RadrootsTradeOrderDecisionEvent {
            order_id: "order-1".into(),
            listing_addr: "30402:seller:AAAAAAAAAAAAAAAAAAAAAg".into(),
            buyer_pubkey: "buyer".into(),
            seller_pubkey: "seller".into(),
            decision: RadrootsTradeOrderDecision::Accepted {
                inventory_commitments: vec![RadrootsTradeInventoryCommitment {
                    bin_id: "lb".into(),
                    bin_count: 3,
                }],
            },
        }
    }

    fn active_fulfillment_update() -> RadrootsTradeFulfillmentUpdated {
        RadrootsTradeFulfillmentUpdated {
            order_id: "order-1".into(),
            listing_addr: "30402:seller:AAAAAAAAAAAAAAAAAAAAAg".into(),
            buyer_pubkey: "buyer".into(),
            seller_pubkey: "seller".into(),
            status: RadrootsActiveTradeFulfillmentState::ReadyForPickup,
        }
    }

    fn active_order_cancelled() -> RadrootsTradeOrderCancelled {
        RadrootsTradeOrderCancelled {
            order_id: "order-1".into(),
            listing_addr: "30402:seller:AAAAAAAAAAAAAAAAAAAAAg".into(),
            buyer_pubkey: "buyer".into(),
            seller_pubkey: "seller".into(),
            reason: "changed plans".into(),
        }
    }

    fn active_buyer_receipt(received: bool) -> RadrootsTradeBuyerReceipt {
        RadrootsTradeBuyerReceipt {
            order_id: "order-1".into(),
            listing_addr: "30402:seller:AAAAAAAAAAAAAAAAAAAAAg".into(),
            buyer_pubkey: "buyer".into(),
            seller_pubkey: "seller".into(),
            received,
            issue: (!received).then(|| "damaged items".into()),
            received_at: 1_777_665_600,
        }
    }

    fn listing_event_ptr() -> RadrootsNostrEventPtr {
        RadrootsNostrEventPtr {
            id: "listing-snapshot".into(),
            relays: Some("wss://relay.example.com".into()),
        }
    }

    #[test]
    fn listing_address_roundtrips() {
        let addr = RadrootsTradeListingAddress::parse("30402:seller:AAAAAAAAAAAAAAAAAAAAAg")
            .expect("parse listing address");
        assert_eq!(addr.as_str(), "30402:seller:AAAAAAAAAAAAAAAAAAAAAg");
    }

    #[test]
    fn parse_order_request_roundtrip() {
        let payload = RadrootsTradeMessagePayload::OrderRequest(base_order());
        let built = trade_envelope_event_build(
            "seller",
            RadrootsTradeMessageType::OrderRequest,
            "30402:seller:AAAAAAAAAAAAAAAAAAAAAg",
            Some("order-1".into()),
            Some(&RadrootsNostrEventPtr {
                id: "listing-snapshot".into(),
                relays: None,
            }),
            None,
            None,
            &payload,
        )
        .expect("build trade envelope");
        let event = RadrootsNostrEvent {
            id: "id".into(),
            author: "buyer".into(),
            created_at: 1,
            kind: built.kind,
            tags: built.tags,
            content: built.content,
            sig: "sig".into(),
        };
        let envelope: RadrootsTradeEnvelope<RadrootsTradeMessagePayload> =
            trade_envelope_from_event(&event).expect("parse trade envelope");
        assert_eq!(
            envelope.message_type,
            RadrootsTradeMessageType::OrderRequest
        );
        assert_eq!(envelope.order_id.as_deref(), Some("order-1"));
    }

    #[test]
    fn active_order_request_builder_emits_canonical_shape() {
        let payload = active_order_request();
        let built = active_trade_order_request_event_build(&listing_event_ptr(), &payload).unwrap();
        let envelope: RadrootsActiveTradeEnvelope<RadrootsTradeOrderRequested> =
            serde_json::from_str(&built.content).unwrap();

        assert_eq!(built.kind, KIND_TRADE_ORDER_REQUEST);
        assert_eq!(
            envelope.message_type,
            RadrootsActiveTradeMessageType::TradeOrderRequested
        );
        assert_eq!(envelope.order_id, "order-1");
        assert_eq!(built.tags[0], vec!["p".to_string(), "seller".to_string()]);
        assert_eq!(
            built.tags[1],
            vec![
                "a".to_string(),
                "30402:seller:AAAAAAAAAAAAAAAAAAAAAg".to_string()
            ]
        );
        assert_eq!(
            built.tags[2],
            vec![TAG_D.to_string(), "order-1".to_string()]
        );
        assert!(
            built
                .tags
                .iter()
                .any(|tag| tag.first().map(String::as_str) == Some(TAG_LISTING_EVENT))
        );
        assert!(
            !built
                .tags
                .iter()
                .any(|tag| tag.first().map(String::as_str) == Some(TAG_E_ROOT))
        );
    }

    #[test]
    fn active_order_decision_builder_emits_canonical_chain_shape() {
        let payload = active_order_decision();
        let built =
            active_trade_order_decision_event_build("root-event", "prev-event", &payload).unwrap();
        let envelope: RadrootsActiveTradeEnvelope<RadrootsTradeOrderDecisionEvent> =
            serde_json::from_str(&built.content).unwrap();

        assert_eq!(built.kind, KIND_TRADE_ORDER_DECISION);
        assert_eq!(
            envelope.message_type,
            RadrootsActiveTradeMessageType::TradeOrderDecision
        );
        assert_eq!(built.tags[0], vec!["p".to_string(), "buyer".to_string()]);
        assert_eq!(
            built.tags[2],
            vec![TAG_D.to_string(), "order-1".to_string()]
        );
        assert!(
            built
                .tags
                .iter()
                .any(|tag| tag == &vec![TAG_E_ROOT.to_string(), "root-event".to_string()])
        );
        assert!(
            built
                .tags
                .iter()
                .any(|tag| tag == &vec![TAG_E_PREV.to_string(), "prev-event".to_string()])
        );
    }

    #[test]
    fn active_fulfillment_update_builder_emits_canonical_chain_shape() {
        let payload = active_fulfillment_update();
        let built =
            active_trade_fulfillment_update_event_build("root-event", "prev-event", &payload)
                .unwrap();
        let envelope: RadrootsActiveTradeEnvelope<RadrootsTradeFulfillmentUpdated> =
            serde_json::from_str(&built.content).unwrap();

        assert_eq!(built.kind, KIND_TRADE_FULFILLMENT_UPDATE);
        assert_eq!(
            envelope.message_type,
            RadrootsActiveTradeMessageType::TradeFulfillmentUpdated
        );
        assert_eq!(envelope.payload.status, payload.status);
        assert_eq!(built.tags[0], vec!["p".to_string(), "buyer".to_string()]);
        assert_eq!(
            built.tags[2],
            vec![TAG_D.to_string(), "order-1".to_string()]
        );
        assert!(
            built
                .tags
                .iter()
                .any(|tag| tag == &vec![TAG_E_ROOT.to_string(), "root-event".to_string()])
        );
        assert!(
            built
                .tags
                .iter()
                .any(|tag| tag == &vec![TAG_E_PREV.to_string(), "prev-event".to_string()])
        );
    }

    #[test]
    fn active_order_cancel_builder_emits_canonical_buyer_chain_shape() {
        let payload = active_order_cancelled();
        let built =
            active_trade_order_cancel_event_build("root-event", "prev-event", &payload).unwrap();
        let envelope: RadrootsActiveTradeEnvelope<RadrootsTradeOrderCancelled> =
            serde_json::from_str(&built.content).unwrap();

        assert_eq!(built.kind, KIND_TRADE_CANCEL);
        assert_eq!(
            envelope.message_type,
            RadrootsActiveTradeMessageType::TradeOrderCancelled
        );
        assert_eq!(envelope.payload.reason, payload.reason);
        assert_eq!(built.tags[0], vec!["p".to_string(), "seller".to_string()]);
        assert_eq!(
            built.tags[2],
            vec![TAG_D.to_string(), "order-1".to_string()]
        );
        assert!(
            built
                .tags
                .iter()
                .any(|tag| tag == &vec![TAG_E_ROOT.to_string(), "root-event".to_string()])
        );
        assert!(
            built
                .tags
                .iter()
                .any(|tag| tag == &vec![TAG_E_PREV.to_string(), "prev-event".to_string()])
        );
    }

    #[test]
    fn active_buyer_receipt_builder_emits_canonical_buyer_chain_shape() {
        let payload = active_buyer_receipt(false);
        let built =
            active_trade_buyer_receipt_event_build("root-event", "prev-event", &payload).unwrap();
        let envelope: RadrootsActiveTradeEnvelope<RadrootsTradeBuyerReceipt> =
            serde_json::from_str(&built.content).unwrap();

        assert_eq!(built.kind, KIND_TRADE_RECEIPT);
        assert_eq!(
            envelope.message_type,
            RadrootsActiveTradeMessageType::TradeBuyerReceipt
        );
        assert_eq!(envelope.payload.received, false);
        assert_eq!(envelope.payload.issue.as_deref(), Some("damaged items"));
        assert_eq!(built.tags[0], vec!["p".to_string(), "seller".to_string()]);
        assert_eq!(
            built.tags[2],
            vec![TAG_D.to_string(), "order-1".to_string()]
        );
        assert!(
            built
                .tags
                .iter()
                .any(|tag| tag == &vec![TAG_E_ROOT.to_string(), "root-event".to_string()])
        );
        assert!(
            built
                .tags
                .iter()
                .any(|tag| tag == &vec![TAG_E_PREV.to_string(), "prev-event".to_string()])
        );
    }

    #[test]
    fn active_order_request_parse_roundtrips_and_validates_tags() {
        let payload = active_order_request();
        let built = active_trade_order_request_event_build(&listing_event_ptr(), &payload).unwrap();
        let event = RadrootsNostrEvent {
            id: "event-id".into(),
            author: "buyer".into(),
            created_at: 1,
            kind: built.kind,
            tags: built.tags,
            content: built.content,
            sig: "sig".into(),
        };
        let envelope = active_trade_order_request_from_event(&event).unwrap();

        assert_eq!(envelope.payload, payload);
        assert_eq!(
            envelope.message_type,
            RadrootsActiveTradeMessageType::TradeOrderRequested
        );
    }

    #[test]
    fn active_order_decision_parse_roundtrips_and_validates_chain_tags() {
        let payload = active_order_decision();
        let built =
            active_trade_order_decision_event_build("root-event", "prev-event", &payload).unwrap();
        let event = RadrootsNostrEvent {
            id: "event-id".into(),
            author: "seller".into(),
            created_at: 1,
            kind: built.kind,
            tags: built.tags,
            content: built.content,
            sig: "sig".into(),
        };
        let envelope = active_trade_order_decision_from_event(&event).unwrap();

        assert_eq!(envelope.payload, payload);
        assert_eq!(
            envelope.message_type,
            RadrootsActiveTradeMessageType::TradeOrderDecision
        );
    }

    #[test]
    fn active_fulfillment_update_parse_roundtrips_and_validates_chain_tags() {
        let payload = active_fulfillment_update();
        let built =
            active_trade_fulfillment_update_event_build("root-event", "prev-event", &payload)
                .unwrap();
        let event = RadrootsNostrEvent {
            id: "event-id".into(),
            author: "seller".into(),
            created_at: 1,
            kind: built.kind,
            tags: built.tags,
            content: built.content,
            sig: "sig".into(),
        };
        let envelope = active_trade_fulfillment_update_from_event(&event).unwrap();

        assert_eq!(envelope.payload, payload);
        assert_eq!(
            envelope.message_type,
            RadrootsActiveTradeMessageType::TradeFulfillmentUpdated
        );
    }

    #[test]
    fn active_order_cancel_parse_roundtrips_and_validates_buyer_actor() {
        let payload = active_order_cancelled();
        let built =
            active_trade_order_cancel_event_build("root-event", "prev-event", &payload).unwrap();
        let event = RadrootsNostrEvent {
            id: "event-id".into(),
            author: "buyer".into(),
            created_at: 1,
            kind: built.kind,
            tags: built.tags,
            content: built.content,
            sig: "sig".into(),
        };
        let envelope = active_trade_order_cancel_from_event(&event).unwrap();

        assert_eq!(envelope.payload, payload);
        assert_eq!(
            envelope.message_type,
            RadrootsActiveTradeMessageType::TradeOrderCancelled
        );
    }

    #[test]
    fn active_buyer_receipt_parse_roundtrips_and_validates_buyer_actor() {
        let payload = active_buyer_receipt(true);
        let built =
            active_trade_buyer_receipt_event_build("root-event", "prev-event", &payload).unwrap();
        let event = RadrootsNostrEvent {
            id: "event-id".into(),
            author: "buyer".into(),
            created_at: 1,
            kind: built.kind,
            tags: built.tags,
            content: built.content,
            sig: "sig".into(),
        };
        let envelope = active_trade_buyer_receipt_from_event(&event).unwrap();

        assert_eq!(envelope.payload, payload);
        assert_eq!(
            envelope.message_type,
            RadrootsActiveTradeMessageType::TradeBuyerReceipt
        );
    }

    #[test]
    fn active_parse_rejects_forbidden_kind() {
        let event = RadrootsNostrEvent {
            id: "event-id".into(),
            author: "seller".into(),
            created_at: 1,
            kind: 3431,
            tags: Vec::new(),
            content: "{}".into(),
            sig: "sig".into(),
        };
        let err = active_trade_envelope_from_event::<serde_json::Value>(&event).unwrap_err();
        assert_eq!(
            err,
            RadrootsActiveTradeEnvelopeParseError::InvalidKind(3431)
        );
    }

    #[test]
    fn active_parse_rejects_missing_required_refs() {
        let payload = active_order_decision();
        let built =
            active_trade_order_decision_event_build("root-event", "prev-event", &payload).unwrap();
        let mut event = RadrootsNostrEvent {
            id: "event-id".into(),
            author: "seller".into(),
            created_at: 1,
            kind: built.kind,
            tags: built.tags,
            content: built.content,
            sig: "sig".into(),
        };
        event
            .tags
            .retain(|tag| tag.first().map(String::as_str) != Some(TAG_E_PREV));

        let err = active_trade_order_decision_from_event(&event).unwrap_err();
        assert_eq!(
            err,
            RadrootsActiveTradeEnvelopeParseError::MissingTag(TAG_E_PREV)
        );
    }

    #[test]
    fn active_parse_rejects_author_and_counterparty_mismatch() {
        let payload = active_order_request();
        let built = active_trade_order_request_event_build(&listing_event_ptr(), &payload).unwrap();
        let mut event = RadrootsNostrEvent {
            id: "event-id".into(),
            author: "seller".into(),
            created_at: 1,
            kind: built.kind,
            tags: built.tags.clone(),
            content: built.content.clone(),
            sig: "sig".into(),
        };
        let err = active_trade_order_request_from_event(&event).unwrap_err();
        assert_eq!(err, RadrootsActiveTradeEnvelopeParseError::AuthorMismatch);

        event.author = "buyer".into();
        event.tags[0] = vec!["p".into(), "other-seller".into()];
        let err = active_trade_order_request_from_event(&event).unwrap_err();
        assert_eq!(
            err,
            RadrootsActiveTradeEnvelopeParseError::CounterpartyTagMismatch
        );
    }

    #[test]
    fn active_buyer_lifecycle_parse_rejects_wrong_actor_or_counterparty() {
        let cancellation = active_order_cancelled();
        let cancellation_parts =
            active_trade_order_cancel_event_build("root-event", "prev-event", &cancellation)
                .unwrap();
        let cancellation_event = RadrootsNostrEvent {
            id: "event-id".into(),
            author: "seller".into(),
            created_at: 1,
            kind: cancellation_parts.kind,
            tags: cancellation_parts.tags,
            content: cancellation_parts.content,
            sig: "sig".into(),
        };
        let err = active_trade_order_cancel_from_event(&cancellation_event).unwrap_err();
        assert_eq!(err, RadrootsActiveTradeEnvelopeParseError::AuthorMismatch);

        let receipt = active_buyer_receipt(true);
        let receipt_parts =
            active_trade_buyer_receipt_event_build("root-event", "prev-event", &receipt).unwrap();
        let mut receipt_event = RadrootsNostrEvent {
            id: "event-id".into(),
            author: "buyer".into(),
            created_at: 1,
            kind: receipt_parts.kind,
            tags: receipt_parts.tags,
            content: receipt_parts.content,
            sig: "sig".into(),
        };
        receipt_event.tags[0] = vec!["p".into(), "other-seller".into()];
        let err = active_trade_buyer_receipt_from_event(&receipt_event).unwrap_err();
        assert_eq!(
            err,
            RadrootsActiveTradeEnvelopeParseError::CounterpartyTagMismatch
        );
    }

    #[test]
    fn parse_rejects_listing_addr_mismatch() {
        let payload = RadrootsTradeMessagePayload::OrderRequest(base_order());
        let built = trade_envelope_event_build(
            "seller",
            RadrootsTradeMessageType::OrderRequest,
            "30402:seller:AAAAAAAAAAAAAAAAAAAAAg",
            Some("order-1".into()),
            Some(&RadrootsNostrEventPtr {
                id: "listing-snapshot".into(),
                relays: None,
            }),
            None,
            None,
            &payload,
        )
        .expect("build trade envelope");
        let mut envelope: RadrootsTradeEnvelope<serde_json::Value> =
            serde_json::from_str(&built.content).expect("decode json");
        envelope.listing_addr = "30402:seller:BBBBBBBBBBBBBBBBBBBBBg".into();
        let event = RadrootsNostrEvent {
            id: "id".into(),
            author: "buyer".into(),
            created_at: 1,
            kind: built.kind,
            tags: built.tags,
            content: serde_json::to_string(&envelope).expect("encode json"),
            sig: "sig".into(),
        };
        let err = trade_envelope_from_event::<serde_json::Value>(&event).unwrap_err();
        assert_eq!(err, RadrootsTradeEnvelopeParseError::ListingAddrTagMismatch);
    }

    #[test]
    fn parse_rejects_missing_public_snapshot_tag() {
        let payload = RadrootsTradeMessagePayload::OrderRequest(base_order());
        let built = trade_envelope_event_build(
            "seller",
            RadrootsTradeMessageType::OrderRequest,
            "30402:seller:AAAAAAAAAAAAAAAAAAAAAg",
            Some("order-1".into()),
            Some(&RadrootsNostrEventPtr {
                id: "listing-snapshot".into(),
                relays: None,
            }),
            None,
            None,
            &payload,
        )
        .expect("build trade envelope");
        let mut event = RadrootsNostrEvent {
            id: "id".into(),
            author: "buyer".into(),
            created_at: 1,
            kind: built.kind,
            tags: built.tags,
            content: built.content,
            sig: "sig".into(),
        };
        event
            .tags
            .retain(|tag| tag.first().map(|value| value.as_str()) != Some(TAG_LISTING_EVENT));
        let err = trade_envelope_from_event::<RadrootsTradeMessagePayload>(&event).unwrap_err();
        assert_eq!(
            err,
            RadrootsTradeEnvelopeParseError::MissingTag(TAG_LISTING_EVENT)
        );
    }

    #[test]
    fn parse_rejects_missing_public_chain_tags_after_order_request() {
        let payload = RadrootsTradeMessagePayload::OrderResponse(
            radroots_events::trade::RadrootsTradeOrderResponse {
                accepted: true,
                reason: None,
            },
        );
        let built = trade_envelope_event_build(
            "buyer",
            RadrootsTradeMessageType::OrderResponse,
            "30402:seller:AAAAAAAAAAAAAAAAAAAAAg",
            Some("order-1".into()),
            None,
            Some("root"),
            Some("prev"),
            &payload,
        )
        .expect("build trade envelope");
        let mut event = RadrootsNostrEvent {
            id: "id".into(),
            author: "seller".into(),
            created_at: 1,
            kind: built.kind,
            tags: built.tags,
            content: built.content,
            sig: "sig".into(),
        };
        event
            .tags
            .retain(|tag| tag.first().map(|value| value.as_str()) != Some(TAG_E_PREV));
        let err = trade_envelope_from_event::<RadrootsTradeMessagePayload>(&event).unwrap_err();
        assert_eq!(err, RadrootsTradeEnvelopeParseError::MissingTag(TAG_E_PREV));
    }

    #[test]
    fn parse_trade_event_context_extracts_public_refs() {
        let context = trade_event_context_from_tags(
            RadrootsTradeMessageType::OrderResponse,
            &[
                vec!["p".into(), "buyer".into()],
                vec!["a".into(), "30402:seller:AAAAAAAAAAAAAAAAAAAAAg".into()],
                vec![TAG_D.into(), "order-1".into()],
                vec![TAG_E_ROOT.into(), "root-id".into()],
                vec![TAG_E_PREV.into(), "prev-id".into()],
            ],
        )
        .expect("event context");
        assert_eq!(context.counterparty_pubkey, "buyer");
        assert_eq!(context.root_event_id.as_deref(), Some("root-id"));
        assert_eq!(context.prev_event_id.as_deref(), Some("prev-id"));
        assert!(context.listing_event.is_none());
    }
}
