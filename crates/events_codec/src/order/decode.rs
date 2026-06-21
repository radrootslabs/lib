#[cfg(all(not(feature = "std"), feature = "serde_json"))]
use alloc::{borrow::ToOwned, format, string::String, vec::Vec};

#[cfg(feature = "serde_json")]
use radroots_events::{
    RadrootsNostrEvent, RadrootsNostrEventPtr,
    ids::{RadrootsEventId, RadrootsIdParseError, RadrootsListingAddress, RadrootsPublicKey},
    kinds::is_order_event_kind,
    order::{
        RadrootsOrderCancellation, RadrootsOrderDecision, RadrootsOrderEnvelope,
        RadrootsOrderEnvelopeError, RadrootsOrderEventType, RadrootsOrderPayloadError,
        RadrootsOrderRequest, RadrootsOrderRevisionDecision, RadrootsOrderRevisionProposal,
    },
    tags::{TAG_D, TAG_E_PREV, TAG_E_ROOT},
};
#[cfg(feature = "serde_json")]
use serde::de::DeserializeOwned;

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
    InvalidListingAddr(RadrootsIdParseError),
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
    pub counterparty_pubkey: RadrootsPublicKey,
    pub listing_event: Option<RadrootsNostrEventPtr>,
    pub root_event_id: Option<RadrootsEventId>,
    pub prev_event_id: Option<RadrootsEventId>,
}

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
    RadrootsListingAddress::parse(&envelope.listing_addr)
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
pub fn order_event_context_from_tags(
    message_type: RadrootsOrderEventType,
    tags: &[Vec<String>],
) -> Result<RadrootsOrderEventContext, RadrootsOrderEnvelopeParseError> {
    let counterparty_pubkey =
        parse_order_counterparty_tag(tags).map_err(map_tag_parse_error_for_order_envelope)?;
    let counterparty_pubkey = RadrootsPublicKey::parse(&counterparty_pubkey)
        .map_err(|_| RadrootsOrderEnvelopeParseError::InvalidTag("p"))?;
    let listing_event =
        parse_order_listing_event_tag(tags).map_err(map_tag_parse_error_for_order_envelope)?;
    let root_event_id =
        parse_order_root_tag(tags).map_err(map_tag_parse_error_for_order_envelope)?;
    let root_event_id = root_event_id
        .map(|id| {
            RadrootsEventId::parse(id)
                .map_err(|_| RadrootsOrderEnvelopeParseError::InvalidTag(TAG_E_ROOT))
        })
        .transpose()?;
    let prev_event_id =
        parse_order_prev_tag(tags).map_err(map_tag_parse_error_for_order_envelope)?;
    let prev_event_id = prev_event_id
        .map(|id| {
            RadrootsEventId::parse(id)
                .map_err(|_| RadrootsOrderEnvelopeParseError::InvalidTag(TAG_E_PREV))
        })
        .transpose()?;

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
    let context = order_event_context_from_tags(envelope.message_type, &event.tags)?;
    if context.counterparty_pubkey.as_str() != expected_counterparty {
        return Err(RadrootsOrderEnvelopeParseError::CounterpartyTagMismatch);
    }
    Ok(())
}

#[cfg(all(test, feature = "serde_json"))]
mod tests {
    use super::{
        RadrootsOrderEnvelopeParseError, map_tag_parse_error_for_order_envelope,
        order_cancellation_from_event, order_decision_from_event, order_envelope_from_event,
        order_event_context_from_tags, order_request_from_event,
        order_revision_decision_from_event, order_revision_proposal_from_event,
    };
    use crate::order::encode::{
        order_cancellation_event_build, order_decision_event_build, order_request_event_build,
        order_revision_decision_event_build, order_revision_proposal_event_build,
    };
    use crate::order::tags::TAG_LISTING_EVENT;
    use radroots_core::{
        RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreUnit,
    };
    use radroots_events::{
        RadrootsNostrEvent, RadrootsNostrEventPtr,
        ids::{
            RadrootsEventId, RadrootsInventoryBinId, RadrootsListingAddress, RadrootsOrderId,
            RadrootsOrderQuoteId, RadrootsOrderRevisionId, RadrootsPublicKey,
        },
        kinds::{
            KIND_ORDER_CANCELLATION, KIND_ORDER_DECISION, KIND_ORDER_REQUEST,
            KIND_ORDER_REVISION_DECISION, KIND_ORDER_REVISION_PROPOSAL,
        },
        order::{
            RadrootsOrderCancellation, RadrootsOrderDecision, RadrootsOrderDecisionOutcome,
            RadrootsOrderEconomicItem, RadrootsOrderEconomicLine, RadrootsOrderEconomics,
            RadrootsOrderEnvelope, RadrootsOrderEnvelopeError, RadrootsOrderEventType,
            RadrootsOrderInventoryCommitment, RadrootsOrderItem, RadrootsOrderPayloadError,
            RadrootsOrderPricingBasis, RadrootsOrderRequest, RadrootsOrderRevisionDecision,
            RadrootsOrderRevisionOutcome, RadrootsOrderRevisionProposal,
        },
        tags::{TAG_D, TAG_E_PREV, TAG_E_ROOT},
    };

