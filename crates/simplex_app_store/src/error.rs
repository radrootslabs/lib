use alloc::string::String;
use core::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadrootsSimplexAppStoreError {
    MessageLifecycle(String),
}

impl fmt::Display for RadrootsSimplexAppStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MessageLifecycle(message) => {
                write!(formatter, "SimpleX app message lifecycle error: {message}")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsSimplexAppStoreError {}
