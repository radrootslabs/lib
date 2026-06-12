pub mod encode;

#[cfg(feature = "serde_json")]
pub mod decode;

#[cfg(all(test, feature = "serde_json"))]
mod tests {
    use radroots_events::{
        farm::RadrootsFarmRef,
        farm_crdt::KIND_FARM_CRDT_CHANGE,
        farm_workspace::{
            KIND_FARM_WORKSPACE_MANIFEST, RADROOTS_FARM_WORKSPACE_PROTOCOL_VERSION,
            RADROOTS_FARM_WORKSPACE_SCHEMA, RADROOTS_FARM_WORKSPACE_TAG,
            RadrootsFarmWorkspaceManifest, RadrootsFarmWorkspaceMediaServer,
            RadrootsFarmWorkspaceRelay, RadrootsFarmWorkspaceRelayMode,
        },
        kinds::KIND_POST,
    };

    use crate::error::{EventEncodeError, EventParseError};
    use crate::farm_workspace::decode::farm_workspace_from_event;
    use crate::farm_workspace::encode::{
        farm_workspace_build_tags, to_wire_parts, to_wire_parts_with_kind,
    };

    const D_TAG: &str = "AAAAAAAAAAAAAAAAAAAAAA";
    const FARM_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAAAQ";
    const GROUP_ID: &str = "field-group";

    #[test]
    fn farm_workspace_manifest_encodes_canonical_tags_and_decodes() {
        let manifest = sample_manifest();
        let parts = to_wire_parts(&manifest).expect("workspace wire parts");

        assert_eq!(parts.kind, KIND_FARM_WORKSPACE_MANIFEST);
        assert!(parts.tags.contains(&tag("d", D_TAG)));
        assert!(parts.tags.contains(&tag("h", GROUP_ID)));
        assert!(parts.tags.contains(&tag("p", "workspace_owner_pubkey")));
        assert!(parts.tags.contains(&tag("t", RADROOTS_FARM_WORKSPACE_TAG)));
        assert!(
            parts
                .tags
                .contains(&tag("a", "30340:farm_pubkey:AAAAAAAAAAAAAAAAAAAAAQ"))
        );

        let decoded = farm_workspace_from_event(parts.kind, &parts.tags, &parts.content)
            .expect("workspace decode");
        assert_eq!(decoded.d_tag, D_TAG);
        assert_eq!(decoded.schema, RADROOTS_FARM_WORKSPACE_SCHEMA);
        assert_eq!(decoded.farm_group_id, GROUP_ID);
        assert_eq!(decoded.supported_kinds, vec![KIND_FARM_CRDT_CHANGE, 30078]);
    }

    #[test]
    fn farm_workspace_manifest_rejects_missing_h_and_d_mismatch() {
        let parts = to_wire_parts(&sample_manifest()).expect("workspace wire parts");
        let without_h = parts
            .tags
            .iter()
            .filter(|tag| tag.first().map(|value| value.as_str()) != Some("h"))
            .cloned()
            .collect::<Vec<_>>();

        let missing_h =
            farm_workspace_from_event(parts.kind, &without_h, &parts.content).unwrap_err();
        assert!(matches!(missing_h, EventParseError::MissingTag("h")));

        let mut mismatched_d = parts.tags.clone();
        for tag in mismatched_d.iter_mut() {
            if tag.first().map(|value| value.as_str()) == Some("d") {
                tag[1] = "AAAAAAAAAAAAAAAAAAAAAg".to_string();
            }
        }
        let mismatch =
            farm_workspace_from_event(parts.kind, &mismatched_d, &parts.content).unwrap_err();
        assert!(matches!(mismatch, EventParseError::InvalidTag("d")));
    }

    #[test]
    fn farm_workspace_manifest_rejects_bad_d_tag_kind_and_schema() {
        let mut bad_d_tag = sample_manifest();
        bad_d_tag.d_tag = "bad".to_string();
        let encode_err = farm_workspace_build_tags(&bad_d_tag).unwrap_err();
        assert!(matches!(
            encode_err,
            EventEncodeError::InvalidField("d_tag")
        ));

        let wrong_kind = to_wire_parts_with_kind(&sample_manifest(), KIND_POST).unwrap_err();
        assert!(matches!(
            wrong_kind,
            EventEncodeError::InvalidKind(KIND_POST)
        ));

        let mut bad_schema = sample_manifest();
        bad_schema.schema = "radroots.farm.workspace.invalid".to_string();
        let content = serde_json::to_string(&bad_schema).expect("bad schema content");
        let tags = farm_workspace_build_tags(&sample_manifest()).expect("workspace tags");
        let schema_err =
            farm_workspace_from_event(KIND_FARM_WORKSPACE_MANIFEST, &tags, &content).unwrap_err();
        assert!(matches!(schema_err, EventParseError::InvalidJson("schema")));
    }

    #[test]
    fn farm_workspace_manifest_rejects_missing_field_usage_kinds_and_relays() {
        let mut no_relays = sample_manifest();
        no_relays.relays.clear();
        let relay_err = to_wire_parts(&no_relays).unwrap_err();
        assert!(matches!(
            relay_err,
            EventEncodeError::EmptyRequiredField("relays")
        ));

        let mut unsupported = sample_manifest();
        unsupported.supported_kinds = vec![KIND_FARM_WORKSPACE_MANIFEST];
        let supported_err = to_wire_parts(&unsupported).unwrap_err();
        assert!(matches!(
            supported_err,
            EventEncodeError::InvalidField("supported_kinds")
        ));
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
            supported_kinds: vec![KIND_FARM_CRDT_CHANGE, KIND_FARM_WORKSPACE_MANIFEST],
            protocol_version: RADROOTS_FARM_WORKSPACE_PROTOCOL_VERSION.to_string(),
            created_at_ms: 1_780_000_000_000,
            updated_at_ms: None,
        }
    }

    fn tag(key: &str, value: &str) -> Vec<String> {
        vec![key.to_string(), value.to_string()]
    }
}
