use radroots_authority::RadrootsActorContext;
use radroots_core::{
    RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreQuantity,
    RadrootsCoreQuantityPrice, RadrootsCoreUnit,
};
use radroots_events::contract::RadrootsActorRole;
use radroots_events::ids::{RadrootsDTag, RadrootsInventoryBinId};
use radroots_sdk::protocol::farm::RadrootsFarmRef;
use radroots_sdk::protocol::listing::{
    RadrootsListing, RadrootsListingBin, RadrootsListingProduct,
};
use radroots_sdk::{ListingPreparePublishRequest, RadrootsClient, RadrootsSdkTimestamp};

const SELLER: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sdk = RadrootsClient::builder()
        .fixed_clock(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_000))
        .build()
        .await?;
    let actor = RadrootsActorContext::test(SELLER, [RadrootsActorRole::Seller])?;
    let request = ListingPreparePublishRequest::new(actor, sample_listing(SELLER));

    let plan = sdk.listings().prepare_publish(request)?;

    println!("public listing: {}", plan.public_listing_addr.as_str());
    println!("draft listing: {}", plan.draft_listing_addr.as_str());
    println!("expected event: {}", plan.expected_event_id.as_str());
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
