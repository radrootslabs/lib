#[path = "../src/test_fixtures.rs"]
mod test_fixtures;

use std::str::FromStr;

use radroots_core::{
    RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreQuantity,
    RadrootsCoreQuantityPrice, RadrootsCoreUnit,
};
use radroots_events::{
    coop::{RadrootsCoop, RadrootsCoopLocation, RadrootsCoopRef},
    document::{RadrootsDocument, RadrootsDocumentSubject},
    farm::{
        RadrootsFarm, RadrootsFarmLocation, RadrootsFarmRef, RadrootsGcsLocation,
        RadrootsGeoJsonPoint, RadrootsGeoJsonPolygon,
    },
    listing::{
        RadrootsListing, RadrootsListingAvailability, RadrootsListingBin,
        RadrootsListingDeliveryMethod, RadrootsListingFarmRef, RadrootsListingLocation,
        RadrootsListingProduct,
    },
    plot::{RadrootsPlot, RadrootsPlotLocation, RadrootsPlotRef},
    resource_area::{RadrootsResourceArea, RadrootsResourceAreaLocation, RadrootsResourceAreaRef},
    resource_cap::{RadrootsResourceHarvestCap, RadrootsResourceHarvestProduct},
};
use radroots_events_codec::coop::encode::{coop_build_tags, coop_ref_tags};
use radroots_events_codec::coop::list_sets::{coop_members_farms_list_set, coop_members_list_set};
use radroots_events_codec::document::encode::document_build_tags;
use radroots_events_codec::error::EventEncodeError;
use radroots_events_codec::farm::encode::{farm_build_tags, farm_ref_tags};
use radroots_events_codec::farm::list_sets::{farm_listings_list_set, farm_members_list_set};
use radroots_events_codec::listing::encode::listing_build_tags;
use radroots_events_codec::listing::tags::{
    ListingTagOptions, listing_tags_full, listing_tags_with_options,
};
use radroots_events_codec::plot::encode::{plot_address, plot_build_tags};
use radroots_events_codec::resource_area::encode::{
    resource_area_build_tags, resource_area_ref_tags,
};
use radroots_events_codec::resource_area::list_sets::{
    resource_area_members_farms_list_set, resource_area_members_plots_list_set,
    resource_area_stewards_list_set,
};
use radroots_events_codec::resource_cap::encode::resource_harvest_cap_build_tags;
use test_fixtures::FIXTURE_ALICE_PUBLIC_KEY_HEX;

const VALID_PUBKEY: &str = FIXTURE_ALICE_PUBLIC_KEY_HEX;
const VALID_FARM_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAAAA";
const VALID_PLOT_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAAAQ";
const VALID_COOP_D_TAG: &str = "BAAAAAAAAAAAAAAAAAAAAA";
const VALID_AREA_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAAAw";
const VALID_CAP_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAABA";
const VALID_DOC_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAAAg";

fn decimal(value: &str) -> RadrootsCoreDecimal {
    RadrootsCoreDecimal::from_str(value).expect("valid decimal")
}

fn sample_gcs(geohash: &str) -> RadrootsGcsLocation {
    RadrootsGcsLocation {
        lat: 37.0,
        lng: -122.0,
        geohash: geohash.to_string(),
        point: RadrootsGeoJsonPoint {
            r#type: "Point".to_string(),
            coordinates: [-122.0, 37.0],
        },
        polygon: RadrootsGeoJsonPolygon {
            r#type: "Polygon".to_string(),
            coordinates: vec![vec![
                [-122.0, 37.0],
                [-122.0, 37.0001],
                [-122.0001, 37.0001],
                [-122.0, 37.0],
            ]],
        },
        accuracy: None,
        altitude: None,
        tag_0: None,
        label: None,
        area: None,
        elevation: None,
        soil: None,
        climate: None,
        gc_id: None,
        gc_name: None,
        gc_admin1_id: None,
        gc_admin1_name: None,
        gc_country_id: None,
        gc_country_name: None,
    }
}

fn sample_coop() -> RadrootsCoop {
    RadrootsCoop {
        d_tag: VALID_COOP_D_TAG.to_string(),
        name: "Test Coop".to_string(),
        about: None,
        website: None,
        picture: None,
        banner: None,
        location: Some(RadrootsCoopLocation {
            primary: None,
            city: None,
            region: None,
            country: None,
            gcs: sample_gcs("9q8yy"),
        }),
        tags: Some(vec!["regional".to_string()]),
    }
}

fn sample_farm() -> RadrootsFarm {
    RadrootsFarm {
        d_tag: VALID_FARM_D_TAG.to_string(),
        name: "Test Farm".to_string(),
        about: None,
        website: None,
        picture: None,
        banner: None,
        location: Some(RadrootsFarmLocation {
            primary: None,
            city: None,
            region: None,
            country: None,
            gcs: sample_gcs("9q8yy"),
        }),
        tags: Some(vec!["orchard".to_string()]),
    }
}

fn sample_plot() -> RadrootsPlot {
    RadrootsPlot {
        d_tag: VALID_PLOT_D_TAG.to_string(),
        farm: RadrootsFarmRef {
            pubkey: VALID_PUBKEY.to_string(),
            d_tag: VALID_FARM_D_TAG.to_string(),
        },
        name: "Plot 1".to_string(),
        about: None,
        location: Some(RadrootsPlotLocation {
            primary: None,
            city: None,
            region: None,
            country: None,
            gcs: sample_gcs("9q8yy"),
        }),
        tags: Some(vec!["orchard".to_string()]),
    }
}

