pub mod decode;
pub mod encode;

#[cfg(test)]
mod tests {
    use radroots_events::{
        farm::{RadrootsFarmRef, RadrootsGcsLocation, RadrootsGeoJsonPoint, RadrootsGeoJsonPolygon},
        plot::{RadrootsPlot, RadrootsPlotLocation},
    };
    use crate::plot::encode::plot_build_tags;

    #[test]
    fn plot_tags_include_farm_address() {
        let plot = RadrootsPlot {
            d_tag: "AAAAAAAAAAAAAAAAAAAAAQ".to_string(),
            farm: RadrootsFarmRef {
                pubkey: "farm_pubkey".to_string(),
                d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
            },
            name: "Orchard".to_string(),
            about: None,
            location: Some(RadrootsPlotLocation {
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

        let tags = plot_build_tags(&plot).expect("tags");
        let has_a = tags.iter().any(|tag| tag.get(0).map(|v| v.as_str()) == Some("a"));
        let has_p = tags.iter().any(|tag| tag.get(0).map(|v| v.as_str()) == Some("p"));
        assert!(has_a);
        assert!(has_p);
    }
}
