use radroots_core::{Currency, Decimal, Money, Quantity, QuantityPrice, Unit};
use radroots_event::contract::AuthorRole;
use radroots_event::farm::FarmRef;
use radroots_event::id::{DTag, InventoryBinId};
use radroots_event::listing::operational::{
    OperationalListing, OperationalListingAvailability, OperationalListingBin,
    OperationalListingDeliveryMethod, OperationalListingProduct, OperationalListingPublicLocation,
    OperationalListingStatus,
};
use radroots_nostr::signing::LocalSigner;
use radroots_sdk::{
    ListingPreparePublishRequest, NostrRelayUrlPolicy, PushOutboxRequest, RadrootsClient,
    RadrootsSdkError, RadrootsSdkLocalKeySigner, RadrootsSdkSignerProvider, RadrootsSdkTimestamp,
    SdkIdempotencyKey, TargetPolicy,
};
use radroots_signing::{Actor, actor::ActorSource};

const RELAY: &str = "wss://relay.example.com";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let signer = LocalSigner::generate()?;
    let seller = signer.public_key().to_hex();
    let signer = RadrootsSdkLocalKeySigner::from_signer(signer, seller.as_str())?;
    let sdk = RadrootsClient::builder()
        .fixed_clock(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_000))
        .signer_provider(RadrootsSdkSignerProvider::LocalKey(signer))
        .build()
        .await?;
    let actor = Actor::from_public_key_hex(
        seller.as_str(),
        ActorSource::ExplicitPublicKey,
        [AuthorRole::Seller],
    )?;
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

fn sample_listing(seller: &str) -> OperationalListing {
    OperationalListing {
        d_tag: DTag::parse("AAAAAAAAAAAAAAAAAAAAAQ").expect("d tag"),
        published_at: None,
        farm: FarmRef {
            pubkey: seller.to_owned(),
            d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_owned(),
        },
        product: OperationalListingProduct {
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
        primary_bin_id: InventoryBinId::parse("bin-1").expect("bin id"),
        bins: vec![OperationalListingBin {
            bin_id: InventoryBinId::parse("bin-1").expect("bin id"),
            quantity: Quantity::try_new(Decimal::from(1000u32), Unit::MassG)
                .expect("positive example quantity"),
            price_per_canonical_unit: QuantityPrice::try_new(
                Money::try_new(Decimal::from(20u32), Currency::USD)
                    .expect("non-negative example money"),
                Quantity::try_new(Decimal::from(1u32), Unit::MassG)
                    .expect("positive example pricing quantity"),
            )
            .expect("non-zero example pricing quantity"),
            display_amount: None,
            display_unit: None,
            display_label: None,
            display_price: None,
            display_price_unit: None,
        }],
        resource_area: None,
        plot: None,
        discounts: None,
        inventory_available: Some(Decimal::from(5u32)),
        availability: Some(OperationalListingAvailability::Status {
            status: OperationalListingStatus::Active,
        }),
        delivery_method: Some(OperationalListingDeliveryMethod::Pickup),
        location: Some(OperationalListingPublicLocation {
            primary: "Victoria".to_owned(),
            city: Some("Victoria".to_owned()),
            region: Some("British Columbia".to_owned()),
            country: Some("CA".to_owned()),
            geohash: "c287g".to_owned(),
        }),
        images: None,
    }
}
