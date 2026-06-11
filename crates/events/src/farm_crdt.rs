#![forbid(unsafe_code)]

use crate::farm_workspace::RadrootsFarmWorkspaceRef;
use crate::kinds::KIND_FARM_CRDT_CHANGE as KIND_FARM_CRDT_CHANGE_EVENT;

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

pub const KIND_FARM_CRDT_CHANGE: u32 = KIND_FARM_CRDT_CHANGE_EVENT;
pub const RADROOTS_FARM_CRDT_CHANGE_SCHEMA: &str = "radroots.farm.crdt.change.v1";
pub const RADROOTS_FARM_CRDT_TAG: &str = "radroots:farm:crdt";

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsFarmCrdtChange {
    pub schema: String,
    pub workspace: RadrootsFarmWorkspaceRef,
    pub farm_group_id: String,
    pub document_id: String,
    pub document_kind: RadrootsFarmCrdtDocumentKind,
    pub crdt_backend: RadrootsCrdtBackend,
    pub crdt_backend_version: Option<String>,
    pub actor_id: String,
    pub change_hash: String,
    pub dependencies: Vec<String>,
    pub encoded_change: String,
    pub semantic_kind: RadrootsFarmSemanticKind,
    pub business_time_ms: u64,
    pub author_member_id: Option<String>,
    pub app_version: Option<String>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsFarmCrdtDocumentKind {
    FarmTask,
    FarmWorkSession,
    FarmHarvestRecord,
    FarmInventoryItem,
    FarmMediaAsset,
    FarmObservation,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsCrdtBackend {
    Automerge,
    Yjs,
    Loro,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsFarmSemanticKind {
    FarmTaskCreate,
    FarmTaskUpdate,
    FarmTaskComplete,
    FarmWorkSessionStart,
    FarmWorkSessionUpdate,
    FarmWorkSessionEnd,
    FarmHarvestRecordCreate,
    FarmHarvestRecordUpdate,
    FarmInventoryItemUpdate,
    FarmMediaAssetAttach,
    FarmObservationCreate,
    FarmWorkspaceUpdate,
}

#[cfg(all(test, feature = "serde"))]
mod tests {
    use super::*;

    #[test]
    fn crdt_change_kind_uses_custom_app_data_kind() {
        assert_eq!(KIND_FARM_CRDT_CHANGE, 78);
    }

    #[test]
    fn crdt_change_represents_required_envelope_fields() {
        let change = sample_change();

        assert_eq!(change.schema, RADROOTS_FARM_CRDT_CHANGE_SCHEMA);
        assert_eq!(change.workspace.pubkey, "workspace_pubkey");
        assert_eq!(change.farm_group_id, "BCDEFGHIJKLMNOPQRSTUVW");
        assert_eq!(change.document_id, "DEFGHIJKLMNOPQRSTUVWXY");
        assert_eq!(change.document_kind, RadrootsFarmCrdtDocumentKind::FarmTask);
        assert_eq!(change.crdt_backend, RadrootsCrdtBackend::Automerge);
        assert_eq!(change.dependencies, Vec::<String>::new());
        assert_eq!(
            change.semantic_kind,
            RadrootsFarmSemanticKind::FarmTaskCreate
        );
        assert_eq!(change.business_time_ms, 1_780_000_000_000);
        assert_eq!(change.author_member_id.as_deref(), Some("member_abc"));
        assert_eq!(change.app_version.as_deref(), Some("0.1.0"));
    }

    #[test]
    fn crdt_change_serializes_stable_content_shape() {
        let value = serde_json::to_value(sample_change()).unwrap();

        assert_eq!(value["schema"], RADROOTS_FARM_CRDT_CHANGE_SCHEMA);
        assert_eq!(value["workspace"]["d_tag"], "ABCDEFGHIJKLMNOPQRSTUV");
        assert_eq!(value["document_kind"], "FarmTask");
        assert_eq!(value["crdt_backend"], "Automerge");
        assert_eq!(value["semantic_kind"], "FarmTaskCreate");
        assert_eq!(value["business_time_ms"], 1_780_000_000_000_u64);
    }

    fn sample_change() -> RadrootsFarmCrdtChange {
        RadrootsFarmCrdtChange {
            schema: RADROOTS_FARM_CRDT_CHANGE_SCHEMA.to_string(),
            workspace: RadrootsFarmWorkspaceRef {
                pubkey: "workspace_pubkey".to_string(),
                d_tag: "ABCDEFGHIJKLMNOPQRSTUV".to_string(),
            },
            farm_group_id: "BCDEFGHIJKLMNOPQRSTUVW".to_string(),
            document_id: "DEFGHIJKLMNOPQRSTUVWXY".to_string(),
            document_kind: RadrootsFarmCrdtDocumentKind::FarmTask,
            crdt_backend: RadrootsCrdtBackend::Automerge,
            crdt_backend_version: Some("0.x".to_string()),
            actor_id: "actor_abc".to_string(),
            change_hash: "crdt_hash_abc".to_string(),
            dependencies: Vec::new(),
            encoded_change: "base64url-encoded-change".to_string(),
            semantic_kind: RadrootsFarmSemanticKind::FarmTaskCreate,
            business_time_ms: 1_780_000_000_000,
            author_member_id: Some("member_abc".to_string()),
            app_version: Some("0.1.0".to_string()),
        }
    }
}
