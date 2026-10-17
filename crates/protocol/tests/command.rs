#![cfg(feature = "commands")]

use radroots_domain::{
    ActorRef, ActorRole, Approval, AuthorityRef, CommandContextData, CorrelationId, ExecutorId,
    Generation, GrantId, MessageId, OperationId, PublicKey, RouteDigest, SourceId, ThreadId,
    UnixSeconds, WorkspaceId,
};
use radroots_protocol::command::{
    CHAT_MAX_BYTES, COMMAND_MAX_BYTES, CapturedChatCommand, ChatKind, CommandError, InquireInput,
    ListingRevisionRef, ReplyInput,
};
use serde::Deserialize;
use serde_json::{Value, json};

fn alice() -> PublicKey {
    PublicKey::from_hex("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798").unwrap()
}
fn bob() -> PublicKey {
    PublicKey::from_hex("c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5").unwrap()
}
fn context(correlation: Option<&str>, approval: Option<Approval>) -> CommandContextData {
    CommandContextData::new(
        OperationId::from_bytes([1; 32]),
        ActorRef::new(
            alice(),
            ActorRole::Buyer,
            None,
            Some(GrantId::from_bytes([2; 32])),
        ),
        WorkspaceId::from_bytes([3; 32]),
        AuthorityRef::new(
            ExecutorId::from_bytes([4; 32]),
            Generation::new(9_007_199_254_740_993).unwrap(),
        ),
        RouteDigest::from_bytes([5; 32]),
        approval,
        None,
        UnixSeconds::new(200).unwrap(),
        correlation.map(|v| CorrelationId::new(v).unwrap()),
    )
    .unwrap()
}
fn inquiry(
    correlation: Option<&str>,
    approval: Option<Approval>,
    text: &str,
) -> CapturedChatCommand {
    CapturedChatCommand::inquire(
        context(correlation, approval),
        InquireInput::new(ThreadId::from_bytes([6; 32]), bob(), None, text).unwrap(),
    )
    .unwrap()
}
#[derive(Deserialize)]
struct Vectors {
    vectors: Vec<Vector>,
}
#[derive(Deserialize)]
struct Vector {
    name: String,
    wire: String,
    command_digest: String,
    approval_intent_digest: String,
}

#[test]
fn independent_ecmascript_vectors_match_exact_bytes_and_both_hash_domains() {
    let vectors: Vectors =
        serde_json::from_str(include_str!("fixtures/chat_commands.v1.json")).unwrap();
    assert_eq!(vectors.vectors.len(), 3);
    for vector in vectors.vectors {
        let command = CapturedChatCommand::parse(vector.wire.as_bytes()).unwrap();
        assert_eq!(
            command.protected_wire(),
            vector.wire.as_bytes(),
            "{}",
            vector.name
        );
        assert_eq!(
            command.digest().to_string(),
            vector.command_digest,
            "{}",
            vector.name
        );
        assert_eq!(
            command.approval_intent_digest().to_string(),
            vector.approval_intent_digest,
            "{}",
            vector.name
        );
        assert_eq!(
            command.context().authority().generation().get(),
            9_007_199_254_740_993
        );
        assert_eq!(command.peer(), bob());
        assert_eq!(command.thread_id(), ThreadId::from_bytes([6; 32]));
        match command.kind() {
            ChatKind::Inquire => assert_eq!(command.parent(), None),
            ChatKind::Reply => assert_eq!(command.parent(), Some(MessageId::from_bytes([11; 32]))),
        }
        if let Some(source) = command.source() {
            assert_eq!(source.author(), bob());
            assert_eq!(source.d(), "55".repeat(16));
            assert_eq!(source.event_id(), SourceId::from_bytes([10; 32]));
        }
    }
}

