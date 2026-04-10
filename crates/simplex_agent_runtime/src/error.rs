use alloc::string::String;
use core::fmt;
use radroots_simplex_agent_proto::prelude::RadrootsSimplexAgentProtoError;
use radroots_simplex_agent_store::prelude::RadrootsSimplexAgentStoreError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadrootsSimplexAgentRuntimeError {
    Proto(RadrootsSimplexAgentProtoError),
    Store(RadrootsSimplexAgentStoreError),
    MissingConfig(&'static str),
    InvalidConfig(&'static str),
    Runtime(String),
}

impl From<RadrootsSimplexAgentProtoError> for RadrootsSimplexAgentRuntimeError {
    fn from(value: RadrootsSimplexAgentProtoError) -> Self {
        Self::Proto(value)
    }
}

impl From<RadrootsSimplexAgentStoreError> for RadrootsSimplexAgentRuntimeError {
    fn from(value: RadrootsSimplexAgentStoreError) -> Self {
        Self::Store(value)
    }
}

impl fmt::Display for RadrootsSimplexAgentRuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Proto(error) => write!(f, "{error}"),
            Self::Store(error) => write!(f, "{error}"),
            Self::MissingConfig(field) => {
                write!(f, "missing SimpleX agent runtime config `{field}`")
            }
            Self::InvalidConfig(field) => {
                write!(f, "invalid SimpleX agent runtime config `{field}`")
            }
            Self::Runtime(message) => write!(f, "{message}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsSimplexAgentRuntimeError {}
