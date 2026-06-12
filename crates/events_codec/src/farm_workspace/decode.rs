#![cfg(feature = "serde_json")]

#[cfg(not(feature = "std"))]
use alloc::{
    string::{String, ToString},
    vec::Vec,
};

use radroots_events::{
    RadrootsNostrEvent,
    farm_workspace::{
        KIND_FARM_WORKSPACE_MANIFEST, RADROOTS_FARM_WORKSPACE_SCHEMA, RADROOTS_FARM_WORKSPACE_TAG,
        RadrootsFarmWorkspaceManifest,
    },
    kinds::{KIND_FARM, KIND_FARM_CRDT_CHANGE},
    tags::{TAG_A, TAG_D, TAG_H, TAG_P, TAG_T},
};

use crate::d_tag::validate_d_tag_tag;
use crate::error::EventParseError;
use crate::farm_workspace::encode::validate_manifest;
use crate::field_helpers::{
    optional_tag_value, parse_address_tag_with_kind, required_tag_value, tag_values,
    validate_non_empty_tag_value,
};
use crate::parsed::{RadrootsParsedData, RadrootsParsedEvent};

const EXPECTED_KIND: &str = "30078";

pub fn farm_workspace_from_event(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<RadrootsFarmWorkspaceManifest, EventParseError> {
    if kind != KIND_FARM_WORKSPACE_MANIFEST {
        return Err(EventParseError::InvalidKind {
            expected: EXPECTED_KIND,
            got: kind,
        });
    }
    if content.trim().is_empty() {
        return Err(EventParseError::InvalidJson("content"));
    }
    let d_tag = required_tag_value(tags, TAG_D)?;
    validate_d_tag_tag(&d_tag, TAG_D)?;
    let farm_group_id = required_tag_value(tags, TAG_H)?;
    let manifest: RadrootsFarmWorkspaceManifest =
        serde_json::from_str(content).map_err(|_| EventParseError::InvalidJson("content"))?;
    validate_manifest_content(&manifest)?;
    validate_manifest(&manifest).map_err(encode_error_to_parse_error)?;

    if manifest.d_tag != d_tag {
        return Err(EventParseError::InvalidTag(TAG_D));
    }
    if manifest.farm_group_id != farm_group_id {
        return Err(EventParseError::InvalidTag(TAG_H));
    }
    if let Some(owner_pubkey) = optional_tag_value(tags, TAG_P)? {
        if owner_pubkey != manifest.owner_pubkey {
            return Err(EventParseError::InvalidTag(TAG_P));
        }
    }
    let marker_tags = tag_values(tags, TAG_T)?;
    if !marker_tags
        .iter()
        .any(|value| value == RADROOTS_FARM_WORKSPACE_TAG)
    {
        return Err(EventParseError::MissingTag(TAG_T));
    }
    if let Some(farm) = manifest.farm.as_ref() {
        let farm_address = optional_tag_value(tags, TAG_A)?;
        if let Some(value) = farm_address {
            let address = parse_address_tag_with_kind(&value, KIND_FARM, TAG_A)?;
            if address.pubkey != farm.pubkey || address.d_tag != farm.d_tag {
                return Err(EventParseError::InvalidTag(TAG_A));
            }
        }
    }

    Ok(manifest)
}

pub fn data_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
) -> Result<RadrootsParsedData<RadrootsFarmWorkspaceManifest>, EventParseError> {
    let manifest = farm_workspace_from_event(kind, &tags, &content)?;
    Ok(RadrootsParsedData::new(
        id,
        author,
        published_at,
        kind,
        manifest,
    ))
}

pub fn parsed_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
    sig: String,
) -> Result<RadrootsParsedEvent<RadrootsFarmWorkspaceManifest>, EventParseError> {
    let data = data_from_event(
        id.clone(),
        author.clone(),
        published_at,
        kind,
        content.clone(),
        tags.clone(),
    )?;
    Ok(RadrootsParsedEvent {
        event: RadrootsNostrEvent {
            id,
            author,
            created_at: published_at,
            kind,
            content,
            tags,
            sig,
        },
        data,
    })
}

fn validate_manifest_content(
    manifest: &RadrootsFarmWorkspaceManifest,
) -> Result<(), EventParseError> {
    if manifest.schema != RADROOTS_FARM_WORKSPACE_SCHEMA {
        return Err(EventParseError::InvalidJson("schema"));
    }
    validate_non_empty_tag_value(&manifest.farm_group_id, TAG_H)?;
    validate_non_empty_tag_value(&manifest.owner_pubkey, TAG_P)?;
    if manifest.relays.is_empty() {
        return Err(EventParseError::InvalidJson("relays"));
    }
    if !manifest
        .supported_kinds
        .contains(&KIND_FARM_WORKSPACE_MANIFEST)
        || !manifest.supported_kinds.contains(&KIND_FARM_CRDT_CHANGE)
    {
        return Err(EventParseError::InvalidJson("supported_kinds"));
    }
    Ok(())
}

fn encode_error_to_parse_error(error: crate::error::EventEncodeError) -> EventParseError {
    match error {
        crate::error::EventEncodeError::InvalidKind(kind) => EventParseError::InvalidKind {
            expected: EXPECTED_KIND,
            got: kind,
        },
        crate::error::EventEncodeError::EmptyRequiredField(field)
        | crate::error::EventEncodeError::InvalidField(field) => match field {
            "d_tag" | "farm.d_tag" => EventParseError::InvalidTag(TAG_D),
            "farm_group_id" => EventParseError::InvalidTag(TAG_H),
            "owner_pubkey" => EventParseError::InvalidTag(TAG_P),
            _ => EventParseError::InvalidJson(field),
        },
        crate::error::EventEncodeError::Json => EventParseError::InvalidJson("content"),
    }
}
