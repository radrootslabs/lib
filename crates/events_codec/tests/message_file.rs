#[path = "../src/test_fixtures.rs"]
mod test_fixtures;

use radroots_events::RadrootsNostrEventPtr;
use radroots_events::kinds::{KIND_MESSAGE, KIND_MESSAGE_FILE};
use radroots_events::message::RadrootsMessageRecipient;
use radroots_events::message_file::{RadrootsMessageFile, RadrootsMessageFileDimensions};

use radroots_events_codec::error::{EventEncodeError, EventParseError};
use radroots_events_codec::message_file::decode::{
    data_from_event, message_file_from_tags, parsed_from_event,
};
use radroots_events_codec::message_file::encode::{
    message_file_build_tags, to_wire_parts, to_wire_parts_with_kind,
};
use test_fixtures::{CDN_PRIMARY_HTTPS, RELAY_PRIMARY_WSS, RELAY_SECONDARY_WSS};

fn file_url(path: &str) -> String {
    format!("{CDN_PRIMARY_HTTPS}/{path}")
}

fn sample_message_file() -> RadrootsMessageFile {
    RadrootsMessageFile {
        recipients: vec![
            RadrootsMessageRecipient {
                public_key: "pub1".to_string(),
                relay_url: None,
            },
            RadrootsMessageRecipient {
                public_key: "pub2".to_string(),
                relay_url: Some(RELAY_PRIMARY_WSS.to_string()),
            },
        ],
        file_url: file_url("encrypted.bin"),
        reply_to: Some(RadrootsNostrEventPtr {
            id: "reply".to_string(),
            relays: Some(RELAY_SECONDARY_WSS.to_string()),
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
        thumb: Some(file_url("thumb.bin")),
        fallbacks: vec![file_url("fallback-1.bin"), file_url("fallback-2.bin")],
    }
}

fn minimal_message_file_tags() -> Vec<Vec<String>> {
    vec![
        vec!["p".to_string(), "pub1".to_string()],
        vec!["file-type".to_string(), "image/jpeg".to_string()],
        vec!["encryption-algorithm".to_string(), "aes-gcm".to_string()],
        vec!["decryption-key".to_string(), "key".to_string()],
        vec!["decryption-nonce".to_string(), "nonce".to_string()],
        vec!["x".to_string(), "hash".to_string()],
    ]
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
fn message_file_to_wire_parts_propagates_tag_build_errors() {
    let mut message = sample_message_file();
    message.file_type = " ".to_string();
    let err = to_wire_parts(&message).unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("file_type")
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
fn message_file_build_tags_requires_crypto_fields() {
    let mut message = sample_message_file();
    message.encryption_algorithm = " ".to_string();
    let err = message_file_build_tags(&message).unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("encryption_algorithm")
    ));

    let mut message = sample_message_file();
    message.decryption_key = " ".to_string();
    let err = message_file_build_tags(&message).unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("decryption_key")
    ));

    let mut message = sample_message_file();
    message.decryption_nonce = " ".to_string();
    let err = message_file_build_tags(&message).unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("decryption_nonce")
    ));

    let mut message = sample_message_file();
    message.encrypted_hash = " ".to_string();
    let err = message_file_build_tags(&message).unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("encrypted_hash")
    ));
}

#[test]
fn message_file_build_tags_rejects_invalid_reply_subject_and_fallbacks() {
    let mut message = sample_message_file();
    message.reply_to = Some(RadrootsNostrEventPtr {
        id: " ".to_string(),
        relays: None,
    });
    let err = message_file_build_tags(&message).unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("reply_to.id")
    ));

    let mut message = sample_message_file();
    message.subject = Some(" ".to_string());
    let err = message_file_build_tags(&message).unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("subject")
    ));

    let mut message = sample_message_file();
    message.fallbacks = vec![" ".to_string()];
    let err = message_file_build_tags(&message).unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("fallback")
    ));
}

