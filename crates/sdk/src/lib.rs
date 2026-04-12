#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(feature = "std")]
use std::{string::String, vec::Vec};
#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

pub mod farm;
pub mod listing;
pub mod profile;
pub mod trade;

pub use radroots_events::{
    RadrootsNostrEvent, RadrootsNostrEventPtr, RadrootsNostrEventRef,
    farm::RadrootsFarm,
    listing::RadrootsListing,
    profile::{RadrootsProfile, RadrootsProfileType},
    trade::{RadrootsTradeMessagePayload, RadrootsTradeMessageType},
};
#[cfg(feature = "serde_json")]
pub use radroots_events_codec::trade::{
    RadrootsTradeEnvelopeParseError, RadrootsTradeListingAddress,
    RadrootsTradeListingAddressError,
};
pub use radroots_events_codec::wire::{EventDraft as UnsignedEventDraft, WireEventParts};
pub use radroots_trade::listing::validation::RadrootsTradeListing as TradeListingValidateResult;

pub type NostrTags = Vec<Vec<String>>;
pub type RadrootsTradeEnvelope =
    radroots_events::trade::RadrootsTradeEnvelope<RadrootsTradeMessagePayload>;
