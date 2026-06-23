use alloc::string::String;
use core::fmt;
use radroots_simplex_smp_proto::prelude::RadrootsSimplexSmpProtoError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadrootsSimplexSmpCryptoError {
    Proto(RadrootsSimplexSmpProtoError),
    InvalidShortFieldLength(usize),
    EntropyUnavailable,
    MissingRatchetKey(&'static str),
    IncompletePqHeader,
    RatchetMessageRegression { received: u32, current: u32 },
    RatchetTooManySkipped { skipped: u32, max: u32 },
    InvalidSharedSecretLength(usize),
    InvalidCiphertextLength(usize),
    InvalidNonceLength(usize),
    InvalidMessageLength { actual: usize, padded: usize },
    InvalidPublicKeyLength(usize),
    InvalidPrivateKeyLength(usize),
    InvalidSignatureLength(usize),
    SignatureVerificationFailed,
    InvalidSessionIdentifier(String),
    InvalidKeyDerivationLength(usize),
    InvalidSecretBoxChainKeyLength(usize),
    InvalidShortLinkIdLength(usize),
    InvalidShortLinkKeyLength(usize),
    InvalidShortLinkDataLength { field: &'static str, length: usize },
    ShortLinkDataHashMismatch,
    AesGcmAuthenticationFailed,
    InvalidOfficialRatchetVersion(u16),
    InvalidOfficialRatchetPadding,
    InvalidOfficialX3dhParameters(String),
    InvalidPqKeyLength(usize),
    InvalidPqCiphertextLength(usize),
}

impl From<RadrootsSimplexSmpProtoError> for RadrootsSimplexSmpCryptoError {
    fn from(value: RadrootsSimplexSmpProtoError) -> Self {
        Self::Proto(value)
    }
}

impl fmt::Display for RadrootsSimplexSmpCryptoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Proto(error) => write!(f, "{error}"),
            Self::InvalidShortFieldLength(length) => {
                write!(f, "invalid SMP short field length {length}")
            }
            Self::EntropyUnavailable => {
                write!(f, "unable to obtain entropy for SimpleX SMP key generation")
            }
            Self::MissingRatchetKey(field) => write!(f, "missing SMP ratchet key `{field}`"),
            Self::IncompletePqHeader => {
                write!(
                    f,
                    "SMP PQ ratchet header must include both key and ciphertext"
                )
            }
            Self::RatchetMessageRegression { received, current } => {
                write!(
                    f,
                    "SMP ratchet message regression: received {received}, current {current}"
                )
            }
            Self::RatchetTooManySkipped { skipped, max } => {
                write!(
                    f,
                    "SMP ratchet skipped {skipped} messages, exceeding maximum {max}"
                )
            }
            Self::InvalidSharedSecretLength(length) => {
                write!(f, "invalid SMP shared secret length {length}")
            }
            Self::InvalidCiphertextLength(length) => {
                write!(f, "invalid SMP ciphertext length {length}")
            }
            Self::InvalidNonceLength(length) => {
                write!(f, "invalid SMP nonce length {length}")
            }
            Self::InvalidMessageLength { actual, padded } => {
                write!(
                    f,
                    "invalid SMP padded message length: actual {actual}, padded {padded}"
                )
            }
            Self::InvalidPublicKeyLength(length) => {
                write!(f, "invalid SMP public key length {length}")
            }
            Self::InvalidPrivateKeyLength(length) => {
                write!(f, "invalid SMP private key length {length}")
            }
            Self::InvalidSignatureLength(length) => {
                write!(f, "invalid SMP signature length {length}")
            }
            Self::SignatureVerificationFailed => {
                write!(f, "failed to verify SMP signature")
            }
            Self::InvalidSessionIdentifier(value) => {
                write!(f, "invalid SMP session identifier `{value}`")
            }
            Self::InvalidKeyDerivationLength(length) => {
                write!(f, "invalid SMP key derivation length {length}")
            }
            Self::InvalidSecretBoxChainKeyLength(length) => {
                write!(f, "invalid SMP secretbox chain key length {length}")
            }
            Self::InvalidShortLinkIdLength(length) => {
                write!(f, "invalid SMP short-link id length {length}")
            }
            Self::InvalidShortLinkKeyLength(length) => {
                write!(f, "invalid SMP short-link key length {length}")
            }
            Self::InvalidShortLinkDataLength { field, length } => {
                write!(f, "invalid SMP short-link data `{field}` length {length}")
            }
            Self::ShortLinkDataHashMismatch => {
                write!(f, "SMP short-link data hash mismatch")
            }
            Self::AesGcmAuthenticationFailed => {
                write!(f, "failed to authenticate SMP AES-GCM payload")
            }
            Self::InvalidOfficialRatchetVersion(version) => {
                write!(f, "invalid official SMP ratchet version {version}")
            }
            Self::InvalidOfficialRatchetPadding => {
                write!(f, "invalid official SMP ratchet padding")
            }
            Self::InvalidOfficialX3dhParameters(error) => {
                write!(f, "invalid official SMP X3DH parameters: {error}")
            }
            Self::InvalidPqKeyLength(length) => {
                write!(f, "invalid SMP PQ key length {length}")
            }
            Self::InvalidPqCiphertextLength(length) => {
                write!(f, "invalid SMP PQ ciphertext length {length}")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsSimplexSmpCryptoError {}
