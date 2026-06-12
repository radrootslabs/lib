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
        farm_crdt_change_from_event, farm_crdt_change_from_event_with_author,
    };
    use crate::farm_crdt::encode::{
        farm_crdt_change_build_tags, to_wire_parts, to_wire_parts_with_author,
        to_wire_parts_with_kind,
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
}
