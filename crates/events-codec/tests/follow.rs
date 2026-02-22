use radroots_events::{
    follow::{RadrootsFollow, RadrootsFollowProfile},
    kinds::{KIND_FOLLOW, KIND_POST},
};

use radroots_events_codec::error::{EventEncodeError, EventParseError};
use radroots_events_codec::follow::decode::{
    follow_from_tags, index_from_event, metadata_from_event,
};
use radroots_events_codec::follow::encode::{
    FollowMutation, follow_apply, follow_to_wire_parts_after, to_wire_parts,
    to_wire_parts_with_kind,
};

#[test]
fn follow_to_wire_parts_builds_p_tags() {
    let follow = RadrootsFollow {
        list: vec![RadrootsFollowProfile {
            published_at: 42,
            public_key: "pubkey".to_string(),
            relay_url: Some("wss://relay".to_string()),
            contact_name: Some("alice".to_string()),
        }],
    };

    let parts = to_wire_parts(&follow).unwrap();
    assert_eq!(parts.kind, KIND_FOLLOW);
    assert_eq!(parts.content, "");
    assert_eq!(parts.tags.len(), 1);

    let tag = &parts.tags[0];
    assert_eq!(tag[0], "p");
    assert_eq!(tag[1], "pubkey");
    assert_eq!(tag[2], "wss://relay");
    assert_eq!(tag[3], "alice");
}

#[test]
fn follow_to_wire_parts_requires_public_key() {
    let follow = RadrootsFollow {
        list: vec![RadrootsFollowProfile {
            published_at: 1,
            public_key: "  ".to_string(),
            relay_url: None,
            contact_name: None,
        }],
    };

    let err = to_wire_parts(&follow).unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("follow.public_key")
    ));
}

#[test]
fn follow_from_tags_defaults_published_at() {
    let tags = vec![vec!["p".to_string(), "pubkey".to_string()]];

    let follow = follow_from_tags(KIND_FOLLOW, &tags, 123).unwrap();
    assert_eq!(follow.list.len(), 1);
    assert_eq!(follow.list[0].published_at, 123);
    assert_eq!(follow.list[0].public_key, "pubkey");
    assert!(follow.list[0].relay_url.is_none());
    assert!(follow.list[0].contact_name.is_none());
}

#[test]
fn follow_from_tags_accepts_contact_without_relay() {
    let tags = vec![vec![
        "p".to_string(),
        "pubkey".to_string(),
        "alice".to_string(),
    ]];

    let follow = follow_from_tags(KIND_FOLLOW, &tags, 123).unwrap();
    assert_eq!(follow.list[0].published_at, 123);
    assert_eq!(follow.list[0].public_key, "pubkey");
    assert!(follow.list[0].relay_url.is_none());
    assert_eq!(follow.list[0].contact_name.as_deref(), Some("alice"));
}

#[test]
fn follow_from_tags_accepts_ws_relay_and_contact_name() {
    let tags = vec![vec![
        "p".to_string(),
        "pubkey".to_string(),
        "ws://relay.example.com".to_string(),
        "alice".to_string(),
    ]];

    let follow = follow_from_tags(KIND_FOLLOW, &tags, 123).unwrap();
    assert_eq!(
        follow.list[0].relay_url.as_deref(),
        Some("ws://relay.example.com")
    );
    assert_eq!(follow.list[0].contact_name.as_deref(), Some("alice"));
}

#[test]
fn follow_from_tags_uses_tag_published_at() {
    let tags = vec![vec![
        "p".to_string(),
        "pubkey".to_string(),
        "".to_string(),
        "".to_string(),
        "77".to_string(),
    ]];

    let follow = follow_from_tags(KIND_FOLLOW, &tags, 123).unwrap();
    assert_eq!(follow.list[0].published_at, 77);
}

#[test]
fn follow_from_tags_rejects_wrong_kind() {
    let tags = vec![vec!["p".to_string(), "pubkey".to_string()]];
    let err = follow_from_tags(KIND_POST, &tags, 123).unwrap_err();
    assert!(matches!(
        err,
        EventParseError::InvalidKind {
            expected: "3",
            got: KIND_POST
        }
    ));
}

#[test]
fn follow_from_tags_rejects_invalid_published_at_number() {
    let tags = vec![vec![
        "p".to_string(),
        "pubkey".to_string(),
        "".to_string(),
        "".to_string(),
        "not-a-number".to_string(),
    ]];
    let err = follow_from_tags(KIND_FOLLOW, &tags, 123).unwrap_err();
    assert!(matches!(err, EventParseError::InvalidNumber("p", _)));
}

