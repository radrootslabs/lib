use radroots_events::kinds::{KIND_MESSAGE, KIND_MESSAGE_FILE};
use radroots_events::message::RadrootsMessageRecipient;
use radroots_events::message_file::{RadrootsMessageFile, RadrootsMessageFileDimensions};
use radroots_events::RadrootsNostrEventPtr;

use radroots_events_codec::error::{EventEncodeError, EventParseError};
use radroots_events_codec::message_file::decode::message_file_from_tags;
use radroots_events_codec::message_file::encode::{message_file_build_tags, to_wire_parts};

fn sample_message_file() -> RadrootsMessageFile {
    RadrootsMessageFile {
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
        file_url: "https://files.example/encrypted.bin".to_string(),
        reply_to: Some(RadrootsNostrEventPtr {
            id: "reply".to_string(),
            relays: Some("wss://reply.example".to_string()),
        }),
        subject: Some("topic".to_string()),
        file_type: "image/jpeg".to_string(),
        encryption_algorithm: "aes-gcm".to_string(),
        decryption_key: "key".to_string(),
        decryption_nonce: "nonce".to_string(),
        encrypted_hash: "hash".to_string(),
        original_hash: Some("orig-hash".to_string()),
        size: Some(1200),
        dimensions: Some(RadrootsMessageFileDimensions { w: 1200, h: 800 }),
        blurhash: Some("blurhash".to_string()),
        thumb: Some("https://files.example/thumb.bin".to_string()),
        fallbacks: vec![
            "https://files.example/fallback-1.bin".to_string(),
            "https://files.example/fallback-2.bin".to_string(),
        ],
    }
}

#[test]
fn message_file_build_tags_requires_recipients() {
    let mut message = sample_message_file();
    message.recipients.clear();

    let err = message_file_build_tags(&message).unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("recipients")
    ));
}

#[test]
fn message_file_to_wire_parts_requires_file_url() {
    let mut message = sample_message_file();
    message.file_url = "  ".to_string();

    let err = to_wire_parts(&message).unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("file_url")
    ));
}

#[test]
fn message_file_build_tags_requires_file_type() {
    let mut message = sample_message_file();
    message.file_type = " ".to_string();

    let err = message_file_build_tags(&message).unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("file_type")
    ));
}

#[test]
fn message_file_to_wire_parts_sets_kind_content_and_tags() {
    let message = sample_message_file();
    let parts = to_wire_parts(&message).unwrap();

    assert_eq!(parts.kind, KIND_MESSAGE_FILE);
    assert_eq!(parts.content, message.file_url);
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
            vec!["file-type".to_string(), "image/jpeg".to_string()],
            vec!["encryption-algorithm".to_string(), "aes-gcm".to_string()],
            vec!["decryption-key".to_string(), "key".to_string()],
            vec!["decryption-nonce".to_string(), "nonce".to_string()],
            vec!["x".to_string(), "hash".to_string()],
            vec!["ox".to_string(), "orig-hash".to_string()],
            vec!["size".to_string(), "1200".to_string()],
            vec!["dim".to_string(), "1200x800".to_string()],
            vec!["blurhash".to_string(), "blurhash".to_string()],
            vec!["thumb".to_string(), "https://files.example/thumb.bin".to_string()],
            vec![
                "fallback".to_string(),
                "https://files.example/fallback-1.bin".to_string()
            ],
            vec![
                "fallback".to_string(),
                "https://files.example/fallback-2.bin".to_string()
            ],
        ]
    );
}

#[test]
fn message_file_roundtrip_from_tags() {
    let message = sample_message_file();
    let parts = to_wire_parts(&message).unwrap();

    let decoded = message_file_from_tags(parts.kind, &parts.tags, &parts.content).unwrap();
    assert_eq!(decoded.file_url, message.file_url);
    assert_eq!(decoded.file_type, message.file_type);
    assert_eq!(decoded.encryption_algorithm, message.encryption_algorithm);
    assert_eq!(decoded.decryption_key, message.decryption_key);
    assert_eq!(decoded.decryption_nonce, message.decryption_nonce);
    assert_eq!(decoded.encrypted_hash, message.encrypted_hash);
    assert_eq!(decoded.original_hash, message.original_hash);
    assert_eq!(decoded.size, message.size);
    assert_eq!(decoded.dimensions, message.dimensions);
    assert_eq!(decoded.blurhash, message.blurhash);
    assert_eq!(decoded.thumb, message.thumb);
    assert_eq!(decoded.fallbacks, message.fallbacks);
    assert_eq!(decoded.recipients.len(), message.recipients.len());
}

#[test]
fn message_file_from_tags_rejects_wrong_kind() {
    let message = sample_message_file();
    let parts = to_wire_parts(&message).unwrap();

    let err = message_file_from_tags(KIND_MESSAGE, &parts.tags, &parts.content).unwrap_err();
    assert!(matches!(
        err,
        EventParseError::InvalidKind {
            expected: "15",
            got: KIND_MESSAGE
        }
    ));
}
