use radroots_core::{
    RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreQuantity,
    RadrootsCoreQuantityPrice, RadrootsCoreUnit,
};
use radroots_events::farm::{RadrootsFarm, RadrootsFarmRef};
use radroots_events::ids::{RadrootsEventId, RadrootsPublicKey};
use radroots_events::kinds::{
    KIND_FARM, KIND_LISTING, KIND_ORDER_CANCELLATION, KIND_ORDER_DECISION, KIND_ORDER_REQUEST,
    KIND_ORDER_REVISION_DECISION, KIND_ORDER_REVISION_PROPOSAL, KIND_PROFILE,
};
use radroots_events::listing::{
    RadrootsListing, RadrootsListingAvailability, RadrootsListingBin,
    RadrootsListingDeliveryMethod, RadrootsListingProduct, RadrootsListingPublicLocation,
    RadrootsListingStatus,
};
use radroots_events::order::{
    RadrootsOrderCancellation, RadrootsOrderDecision, RadrootsOrderDecisionOutcome,
    RadrootsOrderEconomicItem, RadrootsOrderEconomics, RadrootsOrderInventoryCommitment,
    RadrootsOrderItem, RadrootsOrderPricingBasis, RadrootsOrderRequest,
    RadrootsOrderRevisionDecision, RadrootsOrderRevisionOutcome, RadrootsOrderRevisionProposal,
};
use radroots_events::profile::{RadrootsProfile, RadrootsProfileType};
use radroots_events::resource_area::RadrootsResourceAreaRef;
use radroots_sdk::protocol::events::{RadrootsNostrEvent, RadrootsNostrEventPtr};
use radroots_sdk::protocol::wire::WireEventParts;
use radroots_sdk::protocol::{farm, listing, order, profile};

fn sample_profile() -> RadrootsProfile {
    RadrootsProfile {
        name: "North Farm".into(),
        display_name: Some("North Farm".into()),
        nip05: None,
        about: Some("Organic coffee".into()),
        website: Some("https://example.com".into()),
        picture: None,
        banner: None,
        lud06: None,
        lud16: None,
        bot: None,
    }
}

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
        location: Some(RadrootsListingPublicLocation {
            primary: "North Farm".into(),
            city: None,
            region: None,
            country: None,
            geohash: "9q8yy".into(),
        }),
        images: None,
    }
}

fn listing_event(listing_value: &RadrootsListing) -> RadrootsNostrEvent {
    let parts = listing::build_draft(listing_value).expect("listing draft");
    RadrootsNostrEvent {
        id: "event-1".into(),
        author: listing_value.farm.pubkey.clone(),
        created_at: 1,
        kind: parts.as_wire_parts().kind,
        tags: parts.as_wire_parts().tags.clone(),
        content: parts.as_wire_parts().content.clone(),
        sig: String::new(),
    }
}

#[test]
fn listing_facade_rejects_malformed_resource_area_refs() {
    let mut listing_value = sample_listing();
    listing_value.resource_area = Some(RadrootsResourceAreaRef {
        pubkey: "a".repeat(64),
        d_tag: "bad d tag".to_owned(),
    });

    assert!(listing::build_draft(&listing_value).is_err());
}

fn listing_event_ptr() -> RadrootsNostrEventPtr {
    RadrootsNostrEventPtr {
        id: core::iter::repeat_n('a', 64).collect(),
        relays: Some("wss://listing.relay.example".into()),
    }
}

fn public_key(character: char) -> RadrootsPublicKey {
    core::iter::repeat_n(character, 64)
        .collect::<String>()
        .parse()
        .expect("public key")
}

fn event_id(character: char) -> RadrootsEventId {
    core::iter::repeat_n(character, 64)
        .collect::<String>()
        .parse()
        .expect("event id")
}

