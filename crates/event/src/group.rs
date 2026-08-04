#![forbid(unsafe_code)]

use crate::envelope::kind::{
    KIND_GROUP_ADMINS as KIND_GROUP_ADMINS_EVENT,
    KIND_GROUP_CREATE_GROUP as KIND_GROUP_CREATE_GROUP_EVENT,
    KIND_GROUP_CREATE_INVITE as KIND_GROUP_CREATE_INVITE_EVENT,
    KIND_GROUP_DELETE_EVENT as KIND_GROUP_DELETE_EVENT_EVENT,
    KIND_GROUP_DELETE_GROUP as KIND_GROUP_DELETE_GROUP_EVENT,
    KIND_GROUP_EDIT_METADATA as KIND_GROUP_EDIT_METADATA_EVENT,
    KIND_GROUP_JOIN_REQUEST as KIND_GROUP_JOIN_REQUEST_EVENT,
    KIND_GROUP_LEAVE_REQUEST as KIND_GROUP_LEAVE_REQUEST_EVENT,
    KIND_GROUP_MEMBERS as KIND_GROUP_MEMBERS_EVENT,
    KIND_GROUP_METADATA as KIND_GROUP_METADATA_EVENT,
    KIND_GROUP_PUT_USER as KIND_GROUP_PUT_USER_EVENT,
    KIND_GROUP_REMOVE_USER as KIND_GROUP_REMOVE_USER_EVENT,
    KIND_GROUP_ROLES as KIND_GROUP_ROLES_EVENT,
};

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