    fn pubkey(character: char) -> RadrootsPublicKey {
        core::iter::repeat_n(character, 64)
            .collect::<String>()
            .parse()
            .unwrap()
    }

    fn buyer_pubkey() -> RadrootsPublicKey {
        pubkey('b')
    }

    fn seller_pubkey() -> RadrootsPublicKey {
        pubkey('a')
    }

    fn buyer_pubkey_wire() -> String {
        buyer_pubkey().into_string()
    }

    fn seller_pubkey_wire() -> String {
        seller_pubkey().into_string()
    }

    fn listing_addr() -> RadrootsListingAddress {
        format!("30402:{}:AAAAAAAAAAAAAAAAAAAAAg", seller_pubkey_wire())
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

    fn event_id(character: char) -> RadrootsEventId {
        core::iter::repeat_n(character, 64)
            .collect::<String>()
            .parse()
            .unwrap()
    }

    fn event_id_wire(character: char) -> String {
        event_id(character).into_string()
    }

    fn order_request() -> RadrootsOrderRequest {
        RadrootsOrderRequest {
            order_id: order_id("order-1"),
            listing_addr: listing_addr(),
            buyer_pubkey: buyer_pubkey(),
            seller_pubkey: seller_pubkey(),
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
            buyer_pubkey: buyer_pubkey(),
            seller_pubkey: seller_pubkey(),
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
            buyer_pubkey: buyer_pubkey(),
            seller_pubkey: seller_pubkey(),
            root_event_id: event_id('1'),
            prev_event_id: event_id('2'),
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
            buyer_pubkey: buyer_pubkey(),
            seller_pubkey: seller_pubkey(),
            root_event_id: event_id('1'),
            prev_event_id: event_id('3'),
            decision,
        }
    }

    fn order_cancelled() -> RadrootsOrderCancellation {
        RadrootsOrderCancellation {
            order_id: order_id("order-1"),
            listing_addr: listing_addr(),
            buyer_pubkey: buyer_pubkey(),
            seller_pubkey: seller_pubkey(),
            reason: "changed plans".into(),
        }
    }

    fn listing_event_ptr() -> RadrootsNostrEventPtr {
        RadrootsNostrEventPtr {
            id: event_id_wire('a'),
            relays: Some("wss://relay.example.com".into()),
        }
    }

    fn order_request_tags() -> Vec<Vec<String>> {
        vec![
            vec!["p".into(), seller_pubkey_wire()],
            vec!["a".into(), listing_addr_wire()],
            vec![TAG_D.into(), "order-1".into()],
            vec![TAG_LISTING_EVENT.into(), event_id_wire('a')],
        ]
    }

    fn order_chain_tags(counterparty_pubkey: String) -> Vec<Vec<String>> {
        vec![
            vec!["p".into(), counterparty_pubkey],
            vec!["a".into(), listing_addr_wire()],
            vec![TAG_D.into(), "order-1".into()],
            vec![TAG_E_ROOT.into(), event_id_wire('1')],
            vec![TAG_E_PREV.into(), event_id_wire('2')],
        ]
    }

    fn order_event_with_envelope<T: serde::Serialize>(
        kind: u32,
        author: String,
        message_type: RadrootsOrderEventType,
        listing_addr: impl Into<String>,
        order_id: impl Into<String>,
        payload: &T,
        tags: Vec<Vec<String>>,
    ) -> RadrootsNostrEvent {
        let envelope = RadrootsOrderEnvelope::new(message_type, listing_addr, order_id, payload);
        RadrootsNostrEvent {
            id: event_id_wire('e'),
            author,
            created_at: 1,
            kind,
            tags,
            content: serde_json::to_string(&envelope).unwrap(),
            sig: "sig".into(),
        }
    }

    #[test]
    fn listing_address_roundtrips() {
        let raw = format!("30402:{}:listing-1", seller_pubkey_wire());
        let addr = RadrootsListingAddress::parse(&raw).expect("parse listing address");
        assert_eq!(addr.as_str(), raw);
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
        assert_eq!(built.tags[0], vec!["p".to_string(), seller_pubkey_wire()]);
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
        let root_event_id = event_id('1');
        let prev_event_id = event_id('9');
        let built = order_decision_event_build(&root_event_id, &prev_event_id, &payload).unwrap();
        let envelope: RadrootsOrderEnvelope<RadrootsOrderDecision> =
            serde_json::from_str(&built.content).unwrap();

        assert_eq!(built.kind, KIND_ORDER_DECISION);
        assert_eq!(envelope.message_type, RadrootsOrderEventType::OrderDecision);
        assert_eq!(built.tags[0], vec!["p".to_string(), buyer_pubkey_wire()]);
        assert_eq!(
            built.tags[2],
            vec![TAG_D.to_string(), "order-1".to_string()]
        );
        assert!(
            built
                .tags
                .iter()
                .any(|tag| tag == &vec![TAG_E_ROOT.to_string(), event_id_wire('1')])
        );
        assert!(
            built
                .tags
                .iter()
                .any(|tag| tag == &vec![TAG_E_PREV.to_string(), event_id_wire('9')])
        );
    }

    #[test]
    fn order_revision_proposal_builder_emits_canonical_chain_shape() {
        let payload = order_revision_proposal();
        let built = order_revision_proposal_event_build(
            &payload.root_event_id,
            &payload.prev_event_id,
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
        assert_eq!(built.tags[0], vec!["p".to_string(), buyer_pubkey_wire()]);
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
                .any(|tag| tag == &vec![TAG_E_ROOT.to_string(), event_id_wire('1')])
        );
        assert!(
            built
                .tags
                .iter()
                .any(|tag| tag == &vec![TAG_E_PREV.to_string(), event_id_wire('2')])
        );
    }

    #[test]
    fn order_revision_decision_builder_emits_canonical_chain_shape() {
        let payload = order_revision_decision(RadrootsOrderRevisionOutcome::Accepted);
        let built = order_revision_decision_event_build(
            &payload.root_event_id,
            &payload.prev_event_id,
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
        assert_eq!(built.tags[0], vec!["p".to_string(), seller_pubkey_wire()]);
        assert_eq!(
            built.tags[2],
            vec![TAG_D.to_string(), "order-1".to_string()]
        );
        assert_eq!(envelope.payload.revision_id, "rev-1");
        assert!(
            built
                .tags
                .iter()
                .any(|tag| tag == &vec![TAG_E_ROOT.to_string(), event_id_wire('1')])
        );
        assert!(
            built
                .tags
                .iter()
                .any(|tag| tag == &vec![TAG_E_PREV.to_string(), event_id_wire('3')])
        );
    }

    #[test]
    fn order_cancellation_builder_emits_canonical_buyer_chain_shape() {
        let payload = order_cancelled();
        let root_event_id = event_id('1');
        let prev_event_id = event_id('9');
        let built =
            order_cancellation_event_build(&root_event_id, &prev_event_id, &payload).unwrap();
        let envelope: RadrootsOrderEnvelope<RadrootsOrderCancellation> =
            serde_json::from_str(&built.content).unwrap();

        assert_eq!(built.kind, KIND_ORDER_CANCELLATION);
        assert_eq!(
            envelope.message_type,
            RadrootsOrderEventType::OrderCancelled
        );
        assert_eq!(envelope.payload.reason, payload.reason);
        assert_eq!(built.tags[0], vec!["p".to_string(), seller_pubkey_wire()]);
        assert_eq!(
            built.tags[2],
            vec![TAG_D.to_string(), "order-1".to_string()]
        );
        assert!(
            built
                .tags
                .iter()
                .any(|tag| tag == &vec![TAG_E_ROOT.to_string(), event_id_wire('1')])
        );
        assert!(
            built
                .tags
                .iter()
                .any(|tag| tag == &vec![TAG_E_PREV.to_string(), event_id_wire('9')])
        );
    }

    #[test]
    fn order_request_parse_roundtrips_and_validates_tags() {
        let payload = order_request();
        let built = order_request_event_build(&listing_event_ptr(), &payload).unwrap();
        let event = RadrootsNostrEvent {
            id: event_id_wire('e'),
            author: buyer_pubkey_wire(),
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
            id: event_id_wire('e'),
            author: buyer_pubkey_wire(),
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
        let root_event_id = event_id('1');
        let prev_event_id = event_id('9');
        let built = order_decision_event_build(&root_event_id, &prev_event_id, &payload).unwrap();
        let event = RadrootsNostrEvent {
            id: event_id_wire('e'),
            author: seller_pubkey_wire(),
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
    fn order_cancellation_parse_roundtrips_and_validates_buyer_actor() {
        let payload = order_cancelled();
        let root_event_id = event_id('1');
        let prev_event_id = event_id('9');
        let built =
            order_cancellation_event_build(&root_event_id, &prev_event_id, &payload).unwrap();
        let event = RadrootsNostrEvent {
            id: event_id_wire('e'),
            author: buyer_pubkey_wire(),
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
    fn order_revision_proposal_parse_validates_actor_counterparty_and_chain_payload() {
        let payload = order_revision_proposal();
        let built = order_revision_proposal_event_build(
            &payload.root_event_id,
            &payload.prev_event_id,
            &payload,
        )
        .unwrap();
        let mut event = RadrootsNostrEvent {
            id: event_id_wire('e'),
            author: seller_pubkey_wire(),
            created_at: 1,
            kind: built.kind,
            tags: built.tags,
            content: built.content,
            sig: "sig".into(),
        };
        let envelope = order_revision_proposal_from_event(&event).unwrap();
        assert_eq!(envelope.payload, payload);

        event.author = buyer_pubkey_wire();
        let err = order_revision_proposal_from_event(&event).unwrap_err();
        assert_eq!(err, RadrootsOrderEnvelopeParseError::AuthorMismatch);
    }

    #[test]
    fn order_revision_decision_parse_validates_actor_counterparty_and_chain_payload() {
        let payload = order_revision_decision(RadrootsOrderRevisionOutcome::Declined {
            reason: "no change".into(),
        });
        let built = order_revision_decision_event_build(
            &payload.root_event_id,
            &payload.prev_event_id,
            &payload,
        )
        .unwrap();
        let mut event = RadrootsNostrEvent {
            id: event_id_wire('e'),
            author: buyer_pubkey_wire(),
            created_at: 1,
            kind: built.kind,
            tags: built.tags,
            content: built.content,
            sig: "sig".into(),
        };
        let envelope = order_revision_decision_from_event(&event).unwrap();
        assert_eq!(envelope.payload, payload);

        event.author = seller_pubkey_wire();
        let err = order_revision_decision_from_event(&event).unwrap_err();
        assert_eq!(err, RadrootsOrderEnvelopeParseError::AuthorMismatch);
    }

    #[cfg(feature = "std")]
    #[test]
    fn order_parse_error_display_and_source_cover_variants() {
        use std::error::Error as _;

        let invalid_envelope = RadrootsOrderEnvelopeParseError::InvalidEnvelope(
            RadrootsOrderEnvelopeError::MissingOrderId,
        );
        let invalid_payload = RadrootsOrderEnvelopeParseError::InvalidPayload(
            RadrootsOrderPayloadError::MissingItems,
        );
        let invalid_listing_addr = RadrootsOrderEnvelopeParseError::InvalidListingAddr(
            RadrootsListingAddress::parse("not-a-listing-address").unwrap_err(),
        );
        let errors = [
            RadrootsOrderEnvelopeParseError::InvalidKind(3431),
            RadrootsOrderEnvelopeParseError::InvalidJson,
            invalid_envelope.clone(),
            invalid_payload.clone(),
            RadrootsOrderEnvelopeParseError::MessageTypeKindMismatch {
                event_kind: KIND_ORDER_REQUEST,
                message_type: RadrootsOrderEventType::OrderDecision,
            },
            RadrootsOrderEnvelopeParseError::MissingTag("a"),
            RadrootsOrderEnvelopeParseError::InvalidTag("p"),
            RadrootsOrderEnvelopeParseError::ListingAddrTagMismatch,
            RadrootsOrderEnvelopeParseError::OrderIdTagMismatch,
            RadrootsOrderEnvelopeParseError::PayloadBindingMismatch("order_id"),
            RadrootsOrderEnvelopeParseError::AuthorMismatch,
            RadrootsOrderEnvelopeParseError::CounterpartyTagMismatch,
            invalid_listing_addr.clone(),
        ];

        for error in errors {
            assert!(!error.to_string().is_empty());
        }
        assert!(invalid_envelope.source().is_some());
        assert!(invalid_payload.source().is_some());
        assert!(invalid_listing_addr.source().is_some());
        assert!(
            RadrootsOrderEnvelopeParseError::AuthorMismatch
                .source()
                .is_none()
        );
    }

    #[test]
    fn order_envelope_parse_rejects_content_tag_and_envelope_mismatches() {
        let payload = serde_json::json!({});
        let invalid_json = RadrootsNostrEvent {
            id: event_id_wire('e'),
            author: buyer_pubkey_wire(),
            created_at: 1,
            kind: KIND_ORDER_REQUEST,
            tags: Vec::new(),
            content: "{".into(),
            sig: "sig".into(),
        };
        assert_eq!(
            order_envelope_from_event::<serde_json::Value>(&invalid_json).unwrap_err(),
            RadrootsOrderEnvelopeParseError::InvalidJson
        );

        let mut invalid_version_envelope = RadrootsOrderEnvelope::new(
            RadrootsOrderEventType::OrderRequested,
            listing_addr_wire(),
            "order-1",
            &payload,
        );
        invalid_version_envelope.version = 99;
        let invalid_version = RadrootsNostrEvent {
            id: event_id_wire('e'),
            author: buyer_pubkey_wire(),
            created_at: 1,
            kind: KIND_ORDER_REQUEST,
            tags: order_request_tags(),
            content: serde_json::to_string(&invalid_version_envelope).unwrap(),
            sig: "sig".into(),
        };
        assert!(matches!(
            order_envelope_from_event::<serde_json::Value>(&invalid_version).unwrap_err(),
            RadrootsOrderEnvelopeParseError::InvalidEnvelope(
                RadrootsOrderEnvelopeError::InvalidVersion { .. }
            )
        ));

        let message_type_mismatch = order_event_with_envelope(
            KIND_ORDER_REQUEST,
            buyer_pubkey_wire(),
            RadrootsOrderEventType::OrderDecision,
            listing_addr_wire(),
            "order-1",
            &payload,
            Vec::new(),
        );
        assert_eq!(
            order_envelope_from_event::<serde_json::Value>(&message_type_mismatch).unwrap_err(),
            RadrootsOrderEnvelopeParseError::MessageTypeKindMismatch {
                event_kind: KIND_ORDER_REQUEST,
                message_type: RadrootsOrderEventType::OrderDecision
            }
        );

        let listing_addr_mismatch = order_event_with_envelope(
            KIND_ORDER_REQUEST,
            buyer_pubkey_wire(),
            RadrootsOrderEventType::OrderRequested,
            listing_addr_wire(),
            "order-1",
            &payload,
            vec![
                vec!["a".into(), "30402:pubkey:AAAAAAAAAAAAAAAAAAAAAg".into()],
                vec![TAG_D.into(), "order-1".into()],
            ],
        );
        assert_eq!(
            order_envelope_from_event::<serde_json::Value>(&listing_addr_mismatch).unwrap_err(),
            RadrootsOrderEnvelopeParseError::ListingAddrTagMismatch
        );

        let order_id_mismatch = order_event_with_envelope(
            KIND_ORDER_REQUEST,
            buyer_pubkey_wire(),
            RadrootsOrderEventType::OrderRequested,
            listing_addr_wire(),
            "order-1",
            &payload,
            vec![
                vec!["a".into(), listing_addr_wire()],
                vec![TAG_D.into(), "other-order".into()],
            ],
        );
        assert_eq!(
            order_envelope_from_event::<serde_json::Value>(&order_id_mismatch).unwrap_err(),
            RadrootsOrderEnvelopeParseError::OrderIdTagMismatch
        );

        for tags in [
            Vec::<Vec<String>>::new(),
            vec![vec!["a".into()]],
            vec![vec!["a".into(), " ".into()]],
        ] {
            let event = order_event_with_envelope(
                KIND_ORDER_REQUEST,
                buyer_pubkey_wire(),
                RadrootsOrderEventType::OrderRequested,
                listing_addr_wire(),
                "order-1",
                &payload,
                tags,
            );
            let err = order_envelope_from_event::<serde_json::Value>(&event).unwrap_err();
            assert!(matches!(
                err,
                RadrootsOrderEnvelopeParseError::MissingTag("a")
                    | RadrootsOrderEnvelopeParseError::InvalidTag("a")
            ));
        }

        let invalid_listing_addr = order_event_with_envelope(
            KIND_ORDER_REQUEST,
            buyer_pubkey_wire(),
            RadrootsOrderEventType::OrderRequested,
            "not-a-listing-address",
            "order-1",
            &payload,
            vec![
                vec!["a".into(), "not-a-listing-address".into()],
                vec![TAG_D.into(), "order-1".into()],
            ],
        );
        assert!(matches!(
            order_envelope_from_event::<serde_json::Value>(&invalid_listing_addr).unwrap_err(),
            RadrootsOrderEnvelopeParseError::InvalidListingAddr(_)
        ));
    }

    #[test]
    fn order_typed_parsers_reject_message_type_mismatches() {
        let request_payload = order_request();
        let decision_payload = order_decision();
        let proposal_payload = order_revision_proposal();
        let revision_decision_payload =
            order_revision_decision(RadrootsOrderRevisionOutcome::Accepted);
        let cancellation_payload = order_cancelled();

        let request_as_decision = order_event_with_envelope(
            KIND_ORDER_DECISION,
            buyer_pubkey_wire(),
            RadrootsOrderEventType::OrderDecision,
            listing_addr_wire(),
            "order-1",
            &request_payload,
            order_chain_tags(seller_pubkey_wire()),
        );
        assert!(matches!(
            order_request_from_event(&request_as_decision).unwrap_err(),
            RadrootsOrderEnvelopeParseError::MessageTypeKindMismatch { .. }
        ));

        let decision_as_request = order_event_with_envelope(
            KIND_ORDER_REQUEST,
            seller_pubkey_wire(),
            RadrootsOrderEventType::OrderRequested,
            listing_addr_wire(),
            "order-1",
            &decision_payload,
            order_request_tags(),
        );
        assert!(matches!(
            order_decision_from_event(&decision_as_request).unwrap_err(),
            RadrootsOrderEnvelopeParseError::MessageTypeKindMismatch { .. }
        ));

        let proposal_as_cancellation = order_event_with_envelope(
            KIND_ORDER_CANCELLATION,
            seller_pubkey_wire(),
            RadrootsOrderEventType::OrderCancelled,
            listing_addr_wire(),
            "order-1",
            &proposal_payload,
            order_chain_tags(buyer_pubkey_wire()),
        );
        assert!(matches!(
            order_revision_proposal_from_event(&proposal_as_cancellation).unwrap_err(),
            RadrootsOrderEnvelopeParseError::MessageTypeKindMismatch { .. }
        ));

        let revision_decision_as_cancellation = order_event_with_envelope(
            KIND_ORDER_CANCELLATION,
            buyer_pubkey_wire(),
            RadrootsOrderEventType::OrderCancelled,
            listing_addr_wire(),
            "order-1",
            &revision_decision_payload,
            order_chain_tags(seller_pubkey_wire()),
        );
        assert!(matches!(
            order_revision_decision_from_event(&revision_decision_as_cancellation).unwrap_err(),
            RadrootsOrderEnvelopeParseError::MessageTypeKindMismatch { .. }
        ));

        let cancellation_as_decision = order_event_with_envelope(
            KIND_ORDER_DECISION,
            buyer_pubkey_wire(),
            RadrootsOrderEventType::OrderDecision,
            listing_addr_wire(),
            "order-1",
            &cancellation_payload,
            order_chain_tags(seller_pubkey_wire()),
        );
        assert!(matches!(
            order_cancellation_from_event(&cancellation_as_decision).unwrap_err(),
            RadrootsOrderEnvelopeParseError::MessageTypeKindMismatch { .. }
        ));
    }

    #[test]
    fn order_parse_rejects_payload_and_chain_binding_mismatches() {
        let mut request_payload = order_request();
        request_payload.order_id = order_id("other-order");
        let request_built =
            order_request_event_build(&listing_event_ptr(), &order_request()).unwrap();
        let mut request_event = RadrootsNostrEvent {
            id: event_id_wire('e'),
            author: buyer_pubkey_wire(),
            created_at: 1,
            kind: request_built.kind,
            tags: request_built.tags.clone(),
            content: serde_json::to_string(&RadrootsOrderEnvelope::new(
                RadrootsOrderEventType::OrderRequested,
                listing_addr_wire(),
                "order-1",
                &request_payload,
            ))
            .unwrap(),
            sig: "sig".into(),
        };
        assert_eq!(
            order_request_from_event(&request_event).unwrap_err(),
            RadrootsOrderEnvelopeParseError::PayloadBindingMismatch("order_id")
        );

        request_payload = order_request();
        request_payload.listing_addr =
            format!("30402:{}:BBBBBBBBBBBBBBBBBBBBBA", seller_pubkey_wire())
                .parse()
                .unwrap();
        request_event.content = serde_json::to_string(&RadrootsOrderEnvelope::new(
            RadrootsOrderEventType::OrderRequested,
            listing_addr_wire(),
            "order-1",
            &request_payload,
        ))
        .unwrap();
        assert_eq!(
            order_request_from_event(&request_event).unwrap_err(),
            RadrootsOrderEnvelopeParseError::PayloadBindingMismatch("listing_addr")
        );

        let proposal_payload = order_revision_proposal();
        let proposal_built = order_revision_proposal_event_build(
            &proposal_payload.root_event_id,
            &proposal_payload.prev_event_id,
            &proposal_payload,
        )
        .unwrap();
        let mut proposal_event = RadrootsNostrEvent {
            id: event_id_wire('e'),
            author: seller_pubkey_wire(),
            created_at: 1,
            kind: proposal_built.kind,
            tags: proposal_built.tags.clone(),
            content: proposal_built.content.clone(),
            sig: "sig".into(),
        };
        proposal_event
            .tags
            .iter_mut()
            .find(|tag| tag.first().map(String::as_str) == Some(TAG_E_ROOT))
            .unwrap()[1] = event_id_wire('4');
        assert_eq!(
            order_revision_proposal_from_event(&proposal_event).unwrap_err(),
            RadrootsOrderEnvelopeParseError::PayloadBindingMismatch("root_event_id")
        );

        proposal_event.tags = proposal_built.tags;
        proposal_event
            .tags
            .iter_mut()
            .find(|tag| tag.first().map(String::as_str) == Some(TAG_E_PREV))
            .unwrap()[1] = event_id_wire('5');
        assert_eq!(
            order_revision_proposal_from_event(&proposal_event).unwrap_err(),
            RadrootsOrderEnvelopeParseError::PayloadBindingMismatch("prev_event_id")
        );

        let revision_decision_payload =
            order_revision_decision(RadrootsOrderRevisionOutcome::Accepted);
        let revision_decision_built = order_revision_decision_event_build(
            &revision_decision_payload.root_event_id,
            &revision_decision_payload.prev_event_id,
            &revision_decision_payload,
        )
        .unwrap();
        let mut revision_decision_event = RadrootsNostrEvent {
            id: event_id_wire('e'),
            author: buyer_pubkey_wire(),
            created_at: 1,
            kind: revision_decision_built.kind,
            tags: revision_decision_built.tags.clone(),
            content: revision_decision_built.content,
            sig: "sig".into(),
        };
        revision_decision_event
            .tags
            .iter_mut()
            .find(|tag| tag.first().map(String::as_str) == Some(TAG_E_ROOT))
            .unwrap()[1] = event_id_wire('6');
        assert_eq!(
            order_revision_decision_from_event(&revision_decision_event).unwrap_err(),
            RadrootsOrderEnvelopeParseError::PayloadBindingMismatch("root_event_id")
        );

        revision_decision_event.tags = revision_decision_built.tags;
        revision_decision_event
            .tags
            .iter_mut()
            .find(|tag| tag.first().map(String::as_str) == Some(TAG_E_PREV))
            .unwrap()[1] = event_id_wire('7');
        assert_eq!(
            order_revision_decision_from_event(&revision_decision_event).unwrap_err(),
            RadrootsOrderEnvelopeParseError::PayloadBindingMismatch("prev_event_id")
        );
    }

    #[test]
    fn order_event_context_and_parse_error_mapping_cover_missing_context() {
        let err = order_event_context_from_tags(
            RadrootsOrderEventType::OrderRequested,
            &[vec!["p".into(), seller_pubkey_wire()]],
        )
        .unwrap_err();
        assert_eq!(
            err,
            RadrootsOrderEnvelopeParseError::MissingTag(TAG_LISTING_EVENT)
        );

        let err = order_event_context_from_tags(
            RadrootsOrderEventType::OrderDecision,
            &[
                vec!["p".into(), buyer_pubkey_wire()],
                vec![TAG_E_PREV.into(), event_id_wire('2')],
            ],
        )
        .unwrap_err();
        assert_eq!(err, RadrootsOrderEnvelopeParseError::MissingTag(TAG_E_ROOT));

        let err = order_event_context_from_tags(
            RadrootsOrderEventType::OrderDecision,
            &[
                vec!["p".into(), buyer_pubkey_wire()],
                vec![TAG_E_ROOT.into(), event_id_wire('1')],
                vec![TAG_E_PREV.into(), "not-an-event-id".into()],
            ],
        )
        .unwrap_err();
        assert_eq!(err, RadrootsOrderEnvelopeParseError::InvalidTag(TAG_E_PREV));

        let invalid_number = "x".parse::<u32>().unwrap_err();
        assert_eq!(
            map_tag_parse_error_for_order_envelope(crate::error::EventParseError::MissingTag("p")),
            RadrootsOrderEnvelopeParseError::MissingTag("p")
        );
        assert_eq!(
            map_tag_parse_error_for_order_envelope(crate::error::EventParseError::InvalidTag("p")),
            RadrootsOrderEnvelopeParseError::InvalidTag("p")
        );
        assert_eq!(
            map_tag_parse_error_for_order_envelope(crate::error::EventParseError::InvalidKind {
                expected: "1",
                got: 2,
            }),
            RadrootsOrderEnvelopeParseError::InvalidKind(2)
        );
        assert_eq!(
            map_tag_parse_error_for_order_envelope(crate::error::EventParseError::InvalidNumber(
                "n",
                invalid_number,
            )),
            RadrootsOrderEnvelopeParseError::InvalidTag("n")
        );
        assert_eq!(
            map_tag_parse_error_for_order_envelope(crate::error::EventParseError::InvalidJson(
                "json",
            )),
            RadrootsOrderEnvelopeParseError::InvalidTag("json")
        );
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
                id: event_id_wire('e'),
                author: seller_pubkey_wire(),
                created_at: 1,
                kind,
                tags: vec![
                    vec!["p".into(), buyer_pubkey_wire()],
                    vec!["a".into(), listing_addr_wire()],
                    vec![TAG_D.into(), "order-1".into()],
                    vec![TAG_E_ROOT.into(), event_id_wire('1')],
                    vec![TAG_E_PREV.into(), event_id_wire('9')],
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
            id: event_id_wire('e'),
            author: seller_pubkey_wire(),
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
        let root_event_id = event_id('1');
        let prev_event_id = event_id('9');
        let built = order_decision_event_build(&root_event_id, &prev_event_id, &payload).unwrap();
        let mut event = RadrootsNostrEvent {
            id: event_id_wire('e'),
            author: seller_pubkey_wire(),
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
            id: event_id_wire('e'),
            author: seller_pubkey_wire(),
            created_at: 1,
            kind: built.kind,
            tags: built.tags.clone(),
            content: built.content.clone(),
            sig: "sig".into(),
        };
        let err = order_request_from_event(&event).unwrap_err();
        assert_eq!(err, RadrootsOrderEnvelopeParseError::AuthorMismatch);

        event.author = buyer_pubkey_wire();
        event.tags[0] = vec!["p".into(), pubkey('c').into_string()];
        let err = order_request_from_event(&event).unwrap_err();
        assert_eq!(
            err,
            RadrootsOrderEnvelopeParseError::CounterpartyTagMismatch
        );
    }

    #[test]
    fn order_cancellation_parse_rejects_wrong_actor() {
        let cancellation = order_cancelled();
        let root_event_id = event_id('1');
        let prev_event_id = event_id('9');
        let cancellation_parts =
            order_cancellation_event_build(&root_event_id, &prev_event_id, &cancellation).unwrap();
        let cancellation_event = RadrootsNostrEvent {
            id: event_id_wire('e'),
            author: seller_pubkey_wire(),
            created_at: 1,
            kind: cancellation_parts.kind,
            tags: cancellation_parts.tags,
            content: cancellation_parts.content,
            sig: "sig".into(),
        };
        let err = order_cancellation_from_event(&cancellation_event).unwrap_err();
        assert_eq!(err, RadrootsOrderEnvelopeParseError::AuthorMismatch);
    }

    #[test]
    fn order_parse_rejects_invalid_protocol_tag_values() {
        let payload = order_decision();
        let root_event_id = event_id('1');
        let prev_event_id = event_id('9');
        let built = order_decision_event_build(&root_event_id, &prev_event_id, &payload).unwrap();
        let mut event = RadrootsNostrEvent {
            id: event_id_wire('e'),
            author: seller_pubkey_wire(),
            created_at: 1,
            kind: built.kind,
            tags: built.tags,
            content: built.content,
            sig: "sig".into(),
        };

        event.tags[0] = vec!["p".into(), "not-a-pubkey".into()];
        let err = order_decision_from_event(&event).unwrap_err();
        assert_eq!(err, RadrootsOrderEnvelopeParseError::InvalidTag("p"));

        event.tags[0] = vec!["p".into(), buyer_pubkey_wire()];
        let root_tag = event
            .tags
            .iter_mut()
            .find(|tag| tag.first().map(String::as_str) == Some(TAG_E_ROOT))
            .unwrap();
        root_tag[1] = "not-an-event-id".into();
        let err = order_decision_from_event(&event).unwrap_err();
        assert_eq!(err, RadrootsOrderEnvelopeParseError::InvalidTag(TAG_E_ROOT));
    }
}
