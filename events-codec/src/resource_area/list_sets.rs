#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::{format, string::{String, ToString}, vec, vec::Vec};

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
    if suffix.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("list_set_suffix"));
    }
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
