pub mod decode;
pub mod encode;

#[cfg(test)]
mod tests {
    use radroots_events::farm::{RadrootsFarm, RadrootsFarmLocation, RadrootsFarmRef};
    use crate::farm::encode::{farm_build_tags, farm_ref_tags};

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
                primary: "Somewhere".to_string(),
                city: None,
                region: None,
                country: None,
                lat: None,
                lng: None,
                geohash: Some("9q8yy".to_string()),
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
}
