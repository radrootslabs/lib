//! Explicit operating-system keyring adapters.

use crate::SecretRef;
use crate::error::{Error, Operation};
use crate::id::BackendKind;
use crate::provider::{CapabilitySupport, ResidencySupport, SecretCapabilities, SecretProvider};
use crate::wrapping::{
    BoxFuture, KeyWrapping, SecretMaterial, UnwrapRequest, WrapRequest, WrappedSecret,
};
use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;
use std::sync::Mutex;
use zeroize::Zeroizing;

const SERVICE_NAME_MAX_BYTES: usize = 128;
const VALUE_PREFIX: &str = "radroots-keyring-v1:";
const TOKEN_PREFIX: &[u8] = b"radroots-keyring-key-v1\0";

/// Explicit OS keyring provider with lazy credential access.
pub struct KeyringProvider {
    service_name: String,
    store: Box<dyn CredentialStore>,
    operation_lock: Mutex<()>,
}

impl KeyringProvider {
    /// Creates a provider without reading or writing the operating-system keyring.
    pub fn new(service_name: impl AsRef<str>) -> Result<Self, Error> {
        let service_name = validate_service_name(service_name.as_ref())?;
        Ok(Self {
            service_name,
            store: Box::new(OsCredentialStore),
            operation_lock: Mutex::new(()),
        })
    }

    /// Stores caller-owned material at one exact key version.
    pub fn provision(&self, reference: &SecretRef, material: &SecretMaterial) -> Result<(), Error> {
        validate_keyring_reference(reference)?;
        let _guard = self
            .operation_lock
            .lock()
            .map_err(|_| backend_failure(Operation::Provision))?;
        let account = account_name(reference);
        match self.store.get(&self.service_name, account.as_str()) {
            Ok(_) => {
                return Err(Error::SecretAlreadyExists {
                    backend: BackendKind::Keyring,
                    key_version: reference.key_version().get(),
                });
            }
            Err(StoreError::Missing) => {}
            Err(StoreError::Failure) => return Err(backend_failure(Operation::Provision)),
        }
        let encoded = encode_material(material);
        self.store
            .set(&self.service_name, account.as_str(), encoded.as_str())
            .map_err(|_| backend_failure(Operation::Provision))
    }

    /// Resumes safely when the new version exists before old-version removal.
    pub fn rotate(
        &self,
        current: &SecretRef,
        next: &SecretRef,
        material: &SecretMaterial,
    ) -> Result<(), Error> {
        validate_keyring_reference(current)?;
        validate_keyring_reference(next)?;
        if current.id().as_str() != next.id().as_str()
            || next.key_version() <= current.key_version()
        {
            return Err(Error::InvalidRotation);
        }
        self.read_material(current)?;
        match self.read_material(next) {
            Ok(persisted) => {
                let matches = persisted
                    .expose_secret(|expected| material.expose_secret(|actual| expected == actual));
                if !matches {
                    return Err(Error::SecretAlreadyExists {
                        backend: BackendKind::Keyring,
                        key_version: next.key_version().get(),
                    });
                }
            }
            Err(Error::SecretNotFound { .. }) => self.provision(next, material)?,
            Err(error) => return Err(error),
        }
        self.remove(current)?;
        Ok(())
    }

    /// Deletes one exact key revision when present.
    pub fn remove(&self, reference: &SecretRef) -> Result<bool, Error> {
        validate_keyring_reference(reference)?;
        let _guard = self
            .operation_lock
            .lock()
            .map_err(|_| backend_failure(Operation::Remove))?;
        let account = account_name(reference);
        match self.store.delete(&self.service_name, account.as_str()) {
            Ok(()) => Ok(true),
            Err(StoreError::Missing) => Ok(false),
            Err(StoreError::Failure) => Err(backend_failure(Operation::Remove)),
        }
    }

