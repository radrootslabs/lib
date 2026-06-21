use radroots_events::group::{
    KIND_GROUP_ADMINS, KIND_GROUP_CREATE_GROUP, KIND_GROUP_CREATE_INVITE, KIND_GROUP_DELETE_EVENT,
    KIND_GROUP_DELETE_GROUP, KIND_GROUP_EDIT_METADATA, KIND_GROUP_JOIN_REQUEST,
    KIND_GROUP_LEAVE_REQUEST, KIND_GROUP_MEMBERS, KIND_GROUP_METADATA, KIND_GROUP_PUT_USER,
    KIND_GROUP_REMOVE_USER, KIND_GROUP_ROLES, RadrootsGroupAdmins, RadrootsGroupCreateGroup,
    RadrootsGroupCreateInvite, RadrootsGroupDeleteEvent, RadrootsGroupDeleteGroup,
    RadrootsGroupEditMetadata, RadrootsGroupEditableMetadata, RadrootsGroupJoinRequest,
    RadrootsGroupLeaveRequest, RadrootsGroupMembers, RadrootsGroupMetadata, RadrootsGroupPutUser,
    RadrootsGroupRemoveUser, RadrootsGroupRole, RadrootsGroupRoles, RadrootsGroupUserRef,
};
use radroots_events_codec::error::{EventEncodeError, EventParseError};
use radroots_events_codec::group::decode::{
    group_admins_from_event, group_create_group_from_event, group_create_invite_from_event,
    group_delete_event_from_event, group_delete_group_from_event, group_edit_metadata_from_event,
    group_join_request_from_event, group_leave_request_from_event, group_members_from_event,
    group_metadata_from_event, group_put_user_from_event, group_remove_user_from_event,
    group_roles_from_event,
};
use radroots_events_codec::group::encode::{
    group_admins_to_wire_parts, group_create_group_to_wire_parts,
    group_create_invite_to_wire_parts, group_delete_event_to_wire_parts,
    group_delete_group_to_wire_parts, group_edit_metadata_to_wire_parts,
    group_join_request_to_wire_parts, group_leave_request_to_wire_parts,
    group_members_to_wire_parts, group_metadata_to_wire_parts, group_put_user_to_wire_parts,
    group_remove_user_to_wire_parts, group_roles_to_wire_parts,
};

const GROUP_ID: &str = "field-group";
const PUBKEY: &str = "member_pubkey";

