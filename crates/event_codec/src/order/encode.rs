#[cfg(feature = "serde_json")]
use radroots_event::{
    id::RadrootsEventId,
    tag::RadrootsEventPtr,
    trade::order::{
        RadrootsOrderCancellation, RadrootsOrderDecision, RadrootsOrderEnvelope,
        RadrootsOrderEnvelopeError, RadrootsOrderEventType, RadrootsOrderPayloadError,
        RadrootsOrderRequest,
    },
};

#[cfg(feature = "serde_json")]
use crate::{error::EventEncodeError, order::tags::order_envelope_tags};
#[cfg(feature = "serde_json")]
use radroots_event::wire::RadrootsNip01EventWireParts;
#[cfg(feature = "serde_json")]
use radroots_identity::PublicKey;

#[cfg(feature = "serde_json")]
fn map_order_envelope_error(error: RadrootsOrderEnvelopeError) -> EventEncodeError {
    match error {
        RadrootsOrderEnvelopeError::MissingOrderId => {
            EventEncodeError::EmptyRequiredField("order_id")
        }
        RadrootsOrderEnvelopeError::MissingListingAddr => {
            EventEncodeError::EmptyRequiredField("listing_addr")
        }
        RadrootsOrderEnvelopeError::InvalidVersion { .. } => {
            EventEncodeError::InvalidField("version")
        }
    }
}

#[cfg(feature = "serde_json")]
fn map_order_payload_error(error: RadrootsOrderPayloadError) -> EventEncodeError {
    match error {
        RadrootsOrderPayloadError::EmptyField(field) => EventEncodeError::EmptyRequiredField(field),
        RadrootsOrderPayloadError::MissingItems => EventEncodeError::EmptyRequiredField("items"),
        RadrootsOrderPayloadError::InvalidItemBinCount { .. } => {
            EventEncodeError::InvalidField("items.bin_count")
        }
        RadrootsOrderPayloadError::MissingEconomicItems => {
            EventEncodeError::EmptyRequiredField("economics.items")
        }
        RadrootsOrderPayloadError::InvalidEconomicItemBinCount { .. } => {
            EventEncodeError::InvalidField("economics.items.bin_count")
        }
        RadrootsOrderPayloadError::InvalidEconomicItemQuantity { .. } => {
            EventEncodeError::InvalidField("economics.items.quantity_amount")
        }
        RadrootsOrderPayloadError::InvalidEconomicItemPrice { .. } => {
            EventEncodeError::InvalidField("economics.items.unit_price_amount")
        }
        RadrootsOrderPayloadError::InvalidEconomicItemSubtotal { .. } => {
            EventEncodeError::InvalidField("economics.items.line_subtotal")
        }
        RadrootsOrderPayloadError::InvalidEconomicLineAmount { field, .. }
        | RadrootsOrderPayloadError::InvalidEconomicLineKind { field, .. }
        | RadrootsOrderPayloadError::InvalidEconomicLineEffect { field, .. }
        | RadrootsOrderPayloadError::InvalidEconomicCurrency { field }
        | RadrootsOrderPayloadError::InvalidEconomicOrdering { field }
        | RadrootsOrderPayloadError::InvalidEconomicTotal { field }
        | RadrootsOrderPayloadError::InvalidOrderEconomicsBinding { field } => {
            EventEncodeError::InvalidField(field)
        }
        RadrootsOrderPayloadError::InvalidQuoteVersion => {
            EventEncodeError::InvalidField("economics.quote_version")
        }
        RadrootsOrderPayloadError::MissingInventoryCommitments => {
            EventEncodeError::EmptyRequiredField("inventory_commitments")
        }
        RadrootsOrderPayloadError::InvalidInventoryCommitmentCount { .. } => {
            EventEncodeError::InvalidField("inventory_commitments.bin_count")
        }
    }
}

#[cfg(feature = "serde_json")]
struct OrderEnvelopeEventBuildParts<'a, T> {
    recipient_pubkey: &'a PublicKey,
    message_type: RadrootsOrderEventType,
    listing_addr: &'a str,
    order_id: &'a str,
    listing_event: Option<&'a RadrootsEventPtr>,
    root_event_id: Option<&'a RadrootsEventId>,
    prev_event_id: Option<&'a RadrootsEventId>,
    payload: &'a T,
}

