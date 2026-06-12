use radroots_core::{
    RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreQuantity,
    RadrootsCoreQuantityPrice, RadrootsCoreUnit,
};
use radroots_events::farm::{RadrootsFarm, RadrootsFarmRef};
use radroots_events::kinds::{
    KIND_FARM, KIND_LISTING, KIND_PROFILE, KIND_TRADE_CANCEL, KIND_TRADE_FULFILLMENT_UPDATE,
    KIND_TRADE_LISTING_VALIDATE_REQ, KIND_TRADE_ORDER_REQUEST, KIND_TRADE_ORDER_RESPONSE,
    KIND_TRADE_ORDER_REVISION, KIND_TRADE_ORDER_REVISION_RESPONSE, KIND_TRADE_RECEIPT,
};
use radroots_events::listing::{
    RadrootsListing, RadrootsListingAvailability, RadrootsListingBin,
    RadrootsListingDeliveryMethod, RadrootsListingLocation, RadrootsListingProduct,
    RadrootsListingStatus,
};
use radroots_events::profile::{RadrootsProfile, RadrootsProfileType};
use radroots_events::trade::{
    RadrootsActiveTradeFulfillmentState, RadrootsTradeBuyerReceipt,
    RadrootsTradeFulfillmentUpdated, RadrootsTradeInventoryCommitment,
    RadrootsTradeListingValidateRequest, RadrootsTradeMessagePayload, RadrootsTradeOrderCancelled,
    RadrootsTradeOrderDecision, RadrootsTradeOrderDecisionEvent, RadrootsTradeOrderEconomicItem,
    RadrootsTradeOrderEconomics, RadrootsTradeOrderItem, RadrootsTradeOrderRequested,
    RadrootsTradeOrderRevisionDecision, RadrootsTradeOrderRevisionDecisionEvent,
    RadrootsTradeOrderRevisionProposed, RadrootsTradePricingBasis,
};
use radroots_sdk::{
    RADROOTS_SDK_PRODUCTION_RELAY_URL, RadrootsNostrEvent, RadrootsNostrEventPtr,
    RadrootsSdkClient, RadrootsSdkConfig, RelayConfig, SdkConfigError, SdkEnvironment,
    SdkPublishError, SdkRadrootsdPublishReceipt, SdkRelayFailure, SdkResolvedTransportTarget,
    SdkTransportMode, SignerConfig, WireEventParts,
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
        farm: RadrootsFarmRef {
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

fn decimal(raw: &str) -> RadrootsCoreDecimal {
    raw.parse().expect("decimal")
}

fn usd(raw: &str) -> RadrootsCoreMoney {
    RadrootsCoreMoney::new(decimal(raw), RadrootsCoreCurrency::USD)
}

fn listing_event_ptr() -> RadrootsNostrEventPtr {
    RadrootsNostrEventPtr {
        id: "listing-event-1".into(),
        relays: Some("wss://listing.relay.example".into()),
    }
}

fn sample_order_request(
    buyer_pubkey: String,
    seller_pubkey: String,
) -> RadrootsTradeOrderRequested {
    RadrootsTradeOrderRequested {
        order_id: "order-1".into(),
        listing_addr: format!("{KIND_LISTING}:{seller_pubkey}:AAAAAAAAAAAAAAAAAAAAAg"),
        buyer_pubkey,
        seller_pubkey,
        items: vec![RadrootsTradeOrderItem {
            bin_id: "bin-1".into(),
            bin_count: 2,
        }],
        economics: RadrootsTradeOrderEconomics {
            quote_id: "quote-1".into(),
            quote_version: 1,
            pricing_basis: RadrootsTradePricingBasis::ListingEvent,
            currency: RadrootsCoreCurrency::USD,
            items: vec![RadrootsTradeOrderEconomicItem {
                bin_id: "bin-1".into(),
                bin_count: 2,
                quantity_amount: decimal("1"),
                quantity_unit: RadrootsCoreUnit::Each,
                unit_price_amount: decimal("5"),
                unit_price_currency: RadrootsCoreCurrency::USD,
                line_subtotal: usd("10"),
            }],
            discounts: Vec::new(),
            adjustments: Vec::new(),
            subtotal: usd("10"),
            discount_total: usd("0"),
            adjustment_total: usd("0"),
            total: usd("10"),
        },
    }
}

fn sample_order_decision(
    buyer_pubkey: String,
    seller_pubkey: String,
) -> RadrootsTradeOrderDecisionEvent {
    RadrootsTradeOrderDecisionEvent {
        order_id: "order-1".into(),
        listing_addr: format!("{KIND_LISTING}:{seller_pubkey}:AAAAAAAAAAAAAAAAAAAAAg"),
        buyer_pubkey,
        seller_pubkey,
        decision: RadrootsTradeOrderDecision::Accepted {
            inventory_commitments: vec![RadrootsTradeInventoryCommitment {
                bin_id: "bin-1".into(),
                bin_count: 2,
            }],
        },
    }
}

fn sample_order_revision_proposal(
    buyer_pubkey: String,
    seller_pubkey: String,
    root_event_id: String,
    prev_event_id: String,
) -> RadrootsTradeOrderRevisionProposed {
    RadrootsTradeOrderRevisionProposed {
        revision_id: "revision-1".into(),
        order_id: "order-1".into(),
        listing_addr: format!("{KIND_LISTING}:{seller_pubkey}:AAAAAAAAAAAAAAAAAAAAAg"),
        buyer_pubkey,
        seller_pubkey,
        root_event_id,
        prev_event_id,
        items: vec![RadrootsTradeOrderItem {
            bin_id: "bin-1".into(),
            bin_count: 3,
        }],
        economics: RadrootsTradeOrderEconomics {
            quote_id: "revision-quote-1".into(),
            quote_version: 2,
            pricing_basis: RadrootsTradePricingBasis::ListingEvent,
            currency: RadrootsCoreCurrency::USD,
            items: vec![RadrootsTradeOrderEconomicItem {
                bin_id: "bin-1".into(),
                bin_count: 3,
                quantity_amount: decimal("1"),
                quantity_unit: RadrootsCoreUnit::Each,
                unit_price_amount: decimal("5"),
                unit_price_currency: RadrootsCoreCurrency::USD,
                line_subtotal: usd("15"),
            }],
            discounts: Vec::new(),
            adjustments: Vec::new(),
            subtotal: usd("15"),
            discount_total: usd("0"),
            adjustment_total: usd("0"),
            total: usd("15"),
        },
        reason: "update count".into(),
    }
}

fn sample_order_revision_decision(
    proposal: &RadrootsTradeOrderRevisionProposed,
    decision: RadrootsTradeOrderRevisionDecision,
) -> RadrootsTradeOrderRevisionDecisionEvent {
    RadrootsTradeOrderRevisionDecisionEvent {
        revision_id: proposal.revision_id.clone(),
        order_id: proposal.order_id.clone(),
        listing_addr: proposal.listing_addr.clone(),
        buyer_pubkey: proposal.buyer_pubkey.clone(),
        seller_pubkey: proposal.seller_pubkey.clone(),
        root_event_id: proposal.root_event_id.clone(),
        prev_event_id: "order-revision-proposal-event-1".into(),
        decision,
    }
}

fn sample_fulfillment_update(
    buyer_pubkey: String,
    seller_pubkey: String,
) -> RadrootsTradeFulfillmentUpdated {
    RadrootsTradeFulfillmentUpdated {
        order_id: "order-1".into(),
        listing_addr: format!("{KIND_LISTING}:{seller_pubkey}:AAAAAAAAAAAAAAAAAAAAAg"),
        buyer_pubkey,
        seller_pubkey,
        status: RadrootsActiveTradeFulfillmentState::ReadyForPickup,
    }
}

fn sample_order_cancellation(
    buyer_pubkey: String,
    seller_pubkey: String,
) -> RadrootsTradeOrderCancelled {
    RadrootsTradeOrderCancelled {
        order_id: "order-1".into(),
        listing_addr: format!("{KIND_LISTING}:{seller_pubkey}:AAAAAAAAAAAAAAAAAAAAAg"),
        buyer_pubkey,
        seller_pubkey,
        reason: "schedule changed".into(),
    }
}

fn sample_buyer_receipt(buyer_pubkey: String, seller_pubkey: String) -> RadrootsTradeBuyerReceipt {
    RadrootsTradeBuyerReceipt {
        order_id: "order-1".into(),
        listing_addr: format!("{KIND_LISTING}:{seller_pubkey}:AAAAAAAAAAAAAAAAAAAAAg"),
        buyer_pubkey,
        seller_pubkey,
        received: true,
        issue: None,
        received_at: 1_785_000_000,
    }
}

fn event_from_parts(
    id: &str,
    author: &str,
    created_at: u32,
    parts: WireEventParts,
) -> RadrootsNostrEvent {
    RadrootsNostrEvent {
        id: id.into(),
        author: author.into(),
        created_at,
        kind: parts.kind,
        tags: parts.tags,
        content: parts.content,
        sig: String::new(),
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
    assert_eq!(client.trade().transport(), SdkTransportMode::Radrootsd);
    #[cfg(feature = "radrootsd-client")]
    assert_eq!(client.radrootsd().transport(), SdkTransportMode::Radrootsd);
    assert_eq!(client.signer(), SignerConfig::LocalIdentity);
    assert_eq!(client.profile().signer(), SignerConfig::LocalIdentity);
    assert_eq!(client.farm().signer(), SignerConfig::LocalIdentity);
    assert_eq!(client.listing().signer(), SignerConfig::LocalIdentity);
    assert_eq!(client.trade().signer(), SignerConfig::LocalIdentity);
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
    let trade = client.trade();

    assert_eq!(client.config().environment, SdkEnvironment::Production);
    assert!(std::ptr::eq(profile.sdk(), &client));
    assert!(std::ptr::eq(farm.sdk(), &client));
    assert!(std::ptr::eq(listing.sdk(), &client));
    assert!(std::ptr::eq(trade.sdk(), &client));

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
    invalid_listing.d_tag.clear();
    assert!(listing.build_draft(&invalid_listing).is_err());
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
    let envelope_event = RadrootsNostrEvent {
        id: "trade-event-1".into(),
        author: "seller".into(),
        created_at: 2,
        kind: envelope.kind,
        tags: envelope.tags,
        content: envelope.content,
        sig: String::new(),
    };
    assert_eq!(
        client
            .trade()
            .parse_envelope(&envelope_event)
            .expect("trade envelope")
            .message_type,
        payload.message_type()
    );
    let parsed_addr = client
        .trade()
        .parse_listing_address(&listing_addr)
        .expect("listing address");
    assert_eq!(parsed_addr.listing_id, listing_value.d_tag);
}

#[test]
fn active_trade_facades_round_trip_all_draft_types() {
    let client =
        RadrootsSdkClient::from_config(RadrootsSdkConfig::production()).expect("sdk client");
    let trade = client.trade();
    let buyer_pubkey = "b".repeat(64);
    let seller_pubkey = "a".repeat(64);
    let root_event_id = "order-request-event-1";
    let decision_event_id = "order-decision-event-1";
    let proposal_event_id = "order-revision-proposal-event-1";
    let fulfillment_event_id = "fulfillment-event-1";

    let order = sample_order_request(buyer_pubkey.clone(), seller_pubkey.clone());
    let order_draft = trade
        .build_order_request_draft(&listing_event_ptr(), &order)
        .expect("order request draft");
    assert_eq!(order_draft.as_wire_parts().kind, KIND_TRADE_ORDER_REQUEST);
    let order_event = event_from_parts(
        root_event_id,
        &buyer_pubkey,
        1,
        order_draft.clone().into_wire_parts(),
    );
    let order_envelope = trade
        .parse_order_request(&order_event)
        .expect("order request envelope");
    assert_eq!(order_envelope.payload.economics.total, usd("10"));

    let decision = sample_order_decision(buyer_pubkey.clone(), seller_pubkey.clone());
    let decision_draft = trade
        .build_order_decision_draft(root_event_id, root_event_id, &decision)
        .expect("order decision draft");
    assert_eq!(
        decision_draft.as_wire_parts().kind,
        KIND_TRADE_ORDER_RESPONSE
    );
    let decision_event = event_from_parts(
        decision_event_id,
        &seller_pubkey,
        2,
        decision_draft.clone().into_wire_parts(),
    );
    assert_eq!(
        trade
            .parse_order_decision(&decision_event)
            .expect("order decision envelope")
            .payload
            .decision,
        decision.decision
    );

    let proposal = sample_order_revision_proposal(
        buyer_pubkey.clone(),
        seller_pubkey.clone(),
        root_event_id.into(),
        decision_event_id.into(),
    );
    let proposal_draft = trade
        .build_order_revision_proposal_draft(root_event_id, decision_event_id, &proposal)
        .expect("revision proposal draft");
    assert_eq!(
        proposal_draft.as_wire_parts().kind,
        KIND_TRADE_ORDER_REVISION
    );
    let proposal_event = event_from_parts(
        proposal_event_id,
        &seller_pubkey,
        3,
        proposal_draft.clone().into_wire_parts(),
    );
    assert_eq!(
        trade
            .parse_order_revision_proposal(&proposal_event)
            .expect("revision proposal envelope")
            .payload
            .economics
            .total,
        usd("15")
    );

    let revision_decision =
        sample_order_revision_decision(&proposal, RadrootsTradeOrderRevisionDecision::Accepted);
    let revision_decision_draft = trade
        .build_order_revision_decision_draft(
            root_event_id,
            revision_decision.prev_event_id.as_str(),
            &revision_decision,
        )
        .expect("revision decision draft");
    assert_eq!(
        revision_decision_draft.as_wire_parts().kind,
        KIND_TRADE_ORDER_REVISION_RESPONSE
    );
    let revision_decision_event = event_from_parts(
        "order-revision-decision-event-1",
        &buyer_pubkey,
        4,
        revision_decision_draft.clone().into_wire_parts(),
    );
    assert_eq!(
        trade
            .parse_order_revision_decision(&revision_decision_event)
            .expect("revision decision envelope")
            .payload
            .revision_id,
        revision_decision.revision_id
    );

    let fulfillment = sample_fulfillment_update(buyer_pubkey.clone(), seller_pubkey.clone());
    let fulfillment_draft = trade
        .build_fulfillment_update_draft(root_event_id, decision_event_id, &fulfillment)
        .expect("fulfillment draft");
    assert_eq!(
        fulfillment_draft.as_wire_parts().kind,
        KIND_TRADE_FULFILLMENT_UPDATE
    );
    let fulfillment_event = event_from_parts(
        fulfillment_event_id,
        &seller_pubkey,
        5,
        fulfillment_draft.clone().into_wire_parts(),
    );
    assert_eq!(
        trade
            .parse_fulfillment_update(&fulfillment_event)
            .expect("fulfillment envelope")
            .payload
            .status,
        fulfillment.status
    );

    let cancellation = sample_order_cancellation(buyer_pubkey.clone(), seller_pubkey.clone());
    let cancellation_draft = trade
        .build_order_cancellation_draft(root_event_id, decision_event_id, &cancellation)
        .expect("cancellation draft");
    assert_eq!(cancellation_draft.as_wire_parts().kind, KIND_TRADE_CANCEL);
    let cancellation_event = event_from_parts(
        "order-cancellation-event-1",
        &buyer_pubkey,
        6,
        cancellation_draft.clone().into_wire_parts(),
    );
    assert_eq!(
        trade
            .parse_order_cancellation(&cancellation_event)
            .expect("cancellation envelope")
            .payload
            .reason,
        cancellation.reason
    );

    let receipt = sample_buyer_receipt(buyer_pubkey.clone(), seller_pubkey.clone());
    let receipt_draft = trade
        .build_buyer_receipt_draft(root_event_id, fulfillment_event_id, &receipt)
        .expect("receipt draft");
    assert_eq!(receipt_draft.as_wire_parts().kind, KIND_TRADE_RECEIPT);
    let receipt_event = event_from_parts(
        "receipt-event-1",
        &buyer_pubkey,
        7,
        receipt_draft.clone().into_wire_parts(),
    );
    assert!(
        trade
            .parse_buyer_receipt(&receipt_event)
            .expect("receipt envelope")
            .payload
            .received
    );
}

#[test]
fn active_trade_draft_facades_return_encoder_errors() {
    let client =
        RadrootsSdkClient::from_config(RadrootsSdkConfig::production()).expect("sdk client");
    let trade = client.trade();
    let buyer_pubkey = "b".repeat(64);
    let seller_pubkey = "a".repeat(64);
    let root_event_id = "order-request-event-1";
    let decision_event_id = "order-decision-event-1";

    let mut invalid_order = sample_order_request(buyer_pubkey.clone(), seller_pubkey.clone());
    invalid_order.order_id.clear();
    assert!(
        trade
            .build_order_request_draft(&listing_event_ptr(), &invalid_order)
            .is_err()
    );

    let mut invalid_decision = sample_order_decision(buyer_pubkey.clone(), seller_pubkey.clone());
    invalid_decision.buyer_pubkey.clear();
    assert!(
        trade
            .build_order_decision_draft(root_event_id, root_event_id, &invalid_decision)
            .is_err()
    );

    let proposal = sample_order_revision_proposal(
        buyer_pubkey.clone(),
        seller_pubkey.clone(),
        root_event_id.into(),
        decision_event_id.into(),
    );
    assert!(
        trade
            .build_order_revision_proposal_draft("different-root", decision_event_id, &proposal)
            .is_err()
    );

    let revision_decision =
        sample_order_revision_decision(&proposal, RadrootsTradeOrderRevisionDecision::Accepted);
    assert!(
        trade
            .build_order_revision_decision_draft(
                root_event_id,
                "different-prev",
                &revision_decision,
            )
            .is_err()
    );

    let mut fulfillment = sample_fulfillment_update(buyer_pubkey.clone(), seller_pubkey.clone());
    fulfillment.status = RadrootsActiveTradeFulfillmentState::AcceptedNotFulfilled;
    assert!(
        trade
            .build_fulfillment_update_draft(root_event_id, decision_event_id, &fulfillment)
            .is_err()
    );

    let mut cancellation = sample_order_cancellation(buyer_pubkey.clone(), seller_pubkey.clone());
    cancellation.reason.clear();
    assert!(
        trade
            .build_order_cancellation_draft(root_event_id, decision_event_id, &cancellation)
            .is_err()
    );

    let mut receipt = sample_buyer_receipt(buyer_pubkey, seller_pubkey);
    receipt.received = false;
    assert!(
        trade
            .build_buyer_receipt_draft(root_event_id, decision_event_id, &receipt)
            .is_err()
    );
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
            operation: "trade.publish",
        }
        .to_string(),
        SdkPublishError::UnsupportedSignerMode {
            transport: SdkTransportMode::RelayDirect,
            signer: SignerConfig::DraftOnly,
            required: SignerConfig::LocalIdentity,
            operation: "trade.publish",
        }
        .to_string(),
        SdkPublishError::Relay("relay failed".into()).to_string(),
        SdkPublishError::RelaySetup {
            transport: SdkTransportMode::RelayDirect,
            operation: "trade.publish",
            target_relays: Vec::new(),
            error: "setup failed".into(),
        }
        .to_string(),
        SdkPublishError::RelaySetup {
            transport: SdkTransportMode::RelayDirect,
            operation: "trade.publish",
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
