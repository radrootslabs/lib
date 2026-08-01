//! Data-key wrapping contracts.

use crate::SecretRef;
use crate::error::Error;
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::fmt;
use core::future::Future;
use core::pin::Pin;
use zeroize::Zeroizing;

/// Maximum plaintext accepted by the generic wrapping boundary.
pub const SECRET_MATERIAL_MAX_BYTES: usize = 64 * 1024;
/// Maximum protected value accepted by the generic wrapping boundary.
pub const WRAPPED_SECRET_MAX_BYTES: usize = 128 * 1024;

/// A sendable provider future that does not prescribe an executor.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Opaque, single-owner plaintext material that zeroizes on drop.
///
/// This type never implements `Clone` or `Serialize`, and its diagnostics are
/// always redacted. Callers must opt in to the narrow [`Self::expose_secret`]
/// scope when invoking cryptographic code.
///
/// ```compile_fail
/// use radroots_secrets::wrapping::SecretMaterial;
///
/// let material = SecretMaterial::from_slice(b"secret")?;
/// let _duplicate = material.clone();
/// # Ok::<(), radroots_secrets::Error>(())
/// ```
///
/// ```compile_fail
/// use radroots_secrets::wrapping::SecretMaterial;
///
/// let material = SecretMaterial::from_slice(b"secret")?;
/// let _json = serde_json::to_string(&material)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct SecretMaterial(Zeroizing<Box<[u8]>>);

impl SecretMaterial {
    /// Copies caller-supplied material into a zeroizing owner.
    pub fn from_slice(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.is_empty() || bytes.len() > SECRET_MATERIAL_MAX_BYTES {
            return Err(Error::InvalidSecretLength {
                actual_bytes: bytes.len(),
                max_bytes: SECRET_MATERIAL_MAX_BYTES,
            });
        }
        Ok(Self(Zeroizing::new(Box::from(bytes))))
    }

    pub(crate) fn from_owned(bytes: Vec<u8>) -> Result<Self, Error> {
        let bytes = Zeroizing::new(bytes);
        Self::from_slice(bytes.as_slice())
    }

    /// Exposes plaintext only for the lifetime of an explicit closure call.
    pub fn expose_secret<T>(&self, use_secret: impl FnOnce(&[u8]) -> T) -> T {
        use_secret(&self.0[..])
    }

    /// Returns the plaintext length without exposing its contents.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns whether the value is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for SecretMaterial {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SecretMaterial(<redacted>)")
    }
}

/// Opaque provider-wrapped material safe for persistence but not diagnostics.
#[derive(Clone, PartialEq, Eq)]
pub struct WrappedSecret(Vec<u8>);

impl WrappedSecret {
    /// Validates and owns provider-wrapped material.
    pub fn from_bytes(bytes: impl Into<Vec<u8>>) -> Result<Self, Error> {
        let bytes = bytes.into();
        if bytes.is_empty() || bytes.len() > WRAPPED_SECRET_MAX_BYTES {
            return Err(Error::InvalidWrappedLength {
                actual_bytes: bytes.len(),
                max_bytes: WRAPPED_SECRET_MAX_BYTES,
            });
        }
        Ok(Self(bytes))
    }

    /// Returns the wrapped representation for envelope persistence.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_slice()
    }
}

impl fmt::Debug for WrappedSecret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("WrappedSecret(<redacted>)")
    }
}

/// Borrowed input for one key-wrapping operation.
#[derive(Debug, Clone, Copy)]
pub struct WrapRequest<'a> {
    reference: &'a SecretRef,
    plaintext: &'a SecretMaterial,
}

impl<'a> WrapRequest<'a> {
    /// Creates an explicit wrapping request.
    #[must_use]
    pub const fn new(reference: &'a SecretRef, plaintext: &'a SecretMaterial) -> Self {
        Self {
            reference,
            plaintext,
        }
    }

    /// Returns the provider capability reference.
    #[must_use]
    pub const fn reference(&self) -> &'a SecretRef {
        self.reference
    }

    /// Returns the single-owner plaintext wrapper.
    #[must_use]
    pub const fn plaintext(&self) -> &'a SecretMaterial {
        self.plaintext
    }
}

/// Borrowed input for one key-unwrapping operation.
#[derive(Debug, Clone, Copy)]
pub struct UnwrapRequest<'a> {
    reference: &'a SecretRef,
    wrapped: &'a WrappedSecret,
}

impl<'a> UnwrapRequest<'a> {
    /// Creates an explicit unwrapping request.
    #[must_use]
    pub const fn new(reference: &'a SecretRef, wrapped: &'a WrappedSecret) -> Self {
        Self { reference, wrapped }
    }

    /// Returns the provider capability reference.
    #[must_use]
    pub const fn reference(&self) -> &'a SecretRef {
        self.reference
    }

    /// Returns the provider-wrapped value.
    #[must_use]
    pub const fn wrapped(&self) -> &'a WrappedSecret {
        self.wrapped
    }
}

/// Executor-neutral, dyn-compatible data-key wrapping.
pub trait KeyWrapping: Send + Sync {
    /// Wraps explicit caller-owned plaintext for the selected reference.
    fn wrap<'a>(&'a self, request: WrapRequest<'a>) -> BoxFuture<'a, Result<WrappedSecret, Error>>;

    /// Unwraps provider-owned protected material into a zeroizing owner.
    fn unwrap<'a>(
        &'a self,
        request: UnwrapRequest<'a>,
    ) -> BoxFuture<'a, Result<SecretMaterial, Error>>;
}

#[cfg(test)]
mod tests {
    use super::SecretMaterial;
    use zeroize::Zeroize;

    #[test]
    fn owned_plaintext_buffer_zeroizes_in_place() {
        let mut material =
            SecretMaterial::from_slice(b"owned plaintext sentinel").expect("valid secret material");
        let original_len = material.len();
        material.0.zeroize();
        assert_eq!(material.len(), original_len);
        material.expose_secret(|bytes| assert!(bytes.iter().all(|byte| *byte == 0)));
    }

    #[test]
    fn rejected_owned_plaintext_is_wrapped_before_validation() {
        assert!(SecretMaterial::from_owned(Vec::new()).is_err());
    }
}
