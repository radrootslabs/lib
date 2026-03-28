use alloc::string::String;
use core::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadrootsSimplexAgentStoreError {
    ConnectionNotFound(String),
    QueueNotFound(String),
    CommandNotFound(u64),
    MissingPrimarySendQueue(String),
    PendingOutboundMessage(String),
    StagedOutboundMessageMissing(String),
    StagedOutboundMessageMismatch {
        connection_id: String,
        expected: u64,
        actual: u64,
    },
    QueueAuthStateMissing(String),
    Persistence(String),
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
            Self::PendingOutboundMessage(id) => {
                write!(
                    f,
                    "SimpleX agent connection `{id}` already has a staged outbound message"
                )
            }
            Self::StagedOutboundMessageMissing(id) => {
                write!(
                    f,
                    "SimpleX agent connection `{id}` has no staged outbound message"
                )
            }
            Self::StagedOutboundMessageMismatch {
                connection_id,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "SimpleX agent connection `{connection_id}` staged outbound message mismatch: expected `{expected}`, got `{actual}`"
                )
            }
            Self::QueueAuthStateMissing(id) => {
                write!(
                    f,
                    "SimpleX agent queue `{id}` is missing transport auth state"
                )
            }
            Self::Persistence(message) => write!(f, "{message}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsSimplexAgentStoreError {}
