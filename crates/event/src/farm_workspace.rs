#![forbid(unsafe_code)]

use crate::farm::RadrootsFarmRef;
use crate::kinds::KIND_FARM_WORKSPACE_MANIFEST as KIND_FARM_WORKSPACE_MANIFEST_EVENT;

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

pub const KIND_FARM_WORKSPACE_MANIFEST: u32 = KIND_FARM_WORKSPACE_MANIFEST_EVENT;
pub const RADROOTS_FARM_WORKSPACE_SCHEMA: &str = "radroots.farm.workspace.v1";
pub const RADROOTS_FARM_WORKSPACE_PROTOCOL_VERSION: &str = "field-nostr-v1";
pub const RADROOTS_FARM_WORKSPACE_TAG: &str = "radroots:farm:workspace";

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsFarmWorkspaceManifest {
    pub d_tag: String,
    pub schema: String,
    pub farm_group_id: String,
    pub name: String,
    pub owner_pubkey: String,
    pub farm: Option<RadrootsFarmRef>,
    pub relays: Vec<RadrootsFarmWorkspaceRelay>,
    pub media_servers: Vec<RadrootsFarmWorkspaceMediaServer>,
    pub supported_kinds: Vec<u32>,
    pub protocol_version: String,
    pub created_at_ms: u64,
    pub updated_at_ms: Option<u64>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RadrootsFarmWorkspaceRef {
    pub pubkey: String,
    pub d_tag: String,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsFarmWorkspaceRelay {
    pub url: String,
    pub mode: RadrootsFarmWorkspaceRelayMode,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsFarmWorkspaceRelayMode {
    Read,
    Write,
    ReadWrite,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsFarmWorkspaceMediaServer {
    pub url: String,
    pub service: String,
}

#[cfg(all(test, feature = "serde"))]
mod tests {
    use super::*;
    use crate::kinds::{
        KIND_APP_DATA, KIND_FARM_CRDT_CHANGE, KIND_FARM_FILE_METADATA,
        KIND_FARM_WORKSPACE_MANIFEST as KIND_FARM_WORKSPACE_MANIFEST_KIND, KIND_HTTP_AUTH,
        KIND_RELAY_AUTH,
    };

    #[test]
    fn manifest_kind_aliases_existing_app_data_kind() {
        assert_eq!(KIND_FARM_WORKSPACE_MANIFEST, KIND_APP_DATA);
        assert_eq!(KIND_FARM_WORKSPACE_MANIFEST_KIND, KIND_APP_DATA);
    }

    #[test]
    fn manifest_represents_required_workspace_contract_fields() {
        let manifest = sample_manifest();

        assert_eq!(manifest.d_tag, "ABCDEFGHIJKLMNOPQRSTUV");
        assert_eq!(manifest.schema, RADROOTS_FARM_WORKSPACE_SCHEMA);
        assert_eq!(manifest.farm_group_id, "BCDEFGHIJKLMNOPQRSTUVW");
        assert_eq!(manifest.relays.len(), 1);
        assert_eq!(manifest.media_servers.len(), 1);
        assert!(manifest.supported_kinds.contains(&KIND_FARM_CRDT_CHANGE));
        assert!(
            manifest
                .supported_kinds
                .contains(&KIND_FARM_WORKSPACE_MANIFEST)
        );
        assert_eq!(manifest.created_at_ms, 1_780_000_000_000);
        assert_eq!(manifest.updated_at_ms, None);
    }

    #[test]
    fn manifest_serializes_stable_content_shape() {
        let value = serde_json::to_value(sample_manifest()).unwrap();

        assert_eq!(value["schema"], RADROOTS_FARM_WORKSPACE_SCHEMA);
        assert_eq!(
            value["protocol_version"],
            RADROOTS_FARM_WORKSPACE_PROTOCOL_VERSION
        );
        assert_eq!(value["farm"]["d_tag"], "CDEFGHIJKLMNOPQRSTUVWX");
        assert_eq!(value["relays"][0]["mode"], "ReadWrite");
        assert_eq!(value["media_servers"][0]["service"], "RadrootsPrivateMedia");
        assert_eq!(value["supported_kinds"][0], KIND_FARM_CRDT_CHANGE);
        assert_eq!(value["supported_kinds"][1], KIND_FARM_WORKSPACE_MANIFEST);
    }

    fn sample_manifest() -> RadrootsFarmWorkspaceManifest {
        RadrootsFarmWorkspaceManifest {
            d_tag: "ABCDEFGHIJKLMNOPQRSTUV".to_string(),
            schema: RADROOTS_FARM_WORKSPACE_SCHEMA.to_string(),
            farm_group_id: "BCDEFGHIJKLMNOPQRSTUVW".to_string(),
            name: "Small Regen Farm".to_string(),
            owner_pubkey: "owner_pubkey".to_string(),
            farm: Some(RadrootsFarmRef {
                pubkey: "farm_pubkey".to_string(),
                d_tag: "CDEFGHIJKLMNOPQRSTUVWX".to_string(),
            }),
            relays: vec![RadrootsFarmWorkspaceRelay {
                url: "wss://relay.example.invalid/farm/ABCDEFGHIJKLMNOPQRSTUV".to_string(),
                mode: RadrootsFarmWorkspaceRelayMode::ReadWrite,
            }],
            media_servers: vec![RadrootsFarmWorkspaceMediaServer {
                url: "https://media.example.invalid/farm/ABCDEFGHIJKLMNOPQRSTUV".to_string(),
                service: "RadrootsPrivateMedia".to_string(),
            }],
            supported_kinds: vec![
                KIND_FARM_CRDT_CHANGE,
                KIND_FARM_WORKSPACE_MANIFEST,
                KIND_FARM_FILE_METADATA,
                KIND_RELAY_AUTH,
                KIND_HTTP_AUTH,
            ],
            protocol_version: RADROOTS_FARM_WORKSPACE_PROTOCOL_VERSION.to_string(),
            created_at_ms: 1_780_000_000_000,
            updated_at_ms: None,
        }
    }
}
