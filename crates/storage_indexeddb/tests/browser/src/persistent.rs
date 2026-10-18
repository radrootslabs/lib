use futures::FutureExt;
use radroots_secrets::{
    EncryptedEnvelope, KeyWrapping, SecretId, SecretProvider, SecretRef,
    browser::BrowserProvider,
    context::{EnvelopeContext, EnvelopePurpose, EnvelopeSubject, PayloadSchemaId},
    envelope::{Nonce, SealMaterial, SealRequest},
    id::{BackendKind, KeyVersion},
    provider::{
        AccessPolicy, CapabilitySupport, HardwarePolicy, ResidencyPolicy, ResidencySupport,
        SelectionPolicy, UserPresencePolicy,
    },
    wrapping::{SecretMaterial, UnwrapRequest, WrapRequest},
};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;

fn reference() -> SecretRef {
    SecretRef::new(
        SecretId::parse("persistent_fixture_key").unwrap(),
        BackendKind::External,
        KeyVersion::new(1).unwrap(),
    )
}
fn namespace(n: u8) -> SecretId {
    SecretId::parse(format!("persistent_fixture_{n}")).unwrap()
}
fn context(n: u8) -> EnvelopeContext {
    EnvelopeContext::new(
        EnvelopePurpose::parse("radroots.persistent_fixture").unwrap(),
        EnvelopeSubject::parse("fixture", n.to_string()).unwrap(),
        PayloadSchemaId::parse("radroots.persistent_fixture.v1").unwrap(),
    )
}
fn fail(_: radroots_secrets::Error) -> JsValue {
    JsValue::from_str("persistent_crypto_refused")
}
fn rng() -> web_sys::Crypto {
    js_sys::Reflect::get(&js_sys::global(), &JsValue::from_str("crypto"))
        .unwrap()
        .dyn_into()
        .unwrap()
}

