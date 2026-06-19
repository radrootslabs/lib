use radroots_core::{
    RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreQuantity,
    RadrootsCoreQuantityPrice, RadrootsCoreUnit,
};
use radroots_events::farm::{RadrootsFarm, RadrootsFarmRef};
use radroots_events::kinds::{KIND_FARM, KIND_LISTING, KIND_PROFILE};
use radroots_events::listing::{
    RadrootsListing, RadrootsListingAvailability, RadrootsListingBin,
    RadrootsListingDeliveryMethod, RadrootsListingLocation, RadrootsListingProduct,
    RadrootsListingStatus,
};
use radroots_events::profile::{RadrootsProfile, RadrootsProfileType};
use radroots_sdk::client::{
    RadrootsSdkClient, SdkPublishError, SdkRadrootsdPublishReceipt, SdkRelayFailure,
    SdkResolvedTransportTarget,
};
use radroots_sdk::config::{
    RADROOTS_SDK_PRODUCTION_RELAY_URL, RadrootsSdkConfig, RelayConfig, SdkConfigError,
    SdkEnvironment, SdkTransportMode, SignerConfig,
};
use radroots_sdk::protocol::events::RadrootsNostrEvent;

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
        d_tag: "AAAAAAAAAAAAAAAAAAAAAg".parse().expect("listing d tag"),
        published_at: None,
        farm: RadrootsFarmRef {
            pubkey: "a".repeat(64),
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
        primary_bin_id: "bin-1".parse().expect("primary bin id"),
        bins: vec![RadrootsListingBin {
            bin_id: "bin-1".parse().expect("bin id"),
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

fn sample_profile() -> RadrootsProfile {
    RadrootsProfile {
        name: "north-farm".into(),
        display_name: Some("North Farm".into()),
        nip05: None,
        about: Some("Farm profile".into()),
        website: None,
        picture: None,
        banner: None,
        lud06: None,
        lud16: None,
        bot: None,
    }
}

#[test]
fn client_default_config_uses_production_relay_direct() {
    let client = RadrootsSdkClient::from_config(RadrootsSdkConfig::default()).expect("sdk client");

    assert_eq!(client.transport(), SdkTransportMode::RelayDirect);
    assert_eq!(
        client.resolved_transport_target(),
        &SdkResolvedTransportTarget::RelayDirect {
            relay_urls: vec![RADROOTS_SDK_PRODUCTION_RELAY_URL.to_string()],
        }
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
fn client_rejects_invalid_radrootsd_config_on_construction() {
    let mut missing = RadrootsSdkConfig::custom();
    missing.transport = SdkTransportMode::Radrootsd;

    assert_eq!(
        RadrootsSdkClient::from_config(missing).expect_err("missing radrootsd endpoint"),
        SdkConfigError::MissingCustomRadrootsdEndpoint
    );

    let mut invalid = RadrootsSdkConfig::custom();
    invalid.transport = SdkTransportMode::Radrootsd;
    invalid.radrootsd.endpoint = Some("wss://rpc.bad".into());

    assert_eq!(
        RadrootsSdkClient::from_config(invalid).expect_err("invalid radrootsd endpoint"),
        SdkConfigError::InvalidRadrootsdEndpoint("wss://rpc.bad".into())
    );
}

#[test]
fn client_allows_custom_relay_without_radrootsd_endpoint() {
    let mut config = RadrootsSdkConfig::custom();
    config.transport = SdkTransportMode::RelayDirect;
    config.relay = RelayConfig {
        urls: vec!["wss://radroots.org".into()],
    };

    let client = RadrootsSdkClient::from_config(config).expect("relay-only sdk client");
    assert_eq!(
        client.resolved_transport_target(),
        &SdkResolvedTransportTarget::RelayDirect {
            relay_urls: vec!["wss://radroots.org".to_string()],
        }
    );
}

#[test]
fn client_allows_custom_radrootsd_without_relay_urls() {
    let endpoint = "https://custom.radroots.org/jsonrpc";
    let mut config = RadrootsSdkConfig::custom();
    config.transport = SdkTransportMode::Radrootsd;
    config.radrootsd.endpoint = Some(endpoint.into());

    let client = RadrootsSdkClient::from_config(config).expect("radrootsd-only sdk client");
    assert_eq!(
        client.resolved_transport_target(),
        &SdkResolvedTransportTarget::Radrootsd {
            endpoint: endpoint.to_string(),
        }
    );
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
    #[cfg(feature = "radrootsd-client")]
    assert_eq!(client.radrootsd().transport(), SdkTransportMode::Radrootsd);
    assert_eq!(client.signer(), SignerConfig::LocalIdentity);
    assert_eq!(client.profile().signer(), SignerConfig::LocalIdentity);
    assert_eq!(client.farm().signer(), SignerConfig::LocalIdentity);
    assert_eq!(client.listing().signer(), SignerConfig::LocalIdentity);
    #[cfg(feature = "radrootsd-client")]
    assert_eq!(client.radrootsd().signer(), SignerConfig::LocalIdentity);
}

#[test]
fn namespace_clients_expose_parent_sdk_and_draft_facades() {
    let client =
        RadrootsSdkClient::from_config(RadrootsSdkConfig::production()).expect("sdk client");
    let profile = client.profile();
    let farm = client.farm();
    let listing = client.listing();

    assert_eq!(client.config().environment, SdkEnvironment::Production);
    assert!(std::ptr::eq(profile.sdk(), &client));
    assert!(std::ptr::eq(farm.sdk(), &client));
    assert!(std::ptr::eq(listing.sdk(), &client));

    let profile_draft = profile
        .build_draft(&sample_profile(), Some(RadrootsProfileType::Farm))
        .expect("profile draft");
    assert_eq!(profile_draft.kind, KIND_PROFILE);

    let farm_draft = farm.build_draft(&sample_farm()).expect("farm draft");
    assert_eq!(farm_draft.kind, KIND_FARM);

    let listing_draft = listing
        .build_draft(&sample_listing())
        .expect("listing draft");
    assert_eq!(listing_draft.as_wire_parts().kind, KIND_LISTING);
    assert_eq!(listing_draft.into_wire_parts().kind, KIND_LISTING);

    let mut invalid_listing = sample_listing();
    invalid_listing.farm.pubkey.clear();
    assert!(listing.build_draft(&invalid_listing).is_err());
}

#[test]
fn listing_client_wraps_existing_sdk_facade() {
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
        author: listing_value.farm.pubkey.clone(),
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
}

#[test]
fn publish_receipts_and_errors_format_public_details() {
    let receipt = SdkRadrootsdPublishReceipt {
        accepted: true,
        deduplicated: true,
        job_id: Some("job-1".into()),
        status: Some("accepted".into()),
        signer_mode: Some("secret-mode".into()),
        signer_session_id: Some("secret-session".into()),
        event_addr: Some("3432:pubkey:order-1".into()),
        relay_count: Some(2),
        acknowledged_relay_count: Some(1),
    };
    let debug = format!("{receipt:?}");

    assert!(debug.contains("SdkRadrootsdPublishReceipt"));
    assert!(debug.contains("<redacted>"));
    assert!(!debug.contains("secret-mode"));
    assert!(!debug.contains("secret-session"));

    let relay_failure = SdkRelayFailure {
        relay_url: "wss://relay.example".into(),
        error: "closed".into(),
    };
    let formatted = [
        SdkPublishError::from(SdkConfigError::EmptyRelayUrl).to_string(),
        SdkPublishError::Encode("encode failed".into()).to_string(),
        SdkPublishError::UnsupportedTransport {
            transport: SdkTransportMode::Radrootsd,
            operation: "order.publish",
        }
        .to_string(),
        SdkPublishError::UnsupportedSignerMode {
            transport: SdkTransportMode::RelayDirect,
            signer: SignerConfig::DraftOnly,
            required: SignerConfig::LocalIdentity,
            operation: "order.publish",
        }
        .to_string(),
        SdkPublishError::Relay("relay failed".into()).to_string(),
        SdkPublishError::RelaySetup {
            transport: SdkTransportMode::RelayDirect,
            operation: "order.publish",
            target_relays: Vec::new(),
            error: "setup failed".into(),
        }
        .to_string(),
        SdkPublishError::RelaySetup {
            transport: SdkTransportMode::RelayDirect,
            operation: "order.publish",
            target_relays: vec!["wss://relay.example".into()],
            error: "setup failed".into(),
        }
        .to_string(),
        SdkPublishError::RelayNotAcknowledged {
            transport: SdkTransportMode::RelayDirect,
            failed_relays: Vec::new(),
        }
        .to_string(),
        SdkPublishError::RelayNotAcknowledged {
            transport: SdkTransportMode::RelayDirect,
            failed_relays: vec![relay_failure],
        }
        .to_string(),
        SdkPublishError::Radrootsd("radrootsd failed".into()).to_string(),
    ];

    assert!(
        formatted
            .iter()
            .any(|message| message == "relay url must not be empty")
    );
    assert!(formatted.iter().any(|message| message == "encode failed"));
    assert!(
        formatted
            .iter()
            .any(|message| message.contains("requires signer mode `local_identity`"))
    );
    assert!(formatted.iter().any(|message| {
        message.contains("failed to prepare RelayDirect relay publish for wss://relay.example")
    }));
    assert!(
        formatted
            .iter()
            .any(|message| message.contains("wss://relay.example: closed"))
    );
    assert!(
        formatted
            .iter()
            .any(|message| message == "radrootsd failed")
    );
}

#[test]
fn farm_client_wraps_existing_farm_facade() {
    let client =
        RadrootsSdkClient::from_config(RadrootsSdkConfig::production()).expect("sdk client");
    let farm = sample_farm();

    let draft = client.farm().build_draft(&farm).expect("farm draft");
    assert!(!draft.tags.is_empty());
}
