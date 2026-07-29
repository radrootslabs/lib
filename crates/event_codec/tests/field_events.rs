#![cfg(feature = "serde_json")]

use radroots_event::{
    envelope::kind::{KIND_FARM_FILE_METADATA, KIND_POST},
    farm::FarmRef,
    farm::crdt::{
        CrdtBackend, FarmCrdtChange, FarmCrdtDocumentKind, FarmSemanticKind,
        RADROOTS_FARM_CRDT_CHANGE_SCHEMA,
    },
    farm::file::{FarmFileDimensions, FarmFileMetadata, FarmFileSource},
    farm::workspace::{
        FarmWorkspaceManifest, FarmWorkspaceMediaServer, FarmWorkspaceRef, FarmWorkspaceRelay,
        FarmWorkspaceRelayMode, RADROOTS_FARM_WORKSPACE_PROTOCOL_VERSION,
        RADROOTS_FARM_WORKSPACE_SCHEMA,
    },
    social::group::{
        GroupAdmins, GroupCreateInvite, GroupEditableMetadata, GroupMetadata, GroupPutUser,
        GroupUserRef, KIND_GROUP_CREATE_INVITE, KIND_GROUP_METADATA,
    },
    social::http_auth::HttpAuth,
    social::relay_auth::RelayAuth,
};
use radroots_event_codec::{
    error::EventParseError,
    farm_crdt::{
        decode::farm_crdt_change_from_event_with_author,
        encode::{to_wire_parts as crdt_to_wire_parts, to_wire_parts_with_author},
    },
    farm_file::{
        decode::farm_file_metadata_from_event, encode::to_wire_parts as file_to_wire_parts,
    },
    farm_workspace::{
        decode::farm_workspace_from_event, encode::to_wire_parts as workspace_to_wire_parts,
    },
    group::{
        decode::{
            group_admins_from_event, group_create_invite_from_event, group_metadata_from_event,
            group_put_user_from_event,
        },
        encode::{
            group_admins_to_wire_parts, group_create_invite_to_wire_parts,
            group_metadata_to_wire_parts, group_put_user_to_wire_parts,
        },
    },
    http_auth::{decode::http_auth_from_event, encode::to_wire_parts as http_auth_to_wire_parts},
    relay_auth::{
        decode::relay_auth_from_event, encode::to_wire_parts as relay_auth_to_wire_parts,
    },
};

const WORKSPACE_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAAAA";
const FILE_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAAAQ";
const DOCUMENT_ID: &str = "AAAAAAAAAAAAAAAAAAAAAg";
const GROUP_ID: &str = "field-group";
const AUTHOR: &str = "author_pubkey";
const SHA256: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

