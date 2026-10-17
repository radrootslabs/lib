//! Participant identity and custody-preserving private-message capabilities.
//!
//! Crypto primitives do not authorize a participant, verify a message carrier,
//! admit a command or establish durable execution. The composing executor must
//! authenticate the signed carrier before decryption and enforce its stricter
//! record/rumor/event budgets. Obtained results remain owned by that executor.

#[cfg(not(feature = "std"))]
use alloc::{boxed::Box, string::String};
use core::{fmt, num::NonZeroU64};
use radroots_identity::PublicKey;
#[cfg(feature = "std")]
use std::string::String;
use zeroize::Zeroizing;

use crate::{
    Signer,
    host::{HostBound, Shared},
    request::CancellationSignal,
    signer::BoxFuture,
};

/// Maximum bytes in one crypto boundary input or output. Profile budgets are
/// independently enforced before invoking this lower-level capability.
pub const PRIVATE_CRYPTO_MAX_BYTES: usize = 65_536;

/// Fixed, source-redacted private-provider failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ProviderError {
    /// Input representation is invalid.
    InvalidInput,
    /// The bounded crypto boundary cannot represent the input or output.
    LimitExceeded,
    /// The payload's encryption version is unsupported.
    UnsupportedVersion,
    /// Required provider support or entropy is unavailable.
    CapabilityUnavailable,
    /// The selected participant differs from this provider's current key.
    IdentityChanged,
    /// Authentication of the encrypted payload failed.
    AuthenticationFailed,
    /// Cancellation was observed before beginning work.
    Cancelled,
    /// The call's deadline expired before work began.
    DeadlineExceeded,
    /// A provider failed without returning private dependency diagnostics.
    ProviderFailure,
}

impl ProviderError {
    /// Returns a stable diagnostic without private input or dependency prose.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidInput => "invalid_input",
            Self::LimitExceeded => "limit_exceeded",
            Self::UnsupportedVersion => "unsupported_version",
            Self::CapabilityUnavailable => "capability_unavailable",
            Self::IdentityChanged => "identity_changed",
            Self::AuthenticationFailed => "authentication_failed",
            Self::Cancelled => "cancelled",
            Self::DeadlineExceeded => "deadline_exceeded",
            Self::ProviderFailure => "provider_failure",
        }
    }
}
impl fmt::Display for ProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
impl core::error::Error for ProviderError {}

/// Owned private UTF-8 text with bounded construction and cleared storage on
/// drop. It has no serialization, cloning or input-bearing diagnostic surface.
pub struct PrivateText(Zeroizing<String>);

impl PrivateText {
    /// Checks the byte budget before copying borrowed input.
    pub fn new(value: &str) -> Result<Self, ProviderError> {
        if value.is_empty() {
            return Err(ProviderError::InvalidInput);
        }
        if value.len() > PRIVATE_CRYPTO_MAX_BYTES {
            return Err(ProviderError::LimitExceeded);
        }
        Ok(Self(Zeroizing::new(String::from(value))))
    }

    /// Takes already obtained private text without copying it. Failure also
    /// clears the owned buffer; no rejected text is retained in an error.
    pub fn from_owned(value: String) -> Result<Self, ProviderError> {
        let value = Zeroizing::new(value);
        if value.is_empty() {
            return Err(ProviderError::InvalidInput);
        }
        if value.len() > PRIVATE_CRYPTO_MAX_BYTES {
            return Err(ProviderError::LimitExceeded);
        }
        Ok(Self(value))
    }

    /// Explicitly borrows protected plaintext for its authorized consumer.
    pub fn expose(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for PrivateText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PrivateText([redacted])")
    }
}

/// Owned bounded encoded ciphertext. Construction is structural only; it does
/// not verify its MAC, encryption version, author or signed-event carrier.
pub struct PrivateCiphertext(String);