#[wasm_bindgen]
pub struct Persistent {
    provider: BrowserProvider,
    n: u8,
}
#[wasm_bindgen]
impl Persistent {
    pub fn abandon_setup_batch(first: u8) -> Result<(), JsValue> {
        for n in first..first + 32 {
            Self::abandon_create(n)?;
        }
        if !matches!(
            BrowserProvider::create(namespace(first + 32), reference()).now_or_never(),
            Some(Err(radroots_secrets::browser::BrowserError::Capacity))
        ) {
            return Err(JsValue::from_str("setup_admission_was_not_bounded"));
        }
        if !matches!(
            BrowserProvider::open(namespace(first), reference(), [0; 32]).now_or_never(),
            Some(Err(radroots_secrets::browser::BrowserError::Capacity))
        ) {
            return Err(JsValue::from_str("setup_open_admission_was_not_bounded"));
        }
        Ok(())
    }
    pub fn abandon_create(n: u8) -> Result<(), JsValue> {
        if BrowserProvider::create(namespace(n), reference())
            .now_or_never()
            .is_some()
        {
            return Err(JsValue::from_str("unexpected_immediate_create"));
        }
        Ok(())
    }
    pub async fn create(n: u8) -> Result<Persistent, JsValue> {
        Ok(Self {
            provider: BrowserProvider::create(namespace(n), reference())
                .await
                .map_err(|e| JsValue::from_str(e.code()))?,
            n,
        })
    }
    pub async fn open(n: u8, fingerprint: Vec<u8>) -> Result<Persistent, JsValue> {
        let fingerprint = fingerprint
            .as_slice()
            .try_into()
            .map_err(|_| JsValue::from_str("invalid_fingerprint"))?;
        Ok(Self {
            provider: BrowserProvider::open(namespace(n), reference(), fingerprint)
                .await
                .map_err(|e| JsValue::from_str(e.code()))?,
            n,
        })
    }
    pub fn fingerprint(&self) -> Vec<u8> {
        self.provider.fingerprint().to_vec()
    }
    pub fn truthful_capabilities(&self) -> bool {
        let capabilities = self.provider.capabilities();
        assert_eq!(capabilities.residency(), ResidencySupport::UserProfile);
        assert_eq!(
            capabilities.hardware_backed(),
            CapabilitySupport::Unavailable
        );
        assert_eq!(capabilities.user_presence(), CapabilitySupport::Unavailable);
        let candidates: [&dyn SecretProvider; 1] = [&self.provider];
        assert!(
            SelectionPolicy::new(BackendKind::External, AccessPolicy::standard())
                .select(&candidates)
                .is_ok()
        );
        for policy in [
            AccessPolicy::new(
                ResidencyPolicy::DeviceLocal,
                UserPresencePolicy::NotRequired,
                HardwarePolicy::Any,
            ),
            AccessPolicy::new(
                ResidencyPolicy::Any,
                UserPresencePolicy::Required,
                HardwarePolicy::Any,
            ),
            AccessPolicy::new(
                ResidencyPolicy::Any,
                UserPresencePolicy::NotRequired,
                HardwarePolicy::RequireHardwareBacked,
            ),
        ] {
            assert!(
                SelectionPolicy::new(BackendKind::External, policy)
                    .select(&candidates)
                    .is_err()
            );
        }
        true
    }
    pub async fn protect(&self, bytes: Vec<u8>) -> Result<Vec<u8>, JsValue> {
        let mut key = [0; 32];
        let mut nonce = [0; 24];
        rng().get_random_values_with_u8_array(&mut key).unwrap();
        rng().get_random_values_with_u8_array(&mut nonce).unwrap();
        let material = SecretMaterial::from_slice(&key).map_err(fail)?;
        key.fill(0);
        let plaintext = SecretMaterial::from_slice(&bytes).map_err(fail)?;
        EncryptedEnvelope::seal(
            &self.provider,
            SealRequest::new(
                reference(),
                context(self.n),
                &plaintext,
                SealMaterial::new(material, Nonce::new(nonce)),
            ),
        )
        .await
        .map_err(fail)?
        .encode()
        .map_err(fail)
    }
    pub async fn reveal(&self, encoded: Vec<u8>) -> Result<Vec<u8>, JsValue> {
        let envelope = EncryptedEnvelope::decode(&encoded).map_err(fail)?;
        envelope
            .open(&self.provider, &context(self.n))
            .await
            .map_err(fail)
            .map(|plain| plain.expose_secret(|bytes| bytes.to_vec()))
    }
    pub async fn context_and_reference_refusal(&self) -> Result<bool, JsValue> {
        let reference = reference();
        let expected = context(self.n);
        let wrong = context(self.n.wrapping_add(1));
        let plaintext = SecretMaterial::from_slice(&[0x31; 32]).map_err(fail)?;
        let wrapped = self
            .provider
            .wrap(WrapRequest::new(&reference, &expected, &plaintext))
            .await
            .map_err(fail)?;
        assert!(
            self.provider
                .unwrap(UnwrapRequest::new(&reference, &wrong, &wrapped))
                .await
                .is_err()
        );
        let foreign = SecretRef::new(
            SecretId::parse("foreign_key").unwrap(),
            BackendKind::External,
            KeyVersion::new(1).unwrap(),
        );
        assert!(
            self.provider
                .wrap(WrapRequest::new(&foreign, &expected, &plaintext))
                .await
                .is_err()
        );
        let version = SecretRef::new(
            SecretId::parse("persistent_fixture_key").unwrap(),
            BackendKind::External,
            KeyVersion::new(2).unwrap(),
        );
        assert!(
            self.provider
                .unwrap(UnwrapRequest::new(&version, &expected, &wrapped))
                .await
                .is_err()
        );
        let memory = SecretRef::new(
            SecretId::parse("persistent_fixture_key").unwrap(),
            BackendKind::Memory,
            KeyVersion::new(1).unwrap(),
        );
        assert!(
            self.provider
                .wrap(WrapRequest::new(&memory, &expected, &plaintext))
                .await
                .is_err()
        );
        let short = SecretMaterial::from_slice(&[1; 31]).map_err(fail)?;
        assert!(
            self.provider
                .wrap(WrapRequest::new(&reference, &expected, &short))
                .await
                .is_err()
        );
        Ok(true)
    }
    pub fn abandon_wraps(&self, count: usize) -> Result<usize, JsValue> {
        let reference = reference();
        let context = context(self.n);
        let plaintext = SecretMaterial::from_slice(&[1; 32]).map_err(fail)?;
        for index in 0..count {
            if let Some(result) = self
                .provider
                .wrap(WrapRequest::new(&reference, &context, &plaintext))
                .now_or_never()
            {
                return if result.is_err() {
                    Ok(index)
                } else {
                    Err(JsValue::from_str("unexpected_immediate_wrap"))
                };
            }
        }
        Ok(count)
    }
    pub fn abandon_close(&self) -> Result<(), JsValue> {
        if self.provider.close().now_or_never().is_some() {
            return Err(JsValue::from_str("unexpected_immediate_close"));
        }
        Ok(())
    }
    pub async fn close(&self) -> Result<(), JsValue> {
        self.provider
            .close()
            .await
            .map_err(|e| JsValue::from_str(e.code()))
    }
}
