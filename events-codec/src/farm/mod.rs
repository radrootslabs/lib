pub mod decode;
pub mod encode;
pub mod list_sets;

#[cfg(test)]
mod tests {
    use radroots_events::farm::{RadrootsFarm, RadrootsFarmLocation, RadrootsFarmRef};
    use radroots_events::plot::RadrootsPlot;
    use crate::farm::encode::{farm_build_tags, farm_ref_tags};
    use crate::farm::list_sets::{
        farm_members_list_set,
        farm_plots_list_set_from_plots,
        member_of_farms_list_set,
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
            geometry: None,
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
}
