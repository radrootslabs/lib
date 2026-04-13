#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};
#[cfg(feature = "std")]
use std::{string::String, vec::Vec};

use crate::config::{RadrootsSdkConfig, SdkConfigError, SdkTransportMode};
use crate::{
    NostrTags, RadrootsNostrEvent, RadrootsNostrEventPtr, RadrootsProfile, RadrootsProfileType,
    RadrootsTradeEnvelope, TradeListingValidateResult, WireEventParts, farm, listing, profile,
    trade,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSdkClient {
    config: RadrootsSdkConfig,
}

impl RadrootsSdkClient {
    pub fn from_config(config: RadrootsSdkConfig) -> Result<Self, SdkConfigError> {
        config.resolved_relay_urls()?;
        config.resolved_radrootsd_endpoint()?;
        Ok(Self { config })
    }

    pub fn config(&self) -> &RadrootsSdkConfig {
        &self.config
    }

    pub fn transport(&self) -> SdkTransportMode {
        self.config.transport
    }

    pub fn resolved_relay_urls(&self) -> Result<Vec<String>, SdkConfigError> {
        self.config.resolved_relay_urls()
    }

    pub fn resolved_radrootsd_endpoint(&self) -> Result<String, SdkConfigError> {
        self.config.resolved_radrootsd_endpoint()
    }

    pub fn profile(&self) -> ProfileClient<'_> {
        ProfileClient { client: self }
    }

    pub fn farm(&self) -> FarmClient<'_> {
        FarmClient { client: self }
    }

    pub fn listing(&self) -> ListingClient<'_> {
        ListingClient { client: self }
    }

    pub fn trade(&self) -> TradeClient<'_> {
        TradeClient { client: self }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ProfileClient<'a> {
    client: &'a RadrootsSdkClient,
}

impl<'a> ProfileClient<'a> {
    pub fn sdk(&self) -> &'a RadrootsSdkClient {
        self.client
    }

    pub fn transport(&self) -> SdkTransportMode {
        self.client.transport()
    }

    #[cfg(feature = "serde_json")]
    pub fn build_draft(
        &self,
        profile_value: &RadrootsProfile,
        profile_type: Option<RadrootsProfileType>,
    ) -> Result<WireEventParts, profile::ProfileEncodeError> {
        profile::build_draft(profile_value, profile_type)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FarmClient<'a> {
    client: &'a RadrootsSdkClient,
}

impl<'a> FarmClient<'a> {
    pub fn sdk(&self) -> &'a RadrootsSdkClient {
        self.client
    }

    pub fn transport(&self) -> SdkTransportMode {
        self.client.transport()
    }

    #[cfg(feature = "serde_json")]
    pub fn build_draft(
        &self,
        farm_value: &farm::RadrootsFarm,
    ) -> Result<WireEventParts, farm::EventEncodeError> {
        farm::build_draft(farm_value)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ListingClient<'a> {
    client: &'a RadrootsSdkClient,
}

impl<'a> ListingClient<'a> {
    pub fn sdk(&self) -> &'a RadrootsSdkClient {
        self.client
    }

    pub fn transport(&self) -> SdkTransportMode {
        self.client.transport()
    }

    pub fn build_tags(
        &self,
        listing_value: &listing::RadrootsListing,
    ) -> Result<NostrTags, listing::EventEncodeError> {
        listing::build_tags(listing_value)
    }

    #[cfg(feature = "serde_json")]
    pub fn build_draft(
        &self,
        listing_value: &listing::RadrootsListing,
    ) -> Result<WireEventParts, listing::EventEncodeError> {
        listing::build_draft(listing_value)
    }

    #[cfg(feature = "serde_json")]
    pub fn parse_event(
        &self,
        event: &RadrootsNostrEvent,
    ) -> Result<listing::RadrootsListing, listing::RadrootsTradeListingParseError> {
        listing::parse_event(event)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TradeClient<'a> {
    client: &'a RadrootsSdkClient,
}

impl<'a> TradeClient<'a> {
    pub fn sdk(&self) -> &'a RadrootsSdkClient {
        self.client
    }

    pub fn transport(&self) -> SdkTransportMode {
        self.client.transport()
    }

    #[cfg(feature = "serde_json")]
    #[allow(clippy::too_many_arguments)]
    pub fn build_envelope_draft(
        &self,
        recipient_pubkey: impl Into<String>,
        message_type: trade::RadrootsTradeMessageType,
        listing_addr: impl Into<String>,
        order_id: Option<String>,
        listing_event: Option<&RadrootsNostrEventPtr>,
        root_event_id: Option<&str>,
        prev_event_id: Option<&str>,
        payload: &trade::RadrootsTradeMessagePayload,
    ) -> Result<WireEventParts, trade::EventEncodeError> {
        trade::build_envelope_draft(
            recipient_pubkey,
            message_type,
            listing_addr,
            order_id,
            listing_event,
            root_event_id,
            prev_event_id,
            payload,
        )
    }

    #[cfg(feature = "serde_json")]
    pub fn parse_envelope(
        &self,
        event: &RadrootsNostrEvent,
    ) -> Result<RadrootsTradeEnvelope, trade::RadrootsTradeEnvelopeParseError> {
        trade::parse_envelope(event)
    }

    #[cfg(feature = "serde_json")]
    pub fn parse_listing_address(
        &self,
        listing_addr: &str,
    ) -> Result<trade::RadrootsTradeListingAddress, trade::RadrootsTradeListingAddressError> {
        trade::parse_listing_address(listing_addr)
    }

    #[cfg(feature = "serde_json")]
    pub fn validate_listing_event(
        &self,
        event: &RadrootsNostrEvent,
    ) -> Result<TradeListingValidateResult, trade::RadrootsTradeListingValidationError> {
        trade::validate_listing_event(event)
    }
}