impl PrivateCiphertext {
    /// Checks bounds before copying an untrusted provider or event payload.
    pub fn parse(value: &str) -> Result<Self, ProviderError> {
        if value.len() > PRIVATE_CRYPTO_MAX_BYTES {
            return Err(ProviderError::LimitExceeded);
        }
        if value.starts_with('#') {
            return Err(ProviderError::UnsupportedVersion);
        }
        if value.len() < 132
            || !value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'='))
        {
            return Err(ProviderError::InvalidInput);
        }
        Ok(Self(String::from(value)))
    }

    /// Borrows encoded private evidence for storage or its authenticated carrier.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for PrivateCiphertext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PrivateCiphertext([redacted])")
    }
}

/// One observed private-crypto method capability, not evidence of qualification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrivateCryptoCapability {
    /// This provider cannot perform this method in its current configuration.
    Unavailable,
    /// The provider implements the bounded NIP-44 version-2 method.
    Nip44V2,
}

/// An identity/capability observation. It supplies neither a scoped grant nor
/// permission to sign, allocate stock or reinterpret a later identity change.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderState {
    identity: PublicKey,
    encryption: PrivateCryptoCapability,
    decryption: PrivateCryptoCapability,
    signing_replay: crate::recovery::ReplayCapability,
    exact_issuance: Option<crate::recovery::ReplayCapability>,
}
impl ProviderState {
    /// Reports capabilities actually implemented by the observing provider.
    pub const fn new(
        identity: PublicKey,
        encryption: PrivateCryptoCapability,
        decryption: PrivateCryptoCapability,
        signing_replay: crate::recovery::ReplayCapability,
    ) -> Self {
        Self {
            identity,
            encryption,
            decryption,
            signing_replay,
            exact_issuance: None,
        }
    }
    /// Returns the observed participant key, independently of any hosted principal.
    pub const fn identity(self) -> PublicKey {
        self.identity
    }
    /// Returns current encryption support.
    pub const fn encryption(self) -> PrivateCryptoCapability {
        self.encryption
    }
    /// Returns current decryption support.
    pub const fn decryption(self) -> PrivateCryptoCapability {
        self.decryption
    }
    /// Returns signer replay behavior for this identity observation. This never
    /// overrides retained uncertainty or permits blind signing reissue.
    pub const fn signing_replay(self) -> crate::recovery::ReplayCapability {
        self.signing_replay
    }

    /// Reports support for the exact-intent issuance contract separately from
    /// the retained legacy signing API. Absence means unavailable.
    pub const fn exact_issuance(self) -> Option<crate::recovery::ReplayCapability> {
        self.exact_issuance
    }

    /// Records the actual provider's qualified exact-intent support. This
    /// observation alone never authorizes replay or business execution.
    pub const fn with_exact_issuance(
        mut self,
        replay: Option<crate::recovery::ReplayCapability>,
    ) -> Self {
        self.exact_issuance = replay;
        self
    }
}

/// Explicit per-call identity, deadline and cooperative cancellation binding.
/// This value is not an authenticated command context or durable admission.
#[derive(Clone)]
pub struct ProviderCall {
    identity: PublicKey,
    deadline_unix_ms: NonZeroU64,
    cancellation: CancellationSignal,
}
impl ProviderCall {
    /// Freezes the identity observed by the owning session and its call budget.
    pub const fn new(
        identity: PublicKey,
        deadline_unix_ms: NonZeroU64,
        cancellation: CancellationSignal,
    ) -> Self {
        Self {
            identity,
            deadline_unix_ms,
            cancellation,
        }
    }
    /// Returns the participant key frozen by the owning session.
    pub const fn identity(&self) -> PublicKey {
        self.identity
    }

    /// Returns the captured first-issue deadline without consulting a clock.
    pub const fn deadline_unix_ms(&self) -> NonZeroU64 {
        self.deadline_unix_ms
    }
    /// Validates admission before an effect; never discard an obtained result
    /// by repeating this check after the effect.
    pub fn check_before_effect(
        &self,
        current_identity: PublicKey,
        now_unix_ms: u64,
    ) -> Result<(), ProviderError> {
        if current_identity != self.identity {
            return Err(ProviderError::IdentityChanged);
        }
        if self.cancellation.is_cancelled() {
            return Err(ProviderError::Cancelled);
        }
        if now_unix_ms == 0 {
            return Err(ProviderError::ProviderFailure);
        }
        if now_unix_ms >= self.deadline_unix_ms.get() {
            return Err(ProviderError::DeadlineExceeded);
        }
        Ok(())
    }
}

