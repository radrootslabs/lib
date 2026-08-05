use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use radroots_secrets::context::{
    EnvelopeContext, EnvelopePurpose, EnvelopeSubject, PayloadSchemaId,
};

const V1_ENVELOPE_HEX: &str = "52525331000101010100000007000c656e76656c6f70652d6b6579222222222222222222222222222222222222222222222222000000204b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b00000028f106837e33d690e7c5287abdd815ce9257b7b5b176ea9596abf3b7fe745aec5a8c2487a553d4659d";
const V2_CONTEXT_HEX: &str = "0001726164726f6f74732e656e76656c6f70655f636f6e746578742e76310019726164726f6f74732e707269766174655f61727469666163740010707269766174655f617274696661637400203031303130313031303130313031303130313031303130313031303130313031001674726164652e707269766174655f7465726d732e7631";
const V2_HEADER_HEX: &str = "52525331000201010100000007000c656e76656c6f70652d6b65790001726164726f6f74732e656e76656c6f70655f636f6e746578742e76310019726164726f6f74732e707269766174655f61727469666163740010707269766174655f617274696661637400203031303130313031303130313031303130313031303130313031303130313031001674726164652e707269766174655f7465726d732e7631222222222222222222222222222222222222222222222222000000204b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b00000028";
const V2_CIPHERTEXT_HEX: &str =
    "f106837e33d690e7c5287abdd815ce9257b7b5b176ea9596f762615d1c221c5c10964c4f799d5d59";
const V2_ENVELOPE_HEX: &str = "52525331000201010100000007000c656e76656c6f70652d6b65790001726164726f6f74732e656e76656c6f70655f636f6e746578742e76310019726164726f6f74732e707269766174655f61727469666163740010707269766174655f617274696661637400203031303130313031303130313031303130313031303130313031303130313031001674726164652e707269766174655f7465726d732e7631222222222222222222222222222222222222222222222222000000204b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b00000028f106837e33d690e7c5287abdd815ce9257b7b5b176ea9596f762615d1c221c5c10964c4f799d5d59";

fn context() -> EnvelopeContext {
    EnvelopeContext::new(
        EnvelopePurpose::parse("radroots.private_artifact").expect("purpose"),
        EnvelopeSubject::parse("private_artifact", "01010101010101010101010101010101")
            .expect("subject"),
        PayloadSchemaId::parse("trade.private_terms.v1").expect("schema"),
    )
}

fn independent_v2_vector() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let context = context().to_canonical_bytes();
    let mut header = Vec::new();
    header.extend_from_slice(b"RRS1");
    header.extend_from_slice(&2_u16.to_be_bytes());
    header.extend_from_slice(&[1, 1, 1]);
    header.extend_from_slice(&7_u32.to_be_bytes());
    header.extend_from_slice(&12_u16.to_be_bytes());
    header.extend_from_slice(b"envelope-key");
    header.extend_from_slice(&context);
    header.extend_from_slice(&[0x22; 24]);
    header.extend_from_slice(&32_u32.to_be_bytes());
    header.extend_from_slice(&[0x4b; 32]);
    header.extend_from_slice(&40_u32.to_be_bytes());

    let cipher = XChaCha20Poly1305::new(Key::from_slice(&[0x11; 32]));
    let ciphertext = cipher
        .encrypt(
            XNonce::from_slice(&[0x22; 24]),
            Payload {
                msg: b"radroots envelope vector",
                aad: &header,
            },
        )
        .expect("independent encryption");
    let mut envelope = header.clone();
    envelope.extend_from_slice(&ciphertext);
    (header, ciphertext, envelope)
}

#[test]
fn v1_corpus_is_stable_and_strictly_bounded() {
    let bytes = hex::decode(V1_ENVELOPE_HEX).expect("v1 vector hex");
    assert_eq!(&bytes[..4], b"RRS1");
    assert_eq!(u16::from_be_bytes([bytes[4], bytes[5]]), 1);
    assert!(bytes.len() < radroots_secrets::envelope::ENVELOPE_MAX_BYTES);
    let decoded = radroots_secrets::EncryptedEnvelope::decode(&bytes).expect("v1 corpus decode");
    assert_eq!(decoded.encode().expect("v1 re-encode"), bytes);
}

#[test]
fn v2_vector_freezes_context_header_ciphertext_and_envelope() {
    let (header, ciphertext, envelope) = independent_v2_vector();
    assert_eq!(hex::encode(context().to_canonical_bytes()), V2_CONTEXT_HEX);
    assert_eq!(hex::encode(header), V2_HEADER_HEX);
    assert_eq!(hex::encode(ciphertext), V2_CIPHERTEXT_HEX);
    assert_eq!(hex::encode(envelope), V2_ENVELOPE_HEX);
}

#[test]
fn v2_header_positions_and_length_prefixes_are_exact() {
    let (header, ciphertext, envelope) = independent_v2_vector();
    let context_bytes = context().to_canonical_bytes();
    assert_eq!(&header[..4], b"RRS1");
    assert_eq!(&header[4..6], &2_u16.to_be_bytes());
    assert_eq!(&header[6..9], &[1, 1, 1]);
    assert_eq!(&header[9..13], &7_u32.to_be_bytes());
    assert_eq!(&header[13..15], &12_u16.to_be_bytes());
    assert_eq!(&header[15..27], b"envelope-key");
    assert_eq!(&header[27..27 + context_bytes.len()], &context_bytes);
    let nonce_offset = 27 + context_bytes.len();
    assert_eq!(&header[nonce_offset..nonce_offset + 24], &[0x22; 24]);
    let wrapped_length_offset = nonce_offset + 24;
    assert_eq!(
        &header[wrapped_length_offset..wrapped_length_offset + 4],
        &32_u32.to_be_bytes()
    );
    assert_eq!(
        &header[wrapped_length_offset + 4..wrapped_length_offset + 36],
        &[0x4b; 32]
    );
    assert_eq!(&header[header.len() - 4..], &40_u32.to_be_bytes());
    assert_eq!(ciphertext.len(), 40);
    assert_eq!(envelope.len(), header.len() + ciphertext.len());
}

#[test]
fn vector_corruption_and_truncation_change_authenticated_material() {
    let (_, _, envelope) = independent_v2_vector();
    for offset in [0, 4, 6, 8, 9, 13, 15, 27, envelope.len() - 1] {
        let mut corrupted = envelope.clone();
        corrupted[offset] ^= 1;
        assert_ne!(hex::encode(corrupted), V2_ENVELOPE_HEX);
    }
    for length in [0, 1, 4, 6, 27, envelope.len() - 1] {
        assert_ne!(hex::encode(&envelope[..length]), V2_ENVELOPE_HEX);
    }
}
