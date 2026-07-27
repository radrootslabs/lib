use super::*;
use serde_json::json;

fn hex_64(character: char) -> String {
    crate::test_valid_hex_64(character)
}

fn hex_128(character: char) -> String {
    core::iter::repeat_n(character, 128).collect()
}

fn valid_event_value(content: &str, tags: Vec<Vec<String>>) -> Value {
    let pubkey = hex_64('a');
    let id = compute_canonical_nip01_event_id(pubkey.as_str(), 1_700_000_000, 1, &tags, content)
        .expect("event id")
        .into_string();
    json!({
        "id": id,
        "pubkey": pubkey,
        "created_at": 1_700_000_000u64,
        "kind": 1u32,
        "tags": tags,
        "content": content,
        "sig": hex_128('b')
    })
}

fn raw_json(value: &Value) -> String {
    serde_json::to_string(value).expect("event json")
}

fn valid_event_json(content: &str, tags: Vec<Vec<String>>) -> String {
    raw_json(&valid_event_value(content, tags))
}

fn default_tags() -> Vec<Vec<String>> {
    vec![vec!["t".to_owned(), "soil".to_owned()]]
}

fn tag_with_total_elements(element_count: usize) -> Vec<Vec<String>> {
    assert!(element_count > 0);
    let mut tag = Vec::with_capacity(element_count);
    tag.push("t".to_owned());
    tag.resize(element_count, String::new());
    vec![tag]
}

fn valid_envelope_parts(content: &str, tags: Vec<Vec<String>>) -> RadrootsEventEnvelopeParts {
    let pubkey = hex_64('a');
    let id = compute_canonical_nip01_event_id(pubkey.as_str(), 1_700_000_000, 1, &tags, content)
        .expect("event id")
        .into_string();
    RadrootsEventEnvelopeParts {
        id,
        author: pubkey,
        created_at: 1_700_000_000,
        kind: 1,
        tags,
        content: content.to_owned(),
        sig: hex_128('b'),
    }
}

#[test]
fn canonical_preimage_escapes_required_json_characters() {
    let preimage = canonical_nip01_event_id_preimage(
        hex_64('a').as_str(),
        10,
        1,
        &[vec!["t".to_owned(), "line\nsoil".to_owned()]],
        "\"\\\n\r\t\u{08}\u{0c}\u{01}",
    )
    .expect("preimage");

    assert_eq!(
        preimage,
        r#"[0,"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",10,1,[["t","line\nsoil"]],"\"\\\n\r\t\b\f\u0001"]"#
    );
}

#[test]
fn parses_wire_json_preserves_extra_and_verifies_id() {
    let mut value = valid_event_value("hello", default_tags());
    value
        .as_object_mut()
        .expect("object")
        .insert("client".to_owned(), json!({"name":"radroots-test"}));
    let wire = RadrootsNip01EventWire::parse_json(raw_json(&value).as_str()).expect("wire");

    assert_eq!(wire.pubkey, hex_64('a'));
    assert_eq!(wire.created_at, 1_700_000_000);
    assert_eq!(wire.kind, 1);
    assert_eq!(wire.tags, default_tags());
    assert_eq!(wire.content, "hello");
    assert_eq!(
        wire.extra.get("client").expect("client extra"),
        &json!({"name":"radroots-test"})
    );
    wire.verify_id().expect("verified id");
    assert_eq!(
        wire.canonical_id_preimage().expect("preimage"),
        r#"[0,"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",1700000000,1,[["t","soil"]],"hello"]"#
    );
}