/// Injected nonce entropy. Production implementations must supply fresh,
/// uniformly random 32-byte output from a qualified CSPRNG. The trait and a
/// returned array do not themselves prove entropy quality or uniqueness.
pub trait NonceSource: HostBound {
    /// Obtains one nonce or reports failure before encryption begins.
    fn next_nonce(&self) -> Result<[u8; 32], ProviderError>;
}

/// Custody-preserving participant provider. Native futures/handles retain their
/// thread guarantees; browser implementations may retain local UI/key handles.
/// No method exposes a private key or implicitly publishes a message.
pub trait ParticipantProvider: Signer {
    /// Issues one exact captured intent. Unsupported adapters do not delegate
    /// to the legacy method, which may discard already obtained results.
    /// A result is not durable until the executor confirms its capture.
    fn sign_exact(
        &self,
        intent: crate::issuance::SigningIntent,
    ) -> BoxFuture<'_, crate::issuance::SigningOutcome> {
        Box::pin(async move {
            crate::issuance::SigningOutcome::not_dispatched(
                intent.binding(),
                ProviderError::CapabilityUnavailable,
            )
        })
    }

    /// Observes the actual key and currently available methods without signing.
    fn provider_state(&self) -> BoxFuture<'_, Result<ProviderState, ProviderError>>;
    /// Encrypts private text using provider-owned custody and qualified entropy.
    /// The executor retains the returned preimage before any signing request.
    fn encrypt_private(
        &self,
        call: ProviderCall,
        recipient: PublicKey,
        plaintext: PrivateText,
    ) -> BoxFuture<'_, Result<PrivateCiphertext, ProviderError>>;
    /// Decrypts an authenticated carrier's ciphertext. The executor must verify
    /// its event ID/signature and bind its author before invoking this primitive.
    fn decrypt_private(
        &self,
        call: ProviderCall,
        sender: PublicKey,
        ciphertext: PrivateCiphertext,
    ) -> BoxFuture<'_, Result<PrivateText, ProviderError>>;
}

/// Host callback roots belonging to one scoped participant-provider handle.
/// Host implementations must dispatch on their required thread and retain
/// partial progress across cancellation and known failure. These methods do
/// not revoke another session or erase already obtained signing evidence.
pub trait SignerCallbacks: HostBound {
    /// Stops new dispatch and safely unblocks pending identity observations.
    /// Keep roots and every already obtained signing result until retained.
    fn quiesce(&self) -> BoxFuture<'_, Result<(), ProviderError>>;

    /// Drains dispatched callbacks, including callbacks whose Rust observer
    /// was dropped. Success permits releasing this scope's callback roots.
    fn close(&self) -> BoxFuture<'_, Result<(), ProviderError>>;
}

/// Owned provider and its exact host callback scope. Construction invokes
/// neither object and grants no workspace, command or stock authority.
/// The host must pair these capabilities from the same actual dispatch scope.
pub struct SignerHandle {
    provider: Shared<dyn ParticipantProvider>,
    callbacks: Shared<dyn SignerCallbacks>,
}

impl SignerHandle {
    /// Takes ownership of one explicitly constructed provider scope.
    pub fn new(
        provider: Shared<dyn ParticipantProvider>,
        callbacks: Shared<dyn SignerCallbacks>,
    ) -> Self {
        Self {
            provider,
            callbacks,
        }
    }

    /// Transfers both owned capabilities to the composing lifecycle owner.
    /// This exposes no private key and supplies no authorization or proof of
    /// provider qualification. The owner must retain roots through closure.
    pub fn into_parts(self) -> (Shared<dyn ParticipantProvider>, Shared<dyn SignerCallbacks>) {
        (self.provider, self.callbacks)
    }
}
