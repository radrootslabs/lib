use radroots_domain::{
    CHAT_MAX_BYTES, ChatMessage, ChatRelation, MessageError, MessageId, PublicKey, SourceId,
    ThreadId, UnixSeconds,
};

fn message(text: &str, relation: ChatRelation) -> Result<ChatMessage, MessageError> {
    ChatMessage::new(
        MessageId::from_bytes([0x11; 32]),
        ThreadId::from_bytes([0x22; 32]),
        PublicKey::from_hex("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798")
            .unwrap(),
        PublicKey::from_hex("c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5")
            .unwrap(),
        UnixSeconds::new(100).unwrap(),
        relation,
        text,
    )
}

#[test]
fn private_logical_views_preserve_exact_relations_and_utf8_bounds_without_diagnostics() {
    let relation = ChatRelation::Inquire {
        source: Some(SourceId::from_bytes([0x33; 32])),
    };
    let text = "🥕".repeat(CHAT_MAX_BYTES / 4);
    let value = message(&text, relation).unwrap();
    let owned = value.clone();
    drop(value);
    assert_eq!(owned.text(), text);
    assert_eq!(owned.id(), MessageId::from_bytes([0x11; 32]));
    assert_eq!(owned.thread_id(), ThreadId::from_bytes([0x22; 32]));
    assert_ne!(owned.author(), owned.peer());
    assert_eq!(owned.created_at().get(), 100);
    assert_eq!(owned.relation(), relation);
    assert_eq!(format!("{owned:?}"), "ChatMessage(<redacted>)");
    assert_eq!(format!("{relation:?}"), "ChatRelation(<redacted>)");
    assert!(matches!(
        message(&(text + "x"), relation),
        Err(MessageError::LimitExceeded)
    ));
    assert_eq!(
        MessageError::LimitExceeded.to_string(),
        "chat text limit exceeded"
    );

    let parent = MessageId::from_bytes([0x44; 32]);
    let reply = message("", ChatRelation::Reply { parent }).unwrap();
    assert_eq!(reply.text(), "");
    assert_eq!(reply.relation(), ChatRelation::Reply { parent });
    assert_eq!(
        message("e\u{301}\0\n", relation).unwrap().text(),
        "e\u{301}\0\n"
    );
}
