#![cfg(feature = "chat")]

use radroots_domain::{PublicKey, UnixSeconds};
use radroots_protocol::{chat::*, command::CapturedChatCommand, nip01::*};
use serde_json::Value;

fn vectors() -> Value {
    serde_json::from_str(include_str!("fixtures/chat_wire.v1.json")).unwrap()
}
fn time(value: u64) -> UnixSeconds {
    UnixSeconds::new(value).unwrap()
}
fn text<'a>(row: &'a Value, key: &str) -> &'a str {
    row[key].as_str().unwrap()
}

#[test]
fn independent_wire_vectors_preserve_plaintext_tags_recipient_set_and_exact_seal() {
    for row in vectors()["vectors"].as_array().unwrap() {
        let command = CapturedChatCommand::parse(text(row, "wire").as_bytes()).unwrap();
        let rumor = ChatRumor::from_command(&command, time(1_700_000_000)).unwrap();
        assert_eq!(rumor.json(), text(row, "rumor"));
        assert_eq!(rumor.message_id().to_string(), text(row, "message_id"));
        assert_eq!(rumor.author(), command.context().actor().participant());
        assert_eq!(rumor.created_at(), time(1_700_000_000));
        let recipients: Vec<_> = rumor.recipients().iter().map(|key| key.to_hex()).collect();
        assert_eq!(serde_json::to_value(recipients).unwrap(), row["recipients"]);
        let json: Value = serde_json::from_str(rumor.json()).unwrap();
        assert!(json.get("sig").is_none());
        assert_eq!(json["content"], command.text());
        for recipient in rumor.recipients() {
            let seal = ChatSeal::new(
                &rumor,
                *recipient,
                time(1_699_999_990),
                text(row, "structural_ciphertext"),
            )
            .unwrap();
            assert_eq!(
                canonical_preimage(&seal.fields()),
                text(row, "seal_preimage")
            );
            assert_eq!(
                signed_json_len(&seal.fields(), SIGNED_MAX_BYTES).unwrap() as u64,
                row["signed_seal_bytes"].as_u64().unwrap()
            );
            assert_eq!(seal.recipient(), *recipient);
            let encoded = UnsignedEvent::encode(&seal.fields(), SIGNED_MAX_BYTES).unwrap();
            assert_eq!(seal.event_id(), encoded.event_id());
            assert_eq!(
                serde_json::from_str::<Value>(encoded.json()).unwrap()["id"],
                row["seal_id"]
            );
            assert!(!format!("{seal:?}").contains(text(row, "structural_ciphertext")));
        }
        assert!(!format!("{rumor:?}").contains(command.text()));
    }
}

#[test]
fn complete_nested_budget_and_padding_match_independent_serialized_objects() {
    let data = vectors();
    let key = PublicKey::from_hex(text(&data["canonical"][0], "author")).unwrap();
    for row in data["budgets"].as_array().unwrap() {
        let budget =
            ChatBudget::for_rumor(row["rumor_bytes"].as_u64().unwrap() as usize, key).unwrap();
        assert_eq!(
            budget.rumor_ciphertext_bytes() as u64,
            row["rumor_ciphertext_bytes"].as_u64().unwrap()
        );
        assert_eq!(
            budget.signed_seal_max_bytes() as u64,
            row["signed_seal_max_bytes"].as_u64().unwrap()
        );
        assert_eq!(
            budget.envelope_ciphertext_max_bytes() as u64,
            row["envelope_ciphertext_max_bytes"].as_u64().unwrap()
        );
        assert_eq!(
            budget.signed_envelope_max_bytes() as u64,
            row["signed_envelope_max_bytes"].as_u64().unwrap()
        );
    }
    for row in data["padding"].as_array().unwrap() {
        assert_eq!(
            nip44_v2_payload_len(row["plaintext_bytes"].as_u64().unwrap() as usize).unwrap() as u64,
            row["encoded_bytes"].as_u64().unwrap()
        );
    }
    for size in [0, 65_536, usize::MAX] {
        assert_eq!(nip44_v2_payload_len(size), Err(WireError::InvalidInput));
    }
    assert_eq!(ChatBudget::for_rumor(0, key), Err(WireError::InvalidInput));
    assert_eq!(
        ChatBudget::for_rumor(RUMOR_MAX_BYTES + 1, key),
        Err(WireError::LimitExceeded)
    );
}