#[test]
fn group_public_codecs_roundtrip_all_group_wire_shapes() {
    let put = RadrootsGroupPutUser {
        group_id: GROUP_ID.to_string(),
        message: None,
        pubkey: PUBKEY.to_string(),
        roles: Vec::new(),
    };
    let remove = RadrootsGroupRemoveUser {
        group_id: GROUP_ID.to_string(),
        message: Some("remove member".to_string()),
        pubkey: PUBKEY.to_string(),
    };
    let create = RadrootsGroupCreateGroup {
        group_id: GROUP_ID.to_string(),
        message: Some("create group".to_string()),
        metadata: full_metadata(),
    };
    let edit = RadrootsGroupEditMetadata {
        group_id: GROUP_ID.to_string(),
        message: None,
        metadata: minimal_metadata(),
    };
    let delete_group = RadrootsGroupDeleteGroup {
        group_id: GROUP_ID.to_string(),
        message: None,
    };
    let delete_event = RadrootsGroupDeleteEvent {
        group_id: GROUP_ID.to_string(),
        message: Some("delete event".to_string()),
        event_id: "event_id".to_string(),
    };
    let invite = RadrootsGroupCreateInvite {
        group_id: GROUP_ID.to_string(),
        message: None,
        code: "invite-code".to_string(),
    };
    let join = RadrootsGroupJoinRequest {
        group_id: GROUP_ID.to_string(),
        message: None,
        code: None,
    };
    let leave = RadrootsGroupLeaveRequest {
        group_id: GROUP_ID.to_string(),
        message: None,
    };
    let metadata = RadrootsGroupMetadata {
        d_tag: GROUP_ID.to_string(),
        metadata: minimal_metadata(),
    };
    let admins = RadrootsGroupAdmins {
        d_tag: GROUP_ID.to_string(),
        description: None,
        admins: vec![
            RadrootsGroupUserRef {
                pubkey: "admin_pubkey".to_string(),
                roles: vec!["admin".to_string()],
            },
            RadrootsGroupUserRef {
                pubkey: "observer_pubkey".to_string(),
                roles: Vec::new(),
            },
        ],
    };
    let members = RadrootsGroupMembers {
        d_tag: GROUP_ID.to_string(),
        description: Some("group members".to_string()),
        members: vec![RadrootsGroupUserRef {
            pubkey: PUBKEY.to_string(),
            roles: vec!["member".to_string()],
        }],
    };
    let roles = RadrootsGroupRoles {
        d_tag: GROUP_ID.to_string(),
        description: None,
        roles: vec![
            RadrootsGroupRole {
                name: "admin".to_string(),
                description: Some("full access".to_string()),
                permissions: vec!["read".to_string(), "write".to_string()],
            },
            RadrootsGroupRole {
                name: "viewer".to_string(),
                description: None,
                permissions: Vec::new(),
            },
        ],
    };

    let put_parts = group_put_user_to_wire_parts(&put).unwrap();
    let remove_parts = group_remove_user_to_wire_parts(&remove).unwrap();
    let create_parts = group_create_group_to_wire_parts(&create).unwrap();
    let edit_parts = group_edit_metadata_to_wire_parts(&edit).unwrap();
    let delete_group_parts = group_delete_group_to_wire_parts(&delete_group).unwrap();
    let delete_event_parts = group_delete_event_to_wire_parts(&delete_event).unwrap();
    let invite_parts = group_create_invite_to_wire_parts(&invite).unwrap();
    let join_parts = group_join_request_to_wire_parts(&join).unwrap();
    let leave_parts = group_leave_request_to_wire_parts(&leave).unwrap();
    let metadata_parts = group_metadata_to_wire_parts(&metadata).unwrap();
    let admins_parts = group_admins_to_wire_parts(&admins).unwrap();
    let members_parts = group_members_to_wire_parts(&members).unwrap();
    let roles_parts = group_roles_to_wire_parts(&roles).unwrap();

    assert_eq!(put_parts.kind, KIND_GROUP_PUT_USER);
    assert_eq!(remove_parts.kind, KIND_GROUP_REMOVE_USER);
    assert_eq!(create_parts.kind, KIND_GROUP_CREATE_GROUP);
    assert_eq!(edit_parts.kind, KIND_GROUP_EDIT_METADATA);
    assert_eq!(delete_group_parts.kind, KIND_GROUP_DELETE_GROUP);
    assert_eq!(delete_event_parts.kind, KIND_GROUP_DELETE_EVENT);
    assert_eq!(invite_parts.kind, KIND_GROUP_CREATE_INVITE);
    assert_eq!(join_parts.kind, KIND_GROUP_JOIN_REQUEST);
    assert_eq!(leave_parts.kind, KIND_GROUP_LEAVE_REQUEST);
    assert_eq!(metadata_parts.kind, KIND_GROUP_METADATA);
    assert_eq!(admins_parts.kind, KIND_GROUP_ADMINS);
    assert_eq!(members_parts.kind, KIND_GROUP_MEMBERS);
    assert_eq!(roles_parts.kind, KIND_GROUP_ROLES);

    assert_eq!(
        group_put_user_from_event(put_parts.kind, &put_parts.tags, &put_parts.content).unwrap(),
        put
    );
    assert_eq!(
        group_remove_user_from_event(remove_parts.kind, &remove_parts.tags, &remove_parts.content)
            .unwrap(),
        remove
    );
    assert_eq!(
        group_create_group_from_event(create_parts.kind, &create_parts.tags, &create_parts.content)
            .unwrap(),
        create
    );
    assert_eq!(
        group_edit_metadata_from_event(edit_parts.kind, &edit_parts.tags, &edit_parts.content)
            .unwrap(),
        edit
    );
    assert_eq!(
        group_delete_group_from_event(
            delete_group_parts.kind,
            &delete_group_parts.tags,
            &delete_group_parts.content
        )
        .unwrap(),
        delete_group
    );
    assert_eq!(
        group_delete_event_from_event(
            delete_event_parts.kind,
            &delete_event_parts.tags,
            &delete_event_parts.content
        )
        .unwrap(),
        delete_event
    );
    assert_eq!(
        group_create_invite_from_event(
            invite_parts.kind,
            &invite_parts.tags,
            &invite_parts.content
        )
        .unwrap(),
        invite
    );
    assert_eq!(
        group_join_request_from_event(join_parts.kind, &join_parts.tags, &join_parts.content)
            .unwrap(),
        join
    );
    assert_eq!(
        group_leave_request_from_event(leave_parts.kind, &leave_parts.tags, &leave_parts.content)
            .unwrap(),
        leave
    );
    assert_eq!(
        group_metadata_from_event(
            metadata_parts.kind,
            &metadata_parts.tags,
            &metadata_parts.content
        )
        .unwrap(),
        metadata
    );
    assert_eq!(
        group_admins_from_event(admins_parts.kind, &admins_parts.tags, &admins_parts.content)
            .unwrap(),
        admins
    );
    assert_eq!(
        group_members_from_event(
            members_parts.kind,
            &members_parts.tags,
            &members_parts.content
        )
        .unwrap(),
        members
    );
    assert_eq!(
        group_roles_from_event(roles_parts.kind, &roles_parts.tags, &roles_parts.content).unwrap(),
        roles
    );
}

