//! Volatile fixture provider. No persistent custody or hardware claim.
use radroots_secrets::context::{
    EnvelopeContext, EnvelopePurpose, EnvelopeSubject, PayloadSchemaId,
};
use radroots_secrets::envelope::{LegacyV1ResealAuthority, Nonce, SealMaterial, SealRequest};
use radroots_secrets::id::{BackendKind, KeyVersion};
use radroots_secrets::memory::MemoryProvider;
use radroots_secrets::provider::SecretCapabilities;
use radroots_secrets::wrapping::{
    BoxFuture, LegacyV1UnwrapRequest, SecretMaterial, UnwrapRequest, WrapRequest, WrappedSecret,
};
use radroots_secrets::{
    EncryptedEnvelope, Error, KeyWrapping, SecretId, SecretProvider, SecretRef,
};
use radroots_storage::event::SourceGeneration;
use radroots_storage_indexeddb::{Batch, Database, Mutation, Namespace, OpenMode, RecordKey};
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

#[derive(Default)]
struct Calls {
    wraps: usize,
    unwraps: usize,
    legacy: usize,
}
struct LocalProvider {
    inner: MemoryProvider,
    calls: Rc<RefCell<Calls>>,
}
async fn pause() {
    JsFuture::from(js_sys::Promise::resolve(&JsValue::UNDEFINED))
        .await
        .expect("fixture microtask");
}
impl KeyWrapping for LocalProvider {
    fn wrap<'a>(&'a self, request: WrapRequest<'a>) -> BoxFuture<'a, Result<WrappedSecret, Error>> {
        let calls = self.calls.clone();
        Box::pin(async move {
            pause().await;
            calls.borrow_mut().wraps += 1;
            self.inner.wrap(request).await
        })
    }
    fn unwrap<'a>(
        &'a self,
        request: UnwrapRequest<'a>,
    ) -> BoxFuture<'a, Result<SecretMaterial, Error>> {
        let calls = self.calls.clone();
        Box::pin(async move {
            pause().await;
            calls.borrow_mut().unwraps += 1;
            self.inner.unwrap(request).await
        })
    }
    fn unwrap_legacy_v1<'a>(
        &'a self,
        request: LegacyV1UnwrapRequest<'a>,
    ) -> BoxFuture<'a, Result<SecretMaterial, Error>> {
        let calls = self.calls.clone();
        Box::pin(async move {
            pause().await;
            calls.borrow_mut().legacy += 1;
            // Exact frozen envelope_contract.rs vector, never a runtime key.
            if request.reference().id().as_str() != "envelope-key"
                || request.reference().backend() != BackendKind::Memory
                || request.reference().key_version().get() != 7
                || request.wrapped().as_bytes() != [0x4b; 32]
            {
                return Err(Error::LegacyEnvelopeDenied);
            }
            SecretMaterial::from_slice(&[0x11; 32])
        })
    }
}
impl SecretProvider for LocalProvider {
    fn backend_kind(&self) -> BackendKind {
        self.inner.backend_kind()
    }
    fn capabilities(&self) -> SecretCapabilities {
        self.inner.capabilities()
    }
}
fn reference() -> SecretRef {
    SecretRef::new(
        SecretId::parse("local-fixture-key").unwrap(),
        BackendKind::Memory,
        KeyVersion::new(1).unwrap(),
    )
}
fn context(subject: &str) -> EnvelopeContext {
    EnvelopeContext::new(
        EnvelopePurpose::parse("radroots.browser_fixture").unwrap(),
        EnvelopeSubject::parse("fixture", subject).unwrap(),
        PayloadSchemaId::parse("radroots.browser_fixture.v1").unwrap(),
    )
}
fn secret_error(_: Error) -> JsValue {
    JsValue::from_str("protected_fixture_failure")
}

