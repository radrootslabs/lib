use thiserror::Error;

/// Errors produced while validating public identity values.
#[non_exhaustive]
#[derive(Debug, Error)]
pub enum Error {
    #[error("identifier byte representation must contain {expected} bytes, but contained {actual}")]
    InvalidByteLength { expected: usize, actual: usize },

    #[error(
        "identifier hexadecimal representation must contain {expected} bytes, but contained {actual}"
    )]
    InvalidHexLength { expected: usize, actual: usize },

    #[error("identifier contains non-hexadecimal data at byte {index}")]
    InvalidHexCharacter { index: usize },

    #[error("public key bytes are not a valid secp256k1 x-only public key")]
    InvalidPublicKeyBytes,

    #[error("public identity identifier does not match its public key")]
    IdentityIdMismatch,

    #[error("username length must be between {min} and {max} ASCII bytes, but was {actual}")]
    InvalidUsernameLength {
        min: usize,
        max: usize,
        actual: usize,
    },

    #[error("username contains an invalid character at byte {index}")]
    InvalidUsernameCharacter { index: usize },

    #[error("username dots cannot be leading, trailing, or consecutive")]
    InvalidUsernameDotPlacement,
}
