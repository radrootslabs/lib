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
    let frame = RadrootsMeshFrame::new(scope, Vec::new());
    let encoded = encode_mesh_frame_cbor(&frame).expect("encode custom scope");
    let decoded = decode_mesh_frame_cbor(&encoded).expect("decode custom scope");

    assert_eq!(decoded.scope.cbor_label(), "custom:farm-north");
}
