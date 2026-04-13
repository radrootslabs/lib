use radroots_core::{
    RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreQuantity,
    RadrootsCoreQuantityPrice, RadrootsCoreUnit,
};
use radroots_events::farm::RadrootsFarm;
use radroots_events::kinds::{KIND_LISTING, KIND_TRADE_LISTING_VALIDATE_REQ};
use radroots_events::listing::{
    RadrootsListing, RadrootsListingAvailability, RadrootsListingBin,
    RadrootsListingDeliveryMethod, RadrootsListingFarmRef, RadrootsListingLocation,
    RadrootsListingProduct, RadrootsListingStatus,
};
use radroots_events::trade::{RadrootsTradeListingValidateRequest, RadrootsTradeMessagePayload};
use radroots_sdk::{
    RADROOTS_SDK_PRODUCTION_RADROOTSD_ENDPOINT, RADROOTS_SDK_PRODUCTION_RELAY_URL,
    RadrootsNostrEvent, RadrootsSdkClient, RadrootsSdkConfig, RelayConfig, SdkConfigError,
    SdkEnvironment, SdkTransportMode, SignerConfig,
};

fn sample_farm() -> RadrootsFarm {
    RadrootsFarm {
        d_tag: "AAAAAAAAAAAAAAAAAAAAAA".into(),
        name: "North Farm".into(),
        about: Some("Organic coffee".into()),
        website: None,
        picture: None,
        banner: None,
        location: None,
        tags: Some(vec!["coffee".into()]),
    }
}

fn sample_listing() -> RadrootsListing {
    RadrootsListing {
        d_tag: "AAAAAAAAAAAAAAAAAAAAAg".into(),
        farm: RadrootsListingFarmRef {
            pubkey: "seller".into(),
            d_tag: "AAAAAAAAAAAAAAAAAAAAAA".into(),
        },
        product: RadrootsListingProduct {
            key: "coffee".into(),
            title: "Coffee".into(),
            category: "coffee".into(),
            summary: Some("Single origin coffee".into()),
            process: None,
            lot: None,
            location: None,
            profile: None,
            year: None,
        },
        primary_bin_id: "bin-1".into(),
        bins: vec![RadrootsListingBin {
            bin_id: "bin-1".into(),
            quantity: RadrootsCoreQuantity::new(
                RadrootsCoreDecimal::from(1000u32),
                RadrootsCoreUnit::MassG,
            ),
            price_per_canonical_unit: RadrootsCoreQuantityPrice {
                amount: RadrootsCoreMoney::new(
                    RadrootsCoreDecimal::from(20u32),
                    RadrootsCoreCurrency::USD,
                ),
                quantity: RadrootsCoreQuantity::new(
                    RadrootsCoreDecimal::from(1u32),
                    RadrootsCoreUnit::MassG,
                ),
            },
            display_amount: None,
            display_unit: None,
            display_label: None,
            display_price: None,
            display_price_unit: None,
        }],
        resource_area: None,
        plot: None,
        discounts: None,
        inventory_available: Some(RadrootsCoreDecimal::from(5u32)),
        availability: Some(RadrootsListingAvailability::Status {
            status: RadrootsListingStatus::Active,
        }),
        delivery_method: Some(RadrootsListingDeliveryMethod::Pickup),
        location: Some(RadrootsListingLocation {
            primary: "North Farm".into(),
            city: None,
            region: None,
            country: None,
            lat: None,
            lng: None,
            geohash: None,
        }),
        images: None,
    }
}

#[test]
fn client_default_config_uses_production_relay_direct() {
    let client = RadrootsSdkClient::from_config(RadrootsSdkConfig::default()).expect("sdk client");

    assert_eq!(client.transport(), SdkTransportMode::RelayDirect);
    assert_eq!(
        client.resolved_relay_urls().expect("resolved relays"),
        vec![RADROOTS_SDK_PRODUCTION_RELAY_URL.to_string()]
    );
    assert_eq!(
        client
            .resolved_radrootsd_endpoint()
            .expect("resolved radrootsd"),
        RADROOTS_SDK_PRODUCTION_RADROOTSD_ENDPOINT
    );
}

#[test]
fn client_rejects_invalid_config_on_construction() {
    let mut config = RadrootsSdkConfig::custom();
    config.transport = SdkTransportMode::RelayDirect;
    config.relay = RelayConfig {
        urls: vec!["https://radroots.org".into()],
    };

    let error = RadrootsSdkClient::from_config(config).expect_err("invalid config");
    assert_eq!(
        error,
        SdkConfigError::InvalidRelayUrl("https://radroots.org".into())
    );
}

