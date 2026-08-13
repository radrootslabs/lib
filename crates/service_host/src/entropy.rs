//! Injected entropy contract and production operating-system adapter.

use core::fmt;
use std::error::Error;

/// Failure to obtain cryptographically secure host entropy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntropyError {
    Unavailable,
}

impl fmt::Display for EntropyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("host entropy is unavailable")
    }
}

impl Error for EntropyError {}

/// Injected source of cryptographically secure bytes.
pub trait EntropySource: Send + Sync {
    /// Fills the complete destination or returns an error without partial success.
    fn fill_bytes(&self, destination: &mut [u8]) -> Result<(), EntropyError>;
}

/// Production entropy source backed by the operating system.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemEntropy;

impl EntropySource for SystemEntropy {
    fn fill_bytes(&self, destination: &mut [u8]) -> Result<(), EntropyError> {
        getrandom::getrandom(destination).map_err(|_| EntropyError::Unavailable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FailingEntropy;

    impl EntropySource for FailingEntropy {
        fn fill_bytes(&self, _destination: &mut [u8]) -> Result<(), EntropyError> {
            Err(EntropyError::Unavailable)
        }
    }

    #[test]
    fn injected_entropy_errors_are_typed_and_safe() {
        let mut destination = [0_u8; 16];
        let error = FailingEntropy
            .fill_bytes(&mut destination)
            .expect_err("entropy failure");

        assert_eq!(error, EntropyError::Unavailable);
        assert_eq!(error.to_string(), "host entropy is unavailable");
        assert_eq!(destination, [0; 16]);
    }

    #[test]
    fn production_entropy_adapter_smoke_test() {
        let mut destination = [0_u8; 32];
        SystemEntropy
            .fill_bytes(&mut destination)
            .expect("operating-system entropy");
    }
}
