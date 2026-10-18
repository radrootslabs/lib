#![cfg(feature = "chat")]
use radroots_domain::UnixSeconds;
use radroots_protocol::{chat::ChatRumor, command::CapturedChatCommand};
use serde_json::Value;

#[test]
fn distinct_operations_with_identical_text_in_one_second_have_distinct_logical_messages() {
    let vectors: Value =
        serde_json::from_str(include_str!("fixtures/chat_commands.v1.json")).unwrap();
    let mut source: Value =
        serde_json::from_str(vectors["vectors"][0]["wire"].as_str().unwrap()).unwrap();
    let first = CapturedChatCommand::parse(source.to_string().as_bytes()).unwrap();
    source["context"]["operation_id"] = "42".repeat(32).into();
    let second = CapturedChatCommand::parse(source.to_string().as_bytes()).unwrap();
    assert_ne!(first.digest(), second.digest());
    let captured = UnixSeconds::new(1_700_000_000).unwrap();
    let original = ChatRumor::from_command(&first, captured).unwrap();
    let replay = ChatRumor::from_command(&first, captured).unwrap();
    let distinct = ChatRumor::from_command(&second, captured).unwrap();
    assert_eq!(original.json(), replay.json());
    assert_eq!(original.message_id(), replay.message_id());
    assert_ne!(original.message_id(), distinct.message_id());
}
