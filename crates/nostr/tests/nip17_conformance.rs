#![cfg(feature = "nip17")]

use nostr::{Keys, SecretKey};
use radroots_event::social::message::{Message, MessageRecipient};
use radroots_event::social::message_file::{MessageFile, MessageFileDimensions};
use radroots_nostr::nip17::{
    RadrootsNip17Rumor, RadrootsNip17WrapOptions, radroots_nostr_unwrap_gift_wrap,
    radroots_nostr_wrap_message, radroots_nostr_wrap_message_file,
};
use radroots_test_fixtures::{
    FIXTURE_ALICE_PUBLIC_KEY_HEX, FIXTURE_ALICE_SECRET_KEY_HEX, FIXTURE_BOB_PUBLIC_KEY_HEX,
    FIXTURE_BOB_SECRET_KEY_HEX,
};
use serde::Deserialize;
use serde_json::Value;
use std::{borrow::Cow, fs, path::Path};

const PACKAGED_VECTORS: &str = include_str!("fixtures/nip17_adapter.v1.json");
const WORKSPACE_VECTOR_PATH: &str = "../../contracts/conformance/vectors/nip17/adapter.v1.json";
const WORKSPACE_CONTRACT_MARKER_PATH: &str = "../../contracts/manifest.toml";

#[derive(Debug, Deserialize)]
struct Suite {
    suite: String,
    contract_version: String,
    vectors: Vec<Vector>,
}

#[derive(Debug, Deserialize)]
struct Vector {
    id: String,
    kind: String,
    input: Value,
    expected: Value,
}

#[tokio::test]
async fn checked_in_nip17_vectors_execute_against_public_api() {
    let vectors = conformance_vectors();
    let suite: Suite = serde_json::from_str(&vectors).expect("NIP-17 vectors must parse");
    assert_eq!(suite.suite, "nip17_adapter");
    assert_eq!(suite.contract_version, "1.0.0");
    assert!(!suite.vectors.is_empty());

    for vector in &suite.vectors {
        execute_vector(vector).await;
    }
}

fn conformance_vectors() -> Cow<'static, str> {
    let workspace_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(WORKSPACE_VECTOR_PATH);
    match fs::read_to_string(&workspace_path) {
        Ok(canonical) => {
            assert_eq!(
                canonical,
                PACKAGED_VECTORS,
                "packaged NIP-17 vectors must match {}",
                workspace_path.display()
            );
            Cow::Owned(canonical)
        }
        Err(error)
            if error.kind() == std::io::ErrorKind::NotFound
                && !Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join(WORKSPACE_CONTRACT_MARKER_PATH)
                    .is_file() =>
        {
            Cow::Borrowed(PACKAGED_VECTORS)
        }
        Err(error) => panic!("failed to read {}: {error}", workspace_path.display()),
    }
}

async fn execute_vector(vector: &Vector) {
    match vector.kind.as_str() {
        "nip17.message.roundtrip" => execute_message_roundtrip(vector).await,
        "nip17.message_file.roundtrip" => execute_message_file_roundtrip(vector).await,
        "nip17.message.invalid_recipient" => execute_invalid_recipient(vector).await,
        kind => panic!("{} uses unsupported vector kind {kind}", vector.id),
    }
}

async fn execute_message_roundtrip(vector: &Vector) {
    assert_eq!(
        input_str(vector, "recipient_public_key"),
        FIXTURE_BOB_PUBLIC_KEY_HEX
    );
    let sender = sender_keys();
    let receiver = receiver_keys();
    let message = Message {
        recipients: vec![MessageRecipient {
            public_key: input_str(vector, "recipient_public_key").to_owned(),
            relay_url: None,
        }],
        content: input_str(vector, "content").to_owned(),
        reply_to: None,
        subject: Some(input_str(vector, "subject").to_owned()),
    };
    let events = radroots_nostr_wrap_message(&sender, &message, options(vector))
        .await
        .unwrap_or_else(|error| panic!("{} failed: {error}", vector.id));
    assert_eq!(events.len(), expected_u64(vector, "event_count") as usize);

    let rumor = radroots_nostr_unwrap_gift_wrap(&receiver, &events[0])
        .await
        .unwrap_or_else(|error| panic!("{} failed: {error}", vector.id));
    let rendered = format!("{rumor:?}");
    for private_value in [
        message.content.as_str(),
        FIXTURE_ALICE_PUBLIC_KEY_HEX,
        FIXTURE_BOB_PUBLIC_KEY_HEX,
    ] {
        assert!(!rendered.contains(private_value), "{}", vector.id);
    }
    match rumor {
        RadrootsNip17Rumor::Message(metadata) => {
            assert_eq!(
                metadata.author,
                expected_str(vector, "author"),
                "{}",
                vector.id
            );
            assert_eq!(
                metadata.published_at,
                expected_u64(vector, "published_at"),
                "{}",
                vector.id
            );
            assert_eq!(
                metadata.data.content,
                expected_str(vector, "content"),
                "{}",
                vector.id
            );
            assert_eq!(
                metadata.data.recipients.len(),
                expected_u64(vector, "recipient_count") as usize,
                "{}",
                vector.id
            );
            assert_eq!(
                metadata.data.subject.as_deref(),
                Some(expected_str(vector, "subject")),
                "{}",
                vector.id
            );
        }
        other => panic!("{} expected a message rumor, got {other:?}", vector.id),
    }
}