#[test]
fn group_public_codecs_reject_invalid_decode_shapes() {
    assert!(matches!(
        group_put_user_from_event(KIND_GROUP_REMOVE_USER, &[], "").unwrap_err(),
        EventParseError::InvalidKind {
            expected: "9000",
            got: KIND_GROUP_REMOVE_USER
        }
    ));
    assert!(matches!(
        group_put_user_from_event(KIND_GROUP_PUT_USER, &[tag("h", GROUP_ID)], "").unwrap_err(),
        EventParseError::MissingTag("p")
    ));
    assert!(matches!(
        group_metadata_from_event(KIND_GROUP_METADATA, &[tag("d", GROUP_ID)], "content")
            .unwrap_err(),
        EventParseError::InvalidJson("content")
    ));
    assert!(matches!(
        group_metadata_from_event(
            KIND_GROUP_METADATA,
            &[tag("d", GROUP_ID), tag("private", "true")],
            ""
        )
        .unwrap_err(),
        EventParseError::InvalidTag("private")
    ));
    assert!(matches!(
        group_metadata_from_event(
            KIND_GROUP_METADATA,
            &[
                tag("d", GROUP_ID),
                tag("supported_kinds", "78"),
                tag("supported_kinds", "30078")
            ],
            ""
        )
        .unwrap_err(),
        EventParseError::InvalidTag("supported_kinds")
    ));
    assert!(matches!(
        group_metadata_from_event(KIND_GROUP_METADATA, &[tag("d", GROUP_ID)], "").unwrap(),
        RadrootsGroupMetadata { .. }
    ));
    assert!(matches!(
        group_metadata_from_event(
            KIND_GROUP_METADATA,
            &[tag("d", GROUP_ID), tag("supported_kinds", "bad")],
            ""
        )
        .unwrap_err(),
        EventParseError::InvalidNumber("supported_kinds", _)
    ));
    assert!(matches!(
        group_put_user_from_event(
            KIND_GROUP_PUT_USER,
            &[
                tag("h", GROUP_ID),
                vec!["p".to_string(), PUBKEY.to_string(), "".to_string()]
            ],
            ""
        )
        .unwrap_err(),
        EventParseError::InvalidTag("p")
    ));
    assert!(matches!(
        group_roles_from_event(
            KIND_GROUP_ROLES,
            &[
                tag("d", GROUP_ID),
                vec!["role".to_string(), "member".to_string(), "".to_string()]
            ],
            ""
        )
        .unwrap_err(),
        EventParseError::InvalidTag("role")
    ));
    assert!(matches!(
        group_roles_from_event(
            KIND_GROUP_ROLES,
            &[
                tag("d", GROUP_ID),
                vec![
                    "role".to_string(),
                    "member".to_string(),
                    "can read".to_string(),
                    "".to_string()
                ]
            ],
            ""
        )
        .unwrap_err(),
        EventParseError::InvalidTag("role")
    ));
}

#[test]
fn group_public_encoders_reject_empty_required_fields() {
    assert_empty_required(
        group_create_group_to_wire_parts(&RadrootsGroupCreateGroup {
            group_id: GROUP_ID.to_string(),
            message: Some(String::new()),
            metadata: minimal_metadata(),
        }),
        "message",
    );
    assert_empty_required(
        group_edit_metadata_to_wire_parts(&RadrootsGroupEditMetadata {
            group_id: GROUP_ID.to_string(),
            message: None,
            metadata: RadrootsGroupEditableMetadata {
                about: Some(String::new()),
                ..minimal_metadata()
            },
        }),
        "about",
    );
    assert_empty_required(
        group_join_request_to_wire_parts(&RadrootsGroupJoinRequest {
            group_id: GROUP_ID.to_string(),
            message: None,
            code: Some(String::new()),
        }),
        "code",
    );
    assert_empty_required(
        group_roles_to_wire_parts(&RadrootsGroupRoles {
            d_tag: GROUP_ID.to_string(),
            description: None,
            roles: vec![RadrootsGroupRole {
                name: "member".to_string(),
                description: Some(String::new()),
                permissions: Vec::new(),
            }],
        }),
        "role.description",
    );
    assert_empty_required(
        group_roles_to_wire_parts(&RadrootsGroupRoles {
            d_tag: GROUP_ID.to_string(),
            description: None,
            roles: vec![RadrootsGroupRole {
                name: "member".to_string(),
                description: None,
                permissions: vec![String::new()],
            }],
        }),
        "role.permissions",
    );
}

fn full_metadata() -> RadrootsGroupEditableMetadata {
    RadrootsGroupEditableMetadata {
        name: Some("Small Regen Farm".to_string()),
        about: Some("Field app group".to_string()),
        picture: Some("https://media.example.invalid/group.png".to_string()),
        is_private: true,
        is_restricted: true,
        is_closed: true,
        is_hidden: true,
        supported_kinds: Some(vec![78, 30078]),
    }
}

fn minimal_metadata() -> RadrootsGroupEditableMetadata {
    RadrootsGroupEditableMetadata {
        name: None,
        about: None,
        picture: None,
        is_private: false,
        is_restricted: false,
        is_closed: false,
        is_hidden: false,
        supported_kinds: None,
    }
}

fn assert_empty_required<T>(result: Result<T, EventEncodeError>, field: &'static str) {
    let err = match result {
        Ok(_) => panic!("expected empty required field error"),
        Err(err) => err,
    };
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField(found) if found == field
    ));
}

fn tag(key: &str, value: &str) -> Vec<String> {
    vec![key.to_string(), value.to_string()]
}
