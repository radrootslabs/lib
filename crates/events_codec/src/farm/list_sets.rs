#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::{
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};

use radroots_events::kinds::KIND_LISTING;
use radroots_events::list::RadrootsListEntry;
use radroots_events::list_set::RadrootsListSet;
use radroots_events::listing::RadrootsListing;
use radroots_events::plot::RadrootsPlot;

use crate::d_tag::validate_d_tag;
use crate::error::EventEncodeError;
use crate::plot::encode::plot_address;

const MEMBER_OF_FARMS: &str = "member_of.farms";

fn farm_list_set_id(farm_id: &str, suffix: &str) -> Result<String, EventEncodeError> {
    let farm_id = farm_id.trim();
    if farm_id.is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("farm_id"));
    }
    validate_d_tag(farm_id, "farm_id")?;
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

pub fn farm_listings_list_set<I, S>(
    farm_id: &str,
    farm_pubkey: &str,
    listing_ids: I,
) -> Result<RadrootsListSet, EventEncodeError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut entries = Vec::new();
    for listing_id in listing_ids {
        let listing_id = listing_id.as_ref().trim();
        if listing_id.is_empty() {
            return Err(EventEncodeError::EmptyRequiredField("listing_id"));
        }
        validate_d_tag(listing_id, "listing_id")?;
        let mut address = String::new();
        address.push_str(&KIND_LISTING.to_string());
        address.push(':');
        address.push_str(farm_pubkey);
        address.push(':');
        address.push_str(listing_id);
        entries.push(RadrootsListEntry {
            tag: "a".to_string(),
            values: vec![address],
        });
    }
    Ok(RadrootsListSet {
        d_tag: farm_list_set_id(farm_id, "listings")?,
        content: String::new(),
        entries,
        title: None,
        description: None,
        image: None,
    })
}

pub fn farm_listings_list_set_from_listings<'a, I>(
    farm_id: &str,
    farm_pubkey: &str,
    listings: I,
) -> Result<RadrootsListSet, EventEncodeError>
where
    I: IntoIterator<Item = &'a RadrootsListing>,
{
    farm_listings_list_set(
        farm_id,
        farm_pubkey,
        listings.into_iter().map(|listing| listing.d_tag.as_str()),
    )
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

pub fn member_of_farms_list_set<I, S>(farm_pubkeys: I) -> Result<RadrootsListSet, EventEncodeError>
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::FIXTURE_ALICE_PUBLIC_KEY_HEX;

    #[test]
    fn farm_list_set_id_validates_farm_id() {
        let err = farm_list_set_id(" ", "members").expect_err("expected farm_id error");
        assert!(matches!(
            err,
            EventEncodeError::EmptyRequiredField("farm_id")
        ));
    }

    #[test]
    fn farm_list_set_builders_cover_success_and_error_paths() {
        let farm_id = "AAAAAAAAAAAAAAAAAAAAAA";
        let farm_pubkey = FIXTURE_ALICE_PUBLIC_KEY_HEX;

        let err = farm_members_list_set("invalid", ["member-a"]).expect_err("invalid farm id");
        assert!(matches!(err, EventEncodeError::InvalidField("farm_id")));

        let owners = farm_owners_list_set(farm_id, ["owner-a"]).expect("owners list set");
        assert_eq!(owners.d_tag, "farm:AAAAAAAAAAAAAAAAAAAAAA:members.owners");
        assert_eq!(owners.entries[0].tag, "p");
        let err = farm_owners_list_set("invalid", ["owner-a"]).expect_err("invalid farm id");
        assert!(matches!(err, EventEncodeError::InvalidField("farm_id")));
        let err = farm_owners_list_set(farm_id, [" "]).expect_err("invalid owner entry");
        assert!(matches!(
            err,
            EventEncodeError::EmptyRequiredField("entry.values")
        ));

        let workers = farm_workers_list_set(farm_id, ["worker-a"]).expect("workers list set");
        assert_eq!(workers.d_tag, "farm:AAAAAAAAAAAAAAAAAAAAAA:members.workers");
        assert_eq!(workers.entries[0].tag, "p");
        let err = farm_workers_list_set("invalid", ["worker-a"]).expect_err("invalid farm id");
        assert!(matches!(err, EventEncodeError::InvalidField("farm_id")));
        let err = farm_workers_list_set(farm_id, [" "]).expect_err("invalid worker entry");
        assert!(matches!(
            err,
            EventEncodeError::EmptyRequiredField("entry.values")
        ));

        let plots =
            farm_plots_list_set(farm_id, farm_pubkey, ["AAAAAAAAAAAAAAAAAAAAAA"]).expect("plots");
        assert_eq!(plots.d_tag, "farm:AAAAAAAAAAAAAAAAAAAAAA:plots");
        assert_eq!(plots.entries[0].tag, "a");
        let err = farm_plots_list_set("invalid", farm_pubkey, ["AAAAAAAAAAAAAAAAAAAAAA"])
            .expect_err("invalid farm id");
        assert!(matches!(err, EventEncodeError::InvalidField("farm_id")));
        let err =
            farm_plots_list_set(farm_id, farm_pubkey, ["invalid"]).expect_err("invalid plot_id");
        assert!(matches!(err, EventEncodeError::InvalidField("plot.d_tag")));

        let listings = farm_listings_list_set(farm_id, farm_pubkey, ["AAAAAAAAAAAAAAAAAAAAAA"])
            .expect("listings");
        assert_eq!(listings.d_tag, "farm:AAAAAAAAAAAAAAAAAAAAAA:listings");
        assert_eq!(listings.entries[0].tag, "a");
        let err = farm_listings_list_set("invalid", farm_pubkey, ["AAAAAAAAAAAAAAAAAAAAAA"])
            .expect_err("invalid farm id");
        assert!(matches!(err, EventEncodeError::InvalidField("farm_id")));
        let err = farm_listings_list_set(farm_id, farm_pubkey, ["invalid"])
            .expect_err("invalid listing_id");
        assert!(matches!(err, EventEncodeError::InvalidField("listing_id")));

        let err =
            farm_listings_list_set(farm_id, farm_pubkey, [" "]).expect_err("empty listing_id");
        assert!(matches!(
            err,
            EventEncodeError::EmptyRequiredField("listing_id")
        ));

        let member_of = member_of_farms_list_set(["farm-pubkey"]).expect("member_of farms");
        assert_eq!(member_of.d_tag, "member_of.farms");
        assert_eq!(member_of.entries[0].tag, "p");
        let err = member_of_farms_list_set([" "]).expect_err("invalid member_of entry");
        assert!(matches!(
            err,
            EventEncodeError::EmptyRequiredField("entry.values")
        ));
    }
}
