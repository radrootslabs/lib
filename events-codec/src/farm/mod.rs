pub mod decode;
pub mod encode;
pub mod list_sets;

#[cfg(test)]
mod tests {
    use radroots_events::farm::{
        RadrootsFarm,
        RadrootsFarmLocation,
        RadrootsFarmRef,
        RadrootsGcsLocation,
        RadrootsGeoJsonPoint,
        RadrootsGeoJsonPolygon,
    };
    use radroots_events::plot::RadrootsPlot;
    use crate::farm::encode::{farm_build_tags, farm_ref_tags};
    use crate::farm::list_sets::{
        farm_members_list_set,
        farm_listings_list_set_from_listings,
        farm_plots_list_set_from_plots,
        member_of_farms_list_set,
    };
    use radroots_core::{
        RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreQuantity,
        RadrootsCoreQuantityPrice, RadrootsCoreUnit,
    };
    use radroots_events::listing::{
        RadrootsListing, RadrootsListingBin, RadrootsListingFarmRef, RadrootsListingProduct,
    };

    #[test]
    fn farm_tags_include_required_fields() {
        let farm = RadrootsFarm {
            d_tag: "farm-1".to_string(),
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
                gcs: RadrootsGcsLocation {
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
                },
            }),
            tags: Some(vec!["orchard".to_string()]),
        };

        let tags = farm_build_tags(&farm).expect("tags");
        assert!(tags.iter().any(|tag| tag.get(0) == Some(&"d".to_string())));
        assert!(tags.iter().any(|tag| tag.get(0) == Some(&"t".to_string())));
        assert!(tags.iter().any(|tag| tag.get(0) == Some(&"g".to_string())));
    }

    #[test]
    fn farm_ref_tags_include_p_and_a() {
        let farm = RadrootsFarmRef {
            pubkey: "farm_pubkey".to_string(),
            d_tag: "farm-1".to_string(),
        };

        let tags = farm_ref_tags(&farm).expect("farm ref tags");
        let has_a = tags.iter().any(|tag| tag.get(0).map(|v| v.as_str()) == Some("a"));
        let has_p = tags.iter().any(|tag| tag.get(0).map(|v| v.as_str()) == Some("p"));
        assert!(has_a);
        assert!(has_p);
    }

    #[test]
    fn farm_list_sets_include_expected_tags() {
        let members = farm_members_list_set("farm-1", ["owner_pubkey"]).expect("members list");
        assert_eq!(members.d_tag, "farm:farm-1:members");
        assert_eq!(members.entries.len(), 1);
        assert_eq!(members.entries[0].tag, "p");

        let claims = member_of_farms_list_set(["farm_pubkey"]).expect("claims list");
        assert_eq!(claims.d_tag, "member_of.farms");
        assert_eq!(claims.entries.len(), 1);
        assert_eq!(claims.entries[0].tag, "p");
    }

    #[test]
    fn farm_plots_list_set_uses_plot_addresses() {
        let plots = vec![RadrootsPlot {
            d_tag: "plot-1".to_string(),
            farm: RadrootsFarmRef {
                pubkey: "farm_pubkey".to_string(),
                d_tag: "farm-1".to_string(),
            },
            name: "Plot 1".to_string(),
            about: None,
            location: None,
            tags: None,
        }];

        let plots_list = farm_plots_list_set_from_plots("farm-1", "farm_pubkey", &plots)
            .expect("plots list");
        assert_eq!(plots_list.d_tag, "farm:farm-1:plots");
        assert_eq!(plots_list.entries.len(), 1);
        assert_eq!(plots_list.entries[0].tag, "a");
        assert_eq!(
            plots_list.entries[0].values[0],
            "30350:farm_pubkey:plot-1"
        );
    }

    #[test]
    fn farm_listings_list_set_uses_listing_addresses() {
        let listings = vec![RadrootsListing {
            d_tag: "listing-1".to_string(),
            farm: RadrootsListingFarmRef {
                pubkey: "farm_pubkey".to_string(),
                d_tag: "farm-1".to_string(),
            },
            product: RadrootsListingProduct {
                key: "coffee".to_string(),
                title: "Coffee".to_string(),
                category: "coffee".to_string(),
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
                quantity: RadrootsCoreQuantity::new(
                    RadrootsCoreDecimal::from(1u32),
                    RadrootsCoreUnit::Each,
                ),
                price_per_canonical_unit: RadrootsCoreQuantityPrice::new(
                    RadrootsCoreMoney::new(
                        RadrootsCoreDecimal::from(10u32),
                        RadrootsCoreCurrency::USD,
                    ),
                    RadrootsCoreQuantity::new(
                        RadrootsCoreDecimal::from(1u32),
                        RadrootsCoreUnit::Each,
                    ),
                ),
                display_amount: None,
                display_unit: None,
                display_label: None,
                display_price: None,
                display_price_unit: None,
            }],
            discounts: None,
            inventory_available: None,
            availability: None,
            delivery_method: None,
            location: None,
            images: None,
        }];

        let listings_list = farm_listings_list_set_from_listings("farm-1", "farm_pubkey", &listings)
            .expect("listings list");
        assert_eq!(listings_list.d_tag, "farm:farm-1:listings");
        assert_eq!(listings_list.entries.len(), 1);
        assert_eq!(listings_list.entries[0].tag, "a");
        assert_eq!(
            listings_list.entries[0].values[0],
            "30402:farm_pubkey:listing-1"
        );
    }
}