#[test]
fn field_codec_matrix_roundtrips_all_new_event_families() {
    let workspace = sample_workspace();
    let workspace_parts = workspace_to_wire_parts(&workspace).expect("workspace parts");
    assert_eq!(
        farm_workspace_from_event(
            workspace_parts.kind,
            &workspace_parts.tags,
            &workspace_parts.content,
        )
        .expect("workspace decode")
        .d_tag,
        WORKSPACE_D_TAG
    );

    let crdt = sample_crdt_change();
    let crdt_parts = to_wire_parts_with_author(&crdt, AUTHOR).expect("crdt parts");
    assert_eq!(
        farm_crdt_change_from_event_with_author(
            crdt_parts.kind,
            &crdt_parts.tags,
            &crdt_parts.content,
            AUTHOR,
        )
        .expect("crdt decode")
        .document_id,
        DOCUMENT_ID
    );

    let file = sample_file_metadata();
    let file_parts = file_to_wire_parts(&file).expect("file parts");
    assert_eq!(
        farm_file_metadata_from_event(file_parts.kind, &file_parts.tags, &file_parts.content)
            .expect("file decode"),
        file
    );

    let relay_auth = RelayAuth {
        relay: "wss://relay.example.invalid/farm/field-group".to_string(),
        challenge: "relay-provided-challenge".to_string(),
    };
    let relay_parts = relay_auth_to_wire_parts(&relay_auth).expect("relay auth parts");
    assert_eq!(
        relay_auth_from_event(relay_parts.kind, &relay_parts.tags, &relay_parts.content)
            .expect("relay auth decode"),
        relay_auth
    );

    let http_auth = HttpAuth {
        url: "https://media.example.invalid/upload".to_string(),
        method: "POST".to_string(),
        payload_sha256: Some(SHA256.to_string()),
    };
    let http_parts = http_auth_to_wire_parts(&http_auth).expect("http auth parts");
    assert_eq!(
        http_auth_from_event(http_parts.kind, &http_parts.tags, &http_parts.content)
            .expect("http auth decode"),
        http_auth
    );

    let metadata = GroupMetadata {
        d_tag: GROUP_ID.to_string(),
        metadata: sample_group_metadata(),
    };
    let metadata_parts = group_metadata_to_wire_parts(&metadata).expect("metadata parts");
    assert_eq!(
        group_metadata_from_event(
            metadata_parts.kind,
            &metadata_parts.tags,
            &metadata_parts.content,
        )
        .expect("metadata decode"),
        metadata
    );

    let admins = GroupAdmins {
        d_tag: GROUP_ID.to_string(),
        description: Some("field group admins".to_string()),
        admins: vec![GroupUserRef {
            pubkey: "admin_pubkey".to_string(),
            roles: vec!["admin".to_string()],
        }],
    };
    let admins_parts = group_admins_to_wire_parts(&admins).expect("admins parts");
    assert_eq!(
        group_admins_from_event(admins_parts.kind, &admins_parts.tags, &admins_parts.content)
            .expect("admins decode"),
        admins
    );

    let put = GroupPutUser {
        group_id: GROUP_ID.to_string(),
        message: Some("add field member".to_string()),
        pubkey: "member_pubkey".to_string(),
        roles: vec!["member".to_string()],
    };
    let put_parts = group_put_user_to_wire_parts(&put).expect("put parts");
    assert_eq!(
        group_put_user_from_event(put_parts.kind, &put_parts.tags, &put_parts.content)
            .expect("put decode"),
        put
    );

    let invite = GroupCreateInvite {
        group_id: GROUP_ID.to_string(),
        message: Some("join the field group".to_string()),
        code: "invite-code".to_string(),
    };
    let invite_parts = group_create_invite_to_wire_parts(&invite).expect("invite parts");
    assert_eq!(
        group_create_invite_from_event(
            invite_parts.kind,
            &invite_parts.tags,
            &invite_parts.content
        )
        .expect("invite decode"),
        invite
    );
}

#[test]
fn field_codec_matrix_rejects_missing_required_tags_and_mismatches() {
    let workspace_parts = workspace_to_wire_parts(&sample_workspace()).expect("workspace parts");
    let workspace_without_h = without_tag(&workspace_parts.tags, "h");
    assert!(matches!(
        farm_workspace_from_event(
            workspace_parts.kind,
            &workspace_without_h,
            &workspace_parts.content,
        ),
        Err(EventParseError::MissingTag("h"))
    ));

    let file_parts = file_to_wire_parts(&sample_file_metadata()).expect("file parts");
    let file_without_x = without_tag(&file_parts.tags, "x");
    assert!(matches!(
        farm_file_metadata_from_event(file_parts.kind, &file_without_x, &file_parts.content),
        Err(EventParseError::MissingTag("x"))
    ));

    let mut duplicate_d = file_parts.tags.clone();
    duplicate_d.push(vec!["d".to_string(), WORKSPACE_D_TAG.to_string()]);
    assert!(matches!(
        farm_file_metadata_from_event(file_parts.kind, &duplicate_d, &file_parts.content),
        Err(EventParseError::InvalidTag("d"))
    ));

    let put_parts = group_put_user_to_wire_parts(&GroupPutUser {
        group_id: GROUP_ID.to_string(),
        message: None,
        pubkey: "member_pubkey".to_string(),
        roles: vec!["member".to_string()],
    })
    .expect("put parts");
    assert!(matches!(
        group_put_user_from_event(put_parts.kind, &without_tag(&put_parts.tags, "h"), ""),
        Err(EventParseError::MissingTag("h"))
    ));

    let valued_marker_tags = vec![
        vec!["d".to_string(), GROUP_ID.to_string()],
        vec!["private".to_string(), "true".to_string()],
    ];
    assert!(matches!(
        group_metadata_from_event(KIND_GROUP_METADATA, &valued_marker_tags, ""),
        Err(EventParseError::InvalidTag("private"))
    ));

    let first_pass_invite_tags = vec![
        vec!["h".to_string(), GROUP_ID.to_string()],
        vec!["p".to_string(), "member_pubkey".to_string()],
        vec!["role".to_string(), "member".to_string()],
        vec!["claim".to_string(), "claim-token".to_string()],
    ];
    assert!(matches!(
        group_create_invite_from_event(KIND_GROUP_CREATE_INVITE, &first_pass_invite_tags, ""),
        Err(EventParseError::MissingTag("code"))
    ));
}