fn sample_listing() -> RadrootsListing {
    let quantity =
        RadrootsCoreQuantity::new(RadrootsCoreDecimal::from(1u32), RadrootsCoreUnit::Each);
    let price_per_canonical_unit = RadrootsCoreQuantityPrice::new(
        RadrootsCoreMoney::new(RadrootsCoreDecimal::from(10u32), RadrootsCoreCurrency::USD),
        quantity.clone(),
    );

    RadrootsListing {
        d_tag: VALID_DOC_D_TAG.to_string(),
        farm: RadrootsListingFarmRef {
            pubkey: VALID_PUBKEY.to_string(),
            d_tag: VALID_FARM_D_TAG.to_string(),
        },
        product: RadrootsListingProduct {
            key: "nutmeg".to_string(),
            title: "Nutmeg".to_string(),
            category: "spice".to_string(),
            summary: None,
            process: None,
            lot: None,
            location: None,
            profile: None,
            year: None,
        },
        primary_bin_id: "bin-1".to_string(),
        bins: vec![RadrootsListingBin {
            bin_id: "bin-1".to_string(),
            quantity,
            price_per_canonical_unit,
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
        availability: Some(RadrootsListingAvailability::Window {
            start: Some(1),
            end: Some(2),
        }),
        delivery_method: Some(RadrootsListingDeliveryMethod::Shipping),
        location: None,
        images: None,
    }
}

fn sample_resource_area() -> RadrootsResourceArea {
    RadrootsResourceArea {
        d_tag: VALID_AREA_D_TAG.to_string(),
        name: "Banda Grove".to_string(),
        about: None,
        location: RadrootsResourceAreaLocation {
            primary: None,
            city: None,
            region: None,
            country: None,
            gcs: sample_gcs("pmb5v"),
        },
        tags: Some(vec!["nutmeg".to_string()]),
    }
}

fn sample_resource_cap() -> RadrootsResourceHarvestCap {
    RadrootsResourceHarvestCap {
        d_tag: VALID_CAP_D_TAG.to_string(),
        resource_area: RadrootsResourceAreaRef {
            pubkey: VALID_PUBKEY.to_string(),
            d_tag: VALID_AREA_D_TAG.to_string(),
        },
        product: RadrootsResourceHarvestProduct {
            key: "nutmeg".to_string(),
            category: Some("spice".to_string()),
        },
        start: 1,
        end: 2,
        cap_quantity: RadrootsCoreQuantity::new(decimal("1000"), RadrootsCoreUnit::MassG),
        display_amount: None,
        display_unit: None,
        display_label: None,
        tags: None,
    }
}

fn sample_document() -> RadrootsDocument {
    RadrootsDocument {
        d_tag: VALID_DOC_D_TAG.to_string(),
        doc_type: "charter".to_string(),
        title: "Charter".to_string(),
        version: "1.0.0".to_string(),
        summary: None,
        effective_at: None,
        body_markdown: None,
        subject: RadrootsDocumentSubject {
            pubkey: VALID_PUBKEY.to_string(),
            address: Some(format!("30340:{VALID_PUBKEY}:{VALID_FARM_D_TAG}")),
        },
        tags: Some(vec!["policy".to_string()]),
    }
}

#[test]
fn coop_encode_and_list_set_paths() {
    let tags = coop_build_tags(&sample_coop()).expect("coop tags");
    assert!(
        tags.iter()
            .any(|tag| tag.first().map(|v| v.as_str()) == Some("d"))
    );
    assert!(
        tags.iter()
            .any(|tag| tag.first().map(|v| v.as_str()) == Some("t"))
    );
    assert!(
        tags.iter()
            .any(|tag| tag.first().map(|v| v.as_str()) == Some("g"))
    );

    let mut coop = sample_coop();
    coop.tags = None;
    coop.location = None;
    let tags = coop_build_tags(&coop).expect("coop tags without optional fields");
    assert!(
        !tags
            .iter()
            .any(|tag| tag.first().map(|v| v.as_str()) == Some("t"))
    );
    assert!(
        !tags
            .iter()
            .any(|tag| tag.first().map(|v| v.as_str()) == Some("g"))
    );

    let mut coop = sample_coop();
    coop.d_tag = " ".to_string();
    let err = coop_build_tags(&coop).expect_err("empty d_tag");
    assert!(matches!(err, EventEncodeError::EmptyRequiredField("d_tag")));

    let mut coop = sample_coop();
    coop.name = " ".to_string();
    let err = coop_build_tags(&coop).expect_err("empty name");
    assert!(matches!(err, EventEncodeError::EmptyRequiredField("name")));

    let mut coop = sample_coop();
    coop.location.as_mut().expect("location").gcs.geohash = " ".to_string();
    let err = coop_build_tags(&coop).expect_err("empty geohash");
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("location.gcs.geohash")
    ));

    let mut coop = sample_coop();
    coop.d_tag = "invalid".to_string();
    let err = coop_build_tags(&coop).expect_err("invalid d_tag");
    assert!(matches!(err, EventEncodeError::InvalidField("d_tag")));

    let tags = coop_ref_tags(&RadrootsCoopRef {
        pubkey: VALID_PUBKEY.to_string(),
        d_tag: VALID_COOP_D_TAG.to_string(),
    })
    .expect("coop ref tags");
    assert_eq!(tags.len(), 2);

    let err = coop_ref_tags(&RadrootsCoopRef {
        pubkey: " ".to_string(),
        d_tag: VALID_COOP_D_TAG.to_string(),
    })
    .expect_err("empty coop pubkey");
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("coop.pubkey")
    ));

    let err = coop_ref_tags(&RadrootsCoopRef {
        pubkey: VALID_PUBKEY.to_string(),
        d_tag: " ".to_string(),
    })
    .expect_err("empty coop d_tag");
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("coop.d_tag")
    ));

    let err = coop_ref_tags(&RadrootsCoopRef {
        pubkey: VALID_PUBKEY.to_string(),
        d_tag: "invalid".to_string(),
    })
    .expect_err("invalid coop d_tag");
    assert!(matches!(err, EventEncodeError::InvalidField("coop.d_tag")));

    let err = coop_members_list_set("invalid", ["member"]).expect_err("invalid coop id");
    assert!(matches!(err, EventEncodeError::InvalidField("coop_id")));

    let err = coop_members_list_set(" ", ["member"]).expect_err("empty coop id");
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("coop_id")
    ));

    let members = coop_members_list_set(VALID_COOP_D_TAG, ["member"]).expect("members list set");
    assert_eq!(members.entries.len(), 1);
    assert_eq!(members.entries[0].tag, "p");

    let err = coop_members_list_set(VALID_COOP_D_TAG, [" "]).expect_err("empty member entry");
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("entry.values")
    ));

    let member_farms = coop_members_farms_list_set(
        VALID_COOP_D_TAG,
        vec![RadrootsFarmRef {
            pubkey: VALID_PUBKEY.to_string(),
            d_tag: VALID_FARM_D_TAG.to_string(),
        }],
    )
    .expect("member farms list set");
    assert_eq!(member_farms.entries.len(), 2);
    assert_eq!(member_farms.entries[0].tag, "a");
    assert_eq!(member_farms.entries[1].tag, "p");

    let member_farms_from_array = coop_members_farms_list_set(
        VALID_COOP_D_TAG,
        [RadrootsFarmRef {
            pubkey: VALID_PUBKEY.to_string(),
            d_tag: VALID_FARM_D_TAG.to_string(),
        }],
    )
    .expect("member farms list set array");
    assert_eq!(member_farms_from_array.entries.len(), 2);
    assert_eq!(member_farms_from_array.entries[0].tag, "a");
    assert_eq!(member_farms_from_array.entries[1].tag, "p");

    let err = coop_members_farms_list_set(
        VALID_COOP_D_TAG,
        vec![RadrootsFarmRef {
            pubkey: " ".to_string(),
            d_tag: VALID_FARM_D_TAG.to_string(),
        }],
    )
    .expect_err("empty farm pubkey");
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("farm.pubkey")
    ));

    let err = coop_members_farms_list_set(
        VALID_COOP_D_TAG,
        vec![RadrootsFarmRef {
            pubkey: VALID_PUBKEY.to_string(),
            d_tag: " ".to_string(),
        }],
    )
    .expect_err("empty farm d_tag");
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("farm.d_tag")
    ));

    let err = coop_members_farms_list_set(
        VALID_COOP_D_TAG,
        vec![RadrootsFarmRef {
            pubkey: VALID_PUBKEY.to_string(),
            d_tag: "invalid".to_string(),
        }],
    )
    .expect_err("invalid farm d_tag");
    assert!(matches!(err, EventEncodeError::InvalidField("farm.d_tag")));
}

