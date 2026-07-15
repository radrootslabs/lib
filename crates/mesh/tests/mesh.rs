use radroots_mesh::{
    RADROOTS_MESH_PREVIEW_DENIAL_MESSAGE, RADROOTS_MESH_PREVIEW_POLICY_ID,
    RadrootsMeshAdmissionDecision, RadrootsMeshAdmissionInput, RadrootsMeshCompressionPolicy,
    RadrootsMeshError, RadrootsMeshFrame, RadrootsMeshFrameType, RadrootsMeshPayload,
    RadrootsMeshPayloadPolicy, RadrootsMeshPolicyDenyReason, RadrootsMeshPrivacyClass,
    RadrootsMeshScope, decode_mesh_frame_cbor, encode_mesh_frame_cbor,
};
use serde_json::Value;

fn default_frame() -> RadrootsMeshFrame {
    RadrootsMeshFrame::new(
        RadrootsMeshFrameType::Hello,
        RadrootsMeshScope::Local,
        "message-1",
        42,
        60_000,
    )
}

fn default_encoded_with_replacement(
    start: usize,
    end: usize,
    replacement: impl IntoIterator<Item = u8>,
) -> Vec<u8> {
    let mut encoded = encode_mesh_frame_cbor(&default_frame()).expect("encode default");
    encoded.splice(start..end, replacement);
    encoded
}

#[test]
fn default_frame_encodes_as_mesh_frame_v1_cddl_cbor() {
    let frame = default_frame();
    let encoded = encode_mesh_frame_cbor(&frame).expect("encode frame");

    assert_eq!(
        encoded,
        [
            0xa7, 0x00, 0x01, 0x01, 0x00, 0x02, 0x65, b'l', b'o', b'c', b'a', b'l', 0x03, 0x69,
            b'm', b'e', b's', b's', b'a', b'g', b'e', b'-', b'1', 0x04, 0x18, 0x2a, 0x05, 0x19,
            0xea, 0x60, 0x06, 0xa0,
        ]
    );
    assert_eq!(
        decode_mesh_frame_cbor(&encoded).expect("decode frame"),
        frame
    );
}

#[test]
fn all_frame_types_round_trip_with_stable_codes_and_labels() {
    let cases = [
        (RadrootsMeshFrameType::Hello, 0, "hello"),
        (
            RadrootsMeshFrameType::EventHeadAnnounce,
            1,
            "event_head_announce",
        ),
        (RadrootsMeshFrameType::EventRequest, 2, "event_request"),
        (RadrootsMeshFrameType::EventChunk, 3, "event_chunk"),
        (RadrootsMeshFrameType::EventAck, 4, "event_ack"),
        (RadrootsMeshFrameType::RouteProbe, 5, "route_probe"),
    ];

    for (frame_type, code, label) in cases {
        let frame = RadrootsMeshFrame::new(
            frame_type,
            RadrootsMeshScope::Community,
            format!("{label}-message"),
            code + 1,
            1_000,
        );
        let encoded = encode_mesh_frame_cbor(&frame).expect("encode frame");
        let decoded = decode_mesh_frame_cbor(&encoded).expect("decode frame");

        assert_eq!(decoded, frame);
        assert_eq!(encode_mesh_frame_cbor(&decoded).expect("reencode"), encoded);
        assert_eq!(frame_type.code(), code);
        assert_eq!(frame_type.label(), label);
    }
}

#[test]
fn preview_policy_has_zero_delivery_budgets_and_disabled_compression() {
    let policy = RadrootsMeshPayloadPolicy::preview_unavailable();

    assert_eq!(policy.policy_id(), RADROOTS_MESH_PREVIEW_POLICY_ID);
    assert_eq!(policy.max_payload_bytes, 0);
    assert_eq!(policy.max_frame_bytes, 0);
    assert_eq!(policy.compression, RadrootsMeshCompressionPolicy::Disabled);
    assert_eq!(policy.compression.label(), "disabled");
    assert!(!policy.custom_scopes_enabled);
    assert!(!policy.usable_for_delivery());
}

