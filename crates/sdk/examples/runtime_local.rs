use radroots_authority::{
    RadrootsActorContext, RadrootsEventSigner, RadrootsSignerError, RadrootsSignerIdentity,
};
use radroots_core::{
    RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreQuantity,
    RadrootsCoreQuantityPrice, RadrootsCoreUnit,
};
use radroots_events::contract::RadrootsActorRole;
use radroots_events::draft::{
    RadrootsFrozenEventDraft, RadrootsSignedNostrEvent, RadrootsSignedNostrEventParts,
};
use radroots_events::ids::{RadrootsDTag, RadrootsInventoryBinId};
use radroots_relay_transport::RadrootsMockRelayPublishAdapter;
use radroots_sdk::protocol::farm::RadrootsFarmRef;
use radroots_sdk::protocol::listing::{
    RadrootsListing, RadrootsListingBin, RadrootsListingProduct,
};
use radroots_sdk::{
    ListingPublishRequest, OrderStatusRequest, PushOutboxRequest, RadrootsSdk, RadrootsSdkTimestamp,
};

const SELLER: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const RELAY: &str = "wss://relay.example.com";

#[derive(Clone)]
struct FixtureSigner {
    identity: RadrootsSignerIdentity,
}

impl FixtureSigner {
    fn new(pubkey: &str) -> Self {
        Self {
            identity: RadrootsSignerIdentity::new(pubkey).expect("identity"),
        }
    }
}

impl RadrootsEventSigner for FixtureSigner {
    fn pubkey(&self) -> &radroots_events::ids::RadrootsPublicKey {
        self.identity.pubkey()
    }

    fn sign_frozen_draft(
        &self,
        draft: &RadrootsFrozenEventDraft,
    ) -> Result<RadrootsSignedNostrEvent, RadrootsSignerError> {
        let sig = "f".repeat(128);
        let raw_json = serde_json::json!({
            "id": draft.expected_event_id,
            "pubkey": self.pubkey().as_str(),
            "created_at": draft.created_at,
            "kind": draft.kind,
            "tags": draft.tags,
            "content": draft.content,
            "sig": sig,
        })
        .to_string();
        RadrootsSignedNostrEvent::new(RadrootsSignedNostrEventParts {
            id: draft.expected_event_id.clone(),
            pubkey: self.pubkey().as_str().to_owned(),
            created_at: draft.created_at,
            kind: draft.kind,
            tags: draft.tags.clone(),
            content: draft.content.clone(),
            sig,
            raw_json,
        })
        .map_err(|error| RadrootsSignerError::SigningFailed {
            message: error.to_string(),
        })
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sdk = RadrootsSdk::builder()
        .fixed_clock(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_000))
        .relay_url(RELAY)
        .build()
        .await?;
    let actor = RadrootsActorContext::test(SELLER, [RadrootsActorRole::Seller])?;
    let request = ListingPublishRequest::new(sample_listing()).with_idempotency_key("example-1");

    let prepared = sdk.listings().prepare_publish(&actor, request.clone())?;
    let enqueue = sdk
        .listings()
        .enqueue_publish(&actor, &FixtureSigner::new(SELLER), request)
        .await?;
    let push = sdk
        .sync()
        .push_outbox(
            &RadrootsMockRelayPublishAdapter::new(),
            PushOutboxRequest::new().with_limit(1),
        )
        .await?;
    let order_status = sdk
        .orders()
        .status(OrderStatusRequest::new("example-order-1"))
        .await?;

    assert_eq!(prepared.listing_address, enqueue.listing_address);
    assert_eq!(push.attempted_events, 1);
    assert!(!order_status.found);
    Ok(())
}

fn sample_listing() -> RadrootsListing {
    RadrootsListing {
        d_tag: RadrootsDTag::parse("AAAAAAAAAAAAAAAAAAAAAQ").expect("d tag"),
        published_at: None,
        farm: RadrootsFarmRef {
            pubkey: SELLER.to_owned(),
            d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_owned(),
        },
        product: RadrootsListingProduct {
            key: "coffee".to_owned(),
            title: "Coffee".to_owned(),
            category: "coffee".to_owned(),
            summary: Some("Single origin coffee".to_owned()),
            process: None,
            lot: None,
            location: None,
            profile: None,
            year: None,
        },
        primary_bin_id: RadrootsInventoryBinId::parse("bin-1").expect("bin id"),
        bins: vec![RadrootsListingBin {
            bin_id: RadrootsInventoryBinId::parse("bin-1").expect("bin id"),
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
        inventory_available: None,
        availability: None,
        delivery_method: None,
        location: None,
        images: None,
    }
}
