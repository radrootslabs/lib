use radroots_events::{
    RadrootsNostrEventPtr,
    message::{RadrootsMessage, RadrootsMessageRecipient},
};
use radroots_events_codec::error::{EventEncodeError, EventParseError};
use radroots_events_codec::message::decode::message_from_tags;
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
    assert_eq!(parts.kind, 14);
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
    let err = message_from_tags(1, &tags, "hello").unwrap_err();
    assert!(matches!(
        err,
        EventParseError::InvalidKind { expected: "14", got: 1 }
    ));

    let err = message_from_tags(14, &tags, "  ").unwrap_err();
    assert!(matches!(err, EventParseError::InvalidTag("content")));

    let err = message_from_tags(14, &[], "hello").unwrap_err();
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

    let message = message_from_tags(14, &tags, "hello").unwrap();

    assert_eq!(message.recipients.len(), 2);
    assert_eq!(message.recipients[0].public_key, "pub1");
    assert_eq!(message.recipients[0].relay_url, None);
    assert_eq!(message.recipients[1].public_key, "pub2");
    assert_eq!(
        message.recipients[1].relay_url,
        Some("wss://relay.example".to_string())
    );
    assert_eq!(message.content, "hello");
    assert_eq!(message.reply_to.as_ref().map(|r| r.id.as_str()), Some("reply"));
    assert_eq!(
        message.reply_to.as_ref().and_then(|r| r.relays.as_deref()),
        Some("wss://reply.example")
    );
    assert_eq!(message.subject.as_deref(), Some("topic"));
}
