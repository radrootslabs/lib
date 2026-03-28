use alloc::string::String;
use core::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadrootsSimplexAgentStoreError {
    ConnectionNotFound(String),
    QueueNotFound(String),
    CommandNotFound(u64),
    MissingPrimarySendQueue(String),
}

impl fmt::Display for RadrootsSimplexAgentStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConnectionNotFound(id) => write!(f, "SimpleX agent connection `{id}` not found"),
            Self::QueueNotFound(id) => write!(f, "SimpleX agent queue `{id}` not found"),
            Self::CommandNotFound(id) => write!(f, "SimpleX agent command `{id}` not found"),
            Self::MissingPrimarySendQueue(id) => {
                write!(
                    f,
                    "SimpleX agent connection `{id}` has no primary send queue"
                )
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsSimplexAgentStoreError {}
