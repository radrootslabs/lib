#![forbid(unsafe_code)]

use core::str::FromStr;

use radroots_events::ids::{
    RadrootsEventId, RadrootsIdParseError, RadrootsListingAddress, RadrootsOrderId,
    RadrootsPublicKey,
};

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
    pub listing_addr: Option<RadrootsListingAddress>,
    pub buyer_pubkey: Option<RadrootsPublicKey>,
    pub seller_pubkey: Option<RadrootsPublicKey>,
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

    pub fn with_listing_addr(mut self, listing_addr: RadrootsListingAddress) -> Self {
        self.listing_addr = Some(listing_addr);
        self
    }

    pub fn with_buyer_pubkey(mut self, buyer_pubkey: RadrootsPublicKey) -> Self {
        self.buyer_pubkey = Some(buyer_pubkey);
        self
    }

    pub fn with_seller_pubkey(mut self, seller_pubkey: RadrootsPublicKey) -> Self {
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
    pub listing_addr: RadrootsListingAddress,
    pub buyer_pubkey: RadrootsPublicKey,
    pub seller_pubkey: RadrootsPublicKey,
}

impl RadrootsTradeLocatorCandidate {
    pub fn locator(&self) -> RadrootsTradeLocator {
        RadrootsTradeLocator {
            trade_id: self.trade_id.clone(),
            root_event_id: Some(self.root_event_id.clone()),
            listing_addr: Some(self.listing_addr.clone()),
            buyer_pubkey: Some(self.buyer_pubkey.clone()),
            seller_pubkey: Some(self.seller_pubkey.clone()),
        }
    }
}
