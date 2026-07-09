#![cfg(feature = "runtime")]

use radroots_authority::{
    RadrootsActorContext, RadrootsEventSigner, RadrootsSignerError, RadrootsSignerIdentity,
};
use radroots_core::{
    RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreQuantity,
    RadrootsCoreQuantityPrice, RadrootsCoreUnit,
};
use radroots_events::{
    contract::RadrootsActorRole,
    draft::{RadrootsFrozenEventDraft, RadrootsSignedNostrEvent},
    farm::RadrootsFarmRef,
    ids::{RadrootsDTag, RadrootsInventoryBinId},
    listing::{
        RadrootsListing, RadrootsListingAvailability, RadrootsListingBin,
        RadrootsListingDeliveryMethod, RadrootsListingProduct, RadrootsListingPublicLocation,
        RadrootsListingStatus,
    },
};
use radroots_nostr::prelude::{
    RadrootsNostrKeys, RadrootsNostrSecretKey, radroots_nostr_sign_frozen_draft,
};
use radroots_sdk::{
    ListingEnqueuePublishRequest, MarketSearchRequest, NostrProfile, NostrRelayUrlPolicy,
    RadrootsClient, RadrootsSdkError, RadrootsSdkTimestamp, SyncProjectionRefreshRequest,
    TargetPolicy, TransportProfile,
};

const SELLER_SECRET_KEY_HEX: &str =
    "59392e9068f66431b12f70218fb61281cb6b433d7f27c55d61f1a63fe1a96ff8";
const SELLER_PUBLIC_KEY_HEX: &str =
    "e0266e3cfb0d2886f91c73f5f868f3b98273713e5fcd97c081663f5518a4b3af";
const FARM_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAAAA";
const LISTING_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAAAg";
const RELAY: &str = "wss://relay.radroots.test";

#[derive(Clone)]
struct FixtureSigner {
    identity: RadrootsSignerIdentity,
    keys: RadrootsNostrKeys,
}