#[test]
fn farm_encode_and_list_set_paths() {
    let tags = farm_build_tags(&sample_farm()).expect("farm tags");
    assert!(
        tags.iter()
            .any(|tag| tag.first().map(|v| v.as_str()) == Some("d"))
    );
    assert!(
        tags.iter()
            .any(|tag| tag.first().map(|v| v.as_str()) == Some("t"))
    );
    assert!(
        tags.iter()
            .any(|tag| tag.first().map(|v| v.as_str()) == Some("g"))
    );

    let mut farm = sample_farm();
    farm.tags = None;
    farm.location = None;
    let tags = farm_build_tags(&farm).expect("farm tags without optional fields");
    assert!(
        !tags
            .iter()
            .any(|tag| tag.first().map(|v| v.as_str()) == Some("t"))
    );
    assert!(
        !tags
            .iter()
            .any(|tag| tag.first().map(|v| v.as_str()) == Some("g"))
    );

    let mut farm = sample_farm();
    farm.d_tag = " ".to_string();
    let err = farm_build_tags(&farm).expect_err("empty d_tag");
    assert!(matches!(err, EventEncodeError::EmptyRequiredField("d_tag")));

    let mut farm = sample_farm();
    farm.name = " ".to_string();
    let err = farm_build_tags(&farm).expect_err("empty name");
    assert!(matches!(err, EventEncodeError::EmptyRequiredField("name")));

    let mut farm = sample_farm();
    farm.location.as_mut().expect("location").gcs.geohash = " ".to_string();
    let err = farm_build_tags(&farm).expect_err("empty geohash");
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("location.gcs.geohash")
    ));

    let tags = farm_ref_tags(&RadrootsFarmRef {
        pubkey: VALID_PUBKEY.to_string(),
        d_tag: VALID_FARM_D_TAG.to_string(),
    })
    .expect("farm ref tags");
    assert_eq!(tags.len(), 2);

    let err = farm_ref_tags(&RadrootsFarmRef {
        pubkey: " ".to_string(),
        d_tag: VALID_FARM_D_TAG.to_string(),
    })
    .expect_err("empty farm pubkey");
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("farm.pubkey")
    ));

    let err = farm_ref_tags(&RadrootsFarmRef {
        pubkey: VALID_PUBKEY.to_string(),
        d_tag: " ".to_string(),
    })
    .expect_err("empty farm d_tag");
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("farm.d_tag")
    ));

    let err = farm_ref_tags(&RadrootsFarmRef {
        pubkey: VALID_PUBKEY.to_string(),
        d_tag: "invalid".to_string(),
    })
    .expect_err("invalid farm d_tag");
    assert!(matches!(err, EventEncodeError::InvalidField("farm.d_tag")));

    let err = farm_members_list_set("invalid", ["member"]).expect_err("invalid farm id");
    assert!(matches!(err, EventEncodeError::InvalidField("farm_id")));

    let err = farm_members_list_set(VALID_FARM_D_TAG, [" "]).expect_err("empty member entry");
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("entry.values")
    ));

    let err = farm_listings_list_set(VALID_FARM_D_TAG, VALID_PUBKEY, [" "])
        .expect_err("empty listing id");
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("listing_id")
    ));

    let err = farm_listings_list_set(VALID_FARM_D_TAG, VALID_PUBKEY, ["invalid"])
        .expect_err("invalid listing id");
    assert!(matches!(err, EventEncodeError::InvalidField("listing_id")));
}

