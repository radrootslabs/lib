//! Nostr key encoding and NIP-19 conversion for Radroots identities.
//!
//! This durable adapter keeps Nostr representation policy out of the identity
//! crate and keeps local secret material opaque.

use alloc::string::String;

use nostr::nips::nip19::{FromBech32, ToBech32};
use radroots_identity::PublicKey;

use crate::Error;

/// Converts a canonical Radroots public key into its Nostr representation.
pub fn public_key_to_nostr(public_key: PublicKey) -> Result<nostr::PublicKey, Error> {
    nostr::PublicKey::from_slice(public_key.as_bytes()).map_err(|_| Error::InvalidPublicKey)
}

/// Converts a Nostr public key into the canonical Radroots representation.
pub fn public_key_from_nostr(public_key: nostr::PublicKey) -> Result<PublicKey, Error> {
    PublicKey::from_bytes(public_key.to_bytes()).map_err(|_| Error::InvalidPublicKey)
}

/// Encodes a canonical Radroots public key as a NIP-19 `npub`.
pub fn public_key_to_npub(public_key: PublicKey) -> Result<String, Error> {
    let public_key = public_key_to_nostr(public_key)?;
    match public_key.to_bech32() {
        Ok(encoded) => Ok(encoded),
        Err(error) => match error {},
    }
}

/// Decodes a NIP-19 `npub` into the canonical Radroots public-key value.
pub fn public_key_from_npub(encoded: &str) -> Result<PublicKey, Error> {
    nostr::PublicKey::from_bech32(encoded)
        .map_err(|_| Error::InvalidNpub)
        .and_then(public_key_from_nostr)
}

/// Parses a canonical hexadecimal public key or a NIP-19 `npub`.
pub fn parse_public_key(encoded: &str) -> Result<PublicKey, Error> {
    if encoded.starts_with("npub1") {
        public_key_from_npub(encoded)
    } else {
        PublicKey::from_hex(encoded).map_err(|_| Error::InvalidPublicKey)
    }
}

/// An opaque local Nostr secret key.
///
/// The value does not implement `Clone`, serialization, or unrestricted
/// plaintext access. Debug output is always redacted; the concrete local
/// signing adapter consumes the value through crate-private integration.
///
/// ```compile_fail
/// use radroots_nostr::key::SecretKey;
///
/// let key = SecretKey::parse(
///     "0000000000000000000000000000000000000000000000000000000000000001",
/// )?;
/// let _duplicate = key.clone();
/// # Ok::<(), radroots_nostr::Error>(())
/// ```
#[cfg(feature = "signing")]
pub struct SecretKey {
    inner: nostr::SecretKey,
}

#[cfg(feature = "signing")]
impl SecretKey {
    /// Generates a fresh local secret without exposing its representation.
    #[must_use]
    pub fn generate() -> Self {
        Self {
            inner: nostr::SecretKey::generate(),
        }
    }

    /// Parses exact hexadecimal or NIP-19 `nsec` text.
    ///
    /// Errors never retain or render the supplied secret material.
    pub fn parse(encoded: &str) -> Result<Self, Error> {
        nostr::SecretKey::parse(encoded)
            .map(|inner| Self { inner })
            .map_err(|_| Error::InvalidSecretKey)
    }

    /// Derives the canonical public identity without exposing secret bytes.
    pub fn public_key(&self) -> Result<PublicKey, Error> {
        let public_key = nostr::Keys::new(self.inner.clone()).public_key();
        public_key_from_nostr(public_key)
    }

    pub(crate) fn into_keys(self) -> nostr::Keys {
        nostr::Keys::new(self.inner)
    }
}

#[cfg(feature = "signing")]
impl core::fmt::Debug for SecretKey {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("SecretKey")
            .field(&"[redacted]")
            .finish()
    }
}

/// NIP-49 metadata describing how the plaintext key was previously handled.
#[cfg(feature = "signing")]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Nip49KeySecurity {
    /// The plaintext key is known to have been handled insecurely.
    Weak,
    /// The plaintext key is not known to have been handled insecurely.
    Medium,
    /// The caller does not track plaintext-key handling.
    #[default]
    Unknown,
}

#[cfg(feature = "signing")]
impl From<Nip49KeySecurity> for nostr::nips::nip49::KeySecurity {
    fn from(value: Nip49KeySecurity) -> Self {
        match value {
            Nip49KeySecurity::Weak => Self::Weak,
            Nip49KeySecurity::Medium => Self::Medium,
            Nip49KeySecurity::Unknown => Self::Unknown,
        }
    }
}