#[test]
fn preview_policy_denies_real_payload_admission_deterministically() {
    let policy = RadrootsMeshPayloadPolicy::preview_unavailable();
    let input = RadrootsMeshAdmissionInput::new(
        RadrootsMeshScope::Local,
        RadrootsMeshPrivacyClass::PublicEvent,
        256,
        512,
    );
    let decision = policy.evaluate(&input);

    assert_eq!(
        decision,
        RadrootsMeshAdmissionDecision::Denied {
            reason: RadrootsMeshPolicyDenyReason::PreviewUnavailable
        }
    );
    assert_eq!(decision.label(), "denied");
    assert_eq!(
        decision.deny_reason(),
        Some(RadrootsMeshPolicyDenyReason::PreviewUnavailable)
    );
    assert_eq!(decision.message(), RADROOTS_MESH_PREVIEW_DENIAL_MESSAGE);
    assert!(!decision.usable_for_delivery());
    assert_eq!(
        RadrootsMeshPrivacyClass::PublicEvent.label(),
        "public_event"
    );
    assert_eq!(
        RadrootsMeshPrivacyClass::PrivateEvent.label(),
        "private_event"
    );
    assert_eq!(
        RadrootsMeshPolicyDenyReason::PreviewUnavailable.label(),
        "preview_unavailable"
    );
}

#[test]
fn custom_scope_has_explicit_namespace() {
    let scope = RadrootsMeshScope::custom("farm-north.preview_1").expect("custom scope");
    assert_eq!(scope.label(), "farm-north.preview_1");
    let frame = RadrootsMeshFrame::new(
        RadrootsMeshFrameType::RouteProbe,
        scope,
        "route-probe-1",
        10,
        1,
    );
    let encoded = encode_mesh_frame_cbor(&frame).expect("encode custom scope");
    let decoded = decode_mesh_frame_cbor(&encoded).expect("decode custom scope");

    assert_eq!(decoded.scope_id.cbor_label(), "custom:farm-north.preview_1");
    assert_eq!(encode_mesh_frame_cbor(&decoded).expect("reencode"), encoded);
}

#[test]
fn mesh_parsers_and_validation_reject_unknown_empty_or_invalid_values() {
    assert_eq!(
        RadrootsMeshScope::custom("").expect_err("empty custom scope"),
        RadrootsMeshError::EmptyCustomScope
    );
    for invalid in [
        " ",
        " farm-north",
        "farm-north ",
        "farm north",
        "farm/north",
        "farm:north",
        "farm\nnorth",
    ] {
        assert_eq!(
            RadrootsMeshScope::custom(invalid).expect_err("invalid custom scope"),
            RadrootsMeshError::InvalidCustomScope
        );
    }
    assert_eq!(
        RadrootsMeshScope::parse("custom:").expect_err("empty parsed custom scope"),
        RadrootsMeshError::EmptyCustomScope
    );
    assert_eq!(
        RadrootsMeshScope::parse("custom:farm north").expect_err("invalid parsed custom scope"),
        RadrootsMeshError::InvalidCustomScope
    );
    assert_eq!(
        RadrootsMeshScope::parse("unscoped").expect_err("unknown scope"),
        RadrootsMeshError::UnknownScope
    );
    assert_eq!(
        RadrootsMeshFrameType::parse_code(6).expect_err("unknown frame type"),
        RadrootsMeshError::UnknownFrameType
    );
    assert_eq!(RadrootsMeshScope::Local.label(), "local");
    assert_eq!(RadrootsMeshScope::Community.label(), "community");

    let empty_message = RadrootsMeshFrame::new(
        RadrootsMeshFrameType::Hello,
        RadrootsMeshScope::Local,
        " ",
        1,
        1,
    );
    assert_eq!(
        empty_message.validate().expect_err("empty message id"),
        RadrootsMeshError::EmptyMessageId
    );

    let zero_ttl = RadrootsMeshFrame::new(
        RadrootsMeshFrameType::Hello,
        RadrootsMeshScope::Local,
        "message-1",
        1,
        0,
    );
    assert_eq!(
        zero_ttl.validate().expect_err("zero ttl"),
        RadrootsMeshError::InvalidTtl
    );
}

#[test]
fn mesh_errors_have_stable_display_strings() {
    let cases = [
        (
            RadrootsMeshError::EmptyCustomScope,
            "mesh custom scope is empty",
        ),
        (
            RadrootsMeshError::InvalidCustomScope,
            "mesh custom scope is invalid",
        ),
        (
            RadrootsMeshError::EmptyMessageId,
            "mesh message id is empty",
        ),
        (RadrootsMeshError::InvalidTtl, "mesh frame TTL is invalid"),
        (
            RadrootsMeshError::PayloadTransmissionForbidden,
            "mesh payload transmission is forbidden",
        ),
        (RadrootsMeshError::InvalidCbor, "mesh frame CBOR is invalid"),
        (
            RadrootsMeshError::InvalidUtf8,
            "mesh frame text is invalid UTF-8",
        ),
        (
            RadrootsMeshError::UnknownFrameType,
            "mesh frame type is unknown",
        ),
        (RadrootsMeshError::UnknownScope, "mesh scope is unknown"),
        (
            RadrootsMeshError::UnsupportedVersion,
            "mesh frame version is unsupported",
        ),
    ];

    for (error, message) in cases {
        assert_eq!(error.to_string(), message);
    }
}

