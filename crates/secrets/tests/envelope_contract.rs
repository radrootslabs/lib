use futures_executor::block_on;
use radroots_secrets::context::{
    EnvelopeContext, EnvelopePurpose, EnvelopeSubject, PayloadSchemaId,
};
use radroots_secrets::envelope::{
    Cipher, ENVELOPE_VERSION, KeySource, Nonce, SealMaterial, SealRequest,
};
use radroots_secrets::error::Operation;
use radroots_secrets::id::{BackendKind, KeyVersion};
use radroots_secrets::wrapping::{
    BoxFuture, SecretMaterial, UnwrapRequest, WrapRequest, WrappedSecret,
};
use radroots_secrets::{EncryptedEnvelope, Error, KeyWrapping, SecretId, SecretRef};

struct VectorWrapping;

impl KeyWrapping for VectorWrapping {
    fn wrap<'a>(&'a self, request: WrapRequest<'a>) -> BoxFuture<'a, Result<WrappedSecret, Error>> {
        Box::pin(async move {
            if request.reference().id().as_str() != "envelope-key" {
                return Err(Error::BackendFailure {
                    backend: BackendKind::Memory,
                    operation: Operation::Wrap,
                });
            }
            let wrapped = request
                .plaintext()
                .expose_secret(|bytes| bytes.iter().map(|byte| byte ^ 0x5A).collect::<Vec<_>>());
            WrappedSecret::from_bytes(wrapped)
        })
    }

    fn unwrap<'a>(
        &'a self,
        request: UnwrapRequest<'a>,
    ) -> BoxFuture<'a, Result<SecretMaterial, Error>> {
        Box::pin(async move {
            if request.reference().id().as_str() != "envelope-key" {
                return Err(Error::BackendFailure {
                    backend: BackendKind::Memory,
                    operation: Operation::Unwrap,
                });
            }
            let plaintext = request
                .wrapped()
                .as_bytes()
                .iter()
                .map(|byte| byte ^ 0x5A)
                .collect::<Vec<_>>();
            SecretMaterial::from_slice(plaintext.as_slice())
        })
    }
}

fn reference() -> SecretRef {
    SecretRef::new(
        SecretId::parse("envelope-key").expect("valid id"),
        BackendKind::Memory,
        KeyVersion::new(7).expect("valid version"),
    )
}

fn context() -> EnvelopeContext {
    EnvelopeContext::new(
        EnvelopePurpose::parse("radroots.private_artifact").expect("purpose"),
        EnvelopeSubject::parse("private_artifact", "01010101010101010101010101010101")
            .expect("subject"),
        PayloadSchemaId::parse("trade.private_terms.v1").expect("schema"),
    )
}

fn seal(plaintext: &[u8]) -> EncryptedEnvelope {
    let plaintext = SecretMaterial::from_slice(plaintext).expect("plaintext");
    let data_key = SecretMaterial::from_slice(&[0x11; 32]).expect("data key");
    block_on(EncryptedEnvelope::seal(
        &VectorWrapping,
        SealRequest::new(
            reference(),
            context(),
            &plaintext,
            SealMaterial::new(data_key, Nonce::new([0x22; 24])),
        ),
    ))
    .expect("seal")
}

#[test]
fn deterministic_envelope_vector_round_trips() {
    let envelope = seal(b"radroots envelope vector");
    assert_eq!(envelope.version(), ENVELOPE_VERSION);
    assert_eq!(envelope.cipher(), Cipher::XChaCha20Poly1305);
    assert_eq!(envelope.key_source(), KeySource::ProviderWrapped);
    assert_eq!(envelope.reference().id().as_str(), "envelope-key");

    let encoded = envelope.encode().expect("encode");
    assert_eq!(
        hex::encode(&encoded),
        "52525331000201010100000007000c656e76656c6f70652d6b65790001726164726f6f74732e656e76656c6f70655f636f6e746578742e76310019726164726f6f74732e707269766174655f61727469666163740010707269766174655f617274696661637400203031303130313031303130313031303130313031303130313031303130313031001674726164652e707269766174655f7465726d732e7631222222222222222222222222222222222222222222222222000000204b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b00000028f106837e33d690e7c5287abdd815ce9257b7b5b176ea9596f762615d1c221c5c10964c4f799d5d59"
    );
    let decoded = EncryptedEnvelope::decode(&encoded).expect("decode");
    let opened = block_on(decoded.open(&VectorWrapping, &context())).expect("open");
    opened.expose_secret(|bytes| assert_eq!(bytes, b"radroots envelope vector"));
}

