#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::{format, string::{String, ToString}, vec, vec::Vec};

use radroots_events::farm::RadrootsFarmRef;
use radroots_events::kinds::KIND_FARM;
use radroots_events::list::RadrootsListEntry;
use radroots_events::list_set::RadrootsListSet;

use crate::d_tag::validate_d_tag;
use crate::error::EventEncodeError;

const MEMBER_OF_COOPS: &str = "member_of.coops";

fn coop_list_set_id(coop_id: &str, suffix: &str) -> Result<String, EventEncodeError> {
    let coop_id = coop_id.trim();
    if coop_id.is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("coop_id"));
    }
    validate_d_tag(coop_id, "coop_id")?;
    if suffix.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("list_set_suffix"));
    }
    Ok(format!("coop:{coop_id}:{suffix}"))
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

pub fn coop_members_list_set<I, S>(
    coop_id: &str,
    members: I,
) -> Result<RadrootsListSet, EventEncodeError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    Ok(RadrootsListSet {
        d_tag: coop_list_set_id(coop_id, "members")?,
        content: String::new(),
        entries: list_entries("p", members)?,
        title: None,
        description: None,
        image: None,
    })
}

pub fn coop_members_farms_list_set<I>(
    coop_id: &str,
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
        d_tag: coop_list_set_id(coop_id, "members.farms")?,
        content: String::new(),
        entries,
        title: None,
        description: None,
        image: None,
    })
}

pub fn coop_owners_list_set<I, S>(
    coop_id: &str,
    owners: I,
) -> Result<RadrootsListSet, EventEncodeError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    Ok(RadrootsListSet {
        d_tag: coop_list_set_id(coop_id, "members.owners")?,
        content: String::new(),
        entries: list_entries("p", owners)?,
        title: None,
        description: None,
        image: None,
    })
}

pub fn coop_admins_list_set<I, S>(
    coop_id: &str,
    admins: I,
) -> Result<RadrootsListSet, EventEncodeError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    Ok(RadrootsListSet {
        d_tag: coop_list_set_id(coop_id, "members.admins")?,
        content: String::new(),
        entries: list_entries("p", admins)?,
        title: None,
        description: None,
        image: None,
    })
}

pub fn coop_items_list_set<I, S>(
    coop_id: &str,
    item_addresses: I,
) -> Result<RadrootsListSet, EventEncodeError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    Ok(RadrootsListSet {
        d_tag: coop_list_set_id(coop_id, "items")?,
        content: String::new(),
        entries: list_entries("a", item_addresses)?,
        title: None,
        description: None,
        image: None,
    })
}

pub fn member_of_coops_list_set<I, S>(
    coop_pubkeys: I,
) -> Result<RadrootsListSet, EventEncodeError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    Ok(RadrootsListSet {
        d_tag: MEMBER_OF_COOPS.to_string(),
        content: String::new(),
        entries: list_entries("p", coop_pubkeys)?,
        title: None,
        description: None,
        image: None,
    })
}
