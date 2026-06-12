#[cfg(not(feature = "std"))]
use alloc::{
    string::{String, ToString},
    vec::Vec,
};

#[cfg(feature = "serde_json")]
use radroots_events::farm_crdt::KIND_FARM_CRDT_CHANGE;
use radroots_events::{
    farm_crdt::{RADROOTS_FARM_CRDT_CHANGE_SCHEMA, RADROOTS_FARM_CRDT_TAG, RadrootsFarmCrdtChange},
    farm_workspace::KIND_FARM_WORKSPACE_MANIFEST,
    tags::{TAG_A, TAG_D, TAG_H, TAG_P, TAG_T},
};

use crate::d_tag::validate_d_tag;
use crate::error::EventEncodeError;
use crate::field_helpers::{
    address_string, push_optional_tag, push_tag, validate_non_empty_base64url,
    validate_non_empty_field,
};
#[cfg(feature = "serde_json")]
use crate::wire::WireEventParts;

pub fn farm_crdt_change_build_tags(
    change: &RadrootsFarmCrdtChange,
) -> Result<Vec<Vec<String>>, EventEncodeError> {
    farm_crdt_change_build_tags_with_author(change, None)
}

pub fn farm_crdt_change_build_tags_with_author(
    change: &RadrootsFarmCrdtChange,
    author_pubkey: Option<&str>,
) -> Result<Vec<Vec<String>>, EventEncodeError> {
    validate_change(change)?;
    if let Some(author_pubkey) = author_pubkey {
        validate_non_empty_field(author_pubkey, "author_pubkey")?;
    }
    let workspace = address_string(
        KIND_FARM_WORKSPACE_MANIFEST,
        &change.workspace.pubkey,
        &change.workspace.d_tag,
        "workspace",
    )?;
    let mut tags = Vec::new();
    push_tag(&mut tags, TAG_H, change.farm_group_id.as_str());
    push_tag(&mut tags, TAG_D, change.document_id.as_str());
    push_tag(&mut tags, TAG_A, workspace);
    push_optional_tag(&mut tags, TAG_P, author_pubkey);
    push_tag(&mut tags, TAG_T, RADROOTS_FARM_CRDT_TAG);
    Ok(tags)
}

#[cfg(feature = "serde_json")]
pub fn to_wire_parts(change: &RadrootsFarmCrdtChange) -> Result<WireEventParts, EventEncodeError> {
    to_wire_parts_with_kind(change, KIND_FARM_CRDT_CHANGE)
}

#[cfg(feature = "serde_json")]
pub fn to_wire_parts_with_author(
    change: &RadrootsFarmCrdtChange,
    author_pubkey: &str,
) -> Result<WireEventParts, EventEncodeError> {
    to_wire_parts_with_kind_and_author(change, KIND_FARM_CRDT_CHANGE, Some(author_pubkey))
}

#[cfg(feature = "serde_json")]
pub fn to_wire_parts_with_kind(
    change: &RadrootsFarmCrdtChange,
    kind: u32,
) -> Result<WireEventParts, EventEncodeError> {
    to_wire_parts_with_kind_and_author(change, kind, None)
}

#[cfg(feature = "serde_json")]
pub fn to_wire_parts_with_kind_and_author(
    change: &RadrootsFarmCrdtChange,
    kind: u32,
    author_pubkey: Option<&str>,
) -> Result<WireEventParts, EventEncodeError> {
    if kind != KIND_FARM_CRDT_CHANGE {
        return Err(EventEncodeError::InvalidKind(kind));
    }
    let tags = farm_crdt_change_build_tags_with_author(change, author_pubkey)?;
    let content = serde_json::to_string(change).map_err(|_| EventEncodeError::Json)?;
    Ok(WireEventParts {
        kind,
        content,
        tags,
    })
}

pub(crate) fn validate_change(change: &RadrootsFarmCrdtChange) -> Result<(), EventEncodeError> {
    if change.schema != RADROOTS_FARM_CRDT_CHANGE_SCHEMA {
        return Err(EventEncodeError::InvalidField("schema"));
    }
    validate_non_empty_field(&change.farm_group_id, "farm_group_id")?;
    validate_d_tag(&change.document_id, "document_id")?;
    validate_non_empty_field(&change.workspace.pubkey, "workspace.pubkey")?;
    validate_d_tag(&change.workspace.d_tag, "workspace.d_tag")?;
    validate_non_empty_field(&change.actor_id, "actor_id")?;
    validate_non_empty_field(&change.change_hash, "change_hash")?;
    for dependency in &change.dependencies {
        validate_non_empty_field(dependency, "dependencies")?;
    }
    validate_non_empty_base64url(&change.encoded_change, "encoded_change")?;
    if change.business_time_ms == 0 {
        return Err(EventEncodeError::InvalidField("business_time_ms"));
    }
    if let Some(version) = change.crdt_backend_version.as_deref() {
        validate_non_empty_field(version, "crdt_backend_version")?;
    }
    if let Some(member_id) = change.author_member_id.as_deref() {
        validate_non_empty_field(member_id, "author_member_id")?;
    }
    if let Some(app_version) = change.app_version.as_deref() {
        validate_non_empty_field(app_version, "app_version")?;
    }
    Ok(())
}
