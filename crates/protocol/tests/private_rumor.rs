#![cfg(feature = "private_rumor")]
use radroots_domain::PublicKey;
use radroots_protocol::{
    nip01::{EventFields, UnsignedEvent},
    private_rumor::{DecodedRumor, RumorWireError},
    profile,
};
use serde_json::{Value, json};

fn author() -> PublicKey {
    PublicKey::from_hex("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798").unwrap()
}
fn wire(kind: u32, tags: &[Vec<String>], content: &str) -> UnsignedEvent {
    UnsignedEvent::encode(
        &EventFields {
            author: author(),
            created_at: 450,
            kind,
            tags,
            content,
        },
        profile::UNSIGNED_RUMOR_MAX_BYTES,
    )
    .unwrap()
}
fn error(value: &Value) -> RumorWireError {
    DecodedRumor::decode(&serde_json::to_vec(value).unwrap()).unwrap_err()
}

#[test]
fn identity_checked_unknown_data_preserves_unicode_tags_without_interpreting_them() {
    let tags = vec![
        vec!["p".into(), "future participant representation".into()],
        vec!["x".into(), "東京 🥕\n\\\"".into()],
    ];
    let encoded = wire(65535, &tags, "e\u{301} and é remain distinct\n🧅");
    let value = DecodedRumor::decode(encoded.json().as_bytes()).unwrap();
    assert_eq!(value.event_id().into_bytes(), encoded.event_id());
    assert_eq!(value.author(), author());
    assert_eq!(value.created_at(), 450);
    assert_eq!(value.kind(), 65535);
    assert!(value.has_tags());
    assert_eq!(
        value.protected_content(),
        "e\u{301} and é remain distinct\n🧅"
    );
    assert!(
        value.retained_bytes()
            >= std::mem::size_of::<DecodedRumor>() + value.protected_content().len()
    );
    assert_eq!(format!("{value:?}"), "DecodedRumor(<redacted>)");
}

#[test]
fn every_unsigned_field_is_required_closed_canonical_and_duplicate_safe() {
    let encoded = wire(8890, &[], "{}");
    let value: Value = serde_json::from_str(encoded.json()).unwrap();
    for key in ["id", "pubkey", "created_at", "kind", "tags", "content"] {
        let mut missing = value.clone();
        missing.as_object_mut().unwrap().remove(key);
        assert_eq!(error(&missing), RumorWireError::InvalidInput, "{key}");
        let duplicate = format!(
            "{{{}:{},{}",
            serde_json::to_string(key).unwrap(),
            value[key],
            &encoded.json()[1..]
        );
        assert_eq!(
            DecodedRumor::decode(duplicate.as_bytes()).unwrap_err(),
            RumorWireError::InvalidInput,
            "{key}"
        );
    }
    for (key, replacement) in [
        ("kind", json!(65536)),
        ("kind", json!(-1)),
        ("kind", json!(1.5)),
        ("created_at", json!(253402300800_u64)),
        ("created_at", json!("450")),
        ("pubkey", json!(author().to_hex().to_uppercase())),
        ("id", json!(value["id"].as_str().unwrap().to_uppercase())),
        ("tags", json!({})),
        ("content", json!({})),
    ] {
        let mut changed = value.clone();
        changed[key] = replacement;
        assert_eq!(error(&changed), RumorWireError::InvalidInput, "{key}");
    }
    let mut extra = value.clone();
    extra["future"] = json!(true);
    assert_eq!(error(&extra), RumorWireError::InvalidInput);
    assert_eq!(
        error(&json!(
            value.as_object().unwrap().values().collect::<Vec<_>>()
        )),
        RumorWireError::InvalidInput
    );
}

#[test]
fn private_tag_shapes_and_original_identity_are_checked_before_returning_data() {
    let original = wire(8890, &[], "{}");
    let value: Value = serde_json::from_str(original.json()).unwrap();
    for tags in [
        json!([[]]),
        json!([[""]]),
        json!([["\n", "x"]]),
        json!([["x", 1]]),
        json!(["tag"]),
    ] {
        let mut changed = value.clone();
        changed["tags"] = tags;
        assert_eq!(error(&changed), RumorWireError::InvalidInput);
    }
    for key in ["content", "id"] {
        let mut changed = value.clone();
        changed[key] = if key == "id" {
            json!("00".repeat(32))
        } else {
            json!("changed")
        };
        assert_eq!(error(&changed), RumorWireError::IdentityMismatch);
    }
    assert_eq!(
        RumorWireError::IdentityMismatch.to_string(),
        "rumor identity mismatch"
    );
}

#[test]
fn decoded_content_byte_budget_and_complete_frame_budget_remain_independent() {
    let exact = wire(8890, &[], &"é".repeat(8192));
    let decoded = DecodedRumor::decode(exact.json().as_bytes()).unwrap();
    assert_eq!(decoded.protected_content().len(), 16384);
    let extra = wire(8890, &[], &("é".repeat(8192) + "a"));
    assert_eq!(
        DecodedRumor::decode(extra.json().as_bytes()).unwrap_err(),
        RumorWireError::LimitExceeded
    );
    let generic = wire(65535, &[], &"é".repeat(8193));
    assert!(DecodedRumor::decode(generic.json().as_bytes()).is_ok());
    assert_eq!(
        DecodedRumor::decode(&vec![b' '; 24577]).unwrap_err(),
        RumorWireError::LimitExceeded
    );
    let nested = format!("{{\"x\":{}0{}}}", "[".repeat(16), "]".repeat(16));
    assert!(DecodedRumor::decode(nested.as_bytes()).is_err());
}
