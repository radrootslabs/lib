#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::{
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};

use radroots_events::farm::RadrootsFarmRef;
use radroots_events::kinds::{KIND_FARM, KIND_PLOT};
use radroots_events::list::RadrootsListEntry;
use radroots_events::list_set::RadrootsListSet;
use radroots_events::plot::RadrootsPlotRef;

use crate::d_tag::validate_d_tag;
use crate::error::EventEncodeError;

fn resource_list_set_id(area_id: &str, suffix: &str) -> Result<String, EventEncodeError> {
    let area_id = area_id.trim();
    if area_id.is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("area_id"));
    }
    validate_d_tag(area_id, "area_id")?;
    Ok(format!("resource:{area_id}:{suffix}"))
}

fn list_entries<I, S>(tag: &str, values: I) -> Result<Vec<RadrootsListEntry>, EventEncodeError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut entries = Vec::new();
    for value in values {
        let value = value.as_ref().trim();
        if value.is_empty() {
            return Err(EventEncodeError::EmptyRequiredField("entry.values"));
        }
        entries.push(RadrootsListEntry {
            tag: tag.to_string(),
            values: vec![value.to_string()],
        });
    }
    Ok(entries)
}

fn farm_address(farm: &RadrootsFarmRef) -> Result<String, EventEncodeError> {
    if farm.pubkey.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("farm.pubkey"));
    }
    if farm.d_tag.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("farm.d_tag"));
    }
    validate_d_tag(&farm.d_tag, "farm.d_tag")?;
    let mut addr = String::new();
    addr.push_str(&KIND_FARM.to_string());
    addr.push(':');
    addr.push_str(&farm.pubkey);
    addr.push(':');
    addr.push_str(&farm.d_tag);
    Ok(addr)
}

fn plot_address(plot: &RadrootsPlotRef) -> Result<String, EventEncodeError> {
    if plot.pubkey.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("plot.pubkey"));
    }
    if plot.d_tag.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("plot.d_tag"));
    }
    validate_d_tag(&plot.d_tag, "plot.d_tag")?;
    let mut addr = String::new();
    addr.push_str(&KIND_PLOT.to_string());
    addr.push(':');
    addr.push_str(&plot.pubkey);
    addr.push(':');
    addr.push_str(&plot.d_tag);
    Ok(addr)
}

pub fn resource_area_members_farms_list_set<I>(
    area_id: &str,
    farms: I,
) -> Result<RadrootsListSet, EventEncodeError>
where
    I: IntoIterator<Item = RadrootsFarmRef>,
{
    let mut entries = Vec::new();
    for farm in farms {
        let address = farm_address(&farm)?;
        entries.push(RadrootsListEntry {
            tag: "a".to_string(),
            values: vec![address],
        });
        entries.push(RadrootsListEntry {
            tag: "p".to_string(),
            values: vec![farm.pubkey],
        });
    }
    Ok(RadrootsListSet {
        d_tag: resource_list_set_id(area_id, "members.farms")?,
        content: String::new(),
        entries,
        title: None,
        description: None,
        image: None,
    })
}

pub fn resource_area_members_plots_list_set<I>(
    area_id: &str,
    plots: I,
) -> Result<RadrootsListSet, EventEncodeError>
where
    I: IntoIterator<Item = RadrootsPlotRef>,
{
    let mut entries = Vec::new();
    for plot in plots {
        let address = plot_address(&plot)?;
        entries.push(RadrootsListEntry {
            tag: "a".to_string(),
            values: vec![address],
        });
        entries.push(RadrootsListEntry {
            tag: "p".to_string(),
            values: vec![plot.pubkey],
        });
    }
    Ok(RadrootsListSet {
        d_tag: resource_list_set_id(area_id, "members.plots")?,
        content: String::new(),
        entries,
        title: None,
        description: None,
        image: None,
    })
}

