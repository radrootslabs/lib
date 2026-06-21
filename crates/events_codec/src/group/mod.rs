pub mod decode;
pub mod encode;

#[cfg(test)]
mod tests {
    use radroots_events::group::{
        KIND_GROUP_ADMINS, KIND_GROUP_CREATE_GROUP, KIND_GROUP_CREATE_INVITE,
        KIND_GROUP_DELETE_EVENT, KIND_GROUP_DELETE_GROUP, KIND_GROUP_EDIT_METADATA,
        KIND_GROUP_JOIN_REQUEST, KIND_GROUP_LEAVE_REQUEST, KIND_GROUP_MEMBERS, KIND_GROUP_METADATA,
        KIND_GROUP_PUT_USER, KIND_GROUP_REMOVE_USER, KIND_GROUP_ROLES, RadrootsGroupAdmins,
        RadrootsGroupCreateGroup, RadrootsGroupCreateInvite, RadrootsGroupDeleteEvent,
        RadrootsGroupDeleteGroup, RadrootsGroupEditMetadata, RadrootsGroupEditableMetadata,
        RadrootsGroupJoinRequest, RadrootsGroupLeaveRequest, RadrootsGroupMembers,
        RadrootsGroupMetadata, RadrootsGroupPutUser, RadrootsGroupRemoveUser, RadrootsGroupRole,
        RadrootsGroupRoles, RadrootsGroupUserRef,
    };

    use crate::error::{EventEncodeError, EventParseError};
    use crate::group::decode::{
        group_admins_from_event, group_create_group_from_event, group_create_invite_from_event,
        group_delete_event_from_event, group_delete_group_from_event,
        group_edit_metadata_from_event, group_join_request_from_event,
        group_leave_request_from_event, group_members_from_event, group_metadata_from_event,
        group_put_user_from_event, group_remove_user_from_event, group_roles_from_event,
    };
    use crate::group::encode::{
        group_admins_to_wire_parts, group_create_group_to_wire_parts,
        group_create_invite_to_wire_parts, group_delete_event_to_wire_parts,
        group_delete_group_to_wire_parts, group_edit_metadata_to_wire_parts,
        group_join_request_to_wire_parts, group_leave_request_to_wire_parts,
        group_members_to_wire_parts, group_metadata_to_wire_parts, group_put_user_to_wire_parts,
        group_remove_user_to_wire_parts, group_roles_to_wire_parts,
    };

    #[test]
    fn group_user_operations_use_h_group_id_routing() {
        let put = RadrootsGroupPutUser {
            group_id: "field-group".to_string(),
            message: Some("add member".to_string()),
            pubkey: "member_pubkey".to_string(),
            roles: vec!["member".to_string()],
        };
        let remove = RadrootsGroupRemoveUser {
            group_id: "field-group".to_string(),
            message: Some("remove member".to_string()),
            pubkey: "member_pubkey".to_string(),
        };

        let put_parts = group_put_user_to_wire_parts(&put).expect("put user");
        let remove_parts = group_remove_user_to_wire_parts(&remove).expect("remove user");

        assert_eq!(put_parts.kind, KIND_GROUP_PUT_USER);
        assert_eq!(remove_parts.kind, KIND_GROUP_REMOVE_USER);
        assert_eq!(put_parts.content, "add member");
        assert_eq!(remove_parts.content, "remove member");
        assert!(put_parts.tags.contains(&tag("h", "field-group")));
        assert!(
            !put_parts
                .tags
                .iter()
                .any(|tag| tag.first().map(|v| v.as_str()) == Some("d"))
        );
        assert_eq!(
            group_put_user_from_event(put_parts.kind, &put_parts.tags, &put_parts.content)
                .expect("decode put"),
            put
        );
        assert_eq!(
            group_remove_user_from_event(
                remove_parts.kind,
                &remove_parts.tags,
                &remove_parts.content
            )
            .expect("decode remove"),
            remove
        );
    }

