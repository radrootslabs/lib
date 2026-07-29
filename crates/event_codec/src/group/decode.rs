#[cfg(not(feature = "std"))]
use alloc::{
    string::{String, ToString},
    vec::Vec,
};

use radroots_event::{
    social::group::{
        GroupAdmins, GroupCreateGroup, GroupCreateInvite, GroupDeleteEvent, GroupDeleteGroup,
        GroupEditMetadata, GroupEditableMetadata, GroupJoinRequest, GroupLeaveRequest,
        GroupMembers, GroupMetadata, GroupPutUser, GroupRemoveUser, GroupRole, GroupRoles,
        GroupUserRef, KIND_GROUP_ADMINS, KIND_GROUP_CREATE_GROUP, KIND_GROUP_CREATE_INVITE,
        KIND_GROUP_DELETE_EVENT, KIND_GROUP_DELETE_GROUP, KIND_GROUP_EDIT_METADATA,
        KIND_GROUP_JOIN_REQUEST, KIND_GROUP_LEAVE_REQUEST, KIND_GROUP_MEMBERS, KIND_GROUP_METADATA,
        KIND_GROUP_PUT_USER, KIND_GROUP_REMOVE_USER, KIND_GROUP_ROLES,
    },
    tag::name::{TAG_D, TAG_E, TAG_H, TAG_P},
};

use crate::error::EventParseError;
use crate::field_helpers::{
    optional_tag_value, require_empty_content, required_tag_value, validate_non_empty_tag_value,
};

const TAG_ABOUT: &str = "about";
const TAG_CLOSED: &str = "closed";
const TAG_CODE: &str = "code";
const TAG_HIDDEN: &str = "hidden";
const TAG_NAME: &str = "name";
const TAG_PICTURE: &str = "picture";
const TAG_PRIVATE: &str = "private";
const TAG_RESTRICTED: &str = "restricted";
const TAG_ROLE: &str = "role";
const TAG_SUPPORTED_KINDS: &str = "supported_kinds";