#[test]
fn into_envelope_verifies_id_before_domain_conversion() {
    let wire =
        RadrootsNip01EventWire::parse_json(valid_event_json("hello", default_tags()).as_str())
            .expect("wire");

    let envelope = wire.clone().into_envelope().expect("envelope");
    assert_eq!(envelope.id_str(), wire.id);
    assert_eq!(envelope.content(), "hello");

    let mut tampered_id = wire.clone();
    tampered_id.id = hex_64('f');
    assert!(matches!(
        tampered_id.into_envelope(),
        Err(RadrootsEventWireError::EventIdMismatch { .. })
    ));

    let mut tampered_content = wire;
    tampered_content.content = "tampered".to_owned();
    assert!(matches!(
        tampered_content.into_envelope(),
        Err(RadrootsEventWireError::EventIdMismatch { .. })
    ));
}

#[test]
fn into_envelope_ignores_extra_for_id_and_propagates_domain_limits() {
    let mut value = valid_event_value("hello", default_tags());
    value
        .as_object_mut()
        .expect("object")
        .insert("client".to_owned(), json!("radroots-test"));
    let wire = RadrootsNip01EventWire::parse_json(raw_json(&value).as_str()).expect("wire");
    let envelope = wire.into_envelope().expect("envelope");
    assert_eq!(envelope.content(), "hello");

    let content = core::iter::repeat_n('x', DEFAULT_CONTENT_MAX_BYTES + 1).collect::<String>();
    let tags = default_tags();
    let pubkey = hex_64('a');
    let id = compute_canonical_nip01_event_id(
        pubkey.as_str(),
        1_700_000_000,
        1,
        &tags,
        content.as_str(),
    )
    .expect("event id")
    .into_string();
    let wire = RadrootsNip01EventWire {
        id,
        pubkey,
        created_at: 1_700_000_000,
        kind: 1,
        tags,
        content,
        sig: hex_128('b'),
        extra: Default::default(),
    };

    assert!(matches!(
        wire.into_envelope(),
        Err(RadrootsEventWireError::Envelope(
            RadrootsEventEnvelopeError::ContentTooLarge { .. }
        ))
    ));
}

#[cfg(feature = "serde")]
#[test]
fn serde_flatten_preserves_extra_fields() {
    let mut value = valid_event_value("hello", default_tags());
    value
        .as_object_mut()
        .expect("object")
        .insert("client".to_owned(), json!("radroots-test"));
    let wire = RadrootsNip01EventWire::parse_json(raw_json(&value).as_str()).expect("wire");
    let encoded = serde_json::to_value(&wire).expect("encoded");

    assert_eq!(encoded.get("client"), Some(&json!("radroots-test")));
    assert_eq!(encoded.get("id"), Some(&Value::String(wire.id)));
}

#[test]
fn parse_json_rejects_required_field_errors_and_id_mismatch() {
    let mut value = valid_event_value("hello", default_tags());
    value.as_object_mut().expect("object").remove("id");
    assert!(matches!(
        RadrootsNip01EventWire::parse_json(raw_json(&value).as_str()),
        Err(RadrootsEventWireError::MissingField("id"))
    ));

    let mut value = valid_event_value("hello", default_tags());
    value
        .as_object_mut()
        .expect("object")
        .insert("pubkey".to_owned(), json!("not-hex"));
    assert!(matches!(
        RadrootsNip01EventWire::parse_json(raw_json(&value).as_str()),
        Err(RadrootsEventWireError::InvalidIdentifier {
            field: "pubkey",
            ..
        })
    ));

    let mut value = valid_event_value("hello", default_tags());
    value
        .as_object_mut()
        .expect("object")
        .insert("sig".to_owned(), json!(hex_64('b')));
    assert!(matches!(
        RadrootsNip01EventWire::parse_json(raw_json(&value).as_str()),
        Err(RadrootsEventWireError::InvalidIdentifier { field: "sig", .. })
    ));

    let mut value = valid_event_value("hello", default_tags());
    value
        .as_object_mut()
        .expect("object")
        .insert("id".to_owned(), json!(hex_64('f')));
    assert!(matches!(
        RadrootsNip01EventWire::parse_json(raw_json(&value).as_str()),
        Err(RadrootsEventWireError::EventIdMismatch { .. })
    ));

    let mut value = valid_event_value("hello", default_tags());
    value
        .as_object_mut()
        .expect("object")
        .insert("id".to_owned(), json!(hex_64('A')));
    assert!(matches!(
        RadrootsNip01EventWire::parse_json(raw_json(&value).as_str()),
        Err(RadrootsEventWireError::NonCanonicalIdentifier { field: "id" })
    ));
}

