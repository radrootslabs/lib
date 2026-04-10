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
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsSimplexSmpCryptoError {}
