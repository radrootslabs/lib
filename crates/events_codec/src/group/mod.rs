pub mod decode;
pub mod encode;

#[cfg(test)]
mod tests {
    use radroots_events::group::{
        KIND_GROUP_ADMINS, KIND_GROUP_CREATE_INVITE, KIND_GROUP_JOIN_REQUEST, KIND_GROUP_MEMBERS,
        KIND_GROUP_METADATA, KIND_GROUP_PUT_USER, KIND_GROUP_REMOVE_USER, KIND_GROUP_ROLES,
        RadrootsGroupAdmins, RadrootsGroupCreateInvite, RadrootsGroupEditableMetadata,
        RadrootsGroupJoinRequest, RadrootsGroupMembers, RadrootsGroupMetadata,
        RadrootsGroupPutUser, RadrootsGroupRemoveUser, RadrootsGroupRole, RadrootsGroupRoles,
        RadrootsGroupUserRef,
    };

    use crate::error::EventParseError;
    use crate::group::decode::{
        group_admins_from_event, group_create_invite_from_event, group_join_request_from_event,
        group_members_from_event, group_metadata_from_event, group_put_user_from_event,
        group_remove_user_from_event, group_roles_from_event,
    };
    use crate::group::encode::{
        group_admins_to_wire_parts, group_create_invite_to_wire_parts,
        group_join_request_to_wire_parts, group_members_to_wire_parts,
        group_metadata_to_wire_parts, group_put_user_to_wire_parts,
        group_remove_user_to_wire_parts, group_roles_to_wire_parts,
    };

    #[test]
    fn group_user_operations_use_h_group_id_routing() {
        let put = RadrootsGroupPutUser {
            group_id: "field-group".to_string(),
            pubkey: "member_pubkey".to_string(),
            roles: vec!["member".to_string()],
        };
        let remove = RadrootsGroupRemoveUser {
            group_id: "field-group".to_string(),
            pubkey: "member_pubkey".to_string(),
        };

        let put_parts = group_put_user_to_wire_parts(&put).expect("put user");
        let remove_parts = group_remove_user_to_wire_parts(&remove).expect("remove user");

        assert_eq!(put_parts.kind, KIND_GROUP_PUT_USER);
        assert_eq!(remove_parts.kind, KIND_GROUP_REMOVE_USER);
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
            admins: vec![sample_user("admin_pubkey", "admin")],
        };
        let members = RadrootsGroupMembers {
            d_tag: "field-group".to_string(),
            members: vec![sample_user("member_pubkey", "member")],
        };
        let roles = RadrootsGroupRoles {
            d_tag: "field-group".to_string(),
            roles: vec![sample_role()],
        };

        let metadata_parts = group_metadata_to_wire_parts(&metadata).expect("metadata");
        let admins_parts = group_admins_to_wire_parts(&admins).expect("admins");
        let members_parts = group_members_to_wire_parts(&members).expect("members");
        let roles_parts = group_roles_to_wire_parts(&roles).expect("roles");

        assert_eq!(metadata_parts.kind, KIND_GROUP_METADATA);
        assert!(metadata_parts.tags.contains(&tag("d", "field-group")));
        assert!(
            !metadata_parts
                .tags
                .iter()
                .any(|tag| tag.first().map(|v| v.as_str()) == Some("h"))
        );
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
            invitee_pubkey: Some("member_pubkey".to_string()),
            roles: vec!["member".to_string()],
            expires_at: Some(1_780_000_000),
            claim: Some("claim-token".to_string()),
        };
        let join = RadrootsGroupJoinRequest {
            group_id: "field-group".to_string(),
            message: Some("requesting access".to_string()),
        };

        let invite_parts = group_create_invite_to_wire_parts(&invite).expect("invite");
        let join_parts = group_join_request_to_wire_parts(&join).expect("join");

        assert_eq!(invite_parts.kind, KIND_GROUP_CREATE_INVITE);
        assert_eq!(join_parts.kind, KIND_GROUP_JOIN_REQUEST);
        assert!(invite_parts.tags.contains(&tag("h", "field-group")));
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

    fn sample_metadata() -> RadrootsGroupEditableMetadata {
        RadrootsGroupEditableMetadata {
            name: Some("Small Regen Farm".to_string()),
            about: Some("Field app group".to_string()),
            picture: Some("https://media.example.invalid/group.png".to_string()),
            is_private: false,
            is_closed: false,
            is_hidden: false,
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
}
