#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::{format, string::{String, ToString}, vec, vec::Vec};

use radroots_events::list::RadrootsListEntry;
use radroots_events::list_set::RadrootsListSet;
use radroots_events::plot::RadrootsPlot;

use crate::error::EventEncodeError;
use crate::plot::encode::plot_address;

const MEMBER_OF_FARMS: &str = "member_of.farms";

fn farm_list_set_id(farm_id: &str, suffix: &str) -> Result<String, EventEncodeError> {
    let farm_id = farm_id.trim();
    if farm_id.is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("farm_id"));
    }
    if suffix.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("list_set_suffix"));
    }
    Ok(format!("farm:{farm_id}:{suffix}"))
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

pub fn farm_members_list_set<I, S>(
    farm_id: &str,
    members: I,
) -> Result<RadrootsListSet, EventEncodeError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    Ok(RadrootsListSet {
        d_tag: farm_list_set_id(farm_id, "members")?,
        content: String::new(),
        entries: list_entries("p", members)?,
        title: None,
        description: None,
        image: None,
    })
}

pub fn farm_owners_list_set<I, S>(
    farm_id: &str,
    owners: I,
) -> Result<RadrootsListSet, EventEncodeError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    Ok(RadrootsListSet {
        d_tag: farm_list_set_id(farm_id, "members.owners")?,
        content: String::new(),
        entries: list_entries("p", owners)?,
        title: None,
        description: None,
        image: None,
    })
}

pub fn farm_workers_list_set<I, S>(
    farm_id: &str,
    workers: I,
) -> Result<RadrootsListSet, EventEncodeError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    Ok(RadrootsListSet {
        d_tag: farm_list_set_id(farm_id, "members.workers")?,
        content: String::new(),
        entries: list_entries("p", workers)?,
        title: None,
        description: None,
        image: None,
    })
}

pub fn farm_plots_list_set<I, S>(
    farm_id: &str,
    farm_pubkey: &str,
    plot_ids: I,
) -> Result<RadrootsListSet, EventEncodeError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut entries = Vec::new();
    for plot_id in plot_ids {
        let plot_id = plot_id.as_ref();
        let address = plot_address(farm_pubkey, plot_id)?;
        entries.push(RadrootsListEntry {
            tag: "a".to_string(),
            values: vec![address],
        });
    }
    Ok(RadrootsListSet {
        d_tag: farm_list_set_id(farm_id, "plots")?,
        content: String::new(),
        entries,
        title: None,
        description: None,
        image: None,
    })
}

pub fn farm_plots_list_set_from_plots<'a, I>(
    farm_id: &str,
    farm_pubkey: &str,
    plots: I,
) -> Result<RadrootsListSet, EventEncodeError>
where
    I: IntoIterator<Item = &'a RadrootsPlot>,
{
    farm_plots_list_set(
        farm_id,
        farm_pubkey,
        plots.into_iter().map(|plot| plot.d_tag.as_str()),
    )
}

pub fn member_of_farms_list_set<I, S>(
    farm_pubkeys: I,
) -> Result<RadrootsListSet, EventEncodeError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    Ok(RadrootsListSet {
        d_tag: MEMBER_OF_FARMS.to_string(),
        content: String::new(),
        entries: list_entries("p", farm_pubkeys)?,
        title: None,
        description: None,
        image: None,
    })
}
