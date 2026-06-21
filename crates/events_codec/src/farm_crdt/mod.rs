pub mod encode;

#[cfg(feature = "serde_json")]
pub mod decode;

#[cfg(all(test, feature = "serde_json"))]
mod tests {
    use radroots_events::{
        farm_crdt::{
            KIND_FARM_CRDT_CHANGE, RADROOTS_FARM_CRDT_CHANGE_SCHEMA, RADROOTS_FARM_CRDT_TAG,
            RadrootsCrdtBackend, RadrootsFarmCrdtChange, RadrootsFarmCrdtDocumentKind,
            RadrootsFarmSemanticKind,
        },
        farm_workspace::{KIND_FARM_WORKSPACE_MANIFEST, RadrootsFarmWorkspaceRef},
        kinds::KIND_POST,
    };

    use crate::error::{EventEncodeError, EventParseError};
    use crate::farm_crdt::decode::{
        data_from_event, farm_crdt_change_from_event, farm_crdt_change_from_event_with_author,
        parsed_from_event,
    };
    use crate::farm_crdt::encode::{
        farm_crdt_change_build_tags, farm_crdt_change_build_tags_with_author, to_wire_parts,
        to_wire_parts_with_author, to_wire_parts_with_kind, to_wire_parts_with_kind_and_author,
    };

    const WORKSPACE_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAAAA";
    const DOCUMENT_ID: &str = "AAAAAAAAAAAAAAAAAAAAAg";
    const GROUP_ID: &str = "field-group";
    const AUTHOR: &str = "author_pubkey";

    #[test]
    fn farm_crdt_change_encodes_and_decodes_task_change() {
        let change = sample_change();
        let parts = to_wire_parts_with_author(&change, AUTHOR).expect("crdt wire parts");

        assert_eq!(parts.kind, KIND_FARM_CRDT_CHANGE);
        assert!(parts.tags.contains(&tag("h", GROUP_ID)));
        assert!(parts.tags.contains(&tag("d", DOCUMENT_ID)));
        assert!(
            parts
                .tags
                .contains(&tag("a", "30078:workspace_pubkey:AAAAAAAAAAAAAAAAAAAAAA"))
        );
        assert!(parts.tags.contains(&tag("p", AUTHOR)));
        assert!(parts.tags.contains(&tag("t", RADROOTS_FARM_CRDT_TAG)));

        let decoded = farm_crdt_change_from_event_with_author(
            parts.kind,
            &parts.tags,
            &parts.content,
            AUTHOR,
        )
        .expect("crdt decode");
        assert_eq!(decoded.schema, RADROOTS_FARM_CRDT_CHANGE_SCHEMA);
        assert_eq!(decoded.document_id, DOCUMENT_ID);
        assert_eq!(decoded.workspace.d_tag, WORKSPACE_D_TAG);
        assert_eq!(decoded.business_time_ms, 1_780_000_000_000);
    }

