use radroots_events::{
    kinds::{KIND_MESSAGE, KIND_POST},
    message::{RadrootsMessage, RadrootsMessageRecipient},
    RadrootsNostrEventPtr,
};
use radroots_events_codec::error::{EventEncodeError, EventParseError};
use radroots_events_codec::message::decode::{
    index_from_event, message_from_tags, metadata_from_event,
};
use radroots_events_codec::message::encode::{message_build_tags, to_wire_parts};

#[test]
fn message_build_tags_requires_recipients() {
    let message = RadrootsMessage {
        recipients: Vec::new(),
        content: "hello".to_string(),
        reply_to: None,
        subject: None,
    };

    let err = message_build_tags(&message).unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("recipients")
    ));
}

#[test]
fn message_build_tags_requires_recipient_pubkey() {
    let message = RadrootsMessage {
        recipients: vec![RadrootsMessageRecipient {
            public_key: "  ".to_string(),
            relay_url: None,
        }],
        content: "hello".to_string(),
        reply_to: None,
        subject: None,
    };

    let err = message_build_tags(&message).unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("recipients.public_key")
    ));
}

#[test]
fn message_to_wire_parts_requires_content() {
    let message = RadrootsMessage {
        recipients: vec![RadrootsMessageRecipient {
            public_key: "pub".to_string(),
            relay_url: None,
        }],
        content: "   ".to_string(),
        reply_to: None,
        subject: None,
    };

    let err = to_wire_parts(&message).unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("content")
    ));
}

#[test]
fn message_to_wire_parts_sets_tags() {
    let message = RadrootsMessage {
        recipients: vec![
            RadrootsMessageRecipient {
                public_key: "pub1".to_string(),
                relay_url: None,
            },
            RadrootsMessageRecipient {
                public_key: "pub2".to_string(),
                relay_url: Some("wss://relay.example".to_string()),
            },
        ],
        content: "hello".to_string(),
        reply_to: Some(RadrootsNostrEventPtr {
            id: "reply".to_string(),
            relays: Some("wss://reply.example".to_string()),
        }),
        subject: Some("topic".to_string()),
    };

    let parts = to_wire_parts(&message).unwrap();
    assert_eq!(parts.kind, KIND_MESSAGE);
    assert_eq!(parts.content, "hello");
    assert_eq!(
        parts.tags,
        vec![
            vec!["p".to_string(), "pub1".to_string()],
            vec![
                "p".to_string(),
                "pub2".to_string(),
                "wss://relay.example".to_string()
            ],
            vec![
                "e".to_string(),
                "reply".to_string(),
                "wss://reply.example".to_string()
            ],
            vec!["subject".to_string(), "topic".to_string()],
        ]
    );
}

#[test]
fn message_from_tags_requires_kind_content_and_recipients() {
    let tags = vec![vec!["p".to_string(), "pub".to_string()]];
    let err = message_from_tags(KIND_POST, &tags, "hello").unwrap_err();
    assert!(matches!(
        err,
        EventParseError::InvalidKind {
            expected: "14",
            got: KIND_POST
        }
    ));

    let err = message_from_tags(KIND_MESSAGE, &tags, "  ").unwrap_err();
    assert!(matches!(err, EventParseError::InvalidTag("content")));

    let err = message_from_tags(KIND_MESSAGE, &[], "hello").unwrap_err();
    assert!(matches!(err, EventParseError::MissingTag("p")));
}

#[test]
fn message_roundtrip_from_tags() {
    let tags = vec![
        vec!["p".to_string(), "pub1".to_string()],
        vec![
            "p".to_string(),
            "pub2".to_string(),
            "wss://relay.example".to_string(),
        ],
        vec![
            "e".to_string(),
            "reply".to_string(),
            "wss://reply.example".to_string(),
        ],
        vec!["subject".to_string(), "topic".to_string()],
    ];

    let message = message_from_tags(KIND_MESSAGE, &tags, "hello").unwrap();

    assert_eq!(message.recipients.len(), 2);
    assert_eq!(message.recipients[0].public_key, "pub1");
    assert_eq!(message.recipients[0].relay_url, None);
    assert_eq!(message.recipients[1].public_key, "pub2");
    assert_eq!(
        message.recipients[1].relay_url,
        Some("wss://relay.example".to_string())
    );
    assert_eq!(message.content, "hello");
    assert_eq!(
        message.reply_to.as_ref().map(|r| r.id.as_str()),
        Some("reply")
    );
    assert_eq!(
        message.reply_to.as_ref().and_then(|r| r.relays.as_deref()),
        Some("wss://reply.example")
    );
    assert_eq!(message.subject.as_deref(), Some("topic"));
}