#[test]
fn canonical_wire_preserves_original_escaping_and_full_legacy_integer_range() {
    for row in vectors()["canonical"].as_array().unwrap() {
        let tags: Vec<Vec<String>> = serde_json::from_value(row["tags"].clone()).unwrap();
        let fields = EventFields {
            author: PublicKey::from_hex(text(row, "author")).unwrap(),
            created_at: text(row, "created_at").parse().unwrap(),
            kind: row["kind"].as_u64().unwrap() as u32,
            tags: &tags,
            content: text(row, "content"),
        };
        let expected = text(row, "preimage");
        assert_eq!(canonical_preimage(&fields), expected);
        assert_eq!(
            preimage_len(&fields, expected.len()).unwrap(),
            expected.len()
        );
        assert_eq!(
            preimage_len(&fields, expected.len() - 1),
            Err(WireError::LimitExceeded)
        );
        let signed_size = row["signed_json_bytes"].as_u64().unwrap() as usize;
        assert_eq!(signed_json_len(&fields, signed_size).unwrap(), signed_size);
        assert_eq!(
            signed_json_len(&fields, signed_size - 1),
            Err(WireError::LimitExceeded)
        );
        let unsigned = UnsignedEvent::encode(&fields, expected.len() + 116).unwrap();
        assert_eq!(unsigned.json().len(), expected.len() + 116);
        assert_eq!(
            serde_json::from_str::<Value>(unsigned.json()).unwrap()["id"],
            row["event_id"]
        );
        assert!(matches!(
            UnsignedEvent::encode(&fields, expected.len() + 115),
            Err(WireError::LimitExceeded)
        ));
        assert!(matches!(
            UnsignedEvent::encode(&fields, 0),
            Err(WireError::LimitExceeded)
        ));
        assert!(
            !format!("{fields:?} {unsigned:?}").contains(fields.content)
                || fields.content.is_empty()
        );
    }
}

#[test]
fn noncanonical_base64_padding_version_and_alphabet_are_refused_before_seal_capture() {
    for row in vectors()["vectors"].as_array().unwrap() {
        let command = CapturedChatCommand::parse(text(row, "wire").as_bytes()).unwrap();
        let rumor = ChatRumor::from_command(&command, time(1_700_000_000)).unwrap();
        let original = text(row, "structural_ciphertext").as_bytes();
        let mut changes = Vec::new();
        let mut invalid = original.to_vec();
        invalid[2] = b'!';
        changes.push(invalid);
        let mut wrong_version = original.to_vec();
        wrong_version[1] = b'Q';
        changes.push(wrong_version);
        let padding = original
            .iter()
            .rev()
            .take_while(|byte| **byte == b'=')
            .count();
        if padding != 0 {
            let mut missing_padding = original.to_vec();
            *missing_padding.last_mut().unwrap() = b'A';
            changes.push(missing_padding);
            let mut nonzero_tail = original.to_vec();
            nonzero_tail[original.len() - padding - 1] = b'B';
            changes.push(nonzero_tail);
        }
        for changed in changes {
            assert!(matches!(
                ChatSeal::new(
                    &rumor,
                    rumor.recipients()[0],
                    time(1_699_999_990),
                    core::str::from_utf8(&changed).unwrap()
                ),
                Err(WireError::InvalidInput)
            ));
        }
    }
}

#[test]
fn rumor_and_seal_refuse_invalid_time_recipient_nested_size_and_ciphertext_shape() {
    let data = vectors();
    let row = &data["vectors"][0];
    let command = CapturedChatCommand::parse(text(row, "wire").as_bytes()).unwrap();
    assert!(matches!(
        ChatRumor::from_command(&command, time(0)),
        Err(WireError::InvalidInput)
    ));
    let rumor = ChatRumor::from_command(&command, time(1_700_000_000)).unwrap();
    let peer = rumor.recipients()[0];
    for timestamp in [0, 1_700_000_001, 1_700_000_000 - 172_801] {
        assert!(matches!(
            ChatSeal::new(
                &rumor,
                peer,
                time(timestamp),
                text(row, "structural_ciphertext")
            ),
            Err(WireError::InvalidInput)
        ));
    }
    let stranger =
        PublicKey::from_hex("f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9")
            .unwrap();
    assert!(matches!(
        ChatSeal::new(
            &rumor,
            stranger,
            time(1_699_999_990),
            text(row, "structural_ciphertext")
        ),
        Err(WireError::InvalidInput)
    ));
    for change in [
        "",
        "#unsupported",
        "AA",
        &"A".repeat(text(row, "structural_ciphertext").len()),
    ] {
        assert!(matches!(
            ChatSeal::new(&rumor, peer, time(1_699_999_990), change),
            Err(WireError::InvalidInput)
        ));
    }
    let mut wire: Value = serde_json::from_str(text(row, "wire")).unwrap();
    wire["input"]["text"] = Value::String("\u{0000}".repeat(4096));
    let expanded = CapturedChatCommand::parse(&serde_json::to_vec(&wire).unwrap()).unwrap();
    assert!(matches!(
        ChatRumor::from_command(&expanded, time(1_700_000_000)),
        Err(WireError::LimitExceeded)
    ));
    wire["input"]["text"] = Value::String("a".repeat(4096));
    let bounded = CapturedChatCommand::parse(&serde_json::to_vec(&wire).unwrap()).unwrap();
    assert!(ChatRumor::from_command(&bounded, time(1_700_000_000)).is_ok());
}
