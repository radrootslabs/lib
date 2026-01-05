#![forbid(unsafe_code)]

pub mod decode;
pub mod encode;
pub mod list_sets;

#[cfg(test)]
mod tests {
    use radroots_events::coop::{RadrootsCoop, RadrootsCoopLocation, RadrootsCoopRef};
    use radroots_events::farm::{RadrootsFarmRef, RadrootsGcsLocation, RadrootsGeoJsonPoint, RadrootsGeoJsonPolygon};
    use crate::coop::encode::{coop_build_tags, coop_ref_tags};
    use crate::coop::list_sets::{
        coop_items_list_set,
        coop_members_farms_list_set,
        coop_members_list_set,
        member_of_coops_list_set,
    };

    #[test]
    fn coop_tags_include_required_fields() {
        let coop = RadrootsCoop {
            d_tag: "BAAAAAAAAAAAAAAAAAAAAA".to_string(),
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
            tags: Some(vec!["regional".to_string()]),
        };

        let tags = coop_build_tags(&coop).expect("tags");
        assert!(tags.iter().any(|tag| tag.get(0) == Some(&"d".to_string())));
        assert!(tags.iter().any(|tag| tag.get(0) == Some(&"t".to_string())));
        assert!(tags.iter().any(|tag| tag.get(0) == Some(&"g".to_string())));
    }

    #[test]
    fn coop_ref_tags_include_p_and_a() {
        let coop = RadrootsCoopRef {
            pubkey: "coop_pubkey".to_string(),
            d_tag: "BAAAAAAAAAAAAAAAAAAAAA".to_string(),
        };

        let tags = coop_ref_tags(&coop).expect("coop ref tags");
        let has_a = tags.iter().any(|tag| tag.get(0).map(|v| v.as_str()) == Some("a"));
        let has_p = tags.iter().any(|tag| tag.get(0).map(|v| v.as_str()) == Some("p"));
        assert!(has_a);
        assert!(has_p);
    }

    #[test]
    fn coop_list_sets_include_expected_tags() {
        let members = coop_members_list_set("BAAAAAAAAAAAAAAAAAAAAA", ["member_pubkey"]).expect("members list");
        assert_eq!(members.d_tag, "coop:BAAAAAAAAAAAAAAAAAAAAA:members");
        assert_eq!(members.entries.len(), 1);
        assert_eq!(members.entries[0].tag, "p");

        let farm_members = coop_members_farms_list_set(
            "BAAAAAAAAAAAAAAAAAAAAA",
            [RadrootsFarmRef {
                pubkey: "farm_pubkey".to_string(),
                d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
            }],
        )
        .expect("farm members list");
        assert_eq!(farm_members.d_tag, "coop:BAAAAAAAAAAAAAAAAAAAAA:members.farms");
        assert!(farm_members.entries.iter().any(|entry| entry.tag == "a"));
        assert!(farm_members.entries.iter().any(|entry| entry.tag == "p"));

        let items = coop_items_list_set("BAAAAAAAAAAAAAAAAAAAAA", ["30361:coop_pubkey:FAAAAAAAAAAAAAAAAAAAAA"])
            .expect("items list");
        assert_eq!(items.d_tag, "coop:BAAAAAAAAAAAAAAAAAAAAA:items");
        assert_eq!(items.entries.len(), 1);
        assert_eq!(items.entries[0].tag, "a");

        let claims = member_of_coops_list_set(["coop_pubkey"]).expect("claims list");
        assert_eq!(claims.d_tag, "member_of.coops");
        assert_eq!(claims.entries.len(), 1);
        assert_eq!(claims.entries[0].tag, "p");
    }
}
