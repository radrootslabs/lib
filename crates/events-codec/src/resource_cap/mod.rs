#![forbid(unsafe_code)]

pub mod decode;
pub mod encode;

#[cfg(test)]
mod tests {
    use crate::error::EventEncodeError;
    use crate::resource_cap::encode::resource_harvest_cap_build_tags;
    use radroots_core::{RadrootsCoreDecimal, RadrootsCoreQuantity, RadrootsCoreUnit};
    use radroots_events::resource_area::RadrootsResourceAreaRef;
    use radroots_events::resource_cap::{
        RadrootsResourceHarvestCap, RadrootsResourceHarvestProduct,
    };

    #[test]
    fn resource_harvest_cap_tags_include_required_fields() {
        let cap = RadrootsResourceHarvestCap {
            d_tag: "DAAAAAAAAAAAAAAAAAAAAA".to_string(),
            resource_area: RadrootsResourceAreaRef {
                pubkey: "area_pubkey".to_string(),
                d_tag: "AAAAAAAAAAAAAAAAAAAAAw".to_string(),
            },
            product: RadrootsResourceHarvestProduct {
                key: "nutmeg".to_string(),
                category: Some("spice".to_string()),
            },
            start: 1735689600,
            end: 1767225600,
            cap_quantity: RadrootsCoreQuantity::new(
                RadrootsCoreDecimal::from(100000u32),
                RadrootsCoreUnit::MassG,
            ),
            display_amount: None,
            display_unit: None,
            display_label: None,
            tags: Some(vec!["community".to_string()]),
        };

        let tags = resource_harvest_cap_build_tags(&cap).expect("tags");
        assert!(
            tags.iter()
                .any(|tag| tag.get(0).map(|v| v.as_str()) == Some("d"))
        );
        assert!(
            tags.iter()
                .any(|tag| tag.get(0).map(|v| v.as_str()) == Some("a"))
        );
        assert!(
            tags.iter()
                .any(|tag| tag.get(0).map(|v| v.as_str()) == Some("key"))
        );
        assert!(
            tags.iter()
                .any(|tag| tag.get(0).map(|v| v.as_str()) == Some("start"))
        );
        assert!(
            tags.iter()
                .any(|tag| tag.get(0).map(|v| v.as_str()) == Some("end"))
        );
    }

    #[test]
    fn resource_harvest_cap_build_tags_rejects_invalid_d_tags() {
        let mut cap = RadrootsResourceHarvestCap {
            d_tag: "DAAAAAAAAAAAAAAAAAAAAA".to_string(),
            resource_area: RadrootsResourceAreaRef {
                pubkey: "area_pubkey".to_string(),
                d_tag: "AAAAAAAAAAAAAAAAAAAAAw".to_string(),
            },
            product: RadrootsResourceHarvestProduct {
                key: "nutmeg".to_string(),
                category: Some("spice".to_string()),
            },
            start: 1735689600,
            end: 1767225600,
            cap_quantity: RadrootsCoreQuantity::new(
                RadrootsCoreDecimal::from(100000u32),
                RadrootsCoreUnit::MassG,
            ),
            display_amount: None,
            display_unit: None,
            display_label: None,
            tags: None,
        };

        cap.resource_area.d_tag = "invalid".to_string();
        let err = resource_harvest_cap_build_tags(&cap).expect_err("invalid resource area d_tag");
        assert!(matches!(
            err,
            EventEncodeError::InvalidField("resource_area.d_tag")
        ));

        cap.resource_area.d_tag = "AAAAAAAAAAAAAAAAAAAAAw".to_string();
        cap.d_tag = "invalid".to_string();
        let err = resource_harvest_cap_build_tags(&cap).expect_err("invalid cap d_tag");
        assert!(matches!(err, EventEncodeError::InvalidField("d_tag")));
    }
}