fn sample_order_request() -> RadrootsOrderRequest {
    let seller_pubkey = public_key('a');

    RadrootsOrderRequest {
        order_id: "order-1".parse().expect("order id"),
        listing_addr: format!("{KIND_LISTING}:{seller_pubkey}:AAAAAAAAAAAAAAAAAAAAAg")
            .parse()
            .expect("listing address"),
        buyer_pubkey: public_key('b'),
        seller_pubkey,
        items: vec![RadrootsOrderItem {
            bin_id: "bin-1".parse().expect("bin id"),
            bin_count: 2,
        }],
        economics: RadrootsOrderEconomics {
            quote_id: "quote-1".parse().expect("quote id"),
            quote_version: 1,
            pricing_basis: RadrootsOrderPricingBasis::ListingEvent,
            currency: RadrootsCoreCurrency::USD,
            items: vec![RadrootsOrderEconomicItem {
                bin_id: "bin-1".parse().expect("bin id"),
                bin_count: 2,
                quantity_amount: RadrootsCoreDecimal::from(1u32),
                quantity_unit: RadrootsCoreUnit::Each,
                unit_price_amount: RadrootsCoreDecimal::from(5u32),
                unit_price_currency: RadrootsCoreCurrency::USD,
                line_subtotal: RadrootsCoreMoney::new(
                    RadrootsCoreDecimal::from(10u32),
                    RadrootsCoreCurrency::USD,
                ),
            }],
            discounts: Vec::new(),
            adjustments: Vec::new(),
            subtotal: RadrootsCoreMoney::new(
                RadrootsCoreDecimal::from(10u32),
                RadrootsCoreCurrency::USD,
            ),
            discount_total: RadrootsCoreMoney::new(
                RadrootsCoreDecimal::from(0u32),
                RadrootsCoreCurrency::USD,
            ),
            adjustment_total: RadrootsCoreMoney::new(
                RadrootsCoreDecimal::from(0u32),
                RadrootsCoreCurrency::USD,
            ),
            total: RadrootsCoreMoney::new(
                RadrootsCoreDecimal::from(10u32),
                RadrootsCoreCurrency::USD,
            ),
        },
    }
}

fn sample_order_decision() -> RadrootsOrderDecision {
    let request = sample_order_request();

    RadrootsOrderDecision {
        order_id: request.order_id,
        listing_addr: request.listing_addr,
        buyer_pubkey: request.buyer_pubkey,
        seller_pubkey: request.seller_pubkey,
        decision: RadrootsOrderDecisionOutcome::Accepted {
            inventory_commitments: vec![RadrootsOrderInventoryCommitment {
                bin_id: "bin-1".parse().expect("bin id"),
                bin_count: 2,
            }],
        },
    }
}

fn sample_order_revision_proposal(
    root_event_id: &RadrootsEventId,
    prev_event_id: &RadrootsEventId,
) -> RadrootsOrderRevisionProposal {
    let request = sample_order_request();

    RadrootsOrderRevisionProposal {
        revision_id: "revision-1".parse().expect("revision id"),
        order_id: request.order_id,
        listing_addr: request.listing_addr,
        buyer_pubkey: request.buyer_pubkey,
        seller_pubkey: request.seller_pubkey,
        root_event_id: root_event_id.clone(),
        prev_event_id: prev_event_id.clone(),
        items: vec![RadrootsOrderItem {
            bin_id: "bin-1".parse().expect("bin id"),
            bin_count: 2,
        }],
        economics: request.economics,
        reason: "more quantity".into(),
    }
}

fn sample_order_revision_decision(
    proposal: &RadrootsOrderRevisionProposal,
    prev_event_id: &RadrootsEventId,
) -> RadrootsOrderRevisionDecision {
    RadrootsOrderRevisionDecision {
        revision_id: proposal.revision_id.clone(),
        order_id: proposal.order_id.clone(),
        listing_addr: proposal.listing_addr.clone(),
        buyer_pubkey: proposal.buyer_pubkey.clone(),
        seller_pubkey: proposal.seller_pubkey.clone(),
        root_event_id: proposal.root_event_id.clone(),
        prev_event_id: prev_event_id.clone(),
        decision: RadrootsOrderRevisionOutcome::Accepted,
    }
}

fn sample_order_cancellation() -> RadrootsOrderCancellation {
    let request = sample_order_request();

    RadrootsOrderCancellation {
        order_id: request.order_id,
        listing_addr: request.listing_addr,
        buyer_pubkey: request.buyer_pubkey,
        seller_pubkey: request.seller_pubkey,
        reason: "buyer changed plan".into(),
    }
}