#[test]
fn correlation_is_diagnostic_and_approval_is_bound_only_to_the_final_command() {
    let original = inquiry(Some("original"), None, "private");
    let other = inquiry(Some("other"), None, "private");
    let absent = inquiry(None, None, "private");
    assert_ne!(original.protected_wire(), other.protected_wire());
    assert_eq!(original.digest(), other.digest());
    assert_eq!(original.digest(), absent.digest());
    let approval = Approval::new(
        original.approval_intent_digest(),
        "inbox.inquire",
        UnixSeconds::new(190).unwrap(),
    )
    .unwrap();
    let approved = inquiry(Some("other"), Some(approval), "private");
    assert_eq!(
        original.approval_intent_digest(),
        approved.approval_intent_digest()
    );
    assert_ne!(original.digest(), approved.digest());
    assert_eq!(
        CapturedChatCommand::parse(approved.protected_wire())
            .unwrap()
            .digest(),
        approved.digest()
    );
    assert!(!format!("{approved:?}").contains("private"));
    assert_eq!(format!("{approved:?}"), "CapturedChatCommand(<redacted>)");
}

#[test]
fn each_semantic_change_changes_the_command_commitment_without_unicode_normalization() {
    let original = inquiry(None, None, "Å");
    let decomposed = inquiry(None, None, "A\u{030a}");
    assert_ne!(original.digest(), decomposed.digest());
    let base: Value = serde_json::from_slice(original.protected_wire()).unwrap();
    for (pointer, replacement) in [
        ("/context/operation_id", json!("10".repeat(32))),
        ("/context/actor/role", json!("seller")),
        ("/context/actor/participant", json!(bob().to_hex())),
        ("/context/actor/organization", json!(bob().to_hex())),
        ("/context/actor/grant_id", json!("11".repeat(32))),
        ("/context/workspace_id", json!("12".repeat(32))),
        ("/context/authority/executor_id", json!("13".repeat(32))),
        ("/context/authority/generation", json!("2")),
        ("/context/route_digest", json!("14".repeat(32))),
        ("/context/not_after", json!("201")),
        ("/input/thread_id", json!("15".repeat(32))),
        ("/input/peer", json!(alice().to_hex())),
        ("/input/text", json!("different")),
    ] {
        let mut changed = base.clone();
        *changed.pointer_mut(pointer).unwrap() = replacement;
        let changed = CapturedChatCommand::parse(&serde_json::to_vec(&changed).unwrap()).unwrap();
        assert_ne!(changed.digest(), original.digest(), "{pointer}");
    }
    let source =
        ListingRevisionRef::new(bob(), &"55".repeat(16), SourceId::from_bytes([10; 32])).unwrap();
    let sourced = CapturedChatCommand::inquire(
        context(None, None),
        InquireInput::new(ThreadId::from_bytes([6; 32]), bob(), Some(source), "Å").unwrap(),
    )
    .unwrap();
    assert_ne!(sourced.digest(), original.digest());
    let reply = CapturedChatCommand::reply(
        context(None, None),
        ReplyInput::new(
            ThreadId::from_bytes([6; 32]),
            bob(),
            MessageId::from_bytes([11; 32]),
            "reply",
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        CapturedChatCommand::parse(reply.protected_wire())
            .unwrap()
            .digest(),
        reply.digest()
    );
}

#[test]
fn closed_json_requires_explicit_null_and_rejects_duplicates_aliases_unknowns_and_bad_sources() {
    let command = inquiry(None, None, "private");
    let wire = std::str::from_utf8(command.protected_wire()).unwrap();
    let duplicate = wire.replacen(
        "\"type\":\"inbox.inquire\"",
        "\"type\":\"inbox.inquire\",\"t\\u0079pe\":\"inbox.inquire\"",
        1,
    );
    assert!(matches!(
        CapturedChatCommand::parse(duplicate.as_bytes()),
        Err(CommandError::InvalidInput)
    ));
    let duplicate = wire.replacen(
        "\"text\":\"private\"",
        "\"text\":\"private\",\"te\\u0078t\":\"other\"",
        1,
    );
    assert!(matches!(
        CapturedChatCommand::parse(duplicate.as_bytes()),
        Err(CommandError::InvalidInput)
    ));
    let base: Value = serde_json::from_slice(command.protected_wire()).unwrap();
    for pointer in [
        "/context/correlation_id",
        "/context/approval",
        "/context/expected",
        "/input/source",
    ] {
        let mut value = base.clone();
        let (parent, key) = pointer.rsplit_once('/').unwrap();
        value
            .pointer_mut(parent)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove(key);
        assert!(
            matches!(
                CapturedChatCommand::parse(&serde_json::to_vec(&value).unwrap()),
                Err(CommandError::InvalidInput)
            ),
            "{pointer}"
        );
    }
    let mut extra = base.clone();
    extra["extra"] = json!(true);
    assert!(matches!(
        CapturedChatCommand::parse(&serde_json::to_vec(&extra).unwrap()),
        Err(CommandError::InvalidInput)
    ));
    let mut extra = base.clone();
    extra["input"]["extra"] = json!(true);
    assert!(matches!(
        CapturedChatCommand::parse(&serde_json::to_vec(&extra).unwrap()),
        Err(CommandError::InvalidInput)
    ));
    let mut unsupported = base.clone();
    unsupported["type"] = json!("orders.accept");
    assert!(matches!(
        CapturedChatCommand::parse(&serde_json::to_vec(&unsupported).unwrap()),
        Err(CommandError::UnsupportedCommand)
    ));
    unsupported["type"] = json!("x".repeat(81));
    assert!(matches!(
        CapturedChatCommand::parse(&serde_json::to_vec(&unsupported).unwrap()),
        Err(CommandError::LimitExceeded)
    ));
    for d in ["f".repeat(31), "F".repeat(32), "é".repeat(16)] {
        assert!(matches!(
            ListingRevisionRef::new(bob(), &d, SourceId::from_bytes([10; 32])),
            Err(CommandError::InvalidInput)
        ));
    }
    let mut source = base;
    source["input"]["source"] = json!({"coordinate":{"kind":30403,"author":bob().to_hex(),"d":"55".repeat(16)},"event_id":"0a".repeat(32)});
    assert!(matches!(
        CapturedChatCommand::parse(&serde_json::to_vec(&source).unwrap()),
        Err(CommandError::InvalidInput)
    ));
    assert!(
        !format!(
            "{} {:?}",
            CommandError::InvalidInput,
            CommandError::LimitExceeded
        )
        .contains("private")
    );
}

#[test]
fn byte_and_depth_budgets_are_enforced_before_untrusted_json_materialization() {
    let full = "é".repeat(CHAT_MAX_BYTES / 2);
    let command = inquiry(None, None, &full);
    assert_eq!(
        CapturedChatCommand::parse(command.protected_wire())
            .unwrap()
            .text(),
        full
    );
    assert!(matches!(
        InquireInput::new(
            ThreadId::from_bytes([6; 32]),
            bob(),
            None,
            &(full.clone() + "é")
        ),
        Err(CommandError::LimitExceeded)
    ));
    assert!(matches!(
        ReplyInput::new(
            ThreadId::from_bytes([6; 32]),
            bob(),
            MessageId::from_bytes([11; 32]),
            &"x".repeat(CHAT_MAX_BYTES + 1)
        ),
        Err(CommandError::LimitExceeded)
    ));
    assert_eq!(inquiry(None, None, "").text(), "");
    let mut raw = command.protected_wire().to_vec();
    raw.resize(COMMAND_MAX_BYTES + 1, b' ');
    assert!(matches!(
        CapturedChatCommand::parse(&raw),
        Err(CommandError::LimitExceeded)
    ));
    for (depth, expected) in [
        (16, CommandError::InvalidInput),
        (17, CommandError::LimitExceeded),
    ] {
        let nested = format!("{}0{}", "[".repeat(depth), "]".repeat(depth));
        assert!(
            matches!(CapturedChatCommand::parse(nested.as_bytes()), Err(error) if error == expected)
        );
    }
    let bracket_text = "[ { \\\" ] }";
    let command = inquiry(None, None, bracket_text);
    assert_eq!(
        CapturedChatCommand::parse(command.protected_wire())
            .unwrap()
            .text(),
        bracket_text
    );
    for raw in [b"}".as_slice(), b"{", b"\"unterminated", b"\xff"] {
        assert!(matches!(
            CapturedChatCommand::parse(raw),
            Err(CommandError::InvalidInput)
        ));
    }
    let mut too_long: Value = serde_json::from_slice(command.protected_wire()).unwrap();
    too_long["input"]["text"] = json!("x".repeat(CHAT_MAX_BYTES + 1));
    assert!(matches!(
        CapturedChatCommand::parse(&serde_json::to_vec(&too_long).unwrap()),
        Err(CommandError::LimitExceeded)
    ));
}
