#![cfg(all(
    feature = "identity-models",
    feature = "relay-client",
    feature = "signing"
))]

use futures::{SinkExt, StreamExt};
use nostr::{ClientMessage, JsonUtil, RelayMessage};
use radroots_core::{
    RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreQuantity,
    RadrootsCoreQuantityPrice, RadrootsCoreUnit,
};
use radroots_events::ids::{RadrootsEventId, RadrootsPublicKey};
use radroots_sdk::farm::{RadrootsFarm, RadrootsFarmLocation, RadrootsFarmRef};
use radroots_sdk::identity::RadrootsIdentity;
use radroots_sdk::listing::{
    RadrootsListing, RadrootsListingAvailability, RadrootsListingBin,
    RadrootsListingDeliveryMethod, RadrootsListingLocation, RadrootsListingProduct,
    RadrootsListingStatus,
};
use radroots_sdk::order::{
    RadrootsOrderCancellation, RadrootsOrderDecision, RadrootsOrderDecisionOutcome,
    RadrootsOrderEconomicItem, RadrootsOrderEconomics, RadrootsOrderFulfillmentState,
    RadrootsOrderFulfillmentUpdate, RadrootsOrderInventoryCommitment, RadrootsOrderItem,
    RadrootsOrderPricingBasis, RadrootsOrderReceipt, RadrootsOrderRequest,
    RadrootsOrderRevisionDecision, RadrootsOrderRevisionOutcome, RadrootsOrderRevisionProposal,
};
use radroots_sdk::profile::{RadrootsProfile, RadrootsProfileType};
use radroots_sdk::{
    RadrootsNostrEventPtr, RadrootsSdkClient, RadrootsSdkConfig, RelayConfig, SdkEnvironment,
    SdkPublishError, SdkTransportMode, SdkTransportReceipt, SignerConfig,
};
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio_tungstenite::tungstenite::Message;

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

struct AckRelay {
    url: String,
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl AckRelay {
    async fn spawn() -> TestResult<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let url = format!("ws://{addr}");
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut shutdown_rx => break,
                    accept = listener.accept() => {
                        let Ok((stream, _)) = accept else {
                            break;
                        };
                        tokio::spawn(async move {
                            let Ok(websocket) = tokio_tungstenite::accept_async(stream).await else {
                                return;
                            };
                            let (mut writer, mut reader) = websocket.split();
                            while let Some(message) = reader.next().await {
                                let Ok(message) = message else {
                                    break;
                                };
                                let Message::Text(text) = message else {
                                    continue;
                                };
                                let Ok(client_message) = ClientMessage::from_json(text.as_str()) else {
                                    continue;
                                };
                                if let ClientMessage::Event(event) = client_message {
                                    let relay_message =
                                        RelayMessage::ok(event.id, true, "").as_json();
                                    if writer
                                        .send(Message::Text(relay_message.into()))
                                        .await
                                        .is_err()
                                    {
                                        break;
                                    }
                                }
                            }
                        });
                    }
                }
            }
        });

        Ok(Self {
            url,
            shutdown_tx: Some(shutdown_tx),
        })
    }

    fn url(&self) -> &str {
        self.url.as_str()
    }
}

impl Drop for AckRelay {
    fn drop(&mut self) {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }
    }
}

