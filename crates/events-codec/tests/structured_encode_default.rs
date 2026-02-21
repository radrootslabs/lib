use radroots_core::{
    RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreQuantity,
    RadrootsCoreQuantityPrice, RadrootsCoreUnit,
};
use radroots_events::coop::{RadrootsCoop, RadrootsCoopLocation, RadrootsCoopRef};
use radroots_events::document::{RadrootsDocument, RadrootsDocumentSubject};
use radroots_events::farm::{
    RadrootsFarm, RadrootsFarmLocation, RadrootsFarmRef, RadrootsGcsLocation, RadrootsGeoJsonPoint,
    RadrootsGeoJsonPolygon,
};
use radroots_events::list_set::RadrootsListSet;
use radroots_events::listing::{
    RadrootsListing, RadrootsListingBin, RadrootsListingFarmRef, RadrootsListingProduct,
};
use radroots_events::plot::{RadrootsPlot, RadrootsPlotLocation, RadrootsPlotRef};
use radroots_events::resource_area::{
    RadrootsResourceArea, RadrootsResourceAreaLocation, RadrootsResourceAreaRef,
};
use radroots_events::resource_cap::{RadrootsResourceHarvestCap, RadrootsResourceHarvestProduct};
use radroots_events_codec::coop::encode::{coop_build_tags, coop_ref_tags};
use radroots_events_codec::coop::list_sets::{
    coop_admins_list_set, coop_items_list_set, coop_members_farms_list_set, coop_members_list_set,
    coop_owners_list_set, member_of_coops_list_set,
};
use radroots_events_codec::document::encode::document_build_tags;
use radroots_events_codec::error::EventEncodeError;
use radroots_events_codec::farm::encode::{farm_build_tags, farm_ref_tags};
use radroots_events_codec::farm::list_sets::{
    farm_listings_list_set, farm_listings_list_set_from_listings, farm_members_list_set,
    farm_owners_list_set, farm_plots_list_set, farm_plots_list_set_from_plots,
    farm_workers_list_set, member_of_farms_list_set,
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

const TEST_PUBKEY_HEX: &str = "58e318557257f2ab58a415d21bb57082b4824cf667a1d64e72bcbc5acc018c62";

fn sample_gcs() -> RadrootsGcsLocation {
    RadrootsGcsLocation {
        lat: 37.0,
        lng: -122.0,
        geohash: "9q8yy".to_string(),
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

fn sample_listing(d_tag: &str) -> RadrootsListing {
    let quantity =
        RadrootsCoreQuantity::new(RadrootsCoreDecimal::from(1u32), RadrootsCoreUnit::Each);
    let price = RadrootsCoreQuantityPrice::new(
        RadrootsCoreMoney::new(RadrootsCoreDecimal::from(10u32), RadrootsCoreCurrency::USD),
        quantity.clone(),
    );
    RadrootsListing {
        d_tag: d_tag.to_string(),
        farm: RadrootsListingFarmRef {
            pubkey: TEST_PUBKEY_HEX.to_string(),
            d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
        },
        product: RadrootsListingProduct {
            key: "sku".to_string(),
            title: "Widget".to_string(),
            category: "Tools".to_string(),
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
            price_per_canonical_unit: price,
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

#[test]
fn structured_build_tags_cover_optional_and_error_paths() {
    let farm = RadrootsFarm {
        d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
        name: "Farm".to_string(),
        about: None,
        website: None,
        picture: None,
        banner: None,
        location: Some(RadrootsFarmLocation {
            primary: Some("farm".to_string()),
            city: None,
            region: None,
            country: None,
            gcs: sample_gcs(),
        }),
        tags: Some(vec!["organic".to_string(), " ".to_string()]),
    };
    let farm_tags = farm_build_tags(&farm).unwrap();
    assert!(farm_tags.iter().any(|tag| tag[0] == "d"));
    assert!(farm_tags
        .iter()
        .any(|tag| tag[0] == "t" && tag[1] == "organic"));
    assert!(farm_tags.iter().any(|tag| tag[0] == "g"));

    let mut invalid_farm = farm.clone();
    invalid_farm.location.as_mut().unwrap().gcs.geohash = " ".to_string();
    let err = farm_build_tags(&invalid_farm).unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("location.gcs.geohash")
    ));

    let farm_ref_tags = farm_ref_tags(&RadrootsFarmRef {
        pubkey: TEST_PUBKEY_HEX.to_string(),
        d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
    })
    .unwrap();
    assert_eq!(farm_ref_tags.len(), 2);

    let coop = RadrootsCoop {
        d_tag: "AAAAAAAAAAAAAAAAAAAAAQ".to_string(),
        name: "Coop".to_string(),
        about: None,
        website: None,
        picture: None,
        banner: None,
        location: Some(RadrootsCoopLocation {
            primary: Some("coop".to_string()),
            city: None,
            region: None,
            country: None,
            gcs: sample_gcs(),
        }),
        tags: Some(vec!["co-op".to_string(), " ".to_string()]),
    };
    let coop_tags = coop_build_tags(&coop).unwrap();
    assert!(coop_tags.iter().any(|tag| tag[0] == "g"));
    assert!(coop_tags
        .iter()
        .any(|tag| tag[0] == "t" && tag[1] == "co-op"));
    let coop_ref_tags = coop_ref_tags(&RadrootsCoopRef {
        pubkey: TEST_PUBKEY_HEX.to_string(),
        d_tag: "AAAAAAAAAAAAAAAAAAAAAQ".to_string(),
    })
    .unwrap();
    assert_eq!(coop_ref_tags.len(), 2);

    let document = RadrootsDocument {
        d_tag: "AAAAAAAAAAAAAAAAAAAAAg".to_string(),
        doc_type: "charter".to_string(),
        title: "Charter".to_string(),
        version: "1.0.0".to_string(),
        summary: None,
        effective_at: None,
        body_markdown: None,
        subject: RadrootsDocumentSubject {
            pubkey: TEST_PUBKEY_HEX.to_string(),
            address: Some("30340:58e318557257f2ab58a415d21bb57082b4824cf667a1d64e72bcbc5acc018c62:AAAAAAAAAAAAAAAAAAAAAA".to_string()),
        },
        tags: Some(vec!["policy".to_string(), " ".to_string()]),
    };
    let doc_tags = document_build_tags(&document).unwrap();
    assert!(doc_tags.iter().any(|tag| tag[0] == "p"));
    assert!(doc_tags.iter().any(|tag| tag[0] == "a"));
    assert!(doc_tags
        .iter()
        .any(|tag| tag[0] == "t" && tag[1] == "policy"));

    let mut invalid_document = document.clone();
    invalid_document.subject.address = Some(" ".to_string());
    let err = document_build_tags(&invalid_document).unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("subject.address")
    ));

    let plot = RadrootsPlot {
        d_tag: "AAAAAAAAAAAAAAAAAAAABQ".to_string(),
        farm: RadrootsFarmRef {
            pubkey: TEST_PUBKEY_HEX.to_string(),
            d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
        },
        name: "Plot".to_string(),
        about: None,
        location: Some(RadrootsPlotLocation {
            primary: Some("plot".to_string()),
            city: None,
            region: None,
            country: None,
            gcs: sample_gcs(),
        }),
        tags: Some(vec!["shade-grown".to_string(), " ".to_string()]),
    };
    let plot_tags = plot_build_tags(&plot).unwrap();
    assert!(plot_tags.iter().any(|tag| tag[0] == "a"));
    assert!(plot_tags.iter().any(|tag| tag[0] == "p"));
    assert!(plot_tags.iter().any(|tag| tag[0] == "g"));
    assert!(plot_tags
        .iter()
        .any(|tag| tag[0] == "t" && tag[1] == "shade-grown"));

    let mut invalid_plot = plot.clone();
    invalid_plot.location.as_mut().unwrap().gcs.geohash = " ".to_string();
    let err = plot_build_tags(&invalid_plot).unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("location.gcs.geohash")
    ));

    let err = plot_address("", "AAAAAAAAAAAAAAAAAAAABQ").unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("plot.author_pubkey")
    ));

    let area = RadrootsResourceArea {
        d_tag: "AAAAAAAAAAAAAAAAAAAAAw".to_string(),
        name: "Area".to_string(),
        about: None,
        location: RadrootsResourceAreaLocation {
            primary: None,
            city: None,
            region: None,
            country: None,
            gcs: sample_gcs(),
        },
        tags: Some(vec!["orchard".to_string(), " ".to_string()]),
    };
    let area_tags = resource_area_build_tags(&area).unwrap();
    assert!(area_tags.iter().any(|tag| tag[0] == "d"));
    assert!(area_tags.iter().any(|tag| tag[0] == "g"));
    assert!(area_tags
        .iter()
        .any(|tag| tag[0] == "t" && tag[1] == "orchard"));
    let area_ref_tags = resource_area_ref_tags(&RadrootsResourceAreaRef {
        pubkey: TEST_PUBKEY_HEX.to_string(),
        d_tag: "AAAAAAAAAAAAAAAAAAAAAw".to_string(),
    })
    .unwrap();
    assert_eq!(area_ref_tags.len(), 2);

    let mut invalid_area = area.clone();
    invalid_area.location.gcs.geohash = " ".to_string();
    let err = resource_area_build_tags(&invalid_area).unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("location.gcs.geohash")
    ));

    let cap = RadrootsResourceHarvestCap {
        d_tag: "AAAAAAAAAAAAAAAAAAAABA".to_string(),
        resource_area: RadrootsResourceAreaRef {
            pubkey: TEST_PUBKEY_HEX.to_string(),
            d_tag: "AAAAAAAAAAAAAAAAAAAAAw".to_string(),
        },
        product: RadrootsResourceHarvestProduct {
            key: "nutmeg".to_string(),
            category: Some("spice".to_string()),
        },
        start: 1,
        end: 2,
        cap_quantity: RadrootsCoreQuantity::new(
            RadrootsCoreDecimal::from(1000u32),
            RadrootsCoreUnit::MassG,
        ),
        display_amount: None,
        display_unit: None,
        display_label: None,
        tags: Some(vec!["seasonal".to_string(), " ".to_string()]),
    };
    let cap_tags = resource_harvest_cap_build_tags(&cap).unwrap();
    assert!(cap_tags
        .iter()
        .any(|tag| tag[0] == "category" && tag[1] == "spice"));
    assert!(cap_tags
        .iter()
        .any(|tag| tag[0] == "t" && tag[1] == "seasonal"));

    let mut invalid_cap = cap.clone();
    invalid_cap.product.key = " ".to_string();
    let err = resource_harvest_cap_build_tags(&invalid_cap).unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("product.key")
    ));
}

