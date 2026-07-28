pub mod decode;
pub mod encode;
pub mod list_sets;

#[cfg(test)]
mod tests {
    use crate::error::EventEncodeError;
    #[cfg(feature = "serde_json")]
    use crate::error::EventParseError;
    #[cfg(feature = "serde_json")]
    use crate::farm::decode::{farm_from_event, parsed_from_event};
    use crate::farm::encode::{farm_build_tags, farm_ref_tags};
    use crate::farm::list_sets::{
        farm_members_list_set, farm_operational_listings_list_set_from_listings,
        farm_plots_list_set_from_plots, member_of_farms_list_set,
    };
    use radroots_core::{Currency, Decimal, Money, Quantity, QuantityPrice, Unit};
    use radroots_event::farm::{RadrootsFarm, RadrootsFarmPublicLocation, RadrootsFarmRef};
    use radroots_event::ids::{RadrootsDTag, RadrootsInventoryBinId};
    #[cfg(feature = "serde_json")]
    use radroots_event::kinds::KIND_FARM;
    use radroots_event::operational_listing::{
        RadrootsOperationalListing, RadrootsOperationalListingBin,
        RadrootsOperationalListingProduct,
    };
    use radroots_event::plot::RadrootsPlot;

    #[cfg(feature = "serde_json")]
    const EVENT_ID: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    #[cfg(feature = "serde_json")]
    const AUTHOR: &str = crate::test_fixtures::FIXTURE_ALICE_PUBLIC_KEY_HEX;
    #[cfg(feature = "serde_json")]
    const EVENT_SIG: &str = concat!(
        "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
        "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
    );

    fn d_tag(raw: &str) -> RadrootsDTag {
        raw.parse().unwrap()
    }

    fn bin_id(raw: &str) -> RadrootsInventoryBinId {
        raw.parse().unwrap()
    }

    #[test]
    fn farm_tags_include_required_fields() {
        let farm = RadrootsFarm {
            d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
            name: "Test Farm".to_string(),
            about: None,
            website: None,
            picture: None,
            banner: None,
            location: Some(RadrootsFarmPublicLocation {
                primary: "Test Farm".to_string(),
                city: Some("Santa Cruz".to_string()),
                region: Some("California".to_string()),
                country: Some("US".to_string()),
                geohash: "9q8yy".to_string(),
            }),
            tags: Some(vec!["orchard".to_string()]),
        };

        let tags = farm_build_tags(&farm).expect("tags");
        assert!(tags.iter().any(|tag| tag.first() == Some(&"d".to_string())));
        assert!(tags.iter().any(|tag| tag.first() == Some(&"t".to_string())));
        assert!(tags.iter().any(|tag| tag.first() == Some(&"g".to_string())));
    }

    #[test]
    fn farm_tags_allow_missing_optional_fields() {
        let farm = RadrootsFarm {
            d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
            name: "Test Farm".to_string(),
            about: None,
            website: None,
            picture: None,
            banner: None,
            location: None,
            tags: None,
        };

        let tags = farm_build_tags(&farm).expect("tags without optional fields");
        assert!(
            tags.iter()
                .any(|tag| tag.first().map(|v| v.as_str()) == Some("d"))
        );
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
    }