#[cfg(feature = "serde_json")]
fn order_envelope_event_build<T: serde::Serialize>(
    parts: OrderEnvelopeEventBuildParts<'_, T>,
) -> Result<RadrootsNip01EventWireParts, EventEncodeError> {
    if parts.message_type.requires_listing_snapshot() && parts.listing_event.is_none() {
        return Err(EventEncodeError::EmptyRequiredField("listing_event.id"));
    }
    if parts.message_type.requires_order_chain() {
        if parts.root_event_id.is_none() {
            return Err(EventEncodeError::EmptyRequiredField("root_event_id"));
        }
        if parts.prev_event_id.is_none() {
            return Err(EventEncodeError::EmptyRequiredField("prev_event_id"));
        }
    }

    let envelope = RadrootsOrderEnvelope::new(
        parts.message_type,
        parts.listing_addr,
        parts.order_id,
        parts.payload,
    );
    envelope.validate().map_err(map_order_envelope_error)?;
    let content = serde_json::to_string(&envelope).map_err(|_| EventEncodeError::Json)?;
    let root_event_id = parts.root_event_id.map(RadrootsEventId::to_hex);
    let prev_event_id = parts.prev_event_id.map(RadrootsEventId::to_hex);
    let tags = order_envelope_tags(
        parts.recipient_pubkey.to_hex(),
        parts.listing_addr,
        Some(parts.order_id),
        parts.listing_event,
        root_event_id.as_deref(),
        prev_event_id.as_deref(),
    )?;
    Ok(RadrootsNip01EventWireParts {
        kind: parts.message_type.kind(),
        content,
        tags,
    })
}

#[cfg(feature = "serde_json")]
pub fn order_request_event_build(
    listing_event: &RadrootsEventPtr,
    payload: &RadrootsOrderRequest,
) -> Result<RadrootsNip01EventWireParts, EventEncodeError> {
    payload.validate().map_err(map_order_payload_error)?;
    order_envelope_event_build(OrderEnvelopeEventBuildParts {
        recipient_pubkey: &payload.seller_pubkey,
        message_type: RadrootsOrderEventType::OrderRequested,
        listing_addr: payload.listing_addr.as_str(),
        order_id: payload.order_id.as_str(),
        listing_event: Some(listing_event),
        root_event_id: None,
        prev_event_id: None,
        payload,
    })
}

#[cfg(feature = "serde_json")]
pub fn order_decision_event_build(
    root_event_id: &RadrootsEventId,
    prev_event_id: &RadrootsEventId,
    payload: &RadrootsOrderDecision,
) -> Result<RadrootsNip01EventWireParts, EventEncodeError> {
    payload.validate().map_err(map_order_payload_error)?;
    order_envelope_event_build(OrderEnvelopeEventBuildParts {
        recipient_pubkey: &payload.buyer_pubkey,
        message_type: RadrootsOrderEventType::OrderDecision,
        listing_addr: payload.listing_addr.as_str(),
        order_id: payload.order_id.as_str(),
        listing_event: None,
        root_event_id: Some(root_event_id),
        prev_event_id: Some(prev_event_id),
        payload,
    })
}

#[cfg(feature = "serde_json")]
pub fn order_cancellation_event_build(
    root_event_id: &RadrootsEventId,
    prev_event_id: &RadrootsEventId,
    payload: &RadrootsOrderCancellation,
) -> Result<RadrootsNip01EventWireParts, EventEncodeError> {
    payload.validate().map_err(map_order_payload_error)?;
    order_envelope_event_build(OrderEnvelopeEventBuildParts {
        recipient_pubkey: &payload.seller_pubkey,
        message_type: RadrootsOrderEventType::OrderCancelled,
        listing_addr: payload.listing_addr.as_str(),
        order_id: payload.order_id.as_str(),
        listing_event: None,
        root_event_id: Some(root_event_id),
        prev_event_id: Some(prev_event_id),
        payload,
    })
}

#[cfg(all(test, feature = "serde_json"))]
mod tests {
    use super::{
        OrderEnvelopeEventBuildParts, map_order_envelope_error, map_order_payload_error,
        order_envelope_event_build,
    };
    use crate::error::EventEncodeError;
    use radroots_event::{
        id::RadrootsEventId,
        tag::RadrootsEventPtr,
        trade::order::{
            RadrootsOrderEnvelopeError, RadrootsOrderEventType, RadrootsOrderPayloadError,
        },
    };
    use radroots_identity::PublicKey;

    const RECIPIENT: &str = "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df";

    fn event_id(character: char) -> RadrootsEventId {
        core::iter::repeat_n(character, 64)
            .collect::<String>()
            .parse()
            .unwrap()
    }

    fn payload() -> serde_json::Value {
        serde_json::json!({})
    }