    fn read_material(&self, reference: &SecretRef) -> Result<SecretMaterial, Error> {
        let account = account_name(reference);
        let encoded = match self.store.get(&self.service_name, account.as_str()) {
            Ok(value) => Zeroizing::new(value),
            Err(StoreError::Missing) => return Err(not_found(reference)),
            Err(StoreError::Failure) => return Err(backend_failure(Operation::Read)),
        };
        decode_material(encoded.as_str())
    }

    #[cfg(test)]
    fn with_store(service_name: &str, store: Box<dyn CredentialStore>) -> Result<Self, Error> {
        Ok(Self {
            service_name: validate_service_name(service_name)?,
            store,
            operation_lock: Mutex::new(()),
        })
    }
}

impl fmt::Debug for KeyringProvider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("KeyringProvider(<redacted>)")
    }
}

impl KeyWrapping for KeyringProvider {
    fn wrap<'a>(&'a self, request: WrapRequest<'a>) -> BoxFuture<'a, Result<WrappedSecret, Error>> {
        Box::pin(async move {
            validate_keyring_reference(request.reference())?;
            let persisted = self.read_material(request.reference())?;
            let matches = persisted.expose_secret(|expected| {
                request
                    .plaintext()
                    .expose_secret(|actual| expected == actual)
            });
            if !matches {
                return Err(backend_failure(Operation::Wrap));
            }
            WrappedSecret::from_bytes(wrapping_token(request.reference(), request.context()))
        })
    }

    fn unwrap<'a>(
        &'a self,
        request: UnwrapRequest<'a>,
    ) -> BoxFuture<'a, Result<SecretMaterial, Error>> {
        Box::pin(async move {
            validate_keyring_reference(request.reference())?;
            if request.wrapped().as_bytes()
                != wrapping_token(request.reference(), request.context()).as_slice()
            {
                return Err(backend_failure(Operation::Unwrap));
            }
            self.read_material(request.reference())
        })
    }
}

fn wrapping_token(reference: &SecretRef, context: &crate::context::EnvelopeContext) -> Vec<u8> {
    let mut token = reference_token(reference);
    token.extend_from_slice(&context.authentication_digest());
    token
}

impl SecretProvider for KeyringProvider {
    fn backend_kind(&self) -> BackendKind {
        BackendKind::Keyring
    }

    fn capabilities(&self) -> SecretCapabilities {
        SecretCapabilities::available(
            ResidencySupport::UserProfile,
            CapabilitySupport::Unavailable,
            CapabilitySupport::Unavailable,
        )
    }
}

trait CredentialStore: Send + Sync {
    fn set(&self, service: &str, account: &str, value: &str) -> Result<(), StoreError>;
    fn get(&self, service: &str, account: &str) -> Result<String, StoreError>;
    fn delete(&self, service: &str, account: &str) -> Result<(), StoreError>;
}

struct OsCredentialStore;

impl CredentialStore for OsCredentialStore {
    fn set(&self, service: &str, account: &str, value: &str) -> Result<(), StoreError> {
        let entry = keyring::Entry::new(service, account).map_err(|_| StoreError::Failure)?;
        entry.set_password(value).map_err(|_| StoreError::Failure)
    }

    fn get(&self, service: &str, account: &str) -> Result<String, StoreError> {
        let entry = keyring::Entry::new(service, account).map_err(|_| StoreError::Failure)?;
        match entry.get_password() {
            Ok(value) => Ok(value),
            Err(keyring::Error::NoEntry) => Err(StoreError::Missing),
            Err(_) => Err(StoreError::Failure),
        }
    }