pub fn resource_area_stewards_list_set<I, S>(
    area_id: &str,
    stewards: I,
) -> Result<RadrootsListSet, EventEncodeError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    Ok(RadrootsListSet {
        d_tag: resource_list_set_id(area_id, "members.stewards")?,
        content: String::new(),
        entries: list_entries("p", stewards)?,
        title: None,
        description: None,
        image: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::{FIXTURE_ALICE_PUBLIC_KEY_HEX, FIXTURE_BOB_PUBLIC_KEY_HEX};

    #[test]
    fn resource_list_set_id_validates_area_id() {
        let err =
            resource_list_set_id(" ", "members.farms").expect_err("expected empty area_id error");
        assert!(matches!(
            err,
            EventEncodeError::EmptyRequiredField("area_id")
        ));
    }

    #[test]
    fn list_entries_rejects_empty_values() {
        let err = list_entries("p", [" "]).expect_err("expected empty entry error");
        assert!(matches!(
            err,
            EventEncodeError::EmptyRequiredField("entry.values")
        ));
    }

    #[test]
    fn list_entries_accepts_empty_iterators() {
        let entries = list_entries::<_, &str>("p", core::iter::empty())
            .expect("empty iterators should be accepted");
        assert!(entries.is_empty());
    }

    #[test]
    fn list_entries_cover_string_iterators() {
        let entries = list_entries("p", vec!["steward".to_string()]).expect("valid string entries");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].values[0], "steward");

        let err = list_entries("p", vec![" ".to_string()]).expect_err("blank string entry");
        assert!(matches!(
            err,
            EventEncodeError::EmptyRequiredField("entry.values")
        ));
    }

    #[test]
    fn farm_and_plot_address_helpers_reject_empty_d_tags() {
        let err = farm_address(&RadrootsFarmRef {
            pubkey: " ".to_string(),
            d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
        })
        .expect_err("expected farm pubkey error");
        assert!(matches!(
            err,
            EventEncodeError::EmptyRequiredField("farm.pubkey")
        ));

        let err = farm_address(&RadrootsFarmRef {
            pubkey: "farm_pubkey".to_string(),
            d_tag: " ".to_string(),
        })
        .expect_err("expected farm d_tag error");
        assert!(matches!(
            err,
            EventEncodeError::EmptyRequiredField("farm.d_tag")
        ));

        let err = plot_address(&RadrootsPlotRef {
            pubkey: " ".to_string(),
            d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
        })
        .expect_err("expected plot pubkey error");
        assert!(matches!(
            err,
            EventEncodeError::EmptyRequiredField("plot.pubkey")
        ));

        let err = plot_address(&RadrootsPlotRef {
            pubkey: "plot_pubkey".to_string(),
            d_tag: " ".to_string(),
        })
        .expect_err("expected plot d_tag error");
        assert!(matches!(
            err,
            EventEncodeError::EmptyRequiredField("plot.d_tag")
        ));
    }

    #[test]
    fn resource_area_list_set_builders_cover_success_and_error_paths() {
        let area_id = "AAAAAAAAAAAAAAAAAAAAAA";
        let farm_pubkey = FIXTURE_ALICE_PUBLIC_KEY_HEX;
        let plot_pubkey = FIXTURE_BOB_PUBLIC_KEY_HEX;

        let err = resource_area_members_farms_list_set(
            "invalid",
            vec![RadrootsFarmRef {
                pubkey: farm_pubkey.to_string(),
                d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
            }],
        )
        .expect_err("expected invalid area_id");
        assert!(matches!(err, EventEncodeError::InvalidField("area_id")));

        let farms = resource_area_members_farms_list_set(
            area_id,
            vec![RadrootsFarmRef {
                pubkey: farm_pubkey.to_string(),
                d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
            }],
        )
        .expect("resource area farms list set");
        assert_eq!(farms.d_tag, "resource:AAAAAAAAAAAAAAAAAAAAAA:members.farms");
        assert_eq!(farms.entries.len(), 2);
        assert_eq!(farms.entries[0].tag, "a");
        assert_eq!(farms.entries[1].tag, "p");

        let err = resource_area_members_farms_list_set(
            area_id,
            vec![RadrootsFarmRef {
                pubkey: farm_pubkey.to_string(),
                d_tag: "invalid".to_string(),
            }],
        )
        .expect_err("expected invalid farm d_tag");
        assert!(matches!(err, EventEncodeError::InvalidField("farm.d_tag")));

        let plots = resource_area_members_plots_list_set(
            area_id,
            vec![RadrootsPlotRef {
                pubkey: plot_pubkey.to_string(),
                d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
            }],
        )
        .expect("resource area plots list set");
        assert_eq!(plots.d_tag, "resource:AAAAAAAAAAAAAAAAAAAAAA:members.plots");
        assert_eq!(plots.entries.len(), 2);
        assert_eq!(plots.entries[0].tag, "a");
        assert_eq!(plots.entries[1].tag, "p");

        let err = resource_area_members_plots_list_set(
            "invalid",
            vec![RadrootsPlotRef {
                pubkey: plot_pubkey.to_string(),
                d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
            }],
        )
        .expect_err("expected invalid area_id for plots list set");
        assert!(matches!(err, EventEncodeError::InvalidField("area_id")));

        let err = resource_area_members_plots_list_set(
            area_id,
            vec![RadrootsPlotRef {
                pubkey: plot_pubkey.to_string(),
                d_tag: "invalid".to_string(),
            }],
        )
        .expect_err("expected invalid plot d_tag");
        assert!(matches!(err, EventEncodeError::InvalidField("plot.d_tag")));

        let stewards =
            resource_area_stewards_list_set(area_id, ["steward-a"]).expect("stewards list set");
        assert_eq!(
            stewards.d_tag,
            "resource:AAAAAAAAAAAAAAAAAAAAAA:members.stewards"
        );
        assert_eq!(stewards.entries[0].tag, "p");
        let err = resource_area_stewards_list_set(area_id, [" "])
            .expect_err("expected invalid steward entry");
        assert!(matches!(
            err,
            EventEncodeError::EmptyRequiredField("entry.values")
        ));
    }
}