    #[test]
    fn farm_crdt_change_roundtrips_representative_mvp_semantics() {
        let cases = vec![
            (
                RadrootsFarmCrdtDocumentKind::FarmTask,
                RadrootsFarmSemanticKind::FarmTaskCreate,
            ),
            (
                RadrootsFarmCrdtDocumentKind::FarmTask,
                RadrootsFarmSemanticKind::FarmTaskStatusSet,
            ),
            (
                RadrootsFarmCrdtDocumentKind::FarmWorkSession,
                RadrootsFarmSemanticKind::FarmWorkSessionStart,
            ),
            (
                RadrootsFarmCrdtDocumentKind::FarmWorkSession,
                RadrootsFarmSemanticKind::FarmWorkSessionStop,
            ),
            (
                RadrootsFarmCrdtDocumentKind::FarmWorkSession,
                RadrootsFarmSemanticKind::FarmWorkSessionSubmit,
            ),
            (
                RadrootsFarmCrdtDocumentKind::FarmWorkSession,
                RadrootsFarmSemanticKind::FarmWorkSessionApprove,
            ),
            (
                RadrootsFarmCrdtDocumentKind::FarmWorkSession,
                RadrootsFarmSemanticKind::FarmWorkSessionReject,
            ),
            (
                RadrootsFarmCrdtDocumentKind::FarmWorkSession,
                RadrootsFarmSemanticKind::FarmWorkSessionCorrect,
            ),
            (
                RadrootsFarmCrdtDocumentKind::FarmHarvestRecord,
                RadrootsFarmSemanticKind::FarmHarvestLineAdd,
            ),
            (
                RadrootsFarmCrdtDocumentKind::FarmHarvestRecord,
                RadrootsFarmSemanticKind::FarmHarvestLineCorrect,
            ),
            (
                RadrootsFarmCrdtDocumentKind::FarmHarvestRecord,
                RadrootsFarmSemanticKind::FarmHarvestLineVoid,
            ),
            (
                RadrootsFarmCrdtDocumentKind::FarmMembership,
                RadrootsFarmSemanticKind::FarmMemberInviteCreate,
            ),
            (
                RadrootsFarmCrdtDocumentKind::FarmMembership,
                RadrootsFarmSemanticKind::FarmMemberApprove,
            ),
            (
                RadrootsFarmCrdtDocumentKind::FarmMembership,
                RadrootsFarmSemanticKind::FarmMemberRoleSet,
            ),
            (
                RadrootsFarmCrdtDocumentKind::FarmPayPeriod,
                RadrootsFarmSemanticKind::FarmPayPeriodClose,
            ),
            (
                RadrootsFarmCrdtDocumentKind::FarmPayPeriod,
                RadrootsFarmSemanticKind::FarmReportExportMark,
            ),
        ];

        for (index, (document_kind, semantic_kind)) in cases.into_iter().enumerate() {
            let document_id = document_id(index);
            let change = sample_change_with(document_id.as_str(), document_kind, semantic_kind);
            let parts = to_wire_parts_with_author(&change, AUTHOR).expect("crdt wire parts");
            let decoded = farm_crdt_change_from_event_with_author(
                parts.kind,
                &parts.tags,
                &parts.content,
                AUTHOR,
            )
            .expect("crdt decode");

            assert_eq!(decoded.document_id, document_id);
            assert_eq!(decoded.document_kind, change.document_kind);
            assert_eq!(decoded.semantic_kind, change.semantic_kind);
        }
    }

    #[test]
    fn farm_crdt_change_rejects_missing_t_and_d_mismatch() {
        let parts = to_wire_parts(&sample_change()).expect("crdt wire parts");
        let without_t = parts
            .tags
            .iter()
            .filter(|tag| tag.first().map(|value| value.as_str()) != Some("t"))
            .cloned()
            .collect::<Vec<_>>();

        let missing_t =
            farm_crdt_change_from_event(parts.kind, &without_t, &parts.content).unwrap_err();
        assert!(matches!(missing_t, EventParseError::MissingTag("t")));

        let mut mismatched_d = parts.tags.clone();
        for tag in mismatched_d.iter_mut() {
            if tag.first().map(|value| value.as_str()) == Some("d") {
                tag[1] = WORKSPACE_D_TAG.to_string();
            }
        }
        let mismatch =
            farm_crdt_change_from_event(parts.kind, &mismatched_d, &parts.content).unwrap_err();
        assert!(matches!(mismatch, EventParseError::InvalidTag("d")));
    }

    #[test]
    fn farm_crdt_change_rejects_bad_workspace_address_and_author() {
        let parts = to_wire_parts_with_author(&sample_change(), AUTHOR).expect("crdt wire parts");
        let mut bad_workspace = parts.tags.clone();
        for tag in bad_workspace.iter_mut() {
            if tag.first().map(|value| value.as_str()) == Some("a") {
                tag[1] = format!("{KIND_FARM_WORKSPACE_MANIFEST}:workspace_pubkey:bad");
            }
        }
        let workspace_err =
            farm_crdt_change_from_event(parts.kind, &bad_workspace, &parts.content).unwrap_err();
        assert!(matches!(workspace_err, EventParseError::InvalidTag("a")));

        let author_err = farm_crdt_change_from_event_with_author(
            parts.kind,
            &parts.tags,
            &parts.content,
            "other_author",
        )
        .unwrap_err();
        assert!(matches!(author_err, EventParseError::InvalidTag("p")));
    }