    #[test]
    fn group_metadata_and_lists_use_d_tag_routing() {
        let metadata = RadrootsGroupMetadata {
            d_tag: "field-group".to_string(),
            metadata: sample_metadata(),
        };
        let admins = RadrootsGroupAdmins {
            d_tag: "field-group".to_string(),
            description: Some("group admins".to_string()),
            admins: vec![sample_user("admin_pubkey", "admin")],
        };
        let members = RadrootsGroupMembers {
            d_tag: "field-group".to_string(),
            description: Some("group members".to_string()),
            members: vec![sample_user("member_pubkey", "member")],
        };
        let roles = RadrootsGroupRoles {
            d_tag: "field-group".to_string(),
            description: Some("group roles".to_string()),
            roles: vec![sample_role()],
        };

        let metadata_parts = group_metadata_to_wire_parts(&metadata).expect("metadata");
        let admins_parts = group_admins_to_wire_parts(&admins).expect("admins");
        let members_parts = group_members_to_wire_parts(&members).expect("members");
        let roles_parts = group_roles_to_wire_parts(&roles).expect("roles");

        assert_eq!(metadata_parts.kind, KIND_GROUP_METADATA);
        assert!(metadata_parts.tags.contains(&tag("d", "field-group")));
        assert!(metadata_parts.tags.contains(&marker("restricted")));
        assert!(metadata_parts.tags.contains(&marker("closed")));
        assert!(metadata_parts.tags.contains(&vec![
            "supported_kinds".to_string(),
            "78".to_string(),
            "30078".to_string()
        ]));
        assert!(
            !metadata_parts
                .tags
                .iter()
                .any(|tag| tag.first().map(|v| v.as_str()) == Some("h"))
        );
        assert_eq!(admins_parts.content, "group admins");
        assert_eq!(members_parts.content, "group members");
        assert_eq!(roles_parts.content, "group roles");
        assert_eq!(
            group_metadata_from_event(
                metadata_parts.kind,
                &metadata_parts.tags,
                &metadata_parts.content
            )
            .expect("decode metadata"),
            metadata
        );
        assert_eq!(
            group_admins_from_event(admins_parts.kind, &admins_parts.tags, &admins_parts.content)
                .expect("decode admins"),
            admins
        );
        assert_eq!(
            group_members_from_event(
                members_parts.kind,
                &members_parts.tags,
                &members_parts.content
            )
            .expect("decode members"),
            members
        );
        assert_eq!(
            group_roles_from_event(roles_parts.kind, &roles_parts.tags, &roles_parts.content)
                .expect("decode roles"),
            roles
        );
        assert_eq!(admins_parts.kind, KIND_GROUP_ADMINS);
        assert_eq!(members_parts.kind, KIND_GROUP_MEMBERS);
        assert_eq!(roles_parts.kind, KIND_GROUP_ROLES);
    }

    #[test]
    fn group_invites_and_join_requests_roundtrip_without_field_authorization() {
        let invite = RadrootsGroupCreateInvite {
            group_id: "field-group".to_string(),
            message: Some("join the field group".to_string()),
            code: "invite-code".to_string(),
        };
        let join = RadrootsGroupJoinRequest {
            group_id: "field-group".to_string(),
            message: Some("requesting access".to_string()),
            code: Some("invite-code".to_string()),
        };

        let invite_parts = group_create_invite_to_wire_parts(&invite).expect("invite");
        let join_parts = group_join_request_to_wire_parts(&join).expect("join");

        assert_eq!(invite_parts.kind, KIND_GROUP_CREATE_INVITE);
        assert_eq!(join_parts.kind, KIND_GROUP_JOIN_REQUEST);
        assert!(invite_parts.tags.contains(&tag("h", "field-group")));
        assert!(invite_parts.tags.contains(&tag("code", "invite-code")));
        assert!(join_parts.tags.contains(&tag("code", "invite-code")));
        assert_eq!(invite_parts.content, "join the field group");
        assert_eq!(join_parts.content, "requesting access");
        assert_eq!(
            group_create_invite_from_event(
                invite_parts.kind,
                &invite_parts.tags,
                &invite_parts.content
            )
            .expect("decode invite"),
            invite
        );
        assert_eq!(
            group_join_request_from_event(join_parts.kind, &join_parts.tags, &join_parts.content)
                .expect("decode join"),
            join
        );
    }