#[test]
fn message_metadata_and_index_from_event_roundtrip() {
    let tags = vec![
        vec!["p".to_string(), "pub1".to_string()],
        vec![
            "p".to_string(),
            "pub2".to_string(),
            "wss://relay.example".to_string(),
        ],
        vec![
            "e".to_string(),
            "reply".to_string(),
            "wss://reply.example".to_string(),
        ],
        vec!["subject".to_string(), "topic".to_string()],
    ];
    let metadata = metadata_from_event(
        "id".to_string(),
        "author".to_string(),
        77,
        KIND_MESSAGE,
        "hello".to_string(),
        tags.clone(),
    )
    .unwrap();
    assert_eq!(metadata.id, "id");
    assert_eq!(metadata.author, "author");
    assert_eq!(metadata.published_at, 77);
    assert_eq!(metadata.kind, KIND_MESSAGE);
    assert_eq!(metadata.message.recipients.len(), 2);
    assert_eq!(metadata.message.content, "hello");
    assert_eq!(metadata.message.subject.as_deref(), Some("topic"));

    let index = index_from_event(
        "id".to_string(),
        "author".to_string(),
        77,
        KIND_MESSAGE,
        "hello".to_string(),
        tags,
        "sig".to_string(),
    )
    .unwrap();
    assert_eq!(index.event.kind, KIND_MESSAGE);
    assert_eq!(index.event.sig, "sig");
    assert_eq!(index.metadata.message.recipients.len(), 2);
}

#[test]
fn message_build_tags_rejects_invalid_optional_fields() {
    let message = RadrootsMessage {
        recipients: vec![RadrootsMessageRecipient {
            public_key: "pub".to_string(),
            relay_url: Some(" ".to_string()),
        }],
        content: "hello".to_string(),
        reply_to: None,
        subject: None,
    };
    let err = message_build_tags(&message).unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("recipients.relay_url")
    ));

    let message = RadrootsMessage {
        recipients: vec![RadrootsMessageRecipient {
            public_key: "pub".to_string(),
            relay_url: None,
        }],
        content: "hello".to_string(),
        reply_to: Some(RadrootsNostrEventPtr {
            id: " ".to_string(),
            relays: None,
        }),
        subject: None,
    };
    let err = message_build_tags(&message).unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("reply_to.id")
    ));

    let message = RadrootsMessage {
        recipients: vec![RadrootsMessageRecipient {
            public_key: "pub".to_string(),
            relay_url: None,
        }],
        content: "hello".to_string(),
        reply_to: None,
        subject: Some(" ".to_string()),
    };
    let err = message_build_tags(&message).unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("subject")
    ));
}

#[test]
fn message_from_tags_rejects_invalid_optional_tags() {
    let err = message_from_tags(
        KIND_MESSAGE,
        &[
            vec!["p".to_string(), "pub".to_string(), " ".to_string()],
            vec!["e".to_string(), "reply".to_string()],
        ],
        "hello",
    )
    .unwrap_err();
    assert!(matches!(err, EventParseError::InvalidTag("p")));

    let err = message_from_tags(
        KIND_MESSAGE,
        &[
            vec!["p".to_string(), "pub".to_string()],
            vec!["e".to_string(), " ".to_string()],
        ],
        "hello",
    )
    .unwrap_err();
    assert!(matches!(err, EventParseError::InvalidTag("e")));

    let err = message_from_tags(
        KIND_MESSAGE,
        &[
            vec!["p".to_string(), "pub".to_string()],
            vec!["subject".to_string(), " ".to_string()],
        ],
        "hello",
    )
    .unwrap_err();
    assert!(matches!(err, EventParseError::InvalidTag("subject")));
}
