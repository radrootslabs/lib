#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::{
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};

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

pub fn member_of_coops_list_set<I, S>(coop_pubkeys: I) -> Result<RadrootsListSet, EventEncodeError>
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

#[cfg(test)]
mod tests {
    use super::*;
    use radroots_test_fixtures::FIXTURE_ALICE_PUBLIC_KEY_HEX;

    #[test]
    fn coop_list_set_id_validates_coop_id() {
        let err = coop_list_set_id(" ", "members").expect_err("expected coop_id validation error");
        assert!(matches!(
            err,
            EventEncodeError::EmptyRequiredField("coop_id")
        ));
    }

    #[test]
    fn list_entries_rejects_blank_values() {
        let err = list_entries("p", [" "]).expect_err("expected blank entry error");
        assert!(matches!(
            err,
            EventEncodeError::EmptyRequiredField("entry.values")
        ));
    }

    #[test]
    fn list_entries_cover_string_iterators() {
        let entries = list_entries("p", vec!["member".to_string()]).expect("valid string entries");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].values[0], "member");

        let err = list_entries("p", vec![" ".to_string()]).expect_err("blank string entry");
        assert!(matches!(
            err,
            EventEncodeError::EmptyRequiredField("entry.values")
        ));
    }

    #[test]
    fn list_entries_accepts_empty_iterators() {
        let entries = list_entries("p", Vec::<&str>::new()).expect("empty list entries");
        assert!(entries.is_empty());

        let entries = list_entries("p", vec!["member"]).expect("non-empty vec<&str> entries");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].values[0], "member");

        let err = list_entries("p", vec![" "]).expect_err("blank vec<&str> entry");
        assert!(matches!(
            err,
            EventEncodeError::EmptyRequiredField("entry.values")
        ));
    }

    #[test]
    fn farm_address_rejects_empty_and_invalid_d_tag() {
        let err = farm_address(&RadrootsFarmRef {
            pubkey: " ".to_string(),
            d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
        })
        .expect_err("expected empty pubkey error");
        assert!(matches!(
            err,
            EventEncodeError::EmptyRequiredField("farm.pubkey")
        ));

        let err = farm_address(&RadrootsFarmRef {
            pubkey: FIXTURE_ALICE_PUBLIC_KEY_HEX.to_string(),
            d_tag: " ".to_string(),
        })
        .expect_err("expected empty d_tag error");
        assert!(matches!(
            err,
            EventEncodeError::EmptyRequiredField("farm.d_tag")
        ));

        let err = farm_address(&RadrootsFarmRef {
            pubkey: FIXTURE_ALICE_PUBLIC_KEY_HEX.to_string(),
            d_tag: "invalid".to_string(),
        })
        .expect_err("expected invalid d_tag error");
        assert!(matches!(err, EventEncodeError::InvalidField("farm.d_tag")));
    }

    #[test]
    fn coop_list_set_builders_cover_success_and_error_paths() {
        let coop_id = "AAAAAAAAAAAAAAAAAAAAAA";

        let members = coop_members_list_set(coop_id, ["member-a"]).expect("members list set");
        assert_eq!(members.d_tag, "coop:AAAAAAAAAAAAAAAAAAAAAA:members");
        assert_eq!(members.entries.len(), 1);
        assert_eq!(members.entries[0].tag, "p");

        let err =
            coop_members_list_set("invalid", ["member-a"]).expect_err("expected invalid coop_id");
        assert!(matches!(err, EventEncodeError::InvalidField("coop_id")));

        let err =
            coop_members_list_set(coop_id, [" "]).expect_err("expected invalid members entry");
        assert!(matches!(
            err,
            EventEncodeError::EmptyRequiredField("entry.values")
        ));

        let owners = coop_owners_list_set(coop_id, ["owner-a"]).expect("owners list set");
        assert_eq!(owners.d_tag, "coop:AAAAAAAAAAAAAAAAAAAAAA:members.owners");
        assert_eq!(owners.entries[0].tag, "p");
        let err = coop_owners_list_set("invalid", ["owner-a"]).expect_err("invalid coop_id");
        assert!(matches!(err, EventEncodeError::InvalidField("coop_id")));
        let err = coop_owners_list_set(coop_id, [" "]).expect_err("expected invalid owner entry");
        assert!(matches!(
            err,
            EventEncodeError::EmptyRequiredField("entry.values")
        ));

        let admins = coop_admins_list_set(coop_id, ["admin-a"]).expect("admins list set");
        assert_eq!(admins.d_tag, "coop:AAAAAAAAAAAAAAAAAAAAAA:members.admins");
        assert_eq!(admins.entries[0].tag, "p");
        let err = coop_admins_list_set("invalid", ["admin-a"]).expect_err("invalid coop_id");
        assert!(matches!(err, EventEncodeError::InvalidField("coop_id")));
        let err = coop_admins_list_set(coop_id, [" "]).expect_err("expected invalid admin entry");
        assert!(matches!(
            err,
            EventEncodeError::EmptyRequiredField("entry.values")
        ));

        let items = coop_items_list_set(coop_id, ["30317:author:AAAAAAAAAAAAAAAAAAAAAA"])
            .expect("items list set");
        assert_eq!(items.d_tag, "coop:AAAAAAAAAAAAAAAAAAAAAA:items");
        assert_eq!(items.entries[0].tag, "a");
        let err = coop_items_list_set("invalid", ["30317:author:AAAAAAAAAAAAAAAAAAAAAA"])
            .expect_err("invalid coop_id");
        assert!(matches!(err, EventEncodeError::InvalidField("coop_id")));
        let err = coop_items_list_set(coop_id, [" "]).expect_err("expected invalid item entry");
        assert!(matches!(
            err,
            EventEncodeError::EmptyRequiredField("entry.values")
        ));

        let member_of = member_of_coops_list_set(["coop-pubkey"]).expect("member_of list set");
        assert_eq!(member_of.d_tag, "member_of.coops");
        assert_eq!(member_of.entries[0].tag, "p");
        let err =
            member_of_coops_list_set([" "]).expect_err("expected invalid member_of coop entry");
        assert!(matches!(
            err,
            EventEncodeError::EmptyRequiredField("entry.values")
        ));
    }

    #[test]
    fn coop_members_farms_list_set_covers_success_and_invalid_coop_id() {
        let farms = vec![RadrootsFarmRef {
            pubkey: FIXTURE_ALICE_PUBLIC_KEY_HEX.to_string(),
            d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
        }];
        let list_set = coop_members_farms_list_set("AAAAAAAAAAAAAAAAAAAAAA", farms.clone())
            .expect("members farms list set");
        assert_eq!(list_set.d_tag, "coop:AAAAAAAAAAAAAAAAAAAAAA:members.farms");
        assert_eq!(list_set.entries.len(), 2);
        assert_eq!(list_set.entries[0].tag, "a");
        assert_eq!(list_set.entries[1].tag, "p");

        let err = coop_members_farms_list_set("invalid", farms)
            .expect_err("expected invalid coop_id in members farms list set");
        assert!(matches!(err, EventEncodeError::InvalidField("coop_id")));
    }
}