    #[test]
    fn group_lifecycle_and_moderation_events_roundtrip() {
        let metadata = RadrootsGroupEditableMetadata {
            is_private: true,
            is_hidden: true,
            ..sample_metadata()
        };
        let create = RadrootsGroupCreateGroup {
            group_id: "field-group".to_string(),
            message: Some("create group".to_string()),
            metadata: metadata.clone(),
        };
        let edit = RadrootsGroupEditMetadata {
            group_id: "field-group".to_string(),
            message: Some("edit group".to_string()),
            metadata,
        };
        let delete_group = RadrootsGroupDeleteGroup {
            group_id: "field-group".to_string(),
            message: Some("delete group".to_string()),
        };
        let delete_event = RadrootsGroupDeleteEvent {
            group_id: "field-group".to_string(),
            message: Some("delete event".to_string()),
            event_id: "event_id".to_string(),
        };
        let leave = RadrootsGroupLeaveRequest {
            group_id: "field-group".to_string(),
            message: None,
        };

        let create_parts = group_create_group_to_wire_parts(&create).expect("create");
        let edit_parts = group_edit_metadata_to_wire_parts(&edit).expect("edit");
        let delete_group_parts =
            group_delete_group_to_wire_parts(&delete_group).expect("delete group");
        let delete_event_parts =
            group_delete_event_to_wire_parts(&delete_event).expect("delete event");
        let leave_parts = group_leave_request_to_wire_parts(&leave).expect("leave");

        assert_eq!(create_parts.kind, KIND_GROUP_CREATE_GROUP);
        assert_eq!(edit_parts.kind, KIND_GROUP_EDIT_METADATA);
        assert_eq!(delete_group_parts.kind, KIND_GROUP_DELETE_GROUP);
        assert_eq!(delete_event_parts.kind, KIND_GROUP_DELETE_EVENT);
        assert_eq!(leave_parts.kind, KIND_GROUP_LEAVE_REQUEST);
        assert!(create_parts.tags.contains(&marker("private")));
        assert!(create_parts.tags.contains(&marker("hidden")));
        assert!(delete_event_parts.tags.contains(&tag("e", "event_id")));
        assert_eq!(leave_parts.content, "");
        assert_eq!(
            group_create_group_from_event(
                create_parts.kind,
                &create_parts.tags,
                &create_parts.content
            )
            .expect("decode create"),
            create
        );
        assert_eq!(
            group_edit_metadata_from_event(edit_parts.kind, &edit_parts.tags, &edit_parts.content)
                .expect("decode edit"),
            edit
        );
        assert_eq!(
            group_delete_group_from_event(
                delete_group_parts.kind,
                &delete_group_parts.tags,
                &delete_group_parts.content
            )
            .expect("decode delete group"),
            delete_group
        );
        assert_eq!(
            group_delete_event_from_event(
                delete_event_parts.kind,
                &delete_event_parts.tags,
                &delete_event_parts.content
            )
            .expect("decode delete event"),
            delete_event
        );
        assert_eq!(
            group_leave_request_from_event(
                leave_parts.kind,
                &leave_parts.tags,
                &leave_parts.content
            )
            .expect("decode leave"),
            leave
        );
    }

    #[test]
    fn group_codecs_reject_wrong_routing_tags() {
        let metadata = RadrootsGroupMetadata {
            d_tag: "field-group".to_string(),
            metadata: sample_metadata(),
        };
        let mut metadata_parts = group_metadata_to_wire_parts(&metadata).expect("metadata");
        metadata_parts
            .tags
            .retain(|tag| tag.first().map(|value| value.as_str()) != Some("d"));
        metadata_parts.tags.push(tag("h", "field-group"));
        let metadata_err = group_metadata_from_event(
            metadata_parts.kind,
            &metadata_parts.tags,
            &metadata_parts.content,
        )
        .unwrap_err();
        assert!(matches!(metadata_err, EventParseError::MissingTag("d")));

        let put = RadrootsGroupPutUser {
            group_id: "field-group".to_string(),
            message: None,
            pubkey: "member_pubkey".to_string(),
            roles: vec!["member".to_string()],
        };
        let mut put_parts = group_put_user_to_wire_parts(&put).expect("put");
        put_parts
            .tags
            .retain(|tag| tag.first().map(|value| value.as_str()) != Some("h"));
        put_parts.tags.push(tag("d", "field-group"));
        let put_err =
            group_put_user_from_event(put_parts.kind, &put_parts.tags, &put_parts.content)
                .unwrap_err();
        assert!(matches!(put_err, EventParseError::MissingTag("h")));
    }

    #[test]
    fn group_codecs_reject_nonstandard_first_pass_group_shapes() {
        let valued_marker_tags = vec![
            tag("d", "field-group"),
            tag("private", "true"),
            tag("supported_kinds", "78"),
        ];
        let metadata_err =
            group_metadata_from_event(KIND_GROUP_METADATA, &valued_marker_tags, "").unwrap_err();
        assert!(matches!(
            metadata_err,
            EventParseError::InvalidTag("private")
        ));

        let first_pass_invite_tags = vec![
            tag("h", "field-group"),
            tag("p", "member_pubkey"),
            tag("role", "member"),
            tag("claim", "claim-token"),
        ];
        let invite_err =
            group_create_invite_from_event(KIND_GROUP_CREATE_INVITE, &first_pass_invite_tags, "")
                .unwrap_err();
        assert!(matches!(invite_err, EventParseError::MissingTag("code")));
    }