pub const KIND_GROUP_PUT_USER: u32 = KIND_GROUP_PUT_USER_EVENT;
pub const KIND_GROUP_REMOVE_USER: u32 = KIND_GROUP_REMOVE_USER_EVENT;
pub const KIND_GROUP_EDIT_METADATA: u32 = KIND_GROUP_EDIT_METADATA_EVENT;
pub const KIND_GROUP_DELETE_EVENT: u32 = KIND_GROUP_DELETE_EVENT_EVENT;
pub const KIND_GROUP_CREATE_GROUP: u32 = KIND_GROUP_CREATE_GROUP_EVENT;
pub const KIND_GROUP_DELETE_GROUP: u32 = KIND_GROUP_DELETE_GROUP_EVENT;
pub const KIND_GROUP_CREATE_INVITE: u32 = KIND_GROUP_CREATE_INVITE_EVENT;
pub const KIND_GROUP_JOIN_REQUEST: u32 = KIND_GROUP_JOIN_REQUEST_EVENT;
pub const KIND_GROUP_LEAVE_REQUEST: u32 = KIND_GROUP_LEAVE_REQUEST_EVENT;
pub const KIND_GROUP_METADATA: u32 = KIND_GROUP_METADATA_EVENT;
pub const KIND_GROUP_ADMINS: u32 = KIND_GROUP_ADMINS_EVENT;
pub const KIND_GROUP_MEMBERS: u32 = KIND_GROUP_MEMBERS_EVENT;
pub const KIND_GROUP_ROLES: u32 = KIND_GROUP_ROLES_EVENT;

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupPutUser {
    pub group_id: String,
    pub message: Option<String>,
    pub pubkey: String,
    pub roles: Vec<String>,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupRemoveUser {
    pub group_id: String,
    pub message: Option<String>,
    pub pubkey: String,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupCreateGroup {
    pub group_id: String,
    pub message: Option<String>,
    pub metadata: GroupEditableMetadata,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupEditMetadata {
    pub group_id: String,
    pub message: Option<String>,
    pub metadata: GroupEditableMetadata,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupDeleteGroup {
    pub group_id: String,
    pub message: Option<String>,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupDeleteEvent {
    pub group_id: String,
    pub message: Option<String>,
    pub event_id: String,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupCreateInvite {
    pub group_id: String,
    pub message: Option<String>,
    pub code: String,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupJoinRequest {
    pub group_id: String,
    pub message: Option<String>,
    pub code: Option<String>,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupLeaveRequest {
    pub group_id: String,
    pub message: Option<String>,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupMetadata {
    pub d_tag: String,
    pub metadata: GroupEditableMetadata,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupAdmins {
    pub d_tag: String,
    pub description: Option<String>,
    pub admins: Vec<GroupUserRef>,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupMembers {
    pub d_tag: String,
    pub description: Option<String>,
    pub members: Vec<GroupUserRef>,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupRoles {
    pub d_tag: String,
    pub description: Option<String>,
    pub roles: Vec<GroupRole>,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GroupEditableMetadata {
    pub name: Option<String>,
    pub about: Option<String>,
    pub picture: Option<String>,
    pub is_private: bool,
    pub is_restricted: bool,
    pub is_closed: bool,
    pub is_hidden: bool,
    pub supported_kinds: Option<Vec<u32>>,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupUserRef {
    pub pubkey: String,
    pub roles: Vec<String>,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupRole {
    pub name: String,
    pub description: Option<String>,
    pub permissions: Vec<String>,
}

#[cfg(all(test, feature = "serde"))]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    #[test]
    fn group_user_and_moderation_models_use_h_group_id_semantics() {
        let put = GroupPutUser {
            group_id: "field-group".to_string(),
            message: Some("add member".to_string()),
            pubkey: "member_pubkey".to_string(),
            roles: vec!["member".to_string()],
        };
        let delete = GroupDeleteEvent {
            group_id: "field-group".to_string(),
            message: Some("remove duplicate event".to_string()),
            event_id: "event_id".to_string(),
        };
        let join = GroupJoinRequest {
            group_id: "field-group".to_string(),
            message: Some("requesting access".to_string()),
            code: Some("invite-code".to_string()),
        };

        assert_eq!(put.group_id, "field-group");
        assert_eq!(delete.group_id, "field-group");
        assert_eq!(join.group_id, "field-group");
        assert_eq!(KIND_GROUP_PUT_USER, 9000);
        assert_eq!(KIND_GROUP_DELETE_EVENT, 9005);
        assert_eq!(KIND_GROUP_JOIN_REQUEST, 9021);
    }

    #[test]
    fn group_metadata_and_lists_use_d_tag_semantics() {
        let metadata = GroupMetadata {
            d_tag: "field-group".to_string(),
            metadata: sample_metadata(),
        };
        let members = GroupMembers {
            d_tag: "field-group".to_string(),
            description: Some("group members".to_string()),
            members: vec![sample_user_ref()],
        };
        let roles = GroupRoles {
            d_tag: "field-group".to_string(),
            description: Some("group roles".to_string()),
            roles: vec![GroupRole {
                name: "member".to_string(),
                description: Some("can read and write group events".to_string()),
                permissions: vec!["read".to_string(), "write".to_string()],
            }],
        };

        assert_eq!(metadata.d_tag, "field-group");
        assert_eq!(members.d_tag, "field-group");
        assert_eq!(roles.d_tag, "field-group");
        assert_eq!(KIND_GROUP_METADATA, 39000);
        assert_eq!(KIND_GROUP_MEMBERS, 39002);
        assert_eq!(KIND_GROUP_ROLES, 39003);
    }

    #[test]
    fn group_models_are_infrastructure_not_field_business_authorization() {
        let admins = GroupAdmins {
            d_tag: "field-group".to_string(),
            description: Some("group admins".to_string()),
            admins: vec![sample_user_ref()],
        };

        assert_eq!(admins.admins[0].roles, vec!["admin".to_string()]);
        assert_eq!(KIND_GROUP_ADMINS, 39001);
    }

    #[test]
    fn group_models_serialize_stable_shapes() {
        let create = GroupCreateGroup {
            group_id: "field-group".to_string(),
            message: None,
            metadata: sample_metadata(),
        };
        let invite = GroupCreateInvite {
            group_id: "field-group".to_string(),
            message: Some("join the field group".to_string()),
            code: "invite-code".to_string(),
        };

        let create_value = serde_json::to_value(create).unwrap();
        let invite_value = serde_json::to_value(invite).unwrap();

        assert_eq!(create_value["group_id"], "field-group");
        assert_eq!(create_value["metadata"]["name"], "Small Regen Farm");
        assert_eq!(invite_value["code"], "invite-code");
        assert_eq!(invite_value["message"], "join the field group");
        assert_eq!(KIND_GROUP_CREATE_GROUP, 9007);
        assert_eq!(KIND_GROUP_CREATE_INVITE, 9009);
    }

    fn sample_metadata() -> GroupEditableMetadata {
        GroupEditableMetadata {
            name: Some("Small Regen Farm".to_string()),
            about: Some("Field app group".to_string()),
            picture: Some("https://media.example.invalid/group.png".to_string()),
            is_private: false,
            is_restricted: true,
            is_closed: false,
            is_hidden: false,
            supported_kinds: Some(vec![78, 30078]),
        }
    }

    fn sample_user_ref() -> GroupUserRef {
        GroupUserRef {
            pubkey: "admin_pubkey".to_string(),
            roles: vec!["admin".to_string()],
        }
    }
}
