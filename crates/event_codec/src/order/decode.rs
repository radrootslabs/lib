#[cfg(all(not(feature = "std"), feature = "json"))]
use alloc::{string::String, vec::Vec};

#[cfg(feature = "json")]
use radroots_event::{
    envelope::EventEnvelope,
    envelope::kind::is_order_event_kind,
    id::{ClassifiedListingAddress, EventId, ParseError},
    tag::EventPtr,
    tag::name::{TAG_D, TAG_E_PREV, TAG_E_ROOT},
    trade::order::{
        OrderCancellation, OrderDecision, OrderEnvelope, OrderEnvelopeError, OrderEventType,
        OrderPayloadError, OrderRequest,
    },
};
#[cfg(feature = "json")]
use radroots_identity::PublicKey;
#[cfg(feature = "json")]
use serde::de::DeserializeOwned;

#[cfg(feature = "json")]
use crate::order::tags::{
    TAG_LISTING_EVENT, parse_order_counterparty_tag, parse_order_listing_event_tag,
    parse_order_prev_tag, parse_order_root_tag,
};

#[cfg(feature = "json")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsOrderEnvelopeParseError {
    InvalidKind(u32),
    InvalidJson,
    InvalidEnvelope(OrderEnvelopeError),
    InvalidPayload(OrderPayloadError),
    MessageTypeKindMismatch {
        event_kind: u32,
        message_type: OrderEventType,
    },
    MissingTag(&'static str),
    InvalidTag(&'static str),
    ListingAddrTagMismatch,
    OrderIdTagMismatch,
    PayloadBindingMismatch(&'static str),
    AuthorMismatch,
    CounterpartyTagMismatch,
    InvalidListingAddr(ParseError),
}

#[cfg(feature = "json")]
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

#[cfg(all(feature = "std", feature = "json"))]
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

#[cfg(feature = "json")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOrderEventContext {
    pub counterparty_pubkey: PublicKey,
    pub listing_event: Option<EventPtr>,
    pub root_event_id: Option<EventId>,
    pub prev_event_id: Option<EventId>,
}

#[cfg(feature = "json")]
pub fn order_envelope_from_event<T: DeserializeOwned>(
    event: &EventEnvelope,
) -> Result<OrderEnvelope<T>, RadrootsOrderEnvelopeParseError> {
    let event_kind = event.kind_u32();
    let event_tags = event.tags_as_vec();
    if !is_order_event_kind(event_kind) {
        return Err(RadrootsOrderEnvelopeParseError::InvalidKind(event_kind));
    }
    let envelope = serde_json::from_str::<OrderEnvelope<T>>(event.content())
        .map_err(|_| RadrootsOrderEnvelopeParseError::InvalidJson)?;
    envelope
        .validate()
        .map_err(RadrootsOrderEnvelopeParseError::InvalidEnvelope)?;
    if envelope.message_type.kind() != event_kind {
        return Err(RadrootsOrderEnvelopeParseError::MessageTypeKindMismatch {
            event_kind,
            message_type: envelope.message_type,
        });
    }

    let listing_addr = required_order_tag_value(&event_tags, "a")?;
    if envelope.listing_addr != listing_addr {
        return Err(RadrootsOrderEnvelopeParseError::ListingAddrTagMismatch);
    }
    ClassifiedListingAddress::parse(&envelope.listing_addr)
        .map_err(RadrootsOrderEnvelopeParseError::InvalidListingAddr)?;

    let tag_order_id = required_order_tag_value(&event_tags, TAG_D)?;
    if tag_order_id != envelope.order_id {
        return Err(RadrootsOrderEnvelopeParseError::OrderIdTagMismatch);
    }

    order_event_context_from_tags(envelope.message_type, &event_tags)?;
    Ok(envelope)
}

#[cfg(feature = "json")]
pub fn order_request_from_event(
    event: &EventEnvelope,
) -> Result<OrderEnvelope<OrderRequest>, RadrootsOrderEnvelopeParseError> {
    let envelope = order_envelope_from_event::<OrderRequest>(event)?;
    if envelope.message_type != OrderEventType::OrderRequested {
        return Err(RadrootsOrderEnvelopeParseError::MessageTypeKindMismatch {
            event_kind: event.kind_u32(),
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
        envelope.payload.order_id.as_str(),
        envelope.payload.listing_addr.as_str(),
        &envelope.payload.buyer_pubkey,
        &envelope.payload.seller_pubkey,
    )?;
    Ok(envelope)
}

#[cfg(feature = "json")]
pub fn order_decision_from_event(
    event: &EventEnvelope,
) -> Result<OrderEnvelope<OrderDecision>, RadrootsOrderEnvelopeParseError> {
    let envelope = order_envelope_from_event::<OrderDecision>(event)?;
    if envelope.message_type != OrderEventType::OrderDecision {
        return Err(RadrootsOrderEnvelopeParseError::MessageTypeKindMismatch {
            event_kind: event.kind_u32(),
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
        envelope.payload.order_id.as_str(),
        envelope.payload.listing_addr.as_str(),
        &envelope.payload.seller_pubkey,
        &envelope.payload.buyer_pubkey,
    )?;
    Ok(envelope)
}

#[cfg(feature = "json")]
pub fn order_cancellation_from_event(
    event: &EventEnvelope,
) -> Result<OrderEnvelope<OrderCancellation>, RadrootsOrderEnvelopeParseError> {
    let envelope = order_envelope_from_event::<OrderCancellation>(event)?;
    if envelope.message_type != OrderEventType::OrderCancelled {
        return Err(RadrootsOrderEnvelopeParseError::MessageTypeKindMismatch {
            event_kind: event.kind_u32(),
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
        envelope.payload.order_id.as_str(),
        envelope.payload.listing_addr.as_str(),
        &envelope.payload.buyer_pubkey,
        &envelope.payload.seller_pubkey,
    )?;
    Ok(envelope)
}

#[cfg(feature = "json")]
pub fn order_event_context_from_tags(
    message_type: OrderEventType,
    tags: &[Vec<String>],
) -> Result<RadrootsOrderEventContext, RadrootsOrderEnvelopeParseError> {
    let counterparty_pubkey =
        parse_order_counterparty_tag(tags).map_err(map_tag_parse_error_for_order_envelope)?;
    let counterparty_pubkey = PublicKey::from_hex(&counterparty_pubkey)
        .map_err(|_| RadrootsOrderEnvelopeParseError::InvalidTag("p"))?;
    let listing_event =
        parse_order_listing_event_tag(tags).map_err(map_tag_parse_error_for_order_envelope)?;
    let root_event_id =
        parse_order_root_tag(tags).map_err(map_tag_parse_error_for_order_envelope)?;
    let root_event_id = root_event_id
        .map(|id| {
            EventId::parse(id).map_err(|_| RadrootsOrderEnvelopeParseError::InvalidTag(TAG_E_ROOT))
        })
        .transpose()?;
    let prev_event_id =
        parse_order_prev_tag(tags).map_err(map_tag_parse_error_for_order_envelope)?;
    let prev_event_id = prev_event_id
        .map(|id| {
            EventId::parse(id).map_err(|_| RadrootsOrderEnvelopeParseError::InvalidTag(TAG_E_PREV))
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

#[cfg(feature = "json")]
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

#[cfg(feature = "json")]
fn map_tag_parse_error_for_order_envelope(
    error: crate::error::EventParseError,
) -> RadrootsOrderEnvelopeParseError {
    match error {
        crate::error::EventParseError::MissingTag(tag) => {
            RadrootsOrderEnvelopeParseError::MissingTag(tag)
        }
        crate::error::EventParseError::InvalidTag(tag)
        | crate::error::EventParseError::DuplicateTag(tag) => {
            RadrootsOrderEnvelopeParseError::InvalidTag(tag)
        }
        crate::error::EventParseError::InvalidKind { expected: _, got } => {
            RadrootsOrderEnvelopeParseError::InvalidKind(got)
        }
        crate::error::EventParseError::InvalidNumber(tag, _)
        | crate::error::EventParseError::InvalidJson(tag) => {
            RadrootsOrderEnvelopeParseError::InvalidTag(tag)
        }
        crate::error::EventParseError::InvalidEnvelope => {
            RadrootsOrderEnvelopeParseError::InvalidTag("event_envelope")
        }
    }
}

#[cfg(feature = "json")]
fn validate_order_binding<T>(
    event: &EventEnvelope,
    envelope: &OrderEnvelope<T>,
    payload_order_id: &str,
    payload_listing_addr: &str,
    expected_author: &PublicKey,
    expected_counterparty: &PublicKey,
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
    if event.author() != expected_author {
        return Err(RadrootsOrderEnvelopeParseError::AuthorMismatch);
    }
    let context = order_event_context_from_tags(envelope.message_type, &event.tags_as_vec())?;
    if &context.counterparty_pubkey != expected_counterparty {
        return Err(RadrootsOrderEnvelopeParseError::CounterpartyTagMismatch);
    }
    Ok(())
}

#[cfg(all(test, feature = "json"))]
mod tests {
    use super::{
        RadrootsOrderEnvelopeParseError, map_tag_parse_error_for_order_envelope,
        order_cancellation_from_event, order_decision_from_event, order_envelope_from_event,
        order_event_context_from_tags, order_request_from_event,
    };
    use crate::order::encode::{
        order_cancellation_event_build, order_decision_event_build, order_request_event_build,
    };
    use crate::order::tags::TAG_LISTING_EVENT;
    use radroots_core::{Currency, Decimal, Money, Unit};
    use radroots_event::{
        envelope::EventEnvelope,
        envelope::EventEnvelopeParts,
        envelope::kind::{KIND_ORDER_CANCELLATION, KIND_ORDER_DECISION, KIND_ORDER_REQUEST},
        id::{ClassifiedListingAddress, EventId, InventoryBinId, OrderId, OrderQuoteId},
        tag::EventPtr,
        tag::name::{TAG_D, TAG_E_PREV, TAG_E_ROOT},
        trade::order::{
            OrderCancellation, OrderDecision, OrderDecisionOutcome, OrderEconomicItem,
            OrderEconomicLine, OrderEconomics, OrderEnvelope, OrderEnvelopeError, OrderEventType,
            OrderInventoryCommitment, OrderItem, OrderPayloadError, OrderPricingBasis,
            OrderRequest,
        },
    };
    use radroots_identity::PublicKey;

    fn pubkey(character: char) -> PublicKey {
        crate::test_fixtures::fixture_public_key_hex(character)
            .parse()
            .unwrap()
    }

    fn buyer_pubkey() -> PublicKey {
        pubkey('b')
    }

    fn seller_pubkey() -> PublicKey {
        pubkey('a')
    }

    fn buyer_pubkey_wire() -> String {
        buyer_pubkey().to_hex()
    }

    fn seller_pubkey_wire() -> String {
        seller_pubkey().to_hex()
    }

    fn listing_addr() -> ClassifiedListingAddress {
        format!("30402:{}:AAAAAAAAAAAAAAAAAAAAAg", seller_pubkey_wire())
            .parse()
            .unwrap()
    }

    fn listing_addr_wire() -> String {
        listing_addr().into_string()
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

    fn event_id(character: char) -> EventId {
        core::iter::repeat_n(character, 64)
            .collect::<String>()
            .parse()
            .unwrap()
    }

    fn event_id_wire(character: char) -> String {
        event_id(character).into_string()
    }

    fn event_signature_wire() -> String {
        core::iter::repeat_n('f', 128).collect()
    }

    fn event_envelope(
        author: String,
        kind: u32,
        tags: Vec<Vec<String>>,
        content: String,
    ) -> EventEnvelope {
        EventEnvelope::new(EventEnvelopeParts {
            id: event_id_wire('e'),
            author,
            created_at: 1,
            kind,
            tags,
            content,
            sig: event_signature_wire(),
        })
        .unwrap()
    }

    fn order_request() -> OrderRequest {
        OrderRequest {
            order_id: order_id("order-1"),
            listing_addr: listing_addr(),
            buyer_pubkey: buyer_pubkey(),
            seller_pubkey: seller_pubkey(),
            items: vec![OrderItem {
                bin_id: bin_id("lb"),
                bin_count: 3,
            }],
            economics: request_economics(),
        }
    }

    fn decimal(raw: &str) -> Decimal {
        raw.parse().unwrap()
    }

    fn usd(raw: &str) -> Money {
        Money::try_new(decimal(raw), Currency::USD).unwrap()
    }

    fn request_economics() -> OrderEconomics {
        OrderEconomics {
            quote_id: quote_id("quote-1"),
            quote_version: 1,
            pricing_basis: OrderPricingBasis::ListingEvent,
            currency: Currency::USD,
            items: vec![OrderEconomicItem {
                bin_id: bin_id("lb"),
                bin_count: 3,
                quantity_amount: decimal("1"),
                quantity_unit: Unit::Each,
                unit_price_amount: decimal("5"),
                unit_price_currency: Currency::USD,
                line_subtotal: usd("15"),
            }],
            discounts: Vec::<OrderEconomicLine>::new(),
            adjustments: Vec::<OrderEconomicLine>::new(),
            subtotal: usd("15"),
            discount_total: usd("0"),
            adjustment_total: usd("0"),
            total: usd("15"),
        }
    }

    fn order_decision() -> OrderDecision {
        OrderDecision {
            order_id: order_id("order-1"),
            listing_addr: listing_addr(),
            buyer_pubkey: buyer_pubkey(),
            seller_pubkey: seller_pubkey(),
            decision: OrderDecisionOutcome::Accepted {
                inventory_commitments: vec![OrderInventoryCommitment {
                    bin_id: bin_id("lb"),
                    bin_count: 3,
                }],
            },
        }
    }

    fn order_cancelled() -> OrderCancellation {
        OrderCancellation {
            order_id: order_id("order-1"),
            listing_addr: listing_addr(),
            buyer_pubkey: buyer_pubkey(),
            seller_pubkey: seller_pubkey(),
            reason: "changed plans".into(),
        }
    }

    fn listing_event_ptr() -> EventPtr {
        EventPtr {
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
        message_type: OrderEventType,
        listing_addr: impl Into<String>,
        order_id: impl Into<String>,
        payload: &T,
        tags: Vec<Vec<String>>,
    ) -> EventEnvelope {
        let envelope = OrderEnvelope::new(message_type, listing_addr, order_id, payload);
        event_envelope(
            author,
            kind,
            tags,
            serde_json::to_string(&envelope).unwrap(),
        )
    }

    #[test]
    fn listing_address_roundtrips() {
        let raw = format!("30402:{}:listing-1", seller_pubkey_wire());
        let addr = ClassifiedListingAddress::parse(&raw).expect("parse listing address");
        assert_eq!(addr.as_str(), raw);
    }

    #[test]
    fn order_request_builder_emits_canonical_shape() {
        let payload = order_request();
        let built = order_request_event_build(&listing_event_ptr(), &payload).unwrap();
        let envelope: OrderEnvelope<OrderRequest> = serde_json::from_str(&built.content).unwrap();

        assert_eq!(built.kind, KIND_ORDER_REQUEST);
        assert_eq!(envelope.message_type, OrderEventType::OrderRequested);
        assert_eq!(envelope.order_id, "order-1");
        assert_eq!(built.tags[0], vec!["p".to_string(), seller_pubkey_wire()]);
        assert_eq!(built.tags[1], vec!["a".to_string(), listing_addr_wire()]);
        assert_eq!(
            built.tags[2],
            vec![TAG_D.to_string(), "order-1".to_string()]
        );
        assert_eq!(envelope.payload.economics.quote_id.as_str(), "quote-1");
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
        let envelope: OrderEnvelope<OrderDecision> = serde_json::from_str(&built.content).unwrap();

        assert_eq!(built.kind, KIND_ORDER_DECISION);
        assert_eq!(envelope.message_type, OrderEventType::OrderDecision);
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
    fn order_cancellation_builder_emits_canonical_buyer_chain_shape() {
        let payload = order_cancelled();
        let root_event_id = event_id('1');
        let prev_event_id = event_id('9');
        let built =
            order_cancellation_event_build(&root_event_id, &prev_event_id, &payload).unwrap();
        let envelope: OrderEnvelope<OrderCancellation> =
            serde_json::from_str(&built.content).unwrap();

        assert_eq!(built.kind, KIND_ORDER_CANCELLATION);
        assert_eq!(envelope.message_type, OrderEventType::OrderCancelled);
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
        let event = event_envelope(buyer_pubkey_wire(), built.kind, built.tags, built.content);
        let envelope = order_request_from_event(&event).unwrap();

        assert_eq!(envelope.payload, payload);
        assert_eq!(envelope.message_type, OrderEventType::OrderRequested);
    }

    #[test]
    fn order_request_parse_rejects_mismatched_economics() {
        let mut payload = order_request();
        let built = order_request_event_build(&listing_event_ptr(), &payload).unwrap();
        payload.economics.items[0].bin_id = bin_id("other-bin");
        let envelope = OrderEnvelope::new(
            OrderEventType::OrderRequested,
            payload.listing_addr.clone(),
            payload.order_id.clone(),
            payload,
        );
        let event = event_envelope(
            buyer_pubkey_wire(),
            built.kind,
            built.tags,
            serde_json::to_string(&envelope).unwrap(),
        );
        let err = order_request_from_event(&event).unwrap_err();
        assert_eq!(
            err,
            RadrootsOrderEnvelopeParseError::InvalidPayload(
                OrderPayloadError::InvalidOrderEconomicsBinding {
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
        let event = event_envelope(seller_pubkey_wire(), built.kind, built.tags, built.content);
        let envelope = order_decision_from_event(&event).unwrap();

        assert_eq!(envelope.payload, payload);
        assert_eq!(envelope.message_type, OrderEventType::OrderDecision);
    }

    #[test]
    fn order_cancellation_parse_roundtrips_and_validates_buyer_actor() {
        let payload = order_cancelled();
        let root_event_id = event_id('1');
        let prev_event_id = event_id('9');
        let built =
            order_cancellation_event_build(&root_event_id, &prev_event_id, &payload).unwrap();
        let event = event_envelope(buyer_pubkey_wire(), built.kind, built.tags, built.content);
        let envelope = order_cancellation_from_event(&event).unwrap();

        assert_eq!(envelope.payload, payload);
        assert_eq!(envelope.message_type, OrderEventType::OrderCancelled);
    }

    #[cfg(feature = "std")]
    #[test]
    fn order_parse_error_display_and_source_cover_variants() {
        use std::error::Error as _;

        let invalid_envelope =
            RadrootsOrderEnvelopeParseError::InvalidEnvelope(OrderEnvelopeError::MissingOrderId);
        let invalid_payload =
            RadrootsOrderEnvelopeParseError::InvalidPayload(OrderPayloadError::MissingItems);
        let invalid_listing_addr = RadrootsOrderEnvelopeParseError::InvalidListingAddr(
            ClassifiedListingAddress::parse("not-a-listing-address").unwrap_err(),
        );
        let errors = [
            RadrootsOrderEnvelopeParseError::InvalidKind(3431),
            RadrootsOrderEnvelopeParseError::InvalidJson,
            invalid_envelope.clone(),
            invalid_payload.clone(),
            RadrootsOrderEnvelopeParseError::MessageTypeKindMismatch {
                event_kind: KIND_ORDER_REQUEST,
                message_type: OrderEventType::OrderDecision,
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
        let invalid_json = event_envelope(
            buyer_pubkey_wire(),
            KIND_ORDER_REQUEST,
            Vec::new(),
            "{".into(),
        );
        assert_eq!(
            order_envelope_from_event::<serde_json::Value>(&invalid_json).unwrap_err(),
            RadrootsOrderEnvelopeParseError::InvalidJson
        );

        let mut invalid_version_envelope = OrderEnvelope::new(
            OrderEventType::OrderRequested,
            listing_addr_wire(),
            "order-1",
            &payload,
        );
        invalid_version_envelope.version = 99;
        let invalid_version = event_envelope(
            buyer_pubkey_wire(),
            KIND_ORDER_REQUEST,
            order_request_tags(),
            serde_json::to_string(&invalid_version_envelope).unwrap(),
        );
        assert!(matches!(
            order_envelope_from_event::<serde_json::Value>(&invalid_version).unwrap_err(),
            RadrootsOrderEnvelopeParseError::InvalidEnvelope(
                OrderEnvelopeError::InvalidVersion { .. }
            )
        ));

        let message_type_mismatch = order_event_with_envelope(
            KIND_ORDER_REQUEST,
            buyer_pubkey_wire(),
            OrderEventType::OrderDecision,
            listing_addr_wire(),
            "order-1",
            &payload,
            Vec::new(),
        );
        assert_eq!(
            order_envelope_from_event::<serde_json::Value>(&message_type_mismatch).unwrap_err(),
            RadrootsOrderEnvelopeParseError::MessageTypeKindMismatch {
                event_kind: KIND_ORDER_REQUEST,
                message_type: OrderEventType::OrderDecision
            }
        );

        let listing_addr_mismatch = order_event_with_envelope(
            KIND_ORDER_REQUEST,
            buyer_pubkey_wire(),
            OrderEventType::OrderRequested,
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
            OrderEventType::OrderRequested,
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
                OrderEventType::OrderRequested,
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
            OrderEventType::OrderRequested,
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
        let cancellation_payload = order_cancelled();

        let request_as_decision = order_event_with_envelope(
            KIND_ORDER_DECISION,
            buyer_pubkey_wire(),
            OrderEventType::OrderDecision,
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
            OrderEventType::OrderRequested,
            listing_addr_wire(),
            "order-1",
            &decision_payload,
            order_request_tags(),
        );
        assert!(matches!(
            order_decision_from_event(&decision_as_request).unwrap_err(),
            RadrootsOrderEnvelopeParseError::MessageTypeKindMismatch { .. }
        ));

        let cancellation_as_decision = order_event_with_envelope(
            KIND_ORDER_DECISION,
            buyer_pubkey_wire(),
            OrderEventType::OrderDecision,
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
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn order_parse_rejects_payload_and_chain_binding_mismatches() {
        let mut request_payload = order_request();
        request_payload.order_id = order_id("other-order");
        let request_built =
            order_request_event_build(&listing_event_ptr(), &order_request()).unwrap();
        let request_event = event_envelope(
            buyer_pubkey_wire(),
            request_built.kind,
            request_built.tags.clone(),
            serde_json::to_string(&OrderEnvelope::new(
                OrderEventType::OrderRequested,
                listing_addr_wire(),
                "order-1",
                &request_payload,
            ))
            .unwrap(),
        );
        assert_eq!(
            order_request_from_event(&request_event).unwrap_err(),
            RadrootsOrderEnvelopeParseError::PayloadBindingMismatch("order_id")
        );

        request_payload = order_request();
        request_payload.listing_addr =
            format!("30402:{}:BBBBBBBBBBBBBBBBBBBBBA", seller_pubkey_wire())
                .parse()
                .unwrap();
        let request_event = event_envelope(
            buyer_pubkey_wire(),
            request_built.kind,
            request_built.tags,
            serde_json::to_string(&OrderEnvelope::new(
                OrderEventType::OrderRequested,
                listing_addr_wire(),
                "order-1",
                &request_payload,
            ))
            .unwrap(),
        );
        assert_eq!(
            order_request_from_event(&request_event).unwrap_err(),
            RadrootsOrderEnvelopeParseError::PayloadBindingMismatch("listing_addr")
        );

        let mut decision_payload = order_decision();
        decision_payload.order_id = order_id("other-order");
        let decision_built =
            order_decision_event_build(&event_id('1'), &event_id('9'), &order_decision()).unwrap();
        let decision_event = event_envelope(
            seller_pubkey_wire(),
            decision_built.kind,
            decision_built.tags,
            serde_json::to_string(&OrderEnvelope::new(
                OrderEventType::OrderDecision,
                listing_addr_wire(),
                "order-1",
                &decision_payload,
            ))
            .unwrap(),
        );
        assert_eq!(
            order_decision_from_event(&decision_event).unwrap_err(),
            RadrootsOrderEnvelopeParseError::PayloadBindingMismatch("order_id")
        );
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn order_event_context_and_parse_error_mapping_cover_missing_context() {
        let err = order_event_context_from_tags(
            OrderEventType::OrderRequested,
            &[vec!["p".into(), seller_pubkey_wire()]],
        )
        .unwrap_err();
        assert_eq!(
            err,
            RadrootsOrderEnvelopeParseError::MissingTag(TAG_LISTING_EVENT)
        );

        let err = order_event_context_from_tags(
            OrderEventType::OrderDecision,
            &[
                vec!["p".into(), buyer_pubkey_wire()],
                vec![TAG_E_PREV.into(), event_id_wire('2')],
            ],
        )
        .unwrap_err();
        assert_eq!(err, RadrootsOrderEnvelopeParseError::MissingTag(TAG_E_ROOT));

        let err = order_event_context_from_tags(
            OrderEventType::OrderDecision,
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
            map_tag_parse_error_for_order_envelope(crate::error::EventParseError::DuplicateTag(
                "p",
            )),
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
        assert_eq!(
            map_tag_parse_error_for_order_envelope(crate::error::EventParseError::InvalidEnvelope),
            RadrootsOrderEnvelopeParseError::InvalidTag("event_envelope")
        );
    }

    #[test]
    fn order_parse_rejects_forbidden_kind() {
        let event = event_envelope(seller_pubkey_wire(), 3431, Vec::new(), "{}".into());
        let err = order_envelope_from_event::<serde_json::Value>(&event).unwrap_err();
        assert_eq!(err, RadrootsOrderEnvelopeParseError::InvalidKind(3431));
    }

    #[test]
    fn order_parse_rejects_missing_required_refs() {
        let payload = order_decision();
        let root_event_id = event_id('1');
        let prev_event_id = event_id('9');
        let built = order_decision_event_build(&root_event_id, &prev_event_id, &payload).unwrap();
        let mut tags = built.tags;
        tags.retain(|tag| tag.first().map(String::as_str) != Some(TAG_E_PREV));
        let event = event_envelope(seller_pubkey_wire(), built.kind, tags, built.content);

        let err = order_decision_from_event(&event).unwrap_err();
        assert_eq!(err, RadrootsOrderEnvelopeParseError::MissingTag(TAG_E_PREV));
    }

    #[test]
    fn order_parse_rejects_author_and_counterparty_mismatch() {
        let payload = order_request();
        let built = order_request_event_build(&listing_event_ptr(), &payload).unwrap();
        let event = event_envelope(
            seller_pubkey_wire(),
            built.kind,
            built.tags.clone(),
            built.content.clone(),
        );
        let err = order_request_from_event(&event).unwrap_err();
        assert_eq!(err, RadrootsOrderEnvelopeParseError::AuthorMismatch);

        let mut tags = built.tags;
        tags[0] = vec!["p".into(), pubkey('c').to_hex()];
        let counterparty_mismatch =
            event_envelope(buyer_pubkey_wire(), built.kind, tags, built.content);
        let err = order_request_from_event(&counterparty_mismatch).unwrap_err();
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
        let cancellation_event = event_envelope(
            seller_pubkey_wire(),
            cancellation_parts.kind,
            cancellation_parts.tags,
            cancellation_parts.content,
        );
        let err = order_cancellation_from_event(&cancellation_event).unwrap_err();
        assert_eq!(err, RadrootsOrderEnvelopeParseError::AuthorMismatch);
    }

    #[test]
    fn order_parse_rejects_invalid_protocol_tag_values() {
        let payload = order_decision();
        let root_event_id = event_id('1');
        let prev_event_id = event_id('9');
        let built = order_decision_event_build(&root_event_id, &prev_event_id, &payload).unwrap();
        let mut tags = built.tags.clone();
        tags[0] = vec!["p".into(), "not-a-pubkey".into()];
        let invalid_counterparty = event_envelope(
            seller_pubkey_wire(),
            built.kind,
            tags,
            built.content.clone(),
        );
        let err = order_decision_from_event(&invalid_counterparty).unwrap_err();
        assert_eq!(err, RadrootsOrderEnvelopeParseError::InvalidTag("p"));

        let mut tags = built.tags;
        tags[0] = vec!["p".into(), buyer_pubkey_wire()];
        let root_tag = tags
            .iter_mut()
            .find(|tag| tag.first().map(String::as_str) == Some(TAG_E_ROOT))
            .unwrap();
        root_tag[1] = "not-an-event-id".into();
        let invalid_root = event_envelope(seller_pubkey_wire(), built.kind, tags, built.content);
        let err = order_decision_from_event(&invalid_root).unwrap_err();
        assert_eq!(err, RadrootsOrderEnvelopeParseError::InvalidTag(TAG_E_ROOT));
    }
}
