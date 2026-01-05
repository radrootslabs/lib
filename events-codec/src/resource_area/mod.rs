#![forbid(unsafe_code)]

pub mod encode;
pub mod decode;
pub mod list_sets;

#[cfg(test)]
mod tests {
    use radroots_events::farm::{RadrootsGcsLocation, RadrootsGeoJsonPoint, RadrootsGeoJsonPolygon};
    use radroots_events::resource_area::{RadrootsResourceArea, RadrootsResourceAreaLocation, RadrootsResourceAreaRef};
    use crate::resource_area::encode::{resource_area_build_tags, resource_area_ref_tags};
    use crate::resource_area::list_sets::{
        resource_area_members_farms_list_set,
        resource_area_members_plots_list_set,
        resource_area_stewards_list_set,
    };
    use radroots_events::farm::RadrootsFarmRef;
    use radroots_events::plot::RadrootsPlotRef;

    fn sample_location() -> RadrootsResourceAreaLocation {
        RadrootsResourceAreaLocation {
            primary: None,
            city: None,
            region: None,
            country: None,
            gcs: RadrootsGcsLocation {
                lat: -4.527,
                lng: 129.898,
                geohash: "pmb5v".to_string(),
                point: RadrootsGeoJsonPoint {
                    r#type: "Point".to_string(),
                    coordinates: [129.898, -4.527],
                },
                polygon: RadrootsGeoJsonPolygon {
                    r#type: "Polygon".to_string(),
                    coordinates: vec![vec![
                        [129.898, -4.527],
                        [129.899, -4.527],
                        [129.899, -4.528],
                        [129.898, -4.527],
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
        }
    }

    #[test]
    fn resource_area_tags_include_required_fields() {
        let area = RadrootsResourceArea {
            d_tag: "AAAAAAAAAAAAAAAAAAAAAw".to_string(),
            name: "Banda Grove".to_string(),
            about: None,
            location: sample_location(),
            tags: Some(vec!["nutmeg".to_string()]),
        };

        let tags = resource_area_build_tags(&area).expect("tags");
        assert!(tags.iter().any(|tag| tag.get(0).map(|v| v.as_str()) == Some("d")));
        assert!(tags.iter().any(|tag| tag.get(0).map(|v| v.as_str()) == Some("g")));
        assert!(tags.iter().any(|tag| tag.get(0).map(|v| v.as_str()) == Some("t")));
    }

    #[test]
    fn resource_area_ref_tags_include_p_and_a() {
        let area_ref = RadrootsResourceAreaRef {
            pubkey: "area_pubkey".to_string(),
            d_tag: "AAAAAAAAAAAAAAAAAAAAAw".to_string(),
        };

        let tags = resource_area_ref_tags(&area_ref).expect("ref tags");
        assert!(tags.iter().any(|tag| tag.get(0).map(|v| v.as_str()) == Some("p")));
        assert!(tags.iter().any(|tag| tag.get(0).map(|v| v.as_str()) == Some("a")));
    }

    #[test]
    fn resource_area_list_sets_include_expected_tags() {
        let farms = resource_area_members_farms_list_set(
            "AAAAAAAAAAAAAAAAAAAAAw",
            [RadrootsFarmRef {
                pubkey: "farm_pubkey".to_string(),
                d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
            }],
        )
        .expect("farm members");
        assert_eq!(farms.d_tag, "resource:AAAAAAAAAAAAAAAAAAAAAw:members.farms");
        assert!(farms.entries.iter().any(|entry| entry.tag == "a"));
        assert!(farms.entries.iter().any(|entry| entry.tag == "p"));

        let plots = resource_area_members_plots_list_set(
            "AAAAAAAAAAAAAAAAAAAAAw",
            [RadrootsPlotRef {
                pubkey: "farm_pubkey".to_string(),
                d_tag: "AAAAAAAAAAAAAAAAAAAAAQ".to_string(),
            }],
        )
        .expect("plot members");
        assert_eq!(plots.d_tag, "resource:AAAAAAAAAAAAAAAAAAAAAw:members.plots");
        assert!(plots.entries.iter().any(|entry| entry.tag == "a"));
        assert!(plots.entries.iter().any(|entry| entry.tag == "p"));

        let stewards = resource_area_stewards_list_set("AAAAAAAAAAAAAAAAAAAAAw", ["steward_pubkey"])
            .expect("stewards");
        assert_eq!(stewards.d_tag, "resource:AAAAAAAAAAAAAAAAAAAAAw:members.stewards");
        assert!(stewards.entries.iter().any(|entry| entry.tag == "p"));
    }
}