    #[test]
    fn farm_crdt_change_rejects_bad_encoded_change_missing_h_and_kind() {
        let mut bad_change = sample_change();
        bad_change.encoded_change = "abc/def".to_string();
        let encode_err = farm_crdt_change_build_tags(&bad_change).unwrap_err();
        assert!(matches!(
            encode_err,
            EventEncodeError::InvalidField("encoded_change")
        ));

        let parts = to_wire_parts(&sample_change()).expect("crdt wire parts");
        let without_h = parts
            .tags
            .iter()
            .filter(|tag| tag.first().map(|value| value.as_str()) != Some("h"))
            .cloned()
            .collect::<Vec<_>>();
        let missing_h =
            farm_crdt_change_from_event(parts.kind, &without_h, &parts.content).unwrap_err();
        assert!(matches!(missing_h, EventParseError::MissingTag("h")));

        let wrong_kind = to_wire_parts_with_kind(&sample_change(), KIND_POST).unwrap_err();
        assert!(matches!(
            wrong_kind,
            EventEncodeError::InvalidKind(KIND_POST)
        ));

        let decode_wrong_kind =
            farm_crdt_change_from_event(KIND_POST, &parts.tags, &parts.content).unwrap_err();
        assert!(matches!(
            decode_wrong_kind,
            EventParseError::InvalidKind {
                expected: "78",
                got: KIND_POST
            }
        ));
    }

    #[test]
    fn farm_crdt_change_rejects_zero_business_time_and_schema_mismatch() {
        let mut zero_time = sample_change();
        zero_time.business_time_ms = 0;
        let zero_err = to_wire_parts(&zero_time).unwrap_err();
        assert!(matches!(
            zero_err,
            EventEncodeError::InvalidField("business_time_ms")
        ));

        let parts = to_wire_parts(&sample_change()).expect("crdt wire parts");
        let mut bad_schema = sample_change();
        bad_schema.schema = "radroots.farm.crdt.invalid".to_string();
        let content = serde_json::to_string(&bad_schema).expect("bad schema content");
        let schema_err =
            farm_crdt_change_from_event(parts.kind, &parts.tags, &content).unwrap_err();
        assert!(matches!(schema_err, EventParseError::InvalidJson("schema")));
    }

    #[test]
    fn farm_crdt_change_wrappers_preserve_event_metadata() {
        let change = sample_change();
        let parts = to_wire_parts_with_author(&change, AUTHOR).expect("crdt wire parts");

        let data = data_from_event(
            "event-id".to_string(),
            AUTHOR.to_string(),
            99,
            parts.kind,
            parts.content.clone(),
            parts.tags.clone(),
        )
        .expect("parsed data");
        assert_eq!(data.id, "event-id");
        assert_eq!(data.author, AUTHOR);
        assert_eq!(data.published_at, 99);
        assert_eq!(data.kind, KIND_FARM_CRDT_CHANGE);
        assert_eq!(data.data, change);

        let parsed = parsed_from_event(
            "event-id".to_string(),
            AUTHOR.to_string(),
            99,
            parts.kind,
            parts.content,
            parts.tags,
            "sig".to_string(),
        )
        .expect("parsed event");
        assert_eq!(parsed.event.sig, "sig");
        assert_eq!(parsed.data.data, change);

        let no_author_parts = to_wire_parts(&change).expect("crdt wire parts");
        let decoded = farm_crdt_change_from_event_with_author(
            no_author_parts.kind,
            &no_author_parts.tags,
            &no_author_parts.content,
            AUTHOR,
        )
        .expect("author context without p tag remains valid");
        assert_eq!(decoded, change);

        let empty_author = farm_crdt_change_from_event_with_author(
            no_author_parts.kind,
            &no_author_parts.tags,
            &no_author_parts.content,
            " ",
        )
        .unwrap_err();
        assert!(matches!(empty_author, EventParseError::InvalidTag("p")));
    }