#[wasm_bindgen]
pub async fn protected_round_trip(n: u8) -> Result<(), JsValue> {
    let calls = Rc::new(RefCell::new(Calls::default()));
    let provider = LocalProvider {
        inner: MemoryProvider::new(),
        calls: calls.clone(),
    };
    provider
        .inner
        .provision(
            &reference(),
            SecretMaterial::from_slice(&[0x41; 32]).map_err(secret_error)?,
        )
        .map_err(secret_error)?;
    let plaintext =
        SecretMaterial::from_slice(b"browser protected fixture").map_err(secret_error)?;
    let expected = context(&n.to_string());
    let encoded = EncryptedEnvelope::seal(
        &provider,
        SealRequest::new(
            reference(),
            expected.clone(),
            &plaintext,
            SealMaterial::new(
                SecretMaterial::from_slice(&[0x41; 32]).map_err(secret_error)?,
                Nonce::new([0x24; 24]),
            ),
        ),
    )
    .await
    .map_err(secret_error)?
    .encode()
    .map_err(secret_error)?;
    assert!(
        !encoded
            .windows(plaintext.len())
            .any(|value| plaintext.expose_secret(|plain| plain == value))
    );
    let namespace = Namespace::new(SourceGeneration::new([n; 32]).unwrap());
    let record = RecordKey::new([1; 32]);
    let db = Database::open(namespace, OpenMode::CreateNew)
        .await
        .map_err(super::failure)?;
    db.compare_exchange(
        Batch::new(vec![
            Mutation::new(record, None, Some(encoded)).map_err(super::failure)?,
        ])
        .map_err(super::failure)?,
    )
    .await
    .map_err(super::failure)?;
    db.close().await.map_err(super::failure)?;
    let reopened = Database::open(namespace, OpenMode::Existing)
        .await
        .map_err(super::failure)?;
    let mut stored = reopened
        .get(record)
        .await
        .map_err(super::failure)?
        .ok_or_else(|| JsValue::from_str("missing_protected_fixture"))?;
    let envelope = EncryptedEnvelope::decode(&stored).map_err(secret_error)?;
    let opened = envelope
        .open(&provider, &expected)
        .await
        .map_err(secret_error)?;
    opened.expose_secret(|bytes| assert_eq!(bytes, b"browser protected fixture"));
    assert_eq!(
        envelope.open(&provider, &context("foreign")).await.err(),
        Some(Error::ContextMismatch)
    );
    *stored.last_mut().unwrap() ^= 1;
    let tampered = EncryptedEnvelope::decode(&stored).map_err(secret_error)?;
    assert_eq!(
        tampered.open(&provider, &expected).await.err(),
        Some(Error::DecryptFailed)
    );
    reopened.close().await.map_err(super::failure)?;

    // Existing independently frozen v1 ciphertext and its public fixture key.
    let legacy=hex::decode("52525331000101010100000007000c656e76656c6f70652d6b6579222222222222222222222222222222222222222222222222000000204b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b00000028f106837e33d690e7c5287abdd815ce9257b7b5b176ea9596abf3b7fe745aec5a8c2487a553d4659d").unwrap();
    let legacy = EncryptedEnvelope::decode(&legacy).map_err(secret_error)?;
    assert_eq!(
        legacy.open(&provider, &expected).await.err(),
        Some(Error::LegacyEnvelopeDenied)
    );
    let authority = LegacyV1ResealAuthority::new();
    let original = SecretRef::new(
        SecretId::parse("envelope-key").unwrap(),
        BackendKind::Memory,
        KeyVersion::new(7).unwrap(),
    );
    let validations = Rc::new(RefCell::new(0));
    let validator = |bytes: &[u8]| {
        *validations.borrow_mut() += 1;
        bytes == b"radroots envelope vector"
    };
    let resealed = legacy
        .reseal_legacy_v1(
            &provider,
            &authority,
            &original,
            reference(),
            expected.clone(),
            &validator,
            SealMaterial::new(
                SecretMaterial::from_slice(&[0x41; 32]).map_err(secret_error)?,
                Nonce::new([0x55; 24]),
            ),
        )
        .await
        .map_err(secret_error)?
        .into_envelope();
    assert_eq!(*validations.borrow(), 1);
    resealed
        .open(&provider, &expected)
        .await
        .map_err(secret_error)?
        .expose_secret(|bytes| assert_eq!(bytes, b"radroots envelope vector"));
    let calls = calls.borrow();
    assert_eq!((calls.wraps, calls.unwraps, calls.legacy), (2, 3, 1));
    Ok(())
}
