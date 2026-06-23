use radroots_authority::RadrootsActorContext;
use radroots_core::{
    RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreQuantity,
    RadrootsCoreQuantityPrice, RadrootsCoreUnit,
};
use radroots_events::contract::RadrootsActorRole;
use radroots_events::ids::{RadrootsDTag, RadrootsInventoryBinId};
use radroots_nostr::prelude::RadrootsNostrKeys;
use radroots_relay_transport::RadrootsMockRelayPublishAdapter;
use radroots_sdk::protocol::farm::RadrootsFarmRef;
use radroots_sdk::protocol::listing::{
    RadrootsListing, RadrootsListingBin, RadrootsListingProduct,
};
use radroots_sdk::{
    ListingPreparePublishRequest, OrderStatusRequest, PushOutboxRequest, RadrootsSdk,
    RadrootsSdkLocalKeySigner, RadrootsSdkSignerProvider, RadrootsSdkTimestamp, SdkIdempotencyKey,
    SdkRelayTargetPolicy, SdkRelayTargetSet, SdkRelayUrlPolicy,
};

const LOCAL_RELAY: &str = "ws://localhost:7777";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let keys = RadrootsNostrKeys::generate();
    let seller = keys.public_key().to_hex();
    let signer = RadrootsSdkLocalKeySigner::new(keys)?;
    let sdk = RadrootsSdk::builder()
        .fixed_clock(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_000))
        .signer_provider(RadrootsSdkSignerProvider::LocalKey(signer))
        .build()
        .await?;
    let actor = RadrootsActorContext::test(seller.as_str(), [RadrootsActorRole::Seller])?;
    let targets = SdkRelayTargetSet::new([LOCAL_RELAY], SdkRelayUrlPolicy::Localhost)?;
    let target_policy = SdkRelayTargetPolicy::explicit(targets);

    let prepared = sdk
        .listings()
        .prepare_publish(ListingPreparePublishRequest::new(
            actor.clone(),
            sample_listing(seller.as_str()),
        ))?;
    let enqueue = sdk
        .listings()
        .enqueue_prepared_publish(
            &actor,
            prepared,
            target_policy,
            Some(SdkIdempotencyKey::new("sdk-v1-local-example")?),
        )
        .await?;
    let adapter = RadrootsMockRelayPublishAdapter::new();
    let push = sdk
        .sync()
        .push_outbox_with_adapter(&adapter, PushOutboxRequest::new().with_limit(1))
        .await?;
    let order_status = sdk
        .orders()
        .status(OrderStatusRequest::parse("example-order-1")?)
        .await?;

    println!("queued listing event: {}", enqueue.signed_event_id.as_str());
    println!("published events: {}", push.published_events);
    println!("order found: {}", order_status.found);
    Ok(())
}

fn sample_listing(seller: &str) -> RadrootsListing {
    RadrootsListing {
        d_tag: RadrootsDTag::parse("AAAAAAAAAAAAAAAAAAAAAQ").expect("d tag"),
        published_at: None,
        farm: RadrootsFarmRef {
            pubkey: seller.to_owned(),
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