    #[test]
    fn farm_crdt_change_rejects_decode_tag_and_content_edges() {
        let parts = to_wire_parts_with_author(&sample_change(), AUTHOR).expect("crdt wire parts");

        let empty_content = farm_crdt_change_from_event(parts.kind, &parts.tags, " ").unwrap_err();
        assert!(matches!(
            empty_content,
            EventParseError::InvalidJson("content")
        ));

        let bad_json = farm_crdt_change_from_event(parts.kind, &parts.tags, "{").unwrap_err();
        assert!(matches!(bad_json, EventParseError::InvalidJson("content")));

        let mut empty_author_tag = parts.tags.clone();
        replace_first_tag(&mut empty_author_tag, "p", tag("p", " "));
        let err = farm_crdt_change_from_event_with_author(
            parts.kind,
            &empty_author_tag,
            &parts.content,
            AUTHOR,
        )
        .unwrap_err();
        assert!(matches!(err, EventParseError::InvalidTag("p")));

        let mut bad_document_tag = parts.tags.clone();
        replace_first_tag(&mut bad_document_tag, "d", tag("d", "bad"));
        let err =
            farm_crdt_change_from_event(parts.kind, &bad_document_tag, &parts.content).unwrap_err();
        assert!(matches!(err, EventParseError::InvalidTag("d")));

        for replacement in [
            tag("a", "30023:workspace_pubkey:AAAAAAAAAAAAAAAAAAAAAA"),
            tag("a", "30078::AAAAAAAAAAAAAAAAAAAAAA"),
            tag("a", "30078:workspace_pubkey:bad d"),
            tag("a", "30078:workspace_pubkey:AAAAAAAAAAAAAAAAAAAAAA:extra"),
        ] {
            let mut tags = parts.tags.clone();
            replace_first_tag(&mut tags, "a", replacement);
            let err = farm_crdt_change_from_event(parts.kind, &tags, &parts.content).unwrap_err();
            assert!(matches!(err, EventParseError::InvalidTag("a")));
        }

        let mut wrong_marker = parts.tags.clone();
        remove_tags(&mut wrong_marker, "t");
        wrong_marker.push(tag("t", "radroots:farm:other"));
        let err =
            farm_crdt_change_from_event(parts.kind, &wrong_marker, &parts.content).unwrap_err();
        assert!(matches!(err, EventParseError::MissingTag("t")));

        let mut group_mismatch = parts.tags.clone();
        replace_first_tag(&mut group_mismatch, "h", tag("h", "other-group"));
        let err =
            farm_crdt_change_from_event(parts.kind, &group_mismatch, &parts.content).unwrap_err();
        assert!(matches!(err, EventParseError::InvalidTag("h")));
    }

    #[test]
    fn farm_crdt_change_rejects_content_validation_edges() {
        let parts = to_wire_parts(&sample_change()).expect("crdt wire parts");

        for (change, expected) in [
            {
                let mut change = sample_change();
                change.farm_group_id.clear();
                (change, EventParseError::InvalidTag("h"))
            },
            {
                let mut change = sample_change();
                change.document_id = "bad".to_string();
                (change, EventParseError::InvalidTag("d"))
            },
            {
                let mut change = sample_change();
                change.workspace.pubkey.clear();
                (change, EventParseError::InvalidTag("a"))
            },
            {
                let mut change = sample_change();
                change.workspace.d_tag = "bad".to_string();
                (change, EventParseError::InvalidTag("a"))
            },
            {
                let mut change = sample_change();
                change.encoded_change = "abc/def".to_string();
                (change, EventParseError::InvalidJson("encoded_change"))
            },
            {
                let mut change = sample_change();
                change.change_hash.clear();
                (change, EventParseError::InvalidJson("change_hash"))
            },
            {
                let mut change = sample_change();
                change.business_time_ms = 0;
                (change, EventParseError::InvalidJson("business_time_ms"))
            },
            {
                let mut change = sample_change();
                change.actor_id.clear();
                (change, EventParseError::InvalidJson("actor_id"))
            },
            {
                let mut change = sample_change();
                change.dependencies.push(String::new());
                (change, EventParseError::InvalidJson("dependencies"))
            },
            {
                let mut change = sample_change();
                change.crdt_backend_version = Some(" ".to_string());
                (change, EventParseError::InvalidJson("crdt_backend_version"))
            },
            {
                let mut change = sample_change();
                change.author_member_id = Some(" ".to_string());
                (change, EventParseError::InvalidJson("author_member_id"))
            },
            {
                let mut change = sample_change();
                change.app_version = Some(" ".to_string());
                (change, EventParseError::InvalidJson("app_version"))
            },
        ] {
            let content = serde_json::to_string(&change).expect("crdt content");
            let err = farm_crdt_change_from_event(parts.kind, &parts.tags, &content).unwrap_err();
            assert_same_parse_error(err, expected);
        }
    }