#[test]
fn payload_transmission_is_forbidden_in_mvp_frames() {
    let mut frame = default_frame();
    frame.payload = RadrootsMeshPayload::Bytes(vec![1, 2, 3]);

    assert_eq!(
        encode_mesh_frame_cbor(&frame).expect_err("payload must fail"),
        RadrootsMeshError::PayloadTransmissionForbidden
    );

    let mut encoded_payload = encode_mesh_frame_cbor(&default_frame()).expect("encode default");
    let payload_offset = encoded_payload.len() - 1;
    encoded_payload[payload_offset] = 0x43;
    encoded_payload.extend_from_slice(&[1, 2, 3]);
    assert_eq!(
        decode_mesh_frame_cbor(&encoded_payload).expect_err("decode payload"),
        RadrootsMeshError::PayloadTransmissionForbidden
    );
}

#[test]
fn cbor_codec_covers_extended_integer_widths() {
    let frame = RadrootsMeshFrame::new(
        RadrootsMeshFrameType::EventAck,
        RadrootsMeshScope::Community,
        "wide-created-at",
        u64::MAX,
        u64::MAX,
    );
    let encoded = encode_mesh_frame_cbor(&frame).expect("encode wide frame");
    let decoded = decode_mesh_frame_cbor(&encoded).expect("decode wide frame");

    assert_eq!(decoded, frame);
}

#[test]
fn decoder_rejects_noncanonical_cbor_widths() {
    let cases = [
        (
            "over-wide map length",
            default_encoded_with_replacement(0, 1, [0xb8, 0x07]),
        ),
        (
            "over-wide key",
            default_encoded_with_replacement(1, 2, [0x18, 0x00]),
        ),
        (
            "over-wide version",
            default_encoded_with_replacement(2, 3, [0x18, 0x01]),
        ),
        (
            "over-wide frame type",
            default_encoded_with_replacement(4, 5, [0x1a, 0x00, 0x00, 0x00, 0x00]),
        ),
        (
            "over-wide text length",
            default_encoded_with_replacement(6, 7, [0x78, 0x05]),
        ),
        (
            "over-wide created-at",
            default_encoded_with_replacement(24, 26, [0x19, 0x00, 0x2a]),
        ),
        (
            "over-wide ttl",
            default_encoded_with_replacement(
                27,
                30,
                [0x1b, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xea, 0x60],
            ),
        ),
        (
            "over-wide forbidden byte payload length",
            default_encoded_with_replacement(31, 32, [0x58, 0x03, 1, 2, 3]),
        ),
    ];

    for (label, encoded) in cases {
        assert_eq!(
            decode_mesh_frame_cbor(&encoded).expect_err(label),
            RadrootsMeshError::InvalidCbor
        );
    }
}

#[test]
fn decoder_rejects_previous_five_field_frame_shape() {
    let previous_shape = vec![
        0xa5, 0x01, 0x01, 0x02, 0x65, b'l', b'o', b'c', b'a', b'l', 0x03, 0x71, b'p', b'a', b'y',
        b'l', b'o', b'a', b'd', b'-', b'f', b'o', b'r', b'b', b'i', b'd', b'd', b'e', b'n', 0x04,
        0x80, 0x05, 0xf6,
    ];

    assert_eq!(
        decode_mesh_frame_cbor(&previous_shape).expect_err("old frame shape"),
        RadrootsMeshError::InvalidCbor
    );
}