#[test]
fn parse_json_rejects_tag_shape_errors() {
    let mut value = valid_event_value("hello", default_tags());
    value
        .as_object_mut()
        .expect("object")
        .insert("tags".to_owned(), json!([[]]));
    assert!(matches!(
        RadrootsNip01EventWire::parse_json(raw_json(&value).as_str()),
        Err(RadrootsEventWireError::EmptyTag { index: 0 })
    ));

    let mut value = valid_event_value("hello", default_tags());
    value
        .as_object_mut()
        .expect("object")
        .insert("tags".to_owned(), json!([["", "soil"]]));
    assert!(matches!(
        RadrootsNip01EventWire::parse_json(raw_json(&value).as_str()),
        Err(RadrootsEventWireError::EmptyTagKey { index: 0 })
    ));

    let mut value = valid_event_value("hello", default_tags());
    value
        .as_object_mut()
        .expect("object")
        .insert("tags".to_owned(), json!([["t\n", "soil"]]));
    assert!(matches!(
        RadrootsNip01EventWire::parse_json(raw_json(&value).as_str()),
        Err(RadrootsEventWireError::ControlCharacterTagKey { index: 0 })
    ));
}

#[test]
fn parse_json_rejects_resource_budget_violations() {
    let raw = valid_event_json("hello", default_tags());
    assert!(matches!(
        RadrootsNip01EventWire::parse_json_with_limits(
            raw.as_str(),
            RadrootsEventWireLimits {
                max_raw_json_bytes: 1,
                ..RadrootsEventWireLimits::default()
            }
        ),
        Err(RadrootsEventWireError::RawJsonTooLarge { .. })
    ));

    let raw = valid_event_json("hello", default_tags());
    assert!(matches!(
        RadrootsNip01EventWire::parse_json_with_limits(
            raw.as_str(),
            RadrootsEventWireLimits {
                max_content_bytes: 1,
                ..RadrootsEventWireLimits::default()
            }
        ),
        Err(RadrootsEventWireError::ContentTooLarge { .. })
    ));

    let raw = valid_event_json("hello", default_tags());
    assert!(matches!(
        RadrootsNip01EventWire::parse_json_with_limits(
            raw.as_str(),
            RadrootsEventWireLimits {
                max_tag_count: 0,
                ..RadrootsEventWireLimits::default()
            }
        ),
        Err(RadrootsEventWireError::TooManyTags { .. })
    ));

    let raw = valid_event_json("hello", default_tags());
    assert_eq!(
        RadrootsNip01EventWire::parse_json_with_limits(
            raw.as_str(),
            RadrootsEventWireLimits {
                max_total_tag_elements: 1,
                ..RadrootsEventWireLimits::default()
            }
        ),
        Err(RadrootsEventWireError::TooManyTagElements { max: 1, actual: 2 })
    );

    let raw = valid_event_json("hello", vec![vec!["t".to_owned(), "soil".to_owned()]]);
    assert!(matches!(
        RadrootsNip01EventWire::parse_json_with_limits(
            raw.as_str(),
            RadrootsEventWireLimits {
                max_tag_element_bytes: 1,
                ..RadrootsEventWireLimits::default()
            }
        ),
        Err(RadrootsEventWireError::TagElementTooLarge { .. })
    ));

    let raw = valid_event_json("hello", default_tags());
    assert!(matches!(
        RadrootsNip01EventWire::parse_json_with_limits(
            raw.as_str(),
            RadrootsEventWireLimits {
                max_total_tag_bytes: 1,
                ..RadrootsEventWireLimits::default()
            }
        ),
        Err(RadrootsEventWireError::TagsTooLarge { .. })
    ));

    let mut value = valid_event_value("hello", default_tags());
    value
        .as_object_mut()
        .expect("object")
        .insert("client".to_owned(), json!("radroots-test"));
    let raw = raw_json(&value);
    assert!(matches!(
        RadrootsNip01EventWire::parse_json_with_limits(
            raw.as_str(),
            RadrootsEventWireLimits {
                max_extra_fields: 0,
                ..RadrootsEventWireLimits::default()
            }
        ),
        Err(RadrootsEventWireError::TooManyExtraFields { .. })
    ));

    assert!(matches!(
        RadrootsNip01EventWire::parse_json_with_limits(
            raw.as_str(),
            RadrootsEventWireLimits {
                max_total_extra_json_bytes: 1,
                ..RadrootsEventWireLimits::default()
            }
        ),
        Err(RadrootsEventWireError::ExtraJsonTooLarge { .. })
    ));
}

