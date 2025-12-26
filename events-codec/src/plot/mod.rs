pub mod decode;
pub mod encode;

#[cfg(test)]
mod tests {
    use radroots_events::{farm::RadrootsFarmRef, plot::{RadrootsPlot, RadrootsPlotLocation}};
    use crate::plot::encode::plot_build_tags;

    #[test]
    fn plot_tags_include_farm_address() {
        let plot = RadrootsPlot {
            d_tag: "plot-1".to_string(),
            farm: RadrootsFarmRef {
                pubkey: "farm_pubkey".to_string(),
                d_tag: "farm-1".to_string(),
            },
            name: "Orchard".to_string(),
            about: None,
            location: Some(RadrootsPlotLocation {
                primary: "Somewhere".to_string(),
                city: None,
                region: None,
                country: None,
                lat: None,
                lng: None,
                geohash: None,
            }),
            geometry: None,
            tags: Some(vec!["orchard".to_string()]),
        };

        let tags = plot_build_tags(&plot).expect("tags");
        let has_a = tags.iter().any(|tag| tag.get(0).map(|v| v.as_str()) == Some("a"));
        let has_p = tags.iter().any(|tag| tag.get(0).map(|v| v.as_str()) == Some("p"));
        assert!(has_a);
        assert!(has_p);
    }
}