fn order_event_from_parts(
    id_character: char,
    author: String,
    created_at: u32,
    parts: WireEventParts,
) -> RadrootsNostrEvent {
    RadrootsNostrEvent {
        id: core::iter::repeat_n(id_character, 64).collect(),
        author,
        created_at,
        kind: parts.kind,
        tags: parts.tags,
        content: parts.content,
        sig: String::new(),
    }
}

#[test]
fn profile_build_draft_wraps_profile_encoder() {
    let parts =
        profile::build_draft(&sample_profile(), Some(RadrootsProfileType::Farm)).expect("profile");

    assert_eq!(parts.kind, KIND_PROFILE);
    assert!(parts.tags.iter().any(|tag| {
        tag.first().map(|value| value.as_str()) == Some("t")
            && tag.get(1).map(|value| value.as_str()) == Some("radroots:type:farm")
    }));
}

#[test]
fn farm_build_draft_wraps_farm_encoder() {
    let parts = farm::build_draft(&sample_farm()).expect("farm");

    assert_eq!(parts.kind, KIND_FARM);
    assert!(
        parts
            .tags
            .iter()
            .any(|tag| tag.first().map(|value| value.as_str()) == Some("d"))
    );
}

#[test]
fn listing_facade_wraps_build_parse_and_validate() {
    let listing_value = sample_listing();
    let tags = listing::build_tags(&listing_value).expect("listing tags");
    assert!(!tags.is_empty());
    let parts = listing::build_draft(&listing_value).expect("listing draft");
    assert_eq!(parts.clone().into_wire_parts().kind, KIND_LISTING);

    let event = listing_event(&listing_value);
    let parsed = listing::parse_event(&event).expect("parsed listing");
    assert_eq!(parsed.d_tag, listing_value.d_tag);

    let validated = order::validate_listing_event(&event).expect("validated listing");
    assert_eq!(validated.listing_id, listing_value.d_tag);
    assert_eq!(event.kind, KIND_LISTING);
}

#[test]
fn listing_parse_rejects_non_listing_kind() {
    let listing_value = sample_listing();
    let mut event = listing_event(&listing_value);
    event.kind = KIND_PROFILE;

    assert_eq!(
        listing::parse_event(&event).expect_err("listing kind error"),
        listing::RadrootsListingParseError::InvalidKind(KIND_PROFILE)
    );
}