#[test]
fn structured_build_tags_cover_required_field_errors() {
    let document = RadrootsDocument {
        d_tag: "AAAAAAAAAAAAAAAAAAAAAg".to_string(),
        doc_type: "charter".to_string(),
        title: "Charter".to_string(),
        version: "1.0.0".to_string(),
        summary: None,
        effective_at: None,
        body_markdown: None,
        subject: RadrootsDocumentSubject {
            pubkey: TEST_PUBKEY_HEX.to_string(),
            address: Some(
                "30340:58e318557257f2ab58a415d21bb57082b4824cf667a1d64e72bcbc5acc018c62:AAAAAAAAAAAAAAAAAAAAAA"
                    .to_string(),
            ),
        },
        tags: None,
    };
    let document_tags = document_build_tags(&document).unwrap();
    assert!(document_tags.iter().any(|tag| tag[0] == "a"));

    let mut invalid_document = document.clone();
    invalid_document.d_tag = " ".to_string();
    let err = document_build_tags(&invalid_document).unwrap_err();
    assert!(matches!(err, EventEncodeError::EmptyRequiredField("d_tag")));
    invalid_document = document.clone();
    invalid_document.doc_type = " ".to_string();
    let err = document_build_tags(&invalid_document).unwrap_err();
    assert!(matches!(err, EventEncodeError::EmptyRequiredField("doc_type")));
    invalid_document = document.clone();
    invalid_document.title = " ".to_string();
    let err = document_build_tags(&invalid_document).unwrap_err();
    assert!(matches!(err, EventEncodeError::EmptyRequiredField("title")));
    invalid_document = document.clone();
    invalid_document.version = " ".to_string();
    let err = document_build_tags(&invalid_document).unwrap_err();
    assert!(matches!(err, EventEncodeError::EmptyRequiredField("version")));
    invalid_document = document.clone();
    invalid_document.subject.pubkey = " ".to_string();
    let err = document_build_tags(&invalid_document).unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("subject.pubkey")
    ));

    let farm = RadrootsFarm {
        d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
        name: "Farm".to_string(),
        about: None,
        website: None,
        picture: None,
        banner: None,
        location: None,
        tags: None,
    };
    let mut invalid_farm = farm.clone();
    invalid_farm.d_tag = " ".to_string();
    let err = farm_build_tags(&invalid_farm).unwrap_err();
    assert!(matches!(err, EventEncodeError::EmptyRequiredField("d_tag")));
    invalid_farm = farm.clone();
    invalid_farm.name = " ".to_string();
    let err = farm_build_tags(&invalid_farm).unwrap_err();
    assert!(matches!(err, EventEncodeError::EmptyRequiredField("name")));
    let err = farm_ref_tags(&RadrootsFarmRef {
        pubkey: " ".to_string(),
        d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
    })
    .unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("farm.pubkey")
    ));
    let err = farm_ref_tags(&RadrootsFarmRef {
        pubkey: TEST_PUBKEY_HEX.to_string(),
        d_tag: " ".to_string(),
    })
    .unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("farm.d_tag")
    ));

    let coop = RadrootsCoop {
        d_tag: "AAAAAAAAAAAAAAAAAAAAAQ".to_string(),
        name: "Coop".to_string(),
        about: None,
        website: None,
        picture: None,
        banner: None,
        location: None,
        tags: None,
    };
    let mut invalid_coop = coop.clone();
    invalid_coop.d_tag = " ".to_string();
    let err = coop_build_tags(&invalid_coop).unwrap_err();
    assert!(matches!(err, EventEncodeError::EmptyRequiredField("d_tag")));
    invalid_coop = coop.clone();
    invalid_coop.name = " ".to_string();
    let err = coop_build_tags(&invalid_coop).unwrap_err();
    assert!(matches!(err, EventEncodeError::EmptyRequiredField("name")));
    invalid_coop = coop.clone();
    invalid_coop.location = Some(RadrootsCoopLocation {
        primary: None,
        city: None,
        region: None,
        country: None,
        gcs: RadrootsGcsLocation {
            geohash: " ".to_string(),
            ..sample_gcs()
        },
    });
    let err = coop_build_tags(&invalid_coop).unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("location.gcs.geohash")
    ));
    let err = coop_ref_tags(&RadrootsCoopRef {
        pubkey: " ".to_string(),
        d_tag: "AAAAAAAAAAAAAAAAAAAAAQ".to_string(),
    })
    .unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("coop.pubkey")
    ));
    let err = coop_ref_tags(&RadrootsCoopRef {
        pubkey: TEST_PUBKEY_HEX.to_string(),
        d_tag: " ".to_string(),
    })
    .unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("coop.d_tag")
    ));

    let plot = RadrootsPlot {
        d_tag: "AAAAAAAAAAAAAAAAAAAABQ".to_string(),
        farm: RadrootsFarmRef {
            pubkey: TEST_PUBKEY_HEX.to_string(),
            d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
        },
        name: "Plot".to_string(),
        about: None,
        location: None,
        tags: None,
    };
    let mut invalid_plot = plot.clone();
    invalid_plot.d_tag = " ".to_string();
    let err = plot_build_tags(&invalid_plot).unwrap_err();
    assert!(matches!(err, EventEncodeError::EmptyRequiredField("d_tag")));
    invalid_plot = plot.clone();
    invalid_plot.name = " ".to_string();
    let err = plot_build_tags(&invalid_plot).unwrap_err();
    assert!(matches!(err, EventEncodeError::EmptyRequiredField("name")));
    invalid_plot = plot.clone();
    invalid_plot.farm.pubkey = " ".to_string();
    let err = plot_build_tags(&invalid_plot).unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("farm.pubkey")
    ));
    invalid_plot = plot.clone();
    invalid_plot.farm.d_tag = " ".to_string();
    let err = plot_build_tags(&invalid_plot).unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("farm.d_tag")
    ));
    let err = plot_address(TEST_PUBKEY_HEX, " ").unwrap_err();
    assert!(matches!(err, EventEncodeError::EmptyRequiredField("plot.d_tag")));

    let area = RadrootsResourceArea {
        d_tag: "AAAAAAAAAAAAAAAAAAAAAw".to_string(),
        name: "Area".to_string(),
        about: None,
        location: RadrootsResourceAreaLocation {
            primary: None,
            city: None,
            region: None,
            country: None,
            gcs: sample_gcs(),
        },
        tags: None,
    };
    let mut invalid_area = area.clone();
    invalid_area.d_tag = " ".to_string();
    let err = resource_area_build_tags(&invalid_area).unwrap_err();
    assert!(matches!(err, EventEncodeError::EmptyRequiredField("d_tag")));
    invalid_area = area.clone();
    invalid_area.name = " ".to_string();
    let err = resource_area_build_tags(&invalid_area).unwrap_err();
    assert!(matches!(err, EventEncodeError::EmptyRequiredField("name")));
    let err = resource_area_ref_tags(&RadrootsResourceAreaRef {
        pubkey: " ".to_string(),
        d_tag: "AAAAAAAAAAAAAAAAAAAAAw".to_string(),
    })
    .unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("resource_area.pubkey")
    ));
    let err = resource_area_ref_tags(&RadrootsResourceAreaRef {
        pubkey: TEST_PUBKEY_HEX.to_string(),
        d_tag: " ".to_string(),
    })
    .unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("resource_area.d_tag")
    ));

    let cap = RadrootsResourceHarvestCap {
        d_tag: "AAAAAAAAAAAAAAAAAAAABA".to_string(),
        resource_area: RadrootsResourceAreaRef {
            pubkey: TEST_PUBKEY_HEX.to_string(),
            d_tag: "AAAAAAAAAAAAAAAAAAAAAw".to_string(),
        },
        product: RadrootsResourceHarvestProduct {
            key: "nutmeg".to_string(),
            category: Some("spice".to_string()),
        },
        start: 1,
        end: 2,
        cap_quantity: RadrootsCoreQuantity::new(
            RadrootsCoreDecimal::from(1000u32),
            RadrootsCoreUnit::MassG,
        ),
        display_amount: None,
        display_unit: None,
        display_label: None,
        tags: None,
    };
    let mut invalid_cap = cap.clone();
    invalid_cap.d_tag = " ".to_string();
    let err = resource_harvest_cap_build_tags(&invalid_cap).unwrap_err();
    assert!(matches!(err, EventEncodeError::EmptyRequiredField("d_tag")));
    invalid_cap = cap.clone();
    invalid_cap.resource_area.pubkey = " ".to_string();
    let err = resource_harvest_cap_build_tags(&invalid_cap).unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("resource_area.pubkey")
    ));
    invalid_cap = cap.clone();
    invalid_cap.resource_area.d_tag = " ".to_string();
    let err = resource_harvest_cap_build_tags(&invalid_cap).unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("resource_area.d_tag")
    ));
    let mut no_category = cap.clone();
    no_category.product.category = Some(" ".to_string());
    let tags = resource_harvest_cap_build_tags(&no_category).unwrap();
    assert!(!tags.iter().any(|tag| tag[0] == "category"));
}

