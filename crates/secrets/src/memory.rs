//! Deterministic in-process secret adapters.

use crate::SecretRef;
use crate::error::{Error, Operation};
use crate::id::BackendKind;
use crate::provider::{CapabilitySupport, ResidencySupport, SecretCapabilities, SecretProvider};
use crate::wrapping::{
    BoxFuture, KeyWrapping, SecretMaterial, UnwrapRequest, WrapRequest, WrappedSecret,
};
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;
use std::sync::RwLock;

const TOKEN_MAGIC: &[u8] = b"radroots-memory-key-v1\0";

/// Explicit-lifecycle, in-process provider for development and tests.
#[derive(Default)]
pub struct MemoryProvider {
    entries: RwLock<BTreeMap<MemoryKey, SecretMaterial>>,
}

impl MemoryProvider {
    /// Creates an empty provider without generating or installing keys.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Installs caller-owned material at one exact reference.
    pub fn provision(&self, reference: &SecretRef, material: SecretMaterial) -> Result<(), Error> {
        validate_memory_reference(reference)?;
        let key = MemoryKey::from_reference(reference);
        let mut entries = self
            .entries
            .write()
            .map_err(|_| backend_failure(Operation::Provision))?;
        if entries.contains_key(&key) {
            return Err(Error::SecretAlreadyExists {
                backend: BackendKind::Memory,
                key_version: reference.key_version().get(),
            });
        }
        entries.insert(key, material);
        Ok(())
    }

    /// Atomically replaces one version with a higher version of the same ID.
    pub fn rotate(
        &self,
        current: &SecretRef,
        next: &SecretRef,
        material: SecretMaterial,
    ) -> Result<(), Error> {
        validate_memory_reference(current)?;
        validate_memory_reference(next)?;
        if current.id().as_str() != next.id().as_str()
            || next.key_version() <= current.key_version()
        {
            return Err(Error::InvalidRotation);
        }
        let current_key = MemoryKey::from_reference(current);
        let next_key = MemoryKey::from_reference(next);
        let mut entries = self
            .entries
            .write()
            .map_err(|_| backend_failure(Operation::Rotate))?;
        if !entries.contains_key(&current_key) {
            return Err(not_found(current));
        }
        if entries.contains_key(&next_key) {
            return Err(Error::SecretAlreadyExists {
                backend: BackendKind::Memory,
                key_version: next.key_version().get(),
            });
        }
        entries.remove(&current_key);
        entries.insert(next_key, material);
        Ok(())
    }

    /// Removes and zeroizes provider-owned material when present.
    pub fn remove(&self, reference: &SecretRef) -> Result<bool, Error> {
        validate_memory_reference(reference)?;
        self.entries
            .write()
            .map(|mut entries| {
                entries
                    .remove(&MemoryKey::from_reference(reference))
                    .is_some()
            })
            .map_err(|_| backend_failure(Operation::Remove))
    }

    /// Reports whether one exact reference is provisioned.
    pub fn contains(&self, reference: &SecretRef) -> Result<bool, Error> {
        validate_memory_reference(reference)?;
        self.entries
            .read()
            .map(|entries| entries.contains_key(&MemoryKey::from_reference(reference)))
            .map_err(|_| backend_failure(Operation::Unwrap))
    }

    fn wrapped_token(reference: &SecretRef) -> Result<WrappedSecret, Error> {
        let id = reference.id().as_str().as_bytes();
        let mut token = Vec::with_capacity(TOKEN_MAGIC.len() + 4 + 2 + id.len());
        token.extend_from_slice(TOKEN_MAGIC);
        token.extend_from_slice(&reference.key_version().get().to_be_bytes());
        let id_len = u16::try_from(id.len()).map_err(|_| backend_failure(Operation::Wrap))?;
        token.extend_from_slice(&id_len.to_be_bytes());
        token.extend_from_slice(id);
        WrappedSecret::from_bytes(token)
    }

    fn clone_material(
        &self,
        reference: &SecretRef,
        operation: Operation,
    ) -> Result<SecretMaterial, Error> {
        let entries = self
            .entries
            .read()
            .map_err(|_| backend_failure(operation))?;
        let material = entries
            .get(&MemoryKey::from_reference(reference))
            .ok_or_else(|| not_found(reference))?;
        material.expose_secret(SecretMaterial::from_slice)
    }
}

impl fmt::Debug for MemoryProvider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("MemoryProvider(<redacted>)")
    }
}

impl KeyWrapping for MemoryProvider {
    fn wrap<'a>(&'a self, request: WrapRequest<'a>) -> BoxFuture<'a, Result<WrappedSecret, Error>> {
        Box::pin(async move {
            validate_memory_reference(request.reference())?;
            let provisioned = self.clone_material(request.reference(), Operation::Wrap)?;
            let matches = provisioned.expose_secret(|expected| {
                request
                    .plaintext()
                    .expose_secret(|actual| expected == actual)
            });
            if !matches {
                return Err(backend_failure(Operation::Wrap));
            }
            Self::wrapped_token(request.reference())
        })
    }

    fn unwrap<'a>(
        &'a self,
        request: UnwrapRequest<'a>,
    ) -> BoxFuture<'a, Result<SecretMaterial, Error>> {
        Box::pin(async move {
            validate_memory_reference(request.reference())?;
            let expected = Self::wrapped_token(request.reference())?;
            if expected.as_bytes() != request.wrapped().as_bytes() {
                return Err(backend_failure(Operation::Unwrap));
            }
            self.clone_material(request.reference(), Operation::Unwrap)
        })
    }
}

impl SecretProvider for MemoryProvider {
    fn backend_kind(&self) -> BackendKind {
        BackendKind::Memory
    }

    fn capabilities(&self) -> SecretCapabilities {
        SecretCapabilities::available(
            ResidencySupport::Volatile,
            CapabilitySupport::Unavailable,
            CapabilitySupport::Unavailable,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct MemoryKey {
    id: String,
    version: u32,
}

impl MemoryKey {
    fn from_reference(reference: &SecretRef) -> Self {
        Self {
            id: reference.id().as_str().to_string(),
            version: reference.key_version().get(),
        }
    }
}

fn validate_memory_reference(reference: &SecretRef) -> Result<(), Error> {
    if reference.backend() != BackendKind::Memory {
        return Err(Error::BackendMismatch {
            provider: BackendKind::Memory,
            reference: reference.backend(),
        });
    }
    Ok(())
}

const fn backend_failure(operation: Operation) -> Error {
    Error::BackendFailure {
        backend: BackendKind::Memory,
        operation,
    }
}

fn not_found(reference: &SecretRef) -> Error {
    Error::SecretNotFound {
        backend: BackendKind::Memory,
        key_version: reference.key_version().get(),
    }
}
