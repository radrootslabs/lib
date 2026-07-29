use radroots_authority::RadrootsActorContext;
use radroots_core::{Currency, Decimal, Money, Quantity, QuantityPrice, Unit};
use radroots_event::contract::AuthorRole;
use radroots_event::farm::RadrootsFarmRef;
use radroots_event::id::{RadrootsDTag, RadrootsInventoryBinId};
use radroots_event::listing::operational::{
    RadrootsOperationalListing, RadrootsOperationalListingAvailability,
    RadrootsOperationalListingBin, RadrootsOperationalListingDeliveryMethod,
    RadrootsOperationalListingProduct, RadrootsOperationalListingPublicLocation,
    RadrootsOperationalListingStatus,
};
use radroots_sdk::{ListingPreparePublishRequest, RadrootsClient, RadrootsSdkTimestamp};

const SELLER: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sdk = RadrootsClient::builder()
        .fixed_clock(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_000))
        .build()
        .await?;
    let actor = RadrootsActorContext::test(SELLER, [AuthorRole::Seller])?;
    let request = ListingPreparePublishRequest::new(actor, sample_listing(SELLER));

    let plan = sdk.listings().prepare_publish(request)?;

    println!("public listing: {}", plan.public_listing_addr().as_str());
    println!("expected event: {}", plan.expected_event_id().to_hex());
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