#[test]
fn plot_encode_paths() {
    let tags = plot_build_tags(&sample_plot()).expect("plot tags");
    assert!(
        tags.iter()
            .any(|tag| tag.first().map(|v| v.as_str()) == Some("a"))
    );
    assert!(
        tags.iter()
            .any(|tag| tag.first().map(|v| v.as_str()) == Some("p"))
    );
    assert!(
        tags.iter()
            .any(|tag| tag.first().map(|v| v.as_str()) == Some("t"))
    );
    assert!(
        tags.iter()
            .any(|tag| tag.first().map(|v| v.as_str()) == Some("g"))
    );

    let mut plot = sample_plot();
    plot.tags = None;
    plot.location = None;
    let tags = plot_build_tags(&plot).expect("plot tags without optional fields");
    assert!(
        !tags
            .iter()
            .any(|tag| tag.first().map(|v| v.as_str()) == Some("t"))
    );
    assert!(
        !tags
            .iter()
            .any(|tag| tag.first().map(|v| v.as_str()) == Some("g"))
    );

    let err = plot_address(" ", VALID_PLOT_D_TAG).expect_err("empty author pubkey");
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("plot.author_pubkey")
    ));

    let err = plot_address(VALID_PUBKEY, " ").expect_err("empty plot d_tag");
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("plot.d_tag")
    ));

    let err = plot_address(VALID_PUBKEY, "invalid").expect_err("invalid plot d_tag");
    assert!(matches!(err, EventEncodeError::InvalidField("plot.d_tag")));

    let mut plot = sample_plot();
    plot.d_tag = " ".to_string();
    let err = plot_build_tags(&plot).expect_err("empty plot d_tag");
    assert!(matches!(err, EventEncodeError::EmptyRequiredField("d_tag")));

    let mut plot = sample_plot();
    plot.name = " ".to_string();
    let err = plot_build_tags(&plot).expect_err("empty plot name");
    assert!(matches!(err, EventEncodeError::EmptyRequiredField("name")));

    let mut plot = sample_plot();
    plot.farm.pubkey = " ".to_string();
    let err = plot_build_tags(&plot).expect_err("empty farm pubkey");
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("farm.pubkey")
    ));

    let mut plot = sample_plot();
    plot.farm.d_tag = " ".to_string();
    let err = plot_build_tags(&plot).expect_err("empty farm d_tag");
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("farm.d_tag")
    ));

    let mut plot = sample_plot();
    plot.farm.d_tag = "invalid".to_string();
    let err = plot_build_tags(&plot).expect_err("invalid farm d_tag");
    assert!(matches!(err, EventEncodeError::InvalidField("farm.d_tag")));

    let mut plot = sample_plot();
    plot.location.as_mut().expect("location").gcs.geohash = " ".to_string();
    let err = plot_build_tags(&plot).expect_err("empty geohash");
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("location.gcs.geohash")
    ));
}