#[test]
fn follow_metadata_and_index_from_event_roundtrip() {
    let tags = vec![vec![
        "p".to_string(),
        "pubkey".to_string(),
        "wss://relay.example.com".to_string(),
        "alice".to_string(),
        "88".to_string(),
    ]];
    let metadata = metadata_from_event(
        "id".to_string(),
        "author".to_string(),
        50,
        KIND_FOLLOW,
        "".to_string(),
        tags.clone(),
    )
    .unwrap();
    assert_eq!(metadata.id, "id");
    assert_eq!(metadata.author, "author");
    assert_eq!(metadata.published_at, 50);
    assert_eq!(metadata.kind, KIND_FOLLOW);
    assert_eq!(metadata.follow.list.len(), 1);
    assert_eq!(metadata.follow.list[0].published_at, 88);
    assert_eq!(metadata.follow.list[0].public_key, "pubkey");
    assert_eq!(
        metadata.follow.list[0].relay_url.as_deref(),
        Some("wss://relay.example.com")
    );
    assert_eq!(
        metadata.follow.list[0].contact_name.as_deref(),
        Some("alice")
    );

    let index = index_from_event(
        "id".to_string(),
        "author".to_string(),
        50,
        KIND_FOLLOW,
        "".to_string(),
        tags,
        "sig".to_string(),
    )
    .unwrap();
    assert_eq!(index.event.kind, KIND_FOLLOW);
    assert_eq!(index.event.sig, "sig");
    assert_eq!(index.metadata.follow.list.len(), 1);
}

#[test]
fn follow_index_from_event_propagates_parse_errors() {
    let err = index_from_event(
        "id".to_string(),
        "author".to_string(),
        50,
        KIND_POST,
        "".to_string(),
        Vec::new(),
        "sig".to_string(),
    )
    .unwrap_err();
    assert!(matches!(
        err,
        EventParseError::InvalidKind {
            expected: "3",
            got: KIND_POST
        }
    ));
}

#[test]
fn follow_apply_adds_and_updates_entries() {
    let follow = RadrootsFollow {
        list: vec![
            RadrootsFollowProfile {
                published_at: 1,
                public_key: "pubkey-a".to_string(),
                relay_url: None,
                contact_name: Some("alice".to_string()),
            },
            RadrootsFollowProfile {
                published_at: 1,
                public_key: "pubkey-b".to_string(),
                relay_url: None,
                contact_name: Some("bob".to_string()),
            },
        ],
    };

    let updated = follow_apply(
        &follow,
        FollowMutation::Follow {
            public_key: "pubkey-a".to_string(),
            relay_url: Some("wss://relay".to_string()),
            contact_name: Some("alice-updated".to_string()),
        },
    )
    .unwrap();
    assert_eq!(updated.list.len(), 2);
    assert_eq!(updated.list[0].public_key, "pubkey-a");
    assert_eq!(updated.list[0].relay_url.as_deref(), Some("wss://relay"));
    assert_eq!(
        updated.list[0].contact_name.as_deref(),
        Some("alice-updated")
    );

    let added = follow_apply(
        &follow,
        FollowMutation::Follow {
            public_key: "pubkey-c".to_string(),
            relay_url: None,
            contact_name: Some("cara".to_string()),
        },
    )
    .unwrap();
    assert_eq!(added.list.len(), 3);
    assert_eq!(added.list[2].public_key, "pubkey-c");
}

#[test]
fn follow_apply_unfollow_removes_entries() {
    let follow = RadrootsFollow {
        list: vec![
            RadrootsFollowProfile {
                published_at: 1,
                public_key: "pubkey-a".to_string(),
                relay_url: None,
                contact_name: None,
            },
            RadrootsFollowProfile {
                published_at: 1,
                public_key: "pubkey-b".to_string(),
                relay_url: None,
                contact_name: None,
            },
        ],
    };

    let removed = follow_apply(
        &follow,
        FollowMutation::Unfollow {
            public_key: "pubkey-b".to_string(),
        },
    )
    .unwrap();
    assert_eq!(removed.list.len(), 1);
    assert_eq!(removed.list[0].public_key, "pubkey-a");
}

