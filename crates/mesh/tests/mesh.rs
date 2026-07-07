use radroots_mesh::{
    RadrootsMeshError, RadrootsMeshEventHead, RadrootsMeshFrame, RadrootsMeshPayloadPolicy,
    RadrootsMeshScope, decode_mesh_frame_cbor, encode_mesh_frame_cbor,
};

#[test]
fn default_frame_encodes_as_deterministic_cbor() {
    let frame = RadrootsMeshFrame::new(RadrootsMeshScope::Local, Vec::new());
    let encoded = encode_mesh_frame_cbor(&frame).expect("encode frame");

    assert_eq!(
        encoded,
        [
            0xa5, 0x01, 0x01, 0x02, 0x65, b'l', b'o', b'c', b'a', b'l', 0x03, 0x71, b'p', b'a',
            b'y', b'l', b'o', b'a', b'd', b'-', b'f', b'o', b'r', b'b', b'i', b'd', b'd', b'e',
            b'n', 0x04, 0x80, 0x05, 0xf6,
        ]
    );
    assert_eq!(
        decode_mesh_frame_cbor(&encoded).expect("decode frame"),
        frame
    );
}

#[test]
fn event_head_frames_round_trip() {
    let frame = RadrootsMeshFrame::new(
        RadrootsMeshScope::Community,
        vec![RadrootsMeshEventHead {
            event_id: "event-1".to_string(),
            author: "author-1".to_string(),
            kind: 30818,
            created_at: 1_725_000_000,
        }],
    );
    let encoded = encode_mesh_frame_cbor(&frame).expect("encode frame");
    let decoded = decode_mesh_frame_cbor(&encoded).expect("decode frame");

    assert_eq!(decoded, frame);
    assert_eq!(encode_mesh_frame_cbor(&decoded).expect("reencode"), encoded);
}

#[test]
fn payload_transmission_is_forbidden_in_mvp_frames() {
    let mut frame = RadrootsMeshFrame::new(RadrootsMeshScope::Local, Vec::new());
    frame.payload_policy = RadrootsMeshPayloadPolicy::PayloadTransmissionForbidden;
    frame.payload = Some(vec![1, 2, 3]);

    assert_eq!(
        encode_mesh_frame_cbor(&frame).expect_err("payload must fail"),
        RadrootsMeshError::PayloadTransmissionForbidden
    );
}

#[test]
fn custom_scope_has_explicit_namespace() {
    let scope = RadrootsMeshScope::custom(" Farm-North ").expect("custom scope");
    assert_eq!(scope.label(), "farm-north");
    let frame = RadrootsMeshFrame::new(scope, Vec::new());
    let encoded = encode_mesh_frame_cbor(&frame).expect("encode custom scope");
    let decoded = decode_mesh_frame_cbor(&encoded).expect("decode custom scope");

    assert_eq!(decoded.scope.cbor_label(), "custom:farm-north");
}

#[test]
fn mesh_scope_and_payload_policy_parsers_reject_unknown_values() {
    assert_eq!(
        RadrootsMeshScope::custom(" ").expect_err("empty custom scope"),
        RadrootsMeshError::EmptyCustomScope
    );
    assert_eq!(
        RadrootsMeshScope::parse("unscoped").expect_err("unknown scope"),
        RadrootsMeshError::UnknownScope
    );
    assert_eq!(RadrootsMeshScope::Local.label(), "local");
    assert_eq!(RadrootsMeshScope::Community.label(), "community");
    assert_eq!(
        RadrootsMeshPayloadPolicy::parse("inline-payloads").expect_err("unknown policy"),
        RadrootsMeshError::UnknownPayloadPolicy
    );
    assert_eq!(
        RadrootsMeshPayloadPolicy::PayloadTransmissionForbidden.label(),
        "payload-forbidden"
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
            RadrootsMeshError::PayloadTransmissionForbidden,
            "mesh payload transmission is forbidden",
        ),
        (RadrootsMeshError::InvalidCbor, "mesh frame CBOR is invalid"),
        (
            RadrootsMeshError::InvalidUtf8,
            "mesh frame text is invalid UTF-8",
        ),
        (RadrootsMeshError::UnknownScope, "mesh scope is unknown"),
        (
            RadrootsMeshError::UnknownPayloadPolicy,
            "mesh payload policy is unknown",
        ),
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
fn cbor_codec_covers_extended_integer_widths() {
    let frame = RadrootsMeshFrame::new(
        RadrootsMeshScope::Community,
        vec![RadrootsMeshEventHead {
            event_id: "event-with-wide-created-at".to_string(),
            author: "author-with-wide-created-at".to_string(),
            kind: 24,
            created_at: u64::MAX,
        }],
    );
    let encoded = encode_mesh_frame_cbor(&frame).expect("encode wide frame");
    let decoded = decode_mesh_frame_cbor(&encoded).expect("decode wide frame");

    assert_eq!(decoded, frame);
}

#[test]
fn decoder_rejects_malformed_cbor_shapes() {
    let encoded = encode_mesh_frame_cbor(&RadrootsMeshFrame::new(
        RadrootsMeshScope::Local,
        Vec::new(),
    ))
    .expect("encode default");
    let mut unsupported_version = encoded.clone();
    unsupported_version[2] = 2;
    assert_eq!(
        decode_mesh_frame_cbor(&unsupported_version).expect_err("unsupported version"),
        RadrootsMeshError::UnsupportedVersion
    );
    let mut frame = RadrootsMeshFrame::new(RadrootsMeshScope::Local, Vec::new());
    frame.version = 2;
    assert_eq!(
        frame.validate().expect_err("unsupported frame version"),
        RadrootsMeshError::UnsupportedVersion
    );

    let cases = [
        vec![0x80],
        vec![0xbc],
        vec![0xa4],
        vec![0xa5, 0x02],
        vec![0xa5, 0x01, 0x01, 0x02, 0x61, 0xff],
        vec![
            0xa5, 0x01, 0x01, 0x02, 0x63, b'b', b'a', b'd', 0x03, 0x71, b'p', b'a', b'y', b'l',
            b'o', b'a', b'd', b'-', b'f', b'o', b'r', b'b', b'i', b'd', b'd', b'e', b'n', 0x04,
            0x80, 0x05, 0xf6,
        ],
        vec![
            0xa5, 0x01, 0x01, 0x02, 0x65, b'l', b'o', b'c', b'a', b'l', 0x03, 0x67, b'u', b'n',
            b'k', b'n', b'o', b'w', b'n', 0x04, 0x80, 0x05, 0xf6,
        ],
        vec![
            0xa5, 0x01, 0x01, 0x02, 0x65, b'l', b'o', b'c', b'a', b'l', 0x03, 0x71, b'p', b'a',
            b'y', b'l', b'o', b'a', b'd', b'-', b'f', b'o', b'r', b'b', b'i', b'd', b'd', b'e',
            b'n', 0x04, 0x80, 0x05, 0x00,
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