#[test]
fn tamper_nonce_and_ciphertext_are_rejected() {
    let encoded = seal(b"authenticated plaintext").encode().expect("encode");

    let mut nonce_tamper = encoded.clone();
    let nonce_offset =
        4 + 2 + 1 + 1 + 1 + 4 + 2 + "envelope-key".len() + context().to_canonical_bytes().len();
    nonce_tamper[nonce_offset] ^= 0x01;
    let envelope = EncryptedEnvelope::decode(&nonce_tamper).expect("decode nonce tamper");
    assert!(matches!(
        block_on(envelope.open(&VectorWrapping, &context())),
        Err(Error::DecryptFailed)
    ));

    let mut ciphertext_tamper = encoded;
    let last = ciphertext_tamper.last_mut().expect("ciphertext byte");
    *last ^= 0x01;
    let envelope = EncryptedEnvelope::decode(&ciphertext_tamper).expect("decode ciphertext tamper");
    assert!(matches!(
        block_on(envelope.open(&VectorWrapping, &context())),
        Err(Error::DecryptFailed)
    ));
}

#[test]
fn context_mismatch_and_authenticated_context_tamper_fail_closed() {
    let encoded = seal(b"context-bound plaintext").encode().expect("encode");
    let different = EnvelopeContext::new(
        EnvelopePurpose::parse("radroots.different_artifact").expect("purpose"),
        EnvelopeSubject::parse("private_artifact", "01010101010101010101010101010101")
            .expect("subject"),
        PayloadSchemaId::parse("trade.private_terms.v1").expect("schema"),
    );
    let envelope = EncryptedEnvelope::decode(&encoded).expect("decode");
    assert_eq!(
        block_on(envelope.open(&VectorWrapping, &different)).err(),
        Some(Error::ContextMismatch)
    );

    let purpose = b"radroots.private_artifact";
    let purpose_offset = encoded
        .windows(purpose.len())
        .position(|window| window == purpose)
        .expect("purpose offset");
    let mut tampered = encoded;
    tampered[purpose_offset] = b's';
    let envelope = EncryptedEnvelope::decode(&tampered).expect("valid altered context");
    let altered = envelope.context().expect("v2 context").clone();
    assert_eq!(
        block_on(envelope.open(&VectorWrapping, &altered)).err(),
        Some(Error::DecryptFailed)
    );
}

#[test]
fn normal_open_denies_the_frozen_v1_corpus() {
    let legacy = hex::decode("52525331000101010100000007000c656e76656c6f70652d6b6579222222222222222222222222222222222222222222222222000000204b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b00000028f106837e33d690e7c5287abdd815ce9257b7b5b176ea9596abf3b7fe745aec5a8c2487a553d4659d").expect("legacy hex");
    let envelope = EncryptedEnvelope::decode(&legacy).expect("legacy decode");
    assert_eq!(envelope.version(), 1);
    assert!(envelope.context().is_none());
    assert_eq!(
        block_on(envelope.open(&VectorWrapping, &context())).err(),
        Some(Error::LegacyEnvelopeDenied)
    );
}

#[test]
fn wrong_key_slot_and_invalid_version_fail_closed() {
    let encoded = seal(b"slot-bound plaintext").encode().expect("encode");
    let id_offset = 4 + 2 + 1 + 1 + 1 + 4 + 2;
    let mut wrong_slot = encoded.clone();
    wrong_slot[id_offset] = b'x';
    let envelope = EncryptedEnvelope::decode(&wrong_slot).expect("decode slot tamper");
    assert!(matches!(
        block_on(envelope.open(&VectorWrapping, &context())),
        Err(Error::BackendFailure {
            backend: BackendKind::Memory,
            operation: Operation::Unwrap,
        })
    ));

    let mut bad_version = encoded;
    bad_version[5] = 3;
    assert!(matches!(
        EncryptedEnvelope::decode(&bad_version),
        Err(Error::UnsupportedEnvelopeVersion { version: 3 })
    ));
}

#[test]
fn malformed_lengths_and_wrong_data_key_lengths_are_rejected() {
    assert!(matches!(
        EncryptedEnvelope::decode(b"short"),
        Err(Error::EnvelopeMalformed)
    ));

    let plaintext = SecretMaterial::from_slice(b"payload").expect("plaintext");
    let short_key = SecretMaterial::from_slice(&[0x11; 31]).expect("short key");
    assert!(matches!(
        block_on(EncryptedEnvelope::seal(
            &VectorWrapping,
            SealRequest::new(
                reference(),
                context(),
                &plaintext,
                SealMaterial::new(short_key, Nonce::new([0x22; 24])),
            ),
        )),
        Err(Error::InvalidDataKeyLength { actual_bytes: 31 })
    ));
}

#[cfg(feature = "serde")]
#[test]
fn serde_uses_the_validated_binary_envelope() {
    let envelope = seal(b"serde envelope");
    let json = serde_json::to_vec(&envelope).expect("serialize envelope");
    let decoded: EncryptedEnvelope = serde_json::from_slice(&json).expect("deserialize envelope");
    let opened = block_on(decoded.open(&VectorWrapping, &context())).expect("open");
    opened.expose_secret(|bytes| assert_eq!(bytes, b"serde envelope"));
}

#[test]
fn envelope_diagnostics_are_redacted() {
    let diagnostic = format!("{:?}", seal(b"must-not-appear"));
    assert!(diagnostic.contains("<redacted>"));
    assert!(!diagnostic.contains("must-not-appear"));
    assert!(!diagnostic.contains("envelope-key"));
}