#[test]
fn follow_apply_toggle_adds_or_removes() {
    let follow = RadrootsFollow {
        list: vec![RadrootsFollowProfile {
            published_at: 1,
            public_key: "pubkey-a".to_string(),
            relay_url: None,
            contact_name: None,
        }],
    };

    let removed = follow_apply(
        &follow,
        FollowMutation::Toggle {
            public_key: "pubkey-a".to_string(),
            relay_url: None,
            contact_name: None,
        },
    )
    .unwrap();
    assert!(removed.list.is_empty());

    let added = follow_apply(
        &follow,
        FollowMutation::Toggle {
            public_key: "pubkey-b".to_string(),
            relay_url: None,
            contact_name: Some("bob".to_string()),
        },
    )
    .unwrap();
    assert_eq!(added.list.len(), 2);
    assert_eq!(added.list[1].public_key, "pubkey-b");
}

#[test]
fn follow_apply_rejects_empty_pubkey() {
    let follow = RadrootsFollow { list: Vec::new() };
    let err = follow_apply(
        &follow,
        FollowMutation::Follow {
            public_key: "  ".to_string(),
            relay_url: None,
            contact_name: None,
        },
    )
    .unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("follow.public_key")
    ));
}

#[test]
fn follow_build_tags_normalizes_empty_optional_values() {
    let follow = RadrootsFollow {
        list: vec![RadrootsFollowProfile {
            published_at: 1,
            public_key: "pubkey".to_string(),
            relay_url: Some("".to_string()),
            contact_name: Some(" ".to_string()),
        }],
    };
    let parts = to_wire_parts(&follow).unwrap();
    assert_eq!(
        parts.tags,
        vec![vec!["p".to_string(), "pubkey".to_string(), " ".to_string()]]
    );
}

#[test]
fn follow_to_wire_parts_with_kind_and_after_mutation_work() {
    let follow = RadrootsFollow {
        list: vec![RadrootsFollowProfile {
            published_at: 1,
            public_key: "pubkey-a".to_string(),
            relay_url: None,
            contact_name: None,
        }],
    };
    let parts = to_wire_parts_with_kind(&follow, KIND_POST).unwrap();
    assert_eq!(parts.kind, KIND_POST);

    let toggled = follow_to_wire_parts_after(
        &follow,
        FollowMutation::Toggle {
            public_key: "pubkey-b".to_string(),
            relay_url: Some("wss://relay.example.com".to_string()),
            contact_name: Some("alice".to_string()),
        },
    )
    .unwrap();
    assert_eq!(toggled.kind, KIND_FOLLOW);
    assert_eq!(toggled.tags.len(), 2);
}

#[test]
fn follow_apply_normalizes_optional_fields_and_deduplicates_existing_list() {
    let follow = RadrootsFollow {
        list: vec![
            RadrootsFollowProfile {
                published_at: 1,
                public_key: " pubkey-a ".to_string(),
                relay_url: Some(" ".to_string()),
                contact_name: Some(" ".to_string()),
            },
            RadrootsFollowProfile {
                published_at: 2,
                public_key: "pubkey-a".to_string(),
                relay_url: Some("wss://duplicate.example.com".to_string()),
                contact_name: Some("duplicate".to_string()),
            },
        ],
    };

    let updated = follow_apply(
        &follow,
        FollowMutation::Follow {
            public_key: "pubkey-a".to_string(),
            relay_url: Some(" ".to_string()),
            contact_name: Some(" ".to_string()),
        },
    )
    .unwrap();

    assert_eq!(updated.list.len(), 1);
    assert_eq!(updated.list[0].public_key, "pubkey-a");
    assert!(updated.list[0].relay_url.is_none());
    assert!(updated.list[0].contact_name.is_none());
}

#[test]
fn follow_apply_follow_with_none_preserves_existing_values() {
    let follow = RadrootsFollow {
        list: vec![RadrootsFollowProfile {
            published_at: 1,
            public_key: "pubkey-a".to_string(),
            relay_url: Some("wss://relay.example.com".to_string()),
            contact_name: Some("alice".to_string()),
        }],
    };

    let updated = follow_apply(
        &follow,
        FollowMutation::Follow {
            public_key: "pubkey-a".to_string(),
            relay_url: None,
            contact_name: None,
        },
    )
    .unwrap();
    assert_eq!(updated.list.len(), 1);
    assert_eq!(
        updated.list[0].relay_url.as_deref(),
        Some("wss://relay.example.com")
    );
    assert_eq!(updated.list[0].contact_name.as_deref(), Some("alice"));
}