#[test]
fn decoder_rejects_malformed_cbor_shapes() {
    let encoded = encode_mesh_frame_cbor(&default_frame()).expect("encode default");
    let invalid_custom_scope = default_encoded_with_replacement(
        6,
        12,
        [
            0x70, b'c', b'u', b's', b't', b'o', b'm', b':', b'f', b'a', b'r', b'm', b' ', b'n',
            b'o', b'r', b't',
        ],
    );
    assert_eq!(
        decode_mesh_frame_cbor(&invalid_custom_scope).expect_err("invalid custom scope"),
        RadrootsMeshError::InvalidCustomScope
    );

    let mut unsupported_version = encoded.clone();
    unsupported_version[2] = 2;
    assert_eq!(
        decode_mesh_frame_cbor(&unsupported_version).expect_err("unsupported version"),
        RadrootsMeshError::UnsupportedVersion
    );

    let mut out_of_range_version = encoded.clone();
    out_of_range_version.splice(2..3, [0x1a, 0x00, 0x01, 0x00, 0x01]);
    assert_eq!(
        decode_mesh_frame_cbor(&out_of_range_version).expect_err("out of range version"),
        RadrootsMeshError::InvalidCbor
    );

    let mut unknown_frame_type = encoded.clone();
    unknown_frame_type[4] = 6;
    assert_eq!(
        decode_mesh_frame_cbor(&unknown_frame_type).expect_err("unknown frame type"),
        RadrootsMeshError::UnknownFrameType
    );

    let mut zero_ttl = encoded.clone();
    zero_ttl.splice(27..30, [0x00]);
    assert_eq!(
        decode_mesh_frame_cbor(&zero_ttl).expect_err("zero ttl"),
        RadrootsMeshError::InvalidTtl
    );

    let mut wrong_key_order = encoded.clone();
    wrong_key_order[3] = 2;
    assert_eq!(
        decode_mesh_frame_cbor(&wrong_key_order).expect_err("wrong key order"),
        RadrootsMeshError::InvalidCbor
    );

    let cases = [
        vec![0x80],
        vec![0xbc],
        vec![0xa6],
        vec![0xa7, 0x01],
        vec![0xa7, 0x00, 0x01, 0x01, 0x00, 0x02, 0x61, 0xff],
        vec![
            0xa7, 0x00, 0x01, 0x01, 0x00, 0x02, 0x63, b'b', b'a', b'd', 0x03, 0x69, b'm', b'e',
            b's', b's', b'a', b'g', b'e', b'-', b'1', 0x04, 0x18, 0x2a, 0x05, 0x19, 0xea, 0x60,
            0x06, 0xa0,
        ],
        vec![
            0xa7, 0x00, 0x01, 0x01, 0x00, 0x02, 0x7b, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff,
        ],
        vec![
            0xa7, 0x00, 0x01, 0x01, 0x00, 0x02, 0x65, b'l', b'o', b'c', b'a', b'l', 0x03, 0x60,
            0x04, 0x18, 0x2a, 0x05, 0x19, 0xea, 0x60, 0x06, 0xa0,
        ],
        vec![
            0xa7, 0x00, 0x01, 0x01, 0x00, 0x02, 0x65, b'l', b'o', b'c', b'a', b'l', 0x03, 0x69,
            b'm', b'e', b's', b's', b'a', b'g', b'e', b'-', b'1', 0x04, 0x18, 0x2a, 0x05, 0x19,
            0xea, 0x60, 0x06, 0xa1,
        ],
    ];

    for malformed in cases {
        assert!(decode_mesh_frame_cbor(&malformed).is_err());
    }

    let mut trailing = encoded;
    trailing.push(0x00);
    assert_eq!(
        decode_mesh_frame_cbor(&trailing).expect_err("trailing byte"),
        RadrootsMeshError::InvalidCbor
    );
}

#[test]
fn checked_in_mesh_cbor_vectors_match_decoder_behavior() {
    let vectors = include_str!("../../../contracts/conformance/vectors/mesh/frame_cbor.v1.json");
    let document: Value = serde_json::from_str(vectors).expect("mesh cbor vector json");
    let entries = document
        .get("vectors")
        .and_then(Value::as_array)
        .expect("mesh cbor vectors");

    for entry in entries {
        let kind = entry.get("kind").and_then(Value::as_str).expect("kind");
        let hex = entry
            .get("input")
            .and_then(|input| input.get("hex"))
            .and_then(Value::as_str)
            .expect("input hex");
        let bytes = decode_hex(hex);
        match kind {
            "mesh.frame_cbor.valid" => {
                let frame = decode_mesh_frame_cbor(bytes.as_slice()).expect("mesh frame");
                let expected = entry.get("expected").expect("expected");
                assert_eq!(
                    frame.message_id,
                    expected
                        .get("message_id")
                        .and_then(Value::as_str)
                        .expect("message id")
                );
                assert_eq!(
                    frame.scope_id.label(),
                    expected
                        .get("scope")
                        .and_then(Value::as_str)
                        .expect("scope")
                );
            }
            "mesh.frame_cbor.invalid" => {
                assert!(decode_mesh_frame_cbor(bytes.as_slice()).is_err());
            }
            other => panic!("unknown mesh cbor vector kind {other}"),
        }
    }
}

fn decode_hex(value: &str) -> Vec<u8> {
    assert_eq!(value.len() % 2, 0, "hex fixture length must be even");
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|chunk| {
            let high = hex_nibble(chunk[0]);
            let low = hex_nibble(chunk[1]);
            (high << 4) | low
        })
        .collect()
}

fn hex_nibble(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        _ => panic!("hex fixture contains non-lowercase-hex byte"),
    }
}
