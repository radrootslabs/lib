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
        kinds::{KIND_FARM, KIND_FARM_FILE_METADATA, KIND_POST},
    };

    use crate::error::{EventEncodeError, EventParseError};
    use crate::farm_workspace::decode::{
        data_from_event, farm_workspace_from_event, parsed_from_event,
    };
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
        assert_eq!(
            decoded.supported_kinds,
            vec![
                KIND_FARM_CRDT_CHANGE,
                KIND_FARM_WORKSPACE_MANIFEST,
                KIND_FARM_FILE_METADATA
            ]
        );
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

    #[test]
    fn farm_workspace_manifest_requires_farm_file_support_for_media_servers() {
        let mut no_file_support = sample_manifest();
        no_file_support.supported_kinds = vec![KIND_FARM_CRDT_CHANGE, KIND_FARM_WORKSPACE_MANIFEST];
        let encode_err = to_wire_parts(&no_file_support).unwrap_err();
        assert!(matches!(
            encode_err,
            EventEncodeError::InvalidField("supported_kinds")
        ));

        let mut no_media = no_file_support.clone();
        no_media.media_servers.clear();
        let parts = to_wire_parts(&no_media).expect("non-media manifest remains valid");
        let decoded = farm_workspace_from_event(parts.kind, &parts.tags, &parts.content)
            .expect("non-media manifest decodes");
        assert!(decoded.media_servers.is_empty());

        let mut content_missing_file_support = sample_manifest();
        content_missing_file_support.supported_kinds =
            vec![KIND_FARM_CRDT_CHANGE, KIND_FARM_WORKSPACE_MANIFEST];
        let content =
            serde_json::to_string(&content_missing_file_support).expect("workspace content");
        let tags = farm_workspace_build_tags(&sample_manifest()).expect("workspace tags");
        let parse_err =
            farm_workspace_from_event(KIND_FARM_WORKSPACE_MANIFEST, &tags, &content).unwrap_err();
        assert!(matches!(
            parse_err,
            EventParseError::InvalidJson("supported_kinds")
        ));
    }

    #[test]
    fn farm_workspace_manifest_wrappers_roundtrip_optional_farm() {
        let mut manifest = sample_manifest();
        manifest.farm = None;
        manifest.media_servers.clear();
        let parts = to_wire_parts(&manifest).expect("workspace wire parts");
        assert!(
            !parts
                .tags
                .iter()
                .any(|tag| tag.first().map(String::as_str) == Some("a"))
        );

        let data = data_from_event(
            "event-id".to_string(),
            "author-pubkey".to_string(),
            99,
            parts.kind,
            parts.content.clone(),
            parts.tags.clone(),
        )
        .expect("parsed data");
        assert_eq!(data.id, "event-id");
        assert_eq!(data.author, "author-pubkey");
        assert_eq!(data.published_at, 99);
        assert_eq!(data.kind, KIND_FARM_WORKSPACE_MANIFEST);
        assert_same_manifest(&data.data, &manifest);

        let parsed = parsed_from_event(
            "event-id".to_string(),
            "author-pubkey".to_string(),
            99,
            parts.kind,
            parts.content,
            parts.tags,
            "sig".to_string(),
        )
        .expect("parsed event");
        assert_eq!(parsed.event.sig, "sig");
        assert_same_manifest(&parsed.data.data, &manifest);
    }

    #[test]
    fn farm_workspace_manifest_rejects_decode_tag_and_content_edges() {
        let parts = to_wire_parts(&sample_manifest()).expect("workspace wire parts");

        let wrong_kind =
            farm_workspace_from_event(KIND_POST, &parts.tags, &parts.content).unwrap_err();
        assert!(matches!(
            wrong_kind,
            EventParseError::InvalidKind {
                expected: "30078",
                got: KIND_POST
            }
        ));

        let empty_content = farm_workspace_from_event(parts.kind, &parts.tags, " ").unwrap_err();
        assert!(matches!(
            empty_content,
            EventParseError::InvalidJson("content")
        ));

        let bad_json = farm_workspace_from_event(parts.kind, &parts.tags, "{").unwrap_err();
        assert!(matches!(bad_json, EventParseError::InvalidJson("content")));

        let mut owner_mismatch = parts.tags.clone();
        replace_first_tag(&mut owner_mismatch, "p", tag("p", "other_owner"));
        let err =
            farm_workspace_from_event(parts.kind, &owner_mismatch, &parts.content).unwrap_err();
        assert!(matches!(err, EventParseError::InvalidTag("p")));

        let mut missing_marker = parts.tags.clone();
        remove_tags(&mut missing_marker, "t");
        let err =
            farm_workspace_from_event(parts.kind, &missing_marker, &parts.content).unwrap_err();
        assert!(matches!(err, EventParseError::MissingTag("t")));

        for replacement in [
            tag(
                "a",
                &format!("{KIND_FARM}:farm_pubkey:AAAAAAAAAAAAAAAAAAAAAg"),
            ),
            tag("a", "30023:farm_pubkey:AAAAAAAAAAAAAAAAAAAAAQ"),
            tag("a", &format!("{KIND_FARM}:farm_pubkey:bad d")),
        ] {
            let mut tags = parts.tags.clone();
            replace_first_tag(&mut tags, "a", replacement);
            let err = farm_workspace_from_event(parts.kind, &tags, &parts.content).unwrap_err();
            assert!(matches!(err, EventParseError::InvalidTag("a")));
        }
    }

    #[test]
    fn farm_workspace_manifest_rejects_content_validation_edges() {
        let parts = to_wire_parts(&sample_manifest()).expect("workspace wire parts");

        for (manifest, expected) in [
            {
                let mut manifest = sample_manifest();
                manifest.farm_group_id.clear();
                (manifest, EventParseError::InvalidTag("h"))
            },
            {
                let mut manifest = sample_manifest();
                manifest.owner_pubkey.clear();
                (manifest, EventParseError::InvalidTag("p"))
            },
            {
                let mut manifest = sample_manifest();
                manifest.relays.clear();
                (manifest, EventParseError::InvalidJson("relays"))
            },
            {
                let mut manifest = sample_manifest();
                manifest.supported_kinds = vec![KIND_FARM_WORKSPACE_MANIFEST];
                (manifest, EventParseError::InvalidJson("supported_kinds"))
            },
            {
                let mut manifest = sample_manifest();
                manifest.name.clear();
                (manifest, EventParseError::InvalidJson("name"))
            },
            {
                let mut manifest = sample_manifest();
                manifest.protocol_version.clear();
                (manifest, EventParseError::InvalidJson("protocol_version"))
            },
            {
                let mut manifest = sample_manifest();
                manifest.relays[0].url.clear();
                (manifest, EventParseError::InvalidJson("relays.url"))
            },
            {
                let mut manifest = sample_manifest();
                manifest.media_servers[0].service.clear();
                (
                    manifest,
                    EventParseError::InvalidJson("media_servers.service"),
                )
            },
            {
                let mut manifest = sample_manifest();
                manifest.farm.as_mut().unwrap().d_tag = "bad d".to_string();
                (manifest, EventParseError::InvalidTag("d"))
            },
        ] {
            let content = serde_json::to_string(&manifest).expect("workspace content");
            let err = farm_workspace_from_event(parts.kind, &parts.tags, &content).unwrap_err();
            assert_same_parse_error(err, expected);
        }
    }

    #[test]
    fn farm_workspace_manifest_rejects_encoder_validation_edges() {
        for (manifest, expected) in [
            {
                let mut manifest = sample_manifest();
                manifest.name.clear();
                (manifest, EventEncodeError::EmptyRequiredField("name"))
            },
            {
                let mut manifest = sample_manifest();
                manifest.owner_pubkey.clear();
                (
                    manifest,
                    EventEncodeError::EmptyRequiredField("owner_pubkey"),
                )
            },
            {
                let mut manifest = sample_manifest();
                manifest.protocol_version.clear();
                (
                    manifest,
                    EventEncodeError::EmptyRequiredField("protocol_version"),
                )
            },
            {
                let mut manifest = sample_manifest();
                manifest.relays[0].url.clear();
                (manifest, EventEncodeError::EmptyRequiredField("relays.url"))
            },
            {
                let mut manifest = sample_manifest();
                manifest.media_servers[0].url.clear();
                (
                    manifest,
                    EventEncodeError::EmptyRequiredField("media_servers.url"),
                )
            },
            {
                let mut manifest = sample_manifest();
                manifest.media_servers[0].service.clear();
                (
                    manifest,
                    EventEncodeError::EmptyRequiredField("media_servers.service"),
                )
            },
            {
                let mut manifest = sample_manifest();
                manifest.farm.as_mut().unwrap().pubkey.clear();
                (
                    manifest,
                    EventEncodeError::EmptyRequiredField("farm.pubkey"),
                )
            },
            {
                let mut manifest = sample_manifest();
                manifest.farm.as_mut().unwrap().d_tag = "bad d".to_string();
                (manifest, EventEncodeError::InvalidField("farm.d_tag"))
            },
        ] {
            let err = farm_workspace_build_tags(&manifest).unwrap_err();
            assert_same_encode_error(err, expected);
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

    fn tag(key: &str, value: &str) -> Vec<String> {
        vec![key.to_string(), value.to_string()]
    }

    fn remove_tags(tags: &mut Vec<Vec<String>>, name: &str) {
        tags.retain(|tag| tag.first().map(String::as_str) != Some(name));
    }

    fn replace_first_tag(tags: &mut [Vec<String>], name: &str, replacement: Vec<String>) {
        let tag = tags
            .iter_mut()
            .find(|tag| tag.first().map(String::as_str) == Some(name))
            .expect("tag");
        *tag = replacement;
    }

    fn assert_same_manifest(
        actual: &RadrootsFarmWorkspaceManifest,
        expected: &RadrootsFarmWorkspaceManifest,
    ) {
        assert_eq!(
            serde_json::to_value(actual).expect("actual manifest value"),
            serde_json::to_value(expected).expect("expected manifest value")
        );
    }

    fn assert_same_parse_error(actual: EventParseError, expected: EventParseError) {
        match (actual, expected) {
            (EventParseError::MissingTag(actual), EventParseError::MissingTag(expected))
            | (EventParseError::InvalidTag(actual), EventParseError::InvalidTag(expected))
            | (EventParseError::InvalidJson(actual), EventParseError::InvalidJson(expected)) => {
                assert_eq!(actual, expected);
            }
            (
                EventParseError::InvalidKind {
                    expected: actual_expected,
                    got: actual_got,
                },
                EventParseError::InvalidKind { expected, got },
            ) => {
                assert_eq!(actual_expected, expected);
                assert_eq!(actual_got, got);
            }
            (
                EventParseError::InvalidNumber(actual, _),
                EventParseError::InvalidNumber(expected, _),
            ) => {
                assert_eq!(actual, expected);
            }
            (actual, expected) => {
                panic!("unexpected parse error {actual:?}, expected {expected:?}")
            }
        }
    }

    fn assert_same_encode_error(actual: EventEncodeError, expected: EventEncodeError) {
        match (actual, expected) {
            (
                EventEncodeError::EmptyRequiredField(actual),
                EventEncodeError::EmptyRequiredField(expected),
            )
            | (EventEncodeError::InvalidField(actual), EventEncodeError::InvalidField(expected)) => {
                assert_eq!(actual, expected);
            }
            (EventEncodeError::InvalidKind(actual), EventEncodeError::InvalidKind(expected)) => {
                assert_eq!(actual, expected);
            }
            (EventEncodeError::Json, EventEncodeError::Json) => {}
            (actual, expected) => {
                panic!("unexpected encode error {actual:?}, expected {expected:?}")
            }
        }
    }
}
