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

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    #[test]
    fn display_covers_all_error_variants() {
        let cases = [
            (
                RadrootsProtectedStoreError::EntropyUnavailable,
                "protected-store entropy is unavailable",
            ),
            (
                RadrootsProtectedStoreError::UnsupportedEnvelopeVersion(7),
                "protected-store envelope version 7 is unsupported",
            ),
            (
                RadrootsProtectedStoreError::InvalidStoreKeyLength(31),
                "protected-store key must be 32 bytes, got 31",
            ),
            (
                RadrootsProtectedStoreError::EnvelopeEncodeFailed,
                "protected-store envelope encoding failed",
            ),
            (
                RadrootsProtectedStoreError::EnvelopeDecodeFailed,
                "protected-store envelope decoding failed",
            ),
            (
                RadrootsProtectedStoreError::KeyWrapFailed,
                "protected-store key wrapping failed",
            ),
            (
                RadrootsProtectedStoreError::KeyUnwrapFailed,
                "protected-store key unwrapping failed",
            ),
            (
                RadrootsProtectedStoreError::EncryptFailed,
                "protected-store encryption failed",
            ),
            (
                RadrootsProtectedStoreError::DecryptFailed,
                "protected-store decryption failed",
            ),
        ];

        for (error, expected) in cases {
            assert_eq!(error.to_string(), expected);
        }
    }

    #[cfg(feature = "std")]
    #[test]
    fn error_trait_is_available_with_std() {
        let error = RadrootsProtectedStoreError::DecryptFailed;
        let dyn_error: &dyn std::error::Error = &error;

        assert_eq!(dyn_error.to_string(), "protected-store decryption failed");
    }
}
