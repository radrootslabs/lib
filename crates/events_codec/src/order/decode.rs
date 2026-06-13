#[cfg(all(not(feature = "std"), feature = "serde_json"))]
use alloc::{borrow::ToOwned, format, string::String, vec::Vec};

#[cfg(feature = "serde_json")]
use radroots_events::{
    RadrootsNostrEvent, RadrootsNostrEventPtr,
    kinds::{KIND_PROFILE, is_order_event_kind},
    order::{
        RadrootsOrderCancellation, RadrootsOrderDecision, RadrootsOrderEnvelope,
        RadrootsOrderEnvelopeError, RadrootsOrderEventType, RadrootsOrderFulfillmentUpdate,
        RadrootsOrderPayloadError, RadrootsOrderPaymentRecord, RadrootsOrderReceipt,
        RadrootsOrderRequest, RadrootsOrderRevisionDecision, RadrootsOrderRevisionProposal,
        RadrootsOrderSettlementDecision,
    },
    tags::{TAG_D, TAG_E_PREV, TAG_E_ROOT},
};
#[cfg(feature = "serde_json")]
use serde::de::DeserializeOwned;

#[cfg(feature = "serde_json")]
use crate::d_tag::is_d_tag_base64url;
#[cfg(feature = "serde_json")]
use crate::order::tags::{
    TAG_LISTING_EVENT, parse_order_counterparty_tag, parse_order_listing_event_tag,
    parse_order_prev_tag, parse_order_root_tag,
};

#[cfg(feature = "serde_json")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsOrderEnvelopeParseError {
    InvalidKind(u32),
    InvalidJson,
    InvalidEnvelope(RadrootsOrderEnvelopeError),
    InvalidPayload(RadrootsOrderPayloadError),
    MessageTypeKindMismatch {
        event_kind: u32,
        message_type: RadrootsOrderEventType,
    },
    MissingTag(&'static str),
    InvalidTag(&'static str),
    ListingAddrTagMismatch,
    OrderIdTagMismatch,
    PayloadBindingMismatch(&'static str),
    AuthorMismatch,
    CounterpartyTagMismatch,
    InvalidListingAddr(RadrootsOrderListingAddressError),
}

#[cfg(feature = "serde_json")]
impl core::fmt::Display for RadrootsOrderEnvelopeParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidKind(kind) => write!(f, "invalid order event kind: {kind}"),
            Self::InvalidJson => write!(f, "invalid order envelope json"),
            Self::InvalidEnvelope(error) => write!(f, "{error}"),
            Self::InvalidPayload(error) => write!(f, "{error}"),
            Self::MessageTypeKindMismatch {
                event_kind,
                message_type,
            } => write!(
                f,
                "order envelope type {message_type:?} does not match event kind {event_kind}"
            ),
            Self::MissingTag(tag) => write!(f, "missing required order tag: {tag}"),
            Self::InvalidTag(tag) => write!(f, "invalid order tag: {tag}"),
            Self::ListingAddrTagMismatch => {
                write!(f, "order listing address tag does not match envelope")
            }
            Self::OrderIdTagMismatch => {
                write!(f, "order order id tag does not match envelope")
            }
            Self::PayloadBindingMismatch(field) => {
                write!(f, "order payload {field} does not match envelope")
            }
            Self::AuthorMismatch => write!(f, "order event author does not match payload"),
            Self::CounterpartyTagMismatch => {
                write!(f, "order counterparty tag does not match payload")
            }
            Self::InvalidListingAddr(error) => write!(f, "{error}"),
        }
    }
}

#[cfg(all(feature = "std", feature = "serde_json"))]
impl std::error::Error for RadrootsOrderEnvelopeParseError {
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
pub struct RadrootsOrderEventContext {
    pub counterparty_pubkey: String,
    pub listing_event: Option<RadrootsNostrEventPtr>,
    pub root_event_id: Option<String>,
    pub prev_event_id: Option<String>,
}

#[cfg(feature = "serde_json")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOrderListingAddress {
    pub kind: u32,
    pub seller_pubkey: String,
    pub listing_id: String,
}

#[cfg(feature = "serde_json")]
impl RadrootsOrderListingAddress {
    pub fn parse(addr: &str) -> Result<Self, RadrootsOrderListingAddressError> {
        let (kind_raw, seller_and_listing) = addr
            .split_once(':')
            .ok_or(RadrootsOrderListingAddressError::InvalidFormat)?;
        let (seller_pubkey_raw, listing_id_raw) = seller_and_listing
            .split_once(':')
            .ok_or(RadrootsOrderListingAddressError::InvalidFormat)?;
        if listing_id_raw.contains(':') {
            return Err(RadrootsOrderListingAddressError::InvalidFormat);
        }
        let kind = kind_raw
            .parse::<u32>()
            .map_err(|_| RadrootsOrderListingAddressError::InvalidFormat)?;
        let seller_pubkey = seller_pubkey_raw.to_owned();
        let listing_id = listing_id_raw.to_owned();
        if kind == KIND_PROFILE
            || seller_pubkey.trim().is_empty()
            || listing_id.trim().is_empty()
            || !is_d_tag_base64url(&listing_id)
        {
            return Err(RadrootsOrderListingAddressError::InvalidFormat);
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
pub enum RadrootsOrderListingAddressError {
    InvalidFormat,
}

#[cfg(feature = "serde_json")]
impl core::fmt::Display for RadrootsOrderListingAddressError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidFormat => write!(f, "invalid listing address format"),
        }
    }
}

#[cfg(all(feature = "std", feature = "serde_json"))]
impl std::error::Error for RadrootsOrderListingAddressError {}