impl FixtureSigner {
    fn new(secret_key_hex: &str) -> Self {
        let secret_key = RadrootsNostrSecretKey::from_hex(secret_key_hex).expect("secret key");
        let keys = RadrootsNostrKeys::new(secret_key);
        let pubkey = keys.public_key().to_hex();
        Self {
            identity: RadrootsSignerIdentity::new(pubkey).expect("identity"),
            keys,
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
        radroots_nostr_sign_frozen_draft(&self.keys, draft).map_err(|error| {
            RadrootsSignerError::SigningFailed {
                message: error.to_string(),
            }
        })
    }
}

fn seller_actor() -> RadrootsActorContext {
    RadrootsActorContext::test(SELLER_PUBLIC_KEY_HEX, [RadrootsActorRole::Seller]).expect("actor")
}

fn decimal(raw: &str) -> RadrootsCoreDecimal {
    raw.parse().expect("decimal")
}

fn listing(title: &str) -> RadrootsListing {
    RadrootsListing {
        d_tag: RadrootsDTag::parse(LISTING_D_TAG).expect("d tag"),
        published_at: Some(1_700_000_000),
        farm: RadrootsFarmRef {
            pubkey: SELLER_PUBLIC_KEY_HEX.to_owned(),
            d_tag: FARM_D_TAG.to_owned(),
        },
        product: RadrootsListingProduct {
            key: "blueberries".to_owned(),
            title: title.to_owned(),
            category: "fruit".to_owned(),
            summary: Some("Fresh field berries".to_owned()),
            process: None,
            lot: None,
            location: None,
            profile: None,
            year: None,
        },
        primary_bin_id: RadrootsInventoryBinId::parse("pint").expect("bin id"),
        bins: vec![RadrootsListingBin {
            bin_id: RadrootsInventoryBinId::parse("pint").expect("bin id"),
            quantity: RadrootsCoreQuantity::new(decimal("1"), RadrootsCoreUnit::Each),
            price_per_canonical_unit: RadrootsCoreQuantityPrice {
                amount: RadrootsCoreMoney::new(decimal("6"), RadrootsCoreCurrency::USD),
                quantity: RadrootsCoreQuantity::new(decimal("1"), RadrootsCoreUnit::Each),
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
        inventory_available: Some(decimal("12")),
        availability: Some(RadrootsListingAvailability::Status {
            status: RadrootsListingStatus::Active,
        }),
        delivery_method: Some(RadrootsListingDeliveryMethod::Pickup),
        location: Some(RadrootsListingPublicLocation {
            primary: "Fernwood".to_owned(),
            city: Some("Victoria".to_owned()),
            region: Some("BC".to_owned()),
            country: Some("CA".to_owned()),
            geohash: "c2b2q".to_owned(),
        }),
        images: None,
    }
}

async fn directory_sdk() -> (tempfile::TempDir, RadrootsClient) {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let sdk = RadrootsClient::builder()
        .directory_storage(tempdir.path().join("sdk"))
        .fixed_clock(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_000))
        .transport_profile(TransportProfile::nostr(
            NostrProfile::new([RELAY], NostrRelayUrlPolicy::Public).expect("Nostr profile"),
        ))
        .build()
        .await
        .expect("sdk");
    (tempdir, sdk)
}

#[tokio::test]
async fn market_search_refreshes_local_projection_and_reads_fts() {
    let (_tempdir, sdk) = directory_sdk().await;
    let publish = ListingEnqueuePublishRequest::new(
        seller_actor(),
        listing("Blueberries"),
        TargetPolicy::default_profile(),
    )
    .try_with_nostr_targets([RELAY], NostrRelayUrlPolicy::Public)
    .expect("target relays");
    let receipt = sdk
        .listings()
        .enqueue_publish_with_explicit_signer(publish, &FixtureSigner::new(SELLER_SECRET_KEY_HEX))
        .await
        .expect("publish");

    let search = sdk
        .market()
        .search(MarketSearchRequest::new("blueberries victoria").with_limit(10))
        .await
        .expect("search");

    assert_eq!(
        search.source,
        radroots_sdk::MarketSearchSource::LocalProjectionFts
    );
    assert_eq!(
        search.refresh.projection_id,
        "radroots.product_projection.v1"
    );
    assert_eq!(search.refresh.projection_version, 1);
    assert_eq!(search.refresh.refreshed_at_ms, 1_700_000_000_000);
    assert_eq!(search.refresh.scanned_events, 1);
    assert_eq!(search.refresh.listing_upserts, 1);
    assert_eq!(search.refresh.trade_upserts, 0);
    assert_eq!(search.refresh.validation_receipts, 0);
    assert_eq!(search.listings.len(), 1);
    assert_eq!(search.listings[0].listing_event_id, receipt.signed_event_id);
    assert_eq!(
        search.listings[0].seller_pubkey.as_str(),
        SELLER_PUBLIC_KEY_HEX
    );
    assert_eq!(search.listings[0].title, "Blueberries");
    assert_eq!(search.listings[0].product_type, "fruit");
    assert_eq!(
        search.listings[0].locality_city.as_deref(),
        Some("Victoria")
    );
    assert_eq!(search.listings[0].geohash5, "c2b2q");

    let second = sdk
        .market()
        .search(MarketSearchRequest::new("").with_limit(10))
        .await
        .expect("second search");
    assert_eq!(second.refresh.scanned_events, 0);
    assert_eq!(second.listings.len(), 1);
}

#[tokio::test]
async fn projection_refresh_limits_are_request_errors() {
    let (_tempdir, sdk) = directory_sdk().await;

    let sync_error = sdk
        .sync()
        .refresh_projections(SyncProjectionRefreshRequest::new().with_limit(0))
        .await
        .expect_err("sync limit");
    assert!(matches!(
        sync_error,
        RadrootsSdkError::InvalidRequest { .. }
    ));
    assert_eq!(sync_error.code(), "invalid_request");

    let market_error = sdk
        .market()
        .search(MarketSearchRequest::new("berries").with_limit(0))
        .await
        .expect_err("market limit");
    assert!(matches!(
        market_error,
        RadrootsSdkError::InvalidRequest { .. }
    ));
}