#[test]
fn listing_encode_paths() {
    let listing = sample_listing();
    let tags = listing_build_tags(&listing).expect("listing tags");
    assert!(
        tags.iter()
            .any(|tag| tag.first().map(|v| v.as_str()) == Some("d"))
    );

    let full_tags = listing_tags_full(&listing).expect("listing full tags");
    assert!(full_tags.iter().any(|tag| {
        tag.first().map(|v| v.as_str()) == Some("inventory")
            && tag.get(1).map(|v| v.as_str()) == Some("12")
    }));
    assert!(full_tags.iter().any(|tag| {
        tag.first().map(|v| v.as_str()) == Some("radroots:availability_start")
            && tag.get(1).map(|v| v.as_str()) == Some("1")
    }));
    assert!(full_tags.iter().any(|tag| {
        tag.first().map(|v| v.as_str()) == Some("expires_at")
            && tag.get(1).map(|v| v.as_str()) == Some("2")
    }));
    assert!(full_tags.iter().any(|tag| {
        tag.first().map(|v| v.as_str()) == Some("delivery")
            && tag.get(1).map(|v| v.as_str()) == Some("shipping")
    }));

    let with_trade_fields: fn() -> ListingTagOptions = ListingTagOptions::with_trade_fields;
    let option_tags =
        listing_tags_with_options(&listing, with_trade_fields()).expect("listing option tags");
    assert!(option_tags.iter().any(|tag| {
        tag.first().map(|v| v.as_str()) == Some("inventory")
            && tag.get(1).map(|v| v.as_str()) == Some("12")
    }));

    let mut listing_with_display_fallback = sample_listing();
    listing_with_display_fallback.bins[0].quantity = listing_with_display_fallback.bins[0]
        .quantity
        .clone()
        .with_label("fallback-label");
    listing_with_display_fallback.bins[0].display_amount = Some(decimal("1"));
    listing_with_display_fallback.bins[0].display_unit = Some(RadrootsCoreUnit::Each);
    listing_with_display_fallback.bins[0].display_label = None;
    let display_tags =
        listing_tags_with_options(&listing_with_display_fallback, ListingTagOptions::default())
            .expect("listing tags with display fallback");
    assert!(display_tags.iter().any(|tag| {
        tag.first().map(|v| v.as_str()) == Some("radroots:bin")
            && tag.last().map(|v| v.as_str()) == Some("fallback-label")
    }));

    let mut listing_with_geohash = sample_listing();
    listing_with_geohash.location = Some(RadrootsListingLocation {
        primary: "Origin".to_string(),
        city: None,
        region: None,
        country: None,
        lat: None,
        lng: None,
        geohash: Some("6gkzwgjzn".to_string()),
    });
    let decoded_tags = listing_tags_with_options(
        &listing_with_geohash,
        ListingTagOptions {
            include_geohash: false,
            include_gps: true,
            ..ListingTagOptions::default()
        },
    )
    .expect("listing tags with decoded geohash");
    assert!(decoded_tags.iter().any(|tag| {
        tag.first().map(|v| v.as_str()) == Some("l") && tag.get(2).map(|v| v.as_str()) == Some("dd")
    }));

    let mut listing_with_shared_geohash = sample_listing();
    listing_with_shared_geohash.location = Some(RadrootsListingLocation {
        primary: "Origin".to_string(),
        city: None,
        region: None,
        country: None,
        lat: None,
        lng: None,
        geohash: Some("6gkzwgjzn".to_string()),
    });
    let shared_geohash_tags =
        listing_tags_with_options(&listing_with_shared_geohash, ListingTagOptions::default())
            .expect("listing tags with shared geohash");
    assert!(shared_geohash_tags.iter().any(|tag| {
        tag.first().map(|v| v.as_str()) == Some("g")
            && tag.get(1).map(|v| v.as_str()) == Some("6gkzwgjzn")
    }));

    let mut listing_without_coordinates = sample_listing();
    listing_without_coordinates.location = Some(RadrootsListingLocation {
        primary: "Origin".to_string(),
        city: None,
        region: None,
        country: None,
        lat: None,
        lng: None,
        geohash: None,
    });
    let no_coordinates_tags =
        listing_tags_with_options(&listing_without_coordinates, ListingTagOptions::default())
            .expect("listing tags without coordinates");
    assert!(
        !no_coordinates_tags
            .iter()
            .any(|tag| tag.first().map(|v| v.as_str()) == Some("L"))
    );
    assert!(
        !no_coordinates_tags
            .iter()
            .any(|tag| tag.first().map(|v| v.as_str()) == Some("g"))
    );

    let mut listing_with_blank_optionals = sample_listing();
    listing_with_blank_optionals.product.summary = Some(" ".to_string());
    listing_with_blank_optionals.product.process = Some("null".to_string());
    listing_with_blank_optionals.product.location = Some(" ".to_string());
    listing_with_blank_optionals.product.profile = Some("null".to_string());
    listing_with_blank_optionals.product.year = Some(" ".to_string());
    listing_with_blank_optionals.location = Some(RadrootsListingLocation {
        primary: " ".to_string(),
        city: Some(" ".to_string()),
        region: Some("null".to_string()),
        country: Some(" ".to_string()),
        lat: None,
        lng: None,
        geohash: None,
    });
    let blank_optional_tags =
        listing_tags_with_options(&listing_with_blank_optionals, ListingTagOptions::default())
            .expect("listing tags with blank optional values");
    assert!(
        !blank_optional_tags
            .iter()
            .any(|tag| tag.first().map(|v| v.as_str()) == Some("summary"))
    );
    assert!(
        !blank_optional_tags
            .iter()
            .any(|tag| tag.first().map(|v| v.as_str()) == Some("location"))
    );

    let mut listing_no_gps = sample_listing();
    listing_no_gps.location = Some(RadrootsListingLocation {
        primary: "Origin".to_string(),
        city: None,
        region: None,
        country: None,
        lat: Some(37.0),
        lng: Some(-122.0),
        geohash: None,
    });
    let no_gps_tags = listing_tags_with_options(
        &listing_no_gps,
        ListingTagOptions {
            include_gps: false,
            ..ListingTagOptions::default()
        },
    )
    .expect("listing tags without gps labels");
    assert!(
        !no_gps_tags
            .iter()
            .any(|tag| tag.first().map(|v| v.as_str()) == Some("L"))
    );

    let mut listing_without_availability = sample_listing();
    listing_without_availability.availability = None;
    let no_availability_tags =
        listing_tags_with_options(&listing_without_availability, with_trade_fields())
            .expect("listing tags without availability");
    assert!(
        !no_availability_tags
            .iter()
            .any(|tag| { tag.first().map(|v| v.as_str()) == Some("radroots:availability_start") })
    );

    let mut listing_pickup = sample_listing();
    listing_pickup.delivery_method = Some(RadrootsListingDeliveryMethod::Pickup);
    let pickup_tags = listing_tags_with_options(&listing_pickup, with_trade_fields())
        .expect("listing tags with pickup delivery");
    assert!(pickup_tags.iter().any(|tag| {
        tag.first().map(|v| v.as_str()) == Some("delivery")
            && tag.get(1).map(|v| v.as_str()) == Some("pickup")
    }));

    let mut listing_local = sample_listing();
    listing_local.delivery_method = Some(RadrootsListingDeliveryMethod::LocalDelivery);
    let local_tags = listing_tags_with_options(&listing_local, with_trade_fields())
        .expect("listing tags with local delivery");
    assert!(local_tags.iter().any(|tag| {
        tag.first().map(|v| v.as_str()) == Some("delivery")
            && tag.get(1).map(|v| v.as_str()) == Some("local_delivery")
    }));

    let mut listing_other_delivery = sample_listing();
    listing_other_delivery.delivery_method = Some(RadrootsListingDeliveryMethod::Other {
        method: "courier".to_string(),
    });
    let other_delivery_tags =
        listing_tags_with_options(&listing_other_delivery, with_trade_fields())
            .expect("listing tags with other delivery");
    assert!(other_delivery_tags.iter().any(|tag| {
        tag.first().map(|v| v.as_str()) == Some("delivery")
            && tag.get(1).map(|v| v.as_str()) == Some("other")
            && tag.get(2).map(|v| v.as_str()) == Some("courier")
    }));

    let mut invalid = sample_listing();
    invalid.bins[0].bin_id = " ".to_string();
    let err = listing_tags_with_options(&invalid, ListingTagOptions::default())
        .expect_err("empty bin_id");
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("bin_id")
    ));

    let mut invalid = sample_listing();
    invalid.bins[0].display_price = None;
    invalid.bins[0].display_price_unit = Some(RadrootsCoreUnit::Each);
    let err = listing_tags_with_options(&invalid, ListingTagOptions::default())
        .expect_err("missing display price");
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("bin.display_price")
    ));

    let mut invalid = sample_listing();
    invalid.bins[0].display_price = Some(RadrootsCoreMoney::new(
        decimal("10"),
        RadrootsCoreCurrency::USD,
    ));
    invalid.bins[0].display_price_unit = None;
    let err = listing_tags_with_options(&invalid, ListingTagOptions::default())
        .expect_err("missing display price unit");
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("bin.display_price_unit")
    ));

    let mut listing_with_display_price = sample_listing();
    listing_with_display_price.bins[0].display_price = Some(RadrootsCoreMoney::new(
        decimal("10"),
        RadrootsCoreCurrency::USD,
    ));
    listing_with_display_price.bins[0].display_price_unit = Some(RadrootsCoreUnit::Each);
    let display_price_tags =
        listing_tags_with_options(&listing_with_display_price, ListingTagOptions::default())
            .expect("listing tags with display price");
    assert!(display_price_tags.iter().any(|tag| {
        tag.first().map(|v| v.as_str()) == Some("radroots:price")
            && tag.get(6).map(|v| v.as_str()) == Some("10")
            && tag.get(7).map(|v| v.as_str()) == Some("each")
    }));

    let mut invalid = listing_with_display_price.clone();
    invalid.bins[0].display_price = Some(RadrootsCoreMoney::new(
        decimal("10"),
        RadrootsCoreCurrency::EUR,
    ));
    let err = listing_tags_with_options(&invalid, ListingTagOptions::default())
        .expect_err("display price currency mismatch");
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("bin.display_price")
    ));

    let mut invalid = sample_listing();
    invalid.bins[0].display_amount = None;
    invalid.bins[0].display_unit = Some(RadrootsCoreUnit::Each);
    let err = listing_tags_with_options(&invalid, ListingTagOptions::default())
        .expect_err("missing display amount");
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("bin.display_amount")
    ));

    let mut invalid = sample_listing();
    invalid.bins[0].quantity = RadrootsCoreQuantity::new(decimal("1"), RadrootsCoreUnit::MassKg);
    invalid.bins[0].price_per_canonical_unit = RadrootsCoreQuantityPrice::new(
        RadrootsCoreMoney::new(decimal("10"), RadrootsCoreCurrency::USD),
        RadrootsCoreQuantity::new(decimal("1"), RadrootsCoreUnit::MassG),
    );
    let err = listing_tags_with_options(&invalid, ListingTagOptions::default())
        .expect_err("non-canonical bin quantity");
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("bin.quantity")
    ));

    let mut invalid = sample_listing();
    invalid.bins[0].price_per_canonical_unit = RadrootsCoreQuantityPrice::new(
        RadrootsCoreMoney::new(decimal("10"), RadrootsCoreCurrency::USD),
        RadrootsCoreQuantity::new(decimal("2"), RadrootsCoreUnit::Each),
    );
    let err = listing_tags_with_options(&invalid, ListingTagOptions::default())
        .expect_err("price must be per canonical unit");
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("bin.price_per_canonical_unit")
    ));

    let mut invalid = sample_listing();
    invalid.bins[0].price_per_canonical_unit = RadrootsCoreQuantityPrice::new(
        RadrootsCoreMoney::new(decimal("10"), RadrootsCoreCurrency::USD),
        RadrootsCoreQuantity::new(decimal("1"), RadrootsCoreUnit::MassG),
    );
    let err = listing_tags_with_options(&invalid, ListingTagOptions::default())
        .expect_err("non-convertible bin total price");
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("bin.price_per_canonical_unit")
    ));

    let mut invalid = sample_listing();
    invalid.farm.d_tag = " ".to_string();
    let err = listing_build_tags(&invalid).expect_err("empty listing farm d_tag");
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("farm.d_tag")
    ));

    let mut invalid = sample_listing();
    invalid.d_tag = " ".to_string();
    let err =
        listing_tags_with_options(&invalid, ListingTagOptions::default()).expect_err("empty d");
    assert!(matches!(err, EventEncodeError::EmptyRequiredField("d")));

    let mut invalid = sample_listing();
    invalid.primary_bin_id = " ".to_string();
    let err = listing_tags_with_options(&invalid, ListingTagOptions::default())
        .expect_err("empty primary_bin_id");
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("primary_bin_id")
    ));

    let mut invalid = sample_listing();
    invalid.bins.clear();
    let err =
        listing_tags_with_options(&invalid, ListingTagOptions::default()).expect_err("empty bins");
    assert!(matches!(err, EventEncodeError::EmptyRequiredField("bins")));
}