#[test]
fn structured_list_sets_cover_success_and_error_paths() {
    let farm_id = "AAAAAAAAAAAAAAAAAAAAAA";
    let members = farm_members_list_set(farm_id, [TEST_PUBKEY_HEX]).unwrap();
    assert_eq!(members.d_tag, format!("farm:{farm_id}:members"));
    let owners = farm_owners_list_set(farm_id, [TEST_PUBKEY_HEX]).unwrap();
    assert_eq!(owners.d_tag, format!("farm:{farm_id}:members.owners"));
    let workers = farm_workers_list_set(farm_id, [TEST_PUBKEY_HEX]).unwrap();
    assert_eq!(workers.d_tag, format!("farm:{farm_id}:members.workers"));

    let plots = farm_plots_list_set(farm_id, TEST_PUBKEY_HEX, ["AAAAAAAAAAAAAAAAAAAABQ"]).unwrap();
    assert_eq!(plots.d_tag, format!("farm:{farm_id}:plots"));
    assert_eq!(plots.entries.len(), 1);

    let listings =
        farm_listings_list_set(farm_id, TEST_PUBKEY_HEX, ["AAAAAAAAAAAAAAAAAAAAAg"]).unwrap();
    assert_eq!(listings.d_tag, format!("farm:{farm_id}:listings"));
    assert_eq!(listings.entries.len(), 1);

    let listings_from = farm_listings_list_set_from_listings(
        farm_id,
        TEST_PUBKEY_HEX,
        [sample_listing("AAAAAAAAAAAAAAAAAAAAAg")].iter(),
    )
    .unwrap();
    assert_eq!(listings_from.entries.len(), 1);

    let plots_from = farm_plots_list_set_from_plots(
        farm_id,
        TEST_PUBKEY_HEX,
        [RadrootsPlot {
            d_tag: "AAAAAAAAAAAAAAAAAAAABQ".to_string(),
            farm: RadrootsFarmRef {
                pubkey: TEST_PUBKEY_HEX.to_string(),
                d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
            },
            name: "plot".to_string(),
            about: None,
            location: None,
            tags: None,
        }]
        .iter(),
    )
    .unwrap();
    assert_eq!(plots_from.entries.len(), 1);

    let member_of_farms = member_of_farms_list_set([TEST_PUBKEY_HEX]).unwrap();
    assert_eq!(member_of_farms.d_tag, "member_of.farms");

    let err = farm_members_list_set("", [TEST_PUBKEY_HEX]).unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("farm_id")
    ));
    let err = farm_members_list_set(farm_id, [" "]).unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("entry.values")
    ));
    let err = farm_listings_list_set(farm_id, TEST_PUBKEY_HEX, [" "]).unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("listing_id")
    ));

    let coop_id = "AAAAAAAAAAAAAAAAAAAAAQ";
    let coop_members = coop_members_list_set(coop_id, [TEST_PUBKEY_HEX]).unwrap();
    assert_eq!(coop_members.d_tag, format!("coop:{coop_id}:members"));
    let coop_owners = coop_owners_list_set(coop_id, [TEST_PUBKEY_HEX]).unwrap();
    assert_eq!(coop_owners.d_tag, format!("coop:{coop_id}:members.owners"));
    let coop_admins = coop_admins_list_set(coop_id, [TEST_PUBKEY_HEX]).unwrap();
    assert_eq!(coop_admins.d_tag, format!("coop:{coop_id}:members.admins"));
    let coop_items = coop_items_list_set(coop_id, ["30340:pubkey:AAAAAAAAAAAAAAAAAAAAAA"]).unwrap();
    assert_eq!(coop_items.d_tag, format!("coop:{coop_id}:items"));
    let member_of_coops = member_of_coops_list_set([TEST_PUBKEY_HEX]).unwrap();
    assert_eq!(member_of_coops.d_tag, "member_of.coops");

    let coop_farms = coop_members_farms_list_set(
        coop_id,
        [RadrootsFarmRef {
            pubkey: TEST_PUBKEY_HEX.to_string(),
            d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
        }],
    )
    .unwrap();
    assert_eq!(coop_farms.entries.len(), 2);

    let err = coop_members_list_set("", [TEST_PUBKEY_HEX]).unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("coop_id")
    ));
    let err = coop_members_farms_list_set(
        coop_id,
        [RadrootsFarmRef {
            pubkey: "".to_string(),
            d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
        }],
    )
    .unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("farm.pubkey")
    ));

    let area_id = "AAAAAAAAAAAAAAAAAAAAAw";
    let resource_farms = resource_area_members_farms_list_set(
        area_id,
        [RadrootsFarmRef {
            pubkey: TEST_PUBKEY_HEX.to_string(),
            d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
        }],
    )
    .unwrap();
    assert_eq!(resource_farms.entries.len(), 2);

    let resource_plots = resource_area_members_plots_list_set(
        area_id,
        [RadrootsPlotRef {
            pubkey: TEST_PUBKEY_HEX.to_string(),
            d_tag: "AAAAAAAAAAAAAAAAAAAABQ".to_string(),
        }],
    )
    .unwrap();
    assert_eq!(resource_plots.entries.len(), 2);

    let resource_stewards = resource_area_stewards_list_set(area_id, [TEST_PUBKEY_HEX]).unwrap();
    assert_eq!(resource_stewards.entries.len(), 1);

    let err = resource_area_stewards_list_set("", [TEST_PUBKEY_HEX]).unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("area_id")
    ));
    let err = resource_area_members_plots_list_set(
        area_id,
        [RadrootsPlotRef {
            pubkey: "".to_string(),
            d_tag: "AAAAAAAAAAAAAAAAAAAABQ".to_string(),
        }],
    )
    .unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("plot.pubkey")
    ));
}

#[test]
fn structured_list_set_outputs_remain_deterministic() {
    let list_set: RadrootsListSet =
        farm_members_list_set("AAAAAAAAAAAAAAAAAAAAAA", [TEST_PUBKEY_HEX, TEST_PUBKEY_HEX])
            .unwrap();
    assert_eq!(list_set.entries.len(), 2);
    assert_eq!(list_set.entries[0].tag, "p");
}