#[test]
fn message_file_to_wire_parts_with_kind_enforces_kind() {
    let message = sample_message_file();
    let parts = to_wire_parts_with_kind(&message, KIND_MESSAGE_FILE).unwrap();
    assert_eq!(parts.kind, KIND_MESSAGE_FILE);

    let err = to_wire_parts_with_kind(&message, KIND_MESSAGE).unwrap_err();
    assert!(matches!(err, EventEncodeError::InvalidKind(KIND_MESSAGE)));
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
                RELAY_PRIMARY_WSS.to_string()
            ],
            vec![
                "e".to_string(),
                "reply".to_string(),
                RELAY_SECONDARY_WSS.to_string()
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
            vec!["thumb".to_string(), file_url("thumb.bin")],
            vec!["fallback".to_string(), file_url("fallback-1.bin")],
            vec!["fallback".to_string(), file_url("fallback-2.bin")],
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

#[test]
fn message_file_from_tags_rejects_invalid_optional_tags() {
    let message = sample_message_file();
    let mut parts = to_wire_parts(&message).unwrap();
    let size_tag = parts
        .tags
        .iter_mut()
        .find(|tag| tag.first().map(|value| value.as_str()) == Some("size"))
        .expect("size tag");
    size_tag[1] = "not-a-number".to_string();
    let err = message_file_from_tags(KIND_MESSAGE_FILE, &parts.tags, &parts.content).unwrap_err();
    assert!(matches!(err, EventParseError::InvalidNumber("size", _)));

    let err = message_file_from_tags(
        KIND_MESSAGE_FILE,
        &[
            vec!["p".to_string(), "pub1".to_string()],
            vec!["file-type".to_string(), "image/jpeg".to_string()],
            vec!["encryption-algorithm".to_string(), "aes-gcm".to_string()],
            vec!["decryption-key".to_string(), "key".to_string()],
            vec!["decryption-nonce".to_string(), "nonce".to_string()],
            vec!["x".to_string(), "hash".to_string()],
            vec!["dim".to_string(), "10".to_string()],
        ],
        &file_url("encrypted.bin"),
    )
    .unwrap_err();
    assert!(matches!(err, EventParseError::InvalidTag("dim")));

    let err = message_file_from_tags(
        KIND_MESSAGE_FILE,
        &[
            vec!["p".to_string(), "pub1".to_string()],
            vec!["file-type".to_string(), "image/jpeg".to_string()],
            vec!["encryption-algorithm".to_string(), "aes-gcm".to_string()],
            vec!["decryption-key".to_string(), "key".to_string()],
            vec!["decryption-nonce".to_string(), "nonce".to_string()],
            vec!["x".to_string(), "hash".to_string()],
            vec!["fallback".to_string()],
        ],
        &file_url("encrypted.bin"),
    )
    .unwrap_err();
    assert!(matches!(err, EventParseError::InvalidTag("fallback")));

    let err = message_file_from_tags(
        KIND_MESSAGE_FILE,
        &[
            vec!["p".to_string(), "pub1".to_string()],
            vec!["file-type".to_string(), " ".to_string()],
            vec!["encryption-algorithm".to_string(), "aes-gcm".to_string()],
            vec!["decryption-key".to_string(), "key".to_string()],
            vec!["decryption-nonce".to_string(), "nonce".to_string()],
            vec!["x".to_string(), "hash".to_string()],
        ],
        &file_url("encrypted.bin"),
    )
    .unwrap_err();
    assert!(matches!(err, EventParseError::InvalidTag("file-type")));

    let err = message_file_from_tags(
        KIND_MESSAGE_FILE,
        &[
            vec!["p".to_string(), "pub1".to_string()],
            vec!["file-type".to_string(), "image/jpeg".to_string()],
            vec!["encryption-algorithm".to_string(), "aes-gcm".to_string()],
            vec!["decryption-key".to_string(), "key".to_string()],
            vec!["decryption-nonce".to_string(), "nonce".to_string()],
            vec!["x".to_string(), "hash".to_string()],
            vec!["size".to_string(), " ".to_string()],
        ],
        &file_url("encrypted.bin"),
    )
    .unwrap_err();
    assert!(matches!(err, EventParseError::InvalidTag("size")));

    let err = message_file_from_tags(
        KIND_MESSAGE_FILE,
        &[
            vec!["p".to_string(), "pub1".to_string()],
            vec!["file-type".to_string(), "image/jpeg".to_string()],
            vec!["encryption-algorithm".to_string(), "aes-gcm".to_string()],
            vec!["decryption-key".to_string(), "key".to_string()],
            vec!["decryption-nonce".to_string(), "nonce".to_string()],
            vec!["x".to_string(), "hash".to_string()],
            vec!["dim".to_string(), " ".to_string()],
        ],
        &file_url("encrypted.bin"),
    )
    .unwrap_err();
    assert!(matches!(err, EventParseError::InvalidTag("dim")));

    let err = message_file_from_tags(
        KIND_MESSAGE_FILE,
        &[
            vec!["p".to_string(), "pub1".to_string()],
            vec!["file-type".to_string(), "image/jpeg".to_string()],
            vec!["encryption-algorithm".to_string(), "aes-gcm".to_string()],
            vec!["decryption-key".to_string(), "key".to_string()],
            vec!["decryption-nonce".to_string(), "nonce".to_string()],
            vec!["x".to_string(), "hash".to_string()],
            vec!["thumb".to_string(), " ".to_string()],
        ],
        &file_url("encrypted.bin"),
    )
    .unwrap_err();
    assert!(matches!(err, EventParseError::InvalidTag("thumb")));

    let err = message_file_from_tags(
        KIND_MESSAGE_FILE,
        &[
            vec!["p".to_string(), "pub1".to_string()],
            vec!["file-type".to_string(), "image/jpeg".to_string()],
            vec!["encryption-algorithm".to_string(), "aes-gcm".to_string()],
            vec!["decryption-key".to_string(), "key".to_string()],
            vec!["decryption-nonce".to_string(), "nonce".to_string()],
            vec!["x".to_string(), "hash".to_string()],
            vec!["fallback".to_string(), " ".to_string()],
        ],
        &file_url("encrypted.bin"),
    )
    .unwrap_err();
    assert!(matches!(err, EventParseError::InvalidTag("fallback")));
}

#[test]
fn message_file_metadata_and_index_from_event_roundtrip() {
    let message = sample_message_file();
    let parts = to_wire_parts(&message).unwrap();
    let metadata = data_from_event(
        "id".to_string(),
        "author".to_string(),
        77,
        parts.kind,
        parts.content.clone(),
        parts.tags.clone(),
    )
    .unwrap();
    assert_eq!(metadata.id, "id");
    assert_eq!(metadata.author, "author");
    assert_eq!(metadata.published_at, 77);
    assert_eq!(metadata.kind, KIND_MESSAGE_FILE);
    assert_eq!(metadata.data.file_type, "image/jpeg");
    assert_eq!(metadata.data.recipients.len(), 2);

    let index = parsed_from_event(
        "id".to_string(),
        "author".to_string(),
        77,
        parts.kind,
        parts.content,
        parts.tags,
        "sig".to_string(),
    )
    .unwrap();
    assert_eq!(index.event.kind, KIND_MESSAGE_FILE);
    assert_eq!(index.event.sig, "sig");
    assert_eq!(index.data.data.file_type, "image/jpeg");
}

#[test]
fn message_file_index_from_event_propagates_parse_errors() {
    let err = parsed_from_event(
        "id".to_string(),
        "author".to_string(),
        77,
        KIND_MESSAGE,
        "payload".to_string(),
        Vec::new(),
        "sig".to_string(),
    )
    .unwrap_err();
    assert!(matches!(
        err,
        EventParseError::InvalidKind {
            expected: "15",
            got: KIND_MESSAGE
        }
    ));
}

#[test]
fn message_file_from_tags_rejects_empty_content() {
    let err = message_file_from_tags(
        KIND_MESSAGE_FILE,
        &[
            vec!["p".to_string(), "pub1".to_string()],
            vec!["file-type".to_string(), "image/jpeg".to_string()],
            vec!["encryption-algorithm".to_string(), "aes-gcm".to_string()],
            vec!["decryption-key".to_string(), "key".to_string()],
            vec!["decryption-nonce".to_string(), "nonce".to_string()],
            vec!["x".to_string(), "hash".to_string()],
        ],
        " ",
    )
    .unwrap_err();
    assert!(matches!(err, EventParseError::InvalidTag("content")));
}

#[test]
fn message_file_from_tags_rejects_more_invalid_tag_shapes() {
    let mut tags = minimal_message_file_tags();
    tags[1].truncate(1);
    let err =
        message_file_from_tags(KIND_MESSAGE_FILE, &tags, &file_url("encrypted.bin")).unwrap_err();
    assert!(matches!(err, EventParseError::MissingTag("file-type")));

    let mut tags = minimal_message_file_tags();
    tags[0][1] = " ".to_string();
    let err =
        message_file_from_tags(KIND_MESSAGE_FILE, &tags, &file_url("encrypted.bin")).unwrap_err();
    assert!(matches!(err, EventParseError::InvalidTag("p")));

    let mut tags = minimal_message_file_tags();
    tags.push(vec!["e".to_string(), " ".to_string()]);
    let err =
        message_file_from_tags(KIND_MESSAGE_FILE, &tags, &file_url("encrypted.bin")).unwrap_err();
    assert!(matches!(err, EventParseError::InvalidTag("e")));

    let mut tags = minimal_message_file_tags();
    tags.push(vec!["subject".to_string(), " ".to_string()]);
    let err =
        message_file_from_tags(KIND_MESSAGE_FILE, &tags, &file_url("encrypted.bin")).unwrap_err();
    assert!(matches!(err, EventParseError::InvalidTag("subject")));

    let mut tags = minimal_message_file_tags();
    tags[2][1] = " ".to_string();
    let err =
        message_file_from_tags(KIND_MESSAGE_FILE, &tags, &file_url("encrypted.bin")).unwrap_err();
    assert!(matches!(
        err,
        EventParseError::InvalidTag("encryption-algorithm")
    ));

    let mut tags = minimal_message_file_tags();
    tags[3][1] = " ".to_string();
    let err =
        message_file_from_tags(KIND_MESSAGE_FILE, &tags, &file_url("encrypted.bin")).unwrap_err();
    assert!(matches!(err, EventParseError::InvalidTag("decryption-key")));

    let mut tags = minimal_message_file_tags();
    tags[4][1] = " ".to_string();
    let err =
        message_file_from_tags(KIND_MESSAGE_FILE, &tags, &file_url("encrypted.bin")).unwrap_err();
    assert!(matches!(
        err,
        EventParseError::InvalidTag("decryption-nonce")
    ));

    let mut tags = minimal_message_file_tags();
    tags[5][1] = " ".to_string();
    let err =
        message_file_from_tags(KIND_MESSAGE_FILE, &tags, &file_url("encrypted.bin")).unwrap_err();
    assert!(matches!(err, EventParseError::InvalidTag("x")));

    let mut tags = minimal_message_file_tags();
    tags.push(vec!["ox".to_string(), " ".to_string()]);
    let err =
        message_file_from_tags(KIND_MESSAGE_FILE, &tags, &file_url("encrypted.bin")).unwrap_err();
    assert!(matches!(err, EventParseError::InvalidTag("ox")));

    let mut tags = minimal_message_file_tags();
    tags.push(vec!["blurhash".to_string(), " ".to_string()]);
    let err =
        message_file_from_tags(KIND_MESSAGE_FILE, &tags, &file_url("encrypted.bin")).unwrap_err();
    assert!(matches!(err, EventParseError::InvalidTag("blurhash")));
}

#[test]
fn message_file_from_tags_rejects_invalid_dimension_components() {
    let mut tags = minimal_message_file_tags();
    tags.push(vec!["dim".to_string(), "badx10".to_string()]);
    let err =
        message_file_from_tags(KIND_MESSAGE_FILE, &tags, &file_url("encrypted.bin")).unwrap_err();
    assert!(matches!(err, EventParseError::InvalidTag("dim")));

    let mut tags = minimal_message_file_tags();
    tags.push(vec!["dim".to_string(), "10xbad".to_string()]);
    let err =
        message_file_from_tags(KIND_MESSAGE_FILE, &tags, &file_url("encrypted.bin")).unwrap_err();
    assert!(matches!(err, EventParseError::InvalidTag("dim")));
}