#[cfg(feature = "serde_json")]
pub fn order_envelope_from_event<T: DeserializeOwned>(
    event: &RadrootsNostrEvent,
) -> Result<RadrootsOrderEnvelope<T>, RadrootsOrderEnvelopeParseError> {
    if !is_order_event_kind(event.kind) {
        return Err(RadrootsOrderEnvelopeParseError::InvalidKind(event.kind));
    }
    let envelope = serde_json::from_str::<RadrootsOrderEnvelope<T>>(&event.content)
        .map_err(|_| RadrootsOrderEnvelopeParseError::InvalidJson)?;
    envelope
        .validate()
        .map_err(RadrootsOrderEnvelopeParseError::InvalidEnvelope)?;
    if envelope.message_type.kind() != event.kind {
        return Err(RadrootsOrderEnvelopeParseError::MessageTypeKindMismatch {
            event_kind: event.kind,
            message_type: envelope.message_type,
        });
    }

    let listing_addr = required_order_tag_value(&event.tags, "a")?;
    if envelope.listing_addr != listing_addr {
        return Err(RadrootsOrderEnvelopeParseError::ListingAddrTagMismatch);
    }
    RadrootsOrderListingAddress::parse(&envelope.listing_addr)
        .map_err(RadrootsOrderEnvelopeParseError::InvalidListingAddr)?;

    let tag_order_id = required_order_tag_value(&event.tags, TAG_D)?;
    if tag_order_id != envelope.order_id {
        return Err(RadrootsOrderEnvelopeParseError::OrderIdTagMismatch);
    }

    order_event_context_from_tags(envelope.message_type, &event.tags)?;
    Ok(envelope)
}