#[test]
fn resource_area_encode_and_list_set_paths() {
    let tags = resource_area_build_tags(&sample_resource_area()).expect("resource area tags");
    assert!(
        tags.iter()
            .any(|tag| tag.first().map(|v| v.as_str()) == Some("d"))
    );
    assert!(
        tags.iter()
            .any(|tag| tag.first().map(|v| v.as_str()) == Some("g"))
    );
    assert!(
        tags.iter()
            .any(|tag| tag.first().map(|v| v.as_str()) == Some("t"))
    );

    let mut area = sample_resource_area();
    area.tags = None;
    let tags = resource_area_build_tags(&area).expect("resource area tags without optional tags");
    assert!(
        !tags
            .iter()
            .any(|tag| tag.first().map(|v| v.as_str()) == Some("t"))
    );

    let mut area = sample_resource_area();
    area.d_tag = " ".to_string();
    let err = resource_area_build_tags(&area).expect_err("empty d_tag");
    assert!(matches!(err, EventEncodeError::EmptyRequiredField("d_tag")));

    let mut area = sample_resource_area();
    area.name = " ".to_string();
    let err = resource_area_build_tags(&area).expect_err("empty name");
    assert!(matches!(err, EventEncodeError::EmptyRequiredField("name")));

    let mut area = sample_resource_area();
    area.location.gcs.geohash = " ".to_string();
    let err = resource_area_build_tags(&area).expect_err("empty geohash");
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("location.gcs.geohash")
    ));

    let tags = resource_area_ref_tags(&RadrootsResourceAreaRef {
        pubkey: VALID_PUBKEY.to_string(),
        d_tag: VALID_AREA_D_TAG.to_string(),
    })
    .expect("resource area ref tags");
    assert_eq!(tags.len(), 2);

    let err = resource_area_ref_tags(&RadrootsResourceAreaRef {
        pubkey: " ".to_string(),
        d_tag: VALID_AREA_D_TAG.to_string(),
    })
    .expect_err("empty resource area pubkey");
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("resource_area.pubkey")
    ));

    let err = resource_area_ref_tags(&RadrootsResourceAreaRef {
        pubkey: VALID_PUBKEY.to_string(),
        d_tag: " ".to_string(),
    })
    .expect_err("empty resource area d_tag");
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("resource_area.d_tag")
    ));

    let err = resource_area_ref_tags(&RadrootsResourceAreaRef {
        pubkey: VALID_PUBKEY.to_string(),
        d_tag: "invalid".to_string(),
    })
    .expect_err("invalid resource area d_tag");
    assert!(matches!(
        err,
        EventEncodeError::InvalidField("resource_area.d_tag")
    ));

    let err = resource_area_members_farms_list_set(
        "invalid",
        vec![RadrootsFarmRef {
            pubkey: VALID_PUBKEY.to_string(),
            d_tag: VALID_FARM_D_TAG.to_string(),
        }],
    )
    .expect_err("invalid area id");
    assert!(matches!(err, EventEncodeError::InvalidField("area_id")));

    let err =
        resource_area_stewards_list_set(VALID_AREA_D_TAG, [" "]).expect_err("empty steward entry");
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("entry.values")
    ));

    let stewards =
        resource_area_stewards_list_set(VALID_AREA_D_TAG, ["steward"]).expect("stewards list set");
    assert_eq!(stewards.entries.len(), 1);
    assert_eq!(stewards.entries[0].tag, "p");

    let err = resource_area_members_farms_list_set(
        VALID_AREA_D_TAG,
        vec![RadrootsFarmRef {
            pubkey: " ".to_string(),
            d_tag: VALID_FARM_D_TAG.to_string(),
        }],
    )
    .expect_err("empty farm pubkey");
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("farm.pubkey")
    ));

    let err = resource_area_members_farms_list_set(
        VALID_AREA_D_TAG,
        vec![RadrootsFarmRef {
            pubkey: VALID_PUBKEY.to_string(),
            d_tag: " ".to_string(),
        }],
    )
    .expect_err("empty farm d_tag");
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("farm.d_tag")
    ));

    let err = resource_area_members_farms_list_set(
        VALID_AREA_D_TAG,
        vec![RadrootsFarmRef {
            pubkey: VALID_PUBKEY.to_string(),
            d_tag: "invalid".to_string(),
        }],
    )
    .expect_err("invalid farm d_tag");
    assert!(matches!(err, EventEncodeError::InvalidField("farm.d_tag")));

    let err = resource_area_members_plots_list_set(
        VALID_AREA_D_TAG,
        vec![RadrootsPlotRef {
            pubkey: " ".to_string(),
            d_tag: VALID_PLOT_D_TAG.to_string(),
        }],
    )
    .expect_err("empty plot pubkey");
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("plot.pubkey")
    ));

    let err = resource_area_members_plots_list_set(
        VALID_AREA_D_TAG,
        vec![RadrootsPlotRef {
            pubkey: VALID_PUBKEY.to_string(),
            d_tag: " ".to_string(),
        }],
    )
    .expect_err("empty plot d_tag");
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("plot.d_tag")
    ));

    let err = resource_area_members_plots_list_set(
        VALID_AREA_D_TAG,
        vec![RadrootsPlotRef {
            pubkey: VALID_PUBKEY.to_string(),
            d_tag: "invalid".to_string(),
        }],
    )
    .expect_err("invalid plot d_tag");
    assert!(matches!(err, EventEncodeError::InvalidField("plot.d_tag")));
}