    #[test]
    fn farm_crdt_change_rejects_encoder_validation_edges() {
        for (change, expected) in [
            {
                let mut change = sample_change();
                change.schema = "radroots.farm.crdt.invalid".to_string();
                (change, EventEncodeError::InvalidField("schema"))
            },
            {
                let mut change = sample_change();
                change.farm_group_id.clear();
                (
                    change,
                    EventEncodeError::EmptyRequiredField("farm_group_id"),
                )
            },
            {
                let mut change = sample_change();
                change.document_id = "bad".to_string();
                (change, EventEncodeError::InvalidField("document_id"))
            },
            {
                let mut change = sample_change();
                change.workspace.pubkey.clear();
                (
                    change,
                    EventEncodeError::EmptyRequiredField("workspace.pubkey"),
                )
            },
            {
                let mut change = sample_change();
                change.workspace.d_tag = "bad".to_string();
                (change, EventEncodeError::InvalidField("workspace.d_tag"))
            },
            {
                let mut change = sample_change();
                change.actor_id.clear();
                (change, EventEncodeError::EmptyRequiredField("actor_id"))
            },
            {
                let mut change = sample_change();
                change.change_hash.clear();
                (change, EventEncodeError::EmptyRequiredField("change_hash"))
            },
            {
                let mut change = sample_change();
                change.dependencies.push(String::new());
                (change, EventEncodeError::EmptyRequiredField("dependencies"))
            },
            {
                let mut change = sample_change();
                change.encoded_change = "abc/def".to_string();
                (change, EventEncodeError::InvalidField("encoded_change"))
            },
            {
                let mut change = sample_change();
                change.business_time_ms = 0;
                (change, EventEncodeError::InvalidField("business_time_ms"))
            },
            {
                let mut change = sample_change();
                change.crdt_backend_version = Some(" ".to_string());
                (
                    change,
                    EventEncodeError::EmptyRequiredField("crdt_backend_version"),
                )
            },
            {
                let mut change = sample_change();
                change.author_member_id = Some(" ".to_string());
                (
                    change,
                    EventEncodeError::EmptyRequiredField("author_member_id"),
                )
            },
            {
                let mut change = sample_change();
                change.app_version = Some(" ".to_string());
                (change, EventEncodeError::EmptyRequiredField("app_version"))
            },
        ] {
            let err = farm_crdt_change_build_tags(&change).unwrap_err();
            assert_same_encode_error(err, expected);
        }

        let author_err =
            farm_crdt_change_build_tags_with_author(&sample_change(), Some(" ")).unwrap_err();
        assert_same_encode_error(
            author_err,
            EventEncodeError::EmptyRequiredField("author_pubkey"),
        );

        let wrong_kind =
            to_wire_parts_with_kind_and_author(&sample_change(), KIND_POST, Some(AUTHOR))
                .unwrap_err();
        assert_same_encode_error(wrong_kind, EventEncodeError::InvalidKind(KIND_POST));
    }

    fn sample_change() -> RadrootsFarmCrdtChange {
        sample_change_with(
            DOCUMENT_ID,
            RadrootsFarmCrdtDocumentKind::FarmTask,
            RadrootsFarmSemanticKind::FarmTaskCreate,
        )
    }

    fn sample_change_with(
        document_id: &str,
        document_kind: RadrootsFarmCrdtDocumentKind,
        semantic_kind: RadrootsFarmSemanticKind,
    ) -> RadrootsFarmCrdtChange {
        RadrootsFarmCrdtChange {
            schema: RADROOTS_FARM_CRDT_CHANGE_SCHEMA.to_string(),
            workspace: RadrootsFarmWorkspaceRef {
                pubkey: "workspace_pubkey".to_string(),
                d_tag: WORKSPACE_D_TAG.to_string(),
            },
            farm_group_id: GROUP_ID.to_string(),
            document_id: document_id.to_string(),
            document_kind,
            crdt_backend: RadrootsCrdtBackend::Automerge,
            crdt_backend_version: Some("0.x".to_string()),
            actor_id: "actor_abc".to_string(),
            change_hash: "crdt_hash_abc".to_string(),
            dependencies: Vec::new(),
            encoded_change: "abc-DEF_012".to_string(),
            semantic_kind,
            business_time_ms: 1_780_000_000_000,
            author_member_id: Some("member_abc".to_string()),
            app_version: Some("0.1.0".to_string()),
        }
    }

    fn document_id(index: usize) -> String {
        format!("{index:02}AAAAAAAAAAAAAAAAAAAA")
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