/// Explicit NIP-49 encryption parameters.
#[cfg(feature = "signing")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Nip49Options {
    log_n: u8,
    key_security: Nip49KeySecurity,
}

#[cfg(feature = "signing")]
impl Nip49Options {
    /// Creates NIP-49 options with an explicit scrypt `log2(N)` work factor.
    #[must_use]
    pub const fn new(log_n: u8, key_security: Nip49KeySecurity) -> Self {
        Self {
            log_n,
            key_security,
        }
    }

    /// Returns the scrypt `log2(N)` work factor.
    #[must_use]
    pub const fn log_n(self) -> u8 {
        self.log_n
    }

    /// Returns the NIP-49 plaintext-key handling metadata.
    #[must_use]
    pub const fn key_security(self) -> Nip49KeySecurity {
        self.key_security
    }
}

#[cfg(feature = "signing")]
impl Default for Nip49Options {
    fn default() -> Self {
        Self::new(16, Nip49KeySecurity::Unknown)
    }
}

/// Parses a Nostr secret key from exact hexadecimal or NIP-19 `nsec` text.
///
/// Errors never retain or render the supplied secret material.
#[cfg(feature = "signing")]
pub fn parse_secret_key(encoded: &str) -> Result<SecretKey, Error> {
    SecretKey::parse(encoded)
}

/// Encodes a Nostr secret key as NIP-19 `nsec` text.
#[cfg(feature = "signing")]
pub fn secret_key_to_nsec(secret_key: &SecretKey) -> String {
    match secret_key.inner.to_bech32() {
        Ok(encoded) => encoded,
        Err(error) => match error {},
    }
}

/// Encrypts a Nostr secret key into a NIP-49 `ncryptsec` payload.
#[cfg(feature = "signing")]
pub fn encrypt_secret_key_nip49(secret_key: &SecretKey, password: &str) -> Result<String, Error> {
    encrypt_secret_key_nip49_with_options(secret_key, password, Nip49Options::default())
}

/// Encrypts a Nostr secret key with explicit NIP-49 parameters.
#[cfg(feature = "signing")]
pub fn encrypt_secret_key_nip49_with_options(
    secret_key: &SecretKey,
    password: &str,
    options: Nip49Options,
) -> Result<String, Error> {
    let encrypted = nostr::nips::nip49::EncryptedSecretKey::new(
        &secret_key.inner,
        password,
        options.log_n,
        options.key_security.into(),
    )
    .map_err(|_| Error::SecretKeyEncryption)?;
    encrypted
        .to_bech32()
        .map_err(|_| Error::SecretKeyEncryption)
}