#[test]
fn order_facade_wraps_build_parse_and_address_ops() {
    let listing_value = sample_listing();
    let seller_pubkey = "a".repeat(64);
    let listing_addr = format!("{KIND_LISTING}:{seller_pubkey}:{}", listing_value.d_tag);
    let payload = sample_order_request();
    let parts =
        order::build_order_request_draft(&listing_event_ptr(), &payload).expect("order draft");

    assert_eq!(parts.as_wire_parts().kind, KIND_ORDER_REQUEST);
    assert_eq!(parts.clone().into_wire_parts().kind, KIND_ORDER_REQUEST);

    let parsed_addr = order::parse_listing_address(&listing_addr).expect("listing address");
    assert_eq!(parsed_addr, listing_addr);

    let event = order_event_from_parts(
        'b',
        payload.buyer_pubkey.to_string(),
        2,
        parts.into_wire_parts(),
    );
    let envelope = order::parse_order_request(&event).expect("order envelope");
    assert_eq!(envelope.payload.order_id, payload.order_id);
    assert_eq!(envelope.payload.listing_addr, listing_addr);

    let root_event_id = event_id('c');
    let previous_event_id = event_id('d');
    let decision = sample_order_decision();
    let decision_parts =
        order::build_order_decision_draft(&root_event_id, &root_event_id, &decision)
            .expect("decision draft");
    assert_eq!(decision_parts.as_wire_parts().kind, KIND_ORDER_DECISION);
    assert_eq!(
        decision_parts.clone().into_wire_parts().kind,
        KIND_ORDER_DECISION
    );
    let decision_event = order_event_from_parts(
        'e',
        decision.seller_pubkey.to_string(),
        3,
        decision_parts.into_wire_parts(),
    );
    let decision_envelope =
        order::parse_order_decision(&decision_event).expect("decision envelope");
    assert_eq!(decision_envelope.payload.order_id, decision.order_id);

    let proposal = sample_order_revision_proposal(&root_event_id, &previous_event_id);
    let proposal_parts =
        order::build_order_revision_proposal_draft(&root_event_id, &previous_event_id, &proposal)
            .expect("proposal draft");
    assert_eq!(
        proposal_parts.as_wire_parts().kind,
        KIND_ORDER_REVISION_PROPOSAL
    );
    assert_eq!(
        proposal_parts.clone().into_wire_parts().kind,
        KIND_ORDER_REVISION_PROPOSAL
    );
    let proposal_event = order_event_from_parts(
        'f',
        proposal.seller_pubkey.to_string(),
        4,
        proposal_parts.into_wire_parts(),
    );
    let proposal_envelope =
        order::parse_order_revision_proposal(&proposal_event).expect("proposal envelope");
    assert_eq!(proposal_envelope.payload.revision_id, proposal.revision_id);

    let revision_decision = sample_order_revision_decision(&proposal, &previous_event_id);
    let revision_decision_parts = order::build_order_revision_decision_draft(
        &root_event_id,
        &previous_event_id,
        &revision_decision,
    )
    .expect("revision decision draft");
    assert_eq!(
        revision_decision_parts.as_wire_parts().kind,
        KIND_ORDER_REVISION_DECISION
    );
    assert_eq!(
        revision_decision_parts.clone().into_wire_parts().kind,
        KIND_ORDER_REVISION_DECISION
    );
    let revision_decision_event = order_event_from_parts(
        '0',
        revision_decision.buyer_pubkey.to_string(),
        5,
        revision_decision_parts.into_wire_parts(),
    );
    let revision_decision_envelope = order::parse_order_revision_decision(&revision_decision_event)
        .expect("revision decision envelope");
    assert_eq!(
        revision_decision_envelope.payload.revision_id,
        revision_decision.revision_id
    );

    let cancellation = sample_order_cancellation();
    let cancellation_parts =
        order::build_order_cancellation_draft(&root_event_id, &previous_event_id, &cancellation)
            .expect("cancellation draft");
    assert_eq!(
        cancellation_parts.as_wire_parts().kind,
        KIND_ORDER_CANCELLATION
    );
    assert_eq!(
        cancellation_parts.clone().into_wire_parts().kind,
        KIND_ORDER_CANCELLATION
    );
    let cancellation_event = order_event_from_parts(
        '1',
        cancellation.buyer_pubkey.to_string(),
        6,
        cancellation_parts.into_wire_parts(),
    );
    let cancellation_envelope =
        order::parse_order_cancellation(&cancellation_event).expect("cancellation envelope");
    assert_eq!(
        cancellation_envelope.payload.order_id,
        cancellation.order_id
    );
}

#[test]
fn order_facade_surfaces_order_draft_build_errors() {
    let root_event_id = event_id('c');
    let previous_event_id = event_id('d');

    let mut decision = sample_order_decision();
    decision.decision = RadrootsOrderDecisionOutcome::Accepted {
        inventory_commitments: Vec::new(),
    };
    assert!(order::build_order_decision_draft(&root_event_id, &root_event_id, &decision).is_err());

    let mut proposal = sample_order_revision_proposal(&root_event_id, &previous_event_id);
    proposal.reason = " ".into();
    assert!(
        order::build_order_revision_proposal_draft(&root_event_id, &previous_event_id, &proposal)
            .is_err()
    );

    let proposal = sample_order_revision_proposal(&root_event_id, &previous_event_id);
    let mut revision_decision = sample_order_revision_decision(&proposal, &previous_event_id);
    revision_decision.decision = RadrootsOrderRevisionOutcome::Declined { reason: " ".into() };
    assert!(
        order::build_order_revision_decision_draft(
            &root_event_id,
            &previous_event_id,
            &revision_decision
        )
        .is_err()
    );

    let mut cancellation = sample_order_cancellation();
    cancellation.reason = " ".into();
    assert!(
        order::build_order_cancellation_draft(&root_event_id, &previous_event_id, &cancellation)
            .is_err()
    );
}
