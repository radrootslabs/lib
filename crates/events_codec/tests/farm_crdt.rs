#![cfg(feature = "serde_json")]

use radroots_events::{
    farm_crdt::{
        RADROOTS_FARM_CRDT_CHANGE_SCHEMA, RadrootsCrdtBackend, RadrootsFarmCrdtChange,
        RadrootsFarmCrdtDocumentKind, RadrootsFarmSemanticKind,
    },
    farm_workspace::RadrootsFarmWorkspaceRef,
};
use radroots_events_codec::farm_crdt::encode::to_wire_parts;

const WORKSPACE_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAAAA";
const DOCUMENT_ID: &str = "AAAAAAAAAAAAAAAAAAAAAg";

#[test]
fn farm_crdt_change_encodes_without_optional_metadata() {
    let change = RadrootsFarmCrdtChange {
        schema: RADROOTS_FARM_CRDT_CHANGE_SCHEMA.to_string(),
        workspace: RadrootsFarmWorkspaceRef {
            pubkey: "workspace_pubkey".to_string(),
            d_tag: WORKSPACE_D_TAG.to_string(),
        },
        farm_group_id: "field-group".to_string(),
        document_id: DOCUMENT_ID.to_string(),
        document_kind: RadrootsFarmCrdtDocumentKind::FarmTask,
        crdt_backend: RadrootsCrdtBackend::Automerge,
        crdt_backend_version: None,
        actor_id: "actor_abc".to_string(),
        change_hash: "crdt_hash_abc".to_string(),
        dependencies: Vec::new(),
        encoded_change: "abc-DEF_012".to_string(),
        semantic_kind: RadrootsFarmSemanticKind::FarmTaskCreate,
        business_time_ms: 1_780_000_000_000,
        author_member_id: None,
        app_version: None,
    };

    let parts = to_wire_parts(&change).unwrap();
    assert!(parts.content.contains("\"actor_id\":\"actor_abc\""));
    assert!(parts.tags.iter().any(|tag| {
        tag.first().map(String::as_str) == Some("a")
            && tag.get(1).map(String::as_str)
                == Some("30078:workspace_pubkey:AAAAAAAAAAAAAAAAAAAAAA")
    }));
}