#[test]
fn client_allows_custom_relay_without_radrootsd_endpoint() {
    let mut config = RadrootsSdkConfig::custom();
    config.transport = SdkTransportMode::RelayDirect;
    config.relay = RelayConfig {
        urls: vec!["wss://radroots.org".into()],
    };

    RadrootsSdkClient::from_config(config).expect("relay-only sdk client");
}

#[test]
fn client_allows_custom_radrootsd_without_relay_urls() {
    let mut config = RadrootsSdkConfig::custom();
    config.transport = SdkTransportMode::Radrootsd;
    config.radrootsd.endpoint = Some("https://rpc.radroots.org/jsonrpc".into());

    RadrootsSdkClient::from_config(config).expect("radrootsd-only sdk client");
}

#[test]
fn namespace_clients_reflect_explicit_transport_mode() {
    let mut config = RadrootsSdkConfig::for_environment(SdkEnvironment::Production);
    config.transport = SdkTransportMode::Radrootsd;
    config.signer = SignerConfig::LocalIdentity;

    let client = RadrootsSdkClient::from_config(config).expect("sdk client");

    assert_eq!(client.transport(), SdkTransportMode::Radrootsd);
    assert_eq!(client.profile().transport(), SdkTransportMode::Radrootsd);
    assert_eq!(client.farm().transport(), SdkTransportMode::Radrootsd);
    assert_eq!(client.listing().transport(), SdkTransportMode::Radrootsd);
    assert_eq!(client.trade().transport(), SdkTransportMode::Radrootsd);
    assert_eq!(client.signer(), SignerConfig::LocalIdentity);
    assert_eq!(client.profile().signer(), SignerConfig::LocalIdentity);
    assert_eq!(client.farm().signer(), SignerConfig::LocalIdentity);
    assert_eq!(client.listing().signer(), SignerConfig::LocalIdentity);
    assert_eq!(client.trade().signer(), SignerConfig::LocalIdentity);
}

#[test]
fn listing_and_trade_clients_wrap_existing_sdk_facades() {
    let client = RadrootsSdkClient::from_config(RadrootsSdkConfig::local()).expect("sdk client");
    let listing_value = sample_listing();

    let tags = client
        .listing()
        .build_tags(&listing_value)
        .expect("listing tags");
    assert!(!tags.is_empty());

    let draft = client
        .listing()
        .build_draft(&listing_value)
        .expect("listing draft");
    assert_eq!(draft.as_wire_parts().kind, KIND_LISTING);

    let event = RadrootsNostrEvent {
        id: "listing-1".into(),
        author: "seller".into(),
        created_at: 1,
        kind: draft.as_wire_parts().kind,
        tags: draft.as_wire_parts().tags.clone(),
        content: draft.as_wire_parts().content.clone(),
        sig: String::new(),
    };
    let parsed = client
        .listing()
        .parse_event(&event)
        .expect("parsed listing");
    assert_eq!(parsed.d_tag, listing_value.d_tag);

    let validated = client
        .trade()
        .validate_listing_event(&event)
        .expect("validated listing");
    assert_eq!(validated.listing_id, listing_value.d_tag);

    let listing_addr = format!("{KIND_LISTING}:seller:{}", listing_value.d_tag);
    let payload =
        RadrootsTradeMessagePayload::ListingValidateRequest(RadrootsTradeListingValidateRequest {
            listing_event: None,
        });
    let envelope = client
        .trade()
        .build_envelope_draft(
            "buyer",
            payload.message_type(),
            listing_addr.clone(),
            None,
            None,
            None,
            None,
            &payload,
        )
        .expect("trade draft");
    assert_eq!(envelope.kind, KIND_TRADE_LISTING_VALIDATE_REQ);
    let parsed_addr = client
        .trade()
        .parse_listing_address(&listing_addr)
        .expect("listing address");
    assert_eq!(parsed_addr.listing_id, listing_value.d_tag);
}

#[test]
fn farm_client_wraps_existing_farm_facade() {
    let client =
        RadrootsSdkClient::from_config(RadrootsSdkConfig::production()).expect("sdk client");
    let farm = sample_farm();

    let draft = client.farm().build_draft(&farm).expect("farm draft");
    assert!(!draft.tags.is_empty());
}