    fn delete(&self, service: &str, account: &str) -> Result<(), StoreError> {
        let entry = keyring::Entry::new(service, account).map_err(|_| StoreError::Failure)?;
        match entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Err(StoreError::Missing),
            Err(_) => Err(StoreError::Failure),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StoreError {
    Missing,
    Failure,
}

fn validate_service_name(value: &str) -> Result<String, Error> {
    if value.is_empty()
        || value.len() > SERVICE_NAME_MAX_BYTES
        || !value.chars().enumerate().all(|(index, character)| {
            character.is_ascii_alphanumeric() || (index > 0 && matches!(character, '.' | '_' | '-'))
        })
    {
        return Err(Error::InvalidServiceName);
    }
    Ok(value.to_string())
}

fn validate_keyring_reference(reference: &SecretRef) -> Result<(), Error> {
    if reference.backend() != BackendKind::Keyring {
        return Err(Error::BackendMismatch {
            provider: BackendKind::Keyring,
            reference: reference.backend(),
        });
    }
    Ok(())
}

fn account_name(reference: &SecretRef) -> String {
    alloc::format!(
        "{}.v{}",
        hex_encode(reference.id().as_str().as_bytes()),
        reference.key_version().get()
    )
}

fn reference_token(reference: &SecretRef) -> Vec<u8> {
    let mut token = Vec::from(TOKEN_PREFIX);
    token.extend_from_slice(&reference.key_version().get().to_be_bytes());
    token.extend_from_slice(reference.id().as_str().as_bytes());
    token
}

fn encode_material(material: &SecretMaterial) -> Zeroizing<String> {
    material.expose_secret(|bytes| {
        let mut encoded = String::with_capacity(VALUE_PREFIX.len() + bytes.len() * 2);
        encoded.push_str(VALUE_PREFIX);
        encoded.push_str(hex_encode(bytes).as_str());
        Zeroizing::new(encoded)
    })
}

fn decode_material(encoded: &str) -> Result<SecretMaterial, Error> {
    let hex = encoded
        .strip_prefix(VALUE_PREFIX)
        .ok_or_else(|| backend_failure(Operation::Read))?;
    let decoded = Zeroizing::new(hex_decode(hex)?);
    SecretMaterial::from_slice(decoded.as_slice())
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

fn hex_decode(value: &str) -> Result<Vec<u8>, Error> {
    if value.is_empty() || !value.len().is_multiple_of(2) {
        return Err(backend_failure(Operation::Read));
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let high = hex_nibble(pair[0])?;
            let low = hex_nibble(pair[1])?;
            Ok((high << 4) | low)
        })
        .collect()
}

const fn hex_nibble(value: u8) -> Result<u8, Error> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        _ => Err(backend_failure(Operation::Read)),
    }
}

fn not_found(reference: &SecretRef) -> Error {
    Error::SecretNotFound {
        backend: BackendKind::Keyring,
        key_version: reference.key_version().get(),
    }
}