    #[test]
    fn group_encoders_reject_empty_required_fields() {
        assert_empty_required(
            group_put_user_to_wire_parts(&RadrootsGroupPutUser {
                group_id: "".to_string(),
                message: None,
                pubkey: "member_pubkey".to_string(),
                roles: vec![],
            }),
            "group_id",
        );
        assert_empty_required(
            group_put_user_to_wire_parts(&RadrootsGroupPutUser {
                group_id: "field-group".to_string(),
                message: None,
                pubkey: "".to_string(),
                roles: vec![],
            }),
            "pubkey",
        );
        assert_empty_required(
            group_put_user_to_wire_parts(&RadrootsGroupPutUser {
                group_id: "field-group".to_string(),
                message: None,
                pubkey: "member_pubkey".to_string(),
                roles: vec!["".to_string()],
            }),
            "roles",
        );
        assert_empty_required(
            group_remove_user_to_wire_parts(&RadrootsGroupRemoveUser {
                group_id: "field-group".to_string(),
                message: None,
                pubkey: "".to_string(),
            }),
            "pubkey",
        );
        assert_empty_required(
            group_create_group_to_wire_parts(&RadrootsGroupCreateGroup {
                group_id: "field-group".to_string(),
                message: Some("".to_string()),
                metadata: sample_metadata(),
            }),
            "message",
        );
        assert_empty_required(
            group_edit_metadata_to_wire_parts(&RadrootsGroupEditMetadata {
                group_id: "field-group".to_string(),
                message: None,
                metadata: RadrootsGroupEditableMetadata {
                    name: Some("".to_string()),
                    ..sample_metadata()
                },
            }),
            "name",
        );
        assert_empty_required(
            group_delete_event_to_wire_parts(&RadrootsGroupDeleteEvent {
                group_id: "field-group".to_string(),
                message: None,
                event_id: "".to_string(),
            }),
            "event_id",
        );
        assert_empty_required(
            group_create_invite_to_wire_parts(&RadrootsGroupCreateInvite {
                group_id: "field-group".to_string(),
                message: None,
                code: "".to_string(),
            }),
            "code",
        );
        assert_empty_required(
            group_join_request_to_wire_parts(&RadrootsGroupJoinRequest {
                group_id: "field-group".to_string(),
                message: None,
                code: Some("".to_string()),
            }),
            "code",
        );
        assert_empty_required(
            group_leave_request_to_wire_parts(&RadrootsGroupLeaveRequest {
                group_id: "".to_string(),
                message: None,
            }),
            "group_id",
        );
        assert_empty_required(
            group_metadata_to_wire_parts(&RadrootsGroupMetadata {
                d_tag: "".to_string(),
                metadata: sample_metadata(),
            }),
            "d_tag",
        );
        assert_empty_required(
            group_admins_to_wire_parts(&RadrootsGroupAdmins {
                d_tag: "field-group".to_string(),
                description: None,
                admins: vec![RadrootsGroupUserRef {
                    pubkey: "".to_string(),
                    roles: vec![],
                }],
            }),
            "pubkey",
        );
        assert_empty_required(
            group_members_to_wire_parts(&RadrootsGroupMembers {
                d_tag: "field-group".to_string(),
                description: Some("".to_string()),
                members: vec![],
            }),
            "message",
        );
        assert_empty_required(
            group_members_to_wire_parts(&RadrootsGroupMembers {
                d_tag: "field-group".to_string(),
                description: None,
                members: vec![RadrootsGroupUserRef {
                    pubkey: "member_pubkey".to_string(),
                    roles: vec!["".to_string()],
                }],
            }),
            "roles",
        );
        assert_empty_required(
            group_roles_to_wire_parts(&RadrootsGroupRoles {
                d_tag: "field-group".to_string(),
                description: None,
                roles: vec![RadrootsGroupRole {
                    name: "".to_string(),
                    description: None,
                    permissions: vec![],
                }],
            }),
            "role.name",
        );
        assert_empty_required(
            group_roles_to_wire_parts(&RadrootsGroupRoles {
                d_tag: "field-group".to_string(),
                description: None,
                roles: vec![RadrootsGroupRole {
                    name: "member".to_string(),
                    description: Some("".to_string()),
                    permissions: vec![],
                }],
            }),
            "role.description",
        );
        assert_empty_required(
            group_roles_to_wire_parts(&RadrootsGroupRoles {
                d_tag: "field-group".to_string(),
                description: None,
                roles: vec![RadrootsGroupRole {
                    name: "member".to_string(),
                    description: None,
                    permissions: vec!["".to_string()],
                }],
            }),
            "role.permissions",
        );
    }