#[test]
fn wire_accepts_exact_total_tag_element_boundary() {
    let raw = valid_event_json("hello", default_tags());
    let wire = RadrootsNip01EventWire::parse_json_with_limits(
        raw.as_str(),
        RadrootsEventWireLimits {
            max_total_tag_elements: 2,
            ..RadrootsEventWireLimits::default()
        },
    )
    .expect("wire at exact tag element boundary");

    assert_eq!(wire.tags, default_tags());
}

#[test]
fn wire_and_envelope_share_default_total_tag_element_boundary() {
    let exact_tags = tag_with_total_elements(DEFAULT_TAG_TOTAL_ELEMENT_MAX_COUNT);
    RadrootsNip01EventWire::parse_json(valid_event_json("", exact_tags.clone()).as_str())
        .expect("wire at default tag element boundary");
    RadrootsEventEnvelope::new(valid_envelope_parts("", exact_tags))
        .expect("envelope at default tag element boundary");

    let overflow_elements = DEFAULT_TAG_TOTAL_ELEMENT_MAX_COUNT + 1;
    let overflow_tags = tag_with_total_elements(overflow_elements);
    assert_eq!(
        RadrootsNip01EventWire::parse_json(valid_event_json("", overflow_tags.clone()).as_str()),
        Err(RadrootsEventWireError::TooManyTagElements {
            max: DEFAULT_TAG_TOTAL_ELEMENT_MAX_COUNT,
            actual: overflow_elements,
        })
    );
    assert_eq!(
        RadrootsEventEnvelope::new(valid_envelope_parts("", overflow_tags)),
        Err(RadrootsEventEnvelopeError::TooManyTagElements {
            max: DEFAULT_TAG_TOTAL_ELEMENT_MAX_COUNT,
            actual: overflow_elements,
        })
    );
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn wire_parser_and_error_contracts_cover_all_typed_failures() {
    let parse_error = RadrootsEventId::parse("bad").expect_err("invalid id");
    for error in [
        RadrootsEventWireError::Json("bad json".to_owned()),
        RadrootsEventWireError::RootNotObject,
        RadrootsEventWireError::MissingField("id"),
        RadrootsEventWireError::InvalidField("kind"),
        RadrootsEventWireError::InvalidIdentifier {
            field: "id",
            error: parse_error.clone(),
        },
        RadrootsEventWireError::NonCanonicalIdentifier { field: "id" },
        RadrootsEventWireError::RawJsonTooLarge { max: 1, actual: 2 },
        RadrootsEventWireError::ContentTooLarge { max: 1, actual: 2 },
        RadrootsEventWireError::TooManyTags { max: 1, actual: 2 },
        RadrootsEventWireError::TooManyTagElements { max: 1, actual: 2 },
        RadrootsEventWireError::EmptyTag { index: 1 },
        RadrootsEventWireError::EmptyTagKey { index: 1 },
        RadrootsEventWireError::ControlCharacterTagKey { index: 1 },
        RadrootsEventWireError::TagElementTooLarge {
            tag_index: 1,
            element_index: 2,
            max: 3,
            actual: 4,
        },
        RadrootsEventWireError::TagsTooLarge { max: 1, actual: 2 },
        RadrootsEventWireError::TooManyExtraFields { max: 1, actual: 2 },
        RadrootsEventWireError::ExtraJsonTooLarge { max: 1, actual: 2 },
        RadrootsEventWireError::from(RadrootsCanonicalEventIdError::InvalidPubkey(
            parse_error.clone(),
        )),
        RadrootsEventWireError::from(RadrootsEventEnvelopeError::NonCanonicalId),
        RadrootsEventWireError::EventIdMismatch {
            declared: "a".to_owned(),
            computed: "b".to_owned(),
        },
    ] {
        assert!(!error.to_string().is_empty());
    }

    for raw in ["{", "[]", "null"] {
        assert!(RadrootsNip01EventWire::parse_json(raw).is_err());
    }

    for (field, replacement) in [
        ("id", json!(7)),
        ("id", json!("bad")),
        ("pubkey", json!(7)),
        ("pubkey", json!(hex_64('A'))),
        ("created_at", json!("bad")),
        ("created_at", json!(-1)),
        ("kind", json!("bad")),
        ("kind", json!(u64::from(u32::MAX) + 1)),
        ("tags", json!("bad")),
        ("tags", json!(["bad"])),
        ("tags", json!([["t", 7]])),
        ("content", json!(7)),
        ("sig", json!(7)),
        ("sig", json!("bad")),
        ("sig", json!(hex_128('B'))),
    ] {
        let mut value = valid_event_value("hello", default_tags());
        value
            .as_object_mut()
            .expect("object")
            .insert(field.to_owned(), replacement);
        assert!(RadrootsNip01EventWire::parse_json(raw_json(&value).as_str()).is_err());
    }

    for field in ["pubkey", "created_at", "kind", "tags", "content", "sig"] {
        let mut value = valid_event_value("hello", default_tags());
        value.as_object_mut().expect("object").remove(field);
        assert!(RadrootsNip01EventWire::parse_json(raw_json(&value).as_str()).is_err());
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn checked_in_conformance_vectors_match_wire_behavior() {
    let vectors =
        include_str!("../../../../../contracts/conformance/vectors/event/nip01_wire.v1.json");
    let document: Value = serde_json::from_str(vectors).expect("vectors json");
    let entries = document
        .get("vectors")
        .and_then(Value::as_array)
        .expect("vector entries");

    for entry in entries {
        match entry.get("kind").and_then(Value::as_str).expect("kind") {
            "event.nip01_wire.valid" => {
                let raw = entry
                    .get("input")
                    .and_then(|input| input.get("raw_json"))
                    .and_then(Value::as_str)
                    .expect("raw json");
                let expected = entry.get("expected").expect("expected");
                let wire = RadrootsNip01EventWire::parse_json(raw).expect("wire");
                assert_eq!(
                    wire.canonical_id_preimage().expect("preimage"),
                    expected
                        .get("canonical_id_preimage")
                        .and_then(Value::as_str)
                        .expect("expected preimage")
                );
                assert_eq!(
                    wire.computed_event_id().expect("event id").as_str(),
                    expected
                        .get("computed_event_id")
                        .and_then(Value::as_str)
                        .expect("expected event id")
                );
            }
            "event.nip01_wire.invalid" => {
                let raw = entry
                    .get("input")
                    .and_then(|input| input.get("raw_json"))
                    .and_then(Value::as_str)
                    .expect("raw json");
                assert!(RadrootsNip01EventWire::parse_json(raw).is_err());
            }
            other => panic!("unknown event wire vector kind {other}"),
        }
    }
}