const fn backend_failure(operation: Operation) -> Error {
    Error::BackendFailure {
        backend: BackendKind::Keyring,
        operation,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SecretId;
    use crate::context::{EnvelopeContext, EnvelopePurpose, EnvelopeSubject, PayloadSchemaId};
    use crate::id::KeyVersion;
    use alloc::collections::BTreeMap;
    use std::sync::Arc;

    #[derive(Default)]
    struct MockState {
        values: BTreeMap<(String, String), String>,
        calls: usize,
        fail: bool,
    }

    #[derive(Clone, Default)]
    struct MockStore {
        state: Arc<Mutex<MockState>>,
    }

    impl CredentialStore for MockStore {
        fn set(&self, service: &str, account: &str, value: &str) -> Result<(), StoreError> {
            let mut state = self.state.lock().map_err(|_| StoreError::Failure)?;
            state.calls += 1;
            if state.fail {
                return Err(StoreError::Failure);
            }
            state.values.insert(
                (service.to_string(), account.to_string()),
                value.to_string(),
            );
            Ok(())
        }

        fn get(&self, service: &str, account: &str) -> Result<String, StoreError> {
            let mut state = self.state.lock().map_err(|_| StoreError::Failure)?;
            state.calls += 1;
            if state.fail {
                return Err(StoreError::Failure);
            }
            state
                .values
                .get(&(service.to_string(), account.to_string()))
                .cloned()
                .ok_or(StoreError::Missing)
        }

        fn delete(&self, service: &str, account: &str) -> Result<(), StoreError> {
            let mut state = self.state.lock().map_err(|_| StoreError::Failure)?;
            state.calls += 1;
            if state.fail {
                return Err(StoreError::Failure);
            }
            state
                .values
                .remove(&(service.to_string(), account.to_string()))
                .map(|_| ())
                .ok_or(StoreError::Missing)
        }
    }

    fn reference(id: &str, version: u32) -> SecretRef {
        SecretRef::new(
            SecretId::parse(id).expect("valid id"),
            BackendKind::Keyring,
            KeyVersion::new(version).expect("version"),
        )
    }

    fn context() -> EnvelopeContext {
        EnvelopeContext::new(
            EnvelopePurpose::parse("radroots.keyring_test").expect("purpose"),
            EnvelopeSubject::parse("keyring_test", "fixture").expect("subject"),
            PayloadSchemaId::parse("radroots.keyring_test.v1").expect("schema"),
        )
    }

    fn different_context() -> EnvelopeContext {
        EnvelopeContext::new(
            EnvelopePurpose::parse("radroots.keyring_test").expect("purpose"),
            EnvelopeSubject::parse("keyring_test", "different").expect("subject"),
            PayloadSchemaId::parse("radroots.keyring_test.v1").expect("schema"),
        )
    }

    #[test]
    fn construction_has_no_store_side_effect() {
        let store = MockStore::default();
        let state = Arc::clone(&store.state);
        let provider =
            KeyringProvider::with_store("org.radroots.test", Box::new(store)).expect("provider");
        assert_eq!(state.lock().expect("state").calls, 0);
        assert_eq!(provider.backend_kind(), BackendKind::Keyring);
        assert_eq!(format!("{provider:?}"), "KeyringProvider(<redacted>)");
    }

    #[test]
    fn mock_store_round_trip_missing_rotation_and_removal() {
        let store = MockStore::default();
        let provider =
            KeyringProvider::with_store("org.radroots.test", Box::new(store)).expect("provider");
        let current = reference("keyring-key", 1);
        let next = reference("keyring-key", 2);
        let current_material = SecretMaterial::from_slice(&[0x11; 32]).expect("material");
        let next_material = SecretMaterial::from_slice(&[0x22; 32]).expect("material");
        let context = context();

        assert!(matches!(
            futures_executor::block_on(provider.wrap(WrapRequest::new(
                &current,
                &context,
                &current_material
            ))),
            Err(Error::SecretNotFound { .. })
        ));
        provider
            .provision(&current, &current_material)
            .expect("provision");
        let wrapped = futures_executor::block_on(provider.wrap(WrapRequest::new(
            &current,
            &context,
            &current_material,
        )))
        .expect("wrap");
        let opened = futures_executor::block_on(
            provider.unwrap(UnwrapRequest::new(&current, &context, &wrapped)),
        )
        .expect("unwrap");
        opened.expose_secret(|bytes| assert_eq!(bytes, &[0x11; 32]));
        assert!(matches!(
            futures_executor::block_on(provider.unwrap(UnwrapRequest::new(
                &current,
                &different_context(),
                &wrapped,
            ))),
            Err(Error::BackendFailure {
                backend: BackendKind::Keyring,
                operation: Operation::Unwrap,
            })
        ));

        provider
            .rotate(&current, &next, &next_material)
            .expect("rotate");
        assert!(matches!(
            provider.read_material(&current),
            Err(Error::SecretNotFound { .. })
        ));
        assert!(provider.read_material(&next).is_ok());
        assert!(provider.remove(&next).expect("remove"));
        assert!(!provider.remove(&next).expect("idempotent remove"));
    }

    #[test]
    fn native_store_failures_are_normalized() {
        let store = MockStore::default();
        store.state.lock().expect("state").fail = true;
        let provider =
            KeyringProvider::with_store("org.radroots.test", Box::new(store)).expect("provider");
        let reference = reference("keyring-key", 1);
        let material = SecretMaterial::from_slice(&[0x11; 32]).expect("material");
        assert_eq!(
            provider.provision(&reference, &material),
            Err(backend_failure(Operation::Provision))
        );
    }
}
