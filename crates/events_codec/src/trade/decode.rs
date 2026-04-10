#[cfg(all(not(feature = "std"), feature = "serde_json"))]
use alloc::{borrow::ToOwned, format, string::String, vec::Vec};

#[cfg(feature = "serde_json")]
use radroots_events::{
    RadrootsNostrEvent, RadrootsNostrEventPtr,
    kinds::{KIND_PROFILE, is_trade_kind},
    tags::{TAG_D, TAG_E_PREV, TAG_E_ROOT},
    trade::{RadrootsTradeEnvelope, RadrootsTradeEnvelopeError, RadrootsTradeMessageType},
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

#[cfg(all(test, feature = "serde_json"))]
mod tests {
    use super::{
        RadrootsTradeEnvelopeParseError, RadrootsTradeListingAddress, trade_envelope_from_event,
        trade_event_context_from_tags,
    };
    use crate::trade::encode::trade_envelope_event_build;
    use crate::trade::tags::TAG_LISTING_EVENT;
    use radroots_events::{
        RadrootsNostrEvent, RadrootsNostrEventPtr,
        tags::{TAG_D, TAG_E_PREV, TAG_E_ROOT},
        trade::{
            RadrootsTradeEnvelope, RadrootsTradeMessagePayload, RadrootsTradeMessageType,
            RadrootsTradeOrder, RadrootsTradeOrderItem,
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
