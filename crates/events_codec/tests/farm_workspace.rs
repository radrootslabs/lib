#![cfg(feature = "serde_json")]

use radroots_events::{
    farm::RadrootsFarmRef,
    farm_crdt::KIND_FARM_CRDT_CHANGE,
    farm_workspace::{
        KIND_FARM_WORKSPACE_MANIFEST, RADROOTS_FARM_WORKSPACE_PROTOCOL_VERSION,
        RADROOTS_FARM_WORKSPACE_SCHEMA, RadrootsFarmWorkspaceManifest,
        RadrootsFarmWorkspaceMediaServer, RadrootsFarmWorkspaceRelay,
        RadrootsFarmWorkspaceRelayMode,
    },
    kinds::{KIND_FARM, KIND_FARM_FILE_METADATA},
    tags::{TAG_A, TAG_H, TAG_P},
};
use radroots_events_codec::{
    error::{EventEncodeError, EventParseError},
    farm_workspace::decode::farm_workspace_from_event,
    farm_workspace::encode::to_wire_parts,
};

const D_TAG: &str = "AAAAAAAAAAAAAAAAAAAAAA";
const FARM_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAAAQ";
const OTHER_FARM_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAAAg";
const GROUP_ID: &str = "field-group";

#[test]
fn farm_workspace_decode_handles_optional_and_mismatch_edges() {
    let manifest = sample_manifest();
    let parts = to_wire_parts(&manifest).unwrap();

    let mut mismatched_group = manifest.clone();
    mismatched_group.farm_group_id = "other-group".to_string();
    let content = serde_json::to_string(&mismatched_group).unwrap();
    assert!(matches!(
        farm_workspace_from_event(parts.kind, &parts.tags, &content),
        Err(EventParseError::InvalidTag(TAG_H))
    ));

    let mut without_owner = parts.tags.clone();
    without_owner.retain(|tag| tag.first().map(String::as_str) != Some(TAG_P));
    let decoded = farm_workspace_from_event(parts.kind, &without_owner, &parts.content).unwrap();
    assert_eq!(decoded.owner_pubkey, "workspace_owner_pubkey");

    let mut without_farm_address = parts.tags.clone();
    without_farm_address.retain(|tag| tag.first().map(String::as_str) != Some(TAG_A));
    let decoded =
        farm_workspace_from_event(parts.kind, &without_farm_address, &parts.content).unwrap();
    assert_eq!(
        decoded.farm.as_ref().map(|farm| farm.d_tag.as_str()),
        Some(FARM_D_TAG)
    );

    let mut mismatched_farm_address = parts.tags.clone();
    replace_first_tag(
        &mut mismatched_farm_address,
        TAG_A,
        vec![
            TAG_A.to_string(),
            format!("{KIND_FARM}:farm_pubkey:{OTHER_FARM_D_TAG}"),
        ],
    );
    assert!(matches!(
        farm_workspace_from_event(parts.kind, &mismatched_farm_address, &parts.content),
        Err(EventParseError::InvalidTag(TAG_A))
    ));

    let mut mismatched_farm_pubkey = parts.tags.clone();
    replace_first_tag(
        &mut mismatched_farm_pubkey,
        TAG_A,
        vec![
            TAG_A.to_string(),
            format!("{KIND_FARM}:other_farm:{FARM_D_TAG}"),
        ],
    );
    assert!(matches!(
        farm_workspace_from_event(parts.kind, &mismatched_farm_pubkey, &parts.content),
        Err(EventParseError::InvalidTag(TAG_A))
    ));

    for supported_kinds in [
        vec![KIND_FARM_WORKSPACE_MANIFEST],
        vec![KIND_FARM_CRDT_CHANGE],
    ] {
        let mut unsupported = manifest.clone();
        unsupported.supported_kinds = supported_kinds;
        let content = serde_json::to_string(&unsupported).unwrap();
        assert!(matches!(
            farm_workspace_from_event(parts.kind, &parts.tags, &content),
            Err(EventParseError::InvalidJson("supported_kinds"))
        ));
    }
}

#[test]
fn farm_workspace_encode_rejects_schema_and_supported_kind_edges() {
    let mut bad_schema = sample_manifest();
    bad_schema.schema = "radroots.farm.workspace.invalid".to_string();
    assert!(matches!(
        to_wire_parts(&bad_schema),
        Err(EventEncodeError::InvalidField("schema"))
    ));

    for supported_kinds in [
        vec![KIND_FARM_WORKSPACE_MANIFEST],
        vec![KIND_FARM_CRDT_CHANGE],
    ] {
        let mut unsupported = sample_manifest();
        unsupported.supported_kinds = supported_kinds;
        assert!(matches!(
            to_wire_parts(&unsupported),
            Err(EventEncodeError::InvalidField("supported_kinds"))
        ));
    }
}

fn sample_manifest() -> RadrootsFarmWorkspaceManifest {
    RadrootsFarmWorkspaceManifest {
        d_tag: D_TAG.to_string(),
        schema: RADROOTS_FARM_WORKSPACE_SCHEMA.to_string(),
        farm_group_id: GROUP_ID.to_string(),
        name: "Small Regen Farm".to_string(),
        owner_pubkey: "workspace_owner_pubkey".to_string(),
        farm: Some(RadrootsFarmRef {
            pubkey: "farm_pubkey".to_string(),
            d_tag: FARM_D_TAG.to_string(),
        }),
        relays: vec![RadrootsFarmWorkspaceRelay {
            url: "wss://relay.example.invalid/farm/field-group".to_string(),
            mode: RadrootsFarmWorkspaceRelayMode::ReadWrite,
        }],
        media_servers: vec![RadrootsFarmWorkspaceMediaServer {
            url: "https://media.example.invalid/farm/field-group".to_string(),
            service: "RadrootsPrivateMedia".to_string(),
        }],
        supported_kinds: vec![
            KIND_FARM_CRDT_CHANGE,
            KIND_FARM_WORKSPACE_MANIFEST,
            KIND_FARM_FILE_METADATA,
        ],
        protocol_version: RADROOTS_FARM_WORKSPACE_PROTOCOL_VERSION.to_string(),
        created_at_ms: 1_780_000_000_000,
        updated_at_ms: None,
    }
}

fn replace_first_tag(tags: &mut [Vec<String>], name: &str, replacement: Vec<String>) {
    let tag = tags
        .iter_mut()
        .find(|tag| tag.first().map(String::as_str) == Some(name))
        .expect("tag");
    *tag = replacement;
}