    #[test]
    fn order_encode_error_mappers_cover_envelope_and_payload_variants() {
        assert_empty_required(
            map_order_envelope_error(RadrootsOrderEnvelopeError::MissingOrderId),
            "order_id",
        );
        assert_empty_required(
            map_order_envelope_error(RadrootsOrderEnvelopeError::MissingListingAddr),
            "listing_addr",
        );
        assert_invalid_field(
            map_order_envelope_error(RadrootsOrderEnvelopeError::InvalidVersion {
                expected: 1,
                got: 2,
            }),
            "version",
        );
        assert_empty_required(
            map_order_payload_error(RadrootsOrderPayloadError::EmptyField("buyer_pubkey")),
            "buyer_pubkey",
        );
        assert_empty_required(
            map_order_payload_error(RadrootsOrderPayloadError::MissingItems),
            "items",
        );
        assert_invalid_field(
            map_order_payload_error(RadrootsOrderPayloadError::InvalidItemBinCount { index: 0 }),
            "items.bin_count",
        );
        assert_empty_required(
            map_order_payload_error(RadrootsOrderPayloadError::MissingEconomicItems),
            "economics.items",
        );
        assert_invalid_field(
            map_order_payload_error(RadrootsOrderPayloadError::InvalidEconomicItemBinCount {
                index: 0,
            }),
            "economics.items.bin_count",
        );
        assert_invalid_field(
            map_order_payload_error(RadrootsOrderPayloadError::InvalidEconomicItemQuantity {
                index: 0,
            }),
            "economics.items.quantity_amount",
        );
        assert_invalid_field(
            map_order_payload_error(RadrootsOrderPayloadError::InvalidEconomicItemPrice {
                index: 0,
            }),
            "economics.items.unit_price_amount",
        );
        assert_invalid_field(
            map_order_payload_error(RadrootsOrderPayloadError::InvalidEconomicItemSubtotal {
                index: 0,
            }),
            "economics.items.line_subtotal",
        );
        for error in [
            RadrootsOrderPayloadError::InvalidEconomicLineAmount {
                field: "adjustments.amount",
                index: 0,
            },
            RadrootsOrderPayloadError::InvalidEconomicLineKind {
                field: "discounts.kind",
                index: 0,
            },
            RadrootsOrderPayloadError::InvalidEconomicLineEffect {
                field: "discounts.effect",
                index: 0,
            },
            RadrootsOrderPayloadError::InvalidEconomicCurrency {
                field: "subtotal.currency",
            },
            RadrootsOrderPayloadError::InvalidEconomicOrdering {
                field: "adjustments",
            },
            RadrootsOrderPayloadError::InvalidEconomicTotal { field: "total" },
            RadrootsOrderPayloadError::InvalidOrderEconomicsBinding { field: "items" },
        ] {
            assert!(matches!(
                map_order_payload_error(error),
                EventEncodeError::InvalidField(_)
            ));
        }
        assert_invalid_field(
            map_order_payload_error(RadrootsOrderPayloadError::InvalidQuoteVersion),
            "economics.quote_version",
        );
        assert_empty_required(
            map_order_payload_error(RadrootsOrderPayloadError::MissingInventoryCommitments),
            "inventory_commitments",
        );
        assert_invalid_field(
            map_order_payload_error(RadrootsOrderPayloadError::InvalidInventoryCommitmentCount {
                index: 0,
            }),
            "inventory_commitments.bin_count",
        );
    }

    #[test]
    fn order_envelope_event_build_requires_context_tags_by_message_type() {
        let payload = payload();
        let recipient_pubkey = PublicKey::from_hex(RECIPIENT).expect("recipient public key");
        let root_event_id = event_id('1');
        let prev_event_id = event_id('2');

        let missing_listing_event = order_envelope_event_build(OrderEnvelopeEventBuildParts {
            recipient_pubkey: &recipient_pubkey,
            message_type: RadrootsOrderEventType::OrderRequested,
            listing_addr: "listing-address",
            order_id: "order-1",
            listing_event: None,
            root_event_id: None,
            prev_event_id: None,
            payload: &payload,
        })
        .unwrap_err();
        assert_empty_required(missing_listing_event, "listing_event.id");

        let missing_root = order_envelope_event_build(OrderEnvelopeEventBuildParts {
            recipient_pubkey: &recipient_pubkey,
            message_type: RadrootsOrderEventType::OrderDecision,
            listing_addr: "listing-address",
            order_id: "order-1",
            listing_event: None,
            root_event_id: None,
            prev_event_id: Some(&prev_event_id),
            payload: &payload,
        })
        .unwrap_err();
        assert_empty_required(missing_root, "root_event_id");

        let missing_prev = order_envelope_event_build(OrderEnvelopeEventBuildParts {
            recipient_pubkey: &recipient_pubkey,
            message_type: RadrootsOrderEventType::OrderDecision,
            listing_addr: "listing-address",
            order_id: "order-1",
            listing_event: None,
            root_event_id: Some(&root_event_id),
            prev_event_id: None,
            payload: &payload,
        })
        .unwrap_err();
        assert_empty_required(missing_prev, "prev_event_id");

        let invalid_listing_event = order_envelope_event_build(OrderEnvelopeEventBuildParts {
            recipient_pubkey: &recipient_pubkey,
            message_type: RadrootsOrderEventType::OrderRequested,
            listing_addr: "listing-address",
            order_id: "order-1",
            listing_event: Some(&RadrootsEventPtr {
                id: String::new(),
                relays: None,
            }),
            root_event_id: None,
            prev_event_id: None,
            payload: &payload,
        })
        .unwrap_err();
        assert_empty_required(invalid_listing_event, "listing_event.id");
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn assert_empty_required(error: EventEncodeError, field: &'static str) {
        match error {
            EventEncodeError::EmptyRequiredField(found) => assert_eq!(found, field),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn assert_invalid_field(error: EventEncodeError, field: &'static str) {
        match error {
            EventEncodeError::InvalidField(found) => assert_eq!(found, field),
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