    #[test]
    fn group_decoders_reject_invalid_tag_shapes_and_kinds() {
        let invalid_kind = group_put_user_from_event(KIND_GROUP_REMOVE_USER, &[], "").unwrap_err();
        assert!(matches!(
            invalid_kind,
            EventParseError::InvalidKind {
                expected: "9000",
                got: KIND_GROUP_REMOVE_USER
            }
        ));

        let metadata_content =
            group_metadata_from_event(KIND_GROUP_METADATA, &[tag("d", "field-group")], "not empty")
                .unwrap_err();
        assert!(matches!(
            metadata_content,
            EventParseError::InvalidJson("content")
        ));

        for tags in [
            vec![tag("d", "field-group"), marker("hidden"), marker("hidden")],
            vec![
                tag("d", "field-group"),
                tag("supported_kinds", "78"),
                tag("supported_kinds", "30078"),
            ],
            vec![tag("d", "field-group"), tag("supported_kinds", "")],
        ] {
            let err = group_metadata_from_event(KIND_GROUP_METADATA, &tags, "").unwrap_err();
            assert!(matches!(
                err,
                EventParseError::InvalidTag("hidden")
                    | EventParseError::InvalidTag("supported_kinds")
            ));
        }

        let invalid_supported_kind = group_metadata_from_event(
            KIND_GROUP_METADATA,
            &[tag("d", "field-group"), tag("supported_kinds", "bad")],
            "",
        )
        .unwrap_err();
        assert!(matches!(
            invalid_supported_kind,
            EventParseError::InvalidNumber("supported_kinds", _)
        ));

        for tags in [
            vec![tag("h", "field-group"), marker("p")],
            vec![tag("h", "field-group"), tag("p", "")],
            vec![
                tag("h", "field-group"),
                vec!["p".to_string(), "member_pubkey".to_string(), "".to_string()],
            ],
        ] {
            let err = group_put_user_from_event(KIND_GROUP_PUT_USER, &tags, "").unwrap_err();
            assert!(matches!(err, EventParseError::InvalidTag("p")));
        }

        for tags in [
            vec![tag("d", "field-group"), marker("role")],
            vec![tag("d", "field-group"), tag("role", "")],
            vec![
                tag("d", "field-group"),
                vec!["role".to_string(), "member".to_string(), "".to_string()],
            ],
            vec![
                tag("d", "field-group"),
                vec![
                    "role".to_string(),
                    "member".to_string(),
                    "can read".to_string(),
                    "".to_string(),
                ],
            ],
        ] {
            let err = group_roles_from_event(KIND_GROUP_ROLES, &tags, "").unwrap_err();
            assert!(matches!(err, EventParseError::InvalidTag("role")));
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

    fn sample_metadata() -> RadrootsGroupEditableMetadata {
        RadrootsGroupEditableMetadata {
            name: Some("Small Regen Farm".to_string()),
            about: Some("Field app group".to_string()),
            picture: Some("https://media.example.invalid/group.png".to_string()),
            is_private: false,
            is_restricted: true,
            is_closed: true,
            is_hidden: false,
            supported_kinds: Some(vec![78, 30078]),
        }
    }

    fn sample_user(pubkey: &str, role: &str) -> RadrootsGroupUserRef {
        RadrootsGroupUserRef {
            pubkey: pubkey.to_string(),
            roles: vec![role.to_string()],
        }
    }

    fn sample_role() -> RadrootsGroupRole {
        RadrootsGroupRole {
            name: "member".to_string(),
            description: Some("can read and write group events".to_string()),
            permissions: vec!["read".to_string(), "write".to_string()],
        }
    }

    fn tag(key: &str, value: &str) -> Vec<String> {
        vec![key.to_string(), value.to_string()]
    }

    fn marker(key: &str) -> Vec<String> {
        vec![key.to_string()]
    }
}