#[test]
fn resource_harvest_cap_encode_paths() {
    let tags = resource_harvest_cap_build_tags(&sample_resource_cap()).expect("resource cap tags");
    assert!(
        tags.iter()
            .any(|tag| tag.first().map(|v| v.as_str()) == Some("category"))
    );

    let mut cap = sample_resource_cap();
    cap.product.category = None;
    let tags = resource_harvest_cap_build_tags(&cap).expect("resource cap tags without category");
    assert!(
        !tags
            .iter()
            .any(|tag| tag.first().map(|v| v.as_str()) == Some("category"))
    );

    let mut cap = sample_resource_cap();
    cap.d_tag = " ".to_string();
    let err = resource_harvest_cap_build_tags(&cap).expect_err("empty cap d_tag");
    assert!(matches!(err, EventEncodeError::EmptyRequiredField("d_tag")));

    let mut cap = sample_resource_cap();
    cap.d_tag = "invalid".to_string();
    let err = resource_harvest_cap_build_tags(&cap).expect_err("invalid cap d_tag");
    assert!(matches!(err, EventEncodeError::InvalidField("d_tag")));

    let mut cap = sample_resource_cap();
    cap.product.key = " ".to_string();
    let err = resource_harvest_cap_build_tags(&cap).expect_err("empty product key");
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("product.key")
    ));

    let mut cap = sample_resource_cap();
    cap.resource_area.pubkey = " ".to_string();
    let err = resource_harvest_cap_build_tags(&cap).expect_err("empty resource_area pubkey");
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("resource_area.pubkey")
    ));

    let mut cap = sample_resource_cap();
    cap.resource_area.d_tag = " ".to_string();
    let err = resource_harvest_cap_build_tags(&cap).expect_err("empty resource_area d_tag");
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("resource_area.d_tag")
    ));

    let mut cap = sample_resource_cap();
    cap.resource_area.d_tag = "invalid".to_string();
    let err = resource_harvest_cap_build_tags(&cap).expect_err("invalid resource_area d_tag");
    assert!(matches!(
        err,
        EventEncodeError::InvalidField("resource_area.d_tag")
    ));
}

