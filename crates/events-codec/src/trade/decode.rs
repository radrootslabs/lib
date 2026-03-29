#[cfg(not(feature = "std"))]
use alloc::{format, string::String};

#[cfg(feature = "serde_json")]
use radroots_events::{
    RadrootsNostrEvent,
    kinds::{KIND_PROFILE, is_trade_listing_kind},
    tags::TAG_D,
    trade::{RadrootsTradeEnvelope, RadrootsTradeEnvelopeError, RadrootsTradeMessageType},
};
#[cfg(feature = "serde_json")]
use serde::de::DeserializeOwned;

#[cfg(feature = "serde_json")]
use crate::d_tag::is_d_tag_base64url;

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
            Self::InvalidKind(kind) => write!(f, "invalid trade listing event kind: {kind}"),
            Self::InvalidJson => write!(f, "invalid trade listing envelope json"),
            Self::InvalidEnvelope(error) => write!(f, "{error}"),
            Self::MessageTypeKindMismatch {
                event_kind,
                message_type,
            } => write!(
                f,
                "trade listing envelope type {message_type:?} does not match event kind {event_kind}"
            ),
            Self::MissingTag(tag) => write!(f, "missing required trade listing tag: {tag}"),
            Self::InvalidTag(tag) => write!(f, "invalid trade listing tag: {tag}"),
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
    if !is_trade_listing_kind(event.kind) {
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

    Ok(envelope)
}

#[cfg(all(test, feature = "serde_json"))]
mod tests {
    use super::{
        RadrootsTradeEnvelopeParseError, RadrootsTradeListingAddress,
        trade_envelope_from_event,
    };
    use crate::trade::encode::trade_envelope_event_build;
    use radroots_events::{
        RadrootsNostrEvent,
        trade::{
            RadrootsTradeEnvelope, RadrootsTradeMessagePayload, RadrootsTradeMessageType,
            RadrootsTradeOrder, RadrootsTradeOrderItem, RadrootsTradeOrderStatus,
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
            notes: None,
            status: RadrootsTradeOrderStatus::Requested,
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
        assert_eq!(envelope.message_type, RadrootsTradeMessageType::OrderRequest);
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
}