#[test]
fn field_codec_matrix_rejects_bad_hash_base64_kind_and_content() {
    let mut file_parts = file_to_wire_parts(&sample_file_metadata()).expect("file parts");
    replace_tag_value(&mut file_parts.tags, "x", "ABC");
    assert!(matches!(
        farm_file_metadata_from_event(file_parts.kind, &file_parts.tags, &file_parts.content),
        Err(EventParseError::InvalidTag("x"))
    ));

    let crdt_parts = crdt_to_wire_parts(&sample_crdt_change()).expect("crdt parts");
    let mut bad_crdt = sample_crdt_change();
    bad_crdt.encoded_change = "abc/def".to_string();
    let bad_crdt_content = serde_json::to_string(&bad_crdt).expect("bad crdt content");
    assert!(matches!(
        farm_crdt_change_from_event_with_author(
            crdt_parts.kind,
            &crdt_parts.tags,
            &bad_crdt_content,
            AUTHOR,
        ),
        Err(EventParseError::InvalidJson("encoded_change"))
    ));

    assert!(matches!(
        farm_workspace_from_event(KIND_POST, &[], ""),
        Err(EventParseError::InvalidKind {
            expected: "30078",
            got: KIND_POST
        })
    ));

    let relay_parts = relay_auth_to_wire_parts(&RelayAuth {
        relay: "wss://relay.example.invalid/farm/field-group".to_string(),
        challenge: "relay-provided-challenge".to_string(),
    })
    .expect("relay auth parts");
    assert!(matches!(
        relay_auth_from_event(relay_parts.kind, &relay_parts.tags, "not empty"),
        Err(EventParseError::InvalidJson("content"))
    ));

    let mut http_parts = http_auth_to_wire_parts(&HttpAuth {
        url: "https://media.example.invalid/upload".to_string(),
        method: "POST".to_string(),
        payload_sha256: Some(SHA256.to_string()),
    })
    .expect("http auth parts");
    replace_tag_value(&mut http_parts.tags, "payload", "ABC");
    assert!(matches!(
        http_auth_from_event(http_parts.kind, &http_parts.tags, &http_parts.content),
        Err(EventParseError::InvalidTag("payload"))
    ));
}

fn sample_workspace() -> FarmWorkspaceManifest {
    FarmWorkspaceManifest {
        d_tag: WORKSPACE_D_TAG.to_string(),
        schema: RADROOTS_FARM_WORKSPACE_SCHEMA.to_string(),
        farm_group_id: GROUP_ID.to_string(),
        name: "Small Regen Farm".to_string(),
        owner_pubkey: "workspace_owner_pubkey".to_string(),
        farm: Some(FarmRef {
            pubkey: "farm_pubkey".to_string(),
            d_tag: FILE_D_TAG.to_string(),
        }),
        relays: vec![FarmWorkspaceRelay {
            url: "wss://relay.example.invalid/farm/field-group".to_string(),
            mode: FarmWorkspaceRelayMode::ReadWrite,
        }],
        media_servers: vec![FarmWorkspaceMediaServer {
            url: "https://media.example.invalid/farm/field-group".to_string(),
            service: "RadrootsPrivateMedia".to_string(),
        }],
        supported_kinds: vec![78, 30078, KIND_FARM_FILE_METADATA],
        protocol_version: RADROOTS_FARM_WORKSPACE_PROTOCOL_VERSION.to_string(),
        created_at_ms: 1_780_000_000_000,
        updated_at_ms: None,
    }
}