async fn execute_message_file_roundtrip(vector: &Vector) {
    assert_eq!(
        input_str(vector, "recipient_public_key"),
        FIXTURE_BOB_PUBLIC_KEY_HEX
    );
    let sender = sender_keys();
    let receiver = receiver_keys();
    let message = MessageFile {
        recipients: vec![MessageRecipient {
            public_key: input_str(vector, "recipient_public_key").to_owned(),
            relay_url: None,
        }],
        file_url: input_str(vector, "file_url").to_owned(),
        reply_to: None,
        subject: None,
        file_type: input_str(vector, "file_type").to_owned(),
        encryption_algorithm: input_str(vector, "encryption_algorithm").to_owned(),
        decryption_key: input_str(vector, "decryption_key").to_owned(),
        decryption_nonce: input_str(vector, "decryption_nonce").to_owned(),
        encrypted_hash: input_str(vector, "encrypted_hash").to_owned(),
        original_hash: None,
        size: Some(input_u64(vector, "size")),
        dimensions: Some(MessageFileDimensions {
            w: input_u64(vector, "width") as u32,
            h: input_u64(vector, "height") as u32,
        }),
        blurhash: None,
        thumb: None,
        fallbacks: Vec::new(),
    };
    let events = radroots_nostr_wrap_message_file(&sender, &message, options(vector))
        .await
        .unwrap_or_else(|error| panic!("{} failed: {error}", vector.id));
    assert_eq!(events.len(), expected_u64(vector, "event_count") as usize);

    let rumor = radroots_nostr_unwrap_gift_wrap(&receiver, &events[0])
        .await
        .unwrap_or_else(|error| panic!("{} failed: {error}", vector.id));
    let rendered = format!("{rumor:?}");
    for private_value in [
        message.file_url.as_str(),
        message.decryption_key.as_str(),
        message.decryption_nonce.as_str(),
        message.encrypted_hash.as_str(),
        FIXTURE_ALICE_PUBLIC_KEY_HEX,
        FIXTURE_BOB_PUBLIC_KEY_HEX,
    ] {
        assert!(!rendered.contains(private_value), "{}", vector.id);
    }
    match rumor {
        RadrootsNip17Rumor::MessageFile(metadata) => {
            assert_eq!(
                metadata.author,
                expected_str(vector, "author"),
                "{}",
                vector.id
            );
            assert_eq!(
                metadata.published_at,
                expected_u64(vector, "published_at"),
                "{}",
                vector.id
            );
            assert_eq!(
                metadata.data.file_url,
                expected_str(vector, "file_url"),
                "{}",
                vector.id
            );
            assert_eq!(
                metadata.data.file_type,
                expected_str(vector, "file_type"),
                "{}",
                vector.id
            );
            assert_eq!(
                metadata.data.encrypted_hash,
                expected_str(vector, "encrypted_hash"),
                "{}",
                vector.id
            );
            assert_eq!(
                metadata.data.size,
                Some(expected_u64(vector, "size")),
                "{}",
                vector.id
            );
            assert_eq!(
                metadata.data.dimensions,
                Some(MessageFileDimensions {
                    w: expected_u64(vector, "width") as u32,
                    h: expected_u64(vector, "height") as u32,
                }),
                "{}",
                vector.id
            );
        }
        other => panic!("{} expected a message-file rumor, got {other:?}", vector.id),
    }
}

async fn execute_invalid_recipient(vector: &Vector) {
    let sender = sender_keys();
    let recipient = input_str(vector, "recipient_public_key");
    let message = Message {
        recipients: vec![MessageRecipient {
            public_key: recipient.to_owned(),
            relay_url: None,
        }],
        content: input_str(vector, "content").to_owned(),
        reply_to: None,
        subject: None,
    };
    let error = radroots_nostr_wrap_message(&sender, &message, options(vector))
        .await
        .expect_err("invalid recipient vector must fail");
    assert_eq!(error.code(), expected_str(vector, "error"), "{}", vector.id);
    let rendered = format!("{error:?} {error}");
    assert!(!rendered.contains(recipient), "{}", vector.id);
    assert!(!rendered.contains(&message.content), "{}", vector.id);
}

fn options(vector: &Vector) -> RadrootsNip17WrapOptions {
    RadrootsNip17WrapOptions {
        include_sender: false,
        rumor_created_at: Some(input_u64(vector, "created_at")),
        gift_wrap_tags: Vec::new(),
    }
}

fn sender_keys() -> Keys {
    let keys = Keys::new(
        SecretKey::from_hex(FIXTURE_ALICE_SECRET_KEY_HEX).expect("approved sender fixture key"),
    );
    assert_eq!(keys.public_key().to_string(), FIXTURE_ALICE_PUBLIC_KEY_HEX);
    keys
}

fn receiver_keys() -> Keys {
    let keys = Keys::new(
        SecretKey::from_hex(FIXTURE_BOB_SECRET_KEY_HEX).expect("approved receiver fixture key"),
    );
    assert_eq!(keys.public_key().to_string(), FIXTURE_BOB_PUBLIC_KEY_HEX);
    keys
}

fn input_str<'a>(vector: &'a Vector, field: &str) -> &'a str {
    vector
        .input
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("{} missing string input {field}", vector.id))
}

fn input_u64(vector: &Vector, field: &str) -> u64 {
    vector
        .input
        .get(field)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("{} missing u64 input {field}", vector.id))
}

fn expected_str<'a>(vector: &'a Vector, field: &str) -> &'a str {
    vector
        .expected
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("{} missing string expectation {field}", vector.id))
}

fn expected_u64(vector: &Vector, field: &str) -> u64 {
    vector
        .expected
        .get(field)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("{} missing u64 expectation {field}", vector.id))
}
