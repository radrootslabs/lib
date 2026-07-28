#![forbid(unsafe_code)]

use core::str::FromStr;

use radroots_event::ids::{
    RadrootsClassifiedListingAddress, RadrootsEventId, RadrootsIdParseError, RadrootsOrderId,
};
use radroots_identity::PublicKey;

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(as = "string"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RadrootsTradeId(RadrootsOrderId);

impl RadrootsTradeId {
    pub fn parse(value: impl AsRef<str>) -> Result<Self, RadrootsIdParseError> {
        RadrootsOrderId::parse(value).map(Self)
    }

    pub fn as_order_id(&self) -> &RadrootsOrderId {
        &self.0
    }

    pub fn into_order_id(self) -> RadrootsOrderId {
        self.0
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl From<RadrootsOrderId> for RadrootsTradeId {
    fn from(order_id: RadrootsOrderId) -> Self {
        Self(order_id)
    }
}

impl From<RadrootsTradeId> for RadrootsOrderId {
    fn from(trade_id: RadrootsTradeId) -> Self {
        trade_id.into_order_id()
    }
}

impl AsRef<str> for RadrootsTradeId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl FromStr for RadrootsTradeId {
    type Err = RadrootsIdParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeLocator {
    pub trade_id: RadrootsTradeId,
    pub root_event_id: Option<RadrootsEventId>,
    pub listing_addr: Option<RadrootsClassifiedListingAddress>,
    #[cfg_attr(feature = "dto-bindgen", dto(as = "string"))]
    pub buyer_pubkey: Option<PublicKey>,
    #[cfg_attr(feature = "dto-bindgen", dto(as = "string"))]
    pub seller_pubkey: Option<PublicKey>,
}

impl RadrootsTradeLocator {
    pub fn new(trade_id: impl Into<RadrootsTradeId>) -> Self {
        Self {
            trade_id: trade_id.into(),
            root_event_id: None,
            listing_addr: None,
            buyer_pubkey: None,
            seller_pubkey: None,
        }
    }

    pub fn from_order_id(order_id: RadrootsOrderId) -> Self {
        Self::new(order_id)
    }

    pub fn order_id(&self) -> &RadrootsOrderId {
        self.trade_id.as_order_id()
    }

    pub fn with_root_event_id(mut self, root_event_id: RadrootsEventId) -> Self {
        self.root_event_id = Some(root_event_id);
        self
    }

    pub fn with_listing_addr(mut self, listing_addr: RadrootsClassifiedListingAddress) -> Self {
        self.listing_addr = Some(listing_addr);
        self
    }

    pub fn with_buyer_pubkey(mut self, buyer_pubkey: PublicKey) -> Self {
        self.buyer_pubkey = Some(buyer_pubkey);
        self
    }

    pub fn with_seller_pubkey(mut self, seller_pubkey: PublicKey) -> Self {
        self.seller_pubkey = Some(seller_pubkey);
        self
    }
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeLocatorCandidate {
    pub trade_id: RadrootsTradeId,
    pub root_event_id: RadrootsEventId,
    pub listing_addr: RadrootsClassifiedListingAddress,
    #[cfg_attr(feature = "dto-bindgen", dto(as = "string"))]
    pub buyer_pubkey: PublicKey,
    #[cfg_attr(feature = "dto-bindgen", dto(as = "string"))]
    pub seller_pubkey: PublicKey,
}

impl RadrootsTradeLocatorCandidate {
    pub fn locator(&self) -> RadrootsTradeLocator {
        RadrootsTradeLocator {
            trade_id: self.trade_id.clone(),
            root_event_id: Some(self.root_event_id.clone()),
            listing_addr: Some(self.listing_addr.clone()),
            buyer_pubkey: Some(self.buyer_pubkey),
            seller_pubkey: Some(self.seller_pubkey),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use radroots_event::kinds::KIND_CLASSIFIED_LISTING;
    use radroots_test_fixtures::{FIXTURE_ALICE_PUBLIC_KEY_HEX, FIXTURE_BOB_PUBLIC_KEY_HEX};

    const BUYER: &str = FIXTURE_BOB_PUBLIC_KEY_HEX;
    const SELLER: &str = FIXTURE_ALICE_PUBLIC_KEY_HEX;

    fn event_id(raw: u8) -> RadrootsEventId {
        RadrootsEventId::parse(format!("{raw:064x}")).expect("event id")
    }

    fn order_id() -> RadrootsOrderId {
        RadrootsOrderId::parse("order-1").expect("order id")
    }

    fn public_key(raw: &str) -> PublicKey {
        PublicKey::from_hex(raw).expect("public key")
    }

    fn listing_addr() -> RadrootsClassifiedListingAddress {
        RadrootsClassifiedListingAddress::parse(format!(
            "{KIND_CLASSIFIED_LISTING}:{SELLER}:AAAAAAAAAAAAAAAAAAAAAg"
        ))
        .expect("listing address")
    }

    #[test]
    fn trade_id_and_locator_accessors_cover_public_surface() {
        let order_id = order_id();
        let trade_id = RadrootsTradeId::parse(order_id.as_str()).expect("trade id");

        assert_eq!(trade_id.as_order_id(), &order_id);
        assert_eq!(trade_id.as_str(), "order-1");
        assert_eq!(trade_id.as_ref(), "order-1");
        assert_eq!(RadrootsTradeId::from_str("order-1").unwrap(), trade_id);
        assert!(RadrootsTradeId::parse(" ").is_err());
        assert_eq!(
            RadrootsOrderId::from(trade_id.clone()),
            trade_id.clone().into_order_id()
        );

        let locator = RadrootsTradeLocator::from_order_id(order_id.clone())
            .with_root_event_id(event_id(1))
            .with_listing_addr(listing_addr())
            .with_buyer_pubkey(public_key(BUYER))
            .with_seller_pubkey(public_key(SELLER));

        assert_eq!(locator.order_id(), &order_id);
        assert_eq!(locator.trade_id.as_order_id(), &order_id);
        assert_eq!(locator.root_event_id, Some(event_id(1)));
        assert_eq!(locator.listing_addr, Some(listing_addr()));
        assert_eq!(locator.buyer_pubkey, Some(public_key(BUYER)));
        assert_eq!(locator.seller_pubkey, Some(public_key(SELLER)));
    }

    #[test]
    fn locator_candidate_converts_to_specific_locator() {
        let candidate = RadrootsTradeLocatorCandidate {
            trade_id: order_id().into(),
            root_event_id: event_id(1),
            listing_addr: listing_addr(),
            buyer_pubkey: public_key(BUYER),
            seller_pubkey: public_key(SELLER),
        };

        let locator = candidate.locator();

        assert_eq!(locator.trade_id, candidate.trade_id);
        assert_eq!(locator.root_event_id, Some(candidate.root_event_id));
        assert_eq!(locator.listing_addr, Some(candidate.listing_addr));
        assert_eq!(locator.buyer_pubkey, Some(candidate.buyer_pubkey));
        assert_eq!(locator.seller_pubkey, Some(candidate.seller_pubkey));
    }
}