    #[test]
    fn farm_build_tags_rejects_invalid_d_tag() {
        let farm = RadrootsFarm {
            d_tag: "farm:invalid".to_string(),
            name: "Test Farm".to_string(),
            about: None,
            website: None,
            picture: None,
            banner: None,
            location: None,
            tags: None,
        };

        let err = farm_build_tags(&farm).expect_err("expected invalid d_tag");
        assert!(matches!(err, EventEncodeError::InvalidField("d_tag")));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    #[cfg(feature = "serde_json")]
    fn farm_decode_rejects_empty_d_tag_and_content() {
        let farm = RadrootsFarm {
            d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
            name: "Test Farm".to_string(),
            about: None,
            website: None,
            picture: None,
            banner: None,
            location: None,
            tags: None,
        };
        let content = serde_json::to_string(&farm).expect("farm content");

        let empty_d = farm_from_event(
            KIND_FARM,
            &[vec!["d".to_string(), " ".to_string()]],
            &content,
        )
        .expect_err("empty d tag");
        assert!(matches!(
            empty_d,
            crate::error::EventParseError::InvalidTag("d")
        ));

        let empty_content = farm_from_event(
            KIND_FARM,
            &[vec!["d".to_string(), "AAAAAAAAAAAAAAAAAAAAAA".to_string()]],
            " ",
        )
        .expect_err("empty content");
        assert!(matches!(
            empty_content,
            crate::error::EventParseError::InvalidJson("content")
        ));

        let with_empty_tag = farm_from_event(
            KIND_FARM,
            &[
                Vec::new(),
                vec!["d".to_string(), "AAAAAAAAAAAAAAAAAAAAAA".to_string()],
            ],
            &content,
        )
        .expect("empty unrelated tags are ignored");
        assert_eq!(with_empty_tag.name, "Test Farm");

        let parsed_error = parsed_from_event(
            EVENT_ID.to_string(),
            AUTHOR.to_string(),
            42,
            KIND_FARM + 1,
            content,
            vec![vec!["d".to_string(), "AAAAAAAAAAAAAAAAAAAAAA".to_string()]],
            EVENT_SIG.to_string(),
        )
        .expect_err("parsed wrapper propagates decode failures");
        assert!(matches!(
            parsed_error,
            crate::error::EventParseError::InvalidKind { .. }
        ));
    }

    #[test]
    fn farm_ref_tags_include_p_and_a() {
        let farm = RadrootsFarmRef {
            pubkey: "farm_pubkey".to_string(),
            d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
        };

        let tags = farm_ref_tags(&farm).expect("farm ref tags");
        let has_a = tags
            .iter()
            .any(|tag| tag.first().map(|v| v.as_str()) == Some("a"));
        let has_p = tags
            .iter()
            .any(|tag| tag.first().map(|v| v.as_str()) == Some("p"));
        assert!(has_a);
        assert!(has_p);

        let err = farm_ref_tags(&RadrootsFarmRef {
            pubkey: "farm_pubkey".to_string(),
            d_tag: "invalid".to_string(),
        })
        .expect_err("expected invalid farm.d_tag");
        assert!(matches!(err, EventEncodeError::InvalidField("farm.d_tag")));
    }

    #[test]
    fn farm_encode_rejects_empty_required_fields() {
        let mut farm = RadrootsFarm {
            d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
            name: "Test Farm".to_string(),
            about: None,
            website: None,
            picture: None,
            banner: None,
            location: Some(RadrootsFarmPublicLocation {
                primary: "Test Farm".to_string(),
                city: Some("Santa Cruz".to_string()),
                region: Some("California".to_string()),
                country: Some("US".to_string()),
                geohash: "9q8yy".to_string(),
            }),
            tags: None,
        };

        farm.d_tag = " ".to_string();
        let err = farm_build_tags(&farm).expect_err("expected empty d_tag");
        assert!(matches!(err, EventEncodeError::EmptyRequiredField("d_tag")));

        farm.d_tag = "AAAAAAAAAAAAAAAAAAAAAA".to_string();
        farm.name = " ".to_string();
        let err = farm_build_tags(&farm).expect_err("expected empty name");
        assert!(matches!(err, EventEncodeError::EmptyRequiredField("name")));

        farm.name = "Test Farm".to_string();
        farm.location.as_mut().expect("location").geohash = " ".to_string();
        let err = farm_build_tags(&farm).expect_err("expected empty geohash");
        assert!(matches!(
            err,
            EventEncodeError::EmptyRequiredField("location.geohash")
        ));

        farm.location.as_mut().expect("location").geohash = "9q8yy6".to_string();
        let err = farm_build_tags(&farm).expect_err("expected invalid geohash");
        assert!(matches!(
            err,
            EventEncodeError::InvalidField("location.geohash")
        ));

        let err = farm_ref_tags(&RadrootsFarmRef {
            pubkey: " ".to_string(),
            d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
        })
        .expect_err("expected empty farm.pubkey");
        assert!(matches!(
            err,
            EventEncodeError::EmptyRequiredField("farm.pubkey")
        ));

        let err = farm_ref_tags(&RadrootsFarmRef {
            pubkey: "farm_pubkey".to_string(),
            d_tag: " ".to_string(),
        })
        .expect_err("expected empty farm.d_tag");
        assert!(matches!(
            err,
            EventEncodeError::EmptyRequiredField("farm.d_tag")
        ));

        let mut farm = RadrootsFarm {
            d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
            name: "Test Farm".to_string(),
            about: None,
            website: None,
            picture: None,
            banner: None,
            location: Some(RadrootsFarmPublicLocation {
                primary: " ".to_string(),
                city: Some("null".to_string()),
                region: None,
                country: None,
                geohash: "9q8yy".to_string(),
            }),
            tags: None,
        };
        let err = farm_build_tags(&farm).expect_err("expected missing locality");
        assert!(matches!(
            err,
            EventEncodeError::EmptyRequiredField("location.locality")
        ));

        let location = farm.location.as_mut().expect("location");
        location.primary = "Test Farm".to_string();
        location.city = Some("Santa Cruz".to_string());
        let tags = farm_build_tags(&farm).expect("valid location after locality repair");
        assert!(
            tags.iter()
                .any(|tag| tag.first().map(|value| value.as_str()) == Some("g"))
        );
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    #[cfg(feature = "serde_json")]
    fn farm_decode_rejects_private_location_and_ops_shapes() {
        let farm = RadrootsFarm {
            d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
            name: "Test Farm".to_string(),
            about: None,
            website: None,
            picture: None,
            banner: None,
            location: Some(RadrootsFarmPublicLocation {
                primary: "Test Farm".to_string(),
                city: Some("Santa Cruz".to_string()),
                region: Some("California".to_string()),
                country: Some("US".to_string()),
                geohash: "9q8yy".to_string(),
            }),
            tags: None,
        };
        let content = serde_json::to_string(&farm).expect("farm content");
        let tags = vec![
            vec!["d".to_string(), "AAAAAAAAAAAAAAAAAAAAAA".to_string()],
            vec!["g".to_string(), "9q8yy".to_string()],
        ];
        let parsed = parsed_from_event(
            EVENT_ID.to_string(),
            AUTHOR.to_string(),
            42,
            KIND_FARM,
            content.clone(),
            tags,
            EVENT_SIG.to_string(),
        )
        .expect("parsed farm");
        assert_eq!(parsed.event.signature_hex(), EVENT_SIG);
        assert_eq!(parsed.data.data.name, "Test Farm");

        for (tag, expected) in [
            (vec!["g".to_string()], "g"),
            (vec!["g".to_string(), "9q8ya".to_string()], "g"),
            (vec!["dd".to_string(), "secret".to_string()], "dd"),
            (vec!["dd.lat".to_string(), "1".to_string()], "dd.lat"),
            (vec!["dd.lon".to_string(), "1".to_string()], "dd.lon"),
            (vec!["l".to_string(), "private".to_string()], "l"),
            (vec!["L".to_string(), "private".to_string()], "L"),
        ] {
            let tags = vec![
                vec!["d".to_string(), "AAAAAAAAAAAAAAAAAAAAAA".to_string()],
                tag,
            ];
            let err = farm_from_event(KIND_FARM, &tags, &content).unwrap_err();
            assert!(matches!(err, EventParseError::InvalidTag(found) if found == expected));
        }

        let invalid_geohash_content = r#"{"d_tag":"AAAAAAAAAAAAAAAAAAAAAA","name":"Test Farm","location":{"primary":"Test Farm","city":"Santa Cruz","region":"California","country":"US","geohash":"9q8ya"}}"#;
        let err = farm_from_event(
            KIND_FARM,
            &[vec!["d".to_string(), "AAAAAAAAAAAAAAAAAAAAAA".to_string()]],
            invalid_geohash_content,
        )
        .unwrap_err();
        assert!(matches!(err, EventParseError::InvalidTag("g")));

        let missing_locality_content = r#"{"d_tag":"AAAAAAAAAAAAAAAAAAAAAA","name":"Test Farm","location":{"primary":" ","city":"null","region":null,"country":null,"geohash":"9q8yy"}}"#;
        let err = farm_from_event(
            KIND_FARM,
            &[vec!["d".to_string(), "AAAAAAAAAAAAAAAAAAAAAA".to_string()]],
            missing_locality_content,
        )
        .unwrap_err();
        assert!(matches!(err, EventParseError::InvalidTag("g")));

        let err = farm_from_event(
            KIND_FARM,
            &[vec!["d".to_string(), "AAAAAAAAAAAAAAAAAAAAAA".to_string()]],
            "[]",
        )
        .unwrap_err();
        assert!(matches!(err, EventParseError::InvalidJson("content")));

        for key in [
            "workspace",
            "farm_group_id",
            "document_id",
            "document_kind",
            "crdt_backend",
            "encoded_change",
            "semantic_kind",
            "owner_document_kind",
            "owner_document_id",
            "relays",
            "media_servers",
            "supported_kinds",
            "protocol_version",
        ] {
            let content =
                format!(r#"{{"d_tag":"AAAAAAAAAAAAAAAAAAAAAA","name":"Test Farm","{key}":"x"}}"#);
            let err = farm_from_event(
                KIND_FARM,
                &[vec!["d".to_string(), "AAAAAAAAAAAAAAAAAAAAAA".to_string()]],
                &content,
            )
            .unwrap_err();
            assert!(matches!(err, EventParseError::InvalidJson("content")));
        }

        for key in [
            "gcs",
            "lat",
            "lng",
            "lon",
            "point",
            "polygon",
            "coordinates",
            "accuracy",
            "altitude",
            "label",
            "tag_0",
        ] {
            let content = format!(
                r#"{{"d_tag":"AAAAAAAAAAAAAAAAAAAAAA","name":"Test Farm","location":{{"primary":"Test Farm","city":"Santa Cruz","region":"California","country":"US","geohash":"9q8yy","{key}":"x"}}}}"#
            );
            let err = farm_from_event(
                KIND_FARM,
                &[vec!["d".to_string(), "AAAAAAAAAAAAAAAAAAAAAA".to_string()]],
                &content,
            )
            .unwrap_err();
            assert!(matches!(err, EventParseError::InvalidJson("content")));
        }
    }

    #[test]
    fn farm_list_sets_include_expected_tags() {
        let members = farm_members_list_set("AAAAAAAAAAAAAAAAAAAAAA", ["owner_pubkey"])
            .expect("members list");
        assert_eq!(members.d_tag, "farm:AAAAAAAAAAAAAAAAAAAAAA:members");
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
            d_tag: "AAAAAAAAAAAAAAAAAAAAAQ".to_string(),
            farm: RadrootsFarmRef {
                pubkey: "farm_pubkey".to_string(),
                d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
            },
            name: "Plot 1".to_string(),
            about: None,
            location: None,
            tags: None,
        }];

        let plots_list =
            farm_plots_list_set_from_plots("AAAAAAAAAAAAAAAAAAAAAA", "farm_pubkey", &plots)
                .expect("plots list");
        assert_eq!(plots_list.d_tag, "farm:AAAAAAAAAAAAAAAAAAAAAA:plots");
        assert_eq!(plots_list.entries.len(), 1);
        assert_eq!(plots_list.entries[0].tag, "a");
        assert_eq!(
            plots_list.entries[0].values[0],
            "30350:farm_pubkey:AAAAAAAAAAAAAAAAAAAAAQ"
        );
    }

    #[test]
    fn farm_listings_list_set_uses_listing_addresses() {
        let listings = vec![RadrootsOperationalListing {
            d_tag: d_tag("AAAAAAAAAAAAAAAAAAAAAg"),
            published_at: None,
            farm: RadrootsFarmRef {
                pubkey: "farm_pubkey".to_string(),
                d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
            },
            product: RadrootsOperationalListingProduct {
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
            primary_bin_id: bin_id("bin-1"),
            bins: vec![RadrootsOperationalListingBin {
                bin_id: bin_id("bin-1"),
                quantity: Quantity::try_new(Decimal::from(1u32), Unit::Each).unwrap(),
                price_per_canonical_unit: QuantityPrice::try_new(
                    Money::try_new(Decimal::from(10u32), Currency::USD).unwrap(),
                    Quantity::try_new(Decimal::from(1u32), Unit::Each).unwrap(),
                )
                .unwrap(),
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
        }];

        let listings_list = farm_operational_listings_list_set_from_listings(
            "AAAAAAAAAAAAAAAAAAAAAA",
            "farm_pubkey",
            &listings,
        )
        .expect("listings list");
        assert_eq!(listings_list.d_tag, "farm:AAAAAAAAAAAAAAAAAAAAAA:listings");
        assert_eq!(listings_list.entries.len(), 1);
        assert_eq!(listings_list.entries[0].tag, "a");
        assert_eq!(
            listings_list.entries[0].values[0],
            "30402:farm_pubkey:AAAAAAAAAAAAAAAAAAAAAg"
        );
    }
}