pub fn group_put_user_from_event(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<GroupPutUser, EventParseError> {
    require_kind(kind, KIND_GROUP_PUT_USER, "9000")?;
    let (pubkey, roles) = required_user_tag(tags)?;
    Ok(GroupPutUser {
        group_id: required_tag_value(tags, TAG_H)?,
        message: optional_content(content),
        pubkey,
        roles,
    })
}

pub fn group_remove_user_from_event(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<GroupRemoveUser, EventParseError> {
    require_kind(kind, KIND_GROUP_REMOVE_USER, "9001")?;
    let (pubkey, _) = required_user_tag(tags)?;
    Ok(GroupRemoveUser {
        group_id: required_tag_value(tags, TAG_H)?,
        message: optional_content(content),
        pubkey,
    })
}

pub fn group_create_group_from_event(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<GroupCreateGroup, EventParseError> {
    require_kind(kind, KIND_GROUP_CREATE_GROUP, "9007")?;
    Ok(GroupCreateGroup {
        group_id: required_tag_value(tags, TAG_H)?,
        message: optional_content(content),
        metadata: metadata_from_tags(tags)?,
    })
}

pub fn group_edit_metadata_from_event(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<GroupEditMetadata, EventParseError> {
    require_kind(kind, KIND_GROUP_EDIT_METADATA, "9002")?;
    Ok(GroupEditMetadata {
        group_id: required_tag_value(tags, TAG_H)?,
        message: optional_content(content),
        metadata: metadata_from_tags(tags)?,
    })
}

pub fn group_delete_group_from_event(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<GroupDeleteGroup, EventParseError> {
    require_kind(kind, KIND_GROUP_DELETE_GROUP, "9008")?;
    Ok(GroupDeleteGroup {
        group_id: required_tag_value(tags, TAG_H)?,
        message: optional_content(content),
    })
}

pub fn group_delete_event_from_event(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<GroupDeleteEvent, EventParseError> {
    require_kind(kind, KIND_GROUP_DELETE_EVENT, "9005")?;
    Ok(GroupDeleteEvent {
        group_id: required_tag_value(tags, TAG_H)?,
        message: optional_content(content),
        event_id: required_tag_value(tags, TAG_E)?,
    })
}

pub fn group_create_invite_from_event(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<GroupCreateInvite, EventParseError> {
    require_kind(kind, KIND_GROUP_CREATE_INVITE, "9009")?;
    Ok(GroupCreateInvite {
        group_id: required_tag_value(tags, TAG_H)?,
        message: optional_content(content),
        code: required_tag_value(tags, TAG_CODE)?,
    })
}

pub fn group_join_request_from_event(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<GroupJoinRequest, EventParseError> {
    require_kind(kind, KIND_GROUP_JOIN_REQUEST, "9021")?;
    Ok(GroupJoinRequest {
        group_id: required_tag_value(tags, TAG_H)?,
        message: optional_content(content),
        code: optional_tag_value(tags, TAG_CODE)?,
    })
}

pub fn group_leave_request_from_event(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<GroupLeaveRequest, EventParseError> {
    require_kind(kind, KIND_GROUP_LEAVE_REQUEST, "9022")?;
    Ok(GroupLeaveRequest {
        group_id: required_tag_value(tags, TAG_H)?,
        message: optional_content(content),
    })
}

pub fn group_metadata_from_event(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<GroupMetadata, EventParseError> {
    require_kind(kind, KIND_GROUP_METADATA, "39000")?;
    require_empty_content(content, "content")?;
    Ok(GroupMetadata {
        d_tag: required_tag_value(tags, TAG_D)?,
        metadata: metadata_from_tags(tags)?,
    })
}

pub fn group_admins_from_event(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<GroupAdmins, EventParseError> {
    require_kind(kind, KIND_GROUP_ADMINS, "39001")?;
    Ok(GroupAdmins {
        d_tag: required_tag_value(tags, TAG_D)?,
        description: optional_content(content),
        admins: user_refs_from_tags(tags)?,
    })
}

pub fn group_members_from_event(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<GroupMembers, EventParseError> {
    require_kind(kind, KIND_GROUP_MEMBERS, "39002")?;
    Ok(GroupMembers {
        d_tag: required_tag_value(tags, TAG_D)?,
        description: optional_content(content),
        members: user_refs_from_tags(tags)?,
    })
}

pub fn group_roles_from_event(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<GroupRoles, EventParseError> {
    require_kind(kind, KIND_GROUP_ROLES, "39003")?;
    Ok(GroupRoles {
        d_tag: required_tag_value(tags, TAG_D)?,
        description: optional_content(content),
        roles: roles_from_tags(tags)?,
    })
}

fn require_kind(
    kind: u32,
    expected_kind: u32,
    expected: &'static str,
) -> Result<(), EventParseError> {
    if kind == expected_kind {
        Ok(())
    } else {
        Err(EventParseError::InvalidKind {
            expected,
            got: kind,
        })
    }
}

fn metadata_from_tags(tags: &[Vec<String>]) -> Result<GroupEditableMetadata, EventParseError> {
    Ok(GroupEditableMetadata {
        name: optional_tag_value(tags, TAG_NAME)?,
        about: optional_tag_value(tags, TAG_ABOUT)?,
        picture: optional_tag_value(tags, TAG_PICTURE)?,
        is_private: marker_tag(tags, TAG_PRIVATE)?,
        is_restricted: marker_tag(tags, TAG_RESTRICTED)?,
        is_closed: marker_tag(tags, TAG_CLOSED)?,
        is_hidden: marker_tag(tags, TAG_HIDDEN)?,
        supported_kinds: supported_kinds_from_tags(tags)?,
    })
}

fn marker_tag(tags: &[Vec<String>], key: &'static str) -> Result<bool, EventParseError> {
    let mut found = false;
    for tag in tags
        .iter()
        .filter(|tag| tag.first().map(|value| value.as_str()) == Some(key))
    {
        if found || tag.len() != 1 {
            return Err(EventParseError::InvalidTag(key));
        }
        found = true;
    }
    Ok(found)
}

fn supported_kinds_from_tags(tags: &[Vec<String>]) -> Result<Option<Vec<u32>>, EventParseError> {
    let mut matches = tags
        .iter()
        .filter(|tag| tag.first().map(|value| value.as_str()) == Some(TAG_SUPPORTED_KINDS));
    let Some(tag) = matches.next() else {
        return Ok(None);
    };
    if matches.next().is_some() {
        return Err(EventParseError::InvalidTag(TAG_SUPPORTED_KINDS));
    }
    let mut supported_kinds = Vec::new();
    for value in tag.iter().skip(1) {
        validate_non_empty_tag_value(value, TAG_SUPPORTED_KINDS)?;
        supported_kinds.push(
            value
                .parse::<u32>()
                .map_err(|err| EventParseError::InvalidNumber(TAG_SUPPORTED_KINDS, err))?,
        );
    }
    Ok(Some(supported_kinds))
}

fn required_user_tag(tags: &[Vec<String>]) -> Result<(String, Vec<String>), EventParseError> {
    let tag = tags
        .iter()
        .find(|tag| tag.first().map(|value| value.as_str()) == Some(TAG_P))
        .ok_or(EventParseError::MissingTag(TAG_P))?;
    user_from_tag(tag)
}

fn user_refs_from_tags(tags: &[Vec<String>]) -> Result<Vec<GroupUserRef>, EventParseError> {
    tags.iter()
        .filter(|tag| tag.first().map(|value| value.as_str()) == Some(TAG_P))
        .map(|tag| {
            let (pubkey, roles) = user_from_tag(tag)?;
            Ok(GroupUserRef { pubkey, roles })
        })
        .collect()
}

fn user_from_tag(tag: &[String]) -> Result<(String, Vec<String>), EventParseError> {
    let pubkey = tag
        .get(1)
        .cloned()
        .ok_or(EventParseError::InvalidTag(TAG_P))?;
    validate_non_empty_tag_value(&pubkey, TAG_P)?;
    let mut roles = Vec::new();
    for role in tag.iter().skip(2) {
        validate_non_empty_tag_value(role, TAG_P)?;
        roles.push(role.clone());
    }
    Ok((pubkey, roles))
}

fn roles_from_tags(tags: &[Vec<String>]) -> Result<Vec<GroupRole>, EventParseError> {
    tags.iter()
        .filter(|tag| tag.first().map(|value| value.as_str()) == Some(TAG_ROLE))
        .map(|tag| {
            let name = tag
                .get(1)
                .cloned()
                .ok_or(EventParseError::InvalidTag(TAG_ROLE))?;
            validate_non_empty_tag_value(&name, TAG_ROLE)?;
            let description = tag.get(2).cloned();
            if let Some(description) = description.as_deref() {
                validate_non_empty_tag_value(description, TAG_ROLE)?;
            }
            let mut permissions = Vec::new();
            for permission in tag.iter().skip(3) {
                validate_non_empty_tag_value(permission, TAG_ROLE)?;
                permissions.push(permission.clone());
            }
            Ok(GroupRole {
                name,
                description,
                permissions,
            })
        })
        .collect()
}

fn optional_content(content: &str) -> Option<String> {
    if content.is_empty() {
        None
    } else {
        Some(content.to_string())
    }
}