#[cfg(feature = "serde_json")]
pub fn order_request_from_event(
    event: &RadrootsNostrEvent,
) -> Result<RadrootsOrderEnvelope<RadrootsOrderRequest>, RadrootsOrderEnvelopeParseError> {
    let envelope = order_envelope_from_event::<RadrootsOrderRequest>(event)?;
    if envelope.message_type != RadrootsOrderEventType::OrderRequested {
        return Err(RadrootsOrderEnvelopeParseError::MessageTypeKindMismatch {
            event_kind: event.kind,
            message_type: envelope.message_type,
        });
    }
    envelope
        .payload
        .validate()
        .map_err(RadrootsOrderEnvelopeParseError::InvalidPayload)?;
    validate_order_binding(
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
pub fn order_decision_from_event(
    event: &RadrootsNostrEvent,
) -> Result<RadrootsOrderEnvelope<RadrootsOrderDecision>, RadrootsOrderEnvelopeParseError> {
    let envelope = order_envelope_from_event::<RadrootsOrderDecision>(event)?;
    if envelope.message_type != RadrootsOrderEventType::OrderDecision {
        return Err(RadrootsOrderEnvelopeParseError::MessageTypeKindMismatch {
            event_kind: event.kind,
            message_type: envelope.message_type,
        });
    }
    envelope
        .payload
        .validate()
        .map_err(RadrootsOrderEnvelopeParseError::InvalidPayload)?;
    validate_order_binding(
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
pub fn order_revision_proposal_from_event(
    event: &RadrootsNostrEvent,
) -> Result<RadrootsOrderEnvelope<RadrootsOrderRevisionProposal>, RadrootsOrderEnvelopeParseError> {
    let envelope = order_envelope_from_event::<RadrootsOrderRevisionProposal>(event)?;
    if envelope.message_type != RadrootsOrderEventType::OrderRevisionProposed {
        return Err(RadrootsOrderEnvelopeParseError::MessageTypeKindMismatch {
            event_kind: event.kind,
            message_type: envelope.message_type,
        });
    }
    envelope
        .payload
        .validate()
        .map_err(RadrootsOrderEnvelopeParseError::InvalidPayload)?;
    validate_order_binding(
        event,
        &envelope,
        &envelope.payload.order_id,
        &envelope.payload.listing_addr,
        &envelope.payload.seller_pubkey,
        &envelope.payload.buyer_pubkey,
    )?;
    let context = order_event_context_from_tags(envelope.message_type, &event.tags)?;
    if context.root_event_id.as_deref() != Some(envelope.payload.root_event_id.as_str()) {
        return Err(RadrootsOrderEnvelopeParseError::PayloadBindingMismatch(
            "root_event_id",
        ));
    }
    if context.prev_event_id.as_deref() != Some(envelope.payload.prev_event_id.as_str()) {
        return Err(RadrootsOrderEnvelopeParseError::PayloadBindingMismatch(
            "prev_event_id",
        ));
    }
    Ok(envelope)
}

#[cfg(feature = "serde_json")]
pub fn order_revision_decision_from_event(
    event: &RadrootsNostrEvent,
) -> Result<RadrootsOrderEnvelope<RadrootsOrderRevisionDecision>, RadrootsOrderEnvelopeParseError> {
    let envelope = order_envelope_from_event::<RadrootsOrderRevisionDecision>(event)?;
    if envelope.message_type != RadrootsOrderEventType::OrderRevisionDecision {
        return Err(RadrootsOrderEnvelopeParseError::MessageTypeKindMismatch {
            event_kind: event.kind,
            message_type: envelope.message_type,
        });
    }
    envelope
        .payload
        .validate()
        .map_err(RadrootsOrderEnvelopeParseError::InvalidPayload)?;
    validate_order_binding(
        event,
        &envelope,
        &envelope.payload.order_id,
        &envelope.payload.listing_addr,
        &envelope.payload.buyer_pubkey,
        &envelope.payload.seller_pubkey,
    )?;
    let context = order_event_context_from_tags(envelope.message_type, &event.tags)?;
    if context.root_event_id.as_deref() != Some(envelope.payload.root_event_id.as_str()) {
        return Err(RadrootsOrderEnvelopeParseError::PayloadBindingMismatch(
            "root_event_id",
        ));
    }
    if context.prev_event_id.as_deref() != Some(envelope.payload.prev_event_id.as_str()) {
        return Err(RadrootsOrderEnvelopeParseError::PayloadBindingMismatch(
            "prev_event_id",
        ));
    }
    Ok(envelope)
}

#[cfg(feature = "serde_json")]
pub fn order_fulfillment_update_from_event(
    event: &RadrootsNostrEvent,
) -> Result<RadrootsOrderEnvelope<RadrootsOrderFulfillmentUpdate>, RadrootsOrderEnvelopeParseError>
{
    let envelope = order_envelope_from_event::<RadrootsOrderFulfillmentUpdate>(event)?;
    if envelope.message_type != RadrootsOrderEventType::FulfillmentUpdated {
        return Err(RadrootsOrderEnvelopeParseError::MessageTypeKindMismatch {
            event_kind: event.kind,
            message_type: envelope.message_type,
        });
    }
    envelope
        .payload
        .validate()
        .map_err(RadrootsOrderEnvelopeParseError::InvalidPayload)?;
    validate_order_binding(
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
pub fn order_cancellation_from_event(
    event: &RadrootsNostrEvent,
) -> Result<RadrootsOrderEnvelope<RadrootsOrderCancellation>, RadrootsOrderEnvelopeParseError> {
    let envelope = order_envelope_from_event::<RadrootsOrderCancellation>(event)?;
    if envelope.message_type != RadrootsOrderEventType::OrderCancelled {
        return Err(RadrootsOrderEnvelopeParseError::MessageTypeKindMismatch {
            event_kind: event.kind,
            message_type: envelope.message_type,
        });
    }
    envelope
        .payload
        .validate()
        .map_err(RadrootsOrderEnvelopeParseError::InvalidPayload)?;
    validate_order_binding(
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
pub fn order_receipt_from_event(
    event: &RadrootsNostrEvent,
) -> Result<RadrootsOrderEnvelope<RadrootsOrderReceipt>, RadrootsOrderEnvelopeParseError> {
    let envelope = order_envelope_from_event::<RadrootsOrderReceipt>(event)?;
    if envelope.message_type != RadrootsOrderEventType::BuyerReceipt {
        return Err(RadrootsOrderEnvelopeParseError::MessageTypeKindMismatch {
            event_kind: event.kind,
            message_type: envelope.message_type,
        });
    }
    envelope
        .payload
        .validate()
        .map_err(RadrootsOrderEnvelopeParseError::InvalidPayload)?;
    validate_order_binding(
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
pub fn order_payment_record_from_event(
    event: &RadrootsNostrEvent,
) -> Result<RadrootsOrderEnvelope<RadrootsOrderPaymentRecord>, RadrootsOrderEnvelopeParseError> {
    let envelope = order_envelope_from_event::<RadrootsOrderPaymentRecord>(event)?;
    if envelope.message_type != RadrootsOrderEventType::PaymentRecorded {
        return Err(RadrootsOrderEnvelopeParseError::MessageTypeKindMismatch {
            event_kind: event.kind,
            message_type: envelope.message_type,
        });
    }
    envelope
        .payload
        .validate()
        .map_err(RadrootsOrderEnvelopeParseError::InvalidPayload)?;
    validate_order_binding(
        event,
        &envelope,
        &envelope.payload.order_id,
        &envelope.payload.listing_addr,
        &envelope.payload.buyer_pubkey,
        &envelope.payload.seller_pubkey,
    )?;
    let context = order_event_context_from_tags(envelope.message_type, &event.tags)?;
    if context.root_event_id.as_deref() != Some(envelope.payload.root_event_id.as_str()) {
        return Err(RadrootsOrderEnvelopeParseError::PayloadBindingMismatch(
            "root_event_id",
        ));
    }
    if context.prev_event_id.as_deref() != Some(envelope.payload.previous_event_id.as_str()) {
        return Err(RadrootsOrderEnvelopeParseError::PayloadBindingMismatch(
            "previous_event_id",
        ));
    }
    Ok(envelope)
}

#[cfg(feature = "serde_json")]
pub fn order_settlement_decision_from_event(
    event: &RadrootsNostrEvent,
) -> Result<RadrootsOrderEnvelope<RadrootsOrderSettlementDecision>, RadrootsOrderEnvelopeParseError>
{
    let envelope = order_envelope_from_event::<RadrootsOrderSettlementDecision>(event)?;
    if envelope.message_type != RadrootsOrderEventType::SettlementDecision {
        return Err(RadrootsOrderEnvelopeParseError::MessageTypeKindMismatch {
            event_kind: event.kind,
            message_type: envelope.message_type,
        });
    }
    envelope
        .payload
        .validate()
        .map_err(RadrootsOrderEnvelopeParseError::InvalidPayload)?;
    validate_order_binding(
        event,
        &envelope,
        &envelope.payload.order_id,
        &envelope.payload.listing_addr,
        &envelope.payload.seller_pubkey,
        &envelope.payload.buyer_pubkey,
    )?;
    let context = order_event_context_from_tags(envelope.message_type, &event.tags)?;
    if context.root_event_id.as_deref() != Some(envelope.payload.root_event_id.as_str()) {
        return Err(RadrootsOrderEnvelopeParseError::PayloadBindingMismatch(
            "root_event_id",
        ));
    }
    if context.prev_event_id.as_deref() != Some(envelope.payload.previous_event_id.as_str()) {
        return Err(RadrootsOrderEnvelopeParseError::PayloadBindingMismatch(
            "previous_event_id",
        ));
    }
    Ok(envelope)
}

#[cfg(feature = "serde_json")]
pub fn order_event_context_from_tags(
    message_type: RadrootsOrderEventType,
    tags: &[Vec<String>],
) -> Result<RadrootsOrderEventContext, RadrootsOrderEnvelopeParseError> {
    let counterparty_pubkey =
        parse_order_counterparty_tag(tags).map_err(map_tag_parse_error_for_order_envelope)?;
    let listing_event =
        parse_order_listing_event_tag(tags).map_err(map_tag_parse_error_for_order_envelope)?;
    let root_event_id =
        parse_order_root_tag(tags).map_err(map_tag_parse_error_for_order_envelope)?;
    let prev_event_id =
        parse_order_prev_tag(tags).map_err(map_tag_parse_error_for_order_envelope)?;

    if message_type.requires_listing_snapshot() && listing_event.is_none() {
        return Err(RadrootsOrderEnvelopeParseError::MissingTag(
            TAG_LISTING_EVENT,
        ));
    }
    if message_type.requires_order_chain() {
        if root_event_id.is_none() {
            return Err(RadrootsOrderEnvelopeParseError::MissingTag(TAG_E_ROOT));
        }
        if prev_event_id.is_none() {
            return Err(RadrootsOrderEnvelopeParseError::MissingTag(TAG_E_PREV));
        }
    }

    Ok(RadrootsOrderEventContext {
        counterparty_pubkey,
        listing_event,
        root_event_id,
        prev_event_id,
    })
}

#[cfg(feature = "serde_json")]
fn required_order_tag_value<'a>(
    tags: &'a [Vec<String>],
    key: &'static str,
) -> Result<&'a str, RadrootsOrderEnvelopeParseError> {
    let tag = tags
        .iter()
        .find(|tag| tag.first().map(|value| value.as_str()) == Some(key))
        .ok_or(RadrootsOrderEnvelopeParseError::MissingTag(key))?;
    let value = tag
        .get(1)
        .map(|value| value.as_str())
        .ok_or(RadrootsOrderEnvelopeParseError::InvalidTag(key))?;
    if value.trim().is_empty() {
        return Err(RadrootsOrderEnvelopeParseError::InvalidTag(key));
    }
    Ok(value)
}

#[cfg(feature = "serde_json")]
fn map_tag_parse_error_for_order_envelope(
    error: crate::error::EventParseError,
) -> RadrootsOrderEnvelopeParseError {
    match error {
        crate::error::EventParseError::MissingTag(tag) => {
            RadrootsOrderEnvelopeParseError::MissingTag(tag)
        }
        crate::error::EventParseError::InvalidTag(tag) => {
            RadrootsOrderEnvelopeParseError::InvalidTag(tag)
        }
        crate::error::EventParseError::InvalidKind { expected: _, got } => {
            RadrootsOrderEnvelopeParseError::InvalidKind(got)
        }
        crate::error::EventParseError::InvalidNumber(tag, _)
        | crate::error::EventParseError::InvalidJson(tag) => {
            RadrootsOrderEnvelopeParseError::InvalidTag(tag)
        }
    }
}

#[cfg(feature = "serde_json")]
fn validate_order_binding<T>(
    event: &RadrootsNostrEvent,
    envelope: &RadrootsOrderEnvelope<T>,
    payload_order_id: &str,
    payload_listing_addr: &str,
    expected_author: &str,
    expected_counterparty: &str,
) -> Result<(), RadrootsOrderEnvelopeParseError> {
    if envelope.order_id != payload_order_id {
        return Err(RadrootsOrderEnvelopeParseError::PayloadBindingMismatch(
            "order_id",
        ));
    }
    if envelope.listing_addr != payload_listing_addr {
        return Err(RadrootsOrderEnvelopeParseError::PayloadBindingMismatch(
            "listing_addr",
        ));
    }
    if event.author != expected_author {
        return Err(RadrootsOrderEnvelopeParseError::AuthorMismatch);
    }
    let counterparty = parse_order_counterparty_tag(&event.tags)
        .map_err(map_tag_parse_error_for_order_envelope)?;
    if counterparty != expected_counterparty {
        return Err(RadrootsOrderEnvelopeParseError::CounterpartyTagMismatch);
    }
    Ok(())
}

#[cfg(all(test, feature = "serde_json"))]
mod tests {
    use super::{
        RadrootsOrderEnvelopeParseError, RadrootsOrderListingAddress,
        order_cancellation_from_event, order_decision_from_event, order_envelope_from_event,
        order_fulfillment_update_from_event, order_payment_record_from_event,
        order_receipt_from_event, order_request_from_event, order_revision_decision_from_event,
        order_revision_proposal_from_event, order_settlement_decision_from_event,
    };
    use crate::order::encode::{
        order_cancellation_event_build, order_decision_event_build,
        order_fulfillment_update_event_build, order_payment_record_event_build,
        order_receipt_event_build, order_request_event_build, order_revision_decision_event_build,
        order_revision_proposal_event_build, order_settlement_decision_event_build,
    };
    use crate::order::tags::TAG_LISTING_EVENT;
    use radroots_core::{
        RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreUnit,
    };
    use radroots_events::{
        RadrootsNostrEvent, RadrootsNostrEventPtr,
        ids::{
            RadrootsEconomicsDigest, RadrootsInventoryBinId, RadrootsListingAddress,
            RadrootsOrderId, RadrootsOrderQuoteId, RadrootsOrderRevisionId,
        },
        kinds::{
            KIND_ORDER_CANCELLATION, KIND_ORDER_DECISION, KIND_ORDER_FULFILLMENT_UPDATE,
            KIND_ORDER_PAYMENT_RECORD, KIND_ORDER_RECEIPT, KIND_ORDER_REQUEST,
            KIND_ORDER_REVISION_DECISION, KIND_ORDER_REVISION_PROPOSAL,
            KIND_ORDER_SETTLEMENT_DECISION,
        },
        order::{
            RadrootsOrderCancellation, RadrootsOrderDecision, RadrootsOrderDecisionOutcome,
            RadrootsOrderEconomicItem, RadrootsOrderEconomicLine, RadrootsOrderEconomics,
            RadrootsOrderEnvelope, RadrootsOrderEventType, RadrootsOrderFulfillmentState,
            RadrootsOrderFulfillmentUpdate, RadrootsOrderInventoryCommitment, RadrootsOrderItem,
            RadrootsOrderPayloadError, RadrootsOrderPaymentMethod, RadrootsOrderPaymentRecord,
            RadrootsOrderPricingBasis, RadrootsOrderReceipt, RadrootsOrderRequest,
            RadrootsOrderRevisionDecision, RadrootsOrderRevisionOutcome,
            RadrootsOrderRevisionProposal, RadrootsOrderSettlementDecision,
            RadrootsOrderSettlementOutcome,
        },
        tags::{TAG_D, TAG_E_PREV, TAG_E_ROOT},
    };

    fn seller_pubkey() -> String {
        "a".repeat(64)
    }

    fn listing_addr() -> RadrootsListingAddress {
        format!("30402:{}:AAAAAAAAAAAAAAAAAAAAAg", seller_pubkey())
            .parse()
            .unwrap()
    }

    fn listing_addr_wire() -> String {
        listing_addr().into_string()
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

    fn order_request() -> RadrootsOrderRequest {
        RadrootsOrderRequest {
            order_id: order_id("order-1"),
            listing_addr: listing_addr(),
            buyer_pubkey: "buyer".into(),
            seller_pubkey: "seller".into(),
            items: vec![RadrootsOrderItem {
                bin_id: bin_id("lb"),
                bin_count: 3,
            }],
            economics: request_economics(),
        }
    }

    fn decimal(raw: &str) -> RadrootsCoreDecimal {
        raw.parse().unwrap()
    }

    fn usd(raw: &str) -> RadrootsCoreMoney {
        RadrootsCoreMoney::new(decimal(raw), RadrootsCoreCurrency::USD)
    }

    fn request_economics() -> RadrootsOrderEconomics {
        RadrootsOrderEconomics {
            quote_id: quote_id("quote-1"),
            quote_version: 1,
            pricing_basis: RadrootsOrderPricingBasis::ListingEvent,
            currency: RadrootsCoreCurrency::USD,
            items: vec![RadrootsOrderEconomicItem {
                bin_id: bin_id("lb"),
                bin_count: 3,
                quantity_amount: decimal("1"),
                quantity_unit: RadrootsCoreUnit::Each,
                unit_price_amount: decimal("5"),
                unit_price_currency: RadrootsCoreCurrency::USD,
                line_subtotal: usd("15"),
            }],
            discounts: Vec::<RadrootsOrderEconomicLine>::new(),
            adjustments: Vec::<RadrootsOrderEconomicLine>::new(),
            subtotal: usd("15"),
            discount_total: usd("0"),
            adjustment_total: usd("0"),
            total: usd("15"),
        }
    }

    fn order_decision() -> RadrootsOrderDecision {
        RadrootsOrderDecision {
            order_id: order_id("order-1"),
            listing_addr: listing_addr(),
            buyer_pubkey: "buyer".into(),
            seller_pubkey: "seller".into(),
            decision: RadrootsOrderDecisionOutcome::Accepted {
                inventory_commitments: vec![RadrootsOrderInventoryCommitment {
                    bin_id: bin_id("lb"),
                    bin_count: 3,
                }],
            },
        }
    }

    fn order_revision_proposal() -> RadrootsOrderRevisionProposal {
        let mut economics = request_economics();
        economics.quote_id = quote_id("revision-quote-1");
        economics.quote_version = 2;
        economics.items[0].bin_count = 4;
        economics.items[0].line_subtotal = usd("20");
        economics.subtotal = usd("20");
        economics.total = usd("20");
        economics.canonicalize();
        RadrootsOrderRevisionProposal {
            revision_id: revision_id("rev-1"),
            order_id: order_id("order-1"),
            listing_addr: listing_addr(),
            buyer_pubkey: "buyer".into(),
            seller_pubkey: "seller".into(),
            root_event_id: "root-event".into(),
            prev_event_id: "decision-event".into(),
            items: vec![RadrootsOrderItem {
                bin_id: bin_id("lb"),
                bin_count: 4,
            }],
            economics,
            reason: "update count".into(),
        }
    }

    fn order_revision_decision(
        decision: RadrootsOrderRevisionOutcome,
    ) -> RadrootsOrderRevisionDecision {
        RadrootsOrderRevisionDecision {
            revision_id: revision_id("rev-1"),
            order_id: order_id("order-1"),
            listing_addr: listing_addr(),
            buyer_pubkey: "buyer".into(),
            seller_pubkey: "seller".into(),
            root_event_id: "root-event".into(),
            prev_event_id: "revision-event".into(),
            decision,
        }
    }

    fn order_fulfillment_update() -> RadrootsOrderFulfillmentUpdate {
        RadrootsOrderFulfillmentUpdate {
            order_id: order_id("order-1"),
            listing_addr: listing_addr(),
            buyer_pubkey: "buyer".into(),
            seller_pubkey: "seller".into(),
            status: RadrootsOrderFulfillmentState::ReadyForPickup,
        }
    }

    fn order_cancelled() -> RadrootsOrderCancellation {
        RadrootsOrderCancellation {
            order_id: order_id("order-1"),
            listing_addr: listing_addr(),
            buyer_pubkey: "buyer".into(),
            seller_pubkey: "seller".into(),
            reason: "changed plans".into(),
        }
    }

    fn order_buyer_receipt(received: bool) -> RadrootsOrderReceipt {
        RadrootsOrderReceipt {
            order_id: order_id("order-1"),
            listing_addr: listing_addr(),
            buyer_pubkey: "buyer".into(),
            seller_pubkey: "seller".into(),
            received,
            issue: (!received).then(|| "damaged items".into()),
            received_at: 1_777_665_600,
        }
    }

    fn order_payment_recorded() -> RadrootsOrderPaymentRecord {
        RadrootsOrderPaymentRecord {
            order_id: order_id("order-1"),
            listing_addr: listing_addr(),
            buyer_pubkey: "buyer".into(),
            seller_pubkey: "seller".into(),
            root_event_id: "root-event".into(),
            previous_event_id: "agreement-event".into(),
            agreement_event_id: "agreement-event".into(),
            quote_id: quote_id("quote-1"),
            quote_version: 1,
            economics_digest: digest("digest-1"),
            amount: decimal("15"),
            currency: RadrootsCoreCurrency::USD,
            method: RadrootsOrderPaymentMethod::Cash,
            reference: Some("cash drawer".into()),
            paid_at: Some(1_777_665_600),
        }
    }

    fn order_settlement_decision(
        decision: RadrootsOrderSettlementOutcome,
    ) -> RadrootsOrderSettlementDecision {
        RadrootsOrderSettlementDecision {
            order_id: order_id("order-1"),
            listing_addr: listing_addr(),
            seller_pubkey: "seller".into(),
            buyer_pubkey: "buyer".into(),
            root_event_id: "root-event".into(),
            previous_event_id: "payment-event".into(),
            agreement_event_id: "agreement-event".into(),
            payment_event_id: "payment-event".into(),
            quote_id: quote_id("quote-1"),
            quote_version: 1,
            economics_digest: digest("digest-1"),
            amount: decimal("15"),
            currency: RadrootsCoreCurrency::USD,
            decision,
            reason: (decision == RadrootsOrderSettlementOutcome::Rejected)
                .then(|| "reference mismatch".into()),
        }
    }

    fn event_id(character: char) -> String {
        core::iter::repeat_n(character, 64).collect()
    }

    fn listing_event_ptr() -> RadrootsNostrEventPtr {
        RadrootsNostrEventPtr {
            id: event_id('a'),
            relays: Some("wss://relay.example.com".into()),
        }
    }

    #[test]
    fn listing_address_roundtrips() {
        let addr = RadrootsOrderListingAddress::parse("30402:seller:AAAAAAAAAAAAAAAAAAAAAg")
            .expect("parse listing address");
        assert_eq!(addr.as_str(), "30402:seller:AAAAAAAAAAAAAAAAAAAAAg");
    }

    #[test]
    fn order_request_builder_emits_canonical_shape() {
        let payload = order_request();
        let built = order_request_event_build(&listing_event_ptr(), &payload).unwrap();
        let envelope: RadrootsOrderEnvelope<RadrootsOrderRequest> =
            serde_json::from_str(&built.content).unwrap();

        assert_eq!(built.kind, KIND_ORDER_REQUEST);
        assert_eq!(
            envelope.message_type,
            RadrootsOrderEventType::OrderRequested
        );
        assert_eq!(envelope.order_id, "order-1");
        assert_eq!(built.tags[0], vec!["p".to_string(), "seller".to_string()]);
        assert_eq!(built.tags[1], vec!["a".to_string(), listing_addr_wire()]);
        assert_eq!(
            built.tags[2],
            vec![TAG_D.to_string(), "order-1".to_string()]
        );
        assert_eq!(envelope.payload.economics.quote_id, "quote-1");
        assert_eq!(envelope.payload.economics.total, usd("15"));
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
    fn order_decision_builder_emits_canonical_chain_shape() {
        let payload = order_decision();
        let built = order_decision_event_build("root-event", "prev-event", &payload).unwrap();
        let envelope: RadrootsOrderEnvelope<RadrootsOrderDecision> =
            serde_json::from_str(&built.content).unwrap();

        assert_eq!(built.kind, KIND_ORDER_DECISION);
        assert_eq!(envelope.message_type, RadrootsOrderEventType::OrderDecision);
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
    fn order_revision_proposal_builder_emits_canonical_chain_shape() {
        let payload = order_revision_proposal();
        let built = order_revision_proposal_event_build(
            payload.root_event_id.as_str(),
            payload.prev_event_id.as_str(),
            &payload,
        )
        .unwrap();
        let envelope: RadrootsOrderEnvelope<RadrootsOrderRevisionProposal> =
            serde_json::from_str(&built.content).unwrap();

        assert_eq!(built.kind, KIND_ORDER_REVISION_PROPOSAL);
        assert_eq!(
            envelope.message_type,
            RadrootsOrderEventType::OrderRevisionProposed
        );
        assert_eq!(built.tags[0], vec!["p".to_string(), "buyer".to_string()]);
        assert_eq!(
            built.tags[2],
            vec![TAG_D.to_string(), "order-1".to_string()]
        );
        assert_eq!(envelope.payload.revision_id, "rev-1");
        assert_eq!(envelope.payload.economics.quote_version, 2);
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
                .any(|tag| tag == &vec![TAG_E_PREV.to_string(), "decision-event".to_string()])
        );
    }

    #[test]
    fn order_revision_decision_builder_emits_canonical_chain_shape() {
        let payload = order_revision_decision(RadrootsOrderRevisionOutcome::Accepted);
        let built = order_revision_decision_event_build(
            payload.root_event_id.as_str(),
            payload.prev_event_id.as_str(),
            &payload,
        )
        .unwrap();
        let envelope: RadrootsOrderEnvelope<RadrootsOrderRevisionDecision> =
            serde_json::from_str(&built.content).unwrap();

        assert_eq!(built.kind, KIND_ORDER_REVISION_DECISION);
        assert_eq!(
            envelope.message_type,
            RadrootsOrderEventType::OrderRevisionDecision
        );
        assert_eq!(built.tags[0], vec!["p".to_string(), "seller".to_string()]);
        assert_eq!(
            built.tags[2],
            vec![TAG_D.to_string(), "order-1".to_string()]
        );
        assert_eq!(envelope.payload.revision_id, "rev-1");
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
                .any(|tag| tag == &vec![TAG_E_PREV.to_string(), "revision-event".to_string()])
        );
    }

    #[test]
    fn order_fulfillment_update_builder_emits_canonical_chain_shape() {
        let payload = order_fulfillment_update();
        let built =
            order_fulfillment_update_event_build("root-event", "prev-event", &payload).unwrap();
        let envelope: RadrootsOrderEnvelope<RadrootsOrderFulfillmentUpdate> =
            serde_json::from_str(&built.content).unwrap();

        assert_eq!(built.kind, KIND_ORDER_FULFILLMENT_UPDATE);
        assert_eq!(
            envelope.message_type,
            RadrootsOrderEventType::FulfillmentUpdated
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
    fn order_cancellation_builder_emits_canonical_buyer_chain_shape() {
        let payload = order_cancelled();
        let built = order_cancellation_event_build("root-event", "prev-event", &payload).unwrap();
        let envelope: RadrootsOrderEnvelope<RadrootsOrderCancellation> =
            serde_json::from_str(&built.content).unwrap();

        assert_eq!(built.kind, KIND_ORDER_CANCELLATION);
        assert_eq!(
            envelope.message_type,
            RadrootsOrderEventType::OrderCancelled
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
    fn order_buyer_receipt_builder_emits_canonical_buyer_chain_shape() {
        let payload = order_buyer_receipt(false);
        let built = order_receipt_event_build("root-event", "prev-event", &payload).unwrap();
        let envelope: RadrootsOrderEnvelope<RadrootsOrderReceipt> =
            serde_json::from_str(&built.content).unwrap();

        assert_eq!(built.kind, KIND_ORDER_RECEIPT);
        assert_eq!(envelope.message_type, RadrootsOrderEventType::BuyerReceipt);
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
    fn order_payment_recorded_builder_emits_canonical_buyer_chain_shape() {
        let payload = order_payment_recorded();
        let built = order_payment_record_event_build(
            payload.root_event_id.as_str(),
            payload.previous_event_id.as_str(),
            &payload,
        )
        .unwrap();
        let envelope: RadrootsOrderEnvelope<RadrootsOrderPaymentRecord> =
            serde_json::from_str(&built.content).unwrap();

        assert_eq!(built.kind, KIND_ORDER_PAYMENT_RECORD);
        assert_eq!(
            envelope.message_type,
            RadrootsOrderEventType::PaymentRecorded
        );
        assert_eq!(envelope.payload.amount, decimal("15"));
        assert_eq!(envelope.payload.method, RadrootsOrderPaymentMethod::Cash);
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
                .any(|tag| tag == &vec![TAG_E_PREV.to_string(), "agreement-event".to_string()])
        );
    }

    #[test]
    fn order_settlement_decision_builder_emits_canonical_seller_chain_shape() {
        let payload = order_settlement_decision(RadrootsOrderSettlementOutcome::Accepted);
        let built = order_settlement_decision_event_build(
            payload.root_event_id.as_str(),
            payload.previous_event_id.as_str(),
            &payload,
        )
        .unwrap();
        let envelope: RadrootsOrderEnvelope<RadrootsOrderSettlementDecision> =
            serde_json::from_str(&built.content).unwrap();

        assert_eq!(built.kind, KIND_ORDER_SETTLEMENT_DECISION);
        assert_eq!(
            envelope.message_type,
            RadrootsOrderEventType::SettlementDecision
        );
        assert_eq!(
            envelope.payload.decision,
            RadrootsOrderSettlementOutcome::Accepted
        );
        assert_eq!(envelope.payload.reason, None);
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
                .any(|tag| tag == &vec![TAG_E_PREV.to_string(), "payment-event".to_string()])
        );
    }

    #[test]
    fn order_request_parse_roundtrips_and_validates_tags() {
        let payload = order_request();
        let built = order_request_event_build(&listing_event_ptr(), &payload).unwrap();
        let event = RadrootsNostrEvent {
            id: "event-id".into(),
            author: "buyer".into(),
            created_at: 1,
            kind: built.kind,
            tags: built.tags,
            content: built.content,
            sig: "sig".into(),
        };
        let envelope = order_request_from_event(&event).unwrap();

        assert_eq!(envelope.payload, payload);
        assert_eq!(
            envelope.message_type,
            RadrootsOrderEventType::OrderRequested
        );
    }

    #[test]
    fn order_request_parse_rejects_mismatched_economics() {
        let mut payload = order_request();
        let built = order_request_event_build(&listing_event_ptr(), &payload).unwrap();
        payload.economics.items[0].bin_id = bin_id("other-bin");
        let envelope = RadrootsOrderEnvelope::new(
            RadrootsOrderEventType::OrderRequested,
            payload.listing_addr.clone(),
            payload.order_id.clone(),
            payload,
        );
        let event = RadrootsNostrEvent {
            id: "event-id".into(),
            author: "buyer".into(),
            created_at: 1,
            kind: built.kind,
            tags: built.tags,
            content: serde_json::to_string(&envelope).unwrap(),
            sig: "sig".into(),
        };
        let err = order_request_from_event(&event).unwrap_err();
        assert_eq!(
            err,
            RadrootsOrderEnvelopeParseError::InvalidPayload(
                RadrootsOrderPayloadError::InvalidOrderEconomicsBinding {
                    field: "items.bin_id"
                }
            )
        );
    }

    #[test]
    fn order_decision_parse_roundtrips_and_validates_chain_tags() {
        let payload = order_decision();
        let built = order_decision_event_build("root-event", "prev-event", &payload).unwrap();
        let event = RadrootsNostrEvent {
            id: "event-id".into(),
            author: "seller".into(),
            created_at: 1,
            kind: built.kind,
            tags: built.tags,
            content: built.content,
            sig: "sig".into(),
        };
        let envelope = order_decision_from_event(&event).unwrap();

        assert_eq!(envelope.payload, payload);
        assert_eq!(envelope.message_type, RadrootsOrderEventType::OrderDecision);
    }

    #[test]
    fn order_fulfillment_update_parse_roundtrips_and_validates_chain_tags() {
        let payload = order_fulfillment_update();
        let built =
            order_fulfillment_update_event_build("root-event", "prev-event", &payload).unwrap();
        let event = RadrootsNostrEvent {
            id: "event-id".into(),
            author: "seller".into(),
            created_at: 1,
            kind: built.kind,
            tags: built.tags,
            content: built.content,
            sig: "sig".into(),
        };
        let envelope = order_fulfillment_update_from_event(&event).unwrap();

        assert_eq!(envelope.payload, payload);
        assert_eq!(
            envelope.message_type,
            RadrootsOrderEventType::FulfillmentUpdated
        );
    }

    #[test]
    fn order_cancellation_parse_roundtrips_and_validates_buyer_actor() {
        let payload = order_cancelled();
        let built = order_cancellation_event_build("root-event", "prev-event", &payload).unwrap();
        let event = RadrootsNostrEvent {
            id: "event-id".into(),
            author: "buyer".into(),
            created_at: 1,
            kind: built.kind,
            tags: built.tags,
            content: built.content,
            sig: "sig".into(),
        };
        let envelope = order_cancellation_from_event(&event).unwrap();

        assert_eq!(envelope.payload, payload);
        assert_eq!(
            envelope.message_type,
            RadrootsOrderEventType::OrderCancelled
        );
    }

    #[test]
    fn order_buyer_receipt_parse_roundtrips_and_validates_buyer_actor() {
        let payload = order_buyer_receipt(true);
        let built = order_receipt_event_build("root-event", "prev-event", &payload).unwrap();
        let event = RadrootsNostrEvent {
            id: "event-id".into(),
            author: "buyer".into(),
            created_at: 1,
            kind: built.kind,
            tags: built.tags,
            content: built.content,
            sig: "sig".into(),
        };
        let envelope = order_receipt_from_event(&event).unwrap();

        assert_eq!(envelope.payload, payload);
        assert_eq!(envelope.message_type, RadrootsOrderEventType::BuyerReceipt);
    }

    #[test]
    fn order_payment_recorded_parse_roundtrips_and_validates_buyer_actor() {
        let payload = order_payment_recorded();
        let built = order_payment_record_event_build(
            payload.root_event_id.as_str(),
            payload.previous_event_id.as_str(),
            &payload,
        )
        .unwrap();
        let event = RadrootsNostrEvent {
            id: "payment-event".into(),
            author: "buyer".into(),
            created_at: 1,
            kind: built.kind,
            tags: built.tags,
            content: built.content,
            sig: "sig".into(),
        };
        let envelope = order_payment_record_from_event(&event).unwrap();

        assert_eq!(envelope.payload, payload);
        assert_eq!(
            envelope.message_type,
            RadrootsOrderEventType::PaymentRecorded
        );
    }

    #[test]
    fn order_settlement_decision_parse_roundtrips_and_validates_seller_actor() {
        let payload = order_settlement_decision(RadrootsOrderSettlementOutcome::Rejected);
        let built = order_settlement_decision_event_build(
            payload.root_event_id.as_str(),
            payload.previous_event_id.as_str(),
            &payload,
        )
        .unwrap();
        let event = RadrootsNostrEvent {
            id: "settlement-event".into(),
            author: "seller".into(),
            created_at: 1,
            kind: built.kind,
            tags: built.tags,
            content: built.content,
            sig: "sig".into(),
        };
        let envelope = order_settlement_decision_from_event(&event).unwrap();

        assert_eq!(envelope.payload, payload);
        assert_eq!(
            envelope.message_type,
            RadrootsOrderEventType::SettlementDecision
        );
    }

    #[test]
    fn order_revision_proposal_parse_validates_actor_counterparty_and_chain_payload() {
        let payload = order_revision_proposal();
        let built = order_revision_proposal_event_build(
            payload.root_event_id.as_str(),
            payload.prev_event_id.as_str(),
            &payload,
        )
        .unwrap();
        let mut event = RadrootsNostrEvent {
            id: "event-id".into(),
            author: "seller".into(),
            created_at: 1,
            kind: built.kind,
            tags: built.tags,
            content: built.content,
            sig: "sig".into(),
        };
        let envelope = order_revision_proposal_from_event(&event).unwrap();
        assert_eq!(envelope.payload, payload);

        event.author = "buyer".into();
        let err = order_revision_proposal_from_event(&event).unwrap_err();
        assert_eq!(err, RadrootsOrderEnvelopeParseError::AuthorMismatch);
    }

    #[test]
    fn order_revision_decision_parse_validates_actor_counterparty_and_chain_payload() {
        let payload = order_revision_decision(RadrootsOrderRevisionOutcome::Declined {
            reason: "no change".into(),
        });
        let built = order_revision_decision_event_build(
            payload.root_event_id.as_str(),
            payload.prev_event_id.as_str(),
            &payload,
        )
        .unwrap();
        let mut event = RadrootsNostrEvent {
            id: "event-id".into(),
            author: "buyer".into(),
            created_at: 1,
            kind: built.kind,
            tags: built.tags,
            content: built.content,
            sig: "sig".into(),
        };
        let envelope = order_revision_decision_from_event(&event).unwrap();
        assert_eq!(envelope.payload, payload);

        event.author = "seller".into();
        let err = order_revision_decision_from_event(&event).unwrap_err();
        assert_eq!(err, RadrootsOrderEnvelopeParseError::AuthorMismatch);
    }

    #[test]
    fn order_revision_kinds_parse_with_chain_tags() {
        for (kind, message_type) in [
            (
                KIND_ORDER_REVISION_PROPOSAL,
                RadrootsOrderEventType::OrderRevisionProposed,
            ),
            (
                KIND_ORDER_REVISION_DECISION,
                RadrootsOrderEventType::OrderRevisionDecision,
            ),
        ] {
            let payload = serde_json::json!({});
            let envelope =
                RadrootsOrderEnvelope::new(message_type, listing_addr_wire(), "order-1", &payload);
            let event = RadrootsNostrEvent {
                id: "event-id".into(),
                author: "seller".into(),
                created_at: 1,
                kind,
                tags: vec![
                    vec!["p".into(), "buyer".into()],
                    vec!["a".into(), listing_addr_wire()],
                    vec![TAG_D.into(), "order-1".into()],
                    vec![TAG_E_ROOT.into(), "root-event".into()],
                    vec![TAG_E_PREV.into(), "prev-event".into()],
                ],
                content: serde_json::to_string(&envelope).unwrap(),
                sig: "sig".into(),
            };
            let parsed = order_envelope_from_event::<serde_json::Value>(&event).unwrap();

            assert_eq!(parsed.message_type, message_type);
            assert_eq!(parsed.order_id, "order-1");
        }
    }

    #[test]
    fn order_parse_rejects_forbidden_kind() {
        let event = RadrootsNostrEvent {
            id: "event-id".into(),
            author: "seller".into(),
            created_at: 1,
            kind: 3431,
            tags: Vec::new(),
            content: "{}".into(),
            sig: "sig".into(),
        };
        let err = order_envelope_from_event::<serde_json::Value>(&event).unwrap_err();
        assert_eq!(err, RadrootsOrderEnvelopeParseError::InvalidKind(3431));
    }

    #[test]
    fn order_parse_rejects_missing_required_refs() {
        let payload = order_decision();
        let built = order_decision_event_build("root-event", "prev-event", &payload).unwrap();
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

        let err = order_decision_from_event(&event).unwrap_err();
        assert_eq!(err, RadrootsOrderEnvelopeParseError::MissingTag(TAG_E_PREV));
    }

    #[test]
    fn order_parse_rejects_author_and_counterparty_mismatch() {
        let payload = order_request();
        let built = order_request_event_build(&listing_event_ptr(), &payload).unwrap();
        let mut event = RadrootsNostrEvent {
            id: "event-id".into(),
            author: "seller".into(),
            created_at: 1,
            kind: built.kind,
            tags: built.tags.clone(),
            content: built.content.clone(),
            sig: "sig".into(),
        };
        let err = order_request_from_event(&event).unwrap_err();
        assert_eq!(err, RadrootsOrderEnvelopeParseError::AuthorMismatch);

        event.author = "buyer".into();
        event.tags[0] = vec!["p".into(), "other-seller".into()];
        let err = order_request_from_event(&event).unwrap_err();
        assert_eq!(
            err,
            RadrootsOrderEnvelopeParseError::CounterpartyTagMismatch
        );
    }

    #[test]
    fn order_buyer_lifecycle_parse_rejects_wrong_actor_or_counterparty() {
        let cancellation = order_cancelled();
        let cancellation_parts =
            order_cancellation_event_build("root-event", "prev-event", &cancellation).unwrap();
        let cancellation_event = RadrootsNostrEvent {
            id: "event-id".into(),
            author: "seller".into(),
            created_at: 1,
            kind: cancellation_parts.kind,
            tags: cancellation_parts.tags,
            content: cancellation_parts.content,
            sig: "sig".into(),
        };
        let err = order_cancellation_from_event(&cancellation_event).unwrap_err();
        assert_eq!(err, RadrootsOrderEnvelopeParseError::AuthorMismatch);

        let receipt = order_buyer_receipt(true);
        let receipt_parts =
            order_receipt_event_build("root-event", "prev-event", &receipt).unwrap();
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
        let err = order_receipt_from_event(&receipt_event).unwrap_err();
        assert_eq!(
            err,
            RadrootsOrderEnvelopeParseError::CounterpartyTagMismatch
        );
    }
}
