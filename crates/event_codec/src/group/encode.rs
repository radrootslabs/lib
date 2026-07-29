#[cfg(not(feature = "std"))]
use alloc::{
    string::{String, ToString},
    vec,
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

use crate::error::EventEncodeError;
use crate::field_helpers::{
    push_optional_tag, push_tag, push_tag_values, validate_non_empty_field,
};
use radroots_event::wire::Nip01EventWireParts;

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

pub fn group_put_user_build_tags(
    event: &GroupPutUser,
) -> Result<Vec<Vec<String>>, EventEncodeError> {
    let mut tags = h_tags(&event.group_id)?;
    push_user_tag(&mut tags, &event.pubkey, &event.roles)?;
    Ok(tags)
}

pub fn group_remove_user_build_tags(
    event: &GroupRemoveUser,
) -> Result<Vec<Vec<String>>, EventEncodeError> {
    let mut tags = h_tags(&event.group_id)?;
    push_tag(&mut tags, TAG_P, event.pubkey.as_str());
    validate_non_empty_field(&event.pubkey, "pubkey")?;
    Ok(tags)
}

pub fn group_create_group_build_tags(
    event: &GroupCreateGroup,
) -> Result<Vec<Vec<String>>, EventEncodeError> {
    let mut tags = h_tags(&event.group_id)?;
    push_metadata_tags(&mut tags, &event.metadata)?;
    Ok(tags)
}

pub fn group_edit_metadata_build_tags(
    event: &GroupEditMetadata,
) -> Result<Vec<Vec<String>>, EventEncodeError> {
    let mut tags = h_tags(&event.group_id)?;
    push_metadata_tags(&mut tags, &event.metadata)?;
    Ok(tags)
}

pub fn group_delete_group_build_tags(
    event: &GroupDeleteGroup,
) -> Result<Vec<Vec<String>>, EventEncodeError> {
    h_tags(&event.group_id)
}

pub fn group_delete_event_build_tags(
    event: &GroupDeleteEvent,
) -> Result<Vec<Vec<String>>, EventEncodeError> {
    let mut tags = h_tags(&event.group_id)?;
    validate_non_empty_field(&event.event_id, "event_id")?;
    push_tag(&mut tags, TAG_E, event.event_id.as_str());
    Ok(tags)
}

pub fn group_create_invite_build_tags(
    event: &GroupCreateInvite,
) -> Result<Vec<Vec<String>>, EventEncodeError> {
    let mut tags = h_tags(&event.group_id)?;
    validate_non_empty_field(&event.code, "code")?;
    push_tag(&mut tags, TAG_CODE, event.code.as_str());
    Ok(tags)
}

pub fn group_join_request_build_tags(
    event: &GroupJoinRequest,
) -> Result<Vec<Vec<String>>, EventEncodeError> {
    let mut tags = h_tags(&event.group_id)?;
    push_optional_tag(&mut tags, TAG_CODE, event.code.as_deref());
    validate_optional(event.code.as_deref(), "code")?;
    Ok(tags)
}

pub fn group_leave_request_build_tags(
    event: &GroupLeaveRequest,
) -> Result<Vec<Vec<String>>, EventEncodeError> {
    h_tags(&event.group_id)
}

pub fn group_metadata_build_tags(
    event: &GroupMetadata,
) -> Result<Vec<Vec<String>>, EventEncodeError> {
    let mut tags = d_tags(&event.d_tag)?;
    push_metadata_tags(&mut tags, &event.metadata)?;
    Ok(tags)
}

pub fn group_admins_build_tags(event: &GroupAdmins) -> Result<Vec<Vec<String>>, EventEncodeError> {
    let mut tags = d_tags(&event.d_tag)?;
    push_user_refs(&mut tags, &event.admins)?;
    Ok(tags)
}

pub fn group_members_build_tags(
    event: &GroupMembers,
) -> Result<Vec<Vec<String>>, EventEncodeError> {
    let mut tags = d_tags(&event.d_tag)?;
    push_user_refs(&mut tags, &event.members)?;
    Ok(tags)
}

pub fn group_roles_build_tags(event: &GroupRoles) -> Result<Vec<Vec<String>>, EventEncodeError> {
    let mut tags = d_tags(&event.d_tag)?;
    for role in &event.roles {
        validate_role(role)?;
        let mut values = vec![role.name.clone()];
        if let Some(description) = role.description.as_deref() {
            values.push(description.to_string());
        }
        values.extend(role.permissions.iter().cloned());
        push_tag_values(&mut tags, TAG_ROLE, values);
    }
    Ok(tags)
}

pub fn group_put_user_to_wire_parts(
    event: &GroupPutUser,
) -> Result<Nip01EventWireParts, EventEncodeError> {
    message_wire(
        KIND_GROUP_PUT_USER,
        group_put_user_build_tags(event)?,
        event.message.as_deref(),
    )
}

pub fn group_remove_user_to_wire_parts(
    event: &GroupRemoveUser,
) -> Result<Nip01EventWireParts, EventEncodeError> {
    message_wire(
        KIND_GROUP_REMOVE_USER,
        group_remove_user_build_tags(event)?,
        event.message.as_deref(),
    )
}

pub fn group_create_group_to_wire_parts(
    event: &GroupCreateGroup,
) -> Result<Nip01EventWireParts, EventEncodeError> {
    message_wire(
        KIND_GROUP_CREATE_GROUP,
        group_create_group_build_tags(event)?,
        event.message.as_deref(),
    )
}

pub fn group_edit_metadata_to_wire_parts(
    event: &GroupEditMetadata,
) -> Result<Nip01EventWireParts, EventEncodeError> {
    message_wire(
        KIND_GROUP_EDIT_METADATA,
        group_edit_metadata_build_tags(event)?,
        event.message.as_deref(),
    )
}

pub fn group_delete_group_to_wire_parts(
    event: &GroupDeleteGroup,
) -> Result<Nip01EventWireParts, EventEncodeError> {
    message_wire(
        KIND_GROUP_DELETE_GROUP,
        group_delete_group_build_tags(event)?,
        event.message.as_deref(),
    )
}

pub fn group_delete_event_to_wire_parts(
    event: &GroupDeleteEvent,
) -> Result<Nip01EventWireParts, EventEncodeError> {
    message_wire(
        KIND_GROUP_DELETE_EVENT,
        group_delete_event_build_tags(event)?,
        event.message.as_deref(),
    )
}

pub fn group_create_invite_to_wire_parts(
    event: &GroupCreateInvite,
) -> Result<Nip01EventWireParts, EventEncodeError> {
    message_wire(
        KIND_GROUP_CREATE_INVITE,
        group_create_invite_build_tags(event)?,
        event.message.as_deref(),
    )
}

pub fn group_join_request_to_wire_parts(
    event: &GroupJoinRequest,
) -> Result<Nip01EventWireParts, EventEncodeError> {
    message_wire(
        KIND_GROUP_JOIN_REQUEST,
        group_join_request_build_tags(event)?,
        event.message.as_deref(),
    )
}

pub fn group_leave_request_to_wire_parts(
    event: &GroupLeaveRequest,
) -> Result<Nip01EventWireParts, EventEncodeError> {
    message_wire(
        KIND_GROUP_LEAVE_REQUEST,
        group_leave_request_build_tags(event)?,
        event.message.as_deref(),
    )
}

pub fn group_metadata_to_wire_parts(
    event: &GroupMetadata,
) -> Result<Nip01EventWireParts, EventEncodeError> {
    empty_wire(KIND_GROUP_METADATA, group_metadata_build_tags(event)?)
}

pub fn group_admins_to_wire_parts(
    event: &GroupAdmins,
) -> Result<Nip01EventWireParts, EventEncodeError> {
    message_wire(
        KIND_GROUP_ADMINS,
        group_admins_build_tags(event)?,
        event.description.as_deref(),
    )
}

pub fn group_members_to_wire_parts(
    event: &GroupMembers,
) -> Result<Nip01EventWireParts, EventEncodeError> {
    message_wire(
        KIND_GROUP_MEMBERS,
        group_members_build_tags(event)?,
        event.description.as_deref(),
    )
}

pub fn group_roles_to_wire_parts(
    event: &GroupRoles,
) -> Result<Nip01EventWireParts, EventEncodeError> {
    message_wire(
        KIND_GROUP_ROLES,
        group_roles_build_tags(event)?,
        event.description.as_deref(),
    )
}

fn h_tags(group_id: &str) -> Result<Vec<Vec<String>>, EventEncodeError> {
    validate_non_empty_field(group_id, "group_id")?;
    let mut tags = Vec::new();
    push_tag(&mut tags, TAG_H, group_id);
    Ok(tags)
}

fn d_tags(d_tag: &str) -> Result<Vec<Vec<String>>, EventEncodeError> {
    validate_non_empty_field(d_tag, "d_tag")?;
    let mut tags = Vec::new();
    push_tag(&mut tags, TAG_D, d_tag);
    Ok(tags)
}

fn push_metadata_tags(
    tags: &mut Vec<Vec<String>>,
    metadata: &GroupEditableMetadata,
) -> Result<(), EventEncodeError> {
    push_optional_tag(tags, TAG_NAME, metadata.name.as_deref());
    push_optional_tag(tags, TAG_ABOUT, metadata.about.as_deref());
    push_optional_tag(tags, TAG_PICTURE, metadata.picture.as_deref());
    if metadata.is_private {
        push_marker_tag(tags, TAG_PRIVATE);
    }
    if metadata.is_restricted {
        push_marker_tag(tags, TAG_RESTRICTED);
    }
    if metadata.is_closed {
        push_marker_tag(tags, TAG_CLOSED);
    }
    if metadata.is_hidden {
        push_marker_tag(tags, TAG_HIDDEN);
    }
    if let Some(supported_kinds) = metadata.supported_kinds.as_deref() {
        push_tag_values(
            tags,
            TAG_SUPPORTED_KINDS,
            supported_kinds.iter().map(ToString::to_string),
        );
    }
    validate_optional(metadata.name.as_deref(), "name")?;
    validate_optional(metadata.about.as_deref(), "about")?;
    validate_optional(metadata.picture.as_deref(), "picture")?;
    Ok(())
}

fn push_marker_tag(tags: &mut Vec<Vec<String>>, key: &str) {
    tags.push(vec![key.to_string()]);
}

fn push_user_refs(
    tags: &mut Vec<Vec<String>>,
    users: &[GroupUserRef],
) -> Result<(), EventEncodeError> {
    for user in users {
        push_user_tag(tags, &user.pubkey, &user.roles)?;
    }
    Ok(())
}

fn push_user_tag(
    tags: &mut Vec<Vec<String>>,
    pubkey: &str,
    roles: &[String],
) -> Result<(), EventEncodeError> {
    validate_non_empty_field(pubkey, "pubkey")?;
    for role in roles {
        validate_non_empty_field(role, "roles")?;
    }
    let mut values = vec![pubkey.to_string()];
    values.extend(roles.iter().cloned());
    push_tag_values(tags, TAG_P, values);
    Ok(())
}

fn validate_role(role: &GroupRole) -> Result<(), EventEncodeError> {
    validate_non_empty_field(&role.name, "role.name")?;
    validate_optional(role.description.as_deref(), "role.description")?;
    for permission in &role.permissions {
        validate_non_empty_field(permission, "role.permissions")?;
    }
    Ok(())
}

fn validate_optional(value: Option<&str>, field: &'static str) -> Result<(), EventEncodeError> {
    if let Some(value) = value {
        validate_non_empty_field(value, field)?;
    }
    Ok(())
}

fn empty_wire(kind: u32, tags: Vec<Vec<String>>) -> Result<Nip01EventWireParts, EventEncodeError> {
    Ok(Nip01EventWireParts {
        kind,
        content: String::new(),
        tags,
    })
}

fn message_wire(
    kind: u32,
    tags: Vec<Vec<String>>,
    message: Option<&str>,
) -> Result<Nip01EventWireParts, EventEncodeError> {
    validate_optional(message, "message")?;
    Ok(Nip01EventWireParts {
        kind,
        content: message.unwrap_or_default().to_string(),
        tags,
    })
}