fn sample_listing() -> RadrootsListing {
    RadrootsListing {
        d_tag: "AAAAAAAAAAAAAAAAAAAAAg".parse().expect("listing d tag"),
        published_at: None,
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

fn sample_farm() -> RadrootsFarm {
    RadrootsFarm {
        d_tag: "AAAAAAAAAAAAAAAAAAAAAA".into(),
        name: "North Farm".into(),
        about: Some("Vegetable farm".into()),
        website: None,
        picture: None,
        banner: None,
        location: Some(RadrootsFarmLocation {
            primary: Some("North Road".into()),
            city: None,
            region: None,
            country: Some("US".into()),
            gcs: None,
        }),
        tags: Some(vec!["vegetables".into()]),
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
        id: event_id_wire('a'),
        relays: Some("wss://listing.relay.example".into()),
    }
}

fn public_key(value: String) -> RadrootsPublicKey {
    value.parse().expect("public key")
}

fn event_id(character: char) -> RadrootsEventId {
    core::iter::repeat_n(character, 64)
        .collect::<String>()
        .parse()
        .expect("event id")
}

fn event_id_wire(character: char) -> String {
    event_id(character).into_string()
}

fn sample_order_request(buyer_pubkey: String, seller_pubkey: String) -> RadrootsOrderRequest {
    let buyer_pubkey = public_key(buyer_pubkey);
    let seller_pubkey = public_key(seller_pubkey);
    RadrootsOrderRequest {
        order_id: "order-1".parse().expect("order id"),
        listing_addr: format!("30402:{seller_pubkey}:AAAAAAAAAAAAAAAAAAAAAg")
            .parse()
            .expect("listing address"),
        buyer_pubkey,
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

fn sample_order_decision(buyer_pubkey: String, seller_pubkey: String) -> RadrootsOrderDecision {
    let buyer_pubkey = public_key(buyer_pubkey);
    let seller_pubkey = public_key(seller_pubkey);
    RadrootsOrderDecision {
        order_id: "order-1".parse().expect("order id"),
        listing_addr: format!("30402:{seller_pubkey}:AAAAAAAAAAAAAAAAAAAAAg")
            .parse()
            .expect("listing address"),
        buyer_pubkey,
        seller_pubkey,
        decision: RadrootsOrderDecisionOutcome::Accepted {
            inventory_commitments: vec![RadrootsOrderInventoryCommitment {
                bin_id: "bin-1".parse().expect("bin id"),
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
) -> RadrootsOrderRevisionProposal {
    let buyer_pubkey = public_key(buyer_pubkey);
    let seller_pubkey = public_key(seller_pubkey);
    RadrootsOrderRevisionProposal {
        revision_id: "revision-1".parse().expect("revision id"),
        order_id: "order-1".parse().expect("order id"),
        listing_addr: format!("30402:{seller_pubkey}:AAAAAAAAAAAAAAAAAAAAAg")
            .parse()
            .expect("listing address"),
        buyer_pubkey,
        seller_pubkey,
        root_event_id: root_event_id.parse().expect("root event id"),
        prev_event_id: prev_event_id.parse().expect("previous event id"),
        items: vec![RadrootsOrderItem {
            bin_id: "bin-1".parse().expect("bin id"),
            bin_count: 3,
        }],
        economics: RadrootsOrderEconomics {
            quote_id: "revision-quote-1".parse().expect("revision quote id"),
            quote_version: 2,
            pricing_basis: RadrootsOrderPricingBasis::ListingEvent,
            currency: RadrootsCoreCurrency::USD,
            items: vec![RadrootsOrderEconomicItem {
                bin_id: "bin-1".parse().expect("bin id"),
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
    proposal: &RadrootsOrderRevisionProposal,
    decision: RadrootsOrderRevisionOutcome,
) -> RadrootsOrderRevisionDecision {
    RadrootsOrderRevisionDecision {
        revision_id: proposal.revision_id.clone(),
        order_id: proposal.order_id.clone(),
        listing_addr: proposal.listing_addr.clone(),
        buyer_pubkey: proposal.buyer_pubkey.clone(),
        seller_pubkey: proposal.seller_pubkey.clone(),
        root_event_id: proposal.root_event_id.clone(),
        prev_event_id: event_id('3'),
        decision,
    }
}

fn sample_fulfillment_update(
    buyer_pubkey: String,
    seller_pubkey: String,
) -> RadrootsOrderFulfillmentUpdate {
    let buyer_pubkey = public_key(buyer_pubkey);
    let seller_pubkey = public_key(seller_pubkey);
    RadrootsOrderFulfillmentUpdate {
        order_id: "order-1".parse().expect("order id"),
        listing_addr: format!("30402:{seller_pubkey}:AAAAAAAAAAAAAAAAAAAAAg")
            .parse()
            .expect("listing address"),
        buyer_pubkey,
        seller_pubkey,
        status: RadrootsOrderFulfillmentState::ReadyForPickup,
    }
}

fn sample_order_cancellation(
    buyer_pubkey: String,
    seller_pubkey: String,
) -> RadrootsOrderCancellation {
    let buyer_pubkey = public_key(buyer_pubkey);
    let seller_pubkey = public_key(seller_pubkey);
    RadrootsOrderCancellation {
        order_id: "order-1".parse().expect("order id"),
        listing_addr: format!("30402:{seller_pubkey}:AAAAAAAAAAAAAAAAAAAAAg")
            .parse()
            .expect("listing address"),
        buyer_pubkey,
        seller_pubkey,
        reason: "schedule changed".into(),
    }
}

fn sample_buyer_receipt(buyer_pubkey: String, seller_pubkey: String) -> RadrootsOrderReceipt {
    let buyer_pubkey = public_key(buyer_pubkey);
    let seller_pubkey = public_key(seller_pubkey);
    RadrootsOrderReceipt {
        order_id: "order-1".parse().expect("order id"),
        listing_addr: format!("30402:{seller_pubkey}:AAAAAAAAAAAAAAAAAAAAAg")
            .parse()
            .expect("listing address"),
        buyer_pubkey,
        seller_pubkey,
        received: true,
        issue: None,
        received_at: 1_785_000_000,
    }
}

#[tokio::test]
async fn relay_direct_farm_publish_accepts_sdk_built_draft() -> TestResult<()> {
    let relay = AckRelay::spawn().await?;
    let identity = RadrootsIdentity::generate();
    let mut config = RadrootsSdkConfig::for_environment(SdkEnvironment::Custom);
    config.transport = SdkTransportMode::RelayDirect;
    config.signer = SignerConfig::LocalIdentity;
    config.relay = RelayConfig {
        urls: vec![relay.url().to_owned()],
    };
    let client = RadrootsSdkClient::from_config(config)?;
    let draft = client.farm().build_draft(&sample_farm())?;

    let receipt = client
        .farm()
        .publish_draft_with_identity(&identity, draft)
        .await?;

    assert_eq!(receipt.transport, SdkTransportMode::RelayDirect);
    assert_eq!(receipt.event_kind, Some(30340));
    assert!(receipt.event_id.is_some());
    match receipt.transport_receipt {
        SdkTransportReceipt::RelayDirect(relay_receipt) => {
            assert_eq!(
                receipt.event_id.as_deref(),
                Some(relay_receipt.event_id.as_str())
            );
            assert_eq!(relay_receipt.event.kind, 30340);
            assert_eq!(relay_receipt.event.author, identity.public_key_hex());
            assert_eq!(
                relay_receipt.event.tags,
                vec![
                    vec!["d".to_owned(), "AAAAAAAAAAAAAAAAAAAAAA".to_owned()],
                    vec!["t".to_owned(), "vegetables".to_owned()]
                ]
            );
            assert_eq!(relay_receipt.target_relays, vec![relay.url().to_owned()]);
            assert_eq!(relay_receipt.connected_relays, vec![relay.url().to_owned()]);
            assert_eq!(
                relay_receipt.acknowledged_relays,
                vec![relay.url().to_owned()]
            );
            assert!(relay_receipt.failed_relays.is_empty());
        }
        SdkTransportReceipt::Radrootsd(_) => panic!("unexpected radrootsd receipt"),
    }

    Ok(())
}

#[tokio::test]
async fn relay_direct_order_request_publish_accepts_sdk_built_draft() -> TestResult<()> {
    let relay = AckRelay::spawn().await?;
    let buyer_identity = RadrootsIdentity::generate();
    let seller_identity = RadrootsIdentity::generate();
    let listing_event = listing_event_ptr();
    let payload = sample_order_request(
        buyer_identity.public_key_hex(),
        seller_identity.public_key_hex(),
    );
    let mut config = RadrootsSdkConfig::for_environment(SdkEnvironment::Custom);
    config.transport = SdkTransportMode::RelayDirect;
    config.signer = SignerConfig::LocalIdentity;
    config.relay = RelayConfig {
        urls: vec![relay.url().to_owned()],
    };
    let client = RadrootsSdkClient::from_config(config)?;
    let draft = client
        .order()
        .build_order_request_draft(&listing_event, &payload)?;
    assert_eq!(draft.as_wire_parts().kind, 3422);

    let receipt = client
        .order()
        .publish_order_request_draft_with_identity(&buyer_identity, draft)
        .await?;

    assert_eq!(receipt.transport, SdkTransportMode::RelayDirect);
    assert_eq!(receipt.event_kind, Some(3422));
    assert!(receipt.event_id.is_some());
    match receipt.transport_receipt {
        SdkTransportReceipt::RelayDirect(relay_receipt) => {
            assert_eq!(
                receipt.event_id.as_deref(),
                Some(relay_receipt.event_id.as_str())
            );
            assert_eq!(receipt.event_kind, Some(relay_receipt.event_kind));
            assert_eq!(relay_receipt.event.kind, 3422);
            assert_eq!(relay_receipt.event_id, relay_receipt.event.id);
            assert_eq!(relay_receipt.signature, relay_receipt.event.sig);
            assert_eq!(relay_receipt.created_at, relay_receipt.event.created_at);
            assert_eq!(relay_receipt.event.author, buyer_identity.public_key_hex());
            assert!(
                relay_receipt
                    .event
                    .tags
                    .contains(&vec!["p".to_owned(), seller_identity.public_key_hex()])
            );
            assert!(
                relay_receipt
                    .event
                    .tags
                    .contains(&vec!["a".to_owned(), payload.listing_addr.to_string()])
            );
            assert!(
                relay_receipt
                    .event
                    .tags
                    .contains(&vec!["d".to_owned(), payload.order_id.to_string()])
            );
            assert!(relay_receipt.event.tags.contains(&vec![
                "listing_event".to_owned(),
                listing_event.id.clone(),
                listing_event.relays.clone().expect("listing relay")
            ]));
            assert_eq!(relay_receipt.target_relays, vec![relay.url().to_owned()]);
            assert_eq!(relay_receipt.connected_relays, vec![relay.url().to_owned()]);
            assert_eq!(
                relay_receipt.acknowledged_relays,
                vec![relay.url().to_owned()]
            );
            assert!(relay_receipt.failed_relays.is_empty());
            let envelope = client
                .order()
                .parse_order_request(&relay_receipt.event)
                .expect("order request");
            assert_eq!(envelope.order_id, payload.order_id);
            assert_eq!(envelope.listing_addr, payload.listing_addr);
            assert_eq!(envelope.payload.economics.quote_id, "quote-1");
            assert_eq!(envelope.payload.economics.total, usd("10"));
        }
        SdkTransportReceipt::Radrootsd(_) => panic!("unexpected radrootsd receipt"),
    }

    Ok(())
}

#[tokio::test]
async fn relay_direct_order_decision_publish_accepts_sdk_built_draft() -> TestResult<()> {
    let relay = AckRelay::spawn().await?;
    let buyer_identity = RadrootsIdentity::generate();
    let seller_identity = RadrootsIdentity::generate();
    let root_event_id = event_id('1');
    let payload = sample_order_decision(
        buyer_identity.public_key_hex(),
        seller_identity.public_key_hex(),
    );
    let mut config = RadrootsSdkConfig::for_environment(SdkEnvironment::Custom);
    config.transport = SdkTransportMode::RelayDirect;
    config.signer = SignerConfig::LocalIdentity;
    config.relay = RelayConfig {
        urls: vec![relay.url().to_owned()],
    };
    let client = RadrootsSdkClient::from_config(config)?;
    let draft =
        client
            .order()
            .build_order_decision_draft(&root_event_id, &root_event_id, &payload)?;
    assert_eq!(draft.as_wire_parts().kind, 3423);

    let receipt = client
        .order()
        .publish_order_decision_draft_with_identity(&seller_identity, draft)
        .await?;

    assert_eq!(receipt.transport, SdkTransportMode::RelayDirect);
    assert_eq!(receipt.event_kind, Some(3423));
    assert!(receipt.event_id.is_some());
    match receipt.transport_receipt {
        SdkTransportReceipt::RelayDirect(relay_receipt) => {
            assert_eq!(
                receipt.event_id.as_deref(),
                Some(relay_receipt.event_id.as_str())
            );
            assert_eq!(receipt.event_kind, Some(relay_receipt.event_kind));
            assert_eq!(relay_receipt.event.kind, 3423);
            assert_eq!(relay_receipt.event.author, seller_identity.public_key_hex());
            assert!(
                relay_receipt
                    .event
                    .tags
                    .contains(&vec!["p".to_owned(), buyer_identity.public_key_hex()])
            );
            assert!(
                relay_receipt
                    .event
                    .tags
                    .contains(&vec!["a".to_owned(), payload.listing_addr.to_string()])
            );
            assert!(
                relay_receipt
                    .event
                    .tags
                    .contains(&vec!["d".to_owned(), payload.order_id.to_string()])
            );
            assert!(
                relay_receipt
                    .event
                    .tags
                    .contains(&vec!["e_root".to_owned(), root_event_id.to_string()])
            );
            assert!(
                relay_receipt
                    .event
                    .tags
                    .contains(&vec!["e_prev".to_owned(), root_event_id.to_string()])
            );
            assert_eq!(relay_receipt.target_relays, vec![relay.url().to_owned()]);
            assert_eq!(relay_receipt.connected_relays, vec![relay.url().to_owned()]);
            assert_eq!(
                relay_receipt.acknowledged_relays,
                vec![relay.url().to_owned()]
            );
            assert!(relay_receipt.failed_relays.is_empty());
            let envelope = client
                .order()
                .parse_order_decision(&relay_receipt.event)
                .expect("order decision");
            assert_eq!(envelope.order_id, payload.order_id);
            assert_eq!(envelope.listing_addr, payload.listing_addr);
            assert_eq!(envelope.payload.decision, payload.decision);
        }
        SdkTransportReceipt::Radrootsd(_) => panic!("unexpected radrootsd receipt"),
    }

    Ok(())
}

#[tokio::test]
async fn relay_direct_order_revision_publish_accepts_sdk_built_payloads() -> TestResult<()> {
    let relay = AckRelay::spawn().await?;
    let buyer_identity = RadrootsIdentity::generate();
    let seller_identity = RadrootsIdentity::generate();
    let buyer_pubkey = buyer_identity.public_key_hex();
    let seller_pubkey = seller_identity.public_key_hex();
    let root_event_id = event_id('1');
    let decision_event_id = event_id('2');
    let proposal = sample_order_revision_proposal(
        buyer_pubkey.clone(),
        seller_pubkey.clone(),
        root_event_id.to_string(),
        decision_event_id.to_string(),
    );
    let decision =
        sample_order_revision_decision(&proposal, RadrootsOrderRevisionOutcome::Accepted);
    let mut config = RadrootsSdkConfig::for_environment(SdkEnvironment::Custom);
    config.transport = SdkTransportMode::RelayDirect;
    config.signer = SignerConfig::LocalIdentity;
    config.relay = RelayConfig {
        urls: vec![relay.url().to_owned()],
    };
    let client = RadrootsSdkClient::from_config(config)?;

    let proposal_receipt = client
        .order()
        .publish_order_revision_proposal_with_identity(
            &seller_identity,
            &root_event_id,
            &decision_event_id,
            &proposal,
        )
        .await?;
    let decision_receipt = client
        .order()
        .publish_order_revision_decision_with_identity(
            &buyer_identity,
            &root_event_id,
            &decision.prev_event_id,
            &decision,
        )
        .await?;

    assert_eq!(proposal_receipt.event_kind, Some(3424));
    assert_eq!(decision_receipt.event_kind, Some(3425));

    match proposal_receipt.transport_receipt {
        SdkTransportReceipt::RelayDirect(relay_receipt) => {
            assert_eq!(relay_receipt.event.kind, 3424);
            assert_eq!(relay_receipt.event.author, seller_pubkey);
            assert!(
                relay_receipt
                    .event
                    .tags
                    .contains(&vec!["p".to_owned(), buyer_pubkey.clone()])
            );
            assert!(
                relay_receipt
                    .event
                    .tags
                    .contains(&vec!["e_root".to_owned(), root_event_id.to_string()])
            );
            assert!(
                relay_receipt
                    .event
                    .tags
                    .contains(&vec!["e_prev".to_owned(), decision_event_id.to_string()])
            );
            let envelope = client
                .order()
                .parse_order_revision_proposal(&relay_receipt.event)
                .expect("order revision proposal");
            assert_eq!(envelope.order_id, proposal.order_id);
            assert_eq!(envelope.listing_addr, proposal.listing_addr);
            assert_eq!(envelope.payload.revision_id, "revision-1");
            assert_eq!(envelope.payload.economics.total, usd("15"));
            assert_eq!(envelope.payload.reason, "update count");
        }
        SdkTransportReceipt::Radrootsd(_) => panic!("unexpected radrootsd receipt"),
    }

    match decision_receipt.transport_receipt {
        SdkTransportReceipt::RelayDirect(relay_receipt) => {
            assert_eq!(relay_receipt.event.kind, 3425);
            assert_eq!(relay_receipt.event.author, buyer_pubkey);
            assert!(
                relay_receipt
                    .event
                    .tags
                    .contains(&vec!["p".to_owned(), seller_pubkey])
            );
            assert!(
                relay_receipt
                    .event
                    .tags
                    .contains(&vec!["e_root".to_owned(), root_event_id.to_string()])
            );
            assert!(
                relay_receipt
                    .event
                    .tags
                    .contains(&vec!["e_prev".to_owned(), event_id_wire('3')])
            );
            let envelope = client
                .order()
                .parse_order_revision_decision(&relay_receipt.event)
                .expect("order revision decision");
            assert_eq!(envelope.order_id, decision.order_id);
            assert_eq!(envelope.listing_addr, decision.listing_addr);
            assert_eq!(envelope.payload.revision_id, decision.revision_id);
            assert_eq!(
                envelope.payload.decision,
                RadrootsOrderRevisionOutcome::Accepted
            );
        }
        SdkTransportReceipt::Radrootsd(_) => panic!("unexpected radrootsd receipt"),
    }

    Ok(())
}

#[tokio::test]
async fn relay_direct_order_lifecycle_publish_accepts_sdk_built_payloads() -> TestResult<()> {
    let relay = AckRelay::spawn().await?;
    let buyer_identity = RadrootsIdentity::generate();
    let seller_identity = RadrootsIdentity::generate();
    let buyer_pubkey = buyer_identity.public_key_hex();
    let seller_pubkey = seller_identity.public_key_hex();
    let root_event_id = event_id('1');
    let decision_event_id = event_id('2');
    let fulfillment_event_id = event_id('4');
    let fulfillment = sample_fulfillment_update(buyer_pubkey.clone(), seller_pubkey.clone());
    let cancellation = sample_order_cancellation(buyer_pubkey.clone(), seller_pubkey.clone());
    let receipt = sample_buyer_receipt(buyer_pubkey.clone(), seller_pubkey.clone());
    let mut config = RadrootsSdkConfig::for_environment(SdkEnvironment::Custom);
    config.transport = SdkTransportMode::RelayDirect;
    config.signer = SignerConfig::LocalIdentity;
    config.relay = RelayConfig {
        urls: vec![relay.url().to_owned()],
    };
    let client = RadrootsSdkClient::from_config(config)?;

    let fulfillment_receipt = client
        .order()
        .publish_fulfillment_update_with_identity(
            &seller_identity,
            &root_event_id,
            &decision_event_id,
            &fulfillment,
        )
        .await?;
    let cancellation_receipt = client
        .order()
        .publish_order_cancellation_with_identity(
            &buyer_identity,
            &root_event_id,
            &root_event_id,
            &cancellation,
        )
        .await?;
    let buyer_receipt = client
        .order()
        .publish_buyer_receipt_with_identity(
            &buyer_identity,
            &root_event_id,
            &fulfillment_event_id,
            &receipt,
        )
        .await?;

    assert_eq!(fulfillment_receipt.event_kind, Some(3433));
    assert_eq!(cancellation_receipt.event_kind, Some(3432));
    assert_eq!(buyer_receipt.event_kind, Some(3434));

    match fulfillment_receipt.transport_receipt {
        SdkTransportReceipt::RelayDirect(relay_receipt) => {
            assert_eq!(relay_receipt.event.kind, 3433);
            assert_eq!(relay_receipt.event.author, seller_pubkey);
            assert!(
                relay_receipt
                    .event
                    .tags
                    .contains(&vec!["p".to_owned(), buyer_pubkey.clone()])
            );
            assert!(
                relay_receipt
                    .event
                    .tags
                    .contains(&vec!["e_root".to_owned(), root_event_id.to_string()])
            );
            assert!(
                relay_receipt
                    .event
                    .tags
                    .contains(&vec!["e_prev".to_owned(), decision_event_id.to_string()])
            );
            let envelope = client
                .order()
                .parse_fulfillment_update(&relay_receipt.event)
                .expect("active fulfillment update");
            assert_eq!(envelope.order_id, fulfillment.order_id);
            assert_eq!(envelope.listing_addr, fulfillment.listing_addr);
            assert_eq!(envelope.payload.status, fulfillment.status);
        }
        SdkTransportReceipt::Radrootsd(_) => panic!("unexpected radrootsd receipt"),
    }

    match cancellation_receipt.transport_receipt {
        SdkTransportReceipt::RelayDirect(relay_receipt) => {
            assert_eq!(relay_receipt.event.kind, 3432);
            assert_eq!(relay_receipt.event.author, buyer_pubkey);
            assert!(
                relay_receipt
                    .event
                    .tags
                    .contains(&vec!["p".to_owned(), seller_pubkey.clone()])
            );
            assert!(
                relay_receipt
                    .event
                    .tags
                    .contains(&vec!["e_root".to_owned(), root_event_id.to_string()])
            );
            assert!(
                relay_receipt
                    .event
                    .tags
                    .contains(&vec!["e_prev".to_owned(), root_event_id.to_string()])
            );
            let envelope = client
                .order()
                .parse_order_cancellation(&relay_receipt.event)
                .expect("order cancellation");
            assert_eq!(envelope.order_id, cancellation.order_id);
            assert_eq!(envelope.listing_addr, cancellation.listing_addr);
            assert_eq!(envelope.payload.reason, cancellation.reason);
        }
        SdkTransportReceipt::Radrootsd(_) => panic!("unexpected radrootsd receipt"),
    }

    match buyer_receipt.transport_receipt {
        SdkTransportReceipt::RelayDirect(relay_receipt) => {
            assert_eq!(relay_receipt.event.kind, 3434);
            assert_eq!(relay_receipt.event.author, buyer_pubkey);
            assert!(
                relay_receipt
                    .event
                    .tags
                    .contains(&vec!["p".to_owned(), seller_pubkey])
            );
            assert!(
                relay_receipt
                    .event
                    .tags
                    .contains(&vec!["e_root".to_owned(), root_event_id.to_string()])
            );
            assert!(
                relay_receipt
                    .event
                    .tags
                    .contains(&vec!["e_prev".to_owned(), fulfillment_event_id.to_string()])
            );
            let envelope = client
                .order()
                .parse_buyer_receipt(&relay_receipt.event)
                .expect("active buyer receipt");
            assert_eq!(envelope.order_id, receipt.order_id);
            assert_eq!(envelope.listing_addr, receipt.listing_addr);
            assert_eq!(envelope.payload.received, receipt.received);
        }
        SdkTransportReceipt::Radrootsd(_) => panic!("unexpected radrootsd receipt"),
    }

    Ok(())
}

#[tokio::test]
async fn relay_direct_order_decision_publish_builds_and_publishes_payload() -> TestResult<()> {
    let relay = AckRelay::spawn().await?;
    let buyer_identity = RadrootsIdentity::generate();
    let seller_identity = RadrootsIdentity::generate();
    let payload = sample_order_decision(
        buyer_identity.public_key_hex(),
        seller_identity.public_key_hex(),
    );
    let mut config = RadrootsSdkConfig::for_environment(SdkEnvironment::Custom);
    config.transport = SdkTransportMode::RelayDirect;
    config.signer = SignerConfig::LocalIdentity;
    config.relay = RelayConfig {
        urls: vec![relay.url().to_owned()],
    };
    let client = RadrootsSdkClient::from_config(config)?;
    let root_event_id = event_id('1');

    let receipt = client
        .order()
        .publish_order_decision_with_identity(
            &seller_identity,
            &root_event_id,
            &root_event_id,
            &payload,
        )
        .await?;

    assert_eq!(receipt.transport, SdkTransportMode::RelayDirect);
    assert_eq!(receipt.event_kind, Some(3423));

    Ok(())
}

#[tokio::test]
async fn relay_direct_order_request_publish_builds_and_publishes_payload() -> TestResult<()> {
    let relay = AckRelay::spawn().await?;
    let buyer_identity = RadrootsIdentity::generate();
    let seller_identity = RadrootsIdentity::generate();
    let payload = sample_order_request(
        buyer_identity.public_key_hex(),
        seller_identity.public_key_hex(),
    );
    let mut config = RadrootsSdkConfig::for_environment(SdkEnvironment::Custom);
    config.transport = SdkTransportMode::RelayDirect;
    config.signer = SignerConfig::LocalIdentity;
    config.relay = RelayConfig {
        urls: vec![relay.url().to_owned()],
    };
    let client = RadrootsSdkClient::from_config(config)?;

    let receipt = client
        .order()
        .publish_order_request_with_identity(&buyer_identity, &listing_event_ptr(), &payload)
        .await?;

    assert_eq!(receipt.transport, SdkTransportMode::RelayDirect);
    assert_eq!(receipt.event_kind, Some(3422));

    Ok(())
}

#[tokio::test]
async fn relay_direct_order_request_publish_rejects_radrootsd_transport_mode() -> TestResult<()> {
    let buyer_identity = RadrootsIdentity::generate();
    let seller_identity = RadrootsIdentity::generate();
    let payload = sample_order_request(
        buyer_identity.public_key_hex(),
        seller_identity.public_key_hex(),
    );
    let mut config = RadrootsSdkConfig::production();
    config.transport = SdkTransportMode::Radrootsd;
    config.signer = SignerConfig::LocalIdentity;
    let client = RadrootsSdkClient::from_config(config)?;

    let error = client
        .order()
        .publish_order_request_with_identity(&buyer_identity, &listing_event_ptr(), &payload)
        .await
        .expect_err("unsupported transport");

    assert!(matches!(
        error,
        SdkPublishError::UnsupportedTransport {
            transport: SdkTransportMode::Radrootsd,
            operation: "order.publish_order_request_with_identity",
        }
    ));

    Ok(())
}

#[tokio::test]
async fn relay_direct_order_request_publish_rejects_draft_only_signer_mode() -> TestResult<()> {
    let relay = AckRelay::spawn().await?;
    let buyer_identity = RadrootsIdentity::generate();
    let seller_identity = RadrootsIdentity::generate();
    let payload = sample_order_request(
        buyer_identity.public_key_hex(),
        seller_identity.public_key_hex(),
    );
    let mut config = RadrootsSdkConfig::for_environment(SdkEnvironment::Custom);
    config.transport = SdkTransportMode::RelayDirect;
    config.signer = SignerConfig::DraftOnly;
    config.relay = RelayConfig {
        urls: vec![relay.url().to_owned()],
    };
    let client = RadrootsSdkClient::from_config(config)?;

    let error = client
        .order()
        .publish_order_request_with_identity(&buyer_identity, &listing_event_ptr(), &payload)
        .await
        .expect_err("unsupported signer mode");

    assert!(matches!(
        error,
        SdkPublishError::UnsupportedSignerMode {
            transport: SdkTransportMode::RelayDirect,
            signer: SignerConfig::DraftOnly,
            required: SignerConfig::LocalIdentity,
            operation: "order.publish_order_request_with_identity",
        }
    ));

    Ok(())
}

#[tokio::test]
async fn relay_direct_order_request_publish_rejects_invalid_economics() -> TestResult<()> {
    let buyer_identity = RadrootsIdentity::generate();
    let seller_identity = RadrootsIdentity::generate();
    let mut payload = sample_order_request(
        buyer_identity.public_key_hex(),
        seller_identity.public_key_hex(),
    );
    payload.economics.items[0].bin_count = 1;
    let mut config = RadrootsSdkConfig::for_environment(SdkEnvironment::Custom);
    config.transport = SdkTransportMode::RelayDirect;
    config.signer = SignerConfig::LocalIdentity;
    config.relay = RelayConfig {
        urls: vec!["ws://127.0.0.1:9".to_owned()],
    };
    let client = RadrootsSdkClient::from_config(config)?;

    let error = client
        .order()
        .publish_order_request_with_identity(&buyer_identity, &listing_event_ptr(), &payload)
        .await
        .expect_err("invalid economics");

    assert!(matches!(error, SdkPublishError::Encode(_)));

    Ok(())
}

#[tokio::test]
async fn relay_direct_order_request_publish_reports_setup_error_detail() -> TestResult<()> {
    let buyer_identity = RadrootsIdentity::generate();
    let seller_identity = RadrootsIdentity::generate();
    let payload = sample_order_request(
        buyer_identity.public_key_hex(),
        seller_identity.public_key_hex(),
    );
    let mut config = RadrootsSdkConfig::for_environment(SdkEnvironment::Custom);
    config.transport = SdkTransportMode::RelayDirect;
    config.signer = SignerConfig::LocalIdentity;
    config.network.timeout_ms = 10;
    config.relay = RelayConfig {
        urls: vec!["ws://127.0.0.1:9".to_owned()],
    };
    let client = RadrootsSdkClient::from_config(config)?;

    let error = client
        .order()
        .publish_order_request_with_identity(&buyer_identity, &listing_event_ptr(), &payload)
        .await
        .expect_err("relay setup error");

    assert!(matches!(
        error,
        SdkPublishError::RelaySetup {
            transport: SdkTransportMode::RelayDirect,
            operation: "order.publish_order_request_with_identity",
            target_relays,
            error: _,
        } if target_relays == vec!["ws://127.0.0.1:9".to_owned()]
    ));

    Ok(())
}

#[tokio::test]
async fn relay_direct_farm_publish_rejects_radrootsd_transport_mode() -> TestResult<()> {
    let identity = RadrootsIdentity::generate();
    let mut config = RadrootsSdkConfig::production();
    config.transport = SdkTransportMode::Radrootsd;
    config.signer = SignerConfig::LocalIdentity;
    let client = RadrootsSdkClient::from_config(config)?;

    let error = client
        .farm()
        .publish_with_identity(&identity, &sample_farm())
        .await
        .expect_err("unsupported transport");

    assert!(matches!(
        error,
        SdkPublishError::UnsupportedTransport {
            transport: SdkTransportMode::Radrootsd,
            operation: "farm.publish_with_identity",
        }
    ));

    Ok(())
}

#[tokio::test]
async fn relay_direct_farm_publish_rejects_draft_only_signer_mode() -> TestResult<()> {
    let relay = AckRelay::spawn().await?;
    let identity = RadrootsIdentity::generate();
    let mut config = RadrootsSdkConfig::for_environment(SdkEnvironment::Custom);
    config.transport = SdkTransportMode::RelayDirect;
    config.signer = SignerConfig::DraftOnly;
    config.relay = RelayConfig {
        urls: vec![relay.url().to_owned()],
    };
    let client = RadrootsSdkClient::from_config(config)?;

    let error = client
        .farm()
        .publish_with_identity(&identity, &sample_farm())
        .await
        .expect_err("unsupported signer mode");

    assert!(matches!(
        error,
        SdkPublishError::UnsupportedSignerMode {
            transport: SdkTransportMode::RelayDirect,
            signer: SignerConfig::DraftOnly,
            required: SignerConfig::LocalIdentity,
            operation: "farm.publish_with_identity",
        }
    ));

    Ok(())
}

#[tokio::test]
async fn relay_direct_profile_publish_accepts_sdk_built_draft() -> TestResult<()> {
    let relay = AckRelay::spawn().await?;
    let identity = RadrootsIdentity::generate();
    let mut config = RadrootsSdkConfig::for_environment(SdkEnvironment::Custom);
    config.transport = SdkTransportMode::RelayDirect;
    config.signer = SignerConfig::LocalIdentity;
    config.relay = RelayConfig {
        urls: vec![relay.url().to_owned()],
    };
    let client = RadrootsSdkClient::from_config(config)?;
    let draft = client
        .profile()
        .build_draft(&sample_profile(), Some(RadrootsProfileType::Farm))?;

    let receipt = client
        .profile()
        .publish_draft_with_identity(&identity, draft)
        .await?;

    assert_eq!(receipt.transport, SdkTransportMode::RelayDirect);
    assert_eq!(receipt.event_kind, Some(0));
    assert!(receipt.event_id.is_some());
    match receipt.transport_receipt {
        SdkTransportReceipt::RelayDirect(relay_receipt) => {
            assert_eq!(
                receipt.event_id.as_deref(),
                Some(relay_receipt.event_id.as_str())
            );
            assert_eq!(relay_receipt.event.kind, 0);
            assert_eq!(relay_receipt.event.author, identity.public_key_hex());
            assert_eq!(
                relay_receipt.event.tags,
                vec![vec!["t".to_owned(), "radroots:type:farm".to_owned()]]
            );
            assert_eq!(relay_receipt.target_relays, vec![relay.url().to_owned()]);
            assert_eq!(relay_receipt.connected_relays, vec![relay.url().to_owned()]);
            assert_eq!(
                relay_receipt.acknowledged_relays,
                vec![relay.url().to_owned()]
            );
            assert!(relay_receipt.failed_relays.is_empty());
        }
        SdkTransportReceipt::Radrootsd(_) => panic!("unexpected radrootsd receipt"),
    }

    Ok(())
}

#[tokio::test]
async fn relay_direct_profile_publish_rejects_radrootsd_transport_mode() -> TestResult<()> {
    let identity = RadrootsIdentity::generate();
    let mut config = RadrootsSdkConfig::production();
    config.transport = SdkTransportMode::Radrootsd;
    config.signer = SignerConfig::LocalIdentity;
    let client = RadrootsSdkClient::from_config(config)?;

    let error = client
        .profile()
        .publish_with_identity(
            &identity,
            &sample_profile(),
            Some(RadrootsProfileType::Farm),
        )
        .await
        .expect_err("unsupported transport");

    assert!(matches!(
        error,
        SdkPublishError::UnsupportedTransport {
            transport: SdkTransportMode::Radrootsd,
            operation: "profile.publish_with_identity",
        }
    ));

    Ok(())
}

#[tokio::test]
async fn relay_direct_profile_publish_rejects_draft_only_signer_mode() -> TestResult<()> {
    let relay = AckRelay::spawn().await?;
    let identity = RadrootsIdentity::generate();
    let mut config = RadrootsSdkConfig::for_environment(SdkEnvironment::Custom);
    config.transport = SdkTransportMode::RelayDirect;
    config.signer = SignerConfig::DraftOnly;
    config.relay = RelayConfig {
        urls: vec![relay.url().to_owned()],
    };
    let client = RadrootsSdkClient::from_config(config)?;

    let error = client
        .profile()
        .publish_with_identity(
            &identity,
            &sample_profile(),
            Some(RadrootsProfileType::Farm),
        )
        .await
        .expect_err("unsupported signer mode");

    assert!(matches!(
        error,
        SdkPublishError::UnsupportedSignerMode {
            transport: SdkTransportMode::RelayDirect,
            signer: SignerConfig::DraftOnly,
            required: SignerConfig::LocalIdentity,
            operation: "profile.publish_with_identity",
        }
    ));

    Ok(())
}

#[tokio::test]
async fn relay_direct_listing_publish_accepts_sdk_built_draft() -> TestResult<()> {
    let relay = AckRelay::spawn().await?;
    let identity = RadrootsIdentity::generate();
    let mut config = RadrootsSdkConfig::for_environment(SdkEnvironment::Custom);
    config.transport = SdkTransportMode::RelayDirect;
    config.signer = SignerConfig::LocalIdentity;
    config.relay = RelayConfig {
        urls: vec![relay.url().to_owned()],
    };
    let client = RadrootsSdkClient::from_config(config)?;
    let draft = client.listing().build_draft(&sample_listing())?;

    let receipt = client
        .listing()
        .publish_draft_with_identity(&identity, draft)
        .await?;

    assert_eq!(receipt.transport, SdkTransportMode::RelayDirect);
    assert_eq!(receipt.event_kind, Some(30402));
    assert!(receipt.event_id.is_some());
    match receipt.transport_receipt {
        SdkTransportReceipt::RelayDirect(relay_receipt) => {
            assert_eq!(
                receipt.event_id.as_deref(),
                Some(relay_receipt.event_id.as_str())
            );
            assert_eq!(receipt.event_kind, Some(relay_receipt.event_kind));
            assert_eq!(relay_receipt.event.kind, 30402);
            assert_eq!(relay_receipt.event_id, relay_receipt.event.id);
            assert_eq!(relay_receipt.signature, relay_receipt.event.sig);
            assert_eq!(relay_receipt.created_at, relay_receipt.event.created_at);
            assert_eq!(relay_receipt.event.author, identity.public_key_hex());
            assert_eq!(relay_receipt.target_relays, vec![relay.url().to_owned()]);
            assert_eq!(relay_receipt.connected_relays, vec![relay.url().to_owned()]);
            assert_eq!(
                relay_receipt.acknowledged_relays,
                vec![relay.url().to_owned()]
            );
            assert!(relay_receipt.failed_relays.is_empty());
        }
        SdkTransportReceipt::Radrootsd(_) => panic!("unexpected radrootsd receipt"),
    }

    Ok(())
}

#[tokio::test]
async fn relay_direct_publish_rejects_radrootsd_transport_mode() -> TestResult<()> {
    let identity = RadrootsIdentity::generate();
    let mut config = RadrootsSdkConfig::production();
    config.transport = SdkTransportMode::Radrootsd;
    config.signer = SignerConfig::LocalIdentity;
    let client = RadrootsSdkClient::from_config(config)?;

    let error = client
        .listing()
        .publish_with_identity(&identity, &sample_listing())
        .await
        .expect_err("unsupported transport");

    assert!(matches!(
        error,
        SdkPublishError::UnsupportedTransport {
            transport: SdkTransportMode::Radrootsd,
            operation: "listing.publish_with_identity",
        }
    ));

    Ok(())
}

#[tokio::test]
async fn relay_direct_publish_rejects_draft_only_signer_mode() -> TestResult<()> {
    let relay = AckRelay::spawn().await?;
    let identity = RadrootsIdentity::generate();
    let mut config = RadrootsSdkConfig::for_environment(SdkEnvironment::Custom);
    config.transport = SdkTransportMode::RelayDirect;
    config.signer = SignerConfig::DraftOnly;
    config.relay = RelayConfig {
        urls: vec![relay.url().to_owned()],
    };
    let client = RadrootsSdkClient::from_config(config)?;

    let error = client
        .listing()
        .publish_with_identity(&identity, &sample_listing())
        .await
        .expect_err("unsupported signer mode");

    assert!(matches!(
        error,
        SdkPublishError::UnsupportedSignerMode {
            transport: SdkTransportMode::RelayDirect,
            signer: SignerConfig::DraftOnly,
            required: SignerConfig::LocalIdentity,
            operation: "listing.publish_with_identity",
        }
    ));

    Ok(())
}

#[tokio::test]
async fn relay_direct_publish_rejects_nip46_signer_mode() -> TestResult<()> {
    let relay = AckRelay::spawn().await?;
    let identity = RadrootsIdentity::generate();
    let mut config = RadrootsSdkConfig::for_environment(SdkEnvironment::Custom);
    config.transport = SdkTransportMode::RelayDirect;
    config.signer = SignerConfig::Nip46;
    config.relay = RelayConfig {
        urls: vec![relay.url().to_owned()],
    };
    let client = RadrootsSdkClient::from_config(config)?;

    let error = client
        .listing()
        .publish_with_identity(&identity, &sample_listing())
        .await
        .expect_err("unsupported signer mode");

    assert!(matches!(
        error,
        SdkPublishError::UnsupportedSignerMode {
            transport: SdkTransportMode::RelayDirect,
            signer: SignerConfig::Nip46,
            required: SignerConfig::LocalIdentity,
            operation: "listing.publish_with_identity",
        }
    ));

    Ok(())
}
