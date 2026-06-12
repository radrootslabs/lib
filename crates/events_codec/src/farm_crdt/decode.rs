#![cfg(feature = "serde_json")]

#[cfg(not(feature = "std"))]
use alloc::{
    string::{String, ToString},
    vec::Vec,
};

use radroots_events::{
    RadrootsNostrEvent,
    farm_crdt::{
        KIND_FARM_CRDT_CHANGE, RADROOTS_FARM_CRDT_CHANGE_SCHEMA, RADROOTS_FARM_CRDT_TAG,
        RadrootsFarmCrdtChange,
    },
    farm_workspace::KIND_FARM_WORKSPACE_MANIFEST,
    tags::{TAG_A, TAG_D, TAG_H, TAG_P, TAG_T},
};

use crate::d_tag::validate_d_tag_tag;
use crate::error::EventParseError;
use crate::farm_crdt::encode::validate_change;
use crate::field_helpers::{
    is_non_empty_base64url, optional_tag_value, parse_address_tag_with_kind, required_tag_value,
    tag_values, validate_non_empty_tag_value,
};
use crate::parsed::{RadrootsParsedData, RadrootsParsedEvent};

const EXPECTED_KIND: &str = "78";

pub fn farm_crdt_change_from_event(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<RadrootsFarmCrdtChange, EventParseError> {
    farm_crdt_change_from_event_inner(kind, tags, content, None)
}

pub fn farm_crdt_change_from_event_with_author(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
    author_pubkey: &str,
) -> Result<RadrootsFarmCrdtChange, EventParseError> {
    validate_non_empty_tag_value(author_pubkey, TAG_P)?;
    farm_crdt_change_from_event_inner(kind, tags, content, Some(author_pubkey))
}

pub fn data_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
) -> Result<RadrootsParsedData<RadrootsFarmCrdtChange>, EventParseError> {
    let change = farm_crdt_change_from_event_with_author(kind, &tags, &content, &author)?;
    Ok(RadrootsParsedData::new(
        id,
        author,
        published_at,
        kind,
        change,
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
) -> Result<RadrootsParsedEvent<RadrootsFarmCrdtChange>, EventParseError> {
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

fn farm_crdt_change_from_event_inner(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
    author_pubkey: Option<&str>,
) -> Result<RadrootsFarmCrdtChange, EventParseError> {
    if kind != KIND_FARM_CRDT_CHANGE {
        return Err(EventParseError::InvalidKind {
            expected: EXPECTED_KIND,
            got: kind,
        });
    }
    if content.trim().is_empty() {
        return Err(EventParseError::InvalidJson("content"));
    }

    let farm_group_id = required_tag_value(tags, TAG_H)?;
    let document_id = required_tag_value(tags, TAG_D)?;
    validate_d_tag_tag(&document_id, TAG_D)?;
    let workspace_address = required_tag_value(tags, TAG_A)?;
    let workspace =
        parse_address_tag_with_kind(&workspace_address, KIND_FARM_WORKSPACE_MANIFEST, TAG_A)?;
    let marker_tags = tag_values(tags, TAG_T)?;
    if !marker_tags
        .iter()
        .any(|value| value == RADROOTS_FARM_CRDT_TAG)
    {
        return Err(EventParseError::MissingTag(TAG_T));
    }
    if let Some(tag_author) = optional_tag_value(tags, TAG_P)? {
        if let Some(author_pubkey) = author_pubkey {
            if tag_author != author_pubkey {
                return Err(EventParseError::InvalidTag(TAG_P));
            }
        }
    }

    let change: RadrootsFarmCrdtChange =
        serde_json::from_str(content).map_err(|_| EventParseError::InvalidJson("content"))?;
    validate_change_content(&change)?;
    validate_change(&change).map_err(encode_error_to_parse_error)?;
    if change.farm_group_id != farm_group_id {
        return Err(EventParseError::InvalidTag(TAG_H));
    }
    if change.document_id != document_id {
        return Err(EventParseError::InvalidTag(TAG_D));
    }
    if change.workspace.pubkey != workspace.pubkey || change.workspace.d_tag != workspace.d_tag {
        return Err(EventParseError::InvalidTag(TAG_A));
    }
    Ok(change)
}

fn validate_change_content(change: &RadrootsFarmCrdtChange) -> Result<(), EventParseError> {
    if change.schema != RADROOTS_FARM_CRDT_CHANGE_SCHEMA {
        return Err(EventParseError::InvalidJson("schema"));
    }
    validate_non_empty_tag_value(&change.farm_group_id, TAG_H)?;
    validate_d_tag_tag(&change.document_id, TAG_D)?;
    validate_non_empty_tag_value(&change.workspace.pubkey, TAG_A)?;
    validate_d_tag_tag(&change.workspace.d_tag, TAG_A)?;
    if !is_non_empty_base64url(&change.encoded_change) {
        return Err(EventParseError::InvalidJson("encoded_change"));
    }
    if change.change_hash.trim().is_empty() {
        return Err(EventParseError::InvalidJson("change_hash"));
    }
    if change.business_time_ms == 0 {
        return Err(EventParseError::InvalidJson("business_time_ms"));
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
            "farm_group_id" => EventParseError::InvalidTag(TAG_H),
            "document_id" => EventParseError::InvalidTag(TAG_D),
            "workspace.pubkey" | "workspace.d_tag" => EventParseError::InvalidTag(TAG_A),
            _ => EventParseError::InvalidJson(field),
        },
        crate::error::EventEncodeError::Json => EventParseError::InvalidJson("content"),
    }
}