/// Decrypts a NIP-49 `ncryptsec` payload into a Nostr secret key.
///
/// Parse, password, and ciphertext failures are deliberately normalized so
/// diagnostics never retain the encrypted payload, password, or plaintext.
#[cfg(feature = "signing")]
pub fn decrypt_secret_key_nip49(encrypted: &str, password: &str) -> Result<SecretKey, Error> {
    nostr::nips::nip49::EncryptedSecretKey::from_bech32(encrypted)
        .map_err(|_| Error::InvalidEncryptedSecretKey)?
        .decrypt(password)
        .map(|inner| SecretKey { inner })
        .map_err(|_| Error::SecretKeyDecryption)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::FIXTURE_ALICE;

    #[cfg(feature = "signing")]
    const NCRYPTSEC: &str = "ncryptsec1qgg9947rlpvqu76pj5ecreduf9jxhselq2nae2kghhvd5g7dgjtcxfqtd67p9m0w57lspw8gsq6yphnm8623nsl8xn9j4jdzz84zm3frztj3z7s35vpzmqf6ksu8r89qk5z2zxfmu5gv8th8wclt0h4p";
    #[cfg(feature = "signing")]
    const NCRYPTSEC_SECRET_HEX: &str =
        "3501454135014541350145413501453fefb02227e449e57cf4d3a3ce05378683";

    #[test]
    fn native_public_key_round_trips_through_nostr_hex_and_npub() {
        let native =
            PublicKey::from_hex(FIXTURE_ALICE.public_key_hex).expect("native public key fixture");
        let nostr = public_key_to_nostr(native).expect("Nostr public key");

        assert_eq!(nostr.to_hex(), FIXTURE_ALICE.public_key_hex);
        assert_eq!(
            public_key_from_nostr(nostr).expect("native public key"),
            native
        );
        assert_eq!(
            public_key_to_npub(native).expect("npub"),
            FIXTURE_ALICE.npub
        );
        assert_eq!(
            public_key_from_npub(FIXTURE_ALICE.npub).expect("native npub"),
            native
        );
        assert_eq!(
            parse_public_key(FIXTURE_ALICE.public_key_hex).expect("hex public key"),
            native
        );
        assert_eq!(
            parse_public_key(FIXTURE_ALICE.npub).expect("npub public key"),
            native
        );
    }

    #[test]
    fn public_key_parsing_rejects_wrong_nip19_kinds_without_echoing_input() {
        let invalid = "nsec1-do-not-disclose-public-key-input";
        let error = parse_public_key(invalid).expect_err("secret HRP is not a public key");

        assert!(matches!(error, Error::InvalidPublicKey));
        assert!(!error.to_string().contains(invalid));
        assert!(!format!("{error:?}").contains(invalid));

        let malformed_npub = "npub1-do-not-disclose-public-key-input";
        let error = public_key_from_npub(malformed_npub).expect_err("malformed npub");
        assert!(matches!(error, Error::InvalidNpub));
        assert!(!error.to_string().contains(malformed_npub));
        assert!(!format!("{error:?}").contains(malformed_npub));
    }

    #[cfg(feature = "signing")]
    #[test]
    fn secret_key_hex_and_nsec_vectors_round_trip() {
        let from_hex = parse_secret_key(FIXTURE_ALICE.secret_key_hex).expect("hex secret key");
        let from_nsec = parse_secret_key(FIXTURE_ALICE.nsec).expect("nsec secret key");

        assert_eq!(secret_key_to_nsec(&from_hex), FIXTURE_ALICE.nsec);
        assert_eq!(secret_key_to_nsec(&from_nsec), FIXTURE_ALICE.nsec);
        assert_eq!(
            from_hex.public_key().expect("public key").to_hex(),
            FIXTURE_ALICE.public_key_hex
        );
        for rendered in [format!("{from_hex:?}"), format!("{from_nsec:?}")] {
            assert!(rendered.contains("[redacted]"));
            assert!(!rendered.contains(FIXTURE_ALICE.secret_key_hex));
            assert!(!rendered.contains(FIXTURE_ALICE.nsec));
        }
    }

    #[cfg(feature = "signing")]
    #[test]
    fn nip49_known_vector_decrypts_and_round_trips_with_explicit_options() {
        let decrypted = decrypt_secret_key_nip49(NCRYPTSEC, "nostr").expect("known ncryptsec");
        let expected = parse_secret_key(NCRYPTSEC_SECRET_HEX).expect("known secret key");
        assert_eq!(
            secret_key_to_nsec(&decrypted),
            secret_key_to_nsec(&expected)
        );

        let options = Nip49Options::new(10, Nip49KeySecurity::Medium);
        assert_eq!(options.log_n(), 10);
        assert_eq!(options.key_security(), Nip49KeySecurity::Medium);
        let encrypted = encrypt_secret_key_nip49_with_options(&decrypted, "test-password", options)
            .expect("encrypt ncryptsec");
        let round_trip =
            decrypt_secret_key_nip49(&encrypted, "test-password").expect("decrypt ncryptsec");
        assert_eq!(
            secret_key_to_nsec(&round_trip),
            secret_key_to_nsec(&decrypted)
        );
    }

    #[cfg(feature = "signing")]
    #[test]
    fn secret_failures_are_redacted() {
        let invalid_secret = "nsec1-do-not-disclose-secret-input";
        let parse_error = parse_secret_key(invalid_secret).expect_err("invalid secret");
        assert!(matches!(parse_error, Error::InvalidSecretKey));
        assert!(!parse_error.to_string().contains(invalid_secret));
        assert!(!format!("{parse_error:?}").contains(invalid_secret));

        let password = "do-not-disclose-password";
        let decrypt_error =
            decrypt_secret_key_nip49(NCRYPTSEC, password).expect_err("wrong password");
        assert!(matches!(decrypt_error, Error::SecretKeyDecryption));
        for rendered in [decrypt_error.to_string(), format!("{decrypt_error:?}")] {
            assert!(!rendered.contains(password));
            assert!(!rendered.contains(NCRYPTSEC));
            assert!(!rendered.contains(NCRYPTSEC_SECRET_HEX));
        }

        let encrypted_error =
            decrypt_secret_key_nip49(invalid_secret, password).expect_err("invalid ncryptsec");
        assert!(matches!(encrypted_error, Error::InvalidEncryptedSecretKey));
        assert!(!encrypted_error.to_string().contains(invalid_secret));
        assert!(!format!("{encrypted_error:?}").contains(invalid_secret));
    }
}
