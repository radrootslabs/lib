use radroots_authority::{RadrootsActorContext, RadrootsLocalEventSigner};
use radroots_core::{
    RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreQuantity,
    RadrootsCoreQuantityPrice, RadrootsCoreUnit,
};
use radroots_event::contract::RadrootsActorRole;
use radroots_event::farm::RadrootsFarmRef;
use radroots_event::ids::{RadrootsDTag, RadrootsInventoryBinId};
use radroots_event::operational_listing::{
    RadrootsOperationalListing, RadrootsOperationalListingAvailability,
    RadrootsOperationalListingBin, RadrootsOperationalListingDeliveryMethod,
    RadrootsOperationalListingProduct, RadrootsOperationalListingPublicLocation,
    RadrootsOperationalListingStatus,
};
use radroots_nostr::prelude::RadrootsNostrKeys;
use radroots_sdk::{
    ListingPreparePublishRequest, NostrRelayUrlPolicy, PushOutboxRequest, RadrootsClient,
    RadrootsSdkError, RadrootsSdkLocalKeySigner, RadrootsSdkSignerProvider, RadrootsSdkTimestamp,
    SdkIdempotencyKey, TargetPolicy,
};

const RELAY: &str = "wss://relay.example.com";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let keys = RadrootsNostrKeys::generate();
    let seller = keys.public_key().to_hex();
    let signer =
        RadrootsSdkLocalKeySigner::from_event_signer(RadrootsLocalEventSigner::new(keys)?)?;
    let sdk = RadrootsClient::builder()
        .fixed_clock(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_000))
        .signer_provider(RadrootsSdkSignerProvider::LocalKey(signer))
        .build()
        .await?;
    let actor = RadrootsActorContext::test(seller.as_str(), [RadrootsActorRole::Seller])?;
    let listing = sample_listing(seller.as_str());
    let prepare_request = ListingPreparePublishRequest::new(actor.clone(), listing);
    let target_policy = TargetPolicy::try_nostr_relays([RELAY], NostrRelayUrlPolicy::Public)?;
    let idempotency_key = SdkIdempotencyKey::new("01890f0e-6c00-7000-8000-000000000202")?;

    let prepared = sdk.listings().prepare_publish(prepare_request)?;
    let enqueue = sdk
        .listings()
        .enqueue_prepared_publish(
            &actor,
            prepared.clone(),
            target_policy,
            Some(idempotency_key),
        )
        .await?;
    let push = sdk
        .sync()
        .push_outbox(PushOutboxRequest::new().with_limit(1))
        .await;
    assert_eq!(
        prepared.public_listing_addr().as_str(),
        enqueue.public_listing_addr.as_str()
    );
    #[cfg(feature = "transport-nostr-runtime")]
    assert!(matches!(
        push,
        Err(RadrootsSdkError::ProductSyncTransportSetupFailure { .. })
    ));
    #[cfg(not(feature = "transport-nostr-runtime"))]
    assert!(matches!(
        push,
        Err(RadrootsSdkError::ProductSyncUnsupported { .. })
    ));
    Ok(())
}

fn sample_listing(seller: &str) -> RadrootsOperationalListing {
    RadrootsOperationalListing {
        d_tag: RadrootsDTag::parse("AAAAAAAAAAAAAAAAAAAAAQ").expect("d tag"),
        published_at: None,
        farm: RadrootsFarmRef {
            pubkey: seller.to_owned(),
            d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_owned(),
        },
        product: RadrootsOperationalListingProduct {
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
        bins: vec![RadrootsOperationalListingBin {
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
        inventory_available: Some(RadrootsCoreDecimal::from(5u32)),
        availability: Some(RadrootsOperationalListingAvailability::Status {
            status: RadrootsOperationalListingStatus::Active,
        }),
        delivery_method: Some(RadrootsOperationalListingDeliveryMethod::Pickup),
        location: Some(RadrootsOperationalListingPublicLocation {
            primary: "Victoria".to_owned(),
            city: Some("Victoria".to_owned()),
            region: Some("British Columbia".to_owned()),
            country: Some("CA".to_owned()),
            geohash: "c287g".to_owned(),
        }),
        images: None,
    }
}
