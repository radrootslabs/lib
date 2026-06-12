#![forbid(unsafe_code)]

use crate::farm_workspace::RadrootsFarmWorkspaceRef;
use crate::kinds::KIND_FARM_CRDT_CHANGE as KIND_FARM_CRDT_CHANGE_EVENT;

#[cfg(not(feature = "std"))]
use alloc::{
    string::{String, ToString},
    vec::Vec,
};

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
#[cfg_attr(feature = "serde", serde(from = "String", into = "String"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsFarmCrdtDocumentKind {
    FarmMembership,
    FarmRolePolicy,
    FarmTask,
    FarmWorkSession,
    FarmActivity,
    FarmHarvestRecord,
    FarmLocation,
    FarmCrop,
    FarmCropVariety,
    FarmCropCycle,
    FarmAttachment,
    FarmPayPeriod,
    FarmInventoryItem,
    FarmMediaAsset,
    FarmObservation,
    Other { value: String },
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsCrdtBackend {
    Automerge,
    Yjs,
    Loro,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(from = "String", into = "String"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsFarmSemanticKind {
    FarmTaskCreate,
    FarmTaskAssign,
    FarmTaskJoin,
    FarmTaskStatusSet,
    FarmTaskChecklistItemAdd,
    FarmTaskCommentAdd,
    FarmTaskAttachmentAttach,
    FarmTaskFollowUpCreate,
    FarmTaskUpdate,
    FarmTaskComplete,
    FarmWorkSessionStart,
    FarmWorkSessionStop,
    FarmWorkSessionSubmit,
    FarmWorkSessionManualEntryCreate,
    FarmWorkSessionApprove,
    FarmWorkSessionReject,
    FarmWorkSessionCorrect,
    FarmWorkSessionUpdate,
    FarmWorkSessionEnd,
    FarmHarvestRecordCreate,
    FarmHarvestLineAdd,
    FarmHarvestLineCorrect,
    FarmHarvestLineVoid,
    FarmHarvestAttachmentAttach,
    FarmHarvestRecordUpdate,
    FarmActivityCreate,
    FarmNoteCreate,
    FarmLocationCreate,
    FarmCropCreate,
    FarmCropVarietyCreate,
    FarmCropCycleCreate,
    FarmMemberInviteCreate,
    FarmMemberApprove,
    FarmMemberRoleSet,
    FarmMemberDeactivate,
    FarmPayPeriodOpen,
    FarmPayPeriodClose,
    FarmReportExportMark,
    FarmInventoryItemUpdate,
    FarmMediaAssetAttach,
    FarmObservationCreate,
    FarmWorkspaceUpdate,
    Other { value: String },
}

impl RadrootsFarmCrdtDocumentKind {
    pub fn as_str(&self) -> &str {
        match self {
            Self::FarmMembership => "FarmMembership",
            Self::FarmRolePolicy => "FarmRolePolicy",
            Self::FarmTask => "FarmTask",
            Self::FarmWorkSession => "FarmWorkSession",
            Self::FarmActivity => "FarmActivity",
            Self::FarmHarvestRecord => "FarmHarvestRecord",
            Self::FarmLocation => "FarmLocation",
            Self::FarmCrop => "FarmCrop",
            Self::FarmCropVariety => "FarmCropVariety",
            Self::FarmCropCycle => "FarmCropCycle",
            Self::FarmAttachment => "FarmAttachment",
            Self::FarmPayPeriod => "FarmPayPeriod",
            Self::FarmInventoryItem => "FarmInventoryItem",
            Self::FarmMediaAsset => "FarmMediaAsset",
            Self::FarmObservation => "FarmObservation",
            Self::Other { value } => value.as_str(),
        }
    }
}

impl From<String> for RadrootsFarmCrdtDocumentKind {
    fn from(value: String) -> Self {
        match value.as_str() {
            "FarmMembership" => Self::FarmMembership,
            "FarmRolePolicy" => Self::FarmRolePolicy,
            "FarmTask" => Self::FarmTask,
            "FarmWorkSession" => Self::FarmWorkSession,
            "FarmActivity" => Self::FarmActivity,
            "FarmHarvestRecord" => Self::FarmHarvestRecord,
            "FarmLocation" => Self::FarmLocation,
            "FarmCrop" => Self::FarmCrop,
            "FarmCropVariety" => Self::FarmCropVariety,
            "FarmCropCycle" => Self::FarmCropCycle,
            "FarmAttachment" => Self::FarmAttachment,
            "FarmPayPeriod" => Self::FarmPayPeriod,
            "FarmInventoryItem" => Self::FarmInventoryItem,
            "FarmMediaAsset" => Self::FarmMediaAsset,
            "FarmObservation" => Self::FarmObservation,
            _ => Self::Other { value },
        }
    }
}

impl From<RadrootsFarmCrdtDocumentKind> for String {
    fn from(value: RadrootsFarmCrdtDocumentKind) -> Self {
        match value {
            RadrootsFarmCrdtDocumentKind::Other { value } => value,
            value => value.as_str().to_string(),
        }
    }
}

impl RadrootsFarmSemanticKind {
    pub fn as_str(&self) -> &str {
        match self {
            Self::FarmTaskCreate => "FarmTaskCreate",
            Self::FarmTaskAssign => "FarmTaskAssign",
            Self::FarmTaskJoin => "FarmTaskJoin",
            Self::FarmTaskStatusSet => "FarmTaskStatusSet",
            Self::FarmTaskChecklistItemAdd => "FarmTaskChecklistItemAdd",
            Self::FarmTaskCommentAdd => "FarmTaskCommentAdd",
            Self::FarmTaskAttachmentAttach => "FarmTaskAttachmentAttach",
            Self::FarmTaskFollowUpCreate => "FarmTaskFollowUpCreate",
            Self::FarmTaskUpdate => "FarmTaskUpdate",
            Self::FarmTaskComplete => "FarmTaskComplete",
            Self::FarmWorkSessionStart => "FarmWorkSessionStart",
            Self::FarmWorkSessionStop => "FarmWorkSessionStop",
            Self::FarmWorkSessionSubmit => "FarmWorkSessionSubmit",
            Self::FarmWorkSessionManualEntryCreate => "FarmWorkSessionManualEntryCreate",
            Self::FarmWorkSessionApprove => "FarmWorkSessionApprove",
            Self::FarmWorkSessionReject => "FarmWorkSessionReject",
            Self::FarmWorkSessionCorrect => "FarmWorkSessionCorrect",
            Self::FarmWorkSessionUpdate => "FarmWorkSessionUpdate",
            Self::FarmWorkSessionEnd => "FarmWorkSessionEnd",
            Self::FarmHarvestRecordCreate => "FarmHarvestRecordCreate",
            Self::FarmHarvestLineAdd => "FarmHarvestLineAdd",
            Self::FarmHarvestLineCorrect => "FarmHarvestLineCorrect",
            Self::FarmHarvestLineVoid => "FarmHarvestLineVoid",
            Self::FarmHarvestAttachmentAttach => "FarmHarvestAttachmentAttach",
            Self::FarmHarvestRecordUpdate => "FarmHarvestRecordUpdate",
            Self::FarmActivityCreate => "FarmActivityCreate",
            Self::FarmNoteCreate => "FarmNoteCreate",
            Self::FarmLocationCreate => "FarmLocationCreate",
            Self::FarmCropCreate => "FarmCropCreate",
            Self::FarmCropVarietyCreate => "FarmCropVarietyCreate",
            Self::FarmCropCycleCreate => "FarmCropCycleCreate",
            Self::FarmMemberInviteCreate => "FarmMemberInviteCreate",
            Self::FarmMemberApprove => "FarmMemberApprove",
            Self::FarmMemberRoleSet => "FarmMemberRoleSet",
            Self::FarmMemberDeactivate => "FarmMemberDeactivate",
            Self::FarmPayPeriodOpen => "FarmPayPeriodOpen",
            Self::FarmPayPeriodClose => "FarmPayPeriodClose",
            Self::FarmReportExportMark => "FarmReportExportMark",
            Self::FarmInventoryItemUpdate => "FarmInventoryItemUpdate",
            Self::FarmMediaAssetAttach => "FarmMediaAssetAttach",
            Self::FarmObservationCreate => "FarmObservationCreate",
            Self::FarmWorkspaceUpdate => "FarmWorkspaceUpdate",
            Self::Other { value } => value.as_str(),
        }
    }
}

impl From<String> for RadrootsFarmSemanticKind {
    fn from(value: String) -> Self {
        match value.as_str() {
            "FarmTaskCreate" => Self::FarmTaskCreate,
            "FarmTaskAssign" => Self::FarmTaskAssign,
            "FarmTaskJoin" => Self::FarmTaskJoin,
            "FarmTaskStatusSet" => Self::FarmTaskStatusSet,
            "FarmTaskChecklistItemAdd" => Self::FarmTaskChecklistItemAdd,
            "FarmTaskCommentAdd" => Self::FarmTaskCommentAdd,
            "FarmTaskAttachmentAttach" => Self::FarmTaskAttachmentAttach,
            "FarmTaskFollowUpCreate" => Self::FarmTaskFollowUpCreate,
            "FarmTaskUpdate" => Self::FarmTaskUpdate,
            "FarmTaskComplete" => Self::FarmTaskComplete,
            "FarmWorkSessionStart" => Self::FarmWorkSessionStart,
            "FarmWorkSessionStop" => Self::FarmWorkSessionStop,
            "FarmWorkSessionSubmit" => Self::FarmWorkSessionSubmit,
            "FarmWorkSessionManualEntryCreate" => Self::FarmWorkSessionManualEntryCreate,
            "FarmWorkSessionApprove" => Self::FarmWorkSessionApprove,
            "FarmWorkSessionReject" => Self::FarmWorkSessionReject,
            "FarmWorkSessionCorrect" => Self::FarmWorkSessionCorrect,
            "FarmWorkSessionUpdate" => Self::FarmWorkSessionUpdate,
            "FarmWorkSessionEnd" => Self::FarmWorkSessionEnd,
            "FarmHarvestRecordCreate" => Self::FarmHarvestRecordCreate,
            "FarmHarvestLineAdd" => Self::FarmHarvestLineAdd,
            "FarmHarvestLineCorrect" => Self::FarmHarvestLineCorrect,
            "FarmHarvestLineVoid" => Self::FarmHarvestLineVoid,
            "FarmHarvestAttachmentAttach" => Self::FarmHarvestAttachmentAttach,
            "FarmHarvestRecordUpdate" => Self::FarmHarvestRecordUpdate,
            "FarmActivityCreate" => Self::FarmActivityCreate,
            "FarmNoteCreate" => Self::FarmNoteCreate,
            "FarmLocationCreate" => Self::FarmLocationCreate,
            "FarmCropCreate" => Self::FarmCropCreate,
            "FarmCropVarietyCreate" => Self::FarmCropVarietyCreate,
            "FarmCropCycleCreate" => Self::FarmCropCycleCreate,
            "FarmMemberInviteCreate" => Self::FarmMemberInviteCreate,
            "FarmMemberApprove" => Self::FarmMemberApprove,
            "FarmMemberRoleSet" => Self::FarmMemberRoleSet,
            "FarmMemberDeactivate" => Self::FarmMemberDeactivate,
            "FarmPayPeriodOpen" => Self::FarmPayPeriodOpen,
            "FarmPayPeriodClose" => Self::FarmPayPeriodClose,
            "FarmReportExportMark" => Self::FarmReportExportMark,
            "FarmInventoryItemUpdate" => Self::FarmInventoryItemUpdate,
            "FarmMediaAssetAttach" => Self::FarmMediaAssetAttach,
            "FarmObservationCreate" => Self::FarmObservationCreate,
            "FarmWorkspaceUpdate" => Self::FarmWorkspaceUpdate,
            _ => Self::Other { value },
        }
    }
}

impl From<RadrootsFarmSemanticKind> for String {
    fn from(value: RadrootsFarmSemanticKind) -> Self {
        match value {
            RadrootsFarmSemanticKind::Other { value } => value,
            value => value.as_str().to_string(),
        }
    }
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

    #[test]
    fn document_kinds_serialize_as_stable_strings() {
        for (kind, expected) in [
            (
                RadrootsFarmCrdtDocumentKind::FarmMembership,
                "FarmMembership",
            ),
            (
                RadrootsFarmCrdtDocumentKind::FarmRolePolicy,
                "FarmRolePolicy",
            ),
            (RadrootsFarmCrdtDocumentKind::FarmTask, "FarmTask"),
            (
                RadrootsFarmCrdtDocumentKind::FarmWorkSession,
                "FarmWorkSession",
            ),
            (RadrootsFarmCrdtDocumentKind::FarmActivity, "FarmActivity"),
            (
                RadrootsFarmCrdtDocumentKind::FarmHarvestRecord,
                "FarmHarvestRecord",
            ),
            (RadrootsFarmCrdtDocumentKind::FarmLocation, "FarmLocation"),
            (RadrootsFarmCrdtDocumentKind::FarmCrop, "FarmCrop"),
            (
                RadrootsFarmCrdtDocumentKind::FarmCropVariety,
                "FarmCropVariety",
            ),
            (RadrootsFarmCrdtDocumentKind::FarmCropCycle, "FarmCropCycle"),
            (
                RadrootsFarmCrdtDocumentKind::FarmAttachment,
                "FarmAttachment",
            ),
            (RadrootsFarmCrdtDocumentKind::FarmPayPeriod, "FarmPayPeriod"),
            (
                RadrootsFarmCrdtDocumentKind::FarmInventoryItem,
                "FarmInventoryItem",
            ),
            (
                RadrootsFarmCrdtDocumentKind::FarmMediaAsset,
                "FarmMediaAsset",
            ),
            (
                RadrootsFarmCrdtDocumentKind::FarmObservation,
                "FarmObservation",
            ),
        ] {
            let encoded = serde_json::to_string(&kind).unwrap();
            assert_eq!(encoded, format!("\"{expected}\""));
            let decoded: RadrootsFarmCrdtDocumentKind = serde_json::from_str(&encoded).unwrap();
            assert_eq!(decoded, kind);
        }
    }

    #[test]
    fn semantic_kinds_serialize_as_stable_strings() {
        for (kind, expected) in [
            (RadrootsFarmSemanticKind::FarmTaskCreate, "FarmTaskCreate"),
            (RadrootsFarmSemanticKind::FarmTaskAssign, "FarmTaskAssign"),
            (RadrootsFarmSemanticKind::FarmTaskJoin, "FarmTaskJoin"),
            (
                RadrootsFarmSemanticKind::FarmTaskStatusSet,
                "FarmTaskStatusSet",
            ),
            (
                RadrootsFarmSemanticKind::FarmTaskChecklistItemAdd,
                "FarmTaskChecklistItemAdd",
            ),
            (
                RadrootsFarmSemanticKind::FarmTaskCommentAdd,
                "FarmTaskCommentAdd",
            ),
            (
                RadrootsFarmSemanticKind::FarmTaskAttachmentAttach,
                "FarmTaskAttachmentAttach",
            ),
            (
                RadrootsFarmSemanticKind::FarmTaskFollowUpCreate,
                "FarmTaskFollowUpCreate",
            ),
            (RadrootsFarmSemanticKind::FarmTaskUpdate, "FarmTaskUpdate"),
            (
                RadrootsFarmSemanticKind::FarmTaskComplete,
                "FarmTaskComplete",
            ),
            (
                RadrootsFarmSemanticKind::FarmWorkSessionStart,
                "FarmWorkSessionStart",
            ),
            (
                RadrootsFarmSemanticKind::FarmWorkSessionStop,
                "FarmWorkSessionStop",
            ),
            (
                RadrootsFarmSemanticKind::FarmWorkSessionSubmit,
                "FarmWorkSessionSubmit",
            ),
            (
                RadrootsFarmSemanticKind::FarmWorkSessionManualEntryCreate,
                "FarmWorkSessionManualEntryCreate",
            ),
            (
                RadrootsFarmSemanticKind::FarmWorkSessionApprove,
                "FarmWorkSessionApprove",
            ),
            (
                RadrootsFarmSemanticKind::FarmWorkSessionReject,
                "FarmWorkSessionReject",
            ),
            (
                RadrootsFarmSemanticKind::FarmWorkSessionCorrect,
                "FarmWorkSessionCorrect",
            ),
            (
                RadrootsFarmSemanticKind::FarmWorkSessionUpdate,
                "FarmWorkSessionUpdate",
            ),
            (
                RadrootsFarmSemanticKind::FarmWorkSessionEnd,
                "FarmWorkSessionEnd",
            ),
            (
                RadrootsFarmSemanticKind::FarmHarvestRecordCreate,
                "FarmHarvestRecordCreate",
            ),
            (
                RadrootsFarmSemanticKind::FarmHarvestLineAdd,
                "FarmHarvestLineAdd",
            ),
            (
                RadrootsFarmSemanticKind::FarmHarvestLineCorrect,
                "FarmHarvestLineCorrect",
            ),
            (
                RadrootsFarmSemanticKind::FarmHarvestLineVoid,
                "FarmHarvestLineVoid",
            ),
            (
                RadrootsFarmSemanticKind::FarmHarvestAttachmentAttach,
                "FarmHarvestAttachmentAttach",
            ),
            (
                RadrootsFarmSemanticKind::FarmHarvestRecordUpdate,
                "FarmHarvestRecordUpdate",
            ),
            (
                RadrootsFarmSemanticKind::FarmActivityCreate,
                "FarmActivityCreate",
            ),
            (RadrootsFarmSemanticKind::FarmNoteCreate, "FarmNoteCreate"),
            (
                RadrootsFarmSemanticKind::FarmLocationCreate,
                "FarmLocationCreate",
            ),
            (RadrootsFarmSemanticKind::FarmCropCreate, "FarmCropCreate"),
            (
                RadrootsFarmSemanticKind::FarmCropVarietyCreate,
                "FarmCropVarietyCreate",
            ),
            (
                RadrootsFarmSemanticKind::FarmCropCycleCreate,
                "FarmCropCycleCreate",
            ),
            (
                RadrootsFarmSemanticKind::FarmMemberInviteCreate,
                "FarmMemberInviteCreate",
            ),
            (
                RadrootsFarmSemanticKind::FarmMemberApprove,
                "FarmMemberApprove",
            ),
            (
                RadrootsFarmSemanticKind::FarmMemberRoleSet,
                "FarmMemberRoleSet",
            ),
            (
                RadrootsFarmSemanticKind::FarmMemberDeactivate,
                "FarmMemberDeactivate",
            ),
            (
                RadrootsFarmSemanticKind::FarmPayPeriodOpen,
                "FarmPayPeriodOpen",
            ),
            (
                RadrootsFarmSemanticKind::FarmPayPeriodClose,
                "FarmPayPeriodClose",
            ),
            (
                RadrootsFarmSemanticKind::FarmReportExportMark,
                "FarmReportExportMark",
            ),
            (
                RadrootsFarmSemanticKind::FarmInventoryItemUpdate,
                "FarmInventoryItemUpdate",
            ),
            (
                RadrootsFarmSemanticKind::FarmMediaAssetAttach,
                "FarmMediaAssetAttach",
            ),
            (
                RadrootsFarmSemanticKind::FarmObservationCreate,
                "FarmObservationCreate",
            ),
            (
                RadrootsFarmSemanticKind::FarmWorkspaceUpdate,
                "FarmWorkspaceUpdate",
            ),
        ] {
            let encoded = serde_json::to_string(&kind).unwrap();
            assert_eq!(encoded, format!("\"{expected}\""));
            let decoded: RadrootsFarmSemanticKind = serde_json::from_str(&encoded).unwrap();
            assert_eq!(decoded, kind);
        }
    }

    #[test]
    fn unknown_crdt_kinds_roundtrip_as_other_strings() {
        let document_kind: RadrootsFarmCrdtDocumentKind =
            serde_json::from_str("\"FarmSoilTest\"").unwrap();
        let semantic_kind: RadrootsFarmSemanticKind =
            serde_json::from_str("\"FarmSoilTestCreate\"").unwrap();

        assert_eq!(
            document_kind,
            RadrootsFarmCrdtDocumentKind::Other {
                value: "FarmSoilTest".to_string()
            }
        );
        assert_eq!(
            semantic_kind,
            RadrootsFarmSemanticKind::Other {
                value: "FarmSoilTestCreate".to_string()
            }
        );
        assert_eq!(
            serde_json::to_string(&document_kind).unwrap(),
            "\"FarmSoilTest\""
        );
        assert_eq!(
            serde_json::to_string(&semantic_kind).unwrap(),
            "\"FarmSoilTestCreate\""
        );
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
