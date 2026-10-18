use super::BrowserError;
use crate::{
    SecretId, SecretRef,
    context::EnvelopeContext,
    wrapping::{SecretMaterial, WrappedSecret},
};
use js_sys::{Array, ArrayBuffer, Reflect, Uint8Array};
use sha2::{Digest, Sha256};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{AesKeyGenParams, Crypto, CryptoKey, HkdfParams, SubtleCrypto};

const HEADER: &[u8; 5] = b"RRBW\x01";
const ENCODED_BYTES: usize = 45;

struct PlainBytes(Uint8Array);
impl Drop for PlainBytes {
    fn drop(&mut self) {
        self.0.fill(0, 0, self.0.length());
    }
}
fn usages(names: &[&str]) -> Array {
    names.iter().map(|name| JsValue::from_str(name)).collect()
}
pub(super) fn host() -> Result<(Crypto, SubtleCrypto), BrowserError> {
    let crypto = Reflect::get(&js_sys::global(), &JsValue::from_str("crypto"))
        .map_err(|_| BrowserError::Unavailable)?
        .dyn_into::<Crypto>()
        .map_err(|_| BrowserError::Unavailable)?;
    let subtle = Reflect::get(crypto.as_ref(), &JsValue::from_str("subtle"))
        .map_err(|_| BrowserError::Unavailable)?
        .dyn_into::<SubtleCrypto>()
        .map_err(|_| BrowserError::Unavailable)?;
    Ok((crypto, subtle))
}
pub(super) async fn generate(
    crypto: &Crypto,
    subtle: &SubtleCrypto,
) -> Result<CryptoKey, BrowserError> {
    let seed = PlainBytes(Uint8Array::new_with_length(32));
    crypto
        .get_random_values_with_js_u8_array(&seed.0)
        .map_err(|_| BrowserError::Unavailable)?;
    let promise = subtle
        .import_key_with_str("raw", &seed.0, "HKDF", false, &usages(&["deriveKey"]))
        .map_err(|_| BrowserError::Unavailable)?;
    let key = JsFuture::from(promise)
        .await
        .map_err(|_| BrowserError::Unavailable)?
        .dyn_into::<CryptoKey>()
        .map_err(|_| BrowserError::KeyMismatch)?;
    validate(&key)?;
    Ok(key)
}
pub(super) fn validate(key: &CryptoKey) -> Result<(), BrowserError> {
    let algorithm = key.algorithm().map_err(|_| BrowserError::KeyMismatch)?;
    if key.type_() != "secret"
        || key.extractable()
        || key.usages().length() != 1
        || key.usages().get(0).as_string().as_deref() != Some("deriveKey")
        || Reflect::get(&algorithm, &JsValue::from_str("name"))
            .ok()
            .and_then(|v| v.as_string())
            .as_deref()
            != Some("HKDF")
    {
        return Err(BrowserError::KeyMismatch);
    }
    Ok(())
}
fn scope(namespace: &SecretId, reference: &SecretRef) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"radroots.browser.key_scope.v1\0");
    for value in [namespace.as_str(), reference.id().as_str()] {
        hash.update((value.len() as u16).to_be_bytes());
        hash.update(value.as_bytes());
    }
    hash.update(reference.key_version().get().to_be_bytes());
    hash.finalize().into()
}
async fn derive(
    subtle: &SubtleCrypto,
    key: &CryptoKey,
    namespace: &SecretId,
    reference: &SecretRef,
    context: Option<&EnvelopeContext>,
) -> Result<CryptoKey, BrowserError> {
    validate(key)?;
    let salt = Uint8Array::from(scope(namespace, reference).as_slice());
    let mut info = Vec::from(if context.is_some() {
        b"radroots.browser.key_wrap.v1\0".as_slice()
    } else {
        b"radroots.browser.key_probe.v1\0".as_slice()
    });
    if let Some(context) = context {
        info.extend_from_slice(&context.authentication_digest());
    }
    let params =
        HkdfParams::new_with_str("HKDF", "SHA-256", &Uint8Array::from(info.as_slice()), &salt);
    let algorithm = AesKeyGenParams::new("AES-KW", 256);
    let promise = subtle
        .derive_key_with_object_and_object(
            &params,
            key,
            &algorithm,
            false,
            &usages(&["wrapKey", "unwrapKey"]),
        )
        .map_err(|_| BrowserError::Unavailable)?;
    JsFuture::from(promise)
        .await
        .map_err(|_| BrowserError::Unavailable)?
        .dyn_into::<CryptoKey>()
        .map_err(|_| BrowserError::KeyMismatch)
}
async fn encode(
    subtle: &SubtleCrypto,
    wrapping: &CryptoKey,
    bytes: &[u8],
) -> Result<Vec<u8>, BrowserError> {
    if bytes.len() != 32 {
        return Err(BrowserError::Provider(crate::Error::InvalidDataKeyLength {
            actual_bytes: bytes.len(),
        }));
    }
    let raw = PlainBytes(Uint8Array::from(bytes));
    let promise = subtle
        .import_key_with_str("raw", &raw.0, "AES-GCM", true, &usages(&["encrypt"]))
        .map_err(|_| BrowserError::Unavailable)?;
    let data_key = JsFuture::from(promise)
        .await
        .map_err(|_| BrowserError::Unavailable)?
        .dyn_into::<CryptoKey>()
        .map_err(|_| BrowserError::KeyMismatch)?;
    let promise = subtle
        .wrap_key_with_str("raw", &data_key, wrapping, "AES-KW")
        .map_err(|_| BrowserError::Unavailable)?;
    let buffer = JsFuture::from(promise)
        .await
        .map_err(|_| BrowserError::Unavailable)?
        .dyn_into::<ArrayBuffer>()
        .map_err(|_| BrowserError::KeyMismatch)?;
    if buffer.byte_length() != 40 {
        return Err(BrowserError::KeyMismatch);
    }
    Ok(Uint8Array::new(&buffer).to_vec())
}
pub(super) async fn fingerprint(
    subtle: &SubtleCrypto,
    key: &CryptoKey,
    namespace: &SecretId,
    reference: &SecretRef,
) -> Result<[u8; 32], BrowserError> {
    let derived = derive(subtle, key, namespace, reference, None).await?;
    // Public fixed probe in its own KDF domain; no payload or provider key is exposed.
    Ok(Sha256::digest(encode(subtle, &derived, &[0xa5; 32]).await?).into())
}
pub(super) async fn wrap(
    subtle: &SubtleCrypto,
    key: &CryptoKey,
    namespace: &SecretId,
    reference: &SecretRef,
    context: &EnvelopeContext,
    plaintext: &SecretMaterial,
) -> Result<WrappedSecret, BrowserError> {
    let derived = derive(subtle, key, namespace, reference, Some(context)).await?;
    let raw = plaintext.expose_secret(|bytes| zeroize::Zeroizing::new(bytes.to_vec()));
    let mut encoded = Vec::from(HEADER.as_slice());
    encoded.extend_from_slice(&encode(subtle, &derived, raw.as_slice()).await?);
    WrappedSecret::from_bytes(encoded).map_err(BrowserError::Provider)
}
pub(super) fn validate_wrapped(wrapped: &WrappedSecret) -> Result<(), BrowserError> {
    if wrapped.as_bytes().len() != ENCODED_BYTES || !wrapped.as_bytes().starts_with(HEADER) {
        return Err(BrowserError::KeyMismatch);
    }
    Ok(())
}
pub(super) async fn unwrap(
    subtle: &SubtleCrypto,
    key: &CryptoKey,
    namespace: &SecretId,
    reference: &SecretRef,
    context: &EnvelopeContext,
    wrapped: &WrappedSecret,
) -> Result<SecretMaterial, BrowserError> {
    validate_wrapped(wrapped)?;
    let derived = derive(subtle, key, namespace, reference, Some(context)).await?;
    let ciphertext = Uint8Array::from(&wrapped.as_bytes()[HEADER.len()..]);
    let promise = subtle
        .unwrap_key_with_buffer_source_and_str_and_str(
            "raw",
            &ciphertext,
            &derived,
            "AES-KW",
            "AES-GCM",
            true,
            &usages(&["encrypt"]),
        )
        .map_err(|_| BrowserError::KeyMismatch)?;
    let data_key = JsFuture::from(promise)
        .await
        .map_err(|_| BrowserError::KeyMismatch)?
        .dyn_into::<CryptoKey>()
        .map_err(|_| BrowserError::KeyMismatch)?;
    let promise = subtle
        .export_key("raw", &data_key)
        .map_err(|_| BrowserError::KeyMismatch)?;
    let buffer = JsFuture::from(promise)
        .await
        .map_err(|_| BrowserError::KeyMismatch)?
        .dyn_into::<ArrayBuffer>()
        .map_err(|_| BrowserError::KeyMismatch)?;
    let plaintext = PlainBytes(Uint8Array::new(&buffer));
    if plaintext.0.length() != 32 {
        return Err(BrowserError::KeyMismatch);
    }
    SecretMaterial::from_owned(plaintext.0.to_vec()).map_err(BrowserError::Provider)
}