fn sample_crdt_change() -> FarmCrdtChange {
    FarmCrdtChange {
        schema: RADROOTS_FARM_CRDT_CHANGE_SCHEMA.to_string(),
        workspace: FarmWorkspaceRef {
            pubkey: "workspace_pubkey".to_string(),
            d_tag: WORKSPACE_D_TAG.to_string(),
        },
        farm_group_id: GROUP_ID.to_string(),
        document_id: DOCUMENT_ID.to_string(),
        document_kind: FarmCrdtDocumentKind::FarmTask,
        crdt_backend: CrdtBackend::Automerge,
        crdt_backend_version: Some("0.x".to_string()),
        actor_id: "actor_abc".to_string(),
        change_hash: "crdt_hash_abc".to_string(),
        dependencies: Vec::new(),
        encoded_change: "abc-DEF_012".to_string(),
        semantic_kind: FarmSemanticKind::FarmTaskCreate,
        business_time_ms: 1_780_000_000_000,
        author_member_id: Some("member_abc".to_string()),
        app_version: Some("0.1.0".to_string()),
    }
}

fn sample_file_metadata() -> FarmFileMetadata {
    FarmFileMetadata {
        d_tag: FILE_D_TAG.to_string(),
        workspace: FarmWorkspaceRef {
            pubkey: "workspace_pubkey".to_string(),
            d_tag: WORKSPACE_D_TAG.to_string(),
        },
        farm_group_id: GROUP_ID.to_string(),
        owner_document_id: DOCUMENT_ID.to_string(),
        owner_document_kind: FarmCrdtDocumentKind::FarmTask,
        caption: Some("Tomatoes harvested from Patch Y.".to_string()),
        url: "https://media.example.invalid/blob/sha256".to_string(),
        mime_type: "image/jpeg".to_string(),
        sha256: SHA256.to_string(),
        original_sha256: None,
        size_bytes: Some(123_456),
        dimensions: Some(FarmFileDimensions { w: 1600, h: 1200 }),
        blurhash: None,
        thumb: Some(FarmFileSource {
            url: "https://media.example.invalid/thumb/sha256".to_string(),
            mime_type: Some("image/jpeg".to_string()),
            dimensions: Some(FarmFileDimensions { w: 320, h: 240 }),
        }),
        image: None,
        alt: Some("Harvested tomatoes in a crate".to_string()),
        fallbacks: Vec::new(),
    }
}

fn sample_group_metadata() -> GroupEditableMetadata {
    GroupEditableMetadata {
        name: Some("Small Regen Farm".to_string()),
        about: Some("Field app group".to_string()),
        picture: Some("https://media.example.invalid/group.png".to_string()),
        is_private: false,
        is_restricted: true,
        is_closed: false,
        is_hidden: false,
        supported_kinds: Some(vec![78, 30078, KIND_FARM_FILE_METADATA]),
    }
}

fn without_tag(tags: &[Vec<String>], key: &str) -> Vec<Vec<String>> {
    tags.iter()
        .filter(|tag| tag.first().map(|value| value.as_str()) != Some(key))
        .cloned()
        .collect()
}

fn replace_tag_value(tags: &mut [Vec<String>], key: &str, value: &str) {
    for tag in tags {
        if tag.first().map(|tag_key| tag_key.as_str()) == Some(key) && tag.len() > 1 {
            tag[1] = value.to_string();
        }
    }
}