#[test]
fn document_encode_paths() {
    let tags = document_build_tags(&sample_document()).expect("document tags");
    assert!(
        tags.iter()
            .any(|tag| tag.first().map(|v| v.as_str()) == Some("d"))
    );
    assert!(
        tags.iter()
            .any(|tag| tag.first().map(|v| v.as_str()) == Some("p"))
    );
    assert!(
        tags.iter()
            .any(|tag| tag.first().map(|v| v.as_str()) == Some("a"))
    );
    assert!(
        tags.iter()
            .any(|tag| tag.first().map(|v| v.as_str()) == Some("t"))
    );

    let mut document = sample_document();
    document.subject.address = None;
    document.tags = None;
    let tags = document_build_tags(&document).expect("document without optional tags");
    assert!(
        !tags
            .iter()
            .any(|tag| tag.first().map(|v| v.as_str()) == Some("a"))
    );
    assert!(
        !tags
            .iter()
            .any(|tag| tag.first().map(|v| v.as_str()) == Some("t"))
    );

    let mut document = sample_document();
    document.d_tag = " ".to_string();
    let err = document_build_tags(&document).expect_err("empty d_tag");
    assert!(matches!(err, EventEncodeError::EmptyRequiredField("d_tag")));

    let mut document = sample_document();
    document.doc_type = " ".to_string();
    let err = document_build_tags(&document).expect_err("empty doc_type");
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("doc_type")
    ));

    let mut document = sample_document();
    document.title = " ".to_string();
    let err = document_build_tags(&document).expect_err("empty title");
    assert!(matches!(err, EventEncodeError::EmptyRequiredField("title")));

    let mut document = sample_document();
    document.version = " ".to_string();
    let err = document_build_tags(&document).expect_err("empty version");
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("version")
    ));

    let mut document = sample_document();
    document.subject.pubkey = " ".to_string();
    let err = document_build_tags(&document).expect_err("empty subject pubkey");
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("subject.pubkey")
    ));

    let mut document = sample_document();
    document.subject.address = Some(" ".to_string());
    let err = document_build_tags(&document).expect_err("empty subject address");
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("subject.address")
    ));

    let mut document = sample_document();
    document.d_tag = "invalid".to_string();
    let err = document_build_tags(&document).expect_err("invalid d_tag");
    assert!(matches!(err, EventEncodeError::InvalidField("d_tag")));
}

#[test]
fn resource_harvest_cap_money_type_sanity() {
    let money = RadrootsCoreMoney::new(decimal("1"), RadrootsCoreCurrency::USD);
    assert_eq!(money.amount.to_string(), "1");
}
