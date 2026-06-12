#[cfg(not(feature = "std"))]
use alloc::{
    string::{String, ToString},
    vec::Vec,
};

use radroots_events::{
    farm_workspace::{
        KIND_FARM_WORKSPACE_MANIFEST, RADROOTS_FARM_WORKSPACE_SCHEMA, RADROOTS_FARM_WORKSPACE_TAG,
        RadrootsFarmWorkspaceManifest,
    },
    kinds::{KIND_FARM, KIND_FARM_CRDT_CHANGE, KIND_FARM_FILE_METADATA},
    tags::{TAG_A, TAG_D, TAG_H, TAG_P, TAG_T},
};

use crate::d_tag::validate_d_tag;
use crate::error::EventEncodeError;
use crate::field_helpers::{address_string, push_tag, validate_non_empty_field};
#[cfg(feature = "serde_json")]
use crate::wire::WireEventParts;

pub fn farm_workspace_build_tags(
    manifest: &RadrootsFarmWorkspaceManifest,
) -> Result<Vec<Vec<String>>, EventEncodeError> {
    validate_manifest(manifest)?;
    let mut tags = Vec::new();
    push_tag(&mut tags, TAG_D, manifest.d_tag.as_str());
    push_tag(&mut tags, TAG_H, manifest.farm_group_id.as_str());
    push_tag(&mut tags, TAG_P, manifest.owner_pubkey.as_str());
    push_tag(&mut tags, TAG_T, RADROOTS_FARM_WORKSPACE_TAG);
    if let Some(farm) = manifest.farm.as_ref() {
        let address = address_string(KIND_FARM, &farm.pubkey, &farm.d_tag, "farm")?;
        push_tag(&mut tags, TAG_A, address);
    }
    Ok(tags)
}

#[cfg(feature = "serde_json")]
pub fn to_wire_parts(
    manifest: &RadrootsFarmWorkspaceManifest,
) -> Result<WireEventParts, EventEncodeError> {
    to_wire_parts_with_kind(manifest, KIND_FARM_WORKSPACE_MANIFEST)
}

#[cfg(feature = "serde_json")]
pub fn to_wire_parts_with_kind(
    manifest: &RadrootsFarmWorkspaceManifest,
    kind: u32,
) -> Result<WireEventParts, EventEncodeError> {
    if kind != KIND_FARM_WORKSPACE_MANIFEST {
        return Err(EventEncodeError::InvalidKind(kind));
    }
    let tags = farm_workspace_build_tags(manifest)?;
    let content = serde_json::to_string(manifest).map_err(|_| EventEncodeError::Json)?;
    Ok(WireEventParts {
        kind,
        content,
        tags,
    })
}

pub(crate) fn validate_manifest(
    manifest: &RadrootsFarmWorkspaceManifest,
) -> Result<(), EventEncodeError> {
    validate_d_tag(&manifest.d_tag, "d_tag")?;
    validate_non_empty_field(&manifest.farm_group_id, "farm_group_id")?;
    validate_non_empty_field(&manifest.name, "name")?;
    validate_non_empty_field(&manifest.owner_pubkey, "owner_pubkey")?;
    validate_non_empty_field(&manifest.protocol_version, "protocol_version")?;
    if manifest.schema != RADROOTS_FARM_WORKSPACE_SCHEMA {
        return Err(EventEncodeError::InvalidField("schema"));
    }
    if manifest.relays.is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("relays"));
    }
    if !manifest
        .supported_kinds
        .contains(&KIND_FARM_WORKSPACE_MANIFEST)
        || !manifest.supported_kinds.contains(&KIND_FARM_CRDT_CHANGE)
    {
        return Err(EventEncodeError::InvalidField("supported_kinds"));
    }
    if !manifest.media_servers.is_empty()
        && !manifest.supported_kinds.contains(&KIND_FARM_FILE_METADATA)
    {
        return Err(EventEncodeError::InvalidField("supported_kinds"));
    }
    for relay in &manifest.relays {
        validate_non_empty_field(&relay.url, "relays.url")?;
    }
    for media_server in &manifest.media_servers {
        validate_non_empty_field(&media_server.url, "media_servers.url")?;
        validate_non_empty_field(&media_server.service, "media_servers.service")?;
    }
    if let Some(farm) = manifest.farm.as_ref() {
        validate_non_empty_field(&farm.pubkey, "farm.pubkey")?;
        validate_d_tag(&farm.d_tag, "farm.d_tag")?;
    }
    Ok(())
}
