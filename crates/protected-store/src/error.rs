use core::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RadrootsProtectedStoreError {
    EntropyUnavailable,
    UnsupportedEnvelopeVersion(u8),
    InvalidStoreKeyLength(usize),
    EnvelopeEncodeFailed,
    EnvelopeDecodeFailed,
    KeyWrapFailed,
    KeyUnwrapFailed,
    EncryptFailed,
    DecryptFailed,
}

impl fmt::Display for RadrootsProtectedStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EntropyUnavailable => f.write_str("protected-store entropy is unavailable"),
            Self::UnsupportedEnvelopeVersion(version) => {
                write!(
                    f,
                    "protected-store envelope version {version} is unsupported"
                )
            }
            Self::InvalidStoreKeyLength(length) => {
                write!(f, "protected-store key must be 32 bytes, got {length}")
            }
            Self::EnvelopeEncodeFailed => f.write_str("protected-store envelope encoding failed"),
            Self::EnvelopeDecodeFailed => f.write_str("protected-store envelope decoding failed"),
            Self::KeyWrapFailed => f.write_str("protected-store key wrapping failed"),
            Self::KeyUnwrapFailed => f.write_str("protected-store key unwrapping failed"),
            Self::EncryptFailed => f.write_str("protected-store encryption failed"),
            Self::DecryptFailed => f.write_str("protected-store decryption failed"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsProtectedStoreError {}
