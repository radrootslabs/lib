//! Authenticated, protected NIP-59 evidence through participant-owned custody.
//! Cryptographic provenance supplies no consent, stock or history authority.
use alloc::string::String;
use base64::Engine as _;
use core::fmt;
use radroots_identity::PublicKey;
use radroots_protocol::private_rumor::{DecodedRumor, RumorWireError};
use radroots_signing::provider::{
    ParticipantProvider, PrivateCiphertext, PrivateCryptoCapability, PrivateText, ProviderCall,
    ProviderError,
};

/// Fixed diagnostics without private text or dependency-owned causes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpeningError {
    InvalidInput,
    LimitExceeded,
    Unauthenticated,
    RecipientMismatch,
    CarrierMismatch,
    UnsupportedVersion,
    Provider(ProviderError),
}
impl fmt::Display for OpeningError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidInput => "invalid_input",
            Self::LimitExceeded => "limit_exceeded",
            Self::Unauthenticated => "unauthenticated",
            Self::RecipientMismatch => "recipient_mismatch",
            Self::CarrierMismatch => "carrier_mismatch",
            Self::UnsupportedVersion => "unsupported_version",
            Self::Provider(error) => return write!(f, "provider_{error}"),
        })
    }
}
impl core::error::Error for OpeningError {}
impl From<ProviderError> for OpeningError {
    fn from(value: ProviderError) -> Self {
        Self::Provider(value)
    }
}
impl From<crate::event_verify::SignedReadError> for OpeningError {
    fn from(value: crate::event_verify::SignedReadError) -> Self {
        use crate::event_verify::SignedReadError;
        match value {
            SignedReadError::InvalidInput => Self::InvalidInput,
            SignedReadError::LimitExceeded => Self::LimitExceeded,
            SignedReadError::Unauthenticated => Self::Unauthenticated,
        }
    }
}

/// Obtained carrier evidence. Only this module can construct this value after
/// complete carrier admission and provider decryption. No automatic cloning or
/// serialization exposes its private data. Provider method honesty remains a
/// host qualification obligation.
pub struct AuthenticatedCarrier {
    recipient: PublicKey,
    envelope_id: radroots_event::EventId,
    seal_id: radroots_event::EventId,
    raw_outer: String,
    raw_seal: PrivateText,
    raw_rumor: PrivateText,
    rumor: DecodedRumor,
}
impl fmt::Debug for AuthenticatedCarrier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AuthenticatedCarrier(<redacted>)")
    }
}
impl AuthenticatedCarrier {
    pub const fn recipient(&self) -> PublicKey {
        self.recipient
    }
    pub const fn envelope_id(&self) -> radroots_event::EventId {
        self.envelope_id
    }
    pub const fn seal_id(&self) -> radroots_event::EventId {
        self.seal_id
    }
    pub const fn rumor(&self) -> &DecodedRumor {
        &self.rumor
    }
    /// Borrow original encrypted evidence for protected retention.
    pub fn protected_outer(&self) -> &str {
        &self.raw_outer
    }
    /// Borrow original decrypted evidence for protected retention.
    pub fn protected_seal(&self) -> &str {
        self.raw_seal.expose()
    }
    /// Borrow original unsigned evidence, including unsupported content.
    pub fn protected_rumor(&self) -> &str {
        self.raw_rumor.expose()
    }
}

fn ciphertext(value: &str, plaintext_limit: usize) -> Result<PrivateCiphertext, OpeningError> {
    let encoded = PrivateCiphertext::parse(value).map_err(|error| match error {
        ProviderError::LimitExceeded => OpeningError::LimitExceeded,
        ProviderError::UnsupportedVersion => OpeningError::UnsupportedVersion,
        _ => OpeningError::InvalidInput,
    })?;
    let raw = base64::engine::general_purpose::STANDARD
        .decode(value)
        .map_err(|_| OpeningError::InvalidInput)?;
    if raw.first() != Some(&2) {
        return Err(OpeningError::UnsupportedVersion);
    }
    if raw.len() < 99 || (raw.len() - 67) % 32 != 0 {
        return Err(OpeningError::InvalidInput);
    }
    // NIP44's authenticated short-length padding bounds the largest possible
    // plaintext before asking the provider to allocate/decrypt it.
    let power = plaintext_limit.next_power_of_two();
    let chunk = if power <= 256 { 32 } else { power / 8 };
    let padded_limit = plaintext_limit.div_ceil(chunk) * chunk;
    if raw.len() > padded_limit + 67 {
        return Err(OpeningError::LimitExceeded);
    }
    Ok(encoded)
}

async fn identity(
    provider: &dyn ParticipantProvider,
    expected: PublicKey,
) -> Result<(), OpeningError> {
    let state = provider.provider_state().await?;
    if state.identity() != expected {
        return Err(ProviderError::IdentityChanged.into());
    }
    if state.decryption() != PrivateCryptoCapability::Nip44V2 {
        return Err(ProviderError::CapabilityUnavailable.into());
    }
    Ok(())
}

/// Admit the original bounded outer before callbacks; authenticate each nested
/// signed carrier before decryption and preserve the obtained protected bytes.
/// The provider enforces fresh deadlines/cancellation before each crypto call.
pub async fn open(
    provider: &dyn ParticipantProvider,
    call: ProviderCall,
    raw_outer: &str,
) -> Result<AuthenticatedCarrier, OpeningError> {
    let outer = crate::event_verify::signed_json(raw_outer)?;
    if outer.kind_u32() != 1059 {
        return Err(OpeningError::CarrierMismatch);
    }
    let recipient = call.identity();
    let mut selected = false;
    for tag in outer.tags_as_vec() {
        if tag.first().map(String::as_str) == Some("p") {
            if selected || !(2..=3).contains(&tag.len()) || tag[1] != recipient.to_hex() {
                return Err(OpeningError::RecipientMismatch);
            }
            selected = true;
        }
    }
    if !selected {
        return Err(OpeningError::RecipientMismatch);
    }
    let encrypted = ciphertext(
        outer.content(),
        radroots_protocol::profile::SIGNED_EVENT_MAX_BYTES,
    )?;
    identity(provider, recipient).await?;
    let raw_seal = provider
        .decrypt_private(call.clone(), *outer.author(), encrypted)
        .await?;
    identity(provider, recipient).await?;
    let seal = crate::event_verify::signed_json(raw_seal.expose())?;
    if seal.kind_u32() != 13 || !seal.tags_as_vec().is_empty() {
        return Err(OpeningError::CarrierMismatch);
    }
    let encrypted = ciphertext(
        seal.content(),
        radroots_protocol::profile::UNSIGNED_RUMOR_MAX_BYTES,
    )?;
    let raw_rumor = provider
        .decrypt_private(call, *seal.author(), encrypted)
        .await?;
    identity(provider, recipient).await?;
    let rumor =
        DecodedRumor::decode(raw_rumor.expose().as_bytes()).map_err(|error| match error {
            RumorWireError::InvalidInput => OpeningError::InvalidInput,
            RumorWireError::LimitExceeded => OpeningError::LimitExceeded,
            RumorWireError::IdentityMismatch => OpeningError::Unauthenticated,
        })?;
    if rumor.author() != *seal.author() {
        return Err(OpeningError::CarrierMismatch);
    }
    Ok(AuthenticatedCarrier {
        recipient,
        envelope_id: *outer.id(),
        seal_id: *seal.id(),
        raw_outer: String::from(raw_outer),
        raw_seal,
        raw_rumor,
        rumor,
    })
}
